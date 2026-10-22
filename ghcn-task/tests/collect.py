#!/usr/bin/env python3
"""Evidence collection for the GHCN-Daily codec.

Builds /app/codec as an unprivileged user, compresses every sealed station
file as a second user, removes everything that user left behind, then
decompresses as a third user that only receives the compressed bytes, and
checks the round trip. Writes /logs/verifier/evidence.json; every scoring
decision is made by compute_reward.py.
"""
import concurrent.futures as cf
import json
import os
import pwd
import shutil
import stat
import subprocess
import tempfile
import time
import traceback

OUT = "/logs/verifier"
HIDDEN = "/tests/hidden"
SRC = "/app/codec"
USERS = {"build": "codecbuild", "pack": "codecpack", "unpack": "codecunpack"}
PACKAGE_LIMIT = 16 * 1024 * 1024
FILE_TIMEOUT = 120
PHASE_LIMIT = 1200
BUILD_TIMEOUT = 900
WORKERS = 4
SCRATCH_ROOTS = ["/", "/tmp", "/var/tmp", "/dev/shm", "/run", "/app", "/home"]

NS_PREFIX = []
WRITABLE_DIRS = []


def ensure_user(name):
    try:
        return pwd.getpwnam(name)
    except KeyError:
        subprocess.run(["useradd", "--system", "--no-create-home", "--shell", "/usr/sbin/nologin", name],
                       capture_output=True)
        return pwd.getpwnam(name)


def detect_namespaces():
    """Use fresh network and IPC namespaces when the sandbox allows them."""
    global NS_PREFIX
    try:
        r = subprocess.run(["unshare", "--net", "--ipc", "true"], capture_output=True, timeout=20)
        NS_PREFIX = ["unshare", "--net", "--ipc", "--"] if r.returncode == 0 else []
    except (OSError, subprocess.TimeoutExpired):
        NS_PREFIX = []
    return bool(NS_PREFIX)


def drop(pw, cmd):
    """Run CMD as PW with no_new_privs, so setuid bits are ignored."""
    return NS_PREFIX + ["setpriv", "--reuid=%d" % pw.pw_uid, "--regid=%d" % pw.pw_gid, "--clear-groups",
                        "--no-new-privs", "--"] + cmd


def limits_prefix(cpu, as_bytes, fsize):
    return ["sh", "-c", 'ulimit -t %d; ulimit -v %d; ulimit -f %d; exec "$@"' % (cpu, as_bytes // 1024, fsize // 512),
            "sh"]


def run_limited(pw, argv, cwd, timeout, cpu, as_bytes=4 << 30, fsize=1 << 30):
    env = {"PATH": "/usr/local/bin:/usr/bin:/bin", "HOME": cwd, "LC_ALL": "C", "TMPDIR": cwd}
    cmd = drop(pw, limits_prefix(cpu, as_bytes, fsize) + argv)
    logf = tempfile.TemporaryFile()
    t0 = time.time()
    p = subprocess.Popen(cmd, cwd=cwd, env=env, stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL,
                         stderr=logf, start_new_session=True)
    try:
        rc = p.wait(timeout=timeout)
    except subprocess.TimeoutExpired:
        rc = None
    secs = time.time() - t0
    try:
        os.killpg(p.pid, 9)  # also removes anything it left running in its session
    except (ProcessLookupError, PermissionError):
        pass
    if rc is None:
        p.wait()
    logf.seek(0)
    err = logf.read()[-2000:].decode("latin-1")
    logf.close()
    return rc, secs, err


def purge_grading_users():
    """Delete anything already owned by the grading accounts (it could only
    have been planted), in one scan."""
    for root in SCRATCH_ROOTS:
        if os.path.isdir(root):
            cmd = ["find", root, "-xdev", "("]
            for i, u in enumerate(USERS.values()):
                cmd += (["-o"] if i else []) + ["-user", u]
            cmd += [")", "-depth", "-delete"]
            subprocess.run(cmd, capture_output=True)


def scan_writable():
    """Find world-writable directories (the only places the grading users
    can create files) and revoke other-write on pre-existing files, FIFOs
    and sockets, which could otherwise carry data between the users.
    Returns the modes to restore."""
    global WRITABLE_DIRS
    dirs = set()
    changed = []
    for root in SCRATCH_ROOTS:
        if not os.path.isdir(root):
            continue
        r = subprocess.run(["find", root, "-xdev", "-perm", "-0002", "(", "-type", "d", "-o", "-type", "f", "-o",
                            "-type", "p", "-o", "-type", "s", ")", "-printf", "%y %p\\0"], capture_output=True)
        for item in r.stdout.split(b"\0"):
            if not item:
                continue
            kind, path = item[:1], item[2:].decode("utf-8", "surrogateescape")
            if kind == b"d":
                dirs.add(path)
                continue
            try:
                st = os.lstat(path)
                if stat.S_ISLNK(st.st_mode):
                    continue
                os.chmod(path, stat.S_IMODE(st.st_mode) & ~0o002)
                changed.append((path, stat.S_IMODE(st.st_mode)))
            except OSError:
                pass
    WRITABLE_DIRS = sorted(dirs)
    return changed


def wipe_user(name):
    subprocess.run(["pkill", "-KILL", "-u", name], capture_output=True)
    for d in WRITABLE_DIRS:
        if os.path.isdir(d):
            subprocess.run(["find", d, "-xdev", "-user", name, "-depth", "-delete"], capture_output=True)


def tree_size(path):
    total = 0
    for root, _, files in os.walk(path):
        for f in files:
            p = os.path.join(root, f)
            if not os.path.islink(p):
                total += os.path.getsize(p)
    return total


def main(ev):
    names = sorted(f for f in os.listdir(HIDDEN) if f.endswith(".dly"))
    if not names:
        raise RuntimeError("no sealed station files")
    ev["files_total"] = len(names)
    pws = {k: ensure_user(v) for k, v in USERS.items()}
    for u in USERS.values():
        subprocess.run(["pkill", "-KILL", "-u", u], capture_output=True)
    purge_grading_users()
    ev["namespaces"] = detect_namespaces()
    ev["_restore"] = scan_writable()

    for p in ("/tests", "/solution", "/logs", "/root"):
        if os.path.isdir(p):
            ev["_locked"].append((p, stat.S_IMODE(os.stat(p).st_mode)))
            os.chmod(p, 0o700)

    work = tempfile.mkdtemp(prefix="codecgrade-", dir="/tmp")
    ev["_work"] = work
    os.chmod(work, 0o711)

    # ---- build, as the build user
    build = os.path.join(work, "build")
    if not os.path.isdir(SRC):
        ev["gate_reason"] = "/app/codec is missing"
        return
    shutil.copytree(SRC, build, symlinks=False, ignore=shutil.ignore_patterns(".git"))
    b = pws["build"]
    for root, dirs, files in os.walk(build):
        for x in dirs + files:
            os.lchown(os.path.join(root, x), b.pw_uid, b.pw_gid)
    os.chown(build, b.pw_uid, b.pw_gid)
    ev["_locked"].append(("/app", stat.S_IMODE(os.stat("/app").st_mode)))
    os.chmod("/app", 0o700)
    rc, _, log = run_limited(b, ["make"], build, BUILD_TIMEOUT, BUILD_TIMEOUT, as_bytes=8 << 30)
    ev["build_log"] = log
    exe = os.path.join(build, "codec")
    if rc != 0 or not os.path.isfile(exe) or os.path.islink(exe):
        ev["gate_reason"] = "build failed" if rc is not None else "build timed out"
        return
    ev["build_ok"] = True

    pkg = os.path.join(work, "pkg")
    os.makedirs(pkg)
    shutil.copyfile(exe, os.path.join(pkg, "codec"))
    if os.path.isdir(os.path.join(build, "model")) and not os.path.islink(os.path.join(build, "model")):
        shutil.copytree(os.path.join(build, "model"), os.path.join(pkg, "model"), symlinks=False)
    for root, dirs, files in os.walk(pkg):
        os.chmod(root, 0o755)
        for f in files:
            os.chmod(os.path.join(root, f), 0o644)
    os.chmod(os.path.join(pkg, "codec"), 0o755)
    shutil.rmtree(build, ignore_errors=True)
    wipe_user(USERS["build"])
    ev["package_bytes"] = tree_size(pkg)
    ev["package_ok"] = ev["package_bytes"] <= PACKAGE_LIMIT
    if not ev["package_ok"]:
        ev["gate_reason"] = "package is %d bytes (limit %d)" % (ev["package_bytes"], PACKAGE_LIMIT)
        return
    codec = os.path.join(pkg, "codec")

    # ---- compress, as the packing user
    cdir = os.path.join(work, "c")
    sdir = os.path.join(work, "s")
    os.makedirs(cdir)
    os.makedirs(sdir)
    os.chmod(cdir, 0o711)
    os.chmod(sdir, 0o700)
    packer = pws["pack"]

    def compress(name):
        d = os.path.join(cdir, name)
        os.makedirs(d)
        src = os.path.join(d, "in.dly")
        shutil.copyfile(os.path.join(HIDDEN, name), src)
        os.chown(src, packer.pw_uid, packer.pw_gid)
        os.chmod(src, 0o400)
        os.chown(d, packer.pw_uid, packer.pw_gid)
        os.chmod(d, 0o700)
        rc, secs, err = run_limited(packer, [codec, "compress", "in.dly", "out.bin"], d, FILE_TIMEOUT, FILE_TIMEOUT)
        outp = os.path.join(d, "out.bin")
        if rc != 0 or not os.path.isfile(outp) or os.path.islink(outp):
            return name, secs, None, "compress failed (rc=%s) %s" % (rc, err[-200:])
        shutil.copyfile(outp, os.path.join(sdir, name + ".bin"))
        return name, secs, os.path.getsize(os.path.join(sdir, name + ".bin")), ""

    results = {}
    with cf.ThreadPoolExecutor(WORKERS) as ex:
        for name, secs, size, err in ex.map(compress, names):
            results[name] = {"size": size, "err": err}
            ev["compress_secs"] += secs
    shutil.rmtree(cdir, ignore_errors=True)
    wipe_user(USERS["pack"])

    # ---- decompress, as the unpacking user
    ddir = os.path.join(work, "d")
    os.makedirs(ddir)
    os.chmod(ddir, 0o711)
    unpacker = pws["unpack"]

    def decompress(name):
        r = results[name]
        if r["size"] is None:
            return name, 0.0, False, r["err"]
        d = os.path.join(ddir, name)
        os.makedirs(d)
        src = os.path.join(d, "in.bin")
        shutil.copyfile(os.path.join(sdir, name + ".bin"), src)
        os.chown(src, unpacker.pw_uid, unpacker.pw_gid)
        os.chmod(src, 0o400)
        os.chown(d, unpacker.pw_uid, unpacker.pw_gid)
        os.chmod(d, 0o700)
        rc, secs, err = run_limited(unpacker, [codec, "decompress", "in.bin", "out.dly"], d, FILE_TIMEOUT, FILE_TIMEOUT)
        outp = os.path.join(d, "out.dly")
        ok = False
        if rc == 0 and os.path.isfile(outp) and not os.path.islink(outp):
            with open(outp, "rb") as a, open(os.path.join(HIDDEN, name), "rb") as b2:
                ok = a.read() == b2.read()
        shutil.rmtree(d, ignore_errors=True)
        why = "" if ok else ("output differs" if rc == 0 else "decompress failed (rc=%s) %s" % (rc, err[-200:]))
        return name, secs, ok, why

    with cf.ThreadPoolExecutor(WORKERS) as ex:
        for name, secs, ok, err in ex.map(decompress, names):
            ev["decompress_secs"] += secs
            if ok:
                ev["files_ok"] += 1
            elif len(ev["failures"]) < 20:
                ev["failures"].append("%s: %s" % (name, err))
    wipe_user(USERS["unpack"])

    for name in names:
        ev["orig_bytes"] += os.path.getsize(os.path.join(HIDDEN, name))
        if results[name]["size"] is not None:
            ev["comp_bytes"] += results[name]["size"]
    ev["time_ok"] = ev["compress_secs"] <= PHASE_LIMIT and ev["decompress_secs"] <= PHASE_LIMIT
    if ev["files_ok"] != ev["files_total"]:
        ev["gate_reason"] = "%d of %d files did not round-trip" % (ev["files_total"] - ev["files_ok"], ev["files_total"])
    elif not ev["time_ok"]:
        ev["gate_reason"] = "time limit exceeded (compress %.0fs, decompress %.0fs)" % (
            ev["compress_secs"], ev["decompress_secs"])


if __name__ == "__main__":
    os.makedirs(OUT, exist_ok=True)
    t0 = time.time()
    ev = {"runtime_ok": True, "runtime_error": "", "build_ok": False, "build_log": "", "package_bytes": 0,
          "package_ok": False, "files_total": 0, "files_ok": 0, "orig_bytes": 0, "comp_bytes": 0,
          "compress_secs": 0.0, "decompress_secs": 0.0, "time_ok": False, "failures": [], "gate_reason": "",
          "namespaces": False, "_locked": [], "_restore": [], "_work": None}
    try:
        main(ev)
    except Exception:
        ev["runtime_ok"] = False
        ev["runtime_error"] = traceback.format_exc()[-2000:]
    finally:
        for u in USERS.values():
            subprocess.run(["pkill", "-KILL", "-u", u], capture_output=True)
        if ev["_work"]:
            shutil.rmtree(ev["_work"], ignore_errors=True)
        for p, mode in ev["_restore"]:
            try:
                os.chmod(p, mode)
            except OSError:
                pass
        for p, mode in reversed(ev["_locked"]):
            try:
                os.chmod(p, mode)
            except OSError:
                pass
    ev["world_writable_revoked"] = len(ev.pop("_restore"))
    ev.pop("_locked")
    ev.pop("_work")
    ev["grader_secs"] = round(time.time() - t0, 1)
    ev["compress_secs"] = round(ev["compress_secs"], 2)
    ev["decompress_secs"] = round(ev["decompress_secs"], 2)
    with open(os.path.join(OUT, "evidence.json"), "w") as f:
        json.dump(ev, f, indent=1)
    print("collect: build_ok=%s files_ok=%d/%d orig=%d comp=%d gate=%r" % (
        ev["build_ok"], ev["files_ok"], ev["files_total"], ev["orig_bytes"], ev["comp_bytes"], ev["gate_reason"]))
