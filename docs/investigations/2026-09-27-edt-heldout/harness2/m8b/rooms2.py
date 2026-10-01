"""SPPS-fresh-2's rooms G1-G7 (HARNESS-PLAN-2.md section 2, with section 9 M1 and M2): the seven projects.

Geometry, materials, source and receivers are the plan's table, typed in _PLAN; the build is rooms.py's own
(rooms._build, rooms._Grid, rooms.obj_text, rooms_mod.write_projects(..., room_dict=rooms()), rooms.mesh_projects),
so a G room is made, meshed and checked exactly as an F room was. rooms.py itself is round 1's file; its one
edit is that _build reads a `coupled` flag from the spec in place of `name == 'F3'`.

Contract:
- NAMES: ('G1', ..., 'G7').
- rooms() -> {name: room}, a room as rooms.rooms()'s (kind, boxes, volume, surface, bbox, materials, source,
  8 receivers with their class and blocked flag, design_t60_s by P5, role). G3 is coupled: its design_t60_s
  is the chamber's with the doorway at alpha 1 (the late slope), design_t60_early_s the main room's.
- check_rooms() -> {name: dict(min_clearance_m)}: every point at least 0.6 m from every surface, 8
  receivers per room, near and mid receivers everywhere, far ones except in G1 and G7, a blocked one in G3
  and in G4 (and nowhere else), as the plan's table says. Raises ValueError.
- geometry_sha256() -> str: sha256 of the canonical JSON of rooms() (geometry, materials, source, receivers,
  design T60s): what preview_pin.json records (section 9 M3).
- write_projects(out_dir, simpa_exe, names=None) and mesh_projects(...): rooms.py's, for these rooms.

What the plan fixes and this file keeps to the letter:
- G2 (section 9 M1): box 28 x 12 x 7 m at alpha 0.10, absorption only, geometry kept (so P3 still clears at
  0.246): design T60 2.90 / 2.79 / 2.71 / 1.95 s at 125 / 500 / 1 k / 4 k Hz. 1 kHz sits at 2.71 s, inside
  [2.6, 2.9] and clear of H4's edge at 3.0 s; every band is under 3.0 s. P17's run length is 5.9 s
  (0.07 s of arrival + 2 x 2.896 s, rounded up to 0.1 s), 59,000 steps at 0.1 ms, under the solver's 65,536.
- M2: every G room is scored whatever its truth shows. A feature that misses on the truth runs (G2 T30 under
  2.5 s at 1 kHz, G7 T30 over 0.25 s, G3 not double-sloped) is reported as missing, never repaired; no room is
  swapped, redesigned or dropped once any truth run exists.
- G3's doorway is y 2.4-3.9 m, z 0-2.2 m in a 0.2 m partition at x 7.5-7.7 m; the main room's faces, its
  partition face included, are alpha 0.35, the chamber's, the reveals' and its partition face's 0.06.
"""
import hashlib
import json

from . import rooms as rooms_mod

NAMES = ('G1', 'G2', 'G3', 'G4', 'G5', 'G6', 'G7')

_DOOR = dict(y0=2.4, y1=3.9, z0=0.0, z1=2.2)         # G3's doorway; the partition is x 7.5 to 7.7 m
# boxes: (lo, hi, the material of the faces bounding that box's air); materials: (alpha, scattering)
_PLAN = {
    'G1': dict(boxes=[((0.0, 0.0, 0.0), (4.2, 2.5, 2.2), 'walls')], materials={'walls': (0.20, 1.0)},
               role='small room (V <= 30 m3)', source=(0.7, 1.25, 1.1),
               receivers=[(1.5, 1.5, 1.0), (1.9, 0.9, 1.4), (1.4, 1.9, 0.8), (2.4, 1.2, 1.2),
                          (3.0, 1.6, 1.4), (3.4, 1.0, 1.0), (3.6, 1.9, 1.6), (2.9, 1.4, 0.8)]),
    'G2': dict(boxes=[((0.0, 0.0, 0.0), (28.0, 12.0, 7.0), 'walls')], materials={'walls': (0.10, 1.0)},
               role='T60 >= 2.5 s at 1 kHz; the longest T60 (alpha 0.10: design T60 2.71 s at 1 kHz, inside H4)',
               source=(3.5, 4.0, 1.6),
               receivers=[(4.7, 4.7, 1.3), (3.9, 5.6, 1.7), (2.6, 3.0, 1.2), (8.0, 6.0, 1.4),
                          (13.0, 3.0, 2.2), (9.0, 10.0, 1.5), (20.0, 8.0, 1.4), (26.5, 10.5, 3.0)]),
    'G3': dict(boxes=[((0.0, 0.0, 0.0), (7.5, 6.0, 3.5), 'main'),
                      ((7.5, _DOOR['y0'], _DOOR['z0']), (7.7, _DOOR['y1'], _DOOR['z1']), 'chamber'),
                      ((7.7, 0.0, 0.0), (17.7, 6.0, 3.5), 'chamber')],
               materials={'main': (0.35, 1.0), 'chamber': (0.06, 1.0)},
               coupled=True, doorway_area_m2=(_DOOR['y1'] - _DOOR['y0']) * (_DOOR['z1'] - _DOOR['z0']),
               role='non-uniform absorption, double slope, coupled; blocked receiver', source=(2.0, 3.2, 1.5),
               receivers=[(3.0, 3.8, 1.2), (1.2, 4.4, 1.7), (3.1, 2.0, 1.4), (5.0, 4.5, 1.3),
                          (6.4, 1.5, 1.8), (4.2, 5.2, 2.2), (15.0, 3.4, 1.2), (13.0, 0.9, 1.3)]),
    'G4': dict(boxes=[((0.0, 0.0, 0.0), (14.0, 4.0, 3.0), 'walls'), ((10.0, 4.0, 0.0), (14.0, 12.0, 3.0), 'walls')],
               materials={'walls': (0.18, 1.0)}, role='L-shaped; blocked receiver', source=(1.8, 2.0, 1.4),
               receivers=[(2.9, 2.7, 1.2), (1.0, 3.0, 1.5), (2.6, 1.0, 1.6), (5.5, 2.5, 1.4),
                          (8.0, 1.5, 1.7), (10.5, 3.0, 1.2), (13.0, 1.5, 1.3), (12.5, 10.5, 1.2)]),
    'G5': dict(boxes=[((0.0, 0.0, 0.0), (18.5, 10.5, 3.0), 'walls')], materials={'walls': (0.20, 0.1)},
               role='specular low hall (scattering 0.1)', source=(3.0, 5.25, 1.5),
               receivers=[(4.2, 5.7, 1.2), (2.2, 4.2, 1.7), (3.6, 6.8, 1.8), (7.5, 4.0, 1.3),
                          (9.0, 7.5, 1.8), (6.5, 9.0, 2.0), (14.0, 8.5, 1.3), (15.0, 2.0, 1.6)]),
    'G6': dict(boxes=[((0.0, 0.0, 0.0), (32.0, 6.5, 2.5), 'walls')], materials={'walls': (0.35, 1.0)},
               role='long, absorbent room; the 50k runs', source=(2.0, 3.25, 1.5),
               receivers=[(3.2, 3.8, 1.2), (2.7, 2.2, 1.6), (1.0, 4.6, 1.3), (6.0, 2.5, 1.4),
                          (9.5, 4.5, 1.8), (11.0, 1.5, 1.2), (16.0, 3.0, 1.3), (29.5, 5.0, 1.6)]),
    'G7': dict(boxes=[((0.0, 0.0, 0.0), (9.0, 7.5, 2.5), 'walls')], materials={'walls': (0.50, 1.0)},
               role='dead room: design T60 at 1 kHz 0.179 s, at most 0.25 s', source=(2.2, 2.4, 1.3),
               receivers=[(3.2, 3.0, 1.2), (2.6, 1.2, 1.5), (1.1, 3.6, 1.4), (5.0, 3.5, 1.4),
                          (7.0, 2.0, 1.7), (6.5, 6.2, 1.2), (8.0, 6.8, 1.6), (4.5, 5.5, 1.0)]),
}
_FAR = ('G2', 'G3', 'G4', 'G5', 'G6')
_BLOCKED = ('G3', 'G4')


def _with_grids():
    return {name: rooms_mod._build(name, spec, (spec['source'], spec['receivers'])) for name, spec in _PLAN.items()}


def rooms():
    """Every G room, as rooms.rooms() makes an F room."""
    out = {}
    for name, room in _with_grids().items():
        room = dict(room)
        del room['_grid']
        out[name] = room
    return out


def check_rooms():
    out = {}
    for name, room in _with_grids().items():
        grid = room['_grid']
        pts = [room['source_m']] + [r['position_m'] for r in room['receivers']]
        cl = [grid.clearance(p) for p in pts]
        if min(cl) < rooms_mod.CLEARANCE_M - 1e-9:
            raise ValueError('%s: a point is %.4f m from a surface, under %.3f m' % (name, min(cl), rooms_mod.CLEARANCE_M))
        if len(room['receivers']) != 8:
            raise ValueError('%s: %d receivers' % (name, len(room['receivers'])))
        classes = {r['class'] for r in room['receivers']}
        if not {'near', 'mid'} <= classes or ('far' in classes) != (name in _FAR):
            raise ValueError('%s: receiver classes %s' % (name, sorted(classes)))
        blocked = [r['name'] for r in room['receivers'] if r['blocked']]
        if bool(blocked) != (name in _BLOCKED):
            raise ValueError('%s: blocked receivers %s' % (name, blocked))
        out[name] = dict(min_clearance_m=min(cl))
    return out


def geometry_sha256():
    canon = json.dumps(rooms(), sort_keys=True, separators=(',', ':'))
    return hashlib.sha256(canon.encode('utf-8')).hexdigest()


def write_projects(out_dir, simpa_exe, names=None):
    return rooms_mod.write_projects(out_dir, simpa_exe, names=list(NAMES if names is None else names), room_dict=rooms())


mesh_projects = rooms_mod.mesh_projects
