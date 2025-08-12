#!/bin/bash

set -euo pipefail

cd $(dirname $0)

mkdir -p dist

cargo build -p engine
cp -f target/debug/engine dist/engine

RUSTFLAGS="--emit=obj" cargo build -p runtime
cp target/debug/libruntime.a dist/libruntime.a

cargo run compile sample.bf --sanitize > dist/prog.ll

clang dist/prog.ll dist/libruntime.a -o dist/out

./dist/out
