"""Tutorial 3, upstream's expected vs ours: the difference map (parity A43).

Upstream's only quantitative result for tutorial 3's comparison is a picture: Docs/tutorial_industrial_hall.rst:432-439,
images/Tutorial/Screenshot_6_tutorial_3.PNG, "SPL sound map showing the difference between two calculations": the
'Power gain' map of the plane receiver, Global, cumulated, with absorbing material on diff_wall against the reference.
Its legend bar (pixel rows 84 to 423, ticks -0.5 dB at row 160 and -1.0 dB at row 259, read off the image) spans
about -0.12 to -1.83 dB, with contours at -0.5 and -1.0 dB.

Ours: the two runs the app made of the shipped tutorial project, reference then Absorbing_material on diff_wall. Each
face's cumulated energy is the sum of its records in Surface receiver/Global/rs_cut.csbin (read through `simpa dump
csbin`, the canonical dump), and the difference is 10 log10(absorbing / reference) per face, as the app's
"Difference from baseline" draws it (this run minus the baseline, in dB).
usage: python t3_compare.py <simpa.exe> <reference run dir> <absorbing run dir> <out.json>
"""
import json
import math
import struct
import subprocess
import sys

simpa, ref_dir, abs_dir, out = sys.argv[1:5]
REL = r"solve\Surface receiver\Global\rs_cut.csbin"


def f32(h):
    return struct.unpack(">f", bytes.fromhex(h))[0]


def faces(run_dir):
    txt = subprocess.run([simpa, "dump", "csbin", f"{run_dir}\\{REL}"], capture_output=True, encoding="utf-8", check=True).stdout
    e, cur = {}, None
    for line in txt.splitlines():
        parts = line.split()
        if not parts:
            continue
        if parts[0] == "face":
            cur = len(e)
            e[cur] = 0.0
        elif parts[0] == "record" and cur is not None:
            e[cur] += f32(parts[2])
    return e


ref, ab = faces(ref_dir), faces(abs_dir)
assert len(ref) == len(ab), (len(ref), len(ab))
d = sorted(10 * math.log10(ab[i] / ref[i]) for i in ref if ref[i] > 0 and ab[i] > 0)
n = len(d)


def pct(p):
    return round(d[min(n - 1, int(p / 100 * n))], 3)


rec = {
    "upstream": {"source": "Docs/tutorial_industrial_hall.rst:433-450, images/Tutorial/Screenshot_6_tutorial_3.PNG",
                 "legend_range_db": [-0.12, -1.83], "contours_db": [-0.5, -1.0],
                 "particles_per_source": "not stated for the figure (the document's table, rst:358-359: 150 000)"},
    "ours": {"reference_run": ref_dir, "absorbing_run": abs_dir, "faces": len(ref), "faces_compared": n,
             "min_db": round(d[0], 3), "p5_db": pct(5), "median_db": pct(50), "p95_db": pct(95), "max_db": round(d[-1], 3),
             "share_above_minus_0_5": round(sum(x > -0.5 for x in d) / n, 3),
             "share_between": round(sum(-1.0 <= x <= -0.5 for x in d) / n, 3),
             "share_below_minus_1_0": round(sum(x < -1.0 for x in d) / n, 3)},
}
json.dump(rec, open(out, "w", encoding="utf-8"), indent=1)
print(json.dumps(rec, indent=1))
