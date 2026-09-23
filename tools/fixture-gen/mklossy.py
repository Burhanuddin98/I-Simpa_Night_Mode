"""Write spps_lossy: spps_ok with every tetrahedron neighbour link cut.

    python tools/fixture-gen/mklossy.py <runs-dir>

Reads <runs-dir>/spps_ok (written by mkcfg.py) and replaces <runs-dir>/spps_lossy. Every
neighbour index >= 0 in tetramesh.mbin becomes -2, "no tetrahedron across" (docs/formats/mbin.md),
so a particle reaching an interior face finds neither a neighbour nor a model face and is lost.
config.xml and mesh.cbin are copied unchanged. Ported from the survey's scratch mklossy.py
(docs/rebuild-plan-raw-2026-09-23.json:1541, run_lossy).
"""
import sys
from pathlib import Path

import fixture_common as fc


def main() -> None:
    if len(sys.argv) != 2:
        fc.die(__doc__.strip().splitlines()[2].strip())
    runs = Path(sys.argv[1])
    src = runs / "spps_ok"
    dst = fc.fresh_dir(runs / "spps_lossy")
    for name in ("config.xml", "mesh.cbin"):
        fc.write_bytes(dst / name, (src / name).read_bytes())
    mbin, cut = fc.mbin_cut_links((src / "tetramesh.mbin").read_bytes())
    if cut == 0:
        fc.die("spps_ok's mesh has no neighbour link to cut")
    fc.write_bytes(dst / "tetramesh.mbin", mbin)
    fc.check_self_contained(dst)
    print(f"wrote spps_lossy: {cut} neighbour links cut")


if __name__ == "__main__":
    main()
