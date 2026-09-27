"""Variant check: instead of refusing at U/S10 > 2 %, widen the long side of the range by the factor
(1 + U/S10) and refuse only above U/S10 = 0.5. Scan 2b construction, truth = infinite-run ISO EDT."""
import sys, math, itertools, collections
sys.dont_write_bytecode = True
sys.path.insert(0, 'B:/repos/I-Simpa_Night_Mode/target/agents/edt-simplify/critique')
sys.path.insert(0, 'B:/repos/I-Simpa_Night_Mode/target/agents/edt-simplify/final')
import numpy as np, synth as S, method as F
from common import score
F.TAIL_SHARE = math.inf
tal = collections.Counter(); byfrac = collections.defaultdict(collections.Counter)
for T60, dt_ms, frac, shape, drr_db in itertools.product((0.5, 1.0, 2.0, 4.0, 8.0), (1.0, 2.0, 10.0),
        (0.15, 0.2, 0.25, 0.3, 0.35, 0.4, 0.5, 0.6, 0.8, 1.0, 1.5), ('single', 'double-20', 'double-15', 'double-10'), (-12.0, -6.0, -3.0)):
    dt = dt_ms * 1e-3; k1 = 6 * math.log(10) / T60
    if shape == 'single': A, k = [1.0], [k1]
    else:
        lvl = float(shape.split('-')[1]); k2 = k1 / 3.0; E1 = 1.0 / k1; E2 = E1 * 10 ** (-lvl / 10); A, k = [1.0, E2 * k2], [k1, k2]
    t_arr = 0.02 + 0.37 * dt; Srev = sum(a / kk for a, kk in zip(A, k)); Ed = Srev * 10 ** (drr_db / 10)
    truth = S.truth_edt(t_arr, Ed, 0.0, A, k)
    b = S.histogram(dt, t_arr + frac * T60, t_arr, 0.0, Ed, 0.0, A, k)
    r = F.analyse(b, dt, t_arr, {'half_width': 0.0})
    if r['edt'] is not None:
        u = float(r['reason'].split('tail=')[1].split(';')[0])
        if not u <= 0.5:
            r = dict(edt=None, status='refused')
        else:
            r['edt_hi'] *= (1 + u)
            r['status'] = 'ok' if (r['edt_hi'] - r['edt_lo']) / 2 / r['edt'] <= 0.05 else 'wide'
    kind, e = score(r, truth)
    tal[kind] += 1; byfrac[frac][kind] += 1
print(dict(tal))
for f in sorted(byfrac): print('  run=%.2f*T60' % f, dict(byfrac[f]))
