# M11 foundation: what the packages build on

2026-09-29, branch `m11`. Built to `PLAN.md` section 9.0, as far as this session's task took it
(the core, the fixtures, the app's IPC, the UI's shared pieces and slots, and the harness with one
smoke spec). This page lists what exists, where it departs from the plan, the calls made on the
way, and the receipts. Package owners read "What a package gets" first.

## What a package gets

**Core** (commits `34d5408`, `e25f6eb`): C1 to C9 of PLAN.md 2.9, each with its test.
- `validate::is_placeholder_material` is the one placeholder predicate; `material_placeholder` is a
  project error (43 rules), refused at the validate stage, exit class 2.
- `RunEvent::Started(&Path)` is the first event of every run that gets a folder.
- `RunOptions::verify` checks the executables first (stage `solvers`, `solver_unverified`, exit
  class 2) and records them in `run.json`'s `solvers` key, absent when not asked.
- `mesh::codes::ALL`; children at below-normal priority; the mesher's panic no longer held.
- `geometry::import::library_material` builds a reference material, for the `.proj` import and the
  app's library alike; `reference_material` is re-exported.

**Commands** (`app/src-tauri/src/commands.rs`, `runs.rs`): the nine of PLAN.md 2.2, each async and
through `guard::blocking`, registered in all four places: 37 commands. `StartupInfo.pid`.
`CheckSummary.enclosed_volume_m3` is `number | null` (null when the check refuses the model).
`SceneState.run_blockers` no longer holds `M11_PENDING`; the core's `material_placeholder` has the
UI code `MATERIALS_UNASSIGNED` and is listed once.

**The window** (`main.rs`, `tauri.conf.json`): the config declares it `"create": false`; `setup`
builds it with `.focused(!(e2e || selftest))`. A close request is held and sent to the UI once
`app_events` has registered its channel; `app_quit` cancels a run, waits up to 3 s, exits.

**UI modules** (`app/ui/src/`):

| Module | What it holds |
|---|---|
| `store.ts` | PLAN.md 3.1's stores: `runStore`, `runLinesStore` (`RunLog`: counts, seqs, dupes, gaps, lines), `runsStore`, `selectedRunStore`, `resultsStore`, `solverStore`, `solversStatusStore`, `promptStore`, `libraryStore`. `ConsoleLine` gains `source`, `run`, `verbatim` and `parts` (a diagnostic part is `{text, diagnostic, run}`, to render as a leaf `data-diagnostic` span) |
| `flow.ts` (tested) | `joinBlockers` (project, solvers, `RUN_ACTIVE`), `needsSavePrompt`, `foldEvent` (the stream fold), `endLine` (the app's line for an ended run, numbers in diagnostic parts), `statusWord`, `progressText` |
| `actions.ts` | `runStart` (saves first, PQ1), `runCancel`, `refreshRuns`, `selectRun`, `resultsFor`, `refreshSolvers`, `loadLibrary`, `confirmDiscard` (A9), `importProj`, `addFromLibrary`, `setSourceEnabled`, `setLaw`, `listenAppEvents`; New, Open and `openPath` go through the prompt and wait while a run is active (PQ4); `openPath` opens `.proj` |
| `ops.ts` (tested) | `setSourceEnabled`, `replaceMaterial`, `withLaw`, `libraryMaterial` |
| `testhooks.ts` | PLAN.md 3.5's hooks, plus `openPath` and `newProject` (the prompt's paths); `runBlockers` now returns the joined list the Run button shows |

**Slots.** `App.tsx` boots `listenAppEvents`, `refreshSolvers`, `loadLibrary`, `refreshRuns`, and
maps F5 (Run, only when nothing blocks it) and Home (Frame model). No slot takes props.

| Package | Slot (stub or moved code) |
|---|---|
| simulate | `features/simulate/SimulatePanel.tsx`, `ResultsPanel.tsx` (stubs), `model.ts` (`simulateSub` returns `''`), `simulate.css` (the Run button's rules, moved), `chrome/RunButton.tsx` (minimal: joined blockers, enabled when none, a click runs `solverStore`'s solver) |
| dock | `features/dock/Dock.tsx`, `ConsolePane.tsx`, `RunsPane.tsx` and `dock.css`, moved unchanged from `chrome/` |
| project | `chrome/SavePrompt.tsx` (a minimal working dialog: `[data-prompt]`, `[data-choice=save\|discard\|cancel]`), `chrome.css` without the dock's and Run's rules |

The status bar reads "Simulating · <p> %" during a run, the percentage in a
`data-diagnostic="progress_pct"` span, SPPS's own text after its `#`.

**Harness**: `app/e2e/m11.conf.ts`, `lib/procs.ts` (foreground and window state, processes by
path, `WM_CLOSE`, Stop-Process; nothing moves or activates a window), `specs/m11.smoke.e2e.ts`
(`m11-smoke`), and `tools/gates/m11.ps1`, a skeleton that runs every step of PLAN.md 4.4 but the
focus watcher, and reports missing spec files as pending.

## Where this departs from PLAN.md, and the calls made (row 13)

| # | Call | Why |
|---|---|---|
| F-1 | **PQ7 (product question, default built): a project never saved and never edited does not prompt.** `needsSavePrompt` asks when the project is dirty and has a file, or has been edited (undo depth > 0). A new empty project, or a model just imported, leaves without a prompt | Nothing of the user's is lost: the source file is untouched. It also keeps M10's scene spec passing unedited, which PLAN.md 9.3 requires: its "step subs" test imports the raw hall and chooses File › New, and a prompt there failed it and the four tests after it (measured, 20:26) |
| F-2 | `runBlockers` (the hook) returns the joined list, not the project's alone | `m10-a-run` compares the DOM's `data-blockers` with the hook; since the button shows the joined list (PLAN.md 3.3), the hook must too, or the two differ whenever a solver blocker is present |
| F-3 | `m10.ps1`'s e2e passes `SIMPA_SOLVERS_DIR` to the app (`-SolversDir`, else the repo's build, else M10's copy) | The teaching room's control now reads Run enabled (PLAN.md 4.5), which needs the verified solvers found |
| F-4 | The Run button's foundation version runs a click | So `m10-a-run`'s control and `m11-smoke` hold before the simulate package lands; the package replaces the label |
| F-5 | `RunStreamEvent::Ended` carries `Box<RunRow>` | clippy's `large_enum_variant`; the JSON is the same |
| F-6 | A band with no particles has lost none (`0.00`) | 0 of 0; such a run fails `particle_total_short` anyway, shown in its reasons |
| F-7 | An unreadable `run.json` lists as FAIL with `results_manifest_invalid` and `manifest_error` | PLAN.md 2.2 names only the missing case (Interrupted); an unreadable one is not a run that was interrupted |
| F-8 | `model_import` opens a `.proj` whatever unit was chosen, besides the `proj_import` command | A `.proj` carries its own units; the task names ".proj open in model_import" |
| F-9 | `m11.ps1` requires `-TargetDir` off the repository's drive, and fails on any file a run leaves untracked in the repository | The exFAT rules, made checks |
| F-10 | F9 (page focus): **no emulation is built.** Measured: with the window unfocused (`document.hasFocus()` false), all 27 of M10's e2e tests pass, `m10-f`'s 50 Ctrl+Z and the materials grid's focused cell included | T11 applies only if a spec fails on page focus; none did |

## Receipts (2026-09-29, Grace)

| Check | Result |
|---|---|
| C1's test on the unfixed core | **FAILED, held 120.01 s**; fixed: passes in under 1 s |
| `mesh_project` (the formerly hanging `every_failure_code_fires_on_its_input` included), scratch on C: | 18 passed, 1.05 s |
| `run_manager`, `run_manifest`, `run_verdict`, `run_contract_docs`, `reason_codes_docs`, `validate_projects`, `validate_contract_docs`, `process_job`, lib | all pass |
| `cli_run`'s two tests, target on C:, **no** `target\solvers\bin` staged | 2 passed |
| `cargo test -p app` | 51 passed (41 in M10), the stream order, a panicking run, every kind of Runs row, the UI-code table, the integer formulas, run names and the library among them; `cancel_ends_the_run_through_its_token` ends a fake solver's 30 s `ping` grandchild through `RunSlot::cancel` in under a second |
| clippy `-D warnings` (app; simpa-core and simpa, all targets), `cargo fmt --all --check` | clean |
| `npm run typecheck`; `npm test` | clean; 93 of 93 |
| `m11.ps1 -Only static -SkipCore` | every check passed, M10's and M9's static checks included |
| `m11.ps1 -Only e2e -Spec smoke` (20:24) | `m11-smoke` passed. Receipts: foreground pid 3180, app pid 22628, the window visible, not minimised, on a monitor, at launch and after the run; `document.hasFocus()` false; the four solvers found in the gate's private copy, each verified; the box run `20260929-202553-613-spps` ended OK in 1.6 s; run.json's lines progress 9,999, info 2, ok 1, warn 0, fail 0 equal the streamed counts, n 10,002, seq 0 to 10,001, no dupe, no gap; `run.json` records spps, tetgen and preprocess verified; the results verify |
| `m10.ps1 -Only e2e` (20:29), the window unfocused | 27 of 27 passed, the 13 required ids included, 0 failures |
| The mesh-failure run by the core with `tetgen_skips.bat` | exit 4, stage mesh, FAIL, `tetgen_exit_nonzero, tetgen_skipped_facets, tetgen_output_missing, neigh_missing` |

| `m11.ps1 -Only static` with the core crates' suite (20:41-20:49), 4 test threads, scratch on C:, no dev-tree staging, only the `<repo>\target` writers left out | **761 passed, 0 failed, 30 ignored** in 72 binaries, 499 s. The first full run (20:30) had 3 failures, both causes fixed: the two TetGen 1.6.0 reference tests had no `SIMPA_TETGEN160` (m11.ps1 now passes it, `-Tetgen160`), and `validate_fixtures` requires a negative fixture per project rule (`material_placeholder.simpa` added through its generator) |
| `m9.ps1 -TargetDir C:\tmp\nm-target`, in full (20:49) | **M9 PASSED**, (e) the bindings regenerating to the committed blobs and the unfocused self-test window included |

## Not done here (the next foundation steps, not deferred past v1)

- The gate's specs: `m11.gate` (a, b, c, e, h, r22-default), `close`, `kill`, `after`; the
  package spec shells.
- `m11-h`'s checker (`lib/acoustic.ts` and its suite), `runs.ts` (the BigInt formulas in Node),
  and `emulation.ts` (not needed, F-10).
- The focus watcher (`focus-watch.ps1`, `lib/focus.ts` and its suite, `-FocusSayNo`).
- The packages: simulate, dock, project (PLAN.md 9.1 to 9.3).
