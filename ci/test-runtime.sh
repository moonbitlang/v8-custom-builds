#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
probe="target/native-checks/$TARGET/wee8-runtime-check"
if [[ "$TARGET" == *-windows-msvc ]]; then
  probe+=.exe
fi
"$probe" '--liftoff-only --no-wasm-lazy-compilation'
"$probe" '--no-liftoff --no-wasm-lazy-compilation'
"$probe" '--wasm-lazy-compilation'
"$probe" '--wasm-sync-tier-up --wasm-tiering-budget=1'
