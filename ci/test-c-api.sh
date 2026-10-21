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

# Run the behavioral tests against this archive, including exception propagation
# and ownership. The C consumer above checks the packaged header/archive ABI.
export WEE8_ARCHIVE="$WEE8_DIR/$archive.$ext"
export WEE8_HEADER="$WEE8_DIR/wasm.h"
cargo test --locked --manifest-path crates/wee8/Cargo.toml --target-dir target/wee8
