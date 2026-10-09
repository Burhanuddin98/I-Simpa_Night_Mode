"""SPPS (verified spps-gpu, Night Mode's own run) vs FDTD (chamber-calibrated) vs Eyring vs BRAS measured, per band.

  python spps_compare.py <ROOM> <simpa results --json file>

Measured metrics use probe.metrics (the same code as every FDTD arm). SPPS values are Night Mode's own
parameters from `simpa results --json` (point_receivers[].per_source[].bands[].parameters).
"""
import json, os, sys
from pathlib import Path

import numpy as np
import scipy.io.wavfile as wavfile

room, res_file = sys.argv[1], Path(sys.argv[2])
os.environ['PROBE_ROOM'] = room
import probe   # noqa: E402  (reads PROBE_ROOM at import)

FD = Path(r'C:\tmp\nm-fdtd-probe') / ('out_CR2_curve_1000' if room == 'CR2' else f'out_{room}_curve_400')
BANDS = [125, 250, 500]
KEY = {'T30': 't30_s', 'EDT': 'edt_s', 'C80': 'c80_db'}

d = json.loads(res_file.read_text())
spps = {}
for r in d['spps']['point_receivers']:
    for ps in r['per_source']:
        for b in ps['bands']:
            p = b.get('parameters', {})
            spps[(ps['source'], r['label'], int(b['freq_hz']))] = {
                m: (p.get(k, {}) or {}).get('value') for m, k in KEY.items()}

ey, _, _ = probe.eyring_sabine()
rows = []
for src in ['LS1', 'LS2']:
    z = np.load(FD / f'sim_{src}' / 'irs.npz')
    for k, mp in enumerate(list(z['receivers'])):
        fsm, xm = wavfile.read(probe.MEAS / f'{probe.MEAS_TAG}_{src}_{mp}_Dodecahedron.wav')
        xm = xm.astype(np.float64) if xm.ndim == 1 else xm[:, 0].astype(np.float64)
        for fc in BANDS:
            if fc == 500 and room != 'CR2':   # CR1/CR3/CR4 grids are fmax 400 Hz: no valid 500 Hz octave
                fd = {'T30': np.nan, 'EDT': np.nan, 'C80': np.nan}
            else:
                fd = probe.metrics(z['irs'][k], float(z['fs']), fc, noisy=False)
            rows.append({'pair': f'{src}-{mp}', 'band': fc, 'meas': probe.metrics(xm, fsm, fc, noisy=True),
                         'fdtd': fd, 'spps': spps.get((src, mp, fc), {})})

L = [f'{room}: measured / SPPS / FDTD (chamber-calibrated) / Eyring, means over pairs; per-pair error in brackets.', '',
     '| band | metric | measured | SPPS | FDTD | Eyring | SPPS err | FDTD err | Eyring err | best |', '|---|---|---|---|---|---|---|---|---|---|']
summary = {}
for fc in BANDS:
    R = [r for r in rows if r['band'] == fc]
    for m in ['T30', 'EDT', 'C80']:
        mv = np.array([r['meas'][m] for r in R], float)
        sv = np.array([np.nan if r['spps'].get(m) is None else r['spps'][m] for r in R], float)
        fv = np.array([r['fdtd'][m] for r in R], float)
        e = ey[fc]['C80_Eyring'] if m == 'C80' else ey[fc]['Eyring']
        if m == 'C80':
            es, ef, ee = np.nanmean(np.abs(sv - mv)), np.nanmean(np.abs(fv - mv)), np.nanmean(np.abs(e - mv))
            fmt = lambda x: f'{x:.2f} dB'
        else:
            es, ef, ee = (np.nanmean(np.abs(v - mv) / mv) for v in (sv, fv, np.full_like(mv, e)))
            fmt = lambda x: f'{100 * x:.1f} %'
        errs = {'SPPS': es, 'FDTD': ef, 'Eyring': ee}
        best = min((v, k) for k, v in errs.items() if np.isfinite(v))[1]
        summary[f'{fc}_{m}'] = {k: float(v) for k, v in errs.items()}
        u = ' dB' if m == 'C80' else ' s'
        L.append(f'| {fc} | {m} | {np.nanmean(mv):.2f}{u} | {np.nanmean(sv):.2f}{u} | {np.nanmean(fv):.2f}{u} | {e:.2f}{u} '
                 f'| {fmt(es)} | {fmt(ef) if np.isfinite(ef) else "-"} | {fmt(ee)} | {best} |')
out = FD.parent / f'compare_{room}.md'
out.write_text('\n'.join(L) + '\n')
(FD.parent / f'compare_{room}.json').write_text(json.dumps({'rows': rows, 'summary': summary}, indent=1, default=float))
print('\n'.join(L))
