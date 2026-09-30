"""Round 1 re-judge: every numeric leaf of tcr[] that differs from the original report, in units in the
last place, and D's reference as the run's own (run_analytic_eyring_s) held to the original's
analytic_eyring_s. Reads only.

usage: py -3 r1_d_ulps.py <original report.json> <re-judged report.json>
"""
import sys as _s; _s.stdout.reconfigure(encoding="utf-8")
import json
import math
import struct
import sys
from collections import Counter

old = json.load(open(sys.argv[1], encoding="utf-8"))
new = json.load(open(sys.argv[2], encoding="utf-8"))


def ulps(a, b):
    ia = struct.unpack("<q", struct.pack("<d", a))[0]
    ib = struct.unpack("<q", struct.pack("<d", b))[0]
    if ia < 0:
        ia = -(1 << 63) - ia
    if ib < 0:
        ib = -(1 << 63) - ib
    return abs(ia - ib)


leaves = 0
diff = []


def walk(a, b, path):
    global leaves
    if isinstance(a, dict):
        for k in a:
            if k in b:
                walk(a[k], b[k], f"{path}.{k}")
    elif isinstance(a, list):
        for i, (x, y) in enumerate(zip(a, b)):
            walk(x, y, f"{path}[{i}]")
    elif isinstance(a, (int, float)) and not isinstance(a, bool) and isinstance(b, (int, float)):
        leaves += 1
        if a != b:
            diff.append((path, a, b))


walk(old["tcr"], new["tcr"], "tcr")
print(f"tcr numeric leaves: {leaves}; differing: {len(diff)}")
by_kind = Counter()
worst = {}
for p, a, b in diff:
    kind = p.rsplit(".", 1)[-1].split("[")[0] if "." in p else p
    kind = "sabine_against_analytic" if "sabine_against_analytic" in p else kind
    by_kind[kind] += 1
    u = ulps(float(a), float(b))
    rec = worst.get(kind)
    if rec is None or u > rec[0]:
        worst[kind] = (u, p, a, b, abs(a - b))
for k, n in by_kind.items():
    u, p, a, b, d = worst[k]
    print(f"  {k}: {n} leaves differ; largest {u} ulp, |d| {d:.3e} at {p}: {a!r} vs {b!r}")

# D's reference: the plan's (new analytic_eyring_s) against the run's own (original analytic_eyring_s,
# and new run_analytic_eyring_s).
rel_max = 0.0
u_max = 0
run_same = 0
bands = 0
verdicts_same = 0
dev_max = 0.0
for to, tn in zip(old["tcr"], new["tcr"]):
    assert to["id"] == tn["id"]
    if not to.get("d"):
        continue
    for bo, bn in zip(to["d"]["bands"], tn["d"]["bands"]):
        bands += 1
        if bn.get("run_analytic_eyring_s") == bo["analytic_eyring_s"]:
            run_same += 1
        rel = abs(bn["analytic_eyring_s"] - bo["analytic_eyring_s"]) / abs(bo["analytic_eyring_s"])
        rel_max = max(rel_max, rel)
        u_max = max(u_max, ulps(bn["analytic_eyring_s"], bo["analytic_eyring_s"]))
        verdicts_same += bo["verdict"] == bn["verdict"]
        dev_max = max(dev_max, abs(bn["deviation"]))
print(f"D bands: {bands}; the plan's reference against the run's own: largest relative difference {rel_max:.3e}, "
      f"largest {u_max} ulp")
print(f"  run_analytic_eyring_s equal to the original analytic_eyring_s: {run_same} of {bands}")
print(f"  D band verdicts equal: {verdicts_same} of {bands}; largest |deviation| now {dev_max:.3e} against the 0.5 % limit")
