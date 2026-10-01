"""SPPS-fresh rows (HARNESS-PLAN.md P13-P19, P27; step 7b wires 8.2's third call into this builder,
which did not exist before step 7b: the driver's matrix execution and row assembly are future work,
per section 8.1, so this module is what that future caller is expected to use).

Turns one tested SPPS run's report, together with its K = 4 reference runs' reports (both read
through driver.read_run / driver.series_from_report, the `simpa results` schema), and the room's own
known geometry (rooms.rooms()), into score.evaluate's 'spps' input dicts:
- truth and truth_status come from truth.assess (P15-P19, P27) on the K references, summed bin by
  bin, exactly as P16-P18 state it: truth = the corpus truth function (truth.truth_ideal) on the
  summed references, u = spread / sqrt(K) (P19), truth_truncated when the summed series' last-tenth
  share exceeds 1e-6 (P18), truth_uncertain when u exceeds 1 % and the row is otherwise unresolved.
- HARNESS-PLAN.md 8.2's third call is then applied, once the frozen method's own edt on the tested
  run is known: truth.split_borderline(edt, truth, gap_s, h, band_hz), gap_s the room's own first
  reflection's geometric gap after the direct sound (_gap_s below: equals corpus.first_order_d1 in a
  box room, generalised to every room per 8.2's last call). A row whose truth_status assess() gave 'ok' and whose
  split is borderline is relabelled 'truth_split_borderline' before it is returned; every other
  truth_status (truth_nan, truth_truncated, truth_uncertain) is left exactly as assess() gave it, as
  P27's existing three exclusions already are. This relabelling happens inside rows_from_runs itself,
  for every row it returns: no caller of this function can get a row whose truth_status is 'ok' while
  its own split was borderline.

Contract:
- rows_from_runs(room_name, tested_dir, reference_dirs, *, mode, particles, seed, data_root=None,
  method_path=None) -> [dict, ...]: one row per (point receiver, band) of the tested run's report
  (driver.series_from_report's own grouping). reference_dirs must be exactly 4 run folders (P16);
  each is read once (driver.read_run), then every row's references are the four reports' series at
  its own (label, band_hz) pair. A receiver-band missing from any of the four references is a
  ValueError: P16's K = 4 is not optional.
- Each row: 'set'='spps', 'id' ('spps|<room>|<label>|<band>|<step>ms|seed<seed>'), 'bins' (the tested
  run's energy_pa2), 'dt', 't_arrival' (the tested run's arrival_s), 'meta'={'half_width'}, 'truth'
  (assess()'s, float or NaN), 'truth_status', 'room', 'd_m' (the room's own geometry, not the report:
  rooms.rooms()[room_name]'s receiver distance), 'step_ms' (dt in ms, so score.py's own dt/step_ms
  check always holds), 'band_hz', 'design_t60_s' (the room's own, P5/P30), 'mode', 'particles', 'seed'.
- The room's geometry comes from rooms.rooms()[room_name] (every room, box or non-box: 'boxes' and
  'box_materials' give the boundary faces _gap_s mirrors across). A receiver's index is read from its
  report label ('R000' -> 0, as rooms.py's _build names them), and its 'blocked' flag (rooms.rooms())
  is passed to truth.assess unchanged, so a blocked receiver's truth reads the critique's occluded
  definition (truth.split).

Where the contract is silent:
- gap_s (the first reflection's gap after the direct sound) is geometric, from the room's own
  geometry and corpus.C_SPPS (343.2 m/s, frozen/method.py's own constant): the harness's fresh sets
  all compute it the same way, not from the measured bins, so it does not depend on which run (tested
  or any reference) is asked. HARNESS-PLAN.md 8.2's last call (audit of 03470f9): the gap is a lower
  bound taken the same way in every room, not only boxes -- mirror the source in the plane of every
  boundary face of the room's own geometry (rooms_mod._Grid's faces: the quads rooms.py meshes,
  obj_text writes and the solver sees), take the shortest image-to-receiver distance, subtract the
  direct distance, divide by C. *Amended 12:14 (HARNESS-PLAN.md 8.2, audit of `ff85de8`, which gave
  negative gaps: F3 rec1 -0.55 ms, F4 rec7 -9.92 ms)*: a face counts only when the source and the
  receiver both lie strictly in front of it (the side its inward normal points to, i.e. the side the
  owning box's air is on). Mirroring across a face neither point actually fronts is not a real
  reflection off that surface (both ends would have to pass back through the solid the face bounds to
  reach it), and can give an image closer than the direct path, which is how the gap went negative.
  With front-side filtering the gap is never negative (tests/test_gap_all_rooms.py checks every
  receiver of every room). In a box room every one of the six walls fronts every interior point (the
  room's own 0.6 m clearance, rooms.py's CLEARANCE_M), so no face is ever filtered out there and the
  result still equals corpus.first_order_d1 exactly (tests/test_gap_all_rooms.py pins the
  equivalence). F3 and F4 are multi-box ('non-box' kind, rooms.py:268): a receiver in F3's chamber or
  behind F4's corner no longer gets the main box's gap, and a face on the far side of a partition from
  the source (e.g. the chamber's own near wall, when the source is in the main room) is excluded
  rather than used to mirror a path that was never physically possible.
- split_borderline is evaluated only when both assess()'s truth_status is 'ok' and the tested run's
  own frozen-method analysis (method.load(method_path).analyse) is itself status 'ok' (a finite edt):
  a row that is not ok has no wrong-silent verdict to protect (score.classify: wrong_silent is False,
  never computed, for a row that is not ok), so 8.2's third call has nothing to flag there. A blocked
  receiver (rooms.rooms()'s 'blocked': its straight segment to the source leaves the room) has no
  direct sound to split from, so 8.2's third call is not applied to it either, even when assess()'s
  own status (already computed with blocked=True, the critique's occluded truth) comes back 'ok': that
  status is what the harness already gives such a receiver, and rows_from_runs leaves it exactly as
  assess() gave it, the same way it already leaves truth_nan/truth_truncated/truth_uncertain rows
  alone. _gap_s itself is not called for a blocked receiver's row.
"""
import math

from . import corpus, driver, method, rooms as rooms_mod, truth

K_REFS = 4                              # P16


def _gap_s(room_geom, idx):
    """HARNESS-PLAN.md 8.2's last call (*amended 12:14*): the first-reflection gap, as a lower bound
    taken the same way in every room, not only boxes (audit of 03470f9, which read 'boxes'[0] alone,
    silently wrong for F3 and F4's chamber/corner receivers). Mirror the source in the plane of every
    boundary face of the room's own geometry (rooms_mod._Grid's faces, built from this room's 'boxes'
    and 'box_materials' exactly as rooms.py meshes and obj_text writes them -- the geometry the solver
    sees) that the source and the receiver are BOTH strictly in front of (on the side the face's
    inward normal points to, where the owning box's air is: a _Grid face's 'sign' is +1 when the air
    is on the lower side of its coordinate, -1 when it is on the higher side, so the inward normal's
    component on that axis is -sign; a point is in front when -sign * (point[ax] - coord) > 0, i.e.
    sign * (coord - point[ax]) > 0). A face neither point fronts is not a reflection either of them
    can actually make off it (audit of `ff85de8`, which mirrored across every face unconditionally and
    gave negative gaps: F3 rec1 -0.55 ms, F4 rec7 -9.92 ms, both from mirroring across a partition face
    on the far side from the source). Take the shortest image-to-receiver distance over the faces that
    qualify, subtract the direct distance, divide by C. In a box room every point is more than 0 m
    (rooms.py's 0.6 m clearance) inside every one of the box's six walls, so every wall fronts every
    source and receiver there and none is ever filtered out: the result still equals
    corpus.first_order_d1 exactly. Raises ValueError if the room's geometry cannot be read into
    boundary faces, or if no face fronts both the source and this receiver; never falls back to the
    first box. 'box_materials' only labels which box a face bounds (rooms.py:270) and does not affect
    which faces are boundary faces, so a room dict that omits it (as rooms.rooms() never does, but a
    hand-built one may) gets placeholder per-box labels instead of being treated as unreadable; a
    missing 'boxes', 'source_m' or receiver position is."""
    try:
        boxes = room_geom['boxes']
        src = room_geom['source_m']
        rec = room_geom['receivers'][idx]['position_m']
    except (KeyError, IndexError, TypeError) as exc:
        raise ValueError('room %r: geometry cannot be read: %r' % (room_geom.get('name'), exc)) from exc
    box_materials = room_geom.get('box_materials')
    if box_materials is None:
        box_materials = ['_box%d' % i for i in range(len(boxes))]
    if not boxes or len(boxes) != len(box_materials):
        raise ValueError('room %r: %d boxes, %d box_materials' % (room_geom.get('name'), len(boxes),
                                                                   len(box_materials)))
    try:
        grid = rooms_mod._Grid([(tuple(b[0]), tuple(b[1]), m) for b, m in zip(boxes, box_materials)])
    except (KeyError, IndexError, TypeError, ValueError) as exc:
        raise ValueError('room %r: geometry cannot be read: %r' % (room_geom.get('name'), exc)) from exc
    if not grid.faces:
        raise ValueError('room %r: geometry has no boundary faces' % room_geom.get('name'))
    d = math.dist(src, rec)
    best = math.inf
    for ax, coord, sign, _u, _v, _mat in grid.faces:
        if sign * (coord - src[ax]) <= 0 or sign * (coord - rec[ax]) <= 0:
            continue                    # neither the source nor the receiver can reflect off this face
        img = list(src)
        img[ax] = 2 * coord - src[ax]
        best = min(best, math.dist(img, rec))
    if not math.isfinite(best):
        raise ValueError('room %r, receiver %d: no boundary face fronts both the source and the '
                         'receiver' % (room_geom.get('name'), idx))
    return (best - d) / corpus.C_SPPS


def _receiver_index(label):
    i = label.lstrip('Rr')
    if not i.isdigit():
        raise ValueError('receiver label %r is not rooms.py\'s "R%%03d" % i form' % (label,))
    return int(i)


def rows_from_runs(room_name, tested_dir, reference_dirs, *, mode, particles, seed, data_root=None,
                   method_path=None):
    reference_dirs = list(reference_dirs)
    if len(reference_dirs) != K_REFS:
        raise ValueError('P16: exactly %d references, not %d' % (K_REFS, len(reference_dirs)))
    kw = {} if data_root is None else dict(data_root=data_root)
    tested = driver.read_run(tested_dir, **kw)
    refs = [driver.read_run(p, **kw) for p in reference_dirs]
    geom = rooms_mod.rooms()[room_name]
    m = method.load(method_path)
    out = []
    for t in tested:
        key = (t['label'], t['band_hz'])
        matched = [next((s for s in r if (s['label'], s['band_hz']) == key), None) for r in refs]
        if any(s is None for s in matched):
            raise ValueError('room %s, %s at %g Hz: not every one of the %d references carries this '
                             'receiver-band' % (room_name, t['label'], t['band_hz'], K_REFS))
        idx = _receiver_index(t['label'])
        rec_geom = geom['receivers'][idx]
        ref0 = matched[0]
        a = truth.assess([s['energy_pa2'] for s in matched], ref0['dt'], ref0['arrival_s'], ref0['half_width'],
                         blocked=rec_geom['blocked'])
        truth_status = a['status']
        # HARNESS-PLAN.md 8.2's last call, amended 12:14: a blocked receiver has no direct sound to
        # split from, so the third call is not applied to it; its truth_status stays exactly what
        # assess() gave (already computed with blocked=True above), the status the harness already
        # gives such a receiver, and _gap_s is not called for it.
        if truth_status == 'ok' and not rec_geom['blocked']:
            res = m.analyse(t['energy_pa2'], t['dt'], t['arrival_s'], dict(half_width=t['half_width']))
            if res['status'] == 'ok':
                gap_s = _gap_s(geom, idx)
                # HARNESS-PLAN.md 8.2's third call, wired: every otherwise-'ok', unblocked row is
                # tested, not just the ones a caller remembers to check.
                if truth.split_borderline(res['edt'], a['truth'], gap_s, t['half_width'], t['band_hz']):
                    truth_status = 'truth_split_borderline'
        t60 = geom['design_t60_s']
        out.append(dict(set='spps', id='spps|%s|%s|%g|%gms|seed%d' % (room_name, t['label'], t['band_hz'],
                                                                      t['dt'] * 1e3, seed),
                        bins=t['energy_pa2'], dt=t['dt'], t_arrival=t['arrival_s'],
                        meta=dict(half_width=t['half_width']), truth=a['truth'], truth_status=truth_status,
                        room=room_name, d_m=rec_geom['d_m'], step_ms=t['dt'] * 1e3, band_hz=t['band_hz'],
                        design_t60_s=t60.get(t['band_hz'], t60.get(str(t['band_hz']))),
                        mode=mode, particles=particles, seed=seed))
    return out
