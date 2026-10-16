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
from `v0.106.0-moonbit.1`. It verifies the release SHA-256 manifest and a separately
pinned header hash, caches verified downloads in Cargo's private `OUT_DIR`, and
links the static library. Building requires Rust 1.95 and libclang for bindgen.

Published targets are Linux and macOS x64/ARM64, plus Windows x64 MSVC. V8 itself
is not rebuilt by this crate. `WEE8_MIRROR` replaces the release-download base URL;
the build script appends the pinned release tag. Set both `WEE8_ARCHIVE` and
`WEE8_HEADER` to use a matching decompressed source-build or offline archive and
header. Providing only one override is an error. The matching header is mandatory
because older V8 C APIs used identical symbols with incompatible signatures.

## Supported interface

The validated guest subset is Wasm 1.0 plus bulk-memory, multi-value and SIMD
instructions. SIMD values may appear in internal functions, locals and globals.
Function and global imports/exports containing `v128` are rejected before
native compilation: V8 executes SIMD, but this C API cannot describe vector
values at the embedding boundary. Host calls and exported function calls accept
scalar numeric values and at most one result. Other guest proposals, including
relaxed SIMD and shared memory, remain rejected before native introspection.
Function imports are supplied in module declaration order.

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
execution, checksum parsing, and public API examples using the published archive.
The Rust-crate workflow runs these checks on Linux, macOS, and Windows without
building V8. Native archive production remains in the separately dispatched
build workflow.
