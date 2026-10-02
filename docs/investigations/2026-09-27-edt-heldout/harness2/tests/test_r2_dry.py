"""T39 (../../HARNESS-PLAN-2.md section 6): the dry run on planted inputs, scored end to end by round 2's scorer
(frozen2/method.py and upstream's port on every row), every count and every H verdict equal to
`../expected_dry_2.json`, which was committed before this file's code existed (ddda75b) and is never edited to
make a value agree. Planted inputs only: round 1's D2 plant (432 rows of exact exponentials in one mock room),
one dead-room row from the corpus ISM generator, the room's geometry handed in through rows_from_runs'
geometry= argument. No round-2 held-out data, no solver, nothing outside pytest's tmp_path.
"""
import json

from conftest import INVESTIGATION

from dry import run_dry2


def test_t39_dry_run_equals_expected_dry_2(tmp_path):
    expected = json.loads((INVESTIGATION / 'expected_dry_2.json').read_text(encoding='utf-8'))
    assert expected['schema'] == 'm8b.expected_dry/2'
    res = run_dry2.run_d2p(tmp_path / 'd2p', expected['D2p'])
    assert res is not None and len(res) >= 60, 'the dry run compared too few values'
    bad = [r for r in res if r['match'] is False]
    assert not bad, 'mismatches against expected_dry_2.json: %s' % bad[:6]
    compared = {r['path'] for r in res if r['match'] is True}
    for must in ('status_tally.n', 'status_tally.n_refused', 'status_tally.refused.receiver_too_large', 'H2.pass', 'H3.pass',
                 'H4.n_before', 'H4.n_receiver_too_large', 'H4.n', 'H4.pass', 'rtl_by_r_class.<=0.5.n_receiver_too_large',
                 'H5.n_paired', 'method_sha256'):
        assert must in compared, must
