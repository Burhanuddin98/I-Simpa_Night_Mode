"""Known-answer and adversarial tests for band_early.py (edt-band/SPEC.md section 9), round 2.

  T1  exact exponential decay (closed-form truth) at 2, 1, 0.5, 0.2, 0.1 ms, with and without air,
      direct sound as an impulse and as SPPS's ball-spread pulse, with and without a first-
      reflection bound, f64 and f32 bins;
  T2  a single impulse (a lone reflection; the direct sound alone; one on top of a decay);
  T3  two impulses straddling a bin edge;
  T4  random bin sums, and random arrangements inside them brute-forced at 0.001 ms;
  T5  an adversarial climb on EDT with every move the proof's extremes need: single atoms anywhere,
      two-atom splits with the curve held exactly on -10 dB, the rounding sign of every bin, the
      late mass and the tail;
  T6  the exact EDT band at a = 0, eps = 0 (an independent layer-cake extremiser) against the
      certified band: containment, and how tight the band is;
  T7  the adversary's edge cases through the code: the U_min = 0 histogram, Lemma W's precondition,
      a capped Dinkelbach, per-bin eps with unrecorded energy inside the series, an unsupported
      premise, and crossing ranges with U_max > 2 U_min;
  and the upstream I-Simpa port's EDT on every series of T1-T4, printed beside the band.

Every truth is computed from an explicit arrangement (exact_params, or the batched evaluator here,
checked against it) or in closed form, never from the band code. Python only; no solver is run.
At most 8 worker processes.

Usage: python -B test_band_early.py [t1 ... t7] [--workers 8] [--series 400]
Writes results_<test>.json and test_log_<tests>.txt beside this file.
"""
import json
import math
import os
import sys
import time
import traceback

sys.dont_write_bytecode = True
os.environ['PYTHONDONTWRITEBYTECODE'] = '1'
import numpy as np  # noqa: E402

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import band_early as be  # noqa: E402

C = 343.2
FINE = 1e-6                      # the brute-force grid, s
LOG = []


def log(*a):
    s = ' '.join(str(x) for x in a)
    print(s, flush=True)
    LOG.append(s)


def iso9613_m(F, H=50.0, P=101325.0, T_c=20.0):
    """Energy attenuation m (Np/m) as SPPS's Coef_Att_Atmos computes it (ISO 9613-1), x ln10/10."""
    K = T_c + 273.15
    K01, Pref, Kref = 273.16, 101325.0, 293.15
    Cc = -6.8346 * (K01 / K) ** 1.261 + 4.6151
    hmol = H * (Pref * 10 ** Cc) / Pref
    cson = 343.2 * math.sqrt(K / Kref)
    Acr = (Pref / P) * 1.60e-10 * math.sqrt(K / Kref) * F ** 2
    Fr = (P / Pref) * (24. + 4.04e4 * hmol * (0.02 + hmol) / (0.391 + hmol))
    Am = 1.559 * 0.209 * math.exp(-2239.1 / K) * (2239.1 / K) ** 2
    AvO = Am * (F / cson) * 2. * (F / Fr) / (1 + (F / Fr) ** 2)
    Fr = (P / Pref) * math.sqrt(Kref / K) * (9. + 280. * hmol * math.exp(-4.170 * ((K / Kref) ** (-1. / 3.) - 1)))
    Am = 1.559 * 0.781 * math.exp(-3352.0 / K) * (3352.0 / K) ** 2
    AvN = Am * (F / cson) * 2. * (F / Fr) / (1 + (F / Fr) ** 2)
    return (Acr + AvO + AvN) * math.log(10) / 10


AIR = {'none': 0.0, '1k': iso9613_m(1000) * C, '8k': iso9613_m(8000) * C, '16k': iso9613_m(16000) * C,
       '20k': iso9613_m(20000) * C}


# ------------------------------------------------------------------------------------------------
# Generators: recorded bins from an explicit true arrangement, with SPPS's per-step air
# ------------------------------------------------------------------------------------------------
def rec_factor(tau, dt, a, kappa=1.0):
    """recorded / true for energy arriving at tau: exp(-a ((n+kappa) dt - tau)), n = floor(tau/dt)."""
    n = np.floor(tau / dt)
    return np.exp(-a * ((n + kappa) * dt - tau)), n.astype(np.int64)


def ball_direct_atoms(ta, h, Ed, step=FINE):
    """SPPS's ball receiver: energy x chord density ~ (R^2 - (rho - D)^2) / rho over rho = c tau in
    [D - R, D + R] (skeptic-gf3/ism.py). As atoms on a `step` grid, total true energy Ed."""
    if h <= 0:
        return np.array([ta]), np.array([Ed])
    n = max(int(round(2 * h / step)), 1)
    tau = ta - h + (np.arange(n) + 0.5) * (2 * h / n)
    D, R = C * ta, C * h
    rho = C * tau
    w = np.maximum(R * R - (rho - D) ** 2, 0.0) / rho
    return tau, Ed * w / w.sum()


def expo_bins(dt, ta, direct_tau, direct_E, A, k, tr, a, n_bins, kappa=1.0):
    """Recorded bins of: direct atoms (true energies) + a true density A e^{-k tau} on [tr, inf)."""
    n = np.arange(n_bins)
    t0 = n * dt
    t1 = t0 + dt
    lo = np.maximum(t0, tr)
    kk = k - a
    seg = np.where(t1 > lo, A * np.exp(-a * (n + kappa) * dt) * (np.exp(-kk * lo) - np.exp(-kk * t1)) / kk, 0.0)
    f, nn = rec_factor(direct_tau, dt, a, kappa)
    np.add.at(seg, nn, direct_E * f)
    remainder = A / k * math.exp(-k * max(n_bins * dt, tr))     # true energy after the series
    return seg, remainder


def expo_truth(ta, Ed, A, k, tr, te_list=(0.05, 0.08)):
    """Closed-form continuous-time values: direct Ed at u = 0, density A e^{-k tau} from tr >= ta."""
    g = tr - ta
    Sr = A / k * math.exp(-k * tr)
    S0 = Ed + Sr
    phi = 0.1 * S0
    out = {}
    L = math.log(Sr / phi) / k
    U = g + L
    J = -k * (L ** 3 / 3 + (g - U / 2) * L ** 2 / 2)
    slope = be.DB * 12 * J / U ** 3
    out['edt'] = -60 / slope
    out['ts'] = A * math.exp(-k * tr) * (g / k + 1 / k ** 2) / S0
    for te in te_list:
        early = Ed + (Sr * (1 - math.exp(-k * (te - g))) if te > g else 0.0)
        out['d%g' % (te * 1e3)] = early / S0
        out['c%g' % (te * 1e3)] = be.DB * math.log(early / (S0 - early))
    return out


def within(band, x, rel=1e-9, ab=1e-12):
    if band is None:
        return None
    lo, hi = band
    if not np.isfinite(x):
        return (x == hi) or (x == lo)
    return (lo - rel * abs(lo) - ab) <= x <= (hi + rel * abs(hi) + ab)


def summarise(res, truth):
    """Row of a results table: each quantity's band, value, half-width, verdict, truth inside."""
    row = {}
    for q, key in (('edt', 'edt'), ('ts', 'ts'), ('c50', 'c50'), ('c80', 'c80'), ('d50', 'd50')):
        r = res[q]
        t = truth.get(key)
        row[q] = dict(band=r.get('band'), value=r.get('value'), refused=r.get('refused'),
                      half_width_L=r.get('half_width_L'), half_width_JND=r.get('half_width_JND'),
                      half_width_pct=r.get('half_width_pct'), truth=t,
                      inside=within(r.get('band'), t) if (t is not None and r.get('band') is not None) else None)
    row['edt']['isimpa'] = res['edt'].get('isimpa')
    row['edt']['witness'] = res['edt'].get('witness')
    return row


def jsonable(x):
    if isinstance(x, dict):
        return {str(k): jsonable(v) for k, v in x.items()}
    if isinstance(x, (list, tuple)):
        return [jsonable(v) for v in x]
    if isinstance(x, (np.floating, float)):
        v = float(x)
        return v if math.isfinite(v) else repr(v)
    if isinstance(x, (np.integer,)):
        return int(x)
    if isinstance(x, (np.bool_,)):
        return bool(x)
    return x


def fmt_band(b, scale=1.0, nd=5):
    if b is None:
        return '-'
    return '[%.*f, %.*f]' % (nd, b[0] * scale, nd, b[1] * scale)


# ------------------------------------------------------------------------------------------------
# T1: exact exponential decay
# ------------------------------------------------------------------------------------------------
def t1():
    log('\n=== T1: exact exponential decay, closed-form truth ===')
    rows = []
    fails = 0
    ta = 0.01234
    T60 = 0.5
    for dt in (2e-3, 1e-3, 5e-4, 2e-4, 1e-4):
        for air in ('none', '1k', '16k', '20k'):
            a = AIR[air]
            k = 6 * math.log(10) / T60 + 0.0          # total decay rate of energy (incl. air), 1/s
            for dname, h in (('impulse', 0.0), ('ball R=0.31', 0.31 / C)):
                for gap in (0.0, 2.5e-3):
                    for prec in ('f64', 'f32'):
                        tr = ta + gap
                        Ed = 0.5 * (1.0 / k) * math.exp(-k * tr)     # direct = half the reverberant
                        A = 1.0
                        dtau, dE = ball_direct_atoms(ta, h, Ed)
                        n_bins = int(math.ceil(3.0 / dt))
                        B, rem = expo_bins(dt, ta, dtau, dE, A, k, tr, a, n_bins)
                        eps = 0.0
                        if prec == 'f32':
                            B = B.astype(np.float32).astype(np.float64)
                            eps = 2.0 ** -24
                        truth = expo_truth(ta, Ed, A, k, tr)
                        s = be.Setup(B, dt, ta, half_width=h, air_rate=a, rel_eps=eps, tail_max=rem * 1.001,
                                     tail_t_max=n_bins * dt + 100.0,
                                     t_refl_min=(tr if gap > 0 else None))
                        t = time.time()
                        res = s.all_bands()
                        el = time.time() - t
                        row = summarise(res, truth)
                        row.update(dt=dt, air=air, a=a, direct=dname, gap=gap, prec=prec, seconds=el)
                        rows.append(row)
                        bad = [q for q in ('edt', 'ts', 'c50', 'c80', 'd50') if row[q]['inside'] is False]
                        fails += len(bad)
                        if prec == 'f64':
                            e = row['edt']
                            up = e['isimpa']
                            log('dt %4.1f ms  air %-4s  %-11s gap %.1f ms | EDT %s true %.5f in=%s hw %.3f%% = %.2f L %s | '
                                'I-Simpa %.5f (%+.2f%%) | Ts in=%s C50 in=%s C80 in=%s D50 in=%s  %.2fs%s' % (
                                    dt * 1e3, air, dname, gap * 1e3, fmt_band(e['band']), truth['edt'], e['inside'],
                                    e['half_width_pct'] or float('nan'), e['half_width_L'] or float('nan'),
                                    ('REFUSED ' + e['refused']) if e['refused'] else 'accepted',
                                    up, 100 * (up / truth['edt'] - 1), row['ts']['inside'], row['c50']['inside'],
                                    row['c80']['inside'], row['d50']['inside'], el, ('  FAIL ' + str(bad)) if bad else ''))
    log('T1: %d rows, %d truths outside their band' % (len(rows), fails))
    return dict(rows=rows, fails=fails)


# ------------------------------------------------------------------------------------------------
# Fine arrangements (T2-T5)
# ------------------------------------------------------------------------------------------------
def bins_from_atoms(tau, E, dt, a, n_bins, kappa=1.0):
    f, nn = rec_factor(tau, dt, a, kappa)
    B = np.zeros(n_bins)
    np.add.at(B, nn, E * f)
    return B


def reading(tau, ta, direct_mask):
    """Reading time of each atom under definition (A): direct -> 0; before ta -> 0; else tau - ta."""
    u = tau - ta
    return np.where(direct_mask | (u <= 0), 0.0, u)


def expo_atoms(ta, tr, A, k, t_end, step=FINE, rng=None):
    """A e^{-k tau} on [tr, t_end) as atoms, one per `step` cell (at a random point of it when rng)."""
    n = int(math.ceil((t_end - tr) / step))
    left = tr + np.arange(n) * step
    E = A / k * (np.exp(-k * left) - np.exp(-k * np.minimum(left + step, t_end)))
    tau = left + (rng.random(n) if rng is not None else 0.5) * step
    return tau, E


def eval_fine(tau, E, direct, ta):
    return be.exact_params(reading(tau, ta, direct), E)


def t2():
    log('\n=== T2: a single impulse ===')
    out = []
    fails = 0
    ta = 0.01234
    for dt in (2e-3, 1e-3, 5e-4, 1e-4):
        for air in ('none', '20k'):
            a = AIR[air]
            cases = []
            # (a) direct + one reflection 63.7 ms later, nothing else
            cases.append(('direct + lone reflection', np.array([ta, ta + 0.0637]), np.array([1.0, 0.6]),
                          np.array([True, False])))
            # (b) the direct sound alone
            cases.append(('direct alone', np.array([ta]), np.array([1.0]), np.array([True])))
            # (c) a strong reflection at u = 23.45 ms on top of an exponential decay
            k = 6 * math.log(10) / 0.6
            et, eE = expo_atoms(ta, ta, 1.0, k, ta + 1.6)
            Sr = eE.sum()
            cases.append(('reflection on a decay', np.concatenate([[ta], et, [ta + 0.02345]]),
                          np.concatenate([[0.4 * Sr], eE, [0.3 * Sr]]),
                          np.concatenate([[True], np.zeros(len(et), bool), [False]])))
            # (d) energy before the direct window: the premises exclude it
            cases.append(('energy before the arrival', np.concatenate([[ta - 0.004], et[:1] * 0 + ta, et]),
                          np.concatenate([[0.05 * Sr], [0.4 * Sr], eE]),
                          np.concatenate([[False, True], np.zeros(len(et), bool)])))
            for name, tau, E, direct in cases:
                n_bins = int(math.ceil((tau.max() + dt) / dt)) + 2
                B = bins_from_atoms(tau, E, dt, a, n_bins)
                truth = eval_fine(tau, E, direct, ta)
                s = be.Setup(B, dt, ta, air_rate=a)
                res = s.all_bands()
                row = summarise(res, truth)
                row.update(case=name, dt=dt, air=air)
                if name == 'energy before the arrival':
                    refs = [res[q].get('refused') for q in ('edt', 'ts', 'c50', 'c80', 'd50')]
                    ok = all(r == 'arrival_misfit' for r in refs)
                    fails += 0 if ok else 1
                    log('%-26s dt %4.1f ms air %-4s | every quantity refused %s: %s  (misfit share %.3g)%s' % (
                        name, dt * 1e3, air, refs[0], ok, res['diagnostics']['misfit_share'], '' if ok else '  FAIL'))
                    out.append(row)
                    continue
                bad = [q for q in ('edt', 'ts', 'c50', 'c80', 'd50') if row[q]['inside'] is False]
                fails += len(bad)
                out.append(row)
                e = row['edt']
                log('%-26s dt %4.1f ms air %-4s | EDT %s truth %s %s | Ts %s truth %.6f in=%s | D50 %s truth %.4f in=%s | '
                    'C50 %s (%s) truth %.3f | C80 %s truth %s | I-Simpa EDT %s%s' % (
                        name, dt * 1e3, air, fmt_band(e['band']), '%.5f' % truth['edt'] if np.isfinite(truth['edt']) else truth['edt'],
                        ('refused ' + e['refused']) if e['refused'] else ('in=%s' % e['inside']),
                        fmt_band(row['ts']['band'], 1, 6), truth['ts'], row['ts']['inside'],
                        fmt_band(row['d50']['band'], 1, 4), truth['d50'], row['d50']['inside'],
                        fmt_band(row['c50']['band'], 1, 3), row['c50']['refused'] or 'accepted', truth['c50'],
                        row['c80']['refused'] or fmt_band(row['c80']['band'], 1, 3), truth['c80'],
                        '%.5f' % e['isimpa'] if e['isimpa'] is not None and np.isfinite(e['isimpa']) else e['isimpa'],
                        ('  FAIL ' + str(bad)) if bad else ''))
    log('T2: %d cases, %d truths outside their band' % (len(out), fails))
    return dict(rows=out, fails=fails)


def t3():
    log('\n=== T3: two impulses straddling a bin edge ===')
    out = []
    fails = 0
    ta = 0.01234
    k = 6 * math.log(10) / 0.4
    for dt in (2e-3, 1e-3, 5e-4, 2e-4, 1e-4):
        for air in ('none', '16k'):
            a = AIR[air]
            et, eE = expo_atoms(ta, ta, 1.0, k, ta + 1.2)
            Sr = eE.sum()
            for u_edge_target in (0.0081, 0.0213, 0.0405):
                edge = math.ceil((ta + u_edge_target) / dt) * dt          # a bin edge
                for w1, w2 in ((0.2, 0.05), (0.05, 0.2), (0.12, 0.12)):
                    tau = np.concatenate([[ta], et, [edge - 0.5e-6, edge + 0.5e-6]])
                    E = np.concatenate([[0.5 * Sr], eE, [w1 * Sr, w2 * Sr]])
                    direct = np.concatenate([[True], np.zeros(len(et) + 2, bool)])
                    n_bins = int(math.ceil((tau.max() + dt) / dt)) + 2
                    B = bins_from_atoms(tau, E, dt, a, n_bins)
                    truth = eval_fine(tau, E, direct, ta)
                    s = be.Setup(B, dt, ta, air_rate=a)
                    res = s.all_bands()
                    row = summarise(res, truth)
                    row.update(dt=dt, air=air, edge_u=edge - ta, w=(w1, w2))
                    bad = [q for q in ('edt', 'ts', 'c50', 'c80', 'd50') if row[q]['inside'] is False]
                    fails += len(bad)
                    out.append(row)
                    e = row['edt']
                    log('dt %4.1f ms air %-4s edge at u=%.4f s masses (%.2f, %.2f) | EDT %s truth %.5f in=%s hw %.2f L %s | '
                        'I-Simpa %.5f (%+.2f%%) | Ts/C50/C80/D50 in=%s/%s/%s/%s%s' % (
                            dt * 1e3, air, edge - ta, w1, w2, fmt_band(e['band']), truth['edt'], e['inside'],
                            e['half_width_L'] or float('nan'), e['refused'] or 'accepted', e['isimpa'],
                            100 * (e['isimpa'] / truth['edt'] - 1), row['ts']['inside'], row['c50']['inside'],
                            row['c80']['inside'], row['d50']['inside'], ('  FAIL ' + str(bad)) if bad else ''))
    log('T3: %d cases, %d truths outside their band' % (len(out), fails))
    return dict(rows=out, fails=fails)


# ------------------------------------------------------------------------------------------------
# T4: random bin sums, random arrangements at 0.001 ms
# ------------------------------------------------------------------------------------------------
def random_series(seed):
    """Random room-like recorded bins, and everything needed to draw arrangements consistent with them."""
    rng = np.random.default_rng(seed)
    dt = float(rng.choice([2e-3, 1e-3, 5e-4, 2e-4, 1e-4]))
    air = str(rng.choice(list(AIR)))
    a = AIR[air]
    d = float(rng.uniform(0.7, 25.0))
    ta = d / C + float(rng.integers(0, 3)) * dt      # a source delay of whole steps
    h = float(rng.choice([0.0, 0.1, 0.31, 0.5])) / C
    use_t1 = bool(rng.random() < 0.5)
    gap = float(rng.uniform(0.0, 6e-3)) if use_t1 else 0.0
    trm = ta - h + gap if use_t1 else ta - h
    T60 = float(rng.uniform(0.12, 1.0))
    k = 6 * math.log(10) / T60
    t_end = ta + min(1.3 * T60, 1.0)
    n_bins = int(math.ceil(t_end / dt)) + 1
    t0 = np.arange(n_bins) * dt
    t1_ = t0 + dt
    # admissible physical span of each bin: direct window, and reflected from trm on
    lo_dir = np.maximum(t0, ta - h)
    hi_dir = np.minimum(t1_, ta + h)
    lo_ref = np.maximum(t0, trm)
    ok = (lo_dir <= hi_dir) | (lo_ref < t1_)
    env = np.where(t0 + dt > trm, np.exp(-k * np.maximum(t0 - trm, 0.0)), 0.0)
    B = env * dt * rng.lognormal(0.0, float(rng.choice([0.05, 0.3, 1.0])), n_bins)
    # sparse early reflections
    for _ in range(int(rng.integers(0, 8))):
        j = int(np.searchsorted(t0, trm + rng.exponential(0.02), side='right')) - 1
        if 0 <= j < n_bins:
            B[j] += float(rng.uniform(0.2, 3.0)) * dt * math.exp(-k * max(t0[j] - trm, 0.0)) * rng.uniform(1, 20)
    # direct sound
    Ed_rel = float(rng.choice([0.05, 0.3, 1.0, 3.0]))
    Sr = B.sum()
    jd = np.nonzero(lo_dir <= hi_dir)[0]
    B[jd] += Ed_rel * Sr * rng.dirichlet(np.ones(len(jd)))
    B = np.where(ok, B, 0.0)
    B = B.astype(np.float32).astype(np.float64)
    # a third of the series are cut short: the band then gets only the bins before the cut and a
    # rigorous bound on the true energy after it (recorded x 1/p, the largest air correction)
    cut = None
    if rng.random() < 0.33:
        cut = int(n_bins * float(rng.uniform(0.35, 0.9)))
    return dict(seed=seed, dt=dt, air=air, a=a, ta=ta, h=h, trm=trm, use_t1=use_t1, T60=T60, B=B,
                n_bins=n_bins, d=d, cut=cut)


def setup_for(S, eps=0.0):
    kw = dict(half_width=S['h'], air_rate=S['a'], t_refl_min=(S['trm'] if S['use_t1'] else None), rel_eps=eps)
    if S['cut'] is None:
        return be.Setup(S['B'], S['dt'], S['ta'], **kw)
    B = S['B']
    c = S['cut']
    # a rigorous bound on the true energy after the cut: recorded x (1 + eps) x 1/p
    tail = float(B[c:].sum()) * (1 + eps) * math.exp(S['a'] * S['dt'])
    return be.Setup(B[:c], S['dt'], S['ta'], tail_max=tail * (1 + 1e-12), tail_t_max=len(B) * S['dt'], **kw)


def admissible_points(S, n, rng=None):
    """Bin n's 1 us cells, each cut to the part where energy can arrive (the direct window, or from
    t_refl_min on), with one point per non-empty cut (random inside it, or its middle) and whether
    energy there can be direct and can be reflected."""
    dt, ta, h, trm = S['dt'], S['ta'], S['h'], S['trm']
    t0 = n * dt
    t1 = t0 + dt
    ncell = int(round(dt / FINE))
    cl = t0 + np.arange(ncell) * FINE
    ch = np.minimum(cl + FINE, t1)
    dl, dh = max(t0, ta - h), min(t1, ta + h)
    rl = max(t0, trm)
    d_l, d_h = np.maximum(cl, dl), np.minimum(ch, dh)
    r_l, r_h = np.maximum(cl, rl), ch
    has_d = d_l <= d_h
    has_r = r_l < r_h
    lo = np.where(has_r, r_l, d_l)
    hi = np.where(has_r, r_h, d_h)
    both = has_d & has_r
    if rng is not None:
        pick_d = both & (rng.random(ncell) < 0.5)
    else:
        pick_d = np.zeros(ncell, bool)
    lo = np.where(pick_d, d_l, lo)
    hi = np.where(pick_d, d_h, hi)
    ok = has_d | has_r
    frac = rng.random(ncell) if rng is not None else np.full(ncell, 0.5)
    pt = lo + frac * (hi - lo)
    pt = np.minimum(pt, np.nextafter(t1, -np.inf))
    can_d = (pt >= ta - h) & (pt <= ta + h)
    can_r = pt >= trm
    return pt[ok], can_d[ok], can_r[ok]


def random_arrangement(S, rng, style):
    """An arrangement consistent with S['B'] at 0.001 ms: per bin, recorded-frame weights on the
    bin's admissible 1 us cells (a random point inside each), true = recorded / SPPS's factor."""
    dt, ta, a = S['dt'], S['ta'], S['a']
    B = S['B']
    taus, Es, dirs = [], [], []
    for n in np.nonzero(B > 0)[0]:
        tau, can_d, can_r = admissible_points(S, n, rng)
        m = len(tau)
        if m == 0:
            raise RuntimeError('bin %d holds energy but has no admissible point' % n)
        if style == 'single':
            w = np.zeros(m)
            w[rng.integers(m)] = 1.0
        elif style == 'few':
            w = np.zeros(m)
            w[rng.integers(m, size=int(rng.integers(2, 4)))] = rng.random(1)[0] + 0.1
        elif style == 'edge':
            w = np.zeros(m)
            w[0 if rng.random() < 0.5 else -1] = 1.0
        elif style == 'dirichlet':
            w = rng.dirichlet(np.full(m, float(rng.choice([0.05, 1.0]))))
        else:
            w = np.ones(m)
        w = w / w.sum()
        f = np.exp(-a * ((n + 1.0) * dt - tau))           # SPPS: recorded = true * p^(n+1) / e^{-a tau}
        E = B[n] * w / f
        p_dir = float(rng.random())
        pick_dir = np.where(can_d & can_r, rng.random(m) < p_dir, can_d & ~can_r)
        taus.append(tau)
        Es.append(E)
        dirs.append(pick_dir)
    tau = np.concatenate(taus)
    E = np.concatenate(Es)
    direct = np.concatenate(dirs)
    keep = E > 0
    return tau[keep], E[keep], direct[keep]


QKEYS = (('edt', 'edt'), ('ts', 'ts'), ('c50', 'c50'), ('c80', 'c80'), ('d50', 'd50'))


def t4_worker(args):
    seed, n_arr = args
    try:
        S = random_series(seed)
        s = setup_for(S)
        t = time.time()
        res = s.all_bands()
        el = time.time() - t
        rng = np.random.default_rng(seed + 10 ** 6)
        viol = []
        pos_in = {q: [] for q, _ in QKEYS}      # where each truth sits in its band, 0 = lo, 1 = hi
        up = res['edt'].get('isimpa')
        up_inside = within(res['edt'].get('band'), up) if res['edt'].get('band') is not None else None
        edt_truths = []
        for i in range(n_arr):
            style = ['single', 'few', 'edge', 'dirichlet', 'uniform'][i % 5]
            tau, E, direct = random_arrangement(S, rng, style)
            tr = eval_fine(tau, E, direct, S['ta'])
            for q, key in QKEYS:
                b = res[q].get('band')
                x = tr.get(key)
                if b is None or x is None:
                    continue
                if not np.isfinite(x):
                    if not ((x > 0 and b[1] == np.inf) or (x < 0 and b[0] == -np.inf)):
                        viol.append(dict(q=q, style=style, truth=repr(x), band=b))
                    continue
                if not within(b, x):
                    viol.append(dict(q=q, style=style, truth=x, band=b))
                if np.isfinite(b[0]) and np.isfinite(b[1]) and b[1] > b[0]:
                    pos_in[q].append((x - b[0]) / (b[1] - b[0]))
            if np.isfinite(tr['edt']):
                edt_truths.append(tr['edt'])
        up_err = None
        if edt_truths and up is not None and np.isfinite(up):
            up_err = max(abs(up / x - 1) for x in edt_truths)
        return dict(seed=seed, dt=S['dt'], air=S['air'], a=S['a'], h=S['h'], use_t1=S['use_t1'], T60=S['T60'],
                    n_bins=S['n_bins'], cut=S['cut'], seconds=el, violations=viol,
                    bands={q: res[q].get('band') for q, _ in QKEYS},
                    refused={q: res[q].get('refused') for q, _ in QKEYS},
                    hwL={q: res[q].get('half_width_L') for q, _ in QKEYS},
                    edt_witness=res['edt'].get('witness'), edt_widened=res['edt'].get('widened_to_witness'),
                    reach={q: (min(v), max(v)) if v else None for q, v in pos_in.items()},
                    isimpa=up, isimpa_inside=up_inside, isimpa_max_rel_err_vs_arrangements=up_err,
                    n_arr=n_arr)
    except Exception:
        return dict(seed=seed, error=traceback.format_exc())


def t4(workers=8, n_series=160, n_arr=40):
    log('\n=== T4: random bin sums, %d series x %d arrangements at 0.001 ms (%d workers) ===' % (
        n_series, n_arr, workers))
    from multiprocessing import Pool
    args = [(1000 + i, n_arr) for i in range(n_series)]
    t = time.time()
    with Pool(min(workers, 8)) as pool:
        rows = pool.map(t4_worker, args, chunksize=1)
    errs = [r for r in rows if 'error' in r]
    rows = [r for r in rows if 'error' not in r]
    nviol = sum(len(r['violations']) for r in rows)
    n_checks = sum(r['n_arr'] for r in rows) * 5
    log('T4: %d series (%d errors), %d arrangement-quantity checks, %d outside their band, %.0f s' % (
        len(rows), len(errs), n_checks, nviol, time.time() - t))
    for e in errs:
        log('ERROR seed %d:\n%s' % (e['seed'], e['error']))
    for r in rows:
        for v in r['violations']:
            log('VIOLATION seed %d %s' % (r['seed'], v))
    # how much of each band random arrangements reach
    for q, _ in QKEYS:
        reach = [r['reach'][q] for r in rows if r['reach'][q] is not None]
        acc = sum(1 for r in rows if r['refused'][q] is None)
        if reach:
            lo = np.median([x[0] for x in reach])
            hi = np.median([x[1] for x in reach])
            log('  %-4s accepted %3d/%d; random arrangements reach a median [%.2f, %.2f] of the band' % (
                q, acc, len(rows), lo, hi))
    log('  EDT band widened to a witness by the rounding guard in %d of %d series' % (
        sum(1 for r in rows if r.get('edt_widened')), len(rows)))
    ins = [r['isimpa_inside'] for r in rows if r['isimpa_inside'] is not None]
    errs_up = [r['isimpa_max_rel_err_vs_arrangements'] for r in rows if r['isimpa_max_rel_err_vs_arrangements'] is not None]
    if ins:
        log('  I-Simpa EDT inside the band in %d of %d series; its largest error against an arrangement: '
            'median %.2f %%, max %.1f %%' % (sum(ins), len(ins), 100 * np.median(errs_up), 100 * max(errs_up)))
    by_dt = {}
    for r in rows:
        if r['hwL']['edt'] is not None:
            by_dt.setdefault(r['dt'], []).append(r['hwL']['edt'])
    for dt in sorted(by_dt):
        v = by_dt[dt]
        log('  EDT half-width at %.1f ms: median %.2f L, %d of %d within L' % (
            dt * 1e3, float(np.median(v)), sum(1 for x in v if x <= 1), len(v)))
    return dict(rows=rows, errors=errs, violations=nviol, checks=n_checks)




# ------------------------------------------------------------------------------------------------
# A batched, independent EDT evaluator (Definition A) for the searches below; cross-checked against
# be.exact_params in T5 before it is trusted
# ------------------------------------------------------------------------------------------------
def edt_batch(pos, mass):
    """EDT of K arrangements. pos, mass: (K, M). u == 0 counts in S(0) only. nan when undefined."""
    pos = np.atleast_2d(np.asarray(pos, float))
    mass = np.atleast_2d(np.asarray(mass, float))
    K = pos.shape[0]
    S0 = mass.sum(1)
    p = np.where(pos > 0, pos, 0.0)
    m = np.where(pos > 0, mass, 0.0)
    o = np.argsort(p, axis=1, kind='stable')
    p = np.take_along_axis(p, o, 1)
    m = np.take_along_axis(m, o, 1)
    tail = np.cumsum(m[:, ::-1], 1)[:, ::-1]
    ok = tail >= 0.1 * S0[:, None]
    cnt = ok.sum(1)
    i_star = np.maximum(cnt - 1, 0)
    U = p[np.arange(K), i_star]
    left = np.concatenate([np.zeros((K, 1)), p[:, :-1]], 1)
    hU = 0.5 * U[:, None]
    seg = 0.5 * ((p - hU) ** 2 - (left - hU) ** 2)
    with np.errstate(divide='ignore', invalid='ignore'):
        lev = np.log(np.where(ok & (tail > 0), tail / S0[:, None], 1.0))
        J = np.where(ok, lev * seg, 0.0).sum(1)
        slope = be.DB * 12.0 * J / U ** 3
    edt = np.where(slope < 0, -60.0 / np.where(slope < 0, slope, -1.0), np.inf)
    return np.where((cnt == 0) | ~(U > 0), np.nan, edt)


# ------------------------------------------------------------------------------------------------
# T5: adversarial climb on EDT with every move the proof's extremes need
# ------------------------------------------------------------------------------------------------
FGRID = np.array([0.02, 0.1, 0.25, 0.5, 0.75, 0.9, 0.98])


class Climber:
    """Coordinate search over the admissible set of a series, built from the premises (not from the
    band code): window bins hold up to two atoms each (direct or reflected, any admissible time on a
    K-point grid plus span ends and t_a, t_refl_min, the direct-window ends), their recorded total
    is B (1 + s eps) with s = -1 or +1; bins read after the window are one late true mass anywhere
    in its range, with the unrecorded energy in [0, Tmax] added to it."""

    def __init__(self, B, dt, ta, h, trm, a, eps, Tmax, U_hi, K=24):
        self.B, self.dt, self.ta, self.h, self.trm, self.a, self.eps = B, dt, ta, h, trm, a, eps
        N = len(B)
        win, grids = [], []
        late_lo = late_hi = 0.0
        for n in np.nonzero(B > 0)[0]:
            t0, t1 = n * dt, (n + 1) * dt
            t1m = np.nextafter(t1, -np.inf)
            taus, dirs = [], []
            dl, dh = max(t0, ta - h), min(t1m, ta + h)
            if dl <= dh:
                x = np.unique(np.concatenate([np.linspace(dl, dh, K), [dl, dh]]))
                taus.append(x)
                dirs.append(np.ones(len(x), bool))
            rl = max(t0, trm)
            r_ok = rl <= t1m
            if r_ok:
                sp = [v for v in (ta, trm, ta - h, ta + h, np.nextafter(ta, np.inf)) if rl <= v <= t1m]
                x = np.unique(np.concatenate([np.linspace(rl, t1m, K), sp, [rl, t1m]]))
                taus.append(x)
                dirs.append(np.zeros(len(x), bool))
            if not taus:
                raise ValueError('bin %d holds energy the premises exclude' % n)
            tau = np.concatenate(taus)
            d = np.concatenate(dirs)
            u = np.where(d | (tau <= ta), 0.0, tau - ta)
            th = np.exp(a * ((n + 1.0) * dt - tau))
            g0 = max(t0, ta) - ta
            if dl <= dh or g0 <= U_hi + 2 * dt:
                win.append(n)
                grids.append((u, th, d))
            else:
                late_lo += B[n] * (1 - eps) * float(np.exp(a * ((n + 1.0) * dt - t1m)))
                late_hi += B[n] * (1 + eps) * float(np.exp(a * ((n + 1.0) * dt - rl)))
        self.win = np.array(win)
        self.grids = grids
        self.late = (late_lo, late_hi + Tmax)
        self.far = N * dt - ta + 1.0
        W = len(win)
        self.iA = np.zeros(W, int)
        self.iB = np.zeros(W, int)
        self.f = np.ones(W)
        self.s = np.zeros(W)
        self.lam = 0.5

    def rec(self, i, s):
        return self.B[self.win[i]] * (1 + s * self.eps)

    def atoms(self):
        W = len(self.win)
        uA = np.array([self.grids[i][0][self.iA[i]] for i in range(W)])
        uB = np.array([self.grids[i][0][self.iB[i]] for i in range(W)])
        tA = np.array([self.grids[i][1][self.iA[i]] for i in range(W)])
        tB = np.array([self.grids[i][1][self.iB[i]] for i in range(W)])
        r = self.B[self.win] * (1 + self.s * self.eps)
        late = self.late[0] + self.lam * (self.late[1] - self.late[0])
        return (np.concatenate([uA, uB, [self.far]]),
                np.concatenate([r * self.f * tA, r * (1 - self.f) * tB, [late]]))

    def value(self):
        p, m = self.atoms()
        return float(edt_batch(p, m)[0])

    def candidates(self, i, two):
        """Candidate states of window bin i: arrays (iA, iB, f, s); f = nan: the plateau split."""
        u, th, d = self.grids[i]
        G = len(u)
        svals = [-1.0, 1.0] if self.eps > 0 else [0.0]
        cA, cB, cf, cs = [], [], [], []
        for sv in svals:
            cA.append(np.arange(G)); cB.append(np.arange(G)); cf.append(np.ones(G)); cs.append(np.full(G, sv))
        if two and G > 1:
            anchors = set()
            for flag in (True, False):
                sel = np.nonzero(d == flag)[0]
                if len(sel):
                    anchors.update([int(sel[0]), int(sel[-1])])
            for j in anchors:
                for sv in svals:
                    for fv in list(FGRID) + [np.nan]:
                        cA.append(np.full(G, j)); cB.append(np.arange(G)); cf.append(np.full(G, fv))
                        cs.append(np.full(G, sv))
        return np.concatenate(cA), np.concatenate(cB), np.concatenate(cf), np.concatenate(cs)

    def batch(self, i, cand):
        iA, iB, f, s = cand
        u, th, d = self.grids[i]
        p0, m0 = self.atoms()
        W = len(self.win)
        keep = np.ones(len(p0), bool)
        keep[i] = keep[W + i] = False
        pr, mr = p0[keep], m0[keep]
        r = self.B[self.win[i]] * (1 + s * self.eps)
        uA, uB, thA, thB = u[iA], u[iB], th[iA], th[iB]
        f = f.copy()
        nanf = np.isnan(f)
        if nanf.any():
            # the plateau split: the curve just after the earlier atom held at phi (1 + 1e-10)
            x = np.minimum(uA[nanf], uB[nanf])
            R0 = mr.sum()
            srt = np.argsort(pr)
            ps, ms = pr[srt], mr[srt]
            suf = np.concatenate([np.cumsum(ms[::-1])[::-1], [0.0]])
            Rx = suf[np.searchsorted(ps, x, side='right')]
            a1 = (uA[nanf] > x) * thA[nanf]
            b1 = (uB[nanf] > x) * thB[nanf]
            rr = r[nanf]
            tg = 0.1 * (1 + 1e-10)
            num = tg * (R0 + rr * thB[nanf]) - Rx - rr * b1
            den = rr * (a1 - b1) - tg * rr * (thA[nanf] - thB[nanf])
            with np.errstate(divide='ignore', invalid='ignore'):
                ff = num / den
            f[nanf] = np.where(np.isfinite(ff), np.clip(ff, 0.0, 1.0), 0.5)
        C = len(iA)
        PP = np.concatenate([np.broadcast_to(pr, (C, len(pr))), uA[:, None], uB[:, None]], 1)
        MM = np.concatenate([np.broadcast_to(mr, (C, len(mr))), (r * f * thA)[:, None], (r * (1 - f) * thB)[:, None]], 1)
        return PP, MM, f

    def start(self, kind, rng, U_guess):
        for i, (u, th, d) in enumerate(self.grids):
            if kind == 'start':
                j = int(np.argmin(np.where(d, np.inf, u))) if (~d).any() else 0
            elif kind == 'end':
                j = int(np.argmax(u))
            elif kind == 'half':
                j = int(np.argmin(np.abs(u - 0.5 * U_guess)))
            else:
                j = int(rng.integers(len(u)))
            self.iA[i] = self.iB[i] = j
            self.f[i] = 1.0
            self.s[i] = float(rng.choice([-1.0, 1.0])) if self.eps > 0 else 0.0
        self.lam = float(rng.random())

    def climb(self, mode, rng, two_mask, sweeps=4, budget=20.0):
        t0 = time.time()
        sg = 1.0 if mode == 'max' else -1.0
        cur = self.value()
        if not np.isfinite(cur):
            cur = -np.inf if mode == 'max' else np.inf
        for _ in range(sweeps):
            improved = False
            for i in rng.permutation(len(self.win)):
                cand = self.candidates(i, bool(two_mask[i]))
                PP, MM, f = self.batch(i, cand)
                e = edt_batch(PP, MM)
                e = np.where(np.isfinite(e), e, np.nan)
                if np.all(np.isnan(e)):
                    continue
                j = int(np.nanargmax(sg * e))
                if sg * e[j] > sg * cur * (1 + 1e-13) or not np.isfinite(cur):
                    self.iA[i], self.iB[i], self.f[i], self.s[i] = cand[0][j], cand[1][j], f[j], cand[3][j]
                    cur = float(e[j])
                    improved = True
                if time.time() - t0 > budget:
                    break
            lam0 = self.lam
            for lam in np.linspace(0, 1, 21):
                self.lam = lam
                v = self.value()
                if np.isfinite(v) and sg * v > sg * cur * (1 + 1e-13):
                    cur, lam0, improved = v, lam, True
            self.lam = lam0
            if not improved or time.time() - t0 > budget:
                break
        return cur


def t5_series(seed):
    """A T4 series with a recording tolerance and, for a third of them, a cut with a tail bound."""
    S = random_series(seed)
    rng = np.random.default_rng(seed + 99)
    S['eps'] = float(rng.choice([0.0, 2.0 ** -24, 1e-3, 1e-2]))
    if S['dt'] < 5e-4:
        S['dt_orig'] = S['dt']
    return S


def wide_series(seed):
    """A series whose -10 dB crossing can lie anywhere over more than a factor 2 in U (the regime
    of the adversary's Lemma W finding): the arrival late in its bin, a dominant direct sound that
    leaves the level just after the arrival between -9 and -4 dB, early reflections near -10 dB,
    air and rounding."""
    rng = np.random.default_rng(80000 + seed)
    dt = float(rng.choice([2e-3, 1e-3]))
    air = str(rng.choice(['1k', '8k', '16k', '20k']))
    a = AIR[air]
    ta = (int(rng.integers(2, 30)) + float(rng.uniform(0.55, 0.97))) * dt
    h = float(rng.choice([0.0, 0.1, 0.31])) / C
    h = min(h, 0.9 * (math.ceil(ta / dt) * dt - ta))           # keep the window inside the arrival bin
    use_t1 = bool(rng.random() < 0.3)
    trm = ta - h + (float(rng.uniform(0, 0.5)) * (math.ceil(ta / dt) * dt - ta) if use_t1 else 0.0)
    T60 = float(rng.uniform(0.15, 1.2))
    k = 6 * math.log(10) / T60
    n_bins = int(math.ceil((ta + 1.2 * T60) / dt)) + 1
    t0 = np.arange(n_bins) * dt
    B = np.where(t0 + dt > trm, np.exp(-k * np.maximum(t0 - trm, 0.0)) * dt * rng.lognormal(0, 0.6, n_bins), 0.0)
    na = int(math.floor(ta / dt))
    for j in range(na + 1, na + 4):
        if j < n_bins:
            B[j] *= float(rng.uniform(0.2, 6.0))
    Sr = B.sum()
    lev = float(rng.uniform(-9.0, -4.0))                          # dB just after the arrival
    B[na] += Sr * (10 ** (-lev / 10) - 1)
    B = B.astype(np.float32).astype(np.float64)
    return dict(seed=seed, dt=dt, air=air, a=a, ta=ta, h=h, trm=trm, use_t1=use_t1, T60=T60, B=B,
                n_bins=n_bins, d=ta * C, cut=None, eps=float(rng.choice([0.0, 2.0 ** -24, 1e-3, 1e-2])))


def t5_worker(args):
    seed, budget = args[:2]
    gen = args[2] if len(args) > 2 else 't5'
    try:
        S = t5_series(seed) if gen == 't5' else wide_series(seed)
        if S['dt'] < 5e-4:
            return dict(seed=seed, skipped='dt below 0.5 ms (window too long for the climb)')
        s = setup_for(S, eps=S['eps'])
        band, info = s.edt_band()
        if band is None or not all(np.isfinite(band)):
            return dict(seed=seed, skipped=info.get('refused'))
        Bb = S['B'] if S['cut'] is None else S['B'][:S['cut']]
        Tmax = 0.0 if S['cut'] is None else float(S['B'][S['cut']:].sum()) * math.exp(S['a'] * S['dt'])
        cl = Climber(Bb, S['dt'], S['ta'], S['h'], S['trm'], S['a'], S['eps'], Tmax, info['U_max'])
        rng = np.random.default_rng(seed + 7)
        # the evaluator against exact_params on this series' own arrangements
        chk = 0.0
        for _ in range(5):
            cl.start('random', rng, info['U_max'])
            p, m = cl.atoms()
            e1 = float(edt_batch(p, m)[0])
            e2 = be.exact_params(p, m, te_list=())['edt']
            if np.isfinite(e1) and np.isfinite(e2):
                chk = max(chk, abs(e1 / e2 - 1))
        # two-atom moves where they can matter: bins near the crossing range, and bins with a
        # direct-sound option (the direct / reflected split)
        near = np.array([((max(n * S['dt'], S['ta']) - S['ta']) >= info['U_min'] - 3 * S['dt'] and
                          (n * S['dt'] - S['ta']) <= info['U_max'] + 2 * S['dt']) or bool(g[2].any())
                         for n, g in zip(cl.win, cl.grids)])
        out = {}
        for mode in ('min', 'max'):
            best = None
            for st in ('start', 'end', 'half', 'half', 'random'):
                cl.start(st, rng, float(rng.uniform(info['U_min'], info['U_max'])))
                v = cl.climb(mode, rng, near, budget=budget / 10)
                if np.isfinite(v) and (best is None or (v < best if mode == 'min' else v > best)):
                    best = v
            out[mode] = best
        lo, hi = band
        ok = (out['min'] is None or out['min'] >= lo * (1 - 1e-9)) and (out['max'] is None or out['max'] <= hi * (1 + 1e-9))
        return dict(seed=seed, dt=S['dt'], air=S['air'], eps=S['eps'], cut=S['cut'], W=len(cl.win),
                    band=band, witness=info['witness'], n_initial=info['n_initial'], climbed=out, inside=ok,
                    evaluator_check=chk,
                    reach_hi=(out['max'] - lo) / (hi - lo) if out['max'] is not None else None,
                    reach_lo=(out['min'] - lo) / (hi - lo) if out['min'] is not None else None)
    except Exception:
        return dict(seed=seed, error=traceback.format_exc())


def t5(workers=8, n_series=40, budget=120.0):
    log('\n=== T5: adversarial climb on EDT: two-atom splits held on -10 dB, eps signs, late mass, tails '
        '(%d series) ===' % n_series)
    from multiprocessing import Pool
    t = time.time()
    with Pool(min(workers, 8)) as pool:
        rows = pool.map(t5_worker, [(5000 + i, budget) for i in range(n_series)], chunksize=1)
    bad = 0
    errs = 0
    chk = 0.0
    for r in rows:
        if 'error' in r:
            errs += 1
            log('ERROR seed %d\n%s' % (r['seed'], r['error']))
            continue
        if 'skipped' in r:
            log('seed %d: skipped (%s)' % (r['seed'], r['skipped']))
            continue
        chk = max(chk, r['evaluator_check'])
        bad += 0 if r['inside'] else 1
        log('seed %d dt %.1f ms air %-4s eps %-7.2g cut %-5s W %3d n0 %d band [%.5f, %.5f] witness [%.5f, %.5f] '
            'climbed [%.5f, %.5f] (reaches %.3f..%.3f of the band) %s' % (
                r['seed'], r['dt'] * 1e3, r['air'], r['eps'], r['cut'] is not None, r['W'], r['n_initial'],
                r['band'][0], r['band'][1], r['witness'][0], r['witness'][1],
                r['climbed']['min'], r['climbed']['max'], r['reach_lo'], r['reach_hi'],
                'inside' if r['inside'] else 'ESCAPED'))
    log('T5: %d climbs escaped the band, %d errors; batched evaluator vs exact_params: max relative '
        'difference %.2e; %.0f s' % (bad, errs, chk, time.time() - t))
    return dict(rows=rows, escaped=bad + errs)


# ------------------------------------------------------------------------------------------------
# T6: the exact band at a = 0, eps = 0 (independent extremiser) against the certified band
# ------------------------------------------------------------------------------------------------
def a0_series(seed):
    """Bins with no air and no rounding: exponential, random, flutter, sparse, plateau near -10 dB,
    direct-dominated. The direct sound is a step at t_a (h = 0)."""
    rng = np.random.default_rng(70000 + seed)
    kind = ['expo', 'random', 'flutter', 'sparse', 'plateau', 'direct'][seed % 6]
    dt = float(rng.choice([2e-3, 1e-3, 5e-4, 2e-4, 1e-4]))
    ta = float(rng.uniform(0.7, 20.0)) / C + int(rng.integers(0, 3)) * dt
    use_trm = bool(rng.random() < 0.4)
    trm = ta + float(rng.uniform(0, 4e-3)) if use_trm else ta
    T60 = float(rng.uniform(0.08, 1.5))
    k = 6 * math.log(10) / T60
    N = int(math.ceil((ta + 1.3 * T60) / dt)) + 2
    t0 = np.arange(N) * dt
    na = int(math.floor(ta / dt))
    base = np.where(t0 + dt > trm, np.exp(-k * np.maximum(t0 - trm, 0.0)) * dt, 0.0)
    if kind == 'expo':
        B = base.copy()
    elif kind == 'random':
        B = base * rng.lognormal(0, float(rng.choice([0.1, 0.5, 1.5])), N)
    elif kind == 'flutter':
        per = int(rng.integers(2, 12))
        B = base * 0.02
        idx = np.arange(na + 1 + int(rng.integers(0, per)), N, per)
        B[idx] += base[idx] * per
    elif kind == 'sparse':
        B = base * 1e-3
        idx = np.unique(na + 1 + rng.integers(0, max(2, int(0.05 / dt)), int(rng.integers(2, 8))))
        B[idx[idx < N]] += rng.lognormal(0, 1, int((idx < N).sum()))
    elif kind == 'plateau':
        B = base * rng.lognormal(0, 0.3, N)
        cut = na + 1 + int(rng.integers(2, max(3, int(0.02 / dt))))
        gn = int(rng.integers(2, max(3, int(0.08 / dt))))
        B[cut:cut + gn] = 0.0
    else:
        B = base * rng.lognormal(0, 0.5, N)
    B = np.where(t0 + dt > trm, B, 0.0)
    B[na] += float(rng.choice([0.02, 0.2, 1.0, 3.0, 6.0])) * B.sum()
    if kind == 'plateau' and B[cut + gn:].sum() > 0:
        S0 = B.sum()
        late = B[cut + gn:].sum()
        f = float(rng.uniform(0.97, 1.03))
        B[cut + gn:] *= 0.1 * f * (S0 - late) / (late * (1 - 0.1 * f))
    return dict(seed=seed, kind=kind, dt=dt, ta=ta, trm=(trm if use_trm else None), B=B)


def exact_band_a0(B, dt, ta, trm, nU=801):
    """The exact EDT band (to the U grid) for a = 0, eps = 0, h = 0, no tail, by the layer-cake
    argument: for a window end U in crossing bin k, J(U) is extremal with one atom per earlier bin
    (at an end of its range or at u = 0 for the max; also at clip(U/2) for the min) and two atoms in
    bin k: (E_k - phi) at the best v <= U and (phi - E_{k+1}) at U. Each extreme is built as an
    explicit arrangement and evaluated with exact_params. Returns (lo, hi) or None."""
    N = len(B)
    n = np.arange(N)
    t0, t1 = n * dt, (n + 1) * dt
    trm_ = ta if trm is None else max(trm, ta)
    g0 = np.maximum(t0, ta) - ta
    g1 = t1 - ta
    after = t1 > ta
    na = int(np.nonzero(after)[0][0])
    r0 = np.maximum(np.maximum(t0, trm_), ta) - ta
    has_r = (np.maximum(t0, trm_) < t1) & after
    z = np.zeros(N, bool)
    z[na] = True                                  # the direct sound (h = 0) sits in bin na
    S0 = float(B.sum())
    phi = 0.1 * S0
    E = np.concatenate([np.cumsum(B[::-1])[::-1], [0.0]])
    pre = float(B[:na].sum())
    cross = [k for k in range(na, N) if B[k] > 0 and has_r[k] and E[k] >= phi >= E[k + 1] and not z[k]]
    if not cross:
        return None

    def build(U, k, mode):
        hU = 0.5 * U
        idx = np.nonzero((n >= na) & (n < k) & (B > 0))[0]
        gz = g0[idx]
        opt_pos = [np.where(z[idx], 0.0, np.nan), np.where(has_r[idx], r0[idx], np.nan),
                   np.where(has_r[idx], g1[idx], np.nan)]
        opt_v = [gz, r0[idx], g1[idx]]
        if mode == 'min':
            cc = np.clip(hU, r0[idx], g1[idx])
            opt_pos.append(np.where(has_r[idx], cc, np.nan))
            opt_v.append(cc)
        # an atom at v: the bin's levels sit at E_n up to v, E_{n+1} after, so its share of J is
        # (ln E_n - ln E_{n+1}) W(g0, v) plus a constant: pick the extreme W
        V = np.stack([np.where(np.isnan(pp), np.nan, 0.5 * ((vv - hU) ** 2 - (gz - hU) ** 2))
                      for pp, vv in zip(opt_pos, opt_v)], 1)
        Vf = np.where(np.isnan(V), -np.inf if mode == 'max' else np.inf, V)
        j = np.argmax(Vf, 1) if mode == 'max' else np.argmin(Vf, 1)
        P = np.stack(opt_pos, 1)[np.arange(len(idx)), j]
        ck = [r0[k], U] + ([min(max(hU, r0[k]), U)] if mode == 'min' else [])
        wk = [0.5 * ((v - hU) ** 2 - (g0[k] - hU) ** 2) for v in ck]
        vk = ck[int(np.argmax(wk)) if mode == 'max' else int(np.argmin(wk))]
        dl = min(1e-12 * S0, 0.5 * (E[k] - phi))
        later = np.nonzero((n > k) & (B > 0))[0]
        pos = np.concatenate([[0.0], P, [vk, U], 0.5 * (r0[later] + g1[later])])
        mass = np.concatenate([[pre], B[idx], [E[k] - phi - dl, phi - E[k + 1] + dl], B[later]])
        return pos, mass

    out = {}
    for mode in ('min', 'max'):
        best = None
        for k in cross:
            lo_u, hi_u = max(r0[k], 1e-12), g1[k]
            Us = np.unique(np.concatenate([np.linspace(lo_u, hi_u, nU), [hi_u]]))
            for _round in range(4):
                vals = []
                for U in Us:
                    p, m = build(U, k, mode)
                    vals.append(be.exact_params(p, m, te_list=())['edt'])
                vals = np.array(vals)
                vals = np.where(np.isnan(vals), -np.inf if mode == 'max' else np.inf, vals)
                i = int(np.argmax(vals) if mode == 'max' else np.argmin(vals))
                if np.isfinite(vals[i]) or vals[i] == np.inf:
                    if best is None or ((vals[i] > best) if mode == 'max' else (vals[i] < best)):
                        best = float(vals[i])
                a_, b_ = Us[max(i - 1, 0)], Us[min(i + 1, len(Us) - 1)]
                Us = np.linspace(a_, b_, 101)
        out[mode] = best
    if out['min'] is None or out['max'] is None:
        return None
    return out['min'], out['max']


def t6_worker(seed):
    try:
        S = a0_series(seed)
        s = be.Setup(S['B'], S['dt'], S['ta'], t_refl_min=S['trm'])
        band, info = s.edt_band()
        row = dict(seed=seed, kind=S['kind'], dt=S['dt'], band=band, refused=info.get('refused'),
                   n_initial=info.get('n_initial'), n_eval=info.get('n_eval'), widened=info.get('widened_to_witness'))
        if band is None:
            return row
        ex = exact_band_a0(S['B'], S['dt'], S['ta'], S['trm'], nU=201)
        row['exact'] = ex
        if ex is None:
            return row
        lo, hi = band
        row['viol'] = bool(ex[0] < lo * (1 - 1e-9) or (np.isfinite(ex[1]) and ex[1] > hi * (1 + 1e-9))
                           or (ex[1] == np.inf and hi != np.inf))
        row['gap_lo'] = (ex[0] - lo) / ex[0] if np.isfinite(ex[0]) else None
        row['gap_hi'] = (hi - ex[1]) / ex[1] if (np.isfinite(ex[1]) and np.isfinite(hi)) else None
        # in L: the band's half-width against the exact band's
        if np.isfinite(ex[1]) and np.isfinite(hi):
            row['hw_L'] = (hi - lo) / (hi + lo) / be.LIMIT['edt']
            row['hw_exact_L'] = (ex[1] - ex[0]) / (ex[1] + ex[0]) / be.LIMIT['edt']
        return row
    except Exception:
        return dict(seed=seed, error=traceback.format_exc())


def t6(workers=8, n_series=240):
    log('\n=== T6: exact band at a = 0, eps = 0 (layer-cake extremiser) against the certified band '
        '(%d series) ===' % n_series)
    from multiprocessing import Pool
    t = time.time()
    with Pool(min(workers, 8)) as pool:
        rows = pool.map(t6_worker, list(range(n_series)), chunksize=1)
    errs = [r for r in rows if 'error' in r]
    done = [r for r in rows if r.get('exact') is not None and 'error' not in r]
    viol = [r for r in done if r['viol']]
    for e in errs[:5]:
        log('ERROR seed %d\n%s' % (e['seed'], e['error']))
    for r in viol:
        log('VIOLATION seed %d %s dt %.1f ms band %s exact %s' % (r['seed'], r['kind'], r['dt'] * 1e3, r['band'], r['exact']))
    gl = [r['gap_lo'] for r in done if r.get('gap_lo') is not None]
    gh = [r['gap_hi'] for r in done if r.get('gap_hi') is not None]
    extra = [r['hw_L'] - r['hw_exact_L'] for r in done if r.get('hw_L') is not None]
    log('T6: %d series, %d errors, %d with a band and an exact band, %d VIOLATIONS, %.0f s' % (
        len(rows), len(errs), len(done), len(viol), time.time() - t))
    if gl:
        log('  certified over exact, relative: lo median %.2e max %.2e | hi median %.2e max %.2e' % (
            float(np.median(gl)), max(gl), float(np.median(gh)), max(gh)))
        log('  half-width excess over the exact band, in L: median %.3f, 90th pct %.3f, max %.3f (n = %d)' % (
            float(np.median(extra)), float(np.percentile(extra, 90)), max(extra), len(extra)))
    worst = sorted([r for r in done if r.get('gap_hi') is not None],
                   key=lambda r: max(r['gap_lo'] or 0, r['gap_hi'] or 0), reverse=True)[:6]
    for r in worst:
        log('  loosest: seed %d %s dt %.1f ms band [%.6g, %.6g] exact [%.6g, %.6g] n0 %s' % (
            r['seed'], r['kind'], r['dt'] * 1e3, r['band'][0], r['band'][1], r['exact'][0], r['exact'][1],
            r['n_initial']))
    ref = {}
    for r in rows:
        if r.get('refused'):
            ref[r['refused']] = ref.get(r['refused'], 0) + 1
    log('  refusals:', ref)
    log('  EDT band widened to a witness by the rounding guard in %d of %d series' % (
        sum(1 for r in rows if r.get('widened')), len(rows)))
    return dict(rows=rows, violations=len(viol) + len(errs))


# ------------------------------------------------------------------------------------------------
# T7: the adversary's edge cases, run through the code
# ------------------------------------------------------------------------------------------------
def t7_e1():
    """The histogram on which round 1 raised ZeroDivisionError (adversary receipt, archived in
    round1/attack1_edge.json E1): U_min was 0 with no direct-sound candidate."""
    E = json.load(open(os.path.join(HERE, 'round1', 'attack1_edge.json')))['E1']
    c = [x for x in E if x.get('hit')][0]
    s = be.Setup(np.array(c['B']), c['dt'], c['ta'], half_width=c['h'], air_rate=c['a'])
    band, info = s.edt_band()
    found_min = 0.6124653979672127            # the adversary's search minimum (attack1_edge_e1_search)
    ok = band is not None and band[0] <= found_min and band[1] == np.inf and info.get('refused') == 'not_decaying'
    log('E1 histogram: band %s refused %s (%s); witness %s; the adversary found EDTs from %.4f s to 2213 s: %s' % (
        band, info.get('refused'), info.get('why'), info.get('witness'), found_min, 'OK' if ok else 'FAIL'))
    return dict(band=band, refused=info.get('refused'), why=info.get('why'), witness=info.get('witness'),
                crossing=(info.get('U_min'), info.get('U_max')), ok=ok)


def t7_lemma_w(n=4000):
    """Lemma W numerically: for random admissible curves with U in [Ua, Ub], Ub <= 2 Ua,
    G-(x) <= J(U) <= G+(x); and the adversary's E2 arrangement (x > Ua) violates only the
    precondition."""
    rng = np.random.default_rng(11)
    worst_lo = worst_hi = np.inf
    for _ in range(n):
        m = int(rng.integers(2, 9))
        pos = np.sort(rng.uniform(1e-4, 0.05, m))
        mass = rng.lognormal(0, 1.5, m)
        mass = np.concatenate([[rng.uniform(0, 3) * mass.sum()], mass])
        pos = np.concatenate([[0.0], pos])
        r = be.exact_params(pos, mass, te_list=())
        if not np.isfinite(r.get('U', np.nan)) or not r['U'] > 0:
            continue
        U = r['U']
        Ua = U * float(rng.uniform(0.5, 1.0))           # x = U - Ua <= Ua
        x = U - Ua
        S0 = mass.sum()
        phi = 0.1 * S0

        def S(u):
            return mass[(pos >= u) & (pos > 0)].sum()
        # exact integrals on the step curve
        edges = np.unique(np.concatenate([[0.0], pos[(pos > 0) & (pos <= U)], [Ua, U]]))
        J = Na = I = 0.0
        for a_, b_ in zip(edges[:-1], edges[1:]):
            lt = math.log(S(b_) / phi)
            J += lt * 0.5 * ((b_ - U / 2) ** 2 - (a_ - U / 2) ** 2)
            if b_ <= Ua:
                Na += lt * 0.5 * ((b_ - Ua / 2) ** 2 - (a_ - Ua / 2) ** 2)
                I += lt * (b_ - a_)
        lta = math.log(S(Ua) / phi)
        Gm = Na - x * I / 2
        Gp = Gm + x * Ua * lta / 2
        # J's natural scale is U^2 (|J| <= U^2 ln 10 / 4); the upper bound is attained when S is
        # constant on [Ua, U], so a relative-to-|J| test would read float noise as failure
        sc = U * U
        worst_lo = min(worst_lo, (J - Gm) / sc)
        worst_hi = min(worst_hi, (Gp - J) / sc)
    ok = worst_lo >= -1e-12 and worst_hi >= -1e-12
    # E2 (adversary): u = [0, .5, 1, 2.8, 6, 50] ms, U = 6 ms, Ua = 1 ms: x = 5 ms > Ua
    log('Lemma W on %d random curves with x <= Ua: min (J - G-)/U^2 = %.3e, min (G+ - J)/U^2 = %.3e: %s; '
        'the adversary\'s E2 has x = 5 ms > Ua = 1 ms, outside the precondition the method now enforces '
        '(every window-end interval has Ub <= 2 Ua)' % (n, worst_lo, worst_hi, 'OK' if ok else 'FAIL'))
    return dict(worst_lo=worst_lo, worst_hi=worst_hi, ok=ok)


def t7_ts_cap(n=60):
    """The Ts certificate is taken with the t at which F was evaluated, so any iteration cap covers."""
    bad = tot = 0
    for seed in range(n):
        S = random_series(2000 + seed)
        S['cut'] = None
        s = setup_for(S)
        full, _ = s.ts_band()
        if full is None:
            continue
        for cap in (1, 2, 3):
            b, _ = s.ts_band(max_iter=cap)
            tot += 1
            bad += 0 if (b[0] <= full[0] * (1 + 1e-12) and b[1] >= full[1] * (1 - 1e-12)) else 1
    log('Ts with Dinkelbach capped at 1, 2, 3 iterations: %d of %d bands fail to cover the converged band' % (bad, tot))
    return dict(bad=bad, total=tot, ok=bad == 0)


def perturbed_arrangement(S, rng, style, eps_n):
    """random_arrangement whose bin n holds an exact value B_n (1 + xi_n eps_n), xi_n = -1 or +1
    (half the bins) or uniform in [-1, 1]: one factor per bin, applied to all its atoms."""
    tau, E, direct = random_arrangement(S, rng, style)
    n = np.floor(tau / S['dt']).astype(int)
    nb = len(S['B'])
    xi = np.where(rng.random(nb) < 0.5, rng.choice([-1.0, 1.0], size=nb), rng.uniform(-1, 1, nb))
    return tau, E * (1.0 + xi[n] * eps_n[n]), direct


def t7_eps_and_unrecorded(n_series=40, n_arr=30):
    """Per-bin eps (an array) and unrecorded energy allowed inside the series (tail_t_min early):
    random admissible arrangements, every quantity checked."""
    viol = []
    checks = 0
    for seed in range(n_series):
        S = random_series(3000 + seed)
        S['cut'] = None
        rng = np.random.default_rng(3000 + seed)
        N = len(S['B'])
        eps_n = np.linspace(1e-4, 2e-2, N) * float(rng.uniform(0.2, 1.0))
        extra_rel = float(rng.choice([0.0, 0.003, 0.02, 0.06]))
        t_min = S['ta'] + float(rng.uniform(0.002, 0.08))
        t_max = N * S['dt'] + float(rng.uniform(0.0, 0.3))
        Etot = float(S['B'].sum())
        Tmax = extra_rel * Etot * math.exp(S['a'] * S['dt'])
        kw = dict(half_width=S['h'], air_rate=S['a'], t_refl_min=(S['trm'] if S['use_t1'] else None), rel_eps=eps_n)
        if Tmax > 0:
            kw.update(tail_max=Tmax, tail_t_min=t_min, tail_t_max=t_max)
        s = be.Setup(S['B'], S['dt'], S['ta'], **kw)
        res = s.all_bands(upstream=False)
        for i in range(n_arr):
            style = ['single', 'few', 'edge', 'dirichlet', 'uniform'][i % 5]
            tau, E, direct = perturbed_arrangement(S, rng, style, eps_n)
            if Tmax > 0:
                k = int(rng.integers(1, 4))
                w = rng.dirichlet(np.ones(k)) * Tmax * float(rng.choice([1.0, rng.random()]))
                tp = rng.uniform(t_min, t_max, k)
                tau = np.concatenate([tau, tp])
                E = np.concatenate([E, w])
                direct = np.concatenate([direct, np.zeros(k, bool)])
            tr = eval_fine(tau, E, direct, S['ta'])
            for q, key in QKEYS:
                b = res[q].get('band')
                x = tr.get(key)
                if b is None or x is None:
                    continue
                checks += 1
                if not np.isfinite(x):
                    if not ((x > 0 and b[1] == np.inf) or (x < 0 and b[0] == -np.inf)):
                        viol.append(dict(seed=seed, q=q, truth=repr(x), band=b))
                    continue
                if not within(b, x):
                    viol.append(dict(seed=seed, q=q, truth=x, band=b, style=style))
    log('per-bin eps and unrecorded energy inside the series: %d checks, %d outside' % (checks, len(viol)))
    for v in viol[:10]:
        log('   VIOLATION', v)
    return dict(checks=checks, violations=viol, ok=not viol)


def t7_unsupported():
    S = random_series(1234)
    s = be.Setup(S['B'], S['dt'], S['ta'], air_rate=S['a'], n_sources=2)
    r = s.all_bands(upstream=False)
    refs = [r[q].get('refused') for q in ('edt', 'ts', 'c50', 'c80', 'd50')]
    ok = all(x == 'premise_unsupported' for x in refs)
    log('two sources: every quantity refused %s: %s' % (refs[0], 'OK' if ok else 'FAIL'))
    return dict(refused=refs, ok=ok)


def t7_wide_range(n_seeds=600, n_keep=16, budget=90.0):
    """Series whose crossing range has U_max > 2 U_min (the regime of the adversary's Lemma W
    finding): the geometric cover is used, and the T5 climb must stay inside."""
    picks = []
    for seed in range(n_seeds):
        S = wide_series(seed)
        s = setup_for(S, eps=S['eps'])
        rng_, inf_ = s.crossing_range()
        if rng_ is not None and rng_[1] > 2 * rng_[0]:
            picks.append(seed)
        if len(picks) >= n_keep:
            break
    from multiprocessing import Pool
    with Pool(min(8, max(len(picks), 1))) as pool:
        rows = pool.map(t5_worker, [(p, budget, 'wide') for p in picks], chunksize=1)
    bad = 0
    for r in rows:
        if 'error' in r:
            bad += 1
            log('ERROR seed %d\n%s' % (r['seed'], r['error']))
            continue
        if 'skipped' in r:
            log('  seed %d skipped (%s)' % (r['seed'], r['skipped']))
            continue
        bad += 0 if r['inside'] else 1
        log('  seed %d dt %.1f ms air %-4s n0 %d band [%.5f, %.5f] climbed [%.5f, %.5f] %s' % (
            r['seed'], r['dt'] * 1e3, r['air'], r['n_initial'], r['band'][0], r['band'][1],
            r['climbed']['min'], r['climbed']['max'], 'inside' if r['inside'] else 'ESCAPED'))
    log('U_max > 2 U_min: %d series found in %d seeds, %d escapes' % (len(picks), n_seeds, bad))
    return dict(seeds=picks, rows=rows, ok=bad == 0)


def t7():
    log('\n=== T7: edge cases from the adversary, run through the code ===')
    out = {}
    fails = 0
    for name, f in (('e1', t7_e1), ('lemma_w', t7_lemma_w), ('ts_cap', t7_ts_cap),
                    ('eps_unrecorded', t7_eps_and_unrecorded), ('unsupported', t7_unsupported),
                    ('wide_range', t7_wide_range)):
        try:
            r = f()
        except Exception:
            r = dict(ok=False, error=traceback.format_exc())
            log('ERROR in %s\n%s' % (name, r['error']))
        out[name] = r
        fails += 0 if r.get('ok') else 1
    log('T7: %d of %d edge checks failed' % (fails, len(out)))
    return dict(checks=out, fails=fails)


# ------------------------------------------------------------------------------------------------
def main():
    argv = sys.argv[1:]
    workers = 8
    n_series = 400
    if '--workers' in argv:
        i = argv.index('--workers')
        workers = min(int(argv[i + 1]), 8)
        del argv[i:i + 2]
    if '--series' in argv:
        i = argv.index('--series')
        n_series = int(argv[i + 1])
        del argv[i:i + 2]
    which = argv or ['t1', 't2', 't3', 't4', 't5', 't6', 't7']
    import shutil
    free = shutil.disk_usage('B:/').free / 2 ** 30
    log('free space on B: %.1f GiB' % free)
    if free < 40:
        log('below 40 GiB: stopping')
        return 2
    total_fail = 0
    for w in which:
        t = time.time()
        if w == 't1':
            r = t1()
            total_fail += r['fails']
        elif w == 't2':
            r = t2()
            total_fail += r['fails']
        elif w == 't3':
            r = t3()
            total_fail += r['fails']
        elif w == 't4':
            r = t4(workers, n_series)
            total_fail += r['violations'] + len(r['errors'])
        elif w == 't5':
            r = t5(workers)
            total_fail += r['escaped']
        elif w == 't6':
            r = t6(workers)
            total_fail += r['violations']
        elif w == 't7':
            r = t7()
            total_fail += r['fails']
        else:
            raise SystemExit('unknown test %s' % w)
        with open(os.path.join(HERE, 'results_%s.json' % w), 'w') as f:
            json.dump(jsonable(r), f, indent=1)
        log('[%s done in %.0f s]' % (w, time.time() - t))
    log('\nTOTAL: %d failures' % total_fail)
    with open(os.path.join(HERE, 'test_log_%s.txt' % '_'.join(which)), 'w') as f:
        f.write('\n'.join(LOG) + '\n')
    return 1 if total_fail else 0


if __name__ == '__main__':
    sys.exit(main())
