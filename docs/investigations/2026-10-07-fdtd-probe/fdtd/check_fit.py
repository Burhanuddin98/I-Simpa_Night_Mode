"""Achieved random-incidence (Paris) absorption of PFFDTD's fitted DEF admittances vs the target octave alpha."""
import json, sys
from pathlib import Path
import numpy as np, h5py
sys.path.insert(0, r'C:\RoomGUI\pffdtd\python')
from materials.adm_funcs import compute_Rf_from_DEF

simpa = json.loads(Path(r'C:\repos\Room-Acoustics-Engine\I-Simpa FDTD\app\src-tauri\examples\bras_cr2.simpa').read_text(encoding='utf-8'))
th = np.linspace(0, np.pi / 2, 2001)
print(f"{'material':<20}" + ''.join(f'{f:>14}' for f in [63, 125, 250, 500, 1000]))
for m in simpa['materials']:
    h = h5py.File(rf"C:\tmp\nm-fdtd-probe\out\mats\{m['name']}.h5", 'r')
    D, E, F = h['DEF'][()].T if h['DEF'][()].shape[1] == 3 else h['DEF'][()]
    al = dict(zip(simpa['bands']['frequencies_hz'], m['absorption']))
    row = []
    for f in [63, 125, 250, 500, 1000]:
        fv = np.geomspace(f / np.sqrt(2), f * np.sqrt(2), 41)
        _, Yn, _, _ = compute_Rf_from_DEF(1j * 2 * np.pi * fv, D, E, F)
        a = []
        for y in Yn:
            R = (np.cos(th) - y) / (np.cos(th) + y)
            a.append(np.trapezoid((1 - np.abs(R) ** 2) * np.sin(2 * th), th))
        tgt = al.get(f, al[min(al, key=lambda b: abs(np.log(b / f)))])
        row.append(f'{np.mean(a):.3f}/{tgt:.3f}')
    print(f"{m['name']:<20}" + ''.join(f'{r:>14}' for r in row))
print('(achieved / target, octave-averaged)')
