// Positive selection of generated JS-entry bodies for the private Wasm context.
// IDs, argc, Code objects and registration order remain unchanged. Other kinds
// of generated stubs keep their original bodies in this stage; the inventory
// records them explicitly so this is not mistaken for a whole-engine allowlist.
#ifndef WASM_CONTEXT_GENERATED_JS_BUILTIN_SELECTION_H_
#define WASM_CONTEXT_GENERATED_JS_BUILTIN_SELECTION_H_

#include "src/builtins/builtins.h"

#if defined(V8_WASM_NATIVE_CONTEXT)
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

// Wasm well-known-import recognition shares Math identities and fallbacks.
// Retain this bounded group conservatively; no Math global is installed.
#define WASM_CONTEXT_MATH_JS_BUILTINS(V)                                       \
  V(MathAbs)                                                                   \
  V(MathCeil)                                                                  \
  V(MathFloor)                                                                 \
  V(MathRound)                                                                 \
  V(MathTrunc)                                                                 \
  V(MathPow)                                                                   \
  V(MathMax)                                                                   \
  V(MathMin)                                                                   \
  V(MathAcos)                                                                  \
  V(MathAcosh)                                                                 \
  V(MathAsin)                                                                  \
  V(MathAsinh)                                                                 \
  V(MathAtan)                                                                  \
  V(MathAtan2)                                                                 \
  V(MathAtanh)                                                                 \
  V(MathCbrt)                                                                  \
  V(MathClz32)                                                                 \
  V(MathCos)                                                                   \
  V(MathCosh)                                                                  \
  V(MathExp)                                                                   \
  V(MathExpm1)                                                                 \
  V(MathFround)                                                                \
  V(MathF16round)                                                              \
  V(MathImul)                                                                  \
  V(MathLog)                                                                   \
  V(MathLog1p)                                                                 \
  V(MathLog10)                                                                 \
  V(MathLog2)                                                                  \
  V(MathSin)                                                                   \
  V(MathSign)                                                                  \
  V(MathSinh)                                                                  \
  V(MathSqrt)                                                                  \
  V(MathTan)                                                                   \
  V(MathTanh)                                                                  \
  V(MathHypot)                                                                 \
  V(MathRandom)

// Wasm well-known imports recognize these accessors and use their fallbacks.
// Retain the bodies without exposing DataView construction or methods.
#define WASM_CONTEXT_DATAVIEW_JS_BUILTINS(V)                                   \
  V(DataViewPrototypeGetByteLength)                                            \
  V(DataViewPrototypeGetBigInt64)                                              \
  V(DataViewPrototypeGetBigUint64)                                             \
  V(DataViewPrototypeGetFloat32)                                               \
  V(DataViewPrototypeGetFloat64)                                               \
  V(DataViewPrototypeGetInt8)                                                  \
  V(DataViewPrototypeGetInt16)                                                 \
  V(DataViewPrototypeGetInt32)                                                 \
  V(DataViewPrototypeGetUint8)                                                 \
  V(DataViewPrototypeGetUint16)                                                \
  V(DataViewPrototypeGetUint32)                                                \
  V(DataViewPrototypeSetBigInt64)                                              \
  V(DataViewPrototypeSetBigUint64)                                             \
  V(DataViewPrototypeSetFloat32)                                               \
  V(DataViewPrototypeSetFloat64)                                               \
  V(DataViewPrototypeSetInt8)                                                  \
  V(DataViewPrototypeSetInt16)                                                 \
  V(DataViewPrototypeSetInt32)                                                 \
  V(DataViewPrototypeSetUint8)                                                 \
  V(DataViewPrototypeSetUint16)                                                \
  V(DataViewPrototypeSetUint32)

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

constexpr bool WasmContextKeepsJsBuiltin(Builtin builtin) {
  switch (builtin) {
#define RETAIN(Name) case Builtin::k##Name:
    WASM_CONTEXT_ENTRY_JS_BUILTINS(RETAIN)
    WASM_CONTEXT_PRIMITIVE_JS_BUILTINS(RETAIN)
    WASM_CONTEXT_ITERATION_JS_BUILTINS(RETAIN)
    WASM_CONTEXT_MATH_JS_BUILTINS(RETAIN)
    WASM_CONTEXT_DATAVIEW_JS_BUILTINS(RETAIN)
    WASM_CONTEXT_WASM_JS_BUILTINS(RETAIN)
#undef RETAIN
    return true;
  default:
    return false;
  }
}

} // namespace v8::internal
#endif // V8_WASM_NATIVE_CONTEXT
#endif // WASM_CONTEXT_GENERATED_JS_BUILTIN_SELECTION_H_
