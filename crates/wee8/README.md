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
Cargo runs this crate's build script to download the target's archive and header
from `v0.106.0-moonbit.2`. It verifies the release SHA-256 manifest and a separately
pinned header hash, caches verified downloads in Cargo's private `OUT_DIR`, and
links the static library. Building requires Rust 1.95 and libclang for bindgen.

Published targets are Linux and macOS x64/ARM64, plus Windows x64 MSVC. V8 itself
is not rebuilt by this crate. `WEE8_MIRROR` replaces the release-download base URL;
the build script appends the pinned release tag. Set both `WEE8_ARCHIVE` and
`WEE8_HEADER` to use a matching decompressed source-build or offline archive and
header. Providing only one override is an error. The matching header is mandatory
because older V8 C APIs used identical symbols with incompatible signatures.

## Supported interface

The validated guest subset is Wasm 1.0 plus bulk-memory, multi-value, saturating
float-to-integer conversions, SIMD, reference types, typed function references
and GC. Structs, arrays and typed
function references may stay inside the guest. The C API boundary is checked
separately: only scalar numbers and nullable `externref`/`funcref` can appear in
import/export metadata. GC references, non-null references and `v128` at that
boundary are rejected before native introspection, which would otherwise abort.
The Rust call interface supports scalar numbers and nullable `externref`, with
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

`extern.convert_any` remains rejected in function bodies and constant
expressions. This pinned V8 C API represents reference values with `JSReceiver`
handles, while externalizing an `i31ref` can produce a native immediate. The
wrapper cannot safely expose every result of that operation yet. Thus this is
internal GC support, not support for every GC module or GC values at the host
boundary. Exceptions/tags, relaxed SIMD, shared memory and other unlisted
proposals remain excluded. Function imports are supplied in declaration order.

A store owns its native objects; their Rust handles borrow the store and stay on
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
