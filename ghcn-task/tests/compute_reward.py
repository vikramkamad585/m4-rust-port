#!/usr/bin/env python3
"""Every scoring decision for the GHCN-Daily codec.

Gates (any failure scores exactly 0): the build, the 16 MiB package limit,
an exact round trip of every sealed file, and the time limits.
Metric: compression ratio = original bytes / compressed bytes over the
sealed set (higher is better). The reward is the ratio normalised between
the shipped zlib codec and the reference codec, clamped to [0, 1].
"""
import json
import os
import sys

OUT = "/logs/verifier"
ORIG = 48816810
BASELINE = ORIG / 5393404      # shipped zlib-9 codec on the sealed set: 9.0512x
REFERENCE = ORIG / 2327319     # reference context-mixing codec: 20.9756x


def emit(reward, valid, reason, subs, gate_passed, metric):
    reward = round(min(1.0, max(0.0, reward)), 6)
    flat = {"reward": reward, "score": reward, "valid": int(valid)}
    for k, v in subs.items():
        flat[k] = round(float(v), 6)
    flat.update({"gate_passed": int(gate_passed), "baseline_metric": round(BASELINE, 6),
                 "oracle_metric": round(REFERENCE, 6)})
    if metric is not None:
        flat["metric"] = round(metric, 6)
    with open(os.path.join(OUT, "reward.json"), "w") as f:
        json.dump(flat, f)
    with open(os.path.join(OUT, "reward.txt"), "w") as f:
        f.write("%s\n" % reward)
    with open(os.path.join(OUT, "details.json"), "w") as f:
        json.dump({"subscores": [{"name": k, "score": round(float(v), 6)} for k, v in subs.items()],
                   "additional_data": {"reason": reason, "metric_direction": "higher",
                                       "metric_transform": "none"}}, f, indent=1)
    print("reward=%s valid=%d: %s" % (reward, valid, reason))
    sys.exit(0 if reward > 0 else 1)


def main():
    os.makedirs(OUT, exist_ok=True)
    try:
        ev = json.load(open(os.path.join(OUT, "evidence.json")))
    except (OSError, ValueError) as e:
        emit(0.0, 0, "evidence missing: %s" % e, {}, False, None)
    if not ev.get("runtime_ok"):
        emit(0.0, 0, "grading harness failure: " + ev.get("runtime_error", "")[-400:], {}, False, None)
    total = max(1, int(ev.get("files_total", 0)))
    comp = int(ev.get("comp_bytes", 0))
    orig = int(ev.get("orig_bytes", 0))
    ratio = orig / comp if comp > 0 else 0.0
    subs = {
        "build_ok": 1.0 if ev.get("build_ok") else 0.0,
        "package_ok": 1.0 if ev.get("package_ok") else 0.0,
        "roundtrip_fraction": int(ev.get("files_ok", 0)) / total,
        "time_ok": 1.0 if ev.get("time_ok") else 0.0,
        "compression_ratio": ratio,
        "package_bytes": float(ev.get("package_bytes", 0)),
        "compress_secs": float(ev.get("compress_secs", 0)),
        "decompress_secs": float(ev.get("decompress_secs", 0)),
    }
    if ev.get("gate_reason"):
        emit(0.0, 1, ev["gate_reason"], subs, False, ratio if comp > 0 else None)
    u = (ratio - BASELINE) / (REFERENCE - BASELINE)
    emit(u, 1, "compression ratio %.4fx (zlib baseline %.4fx, reference %.4fx)" % (ratio, BASELINE, REFERENCE),
         subs, True, ratio)


main()
