"""A6 audit fix 2, the bed row: a run that overflows the child pool in band 3 of 5 must end FAIL with no
readable result file left. Builds a run-folder template from A4's `trans` solve folder with five bands,
125, 250, 500, 1000 and 2000 Hz. The panel transmits only from 500 Hz (TL 10, 20, 20 dB), so bands 1
and 2 make no child and write their files, and band 3 is the first that can spill. It adds a cutting
plane, saved particles and per-band output, so the earlier bands leave files to be removed.

usage: python a6_band3.py <a4-trans-solve-dir> <template-dir>
Run it with `simpa run-folder <template> --solver spps --device gpu` and SPPS_GPU_POOL_CAP=1000.
"""
import copy, os, sys
import xml.etree.ElementTree as ET

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import stage  # noqa: E402

BANDS = {"125": ("500", False), "250": ("500", False), "500": ("500", True), "1000": ("1000", True), "2000": ("1000", True)}


def main(src, dest):
    stage.stage(src, dest, {"nbparticules": "2000", "nbparticules_rendu": "10", "random_seed": "101", "output_recs_byfreq": "1"},
                template=True)
    cfg = os.path.join(dest, "config.xml")
    tree = ET.parse(cfg)
    for parent in list(tree.getroot().iter()):
        kids = [k for k in parent if k.tag == "bfreq"]
        if not kids:
            continue
        by = {k.get("freq"): k for k in kids}
        for k in kids:
            parent.remove(k)
        for f, (from_f, transmit) in BANDS.items():
            k = copy.deepcopy(by[from_f])
            k.set("freq", f)
            if not transmit and "affaiblissement" in k.attrib:
                del k.attrib["affaiblissement"]
            if k.get("affaiblissement") is not None and f == "500":
                k.set("affaiblissement", "10")
            parent.append(k)
    rs = tree.getroot().find("recepteurss")
    ET.SubElement(rs, "recepteur_surfacique_coupe", {"id": "1", "name": "Cut", "ax": "0.5", "ay": "4.5", "az": "1.2",
                                                     "bx": "0.5", "by": "0.5", "bz": "1.2", "cx": "5.5", "cy": "0.5",
                                                     "cz": "1.2", "resolution": "1"})
    tree.write(cfg, encoding="UTF-8", xml_declaration=True)
    stage.check(dest, template=True)
    print(f"band-3 template {dest}")


if __name__ == "__main__":
    if len(sys.argv) < 3:
        sys.exit(__doc__)
    main(sys.argv[1], sys.argv[2])
