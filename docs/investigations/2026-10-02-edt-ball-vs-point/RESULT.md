# EDT, ball receiver vs ISO point receiver: result

2026-10-02 08:58-09:00. Pre-registration `PREREG.md` (`e926638`). Script `run.py`. Outputs
`B:\data\m8b-edt\ball-vs-point\` (main grid) and `...\amendment1-mixed\` (Amendment 1). Reviewed by sentinel; its
corrections are applied below.

## Verdict as pre-registered: PARTIAL

- 45 gated rows (R ≤ 0.5 m, 1 ms, answered). None is beyond the 5 % JND.
- 6 rows sit between the 0.5 % bar (1/10 JND) and the JND, worst 1.10 %. All 6 are in the two dead rooms
  (S-dead, T60 0.23 s; M-dead, T60 0.56 s).
- The live rooms and the hall agree to within 0.1 %.
- The error is the same size at R = 0.1 m as at R = 0.5 m. It comes from the method in short decays, not from the ball.

## What it means

- **The ball is not the issue.** Round 2's truth (direct sound as a point step, reflections recorded by the ball)
  differs from the ISO A.2.2 point-receiver EDT by at most 0.085 % on the gated rows and 0.17 % on any answered row.
  Round 2's verdict therefore holds against the ISO point definition.
- **The product's shown range always holds the ISO point EDT.** 279 of 279 answered rows across every radius and
  step, Amendment 1 included, have it inside [`edt_lo`, `edt_hi`]. Of the 80 rows over 0.5 %, 70 are marked `wide`
  (the product says its range exceeds the JND) and 10 are `ok`. One `ok` row is in the gate: S-dead, rec0, R 0.5,
  -0.97 %, inside its range.
- **So:** EDT is correct to the JND and honest about its range everywhere tested. It is not accurate to 1/10 JND in
  dead rooms. That 1/10 bar is the plan's own (PLAN.md, M2), not a claim the product makes for EDT, whose claim is
  "truth inside the shown range" (row 9).
- **Coarser steps are worse in dead rooms.** At 2 ms the worst is 1.67 %; at 5 ms it is 3.37 % (S-dead, R 0.1).
  These rows are not gated and all are inside their range.

## Refusals and exclusions (main grid)

- **Mixed room, `run_too_short`:** 51 rows, all of it. The image set at 0.6 × Eyring T60 ended before the decay did,
  so the pre-registration's "under 0.1 % missing" premise is false for this room. The cause was not measured.
- **S-dead, `receiver_too_large`:** 6 rows (rec0 and rec1 at R = 1.0, every step). The refusal works as designed.
- **Excluded, ball crosses a wall:** 13 rows. No receiver was dropped for being too near the source.

## Amendment 1, declared after the main grid

The Mixed room was re-run alone at 2.0 × Eyring T60. This deviates from the pre-registered 0.6 and is reported beside
the pre-registered verdict, not in place of it.

- 9 gated rows, worst 0.98 %, and 3 rows over 0.5 % (rec2, R 0.1 / 0.31 / 0.5: 0.66 / 0.67 / 0.98 %, all `wide`).
- Same verdict, PARTIAL. All rows are inside their range.

## Instrument checks, and how much they prove

- **(i)** Point and ball totals for one image agree to 7.7e-4, against a 1e-3 gate. This checks scale only.
- **(ii)** A planted 1 % error is flagged. This is a unit test of the verdict rule only.
- The stronger evidence that the comparison can see a difference is in the data: it resolves the 5 ms degradation
  (3.4 %) and the dead-versus-live split.
- `run.py` gained output fields (`edt_lo`, `edt_hi`, `covered`) and a per-job image factor after the
  pre-registration. The gated values reproduce exactly across both runs (`run.log`, S-dead rec2 R 0.5 +0.0110 in both).

## Also measured here: T20 does not see the ball shelf

On the M8a decays (`C:\tmp\nm-m8a-bed\20260929T093134Z\decays`, 7,680 SPPS curves), the -5 dB crossing comes no
earlier than 14.0 ms after the onset (median 74.8 ms). The default ball's crossing time is 2R/c = 1.8 ms. T20's fit,
which starts at -5 dB, never sees the first 2R/c, for any R under about 2.4 m.
