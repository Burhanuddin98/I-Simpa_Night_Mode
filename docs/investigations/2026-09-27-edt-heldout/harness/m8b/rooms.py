"""SPPS-fresh's rooms F1-F7 and the probes' stand-ins P0 and P0b: the nine projects (HARNESS-PLAN.md
2.2, P3, P5, P7, section 7).

STUB (HARNESS-PLAN.md section 6, step 3): every function returns None. Tests T10 and T11 hold the
contract below.

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
"""
NAMES = ('F1', 'F2', 'F3', 'F4', 'F5', 'F6', 'F7')
PROBES = ('P0', 'P0b')
BANDS_HZ = (125, 250, 500, 1000, 2000, 4000)


def rooms():
    return None


def near_duplicate(a, b):
    return None


def obj_text(room):
    return None


def write_projects(out_dir, simpa_exe):
    return None
