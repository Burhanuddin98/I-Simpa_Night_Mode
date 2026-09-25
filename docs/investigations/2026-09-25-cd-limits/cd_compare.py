"""Compares cd_before.json and cd_after.json (cd_limits.py): per group (tutorial 1 by method,
the calibration cells by method and cell) and quantity, receiver-bands with a value before and
after, and what the newly given ones were refused for before. Also checks that nothing else
changed: every quantity other than C50, C80 and D50 must read the same, and no value given
before may be refused after.

    python cd_compare.py > cd_compare.txt
"""
import json
from collections import Counter, defaultdict

B = json.load(open("cd_before.json", encoding="utf-8"))["runs"]
A = json.load(open("cd_after.json", encoding="utf-8"))["runs"]
CD = ["c50_db", "c80_db", "d50"]
OTHER = ["ts_s", "edt_s", "t20_s", "t30_s", "spl_db"]

assert B.keys() == A.keys(), "not the same runs"


def group(label: str) -> str:
    parts = label.split("/")
    if parts[0].startswith("tutorial1-default"):
        return "tutorial 1 at upstream's default, " + parts[1]
    return "cell " + parts[1]


tot = defaultdict(lambda: Counter())
was = defaultdict(Counter)
moved = []
other_changed = []
lost = []
for run, b in B.items():
    a = A[run]
    assert b["exit"] == a["exit"], run
    if b["exit"] not in (0, 6):
        continue
    g = group(b["label"])
    parts = b["label"].split("/")
    tag = parts[1]
    method = "energetic" if (tag == "Energetic" or "-E" in tag) else "random"
    for rb, ra in zip(b["rows"], a["rows"]):
        assert (rb["receiver"], rb["freq_hz"]) == (ra["receiver"], ra["freq_hz"])
        for q in CD:
            vb, va = rb[q], ra[q]
            key = (g, method, q)
            tot[key]["receiver_bands"] += 1
            tot[key]["before"] += vb[0] == "value"
            tot[key]["after"] += va[0] == "value"
            if vb[0] == "value" and va[0] != "value":
                lost.append((run, q, vb, va))
            if vb[0] == "value" and va[0] == "value" and vb[1] != va[1]:
                other_changed.append((run, q, vb, va))
            if vb[0] != "value" and va[0] == "value":
                was[(method, q)][f"{vb[1]}: {vb[2]}"] += 1
            if vb[0] != "value" and va[0] != "value" and vb[1:] != va[1:]:
                was[(method, q)][f"still refused, reason changed: {vb[2]} -> {va[2]}"] += 1
        for q in OTHER:
            if rb[q] != ra[q]:
                other_changed.append((run, q, rb[q], ra[q]))

print("C50, C80 and D50 given as values, before (limits 0.01 dB, 0.1 points) and after (0.1 dB,")
print("0.5 points), receiver-bands summed over each group's ten seeds and six receivers.\n")
by_method = defaultdict(Counter)
for (g, m, q), c in sorted(tot.items()):
    by_method[(m, q)].update(c)
    if c["before"] != c["after"]:
        print(f"{g:<45} {m:<10} {q:<7} {c['before']:>5} -> {c['after']:>5} of {c['receiver_bands']}")
print("\nAll groups, by method:")
for (m, q), c in sorted(by_method.items()):
    print(f"  {m:<10} {q:<7} {c['before']:>6} -> {c['after']:>6} of {c['receiver_bands']}")
print("\nWhat the newly given values were refused for before:")
for (m, q), c in sorted(was.items()):
    for why, n in c.most_common():
        print(f"  {m:<10} {q:<7} {n:>6}  {why}")
print(f"\nValues given before and refused after: {len(lost)}")
for x in lost[:10]:
    print("  ", x)
print(f"Values that changed, or other quantities that changed: {len(other_changed)}")
for x in other_changed[:10]:
    print("  ", x)

print("\nTutorial 1 at upstream's default (no change expected or found above if absent):")
for (g, m, q), c in sorted(tot.items()):
    if g.startswith("tutorial"):
        print(f"  {g:<45} {q:<7} {c['before']:>5} -> {c['after']:>5} of {c['receiver_bands']}")

# How far the newly given values lie from the same receiver-band's values given before by the
# other seeds of the cell (their mean), where at least three seeds gave one before.
import statistics as st
before_vals = defaultdict(list)
new_vals = []
for run, b in B.items():
    a = A[run]
    if b["exit"] not in (0, 6):
        continue
    cell = b["label"].split("/")[1]
    for rb, ra in zip(b["rows"], a["rows"]):
        for q in CD:
            k = (cell, rb["receiver"], rb["freq_hz"], q)
            if rb[q][0] == "value":
                before_vals[k].append(rb[q][1])
            elif ra[q][0] == "value":
                new_vals.append((k, ra[q][1]))
dev = defaultdict(list)
for k, v in new_vals:
    ref = before_vals.get(k, [])
    if len(ref) >= 3:
        d = v - st.mean(ref)
        dev[k[3]].append(100 * d if k[3] == "d50" else d)
print("\nNewly given values against the mean of the same receiver-band's values given before by")
print("other seeds (at least 3), dB for C, points for D50:")
for q, ds in sorted(dev.items()):
    ds_abs = sorted(abs(x) for x in ds)
    se = st.stdev(ds) / len(ds) ** 0.5
    print(f"  {q:<7} n {len(ds):>5}, mean {st.mean(ds):+.4f} +- {se:.4f} (SE), median |d| {st.median(ds_abs):.4f}, "
          f"95th |d| {ds_abs[int(0.95 * (len(ds_abs) - 1))]:.4f}, max |d| {ds_abs[-1]:.4f}")
