"""Scan 4 (I1): occluded direct path. Night Mode's arrival_s is geometric (distance/c, results/spps.rs
arrival_from) whether or not the path is blocked. No direct sound; first reflected energy arrives
`delay` after the geometric arrival. Truth: ISO EDT with 0 dB at the first arrival (pre-onset silence
excluded, as ISO 3382-1 / every surveyed tool does). Also: a weak (diffracted) direct at -10/-20 dB."""
import sys, math, itertools, collections
sys.dont_write_bytecode = True
import numpy as np
import synth as S
from common import run_all, score

rows = []
for T60, delay_ms, dt_ms, dir_db in itertools.product((0.4, 0.8, 1.5, 3.0), (2.0, 5.0, 10.0, 20.0, 40.0),
                                                      (1.0, 2.0, 10.0), (None, -20.0, -10.0)):
    dt = dt_ms * 1e-3; k = 6 * math.log(10) / T60
    t_arr = 0.030 + 0.41 * dt
    gap = delay_ms * 1e-3
    A = [1.0]; Srev = 1.0 / k
    Ed = 0.0 if dir_db is None else Srev * 10 ** (dir_db / 10)
    if Ed == 0.0:
        truth = S.truth_edt(t_arr + gap, 0.0, 0.0, A, [k])          # 0 dB at first arrival
    else:
        truth = S.truth_edt(t_arr, Ed, gap, A, [k])                  # direct exists: Definition A as usual
    b = S.histogram(dt, t_arr + gap + 2.0 * T60, t_arr, 0.31 / S.C, Ed, gap, A, [k])
    res = run_all(b, dt, t_arr)
    rows.append(dict(T60=T60, delay=delay_ms, dt=dt_ms, dir_db=dir_db, truth=truth,
                     res={n: (r['edt'], r['status'], str(r['reason'])[:50]) for n, r in res.items()},
                     sc={n: score(r, truth) for n, r in res.items()}))
for n in ('minimal', 'leanband', 'practice'):
    cnt = collections.Counter(r['sc'][n][0] for r in rows)
    print(n, dict(cnt))
    bad = sorted([r for r in rows if r['sc'][n][0] == 'ok_WRONG'], key=lambda r: -abs(r['sc'][n][1]))
    for r in bad[:6]:
        print('   err=%+.1f%% T60=%g delay=%gms dt=%gms direct=%s truth=%.3f -> %s' % (100 * r['sc'][n][1], r['T60'], r['delay'], r['dt'], r['dir_db'], r['truth'], r['res'][n]))
    if bad:
        print('   by delay:', dict(sorted(collections.Counter(r['delay'] for r in bad).items())), ' by direct:', dict(collections.Counter(str(r['dir_db']) for r in bad)))
