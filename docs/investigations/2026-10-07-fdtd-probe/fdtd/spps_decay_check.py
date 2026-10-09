"""Is SPPS's long T30 in Night Mode's parameter code or in the SPPS physics?

For each scattering-1 run: Night Mode's reported t30_s (mean over receiver x source) against T30 recomputed here
from the same raw per-band energy echograms (Schroeder from the first nonzero bin, -5..-35 dB), against the
room-wide total-energy decay, against SPPS's own Kuttruff reference.
"""
import json
from pathlib import Path

import numpy as np

RUNS = Path(r'C:\tmp\nm-spps-runs')


def t30(L, t):
    sel = (L <= -5) & (L >= -35)
    return -60.0 / np.polyfit(t[sel], L[sel], 1)[0] if sel.sum() > 5 else float('nan')


for name in ['cr2_s1', 'cr3_s1', 'cr4_base_s1']:
    d = json.loads((RUNS / f'{name}.json').read_text())
    s = d['spps']; dt = s['time_step_s']
    ref = {b['freq_hz']: (b.get('kuttruff_s') or {}).get('value') for b in s['reference']['bands']}
    tot = {b['freq_hz']: b.get('energy') for b in s['total_energy']}
    parts = []
    for fc in [125, 250, 500]:
        nm, mine = [], []
        for r in s['point_receivers']:
            for ps in r['per_source']:
                b = next(x for x in ps['bands'] if int(x['freq_hz']) == fc)
                nm.append(b['parameters']['t30_s']['value'])
                e = np.array(b['energy_pa2'], float); i0 = int(np.argmax(e > 0))
                edc = np.cumsum(e[i0:][::-1])[::-1]
                mine.append(t30(10 * np.log10(edc / edc[0] + 1e-300), np.arange(len(edc)) * dt))
        g = 'n/a'
        E = tot.get(fc)
        if isinstance(E, list) and len(E) > 10:
            E = np.array(E, float); k = int(np.argmax(E)); E = E[k:] / E[k]
            g = f'{t30(10 * np.log10(E + 1e-300), np.arange(len(E)) * dt):.2f}'
        parts.append(f'{fc}: NM {np.nanmean(nm):.2f} mine {np.nanmean(mine):.2f} global {g} kuttruff {ref[fc]:.2f}')
    print(f'{name:12s} ' + ' | '.join(parts))
print('total_energy entry keys:', list(d['spps']['total_energy'][0].keys()),
      type(d['spps']['total_energy'][0].get('energy')).__name__)
