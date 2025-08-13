#!/bin/bash

set -euo pipefail

cd $(dirname $0)

mkdir -p dist

RUSTFLAGS="--emit=obj" cargo build -p runtime
cp target/debug/libruntime.a dist/libruntime.a

cargo run compile sample.bf --sanitize > dist/main.ll

clang dist/main.ll dist/libruntime.a -o dist/out

./dist/out
