"""Write the room fixtures derived from the two imported rooms.

    python tools/fixture-gen/mkrooms.py <rooms-dir> [--simpa <simpa.exe>]

From <rooms-dir>/tutorial1_box.simpa and elmia_corrected.simpa (tests/fixtures/rooms/), writes:
- tutorial1_box_seeded.simpa: SPPS random_seed 1 and 10,000 particles per source, M1's reference
  configuration (tests/fixtures/upstream/tutorial1/PROVENANCE.md), for gate M6(a);
- elmia_loss_gate.simpa: SPPS random_seed 1 and 100,000 particles per source, and both solvers'
  bands_computed true for 125, 250, 500, 1000, 2000 and 4000 Hz only, for gate M6(c);
- tutorial1_box_fitting.simpa: tutorial1_box_seeded plus one enabled box fitting zone from
  (1, 1, 0.5) to (2, 2, 1.5) m, for gate M5(e).

The edits are made on the canonical text (docs of `schema::to_json`: two-space indent, scalar
arrays on one line), so the files keep the writer's layout. Two checks can refuse the result:
- parsed as JSON, each output must equal its source with exactly the stated changes applied;
- with --simpa, `simpa validate` must exit 0 on each, and `simpa repair <file> <copy>` (which
  loads and re-saves through the canonical writer, and changes nothing on a clean room) must
  give back the same bytes.
"""
import argparse
import copy
import json
import subprocess
import sys
import tempfile
from pathlib import Path

import fixture_common as fc

SEED = 1
BOX_PARTICLES = 10_000
HALL_PARTICLES = 100_000
LOSS_GATE_BANDS = (125, 250, 500, 1000, 2000, 4000)
ZONE = {
    # A fixed id, so the file is the same on every run (the negative fixtures do the same).
    "id": "0c0be000-0000-4000-8000-00000000f177",
    "name": "Fitting zone",
    "enabled": True,
    # No upstream corner order: a box drawn here (schema FittingShape::Box::destination).
    "shape": {"kind": "box", "min": [1.0, 1.0, 0.5], "max": [2.0, 2.0, 1.5], "destination": None},
    # Valid under fitting_parameters_invalid (docs/solver-contract.md:122): 0 <= alpha <= 1,
    # mean free path > 0, in every band.
    "absorption": 0.1,
    "mean_free_path_m": 1.0,
    "diffusion_law": "uniform",
    # No pinned solver id: a zone drawn here takes the numbering export assigns, 2 as the first
    # zone (docs/m5-m6-design.md, decision 13 pins only what a .proj import brings).
    "solver_id": None,
}


def replace_once(text: str, old: str, new: str, start: int = 0, end: int | None = None) -> str:
    end = len(text) if end is None else end
    span = text[start:end]
    if span.count(old) != 1:
        fc.die(f"expected one {old!r} in the section, found {span.count(old)}")
    return text[:start] + span.replace(old, new) + text[end:]


def section(text: str, key: str) -> tuple[int, int]:
    """Start and end of the object `"key": {` ... its closing brace at the same indent."""
    head = f'"{key}": {{'
    if text.count(head) != 1:
        fc.die(f"expected one {head!r}, found {text.count(head)}")
    start = text.index(head)
    indent = text.rfind("\n", 0, start) + 1
    pad = text[indent:start]
    end = text.index("\n" + pad + "}", start)
    return start, end


def bool_array(values: list[bool]) -> str:
    return "[" + ", ".join("true" if v else "false" for v in values) + "]"


def set_spps(text: str, seed: int, particles: int, old: dict) -> str:
    s, e = section(text, "spps")
    text = replace_once(text, f'"random_seed": {old["random_seed"]},', f'"random_seed": {seed},', s, e)
    s, e = section(text, "spps")
    return replace_once(text, f'"particles_per_source": {old["particles_per_source"]},',
                        f'"particles_per_source": {particles},', s, e)


def set_bands(text: str, solver: str, old: list[bool], new: list[bool]) -> str:
    s, e = section(text, solver)
    return replace_once(text, f'"bands_computed": {bool_array(old)}', f'"bands_computed": {bool_array(new)}', s, e)


def zone_text(n_bands: int) -> str:
    z = ZONE
    lines = [
        '  "fitting_zones": [',
        "    {",
        f'      "id": "{z["id"]}",',
        f'      "name": "{z["name"]}",',
        '      "enabled": true,',
        '      "shape": {',
        f'        "kind": "{z["shape"]["kind"]}",',
        f'        "min": {json.dumps(z["shape"]["min"])},',
        f'        "max": {json.dumps(z["shape"]["max"])},',
        f'        "destination": {json.dumps(z["shape"]["destination"])}',
        "      },",
        f'      "absorption": {json.dumps([z["absorption"]] * n_bands)},',
        f'      "mean_free_path_m": {json.dumps([z["mean_free_path_m"]] * n_bands)},',
        f'      "diffusion_law": {json.dumps([z["diffusion_law"]] * n_bands)},',
        f'      "solver_id": {json.dumps(z["solver_id"])}',
        "    }",
        "  ],",
    ]
    return "\n".join(lines)


def expect_equal(name: str, got_text: str, want: dict) -> None:
    got = json.loads(got_text)
    if got != want:
        fc.die(f"{name}: the edit changed more (or less) than stated")


def derive(rooms: Path) -> dict[str, str]:
    box_text = (rooms / "tutorial1_box.simpa").read_bytes().decode("utf-8")
    hall_text = (rooms / "elmia_corrected.simpa").read_bytes().decode("utf-8")
    box, hall = json.loads(box_text), json.loads(hall_text)
    out = {}

    seeded = set_spps(box_text, SEED, BOX_PARTICLES, box["solvers"]["spps"])
    want = copy.deepcopy(box)
    want["solvers"]["spps"].update(random_seed=SEED, particles_per_source=BOX_PARTICLES)
    expect_equal("tutorial1_box_seeded", seeded, want)
    out["tutorial1_box_seeded.simpa"] = seeded

    n = len(box["bands"]["frequencies_hz"])
    fitting = replace_once(seeded, '  "fitting_zones": [],', zone_text(n))
    want = copy.deepcopy(want)
    want["fitting_zones"] = [dict(ZONE, absorption=[ZONE["absorption"]] * n,
                                  mean_free_path_m=[ZONE["mean_free_path_m"]] * n,
                                  diffusion_law=[ZONE["diffusion_law"]] * n)]
    expect_equal("tutorial1_box_fitting", fitting, want)
    out["tutorial1_box_fitting.simpa"] = fitting

    gate = set_spps(hall_text, SEED, HALL_PARTICLES, hall["solvers"]["spps"])
    bands = [f in LOSS_GATE_BANDS for f in hall["bands"]["frequencies_hz"]]
    if sum(bands) != len(LOSS_GATE_BANDS):
        fc.die("elmia_corrected lacks one of the loss-gate bands")
    for solver in ("spps", "tcr"):
        old = hall["solvers"][solver]["bands_computed"]
        if old != bands:
            gate = set_bands(gate, solver, old, bands)
    want = copy.deepcopy(hall)
    want["solvers"]["spps"].update(random_seed=SEED, particles_per_source=HALL_PARTICLES,
                                   bands_computed=bands)
    want["solvers"]["tcr"]["bands_computed"] = bands
    expect_equal("elmia_loss_gate", gate, want)
    out["elmia_loss_gate.simpa"] = gate
    return out


def check_with_cli(simpa: Path, path: Path) -> None:
    r = subprocess.run([str(simpa), "validate", str(path)], capture_output=True, text=True)
    if r.returncode != 0:
        fc.die(f"simpa validate {path.name} exited {r.returncode}:\n{r.stdout}{r.stderr}")
    with tempfile.TemporaryDirectory() as tmp:
        copy_path = Path(tmp) / path.name
        r = subprocess.run([str(simpa), "repair", str(path), str(copy_path)], capture_output=True, text=True)
        if r.returncode != 0 or r.stdout.strip():
            fc.die(f"simpa repair {path.name}: exit {r.returncode}, changes {r.stdout!r}")
        if copy_path.read_bytes() != path.read_bytes():
            fc.die(f"{path.name} is not in the canonical layout: a load and save changes it")
    print(f"  {path.name}: simpa validate exit 0, canonical")


def main() -> None:
    ap = argparse.ArgumentParser(description="Write the derived room fixtures.")
    ap.add_argument("rooms", type=Path)
    ap.add_argument("--simpa", type=Path)
    a = ap.parse_args()
    if a.simpa:
        for base in ("tutorial1_box.simpa", "elmia_corrected.simpa"):
            check_with_cli(a.simpa, a.rooms / base)
    for name, text in derive(a.rooms).items():
        fc.write_text(a.rooms / name, text)
        print("wrote", name)
        if a.simpa:
            check_with_cli(a.simpa, a.rooms / name)


if __name__ == "__main__":
    sys.exit(main())
