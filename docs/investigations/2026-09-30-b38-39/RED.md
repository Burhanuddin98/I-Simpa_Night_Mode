# Backlog 38 and 39: RED

Step 1 of `PLAN.md`: the tests written first, run on branch `b38-39` at `bc1b3f1` plus this commit's
tests and stubs, 2026-09-30 22:10-22:25. Every red test below fails on its assertion and every
control passes. **Pass bar 1 still needs an agent other than the writer to confirm this.**

## Results

| Id | File :: test | Kind | Result | Failure line |
|---|---|---|---|---|
| T38-1 | `crates/simpa-core/tests/results_solver_build.rs` :: `t38_1_a_run_without_a_solver_record_loads_and_its_build_is_unverified` | red | fails on assertion | `results_solver_build.rs:62`: `results/seats_spps: Verified` (`load` was Ok first, C1) |
| T38-2 | `crates/simpa-core/tests/results_solver_build.rs` :: `t38_2_a_recorded_check_that_does_not_match_leaves_the_build_unverified` | red | fails on assertion | `results_solver_build.rs:89`: `the solver's own check: Verified` |
| T38-3 | `crates/simpa-core/tests/results_solver_build.rs` :: `t38_3_an_empty_solver_record_leaves_the_build_unverified` | red | fails on assertion | `results_solver_build.rs:105`: `an empty list: Verified` |
| T38-4 | `crates/simpa-core/tests/results_solver_build.rs` :: `t38_4_every_recorded_check_matching_verifies_the_build` | control | passes | |
| T38-5 | `app/src-tauri/src/runs.rs` :: `runs::tests::t38_5_results_state_marks_a_run_without_a_solver_record_unverified` | red | fails on assertion | `runs.rs:1858`: `a run with no solver record is not verified: ResultsState { run: "20260924-115031-155-tcr", verified: true, refusal: None, unverified: None }` |
| T38-6 | `crates/simpa/tests/cli_results.rs` :: `t38_6_results_prints_the_unverified_solver_build_and_still_exits_0` | red | fails on assertion | `cli_results.rs:2651`: `no line of the text names the unverified verdict and solver_build_unrecorded` (exit 0 asserted first, passed) |
| T38-7 | `app/ui/src/features/simulate/model.test.ts` :: `t38_7 the Results step shows an unverified solver build as unverified with its reason code` | red | fails on assertion | `model.test.ts:401`: `AssertionError: marked unverified, not refused (C6)`, actual `'refused'`, expected `'unverified'` |
| T39-1 | `crates/simpa/tests/cli_run.rs` :: `t39_1_run_without_variant_exports_the_active_variant` | red | fails on assertion | `cli_run.rs:1289`: `without --variant the run must export the active variant 'rotated' [.. absorb 0.1, 0.2, 0.3], not the base [..]; it exported [.. absorb 0.3, 0.1, 0.2]` |
| T39-2 | `crates/simpa/tests/cli_run.rs` :: `t39_2_an_explicit_variant_wins_over_the_active_one` | control | passes | |
| T39-3 | `crates/simpa/tests/cli_run.rs` :: `t39_3_base_runs_the_base_when_an_active_variant_is_set` | red | fails on assertion | `cli_run.rs:1335`: `assertion left != right failed: --base is refused as a usage error: simpa: unknown option '--base'` |
| T39-4 | `crates/simpa/tests/cli_run.rs` :: `t39_4_with_no_active_variant_the_default_runs_the_base` | control | passes | |
| T39-5 | `crates/simpa/tests/cli_run.rs` :: `t39_5_the_help_states_the_default_variant_and_the_base_spelling` | red | fails on assertion | `cli_run.rs:1393`: `the help of run names no --base` |

What each test pins beyond the table's one line, so that GREEN is held to it:

- **T38-1** loads both committed runs (`seats_spps`, `seats_tcr`, no `solvers` key) and requires
  code `solver_build_unrecorded`.
- **T38-2** covers a mismatch in the solver's own check and in `tetgen.exe`'s, each
  `solver_build_mismatch`.
- **T38-3** covers an empty list (`solver_build_unrecorded`) and, from C5, checks that cover
  `tetgen.exe` but not the solver the run executed (`solver_build_unchecked`).
- **T38-5** requires `refusal: None` (C6: unverified is not refused) and the reason in the new field
  `ResultsState.unverified`, core code `solver_build_unrecorded`, UI code `SOLVER_BUILD_UNRECORDED`.
- **T38-6** requires exit 0 in text and `--json`, a text line holding both `unverified` (any case) and
  the code, and the code anywhere in the JSON. It does not pin where the JSON carries it.
- **T38-7** requires state `unverified` and `resultsCodes(..)` equal to the reason. It also checks
  that a refusal stays `refused` with its code and that a verified run shows no code.
- **T39-1** also requires `run.json`'s `source.variant` to name the variant written, by id or name.
- **T39-3** also requires `source.variant` null, and exit 2 with nothing on stdout for `--base`
  combined with `--variant` (C4).
- **T39-5** reads the usage from `simpa run <project.simpa>` to `simpa results` (run, run-folder and
  their shared paragraph). It needs `--base` and `active variant` (any case) in that text.

## Commands

Environment, set in each PowerShell call:

```powershell
$env:CARGO_TARGET_DIR='C:\tmp\nm-target'; $env:CARGO_INCREMENTAL='0'; $env:CARGO_BUILD_JOBS='16'
$env:RUST_TEST_THREADS='4'; $env:SIMPA_TEST_SCRATCH_ROOT='C:\tmp\nm-target\test-scratch'
Set-Location 'B:\repos\I-Simpa_Night_Mode\.claude\worktrees\b38-39'
```

Then, each target by name only (no full suite, no gate):

```powershell
cargo test -p simpa-core --test results_solver_build t38_      # T38-1..4
cargo test -p app --bin app t38_                               # T38-5
cargo test -p simpa --test cli_results t38_                    # T38-6
cargo test -p simpa --test cli_run t39_                        # T39-1..5
# T38-7, from app/ (Node 24.15.0; needs no node_modules):
Set-Location 'B:\repos\I-Simpa_Night_Mode\.claude\worktrees\b38-39\app'
node --test --test-name-pattern=t38_7 ui/src/features/simulate/model.test.ts
```

Also run: `cargo fmt --all -- --check`. It is clean after `cargo fmt -p simpa`, which touched only
this commit's lines in `cli_run.rs`. Clippy and `npm run typecheck` were not run.

**No SPPS or TCR run.** T38-1 to T38-6 read the committed run folders under
`tests/fixtures/results/`. T39-1 to T39-4 pass `--solver-exe` the stub solver
(`simpa-stub-solver.exe`), which finds no `stub.json` and exits at once. Each run meshes the box
with TetGen and writes `solve/config.xml` first. T39-3's combined-flags call and T39-5 are usage
errors and run nothing. The T39 tests take 0.17 s together.

## Stubs

Each stub reproduces today's behaviour, so the tests compile and fail on their assertions:

| Where | What | Today's behaviour it reproduces |
|---|---|---|
| `crates/simpa-core/src/results.rs:417` | `pub mod build_codes`: `UNRECORDED` `solver_build_unrecorded`, `MISMATCH` `solver_build_mismatch`, `UNCHECKED` `solver_build_unchecked`, `ALL` | New constants only; nothing reads them yet |
| `crates/simpa-core/src/results.rs:435` | `pub enum SolverBuild { Verified, Unverified { reason: Reason } }`, with `is_verified()` and `reason()` | New type |
| `crates/simpa-core/src/results.rs:464` | `pub fn solver_build(&RunManifest) -> SolverBuild`, the C5 predicate | Always `Verified`: a run whose results load counts as verified whatever its `solvers` record says |
| `app/src-tauri/src/runs.rs:265` | `ResultsState.unverified: Option<ReasonUi>` | Always `None` (set at `:673` and `:683`); `verified` is still `results::load(..).is_ok()` (`:666`) |
| `app/ui/src/features/simulate/model.ts:408` | `ResultsStateName` gains `'unverified'` | Type only; `resultsStateName` (`:419`) still maps `verified: false` to `refused` |
| `app/ui/src/features/simulate/model.ts:439` | `resultsCodes(results): ReasonUi[]`, the codes the Results step shows | The refusal's code only, as `ResultsPanel.tsx` renders today |

`results::load`, `check_status`, `checked_report` and everything under `crates/simpa-core/src/bed/`
are unchanged (C1, C2). The CLI (`crates/simpa/src`) is unchanged, so T38-6, T39-1, T39-3 and T39-5
fail against today's real code, with no stub in the way.

## The base spelling (C4)

**None exists today, so the spelling is the new `--base`.** `config_xml::resolve_variant`
(`crates/simpa-core/src/config_xml/write.rs:123-143`) maps `None` to the base and anything else to a
variant, by id first and then by unique name. It has no keyword for the base. `simpa run` parses
only `--variant <v>` (`crates/simpa/src/mesh_run.rs:536-566`), and `--base` is refused today as an
unknown option (T39-3's failure line). A variant could be named `base`, so a reserved `--variant`
value would be ambiguous. That is one more reason for a separate flag.

## T38-7's route

**The UI unit-test harness.** `app/package.json`'s `test` script runs `node --test` over
`ui/src/**/*.test.ts`. No e2e id is added, so the M11 gate keeps its 31 required ids. The harness
renders no React. T38-7 therefore tests the two model functions the Results step's panel is built
from: `resultsStateName`, which gives the state, and `resultsCodes`, which gives the codes shown.
**GREEN must make `ResultsPanel.tsx` render state `unverified` and `resultsCodes(answer)`**, or the
test does not cover what the user sees. Single-id command, from
`B:\repos\I-Simpa_Night_Mode\.claude\worktrees\b38-39\app`:

```powershell
node --test --test-name-pattern=t38_7 ui/src/features/simulate/model.test.ts
```

## For GREEN

- **Bindings.** `ResultsState` gained a Rust field, but `app/ui/src/bindings/ipc.{json,ts}` were not
  regenerated: that needs `npm run bindings`, and this worktree has no `node_modules`. M9 gate (e)
  and `npm run typecheck` fail until they are. T38-7's object literals carry `unverified`, which only
  type-checks after regeneration.
- **New codes.** The three `solver_build_*` codes need rows wherever reason codes are documented, and
  UI codes in `run_ui_code`'s table (`runs.rs`,
  `the_ui_code_table_covers_every_code_a_run_can_carry`) if they reach the Runs tab.
- **The Runs row.** `solversMark` (`app/ui/src/features/dock/model.ts:254`) already decides the Runs
  row's solver status from `solvers`. C5 says it moves to core; that is not stubbed here.
- **A PLAN reference, corrected.** `validate.rs:358` is `validate_with`. The validator reads the
  active variant at `crates/simpa-core/src/validate/project.rs:79`. The app passes it at
  `app/src-tauri/src/runs.rs:932`. Every other file:line in the PLAN's "What the code does today"
  matched.

## Left behind

Failing tests keep their scratch folders by design, and none is on B:.
`C:\tmp\nm-target\test-scratch\cli\t39-1-*` and `t39-3-*` hold four run folders, about 1.5 MB with
the rest of `cli\`. `%TEMP%\simpa-app-m11-t38-5-*` holds one copy of `seats_tcr`. C: had 19 GB free
at 22:23.
