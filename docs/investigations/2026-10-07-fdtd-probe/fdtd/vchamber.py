"""Virtual ISO 354 reverberation chamber in PFFDTD: what absorption does the default boundary deliver?

Pre-registration: <repo>/docs/investigations/2026-10-07-fdtd-probe/PREREG-3.md (Test A).

  python vchamber.py run [empty|0.05|0.1|...]   # one run per sample alpha (default: all)
  python vchamber.py analyze

The model is built here directly (shell, floor sample patch, hanging rigid diffusers); nothing from a .simpa.
Outputs: C:\\tmp\\nm-fdtd-probe\\vchamber_<mapping>\\.
"""
import json, math, os, subprocess, sys, time
from pathlib import Path

import numpy as np

import probe   # octave filters, metrics, the GPU/DLL paths, the Paris/normal mapping helpers

MAP = os.environ.get('VC_MAP', 'paris')          # paris (PFFDTD default) | curve (alpha pre-mapped by VC_CURVE)
OUT = Path(r'C:\tmp\nm-fdtd-probe') / f'vchamber_{MAP}'
FMAX, PPW, DUR = 400.0, 10.0, 8.0
ALPHA_WALL = 0.02
LEVELS = [0.05, 0.10, 0.20, 0.40, 0.70]
BANDS = [125, 250]
C0 = 343.2   # PFFDTD at Tc = 20

FLOOR = np.array([[0, 0], [7.2, 0.3], [7.6, 5.6], [-0.3, 5.2]], float)
CEIL = np.array([[0.2, 0.2, 4.8], [6.9, 0.1, 5.3], [7.3, 5.3, 5.0], [0.1, 5.0, 5.6]], float)
HOLE = np.array([[2.4, 1.2], [5.4, 1.2], [5.4, 4.8], [2.4, 4.8]], float)      # 3.0 x 3.6 = 10.8 m2 sample
SRC = [1.2, 1.0, 1.5]
RCV = [[6.0, 1.2, 1.3], [5.8, 4.4, 1.8], [2.0, 4.3, 1.4], [4.0, 2.6, 2.0], [6.3, 3.0, 1.2], [3.0, 1.6, 1.7]]


def geometry():
    fl = np.c_[FLOOR, np.zeros(4)]
    ho = np.c_[HOLE, np.zeros(4)]
    pts = np.vstack([fl, CEIL, ho])                  # 0-3 floor, 4-7 ceiling, 8-11 sample corners
    wall_tris, floor_tris = [], []
    for i in range(4):                               # four walls, two triangles each
        j = (i + 1) % 4
        wall_tris += [[i, j, 4 + j], [i, 4 + j, 4 + i]]
    wall_tris += [[4, 5, 6], [4, 6, 7]]              # ceiling
    for i in range(4):                               # floor ring around the sample hole
        j = (i + 1) % 4
        floor_tris += [[i, j, 8 + j], [i, 8 + j, 8 + i]]
    sample_tris = [[8, 9, 10], [8, 10, 11]]
    # hanging diffusers: 6 rigid 1.2 x 1.5 m panels, fixed seed, absorbing both sides
    rng = np.random.default_rng(354)
    panels, centers = [], []
    grid = [(x, y) for x in (1.8, 3.8, 5.8) for y in (1.6, 4.0)]      # 3 x 2 cells, jittered, 2 m apart
    for gx, gy in grid:
        c = np.array([gx + rng.uniform(-0.25, 0.25), gy + rng.uniform(-0.25, 0.25), rng.uniform(2.6, 3.6)])
        n = rng.normal(size=3); n /= np.linalg.norm(n)
        u = np.cross(n, [0, 0, 1.0] if abs(n[2]) < 0.9 else [1.0, 0, 0]); u /= np.linalg.norm(u)
        v = np.cross(n, u)
        quad = [c + 0.6 * u * a + 0.75 * v * b for a, b in [(-1, -1), (1, -1), (1, 1), (-1, 1)]]
        centers.append(c); panels.append(quad)
    ppts = np.vstack(panels)
    panel_tris = [[4 * k, 4 * k + 1, 4 * k + 2] for k in range(6)] + [[4 * k, 4 * k + 2, 4 * k + 3] for k in range(6)]
    return pts, wall_tris, floor_tris, sample_tris, ppts, panel_tris


def shell_sides(pts, tris):
    """Air side of each shell triangle: 2 if the normal points into the (convex-enough) chamber, else 1."""
    cen = pts[:8].mean(0)
    out = []
    for t in tris:
        a, b, c = pts[t]
        n = np.cross(b - a, c - a)
        out.append(2 if np.dot(n, cen - (a + b + c) / 3) > 0 else 1)
    return out


def volume_area():
    pts, wt, ft, st, _, _ = geometry()
    tris = np.array(wt + ft + st)
    a, b, c = pts[tris[:, 0]], pts[tris[:, 1]], pts[tris[:, 2]]
    return abs(np.einsum('ij,ij->i', a, np.cross(b, c)).sum()) / 6.0, 0.5 * np.linalg.norm(np.cross(b - a, c - a), axis=1).sum()


def mapped(alpha):
    """Alpha as handed to fit_to_Sabs_oct_11 (11 octaves, flat)."""
    if MAP == 'paris':
        return [alpha] * 11
    return [probe.chamber_input(alpha, f) for f in probe.OCT11]      # MAP == 'curve': closure check


def run_one(level):
    sys.path.insert(0, str(probe.PFF)); cwd = os.getcwd(); os.chdir(probe.PFF)
    from materials.adm_funcs import fit_to_Sabs_oct_11
    from sim_setup import sim_setup
    os.chdir(cwd)
    tag = 'empty' if level is None else f'a{level:.2f}'
    dd = OUT / tag; md = OUT / 'mats'; md.mkdir(parents=True, exist_ok=True)
    pts, wt, ft, st, ppts, pt = geometry()
    fit_to_Sabs_oct_11(np.array(mapped(ALPHA_WALL)), filename=str(md / 'wall.h5'), plot=False)
    sample_alpha = ALPHA_WALL if level is None else level
    fit_to_Sabs_oct_11(np.array(mapped(sample_alpha)), filename=str(md / f'{tag}.h5'), plot=False)
    mats = {'wall': {'tris': wt + ft, 'pts': pts.tolist(), 'color': [150] * 3, 'sides': shell_sides(pts, wt + ft)},
            'sample': {'tris': st, 'pts': pts.tolist(), 'color': [200, 50, 50], 'sides': shell_sides(pts, st)},
            'panel': {'tris': pt, 'pts': ppts.tolist(), 'color': [100] * 3, 'sides': [3] * len(pt)}}
    model = {'mats_hash': mats, 'sources': [{'xyz': SRC, 'name': 'S'}],
             'receivers': [{'xyz': r, 'name': f'R{i + 1}'} for i, r in enumerate(RCV)]}
    mj = OUT / f'model_{tag}.json'; mj.write_text(json.dumps(model))
    os.chdir(probe.PFF)
    sim_setup(model_json_file=str(mj), mat_folder=str(md), source_num=1, insig_type='impulse', diff_source=True,
              mat_files_dict={'wall': 'wall.h5', 'sample': f'{tag}.h5', 'panel': 'wall.h5'}, duration=DUR,
              Tc=20, rh=50, fcc_flag=False, PPW=PPW, fmax=FMAX, Nprocs=1, save_folder=str(dd), compress=0,
              draw_vox=False)
    os.chdir(cwd)
    env = os.environ.copy(); env['PATH'] = ';'.join(probe.DLL_PATH) + ';' + env['PATH']
    t = time.time()
    r = subprocess.run([probe.GPU_EXE], cwd=str(dd), env=env, capture_output=True, text=True)
    (dd / 'gpu_stdout.txt').write_text(r.stdout + '\n--- stderr ---\n' + r.stderr)
    if r.returncode != 0:
        raise SystemExit(f'[{tag}] GPU exit {r.returncode}: {r.stderr[-1500:]}')
    os.chdir(probe.PFF)
    from fdtd.process_outputs import ProcessOutputs
    po = ProcessOutputs(str(dd)); po.initial_process(fcut=10.0); po.apply_lowpass(fcut=FMAX)
    os.chdir(cwd)
    np.savez(dd / 'irs.npz', irs=np.asarray(po.r_out_f, float), fs=float(po.Fs_f))
    print(f'[{tag}] GPU {time.time() - t:.0f} s, saved', flush=True)


def t_mean(tag, fc):
    z = np.load(OUT / tag / 'irs.npz')
    ts = [probe.metrics(x, float(z['fs']), fc, noisy=False)['T30'] for x in z['irs']]
    return float(np.nanmean(ts)), float(np.nanstd(ts))


def analyze():
    V, S_all = volume_area()
    S = 10.8
    lines = [f'Virtual chamber, mapping {MAP}: V = {V:.1f} m3, shell + sample area {S_all:.1f} m2, sample {S} m2, '
             f'wall alpha {ALPHA_WALL}, fmax {FMAX:.0f} Hz.', '',
             '| band | T empty (s) | target alpha | T sample (s) | delivered alpha | delivered / target |',
             '|---|---|---|---|---|---|']
    res = {}
    for fc in BANDS:
        t1, s1 = t_mean('empty', fc)
        for a in LEVELS:
            t2, s2 = t_mean(f'a{a:.2f}', fc)
            d = 55.3 * V / (C0 * S) * (1 / t2 - 1 / t1) + ALPHA_WALL   # + the floor alpha the sample covers
            res.setdefault(fc, []).append({'target': a, 'delivered': d, 'T_empty': t1, 'T_sample': t2})
            lines.append(f'| {fc} | {t1:.2f} +- {s1:.2f} | {a:.2f} | {t2:.2f} +- {s2:.2f} | {d:.3f} | {d / a:.2f} |')
    (OUT / 'chamber.json').write_text(json.dumps({'V': V, 'S': S, 'results': res}, indent=1))
    (OUT / 'chamber.md').write_text('\n'.join(lines) + '\n')
    print('\n'.join(lines))


if __name__ == '__main__':
    if sys.argv[1] == 'run':
        which = sys.argv[2:] or ['empty'] + [str(a) for a in LEVELS]
        for w in which:
            run_one(None if w == 'empty' else float(w))
    elif sys.argv[1] == 'geom':
        V, S = volume_area(); print(f'V = {V:.1f} m3, S = {S:.1f} m2')
    else:
        analyze()
