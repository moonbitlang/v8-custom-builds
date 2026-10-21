// Externalized values must remain valid roots through the packaged C ABI.
#undef NDEBUG
#include <cstdint>
#include <vector>

#include "wee8.h"

struct ReferenceState {
  wasm_store_t *store;
  wasm_tag_t *tag;
  unsigned collected = 0;
};

static wasm_trap_t *echo(void *data, const wasm_val_vec_t *args,
                         wasm_val_vec_t *results) {
  auto *state = static_cast<ReferenceState *>(data);
  assert(args->size == 1 && args->data[0].kind == WASM_EXTERNREF);
  // i31 immediates, including zero, and Wasm GC objects are ordinary owned
  // roots, not Rust host-info pointers or nullable sentinel addresses.
  assert(args->data[0].of.ref);
  assert(!wasm_ref_get_host_info(args->data[0].of.ref));
  assert(!wee8_ref_is_string(args->data[0].of.ref));
  wasm_trap_t *exception = wee8_exception_new(state->store, state->tag, args);
  assert(exception);
  // Prove a GC occurred while the callback and exception both retain the value.
  unsigned before = state->collected;
  auto *foreign = wasm_foreign_new(state->store);
  wasm_foreign_set_host_info_with_finalizer(foreign, state, [](void *env) {
    static_cast<ReferenceState *>(env)->collected++;
  });
  wasm_foreign_delete(foreign);
  std::vector<uint16_t> pressure(65536, 0xd800);
  for (unsigned n = 0; n < 4096 && state->collected == before; ++n) {
    auto *string =
        wee8_string_new_utf16(state->store, pressure.data(), pressure.size());
    assert(string);
    wasm_ref_delete(string);
  }
  assert(state->collected > before);
  wasm_val_vec_t payload;
  assert(wee8_exception_get_args(exception, state->tag, &payload));
  assert(payload.size == 1);
  assert(wasm_ref_same(args->data[0].of.ref, payload.data[0].of.ref));
  wasm_val_copy(&results->data[0], &payload.data[0]);
  wasm_val_vec_delete(&payload);
  wasm_trap_delete(exception);
  return nullptr;
}

extern "C" void wee8_externref_probe(wasm_store_t *store, const uint8_t *bytes,
                                     size_t len) {
  wasm_byte_vec_t binary = {
      len, reinterpret_cast<char *>(const_cast<uint8_t *>(bytes))};
  assert(wasm_module_validate(store, &binary));
  auto *module = wasm_module_new(store, &binary);
  assert(module);
  wasm_byte_vec_t serialized;
  wasm_module_serialize(module, &serialized);
  wasm_module_delete(module);
  module = wasm_module_deserialize(store, &serialized);
  wasm_byte_vec_delete(&serialized);
  assert(module);
  auto *tag_type =
      wasm_tagtype_new(wasm_functype_new_1_0(wasm_valtype_new(WASM_EXTERNREF)));
  ReferenceState state{store, wasm_tag_new(store, tag_type)};
  wasm_tagtype_delete(tag_type);
  assert(state.tag);
  auto *unary = wasm_functype_new_1_1(wasm_valtype_new(WASM_EXTERNREF),
                                      wasm_valtype_new(WASM_EXTERNREF));
  auto *echo_host = wasm_func_new_with_env(store, unary, echo, &state, nullptr);
  wasm_functype_delete(unary);
  wasm_extern_t *entries[] = {wasm_func_as_extern(echo_host)};
  wasm_extern_vec_t imports = {1, entries};
  wasm_trap_t *trap = nullptr;
  auto *instance = wasm_instance_new(store, module, &imports, &trap);
  assert(instance && !trap);
  wasm_extern_vec_t exports;
  wasm_instance_exports(instance, &exports);
  assert(exports.size == 10);
  auto *global = wasm_extern_as_global(exports.data[0]);
  auto *table = wasm_extern_as_table(exports.data[1]);
  wasm_val_t initial;
  wasm_global_get(global, &initial);
  assert(initial.of.ref);  // Externalized i31(0), not null.
  wasm_val_delete(&initial);
  auto *table_initial = wasm_table_get(table, 0);
  assert(table_initial);
  wasm_ref_delete(table_initial);
  wasm_val_vec_t none = {0, nullptr};
  for (size_t constructor : {2, 4, 6}) {
    for (int32_t number : {0, -1, 1073741823, -1073741824}) {
      wasm_val_t n = WASM_I32_VAL(number);
      wasm_val_vec_t input = {1, &n};
      wasm_val_t reference = WASM_REF_VAL(nullptr);
      wasm_val_vec_t output = {1, &reference};
      assert(!wasm_func_call(wasm_extern_as_func(exports.data[constructor]),
                             &input, &output));
      assert(reference.of.ref);
      wasm_global_set(global, &reference);
      assert(wasm_table_set(table, 0, reference.of.ref));
      assert(wasm_table_grow(table, 1, reference.of.ref));
      wasm_val_t echoed = WASM_REF_VAL(nullptr);
      wasm_val_vec_t echo_result = {1, &echoed};
      assert(!wasm_func_call(wasm_extern_as_func(exports.data[9]), &output,
                             &echo_result));
      assert(wasm_ref_same(reference.of.ref, echoed.of.ref));
      wasm_val_delete(&reference);
      wasm_val_t read = WASM_I32_VAL(0);
      wasm_val_vec_t decoded = {1, &read};
      assert(!wasm_func_call(wasm_extern_as_func(exports.data[constructor + 1]),
                             &echo_result, &decoded));
      assert(read.of.i32 == number);
      auto *table_value = wasm_table_get(table, 0);
      assert(wasm_ref_same(table_value, echoed.of.ref));
      wasm_ref_delete(table_value);
      wasm_val_t global_value;
      wasm_global_get(global, &global_value);
      assert(wasm_ref_same(global_value.of.ref, echoed.of.ref));
      wasm_val_delete(&global_value);
      wasm_val_delete(&echoed);
    }
  }
  wasm_val_t null_value = WASM_REF_VAL(nullptr);
  wasm_val_vec_t null_result = {1, &null_value};
  assert(!wasm_func_call(wasm_extern_as_func(exports.data[8]), &none,
                         &null_result));
  assert(!null_value.of.ref);

  wasm_extern_vec_delete(&exports);
  wasm_instance_delete(instance);
  wasm_func_delete(echo_host);
  wasm_tag_delete(state.tag);
  wasm_module_delete(module);
}
