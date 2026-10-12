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
case "$TARGET" in
  *-windows-msvc) export RUSTY_V8_ARCHIVE="$out/rusty_v8_release_$TARGET.lib.gz" ;;
  *) export RUSTY_V8_ARCHIVE="$out/librusty_v8_release_$TARGET.a.gz" ;;
esac
export RUSTY_V8_SRC_BINDING_PATH="$out/src_binding_release_$TARGET.rs"
unset V8_FROM_SOURCE GN_ARGS EXTRA_GN_ARGS V8_FORCE_DEBUG

# Run the probe as a temporary Moonrun example to use the exact downstream crate and features.
mkdir -p moon/crates/moonrun/examples
cp ci/probe.rs moon/crates/moonrun/examples/v8_no_icu.rs
cd moon
moon version --all
cargo run --release --locked -p moonrun --example v8_no_icu
cargo test --release --locked -p moonrun
