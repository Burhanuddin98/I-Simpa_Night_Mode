"""Set B solver runs (../PREREG.md): rooms G1-G7, tested at the new-project defaults and high-count truths.

Each project is round 2's tested-G?-energetic-1.0ms-150k-3101/project.simpa with only the SPPS settings below
changed. Solvers: round 2's exact binaries (sha-checked by `simpa run`). 4 runs at a time, truths first (longest).
Usage: python run_b.py [outdir]   (default B:\\data\\m8b-bed\\B); a finished run (done.json) is skipped.
--sti (../ADDENDUM-2-STI.md, set B7): the same rooms with the seven octaves 125 Hz-8 kHz (bands7.py: checked to
differ from the 6-band project by the added band only, then `simpa validate`d), the STI build (e298a31), truths at
seeds 9301-9302, tested at 4301-4303; default outdir B:\\data\\m8b-bed\\B7.
"""
import json, subprocess, sys, time
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

import bands7

STI = "--sti" in sys.argv
SIMPA = r"C:\tmp\nm-target-g\release\simpa.exe" if STI else r"C:\tmp\nm-target-f\release\simpa.exe"
SOLV = r"C:\tmp\nm-m8a-solvers"
R2 = Path(r"B:\data\m8b-edt\round2\heldout")
ARGS = [a for a in sys.argv[1:] if not a.startswith("--")]
OUT = Path(ARGS[0] if ARGS else (r"B:\data\m8b-bed\B7" if STI else r"B:\data\m8b-bed\B"))
ROOMS = [f"G{i}" for i in range(1, 8)]
TESTED = dict(particles_per_source=150_000, time_step_s=0.001, duration_s=10.0, extinction_exponent=7.0)
TRUTH = dict(particles_per_source=1_000_000, time_step_s=0.0005, duration_s=10.0, extinction_exponent=9.0)
WORKERS = 4


def one(kind, room, seed):
    rid = f"{kind}-{room}-{seed}"
    d = OUT / rid
    if (d / "done.json").exists():
        return json.loads((d / "done.json").read_text())
    d.mkdir(parents=True, exist_ok=True)
    p = json.loads((R2 / f"tested-{room}-energetic-1.0ms-150k-3101" / "project.simpa").read_text(encoding="utf-8"))
    if STI:
        p7 = bands7.to_seven(p)
        diff = bands7.check(p, p7)
        assert not diff, (room, diff)
        p = p7
    sp = p["solvers"]["spps"]
    sp.update(TESTED if kind == "tested" else TRUTH)
    sp["method"] = "energetic"
    sp["random_seed"] = seed
    proj = d / "project.simpa"
    proj.write_text(json.dumps(p, indent=2), encoding="utf-8")
    if STI:
        code, v = bands7.validate(SIMPA, proj)
        (d / "simpa-validate.json").write_text(json.dumps(v, indent=1) if not isinstance(v, str) else v)
        assert code == 0, (rid, v)
    cmd = [SIMPA, "run", str(proj), "--solver", "spps", "--runs", str(d), "--json",
           "--solver-exe", rf"{SOLV}\spps.exe", "--tetgen", rf"{SOLV}\tetgen.exe",
           "--preprocess", rf"{SOLV}\preprocess.exe"]
    t0 = time.time()
    r = subprocess.run(cmd, capture_output=True, text=True)
    (d / "simpa-run.stdout.json").write_text(r.stdout)
    (d / "simpa-run.stderr.txt").write_text(r.stderr)
    folders = sorted(x for x in d.iterdir() if x.is_dir() and x.name.endswith("-spps"))
    res = {"kind": kind, "room": room, "seed": seed, "run_exit": r.returncode, "wall_s": round(time.time() - t0, 1),
           "run_folder": str(folders[-1]) if folders else None, "finished": time.strftime("%H:%M:%S"),
           "settings": {k: sp[k] for k in ("method", "particles_per_source", "time_step_s", "duration_s",
                                           "extinction_exponent", "random_seed")}}
    (d / "done.json").write_text(json.dumps(res, indent=1))
    print(json.dumps({k: res[k] for k in ("kind", "room", "seed", "run_exit", "wall_s", "finished")}), flush=True)
    return res


if __name__ == "__main__":
    OUT.mkdir(parents=True, exist_ok=True)
    # --fresh: only tested runs at seeds 4201-4203 (the fresh draw after build F); truths reused.
    if STI:
        jobs = [("truth", r, s) for s in (9301, 9302) for r in ROOMS] + \
               [("tested", r, s) for s in (4301, 4302, 4303) for r in ROOMS]
    elif "--fresh" in sys.argv:
        jobs = [("tested", r, s) for s in (4201, 4202, 4203) for r in ROOMS]
    else:
        jobs = [("truth", r, s) for s in (9201, 9202) for r in ROOMS] + \
               [("tested", r, s) for s in (4101, 4102, 4103) for r in ROOMS]
    with ThreadPoolExecutor(WORKERS) as ex:
        out = list(ex.map(lambda j: one(*j), jobs))
    (OUT / "runs.json").write_text(json.dumps(out, indent=1))
    bad = [o for o in out if o["run_exit"] != 0]
    print(f"exit {len(bad)}", flush=True)
