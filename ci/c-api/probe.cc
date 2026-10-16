// Compile against the packaged header, then link only the packaged wee8 archive.
// Keep assertions active in release builds: this executable is a smoke test.
#undef NDEBUG
#include "wasm.h"
#include <initializer_list>

struct HostState {
  int calls = 0;
  int finalized = 0;
};

static wasm_trap_t* sum(void* env, const wasm_val_vec_t* args,
                        wasm_val_vec_t* results) {
  assert(args->size == 2 && results->size == 1);
  assert(args->data[0].kind == WASM_I32 && args->data[1].kind == WASM_I32);
  static_cast<HostState*>(env)->calls++;
  results->data[0] = WASM_I32_VAL(args->data[0].of.i32 + args->data[1].of.i32);
  return nullptr;
}

static void finalize(void* env) {
  static_cast<HostState*>(env)->finalized++;
}

static wasm_trap_t* increment(const wasm_val_vec_t* args,
                              wasm_val_vec_t* results) {
  assert(args->size == 1 && results->size == 1);
  assert(args->data[0].kind == WASM_I32);
  results->data[0] = WASM_I32_VAL(args->data[0].of.i32 + 1);
  return nullptr;
}

extern "C" void wee8_probe(const uint8_t* bytes, size_t len,
                           const uint8_t* start_bytes, size_t start_len,
                           const uint8_t* imported_start_bytes, size_t imported_start_len) {
  wasm_engine_t* engine = wasm_engine_new();
  assert(engine);
  wasm_store_t* store = wasm_store_new(engine);
  assert(store);
  wasm_byte_vec_t binary = {len, reinterpret_cast<char*>(const_cast<uint8_t*>(bytes))};
  wasm_module_t* module = wasm_module_new(store, &binary);
  assert(module);

  // A trapped module start is returned as a trap without writing to host stdio.
  wasm_byte_vec_t start_binary = {
      start_len, reinterpret_cast<char*>(const_cast<uint8_t*>(start_bytes))};
  wasm_module_t* start_module = wasm_module_new(store, &start_binary);
  assert(start_module);
  wasm_extern_vec_t no_imports = {0, nullptr};
  wasm_trap_t* start_trap = nullptr;
  assert(!wasm_instance_new(store, start_module, &no_imports, &start_trap));
  assert(start_trap);
  wasm_trap_delete(start_trap);
  wasm_module_delete(start_module);

  HostState state;
  wasm_functype_t* type = wasm_functype_new_2_1(
      wasm_valtype_new_i32(), wasm_valtype_new_i32(), wasm_valtype_new_i32());
  wasm_func_t* host = wasm_func_new_with_env(store, type, sum, &state, finalize);
  assert(host);
  wasm_functype_delete(type);
  assert(wasm_func_param_arity(host) == 2 && wasm_func_result_arity(host) == 1);

  wasm_extern_vec_t empty_imports = {0, nullptr};
  wasm_trap_t* trap = nullptr;
  assert(!wasm_instance_new(store, module, &empty_imports, &trap));
  assert(trap);
  wasm_trap_delete(trap);
  trap = nullptr;
  wasm_extern_t* imports_data[] = {wasm_func_as_extern(host)};
  wasm_extern_vec_t imports = {1, imports_data};
  wasm_instance_t* instance = wasm_instance_new(store, module, &imports, &trap);
  assert(instance && !trap);
  wasm_module_delete(module);
  wasm_func_delete(host);

  // The guest retains the callback after its original host handle is deleted.
  wasm_extern_vec_t exports;
  wasm_instance_exports(instance, &exports);
  assert(exports.size == 5);
  wasm_func_t* run = wasm_extern_as_func(exports.data[0]);
  wasm_memory_t* memory = wasm_extern_as_memory(exports.data[1]);
  wasm_func_t* fail = wasm_extern_as_func(exports.data[2]);
  assert(run && memory && fail);
  wasm_val_t arg = WASM_I32_VAL(41);
  wasm_val_t result = WASM_I32_VAL(0);
  wasm_val_vec_t args = {1, &arg};
  wasm_val_vec_t results = {1, &result};
  assert(!wasm_func_call(run, &args, &results));
  assert(result.kind == WASM_I32 && result.of.i32 == 42 && state.calls == 1);

  wasm_val_vec_t empty = {0, nullptr};
  // Imported starts must use the exporting instance's implicit argument.
  // Check both a successful start/re-export call and a trap across instances.
  wasm_byte_vec_t imported_binary = {
      imported_start_len,
      reinterpret_cast<char*>(const_cast<uint8_t*>(imported_start_bytes))};
  wasm_module_t* imported_module = wasm_module_new(store, &imported_binary);
  assert(imported_module);
  for (int index : {3, 2}) {
    wasm_extern_t* imported_data[] = {exports.data[index]};
    wasm_extern_vec_t imported = {1, imported_data};
    wasm_trap_t* imported_trap = nullptr;
    wasm_instance_t* started =
        wasm_instance_new(store, imported_module, &imported, &imported_trap);
    if (index == 3) {
      assert(started && !imported_trap);
      wasm_extern_vec_t started_exports;
      wasm_instance_exports(started, &started_exports);
      assert(started_exports.size == 1);
      assert(!wasm_func_call(wasm_extern_as_func(started_exports.data[0]),
                            &empty, &empty));
      assert(!wasm_func_call(wasm_extern_as_func(exports.data[4]),
                            &empty, &results));
      assert(result.of.i32 == 2);
      wasm_extern_vec_delete(&started_exports);
      wasm_instance_delete(started);
    } else {
      assert(!started && imported_trap);
      wasm_trap_delete(imported_trap);
    }
  }
  wasm_module_delete(imported_module);

  trap = wasm_func_call(run, &empty, &results);
  assert(trap);
  wasm_trap_delete(trap);
  trap = wasm_func_call(run, &args, &empty);
  assert(trap);
  wasm_trap_delete(trap);
  trap = wasm_func_call(fail, &empty, &empty);
  assert(trap);
  wasm_byte_vec_t message;
  wasm_trap_message(trap, &message);
  assert(message.size > 0);
  wasm_byte_vec_delete(&message);
  wasm_trap_delete(trap);

  type = wasm_functype_new_1_1(wasm_valtype_new_i32(), wasm_valtype_new_i32());
  wasm_func_t* plain = wasm_func_new(store, type, increment);
  assert(plain);
  wasm_functype_delete(type);
  assert(!wasm_func_call(plain, &args, &results));
  assert(result.of.i32 == 42);
  wasm_func_delete(plain);

  // Regression for the pinned adapter's reversed wasm_val_copy arguments.
  wasm_val_vec_t values;
  wasm_val_vec_new(&values, 1, &arg);
  wasm_val_vec_t copy;
  wasm_val_vec_copy(&copy, &values);
  assert(values.size == 1 && values.data[0].of.i32 == 41);
  wasm_val_vec_delete(&values);
  assert(copy.size == 1 && copy.data[0].kind == WASM_I32 && copy.data[0].of.i32 == 41);
  wasm_val_vec_delete(&copy);

  assert(wasm_memory_data_size(memory) == 65536);
  wasm_memory_data(memory)[0] = 42;
  assert(wasm_memory_grow(memory, 1));
  assert(!wasm_memory_grow(memory, 1));
  assert(wasm_memory_data_size(memory) == 131072);
  assert(wasm_memory_data(memory)[0] == 42);
  wasm_memory_data(memory)[65536] = 7;
  assert(wasm_memory_data(memory)[65536] == 7);

  wasm_extern_vec_delete(&exports);
  wasm_instance_delete(instance);
  wasm_store_delete(store);
  assert(state.finalized == 1);
  wasm_engine_delete(engine);
}
