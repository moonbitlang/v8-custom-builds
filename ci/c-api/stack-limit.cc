// Exercise the native stack guard through the packaged release ABI.
#undef NDEBUG
#include <cstdint>

#include "wee8.h"

static wasm_trap_t* tick(void* data, const wasm_val_vec_t*, wasm_val_vec_t*) {
  ++*static_cast<size_t*>(data);
  return nullptr;
}

extern "C" void wee8_stack_limit_probe(wasm_engine_t* engine,
                                       wasm_store_t* store,
                                       const uint8_t* bytes, size_t len) {
  wasm_byte_vec_t binary = {
      len, reinterpret_cast<char*>(const_cast<uint8_t*>(bytes))};
  auto* module = wasm_module_new(store, &binary);
  assert(module);
  size_t ticks = 0;
  auto* type = wasm_functype_new_0_0();
  auto* host = wasm_func_new_with_env(store, type, tick, &ticks, nullptr);
  wasm_functype_delete(type);
  wasm_extern_t* entries[] = {wasm_func_as_extern(host)};
  wasm_extern_vec_t imports = {1, entries};
  wasm_trap_t* trap = nullptr;
  auto* instance = wasm_instance_new(store, module, &imports, &trap);
  assert(instance && !trap);
  wasm_extern_vec_t exports;
  wasm_instance_exports(instance, &exports);
  assert(exports.size == 1);
  auto* recurse = wasm_extern_as_func(exports.data[0]);

  // Both limits are inside this thread's existing native stack. They change
  // the Wasm trap boundary, not the OS allocation; restore the old limit below.
  uint8_t marker;
  uintptr_t small = reinterpret_cast<uintptr_t>(&marker) - 128 * 1024;
  uintptr_t large = reinterpret_cast<uintptr_t>(&marker) - 256 * 1024;
  uintptr_t previous = wee8_store_set_stack_limit(store, small);
  assert(previous && !wee8_store_set_stack_limit(store, 0));
  auto* second = wasm_store_new(engine);
  auto other_previous = wee8_store_set_stack_limit(second, large);
  assert(other_previous);
  assert(wee8_store_set_stack_limit(second, other_previous) == large);
  wasm_store_delete(second);
  wasm_val_t depth = WASM_I32_VAL(100000);
  wasm_val_t answer = WASM_I32_VAL(0);
  wasm_val_vec_t argument = {1, &depth}, result = {1, &answer};
  trap = wasm_func_call(recurse, &argument, &result);
  assert(trap && !wee8_trap_is_exception(trap));
  wasm_trap_delete(trap);
  size_t small_depth = ticks;
  assert(small_depth > 0);
  assert(wee8_store_set_stack_limit(store, large) == small);
  ticks = 0;
  trap = wasm_func_call(recurse, &argument, &result);
  assert(trap && ticks > small_depth);
  wasm_trap_delete(trap);
  assert(wee8_store_set_stack_limit(store, previous) == large);
  depth.of.i32 = 5;
  assert(!wasm_func_call(recurse, &argument, &result) && answer.of.i32 == 5);

  wasm_extern_vec_delete(&exports);
  wasm_instance_delete(instance);
  wasm_func_delete(host);
  wasm_module_delete(module);
}
