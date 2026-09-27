"""Ablation of final/method.py on the evaluator's four sets: what each component costs or buys.
Caches the ISM rows to ism_rows.pkl (loading them costs ~8 min). Single process, no solver.
Upstream's 'good' flags are joined by id from the evaluator's results_*.pkl (read-only)."""
import sys, os, math, pickle, collections
sys.dont_write_bytecode = True
SP = 'C:/Users/Burhan/AppData/Local/Temp/claude/b--repos-I-Simpa-Night-Mode/97f2c13c-beea-4d4b-b842-603feb756e57/scratchpad/edtsimp'
HERE = 'B:/repos/I-Simpa_Night_Mode/target/agents/edt-simplify/final'
sys.path.insert(0, SP)
import wrappers  # noqa: F401
import loaders as L
sys.modules.pop('common', None)
sys.path.insert(0, HERE)
import method as F

cache = HERE + '/dev/ism_rows.pkl'
sets = dict(t1=L.load_t1(), z3=L.load_z3(), real=L.load_real())
if os.path.exists(cache):
    sets['ism'] = pickle.load(open(cache, 'rb'))
else:
    sets['ism'] = [r for r in L.load_ism(n_tasks=160, steps_ms=(1.0, 2.0))]
    pickle.dump(sets['ism'], open(cache, 'wb'))

BASE = dict(TAIL_SHARE=0.02, MIN_POINTS=8, HW_FLOOR=0.0, NOISE_TERM=True, Z=2.0)
VARIANTS = {
    'base (method.py before HW_FLOOR was set to 0.5 %)': {},
    'floor 0.5 %': dict(HW_FLOOR=0.005),
    'floor 1 %': dict(HW_FLOOR=0.01),
    'no tail gate': dict(TAIL_SHARE=math.inf),
    'MIN_POINTS 4': dict(MIN_POINTS=4),
    'no noise term': dict(NOISE_TERM=False),
    'Z = 3': dict(Z=3.0),
}
up_good = {}
for s in sets:
    for r in pickle.load(open(f'{SP}/results_{s}.pkl', 'rb'))['results']:
        if r.get('method') == 'upstream':
            t = r.get('truth_edt')
            up_good[(s, r['id'])] = (r['status'] == 'ok' and t is not None and math.isfinite(t)
                                     and abs(r['edt'] / t - 1) <= 0.05)
for name, over in VARIANTS.items():
    for k, v in {**BASE, **over}.items():
        setattr(F, k, v)
    print('==', name)
    for s, rows in sets.items():
        c = collections.Counter()
        for r in rows:
            if r.get('bins') is None:
                continue
            t = r['truth_edt']
            x = F.analyse(r['bins'], r['dt'], r['t_arrival'], dict(r['meta']))
            if x['status'] == 'refused':
                kind = 'refused'
            elif t is None or not math.isfinite(t):
                kind = 'no_truth'
            else:
                e = abs(x['edt'] / t - 1)
                inside = x['edt_lo'] <= t <= x['edt_hi']
                kind = ('ok_WRONG' if e > 0.05 else 'ok_good') if x['status'] == 'ok' else ('wide_in' if inside else 'wide_out')
                if x['status'] == 'ok' and not inside:
                    c['ok_but_out'] += 1
            c[kind] += 1
            g = kind == 'ok_good'
            ug = up_good.get((s, r['id']))
            if ug is not None and t is not None and math.isfinite(t):
                c['benefit'] += g and not ug
                c['regress'] += ug and not g
        print('  %-5s %s' % (s, dict(sorted(c.items()))))
    sys.stdout.flush()
for k, v in BASE.items():
    setattr(F, k, v)
