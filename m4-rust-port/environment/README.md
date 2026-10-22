# m4rs workspace

    m4rs/            the Rust crate you implement (binary `m4rs`, std only)
      Cargo.toml     informational; grading uses its own identical manifest
      src/main.rs    entry point (currently a stub that exits 70)
    docs/m4-manual.txt   GNU M4 1.4.19 manual, plain text (the specification)
    docs/m4.1            GNU M4 1.4.19 man page
    examples/cases.jsonl 494 worked cases with GNU m4's exact output
    tools/selfcheck.py   build like the grader, check policy, run the cases
    tools/policy.py      the source policy enforced at grading
    timer.sh, .timer/    remaining-time files (see the task brief)

Quick loop:

    cd /app/m4rs && cargo build --release --offline
    python3 /app/tools/selfcheck.py            # score estimate
    python3 /app/tools/selfcheck.py --show v00012
    printf 'define(`x', `y')x\n' | ./target/release/m4rs

Case format (one JSON object per line): `files` are written into an empty
working directory, the binary runs there as `m4rs ARGS...` with `stdin` on
standard input, and `expected` holds GNU m4's stdout, stderr and exit status.
A case passes when stdout and the exit status match exactly and, when
`check_stderr` is true, stderr matches byte for byte as well.
