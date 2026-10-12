#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
out="$PWD/dist/$TARGET"
export RUSTY_V8_ARCHIVE="$out/librusty_v8_release_$TARGET.a.gz"
export RUSTY_V8_SRC_BINDING_PATH="$out/src_binding_release_$TARGET.rs"
unset V8_FROM_SOURCE GN_ARGS EXTRA_GN_ARGS V8_FORCE_DEBUG
cd moon
cargo zigbuild --release --locked -p moonrun --target "$TARGET.2.23"
# A successful link is necessary but also inspect the final dynamic symbol requirements.
required=$(readelf --version-info "target/$TARGET/release/moonrun" |
  sed -n 's/.*Name: GLIBC_\([0-9.]*\).*/\1/p' | sort -Vu | tail -1)
test -n "$required"
test "$(printf '%s\n' 2.23 "$required" | sort -V | tail -1)" = 2.23
"target/$TARGET/release/moonrun" --version
