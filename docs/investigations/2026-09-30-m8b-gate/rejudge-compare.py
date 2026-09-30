"""Compare the M8a bed's report.json with the one a -From rerun of the fixed gate wrote.

Reads both reports only. Prints: per-section largest numeric difference (absolute and relative),
every non-numeric difference (verdicts, codes, failures), and every key present in only one report.
"""
import json
import math
import sys
from collections import defaultdict

old_path, new_path = sys.argv[1], sys.argv[2]
old = json.load(open(old_path, encoding="utf-8"))
new = json.load(open(new_path, encoding="utf-8"))

# Paths whose values differ by construction between a run and a -From reread.
IGNORE_KEYS = {"meta", "files"}
PATH_KEYS = {"folder", "root", "from"}

num = defaultdict(lambda: (0.0, 0.0, None, 0))  # section -> (max abs, max rel, where, count)
other = []  # non-numeric differences
only_old, only_new = [], []


def section_of(path):
    # e.g. cells[3].a.bands[0].interval.mean -> "cells.a"; tcr[2].d... -> "tcr.d"
    parts = path.split(".")
    head = parts[0].split("[")[0]
    if head in ("cells", "tcr") and len(parts) > 1:
        return f"{head}.{parts[1].split('[')[0]}"
    return head


def walk(a, b, path):
    if isinstance(a, dict) and isinstance(b, dict):
        for k in a:
            p = f"{path}.{k}" if path else k
            if not path and k in IGNORE_KEYS:
                continue
            if k not in b:
                only_old.append(p)
                continue
            walk(a[k], b[k], p)
        for k in b:
            p = f"{path}.{k}" if path else k
            if not path and k in IGNORE_KEYS:
                continue
            if k not in a:
                only_new.append((p, b[k]))
        return
    if isinstance(a, list) and isinstance(b, list):
        if len(a) != len(b):
            other.append((path, f"length {len(a)} vs {len(b)}"))
        for i, (x, y) in enumerate(zip(a, b)):
            walk(x, y, f"{path}[{i}]")
        return
    key = path.rsplit(".", 1)[-1]
    if key in PATH_KEYS:
        if str(a).replace("/", "\\") != str(b).replace("/", "\\"):
            other.append((path, f"{a!r} vs {b!r}"))
        return
    if isinstance(a, bool) or isinstance(b, bool) or a is None or b is None or isinstance(a, str) or isinstance(b, str):
        if a != b:
            other.append((path, f"{a!r} vs {b!r}"))
        return
    if isinstance(a, (int, float)) and isinstance(b, (int, float)):
        s = section_of(path)
        mabs, mrel, where, n = num[s]
        if math.isnan(a) and math.isnan(b):
            d = 0.0
            r = 0.0
        else:
            d = abs(a - b)
            r = d / abs(a) if a != 0 else (0.0 if d == 0 else math.inf)
        if d > mabs:
            mabs, where = d, f"{path}: {a!r} vs {b!r}"
        num[s] = (mabs, max(mrel, r), where, n + 1)
        return
    if a != b:
        other.append((path, f"{a!r} vs {b!r}"))


walk(old, new, "")

print(f"old: {old_path}")
print(f"new: {new_path}")
print(f"pass: {old['pass']} vs {new['pass']}; failures: {len(old['failures'])} vs {len(new['failures'])}; "
      f"exploratory: {old['exploratory']} vs {new['exploratory']}; needs_extension: {old['needs_extension']} vs {new['needs_extension']}")
cv_old = [(c["id"], c["verdict"]) for c in old["cells"]]
cv_new = [(c["id"], c["verdict"]) for c in new["cells"]]
print(f"cell verdicts equal: {cv_old == cv_new} ({len(cv_old)} cells)")
tv_old = [(t["id"], t["verdict"], t["d"]["verdict"] if t.get("d") else None) for t in old["tcr"]]
tv_new = [(t["id"], t["verdict"], t["d"]["verdict"] if t.get("d") else None) for t in new["tcr"]]
print(f"tcr verdicts equal: {tv_old == tv_new} ({len(tv_old)} TCR runs)")
print(f"say_no n5 as_required: {old['say_no']['n5']['as_required']} vs {new['say_no']['n5']['as_required']}; "
      f"n6: {old['say_no']['n6']['as_required']} vs {new['say_no']['n6']['as_required']}")
print()
print("numeric leaves, per section: count, largest |difference|, largest relative difference, where")
overall = 0.0
for s in sorted(num):
    mabs, mrel, where, n = num[s]
    overall = max(overall, mabs)
    print(f"  {s:32s} n={n:6d}  max|d|={mabs:.3e}  max rel={mrel:.3e}  {where or '-'}")
print(f"  overall max|d| = {overall:.3e}")
print()
print(f"non-numeric differences: {len(other)}")
for p, d in other[:60]:
    print(f"  {p}: {d}")
print()
print(f"keys only in the old report: {len(only_old)}")
for p in only_old[:30]:
    print(f"  {p}")
kinds = defaultdict(list)
for p, v in only_new:
    kinds[p.rsplit(".", 1)[-1]].append((p, v))
print(f"keys only in the new report: {len(only_new)}, by name:")
for k, lst in sorted(kinds.items()):
    vals = defaultdict(int)
    for _, v in lst:
        vals[json.dumps(v)[:80]] += 1
    shown = ", ".join(f"{v} x{n}" for v, n in list(vals.items())[:4])
    print(f"  {k}: {len(lst)} (e.g. {lst[0][0]}); values: {shown}{' ...' if len(vals) > 4 else ''}")
