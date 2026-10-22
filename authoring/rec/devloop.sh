#!/bin/sh
# Rebuild the reference and check it against a frozen case file.
set -e
cd "$(dirname "$0")"
FROZEN=${1:-out/dev.frozen.jsonl}
docker run --rm -v "$PWD/ref":/w -v m4cargo:/usr/local/cargo/registry -w /w rust:1.83.0-slim-bookworm sh -c 'cargo build --release 2>&1 | grep -E "^error" -A 12 | head -60'
docker run --rm -v "$PWD":/w -w /w recoracle python3 run_rec.py check "$FROZEN" /w/ref/target/release/recrs --fails out/fails.jsonl
