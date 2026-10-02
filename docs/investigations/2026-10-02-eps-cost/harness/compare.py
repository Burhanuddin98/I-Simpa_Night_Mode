"""Compare eps 5/6/7 runs: wall time, per-metric answered/refused counts by reason, value shifts.
Usage: python compare.py [outdir]  -> prints a table, writes compare.json
"""
import json, re, sys
from collections import Counter, defaultdict
from pathlib import Path

OUT = Path(sys.argv[1] if len(sys.argv) > 1 else r"B:\data\m8b-t20\eps-cost")
METRICS = ["edt_s", "t20_s", "t30_s", "ts_s", "c50_db", "c80_db", "d50", "spl_db"]


def why(ne):
    """Reason code: the token after the quantity in the message."""
    m = re.match(r"params_not_evaluable: [^:]+: ([a-z_]+)", ne.get("message", ""))
    return m.group(1) if m else ne.get("code", "?")


def cells(rep):
    out = {}
    for ri, rcv in enumerate(rep["spps"]["point_receivers"]):
        for bi, b in enumerate(rcv["bands"]):
            for m in METRICS:
                p = b.get("parameters", {}).get(m)
                if p is None:
                    continue
                key = (rcv.get("name", ri), b.get("freq_hz", bi), m)
                if "not_evaluable" in p:
                    out[key] = ("refused:" + why(p["not_evaluable"]), None, None, None)
                else:
                    out[key] = (p.get("status", "answered"), p.get("value"), p.get("lo"), p.get("hi"))
    return out


runs = {}
for d in sorted(OUT.iterdir()):
    f = d / "done.json"
    if not f.exists():
        continue
    done = json.loads(f.read_text())
    rep = json.loads((d / "report.json").read_text()) if (d / "report.json").exists() else None
    runs[(done["room"], done["eps"])] = (done, rep)

summary = {}
for room in sorted({k[0] for k in runs}):
    base = runs.get((room, 5))
    bc = cells(base[1]) if base and base[1] else {}
    for eps in (5, 6, 7):
        if (room, eps) not in runs:
            continue
        done, rep = runs[(room, eps)]
        if rep is None:
            print(room, eps, "NO REPORT", done)
            continue
        c = cells(rep)
        rem = [b["remaining"] for b in rep["spps"]["particles"]["bands"]]
        cnt = defaultdict(Counter)
        shifts = defaultdict(list)
        for k, (st, v, lo, hi) in c.items():
            cnt[k[2]][st.split(":")[0] if not st.startswith("refused") else st] += 1
            if eps != 5 and k in bc and bc[k][1] is not None and v is not None:
                shifts[k[2]].append(100 * (v - bc[k][1]) / bc[k][1])
        summary[f"{room}-eps{eps}"] = {
            "wall_s": done["wall_s"], "remaining": rem,
            "counts": {m: dict(cnt[m]) for m in METRICS if cnt[m]},
            "max_abs_shift_pct_vs_eps5": {m: round(max(map(abs, s)), 3) for m, s in shifts.items() if s},
        }
        s = summary[f"{room}-eps{eps}"]
        print(f"{room} eps{eps} wall {done['wall_s']:6.1f}s remaining {rem}")
        for m in ("edt_s", "t20_s", "t30_s"):
            print(f"    {m:6s} {s['counts'].get(m)}  shift {s['max_abs_shift_pct_vs_eps5'].get(m)}")
        allref = Counter(t[0] for t in c.values() if t[0].startswith("refused"))
        print(f"    all refusals: {dict(allref)}")

(OUT / "compare.json").write_text(json.dumps(summary, indent=1))


# Range check: eps 5 answered vs eps 7 (floor 100x lower). Outside = eps 5 shown range misses eps 7's value
# by more than eps 7's own sd (|v5-v7| > (hi5-v5) + 2.5*sd7 is a crude two-sided version).
print("\nRANGE CHECK eps5 answered vs eps7 answered (per metric: n both, n eps7 value outside eps5 [lo,hi], worst ok-cell shift %)")
for room in sorted({k[0] for k in runs}):
    if (room, 5) not in runs or (room, 7) not in runs or not runs[(room, 7)][1]:
        continue
    c5, c7 = cells(runs[(room, 5)][1]), cells(runs[(room, 7)][1])
    for m in METRICS:
        both = [(c5[k], c7[k]) for k in c5 if k[2] == m and k in c7 and c5[k][1] is not None and c7[k][1] is not None]
        if not both:
            continue
        out = [(a, b) for a, b in both if a[2] is not None and not (a[2] <= b[1] <= a[3])]
        okw = [abs(100 * (b[1] - a[1]) / a[1]) for a, b in both if a[0] == "ok"]
        print(f"  {room} {m:7s} both {len(both):3d}  eps7 outside eps5 range {len(out):3d} (of which eps5 ok: {sum(1 for a,_ in out if a[0]=='ok')})  worst ok shift {max(okw) if okw else None}")
