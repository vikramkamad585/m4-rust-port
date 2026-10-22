#!/usr/bin/env bash
# Reference solution: install the full Rust m4 implementation into the crate.
set -euo pipefail
SRC="$(cd "$(dirname "$0")" && pwd)/src"
rm -rf /app/m4rs/src
mkdir -p /app/m4rs/src
cp "$SRC"/*.rs /app/m4rs/src/
cd /app/m4rs
cargo build --release --offline --quiet
printf 'define(`x'"'"', `ok'"'"')x\n' | ./target/release/m4rs
