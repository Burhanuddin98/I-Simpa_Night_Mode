"""Tables for the composite: real data, synthetic, image-source. Usage: python report.py -> report.txt"""
import collections, math, os, pickle, sys
HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import harness as H  # noqa: E402

NAMES = ('ship', 'w1d', 'W1G', 'W2G', 'W1G-G3', 'W1G-G3b', 'W1G-G12')
out = []
P = lambda s='': (print(s), out.append(s))


def table(rows, step_of, truth_of, title, groups=None, extra_truth=None):
    P(f'== {title}')
    steps = sorted({step_of(r) for r in rows})
    for q in ('edt', 'ts'):
        P(f'  {q.upper()}: variant | per step: false/accepted/N (refused %) worst err/L' + (' ; false vs second truth' if extra_truth else ''))
        for name in NAMES:
            cells = []
            for st in steps:
                sel = [r for r in rows if step_of(r) == st and truth_of(r, q) is not None]
                acc = [r for r in sel if r['res'][name][q][0]]
                errs = [H.err(q, r['res'][name][q][1], truth_of(r, q)) for r in acc]
                bad = sum(1 for e in errs if e is not None and e > 1)
                worst = max([e for e in errs if e is not None], default=0.0)
                s = f'{st:g} ms {bad}/{len(acc)}/{len(sel)} ({100 * (1 - len(acc) / max(len(sel), 1)):.1f} %) {worst:.2f}'
                if extra_truth:
                    sel2 = [r for r in acc if extra_truth(r, q) is not None]
                    bad2 = sum(1 for r in sel2 if H.err(q, r['res'][name][q][1], extra_truth(r, q)) > 1)
                    s += f' ; {bad2}'
                cells.append(s)
            P(f'    {name:8s} | ' + ' | '.join(cells))
    if groups:
        for gname, gfun in groups:
            P(f'  -- W1G by {gname} (EDT acc/N, Ts acc/N, false EDT/Ts) per step')
            keys = sorted({gfun(r) for r in rows})
            for key in keys:
                cells = []
                for st in steps:
                    sel = [r for r in rows if step_of(r) == st and gfun(r) == key]
                    se = [r for r in sel if truth_of(r, 'edt') is not None]
                    ss = [r for r in sel if truth_of(r, 'ts') is not None]
                    ae = [r for r in se if r['res']['W1G']['edt'][0]]
                    as_ = [r for r in ss if r['res']['W1G']['ts'][0]]
                    fe = sum(1 for r in ae if H.err('edt', r['res']['W1G']['edt'][1], truth_of(r, 'edt')) > 1)
                    fs = sum(1 for r in as_ if H.err('ts', r['res']['W1G']['ts'][1], truth_of(r, 'ts')) > 1)
                    cells.append(f'{st:g}: {len(ae)}/{len(se)} {len(as_)}/{len(ss)} F{fe}/{fs}')
                P(f'     {str(key):26s} ' + ' | '.join(cells))


def reasons(rows, step_of, st, title):
    c = collections.Counter()
    for r in rows:
        if step_of(r) != st:
            continue
        for q in ('edt', 'ts'):
            ok, val, why = r['res']['W1G'][q]
            c[(q, 'accepted' if ok else why)] += 1
    P(f'  W1G at {st:g} ms, {title}: ' + ', '.join(f'{q} {w} {n}' for (q, w), n in sorted(c.items())))


def worst_list(rows, step_of, truth_of, label_of, name, n=12):
    lst = []
    for r in rows:
        for q in ('edt', 'ts'):
            ok, val, why = r['res'][name][q]
            tr = truth_of(r, q)
            if ok and tr is not None:
                e = H.err(q, val, tr)
                lst.append((e, q, step_of(r), label_of(r), val, tr))
    lst.sort(key=lambda x: -x[0])
    P(f'  worst accepted by {name}:')
    for e, q, st, lab, val, tr in lst[:n]:
        P(f'     {e:5.2f}x L {q} {st:g} ms {lab}: {val:.6g} vs {tr:.6g}')


# ---------------- real
real = pickle.load(open(os.path.join(HERE, 'real_rows.pkl'), 'rb'))
tr_real = lambda r, q: r['truth'][q]
table(real, lambda r: r['ms'], tr_real, 'REAL: 16 fine-step datasets (26,664 series-steps), truth the fine shipped midpoint',
      groups=[('dataset', lambda r: r['ds'])])
for st in (1, 2, 3):
    reasons(real, lambda r: r['ms'], st, 'real')
worst_list(real, lambda r: r['ms'], tr_real, lambda r: f"{r['ds']} run {r['run']} {r['rec']} {r['F']} Hz", 'W1G')
lv = [r['res']['feat'].get('lvl_direct') for r in real if r['res']['feat']]
P(f'  real: level after the direct sound (W1G reading): min {min(lv):.2f} dB, rows below -3 dB {sum(1 for x in lv if x < -3)} of {len(lv)}')
P()

# ---------------- synthetic
if os.path.exists(os.path.join(HERE, 'synth_rows.pkl')):
    syn = [r for r in pickle.load(open(os.path.join(HERE, 'synth_rows.pkl'), 'rb')) if 'error' not in r]
    tr_syn = lambda r, q: r['truth'][q]
    table(syn, lambda r: r['ms'], tr_syn, f'SYNTHETIC (bracket skeptic rooms, {len(syn)} series-steps), truth shipped midpoint at 0.02 ms',
          groups=[('room/band', lambda r: f"{r['room']} {r['band']}"), ('R', lambda r: r['R'])])
    for st in (1, 2):
        reasons(syn, lambda r: r['ms'], st, 'synthetic')
    worst_list(syn, lambda r: r['ms'], tr_syn, lambda r: f"{r['room']} {r['band']} R {r['R']} {r['kind']} d {r['d']:.2f}", 'W1G')
    worst_list(syn, lambda r: r['ms'], tr_syn, lambda r: f"{r['room']} {r['band']} R {r['R']} {r['kind']} d {r['d']:.2f}", 'w1d', 8)
    P()

# ---------------- POS200 noise-free
if os.path.exists(os.path.join(HERE, 'pos200_rows.pkl')):
    pz = [r for r in pickle.load(open(os.path.join(HERE, 'pos200_rows.pkl'), 'rb')) if 'error' not in r]
    tr_p = lambda r, q: r['truth'][q]
    table(pz, lambda r: r['ms'], tr_p, f"POS200 POSITIONS, NOISE-FREE (tutorial 1's room, specular model, {len(pz)} series-steps), truth shipped midpoint at 0.02 ms",
          groups=[('band', lambda r: r['band'])])
    for st in (1, 2):
        reasons(pz, lambda r: r['ms'], st, 'POS200 noise-free')
    worst_list(pz, lambda r: r['ms'], tr_p, lambda r: f"{r['band']} Hz rec {r['rec']} d {r['d']:.2f}", 'W1G', 8)
    P()

# ---------------- image source
if os.path.exists(os.path.join(HERE, 'ism_rows.pkl')):
    im = [r for r in pickle.load(open(os.path.join(HERE, 'ism_rows.pkl'), 'rb')) if 'error' not in r]
    def tr_i(r, q):
        x = r['truth']['te_i' if q == 'edt' else 'ts_i']
        return x if (x is not None and x == x) else None
    def tr_f(r, q):
        x = r['truth']['te_f' if q == 'edt' else 'ts_f']
        return x if (x is not None and x == x) else None
    table(im, lambda r: r['step'], tr_i, f'IMAGE SOURCE (GF3 skeptic rooms, runs with per-step air, {len(im)} series-steps), truth ideal; after ";" false vs the fine reading',
          groups=[('set', lambda r: r['set']), ('band', lambda r: r['F'])], extra_truth=tr_f)
    for st in (1.0, 1.5, 2.0):
        reasons(im, lambda r: r['step'], st, 'image source')
    worst_list(im, lambda r: r['step'], tr_i, lambda r: f"{r['set']} {r['F']} Hz rec {r['rec']} d {r['d']:.2f}", 'W1G')
    worst_list(im, lambda r: r['step'], tr_i, lambda r: f"{r['set']} {r['F']} Hz rec {r['rec']} d {r['d']:.2f}", 'W1G-G3', 8)
    worst_list(im, lambda r: r['step'], tr_i, lambda r: f"{r['set']} {r['F']} Hz rec {r['rec']} d {r['d']:.2f}", 'W1G-G3b', 8)
    worst_list(im, lambda r: r['step'], tr_i, lambda r: f"{r['set']} {r['F']} Hz rec {r['rec']} d {r['d']:.2f}", 'W1G-G12', 8)

open(os.path.join(HERE, 'report.txt'), 'w').write('\n'.join(out) + '\n')
