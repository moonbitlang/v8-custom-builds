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

The producer builds `//:custom_wee8`, an entry point for `//v8:wee8` in a
separate GN toolchain scope. `gn/custom_wee8/BUILD.gn` owns the profile settings
and reuses the upstream platform toolchain templates. Ordinary `//v8:wee8`
remains available in the same build with its default capabilities. Only requested
targets are compiled; building the custom target does not build both runtimes.
The two archives are alternative implementations and must not be linked together.

This scope keeps compile definitions, Torque outputs and snapshots consistent
without copying upstream target definitions. The producer currently supports
native builds; cross compilation needs a corresponding snapshot-toolchain scope.
The profile disables ICU, Maglev and Sparkplug through GN settings.
Within this profile, `no-js-optimizer.patch` fixes JavaScript optimizing-tier flags to false, while
retaining shared compiler infrastructure needed by Wasm. `wee8-without-asmjs.patch`
excludes the specialized asm.js-to-Wasm implementation.

`no-regexp.patch` adds `v8_enable_regexp` (default `true`). The custom profile sets it to `false`, guarding the JavaScript RegExp library at its
registration and call-site boundaries:

- Context initialization does not install `RegExp` or String's `match`,
  `matchAll`, `search`, `replace`, `replaceAll` and `split` methods.
- `V8_ENABLE_REGEXP` guards builtin/runtime IDs, external references, compiler
  reductions and serialization dispatch. GN selects the corresponding Torque
  inputs before builtin tables and snapshots are generated.
- The parser rejects regexp literals; the existing public API creation boundary
  reports an unsupported-feature syntax error before accessing the absent
  constructor. No fake parser, compiler, matcher or interpreter is supplied.
- Native source lists and upstream implementation bodies stay intact. Guards
  skip the unavailable implementation; cache clearing and flag formatting stay
  in their original file. Existing heap layouts, GC metadata and stack/cache
  lifecycle remain. Unreachable shared code is left for final-link stripping
  or LTO; build-time source exclusion is not the size objective.
- Ordinary string predicates (`includes`, `startsWith`, `endsWith`) retain their
  actual `Symbol.match` protocol check; TypedArray registrations are unchanged.

Removing IDs requires fresh generated tables and snapshots. Do not mix generated
files or snapshots from different recipes. This runtime is not a general-purpose
JavaScript implementation. The object machinery and other libraries still exist;
the source-parser boundary below removes JavaScript parsing separately.
Temporal, JSON and Date require separate dependency analysis. Platform clocks,
Wasm memory, traps, interrupts and debug machinery are outside this RegExp cut.
A custom debugger and fuel controls are not implemented yet.

`no-source-parser.patch` adds `v8_enable_javascript_parser` (default `true`).
The custom profile sets it to `false`. GN selects a separate
`v8_javascript_parser_disabled` target and excludes the parser, preparser,
scanner, source character streams, literal buffer, rewriter and name inferrer
from compilation. The original parser
implementations and enabled source list remain intact. The alternate parsing
entry points report an explicit unsupported-source compilation error, including
when called by debugger or error-reporting code. One shared compiler call site
guards direct background parsing; script streaming is read-only disabled.

This capability does not remove the JavaScript object model, native builtins,
Ignition or AST metadata. Wasm continues to consume
binary modules. Subject to the library profile below, V8 public APIs and
execution of retained builtins and Wasm wrappers remain; source-compilation APIs report an unsupported-source error.
Snapshot generation and native context creation must work without
parsing JavaScript. For upgrades, audit direct parser callers and the source list,
then inspect both targets' GN sources and definitions for profile leakage and
run the custom runtime/consumer probes. CI does not compile ordinary wee8.

The patches keep separate concerns: `no-source-parser.patch` adds the optional
parsing capability, `wee8-profile.patch` supplies the profile marker used by the
existing tier cuts, and `native-target.patch` connects the build driver
and probes. The profile settings live in the added GN file rather than global
`GN_ARGS`.

`wee8-without-asmjs.patch` adds `v8_enable_asmjs` (default `true`), disabled
only in `custom_wee8`. GN excludes the four asm.js translation units while
preserving the upstream implementation. Compiler guards remove the compilation
job entry point, and the retained `InstantiateAsmJs` runtime ID throws
`TypeError: asm.js is disabled` instead of falling back to JS compilation.
A small selected source retains the export-name constant used by shared Wasm
instantiation. Shared Wasm origins, opcodes, metadata and builtin IDs remain;
removing those is a separate, broader change. Validation is read-only false
when this capability is disabled.

For upgrades, audit direct `AsmJs` callers and all sources under `src/asmjs`.
Check both GN scopes without compiling ordinary wee8: the custom scope must
exclude `asm-js.cc`, `asm-parser.cc`, `asm-scanner.cc` and `asm-types.cc`, while
the default scope must retain them. Inspect the custom archive for translation
and instantiation implementation symbols. The runtime probe calls the retained
runtime entry directly and checks its explicit exception; source rejection alone
would only prove the earlier JS parser cut. Existing binary Wasm tests then
check that the runtime remains usable after the error.

## Unsupported JS library methods

`wee8-without-library-bodies.patch` makes 149 named JS library entry points
unsupported in custom wee8 by emitting distinct trap bodies. IDs, descriptors,
registrations, object layouts and C++ dispatch tables remain. Invoking an omitted
method terminates the process; preserving interface structure does not preserve
that method's functionality. Ordinary V8 retains the original implementations.

See [the selection and retained dependencies](ci/library-builtins/README.md) and
[the exact method list](ci/library-builtins/omitted-builtins.md). Native candidate
CI checks every omitted entry and retained Wasm/iterator/Promise behavior.

## Parser-cut validation

CI builds and tests the custom artifact only. The runtime probe checks the
changed boundary rather than retesting ordinary V8 language semantics:

| Contract | Check | Regression caught |
| --- | --- | --- |
| JS source is unavailable | Script, module, function, `eval` and `Function` entry points return the explicit parser-disabled exception | A source-compilation path still accepts text, crashes, or reports an unrelated failure |
| Streaming is unavailable | `StartStreaming` returns no task and does not read the supplied stream | Background parsing remains reachable |
| Runtime execution remains available | Create a context from the generated snapshot; compile binary Wasm; instantiate through the retained `WebAssembly.Instance` builtin and call exported functions | Snapshot/runtime mismatch or accidental removal of JS API execution machinery |
| GC survives host calls | WasmGC retains objects across a native callback that requests collection | Broken wrappers or rooting after the build cuts |
| Wasm tiers still work | Run the probe with baseline-only, optimizing-only, lazy compilation and synchronous tier-up | A shared Wasm compiler path was disabled with the JS frontend |

The existing packaged C API and Rust tests are reused for callbacks, ownership,
memory, traps, serialization and start functions. They link the candidate archive;
passing the published rusty_v8 library's tests would not validate this artifact.
The native producer already reuses rusty_v8's pinned toolchain/bootstrap. Its
full Rust adapter and source-based JS test suite are not part of this profile's
test target. Debugger protocol behavior and precompiled JS cache compatibility
are not claimed by these checks.

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

- A GN-built runtime probe checks JavaScript script/module/function rejection,
  absent RegExp/Intl registrations, forced GC, and Wasm execution under Liftoff,
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
