# M8b: EDT v2.1 ported to Rust, and shown equal to the Python

2026-10-02. Branch `m8b-edt`. The method is `docs/investigations/2026-09-27-edt-heldout/frozen2/method.py`
(sha256 `029d90ac5e8f6a634a51a7ffce136cbd4c0b7ea3d3ad8a18b78fd8240ff224a0`, Z = 2.5), the one `VERDICT-2.md`
passed for Random-mode runs (H1-H6) and failed for Energetic mode (H3). Nothing under `2026-09-27-edt-heldout/` was edited.

## 1. The map (before any change)

- **Where EDT was computed:** `crates/simpa-core/src/params/decay.rs` (`DecayRange::Edt`, a 0 to -10 dB fit on the
  Schroeder curve with the M7 model-free band, decision-log row 7), wrapped by `params/noise.rs` (`QUANTITIES[1]`: 200
  bootstrap resamples, calibrated `mc_sd`, `noise_uncalibrated` refusals) and surfaced by `results/report.rs`
  (`evaluated()` into `Parameters::edt_s: Evaluated`).
- **What it returned:** `{"value": s, "mc_sd": s}` or a `params_not_evaluable` refusal (`monte_carlo_noise`,
  `noise_uncalibrated`, `truncated`, `early_unresolved`, ...). No range, no status. (Row 9, "show the range always",
  was not implemented by the old code: it had a point value and an `sd`.)
- **`arrival_s`:** `SppsResults::arrival_from` (`results/spps.rs`): per source `emission_s + d / c`, the earliest over
  the sources named; `emission_s` is `spps::emission_s(delay, dt)`, `ceil(delay/dt)` whole steps in `f32`, times `dt`.
  `report.rs` passes it through `known_arrival` as `Arrival::Known{time_s, half_width_s = R/c}`.
- **Consumers of the EDT result:** the results JSON (`parameters.edt_s` in SPPS bands, SPPS aggregates, per-source bands,
  per-source aggregates; TCR carries it refused `no_time_series`), its committed schema
  `docs/formats/results-json.schema.json` (test `the_committed_schema_is_the_one_results_schema_prints`), the CLI table
  `crates/simpa/src/results_cmd.rs` (`cell(&p.edt_s, ...)`, reads `.value()`), the tests that read `edt_s`
  (`cli_results.rs`, `m8_evidence.rs`, `noise_calibration.rs`, `results_load.rs`). **The UI and Tauri app do not read the
  results JSON at all** (no `edt` in `app/ui/src` or `app/src-tauri/src`; `runs.rs` only counts lost particles), and the
  generated bindings (`app/ui/src/bindings`) do not contain the results schema, so they did not change (M9 (e) confirms).

## 2. Fixture

`tools/edt_port/make_fixture.py` draws 575 rows into `testdata/edt_parity/` (`manifest.jsonl` 0.22 MB, `bins.bin` 3.55 MB,
little-endian f64, dense or sparse; read by plain `std` + `serde_json`): round 2's rows stratified by (set, SPPS mode,
step, status, refusal reason), at most 10 per stratum, seeded (551 rows: SPPS random and energetic at 1, 2 and 5 ms, ISM
at 1, 2 and 5 ms, Synth at 1, 2, 5 and 10 ms; ok, wide and every refusal reason round 2 produced), plus 24 crafted
rows (zeros, a 3-bin series, bad `dt`, negative energy, energy before the arrival, and a seeded fuzz keeping one row
per outcome: `no_energy_after_arrival`, a missing `t_arrival`, ...). The script loads `frozen2/method.py` only after
its sha256 equals `frozen2.sha256` and the value above, runs it on every row, and **asserts its output equals
`rows.csv.gz` exactly** (status, reason, edt, edt_lo, edt_hi) before writing the fixture.

## 3. Tests first

| test | red (against a stub / before the wiring) | green |
|---|---|---|
| `tests/edt_port.rs::every_fixture_row_equals_frozen2_...` | commit `d41b405`: refused / `not_implemented` vs frozen2 ok on every ok/wide row | 575 rows, max rel diff 1.5e-15 |
| `edt_port.rs::every_refusal_reason_of_frozen2_is_reachable_from_rust` | red, same stub | the 8 reachable reasons reached, equal to frozen2's |
| `edt_port.rs` coverage + missing-arrival tests | pass (fixture checks) | pass |
| `report.rs::the_method_gets_the_arrival_with_the_source_delay_rounded_up_to_the_next_whole_step` | commit `5355040`: `edt` is None | passes; **fails with the delay dropped** (below) |
| `report.rs::edt_s_is_the_methods_value_or_its_refusal_as_edt_refused` | red (no `edt`) | passes |
| `report.rs::energetic_runs_mark_edt_not_yet_validated_and_random_runs_do_not` | red (no `edt`) | passes |

**Tolerance:** 1e-9 relative. Reason: float order of operations. numpy sums with its own unrolled/pairwise order and
takes `log10` from the C library; Rust sums left to right. Each operation is correctly rounded either way, so the values
agree to a few ulps. Measured: 1.5e-15 on the 575-row fixture; **3.6e-14 over every one of the 14,800 round-2 rows
plus the 24 crafted** (a scratch fixture, 112 MB, not committed; reproducible from the tree, section 6). Statuses and reasons (including frozen2's
`hw=...;fit=...;noise=...;tail=...;n=...` strings, character for character) are identical on all of them.

**Delay test.** The test builds an SPPS run whose source has `delay = 0.0153 s` at `dt = 1 ms` (15.3 steps, so the
particles start at step 16, 0.016 s), runs the real `receiver_report`, and requires: the report's `arrival_s` is
`emission_s + d/c` (0.016 + 0.010, not 0.0153 + 0.010); the `edt.arrival_s` the method got is that; the EDT equals
`edt::analyse` given that arrival and differs from `edt::analyse` given `d/c` alone (the test refuses to run if the
series cannot tell the two apart; the direct sound is spread over `[t - R/c, t + R/c]` as SPPS records it, so a too-early
arrival starts the fit inside the direct sound). **Mutation, run and reverted:** with `arrival_from` returning `d / c`
(delay dropped) the test fails (`left: Some(0.01), right: Some(0.026000000759959223)`).

**Refusal reasons.** frozen2 has nine. Eight are reached from Rust on the fixture, equal to the ones frozen2 gives:
`no_energy`, `no_energy_after_arrival`, `run_too_short`, `direct_only`, `step_too_coarse`, `too_few_particles`,
`not_decaying_at_run_end`, `receiver_too_large`. **`not_decaying` is unreachable in frozen2 itself**: the Schroeder level
is non-increasing, the fit starts above -10 dB and ends at or below it, so the least-squares slope is strictly
negative; a seeded fuzz of 6,000 inputs never reached it either. The port keeps the guard. Likewise `noise=unknown` (no
noise window block with energy) is never reached by round 2 or the fuzz; the guard is ported.

**Marker.** `parameters.edt.validated` is `true` and `validation_note` absent for Random-mode runs; `false` with
`"not yet validated: EDT v2.1 passed its held-out test for random-mode runs only; energetic mode failed H3 (VERDICT-2,
2026-10-02)"` for Energetic runs. This is the recorded default (VERDICT-2 ruling 2, P11); Burhan's ruling is pending.
The value is still computed and shown (with the marker), not suppressed: VERDICT-2 says "marked not yet validated".

## 4. What changed

- `crates/simpa-core/src/params/edt.rs` (new): the port, line for line, with frozen2's constants and refusal order.
- `crates/simpa-core/src/results/report.rs`: `edt_report()` runs the port on each SPPS band's, aggregate's and
  per-source band's raw histogram with `arrival_from`'s arrival and `R/c`; `Parameters::edt_s` becomes the port's value
  (`{"value", "mc_sd": null}`) or refused `edt_refused`; new `Parameters::edt` (`EdtReport`: method, status, `value_s`,
  `lo_s`, `hi_s`, `reason`, `arrival_s`, `validated`, `validation_note`); `REPORT_VERSION` 5 to 6. TCR and
  `several_sources` carry no `edt` object.
- `crates/simpa-core/src/params.rs`: `NotEvaluable::EdtRefused { reason }`.
- `docs/formats/results-json.schema.json` regenerated; `docs/formats/results-json.md`, `docs/solver-contract.md` updated.
- `crates/simpa/tests/cli_results.rs`: `every_band_of_the_committed_runs_has_all_eight_parameters_or_their_reasons`
  asserted that the 2,000-particle Seats run has **no value at all** (every value `noise_uncalibrated`). With the port,
  EDT answers that run (6 values, the method judges its own noise and the M7 calibration domain does not gate it), so
  the test now counts EDT apart and checks each EDT against its `edt` object and marker; the other seven quantities are
  still all refused.
- **The old EDT computation is kept** inside `params::decay` / `params::noise` (it is the `QUANTITIES[1]` slot of the
  bootstrap, and `bed`, `m8_evidence.rs` and `noise_calibration.rs` measure it). It no longer reaches the results JSON.
  `report::parameters()` (public, used by those evidence tests) still returns the old `edt_s`.
- **Aggregates:** the port is applied to the summed-bands histogram too; frozen2 was tested on single bands only, so the
  aggregate's EDT is outside what H1-H6 covered and is `validated: false` in every mode (section 6, finding 1).

## 5. Gates (of `9f47fe3`, superseded by section 7)

(recorded below)

Run on Grace, 2026-10-02, after the last code commit (`9f47fe3`), solvers `C:\tmp\nm-m8a-solvers`,
`SIMPA_TETGEN160=C:\tmp\nm-m10-solvers\build\src\tetgen\Release\tetgen.exe`; `npm ci` in `app/` first (0 vulnerabilities).

- `cargo test` (simpa-core, simpa; inside m10 and m11, `--no-fail-fast`, 4 threads, solvers present): **pass**.
  Run by hand before that, without the gate's solver staging, 8 solver-running tests of `cli_results.rs` fail with exit 2;
  they fail identically on the commit before the wiring, so that was the missing staging, not the port.
- `tools/gates/m11.ps1`: first run **FAILED 1 check**: `core crates: clippy -D warnings`, two `chunks_exact` lints in my
  new `tests/edt_port.rs`. Fixed in `9f47fe3`. Rerun: **M11 PASSED** (it ran m10 `-SkipCore` and m9 in full, both PASS,
  e2e 0 failures, focus judge PASS).
- `tools/gates/m10.ps1 -SolversDir C:\tmp\nm-m8a-solvers`: **M10 PASSED**.
- M9 (inside m11, "prior gate: m9.ps1 in full prints M9 PASSED"): **PASS**, bindings regenerate to the same blobs.
- msedgedriver matched WebView2; `-FetchDriver` was not needed.
- Pre-existing, left alone: `app/src-tauri/Cargo.toml` shows modified in the worktree (line endings only); not committed.

## 6. Audit findings closed (test first)

Each test was written and run red against the tree at `b84295e`, then the fix made it green.

**Finding 1 (BLOCKER), the summed-bands EDT read `validated: true` for Random runs.** The held-out test read single bands
only. Red: `report.rs::the_aggregate_edt_is_not_validated_whatever_the_mode` failed at `mode 0` (Random aggregate
`validated == true`). Fix: `edt_report(.., broadband)`; every aggregate (the receiver's and each per-source one) is
`validated: false` with `validation_note` = "not yet validated: broadband EDT is not covered by the held-out test
(VERDICT-2 tested single bands only)" (`EDT_BROADBAND_NOT_VALIDATED`), Random or Energetic. A single Random band is
unchanged (true, no note). `cli_results.rs::every_band_of_the_committed_runs_...` now expects `validated` false on the
aggregate.

**Finding 2 (MAJOR), `edt_s` and the CLI table showed an unvalidated EDT unmarked.** Rule, documented in
`docs/formats/results-json.md` (EDT paragraph): every surface that prints or exports an EDT that is not validated carries
the marker. Schema: `parameters.edt_validated` (boolean, on every `parameters`; true only where `edt.validated` is true;
false where there is no `edt` object, i.e. TCR and several-sources). `edt_s` is unchanged (`{"value","mc_sd"}`), so
consumers that read it keep working; the marker rides beside it. CLI: an unvalidated EDT value gets a `*` after it
(`edt_cell`), refusals are not marked, and a legend line "* EDT not yet validated ..." prints when any cell is marked.
Red: `report.rs::edt_s_carries_its_marker_beside_it_on_every_surface` (Random band `edt_validated` null, not true) and
`cli_results.rs::text_mode_marks_every_edt_that_is_not_validated` ("an unvalidated EDT unmarked: aggregate ... 0.74").
`docs/formats/results-json.schema.json` regenerated (`results_version` stays 6: the branch is unmerged and the field is
additive). No other reader of `edt_s` prints or exports it: the UI and Tauri app do not read the results JSON (section 1).

**Finding 3 (MAJOR), full parity was not reproducible from the tree.** The scratch fixture came from
`make_fixture.py --per-stratum 100000` and an untracked `EDT_FIXTURE_DIR`. Now an ignored test,
`tests/edt_port.rs::full_parity_from_the_round2_inputs`, runs `tools/edt_port/make_fixture.py --per-stratum 1000000`
on `B:\data\m8b-edtound2esults` (`inputs_{spps,ism,synth}.pkl.gz`, `rows.csv.gz`) into scratch (the script checks
frozen2's sha256 and that frozen2's own output equals `rows.csv.gz`, then the Rust port is compared on every row); it
prints SKIPPED and passes when the inputs are absent. Command, from the repo root:

    cargo test -p simpa-core --test edt_port --release -- --ignored --nocapture full_parity

(env `EDT_ROUND2_DIR`, `EDT_FULL_SCRATCH`, `PYTHON` override the results folder, scratch folder and interpreter; Python
needs numpy.) **Run 2026-10-02: 14,824 rows (14,800 round 2 + 24 crafted), max rel diff 3.6e-14 (`synth|5|1ms|205`),
statuses and reasons identical on all rows** (23.7 s).

## 7. Gates after the audit fixes

(recorded below)
