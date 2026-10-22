#!/usr/bin/env python3
"""Every scoring decision for recrs, from /logs/verifier/evidence.json.

Hard gates (any failure scores exactly 0): toolchain integrity, source
rules, offline build. Otherwise reward = cases passed / hidden cases
(fixed denominator; timeouts and crashes count as failures).
"""
import json
import os
import sys

OUT = "/logs/verifier"


def emit(reward, valid, reason, subscores, extra=None):
    reward = round(min(1.0, max(0.0, reward)), 6)
    flat = {"reward": reward, "score": reward, "valid": int(valid)}
    for k, v in subscores.items():
        flat[k] = round(float(v), 6)
    for k, v in (extra or {}).items():
        flat[k] = v
    with open(os.path.join(OUT, "reward.json"), "w") as f:
        json.dump(flat, f)
    with open(os.path.join(OUT, "reward.txt"), "w") as f:
        f.write("%s\n" % reward)
    with open(os.path.join(OUT, "details.json"), "w") as f:
        json.dump({"subscores": [{"name": k, "score": round(float(v), 6)} for k, v in subscores.items()],
                   "additional_data": {"reason": reason}}, f, indent=1)
    print("reward=%s valid=%d: %s" % (reward, valid, reason))
    sys.exit(0 if reward > 0 else 1)


def main():
    os.makedirs(OUT, exist_ok=True)
    try:
        ev = json.load(open(os.path.join(OUT, "evidence.json")))
    except (OSError, ValueError) as e:
        emit(0.0, 0, "evidence missing or unreadable: %s" % e, {})
    if not ev.get("runtime_ok", False):
        emit(0.0, 0, "grading harness failure: " + ev.get("runtime_error", "")[-500:], {})

    total = int(ev.get("cases_total", 0))
    passed = int(ev.get("cases_passed", 0))
    frac = passed / total if total else 0.0
    nviol = len(ev.get("policy_violations", []))
    subs = {
        "toolchain_ok": 1.0 if ev.get("toolchain_ok", True) else 0.0,
        "policy_ok": 1.0 if ev.get("policy_ok") else 0.0,
        "policy_cleanliness": 1.0 / (1.0 + nviol),
        "build_ok": 1.0 if ev.get("build_ok") else 0.0,
        "pass_fraction": frac,
    }
    for cat, (p, t) in sorted(ev.get("per_category", {}).items()):
        subs["cat_" + cat] = p / t if t else 0.0
    counts = {"cases_passed": passed, "cases_total": total}

    if ev.get("cheat_detected"):
        emit(0.0, 1, "integrity gate failed: " + ev.get("gate_reason", ""), subs, counts)
    if not ev.get("policy_ok"):
        emit(0.0, 1, ev.get("gate_reason") or "source rules violated", subs, counts)
    if not ev.get("build_ok"):
        emit(0.0, 1, "build failed: " + ev.get("build_log", "")[-300:], subs, counts)
    if total == 0:
        emit(0.0, 0, "no hidden cases loaded", subs, counts)
    emit(frac, 1, "%d of %d hidden cases passed" % (passed, total), subs, counts)


main()
