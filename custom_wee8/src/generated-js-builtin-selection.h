// Positive selection of generated JS-entry bodies for the private Wasm context.
// IDs, argc, Code objects and registration order remain unchanged. Other kinds
// of generated stubs keep their original bodies in this stage; the inventory
// records them explicitly so this is not mistaken for a whole-engine allowlist.
#ifndef WASM_CONTEXT_GENERATED_JS_BUILTIN_SELECTION_H_
#define WASM_CONTEXT_GENERATED_JS_BUILTIN_SELECTION_H_

#include "src/builtins/builtins.h"

#if defined(V8_CUSTOM_WEE8)
namespace v8::internal {

// Native API trampolines and shared object coercion/diagnostics.
#define WASM_CONTEXT_ENTRY_JS_BUILTINS(V)                                      \
  V(DebugBreakTrampoline)                                                      \
  V(ReturnReceiver)                                                            \
  V(ObjectConstructor)                                                         \
  V(ObjectPrototypeToString)                                                   \
  V(ObjectPrototypeValueOf)                                                    \
  V(GenericLazyDeoptContinuation)

// Primitive representations and conversions shared with native Wasm calls.
#define WASM_CONTEXT_PRIMITIVE_JS_BUILTINS(V)                                  \
  V(BooleanConstructor)                                                        \
  V(BooleanPrototypeToString)                                                  \
  V(BooleanPrototypeValueOf)                                                   \
  V(NumberConstructor)                                                         \
  V(NumberPrototypeToString)                                                   \
  V(NumberPrototypeValueOf)                                                    \
  V(NumberIsFinite)                                                            \
  V(NumberIsInteger)                                                           \
  V(NumberIsNaN)                                                               \
  V(NumberIsSafeInteger)                                                       \
  V(NumberParseFloat)                                                          \
  V(NumberParseInt)                                                            \
  V(GlobalIsFinite)                                                            \
  V(GlobalIsNaN)                                                               \
  V(StringConstructor)                                                         \
  V(StringPrototypeToString)                                                   \
  V(StringPrototypeValueOf)                                                    \
  V(StringFromCharCode)                                                        \
  V(StringPrototypeCharAt)                                                     \
  V(StringPrototypeCharCodeAt)                                                 \
  V(StringPrototypeCodePointAt)                                                \
  V(StringPrototypeConcat)                                                     \
  V(StringPrototypeIndexOf)                                                    \
  V(StringPrototypeSubstring)                                                  \
  V(StringPrototypeIsWellFormed)                                               \
  V(StringPrototypeToWellFormed)                                               \
  V(SymbolPrototypeToPrimitive)                                                \
  V(SymbolPrototypeToString)                                                   \
  V(SymbolPrototypeValueOf)

// Keep ordinary sequence iteration/coercion used by shared Wasm wrappers.
// This does not install iterator globals or retain convenience combinators.
#define WASM_CONTEXT_ITERATION_JS_BUILTINS(V)                                  \
  V(ArrayPrototypeValues)                                                      \
  V(ArrayIteratorPrototypeNext)                                                \
  V(ArrayPrototypeJoin)                                                        \
  V(ArrayPrototypeToString)                                                    \
  V(StringPrototypeIterator)                                                   \
  V(StringIteratorPrototypeNext)                                               \
  V(TypedArrayPrototypeValues)

// Preserve native wrapper bodies and imported-string builtins independently
// of JS linkage. Keeping WasmPromising does not enable a public Promise API.
#define WASM_CONTEXT_WASM_JS_BUILTINS(V)                                       \
  V(JSToJSWrapperInvalidSig)                                                   \
  V(JSToJSWrapper)                                                             \
  V(JSToWasmWrapper)                                                           \
  V(WasmPromising)                                                             \
  V(WebAssemblyStringCast)                                                     \
  V(WebAssemblyStringTest)                                                     \
  V(WebAssemblyStringFromWtf16Array)                                           \
  V(WebAssemblyStringFromUtf8Array)                                            \
  V(WebAssemblyStringIntoUtf8Array)                                            \
  V(WebAssemblyStringToUtf8Array)                                              \
  V(WebAssemblyStringToWtf16Array)                                             \
  V(WebAssemblyStringFromCharCode)                                             \
  V(WebAssemblyStringFromCodePoint)                                            \
  V(WebAssemblyStringCodePointAt)                                              \
  V(WebAssemblyStringCharCodeAt)                                               \
  V(WebAssemblyStringLength)                                                   \
  V(WebAssemblyStringMeasureUtf8)                                              \
  V(WebAssemblyStringConcat)                                                   \
  V(WebAssemblyStringSubstring)                                                \
  V(WebAssemblyStringEquals)                                                   \
  V(WebAssemblyStringCompare)

// Math/DataView JavaScript entrypoints are outside the C adapter interface.
// Wasm numeric and memory instructions retain their own lowering and helpers.
constexpr bool WasmContextKeepsJsBuiltin(Builtin builtin) {
  switch (builtin) {
#define RETAIN(Name) case Builtin::k##Name:
    WASM_CONTEXT_ENTRY_JS_BUILTINS(RETAIN)
    WASM_CONTEXT_PRIMITIVE_JS_BUILTINS(RETAIN)
    WASM_CONTEXT_ITERATION_JS_BUILTINS(RETAIN)
    WASM_CONTEXT_WASM_JS_BUILTINS(RETAIN)
#undef RETAIN
    return true;
  default:
    return false;
  }
}

} // namespace v8::internal
#endif // V8_CUSTOM_WEE8
#endif // WASM_CONTEXT_GENERATED_JS_BUILTIN_SELECTION_H_
