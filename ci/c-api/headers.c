// Check each public include mode as both C and C++, including repeated includes.
#if WEE8_EXTENSIONS
#include "wee8.h"
#include "wee8.h"
#endif
#include "wasm.h"
#include "wasm.h"
#if WEE8_EXTENSIONS
#include "wee8.h"
#endif

#ifdef own
#error "Public headers must not leak the ownership annotation macro"
#endif

void check_header_declarations(wasm_store_t* store) {
  wasm_module_t* (*compile)(wasm_store_t*, const wasm_byte_vec_t*) =
      wasm_module_new;
  (void)compile;
  (void)store;
#if WEE8_EXTENSIONS
  bool (*eager_validation)(void) = wee8_module_validation_is_eager;
  (void)eager_validation;
  wasm_module_t* (*compile_with_error)(wasm_store_t*, const wasm_byte_vec_t*,
      const wee8_compile_options_t*, wasm_byte_vec_t*) = wee8_module_new_with_error;
  size_t (*import_count)(const wasm_module_t*) = wee8_module_import_count;
  bool (*export_name)(const wasm_module_t*, size_t, wasm_name_t*) = wee8_module_export_name;
  bool (*import_types)(const wasm_module_t*, wasm_importtype_vec_t*) = wee8_module_imports;
  bool (*memory_shared)(const wasm_memory_t*) = wee8_memory_is_shared;
  (void)compile_with_error;
  (void)import_count;
  (void)export_name;
  (void)import_types;
  (void)memory_shared;
  wasm_valtype_t* (*ref_type)(bool) = wee8_valtype_new_externref;
  bool (*nullable)(const wasm_valtype_t*) = wee8_valtype_is_nullable;
  (void)ref_type;
  (void)nullable;
  uintptr_t (*limit)(wasm_store_t*, uintptr_t) = wee8_store_set_stack_limit;
  wasm_tagtype_t* type = wasm_tagtype_new(wasm_functype_new_0_0());
  wasm_tag_t* tag = wasm_tag_new(store, type);
  const wasm_extern_t* external = wasm_tag_as_extern_const(tag);
  const wasm_tag_t* borrowed = wasm_extern_as_tag_const(external);
  wasm_val_vec_t empty = WASM_EMPTY_VEC;
  wasm_trap_t* exception = wee8_exception_new(store, borrowed, &empty);
  wee8_compile_options_t options = {false, NULL};
  (void)limit;
  (void)options;
  wasm_trap_delete(exception);
  wasm_tag_delete(tag);
  wasm_tagtype_delete(type);
#endif
}
