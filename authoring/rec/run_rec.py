#!/usr/bin/env python3
"""Run cases against a binary.

  run_cases.py freeze CASES.jsonl OUT.jsonl BINARY
      run each case twice through GNU recutils (BINARY = GNU); keep deterministic, fast,
      bounded cases and attach expected stdout/stderr/exit.
  run_cases.py check FROZEN.jsonl BINARY [--fails FAILS.jsonl] [--keep PASSED.jsonl]
      run BINARY and compare against the frozen expectations.
"""
import concurrent.futures as cf
import json
import os
import subprocess
import sys
import tempfile

TIMEOUT = 5
MAX_OUT = 64 * 1024


def run_one(case, binary, timeout=TIMEOUT):
    with tempfile.TemporaryDirectory() as d:
        for name, text in case["files"].items():
            p = os.path.join(d, name)
            os.makedirs(os.path.dirname(p), exist_ok=True)
            with open(p, "wb") as f:
                f.write(text.encode("latin-1"))
        try:
            cmd = ([case["tool"]] if binary == "GNU" else [binary, case["tool"]]) + case["args"]
            pr = subprocess.run(cmd, cwd=d, input=case["stdin"].encode("latin-1"),
                                capture_output=True, timeout=timeout,
                                env={"PATH": "/usr/bin:/bin", "LC_ALL": "C"})
        except subprocess.TimeoutExpired:
            return None
        return {"stdout": pr.stdout.decode("latin-1"), "stderr": pr.stderr.decode("latin-1"),
                "exit": pr.returncode}


def freeze(cases_path, out_path, binary):
    cases = [json.loads(l) for l in open(cases_path)]

    def work(c):
        a = run_one(c, binary)
        b = run_one(c, binary)
        if a is None or b is None or a != b:
            return None
        if len(a["stdout"]) > MAX_OUT or len(a["stderr"]) > MAX_OUT or a["exit"] < 0:
            return None
        if "\0" in a["stdout"] or "\0" in a["stderr"]:
            return None
        if any(t in a["stderr"] for t in ("internal error", "stack overflow", "memory exhausted", "Memory exhausted")):
            return None
        c = dict(c)
        c["expected"] = a
        return c

    kept = 0
    with cf.ThreadPoolExecutor(8) as ex, open(out_path, "w") as f:
        for r in ex.map(work, cases):
            if r is not None:
                f.write(json.dumps(r) + "\n")
                kept += 1
    print("froze %d / %d" % (kept, len(cases)))


def check(frozen_path, binary, fails_path=None, keep_path=None):
    cases = [json.loads(l) for l in open(frozen_path)]

    def work(c):
        got = run_one(c, binary, 10)
        exp = c["expected"]
        ok = got is not None and got["stdout"] == exp["stdout"] and got["exit"] == exp["exit"] and (
            not c["check_stderr"] or got["stderr"] == exp["stderr"])
        return c, got, ok

    per = {}
    fails = []
    passed = []
    for c, got, ok in cf.ThreadPoolExecutor(8).map(work, cases):
        t = per.setdefault(c["category"], [0, 0])
        t[1] += 1
        if ok:
            t[0] += 1
            passed.append(c)
        else:
            fails.append({"case": c, "got": got})
    tot = sum(v[0] for v in per.values()), sum(v[1] for v in per.values())
    for k, v in sorted(per.items()):
        print("%-16s %4d/%4d" % (k, v[0], v[1]))
    print("TOTAL %d/%d = %.4f" % (tot[0], tot[1], tot[0] / max(1, tot[1])))
    if fails_path:
        with open(fails_path, "w") as f:
            for x in fails:
                f.write(json.dumps(x) + "\n")
    if keep_path:
        with open(keep_path, "w") as f:
            for c in passed:
                f.write(json.dumps(c) + "\n")


if __name__ == "__main__":
    if sys.argv[1] == "freeze":
        freeze(sys.argv[2], sys.argv[3], sys.argv[4])
    else:
        args = sys.argv[2:]
        fails = keep = None
        if "--fails" in args:
            fails = args[args.index("--fails") + 1]
        if "--keep" in args:
            keep = args[args.index("--keep") + 1]
        check(args[0], args[1], fails, keep)
