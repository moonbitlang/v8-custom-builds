#include "include/libplatform/libplatform.h"
#include "include/v8.h"
#include "src/api/api-inl.h"
#include "src/flags/flags.h"
#include "src/runtime/runtime.h"
#include <cstdint>
#include <cstdio>
#include <initializer_list>
#include <memory>
#include <string>
#include <utility>

static_assert(!V8_ENABLE_ASMJS, "The custom runtime must exclude asm.js");
#ifndef V8_CUSTOM_WEE8
#error The runtime probe requires the native Wasm context profile
#endif

int main(int argc, char **argv) {
  if (argc > 1)
    v8::V8::SetFlagsFromString(argv[1]);
  auto platform = v8::platform::NewDefaultPlatform();
  v8::V8::InitializePlatform(platform.get());
  if (!v8::V8::Initialize())
    return 1;
  auto allocator = std::unique_ptr<v8::ArrayBuffer::Allocator>(
      v8::ArrayBuffer::Allocator::NewDefaultAllocator());
  v8::Isolate::CreateParams params;
  params.array_buffer_allocator = allocator.get();
  v8::Isolate *isolate = v8::Isolate::New(params);
  int status = 0;
  {
    v8::Isolate::Scope entered(isolate);
    v8::HandleScope handles(isolate);
    auto context = v8::Context::New(isolate);
    v8::Context::Scope entered_context(context);
    if (v8::internal::v8_flags.turbofan || v8::internal::v8_flags.maglev ||
        v8::internal::v8_flags.sparkplug ||
        v8::internal::v8_flags.validate_asm ||
        v8::internal::v8_flags.script_streaming)
      status = 2;
    {
      // Source rejection alone cannot prove that the asm.js runtime was cut.
      // Call the retained runtime ID directly: it must reject before decoding
      // arguments or attempting the old lazy-JS fallback.
      auto *internal = reinterpret_cast<v8::internal::Isolate *>(isolate);
      auto result = v8::internal::Runtime_InstantiateAsmJs(0, nullptr, internal);
      if (result != v8::internal::ReadOnlyRoots(internal).exception().ptr() ||
          !internal->has_exception()) {
        status = 2;
      } else {
        auto exception = v8::Utils::ToLocal(
            v8::internal::handle(internal->exception(), internal));
        internal->clear_exception();
        v8::String::Utf8Value message(isolate, exception);
        if (!*message ||
            std::string(*message) != "TypeError: asm.js is disabled")
          status = 2;
      }
    }
    // No test setup may compile JavaScript: exercise Wasm through native APIs.
    for (const char *text : {"42", "function f() { return 42; }", "/x/"}) {
      v8::TryCatch caught(isolate);
      auto source = v8::String::NewFromUtf8(isolate, text).ToLocalChecked();
      if (!v8::Script::Compile(context, source).IsEmpty() ||
          !caught.HasCaught())
        status = 2;
      else {
        v8::String::Utf8Value message(isolate, caught.Exception());
        if (!*message ||
            std::string(*message).find(
                "JavaScript source parsing is disabled") == std::string::npos)
          status = 2;
      }
    }
    {
      v8::TryCatch caught(isolate);
      auto text = v8::String::NewFromUtf8Literal(isolate, "export default 42");
      v8::ScriptOrigin origin(v8::String::NewFromUtf8Literal(isolate, "test"),
                              0, 0, false, -1, {}, false, false, true);
      v8::ScriptCompiler::Source source(text, origin);
      if (!v8::ScriptCompiler::CompileModule(isolate, &source).IsEmpty() ||
          !caught.HasCaught())
        status = 3;
      else {
        v8::String::Utf8Value message(isolate, caught.Exception());
        if (!*message ||
            std::string(*message).find(
                "JavaScript source parsing is disabled") == std::string::npos)
          status = 3;
      }
    }
    {
      v8::TryCatch caught(isolate);
      v8::ScriptCompiler::Source source(
          v8::String::NewFromUtf8Literal(isolate, "return 42"));
      if (!v8::ScriptCompiler::CompileFunction(context, &source).IsEmpty() ||
          !caught.HasCaught())
        status = 3;
      else {
        v8::String::Utf8Value message(isolate, caught.Exception());
        if (!*message ||
            std::string(*message).find(
                "JavaScript source parsing is disabled") == std::string::npos)
          status = 3;
      }
    }
    {
      class EmptyStream final
          : public v8::ScriptCompiler::ExternalSourceStream {
      public:
        bool read = false;
        size_t GetMoreData(const uint8_t **src) override {
          read = true;
          *src = nullptr;
          return 0;
        }
      };
      auto stream = std::make_unique<EmptyStream>();
      auto *stream_ptr = stream.get();
      v8::ScriptCompiler::StreamedSource source(
          std::move(stream),
          v8::ScriptCompiler::StreamedSource::UTF8);
      std::unique_ptr<v8::ScriptCompiler::ScriptStreamingTask> task(
          v8::ScriptCompiler::StartStreaming(isolate, &source));
      if (task || stream_ptr->read)
        status = 3;
    }
    // No library globals are installed. Private maps still back Wasm memory,
    // native callbacks and error objects; their behavior is checked via C API.
    auto names = context->Global()->GetOwnPropertyNames(
        context, v8::ALL_PROPERTIES, v8::KeyConversionMode::kConvertToString)
        .ToLocalChecked();
    if (names->Length() != 0) status = 4;
    isolate->LowMemoryNotification();
    {
      v8::TryCatch caught(isolate);
      auto pattern = v8::String::NewFromUtf8Literal(isolate, "x");
      if (!v8::RegExp::New(context, pattern, v8::RegExp::kNone).IsEmpty() ||
          !caught.HasCaught())
        status = 4;
    }
  }
  isolate->Dispose();
  v8::V8::Dispose();
  v8::V8::DisposePlatform();
  if (status == 0)
    std::puts("Private native context and JavaScript source rejection passed");
  if (status != 0)
    std::fprintf(stderr, "Runtime check failed: %d\n", status);
  return status;
}
