#!/usr/bin/env python3
"""Local estimate of the score: build /app/codec the way grading does (a
fresh copy, plain `make`), round-trip a set of station files, and report the
compression ratio, package size, timings and an estimated score.

  python3 /app/tools/evaluate.py              # the dev stations (closest to the sealed set)
  python3 /app/tools/evaluate.py --set train  # the training stations
  python3 /app/tools/evaluate.py --files a.dly b.dly
"""
import argparse
import glob
import os
import shutil
import subprocess
import sys
import tempfile
import time

PACKAGE_LIMIT = 16 * 1024 * 1024
# Dev-set calibration: the shipped zlib codec and the reference codec.
DEV_BASELINE = 7.269723
DEV_REFERENCE = 15.891279


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--set", default="dev", choices=["dev", "train"])
    ap.add_argument("--files", nargs="*")
    a = ap.parse_args()
    work = tempfile.mkdtemp(prefix="codec-eval-")
    build = os.path.join(work, "build")
    shutil.copytree("/app/codec", build, ignore=shutil.ignore_patterns(".git"))
    r = subprocess.run(["make"], cwd=build, capture_output=True, text=True)
    if r.returncode != 0 or not os.path.isfile(os.path.join(build, "codec")):
        print(r.stdout[-3000:], r.stderr[-3000:])
        print("build: FAILED (score would be 0)")
        sys.exit(1)
    size = os.path.getsize(os.path.join(build, "codec"))
    model = os.path.join(build, "model")
    if os.path.isdir(model):
        for root, _, files in os.walk(model):
            size += sum(os.path.getsize(os.path.join(root, f)) for f in files)
    print("build: ok   package: %d bytes (%s)" % (size, "ok" if size <= PACKAGE_LIMIT else "OVER THE 16 MiB LIMIT"))
    codec = os.path.join(build, "codec")
    files = a.files or sorted(glob.glob("/app/data/%s/*.dly" % a.set))
    orig = comp = 0
    tc = td = 0.0
    bad = []
    for f in files:
        cd = tempfile.mkdtemp(dir=work)
        shutil.copyfile(f, os.path.join(cd, "in.dly"))
        t0 = time.time()
        rc1 = subprocess.run([codec, "compress", "in.dly", "out.bin"], cwd=cd).returncode
        tc += time.time() - t0
        dd = tempfile.mkdtemp(dir=work)
        if rc1 == 0 and os.path.exists(os.path.join(cd, "out.bin")):
            shutil.copyfile(os.path.join(cd, "out.bin"), os.path.join(dd, "in.bin"))
            t0 = time.time()
            rc2 = subprocess.run([codec, "decompress", "in.bin", "out.dly"], cwd=dd).returncode
            td += time.time() - t0
            same = rc2 == 0 and open(f, "rb").read() == open(os.path.join(dd, "out.dly"), "rb").read()
            comp += os.path.getsize(os.path.join(cd, "out.bin"))
        else:
            same = False
        orig += os.path.getsize(f)
        if not same:
            bad.append(os.path.basename(f))
    ratio = orig / comp if comp else 0.0
    print("files: %d   round-trip failures: %d %s" % (len(files), len(bad), " ".join(bad[:10])))
    print("original: %d bytes   compressed: %d bytes   ratio: %.4fx" % (orig, comp, ratio))
    print("time: compress %.1fs, decompress %.1fs" % (tc, td))
    if a.set == "dev" and not a.files:
        est = 0.0 if bad else min(1.0, max(0.0, (ratio - DEV_BASELINE) / (DEV_REFERENCE - DEV_BASELINE)))
        print("estimated score (dev calibration): %.4f" % est)
    shutil.rmtree(work, ignore_errors=True)


if __name__ == "__main__":
    main()
