"""Read-only probe of the M8a bed's report.json: for every group of seeds (40 cells and the
atmospheric validation), how many receiver-bands two seeds read the same T30 at (the value, its
mc_sd and its source, compared exactly), against how many both have a value."""
import json, itertools, sys
REPORT = r"C:\tmp\nm-m8a-bed\20260929T093134Z\report.json"
rep = json.load(open(REPORT, encoding="utf-8"), parse_float=lambda s: s)  # keep the text: exact
def key(t):
    return (t.get("t"), t.get("mc_sd"), t.get("source"))
groups = {}
for c in rep["cells"]:
    groups[c["id"]] = {s["seed"]: s.get("t30") or [] for s in c["seeds"]}
atm = rep.get("atmospheric_validation") or {}
aseeds = atm.get("runs") or []
groups["atmospheric-validation"] = {s["seed"]: s.get("t30") or [] for s in aseeds}
worst = (0, None)
pairs = 0
whole_equal = 0
single_equal_values = 0
for g, seeds in groups.items():
    for (a, ta), (b, tb) in itertools.combinations(sorted(seeds.items()), 2):
        pairs += 1
        both = same = 0
        for ra, rb in zip(ta, tb):
            for x, y in zip(ra, rb):
                if x.get("t") is not None and y.get("t") is not None:
                    both += 1
                    if key(x) == key(y):
                        same += 1
                    if x.get("t") == y.get("t"):
                        single_equal_values += 1
        if ta == tb and both:
            whole_equal += 1
        if same > worst[0]:
            worst = (same, (g, a, b, both))
print(f"groups {len(groups)}, seed pairs {pairs}")
print(f"seeds per group: {sorted(set(len(s) for s in groups.values()))}")
print(f"receiver-band values per seed: {sorted(set(sum(len(r) for r in t) for s in groups.values() for t in s.values()))}")
print(f"pairs whose whole T30 arrays are equal: {whole_equal}")
print(f"receiver-bands where two seeds' T30 values are equal (text of the float): {single_equal_values}")
print(f"largest number of equal (t, mc_sd, source) in one pair: {worst}")
