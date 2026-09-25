"""Totals over every set: accepted, false accepts, worst error, per variant (EDT and Ts)."""
import os, pickle, sys
HERE = os.path.dirname(os.path.abspath(__file__)); sys.path.insert(0, HERE)
import harness as H
def load(f): return [r for r in pickle.load(open(os.path.join(HERE, f), 'rb')) if 'error' not in r]
def ti(r, q):
    x = r['truth']['te_i' if q == 'edt' else 'ts_i']; return x if (x is not None and x == x) else None
sets = [('real', load('real_rows.pkl'), lambda r, q: r['truth'][q]),
        ('synthetic', load('synth_rows.pkl'), lambda r, q: r['truth'][q]),
        ('pos200 noise-free', load('pos200_rows.pkl'), lambda r, q: r['truth'][q]),
        ('image source', load('ism_rows.pkl'), ti)]
out = []
for name in ('ship', 'w1d', 'W1G', 'W2G', 'W1G-G3', 'W1G-G3b', 'W1G-G12'):
    line = []
    for q in ('edt', 'ts'):
        n = acc = bad = 0; worst = 0.0
        for sname, rows, tr in sets:
            for r in rows:
                t = tr(r, q)
                if t is None: continue
                n += 1
                ok, val, why = r['res'][name][q]
                if ok:
                    acc += 1; e = H.err(q, val, t); worst = max(worst, e); bad += e > 1
        line.append(f'{q.upper()} false {bad} / accepted {acc} / N {n}, worst {worst:.2f}x L')
    out.append(f'{name:8s}: ' + ' | '.join(line))
out.append('series-steps: ' + ', '.join(f'{s} {len(r)}' for s, r, _ in sets) + f', total {sum(len(r) for _, r, _ in sets)}')
print('\n'.join(out)); open(os.path.join(HERE, 'totals.txt'), 'w').write('\n'.join(out) + '\n')
