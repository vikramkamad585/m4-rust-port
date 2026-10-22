#!/usr/bin/env python3
"""Local estimate of the score, using the visible example cases.

Builds /app/m4rs exactly the way the grader does (only src/**/*.rs is
copied into a fresh crate with the reference Cargo.toml, then
`cargo build --release --offline`), runs the source policy, and runs every
case in /app/examples/cases.jsonl.

  python3 /app/tools/selfcheck.py                 # build + policy + all cases
  python3 /app/tools/selfcheck.py -k regex        # only categories/ids matching
  python3 /app/tools/selfcheck.py --show v00012   # print one case with a diff
  python3 /app/tools/selfcheck.py --bin PATH      # skip the build, test PATH
  python3 /app/tools/selfcheck.py --failures N    # list up to N failing ids
"""
import argparse
import concurrent.futures as cf
import difflib
import json
import os
import shutil
import subprocess
import sys
import tempfile

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import policy  # noqa: E402

CRATE = "/app/m4rs"
CASES = "/app/examples/cases.jsonl"
CARGO_TOML = """[package]
name = "m4rs"
version = "0.1.0"
edition = "2021"

[dependencies]

[profile.release]
opt-level = 3
debug = false
"""


def build(crate):
    work = tempfile.mkdtemp(prefix="m4rs-build-")
    src = os.path.join(crate, "src")
    for root, _, files in os.walk(src):
        for f in files:
            if f.endswith(".rs"):
                rel = os.path.relpath(os.path.join(root, f), src)
                dst = os.path.join(work, "src", rel)
                os.makedirs(os.path.dirname(dst), exist_ok=True)
                shutil.copyfile(os.path.join(root, f), dst)
    with open(os.path.join(work, "Cargo.toml"), "w") as fh:
        fh.write(CARGO_TOML)
    target = os.path.join(tempfile.gettempdir(), "m4rs-selfcheck-target")
    r = subprocess.run(["cargo", "build", "--release", "--offline", "--quiet"], cwd=work,
                       env=dict(os.environ, CARGO_TARGET_DIR=target), capture_output=True, text=True)
    if r.returncode != 0:
        print(r.stderr[-6000:])
        return None
    binp = os.path.join(target, "release", "m4rs")
    keep = os.path.join(tempfile.gettempdir(), "m4rs-selfcheck-bin")
    shutil.copyfile(binp, keep)
    os.chmod(keep, 0o755)
    shutil.rmtree(work, ignore_errors=True)
    return keep


def run_case(binary, case, timeout=10):
    with tempfile.TemporaryDirectory(prefix="m4case-") as d:
        for name, text in case["files"].items():
            p = os.path.join(d, name)
            os.makedirs(os.path.dirname(p), exist_ok=True)
            with open(p, "wb") as fh:
                fh.write(text.encode("latin-1"))
        try:
            r = subprocess.run([binary] + case["args"], cwd=d, input=case["stdin"].encode("latin-1"),
                               capture_output=True, timeout=timeout,
                               env={"PATH": "/usr/bin:/bin", "LC_ALL": "C"})
        except subprocess.TimeoutExpired:
            return None
        return {"stdout": r.stdout.decode("latin-1"), "stderr": r.stderr.decode("latin-1"),
                "exit": r.returncode}


def passed(case, got):
    exp = case["expected"]
    return (got is not None and got["stdout"] == exp["stdout"] and got["exit"] == exp["exit"]
            and (not case["check_stderr"] or got["stderr"] == exp["stderr"]))


def show(case, got):
    print("id=%s category=%s args=%s check_stderr=%s" % (case["id"], case["category"], case["args"], case["check_stderr"]))
    for name, text in case["files"].items():
        print("---- file %s ----\n%s" % (name, text), end="" if text.endswith("\n") else "\n")
    if case["stdin"]:
        print("---- stdin ----\n" + case["stdin"])
    if got is None:
        print("---- TIMEOUT ----")
        return
    for k in ("stdout", "stderr"):
        e, g = case["expected"][k], got[k]
        if e != g:
            print("---- %s diff (expected -> got) ----" % k)
            sys.stdout.writelines(difflib.unified_diff(e.splitlines(True), g.splitlines(True), "expected", "got"))
            print()
    print("exit expected=%d got=%d" % (case["expected"]["exit"], got["exit"]))


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("-k", default="")
    ap.add_argument("--show")
    ap.add_argument("--bin")
    ap.add_argument("--failures", type=int, default=15)
    a = ap.parse_args()
    probs = policy.check_tree(os.path.join(CRATE, "src"))
    for p in probs:
        print("policy:", p)
    print("policy gate:", "PASS" if not probs else "FAIL (score would be 0)")
    binary = a.bin or build(CRATE)
    if not binary:
        print("build gate: FAIL (score would be 0)")
        sys.exit(1)
    print("build gate: PASS")
    cases = [json.loads(l) for l in open(CASES)]
    if a.show:
        c = next(c for c in cases if c["id"] == a.show)
        show(c, run_case(binary, c))
        return
    cases = [c for c in cases if a.k in c["id"] or a.k in c["category"]]
    per, fails = {}, []
    with cf.ThreadPoolExecutor(os.cpu_count() or 4) as ex:
        for c, got in zip(cases, ex.map(lambda c: run_case(binary, c), cases)):
            t = per.setdefault(c["category"], [0, 0])
            t[1] += 1
            if passed(c, got):
                t[0] += 1
            else:
                fails.append(c["id"])
    for k in sorted(per):
        print("  %-16s %4d / %4d" % (k, per[k][0], per[k][1]))
    ok = sum(v[0] for v in per.values())
    tot = sum(v[1] for v in per.values())
    print("visible pass rate: %d / %d = %.4f" % (ok, tot, ok / max(tot, 1)))
    if not probs:
        print("estimated reward (if the hidden set behaves like this sample): %.4f" % (ok / max(tot, 1)))
    if fails:
        print("first failures:", " ".join(fails[:a.failures]))


if __name__ == "__main__":
    main()
