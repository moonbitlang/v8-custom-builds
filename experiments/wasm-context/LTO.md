# Native ThinLTO and upstream size comparison

The allowlist prototype's context probe is **43.68% smaller** than the same
probe linked against upstream rusty_v8 0.102.0, which moonrun main uses.
Against upstream 0.106.0, matching the custom build's V8 version, it is
**42.33% smaller**. These compare complete build profiles; they do not isolate
the effect of source patches from upstream's different build settings.

Native ThinLTO did not reduce size with the tested settings. For the allowlist
C API probe, LTO O2 increased size by **581,056 bytes (4.28%)** and LTO O3 by
**1,191,888 bytes (8.78%)**. Keep native LTO disabled for this prototype's
current size-oriented configuration. Full LTO, size-oriented compilation and
runtime performance have not been measured.

## Common context probe

All rows use `probe.cc` with `PROBE=3`: initialize V8, create an isolate and
context, then dispose of them. This measures the engine linked into a minimal
consumer. It does not execute an application workload. The published upstream
archive does not expose the Wasm C API symbols used by our other probes.

Measurements are ARM64 macOS executable file sizes after dead stripping and
`strip -x`. MB means 1,000,000 bytes. Every executable in this comparison uses
LLVM 18's Mach-O linker, including the non-LTO rows.

| Build | Bytes | MB |
| --- | ---: | ---: |
| Upstream rusty_v8 0.102.0, moonrun's dependency | 24,043,920 | 24.04 |
| Upstream rusty_v8 0.106.0, custom build's version | 23,483,456 | 23.48 |
| Trimmed custom build before the context allowlist | 13,724,176 | 13.72 |
| Context allowlist, no native LTO | 13,542,480 | 13.54 |
| Context allowlist, native ThinLTO O2 | 14,107,056 | 14.11 |
| Context allowlist, native ThinLTO O3 | 14,717,888 | 14.72 |

The accumulated custom-build changes save 10,501,440 bytes versus moonrun's
upstream dependency, or 9,940,976 bytes versus upstream 0.106.0. Within the
matched custom build, the context allowlist itself saves 181,696 bytes (1.32%)
in this probe. Most of the accumulated reduction predates the context allowlist.

The upstream 0.102.0 archive uses V8 12.8.374.6 and pointer compression; the
0.106.0 archive and custom build use V8 12.9.202.13 without pointer compression.
The upstream archives bundle their C++ standard library; the custom build uses
the system library. Matching the version does not eliminate these configuration
differences. Probe compilation matches each archive's public ABI, and V8's
initialization-time build-configuration checks pass.

For a whole-application reference, moonrun main at
[`efda7a5737998d5af78bdaafbd69a6966e8405d9`](https://github.com/moonbitlang/moonrun/commit/efda7a5737998d5af78bdaafbd69a6966e8405d9)
built with `cargo build --release --locked` is **26,098,000 bytes (26.10 MB)**.
Its manifest uses rusty_v8 0.102.0 and enables Rust release LTO and stripping.
The version command and execution of a minimal Wasm `_start` module pass.
There is no corresponding full moonrun binary using this allowlist prototype
yet, so the probe reduction must not be presented as a measured application
reduction. Rust LTO alone cannot optimize the native code inside the published
machine-code V8 archive.

## Matched native LTO comparison

| Probe | No native LTO | ThinLTO O2 | ThinLTO O3 |
| --- | ---: | ---: | ---: |
| Baseline context | 13,724,176 | 14,288,736 | 14,899,568 |
| Baseline C API | 13,757,600 | 14,322,112 | 14,949,456 |
| Baseline C API with WasmGC | 13,741,088 | 14,322,112 | 14,932,944 |
| Allowlist context | 13,542,480 | 14,107,056 | 14,717,888 |
| Allowlist C API | 13,575,888 | 14,156,944 | 14,767,776 |
| Allowlist C API with WasmGC | 13,575,888 | 14,140,432 | 14,767,776 |

The baseline is producer revision
`47697b35234e2f4a2f4892b25deb9fcb6c7fed1c`, including the separate
Promise-closure candidate. The allowlist implementation is `0894040`.
Neither label means a newly published production release.

`build-thinlto.py` replays the existing custom_wee8 GN compilation commands,
adding `-flto=thin`. It rebuilds all **1,234 C/C++ archive inputs** and verifies
that each output is LLVM bitcode. The remaining input is the generated embedded
builtin assembly object. The baseline and allowlist use their respective
previously regenerated snapshots; the embedded blob is byte-identical.
Shared sources use the same bitcode in both archives. Bootstrap and snapshot
objects are compiled separately for the two variants.

The non-LTO comparison reuses the existing native archives with the same GN
compile settings and relinks their probes with the same LLVM linker used for
ThinLTO. Native compilation is O2 in both cases. The two LTO variants change
the link-time optimizer level (`--lto-O2` or `--lto-O3`), not the source
compilation level. No Torque regeneration or full platform matrix was needed.

The size increase is executable code, not retained debug information. In the
allowlist context probe, the `__text` section grows from **11,750,672 bytes**
without LTO to **12,372,816 bytes** with ThinLTO O2. This does not establish
which individual optimizer decisions caused the growth.

The positive context selection still reduces size under LTO, but it does not
remove independent references from builtin metadata and external-reference
tables. The [existing dependency trace](README.md#why-the-executable-is-still-large)
identifies those roots. Removing them consistently with a builtin allowlist is
separate work; these measurements do not predict its eventual savings.

## Validation

Both custom archives pass at each of the three link configurations:

- Context lifecycle, clean rejection of JS text, and an empty global-object
  own-property list in the allowlist candidate.
- Native C API calls, callbacks and ownership/finalization, arity and import
  errors, traps, memory reads/writes, growth and rejected excessive growth.
- WasmGC structs and arrays surviving 100 collections triggered by a native
  callback, plus host-info round-trip and exactly-once finalization.
- Both C API probes in Liftoff-only, optimizing-only, lazy compilation and
  synchronous tier-up modes.

Used-heap readings are unchanged by LTO: 462,568 bytes for the baseline and
335,528 bytes for the allowlist, repeated in three processes per configuration.
The broader prototype limitations in the [README](README.md#validation)
continue to apply. No full moonrun integration suite or platform matrix has
been run against the allowlist candidate.

Exact byte counts, upstream archive hashes and validation outputs are in
[`lto-results.json`](lto-results.json).

## Reproduction

These scripts target the existing ARM64 macOS custom_wee8 build layout.
Use the compiler recorded in the GN commands and a matching LLVM linker and
archiver. The baseline directory must contain its matching `snapshot.cc` and
`embedded.o`; the candidate directory is the output of `build.py`.
Use an unchanged source/generated-header tree while measuring. The resume cache
fingerprints source files and command arguments, not headers or compiler
binaries; use a fresh work directory after changing either.

```sh
python3 experiments/wasm-context/build-thinlto.py \
  --out "$GN_OUT" --baseline "$BASELINE_SNAPSHOT" \
  --candidate "$EXPERIMENT_OUT" --work "$LTO_OUT" \
  --ar "$LLVM_AR" --jobs 6

python3 experiments/wasm-context/check.py \
  --source "$RUSTY_V8_SOURCE" --out "$GN_OUT" \
  --baseline "$BASELINE_ARCHIVE" --candidate "$EXPERIMENT_OUT/libwee8.a" \
  --work "$NO_LTO_CHECKS" --cxx "$CXX" --linker "$LLVM_LINKER" \
  --probes context c-api c-api-gc

python3 experiments/wasm-context/check.py \
  --source "$RUSTY_V8_SOURCE" --out "$GN_OUT" \
  --baseline "$LTO_OUT/baseline/libwee8.a" \
  --candidate "$LTO_OUT/candidate/libwee8.a" \
  --work "$LTO_O2_CHECKS" --cxx "$CXX" --linker "$LLVM_LINKER" \
  --thin-lto --lto-opt-level 2 --probes context c-api c-api-gc
```

Repeat the last command with a separate work directory and
`--lto-opt-level 3` for the O3 row. For upstream, use the published
`librusty_v8_release_aarch64_apple_darwin.a.gz` archive and matching crate source
package, including its bundled C++ headers:

```sh
python3 experiments/wasm-context/measure-upstream.py \
  --source "$UPSTREAM_0102_SOURCE" --archive "$UPSTREAM_0102_ARCHIVE" \
  --work "$UPSTREAM_0102_CHECKS" --cxx "$CXX" \
  --linker "$LLVM_LINKER" --sdk "$SDK" --pointer-compression
```

For upstream 0.106.0, use that version's source and archive and omit
`--pointer-compression`.
