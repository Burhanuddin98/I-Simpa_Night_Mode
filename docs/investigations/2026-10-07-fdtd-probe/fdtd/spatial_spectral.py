"""What a wave solver gives that Eyring cannot: per-position variation and the low-frequency transfer function.

  python spatial_spectral.py [out_dir]      (default out = the Paris arm)

1. Per-position: for each band, correlate FDTD's per-pair EDT/C80 deviation from its 10-pair mean with the
   measured deviation (Eyring has one value per room, so its correlation is undefined: zero skill).
2. Spectrum: per pair, Pearson correlation of 1/24-octave-smoothed magnitude (dB) over 40-250 Hz, FDTD vs its
   own measured pair, against a null of FDTD vs the OTHER 9 measured pairs.
"""
import json, math, sys
from pathlib import Path

import numpy as np
import scipy.io.wavfile as wavfile
from scipy import signal

OUT = Path(sys.argv[1]) if len(sys.argv) > 1 else Path(r'C:\tmp\nm-fdtd-probe\out')
MEAS = Path(r'C:\tmp\bras_dl\targets\CR2')
F_LO, F_HI = 40.0, 250.0


def smoothed_db(x, fs, fgrid):
    n = 1 << int(math.ceil(math.log2(len(x))))
    X = np.abs(np.fft.rfft(x, n)) ** 2
    f = np.fft.rfftfreq(n, 1 / fs)
    out = []
    for fc in fgrid:
        sel = (f >= fc * 2 ** (-1 / 48)) & (f <= fc * 2 ** (1 / 48))
        out.append(10 * np.log10(X[sel].mean() + 1e-300))
    v = np.array(out)
    return v - v.mean()


res = json.loads((OUT / 'results.json').read_text())
rows = res['rows']
lines = [f'Out: {OUT}', '', '## Per-position skill (Pearson r of deviation from the 10-pair mean, FDTD vs measured)', '',
         '| band | EDT r | C80 r | T30 r |', '|---|---|---|---|']
for fc in [125, 250, 500]:
    R = [r for r in rows if r['band'] == fc]
    cs = []
    for m in ['EDT', 'C80', 'T30']:
        mv = np.array([r['meas'][m] for r in R]); fv = np.array([r['fdtd'][m] for r in R])
        ok = np.isfinite(mv) & np.isfinite(fv)
        cs.append(np.corrcoef(mv[ok] - mv[ok].mean(), fv[ok] - fv[ok].mean())[0, 1])
    lines.append(f'| {fc} | {cs[0]:+.2f} | {cs[1]:+.2f} | {cs[2]:+.2f} |')

fgrid = np.geomspace(F_LO, F_HI, int(24 * math.log2(F_HI / F_LO)))
meas, fdtd, names = [], [], []
for src in ['LS1', 'LS2']:
    z = np.load(OUT / f'sim_{src}' / 'irs.npz')
    for k, mp in enumerate(list(z['receivers'])):
        fsm, xm = wavfile.read(MEAS / f'CR2_RIR_{src}_{mp}_Dodecahedron.wav')
        meas.append(smoothed_db(xm.astype(np.float64), fsm, fgrid))
        fdtd.append(smoothed_db(z['irs'][k], float(z['fs']), fgrid))
        names.append(f'{src}-{mp}')
own = [np.corrcoef(fdtd[i], meas[i])[0, 1] for i in range(len(names))]
null = [np.corrcoef(fdtd[i], meas[j])[0, 1] for i in range(len(names)) for j in range(len(names)) if j != i]
lines += ['', f'## Spectrum {F_LO:.0f}-{F_HI:.0f} Hz, 1/24-octave smoothing (Pearson r of dB magnitude)', '',
          '| pair | FDTD vs own measured |', '|---|---|']
lines += [f'| {n} | {r:+.2f} |' for n, r in zip(names, own)]
lines += ['', f'Own-pair mean r = {np.mean(own):+.2f}; null (FDTD vs the other 9 measured pairs) mean r = '
          f'{np.mean(null):+.2f}, 95th pct {np.percentile(null, 95):+.2f}.']
(OUT / 'spatial_spectral.md').write_text('\n'.join(lines) + '\n')
print('\n'.join(lines))
