// Reuse the producer's retained-runtime tests against the same candidate.
#define main RetainedRuntimeMain
#include "wee8-runtime-check.cc"
#undef main

#include "src/builtins/builtins-inl.h"
#include "src/builtins/moonrun-omitted-builtins.h"
#include <set>

int main(int argc, char** argv) {
  const bool retained = argc == 2 &&
      std::string(argv[1]).find("--retained=") == 0;
  if (argc != 2 || (std::string(argv[1]) != "--inspect" &&
                    std::string(argv[1]).find("--invoke=") != 0 && !retained)) {
    return RetainedRuntimeMain(argc, argv);
  }
  v8::V8::SetFlagsFromString("--harmony-set-methods --harmony-iterator-helpers --js-promise-withresolvers");
  if (retained) v8::V8::SetFlagsFromString(argv[1] + 11);
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
    // Fetch the real prototype and constructor without compiling JS source.
    v8::Local<v8::Value> iterator_constructor;
    v8::Local<v8::Value> iterator_prototype;
    v8::Local<v8::Value> promise_constructor;
    if (!context->Global()->Get(context, v8::String::NewFromUtf8Literal(
            isolate, "Iterator")).ToLocal(&iterator_constructor) ||
        !iterator_constructor.As<v8::Object>()->Get(context,
            v8::String::NewFromUtf8Literal(isolate, "prototype"))
             .ToLocal(&iterator_prototype) ||
        !context->Global()->Get(context, v8::String::NewFromUtf8Literal(
            isolate, "Promise")).ToLocal(&promise_constructor)) return 6;
    auto Iterator = iterator_prototype.As<v8::Object>();
    auto Promise = promise_constructor.As<v8::Object>();
    // Exercise the retained protocol, not the omitted iterator conveniences.
    v8::Local<v8::Value> values_method;
    v8::Local<v8::Value> iterator_value;
    v8::Local<v8::Value> next_method;
    v8::Local<v8::Value> next_result;
    if (!Array->Set(context, 0, v8::Integer::New(isolate, 42)).FromMaybe(false) ||
        !Array->Get(context, v8::String::NewFromUtf8Literal(isolate, "values"))
             .ToLocal(&values_method) ||
        !values_method.As<v8::Function>()->Call(context, Array, 0, nullptr)
             .ToLocal(&iterator_value) ||
        !iterator_value.As<v8::Object>()->Get(context,
             v8::String::NewFromUtf8Literal(isolate, "next")).ToLocal(&next_method) ||
        !next_method.As<v8::Function>()->Call(context, iterator_value, 0, nullptr)
             .ToLocal(&next_result)) return 7;
    v8::Local<v8::Value> yielded;
    if (!next_result.As<v8::Object>()->Get(context,
             v8::String::NewFromUtf8Literal(isolate, "value")).ToLocal(&yielded) ||
        yielded->Int32Value(context).FromMaybe(0) != 42) return 7;

    // Promise combinators are omitted; native resolution, reactions and the
    // microtask queue must still work for async Wasm and host callbacks.
    int resolved = 0;
    auto callback = v8::Function::New(context,
        [](const v8::FunctionCallbackInfo<v8::Value>& args) {
          auto* result = static_cast<int*>(args.Data().As<v8::External>()->Value());
          *result = args[0]->Int32Value(args.GetIsolate()->GetCurrentContext())
                        .FromMaybe(0);
        }, v8::External::New(isolate, &resolved)).ToLocalChecked();
    auto resolver = v8::Promise::Resolver::New(context).ToLocalChecked();
    v8::Local<v8::Promise> chained;
    if (!resolver->GetPromise()->Then(context, callback).ToLocal(&chained) ||
        !resolver->Resolve(context, v8::Integer::New(isolate, 42))
             .FromMaybe(false)) return 8;
    isolate->PerformMicrotaskCheckpoint();
    if (resolved != 42 || chained->State() != v8::Promise::kFulfilled) return 8;

    // A JS host import returns an iterator for two Wasm results. This covers
    // IterableToFixedArrayForWasm and next(), beyond ordinary Array iteration.
    constexpr uint8_t multi_result_bytes[] = {
        0, 97, 115, 109, 1, 0, 0, 0,
        1, 10, 2, 96, 0, 2, 127, 127, 96, 0, 1, 127,
        2, 12, 1, 3, 'e', 'n', 'v', 4, 'p', 'a', 'i', 'r', 0, 0,
        3, 2, 1, 1, 7, 7, 1, 3, 'r', 'u', 'n', 0, 1,
        10, 7, 1, 5, 0, 16, 0, 106, 11};
    auto pair = v8::Function::New(context,
        [](const v8::FunctionCallbackInfo<v8::Value>& args) {
          auto* isolate = args.GetIsolate();
          auto context = isolate->GetCurrentContext();
          auto array = v8::Array::New(isolate, 2);
          array->Set(context, 0, v8::Integer::New(isolate, 20)).Check();
          array->Set(context, 1, v8::Integer::New(isolate, 22)).Check();
          auto values = array->Get(context,
              v8::String::NewFromUtf8Literal(isolate, "values"))
                  .ToLocalChecked().As<v8::Function>();
          args.GetReturnValue().Set(
              values->Call(context, array, 0, nullptr).ToLocalChecked());
        }).ToLocalChecked();
    auto env = v8::Object::New(isolate);
    env->Set(context, v8::String::NewFromUtf8Literal(isolate, "pair"), pair).Check();
    auto imports = v8::Object::New(isolate);
    imports->Set(context, v8::String::NewFromUtf8Literal(isolate, "env"), env).Check();
    auto module = v8::WasmModuleObject::Compile(isolate,
        v8::MemorySpan<const uint8_t>(multi_result_bytes, sizeof(multi_result_bytes)))
            .ToLocalChecked();
    auto wasm = context->Global()->Get(context,
        v8::String::NewFromUtf8Literal(isolate, "WebAssembly"))
            .ToLocalChecked().As<v8::Object>();
    auto constructor = wasm->Get(context,
        v8::String::NewFromUtf8Literal(isolate, "Instance"))
            .ToLocalChecked().As<v8::Function>();
    v8::Local<v8::Value> instance_args[] = {module, imports};
    auto instance = constructor->NewInstance(context, 2, instance_args).ToLocalChecked();
    auto exports = instance->Get(context,
        v8::String::NewFromUtf8Literal(isolate, "exports"))
            .ToLocalChecked().As<v8::Object>();
    auto run = exports->Get(context, v8::String::NewFromUtf8Literal(isolate, "run"))
                   .ToLocalChecked().As<v8::Function>();
    v8::Local<v8::Value> sum;
    if (!run->Call(context, v8::Undefined(isolate), 0, nullptr).ToLocal(&sum) ||
        sum->Int32Value(context).FromMaybe(0) != 42) return 9;

    auto global_object = [&](const char* name) {
      return context->Global()->Get(context,
          v8::String::NewFromUtf8(isolate, name).ToLocalChecked())
              .ToLocalChecked().As<v8::Object>();
    };
    auto ArrayConstructor = global_object("Array");
    auto MapConstructor = global_object("Map");
    auto ObjectConstructor = global_object("Object");
    auto DateConstructor = global_object("Date");
    auto Reflect = global_object("Reflect");
    auto JSON = global_object("JSON");
    auto Date = v8::Date::New(context, 0).ToLocalChecked().As<v8::Object>();
    auto Number = v8::NumberObject::New(isolate, 42).As<v8::Object>();
    auto String = v8::StringObject::New(isolate,
        v8::String::NewFromUtf8Literal(isolate, "abc")).As<v8::Object>();
    // JS convenience entries are separate from the embedding API and coercion
    // primitives. Keep native JSON, property enumeration and Date coercion live.
    v8::Local<v8::Value> parsed;
    v8::Local<v8::String> serialized;
    v8::Local<v8::Array> names;
    if (!v8::JSON::Parse(context,
            v8::String::NewFromUtf8Literal(isolate, "{\"x\":42}"))
             .ToLocal(&parsed) ||
        !v8::JSON::Stringify(context, parsed).ToLocal(&serialized) ||
        !parsed.As<v8::Object>()->GetOwnPropertyNames(context).ToLocal(&names) ||
        names->Length() != 1 ||
        Date->NumberValue(context).FromMaybe(-1) != 0) return 11;
    v8::String::Utf8Value json_text(isolate, serialized);
    if (!*json_text || std::string(*json_text) != "{\"x\":42}") return 11;
    std::set<v8::internal::Address> identities;
    const bool inspect = std::string(argv[1]) == "--inspect";
#define CHECK_BUILTIN(Name, Receiver, Method)                                  \
    {                                                                          \
      auto id = v8::internal::Builtin::k##Name;                                  \
      auto code = internal->builtins()->code(id);                               \
      if (v8::internal::Builtins::KindOf(id) == v8::internal::Builtins::CPP && \
          v8::internal::Builtins::CppEntryOf(id) == 0) return 10;                \
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
                    (v8::internal::Builtins::KindOf(id) == v8::internal::Builtins::TFJ \
                         ? v8::internal::Builtins::GetStackParameterCount(id) : -1),         \
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
    if (!retained) {
      MOONRUN_OMITTED_JS_BUILTINS(CHECK_BUILTIN)
    }
#undef CHECK_BUILTIN
    if (!inspect && !invoked && !retained) status = 5;
  }
  isolate->Dispose();
  v8::V8::Dispose();
  v8::V8::DisposePlatform();
  return status;
}
