"""Write the stub-solver fixtures: classifier rows that no run through `run-folder` can reach.

    python tools/fixture-gen/mkstubs.py <runs-dir>

Each stub_<row>/ holds spps_ok's config.xml, mesh.cbin and tetramesh.mbin (so run-folder's
pre-launch mesh check passes) and a stub.json in the format of docs/m5-m6-design.md ("Stub
solver"): the exit code, the lines in order with their stream and whether each ends in a
newline, and no files. `run-folder <dir> --solver spps --solver-exe <stub>` plays it back.

A row needs a stub when the real case that prints it is refused before launch (its mesh fails
mesh::verify: scene_mesh_unreadable, tetra_mesh_unreadable, tetra_mesh_empty,
degenerate_tetrahedron, particle_loss_reported), cannot be launched through run-folder
(config_path_missing needs no argument), or has no real case at all (source_not_located,
unclassified_line). Every text is the solver's own: mkexpected.py checks it against a real
run's output or, where no run prints it, against the string literal in upstream's source.
"""
import json
import sys
from pathlib import Path

import fixture_common as fc

BANNER = "SPPS version 2.2.1"
END = ["End of calculation.", "Output results files."]
LOSS = (
    "Warning 4000 particles has been in error on 4000 particles. The computation result may be "
    "wrong, please check the particles statitics file for more details."
)
PATH_CBIN = "C:\\runs\\stub\\solve\\mesh.cbin"
PATH_MBIN = "C:\\runs\\stub\\solve\\tetramesh.mbin"
NO_MBIN = "Unable to read the tetrahedalization of the scene mesh file, calculation canceled."


def out(text: str, newline: bool = True) -> dict:
    return {"stream": "stdout", "text": text, "newline": newline, "delay_ms": 0}


def err(text: str, newline: bool = True) -> dict:
    return {"stream": "stderr", "text": text, "newline": newline, "delay_ms": 0}


# name: (exit code, lines). The row each one exists for is in its name.
STUBS = {
    # sppsNantes.cpp:285-289: no path argument; MainProcess returns 1 and main returns 0.
    "stub_config_path_missing": (
        0,
        [out(BANNER), out("The path of the XML configuration file must be specified!")],
    ),
    # coreTypes.cpp:213-215: fprintf without a newline, then exit(1). Text from spps_degenerate.
    "stub_degenerate_tetrahedron": (
        1,
        [
            out(BANNER),
            err(
                "Error in input mesh, a tetrahedra have at least the same two vertices "
                "idTetra:0 vertices:4 5 7 4",
                newline=False,
            ),
        ],
    ),
    # sppsNantes.cpp:57-65: cerr without a newline, once per band (2 here), so the two
    # messages share one stderr line; the run then completes. No real run reaches it: a source
    # outside the mesh crashes first (sppsInitialisation.cpp:13-20).
    "stub_source_not_located": (
        0,
        [
            out(BANNER),
            err("Unable to find the source position!", newline=False),
            err("Unable to find the source position!", newline=False),
        ]
        + [out(t) for t in END],
    ),
    # sppsNantes.cpp:425-439: the loss warning, fprintf without a newline, is the last thing
    # SPPS writes. Text from spps_lossy.
    "stub_particle_loss_unterminated": (
        0,
        [out(BANNER), out("#50"), out("#100")] + [out(t) for t in END] + [err(LOSS, newline=False)],
    ),
    # coreinitialisation.cpp:394-397: the message, then the path on the next line. Text from
    # spps_unreadable_mesh.
    "stub_scene_mesh_unreadable": (
        0,
        [out(BANNER), out("Unable to read the scene mesh file :"), out(PATH_CBIN)],
    ),
    # coreinitialisation.cpp:454-457. Text from spps_nomesh.
    "stub_tetra_mesh_unreadable": (0, [out(BANNER), out(NO_MBIN)]),
    # coreTypes.cpp:241: the path, then the message; initTetraMesh then adds its own line.
    # Text from spps_emptymesh.
    "stub_tetra_mesh_empty": (
        0,
        [
            out(BANNER),
            out(PATH_MBIN),
            out("Tetrahedron file is empty, the calculation can't be done !"),
            out(NO_MBIN),
        ],
    ),
    # A line no row matches. The text is SPPS's own, from a _DEBUG build only
    # (CalculationCore.cpp:367-369, where it has no newline); a release build never prints it.
    # Upstream's source holds U+FFFD where the accents were, so a debug build prints those.
    "stub_unclassified_line": (
        0,
        [
            out(BANNER),
            out(
                "La particule va sortir du perim�tre du volume car une face du domaine est "
                "mal orient�e ou le maillage est incorrect. La particule a �t� "
                "supprim�e"
            ),
        ]
        + [out(t) for t in END],
    ),
}


def main() -> None:
    if len(sys.argv) != 2:
        fc.die(__doc__.strip().splitlines()[2].strip())
    runs = Path(sys.argv[1])
    src = runs / "spps_ok"
    for name, (code, lines) in STUBS.items():
        d = fc.fresh_dir(runs / name)
        for f in ("config.xml", "mesh.cbin", "tetramesh.mbin"):
            fc.write_bytes(d / f, (src / f).read_bytes())
        stub = {"exit_code": code, "lines": lines, "files": {}}
        fc.write_text(d / fc.STUB_JSON, json.dumps(stub, indent=2, ensure_ascii=False) + "\n")
        fc.check_self_contained(d)
        print("wrote", name)


if __name__ == "__main__":
    main()
