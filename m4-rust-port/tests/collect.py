#!/usr/bin/env python3
"""Evidence collection for m4rs: integrity checks, source policy, a clean
offline build, and the sealed differential cases. Writes
/logs/verifier/evidence.json. Every scoring decision is left to
compute_reward.py.

  collect.py              grade /app/m4rs
  collect.py --manifest   print toolchain hashes (authoring aid)
"""
import concurrent.futures as cf
import glob
import gzip
import hashlib
import json
import os
import platform
import pwd
import resource
import shutil
import signal
import stat
import subprocess
import sys
import tempfile
import time
import traceback

sys.path.insert(0, "/tests")
import policy  # noqa: E402  (the grader's own copy)

OUT = "/logs/verifier"
CASES = "/tests/cases/hidden.jsonl.gz"
MANIFEST = "/tests/toolchain_manifest.json"
CRATE_SRC = "/app/m4rs/src"
RUST_VERSION = "1.83.0"
RUN_USER = "m4grade"
CASE_TIMEOUT = 10
WORKERS = max(2, min(8, os.cpu_count() or 2))
CARGO_TOML = """[package]
name = "m4rs"
version = "0.1.0"
edition = "2021"

[dependencies]

[profile.release]
opt-level = 3
debug = false
"""


def triple():
    m = platform.machine()
    return {"x86_64": "x86_64-unknown-linux-gnu", "aarch64": "aarch64-unknown-linux-gnu",
            "arm64": "aarch64-unknown-linux-gnu"}.get(m, m + "-unknown-linux-gnu")


def toolchain_dir():
    return "/usr/local/rustup/toolchains/%s-%s" % (RUST_VERSION, triple())


def toolchain_files():
    tc = toolchain_dir()
    files = [tc + "/bin/cargo", tc + "/bin/rustc"]
    files += sorted(glob.glob(tc + "/lib/librustc_driver-*.so"))
    files += sorted(glob.glob(tc + "/lib/rustlib/%s/lib/libstd-*.rlib" % triple()))
    files += sorted(glob.glob(tc + "/lib/rustlib/%s/lib/libcore-*.rlib" % triple()))
    return files


def sha256(path):
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def toolchain_hashes():
    out = {}
    for p in toolchain_files():
        out[p] = sha256(p) if os.path.isfile(p) else "missing"
    return out


def check_toolchain():
    """Returns (ok, reason). Unknown architectures are not penalised."""
    try:
        manifest = json.load(open(MANIFEST))
    except (OSError, ValueError):
        return True, "no manifest"
    want = manifest.get(triple())
    if not want:
        return True, "no manifest entry for " + triple()
    got = toolchain_hashes()
    bad = [p for p, h in want.items() if got.get(p) != h]
    if bad:
        return False, "toolchain files modified: " + ", ".join(bad[:5])
    for d in ("/", "/tmp"):
        for name in (".cargo/config", ".cargo/config.toml"):
            if os.path.exists(os.path.join(d, name)):
                return False, "cargo configuration planted at " + os.path.join(d, name)
    return True, "ok"


def build(work):
    """Copy src/**/*.rs into a fresh crate and build it offline."""
    crate = os.path.join(work, "crate")
    os.makedirs(os.path.join(crate, "src"))
    for root, _, files in os.walk(CRATE_SRC):
        for f in files:
            if f.endswith(".rs"):
                src = os.path.join(root, f)
                if os.path.islink(src):
                    continue
                rel = os.path.relpath(src, CRATE_SRC)
                dst = os.path.join(crate, "src", rel)
                os.makedirs(os.path.dirname(dst), exist_ok=True)
                shutil.copyfile(src, dst)
    with open(os.path.join(crate, "Cargo.toml"), "w") as f:
        f.write(CARGO_TOML)
    tc = toolchain_dir()
    env = {
        "PATH": tc + "/bin:/usr/bin:/bin",
        "HOME": work,
        "CARGO_HOME": os.path.join(work, "cargo-home"),
        "CARGO_TARGET_DIR": os.path.join(work, "target"),
        "RUSTC": tc + "/bin/rustc",
        "LC_ALL": "C",
    }
    cargo = tc + "/bin/cargo"
    if not os.path.isfile(cargo):
        return None, "cargo not found at " + cargo
    try:
        r = subprocess.run([cargo, "build", "--release", "--offline", "--quiet"],
                           cwd=crate, env=env, capture_output=True, text=True, timeout=1800)
    except subprocess.TimeoutExpired:
        return None, "build timed out"
    if r.returncode != 0:
        return None, (r.stdout + r.stderr)[-4000:]
    binp = os.path.join(work, "target", "release", "m4rs")
    if not os.path.isfile(binp):
        return None, "build produced no m4rs binary"
    return binp, "ok"


def ensure_user():
    try:
        return pwd.getpwnam(RUN_USER)
    except KeyError:
        subprocess.run(["useradd", "--system", "--no-create-home", "--shell", "/usr/sbin/nologin", RUN_USER],
                       capture_output=True)
        return pwd.getpwnam(RUN_USER)


def limits():
    resource.setrlimit(resource.RLIMIT_CPU, (CASE_TIMEOUT + 2, CASE_TIMEOUT + 2))
    resource.setrlimit(resource.RLIMIT_FSIZE, (8 << 20, 8 << 20))
    try:
        resource.setrlimit(resource.RLIMIT_AS, (4 << 30, 4 << 30))
    except (ValueError, OSError):
        pass


def run_case(binary, case, rundir, pw):
    d = tempfile.mkdtemp(prefix="c-", dir=rundir)
    for name, text in case["files"].items():
        p = os.path.normpath(os.path.join(d, name))
        os.makedirs(os.path.dirname(p), exist_ok=True)
        with open(p, "wb") as f:
            f.write(text.encode("latin-1"))
    for root, dirs, files in os.walk(d):
        for x in dirs + files:
            os.chown(os.path.join(root, x), pw.pw_uid, pw.pw_gid)
    os.chown(d, pw.pw_uid, pw.pw_gid)
    out_path = os.path.join(rundir, os.path.basename(d) + ".out")
    err_path = os.path.join(rundir, os.path.basename(d) + ".err")
    result = None
    with open(out_path, "wb") as fo, open(err_path, "wb") as fe:
        try:
            p = subprocess.Popen([binary] + case["args"], cwd=d, stdin=subprocess.PIPE, stdout=fo, stderr=fe,
                                 env={"PATH": "/usr/bin:/bin", "LC_ALL": "C", "HOME": d},
                                 user=pw.pw_uid, group=pw.pw_gid, extra_groups=[],
                                 start_new_session=True, preexec_fn=limits, close_fds=True)
        except OSError as e:
            return {"error": "spawn failed: %s" % e}
        try:
            p.communicate(case["stdin"].encode("latin-1"), timeout=CASE_TIMEOUT)
            result = p.returncode
        except subprocess.TimeoutExpired:
            result = None
        finally:
            try:
                os.killpg(p.pid, signal.SIGKILL)
            except (ProcessLookupError, PermissionError):
                pass
            p.wait()
    if result is None:
        got = {"timeout": True}
    else:
        with open(out_path, "rb") as f:
            so = f.read()
        with open(err_path, "rb") as f:
            se = f.read()
        got = {"stdout": so.decode("latin-1"), "stderr": se.decode("latin-1"), "exit": result}
    shutil.rmtree(d, ignore_errors=True)
    for x in (out_path, err_path):
        try:
            os.unlink(x)
        except OSError:
            pass
    return got


def passed(case, got):
    if "exit" not in got:
        return False
    exp = case["expected"]
    if got["exit"] < 0:
        return False
    return (got["stdout"] == exp["stdout"] and got["exit"] == exp["exit"]
            and (not case["check_stderr"] or got["stderr"] == exp["stderr"]))


def main():
    ev = {"runtime_ok": True, "runtime_error": "", "build_ok": False, "build_log": "",
          "policy_ok": False, "policy_violations": [], "toolchain_ok": True, "toolchain_reason": "",
          "cheat_detected": False, "gate_reason": "", "cases_total": 0, "cases_passed": 0,
          "per_category": {}, "failed_sample": [], "seconds": 0.0}
    t0 = time.time()
    locked = []
    try:
        try:
            with gzip.open(CASES, "rt") as f:
                cases = [json.loads(l) for l in f]
        except Exception as e:  # sealed data missing: harness problem
            ev["runtime_ok"] = False
            ev["runtime_error"] = "cannot load hidden cases: %s" % e
            return ev
        ev["cases_total"] = len(cases)
        for c in cases:
            ev["per_category"].setdefault(c["category"], [0, 0])[1] += 1

        ok, why = check_toolchain()
        ev["toolchain_ok"], ev["toolchain_reason"] = ok, why
        if not ok:
            ev["cheat_detected"] = True
            ev["gate_reason"] = why
            return ev

        probs = policy.check_tree(CRATE_SRC) if os.path.isdir(CRATE_SRC) else ["%s is missing" % CRATE_SRC]
        ev["policy_violations"] = probs[:50]
        ev["policy_ok"] = not probs
        if probs:
            ev["gate_reason"] = "source policy violated: " + probs[0]
            return ev

        work = tempfile.mkdtemp(prefix="m4grade-", dir="/tmp")
        os.chmod(work, 0o700)
        binp, log = build(work)
        ev["build_log"] = log[-4000:]
        if not binp:
            ev["gate_reason"] = "build failed"
            shutil.rmtree(work, ignore_errors=True)
            return ev
        ev["build_ok"] = True

        pw = ensure_user()
        rundir = tempfile.mkdtemp(prefix="m4run-", dir="/tmp")
        os.chmod(rundir, 0o711)
        binary = os.path.join(rundir, "m4rs")
        shutil.copyfile(binp, binary)
        os.chmod(binary, 0o755)
        shutil.rmtree(work, ignore_errors=True)

        # Keep everything sealed or agent-written out of the candidate's reach.
        for p in ("/tests", "/solution", "/logs", "/app", "/root"):
            if os.path.isdir(p):
                mode = stat.S_IMODE(os.stat(p).st_mode)
                locked.append((p, mode))
                os.chmod(p, 0o700)

        with cf.ThreadPoolExecutor(WORKERS) as ex:
            results = list(ex.map(lambda c: run_case(binary, c, rundir, pw), cases))
        npass = 0
        for c, got in zip(cases, results):
            if passed(c, got):
                npass += 1
                ev["per_category"][c["category"]][0] += 1
            elif len(ev["failed_sample"]) < 40:
                ev["failed_sample"].append(c["id"])
        ev["cases_passed"] = npass
        subprocess.run(["pkill", "-KILL", "-u", RUN_USER], capture_output=True)
        shutil.rmtree(rundir, ignore_errors=True)
        return ev
    except Exception:
        ev["runtime_ok"] = False
        ev["runtime_error"] = traceback.format_exc()[-2000:]
        return ev
    finally:
        for p, mode in locked:
            try:
                os.chmod(p, mode)
            except OSError:
                pass
        ev["seconds"] = round(time.time() - t0, 1)


if __name__ == "__main__":
    if len(sys.argv) > 1 and sys.argv[1] == "--manifest":
        print(json.dumps({triple(): toolchain_hashes()}, indent=1))
        sys.exit(0)
    os.makedirs(OUT, exist_ok=True)
    evidence = main()
    with open(os.path.join(OUT, "evidence.json"), "w") as f:
        json.dump(evidence, f, indent=1)
    print("collect: build_ok=%s policy_ok=%s passed=%d/%d (%ss)" % (
        evidence["build_ok"], evidence["policy_ok"], evidence["cases_passed"], evidence["cases_total"],
        evidence["seconds"]))
