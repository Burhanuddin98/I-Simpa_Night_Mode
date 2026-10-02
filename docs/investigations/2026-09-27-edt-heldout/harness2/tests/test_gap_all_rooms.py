"""HARNESS-PLAN.md 8.2's last call, as amended 12:14 (audit of 03470f9, then `ff85de8`):
m8b/spps_rows.py:_gap_s used to read room_geom['boxes'][0] alone for every room, so an F3 or F4
receiver (both 'non-box', multi-box rooms, rooms.py:268) got its first-reflection gap off the main
box's walls only, silently wrong whenever the true shortest image-to-receiver path runs through a
different box (the doorway, the second leg) -- fixed in `ff85de8` by mirroring across every boundary
face of the room's own geometry. That fix still mirrored across a face regardless of which side of it
the source and receiver were actually on, which could mirror a point across a face it could never
really reflect off (a partition face on the far side from the source), giving gaps that went negative
(F3 rec1 -0.55 ms, F4 rec7 -9.92 ms -- see this file's git history for the values before this
amendment). The fix here: a face counts only when the source AND the receiver are both strictly in
front of it (its inward normal's side, where the owning box's air is).

The reference here is corpus.first_order_d1 (a module _gap_s does not call for non-box rooms) applied
by hand to one box at a time, never spps_rows's own old code (never reintroduced): a module under
test is never its own reference (conftest.py's rule).

No solver is run, nothing is read or written under a `heldout` folder, and rooms.rooms() is pure
geometry (no project, mesh or solver touched).
"""
import json
import math

import pytest

from m8b import corpus, rooms, rooms2, spps_rows

R = dict(rooms.rooms(), **rooms2.rooms())      # round 2: the G rooms join the F rooms and the probes


def _single_box_gap(lo, hi, source_m, rec_m):
    """The old, box-only, unfiltered formula (corpus.first_order_d1 in that box's local frame),
    applied to one box's six walls in isolation, with no front-side test -- what
    m8b/spps_rows.py:_gap_s computed from room_geom['boxes'][0] alone before `ff85de8`, and what
    HARNESS-PLAN.md 8.2's last call (pre-amendment) called 'the gap from any single box's walls'."""
    dims = [h - l for l, h in zip(lo, hi)]
    src = [s - l for s, l in zip(source_m, lo)]
    rec = [r - l for r, l in zip(rec_m, lo)]
    d = math.dist(src, rec)
    d1 = corpus.first_order_d1(src, rec, dims)
    return (d1 - d) / corpus.C_SPPS


ALL_ROOMS = sorted(R)


# ---- (1) box rooms: the new gap equals the old box formula exactly -------------------------------
def test_box_rooms_match_first_order_d1_to_1e12_relative():
    """Every room rooms.rooms() marks 'box' (a single box, lo at the origin in every F/P plan) has
    exactly six boundary faces: its own six walls. Every source and every receiver sits more than 0 m
    (rooms.py's 0.6 m CLEARANCE_M) inside every one of them, so the front-side test (added this
    amendment) never excludes a wall there, and mirroring across _Grid's faces must give the same six
    images corpus.first_order_d1 computes directly: the two gaps should agree to float rounding, not
    merely to some workable tolerance."""
    checked = 0
    for name, room in R.items():
        assert room['kind'] in ('box', 'non-box'), name
        if room['kind'] != 'box':
            continue
        (lo, hi), = room['boxes']
        for idx, rec in enumerate(room['receivers']):
            new = spps_rows._gap_s(room, idx)
            old = _single_box_gap(lo, hi, room['source_m'], rec['position_m'])
            assert new == pytest.approx(old, rel=1e-12, abs=1e-15), (name, idx, new, old)
            checked += 1
    assert checked == 12 * 8, 'expected 12 box rooms (F1 F2 F5 F6 F7 P0 P0b G1 G2 G5 G6 G7) x 8 receivers'


def test_box_room_every_wall_fronts_every_source_and_receiver():
    """Orientation check, independent of spps_rows._gap_s's own control flow: for a box room,
    reconstruct the six boundary faces the same way rooms_mod._Grid does (one call into it, read only
    its 'faces' -- 'ax', 'coord', 'sign') and confirm every one of them has both the source and every
    receiver strictly in front (sign * (coord - point[ax]) > 0), the condition HARNESS-PLAN.md 8.2's
    amendment states. If the sign convention were inverted, every wall would fail this for an interior
    point and test_box_rooms_match_first_order_d1_to_1e12_relative above would see every candidate
    filtered out (an empty mirror set raises, per _gap_s's contract) rather than silently agree; this
    test pins the orientation directly, by count."""
    from m8b import rooms as rooms_mod

    checked_rooms = 0
    for name, room in R.items():
        if room['kind'] != 'box':
            continue
        (lo, hi), = room['boxes']
        grid = rooms_mod._Grid([(tuple(lo), tuple(hi), room['box_materials'][0])])
        assert len(grid.faces) == 6, (name, len(grid.faces))
        points = [room['source_m']] + [r['position_m'] for r in room['receivers']]
        for ax, coord, sign, _u, _v, _mat in grid.faces:
            for p in points:
                assert sign * (coord - p[ax]) > 0, (name, ax, coord, sign, p)
        checked_rooms += 1
    assert checked_rooms == 12                    # F1 F2 F5 F6 F7 P0 P0b and G1 G2 G5 G6 G7


# ---- (2) every receiver of every room: gap >= 0, and it is not boxes[0] alone ---------------------
def test_no_room_gives_a_negative_gap():
    """HARNESS-PLAN.md 8.2's amendment: with the front-side test, two points on the same side of every
    face they mirror across can never give an image closer than the direct path, so the gap is never
    negative. Checked on every receiver of every room (F1-F7, P0, P0b), not just the two that were
    negative before this amendment (F3 rec1, F4 rec7)."""
    checked = 0
    for name, room in R.items():
        for idx in range(len(room['receivers'])):
            g = spps_rows._gap_s(room, idx)
            assert g >= -1e-12, (name, idx, g)
            checked += 1
    assert checked == len(ALL_ROOMS) * 8


F3_NEAR_DOORWAY_IDX = 1          # HARNESS-PLAN.md 2.2: (2.0, 5.0, 1.4), main room, close to the doorway
F4_SECOND_LEG_IDX = 7            # (13.5, 12.0, 1.2): rooms.py marks it 'blocked', in the L's second leg


def test_f3_f4_receivers_need_a_box_other_than_boxes0():
    """(2) F3's receiver 1 and F4's receiver 7 are the two that `ff85de8`'s unfiltered rule gave a
    negative gap (reproduced below with _single_box_gap, the old boxes[0]-alone formula, which is
    itself one example of an unfiltered -- and here wrong -- computation): the fixed, front-filtered
    gap is non-negative and differs from boxes[0] alone's gap, so the function's answer is still not
    determined by boxes[0] alone (the defect audit of 03470f9 first caught)."""
    for name, idx in (('F3', F3_NEAR_DOORWAY_IDX), ('F4', F4_SECOND_LEG_IDX)):
        room = R[name]
        assert room['kind'] == 'non-box', name
        assert len(room['boxes']) > 1, '%s: this check needs a multi-box room' % name
        rec = room['receivers'][idx]['position_m']
        new = spps_rows._gap_s(room, idx)
        (lo0, hi0) = room['boxes'][0]
        box0_only = _single_box_gap(lo0, hi0, room['source_m'], rec)
        assert new >= -1e-12, (name, idx, new)
        assert abs(new - box0_only) > 1e-6, (
            '%s receiver %d: the new gap (%.6g s) must differ from boxes[0] alone\'s gap (%.6g s), or '
            'the function is still reading only the first box' % (name, idx, new, box0_only))


def _unfiltered_multi_box_gap(room, idx):
    """`ff85de8`'s rule, reproduced directly (never as a call into spps_rows._gap_s, which no longer
    has this behaviour): mirror across every boundary face of rooms_mod._Grid, with no front-side
    test, and take the shortest image-to-receiver distance -- the version that gave F3 rec1 and F4
    rec7 a negative gap, which is what showed the front-side test was needed."""
    from m8b import rooms as rooms_mod

    boxes, box_materials = room['boxes'], room['box_materials']
    grid = rooms_mod._Grid([(tuple(b[0]), tuple(b[1]), m) for b, m in zip(boxes, box_materials)])
    src, rec = room['source_m'], room['receivers'][idx]['position_m']
    d = math.dist(src, rec)
    best = math.inf
    for ax, coord, _sign, _u, _v, _mat in grid.faces:
        img = list(src)
        img[ax] = 2 * coord - src[ax]
        best = min(best, math.dist(img, rec))
    return (best - d) / corpus.C_SPPS


def test_old_unfiltered_rule_was_negative_there():
    """The pre-amendment defect, reproduced directly: mirroring across every boundary face with no
    front-side test (`ff85de8`'s rule) gives F3 rec1 and F4 rec7 a negative gap -- physically
    impossible for a lower bound on a reflection's extra path length -- while the front-filtered
    function fixes both to non-negative values (test_f3_f4_receivers_need_a_box_other_than_boxes0
    above), confirming the amendment changed the answer where it had to."""
    for name, idx in (('F3', F3_NEAR_DOORWAY_IDX), ('F4', F4_SECOND_LEG_IDX)):
        room = R[name]
        old_unfiltered = _unfiltered_multi_box_gap(room, idx)
        new = spps_rows._gap_s(room, idx)
        assert old_unfiltered < 0, (name, idx, old_unfiltered)
        assert new >= -1e-12, (name, idx, new)
        assert new > old_unfiltered + 1e-6, (name, idx, new, old_unfiltered)


# ---- (3) geometry that cannot be read raises, never falls back to the first box -------------------
def test_unreadable_geometry_raises_and_never_falls_back_to_boxes0():
    good = R['F1']
    missing_boxes = {k: v for k, v in good.items() if k != 'boxes'}
    with pytest.raises(ValueError):
        spps_rows._gap_s(missing_boxes, 0)

    missing_source = {k: v for k, v in good.items() if k != 'source_m'}
    with pytest.raises(ValueError):
        spps_rows._gap_s(missing_source, 0)

    mismatched = dict(good)
    mismatched['box_materials'] = list(good['box_materials']) + ['extra']
    with pytest.raises(ValueError):
        spps_rows._gap_s(mismatched, 0)

    overlapping = dict(good)
    overlapping['boxes'] = [good['boxes'][0], good['boxes'][0]]
    overlapping['box_materials'] = [good['box_materials'][0], good['box_materials'][0]]
    with pytest.raises(ValueError):
        spps_rows._gap_s(overlapping, 0)

    no_boxes = dict(good)
    no_boxes['boxes'] = []
    no_boxes['box_materials'] = []
    with pytest.raises(ValueError):
        spps_rows._gap_s(no_boxes, 0)


# ---- (4) a blocked receiver: no gap computed, no split_borderline call, status left as assess() gave it
def _write_run(path, seed, report):
    path.mkdir(parents=True)
    (path / 'project.simpa').write_text(json.dumps({'solvers': {'spps': {'random_seed': seed}}}), encoding='utf-8')
    (path / 'report.json').write_text(json.dumps(report), encoding='utf-8')


def _report(n_receivers, bins):
    return {
        'solver_build': {'status': 'verified'},
        'spps': {
            'speed_of_sound_m_s': 343.2, 'receiver_radius_m': 0.31, 'time_step_s': 0.001,
            'point_receivers': [
                {'label': 'R%03d' % i, 'arrival_s': 0.02, 'bands': [{'freq_hz': 500, 'energy_pa2': bins}]}
                for i in range(n_receivers)
            ],
        },
    }


class _FakeMethod:
    def analyse(self, bins, dt, t_arrival, meta=None):
        return dict(status='ok', edt=1.1, edt_lo=1.1, edt_hi=1.1, reason='')


def _fake_two_receiver_room():
    return {
        'FakeRoom': dict(
            boxes=[[[0.0, 0.0, 0.0], [10.0, 8.0, 6.0]]], box_materials=['walls'],
            source_m=[2.0, 2.0, 2.0],
            receivers=[
                dict(position_m=[8.0, 2.0, 2.0], d_m=6.0, **{'class': 'mid'}, blocked=False),
                dict(position_m=[8.0, 6.0, 2.0], d_m=math.dist([2.0, 2.0, 2.0], [8.0, 6.0, 2.0]),
                     **{'class': 'mid'}, blocked=True),
            ],
            design_t60_s={500: 1.0},
        )
    }


def test_blocked_receiver_skips_gap_and_split_borderline(monkeypatch, tmp_path):
    """(3) HARNESS-PLAN.md 8.2, amended 12:14: a blocked receiver (rooms.rooms()'s 'blocked' -- its
    straight segment to the source leaves the room) has no direct sound to split from, so
    rows_from_runs must not call _gap_s or truth.split_borderline for its row, and must leave
    truth_status exactly as truth.assess() gave it (already computed with blocked=True) -- the status
    the harness already gives such a receiver (TRUTH_STATUSES, score.py:131-132; no separate 'blocked'
    status exists, so none is invented here). The unblocked receiver in the same call is unaffected:
    _gap_s and split_borderline are both still consulted for it."""
    bins = [1.0, 0.8, 0.6, 0.4, 0.2, 0.1, 0.05, 0.02]
    tested = tmp_path / 'tested'
    _write_run(tested, 1101, _report(2, bins))
    refs = []
    for i, s in enumerate((9001, 9002, 9003, 9004)):
        p = tmp_path / ('ref%d' % i)
        _write_run(p, s, _report(2, bins))
        refs.append(p)

    monkeypatch.setattr(spps_rows.method, 'load', lambda path=None: _FakeMethod())

    def fake_assess(refs_, dt, t_arr, h, blocked=False):
        return dict(truth=1.0, edts=[1.0] * len(refs_), u=0.0, share=0.0, status='ok')
    monkeypatch.setattr(spps_rows.truth, 'assess', fake_assess)

    split_calls = []
    monkeypatch.setattr(spps_rows.truth, 'split_borderline',
                        lambda *a, **k: split_calls.append(a) or False)

    gap_calls = []
    real_gap_s = spps_rows._gap_s

    def spy_gap_s(room_geom, idx):
        gap_calls.append(idx)
        return real_gap_s(room_geom, idx)
    monkeypatch.setattr(spps_rows, '_gap_s', spy_gap_s)

    rows = spps_rows.rows_from_runs('FakeRoom', tested, refs, mode='random', particles=150000, seed=1101,
                                    data_root=tmp_path, geometry=_fake_two_receiver_room()['FakeRoom'])
    assert len(rows) == 2
    assert gap_calls == [0], '_gap_s must be called only for the unblocked receiver (R000, index 0)'
    assert len(split_calls) == 1, 'split_borderline must be consulted only for the unblocked receiver'
    assert rows[0]['truth_status'] == 'ok'          # R000, unblocked: split_borderline said False -> stays ok
    assert rows[1]['truth_status'] == 'ok'          # R001, blocked: assess()'s own status, untouched
