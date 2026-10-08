"""PREREG-5 scoring for one room: arms S, H, Hp (Night Mode's own parameters from `simpa results --json`) against
the BRAS measurement (probe.metrics, noisy=True, the code of every earlier phase), bands 500-4000 Hz.

  python score.py <ROOM>      writes score_<ROOM>.json; also the early part's completeness per pair
"""
import json, os, sys
from pathlib import Path

import numpy as np
import scipy.io.wavfile as wavfile

ROOM = sys.argv[1]
os.environ['PROBE_ROOM'] = ROOM
sys.path.insert(0, r'C:\tmp\nm-fdtd-probe')
import probe  # noqa: E402

HERE = Path(__file__).parent
BANDS = [500, 1000, 2000, 4000]
KEY = {'T30': 't30_s', 'EDT': 'edt_s', 'C80': 'c80_db'}


def params(path):
    d = json.loads(Path(path).read_text())
    out = {}
    for r in d['spps']['point_receivers']:
        for ps in r['per_source']:
            for b in ps['bands']:
                p = b.get('parameters', {})
                out[(ps['source'], r['label'], int(b['freq_hz']))] = {
                    m: (p.get(k, {}) or {}).get('value') for m, k in KEY.items()}
    return out, d


arms = {a: params(HERE / 'runs' / f'{ROOM}_{a}.json')[0] for a in ['S', 'H', 'Hp']}
_, d = params(HERE / 'runs' / f'{ROOM}_S.json')
s = d['spps']; c = s['speed_of_sound_m_s']
z = np.load(HERE / f'arrivals_{ROOM}.npz')
t_tr, top = float(z['t_tr']), int(z['order_ray'])
pos = {x['name']: np.array(x['position_m']) for x in s['sources']}
rows, comp = [], []
for r in s['point_receivers']:
    for src in pos:
        mp = r['label']
        t_dir = np.linalg.norm(pos[src] - np.array(r['position_m'])) / c
        sel = (z['src'] == src) & (z['rcv'] == mp)
        tt = z['length'][sel] / c; od = z['order'][sel]
        win = tt < t_dir + t_tr
        comp.append({'pair': f'{src}-{mp}', 'in_window_per_order': [int(((od == k) & win).sum()) for k in range(top + 1)],
                     'top_order_in_window': int(((od == top) & win).sum()), 'window_end_ms': 1e3 * (t_dir + t_tr)})
        fsm, xm = wavfile.read(probe.MEAS / f'{probe.MEAS_TAG}_{src}_{mp}_Dodecahedron.wav')
        xm = xm.astype(np.float64) if xm.ndim == 1 else xm[:, 0].astype(np.float64)
        for fc in BANDS:
            rows.append({'pair': f'{src}-{mp}', 'band': fc, 'meas': probe.metrics(xm, fsm, fc, noisy=True),
                         **{a: arms[a].get((src, mp, fc), {}) for a in arms}})
(HERE / f'score_{ROOM}.json').write_text(json.dumps({'rows': rows, 'completeness': comp, 't_tr': t_tr,
                                                     'order_ray': top}, indent=1, default=float))
for x in comp:
    print(f'{x["pair"]}: window ends {x["window_end_ms"]:.1f} ms, valid arrivals in window per order {x["in_window_per_order"]}')
for fc in BANDS:
    R = [x for x in rows if x['band'] == fc]
    line = [f'{ROOM} {fc:4d} Hz']
    for m in ['EDT', 'C80', 'T30']:
        mv = np.array([x['meas'][m] for x in R], float)
        for a in ['S', 'H', 'Hp']:
            v = np.array([np.nan if x[a].get(m) is None else x[a][m] for x in R], float)
            err = np.nanmean(np.abs(v - mv)) if m == 'C80' else 100 * np.nanmean(np.abs(v - mv) / mv)
            line.append(f'{m} {a} {err:.2f}{" dB" if m == "C80" else " %"}')
    print(' | '.join(line))
