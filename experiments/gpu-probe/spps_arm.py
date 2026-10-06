"""The SPPS arm of the probe: write box.simpa from the seats fixture with the probe's settings, run
`simpa run --solver spps`, time it, and print one JSON line. Imports nothing from the codebase; the
project file is edited as JSON.
usage: python spps_arm.py <particles> <steps> <out-dir> [--simpa <simpa.exe>] [--solvers <dir>]"""
import argparse, json, os, shutil, subprocess, sys, time
from pathlib import Path

ap = argparse.ArgumentParser()
ap.add_argument("particles", type=int)
ap.add_argument("steps", type=int)
ap.add_argument("out", type=Path)
ap.add_argument("--simpa", default=r"C:\tmp\nm-target\release\simpa.exe")
ap.add_argument("--solvers", default=r"C:\tmp\nm-solvers-timebin\bin")
ap.add_argument("--fixture", default=str(Path(__file__).resolve().parents[2] / "tests" / "fixtures" / "rooms" / "seats_box.simpa"))
a = ap.parse_args()

a.out.mkdir(parents=True, exist_ok=True)
p = json.load(open(a.fixture, encoding="utf-8"))
dt = 0.001
spps = p["solvers"]["spps"]
spps.update({
    "particles_per_source": a.particles,
    "particles_saved": 0,
    "duration_s": round(a.steps * dt, 6),
    "time_step_s": dt,
    "random_seed": 1,
    "method": "energetic",
    "air_absorption": False,
    "fittings": False,
    "direct_field_only": False,
    "transmission": False,
    "extinction_exponent": 5.0,
    "receiver_radius_m": 0.31,
    "echogram_per_source": False,
    "save_surface_intersections": False,
    "save_receiver_intersections": False,
    "bands_computed": [True, False],
})
p["solvers"]["tcr"]["bands_computed"] = [True, False]
p["surface_receivers"] = []          # no maps: both arms do transport and point receivers only
p["name"] = "GPU probe box"
proj = a.out / "box.simpa"
json.dump(p, open(proj, "w", encoding="utf-8"), indent=1)

env = dict(os.environ, SIMPA_SOLVERS_DIR=a.solvers)
runs = a.out / "runs"
t0 = time.perf_counter()
r = subprocess.run([a.simpa, "run", str(proj), "--solver", "spps", "--runs", str(runs), "--json"],
                   capture_output=True, text=True, env=env)
wall = time.perf_counter() - t0
log = a.out / "spps-stderr.log"
log.write_text(r.stderr, encoding="utf-8")
if r.returncode != 0:
    print(json.dumps({"error": f"simpa run exit {r.returncode}", "stderr_tail": r.stderr[-800:]}))
    sys.exit(r.returncode)
manifest = json.loads(r.stdout)
# The run folder: the newest under runs/ (simpa run makes one per run).
folders = sorted([d for d in runs.iterdir() if d.is_dir()], key=lambda d: d.stat().st_mtime)
folder = str(folders[-1]) if folders else ""
# The solver process's own wall time, run.json's outcome.elapsed_ms; else the subprocess wall
# (which includes export, mesh and the verdict).
solver_s = None
ms = (manifest.get("outcome") or {}).get("elapsed_ms")
if isinstance(ms, (int, float)):
    solver_s = ms / 1000.0
ps = a.particles * a.steps
print(json.dumps({
    "particles": a.particles, "steps": a.steps, "dt_s": dt, "run_folder": folder,
    "wall_s": round(wall, 3), "solver_s": solver_s,
    "particle_steps": ps, "rate_wall": ps / wall, "rate_solver": (ps / solver_s) if solver_s else None,
    "manifest_keys": sorted(manifest.keys())[:40],
}))
