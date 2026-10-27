// Module extensions using V8's validated metadata and compile diagnostics.
// Included after the adapter's private ownership conversions.
namespace {
// Copy options at the boundary: compilation/cache lookup never depends on
// mutable process state or the lifetime of the embedder's namespace buffer.
bool StringCompileImports(const wee8_compile_options_t* options,
                          v8::internal::wasm::CompileTimeImports* imports) {
  if (!options) return true;
  if (options->js_string_builtins) {
    imports->Add(v8::internal::wasm::CompileTimeImport::kJsString);
  }
  if (const wasm_name_t* name = options->imported_string_constants) {
    if (name->size && !name->data) return false;
    // V8 12.9 compares namespace chars with unsigned wire bytes. Restrict this
    // option to ASCII until that upstream matching limitation is resolved.
    for (size_t n = 0; n < name->size; ++n) {
      if (static_cast<unsigned char>(name->data[n]) > 0x7f) return false;
    }
    imports->Add(v8::internal::wasm::CompileTimeImport::kStringConstants);
    if (name->size) imports->constants_module().assign(name->data, name->size);
  }
  return true;
}
}  // namespace

extern "C" {
bool wee8_module_validation_is_eager() {
  return !v8::internal::v8_flags.wasm_lazy_validation;
}

bool wee8_module_validate(wasm_store_t* store, const wasm_byte_vec_t* bytes,
                          const wee8_compile_options_t* options) {
  v8::internal::wasm::CompileTimeImports imports;
  if (!StringCompileImports(options, &imports)) return false;
  auto binary = borrow_byte_vec(bytes);
  return wasm::ValidateModule(store, binary.it, std::move(imports));
}

wasm_module_t* wee8_module_new(wasm_store_t* store,
                               const wasm_byte_vec_t* bytes,
                               const wee8_compile_options_t* options) {
  v8::internal::wasm::CompileTimeImports imports;
  if (!StringCompileImports(options, &imports)) return nullptr;
  auto binary = borrow_byte_vec(bytes);
  return release_module(wasm::MakeModule(store, binary.it, std::move(imports)));
}

wasm_module_t* wee8_module_new_with_error(
    wasm_store_t* store, const wasm_byte_vec_t* bytes,
    const wee8_compile_options_t* options, wasm_byte_vec_t* error) {
  *error = {0, nullptr};
  v8::internal::wasm::CompileTimeImports imports;
  if (!StringCompileImports(options, &imports)) {
    *error = release_byte_vec(wasm::Name::make(std::string(
        "invalid compile options")));
    return nullptr;
  }
  auto binary = borrow_byte_vec(bytes);
  auto message = wasm::vec<byte_t>::make();
  auto module = wasm::MakeModule(store, binary.it, std::move(imports), &message);
  *error = release_byte_vec(std::move(message));
  return release_module(std::move(module));
}

size_t wee8_module_import_count(const wasm_module_t* module) {
  const auto* impl = wasm::impl(reveal_module(module));
  wasm::PtrComprCageAccessScope cage_scope(impl->isolate());
  const auto* native = impl->v8_object()->native_module();
  const auto& table = native->module()->import_table;
  return std::count_if(table.begin(), table.end(), [native](const auto& entry) {
    return !wasm::IsBuiltinImport(native, entry);
  });
}

bool wee8_module_imports(const wasm_module_t* module, wasm_importtype_vec_t* out) {
  *out = {0, nullptr};
  auto types = reveal_module(module)->imports();
  if (!types) return false;
  *out = release_importtype_vec(std::move(types));
  return true;
}

bool wee8_module_export_name(const wasm_module_t* module, size_t index,
                            wasm_name_t* out) {
  *out = {0, nullptr};
  const auto* impl = wasm::impl(reveal_module(module));
  wasm::PtrComprCageAccessScope cage_scope(impl->isolate());
  const auto* native = impl->v8_object()->native_module();
  const auto& table = native->module()->export_table;
  if (index >= table.size()) return false;
  *out = release_byte_vec(wasm::GetNameFromWireBytes(
      table[index].name, native->wire_bytes()));
  return true;
}

bool wee8_memory_is_shared(const wasm_memory_t* memory) {
  const auto* impl = wasm::impl(reveal_memory(memory));
  wasm::PtrComprCageAccessScope cage_scope(impl->isolate());
  return impl->v8_object()->array_buffer()->is_shared();
}

wasm_module_t* wee8_module_deserialize(wasm_store_t* store,
                                       const wasm_byte_vec_t* bytes,
                                       const wee8_compile_options_t* options) {
  v8::internal::wasm::CompileTimeImports imports;
  if (!StringCompileImports(options, &imports)) return nullptr;
  auto binary = borrow_byte_vec(bytes);
  return release_module(
      wasm::DeserializeModule(store, binary.it, std::move(imports)));
}

bool wee8_module_import_is_builtin(const wasm_module_t* module, size_t index) {
  const auto* impl = wasm::impl(reveal_module(module));
  wasm::PtrComprCageAccessScope cage_scope(impl->isolate());
  const auto* native = impl->v8_object()->native_module();
  const auto& table = native->module()->import_table;
  return index < table.size() && wasm::IsBuiltinImport(native, table[index]);
}

}  // extern "C"
