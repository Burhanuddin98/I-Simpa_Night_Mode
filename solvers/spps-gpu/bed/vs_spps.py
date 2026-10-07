"""spps-gpu bed (b): spps-gpu against upstream SPPS, through `simpa run-folder` and `simpa results`.

Each case is a solve folder staged as a run-folder template (workingdirectory "__RUNDIR__", which
`simpa run-folder` replaces with the fresh run's own solve folder), run by each arm into a runs root
under B:\\repos\\I-Simpa_Night_Mode\\.out\\spps-gpu\\bed. Upstream SPPS is the verified build
(C:\\tmp\\nm-solvers-timebin\\bin\\spps.exe, checked against the embedded solvers/manifest.json); spps-gpu
runs against a $SIMPA_SOLVER_MANIFEST override holding its own code sha256, so its runs read
"unverified (override)", as they must.

usage: python vs_spps.py <case> <src-solve-dir> <arm: spps|gpu|cpu> <seed> [attr=value ...]
Prints the run folder and writes <run>/results.json (simpa results --json).
"""
import json, os, subprocess, sys, time
sys.path.insert(0, os.path.dirname(__file__))
import stage  # noqa: E402

# A6 re-runs it on its own tree and builds: BED_REPO, BED_OUT, BED_SIMPA, BED_SPPS, BED_GPU
REPO = os.environ.get("BED_REPO", r"B:\repos\I-Simpa_Night_Mode\.claude\worktrees\spps-gpu")
OUT = os.environ.get("BED_OUT", r"B:\repos\I-Simpa_Night_Mode\.out\spps-gpu\bed")
SIMPA = os.environ.get("BED_SIMPA", r"C:\tmp\nm-target\release\simpa.exe")
SPPS = os.environ.get("BED_SPPS", r"C:\tmp\nm-solvers-timebin\bin\spps.exe")
GPU = os.environ.get("BED_GPU", r"C:\tmp\nm-spps-gpu\bin\spps-gpu.exe")


def override_manifest():
    sys.path.insert(0, os.path.join(REPO, "tools", "fixture-gen"))
    import pe_fingerprint
    from pathlib import Path
    with open(os.path.join(REPO, "solvers", "manifest.json"), encoding="utf-8-sig") as fh:
        m = json.load(fh)
    m["code_sha256"]["spps.exe"] = pe_fingerprint.code_sha256(Path(GPU))
    p = os.path.join(OUT, "manifest-spps-gpu.json")
    os.makedirs(OUT, exist_ok=True)
    with open(p, "w", encoding="utf-8") as fh:
        json.dump(m, fh, indent=1)
    return p, m["code_sha256"]["spps.exe"]


def main(case, src, arm, seed, attrs):
    attrs = dict(attrs)
    attrs["random_seed"] = str(seed)
    tpl = os.path.join(OUT, "templates", f"{case}-{arm}-s{seed}")
    if os.path.exists(tpl):
        sys.exit(f"refused: template {tpl} exists")
    stage.stage(src, tpl, attrs, template=True)
    runs = os.path.join(OUT, "runs", case)
    os.makedirs(runs, exist_ok=True)
    env = dict(os.environ)
    if arm == "spps":
        exe = SPPS
        env["SIMPA_SOLVERS_DIR"] = os.path.dirname(SPPS)
        env.pop("SIMPA_SOLVER_MANIFEST", None)
    else:
        exe = GPU
        env["SIMPA_SOLVER_MANIFEST"], code = override_manifest()
        if arm == "cpu":
            env["SPPS_GPU_BACKEND"] = "cpu"
    t = time.perf_counter()
    p = subprocess.run([SIMPA, "run-folder", tpl, "--solver", "spps", "--runs", runs, "--solver-exe", exe, "--json"],
                       env=env, capture_output=True, text=True)
    wall = time.perf_counter() - t
    try:
        man = json.loads(p.stdout)
    except Exception:
        print(p.stdout[-2000:], p.stderr[-3000:])
        sys.exit(f"run-folder gave no manifest, exit {p.returncode}")
    run_dir = os.path.dirname(man["cwd"]) if man.get("cwd") else None
    print(json.dumps({"case": case, "arm": arm, "seed": seed, "exit": p.returncode, "verdict": man.get("verdict"),
                      "elapsed_ms": man.get("outcome", {}).get("elapsed_ms"), "wall_s": wall, "run": run_dir}))
    if run_dir:
        r = subprocess.run([SIMPA, "results", run_dir, "--json"], capture_output=True, text=True)
        with open(os.path.join(run_dir, "results.json"), "w", encoding="utf-8") as fh:
            fh.write(r.stdout)
        with open(os.path.join(run_dir, "results.stderr.txt"), "w", encoding="utf-8") as fh:
            fh.write(r.stderr)
        t = subprocess.run([SIMPA, "results", run_dir], capture_output=True, text=True)
        with open(os.path.join(run_dir, "results.txt"), "w", encoding="utf-8") as fh:
            fh.write(t.stdout + t.stderr)
        print(json.dumps({"results_exit": r.returncode}))


if __name__ == "__main__":
    a = sys.argv[1:]
    if len(a) < 4:
        sys.exit(__doc__)
    main(a[0], a[1], a[2], int(a[3]), [x.split("=", 1) for x in a[4:]])
