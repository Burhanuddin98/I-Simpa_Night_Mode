# Backlog 38 and 39: GREEN

Step 2 of `PLAN.md`, on branch `b38-39`, 2026-09-30 22:38-23:10. Commits: `910bb8f` (T38-8 alone,
with stubs), `f745900` (the implementation), and this file. No gate was run: M11, M10 and M9 are the
next agent's.

## Results

Environment in every PowerShell call: `CARGO_TARGET_DIR=C:\tmp\nm-target`, `CARGO_INCREMENTAL=0`,
`CARGO_BUILD_JOBS=16`, `RUST_TEST_THREADS=4`, `SIMPA_TEST_SCRATCH_ROOT=C:\tmp\nm-target\test-scratch`,
from the worktree. The UI commands run from `app\`. Each id below was run on `f745900`.

| Id | Result | Command |
|---|---|---|
| T38-1 | pass | `cargo test -p simpa-core --test results_solver_build t38_` (4 passed) |
| T38-2 | pass | the same |
| T38-3 | pass | the same |
| T38-4 (control) | pass | the same |
| T38-5 | pass | `cargo test -p app --bin app t38_` (2 passed, with T38-8) |
| T38-6 | pass | `cargo test -p simpa --test cli_results t38_` |
| T38-7 | pass | `node --test --test-name-pattern=t38_7 ui/src/features/simulate/model.test.ts` |
| T38-8, data (`runs.rs`) | pass | `cargo test -p app --bin app t38_` |
| T38-8, the row's mark (`dock/model.test.ts`) | pass | `node --test --test-name-pattern=t38_8 ui/src/features/dock/model.test.ts` |
| T38-9 (`m11-b38`) | not run | e2e: `tools/gates/m11.ps1`, the next agent's gate run |
| T39-1 | pass | `cargo test -p simpa --test cli_run t39_` (5 passed) |
| T39-2 (control) | pass | the same |
| T39-3 | pass | the same |
| T39-4 (control) | pass | the same |
| T39-5 | pass | the same |
| scanner say-no (not in the table) | pass | `cargo test -p simpa-core --test reason_codes_docs` (3 passed) |

### T38-8's RED

Committed alone in `910bb8f`, against stubs that reproduce today's Runs-row rule (`solversMark`:
every recorded check matching is verified, whatever the checks cover). Both halves failed on their
assertion:

- `app/src-tauri/src/runs.rs:1965:13`: `assertion left == right failed: checks that cover only
  TetGen: the Runs row reads Some(Verified)`, left `true`, right `false`.
- `app/ui/src/features/dock/model.test.ts:266:10`: `AssertionError [ERR_ASSERTION]: checks that
  cover only TetGen are not a verified build`, actual `'verified'`, operator `notStrictEqual`.

The Rust half reads the row where `runs_list` makes it (`list`), for five records of T38-5's run:
TetGen only, none, an empty list, the solver's check failing, all matching. It requires the row's
verdict to be the core predicate's, code and detail, and to agree with `results_state` for the same
run. A row with no `run.json` carries none. The UI half requires the mark to follow
`RunRow.solver_build`, not the checks. In the same run, T38-5 failed at `runs.rs:1892`, as in RED.md.

The scanner's new say-no test failed first too: `reason_codes_docs.rs:449:32`, `solver_build_unrecorded
is not read as produced`. The scanner then read `mod build_codes`, and the code table was added.

### T38-9's route

**The harness can seed such a run, and already does.** `m11.ps1`'s "the planted-loss run" makes a
real box run with `simpa run` in `p\loss`. A CLI run's `run.json` has no `solvers` key, byte for byte
what a run made before M11 wrote (`run/manifest.rs:143-149`). Its verdict is OK, and its results
load: `plant-loss.ts` rewrites only `particles`, and `load` re-reads the solver's own files. So no new
seeding was needed.

The id is `m11-b38`, in `app/e2e/specs/m11.gate.e2e.ts` after `m11-e-results`, and in `m11.ps1`'s
`gate` list, so M11 now requires 32 ids. The test checks that the run is OK with no solver record. It
opens the run's Results step and requires `data-results-state="unverified"` and the codes
`[data-part="unverified"] [data-code]` to be exactly `SOLVER_BUILD_UNRECORDED`. It also requires the
core code in the text, no "Results verified", no `[data-result]`, and no digit in the text or
tooltips outside `[data-run-label]`. `run_results` must answer `verified: false`, no refusal, and
`unverified.code` `solver_build_unrecorded`. The selected Runs row must read
`data-verified="unrecorded"` and `data-build-code="SOLVER_BUILD_UNRECORDED"`.

**Its RED follows from T38-5, so the app was not built twice.** Before the change, `results_state`
answered `verified: true` for such a run (RED.md, T38-5's line), and `resultsStateName` mapped that to
`verified`. The panel read `data-results-state="verified"`, and the row had no `data-build-code`. The
spec and its library were type-checked the way `m11.ps1` does it: `tsc -p app\e2e\tsconfig.json`
exited 0 over 34 files, `m11.gate.e2e.ts` and `lib/m11.ts` among them.

## The rest of every touched test file

The full-file runs used the solver build the gate uses: `SIMPA_SOLVERS_DIR=C:\tmp\nm-m8a-solvers`,
`SIMPA_TETGEN160=C:\tmp\nm-m10-solvers\build\src\tetgen\Release\tetgen.exe`,
`SIMPA_UPSTREAM=B:\repos\I-Simpa-upstream`. The solver runs are the ones those tests make, and none
was run by hand.

| Suite | Result | Command |
|---|---|---|
| both `model.test.ts` in full (simulate, dock) | 33 of 33 | `node --test ui/src/features/simulate/model.test.ts ui/src/features/dock/model.test.ts` |
| the UI's suites | 146 of 146, known answers 6 of 6 | `npm test -s` |
| `cli_results` | 24 passed, 4 ignored (by design) | `cargo test -p simpa --no-fail-fast --test cli_run --test cli_results --bin simpa` |
| `cli_run` | 18 of 18 | the same |
| `simpa` unit tests | 6 of 6 | the same |
| the app crate (the runs tests among them) | 55 of 55 | `cargo test -p app` |
| `reason_codes_docs` | 3 of 3 | below |
| simpa-core `--lib` (`results::tests`, `results::report::tests`, ...) | 208 passed, 1 ignored | `cargo test -p simpa-core --no-fail-fast --lib --test results_load --test results_rooms --test results_solver_build --test reason_codes_docs --test run_contract_docs --test validate_contract_docs --test noise_inputs --test run_manifest --test run_manager --test ui_fixtures` |
| `results_load`, `results_rooms`, `results_solver_build`, `noise_inputs` | 14, 3 (1 ignored), 4, 2 | the same |
| `run_contract_docs`, `validate_contract_docs` (they read the contract page) | 4, 3 | the same |
| `run_manager`, `run_manifest`, `ui_fixtures` | 13, 9, 1 | the same |
| the e2e library's suites | 42 of 42 | `node --test "e2e/lib/*.test.ts"` |

Every other test that reads a report or `simpa results` output is `#[ignore]`d evidence
(`m8_evidence.rs`, `noise_calibration.rs`, `bed_m8a.rs`).

## Checks

| Check | Result | Command |
|---|---|---|
| rustfmt | clean, exit 0 | `cargo fmt --all -- --check` |
| clippy, app (`m11.ps1:296`) | clean, exit 0 | `cargo clippy -q -p app --no-deps --all-targets -- -D warnings` |
| clippy, core (`m11.ps1:302`) | clean, exit 0 | `cargo clippy -q -p simpa-core -p simpa --all-targets -- -D warnings` |
| typecheck | clean, exit 0 | `npm run -s typecheck` |
| e2e typecheck | clean, exit 0 | `node_modules\.bin\tsc.cmd -p e2e\tsconfig.json` |
| bindings freshness (`m9.ps1` (e), by hand) | 4 of 4 identical, `git diff --quiet` exit 0 | `npm run -s bindings -- --out C:\tmp\b38-39\bindings-regen`, then `git hash-object --path=<rel>` of each regenerated file against the work tree, `:<rel>` and `HEAD:<rel>` at `f745900` |

`m9.ps1` dumps with the release `app.exe` (`--app`), and here the debug build dumped (`cargo run`),
from the same code: `schema.json` `6ebd736b7a55`, `ipc.json` `ab5eb5950ffe`, `schema.ts` `e01ff5ac0f4b`,
`ipc.ts` `66ce4c130938`. The dependencies were installed with `npm ci` in `app\` (52 packages, 8 s),
since neither gate script names another way. `app\node_modules` is ignored by `app\.gitignore`.

## Files changed

`910bb8f`, T38-8 with its stubs:
- `app/src-tauri/src/runs.rs`: T38-8's data test, `SolverBuildUi`, `RunRow.solver_build` (stubbed with
  `solversMark`'s rule).
- `app/ui/src/features/dock/model.ts`: `BuildMark` and `buildMark` (stubbed: `solversMark` on the
  checks).
- `app/ui/src/features/dock/model.test.ts`: T38-8's UI test.

`f745900`, the implementation:
- `crates/simpa-core/src/results.rs`: `solver_build`, the one predicate (C5). Checks are matched by
  `check_solvers`' names; missing or empty is `solver_build_unrecorded`, a failing check
  `solver_build_mismatch`, and no check of the run's solver `solver_build_unchecked`.
- `crates/simpa-core/src/results/report.rs`: `Report.solver_build`, which `report()` computes from the
  manifest. `checked_report`'s code, and its Ok or Err, are unchanged (C1).
- `crates/simpa-core/src/run/manager.rs`: `run_project`'s doc comment, and one comment in its body,
  state the new rule. No code changed (C3).
- `crates/simpa-core/tests/reason_codes_docs.rs`: the scanner reads `mod build_codes` as it reads
  `mod codes`, with a say-no test for it.
- `crates/simpa/src/results_cmd.rs`: the verdict and code on one line of the text. `--json` carries
  them through `Report`, and the exits are unchanged.
- `crates/simpa/src/mesh_run.rs`: `simpa run` takes `--base`, and defaults to the file's active variant
  (by id, read with `validate::read_project`). `--base` with `--variant` is exit 2 with nothing on
  stdout, and the doc comment states the rule.
- `crates/simpa/src/main.rs`: the run usage states the default and `--base`.
- `app/src-tauri/src/runs.rs`: `SolverBuildUi::of`, with `row_from_manifest` and `results_state` on the
  core predicate. `run_ui_code` names the build codes at every stage, and the UI-code test now lists
  them (it was extended, not weakened).
- `app/ui/src/bindings/ipc.json`, `ipc.ts`: regenerated (`ResultsState.unverified`,
  `RunRow.solver_build`, `SolverBuildUi`).
- `app/ui/src/features/simulate/model.ts`: `resultsStateName` answers `unverified`, and
  `resultsCodes` gives the refusal's or the unverified reason's code.
- `app/ui/src/features/simulate/ResultsPanel.tsx`: the `unverified` branch, "UNVERIFIED · Results
  unverified" with `resultsCodes(answer)` under `data-part="unverified"`, no digit (C6). The refusal
  branch renders `resultsCodes` too.
- `app/ui/src/features/simulate/simulate.css`: one rule colouring the new verdict's label with the
  existing `--warn` token. `theme.css` is untouched.
- `app/ui/src/features/dock/model.ts`: `buildMark` reads `RunRow.solver_build`. `solversMark` stays
  (the existing dock test pins it) and now supplies only the title's file names.
- `app/ui/src/features/dock/RunsPane.tsx`: the record's mark comes from `buildMark`, with
  `data-build-code`. `data-verified` keeps its three values.
- `app/e2e/specs/m11.gate.e2e.ts`: `m11-b38` (T38-9).
- `app/e2e/lib/m11.ts`: the `resultsState` hook's type gains `unverified`.
- `tools/gates/m11.ps1`: `m11-b38` is a required id.
- `docs/solver-contract.md`: Part B, "Solver build", with the three codes' rows.
- `docs/formats/results-json.md`: `solver_build` in the report, and the text's verdict line.
- `docs/formats/results-json.schema.json`: regenerated with `simpa results --schema`.
- `docs/results.md`: the solver build is judged beside `load`, not by it.

This commit adds `docs/investigations/2026-09-30-b38-39/GREEN.md`.

## For the review

- **C1, C2.** `load`, `check_status` and `checked_report` are not edited, and nothing under
  `crates/simpa-core/src/bed/` is. The report gains a field. The bed reads the report's named fields
  and serialises none of it (`bed/read.rs:315-321`), so its output does not change.
- **`REPORT_VERSION` stays 5.** Its rule is "bumped when a field changes meaning", no field changed
  meaning, and the schema allows extra fields (`results-json.md`). The review may prefer a bump.
- **The Runs row keeps no copy of the rule.** Its verdict arrives as `RunRow.solver_build`. The UI
  maps the core's code `solver_build_unrecorded`, or no verdict, to "unrecorded", which the dock e2e
  expects for CLI and Interrupted rows.
- **Wrong tests: none.** Every RED.md test passes unchanged.

## Existing tests the gates may break

None expected, for these reasons:
- The e2e ids that expect `verified` (`m11-e-results`' control, `m11-sim-numbers`, `m11-smoke`) and
  `m11-sim-link`'s `verified | refused` all read app runs of the box. An app run records the
  solver's, TetGen's and preprocess.exe's checks (`run/manager.rs:572-581`), all matching under the
  gate's private copy, so it stays verified.
- `m11-dock-*` read `data-verified`: `yes` for app runs, `unrecorded` for the CLI and Interrupted rows.
  The same values come out.
- `m11-h` now meets the `unverified` panel on the loss project. Its text and tooltip hold no number
  next to a unit and no parameter with a number.
- Only one fixture project sets an active variant, `tests/fixtures/negative/schema/
  variant_reference_invalid.simpa`, and no test passes it to `simpa run`. The gates' own `simpa run`
  calls (`m11.ps1:497,514`, `m5.ps1`, `m6.ps1`, `m11-r22-default`) use projects with none, so the new
  default does not change what they run.
- `m11-b38` itself is new, and has not run.

## Left for step 5 (docs), not done here

- `docs/decision-log.md` row 29 and `docs/v1.1-backlog.md` rows 38 and 39 (PLAN step 5).
- `docs/m5-m6-design.md:541-545` lists `simpa run`'s options without `--base` or the default.
- `simpa export-config` still defaults to the base. The decision names `simpa run` only, so it was
  left alone.

## Left behind

The failing RED runs of T38-5 and T38-8 at 22:50 kept their scratch folders, one copy of `seats_tcr`
each, under `%TEMP%`: `simpa-app-m11-t38-5-19636-*` and `simpa-app-m11-t38-8-19636-*`. The RED
check's two from 22:19 and 22:25 are there too. Passing tests removed their own. Nothing is on B:
beyond `app\node_modules`, which is ignored. C: had 18.5 GB free at 23:01.
