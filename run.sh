#!/bin/bash

set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname "$0")" && pwd)"
cd "$SCRIPT_DIR"

mkdir -p dist

cargo build -p engine
cp -f target/debug/engine ${SCRIPT_DIR}/dist/engine

RUSTFLAGS="--emit=llvm-ir" cargo build -p runtime
cp -f "$(ls -t target/debug/deps/runtime-*.ll | head -n1)" dist/runtime.ll
