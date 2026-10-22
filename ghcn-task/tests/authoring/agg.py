import json, os, subprocess, sys, time
codec, which = sys.argv[1], sys.argv[2]
split = json.load(open('split.json'))
files = [('train_coop', s) for s in split['train']] if which == 'train' else split[which]
o = c = 0; t = 0.0
for k, s in files:
    f = 'raw/%s/%s.dly' % (k, s)
    t0 = time.time()
    subprocess.run([codec, 'compress', f, '/tmp/x.bin'], check=True)
    subprocess.run([codec, 'decompress', '/tmp/x.bin', '/tmp/x.out'], check=True)
    t += time.time() - t0
    assert open(f, 'rb').read() == open('/tmp/x.out', 'rb').read(), f
    o += os.path.getsize(f); c += os.path.getsize('/tmp/x.bin')
print('%s %s: orig=%d comp=%d ratio=%.6f secs=%.1f' % (codec, which, o, c, o / c, t))
