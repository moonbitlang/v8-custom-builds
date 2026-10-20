// Runtime entry selection for the private Wasm context. Preserve the registry
// layout, argument/result metadata and inline aliases; select implementations.
#ifndef WASM_CONTEXT_RUNTIME_SELECTION_H_
#define WASM_CONTEXT_RUNTIME_SELECTION_H_

#include "src/base/logging.h"
#include "src/runtime/runtime.h"

#if defined(V8_WASM_NATIVE_CONTEXT)
namespace v8::internal {

// Allocation slow paths and C/JS entry machinery remain shared with Wasm.
#define WASM_CONTEXT_CORE_RUNTIMES(V)                                          \
  V(AllocateByteArray)                                                         \
  V(AllocateInYoungGeneration)                                                 \
  V(AllocateInOldGeneration)                                                   \
  V(AllocateHeapNumber)                                                        \
  V(FatalProcessOutOfMemoryInAllocateRaw)                                      \
  V(FatalProcessOutOfMemoryInvalidArrayLength)                                 \
  V(FatalInvalidSize)                                                          \
  V(Abort)                                                                     \
  V(AbortCSADcheck)                                                            \
  V(StackGuard)                                                                \
  V(StackGuardWithGap)                                                         \
  V(HandleNoHeapWritesInterrupts)                                              \
  V(TerminateExecution)                                                        \
  V(UnwindAndFindExceptionHandler)                                             \
  V(PropagateException)                                                        \
  V(SharedValueBarrierSlow)                                                    \
  V(IncrementUseCounter)                                                       \
  V(AccessCheck)                                                               \
  V(GetDerivedMap)                                                             \
  V(NewObject)                                                                 \
  V(CompleteInobjectSlackTrackingForMap)                                       \
  V(TryMigrateInstance)                                                        \
  V(GetProperty)                                                               \
  V(HasProperty)                                                               \
  V(SetNamedProperty)                                                          \
  V(SetKeyedProperty)                                                          \
  V(CreateDataProperty)                                                        \
  V(WeakCollectionSet)                                                         \
  V(WeakCollectionDelete)                                                      \
  V(GrowableSharedArrayBufferByteLength)                                       \
  V(InstantiateAsmJs)

// Retain shared error paths, including clean rejection of disabled APIs.
#define WASM_CONTEXT_ERROR_RUNTIMES(V)                                         \
  V(Throw)                                                                     \
  V(ReThrow)                                                                   \
  V(ReThrowWithMessage)                                                        \
  V(ThrowStackOverflow)                                                        \
  V(ThrowRangeError)                                                           \
  V(ThrowTypeError)                                                            \
  V(ThrowTypeErrorIfStrict)                                                    \
  V(ThrowReferenceError)                                                       \
  V(ThrowInvalidStringLength)                                                  \
  V(ThrowInvalidTypedArrayAlignment)                                           \
  V(ThrowNoAccess)                                                             \
  V(ThrowNotConstructor)                                                       \
  V(ThrowCalledNonCallable)                                                    \
  V(ThrowApplyNonFunction)                                                     \
  V(ThrowConstructedNonConstructable)                                          \
  V(ThrowConstructorReturnedNonObject)                                         \
  V(ThrowConstructorNonCallableError)                                          \
  V(NewError)                                                                  \
  V(NewTypeError)                                                              \
  V(NewReferenceError)

// Wasm conversions and imported-string builtins share primitive slow paths.
// This does not retain JS library constructors, parsing/eval, or replacement.
#define WASM_CONTEXT_PRIMITIVE_RUNTIMES(V)                                     \
  V(ToBigInt)                                                                  \
  V(ToBigIntConvertNumber)                                                     \
  V(BigIntToNumber)                                                            \
  V(NumberToStringSlow)                                                        \
  V(StringParseFloat)                                                          \
  V(StringToNumber)                                                            \
  V(ToNumber)                                                                  \
  V(ToNumeric)                                                                 \
  V(ToString)                                                                  \
  V(ToObject)                                                                  \
  V(ToName)                                                                    \
  V(ToLength)                                                                  \
  V(FlattenString)                                                             \
  V(InternalizeString)                                                         \
  V(StringAdd)                                                                 \
  V(StringCharCodeAt)                                                          \
  V(StringCodePointAt)                                                         \
  V(StringCompare)                                                             \
  V(StringEqual)                                                               \
  V(StringGreaterThan)                                                         \
  V(StringGreaterThanOrEqual)                                                  \
  V(StringLessThan)                                                            \
  V(StringLessThanOrEqual)                                                     \
  V(StringSubstring)                                                           \
  V(StringIsWellFormed)                                                        \
  V(StringToWellFormed)

// Preserve native tracing and debug/interrupt hooks. JS debugger evaluation and
// testing-only optimization controls are not part of the private Wasm context.
#define WASM_CONTEXT_DIAGNOSTIC_RUNTIMES(V)                                    \
  V(CollectGarbage)                                                            \
  V(ClearStepping)                                                             \
  V(DebugBreakAtEntry)                                                         \
  V(DebugOnFunctionCall)                                                       \
  V(HandleDebuggerStatement)                                                   \
  V(IsBreakOnException)                                                        \
  V(ScheduleBreak)                                                             \
  V(WasmTraceEnter)                                                            \
  V(WasmTraceExit)                                                             \
  V(WasmTraceMemory)

constexpr bool WasmContextKeepsRuntime(Runtime::FunctionId id) {
  switch (id) {
#define RETAIN(Name) case Runtime::k##Name:
    WASM_CONTEXT_CORE_RUNTIMES(RETAIN)
    WASM_CONTEXT_ERROR_RUNTIMES(RETAIN)
    WASM_CONTEXT_PRIMITIVE_RUNTIMES(RETAIN)
    WASM_CONTEXT_DIAGNOSTIC_RUNTIMES(RETAIN)
#undef RETAIN
    return true;
#define RETAIN_WASM(Name, ...) case Runtime::k##Name:
    FOR_EACH_INTRINSIC_WASM(RETAIN_WASM, RETAIN_WASM)
#if V8_ENABLE_DRUMBRAKE
    FOR_EACH_INTRINSIC_WASM_DRUMBRAKE_TEST(RETAIN_WASM, RETAIN_WASM)
#endif
#undef RETAIN_WASM
    return true;
  default:
    return false;
  }
}

// Result is Address or ObjectPair, matching the original ABI. Inline aliases
// select the base runtime ID so their fallback address remains identical.
template <Runtime::FunctionId id, typename Result>
V8_NOINLINE Result WasmContextUnsupportedRuntime(int, Address *, Isolate *) {
  FATAL("Unsupported Wasm-context runtime: %s",
        Runtime::FunctionForId(id)->name);
}

template <Runtime::FunctionId id, auto original>
constexpr auto WasmContextRuntimeEntry() {
  if constexpr (WasmContextKeepsRuntime(id)) {
    return original;
  } else {
    using Result = decltype(original(0, nullptr, nullptr));
    return &WasmContextUnsupportedRuntime<id, Result>;
  }
}

} // namespace v8::internal
#endif // V8_WASM_NATIVE_CONTEXT

#if defined(V8_WASM_NATIVE_CONTEXT)
#define WASM_CONTEXT_RUNTIME_ENTRY(Name)                                       \
  FUNCTION_ADDR((WasmContextRuntimeEntry<Runtime::k##Name, &Runtime_##Name>()))
#else
#define WASM_CONTEXT_RUNTIME_ENTRY(Name) FUNCTION_ADDR(Runtime_##Name)
#endif

#endif // WASM_CONTEXT_RUNTIME_SELECTION_H_
