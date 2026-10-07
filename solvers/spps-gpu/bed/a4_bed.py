"""A4 bed driver: arms (a) and (b) on the cases of a4_cases.py.

Sources: each case's solve folder from a meshing run (`simpa run <case>.simpa --solver spps --runs
.out/a4/mesh-runs/<case>`, 2,000 particles, SPPS), copied by stage.py, never run in place.

  python a4_bed.py a <root> [case ...]
      arm (a): spps-gpu --cpu against spps-gpu on the GPU, same config and seed (cpu_gpu.py), with
      SPPS_GPU_EXE = the manifest's pinned build. Appends to <root>/summary.jsonl.
  python a4_bed.py b <tag> <arms spps,gpu> <seeds> [case ...]
      arm (b): `simpa run-folder --device cpu` (spps.exe) or `--device gpu` (spps-gpu.exe), both
      the verified builds in $SIMPA_SOLVERS_DIR = C:/tmp/nm-solvers-a5, then `simpa results --json`.
      One JSON line per run in .out/a4/bed/bed-<tag>.jsonl (analyse.py reads it).
"""
import glob, json, os, subprocess, sys, time

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import stage  # noqa: E402

OUT = r"B:\repos\I-Simpa_Night_Mode\.out\a4"
SIMPA = r"C:\tmp\nm-target-a4\release\simpa.exe"
SOLVERS = r"C:\tmp\nm-solvers-a5"


def src(case):
    runs = sorted(glob.glob(os.path.join(OUT, "mesh-runs", case, "*", "solve")))
    if len(runs) != 1:
        sys.exit(f"{case}: expected one meshing run, found {runs}")
    return runs[0]


# case: (project, simulation attributes). Particle counts equal in every arm.
CASES = {
    "trans": ("trans", {"nbparticules": "200000"}),
    "trans-random": ("trans", {"nbparticules": "200000", "computation_method": "0"}),
    "trans-a1": ("trans-a1", {"nbparticules": "200000"}),
    "beam": ("beam", {"nbparticules": "2000"}),
    "onesided": ("onesided", {"nbparticules": "200000"}),
    "onesided-random": ("onesided", {"nbparticules": "200000", "computation_method": "0"}),
}


def arm_a(root, cases):
    os.makedirs(root, exist_ok=True)
    env = dict(os.environ, SPPS_GPU_EXE=os.path.join(SOLVERS, "spps-gpu.exe"))
    for case in cases:
        proj, attrs = CASES[case]
        cmd = [sys.executable, os.path.join(HERE, "cpu_gpu.py"), root, case, src(proj), "11"] + [f"{k}={v}" for k, v in attrs.items()]
        p = subprocess.run(cmd, capture_output=True, text=True, env=env)
        lines = [l for l in p.stdout.splitlines() if l.startswith("{")]
        with open(os.path.join(root, "summary.jsonl"), "a", encoding="utf-8") as fh:
            for l in lines:
                fh.write(l + "\n")
        print("\n".join(lines) if lines else (case + " FAILED " + p.stdout[-800:] + p.stderr[-1500:]), flush=True)


def arm_b(tag, arms, seeds, cases):
    bed = os.path.join(OUT, "bed")
    os.makedirs(bed, exist_ok=True)
    log = os.path.join(bed, f"bed-{tag}.jsonl")
    env = dict(os.environ, SIMPA_SOLVERS_DIR=SOLVERS)
    env.pop("SIMPA_SOLVER_MANIFEST", None)
    env.pop("SPPS_GPU_BACKEND", None)
    for case in cases:
        proj, attrs = CASES[case]
        for seed in seeds:
            for arm in arms:
                a = dict(attrs, random_seed=str(seed))
                tpl = os.path.join(bed, "templates", f"{case}-{arm}-s{seed}")
                if os.path.exists(tpl):
                    sys.exit(f"refused: template {tpl} exists")
                stage.stage(src(proj), tpl, a, template=True)
                runs = os.path.join(bed, "runs", case)
                os.makedirs(runs, exist_ok=True)
                device = "gpu" if arm == "gpu" else "cpu"
                t = time.perf_counter()
                p = subprocess.run([SIMPA, "run-folder", tpl, "--solver", "spps", "--device", device, "--runs", runs, "--json"],
                                   env=env, capture_output=True, text=True)
                wall = time.perf_counter() - t
                try:
                    man = json.loads(p.stdout)
                except Exception:
                    rec = {"case": case, "arm": arm, "seed": seed, "exit": p.returncode, "error": p.stdout[-1500:] + p.stderr[-1500:]}
                else:
                    run_dir = os.path.dirname(man["cwd"]) if man.get("cwd") else None
                    rec = {"case": case, "arm": arm, "seed": seed, "exit": p.returncode, "verdict": man.get("verdict"),
                           "elapsed_ms": man.get("outcome", {}).get("elapsed_ms"), "wall_s": wall, "run": run_dir}
                    if run_dir:
                        with open(os.path.join(run_dir, "run-folder.stderr.txt"), "w", encoding="utf-8") as fh:
                            fh.write(p.stderr)
                        r = subprocess.run([SIMPA, "results", run_dir, "--json"], capture_output=True, text=True)
                        with open(os.path.join(run_dir, "results.json"), "w", encoding="utf-8") as fh:
                            fh.write(r.stdout)
                        with open(os.path.join(run_dir, "results.stderr.txt"), "w", encoding="utf-8") as fh:
                            fh.write(r.stderr)
                        t2 = subprocess.run([SIMPA, "results", run_dir], capture_output=True, text=True)
                        with open(os.path.join(run_dir, "results.txt"), "w", encoding="utf-8") as fh:
                            fh.write(t2.stdout + t2.stderr)
                        rec["results_exit"] = r.returncode
                        js = os.path.join(run_dir, "solve", "spps-gpu.json")
                        if os.path.exists(js):
                            rec["child_queue_overflow"] = json.load(open(js, encoding="utf-8")).get("child_queue_overflow")
                with open(log, "a", encoding="utf-8") as fh:
                    fh.write(json.dumps(rec) + "\n")
                print(json.dumps(rec), flush=True)


if __name__ == "__main__":
    a = sys.argv[1:]
    if len(a) >= 2 and a[0] == "a":
        arm_a(a[1], a[2:] or list(CASES))
    elif len(a) >= 4 and a[0] == "b":
        arm_b(a[1], a[2].split(","), [int(s) for s in a[3].split(",")], a[4:] or list(CASES))
    else:
        sys.exit(__doc__)
