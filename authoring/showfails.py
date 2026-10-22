import json, sys
n = int(sys.argv[2]) if len(sys.argv) > 2 else 10
for l in list(open(sys.argv[1]))[:n]:
    x = json.loads(l); c = x['case']; g = x['got']
    print('==', c['id'], c['category'], c['args'])
    exp = c['expected']
    if g is None:
        print(' TIMEOUT'); print(c['files'].get('input.m4', '')[:300]); continue
    for k in ('stdout', 'stderr', 'exit'):
        if g[k] != exp[k]:
            el = exp[k].split('\n') if isinstance(exp[k], str) else [exp[k]]
            gl = g[k].split('\n') if isinstance(g[k], str) else [g[k]]
            for i in range(max(len(el), len(gl))):
                a = el[i] if i < len(el) else None; b = gl[i] if i < len(gl) else None
                if a != b:
                    src = c['files'].get('input.m4', '').split('\n')
                    print('  %s line %d\n    EXP %r\n    GOT %r' % (k, i, a, b))
                    break
