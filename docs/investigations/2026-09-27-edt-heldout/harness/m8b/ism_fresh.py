"""ISM-fresh (HARNESS-PLAN.md P20-P22, P33): specular boxes through the corpus's validated image-source
generator, target/agents/followup-design/skeptic-gf3/ism.py (sha256 a7c9d41e...), copied by text,
with the series built as target/agents/followup-design/spec/eval_ism.py:59-88 builds them.

STUB IN PART (HARNESS-PLAN.md section 6, step 3): draw() runs its held-out guard first (step 4, for
T13) and then returns None, and BOUNDS is empty, until the ISM-fresh step. make_row is built (step 6,
with the truth, for T6). Tests T6, T13, T16 and T22 hold the contract below.

GENERATOR, m8b/_ism.py, is ism.py byte for byte (5,340 bytes, 113 lines, LF, as in the source).
generator() executes it only after its bytes, any CRLF made LF, hash to ISM_SHA256 (VoidRun
otherwise, with checked_sha256 and checked_path set and nothing of it executed); it checks on every
call and executes the copy once per path and process, as upstream.port() does. make_row re-types the
corpus's construction, the scratchpad's edtsimp/loaders.py:245-282 (load_ism, which made
final/dev/ism_rows.pkl) with eval_ism.py:64-71 and 78-86, in the same operations and order, and reads
the truth with truth.truth_ideal. T6 holds it to 12 corpus rows to 1e-9; on the corpus's own inputs it
gives all 1,920 rows bit for bit (provenance.json lists the copy, the construction and that receipt).

Contract:
- A room is a dict with 'dims_m' [Lx, Ly, Lz], 'alpha_walls' [x0, x1, y0, y1, floor, ceiling]
  and 'source_m', as corpus_rooms.json's ISM rooms are.
- make_row(room, rec, R, F, step_ms, image_time_s=None, run_s=None) -> dict(bins, dt, t_arrival,
  half_width, truth_edt): ism.echogram on the 0.02 ms grid (path bin C * 2e-5) over the image set
  (image_time_s, or P22's for the room); the series with air applied per step
  (air_factor(..., step)), summed into the step's bins; t_arrival = d / C, half_width = R / C,
  C = 343.20001220703125 (attack_ism.py:28). run_s None: the corpus's form, the series cut after
  its last bin with energy (the scratchpad's edtsimp/loaders.py:270-281). Otherwise the series is
  run_s long (P22: 2.0 s). truth_edt: truth.truth_ideal of the direct and reflected series at
  0.02 ms with continuous air (P20), element 0.
- draw(seed, *, a1=None) -> dict(seed, rooms, rejections): calls driver.require_not_heldout(seed, a1)
  first. 12 rooms (P21): four relatives, one of each corpus ISM room (t1, corridor, dead, deader),
  each dimension times its own factor from U[1.12, 1.33], each wall's alpha times its own factor
  from U[0.9, 1.1], the source scaled with the room; and eight drawn rooms. Every room fresh per
  P3 and at most MAX_IMAGES images; rejections counted by reason. A room dict also carries 'id',
  'kind' ('relative' or 'drawn'), 'parent' (the corpus room, or None), 'image_time_s',
  'design_t60_s' {band: P5's T60}, and 'receivers': 12 of {'position_m', 'R_m', 'd_m', 'class'}
  (P22: 4 near, 4 mid, 4 far where the room allows, otherwise mid).
- BOUNDS: {quantity: (lo, hi)} in weak_spots.json's vocabulary, for every quantity this set draws
  (P21, P22): its ranges, widened where P33 needs it. 'alpha_walls' bounds each wall's value.

Where make_row's contract is silent:
- "P22's for the room" is the room's own 'image_time_s', which draw() sets by P22 (a corpus ISM room
  carries the corpus's). A room without one, and no image_time_s, is refused with ValueError.
- The step is k whole fine bins, k = round(step_ms / 0.02 ms), and dt = k * 2e-5, as the corpus
  made it; a step under one fine bin is refused.
- With run_s, the series is round(run_s / dt) bins, and the image set must reach that far (P22 sets it
  to at least 1.2 times the run): ValueError otherwise. Without it, a series with no energy is refused
  with ValueError (loaders.py skipped such a row; an echogram always holds the direct sound).
- truth_edt is that of the whole image set at 0.02 ms, as the corpus's was, whatever run_s is. An
  exception from the generator or the truth is raised, not turned into a row without a truth as
  loaders.py's except did; truth_edt is NaN when the truth's line has no fit.
"""
import math
from pathlib import Path

import numpy as np

from . import driver, truth
from .corpus import VoidRun, exec_module, sha256_bytes  # one VoidRun for the harness

C = 343.20001220703125              # attack_ism.py:28
DT_FINE = 2e-5                      # attack_ism.py:30
BANDS_HZ = (125, 250, 500, 1000, 2000, 4000, 8000, 16000, 20000)
STEPS_MS = (1.0, 2.0, 5.0)
RUN_S = 2.0
MAX_IMAGES = 1e8
HELDOUT_SEED = 2026100102           # P21
BOUNDS = {}

HERE = Path(__file__).resolve().parent
GENERATOR = HERE / '_ism.py'
ISM_SOURCE = 'target/agents/followup-design/skeptic-gf3/ism.py'
ISM_SHA256 = 'a7c9d41ec61dd119a14583faaa98d4a88c98e897a0cf4a8d69c6d151d2cef747'    # P20

_LOADED = {}        # resolved path -> the module executed from that path's checked bytes


def generator(path=None):
    p = Path(GENERATOR if path is None else path).resolve()
    data = p.read_bytes().replace(b'\r\n', b'\n')
    h = sha256_bytes(data)
    if h != ISM_SHA256:
        e = VoidRun('%s has sha256 %s with its line ends made LF, not %s: it is not the validated generator '
                    '(P20), and nothing of it was executed' % (p, h, ISM_SHA256))
        e.checked_sha256 = h
        e.checked_path = p
        raise e
    mod = _LOADED.get(p)
    if mod is None:
        mod = exec_module('m8b_ism', p, data)
        mod.checked_sha256 = h
        mod.checked_path = p
        _LOADED[p] = mod
    return mod


def _box(room):
    L = tuple(float(x) for x in room['dims_m'])
    a = [float(x) for x in room['alpha_walls']]
    src = tuple(float(x) for x in room['source_m'])
    if len(L) != 3 or len(a) != 6 or len(src) != 3:
        raise ValueError('a room has 3 dimensions, 6 wall absorptions and a 3-D source')
    return L, [(a[0], a[1]), (a[2], a[3]), (a[4], a[5])], src


def make_row(room, rec, R, F, step_ms, image_time_s=None, run_s=None):
    ism = generator()
    L, alpha, src = _box(room)
    rec = tuple(float(x) for x in rec)
    T = room.get('image_time_s') if image_time_s is None else image_time_s
    if T is None:
        raise ValueError('no image set: pass image_time_s, or a room that carries its own (P22)')
    k = int(round(step_ms * 1e-3 / DT_FINE))
    if k < 1:
        raise ValueError('a step of %r ms is under one 0.02 ms fine bin' % (step_ms,))
    # loaders.py:245-252 and eval_ism.py:64-71: the echogram on the fine grid, split direct/reflected.
    d = math.dist(src, rec)
    t = d / C
    h = R / C
    dl = C * DT_FINE
    direct, refl = ism.echogram(L, src, rec, R, alpha, C * float(T), dl)
    # loaders.py:261-268: the truth at 0.02 ms with continuous air (P20).
    m = ism.m_energy(F)
    ac = ism.air_factor(len(direct), dl, m, C, None)
    truth_edt = truth.truth_ideal(direct * ac, refl * ac, t, DT_FINE)[0]
    # loaders.py:271-282 and eval_ism.py:78-86: air per step, summed into the step's bins.
    vc = (direct + refl) * ism.air_factor(len(direct), dl, m, C, k * DT_FINE)
    dt = k * DT_FINE
    if run_s is None:
        n = len(vc) // k
        vb = vc[:n * k].reshape(n, k).sum(1)
        nz = np.nonzero(vb > 0)[0]
        if len(nz) == 0:
            raise ValueError('the series has no energy')
        vb = vb[:int(nz[-1]) + 1]
    else:
        n = int(round(run_s / dt))
        if n < 1 or n * k > len(vc):
            raise ValueError('a run of %r s needs %d fine bins; the image set of %r s holds %d' % (run_s, n * k, T, len(vc)))
        vb = vc[:n * k].reshape(n, k).sum(1)
    return dict(bins=np.asarray(vb, dtype=np.float64), dt=float(dt), t_arrival=float(t), half_width=h,
                truth_edt=float(truth_edt))


def draw(seed, *, a1=None):
    driver.require_not_heldout(seed, a1)
    return None
