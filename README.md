# V8 builds for Moonrun

A thin patch layer over [rusty_v8](https://github.com/denoland/rusty_v8). Git pins
rusty_v8 as a submodule; rusty_v8 pins V8 and its other dependencies. Our patches
and GN settings remove ICU while keeping Moonrun's existing Rust interface.

The output is `librusty_v8.a` (`rusty_v8.lib` on Windows): V8 plus rusty_v8's C++
adapter. Downstream projects use the matching upstream Rust crate and select our
archive through `RUSTY_V8_ARCHIVE`. They can build it from source or download it.

## Build

Install Python 3.11+, Git, Rust/Cargo, and the
[upstream source-build prerequisites](https://github.com/denoland/rusty_v8/blob/v0.106.0/README.md#build-v8-from-source),
then run:

```sh
gh repo clone moonbitlang/v8-custom-builds
cd v8-custom-builds
python3 build.py --jobs 8
```

`build.py` initializes the pinned submodules, applies `patches/*.patch` in filename
order, and invokes upstream's `V8_FROM_SOURCE=1 cargo build --release --locked`.
Upstream owns GN, Ninja, compiler setup, and binding generation. We retain its
default Cargo features and use these GN settings:

```gn
v8_enable_i18n_support=false
icu_use_data_file=false
```

The script builds for the Rust host target. Upstream toolchain settings such as
`CLANG_BASE_PATH`, `GN`, `NINJA`, and `SCCACHE` remain available. It uses one
`build/` directory and recognizes patches already applied to `rusty_v8/`.
The patched submodule appearing dirty in `git status` is expected.

For a changed upstream revision or patch, start from a fresh checkout. When GN
settings or the compiler change, clean the build first: upstream can otherwise
reuse its old GN configuration.

```sh
cargo clean --manifest-path rusty_v8/Cargo.toml --target-dir build
```

Move an existing `dist/<target>/` aside before rebuilding; the script does not
overwrite it. Failed packaging may leave an incomplete directory. A successful
run writes `SHA256SUMS_<target>` last.

## Artifacts and downstream use

`dist/<target>/` contains four files:

- `librusty_v8_release_<target>.a.gz` (Windows: `rusty_v8_release_<target>.lib.gz`)
- `src_binding_release_<target>.rs`
- `licenses_<target>.tar.gz`
- `SHA256SUMS_<target>`

Archive and binding names follow
[rusty_v8's packaging convention](https://github.com/denoland/rusty_v8/blob/v0.106.0/.github/workflows/ci.yml).
The release tag identifies this repository's source, patches, settings, and
upstream submodule pin. There is no separate version manifest or custom cache.

For a published release, download with an authenticated GitHub CLI: this
repository is private, and rusty_v8's URL downloader does not authenticate with
GitHub. Example for a Unix host:

```sh
release=v0.106.0-no-icu.1
target=$(rustc -vV | sed -n 's/^host: //p')
gh release download "$release" --repo moonbitlang/v8-custom-builds \
  --pattern "*${target}*" --dir v8-artifacts
(cd v8-artifacts && shasum -a 256 -c "SHA256SUMS_${target}")
```

From the downstream project, point Cargo at those files, or at the corresponding
source-build outputs:

```sh
artifact_dir=$(cd v8-artifacts && pwd)
target=$(rustc -vV | sed -n 's/^host: //p')
unset V8_FROM_SOURCE GN_ARGS EXTRA_GN_ARGS V8_FORCE_DEBUG
RUSTY_V8_ARCHIVE="$artifact_dir/librusty_v8_release_${target}.a.gz" \
RUSTY_V8_SRC_BINDING_PATH="$artifact_dir/src_binding_release_${target}.rs" \
  cargo build --release -p moonrun
```

On Windows, use the `.lib.gz` asset and the shell's environment-variable syntax.
Keep the Rust crate version, native target, default features, archive, and
bindings matched. The initial upstream version is `v8 = "=0.106.0"`. Simply setting
`V8_FROM_SOURCE=1` in an unmodified downstream dependency does not apply our patch.

## Manual releases

Build from a committed recipe on each intended platform. Check that ICU is absent
and run Moonrun's compatibility tests against the resulting archives before
publishing. The script tests (`python3 -m unittest -v`) simulate compilation and
do not replace native builds or downstream validation.

After validation, tag the recipe commit and collect each target's outputs under
`dist/`. For example:

```sh
release=v0.106.0-no-icu.1
git tag "$release"
git push origin "$release"
gh release create "$release" --repo moonbitlang/v8-custom-builds \
  --verify-tag --draft --title "$release" \
  --notes "ICU-free rusty_v8 archives built from the tagged recipe." dist/*/*
```

Review the draft assets, record the validated targets, and publish manually.
Use a new tag for changed artifacts. There is no CI/CD; GitHub Actions is disabled.
Retain the included upstream license notices when distributing derived artifacts.

## Scope

The ICU patch guards rusty_v8's ICU header and two locale helpers. Its Rust ICU
APIs remain unchanged and must not be called with this archive. V8's non-ICU
date/time-zone fallback remains. Other engine cuts and upgrades are separate work.

A future Rust interface over V8's Wasm C API will keep the same source-or-prebuilt
model. Debugger access remains a requirement. TODO: provide WasmGC host-reference
support; the pinned C API's legacy `ANYREF` means `externref`, so expose it as
`ExternRef` and reserve `AnyRef` for actual WasmGC references.
