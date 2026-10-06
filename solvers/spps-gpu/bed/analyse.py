"""spps-gpu bed (b): spps-gpu's parameters against SPPS's, within the Monte-Carlo ranges `simpa results`
reports with each value.

For every case, receiver, band and parameter (SPL, EDT, T20, T30, C80), a pair of runs agrees when
|a - b| <= sqrt(ha^2 + hb^2), h being each value's own half-range (hi - lo) / 2 from `simpa results
--json` (2.5 Monte-Carlo standard deviations, or the method's own range for EDT). Pairs: spps-gpu
against SPPS at equal seeds, and, for scale, SPPS against SPPS and spps-gpu against spps-gpu (two seeds).
A parameter one run cannot evaluate is compared by its reason code.

usage: python analyse.py <out.json> <out.md> <bed-log.jsonl> [...]
"""
import json, math, os, sys

PARAMS = [("spl_db", "SPL dB"), ("edt_s", "EDT s"), ("t20_s", "T20 s"), ("t30_s", "T30 s"), ("c80_db", "C80 dB")]


def load_runs(logs):
    runs = {}
    for log in logs:
        for line in open(log, encoding="utf-8"):
            r = json.loads(line)
            if r.get("run") and r.get("exit") == 0:
                runs[(r["case"], r["arm"], r["seed"])] = r
    return runs


def params(run_dir):
    d = json.load(open(os.path.join(run_dir, "results.json"), encoding="utf-8"))
    out = {}
    entries = []
    if "spps" not in d:
        # simpa results refused the whole report: compared by its reason
        why = d.get("refused", {})
        out[("(report)", 0, "spl_db")] = ("ne", "refused:" + (why.get("code", "") if isinstance(why, dict) else str(why)), None, None)
        return out, {}, None
    for rp in d["spps"]["point_receivers"]:
        entries.append((rp["label"], rp["bands"]))
        # with output_recp_bysource, each source's own echogram (the sum refuses onset-relative values)
        for ps in rp.get("per_source") or []:
            entries.append((rp["label"] + "/" + ps["source"], ps["bands"]))
    for label, bands in entries:
        rp = {"label": label}
        for b in bands:
            for key, _ in PARAMS:
                p = b["parameters"].get(key)
                if p is None:
                    continue
                if "value" in p and p["value"] is not None:
                    lo, hi = p.get("lo"), p.get("hi")
                    h = (hi - lo) / 2 if lo is not None and hi is not None else (2.5 * p["mc_sd"] if p.get("mc_sd") else 0.0)
                    out[(rp["label"], b["freq_hz"], key)] = ("v", p["value"], h, p.get("status"))
                else:
                    ne = p.get("not_evaluable", {})
                    why = ne.get("error", {}).get("why", {}).get("why") or ne.get("code")
                    out[(rp["label"], b["freq_hz"], key)] = ("ne", why, None, None)
    stats = {}
    run_json = json.load(open(os.path.join(run_dir, "run.json"), encoding="utf-8"))
    for b in run_json.get("particles", {}).get("bands", []):
        stats[b["freq_hz"]] = b
    return out, stats, d.get("solver_build", {}).get("status")


def compare(a, b):
    rows = []
    for k in sorted(set(a) | set(b), key=lambda x: (x[0], x[1], [p for p, _ in PARAMS].index(x[2]))):
        x, y = a.get(k), b.get(k)
        if x is None or y is None:
            rows.append((k, x, y, "missing", None))
            continue
        if x[0] == "v" and y[0] == "v":
            d = abs(x[1] - y[1])
            lim = math.hypot(x[2], y[2])
            rows.append((k, x, y, "within" if d <= lim else "OUTSIDE", d / lim if lim > 0 else float("inf")))
        elif x[0] == "ne" and y[0] == "ne":
            rows.append((k, x, y, "both_ne" if x[1] == y[1] else "ne_differs", None))
        else:
            rows.append((k, x, y, "one_ne", None))
    return rows


def fmt(v):
    if v is None:
        return "-"
    if v[0] == "ne":
        return f"NE({v[1]})"
    return f"{v[1]:.3f}±{v[2]:.3f}" + ("w" if v[3] == "wide" else "")


def main(out_json, out_md, logs):
    runs = load_runs(logs)
    cases = sorted({k[0] for k in runs})
    summary = {"cases": {}}
    md = []
    for case in cases:
        seeds = sorted({k[2] for k in runs if k[0] == case})
        arms = {k[1] for k in runs if k[0] == case}
        res = {a: {s: params(runs[(case, a, s)]["run"]) for s in seeds if (case, a, s) in runs} for a in arms}
        pairs = []
        for s in seeds:
            if s in res.get("gpu", {}) and s in res.get("spps", {}):
                pairs.append((f"gpu s{s} vs spps s{s}", res["gpu"][s], res["spps"][s]))
        ss = sorted(res.get("spps", {}))
        if len(ss) >= 2:
            pairs.append((f"spps s{ss[0]} vs spps s{ss[1]}", res["spps"][ss[0]], res["spps"][ss[1]]))
        gs = sorted(res.get("gpu", {}))
        if len(gs) >= 2:
            pairs.append((f"gpu s{gs[0]} vs gpu s{gs[1]}", res["gpu"][gs[0]], res["gpu"][gs[1]]))
        if "cpu" in res and "spps" in res:
            for s in sorted(res["cpu"]):
                if s in res["spps"]:
                    pairs.append((f"cpu s{s} vs spps s{s}", res["cpu"][s], res["spps"][s]))
        cs = {"pairs": {}, "elapsed_ms": {f"{a} s{s}": runs[(case, a, s)].get("elapsed_ms") for (c, a, s) in runs if c == case},
              "solver_build": {f"{a} s{s}": res[a][s][2] for a in res for s in res[a]}}
        md.append(f"### {case}\n")
        for name, A, B in pairs:
            rows = compare(A[0], B[0])
            n_val = sum(1 for r in rows if r[3] in ("within", "OUTSIDE"))
            n_in = sum(1 for r in rows if r[3] == "within")
            worst = max([r[4] for r in rows if r[4] is not None] or [0])
            other = {}
            for r in rows:
                if r[3] not in ("within", "OUTSIDE"):
                    other[r[3]] = other.get(r[3], 0) + 1
            cs["pairs"][name] = {"compared": n_val, "within": n_in, "worst_ratio": worst, "other": other,
                                 "outside": [{"receiver": r[0][0], "band": r[0][1], "param": r[0][2], "a": fmt(r[1]), "b": fmt(r[2]), "ratio": r[4]} for r in rows if r[3] == "OUTSIDE"],
                                 "one_ne": [{"receiver": r[0][0], "band": r[0][1], "param": r[0][2], "a": fmt(r[1]), "b": fmt(r[2])} for r in rows if r[3] in ("one_ne", "ne_differs")]}
            md.append(f"- **{name}**: {n_in} of {n_val} values within the combined range (worst |d| / range {worst:.2f}); {other or 'no NE'}")
        first = next((p for p in pairs if p[0].startswith("gpu s")), None)
        if first:
            md.append("")
            md.append(f"| receiver | band | param | {first[0].split(' vs ')[0]} | {first[0].split(' vs ')[1]} | verdict |")
            md.append("|---|---|---|---|---|---|")
            for r in compare(first[1][0], first[2][0]):
                v = r[3] + (f" ({r[4]:.2f})" if r[4] is not None else "")
                md.append(f"| {r[0][0]} | {r[0][1]} | {r[0][2]} | {fmt(r[1])} | {fmt(r[2])} | {v} |")
            md.append("")
            md.append("| band | fates spps-gpu (atmo / surf / loop / lost / alive / total) | fates SPPS |")
            md.append("|---|---|---|")
            for f in sorted(first[1][1]):
                g, s_ = first[1][1][f], first[2][1].get(f, {})
                fm = lambda b: " / ".join(str(b.get(k, "-")) for k in ("absorbed_by_atmosphere", "absorbed_by_materials", "lost_by_infinite_loops", "lost_by_meshing_problems", "remaining", "total"))
                md.append(f"| {f} | {fm(g)} | {fm(s_)} |")
        md.append("")
        summary["cases"][case] = cs
    with open(out_json, "w", encoding="utf-8") as fh:
        json.dump(summary, fh, indent=1)
    with open(out_md, "w", encoding="utf-8") as fh:
        fh.write("\n".join(md) + "\n")
    print("\n".join(l for l in md if l.startswith("- ") or l.startswith("###")))


if __name__ == "__main__":
    if len(sys.argv) < 4:
        sys.exit(__doc__)
    main(sys.argv[1], sys.argv[2], sys.argv[3:])
