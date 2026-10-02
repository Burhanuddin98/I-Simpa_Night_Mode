"""Each eps (5, 6, 7) against eps 9, and the seed control (eps 5 seed 3101 against seed 3102).
Significance is judged per ROOM (sign of the room's mean shift), not per cell: cells in one room share its
geometry, source and seed, so they are not independent draws. Usage: python vs9.py > vs9.txt
"""
import statistics as st
from math import comb
import sys
sys.path.insert(0, r"B:\data\m8b-t20\eps-cost")
from functools import lru_cache  # noqa: E402
import bias  # noqa: E402  (bias.py prints its own tables on import)
from bias import rep, ROOMS  # noqa: E402

cells = lru_cache(maxsize=None)(lambda path: bias.cells(path))

print("\n=== vs9 ===")


def compare(m, a_of, b_of, label):
    sh, per = [], {}
    for room in ROOMS:
        a, b = a_of(room), b_of(room)
        rs = [100 * (x["v"] - b[k]["v"]) / b[k]["v"] for k, x in a.items()
              if k[2] == m and "v" in x and "v" in b.get(k, {})]
        if rs:
            per[room] = st.mean(rs)
            sh += rs
    if not sh:
        return
    neg = sum(v < 0 for v in per.values())
    n = len(per)
    # Two-sided sign test over rooms.
    p = min(1.0, 2 * sum(comb(n, i) for i in range(0, min(neg, n - neg) + 1)) / 2 ** n)
    rooms = " ".join(f"{r}:{v:+.2f}({sum(1 for k, x in a_of(r).items() if k[2] == m and 'v' in x and 'v' in b_of(r).get(k, {}))})" for r, v in per.items())
    print(f"{m:6s} {label:22s} cells {len(sh):3d} mean {st.mean(sh):+.2f}% median {st.median(sh):+.2f}% | "
          f"rooms below 0: {neg}/{n} (sign test p {p:.2f}) | {rooms}")


for m in ("t30_s", "t20_s", "edt_s"):
    for e in (5, 6, 7):
        compare(m, lambda r, e=e: cells(rep(r, e)), lambda r: cells(rep(r, 9)), f"eps{e} vs eps9")
    compare(m, lambda r: cells(rep(r, 5)), lambda r: cells(rep(r, 5, 3102)), "ctl eps5 3101 vs 3102")
for m in ("t20_s", "t30_s"):
    print(m, "answered of 336:", {e: sum(sum(1 for k, x in cells(rep(r, e)).items() if k[2] == m and "v" in x) for r in ROOMS) for e in (5, 6, 7, 9)})
