"""Build C: trans_epsilon (project field `extinction_exponent`) 5 -> 6/7, run-time cost and refusals.

Each run is round 2's tested-G?-energetic-1.0ms-150k-3101 project with only `extinction_exponent`
changed. Solvers are round 2's exact binaries (sha-checked by simpa run). 4 runs at a time.
Usage: python run_eps.py [outdir]
"""
import json, subprocess, sys, time, glob, os
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

SIMPA = r"C:\tmp\nm-target-b2\release\simpa.exe"
SOLV = r"C:\tmp\nm-m8a-solvers"
R2 = Path(r"B:\data\m8b-edt\round2\heldout")
OUT = Path(sys.argv[1] if len(sys.argv) > 1 else r"B:\data\m8b-t20\eps-cost")
ROOMS = ["G1", "G2", "G3", "G4", "G5", "G6", "G7"]
EPS = [5, 6, 7]
WORKERS = 4


def one(room, eps):
    rid = f"{room}-energetic-1.0ms-150k-3101-eps{eps}"
    d = OUT / rid
    if (d / "done.json").exists():
        return json.loads((d / "done.json").read_text())
    d.mkdir(parents=True, exist_ok=True)
    src = R2 / f"tested-{room}-energetic-1.0ms-150k-3101" / "project.simpa"
    p = json.loads(src.read_text(encoding="utf-8"))
    sp = p["solvers"]["spps"]
    assert sp["method"] == "energetic" and sp["random_seed"] == 3101 and sp["extinction_exponent"] == 5.0
    sp["extinction_exponent"] = float(eps)
    proj = d / "project.simpa"
    proj.write_text(json.dumps(p, indent=2), encoding="utf-8")
    cmd = [SIMPA, "run", str(proj), "--solver", "spps", "--runs", str(d), "--json",
           "--solver-exe", rf"{SOLV}\spps.exe", "--tetgen", rf"{SOLV}\tetgen.exe",
           "--preprocess", rf"{SOLV}\preprocess.exe"]
    t0 = time.time()
    r = subprocess.run(cmd, capture_output=True, text=True)
    wall = time.time() - t0
    (d / "simpa-run.stdout.json").write_text(r.stdout)
    (d / "simpa-run.stderr.txt").write_text(r.stderr)
    folders = sorted(x for x in d.iterdir() if x.is_dir() and x.name.endswith("-spps"))
    rep_exit = None
    if r.returncode == 0 and folders:
        q = subprocess.run([SIMPA, "results", str(folders[-1]), "--json"], capture_output=True, text=True)
        (d / "report.json").write_text(q.stdout)
        (d / "simpa-results.stderr.txt").write_text(q.stderr)
        rep_exit = q.returncode
    res = {"room": room, "eps": eps, "run_exit": r.returncode, "results_exit": rep_exit,
           "wall_s": round(wall, 2), "run_folder": str(folders[-1]) if folders else None,
           "finished": time.strftime("%H:%M:%S")}
    (d / "done.json").write_text(json.dumps(res, indent=1))
    print(json.dumps(res), flush=True)
    return res


if __name__ == "__main__":
    OUT.mkdir(parents=True, exist_ok=True)
    # Interleave epsilons so each batch of 4 mixes them: load is shared alike across eps.
    jobs = [(r, e) for r in ROOMS for e in EPS]
    with ThreadPoolExecutor(WORKERS) as ex:
        out = list(ex.map(lambda j: one(*j), jobs))
    (OUT / "summary-runs.json").write_text(json.dumps(out, indent=1))
    print("exit 0", flush=True)
