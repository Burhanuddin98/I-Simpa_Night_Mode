"""ISM-fresh (HARNESS-PLAN.md P20-P22, P33): specular boxes through the corpus's validated image-source
generator, target/agents/followup-design/skeptic-gf3/ism.py (sha256 a7c9d41e...), copied by text,
with the series built as target/agents/followup-design/spec/eval_ism.py:59-88 builds them.

Built in HARNESS-PLAN.md section 6, step 6: make_row with the truth (for T6), then draw() and BOUNDS
in the ISM-fresh step (for T16 and T22b). Tests T6, T13, T16 and T22 hold the contract below. draw()
and BOUNDS copy nothing from target/: they are P21's and P22's text, and the image count is the rule
ism.py's own images() follows, so provenance.json has no entry for them.

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

The bounds (BOUNDS; P21 and P22 as the plan now states them). P21's and P22's ranges stand except
one, which P33 widens because weak_spots.json holds a binding value outside it, exactly to that value:
- the source's clearance from every wall, P22's 1.0 m, down to 0.7029691338539124 m: z3grid's
  dis-010 at 1 ms (z3's dis-010 geometry, box032, 8.08 x 5.03 x 3.19 m, with no delay; z3 itself
  ran that case at 0.5 ms, outside this set's steps), wrong-silent at +6.7 %, a 125 Hz receiver with
  R 1.133 m at d 1.533 m. Nine more binding rows have their source under 1.0 m, at 0.90-0.93 m:
  z3's dis-035 (1.5 ms), dis-016, dis-012 and dis-029 (1 ms), and z3grid's dis-013, dis-029 and
  dis-035 at 1 ms and dis-015 at 1 and 2 ms.
Every other binding value lies inside P21's and P22's ranges (240 rows of z3, z3grid, ISM, the real
rows and the seeds at 1-5 ms; C:/tmp/m8b-edt/step6-ism/bind_ism-run1.txt): L 3.0-20.0 m, V 60-240 m3,
alpha 0.05-0.64, T60 at 125 Hz 0.20-0.59 s, R 0.31-1.49 m, d 1.42-13.50 m, d - R from 0.269 m, the
receiver's clearance less R from 0.234 m, bands 125 Hz-20 kHz; so no other bound moves. The tops
P22 does not state are what P21's largest box gives: d up to D_TOP_M, 27.34 m (the source 0.703 m and
a receiver 0.14 m from opposite corners of 24 x 14 x 7 m), d - R up to that less 0.1 m, and the
clearances up to half of L3's 7 m (less R for a receiver). BOUNDS holds the ranges draw() draws from:
the eight drawn rooms' for the room quantities, and every room's for its receivers and its source.
The relatives are P21's factors on their parents and stay inside those ranges too, except the
corridor's length, 22.4-26.6 m against L1's 24 m; BOUNDS leaves that out, so no weak spot is ever
called covered by a relative alone.

Where draw()'s contract is silent:
- np.random.default_rng(SeedSequence(seed)); the four relatives first, in PARENTS' order, then the
  eight drawn rooms. A room is drawn whole, and redrawn whole at the first check it fails, in the
  order below, each failure counted in 'rejections' under its reason (every reason is listed, 0 or
  not, REJECTIONS):
  - a relative: its three dimension factors, then its six alpha factors (x0 x1 y0 y1 floor
    ceiling); P3 ('relative_near_duplicate'); its receivers; the image cap ('relative_over_image_cap').
    The scaled source stays at least SRC_WALL_M from every wall (the parents' clear 1.2-1.47 m);
    ValueError if it did not.
  - a drawn room: L1, L2 and L3 as its x, y and z lengths (sorted, each still lies in its own range,
    which is what T16 checks), the six alphas, and the source uniform over the points at least
    SRC_WALL_M from every wall; then V ('drawn_V_outside'), the design T60 at 125 Hz
    ('drawn_t60_125_outside'), P3 ('drawn_near_duplicate'), its receivers, and the image cap
    ('drawn_over_image_cap').
- Receivers: 4 near (R + 0.2 m <= d < 2 m), 4 mid (2 <= d <= 10 m), then 4 far (d > 10 m) when the
  room allows them, else 4 more mid. A room allows them when a point 1.54 m (the largest R + 0.04 m)
  from every wall lies more than 10.5 m from the source, T16's own threshold: every ball P22 draws
  then has room at least 0.5 m past the 10 m line. Each receiver draws its R first, log-uniform, and
  then its position, uniformly over the points of its class at least R + 0.04 m from every wall.
  Those points lie in a box (the room less R + 0.04 m at every wall, cut to 2 m around the source for
  a near receiver and to 10 m for a mid one), and over a box the distance from the source takes every
  value from its nearest point to its farthest corner; so when no point of the box is in the class,
  which is checked exactly, R is drawn again ('receiver_R_no_room'). Otherwise positions are drawn
  uniformly over the box, BATCH at a time, and the first in the class is taken; the ones before it
  are counted ('receiver_position_outside_class'). After POSITION_CAP of them without one, R is drawn
  again ('receiver_R_position_cap'): the class then holds under about 1 in 2^20 of the box, a sliver.
  Distances are math.dist on the stored floats, as T16 measures them, so no class is a rounding away.
- image_time_s is P22's max(1.2 RUN_S, the largest d / C + 1.1 times the largest design T60 of the
  nine bands), not rounded. 'images' = n_images(dims, C image_time_s + the largest R), what ism.py
  builds for the receiver with the largest ball; over MAX_IMAGES the room is rejected.
- Ids: 'rel:<parent>' and 'drawn:<1-8>'. A room also carries 'far_allowed', 'images', 'src_wall_m',
  'p3' (its smallest P3 gap to a corpus room, and that room) and, for a relative, 'factors'. D also
  carries 'corpus_rooms_sha256', the file P3 was checked against. Every number is a Python float or
  int, and 'design_t60_s' has int band keys.

Added to the contract here; no test reads it:
- make_row's dict also carries 'truth_share', truth.last10_share of the truth's own series (the
  image set at 0.02 ms with continuous air), and 'truth_status', truth.verdict(truth_edt, 0.0,
  share): 'truth_nan', 'truth_truncated' (P22: the truncation check as P18) or 'ok'. The image set
  is one exact series, so P19's u has nothing to measure and is passed as 0.
- HARNESS-PLAN.md 8.2's fourth call: make_row(..., retry_truncated=True) gives a row P18 calls
  truth_truncated one retry, its image set IMAGE_RETRY_FACTOR (1.5) times as long. If the retried
  truth moves by less than IMAGE_RETRY_REL_TOL (1e-3) relative, the row is kept: truth_edt and
  truth_share are the retried values and truth_status is 'ok' unconditionally (convergence is 8.2's
  stated criterion; P18's share test on the longer series does not get to re-decide, since a
  slow-decaying box's tail can still formally exceed TRUNC_SHARE while the truth itself has already
  converged). The dict then carries 'truth_truncated_retry' (dict with image_time_s, truth_edt,
  truth_share and moved_rel) as a receipt. Otherwise the row stays truth_truncated at its original
  image set, and 'truth_truncated_retry' still carries the receipt (what the retry found, for the
  record) with truth_edt, truth_share and truth_status unchanged. A row that is not truth_truncated,
  or for which retry_truncated is left at its default False, carries no 'truth_truncated_retry' key
  and costs one echogram, as before: retry_truncated defaults to False so that T6's bit-for-bit
  reproduction of the corpus's own (pre-8.2) truth_edt values is unaffected by this call; whatever
  builds ISM-fresh's rows for real (draw()/row_specs(), once the driver's matrix is built) is expected
  to pass retry_truncated=True, as 8.2 fixes it going forward.
- row_specs(D) -> the set's rows in a fixed order (room, receiver, band, step), 3,888 for a whole
  draw (P22), each with an 'id' ('ismf|<room>|r<receiver>|<band>|<step>ms') and the fields the
  scorer groups by. n_images, image_time, farthest, design_t60s and in_class are the rules above.
- make_row computes the room's echogram on every call. One per receiver serves all 27 of its rows,
  so the step that makes the 3,888 rows needs 144 echograms, not 3,888.

What the draw gives, on dev seeds only (C:/tmp/m8b-edt/step6-ism/check_ism.py and its
check_ism-*.json, 2026-10-01: 400 draws, seeds 20261001-20261400, 4,800 rooms; the frozen method never
ran). Per draw, 2.89 drawn rooms and 0.34 relatives are redrawn as P3 near-duplicates and 0.02 drawn
rooms for V; the 125 Hz T60 range and the image cap rejected none (with P21's alpha narrowed to
0.02-0.06 in a scratch process, both reject as written). R is drawn again 0.88 times per draw for
want of room in its class. The position cap fired 0.03 times per draw, each time for a mid receiver
in a room the size of dead or deader. 52 % of the drawn rooms, and every corridor relative, have far receivers. 34 % of the drawn sources
sit under 1.0 m from a wall, in the widened part, and 6 % under 0.75 m. R stays log-uniform in every
class (R >= 1.13 m: near 10.2 %, mid 9.6 %, far 10.3 %, against 10.5 %), and about 2 near receivers
per draw have R >= 1.13 m with d - R < 0.4 m, where z3's weak spots sit. Rooms hold up to 7.8e7
images. The design T60 at 125 Hz is 0.10-2.32 s and over 1.5 s in only 2 rooms, so the image set
is 2.4 s in 4,799 rooms and 2.6 s in one.

What P22's image set costs against P18 (dev rooms only; check_trunc.json and check_trunc_sens.json
beside check_ism.json). The image set follows the design T60, Eyring's, but a specular box whose
walls absorb unevenly decays far more slowly late than Eyring says: the corridor's relative, its
walls near 0.05 and its floor near 0.6, has a truth series whose T30 is about 5 s against a design
T60 of 0.7-0.8 s. The last tenth of its 2.4 s image set then holds about 6e-4 of the energy, and
make_row reads truth_truncated (P18). Over the 60 rooms of 5 dev draws, one receiver each, 7 rooms
are truncated at 125 Hz, 6 at 1 kHz, 5 at 4 kHz and none at 16 kHz: the corridor relative in every
draw, at 125 Hz-4 kHz, and 2 of the 40 drawn rooms; the truth series' T30 is a median 1.7 times the
design T60, and up to 7.7 times. In the two dev rooms with the longest design T60 (2.32 and 1.63 s at
125 Hz) every receiver is truncated at 125-500 Hz. On an image set 2 times longer, those two rooms'
truths move by 1.3e-5 and 1.6e-4 relative and pass P18, so there it drops rows whose truth was right.
On one 1.5 times longer (2 times is over 8e7 images), the corridor relative's far receiver moves
+0.8 % at 125 Hz (+0.3 % at 1 kHz) and is still truncated, so there the truth is about 1 % low and
the exclusion protects. Separately, 2 of the 15 near receivers
sampled (d 0.54 and 0.65 m, in absorbing rooms) read truth_nan in every band: the direct sound alone
takes the level past -10 dB, as in Synth-fresh above DRR 9.54 dB. P22 and P18 are the plan's and are
applied as written; HARNESS-PLAN.md 8.2's fourth call now also says what to do about a truncated row:
retry it at 1.5 times the image set and keep it if the truth moves by under 1e-3 relative (above).

What the retry gives, on the two rooms already measured against P18 (above): "step 6 measured 1.3e-5
at 1.5 times and 1.6e-4 at 2 times" (HARNESS-PLAN.md 8.2) for the two dev rooms with the longest
design T60, whose truth moved 1.3e-5 and 1.6e-4 relative on an image set 2 times longer; both pass
IMAGE_RETRY_REL_TOL (1e-3) comfortably at either factor, and 8.2 fixes the cheaper one, 1.5. The
corridor relative's far receiver is a harder case: its truth was still measured moving +0.8 % at
125 Hz on an image set 1.5 times longer (only +0.3 % at 1 kHz), so that particular row's retry does
not converge at 1e-3 and it stays truth_truncated, exactly as 8.2's call intends for a row whose
truth is still moving.

Cost: one echogram (one make_row call) took 2-91 s on the dev rooms' receivers, and 141 s for the
smallest ball in the dev room with the most images (7.8e7), at a peak working set of 7.6 GB, all of
it in ism.images(). A larger ball takes longer: each image's span of fine bins grows with R.
"""
import itertools
import json
import math
import operator
from pathlib import Path

import numpy as np

from . import corpus, driver, rooms, truth
from .corpus import VoidRun, exec_module, sha256_bytes  # one VoidRun for the harness

C = 343.20001220703125              # attack_ism.py:28
DT_FINE = 2e-5                      # attack_ism.py:30
BANDS_HZ = (125, 250, 500, 1000, 2000, 4000, 8000, 16000, 20000)
STEPS_MS = (1.0, 2.0, 5.0)
RUN_S = 2.0
MAX_IMAGES = 1e8
HELDOUT_SEED = 2026100102           # P21

# P21: the rooms.
PARENTS = ('t1', 'corridor', 'dead', 'deader')      # one relative of each corpus ISM room
N_DRAWN = 8
REL_DIMS = (1.12, 1.33)             # each dimension's factor
REL_ALPHA = (0.9, 1.1)              # each wall's alpha's factor
L1_M = (4.0, 24.0)
L2_M = (3.5, 14.0)
L3_M = (2.6, 7.0)
V_M3 = (60.0, 2500.0)
ALPHA = (0.02, 0.80)
T60_125_S = (0.1, 3.0)
# P22: the receivers, the source and the image set.
N_PER_CLASS = 4
R_M = (0.1, 1.5)                    # log-uniform
REC_CLEAR_M = 0.04                  # a receiver at least R + 0.04 m from every wall
NEAR_GAP_M = 0.2                    # a near receiver at d >= R + 0.2 m
NEAR_M, FAR_M = 2.0, 10.0           # near d < 2 m, mid 2-10 m, far d > 10 m
FAR_MARGIN_M = 1.54                 # R_M[1] + REC_CLEAR_M
FAR_REACH_M = 10.5
SRC_WALL_M = 0.7029691338539124     # P22's 1.0 m, widened by P33: z3grid dis-010 at 1 ms
IMAGE_RUN_FACTOR = 1.2
IMAGE_T60_FACTOR = 1.1
BATCH = 64
POSITION_CAP = 1 << 20
IMAGE_RETRY_FACTOR = 1.5            # HARNESS-PLAN.md 8.2, fourth call: a truncated row's one retry
IMAGE_RETRY_REL_TOL = 1e-3          # kept if the retried truth moves by less than this, relative

D_TOP_M = math.hypot(*(L[1] - SRC_WALL_M - (R_M[0] + REC_CLEAR_M) for L in (L1_M, L2_M, L3_M)))
BOUNDS = {
    'L1_m': L1_M,
    'L2_m': L2_M,
    'L3_m': L3_M,
    'V_m3': V_M3,
    'alpha_walls': ALPHA,
    't60_design_125_s': T60_125_S,
    'R_m': R_M,
    'd_m': (R_M[0] + NEAR_GAP_M, D_TOP_M),
    'd_minus_R_m': (NEAR_GAP_M, D_TOP_M - R_M[0]),
    'rec_wall_minus_R_m': (REC_CLEAR_M, L3_M[1] / 2 - R_M[0]),
    'src_wall_m': (SRC_WALL_M, L3_M[1] / 2),
    'band_hz': (BANDS_HZ[0], BANDS_HZ[-1]),
}
REJECTIONS = ('relative_near_duplicate', 'relative_over_image_cap', 'drawn_V_outside', 'drawn_t60_125_outside',
              'drawn_near_duplicate', 'drawn_over_image_cap', 'receiver_R_no_room', 'receiver_position_outside_class',
              'receiver_R_position_cap')

HERE = Path(__file__).resolve().parent
GENERATOR = HERE / '_ism.py'
ISM_SOURCE = 'target/agents/followup-design/skeptic-gf3/ism.py'
ISM_SHA256 = 'a7c9d41ec61dd119a14583faaa98d4a88c98e897a0cf4a8d69c6d151d2cef747'    # P20
CORPUS_ROOMS = corpus.HARNESS / 'corpus_rooms.json'

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


def make_row(room, rec, R, F, step_ms, image_time_s=None, run_s=None, retry_truncated=False):
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
    # P22: the truncation check as P18, on the truth's own series (added to the contract here).
    share = truth.last10_share(direct * ac + refl * ac)
    truth_status = truth.verdict(float(truth_edt), 0.0, share)
    out = dict(bins=np.asarray(vb, dtype=np.float64), dt=float(dt), t_arrival=float(t), half_width=h,
              truth_edt=float(truth_edt), truth_share=share, truth_status=truth_status)
    if truth_status == 'truth_truncated' and retry_truncated:
        # HARNESS-PLAN.md 8.2, fourth call: one retry at IMAGE_RETRY_FACTOR times the image set; kept
        # (truth_status 'ok') if the truth moves by under IMAGE_RETRY_REL_TOL relative.
        T2 = IMAGE_RETRY_FACTOR * float(T)
        direct2, refl2 = ism.echogram(L, src, rec, R, alpha, C * T2, dl)
        ac2 = ism.air_factor(len(direct2), dl, m, C, None)
        truth_edt2 = float(truth.truth_ideal(direct2 * ac2, refl2 * ac2, t, DT_FINE)[0])
        share2 = truth.last10_share(direct2 * ac2 + refl2 * ac2)
        if math.isfinite(truth_edt) and truth_edt != 0 and math.isfinite(truth_edt2):
            moved = abs(truth_edt2 / truth_edt - 1.0)
        else:
            moved = float('inf')
        out['truth_truncated_retry'] = dict(image_time_s=T2, truth_edt=truth_edt2 if math.isfinite(truth_edt2) else None,
                                            truth_share=share2, moved_rel=moved if math.isfinite(moved) else None)
        if moved < IMAGE_RETRY_REL_TOL:
            # Convergence is the kept criterion 8.2's fourth call states; P18's share test on the
            # longer image set does not re-decide (a row can converge while still formally over
            # TRUNC_SHARE, as a slow-decaying box's tail can be, and is kept all the same).
            out['truth_edt'] = truth_edt2
            out['truth_share'] = share2
            out['truth_status'] = 'ok'
    return out


# ---- the draw (P21, P22) -------------------------------------------------------------------------------
def n_images(dims, lmax):
    """How many images ism.images builds for a box out to lmax (ism.py:47-65): per axis n from -nmax to
    nmax and p in (0, 1), nmax = ceil(lmax / 2L) + 1. echogram asks it for lmax = C T + R."""
    return math.prod(2 * (2 * (int(math.ceil(lmax / (2 * float(L)))) + 1) + 1) for L in dims)


def design_t60s(dims, alpha6):
    """P5's design T60 in each of BANDS_HZ (corpus.design_t60: Eyring with ISO 9613-1 air)."""
    return {b: corpus.design_t60(dims, alpha6, float(b)) for b in BANDS_HZ}


def image_time(receivers, design_t60_s):
    """P22: max(1.2 RUN_S, the largest d / C + 1.1 times the largest design T60)."""
    t_arr = max(x['d_m'] for x in receivers) / C
    return max(IMAGE_RUN_FACTOR * RUN_S, t_arr + IMAGE_T60_FACTOR * max(design_t60_s.values()))


def farthest(dims, src, margin):
    """The largest distance from src to a point at least margin from every wall of the box."""
    return max(math.dist(src, q) for q in itertools.product(*((margin, float(L) - margin) for L in dims)))


def in_class(cls, d, R):
    """P22's distance classes: near R + 0.2 m <= d < 2 m, mid 2 <= d <= 10 m, far d > 10 m."""
    if cls == 'near':
        return R + NEAR_GAP_M <= d < NEAR_M
    if cls == 'mid':
        return NEAR_M <= d <= FAR_M
    if cls == 'far':
        return d > FAR_M
    raise ValueError('no class %r' % (cls,))


def _uniform(rng, box):
    lo, hi = box
    return min(max(lo + (hi - lo) * rng.random(), lo), hi)


def _log_uniform(rng, box):
    lo, hi = box
    return min(max(math.exp(math.log(lo) + (math.log(hi) - math.log(lo)) * rng.random()), lo), hi)


def _receiver(rng, dims, src, cls, rej):
    s = np.asarray(src, dtype=np.float64)
    reach = {'near': NEAR_M, 'mid': FAR_M, 'far': math.inf}[cls]
    while True:
        R = _log_uniform(rng, R_M)
        c = R + REC_CLEAR_M
        lo = np.array([c, c, c])
        hi = np.array([float(L) - c for L in dims])
        if math.isfinite(reach):
            lo = np.maximum(lo, s - reach)
            hi = np.minimum(hi, s + reach)
        if np.any(lo > hi):
            rej['receiver_R_no_room'] += 1
            continue
        lo_l, hi_l = lo.tolist(), hi.tolist()
        dmin = math.dist(src, [min(max(x, a), b) for x, a, b in zip(src, lo_l, hi_l)])
        dmax = max(math.dist(src, q) for q in itertools.product(*zip(lo_l, hi_l)))
        if cls == 'near':
            empty = dmin >= NEAR_M or dmax < R + NEAR_GAP_M
        elif cls == 'mid':
            empty = dmin > FAR_M or dmax < NEAR_M
        else:
            empty = dmax <= FAR_M
        if empty:
            rej['receiver_R_no_room'] += 1
            continue
        a = R + NEAR_GAP_M if cls == 'near' else (NEAR_M if cls == 'mid' else FAR_M)
        tried = 0
        while tried < POSITION_CAP:
            P = lo + (hi - lo) * rng.random((BATCH, 3))
            dd = np.sqrt(((P - s) ** 2).sum(axis=1))
            maybe = (dd >= a - 1e-9) & (dd <= reach + 1e-9)
            for i in np.flatnonzero(maybe):
                p = [float(x) for x in P[i]]
                d = math.dist(src, p)
                if in_class(cls, d, R):
                    rej['receiver_position_outside_class'] += tried + int(i)
                    return {'position_m': p, 'R_m': float(R), 'd_m': d, 'class': cls}
            tried += BATCH
        rej['receiver_position_outside_class'] += tried
        rej['receiver_R_position_cap'] += 1


def _box_entry(dims):
    return dict(kind='box', dims_sorted_m=sorted((float(x) for x in dims), reverse=True))


def _furnish(rng, rid, kind, parent, dims, alpha, src, p3, rej, factors=None):
    """The room's receivers, design T60s and image set; None when it is over the image cap."""
    t60 = design_t60s(dims, alpha)
    far = farthest(dims, src, FAR_MARGIN_M) > FAR_REACH_M
    classes = ('near',) * N_PER_CLASS + ('mid',) * N_PER_CLASS + ('far' if far else 'mid',) * N_PER_CLASS
    recs = [_receiver(rng, dims, src, c, rej) for c in classes]
    T = image_time(recs, t60)
    n = n_images(dims, C * T + max(x['R_m'] for x in recs))
    if n > MAX_IMAGES:
        return None
    return dict(id=rid, kind=kind, parent=parent, dims_m=list(dims), alpha_walls=list(alpha), source_m=list(src),
                image_time_s=T, design_t60_s=t60, receivers=recs, far_allowed=bool(far), images=int(n),
                src_wall_m=corpus.wall_distance(src, dims), p3=dict(gap=p3['gap'], nearest=p3['nearest']),
                factors=factors)


def _relative(rng, name, parent, corpus_rooms, rej):
    while True:
        f = [_uniform(rng, REL_DIMS) for _ in range(3)]
        g = [_uniform(rng, REL_ALPHA) for _ in range(6)]
        dims = [float(L) * x for L, x in zip(parent['dims_m'], f)]
        alpha = [float(a) * x for a, x in zip(parent['alpha_walls'], g)]
        src = [float(s) * x for s, x in zip(parent['source_m'], f)]
        if corpus.wall_distance(src, dims) < SRC_WALL_M:
            raise ValueError('%s: the scaled source is %.4f m from a wall, under %r m'
                             % (name, corpus.wall_distance(src, dims), SRC_WALL_M))
        p3 = rooms.p3_clearance(_box_entry(dims), corpus_rooms)
        if not p3['fresh']:
            rej['relative_near_duplicate'] += 1
            continue
        room = _furnish(rng, 'rel:' + name, 'relative', name, dims, alpha, src, p3, rej,
                        factors=dict(dims=f, alpha_walls=g))
        if room is None:
            rej['relative_over_image_cap'] += 1
            continue
        return room


def _drawn(rng, i, corpus_rooms, rej):
    while True:
        dims = [_uniform(rng, L1_M), _uniform(rng, L2_M), _uniform(rng, L3_M)]
        alpha = [_uniform(rng, ALPHA) for _ in range(6)]
        src = [_uniform(rng, (SRC_WALL_M, L - SRC_WALL_M)) for L in dims]
        V = dims[0] * dims[1] * dims[2]
        if not V_M3[0] <= V <= V_M3[1]:
            rej['drawn_V_outside'] += 1
            continue
        if not T60_125_S[0] <= corpus.design_t60(dims, alpha, 125.0) <= T60_125_S[1]:
            rej['drawn_t60_125_outside'] += 1
            continue
        p3 = rooms.p3_clearance(_box_entry(dims), corpus_rooms)
        if not p3['fresh']:
            rej['drawn_near_duplicate'] += 1
            continue
        room = _furnish(rng, 'drawn:%d' % i, 'drawn', None, dims, alpha, src, p3, rej)
        if room is None:
            rej['drawn_over_image_cap'] += 1
            continue
        return room


def draw(seed, *, a1=None):
    driver.require_not_heldout(seed, a1)
    rng = np.random.default_rng(np.random.SeedSequence(operator.index(seed)))
    data = CORPUS_ROOMS.read_bytes()
    corpus_rooms = json.loads(data)
    parents = {r['id'].split(':', 1)[1]: r for r in corpus_rooms['rooms'] if r['set'] == 'ism'}
    if sorted(parents) != sorted(PARENTS):
        raise ValueError('corpus_rooms.json holds the ISM rooms %s, not %s' % (sorted(parents), sorted(PARENTS)))
    rej = dict.fromkeys(REJECTIONS, 0)
    out = [_relative(rng, name, parents[name], corpus_rooms, rej) for name in PARENTS]
    out += [_drawn(rng, i, corpus_rooms, rej) for i in range(1, N_DRAWN + 1)]
    return dict(seed=operator.index(seed), rooms=out, rejections=dict(rej), corpus_rooms_sha256=sha256_bytes(data))


def row_specs(D):
    out = []
    for room in D['rooms']:
        t60 = room['design_t60_s']
        for i, x in enumerate(room['receivers']):
            for band in BANDS_HZ:
                for step in STEPS_MS:
                    out.append(dict(id='ismf|%s|r%02d|%d|%gms' % (room['id'], i, band, step), room=room['id'],
                                    receiver=i, position_m=list(x['position_m']), R_m=x['R_m'], d_m=x['d_m'],
                                    distance_class=x['class'], band_hz=band, step_ms=step, run_s=RUN_S,
                                    image_time_s=room['image_time_s'],
                                    design_t60_s=t60[band] if band in t60 else t60[str(band)]))
    return out
