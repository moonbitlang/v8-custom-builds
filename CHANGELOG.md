# Changelog

Notable changes to the MoonBit wee8 runtime, following [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).
The dated section in the tagged commit supplies the corresponding GitHub Release description.
Unreleased changes are not available through normal downloads until the matching native release is published.

## [Unreleased]

### Changed

- Release publication now requires a dated changelog section and copies it into
  the GitHub Release notes, alongside consumer qualification and provenance.

## [0.106.0-moonbit.6] - 2026-10-27

### Changed

This release fixes per-store native memory retention, delegates compilation and
module metadata to V8, and reduces repeated Rust work when calling exports.
The V8 revision remains unchanged.

- [#31](https://github.com/moonbitlang/v8-custom-builds/pull/31) releases per-store
  task queues and reclaims duplicate canonical-type candidates, while retaining
  genuinely new canonical types.
- [#32](https://github.com/moonbitlang/v8-custom-builds/pull/32) owns the C adapter
  as ordinary source and organizes upstream changes into an ordered patch series
  with an upgrade/validation contract.
- [#34](https://github.com/moonbitlang/v8-custom-builds/pull/34) constructs the
  private Wasm context directly, using V8's existing snapshot format, generator
  and deserializer. Unneeded JavaScript Math/DataView bodies are omitted;
  Wasm numeric and memory operations remain supported.
- [#36](https://github.com/moonbitlang/v8-custom-builds/pull/36) adds cached export
  handles and typed numeric calls. Typed calls check signatures once and avoid
  Rust allocations, but still use the general native C call path.
- [#37](https://github.com/moonbitlang/v8-custom-builds/pull/37) and
  [#38](https://github.com/moonbitlang/v8-custom-builds/pull/38) make V8 the
  authoritative validator and metadata source. Valid SIMD/GC/shared-memory
  modules are accepted; unsupported host representations are rejected when the
  host accesses them. Shared memory cannot become an exclusive Rust byte slice.
  The wrapper requires `.6`; there is no older-release fallback.
- [#39](https://github.com/moonbitlang/v8-custom-builds/pull/39) separates native
  production from wrapper validation and promotes the exact tested archives
  without recompiling V8.

All five targets passed the [native producer](https://github.com/moonbitlang/v8-custom-builds/actions/runs/40444785982)
and [merged-commit consumer qualification](https://github.com/moonbitlang/v8-custom-builds/actions/runs/40464262249).
[Publication](https://github.com/moonbitlang/v8-custom-builds/actions/runs/40465228226)
preserved those archive bytes and their provenance.

External execution termination and a prepared native numeric-call path are not
included. Direct re-exports of native string builtins remain deferred in
[#33](https://github.com/moonbitlang/v8-custom-builds/issues/33).

## [0.106.0-moonbit.5] - 2026-10-22

### Changed

Trimmed wee8 archives fixing two native C API integration problems from `.4` ([#29](https://github.com/moonbitlang/v8-custom-builds/pull/29)).

- Preserve non-null externref function parameters/results through type construction, copying, introspection and native host signatures. The additive `wee8_valtype_new_externref` and `wee8_valtype_is_nullable` APIs let hosts express `(ref extern)` accurately. Null arguments/results at non-null boundaries produce traps.
- Prevent crashes while reading trap origins/traces from native V8 JS-string builtin errors by filtering non-Wasm frames before reading Wasm-specific metadata.

The standard `wasm.h` and runtime `wasm_val_t` layout are unchanged. Non-null global/table/tag construction remains unsupported; this extension covers function signatures. Native V8 string builtins remain in use. No V8 version or trimming-flag changes are included.

Install `wasm_c_api_<target>.h` as `wasm.h` and `wee8_c_api_<target>.h` as `wee8.h` in the same include directory. New extension calls require the matching `.5` library; the `.4` archive does not implement them.

Linux x64/ARM64, macOS x64/ARM64 and Windows x64 passed the [five-platform release validation](https://github.com/moonbitlang/v8-custom-builds/actions/runs/39718319057), including native contracts in four Wasm compilation modes, the C API regressions and all 48 Rust tests including doctests against each produced archive. Linux x64 also passed the wee8 C consumer link, required-symbol ceiling and execution checks for glibc 2.23. Checksums and validation provenance identify commit `344857312d5fcb155ac25f440f31503b25849010`.

The source Rust crate at this release commit still defaults to `.4`; its non-null signature API and Moonrun integration are follow-up work. This native release does not itself establish a completed Moonrun integration or a new 587-test conformance result.

## [0.106.0-moonbit.4] - 2026-10-21

### Changed

Trimmed wee8 archives with native V8 strings and additional Wasm runtime controls.

- Enable V8's `wasm:js-string` builtins and imported string constants per module, with UTF-8/UTF-16 host conversion APIs (#22).
- Restrict optional Wasm proposals, including disabling the legacy `stringref` proposal (#23).
- Expose a per-store native stack boundary setter (#24).
- Capture Wasm frames for native catchable exceptions while preserving tag identity, payloads, and rethrow behavior (#25).
- Support opaque externalized i31 values and GC objects through the C API (#26).
- Ship unchanged upstream `wasm.h` separately from wee8 extensions in `wee8.h`, preserving existing symbols and ABI (#27).

Install `wasm_c_api_<target>.h` as `wasm.h` and `wee8_c_api_<target>.h` as `wee8.h` in the same include directory. Include `wee8.h` when using the extensions.

Linux and macOS x64/ARM64 and Windows x64 passed native checks, packaged C API probes, and Rust candidate tests in [the five-platform build](https://github.com/moonbitlang/v8-custom-builds/actions/runs/39585557690). Linux x64 also passed the glibc 2.23 compatibility check. Archives and headers include checksums and validation provenance for commit `265fe48d2d720b48433a004e2866f37bd2925cce`.

## [0.106.0-moonbit.3] - 2026-10-21

### Changed

This release adds native Wasm exception tags and catchable throws, and switches
wee8 to a dedicated native Wasm context with explicit implementation selections.
It builds on the `.2` runtime cuts.

- [#17](https://github.com/moonbitlang/v8-custom-builds/pull/17) adds tag identity,
  exception payload ownership and Wasm catch/rethrow support. Host payloads support
  numeric values and externref; funcref payloads are rejected at the host interface.
- [#18](https://github.com/moonbitlang/v8-custom-builds/pull/18) separates the Wasm
  context from JavaScript library installation and makes builtin/runtime body
  selections explicit. The snapshot still uses V8's existing machinery.
- [#19](https://github.com/moonbitlang/v8-custom-builds/pull/19) fixes native probe
  linkage against the static C API.

The [five-platform native build](https://github.com/moonbitlang/v8-custom-builds/actions/runs/39548950210)
validated the produced archives. Later releases refine the context and host
representation rules; these notes describe `.3`, not the current interface.

## [0.106.0-moonbit.2] - 2026-10-20

### Changed

Custom wee8 size reductions

This release includes the isolated custom wee8 profile with JavaScript source parsing and asm.js translation disabled, plus the generated-body cut for 149 unsupported JavaScript standard-library entry points.

The library-body cut alone reduces the measured stripped ARM64 macOS test consumer by 280,832 bytes (1.80%) and embedded builtin code by 271,840 bytes (15.61%), relative to the merged asm.js-cut baseline. These measurements did not enable V8 ThinLTO and are not a full moonrun executable measurement.

### Supported boundary

Binary Wasm, the Wasm C API, retained native callbacks, memory operations, GC, iteration and Promise infrastructure remain. Calling one of the explicitly omitted JS library methods intentionally terminates the process. JavaScript source-compilation APIs report that source parsing is unsupported. RegExp and Intl remain disabled.

The exact omitted-method list and dependency rationale are recorded in `ci/library-builtins` at the tagged source revision. Date's retained C++ implementations have not yet been removed in this release.

### Artifacts and validation

Five native targets are included: Linux x86_64/ARM64, macOS x86_64/ARM64, and Windows x86_64. The publishing workflow verifies their checksums and that every artifact comes from the same tested recipe commit.

Candidate validation: https://github.com/moonbitlang/v8-custom-builds/actions/runs/39356580659

The Rust consumer selects the archive through its explicit RELEASE constant. Its update follows publication so that the default download never points to a missing release.

## [0.106.0-moonbit.1] - 2026-10-14

### Changed

Trimmed V8 archives built from the pinned rusty_v8 v0.106.0 source and recipe commit `4f7b5053961c17a59686d14dfe753a98bc90051f`.

- Separate rusty_v8 adapter and experimental wee8 Wasm C API libraries for Linux x64/ARM64, macOS x64/ARM64, and Windows x64.
- ICU, Maglev, Sparkplug, JavaScript optimization and asm.js specialization are disabled. The JavaScript parser/interpreter, RegExp and Wasm compilers remain.
- Each target includes its matching generated rusty_v8 bindings, updated Wasm C API header, SHA-256 manifest, license notices and validation provenance. Decompress the selected `.a.gz` or `.lib.gz` before linking. Use the paired header with wee8: older V8 C API headers have incompatible call signatures.

All five targets passed [source builds and packaged-library consumer tests](https://github.com/moonbitlang/v8-custom-builds/actions/runs/38545495286). The stock rusty_v8 consumer checks disabled ICU APIs and JS tiers, numeric Wasm, and WasmGC across collection with Liftoff and TurboFan. The separate C API consumer checks numeric calls, callbacks, ownership, traps and memory growth. This does not establish WasmGC, reference-value or tag support through the C API. The glibc 2.23 check covers the Linux x64 rusty_v8 consumer only.

Published by the [release workflow](https://github.com/moonbitlang/v8-custom-builds/actions/runs/38562093682), which verified all five artifact bundles and their provenance without recompiling them.
