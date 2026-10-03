"""G and dB(A) end to end on set C (../ADDENDUM-6-GDBA.md, written before any G or dB(A) row was scored).

Truths, from the standards, in Python (no product code read or ported for them):
- G per receiver and band (ISO 3382-1:2009 A.2.1, Eqs. A.1-A.3): the image-source reference's band SPL (set C's cached
  references, copied from C-score-F, never recomputed) minus the same source's free-field level at 10 m,
  L_10 = 10 lg(W rho c / (4 pi 10^2) / p0^2), W the band power the run's config.xml gives the solver (dB re 1 pW),
  rho c of dry air from the run's temperature and pressure (ideal gas, R = 287.05287 J/(kg K) of ISO 2533,
  gamma = 1.4): rho c = P sqrt(gamma / (R T)). No air absorption over the 10 m (the addendum names rho c only).
- dB(A) per receiver: 10 lg sum_k 10^((SPL_k + A_k) / 10) over the run's bands, A_k typed from IEC 61672-1:2013
  Table 3 (nominal frequencies); the test checks them against the standard's Annex E closed form.
Truth uncertainty: G takes the reference SPL's own (the free-field term is exact); dB(A) the energy-share-weighted sum
of the bands' (all errors the same sign: an upper bound to first order). Excluded (counted): truth not finite, tail fit
unsettled, or uncertainty > 0.1 dB.

Tested values: each tested run's report. Primary: the reports the C scorer already read (C-score-F/reports, build F,
917345c). With --recheck EXE the six reports are re-made with that build into C-score-GdBA/reports-<tag>/ and every G
and dB(A) value compared with the primary's.

Pass (the bed's, unchanged): limen 1 dB both; wrong-silent = answered, |value - truth| > 1 dB and the truth outside
[lo, hi]; covered = truth inside [lo, hi] widened by 0.1 dB; pass = no wrong-silent and coverage >= 90 % of answered.

  python -B gdba.py [--recheck EXE]
"""
import sys

sys.dont_write_bytecode = True

import argparse  # noqa: E402
import json  # noqa: E402
import math  # noqa: E402
import subprocess  # noqa: E402
import time  # noqa: E402
import xml.etree.ElementTree as ET  # noqa: E402
from pathlib import Path  # noqa: E402

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))

DATA = Path(r'B:\data\m8b-bed\C')
PRIMARY_REPORTS = Path(r'B:\data\m8b-bed\C-score-F\reports')
OUT = Path(r'B:\data\m8b-bed\C-score-GdBA')
ROOMS = ('S-live', 'Mixed')
SEEDS = (4101, 4102, 4103)
LIMEN = 1.0           # ISO 3382-1 Table A.1 (G); dB(A) takes SPL's 1 dB (ADDENDUM-6)
WITHIN = 0.1          # 1/10 limen
COVERAGE_MIN = 0.90
ANSWERED_FLAG = 0.80

# ---- the standards ---------------------------------------------------------------------------------------------------
P0_SQUARED = (20e-6) ** 2          # ISO 3382-1 A.2.1: p0 = 20 uPa
W0 = 1e-12                         # reference power, 1 pW
R_AIR = 287.05287                  # J/(kg K), dry air, ISO 2533
GAMMA_AIR = 1.4

# IEC 61672-1:2013 Table 3, A-weighting at the nominal frequencies, dB (typed from the standard's table; the standard
# itself is not in hand on this machine (../../2026-10-02-m8b-metrics/STANDARDS-CHECK.md line 11), so test_gdba.py
# checks every entry against the closed form of the same standard's Annex E at the exact base-10 frequency).
A_WEIGHT_TABLE = {
    10: -70.4, 12.5: -63.4, 16: -56.7, 20: -50.5, 25: -44.7, 31.5: -39.4, 40: -34.6, 50: -30.2, 63: -26.2,
    80: -22.5, 100: -19.1, 125: -16.1, 160: -13.4, 200: -10.9, 250: -8.6, 315: -6.6, 400: -4.8, 500: -3.2,
    630: -1.9, 800: -0.8, 1000: 0.0, 1250: 0.6, 1600: 1.0, 2000: 1.2, 2500: 1.3, 3150: 1.2, 4000: 1.0,
    5000: 0.5, 6300: -0.1, 8000: -1.1, 10000: -2.5, 12500: -4.3, 16000: -6.6, 20000: -9.3,
}


def a_weight(freq_hz):
    """The table's A-weight at a nominal octave or third-octave centre."""
    return A_WEIGHT_TABLE[freq_hz]


def a_weight_annex_e(f):
    """IEC 61672-1 Annex E, Eqs. E.6 and E.1-E.4: A(f) = 20 lg[f4^2 f^4 / ((f^2+f1^2) sqrt(f^2+f2^2) sqrt(f^2+f3^2)
    (f^2+f4^2))] - A1000, with the poles from fL = 10^1.5, fH = 10^3.9, fA = 10^2.45 (Eqs. E.1-E.5); A1000 is the
    same expression at 1 kHz (-2.000 dB)."""
    fr, fl, fh, fa = 1000.0, 10 ** 1.5, 10 ** 3.9, 10 ** 2.45
    d = math.sqrt(0.5)
    b = (fr ** 2 + fl ** 2 * fh ** 2 / fr ** 2 - d * (fl ** 2 + fh ** 2)) / (1 - d)
    c = fl ** 2 * fh ** 2
    f1 = math.sqrt((-b - math.sqrt(b * b - 4 * c)) / 2)      # 20.60 Hz
    f4 = math.sqrt((-b + math.sqrt(b * b - 4 * c)) / 2)      # 12194 Hz
    f2 = (3 - math.sqrt(5)) / 2 * fa                          # 107.7 Hz
    f3 = (3 + math.sqrt(5)) / 2 * fa                          # 737.9 Hz

    def raw(x):
        return 20 * math.log10(f4 ** 2 * x ** 4 / ((x ** 2 + f1 ** 2) * math.sqrt(x ** 2 + f2 ** 2)
                                                    * math.sqrt(x ** 2 + f3 ** 2) * (x ** 2 + f4 ** 2)))
    return raw(f) - raw(1000.0)


def rho_c(T_c, P_pa):
    """Characteristic impedance of dry air, ideal gas: rho = P / (R T), c = sqrt(gamma R T)."""
    T = T_c + 273.15
    return P_pa * math.sqrt(GAMMA_AIR / (R_AIR * T))


def free_field_10m_db(W, rc):
    """ISO 3382-1 A.2.1: the level of the same source in a free field at 10 m, point source, spherical spreading."""
    return 10 * math.log10(W * rc / (4 * math.pi * 10.0 ** 2) / P0_SQUARED)


def g_truth(spl_db, W, rc):
    return spl_db - free_field_10m_db(W, rc)


def dba_truth(spl_by_band):
    """{freq: SPL} -> (dB(A), energy shares of the A-weighted bands)."""
    e = {f: 10 ** ((s + a_weight(f)) / 10) for f, s in spl_by_band.items()}
    tot = sum(e.values())
    return 10 * math.log10(tot), {f: v / tot for f, v in e.items()}


# ---- reading the runs ------------------------------------------------------------------------------------------------
def config(run_folder):
    root = ET.parse(Path(run_folder) / 'solve' / 'config.xml').getroot()
    a = root.find('condition_atmospherique')
    srcs = root.find('sources').findall('source')
    assert len(srcs) == 1, run_folder
    lw = {int(b.get('freq')): float(b.get('db')) for b in srcs[0].findall('bfreq')}
    return dict(T_c=float(a.get('temperature')), H=float(a.get('humidite')), P=float(a.get('pression')), lw_db=lw)


def read_param(p):
    """(value, lo, hi, status, code) of one report entry (docs/formats/results-json.md); a refusal has no value."""
    if p is None:
        return None, None, None, 'absent', 'absent'
    if p.get('value') is not None:
        return float(p['value']), p.get('lo'), p.get('hi'), p.get('status'), None
    for v in p.values():
        if isinstance(v, dict) and 'code' in v:
            return None, None, None, 'refused', v['code']
    return None, None, None, 'refused', 'unknown'


def classify(value, lo, hi, truth, lim=LIMEN):
    if value is None:
        return dict(answered=False, wrong_silent=False, covered=False)
    err = abs(value - truth)
    ranged = lo is not None and hi is not None
    inside = ranged and lo <= truth <= hi
    covered = (lo - WITHIN * lim <= truth <= hi + WITHIN * lim) if ranged else err <= WITHIN * lim
    return dict(answered=True, wrong_silent=bool(err > lim and not inside), covered=bool(covered))


def exclusion(truth, u, tail_ok):
    if truth is None or not math.isfinite(truth):
        return 'truth_nan'
    if not tail_ok:
        return 'truth_truncated'
    if u is None or not math.isfinite(u) or u > WITHIN * LIMEN:
        return 'truth_uncertain'
    return None


def done_runs():
    out = {}
    for d in sorted(DATA.iterdir()):
        f = d / 'done.json'
        if d.is_dir() and f.exists():
            r = json.loads(f.read_text())
            out[(r['room'], r['seed'])] = r
    return out


def make_report(exe, run_folder, out_file):
    if not out_file.exists():
        p = subprocess.run([str(exe), 'results', str(run_folder), '--json'], capture_output=True, text=True)
        out_file.with_suffix('.stderr.txt').write_text(p.stderr, encoding='utf-8')
        if p.returncode != 0:
            raise RuntimeError('%s: simpa results exit %d' % (run_folder, p.returncode))
        out_file.write_text(p.stdout, encoding='utf-8')
    return json.loads(out_file.read_text(encoding='utf-8'))


def product_values(rep):
    """{label: dict(bands={f: (spl param, g param)}, dba=param, W={f: W}, spr={f: W rho c}, dba_meta)}."""
    sp = rep['spps']
    assert len(sp['sources']) == 1
    W = dict(zip(rep['bands_hz'], sp['sources'][0]['band_power_w']))
    out = {}
    for pr in sp['point_receivers']:
        bands = {b['freq_hz']: dict(spl=b['parameters'].get('spl_db'), g=b.get('g_db'),
                                    spr=b.get('source_power_rho_c')) for b in pr['bands']}
        dba = (pr.get('aggregate') or {}).get('dba') or {}
        out[pr['label']] = dict(bands=bands, dba=dba.get('level_db'), dba_meta={k: v for k, v in dba.items()
                                                                                if k != 'level_db'},
                                W=W, position=pr['position_m'])
    return out


# ---- scoring ---------------------------------------------------------------------------------------------------------
def score(rows, metric):
    mr = [r for r in rows if r['metric'] == metric]
    ok = [r for r in mr if r['excluded'] is None]
    for r in ok:
        r.update(classify(r['value'], r['lo'], r['hi'], r['truth']))
        r['diff'] = None if r['value'] is None else r['value'] - r['truth']
    ans = [r for r in ok if r['answered']]
    ws = [r for r in ans if r['wrong_silent']]
    cov = sum(1 for r in ans if r['covered'])
    rooms = {}
    for room in ROOMS:
        rr = [r for r in ok if r['room'] == room]
        a = sum(1 for r in rr if r['answered'])
        rooms[room] = dict(rows=len(rr), answered=a, share=a / len(rr) if rr else None,
                           flag_below_80=bool(rr and a / len(rr) < ANSWERED_FLAG))
    excl, refusals = {}, {}
    for r in mr:
        if r['excluded']:
            excl[r['excluded']] = excl.get(r['excluded'], 0) + 1
    for r in ok:
        if not r['answered']:
            refusals[r['code']] = refusals.get(r['code'], 0) + 1
    worst = max(ans, key=lambda r: abs(r['diff'])) if ans else None
    coverage = cov / len(ans) if ans else None
    pass_ = bool(ans and not ws and coverage >= COVERAGE_MIN)
    failing = []
    if not ans:
        failing.append('no answered row')
    if ws:
        failing.append('%d wrong-silent' % len(ws))
    if ans and coverage < COVERAGE_MIN:
        failing.append('coverage %.3f < %.2f' % (coverage, COVERAGE_MIN))
    return dict(rows=len(mr), excluded=excl, scored=len(ok), answered=len(ans),
                answered_share=len(ans) / len(ok) if ok else None, wrong_silent=len(ws),
                wrong_silent_rows=[(r['room'], r['label'], r.get('band_hz'), r['seed'], r['value'], r['truth'],
                                    r['lo'], r['hi']) for r in ws[:20]],
                covered=cov, coverage=coverage, by_room=rooms, refusals=refusals,
                worst_abs_diff=None if worst is None else abs(worst['diff']),
                worst_row=None if worst is None else {k: worst.get(k) for k in (
                    'room', 'label', 'band_hz', 'seed', 'value', 'lo', 'hi', 'status', 'truth', 'u')},
                mean_diff=sum(r['diff'] for r in ans) / len(ans) if ans else None,
                status_counts={s: sum(1 for r in ans if r['status'] == s) for s in sorted({r['status'] for r in ans})},
                pass_=pass_, failing=failing)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--recheck', metavar='EXE', help='re-make the reports with this simpa and compare')
    a = ap.parse_args()
    refs = OUT / 'references'
    if not refs.is_dir():
        raise SystemExit('copy C-score-F/references to %s first (never recomputed)' % refs)
    log = open(OUT / 'run.log', 'a', encoding='utf-8')

    def say(msg):
        line = '%s %s' % (time.strftime('%H:%M:%S'), msg)
        print(line, flush=True)
        log.write(line + '\n')
        log.flush()

    head = subprocess.run(['git', '-C', str(HERE), 'rev-parse', 'HEAD'], capture_output=True, text=True).stdout.strip()
    say('SCORED G and dB(A) set C; head %s; primary reports %s' % (head, PRIMARY_REPORTS))
    runs = done_runs()
    rows, freefield, info = [], [], {}
    for room in ROOMS:
        for seed in SEEDS:
            r = runs[(room, seed)]
            assert r['run_exit'] == 0 and r['run_folder'], (room, seed)
            rid = 'tested-%s-%d' % (room, seed)
            rep = json.loads((PRIMARY_REPORTS / ('%s.json' % rid)).read_text(encoding='utf-8'))
            assert Path(rep['run_folder']).resolve() == Path(r['run_folder']).resolve() or \
                Path(rep['run_folder']).name == Path(r['run_folder']).name, (rid, rep['run_folder'])
            cfg = config(r['run_folder'])
            rc_t = rho_c(cfg['T_c'], cfg['P'])
            pv = product_values(rep)
            info[rid] = dict(results_version=rep.get('results_version'), run_folder=r['run_folder'],
                             atmosphere={k: cfg[k] for k in ('T_c', 'H', 'P')}, rho_c_truth=rc_t,
                             lw_config_db=cfg['lw_db'])
            for label, x in sorted(pv.items()):
                spl_ref, u_ref, tail_ok = {}, {}, True
                for f, b in sorted(x['bands'].items()):
                    ref = json.loads((refs / ('%s-%s-%d.json' % (room, label, f))).read_text())
                    assert ref['atmosphere'] == {k: cfg[k] for k in ('T_c', 'H', 'P')}, (rid, label, f)
                    spl_t, u = ref['truth']['spl']
                    spl_ref[f], u_ref[f] = spl_t, u
                    tail_ok = tail_ok and ref['info']['tail_ok']
                    W_cfg = W0 * 10 ** (cfg['lw_db'][f] / 10)
                    W_rep = x['W'][f]
                    rc_p = b['spr'] / W_rep
                    lff_t = free_field_10m_db(W_cfg, rc_t)
                    sv, slo, shi, sst, scode = read_param(b['spl'])
                    gv, glo, ghi, gst, gcode = read_param(b['g'])
                    lff_p = (sv - gv) if (sv is not None and gv is not None) else None
                    freefield.append(dict(room=room, label=label, seed=seed, band_hz=f, W_config=W_cfg,
                                          W_report=W_rep, rho_c_truth=rc_t, rho_c_product=rc_p,
                                          lff_truth=lff_t, lff_product=lff_p,
                                          lff_product_from_spr=10 * math.log10(b['spr'] / (400 * math.pi) / P0_SQUARED),
                                          diff=None if lff_p is None else lff_p - lff_t))
                    truth = g_truth(spl_t, W_cfg, rc_t)
                    rows.append(dict(metric='g_db', room=room, label=label, band_hz=f, seed=seed, value=gv, lo=glo,
                                     hi=ghi, status=gst, code=gcode, truth=truth, u=u, spl_ref=spl_t,
                                     lff_truth=lff_t, lff_product=lff_p,
                                     excluded=exclusion(truth, u, ref['info']['tail_ok'])))
                dv, dlo, dhi, dst, dcode = read_param(x['dba'])
                dt, share = dba_truth(spl_ref)
                du = sum(share[f] * u_ref[f] for f in share)
                rows.append(dict(metric='dba', room=room, label=label, band_hz=None, seed=seed, value=dv, lo=dlo,
                                 hi=dhi, status=dst, code=dcode, truth=dt, u=du,
                                 weights_truth=[a_weight(f) for f in sorted(spl_ref)],
                                 weights_product=x['dba_meta'].get('weights_db'),
                                 bands_product=x['dba_meta'].get('bands_hz'),
                                 unweighted_product=x['dba_meta'].get('unweighted_hz'),
                                 excluded=exclusion(dt, du, tail_ok)))
    sc = {m: score(rows, m) for m in ('g_db', 'dba')}
    # the free-field constant, per band: truth and product side by side, never adjusted
    ff_band = {}
    for f in sorted({x['band_hz'] for x in freefield}):
        fx = [x for x in freefield if x['band_hz'] == f]
        ff_band[f] = dict(lff_truth=sorted({round(x['lff_truth'], 9) for x in fx}),
                          lff_product=sorted({round(x['lff_product'], 9) for x in fx if x['lff_product'] is not None}),
                          lw_minus_lff_truth=sorted({round(10 * math.log10(x['W_config'] / W0) - x['lff_truth'], 6)
                                                     for x in fx}),
                          lw_minus_lff_product=sorted({round(10 * math.log10(x['W_report'] / W0) - x['lff_product'], 6)
                                                       for x in fx if x['lff_product'] is not None}),
                          rho_c_truth=sorted({round(x['rho_c_truth'], 6) for x in fx}),
                          rho_c_product=sorted({round(x['rho_c_product'], 6) for x in fx}),
                          W_config_vs_report_max_rel=max(abs(x['W_report'] / x['W_config'] - 1) for x in fx),
                          max_abs_diff_db=max(abs(x['diff']) for x in fx if x['diff'] is not None))
    dba_w = [r for r in rows if r['metric'] == 'dba']
    weights_match = all(r['weights_product'] == r['weights_truth'] and r['bands_product'] == sorted(
        {x['band_hz'] for x in freefield}) for r in dba_w)
    recheck = None
    if a.recheck:
        exe = Path(a.recheck)
        ver = subprocess.run([str(exe), '--version'], capture_output=True, text=True).stdout.strip()
        rd = OUT / ('reports-%s' % exe.parent.parent.name)
        rd.mkdir(exist_ok=True)
        dmax = {'g_db': 0.0, 'dba': 0.0}
        status_changes = []
        for room in ROOMS:
            for seed in SEEDS:
                rid = 'tested-%s-%d' % (room, seed)
                pv2 = product_values(make_report(exe, runs[(room, seed)]['run_folder'], rd / ('%s.json' % rid)))
                for r in rows:
                    if r['room'] != room or r['seed'] != seed:
                        continue
                    x = pv2[r['label']]
                    p = x['dba'] if r['metric'] == 'dba' else x['bands'][r['band_hz']]['g']
                    v2, lo2, hi2, st2, _ = read_param(p)
                    if (v2 is None) != (r['value'] is None) or st2 != r['status']:
                        status_changes.append((rid, r['label'], r['band_hz'], r['metric'], r['status'], st2))
                    if v2 is not None and r['value'] is not None:
                        dmax[r['metric']] = max(dmax[r['metric']], abs(v2 - r['value']),
                                                abs((lo2 or 0) - (r['lo'] or 0)), abs((hi2 or 0) - (r['hi'] or 0)))
        recheck = dict(simpa=str(exe), simpa_version=ver, reports=str(rd), max_change=dmax,
                       status_changes=status_changes)
        say('recheck with %s (%s): max change %s, status changes %d' % (exe, ver, json.dumps(dmax),
                                                                       len(status_changes)))
    verdict = {m: dict(pass_=sc[m]['pass_'], failing=sc[m]['failing'], wrong_silent=sc[m]['wrong_silent'],
                       coverage=sc[m]['coverage'], answered=sc[m]['answered_share']) for m in sc}
    for m in verdict:
        verdict[m]['pass'] = verdict[m].pop('pass_')
    res = dict(mode='SCORED', set='c', addendum='docs/investigations/2026-10-02-bed/ADDENDUM-6-GDBA.md',
               head=head, reports=str(PRIMARY_REPORTS), simpa=r'C:\tmp\nm-target-f\release\simpa.exe',
               simpa_note='primary reports made by the C scorer with build F (917345c), C-score-F/run.log',
               score=sc, freefield_by_band=ff_band, dba_weights_match_product=weights_match, recheck=recheck,
               info=info, pass_={m: sc[m]['pass_'] for m in sc}, verdict=verdict)
    (OUT / 'summary.json').write_text(json.dumps(res, indent=1, default=str), encoding='utf-8')
    with open(OUT / 'rows.jsonl', 'w', encoding='utf-8') as fh:
        for r in rows:
            fh.write(json.dumps(r, default=str) + '\n')
    with open(OUT / 'freefield.jsonl', 'w', encoding='utf-8') as fh:
        for x in freefield:
            fh.write(json.dumps(x) + '\n')
    say('SCORED: %s' % json.dumps(verdict))


if __name__ == '__main__':
    main()
