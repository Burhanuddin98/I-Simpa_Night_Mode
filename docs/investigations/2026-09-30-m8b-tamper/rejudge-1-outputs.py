"""Round 1 re-judge: summary.json, decays/*.csv and decays/*.npz of the re-judged stamp against the
original M8a bed's. Reads only.

usage: py -3 r1_compare_outputs.py <original bed> <re-judged stamp>
"""
import sys as _s; _s.stdout.reconfigure(encoding="utf-8")
import hashlib
import json
import math
import os
import sys

import numpy as np

old_dir, new_dir = sys.argv[1], sys.argv[2]
old = json.load(open(os.path.join(old_dir, "summary.json"), encoding="utf-8"))
new = json.load(open(os.path.join(new_dir, "summary.json"), encoding="utf-8"))

print(f"summary.json: pass {old['pass']} vs {new['pass']}; failures {old['failures']} vs {new['failures']}; "
      f"exploratory {old['exploratory']} vs {new['exploratory']}")
print(f"  solvers_code_sha256 equal: {old['solvers_code_sha256'] == new['solvers_code_sha256']}")
new_report_sha = hashlib.sha256(open(os.path.join(new_dir, "report.json"), "rb").read()).hexdigest()
print(f"  new summary's report_sha256 is the new report.json's sha256: {new['report_sha256'] == new_report_sha}")
key = lambda r: (r["cell"], r["check"])
new_by = {}
for r in new["rows"]:
    assert key(r) not in new_by, key(r)
    new_by[key(r)] = r
old_keys = [key(r) for r in old["rows"]]
missing = [k for k in old_keys if k not in new_by]
maxd = 0.0
where = None
unequal = []
for r in old["rows"]:
    n = new_by.get(key(r))
    if n is None:
        continue
    for f in r:
        a, b = r[f], n.get(f)
        if isinstance(a, float) or isinstance(b, float):
            if a is None or b is None:
                if a != b:
                    unequal.append((key(r), f, a, b))
                continue
            d = 0.0 if (math.isnan(a) and math.isnan(b)) else abs(a - b)
            if d > maxd:
                maxd, where = d, (key(r), f, a, b)
            if d != 0:
                unequal.append((key(r), f, a, b))
        elif a != b:
            unequal.append((key(r), f, a, b))
added = [k for k in new_by if k not in set(old_keys)]
order_same = [k for k in [key(r) for r in new["rows"]] if k in set(old_keys)] == old_keys
print(f"  rows: {len(old['rows'])} original, {len(new['rows'])} re-judged; original rows missing: {len(missing)}; "
      f"original rows in the same order: {order_same}")
print(f"  original rows with any field unequal: {len({u[0] for u in unequal})}; largest numeric |difference| {maxd:.3e} {where or ''}")
for u in unequal[:20]:
    print("   ", u)
checks = {}
for k in added:
    checks.setdefault(k[1].split(" ")[0] + " " + k[1].split(" ")[1] if " " in k[1] else k[1], []).append(new_by[k])
print(f"  rows only in the re-judged summary: {len(added)}")
for name, rows in checks.items():
    vals = sorted({(r['value'], r['limit'], r['verdict'], r['gated']) for r in rows})
    print(f"    {name}: {len(rows)} rows; (value, limit, verdict, gated): {vals}")

# decays
od, nd = os.path.join(old_dir, "decays"), os.path.join(new_dir, "decays")
old_files = sorted(os.listdir(od))
new_files = sorted(os.listdir(nd))
print(f"\ndecays: {len(old_files)} original, {len(new_files)} re-judged; names equal: {old_files == new_files}")
sha = lambda p: hashlib.sha256(open(p, "rb").read()).hexdigest()
csv_diff = [f for f in old_files if f.endswith(".csv") and (f not in new_files or sha(os.path.join(od, f)) != sha(os.path.join(nd, f)))]
print(f"  csv: {sum(f.endswith('.csv') for f in old_files)}; not byte-identical: {csv_diff}")
npz_diff = []
npz_bytes_same = 0
arrays = 0
for f in old_files:
    if not f.endswith(".npz"):
        continue
    if f not in new_files:
        npz_diff.append((f, "missing"))
        continue
    if sha(os.path.join(od, f)) == sha(os.path.join(nd, f)):
        npz_bytes_same += 1
    a, b = np.load(os.path.join(od, f)), np.load(os.path.join(nd, f))
    if sorted(a.files) != sorted(b.files):
        npz_diff.append((f, f"keys {sorted(a.files)} vs {sorted(b.files)}"))
        continue
    for k in a.files:
        arrays += 1
        x, y = a[k], b[k]
        if x.dtype != y.dtype or x.shape != y.shape or not np.array_equal(x, y, equal_nan=x.dtype.kind in "fc"):
            npz_diff.append((f, k))
print(f"  npz: {sum(f.endswith('.npz') for f in old_files)}, {arrays} arrays; byte-identical files {npz_bytes_same}; "
      f"arrays not equal (dtype, shape, every value): {npz_diff[:20]}")
