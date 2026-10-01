"""HARNESS-PLAN.md 8.2's last call (audit of 03470f9, Jarvis 12:01): m8b/spps_rows.py:_gap_s used to
read room_geom['boxes'][0] alone for every room, so an F3 or F4 receiver (both 'non-box', multi-box
rooms, rooms.py:268) got its first-reflection gap off the main box's walls only, silently wrong
whenever the true shortest image-to-receiver path runs through a different box (the doorway, the
second leg). The fix mirrors the source across every boundary face of the room's own geometry
(rooms_mod._Grid's faces: the quads rooms.py meshes and obj_text writes) and takes the shortest
image-to-receiver distance over all of them.

The reference here is corpus.first_order_d1 (a module _gap_s does not call for non-box rooms) applied
by hand to one box at a time, never spps_rows's own old code (never reintroduced): a module under
test is never its own reference (conftest.py's rule).

No solver is run, nothing is read or written under a `heldout` folder, and rooms.rooms() is pure
geometry (no project, mesh or solver touched).
"""
import math

import pytest

from m8b import corpus, rooms, spps_rows

R = rooms.rooms()


def _single_box_gap(lo, hi, source_m, rec_m):
    """The old, box-only formula (corpus.first_order_d1 in that box's local frame), applied to one
    box's six walls in isolation -- what m8b/spps_rows.py:_gap_s computed from room_geom['boxes'][0]
    before this fix, and what 'the gap from any single box's walls' means in HARNESS-PLAN.md 8.2's
    last call."""
    dims = [h - l for l, h in zip(lo, hi)]
    src = [s - l for s, l in zip(source_m, lo)]
    rec = [r - l for r, l in zip(rec_m, lo)]
    d = math.dist(src, rec)
    d1 = corpus.first_order_d1(src, rec, dims)
    return (d1 - d) / corpus.C_SPPS


# ---- (1) box rooms: the new gap equals the old box formula exactly -------------------------------
def test_box_rooms_match_first_order_d1_to_1e12_relative():
    """Every room rooms.rooms() marks 'box' (a single box, lo at the origin in every F/P plan) has
    exactly six boundary faces: its own six walls. Mirroring across _Grid's faces must then give the
    same six images corpus.first_order_d1 computes directly, so the two gaps should agree to float
    rounding, not merely to some workable tolerance."""
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
    assert checked == 7 * 8, 'expected 7 box rooms (F1 F2 F5 F6 F7 P0 P0b) x 8 receivers'


# ---- (2) F3 and F4: a receiver whose true gap needs a box other than boxes[0] --------------------
# HARNESS-PLAN.md 2.2: F3's boxes are (main, doorway, chamber); its receiver 1 (2.0, 5.0, 1.4) sits in
# the main room close to the doorway, so its true shortest image path runs through the doorway's own
# reveal walls (the extra planes the doorway box adds), not through the main room's own six walls.
# F4's receiver 7 (13.5, 12.0, 1.2) is the one rooms.py marks 'blocked', sitting in the L's second leg
# (boxes[1]), behind the corner from boxes[0]'s own walls.
F3_NEAR_DOORWAY_IDX = 1
F4_SECOND_LEG_IDX = 7


def test_f3_f4_receivers_need_a_box_other_than_boxes0():
    """(2) For a receiver whose true shortest reflection runs through a box other than boxes[0]:
    - the new gap is <= the gap computed from any single box's walls alone (a lower bound can only
      shrink, never grow, as more boundary planes become candidates for the nearest image);
    - the new gap differs from room_geom['boxes'][0]'s own single-box gap, i.e. the function's answer
      is not determined by boxes[0] alone -- which is the defect this test guards against (audit of
      03470f9)."""
    for name, idx in (('F3', F3_NEAR_DOORWAY_IDX), ('F4', F4_SECOND_LEG_IDX)):
        room = R[name]
        assert room['kind'] == 'non-box', name
        rec = room['receivers'][idx]['position_m']
        new = spps_rows._gap_s(room, idx)
        per_box = [_single_box_gap(lo, hi, room['source_m'], rec) for lo, hi in room['boxes']]
        box0_only = per_box[0]
        assert len(room['boxes']) > 1, '%s: this check needs a multi-box room' % name
        for k, g in enumerate(per_box):
            assert new <= g + 1e-9, (
                '%s receiver %d: the full-geometry gap %.6g must be <= box %d alone\'s gap %.6g'
                % (name, idx, new, k, g))
        assert abs(new - box0_only) > 1e-6, (
            '%s receiver %d: the new gap (%.6g s) must differ from boxes[0] alone\'s gap (%.6g s), or '
            'the function is still reading only the first box' % (name, idx, new, box0_only))


def test_old_boxes0_only_code_disagrees_with_the_fixed_function():
    """The defect, reproduced directly (never as a call into spps_rows._gap_s, which no longer has
    this behaviour): the formula audit 03470f9 used -- room_geom['boxes'][0] alone, nothing else --
    gives F3 and F4's receivers above a different, larger (worse) gap than the fixed function, because
    it never sees the doorway's or the second leg's own boundary faces."""
    for name, idx in (('F3', F3_NEAR_DOORWAY_IDX), ('F4', F4_SECOND_LEG_IDX)):
        room = R[name]
        rec = room['receivers'][idx]['position_m']
        (lo0, hi0) = room['boxes'][0]
        old_boxes0_only = _single_box_gap(lo0, hi0, room['source_m'], rec)
        new = spps_rows._gap_s(room, idx)
        assert old_boxes0_only > new + 1e-6, (name, idx, old_boxes0_only, new)


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
