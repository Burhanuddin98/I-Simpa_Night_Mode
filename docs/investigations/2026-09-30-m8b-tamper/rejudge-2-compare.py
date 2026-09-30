"""Round 2 re-judge: the re-judged report.json against another report, leaf by leaf, reading only.

Every leaf of both (objects by key, lists by index) is compared exactly: numbers as the JSON text
reads them (Python's float, which rounds correctly), strings and booleans as they are. A run's
`folder` is compared with its separators made one (`/` and `\\` spell the same path). `meta` and
`files` (the folder's own counts and times) are reported apart. A key in one report and not the
other is listed.

usage: py -3 rejudge-2-compare.py <report.json> <other report.json> [label]
"""
import sys as _s; _s.stdout.reconfigure(encoding="utf-8")
import json
import sys
from collections import Counter, defaultdict

new = json.load(open(sys.argv[1], encoding="utf-8"))
old = json.load(open(sys.argv[2], encoding="utf-8"))
label = sys.argv[3] if len(sys.argv) > 3 else sys.argv[2]

num_diffs, other_diffs, only_new, only_old = [], [], [], []
numbers = Counter()
APART = (".meta", ".files")


def walk(a, b, path):
    if isinstance(a, dict) and isinstance(b, dict):
        for k in a:
            if k in b:
                walk(a[k], b[k], f"{path}.{k}")
            else:
                only_new.append(f"{path}.{k}")
        for k in b:
            if k not in a:
                only_old.append(f"{path}.{k}")
        return
    if isinstance(a, list) and isinstance(b, list):
        if len(a) != len(b):
            other_diffs.append((path, f"list of {len(a)} against {len(b)}"))
        for i, (x, y) in enumerate(zip(a, b)):
            walk(x, y, f"{path}[{i}]")
        return
    if path.endswith(".folder") and isinstance(a, str) and isinstance(b, str):
        a, b = a.replace("/", "\\"), b.replace("/", "\\")
    is_num = lambda v: isinstance(v, (int, float)) and not isinstance(v, bool)
    if is_num(a) and is_num(b):
        numbers[path.split(".")[1].split("[")[0]] += 1
        if a != b:
            num_diffs.append((path, a, b))
    elif a != b or type(a) is not type(b):
        other_diffs.append((path, f"{a!r} against {b!r}"))


walk(new, old, "")
print(f"re-judged {sys.argv[1]}\nagainst {label}")
print(f"numbers compared, by section: {dict(sorted(numbers.items()))}")
inside = lambda p: not p.startswith(APART)
nd = [d for d in num_diffs if inside(d[0])]
od = [d for d in other_diffs if inside(d[0])]
worst = defaultdict(float)
for p, a, b in nd:
    top = p.split(".")[1].split("[")[0]
    worst[top] = max(worst[top], abs(a - b))
print(f"numbers that differ, outside meta and files: {len(nd)}; by section, the largest difference: {dict(worst)}")
for p, a, b in sorted(nd, key=lambda d: -abs(d[1] - d[2]))[:5]:
    print(f"  {p}: {a!r} against {b!r}")
print(f"other differences, outside meta and files: {len(od)}")
for p, what in od[:20]:
    print(f"  {p}: {what[:200]}")
print("meta and files:")
for p, a, b in [d for d in num_diffs if not inside(d[0])]:
    print(f"  {p}: {a!r} against {b!r}")
for p, what in [d for d in other_diffs if not inside(d[0])]:
    print(f"  {p}: {what[:200]}")
print(f"keys only in the re-judged report: {len(only_new)}; by name: {dict(Counter(p.split('.')[-1] for p in only_new))}")
print(f"keys only in the other: {len(only_old)}; by name: {dict(Counter(p.split('.')[-1] for p in only_old))}")
