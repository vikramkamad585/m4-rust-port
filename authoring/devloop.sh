#!/bin/sh
# Rebuild the reference and check it against a frozen case file.
set -e
cd "$(dirname "$0")/.."
FROZEN=${1:-out/dev.frozen.jsonl}
docker run --rm -v "$PWD":/work -v m4cargo:/usr/local/cargo/registry -w /work/authoring rust:1.83.0-slim-bookworm sh -c 'rm -rf refbuild/src && cp -r /work/m4-rust-port/solution/src refbuild/src && cd refbuild && cargo build --release 2>&1 | grep -E "^(error|warning)" -A 8 | head -60'
docker run --rm -v "$PWD/authoring":/w -w /w m4oracle python3 run_cases.py check "$FROZEN" /w/refbuild/target/release/m4rs --fails out/fails.jsonl
