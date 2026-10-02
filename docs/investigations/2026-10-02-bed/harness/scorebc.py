"""The bed's sets B and C scorer (../PREREG.md; ../ADDENDUM-1.md items 4-10).

Tested values: `simpa results <run folder> --json` (the CURRENT build: C:/tmp/nm-target-e1 when it exists, else
nm-target-d; the one used is recorded), read through harness2's `driver.series_from_report` for the series and the
report's `parameters` for the values and shown ranges.
- Set B truths: the two truth seeds' (9201, 9202) histograms of the same receiver and band, the metric on their mean
  (ADDENDUM-1 item 8), uncertainty half the difference of the two seeds' own metrics. The direct sound is every energy
  before t0 + R/c, at u = 0 (item 4).
- Set C references: harness2's `_ism.echogram` (through ism_fresh's hash gate) of the ball, with the run's own
  recorded air (config.xml's temperature, humidity and pressure) continuous, to 2.5 x the band's Eyring T60 with
  T20 part 2's fitted tail; uncertainty the 1.5 x cut's truth against the 2.5 x one (items 3, 9). Pa^2 by
  source_power_rho_c / (4/3 pi R^3) (item 6).
Rows: room x receiver x band x tested seed, per metric. Excluded (counted): truth not finite, tail fit unsettled, or
truth uncertainty > 1/10 limen. Wrong-silent and covered as PREREG.md defines them (`classify`).

  python -B scorebc.py b|c [--dry ROOM] [--data DIR] [--out DIR] [--simpa EXE]

--dry ROOM reads that one finished room, builds its rows and prints row counts and exclusions only: no tested value is
compared with its truth and no verdict is formed. Without --dry the rows are scored (not to be run before the bed's
order says so).
"""
import sys

sys.dont_write_bytecode = True

import argparse  # noqa: E402
import json  # noqa: E402
import math  # noqa: E402
import re  # noqa: E402
import subprocess  # noqa: E402
import time  # noqa: E402
import xml.etree.ElementTree as ET  # noqa: E402
from pathlib import Path  # noqa: E402

import numpy as np  # noqa: E402

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import seta  # noqa: E402  (puts T20 part 2's harness and harness2 on sys.path)
import t20p2  # noqa: E402
import truth20  # noqa: E402
import truthbed as tb  # noqa: E402
from m8b import driver, ism_fresh, rooms2  # noqa: E402

DATA = dict(b=Path(r'B:\data\m8b-bed\B'), c=Path(r'B:\data\m8b-bed\C'))
OUT = dict(b=Path(r'B:\data\m8b-bed\B-score'), c=Path(r'B:\data\m8b-bed\C-score'))
SIMPA_E1 = Path(r'C:\tmp\nm-target-e1\release\simpa.exe')
SIMPA_D = Path(r'C:\tmp\nm-target-d\release\simpa.exe')
TRUTH_SEEDS = (9201, 9202)
TESTED_SEEDS = (4101, 4102, 4103)
METRICS = seta.METRICS
SCORED = seta.SCORED
PKEY = dict(t20='t20_s', t30='t30_s', ts='ts_s', c50='c50_db', c80='c80_db', d50='d50', spl='spl_db')
WITHIN = 0.1
COVERAGE_MIN = 0.90
ANSWERED_FLAG = 0.80
DT_FINE = t20p2.DT_FINE


# ---- the report -----------------------------------------------------------------------------------------------------
def read_param(p):
    """(value, lo, hi, status, code) of one `parameters` entry (docs/formats/results-json.md)."""
    if p is None:
        return None, None, None, 'absent', 'absent'
    if 'value' in p and p['value'] is not None:
        return float(p['value']), p.get('lo'), p.get('hi'), p.get('status'), None
    for k, v in p.items():
        if isinstance(v, dict) and 'code' in v:
            why = ((v.get('error') or {}).get('why') or {}).get('why')
            return None, None, None, 'refused', v['code'] + (':' + why if why else '')
    return None, None, None, 'refused', 'unknown'


def classify(value, lo, hi, truth, lim):
    """PREREG.md: wrong-silent = answered, |value - truth| > limen and the truth outside [lo, hi] (no range: the value
    only); covered = the truth inside [lo, hi] widened by 1/10 limen (no range: |value - truth| <= 1/10 limen)."""
    if value is None:
        return dict(answered=False, wrong_silent=False, covered=False)
    err = abs(value - truth)
    ranged = lo is not None and hi is not None
    inside = ranged and lo <= truth <= hi
    covered = (lo - WITHIN * lim <= truth <= hi + WITHIN * lim) if ranged else err <= WITHIN * lim
    return dict(answered=True, wrong_silent=bool(err > lim and not inside), covered=bool(covered))


def exclusion(truth, u, lim_fn, tail_ok=True):
    if truth is None or not math.isfinite(truth):
        return 'truth_nan'
    if not tail_ok:
        return 'truth_truncated'
    if u is None or not math.isfinite(u) or u > WITHIN * lim_fn(truth):
        return 'truth_uncertain'
    return None


def simpa_exe(arg=None):
    if arg:
        return Path(arg)
    return SIMPA_E1 if SIMPA_E1.exists() else SIMPA_D


def report(run_folder, out_dir, rid, exe):
    """`simpa results <folder> --json`, saved as out_dir/reports/<rid>.json; its stderr beside it."""
    d = out_dir / 'reports'
    d.mkdir(parents=True, exist_ok=True)
    f = d / ('%s.json' % rid)
    if not f.exists():
        p = subprocess.run([str(exe), 'results', str(run_folder), '--json'], capture_output=True, text=True)
        (d / ('%s.stderr.txt' % rid)).write_text(p.stderr, encoding='utf-8')
        if p.returncode != 0:
            raise RuntimeError('%s: simpa results exit %d: %s' % (rid, p.returncode, p.stderr[-2000:]))
        f.write_text(p.stdout, encoding='utf-8')
    return json.loads(f.read_text(encoding='utf-8'))


def solver_stderr(run_folder):
    """The solver's stderr, read: its lines by class (PROGRESS lines are counted, not kept)."""
    p = Path(run_folder) / 'solver.stderr.txt'
    if not p.exists():
        return dict(missing=True)
    cls = {}
    keep = []
    for line in p.read_text(encoding='utf-8', errors='replace').splitlines():
        c = line.split(' ', 1)[0] if line.strip() else ''
        cls[c] = cls.get(c, 0) + 1
        if c not in ('PROGRESS', 'OK', 'INFO', ''):
            keep.append(line[:200])
    return dict(classes=cls, notable=keep[:20])


def runs(data, room=None):
    """{(kind, room, seed): done.json} of the finished runs (run_exit 0 and a run folder)."""
    out = {}
    for d in sorted(data.iterdir()):
        f = d / 'done.json'
        if d.is_dir() and f.exists():
            r = json.loads(f.read_text())
            if room is None or r['room'] == room:
                out[(r['kind'], r['room'], r['seed'])] = r
    return out


def series(rep):
    """{(label, band): series} via harness2's reader, plus the band's source_power_rho_c and parameters."""
    out = {}
    s = driver.series_from_report(rep)
    for x in s:
        out[(x['label'], x['band_hz'])] = x
    for pr in rep['spps']['point_receivers']:
        for b in pr['bands']:
            out[(pr['label'], b['freq_hz'])].update(parameters=b['parameters'], spr=b.get('source_power_rho_c'),
                                                   position=pr['position_m'])
    return out


# ---- truths ---------------------------------------------------------------------------------------------------------
def metrics_spps(bins, dt, t0, h):
    """ADDENDUM-1 items 4, 5 on an SPPS histogram: energy before t0 + h is the direct sound, at u = 0."""
    b = np.asarray(bins, float)
    n = len(b)
    lo = np.arange(n) * dt
    frac_after = np.clip(((lo + dt) - np.maximum(lo, t0 + h)) / dt, 0.0, 1.0)
    refl = b * frac_after
    direct = b - refl
    out = dict(t20=truth20.t20(direct, refl, t0, dt)[0], t30=truth20.t20(direct, refl, t0, dt, bottom_db=-35.0)[0])
    out.update(tb.binned(0.0, b, dt, t0, t_from=t0 + h))
    return out


def truth_b(a, b, dt, t0, h):
    """Item 8: the metric on the two seeds' mean; uncertainty half the two seeds' difference."""
    a, b = np.asarray(a, float), np.asarray(b, float)
    pooled = metrics_spps(0.5 * (a + b), dt, t0, h)
    m1, m2 = metrics_spps(a, dt, t0, h), metrics_spps(b, dt, t0, h)
    return {m: (pooled[m], abs(m1[m] - m2[m]) / 2) for m in METRICS}


def atmosphere(run_folder):
    c = ET.parse(Path(run_folder) / 'solve' / 'config.xml').getroot().find('condition_atmospherique')
    return dict(T_c=float(c.get('temperature')), H=float(c.get('humidite')), P=float(c.get('pression')))


def eyring(L, a6, m):
    Lx, Ly, Lz = L
    areas = [Ly * Lz, Ly * Lz, Lx * Lz, Lx * Lz, Lx * Ly, Lx * Ly]
    S, V = sum(areas), Lx * Ly * Lz
    abar = sum(a * s for a, s in zip(a6, areas)) / S
    return 0.161 * V / (-S * math.log(1 - abar) + 4 * m * V)


def truth_c(L, a6, src, rec, R, F, c, atm, scale):
    """Items 3, 9: the ball's image-source echogram with the run's air, its metrics and their uncertainty."""
    ism = ism_fresh.generator()
    m = ism.iso9613_db_per_m(float(F), H=atm['H'], P=atm['P'], T_c=atm['T_c']) * math.log(10) / 10
    t60 = eyring(L, a6, m)
    dl = c * DT_FINE
    alpha = [(a6[0], a6[1]), (a6[2], a6[3]), (a6[4], a6[5])]
    bd, br = ism.echogram(L, src, rec, R, alpha, c * t20p2.ECHO_FACTOR * t60, dl)
    nb = len(bd)
    n_cut = int(math.ceil(c * t20p2.CUT_FACTOR * t60 / dl))
    ac = ism.air_factor(nb, dl, m, c, None) * scale
    b_d, b_r = bd * ac, br * ac
    tail, fit = t20p2.extend(b_d + b_r, nb)
    tail_c, fit_c = t20p2.extend(b_d + b_r, n_cut)
    t = math.dist(src, rec) / c
    full = seta.s2_truths(b_d, b_r, nb, tail, t)
    cut = seta.s2_truths(b_d, b_r, n_cut, tail_c, t)
    return {m_: (full[m_], abs(full[m_] - cut[m_])) for m_ in METRICS}, dict(
        eyring_t60=t60, tail_ok=bool(fit['ok'] and fit_c['ok']), tail_t60=fit['t60'], tail_share=fit['share'],
        fine_bins=nb)


# ---- rows -----------------------------------------------------------------------------------------------------------
def rows_for(room, tested, truths, meta):
    """tested: {seed: series dict}; truths: {(label, band): ({metric: (truth, u)}, info)}."""
    rows = []
    for seed, ser in sorted(tested.items()):
        for (label, band), x in sorted(ser.items()):
            tr, info = truths[(label, band)]
            for m in METRICS:
                v, lo, hi, st, code = read_param(x['parameters'].get(PKEY[m]))
                truth, u = tr[m]
                lim_fn = (lambda t, m=m: tb.limen(m, t))
                rows.append(dict(room=room, label=label, band_hz=band, seed=seed, metric=m, value=v, lo=lo, hi=hi,
                                 status=st, code=code, truth=truth, u=u,
                                 excluded=exclusion(truth, u, lim_fn, info.get('tail_ok', True)),
                                 design_t60=meta.get('t60', {}).get(band)))
    return rows


def score(rows):
    """Criterion 2 (no wrong-silent, coverage >= 90 % of answered) and 3 (answered share per metric and room)."""
    out = {}
    for m in METRICS:
        mr = [r for r in rows if r['metric'] == m]
        ok = [r for r in mr if r['excluded'] is None]
        for r in ok:
            r.update(classify(r['value'], r['lo'], r['hi'], r['truth'], tb.limen(m, r['truth'])))
        ans = [r for r in ok if r['answered']]
        ws = [r for r in ans if r['wrong_silent']]
        cov = sum(1 for r in ans if r['covered'])
        rooms = {}
        for room in sorted({r['room'] for r in mr}):
            rr = [r for r in ok if r['room'] == room]
            a = sum(1 for r in rr if r['answered'])
            t60 = max((r['design_t60'] or 0) for r in rr) if rr else None
            rooms[room] = dict(rows=len(rr), answered=a, share=a / len(rr) if rr else None,
                               flag_below_80=bool(rr and a / len(rr) < ANSWERED_FLAG and (t60 or 0) <= 3.0))
        refusals = {}
        for r in ok:
            if not r['answered']:
                refusals[r['code']] = refusals.get(r['code'], 0) + 1
        excl = {}
        for r in mr:
            if r['excluded']:
                excl[r['excluded']] = excl.get(r['excluded'], 0) + 1
        out[m] = dict(rows=len(mr), excluded=excl, scored=len(ok), answered=len(ans),
                      answered_share=len(ans) / len(ok) if ok else None, wrong_silent=len(ws),
                      wrong_silent_rows=[(r['room'], r['label'], r['band_hz'], r['seed'], r['value'], r['truth'],
                                          r['lo'], r['hi']) for r in ws[:20]],
                      covered=cov, coverage=cov / len(ans) if ans else None, by_room=rooms,
                      refusals=dict(sorted(refusals.items())),
                      pass_=bool(ans and not ws and cov / len(ans) >= COVERAGE_MIN))
    return out


# ---- the sets -------------------------------------------------------------------------------------------------------
def set_b(data, out, exe, room_only, say):
    allruns = runs(data, room_only)
    rooms = sorted({k[1] for k in allruns}) if room_only is None else [room_only]
    geom = rooms2.rooms()
    rows, info = [], {}
    for room in rooms:
        need = [('truth', room, s) for s in TRUTH_SEEDS] + [('tested', room, s) for s in TESTED_SEEDS]
        missing = [k for k in need if k not in allruns or allruns[k]['run_exit'] != 0 or not allruns[k]['run_folder']]
        if missing:
            raise RuntimeError('room %s: runs not finished or failed: %s' % (room, missing))
        reps = {k: report(allruns[k]['run_folder'], out, '%s-%s-%d' % k, exe) for k in need}
        info[room] = dict(solver_stderr={'%s-%d' % (k[0], k[2]): solver_stderr(allruns[k]['run_folder']) for k in need})
        ser = {k: series(reps[k]) for k in need}
        t1, t2 = ser[need[0]], ser[need[1]]
        g = geom[room]
        c = reps[need[0]]['spps']['speed_of_sound_m_s']
        truths = {}
        for key, x in t1.items():
            y = t2[key]
            assert x['dt'] == y['dt'] and x['arrival_s'] == y['arrival_s'] and len(x['energy_pa2']) == len(y['energy_pa2'])
            idx = int(key[0].lstrip('Rr'))
            t_geo = g['receivers'][idx]['d_m'] / c
            if abs(t_geo - x['arrival_s']) > 1e-6:
                raise RuntimeError('%s %s: arrival %r against the geometry\'s %r' % (room, key, x['arrival_s'], t_geo))
            truths[key] = (truth_b(x['energy_pa2'], y['energy_pa2'], x['dt'], x['arrival_s'], x['half_width']), {})
        tested = {s: ser[('tested', room, s)] for s in TESTED_SEEDS}
        rows += rows_for(room, tested, truths, dict(t60=g['design_t60_s']))
        say('room %s: %d rows' % (room, sum(1 for r in rows if r['room'] == room)))
    return rows, info


C_ROOMS = ('S-live', 'Mixed')


def set_c(data, out, exe, room_only, say):
    import run_c
    allruns = runs(data, room_only)
    rooms = list(C_ROOMS) if room_only is None else [room_only]
    rows, info = [], {}
    cache = (out.parent if out.name == 'dry' else out) / 'references'      # shared by dry and scored runs
    cache.mkdir(parents=True, exist_ok=True)
    for room in rooms:
        need = [('tested', room, s) for s in TESTED_SEEDS]
        missing = [k for k in need if k not in allruns or allruns[k]['run_exit'] != 0 or not allruns[k]['run_folder']]
        if missing:
            raise RuntimeError('room %s: runs not finished or failed: %s' % (room, missing))
        reps = {k: report(allruns[k]['run_folder'], out, '%s-%s-%d' % k, exe) for k in need}
        info[room] = dict(solver_stderr={'%s-%d' % (k[0], k[2]): solver_stderr(allruns[k]['run_folder']) for k in need})
        ser = {k: series(reps[k]) for k in need}
        L, a6, src, recs = run_c.geometry(room)
        rep0 = reps[need[0]]
        c = rep0['spps']['speed_of_sound_m_s']
        R = rep0['spps']['receiver_radius_m']
        atm = atmosphere(allruns[need[0]]['run_folder'])
        truths, t60s = {}, {}
        for key, x in ser[need[0]].items():
            idx = int(key[0].lstrip('Rr'))
            pos = x['position']
            assert max(abs(p - q) for p, q in zip(pos, recs[idx])) < 1e-6, (room, key, pos, recs[idx])
            scale = x['spr'] / (4.0 / 3.0 * math.pi * R ** 3)
            f = cache / ('%s-%s-%d.json' % (room, key[0], key[1]))
            if f.exists():
                d = json.loads(f.read_text())
                tr, inf = {m: tuple(v) for m, v in d['truth'].items()}, d['info']
            else:
                t0 = time.time()
                tr, inf = truth_c(L, a6, src, recs[idx], R, key[1], c, atm, scale)
                inf['seconds'] = time.time() - t0
                f.write_text(json.dumps(dict(truth=tr, info=inf, atmosphere=atm, c=c, R=R, scale=scale)))
                say('reference %s %s %d Hz: %.1f s' % (room, key[0], key[1], inf['seconds']))
            truths[key] = (tr, inf)
            t60s[key[1]] = inf['eyring_t60']
        tested = {s: ser[('tested', room, s)] for s in TESTED_SEEDS}
        rows += rows_for(room, tested, truths, dict(t60=t60s))
        say('room %s: %d rows' % (room, sum(1 for r in rows if r['room'] == room)))
    return rows, info


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('set', choices=('b', 'c'))
    ap.add_argument('--dry', metavar='ROOM')
    ap.add_argument('--data')
    ap.add_argument('--out')
    ap.add_argument('--simpa')
    ap.add_argument('--seeds', help='tested seeds, comma-separated (default 4101,4102,4103; the fresh draw '
                                    'after build F is 4201,4202,4203)')
    a = ap.parse_args()
    if a.seeds:
        global TESTED_SEEDS
        TESTED_SEEDS = tuple(int(s) for s in a.seeds.split(','))
    data = Path(a.data) if a.data else DATA[a.set]
    out = Path(a.out) if a.out else OUT[a.set]
    if a.dry:
        out = out / 'dry'
    out.mkdir(parents=True, exist_ok=True)
    exe = simpa_exe(a.simpa)
    log = open(out / 'run.log', 'a', encoding='utf-8')

    def say(msg):
        line = '%s %s' % (time.strftime('%H:%M:%S'), msg)
        print(line, flush=True)
        log.write(line + '\n')
        log.flush()

    ver = subprocess.run([str(exe), '--version'], capture_output=True, text=True).stdout.strip()
    say('%s set %s, data %s, simpa %s (%s; e1 exists: %s), hashes %s' % (
        'DRY' if a.dry else 'SCORED', a.set, data, exe, ver, SIMPA_E1.exists(), json.dumps(seta.hashes())))
    rows, info = (set_b if a.set == 'b' else set_c)(data, out, exe, a.dry, say)
    if a.dry:
        counts = {}
        for m in METRICS:
            mr = [r for r in rows if r['metric'] == m]
            ex = {}
            for r in mr:
                if r['excluded']:
                    ex[r['excluded']] = ex.get(r['excluded'], 0) + 1
            counts[m] = dict(rows=len(mr), excluded=ex)
        res = dict(mode='DRY', set=a.set, room=a.dry, simpa=str(exe), simpa_version=ver, counts=counts, info=info)
        (out / ('dry-%s.json' % a.dry)).write_text(json.dumps(res, indent=1, default=str), encoding='utf-8')
        say('DRY %s: %s' % (a.dry, json.dumps(counts)))
        say('solver stderr: %s' % json.dumps({room: {k: v.get('notable') for k, v in i['solver_stderr'].items()}
                                              for room, i in info.items()}))
        return
    sc = score(rows)
    res = dict(mode='SCORED', set=a.set, simpa=str(exe), simpa_version=ver, score=sc, info=info,
               pass_={m: sc[m]['pass_'] for m in SCORED})
    (out / 'summary.json').write_text(json.dumps(res, indent=1, default=str), encoding='utf-8')
    with open(out / 'rows.jsonl', 'w', encoding='utf-8') as f:
        for r in rows:
            f.write(json.dumps(r, default=str) + '\n')
    say('SCORED: %s' % json.dumps(res['pass_']))


if __name__ == '__main__':
    main()
