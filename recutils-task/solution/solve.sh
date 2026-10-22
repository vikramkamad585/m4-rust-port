#!/usr/bin/env bash
# Reference solution: install the complete Rust implementation into the crate.
set -euo pipefail
SRC="$(cd "$(dirname "$0")" && pwd)/src"
rm -rf /app/recrs/src
mkdir -p /app/recrs/src
cp "$SRC"/*.rs /app/recrs/src/
cd /app/recrs
cargo build --release --offline --quiet
printf 'a: 1\n' | ./target/release/recrs recsel
