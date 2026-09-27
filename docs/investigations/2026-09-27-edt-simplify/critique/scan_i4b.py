"""Scan 2b (I4 isolated): for each case, run each candidate on (a) the truncated run and (b) the same
response recorded to -150 dB. An I4 wrong-silent = status ok on the truncated run AND |err vs truth| > 5%
AND the long run's own value is within 5% of truth (so truncation, not another mechanism, did it)."""
import sys, math, itertools, collections, json
sys.dont_write_bytecode = True
import numpy as np
import synth as S
from common import run_all, score

rows = []
for T60, dt_ms, frac, shape, drr_db in itertools.product(
        (0.5, 1.0, 2.0, 4.0, 8.0), (1.0, 2.0, 10.0),
        (0.15, 0.2, 0.25, 0.3, 0.35, 0.4, 0.5, 0.6, 0.8, 1.0, 1.5),
        ('single', 'double-20', 'double-15', 'double-10'), (-12.0, -6.0, -3.0)):
    dt = dt_ms * 1e-3
    k1 = 6 * math.log(10) / T60
    if shape == 'single':
        A, k = [1.0], [k1]
    else:
        lvl = float(shape.split('-')[1]); k2 = k1 / 3.0
        E1 = 1.0 / k1; E2 = E1 * 10 ** (-lvl / 10)
        A, k = [1.0, E2 * k2], [k1, k2]
    t_arr = 0.02 + 0.37 * dt
    Srev = sum(a / kk for a, kk in zip(A, k)); Ed = Srev * 10 ** (drr_db / 10)
    truth = S.truth_edt(t_arr, Ed, 0.0, A, k)
    T_long = t_arr + 150.0 / 60.0 * T60 * (3.0 if shape != 'single' else 1.0)
    b_long = S.histogram(dt, T_long, t_arr, 0.0, Ed, 0.0, A, k)
    b = S.histogram(dt, t_arr + frac * T60, t_arr, 0.0, Ed, 0.0, A, k)
    if len(b) < 4:
        continue
    tail_share = sum(a / kk * math.exp(-kk * (len(b) * dt - t_arr)) for a, kk in zip(A, k)) / (Ed + Srev)
    rs, rl = run_all(b, dt, t_arr), run_all(b_long, dt, t_arr)
    rows.append(dict(T60=T60, dt_ms=dt_ms, frac=frac, shape=shape, drr_db=drr_db, truth=truth,
                     tail_db=10 * math.log10(max(tail_share, 1e-300)),
                     res={n: (r['edt'], r['edt_lo'], r['edt_hi'], r['status'], str(r['reason'])[:60]) for n, r in rs.items()},
                     long={n: (r['edt'], r['status']) for n, r in rl.items()},
                     sc={n: score(r, truth) for n, r in rs.items()}))
print('rows', len(rows))
for n in ('minimal', 'leanband', 'practice'):
    long_ok = [r for r in rows if r['long'][n][0] is not None and abs(r['long'][n][0] / r['truth'] - 1) <= 0.05]
    cnt = collections.Counter(r['sc'][n][0] for r in long_ok)
    bad = sorted([r for r in long_ok if r['sc'][n][0] == 'ok_WRONG'], key=lambda r: -abs(r['sc'][n][1]))
    print(f'{n}: rows whose long run is within 5% of truth: {len(long_ok)} -> truncated-run outcomes {dict(cnt)}')
    for r in bad[:6]:
        print('   err=%+.1f%% T60=%g dt=%gms run=%.2f*T60 shape=%s DRR=%gdB unrecorded=%.1fdB truth=%.3f long=%.3f -> %s' % (
            100 * r['sc'][n][1], r['T60'], r['dt_ms'], r['frac'], r['shape'], r['drr_db'], r['tail_db'], r['truth'], r['long'][n][0], r['res'][n]))
    if bad:
        print('   by shape:', dict(collections.Counter(r['shape'] for r in bad)), ' by run frac:', dict(sorted(collections.Counter(r['frac'] for r in bad).items())),
              ' unrecorded tail dB range: [%.1f, %.1f]' % (min(r['tail_db'] for r in bad), max(r['tail_db'] for r in bad)))
        sb = [r for r in bad if r['shape'] == 'single']
        for r in sb[:4]:
            print('   single: err=%+.1f%% T60=%g dt=%gms run=%.2f*T60 DRR=%gdB unrecorded=%.1fdB -> %s' % (
                100 * r['sc'][n][1], r['T60'], r['dt_ms'], r['frac'], r['drr_db'], r['tail_db'], r['res'][n]))
json.dump(rows, open('scan_i4b.json', 'w'), default=str)
