# V8 builds for Moonrun

Pinned source revisions and patches for Moonrun's V8 backend.

This repository starts with an ICU-free build compatible with Moonrun's existing
rusty_v8 dependency. The longer-term direction is a Rust interface over V8's
existing Wasm C API, with project-specific extensions where Moonrun needs them.
The Rust binding and C API extensions are not implemented here yet.

## Initial scope

- Fetch the source revisions recorded in `versions.json` into an ignored build directory.
- Apply the patches in manifest order to the rusty_v8 checkout.
- Disable ICU using the recorded GN arguments.
- Build through rusty_v8's Cargo source-build path, which invokes GN and Ninja.
- Keep Rust's ICU APIs unchanged. Moonrun does not call them; the ICU-free native
  archive does not provide their implementations.

The repository contains no CI/CD workflows or automatic publishing. Other engine
cuts and a V8 version upgrade remain separate work.

## ICU patch

`patches/no-icu.patch` guards the ICU locale header and its two C++ locale helpers
with `V8_INTL_SUPPORT`. V8's own build option removes its ICU dependency;
`icu_use_data_file=false` also avoids the ICU data-file dependency during snapshot
creation. V8's non-ICU date/time-zone fallback remains in place.

This first native artifact is a `librusty_v8` archive containing the matching C++
bindings and V8. It is not a plain `libv8` or `wee8` archive. Moonrun continues to
use the upstream Rust crate and selects the artifact with `RUSTY_V8_ARCHIVE`.

## Manual build

Follow the pinned [rusty_v8 source-build prerequisites](https://github.com/denoland/rusty_v8/blob/v0.106.0/README.md#build-v8-from-source).

1. Clone the rusty_v8 repository under `build/rusty_v8`, check out the exact
   manifest commit, and initialize its submodules recursively. Verify the V8
   submodule commit against the manifest.
2. From that fresh checkout, apply `../../patches/no-icu.patch` with
   `git apply --check` followed by `git apply`.
3. Set `GN_ARGS` to the space-separated `gn_args` in `versions.json`, then run
   `V8_FROM_SOURCE=1 cargo build --release --locked` in the checkout.
4. Preserve the resulting native archive together with its SHA-256, source pins,
   effective GN arguments, target triple, compiler information, and required
   upstream license notices. Use a unique artifact name for each configuration.
5. Build and test Moonrun with `RUSTY_V8_ARCHIVE` selecting that archive. Keep its
   C++ runtime link settings consistent with the archive configuration.

Before adopting an artifact, inspect it for remaining ICU implementations and
run Moonrun's compatibility suite. Compare its size with an ICU-enabled build
using the same source revisions, compiler, and other build flags.

## Future Wasm C API work

Reuse V8's Wasm C API and add narrowly scoped extensions for the required engine
services. Keep MoonBit-specific host behavior in Moonrun. Debugger access is a
requirement of the eventual interface.

TODO: add proper WasmGC host-reference support. In the pinned C API, legacy
`ANYREF` denotes `externref`; expose that as `ExternRef` in Rust. Reserve `AnyRef`
for actual WasmGC references. Guest-internal WasmGC execution and host access to
WasmGC values are separate capabilities.

## Upstream sources

- [rusty_v8](https://github.com/denoland/rusty_v8) provides Rust bindings, a C++ adapter,
  and Cargo integration for the native build.
- [V8](https://v8.dev/) provides the engine and its Wasm C API implementation.

The source revisions are fetched for builds rather than vendored into this repository.
Upstream source and binary license obligations remain applicable to derived artifacts.
