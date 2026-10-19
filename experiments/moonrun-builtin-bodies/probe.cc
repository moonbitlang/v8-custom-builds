// Reuse the producer's retained-runtime tests against the same candidate.
#define main RetainedRuntimeMain
#include "wee8-runtime-check.cc"
#undef main

#include "src/builtins/builtins-inl.h"
#include "src/builtins/moonrun-omitted-builtins.h"
#include <set>

int main(int argc, char** argv) {
  if (argc != 2 || (std::string(argv[1]) != "--inspect" &&
                    std::string(argv[1]).find("--invoke=") != 0)) {
    return RetainedRuntimeMain(argc, argv);
  }
  v8::V8::SetFlagsFromString("--harmony-set-methods");
  auto platform = v8::platform::NewDefaultPlatform();
  v8::V8::InitializePlatform(platform.get());
  if (!v8::V8::Initialize()) return 1;
  auto allocator = std::unique_ptr<v8::ArrayBuffer::Allocator>(
      v8::ArrayBuffer::Allocator::NewDefaultAllocator());
  v8::Isolate::CreateParams params;
  params.array_buffer_allocator = allocator.get();
  auto* isolate = v8::Isolate::New(params);
  int status = 0;
  bool invoked = false;
  {
    v8::Isolate::Scope entered(isolate);
    v8::HandleScope handles(isolate);
    auto context = v8::Context::New(isolate);
    v8::Context::Scope entered_context(context);
    auto* internal = reinterpret_cast<v8::internal::Isolate*>(isolate);
    v8::Local<v8::Object> Array = v8::Array::New(isolate);
    v8::Local<v8::Object> TypedArray = v8::Uint8Array::New(
        v8::ArrayBuffer::New(isolate, 8), 0, 8);
    v8::Local<v8::Object> Set = v8::Set::New(isolate);
    std::set<v8::internal::Address> identities;
    const bool inspect = std::string(argv[1]) == "--inspect";
#define CHECK_BUILTIN(Name, Receiver, Method)                                  \
    {                                                                          \
      auto id = v8::internal::Builtin::k##Name;                                  \
      auto code = internal->builtins()->code(id);                               \
      auto key = v8::String::NewFromUtf8Literal(isolate, Method);                \
      v8::Local<v8::Value> value;                                               \
      if (code->builtin_id() != id || !identities.insert(code.ptr()).second ||   \
          code == internal->builtins()->code(v8::internal::Builtin::kIllegal) ||\
          !Receiver->Get(context, key).ToLocal(&value) || !value->IsFunction()) \
        return 2;                                                              \
      auto function = value.As<v8::Function>();                                 \
      if (v8::internal::Cast<v8::internal::JSFunction>(                         \
              *v8::Utils::OpenDirectHandle(*function))->shared()->builtin_id() != id)  \
        return 3;                                                              \
      if (inspect) {                                                           \
        std::printf("%s,%d,%d,%u,%d\n", #Name, static_cast<int>(id),             \
                    v8::internal::Builtins::GetStackParameterCount(id),         \
                    code->parameter_count(), code->instruction_size());         \
      } else if (std::string(argv[1]) == "--invoke=" #Name) {                    \
        invoked = true;                                                        \
        v8::TryCatch caught(isolate);                                          \
        std::fprintf(stderr, "invoking:%s\n", #Name);                           \
        std::fflush(stderr);                                                   \
        auto result = function->Call(context, Receiver, 0, nullptr);            \
        if (result.IsEmpty() && !caught.HasCaught()) status = 4;                 \
      }                                                                        \
    }
    MOONRUN_OMITTED_JS_BUILTINS(CHECK_BUILTIN)
#undef CHECK_BUILTIN
    if (!inspect && !invoked) status = 5;
  }
  isolate->Dispose();
  v8::V8::Dispose();
  v8::V8::DisposePlatform();
  return status;
}
