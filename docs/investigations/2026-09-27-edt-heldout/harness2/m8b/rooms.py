"""SPPS-fresh's rooms F1-F7 and the probes' stand-ins P0 and P0b: the nine projects (HARNESS-PLAN.md
2.2, P3, P5, P7, section 7).

Built in HARNESS-PLAN.md section 6, step 4. Tests T10 and T11 hold the contract below.

Contract:
- rooms() -> {name: room} for NAMES + PROBES. A room is a dict:
    name; kind 'box' or 'non-box';
    boxes: [[[x0, y0, z0], [x1, y1, z1]], ...], the air volume as a union of disjoint axis-aligned
      boxes on one global grid (P7). F3 is three boxes: the main room [0,0,0]-[9,7,4], the doorway
      through the 0.2 m partition ([9, y0, z0]-[9.2, y0 + 2.0, z0 + 2.5]) and the chamber
      [9.2,0,0]-[15.2,7,4]. F4 is two: [0,0,0]-[16,5,3.5] and [11,5,0]-[16,15,3.5];
    volume_m3; surface_m2 (the union's boundary); bbox_m [[lo], [hi]];
    materials: {name: {'absorption': a, 'scattering': s}}, the same in every band, Lambert law;
    material_area_m2: {name: boundary area it covers};
    source_m: [x, y, z];
    receivers: [{'name', 'position_m', 'd_m' (straight line to the source), 'class' ('near' when
      d < 2 m, 'far' when d > 10 m, else 'mid'), 'blocked' (the straight segment to the source
      leaves the room)}], 8 per room, in 2.2's order;
    design_t60_s: {band_hz: P5's Eyring T60 with ISO 9613-1 air} for BANDS_HZ. F3's is its
      chamber's, with the doorway counted at alpha 1 (the late slope);
    role: the row of 2.2 the room fills.
  F7's source and receivers are F1's scaled per axis to F7's box, except the receiver that scaling
  puts 0.576 m under the ceiling, which is lowered to z = 1.80 m (2.2, 8.1). P0 and P0b are boxes: P0 has
  F2's V, S and alpha within 1 %, every axis more than 10 % off F2's, and F2's source and
  receivers scaled per axis; P0b stands to F7 the same way.
- near_duplicate(a, b) -> bool: P3's rule, for two rooms of this form or of corpus_rooms.json's.
- obj_text(room) -> str: the room as OBJ (P7): one global grid, watertight and conforming, one
  `usemtl` per material, metres, z up.
- write_projects(out_dir, simpa_exe) -> {name: path of <name>.simpa}: each room's OBJ through
  `simpa import <obj> <out> --unit m --up z` (geometry/import.rs), then canonical JSON edits
  (tools/fixture-gen/mkrooms.py:13-18): the materials, the source, the 8 point receivers, and
  P8-P9's settings, which are a freshly imported project's except random_seed non-zero,
  save_surface_intersections and save_receiver_intersections false, and fittings false. Each
  file passes `simpa validate` (exit 0) and `simpa check`. Written under out_dir, never on B:.

Choices the plan leaves open, made here:
- F3's doorway is y 2.9-4.9 m, z 0-2.5 m: 2.2 lists the far receiver (13.0, 5.8, 1.3) as "blocked,
  grazing the doorway edge", and its segment to the source crosses the partition at y 4.92-4.97 m,
  just past an edge at 4.9. A face's material is that of the box whose air it bounds: the main room's
  faces, its partition face included, are 0.40; the chamber's, its partition face, the doorway's
  reveals, lintel and floor are 0.04 (249.0 m2 and 184.8 m2).
- P0b is 2.7 x 4.12 x 2.7 m (x, y, z): V 30.04 m3 and S 59.08 m2 against F7's 30.10 and 59.16
  (-0.2 %, -0.1 %), each axis 12.5-28.9 % off F7's. Its 2.7 m axes come from fixing z at F7's
  2.4 m + 12.5 % and solving x and y for F7's V and S (2.702 and 4.125 m), rounded.
- The receivers are named R000-R007 as the bed names them (bed/file.rs:700-718), the source keeps the
  prototype's name; every id is a uuid5 of the room and the item, so a project is the same on every
  run; no solver id is pinned (export assigns them, as for a drawn item).
- random_seed is PROJECT_SEED, 1, in every room project: a dev seed, neither held out nor reserved
  (P12). The driver's per-run project sets each run's seed, step, length, particles and method.

    python -m m8b.rooms --out C:\\tmp\\m8b-edt\\rooms [--simpa <simpa.exe>] [--solvers <dir>] [--no-mesh]
writes the nine projects, meshes each (`simpa mesh`, then `simpa mesh-verify`: TetGen only, no
solver run) and writes <out>/rooms.json with every room, its P3 clearance and each step's exit code.
"""
import copy
import decimal
import json
import math
import os
import subprocess
import sys
import uuid
from pathlib import Path

from . import corpus

NAMES = ('F1', 'F2', 'F3', 'F4', 'F5', 'F6', 'F7')
PROBES = ('P0', 'P0b')
BANDS_HZ = (125, 250, 500, 1000, 2000, 4000)

C = corpus.C_SPPS                       # P5: 343.2 m/s, frozen/method.py:20
CLEARANCE_M = 0.6                       # 2.2: every point at least 0.6 m from every surface
NEAR_M, FAR_M = 2.0, 10.0               # 2.2: near < 2 m, far > 10 m
P3_TOLERANCE = 0.10                     # P3: within 10 %
PROJECT_SEED = 1                        # P9: non-zero; a dev seed (P12 holds neither 1 nor any seed near it)
TEMPLATE = corpus.REPO / 'tests' / 'fixtures' / 'rooms' / 'tutorial1_box.simpa'
CORPUS_ROOMS = corpus.HARNESS / 'corpus_rooms.json'
ID_NAMESPACE = uuid.UUID('0c0be000-0000-4000-8000-00000008b0ed')   # uuid5 namespace of the M8b projects
DEFAULT_SIMPA = Path(r'C:\tmp\nm-target\release\simpa.exe')
DEFAULT_SOLVERS = Path(r'C:\tmp\nm-m8a-solvers')
DEFAULT_OUT = Path(r'C:\tmp\m8b-edt\rooms')

F7_LOWERED_Z = 1.80                     # 2.2, 8.1: F1's (2.8, 1.2, 1.9) scaled lands 0.576 m under F7's ceiling
DOORWAY = dict(y0=2.9, width=2.0, z0=0.0, height=2.5)          # F3, see the module docstring

# ---- HARNESS-PLAN.md 2.2 ------------------------------------------------------------------------------
# boxes: (lo, hi, the material of the faces bounding that box's air); materials: (alpha, scattering)
_D = DOORWAY
_PLAN = {
    'F1': dict(boxes=[((0.0, 0.0, 0.0), (3.4, 2.9, 2.5), 'walls')], materials={'walls': (0.15, 1.0)},
               role='small room (V <= 30 m3)',
               source=(0.8, 0.8, 1.3),
               receivers=[(1.6, 1.2, 1.2), (2.0, 0.9, 1.5), (1.3, 2.1, 1.1), (2.4, 1.6, 0.9),
                          (2.7, 2.2, 1.6), (2.8, 1.9, 1.2), (2.6, 2.3, 0.7), (2.8, 1.2, 1.9)]),
    'F2': dict(boxes=[((0.0, 0.0, 0.0), (17.0, 12.5, 8.0), 'walls')], materials={'walls': (0.08, 1.0)},
               role='T60 >= 2.5 s; longest T60',
               source=(3.0, 4.0, 1.6),
               receivers=[(4.2, 4.6, 1.3), (3.5, 5.6, 1.7), (2.2, 5.0, 1.2), (7.5, 6.0, 1.4),
                          (10.0, 3.0, 2.2), (6.0, 10.5, 1.5), (14.5, 9.0, 1.4), (16.0, 11.5, 3.0)]),
    'F3': dict(boxes=[((0.0, 0.0, 0.0), (9.0, 7.0, 4.0), 'main'),
                      ((9.0, _D['y0'], _D['z0']), (9.2, _D['y0'] + _D['width'], _D['z0'] + _D['height']), 'chamber'),
                      ((9.2, 0.0, 0.0), (15.2, 7.0, 4.0), 'chamber')],
               materials={'main': (0.40, 1.0), 'chamber': (0.04, 1.0)},
               coupled=True, doorway_area_m2=_D['width'] * _D['height'],
               role='non-uniform absorption, double slope, coupled',
               source=(2.5, 3.5, 1.5),
               receivers=[(3.8, 3.9, 1.2), (2.0, 5.0, 1.4), (3.2, 2.0, 1.7), (6.5, 2.0, 1.2),
                          (7.8, 5.5, 1.5), (5.5, 6.0, 2.2), (14.0, 3.6, 1.2), (13.0, 5.8, 1.3)]),
    'F4': dict(boxes=[((0.0, 0.0, 0.0), (16.0, 5.0, 3.5), 'walls'), ((11.0, 5.0, 0.0), (16.0, 15.0, 3.5), 'walls')],
               materials={'walls': (0.12, 1.0)},
               role='L-shaped; blocked receiver',
               source=(2.0, 2.5, 1.5),
               receivers=[(3.2, 3.0, 1.2), (2.8, 1.2, 1.6), (1.2, 3.8, 1.3), (6.0, 3.5, 1.4),
                          (9.0, 1.5, 1.7), (11.5, 4.0, 1.2), (14.5, 2.5, 1.3), (13.5, 12.0, 1.2)]),
    'F5': dict(boxes=[((0.0, 0.0, 0.0), (14.0, 9.5, 6.0), 'walls')], materials={'walls': (0.20, 0.2)},
               role='mostly specular, like real finishes',
               source=(3.0, 4.75, 1.6),
               receivers=[(4.3, 5.2, 1.3), (2.4, 3.2, 1.4), (3.5, 6.4, 1.8), (7.5, 3.0, 1.2),
                          (9.0, 7.5, 1.7), (6.0, 8.5, 2.5), (13.0, 8.5, 1.3), (13.2, 1.2, 1.5)]),
    'F6': dict(boxes=[((0.0, 0.0, 0.0), (24.0, 5.5, 3.2), 'walls')], materials={'walls': (0.30, 1.0)},
               role='long room at high absorption, where the noise model failed (noise-cal V4-E1/E4); the 50k runs',
               source=(2.0, 2.75, 1.5),
               receivers=[(3.3, 3.2, 1.2), (2.6, 1.3, 1.6), (1.0, 3.9, 1.3), (6.0, 2.0, 1.4),
                          (9.5, 4.5, 1.8), (11.0, 1.5, 1.2), (16.0, 3.0, 1.3), (22.5, 2.2, 1.6)]),
    # F7: F1's points scaled per axis (made in rooms()), its 8th receiver lowered to F7_LOWERED_Z
    'F7': dict(boxes=[((0.0, 0.0, 0.0), (3.8, 3.3, 2.4), 'walls')], materials={'walls': (0.45, 1.0)},
               role='small dead room, the kind of room where 13 of the method\'s 19 real-seed wrong-silent rows sit '
                    '(C-R3, 5 x 4 x 3 m, alpha 0.4)',
               scaled_from='F1', lowered={7: F7_LOWERED_Z}),
    # section 7: never scored; points scaled per axis from the room they stand in for
    'P0': dict(boxes=[((0.0, 0.0, 0.0), (18.9, 9.5, 9.46), 'walls')], materials={'walls': (0.08, 1.0)},
               role='the timing probe\'s stand-in for F2 (section 7); never scored', scaled_from='F2'),
    'P0b': dict(boxes=[((0.0, 0.0, 0.0), (2.7, 4.12, 2.7), 'walls')], materials={'walls': (0.45, 1.0)},
                role='the dead-room probe\'s stand-in for F7 (section 7.1); never scored', scaled_from='F7'),
}


# ---- P5 -----------------------------------------------------------------------------------------------
def eyring(volume, parts, band_hz):
    """P5: T = 24 ln10 V / (c (-S ln(1 - abar) + 4 m V)), c = 343.2 m/s, m from ISO 9613-1 at 20 C and
    50 % RH (corpus.m_energy, the text of ism.py's). parts: [(area m2, alpha)]."""
    S = sum(a for a, _ in parts)
    abar = sum(a * al for a, al in parts) / S
    return 24.0 * math.log(10) * volume / (C * (-S * math.log(1.0 - abar) + 4.0 * corpus.m_energy(band_hz) * volume))


def _box_area(lo, hi):
    w = [h - l for l, h in zip(lo, hi)]
    return 2.0 * (w[0] * w[1] + w[0] * w[2] + w[1] * w[2])


def _box_volume(lo, hi):
    return (hi[0] - lo[0]) * (hi[1] - lo[1]) * (hi[2] - lo[2])


# ---- a union of axis-aligned boxes on one global grid (P7) --------------------------------------------
class _Grid:
    """The cells of the grid that every box corner cuts, which box's air each cell is, and the boundary
    faces between air and the outside: (axis, coordinate, outward sign, (u0, u1), (v0, v1), material),
    where (u, v) are the axes after `axis` in cyclic order, so u x v points along +axis."""

    def __init__(self, boxes):
        self.boxes = boxes
        self.cuts = [sorted({float(b[s][ax]) for b in boxes for s in (0, 1)}) for ax in range(3)]
        self.shape = tuple(len(c) - 1 for c in self.cuts)
        self.owner = {}
        for idx in self._cells():
            centre = [0.5 * (self.cuts[a][idx[a]] + self.cuts[a][idx[a] + 1]) for a in range(3)]
            inside = [k for k, (lo, hi, _) in enumerate(boxes) if all(lo[a] < centre[a] < hi[a] for a in range(3))]
            if len(inside) > 1:
                raise ValueError('boxes overlap at %s' % centre)
            if inside:
                self.owner[idx] = inside[0]
        self.faces = []
        self.volume = 0.0
        for idx in self._cells():
            if idx not in self.owner:
                continue
            w = [self.cuts[a][idx[a] + 1] - self.cuts[a][idx[a]] for a in range(3)]
            self.volume += w[0] * w[1] * w[2]
            material = boxes[self.owner[idx]][2]
            for ax in range(3):
                u, v = (ax + 1) % 3, (ax + 2) % 3
                for sign in (-1, 1):
                    nb = list(idx)
                    nb[ax] += sign
                    if 0 <= nb[ax] < self.shape[ax] and tuple(nb) in self.owner:
                        continue
                    coord = self.cuts[ax][idx[ax] + (1 if sign > 0 else 0)]
                    self.faces.append((ax, coord, sign, (self.cuts[u][idx[u]], self.cuts[u][idx[u] + 1]),
                                       (self.cuts[v][idx[v]], self.cuts[v][idx[v] + 1]), material))
        self.surface = sum((u1 - u0) * (v1 - v0) for _, _, _, (u0, u1), (v0, v1), _ in self.faces)

    def _cells(self):
        for i in range(self.shape[0]):
            for j in range(self.shape[1]):
                for k in range(self.shape[2]):
                    yield (i, j, k)

    def area_by_material(self):
        out = {}
        for _, _, _, (u0, u1), (v0, v1), m in self.faces:
            out[m] = out.get(m, 0.0) + (u1 - u0) * (v1 - v0)
        return out

    def clearance(self, p):
        """The distance from p to the nearest boundary face."""
        best = math.inf
        for ax, coord, _, (u0, u1), (v0, v1), _ in self.faces:
            u, v = (ax + 1) % 3, (ax + 2) % 3
            du = max(u0 - p[u], 0.0, p[u] - u1)
            dv = max(v0 - p[v], 0.0, p[v] - v1)
            best = min(best, math.sqrt((p[ax] - coord) ** 2 + du * du + dv * dv))
        return best


def segment_inside(boxes, a, b, eps=1e-9):
    """Whether the straight segment a-b stays inside the union of the closed boxes: the parameter
    intervals of the segment inside each box (slab method), unioned, cover [0, 1]."""
    spans = []
    for lo, hi, *_ in boxes:
        t0, t1 = 0.0, 1.0
        for ax in range(3):
            d = b[ax] - a[ax]
            if abs(d) < 1e-15:
                if a[ax] < lo[ax] - eps or a[ax] > hi[ax] + eps:
                    t0, t1 = 1.0, 0.0
                    break
                continue
            s0, s1 = (lo[ax] - eps - a[ax]) / d, (hi[ax] + eps - a[ax]) / d
            t0, t1 = max(t0, min(s0, s1)), min(t1, max(s0, s1))
        if t0 <= t1:
            spans.append((t0, t1))
    reach = 0.0
    for t0, t1 in sorted(spans):
        if t0 > reach + 1e-12:
            return False
        reach = max(reach, t1)
    return reach >= 1.0 - 1e-12


def _klass(d):
    return 'near' if d < NEAR_M else ('far' if d > FAR_M else 'mid')


def _scale(p, f):
    return [p[i] * f[i] for i in range(3)]


# ---- the rooms ----------------------------------------------------------------------------------------
def _build(name, spec, points):
    boxes = [(tuple(map(float, lo)), tuple(map(float, hi)), m) for lo, hi, m in spec['boxes']]
    grid = _Grid(boxes)
    source, receivers = points
    lo = [min(b[0][a] for b in boxes) for a in range(3)]
    hi = [max(b[1][a] for b in boxes) for a in range(3)]
    mats = {m: {'absorption': a, 'scattering': s} for m, (a, s) in spec['materials'].items()}
    areas = grid.area_by_material()
    if set(areas) != set(mats):
        raise ValueError('%s: faces carry materials %s, the room lists %s' % (name, sorted(areas), sorted(mats)))
    recs = []
    for i, p in enumerate(receivers):
        d = math.dist(source, p)
        recs.append({'name': 'R%03d' % i, 'position_m': list(p), 'd_m': d, 'class': _klass(d),
                     'blocked': not segment_inside(boxes, source, p)})
    room = {
        'name': name,
        'kind': 'box' if len(boxes) == 1 else 'non-box',
        'boxes': [[list(b[0]), list(b[1])] for b in boxes],
        'box_materials': [b[2] for b in boxes],
        'volume_m3': grid.volume,
        'surface_m2': grid.surface,
        'bbox_m': [lo, hi],
        'materials': mats,
        'material_area_m2': {m: areas[m] for m in mats},
        'source_m': list(source),
        'receivers': recs,
        'role': spec['role'],
    }
    if spec.get('coupled'):
        # P5: the chamber, its doorway counted at alpha 1 (the late slope); the main room the same way (early)
        door = spec['doorway_area_m2']
        (mlo, mhi, _), _, (clo, chi, _) = boxes
        a_main, a_chamber = mats['main']['absorption'], mats['chamber']['absorption']
        room['design_t60_s'] = {f: eyring(_box_volume(clo, chi), [(_box_area(clo, chi) - door, a_chamber), (door, 1.0)], f)
                                for f in BANDS_HZ}
        room['design_t60_early_s'] = {f: eyring(_box_volume(mlo, mhi), [(_box_area(mlo, mhi) - door, a_main), (door, 1.0)], f)
                                      for f in BANDS_HZ}
        room['design_t60_rule'] = 'P5 on the chamber with the doorway at alpha 1 (late); design_t60_early_s: the main room the same way'
    else:
        (alpha, _), = spec['materials'].values()
        room['design_t60_s'] = {f: eyring(grid.volume, [(grid.surface, alpha)], f) for f in BANDS_HZ}
        room['design_t60_rule'] = 'P5 on the room'
    room['_grid'] = grid
    return room


def _rooms_with_grids():
    out = {}
    for name in NAMES + PROBES:
        spec = _PLAN[name]
        ref = spec.get('scaled_from')
        if ref is None:
            points = (spec['source'], spec['receivers'])
        else:
            R = out[ref]
            (lo, hi, _), = spec['boxes']
            f = [(hi[a] - lo[a]) / (R['bbox_m'][1][a] - R['bbox_m'][0][a]) for a in range(3)]
            recs = [_scale(r['position_m'], f) for r in R['receivers']]
            for i, z in spec.get('lowered', {}).items():
                recs[i][2] = z
            points = (_scale(R['source_m'], f), recs)
        out[name] = _build(name, spec, points)
    return out


def rooms():
    """{name: room} for NAMES + PROBES (see the module docstring)."""
    out = {}
    for name, room in _rooms_with_grids().items():
        room = dict(room)
        del room['_grid']
        out[name] = room
    return out


def check_rooms():
    """The checks 2.2 and section 7 state, on this module's own geometry: every point clears every
    surface by 0.6 m (1e-9 allowed; the probes' by 0.6 m times their smallest scale factor); 8
    receivers; near and mid receivers in every F room, far ones in F2-F6 (where the room allows), a
    blocked one in F3 and in F4. Raises ValueError; returns each room's clearance."""
    full = _rooms_with_grids()
    out = {}
    for name, room in full.items():
        grid = room['_grid']
        floor = CLEARANCE_M
        if name in PROBES:
            F = full[_PLAN[name]['scaled_from']]
            f = [(room['bbox_m'][1][a] - room['bbox_m'][0][a]) / (F['bbox_m'][1][a] - F['bbox_m'][0][a]) for a in range(3)]
            floor = CLEARANCE_M * min(1.0, *f)
        pts = [room['source_m']] + [r['position_m'] for r in room['receivers']]
        cl = [grid.clearance(p) for p in pts]
        if min(cl) < floor - 1e-9:
            raise ValueError('%s: a point is %.4f m from a surface, under %.3f m' % (name, min(cl), floor))
        if len(room['receivers']) != 8:
            raise ValueError('%s: %d receivers' % (name, len(room['receivers'])))
        classes = {r['class'] for r in room['receivers']}
        if name in NAMES and not ({'near', 'mid'} <= classes and ('far' in classes or name in ('F1', 'F7'))):
            raise ValueError('%s: receiver classes %s' % (name, sorted(classes)))
        if name in ('F3', 'F4') and not any(r['blocked'] for r in room['receivers']):
            raise ValueError('%s: no blocked receiver' % name)
        out[name] = dict(min_clearance_m=min(cl), clearance_floor_m=floor)
    return out


# ---- P3 -----------------------------------------------------------------------------------------------
def sorted_dims(room):
    if 'dims_sorted_m' in room:
        return [float(x) for x in room['dims_sorted_m']]
    lo, hi = room['bbox_m']
    return sorted((float(b) - float(a) for a, b in zip(lo, hi)), reverse=True)


def p3_gap(room, ref):
    """The largest relative difference |x / x_ref - 1| over what P3 compares (the sorted dimensions;
    for two non-box rooms also V and S), against ref; None when one is a box and the other is not,
    which P3 never calls near-duplicates."""
    if room['kind'] != ref['kind']:
        return None
    gaps = [abs(a / b - 1.0) for a, b in zip(sorted_dims(room), sorted_dims(ref))]
    if room['kind'] != 'box':
        gaps += [abs(room['volume_m3'] / ref['volume_m3'] - 1.0), abs(room['surface_m2'] / ref['surface_m2'] - 1.0)]
    return max(gaps)


def near_duplicate(a, b):
    """P3: two boxes are near-duplicates when each sorted dimension is within 10 %; two non-box rooms
    when their sorted bounding boxes, volumes and surface areas are each within 10 %; a box and a
    non-box room never are. b is the reference (a corpus room): 'within 10 %' is |a / b - 1| <= 0.10,
    as corpus_rooms.json's rule and the tests read it."""
    g = p3_gap(a, b)
    return g is not None and g <= P3_TOLERANCE


def p3_clearance(room, corpus_rooms=None):
    """How far a room is from being a near-duplicate of a corpus room: the smallest p3_gap over the
    corpus rooms of its kind, and that room's id. Fresh when the gap is over 0.10, or when no corpus
    room is of its kind (gap None)."""
    if corpus_rooms is None:
        corpus_rooms = json.loads(CORPUS_ROOMS.read_text(encoding='utf-8'))
    best = (None, None)
    for c in corpus_rooms['rooms']:
        g = p3_gap(room, c)
        if g is not None and (best[0] is None or g < best[0]):
            best = (g, c['id'])
    gap, nearest = best
    return dict(gap=gap, nearest=nearest, fresh=gap is None or gap > P3_TOLERANCE,
                same_kind=sum(1 for c in corpus_rooms['rooms'] if c['kind'] == room['kind']))


# ---- P7: OBJ ------------------------------------------------------------------------------------------
def _num(x):
    """A float as serde_json writes it (ryu's format64): the shortest digits that read back as the
    same double (Python's repr finds the same digits), placed as ryu places them: fixed notation for
    10^-5 <= |x| < 10^16, otherwise d.ddde-7 style, with no '+' in the exponent."""
    x = float(x)
    if not math.isfinite(x):
        raise ValueError('%r has no JSON form' % x)
    if x == 0.0:
        return '-0.0' if math.copysign(1.0, x) < 0 else '0.0'
    t = decimal.Decimal(repr(abs(x))).normalize().as_tuple()
    digits = ''.join(map(str, t.digits))
    k = t.exponent
    n = len(digits)
    kk = n + k                          # 10^(kk - 1) <= |x| < 10^kk
    if 0 <= k and kk <= 16:
        s = digits + '0' * k + '.0'
    elif 0 < kk <= 16:
        s = digits[:kk] + '.' + digits[kk:]
    elif -5 < kk <= 0:
        s = '0.' + '0' * (-kk) + digits
    elif n == 1:
        s = digits + 'e' + str(kk - 1)
    else:
        s = digits[0] + '.' + digits[1:] + 'e' + str(kk - 1)
    return ('-' if x < 0 else '') + s


def obj_text(room):
    """The room as OBJ: every boundary face of the union on the global grid as one quad (so every edge
    is shared by exactly two faces and no vertex lies inside another face's edge), outward-facing,
    grouped by `usemtl` in the order of the room's materials; metres, z up."""
    box_materials = room.get('box_materials')
    if box_materials is None:
        if len(room['materials']) != 1:
            raise ValueError('%s: several materials and no box_materials to place them' % room['name'])
        box_materials = [next(iter(room['materials']))] * len(room['boxes'])
    grid = _Grid([(tuple(b[0]), tuple(b[1]), m) for b, m in zip(room['boxes'], box_materials)])
    verts, index = [], {}

    def vid(p):
        key = tuple(float(c) for c in p)
        if key not in index:
            index[key] = len(verts) + 1
            verts.append(key)
        return index[key]

    blocks = []
    for m in room['materials']:
        faces = []
        for ax, coord, sign, (u0, u1), (v0, v1), mat in grid.faces:
            if mat != m:
                continue
            u, v = (ax + 1) % 3, (ax + 2) % 3
            quad = []
            for uu, vv in ((u0, v0), (u1, v0), (u1, v1), (u0, v1)):
                p = [0.0, 0.0, 0.0]
                p[ax], p[u], p[v] = coord, uu, vv
                quad.append(vid(p))
            if sign < 0:
                quad.reverse()
            faces.append(quad)
        blocks.append((m, faces))
    lines = ['# M8b EDT harness, room %s (HARNESS-PLAN.md 2.2, P7): metres, z up' % room['name']]
    lines += ['v %s %s %s' % tuple(_num(c) for c in p) for p in verts]
    for m, faces in blocks:
        lines.append('usemtl %s' % m)
        lines += ['f %d %d %d %d' % tuple(q) for q in faces]
    return '\n'.join(lines) + '\n'


# ---- the canonical project text (schema/json.rs CanonicalFormatter) ----------------------------------
def _scalar(v):
    if v is None:
        return 'null'
    if v is True:
        return 'true'
    if v is False:
        return 'false'
    if isinstance(v, int):
        return str(v)
    if isinstance(v, float):
        if not math.isfinite(v):
            raise ValueError('a project holds no %r' % v)
        return _num(v)
    if isinstance(v, str):
        return json.dumps(v, ensure_ascii=False)
    raise TypeError('%r' % (v,))


def _canon(v, depth, out):
    pad = '  '
    if isinstance(v, dict):
        if not v:
            out.append('{}')
            return
        out.append('{')
        for i, (k, x) in enumerate(v.items()):
            out.append((',' if i else '') + '\n' + pad * (depth + 1) + json.dumps(k, ensure_ascii=False) + ': ')
            _canon(x, depth + 1, out)
        out.append('\n' + pad * depth + '}')
    elif isinstance(v, list):
        out.append('[')
        multiline = False
        for i, x in enumerate(v):
            if isinstance(x, (dict, list)):
                out.append((',' if i else '') + '\n' + pad * (depth + 1))
                multiline = True
                _canon(x, depth + 1, out)
            else:
                if i:
                    out.append((',\n' + pad * (depth + 1)) if multiline else ', ')
                out.append(_scalar(x))
        if multiline:
            out.append('\n' + pad * depth)
        out.append(']')
    else:
        out.append(_scalar(v))


def canonical_json(value):
    """A project's canonical text as schema::to_json writes it: objects one key per line, two-space
    indent; arrays on one line while their elements are scalars, one element per line once one is an
    object or an array; numbers in serde_json's shortest form (_num); a final newline. write_projects
    checks every result with `simpa repair`, which loads and re-saves through the canonical writer."""
    out = []
    _canon(value, 0, out)
    return ''.join(out) + '\n'


# ---- the projects -------------------------------------------------------------------------------------
def _uid(*parts):
    return str(uuid.uuid5(ID_NAMESPACE, '/'.join(str(p) for p in parts)))


def _not_on_b(path):
    p = Path(os.path.abspath(path))
    if p.drive.upper() == 'B:':
        raise ValueError('%s is on B: (exFAT, 128 KB clusters): the harness writes only on C:' % p)
    return p


def run_simpa(simpa_exe, *args, timeout=600):
    return subprocess.run([str(simpa_exe), *map(str, args)], capture_output=True, text=True, timeout=timeout)


def _template():
    return json.loads(TEMPLATE.read_text(encoding='utf-8'))


def project_from_import(room, imported, template=None):
    """The edits (mkrooms.py:13-18's canonical-edit discipline) that make a fresh import of the room's
    OBJ into its project, and the list of what they change. The material, source and receiver
    prototypes are tutorial1_box.simpa's, handled as bed/file.rs:700-782 handles them: the material
    takes the room's alpha and scattering in every band, Lambert, no transmission, no solver id; the
    receivers are named R000, R001, ..."""
    template = template or _template()
    name = room['name']
    P = copy.deepcopy(imported)
    n = len(P['bands']['frequencies_hz'])
    mats, mat_of = [], {}
    for m, ms in room['materials'].items():
        x = copy.deepcopy(template['materials'][0])
        x.update(id=_uid(name, 'material', m), name='%s %s: alpha %s, scattering %s' % (name, m, ms['absorption'], ms['scattering']),
                 absorption=[float(ms['absorption'])] * n, scattering=[float(ms['scattering'])] * n,
                 reflection_law='lambert', transmission_loss_db=None, solver_id=None)
        mats.append(x)
        mat_of[m] = x['id']
    groups = [g['name'] for g in P['surface_groups']]
    if sorted(groups) != sorted(room['materials']):
        raise ValueError('%s: the import made groups %s, the room has materials %s' % (name, groups, list(room['materials'])))
    for g in P['surface_groups']:
        g['material'] = mat_of[g['name']]
    P['materials'] = mats
    src = copy.deepcopy(template['sources'][0])
    src.update(id=_uid(name, 'source', 0), enabled=True, position=[float(c) for c in room['source_m']],
               delay_s=0.0, group=None, solver_id=None)
    P['sources'] = [src]
    recs = []
    for i, r in enumerate(room['receivers']):
        x = copy.deepcopy(template['point_receivers'][0])
        x.update(id=_uid(name, 'point receiver', i), name=r['name'], position=[float(c) for c in r['position_m']],
                 solver_id=None)
        recs.append(x)
    P['point_receivers'] = recs
    P['description'] = ('M8b EDT held-out test, room %s (docs/investigations/2026-09-27-edt-heldout/HARNESS-PLAN.md '
                        '2.2): %s. Written by harness/m8b/rooms.py from a fresh import of its OBJ.' % (name, room['role']))
    sp = P['solvers']['spps']
    sp.update(random_seed=PROJECT_SEED, save_surface_intersections=False, save_receiver_intersections=False, fittings=False)
    changed = ['description', 'materials', 'surface_groups[*].material', 'sources', 'point_receivers',
               'solvers.spps.random_seed', 'solvers.spps.save_surface_intersections',
               'solvers.spps.save_receiver_intersections', 'solvers.spps.fittings']
    # the stated changes and no other
    a, b = copy.deepcopy(imported), copy.deepcopy(P)
    for x in (a, b):
        for k in ('description', 'materials', 'sources', 'point_receivers'):
            x.pop(k)
        for g in x['surface_groups']:
            g.pop('material')
        for k in ('random_seed', 'save_surface_intersections', 'save_receiver_intersections', 'fittings'):
            x['solvers']['spps'].pop(k)
    if a != b:
        raise AssertionError('%s: the edit changed more than it states' % name)
    return P, changed


def write_projects(out_dir, simpa_exe, names=None, room_dict=None):
    """The nine projects (or those in names) under out_dir (never on B:), each checked: `simpa
    validate` exit 0, `simpa check` exit 0, and `simpa repair` giving back the same bytes (the
    canonical layout). Raises RuntimeError on any failure. Returns {name: path}. room_dict: the rooms to write
    when they are not this module's own (round 2's G rooms come from rooms2)."""
    out = _not_on_b(out_dir)
    imp = out / 'import'
    imp.mkdir(parents=True, exist_ok=True)
    template = _template()
    written = {}
    R = rooms() if room_dict is None else room_dict
    for name in (NAMES + PROBES if names is None else names):
        room = R[name]
        obj = imp / ('%s.obj' % name)
        obj.write_text(obj_text(room), encoding='utf-8', newline='\n')
        fresh = imp / ('%s.simpa' % name)
        if fresh.exists():
            fresh.unlink()
        r = run_simpa(simpa_exe, 'import', obj, fresh, '--unit', 'm', '--up', 'z', '--json')
        if r.returncode != 0:
            raise RuntimeError('%s: simpa import exit %d: %s' % (name, r.returncode, r.stderr[-2000:]))
        rep = json.loads(r.stdout)
        if abs(rep['volume_m3'] / room['volume_m3'] - 1) > 1e-9 or abs(rep['area_m2'] / room['surface_m2'] - 1) > 1e-9:
            raise RuntimeError('%s: the import reads V %r, S %r; the room has %r, %r'
                               % (name, rep['volume_m3'], rep['area_m2'], room['volume_m3'], room['surface_m2']))
        P, _ = project_from_import(room, json.loads(fresh.read_text(encoding='utf-8')), template)
        path = out / ('%s.simpa' % name)
        path.write_text(canonical_json(P), encoding='utf-8', newline='\n')
        for cmd in ('validate', 'check'):
            r = run_simpa(simpa_exe, cmd, path, '--json')
            if r.returncode != 0:
                raise RuntimeError('%s: simpa %s exit %d: %s' % (name, cmd, r.returncode, (r.stdout + r.stderr)[-2000:]))
        copy_path = imp / ('%s.repaired.simpa' % name)
        if copy_path.exists():
            copy_path.unlink()
        r = run_simpa(simpa_exe, 'repair', path, copy_path)
        same = r.returncode == 0 and copy_path.is_file() and copy_path.read_bytes() == path.read_bytes()
        if not same:
            raise RuntimeError('%s: not in the canonical layout (simpa repair exit %d, stdout %r)' % (name, r.returncode, r.stdout[-500:]))
        copy_path.unlink()
        written[name] = path
    return written


def mesh_projects(written, out_dir, simpa_exe, solvers_dir):
    """`simpa mesh <project> --out <out_dir>/<name>` with the solver folder's tetgen.exe and
    preprocess.exe, then `simpa mesh-verify` on the folder: TetGen only, no solver. Returns
    {name: {'mesh_exit', 'verify_exit', 'tetrahedra', 'nodes', 'mesh_codes', 'verify_codes', ...}}."""
    out = _not_on_b(out_dir)
    out.mkdir(parents=True, exist_ok=True)
    solvers = Path(solvers_dir)
    res = {}
    for name, path in written.items():
        d = out / name
        if d.exists() and any(d.iterdir()):
            raise RuntimeError('%s exists and is not empty: a mesh folder is never reused' % d)
        m = run_simpa(simpa_exe, 'mesh', path, '--out', d, '--json', '--tetgen', solvers / 'tetgen.exe',
                      '--preprocess', solvers / 'preprocess.exe', timeout=3600)
        rec = dict(project=str(path), mesh_dir=str(d), mesh_exit=m.returncode, mesh_stderr_lines=len(m.stderr.splitlines()))
        try:
            mm = json.loads(m.stdout)
            build = (mm.get('counts') or {}).get('build') or {}
            rec.update(mesh_status=mm.get('status'), mesh_codes=mm.get('codes'), tetrahedra=build.get('tetrahedra'),
                       nodes=build.get('nodes'))
        except ValueError:
            rec.update(mesh_stdout=m.stdout[-1000:], mesh_stderr=m.stderr[-1000:])
        v = run_simpa(simpa_exe, 'mesh-verify', d, '--json', timeout=3600)
        rec['verify_exit'] = v.returncode
        try:
            vv = json.loads(v.stdout)
            rec.update(verify_codes=vv.get('codes'), verify_mesh=vv.get('mesh'))
        except ValueError:
            rec.update(verify_stdout=v.stdout[-1000:], verify_stderr=v.stderr[-1000:])
        res[name] = rec
    return res


def _jsonable(x):
    if isinstance(x, dict):
        return {str(k): _jsonable(v) for k, v in x.items()}
    if isinstance(x, (list, tuple)):
        return [_jsonable(v) for v in x]
    return x


def main(argv=None):
    import argparse
    import hashlib
    import time

    from . import driver
    ap = argparse.ArgumentParser(description='Write, check and mesh the nine M8b room projects (no solver run).')
    ap.add_argument('--out', type=Path, default=DEFAULT_OUT)
    ap.add_argument('--simpa', type=Path, default=Path(os.environ.get('M8B_SIMPA_EXE', DEFAULT_SIMPA)))
    ap.add_argument('--solvers', type=Path, default=Path(os.environ.get('SIMPA_SOLVERS_DIR', DEFAULT_SOLVERS)))
    ap.add_argument('--no-mesh', action='store_true')
    a = ap.parse_args(argv)
    out = _not_on_b(a.out)
    out.mkdir(parents=True, exist_ok=True)
    t0 = time.time()
    R = rooms()
    clear = check_rooms()
    corpus_rooms = json.loads(CORPUS_ROOMS.read_text(encoding='utf-8'))
    p3 = {n: p3_clearance(R[n], corpus_rooms) for n in NAMES + PROBES}
    written = write_projects(out / 'projects', a.simpa)
    record = dict(simpa=str(a.simpa), simpa_sha256=hashlib.sha256(Path(a.simpa).read_bytes()).hexdigest(),
                  rooms=_jsonable(R), clearance=clear, p3=p3, projects={n: str(p) for n, p in written.items()})
    if not a.no_mesh:
        check = driver.check_solvers(a.solvers)
        record['solvers'] = check
        if not check['verified']:
            raise SystemExit('%s is not the verified build: %s' % (a.solvers, check))
        record['mesh'] = mesh_projects(written, out / 'mesh', a.simpa, a.solvers)
    record['seconds'] = round(time.time() - t0, 1)
    (out / 'rooms.json').write_text(json.dumps(record, indent=1) + '\n', encoding='utf-8', newline='\n')
    for n in NAMES + PROBES:
        m = record.get('mesh', {}).get(n, {})
        g = p3[n]
        print('%-3s %-7s V %8.2f S %8.2f T60(500) %.2f s  P3 gap %s (%s)  mesh %s verify %s  %s tets'
              % (n, R[n]['kind'], R[n]['volume_m3'], R[n]['surface_m2'], R[n]['design_t60_s'][500],
                 '-' if g['gap'] is None else '%.1f %%' % (100 * g['gap']), g['nearest'],
                 m.get('mesh_exit'), m.get('verify_exit'), m.get('tetrahedra')))
    print('wrote %s (%.1f s)' % (out / 'rooms.json', record['seconds']))
    bad = [n for n, m in record.get('mesh', {}).items() if m['mesh_exit'] != 0 or m['verify_exit'] != 0]
    return 1 if bad else 0


if __name__ == '__main__':
    sys.exit(main())
