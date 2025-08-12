#!/bin/bash

set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname "$0")" && pwd)"
cd "$SCRIPT_DIR"

mkdir -p dist

cargo build -p engine
cp -f target/debug/engine ${SCRIPT_DIR}/dist/engine

RUSTFLAGS="--emit=obj" cargo build -p runtime
cp target/debug/libruntime.a dist/libruntime.a

cargo run compile sample.bf > dist/prog.ll

clang dist/prog.ll dist/libruntime.a -o dist/out
