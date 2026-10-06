"""A1's SPPS arm: a solve folder copied from an existing CR4 run and edited to the probe's settings (one
band, one source, energetic, specular at that band, no air absorption, no fittings, no transmission, no
maps, extinction 1e-5, fixed seed), run by spps.exe directly IN THAT FOLDER, timed. Imports nothing from
the codebase.

SAFETY (the 19:30 incident, READER of HANDOFF-2026-10-06): SPPS writes its outputs to the ROOT element's
`workingdirectory`, not the folder it was started in. The copied config.xml still named the source run's
solve folder, so a first version of this script overwrote 23 result files of the CR4-27 run. The root's
attribute is now rewritten to the copy, and the script refuses to start unless it names the copy.

usage: python spps_cr4_arm.py <src-solve-dir> <freq-hz> <source> <particles> <duration_s> <out-dir>"""
import json, os, shutil, subprocess, sys, time
import xml.etree.ElementTree as ET
from pathlib import Path

src, freq, source, n, dur, out = Path(sys.argv[1]), int(sys.argv[2]), sys.argv[3], int(sys.argv[4]), float(sys.argv[5]), Path(sys.argv[6])
spps = Path(os.environ.get("SIMPA_SOLVERS_DIR", r"C:\tmp\nm-solvers-timebin\bin")) / "spps.exe"
folder = (out / "folder").resolve()
folder.mkdir(parents=True, exist_ok=True)
for f in ("mesh.cbin", "tetramesh.mbin"):
    shutil.copy2(src / f, folder / f)
tree = ET.parse(src / "config.xml")
root = tree.getroot()
root.set("workingdirectory", str(folder) + "\\")
sim = root.find("simulation")
for k, v in {"duree_simulation": f"{dur:g}", "pasdetemps": "0.001", "nbparticules": str(n), "nbparticules_rendu": "0",
             "random_seed": "1", "computation_method": "1", "abs_atmo_calc": "0", "enc_calc": "0", "trans_calc": "0",
             "trans_epsilon": "5", "output_recp_bysource": "0", "save_surface_intersection": "0",
             "save_receivers_intersection": "0", "direct_calc": "0"}.items():
    sim.set(k, v)
for bf in sim.find("freq_enum"):
    bf.set("docalc", "1" if int(float(bf.get("freq"))) == freq else "0")
sources = root.find("sources")
for s in list(sources):
    if s.get("name") != source:
        sources.remove(s)
rs = root.find("recepteurss")
if rs is not None:
    for c in list(rs):
        rs.remove(c)
for ts in root.iter("type_surface"):
    for bf in ts.iter("bfreq"):
        if int(float(bf.get("freq"))) == freq:
            bf.set("diffusion", "0")
cfg = folder / "config.xml"
tree.write(cfg, encoding="UTF-8", xml_declaration=True)
# Refuse unless the written file's working directory is the copy.
wd = ET.parse(cfg).getroot().get("workingdirectory", "").rstrip("\\/")
if Path(wd).resolve() != folder:
    print(json.dumps({"error": f"refused: workingdirectory {wd!r} is not the copy {str(folder)!r}"}))
    sys.exit(3)

t0 = time.perf_counter()
r = subprocess.run([str(spps), str(cfg)], capture_output=True, text=True, cwd=str(folder))
wall = time.perf_counter() - t0
(out / "spps-stdout.log").write_text(r.stdout, encoding="utf-8")
(out / "spps-stderr.log").write_text(r.stderr, encoding="utf-8")
written = sorted(str(p.relative_to(folder)) for p in folder.rglob("*") if p.is_file() and p.name not in ("config.xml", "mesh.cbin", "tetramesh.mbin"))
print(json.dumps({"particles": n, "duration_s": dur, "freq": freq, "source": source, "exit": r.returncode,
                  "spps_s": round(wall, 3), "s_per_particle": wall / n, "folder": str(folder),
                  "files_written": len(written), "first_files": written[:6]}))
