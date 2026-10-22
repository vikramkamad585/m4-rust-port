#!/usr/bin/env python3
"""Deterministically sample GHCN-Daily US stations and download their .dly files."""
import os, random, subprocess, sys, json
rows = []
for line in open('ghcnd-stations.txt'):
    sid, state = line[0:11], line[38:40].strip()
    if sid.startswith('US'):
        rows.append((sid, state))
rnd = random.Random(20261022)
remote = ('AK', 'HI', 'PR', 'VI', 'GU', 'AS', 'MP', 'UM', 'FM', 'MH', 'PW')
pools = {
    'train_coop': [s for s, st in rows if s.startswith('USC') and st and st not in remote],
    'hid_airport': [s for s, st in rows if s.startswith('USW') and st not in remote],
    'hid_cocorahs': [s for s, st in rows if s.startswith('US1') and st not in remote],
    'hid_remote': [s for s, st in rows if (s.startswith('USC') or s.startswith('USW')) and st in remote],
}
want = {'train_coop': 70, 'hid_airport': 22, 'hid_cocorahs': 30, 'hid_remote': 18}
dev = {'hid_airport': 3, 'hid_cocorahs': 4, 'hid_remote': 3}
plan = {}
for k, pool in pools.items():
    rnd.shuffle(pool)
    plan[k] = pool
manifest = {}
for k in pools:
    os.makedirs('raw/' + k, exist_ok=True)
    got = []
    need = want[k] + dev.get(k, 0)
    for sid in plan[k]:
        if len(got) >= need:
            break
        dst = 'raw/%s/%s.dly' % (k, sid)
        if not os.path.exists(dst):
            r = subprocess.run(['curl', '-m', '120', '-fsSL', '-o', dst,
                                'https://www.ncei.noaa.gov/pub/data/ghcn/daily/all/%s.dly' % sid])
            if r.returncode != 0:
                if os.path.exists(dst): os.remove(dst)
                continue
        sz = os.path.getsize(dst)
        if sz < 2000 or sz > 6_000_000:   # skip near-empty and very large records
            os.remove(dst)
            continue
        got.append(sid)
    manifest[k] = got
    print(k, len(got), sum(os.path.getsize('raw/%s/%s.dly' % (k, s)) for s in got))
json.dump(manifest, open('manifest.json', 'w'), indent=1)
