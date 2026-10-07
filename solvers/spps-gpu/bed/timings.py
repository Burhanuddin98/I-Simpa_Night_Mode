"""spps-gpu bed (c): wall times of one case's runs, from run.json (simpa's measure of the solver
process) and, for spps-gpu, its own breakdown (spps-gpu.json).

usage: python timings.py <case> <bed-log.jsonl> [...]
"""
import json, os, sys

if __name__ == "__main__":
    case = sys.argv[1]
    for log in sys.argv[2:]:
        for line in open(log, encoding="utf-8"):
            r = json.loads(line)
            if r.get("case") != case or not r.get("run"):
                continue
            run = json.load(open(os.path.join(r["run"], "run.json"), encoding="utf-8"))
            out = {"arm": r["arm"], "seed": r["seed"], "elapsed_s": run["outcome"]["elapsed_ms"] / 1000.0,
                   "particles": [(b["freq_hz"], b["total"]) for b in run.get("particles", {}).get("bands", [])]}
            j = os.path.join(r["run"], "solve", "spps-gpu.json")
            if os.path.exists(j):
                g = json.load(open(j, encoding="utf-8"))
                out.update({k: g[k] for k in g if k.endswith("_seconds") or k in ("backend", "device", "gpu_slots", "gpu_launches")})
            print(json.dumps(out))
