"""Scan 1 (I1/I2/I3): noise-free, long runs (2 x T60, tail < -120 dB), physically consistent direct
share (Sabine D/R), receiver ball R=0.31 m, arrival phase inside the bin varied, first-reflection
gap varied. Rows whose post-direct level is below -8 dB (the direct alone takes >80% of the EDT
window; EDT is ill-conditioned under any definition) are reported separately."""
import sys, math, itertools, collections, json
sys.dont_write_bytecode = True
import numpy as np
import synth as S
from common import run_all, score

R = float(sys.argv[1]) if len(sys.argv) > 1 else 0.31
h = R / S.C
rows = []
seen = set()
for V, T60, d, gap_ms, dt_ms in itertools.product((200.0, 2000.0, 20000.0), (0.3, 0.6, 1.0, 2.0, 3.0),
                                                   (0.7, 1.0, 1.5, 2.0, 3.0, 5.0, 8.0, 12.0, 20.0, 30.0),
                                                   (0.0, 2.0, 5.0, 10.0), (1.0, 2.0, 5.0, 10.0)):
    Lroom = V ** (1 / 3)
    if d > 1.6 * Lroom:
        continue
    dt = dt_ms * 1e-3; gap = gap_ms * 1e-3
    k = 6 * math.log(10) / T60
    for phase in (0.05, 0.5, 0.95):
        t_arr = (math.floor((d / S.C) / dt) + phase) * dt
        dd = t_arr * S.C
        if dd < 0.6:
            continue
        key = (V, T60, round(dd, 4), gap_ms, dt_ms)
        if key in seen:
            continue
        seen.add(key)
        A = [1.0]; Srev = 1.0 / k
        Ed = S.sabine_direct_energy(V, T60, dd, Srev)
        truth = S.truth_edt(t_arr, Ed, gap, A, [k])
        b = S.histogram(dt, 2.0 * T60 + t_arr + gap, t_arr, h, Ed, gap, A, [k])
        res = run_all(b, dt, t_arr)
        lvl = 10 * math.log10(Srev / (Ed + Srev))
        rows.append(dict(V=V, T60=T60, d=round(dd, 3), gap_ms=gap_ms, dt_ms=dt_ms, phase=phase, truth=truth,
                         lvl_after_direct=lvl,
                         res={n: (r['edt'], r['edt_lo'], r['edt_hi'], r['status'], str(r['reason'])[:60]) for n, r in res.items()},
                         sc={n: score(r, truth) for n, r in res.items()}))
print('R =', R, 'rows', len(rows), ' ill-conditioned (lvl<-8 dB):', sum(1 for r in rows if r['lvl_after_direct'] < -8))
for n in ('minimal', 'leanband', 'practice'):
    for regime, sel in (('well-conditioned', lambda r: r['lvl_after_direct'] >= -8), ('ill-conditioned', lambda r: r['lvl_after_direct'] < -8)):
        rr = [r for r in rows if sel(r)]
        cnt = collections.Counter(r['sc'][n][0] for r in rr)
        print(f'{n} [{regime}] n={len(rr)}', dict(cnt))
        bad = sorted([r for r in rr if r['sc'][n][0] == 'ok_WRONG'], key=lambda r: -abs(r['sc'][n][1]))
        for r in bad[:6]:
            print('   err=%+.1f%%  V=%g T60=%g d=%g gap=%gms dt=%gms phase=%g lvl=%.1fdB truth=%.3f  -> %s' % (
                100 * r['sc'][n][1], r['V'], r['T60'], r['d'], r['gap_ms'], r['dt_ms'], r['phase'], r['lvl_after_direct'], r['truth'], r['res'][n]))
        if bad:
            print('   by dt:', dict(collections.Counter(r['dt_ms'] for r in bad)), ' by phase:', dict(collections.Counter(r['phase'] for r in bad)),
                  ' sign:', dict(collections.Counter('+' if r['sc'][n][1] > 0 else '-' for r in bad)))
json.dump(rows, open(f'scan_i12_R{R}.json', 'w'), default=str)
