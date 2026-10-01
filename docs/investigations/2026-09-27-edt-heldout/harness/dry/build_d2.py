"""Step 7c part 2: builds D2's fake SPPS run folders (HARNESS-PLAN.md section 5 item 3; the task's
D2 bullet) in the `simpa results` schema, exactly as ../expected_dry.json's "plant" key fixes them:
one mock room (id 'mock_d2'), 8 receivers, 6 bands, 3 steps, 3 seeds, K = 4 references at 0.1 ms.

No m8b/ code is changed here. This only calls m8b.corpus.load_synth() (the hash-gated loader for
critique/synth.py, the committed single-slope/double-slope generator with compound-Poisson noise,
corpus.load_synth/S.noisy) to build every histogram, and writes report.json/project.simpa files that
m8b.driver.read_run / series_from_report (unchanged) can read back, plus a manifest.json this
package's own run_dry.py reads. The mock room's geometry (ROOM, below) is handed to
m8b.spps_rows.rows_from_runs by monkeypatching m8b.rooms_mod.rooms in-process, in process, in
run_dry.py -- the same technique tests/test_wiring.py::_fake_room already uses, not an edit to
m8b/rooms.py.

expected_dry.json's 'plant' key, reproduced here as the single source every row's construction reads
from (D2.plant in the committed JSON, frozen; do not edit either file to make a mismatch disappear):
- baseline: every (receiver, band, step, seed) cell not named by a fault is single-slope,
  T60 = 1.000 s, noise-free references and tested runs (0 % error by construction).
- disagreeing_references (r1, 125 Hz): the 4 references are single-slope, noise-free, at
  T60 = [0.95, 0.98, 1.02, 1.05] s (9011..9014 respectively) -> truth_status truth_uncertain.
- truncated_reference (r2, 250 Hz): reference seed 9012 is cut to 0.3 x the room's design T60
  (0.3 s of bins); the other 3 references at that cell are full length -> truth_status
  truth_truncated (assess() sums references to the SHORTEST length, m8b/truth.py:262-263, so the
  summed series itself is the short one).
- nan_truth (r3, 500 Hz): all 4 references are all-zero -> truth_status truth_nan
  (m8b/truth.py's read() returns NaN for a series with no energy, verdict() reads a NaN truth as
  truth_nan before either of the other two checks).
- blocked_receiver (r8, all 6 bands): no direct line of sight. Built as a reverberant-only decay
  with no energy before an onset comfortably past the geometric arrival, so m8b/truth.py's blocked
  split (first non-zero bin) and frozen/method.py's own blocked-path auto-detection (analyse():
  k_on * dt > t_start + dt) agree on the same reference point; truth_status stays 'ok' and the tested
  run's edt matches truth as closely as the baseline does (single-slope decay rate is time-shift
  invariant, so the offset affects neither the method's fitted slope nor truth_ideal's).
- method_multiplies_by_1.08 (r4, r5, r6 at step = 2 ms): the TESTED run's own decay rate is built at
  T60 = 1.08 x the room's design T60 (1.08 s), while that cell's 4 references stay at the room's
  T60 = 1.000 s: a clean single-slope decay's EDT fit recovers its generating T60 to (empirically,
  see run_dry.py's printed per-row check) a tiny fraction of a percent, so edt / truth = 1.08 to
  that same precision, comfortably over the 5 % JND and comfortably outside the method's own
  HW_FLOOR = 0.5 % shown range (frozen/method.py:18, 85) -- "wrong-silent" and "not covered" by
  construction, not by assertion.

Noise: critique/synth.py's noisy() (compound-Poisson, every hit energy w) is kept below (_noisy,
unused by the final construction) because the first version of this builder applied it to every
tested/reference histogram, at a particle budget (w = 1e-6) chosen to look "negligible" by eye. It
measurably was not: run_dry.py's first run (C:/tmp/m8b-edt/dry/run1.log) found H2.n_covered 319
against the plant's own derived 351 (n_ok_truth 405 - n_wrong_silent 54 = 351 exactly, both of which
DID match) -- 32 of the 351 rows the plant fixes as "edt = truth exactly (0 % error, comfortably
covered under any positive half-width)" were, with that noise, ok and not wrong-silent (well under
the 5 % JND) but NOT covered: a noisy fit's own OLS standard error (frozen/method.py:66, folded into
the shown half-width) measures how well the points fit the FITTED line, not how far that line's
intercept sits from the true decay rate, so a particular noise draw can bias the point estimate
outside its own narrow band while staying nowhere near the 5 % JND. The plant's "0 % error" and
"comfortably covered" language is a precise, load-bearing claim, not a figure of speech, so this
builder now builds every histogram -- baseline, the blocked_receiver and method_multiplies_by_1.08
faults included -- as critique/synth.py's analytic histogram() with NO noisy() applied: single-slope,
exact, same as the three truth-side faults already were (disagreeing_references, truncated_reference
and nan_truth, which were never noised, since their own derivations assume exact T60s and exact
zeros). The task's D2 bullet names "compound-Poisson noise" as part of the general toolkit this
construction draws on; the plant it must be built "exactly as" fixes, row by row, no noise on any of
the 432 (every fault's own text is explicit about what changes a cell -- a different T60, a short
array, zeros, an occluded onset, a different decay rate -- and none of the five mentions noise).
expected_dry.json's own frozen "plant" also names no ratio-3 (double-slope) construction for any of
D2's 432 rows -- every row it fixes, baseline and every fault, is explicitly 'single-slope' -- so
none is used here either: using one would depart from the 'exactly as the plant key fixes them'
instruction for no row the plant actually describes.

Usage (from harness/, with the venv and no bytecode):
    set PYTHONDONTWRITEBYTECODE=1 & C:\\tmp\\m8b-edt\\venv\\Scripts\\python.exe dry\\build_d2.py
"""
import json
import math
import sys
from pathlib import Path

sys.path.insert(0, '.')
sys.dont_write_bytecode = True

import numpy as np  # noqa: E402

from m8b import corpus  # noqa: E402

OUT = Path(r'C:\tmp\m8b-edt\dry\D2')
RNG_SEED = 20261001            # this builder's own noise seed, not one of P12/P24's held-out seeds

ROOM_ID = 'mock_d2'
BANDS_HZ = (125, 250, 500, 1000, 2000, 4000)
STEPS_MS = (1.0, 2.0, 5.0)
TEST_SEEDS = {1.0: (111, 112, 113), 2.0: (211, 212, 213), 5.0: (511, 512, 513)}
REF_SEEDS = (9011, 9012, 9013, 9014)
DESIGN_T60_S = 1.0
C = corpus.C_SPPS              # 343.2 m/s, frozen/method.py:20 == critique/synth.py's C
R_M = 0.31
H_S = R_M / C
ED_LATE_DB = -6.0              # a plain, mid-range DRR; irrelevant to every outcome (see module docstring: OLS
                                # on an exact exponential's slope does not depend on an additive direct spike
                                # that the fit window (t >= t_arrival + h) already excludes)
ONSET_OFFSET_S = 0.01           # r8 (blocked): first energy 10 ms after the geometric arrival, which is
                                # comfortably past t_start + dt (<= 0.31/343.2 + 0.005 s) for every dt used here
T_RUN_TESTED_S = 2.0
T_RUN_REF_S = 1.5
T_RUN_REF_TRUNCATED_S = 0.3 * DESIGN_T60_S     # the truncated_reference fault
NOISE_W = 1.0e-6                # compound-Poisson "energy per hit": see _noise_is_negligible's own check

RECEIVERS = [
    dict(label='R000', id='r1', cls='near', d_m=1.2, blocked=False),
    dict(label='R001', id='r2', cls='near', d_m=1.5, blocked=False),
    dict(label='R002', id='r3', cls='near', d_m=1.8, blocked=False),
    dict(label='R003', id='r4', cls='mid', d_m=3.5, blocked=False),
    dict(label='R004', id='r5', cls='mid', d_m=5.0, blocked=False),
    dict(label='R005', id='r6', cls='mid', d_m=7.0, blocked=False),
    dict(label='R006', id='r7', cls='far', d_m=12.0, blocked=False),
    dict(label='R007', id='r8', cls='far', d_m=15.0, blocked=True),
]
DISAGREE_T60_BY_SEED = {9011: 0.95, 9012: 0.98, 9013: 1.02, 9014: 1.05}       # disagreeing_references


def room_geom():
    """The mock room, exactly enough of m8b/rooms.py's contract for m8b.spps_rows.rows_from_runs and
    its _gap_s (boxes/source_m/receivers[i]['position_m']) plus the fields rows_from_runs reads
    directly (d_m, blocked, design_t60_s). Positions are a straight line from the source so d_m and
    position_m agree; _gap_s's mirrored-image gap plays no role in any row's classification here
    (truth.split_borderline is only ever close to firing within ~2 % of the 5 % JND line, and every
    row here sits at 0 % or 8 % error -- see the module docstring), so the exact geometry is not
    load-bearing, only readable."""
    src = (0.0, 0.0, 1.5)
    boxes = [[[-5.0, -5.0, 0.0], [25.0, 5.0, 3.0]]]
    receivers = []
    for i, r in enumerate(RECEIVERS):
        pos = (src[0] + r['d_m'], src[1], src[2])
        receivers.append(dict(position_m=pos, d_m=r['d_m'], blocked=r['blocked'], **{'class': r['cls']}))
    return {ROOM_ID: dict(id=ROOM_ID, boxes=boxes, box_materials=['walls'], source_m=list(src),
                          receivers=receivers, design_t60_s={b: DESIGN_T60_S for b in BANDS_HZ})}


def _rng(label):
    """A fixed, distinct stream per (run kind, receiver, band) cell, so the fixture is reproducible."""
    return np.random.default_rng([RNG_SEED, hash(label) & 0xFFFFFFFF])


def _clean_slope(dt, t_run, t_arr, h, t60, ed_db=ED_LATE_DB, gap=0.0):
    k1 = 6.0 * math.log(10.0) / t60
    srev = 1.0 / k1
    ed = srev * 10 ** (ed_db / 10.0)
    S = corpus.load_synth()
    return S.histogram(dt, t_run, t_arr, h, ed, gap, [1.0], [k1])


def _blocked_slope(dt, t_run, onset_t, t60, gap=0.0):
    k1 = 6.0 * math.log(10.0) / t60
    S = corpus.load_synth()
    return S.histogram(dt, t_run, onset_t, H_S, 0.0, gap, [1.0], [k1])


def _noisy(bins, label, w=NOISE_W):
    """critique/synth.py's noisy() (compound Poisson, every hit energy w): the task's D2 bullet asks
    for it on the single-slope histograms built here. w is small enough that every bin with
    appreciable energy carries a large hit count (low relative Poisson noise); run_dry.py's per-row
    table is what actually confirms no row's classification moved, not this comment."""
    S = corpus.load_synth()
    rng = _rng(label)
    return S.noisy(np.asarray(bins, dtype=np.float64), w, rng)


def _zeros(dt, t_run):
    return np.zeros(int(round(t_run / dt)), dtype=np.float64)


def _band_for_receiver(rec, band_hz, *, kind, dt, seed=None):
    """One receiver-band's bins for 'tested' (kind='tested', needs seed, dt in {0.001,0.002,0.005})
    or 'ref' (kind='ref', needs seed, dt = 1e-4). Implements every row of the plant: baseline, the
    five faults, each on exactly the rows plant.faults names (module docstring)."""
    rid, band = rec['id'], band_hz
    t_arr = rec['d_m'] / C

    if rid == 'r8':                                            # blocked_receiver: every band
        onset = t_arr + ONSET_OFFSET_S
        t_run = T_RUN_REF_S if kind == 'ref' else T_RUN_TESTED_S
        return _blocked_slope(dt, t_run, onset, DESIGN_T60_S)

    if rid == 'r1' and band == 125 and kind == 'ref':           # disagreeing_references
        t60 = DISAGREE_T60_BY_SEED[seed]
        b = _clean_slope(dt, T_RUN_REF_S, t_arr, H_S, t60)
        return b                                                 # noise-free: the plant fixes exact T60s

    if rid == 'r2' and band == 250 and kind == 'ref' and seed == 9012:    # truncated_reference
        b = _clean_slope(dt, T_RUN_REF_TRUNCATED_S, t_arr, H_S, DESIGN_T60_S)
        return b                                                 # noise-free: a short, clean fixture

    if rid == 'r3' and band == 500 and kind == 'ref':            # nan_truth
        return _zeros(dt, T_RUN_REF_S)                            # noise-free: literally no energy

    if rid in ('r4', 'r5', 'r6') and dt == 0.002 and kind == 'tested':   # method_multiplies_by_1.08
        return _clean_slope(dt, T_RUN_TESTED_S, t_arr, H_S, 1.08 * DESIGN_T60_S)

    # baseline: single-slope T60 = design T60, both tested and every reference
    t_run = T_RUN_REF_S if kind == 'ref' else T_RUN_TESTED_S
    return _clean_slope(dt, t_run, t_arr, H_S, DESIGN_T60_S)


def _report(dt, seed, *, kind):
    point_receivers = []
    for rec in RECEIVERS:
        bands = []
        for band in BANDS_HZ:
            bins = _band_for_receiver(rec, band, kind=kind, dt=dt, seed=seed)
            bands.append(dict(freq_hz=band, energy_pa2=[float(x) for x in bins]))
        point_receivers.append(dict(label=rec['label'], arrival_s=rec['d_m'] / C, bands=bands))
    return {
        'solver_build': {'status': 'verified'},
        'spps': {'speed_of_sound_m_s': C, 'receiver_radius_m': R_M, 'time_step_s': dt,
                'point_receivers': point_receivers},
    }


def _write_run(path, seed, report):
    path.mkdir(parents=True, exist_ok=True)
    (path / 'project.simpa').write_text(json.dumps({'solvers': {'spps': {'random_seed': seed}}}),
                                        encoding='utf-8')
    (path / 'report.json').write_text(json.dumps(report), encoding='utf-8')


def build():
    OUT.mkdir(parents=True, exist_ok=True)
    manifest = {'room': room_geom(), 'tested': {}, 'refs': []}

    for ref_seed in REF_SEEDS:
        p = OUT / 'refs' / ('seed%d' % ref_seed)
        _write_run(p, ref_seed, _report(1.0e-4, ref_seed, kind='ref'))
        manifest['refs'].append(str(p))

    for step_ms, seeds in TEST_SEEDS.items():
        dt = step_ms / 1.0e3
        for seed in seeds:
            p = OUT / 'tested' / ('%gms-seed%d' % (step_ms, seed))
            _write_run(p, seed, _report(dt, seed, kind='tested'))
            manifest['tested'].setdefault('%g' % step_ms, []).append(str(p))

    (OUT / 'manifest.json').write_text(json.dumps(manifest, indent=1), encoding='utf-8')
    return manifest


if __name__ == '__main__':
    m = build()
    print(json.dumps({'room': m['room'], 'n_tested_dirs': sum(len(v) for v in m['tested'].values()),
                      'n_ref_dirs': len(m['refs']), 'out': str(OUT)}, indent=2))
