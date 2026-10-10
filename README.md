# V8 builds for Moonrun

Pinned [rusty_v8](https://github.com/denoland/rusty_v8) plus our ICU patch. The
submodule pins V8 and its dependencies; upstream Cargo/GN/Ninja scripts do the
build. The output includes V8 and rusty_v8's C++ adapter, so Moonrun keeps using
`v8 = "=0.106.0"` with default features.

## Build

Install the [upstream prerequisites](https://github.com/denoland/rusty_v8/blob/v0.106.0/README.md#build-v8-from-source)
and run in Bash (Git Bash on Windows):

```sh
gh repo clone moonbitlang/v8-custom-builds
cd v8-custom-builds
CARGO_BUILD_JOBS=8 bash build.sh
```

In GitHub Actions, use `shell: bash` on Linux, macOS, or Windows.

The script initializes submodules, applies patches, builds for the Rust host, and
packages upstream's outputs. Cargo controls parallelism through `CARGO_BUILD_JOBS`;
upstream toolchain settings such as `CLANG_BASE_PATH`, `GN`, and `NINJA` still work.
The only GN overrides are `v8_enable_i18n_support=false` and `icu_use_data_file=false`.

For changed revisions or patches, start with a fresh checkout. For changed GN
settings or compilers, clean first because upstream can reuse old GN configuration:

```sh
cargo clean --manifest-path rusty_v8/Cargo.toml --target-dir build
```

Move existing `dist/<target>/` aside before rebuilding. Patches are recognized on
repeat builds; a dirty `rusty_v8` submodule is expected. Failed packaging may leave
partial output. Checksums are written last.

## Download and use

Each `dist/<target>/` contains the compressed native archive, generated Rust
bindings, `NOTICES_<target>.txt`, and `SHA256SUMS_<target>`. Names follow upstream's
[archive convention](https://github.com/denoland/rusty_v8/blob/v0.106.0/.github/workflows/ci.yml).
The release tag identifies the source pin, patch, and build settings.

This repository is private. Download a published release with an authenticated
GitHub CLI, then give rusty_v8 local paths. Example for a Unix host:

```sh
release=v0.106.0-no-icu.1
target=$(rustc -vV | sed -n 's/^host: //p')
gh release download "$release" --repo moonbitlang/v8-custom-builds \
  --pattern "*${target}*" --dir v8-artifacts
(cd v8-artifacts && shasum -a 256 -c "SHA256SUMS_${target}")
```

From the downstream project, select the downloaded files, or copy source-build
outputs into the same directory:

```sh
artifact_dir=$(cd v8-artifacts && pwd)
unset V8_FROM_SOURCE GN_ARGS EXTRA_GN_ARGS V8_FORCE_DEBUG
RUSTY_V8_ARCHIVE="$artifact_dir/librusty_v8_release_${target}.a.gz" \
RUSTY_V8_SRC_BINDING_PATH="$artifact_dir/src_binding_release_${target}.rs" \
  cargo build --release -p moonrun
```

On Windows, the archive is `rusty_v8_release_<target>.lib.gz`. Keep the crate,
target, default features, archive, and bindings matched. Building an unmodified
upstream dependency with `V8_FROM_SOURCE=1` does not apply our patch.

## Release manually

Build from a committed recipe on each intended platform. Verify ICU is absent
and run Moonrun's compatibility tests against those archives. Tag the recipe
commit, then upload all files from each `dist/<target>/` to that GitHub release.
Use a new tag for changed artifacts and retain the bundled license notices.
There is no CI/CD; GitHub Actions is disabled.

## Scope

Rust's ICU APIs remain present but must not be called with this archive. V8's
non-ICU date/time-zone fallback remains. Other engine cuts and upgrades are
separate work.

The future Wasm C API interface will keep source builds and prebuilt downloads,
with debugger access retained. TODO: add WasmGC host-reference support; the pinned
C API's legacy `ANYREF` means `externref`, so expose it as `ExternRef` and reserve
`AnyRef` for actual WasmGC references.
