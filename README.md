# wee8 builds for Moonrun

This repository builds Moonrun's trimmed **wee8 Wasm runtime** and its Rust
ownership wrapper. Moonrun's rusty_v8 backend uses upstream Deno artifacts;
its wasmtime backend is independent. No custom rusty_v8 archive or Rust V8
bindings are produced here.

The pinned rusty_v8 source submodule supplies V8, dependencies and upstream
platform/toolchain bootstrap code. A small native build driver reuses that
bootstrap to select the wee8 target and its regression probe, without compiling
rusty_v8's Rust crate or C++ adapter.

## Build and artifacts

Install the [upstream prerequisites](https://github.com/denoland/rusty_v8/blob/v0.106.0/README.md#build-v8-from-source)
and run in Bash (Git Bash on Windows):

```sh
gh repo clone moonbitlang/v8-custom-builds
cd v8-custom-builds
CARGO_BUILD_JOBS=8 bash build.sh
```

The script checks out dependencies, applies `patches/*.patch`, builds for the
Rust host target, and packages wee8. Upstream settings such as `CLANG_BASE_PATH`,
`GN`, `NINJA`, and compiler caching remain available. Use a fresh checkout when
changing source revisions or patches; remove the build output when changing
compiler or GN settings. Move existing `dist/<target>/` aside before rebuilding.
Checksums are written last; failed packaging may leave incomplete output.

Each `dist/<target>/` contains:

- `libwee8_release_<target>.a.gz`, or `wee8_release_<target>.lib.gz` on Windows.
- `wasm_c_api_<target>.h`, the matching Wasm C API header.
- API license, complete source notices, and a SHA-256 manifest.

Decompress the archive and include the matching header as `wasm.h`. Define
`LIBWASM_STATIC` when compiling a Windows consumer. The complete archive includes
V8, its C adapter and the configured C++ runtime dependencies. It does not contain
rusty_v8's adapter and must not be supplied as `RUSTY_V8_ARCHIVE`.

[`crates/wee8`](crates/wee8) provides Rust ownership and lifetime handling.
Consumers pin a Git revision; the crate downloads and verifies its configured
release, generates C bindings and links wee8. To test a newly built archive,
provide both `WEE8_ARCHIVE` (decompressed) and `WEE8_HEADER`. Changing the native
recipe does not silently change the crate's pinned release. Host services such
as WASI belong to the embedder.

## Runtime cuts

The normal build disables ICU, Maglev and Sparkplug through GN settings.
`no-js-optimizer.patch` fixes JavaScript optimizing-tier flags to false, while
retaining shared compiler infrastructure needed by Wasm. `no-asmjs.patch`
disables the specialized asm.js-to-Wasm path.

`no-regexp.patch` removes the JavaScript RegExp library at its registration and
call-site boundaries:

- Context initialization does not install `RegExp` or String's `match`,
  `matchAll`, `search`, `replace`, `replaceAll` and `split` methods.
- RegExp builtin/runtime IDs, external references and Torque inputs are removed,
  together with their compiler reductions and RegExp serialization dispatch.
- The parser rejects regexp literals; the existing public API creation boundary
  reports an unsupported-feature syntax error before accessing the absent
  constructor. No fake parser, compiler, matcher or interpreter is supplied.
- Engine translation units are excluded intact. `regexp-support.cc` retains only
  genuine cache clearing and flag formatting. Existing heap layouts, GC metadata
  and stack/cache lifecycle remain.
- Ordinary string predicates (`includes`, `startsWith`, `endsWith`) retain their
  actual `Symbol.match` protocol check; TypedArray registrations are unchanged.

Removing IDs requires fresh generated tables and snapshots. Do not mix generated
files or snapshots from different recipes. This runtime is not a general-purpose
JavaScript implementation. The JS parser, object machinery and other libraries
still exist because this is one library boundary cut, not a complete JS removal.
Temporal, JSON and Date require separate dependency analysis. Platform clocks,
Wasm memory, traps, interrupts and debug machinery are outside this RegExp cut.
A custom debugger and fuel controls are not implemented yet.

## C API scope

`wasm-c-api.patch` adapts the pinned V8 implementation to the unmodified
[Wasm C API header at 9d6b937](https://github.com/WebAssembly/wasm-c-api/blob/9d6b93764ac96cdd9db51081c363e09d2d488b4d/include/wasm.h).
It validates sized argument/import vectors, retains callback arities and
finalizers, fixes value copying, returns start-function exceptions as traps
without unsolicited output, and records Windows link dependencies. The internal
C++ Wasm API remains pinned.

This is not full implementation of every header declaration. Tag APIs and WasmGC
values crossing the host boundary remain unsupported; some modern types reach
unimplemented V8 conversions. The legacy `ANYREF` corresponds to `externref`.
The runtime still uses internal V8 contexts and objects. It does not promise a
stable cross-engine ABI or full Wasm conformance.

## CI and releases

Recipe checks and published-release Rust-consumer checks run on every PR and push
to `main`. They do not compile V8. Native **Build and test** remains manually
triggered, with either one target or the full five-platform matrix: Linux and
macOS x86_64/ARM64, and Windows x86_64.

Native validation uses the freshly built artifacts:

- A GN-built runtime probe checks absent RegExp registrations, rejection paths,
  retained string/TypedArray behavior, and Wasm execution under Liftoff,
  optimizing, lazy-compilation and synchronous tier-up settings.
- The C API consumer checks calls, callbacks, ownership/finalization, vectors,
  traps, silent instantiation failures and memory growth.
- The Rust wrapper suite includes numeric calls of all four scalar kinds,
  serialization into fresh stores, start callbacks, growth and trap recovery.
- Linux x86_64 additionally links the C API consumer against glibc 2.23 using Zig
  and inspects its dynamic symbol-version requirements.

Native archives and the separately retained test executable are cached before
validation. Native cache keys include source/build inputs and the native probe;
consumer-only changes can reuse the archive. The probe is not a release asset.
Tests relink against the selected archive. Compiler caching remains available
when rebuilding native code. Choose `rebuild` to bypass the finished-artifact
cache. Failed matrix jobs can be retried without discarding completed builds.

**Publish tested archives** takes a successful full-matrix run and an existing
`v0.106.0-moonbit.N` tag at that run's commit. It verifies all five platforms and
publishes those exact artifacts without rebuilding. Use a new immutable tag for
each release. Build artifacts are retained for 30 days. The crate's release pin
must be updated deliberately after a new release is available.

These tests do not replace full Wasm conformance, debug/sanitizer validation or
Moonrun integration tests. Moonrun integration belongs in the downstream repo.
`host-tools.patch` maintains bootstrap support for native ARM64 and Windows
runners; `native-target.patch` exposes target selection in that bootstrap.
