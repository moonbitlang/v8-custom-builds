#include "gc-bytes.h"
#include "include/libplatform/libplatform.h"
#include "include/v8.h"
#include "src/api/api-inl.h"
#include "src/wasm/wasm-engine.h"
#include "src/wasm/wasm-objects-inl.h"
#include "src/wasm/wasm-result.h"
#include <cstdint>
#include <cstdio>
#include <memory>

int main(int argc, char **argv) {
  if (argc > 1)
    v8::V8::SetFlagsFromString(argv[1]);
  v8::V8::SetFlagsFromString("--allow-natives-syntax");
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
    // (module (func) (start 0)). Instantiation must enter Wasm directly:
    // the old JS start-function wrapper aborts under this execution guard.
    constexpr uint8_t start_bytes[] = {
        0, 97, 115, 109, 1, 0, 0, 0, 1, 4, 1, 96, 0, 0,
        3, 2, 1, 0, 8, 1, 0, 10, 4, 1, 2, 0, 11};
    v8::Local<v8::WasmModuleObject> start_module;
    if (!v8::WasmModuleObject::Compile(isolate,
                                     {start_bytes, sizeof(start_bytes)})
             .ToLocal(&start_module)) {
      status = 8;
    } else {
      auto* internal = reinterpret_cast<v8::internal::Isolate*>(isolate);
      v8::internal::wasm::ErrorThrower thrower(internal, "native start");
      v8::Isolate::DisallowJavascriptExecutionScope no_js(
          isolate, v8::Isolate::DisallowJavascriptExecutionScope::CRASH_ON_FAILURE);
      if (v8::internal::wasm::GetWasmEngine()
              ->SyncInstantiate(internal, &thrower,
                                v8::internal::Cast<v8::internal::WasmModuleObject>(
                                    v8::Utils::OpenHandle(*start_module)),
                                {}, {})
              .is_null())
        status = 8;
    }
    // Check real registration removal, independently of the Wasm consumer.
    auto source = v8::String::NewFromUtf8Literal(
        isolate,
        "!%IsTurbofanEnabled() && !%IsMaglevEnabled() && "
        "!%IsSparkplugEnabled() && "
        "typeof Intl === 'undefined' && "
        "typeof RegExp === 'undefined' && "
        "['match','matchAll','search','replace','replaceAll','split'].every("
        "name => typeof String.prototype[name] === 'undefined') && "
        "'abc'.includes('b') && 'abc'.startsWith('a') && 'abc'.endsWith('c') "
        "&& "
        "['includes','startsWith','endsWith'].every(name => { "
        "try { 'abc'[name]({[Symbol.match]: true}); return false; } "
        "catch (error) { return error instanceof TypeError; } }) && "
        "'abc'.includes({[Symbol.match]: false, toString() { return 'b'; }}) "
        "&& "
        "new Uint8Array([1,2]).includes(2) && "
        "!new Uint8Array([1,2]).includes(3) && "
        "typeof Uint8Array.prototype.startsWith === 'undefined' && "
        "typeof Uint8Array.prototype.endsWith === 'undefined'");
    v8::Local<v8::Script> script;
    v8::Local<v8::Value> result;
    if (!v8::Script::Compile(context, source).ToLocal(&script) ||
        !script->Run(context).ToLocal(&result) || !result->IsTrue())
      status = 2;
    auto asm_source = v8::String::NewFromUtf8Literal(
        isolate, "function asmModule() { 'use asm'; function add(a,b) { "
                 "a=a|0; b=b|0; return (a+b)|0; } return {add:add}; } "
                 "asmModule().add(20,22) === 42 && !%IsAsmWasmCode(asmModule)");
    if (!v8::Script::Compile(context, asm_source).ToLocal(&script) ||
        !script->Run(context).ToLocal(&result) || !result->IsTrue())
      status = 6;
    auto wasm_source = v8::String::NewFromUtf8Literal(
        isolate,
        "const run = new WebAssembly.Instance(new WebAssembly.Module(new "
        "Uint8Array("
        "[0,97,115,109,1,0,0,0,1,5,1,96,0,1,127,3,2,1,0,7,7,1,3,114,117,110,"
        "0,0,10,6,1,4,0,65,42,11]))).exports.run; "
        "let answer = 0; for (let i = 0; i < 100; i++) answer = run(); answer "
        "=== 42");
    if (!v8::Script::Compile(context, wasm_source).ToLocal(&script) ||
        !script->Run(context).ToLocal(&result) || !result->IsTrue())
      status = 5;
    // Keep a Wasm struct inside a Wasm array live across a native GC request.
    auto collect =
        v8::Function::New(context,
                          [](const v8::FunctionCallbackInfo<v8::Value> &args) {
                            args.GetIsolate()->LowMemoryNotification();
                          })
            .ToLocalChecked();
    context->Global()
        ->Set(context, v8::String::NewFromUtf8Literal(isolate, "collect"),
              collect)
        .Check();
    v8::Local<v8::WasmModuleObject> gc_module;
    if (!v8::WasmModuleObject::Compile(isolate, {kWasmGc, sizeof(kWasmGc)})
             .ToLocal(&gc_module)) {
      status = 7;
    } else {
      context->Global()
          ->Set(context, v8::String::NewFromUtf8Literal(isolate, "gcModule"),
                gc_module)
          .Check();
      auto gc_source = v8::String::NewFromUtf8Literal(
          isolate, "new WebAssembly.Instance(gcModule, "
                   "{env:{collect}}).exports.run() === 42");
      if (!v8::Script::Compile(context, gc_source).ToLocal(&script) ||
          !script->Run(context).ToLocal(&result) || !result->IsTrue())
        status = 7;
    }
    {
      v8::TryCatch caught(isolate);
      auto literal = v8::String::NewFromUtf8Literal(isolate, "/x/");
      if (!v8::Script::Compile(context, literal).IsEmpty() ||
          !caught.HasCaught())
        status = 3;
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
    std::puts(
        "Runtime cuts, retained predicates, numeric Wasm and WasmGC passed");
  if (status != 0)
    std::fprintf(stderr, "Runtime check failed: %d\n", status);
  return status;
}
