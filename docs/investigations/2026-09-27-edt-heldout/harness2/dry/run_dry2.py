"""D2' of round 2's dry run (HARNESS-PLAN-2.md section 6, T39): round 1's D2 plant (dry/build_d2.py: 432 SPPS rows of exact
exponentials in one mock room, K = 4 references at 0.1 ms, five planted faults) plus ONE planted dead-room row that
frozen2/method.py must refuse as receiver_too_large, scored end to end by round 2's scorer (score.evaluate: frozen2 and
upstream's port on every row) and compared, value by value, with ../expected_dry_2.json, which was committed before this
code existed and is never edited to make a value agree.

What is planted and what is not real: the 432 rows are build_d2's, read back through driver.read_run and
spps_rows.rows_from_runs with the mock room's geometry handed in as `geometry=` (the G-room path; nothing is patched);
the dead-room row is made by the corpus ISM generator (ism_fresh.make_row: a 6 x 5 x 3 m box at alpha 0.65, a 0.31 m ball,
1 ms steps) and given as an SPPS input directly. Nothing is a round-2 held-out datum, no solver runs, and every file is
written under the folder the caller names (a tmp_path on C:).

    run_d2p(out_root, expected_D2p) -> [{'path', 'expected', 'actual', 'match'}]   (match None: informational)
    python dry/run_dry2.py <out folder>            # from harness2/; prints each value and returns 1 on a mismatch
"""
import json
import math
import sys
from pathlib import Path

sys.path.insert(0, '.')
sys.dont_write_bytecode = True

from m8b import corpus, ism_fresh, score, spps_rows  # noqa: E402

from dry import build_d2  # noqa: E402

EXPECTED_PATH = Path(__file__).resolve().parent.parent.parent / 'expected_dry_2.json'
DEAD_ID = 'spps|mock_d2|R099|1000|1ms|seed999|random'
DEAD_ROOM = dict(dims_m=[6.0, 5.0, 3.0], alpha_walls=[0.65] * 6, source_m=[1.0, 1.0, 1.2], image_time_s=0.8)
DEAD_REC = [4.5, 3.8, 1.3]


def dead_room_row():
    r = ism_fresh.make_row(DEAD_ROOM, DEAD_REC, 0.31, 1000.0, 1.0, run_s=0.6)
    return dict(set='spps', id=DEAD_ID, bins=r['bins'], dt=r['dt'], t_arrival=r['t_arrival'], meta={'half_width': r['half_width']},
                truth=r['truth_edt'], truth_status=r['truth_status'], room='mock_d2',
                d_m=math.dist(DEAD_ROOM['source_m'], DEAD_REC), step_ms=1.0, band_hz=1000, design_t60_s=1.0, mode='random',
                particles=0, seed=999, R_m=0.31)


def _parts(key):
    return key.split('|')


def run_d2p(out_root, expected):
    out_root = Path(out_root)
    res = []

    def record(path, exp, act, match=None):
        res.append(dict(path=path, expected=exp, actual=act, match=(exp == act) if match is None else match))

    def check(path, exp, act):
        record(path, exp, act)

    def check_close(path, exp, act, atol=1e-9):
        record(path, exp, act, exp is not None and act is not None and abs(exp - act) <= atol)

    manifest = build_d2.build(out_root / 'fixture')
    geometry = manifest['room']['mock_d2']
    inputs = []
    for dirs in manifest['tested'].values():
        for tdir in dirs:
            seed = int(Path(tdir).name.rsplit('seed', 1)[1])
            inputs.extend(spps_rows.rows_from_runs('mock_d2', tdir, manifest['refs'], mode='random', particles=0, seed=seed,
                                                   data_root=manifest['out'], geometry=geometry))
    check('n_plant_rows', 432, len(inputs))
    dead = dead_room_row()
    inputs.append(dead)
    check('n_tested_rows', 433, len(inputs))
    check('dead_row.R_m', 0.31, dead['R_m'])
    check('plant_rows_carry_R_m', True, all(x['R_m'] == 0.31 for x in inputs))

    summary = score.evaluate(inputs, out_dir=out_root / 'score')
    check('method_sha256', corpus.FROZEN2_SHA256, summary['method']['sha256'])
    check('Z', 2.5, summary['method']['Z'])
    t = summary['tables']['spps-random']['frozen']
    st = expected['status_tally']
    for k in ('n', 'n_ok', 'n_usable', 'n_refused', 'n_wide'):
        check('status_tally.' + k, st[k], t[k])
    check('status_tally.refused.receiver_too_large', st['refused_by_reason']['receiver_too_large'], t['refused'].get('receiver_too_large'))
    check('status_tally.refused.only', st['refused_by_reason'], t['refused'])
    ex = expected['truth_exclusions']
    for k in ('truth_uncertain', 'truth_truncated', 'truth_nan'):
        check('truth_exclusions.' + k, ex[k], t['excluded'].get(k, 0))
    check('truth_exclusions.total_excluded', ex['total_excluded'], sum(t['excluded'].values()))
    check('truth_exclusions.n_ok_truth', ex['n_ok_truth'], t['n_ok_truth'])
    check('n_wrong_silent', expected['n_wrong_silent']['value'], t['n_wrong_silent'])

    crit = summary['criteria']['frozen']['random']
    h2, e2 = crit['H2'], expected['H2']
    for k in ('n_usable_truth', 'n_covered', 'n_ok_truth', 'n_wrong_silent', 'coverage_pass', 'wrong_silent_pass', 'pass'):
        check('H2.' + k, e2[k], h2[k])
    h3, e3 = crit['H3'], expected['H3']
    subs = {k: v for k, v in h3['subgroups'].items() if _parts(k)[0] == 'spps'}
    check('H3.n_subgroups', e3['n_subgroups'], len(subs))
    check('H3.n_failing', e3['n_failing'], h3['n_failing'])
    check('H3.pass', e3['pass'], h3['pass'])
    for key_txt, eg in e3['subgroups'].items():
        cls, step_txt = key_txt.split('|')
        g = next((v for k, v in subs.items() if _parts(k)[2] == cls and float(_parts(k)[3]) == float(step_txt)), None)
        for f in ('n_ok_truth', 'n_wrong_silent', 'judged', 'pass'):
            check('H3.subgroup[%s].%s' % (key_txt, f), eg[f], g[f] if g else None)

    h4, e4 = crit['H4']['per_set']['spps'], expected['H4']
    for k in ('n_before', 'n_receiver_too_large', 'n', 'n_usable', 'n_ok', 'refused', 'pass'):
        check('H4.' + k, e4[k], h4[k])
    check_close('H4.share_receiver_too_large', e4['share_receiver_too_large'], h4['share_receiver_too_large'])
    check_close('H4.usable_share', e4['usable_share'], h4['usable_share'])
    check_close('H4.ok_share', e4['ok_share'], h4['ok_share'])

    rtl, er = summary['tables']['spps-random']['rtl_by_r_class'], expected['rtl_by_r_class']
    for cls in ('<=0.5', '0.5-1.0', '>1.0'):
        check('rtl_by_r_class.%s.n_rows' % cls, er[cls]['n_rows'], rtl[cls]['n_rows'])
        check('rtl_by_r_class.%s.n_receiver_too_large' % cls, er[cls]['n_receiver_too_large'], rtl[cls]['n_receiver_too_large'])
    # Energetic scored as its own set: the Random rows never feed it
    check('spps-energetic.n', 0, summary['tables']['spps-energetic']['frozen']['n'])

    h5, e5 = crit['H5']['per_set']['spps'], expected['H5']
    check('H5.n_paired', e5['n_paired'], h5['n_paired'])
    check('H5.n_eligible', e5['n_eligible'], h5['n_eligible'])
    check('H5.n_wrong_silent_frozen', e5['n_wrong_silent_frozen'], h5['n_wrong_silent'])
    res.append(dict(path='H5.n_wrong_silent_upstream', expected=None, actual=h5['n_wrong_silent_upstream'], match=None))
    # (informational: the plant does not fix upstream's count)

    # each planted fault lands in its own counter, by row location
    recs = {}
    for line in (out_root / 'score' / 'rows.jsonl').read_text(encoding='utf-8').splitlines():
        r = json.loads(line)
        recs[r['id']] = r
    by_loc = {}
    for x in inputs:
        parts = x['id'].split('|')
        by_loc.setdefault((parts[2], float(parts[3])), []).append(x)

    def ts_count(label, receiver, band, want):
        n = sum(1 for x in by_loc.get((receiver, band), []) if x['truth_status'] == want)
        check('FAULT %s -> truth_status=%s (%s, %g Hz)' % (label, want, receiver, band), 9, n)

    ts_count('disagreeing_references', 'R000', 125.0, 'truth_uncertain')
    ts_count('truncated_reference', 'R001', 250.0, 'truth_truncated')
    ts_count('nan_truth', 'R002', 500.0, 'truth_nan')
    blocked = [x for x in inputs if x['id'].split('|')[2] == 'R007']
    check('FAULT blocked_receiver -> rows at R007', 54, len(blocked))
    check('FAULT blocked_receiver -> truth_status ok', 54, sum(1 for x in blocked if x['truth_status'] == 'ok'))
    mult = {x['id'] for x in inputs if x['id'].split('|')[2] in ('R003', 'R004', 'R005') and abs(x['dt'] * 1e3 - 2.0) < 1e-6}
    check('FAULT method_multiplies_by_1.08 -> rows at r4-r6, 2 ms', 54, len(mult))
    check('FAULT 1.08 -> wrong_silent', 54, sum(1 for i in mult if recs[i]['frozen']['wrong_silent']))
    check('FAULT 1.08 -> not covered', 54, sum(1 for i in mult if recs[i]['frozen']['covered'] is False))
    d = recs[DEAD_ID]
    check('FAULT dead_room -> frozen status', 'refused', d['frozen']['status'])
    check('FAULT dead_room -> frozen reason', 'receiver_too_large', d['frozen']['reason'])
    check('FAULT dead_room -> never wrong_silent', False, d['frozen']['wrong_silent'])
    return res


def main(argv=None):
    out = Path((argv or sys.argv[1:])[0])
    expected = json.loads(EXPECTED_PATH.read_text(encoding='utf-8'))['D2p']
    res = run_d2p(out, expected)
    for r in res:
        print('[%s] %-62s expected=%r actual=%r' % ('OK ' if r['match'] else 'SKIP' if r['match'] is None else 'MISMATCH',
                                                     r['path'], r['expected'], r['actual']))
    bad = [r for r in res if r['match'] is False]
    print('=== %d values compared, %d mismatches ===' % (len(res), len(bad)))
    return 1 if bad else 0


if __name__ == '__main__':
    sys.exit(main())
