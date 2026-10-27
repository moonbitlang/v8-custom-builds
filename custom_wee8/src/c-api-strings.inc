// Native string extensions; included after the adapter's private ownership
// conversions. Values remain V8 strings without host emulation.
extern "C" {
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
