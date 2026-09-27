# GATE 4 golden parity corpus

Frozen inputs and Python outputs the future Rust port of `band_early.py` must reproduce.

## Format

One JSON-lines file per group (`<group>.jsonl`), one JSON object per case. Every float array
(the histogram `B`, `total_energy`) is stored exactly via `{"dtype": ..., "shape": ..., "b64": ...}`
-- base64 of the raw bytes at the array's own dtype (float64 for every histogram here; T1's f32
precision variant is stored as the float64 result of `astype(np.float32).astype(np.float64)`,
i.e. the already-rounded value SPPS itself would carry forward, not a re-encoded f32 buffer).
Scalars (`dt`, `t_arr`, `half_width`, `air_rate`, `rel_eps`, `tail_max`, `tail_t_max`,
`t_refl_min`, `kappa`, `n_sources`) are plain JSON numbers/`null`, exact in the JSON text since
Python's `json` module round-trips a float64 exactly (`repr`-precision).

Each case:
```
{"group": ..., "id": ..., "inputs": {...}, "setup": {...notes...}, "truth": {...or null},
 "outputs": {"edt": {...}, "ts": {...}, "c50": {...}, "c80": {...}, "d50": {...}}}
```

Each quantity's output block: `lo`, `hi`, `value`, `refused` (the refusal code or `null`),
and, for `edt` only, `witness` (`[lo, hi]` or `null`), `n_eval`, `budget_exhausted`, `U`
(`[U_min, U_max]` or `[null, null]`), `n0` (the initial interval count, `n_initial` in
`band_early.py`).

`MANIFEST.json`: `{"files": {"<name>.jsonl": {"cases": n, "sha256": ..., "bytes": ...}}}`.

## Loading in Rust

Read each `.jsonl` file line by line, `serde_json::from_str` per line. Decode `b64` with any
standard base64 crate, reinterpret the bytes as `&[f64]` (all arrays here are `dtype: "float64"`)
-- on a little-endian target (x86_64, aarch64) this is a direct `bytemuck`/`transmute` of the byte
slice; on a big-endian target byte-swap first. Feed `inputs` to the port's constructor the same way
`check_golden.py` feeds them to `band_early.Setup(...)`, call the equivalent of `all_bands()`
(or, for `not_decaying`, `edt_band()` directly; for `run_too_short`, `production_inputs()`), and
compare against `outputs`.

## Exact-match fields

`lo`, `hi` (both finite, or the exact `null`/`inf` pairing already recorded), `refused`, `n_eval`,
`budget_exhausted`, `n0`. These follow deterministically from the frozen input array under
IEEE-754 double arithmetic with no free choice in the algorithm's control flow up to the point the
comparison in `edt_band`'s split loop is evaluated in the same order -- `check_golden.py` reproduces
them bit-for-bit against the frozen record on this machine (see `GATE4.md`'s recorded run for the
count). A correct Rust port compiled to use `f64` throughout must match these exactly, modulo the
usual caveat that a different FMA (fused multiply-add) contraction setting on the Rust side can
change the last bit of a chained multiply-add -- if that happens, the mismatch will be a 1-ULP
difference in `lo`/`hi` only, never in `refused`/`n_eval`/`n0` (integers and enum-like strings, not
floats), and never larger than 1 ULP; anything wider than 1 ULP is a real port bug.

## Tolerance fields (and why)

`value`, `witness`, `U`: `edt_band`'s adaptive bisection (`gap_rel`, `max_split`, `stall` in
`band_early.py`'s `Setup.edt_band`) stops refining a sub-interval once its own certified gap is
below `gap_rel`, or `stall` iterations in a row bought less than 10% further tightening. Both stop
conditions compare floating-point quantities against a threshold; a platform or compiler that
contracts a multiply-add differently can flip a comparison that sits within a few ULP of the
threshold, so the bisection can take one split more or fewer than this Python run took. The
returned bound never leaves the true `[lo, hi]` interval either way (that soundness argument does
not depend on when the loop stops -- SPEC.md section 7), but the specific `value`
(`2*lo*hi/(lo+hi)`, the point whose worst relative error to the band is smallest) and which
sub-interval ended up the tightest witness can differ in their last few digits. `check_golden.py`
uses `VALUE_REL_TOL = 1e-9`, an order tighter than the default `gap_rel` the bisection itself
targets, so it still catches a broken port (wrong formula, wrong quantity) while tolerating a
one-split difference in where bisection stopped.

## Coverage and the scope actually frozen (be honest about the gap)

Built by `build_golden.py`, single Python process, no solver run:

- **`t1.jsonl`**: the full T1 closed-form exponential-decay sweep from `test_band_early.py`
  (5 steps x 4 air conditions x 2 direct-sound styles x 2 arrival gaps x 2 precisions), each case's
  histogram rebuilt with the exact same deterministic generator (`expo_bins`/`ball_direct_atoms`)
  the test itself uses -- bit-identical to what `results_t1.json` was scored against.
- **`edge_cases.jsonl`**: one case per named edge in the brief that has a cheap, exact
  construction -- `not_decaying` (the archived adversary histogram `round1/attack1_edge.json:E1`
  that raised `ZeroDivisionError` before round 2's fix), `premise_unsupported` (two sources, via
  `test_band_early.random_series(1234)`), `empty_arrival_bin`, `direct_sound_only`,
  `truncated_series` (a large explicit `tail_max`), and `run_too_short` (the new GATE 2 refusal:
  an inflated tail bound forced past 1% of a deliberately tight `quantity_tolerance`).

**Not included in this pass**, for lack of time inside one night's session (documented here rather
than silently dropped, per the brief's own honesty requirement):
- The 217 Z3 robust cases and the 1,010 z3grid rows. Both are fully reproducible without a solver
  (`eval_run.z3_tasks()`/`z3_job` rebuilds each histogram from the judge's synthetic `Echo` model,
  confirmed by hand: one geometry group runs in about 3.2 s, 202 groups singly threaded is roughly
  11 minutes). They were deferred only because generating them shares the same `band_early.py`
  process budget as the GATE-2-safety-factor T1-T7 re-run this session already had running, and the
  hard rule caps this session at 2 Python processes -- running them concurrently was not safe to
  do. **To extend:** call `eval_run.z3_tasks()` then `job(task)` for each of the 202 groups,
  capture `ec.run(dt, 'step', delay_steps=dl)` (the same `v` the harness itself computes, immediately
  before it is thrown away) alongside the existing `g` dict and `EC.all_methods` output, and stratify
  the z3grid's 1,010 rows (5 steps x 202 groups) by `step_ms` and the truth EDT range if the full set
  is judged too large.
- Stratified samples (~200 each) of ISM, synth, pos200 and the real rows. `eval_ism.json` etc.
  already carry every derived output per row, but not the raw recorded histogram `B` that produced
  them -- regenerating it bit-for-bit needs the same generators `eval_run.py` uses for each set
  (image-source analytic sums for ISM/synth/pos200, on-disk measured data for real), which was not
  reached this session. **To extend:** thread the raw `v`/`B` array out of `eval_common.band()`
  (currently discarded after use, `eval_common.py:65-79`) for a fixed, hashed row-index sample per
  set, stratified by `step_ms` and the truth EDT range bucket (`eval_summary.rng_label`), including
  every refusal code seen in `eval_refusals.json` for that set.

**GATE 4 status given this:** the frozen corpus here is genuine and bit-exact-checked (see
`GATE4.md`), but it does not yet meet the brief's full coverage list. Treat this as GATE 4 **open**,
not closed, until the Z3/z3grid and ISM/synth/pos200/real groups above are added.

## Size

`t1.jsonl` and `edge_cases.jsonl` together are well under 1 MB (see `MANIFEST.json` for exact
bytes) -- histograms here run from tens to a few thousand bins at float64, nowhere near the
z3grid/real scale that would need stratification to stay under the 50 MB target. When the deferred
groups are added, stratify z3grid (1,010 rows -> a few hundred, spread across the 5 `Z3_GRID` steps
and the truth EDT range buckets) and the four ~200-each sets (already at the target count) before
storing raw histograms, since the real-set histograms alone (`eval_real.json` is 49 MB of *derived
output only*) would blow the budget if stored raw and unstratified.
