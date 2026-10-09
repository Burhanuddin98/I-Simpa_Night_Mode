"""Parity A43: upstream I-Simpa's tutorials 1 to 3 as example projects, each at the configuration its own document
gives (upstream `Docs/tutorial_teaching_room.rst`, `tutorial_Elmia_hall.rst`, `tutorial_industrial_hall.rst`, tag
v1.4.0_snapshot_14_01_2026). Where upstream's tutorial project (`tutorial_N.proj`) and its document disagree, the
document is followed and the difference is listed in the project's description and in `ATTRIBUTION.md`.

usage: python build_tutorials.py <simpa.exe>
Reads, from this repository: tests/fixtures/rooms/tutorial1_box.simpa (tutorial_1.proj imported),
tests/fixtures/rooms/elmia_corrected.simpa (tutorial_2.proj imported) and examples/industrial_hall.simpa (tutorial_3.proj
imported, Receiver 1 moved off the wall: build_industrial.py). Writes tutorial_1.simpa, tutorial_2.simpa and
tutorial_3.simpa beside this file, each passed through `simpa repair` (the canonical writer) and `simpa validate`.
Deterministic: no id is made here.
"""
import json
import os
import subprocess
import sys
import tempfile


HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.abspath(os.path.join(HERE, "..", "..", ".."))
OCTAVES = [125, 250, 500, 1000, 2000, 4000]


def load(path):
    with open(path, encoding="utf-8") as f:
        return json.load(f)


def octave_bands(p):
    """`bands_computed` for the octave bands 125 Hz to 4 kHz on the project's band set ('Building/Road [125-4000]')."""
    return [f in OCTAVES for f in p["bands"]["frequencies_hz"]]


def tutorial_1():
    # The .proj is the tutorial's own end state (rst:24 "with TCR and SPPS results"): the 6 x 10 x 3 m box (rst:8, 43),
    # Ceiling 30 %, Floor 10 %, Walls 20 % (rst:69), the source at (3, 5, 1.8) at 80 dB of white noise (rst:76-80), the
    # floor's surface receiver (rst:85-87), receivers at (1, 1, 1.8) and (3, 7, 1.8) (rst:94-96) and the 0.1 m2
    # surface-receiver constraint (rst:106-108). Nothing to change but the name and the text.
    p = load(os.path.join(REPO, "tests", "fixtures", "rooms", "tutorial1_box.simpa"))
    assert [s["position"] for s in p["sources"]] == [[3.0, 5.0, 1.8]]
    assert [r["position"] for r in p["point_receivers"]] == [[1.0, 1.0, 1.8], [3.0, 7.0, 1.8]]
    assert p["solvers"]["meshing"]["surface_receiver_max_area_m2"] == 0.1
    p["name"] = "Tutorial 1: a teaching room"
    p["description"] = (
        "Upstream I-Simpa's first tutorial, the study of a teaching room (Docs/tutorial_teaching_room.rst), from its "
        "project tutorial_1.proj: a box room, one source, two receivers and a surface receiver on the floor, set up as "
        "the tutorial's steps leave it. Run TCR, then SPPS. Help, Tutorial 1 has the text and what upstream expects.")
    return p


def tutorial_2():
    # tutorial_2.proj's hall (upstream's corrected geometry: our stand-in for the tutorial's 'Average model remesh',
    # which is not in this version) with the document's SPPS settings, rst:150-159: octave bands 125 Hz to 4 kHz,
    # no scene correction, time step 0.005 s, Energetic, 100 000 particles per source, no surface receivers per band.
    p = load(os.path.join(REPO, "tests", "fixtures", "rooms", "elmia_corrected.simpa"))
    sp = p["solvers"]["spps"]
    sp["bands_computed"] = octave_bands(p)
    p["solvers"]["meshing"]["preprocess"] = False
    sp.update({"time_step_s": 0.005, "method": "energetic", "particles_per_source": 100_000, "sound_maps_per_band": False})
    p["name"] = "Tutorial 2: the Elmia hall"
    p["description"] = (
        "Upstream I-Simpa's second tutorial, the Elmia hall of the second Round Robin (Docs/tutorial_Elmia_hall.rst), "
        "from its project tutorial_2.proj, with SPPS set as the tutorial's calculation step says. The hall is the "
        "corrected geometry that project carries, not a new average-model remesh. Help, Tutorial 2 has the text and "
        "what upstream expects.")
    return p


def tutorial_3():
    p = load(os.path.join(HERE, "industrial_hall.simpa"))
    groups = {g["name"]: g for g in p["surface_groups"]}
    mats = {m["name"]: m for m in p["materials"]}
    freqs = p["bands"]["frequencies_hz"]
    n = len(freqs)
    # Sources: pink noise at 80 dB (rst:57, 63); the .proj has white noise.
    for s in p["sources"]:
        assert s["power"]["global_db"] == 80.0
        s["power"]["shape"] = {"kind": "pink"}
    # Fitting zones (rst:113, 136): the scene zone alpha 0.25, lambda 0.5 m; the box zone (13, 4, 0) to (18, 1, 1.2),
    # alpha 0.15, lambda 0.3 m; both uniform. The .proj has 0.2 / 1.0 and 0.0 / 1.0.
    z1, z2 = p["fitting_zones"]
    assert z1["shape"]["kind"] == "surfaces" and z2["shape"]["kind"] == "box"
    assert z2["shape"]["min"] == [13.0, 1.0, 0.0] and z2["shape"]["max"] == [18.0, 4.0, 1.2]
    for z, a, lam in ((z1, 0.25, 0.5), (z2, 0.15, 0.3)):
        z["absorption"] = [a] * n
        z["mean_free_path_m"] = [lam] * n
        z["diffusion_law"] = ["uniform"] * n
    # Open_door transmits in every band with 0 dB of loss (rst:199-245); the .proj leaves 125 Hz off.
    od = mats["Open_door"]
    od["transmission_loss_db"] = [0.0 if f in OCTAVES else tl for f, tl in zip(freqs, od["transmission_loss_db"])]
    # door_room1 and door_room2 take Open_door (rst:197); the .proj has Absorbing_material on door_room2.
    for g in ("door_room1", "door_room2"):
        groups[g]["material"] = od["id"]
    # Every other group 30 % absorbing (rst:247): diff_wall included, the reference of the comparison.
    assert mats["30% absorbing"]["id"] == groups["diff_wall"]["material"]
    # The comparison (rst:378-433) is the user's to make, as upstream's steps say: Absorbing_material, already in the
    # project with the document's values, put on diff_wall in the Materials step, then a second run. Not a variant:
    # diff_wall shares the pinned solver material id of '30% absorbing' with seven other groups (the .proj's), and a
    # variant that overrides one group of such a set is refused at export (shared_solver_id, config_xml/write.rs;
    # docs/v1.1-backlog.md). Setting the group's own material re-numbers it and runs.
    ab = mats["Absorbing_material"]
    want = [0.60, 0.62, 0.77, 0.74, 0.80, 0.81]
    assert [ab["absorption"][freqs.index(f)] for f in OCTAVES] == want, ab["absorption"]
    assert p["variants"] == [] and p["active_variant"] is None
    # SPPS as the document's table says (rst:334-373).
    sp = p["solvers"]["spps"]
    sp.update({
        "air_absorption": True, "fittings": True, "direct_field_only": False, "transmission": True,
        "method": "energetic", "echogram_per_source": False, "sound_maps_per_band": False,
        "extinction_exponent": 5.0, "particles_per_source": 150_000, "particles_saved": 0, "random_seed": 0,
        "receiver_radius_m": 0.31, "duration_s": 2.0, "sound_map": "spl", "time_step_s": 0.002,
    })
    sp["bands_computed"] = octave_bands(p)
    p["name"] = "Tutorial 3: an industrial hall"
    p["description"] = (
        "Upstream I-Simpa's third tutorial, an industrial hall coupled to other rooms (Docs/tutorial_industrial_hall.rst), "
        "from its project tutorial_3.proj, set as the tutorial's document says where the project differs: pink noise, "
        "the fitting zones' parameters, both doors open, SPPS's settings. For the tutorial's last part, put "
        "Absorbing_material on diff_wall and run again. Receiver 1 sits off the wall it touched in the project. "
        "Help, Tutorial 3 has the text and what upstream expects.")
    return p


def main():
    simpa = sys.argv[1]
    for name, p in (("tutorial_1.simpa", tutorial_1()), ("tutorial_2.simpa", tutorial_2()), ("tutorial_3.simpa", tutorial_3())):
        with tempfile.TemporaryDirectory() as tmp:
            raw = os.path.join(tmp, "raw.simpa")
            with open(raw, "w", encoding="utf-8") as f:
                json.dump(p, f, indent=2)
            out = os.path.join(HERE, name)
            subprocess.run([simpa, "repair", raw, out], check=True)
        v = subprocess.run([simpa, "validate", out], capture_output=True, text=True)
        print(name, "validate exit", v.returncode, (v.stdout + v.stderr).strip()[:400])
        if v.returncode != 0:
            sys.exit(1)


if __name__ == "__main__":
    main()
