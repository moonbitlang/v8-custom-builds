// Exercise the packaged string API, native builtins and primitive roots.
#undef NDEBUG
#include <algorithm>
#include <vector>

#include "wee8.h"

struct StringState {
  wasm_store_t* store;
  unsigned finalized = 0;
};

static void collected(void* env) {
  static_cast<StringState*>(env)->finalized++;
}

static wasm_trap_t* echo_with_gc(void* env, const wasm_val_vec_t* args,
                                 wasm_val_vec_t* results) {
  auto* state = static_cast<StringState*>(env);
  assert(args->size == 1 && results->size == 1);
  assert(wee8_ref_is_string(args->data[0].of.ref));
  // Prove a collection occurred while the native primitive argument is rooted.
  auto* foreign = wasm_foreign_new(state->store);
  wasm_foreign_set_host_info_with_finalizer(foreign, state, collected);
  wasm_foreign_delete(foreign);
  std::vector<uint16_t> pressure(65536, 0xd800);
  for (unsigned n = 0; n < 4096 && !state->finalized; ++n) {
    wasm_ref_t* temporary =
        wee8_string_new_utf16(state->store, pressure.data(), pressure.size());
    assert(temporary);
    wasm_ref_delete(temporary);
  }
  assert(state->finalized);
  assert(wee8_string_length(args->data[0].of.ref) == 6);
  wasm_val_copy(&results->data[0], &args->data[0]);
  return nullptr;
}

extern "C" void wee8_string_probe(wasm_engine_t* engine, wasm_store_t* store,
                                  const uint8_t* bytes, size_t length,
                                  const uint8_t* echo_bytes,
                                  size_t echo_length) {
  wasm_ref_t* empty = wee8_string_new_utf16(store, nullptr, 0);
  assert(empty && wee8_string_length(empty) == 0);
  assert(wee8_string_copy_utf16(empty, nullptr, 0));
  wasm_ref_delete(empty);
  assert(!wee8_ref_is_string(nullptr) && wee8_string_length(nullptr) == -1);
  assert(!wee8_string_new_utf16(store, nullptr, 1));
  assert(!wee8_string_new_utf8(store, nullptr, 1));
  assert(!wee8_string_new_utf8(store, "\xc0\x80", 2));
  assert(!wee8_string_new_utf8(store, "\xed\xa0\x80", 3));
  wasm_ref_t* utf8 = wee8_string_new_utf8(store, "A\0\xf0\x9f\x98\x84", 6);
  const uint16_t expected[] = {65, 0, 0xd83d, 0xde04};
  uint16_t copy[4] = {};
  assert(utf8 && wee8_string_length(utf8) == 4);
  assert(!wee8_string_copy_utf16(utf8, copy, 3) && copy[0] == 0);
  assert(wee8_string_copy_utf16(utf8, copy, 4));
  assert(std::equal(std::begin(copy), std::end(copy), std::begin(expected)));
  assert(wasm_ref_get_host_info(utf8) == nullptr);
  wasm_ref_delete(utf8);
  wasm_foreign_t* foreign = wasm_foreign_new(store);
  assert(!wee8_ref_is_string(wasm_foreign_as_ref(foreign)));
  wasm_foreign_delete(foreign);

  wasm_name_t constants = {1, const_cast<char*>("_")};
  wee8_compile_options_t options = {true, &constants};
  wasm_byte_vec_t binary = {
      length, reinterpret_cast<char*>(const_cast<uint8_t*>(bytes))};
  assert(wee8_module_validate(store, &binary, &options));
  wasm_module_t* module = wee8_module_new(store, &binary, &options);
  assert(module);
  for (size_t n = 0; n < 16; ++n)
    assert(wee8_module_import_is_builtin(module, n));
  assert(!wee8_module_import_is_builtin(module, 16));
  wasm_importtype_vec_t import_types;
  wasm_module_imports(module, &import_types);
  assert(import_types.size == 0);
  wasm_importtype_vec_delete(&import_types);
  wasm_extern_vec_t no_imports = {0, nullptr};
  wasm_trap_t* trap = nullptr;
  wasm_instance_t* instance =
      wasm_instance_new(store, module, &no_imports, &trap);
  assert(instance && !trap);
  wasm_extern_vec_t exports;
  wasm_instance_exports(instance, &exports);
  assert(exports.size == 3);
  wasm_val_vec_t none = {0, nullptr};
  assert(!wasm_func_call(wasm_extern_as_func(exports.data[0]), &none, &none));
  wasm_val_t literal = WASM_REF_VAL(nullptr);
  wasm_val_vec_t result = {1, &literal};
  assert(!wasm_func_call(wasm_extern_as_func(exports.data[1]), &none, &result));
  assert(wee8_string_length(literal.of.ref) == 5);
  wasm_val_delete(&literal);

  const uint16_t units[] = {0x61, 0xd800, 0x62, 0xdc00, 0xd83d, 0xde04};
  wasm_ref_t* string = wee8_string_new_utf16(store, units, 6);
  wasm_val_t argument = WASM_REF_VAL(string);
  // String roots also travel through the ordinary global/table APIs.
  wasm_globaltype_t* global_type =
      wasm_globaltype_new(wasm_valtype_new(WASM_EXTERNREF), WASM_VAR);
  wasm_global_t* global = wasm_global_new(store, global_type, &argument);
  wasm_globaltype_delete(global_type);
  wasm_val_t global_value;
  wasm_global_get(global, &global_value);
  assert(wasm_ref_same(string, global_value.of.ref));
  wasm_val_delete(&global_value);
  wasm_global_delete(global);
  wasm_limits_t limits = {1, 2};
  wasm_tabletype_t* table_type =
      wasm_tabletype_new(wasm_valtype_new(WASM_EXTERNREF), &limits);
  wasm_table_t* table = wasm_table_new(store, table_type, string);
  wasm_tabletype_delete(table_type);
  assert(table && wasm_table_grow(table, 1, string));
  wasm_ref_t* entry = wasm_table_get(table, 1);
  assert(entry && wasm_ref_same(entry, string));
  wasm_ref_delete(entry);
  wasm_table_delete(table);

  wasm_val_vec_t arguments = {1, &argument};
  wasm_tagtype_t* tag_type =
      wasm_tagtype_new(wasm_functype_new_1_0(wasm_valtype_new(WASM_EXTERNREF)));
  wasm_tag_t* tag = wasm_tag_new(store, tag_type);
  wasm_tagtype_delete(tag_type);
  wasm_trap_t* exception = wee8_exception_new(store, tag, &arguments);
  wasm_val_vec_t decoded;
  assert(exception && wee8_exception_get_args(exception, tag, &decoded));
  assert(decoded.size == 1 && wasm_ref_same(decoded.data[0].of.ref, string));
  wasm_val_vec_delete(&decoded);
  wasm_trap_delete(exception);
  wasm_tag_delete(tag);

  wasm_val_t returned = WASM_REF_VAL(nullptr);
  result = {1, &returned};
  assert(!wasm_func_call(wasm_extern_as_func(exports.data[2]), &arguments,
                         &result));
  uint16_t roundtrip[6];
  assert(wee8_string_copy_utf16(returned.of.ref, roundtrip, 6));
  assert(std::equal(std::begin(roundtrip), std::end(roundtrip),
                    std::begin(units)));
  wasm_val_delete(&returned);
  wasm_extern_vec_delete(&exports);
  wasm_instance_delete(instance);

  wasm_byte_vec_t serialized;
  wasm_module_serialize(module, &serialized);
  assert(!wee8_module_deserialize(store, &serialized, nullptr));
  wasm_store_t* second = wasm_store_new(engine);
  wasm_module_t* restored =
      wee8_module_deserialize(second, &serialized, &options);
  assert(restored);
  wasm_byte_vec_delete(&serialized);
  instance = wasm_instance_new(second, restored, &no_imports, &trap);
  assert(instance && !trap);
  wasm_instance_exports(instance, &exports);
  assert(!wasm_func_call(wasm_extern_as_func(exports.data[0]), &none, &none));
  wasm_extern_vec_delete(&exports);
  wasm_instance_delete(instance);
  wasm_module_delete(restored);
  wasm_shared_module_t* shared = wasm_module_share(module);
  restored = wasm_module_obtain(second, shared);
  assert(restored);
  wasm_module_imports(restored, &import_types);
  assert(import_types.size == 0);
  wasm_importtype_vec_delete(&import_types);
  wasm_module_delete(restored);
  wasm_shared_module_delete(shared);
  wasm_module_delete(module);
  wasm_store_delete(second);

  wasm_byte_vec_t echo_binary = {
      echo_length, reinterpret_cast<char*>(const_cast<uint8_t*>(echo_bytes))};
  module = wasm_module_new(store, &echo_binary);
  assert(module);
  StringState state{store};
  wasm_functype_t* type = wasm_functype_new_1_1(
      wasm_valtype_new(WASM_EXTERNREF), wasm_valtype_new(WASM_EXTERNREF));
  wasm_func_t* echo =
      wasm_func_new_with_env(store, type, echo_with_gc, &state, nullptr);
  wasm_functype_delete(type);
  wasm_extern_t* imports_data[] = {wasm_func_as_extern(echo)};
  wasm_extern_vec_t imports = {1, imports_data};
  instance = wasm_instance_new(store, module, &imports, &trap);
  assert(instance && !trap);
  wasm_instance_exports(instance, &exports);
  returned = WASM_REF_VAL(nullptr);
  assert(!wasm_func_call(wasm_extern_as_func(exports.data[0]), &arguments,
                         &result));
  assert(wee8_string_copy_utf16(returned.of.ref, roundtrip, 6));
  assert(std::equal(std::begin(roundtrip), std::end(roundtrip),
                    std::begin(units)));
  wasm_val_delete(&returned);
  wasm_val_delete(&argument);
  wasm_extern_vec_delete(&exports);
  wasm_instance_delete(instance);
  wasm_func_delete(echo);
  wasm_module_delete(module);
}
