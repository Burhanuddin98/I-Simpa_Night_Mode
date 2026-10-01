"""The P6 truth-feature report (PREREG-2.md; HARNESS-PLAN-2.md section 9 M2).

Three of the G rooms were designed for a feature that only a truth run can confirm. This module reads the summed truth
references (the K = 4 truth runs of the room, summed bin by bin, exactly as truth.assess sums them) and reports, at 1 kHz:
- G2: truth T30 >= 2.5 s;
- G3: truth T30 / truth EDT >= 1.25 (a double slope) at at least half of its receivers;
- G7: truth T30 <= 0.25 s.
Each is reported with its value, its target and met / missed (RESULTS.md). It decides nothing and removes no room (M2): a
feature that misses is reported as missing, and the room is scored as drawn.

T30 is ISO 3382-1's: the Schroeder backward integral of the summed energy series, in dB relative to its start, and a
least-squares line through the points between -5 and -35 dB; T30 = -60 / slope. A series that has no energy, whose curve
does not reach -35 dB, or whose last tenth still holds more than P18's 1e-6 of its energy (truncated: the backward
integral of a cut-off tail falls to -inf and fakes a decay), has no T30 (nan), and a nan feature is "missed". The EDT is the harness's own truth EDT
(truth.read, Definition A) on the same summed series, so a blocked receiver reads as truth.assess reads it.
"""
import math
from pathlib import Path

import numpy as np

from m8b import driver, rooms2, truth

BAND_HZ = 1000.0
G2_MIN_T30_S = 2.5
G3_MIN_RATIO = 1.25
G3_MIN_SHARE = 0.5
G7_MAX_T30_S = 0.25
ROOMS = ('G2', 'G3', 'G7')
TARGETS = {'G2': '>= 2.5 s', 'G7': '<= 0.25 s', 'G3': '>= 1.25 at at least half the receivers'}


def t30(bins, dt):
    """ISO 3382-1 T30 of an energy series, seconds; nan when the Schroeder curve has no -5..-35 dB span."""
    e = np.asarray(bins, dtype=np.float64)
    if e.ndim != 1 or not len(e) or not float(e.sum()) > 0:
        return float('nan')
    if not truth.last10_share(e) <= truth.TRUNC_SHARE:         # P18's truncation rule: a cut-off tail fakes a falling curve
        return float('nan')
    curve = np.cumsum(e[::-1])[::-1]
    with np.errstate(divide='ignore', invalid='ignore'):
        db = 10.0 * np.log10(curve / curve[0])
    if not float(db.min()) <= -35.0:
        return float('nan')
    sel = np.isfinite(db) & (db <= -5.0) & (db >= -35.0)
    if int(sel.sum()) < 3:
        return float('nan')
    t = np.arange(len(e), dtype=np.float64)[sel] * float(dt)
    slope = np.polyfit(t, db[sel], 1)[0]
    return float(-60.0 / slope) if slope < 0 else float('nan')


def _summed(rec):
    arrs = [np.asarray(r, dtype=np.float64) for r in rec['refs']]
    n = min(len(a) for a in arrs)
    return sum(a[:n] for a in arrs)


def _at_1khz(recs):
    return [r for r in recs if math.isclose(float(r['band_hz']), BAND_HZ)]


def _edt(total, rec):
    try:
        return truth.read(total, rec['dt'], rec['arrival_s'], rec['half_width'], bool(rec.get('blocked')))
    except ValueError:
        return float('nan')


def truth_features(room_series):
    """room_series: {room: [dict(label, band_hz, refs=[the K energy series], dt, arrival_s, half_width, blocked)]}.
    Returns one row per feature: dict(room, feature, value, target, met, ...) and, for G3, n_met, n_receivers, per_receiver."""
    out = []
    for room in ROOMS:
        recs = _at_1khz(room_series.get(room, []))
        if room in ('G2', 'G7'):
            vals = [t30(_summed(r), r['dt']) for r in recs]
            value = vals[0] if vals else float('nan')
            ok = math.isfinite(value) and (value >= G2_MIN_T30_S if room == 'G2' else value <= G7_MAX_T30_S)
            out.append(dict(room=room, feature='T30_1kHz', value=value, target=TARGETS[room], met=bool(ok),
                            n_receivers=len(recs)))
        elif room == 'G3':
            per = []
            for r in recs:
                total = _summed(r)
                t, e = t30(total, r['dt']), _edt(total, r)
                ratio = t / e if math.isfinite(t) and math.isfinite(e) and e > 0 else float('nan')
                per.append(dict(label=r['label'], t30=t, edt=e, ratio=ratio, met=bool(math.isfinite(ratio) and ratio >= G3_MIN_RATIO)))
            n_met = sum(p['met'] for p in per)
            out.append(dict(room='G3', feature='T30_over_EDT_1kHz', value=n_met / len(per) if per else float('nan'),
                            target=TARGETS['G3'], met=bool(per and n_met >= G3_MIN_SHARE * len(per)),
                            n_met=n_met, n_receivers=len(per), per_receiver=per))
    return out


def format_features(rows):
    """One sentence per feature for RESULTS.md: value, target, met or missed."""
    parts = []
    for r in rows:
        verdict = 'met' if r['met'] else 'missed'
        if r['feature'] == 'T30_over_EDT_1kHz':
            ratios = ', '.join('%s %s' % (p['label'], '%.2f' % p['ratio'] if math.isfinite(p['ratio']) else 'n/a')
                               for p in r['per_receiver'])
            parts.append('G3 truth T30/EDT at 1 kHz >= %.2f at %d of %d receivers (%s), target at least half of them: %s' % (
                G3_MIN_RATIO, r['n_met'], r['n_receivers'], ratios, verdict))
        else:
            v = r['value']
            parts.append('%s truth T30 at 1 kHz = %s, target %s: %s' % (
                r['room'], '%.3f s' % v if math.isfinite(v) else 'n/a (no valid T30: short, truncated or empty series)', r['target'], verdict))
    return '; '.join(parts)


def from_runs(data_root, read=None):
    """Read the four truth runs of each of G2, G3 and G7 (reports only; no solver is run), sum them as truth.assess does and
    return truth_features' rows. `read` defaults to driver.read_run."""
    read = read or driver.read_run
    geom = rooms2.rooms()
    runs = {}
    for r in driver.plan():
        if r['kind'] == 'truth' and r['room'] in ROOMS:
            runs.setdefault(r['room'], []).append(r['run_id'])
    series = {}
    for room, ids in runs.items():
        reps = [read(Path(data_root) / i) for i in ids]
        recs = []
        for s0 in reps[0]:
            key = (s0['label'], s0['band_hz'])
            matched = [next((s for s in rep if (s['label'], s['band_hz']) == key), None) for rep in reps]
            if any(s is None for s in matched):
                raise ValueError('room %s, %s at %g Hz: not every truth run carries this receiver-band' % (room, key[0], key[1]))
            idx = int(key[0].lstrip('Rr'))
            recs.append(dict(label=key[0], band_hz=key[1], refs=[s['energy_pa2'] for s in matched], dt=s0['dt'],
                             arrival_s=s0['arrival_s'], half_width=s0['half_width'],
                             blocked=geom[room]['receivers'][idx]['blocked']))
        series[room] = recs
    return truth_features(series)
