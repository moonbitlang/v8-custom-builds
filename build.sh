#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")"
root=$PWD
target=$(rustc -vV | sed -n 's/^host: //p' | tr -d '\r')
out="$root/dist/$target"
if [[ -e "$out" ]]; then
  echo "Move dist/$target aside before rebuilding." >&2
  exit 1
fi

# Native builds explicitly opt into source checkout; Cargo consumers skip it.
git submodule update --init --recursive --checkout --depth=1
for patch in "$root"/patches/*.patch; do
  git -C rusty_v8 apply --reverse --check "$patch" 2>/dev/null ||
    git -C rusty_v8 apply "$patch"
done

cp ci/runtime/probe.cc rusty_v8/wee8-runtime-check.cc
cp ci/runtime/gc-bytes.h rusty_v8/gc-bytes.h
unset DOCS_RS DENO_TRYBUILD V8_FORCE_DEBUG EXTRA_GN_ARGS
mkdir -p rusty_v8/custom_wee8
cp gn/custom_wee8/BUILD.gn rusty_v8/custom_wee8/BUILD.gn
unset GN_ARGS
cargo build -vv --release --locked --manifest-path tools/native/Cargo.toml \
  --target "$target" --target-dir "$root/build"

case "$target" in
  *-windows-msvc) c_api=wee8; ext=lib; exe=.exe ;;
  *) c_api=libwee8; ext=a; exe= ;;
esac
gn_out="$root/build/$target/release/gn_out"
mkdir -p "$out"
gzip -9nc "$gn_out/custom_wee8/obj/v8/$c_api.$ext" > "$out/${c_api}_release_$target.$ext.gz"
mkdir -p "$root/target/native-checks/$target"
cp "$gn_out/custom_wee8/wee8-runtime-check$exe" "$root/target/native-checks/$target/"
cp rusty_v8/v8/third_party/wasm-api/wasm.h "$out/wasm_c_api_$target.h"
cp "$root/licenses/wasm-c-api.txt" "$out/WASM_API_LICENSE_$target.txt"

# Keep notices from the complete source tree without archive metadata.
git -C rusty_v8 ls-files --recurse-submodules -z -- \
  ':(glob,icase)**/LICENSE*' ':(glob,icase)**/COPYING*' ':(glob,icase)**/NOTICE*' |
  while IFS= read -r -d '' notice; do
    printf '\n===== %s =====\n' "$notice"
    cat "rusty_v8/$notice"
  done > "$out/NOTICES_$target.txt"

cd "$out"
if command -v sha256sum >/dev/null; then
  sha256sum ./*.gz ./*.h ./*.txt
else
  shasum -a 256 ./*.gz ./*.h ./*.txt
fi > "SHA256SUMS_$target"
