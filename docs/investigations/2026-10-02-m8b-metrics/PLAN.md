# M8b, the other seven metrics: plan

Written 2026-10-02 on branch `m8b-edt`. Planning only: no solver was run, no code and nothing under
`docs/investigations/2026-09-27-edt-heldout/` was touched. Not committed. Paths are relative to the worktree.
Sources read: `docs/scope.md`, `decision-log.md` rows 1-36, `rebuild-plan.md`, `v1.1-backlog.md`, `params.md`, `results.md`,
`2026-09-29-m8a/` (SPEC, RUN, VERDICT), `2026-09-30-m8b-gate/`, `2026-09-27-edt-heldout/` (PREREG, PREREG-2, VERDICT, DIAGNOSIS,
HARNESS-PLAN-2, `frozen2/`, `harness2/m8b/`), `2026-09-27-edt-simplify/FINAL.md`, `2026-09-25-cd-limits/`, upstream
`src/isimpa/data_manager/projet_calculation.cpp` (the audit calls it `calc.cpp`).

## In plain words

Burhan ruled that M8b is done only when EDT, T20, Ts, C50, C80, D50, SPL and STI each pass a test; dB(A) and G get unit tests on
top of SPL. EDT is being tested now. This plan covers the other seven, in four groups. **T20** is nearly free: it rides on the
M8a bed, whose 433 runs are still on disk. **C50, C80, D50 and Ts** are tested together against the exact echogram of a box
(image sources), closed-form decays and long high-particle SPPS truths, the same way EDT is. **SPL** is tested against the
same exact echogram and against M8a's runs, with a fresh absolute scale; dB(A) and G follow as unit tests. **STI** goes last: the
product has no STI code yet, upstream's has bugs, and the standard's text is not in the repo. The riskiest piece is STI, then the
early-energy group (the product may refuse too often, as EDT's first round did). What Burhan must choose is in the last section;
the ones that block work are 1 (the standards) and 3 (what "passes" means).

## What is reused from the EDT work

| Piece | Where | Reused for |
|---|---|---|
| Exact expected SPPS ball echogram of a specular box (image sources, ball integral, per-step air) | `harness2/m8b/_ism.py` `echogram`, `air_factor` | C, D, Ts, T20, SPL, G, STI truth for specular rooms |
| Closed-form decays (direct + gap + double slope), histogram and truth | `harness2/m8b/synth_fresh.py`, `edt-simplify/critique/synth.py` | closed forms of every metric (new truth functions, same generator) |
| 7 fresh SPPS rooms G1-G7, K = 4 truths at 1M particles and 0.1 ms, 150k tested runs at 1/2/5 ms, 3 seeds, both modes | `HARNESS-PLAN-2.md` P13-P19, `rooms2.py`, `driver.py` | SPPS rows for all seven; no new solver run for them |
| Row scoring (ok / usable / wrong-silent / covered), subgroups, upstream-port comparison | `score.py` (P27-P31), `_upstream_edt.py` | same definitions; upstream ports for C/D/Ts/SPL/T20 from `crates/simpa-core/tests/params_upstream_gui.rs` (f32, reproduces the stored table, 216 of 216) |
| M8a bed: matrix, Kuttruff, independent Lambert transport (`params::lambert::decay`, `time_step_s` settable), judge, `simpa bed --from` | `2026-09-29-m8a/`, `crates/simpa-core/src/bed/` | T20, C/D/Ts, SPL on Lambert boxes |
| Hash-pinned freeze, failing-first tests, B1-style addendum | `ADDENDUM-B1.md`, `freeze2.py` | same discipline, one addendum per group |

The held-out rule: the Rust product code for these metrics is not being tuned on round-2 data, so round-2's rooms, truths and
runs are fresh for it. Nothing here reads a round-2 row before the EDT round-2 harness has written its results, and each group's
criteria are committed (and the product's `simpa.exe` and `params` hashes pinned) before that group is scored. New scoring code
lives in `docs/investigations/2026-10-02-m8b-metrics/harness3/`, importing `harness2` read-only.

## Shared definitions (all seven)

| Item | Rule (technical call, Jarvis; Burhan may overrule) |
|---|---|
| Limens | ISO 3382-1 Annex A as commonly stated, **not read** (`params.md` "Sources": paywalled): EDT/T 5 %, C 1 dB, D 0.05, Ts 10 ms, G/SPL 1 dB; STI 0.03 (IEC 60268-16, as commonly stated). The product's own refusal limit is 1/10 of each (`decay.rs:62-84`) |
| answered | value with no refusal; usable = answered |
| wrong-silent | answered and `|value - truth|` beyond the limen |
| covered | truth within value +/- 2.5 `mc_sd` + 1/10 limen (2.5 is Burhan's Z for EDT, 2026-10-02 00:24) |
| truth uncertainty | a row whose truth uncertainty exceeds 1/10 limen leaves the denominators and is counted |
| Receiver | the ball the solver records (R = 0.31 m default). Truth is the ball's. A **ball-versus-point** difference (ISM with R -> 0) is reported per metric and bounded at 1/10 limen for R <= 0.5 m |

## Per metric

### T20

1. **Definition.** ISO 3382-1 Annex A (not read), -5 to -25 dB, least squares on the Schroeder curve, `T = -60/slope`
   (`params.md` "Decay times"). Tolerance: 5 % limen, bed cross-check 0.5-1 % (below).
2. **Product / upstream.** `decay.rs:395` `decay_time(.., DecayRange::T20)`, fit over time on the log-linear curve, refusals
   `range_too_short`, `range_not_reached`, `truncated`, `unresolved`, noise (`noise.rs`, T20 is in the four calibration rounds).
   Upstream `Compute_TR_Param(5, 25)` (`projet_calculation.cpp:190-219`) fits from the first bin, so a pre-arrival silence is fitted:
   +0.3 % at 2 m, +20 % at 20 m for a 1 s decay (`FINAL.md` I1).
3. **Reference.** (a) The independent transport's T20 (same code path as its T30, `lambert.rs:1214`, one range changed) at
   0.5-1 % on the M8a cells, from the 433 runs already on disk (`C:\tmp\nm-m8a-bed\20260929T093134Z`, re-read in 290 s). (b) Closed-form
   double-slope decays (Synth), where T20 differs from T30 legitimately. (c) K = 4 SPPS truths and ISM-fresh for T20 at product
   counts. Kuttruff is T60-like: reported against T20 at 5 %, not gated (T20 and T30 differ by curvature in Lambert boxes).
   It cannot cover: scattering 0 walls (M8a's E6 refuses them), coupled rooms beyond G3, single-slope assumption.
4. **Failure modes (by analogy with EDT).** The fit starts at -5 dB, past the direct step: no shelf, so no `receiver_too_large`
   analogue is expected; check it on the dead room G7 and the largest balls. Noise: a 20 dB window is shorter than T30's, so
   T20 is noisier per run; M8a's counts were sized for T30 (sd <= 0.8 %). Run too short: window ends at -25 dB, nearer than T30's,
   so milder. Coarse step: 2 bins per range minimum (`MIN_REGRESSION_BINS`). A plain tolerance suffices; no range needed beyond
   the noise refusal.
5. **Test.** Sets: S4 (M8a re-read, gated C-style: 95 % interval of cell mean vs transport T20 inside a tolerance fixed from the
   seed spread alone, before any T20-vs-transport number is computed; INCONCLUSIVE extension rule of M8a SPEC 5.1), S1 (Synth,
   J1), S3 (SPPS-fresh, J2-J4), S2 (ISM, J1). Reused: all of the above; new: T20 truth functions and the transport T20 phase
   (about 30 min, as M8a's). **0 new solver runs** for S4, 0 for S3.

### C50, C80, D50

1. **Definition.** ISO 3382-1 (as commonly stated): `C_te = 10 lg(E[0,te] / E[te,inf))`, `D50 = E[0,50 ms] / E[0,inf)`, time zero at the
   direct sound (`params.md` 671-690). Limens 1 dB and 0.05; test uses them, product limits 0.1 dB and 0.005
   (`decay.rs:62-84`; `2026-09-25-cd-limits/`).
2. **Product / upstream.** `decay.rs:1382` `clarity_db`, `:1417` `definition` on one continuous Schroeder curve, arrival `Known` from
   d/c with half-width R/c (`report.rs:1074-1075`) or `Detected` (both onset-bin ends computed, refused `unresolved`), tail and missing
   energy bracketed (`settle` `:1127`). Upstream `Compute_C_Param` `:350-383`, `Compute_D_Param` `:386-415`: `GetTimeDecay` (`:127-139`)
   anchors at the last bin before the first change from bin 0 (arrival anchor is right), but `GetSumLimit` includes both edges (`:102`) so C's
   boundary bin counts as early and late (+1.05 dB at tutorial 1, `params.md` 719-728), the threshold 1e-18 is an absolute Pa^2, the
   window is cut on bin labels, no noise, step or tail handling.
3. **Reference.** (a) ISM ball echogram of a specular box, fine bins (0.02 ms), exact expected SPPS record, truncated where the
   remaining late energy is <= 2 % of the late energy at te (so C moves <= 0.1 dB; a rule like EDT's P19 u <= 1 %). (b) Synth closed
   form: direct step at the arrival, gap, double slope: `E[0,te]`, `E[te,inf)` are sums of exponentials. (c) K = 4 SPPS truths at
   0.1 ms (straddle bin 1/10 as wide), computed from the same arrays. (d) Lambert transport at 0.1 ms bins against M8a's runs (S4). **Theorem-CD**
   (`FINAL.md` section 2: only the bin straddling te is ambiguous) gives the planted test: te placed at 20 phases inside a bin on a
   known decay. Cannot cover: the real ball-versus-point question (reported), absorption-edge geometries beyond G1-G7.
4. **Failure modes.** Straddling bin (the main one): at 10 ms te = 50 ms sits inside a bin holding 1-2 % of the early energy, so most
   refused (`rebuild-plan.md` line 148: refused at most receivers at 1 ms before the arrival was given; since, energetic C50 passes in
   12,174 of 21,060 receiver-bands, `results.md` "C and D limits"). Arrival not known (blocked path, `t_arrival = None`): onset-bin
   ambiguity, `unresolved`. Origin vs ball crossing (h = R/c): the direct sound spans 2R/c = 1.9 ms inside the window, so no shelf as in
   EDT's fit, but early reflections inside the ball window move the split by up to 1.5 ms: bounded by the ball-versus-point report.
   Count noise: early energy rests on few reflections; bins in a window are summed, so correlation matters less than for a slope.
   Run too short: the late energy is the denominator of C, a truncated tail raises C (limit 0.1 dB, tail bound from `tail_ok`).
   Emission delay and trans_epsilon floor (missing energy, `missing_moves`). **A refusal rule already exists** (value or refuse
   at 1/10 limen): a plain tolerance plus the answers-often-enough test decides whether refusals are too many, as EDT's H4 did.
5. **Test.** Sets S1, S2, S3, S4 + a planted-te scan (S1), J1-J5 per metric (`J` below). Answers often enough J4 >= 90 % at 1 ms,
   T60 <= 3 s, R <= 0.5 m (decision 4). Reused: whole EDT scorer. New: C/D/Ts truth functions on `_ism.py` and Synth; the
   transport at 0.1 ms bins; the Rust scoring shim (below). If the shipped Rust fails J4, the fix is Theorem-CD's bracket and a fresh set.

### Ts

1. **Definition.** ISO 3382-1 (as commonly stated): `Ts = int(t E)/int(E)` from the arrival, `= int S du / S(0)` (`params.md` 671-690).
   Limen 10 ms (product limit min(1 ms, 0.5 % of Ts)).
2. **Product / upstream.** `decay.rs:1446`. Upstream `Compute_TS_Param` `:418-447` weights by the **absolute** bin label
   (`:432`): Ts includes d/c (13 ms at tutorial 1) and half a bin: -13 ms and -5 ms against ours (`params.md` 728). Both exceed the 10 ms limen
   in the gap alone; the bug-compatible port shows it.
3. **Reference.** As C/D (ISM, Synth closed form `tau`-weighted sums, K = 4 truths, transport at 0.1 ms). Cannot cover: nothing extra.
4. **Failure modes.** Tail: Ts weights late energy by time, so truncation (run too short) moves it most: how much a 2 % tail share moves it is
   not measured; the tail scan below measures it. Count noise: heavier than C (late bins, large t): the noise model covers `Ts`.
   Origin: error of one ball half-width (1.5 ms) is 15 % of the limen: bounded by the ball-versus-point report. No straddle problem
   (no edge). Plain tolerance suffices; refusal on tail and noise exists.
5. **Test.** With C/D (one scorer, one freeze). Reused: all. Extra: a tail scan at 0.3-3 T60 run lengths (as the critique's I4 scan).

### SPL (and dB(A), G on top)

1. **Definition.** `SPL = 10 lg(sum_k B_k / p0^2)`, `p0 = 20 uPa` (`params.md` 746-758): the steady-state level of a source emitting continuously, over
   every bin; the solver's own energy, `rho c` as SPPS computes it (413.25 at 20 C). Limen 1 dB (ISO 3382-1 G, as commonly stated); product limit 0.1 dB.
   **dB(A)**: octave-band levels plus the A corrections of IEC 61672-1 (125 Hz -16.1, 250 -8.6, 500 -3.2, 1 k 0, 2 k +1.2, 4 k +1.0, 8 k -1.1), energy sum.
   **G**: ISO 3382-1 `G = SPL - SPL_free(10 m)` for the same source, so `rho c` cancels: `G = SPL - [Lw + 10 lg(rho c / (4 pi 100 p0^2 W))]`.
2. **Product / upstream.** `decay.rs:422` `spl_db`, `:1480`; M7 gate (c) holds the *direct field* to the exact free field (mean of 12 receiver-bands
   within 4 sd, seed spread 0.004 dB, `results.md` 509-582). **No dB(A), no G in Rust.** Upstream `dB_Sum_Param` `:220-240`; `dBa_Sum_Param` `:262-300`
   applies the **third-octave** table (`appconfig.cpp:106`, correct values) at nominal frequencies, and sums with the global row; surface
   receivers use `to_deciBelRsurf` with reference 1e-12 (`:50`), a different quantity (the +26 dB family, `params.md` 758).
3. **Reference.** (a) ISM absolute total: `W rho c /(4 pi p0^2) * sum_i w_i <1/D_i^2>_ball` with the closed-form ball average
   (`results.md` 563-569), per-step air, to a tail <= 1/10 limen: exact for a specular box (new SPPS set, below). (b) The Lambert transport,
   scaled to SPPS's `rho c / V` normalisation, validated first on its direct part against the M7 formula; against M8a's 433 runs (SPL is in
   them, 80 dB source). (c) K = 4 truths for the SPPS-fresh rows. (d) Classical Barron-Lee revised theory
   (`10 lg(T/V) + 45` for G, as commonly stated): reported only, since the formula is a diffuse-field approximation good to about a dB.
   TCR's SPL is a different method, not tested here. Cannot cover: scattering partial rooms (no exact reference), 125 Hz wave effects
   (SPPS is geometric; not a product claim).
4. **Failure modes.** Run too short (SPL integrates to infinity; the tail bound exists), energy loss (verdict limit 1 % = 0.04 dB; killed
   particles at -50 dB, 4e-5 dB), reference mix-up (the 26 dB bug: say-NO through `faults::LevelReference`), ball average (0.05 dB at 2 m),
   air (`rho c`), several sources summed, receiver on surface (maps: see decision 9). Count noise: a sum over all bins, so mild,
   `mc_sd` calibrated. Plain tolerance suffices. dB(A) fails only through the table or the sum.
5. **Test.** S5 (specular boxes vs ISM, 2 rooms x alpha {0.1, 0.4} x {random, energetic} x seeds 1-10; the M8a bed refuses scattering 0, so
   this is a new bed file), S4 (433 runs vs transport), S3 (J2-J4), S1 n/a, plus the M7 gate (c) as regression. J1 on ISM absolute at 0.25 dB bed
   tolerance (to be fixed from seed spread alone). **dB(A):** unit tests (single band passes at its weight; flat spectrum sum; third-octave vs octave consistency
   on a pink spectrum to the standard's own tolerance; upstream's table matches IEC 61672-1 at 27 bands). **G:** unit tests (free field at 10 m reads 0 dB;
   room G vs ISM-exact G; `rho c` cancels; 1 dB shift in `Lw` leaves G unchanged). About 80 SPPS runs for S5; the rest reused.

### STI

1. **Definition.** IEC 60268-16 ed. 5 (not in the repo, not read; ed. 4 is what upstream cites): the modulation transfer function from the squared impulse
   response (Schroeder), `m(F) = |int h^2 e^{-j2 pi F t}| / int h^2 * (1 + 10^{-SNR/10})^-1`, 14 modulation frequencies 0.63-12.5 Hz, seven
   octave bands 125-8000 Hz, `SNR_eff` clipped to +/-15 dB, `TI = (SNR+15)/30`, `MTI` = mean over F, `STI = sum alpha_k MTI_k - sum beta_k sqrt(MTI_k MTI_k+1)`,
   gender weights, auditory masking from the lower band and the absolute reception threshold. **Clause numbers, weights and thresholds must be
   pinned against the licensed text** (decision 1). Limen 0.03 (commonly stated).
2. **Product / upstream.** **No STI in the product.** Upstream `Compute_STI_Param` `:586-790`: `if(gen='F')` at `:748` is an assignment, always true, so
   the male branch is dead and every STI uses female weights (callers pass 'M', `:947`, `:1206`); the speech level is the source's own band level, not
   the standard's speech spectrum ("TODO: compensation for speaker spectrum", `:584`); noise is an NC curve only; the masking term at `:693-703`
   uses band k's own level (the standard as I recall it uses the lower band's: **unverified**); `Iamk[0] = 0`; float32 sums. The weights
   in the code match ed. 4's female column as I recall it; ed. 5 not compared.
3. **Reference.** (a) Known answers: `m = 1/sqrt(1 + (2 pi F T / 13.8)^2)` for an exponential decay (exact), `STI(m=1)` at the limit = `sum alpha - sum beta = 1`
   (male, as I recall it), `m = 0` gives 0; (b) an **independent Python implementation written from the licensed text**, hash-pinned, not from upstream, fed Synth
   closed forms, the ISM ball echograms and SPPS K = 4 truths; (c) an upstream **bug-compatible port** as the H5 comparator (the f32 `params_upstream_gui.rs` style);
   (d) a sensitivity scan: STI vs SPL error +/-1 dB (masking depends on level) to prove SPL's limen is enough. Cannot cover: a measured room (v1.1 item 2, BRAS), STIPA.
4. **Failure modes.** Run too short: the lowest F is 0.63 Hz (period 1.6 s), so a truncated tail lowers m at the low end; same tail refusal as EDT. Count noise: adds
   energy-response modulation at every F, biasing `m` up where the true m is small (reverberant rooms, high F): a bias, not just spread; test with seed replicates, debias
   if resolved. Coarse step: m is nearly step-insensitive (sinc(pi 12.5 dt) = 0.9998 at 1 ms, 0.990 at 10 ms) - to be measured, cheap. Level dependence: masking uses
   absolute levels, so SPL must pass first. Direct sound (DRR drives STI): M7 gate (c). Bands: 8 kHz octave must be in the run (7-band requirement), air on. Ball: negligible.
   Needs a decision on what level and spectrum the "speech" is (decision 6). A range is not needed; a plain tolerance on `STI` with the noise `mc_sd` suffices.
5. **Test.** S1 (closed forms, J1 at 0.01 = 1/3 limen, hand-checked cases), S2 (ISM ball echograms, J1), S3 (SPPS-fresh, J2-J4), S5 (specular boxes), the sensitivity
   scan, and the bug-compatible port. Reused: all of the EDT sets. New: the product's STI code (failing-first), the independent implementation, the port.

## Common test, per group (what "J" means)

| | Criterion (pre-registered per group, before its first scored row) |
|---|---|
| J1 | Noise-free sets (S1 Synth closed form, S2 ISM): wrong-silent <= 0.5 % of answered rows, each set, every radius and step; plus >= 99 % of answered rows within 1/10 limen + ball-versus-point bound (the product's own 1/10 claim) |
| J2 | SPPS-fresh (S3), each mode separately: coverage >= 90 % of answered rows, wrong-silent <= 3 % |
| J3 | No subgroup with >= 20 answered rows has wrong-silent > 10 % (round 1's subgroups) |
| J4 | Answers often enough: >= 90 % answered at 1 ms, T60 <= 3 s, R <= 0.5 m; refusals reported by reason and set |
| J5 | Wrong-silent count strictly below the upstream port's on every set |
| J6 | Bed sets (S4 M8a re-read, S5 specular): cell mean's 95 % interval inside the tolerance, M8a's three-outcome rule, one extension to seeds 11-20 |
| J7 | Say-NO: each gate fails when its own known bug is planted (upstream's Ts label, C boundary bin counted twice, `p0` 26 dB, female-only weights, A table off) |

Sets: **S1** Synth closed form (needs new per-metric truths), **S2** ISM-fresh ball echograms (12 rooms), **S3** SPPS-fresh rooms G1-G7 with K = 4 truths,
**S4** M8a's 433 runs against the transport, **S5** specular boxes (scattering 0) against ISM, **S6** unit tests.

## A shim the product needs for scoring

`simpa results` reads run folders only. S1 and S2 need the product's Rust metric code fed a histogram. A test-only shim (an example or test file calling
`params::decay::{evaluate, decay_time, clarity_db, definition, centre_time_s, spl_db}` on a series read from NPY, printing value or refusal) is the one code
addition before scoring; its hash is pinned in each group's addendum. Not written here.

## Order of work

| # | Group | Why this order | New solver runs | Gate |
|---|---|---|---|---|
| 0 | Shim, freeze machinery, failing-first scorer tests, per-metric truth functions | nothing is scored before it | 0 | tests red, then green |
| 1 | **T20** | shares T30's bed and the M8a runs; first receipt that the bed generalises | 0 | J1-J4, J6 on S4 |
| 2 | **SPL**, then **G** and **dB(A)** | STI needs a tested SPL; SPL and the unit tests are cheap | ~80 (S5) | J1-J4, J6, J7; G and dB(A) unit tests |
| 3 | **C50, C80, D50, Ts** | one arrival-anchored energy split, one scorer; most likely group to fail on refusals | 0 (S3 reuses round-2's 172) | J1-J7 |
| 4 | **STI** | builds on echogram + SPL; code does not exist yet | 0-80 (reuses S5) | J1-J7 plus the sensitivity scan |
| 5 | Optional attacker round, C/D/Ts and STI only | EDT's attacker round was held back for the fixed method | 0 | H6-style, 3-judge panel |

Groups 1 and 2 can run together (no shared resource: T20 reads disk, SPL needs 80 runs); group 3's ISM scoring is the long pole and can start while group 2 solves.
EDT round 2 must finish first on the SPPS data (the scorer reads its outputs after the run ends, never while it runs).

## Cost forecast (a forecast, not a promise; measured sources only)

| Part | Forecast | Source |
|---|---|---|
| T20 on M8a: re-read + transport T20 | ~0.6-0.9 h | `REJUDGE.md`: re-read 290 s, transport 1,787 s (1,559 s original) |
| SPL S5: ~80 runs at 12 jobs | ~0.3-0.6 h | `RUN.md`: 433 runs in 19,621 s at 12 jobs (random high-count cells dominate; energetic 1.5M runs shorter) |
| SPL S4 re-read + transport scaling | ~0.7 h | as T20 (shared with it if run together) |
| C/D/Ts: S3 reading round-2's 6,912 rows | ~0.1 h | `HARNESS-PLAN-2.md` section 8: ~4 min |
| C/D/Ts: ISM-fresh echograms, 12 rooms, 2 workers | **~3-4 h** | run1: 27,005 room-seconds / 2 workers; the draw was 2.44e8 images (`HARNESS-PLAN-2.md` section 8). Echograms are not cached by the EDT harness, so they are recomputed |
| C/D/Ts: transport at 0.1 ms bins on M8a | ~0.6 h | as T20, 10x finer bins grow memory, not rays |
| STI: ISM scoring (7 octaves) + SPPS rows + scan | ~1-2 h | same ISM cost per room-band, fewer bands |
| **First pass, machine time** | **~7-10 h** | sum, partly concurrent; excludes authoring, review and fixes |
| One failed round on the riskiest group (C/D/Ts or STI) | +4-5 h | EDT needed two rounds (run1 failed H1-H3, `VERDICT.md`); round 2's harness plan forecasts ~4-4.5 h |

## Decisions that are Burhan's (recommended default first, trade-off after)

| # | Decision | Default | Trade-off |
|---|---|---|---|
| 1 | Obtain the standards: IEC 60268-16 ed. 5 (STI), ISO 3382-1:2009, IEC 61672-1 | Yes for IEC 60268-16, recommended for ISO 3382-1 | Without the text every definition is "as commonly stated" and STI's weights, masking and thresholds cannot be verified; costs money and a day |
| 2 | Limens as the pass line (C 1 dB, D 0.05, Ts 10 ms, SPL/G 1 dB, T 5 %, STI 0.03) | The limen, with 1/10 limen reported | A consultant's tolerance may be tighter than a limen; tighter pass lines mean more refusals |
| 3 | "Passes": value or refusal, as proposed, versus EDT-style "show the range always" for every metric | Value with refusal; range only where noise-limited | A range for all is consistent with EDT (row 9) but clutters C/D and SPL, which are tight by construction |
| 4 | "Answers often enough" threshold for C, D, Ts and STI | 90 % at 1 ms, T60 <= 3 s, R <= 0.5 m (EDT's H4) | Lower accepts more refusals (honest, annoying); higher forces a bracket rule |
| 5 | Ball versus point receiver | Test against the ball; report ball-vs-point; refuse above 1/10 limen | Point truth is what ISO describes but is not what the solver measures |
| 6 | STI's speech spectrum, level, noise and gender | Standard speech spectrum; male and female both computed, male shown; NC curve as background noise | Upstream uses the source's own band level; the standard's spectrum is correct but needs a new input |
| 7 | Reuse round-2's rooms and truths for the other metrics | Yes | Cross-contamination if a later fix is tuned on them; fresh draws cost another ~28 truth runs (2 h 13 min single core) |
| 8 | Barron-Lee revised theory | Reported only | As a gate it needs a ~1 dB tolerance and tests the theory, not the solver |
| 9 | SPL on surface-receiver maps (upstream's 1e-12 reference) | Include in M8b: one ISM patch test and a unit test | Excluded means M12 maps show an untested number or are hidden |
| 10 | Attacker round for C/D/Ts and STI | Yes, those two | Skipping saves hours but leaves unknown unknowns; EDT's was held back, not dropped |
| 11 | New specular-box bed (scattering 0) and its runs (about 80) | Yes | Without it SPL/C/D are not tested against an exact reference, only against the transport |

## Open points to check (not decided here)

- Whether the 433 M8a run folders and the 40 `.npz` decays on `C:\tmp\nm-m8a-bed\20260929T093134Z` survive until group 1; the no-delete guard applies, and `B:` is exFAT with a temp-file leak history (`MEMORY.md`).
- The product's T20 refusal rate at 1 ms in the M8a cells (read from the run JSONs; no run needed).
- Where the EDT Rust port lands, since C/D/Ts share `Analysis`; the C/D/Ts freeze must name the build.
