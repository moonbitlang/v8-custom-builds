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

Download a published release with the GitHub CLI, then give rusty_v8 local paths.
Example for a Unix host:

```sh
release=v0.106.0-moonbit.1
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

## Scope

Rust's ICU APIs remain present but must not be called with this archive. V8's
non-ICU date/time-zone fallback remains. Other engine cuts and upgrades are
separate work.

The future Wasm C API interface will keep source builds and prebuilt downloads,
with debugger access retained. TODO: add WasmGC host-reference support; the pinned
C API's legacy `ANYREF` means `externref`, so expose it as `ExternRef` and reserve
`AnyRef` for actual WasmGC references.

## CI and releases

The workflows separate cheap checks, expensive builds, and publication:

- **Check recipe** runs ShellCheck, actionlint and patch-application checks for
  PRs and recipe changes on main. It does not compile V8.
- **Build and test** runs only when manually dispatched. Select `all` for the
  six-platform matrix, or one target while developing a platform fix. Native
  builds are never triggered by an ordinary push or a release tag. Full and
  targeted runs share a per-platform concurrency group to avoid duplicate builds.
- **Publish tested archives** takes a successful build run ID and an existing
  `v0.106.0-moonbit.N` tag pointing to that run's commit. It requires validated
  artifacts for all six platforms and publishes those exact files without
  rebuilding. Tags are immutable; use a new tag for each release.

Targets are Linux, macOS and Windows, each on x86_64 and ARM64. Builds use
standard GitHub-hosted runners with a five-hour per-platform limit. Only release
native libraries are built. The Rust toolchain and Moon validation revision are
pinned in the build workflow; the MoonBit compiler uses nightly and its version
is recorded with each successful validation.

Completed native archives are cached **before** Moonrun tests. A failed test can
therefore be retried without rebuilding V8. Cache keys include the target,
runner image, toolchain versions, upstream pin and native build recipe. Test-only
changes do not invalidate this cache. Rust compilation caches also include the
native key so a changed library cannot reuse stale downstream link outputs. Use GitHub's
**Re-run failed jobs** to retry a partially failed matrix. Successful validated
artifacts are retained for 30 days; publish before they expire.

Validation checks packaged checksums, uses the matching generated bindings,
checks that `Intl` is absent and WasmGC executes, and runs the default Moonrun
release test suite. Linux x86_64 additionally links through Zig against glibc
2.23 and checks the resulting executable's symbol-version requirements.

`patches/host-tools.patch` adapts upstream's tool bootstrap for native ARM64
runners: Linux's `aarch64` name maps to the `arm64` download, Windows ARM64 uses
the pinned x64 GN/Ninja tools under emulation, and non-Linux ARM64 builds skip
Linux sysroot downloads. It also discovers distribution Clang's compiler-rt
archive and recognizes VS 2026's installation directory and VC145 runtime (as
[current Chromium does](https://chromium.googlesource.com/chromium/src/+/main/build/vs_toolchain.py)).
Linux ARM64 requires the matching compiler-rt development package alongside
Clang. These adaptations do not change V8's runtime features.

`patches/bindgen-constants.patch` spells out the typed-array constant declarations
so bindgen 0.69.4 can read them with newer libclang. Values still come from V8
headers; this does not change the Rust API or the native runtime.
