// Experimental C++ entry selection for the private Wasm context.
// Keep builtin IDs, table positions and distinct unsupported entry addresses.
// This selects CPP builtins only, not Wasm stubs or all JS-linkage builtins.
#ifndef WASM_CONTEXT_CPP_BUILTIN_SELECTION_H_
#define WASM_CONTEXT_CPP_BUILTIN_SELECTION_H_

#include "src/base/logging.h"
#include "src/builtins/builtins.h"

namespace v8::internal {

// Native function templates and the unchanged API adapter share these entries.
#define WASM_CONTEXT_NATIVE_CPP_BUILTINS(V) \
  V(HandleApiConstruct) \
  V(HandleApiCallAsFunctionDelegate) \
  V(HandleApiCallAsConstructorDelegate)

// Empty functions, invalid invocations and private constructor sentinels.
#define WASM_CONTEXT_CORE_CPP_BUILTINS(V) \
  V(EmptyFunction) \
  V(Illegal) \
  V(IllegalInvocationThrower) \
  V(StrictPoisonPillThrower) \
  V(UnsupportedThrower)

// Wasm traps and compile/link errors retain Error/CallSite infrastructure.
// Preserve diagnostic behavior until it has its own interface and validation.
#define WASM_CONTEXT_DIAGNOSTIC_CPP_BUILTINS(V) \
  V(ErrorConstructor) \
  V(ErrorCaptureStackTrace) \
  V(ErrorPrototypeToString) \
  V(CallSitePrototypeGetColumnNumber) \
  V(CallSitePrototypeGetEnclosingColumnNumber) \
  V(CallSitePrototypeGetEnclosingLineNumber) \
  V(CallSitePrototypeGetEvalOrigin) \
  V(CallSitePrototypeGetFileName) \
  V(CallSitePrototypeGetFunction) \
  V(CallSitePrototypeGetFunctionName) \
  V(CallSitePrototypeGetLineNumber) \
  V(CallSitePrototypeGetMethodName) \
  V(CallSitePrototypeGetPosition) \
  V(CallSitePrototypeGetPromiseIndex) \
  V(CallSitePrototypeGetScriptHash) \
  V(CallSitePrototypeGetScriptNameOrSourceURL) \
  V(CallSitePrototypeGetThis) \
  V(CallSitePrototypeGetTypeName) \
  V(CallSitePrototypeIsAsync) \
  V(CallSitePrototypeIsConstructor) \
  V(CallSitePrototypeIsEval) \
  V(CallSitePrototypeIsNative) \
  V(CallSitePrototypeIsPromiseAll) \
  V(CallSitePrototypeIsToplevel) \
  V(CallSitePrototypeToString)

constexpr bool WasmContextKeepsCppBuiltin(Builtin builtin) {
  switch (builtin) {
#define RETAIN(Name) case Builtin::k##Name:
    WASM_CONTEXT_NATIVE_CPP_BUILTINS(RETAIN)
    WASM_CONTEXT_CORE_CPP_BUILTINS(RETAIN)
    WASM_CONTEXT_DIAGNOSTIC_CPP_BUILTINS(RETAIN)
#undef RETAIN
      return true;
    default:
      return false;
  }
}

// A distinct function per builtin preserves external-reference identity and
// reports exactly which unsupported entry was reached. Never return a fake
// result or share the Illegal builtin's code object.
template <Builtin builtin>
V8_NOINLINE Address WasmContextUnsupportedCppBuiltin(int, Address*, Isolate*) {
  FATAL("Unsupported Wasm-context C++ builtin: %s", Builtins::name(builtin));
}

template <Builtin builtin, Address (*original)(int, Address*, Isolate*)>
constexpr auto WasmContextCppEntry() {
  if constexpr (WasmContextKeepsCppBuiltin(builtin)) {
    return original;
  } else {
    return &WasmContextUnsupportedCppBuiltin<builtin>;
  }
}

}  // namespace v8::internal

#if defined(V8_WASM_CONTEXT_CPP_ALLOWLIST)
#define WASM_CONTEXT_CPP_ENTRY(Name) \
  FUNCTION_ADDR((WasmContextCppEntry<Builtin::k##Name, &Builtin_##Name>()))
#else
#define WASM_CONTEXT_CPP_ENTRY(Name) FUNCTION_ADDR(Builtin_##Name)
#endif

#endif  // WASM_CONTEXT_CPP_BUILTIN_SELECTION_H_
