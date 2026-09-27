# GATE 4b — the run-too-short refusal, fixed (2026-09-27)

## What was frozen going in, what was not

Going into this session GATE 4's golden corpus (`golden/t1.jsonl`, 160 cases; `golden/edge_cases.jsonl`,
6 cases at the time) was already built and bit-exact-checked against `band_early.py` as it stood
then (`gate4_prev/*.pre-gate4`). That freeze predates this fix and is untouched by it except where
the fix changes `band_early.py`'s actual behaviour on one of the frozen cases (it does not -- see
"golden" below). Not frozen, before or after this session: the 217 Z3 robust cases and the 1,010
z3grid rows, and stratified samples of ISM/synth/pos200/real (`golden/README.md`'s own "Not included
in this pass" already says so; this session did not reach them either -- see "golden" below).

## The bug (as verified by the reviewer) and the fix

`production_inputs(..., quantity_tolerance=...)` refused `'run_too_short'` when
`tail_max > 0.01 * quantity_tolerance`. `tail_max` is an absolute energy (units of `sum(B)`);
`quantity_tolerance` is a per-quantity half-width (0.5% relative for EDT, `min(1 ms, 0.5% of Ts)`
for Ts, 0.1 dB for C, 0.005 for D) -- the comparison mixed units and never meant anything.
`gate2_real_check.py` never passed `quantity_tolerance`, so the refusal never actually ran on the
3,600 real rows GATE 2 checked; the only exercise of it was `golden/edge_cases.jsonl`'s
`run_too_short` case, built with a deliberately absurd `quantity_tolerance=1e-9` to force it.

**Fix, in `band_early.py`:**
1. `production_inputs(...)` no longer accepts `quantity_tolerance`; passing it raises `TypeError`
   (message points here). It still returns `(tail_max, eps, detail)` unconditionally --
   `GATE2_TAIL_SAFETY_FACTOR = 1000.0` and `GATE2_TAIL_REFUSAL_TOLERANCE_SHARE = 0.01` are unchanged.
2. `Setup.all_bands()` now solves every quantity's band twice from identical inputs (same `B`,
   `eps`, everything else): once with the caller's `tail_max` and once with an identical Setup
   whose `tail_max = 0` ("notail"). `tail_widening = halfwidth_with_tail - halfwidth_without_tail`,
   in that quantity's own units (relative for EDT, seconds for Ts, dB for C, dimensionless for D --
   the same units `all_bands` already reports and already compares against `tau`). Recorded on
   every quantity's output as `tail_widening`.
3. Where `_package()` used to report `'band_too_wide'` whenever `half_width > tau`, it now checks
   `tail_widening > GATE2_TAIL_REFUSAL_TOLERANCE_SHARE * tau` first: if the tail estimate alone
   already eats more than 1% of tau, the more specific `'run_too_short'` is reported instead of the
   generic `'band_too_wide'` (the tail, not the recorded bins, is the material cause). Each quantity
   is judged independently; a quantity already refused for an unrelated reason (`premise_unsupported`,
   `not_decaying`, `arrival_misfit`, ...) keeps that reason -- the tail check only applies where a
   band was actually computed and would otherwise be reported.
4. Documented in `band_early.py` (the comment block above `GATE2_TAIL_REFUSAL_TOLERANCE_SHARE`, and
   `_package`/`all_bands`'s docstrings) and in `SPEC.md` section 2's "P4 in production" paragraph:
   the 1% share is unchanged from the original (broken) rule -- below it, even a x1000-wrong
   diffuse-field assumption cannot move the reported range by more than a hundredth of the
   quantity's own JND-scale unit; above it, the guarantee rests on that non-model-free assumption,
   so the quantity is outside the method's premises and refused rather than silently widened.

Files changed: `band_early.py` (production_inputs, Setup.all_bands, `_package`, new `_halfwidth`
helper), `SPEC.md` (section 2), `test_band_early.py` (default/max `--workers` 8 -> 2, four new unit
checks), `golden/build_golden.py` and `golden/check_golden.py` (the `run_too_short` edge case
rebuilt), plus two new scripts: `gate4b_real_check.py` (adapted from `gate2_real_check.py`) and this
file. `gate2_real_check.py` itself is untouched (kept as the pre-fix baseline record).
Pre-edit copies of every touched file are in `gate4b_prev/*.pre-gate4b`.

**Which file version the tests below ran on.** The first T1-T7 run (started 02:27:18, workers
imported the module at 02:28:33) tested a version of `band_early.py` from before the 02:32:44 edit
that added the `run_too_short`-vs-`band_too_wide` priority logic in `_package`/`all_bands` -- a real
logic change, not comments. That run was discarded. `test_band_early.py --workers 2` was re-run
in full starting 02:44:16 (after confirming no `python.exe` was still running from the discarded
run), strictly after the last edit to `band_early.py` (02:32:44) and `SPEC.md` (02:35:50); its
output is `gate4b_test_stdout.txt`. All numbers below are from that second, correct run, and from
`golden/build_golden.py`, `golden/check_golden.py` and `gate4b_real_check.py` runs made after it,
against the same final file state (no further edits to `band_early.py` after 02:32:44).

## T1-T7 + new unit tests (`gate4b_test_stdout.txt`, run of 02:44:16-03:01:16, `--workers 2`)

`TOTAL: 0 failures`. Per suite: T1 160/160 rows inside their closed-form band; T2 32/32; T3 90/90;
T4 400 series x 40 arrangements x per-quantity checks = 80,000 checks, 0 outside (735 s, 2 workers);
T5 0/40 adversarial climbs escaped the band; T6 240 series, EDT half-width excess over the exact
band median 0.000 L, max 0.019 L, 0 EDT-band violations, refusals `{'range_in_arrival': 1,
'not_decaying': 3}` (both pre-existing, unrelated to this fix); T7 (6 named checks + the new
`gate4b_refusal` check) 0/7 failed.

**New unit tests** (`t7_gate4b_refusal`, all 4 passed):
- (a) `tail_zero_never_refuses`: a fixed exponential-decay Setup (T1's own generator) with
  `tail_max=0.0` never reports `'run_too_short'` on any of edt/ts/c50/c80/d50. PASS.
- (b) `large_tail_refuses_run_too_short`: `tail_max = 1e-4 * S(onset)` on the same series makes at
  least one quantity refuse `'run_too_short'`. PASS (verified interactively: at this level EDT's
  `tail_widening` is 0.000223 vs. `tau` 0.005 -- 4.5x the 1% share).
- (c) `tiny_tail_does_not_refuse`: `tail_max = 1e-9 * S(onset)` refuses nothing via
  `'run_too_short'`. PASS.
- (d) `quantity_tolerance_raises_typeerror`: `production_inputs(..., quantity_tolerance=1e-9)`
  raises `TypeError`. PASS.

## Real re-run (`gate4b_real_check.json`, from `gate4b_real_check.py`, adapted from
`gate2_real_check.py` -- same 3,600 rows, same 10 load_report datasets, same x1000-inflated
`tail_max` via `production_tail_max`/`production_inputs`, unchanged code path there)

**One material difference from `gate2_real_check.py`, stated plainly.** The original script called
`all_bands(tau=dict(edt=1e9, ts=1e9, c=1e9, d=1e9), ...)` specifically so nothing would ever be
refused, and checked only containment. To exercise the new refusal at all, `gate4b_real_check.py`
uses the real `LIMIT` tau (0.5% EDT, Ts's own rule, 0.1 dB C, 0.005 D). At real coarse-bin (2 ms
rebin) resolution, EDT's band is wider than 0.5% relative on **every one of the 3,600 rows** --
`edt_accepted` (not refused, for any reason) is 0/3600, with or without this fix, because the
generic `'band_too_wide'` or `'run_too_short'` refusal fires either way at this tau. This is a
finding about tau vs. real coarse-bin resolution, not something this fix changed or could change --
it means `old_gate2_edt_accepted` (3,600, from the tau=1e9 baseline) and this run's `edt_accepted`
(0) are not a fair apples-to-apples comparison; `edt_accepted_change = -3600` in the JSON reflects
the tau change, not a regression from this fix. The number that isolates this fix's actual effect is
`edt_run_too_short`: **1,176 of 3,600 rows (32.7%)** have the tail estimate specifically as the
material cause of EDT's refusal (`tail_widening > 1% of tau`); the other 2,424 refused-EDT rows are
refused for reasons the tail did not cause (inherent bin-width vs. tau at this rebin).

**Containment, unaffected by the tau choice** (checked against the band whenever one exists,
regardless of `refused` -- `_package` still returns a sound `[lo, hi]` for a `band_too_wide` or
`run_too_short` quantity, the label only says "wider than this quantity's tau"):
- EDT: 3,600/3,600 checked, **0 outside** (identical to `gate2_real_check.json`'s pre-fix number).
- Ts: 1,350/1,350 checked, **0 outside** (identical; the other 2,250 rows have no Ts truth
  registered at this step, same as before, not a new miss).
- `production_inputs` raised 0 times (same as before).

**Tail-widening distribution** (in units of that quantity's own tau; median / max over all 3,600
rows): EDT 0.0367 / **82.03** (V-R6, `edt_widening_over_tau_max`); Ts 0.0 / 0.0 (Ts's own tau rule
already tracks the band width, so the tail rarely dominates it here). Per-dataset EDT
`run_too_short` counts: C-R3 0/360, C-R4 0/360, C-R6 48/360, V-R2 42/360, V-R6 **360/360**, C-E3
0/360, C-E4 0/360, C-E6 **360/360**, V-E2 6/360, V-E5 **360/360** (full per-dataset table in
`gate4b_real_check.json`'s `summary.per_dataset`). V-R6/C-E6/V-E5 being refused on every row is
itself informative: those datasets' recorded runs are short enough, relative to the receiver's own
decay, that the x1000-inflated tail estimate is not a rounding error against tau -- a real
"this run needed to go longer" signal the old comparison could never raise.

## Golden corpus (`golden/`)

- `t1.jsonl`: **160 cases**, 19,151,779 bytes (unchanged by this fix -- rebuilt and re-hashed
  identically, `sha256 90fa24af6034...`, same as before this session, confirming the fix does not
  perturb T1's closed-form sweep).
- `edge_cases.jsonl`: **7 cases** (was 6): the original `run_too_short|gate2_refusal` case (built on
  the now-removed `quantity_tolerance` path) is replaced by two real-data cases, both from
  `pm8-noise-scratch/runs/noise-cal-1790307822/V-E5/seed01/report.json` (receiver 0, band 0, a
  genuine SPPS production run, not a contrived array):
  - `run_too_short|gate4b_real_truncated`: the fine (1 ms) series truncated to `n_steps=100` of its
    1500 recorded bins. Alive share at truncation (`total_energy[99] / total_energy[onset=4]`) is
    **0.0563** -- inside the 1e-2..1e-1 band the brief asks for. D50 (and C50/C80) are refused
    `'run_too_short'` there (D50's `tail_widening` / tau = 0.4083 / 1% = 82x over).
  - `run_too_short|gate4b_real_full_just_under`: the same receiver/band's full, untruncated 1500-bin
    series (alive share effectively 0; the run's remaining `tail_max` comes from the floor-kill term
    only, since this room used the energetic method at a finite `trans_epsilon`). D50's
    `tail_widening` / tau there is **0.78%**, just under the 1% `GATE2_TAIL_REFUSAL_TOLERANCE_SHARE`
    -- D50 is NOT refused: the same mechanism, same receiver, on the trusted side of the line.
  - `check_golden.py`'s `run_too_short`-prefixed branch was rewritten to match (it used to call
    `production_inputs(..., quantity_tolerance=...)`, which now raises; it now builds the frozen
    `Setup` directly and calls `all_bands()`, checking `lo`/`hi`/`refused` per quantity as the other
    edge cases do -- `rel_eps` for these two cases is a per-bin array, so it goes through the same
    `unarr()` path as `B`, unlike the other cases' scalar `rel_eps`).
- `MANIFEST.json` rebuilt: `t1.jsonl` 160 cases / 19,151,779 bytes / `sha256 90fa24af6034...`;
  `edge_cases.jsonl` 7 cases / 78,800 bytes / `sha256 b5c474a493d2...`.
- `check_golden.py`: **167 cases replayed, 0 mismatches, GOLDEN CHECK: PASS.**
- **Still not frozen** (unchanged from before this session, `golden/README.md`'s own list): the 217
  Z3 robust cases and 1,010 z3grid rows (`eval_run.z3_tasks()`/`z3_job`, reproducible without a
  solver, deferred here for the same reason as before -- generating them is more Python-process time
  than this session's 2-process cap and the T1-T7/real-check re-runs already needed left room for);
  and stratified samples of ISM/synth/pos200/real (raw histograms not yet threaded out of
  `eval_common.band()`). GATE 4 (the original golden-corpus milestone) was already open on this
  account before this session; it remains open on the same account after it -- this session's scope
  was the refusal-formula bug, not closing that gap.

## Problems encountered (receipts)

- A background T1-T7 run was started against a stale copy of `band_early.py` (the priority-logic
  refactor in `_package`/`all_bands` landed at 02:32:44, after the run's workers had already
  imported the module at 02:28:33). Caught before reporting (coordinator flagged it); the run was
  let finish, confirmed no `python.exe` remained, and `test_band_early.py --workers 2` was re-run in
  full against the final file (02:44:16-03:01:16, `gate4b_test_stdout.txt`) -- see "which file
  version" above.
- `gate4b_real_check.py`'s first draft gated containment (`edt_inside`/`ts_inside`) on
  `refused is None`, using the real `LIMIT` tau instead of the original script's `tau=1e9`. Since
  essentially every real row is refused for *something* at real tau (see "one material difference"
  above), that made `edt_checked` 0 across the board -- containment could not be evaluated at all.
  Fixed before the reported run: containment is now checked against the band whenever one exists,
  independent of `refused` (matching what `_package` actually returns -- a valid `[lo, hi]` even
  when the quantity is refused for being too wide by policy).
