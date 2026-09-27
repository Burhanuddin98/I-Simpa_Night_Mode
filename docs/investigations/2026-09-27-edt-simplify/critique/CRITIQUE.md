# EDT-simplify: adversarial critique of the three candidates and of the evaluation

2026-09-27. Read-only on everything outside this folder. No solver run. Every Python process was
single-process and was started only after checking the python.exe count (0 or 1 running each time).
Scripts and raw outputs are in this folder, and each claim below names the script that produced it.

## Bottom line

**None of the three candidates survives.** Each one shows a wrong EDT with `status='ok'` and
|error| > 5 % on inputs that a user can plausibly meet. Some of those inputs come from the evaluator's
own physically realistic sets and some from the repo's own real SPPS seeds.

| | cand_minimal | cand_leanband | cand_practice |
|---|---|---|---|
| Eval: wrong-silent t1 / z3 / real / ism | 0 / 18 / 1 / **301** | 0 / 3 / 1 / 3 | 0 / 32 / 0 / **220** |
| Worst wrong-silent in the eval, physical set (ISM) | −18.1 % (dead room, 0.7 m, 1 ms) | **+29.5 %** (corridor, 0.7 m, 125 Hz, **1 ms**) | **+142 %** (corridor, 0.5 m, 125 Hz, 2 ms) |
| Scan 1, direct sound and origin (I1/I2), plausible rooms, noise-free, long run | 133 / 2268 | 24 / 2268 (−5 to −7.4 %) | 83 / 2268 |
| Scan 2b, run ends early (I4), long run itself within 5 % | 765 / 1892 (to −39 %) | 115 / 1892 (to −10.9 %) | 160 / 1892 (to −12.4 %) |
| Scan 3, particle noise (I5), 20 000 m³ hall, 150 k particles, 1 ms | 140 / 200 seeds | 140 / 200 seeds | 140 / 200 seeds |
| Real seeds, the repo's own 10-seed runs, vs the seed mean | 15 / 3600 | 15 / 3600 (±6-7 %, random-mode cells) | 22 / 3600 |
| Scan 4, blocked direct path (I1), 60 rows with no direct sound | 16 / 60 (to +20.8 %) | 10 / 60 (to +16.2 %) | 16 / 60 (to +20.8 %) |

LeanBand fails least often and has the fewest mechanisms of failure. **Its point value is cand_minimal's
point value on every row** (same `k0 = floor(t_arr/dt)`, same window and same OLS). Over the 5,815 eval
rows where both report an EDT, the largest relative difference is 8.9e-16 (`same_value.py`). What
LeanBand adds is gating. It shows `wide` where minimal shows `ok`. It does not compute a better number.

Four defects are shared by all three, and each has a receipt below:

1. **The origin discards recorded direct sound (I1/I2).** SPPS's ball receiver records the direct sound
   from `t_arr − R/c`. Night Mode's `arrival_s` is the centre time `d/c`
   (`crates/simpa-core/src/results/spps.rs:285-311`). `floor(t_arr/dt)` therefore drops the front part
   of the direct sound: 55 % of it in the ISM corridor row below. cand_practice's `round(t_arr/dt)` drops the whole
   direct bin whenever the fractional part is 0.5 or more.
2. **The 0 dB sample is a regression point (I2).** A strong direct sound's drop gets one bin's weight
   in the line fit. ISO on a finely sampled response, and the evaluator's truth, give that drop zero
   weight. Near receivers come out 5-20 % short in plausible rooms, and down to −50 % in extreme ones.
3. **The range is the OLS slope CI of a backward-integrated curve (I5/I7).** It measures curvature, not
   particle noise. With realistic hit counts it under-reports the seed-to-seed spread by 6-35× (0.1-0.7 %
   implied against 2.2-12.1 % actual). The claim that "Schroeder integration averages the noise away"
   (PRACTICE.md §2.1, repeated in all three designs) is a misreading. Schroeder's result concerns
   random-phase decays of interrupted noise. A backward sum of Poisson particle hits keeps the Poisson
   variance of the hits it contains.
4. **The run-length gates do not cover double-slope decays (I4).** minimal's `run_too_short` never fired. LeanBand's
   extrapolated-line gate and practice's EDT-line Lundeby extrapolate the *early* slope, so they
   underestimate a slower late tail.

---

## 1. cand_minimal: does not survive

**Fatal receipt.** Eval ISM row `ism|dead|(2.52, 2.67, 1.53)|1000|1ms`: EDT 0.2088 against truth 0.2548
(−18.1 %), `status=ok`, range [0.193, 0.228] excluding the truth. The input is a 5×4×3 m room with
α=0.4, a receiver 0.7 m from the source and the default 1 ms step. There are 301 such rows in the
eval's ISM sample, and 299 of them are negative (`inspect_ws.py`). A second receipt is scan 2b
(`scan_i4b.py`): one exponential decay with a run of 0.15·T60 returns −32 % with `status=ok`.

**Claims against numbers**
- "run_too_short / near_run_end gates catch the case where the window doesn't close cleanly" (DESIGN I4):
  **false.** Over 14,717 rows (this critique's scans plus all four eval sets), `run_too_short` fired
  **0 times** (`deletable.py`). A truncated backward sum always reaches −10 dB and −20 dB, because its
  last bin holds only that bin. `near_run_end` changed status on 9 of 3,682 rows (`ablate.py`).
  Scan 2b gives 765 wrong-silent rows from truncation alone.
- "Smoke: 10/10 truth inside [edt_lo, edt_hi]": the eval finds the truth outside the range on 56/160 t1
  rows, 475/3600 real rows and 214/1884 ISM rows (11-35 %), against a stated 95 % interval.
- "a pathological input would show as poor-r² wide/not_linear … a coarser diagnosis, not a wrong
  number": **false.** It gives 301 ISM rows and 18 z3 rows that are `ok` and wrong. A flat,
  non-decaying series returns EDT 3.1 s `wide` (`scan_misc.py (c)`).
- "I3 residual shrinks with dt (0.07 % at 1 ms)": true for its own smoke cases only. The I2 bias does
  not shrink with dt: −18 % at 1 ms on the ISM dead room.

**I1-I7 coverage.** I1 is fixed only for an unblocked path whose ball front does not fall in the previous
bin. I2 is not handled (defect 2). I3 uses bin-end labels, which do not matter for a slope. I4 is not
handled, because the gates are dead. I5 relies on the OLS SE (defect 3). I6 refuses cleanly, but
`t_arrival=None` crashes it, and Night Mode's `arrival_s` is `None` whenever there is a celerity
gradient. I7 shows a range that is miscalibrated. The Ts "range" is a single point (`ts_lo == ts_hi`).

**Deletable:** `run_too_short`, which never fired. `near_run_end` is close to deletable (9/3,682).

## 2. cand_leanband: does not survive. It is the closest of the three.

**Fatal receipt.** Eval ISM row `ism|corridor|(5.45, 2.53, 1.53)|125|1ms`: EDT 1.2626 against truth
0.9749 (+29.5 %), `status=ok`, half-width 4.95 %, r² 0.92. The input is a 20×4×3 m corridor, a receiver
0.7 m from the source, the default 1 ms step and the default 0.31 m ball. **Mechanism, confirmed**
(`ism_mech.py`): 55.4 % of the direct sound lies in bin 1, before `k0 = floor(2.04 ms / 1 ms) = 2`, and
LeanBand drops it. Passing `t_arr − R/c` instead moves the error to −7.9 % and the status to `wide`. The
truth itself is sound. Extending the image set 2× moves it by only 4.6 %, and LeanBand's error stays
+28.8 % (`ism_probe.py`).

A second receipt comes from the repo's own real SPPS noise. Cell C-R3 is a 5×4×3 m room, random mode,
150 k particles (upstream's default count) and a 1 ms step. It gives 10 seed rows with `ok` and a
6-7 % miss against the 10-seed mean, with ranges that exclude the seed mean. V-R2 adds 3 more and V-R6
adds 2, all in random mode, for 15 of 3,600 (`real_seeds.py`). This is not synthetic.

**Claims against numbers**
- "Would any measured case now get a wrong number without warning? Not among the cases this session
  could check": the eval found 7 (3 z3, 1 real, 3 ISM). This critique found more on every axis in the
  table above.
- "The range … the regression's own ~95 % CI": the truth is outside it on 458/3600 real rows (12.7 %),
  212/1897 ISM rows (11.2 %), 56/160 t1 rows (35 %) and 40/203 z3 rows (19.7 %).
- "I5: folded into the regression's own standard error": in synthetic noise the implied sd is
  0.1-0.7 % against an actual 2.2-12.1 %, 6-35× too small (`noise_probe.py`). On real seeds the ratio of seed sd to
  implied sd is p90 2.76 and max 8.6 (`noise_probe_real.py`, `real_seeds.py`).
- "run-length gate … that case and everything shorter is refused": this holds at the one boundary
  tested, for a single slope. It fails for double slopes: 115 wrong-silent rows in scan 2b, 111 of them
  double-slope, to −10.9 %, at run lengths of 0.35-0.6·T60. Night Mode's default run is 2 s
  (`project/project.h:187`, `schema/model.rs:964`), so a reverberant church at 125 Hz with T60 of about
  6 s is at 0.33·T60 by default. The single-slope rows near the gate miss by −5.1 to −5.8 %.
- "PERSIST_BINS … guards against a single noisy dip (I5)": this is impossible. A backward cumulative sum
  is monotone, so it has no dips.
- "I1 fixed structurally … at zero extra cost": this holds only for an unblocked path. In scan 4, a
  receiver whose direct path is blocked, with its first energy 20 ms after the geometric arrival in a
  0.4 s room, gives +16.2 % `ok`. That is upstream's own hinge bias, re-introduced.

**I1-I7 coverage.** I1 is partial (the ball front is dropped and blocked paths are not handled). I2 is not
handled: in scan 1, plausible rooms, noise-free, it gives 24/2268 wrong-silent at −5 to −7.4 %. It
escapes the ISM −18 % rows only because the curvature inflates the SE past 5 %, which makes them `wide`,
but the range shown there still excludes the truth (`wide_out`). I3 is resolution only. I4 holds for a
single slope and fails for a double slope. I5 is not handled (defect 3). I6 refuses cleanly and crashes
on `t_arrival=None`. On I7, EDT always shows a range, but that range is wrong about 12 % of the time.
Ts/C50/C80/D50 are always `wide` with `lo == hi`, a warning that carries no information.

**Deletable (ablation, 3,682 rows, `ablate.py`; 14,717 rows, `deletable.py`):**
- `ARRIVAL_SEARCH`: 0 changes. It tests `tail[idx] > 0`, a cumulative sum that is positive whenever any
  later energy exists, so it can never move `k0`. Receipt: the occluded rows, where it should have moved.
- `PERSIST_BINS`: 0 changes. On a monotone curve it matters only at the last bin, which the run-length
  gate already refuses.
- `r2 >= 0.5` in the `ok` gate: 0 changes over 14,717 rows.

## 3. cand_practice: does not survive

**Fatal receipt.** Eval ISM row `ism|corridor|(5.53, 1.97, 1.53)|125|2ms`: EDT 1.479 against truth 0.611
(**+142 %**), `status=ok`. **Mechanism, confirmed** (`ism_mech.py`): `k0 = round(1.457/2) = 1` drops
bin 0, which holds 93.5 % of the direct sound, so the reported EDT is the reverberant-only EDT. The same
bug produces `ism|dead|(2.82,2.37,1.53)|8000|2ms`, 0.19 against 1.34 (−86 %), `ok`. There are 220
wrong-silent rows in the eval's ISM sample. This `round()` is undocumented: DESIGN says "Bins before the
arrival bin k0 are dropped". The same `k0` feeds Ts/C50/C80/D50.

**Claims against numbers**
- "Bonus: 11,520 real receiver-band rows: 0 refused, 96.6 % ok": no truth was attached, so this is an
  acceptance rate, not accuracy. **Decorative.** The script it cites is not in the candidate folder.
- "5.9 % miss rate matches the ~95 % CI": the `ok`/`wide` status depends only on r², never on the range
  (`method.py:174`). The split-half spread and the dt/2 widening can widen the range but cannot stop a
  wrong `ok`. The z3 median half-width is **40 %**, yet `ok` is set by r² alone, and there are 32 z3 and
  220 ISM wrong-silent rows.
- "Exactly as Lundeby 1995 does": Lundeby fits the late decay down to the noise floor. practice
  extrapolates the **EDT line** to the run end, so a slower late tail is underestimated. Scan 2b gives
  160 wrong-silent rows, 158 of them double-slope, to −12.4 %.
- "I3: bin centres remove most of the bias": a constant time offset does not change a slope. This part
  has no effect on EDT.

**I1-I7 coverage.** I1/I2 are broken by `round()` (above) and share defects 1 and 2 otherwise. I4 is
handled partly: the tail correction is live, changing values on 2,788 of 3,682 rows and status on 5, and
it is better than the others for a single slope, but it fails for a double slope. I5 has defect 3. I6
refuses cleanly and crashes on `t_arrival=None`. On I7, the range is decoupled from the status.

**Deletable:** the dt/2 "I3 widening" has no measured effect on status. The split-half has no effect on
status either (range only).

---

## 4. The evaluation: is "truth" what it claims, and which numbers are decorative?

- **real (3,600)**: the truth is the shipped method at 1 ms **on the same seed**. It shares the particle
  noise realisation and the run truncation, so it **cannot detect an I4 or I5 failure**. The 10 cells are
  all small rooms (60-240 m³) with at least 150 k particles, and their runs are about 1.8·T60 (Sabine) or longer.
  The "0-1 wrong-silent" on real data is evidence about I3 (2 ms against 1 ms) and nothing more. EVAL
  states the non-independence, then still ranks practice "best-calibrated on real".
- **ISM (1,920)**: the truth is independent (Definition A, direct sound as a step), but it is computed on
  an image set truncated at `lmax = c·T` (0.9 / 0.35 / 0.2 s). The methods see the same truncation, so
  **I4 is untested**. The effect on the truth is small: the corridor moves +4.6 % at 2× lmax.
- **t1 (160)**: one room (T60 0.5 s), one arrival (12.34 ms), a direct sound at −1.76 dB and a 3 s run
  (6·T60). The 160 rows are 5 dt × 4 air × 2 ball × 2 gap × 2 precision variants of **one decay**. It
  tests I3 and the ball only.
- **z3 (217)**: adversarial. 13 have a NaN truth. The Ts-tagged rows use a different EDT truth
  construction. dis-046/047 (−65 %) use R = 1.03 m and a 60-step source delay, which is not a
  user-plausible input.
- **No set exercises**: runs that end early (I4), particle noise against an independent truth (I5),
  blocked direct paths (I1), coupled or double-slope decays (apart from the ISM corridor), `t_arrival=None`,
  or the 5-10 ms steps for any physical set.
- **"wrong-silent" ignores the range.** A `wide` row whose range excludes the truth counts as safe.
  Under decision-log row 9 ("Show the range always"), a range that is wrong 11-35 % of the time is itself
  a silent wrong claim.
- **Decorative:** the band's benefit/regression rows (its "regression 2,979" is just refusals);
  upstream's `out-rng = n` and `med hw 0.00` (a point value by construction); `ms/row`, which is
  irrelevant to correctness at under 1 ms.
- **Text errors:** "leanband's tighter gate trades usable% for wrong-silent". LeanBand's usable% is equal
  on t1 and real, higher on ISM (98.8 against 98.1) and lower only on z3 (93.5 against 95.4). The trade
  is mostly `ok`→`wide`. The t1 I2 finding blames a
  "fit window offset". The mechanism is the direct energy discarded before `k0` (receipt above).
- `EVAL.md` reports no crashes, which is correct for its inputs. `t_arrival=None` crashes all three
  (`scan_misc.py (c)`).

## 5. What a simple method needs: evidence, not a new candidate

`fix_probe.py` is LeanBand with four changes and nothing added:
1. Origin at `floor((t_arr − R/c)/dt)`.
2. Schroeder samples at bin *start* times, where `S(i·dt) = Σ bins[i:]` is exact, with no I3 guess.
3. The 0 dB sample is excluded from the fit. The fit runs from `t_arr + R/c` to the first sample at or
   below −10 dB.
4. If the first energy arrives more than one bin after `t_arr + R/c`, the origin moves to that onset.

Results:
- **Scan 1** (all rooms, well-conditioned, deduplicated; `fix_probe_dedup.py`):

  | Step | LeanBand wrong-silent | Probe wrong-silent | LeanBand wide-but-range-wrong | Probe wide-but-range-wrong |
  |---|---|---|---|---|
  | 1 ms | 12/1092 | **0/1092** | 116 | 0 |
  | 2 ms | 12/988 | 1/988 | 74 | 0 |
  | 5 ms | 12/836 | 9/836 | 77 | 6 |
  | 10 ms | 6/732 | **31/732**, worse | 65 | 0 |

  At 1-2 ms the probe closes I1/I2. At 10 ms it accepts more rows and gets more of them wrong. The
  cause is I3, too few bins for the early structure, and it needs a minimum-bins or dt/window gate that
  this probe lacks.
- **Occluded:** from 10 wrong-silent to **0/60**.
- **ISM, the evaluator's own sample, same rows and same truth** (`ism_fix_check.py`, 1,888 rows):
  - LeanBand: 3 wrong-silent, the worst +29.5 %; 184 `wide` rows whose range excludes the truth.
  - Probe: 3 wrong-silent, the worst +5.5 %, all in the α=0.6 room at 1 ms and all borderline;
    23 `wide` rows whose range excludes the truth.
  - `ok` and correct: 1,196 for LeanBand, 1,257 for the probe.
- **I5:** the range can come from counting statistics instead of OLS scatter, in four lines. Compute
  `w = Σ(b_i − b_{i+1})² / (2 Σ b_i)` over the bins *after* the −10 dB point, where the field is diffuse
  and the early reflections, which are structure and not noise, are excluded. Then `N_eff = S(t10)/w`
  and `sd(EDT)/EDT ≈ 0.434/√N_eff`.
  - Synthetic: it predicts 2.4 / 4.8 / 11.6 / 3.7 % against an actual 2.2 / 4.7 / 12.1 / 3.5 %.
  - Real seeds: the ratio of actual sd to predicted sd is median 0.76 and p90 1.45. C-R3 predicts 2.24 %
    against an actual 2.33 %. LeanBand's own ratio is p90 2.76 (`noise_probe*.py`).
  - Taking the estimate over the whole early window instead over-predicts real data about 12×, because the
    early reflections dominate the differences.
- **I4 is still open.** Every gate tested extrapolates the early slope. A double-slope tail has to come
  from the *late* recorded envelope (Hirata from the local end slope), or else the run must be required
  to be long enough. Nothing in this folder tests either.
- The upstream check that the task asked for is `upstream/READING.md`. Upstream resolves none of I1-I7
  for EDT. Its own `GetTimeDecay` (C/D/Ts) does anchor at a detected arrival, and that is the onset
  fallback in change 4 above.

## Files

`inspect_ws.py` (the eval's wrong-silent rows) · `ism_probe.py`, `ism_mech.py` (ISM truth and mechanism) ·
`synth.py` (generator and Definition-A truth, checked against the band's closed form to 1e-5 in
`check_truth.py`) · `scan_i12.py` · `scan_i4.py`, `scan_i4b.py` · `scan_i5.py` · `scan_i1_occluded.py` ·
`scan_misc.py` · `real_seeds.py` · `deletable.py`, `ablate.py` · `fix_probe.py`, `ism_fix_check.py` ·
`noise_probe.py`, `noise_probe_real.py` · `common.py`. JSON outputs: `scan_i12_R0.31.json`, `scan_i4*.json`,
`scan_i5.json`.
