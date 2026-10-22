# m4-rust-port

A long-horizon software-engineering task: **port GNU m4 1.4.19 to a
dependency-free Rust binary** that is byte-compatible with the real tool. It
is graded automatically on 2,175 hidden m4 programs. The expected output for
each one was captured from GNU m4 itself.

| | |
|---|---|
| Class | Implementation |
| Budget | 8 hours, CPU only, offline |
| Deliverable | Rust crate `/app/m4rs` (std only), binary `m4rs` |
| Score | cases passed ÷ 2,175; 0 if the build, source policy or toolchain-integrity check fails |
| Reference solution | scores 1.0 (2,175 / 2,175 hidden, 494 / 494 visible) |

## Repository layout

```
m4-rust-port/              the task bundle (this directory is what gets zipped)
  task.toml                task metadata, budget, resources
  instruction.md           full engineering brief given to the implementer
  PROVENANCE.md            upstream source, how cases and reference were built
  environment/             becomes /app in the container
    Dockerfile             rust:1.83.0-slim-bookworm (digest-pinned) + python3
    m4rs/                  stub crate (exits 70)
    docs/                  GNU M4 1.4.19 manual (plain text) and man page
    examples/cases.jsonl   494 visible cases with GNU m4's exact output
    tools/selfcheck.py     local build + policy + score estimate
    tools/policy.py        source rules enforced at grading
    timer.sh               remaining-time daemon
  tests/                   sealed grader (mounted only at grading time)
    test.sh                hardens the environment, runs collect.py
    collect.py             integrity, policy, clean build, runs hidden cases
    compute_reward.py      gates and score, writes reward.{txt,json}, details.json
    cases/hidden.jsonl.gz  2,175 hidden cases
    toolchain_manifest.json  SHA-256 of the Rust toolchain (amd64, arm64)
    authoring/             case generator and output freezer
  solution/
    solve.sh               installs the reference into /app/m4rs
    src/*.rs               reference port (~3,850 non-blank lines of Rust)
authoring/                 development scripts (generator, freezer, dev loop)
```

## How the cases were made

`gen_cases.py` generates seeded random m4 programs in 21 categories: quoting
and rescanning, arguments, definition stacks, control idioms, multi-character
delimiters, diversions, `m4wrap`, includes, `eval`, string builtins,
`regexp`/`patsubst`, `format`, command lines, diagnostics, line tracking,
and realistic macro libraries. `run_cases.py freeze` runs each program twice
through Debian bookworm's `m4 1.4.19-3` and keeps only deterministic,
bounded results. The visible set (seed 4242) and the hidden set (seed 7331)
do not overlap.

## Running it locally

Requires Docker and [harbor](https://pypi.org/project/harbor/).

```sh
cd m4-rust-port
harbor run -p . -a nop    -e docker -k 1   # untouched stub: reward 0.0
harbor run -p . -a oracle -e docker -k 2   # reference:      reward 1.0
```

Inside the task container, the implementer's loop is:

```sh
cd /app/m4rs && cargo build --release --offline
python3 /app/tools/selfcheck.py            # per-category pass rates, estimated score
python3 /app/tools/selfcheck.py --show v00012
```

## Licensing

The bundled manual (`environment/docs/`) is part of GNU M4 and is distributed
under the GNU Free Documentation License 1.3. The reference solution was
written against the GNU M4 1.4.19 sources, which are licensed under the GPLv3.
