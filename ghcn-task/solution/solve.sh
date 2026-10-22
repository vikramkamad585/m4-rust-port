#!/usr/bin/env bash
# Reference solution: install the context-mixing codec in /app/codec.
set -euo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
cp "$HERE/codec/codec.cpp" "$HERE/codec/Makefile" /app/codec/
make -C /app/codec -B
f=$(ls /app/data/dev/*.dly | head -1)
/app/codec/codec compress "$f" /tmp/solve.bin && /app/codec/codec decompress /tmp/solve.bin /tmp/solve.out && cmp "$f" /tmp/solve.out
