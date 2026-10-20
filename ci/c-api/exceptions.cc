// Test native tag identity and exception propagation through the public header.
#undef NDEBUG
#include <initializer_list>

#include "wasm.h"

struct ExceptionState {
  wasm_store_t* store;
  wasm_tag_t* tag;
  bool host_error = false;
};

static wasm_trap_t* throw_exception(void* env, const wasm_val_vec_t*,
                                    wasm_val_vec_t*) {
  auto* state = static_cast<ExceptionState*>(env);
  if (state->host_error) {
    wasm_message_t message;
    wasm_name_new_from_string_nt(&message, "host failure");
    auto* trap = wasm_trap_new(state->store, &message);
    wasm_byte_vec_delete(&message);
    return trap;
  }
  wasm_val_t value = WASM_I32_VAL(42);
  wasm_val_vec_t values = {1, &value};
  return wee8_exception_new(state->store, state->tag, &values);
}

extern "C" void wee8_exception_probe(const uint8_t* bytes, size_t length) {
  auto* engine = wasm_engine_new();
  auto* store = wasm_store_new(engine);
  auto* signature = wasm_functype_new_1_0(wasm_valtype_new_i32());
  auto* type = wasm_tagtype_new(signature);
  assert(type);
  auto* invalid_type =
      wasm_tagtype_new(wasm_functype_new_0_1(wasm_valtype_new_i32()));
  assert(!invalid_type);
  auto* tag = wasm_tag_new(store, type);
  auto* other = wasm_tag_new(store, type);
  auto* copy = wasm_tag_copy(tag);
  assert(tag && other && copy && wasm_tag_same(tag, copy));
  assert(!wasm_tag_same(tag, other));
  wasm_tag_delete(copy);
  auto* copy_type = wasm_tag_type(tag);
  assert(wasm_functype_params(wasm_tagtype_functype(copy_type))->size == 1);
  assert(wasm_externtype_kind(wasm_tagtype_as_externtype(copy_type)) ==
         WASM_EXTERN_TAG);
  wasm_tagtype_delete(copy_type);

  wasm_byte_vec_t binary = {
      length, reinterpret_cast<char*>(const_cast<uint8_t*>(bytes))};
  auto* module = wasm_module_new(store, &binary);
  assert(module);
  wasm_importtype_vec_t import_types;
  wasm_module_imports(module, &import_types);
  assert(import_types.size == 3);
  assert(wasm_externtype_as_tagtype_const(
      wasm_importtype_type(import_types.data[0])));
  wasm_importtype_vec_delete(&import_types);

  ExceptionState state{store, tag};
  auto* func_type = wasm_functype_new_0_0();
  auto* host = wasm_func_new_with_env(store, func_type, throw_exception, &state,
                                      nullptr);
  wasm_functype_delete(func_type);
  wasm_extern_t* import_data[] = {wasm_tag_as_extern(tag),
                                  wasm_tag_as_extern(other),
                                  wasm_func_as_extern(host)};
  wasm_extern_vec_t imports = {3, import_data};
  wasm_trap_t* trap = nullptr;
  auto* instance = wasm_instance_new(store, module, &imports, &trap);
  assert(instance && !trap);
  wasm_extern_vec_t exports;
  wasm_instance_exports(instance, &exports);
  assert(exports.size == 7);
  assert(wasm_tag_same(tag, wasm_extern_as_tag(exports.data[0])));
  wasm_val_vec_t empty = {0, nullptr};
  wasm_val_t result = WASM_I32_VAL(0);
  wasm_val_vec_t results = {1, &result};
  assert(
      !wasm_func_call(wasm_extern_as_func(exports.data[1]), &empty, &results));
  assert(result.of.i32 == 42);
  for (size_t index : {size_t{2}, size_t{4}, size_t{5}}) {
    trap = wasm_func_call(wasm_extern_as_func(exports.data[index]), &empty,
                          &empty);
    assert(trap && wee8_trap_is_exception(trap));
    assert(wee8_exception_is(trap, tag) && !wee8_exception_is(trap, other));
    wasm_val_vec_t values;
    assert(wee8_exception_get_args(trap, tag, &values));
    assert(values.size == 1 && values.data[0].of.i32 == (index == 4 ? 17 : 42));
    wasm_val_vec_delete(&values);
    assert(!wee8_exception_get_args(trap, other, &values));
    assert(values.size == 0);
    wasm_val_vec_delete(&values);
    wasm_trap_delete(trap);
  }
  assert(!wasm_func_call(wasm_extern_as_func(exports.data[3]), &empty, &empty));
  state.host_error = true;
  trap = wasm_func_call(wasm_extern_as_func(exports.data[3]), &empty, &empty);
  assert(trap && !wee8_trap_is_exception(trap));
  wasm_trap_delete(trap);
  assert(
      !wasm_func_call(wasm_extern_as_func(exports.data[6]), &empty, &results));
  assert(result.of.i32 == 7);

  assert(!wee8_exception_new(store, tag, &empty));
  wasm_val_t wrong = WASM_F64_VAL(1.0);
  wasm_val_vec_t wrong_values = {1, &wrong};
  assert(!wee8_exception_new(store, tag, &wrong_values));
  auto* other_store = wasm_store_new(engine);
  assert(!wee8_exception_new(other_store, tag, &results));
  wasm_store_delete(other_store);

  wasm_extern_vec_delete(&exports);
  wasm_instance_delete(instance);
  wasm_module_delete(module);
  wasm_func_delete(host);
  wasm_tag_delete(tag);
  wasm_tag_delete(other);
  wasm_tagtype_delete(type);
  wasm_store_delete(store);
  wasm_engine_delete(engine);
}
