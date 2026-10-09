"""FDTD probe: Night Mode's bras_cr2.simpa -> PFFDTD (CUDA) -> octave T30/EDT/C80 vs BRAS CR2 measured.

Pre-registration: <repo>/docs/investigations/2026-10-07-fdtd-probe/PREREG.md (targets set before any run).

  python probe.py run [LS1|LS2]  # build model + materials, voxelise, GPU-run LS1 and LS2 (all 5 mics each)
  python probe.py analyze   # metrics for FDTD / Eyring / Sabine / measured, writes out/results.{json,md}

Everything lands in C:\\tmp\\nm-fdtd-probe\\out. Nothing in the repo or in pffdtd is written to.
"""
import json, math, os, subprocess, sys, time
from pathlib import Path

import numpy as np
import scipy.io.wavfile as wavfile
from scipy import signal

PFF = Path(r'C:\RoomGUI\pffdtd\python')
ROOM = os.environ.get('PROBE_ROOM', 'CR2')
SIMPA = Path(r'C:\repos\Room-Acoustics-Engine\I-Simpa FDTD\app\src-tauri\examples') / f'bras_{ROOM.lower()}.simpa'
MEAS = Path(r'C:\tmp\bras_dl\targets') / ROOM
MEAS_TAG = {'CR1': 'CR1_RIR_DoorAngle3'}.get(ROOM, f'{ROOM}_RIR')   # the CR1 example is door angle 3
# paris: alpha read as random-incidence (PFFDTD default); normal: as normal-incidence;
# scale: alpha x PROBE_K per band ("125:1.25,250:1.02"), then read as random-incidence
BC = os.environ.get('PROBE_BC', 'paris')
K = {int(a): float(b) for a, b in (kv.split(':') for kv in os.environ.get('PROBE_K', '').split(',') if kv)}
GPU_EXE = r'C:\RoomGUI\pffdtd\c_cuda\fdtd_main_gpu_single.exe'
DLL_PATH = [r'C:\tmp\hdf5dl\ex\HDF5-1.14.6-win64\bin', r'C:\Program Files\NVIDIA GPU Computing Toolkit\CUDA\v12.4\bin']

FMAX = float(os.environ.get('PROBE_FMAX', 1000)); PPW = 10.0; DUR = float(os.environ.get('PROBE_DUR', 2.5))
_default = ROOM == 'CR2' and FMAX == 1000 and BC == 'paris'
OUT = Path(r'C:\tmp\nm-fdtd-probe') / ('out' if _default else
                                       f"out_{ROOM}_{BC}{'_' + os.environ.get('PROBE_K', '') if K else ''}_{FMAX:.0f}"
                                       .replace(':', '-').replace(',', '_'))
BANDS = [63, 125, 250] + ([500] if FMAX >= 710 else [])
JUDGED = [b for b in BANDS if b >= 125]
TARGET = {'T30': 0.05, 'EDT': 0.05, 'C80': 1.0}
OCT11 = [16, 31.5, 63, 125, 250, 500, 1000, 2000, 4000, 8000, 16000]


CURVE_FILE = Path(r'C:\tmp\nm-fdtd-probe\chamber_curve.json')


def chamber_input(alpha, fc):
    """The alpha to hand PFFDTD's default (Paris) fit so that its boundary DELIVERS `alpha` in the virtual
    ISO 354 chamber (vchamber.py). Per octave: the nearest calibrated band (63 -> 125, >= 500 -> 250);
    above the calibrated range the last delivered/input ratio is carried; capped at 0.95."""
    curve = json.loads(CURVE_FILE.read_text())
    key = min(curve, key=lambda b: abs(math.log(int(b) / fc)))
    d, x = curve[key]['delivered'], curve[key]['input']
    a = float(np.interp(alpha, d, x)) if alpha <= d[-1] else alpha * x[-1] / d[-1]
    return min(0.95, a)


# ---------------------------------------------------------------- model
def load_room():
    p = json.loads(SIMPA.read_text(encoding='utf-8'))
    V = np.array(p['geometry']['vertices'], dtype=np.float64)
    F = np.array([f[:3] for f in p['geometry']['faces']], dtype=np.int64)
    G = [f[3] for f in p['geometry']['faces']]
    mat_of_group = {g['id']: g['material'] for g in p['surface_groups']}
    mats = {m['id']: m for m in p['materials']}
    bands = p['bands']['frequencies_hz']
    return p, V, F, G, mat_of_group, mats, bands


def inside(pts, V, F):
    """Ray-parity point-in-mesh (+x ray, Moller-Trumbore), vectorised over triangles."""
    a, b, c = V[F[:, 0]], V[F[:, 1]], V[F[:, 2]]
    e1, e2 = b - a, c - a
    d = np.array([1.0, 0.0, 0.0]) + np.array([0.0, 1e-7, 2e-7])   # skew off axis-aligned edges
    h = np.cross(d, e2); det = np.einsum('ij,ij->i', e1, h)
    ok = np.abs(det) > 1e-12
    res = []
    for p in pts:
        s = p - a
        u = np.einsum('ij,ij->i', s, h) / np.where(ok, det, 1)
        q = np.cross(s, e1)
        v = (q @ d) / np.where(ok, det, 1)
        t = np.einsum('ij,ij->i', e2, q) / np.where(ok, det, 1)
        hit = ok & (u >= 0) & (v >= 0) & (u + v <= 1) & (t > 1e-9)
        res.append(int(hit.sum()) % 2 == 1)
    return np.array(res)


def build_model(src_name):
    p, V, F, G, mat_of_group, mats, bands = load_room()
    a, b, c = V[F[:, 0]], V[F[:, 1]], V[F[:, 2]]
    n = np.cross(b - a, c - a); n /= np.linalg.norm(n, axis=1, keepdims=True)
    cen = (a + b + c) / 3.0
    front_in = inside(cen + 1e-3 * n, V, F)          # air on the +normal side -> side 2 (front)
    back_in = inside(cen - 1e-3 * n, V, F)
    sides = np.where(front_in & back_in, 3, np.where(front_in, 2, np.where(back_in, 1, 0)))
    print(f'[{ROOM}] faces by air side: front {int((sides == 2).sum())}, back {int((sides == 1).sum())}, '
          f'both {int((sides == 3).sum())}, neither {int((sides == 0).sum())} (neither -> rigid)', flush=True)

    mat_dir = OUT / 'mats'; mat_dir.mkdir(parents=True, exist_ok=True)
    sys.path.insert(0, str(PFF)); cwd = os.getcwd(); os.chdir(PFF)
    from materials.adm_funcs import fit_to_Sabs_oct_11
    os.chdir(cwd)
    mats_hash, mat_files = {}, {}
    for gid, mid in mat_of_group.items():
        idx = [i for i, g in enumerate(G) if g == gid and sides[i] != 0]
        if not idx:
            continue
        m = mats[mid]; name = m['name']
        al = dict(zip(bands, m['absorption']))
        a11 = [al[min(bands, key=lambda bb: abs(math.log(bb / f)))] for f in OCT11]   # nearest band, carried out
        if BC == 'curve':
            a11 = [chamber_input(a, f) for a, f in zip(a11, OCT11)]
        if BC == 'scale':
            a11 = [min(0.95, a * K.get(min(K, key=lambda kb: abs(math.log(kb / f))), 1.0)) for a, f in zip(a11, OCT11)]
        if BC == 'normal':   # admittance g with 4g/(1+g)^2 = alpha, re-expressed as the Paris alpha PFFDTD fits to
            g = [(2 - a - 2 * math.sqrt(1 - a)) / a if a > 0 else 0.0 for a in a11]
            a11 = [min(0.95, 8 * x * (1 + x / (1 + x) - 2 * x * math.log((x + 1) / x))) if x > 0 else 0.0 for x in g]
        h5 = mat_dir / f'{name}.h5'
        fit_to_Sabs_oct_11(np.array(a11), filename=str(h5), plot=False)
        mats_hash[name] = {'tris': F[idx].tolist(), 'pts': V.tolist(), 'color': [160, 160, 160],
                           'sides': sides[idx].tolist()}
        mat_files[name] = h5.name
    src = next(s for s in p['sources'] if s['name'] == src_name)
    if (sides == 0).any():   # buried faces: rigid, which PFFDTD requires to carry sides 0
        mats_hash['_RIGID'] = {'tris': F[sides == 0].tolist(), 'pts': V.tolist(), 'color': [100, 100, 100],
                               'sides': [0] * int((sides == 0).sum())}
    model = {'mats_hash': mats_hash,
             'sources': [{'xyz': src['position'], 'name': src['name']}],
             'receivers': [{'xyz': r['position'], 'name': r['name']} for r in p['point_receivers']]}
    mj = OUT / f'model_{src_name}.json'; mj.write_text(json.dumps(model))
    return mj, mat_dir, mat_files, [r['name'] for r in p['point_receivers']]


def run(srcs=('LS1', 'LS2')):
    OUT.mkdir(parents=True, exist_ok=True)
    sys.path.insert(0, str(PFF))
    for src in srcs:
        mj, mat_dir, mat_files, rnames = build_model(src)
        dd = OUT / f'sim_{src}'
        cwd = os.getcwd(); os.chdir(PFF)
        from sim_setup import sim_setup
        sim_setup(model_json_file=str(mj), mat_folder=str(mat_dir), source_num=1, insig_type='impulse',
                  diff_source=True, mat_files_dict=mat_files, duration=DUR, Tc=20, rh=50, fcc_flag=False,
                  PPW=PPW, fmax=FMAX, Nprocs=1, save_folder=str(dd), compress=0, draw_vox=False)
        os.chdir(cwd)
        env = os.environ.copy(); env['PATH'] = ';'.join(DLL_PATH) + ';' + env['PATH']
        t = time.time()
        r = subprocess.run([GPU_EXE], cwd=str(dd), env=env, capture_output=True, text=True)
        (dd / 'gpu_stdout.txt').write_text(r.stdout + '\n--- stderr ---\n' + r.stderr)
        print(f'[{src}] GPU exit {r.returncode} in {time.time() - t:.0f} s; '
              f'{[l for l in r.stdout.splitlines() if "Combined" in l][-1:]}', flush=True)
        if r.returncode != 0:
            raise SystemExit(r.stderr[-2000:])
        os.chdir(PFF)
        from fdtd.process_outputs import ProcessOutputs
        po = ProcessOutputs(str(dd)); po.initial_process(fcut=10.0); po.apply_lowpass(fcut=FMAX)
        os.chdir(cwd)
        irs = np.asarray(po.r_out_f, dtype=np.float64); fs = float(po.Fs_f)
        np.savez(dd / 'irs.npz', irs=irs, fs=fs, receivers=np.array(rnames))
        for k, rn in enumerate(rnames):
            x = irs[k] / (np.max(np.abs(irs[k])) + 1e-30) * 0.5
            wavfile.write(dd / f'FDTD_{src}_{rn}.wav', int(round(fs)), x.astype(np.float32))
        print(f'[{src}] {irs.shape} @ {fs:.1f} Hz saved', flush=True)


# ---------------------------------------------------------------- metrics
def octave(x, fs, fc):
    sos = signal.butter(4, [fc / math.sqrt(2), fc * math.sqrt(2)], btype='band', fs=fs, output='sos')
    return signal.sosfilt(sos, x)


def onset(x):
    e = x ** 2
    return int(np.argmax(e >= e.max() * 10 ** (-20 / 10)))


def edc_db(e, fs, noisy):
    """Schroeder EDC (dB, 0 at start). noisy: Lundeby-lite truncation + exponential tail compensation."""
    tail = 0.0
    if noisy:
        blk = max(1, int(0.01 * fs)); nb = len(e) // blk
        L = 10 * np.log10(e[:nb * blk].reshape(nb, blk).mean(1) + 1e-300)
        noise = 10 * np.log10(e[int(0.9 * len(e)):].mean() + 1e-300)
        i0 = int(np.argmax(L)); tb = (np.arange(nb) + 0.5) * blk / fs
        sel = np.where((np.arange(nb) > i0) & (L <= L[i0] - 5) & (L >= noise + 10))[0]
        if len(sel) < 3:
            raise ValueError('decay range above noise too short')
        k, q = np.polyfit(tb[sel], L[sel], 1)
        tc = (noise - q) / k
        n1 = min(len(e), max(int(tc * fs), sel[-1] * blk))
        tau = 10 / (-k * math.log(10))                      # energy e-folding time
        tail = (10 ** ((k * n1 / fs + q) / 10)) * tau * fs  # integral of the fitted line beyond n1 (block-mean units)
        e = e[:n1]
    s = np.cumsum(e[::-1])[::-1] + tail
    return 10 * np.log10(s / s[0]), e


def metrics(x, fs, fc, noisy):
    t0 = onset(x)
    y = octave(x, fs, fc)[t0:]
    d, e = edc_db(y ** 2, fs, noisy)
    t = np.arange(len(d)) / fs

    def slope_T(hi, lo):
        sel = (d <= hi) & (d >= lo)
        if sel.sum() < 5 or d.min() > lo:
            return float('nan')
        return -60.0 / np.polyfit(t[sel], d[sel], 1)[0]
    n80 = int(0.08 * fs)
    c80 = 10 * np.log10(e[:n80].sum() / max(e[n80:].sum(), 1e-300))
    return {'EDT': slope_T(0, -10), 'T30': slope_T(-5, -35), 'C80': float(c80)}


def eyring_sabine():
    p, V, F, G, mat_of_group, mats, bands = load_room()
    a, b, c = V[F[:, 0]], V[F[:, 1]], V[F[:, 2]]
    area = 0.5 * np.linalg.norm(np.cross(b - a, c - a), axis=1)
    vol = abs(np.einsum('ij,ij->i', a, np.cross(b, c)).sum()) / 6.0
    out = {}
    for fc in BANDS:
        bi = min(range(len(bands)), key=lambda i: abs(math.log(bands[i] / fc)))
        A = sum(area[i] * mats[mat_of_group[G[i]]]['absorption'][bi] for i in range(len(G)))
        S = area.sum(); am = A / S
        te = 0.161 * vol / (-S * math.log(1 - am)); ts = 0.161 * vol / A
        out[fc] = {'Eyring': te, 'Sabine': ts, 'C80_Eyring': 10 * math.log10(math.exp(0.08 * 13.8155 / te) - 1)}
    return out, vol, float(area.sum())


def analyze():
    rows = []
    for src in ['LS1', 'LS2']:
        z = np.load(OUT / f'sim_{src}' / 'irs.npz')
        irs, fs, rn = z['irs'], float(z['fs']), list(z['receivers'])
        for k, mp in enumerate(rn):
            fsm, xm = wavfile.read(MEAS / f'{MEAS_TAG}_{src}_{mp}_Dodecahedron.wav')
            xm = xm.astype(np.float64) if xm.ndim == 1 else xm[:, 0].astype(np.float64)
            for fc in BANDS:
                rows.append({'pair': f'{src}-{mp}', 'band': fc,
                             'meas': metrics(xm, fsm, fc, noisy=True),
                             'fdtd': metrics(irs[k], fs, fc, noisy=False)})
    ey, vol, S = eyring_sabine()
    summ = {}
    for fc in BANDS:
        R = [r for r in rows if r['band'] == fc]
        s = {}
        for m in ['T30', 'EDT', 'C80']:
            mv = np.array([r['meas'][m] for r in R]); fv = np.array([r['fdtd'][m] for r in R])
            if m == 'C80':
                ev = np.full_like(mv, ey[fc]['C80_Eyring']); err_f = np.abs(fv - mv); err_e = np.abs(ev - mv)
            else:
                ev = np.full_like(mv, ey[fc]['Eyring']); err_f = np.abs(fv - mv) / mv; err_e = np.abs(ev - mv) / mv
            s[m] = {'meas_mean': float(np.nanmean(mv)), 'fdtd_mean': float(np.nanmean(fv)),
                    'eyring': float(ev[0]), 'fdtd_err': float(np.nanmean(err_f)), 'eyring_err': float(np.nanmean(err_e)),
                    'fdtd_pass': bool(np.nanmean(err_f) <= TARGET[m]), 'n': int(np.isfinite(err_f).sum())}
        s['Sabine'] = ey[fc]['Sabine']
        summ[fc] = s
    (OUT / 'results.json').write_text(json.dumps({'rows': rows, 'summary': summ, 'volume_m3': vol, 'area_m2': S,
                                                  'fmax': FMAX, 'ppw': PPW, 'duration_s': DUR}, indent=1))
    L = [f'{ROOM}, {len(rows) // len(BANDS)} LS/MP pairs, boundary {BC} {K or ""}. V = {vol:.1f} m3, S = {S:.1f} m2. FDTD fmax {FMAX:.0f} Hz, PPW {PPW}, {DUR} s.', '',
         '| band | metric | measured | FDTD | Eyring | FDTD err | Eyring err | target | FDTD |',
         '|---|---|---|---|---|---|---|---|---|']
    for fc in BANDS:
        for m in ['T30', 'EDT', 'C80']:
            s = summ[fc][m]; u = ' dB' if m == 'C80' else ' s'
            fe = f"{s['fdtd_err']:.2f} dB" if m == 'C80' else f"{100 * s['fdtd_err']:.1f} %"
            ee = f"{s['eyring_err']:.2f} dB" if m == 'C80' else f"{100 * s['eyring_err']:.1f} %"
            tg = f"<= {TARGET[m]:.0f} dB" if m == 'C80' else f"<= {100 * TARGET[m]:.0f} %"
            verdict = ('PASS' if s['fdtd_pass'] else 'FAIL') if fc in JUDGED else 'not judged'
            L.append(f"| {fc} | {m} | {s['meas_mean']:.2f}{u} | {s['fdtd_mean']:.2f}{u} | {s['eyring']:.2f}{u} "
                     f"| {fe} | {ee} | {tg} | {verdict} (n={s['n']}) |")
    (OUT / 'results.md').write_text('\n'.join(L) + '\n')
    print('\n'.join(L))


if __name__ == '__main__':
    if sys.argv[1] == 'run':
        run(sys.argv[2:] or ('LS1', 'LS2'))
    else:
        analyze()
