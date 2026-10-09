"""Tutorial 2, upstream's expected vs ours at R01 (parity A43).

Upstream's only quantitative result for tutorial 2 is a figure: Docs/tutorial_Elmia_hall.rst:175-179,
images/Tutorial/tutorial_2_Results_Echogram.PNG, "Echogramm at receiver R01 (1 million particles per source)": the
Global (all bands) sound level at R01 with its Schroeder curve. This script reads the black Schroeder curve off the
picture (pixel columns; the axes calibrated from the tick labels 0/20/40 dB and 200..1400 ms) and fits its slope over
200-1000 ms, then computes the same quantity from our run of the shipped tutorial project: R01's energy summed over
the six bands and the three sources, Schroeder-integrated up to the end of upstream's plotted span (1.5 s, where its
curve drops away) and over our whole run, slope fitted over the same 200-1000 ms.
usage: python t2_compare.py <results-t2-spps.json> <upstream png> <out.json>
"""
import json
import math
import sys

import numpy as np
from PIL import Image

res_path, png, out = sys.argv[1:4]

# --- upstream's figure -------------------------------------------------------------------------------------
img = np.asarray(Image.open(png).convert("RGB")).astype(int)
H, W, _ = img.shape
# Axis calibration from the tick labels (pixel rows of 0, 20, 40 dB and columns of 200, 1400 ms, read by eye
# from the image at full size and checked against the gridless tick marks).
Y0, Y20, Y40 = 496, 326, 157
X200, X1400 = 178, 991
px_per_db = ((Y0 - Y20) / 20 + (Y20 - Y40) / 20) / 2
px_per_ms = (X1400 - X200) / 1200


def db_of(y):
    return (Y0 - y) / px_per_db


def ms_of(x):
    return 200 + (x - X200) / px_per_ms


black = (img[:, :, 0] < 60) & (img[:, :, 1] < 60) & (img[:, :, 2] < 60)
curve = []
# Inside the frame only: the y axis is column 45 and the x axis row 623, both black; the curve is the topmost black
# run of each column (the green bars are never black).
for x in range(50, 1055):
    ys = np.nonzero(black[60:600, x])[0]
    if len(ys):
        run = ys[ys <= ys[0] + 3]
        curve.append((ms_of(x), db_of(60 + run.mean())))
curve = np.array(curve)
sel = (curve[:, 0] >= 200) & (curve[:, 0] <= 1000)
slope_up, icpt_up = np.polyfit(curve[sel, 0] / 1000, curve[sel, 1], 1)

# --- ours ----------------------------------------------------------------------------------------------------
r = json.load(open(res_path, encoding="utf-8"))
sp = r["spps"]
dt = sp["time_step_s"]
r01 = next(p for p in sp["point_receivers"] if p["label"] == "R01")
e = np.zeros(sp["steps"])
for b in r01["bands"]:
    e += np.array(b["energy_pa2"], dtype=float)
t = (np.arange(len(e)) + 0.5) * dt


def schroeder_slope(energy, t_end):
    n = int(round(t_end / dt))
    en = energy[:n]
    sch = np.cumsum(en[::-1])[::-1]
    with np.errstate(divide="ignore"):
        lvl = 10 * np.log10(sch / sch[0])
    tt = t[:n]
    m = (tt >= 0.2) & (tt <= 1.0)
    s, _ = np.polyfit(tt[m], lvl[m], 1)
    return s


ours_15 = schroeder_slope(e, 1.5)
ours_full = schroeder_slope(e, sp["duration_s"])
rec = {
    "upstream": {"source": "Docs/tutorial_Elmia_hall.rst:175-179, images/Tutorial/tutorial_2_Results_Echogram.PNG",
                 "particles_per_source": 1_000_000, "curve_points": int(sel.sum()),
                 "slope_db_per_s_200_1000ms": round(float(slope_up), 2),
                 "decay_time_60db_s": round(-60 / float(slope_up), 3)},
    "ours": {"run": r["run_folder"], "particles_per_source": sp["particles_per_source"], "time_step_s": dt,
             "slope_db_per_s_200_1000ms_integrated_to_1_5s": round(float(ours_15), 2),
             "decay_time_60db_s_integrated_to_1_5s": round(-60 / float(ours_15), 3),
             "slope_db_per_s_200_1000ms_whole_run": round(float(ours_full), 2),
             "decay_time_60db_s_whole_run": round(-60 / float(ours_full), 3)},
}
rec["difference_s_like_for_like"] = round(rec["ours"]["decay_time_60db_s_integrated_to_1_5s"] - rec["upstream"]["decay_time_60db_s"], 3)
rec["difference_percent_like_for_like"] = round(100 * rec["difference_s_like_for_like"] / rec["upstream"]["decay_time_60db_s"], 1)
json.dump(rec, open(out, "w", encoding="utf-8"), indent=1)
print(json.dumps(rec, indent=1))
