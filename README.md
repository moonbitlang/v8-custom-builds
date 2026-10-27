# wee8 builds for Moonrun

This repository builds Moonrun's trimmed **wee8 Wasm runtime** and its Rust
ownership wrapper. The consumer is `crates/moonrun` in `moonbitlang/moon`.
Moonrun's rusty_v8 backend uses upstream Deno artifacts; its wasmtime backend is
independent. This producer does not build rusty_v8's adapter or Rust V8 bindings.

## Repository structure

| Directory | Responsibility |
| --- | --- |
| [`custom_wee8`](custom_wee8) | Owned C adapter, headers, GN settings, native context, implementation selections and tests |
| [`patches/series`](patches/series) | Ordered Git patches for upstream insertion points, capabilities, engine fixes and build integration |
| `tools/native` | Reuses the pinned rusty_v8 platform/toolchain bootstrap to build wee8 |
| [`crates/wee8`](crates/wee8) | Rust ownership wrapper and explicit published-release pin |
| `ci` | Packaged C API/Rust consumer checks, platform matrix and release validation |

## Build and artifacts

Install the [upstream prerequisites](https://github.com/denoland/rusty_v8/blob/v0.106.0/README.md#build-v8-from-source)
and run in Bash (Git Bash on Windows):

```sh
gh repo clone moonbitlang/v8-custom-builds
cd v8-custom-builds
CARGO_BUILD_JOBS=8 bash build.sh
```

The script checks out the pinned dependencies, applies `patches/series` through
`tools/prepare-source.py`, copies `custom_wee8` into the source tree, then builds and packages
the Rust host target. Upstream settings such as `CLANG_BASE_PATH`, `GN`, `NINJA`
and compiler caching remain available. Use a fresh checkout when changing source
revisions or patches; remove build output when changing compiler or GN settings.
Move existing `dist/<target>/` aside before rebuilding. Checksums are written
last; failed packaging may leave incomplete output.

Each `dist/<target>/` contains:

- `libwee8_release_<target>.a.gz`, or `wee8_release_<target>.lib.gz` on Windows.
- `wasm_c_api_<target>.h`, the unmodified upstream header, installed as `wasm.h`.
- `wee8_c_api_<target>.h`, the wee8 extension header, installed alongside it as `wee8.h`.
- API license, complete source notices, and a SHA-256 manifest.

Decompress the archive and install both matching headers under those names.
Include `wasm.h` for the upstream API, or `wee8.h` for both APIs. Define
`LIBWASM_STATIC` for Windows consumers. The complete archive includes V8, its C
adapter and the configured C++ runtime dependencies. It must not be supplied as
`RUSTY_V8_ARCHIVE`.

The Rust wrapper downloads and verifies its configured release, generates C
bindings and links wee8. To test a new archive, provide both `WEE8_ARCHIVE`
(decompressed) and `WEE8_HEADER` pointing to `wee8.h`, with its matching `wasm.h`
in the same directory. The authoritative release pin is in
[`crates/wee8/build.rs`](crates/wee8/build.rs). Changing the native recipe does not
silently change that pin. Host services such as WASI belong to the embedder.

## Runtime contract

`//:custom_wee8` builds `//v8:wee8` in an isolated GN toolchain scope with
`v8_custom_wee8 = true`. That one switch selects the native context, implementation
lists and JS optimizing-tier restrictions. The same scope supplies capability
settings to the compiler, Torque, builtin generation and snapshot generation.
Only requested targets compile; ordinary wee8 retains upstream defaults and is
not built alongside the custom runtime. The two archives are alternatives and
must not be linked together. This producer currently supports native builds;
cross compilation needs a matching snapshot-toolchain scope.

The runtime supports native Wasm compilation, execution and host callbacks.
It installs no JS library globals and rejects JavaScript source compilation.
Private object maps, allocation, errors and native wrappers remain where Wasm
needs them. Unselected builtin/runtime implementations have explicit unsupported
traps while retaining their IDs and metadata. This is not a general-purpose JS
runtime or a complete whitelist of every V8 subsystem.

See [`custom_wee8/README.md`](custom_wee8/README.md) for the retained implementations,
patch responsibilities and validation. The [maintenance contract](custom_wee8/MAINTENANCE.md)
describes source ownership, patch regeneration and upstream reconciliation.

## C API scope

The owned [`custom_wee8/src/c-api.cc`](custom_wee8/src/c-api.cc) adapts the pinned
V8 implementation to the unmodified
[Wasm C API header at 9d6b937](https://github.com/WebAssembly/wasm-c-api/blob/9d6b93764ac96cdd9db51081c363e09d2d488b4d/include/wasm.h).
It validates sized argument/import vectors, retains callback arities and
finalizers, fixes value copying, returns start-function exceptions as traps
without unsolicited output, and records native link dependencies.

Our declarations live in [`custom_wee8/include/wee8.h`](custom_wee8/include/wee8.h),
which includes the unchanged upstream `wasm.h`. The owned adapter implements the
extensions without editing that upstream C header. Both headers live together
under `custom_wee8/include` and ship together. Existing function symbols and
ownership contracts are unchanged; runtime `wasm_tag_*` names remain explicitly
marked as extensions for compatibility.

The adapter provides tag types, store-owned runtime tag handles and
catchable exception construction, matching and payload access. Runtime tag and
exception functions are wee8 extensions. The pinned V8's `experimental_wasm_exnref`
flag is enabled before initialization for `try_table` and `throw_ref`. Host-created
exceptions carry native tag identity; ordinary host traps remain uncatchable.
Payloads support scalar numbers and nullable externref. Use the matching header.

The adapter includes `custom_wee8/src/c-api-strings.inc` for explicit per-module
options for V8's JS-string builtins and imported string constants, plus owned
string creation and UTF-16 copying.
Only V8-validated builtin imports are omitted from the host import list. Native
roots retain their owning isolate explicitly, allowing primitive strings through
calls, callbacks, globals, tables and exception payloads. Module sharing retains
its compile options. These additions use the existing string implementation;
they do not change the context allowlists, builtin blob or snapshot. Host-info
attachment remains limited to receiver objects, not primitive strings. These are
V8 strings carried as `externref`, not the legacy Wasm `stringref` type.

The custom runtime allows only modern exception handling and Wasm inlining among
V8's optional feature flags. Legacy `stringref` and `try`/`catch`/`delegate`/`rethrow`
exception opcodes are disabled. Modern `try_table`/`throw_ref` remain supported.
JS-string builtins and literals are selected per module; enabling them does not
enable legacy stringref. See the [feature policy](custom_wee8/README.md#wasm-feature-policy)
for the distinction between obsolete proposals and features not enabled in this
pinned runtime.

It supports `extern.convert_any` transport, including
immediate i31 values and guest GC objects. These remain opaque store-owned roots
through calls, callbacks, globals, tables and exception payloads. The existing
Object-root representation needs no extra boxing; host-info attachment still
requires receiver objects.

The adapter captures Wasm call sites when a host-created
catchable exception is constructed. Existing `wasm_trap_origin` and
`wasm_trap_trace` expose those frames; rethrow preserves the original trace.
Construction outside guest execution yields an empty trace.

Function introspection preserves externref nullability in function
parameters and results. The additive `wee8_valtype_new_externref` constructor and
`wee8_valtype_is_nullable` query complement the standard value kind; copies and
function introspection retain the distinction. A null argument or callback result
at a non-null function boundary traps. This does not extend global, table or tag
construction to non-null references, or change `wasm.h` or `wasm_val_t`.

Trap extraction filters non-Wasm frames captured by native JS-string
builtin failures before reading Wasm instance metadata. Both trap origin and
trace retain the remaining Wasm frames in order.

The adapter exposes `wee8_store_set_stack_limit`, setting a per-store
native stack boundary and returning its previous address for restoration. It does
not resize the OS thread stack. The caller must supply a valid address on the
store's owning thread, reserve space for host/V8 frames, and avoid changing the
boundary during Wasm execution. Zero is rejected without changing the limit.

Not every header declaration is implemented. Direct GC-typed host signatures
and SIMD vectors crossing the host interface remain unsupported; some modern types reach
unimplemented V8 conversions. Legacy `ANYREF` means `externref`. The runtime does
not promise a stable cross-engine ABI or full Wasm conformance.

## CI and releases

Recipe and published-release Rust-consumer checks run on PRs and pushes to
`main`; they do not compile V8. If the pinned release is not yet published, the
published consumer job reports that dependency and skips. Run **Rust crate**
with `native-run` set to a successful **Build and test** run to validate all five
produced archives without rebuilding V8. The selected run and current wrapper
must have matching native inputs; artifact provenance and checksums are verified.

Native **Build and test** is manually triggered for
one target or all five supported platforms: Linux and macOS x86_64/ARM64, and
Windows x86_64. It validates the freshly built archive with:

- The [`custom_wee8` checks](custom_wee8/README.md#validation), including snapshot
  initialization, implementation tables and native Wasm execution in four modes.
- The packaged C API consumer: calls, callbacks, ownership/finalization, vectors,
  traps, silent instantiation failures, memory growth and an exception round trip.
- The Rust wrapper suite: scalar calls, serialization into fresh stores, start
  callbacks, growth, trap recovery, tag matching, catch/rethrow,
  payload ownership, store safety and host errors bypassing guest catches.
- Linux x86_64 linking against glibc 2.23 using Zig, with a dynamic-symbol check.

Native archives and test executables are cached before validation. Native cache
keys cover compiled inputs; runner scripts, fixtures and inventories can be
rechecked against cached probes. Tests relink packaged consumers against the
selected archive. Test executables are not release assets. Use `rebuild` to bypass
the finished-artifact cache while retaining compiler caching; failed matrix jobs
can otherwise reuse completed builds.

**Publish tested archives** takes a successful full-matrix run and an existing
`v0.106.0-moonbit.N` tag at that run's commit. It verifies all five platforms and
publishes those exact artifacts without rebuilding. Use a new immutable tag for
each release. Build artifacts are retained for 30 days. Update the wrapper's
release pin deliberately with the native API change; normal downloads require
that release to be published. Before publication, use the produced artifacts.

These checks do not replace full Wasm conformance, debug/sanitizer validation or
integration with the wee8 backend in `moonbitlang/moon`.
