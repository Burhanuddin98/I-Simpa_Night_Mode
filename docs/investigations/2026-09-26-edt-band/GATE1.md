# GATE 1 — the f32 air-rate gap (closed 2026-09-27)

## The finding

The critic's `critic:band` (`workflow-reports.json`) found that SPPS computes its per-step air
factor `p = expf(-m c dt)` in f32 (`base_core_configuration.cpp:114-115`), so the energy it
propagates decays at `a_f32 = -ln(p_f32)/dt`, which is what `band_early.py`'s `air_rate` models
(correctly, per its own docstring). The band therefore bounds the continuous-time value of the
energy SPPS *actually propagated at a_f32*, not the value under the physical rate `m c`.

`SPEC.md` P1 and section 1.1 claimed this changes no verdict — checked only on the two z3grid
cases in `eval_diag_z3cd.py` and wrongly generalised. The full 1,010-row sweep (`eval_diag_p32.py`)
found 46 misses (23 in `b1`, 23 in `b0`) among C50/C80/D50 at the 0.1-0.2 ms grid steps, by up to
0.0015 L against a truth built on `m c`.

## The fix

Folded into `band_early.py` as two additions (not a scalar hack — both take and return per-bin
arrays):

- `air_rate_mismatch_bound(dt)`: the a priori worst case `|a_f32 - m c| <= Delta(dt)` from the f32
  rounding of `p` alone (derivation in `SPEC.md` section 1.1), for a caller with only `dt`.
- `widen_eps_for_air_rate(rel_eps, n_bins, dt, air_rate_true_diff)`: compounds the rate error onto
  any existing per-bin recording tolerance, `eps_n_fix = (1 + eps_n) * exp(|a_f32 - m c| (n+1) dt) - 1`.
  `spps_rel_eps(...)` now takes an optional `air_rate_true_diff` (+ `dt`) that calls this
  internally, so the fix is available from the one function callers already use for P1's eps.

Derivation (full text in `SPEC.md` 1.1): `p_f32` is formed from the exact `x = -m c dt` through at
most 4 float32 roundings, so `p_f32 = exp(x)(1+delta)`, `|delta| <= (1+u32)^4 - 1`. Then
`a_f32 = m c - ln(1+delta)/dt`, so `|a_f32 - m c| <= Delta(dt) := (delta/(1-delta))/dt` — a bound
in `dt` alone, growing as `dt -> 0` (fewer bits of the exponent survive `expf`'s own rounding),
which is exactly why the misses concentrate at 0.1-0.2 ms. Energy read in bin `n` arrived at some
`tau <= (n+1) dt` since emission, so the true-rate value differs from the a_f32-rate value by at
most `exp(|a_f32 - m c| (n+1) dt)` at every admissible `tau`; compounding that factor onto
`(1 +- eps_n)` keeps one interval consistent with both rates. Widening `eps` only enlarges the
admissible set (SPEC section 2), so the fix can only add truths a band holds, never drop one.

`SPEC.md` 1.1 and point 4 of the five-line summary are corrected to state the true scope of the
"changes no verdict" claim and point at this file.

## Re-run: all 1,010 z3grid rows, both variants (`gate1_check.py`, `gate1_check.json`)

Truth: the judge's Definition-A truth built on the physical rate `m c` (`ES.truth_point(s, q, 'a')`
— confirmed to equal `S_after` computed with the true `m`, not `a_f32`).

| quantity | variant | checked | outside (old) | outside (fixed) |
|---|---|---|---|---|
| C50 | b1 | 1010 | 10 | **0** |
| C50 | b0 | 1010 | 10 | **0** |
| C80 | b1 | 1010 | 3  | **0** |
| C80 | b0 | 1010 | 3  | **0** |
| D50 | b1 | 1010 | 10 | **0** |
| D50 | b0 | 1010 | 10 | **0** |
| EDT | b1 | 935  | 0  | 0 (unchanged) |
| EDT | b0 | 926  | 0  | 0 (unchanged) |
| Ts  | b1 | 1010 | 0  | 0 (unchanged) |
| Ts  | b0 | 1010 | 0  | 0 (unchanged) |

Old-outside totals: C50 20 + C80 6 + D50 20 = 46 across both variants (23 per variant), matching
the critic's count exactly. Fixed: **0/1010 outside in every quantity, both variants.**
0 exceptions, 0 load errors across the sweep (`gate1_check.json["errors"]` is empty).

EDT and Ts containment is **unchanged**: it was already 100% before the fix and stays 100% after
— the fix widens their bands too (it is applied to the same `eps`), but no band that held its
truth stopped holding it, and none needed to change to hold it.

### Width cost (in L; L: EDT 0.5% relative, Ts 1 ms, C 0.1 dB, D 0.005)

| quantity | variant | width ratio (fix/old) median | max | half-width delta, L median | L max |
|---|---|---|---|---|---|
| C50 | b1 | 1.00087 | 3.684 | 0.00043 | 0.00688 |
| C80 | b1 | 1.00098 | 5.040 | 0.00056 | 0.00778 |
| D50 | b1 | 1.00087 | 3.684 | 0.00006 | 0.00673 |
| C50 | b0 | 1.00086 | 3.684 | 0.00043 | 0.00688 |
| C80 | b0 | 1.00098 | 5.039 | 0.00056 | 0.00778 |
| D50 | b0 | 1.00086 | 3.684 | 0.00006 | 0.00673 |
| EDT | b1 | 1.00017 | 2.797 | 0.00137 | 2.914 |
| EDT | b0 | 1.00015 | 1.959 | 0.00140 | 2.546 |
| Ts  | b1 | 1.00031 | 1.179 | 0.00006 | 0.0119 |
| Ts  | b0 | 1.00017 | 1.179 | 0.00006 | 0.0119 |

The typical cost is negligible (median ratio ~1.0009, i.e. ~0.09% wider). The tail is not: C80's
band can widen 5x and EDT's half-width can grow by up to ~2.9 L (~1.5% relative) on some series —
both are rows that were already close to a refusal boundary, where the exponential-in-`(n+1)dt`
term compounds over many bins. This is the price of admitting the physical-rate truth; it is not
tuned down further, per the hard rule against loosening anything beyond what the fix requires.

## Tests (`test_band_early.py`, T1-T7, `--workers 2`)

`TOTAL: 0 failures`. T1-T7 exercise the admissible-set machinery (Lemma W, EDT witnesses, Ts
Dinkelbach certificates, premises, edge cases) unchanged by this fix — the new code is opt-in
(`air_rate_true_diff` / `widen_eps_for_air_rate`), so this confirms the fix did not disturb the
existing certified machinery. Full log: `gate1_test_stdout.txt`; prior results copied to
`gate1_prev/` before the re-run.

## Files

- `band_early.py`: `air_rate_mismatch_bound`, `widen_eps_for_air_rate`, `spps_rel_eps(...,
  air_rate_true_diff=)`.
- `SPEC.md`: 1.1 (derivation + correction), five-line summary point 4 (correction).
- `gate1_check.py` / `gate1_check.json` / `gate1_check_log.txt`: the 1,010-row re-run.
- `gate1_test_stdout.txt`: T1-T7 re-run.
- `gate1_prev/`: pre-fix `results_t*.json` and the prior test log, kept per the no-delete rule.

## What went wrong

Nothing failed. One correction to the plan: the containment check needed the histograms
regenerated from the judge's `Echo` model (the same loader `eval_run.z3_job` and `eval_diag_p32.py`
use), since `eval_z3.json` stores only the already-computed bands, not the raw per-row energy
series; `gate1_check.py` rebuilds them the same way rather than re-implementing the model.
