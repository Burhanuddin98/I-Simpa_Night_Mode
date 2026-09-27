"""Scan 2 (I4): runs that end early. Truth = the infinite-length response's ISO EDT (Definition A).
Single-slope and double-slope (coupled-volume) decays; far-ish receiver so I2 is not the confound
(point receiver, weak direct), plus a near-receiver variant. Noise-free."""
import sys, math, itertools, collections, json
sys.dont_write_bytecode = True
import numpy as np
import synth as S
from common import run_all, score

rows = []
for T60, dt_ms, frac, shape, drr_db in itertools.product(
        (0.5, 1.0, 2.0, 4.0, 8.0), (1.0, 2.0, 10.0),
        (0.12, 0.15, 0.18, 0.2, 0.22, 0.25, 0.3, 0.35, 0.4, 0.5, 0.6, 0.8, 1.0, 1.5),
        ('single', 'double-15', 'double-10', 'double-5'), (-12.0, -3.0, 0.0)):
    dt = dt_ms * 1e-3
    k1 = 6 * math.log(10) / T60
    if shape == 'single':
        A, k = [1.0], [k1]
    else:
        lvl = float(shape.split('-')[1])            # late part's share of the reverberant energy, dB
        k2 = k1 / 4.0                                # late decay 4x slower (coupled volume / flutter)
        E1 = 1.0 / k1; E2 = E1 * 10 ** (-lvl / 10)
        A, k = [1.0, E2 * k2], [k1, k2]
    t_arr = 0.02 + 0.37 * dt                         # 6.9 m-ish, off-grid
    gap = 0.0
    Srev = sum(a / kk for a, kk in zip(A, k))
    Ed = Srev * 10 ** (drr_db / 10)
    truth = S.truth_edt(t_arr, Ed, gap, A, k)
    T_run = t_arr + frac * T60
    b = S.histogram(dt, T_run, t_arr, 0.0, Ed, gap, A, k)
    if len(b) < 4:
        continue
    # true share of energy after the run end (the unrecorded tail), relative to the arrival-onward total
    tail_share = sum(a / kk * math.exp(-kk * (len(b) * dt - t_arr)) for a, kk in zip(A, k)) / (Ed + Srev)
    res = run_all(b, dt, t_arr)
    rows.append(dict(T60=T60, dt_ms=dt_ms, frac=frac, shape=shape, drr_db=drr_db, truth=truth,
                     tail_db=10 * math.log10(max(tail_share, 1e-300)),
                     res={n: (r['edt'], r['edt_lo'], r['edt_hi'], r['status'], str(r['reason'])[:60]) for n, r in res.items()},
                     sc={n: score(r, truth) for n, r in res.items()}))
print('rows', len(rows))
for n in ('minimal', 'leanband', 'practice'):
    cnt = collections.Counter(r['sc'][n][0] for r in rows)
    print(n, dict(cnt))
    bad = sorted([r for r in rows if r['sc'][n][0] == 'ok_WRONG'], key=lambda r: -abs(r['sc'][n][1]))
    for r in bad[:8]:
        print('   err=%+.1f%% T60=%g dt=%gms run=%.2f*T60 shape=%s DRR=%gdB unrecorded=%.1fdB truth=%.3f -> %s' % (
            100 * r['sc'][n][1], r['T60'], r['dt_ms'], r['frac'], r['shape'], r['drr_db'], r['tail_db'], r['truth'], r['res'][n]))
    if bad:
        print('   by shape:', dict(collections.Counter(r['shape'] for r in bad)), ' by run frac:', dict(sorted(collections.Counter(r['frac'] for r in bad).items())),
              ' by unrecorded tail dB: max', round(max(r['tail_db'] for r in bad), 1), 'min', round(min(r['tail_db'] for r in bad), 1))
json.dump(rows, open('scan_i4.json', 'w'), default=str)
