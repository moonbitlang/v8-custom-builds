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

git submodule update --init --recursive --depth=1
for patch in "$root"/patches/*.patch; do
  git -C rusty_v8 apply --reverse --check "$patch" 2>/dev/null ||
    git -C rusty_v8 apply "$patch"
done

cd rusty_v8
unset DOCS_RS DENO_TRYBUILD V8_FORCE_DEBUG EXTRA_GN_ARGS
export V8_FROM_SOURCE=1
export GN_ARGS='v8_enable_i18n_support=false icu_use_data_file=false'
cargo build -vv --release --locked --target "$target" --target-dir "$root/build" -p v8

case "$target" in
  *-windows-msvc) lib=rusty_v8; ext=lib ;;
  *) lib=librusty_v8; ext=a ;;
esac
gn_out="$root/build/$target/release/gn_out"
mkdir -p "$out"
gzip -9nc "$gn_out/obj/$lib.$ext" > "$out/${lib}_release_$target.$ext.gz"
cp "$gn_out/src_binding.rs" "$out/src_binding_release_$target.rs"

# Keep notices from the complete source tree without archive metadata.
git ls-files --recurse-submodules -z -- \
  ':(glob,icase)**/LICENSE*' ':(glob,icase)**/COPYING*' ':(glob,icase)**/NOTICE*' |
  while IFS= read -r -d '' notice; do
    printf '\n===== %s =====\n' "$notice"
    cat "$notice"
  done > "$out/NOTICES_$target.txt"

cd "$out"
if command -v sha256sum >/dev/null; then
  sha256sum ./*.gz ./*.rs ./*.txt
else
  shasum -a 256 ./*.gz ./*.rs ./*.txt
fi > "SHA256SUMS_$target"
