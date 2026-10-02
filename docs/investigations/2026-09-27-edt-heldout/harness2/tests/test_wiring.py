"""Step 7b (../../HARNESS-PLAN.md section 8.2's "Wiring owed" bullet, audit of 7481a67): calls 3 and
4 exist as functions (truth.split_borderline, ism_fresh.make_row(..., retry_truncated=True)) but
step 8.2 built no caller that a real held-out row must pass through. These tests exercise the two
callers step 7b adds (ism_fresh.rows_for_scoring, spps_rows.rows_from_runs) and are built to fail if
either one's wiring is removed: each was run once with its call commented out (never committed) to
confirm the failure, per the task's instruction.

No solver is run and nothing is read or written under a `heldout` folder: ISM-fresh rows are built
from a hand-written room dict (not ism_fresh.draw, so no seed, held out or not, is ever touched), and
SPPS-fresh rows are built from report.json/project.simpa fixtures written under pytest's own tmp_path,
never under driver.DATA_ROOT.
"""
import json
import math

from m8b import ism_fresh, method, rooms as rooms_mod, spps_rows, truth


# ---- call 4: ism_fresh.rows_for_scoring always retries a truncated truth -------------------------
def test_ism_fresh_rows_for_scoring_retries_every_row(monkeypatch):
    """Wiring check with no physics: make_row is replaced with a recorder, so the only thing under
    test is whether rows_for_scoring passes retry_truncated=True for every spec row_specs(D) gives
    it. Fails immediately if rows_for_scoring stops passing it (or passes False)."""
    seen = []

    def fake_make_row(room, rec, R, F, step_ms, image_time_s=None, run_s=None, retry_truncated=False):
        seen.append(retry_truncated)
        return dict(bins=__import__('numpy').array([1.0, 0.0]), dt=0.001, t_arrival=0.01, half_width=0.001,
                   truth_edt=1.0, truth_share=0.0, truth_status='ok')

    monkeypatch.setattr(ism_fresh, 'make_row', fake_make_row)
    monkeypatch.setattr(ism_fresh, 'BANDS_HZ', (125, 500))
    monkeypatch.setattr(ism_fresh, 'STEPS_MS', (1.0, 2.0))
    room = dict(id='drawn:1', dims_m=[20.0, 15.0, 10.0], alpha_walls=[0.2] * 6, source_m=[2.0, 2.0, 2.0],
               image_time_s=2.4, design_t60_s={125: 1.0, 500: 1.0},
               receivers=[dict(position_m=[10.0, 8.0, 5.0], R_m=0.31,
                               d_m=math.dist([2.0, 2.0, 2.0], [10.0, 8.0, 5.0]), **{'class': 'mid'})])
    D = dict(seed=1, rooms=[room], rejections={}, corpus_rooms_sha256='x')
    rows = ism_fresh.rows_for_scoring(D)
    assert len(rows) == 4, 'row_specs(D) with 2 bands x 2 steps x 1 receiver should give 4 rows'
    assert len(seen) == 4
    assert all(v is True for v in seen), (
        'every held-out ISM-fresh row must be built with make_row(..., retry_truncated=True) '
        '(HARNESS-PLAN.md 8.2, fourth call); rows_for_scoring passed %r' % (seen,))
    for r in rows:
        assert r['set'] == 'ism' and r['truth_status'] == 'ok' and r['room'] == 'drawn:1'


def test_ism_fresh_rows_for_scoring_real_retry_rescues_a_truncated_row():
    """Integration check with real physics (no mock): a room/receiver/band/step combination already
    known (step 8.2's own test, test_step8_2.py::test_call4...) to read truth_truncated at its image
    set and 'ok' after one 1.5x retry. Run through rows_for_scoring (not make_row directly), so this
    fails if the function stops wiring the retry in, not just if make_row's own retry breaks (T6,
    test_step8_2 already guard that). Dev-scale: one room, one receiver, one band, one step; ~15 s,
    confirmed below 20 s on this room beforehand."""
    import numpy as np  # noqa: F401  (ism_fresh needs it; imported for clarity only)

    src = [2.0, 2.0, 2.0]
    rec_pos = [10.0, 8.0, 5.0]
    room = dict(id='drawn:1', dims_m=[20.0, 15.0, 10.0], alpha_walls=[0.2] * 6, source_m=src,
               image_time_s=2.0, design_t60_s={125: 2.0},
               receivers=[dict(position_m=rec_pos, R_m=0.31, d_m=math.dist(src, rec_pos), **{'class': 'mid'})])
    D = dict(seed=1, rooms=[room], rejections={}, corpus_rooms_sha256='x')

    import pytest
    mp = pytest.MonkeyPatch()
    try:
        mp.setattr(ism_fresh, 'BANDS_HZ', (125,))
        mp.setattr(ism_fresh, 'STEPS_MS', (1.0,))
        rows = ism_fresh.rows_for_scoring(D)
    finally:
        mp.undo()

    assert len(rows) == 1
    row = rows[0]
    # at this image_time_s (2.0 s) the room is truncated unretried (step 8.2's own module docstring
    # and this test's own prior run, interactively, both read truth_truncated here) and converges at
    # 1.5x (moved_rel ~1.9e-5, far under ism_fresh.IMAGE_RETRY_REL_TOL): wired through
    # rows_for_scoring, the row must come back 'ok', not 'truth_truncated'.
    assert row['truth_status'] == 'ok', (
        'rows_for_scoring did not keep a truncated-but-converged row: retry_truncated was not passed '
        'to make_row (8.2\'s fourth call is unwired)')
    assert 'truth_truncated_retry' in row
    assert row['truth_truncated_retry']['moved_rel'] < ism_fresh.IMAGE_RETRY_REL_TOL


# ---- call 3: spps_rows.rows_from_runs applies truth.split_borderline ------------------------------
def _write_run(path, seed, report):
    path.mkdir(parents=True)
    (path / 'project.simpa').write_text(json.dumps({'solvers': {'spps': {'random_seed': seed}}}), encoding='utf-8')
    (path / 'report.json').write_text(json.dumps(report), encoding='utf-8')


def _report(bins):
    return {
        'solver_build': {'status': 'verified'},
        'spps': {
            'speed_of_sound_m_s': 343.2, 'receiver_radius_m': 0.31, 'time_step_s': 0.001,
            'point_receivers': [{'label': 'R000', 'arrival_s': 0.02,
                                 'bands': [{'freq_hz': 500, 'energy_pa2': bins}]}],
        },
    }


class _FakeMethod:
    def __init__(self, edt):
        self.edt = edt

    def analyse(self, bins, dt, t_arrival, meta=None):
        return dict(status='ok', edt=self.edt, edt_lo=self.edt, edt_hi=self.edt, reason='')


def _fake_room():
    return {
        'FakeRoom': dict(
            boxes=[[[0.0, 0.0, 0.0], [10.0, 8.0, 6.0]]], source_m=[2.0, 2.0, 2.0],
            receivers=[dict(position_m=[8.0, 2.0, 2.0], d_m=6.0, **{'class': 'mid'}, blocked=False)],
            design_t60_s={500: 1.0},
        )
    }


def _build(monkeypatch, tmp_path, *, edt, truth_val, split_result):
    bins = [1.0, 0.8, 0.6, 0.4, 0.2, 0.1, 0.05, 0.02]
    tested = tmp_path / 'tested'
    _write_run(tested, 1101, _report(bins))
    refs = []
    for i, s in enumerate((9001, 9002, 9003, 9004)):
        p = tmp_path / ('ref%d' % i)
        _write_run(p, s, _report(bins))
        refs.append(p)

    monkeypatch.setattr(spps_rows.method, 'load', lambda path=None: _FakeMethod(edt))

    def fake_assess(refs_, dt, t_arr, h, blocked=False):
        return dict(truth=truth_val, edts=[truth_val] * len(refs_), u=0.0, share=0.0, status='ok')
    monkeypatch.setattr(spps_rows.truth, 'assess', fake_assess)

    calls = []

    def fake_split(edt_, truth_, gap_s, h, band_hz):
        calls.append(dict(edt=edt_, truth=truth_, gap_s=gap_s, h=h, band_hz=band_hz))
        return split_result
    monkeypatch.setattr(spps_rows.truth, 'split_borderline', fake_split)

    rows = spps_rows.rows_from_runs('FakeRoom', tested, refs, mode='random', particles=150000, seed=1101,
                                    data_root=tmp_path, geometry=_fake_room()['FakeRoom'])
    return rows, calls


def test_spps_rows_applies_split_borderline_when_flagged(monkeypatch, tmp_path):
    """The wiring check: truth.split_borderline is called with this row's own edt/truth/gap/h/band
    (not placeholders), and when it returns True the row's truth_status is overridden to
    'truth_split_borderline' in place of assess()'s 'ok'. Fails if the SPPS row builder skips the
    call (truth_status would stay 'ok') or calls it with the wrong row's edt or truth."""
    rows, calls = _build(monkeypatch, tmp_path, edt=1.1, truth_val=1.0, split_result=True)
    assert len(rows) == 1
    assert len(calls) == 1, 'split_borderline must be called exactly once per otherwise-ok row'
    assert calls[0]['edt'] == 1.1 and calls[0]['truth'] == 1.0 and calls[0]['band_hz'] == 500
    assert rows[0]['truth_status'] == 'truth_split_borderline', (
        'spps_rows.rows_from_runs did not act on truth.split_borderline\'s True: the third call '
        '(HARNESS-PLAN.md 8.2) is unwired')
    assert rows[0]['truth'] == 1.0            # the row's own truth is unaffected by the relabel


def test_spps_rows_leaves_ok_when_not_borderline(monkeypatch, tmp_path):
    """The control: when split_borderline says False, the row stays 'ok', so the override above is
    conditional on the real return value, not an unconditional relabel that would also pass the
    first test for the wrong reason."""
    rows, calls = _build(monkeypatch, tmp_path, edt=1.1, truth_val=1.0, split_result=False)
    assert len(calls) == 1
    assert rows[0]['truth_status'] == 'ok'


def test_spps_rows_skips_split_borderline_when_truth_is_not_ok(monkeypatch, tmp_path):
    """split_borderline is about an 'ok' row's own wrong-silent risk (score.classify: wrong_silent is
    never computed for a row that already has no truth); a row assess() already excluded must not be
    probed or relabelled."""
    bins = [1.0, 0.8, 0.6, 0.4, 0.2, 0.1, 0.05, 0.02]
    tested = tmp_path / 'tested'
    _write_run(tested, 1101, _report(bins))
    refs = []
    for i, s in enumerate((9001, 9002, 9003, 9004)):
        p = tmp_path / ('ref%d' % i)
        _write_run(p, s, _report(bins))
        refs.append(p)
    monkeypatch.setattr(spps_rows.method, 'load', lambda path=None: _FakeMethod(1.1))

    def fake_assess_truncated(refs_, dt, t_arr, h, blocked=False):
        return dict(truth=float('nan'), edts=[float('nan')] * len(refs_), u=float('nan'), share=1.0,
                   status='truth_truncated')
    monkeypatch.setattr(spps_rows.truth, 'assess', fake_assess_truncated)
    calls = []
    monkeypatch.setattr(spps_rows.truth, 'split_borderline',
                        lambda *a, **k: calls.append(1) or True)

    rows = spps_rows.rows_from_runs('FakeRoom', tested, refs, mode='random', particles=150000, seed=1101,
                                    data_root=tmp_path, geometry=_fake_room()['FakeRoom'])
    assert rows[0]['truth_status'] == 'truth_truncated'
    assert calls == [], 'split_borderline must not be consulted for a row assess() already excluded'
