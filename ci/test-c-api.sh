#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
root=$PWD
if [[ "$TARGET" == *-windows-msvc ]]; then
  root=$(cygpath -m "$root")
fi
out="$root/dist/$TARGET"
(cd "$out"
 if command -v sha256sum >/dev/null; then
   sha256sum -c "SHA256SUMS_$TARGET"
 else
   shasum -a 256 -c "SHA256SUMS_$TARGET"
 fi)
export WEE8_DIR="$root/target/c-api-native"
mkdir -p "$WEE8_DIR"
case "$TARGET" in
  *-windows-msvc) archive=wee8; ext=lib ;;
  *) archive=libwee8; ext=a ;;
esac
gzip -dc "$out/${archive}_release_$TARGET.$ext.gz" > "$WEE8_DIR/$archive.$ext"
cp "$out/wasm_c_api_$TARGET.h" "$WEE8_DIR/wasm.h"
cargo run --release --locked --manifest-path ci/c-api/Cargo.toml --target-dir target/c-api
