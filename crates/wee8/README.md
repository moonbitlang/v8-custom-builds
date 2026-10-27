# wee8

Experimental Rust bindings to MoonBit's patched V8 Wasm C API. `wee8` is the
upstream V8 name for its Wasm C API library. This crate owns native artifact
acquisition, ABI checks, generated declarations, linking, and safe Rust lifetimes.
It has no Moonrun dependency and does not provide WASI or other host services.
Embedders register those operations using `Store::host_func`.

## Use

Depend on this repository with a pinned Git `rev` and `package = "wee8"`.
The repository disables automatic checkout of its native-source submodule for
Cargo consumers; `build.sh` opts into that checkout for native builds.
Cargo runs this crate's build script to download the target's archive and two headers
from `v0.106.0-moonbit.5`. It verifies the release SHA-256 manifest and separately
pinned header hashes, caches verified downloads in Cargo's private `OUT_DIR`, and
links the static library. Building requires Rust 1.95 and libclang for bindgen.

Published targets are Linux and macOS x64/ARM64, plus Windows x64 MSVC. V8 itself
is not rebuilt by this crate. `WEE8_MIRROR` replaces the release-download base URL;
the build script appends the pinned release tag. Set both `WEE8_ARCHIVE` and
`WEE8_HEADER` to use a matching decompressed source-build or offline archive and
headers. `WEE8_HEADER` points to `wee8.h`, with its matching upstream `wasm.h`
in the same directory. Both hashes are checked before binding generation.
Providing only one override is an error. Matching headers are mandatory because
older V8 C APIs used identical symbols with incompatible signatures.

## Supported interface

Guest validation follows the core Wasm proposals enabled by the pinned V8
revision, rather than the instructions currently emitted by MoonBit. This covers
Wasm 2.0 (including sign extension), tail calls, extended constants, relaxed SIMD,
multiple memories, threads/atomics, GC, typed function references, and
standardized exception handling. The feature set is explicit because newer
wasmparser defaults include proposals this engine does not implement.

The Rust validator retains section/type metadata and host-boundary checks.
Archives exposing `wee8_module_validation_is_eager` can take responsibility for
instruction-body validation: successful native compilation must reject every
invalid function, including unexported functions that are never called. Engine
startup disables experimental lazy validation. Older archives, including the
default `.5` release, retain full Rust validation. On native compilation failure,
Rust runs its full validator to preserve detailed invalid-Wasm diagnostics.

The source of this policy is V8's
[always-enabled features](https://github.com/denoland/v8/blob/245ce17ed8483e6bc3de7f668635f46c5bfdd8a0/src/wasm/wasm-features.h)
and this repository's [optional-feature policy](../../custom_wee8/src/wasm-feature-policy.h).
The `.5` release disables legacy exception opcodes, legacy stringref, Memory64
and other staged or experimental proposals. Component-model and newer proposals
are not enabled merely because wasmparser can validate them.

The C API boundary is checked separately. Function metadata supports scalar
numbers, nullable `funcref`, and both nullable and non-null `externref`.
Signature types follow Wasmtime's `ValType::Ref(RefType)` model: `RefType`
combines nullability with a `HeapType`. `ValType::EXTERNREF` describes
`(ref null extern)`; `ValType::Ref(RefType::new(false, HeapType::Extern))`
describes `(ref extern)`. Only heap types supported by the Rust host interface
are constructible; this currently means `HeapType::Extern`. Construction,
introspection and serialized module reuse preserve this distinction. `Value::ExternRef` is the runtime representation for both:
non-null parameters reject `None` before the native call, and a callback returning
`None` for a non-null result produces a host error. Native import matching still
requires the exact guest signature; this API does not erase or relax it.

`Value::matches_ty` checks a concrete value against a signature type, including
nullability. `ValType::matches` checks value-type subtyping, so non-null externref
matches nullable externref, but not vice versa. `==` compares exact types. These
checks are distinct from native function import matching and store identity.
The C ABI value tags are private; both externref nullabilities use the same C
value representation. There is no public `Kind` or `Value::kind()` that could
confuse a representation tag with the full signature type.

Global/table metadata still requires nullable references, and tag payloads permit
only scalar numbers and nullable externref. Other GC reference types and `v128`
at host boundaries are rejected before native introspection, which would
otherwise abort. Shared memories and atomic operations
may stay inside the guest, but importing or exporting a shared memory is rejected:
the host memory API lends an exclusive, non-atomic slice. This does not add a host
thread-spawning or shared-memory interface. Multiple guest memories are allowed;
`Caller::with_memory` still selects the export named `memory`.
The Rust call interface supports scalar numbers and either externref nullability, with
at most one result; callable `funcref` handles are not exposed yet.

`Store::as_context` and `Caller::as_context` provide the same borrowed
`StoreContext` interface for host data and reference creation. Shared host code
can take a context and call `ExternRef::new(context, value)` from either place.
The root borrows the underlying store, so it may outlive the temporary context
but cannot outlive the store or escape a callback's store borrow.
`Store::extern_ref` and `Caller::extern_ref` remain convenience methods.

`ExternRef::data` downcasts their payloads without exposing a native pointer.
Cloning a reference keeps the same object alive; guest globals, tables and GC
objects can retain it after the Rust handle is dropped. Handles borrow their
store, and calls reject references from a different store. Host payloads are
released by native finalizers, including during store teardown.

Internally, the store separates host data `T` from a non-generic `StoreOpaque`
holding native identity and execution state, following Wasmtime's store/context
organization. `Store` owns native teardown; callback environments retain Rust
state until their native finalizers run. `ExternRef` owns a V8 root and borrows
the non-generic store state for lifetime and cross-store checks. Root creation,
copying and deletion use V8's C API; there is no additional Rust root registry.

`extern.convert_any` works in function bodies and constant expressions, including
immediate i31 values and GC structs/arrays. Externalized values are opaque native
roots and can cross the host boundary as nullable `externref`; directly GC-typed
host signatures remain unsupported. Native values have no Rust host payload.
These representation restrictions are independent of proposal support.
Host imports are supplied in declaration order, excluding native builtins enabled
by compile options. Repeated module/name imports share a native binding; different
bindings under one name remain unsupported by this adapter.

A store owns its native objects; native-root handles borrow the store and stay on
its creating thread. `Compiled` may move between threads and create modules in
fresh stores, but its serialized bytes are private. A process-wide V8 engine is
retained for the lifetime of the process because V8 cannot be reinitialized after
engine disposal. Host callback panics become host errors; a captured destructor
that panics during native finalization aborts instead of unwinding through C++.

Host memory access uses the instance's exported `memory` during an exported
function call. Views cannot escape the callback or overlap guest reentry. Memory
access in a Wasm start section is currently unsupported. These restrictions are
independent of any embedder's WASI coverage.

Do not link this V8 archive together with rusty_v8's separate V8 archive in the
same executable. They contain overlapping native symbols and can have different
build configurations. This does not prevent sharing host implementations or
using another engine's native Rust API.

## Validation

`cargo test` exercises native ownership, callbacks, memory borrowing, numeric
execution, internal GC/typed function references, host-reference ownership and
shared store/callback contexts, rejection of unsupported boundary types, checksum
parsing, and public API examples using the published archive.
The Rust-crate workflow runs these checks on Linux, macOS, and Windows without
building V8. Native archive production remains in the separately dispatched
build workflow.

## Exception tags

Tags and exceptions are part of the normal interface. The pinned release supplies
the matching native symbols and header; no Cargo feature or source-build override
is needed. Remove `features = ["exceptions"]` from older dependency declarations.
Standardized `try_table`/`throw_ref` handling is enabled. Legacy EH instructions
remain disabled by the `.4` native release's optional-feature policy.

Integration tests exercise real Wasm through this crate's public API in both the
native build and published-release consumer workflows. Their goals are:

- **Correct propagation:** only matching tags catch an exception, and throws and
  rethrows preserve its payload across the host/guest boundary.
- **Safe ownership:** payload references remain alive while owned, roots are
  released at store teardown, and stale or foreign handles are rejected.
- **Failure separation:** ordinary host errors and panics cannot be swallowed by
  guest `catch_all`; pending exceptions do not contaminate later execution.

These run with ordinary `cargo test`. The packaged C API
consumer has only a small additional header/archive compatibility check for the
new functions; it does not duplicate the exception behavior tests.

`Tag::new(context, params)` creates an identity owned by that store. Following
Wasmtime's store-owned tag model, `Tag` is a copyable handle that can be retained
in host callback captures; every use checks the store identity. Its native root
is released before store teardown, regardless of callback finalization order.
Export lookup reuses an existing tag slot for the same native identity. This
tag table is separate from externrefs, whose roots and payloads remain in V8.

`Module::import_types` describes function/tag/memory imports, and
`instantiate_with_imports` accepts `Extern::Func`, `Extern::Tag` and `Extern::Memory`.
`Instance::tag` retrieves an exported tag. Existing function-only `imports` and
`instantiate` remain available for existing consumers.

```rust,ignore
let tag = wee8::Tag::new(store.as_context(), &[])?;
let throw = store.host_func(&[], &[], move |caller, _| {
    let exception = wee8::Exception::new(caller.as_context(), tag, &[])?;
    Err(caller.throw(&exception))
})?;
let mut instance = module.instantiate_with_imports(&[
    wee8::Extern::Tag(tag), wee8::Extern::Func(&throw),
])?;
```

`Exception` owns a native root and borrows its store. Payloads may contain scalar
numbers and nullable externref. Imported/exported tags with other payload types,
including `funcref`, are rejected at compilation; guest-internal tags may still
carry function references. `Caller::throw` returns an error marker while
the store roots the exception until the callback trampoline takes it. Returning
another outcome discards that pending root. A guest may catch the exception;
ordinary host errors and callback panics still bypass Wasm catches.

An uncaught exception from a call or start section returns
`Error::ThrownException`. `Store::take_exception` transfers its root to the
caller; `Exception::is` tests tag identity and `Exception::args` reads the payload
only with the matching tag. The next guest execution discards an untaken
exception. This follows Wasmtime's separation of an error marker from the
store-owned pending exception, without exposing raw native pointers.

These changes supply exception transport, not an embedder's test driver, and do
not relax the existing start-section memory or guest-reentry restrictions.
Wasm-defined start functions may throw or call throwing host callbacks. Using a
host import directly as the start function remains unsupported by the pinned
V8 C API: that entry goes through a JS call and aborts. This patch does not change
that separate entry-path limitation.

## Native strings

`Store::compile_with_options` explicitly enables V8's `wasm:js-string` builtins
and an optional imported-string-constant namespace. `Store::compile` keeps the
default behavior: every import comes from the host. V8 validates builtin
signatures and classifies them; the wrapper does not maintain a second builtin
catalog. Only those validated imports bypass host-boundary type checks. Their
GC array parameters remain internal to the engine. Host imports and exports
retain the boundary restrictions described above.

`CompileOptions { js_string_builtins: true, imported_string_constants:
Some("_".into()) }` selects MoonBit's native string convention. `None` disables
constants; `Some(String::new())` selects the empty namespace. The pinned V8
supports ASCII namespace names, including embedded NUL. Literal values can
contain Unicode. Options are owned by the module and its `Compiled` result,
including reuse in another store; they never change process-wide defaults.

`StringRef::from_utf8(context, text)` copies a Rust `&str` into V8;
`StringRef::from_utf16(context, units)` preserves every UTF-16 code unit, including
lone surrogates. Use `into_extern_ref()` for guest arguments or callback results,
and `StringRef::try_from(reference)` to check a returned reference. `len()` counts
UTF-16 code units. `to_utf16()` copies them losslessly; `to_utf8()` rejects lone
surrogates, while `to_string_lossy()` explicitly replaces them with U+FFFD.

A string reference owns a native root, borrows its store, and stays on its creating
thread. Guest code uses V8's existing string implementation without a Rust callback
for each builtin. Conversion happens at host-service boundaries; no V8 heap slice
is borrowed across GC. Strings have no Rust `Any` payload (`data()` returns `None`),
and ordinary host payloads do not become strings merely by containing a Rust
`String`. The C API's host-info attachment functions require receiver objects and
must not be used on primitive strings.

The string tests cover all 13 builtins, imported literals, host callbacks,
serialization, mixed host/native imports, guest traps, exception payloads and
store ownership. The packaged C consumer additionally forces collection while
a callback retains its string argument and exercises global/table transport.

### Native stack boundaries and exception frames

`unsafe Store::set_stack_limit(address)` sets a per-store native stack boundary
and returns the old address for restoration. It is an address, not a byte budget;
callers must use bounds of the actual owning thread's stack and reserve enough
space for host/V8 frames. The method rejects zero and calls during guest execution.
It does not allocate a larger OS stack.

`Exception::trace()` returns Wasm function indices and module/function byte offsets
captured when a host creates a catchable exception. A rethrow retains the original
frames. Construction outside a Wasm call has no guest frames; guest-created
exceptions may also have no captured trace. Frames are data snapshots; they do
not keep native instances alive or provide source-map resolution.

### Imported memory and diagnostics

`Memory::new(context, MemoryType { minimum, maximum })` allocates an unshared
wasm32 memory in 64 KiB pages. `ExternType::Memory` describes memory imports;
`Extern::Memory` supplies the same store-owned memory to an instance. Copyable
memory handles may be captured by host callbacks and reject other stores,
including after the original store was destroyed.

`Memory::with_data` lends the current bytes for one closure. All views, including
`Caller::with_memory`, share a store-wide borrow check. Neither growth nor guest
execution may overlap a view. `Memory::grow` returns the previous page count.
An explicitly imported memory can be accessed by a start callback; the existing
instance-export lookup still becomes available only after instantiation.

`Error::Trap` and `Error::Host` contain a `Trap` with its message and owned Wasm `StackFrame`
metadata, captured before the native trap is released. `Caller::capture_stack`
uses the standard C API trap-construction trace without throwing or changing a
pending exception. Exception tracing keeps its existing behavior. These APIs use
the published `.5` archive, including its fix for traces containing native JS-string
builtin frames. No additional native rebuild is required.
