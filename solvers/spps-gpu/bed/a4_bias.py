"""A4 bed (b): the signed difference behind analyse.py's within/outside verdicts, per case, band and
parameter, so a small systematic shift shows even when every value stays inside its range.

For each receiver (summed echogram; a per-source echogram only when the case has two sources, since with
one source it repeats the sum), d = (a - b) / sqrt(ha^2 + hb^2) as analyse.py computes it. Reports the
mean and rms of d over receivers and both seeds for spps-gpu minus SPPS at equal seeds, and for the two
controls: SPPS s202 minus s101 and spps-gpu s202 minus s101. Pure noise gives a mean near 0 and an rms
near 0.4 (A2-REPORT.md, "No bias").

usage: python a4_bias.py <out.json> <bed-log.jsonl> [...]
"""
import json, math, sys, os
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from analyse import load_runs, params, PARAMS  # noqa: E402


def signed(A, B, keep):
    out = {}
    for k in A:
        if k not in B or not keep(k[0]):
            continue
        x, y = A[k], B[k]
        if x[0] == "v" and y[0] == "v":
            lim = math.hypot(x[2], y[2])
            if lim > 0:
                out.setdefault((k[1], k[2]), []).append((x[1] - y[1]) / lim)
    return out


def summary(groups):
    res = {}
    for (band, p), ds in sorted(groups.items()):
        n = len(ds)
        res[f"{band} {p}"] = {"n": n, "mean": sum(ds) / n, "rms": math.sqrt(sum(d * d for d in ds) / n)}
    return res


def main(out, logs):
    runs = load_runs(logs)
    cases = sorted({k[0] for k in runs})
    res = {}
    for case in cases:
        P = {(a, s): params(runs[(c, a, s)]["run"])[0] for (c, a, s) in runs if c == case}
        two = any("/" in k[0] for v in P.values() for k in v) and len({k[0].split("/")[1] for v in P.values() for k in v if "/" in k[0]}) > 1
        keep = (lambda lab: True) if two else (lambda lab: "/" not in lab)
        pairs = {"gpu - spps (equal seeds)": [(("gpu", s), ("spps", s)) for s in (101, 202)],
                 "spps s202 - s101": [(("spps", 202), ("spps", 101))],
                 "gpu s202 - s101": [(("gpu", 202), ("gpu", 101))]}
        cr = {}
        for name, prs in pairs.items():
            groups = {}
            for a, b in prs:
                if a in P and b in P:
                    for k, v in signed(P[a], P[b], keep).items():
                        groups.setdefault(k, []).extend(v)
            cr[name] = summary(groups)
        res[case] = cr
    with open(out, "w", encoding="utf-8") as fh:
        json.dump(res, fh, indent=1)
    for case, cr in res.items():
        print(f"### {case}")
        keys = list(cr["gpu - spps (equal seeds)"])
        print("| band param | n | gpu-spps mean, rms | spps-spps mean, rms | gpu-gpu mean, rms |")
        for k in keys:
            row = [k, str(cr["gpu - spps (equal seeds)"][k]["n"])]
            for name in pairs_order():
                v = cr[name].get(k)
                row.append(f"{v['mean']:+.2f}, {v['rms']:.2f}" if v else "-")
            print("| " + " | ".join(row) + " |")


def pairs_order():
    return ["gpu - spps (equal seeds)", "spps s202 - s101", "gpu s202 - s101"]


if __name__ == "__main__":
    if len(sys.argv) < 3:
        sys.exit(__doc__)
    main(sys.argv[1], sys.argv[2:])
