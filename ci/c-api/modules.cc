// Exercise module acceptance separately from the C value representation.
#undef NDEBUG
#include "wee8.h"

#include <cstring>

extern "C" void wee8_module_probe(wasm_store_t* store, const uint8_t* bytes,
                                   size_t len, const uint8_t* invalid,
                                   size_t invalid_len) {
  wasm_byte_vec_t binary = {len, reinterpret_cast<char*>(const_cast<uint8_t*>(bytes))};
  wasm_byte_vec_t error;
  wasm_module_t* module = wee8_module_new_with_error(store, &binary, nullptr, &error);
  assert(module && error.size == 0);
  wasm_byte_vec_delete(&error);
  assert(wee8_module_import_count(module) == 0);
  wasm_importtype_vec_t import_types;
  assert(wee8_module_imports(module, &import_types) && import_types.size == 0);
  wasm_importtype_vec_delete(&import_types);

  wasm_extern_vec_t imports = {0, nullptr};
  wasm_trap_t* trap = nullptr;
  wasm_instance_t* instance = wasm_instance_new(store, module, &imports, &trap);
  assert(instance && !trap);
  wasm_extern_vec_t exports;
  wasm_instance_exports(instance, &exports);
  assert(exports.size == 5);
  const char* names[] = {"vector", "global", "tag", "memory", "run"};
  for (size_t i = 0; i < exports.size; ++i) {
    wasm_name_t name;
    assert(wee8_module_export_name(module, i, &name));
    assert(name.size == std::strlen(names[i]));
    assert(std::memcmp(name.data, names[i], name.size) == 0);
    wasm_byte_vec_delete(&name);
  }
  wasm_name_t name;
  assert(!wee8_module_export_name(module, exports.size, &name));
  assert(!name.size && !name.data);
  wasm_byte_vec_delete(&name);

  wasm_func_t* vector = wasm_extern_as_func(exports.data[0]);
  assert(!wasm_func_type(vector));
  assert(!wasm_global_type(wasm_extern_as_global(exports.data[1])));
  assert(!wasm_tag_type(wasm_extern_as_tag(exports.data[2])));
  wasm_memory_t* memory = wasm_extern_as_memory(exports.data[3]);
  assert(wee8_memory_is_shared(memory) && !wasm_memory_type(memory));
  wasm_val_vec_t args = {0, nullptr};
  wasm_val_t result = WASM_I32_VAL(0);
  wasm_val_vec_t results = {1, &result};
  trap = wasm_func_call(vector, &args, &results);
  assert(trap);
  wasm_trap_delete(trap);
  assert(!wasm_func_call(wasm_extern_as_func(exports.data[4]), &args, &results));
  assert(result.kind == WASM_I32 && result.of.i32 == 42);
  wasm_exporttype_vec_t types;
  wasm_module_exports(module, &types);
  assert(types.size == 0 && !types.data);
  wasm_exporttype_vec_delete(&types);
  wasm_extern_vec_delete(&exports);
  wasm_instance_delete(instance);
  wasm_module_delete(module);

  binary = {invalid_len, reinterpret_cast<char*>(const_cast<uint8_t*>(invalid))};
  assert(!wee8_module_new_with_error(store, &binary, nullptr, &error));
  assert(error.size > 1 && error.data[error.size - 1] == '\0');
  assert(std::strstr(error.data, "@+"));
  wasm_byte_vec_delete(&error);
}
