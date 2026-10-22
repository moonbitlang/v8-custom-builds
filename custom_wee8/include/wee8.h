// wee8 extensions to WebAssembly/wasm-c-api.
// The adjacent wasm.h is the unmodified upstream header at
// 9d6b93764ac96cdd9db51081c363e09d2d488b4d. Include only wasm.h for its API.
#ifndef WEE8_H
#define WEE8_H

#include "wasm.h"

#ifdef __cplusplus
extern "C" {
#endif

// Ownership follows wasm.h: constructors and copies return owned roots; casts
// borrow their input. Delete tags with wasm_tag_delete, references with
// wasm_ref_delete, exceptions with wasm_trap_delete, and payload vectors with
// wasm_val_vec_delete. All inputs are borrowed unless explicitly stated.
// Runtime wasm_tag_* names are wee8 extensions despite their retained prefix.

// Function-signature externref nullability, absent from upstream wasm.h's
// value kinds. Copies preserve it; wasm_valtype_kind still returns WASM_EXTERNREF.
// Numeric types are not nullable. Use only for function parameters/results;
// non-null global/table/tag construction is not supported by this extension.
// Runtime wasm_val_t values still use WASM_EXTERNREF, with NULL for null.
// A NULL argument/result at a non-null function boundary produces a trap.
WASM_API_EXTERN wasm_valtype_t* wee8_valtype_new_externref(bool nullable);
WASM_API_EXTERN bool wee8_valtype_is_nullable(const wasm_valtype_t*);

// Set the native stack boundary for this store on its owning thread. Returns
// the previous boundary for restoration; zero is rejected and returns zero.
// The embedder must supply an address within the thread's actual stack and
// leave room for host/V8 frames. This does not allocate/grow the OS stack.
// Keep the store on that thread and do not change the limit while Wasm runs.
WASM_API_EXTERN uintptr_t wee8_store_set_stack_limit(wasm_store_t*, uintptr_t);

// Native string extensions. All returned references are owned, store-bound
// roots, deleted with wasm_ref_delete. Host-info attachment requires a receiver
// object and must not be used on primitive strings; lookup returns NULL.
// Constructors copy their input; NULL input requires zero length. Allocation,
// length-limit and invalid UTF-8 failures return NULL.
WASM_API_EXTERN wasm_ref_t* wee8_string_new_utf8(
  wasm_store_t*, const char*, size_t length);
WASM_API_EXTERN wasm_ref_t* wee8_string_new_utf16(
  wasm_store_t*, const uint16_t*, size_t length);
WASM_API_EXTERN bool wee8_ref_is_string(const wasm_ref_t*);
// Length is measured in UTF-16 code units; -1 denotes a non-string (including NULL).
WASM_API_EXTERN int32_t wee8_string_length(const wasm_ref_t*);
// Copy all code units, preserving lone surrogates. False means wrong type or
// insufficient capacity; no output is written on failure. NULL is allowed only
// for an empty string. UTF-8 construction rejects malformed input.
WASM_API_EXTERN bool wee8_string_copy_utf16(const wasm_ref_t*, uint16_t*, size_t capacity);

// Options are borrowed only for the duration of each call. NULL options retain
// standard wasm_module_* behavior. NULL constants disables string constants;
// a non-NULL empty name selects the empty namespace. Names are not NUL-terminated.
// The pinned V8 supports ASCII namespace names; other bytes are rejected.
typedef struct wee8_compile_options_t {
  bool js_string_builtins;
  const wasm_name_t* imported_string_constants;
} wee8_compile_options_t;
WASM_API_EXTERN bool wee8_module_validate(
  wasm_store_t*, const wasm_byte_vec_t*, const wee8_compile_options_t*);
WASM_API_EXTERN wasm_module_t* wee8_module_new(
  wasm_store_t*, const wasm_byte_vec_t*, const wee8_compile_options_t*);
// Serialized code is trusted engine output. Supply the same options used to
// compile it. Incompatible native-code options are rejected by V8.
WASM_API_EXTERN wasm_module_t* wee8_module_deserialize(
  wasm_store_t*, const wasm_byte_vec_t*, const wee8_compile_options_t*);
// Declaration index in the original Wasm import section (before filtering).
// wasm_module_imports returns only imports the host must supply. This predicate
// lets validators distinguish native builtins without duplicating V8's catalog.
WASM_API_EXTERN bool wee8_module_import_is_builtin(const wasm_module_t*, size_t index);

// Tag instances and exceptions are wee8 extensions, not portable wasm-c-api.
// Handles are store-bound roots, following the other reference ownership rules.
// A tag's signature has no results. Exception payloads support scalar numbers
// and nullable externref; unsupported signatures/values return NULL/false.
typedef struct wasm_tag_t wasm_tag_t;
WASM_API_EXTERN void wasm_tag_delete(wasm_tag_t*);
WASM_API_EXTERN wasm_tag_t* wasm_tag_copy(const wasm_tag_t*);
WASM_API_EXTERN bool wasm_tag_same(const wasm_tag_t*, const wasm_tag_t*);
WASM_API_EXTERN void* wasm_tag_get_host_info(const wasm_tag_t*);
WASM_API_EXTERN void wasm_tag_set_host_info(wasm_tag_t*, void*);
WASM_API_EXTERN void wasm_tag_set_host_info_with_finalizer(
  wasm_tag_t*, void*, void (*)(void*));
WASM_API_EXTERN wasm_ref_t* wasm_tag_as_ref(wasm_tag_t*);
WASM_API_EXTERN wasm_tag_t* wasm_ref_as_tag(wasm_ref_t*);
WASM_API_EXTERN const wasm_ref_t* wasm_tag_as_ref_const(const wasm_tag_t*);
WASM_API_EXTERN const wasm_tag_t* wasm_ref_as_tag_const(const wasm_ref_t*);
WASM_API_EXTERN wasm_extern_t* wasm_tag_as_extern(wasm_tag_t*);
WASM_API_EXTERN wasm_tag_t* wasm_extern_as_tag(wasm_extern_t*);
WASM_API_EXTERN const wasm_extern_t* wasm_tag_as_extern_const(const wasm_tag_t*);
WASM_API_EXTERN const wasm_tag_t* wasm_extern_as_tag_const(const wasm_extern_t*);
WASM_API_EXTERN wasm_tag_t* wasm_tag_new(wasm_store_t*, const wasm_tagtype_t*);
WASM_API_EXTERN wasm_tagtype_t* wasm_tag_type(const wasm_tag_t*);

// A catchable Wasm exception uses the existing trap return channel. Return this
// owned handle from a host callback to throw it; wasm_trap_new still creates an
// uncatchable host failure. Copies, deletion and message inspection use trap APIs.
// Construction validates store identity, argument count and types, returning
// NULL on mismatch. All input handles and values are borrowed.
// Captures the current Wasm stack at construction. wasm_trap_origin/trace expose
// those frames; rethrow preserves them. Outside a Wasm call the trace is empty.
WASM_API_EXTERN wasm_trap_t* wee8_exception_new(
  wasm_store_t*, const wasm_tag_t*, const wasm_val_vec_t*);
WASM_API_EXTERN bool wee8_trap_is_exception(const wasm_trap_t*);
WASM_API_EXTERN bool wee8_exception_is(const wasm_trap_t*, const wasm_tag_t*);
// On success returns an owned value vector; on failure returns an empty vector.
WASM_API_EXTERN bool wee8_exception_get_args(
  const wasm_trap_t*, const wasm_tag_t*, wasm_val_vec_t* out);

#ifdef __cplusplus
}  // extern "C"
#endif

#endif  // WEE8_H
