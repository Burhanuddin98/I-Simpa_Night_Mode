"""The M8b EDT held-out scoring run, round 2: H1-H5 on the three fresh sets (PREREG-2.md), not H6.

Assembles the inputs of SPPS-fresh (Random and Energetic, P11), ISM-fresh and Synth-fresh from the
harness's own functions only, hands them to score.evaluate (round 2's method, Z = 2.5, and upstream's
port, both hash-checked there), and writes every row and every count. No physics, no threshold and no
seed is defined here: each one is the harness's, which takes it from ADDENDUM-B1.md.

No-swap rule (HARNESS-PLAN-2.md section 9 M2, written before any round-2 run): every G room is scored whatever its truth
runs show. A designed feature that misses on the truth runs (G2 truth T30 under 2.5 s at 1 kHz, G7 truth T30 over 0.25 s, G3
not double-sloped) is reported as missing in RESULTS.md and VERDICT-2.md. No room is swapped, redesigned or dropped after any
truth run exists, and nothing in this scorer can do so: it scores the rows of the 172 planned runs and no others.

The run order is enforced before anything is read: preflight() refuses (driver.Refused) without B1 committed, on
round 1's folders or results, with any of the 172 run folders missing or partial, and when a fresh draw of ISM-fresh-2,
Synth-fresh-2 or the G rooms differs from preview_pin.json (section 9 M3). --dry skips it: planted and dev inputs only.

Where each set's rows come from:
- SPPS-fresh: driver.plan()'s 144 tested runs (72 Random, 72 Energetic) and its 28 truth runs, folders
  <data_root>/<run_id>, read through spps_rows.rows_from_runs (which reads them with driver.read_run).
  Each tested run's references are its room's four truth runs, seeds driver.TRUTH_SEEDS (9101-9104). A run folder
  named *.partial-* is never looked at: the plan's exact run_id folder is. An 'id' gets '|<mode>' added,
  because score.evaluate refuses one id twice in a set and the two modes' runs share rows_from_runs's id.
- ISM-fresh-2: ism_fresh.draw(driver.ISM_SEED) (P21, P22), rows by ism_fresh.rows_for_scoring, which
  builds every row with retry_truncated=True (8.2's fourth call). It is called once per room, in worker
  processes, and each worker memoises the generator's echogram (a pure function of its arguments, and
  make_row only reads the arrays it gets) so the 27 band-and-step rows of one receiver share one image
  sum instead of repeating it. The workers are few because one large echogram took 7.6 GB (8.2). P34: each
  worker logs 'ism room <id> start: <images> images, pid <pid>' before any work, and 'ism room <id> receiver
  k/n (<s> s)' after each receiver, so a room that takes an hour is visible from its first minute.
- Synth-fresh: synth_fresh.draw(driver.SYNTH_SEED) (P24, P33), histogram(spec) and truth(spec) per
  spec, 'family' the rate ratio (P23).

Outputs under --out (never overwritten: a non-empty folder is refused):
  inputs_<set>.pkl.gz   the inputs as score.evaluate got them (bins included)
  score/                score.evaluate's REPORT.md, summary.json and rows.jsonl
  rows.csv.gz           one line per row: the row's fields and each instance's result
  counts.json           every count: tables, criteria H1-H5 per instance and mode, exclusions, refusals,
                        the draws' rejections
  RESULTS.md            the tables

--dry runs the same code path on planted and dev inputs only: dry/build_d2.py's mock room for SPPS, a
non-held-out seed and a few rows for ISM and Synth. It never reads B:\\data\\m8b-edt\\heldout and
never draws a held-out seed.

Usage, from harness2/ (venv as SETUP.md says):
    C:\\tmp\\m8b-edt\\venv\\Scripts\\python.exe -m m8b.score_heldout --dry --out C:\\tmp\\m8b-edt\\dry\\score_heldout2
    C:\\tmp\\m8b-edt\\venv\\Scripts\\python.exe -m m8b.score_heldout --out B:\\data\\m8b-edt\\round2\\results\\run2
"""
import argparse
import csv
import datetime
import gzip
import json
import multiprocessing
import os
import pickle
import subprocess
import sys
import threading
import time
from pathlib import Path

import numpy as np

from . import corpus, corpus2, driver, ism_fresh, rooms as rooms_mod, rooms2, round2, run_heldout, score, spps_rows, synth_fresh

DATA_ROOT = round2.DATA_ROOT
RESULTS_ROOT = round2.RESULTS_ROOT
PROGRESS_LOG = round2.PROGRESS_LOG
DRY_SEED = 20261001                     # not in driver.HELDOUT_SEEDS
DRY_ISM_RECEIVERS = 3
DRY_SYNTH_PER_CELL = 6
ISM_WORKERS = round2.ISM_WORKERS        # 7.6 GB for one large echogram (8.2); Grace has 32 GB


# One lock for every writer of the log. On Windows an append from two handles is not atomic (the CRT seeks to the end,
# then writes), so P34's lines from two worker processes, and the parent's, can overwrite one another: T41 lost a line
# to it with threads. In the real run the lock is a multiprocessing lock made by _make_pool and handed to each worker.
_LOCK = threading.Lock()


def _init_worker(lock):
    global _LOCK
    _LOCK = lock


class Log:
    def __init__(self, path):
        self.path = Path(path)
        self.path.parent.mkdir(parents=True, exist_ok=True)

    def __call__(self, msg):
        line = '%s score: %s\n' % (datetime.datetime.now().strftime('%H:%M'), msg)
        with _LOCK:
            with open(self.path, 'a', encoding='utf-8') as f:
                f.write(line)
        print(line, end='', flush=True)


# ---- the run order (HARNESS-PLAN-2.md section 7) -----------------------------------------------------------------
def preflight(*, data_root, out, pin=True):
    """Everything that must hold before a row is read: B1 committed, no round-1 folder as the data root or the output,
    all 172 planned run folders complete (a missing or partial one is named), and, when pin is True, a fresh draw of
    ISM-fresh-2, Synth-fresh-2 and the G rooms equal to preview_pin.json. Raises driver.Refused; returns None."""
    if not driver.b1_committed():
        raise driver.Refused('heldout_before_b1', 'ADDENDUM-B1.md is not committed: nothing of round 2 is scored before the freeze')
    for what, path in (('data root', data_root), ('output folder', out)):
        if driver.overlaps_round1(path):
            raise driver.Refused('round1_root', 'the %s %s is, holds or lies inside one of round 1\'s folders: round 1 is never '
                                                're-scored' % (what, path))
    bad = [r['run_id'] for r in driver.plan() if run_heldout.run_status(Path(data_root) / r['run_id']) != 'complete']
    if bad:
        raise driver.Refused('run_folders_incomplete', '%d of the 172 planned run folders under %s are missing or partial, the first '
                                                       '%s' % (len(bad), data_root, bad[0]))
    if pin:
        differs = corpus2.check_pin()
        if differs:
            raise driver.Refused('pin_mismatch', 'a fresh draw differs from preview_pin.json: %s' % differs)


# ---- SPPS-fresh --------------------------------------------------------------------------------------
def build_spps(data_root, log):
    plan = driver.plan()
    tested = [r for r in plan if r['kind'] == 'tested']
    truth_runs = {}
    for r in plan:
        if r['kind'] == 'truth':
            truth_runs.setdefault(r['room'], {})[r['random_seed']] = Path(data_root) / r['run_id']
    cache = {}
    real_read = driver.read_run

    def cached_read(run_dir, **kw):                    # each reference is read once, not once per tested run
        if Path(run_dir).name.startswith('truth-'):
            k = str(run_dir)
            if k not in cache:
                cache[k] = real_read(run_dir, **kw)
            return cache[k]
        return real_read(run_dir, **kw)

    driver.read_run = cached_read
    out = {'random': [], 'energetic': []}
    try:
        for i, run in enumerate(tested, 1):
            tdir = Path(data_root) / run['run_id']
            refs = [truth_runs[run['room']][s] for s in driver.TRUTH_SEEDS]
            for p in [tdir] + refs:
                if not p.is_dir():
                    raise FileNotFoundError('%s: a planned run folder is missing' % p)
            rows = spps_rows.rows_from_runs(run['room'], tdir, refs, mode=run['mode'],
                                            particles=run['particles_per_source'], seed=run['random_seed'],
                                            data_root=data_root)
            for r in rows:
                r['id'] = '%s|%s' % (r['id'], run['mode'])
            out[run['mode']].extend(rows)
            if i % 24 == 0:
                log('spps %d of %d tested runs read' % (i, len(tested)))
    finally:
        driver.read_run = real_read
    return out['random'] + out['energetic']


def build_spps_dry(log):
    from dry import build_d2                                   # planted fixture, harness2/dry/
    manifest = build_d2.build()
    rows = []
    for dirs in manifest['tested'].values():
        for tdir in dirs:
            seed = int(Path(tdir).name.rsplit('seed', 1)[1])
            rows.extend(spps_rows.rows_from_runs('mock_d2', tdir, manifest['refs'], mode='random', particles=0,
                                                 seed=seed, data_root=build_d2.OUT, geometry=manifest['room']['mock_d2']))
    for r in rows:
        r['id'] += '|random'
    # the same rows as an Energetic set, so the two-mode path is exercised
    rows += [dict(r, id=r['id'][:-len('random')] + 'energetic', mode='energetic') for r in rows]
    return rows


# ---- ISM-fresh ---------------------------------------------------------------------------------------
def _memoise_echogram():
    ism = ism_fresh.generator()
    if getattr(ism, '_m8b_memo', False):
        return
    real, memo = ism.echogram, {}

    def echogram(*args, **kw):
        k = repr((args, sorted(kw.items())))
        if k not in memo:
            if len(memo) >= 2:                                 # the receiver's echogram and its 1.5x retry
                memo.pop(next(iter(memo)))
            memo[k] = real(*args, **kw)
        return memo[k]
    ism.echogram = echogram
    ism._m8b_memo = True


def _make_pool(workers):
    global _LOCK
    ctx = multiprocessing.get_context('spawn')
    _LOCK = ctx.Lock()                                  # the parent's lines share it with the workers' (see Log)
    return ctx.Pool(workers, initializer=_init_worker, initargs=(_LOCK,))


def _ism_room(arg):
    """One room in a worker. P34: a line before any work (id, image count, pid), a line after each receiver, so a room
    that takes an hour is visible from its first minute (round 1 logged only when a room finished)."""
    D_room, log_path = arg
    room = D_room['rooms'][0]
    log = Log(log_path)
    images = room.get('images', ism_fresh.n_images(room['dims_m'], 343.2 * room['image_time_s']))
    log('ism room %s start: %d images, pid %d' % (room['id'], images, os.getpid()))
    _memoise_echogram()
    t0 = time.monotonic()
    rows = ism_fresh.rows_for_scoring(D_room, on_receiver=lambda k, n, secs: log(
        'ism room %s receiver %d/%d (%.0f s)' % (room['id'], k, n, secs)))
    return room['id'], rows, time.monotonic() - t0


def build_ism(D, log, workers):
    tasks = [(dict(D, rooms=[room]), log.path) for room in D['rooms']]
    # largest image sets first, so the heavy rooms never wait behind the light ones
    tasks.sort(key=lambda t: -ism_fresh.n_images(t[0]['rooms'][0]['dims_m'], 343.2 * t[0]['rooms'][0]['image_time_s']))
    by_room = {}
    if workers <= 1:
        results = map(_ism_room, tasks)
        pool = None
    else:
        pool = _make_pool(workers)
        results = pool.imap_unordered(_ism_room, tasks)
    try:
        for rid, rows, secs in results:
            by_room[rid] = rows
            log('ism room %s done: %d rows in %.0f s (%d of %d rooms)' % (rid, len(rows), secs, len(by_room),
                                                                          len(tasks)))
    finally:
        if pool is not None:
            pool.close()
            pool.join()
    return [r for room in D['rooms'] for r in by_room[room['id']]]


# ---- Synth-fresh -------------------------------------------------------------------------------------
def build_synth(specs):
    rows = []
    for s in specs:
        rows.append(dict(set='synth', id=s['id'], bins=synth_fresh.histogram(s), dt=s['dt'],
                         t_arrival=s['t_arrival'], meta=dict(half_width=s['half_width']),
                         truth=synth_fresh.truth(s), truth_status='ok', family=s['ratio'],
                         step_ms=s['step_ms'], R_m=s['R_m']))
    return rows


# ---- outputs -----------------------------------------------------------------------------------------
def _dump(path, obj):
    with gzip.open(path, 'wb', compresslevel=3) as f:
        pickle.dump(obj, f, protocol=4)


CSV_FIELDS = ('set', 'id', 'mode', 'room', 'd_m', 'step_ms', 'band_hz', 'particles', 'seed', 'family',
              'design_t60_s', 'R_m', 'truth', 'truth_status', 'excluded_as')
RES_FIELDS = ('status', 'edt', 'edt_lo', 'edt_hi', 'reason', 'err', 'ok', 'usable', 'has_truth', 'wrong_silent',
              'covered')


def write_rows_csv(score_dir, path):
    with gzip.open(path, 'wt', encoding='utf-8', newline='') as f:
        w = csv.writer(f)
        w.writerow(list(CSV_FIELDS) + ['%s_%s' % (i, k) for i in score.INSTANCES for k in RES_FIELDS])
        for line in (Path(score_dir) / 'rows.jsonl').read_text(encoding='utf-8').splitlines():
            r = json.loads(line)
            w.writerow([r.get(k) for k in CSV_FIELDS] + [r[i].get(k) for i in score.INSTANCES for k in RES_FIELDS])


def counts_of(summary, extra):
    """Every count the summary holds, regrouped: tables and exclusions per label and instance, the
    criteria per mode. H6 is left out: the attacker round is a separate step."""
    crit = {}
    for inst, per_mode in summary['criteria'].items():
        crit[inst] = {}
        for mode, c in per_mode.items():
            crit[inst][mode] = {h: c[h] for h in ('H1', 'H2', 'H3', 'H4', 'H5')}
    return dict(inputs=summary['inputs'], tables={k: v for k, v in summary['tables'].items() if k != 'attack'},
                criteria=crit, method=summary['method'], upstream=summary['upstream'],
                generated=summary['generated'], **extra)


def _p(a, b, d=1):
    return 'n/a' if not b else '%.*f %%' % (d, 100.0 * a / b)


def _pf(ok):
    return 'pass' if ok else 'FAIL'


def _h4_line(p):
    return ('H4 **%s**: n = %d rows (%d in the filter, %d refused as receiver_too_large = %s, which leave the denominator); '
            'usable %d of %d = %s (target >= 90 %%); ok share %s (reported, not gated); other refusals %s.' % (
                _pf(p['pass']), p['n'], p['n_before'], p['n_receiver_too_large'], _p(p['n_receiver_too_large'], p['n_before']),
                p['n_usable'], p['n'], _p(p['n_usable'], p['n']), _p(p['n_ok'], p['n']), p['refused'] or 'none'))


def results_md(summary, extra, run_name, script_commit, dry):
    c = summary['criteria']
    T = summary['tables']
    Z = summary['method']['Z']
    L = ['# M8b EDT held-out test, round 2: RESULTS (H1-H5)%s' % (' [DRY RUN: planted and dev inputs, not the held-out data]' if dry else ''),
         '',
         'Run `%s`, scored by `harness2/m8b/score_heldout.py` at commit `%s`. Method: frozen2/method.py (v2.1) sha256 `%s`, Z = %g, '
         'checked before anything ran. H6 (the attacker round) is not part of this run. No secondary Z is reported (PREREG-2.md). '
         'Wrong-silent is |edt/truth - 1| > 5%% on an ok row with a truth, at every receiver radius; refusals are never wrong '
         '(PREREG.md:62).' % (run_name, script_commit, summary['method']['sha256'], Z),
         '',
         '**Features that miss.** Every G room is scored whatever its truth shows (PREREG-2 / HARNESS-PLAN-2.md section 9 M2). A '
         'designed feature that a truth run does not confirm (G2 truth T30 under 2.5 s at 1 kHz, G7 truth T30 over 0.25 s, G3 not '
         'double-sloped) is reported here and in VERDICT-2.md as missing; no room is swapped, redesigned or dropped. The truth '
         'features checked: %s.' % (extra.get('features', 'not checked in this run')), '']
    for mode in ('random', 'energetic'):
        L += ['## SPPS-fresh-2, %s (frozen, Z = %g)' % (mode.capitalize(), Z), '']
        h2, h3, h4, h5 = (c['frozen'][mode][h] for h in ('H2', 'H3', 'H4', 'H5'))
        L.append('- H2 **%s**: coverage %s of %s usable rows with a truth = %s (target >= 90 %%); wrong-silent %s of %s ok '
                 'rows with a truth = %s (target <= 3 %%).' % (
                     _pf(h2['pass']), h2['n_covered'], h2['n_usable_truth'], _p(h2['n_covered'], h2['n_usable_truth']),
                     h2['n_wrong_silent'], h2['n_ok_truth'], _p(h2['n_wrong_silent'], h2['n_ok_truth'], 2)))
        sp = {k: g for k, g in h3['subgroups'].items() if k.startswith('spps|')}
        sp_fail = {k: g for k, g in sp.items() if not g['pass']}
        L.append('- H3 **%s** (all sets in this criterion: %d failing subgroups of %d judged): SPPS-fresh-2 subgroups: %d, '
                 'judged %d, failing %d.' % (_pf(h3['pass']), h3['n_failing'], h3['n_judged'], len(sp),
                                            sum(g['judged'] for g in sp.values()), len(sp_fail)))
        for k, g in sorted(sp_fail.items()):
            L.append('  - `%s`: %d of %d ok rows wrong-silent (%s)' % (k, g['n_wrong_silent'], g['n_ok_truth'],
                                                                     _p(g['n_wrong_silent'], g['n_ok_truth'])))
        L.append('- ' + _h4_line(h4['per_set']['spps']))
        p5 = h5['per_set']['spps']
        L.append('- H5 **%s**: wrong-silent, ours %d against upstream %d, on %d rows with a truth.' % (
            _pf(p5['pass']), p5['n_wrong_silent'], p5['n_wrong_silent_upstream'], p5['n_eligible']))
        L.append('')
    h1, h3r, h4r, h5r = (c['frozen']['random'][h] for h in ('H1', 'H3', 'H4', 'H5'))
    L += ['## ISM-fresh-2 (frozen, Z = %g)' % Z, '']
    p = h1['per_set']['ism']
    L.append('- H1 **%s**: wrong-silent %d of %d ok rows with a truth = %s (target <= 0.5 %%), every receiver radius.' % (
        _pf(p['pass']), p['n_wrong_silent'], p['n_ok_truth'], _p(p['n_wrong_silent'], p['n_ok_truth'], 2)))
    sub = {k: g for k, g in h3r['subgroups'].items() if k.startswith('ism|')}
    bad = {k: g for k, g in sub.items() if not g['pass']}
    L.append('- H3 **%s**: ISM-fresh-2 subgroups %d, judged %d, failing %d.' % (
        _pf(not bad), len(sub), sum(g['judged'] for g in sub.values()), len(bad)))
    for k, g in sorted(bad.items()):
        L.append('  - `%s`: %d of %d ok rows wrong-silent (%s)' % (k, g['n_wrong_silent'], g['n_ok_truth'],
                                                                 _p(g['n_wrong_silent'], g['n_ok_truth'])))
    L.append('- ' + _h4_line(h4r['per_set']['ism']))
    p = h5r['per_set']['ism']
    L.append('- H5 **%s**: wrong-silent, ours %d against upstream %d, on %d rows with a truth.' % (
        _pf(p['pass']), p['n_wrong_silent'], p['n_wrong_silent_upstream'], p['n_eligible']))
    L += ['', '## Synth-fresh-2 (frozen, Z = %g)' % Z, '']
    p = h1['per_set']['synth']
    L.append('- H1 **%s**: wrong-silent %d of %d ok rows with a truth = %s (target <= 0.5 %%), every receiver radius.' % (
        _pf(p['pass']), p['n_wrong_silent'], p['n_ok_truth'], _p(p['n_wrong_silent'], p['n_ok_truth'], 2)))
    sub = {k: g for k, g in h3r['subgroups'].items() if k.startswith('synth|')}
    bad = {k: g for k, g in sub.items() if not g['pass']}
    L.append('- H3 **%s**: Synth-fresh-2 subgroups (family x step) %d, judged %d, failing %d.' % (
        _pf(not bad), len(sub), sum(g['judged'] for g in sub.values()), len(bad)))
    for k, g in sorted(bad.items()):
        L.append('  - `%s`: %d of %d ok rows wrong-silent (%s)' % (k, g['n_wrong_silent'], g['n_ok_truth'],
                                                                 _p(g['n_wrong_silent'], g['n_ok_truth'])))
    p = h5r['per_set']['synth']
    L.append('- H5 **%s**: wrong-silent, ours %d against upstream %d, on %d rows with a truth.' % (
        _pf(p['pass']), p['n_wrong_silent'], p['n_wrong_silent_upstream'], p['n_eligible']))
    L += ['', '## H4 n, by set (the rows left in the denominator; the scorer prints it for every set and mode)', '',
          '| Mode | Set | n before | receiver_too_large | n | usable | ok |', '|---|---|---|---|---|---|---|']
    for mode in ('random', 'energetic'):
        for st, q in c['frozen'][mode]['H4']['per_set'].items():
            L.append('| %s | %s | %d | %d (%s) | %d | %s | %s |' % (mode, score.NAMES[st], q['n_before'], q['n_receiver_too_large'],
                                                                   _p(q['n_receiver_too_large'], q['n_before']), q['n'],
                                                                   _p(q['n_usable'], q['n']), _p(q['n_ok'], q['n'])))
    L += ['', '## Rows, usable and ok shares, exclusions and refusals, by set and instance', '',
          'Each cell: frozen (Z = %g) / upstream.' % Z, '',
          '| Set | rows | usable | ok | wrong-silent of ok with truth | truth outside range | benefit / regress vs upstream (frozen) |',
          '|---|---|---|---|---|---|---|']
    for lab in ('spps-random', 'spps-energetic', 'ism', 'synth'):
        t = T[lab]
        f = [t[i] for i in score.INSTANCES]
        L.append('| %s | %d | %s | %s | %s | %s | %d / %d |' % (
            score.NAMES[lab], f[0]['n'], ' / '.join(_p(x['n_usable'], x['n']) for x in f),
            ' / '.join(_p(x['n_ok'], x['n']) for x in f),
            ' / '.join('%d of %d' % (x['n_wrong_silent'], x['n_ok_truth']) for x in f),
            ' / '.join('%d of %d' % (x['n_out_of_range'], x['n_usable_truth']) for x in f),
            t['versus_upstream']['frozen']['benefit'], t['versus_upstream']['frozen']['regress']))
    L += ['', '### Truth exclusions, by reason and set (leave every truth-based count, P27)', '',
          '| Set | truth_split_borderline | truth_truncated | truth_nan | truth_uncertain | total |', '|---|---|---|---|---|---|']
    for lab in ('spps-random', 'spps-energetic', 'ism', 'synth'):
        e = T[lab]['frozen']['excluded']
        L.append('| %s | %d | %d | %d | %d | %d |' % (score.NAMES[lab], e.get('truth_split_borderline', 0),
                 e.get('truth_truncated', 0), e.get('truth_nan', 0), e.get('truth_uncertain', 0), sum(e.values())))
    L += ['', '### Refusals, by reason and set (frozen / upstream)', '']
    for lab in ('spps-random', 'spps-energetic', 'ism', 'synth'):
        L.append('- %s: %s' % (score.NAMES[lab], ' / '.join(score._counts(T[lab][i]['refused']) for i in score.INSTANCES)))
    L += ['', '### receiver_too_large over all radii, by set and radius class (frozen)', '',
          '| Set | R <= 0.5 m | 0.5-1.0 m | R > 1.0 m |', '|---|---|---|---|']
    for lab in ('spps-random', 'spps-energetic', 'ism', 'synth'):
        t = T[lab]['rtl_by_r_class']
        L.append('| %s | %s |' % (score.NAMES[lab], ' | '.join('%d of %d (%s)' % (
            t[k]['n_receiver_too_large'], t[k]['n_rows'], _p(t[k]['n_receiver_too_large'], t[k]['n_rows'])) for k in score.R_CLASSES)))
    L += ['', '## The draws', '',
          '- ISM-fresh-2 rejections by reason (seed %s): %s' % (extra['ism_seed'], extra['ism_rejections']),
          '- Synth-fresh-2 rejections by reason (seed %s): %s' % (extra['synth_seed'], extra['synth_rejections']),
          '- Rows built: %s.' % extra['rows_built'], '',
          '## score.evaluate\'s own report', '']
    return '\n'.join(L) + '\n'


# ---- main --------------------------------------------------------------------------------------------
def script_commit():
    try:
        r = subprocess.run(['git', '-C', str(corpus.REPO), 'log', '-1', '--format=%h', '--',
                            str(Path(__file__).resolve())], capture_output=True, text=True, timeout=60)
        return r.stdout.strip() or 'uncommitted'
    except (OSError, subprocess.SubprocessError):
        return 'unknown'


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__.split('\n')[0])
    ap.add_argument('--out', required=True, help='a new or empty folder; never overwritten')
    ap.add_argument('--dry', action='store_true', help='planted and dev inputs only; no held-out data, no held-out seed')
    ap.add_argument('--data-root', default=str(DATA_ROOT))
    ap.add_argument('--log', default=None)
    ap.add_argument('--ism-workers', type=int, default=ISM_WORKERS)
    a = ap.parse_args(argv)
    out = Path(a.out)
    if not a.dry:
        try:
            preflight(data_root=Path(a.data_root), out=out)          # before anything is created, read or computed
        except driver.Refused as e:
            sys.exit('score_heldout refused: %s' % e)
    if out.exists() and any(out.iterdir()):
        sys.exit('%s exists and is not empty: a results folder is never reused' % out)
    out.mkdir(parents=True, exist_ok=True)
    log = Log(a.log or (out / 'progress.log' if a.dry else PROGRESS_LOG))
    t0 = time.monotonic()
    log('%sstart, out %s' % ('DRY ' if a.dry else '', out))

    if a.dry:
        spps = build_spps_dry(log)
        D = ism_fresh.draw(DRY_SEED)
        room = min(D['rooms'], key=lambda r: ism_fresh.n_images(r['dims_m'], 343.2 * r['image_time_s']))
        room = dict(room, receivers=room['receivers'][:DRY_ISM_RECEIVERS])
        D = dict(D, rooms=[room])
        specs = synth_fresh.draw(DRY_SEED, rejections=(syn_rej := {}))
        cells = {}
        for s in specs:
            cells.setdefault((s['ratio'], s['step_ms']), []).append(s)
        specs = [s for c in cells.values() for s in c[:DRY_SYNTH_PER_CELL]]
        ism_seed, synth_seed = DRY_SEED, DRY_SEED
    else:
        D = ism_fresh.draw(driver.ISM_SEED)
        syn_rej = {}
        specs = synth_fresh.draw(driver.SYNTH_SEED, rejections=syn_rej)
        spps = None
        ism_seed, synth_seed = driver.ISM_SEED, driver.SYNTH_SEED

    if spps is None:
        log('spps start: 144 tested runs, 28 truth runs')
        spps = build_spps(Path(a.data_root), log)
    log('spps done: %d rows' % len(spps))
    _dump(out / 'inputs_spps.pkl.gz', spps)

    log('ism start: %d rooms, %d workers' % (len(D['rooms']), a.ism_workers))
    ism = build_ism(D, log, a.ism_workers if not a.dry else 1)
    log('ism done: %d rows' % len(ism))
    _dump(out / 'inputs_ism.pkl.gz', ism)

    log('synth start: %d specs' % len(specs))
    synth = build_synth(specs)
    log('synth done: %d rows' % len(synth))
    _dump(out / 'inputs_synth.pkl.gz', synth)

    inputs = spps + ism + synth
    log('evaluate start: %d rows (frozen, upstream)' % len(inputs))
    summary = score.evaluate(inputs, out_dir=out / 'score')
    log('evaluate done')
    write_rows_csv(out / 'score', out / 'rows.csv.gz')

    extra = dict(ism_seed=ism_seed, synth_seed=synth_seed, ism_rejections=D['rejections'],
                 synth_rejections=syn_rej, dry=a.dry,
                 rows_built=dict(spps=len(spps), ism=len(ism), synth=len(synth)),
                 ism_rooms=[dict(id=r['id'], kind=r['kind'], parent=r['parent'], dims_m=r['dims_m'],
                                 alpha_walls=r['alpha_walls'], design_t60_s=r['design_t60_s']) for r in D['rooms']],
                 elapsed_s=round(time.monotonic() - t0))
    counts = counts_of(summary, extra)
    (out / 'counts.json').write_text(json.dumps(counts, indent=1, allow_nan=False, default=str) + '\n',
                                     encoding='utf-8', newline='\n')
    report = (out / 'score' / 'REPORT.md').read_text(encoding='utf-8')
    md = results_md(summary, extra, out.name, script_commit(), a.dry) + report
    (out / 'RESULTS.md').write_text(md, encoding='utf-8', newline='\n')
    log('end: %d rows scored, %d s, results in %s' % (len(inputs), time.monotonic() - t0, out))
    return 0


if __name__ == '__main__':
    sys.dont_write_bytecode = True
    sys.exit(main())
