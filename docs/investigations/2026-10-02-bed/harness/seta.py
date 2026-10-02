"""The bed's set A (../PREREG.md; ../ADDENDUM-1.md items 1-6): exact inputs, no solver runs. The product's own
T20, T30, Ts, C50, C80, D50 and SPL (crates/simpa-core/tests/bed_shim.rs) on S1's closed-form decays and S2's
image-source ball echograms, against the truths of truthbed.py (C, D, Ts, SPL) and T20 part 2's truth20 (T20, T30).

Imports read-only: ../../2026-10-02-t20/harness/t20p2.py (S1 draws, S2 units, `extend`, the shim's file format) and
through it harness2/m8b (synth_fresh, ism_fresh, truth20's checked line). Nothing is written next to the sources.

  python -B seta.py s1|s2 OUT_DIR [--workers N] [--smoke N]

Outputs go to OUT_DIR/s1 or OUT_DIR/s2: rows.csv, summary.json, run.log, shim/ (bins.bin, manifest.jsonl, bed.jsonl);
`python -B seta.py summary OUT_DIR` then writes OUT_DIR/summary.json over both. --smoke N: N units, labelled SMOKE.
"""
import sys

sys.dont_write_bytecode = True

import argparse  # noqa: E402
import csv  # noqa: E402
import hashlib  # noqa: E402
import json  # noqa: E402
import math  # noqa: E402
import os  # noqa: E402
import subprocess  # noqa: E402
import time  # noqa: E402
from concurrent.futures import ProcessPoolExecutor  # noqa: E402
from pathlib import Path  # noqa: E402

import numpy as np  # noqa: E402

HERE = Path(__file__).resolve().parent
BED = HERE.parent
T20H = BED.parent / '2026-10-02-t20' / 'harness'
REPO = BED.parents[2]
sys.path.insert(0, str(T20H))
sys.path.insert(0, str(HERE))
import t20p2  # noqa: E402  (puts harness2 on sys.path)
import truth20  # noqa: E402
import truthbed as tb  # noqa: E402
from m8b import ism_fresh, synth_fresh  # noqa: E402

SHIM_SRC = REPO / 'crates' / 'simpa-core' / 'tests' / 'bed_shim.rs'
DECAY_SRC = REPO / 'crates' / 'simpa-core' / 'src' / 'params' / 'decay.rs'
TARGET_DIR = r'C:\tmp\nm-target-e2'
DT_FINE = t20p2.DT_FINE
STEPS_MS = (1, 10)                     # ADDENDUM-1 items 1, 2
RADII = t20p2.RADII
SLOPES = ('double', 'single')
VARIANTS = ('long', 'short')
BANDS_HZ = t20p2.BANDS_HZ
METRICS = ('t20', 't30', 'ts', 'c50', 'c80', 'd50', 'spl')
SCORED = ('t20', 'ts', 'c50', 'c80', 'd50', 'spl')      # T30 is regression only (PREREG.md line 4)
WITHIN, SHARE_MIN = 0.1, 0.99          # criterion 1: within 1/10 limen in >= 99 % of answered, none beyond it


def sha256(p):
    return hashlib.sha256(Path(p).read_bytes().replace(b'\r\n', b'\n')).hexdigest()


# ---- S1 -------------------------------------------------------------------------------------------------------------
def s1_spec(p, R, step_ms, slope, variant):
    """ADDENDUM-1 item 1. long: to 2.5 x the slowest slope's T60; short: the generator's own run."""
    ratio = p['ratio'] if slope == 'double' else 1.0
    f = t20p2.ECHO_FACTOR * ratio if variant == 'long' else p['run_over_t60']
    s = synth_fresh.make_spec(id='s1|%s|%s|R%g|%gms|%s' % (p['draw'], slope, R, step_ms, variant), ratio=p['ratio'],
                              step_ms=step_ms, t60_s=p['t60_s'], late_share_db=p['late_share_db'],
                              drr_db=p['drr_db'], R_m=R, d_m=p['d_m'], gap_ms=p['gap_ms'], delay_ms=p['delay_ms'],
                              run_over_t60=f)
    if slope == 'single':
        k1 = s['k'][0]
        s['A'], s['k'] = [1.0], [k1]
        s['Ed'] = (1.0 / k1) * 10 ** (p['drr_db'] / 10)
    return s


def s1_fine(spec, slow_t60):
    """t20p2.s1_fine for either slope: the ball's fine echogram (direct, reflected) to 2.5 x the slowest T60."""
    S = synth_fresh.generator()
    t = spec['d_m'] / synth_fresh.C
    T = t + t20p2.ECHO_FACTOR * slow_t60
    direct = S.histogram(DT_FINE, T, t, spec['half_width'], spec['Ed'], spec['gap_ms'] * 1e-3, [0.0], [1.0])
    refl = S.histogram(DT_FINE, T, t, spec['half_width'], 0.0, spec['gap_ms'] * 1e-3, spec['A'], spec['k'])
    n_cut = int(round((t + t20p2.CUT_FACTOR * slow_t60) / DT_FINE))
    return direct, refl, t, n_cut


def s1_unit(job):
    p, R, steps, slopes, variants = job
    t0 = time.time()
    rows = []
    nfine = 0
    for slope in slopes:
        spec1 = s1_spec(p, R, steps[0], slope, 'long')
        slow = p['t60_s'] * (p['ratio'] if slope == 'double' else 1.0)
        direct, refl, t, n_cut = s1_fine(spec1, slow)
        nfine = max(nfine, len(refl))
        tr = {}
        for m, bottom in (('t20', -25.0), ('t30', -35.0)):
            full = truth20.t20(direct, refl, t, DT_FINE, bottom_db=bottom)[0]
            cut = truth20.t20(direct[:n_cut], refl[:n_cut], t, DT_FINE, bottom_db=bottom)[0]
            tr[m] = (full, abs(full - cut))
        cf = tb.closed_form(spec1['Ed'], spec1['gap_ms'] * 1e-3, spec1['A'], spec1['k'])
        for m in tb.METRICS:
            tr[m] = (cf[m], 0.0)
        for step_ms in steps:
            for variant in variants:
                spec = s1_spec(p, R, step_ms, slope, variant)
                rows.append(dict(set='S1', id=spec['id'], unit='%s|R%g' % (p['draw'], R), slope=slope,
                                 variant=variant, ratio=p['ratio'], R=R, step_ms=step_ms, t60=p['t60_s'],
                                 d=p['d_m'], delay_ms=p['delay_ms'], drr_db=p['drr_db'], gap_ms=p['gap_ms'],
                                 run_over_t60=spec['run_over_t60'], dt=spec['dt'], arrival=spec['t_arrival'],
                                 half_width=spec['half_width'],
                                 **{'truth_' + m: v[0] for m, v in tr.items()},
                                 **{'u_' + m: v[1] for m, v in tr.items()},
                                 bins=synth_fresh.histogram(spec)))
    return rows, time.time() - t0, nfine


# ---- S2 -------------------------------------------------------------------------------------------------------------
def s2_truths(direct, refl, n, tail, t):
    """Every truth on the series cut at n (fine bins) with its fitted tail appended to the reflected part."""
    r = np.concatenate([refl[:n], tail])
    d = np.concatenate([direct[:n], np.zeros(len(tail))])
    out = dict(t20=truth20.t20(d, r, t, DT_FINE)[0], t30=truth20.t20(d, r, t, DT_FINE, bottom_db=-35.0)[0])
    out.update(tb.binned(float(d.sum()), r, DT_FINE, t))
    return out


def s2_unit(job):
    u, bands, steps = job
    ism = ism_fresh.generator()
    t0 = time.time()
    C = ism_fresh.C
    dl = C * DT_FINE
    alpha = [(u['a6'][0], u['a6'][1]), (u['a6'][2], u['a6'][3]), (u['a6'][4], u['a6'][5])]
    lmax = C * t20p2.ECHO_FACTOR * u['t60']
    bd, br = ism.echogram(u['L'], u['src'], u['rec'], u['R'], alpha, lmax, dl)
    nb = len(bd)
    n_cut = int(math.ceil(C * t20p2.CUT_FACTOR * u['t60'] / dl))
    t = u['d'] / C
    uid = '%s|rec%d|R%g' % (u['room'], u['rec_i'], u['R'])
    rows = []
    for F in bands:
        ac = ism.air_factor(nb, dl, ism.m_energy(float(F)), C, None)     # continuous air (ADDENDUM-1 item 2)
        b_d, b_r = bd * ac, br * ac
        tail, fit = t20p2.extend(b_d + b_r, nb)
        tail_c, fit_c = t20p2.extend(b_d + b_r, n_cut)
        full = s2_truths(b_d, b_r, nb, tail, t)
        cut = s2_truths(b_d, b_r, n_cut, tail_c, t)
        vc = np.concatenate([b_d + b_r, tail])
        for step_ms in steps:
            k = int(round(step_ms * 1e-3 / DT_FINE))
            n = len(vc) // k
            vb = vc[:n * k].reshape(n, k).sum(1)
            nz = np.nonzero(vb > 0)[0]
            vb = vb[:int(nz[-1]) + 1]
            rows.append(dict(set='S2', id='s2|%s|%dHz|%gms' % (uid, F, step_ms), unit=uid, room=u['room'],
                             rec_i=u['rec_i'], R=u['R'], band_hz=F, step_ms=step_ms, t60=u['t60'], d=u['d'],
                             dt=k * DT_FINE, arrival=t, half_width=u['R'] / C,
                             tail_fit_ok=bool(fit['ok'] and fit_c['ok']), tail_t60=fit['t60'], tail_share=fit['share'],
                             **{'truth_' + m: full[m] for m in METRICS},
                             **{'u_' + m: abs(full[m] - cut[m]) for m in METRICS},
                             bins=vb))
    return rows, time.time() - t0, nb


# ---- the shim -------------------------------------------------------------------------------------------------------
def run_shim(entries, folder, say):
    """entries: [(id, dt, arrival, half_width, bins)] -> {id: shim record}; t20p2.run_shim's format and call."""
    folder.mkdir(parents=True, exist_ok=True)
    off = 0
    with open(folder / 'bins.bin', 'wb') as fb, open(folder / 'manifest.jsonl', 'w', encoding='utf-8') as fm:
        for (i, dt, arr, hw, bins) in entries:
            b = np.ascontiguousarray(bins, dtype='<f8').tobytes()
            fb.write(b)
            fm.write(json.dumps(dict(id=i, dt=dt, arrival=arr, half_width=hw, offset=off, n=len(bins))) + '\n')
            off += len(b)
    env = dict(os.environ, BED_SHIM_DIR=str(folder))
    env.setdefault('CARGO_TARGET_DIR', TARGET_DIR)
    t0 = time.time()
    p = subprocess.run(['cargo', 'test', '-p', 'simpa-core', '--test', 'bed_shim', '--release', '--', '--ignored',
                        '--nocapture', '--exact', 'bed_shim'], cwd=REPO, env=env, capture_output=True, text=True)
    if p.returncode != 0 or 'BED SHIM:' not in p.stdout:
        raise RuntimeError('shim failed:\n%s\n%s' % (p.stdout[-3000:], p.stderr[-3000:]))
    say('shim: %d rows in %.1f s' % (len(entries), time.time() - t0))
    out = {}
    for line in (folder / 'bed.jsonl').read_text(encoding='utf-8').splitlines():
        r = json.loads(line)
        out[r['id']] = r
    return out


# ---- scoring --------------------------------------------------------------------------------------------------------
def status(r, m):
    """ADDENDUM-1 item 3: excluded when the truth is not finite, a tail fit did not settle, or its uncertainty
    exceeds 1/10 of the limen."""
    tr = r['truth_' + m]
    if tr is None or not math.isfinite(tr):
        return 'truth_nan'
    if r.get('tail_fit_ok') is False:
        return 'truth_truncated'
    if not math.isfinite(r['u_' + m]) or r['u_' + m] > WITHIN * tb.limen(m, tr):
        return 'truth_uncertain'
    return 'ok'


def score_metric(rows, m):
    st = {}
    for r in rows:
        s = status(r, m)
        st[s] = st.get(s, 0) + 1
    ok = [r for r in rows if status(r, m) == 'ok']
    ans = [r for r in ok if r[m] is not None]
    fr = [(abs(r[m] - r['truth_' + m]) / tb.limen(m, r['truth_' + m]), r['id']) for r in ans]
    within = sum(1 for f, _ in fr if f <= WITHIN)
    beyond = [(f, i) for f, i in fr if f > 1.0]
    worst = max(fr) if fr else (None, None)
    # Build F (shim m_status): an answered row is `ok` or `wide`; an `ok` row beyond 1/10 limen is what
    # must not exist. Rows from a shim before build F carry no status and count as `ok`.
    st_of = {r['id']: (r.get(m + '_status') or 'ok') for r in ans}
    ok_beyond = sorted(((f, i) for f, i in fr if f > WITHIN and st_of[i] == 'ok'), reverse=True)
    wide = [r for r in ans if st_of[r['id']] == 'wide']
    in_bracket = [r for r in wide if r.get(m + '_lo') is not None and r.get(m + '_hi') is not None
                  and r[m + '_lo'] <= r['truth_' + m] <= r[m + '_hi']]
    refusals = {}
    for r in ok:
        if r[m] is None:
            refusals[r[m + '_code']] = refusals.get(r[m + '_code'], 0) + 1
    out = dict(rows=len(rows), status=st, scored=len(ok), answered=len(ans),
               answered_share=len(ans) / len(ok) if ok else None,
               worst_frac_limen=worst[0], worst_id=worst[1],
               within_tenth=within, within_tenth_share=within / len(ans) if ans else None,
               beyond_limen=len(beyond), beyond_ids=[i for _, i in sorted(beyond, reverse=True)[:10]],
               refusals=dict(sorted(refusals.items())),
               ok_rows=len(ans) - len(wide), wide_rows=len(wide), wide_truth_in_bracket=len(in_bracket),
               ok_beyond_tenth=len(ok_beyond), ok_beyond_tenth_ids=[i for _, i in ok_beyond[:10]])
    out['pass_'] = bool(ans and out['within_tenth_share'] >= SHARE_MIN and not beyond)
    return out


def score(rows, keys):
    out = {}
    for m in METRICS:
        e = dict(all=score_metric(rows, m))
        for key in keys:
            for v in sorted({r[key] for r in rows}, key=str):
                e['%s=%s' % (key, v)] = score_metric([r for r in rows if r[key] == v], m)
        out[m] = e
    return out


# ---- driver ---------------------------------------------------------------------------------------------------------
def hashes():
    diff = subprocess.run(['git', 'diff', '--stat', '--', 'crates/simpa-core/src'], cwd=REPO, capture_output=True,
                          text=True).stdout.strip()
    head = subprocess.run(['git', 'rev-parse', 'HEAD'], cwd=REPO, capture_output=True, text=True).stdout.strip()
    return dict(head=head, src_uncommitted=diff, prereg=sha256(BED / 'PREREG.md'), addendum1=sha256(BED / 'ADDENDUM-1.md'),
                shim=sha256(SHIM_SRC), decay_rs=sha256(DECAY_SRC), seta=sha256(__file__),
                truthbed=sha256(HERE / 'truthbed.py'), t20p2=sha256(T20H / 't20p2.py'), truth20=sha256(T20H / 'truth20.py'))


def spread(n, total):
    return t20p2.spread(n, total)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('set', choices=('s1', 's2', 'summary'))
    ap.add_argument('out')
    ap.add_argument('--workers', type=int, default=4)
    ap.add_argument('--smoke', type=int, default=0)
    a = ap.parse_args()
    if a.set == 'summary':
        return summary(Path(a.out))
    s2 = a.set == 's2'
    out = Path(a.out) / a.set
    if a.smoke:
        out = out / 'smoke'
    out.mkdir(parents=True, exist_ok=True)
    log = open(out / 'run.log', 'a', encoding='utf-8')

    def say(msg):
        line = '%s %s' % (time.strftime('%H:%M:%S'), msg)
        print(line, flush=True)
        log.write(line + '\n')
        log.flush()

    mode = 'SMOKE' if a.smoke else 'SCORED'
    h = hashes()
    say('%s %s -> %s; hashes %s' % (mode, a.set, out, json.dumps(h)))
    if not s2:
        draws, rej = t20p2.s1_draws()
        meta = dict(d_rejections=rej, draws=len(draws))
        jobs = [(p, R, STEPS_MS, SLOPES, VARIANTS) for p in draws for R in RADII]
        fn = s1_unit
    else:
        units, meta = t20p2.s2_units()
        meta['units'] = len(units)
        jobs = [(u, BANDS_HZ, STEPS_MS) for u in units]
        jobs.sort(key=lambda j: -j[0]['t60'] ** 3 / (j[0]['L'][0] * j[0]['L'][1] * j[0]['L'][2]))   # heavy first
        fn = s2_unit
    if a.smoke:
        jobs = [jobs[i] for i in spread(a.smoke, len(jobs))]
    say('meta %s; %d units' % (json.dumps(meta), len(jobs)))
    rows, timing = [], []
    t0 = time.time()
    with ProcessPoolExecutor(a.workers) as ex:
        for i, (res, sec, nfine) in enumerate(ex.map(fn, jobs), 1):
            rows.extend(res)
            timing.append(dict(unit=res[0]['unit'], seconds=sec, fine_bins=nfine))
            if i % 50 == 0 or s2:
                say('%d/%d %s %.1f s' % (i, len(jobs), res[0]['unit'], sec))
    say('rows built: %d in %.1f s' % (len(rows), time.time() - t0))
    shim = run_shim([(r['id'], r['dt'], r['arrival'], r['half_width'], r['bins']) for r in rows], out / 'shim', say)
    for r in rows:
        s = shim[r['id']]
        r['decay_arrival'], r['cdts_arrival'] = s['decay_arrival'], s['arrival']
        for m in METRICS:
            r[m], r[m + '_code'] = s[m], s[m + '_code']
            r[m + '_status'] = s.get(m + '_status')
            for k in ('_lo', '_hi'):
                if m + k in s:
                    r[m + k] = s[m + k]
        r['n_bins'] = len(r['bins'])
    keys = ('slope', 'variant', 'step_ms') if not s2 else ('step_ms', 'band_hz', 'R')
    sc = score(rows, keys)
    summ = dict(mode=mode, set=a.set, hashes=h, meta=meta, timing_total_s=time.time() - t0, score=sc,
                pass_=({m: sc[m]['all']['pass_'] for m in SCORED} if mode == 'SCORED' else None))
    cols = sorted({k for r in rows for k in r if k != 'bins'})
    with open(out / 'rows.csv', 'w', newline='', encoding='utf-8') as f:
        w = csv.DictWriter(f, fieldnames=cols)
        w.writeheader()
        for r in rows:
            w.writerow({k: r.get(k) for k in cols})
    (out / 'summary.json').write_text(json.dumps(summ, indent=1, default=str), encoding='utf-8')
    say('%s: %s' % (mode, json.dumps({m: (sc[m]['all']['answered_share'], sc[m]['all']['worst_frac_limen'],
                                          sc[m]['all']['within_tenth_share'], sc[m]['all']['beyond_limen'])
                                      for m in METRICS})))
    say('build F ok/wide: %s' % json.dumps({m: dict(ok=sc[m]['all']['ok_rows'], wide=sc[m]['all']['wide_rows'],
                                                   wide_truth_in_bracket=sc[m]['all']['wide_truth_in_bracket'],
                                                   ok_beyond_tenth=sc[m]['all']['ok_beyond_tenth'])
                                            for m in METRICS}))
    say('done: %s' % (out / 'summary.json'))


def summary(root):
    """Set A over S1 and S2: criterion 1 per metric on both sets' rows together, and per set."""
    rows = []
    sets = {}
    for s in ('s1', 's2'):
        with open(root / s / 'rows.csv', encoding='utf-8') as f:
            rr = list(csv.DictReader(f))
        sets[s] = json.loads((root / s / 'summary.json').read_text(encoding='utf-8'))
        for r in rr:
            for k in list(r):
                if k in METRICS or k.startswith('truth_') or k.startswith('u_') or k.endswith(('_lo', '_hi')):
                    r[k] = float(r[k]) if r[k] not in ('', 'None') else None
            r['tail_fit_ok'] = {'True': True, 'False': False}.get(r.get('tail_fit_ok'))
            rows.append(r)
    sc = score(rows, ('set',))
    res = dict(prereg_criterion='within 1/10 limen in >= 99 % of answered rows and no answered row beyond the limen',
               hashes={s: v['hashes'] for s, v in sets.items()}, modes={s: v['mode'] for s, v in sets.items()},
               score=sc, pass_={m: sc[m]['all']['pass_'] for m in SCORED},
               t30_regression=sc['t30']['all']['pass_'])
    res['verdict'] = ('PASS' if all(res['pass_'].values()) else 'FAIL') if all(
        v == 'SCORED' for v in res['modes'].values()) else 'NO VERDICT (smoke)'
    (root / 'summary.json').write_text(json.dumps(res, indent=1, default=str), encoding='utf-8')
    for m in METRICS:
        x = sc[m]['all']
        print('%-4s answered %.4f worst %.4f within %.4f beyond %d status %s' % (
            m, x['answered_share'] or 0, x['worst_frac_limen'] or 0, x['within_tenth_share'] or 0, x['beyond_limen'],
            x['status']))
    print('VERDICT', res['verdict'], res['pass_'])


if __name__ == '__main__':
    main()
