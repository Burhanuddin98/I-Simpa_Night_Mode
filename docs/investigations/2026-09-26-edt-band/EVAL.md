# EDT/Ts/C/D band: evaluation (2026-09-26, resumed after the 06:46 crash)

Units: L = 1/10 JND. EDT 0.5 % (JND 5 %), Ts 1 ms, C50/C80 0.1 dB, D50 0.005. tau = 5 L (EDT 2.5 %) and 1 L (EDT 0.5 %).
Band variants: b1 = with t_refl_min = emission + (d1 - R)/c, b0 = without. Numbers below are b1 unless stated.
Same rows for every method: band, upstream's exact algorithm (`up`), the shipped early check (`ship`, mirrors of decay.rs), W1G.

## Crash recovery
- Every earlier output parses and matches its log (`eval_check_outputs.json`). Only `ism_10ms` was missing (65/535 tasks at the crash). It was re-run alone with 7 workers: 3,210 rows, 0 errors, 648 s.
- Before the crash, the ISM 10 ms pool (8 workers) overlapped the pos200 10 ms pool (8 workers) from about 06:44:35 to 06:46:42. That is up to 16 workers, over the 8-worker rule. Whether this caused the crash is not established. Details are in `eval_crash_0646/README.txt`.

## Containment (strict; truth A = continuous-time Definition A)
| Set | Rows (steps) | EDT inside / finite bands | Ts | C50, C80, D50 |
|---|---|---|---|---|
| Z3, 217 robust cases, own step and delay | 217 | 201/201 (least margin 2.75 L) | 217/217 | 217/217 each |
| z3grid (their 202 geometries, 0.1-2 ms) | 1,010 | 935/935 | 1,010/1,010 | **1000/1010, 1007/1010, 1000/1010** |
| ISM | 28,890 (0.1-5 ms) | 27,892/27,892 | all | all |
| synth | 26,368 | 25,293/25,293 | all | all |
| pos200 | 11,200 | 11,200/11,200 | all | all |
| 10 ms: ISM, synth, pos200 | 3,210, 3,296, 1,400 | 2,243, 2,139, 1,398 (all inside) | all | all |
| real (16 SPPS datasets, re-binned) | 26,808 | registered truth 26,808/26,808; fine-step band inside every coarse band | same | fine-step band inside |

**The 23 misses (46 counting b0) are a defect, not noise.** They are all z3grid, C/D, at 0.1 and 0.2 ms, and each lies outside by at most 0.0015 L.
- SPPS applies `p = expf(-m c dt)` in f32, so the energy it propagates decays at `a_f32 = -ln(p_f32)/dt`. That rate is up to 0.9 % off m·c at small m·c·dt.
- The band models a_f32 exactly (P1). The task's target, exp(-m c tau), uses m. So P1 does not cover the f32 rounding of p itself. SPEC 1.1's "changes no verdict" is true for EDT and Ts, and false for C and D at fine steps.
- Proof: with the truth computed at a_f32, all 23 fall inside (`eval_diag_z3cd.*`, extended to all misses in `eval_diag_p32.*`).
- Fix, tested without changing band_early.py: `eps_n = (1+eps_f32)·exp(|a_f32 - m c|·(n+1)·dt) - 1`.
  - All 46 are then inside.
  - Cost on those rows: each C/D half-width grows by at most 0.003 L (the largest stays under 0.21 L); EDT is x1.05 at most, Ts x1.18 at most.

There are no EDT or Ts misses anywhere.
- Refusals are not misses. EDT refusals: `range_in_arrival`, 513 ISM rows (56 series have no Definition-A EDT at all), and `not_decaying`, 485 ISM rows.
- The work budget was exhausted in 67 bands. They stay valid, possibly looser.

## Half-width (median / p90, in L), every set pooled, by EDT range
| Step | EDT 0.3-0.6 s | EDT 0.6-1 s | EDT 1-2 s | Ts (all) | C50 0.6-1 s | C80 0.6-1 s | D50 0.6-1 s |
|---|---|---|---|---|---|---|---|
| 10 ms (upstream default) | 46.8/87.5 | 30.2/38.6 | 18.7/29.2 | 4.22/4.83 | 6.0/7.4 | 4.8/5.7 | 5.7/7.3 |
| 2 ms | 8.7/16.2 | 6.0/7.0 | 3.8/6.2 | 0.85/0.98 | 1.23/1.59 | 0.93/1.21 | 1.18/1.51 |
| 1 ms | 4.6/9.0 | 3.1/3.8 | 2.0/3.5 | 0.43/0.50 | 0.59/0.84 | 0.47/0.63 | 0.56/0.77 |
| 0.5 ms | 2.3/5.0 | 1.55/1.96 | 1.02/1.80 | 0.22/0.27 | 0.28/0.43 | 0.23/0.32 | 0.27/0.40 |
| 0.2 ms | 0.96/2.06 | 0.63/0.87 | 0.42/0.76 | 0.09/0.13 | 0.11/0.17 | 0.09/0.13 | 0.11/0.16 |

- EDT scales as `hw_rel = k·dt/EDT`, with k median 10.5-11 (0.6-1 s) and 12-17 elsewhere. The tail is heavy: in near-source, direct-dominated receivers the -10 dB point sits at U = 5-15 ms and hw ≈ 2·dt/U whatever the EDT.
- EDT under 0.3 s: 19.4 L median at 2 ms, and within 1 L for only 20 % of rows even at 0.1 ms.
- Full tables: `eval_tables.txt` B, `eval_summary.txt`.

## Acceptance: share of rows given a value by the band (noise-free sets ism + synth + pos200 + z3grid), tau 5 L / 1 L
| Step | EDT 0.6-1 s | EDT 1-2 s | EDT all | Ts all (flat) | C50 all | C80 all | D50 all |
|---|---|---|---|---|---|---|---|
| 10 ms | 0 / 0 % | 0 / 0 % | 0 / 0 % | 98 / 0 % | 11 / 0 % | 25 / 0 % | 66 / 23 % |
| 2 ms | 0 / 0 % | 75 / 0 % | 7 / 0 % | 100 / 93 % | 87 / 16 % | 88 / 28 % | 100 / 67 % |
| 1 ms | 96 / 0 % | 95 / 0 % | 46 / 0 % | 100 / 100 % | 99 / 54 % | 100 / 58 % | 100 / 98 % |
| 0.5 ms | 99 / 0 % | 95 / 46 % | 61 / 5 % | 100 / 100 % | 100 / 78 % | 100 / 81 % | 100 / 100 % |
| 0.2 ms | 99 / 94 % | 95 / 94 % | 85 / 43 % | 100 / 100 % | 100 / 99 % | 100 / 100 % | 100 / 100 % |

Ts under the relative rule min(1 ms, 0.5 % Ts), all rows: 0 % at 2 ms, 3 % at 1 ms, 16 % at 0.5 ms, 54 % at 0.2 ms.

## Wrong-but-accepted, same rows (noise-free sets, every step 0.1-5 ms, 67,468 rows)
| Quantity | Band b1: accepted / wrong, 5 L; 1 L | Upstream: wrong at 5 L; 1 L (always gives a value) | Shipped check: accepted / wrong, 5 L; 1 L | W1G: accepted / wrong, 5 L; 1 L |
|---|---|---|---|---|
| EDT | 24,866 / **0**; 8,626 / **0** | 41,865; 59,840 | 57,626 / 2,583; 44,736 / 4,697 | 29,571 / 39; 22,186 / 158 |
| Ts (flat) | 67,468 / 0; 47,035 / 0 | 52,741; 67,467 | 67,468 / 0; 67,468 / 1 | 42,598 / 0; 42,465 / 0 |
| C50 | 57,220 / 0; 29,519 / 0 | 30,278; 62,480 | 26,182 / 7; 26,182 / 1,831 | n/a |
| C80 | 58,301 / 0; 32,301 / 0 | 26,469; 60,726 | 26,182 / 3; 26,182 / 1,385 | n/a |
| D50 | 67,391 / 0; 50,921 / 0 | 4,479; 30,232 | 26,182 / 0; 26,182 / 30 | n/a |

- **Z3's 217 cases**
  - EDT: the band accepts 0 of the 155 EDT cases at 5 L, and every truth is inside (half-width at least 5.5 L).
  - W1G accepts all 155, and all of them are wrong at 1 L (this reproduces the judge, 155/155).
  - The shipped check accepts 154 of them at 5 L, 39 of those wrong.
  - Upstream's EDT error on them: median 44 L, max 670 L.
- **Real set, EDT (registered truth)**
  - The band accepts 1,636 of 26,808 rows at 5 L, 0 wrong.
  - The shipped check: 24 wrong at 5 L and 1,383 at 1 L.
  - Upstream: 11,162 wrong at 5 L.
- Upstream's EDT on the noise-free sets at 2 ms: median error +0.4 L (ISM) and -9.8 L (synth), with tails to -197 and +640 L. It never gave EDT <= 0.

## Step needed (noise-free pool; coarsest evaluated step where >= 50 % / >= 90 % of rows get a value)
| EDT range | tau 2.5 % | tau 0.5 % | Rule of thumb, dt <= tau·EDT/k |
|---|---|---|---|
| 0.3-0.6 s | 1.0 / 0.2 ms | 0.2 ms / not reached at 0.1 ms | k about 12 |
| 0.6-1 s | 1.5 / 1.0 ms | 0.2 / 0.2 ms | k about 11: 1.8 ms and 0.37 ms at EDT 0.8 s |
| 1-2 s | 2.0 / 1.0 ms | 0.2 / 0.2 ms | k about 14: 2.7 ms and 0.54 ms at EDT 1.5 s |
| > 2 s | 4.0 / 1.5 ms | 0.5 ms / not reached | direct-dominated receivers never pass |

Other quantities:
- Ts: 2 ms is enough at a flat 1 ms tau. The relative rule needs 0.2 ms.
- C50 and C80: 1-3 ms at 1 L (0.5-1.5 ms for 90 %), 5 ms at 5 L.
- D50: 1-3 ms at 1 L (1 ms for 90 %).

At upstream's default of 10 ms, no EDT is given at either tau.

## Noise
- Q(expected echogram) lies in [lo(E[B]), hi(E[B])] exactly.
- A run's edges carry the existing noise model unchanged: [lo(B) - z·sigma_lo, hi(B) + z·sigma_hi]. For a seed batch, take the band of the pooled histogram, with SB-5 applied to the per-seed edges. Single-run EDT noise is uncalibrated (D8).
- Measured on the real 10-seed cells at 2 ms: the seed SD of the EDT edges is 1.85 L median against a half-width of 13.0 L (ratio 0.20, p90 0.88). For C80 the ratio has p90 3.2.
- The band shrinks with dt and the noise does not, so below about 1 ms noise, not discretisation, sets what can be given at these particle counts.

## Cost
- numpy, one variant, five quantities: median 6-330 ms per row, max 1.3 s.
- EDT relaxations: median 438 (ISM), 40 (pos200), capped at 6,003.
- Complexity: C and D O(N); Ts O(N) per Dinkelbach step, 3-6 steps; EDT O(N + min(6000·W, 2e6 + 64·W)).

## Files
All scripts and outputs are in this folder:
- Runs: `eval_run.py`, `eval_run_default.py`, `eval_common.py`, and each set's JSON with its `_log`.
- Summary: `eval_summary.py` (`.txt`, `eval_results.json`), `eval_tables.*`, `eval_step_needed.*`.
- Diagnostics: `eval_diag_z3cd.*`, `eval_diag_p32.*`, `eval_refusals.*`, `eval_upstream_port.*`.
- Crash recovery: `eval_check_outputs.*`, `eval_crash_0646/`, and `eval_run1/` (the first synth and pos200 runs).
