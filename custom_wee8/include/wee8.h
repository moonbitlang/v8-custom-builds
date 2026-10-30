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

// Thread-safe control of V8 execution. Acquire on the store's owning thread;
// the owned handle does not retain the store. Requests may race store disposal
// and return false after disposal. They may run concurrently on a live handle;
// delete only after all requests using that handle have returned.
typedef struct wee8_isolate_handle_t wee8_isolate_handle_t;
WASM_API_EXTERN wee8_isolate_handle_t* wee8_store_isolate_handle(wasm_store_t*);
WASM_API_EXTERN void wee8_isolate_handle_delete(wee8_isolate_handle_t*);
// Uses V8's interrupt mechanism, including pure Wasm loops. It cannot preempt a
// blocking host callback; the embedder must unblock its own host operations.
WASM_API_EXTERN bool wee8_isolate_handle_terminate_execution(
  const wee8_isolate_handle_t*);
// Normally wait for the interrupted invocation to unwind before cancellation.
// Cancellation racing execution can allow that invocation to continue.
WASM_API_EXTERN bool wee8_isolate_handle_cancel_terminate_execution(
  const wee8_isolate_handle_t*);
// Owner-thread trap inspection. Termination is distinct from a normal trap or
// catchable Wasm exception, including when a start function is interrupted.
WASM_API_EXTERN bool wee8_trap_is_termination(const wasm_trap_t*);

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
// Query after engine initialization. When true, successful wasm_module_new and
// wee8_module_new have validated every function body, including lazy/uncalled
// functions. V8 flag configuration must not change after engine initialization.
// Older archives without this query require the embedder's validation fallback.
WASM_API_EXTERN bool wee8_module_validation_is_eager(void);
WASM_API_EXTERN bool wee8_module_validate(
  wasm_store_t*, const wasm_byte_vec_t*, const wee8_compile_options_t*);
WASM_API_EXTERN wasm_module_t* wee8_module_new(
  wasm_store_t*, const wasm_byte_vec_t*, const wee8_compile_options_t*);
// Compile using V8's validation and return its diagnostic on failure. The
// required error output is an owned byte vector (empty on success; otherwise
// NUL-terminated), deleted with wasm_byte_vec_delete. Host type limitations do
// not restrict module compilation, instantiation or Wasm-to-Wasm linking.
WASM_API_EXTERN wasm_module_t* wee8_module_new_with_error(
  wasm_store_t*, const wasm_byte_vec_t*, const wee8_compile_options_t*,
  wasm_byte_vec_t* error);
// Names/counts never convert Wasm types into C value kinds. Import counts exclude
// native builtins. Export indices match wasm_instance_exports. Returned names
// are owned byte vectors with no terminator; delete with wasm_name_delete.
WASM_API_EXTERN size_t wee8_module_import_count(const wasm_module_t*);
WASM_API_EXTERN bool wee8_module_export_name(const wasm_module_t*, size_t,
                                            wasm_name_t* out);
// Checked host introspection: false and an empty vector if any import type is
// unrepresentable. Standard wasm_module_imports/exports return an empty vector
// in that case. wasm_func_type/wasm_tag_type/wasm_global_type/wasm_table_type/
// wasm_memory_type return NULL for unsupported representations. C callers must
// successfully inspect global/table types before using their value accessors;
// those accessors do not uniformly report unsupported types. wasm_func_call traps
// on unsupported signatures. Opaque export handles remain usable for linking.
WASM_API_EXTERN bool wee8_module_imports(const wasm_module_t*, wasm_importtype_vec_t* out);
// Shared memory must not be exposed as an exclusive non-atomic host byte slice.
WASM_API_EXTERN bool wee8_memory_is_shared(const wasm_memory_t*);
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
