# M8b EDT held-out: DIAGNOSIS (run1, read-only, nothing fixed)

2026-10-01. Diagnosis only. Scripts and scratch output: `C:\Users\Burhan\AppData\Local\Temp\claude\b--repos-I-Simpa-Night-Mode\5071e985-9550-4183-93c1-64ff7f056f19\scratchpad\diag\`
(`q1`..`q4`, `fine2.py`, `q2r.py`, `q2br.py`). No repo file touched except this one.

## In plain words

**F1 (dead rooms) is a method defect, and the stored truth is correct.** I recomputed the truth two independent ways and it
agrees to 0.06 %. The method fits its line from `t_arrival + R/c` (the back of the receiver ball), so it never sees the first
`R/c` of the decay. The truth (Definition A) fits from `t_arrival`. In a dead room the first 10 dB lasts only EDT/6, about
17-25 ms, and the ball half-width is 2-4 ms of that. The start of the curve is not a straight line: it is a shelf, because the
first reflections arrive about as late as the ball's back (median 1.5 ms after the direct sound, against `R/c` = 3.3 ms).
Dropping the shelf makes the line steeper and EDT low. Replace the truth with "the truth fitted from the method's own start
point" and the method matches it to -0.3 % median. All of the -7.4 % is the start point. Discretisation, air and the -10 dB
end point account for under 1 point. It is a function of ball half-width over EDT (`R/c / EDT`), not of step. The "T60/step < 300"
pattern is only the refusal gate hiding the 5 ms rows. The harness draws R up to 1.5 m; the product default is 0.31 m, and at
that radius no row fails.

**F2 (SPPS range) is a noise-model defect.** The seed-to-seed spread of EDT is 1.68x (Random) and 1.35x (Energetic) what the
half-width says (1.95x at 1 ms in both). The cause is `w`, the compound-Poisson weight, estimated from second differences
of adjacent bins. A ball spans about 2 steps at 1 ms, so adjacent bins are correlated, and the estimate comes out 2.1x too
small in variance (a seed-replicate estimate of the same quantity is correlation-free). There is no bias: the mean of 3 seeds
sits at -0.2 % of truth.

---

## Q1. Truth/harness defect or method defect?

**Method defect. The truth is right.**

1. The method reproduces from stored inputs exactly: 0 of 1066 `ok` ISM rows differ from `frozen_edt` (`frozen/method.py`
   sha256 starts 462c37cf; script `q1`-era check, output `mismatch 0`).
2. I regenerated the ISM echograms from the draw (seed 2026100102 through `ism_fresh.draw` and `_ism.echogram`). The coarse
   bins match the stored `inputs_ism` bins exactly (`np.allclose`, rtol 1e-9; row `ismf|rel:deader|r04|125|1ms`: 700 of 700).
3. Independent truth: `fine2.truthA`. It takes the 0.02 ms Schroeder backward sum of the reflected series from `t`
   (direct a step at `t`, reference level = direct + reflected total), takes the sample points with level >= -10 dB, and runs plain
   OLS on the samples. It does not use the corpus `line()` or `integral()`. Result over all 301 rows examined (84 failing + 217
   others, all run on the 0.7 s truncated echogram): stored vs mine, max |diff| 0.064 %, median 0.009-0.031 %
   (`q2r.py`: "stored truth vs my independent Def A"). The 84 failing rows are the maximum case.
4. Synth-fresh: I recomputed the closed-form truth on a 5 us grid (`q3.py` `mytruth`, ISO OLS on `S(u)`): +0.004 to +0.008 %
   against stored on the worst 6 rows. The regenerated histograms equal stored bins (`hist_match True`).
5. Rows (stored truth / mine / frozen):

| row | R, h | stored | mine | frozen (err) | truth from t+h |
|---|---|---|---|---|---|
| ismf\|rel:deader\|r04\|125\|1ms | 1.03 m, 3.0 ms | 0.1453 | 0.1453 | 0.1374 (-5.4 %) | 0.1369 |
| ismf\|drawn:6\|r07\|16000\|1ms | 0.87 m, 2.5 ms | 0.1156 | 0.1157 | 0.0996 (-13.8 %) | 0.1014 |
| ismf\|drawn:2\|r06\|20000\|1ms | 0.80 m, 2.3 ms | 0.1095 | 0.1095 | 0.1022 (-6.7 %) | 0.1016 |
| ismf\|drawn:2\|r11\|20000\|1ms | 0.66 m, 1.9 ms | 0.1082 | 0.1082 | 0.1005 (-7.1 %) | 0.0986 |
| synth\|5\|1ms\|484 | 1.49 m, 4.3 ms | 0.1843 | 0.1843 | 0.1697 (-7.9 %) | see Q2 |
| neighbour ismf\|rel:deader\|r07\|125\|1ms | 0.57 m, 1.7 ms | 0.1476 | 0.1476 | 0.1444 (-2.2 %) | 0.1464 |
| neighbour ismf\|rel:t1\|r10\|16000\|1ms | 0.16 m | 0.2969 | 0.2969 | 0.2966 (-0.1 %) | n/a |

   The r04/r07 neighbours differ in R (1.03 vs 0.57 m), not in distance d: that is what separates them.
6. The failure rate by `R/c / EDT` (1066 ok ISM rows, `q2a.py`): < 0.01: 0 of 825 wrong. 0.01-0.02: 11 % of 163, median -1.4 %.
   0.02-0.04: 84.6 % of 78, median -7.2 %. Failing rows' R: median 1.14 m (min 0.66); the rest of the rows: median 0.61 m. By R
   among dead rows (EDT/step < 200): R 1.0-1.5 m wrong 40.8 %; R < 0.4 m wrong 0 %.
7. One caveat on truth definition (not a defect of the stored number): where the corpus's own SPPS-style window split
   (bins overlapping `[t-h, t+h)` all counted as direct) is applied to the same ISM echogram, the truth moves +1.6 % median and
   up to 8.4 % on the failing rows (`q2r.py` "Def A-window split truth"), because first reflections sit inside the ball window.
   The truth's definition is thus sensitive to what is "direct" when the ball is wide, which the method cannot see either.

## Q2. Which lines produce the low bias?

`frozen/method.py:43` (`t_start = t_arrival + h`), with `sel` at line 55 selecting `T >= t_start`. Mechanisms switched off in
COPIES of the method (`q2.py`, `q3b.py`), ISM = 84 failing rows, median error vs stored truth:

| variant | ISM 84 failing | Synth 20 failing |
|---|---|---|
| frozen (coarse bins) | -7.37 % | -6.97 % |
| discretisation off (same series at 0.02 ms / 0.01 ms bins) | -6.70 % | -5.72 % |
| air per step replaced by continuous air (coarse) | -7.37 % (identical) | n/a |
| end point: drop the first sample below -10 dB (`<= i10 - 1`) | -6.91 % | -6.42 % |
| `t_start = t_arrival` (start at the arrival, not the ball's back) | -7.76 % | -2.67 % (but max error 60 %) |
| **truth itself restricted to the method's window (start t+h)** | **-6.69 %** (frozen vs this: -0.30 % median, 2.8 % max) | **-5.69 %** |
| truth restricted to start t + h/2 | -5.04 % | n/a |

What this says:
- **Window start (`t+h`) is the cause.** The truth fitted from `t+h` instead of `t` is already -6.7 % (ISM) and -5.7 % (Synth). The
  method then reproduces that to 0.3 %. Across the 241 rows with `h/EDT > 0.01`: correlation of `h/EDT` with the restricted-window
  bias is -0.80.
- Discretisation (S sampled at bin edges): ISM 0.0-0.7 points, Synth 1.2 points. Real, but second-order.
- The -10 dB first-sample overshoot: 0.5 point. Second-order.
- Air per step: 0. Strong direct sound in coarse bins: no separate effect found; the direct is removed by starting at `t+h`.
- Why the early curve is not exponential: Synth has an explicit `gap` (median 8.1 ms on failing rows vs 17 ms on all ok rows);
  the Schroeder curve is flat for `gap`, which the truth's window includes. ISM has no gap but the first reflection arrives a median
  1.5 ms after the direct sound (`q2br.py`), inside the ball window, so the same shelf appears. Level at `t+h` is -3.4 dB median
  (ISM failing): about half the energy is already gone before the method's first sample.
- Starting at `t` is not the fix as it stands: in the same table `t_start = t_arrival` ingests the back half of the direct bump
  and raises the worst error to 60 %. The data in `[t, t+h)` mixes the direct sound with early reflections; the method cannot
  separate them. That is the unobservable part.
- Why "T60/step < 300": the 5 ms rows of all dead cells are refused by `MIN_POINTS` (`step_too_coarse`: 160 of 160 at
  `h/EDT > 0.015`), so the failure shows only at 1 and 2 ms. Step is not the driver.

## Q3. Why does the range not widen?

The half-width is `max(Z*sqrt((se/a)^2 + sd^2), 0.005)` (`method.py:85`). On the 84 failing ISM rows (`reason` field):
- `se/a` (fit term): median 1.6 %, so `Z*` it = 3.2 %. It dominates.
- Noise term `sd`: median 0.4 % (nonzero on a noise-free ISM input: early-reflection structure is read as noise).
- The 0.5 % floor never binds (0 % of rows).
- Result: half-width median 3.4 % (1.8-5.0 %) against a median miss of 7.4 %, so the range needs to be 2.2x wider.

What is missing: a term for how far the slope moves when the window start moves. `se` is the scatter of points around the fitted
line within the window the method looked at. A shelf at the start that the window skipped leaves a clean line over the points it
did see, so `se` is small precisely when the bias is large. There is no term for the unseen first `h`, and none for the sampling
effects (1-2 points).

## Q4. F2: noise

Method: 3 seeds of the same room/receiver/band/step/variant (id prefix up to `seed`, `seed // 100`) grouped, groups with all 3 rows
`ok` and a non-excluded truth: Random 472 groups, Energetic 847 (`q4.py`, `q4b.py`). Seed-to-seed relative sd of EDT against
the model sigma = half-width / Z, pooled as sqrt(mean actual variance / mean model variance):

| | Random | Energetic |
|---|---|---|
| all steps | **1.68** | 1.35 |
| 1 ms | 1.95 | 1.93 |
| 2 ms | 1.46 | 1.16 |
| 5 ms | 1.24 | 1.04 |
| model variance share from fit term | 19 % | 50 % |
| mean-of-3 bias vs truth (median) | -0.23 % | -0.21 % |

(For the 368 Random rows that miss: median seed-sd / sigma 2.05; the truth lies a median 0.36 half-widths outside the range. Selected on
the miss, so read the pooled row above.)

- It is spread, not bias: the 3-seed mean is within 0.23 % of truth.
- The same factor appears directly at `S10`, the quantity the noise term models: relative sd of `S(t_-10dB)` across seeds is 1.67x
  (Random) and 1.36x (Energetic) the model's `sqrt(w/S10)` (`q4b.py`), 1.95 / 1.87 at 1 ms. The EDT scatter factor (1.68) matches the `S10`
  factor (1.67), so the error is in the noise term.
- Cause, in two parts (`method.py:82-83`: `w = sum(r^2) / (1.5 * sum(d))`, `sd = 0.4343*sqrt(w/S10)`):
  (a) `w` is taken from the second difference of adjacent bins, which assumes independent bins. A receiver ball spans about 2 steps at 1 ms
  (2R/c = 1.8 ms), so adjacent bins share particles and the second difference cancels part of the noise. Per-bin variance measured from
  the 3 seeds (correlation-free) divided by the method's `w`: median 2.14 at 1 ms, 1.31 at 2 ms, 1.15 at 5 ms (Random), the same for
  Energetic (2.12 / 1.28 / 1.06). It tracks ball width over step.
  (b) `var(S10) = w * S10` also assumes independent bins when summing; correlated bins sum to more. (a) alone gives 1.46 in sd at
  1 ms; the observed 1.95 needs (b) as well.
- Random vs Energetic: the `w` underestimate is the same (2.1x at 1 ms). Random is worse in the verdict because its half-width is
  noise-dominated (19 % fit share) and has 7x the `S10` variance (0.0027 vs 0.00038); the Energetic half-width carries a large
  deterministic fit term that happens to cover most of the gap (fit term alone is 1.7x too small for Energetic on its own, 3.5x for Random).
  Energetic wrong-silent is low (0.96 %) mainly because its noise is small, not because its model is right.
- The SPPS Random failing subgroups sitting at 1 ms are explained by this (factor 1.95 there, 1.2 at 5 ms).

## What a fix would have to address (mechanisms, not code)

1. The fit window starts at `t + R/c`, but the truth's window starts at `t`. For dead rooms with a wide ball, the unseen first
   `R/c` contains a shelf that changes the slope by several percent. Either the method must estimate that part, or the target must
   be defined on what a ball receiver can observe (a decision about Definition A, not a coding detail). Starting at `t` is
   not enough: it ingests the direct bump.
2. The range needs a term for sensitivity to the window start (and other unobserved window-edge choices), scaled by `(R/c) / (EDT/6)`.
   `se` cannot supply it: it is smallest where the bias is largest.
3. Or refuse when `(R/c)/(EDT/6)` is above the level where the bias exceeds JND. The data say failures begin near `R/c / EDT` > 0.01
   and are 85 % above 0.02. Refusing is what already happens at 5 ms.
4. Smaller sampling effects, 1-2 points: S sampled at bin edges and the first sample below -10 dB. They matter once (2) exists.
5. The noise model needs correlated-bin variance: `w` from second differences is 2.1x low at 1 ms, and `var(S10)` ignores bin
   correlation. Both depend on ball width over step.
6. The harness draws R from 0.1-1.5 m, the product default is 0.31 m. Whether large R must be supported is a scope question for
   Burhan, not a diagnosis finding. At R below 0.4 m (dead rows) there were 0 wrong-silent rows.

Not done: SPPS dead-room rows were not re-derived from a fine echogram (no fine series stored; SPPS truth uses the window split,
Q1 point 7). I truncated the ISM echogram to 0.7 s for speed; the stored truth is reproduced to 0.064 %, so tail truncation is negligible
for these rows.
