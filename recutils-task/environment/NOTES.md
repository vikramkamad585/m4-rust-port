# recrs

    recrs/          Rust crate to implement (binary `recrs`, std only)
    reference/      GNU recutils 1.9 manual (text) and man pages
    samples/<id>/   445 recorded invocations of the real tools:
                      case.json        tool, args, stdin, compare_stderr, group
                      in/              files present in the working directory
                      expected.stdout  expected.stderr  expected.status
    check.py        build like the grader, check rules.py, run all samples
    rules.py        source rules enforced at grading

    cd /app/recrs && cargo build --release --offline
    ./target/release/recrs recsel -t Book /app/samples/v00001/in/data.rec
    python3 /app/check.py -g group
    python3 /app/check.py --diff v00042
