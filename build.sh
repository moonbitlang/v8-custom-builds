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
python3 tools/prepare-source.py
unset DOCS_RS DENO_TRYBUILD V8_FORCE_DEBUG EXTRA_GN_ARGS
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
for probe in wee8-runtime-check cpp-entry-contract runtime-entry-contract generated-builtin-contract \
             generated-builtin-catalog runtime-catalog runtime-boundaries store-lifetimes; do
  cp "$gn_out/custom_wee8/$probe$exe" "$root/target/native-checks/$target/"
done
cp custom_wee8/include/wasm.h "$out/wasm_c_api_$target.h"
cp custom_wee8/include/wee8.h "$out/wee8_c_api_$target.h"
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
