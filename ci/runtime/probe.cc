#include "gc-bytes.h"
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
    auto get = [&](v8::Local<v8::Object> object, const char *name) {
      return object
          ->Get(context,
                v8::String::NewFromUtf8(isolate, name).ToLocalChecked())
          .ToLocalChecked();
    };
    // These builtins are retained public APIs, but their source-text paths
    // must reach the same rejection boundary as ScriptCompiler.
    for (const char *name : {"eval", "Function"}) {
      v8::TryCatch caught(isolate);
      auto value = get(context->Global(), name);
      if (!value->IsFunction()) {
        status = 3;
        continue;
      }
      v8::Local<v8::Value> args[] = {
          v8::String::NewFromUtf8Literal(isolate, "42")};
      if (!value.As<v8::Function>()
               ->Call(context, context->Global(), 1, args)
               .IsEmpty() ||
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
    if (!get(context->Global(), "RegExp")->IsUndefined() ||
        !get(context->Global(), "Intl")->IsUndefined())
      status = 4;

    auto wasm = get(context->Global(), "WebAssembly").As<v8::Object>();
    auto instance_constructor = get(wasm, "Instance").As<v8::Function>();
    constexpr uint8_t numeric_bytes[] = {
        0, 97, 115, 109, 1, 0,   0,   0,   1, 5, 1,  96, 0, 1, 127, 3,  2,  1,
        0, 7,  7,   1,   3, 114, 117, 110, 0, 0, 10, 6,  1, 4, 0,   65, 42, 11};
    auto collect =
        v8::Function::New(context,
                          [](const v8::FunctionCallbackInfo<v8::Value> &args) {
                            args.GetIsolate()->LowMemoryNotification();
                          })
            .ToLocalChecked();
    auto env = v8::Object::New(isolate);
    env->Set(context, v8::String::NewFromUtf8Literal(isolate, "collect"),
             collect)
        .Check();
    auto imports = v8::Object::New(isolate);
    imports->Set(context, v8::String::NewFromUtf8Literal(isolate, "env"), env)
        .Check();
    // The GC module retains a struct in an array across the collect callback.
    for (auto bytes :
         {v8::MemorySpan<const uint8_t>(numeric_bytes, sizeof(numeric_bytes)),
          v8::MemorySpan<const uint8_t>(kWasmGc, sizeof(kWasmGc))}) {
      v8::Local<v8::WasmModuleObject> module;
      if (!v8::WasmModuleObject::Compile(isolate, bytes).ToLocal(&module)) {
        status = 5;
        continue;
      }
      v8::Local<v8::Value> args[] = {module, imports};
      v8::Local<v8::Object> instance;
      if (!instance_constructor->NewInstance(context, 2, args)
               .ToLocal(&instance)) {
        status = 5;
        continue;
      }
      auto run = get(get(instance, "exports").As<v8::Object>(), "run")
                     .As<v8::Function>();
      for (int n = 0; n < 100; ++n) {
        v8::Local<v8::Value> result;
        if (!run->Call(context, v8::Undefined(isolate), 0, nullptr)
                 .ToLocal(&result) ||
            result->Int32Value(context).FromMaybe(0) != 42)
          status = 7;
      }
    }
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
    std::puts("JavaScript source rejection, numeric Wasm and "
              "WasmGC passed");
  if (status != 0)
    std::fprintf(stderr, "Runtime check failed: %d\n", status);
  return status;
}
