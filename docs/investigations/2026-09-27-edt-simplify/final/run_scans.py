"""The critique's own scans (../critique/*.py constructions, same generator and truth), re-run on
final/method.py with LeanBand beside it. Single process, read-only on every input, no solver.
Usage: python run_scans.py [s1|occ|i4|i5|seeds|all]"""
import sys, math, itertools, collections, json, glob, warnings
sys.dont_write_bytecode = True
warnings.filterwarnings('ignore')
import numpy as np
HERE = 'B:/repos/I-Simpa_Night_Mode/target/agents/edt-simplify'
sys.path.insert(0, HERE + '/critique')
sys.path.insert(0, HERE + '/final')
import synth as S                       # noqa: E402
from common import CANDS, score         # noqa: E402
import method as F                      # noqa: E402

M = {'leanband': CANDS['leanband'].analyse, 'final': F.analyse}
h = 0.31 / S.C
which = sys.argv[1] if len(sys.argv) > 1 else 'all'
OUT = {}


def cover(r, truth):
    return r['edt'] is not None and r['edt_lo'] <= truth <= r['edt_hi']


if which in ('s1', 'all'):          # scan 1, deduplicated, well-conditioned (fix_probe_dedup.py grid)
    seen = set(); tal = {m: collections.defaultdict(collections.Counter) for m in M}; worst = []
    for V, T60, d, gap_ms, dt_ms in itertools.product((200.0, 2000.0, 20000.0), (0.3, 0.6, 1.0, 2.0, 3.0),
                                                       (0.7, 1.0, 1.5, 2.0, 3.0, 5.0, 8.0, 12.0, 20.0, 30.0),
                                                       (0.0, 2.0, 5.0, 10.0), (1.0, 2.0, 5.0, 10.0)):
        if d > 1.6 * V ** (1 / 3):
            continue
        dt = dt_ms * 1e-3; k = 6 * math.log(10) / T60
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
            for m, fn in M.items():
                kind, e = score(fn(b, dt, t_arr, {}), truth)
                tal[m][dt_ms][kind] += 1
                if m == 'final' and kind == 'ok_WRONG':
                    worst.append((round(100 * e, 1), V, T60, d, gap_ms, dt_ms, phase))
    OUT['scan1'] = {m: {str(k): dict(v) for k, v in per.items()} for m, per in tal.items()}
    print('SCAN 1 (I1/I2/I3, noise-free, run 2*T60, R=0.31):')
    for m, per in tal.items():
        for dt_ms in sorted(per):
            print('  %-8s %4g ms %s' % (m, dt_ms, dict(per[dt_ms])))
    print('  final worst wrong-silent:', sorted(worst, key=lambda x: -abs(x[0]))[:6])
    sys.stdout.flush()

if which in ('occ', 'all'):         # scan 4 construction (scan_i1_occluded.py), 180 rows
    tal = {m: collections.defaultdict(collections.Counter) for m in M}; worst = []
    for T60, delay_ms, dt_ms, dir_db in itertools.product((0.4, 0.8, 1.5, 3.0), (2.0, 5.0, 10.0, 20.0, 40.0),
                                                          (1.0, 2.0, 10.0), (None, -20.0, -10.0)):
        dt = dt_ms * 1e-3; k = 6 * math.log(10) / T60
        t_arr = 0.030 + 0.41 * dt; gap = delay_ms * 1e-3
        Ed = 0.0 if dir_db is None else (1.0 / k) * 10 ** (dir_db / 10)
        truth = S.truth_edt(t_arr + gap, 0.0, 0.0, [1.0], [k]) if Ed == 0 else S.truth_edt(t_arr, Ed, gap, [1.0], [k])
        b = S.histogram(dt, t_arr + gap + 2.0 * T60, t_arr, h, Ed, gap, [1.0], [k])
        for m, fn in M.items():
            kind, e = score(fn(b, dt, t_arr, {}), truth)
            tal[m][str(dir_db)][kind] += 1
            if m == 'final' and kind == 'ok_WRONG':
                worst.append((round(100 * e, 1), T60, delay_ms, dt_ms, dir_db))
    OUT['occluded'] = {m: {k: dict(v) for k, v in per.items()} for m, per in tal.items()}
    print('SCAN 4 (I1 blocked / weak direct):')
    for m, per in tal.items():
        for kk in per:
            print('  %-8s direct=%s %s' % (m, kk, dict(per[kk])))
    print('  final worst wrong-silent:', sorted(worst, key=lambda x: -abs(x[0]))[:6])
    sys.stdout.flush()

if which in ('i4', 'all'):          # scan 2b construction (scan_i4b.py)
    tal = collections.defaultdict(collections.Counter); worst = []; refused_long = collections.Counter()
    for T60, dt_ms, frac, shape, drr_db in itertools.product(
            (0.5, 1.0, 2.0, 4.0, 8.0), (1.0, 2.0, 10.0),
            (0.15, 0.2, 0.25, 0.3, 0.35, 0.4, 0.5, 0.6, 0.8, 1.0, 1.5),
            ('single', 'double-20', 'double-15', 'double-10'), (-12.0, -6.0, -3.0)):
        dt = dt_ms * 1e-3; k1 = 6 * math.log(10) / T60
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
        for m, fn in M.items():
            meta = {'half_width': 0.0}
            rl = fn(b_long, dt, t_arr, meta)
            if rl['edt'] is None or abs(rl['edt'] / truth - 1) > 0.05:
                refused_long[m] += 1
                continue                   # as scan_i4b: only rows whose long run is within 5 %
            kind, e = score(fn(b, dt, t_arr, meta), truth)
            tal[m][kind] += 1
            if m == 'final' and kind == 'ok_WRONG':
                worst.append((round(100 * e, 1), T60, dt_ms, frac, shape, drr_db))
            if m == 'final':
                tal['final_by_frac_%.2f' % frac][kind] += 1
    OUT['i4'] = {m: dict(v) for m, v in tal.items()}
    print('SCAN 2b (I4, truncated runs; rows whose own long run is within 5 %):')
    for m in M:
        print('  %-8s long-run not within 5%%: %d  truncated outcomes %s' % (m, refused_long[m], dict(tal[m])))
    for kk in sorted(k for k in tal if k.startswith('final_by')):
        print('    %s %s' % (kk, dict(tal[kk])))
    print('  final worst wrong-silent:', sorted(worst, key=lambda x: -abs(x[0]))[:6])
    sys.stdout.flush()

if which in ('i5', 'all'):          # scan 3 construction (scan_i5.py), 200 seeds per config
    CONFIGS = [(200, 0.6, 2.0, 150e3, 1.0), (200, 0.6, 5.0, 150e3, 1.0), (200, 0.6, 5.0, 150e3, 10.0),
               (2000, 1.5, 5.0, 150e3, 1.0), (2000, 1.5, 15.0, 150e3, 1.0), (2000, 1.5, 15.0, 150e3, 10.0),
               (20000, 2.5, 10.0, 150e3, 1.0), (20000, 2.5, 30.0, 150e3, 1.0), (20000, 2.5, 30.0, 150e3, 10.0),
               (20000, 2.5, 30.0, 1.5e6, 1.0), (20000, 2.5, 30.0, 15e3, 1.0), (2000, 1.5, 15.0, 15e3, 1.0)]
    rng = np.random.default_rng(20260927)
    tot = {m: collections.Counter() for m in M}; rows = []
    print('SCAN 3 (I5, random-mode Poisson hits, 200 seeds, run 1.5*T60):')
    for V, T60, d, N, dt_ms in CONFIGS:
        dt = dt_ms * 1e-3; k = 6 * math.log(10) / T60; t_arr = d / S.C
        Ed = S.sabine_direct_energy(V, T60, d, 1.0 / k)
        truth = S.truth_edt(t_arr, Ed, 0.0, [1.0], [k])
        T_run = t_arr + 1.5 * T60
        lam0 = N * math.pi * 0.31 ** 2 * S.C / V; lam_d = N * 0.31 ** 2 / (4 * d * d)
        b_dir = S.histogram(dt, T_run, t_arr, h, Ed, 1e9, [0.0], [k])
        b_rev = S.histogram(dt, T_run, t_arr, h, 0.0, 0.0, [1.0], [k])
        per = {m: collections.Counter() for m in M}; errs = {m: [] for m in M}; cov = {m: 0 for m in M}
        for s in range(200):
            b = S.noisy(b_dir, Ed / lam_d, rng) + S.noisy(b_rev, 1.0 / lam0, rng)
            for m, fn in M.items():
                r = fn(b, dt, t_arr, {})
                kind, e = score(r, truth)
                per[m][kind] += 1; tot[m][kind] += 1
                if e is not None:
                    errs[m].append(e)
                    cov[m] += cover(r, truth)
        clean = F.analyse(b_dir + b_rev, dt, t_arr, {})
        ce = None if clean['edt'] is None else clean['edt'] / truth - 1
        for m in M:
            ee = np.array(errs[m]) if errs[m] else np.array([np.nan])
            print('  V=%-5g T60=%g d=%-4g N=%.0e dt=%-2gms %-8s %s  sd=%.1f%%  truth-in-range %d/%d%s' % (
                V, T60, d, N, dt_ms, m, dict(per[m]), 100 * np.nanstd(ee), cov[m], len(errs[m]),
                ('  (noise-free final err %s)' % ('n/a' if ce is None else '%+.1f%%' % (100 * ce))) if m == 'final' else ''))
            rows.append(dict(V=V, T60=T60, d=d, N=N, dt_ms=dt_ms, m=m, tally=dict(per[m]), cover=cov[m], n=len(errs[m])))
        sys.stdout.flush()
    OUT['i5'] = dict(total={m: dict(v) for m, v in tot.items()}, rows=rows)
    print('  TOTAL', {m: dict(v) for m, v in tot.items()})

if which in ('seeds', 'all'):       # real_seeds.py construction: each seed vs the 10-seed mean, 1 ms -> 2 ms
    AG = 'B:/repos/I-Simpa_Night_Mode/target/agents'
    ALLOWED = ['C-E3', 'C-E4', 'C-E6', 'V-E2', 'V-E5', 'C-R3', 'C-R4', 'C-R6', 'V-R2', 'V-R6']
    ROOTS = (AG + '/pm8-noise-scratch/runs/noise-cal-1790307822', AG + '/pm8-noise-scratch/runs/noise-cal-1790310131')
    K = 2
    st = {m: collections.Counter() for m in M}; cov = {m: [0, 0] for m in M}; ratio = {m: [] for m in M}
    bycell = collections.defaultdict(collections.Counter); worst = []
    for root in ROOTS:
        for c in json.load(open(root + '/cells.json')):
            cc = c if 'id' in c else c['cell']
            if cc['id'] not in ALLOWED or abs(cc['time_step_s'] - 0.001) > 1e-9:
                continue
            runs = [json.load(open(p))['spps'] for p in sorted(glob.glob(f"{root}/{cc['id']}/seed*/report.json"))]
            d0 = runs[0]; hh = d0['receiver_radius_m'] / d0['speed_of_sound_m_s']
            for ri, pr0 in enumerate(d0['point_receivers']):
                for bi, b0 in enumerate(pr0['bands']):
                    ser = [np.asarray(d['point_receivers'][ri]['bands'][bi]['energy_pa2'], float) for d in runs]
                    Lm = min(len(v) for v in ser); nb = Lm // K
                    reb = [v[:nb * K].reshape(nb, K).sum(1) for v in ser]
                    mean = np.mean(reb, axis=0); t_arr = pr0['arrival_s']; dt = d0['time_step_s'] * K
                    for m, fn in M.items():
                        rm = fn(mean, dt, t_arr, {'half_width': hh})
                        if rm['edt'] is None:
                            st[m]['mean_refused'] += 1
                            continue
                        vals, sds = [], []
                        for v in reb:
                            r = fn(v, dt, t_arr, {'half_width': hh})
                            if r['edt'] is None:
                                st[m]['refused'] += 1
                                continue
                            e = r['edt'] / rm['edt'] - 1
                            vals.append(r['edt']); sds.append((r['edt_hi'] - r['edt_lo']) / 4.0 / r['edt'])
                            cov[m][0] += r['edt_lo'] <= rm['edt'] <= r['edt_hi']; cov[m][1] += 1
                            if r['status'] == 'ok':
                                kk = 'ok_WRONG' if abs(e) > 0.05 else 'ok_good'
                                st[m][kk] += 1
                                if m == 'final':
                                    bycell[cc['id']][kk] += 1
                                    if kk == 'ok_WRONG':
                                        worst.append((round(100 * e, 1), cc['id'], pr0.get('label'), b0['freq_hz']))
                            else:
                                st[m]['wide'] += 1
                                if m == 'final':
                                    bycell[cc['id']]['wide'] += 1
                        if len(vals) >= 5:
                            ratio[m].append(np.std(vals, ddof=1) / np.mean(vals) / max(np.median(sds), 1e-12))
    OUT['seeds'] = dict(stats={m: dict(v) for m, v in st.items()}, cover={m: v for m, v in cov.items()})
    print('REAL SEEDS (10 cells, each seed vs its 10-seed mean, 2 ms):')
    for m in M:
        rr = np.array(ratio[m])
        print('  %-8s %s  mean-in-range %d/%d  seed-sd / shown-sd: median %.2f p90 %.2f max %.2f' % (
            m, dict(st[m]), cov[m][0], cov[m][1], np.median(rr), np.percentile(rr, 90), rr.max()))
    print('  final by cell:', {k: dict(v) for k, v in sorted(bycell.items())})
    print('  final worst wrong-silent:', sorted(worst, key=lambda x: -abs(x[0]))[:6])

json.dump(OUT, open(HERE + '/final/scans_%s.json' % which, 'w'), default=str, indent=1)
