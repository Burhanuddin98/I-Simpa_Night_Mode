"""Write the TCR run-folder fixtures.

    python tools/fixture-gen/mktcr.py <runs-dir> [--broken-hall <dir>]

From the SPPS cases mkcfg.py wrote (same config.xml, which TCR reads too):
- tcr_ok, tcr_mat7miss, tcr_srcout: copies of spps_ok, spps_mat7miss, spps_srcout;
- tcr_nomesh: spps_ok without tetramesh.mbin.

With --broken-hall, tcr_broken_hall is rebuilt from <dir>, Night Mode's TCR folder of
2026-09-08 (build-clean/sim_output/tcr/, gitignored and on one machine only): its config.xml,
model.cbin and tetramesh.mbin, with the absolute workingdirectory replaced by the placeholder.
Without it, an existing tcr_broken_hall is left as it is.

Ported from the survey's scratch mktcr.py (docs/rebuild-plan-raw-2026-09-23.json:1267).
"""
import argparse
from pathlib import Path

import fixture_common as fc

FROM_SPPS = (
    ("spps_ok", "tcr_ok", None),
    ("spps_mat7miss", "tcr_mat7miss", None),
    ("spps_srcout", "tcr_srcout", None),
    ("spps_ok", "tcr_nomesh", "tetramesh.mbin"),
)
BROKEN_HALL_FILES = ("config.xml", "model.cbin", "tetramesh.mbin")


def broken_hall(src: Path, dst: Path) -> None:
    xml = (src / "config.xml").read_bytes().decode("utf-8")
    old = fc.attr(xml, "workingdirectory")
    if not (len(old) > 2 and old[1] == ":"):
        fc.die(f"{src}: workingdirectory {old!r} is not an absolute path")
    xml = fc.sub_attr(xml, "workingdirectory", fc.PLACEHOLDER)
    d = fc.fresh_dir(dst)
    fc.write_text(d / "config.xml", xml)
    for name in BROKEN_HALL_FILES[1:]:
        fc.write_bytes(d / name, (src / name).read_bytes())
    fc.check_self_contained(d)
    print(f"wrote {dst.name} from {src} (workingdirectory was {old!r})")


def main() -> None:
    ap = argparse.ArgumentParser(description="Write the TCR run-folder fixtures.")
    ap.add_argument("runs", type=Path)
    ap.add_argument("--broken-hall", type=Path)
    a = ap.parse_args()
    for src, dst, drop in FROM_SPPS:
        s, d = a.runs / src, fc.fresh_dir(a.runs / dst)
        for name in ("config.xml", "mesh.cbin", "tetramesh.mbin"):
            if name != drop:
                fc.write_bytes(d / name, (s / name).read_bytes())
        fc.check_self_contained(d)
        print("wrote", dst)
    if a.broken_hall:
        broken_hall(a.broken_hall, a.runs / "tcr_broken_hall")
    else:
        print("tcr_broken_hall left as it is (no --broken-hall)")


if __name__ == "__main__":
    main()
