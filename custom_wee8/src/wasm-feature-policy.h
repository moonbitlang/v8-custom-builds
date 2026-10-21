// Optional Wasm features accepted by the custom runtime. Features that the
// pinned V8 has already made unconditional retain their upstream behavior.
#ifndef CUSTOM_WEE8_WASM_FEATURE_POLICY_H_
#define CUSTOM_WEE8_WASM_FEATURE_POLICY_H_

#include "src/wasm/wasm-features.h"

namespace v8::internal::wasm {

inline constexpr WasmEnabledFeatures kCustomWee8OptionalFeatures({
    // Modern try_table/throw_ref exception handling.
    WasmEnabledFeature::exnref,
    // Existing Wasm optimization, not a wire format.
    WasmEnabledFeature::inlining,
});

// JS-string builtins and literals use explicit per-module CompileTimeImports.
// They need neither the old stringref instructions nor the imported_strings
// flag, which gates the JavaScript compilation-options API in this V8 version.

} // namespace v8::internal::wasm

#endif // CUSTOM_WEE8_WASM_FEATURE_POLICY_H_
