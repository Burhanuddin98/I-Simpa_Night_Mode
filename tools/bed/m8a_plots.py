"""M8a's decay arrays and plots (docs/investigations/2026-09-29-m8a/SPEC.md, section 6).

    py -3 tools/bed/m8a_plots.py <root>/<stamp>

Reads what `simpa bed` wrote there (report.json, decays/<cell>.csv) and writes, beside them:
- decays/<cell>.npz: the CSV's columns as arrays (source, seed, receiver, freq_hz, u_s,
  level_db), one file per cell;
- plots/<cell>.png: per band, every seed's decay at each receiver, the transport's curves, and the
  slopes of Kuttruff's corrected Eyring, plain Eyring and the transport through the seeds' -5 dB
  point;
- plots/summary.png: checks A, B and C per cell against absorption, per room, method and air, with
  each check's limit shaded.

One process, numpy and matplotlib only. It judges nothing: report.json holds every verdict.
"""

import csv
import json
import sys
from pathlib import Path

import numpy as np
import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt  # noqa: E402

# Categorical slots (dataviz reference palette, light): blue, orange, aqua, violet. Validated
# all-pairs for four series (worst CVD dE 9.2, normal-vision 16.3); aqua is below 3:1 on the
# surface, so every series also has its own marker and a legend.
SERIES = ["#2a78d6", "#eb6834", "#1baf7a", "#4a3aa7"]
MARKERS = ["o", "s", "^", "D"]
INK = "#0b0b0b"
INK_2 = "#52514e"
MUTED = "#8a8985"
GRID = "#e4e3df"
SURFACE = "#fcfcfb"
LIMIT_FILL = "#f0efec"

plt.rcParams.update(
    {
        "figure.facecolor": SURFACE,
        "axes.facecolor": SURFACE,
        "axes.edgecolor": MUTED,
        "axes.labelcolor": INK_2,
        "axes.titlecolor": INK,
        "axes.grid": True,
        "grid.color": GRID,
        "grid.linewidth": 0.6,
        "xtick.color": INK_2,
        "ytick.color": INK_2,
        "font.size": 8,
        "axes.titlesize": 9,
        "legend.frameon": False,
        "savefig.dpi": 130,
    }
)


def read_csv(path):
    cols = {k: [] for k in ["source", "seed", "receiver", "freq_hz", "u_s", "level_db"]}
    with open(path, newline="") as f:
        for row in csv.DictReader(f):
            for k in cols:
                cols[k].append(row[k])
    return {
        "source": np.array(cols["source"]),
        "seed": np.array(cols["seed"], dtype=np.int32),
        "receiver": np.array(cols["receiver"], dtype=np.int32),
        "freq_hz": np.array(cols["freq_hz"], dtype=np.int32),
        "u_s": np.array(cols["u_s"], dtype=np.float64),
        "level_db": np.array(cols["level_db"], dtype=np.float64),
    }


def segments(d, mask):
    """Rows under mask, split into runs of one (source, seed, receiver, band) in file order."""
    idx = np.flatnonzero(mask)
    if idx.size == 0:
        return []
    key = np.stack([d["seed"][idx], d["receiver"][idx], d["freq_hz"][idx]], axis=1)
    breaks = np.flatnonzero(np.any(key[1:] != key[:-1], axis=1)) + 1
    return np.split(idx, breaks)


def slope_line(ax, u5, t, style, color, label):
    if t is None or not np.isfinite(u5):
        return
    u = np.array([u5, u5 + t / 2.0])
    ax.plot(u, [-5.0, -35.0], linestyle=style, color=color, linewidth=1.2, label=label, zorder=4)


def cell_plot(cell, d, out):
    ref = cell.get("reference") or {}
    bands = cell["bands_hz"]
    n = len(bands)
    cols = 4 if n > 6 else 3
    rows = int(np.ceil(n / cols))
    fig, axes = plt.subplots(rows, cols, figsize=(3.2 * cols, 2.6 * rows), squeeze=False)
    for i, f in enumerate(bands):
        ax = axes[i // cols][i % cols]
        spps = (d["source"] == "spps") & (d["freq_hz"] == f)
        for seg in segments(d, spps):
            r = int(d["receiver"][seg[0]])
            ax.plot(
                d["u_s"][seg],
                d["level_db"][seg],
                color=SERIES[r % len(SERIES)],
                linewidth=0.7,
                alpha=0.7,
                zorder=3,
            )
        tr = (d["source"] == "transport") & (d["freq_hz"] == f)
        for seg in segments(d, tr):
            ax.plot(d["u_s"][seg], d["level_db"][seg], color=MUTED, linewidth=2.2, zorder=2)
        fits = (d["source"] == "spps_fit") & (d["freq_hz"] == f) & (d["level_db"] == -5.0)
        u5 = float(np.mean(d["u_s"][fits])) if fits.any() else float("nan")
        band = next((b for b in ref.get("bands", []) if b["freq_hz"] == f), {})
        slope_line(ax, u5, band.get("kuttruff_s"), "-", INK, "Kuttruff slope")
        slope_line(ax, u5, band.get("eyring_s"), "--", INK_2, "plain Eyring slope")
        slope_line(ax, u5, band.get("transport_t"), ":", INK, "transport slope")
        ax.set_ylim(-65, 2)
        umax = d["u_s"][spps].max() if spps.any() else 1.0
        ax.set_xlim(0, umax)
        ax.set_title(f"{f} Hz")
        ax.set_xlabel("time from arrival, s")
        ax.set_ylabel("level, dB")
    for j in range(n, rows * cols):
        axes[j // cols][j % cols].axis("off")
    handles = [
        plt.Line2D([], [], color=SERIES[r], linewidth=1.2, label=f"SPPS R{r:03d} (each seed)")
        for r in range(3)
    ] + [
        plt.Line2D([], [], color=MUTED, linewidth=2.2, label="transport curve"),
        plt.Line2D([], [], color=INK, linewidth=1.2, label="Kuttruff slope"),
        plt.Line2D([], [], color=INK_2, linestyle="--", linewidth=1.2, label="plain Eyring slope"),
        plt.Line2D([], [], color=INK, linestyle=":", linewidth=1.2, label="transport slope"),
    ]
    fig.legend(handles=handles, loc="lower center", ncol=4, bbox_to_anchor=(0.5, 0.0))
    verdict = cell.get("verdict", "?")
    fig.suptitle(
        f"{cell['id']}: {cell['particles_per_source']:,} particles, {len(cell['seeds'])} seeds, "
        f"verdict {verdict}",
        color=INK,
    )
    fig.tight_layout(rect=(0, 0.08, 1, 0.96))
    fig.savefig(out)
    plt.close(fig)


def worst_band(a):
    """A's band with the widest reach from 0, as (mean, lo, hi)."""
    best = None
    for b in (a or {}).get("bands", []):
        i = b.get("interval")
        if not i:
            continue
        reach = max(abs(i["lo"]), abs(i["hi"]))
        if best is None or reach > best[0]:
            best = (reach, i["mean"], i["lo"], i["hi"])
    return None if best is None else best[1:]


def summary_plot(report, out):
    cells = report["cells"]
    rooms = []
    for c in cells:
        if c["room"] not in rooms:
            rooms.append(c["room"])
    series = [("random", False), ("random", True), ("energetic", False), ("energetic", True)]
    alphas = sorted({c["alpha"] for c in cells})
    xpos = {a: i for i, a in enumerate(alphas)}
    checks = [
        ("A: worst band, T30/Kuttruff - 1, %", 5.0),
        ("B: seed spread of the cell mean, %", 2.0),
        ("C: T30/transport - 1, %", 0.5),
    ]
    width = max(8.0, 4.2 * len(rooms))
    fig, axes = plt.subplots(len(checks), len(rooms), figsize=(width, 7.5), squeeze=False)
    for ci, (title, limit) in enumerate(checks):
        for ri, room in enumerate(rooms):
            ax = axes[ci][ri]
            if ci == 1:
                ax.axhspan(0, limit, color=LIMIT_FILL, zorder=0)
                ax.axhline(limit, color=MUTED, linewidth=0.8)
            else:
                ax.axhspan(-limit, limit, color=LIMIT_FILL, zorder=0)
                ax.axhline(0, color=MUTED, linewidth=0.8)
            for si, (method, air) in enumerate(series):
                xs, ys, lo, hi = [], [], [], []
                for c in cells:
                    if c["room"] != room or c["method"] != method or c["air"] != air:
                        continue
                    if ci == 0:
                        w = worst_band(c.get("a"))
                        if w is None:
                            continue
                        y, l, h = w
                    elif ci == 1:
                        b = c.get("b")
                        if not b:
                            continue
                        y = l = h = b["spread"]
                    else:
                        x = c.get("c")
                        if not x:
                            continue
                        i = x["interval"]
                        y, l, h = i["mean"], i["lo"], i["hi"]
                    xs.append(xpos[c["alpha"]] + (si - 1.5) * 0.12)
                    ys.append(100 * y)
                    lo.append(100 * (y - l))
                    hi.append(100 * (h - y))
                if not xs:
                    continue
                ax.errorbar(
                    xs,
                    ys,
                    yerr=[lo, hi],
                    fmt=MARKERS[si],
                    color=SERIES[si],
                    markersize=5,
                    elinewidth=1.2,
                    capsize=0,
                    label=f"{method}, air {'on' if air else 'off'}",
                )
            ax.set_xticks(range(len(alphas)))
            ax.set_xticklabels([f"α {a}" for a in alphas])
            gated = any(c["gated"] for c in cells if c["room"] == room)
            ax.set_title(f"{room} m{'' if gated else ' (reported only)'}: {title}")
    handles, labels = [], []
    for ax in axes.flat:
        h, l = ax.get_legend_handles_labels()
        for hh, ll in zip(h, l):
            if ll not in labels:
                handles.append(hh)
                labels.append(ll)
    fig.legend(handles, labels, loc="lower center", ncol=4, bbox_to_anchor=(0.5, 0.0))
    verdict = "PASS" if report["pass"] else "NOT PASSED"
    extra = " (exploratory)" if report["exploratory"] else ""
    fig.suptitle(f"M8a bed: {verdict}{extra}; shaded: each check's limit", color=INK)
    fig.tight_layout(rect=(0, 0.05, 1, 0.96))
    fig.savefig(out)
    plt.close(fig)


def main(stamp):
    stamp = Path(stamp)
    report = json.loads((stamp / "report.json").read_text(encoding="utf-8"))
    decays = stamp / "decays"
    plots = stamp / "plots"
    plots.mkdir(exist_ok=True)
    written = 0
    for cell in report["cells"]:
        path = decays / f"{cell['id']}.csv"
        if not path.exists():
            print(f"no decays for {cell['id']}")
            continue
        d = read_csv(path)
        np.savez_compressed(decays / f"{cell['id']}.npz", **d)
        cell_plot(cell, d, plots / f"{cell['id']}.png")
        written += 2
    summary_plot(report, plots / "summary.png")
    written += 1
    print(f"wrote {written} files under {stamp}")


if __name__ == "__main__":
    if len(sys.argv) != 2:
        sys.exit("usage: py -3 tools/bed/m8a_plots.py <root>/<stamp>")
    main(sys.argv[1])
