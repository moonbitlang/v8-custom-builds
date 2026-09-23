# V8 builds for Moonrun

Build recipes, patches, and downloadable native archives for Moonrun's V8 backend.
Downstream projects keep using the upstream Rust crate. They can download our
trimmed native archive or build the same recipe from source, then select that
archive through `RUSTY_V8_ARCHIVE`.

The first recipe pins rusty_v8 0.106.0 and V8 12.9.202.13 and removes ICU. Its
output contains both V8 and rusty_v8's matching C++ adapter: `librusty_v8.a` on
Unix, or `rusty_v8.lib` on Windows. Rust bindings remain in the upstream crate.
Source revisions are fetched into an ignored directory, not vendored here.

Builds and release publishing are manual. This repository has no CI/CD workflows,
and GitHub Actions is disabled. Each platform needs a validated build before its
artifacts are published; the scripts alone do not establish runtime compatibility.

## Build from source

Install Python 3.11 or later, Git, Rust/Cargo, and the
[pinned rusty_v8 source-build prerequisites](https://github.com/denoland/rusty_v8/blob/v0.106.0/README.md#build-v8-from-source).
Then run from this repository:

```sh
python3 build.py --dry-run
python3 build.py --jobs 8
```

The script fetches the exact commits in `versions.json`, initializes submodules,
verifies the V8 revision, applies patches in manifest order, and invokes Cargo's
source-build path. Cargo runs GN and Ninja and generates the Rust layout bindings.
`--jobs` limits compilation concurrency; `--prepare-only` stops before compilation.

The recipe preserves rusty_v8's default Cargo features, including its bundled C++
runtime. Its GN overrides are:

```gn
v8_enable_i18n_support=false
icu_use_data_file=false
```

The build script recognizes native builds for `aarch64-apple-darwin`,
`x86_64-apple-darwin`, `aarch64-unknown-linux-gnu`, `x86_64-unknown-linux-gnu`, and
`x86_64-pc-windows-msvc`. Cross-compilation is not configured here.

Sources and Cargo/GN outputs live under `build/<recipe-hash>/<target>/`. Changing
the manifest, patches, or build script selects a fresh directory. The script
rejects edits to its prepared sources and conflicting build overrides. If source
preparation is interrupted, retry; if it reports an incomplete or edited checkout,
move that recipe's build directory aside and retry. Packaging never overwrites
an existing release/target directory.

Outputs are written under `dist/<release>/<target>/`:

| Artifact | Contents |
| --- | --- |
| `librusty_v8_release_<target>.a.gz` | Complete native archive; Windows uses `rusty_v8_release_<target>.lib.gz` |
| `src_binding_release_<target>.rs` | Matching generated Rust layout bindings |
| `licenses_<target>.tar.gz` | Upstream license and notice files |
| `manifest_<target>.json` | Source pins, recipe input hashes, GN overrides, and artifact hashes |
| `SHA256SUMS_<target>` | SHA-256 checksums for all four artifacts |

Source and configuration inputs are pinned; this is not a claim of identical
machine code across different compiler or SDK installations. Compression and
notice packaging omit timestamps so packaging itself does not add that variation.

Running `V8_FROM_SOURCE=1` against an unmodified upstream dependency does not apply
our patches. For a trimmed source build, run this recipe and consume its output
using the same downstream settings as a downloaded archive.

## Download and consume an archive

The repository is private, so downloads require access. rusty_v8's ordinary URL
downloader does not authenticate with GitHub. Use an authenticated GitHub CLI to
download a published release, then give Cargo local file paths. These Unix shell
examples assume the recipe's native target and a matching `v8 = "=0.106.0"`
dependency with its default features enabled:

```sh
release=v0.106.0-no-icu.1
target=$(rustc -vV | sed -n 's/^host: //p')
gh release download "$release" --repo moonbitlang/v8-custom-builds \
  --pattern "*${target}*" --dir v8-artifacts
(cd v8-artifacts && shasum -a 256 -c "SHA256SUMS_${target}")
```

After downloading, or after copying this recipe's source-build outputs into
`v8-artifacts`, run from the downstream project:

```sh
artifact_dir=$(cd v8-artifacts && pwd)
target=$(rustc -vV | sed -n 's/^host: //p')
unset V8_FROM_SOURCE GN_ARGS EXTRA_GN_ARGS V8_FORCE_DEBUG
RUSTY_V8_ARCHIVE="$artifact_dir/librusty_v8_release_${target}.a.gz" \
RUSTY_V8_SRC_BINDING_PATH="$artifact_dir/src_binding_release_${target}.rs" \
  cargo build --release -p moonrun
```

On Windows, select the `.lib.gz` asset instead and set the same environment
variables using the shell's syntax. Keep the crate version, target, feature
configuration, archive, and generated bindings matched. A bare V8 or `wee8`
archive cannot satisfy the current rusty_v8 adapter symbols.

## Publish manually

Before adopting or publishing a target's artifacts, inspect the native archive
for remaining ICU implementations and run Moonrun's compatibility suite against
it. Compare size with an ICU-enabled build using the same source revisions,
compiler, and other flags. Keep generated bindings and notices with the archive.

Commit the recipe and select a new `release` in `versions.json` for each changed
configuration. Build from that committed recipe on each intended platform. After
validation, tag that commit and create a draft release with the resulting assets:

```sh
release=v0.106.0-no-icu.1
git tag "$release"
git push origin "$release"
gh release create "$release" --repo moonbitlang/v8-custom-builds \
  --verify-tag --draft --title "$release" \
  --notes "ICU-free V8 archives. See asset manifests for source pins and configuration." \
  dist/"$release"/*/*
```

Review the draft's assets and record the validated targets and compatibility
results in its notes, then publish it manually. Do not replace assets under an
existing release: issue a new recipe release instead. No release is created by
`build.py`.

## ICU patch and remaining scope

`patches/no-icu.patch` guards the ICU locale header and its two C++ locale helpers
with `V8_INTL_SUPPORT`. V8's own build option removes its ICU dependency;
`icu_use_data_file=false` also avoids the ICU data-file dependency during snapshot
creation. V8's non-ICU date/time-zone fallback remains in place.

The Rust ICU APIs are unchanged. Moonrun does not call them; the ICU-free archive
does not provide their implementations. Calling them would fail to link. Other
engine cuts and a V8 version upgrade remain separate work.

A future Rust interface over V8's Wasm C API will follow the same distribution
model: build the pinned, patched native library from source or download a matching
archive. That migration and any C API extensions are not implemented here yet.
Debugger access is a requirement of that interface; MoonBit host behavior stays
in Moonrun.

TODO: add proper WasmGC host-reference support. In the pinned C API, legacy
`ANYREF` denotes `externref`; expose that as `ExternRef` in Rust. Reserve `AnyRef`
for actual WasmGC references. Guest-internal WasmGC execution and host access to
WasmGC values are separate capabilities.

## Script checks

```sh
python3 -m unittest -v
```

These tests exercise source pinning, patching, cache validation, and packaging
using temporary Git repositories and fixture archives. They do not compile V8 or
replace downstream compatibility tests.

## Upstream sources

- [rusty_v8](https://github.com/denoland/rusty_v8) provides Rust bindings, a C++ adapter,
  and Cargo integration for the native build.
- [V8](https://v8.dev/) provides the engine and its Wasm C API implementation.

Upstream source and binary license obligations apply to derived artifacts.
