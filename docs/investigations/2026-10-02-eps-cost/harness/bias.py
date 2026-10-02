"""Floor bias vs Monte Carlo noise.
Pair A: eps5 seed 3101 vs eps7 seed 3101 (floor 100x lower; same seed, draws diverge after the first particle the floor changes).
Pair B (control): eps5 seed 3101 vs eps5 seed 3102 (different draws only).
Per metric over all rooms: n, mean signed shift % (other - eps5_3101), mean z, share |z| > 2.5,
where z = (v_other - v_5) / sqrt(sd_5^2 + sd_other^2). A floor bias shows as mean z away from 0 in A, not B.
Also: refusal counts per room and eps, and wall-time ratios. Writes bias.json.
"""
import json, math, re
from collections import Counter, defaultdict
from pathlib import Path

OUT = Path(r"B:\data\m8b-t20\eps-cost")
METRICS = ["edt_s", "t20_s", "t30_s", "ts_s", "c50_db", "c80_db", "d50", "spl_db"]
ROOMS = [f"G{i}" for i in range(1, 8)]


def why(ne):
    m = re.match(r"params_not_evaluable: [^:]+: ([a-z_]+)", ne.get("message", ""))
    return m.group(1) if m else ne.get("code", "?")


def cells(path):
    rep = json.loads(Path(path).read_text())
    out = {}
    for ri, rcv in enumerate(rep["spps"]["point_receivers"]):
        for bi, b in enumerate(rcv["bands"]):
            for m, p in b.get("parameters", {}).items():
                if m not in METRICS:
                    continue
                key = (rcv.get("name", ri), b.get("freq_hz", bi), m)
                if "not_evaluable" in p:
                    out[key] = {"st": "refused:" + why(p["not_evaluable"])}
                else:
                    out[key] = {"st": p.get("status", "answered"), "v": p["value"], "sd": p.get("mc_sd")}
    return out


def rep(room, eps, seed=3101):
    if seed == 3101:
        return OUT / f"{room}-energetic-1.0ms-150k-3101-eps{eps}" / "report.json"
    return OUT / "control" / f"{room}-energetic-1.0ms-150k-{seed}-eps5" / "report.json"


def pair(a, b):
    stats = defaultdict(list)
    for k, x in a.items():
        y = b.get(k)
        if not y or "v" not in x or "v" not in y or not x["v"]:
            continue
        sh = 100 * (y["v"] - x["v"]) / abs(x["v"])
        sd = math.hypot(x["sd"] or 0, y["sd"] or 0)
        z = (y["v"] - x["v"]) / sd if sd > 0 else float("nan")
        stats[k[2]].append((sh, z))
    return stats


res = {"pairA_eps7": defaultdict(list), "pairB_seed": defaultdict(list)}
refusals, walls = {}, {}
for room in ROOMS:
    c5, c7, cB = cells(rep(room, 5)), cells(rep(room, 7)), cells(rep(room, 5, 3102))
    for m, v in pair(c5, c7).items():
        res["pairA_eps7"][m] += v
    for m, v in pair(c5, cB).items():
        res["pairB_seed"][m] += v
    for eps in (5, 6, 7):
        c = cells(rep(room, eps))
        refusals[f"{room} eps{eps}"] = dict(Counter(f"{k[2]}:{x['st'][8:]}" for k, x in c.items() if x["st"].startswith("refused")))
        walls[f"{room} eps{eps}"] = json.loads((OUT / f"{room}-energetic-1.0ms-150k-3101-eps{eps}" / "done.json").read_text())["wall_s"]
    refusals[f"{room} eps5 seed3102"] = dict(Counter(f"{k[2]}:{x['st'][8:]}" for k, x in cB.items() if x["st"].startswith("refused")))

summary = {}
print("metric   | pair A eps5->eps7 (same seed)          | pair B eps5 seed 3101->3102 (control)")
print("         |   n  mean%   mean z  |z|>2.5           |   n  mean%   mean z  |z|>2.5")
for m in METRICS:
    row = []
    for p in ("pairA_eps7", "pairB_seed"):
        v = res[p][m]
        zs = [z for _, z in v if z == z]
        r = {"n": len(v), "mean_shift_pct": sum(s for s, _ in v) / len(v) if v else None,
             "mean_z": sum(zs) / len(zs) if zs else None,
             "frac_absz_gt_2p5": sum(abs(z) > 2.5 for z in zs) / len(zs) if zs else None}
        summary[f"{p}:{m}"] = r
        row.append(r)
    f = lambda r: f"{r['n']:4d} {r['mean_shift_pct'] or 0:+6.2f} {r['mean_z'] or 0:+7.2f}   {r['frac_absz_gt_2p5'] or 0:5.1%}"
    print(f"{m:8s} | {f(row[0])}            | {f(row[1])}")

print("\nWALL TIME (s, 4 runs at a time) and ratio to eps5")
for room in ROOMS:
    w = [walls[f"{room} eps{e}"] for e in (5, 6, 7)]
    print(f"  {room}: {w[0]:5.1f} {w[1]:5.1f} {w[2]:5.1f}   x{w[1]/w[0]:.2f} x{w[2]/w[0]:.2f}")
tot = [sum(walls[f"{r} eps{e}"] for r in ROOMS) for e in (5, 6, 7)]
print(f"  all: {tot[0]:.1f} {tot[1]:.1f} {tot[2]:.1f}   x{tot[1]/tot[0]:.2f} x{tot[2]/tot[0]:.2f}")

print("\nREFUSALS by metric:reason (EDT..SPL only)")
for k, v in refusals.items():
    print(f"  {k:18s} {dict(sorted(v.items()))}")

(OUT / "bias.json").write_text(json.dumps({"summary": summary, "walls": walls, "refusals": refusals}, indent=1))
