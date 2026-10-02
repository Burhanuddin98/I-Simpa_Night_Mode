"""T20 part 3b: round 2's SPPS rooms G1-G7 (../PREREG-3B.md, frozen; ../ADDENDUM-3B-1.md). Scores the product's `parameters.t20_s`
on round 2's 144 tested runs against the K = 4 truth runs of each room.

  python -B score3b.py --out OUT_DIR [--simpa EXE] [--solvers DIR] [--data-root DIR] [--only RUN_ID]

Imports harness2 read-only (driver.read_run for the runs, truth.split for the direct sound, rooms2 for the blocked
flags and design T60s) and ./truth20.py (round 2's checked `_mirror.line`, -5/-25 dB). Nothing is written next to
the runs: every output goes to OUT_DIR.

Rows: one per tested run x point receiver x band (P13: 144 runs x 8 x 6 = 6,912).
Truth: truth20.t20 (-5/-25 dB, 0 dB the total energy, the direct sound included) on the MEAN of the room's K = 4
truth histograms, bin by bin to the shortest; the direct sound split as round 2 split it (truth.split: the bins
overlapping [t_arr - h, t_arr + h), t_arr and h = R/c from the first truth run's report; a blocked receiver has no
direct sound and t is its first bin with energy). Truth se: statistics.stdev of the 4 single-run T20s / sqrt(4). A
row whose se / truth exceeds the rule's limit, or whose truth has no fit, leaves the denominators and is counted.
Value: `simpa results --json <run folder>` (the frozen build), spps.point_receivers[r].bands[b].parameters.t20_s.
Only an Evaluated::Value ({value, mc_sd}) is answered; any refusal, noise refusals included, is unanswered and
counted by its `why` (the bed's reading, bed/read.rs), else its `code`.
Two rules, every row scored under both (ADDENDUM-3B-1 item 1):
- gated: truth-se limit 1.0 %; covered |truth - value| <= Z*sqrt(mc_sd^2 + se^2) + 0.005*value. More than 25 % of
  rows excluded at this limit makes the verdict INCONCLUSIVE.
- prereg (PREREG-3B as written, reported beside it): limit 0.5 %; covered |truth - value| <= Z*mc_sd + 0.005*value.
Z = 2.5; no mc_sd: its term is 0, counted. Wrong-silent (both): answered, not covered, |value/truth - 1| > 5 %.
J2 per mode: coverage >= 90 % and wrong-silent <= 3 % of answered rows. J3 per mode (ADDENDUM-3B-1 item 2): no
room x step (receivers pooled) with >= 20 answered rows has wrong-silent > 10 %; the worst room x receiver x step
is reported. J4 per mode: >= 90 % of rows answered at 1 ms, design T60 <= 3.2 s,
R <= 0.5 m (every row in that filter, truth-excluded ones included: answering does not depend on the truth).
--only RUN_ID scores that one tested run against its room's truths and labels everything SMOKE (no verdict).
"""
import sys

sys.dont_write_bytecode = True

import argparse  # noqa: E402
import csv  # noqa: E402
import hashlib  # noqa: E402
import json  # noqa: E402
import math  # noqa: E402
import os  # noqa: E402
import re  # noqa: E402
import statistics  # noqa: E402
import subprocess  # noqa: E402
import time  # noqa: E402
from pathlib import Path  # noqa: E402

import numpy as np  # noqa: E402

HERE = Path(__file__).resolve().parent
INV = HERE.parent
HELD = INV.parent / '2026-09-27-edt-heldout'
sys.path.insert(0, str(HELD / 'harness2'))
sys.path.insert(0, str(HERE))
from m8b import driver, rooms2, round2, truth as T  # noqa: E402
import truth20  # noqa: E402

PREREG = INV / 'PREREG-3B.md'
DEFAULT_SIMPA = Path(r'C:\tmp\nm-target-t20p3\release\simpa.exe')     # build of 6435845
DEFAULT_SOLVERS = Path(r'C:\tmp\nm-m8a-solvers')
DATA_ROOT = round2.DATA_ROOT                                            # B:\data\m8b-edt\round2\heldout

Z = 2.5
Z_SENSITIVITY = (2.0, 3.0)
JND = 0.05
TOL = 0.005                     # 1/10 JND: the covered allowance (and PREREG-3B's truth-se limit)
# ADDENDUM-3B-1 item 1: the gated rule (truth-se limit 1.0 %, both uncertainties in the range) and PREREG-3B's
# original rule (se <= 0.5 %, the product's mc_sd alone), scored side by side on every row.
GATED = dict(name='gated', se_limit=0.010, truth_se_in_range=True)
PREREG_RULE = dict(name='prereg', se_limit=0.005, truth_se_in_range=False)
INCONCLUSIVE_EXCLUDED = 0.25    # ADDENDUM-3B-1: > 25 % of rows excluded at the 1.0 % limit -> INCONCLUSIVE
K = 4
J2_COVERAGE = 0.90
J2_WRONG_SILENT = 0.03
J3_MIN_ROWS = 20
J3_WRONG_SILENT = 0.10
J4_MIN = 0.90
J4_STEP_MS = 1.0
J4_T60_MAX_S = 3.2
J4_R_MAX_M = 0.5
CALLED_OUT = {'G6': 'corridor', 'G3': 'coupled', 'G2': 'long T60'}
N_TESTED, N_TRUTH = 144, 28

TESTED_RE = re.compile(r'^tested-(G\d)-(random|energetic)-(\d+(?:\.\d+)?)ms-(\d+)k-(\d+)$')
TRUTH_RE = re.compile(r'^truth-(G\d)-(\d+)$')


class ScoreError(Exception):
    pass


def log(msg, fh=None):
    line = '%s %s' % (time.strftime('%H:%M:%S'), msg)
    print(line, file=sys.stderr, flush=True)
    if fh is not None:
        fh.write(line + '\n')
        fh.flush()


def sha256_file(p):
    return hashlib.sha256(Path(p).read_bytes()).hexdigest()


# ---- the runs -------------------------------------------------------------------------------------------------------
def discover(data_root):
    """(tested runs, {room: [truth dirs]}), from the folder names, cross-checked against each project.simpa."""
    data_root = Path(data_root)
    tested, truths = [], {}
    for d in sorted(p for p in data_root.iterdir() if p.is_dir()):
        m = TESTED_RE.match(d.name)
        if m:
            room, mode, step, kp, seed = m.groups()
            sp = json.loads((d / 'project.simpa').read_text(encoding='utf-8'))['solvers']['spps']
            want = dict(method=mode, time_step_s=float(step) / 1e3, particles_per_source=int(kp) * 1000,
                        random_seed=int(seed))
            got = {k: sp[k] for k in want}
            if got['method'] != want['method'] or got['random_seed'] != want['random_seed'] \
                    or got['particles_per_source'] != want['particles_per_source'] \
                    or abs(got['time_step_s'] - want['time_step_s']) > 1e-12:
                raise ScoreError('%s: project.simpa says %s, the folder name %s' % (d, got, want))
            tested.append(dict(run_id=d.name, dir=d, room=room, mode=mode, step_ms=float(step),
                               particles=int(kp) * 1000, seed=int(seed)))
            continue
        m = TRUTH_RE.match(d.name)
        if m:
            truths.setdefault(m.group(1), []).append(d)
    return tested, truths


def run_folder(run_dir):
    """The `<stamp>-spps` folder `simpa run` made inside a launch folder: exactly one."""
    sub = [p for p in Path(run_dir).iterdir() if p.is_dir() and p.name.endswith('-spps')]
    if len(sub) != 1:
        raise ScoreError('%s: %d run folders, not 1' % (run_dir, len(sub)))
    return sub[0]


# ---- the truth ------------------------------------------------------------------------------------------------------
def truth_of(bins, dt, t_arr, h, blocked):
    """(T20 s, None) or (nan, reason): round 2's split, then truth20.t20 (-5/-25 dB, 0 dB with the direct sound)."""
    bins = np.asarray(bins, dtype=np.float64)
    if not np.all(np.isfinite(bins)):
        return float('nan'), 'nan_in_series'
    direct, refl, t = T.split(bins, dt, t_arr, h, blocked)
    if not math.isfinite(t) or not float(direct.sum() + refl.sum()) > 0:
        return float('nan'), 'no_energy'
    if t / float(dt) >= len(refl):
        return float('nan'), 'arrival_past_end'
    return truth20.t20(direct, refl, t, float(dt))


def truth_entry(series, dt, t_arr, h, blocked):
    """series: the K histograms of one receiver-band. Truth on their mean; se = stdev(singles) / sqrt(K)."""
    if len(series) != K:
        raise ScoreError('P16: exactly %d truth runs, not %d' % (K, len(series)))
    n = min(len(s) for s in series)
    mean = sum(np.asarray(s[:n], dtype=np.float64) for s in series) / K
    t, why = truth_of(mean, dt, t_arr, h, blocked)
    singles = [truth_of(s[:n], dt, t_arr, h, blocked)[0] for s in series]
    if math.isfinite(t) and all(math.isfinite(x) for x in singles):
        se = statistics.stdev(singles) / math.sqrt(K)
    else:
        se = float('nan')
    return dict(truth=t, truth_why=why, truth_se=se, singles=singles, last10_share=T.last10_share(mean))


def room_truths(room, truth_dirs, data_root, geom):
    """{(label, band_hz): truth_entry + blocked}, from the room's K truth runs (driver.read_run: verified build)."""
    truth_dirs = sorted(truth_dirs)
    if len(truth_dirs) != K:
        raise ScoreError('%s: %d truth runs, not %d' % (room, len(truth_dirs), K))
    reads = [driver.read_run(d, data_root=data_root) for d in truth_dirs]
    out = {}
    for s0 in reads[0]:
        key = (s0['label'], s0['band_hz'])
        matched = [next((s for s in r if (s['label'], s['band_hz']) == key), None) for r in reads]
        if any(s is None for s in matched):
            raise ScoreError('%s %s: not in every truth run' % (room, key))
        if any(abs(s['dt'] - s0['dt']) > 1e-15 for s in matched):
            raise ScoreError('%s %s: truth runs differ in dt' % (room, key))
        idx = int(s0['label'].lstrip('Rr'))
        blocked = bool(geom['receivers'][idx]['blocked'])
        e = truth_entry([s['energy_pa2'] for s in matched], s0['dt'], s0['arrival_s'], s0['half_width'], blocked)
        e['blocked'] = blocked
        out[key] = e
    return out


# ---- the product ----------------------------------------------------------------------------------------------------
def _find_why(o):
    if isinstance(o, dict):
        w = o.get('why')
        if isinstance(w, str):
            return w
        for v in o.values():
            r = _find_why(v)
            if r:
                return r
    elif isinstance(o, list):
        for v in o:
            r = _find_why(v)
            if r:
                return r
    return None


def read_param(p):
    """(value, mc_sd, source): an Evaluated::Value is ('value'); every refusal is (None, None, its why or code)."""
    if p is None:
        return None, None, 'absent'
    if 'value' in p and 'not_evaluable' not in p:
        v = p['value']
        if v is None or not math.isfinite(float(v)):
            return None, None, 'value_not_finite'
        sd = p.get('mc_sd')
        return float(v), (None if sd is None else float(sd)), 'value'
    ne = p.get('not_evaluable')
    if isinstance(ne, dict):
        return None, None, _find_why(ne.get('error')) or ne.get('code') or 'refused'
    return None, None, 'unreadable'


def product_t20(folder, simpa, solvers):
    """{'exit', 'stderr', 'solver_build', 't20': {(label, band): (value, mc_sd, source)}} from `simpa results --json`."""
    env = dict(os.environ)
    env['SIMPA_SOLVERS_DIR'] = str(solvers)
    r = subprocess.run([str(simpa), 'results', '--json', str(folder)], capture_output=True, env=env)
    out = dict(exit=r.returncode, stderr=r.stderr.decode('utf-8', 'replace')[-2000:], solver_build=None, t20={})
    if r.returncode != 0:
        return out
    rep = json.loads(r.stdout)
    out['solver_build'] = (rep.get('solver_build') or {}).get('status')
    sp = rep.get('spps') or {}
    out['receiver_radius_m'] = sp.get('receiver_radius_m')
    for pr in sp.get('point_receivers') or []:
        for b in pr['bands']:
            out['t20'][(pr['label'], b['freq_hz'])] = read_param((b.get('parameters') or {}).get('t20_s'))
    return out


# ---- the rows -------------------------------------------------------------------------------------------------------
def covered(value, mc_sd, truth, z=Z, truth_se=None):
    """|truth - value| <= z*sqrt(mc_sd^2 + truth_se^2) + 0.5 %*value; truth_se None: PREREG-3B's z*mc_sd."""
    sd = math.hypot(mc_sd or 0.0, truth_se or 0.0)
    return abs(truth - value) <= z * sd + TOL * value


def make_rows(run, prod, truths, geom):
    rows = []
    for (label, band), e in sorted(truths.items(), key=lambda kv: (int(kv[0][0].lstrip('Rr')), kv[0][1])):
        if prod['exit'] != 0:
            value, mc_sd, source = None, None, 'results_failed'
        else:
            value, mc_sd, source = prod['t20'].get((label, band), (None, None, 'absent'))
        t60 = geom['design_t60_s']
        r_m = prod.get('receiver_radius_m')
        rows.append(dict(
            run_id=run['run_id'], room=run['room'], mode=run['mode'], step_ms=run['step_ms'],
            particles=run['particles'], seed=run['seed'], receiver=label, freq_hz=band, blocked=e['blocked'],
            design_t60_s=t60.get(band, t60.get(str(band))),
            R_m=None if r_m is None else round(float(r_m), 9),
            value=value, mc_sd=mc_sd, source=source,
            truth=e['truth'], truth_se=e['truth_se'], truth_why=e['truth_why'],
            truth_singles=';'.join('%.9g' % x for x in e['singles']), truth_last10=e['last10_share']))
    if prod['exit'] == 0:
        extra = set(prod['t20']) - set(truths)
        if extra:
            raise ScoreError('%s: the product reports receiver-bands the truths lack: %s' % (run['run_id'],
                                                                                            sorted(extra)[:5]))
    return rows


def classify(row, z=Z, rule=GATED):
    """Adds `status` (excluded_truth_nan / excluded_truth_uncertain / unanswered / answered) and, for answered rows,
    err, covered, wrong_silent, wide and covered at each sensitivity Z, under `rule` (GATED or PREREG_RULE)."""
    t, se = row['truth'], row['truth_se']
    rse = se if rule['truth_se_in_range'] else None
    if t is None or not math.isfinite(t) or not t > 0:
        row['status'] = 'excluded_truth_nan'
    elif se is None or not math.isfinite(se) or se / t > rule['se_limit']:
        row['status'] = 'excluded_truth_uncertain'
    elif row['value'] is None:
        row['status'] = 'unanswered'
    else:
        row['status'] = 'answered'
        row['err'] = row['value'] / t - 1.0
        row['covered'] = covered(row['value'], row['mc_sd'], t, z, rse)
        row['wrong_silent'] = (not row['covered']) and abs(row['err']) > JND
        row['wide'] = row['covered'] and abs(row['err']) > JND
        for zz in Z_SENSITIVITY:
            row['covered_z%g' % zz] = covered(row['value'], row['mc_sd'], t, zz, rse)
    return row


def j4_in(row):
    return (abs(row['step_ms'] - J4_STEP_MS) < 1e-6 and row['design_t60_s'] is not None
            and row['design_t60_s'] <= J4_T60_MAX_S and row['R_m'] is not None and row['R_m'] <= J4_R_MAX_M + 1e-9)


# ---- the score ------------------------------------------------------------------------------------------------------
def _codes(rows):
    out = {}
    for r in rows:
        if r['value'] is None:
            out[r['source']] = out.get(r['source'], 0) + 1
    return dict(sorted(out.items()))


def tally(rows):
    a = [r for r in rows if r['status'] == 'answered']
    n = len(a)
    out = dict(rows=len(rows), answered=n,
               unanswered=sum(r['status'] == 'unanswered' for r in rows),
               excluded=sum(r['status'].startswith('excluded') for r in rows),
               unanswered_by_code=_codes([r for r in rows if r['status'] == 'unanswered']))
    if n:
        out['covered'] = sum(r['covered'] for r in a)
        out['wrong_silent'] = sum(r['wrong_silent'] for r in a)
        out['coverage'] = out['covered'] / n
        out['wrong_silent_rate'] = out['wrong_silent'] / n
        out['wide_covered_over_jnd'] = sum(r['wide'] for r in a)
        for zz in Z_SENSITIVITY:
            out['coverage_z%g' % zz] = sum(r['covered_z%g' % zz] for r in a) / n
        errs = [r['err'] for r in a]
        out['mean_err'] = statistics.fmean(errs)
        out['median_err'] = statistics.median(errs)
        out['min_err'] = min(errs)
        out['max_err'] = max(errs)
        out['no_mc_sd'] = sum(r['mc_sd'] is None for r in a)
    return out


def score_rule(rows, z=Z, smoke=False, rule=GATED):
    """One rule's score of every row (copies; the caller's rows are not changed)."""
    rows = [classify(dict(r), z, rule) for r in rows]
    status_counts = {}
    for r in rows:
        status_counts[r['status']] = status_counts.get(r['status'], 0) + 1
    modes = sorted({r['mode'] for r in rows})

    j2 = {}
    for mode in modes:
        t = tally([r for r in rows if r['mode'] == mode])
        t['coverage_ok'] = t['answered'] > 0 and t['coverage'] >= J2_COVERAGE
        t['wrong_silent_ok'] = t['answered'] > 0 and t['wrong_silent_rate'] <= J2_WRONG_SILENT
        t['holds'] = t['coverage_ok'] and t['wrong_silent_ok']
        j2[mode] = t

    # J3 (ADDENDUM-3B-1 item 2): room x step per mode, receivers pooled; room x receiver x step reported only.
    subgroups, groups, fine_groups = [], {}, {}
    for r in rows:
        groups.setdefault((r['mode'], r['room'], r['step_ms']), []).append(r)
        fine_groups.setdefault((r['mode'], r['room'], r['receiver'], r['step_ms']), []).append(r)
    for (mode, room, step), sub in sorted(groups.items()):
        t = tally(sub)
        t.update(mode=mode, room=room, step_ms=step)
        t['j3_applies'] = t['answered'] >= J3_MIN_ROWS
        t['j3_breach'] = t['j3_applies'] and t['wrong_silent_rate'] > J3_WRONG_SILENT
        subgroups.append(t)
    fine = []
    for (mode, room, rec, step), sub in sorted(fine_groups.items()):
        t = tally(sub)
        t.update(mode=mode, room=room, receiver=rec, step_ms=step)
        fine.append(t)
    j3 = {}
    for mode in modes:
        ss = [s for s in subgroups if s['mode'] == mode]
        br = ['%s %gms' % (s['room'], s['step_ms']) for s in ss if s['j3_breach']]
        ff = [f for f in fine if f['mode'] == mode and f['answered']]
        worst = max(ff, key=lambda f: (f['wrong_silent_rate'], f['wrong_silent']), default=None)
        j3[mode] = dict(holds=not br, breaches=br, subgroups=len(ss), subgroups_applying=sum(s['j3_applies'] for s in ss),
                        worst_room_step=max((s.get('wrong_silent_rate', 0.0) for s in ss), default=None),
                        worst_room_receiver_step=None if worst is None else {
                            k: worst[k] for k in ('room', 'receiver', 'step_ms', 'answered', 'wrong_silent',
                                                  'wrong_silent_rate')})

    j4 = {}
    for mode in modes:
        f = [r for r in rows if r['mode'] == mode and j4_in(r)]
        n = len(f)
        ans = sum(r['value'] is not None for r in f)
        f_ok = [r for r in f if not r['status'].startswith('excluded')]
        j4[mode] = dict(rows=n, answered=ans, share=(ans / n if n else None),
                        holds=bool(n) and ans / n >= J4_MIN,
                        refusals_by_code=_codes(f),
                        refusals_by_room={room: _codes([r for r in f if r['room'] == room])
                                          for room in sorted({r['room'] for r in f})},
                        share_truth_included=(sum(r['value'] is not None for r in f_ok) / len(f_ok) if f_ok else None),
                        filter_left_out=sum(1 for r in rows if r['mode'] == mode and abs(r['step_ms'] - 1) < 1e-6
                                            and not j4_in(r)))

    per_room = {}
    for mode in modes:
        for room in sorted({r['room'] for r in rows}):
            sub = [r for r in rows if r['mode'] == mode and r['room'] == room]
            if not sub:
                continue
            t = tally(sub)
            t['called_out'] = CALLED_OUT.get(room)
            t['by_particles'] = {str(p): tally([r for r in sub if r['particles'] == p])
                                 for p in sorted({r['particles'] for r in sub})}
            t['by_receiver'] = {rec: tally([r for r in sub if r['receiver'] == rec])
                                for rec in sorted({r['receiver'] for r in sub})}
            per_room['%s %s' % (mode, room)] = t
    per_step = {'%s %gms' % (mode, s): tally([r for r in rows if r['mode'] == mode and r['step_ms'] == s])
                for mode in modes for s in sorted({r['step_ms'] for r in rows})}

    wide = [{k: r[k] for k in ('run_id', 'receiver', 'freq_hz', 'value', 'mc_sd', 'truth', 'err')}
            for r in rows if r['status'] == 'answered' and r['wide']]
    wrong = [{k: r[k] for k in ('run_id', 'receiver', 'freq_hz', 'value', 'mc_sd', 'truth', 'err')}
             for r in rows if r['status'] == 'answered' and r['wrong_silent']]
    excl = [r for r in rows if r['status'].startswith('excluded')]
    truth_se_rel = [r['truth_se'] / r['truth'] for r in rows
                    if r['truth'] and math.isfinite(r['truth']) and r['truth_se'] is not None and math.isfinite(r['truth_se'])]
    holds = bool(modes) and all(j2[m]['holds'] and j3[m]['holds'] and j4[m]['holds'] for m in modes)
    excluded_share = len(excl) / len(rows) if rows else None
    inconclusive = bool(rows) and excluded_share > INCONCLUSIVE_EXCLUDED
    verdict = 'SMOKE' if smoke else 'INCONCLUSIVE' if inconclusive else 'PASS' if holds else 'FAIL'
    return dict(
        prereg='docs/investigations/2026-10-02-t20/PREREG-3B.md',
        addendum='docs/investigations/2026-10-02-t20/ADDENDUM-3B-1.md', rule=rule, smoke=smoke, z=z,
        verdict=verdict, criteria_hold=holds, excluded_share=excluded_share, inconclusive=inconclusive,
        rows=len(rows), status_counts=status_counts,
        unanswered_by_code=_codes([r for r in rows if r['status'] == 'unanswered']),
        excluded=dict(count=len(excl), answered_among_them=sum(r['value'] is not None for r in excl),
                      by_room={room: sum(r['room'] == room for r in excl) for room in sorted({r['room'] for r in excl})}),
        truth_se_rel=dict(max=max(truth_se_rel, default=None),
                          median=statistics.median(truth_se_rel) if truth_se_rel else None),
        overall=tally(rows), j2=j2, j3=j3, j4=j4,
        per_room=per_room, per_step=per_step,
        wide_rows=wide, wrong_silent_rows=wrong, subgroups=subgroups, subgroups_room_receiver_step=fine,
    ), rows


ORIG_FIELDS = ('status', 'err', 'covered', 'wrong_silent', 'wide', 'covered_z2', 'covered_z3')


def score(rows, z=Z, smoke=False):
    """The gated score (ADDENDUM-3B-1) with PREREG-3B's original rule beside it, under 'prereg_rule'. Rows carry
    the gated fields plus the original rule's as <field>_prereg."""
    gated, g_rows = score_rule(rows, z, smoke, GATED)
    orig, o_rows = score_rule(rows, z, smoke, PREREG_RULE)
    for g, o in zip(g_rows, o_rows):
        for k in ORIG_FIELDS:
            g[k + '_prereg'] = o.get(k)
    gated['prereg_rule'] = orig
    return gated, g_rows


CSV_FIELDS = ['run_id', 'room', 'mode', 'step_ms', 'particles', 'seed', 'receiver', 'freq_hz', 'blocked',
              'design_t60_s', 'R_m', 'source', 'value', 'mc_sd', 'truth', 'truth_se', 'truth_why', 'truth_singles',
              'truth_last10', 'status', 'err', 'covered', 'wrong_silent', 'wide', 'covered_z2', 'covered_z3'] + \
             [k + '_prereg' for k in ORIG_FIELDS]


def _brief(s):
    b = {k: s[k] for k in ('verdict', 'excluded_share', 'rows', 'status_counts', 'unanswered_by_code', 'excluded',
                           'truth_se_rel')}
    b['j2'] = {m: {k: v for k, v in t.items() if k != 'unanswered_by_code'} for m, t in s['j2'].items()}
    b['j3'] = s['j3']
    b['j4'] = {m: {k: t[k] for k in ('rows', 'answered', 'share', 'holds', 'refusals_by_code')}
               for m, t in s['j4'].items()}
    return b


def main(argv=None):
    ap = argparse.ArgumentParser()
    ap.add_argument('--out', required=True)
    ap.add_argument('--simpa', default=str(DEFAULT_SIMPA))
    ap.add_argument('--solvers', default=str(DEFAULT_SOLVERS))
    ap.add_argument('--data-root', default=str(DATA_ROOT))
    ap.add_argument('--only', help='one tested run id: a SMOKE, no verdict')
    a = ap.parse_args(argv)
    out, data_root = Path(a.out), Path(a.data_root)
    if driver.inside(out, data_root.parent) or driver._norm(out) == driver._norm(data_root.parent):
        print('refused: --out lies inside the round-2 data, which is read-only here', file=sys.stderr)
        return 2
    if (out / 'summary.json').exists():
        print('refused: %s already holds a summary.json' % out, file=sys.stderr)
        return 2
    out.mkdir(parents=True, exist_ok=True)
    fh = open(out / 'run.log', 'a', encoding='utf-8')
    t0 = time.time()
    try:
        tested, truth_dirs = discover(data_root)
        smoke = a.only is not None
        if smoke:
            tested = [r for r in tested if r['run_id'] == a.only]
            if len(tested) != 1:
                raise ScoreError('--only %s: not a tested run under %s' % (a.only, data_root))
        elif len(tested) != N_TESTED or sum(len(v) for v in truth_dirs.values()) != N_TRUTH:
            raise ScoreError('run_folders_incomplete: %d tested (want %d), %d truth (want %d)' % (
                len(tested), N_TESTED, sum(len(v) for v in truth_dirs.values()), N_TRUTH))
        geoms = rooms2.rooms()
        meta = dict(simpa=str(Path(a.simpa).resolve()), simpa_sha256=sha256_file(a.simpa), solvers=a.solvers,
                    data_root=str(data_root), prereg_sha256=sha256_file(PREREG),
                    score3b_sha256=sha256_file(__file__), truth20_sha256=sha256_file(HERE / 'truth20.py'),
                    started=time.strftime('%Y-%m-%dT%H:%M:%S'))
        log('score3b %s: %d tested runs, simpa %s' % ('SMOKE' if smoke else 'FULL', len(tested), meta['simpa_sha256'][:12]), fh)
        rows, truths_out = [], []
        prod_fh = open(out / 'products.jsonl', 'w', encoding='utf-8')
        for room in sorted({r['room'] for r in tested}):
            ts = time.time()
            tr = room_truths(room, truth_dirs.get(room, []), data_root, geoms[room])
            for (label, band), e in sorted(tr.items()):
                truths_out.append(dict(room=room, receiver=label, freq_hz=band, **{k: e[k] for k in (
                    'truth', 'truth_se', 'truth_why', 'last10_share', 'blocked')},
                    singles=';'.join('%.9g' % x for x in e['singles'])))
            log('%s truths: %d receiver-bands in %.1f s' % (room, len(tr), time.time() - ts), fh)
            for run in [r for r in tested if r['room'] == room]:
                ts = time.time()
                folder = run_folder(run['dir'])
                prod = product_t20(folder, a.simpa, a.solvers)
                prod_fh.write(json.dumps(dict(run_id=run['run_id'], folder=str(folder), exit=prod['exit'],
                                              solver_build=prod['solver_build'], stderr=prod['stderr'],
                                              t20=[[k[0], k[1], *v] for k, v in sorted(prod['t20'].items())])) + '\n')
                prod_fh.flush()
                new = make_rows(run, prod, tr, geoms[room])
                rows += new
                log('%s: exit %d, %d rows, %d answered, %.1f s' % (run['run_id'], prod['exit'], len(new),
                                                                   sum(r['value'] is not None for r in new),
                                                                   time.time() - ts), fh)
        prod_fh.close()
        summary, rows = score(rows, smoke=smoke)
    except ScoreError as e:
        log('refused: %s' % e, fh)
        return 2
    summary['meta'] = meta
    summary['meta']['wall_s'] = time.time() - t0
    (out / 'summary.json').write_text(json.dumps(summary, indent=2), encoding='utf-8')
    with open(out / 'rows.csv', 'w', newline='', encoding='utf-8') as f:
        w = csv.DictWriter(f, fieldnames=CSV_FIELDS, extrasaction='ignore')
        w.writeheader()
        w.writerows(rows)
    with open(out / 'truths.csv', 'w', newline='', encoding='utf-8') as f:
        w = csv.DictWriter(f, fieldnames=list(truths_out[0]) if truths_out else ['room'])
        w.writeheader()
        w.writerows(truths_out)
    print(json.dumps({'gated': _brief(summary), 'prereg_rule': _brief(summary['prereg_rule'])}, indent=2))
    log('done in %.1f s' % (time.time() - t0), fh)
    fh.close()
    return 0


if __name__ == '__main__':
    sys.exit(main())
