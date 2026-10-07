"""A6 bed: the omni `trans` case after the child pool. For every run of the case in the bed logs, the
particles run per band (run.json) and the decay values of the room-1 receivers (simpa results), then
the sign test A4 failed: is every spps-gpu value below every SPPS value?

usage: python a6_trans.py <out.json> <bed-log.jsonl> [...]
"""
import json, os, sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
from analyse import params  # noqa: E402

ROOM1 = ["R1a", "R1b", "R1c", "R1d"]
ROOM2 = ["R2a", "R2b", "R2c", "R2d"]


def main(out, logs, case="trans"):
    runs = {}
    for log in logs:
        for line in open(log, encoding="utf-8"):
            r = json.loads(line)
            if r.get("case") == case and r.get("run") and r.get("exit") == 0:
                runs[(r["arm"], r["seed"])] = r
    res = {"case": case, "runs": {}, "decay": {}, "sign_test": {}}
    vals = {}
    for (arm, seed), r in sorted(runs.items()):
        p, stats, build = params(r["run"])
        res["runs"][f"{arm} s{seed}"] = {"run": r["run"], "verdict": r.get("verdict"), "solver_build": build,
                                         "particles": {f: s["total"] for f, s in stats.items()},
                                         "child_pool_given": r.get("child_pool_given"), "child_pool_peak": r.get("child_pool_peak"),
                                         "child_pool_capacity": r.get("child_pool_capacity"),
                                         "child_queue_overflow": r.get("child_queue_overflow"), "version": r.get("version")}
        for (label, f, key), v in p.items():
            if v[0] == "v" and label in ROOM1 + ROOM2:
                vals.setdefault((label, f, key), {})[(arm, seed)] = (v[1], v[2])
    for (label, f, key), d in sorted(vals.items()):
        res["decay"][f"{label} {f} {key}"] = {f"{a} s{s}": {"value": v, "half_range": h} for (a, s), (v, h) in sorted(d.items())}
    for f in (500, 1000):
        for key in ("edt_s", "t20_s", "t30_s"):
            for room, rx in (("room1", ROOM1), ("room2", ROOM2)):
                g = [vals[(l, f, key)][k][0] for l in rx if (l, f, key) in vals for k in vals[(l, f, key)] if k[0] == "gpu"]
                s = [vals[(l, f, key)][k][0] for l in rx if (l, f, key) in vals for k in vals[(l, f, key)] if k[0] == "spps"]
                if not g or not s:
                    continue
                res["sign_test"][f"{room} {f} {key}"] = {
                    "n_gpu": len(g), "n_spps": len(s), "gpu_mean": sum(g) / len(g), "spps_mean": sum(s) / len(s),
                    "gpu_over_spps_minus_1": (sum(g) / len(g)) / (sum(s) / len(s)) - 1,
                    "every_gpu_below_every_spps": max(g) < min(s), "every_gpu_above_every_spps": min(g) > max(s),
                    "gpu_below_spps_mean": sum(1 for x in g if x < sum(s) / len(s))}
    with open(out, "w", encoding="utf-8") as fh:
        json.dump(res, fh, indent=1)
    for k, v in res["runs"].items():
        print(k, v["particles"], v["verdict"], v["child_pool_given"], v["child_pool_peak"], v["child_pool_capacity"], v["child_queue_overflow"])
    for k, v in res["sign_test"].items():
        print(k, json.dumps(v))


if __name__ == "__main__":
    if len(sys.argv) < 3:
        sys.exit(__doc__)
    main(sys.argv[1], sys.argv[2:])
