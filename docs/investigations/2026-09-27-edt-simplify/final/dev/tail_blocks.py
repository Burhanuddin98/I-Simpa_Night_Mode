"""Block length for the late-envelope tail estimate: 10 % blocks (noise-fragile on V-R6), 20 % blocks,
and 10 % with a retry at 20 % when the 10 % pair does not decay. Scored on (a) scan 2b (noise-free
truncation, truth = infinite run) and (b) the real seeds' refusal count. Single process."""
import sys, math, itertools, collections, json, glob
sys.dont_write_bytecode = True
sys.path.insert(0, 'B:/repos/I-Simpa_Night_Mode/target/agents/edt-simplify/critique')
sys.path.insert(0, 'B:/repos/I-Simpa_Night_Mode/target/agents/edt-simplify/final')
import numpy as np, synth as S, method as F
from common import score

def U_of(b, mode):
    n = len(b)
    def pair(m):
        E1, E2 = float(b[n - 2 * m:n - m].sum()), float(b[n - m:].sum())
        return 0.0 if E2 == 0 else (E2 * E2 / (E1 - E2) if E1 > E2 else math.inf)
    if mode == '10': return pair(max(2, n // 10))
    if mode == '20': return pair(max(2, n // 5))
    u = pair(max(2, n // 10))
    return pair(max(2, n // 5)) if math.isinf(u) else u

F.TAIL_SHARE = math.inf
def run(b, dt, t_arr, meta, mode):
    r = F.analyse(b, dt, t_arr, meta)
    if r['edt'] is None: return r
    hh = meta.get('half_width', 0.31 / 343.2); bb = np.clip(np.asarray(b, float), 0, None); Ss = np.cumsum(bb[::-1])[::-1]
    k0 = max(int(math.floor((t_arr - hh) / dt)), 0)
    if bb[k0:].sum() > 0:
        k_on = k0 + int(np.nonzero(bb[k0:] > 0)[0][0])
        if k_on * dt > t_arr + hh + dt: k0 = k_on
    L = 10 * np.log10(np.maximum(Ss[k0:], 1e-300) / Ss[k0])
    S10 = Ss[k0 + int(np.nonzero(L <= -10)[0][0])]
    if U_of(np.clip(np.asarray(b, float), 0, None), mode) > 0.02 * S10:
        return dict(edt=None, status='refused', reason='run_too_short')
    return r

MODES = ('10', '20', 'retry')
tal = {m: collections.Counter() for m in MODES}
for T60, dt_ms, frac, shape, drr_db in itertools.product((0.5, 1.0, 2.0, 4.0, 8.0), (1.0, 2.0, 10.0),
        (0.15, 0.2, 0.25, 0.3, 0.35, 0.4, 0.5, 0.6, 0.8, 1.0, 1.5), ('single', 'double-20', 'double-15', 'double-10'), (-12.0, -6.0, -3.0)):
    dt = dt_ms * 1e-3; k1 = 6 * math.log(10) / T60
    if shape == 'single': A, k = [1.0], [k1]
    else:
        lvl = float(shape.split('-')[1]); k2 = k1 / 3.0; E1 = 1.0 / k1; E2 = E1 * 10 ** (-lvl / 10); A, k = [1.0, E2 * k2], [k1, k2]
    t_arr = 0.02 + 0.37 * dt; Srev = sum(a / kk for a, kk in zip(A, k)); Ed = Srev * 10 ** (drr_db / 10)
    truth = S.truth_edt(t_arr, Ed, 0.0, A, k)
    b = S.histogram(dt, t_arr + frac * T60, t_arr, 0.0, Ed, 0.0, A, k)
    for m in MODES: tal[m][score(run(b, dt, t_arr, {'half_width': 0.0}, m), truth)[0]] += 1
print('scan 2b:', {m: dict(v) for m, v in tal.items()})

AG = 'B:/repos/I-Simpa_Night_Mode/target/agents'
ROOTS = (AG + '/pm8-noise-scratch/runs/noise-cal-1790307822', AG + '/pm8-noise-scratch/runs/noise-cal-1790310131')
ref = {m: collections.Counter() for m in MODES}
for root in ROOTS:
    for c in json.load(open(root + '/cells.json')):
        cc = c if 'id' in c else c['cell']
        if abs(cc['time_step_s'] - 0.001) > 1e-9: continue
        for p in sorted(glob.glob(f"{root}/{cc['id']}/seed*/report.json")):
            d = json.load(open(p))['spps']; hh = d['receiver_radius_m'] / d['speed_of_sound_m_s']
            for pr in d['point_receivers']:
                for bd in pr['bands']:
                    v = np.asarray(bd['energy_pa2'], float); nb = len(v) // 2; v = v[:nb * 2].reshape(nb, 2).sum(1)
                    for m in MODES:
                        r = run(v, 0.002, pr['arrival_s'], {'half_width': hh}, m)
                        ref[m][(cc['id'], r['status'] if r['status'] == 'refused' else 'shown')] += 1
for m in MODES:
    print('real seeds', m, 'refused by cell:', {k[0]: v for k, v in sorted(ref[m].items()) if k[1] == 'refused'},
          ' total shown', sum(v for k, v in ref[m].items() if k[1] == 'shown'))
