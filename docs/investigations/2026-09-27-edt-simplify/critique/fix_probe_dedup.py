"""Like-for-like on the deduplicated scan-1 grid (same rows as scan_i12.py): LeanBand as submitted vs
fix_probe.probe_edt, outcome counts per time step. Single process."""
import sys, math, itertools, collections, warnings
sys.dont_write_bytecode = True
warnings.filterwarnings('ignore')
import synth as S
from common import CANDS, score
from fix_probe import probe_edt

h = 0.31 / S.C
seen = set()
tal = {m: collections.defaultdict(collections.Counter) for m in ('leanband', 'probe')}
for V, T60, d, gap_ms, dt_ms in itertools.product((200.0, 2000.0, 20000.0), (0.3, 0.6, 1.0, 2.0, 3.0),
                                                   (0.7, 1.0, 1.5, 2.0, 3.0, 5.0, 8.0, 12.0, 20.0, 30.0),
                                                   (0.0, 2.0, 5.0, 10.0), (1.0, 2.0, 5.0, 10.0)):
    if d > 1.6 * V ** (1 / 3):
        continue
    dt = dt_ms * 1e-3
    k = 6 * math.log(10) / T60
    for phase in (0.05, 0.5, 0.95):
        t_arr = (math.floor((d / S.C) / dt) + phase) * dt
        if t_arr * S.C < 0.6:
            continue
        key = (V, T60, round(t_arr * S.C, 4), gap_ms, dt_ms)
        if key in seen:
            continue
        seen.add(key)
        Ed = S.sabine_direct_energy(V, T60, t_arr * S.C, 1.0 / k)
        if 10 * math.log10((1 / k) / (Ed + 1 / k)) < -8:
            continue
        truth = S.truth_edt(t_arr, Ed, gap_ms * 1e-3, [1.0], [k])
        b = S.histogram(dt, 2.0 * T60 + t_arr + gap_ms * 1e-3, t_arr, h, Ed, gap_ms * 1e-3, [1.0], [k])
        tal['leanband'][dt_ms][score(CANDS['leanband'].analyse(b, dt, t_arr, {}), truth)[0]] += 1
        tal['probe'][dt_ms][score(probe_edt(b, dt, t_arr), truth)[0]] += 1
for m, per in tal.items():
    tot = collections.Counter()
    for dt_ms in sorted(per):
        tot.update(per[dt_ms])
        print(m, dt_ms, 'ms', dict(per[dt_ms]))
    print(m, 'ALL', dict(tot))
