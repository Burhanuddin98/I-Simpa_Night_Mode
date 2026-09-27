# Inventory: the EDT/Ts/C/D band, honestly (2026-09-27)

Scope: audit only. Nothing in `edt-band/` was changed. No solver was run. Everything below is
sourced from `target/agents/edt-band/{band_early.py,SPEC.md,test_band_early.py,GATE1-4.md,EVAL.md,
eval_*.txt/json}`, `docs/decision-log.md`, and, for "how upstream resolves this," a direct read of
`B:/repos/I-Simpa-upstream/src/isimpa/data_manager/projet_calculation.cpp` (read-only, not edited).
One new fact below (the Ts tail-widening numbers) was produced by running a 12-line read-only probe
script against the already-committed `gate4b_real_check.json` — no solver, no new eval, single
Python process, counted before and after (0 running both times).

**Bottom line up front, since Burhan asked for it plainly.** The complaint is fair on the evidence.
Four successive "gates" (GATE1, GATE2, GATE4, GATE4b) were each opened because the previous round's
rigor had a hole, found by an adversary or a critic, and each was closed by adding more machinery
(an f32 rounding correction, a diffuse-field tail model, a x1000 safety factor, a second twin
solve per quantity to attribute a refusal reason) rather than by simplifying anything. Across that
whole arc, the EDT machinery itself (`Setup.edt_band` and its helpers) is ~505 of `band_early.py`'s
1,369 lines — 37% of the file — implementing a certified bisection over relaxations, worst-case
window geometry, and a work budget, to defend against adversarial constructions that do not occur
in any of the measured real data. On the one dataset built specifically to look like production
(10 real SPPS runs, 3,600 rows, `gate4b_real_check.json`), the measured result is: **EDT is refused
on 3,600 of 3,600 rows, and Ts is refused on 3,600 of 3,600 rows too** — at the 2 ms coarse step
these datasets were checked at, neither method ever hands a user a value. The headline "0 wrong"
number is real and worth keeping (see part (a)), but it is a containment property of an interval
that, on real data, is essentially always too wide to report. And one number in the record
(GATE4.md's "Ts tail-widening 0.0/0.0") is not a measurement at all — see (d).

---

## 1. Machinery inventory: `band_early.py` (1,369 lines total)

Column "issue" uses the task's own I1–I7 labels (fit origin, direct sound, discretisation, tail,
noise, degenerate cases, display). "Measured effect" is the actual, cited number from an eval or
gate re-run; "unmeasured" means no eval file exercises it, even once, on real or synthetic data.

| Component | Lines | Issue(s) | Measured effect |
|---|---|---|---|
| Module header, `LIMIT`/`JND`/`TS_RELATIVE` constants | 1-42 | I7 (the units a range is reported in) | N/A — display convention, not a computation |
| `air_rate_mismatch_bound`, `widen_eps_for_air_rate` (GATE 1) | 52-90 (~37) | I3 (f32 rounding of SPPS's per-step air factor `p`, inside a bin) | Fixes 46 of 1,010 z3grid rows (23 each of `b1`/`b0`) in C50/C80/D50 at 0.1-0.2 ms steps, by up to 0.0015 L, against a truth built on the physical rate `m·c`. **Zero effect on EDT/Ts** (already 100% contained before and after). Typical cost: median band widens ~0.09%; worst case C80 half-width x5.04, EDT half-width +2.9 L. **Never exercised on a single real production row** — see part (b): every real-data harness (`eval_run.py`'s `ism_job`/`synth_job`/`real_job`, and both `gate2_real_check.py`/`gate4b_real_check.py`) passes `air_rate=0.0` / `a=0.0`, so this whole correction only ever ran against Z3's synthetic judge-generated cases. |
| `spps_rel_eps` (P1: the recording-tolerance bound) | 93-137 (~45) | I3, I5 (bounds SPPS's own double/f32 rounding of a bin's recorded value; not statistical Monte-Carlo noise, which SPEC section 8 treats separately) | Foundational, not separately measured: this is the per-bin `eps_n` that every containment number in EVAL.md already assumes. Its own cost is small by construction (SPEC 9's tests show it changes no verdict on its own). |
| `safe_contributions_per_bin` (GATE 2, P1's `m_n` substitute) | 174-188 (~15) | I3, I5 (a model-free upper bound on contributions per bin, since SPPS never reports the true count) | Unmeasured as an isolated delta: it feeds `spps_rel_eps`, and GATE2.md states only that it "only widens, never narrows" — no quantified before/after width is reported anywhere. |
| `production_tail_max` (GATE 2, P4's `tail_max`) | 191-260 (~70) | I4 (energy alive when the run ends, plus particles killed at the `trans_epsilon` floor) | On the 10 real production datasets (3,600 rows), the x1000-inflated bound sits 4-6 orders of magnitude under tolerance (max still-alive fraction 8.36e-5, max kill fraction 1.0e-7) — "changes nothing measurable in this sample" (GATE2.md, its own words). **Not a proof**: GATE2.md states explicitly that no model-free bound is possible here; this is a calibrated, average-case (diffuse-field) model, the same one `docs/results.md`/`tests/params_floor.rs` already ship, inflated x1000 as a hedge. |
| `production_inputs` (GATE 2 entry point) | 263-297 (~35) | wires I3+I4 together | Same evidence as its two callees; itself just plumbing. |
| `min_run_length` (the run-length rule, decision-log row 12) | 300-330 (~30) | I4 (refuse rather than assume a run is long enough) | **Unmeasured end-to-end.** GATE2.md states plainly: "not run against a real room end-to-end... no committed run in this repo's fixtures is short enough to show a refusal." Only exercised by a synthetic curve in a smoke check. |
| `exact_params`, `_curve_at`, `_chord_ratio_extreme` (test/reference machinery) | 336-407 (~72) | I1 (defines the ground truth's own fit origin/window), used to build synthetic truths, not part of the band itself | This is the reference computation every T1-T7 test and z3/z3grid truth is scored against — its correctness is exercised indirectly through every other test in the suite. |
| `Setup.__init__` (the admissible-set construction) | 441-544 (~103) | I1 (t_arr/kappa), I2 (half_width/z0 direct-sound mask), I3 (eps, th0/th_r bounds), I4 (tail_max/tail_t_min/tail_t_max), I6 (misfit_share/misfit_tol, n_sources/homogeneous_celerity premises) | This is where nearly every issue is *modelled*; its correctness is what every other row in this table measures. |
| `S_hi`, `S_hi_open`, `S_lo` (tube bounds, Lemma T/L) | 546-583 (~38) | I3 | Underlies every quantity; T4 (80,000 checks) found 0 outside. |
| `premise_refusal` | 585-595 (~11) | I6 (`premise_unsupported`, `arrival_misfit`) | **Unmeasured on any of the ~94,000 noise-free + real rows evaluated** (`eval_refusals.txt` lists only `not_decaying`, `range_in_arrival`, and the post-hoc `wide>5L`/`given<=NL` buckets across every one of ism/synth/pos200/z3grid/real — neither `premise_unsupported` nor `arrival_misfit` appears once). Exercised only by two synthetic T7 cases (`random_series(1234)` with 2 sources; a fabricated misfit histogram). |
| `cd_band` (Theorem CD, exact closed form) | 600-629 (~30) | I1, I2, I3, I4 | Exact by construction; GATE 1 fixed 46 misses at fine steps (above). 0 wrong across every evaluated noise-free set at the sets/steps GATE 1 actually re-checked (z3grid, 1,010 rows); the wider 67,468-row noise-free claim in EVAL.md predates GATE 1 and was not re-verified against the physical-rate truth outside the z3grid subset (a scope gap, not a known miss — see part (c)). |
| `_ts_step`, `_ts_cert`, `ts_band` (Theorem TS, Dinkelbach) | 634-713 (~80) | I1, I2, I3, I4, I6 (`truncated` refusal) | 0 outside in every noise-free/real containment table. **But on the actual real-production GATE 2/4b check (3,600 rows, the closest thing to a deployment test in this repo), Ts is refused on 3,600 of 3,600 rows** — 2,250 `truncated` (see part (d): this fires whenever `tail_max>0` and no `tail_t_max` was supplied, which is always, in this harness), 1,350 `band_too_wide`. Zero Ts values were ever actually produced in that check. |
| EDT: `_support`, `crossing_range`, `_geometry`, `_tail_best`, `_eval`, `_wit_arr`, `_over`, `_solve`, `_sub_bound`, `edt_band` | 718-1222 (~505) | I1, I2, I3, I4, I6, I7 (witness/half-width reporting) | **The single largest piece of machinery (37% of the file).** This is what delivers the headline "0 wrong EDT" result (0/8,626 wrong even at the tight 1 L tau, across 67,468 noise-free rows) — a real, substantiated advantage over upstream (see part (a)). But acceptance is thin: at the shipped-preset 2 ms, 0% of EDT values in the 0.6-1 s and 0.3-0.6 s ranges are accepted at tau=L; only 7% of ALL noise-free rows get an EDT value at 2 ms and tau=L; **at upstream's own 10 ms default, 0% at any tau**; on the real 3,600-row production check, 0/3,600 accepted. Most of this file's bisection/budget/stall logic (the geometric window cover, `n_split` refinement, `n_double`/`n_bis` inner searches) exists to hold the certified bound tight under adversarial constructions (SPEC 9.3's `attack1_*` scripts); ordinary rows in `bench.py` resolve in 8-40 relaxations, a small fraction of the budget this machinery provides for. |
| `Setup.all_bands`, `_halfwidth`, `_package` (orchestration + GATE 4b priority) | 1225-1343 (~120) | I7 (packaging value/half-width/tau/refused/why), GATE 4b's `run_too_short`-vs-`band_too_wide` priority | On the real 3,600-row check, this relabels **1,176 of 3,600 EDT rows (32.7%)** from a generic "too wide" to the more specific `run_too_short` (the tail estimate is the material cause) — a change in the *reason* shown, not in accept/reject for EDT. For **Ts, this logic never actually engages on real data at all**: see part (d) — `tail_widening` is `None` for all 3,600 Ts rows, so the priority check it exists to perform is never entered. |
| `upstream_edt` / `_upstream_module` (read-only import of the validated upstream port) | 1349-1370 (~22) | comparison baseline, not I1-I7 | Used throughout EVAL.md's "up" column; validated separately against `oracle.json` (upstream's own 2019 output) in `target/agents/upstream-edt/`. |

### `test_band_early.py` (1,436 lines; T1-T7)

Not read line-by-line here (already fully characterised by SPEC.md section 9.1 and the four GATE
reports, cited above): T1 exact-exponential closed form (160 cases), T2 single-impulse edge cases
(32), T3 two-impulse straddle (90), T4 random-series arrangement sweep (400 series x 40 arrangements
= 80,000 checks), T5 adversarial climb to the band's own extremes (40 series), T6 exact band at
zero air (240 series), T7 named edge cases (E1 crash, Lemma W, capped Dinkelbach, per-bin eps with
unrecorded energy, two sources, `U_max > 2 U_min`) plus GATE 4b's four new `run_too_short` unit
checks. All currently pass (`gate4b_test_stdout.txt`, `TOTAL: 0 failures`).

---

## 2. `SPEC.md` inventory (628 lines, 11 sections + a 5-line summary)

| Section | Lines (approx.) | Content |
|---|---|---|
| 0 | 25-48 (~24) | Round-2 finding/fix table (11 adversary findings, all "Correct" or "Partly wrong", plus 2 findings from round 2 itself) |
| 1 | 50-159 (~110) | "What is known, and what is not" — the seven premises (P1-P7), including 1.1 (GATE 1's air-rate correction, added after the fact) and 1.3 (the tail bound's derivation and "what this is not") |
| 2 | 159-233 (~74) | The admissible set and what the band guarantees; GATE 4b's `run_too_short`-vs-`band_too_wide` rule is documented here as a late addition |
| 3 | 233-248 (~15) | C_te/D_te, Theorem CD |
| 4 | 248-411 (~163) | EDT, Theorem E — the largest section, mirroring the largest code block |
| 5 | 411-430 (~19) | Ts, Theorem TS |
| 6 | 430-457 (~27) | Degenerate cases, point value, refusal |
| 7 | 457-483 (~26) | Cost |
| 8 | 483-499 (~16) | Combining with Monte-Carlo noise |
| 9 | 499-585 (~86) | Evidence (the tables cited throughout this inventory) |
| 10 | 585-605 (~20) | What this does not settle |
| 11 | 605-628 (~23) | Output file list |

---

## 3. Direct answers

### (a) What carries a real-world benefit over upstream

- **The band-not-point-value result for EDT itself is real and is the headline Burhan asked for.**
  Upstream never refuses and is frequently badly wrong: on the 26,808-row real set, upstream is
  wrong (outside a truth-derived tolerance) on 11,162 of 26,808 rows at 5 L, median error 4.03 L,
  worst 35.13 L; on the 217 Z3 robust cases, upstream's median error is 44-56 L, worst up to 1,011 L.
  The band variant `b1` is never wrong (0 of 26,808, 0 of 217) when it gives a value at all. This is
  the concrete, measured version of decision-log row 4's order.
- **`spps_rel_eps`/P1 (the recording-tolerance bound)** is a real, cheap fix: it is the mechanism
  that makes "0 wrong" possible at all, by correctly bounding SPPS's actual double-then-f32 rounding
  chain (`sppsTypes.h`, `coreTypes.h`, `reportmanager.cpp`, cited in `SPEC.md` P7) — not the f32
  chain the original adversary wrongly assumed (SPEC 0, row P7).
- **GATE 1's air-rate fix** is a genuine, verified correctness fix (46/1,010 miss rate closed) —
  small in absolute size (up to 0.0015 L) and only relevant at grid steps (0.1-0.2 ms) finer than the
  1 ms default decision-log row 11 actually chose, but it is real, not decorative.
- **`production_tail_max`/GATE 2** is a real improvement over the status quo it replaces: the
  previous default silently assumed the unrecorded tail was exactly 0. A calibrated, tested,
  non-zero bound (even an unprovable one) is strictly more honest than that, per decision-log row 12.

### (b) What exists only to make a theorem hold in cases never seen in measured data

- **Most of the EDT bisection machinery** (`_geometry`, `_solve`'s tangent/bisection search,
  `n_split`/`n_double`/`n_bis`/`stall`, the work budget) is sized for adversarial constructions
  (`attack1_*.py`, Z3-generated near-worst-case curves) that found real gaps (SPEC 0's P1-P6, I1-I5,
  R2-R3). On ordinary rows, `bench.py` shows 8-40 relaxations against a budget that allows up to
  6,000; the difference is entirely insurance against inputs no real receiver histogram in this
  repo's evaluation produces.
- **`premise_refusal`'s two branches never fire on any evaluated data.** `eval_refusals.txt` — the
  refusal-reason breakdown across all of ism (28,890 rows), synth (26,368), pos200 (11,200), z3grid
  (1,010) and real (26,808), every step from 0.1 to 10 ms — lists only `not_decaying`,
  `range_in_arrival`, and the post-hoc `wide`/`given` buckets. **`premise_unsupported` and
  `arrival_misfit` appear zero times in that file.** They exist for a case (multiple sources, a
  celerity gradient, energy where neither the direct window nor a reflection can explain it) that
  the evaluation harness never constructs from real data — only from two synthetic T7 fixtures.
- **The GATE 4b twin-solve (`notail` Setup + `tail_widening`) for Ts specifically never engages on
  real data at all** — see (d): it is there to attribute a refusal to the tail specifically, but on
  every one of the 3,600 real rows checked, Ts either never reaches that comparison (`truncated`,
  refused earlier) or has nothing to compare against (`tail_max` exactly 0). The mechanism is
  correct in principle (it is exercised and passes in the four new synthetic unit tests, GATE4b's
  (a)-(d)) but has zero measured effect on any real row in this repo.
- **`min_run_length`** exists for a refusal (a run that hasn't gone on long enough) that no committed
  fixture in this repo is short enough to trigger — GATE2.md says so itself.

### (c) Decorative numbers

- **The clearest one, found by direct inspection (see (d)): GATE4.md's "Ts tail-widening 0.0 / 0.0
  (median/max)"** is not a measurement of a small effect. It is `float(np.median(w_ts)) if w_ts else
  0.0` (`gate4b_real_check.py:222-223`) applied to an **empty list** — confirmed by loading
  `gate4b_real_check.json` and counting: 0 of 3,600 rows have a non-`None` `ts_tail_widening`. The
  prose around it ("Ts's own tau rule already tracks the band width, so the tail rarely dominates it
  here") is a plausible-sounding explanation for a number that in fact measures nothing.
- **`BIG_TAU = dict(edt=1e9, ts=1e9, c=1e9, d=1e9)` (`eval_common.py:48`)** is used, unconditionally,
  by `band()` (`eval_common.py:76`), which is what produced every containment row in EVAL.md and
  `eval_tables.txt` (the huge ism/synth/pos200/z3grid/real sweep). This is a legitimate way to
  measure pure containment (does the band ever exclude the truth) separately from acceptance (would
  a real tau have refused it) — and the "Acceptance" table in EVAL.md does correctly reapply the real
  tau afterward — but a reader who only sees "real all: b1 1636/26808 ... 0 wrong" without also
  reading the acceptance table could easily mistake a `tau=1e9` containment count for a production
  acceptance count. `gate2_real_check.py` uses the identical `tau=1e9` device for the same reason,
  and GATE4b's own text has to explicitly walk this back ("not a fair apples-to-apples comparison").
- **The 26,808-row noise-free "0 wrong" claim for C50/C80/D50 is broader than what GATE 1 actually
  re-verified.** GATE 1 re-checked the physical-rate (`m·c`) truth only on the 1,010-row z3grid
  subset. The fix is applied uniformly in code, so it should also cover ISM/synth/pos200's ~66,000
  remaining rows, but GATE1.md does not report having re-run the physical-rate truth check against
  them — the "0 wrong" figure for that larger set is still only checked against the a_f32-rate truth
  (SPEC point 4's own scoping: "against a truth built on SPPS's own air rate a_f32"). Not a known
  miss; a scope gap in what was actually re-verified after GATE 1.

### (d) Why the Ts tail widening is exactly 0 on all real rows in `gate4b_real_check.json`: **an evaluation-harness gap that makes the number not a measurement at all, not a genuine "the effect is tiny" finding.** Read the code path directly (all receipts below, verified by loading the committed JSON, not re-running anything):

1. `gate4b_real_check.py:119` calls `band_check(v, dt, t_arr, h, eps, tail_max, None, trm)` —
   **`tail_t_max` is always `None`** for every one of the 3,600 rows. This means `Setup.tv1` (the
   latest time unrecorded energy could arrive) is always `float('inf')` (`band_early.py:488`).
2. `Setup.ts_band()` (`band_early.py:686-690`) checks, before anything else:
   `if self.Tmax > 0 and not math.isfinite(self.tv1): refused='truncated'; return None, info` — "Ts
   has no upper bound." Since `tv1` is always infinite in this harness, **any row with a nonzero
   `tail_max` refuses Ts outright, before a Ts band is ever computed.** Direct count from
   `gate4b_real_check.json`: 2,250 of 3,600 rows have `ts_refused == 'truncated'`.
3. On the other 1,350 rows, `tail_max` came out to exactly `0.0` (both `f_end` and `kill_frac` were
   `0.0` in `production_tail_max` for those rows — confirmed by inspection, e.g. row
   `['C-R3', 0, 'R000', 125]`: `tail_max=0.0, f_end=0.0, kill_frac=0.0`). With `Tmax==0`,
   `Setup.all_bands()`'s own gate (`band_early.py:1244`, `if self.Tmax > 0: notail = Setup(...)`)
   never builds the comparison Setup — there is no tail to widen against, so `tail_widening` stays
   `None` there too, and `ts_refused` is `band_too_wide` for all 1,350.
4. **Net result, verified by direct query of the committed file: 0 of 3,600 rows have a non-`None`
   `ts_tail_widening`.** The reported "0.0/0.0" is `widening_in_tau()`'s empty-list default
   (`gate4b_real_check.py:141-148`, `main()`'s `float(np.median(w_ts)) if w_ts else 0.0`), not a
   small measured value.
5. This is a **harness gap, not a `band_early.py` bug**: `ts_band`'s "no upper bound, refuse" rule is
   a legitimate, documented consequence of Ts needing a finite end to integrate against (SPEC
   section 5) — the actual gap is that no loader in this repo (`bracket/common.py`, the
   `load_report`/`load_solve` paths cited in GATE2.md) computes or passes a real `tail_t_max` (an
   upper bound on when unrecorded energy could still arrive) for production data. Until one does, Ts
   is refused on effectively every real production row with any nonzero tail estimate — which,
   per part (a) above, is nearly all of them (SPPS runs essentially never end with `f_end` and
   `kill_frac` both exactly zero). **On this evaluation, Ts's real-world acceptance rate is 0/3,600,
   for the same underlying reason EDT's is low: this specific harness never supplies what Ts's own
   preconditions ask for.**

### (e) What "truth" means for the real rows, and how it was produced

**There is no independent ground truth for the real datasets. "Truth" is the output of the same
simple point-estimate method being evaluated (the "shipped" `decay.rs`-mirroring calculator),
computed at the finest step the real dataset was recorded at, then used as the reference for
scoring every method (band, upstream, shipped, W1G) at coarser rebinned steps of the same series.**

Receipt: `target/agents/followup-design/spec/eval_real.py`, whose own docstring says exactly this —
"Truth: the same series' shipped midpoint at its fine step, where settled (the design's truth)"
(line 3) — and whose code (`work()`, lines 20-42) computes `sv = C.shipped(v, dtf, t, h)` (the
shipped EDT/Ts calculator) on the full-resolution recorded series `v` at its native step `dtf`, and
stores `truth = dict(edt=e_f['mid'], ts=s_f['mid'])` from that call's own midpoint. This
`real_rows.pkl` file is what `gate2_real_check.py`, `gate4b_real_check.py`, and `eval_run.py`'s
`real_job`/`real_tasks` all read as `reg`/`rows_by_key[...]['truth']`.

**What this means, plainly:**
- It is a reasonable proxy — a finer-resolution estimate from the same family of methods is likely
  closer to reality than a coarser one — but it is not a physical ground truth, and it is not
  independent of the methods it scores. A method's own agreement with the shipped calculator at fine
  resolution is, by this construction, mechanically rewarded.
- It also means the real-set containment numbers (EVAL.md's "real" rows, GATE2/4b's `edt_inside`/
  `ts_inside`) are validating self-consistency across rebin resolution, not validating against a
  physically measured or analytically known decay curve — unlike the Z3/z3grid/ISM/synth sets, whose
  truth comes from the judge's own synthetic `Echo` model or an image-source analytic sum
  (`eval_common.truth_from_split`, `AI.truth_ideal`), which *is* independent of every method scored.
- Confirmed separately (part (b)/(a) above): the real-data harnesses additionally use an
  **exact, oracle-derived `tail_max`** — the true rebin remainder from the fully recorded fine
  series (`eval_run.py:339-346`, `rest = v_fine[n*k:].sum()`) — not `production_inputs`'s calibrated
  model. GATE2.md itself flags this: "every eval script fed the generator's exact dropped tail...
  rather than [testing] anything else." The real-set containment numbers are therefore optimistic
  relative to genuine production use in a second, independent way: they get to see the exact
  remainder that a live deployment could not know.
- Also confirmed while tracing this: **every real-data code path (`ism_job`, `synth_job`, `real_job`
  in `eval_run.py`, and both `gate2_real_check.py`/`gate4b_real_check.py`) passes `air_rate=0.0`.**
  For `real_job` this is a deliberate substitution (the fine-step recording already bakes the air
  factor in, so the residual within-coarse-bin air uncertainty is folded into a wider `eps` instead,
  per the comment at `eval_run.py:314-317`), not an oversight — but the practical consequence, stated
  plainly, is that **GATE 1's air-rate/`a_f32` correction (item 1 of this inventory's machinery
  table) has never been exercised against a single real SPPS production histogram in this repo** —
  only against Z3's synthetic, judge-generated cases.

---

## 4. Data and harness available for evaluation

All paths relative to `B:/repos/I-Simpa_Night_Mode/target/agents/` unless noted.

### Core library
- `edt-band/band_early.py` — the reference implementation. Entry points: `Setup(B, dt, t_arr,
  half_width=, air_rate=, kappa=, rel_eps=, tail_max=, tail_t_max=, t_refl_min=, misfit_tol=,
  n_sources=, homogeneous_celerity=)`, then `.edt_band()`, `.ts_band()`, `.cd_band(te)`, or
  `.all_bands(tau=None, upstream=True)` for all five quantities at once. Production-input helpers:
  `production_inputs(B, dt, onset_step, total_energy, particles_per_source, n_sources=, trans_epsilon=,
  computation_method=, killed_particles=, ...) -> (tail_max, eps, detail)`; `production_tail_max(...)`;
  `safe_contributions_per_bin(particles_per_source, n_sources=1)`; `min_run_length(...)`.
  GATE 1 helpers: `air_rate_mismatch_bound(dt)`, `widen_eps_for_air_rate(rel_eps, n_bins, dt,
  air_rate_true_diff)`. Reference/test truth: `exact_params(pos, mass, te_list=...)`. Upstream
  comparison: `upstream_edt(B, dt, kind='point')` (imports the validated port below, read-only).

### Upstream port (validated I-Simpa-compatible reference)
- `upstream-edt/uphunt_upstream_edt.py`: `edt(series, dt, kind='point', tt=None)` (f32, matches
  upstream's GUI exactly per `oracle.json`), `edt_f64(series, dt, kind='point')`, `schroeder(row,
  conv=to_db_p0)`, `point_time_table`/`surface_time_table`, `get_time_range`, `get_sum_limit`,
  `regression`, `tr_param`, `compute_tr_param_table`, `read_gabe(path)` (reads upstream's own `.gabe`
  output format for cross-checking), `time_table_from_labels`.
- Validated against `upstream-edt/oracle.json` (upstream's actual 2019 GUI output) and cross-checked
  directly against `B:/repos/I-Simpa-upstream/src/isimpa/data_manager/projet_calculation.cpp`
  (`MakeSchroederArray`, `GetTimeRange`, `ComputeLinearRegression`, `Compute_TR_Param`, lines 68-219;
  `Compute_TR_Param(0, 10, ...)` at line 936 is the EDT call). This inventory read that file directly
  (read-only) to confirm decision-log row 4's claim in the actual source: EDT's Schroeder-integral
  window always starts at `timeTable[0]` (emission, bin 0 — `GetTimeRange`'s `fromdB=0` makes the
  very first sample satisfy `abs(row_db[0]-row_db[0])=0>=0` immediately), there is no direct-arrival
  correction (I1 unaddressed), no per-bin uncertainty (I3 unaddressed — a bin's value is treated as
  an exact point on the curve), no tail/kill bound (I4 unaddressed), and no refusal path at all
  (I6 unaddressed: `ComputeLinearRegression` will silently divide by whatever `n*Sx2-Sx*Sx` comes
  out to, even from degenerate input) — it always returns exactly one number (I7: no range, ever).
  Upstream's own worst measured errors on this repo's data (part (a)) are the direct, measured
  consequence of this simplicity.

### Evaluation harness (`edt-band/`)
- `eval_common.py` — shared methods and truths, all read-only: `band(v, dt, g, trm)` (band_early via
  `BIG_TAU`, see part (c)); `upstream(v, dt)`; `shipped(v, dt, t, h)` (the `decay.rs`-mirroring
  point-estimate check currently shipped); `w1g(v, dt, t, h, d1, c, clear, R, m)` (the retracted W1G
  method, kept for comparison); `all_methods(v, dt, g, variants=('b1','b0'))` runs all four on one
  series; `cd_truth(direct, refl, t, dtf, te)` and `truth_from_split(direct, refl, t, dtf, AI)` build
  an independent analytic truth for the synthetic sets (NOT used for the real set — see part (e)).
- `eval_run.py` — per-set task generators and job functions, each independently callable:
  `ism_tasks(limit=None)`/`ism_job(args)`, `synth_tasks`/`synth_job`, `pos200_tasks`, `real_tasks`/
  `real_job`, `z3_tasks()`/`z3_job(args)` (rebuilds each histogram from the Z3 judge's synthetic
  `Echo` model — reproducible without a solver). `main()` drives all of them; `rebin_with_rest(v, k)`
  is the generic coarse-rebin-with-exact-remainder helper reused by several jobs.
- `eval_summary.py` → `eval_summary.txt`/`eval_results.json`; `eval_tables.py` → `eval_tables.txt`/
  `.json` (the "same rows, every method" A/B tables); `eval_step_needed.py` → coarsest step for
  50%/90% acceptance; `eval_refusals.py` → per-set/per-range refusal-reason breakdown (the file used
  for part (b)'s `premise_unsupported`/`arrival_misfit` absence check); `eval_diag_z3cd.py` /
  `eval_diag_p32.py` → the f32-vs-physical-rate diagnostic that led to GATE 1; `eval_check_outputs.py`
  → crash-recovery consistency check; `eval_upstream_port.py` → validates the C/D/Ts upstream mirror
  against `oracle.json`.
- `gate1_check.py`, `gate2_real_check.py`, `gate4b_real_check.py`, `critic_check.py` — the four
  targeted re-runs behind GATE1-4b's own numbers (all read real `report.json` production files from
  `pm8-noise-scratch/runs/noise-cal-*/`, all single-process, all produce a `.json` beside themselves).

### Golden/parity corpus (`edt-band/golden/`)
- `build_golden.py` — builds `t1.jsonl` (160 T1 cases) and `edge_cases.jsonl` (7 named edge cases,
  including `run_too_short`) as base64-encoded exact arrays, for a future Rust port to replay.
- `check_golden.py` — replays every case against `band_early.py`, bit-exact on `lo`/`hi`/`refused`/
  `n_eval`/`budget_exhausted`/`n0`, tolerance-checked (`VALUE_REL_TOL=1e-9`) on `value`/`witness`/`U`.
  Currently: 167 cases, 0 mismatches.
- `MANIFEST.json` — per-file case count, byte size, sha256, for integrity checking.
- **Explicitly not yet covered** (`golden/README.md`'s own list, still true after this session): the
  217 Z3 robust cases and 1,010 z3grid rows (reproducible via `eval_run.z3_tasks()`/`z3_job`, no
  solver needed); stratified ISM/synth/pos200/real samples (the raw histogram is discarded in
  `eval_common.band()` today — would need threading out).

### Real production fixtures actually available
- `pm8-noise-scratch/runs/noise-cal-1790307822/` and `.../noise-cal-1790310131/`: ten real SPPS
  `report.json` production runs (`C-E3,C-E4,C-E6,V-E2,V-E5` energetic; `C-R3,C-R4,C-R6,V-R2,V-R6`
  random), each with real `particles`, `total_energy`, `trans_epsilon` — the only fixtures in this
  repo that exercise `production_inputs` end to end.
- `followup-design/spec/real_rows.pkl` — 16 real fine-step datasets' `truth`/`res` rows (see part (e)
  for what "truth" means here), built by `followup-design/spec/eval_real.py`.
- Two of sixteen real-data loaders (`POS200`, `A02*`, `B02` families) have **no** particle-statistics/
  `total_energy` table wired up in `bracket/common.py::load_solve` — `production_inputs` cannot be
  built for them without extending that loader first (GATE2.md's own finding, unchanged here).

---

## 5. What this means for a simplification pass

Not attempted here (out of this task's scope — this is the inventory a later step should build on),
but stated plainly since it follows directly from the numbers above:

- The part of this design with an actual, large, measured advantage over upstream is narrow: giving
  EDT as a certified interval instead of upstream's single always-wrong-prone number. That is real
  and worth keeping in some form.
- The part that has ballooned — four gates deep, ~500 of 1,369 lines for EDT alone, a twin-solve per
  quantity to label a refusal reason — is defending a worst-case guarantee against adversarial inputs
  that do not occur in any real histogram this repo has, while on the one dataset built to look like
  production, both EDT and Ts are refused on every single row.
- The most actionable single fact for a simplification pass: **Ts's `truncated` refusal, on real
  data, is driven entirely by the harness never supplying `tail_t_max`, not by any real ambiguity in
  the data** — supplying a real upper time bound (or accepting a wider, always-finite one) would
  likely restore most of Ts's real acceptance immediately, for free, with no change to `band_early.py`
  itself. That is the kind of fix decision-log row 15 already called for ("calibrate... before M8b")
  but pointed at the x1000 factor rather than this specific, cheaper gap.
