"""final/method.py on the evaluator's four sets (same loaders, same rows, same truths as ../eval/EVAL.md),
then the EVAL.md metrics and benefit/regression vs upstream, joined by row id with the evaluator's own
upstream/leanband results (scratchpad results_*.pkl, read-only). Single process, no solver.
Usage: python run_eval_final.py t1|z3|real|ism|report"""
import sys, math, pickle, time, statistics as st, collections
sys.dont_write_bytecode = True
SP = 'C:/Users/Burhan/AppData/Local/Temp/claude/b--repos-I-Simpa-Night-Mode/97f2c13c-beea-4d4b-b842-603feb756e57/scratchpad/edtsimp'
HERE = 'B:/repos/I-Simpa_Night_Mode/target/agents/edt-simplify/final'
JND = 5.0
which = sys.argv[1]

if which != 'report':
    sys.path.insert(0, SP)
    import wrappers  # noqa: F401  (import paths the ISM/real loaders need, as run_eval.py does)
    import loaders as L
    sys.modules.pop('common', None)
    sys.path.insert(0, HERE)
    import method as F
    rows = dict(t1=L.load_t1, z3=L.load_z3, real=L.load_real,
                ism=lambda: (pickle.load(open(HERE + '/dev/ism_rows.pkl', 'rb')) if __import__('os').path.exists(HERE + '/dev/ism_rows.pkl')
                             else L.load_ism(n_tasks=160, steps_ms=(1.0, 2.0))))[which]()
    out = []
    t0 = time.time()
    for r in rows:
        if r.get('bins') is None:
            out.append(dict(id=r['id'], truth_edt=r.get('truth_edt'), status='refused', reason='loader', edt=None, edt_lo=None, edt_hi=None))
            continue
        res = F.analyse(r['bins'], r['dt'], r['t_arrival'], dict(r['meta']))
        out.append(dict(id=r['id'], truth_edt=r['truth_edt'], dt=r['dt'], **res))
    pickle.dump(out, open(f'{HERE}/res_{which}.pkl', 'wb'))
    print(which, len(out), 'rows', '%.0f s' % (time.time() - t0))
    sys.exit(0)


def metrics(recs):
    n = len(recs)
    use = [r for r in recs if r['status'] in ('ok', 'wide')]
    errs, ws, out, hws = [], 0, 0, []
    for r in use:
        t = r.get('truth_edt')
        if r['edt_lo'] is not None and math.isfinite(r['edt_lo']) and math.isfinite(r['edt_hi']):
            hws.append(100 * (r['edt_hi'] - r['edt_lo']) / (2 * r['edt']))
            if t is not None and math.isfinite(t) and not (r['edt_lo'] - 1e-9 <= t <= r['edt_hi'] + 1e-9):
                out += 1
        if t is None or not math.isfinite(t) or t == 0:
            continue
        e = 100 * abs(r['edt'] - t) / abs(t)
        errs.append(e)
        ws += (r['status'] == 'ok' and e > JND)
    ok = sum(r['status'] == 'ok' for r in recs)
    return dict(n=n, usable=100 * len(use) / n, ok=100 * ok / n,
                med=st.median(errs) if errs else float('nan'),
                p95=sorted(errs)[max(0, int(round(0.95 * (len(errs) - 1))))] if errs else float('nan'),
                ws=ws, out=out, hw=st.median(hws) if hws else float('nan'))


def good(r):
    t = r.get('truth_edt')
    return (r is not None and r['status'] == 'ok' and r['edt'] is not None and t is not None
            and math.isfinite(t) and 100 * abs(r['edt'] - t) / abs(t) <= JND)


lines = ['set   method       n  usable%   ok%  med|err|%  p95|err|%  wrong-s  out-rng  med hw%',
         '-' * 86]
br = ['', 'set   method     n(truth)  benefit  regression  both-good  neither']
for s in ('t1', 'z3', 'real', 'ism'):
    fin = pickle.load(open(f'{HERE}/res_{s}.pkl', 'rb'))
    ev = pickle.load(open(f'{SP}/results_{s}.pkl', 'rb'))['results']
    by = collections.defaultdict(dict)
    for r in ev:
        if r.get('method'):
            by[r['id']][r['method']] = r
    for r in fin:
        by[r['id']]['final'] = r
    for m in ('final', 'leanband', 'upstream'):
        recs = [v[m] for v in by.values() if m in v]
        x = metrics(recs)
        lines.append('%-5s %-9s %5d %7.2f %6.2f %9.2f %10.2f %8d %8d %8.2f' % (
            s, m, x['n'], x['usable'], x['ok'], x['med'], x['p95'], x['ws'], x['out'], x['hw']))
    for m in ('final', 'leanband'):
        c = collections.Counter()
        for v in by.values():
            up, cd = v.get('upstream'), v.get(m)
            if up is None or cd is None:
                continue
            t = up.get('truth_edt')
            if t is None or not math.isfinite(t):
                continue
            c['n'] += 1
            ug, cg = good(up), good(cd)
            c['benefit' if cg and not ug else 'regression' if ug and not cg else 'both' if ug else 'neither'] += 1
        br.append('%-5s %-9s %8d %8d %11d %10d %8d' % (s, m, c['n'], c['benefit'], c['regression'], c['both'], c['neither']))
    reasons = collections.Counter(r['reason'] for r in fin if r['status'] == 'refused')
    br.append('      final refusals: %s' % dict(reasons))
txt = '\n'.join(lines + br)
print(txt)
open(f'{HERE}/eval_final.txt', 'w').write(txt + '\n')
