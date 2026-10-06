"""The probe's result: rates from both arms, and the sanity check (T20 of each receiver's decay from
the Schroeder-integrated echogram) GPU vs SPPS.
usage: python compare.py <gpu.json> <gpu.csv> <spps.json> <spps-run-folder> [--simpa <simpa.exe>] [--out RESULT.md]"""
import argparse, json, math, subprocess, sys
from pathlib import Path

ap = argparse.ArgumentParser()
ap.add_argument("gpu_json"); ap.add_argument("gpu_csv"); ap.add_argument("spps_json"); ap.add_argument("spps_run")
ap.add_argument("--simpa", default=r"C:\tmp\nm-target\release\simpa.exe")
ap.add_argument("--out", default=None)
ap.add_argument("--machine", default="")
a = ap.parse_args()

gpu = json.loads(Path(a.gpu_json).read_text(encoding="utf-8").strip().splitlines()[-1])
spps = json.loads(Path(a.spps_json).read_text(encoding="utf-8").strip().splitlines()[-1])

def t20(energy, dt):
    """T20 from the backward-integrated echogram: the slope fitted -5 to -25 dB, times 3."""
    e = list(energy)
    total = sum(e)
    if total <= 0: return None
    acc = 0.0; sch = [0.0] * len(e)
    for i in range(len(e) - 1, -1, -1):
        acc += e[i]; sch[i] = acc
    db = [10 * math.log10(s / total) if s > 0 else -300.0 for s in sch]
    pts = [(i * dt, d) for i, d in enumerate(db) if -25.0 <= d <= -5.0]
    if len(pts) < 5: return None
    n = len(pts); sx = sum(p[0] for p in pts); sy = sum(p[1] for p in pts)
    sxx = sum(p[0] ** 2 for p in pts); sxy = sum(p[0] * p[1] for p in pts)
    slope = (n * sxy - sx * sy) / (n * sxx - sx * sx)
    return -60.0 / slope if slope < 0 else None

# GPU histograms.
rows = [l.split(",") for l in Path(a.gpu_csv).read_text().strip().splitlines()[1:]]
g_rx = [[float(r[2 + k]) for r in rows] for k in range(len(rows[0]) - 2)]
g_dt = gpu["dt_s"]

# SPPS echograms from the results.
res = subprocess.run([a.simpa, "results", a.spps_run, "--json"], capture_output=True, text=True)
if res.returncode != 0:
    print("simpa results failed:", res.stderr[-500:]); sys.exit(res.returncode)
rep = json.loads(res.stdout)
s_dt = rep["spps"]["time_step_s"]
s_rx = {pr["label"]: pr["bands"][0]["energy_pa2"] for pr in rep["spps"]["point_receivers"]}
names = ["Seat", "Seat2"]

lines = []
lines.append(f"# GPU probe result{(' on ' + a.machine) if a.machine else ''}")
lines.append("")
lines.append(f"Box 6 x 10 x 3 m (seats fixture), energetic, specular, one band, dt {g_dt*1000:g} ms, {gpu['steps']} steps, "
             f"{gpu['particles']:,} particles. GPU {gpu['gpu']} ({gpu['sm']} SMs). SPPS one thread (fixed seed), the verified build.")
lines.append("")
lines.append("| arm | particles | wall s | particle-steps/s |")
lines.append("|---|---|---|---|")
lines.append(f"| CUDA kernel | {gpu['particles']:,} | {gpu['kernel_s']:.3f} (kernel) / {gpu['wall_s']:.3f} (with copies) | {gpu['rate_kernel']:.3g} (kernel) / {gpu['rate_wall']:.3g} |")
par_path = Path(a.gpu_json).with_name("cpu-all-threads.json")
par = None
if par_path.exists():
    for l in par_path.read_text(encoding="utf-8", errors="replace").splitlines():
        if l.strip().startswith("{") and "cpu_all_threads" in l:
            par = json.loads(l)
if par and par.get("cpu_par_particles"):
    lines.append(f"| same code, CPU all {par['cpu_all_threads']} threads | {par['cpu_par_particles']:,} | {par['cpu_par_s']:.3f} | {par['rate_cpu_par']:.3g} |")
if gpu.get("cpu_particles"):
    lines.append(f"| same code, CPU one thread | {gpu['cpu_particles']:,} | {gpu['cpu_s']:.3f} | {gpu['rate_cpu']:.3g} |")
sp_rate = spps.get("rate_solver") or spps["rate_wall"]
sp_s = spps.get("solver_s") or spps["wall_s"]
lines.append(f"| SPPS | {spps['particles']:,} | {sp_s:.1f}{'' if spps.get('solver_s') else ' (whole `simpa run`: export, mesh, solve, results)'} | {sp_rate:.3g} |")
lines.append("")
if par and par.get("cpu_par_particles"):
    lines.append(f"**GPU over this machine's whole CPU, same code, realised: {par['rate_cpu_par'] and gpu['rate_kernel']/par['rate_cpu_par']:.1f}x.**")
lines.append(f"**Speed-up, realised: CUDA kernel / SPPS = {gpu['rate_kernel']/sp_rate:.1f}x; with copies {gpu['rate_wall']/sp_rate:.1f}x.**")
if gpu.get("cpu_particles"):
    lines.append(f"Same code CPU one thread / SPPS = {gpu['rate_cpu']/sp_rate:.2f}x (how much of the gain is the GPU, not the simpler code). "
                 f"GPU vs CPU histograms: max relative difference {gpu['cpu_gpu_maxrel']:.2g} over {gpu['cpu_gpu_bins']} non-zero bins.")
lines.append("")
lines.append("## Sanity check, not a bed: T20 of each receiver's decay")
lines.append("")
lines.append("| receiver | T20 CUDA, s | T20 SPPS, s | ratio |")
lines.append("|---|---|---|---|")
for k, name in enumerate(names):
    tg = t20(g_rx[k], g_dt) if k < len(g_rx) else None
    ts = t20(s_rx[name], s_dt) if name in s_rx else None
    ratio = f"{tg/ts:.3f}" if tg and ts else "n/a"
    lines.append(f"| {name} | {tg:.3f} | {ts:.3f} | {ratio} |" if tg and ts else f"| {name} | {tg} | {ts} | n/a |")
lines.append("")
lines.append(f"SPPS run folder: `{spps['run_folder']}`. GPU histograms: `{a.gpu_csv}`.")
text = "\n".join(lines)
print(text)
if a.out:
    Path(a.out).write_text(text + "\n", encoding="utf-8")
