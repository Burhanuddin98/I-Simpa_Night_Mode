"""Calibrate final/method.py's TAIL_SHARE on scan 2b's construction: gate off, record the EDT error
of the truncated run against the long run's own value (so only truncation is measured), binned by
the tail estimate U/S(t10). Single process."""
import sys, math, itertools, collections
sys.dont_write_bytecode = True
sys.path.insert(0, 'B:/repos/I-Simpa_Night_Mode/target/agents/edt-simplify/critique')
sys.path.insert(0, 'B:/repos/I-Simpa_Night_Mode/target/agents/edt-simplify/final')
import numpy as np, synth as S, method as F
F.TAIL_SHARE = math.inf
rows = []
for T60, dt_ms, frac, shape, drr_db in itertools.product((0.5, 1.0, 2.0, 4.0, 8.0), (1.0, 2.0, 10.0),
        (0.15, 0.2, 0.25, 0.3, 0.35, 0.4, 0.5, 0.6, 0.8, 1.0, 1.5), ('single', 'double-20', 'double-15', 'double-10'), (-12.0, -6.0, -3.0)):
    dt = dt_ms * 1e-3; k1 = 6 * math.log(10) / T60
    if shape == 'single': A, k = [1.0], [k1]
    else:
        lvl = float(shape.split('-')[1]); k2 = k1 / 3.0; E1 = 1.0 / k1; E2 = E1 * 10 ** (-lvl / 10); A, k = [1.0, E2 * k2], [k1, k2]
    t_arr = 0.02 + 0.37 * dt; Srev = sum(a / kk for a, kk in zip(A, k)); Ed = Srev * 10 ** (drr_db / 10)
    T_long = t_arr + 150.0 / 60.0 * T60 * (3.0 if shape != 'single' else 1.0)
    bl = S.histogram(dt, T_long, t_arr, 0.0, Ed, 0.0, A, k); b = S.histogram(dt, t_arr + frac * T60, t_arr, 0.0, Ed, 0.0, A, k)
    rl = F.analyse(bl, dt, t_arr, {'half_width': 0.0}); r = F.analyse(b, dt, t_arr, {'half_width': 0.0})
    if rl['edt'] is None or r['edt'] is None: continue
    tail = float(r['reason'].split('tail=')[1].split(';')[0])
    rows.append((tail, r['edt'] / rl['edt'] - 1, shape, frac))
edges = [0, 1e-3, 3e-3, 0.01, 0.02, 0.03, 0.05, 0.1, 0.2, 0.5, 1, math.inf]
print('U/S10 bucket | n | max |err| vs own long run | p95 | median  (single / double separately)')
for lo, hi in zip(edges[:-1], edges[1:]):
    for sh in ('single', 'double'):
        e = [abs(x[1]) for x in rows if lo <= x[0] < hi and x[2].startswith(sh)]
        if e: print('  [%g, %g) %-6s n=%4d max %.2f%% p95 %.2f%% med %.2f%%' % (lo, hi, sh, len(e), 100 * max(e), 100 * np.percentile(e, 95), 100 * np.median(e)))
