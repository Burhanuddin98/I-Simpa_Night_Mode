# M11 review 2: the core contract changes (C1, C5, C6, C7, C9, results refusal, runs_list, bed)

2026-09-30, 00:06 to 00:28, Grace. Branch `m11` at `2642cff`, diff `d7ecbda...m11`. Commits in scope: `34d5408`,
`e25f6eb`, `f7b1366`, and the later core edit in `acdf1e6` (the last commit touching `crates/`, 21:44).

**Verdict: one blocker, four minors.** C1, C6, C7 and C9 hold, and each claim's test fails when its change is
reverted. C5 holds in the app and the bed, **but not in the CLI**: `simpa run` validates one variant's materials
and exports another's. A run on upstream's placeholder then reaches the solver, gets verdict OK, and `simpa results`
verifies it (finding 1).

## How it was checked

Nothing in the worktree was edited except this file. No app was launched. `tasklist` showed no `app.exe` before each
run. The only processes launched were TetGen and preprocess.exe (probe runs), classicalTheory.exe (the core tests,
3 probe runs, 1 CLI run, about 1 s each), and my own stand-in executable.

- **Probe crate** (scratchpad `probe/`): depends on the worktree's `simpa-core` by path, pinned to its `Cargo.lock`,
  and built into `C:/tmp/nm-target`. Its modes are a manifest round-trip, a validator sweep, the variant scenarios, a
  priority check and a panic run with a live child. `prio_stub.exe` stands in for tetgen.exe and preprocess.exe: it
  prints its PID and its own `GetPriorityClass`, then sleeps as told.
- **Mutant copy** (scratchpad `mut/`): a byte-for-byte copy of `crates/simpa-core`, with `tests/fixtures`,
  `testdata`, `solvers/manifest.json`, `beds/m8a.json` and `rust-toolchain.toml` (1.98.1). Six changes were reverted
  in the copy only, then built and tested. The copy was then restored and checked equal to the worktree with
  `diff -r`, and the same six targets were run again as the baseline.
- Environment: `SIMPA_SOLVERS_DIR=C:/tmp/nm-m8a-solvers` and `SIMPA_TEST_SCRATCH_ROOT=C:/tmp/nm-rc`, with 4 test
  threads. The first scratch root, under the scratchpad, pushed TCR's output paths past 260 characters. The core's
  own `output_path_too_long` rule refused them, which is correct behaviour; I moved the root to `C:/tmp/nm-rc`.

### Mutations: each claim's test against its reverted change

| Claim | Reverted in the copy | Test | Result on the mutant | Baseline (unmutated copy) |
|---|---|---|---|---|
| C1 | `SetOnDrop` removed, `done.store(true)` after `mesher.run` restored (`mesh.rs:1297,1306`) | `mesh_project::a_mesher_that_panics_is_not_held_for_its_time_limit` | FAILED, `the panic was held 120.015892s` | pass (mesh_project 18/18) |
| C5 | `placeholders(p, out)` not called (`validate/project.rs:20`) | `validate_projects::the_placeholder_material_is_refused_per_group` | FAILED, `left: 0 right: 10` | pass |
| C5 | same | `validate_fixtures::every_project_fixture_yields_exactly_its_code` | FAILED, `material_placeholder.simpa: expected only material_placeholder (Error), got []` | pass |
| C5 | same | `run_manager::a_project_on_the_placeholder_material_is_refused_before_meshing` | FAILED, `left: [] right: ["material_placeholder"]` | pass |
| C6 | `run_folder`'s `on_event(Started)` removed (`manager.rs:1077`) | `run_manager::the_first_event_names_the_run_folder` | FAILED, `left: Some("stage PreLaunch") right: Some("started")` | pass |
| C7 | `run_project`'s verify block never taken (`manager.rs:672`) | `run_manager::a_run_asked_to_verify_its_solvers_checks_them_first` | FAILED, `left: Some("stage Geometry") right: Some("stage Solvers")` | pass |
| C7 | `skip_serializing_if` removed from `RunManifest::solvers` (`manifest.rs:148`) | `run_manifest::the_solver_checks_are_written_only_when_made` | FAILED at `run_manifest.rs:140` (the `"solvers"` key written) | pass |
| C9 | `BELOW_NORMAL_PRIORITY_CLASS` dropped (`winproc.rs:73`) | `process_job::children_run_below_normal_priority` | FAILED, `left: Some(32) right: Some(16384)` | pass |

Baseline, 00:23:29 to 00:23:49: mesh_project 18 passed; run_manager 14 passed and 1 ignored; process_job,
validate_projects, run_manifest and validate_fixtures all passed. The three ignored tests predate M11 (the
`process_job` helper, the 100,000-seed scan, `write_negative_fixtures`), and none is a claim's test.

### Per claim

- **C1, the watch thread ends when the mesher panics: holds.**
  - `call()` (`mesh.rs:1249-1321`) has no early return, before the scope or in it. Inside the scope, the only code
    before the guard (1297) is the watch spawn. A spawn failure panics with no watch thread to wait for.
  - Every mesher call goes through `call()`: TetGen at 732, the `-d` follow-up at 1046, preprocess at 1597.
    `mesher.run` has no other caller.
  - **Live child** (probe `panic-real`): the TetGen stand-in prints `PID` and `CLASS` and would then sleep 60 s. The
    caller's line callback panics on `CLASS`, as the app's event sink could. The panic left `mesh_project_with` after
    **43.8 ms**, and the stand-in was already gone: `OpenProcess` fails, and `tasklist` shows no `prio_stub.exe`.
    `process::drive` drops the `JobTree` on unwind, and its readers are unscoped threads, so no child survives the
    panic.
- **C5, `material_placeholder`: holds for the app and the bed; fails for the CLI (finding 1).**
  - The app path is refused: probe case A-app, a project whose active variant puts every group on the placeholder,
    run with the active variant as `runs.rs:869-899,1007` passes it. It ended at `Validate`, FAIL,
    `material_placeholder`, with no mesh.
  - The bed is not affected: all **464** `project.simpa` files the M8a bed ran (`C:/tmp/nm-m8a-*`, 11 of them
    upstream's atmospheric validation) validate with **0** errors under m11. Bed cells name their material
    `alpha <α>` (`bed/file.rs:759`).
  - Legitimately named materials are not refused: of the 56 `.simpa` fixtures, only
    `negative/schema/material_placeholder.simpa` trips the rule. The 23 valid fixtures give no error. Test (4) covers
    `Default` with α 0.1, with scattering only, and a rename.
  - Caveat: a material a user deliberately names `Default`, with α 0 and scattering 0 everywhere, is refused. That
    follows from the predicate's definition; the message says to choose a material.
- **C6, `RunEvent::Started`: holds.** It is sent right after the folder is made (`manager.rs:668`, `1077`), before
  `stage Solvers` (the C7 test asserts `["started", "stage Solvers"]`). A `run_project` that returns `Err` after
  `Started` (a `run.json` write that fails) leaves a folder that lists as Interrupted, which is honest. The CLI's
  `print_event` is an `if let`, so it needs no new arm.
- **C7, optional solver verification: holds for `run_project`.**
  - A mismatch refuses at `solvers` before geometry, with no mesh and no solve folder (the test, and the mutant
    above).
  - `check_solvers` fails closed: an executable the manifest does not list, or one that cannot be read, is a
    mismatch (`bed/pe.rs:260-295`).
  - **Byte identity:** all **460** `run.json` written by pre-M11 code (the M8a bed, smoke and cell runs) read with
    m11's `RunManifest` and rewrite **byte-identical**. None gains a `solvers` key. The CLI constructs
    `verify: None` (`mesh_run.rs:520`).
  - `run_folder`'s copy of the block is untested (finding 3).
- **C9, below-normal priority: holds, and reaches TetGen and preprocess.** Probe `prio`: children launched by
  `TetgenMesher::run` and `PreprocessProgram::run` each report `GetPriorityClass` = **16384**
  (`BELOW_NORMAL_PRIORITY_CLASS`), while the parent shell is `Normal`. Every product spawn goes through
  `process::run` → `winproc::JobTree::spawn`: `mesh/tetgen.rs:96` `run_logged` for both meshers, and
  `manager.rs:1588` for the solver. The only other `Command::new` in `src/` is test-only (`run/clock.rs:185`, inside
  `#[cfg(test)]`).
- **Results of failed or cancelled runs are refused in the core, not only in the UI: holds.**
  - `run_results` (`commands.rs:469-481`) → `runs::results_state` (`runs.rs:630-660`) → `results::load`, whose
    `check_status` (`results.rs:252-290`, unchanged by M11) refuses FAIL, CRASH and CANCELLED.
  - Probe `results`, on my own run folders: the FAIL run gives `results_run_failed`, and the three runs cancelled at
    export give `results_run_cancelled`.
  - Minor gap: see finding 4.
- **`runs_list` reads the manifests from disk on every call** (`runs.rs:562-617`): no cache, one `read_to_string`
  of each run folder's `run.json`.
  - Missing, and the active run: Running. Missing otherwise: Interrupted.
  - Present but not JSON: **FAIL** with `results_manifest_invalid`. This was a deliberate call (FOUNDATION.md F-7);
    finding 2 is the case it does not fit.
- **M8a bed.** `bed/run.rs` changed by one line (`verify: None`, 364).
  - The bed's `lib` unit tests pass (25, 1 pre-existing ignore).
  - The 464 bed projects still validate clean, and its 460 manifests round-trip byte-identical.
  - Priority: `RUN_TIME_LIMIT_MS` is 3 h against a 21 min longest run (`bed.rs:63-65`), so C9's lower priority
    leaves an 8.5× margin.

## Findings

### 1. BLOCKER: the CLI validates the active variant and exports another, so `material_placeholder` is bypassed and a run on upstream's placeholder is verdict OK and results-verified

- **Where:** `crates/simpa-core/src/run/manager.rs:705` validates `project` as read, which resolves materials under
  `project.active_variant` (`validate/project.rs:100` `p.active_material`, `:79` `materials_in_use`). `manager.rs:796`
  exports under the run's `variant` argument, where `None` means the base materials (`schema/model.rs:147-157`). The
  CLI passes `--variant` as given, and `None` when it is absent (`crates/simpa/src/mesh_run.rs:566`).
- **The setup:** `tests/fixtures/rooms/tutorial1_box_seeded.simpa`, with every group's base material set to
  `library_material(&REFERENCE_MATERIALS[0], ..)`. That is upstream's placeholder exactly as a PLY or `.proj` import
  leaves it (`is_placeholder_material` is true). A variant `real` puts each group back on its own material, and it is
  the active variant. This is an ordinary app save: the app has a `VariantSwitch`, and Run requires a saved file.
- **The CLI, HEAD's build** (`C:/tmp/nm-target/release/simpa.exe`, built 23:03, after the last `crates/` commit at
  21:44):

  | Step | Result |
  |---|---|
  | `simpa validate project.simpa` | exit 0 |
  | `simpa run project.simpa --solver tcr` (no `--variant`) | `OK - exit 0` |
  | the run's `solve/config.xml` | **81 of 81** `absorb="0"` (3 surfaces × 27 bands) |
  | `simpa results <run>` | exit 0; at 1000 Hz: **A 0.00 m², TR (Sabine) 37.933 s** |

- **The same file through `run_project` with the active variant**, as the app passes it (case B-app): absorb 0.1,
  0.2 and 0.3, and TR at 1000 Hz 0.667 s. The CLI's number is 57 times that, with verdict OK and `results::load`
  VERIFIED (case B).
- **The other direction:** probe case A has the base choosing every material, a variant `cli` that puts every group
  on the placeholder, and no active variant. `run_project(.., Some("cli"), ..)` passes Validate with 0 issues and
  writes `absorb="0"` everywhere. It was cancelled at export by the probe; nothing refused it.
- **Scope:**
  - The app is safe. It passes `info.active_variant`, the same variant the validator resolves, and case A-app is
    refused at Validate.
  - The bed is safe: no variants, and `variant: None`.
  - The mechanism predates M11 and applies to every material rule (for example `material_value_out_of_range`). M11's
    claim that C5 refuses the placeholder "on every path a Run takes" is false for the CLI.
- **Direction, a product call:** `run_project` should validate under the variant it exports. Resolve `variant` as
  `config_xml` does and set `project.active_variant` to it (`None` = base) before `validate_with`. Alternatively, or
  as well, the CLI could default `--variant` to the file's active variant, which is "what the GUI shows" in
  `config_xml.rs:107`. Add a `run_manager` test with `variant` different from `active_variant`, both ways.

### 2. MINOR: a torn `run.json` (the app killed while writing it) lists as FAIL, not Interrupted; foreign corrupt manifests are counted as this project's

- **Where:**
  - The core writes `run.json` non-atomically, truncating then writing (`manager.rs:515` `fs::write`).
  - `list` maps any parse failure to `unreadable` → `RunStatusUi::Fail` with `results_manifest_invalid`
    (`app/src-tauri/src/runs.rs:608,619-625`). It checks `active` only when the file is missing (`:589-598`).
- **Evidence:**
  - Probe `manifests`: a real `run.json` (3,614 bytes) cut at 0, 1,807 and 3,612 bytes does not read in any of the
    three cases (`unexpected end of input`, `expected ',' or '}'`).
  - The app's own test asserts `"{"` lists as `Fail` (`runs.rs:1643`).
- **Effect:**
  - An app killed inside that write window, or a `runs_list` racing the active run's write, shows FAIL for a run that
    was interrupted. F-7's reasoning ("an unreadable one is not a run that was interrupted") does not hold for this
    case.
  - A corrupt manifest cannot be checked with `belongs`, so another project's corrupt run is listed and numbered as
    this project's, which shifts the row numbers.
  - No wrong number: `results::load` refuses both.
  - The window is microseconds wide. Not reproduced with a real kill.

### 3. MINOR: `run_folder`'s verification block is untested, and has no cancel check after it

- **Where:** `manager.rs:1078-1083`.
- **Evidence:** `grep -rn "verify: Some\|verify = Some"` over `crates/` and `app/src-tauri/src` finds only
  `run_manager.rs:747,771`, which drive `run_project`, and `runs.rs:986`, the app's `run_project`. No caller or test
  gives `run_folder` a manifest, so no test fails if this block is removed or broken.
- **Also:** unlike `run_project` (`:677-679`), no `cancel.is_cancelled()` check follows the stage.
- **Effect:** no user path today; the app never calls `run_folder`.

### 4. MINOR: "results verified" does not mean "made by the verified build"

- **Where:** `crates/simpa-core/src/results.rs` never reads `RunManifest::solvers` (the only "solver" check is the
  commit, `:234`). `ResultsState.verified` is `results::load(..).is_ok()` (`runs.rs:645-648`).
- **Evidence:** probe `results` on the pre-M11 M8a run
  `atmospheric-validation/s1/20260929-131702-957-spps` returns VERIFIED with no `solvers` key.
- **Effect:** a CLI run made with `--solver-exe` pointing at an unverified build, sitting in the project's `runs/`,
  shows as verified in the Results step. The Runs row can say "not recorded". M11 shows no values, but M12 will.
  Decide whether `verified` should require `solvers` with every check matching.

### 5. MINOR: my scratch left behind, deletion refused by the no-delete guard at this hour

For after 07:00, all folders I created:
- `C:/tmp/nm-rc/`: test scratch kept by the failing mutants, probe runs, one CLI run.
- The scratchpad's `probe/` and `mut/`.
- In `C:/tmp/nm-target/debug/`: `review-core-probe.exe`, `prio_stub.exe`, and the `simpa-core` artifacts built from
  the `mut/` path.

## Not checked, and why

- **The M8a bed itself** was not re-run (about 6 h of SPPS) and not re-judged with `simpa bed --from`, which re-runs
  the roughly 26 min transport phase. The bed's changed surface was checked piece by piece instead: its manifests,
  its projects, its unit tests, and its time-limit margin under C9.
- **SPPS on the placeholder** was not launched. Finding 1 used TCR, which takes about 1 s. SPPS takes the same
  validate and export path.
- **A torn `run.json` from a real app kill** was not reproduced (finding 2 rests on the parse behaviour and the code
  path).
- **The app's own use of these contracts** at run time (Run, Cancel, close, kill) belongs to other reviewers' areas;
  no app was launched here.
