"""The IR (energy echogram) and spectrogram of a verified SPPS run, from `simpa results --json`.

    python plot_ir.py <run-folder> <out-folder> [simpa.exe]

Writes, per point receiver: <label>-echogram.png (the bands summed, dB re p0², 1 ms steps, full and the first
500 ms), <label>-spectrogram.png (every band x every step, dB, black-red-white as the app's ramp), and
<label>.npz (time_s, bands_hz, energy_pa2[band, step]) so nothing is lost to a picture. SPPS counts energy:
this is an energy echogram per band, not a pressure impulse response (see ../2026-10-06-auralization/PLAN.md).
"""
import json
import os
import subprocess
import sys

import matplotlib
matplotlib.use("Agg")
import matplotlib.pyplot as plt
import numpy as np
from matplotlib.colors import LinearSegmentedColormap

P0_SQ = (20e-6) ** 2
RAMP = LinearSegmentedColormap.from_list("nightmode", ["#000000", "#2a0005", "#8b0000", "#e01b24", "#ff6a5c", "#ffd2c8", "#ffffff"])


def band_hz(b):
    for k in ("band_hz", "hz", "freq_hz", "frequency_hz"):
        if k in b:
            return int(b[k])
    raise KeyError(f"no band frequency key in {list(b)[:12]}")


def db(e):
    return 10 * np.log10(np.maximum(e, 1e-300) / P0_SQ)


def main():
    run, out = sys.argv[1], sys.argv[2]
    simpa = sys.argv[3] if len(sys.argv) > 3 else r"C:\tmp\nm-target\release\simpa.exe"
    os.makedirs(out, exist_ok=True)
    r = subprocess.run([simpa, "results", run, "--json"], capture_output=True, text=True)
    if r.returncode != 0:
        raise SystemExit(f"simpa results exit {r.returncode}: {r.stderr[:500]}")
    rep = json.loads(r.stdout)
    spps = rep["spps"]
    dt = float(spps["time_step_s"])
    receivers = spps["receivers"]
    print(f"run {run}: {len(rep['bands_hz'])} bands {rep['bands_hz'][0]}-{rep['bands_hz'][-1]} Hz, dt {dt*1000:g} ms, {len(receivers)} receivers")
    for rx in receivers:
        label = rx["label"]
        bands = sorted(rx["bands"], key=band_hz)
        hz = np.array([band_hz(b) for b in bands])
        E = np.array([b["energy_pa2"] for b in bands], dtype=np.float64)  # [band, step]
        steps = E.shape[1]
        t = np.arange(steps) * dt
        np.savez_compressed(os.path.join(out, f"{label}.npz"), time_s=t, bands_hz=hz, energy_pa2=E)
        total = E.sum(axis=0)
        peak = db(total).max()
        # Echogram: the bands summed.
        fig, axes = plt.subplots(2, 1, figsize=(12, 7), facecolor="black")
        for ax, tmax in zip(axes, (t[-1], 0.5)):
            m = t <= tmax
            ax.set_facecolor("black")
            ax.plot(t[m] * 1000, db(total[m]), color="#e01b24", lw=0.6)
            ax.set_xlim(0, tmax * 1000)
            ax.set_ylim(peak - 80, peak + 3)
            ax.set_xlabel("time (ms)", color="white")
            ax.set_ylabel("dB re 20 µPa (energy per 1 ms step)", color="white")
            ax.tick_params(colors="white")
            for s in ax.spines.values():
                s.set_color("#444")
            ax.grid(color="#333", lw=0.4)
        axes[0].set_title(f"{label}: energy echogram, {len(hz)} bands summed ({hz[0]}-{hz[-1]} Hz), {dt*1000:g} ms steps. SPPS counts energy: not a pressure IR.", color="white", fontsize=10)
        axes[1].set_title("the first 500 ms", color="white", fontsize=10)
        fig.tight_layout()
        fig.savefig(os.path.join(out, f"{label}-echogram.png"), dpi=130, facecolor="black")
        plt.close(fig)
        # Spectrogram: every band x every step, 60 dB under the peak.
        D = db(E)
        top = D.max()
        fig, ax = plt.subplots(figsize=(14, 6), facecolor="black")
        ax.set_facecolor("black")
        im = ax.imshow(D, aspect="auto", origin="lower", cmap=RAMP, vmin=top - 60, vmax=top,
                       extent=[0, t[-1] * 1000, -0.5, len(hz) - 0.5], interpolation="nearest")
        ax.set_yticks(range(len(hz)))
        ax.set_yticklabels([f"{h/1000:g}k" if h >= 1000 else str(h) for h in hz], fontsize=7)
        ax.set_xlabel("time (ms)", color="white")
        ax.set_ylabel("band (Hz)", color="white")
        ax.tick_params(colors="white")
        ax.set_title(f"{label}: energy per band per {dt*1000:g} ms step, dB, 60 dB under the peak. {len(hz)} third-octave bands: the frequency detail SPPS has.", color="white", fontsize=10)
        cb = fig.colorbar(im, ax=ax, pad=0.01)
        cb.ax.yaxis.set_tick_params(color="white")
        plt.setp(cb.ax.get_yticklabels(), color="white")
        fig.tight_layout()
        fig.savefig(os.path.join(out, f"{label}-spectrogram.png"), dpi=130, facecolor="black")
        plt.close(fig)
        with_energy = int((E.sum(axis=0) > 0).sum())
        print(f"  {label}: {steps} steps, energy on {with_energy} of them, peak {peak:.1f} dB, total {total.sum():.3e} Pa²")
    print(f"written to {out}")


if __name__ == "__main__":
    main()
