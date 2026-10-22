#!/usr/bin/env python3
"""Build /app/recrs the way grading does, check the source rules, and run
the sample cases in /app/samples.

  python3 /app/check.py                  # everything, with a score estimate
  python3 /app/check.py -g sort          # only groups / ids containing TEXT
  python3 /app/check.py --diff v00042    # show one case and the differences
  python3 /app/check.py --bin PATH       # test an existing binary, no build
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
import rules  # noqa: E402

CRATE = "/app/recrs"
SAMPLES = "/app/samples"
MANIFEST = """[package]
name = "recrs"
version = "0.1.0"
edition = "2021"

[dependencies]

[profile.release]
opt-level = 3
debug = false
"""


def build():
    work = tempfile.mkdtemp(prefix="recrs-check-")
    src = os.path.join(CRATE, "src")
    for root, _, files in os.walk(src):
        for f in files:
            if f.endswith(".rs"):
                rel = os.path.relpath(os.path.join(root, f), src)
                dst = os.path.join(work, "src", rel)
                os.makedirs(os.path.dirname(dst), exist_ok=True)
                shutil.copyfile(os.path.join(root, f), dst)
    with open(os.path.join(work, "Cargo.toml"), "w") as fh:
        fh.write(MANIFEST)
    target = os.path.join(tempfile.gettempdir(), "recrs-check-target")
    r = subprocess.run(["cargo", "build", "--release", "--offline", "--quiet"], cwd=work,
                       env=dict(os.environ, CARGO_TARGET_DIR=target), capture_output=True, text=True)
    shutil.rmtree(work, ignore_errors=True)
    if r.returncode != 0:
        print(r.stderr[-6000:])
        return None
    out = os.path.join(tempfile.gettempdir(), "recrs-check-bin")
    shutil.copyfile(os.path.join(target, "release", "recrs"), out)
    os.chmod(out, 0o755)
    return out


def load(case_dir):
    meta = json.load(open(os.path.join(case_dir, "case.json")))
    rd = lambda n: open(os.path.join(case_dir, n), "rb").read().decode("latin-1")
    meta["expected"] = {"stdout": rd("expected.stdout"), "stderr": rd("expected.stderr"),
                        "status": int(rd("expected.status"))}
    meta["dir"] = case_dir
    meta["id"] = os.path.basename(case_dir)
    return meta


def run(binary, case):
    with tempfile.TemporaryDirectory(prefix="recrs-case-") as d:
        src = os.path.join(case["dir"], "in")
        if os.path.isdir(src):
            shutil.copytree(src, d, dirs_exist_ok=True)
        try:
            r = subprocess.run([binary, case["tool"]] + case["args"], cwd=d,
                               input=case["stdin"].encode("latin-1"), capture_output=True, timeout=10,
                               env={"PATH": "/usr/bin:/bin", "LC_ALL": "C"})
        except subprocess.TimeoutExpired:
            return None
        return {"stdout": r.stdout.decode("latin-1"), "stderr": r.stderr.decode("latin-1"), "status": r.returncode}


def ok(case, got):
    e = case["expected"]
    return (got is not None and got["stdout"] == e["stdout"] and got["status"] == e["status"]
            and (not case["compare_stderr"] or got["stderr"] == e["stderr"]))


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("-g", default="")
    ap.add_argument("--diff")
    ap.add_argument("--bin")
    a = ap.parse_args()
    problems = rules.check_tree(os.path.join(CRATE, "src"))
    for p in problems:
        print("rules:", p)
    print("source rules:", "ok" if not problems else "VIOLATED (would score 0)")
    binary = a.bin or build()
    if not binary:
        print("build: FAILED (would score 0)")
        sys.exit(1)
    print("build: ok")
    cases = [load(os.path.join(SAMPLES, d)) for d in sorted(os.listdir(SAMPLES))]
    if a.diff:
        c = next(c for c in cases if c["id"] == a.diff)
        print("$ recrs %s %s   (compare_stderr=%s)" % (c["tool"], " ".join(c["args"]), c["compare_stderr"]))
        got = run(binary, c)
        if got is None:
            print("TIMEOUT")
            return
        for k in ("stdout", "stderr"):
            if got[k] != c["expected"][k]:
                print("--- %s (expected -> got)" % k)
                sys.stdout.writelines(difflib.unified_diff(c["expected"][k].splitlines(True), got[k].splitlines(True)))
        print("status expected=%d got=%d" % (c["expected"]["status"], got["status"]))
        return
    cases = [c for c in cases if a.g in c["group"] or a.g in c["id"]]
    per, bad = {}, []
    with cf.ThreadPoolExecutor(os.cpu_count() or 4) as ex:
        for c, got in zip(cases, ex.map(lambda c: run(binary, c), cases)):
            t = per.setdefault(c["group"], [0, 0])
            t[1] += 1
            if ok(c, got):
                t[0] += 1
            else:
                bad.append(c["id"])
    for k in sorted(per):
        print("  %-12s %4d / %4d" % (k, per[k][0], per[k][1]))
    good = sum(v[0] for v in per.values())
    total = sum(v[1] for v in per.values())
    print("samples passed: %d / %d = %.4f" % (good, total, good / max(total, 1)))
    if not problems:
        print("estimated score: %.4f" % (good / max(total, 1)))
    if bad:
        print("failing:", " ".join(bad[:20]))


if __name__ == "__main__":
    main()
