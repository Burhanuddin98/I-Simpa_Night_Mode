"""A6 bed, cost: CR4 1 kHz, 2 x 300,000 particles, 10 s (A5's .out\\a5\\cr4-1k\\CR4-1k.simpa, copied), on the
GPU before (A5's pinned spps-gpu, its simpa) and after (A6's), interleaved before/after so both arms see the
same machine. One JSON line per run: solver elapsed (run.json), spps-gpu.json's kernel and trace seconds.

usage: python a6_cost.py <project.simpa> <out-dir> <runs per arm>
"""
import json, os, subprocess, sys, time

ARMS = {
    "before": (r"C:\tmp\nm-target-a5\release\simpa.exe", r"C:\tmp\nm-solvers-a5"),
    "after": (r"C:\tmp\nm-target-a6\release\simpa.exe", r"C:\tmp\nm-solvers-a6"),
}


def main(project, out, n):
    log = os.path.join(out, "cost.jsonl")
    for i in range(n):
        for arm, (simpa, solvers) in ARMS.items():
            env = dict(os.environ, SIMPA_SOLVERS_DIR=solvers)
            for k in ("SIMPA_SOLVER_MANIFEST", "SPPS_GPU_BACKEND", "SPPS_GPU_POOL_CAP"):
                env.pop(k, None)
            runs = os.path.join(out, "runs-" + arm)
            t = time.perf_counter()
            p = subprocess.run([simpa, "run", project, "--solver", "spps", "--device", "gpu", "--runs", runs, "--json"],
                               env=env, capture_output=True, text=True)
            wall = time.perf_counter() - t
            man = json.loads(p.stdout)
            run = os.path.dirname(man["cwd"])
            g = json.load(open(os.path.join(run, "solve", "spps-gpu.json"), encoding="utf-8"))
            stderr = open(os.path.join(run, "solver.stderr.txt"), encoding="utf-8", errors="replace").read()
            rec = {"arm": arm, "i": i, "exit": p.returncode, "verdict": man["verdict"]["status"], "run": run,
                   "solver_elapsed_s": man["outcome"]["elapsed_ms"] / 1000, "simpa_wall_s": wall,
                   "kernel_s": g["kernel_seconds"], "trace_s": g["trace_seconds"], "wall_s": g["wall_seconds"],
                   "launches": g["gpu_launches"], "version": g["version"], "pool_capacity": g.get("child_pool_capacity"),
                   "solver_stderr_bytes": len(stderr), "particles": man["particles"]["bands"][0]["total"]}
            with open(log, "a", encoding="utf-8") as fh:
                fh.write(json.dumps(rec) + "\n")
            print(json.dumps(rec), flush=True)


if __name__ == "__main__":
    if len(sys.argv) < 4:
        sys.exit(__doc__)
    main(sys.argv[1], sys.argv[2], int(sys.argv[3]))
