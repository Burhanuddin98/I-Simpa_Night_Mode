# EDT, ball receiver vs ISO point receiver: pre-registration

2026-10-02, written and committed before any number below is computed. Decision-log row 37 (6).

## Question

The product's EDT (v2.1, `frozen2/method.py`, port-equal to `crates/simpa-core/src/params/edt.rs`) reads a ball
receiver of radius R. Its fit starts at the ball's back, `t_arrival + R/c`. ISO 3382-1 A.2.2 fits a point
microphone's Schroeder curve from 0 dB to -10 dB, with time zero at the direct sound. Does the product's EDT agree
with the ISO EDT of a point receiver at the same position?

## Arms, all on the exact image-source echogram of a specular box (no noise)

- **prod**: v2.1 `analyse` on the ball echogram (`harness2/m8b/_ism.py` `echogram`, 0.02 ms fine bins) summed
  into the run's steps, with `t_arrival = d/c` and `half_width = R/c`.
- **point**: the ISO A.2.2 EDT of a point receiver. Each image delivers `W R^3 / (3 D^2)` (the ball's total, so
  the scale matches) at path length D, in 0.02 ms fine bins. The direct sound is split off, and EDT comes from
  round 2's checked `truth_ideal`: an exact integral fit from 0 dB to -10 dB, time zero at d/c.
- **ball** (reported): `truth_ideal` on the ball echogram, i.e. round 2's truth. It separates the truth's own
  ball smearing from the method's deviation.

Air absorption is off. Both arms see the same images, and air scales them equally. The image set runs to 0.6 × the
room's Eyring T60, so less than 0.1 % of the energy is missing, the same in every arm.

## Grid, fixed here

| Room | L (m) | α per wall (x-, x+, y-, y+, floor, ceiling) |
|---|---|---|
| S-live | 6 × 5 × 3 | 0.10 all |
| S-dead | 6 × 5 × 3 | 0.40 all |
| M-live | 15 × 10 × 6 | 0.15 all |
| M-dead | 15 × 10 × 6 | 0.35 all |
| Hall | 30 × 20 × 12 | 0.20 all |
| Mixed | 10 × 7 × 3.5 | 0.20, 0.20, 0.20, 0.20, 0.50, 0.10 |

- **Source:** at (0.25, 0.3, 0.45) × L.
- **Receivers:** three per room at fractions (0.55, 0.5, 0.5), (0.75, 0.7, 0.5) and (0.85, 0.85, 0.5) of L.
  A receiver closer than 2 m to the source is dropped.
- **Radii R:** 0.1, 0.31 (default), 0.5, 0.75, 1.0 and 1.5 m. A row whose ball would cross a wall is excluded and
  counted.
- **Steps:** 1 ms (product default), 2 ms and 5 ms.

## Criterion

Measure: `e = EDT_prod / EDT_point - 1`.

- **PASS:** for R ≤ 0.5 m at 1 ms, every answered row (prod not refused) has `|e| ≤ 0.5 %`, which is 1/10 of
  the 5 % JND.
- **FAIL:** any such row with `|e| > 5 %`.
- **PARTIAL:** any such row between those two limits. EDT is then answered correctly to the JND but not to the
  product's 1/10 claim, and the rows are named.
- **Reported, not gated:**
  - R of 0.75, 1.0 and 1.5 m, and steps of 2 and 5 ms.
  - `EDT_ball / EDT_point - 1` for every row.
  - Refusals, by reason.
- **Instrument checks, run first.** If either fails, the check is INCONCLUSIVE.
  - (i) On a single image, the point arm's total energy equals the ball arm's to within 1e-3 at R = 0.31 m, d = 5 m.
  - (ii) A planted 1 % error in prod is flagged by the comparison.

Outputs: `B:\data\m8b-edt\ball-vs-point\` (rows CSV and JSON, log). Script: `run.py` beside this file. It imports
`harness2/m8b/_ism.py`, `_truth_ideal.py` (through `truth.py`) and `frozen2/method.py` read-only.
