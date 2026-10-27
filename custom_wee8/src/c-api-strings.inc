// Native string extensions for V8 12.9.202.13. Included at the end of c-api.cc
// to reuse its private ownership and store adapters. Compilation options are
// module-local; native builtins operate on V8 strings without host emulation.
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

wasm_ref_t* wee8_string_new_utf8(wasm_store_t* store, const char* bytes,
                                 size_t length) {
  if ((length && !bytes) || length > v8::internal::String::kMaxLength)
    return nullptr;
  auto* impl = wasm::impl(reveal_store(store));
  auto* isolate = impl->i_isolate();
  v8::Isolate::Scope scope(impl->isolate());
  v8::internal::HandleScope handles(isolate);
  v8::TryCatch catcher(impl->isolate());
  v8::internal::Handle<v8::internal::String> string;
  if (!isolate->factory()
           ->NewStringFromUtf8(
               {reinterpret_cast<const uint8_t*>(bytes), length},
               unibrow::Utf8Variant::kUtf8NoTrap)
           .ToHandle(&string))
    return nullptr;
  return release_ref(wasm::implement<wasm::Ref>::type::make(impl, string));
}

wasm_ref_t* wee8_string_new_utf16(wasm_store_t* store, const uint16_t* units,
                                  size_t length) {
  if ((length && !units) || length > v8::internal::String::kMaxLength)
    return nullptr;
  auto* impl = wasm::impl(reveal_store(store));
  auto* isolate = impl->i_isolate();
  v8::Isolate::Scope scope(impl->isolate());
  v8::internal::HandleScope handles(isolate);
  v8::TryCatch catcher(impl->isolate());
  v8::internal::Handle<v8::internal::String> string;
  if (!isolate->factory()
           ->NewStringFromTwoByte({units, length})
           .ToHandle(&string))
    return nullptr;
  return release_ref(wasm::implement<wasm::Ref>::type::make(impl, string));
}

bool wee8_ref_is_string(const wasm_ref_t* ref) {
  return wee8_string_length(ref) >= 0;
}

int32_t wee8_string_length(const wasm_ref_t* ref) {
  if (!ref) return -1;
  const auto* impl = wasm::impl(reveal_ref(ref));
  wasm::PtrComprCageAccessScope cage_scope(impl->isolate());
  auto value = impl->v8_object();
  return IsString(*value)
             ? v8::internal::Cast<v8::internal::String>(value)->length()
             : -1;
}

bool wee8_string_copy_utf16(const wasm_ref_t* ref, uint16_t* units,
                            size_t capacity) {
  int32_t length = wee8_string_length(ref);
  if (length < 0 || capacity < static_cast<size_t>(length) ||
      (length && !units))
    return false;
  if (!length) return true;
  const auto* impl = wasm::impl(reveal_ref(ref));
  v8::Isolate::Scope scope(impl->store()->isolate());
  v8::internal::HandleScope handles(impl->isolate());
  auto string = v8::internal::Cast<v8::internal::String>(impl->v8_object());
  v8::internal::String::WriteToFlat(*string, units, 0, length);
  return true;
}
}  // extern "C"
