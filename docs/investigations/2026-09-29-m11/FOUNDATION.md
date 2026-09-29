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
(`m11-smoke`), and `tools/gates/m11.ps1`, which runs every step of PLAN.md 4.4 and reports a
missing spec file as pending (its ids fail).

**The gate's specs and libraries** (the second foundation session, 21:00-21:25):

| File | What it holds |
|---|---|
| `specs/m11.gate.e2e.ts` | `m11-a`, `m11-b-ipc`, `m11-b-frames`, `m11-c`, `m11-e-row`, `m11-e-results`, `m11-h`, `m11-r22-default`, in that order, one session. Written against PLAN.md 3.6's DOM; the packages make it true |
| `specs/m11.close.e2e.ts`, `m11.kill.e2e.ts`, `m11.after.e2e.ts` | `m11-d-close`, `m11-d-kill`, `m11-d-after`, each its own session; the close and kill runs reach the after spec through `<work>\d-runs.json` |
| `lib/runs.ts` (tested) | `run.json` and the logs read in Node; the loss, limit and elapsed rules of PLAN.md 2.3 in BigInt or the same IEEE operations as `runs.rs`; exact decimals |
| `lib/acoustic.ts` (tested) | `m11-h`'s checker: `collectSnapshot` runs in the page (one read, an optional say-NO plant removed before it returns); `judge` holds the snapshot to rules 1-4 of PLAN.md 4.2 with each run's `run.json` and logs; the five say-NO cases |
| `lib/focus.ts` (tested), `lib/focus-judge.ts` | `m11-focus`'s judge and its command line (`--min-sessions`, `--expect-steal`) |
| `lib/stats.ts` (tested), `lib/m11.ts` | nearest-rank percentiles; the typed run hooks, `waitRun` in slices, the Runs row and Console count reads, the gate's environment |
| `tools/gates/focus-watch.ps1` | the watcher of PLAN.md 4.2 item 2: C# through `Add-Type`, a WinEvent foreground hook, low-level mouse and keyboard hooks (times, points, injected flags; no key code), a 250 ms window sample of the gate's `app.exe`s; `-CheckOnly` compiles and probes with no hook |
| `ui/src/testhooks.ts` | one hook added: `issues()` (UI code, core rule, path, severity) |

`m11.ps1` now also: runs the harness libraries' suites and the watcher's `-CheckOnly` in the static
step; makes `<work>\t1_cli.simpa` (`simpa import-proj`) and an 8-byte `<work>\bad.proj`; passes
`M11_GATEWORK`, `M11_SIMPA`, `M11_ELMIA_RAW`, `M11_T1_PROJ`, `M11_T1_CLI`, `M11_BAD_PROJ`; starts the
watcher before the first test window and judges it after the prior gates (`--min-sessions` = the
M11 spec files run); and with `-FocusSayNo` first proves the judge flags a window of its own that
calls `Activate()`.

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
| F-11 | **A defect in the close contract, fixed: the save prompt could be skipped.** `CloseState.requested` was cleared only by `app_quit`, so after Cancel on the prompt, or on a second click of the close button while the prompt was open, a close request within 5 s was taken for a hung UI and closed the window without the prompt, losing unsaved work. Now the UI acknowledges each close request at once by registering a fresh channel (`app_events`, which clears `requested`); a hung UI acknowledges nothing and still closes on the second request. No command or signature changed, so the bindings and the inventory stand | A9 is a feature Burhan asked for (row 22), and a double click on the close button is ordinary. The channel is fresh because re-sending one restarts its message index on the Rust side (`@tauri-apps/api` `Channel`), which stalls it. `m11-d-close` now sends `WM_CLOSE` again with the prompt open and just after Cancel, both inside the 5 s, and requires the prompt each time |
| F-12 | **No package spec shells.** | By 21:14 the three packages had written `m11.simulate`, `m11.dock` and `m11.project` themselves; a shell would have overwritten a package's file. `m11.ps1` already reports a missing spec file as pending and fails its ids, which is what a shell was for |
| F-13 | `m11-h` rule 1 hides every `[data-diagnostic]` and `[data-verbatim]`, not only the proven ones | The same verdict: an unproven one is a violation of rule 3 or 4 by itself |
| F-14 | A verbatim line of source `mesh` is proven against every `*.stdout.txt` and `*.stderr.txt` under the run's `mesh/` (TetGen, its `diag/` follow-up, `preprocess.exe`); one of source `solver` against the solver's two logs only | All three mesher programs stream as `MeshLine` (`manager.rs`), so all are TetGen-side lines; PLAN.md 4.2 names only `mesh/tetgen.*`. Keying the logs by source is tighter than "a line of any of the four" |
| F-15 | A `progress_pct` span keeps the plan's grammar and is proven at its displayed digits, in exact decimals (half up), or as 100 once SPPS printed `End of calculation.` | SPPS's `#` values are multiples of 0.01 (the box: 9,999 lines; the hall at `#25.22`: 2,522 lines, PROVENANCE.md), which the grammar fits |
| F-16 | A diagnostic must be a leaf element, checked under (ii) | PLAN.md 3.4 rule 1 says "leaf span"; a span holding elements could carry other text that rule 1 would never see |
| F-17 | Each say-NO plant must be flagged **under its own rule** (H1 rule 1, H2 rule 2, the elapsed span (iv), the Acoustics loss span (ii), the verbatim line rule 4). The elapsed plant reads 1.8 s, or 2.8 s when the manifest says exactly 1.8 s | Stronger than "flagged at all": a plant caught by the wrong rule would hide a blind rule. The plant must differ from the manifest to be a lie |
| F-18 | The focus judge excuses an invisible sample only when that window is gone from a sample within 1,000 ms; a minimised or off-monitor sample is never excused. A session is an `app.exe` with a titled window that is not an IME window | `DestroyWindow` hides a window before destroying it, and every session ends with its window closed. Every GUI thread has a hidden "Default IME" window |
| F-19 | The watcher is its own `powershell` with `CreateNoWindow`, rooted at the gate's pid; it stops on `<log>.stop` or when the gate's process is gone. A "child" created before its parent is a reused pid and not counted | No window, so starting it takes no focus; no hook outlives the gate even if `m11.ps1` dies. The pid check keeps an unrelated process out of the gate's tree |
| F-20 | The specs wait for a run in slices of at most 20 s, and raise WebDriver's script timeout to 120 s | The default script timeout is 30 s; meshing the hall, or `idle()`, may take longer than one call may |

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

| Second session (21:00-21:25), no app, no e2e, no solver launched | Result |
|---|---|
| `node --test app/e2e/lib/*.test.ts` | 36 of 36 passed (runs 6, acoustic 13, focus 14, stats 3) |
| `tsc -p app/e2e/tsconfig.json` against the pinned WebdriverIO | clean, every spec included (the packages' three too) |
| `npm run typecheck`; `npm test` | clean; 134 of 134 |
| `cargo fmt --all --check`; `cargo clippy -p app --no-deps --all-targets -D warnings` after F-11 | clean; the app crate re-checked (`Checking app`) |
| tsx's transform of `collectSnapshot` (what WebdriverIO serialises into the page) | no `__name` helper; its source parses as a standalone function |
| `focus-watch.ps1 -CheckOnly` | compiles; a read-only probe of Explorer's windows (`-AppName explorer.exe`): 48 processes, 39 windows sampled |
| A 2.4 s live run of the watcher (rooted at a PowerShell of this session; its log in the session's scratchpad, deleted after) | hooks installed (foreground, mouse, keyboard), 9 samples, stopped on its stop file, exit 0; `focus-judge.ts`: PASS |
| Windows PowerShell 5.1 parser on `m11.ps1`, `focus-watch.ps1`, `m10.ps1` | 0 errors |

## Not done here (the next foundation steps, not deferred past v1)

- **The first run of the gate's specs:** `m11.ps1 -Spec smoke,gate,close,kill,after`, and its
  `m11-focus` judgement, with `-FocusSayNo` once. This session was not to launch the app. Until
  the packages land, the ids whose DOM is theirs fail (`m11-a`, `m11-c`, `m11-e-row`,
  `m11-e-results`, `m11-h`, `m11-d-after`); the foundation-only ids are `m11-smoke`, `m11-b-ipc`,
  `m11-b-frames`, `m11-d-close`, `m11-d-kill` and `m11-r22-default`.
- `emulation.ts`: not needed (F-10).
- The packages: simulate, dock, project (PLAN.md 9.1 to 9.3).
