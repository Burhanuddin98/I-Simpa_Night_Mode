"""A4 bed: write the projects for the two rules A2 left untested (transmission, one-sided faces).

Every case is the same 6 x 10 x 3 m box split at y = 5 by an internal panel (a partition: 18 m2,
two triangles, its edges on the outer walls). Room 1 is y 0-5, room 2 is y 5-10. The panel's
winding gives it the normal +y (towards room 2): SPPS computes a scene face's normal as
(b - a) x (c - b) (lib_interface/Core/mathlib.h FaceNormal), so a particle in room 2 moving -y has
dir . n < 0, which is the "doInvertNormal" side (CalculationCore.cpp: dir . n <= -BARELY_EPSILON).

Cases (octave bands 500 and 1000 Hz; walls, floor and ceiling one material):
  trans      panel alpha 0.3, TL 10 dB at 500 Hz and 20 dB at 1 kHz, double-sided; every surface
             fully diffuse (scattering 1, Lambert); one omni source in room 1; four receivers in
             each room. Energetic: the child-spawning branch (0 < alpha < 1).
  trans-a1   the same with the panel alpha 1: the parent passes with E * tau (alpha == 1 branch).
  beam       the trans panel and walls, specular (scattering 0), a unidirectional source in
             room 1 aimed +y at the panel, air absorption off, receivers on the beam axis: every
             particle follows one deterministic path, so the energies are exact (no RNG).
  onesided   panel alpha 0.2, no transmission, single-sided (side_material 0); one source in each
             room; walls and panel diffuse; echograms per source.
  beam-a1    the beam with the trans-a1 panel (alpha 1, TL 10 / 20 dB): the parent itself passes.
  beam-onesided  the onesided panel and walls, specular, air absorption off: a unidirectional source
             in room 1 aimed +y (at the panel's reflecting side) and one in room 2 aimed -y (at its
             passing side), receivers on the beam axis.
Random (particle) mode is the trans case staged with computation_method=0.

usage: python a4_cases.py <out-dir>      writes <case>.simpa for each case
"""
import json, os, sys, uuid

NS = uuid.UUID("a4a4a4a4-0000-4000-8000-000000000a04")


def uid(name):
    return str(uuid.uuid5(NS, name))


V = [
    (0, 0, 0), (6, 0, 0), (6, 0, 3), (0, 0, 3),      # y = 0   (0-3)
    (0, 5, 0), (6, 5, 0), (6, 5, 3), (0, 5, 3),      # y = 5   (4-7)
    (0, 10, 0), (6, 10, 0), (6, 10, 3), (0, 10, 3),  # y = 10  (8-11)
]


def normal(a, b, c):
    A, B, C = V[a], V[b], V[c]
    u = [B[i] - A[i] for i in range(3)]
    w = [C[i] - B[i] for i in range(3)]
    return (u[1] * w[2] - u[2] * w[1], u[2] * w[0] - u[0] * w[2], u[0] * w[1] - u[1] * w[0])


def quad(a, b, c, d, want, group):
    """Two triangles of the quad a-b-c-d, wound so that their normal points along `want`."""
    tris = [(a, b, c), (a, c, d)]
    out = []
    for t in tris:
        n = normal(*t)
        if sum(n[i] * want[i] for i in range(3)) < 0:
            t = (t[0], t[2], t[1])
        assert sum(normal(*t)[i] * want[i] for i in range(3)) > 0
        out.append([t[0], t[1], t[2], group])
    return out


G = {k: uid("group-" + k) for k in ("floor1", "floor2", "ceil1", "ceil2", "walls1", "walls2", "panel")}


def faces():
    f = []
    f += quad(0, 1, 5, 4, (0, 0, -1), G["floor1"])
    f += quad(4, 5, 9, 8, (0, 0, -1), G["floor2"])
    f += quad(3, 2, 6, 7, (0, 0, 1), G["ceil1"])
    f += quad(7, 6, 10, 11, (0, 0, 1), G["ceil2"])
    f += quad(0, 3, 7, 4, (-1, 0, 0), G["walls1"])   # x = 0, room 1
    f += quad(1, 2, 6, 5, (1, 0, 0), G["walls1"])    # x = 6, room 1
    f += quad(0, 1, 2, 3, (0, -1, 0), G["walls1"])   # y = 0
    f += quad(4, 7, 11, 8, (-1, 0, 0), G["walls2"])  # x = 0, room 2
    f += quad(5, 6, 10, 9, (1, 0, 0), G["walls2"])   # x = 6, room 2
    f += quad(8, 9, 10, 11, (0, 1, 0), G["walls2"])  # y = 10
    f += quad(4, 5, 6, 7, (0, 1, 0), G["panel"])     # y = 5, normal +y (towards room 2)
    return f


def material(name, absorb, scatter, law, tl, double_sided, sid):
    return {"id": uid("mat-" + name), "name": name, "color": "#cccccc", "absorption": absorb,
            "scattering": scatter, "reflection_law": law, "transmission_loss_db": tl,
            "double_sided": double_sided, "solver_id": sid}


def receiver(name, pos, sid):
    return {"id": uid("rcv-" + name), "name": name, "position": list(pos), "orientation": [0.0, 1.0, 0.0],
            "background_noise": {"global_db": 0.0, "shape": {"kind": "white"}}, "solver_id": sid}


def source(name, pos, sid, directivity=None):
    return {"id": uid("src-" + name), "name": name, "enabled": True, "position": list(pos),
            "power": {"global_db": 80.0, "shape": {"kind": "white"}},
            "directivity": directivity or {"kind": "omni"}, "delay_s": 0.0, "group": None, "solver_id": sid}


# four receivers per room, each at least 2.4 m from the room-1 source (2.1, 1.4, 1.45)
ROOM_RX = [("R1a", (4.6, 2.9, 1.10)), ("R1b", (1.0, 3.9, 2.05)), ("R1c", (4.9, 0.8, 2.20)), ("R1d", (1.3, 4.3, 0.90)),
           ("R2a", (1.3, 6.1, 1.20)), ("R2b", (4.8, 6.4, 1.90)), ("R2c", (1.5, 8.9, 1.80)), ("R2d", (4.5, 8.7, 1.10))]
BEAM_X, BEAM_Z = 3.1, 1.55
BEAM_RX = [("B1a", (BEAM_X, 0.6, BEAM_Z)), ("B1b", (BEAM_X, 3.3, BEAM_Z)), ("B2a", (BEAM_X, 6.7, BEAM_Z)), ("B2b", (BEAM_X, 8.6, BEAM_Z))]


def project(case):
    lambert = case not in ("beam", "beam-onesided", "beam-a1")
    sc = [1.0, 1.0] if lambert else [0.0, 0.0]
    law = "lambert" if lambert else "specular"
    wall = material("wall", [0.2, 0.2], sc, law, None, True, 31)
    if case in ("trans", "beam"):
        panel = material("panel", [0.3, 0.3], sc, law, [10.0, 20.0], True, 32)
    elif case in ("trans-a1", "beam-a1"):
        panel = material("panel", [1.0, 1.0], [0.0, 0.0], "specular", [10.0, 20.0], True, 32)
    elif case in ("onesided", "beam-onesided"):
        panel = material("panel", [0.2, 0.2], sc, law, None, False, 32)
    else:
        sys.exit(f"unknown case {case}")
    groups = [{"id": G[k], "name": k, "material": (panel if k == "panel" else wall)["id"]} for k in G]
    if case in ("beam", "beam-a1"):
        sources = [source("S1", (BEAM_X, 1.6, BEAM_Z), 901, {"kind": "unidirectional", "direction": [0.0, 1.0, 0.0]})]
        rx = BEAM_RX
    elif case == "beam-onesided":
        sources = [source("Srefl", (BEAM_X, 1.6, BEAM_Z), 901, {"kind": "unidirectional", "direction": [0.0, 1.0, 0.0]}),
                   source("Spass", (BEAM_X, 8.0, BEAM_Z), 902, {"kind": "unidirectional", "direction": [0.0, -1.0, 0.0]})]
        rx = BEAM_RX
    elif case == "onesided":
        sources = [source("Srefl", (2.1, 1.4, 1.45), 901), source("Spass", (3.0, 7.4, 1.60), 902)]
        rx = ROOM_RX
    else:
        sources = [source("S1", (2.1, 1.4, 1.45), 901)]
        rx = ROOM_RX
    return {
        "format_version": 1, "id": uid("project-" + case), "name": "A4 " + case,
        "description": "A4 bed (solvers/spps-gpu/bed/a4_cases.py): 6 x 10 x 3 m box split at y = 5 by an internal panel; case " + case + ".",
        "frame": {"length_unit": "metre", "up_axis": "z"},
        "bands": {"kind": "octave", "frequencies_hz": [500, 1000]},
        "geometry": {"vertices": [list(map(float, v)) for v in V], "faces": faces()},
        "surface_groups": groups,
        "materials": [wall, panel],
        "sources": sources,
        "point_receivers": [receiver(n, p, 1001 + i) for i, (n, p) in enumerate(rx)],
        "surface_receivers": [],
        "fitting_zones": [],
        "environment": {"temperature_c": 20.0, "relative_humidity_percent": 50.0, "pressure_pa": 101325.0,
                        "air_absorption": {"kind": "iso9613"}, "ground_roughness_m": 0.02,
                        "celerity_gradient_log": 0.0, "celerity_gradient_lin": 0.0},
        "solvers": {
            "spps": {"particles_per_source": 2000, "particles_saved": 0, "duration_s": 3.0, "time_step_s": 0.001,
                     "random_seed": 1, "method": "energetic", "air_absorption": not case.startswith("beam"), "fittings": True,
                     "direct_field_only": False, "transmission": True, "extinction_exponent": 7.0,
                     "receiver_radius_m": 0.31, "sound_map": "intensity", "sound_maps_per_band": True,
                     "echogram_per_source": True, "save_surface_intersections": False,
                     "save_receiver_intersections": False, "bands_computed": [True, True]},
            "tcr": {"air_absorption": True, "bands_computed": [True, True]},
            "meshing": {"min_radius_edge_ratio": 2.0, "max_volume_m3": None, "surface_receiver_max_area_m2": 4.0,
                        "preserve_boundary": False, "preprocess": False}},
        "variants": [], "active_variant": None, "view": {"camera": None},
    }


if __name__ == "__main__":
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    out = sys.argv[1]
    os.makedirs(out, exist_ok=True)
    for case in ("trans", "trans-a1", "beam", "onesided", "beam-onesided", "beam-a1"):
        p = os.path.join(out, case + ".simpa")
        with open(p, "w", encoding="utf-8") as fh:
            json.dump(project(case), fh, indent=1)
        print(p)
