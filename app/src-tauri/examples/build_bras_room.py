"""Build a ready-to-run BRAS example (CR1 or CR3) from its sealed geometry, as build_cr2.py built CR2:
BRAS fitted materials (octave = mean of the three third-octave values), the ITA dodecahedron sources and the
microphones from positions.json, octave bands 125 Hz-4 kHz, energetic SPPS. Writes only under .out.

Usage: python build_bras_room.py CR1|CR3 [particles_per_source]
Source: B:\\data\\m12\\pearl-geom\\cr2-solve\\build_cr2.py (10-04), generalised; colours added per material.
"""
import json
import sys
import uuid
from pathlib import Path

import numpy as np

ROOM = sys.argv[1]
PARTICLES = int(sys.argv[2]) if len(sys.argv) > 2 else 300_000
SCENES = {"CR1": "BRAS scene 08 (CR1)", "CR3": "BRAS scene 10 (CR3, chamber music hall)"}
SRC = Path(r"B:\data\m12\pearl-geom") / f"{ROOM.lower()}-clean" / f"{ROOM}_clean.simpa"
OUT = Path(r"B:\repos\I-Simpa_Night_Mode\.out\examples") / f"{ROOM}.simpa"
SURF = Path(r"D:\Datasets\RoomAcoustics\bras\_raw\3 Surface descriptions\_csv\fitted_estimates")
POS = Path(r"D:\Datasets\RoomAcoustics\bras\positions.json")

OCT = [125, 250, 500, 1000, 2000, 4000]
MEMBERS = {125: [100, 125, 160], 250: [200, 250, 315], 500: [400, 500, 630],
           1000: [800, 1000, 1250], 2000: [1600, 2000, 2500], 4000: [3150, 4000, 5000]}
# A swatch per BRAS material, muted, red kept out (it marks actions and selection in the app).
COLORS = {"ceiling": "#d9d9d9", "concrete": "#8c8c8c", "floor": "#a0522d", "plaster": "#f5f0e1",
          "windows": "#87ceeb", "absorber": "#6b8e9f", "paintedConcrete": "#b8b4ab",
          "tablesEquipment": "#9c7a54", "seating": "#7a5c8a", "stagePanels": "#c19a6b",
          "structuredPlaster": "#e8dcc4"}


def read_mat(name):
    rows = [l for l in (SURF / f"mat_{ROOM}_{name}.csv").read_text().splitlines() if l.strip()]
    f = [float(x) for x in rows[0].split(",")]
    a = [float(x) for x in rows[1].split(",")]
    s = [float(x) for x in rows[2].split(",")]
    idx = {round(v): i for i, v in enumerate(f)}
    oa = [float(np.mean([a[idx[m]] for m in MEMBERS[o]])) for o in OCT]
    os_ = [float(np.mean([s[idx[m]] for m in MEMBERS[o]])) for o in OCT]
    return oa, os_


p = json.loads(SRC.read_text(encoding="utf-8"))
p["name"] = ROOM
p["description"] = (f"{SCENES[ROOM]}, sealed geometry; BRAS fitted_estimates materials (octave = arithmetic mean "
                    "of 3 third-octave values); ITA dodecahedron sources and the BRAS microphone positions. "
                    "BRAS database, CC BY-SA 4.0.")
p["bands"] = {"kind": "octave", "frequencies_hz": OCT}
mats = []
for g in p["surface_groups"]:
    short = g["name"].replace(f"mat_{ROOM}_", "")
    a, s = read_mat(short)
    mid = str(uuid.uuid4())
    mats.append({"id": mid, "name": g["name"], "color": COLORS.get(short, "#9a9aa0"),
                 "absorption": [round(x, 6) for x in a], "scattering": [round(x, 6) for x in s],
                 "reflection_law": "lambert", "transmission_loss_db": None,
                 "double_sided": True, "solver_id": None})
    g["material"] = mid
p["materials"] = mats

if ROOM == "CR1":
    # positions.json has no CR1; read it as that file was made, from the SOFA file of the scene the geometry is
    # (DoorAngle3): EmitterPosition rows by EmitterID (LS1, LS2), ReceiverPosition rows by ReceiverID (MP3, MP4).
    import h5py
    with h5py.File(r"D:\Datasets\RoomAcoustics\bras\sofa\CR1_RIRs_DoorAngle3_Dodecahedron.sofa", "r") as f:
        em = np.array(f["EmitterPosition"]).reshape(-1, 3)
        rc = np.array(f["ReceiverPosition"]).reshape(-1, 3)
        eid = np.array(f["EmitterID"]).ravel().astype(int)
        rid = np.array(f["ReceiverID"]).ravel().astype(int)
    pos = {"sources": {f"LS{i}": [round(float(c), 3) for c in xyz] for i, xyz in zip(eid, em)},
           "receivers": {f"MP{i}": [round(float(c), 3) for c in xyz] for i, xyz in zip(rid, rc)}}
else:
    pos = json.loads(POS.read_text())["rooms"][ROOM]["sources"]["dodecahedron"]
p["sources"] = [{"id": str(uuid.uuid4()), "name": n, "enabled": True, "position": xyz,
                 "power": {"global_db": 90.0, "shape": {"kind": "pink"}},
                 "directivity": {"kind": "omni"}, "delay_s": 0.0, "group": None, "solver_id": None}
                for n, xyz in sorted(pos["sources"].items())]
p["point_receivers"] = [{"id": str(uuid.uuid4()), "name": n, "position": xyz,
                         "orientation": [0.0, 0.0, 1.0], "background_noise": None, "solver_id": None}
                        for n, xyz in sorted(pos["receivers"].items())]

sp = p["solvers"]["spps"]
sp["particles_per_source"] = PARTICLES
sp["bands_computed"] = [True] * len(OCT)
sp["echogram_per_source"] = True
sp["method"] = "energetic"
p["solvers"]["tcr"]["bands_computed"] = [True] * len(OCT)

OUT.parent.mkdir(parents=True, exist_ok=True)
OUT.write_text(json.dumps(p, indent=2), encoding="utf-8")
print(ROOM, "sources", [s["name"] for s in p["sources"]], "receivers", [r["name"] for r in p["point_receivers"]],
      "materials", [(m["name"], m["color"]) for m in mats])
