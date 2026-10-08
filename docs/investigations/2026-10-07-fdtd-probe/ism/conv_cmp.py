"""Ray convergence on CR3 LS1: arrivals_CR3.npz (100 k rays) against arrivals_CR3_1000k_LS1.npz (1 M rays).
New valid paths per order and receiver, and gate 3c's E_ISM/E_SPPS at t_c (scattering-0 run) for both sets.
t_c is held at the 100 k set's value so the two ratios cover the same window."""
import json
from pathlib import Path

import numpy as np

HERE = Path(__file__).parent
REPO = Path(r'C:\repos\Room-Acoustics-Engine\I-Simpa FDTD')
RHO_C = 413.25
a = np.load(HERE / 'arrivals_CR3.npz'); b = np.load(HERE / 'arrivals_CR3_1000k_LS1.npz')
assert int(a['n_planes']) == int(b['n_planes'])
d = json.loads(Path(r'C:\tmp\nm-spps-runs\cr3_s0.json').read_text()); s = d['spps']
c, dt = s['speed_of_sound_m_s'], s['time_step_s']
p = json.loads((REPO / 'app/src-tauri/examples/bras_cr3.simpa').read_text(encoding='utf-8'))
mog = {g['id']: g['material'] for g in p['surface_groups']}; mats = {m['id']: m for m in p['materials']}
pg = a['plane_group']; top = int(a['order_ray'])
air = {int(x['freq_hz']): x['air_m_per_metre'] for x in s['reference']['bands']}
W = next(x for x in s['sources'] if x['name'] == 'LS1')['band_power_w']


def energy(z, rx, bi, k_end):
    sel = (z['src'] == 'LS1') & (z['rcv'] == rx) & (z['order'] < top)
    E = 0.0
    for L, pl in zip(z['length'][sel], z['planes'][sel]):
        if np.floor(L / c / dt) <= k_end:
            f = np.prod([1 - mats[mog[pg[q]]]['absorption'][bi] for q in json.loads(pl)])
            E += RHO_C * W[bi] / (4 * np.pi * L ** 2) * np.exp(-air[int(p['bands']['frequencies_hz'][bi])] * L) * f
    return E


for r in s['point_receivers']:
    rx = r['label']
    sa = {(int(o), x) for o, x, s_, r_ in zip(a['order'], a['planes'], a['src'], a['rcv']) if s_ == 'LS1' and r_ == rx}
    sb = {(int(o), x) for o, x, s_, r_ in zip(b['order'], b['planes'], b['src'], b['rcv']) if s_ == 'LS1' and r_ == rx}
    new = sb - sa; lost = sa - sb
    per = [sum(1 for o, _ in new if o == k) for k in range(top + 1)]
    sel = (a['src'] == 'LS1') & (a['rcv'] == rx)
    t_c = a['length'][sel & (a['order'] == top)].min() / c - dt
    kc = int(np.floor(t_c / dt))
    e = np.array(next(x for x in next(ps for ps in r['per_source'] if ps['source'] == 'LS1')['bands'] if int(x['freq_hz']) == 500)['energy_pa2'], float)
    es = e[:kc + 1].sum()
    ra, rb = energy(a, rx, 2, kc), energy(b, rx, 2, kc)
    print(f'LS1-{rx}: new valid paths per order {per} (lost {len(lost)}); 500 Hz E_ISM/E_SPPS at t_c {1e3 * t_c:.0f} ms: '
          f'100k {10 * np.log10(ra / es):+.3f} dB -> 1M {10 * np.log10(rb / es):+.3f} dB')
