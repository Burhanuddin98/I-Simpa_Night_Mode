"""The bed's STI sets (../ADDENDUM-2-STI.md, frozen; ../ADDENDUM-3.md, the builder's rulings before any STI number).

The product's STI (`params::sti`: set A through the shim's `bed_shim_sti` test, sets B7 and C7 from `simpa results
--json`) against the independent reference ../../2026-10-02-sti/reference/sti_reference.py, imported read-only after
its SHA-256 is checked against reference/HASH.txt. The reference is used through its own functions: the MTF of each
distinct band series (`mtf_from_energy_ir`), then `sti_from_mtf` once per sex with that sex's Table A.4 spectrum
(`speech_spectrum`), and its cl. 8.3 a duration rule (`MIN_IR_DURATION_S`, half the reverberation time) as its
`sti_reference` applies it. This is `sti_reference` split so that one MTF serves both sexes and both noise variants.

  python -B stibed.py a-s1|a-s2|b7|c7 [--out DIR] [--workers N] [--smoke N]

Outputs under B:\\data\\m8b-bed\\STI\\<set>\\: rows.jsonl, summary.json, run.log (set A also shim\\).
Reused, not rewritten: seta.s1_unit / seta.s1_spec (S1), t20p2.s2_units / extend and ism_fresh (S2, C7), truth20 (T30),
scorebc.runs / report / solver_stderr / atmosphere / eyring (B7, C7), run_c.geometry (C7), rooms2 (B7 arrival check).
"""
import sys

sys.dont_write_bytecode = True

import argparse  # noqa: E402
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
sys.path.insert(0, str(HERE))
import seta  # noqa: E402  (puts T20 part 2's harness and harness2 on sys.path)
import t20p2  # noqa: E402
import truth20  # noqa: E402
from m8b import ism_fresh, rooms2  # noqa: E402

BED = HERE.parent
REPO = BED.parents[2]
REF_DIR = BED.parent / '2026-10-02-sti' / 'reference'
SIMPA = Path(r'C:\tmp\nm-target-g\release\simpa.exe')
TARGET_DIR = r'C:\tmp\nm-target-e2'
OUT_ROOT = Path(r'B:\data\m8b-bed\STI')
DATA = dict(b7=Path(r'B:\data\m8b-bed\B7'), c7=Path(r'B:\data\m8b-bed\C7'))
BANDS = (125, 250, 500, 1000, 2000, 4000, 8000)
SEXES = ('male', 'female')
P0SQ = 4e-10
RHO_C_W_S2 = 413.25e-4                 # ADDENDUM-3 item 7: Lw 80 dB times SPPS's rho c
NOISE_A = (None, 30.0)                 # ADDENDUM-3 item 8
TOL, AGREE, SHARE_MIN = 0.03, 0.01, 0.99
U_MAX, ANSWERED_MIN = 0.01, 0.80
TRUTH_SEEDS, TESTED_SEEDS = (9301, 9302), (4301, 4302, 4303)
DT_FINE = t20p2.DT_FINE
PAD = False
REFERENCES = None                      # --references: C7's cached references, never recomputed (CACHE_ONLY)
CACHE_ONLY = False


def sha(p):
    return hashlib.sha256(Path(p).read_bytes()).hexdigest()


def reference():
    """The reference module, executed only when its bytes hash to HASH.txt's value."""
    want = (REF_DIR / 'HASH.txt').read_text(encoding='utf-8').split()[0]
    data = (REF_DIR / 'sti_reference.py').read_bytes()
    got = hashlib.sha256(data).hexdigest()
    if got != want:
        raise SystemExit('sti_reference.py sha256 %s is not HASH.txt %s: not used' % (got, want))
    spec = importlib.util.spec_from_file_location('sti_reference', REF_DIR / 'sti_reference.py')
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    mod.SHA256 = got
    return mod


REF = reference()
SPEECH = {s: np.asarray(REF.speech_spectrum(s, REF.DEFAULT_SPEECH_LEVEL_DBA), float) for s in SEXES}
NEEDS = dict(male=list(range(7)), female=list(range(1, 7)))


def ref_mtf(series, dt):
    return REF.mtf_from_energy_ir(series, dt)


def ref_sti(m7, durations, sex, L_s, L_n, T):
    """(STI, None) or (None, reason): the reference's STI of one sex, refused on its own cl. 8.3 a flags over the
    bands that sex uses (female has no 125 Hz)."""
    k = NEEDS[sex]
    d = np.asarray(durations)[k]
    if not np.all(d >= REF.MIN_IR_DURATION_S):
        return None, 'ref_shorter_than_1.6s'
    if T is not None and not np.all(d >= 0.5 * T):
        return None, 'ref_shorter_than_half_T'
    out = REF.sti_from_mtf(np.asarray(m7), L_s, L_n)
    return float(out['sti_' + sex]), None


def transfer_db(total_pa2, power_rho_c):
    """ADDENDUM-3 item 7: the band's SPL less the free field at 1 m of the same source."""
    return 10 * math.log10(total_pa2 / P0SQ) - 10 * math.log10(power_rho_c / (4 * math.pi) / P0SQ)


# ---- set A ----------------------------------------------------------------------------------------------------------
def first_finite(*xs):
    for x in xs:
        if x is not None and math.isfinite(x):
            return float(x)
    return None


def a_s1_unit(job):
    """seta.s1_unit's 16 rows of one (draw, R), as STI cases: the one series in all seven bands."""
    p, R = job
    rows, sec, _ = seta.s1_unit((p, R, seta.STEPS_MS, seta.SLOPES, seta.VARIANTS))
    out = []
    for r in rows:
        spec = seta.s1_spec(p, R, r['step_ms'], r['slope'], r['variant'])
        slow = p['t60_s'] * (p['ratio'] if r['slope'] == 'double' else 1.0)
        b = np.asarray(r['bins'], float)
        out.append(dict(id=r['id'], sub='S1', slope=r['slope'], variant=r['variant'], step_ms=r['step_ms'], R=R,
                        dt=r['dt'], arrival=r['arrival'], half_width=r['half_width'],
                        power_rho_c=4 * math.pi * spec['d_m'] ** 2 * spec['Ed'],
                        T=[first_finite(r['truth_t30'], r['truth_t20'], slow)] * 7, band_bins=[b] * 7))
    return out, sec


def a_s2_unit(u):
    """One S2 unit (t20p2.s2_units) as STI cases at 1 and 10 ms: seta.s2_unit's band construction at seven bands
    (continuous air, the band's own fitted tail after 2.5 x the 1 kHz Eyring T60), in Pa^2 (ADDENDUM-3 item 7)."""
    ism = ism_fresh.generator()
    t0 = time.time()
    C = ism_fresh.C
    dl = C * DT_FINE
    alpha = [(u['a6'][0], u['a6'][1]), (u['a6'][2], u['a6'][3]), (u['a6'][4], u['a6'][5])]
    bd, br = ism.echogram(u['L'], u['src'], u['rec'], u['R'], alpha, C * t20p2.ECHO_FACTOR * u['t60'], dl)
    nb = len(bd)
    t = u['d'] / C
    scale = RHO_C_W_S2 / (4.0 / 3.0 * math.pi * u['R'] ** 3)
    fine, T, tail_ok = [], [], []
    for F in BANDS:
        ac = ism.air_factor(nb, dl, ism.m_energy(float(F)), C, None)
        b_d, b_r = bd * ac, br * ac
        tail, fit = t20p2.extend(b_d + b_r, nb)
        r_ = np.concatenate([b_r, tail])
        d_ = np.concatenate([b_d, np.zeros(len(tail))])
        T.append(first_finite(truth20.t20(d_, r_, t, DT_FINE, bottom_db=-35.0)[0], truth20.t20(d_, r_, t, DT_FINE)[0],
                              fit['t60']))
        fine.append(np.concatenate([b_d + b_r, tail]) * scale)
        tail_ok.append(bool(fit['ok']))
    uid = '%s|rec%d|R%g' % (u['room'], u['rec_i'], u['R'])
    out = []
    for step_ms in seta.STEPS_MS:
        k = int(round(step_ms * 1e-3 / DT_FINE))
        bb = []
        for vc in fine:
            n = len(vc) // k
            vb = vc[:n * k].reshape(n, k).sum(1)
            nz = np.nonzero(vb > 0)[0]
            bb.append(vb[:int(nz[-1]) + 1])
        out.append(dict(id='s2|%s|%gms' % (uid, step_ms), sub='S2', room=u['room'], R=u['R'], step_ms=step_ms,
                        dt=k * DT_FINE, arrival=t, half_width=u['R'] / C, power_rho_c=RHO_C_W_S2, T=T,
                        tail_ok=all(tail_ok), band_bins=bb))
    return out, time.time() - t0


def run_shim(cases, folder, say):
    """Write bins.bin (one copy per distinct series) and sti_manifest.jsonl (each case once per noise variant), run
    `bed_shim_sti`, and return {row id: shim record}."""
    folder.mkdir(parents=True, exist_ok=True)
    off = 0
    with open(folder / 'bins.bin', 'wb') as fb, open(folder / 'sti_manifest.jsonl', 'w', encoding='utf-8') as fm:
        for c in cases:
            refs, seen = [], {}
            for b in c['band_bins']:
                if id(b) not in seen:
                    raw = np.ascontiguousarray(b, dtype='<f8').tobytes()
                    fb.write(raw)
                    seen[id(b)] = (off, len(b))
                    off += len(raw)
                refs.append(seen[id(b)])
            for nz in NOISE_A:
                fm.write(json.dumps(dict(
                    id='%s|noise%s' % (c['id'], 'none' if nz is None else '%g' % nz), dt=c['dt'],
                    arrival=c['arrival'], half_width=c['half_width'],
                    bands=[dict(freq_hz=f, offset=o, n=n, power_rho_c=c['power_rho_c'], noise_db=nz)
                           for f, (o, n) in zip(BANDS, refs)])) + '\n')
    env = dict(os.environ, BED_SHIM_DIR=str(folder), CARGO_TARGET_DIR=TARGET_DIR, CARGO_INCREMENTAL='0')
    t0 = time.time()
    p = subprocess.run(['cargo', 'test', '-p', 'simpa-core', '--test', 'bed_shim', '--release', '--', '--ignored',
                        '--nocapture', '--exact', 'bed_shim_sti'], cwd=REPO, env=env, capture_output=True, text=True)
    (folder / 'cargo.stderr.txt').write_text(p.stderr, encoding='utf-8')
    if p.returncode != 0 or 'BED SHIM STI:' not in p.stdout:
        raise RuntimeError('shim failed:\n%s\n%s' % (p.stdout[-3000:], p.stderr[-3000:]))
    say('shim: %d cases in %.1f s' % (len(cases), time.time() - t0))
    out = {}
    for line in (folder / 'sti.jsonl').read_text(encoding='utf-8').splitlines():
        r = json.loads(line)
        out[r['id']] = r
    return out


def a_rows(cases, shim):
    rows = []
    for c in cases:
        from_ = int(math.floor((c['arrival'] - c['half_width']) / c['dt']))
        mt, mts = [], {}
        for b in c['band_bins']:
            if id(b) not in mts:
                mts[id(b)] = ref_mtf(b[from_:], c['dt'])
            mt.append(mts[id(b)])
        dur = [len(b[from_:]) * c['dt'] for b in c['band_bins']]
        tr = [transfer_db(float(np.sum(b)), c['power_rho_c']) for b in c['band_bins']]
        for nz in NOISE_A:
            rid = '%s|noise%s' % (c['id'], 'none' if nz is None else '%g' % nz)
            s = shim[rid]
            assert all(b['from'] == from_ for b in s['bands']), rid
            L_n = None if nz is None else [nz] * 7
            for sex in SEXES:
                L_s = SPEECH[sex] + np.asarray(tr)
                T = max(c['T'][k] for k in NEEDS[sex])
                ref, why = ref_sti(mt, dur, sex, L_s, L_n, T)
                lp = [b['speech_%s_db' % sex] for b in s['bands']]
                dl = max((abs(a - b) for a, b in zip(lp, L_s) if a is not None and math.isfinite(b)), default=None)
                row = dict(id=rid, set=c['sub'], sex=sex, noise=nz, step_ms=c['step_ms'], R=c['R'],
                           product=s[sex], product_code=s[sex + '_code'], reference=ref, reference_refusal=why,
                           level_diff_db=dl, T_ref=T, min_len_s=min(dur[k] for k in NEEDS[sex]),
                           product_T=max((b['reverberation_s'] or 0) for b in s['bands']))
                for k in ('slope', 'variant', 'room'):
                    if k in c:
                        row[k] = c[k]
                if c.get('tail_ok') is False:
                    row['excluded'] = 'truth_truncated'
                rows.append(row)
    return rows


def score_a(rows):
    """ADDENDUM-2 set A: within 0.01 in >= 99 % of rows both answer, never beyond 0.03; refusals by reason."""
    def one(rr):
        both = [r for r in rr if r['product'] is not None and r['reference'] is not None and not r.get('excluded')]
        d = sorted(((abs(r['product'] - r['reference']), r['id'], r['sex']) for r in both), reverse=True)
        within = sum(1 for x in d if x[0] <= AGREE)
        ref_only = [r for r in rr if r['product'] is None and r['reference'] is not None]
        prod_only = [r for r in rr if r['product'] is not None and r['reference'] is None]
        neither = [r for r in rr if r['product'] is None and r['reference'] is None]
        cnt = {}
        for r in rr:
            if r['product'] is None:
                cnt.setdefault('product', {}).setdefault(r['product_code'], 0)
                cnt['product'][r['product_code']] += 1
            if r['reference'] is None:
                cnt.setdefault('reference', {}).setdefault(r['reference_refusal'], 0)
                cnt['reference'][r['reference_refusal']] += 1
        lv = [r['level_diff_db'] for r in rr if r['level_diff_db'] is not None]
        res = dict(rows=len(rr), both=len(both), within_0_01=within, share=within / len(both) if both else None,
                   beyond_0_03=sum(1 for x in d if x[0] > TOL), max_abs_diff=d[0][0] if d else None,
                   worst=d[:10], product_only=len(prod_only), reference_only=len(ref_only), neither=len(neither),
                   refusals=cnt, excluded=sum(1 for r in rr if r.get('excluded')),
                   max_level_diff_db=max(lv) if lv else None,
                   product_only_ids=[(r['id'], r['sex'], r['reference_refusal']) for r in prod_only[:10]],
                   reference_only_ids=[(r['id'], r['sex'], r['product_code']) for r in ref_only[:10]])
        res['pass_'] = bool(both and res['share'] >= SHARE_MIN and res['beyond_0_03'] == 0)
        return res
    out = dict(all=one(rows))
    for key in ('set', 'sex', 'noise', 'step_ms', 'variant', 'slope'):
        for v in sorted({r.get(key) for r in rows}, key=str):
            out['%s=%s' % (key, v)] = one([r for r in rows if r.get(key) == v])
    return out


def set_a(sub, out, workers, smoke, say):
    if sub == 'a-s1':
        draws, _ = t20p2.s1_draws()
        jobs = [(p, R) for p in draws for R in seta.RADII]
        fn = a_s1_unit
    else:
        units, _ = t20p2.s2_units()
        jobs = sorted(units, key=lambda u: -u['t60'] ** 3 / (u['L'][0] * u['L'][1] * u['L'][2]))
        fn = a_s2_unit
    if smoke:
        jobs = [jobs[i] for i in t20p2.spread(smoke, len(jobs))]
    say('%s: %d units, %d workers' % (sub, len(jobs), workers))
    cases = []
    t0 = time.time()
    with ProcessPoolExecutor(workers) as ex:
        for i, (res, sec) in enumerate(ex.map(fn, jobs), 1):
            cases.extend(res)
            if i % 50 == 0 or sub == 'a-s2':
                say('%d/%d %s %.1f s' % (i, len(jobs), res[0]['id'], sec))
    say('cases built: %d in %.1f s' % (len(cases), time.time() - t0))
    shim = run_shim(cases, out / 'shim', say)
    rows = a_rows(cases, shim)
    return rows, score_a(rows)


# ---- sets B7 and C7 -------------------------------------------------------------------------------------------------
def receivers(rep):
    """{label: receiver dict} of an SPPS report, with its bands by frequency."""
    out = {}
    for pr in rep['spps']['point_receivers']:
        pr['_bands'] = {b['freq_hz']: b for b in pr['bands']}
        pr['_sti_bands'] = {b['freq_hz']: b for b in pr['sti']['bands']}
        out[pr['label']] = pr
    return out


def sti_value(e):
    """(value, code) of the report's `sti.male` / `sti.female`."""
    if e is not None and e.get('value') is not None:
        return float(e['value']), None
    import scorebc
    return None, scorebc.read_param(e)[4]


def product_levels(pr, sex):
    """ADDENDUM-3 item 9: the tested report's own speech and noise levels per band."""
    sb = pr['_sti_bands']
    L_s = np.array([np.nan if sb.get(f, {}).get('speech_%s_db' % sex) is None else sb[f]['speech_%s_db' % sex]
                    for f in BANDS], float)
    nz = [sb.get(f, {}).get('noise_db') for f in BANDS]
    L_n = None if all(x is None for x in nz) else [np.nan if x is None else x for x in nz]
    return L_s, L_n


def leading_bin(arr, dt):
    return int(math.floor((arr['time_s'] - arr['half_width_s']) / dt))


def exclusion(value, u, tail_ok):
    """A row is excluded for its truth only when the product answered (ADDENDUM-4 item 1): a refusal counts as not
    answered, whatever the truth; an answered row is excluded when the truth's tail did not settle, or its uncertainty
    is unknown or above 0.01."""
    if value is None:
        return None
    if not tail_ok:
        return 'truth_truncated'
    return 'truth_uncertain' if (u is None or u > U_MAX) else None


def score_bc(rows, rooms):
    """ADDENDUM-2 sets B and C: wrong-silent = answered and |product - reference| > 0.03; pass: none, and answered in
    >= 80 % of each room's receivers (per sex, over rows not excluded; ADDENDUM-3 item 12)."""
    def one(rr):
        ok = [r for r in rr if not r['excluded']]
        ans = [r for r in ok if r['product'] is not None and r['reference'] is not None]
        ws = [r for r in ans if abs(r['product'] - r['reference']) > TOL]
        d = sorted(((abs(r['product'] - r['reference']), r['room'], r['label'], r['seed'], r['sex']) for r in ans),
                   reverse=True)
        refusals = {}
        for r in ok:
            if r['product'] is None:
                refusals[r['product_code']] = refusals.get(r['product_code'], 0) + 1
        by_room = {}
        for room in rooms:
            for sex in SEXES:
                a = [r for r in rr if r['room'] == room and r['sex'] == sex]
                o = [r for r in a if not r['excluded']]
                n_ans = sum(1 for r in o if r['product'] is not None)
                by_room['%s|%s' % (room, sex)] = dict(
                    rows=len(a), scored=len(o), answered=n_ans, share=n_ans / len(o) if o else None,
                    share_all_rows=sum(1 for r in a if r['product'] is not None) / len(a) if a else None,
                    excluded=len(a) - len(o))
        sens = [abs(r['reference'] - r['reference_own_levels']) for r in ans
                if r.get('reference_own_levels') is not None]
        res = dict(rows=len(rr), excluded=sum(1 for r in rr if r['excluded']), scored=len(ok), answered=len(ans),
                   wrong_silent=len(ws), max_abs_diff=d[0][0] if d else None, worst=d[:10],
                   mean_diff=float(np.mean([r['product'] - r['reference'] for r in ans])) if ans else None,
                   within_0_01=sum(1 for x in d if x[0] <= AGREE), refusals=refusals,
                   reference_refused=sum(1 for r in ok if r['reference'] is None), by_room=by_room,
                   min_room_share=min((v['share'] for v in by_room.values() if v['share'] is not None), default=None),
                   sensitivity_own_levels_max=max(sens) if sens else None)
        res['pass_'] = bool(ans and not ws and res['min_room_share'] is not None
                            and res['min_room_share'] >= ANSWERED_MIN)
        return res
    out = dict(all=one(rows))
    for sex in SEXES:
        out['sex=%s' % sex] = one([r for r in rows if r['sex'] == sex])
    return out


def set_b7(out, say):
    import scorebc
    data = DATA['b7']
    allruns = scorebc.runs(data)
    geom = rooms2.rooms()
    rows, info = [], {}
    rooms = ['G%d' % i for i in range(1, 8)]
    for room in rooms:
        need = [('truth', room, s) for s in TRUTH_SEEDS] + [('tested', room, s) for s in TESTED_SEEDS]
        bad = [k for k in need if k not in allruns or allruns[k]['run_exit'] != 0 or not allruns[k]['run_folder']]
        if bad:
            raise RuntimeError('room %s: runs not finished or failed: %s' % (room, bad))
        reps = {k: scorebc.report(allruns[k]['run_folder'], out, '%s-%s-%d' % k, SIMPA) for k in need}
        info[room] = {'%s-%d' % (k[0], k[2]): scorebc.solver_stderr(allruns[k]['run_folder']) for k in need}
        rc = {k: receivers(reps[k]) for k in need}
        c = reps[need[0]]['spps']['speed_of_sound_m_s']
        T = max(geom[room]['design_t60_s'].values())          # no 8 kHz design value: the bands' largest
        for label, a in rc[need[0]].items():
            b = rc[need[1]][label]
            t_geo = geom[room]['receivers'][int(label.lstrip('Rr'))]['d_m'] / c
            arr = a['_bands'][1000]['arrival']
            if abs(arr['time_s'] - t_geo) > 1e-6:
                raise RuntimeError('%s %s: arrival %r against the geometry\'s %r' % (room, label, arr['time_s'], t_geo))
            dt = reps[need[0]]['spps']['time_step_s']
            fr = leading_bin(arr, dt)
            e1 = [np.asarray(a['_bands'][f]['energy_pa2'], float)[fr:] for f in BANDS]
            e2 = [np.asarray(b['_bands'][f]['energy_pa2'], float)[fr:] for f in BANDS]
            pooled = [0.5 * (x + y) for x, y in zip(e1, e2)]
            m_p, m_1, m_2 = ([ref_mtf(x, dt) for x in s] for s in (pooled, e1, e2))
            dur = [len(x) * dt for x in pooled]
            own_tr = [transfer_db(float(np.sum(x)), a['_bands'][f]['source_power_rho_c']) for x, f in zip(pooled, BANDS)]
            for seed in TESTED_SEEDS:
                pr = rc[('tested', room, seed)][label]
                for sex in SEXES:
                    L_s, L_n = product_levels(pr, sex)
                    v, code = sti_value(pr['sti'][sex])
                    if np.isnan(L_s[NEEDS[sex]]).any():
                        ref = r1 = r2 = own = None
                        why = 'no_product_levels'
                    else:
                        ref, why = ref_sti(m_p, dur, sex, L_s, L_n, T)
                        r1, _ = ref_sti(m_1, dur, sex, L_s, L_n, T)
                        r2, _ = ref_sti(m_2, dur, sex, L_s, L_n, T)
                        own, _ = ref_sti(m_p, dur, sex, SPEECH[sex] + np.asarray(own_tr), L_n, T)
                    u = abs(r1 - r2) / 2 if r1 is not None and r2 is not None else None
                    rows.append(dict(room=room, label=label, seed=seed, sex=sex, product=v, product_code=code,
                                     reference=ref, reference_refusal=why, u=u, reference_own_levels=own,
                                     excluded=exclusion(v, u, True),
                                     noise=pr['sti']['noise'][:5]))
        say('room %s: %d rows' % (room, sum(1 for r in rows if r['room'] == room)))
    return rows, score_bc(rows, rooms), info


def c7_reference(room, label, rec, L, a6, src, R, c, atm, cache, say):
    """ADDENDUM-3 item 11, cached: per band the MTF of the ball's image-source echogram (2.5 x and 1.5 x cuts, each
    with its fitted tail), the durations, the Eyring T60, and the image-source transfer by ADDENDUM-1 item 6."""
    f = cache / ('%s-%s.npz' % (room, label))
    if f.exists():
        z = np.load(f)
        return {k: z[k] for k in z.files}
    if CACHE_ONLY:
        raise RuntimeError('%s: no cached reference, and --references forbids computing one' % f)
    import scorebc
    ism = ism_fresh.generator()
    t0 = time.time()
    ms = [ism.iso9613_db_per_m(float(F), H=atm['H'], P=atm['P'], T_c=atm['T_c']) * math.log(10) / 10 for F in BANDS]
    t60 = [scorebc.eyring(L, a6, m) for m in ms]
    dl = c * DT_FINE
    alpha = [(a6[0], a6[1]), (a6[2], a6[3]), (a6[4], a6[5])]
    bd, br = ism.echogram(L, src, rec, R, alpha, c * t20p2.ECHO_FACTOR * max(t60), dl)
    fr = int(math.floor((math.dist(src, rec) / c - R / c) / DT_FINE))
    m_full, m_cut, d_full, d_cut, tot, ok = [], [], [], [], [], []
    for m, T in zip(ms, t60):
        nb = int(math.ceil(c * t20p2.ECHO_FACTOR * T / dl))
        n_cut = int(math.ceil(c * t20p2.CUT_FACTOR * T / dl))
        e = (bd[:nb] + br[:nb]) * ism.air_factor(nb, dl, m, c, None)
        tail, fit = t20p2.extend(e, nb)
        tail_c, fit_c = t20p2.extend(e, n_cut)
        full = np.concatenate([e, tail])[fr:]
        cut = np.concatenate([e[:n_cut], tail_c])[fr:]
        m_full.append(ref_mtf(full, DT_FINE))
        m_cut.append(ref_mtf(cut, DT_FINE))
        d_full.append(len(full) * DT_FINE)
        d_cut.append(len(cut) * DT_FINE)
        tot.append(float(np.sum(np.concatenate([e, tail]))))
        ok.append(bool(fit['ok'] and fit_c['ok']))
    res = dict(m_full=np.array(m_full), m_cut=np.array(m_cut), d_full=np.array(d_full), d_cut=np.array(d_cut),
               total_per_unit_power=np.array(tot), tail_ok=np.array(ok), t60=np.array(t60), from_bin=np.array(fr),
               fine_bins=np.array(len(bd)), seconds=np.array(time.time() - t0))
    np.savez(f, **res)
    say('reference %s %s: %.1f s, %d fine bins' % (room, label, time.time() - t0, len(bd)))
    return res


def c7_job(args):
    return c7_reference(*args, say=print)


def set_c7(out, say, workers):
    import run_c
    import scorebc
    data = DATA['c7']
    allruns = scorebc.runs(data)
    rooms = list(run_c.ROOMS)
    cache = REFERENCES or OUT_ROOT / 'c7' / 'references'   # shared by the frozen and the padded scoring
    cache.mkdir(parents=True, exist_ok=True)
    rows, info, refs, ctx, rc_dt = [], {}, {}, {}, {}
    for room in rooms:
        need = [('tested', room, s) for s in TESTED_SEEDS]
        bad = [k for k in need if k not in allruns or allruns[k]['run_exit'] != 0 or not allruns[k]['run_folder']]
        if bad:
            raise RuntimeError('room %s: runs not finished or failed: %s' % (room, bad))
        reps = {k: scorebc.report(allruns[k]['run_folder'], out, '%s-%s-%d' % k, SIMPA) for k in need}
        info[room] = {'%s-%d' % (k[0], k[2]): scorebc.solver_stderr(allruns[k]['run_folder']) for k in need}
        L, a6, src, recs = run_c.geometry(room)
        rep0 = reps[need[0]]
        c, R = rep0['spps']['speed_of_sound_m_s'], rep0['spps']['receiver_radius_m']
        rc_dt[room] = rep0['spps']['time_step_s']
        atm = scorebc.atmosphere(allruns[need[0]]['run_folder'])
        rc = {k: receivers(reps[k]) for k in need}
        for label, pr in rc[need[0]].items():
            idx = int(label.lstrip('Rr'))
            assert max(abs(p - q) for p, q in zip(pr['position_m'], recs[idx])) < 1e-6, (room, label)
            ctx[(room, label)] = (room, label, recs[idx], L, a6, src, R, c, atm, cache)
        ctx[room] = (rc, R)
    keys = [k for k in ctx if isinstance(k, tuple)]
    with ProcessPoolExecutor(min(workers, len(keys))) as ex:
        for k, r in zip(keys, ex.map(c7_job, [ctx[k] for k in keys])):
            refs[k] = r
    for room in rooms:
        rc, R = ctx[room]
        for (rm, label) in [k for k in keys if k[0] == room]:
            ref = refs[(rm, label)]
            ok_tail = bool(np.all(ref['tail_ok']))
            for seed in TESTED_SEEDS:
                pr = rc[('tested', room, seed)][label]
                d_full, d_cut = ref['d_full'], ref['d_cut']
                if PAD:      # ADDENDUM-4 item 2 (post hoc): zeros to the tested run's length from its arrival bin
                    b0 = pr['_bands'][1000]
                    run_len = (len(b0['energy_pa2']) - leading_bin(b0['arrival'], rc_dt[room])) * rc_dt[room]
                    d_full, d_cut = np.maximum(d_full, run_len), np.maximum(d_cut, run_len)
                spr = [pr['_bands'][f]['source_power_rho_c'] for f in BANDS]
                own_tr = [transfer_db(t * s / (4.0 / 3.0 * math.pi * R ** 3), s)
                          for t, s in zip(ref['total_per_unit_power'], spr)]
                for sex in SEXES:
                    L_s, L_n = product_levels(pr, sex)
                    v, code = sti_value(pr['sti'][sex])
                    if np.isnan(L_s[NEEDS[sex]]).any():
                        full = cut = own = None
                        why = 'no_product_levels'
                    else:
                        full, why = ref_sti(ref['m_full'], d_full, sex, L_s, L_n, float(ref['t60'].max()))
                        cut, _ = ref_sti(ref['m_cut'], d_cut, sex, L_s, L_n, float(ref['t60'].max()))
                        own, _ = ref_sti(ref['m_full'], d_full, sex, SPEECH[sex] + np.asarray(own_tr), L_n,
                                         float(ref['t60'].max()))
                    u = abs(full - cut) if full is not None and cut is not None else None
                    ex_ = exclusion(v, u, ok_tail)
                    rows.append(dict(room=room, label=label, seed=seed, sex=sex, product=v, product_code=code,
                                     reference=full, reference_refusal=why, u=u, reference_own_levels=own,
                                     excluded=ex_, noise=pr['sti']['noise'][:5]))
        say('room %s: %d rows' % (room, sum(1 for r in rows if r['room'] == room)))
    return rows, score_bc(rows, rooms), info


def main():
    global PAD, SIMPA, TESTED_SEEDS, REFERENCES, CACHE_ONLY
    ap = argparse.ArgumentParser()
    ap.add_argument('set', choices=('a-s1', 'a-s2', 'b7', 'c7'))
    ap.add_argument('--out')
    ap.add_argument('--workers', type=int, default=4)
    ap.add_argument('--smoke', type=int, default=0)
    ap.add_argument('--pad', action='store_true', help='c7 only, ADDENDUM-4 item 2 (post hoc): the reference '
                    'series padded with zeros to the length of the tested run; default out STI/c7-padded')
    ap.add_argument('--simpa', help='the simpa.exe whose `results --json` is scored (default %s)' % SIMPA)
    ap.add_argument('--seeds', help='b7/c7: the tested seeds, comma-separated (default %s); a fresh draw is '
                    'scored by the same rules' % ','.join(map(str, TESTED_SEEDS)))
    ap.add_argument('--references', help='c7: the folder of cached references to read; a missing one is an '
                    'error, never computed')
    a = ap.parse_args()
    PAD = a.pad
    if a.simpa:
        SIMPA = Path(a.simpa)
    if a.seeds:
        TESTED_SEEDS = tuple(int(s) for s in a.seeds.split(','))
    if a.references:
        REFERENCES, CACHE_ONLY = Path(a.references), True
    out = Path(a.out) if a.out else OUT_ROOT / (a.set + ('-padded' if a.pad else ''))
    if a.smoke:
        out = out / 'smoke'
    out.mkdir(parents=True, exist_ok=True)
    log = open(out / 'run.log', 'a', encoding='utf-8')

    def say(msg):
        line = '%s %s' % (time.strftime('%H:%M:%S'), msg)
        print(line, flush=True)
        log.write(line + '\n')
        log.flush()

    head = subprocess.run(['git', 'rev-parse', 'HEAD'], cwd=REPO, capture_output=True, text=True).stdout.strip()
    ver = subprocess.run([str(SIMPA), '--version'], capture_output=True, text=True).stdout.strip()
    hashes = dict(head=head, reference=REF.SHA256, addendum2=sha(BED / 'ADDENDUM-2-STI.md'),
                  addendum3=sha(BED / 'ADDENDUM-3.md'), stibed=sha(__file__),
                  shim=sha(REPO / 'crates' / 'simpa-core' / 'tests' / 'bed_shim.rs'),
                  sti_rs=sha(REPO / 'crates' / 'simpa-core' / 'src' / 'params' / 'sti.rs'), simpa=str(SIMPA),
                  simpa_version=ver, tested_seeds=list(TESTED_SEEDS),
                  references=str(REFERENCES) if REFERENCES else None)
    mode = 'SMOKE' if a.smoke else ('POST-HOC (ADDENDUM-4 item 2)' if a.pad else 'SCORED')
    say('%s %s -> %s; %s' % (mode, a.set, out, json.dumps(hashes)))
    info = None
    if a.set.startswith('a-'):
        rows, sc = set_a(a.set, out, a.workers, a.smoke, say)
    elif a.set == 'b7':
        rows, sc, info = set_b7(out, say)
    else:
        rows, sc, info = set_c7(out, say, a.workers)
    with open(out / 'rows.jsonl', 'w', encoding='utf-8') as f:
        for r in rows:
            f.write(json.dumps(r, default=str) + '\n')
    res = dict(mode=mode, set=a.set, hashes=hashes, score=sc, solver_stderr=info,
               pass_=sc['all']['pass_'] if not a.smoke else None)
    (out / 'summary.json').write_text(json.dumps(res, indent=1, default=str), encoding='utf-8')
    x = sc['all']
    say('%s %s: %s' % (mode, a.set, json.dumps({k: x.get(k) for k in (
        'rows', 'both', 'answered', 'share', 'within_0_01', 'beyond_0_03', 'wrong_silent', 'max_abs_diff',
        'min_room_share', 'excluded', 'product_only', 'reference_only', 'max_level_diff_db',
        'sensitivity_own_levels_max', 'pass_') if k in x}, default=str)))


if __name__ == '__main__':
    main()
