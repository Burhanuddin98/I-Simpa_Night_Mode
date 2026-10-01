"""Step 7c part 2 (HARNESS-PLAN.md section 5 item 3): runs D1, D2 and D3 through the harness's own
full pipeline (score.evaluate, which itself runs the frozen method via its sha256-checked loader,
the Z = 3 instance, and the upstream comparator wrapper, on every input) and compares every count
and every H verdict against ../expected_dry.json, which is FROZEN and is not edited by this script.

D1: the critique's ratio-3 family (scan2b's 'run_scans' form), rebuilt row by row from the same
construction as m8b.corpus.scan2b (HARNESS-PLAN.md section 5 item 3's amendment, commit 4c6f03a):
not a call to corpus.scan2b itself (which only returns tallies, not per-row inputs score.evaluate
needs), but the same math, read off corpus.py's own source (corpus.py:1112-1152) so a drift between
the two would be visible as a status_tally mismatch against weak_spots.json's committed numbers,
which expected_dry.json's D1 values are themselves read from.

D2: dry/build_d2.py's fixture, read back through m8b.driver.read_run and turned into score.evaluate
inputs by m8b.spps_rows.rows_from_runs, with m8b.rooms.rooms monkeypatched to the mock room (the
same technique tests/test_wiring.py uses, not an edit to m8b/rooms.py).

D3: res_ism.pkl (status_tally receipts) and dev/ism_rows.pkl (bins, dt, t_arrival, meta, truth_edt),
read once, read-only, from the main repo's target/ (corpus.find_target_root(), which resolves above
this worktree to the checkout expected_dry.json's own D3 section was built from), restricted to the
t1 and corridor rooms by the room token in each row's id, and re-run through score.evaluate rather
than trusted from the log, so the frozen method's current, hash-checked bytes decide every row here,
not whatever produced the pickle.

Nothing here reads or writes under C:\\tmp\\m8b-edt\\heldout\\, launches a solver, or changes m8b/,
tests/, PREREG.md, frozen/ or expected_dry.json. All scratch and output go under
C:\\tmp\\m8b-edt\\dry\\.

Usage (from harness/, with the venv and no bytecode):
    set PYTHONDONTWRITEBYTECODE=1 & C:\\tmp\\m8b-edt\\venv\\Scripts\\python.exe dry\\run_dry.py
"""
import ast
import itertools
import json
import math
import pickle
import sys
from pathlib import Path

sys.path.insert(0, '.')
sys.dont_write_bytecode = True

from m8b import corpus, driver, rooms as rooms_mod, score, spps_rows  # noqa: E402

from dry import build_d2  # noqa: E402

OUT = Path(r'C:\tmp\m8b-edt\dry')
EXPECTED_PATH = Path('../expected_dry.json')

RESULTS = []    # [{'d': 'D1', 'path': 'H1.synth.pass', 'expected': ..., 'actual': ..., 'match': bool}]


def record(d, path, expected, actual, match, note=''):
    RESULTS.append(dict(d=d, path=path, expected=expected, actual=actual, match=match, note=note))
    status = 'OK ' if match else ('SKIP' if match is None else 'MISMATCH')
    print('[%s] %-28s %-6s expected=%r actual=%r %s' % (d, path, status, expected, actual, note))


def check(d, path, expected, actual, note=''):
    record(d, path, expected, actual, expected == actual, note)


def check_close(d, path, expected, actual, atol, note=''):
    ok = expected is not None and actual is not None and abs(expected - actual) <= atol
    record(d, path, expected, actual, ok, note)


def check_ge(d, path, lower_bound, actual, note=''):
    ok = actual is not None and actual >= lower_bound
    record(d, path, '>= %r' % lower_bound, actual, ok, note)


def _subgroup_parts(key):
    """score.evaluate's summary is run through score._jsonable before it is returned (score.py:631),
    which turns every dict key, including H3's subgroup tuples, into score._key's '|'-joined string
    (an integer-valued float loses its decimal point: 1.0 -> '1', score.py:563-568). Splits it back
    apart; the inverse of _key for the tuples H3 actually produces (2- or 3-tuples after the set)."""
    return key.split('|')


# =================================================================================================
# D1
# =================================================================================================
def build_d1_inputs():
    S = corpus.load_synth()
    inputs = []
    for T60, dt_ms, frac, shape, drr_db in itertools.product(
            (0.5, 1.0, 2.0, 4.0, 8.0), (1.0, 2.0, 10.0),
            (0.15, 0.2, 0.25, 0.3, 0.35, 0.4, 0.5, 0.6, 0.8, 1.0, 1.5),
            ('single', 'double-20', 'double-15', 'double-10'), (-12.0, -6.0, -3.0)):
        dt = dt_ms * 1e-3
        k1 = 6.0 * math.log(10.0) / T60
        A, k, _late = corpus._double(shape, k1, 3.0)
        t_arr = 0.02 + 0.37 * dt
        Srev = sum(a / kk for a, kk in zip(A, k))
        Ed = Srev * 10 ** (drr_db / 10.0)
        truth = S.truth_edt(t_arr, Ed, 0.0, A, k)
        b = S.histogram(dt, t_arr + frac * T60, t_arr, 0.0, Ed, 0.0, A, k)
        if len(b) < 4:
            continue
        rid = 'd1|T%g|%gms|f%g|%s|drr%g' % (T60, dt_ms, frac, shape, drr_db)
        inputs.append(dict(set='synth', id=rid, bins=b, dt=dt, t_arrival=t_arr, meta={'half_width': 0.0},
                           truth=truth, truth_status='ok', family=3.0, step_ms=dt_ms))
    return inputs


def run_d1(expected):
    d = 'D1'
    inputs = build_d1_inputs()
    out_dir = OUT / 'D1' / 'score'
    summary = score.evaluate(inputs, out_dir=out_dir)
    check(d, 'method_sha256', corpus.FROZEN_SHA256, summary['method']['sha256'])

    check(d, 'row_count', expected['row_count']['value'], summary['inputs']['per_set']['synth'])

    t = summary['tables']['synth']['frozen']
    exp = expected['status_tally_frozen_method']
    check(d, 'status_tally.n', exp['n'], t['n'])
    check(d, 'status_tally.n_ok', exp['n_ok'], t['n_ok'])
    check(d, 'status_tally.n_refused', exp['n_refused'], t['n_refused'])
    check(d, 'status_tally.n_wide', exp['n_wide'], t['n_wide'])

    n_excluded = sum(t['excluded'].values())
    check(d, 'truth.n_with_truth', expected['truth']['n_with_truth'], t['n'] - n_excluded)
    check(d, 'truth.n_excluded', expected['truth']['n_excluded'], n_excluded)

    check(d, 'n_wrong_silent', expected['n_wrong_silent']['value'], t['n_wrong_silent'])

    crit = summary['criteria']['frozen']['random']
    h1 = crit['H1']['per_set']['synth']
    expH1 = expected['H1']
    check(d, 'H1.n_ok_truth', expH1['n_ok_truth'], h1['n_ok_truth'])
    check(d, 'H1.n_wrong_silent', expH1['n_wrong_silent'], h1['n_wrong_silent'])
    check(d, 'H1.pass', expH1['pass'], h1['pass'])

    h3 = crit['H3']
    subs = {k: v for k, v in h3['subgroups'].items() if _subgroup_parts(k)[0] == 'synth'}
    expH3 = expected['H3']
    check(d, 'H3.n_subgroups', expH3['n_subgroups'], len(subs))
    steps_seen = sorted({float(_subgroup_parts(k)[2]) for k in subs})
    check(d, 'H3.steps_ms', expH3['steps_ms'], steps_seen)
    for key, g in sorted(subs.items()):
        step = _subgroup_parts(key)[2]
        check_ge(d, 'H3.n_ok_truth[step=%s]' % step, expH3['n_ok_truth_per_step_lower_bound'],
                 g['n_ok_truth'])
        check(d, 'H3.n_wrong_silent[step=%s]' % step, expH3['n_wrong_silent_per_step'], g['n_wrong_silent'])
        check(d, 'H3.pass[step=%s]' % step, expH3['pass_per_step'], g['pass'])
    check(d, 'H3.pass', expH3['pass'], h3['pass'])

    h5 = crit['H5']['per_set']['synth']
    expH5 = expected['H5']
    check(d, 'H5.n_eligible', expH5['n_eligible'], h5['n_eligible'])
    check(d, 'H5.n_wrong_silent_frozen', expH5['n_wrong_silent_frozen'], h5['n_wrong_silent'])
    check(d, 'H5.n_wrong_silent_upstream', expH5['n_wrong_silent_upstream'], h5['n_wrong_silent_upstream'])
    check(d, 'H5.pass', expH5['pass'], h5['pass'])

    for h in ('H2', 'H4', 'H6'):
        record(d, '%s.applicable' % h, False, False, None,
              note='D1 carries no spps/attack rows; %s not asserted (h_verdict_scope_note)' % h)
    return summary


# =================================================================================================
# D2
# =================================================================================================
def run_d2(expected):
    d = 'D2'
    manifest = build_d2.build()
    rooms_mod.rooms = lambda: manifest['room']
    spps_rows.rooms_mod.rooms = lambda: manifest['room']

    inputs = []
    for step_key, dirs in manifest['tested'].items():
        for tested_dir in dirs:
            seed = int(Path(tested_dir).name.rsplit('seed', 1)[1])
            rows = spps_rows.rows_from_runs('mock_d2', tested_dir, manifest['refs'], mode='random',
                                            particles=0, seed=seed, data_root=build_d2.OUT)
            inputs.extend(rows)
    check(d, 'n_tested_rows', 432, len(inputs),
         note="432 = 9 tested dirs (3 steps x 3 seeds) x 48 (8 receivers x 6 bands)")

    out_dir = OUT / 'D2' / 'score'
    summary = score.evaluate(inputs, out_dir=out_dir)
    check(d, 'method_sha256', corpus.FROZEN_SHA256, summary['method']['sha256'])

    t = summary['tables']['spps-random']['frozen']
    exp = expected['status_tally']
    check(d, 'status_tally.n', exp['n'], t['n'])
    check(d, 'status_tally.n_ok', exp['n_ok'], t['n_ok'])
    check(d, 'status_tally.n_refused', exp['n_refused'], t['n_refused'])
    check(d, 'status_tally.n_wide', exp['n_wide'], t['n_wide'])
    check(d, 'status_tally.n_usable', exp['n_usable'], t['n_usable'])

    expE = expected['truth_exclusions']
    exc = t['excluded']
    check(d, 'truth_exclusions.truth_uncertain', expE['truth_uncertain'], exc.get('truth_uncertain', 0))
    check(d, 'truth_exclusions.truth_truncated', expE['truth_truncated'], exc.get('truth_truncated', 0))
    check(d, 'truth_exclusions.truth_nan', expE['truth_nan'], exc.get('truth_nan', 0))
    check(d, 'truth_exclusions.total_excluded', expE['total_excluded'], sum(exc.values()))
    check(d, 'truth_exclusions.n_ok_truth', expE['n_ok_truth'], t['n_ok_truth'])

    check(d, 'n_wrong_silent', expected['n_wrong_silent']['value'], t['n_wrong_silent'])

    crit = summary['criteria']['frozen']['random']
    h2 = crit['H2']
    expH2 = expected['H2']
    check(d, 'H2.n_usable_truth', expH2['n_usable_truth'], h2['n_usable_truth'])
    check(d, 'H2.n_covered', expH2['n_covered'], h2['n_covered'])
    check(d, 'H2.n_ok_truth', expH2['n_ok_truth'], h2['n_ok_truth'])
    check(d, 'H2.n_wrong_silent', expH2['n_wrong_silent'], h2['n_wrong_silent'])
    check(d, 'H2.coverage_pass', expH2['coverage_pass'], h2['coverage_pass'])
    check(d, 'H2.wrong_silent_pass', expH2['wrong_silent_pass'], h2['wrong_silent_pass'])
    check(d, 'H2.pass', expH2['pass'], h2['pass'])

    h3 = crit['H3']
    subs = {k: v for k, v in h3['subgroups'].items() if _subgroup_parts(k)[0] == 'spps'}
    expH3 = expected['H3']
    check(d, 'H3.n_subgroups', expH3['n_subgroups'], len(subs))
    check(d, 'H3.n_failing', expH3['n_failing'], h3['n_failing'])
    check(d, 'H3.pass', expH3['pass'], h3['pass'])
    for key_txt, exp_g in expH3['subgroups'].items():
        cls, step_txt = key_txt.split('|')
        step = float(step_txt)
        key = next((k for k in subs if _subgroup_parts(k)[2] == cls
                   and float(_subgroup_parts(k)[3]) == step), None)
        g = subs.get(key)
        check(d, 'H3.subgroup[%s].n_ok_truth' % key_txt, exp_g['n_ok_truth'], g['n_ok_truth'] if g else None)
        check(d, 'H3.subgroup[%s].n_wrong_silent' % key_txt, exp_g['n_wrong_silent'],
             g['n_wrong_silent'] if g else None)
        check(d, 'H3.subgroup[%s].pass' % key_txt, exp_g['pass'], g['pass'] if g else None)

    h4 = crit['H4']['per_set']['spps']
    expH4 = expected['H4']
    check(d, 'H4.n', expH4['n'], h4['n'])
    check(d, 'H4.n_usable', expH4['n_usable'], h4['n_usable'])
    check(d, 'H4.n_ok', expH4['n_ok'], h4['n_ok'])
    check(d, 'H4.pass', expH4['pass'], h4['pass'])

    record(d, 'H1.applicable', False, False, None, note='D2 carries only spps rows (H1_SETS excludes spps)')
    record(d, 'H5', None, 'not computed: dry-run plant text says D2.H5 stays null '
                          '(real bins ARE materialised here, unlike when expected_dry.json was written, but the '
                          'frozen table is not edited to add a value it deliberately left out)', None,
          note='informational only')
    record(d, 'H6.applicable', False, False, None, note='no attack class in D2')

    # ---- each planted fault lands in its own counter (its own row-location, not just a count) -------
    rows_by_id = {x['id']: x for x in inputs}
    by_loc = {}
    for rid, x in rows_by_id.items():
        parts = rid.split('|')          # spps|mock_d2|R00n|<band>|<step>ms|seed<seed>
        by_loc.setdefault((parts[2], float(parts[3])), []).append(x)

    def ts_count(label, receiver, band, want_status):
        cell = by_loc.get((receiver, band), [])
        n = sum(1 for x in cell if x['truth_status'] == want_status)
        check('D2 FAULT', '%s -> truth_status=%s (receiver %s band %s)' % (label, want_status, receiver, band),
             9, n)

    ts_count('disagreeing_references', 'R000', 125.0, 'truth_uncertain')
    ts_count('truncated_reference', 'R001', 250.0, 'truth_truncated')
    ts_count('nan_truth', 'R002', 500.0, 'truth_nan')

    blocked_rows = [x for x in rows_by_id.values() if x['id'].split('|')[2] == 'R007']
    check('D2 FAULT', 'blocked_receiver -> rows at R007 (r8)', 54, len(blocked_rows))
    blocked_ok_truth = sum(1 for x in blocked_rows if x['truth_status'] == 'ok')
    check('D2 FAULT', 'blocked_receiver -> truth_status=ok at R007', 54, blocked_ok_truth)

    mult_ids = {x['id'] for x in inputs
               if x['id'].split('|')[2] in ('R003', 'R004', 'R005') and abs(x['dt'] * 1e3 - 2.0) < 1e-6}
    check('D2 FAULT', 'method_multiplies_by_1.08 -> rows at r4/r5/r6, step=2ms', 54, len(mult_ids))
    rows_jsonl = (out_dir / 'rows.jsonl').read_text(encoding='utf-8').splitlines()
    recs = {json.loads(line)['id']: json.loads(line) for line in rows_jsonl}
    mult_wrong = sum(1 for rid in mult_ids if recs[rid]['frozen']['wrong_silent'])
    mult_not_covered = sum(1 for rid in mult_ids if recs[rid]['frozen']['covered'] is False)
    check('D2 FAULT', 'method_multiplies_by_1.08 -> wrong_silent at those 54 rows', 54, mult_wrong)
    check('D2 FAULT', 'method_multiplies_by_1.08 -> NOT covered at those 54 rows', 54, mult_not_covered)

    return summary


# =================================================================================================
# D3
# =================================================================================================
ROOM_SRC = {'t1': (3.0, 5.0, 1.8), 'corridor': (5.03, 1.97, 1.53)}
ROOM_DIMS = {'t1': (6.0, 10.0, 3.0), 'corridor': (20.0, 4.0, 3.0)}
ROOM_ALPHA_PAIRS = {
    't1': [(0.2, 0.2), (0.2, 0.2), (0.1, 0.3)],
    'corridor': [(0.05, 0.05), (0.05, 0.05), (0.6, 0.05)],
}


def _design_t60_table(room):
    alpha6 = [a for pair in ROOM_ALPHA_PAIRS[room] for a in pair]
    return {band: corpus.design_t60(ROOM_DIMS[room], alpha6, float(band))
           for band in (125, 250, 500, 1000, 2000, 4000, 8000, 16000, 20000)}


def build_d3_inputs():
    final = corpus.find_target_root() / 'agents' / 'edt-simplify' / 'final'
    res = pickle.loads((final / 'res_ism.pkl').read_bytes())
    rows_in = {r['id']: r for r in pickle.loads((final / 'dev' / 'ism_rows.pkl').read_bytes())}
    t60 = {room: _design_t60_table(room) for room in ROOM_SRC}

    inputs, logged_by_id, per_room_ids = [], {}, {'t1': [], 'corridor': []}
    for r in res:
        parts = r['id'].split('|')
        room = parts[1]
        if room not in ROOM_SRC:
            continue
        rec_pos = ast.literal_eval(parts[2])
        band_hz = float(parts[3])
        step_ms = float(parts[4].rstrip('ms'))
        d_m = math.dist(ROOM_SRC[room], rec_pos)
        inp = rows_in[r['id']]
        inputs.append(dict(set='ism', id=r['id'], bins=inp['bins'], dt=inp['dt'], t_arrival=inp['t_arrival'],
                           meta=inp['meta'], truth=inp['truth_edt'], truth_status='ok', room=room, d_m=d_m,
                           step_ms=step_ms, band_hz=band_hz, design_t60_s=t60[room].get(band_hz)))
        logged_by_id[r['id']] = r
        per_room_ids[room].append(r['id'])
    return inputs, logged_by_id, per_room_ids


def run_d3(expected):
    d = 'D3'
    inputs, logged_by_id, per_room_ids = build_d3_inputs()
    check(d, 'row_count.t1', expected['row_count_for_t1_and_corridor']['t1'], len(per_room_ids['t1']))
    check(d, 'row_count.corridor', expected['row_count_for_t1_and_corridor']['corridor'],
         len(per_room_ids['corridor']))
    check(d, 'row_count.total', expected['row_count_for_t1_and_corridor']['total'], len(inputs))

    out_dir = OUT / 'D3' / 'score'
    summary = score.evaluate(inputs, out_dir=out_dir)
    check(d, 'method_sha256', corpus.FROZEN_SHA256, summary['method']['sha256'])

    rows_jsonl = (out_dir / 'rows.jsonl').read_text(encoding='utf-8').splitlines()
    recs = {json.loads(line)['id']: json.loads(line) for line in rows_jsonl}

    def room_tally(room):
        ids = per_room_ids[room]
        n_ok = sum(1 for i in ids if recs[i]['frozen']['status'] == 'ok')
        n_wide = sum(1 for i in ids if recs[i]['frozen']['status'] == 'wide')
        n_refused = sum(1 for i in ids if recs[i]['frozen']['status'] == 'refused')
        return n_ok, n_wide, n_refused

    expT = expected['status_tally_t1_and_corridor']
    for room in ('t1', 'corridor'):
        n_ok, n_wide, n_refused = room_tally(room)
        check(d, 'status_tally.%s.n_ok' % room, expT[room]['n_ok'], n_ok)
        check(d, 'status_tally.%s.n_wide' % room, expT[room]['n_wide'], n_wide)
        check(d, 'status_tally.%s.n_refused' % room, expT[room]['n_refused'], n_refused)
    t_combined = summary['tables']['ism']['frozen']
    check(d, 'status_tally.combined.n_ok', expT['combined']['n_ok'], t_combined['n_ok'])
    check(d, 'status_tally.combined.n_wide', expT['combined']['n_wide'], t_combined['n_wide'])
    check(d, 'status_tally.combined.n_refused', expT['combined']['n_refused'], t_combined['n_refused'])
    check(d, 'status_tally.combined.n', expT['combined']['n'], t_combined['n'])

    expN = expected['truth_nan_exclusions']
    n_nan = {room: sum(1 for i in per_room_ids[room] if recs[i]['excluded_as'] == 'truth_nan')
            for room in ('t1', 'corridor')}
    check(d, 'truth_nan_exclusions.t1', expN['t1'], n_nan['t1'])
    check(d, 'truth_nan_exclusions.corridor', expN['corridor'], n_nan['corridor'])
    check(d, 'truth_nan_exclusions.total', expN['total'], n_nan['t1'] + n_nan['corridor'])

    check(d, 'n_wrong_silent', expected['n_wrong_silent']['value'], t_combined['n_wrong_silent'])

    # near-miss: not a score.py concept (it has no 'near_miss' field); computed here the same way
    # weak_spots.json defines it (corpus.py NEAR = 2.5 %, status ok and 2.5 % < |err| <= 5 %), over the
    # same frozen-method rows this run just produced, as an independent check of expected_dry.json's
    # n_near_miss derivation (which read it off weak_spots.json instead).
    def is_near_miss(rec_):
        f = rec_['frozen']
        return f['status'] == 'ok' and f['err'] is not None and corpus.NEAR < abs(f['err']) <= score.JND
    n_near_miss = sum(1 for i in (per_room_ids['t1'] + per_room_ids['corridor']) if is_near_miss(recs[i]))
    check(d, 'n_near_miss', expected['n_near_miss']['value'], n_near_miss)

    crit = summary['criteria']['frozen']['random']
    h1 = crit['H1']['per_set']['ism']
    expH1 = expected['H1']
    check(d, 'H1.n_ok_truth', expH1['n_ok_truth'], h1['n_ok_truth'])
    check(d, 'H1.n_wrong_silent', expH1['n_wrong_silent'], h1['n_wrong_silent'])
    check(d, 'H1.pass', expH1['pass'], h1['pass'])

    h3 = crit['H3']
    subs = {k: v for k, v in h3['subgroups'].items() if _subgroup_parts(k)[0] == 'ism'}
    expH3 = expected['H3']
    check(d, 'H3.n_subgroups', expH3['n_subgroups'], len(subs))
    check(d, 'H3.n_judged', expH3['n_judged'], sum(g['judged'] for g in subs.values()))
    check(d, 'H3.n_failing', expH3['n_failing'], sum(not g['pass'] for g in subs.values()))
    check(d, 'H3.pass', expH3['pass'], all(g['pass'] for g in subs.values()))
    for key_txt, exp_g in expH3['subgroups'].items():
        room, cls, step_txt = key_txt.split('|')
        step = float(step_txt)
        key = next((k for k in subs if _subgroup_parts(k)[1] == room and _subgroup_parts(k)[2] == cls
                   and float(_subgroup_parts(k)[3]) == step), None)
        g = subs.get(key)
        check(d, 'H3.subgroup[%s].n_ok_truth' % key_txt, exp_g['n_ok_truth'], g['n_ok_truth'] if g else None)
        check(d, 'H3.subgroup[%s].judged' % key_txt, exp_g['judged'], g['judged'] if g else None)
        check(d, 'H3.subgroup[%s].pass' % key_txt, exp_g['pass'], g['pass'] if g else None)

    h4 = crit['H4']['per_set']['ism']
    expH4 = expected['H4']
    check(d, 'H4.n', expH4['n'], h4['n'])
    check(d, 'H4.n_usable', expH4['n_usable'], h4['n_usable'])
    check_close(d, 'H4.usable_share', expH4['usable_share'], h4['usable_share'], 1e-4)
    check(d, 'H4.pass', expH4['pass'], h4['pass'])
    for room in ('t1', 'corridor'):
        ids = per_room_ids[room]
        n = sum(1 for i in ids if score._step(dict(step_ms=recs[i]['step_ms'])) == 1.0)
        n_usable = sum(1 for i in ids if score._step(dict(step_ms=recs[i]['step_ms'])) == 1.0
                      and recs[i]['frozen']['usable'])
        exp_room = expH4['per_room'][room]
        check(d, 'H4.per_room[%s].n' % room, exp_room['n'], n)
        check(d, 'H4.per_room[%s].n_usable' % room, exp_room['n_usable'], n_usable)

    h5 = crit['H5']['per_set']['ism']
    expH5 = expected['H5']
    check(d, 'H5.n_eligible', expH5['n_eligible'], h5['n_eligible'])
    check(d, 'H5.n_wrong_silent_frozen', expH5['n_wrong_silent_frozen'], h5['n_wrong_silent'])
    check(d, 'H5.n_wrong_silent_upstream', expH5['n_wrong_silent_upstream'], h5['n_wrong_silent_upstream'])
    check(d, 'H5.pass', expH5['pass'], h5['pass'])

    record(d, 'H2.applicable', False, False, None, note='D3 carries ism rows, not spps')
    record(d, 'H6.applicable', False, False, None, note='no attack class in D3')
    return summary


# =================================================================================================
def main():
    expected = json.loads(EXPECTED_PATH.read_text(encoding='utf-8'))
    progress = Path(r'C:\tmp\m8b-edt\progress.log')

    def log(msg):
        import datetime
        line = '%s step7c-dry: %s\n' % (datetime.datetime.now().strftime('%H:%M'), msg)
        with open(progress, 'a', encoding='utf-8') as f:
            f.write(line)
        print(line, end='')

    log('D1 start (scan2b run_scans form, 1,980-row reconstruction)')
    run_d1(expected['D1'])
    n_mismatch_d1 = sum(1 for r in RESULTS if r['d'] == 'D1' and r['match'] is False)
    log('D1 done: %d checks, %d mismatches' % (sum(1 for r in RESULTS if r['d'] == 'D1'), n_mismatch_d1))

    log('D2 start (build_d2 fixture under C:/tmp/m8b-edt/dry/D2)')
    run_d2(expected['D2'])
    n_mismatch_d2 = sum(1 for r in RESULTS if r['d'] in ('D2', 'D2 FAULT') and r['match'] is False)
    log('D2 done: %d checks, %d mismatches' %
       (sum(1 for r in RESULTS if r['d'] in ('D2', 'D2 FAULT')), n_mismatch_d2))

    log('D3 start (res_ism.pkl + ism_rows.pkl, t1+corridor, read-only from target/)')
    run_d3(expected['D3'])
    n_mismatch_d3 = sum(1 for r in RESULTS if r['d'] == 'D3' and r['match'] is False)
    log('D3 done: %d checks, %d mismatches' % (sum(1 for r in RESULTS if r['d'] == 'D3'), n_mismatch_d3))

    (OUT / 'comparison.json').write_text(json.dumps(RESULTS, indent=1, default=str), encoding='utf-8')
    n_total = len(RESULTS)
    n_mismatch = sum(1 for r in RESULTS if r['match'] is False)
    log('all done: %d values compared, %d mismatches, written C:/tmp/m8b-edt/dry/comparison.json' %
       (n_total, n_mismatch))
    print('\n=== %d values compared, %d mismatches ===' % (n_total, n_mismatch))
    if n_mismatch:
        for r in RESULTS:
            if r['match'] is False:
                print('MISMATCH [%s] %s: expected %r, got %r' % (r['d'], r['path'], r['expected'], r['actual']))
    return 1 if n_mismatch else 0


if __name__ == '__main__':
    sys.exit(main())
