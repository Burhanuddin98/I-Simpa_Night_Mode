# EDT for SPPS histograms: upstream, verdict, recommendation

2026-09-27. Built from `upstream/`, `inventory/`, `practice/`, the three candidates, `eval/` and `critique/`, plus
one new measurement: `final/method.py`, with its logs in `final/eval_final.txt`, `final/scans_all.log` and `final/dev/`. No solver was run.

## 1. How upstream I-Simpa handles I1-I7: it handles none of them

I read `projet_calculation.cpp` (GUI post-process) and `src/spps/` cold at tag `v1.4.0_snapshot_14_01_2026`.

| | Upstream | Receipt |
|---|---|---|
| I1 | The fit starts at the first recorded bin: EDT's `Compute_TR_Param(0,10)` makes `GetTimeRange` set `debT = timeTable[0]` on every call, so pre-arrival silence is fitted. Noise-free exponential with EDT = 1 s: **+0.3 % at 2 m, +20 % at 20 m**. The fix, `GetTimeDecay`, is in the same file and used for C/D/Ts, but never for EDT or RT. | `:143-170`, `:936`, `:127-139` |
| I2 | No special treatment. The direct sound uses the same chord-length estimator as every reflection. | `reportmanager.cpp:219-224` |
| I3 | Not handled. Energy is binned by integer step and the time within the step is discarded. Default step 10 ms. | `reportmanager.cpp:526` |
| I4 | Not handled. Energy still in flight at run end is dropped (one `partAlive` count). Kills at `pow(10,-5)` of each particle's own energy (−50 dB; PRACTICE's `/10` is wrong) are never accounted. | `CalculationCore.cpp:91`, `sppsNantes.cpp:75` |
| I5 | Not handled. No variance, count or interval anywhere. | READING §6 |
| I6 | Not handled. No check on the crossing, run length, slope sign or r². A 1-point window writes `NaN` (reproduced). | `:95-219` |
| I7 | One float32 per band, 2 decimals, no range, no flag. | `mathlib.h:43` |

Upstream has no fix to borrow. The one reusable idea is `GetTimeDecay`'s arrival anchor, used below for blocked paths.

## 2. Verdict on the current design

**The current design is over-complicated, and the cause is its premise.** It set out to give a *certified* range under any arrangement of energy inside a bin, at
1/10 JND. That cannot hold for I4: GATE2 showed that no model-free bound on a receiver's unrecorded tail exists. Every hole a critic found
was patched with more machinery (air-rate f32 correction, a ×1000 diffuse tail, a twin solve, 505 lines of certified bisection)
instead of reopening the premise, and acceptance on real data was not measured until gate 4b. The result is 3,433 lines and 60-500 ms per row.
**Usable EDT: 0/3,600 real, 0/217 z3, 0/1,920 ISM and 44/160 t1.** The headline "0 wrong" holds, but only on the rows the band answers, and the old eval's containment tables used `BIG_TAU = 1e9`, which measures containment, not acceptance (INVENTORY (c)).

**Worth keeping:** the arrival-anchored origin, named refusals, a range shown with every value, the Theorem-CD idea for C/D (only the bin straddling te is ambiguous), and the test corpus: t1 golden, z3, ISM, real noise-cal seeds, the upstream port and the critique's scans. That corpus is what broke every candidate.

## 3. Recommendation

**No candidate survives the critique.** All three drop direct sound that falls before their origin bin (LeanBand is +29.5 % while `ok` on the ISM corridor at 1 ms; practice's `round()` gives +142 %), fit the 0 dB sample, use OLS scatter as the noise range (6-35× too narrow), and check the run length with the early slope.

**What survives is a graft:** LeanBand plus the critique's fixes plus three small parts, 75 lines of code in `final/method.py`.
1. **I1/I2.** 0 dB is at the ball's front, `floor((t_arr − R/c)/dt)`. The fit uses exact samples `S(i·dt) = Σ bins[i:]` from `t_arr + R/c` down to the first sample at or below −10 dB. The 0 dB point is not fitted. If the path is blocked, or `t_arr` is `None`, 0 dB is the first recorded energy.
2. **I3/I6.** Refuse if there are fewer than 8 samples in the window (`step_too_coarse` / `direct_only`), or if the slope is ≥ 0.
3. **I4.** Let E1 and E2 be the energies in the last two 20 % blocks of the run. `U = E2²/(E1−E2)` is Hirata's exponential through the *late* envelope. Refuse `run_too_short` if U exceeds 2 % of S(−10 dB). Under that threshold the truncation error is ≤ 0.46 % for a single slope and ≤ 1.58 % for a double slope (`dev/tail_calib.log`).
4. **I5/I7.** The range is `slope·(1 ± 2√(SE_fit² + σ²))`, with a floor of 0.5 %. σ = `0.434·√(w/S₁₀)`, the Poisson count noise of S(−10 dB), where w is the energy per hit estimated from the bin scatter after −10 dB. The status is `ok` if the half-width is ≤ 5 %, otherwise `wide`.

**Evaluator's rows** (EVAL.md's sets and truths). Each cell reads graft · LeanBand · upstream · current band. Wrong-silent means `ok` and more than 5 % off. Benefit/regress is counted against upstream.

| Set (truth) | usable % | ok % | wrong-silent | truth outside range | median half-width | benefit/regress |
|---|---|---|---|---|---|---|
| t1 160 (exact) | 100·100·100·27.5 | 100·100·100·27.5 | **0**·0·108·0 | **0**·56·160·0 | 0.5·0.8·0·0.3 % | 108/0 · 108/0 · – · 38/46 |
| z3 217 (adversarial) | 86.6·93.5·100·0 | 12.9·16.1·100·0 | **4**·3·178·0 | 18·40·204·– | 11.8·9.5 % | 13/15 · 20/14 · – · 0/26 |
| real 3,600 (not independent) | 99.8·100·100·0 | 98.6·100·100·0 | **0**·1·621·0 | 570·458·3,600·– | 2.1·2.0 % | 614/42 · 620/0 · – · 0/2,979 |
| ISM 1,920 (independent) | 83.5·98.8·100·0 | 45.4·62.5·100·0 | **0**·3·781·0 | **1**·212·1,888·– | 4.4·3.6 % | 175/**411** · 330/241 · – · 0/1,107 |

**Critique's scans**, with an independent truth except the last row, which is scored against the 10-seed mean. Each cell reads graft · LeanBand.

| Scan | wrong-silent | other |
|---|---|---|
| I1-I3: 3,648 noise-free rows at 1/2/5/10 ms | **0/0/1/0** · 12/12/12/6 | the graft refuses 0/2/40/160, i.e. 22 % at 10 ms |
| Blocked or weak direct sound, 180 rows | **0** · 10 | |
| I4 truncation, 1,980 rows | **0** · 115 | the graft refuses every run ≤ 0.4·T60 and 106/360 at 0.5-0.6·T60 |
| I5 noise, 2,400 seeds | 16 · 894 | truth inside the range: **93.7 %** · 16.4 % |
| Real seeds against their 10-seed mean, 2 ms | 19 · 15 | mean inside the range: **95.1 %** · 86.0 %; seed sd / shown sd p90 1.47 · 2.76 |

**Where the graft does worse:**
- **Its remaining wrong-silent rows are statistical, not structural.** 13 of its 19 on the real seeds are in C-R3 (σ ≈ 2.3 %), which is the miss rate of about 3 % that a 2σ range allows at the 5 % edge.
- **On ISM it regresses on 411 rows:** 231 `wide`, 154 `run_too_short` and 26 `step_too_coarse`.
  - All 220 of its ISM `run_too_short` refusals are in the corridor, where the image set stops at 0.9 s. The truth stops at the same point, so these cannot be scored; the critique moved that truth by +4.6 % by doubling the image set.
  - σ reads specular spikes as noise: the noise term on its own turns 169 correct `ok` rows `wide` (`dev/ablate_eval.log`).
- **On z3, 3 of its 4 wrong-silent rows** have a ball of 0.9-1.3 m and a direct sound that takes 6.7-8.5 of the 10 dB. A `direct_dominates` refusal at −6 dB would remove those 3 and touch no `ok` row on real or ISM. It is not adopted.

## 4. Delete and keep

**Delete from `band_early.py`:** `Setup` and its tube bounds; `premise_refusal` (0 firings in about 94,000 rows); the EDT search (`:718-1222`); `ts_band` and Dinkelbach (`:634-713`, refused 3,600/3,600 real rows); `all_bands`, `_package` and the twin solve (`:1225-1343`); `production_tail_max`, `production_inputs` and `min_run_length` (the ×1000 model, `:191-330`); `air_rate_mismatch_bound` and `widen_eps_for_air_rate` (at most 0.0015 of the 1/10-JND unit, only at 0.1-0.2 ms, never exercised on real data); `spps_rel_eps` and `safe_contributions_per_bin` (f32 rounding, orders of magnitude below a σ of 1-13 %).
**Delete from `SPEC.md`** §0, §1.1, §1.3, §2, §4 (163 lines), §5, §7 and §8 (§8 is a seed-batch layer that was never built), and replace them with one page on the graft. **Delete from `test_band_early.py`** T4 and T5.
**Keep:** `exact_params` and the T1/T7 generators as fixtures (re-freeze the 167 golden cases against the graft); `upstream_edt` and its port; the Theorem-CD idea, as a short straddle-bin bracket for C/D (not yet written); the refusal names; the corpus, with the critique's scans as the regression suite for M8b.

## 5. Decision log

**Rows this supersedes:**
- **Row 7:** EDT uses the graft, not the band. C/D keep the straddle-bin bracket. Ts is open.
- **Row 12:** the tail is *estimated* from the late envelope, assuming one exponential, and is used only to refuse. It is never added to the value and never assumed to be 0. "Provably negligible" goes, because GATE2 showed it is unreachable.
- **Rows 14 and 15:** deleted.
- **Row 10:** M8b now gates a port of about 75 lines.
- **Row 11:** stands, with a new receipt. At 10 ms the graft refuses 22 % of scan-1 rows; at 1 ms it refuses none.

**Burhan's calls, because users see them:**
1. **Row 9's "guaranteed range" becomes a range of about 95 % (fit plus particle noise).** Choose Z.
   - Z = 2 gives 19 wrong-silent on the real seeds and 16 in the noise scan.
   - Z = 3 gives 6 and 2, but correct `ok` rows fall from 3,532 to 2,933 on the real seeds and from 871 to 613 on ISM.
2. **Short runs are refused, not widened.** At the 2 s default run, every room with T60 ≥ 5 s is refused, and some with T60 of 3.3-4 s are too. Widening the range instead of refusing gave 26 wrong-silent at 0.4·T60 (`dev/tail_widen.log`). The choice is to keep the refusal with its reason, or to raise the default run length.

## 6. Still unmeasured

- **Ts, C50, C80 and D50** under the new origin: only EDT was evaluated.
- **Real SPPS runs that end early:** none exists in the repo. The I4 evidence is synthetic, with a double-slope rate ratio of 3 only.
- **An independent truth for real SPPS:** the "real" truth is the shipped calculator run on the same seed.
- **Inputs never tested:** two sources; a celerity gradient (`t_arrival=None`, one smoke-test row, 0.5 % error); `trans_epsilon` ≤ 2; 5-10 ms steps on a physical set.
- **The thresholds** (8 samples, 2 % tail, 20 % blocks, 0.5 % floor) were chosen on the critique's scans and V-R6, then checked on t1, z3, real and ISM. They have not been tested on data collected after they were chosen.
