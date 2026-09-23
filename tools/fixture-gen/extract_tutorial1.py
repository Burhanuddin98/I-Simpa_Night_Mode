"""Extract upstream tutorial 1's solver inputs into tests/fixtures/upstream/tutorial1/.

Reads tutorial_1.proj (a zip) from the pinned upstream checkout; writes only into this repo.
Edits are deliberate and listed in PROVENANCE.md, which this script regenerates.
"""
import hashlib
import re
import sys
import zipfile
from pathlib import Path

UPSTREAM = Path(r"B:\repos\I-Simpa-upstream")
PROJ = UPSTREAM / "src/isimpa/resources/doc/tutorial/tutorial 1/tutorial_1.proj"
OUT = Path(__file__).resolve().parents[2] / "tests/fixtures/upstream/tutorial1"

SPPS_RUN = "instance2/report/SPPS/2019-06-07_11h58m41s/"
TCR_RUN = "instance2/report/Classical theory of reverberation/2019-06-07_11h57m58s/"
PARTICLES = 10000
SEED = 1


def sub_attr(xml: str, name: str, value: str) -> str:
    new, n = re.subn(rf'\b{name}="[^"]*"', f'{name}="{value}"', xml)
    if n == 0:
        sys.exit(f"attribute {name} not found")
    return new


def main() -> None:
    z = zipfile.ZipFile(PROJ)
    written = []

    def put(member: str, dest: Path, transform=None) -> None:
        data = z.read(member)
        if transform:
            data = transform(data.decode("utf-8")).encode("utf-8")
        dest.parent.mkdir(parents=True, exist_ok=True)
        dest.write_bytes(data)
        written.append((dest.relative_to(OUT).as_posix(), member, hashlib.sha256(data).hexdigest()))

    def spps_config(xml: str) -> str:
        xml = sub_attr(xml, "workingdirectory", "__RUNDIR__")
        xml = sub_attr(xml, "random_seed", str(SEED))
        return sub_attr(xml, "nbparticules", str(PARTICLES))

    def tcr_config(xml: str) -> str:
        return sub_attr(xml, "workingdirectory", "__RUNDIR__")

    for name in ("mesh.cbin", "tetramesh.mbin"):
        put(SPPS_RUN + name, OUT / "spps" / name)
        put(TCR_RUN + name, OUT / "tcr" / name)
    put(SPPS_RUN + "config.xml", OUT / "spps/config.xml", spps_config)
    put(TCR_RUN + "config.xml", OUT / "tcr/config.xml", tcr_config)
    put("instance2/temp/scene_mesh.poly", OUT / "tetgen/scene_mesh.poly")
    put("instance2/temp/scene_mesh.var", OUT / "tetgen/scene_mesh.var")
    # TetGen's output for that .poly, from which the GUI built tetramesh.mbin: the .mbin
    # builder's byte-identity test (crates/simpa-core/tests/mesh_mbin_parity.rs) reads them.
    for ext in ("node", "ele", "face", "neigh", "edge"):
        put(f"instance2/temp/scene_mesh.1.{ext}", OUT / f"tetgen/scene_mesh.1.{ext}")

    proj_sha = hashlib.sha256(PROJ.read_bytes()).hexdigest()
    lines = [
        "# Provenance: upstream tutorial 1",
        "",
        f"Source: `{PROJ.relative_to(UPSTREAM).as_posix()}` at upstream commit 929a5c8 "
        f"(sha256 `{proj_sha}`). Regenerate with `python tools/fixture-gen/extract_tutorial1.py`.",
        "",
        "Edits, all in config.xml:",
        "- `workingdirectory` becomes the placeholder `__RUNDIR__`; the gate writes the run folder in.",
        f"- SPPS `random_seed` 0 (unseeded) becomes {SEED}, so two builds can be compared exactly. "
        "A seed makes SPPS run single-threaded.",
        f"- SPPS `nbparticules` 150000 becomes {PARTICLES}, so the comparison runs in seconds.",
        "",
        "Meshes, the TetGen input and TetGen's output are unmodified. These are format and "
        "equivalence fixtures:",
        "the acoustic values they produce are not evidence of anything.",
        "",
        "| fixture | zip member | sha256 |",
        "|---|---|---|",
    ]
    lines += [f"| `{d}` | `{m}` | `{h[:16]}` |" for d, m, h in written]
    (OUT / "PROVENANCE.md").write_text("\n".join(lines) + "\n", encoding="utf-8")
    print(f"wrote {len(written)} fixtures to {OUT}")


if __name__ == "__main__":
    main()
