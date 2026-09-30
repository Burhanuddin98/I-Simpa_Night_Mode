# Backlog 38 and 39: one capped step

Started 2026-09-30 22:08 on Burhan's "begin" (21:57). Branch `b38-39` from `rebuild` at `2626d7d`,
worktree `.claude/worktrees/b38-39`. This file is committed before any code, and its bar does not move.

## The decisions being built

Made at 04:58 (`session-logs/NIGHT-2026-09-30.md:15`), after Burhan asked "WHY ARE THESE DECISIONS open?":

- **38:** "Results verified" only when the run's solver build was verified. Otherwise the results are
  marked unverified.
- **39:** `simpa run` without `--variant` uses the file's active variant, as the app and `simpa validate` do.

## What the code does today

- **38.** `results::load` (`crates/simpa-core/src/results.rs:363`) judges the run's status only
  (`check_status`, `:252`) and never reads `RunManifest::solvers` (`run/manifest.rs:149`,
  `Option<Vec<SolverCheck>>`; `SolverCheck` at `bed/pe.rs:248` carries `matches`). The app's verdict is
  `results::load(..).is_ok()` (`app/src-tauri/src/runs.rs:649, 663-678`), rendered by
  `app/ui/src/features/simulate/ResultsPanel.tsx:96,99` and `app/ui/src/actions.ts:121`. The CLI's
  `simpa results` maps `load` to exit codes 0/5/6 (`crates/simpa/src/results_cmd.rs:57`). Solver checks
  are written only when a run verifies its solvers (`run/manager.rs:585-604`, called at `:687` and
  `:1093`; test `run_manifest.rs:138`). Review receipt: `docs/investigations/2026-09-29-m11/review2/core.md`
  finding 4.
- **39.** `simpa run` passes `a.value("variant")`, which is `None` when the flag is absent
  (`crates/simpa/src/mesh_run.rs:566`), and `run_project` resolves `None` to the base
  (`run/manager.rs:630, 658-660`). `simpa validate` and the app use `project.active_variant`
  (`validate.rs:358`; `app/src-tauri/src/runs.rs:932`). Today's rule is stated in the doc comment on
  `run_project` (`run/manager.rs:625-629`) and in decision row 29 (`docs/decision-log.md:41`). Review
  receipt: `review2/core.md` finding 1.

## Constraints

- **C1.** `results::load`, `check_status` and `results::checked_report` keep their contract: the same Ok
  or Err for every input. The M8a bed reads its 433 pre-M11 runs, none with a solver record, through
  them (`bed/read.rs:319-321`). Refusing there would refuse the whole bed. The verdict is computed
  beside `load`, from the manifest.
- **C2.** Nothing under `crates/simpa-core/src/bed/` changes, so the M8a bed needs no re-judge and none
  is run.
- **C3.** 39 changes the CLI's default only. `run_project` and `run_folder` keep their signatures and
  semantics; the app and the bed call them directly and are unaffected. The `run_project` doc comment
  is rewritten to state the new rule.
- **C4.** The base stays reachable from the CLI when a file's active variant is set: through an
  existing spelling if `resolve_variant` has one, otherwise through `--base`, which cannot be combined
  with `--variant`. The help states the default and how to reach the base.
- **C5.** One predicate in simpa-core decides "solver build verified" from the manifest: `solvers` is
  present and non-empty, and every check `matches`. If the manifest names the solver the run executed
  and no check covers it, the build is unverified too. The app's Results verdict, the Runs row's solver
  status and the CLI's printed verdict all use this predicate. If the Runs row already has such logic,
  it moves to core instead of being duplicated.
- **C6.** "Unverified" marks a run; it does not refuse it. The Results step shows an unverified state
  with a reason code (row 29: codes, not prose) and never "Results verified". The CLI prints the verdict,
  in text and in `--json` if it has that, and its exit codes 0/5/6 do not change.
- **C7.** Builds and test scratch stay on C: (`CARGO_TARGET_DIR=C:\tmp\nm-target`,
  `SIMPA_TEST_SCRATCH_ROOT=C:\tmp\nm-target\test-scratch`). Stop if C: free space falls below 8 GB. No
  SPPS run beyond what an existing test already makes, and delete the output of any run started by hand.

## Tests, written first

Each test must fail on its assertion before the change. Each control passes both before and after.

| Id | Case | Before the change |
|---|---|---|
| T38-1 | A manifest with no `solvers` key: `results::load` is still Ok (pins C1), and the verdict is unverified with a reason code | fails |
| T38-2 | `solvers` recorded, one check with `matches: false`: unverified, mismatch code | fails |
| T38-3 | `solvers` recorded as an empty list: unverified | fails |
| T38-4 | `solvers` recorded, every check matching: verified | control |
| T38-5 | App boundary: `results_state` returns `verified: false` and the reason code for T38-1's run | fails |
| T38-6 | CLI: `simpa results` on T38-1's run prints the unverified verdict and still exits 0 | fails |
| T38-7 | UI: the Results step shows the unverified state and the reason code, not "Results verified", through the UI test harness if one exists, otherwise a new e2e id | fails |
| T39-1 | A file with an active variant, `simpa run` without `--variant`: the exported config.xml carries the active variant's materials | fails |
| T39-2 | An explicit `--variant X` wins over the active variant | control |
| T39-3 | The base spelling (C4) runs the base when an active variant is set | fails if the spelling is new |
| T39-4 | A file with no active variant: the default runs the base | control |
| T39-5 | The help states the default | fails |

A new function that a test needs in order to compile gets a stub that reproduces today's behaviour, so
the test fails on its assertion and not on compilation.

## Pass bar

The step stops if any of these is not met.

1. **RED.** Every test above fails on its assertion before the change, and every control passes. An agent
   other than the writer confirms this.
2. **GREEN.** All of them pass on the candidate. No assertion is weakened after RED unless the reviewer
   accepts the logged reason.
3. `cargo fmt --check` and clippy with `-D warnings` are clean, as the gates run them.
4. The gates pass on the candidate, in this order:
   - M11 (`tools/gates/m11.ps1`): every required e2e id (31, or 32 if T38-7 is an e2e id), 0 core
     failures, 0 focus steals;
   - M10 (`tools/gates/m10.ps1 -SolversDir C:\tmp\nm-m8a-solvers`);
   - M9 (`tools/gates/m9.ps1`).

   Measured on `c9edf1e`: 14m45s, 9m31s and 45s (`docs/investigations/2026-09-29-m11/GATE.md:42-44`).
5. The diff touches nothing under `crates/simpa-core/src/bed/`, and `load`, `check_status` and
   `checked_report` behave as before (C1, C2).
6. **One review.** Reviewers take distinct lenses, and a skeptic checks each finding. A finding blocks
   only with a concrete reproduction against the decisions or C1-C7. Blockers and majors that survive
   are fixed in at most one fix round, each with a test written first, and then checks 3 and 4 are run
   once more. Everything else goes to `docs/v1.1-backlog.md`. If a blocker survives the fix round,
   nothing merges and the step stops with a report.
7. After the gates, neither tree has new untracked files outside this folder, and C: free space is
   recorded.

## Caps

These are wall-clock limits. At a cap, the running workflow is stopped and its state reported.

| Step | Cap |
|---|---|
| 1. Tests written and RED verified | 75 min |
| 2. Implementation and gates | 120 min |
| 3. Review | 60 min |
| 4. Fix round and gate re-run, only if needed | 120 min |
| 5. Docs (backlog 38-39 closed, decision row 29 rewritten, row 32 added), merge to `rebuild`, push | 20 min |
| Total | 5 h from the start of step 1 |

Each step runs as its own workflow. A timer wakes the main session at each step's cap; otherwise the
workflow's end wakes it.

## Out of scope

- The M8a bed re-judge, which C2 makes unnecessary.
- Backlog 41-42 and the shelved branch `m8b-tamper`.
- The EDT held-out test.
- M12.
- Any change of style.
