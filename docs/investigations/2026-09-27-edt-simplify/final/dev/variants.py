"""Design-time comparison of range constructions for final/method.py (kept for the record).
P = critique probe (OLS SE only); A = P + counting noise (1st diff) in quadrature;
D1/D2 = window-edge refits (sel, sel[:-1], sel[1:]) + counting noise (1st / 2nd diff);
E2 = D2 + OLS SE in quadrature."""
import sys, math, itertools, collections
sys.dont_write_bytecode = True
sys.path.insert(0, 'B:/repos/I-Simpa_Night_Mode/target/agents/edt-simplify/critique')
import numpy as np, synth as S
from common import score
h = 0.31 / S.C

def variant(b, dt, t_arr, mode, hh=h):
    b = np.clip(np.asarray(b, float), 0, None); n = len(b)
    S_ = np.cumsum(b[::-1])[::-1]
    k0 = max(int(math.floor((t_arr - hh) / dt)), 0); t_start = t_arr + hh
    if b[k0:].sum() <= 0: return dict(edt=None, status='refused')
    k_on = k0 + int(np.nonzero(b[k0:] > 0)[0][0])
    if k_on * dt > t_start + dt: k0, t_start = k_on, (k_on + 1) * dt
    L = 10 * np.log10(np.maximum(S_[k0:], 1e-300) / S_[k0]); T = np.arange(k0, n) * dt
    below = np.nonzero(L <= -10)[0]
    if len(below) == 0: return dict(edt=None, status='refused')
    i10 = int(below[0]); sel = np.nonzero((T >= t_start - 1e-12) & (np.arange(len(L)) <= i10))[0]
    if len(sel) < 4: return dict(edt=None, status='refused')
    def fit(ss):
        t, y = T[ss], L[ss]; tb = t.mean(); sxx = ((t - tb) ** 2).sum(); a = ((t - tb) * (y - y.mean())).sum() / sxx
        res = y - (a * (t - tb) + y.mean()); se = math.sqrt((res ** 2).sum() / max(len(t) - 2, 1) / sxx); return a, se
    a, se = fit(sel)
    if a >= 0: return dict(edt=None, status='refused')
    j0 = k0 + i10; dec = b[j0:j0 + max(2 * (i10 + 1), 10)]
    if mode in ('D2', 'E2'):
        r = dec[1:-1] - 0.5 * (dec[:-2] + dec[2:]); w = (r ** 2).sum() / (1.5 * dec[1:-1].sum()) if dec[1:-1].sum() > 0 else 0
    else:
        w = ((dec[:-1] - dec[1:]) ** 2).sum() / (2 * dec.sum()) if dec.sum() > 0 else 0
    sd = 0.4343 * math.sqrt(w / S_[j0])
    edt = -60 / a; se_rel = se / (-a)
    if mode == 'P':
        lo = -60 / (a - 2 * se); hi = -60 / (a + 2 * se) if a + 2 * se < 0 else 10 * edt
    elif mode == 'A':
        q = 2 * math.sqrt(se_rel ** 2 + sd ** 2); lo, hi = edt * (1 - q), edt * (1 + q)
    else:
        es = [edt]
        for ss in (sel[:-1], sel[1:]):
            aa, _ = fit(ss)
            es.append(-60 / aa if aa < 0 else 10 * edt)
        q = 2 * (math.sqrt(sd ** 2 + se_rel ** 2) if mode == 'E2' else sd)
        lo, hi = min(es) * (1 - q), max(es) * (1 + q)
    hw = (hi - lo) / 2 / edt
    return dict(edt=edt, edt_lo=lo, edt_hi=hi, status='ok' if hw <= 0.05 else 'wide', sd=sd)

MODES = ('P', 'A', 'D1', 'D2', 'E2')
if __name__ == '__main__':
    tal = {m: collections.defaultdict(collections.Counter) for m in MODES}; seen = set()
    for V, T60, d, gap_ms, dt_ms in itertools.product((200.0, 2000.0, 20000.0), (0.3, 0.6, 1.0, 2.0, 3.0), (0.7, 1.0, 1.5, 2.0, 3.0, 5.0, 8.0, 12.0, 20.0, 30.0), (0.0, 2.0, 5.0, 10.0), (1.0, 2.0, 5.0, 10.0)):
        if d > 1.6 * V ** (1 / 3): continue
        dt = dt_ms * 1e-3; k = 6 * math.log(10) / T60
        for phase in (0.05, 0.5, 0.95):
            t_arr = (math.floor((d / S.C) / dt) + phase) * dt
            if t_arr * S.C < 0.6: continue
            key = (V, T60, round(t_arr * S.C, 4), gap_ms, dt_ms)
            if key in seen: continue
            seen.add(key)
            Ed = S.sabine_direct_energy(V, T60, t_arr * S.C, 1 / k)
            if 10 * math.log10((1 / k) / (Ed + 1 / k)) < -8: continue
            truth = S.truth_edt(t_arr, Ed, gap_ms * 1e-3, [1.0], [k])
            b = S.histogram(dt, 2 * T60 + t_arr + gap_ms * 1e-3, t_arr, h, Ed, gap_ms * 1e-3, [1.0], [k])
            for m in MODES: tal[m][dt_ms][score(variant(b, dt, t_arr, m), truth)[0]] += 1
    for m in MODES:
        for dt_ms in sorted(tal[m]): print(m, dt_ms, dict(tal[m][dt_ms]))
