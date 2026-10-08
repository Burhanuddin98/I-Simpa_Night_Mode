"""How a .recp float maps to results-json energy_pa2, and what the direct sound's energy is in those units.

  python units.py <results json>
"""
import json, sys
from pathlib import Path

import numpy as np

sys.path.insert(0, r'C:\repos\Room-Acoustics-Engine\I-Simpa FDTD\solvers\spps-gpu\bed')
from recp_totals import read_gabe  # noqa: E402

d = json.loads(Path(sys.argv[1]).read_text())
s = d['spps']
c = s['speed_of_sound_m_s']
src_pos = {x['name']: (np.array(x['position_m']), x['band_power_w']) for x in s['sources']}
for r in s['point_receivers'][:2]:
    for ps in r['per_source']:
        cols = read_gabe(str(Path(d['run_folder']) / 'solve' / ps['file']))
        print(r['label'], ps['source'], [(lab, len(v)) for lab, v in cols])
        pos, W = src_pos[ps['source']]
        dist = np.linalg.norm(pos - np.array(r['position_m']))
        for k, b in enumerate(ps['bands']):
            lab, v = cols[k]
            v = np.array(v); e = np.array(b['energy_pa2']); i = e > 0
            ratio = e[i] / v[:len(e)][i]
            i0 = int(np.argmax(e > 0))
            direct = e[i0:i0 + 3].sum()
            print(f'  {lab:>10s} {b["freq_hz"]:5d} Hz  pa2/recp median {np.median(ratio):.6g} spread {np.ptp(ratio) / np.median(ratio):.2e}'
                  f'  first 3 bins {direct:.4g}  rho c W/(4 pi d^2) {1.2 * c * W[k] / (4 * np.pi * dist ** 2):.4g}'
                  f'  d {dist:.2f} m  arrival*c {ps["arrival_s"] * c:.2f} m')
