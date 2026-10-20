// THROWAWAY: link-reachability experiment, not a replacement runtime API.
#include <cstdio>
#include <memory>
#if PROBE == 4 || PROBE == 5
#include "guest-bytes.h"
#include "../../ci/c-api/probe.cc"
#include "../../ci/runtime/gc-bytes.h"
#include "include/v8.h"
#elif PROBE > 0
#include "include/libplatform/libplatform.h"
#include "include/v8.h"
#endif
int main(int argc, char** argv) {
#if PROBE == 4 || PROBE == 5
  if (argc > 1) v8::V8::SetFlagsFromString(argv[1]);
#endif
#if PROBE == 5
  auto* engine = wasm_engine_new();
  auto* store = wasm_store_new(engine);
  assert(engine && store);
  int collections = 0;
  auto* type = wasm_functype_new_0_0();
  auto* collect = wasm_func_new_with_env(
      store, type,
      [](void* data, const wasm_val_vec_t*, wasm_val_vec_t*) -> wasm_trap_t* {
        auto* isolate = v8::Isolate::GetCurrent();
        assert(isolate);
        isolate->LowMemoryNotification();
        ++*static_cast<int*>(data);
        return nullptr;
      },
      &collections, nullptr);
  wasm_functype_delete(type);
  wasm_byte_vec_t bytes = {
      sizeof(kWasmGc), reinterpret_cast<char*>(const_cast<uint8_t*>(kWasmGc))};
  auto* module = wasm_module_new(store, &bytes);
  assert(module && collect);
  int finalized = 0;
  wasm_ref_set_host_info_with_finalizer(
      wasm_module_as_ref(module), &finalized,
      [](void* data) { ++*static_cast<int*>(data); });
  assert(wasm_ref_get_host_info(wasm_module_as_ref(module)) == &finalized);
  wasm_extern_t* entries[] = {wasm_func_as_extern(collect)};
  wasm_extern_vec_t imports = {1, entries};
  wasm_trap_t* trap = nullptr;
  auto* instance = wasm_instance_new(store, module, &imports, &trap);
  assert(instance && !trap);
  wasm_extern_vec_t exports;
  wasm_instance_exports(instance, &exports);
  assert(exports.size == 1);
  auto* run = wasm_extern_as_func(exports.data[0]);
  assert(run);
  wasm_val_vec_t args = {0, nullptr};
  wasm_val_t result = WASM_I32_VAL(0);
  wasm_val_vec_t results = {1, &result};
  for (int i = 0; i < 100; ++i) {
    assert(!wasm_func_call(run, &args, &results));
    assert(result.kind == WASM_I32 && result.of.i32 == 42);
  }
  assert(collections == 100);
  wasm_extern_vec_delete(&exports);
  wasm_instance_delete(instance);
  wasm_func_delete(collect);
  wasm_module_delete(module);
  wasm_store_delete(store);
  assert(finalized == 1);
  wasm_engine_delete(engine);
#elif PROBE == 4
  wee8_probe(guest, sizeof(guest), start_guest, sizeof(start_guest));
#elif PROBE > 0
  auto platform = v8::platform::NewDefaultPlatform();
  v8::V8::InitializePlatform(platform.get());
  if (!v8::V8::Initialize()) return 1;
#if PROBE >= 2
  auto allocator = std::unique_ptr<v8::ArrayBuffer::Allocator>(
      v8::ArrayBuffer::Allocator::NewDefaultAllocator());
  v8::Isolate::CreateParams params;
  params.array_buffer_allocator = allocator.get();
  auto* isolate = v8::Isolate::New(params);
  if (!isolate) return 2;
  {
    v8::Isolate::Scope entered(isolate);
    v8::HandleScope handles(isolate);
#if PROBE >= 3
    auto context = v8::Context::New(isolate);
    if (context.IsEmpty()) return 3;
    v8::Context::Scope entered_context(context);
#endif
  }
  isolate->Dispose();
#endif
  v8::V8::Dispose();
  v8::V8::DisposePlatform();
#endif
  std::printf("probe %d passed\n", PROBE);
}
