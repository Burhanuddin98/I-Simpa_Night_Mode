"""ISM-fresh (HARNESS-PLAN.md P20-P22, P33): specular boxes through the corpus's validated image-source
generator, target/agents/followup-design/skeptic-gf3/ism.py (sha256 a7c9d41e...), copied by text,
with the series built as target/agents/followup-design/spec/eval_ism.py:59-88 builds them.

STUB (HARNESS-PLAN.md section 6, step 3): every function returns None and BOUNDS is empty. Tests
T6, T13, T16 and T22 hold the contract below.

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
"""
C = 343.20001220703125              # attack_ism.py:28
DT_FINE = 2e-5                      # attack_ism.py:30
BANDS_HZ = (125, 250, 500, 1000, 2000, 4000, 8000, 16000, 20000)
STEPS_MS = (1.0, 2.0, 5.0)
RUN_S = 2.0
MAX_IMAGES = 1e8
HELDOUT_SEED = 2026100102           # P21
BOUNDS = {}


def make_row(room, rec, R, F, step_ms, image_time_s=None, run_s=None):
    return None


def draw(seed, *, a1=None):
    return None
