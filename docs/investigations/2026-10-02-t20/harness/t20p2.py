"""T20 part 2, sets S1 and S2 (../PREREG-S1S2.md, frozen). Builds the rows, runs the product's T20 through the
shim (crates/simpa-core/tests/t20_shim.rs), runs the say-NO checks first, then scores J1-a, J1-b, J1-c and J4.

Imports read-only: harness2/m8b (synth_fresh and its hash-gated critique/synth.py; ism_fresh's hash gate on _ism.py;
truth.copies() for the checked _mirror.line), ../../2026-10-02-edt-ball-vs-point/run.py (rooms, receiver and source
fractions, point_echogram). Nothing is written next to the sources.

  python -B t20p2.py s1|s2 OUT_DIR [--workers N] [--smoke N] [--sayno-only]

Outputs go to OUT_DIR/s1 or OUT_DIR/s2: rows.csv, summary.json, run.log, shim/ (bins.bin, manifest.jsonl, t20.jsonl).
--smoke N computes N rows spread over the set and labels everything SMOKE (no verdict). --sayno-only computes the
rows the say-NO checks need (S1: 1 ms; S2: units with d <= 3 m) and stops after them.

Where the PREREG did not fix a point, ../ADDENDUM-1.md rules (S1_READING; S2's `extend`).
"""
import sys

sys.dont_write_bytecode = True

import argparse  # noqa: E402
import csv  # noqa: E402
import hashlib  # noqa: E402
import importlib.util  # noqa: E402
import json  # noqa: E402
import math  # noqa: E402
import os  # noqa: E402
import subprocess  # noqa: E402
import time  # noqa: E402
from concurrent.futures import ProcessPoolExecutor  # noqa: E402
from pathlib import Path  # noqa: E402

import numpy as np  # noqa: E402

HERE = Path(__file__).resolve().parent
INV = HERE.parent                                   # docs/investigations/2026-10-02-t20
HELD = INV.parent / '2026-09-27-edt-heldout'
BVP = INV.parent / '2026-10-02-edt-ball-vs-point'
REPO = INV.parents[2]
sys.path.insert(0, str(HELD / 'harness2'))
sys.path.insert(0, str(HERE))
from m8b import ism_fresh, synth_fresh  # noqa: E402
import truth20  # noqa: E402

PREREG = INV / 'PREREG-S1S2.md'
SHIM_SRC = REPO / 'crates' / 'simpa-core' / 'tests' / 't20_shim.rs'
DECAY_SRC = REPO / 'crates' / 'simpa-core' / 'src' / 'params' / 'decay.rs'
TARGET_DIR = r'C:\tmp\nm-target-t20p2'

DT_FINE = 2e-5                       # round 2's fine grid (ism_fresh.py:206)
STEPS_MS = (1, 2, 5)
RADII = (0.1, 0.31, 0.5)
CLOSE = 0.005                        # J1-b, J1-c: 0.5 %
WRONG = 0.05                         # wrong-silent: 5 %
J1A_MAX = 0.005                      # J1-a: wrong-silent <= 0.5 % of answered
J1B_MIN = 0.99
J1C_MIN = 0.99
J4_MIN = 0.90
T60_COVERED = 3.0
CONV_TOL = 5e-4                      # truths at the 1.5x and 2.5x cuts agree to 0.05 %
INCONCLUSIVE_SHARE = 0.10
ECHO_FACTOR, CUT_FACTOR = 2.5, 1.5

# ---- S1 -------------------------------------------------------------------------------------------------------------
S1_SEED = 20261002
S1_DRAWS = 400
D_MIN_M = 0.7                        # ADDENDUM-1 item 2: d < 0.7 m redrawn
S1_READING = {                       # ADDENDUM-1 items 1-3
    'slopes': 'double slope only (synth_fresh RATIOS 1.5, 5); single exponentials are say-NO (e)',
    'cells': '400 draws = 200 per ratio, half of each delayed, in synth_fresh._draw\'s per-row order; '
             'the drawn R is consumed from the stream and not used; the R grid gives the half-width; every draw '
             'is run at every step and grid R',
    'd_floor': 'd redrawn while d < 0.7 m, counted as d_near_source',
    'variants': 'long: the truth\'s series, to 2.5 x the slow slope\'s T60 (ratio * T60), read by the product too; '
                'J1-a, J1-b, J4. short: the generator\'s own run, t_arrival + f * T60 (f in 0.3-3); J1-a only. '
                'Truth converged against its cut at 1.5 x',
    'complete': 'EnergySeries::new (not complete), early reverberation unresolved, no floor, no lost share',
}
VARIANTS = ('long', 'short')


def s1_draws():
    """S1's 400 parameter draws (S1_READING['cells'], ['d_floor'])."""
    sf = synth_fresh
    rng = np.random.default_rng(np.random.SeedSequence(S1_SEED))
    rej = {}
    out = []
    per = S1_DRAWS // len(sf.RATIOS)
    for ratio in sf.RATIOS:
        delayed = rng.permutation(np.arange(per) < per // 2)
        for i in range(per):
            t60 = sf._log_uniform(rng, sf.T60_S)
            late = sf._uniform(rng, sf.LATE_SHARE_DB)
            drr = sf._uniform(rng, sf.DRR_DB)
            R = sf._log_uniform(rng, sf.R_M)
            d = sf._uniform(rng, sf.D_M)
            while d < D_MIN_M:
                rej['d_near_source'] = rej.get('d_near_source', 0) + 1
                d = sf._uniform(rng, sf.D_M)
            gap = sf._uniform(rng, sf.GAP_MS)
            delay = sf.DELAY_MS[1] * (1.0 - rng.random()) if delayed[i] else 0.0
            f = sf._uniform(rng, sf.RUN_OVER_T60)
            out.append(dict(draw='%g|%03d' % (ratio, i), ratio=ratio, t60_s=t60, late_share_db=late, drr_db=drr,
                            R_drawn=R, d_m=d, gap_ms=gap, delay_ms=delay, run_over_t60=f))
    return out, rej


def s1_spec(p, R, step_ms, variant='short'):
    """long: run_s = t_arrival + 2.5 x ratio x T60, the truth's series (s1_fine) after the emission's empty steps."""
    f = ECHO_FACTOR * p['ratio'] if variant == 'long' else p['run_over_t60']
    return synth_fresh.make_spec(id='s1|%s|R%g|%gms|%s' % (p['draw'], R, step_ms, variant), ratio=p['ratio'],
                                 step_ms=step_ms, t60_s=p['t60_s'], late_share_db=p['late_share_db'],
                                 drr_db=p['drr_db'], R_m=R, d_m=p['d_m'], gap_ms=p['gap_ms'], delay_ms=p['delay_ms'],
                                 run_over_t60=f)


def s1_fine(spec):
    """The ball's fine echogram of the undelayed row, split (direct, reflected), to 2.5 x the slow slope's T60."""
    S = synth_fresh.generator()
    t = spec['d_m'] / synth_fresh.C
    t60_slow = spec['t60_s'] * spec['ratio']
    T = t + ECHO_FACTOR * t60_slow
    direct = S.histogram(DT_FINE, T, t, spec['half_width'], spec['Ed'], spec['gap_ms'] * 1e-3, [0.0], [1.0])
    refl = S.histogram(DT_FINE, T, t, spec['half_width'], 0.0, spec['gap_ms'] * 1e-3, spec['A'], spec['k'])
    n_cut = int(round((t + CUT_FACTOR * t60_slow) / DT_FINE))
    return direct, refl, t, n_cut


def s1_unit(job):
    """One (draw, R): its truths, and the product's series at each step asked for."""
    p, R, steps, variants, planted = job
    t0 = time.time()
    spec1 = s1_spec(p, R, steps[0])
    direct, refl, t, n_cut = s1_fine(spec1)
    tr, why = truth20.t20(direct, refl, t, DT_FINE)
    tc, _ = truth20.t20(direct[:n_cut], refl[:n_cut], t, DT_FINE)
    plant_a = truth20.t20(direct, refl, t, DT_FINE, top_db=0.0, bottom_db=-20.0)[0] if planted else None
    rows = []
    for step_ms, variant in ((s, v) for s in steps for v in variants):
        spec = s1_spec(p, R, step_ms, variant)
        rows.append(dict(set='S1', id=spec['id'], unit=p['draw'], variant=variant, ratio=p['ratio'], R=R,
                         step_ms=step_ms, t60=p['t60_s'], t60_slow=p['t60_s'] * p['ratio'], d=p['d_m'],
                         delay_ms=p['delay_ms'], drr_db=p['drr_db'], late_share_db=p['late_share_db'],
                         gap_ms=p['gap_ms'], run_over_t60=spec['run_over_t60'], dt=spec['dt'], arrival=spec['t_arrival'],
                         half_width=spec['half_width'], truth=tr, truth_why=why, truth_cut=tc,
                         truth_point=tr, planted_a=plant_a, double_slope=True,
                         bins=synth_fresh.histogram(spec)))
    return rows, time.time() - t0, len(refl)


# ---- S2 -------------------------------------------------------------------------------------------------------------
S2_SEED = 20261003
S2_BOXES = 30
SIDES_M = (3.0, 25.0)
ALPHA = (0.03, 0.5)
T60_CAP = 3.0
BANDS_HZ = (1000, 8000)
_RUN = None


def bvp():
    """../2026-10-02-edt-ball-vs-point/run.py, imported read-only."""
    global _RUN
    if _RUN is None:
        spec = importlib.util.spec_from_file_location('bvp_run', BVP / 'run.py')
        _RUN = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(_RUN)
    return _RUN


def eyring_1k(L, a6, ism):
    """Eyring T60 at 1 kHz: run.py's eyring_t60 with ISO 9613-1 air at 1 kHz added (4 m V)."""
    Lx, Ly, Lz = L
    areas = [Ly * Lz, Ly * Lz, Lx * Lz, Lx * Lz, Lx * Ly, Lx * Ly]
    S = sum(areas)
    V = Lx * Ly * Lz
    abar = sum(a * s for a, s in zip(a6, areas)) / S
    return 0.161 * V / (-S * math.log(1 - abar) + 4 * ism.m_energy(1000.0) * V)


def s2_rooms():
    ism = ism_fresh.generator()
    run = bvp()
    rooms = [(name, tuple(float(x) for x in L), [float(a) for a in a6]) for name, (L, a6) in run.ROOMS.items()]
    rng = np.random.default_rng(np.random.SeedSequence(S2_SEED))
    rejected = 0
    while len(rooms) < len(run.ROOMS) + S2_BOXES:
        L = tuple(float(x) for x in rng.uniform(*SIDES_M, 3))
        a6 = [float(x) for x in rng.uniform(*ALPHA, 6)]
        if eyring_1k(L, a6, ism) > T60_CAP:
            rejected += 1
            continue
        rooms.append(('B%02d' % (len(rooms) - len(run.ROOMS)), L, a6))
    return rooms, rejected


def s2_units():
    run = bvp()
    ism = ism_fresh.generator()
    rooms, rejected = s2_rooms()
    units, excluded = [], {}
    for name, L, a6 in rooms:
        src = tuple(f * l for f, l in zip(run.SRC_FRAC, L))
        t60 = eyring_1k(L, a6, ism)
        for ri, fr in enumerate(run.REC_FRACS):
            rec = tuple(f * l for f, l in zip(fr, L))
            d = math.dist(src, rec)
            for R in RADII:
                if d < 2.0:
                    excluded['near_source'] = excluded.get('near_source', 0) + 1
                    continue
                if R >= min(min(x, l - x) for x, l in zip(rec, L)):
                    excluded['ball_crosses_wall'] = excluded.get('ball_crosses_wall', 0) + 1
                    continue
                units.append(dict(room=name, L=L, a6=a6, src=src, rec=rec, rec_i=ri, R=R, d=d, t60=t60))
    return units, dict(box_rejections_t60_over_3s=rejected, excluded=excluded, rooms=len(rooms))


def s2_unit(job):
    u, bands, steps = job
    ism = ism_fresh.generator()
    run = bvp()
    t0 = time.time()
    C = ism_fresh.C
    dl = C * DT_FINE
    alpha = [(u['a6'][0], u['a6'][1]), (u['a6'][2], u['a6'][3]), (u['a6'][4], u['a6'][5])]
    lmax = C * ECHO_FACTOR * u['t60']
    bd, br = ism.echogram(u['L'], u['src'], u['rec'], u['R'], alpha, lmax, dl)
    pd, pr = run.point_echogram(u['L'], u['src'], u['rec'], u['R'], alpha, lmax, dl)
    assert len(bd) == len(pd)
    nb = len(bd)
    n_cut = int(math.ceil(C * CUT_FACTOR * u['t60'] / dl))
    t = u['d'] / C
    rows = []
    uid = '%s|rec%d|R%g' % (u['room'], u['rec_i'], u['R'])
    for F in bands:
        m = ism.m_energy(float(F))
        ac = ism.air_factor(nb, dl, m, C, None)                     # continuous air for the truths (make_row)
        b_d, b_r, p_d, p_r = bd * ac, br * ac, pd * ac, pr * ac
        # ADDENDUM-1 item 4: each cut extended by its own fitted tail; truth and product read the 2.5 x one.
        tail_b, fit_b = extend(b_d + b_r, nb)
        tail_bc, fit_bc = extend(b_d + b_r, n_cut)
        tail_p, fit_p = extend(p_d + p_r, nb)
        tail_pc, fit_pc = extend(p_d + p_r, n_cut)
        tb, why = truth_ext(b_d, b_r, nb, tail_b, t)
        tbc, _ = truth_ext(b_d, b_r, n_cut, tail_bc, t)
        tp, _ = truth_ext(p_d, p_r, nb, tail_p, t)
        tpc, _ = truth_ext(p_d, p_r, n_cut, tail_pc, t)
        t_refl = truth_ext(b_d, b_r, nb, tail_b, t, anchor='reflected')[0] if u['d'] <= 3.0 else None
        for step_ms in steps:
            k = int(round(step_ms * 1e-3 / DT_FINE))
            vc = (bd + br) * ism.air_factor(nb, dl, m, C, k * DT_FINE)   # air per step for the product (make_row)
            vc = np.concatenate([vc, tail_b])
            n = len(vc) // k
            vb = vc[:n * k].reshape(n, k).sum(1)
            nz = np.nonzero(vb > 0)[0]
            vb = vb[:int(nz[-1]) + 1]
            rows.append(dict(set='S2', id='s2|%s|%dHz|%gms' % (uid, F, step_ms), unit=uid, room=u['room'],
                             rec_i=u['rec_i'], R=u['R'], band_hz=F, step_ms=step_ms, t60=u['t60'], d=u['d'],
                             dt=k * DT_FINE, arrival=t, half_width=u['R'] / C, truth=tb, truth_why=why,
                             truth_cut=tbc, truth_point=tp, truth_point_cut=tpc, truth_reflected_anchor=t_refl,
                             tail_t60=fit_b['t60'], tail_t60_cut=fit_bc['t60'], tail_share=fit_b['share'],
                             tail_fit_ok=all(f['ok'] for f in (fit_b, fit_bc, fit_p, fit_pc)),
                             bins=vb))
    return rows, time.time() - t0, nb


TAIL_FIT_DB = 10.0       # ADDENDUM-1 item 4: the last 10 dB of the cut series' Schroeder curve
TAIL_DEPTH_DB = 60.0     # the tail runs to 60 dB below the energy at the cut


def extend(e, n):
    """The fitted exponential tail of the series e[:n] (ADDENDUM-1 item 4), as fine bins after bin n.

    A Schroeder curve of a cut series falls to -inf at the cut, so "its last 10 dB" is read on the curve the tail
    completes: S(t) = (energy of e[:n] after t) + M, M the tail's energy. The fit is least squares of 10 lg S on time
    over the edges where S is within 10 dB of S at the cut (= M), and M is the fitted line's value at the cut: a fixed
    point: a root in x = 10 lg M of f(x) = (the fitted line's level at the cut) - x, which is positive for a
    negligible tail (the line through the curve's dive lies above its end) and negative for a dominant one (the
    curve is convex, the line ends below it); the largest root is taken (below). Exact for an exponential (its tail is the curve's own continuation).
    The tail is M's exponential at the fitted rate, integrated per fine bin, for as long as the energy takes to fall
    60 dB (one fitted T60). Returns (tail, info); info['ok'] False when no root is found, or the slope is not negative, and the row is then counted truth_truncated.
    """
    e = np.asarray(e[:n], float)
    dt = DT_FINE
    S = np.concatenate([np.cumsum(e[::-1])[::-1], [0.0]])
    tt = np.arange(n + 1) * dt
    T = n * dt

    def fit(x):
        L = 10 * np.log10(S + 10 ** (x / 10))
        w = L <= x + TAIL_FIT_DB
        s1, s0 = np.polyfit(tt[w], L[w], 1)
        return s0 + s1 * T - x, s1

    # f can change sign more than once on a structured late echogram (a fit on the dive alone gives a spurious
    # root at a negligible tail); the root taken is the largest, found scanning down from 100 dB above the total in
    # 5 dB steps to 100 dB below the last bin's energy, then bisected to 1e-10 dB.
    # The scan's floor is the smallest positive S: a point echogram's last fine bins can be empty (S = 0 there).
    pos = S[:n][S[:n] > 0]
    if len(pos) == 0:
        return np.zeros(0), dict(ok=False, t60=float('nan'), share=float('nan'))
    top, bottom = 10 * math.log10(S[0]) + 100.0, 10 * math.log10(pos.min()) - 100.0
    ok = fit(top)[0] < 0
    hi = top
    lo = None
    while ok and hi - 5.0 >= bottom:
        if fit(hi - 5.0)[0] > 0:
            lo = hi - 5.0
            break
        hi -= 5.0
    ok = ok and lo is not None
    if ok:
        while hi - lo > 1e-10:
            mid = 0.5 * (lo + hi)
            if fit(mid)[0] > 0:
                lo = mid
            else:
                hi = mid
    else:
        lo = hi = top
    x = 0.5 * (lo + hi)
    s1 = fit(x)[1]
    k = -s1 * math.log(10) / 10
    M = 10 ** (x / 10)
    ok = bool(ok and k > 0)
    t60 = 6 * math.log(10) / k if k > 0 else float('nan')
    nt = int(math.ceil(TAIL_DEPTH_DB / 10 * math.log(10) / k / dt)) if ok else 0
    j = np.arange(nt + 1) * dt
    tail = M * -np.diff(np.exp(-k * j)) if ok else np.zeros(0)
    return tail, dict(ok=bool(ok), t60=t60, share=M / (S[0] + M) if ok else float('nan'))


def truth_ext(direct, refl, n, tail, t, anchor='total'):
    """truth20.t20 on the series cut at n with its tail appended to the reflected part."""
    r = np.concatenate([refl[:n], tail])
    d = np.concatenate([direct[:n], np.zeros(len(tail))])
    return truth20.t20(d, r, t, DT_FINE, anchor=anchor)


# ---- the shim -------------------------------------------------------------------------------------------------------
def sha256(p):
    return hashlib.sha256(Path(p).read_bytes().replace(b'\r\n', b'\n')).hexdigest()


def run_shim(entries, folder, say):
    """entries: [(id, dt, arrival, half_width, bins)] -> {id: shim record}."""
    folder.mkdir(parents=True, exist_ok=True)
    off = 0
    with open(folder / 'bins.bin', 'wb') as fb, open(folder / 'manifest.jsonl', 'w', encoding='utf-8') as fm:
        for (i, dt, arr, hw, bins) in entries:
            b = np.ascontiguousarray(bins, dtype='<f8').tobytes()
            fb.write(b)
            fm.write(json.dumps(dict(id=i, dt=dt, arrival=arr, half_width=hw, offset=off, n=len(bins))) + '\n')
            off += len(b)
    env = dict(os.environ, T20_SHIM_DIR=str(folder))
    env.setdefault('CARGO_TARGET_DIR', TARGET_DIR)
    t0 = time.time()
    p = subprocess.run(['cargo', 'test', '-p', 'simpa-core', '--test', 't20_shim', '--release', '--', '--ignored',
                        '--nocapture', '--exact', 't20_shim'], cwd=REPO, env=env, capture_output=True, text=True)
    if p.returncode != 0 or 'T20 SHIM:' not in p.stdout:
        raise RuntimeError('shim failed:\n%s\n%s' % (p.stdout[-3000:], p.stderr[-3000:]))
    say('shim: %d rows in %.1f s' % (len(entries), time.time() - t0))
    out = {}
    for line in (folder / 't20.jsonl').read_text(encoding='utf-8').splitlines():
        r = json.loads(line)
        out[r['id']] = r
    return out


# ---- scoring --------------------------------------------------------------------------------------------------------
def truth_status(r):
    if r.get('tail_fit_ok') is False:          # S2: a tail fit that did not settle cannot show convergence
        return 'truth_truncated'
    for k in ('truth', 'truth_cut'):
        if not math.isfinite(r[k]):
            return 'truth_nan'
    return 'ok' if abs(r['truth_cut'] / r['truth'] - 1) <= CONV_TOL else 'truth_truncated'


def j1b_share(rows, value='t20', step=1):
    a = [r for r in rows if r['step_ms'] == step and r[value] is not None and r['status'] == 'ok']
    ok = sum(1 for r in a if abs(r[value] / r['truth'] - 1) <= CLOSE)
    return (ok / len(a) if a else float('nan')), ok, len(a)


def score(rows, s2, j1a_only=False):
    """j1a_only: S1's short variant (ADDENDUM-1 item 3), J1-a and the refusals only."""
    scored = [r for r in rows if r['status'] == 'ok']
    out = dict(rows=len(rows), status={s: sum(1 for r in rows if r['status'] == s)
                                       for s in sorted({r['status'] for r in rows})})
    trunc = out['status'].get('truth_truncated', 0) / len(rows) if rows else float('nan')
    out['truth_truncated_share'] = trunc
    out['inconclusive_truncation'] = bool(trunc > INCONCLUSIVE_SHARE)
    cells = {}
    for R in RADII:
        for s in STEPS_MS:
            a = [r for r in scored if r['R'] == R and r['step_ms'] == s and r['t20'] is not None]
            w = [r for r in a if abs(r['t20'] / r['truth'] - 1) > WRONG]
            cells['R%g|%gms' % (R, s)] = dict(answered=len(a), wrong_silent=len(w),
                                               share=(len(w) / len(a)) if a else 0.0,
                                               worst=max((abs(r['t20'] / r['truth'] - 1) for r in a), default=None),
                                               wrong_ids=[r['id'] for r in w][:20])
    out['J1a'] = dict(cells=cells, pass_=all(c['share'] <= J1A_MAX for c in cells.values()))
    ref = {}
    for r in rows:
        if r['t20'] is None:
            key = '%s|%s|%gms' % (r['reason'], r['status'], r['step_ms'])
            ref[key] = ref.get(key, 0) + 1
    out['refusals_by_reason_status_step'] = dict(sorted(ref.items()))
    if j1a_only:
        return out
    j1b = {}
    for s in STEPS_MS:
        sh, ok, n = j1b_share(rows, step=s)
        j1b['%gms' % s] = dict(share=sh, within=ok, answered=n)
    out['J1b'] = dict(by_step=j1b, pass_=bool(j1b['1ms']['answered'] and j1b['1ms']['share'] >= J1B_MIN))
    g = [r for r in scored if r['step_ms'] == 1 and r['t60'] <= T60_COVERED and r['R'] <= 0.5]
    ans = sum(1 for r in g if r['t20'] is not None)
    out['J4'] = dict(rows=len(g), answered=ans, share=(ans / len(g)) if g else float('nan'),
                     pass_=bool(g and ans / len(g) >= J4_MIN))
    if s2:
        units = {}
        for r in rows:
            if r['R'] <= 0.5 and r['step_ms'] == STEPS_MS[0]:
                units[(r['unit'], r['band_hz'])] = r
        conv = [r for r in units.values() if r['status'] == 'ok' and math.isfinite(r['truth_point'])
                and abs(r['truth_point_cut'] / r['truth_point'] - 1) <= CONV_TOL]
        e = [(abs(r['truth'] / r['truth_point'] - 1), r['unit'], r['band_hz']) for r in conv]
        within = sum(1 for x in e if x[0] <= CLOSE)
        out['J1c'] = dict(rows=len(units), converged=len(conv), within=within,
                          share=(within / len(conv)) if conv else float('nan'),
                          worst=max(e) if e else None, pass_=bool(conv and within / len(conv) >= J1C_MIN))
    return out


# ---- say-NO ---------------------------------------------------------------------------------------------------------
def say_no_e():
    """(e): the truth wrapper on a pure exponential returns its T to 1e-6 (direct impulse included and not)."""
    worst = 0.0
    for T in (0.1, 0.7, 3.0, 10.0):
        k = 6 * math.log(10) / T
        n = int(round(3.0 * T / DT_FINE))          # -180 dB: nothing missing that the fit can see
        e = np.arange(n + 1) * DT_FINE
        t = 3 * DT_FINE + 0.3 * DT_FINE
        refl = np.zeros(n)
        lo = np.maximum(e[:-1], t)
        refl = np.where(e[1:] > t, (np.exp(-k * (lo - t)) - np.exp(-k * (e[1:] - t))) / k, 0.0)
        for ed in (0.0, 0.5 / k):
            direct = np.zeros(n)
            direct[3] = ed
            got = truth20.t20(direct, refl, t, DT_FINE)[0]
            worst = max(worst, abs(got / T - 1))
    return dict(worst_rel=worst, pass_=bool(worst <= 1e-6))


def say_no(rows, shim_late, s2, say):
    res = {'e': say_no_e()}
    if not s2:      # S1: the long variant, which J1-b is scored on (ADDENDUM-1 item 3)
        rows = [r for r in rows if r['variant'] == 'long']
    ok = [r for r in rows if r['status'] == 'ok']
    planted = [dict(r, t20_planted=(r['t20'] * 1.02 if r['t20'] is not None else None)) for r in ok]
    sh, n_ok, n = j1b_share(planted, value='t20_planted')
    res['b'] = dict(share=sh, within=n_ok, answered=n, pass_=bool(n and sh < J1B_MIN))
    if not s2:
        dbl = [r for r in ok if r['double_slope'] and r['planted_a'] is not None and math.isfinite(r['planted_a'])]
        sh, n_ok, n = j1b_share(dbl, value='planted_a')
        res['a'] = dict(share=sh, within=n_ok, rows=n, pass_=bool(n and sh < J1B_MIN))
        diff = [(r['id'], r['t20'], shim_late.get(r['id'])) for r in rows if r['step_ms'] == 1]
        moved = [x for x in diff if x[2] is not None and x[1] != x[2]['t20']]
        res['d'] = dict(rows=len(diff), moved=len(moved), example=moved[0] if moved else None,
                        pass_=bool(moved))
    else:
        near = [r for r in rows if r['d'] <= 3.0 and r['step_ms'] == 1 and r['truth_reflected_anchor'] is not None
                and math.isfinite(r['truth'])]
        dev = [abs(r['truth_reflected_anchor'] / r['truth'] - 1) for r in near]
        res['c'] = dict(rows=len(near), over_half_percent=sum(1 for x in dev if x > CLOSE),
                        max_rel=max(dev) if dev else None, pass_=bool(dev and max(dev) > CLOSE))
    res['all_pass'] = all(v['pass_'] for k, v in res.items() if isinstance(v, dict))
    say('say-NO: %s' % json.dumps({k: v['pass_'] for k, v in res.items() if isinstance(v, dict)}))
    return res


# ---- driver ---------------------------------------------------------------------------------------------------------
def spread(n, total):
    return sorted({int(round(i * (total - 1) / max(n - 1, 1))) for i in range(n)}) if n < total else list(range(total))


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('set', choices=('s1', 's2'))
    ap.add_argument('out')
    ap.add_argument('--workers', type=int, default=4)
    ap.add_argument('--smoke', type=int, default=0)
    ap.add_argument('--sayno-only', action='store_true')
    a = ap.parse_args()
    s2 = a.set == 's2'
    out = Path(a.out) / a.set
    if a.smoke:
        out = out / 'smoke'
    elif a.sayno_only:
        out = out / 'sayno'
    out.mkdir(parents=True, exist_ok=True)
    log = open(out / 'run.log', 'a', encoding='utf-8')

    def say(msg):
        line = '%s %s' % (time.strftime('%H:%M:%S'), msg)
        print(line, flush=True)
        log.write(line + '\n')
        log.flush()

    mode = 'SMOKE' if a.smoke else ('SAYNO-ONLY' if a.sayno_only else 'SCORED')
    hashes = dict(prereg=sha256(PREREG), shim=sha256(SHIM_SRC), decay_rs=sha256(DECAY_SRC),
                  harness=sha256(__file__), truth20=sha256(HERE / 'truth20.py'))
    say('%s %s -> %s; hashes %s' % (mode, a.set, out, json.dumps(hashes)))
    meta = {}
    if not s2:
        draws, rej = s1_draws()
        t60s = [p['t60_s'] for p in draws]
        meta = dict(d_rejections=rej, t60_realised=[min(t60s), max(t60s)], reading=S1_READING)
        steps = (1,) if a.sayno_only else STEPS_MS
        variants = ('long',) if a.sayno_only else VARIANTS
        allrows = [(p, R, s, v) for p in draws for R in RADII for s in steps for v in variants]
        pick = [allrows[i] for i in spread(a.smoke, len(allrows))] if a.smoke else allrows
        byunit = {}
        for p, R, s, v in pick:
            e = byunit.setdefault((p['draw'], R), (p, R, set(), set()))
            e[2].add(s)
            e[3].add(v)
        jobs = [(p, R, tuple(sorted(ss)), tuple(v for v in VARIANTS if v in vv), True)
                for p, R, ss, vv in byunit.values()]
        fn = s1_unit
    else:
        units, meta = s2_units()
        if a.sayno_only:
            units = [u for u in units if u['d'] <= 3.0]
        allrows = [(i, F, s) for i in range(len(units)) for F in BANDS_HZ for s in STEPS_MS]
        pick = [allrows[i] for i in spread(a.smoke, len(allrows))] if a.smoke else allrows
        byunit = {}
        for i, F, s in pick:
            e = byunit.setdefault(i, (units[i], set(), set()))
            e[1].add(F)
            e[2].add(s)
        jobs = [(u, tuple(sorted(F)), tuple(sorted(S))) for u, F, S in byunit.values()]
        jobs.sort(key=lambda j: -j[0]['t60'] ** 3 / (j[0]['L'][0] * j[0]['L'][1] * j[0]['L'][2]))  # heavy first
        fn = s2_unit
        meta['units'] = len(units)
    say('meta %s' % json.dumps(meta))
    rows, timing = [], []
    t0 = time.time()
    with ProcessPoolExecutor(a.workers) as ex:
        for i, (res, sec, nfine) in enumerate(ex.map(fn, jobs), 1):
            rows.extend(res)
            timing.append(dict(unit=res[0]['unit'], seconds=sec, fine_bins=nfine))
            say('%d/%d %s %.1f s' % (i, len(jobs), res[0]['unit'], sec))
    if a.smoke:     # a unit's bands and steps are the union over its picked rows: keep only the rows picked
        keep = set()
        if s2:
            for i, F, s in pick:
                u = units[i]
                keep.add('s2|%s|rec%d|R%g|%dHz|%gms' % (u['room'], u['rec_i'], u['R'], F, s))
        else:
            for p, R, s, v in pick:
                keep.add('s1|%s|R%g|%gms|%s' % (p['draw'], R, s, v))
        rows = [r for r in rows if r['id'] in keep]
    say('rows built: %d in %.1f s' % (len(rows), time.time() - t0))
    entries = [(r['id'], r['dt'], r['arrival'], r['half_width'], r['bins']) for r in rows]
    late = []
    if not s2:      # say-NO (d): the arrival one step late, 1 ms rows
        late = [(r['id'] + '|late', r['dt'], r['arrival'] + r['dt'], r['half_width'], r['bins'])
                for r in rows if r['step_ms'] == 1 and r['variant'] == 'long']
    shim = run_shim(entries + late, out / 'shim', say)
    for r in rows:
        s = shim[r['id']]
        r['t20'] = s['t20']
        r['reason'] = '' if s['t20'] is not None else (s['why'] or s['code'])
        r['decay_arrival'] = s['decay_arrival']
        r['status'] = truth_status(r)
        r['err'] = (r['t20'] / r['truth'] - 1) if (r['t20'] is not None and math.isfinite(r['truth'])) else None
    shim_late = {k[:-5]: v for k, v in shim.items() if k.endswith('|late')}
    sn = say_no(rows, shim_late, s2, say)
    summary = dict(mode=mode, set=a.set, hashes=hashes, meta=meta, say_no=sn, timing=timing,
                   seconds_total=time.time() - t0)
    if not a.sayno_only:
        if s2:
            sc = score(rows, True)
            summary['score'] = sc
            gates = {g: sc[g]['pass_'] for g in ('J1a', 'J1b', 'J4', 'J1c')}
        else:
            sc = score([r for r in rows if r['variant'] == 'long'], False)
            sh = score([r for r in rows if r['variant'] == 'short'], False, j1a_only=True)
            summary['score'] = dict(long=sc, short=sh)
            gates = {'J1a_long': sc['J1a']['pass_'], 'J1a_short': sh['J1a']['pass_'], 'J1b_long': sc['J1b']['pass_'],
                     'J4_long': sc['J4']['pass_']}
        if mode == 'SCORED':
            if not sn['all_pass']:
                v = 'INCONCLUSIVE (say-NO)'
            elif sc['inconclusive_truncation']:
                v = 'INCONCLUSIVE (truth_truncated > 10 %)'
            else:
                v = 'PASS' if all(gates.values()) else 'FAIL'
            summary['verdict'] = v
            say('VERDICT %s %s' % (v, json.dumps(gates)))
    keys = sorted({k for r in rows for k in r if k != 'bins'})
    with open(out / 'rows.csv', 'w', newline='', encoding='utf-8') as f:
        w = csv.DictWriter(f, fieldnames=keys + ['n_bins'])
        w.writeheader()
        for r in rows:
            w.writerow(dict({k: r.get(k) for k in keys}, n_bins=len(r['bins'])))
    (out / 'summary.json').write_text(json.dumps(summary, indent=1, default=str), encoding='utf-8')
    say('done: %s' % (out / 'summary.json'))


if __name__ == '__main__':
    main()
