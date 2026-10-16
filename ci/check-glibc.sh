#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
export WEE8_DIR="$PWD/target/c-api-native"
cargo zigbuild --release --locked --manifest-path ci/c-api/Cargo.toml --target-dir target/c-api --target "$TARGET.2.23"
# A successful link is necessary but also inspect the final dynamic symbol requirements.
required=$(readelf --version-info "target/c-api/$TARGET/release/wee8-smoke" |
  sed -n 's/.*Name: GLIBC_\([0-9.]*\).*/\1/p' | sort -Vu | tail -1)
test -n "$required"
test "$(printf '%s\n' 2.23 "$required" | sort -V | tail -1)" = 2.23
"target/c-api/$TARGET/release/wee8-smoke"
