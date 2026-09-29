# M11 gate: passed

2026-09-29, Grace, branch `m11` at **`acdf1e6`** (the integration commit; the only uncommitted
change during the run was `docs/scope.md`, text only). Built to `PLAN.md` on the foundation
(`34d5408` core, `e25f6eb` fixtures, `10baf3d` and `f7b1366` app and harness, `f80c21b` the gate's
specs and the focus watcher), the three packages committed as `f1eb1b2` (simulate), `436a81b`
(dock) and `d089ae3` (project), and the integration as `acdf1e6`.

| Run | When | Result |
|---|---|---|
| `powershell -File tools/gates/m11.ps1`, bare (Windows PowerShell 5.1: `pwsh` is not installed on Grace, although the gate text says `pwsh`), on `acdf1e6` | 21:44:20-21:57:50 | **M11 PASSED**, exit 0. 30 of 30 required e2e ids, 0 failures, 0 skipped; every static, Rust and harness check PASS; the prior gates inside it: `m10.ps1 -SkipCore` exit 0, `m9.ps1` **M9 PASSED** (23 PASS, 0 FAIL); `m11-focus` PASS over 14 app sessions |
| `powershell -File tools/gates/m10.ps1 -SolversDir C:\tmp\nm-m8a-solvers`, bare, in full, with `SIMPA_TETGEN160` set to the TetGen 1.6.0 reference build, on `7b20504` | 22:01:25-22:11:32 | **M10 PASSED**, exit 0. Every static check PASS; the core crates 72 test binaries, 762 passed, 0 failed, 30 ignored, in 509 s at 4 test threads; the 5 M10 spec files, every required id passed, 0 failures, 0 skipped |
| Earlier, partial: `m11.ps1 -Only e2e -Spec smoke,gate,close,kill,after` on the packages as committed | 21:29:15-21:30:30 | every e2e id passed; **`m11-focus` FAILED** (5 of 5 sessions): the WebView2 console window, F-21 below |
| `m11.ps1 -Only e2e` (all 8 spec files), with F-21 fixed | 21:33:58-21:35:45 | `m11-focus` PASS (0 foreground changes, 8 sessions); **`m11-r22-a3` and `m11-r22-m1` FAILED**, below |
| `m11.ps1 -Only e2e -Spec gate,project`, both fixed | 21:39:06-21:41:00 | all 15 ids passed; `m11-focus` PASS |

After every run, `tasklist` listed no `spps.exe`, `app.exe`, `tauri-driver.exe` or
`msedgedriver.exe`.

**`7b20504`** changes only `m10.ps1`'s core step (4 test threads, and `$SIMPA_TETGEN160` honoured)
and `docs/scope.md`. `m11.ps1` runs `m10.ps1` with `-SkipCore`, so the M11 pass on `acdf1e6` holds
for `7b20504`'s code unchanged. A first bare M10 run (21:58) was stopped by hand in its core step,
before any result: it ran the core tests at the default 28 threads, above the 4-solver-process
limit, and looked for the TetGen 1.6.0 reference beside `C:\tmp\nm-m8a-solvers`, where there is
none. `7b20504` fixes both, as `m11.ps1` already did.

## The gate text against the tests

The gate (`rebuild-plan-raw-2026-09-23.json`, plan.milestones, M11), verbatim, with the passing
run's receipts from `wdio.log` (work folder `C:\tmp\nm-target\gates\m11\20260929-214420`).

| Gate | Test id (spec) | Receipt |
|---|---|---|
| (a) Run the box from the UI. The Runs row shows status OK and the text 'Particles lost 0.00 %' | `m11-a` (gate) | A WebDriver click on `[data-part="run"]` on `box_run.simpa`; run `20260929-215505-489-spps` ended in 1,541 ms; the row's `data-status` and its status text are `OK`; `[data-part="loss"]` reads `Particles lost 0.00 %`, and Node's BigInt recomputation from `run.json` gives the worst band 125 Hz, 0 of 150,000: `0.00`. Control: the raw hall's `data-blockers` read `GEOMETRY_REFUSED MATERIALS_UNASSIGNED SOURCE_NONE` before the box's read empty |
| (a) The Console's per-class line counts equal those in the core's run manifest for the same folder | `m11-a` (gate) | Console `{PROGRESS 9999, INFO 2, OK 1, WARN 0, FAIL 0}` = the stream the app received = `run.json` `lines`; the stream's 10,002 events have `seq` 0 to 10,001, no repeat, no gap. Control: the counts are not trivial (`progress > 0`, `ok == 1`) |
| (b) During the corrected-hall solve, 200 IPC pings have p99 < 100 ms | `m11-b-ipc` (gate) | `hall_run.simpa` (built from `testdata/elmia_corrected.ply`, T13), during its solve: min 0.50, median 0.70, p95 1.00, **p99 2.00**, max 2.50 ms. Control: the run is still `running` and the private `spps.exe` alive after the pings |
| (b) a synthetic orbit drag has frame time p95 < 33 ms | `m11-b-frames` (gate) | A real WebDriver pointer drag (down, 60 moves of 4 px at 33 ms, up) during the same solve: 180 intervals, min 16.20, median 16.70, **p95 16.80**, p99 17.10, max 17.10 ms. Controls: the camera moved; the run still solving |
| (c) Cancel sets the row to 'Cancelled', and 2 s later tasklist lists no spps.exe | `m11-c` (gate) | A click on `[data-part="cancel-run"]`: before it, `spps.exe` pid 6848 ran from the gate's private copy; at t0 + 2,004 ms none did; machine-wide `spps.exe` count 0 (printed, not asserted: F8, T9). The row reads `CANCELLED` / `Cancelled`; `run.json` verdict `CANCELLED`, `cancelled: true`, `exit_code: null`, files present 0 of 16 expected (the solver stopped mid-run) |
| (d) Closing the window mid-solve ... leave[s] no spps.exe 2 s later | `m11-d-close` (close) | A real `WM_CLOSE` on a dirty project first: the save prompt, not a closed window; again at +271 ms (prompt open) and at +937 ms (after Cancel), the prompt each time and the app alive (F-11). Then saved, run `20260929-215521-844-spps`, `WM_CLOSE` mid-solve: `spps.exe` pid 5040 before, none at t0 + 2,008 ms; `app.exe` exited at 2,205 ms; `run.json` `CANCELLED` (the close path cancelled and waited) |
| (d) ... and separately Stop-Process on app.exe ... leave no spps.exe 2 s later | `m11-d-kill` (kill) | Run `20260929-215525-792-spps`, `Stop-Process -Force` on `app.exe` mid-solve: `spps.exe` pid 11336 before, none at t0 + 2,004 ms, `app.exe` dead, and no `run.json` (only the Job Object's `KILL_ON_JOB_CLOSE` can have ended the solver) |
| (e) A forced mesh-failure fixture shows a Runs row FAIL with reason MESH_TETGEN_SKIPPED | `m11-e-row` (gate) | The core's run manager with the stand-in TetGen (section 5, T8): exit 4, stage `mesh`, `FAIL`, reasons `tetgen_exit_nonzero, tetgen_skipped_facets, tetgen_output_missing, neigh_missing`. The row's `data-status` and text are `FAIL`, and its reason reads `MESH_TETGEN_SKIPPED (tetgen_skipped_facets)`. Control: the OK box row carried no reason code |
| (e) ... and the Results step displays no numbers for it | `m11-e-results` (gate) | `data-results-state="refused"`, `RESULTS_RUN_FAILED` shown, 0 `[data-result]`, no digit outside `[data-run-label]`. Control: the box run's panel reads `verified` |

## Checks added by PLAN.md, all passing

| Id | What | Receipt |
|---|---|---|
| `m11-smoke` | The unfocused, visible app runs the box end to end through the IPC contract | Foreground pid 2840 (VS Code) at launch and after the run, outside the app's process tree `22484,16612,4304,22372,15160,8096,7240`; window visible, not minimised, on a monitor; `document.hasFocus()` false; the four solvers found in the private copy, each verified |
| `m11-d-after` | A fresh session lists the killed run honestly | The killed run's row: `INTERRUPTED` / `Interrupted`, "RESULTS_MANIFEST_MISSING (results_manifest_missing): no run.json". Control: the closed run reads `Cancelled` |
| `m11-h` | No solver-computed acoustic number; every diagnostic and verbatim line proven | Five say-NO plants each flagged under its own rule (H1 rule 1, H2 rule 2, a wrong `elapsed_s` rule 3 (iv), a `loss_pct` in the Acoustics panel rule 3 (ii), a false verbatim line rule 4). **3 mid-run views** (the hall solving, the Simulate step, each dock tab), judged once the logs were complete: clean, `progress_pct` proven against the solver's `#` lines in the status bar, the Simulate sub, the Simulate panel, the Run button and the Runs tab (added at integration, below). Then **45 views** (box, hall, mesh failure × 5 steps × 3 dock tabs) clean: 101 diagnostic spans and 1,275 verbatim lines proven |
| `m11-r22-default` | The core refuses the `Default` placeholder at Run | The corrected hall with a source and a receiver: `data-blockers` exactly `MATERIALS_UNASSIGNED`, 10 `material_placeholder` issues; `simpa run` exit 2, stage `validate`, reason `material_placeholder`, 0 files under `solve/`. Control: all 10 groups assigned, blockers empty, `simpa validate` has no `material_placeholder` |
| `m11-r22-a9`, `-a3`, `-g42`, `-m26`, `-m5`, `-m1`, `m11-b18` | Row 22's items and B-18 (project spec) | a3: the app's File › Open of `tutorial_1.proj`, saved as, is **byte-identical** (6,491 B) to `simpa import-proj`; `bad.proj` a FAIL line with `IMPORT_INVALID`, project unchanged. m1: library 0.3 = `widen_f32` recomputed = tutorial 1's `.proj` import. g42: after Frame, position, direction and target 0 from S0. b18: raw hall "Volume none: the model is refused", dimensions `41.70 m, 30.30 m, 16.30 m`; corrected hall `10389.1 m³` against `simpa check` 10,389.096 |
| `m11-sim-*` (6), `m11-dock-*` (5) | The packages' own checks (PLAN 9.1, 9.2) | All passed; among them a TCR run of the box OK with no loss line, "Solving · p %" sampled 5 times during the hall solve and each proven against its `#` lines, the live PROGRESS line single and the Console following the bottom, every per-band loss equal to `run.json` in BigInt |
| `m11-focus` | The test windows stay visible and never take keyboard focus | Static lint PASS (nothing asks for focus, hides, minimises, moves or pins a window; the window built with `.focused(!(e2e \|\| selftest))`). The watcher over the whole run, M11's 8 spec files and M10's and M9's windows: **0 foreground changes in 169 s**, 642 samples, 14 app sessions, every app window visible, restored and on a monitor at every sample; one window hidden at 168,109 ms and gone within 1 s: a closing window (F-18) |

**Static and Rust** (the same run): M10's and M9's static checks PASS; the command inventory 37
in all four places; the lints (run commands only from `actions.ts`, the focus lint, no PID-based
kill, `theme.css` at M10's frozen blob); `npm run typecheck`; `npm test` 134 of 134 and the
checksum known answers 6 of 6; the harness libraries' `node --test` suites 36 of 36; the watcher's
`-CheckOnly`; `cargo test -p app` 51 passed; clippy `-D warnings` on the app and the core crates;
`cargo fmt --all --check`; the fixtures equal their recipe; **the core crates: 72 test binaries,
762 passed, 0 failed, 30 ignored, in 544 s**, 4 test threads, scratch on C:, no dev-tree staging,
the formerly hanging `mesh_project` and the `run::manager` unit test included. Not run, printed on
every run: the targets that write under `<repo>\target` (backlog 6).

## Checks the plan listed that the gate does not run as written, and why

- **`pwsh`** in the gate text: Grace has only Windows PowerShell 5.1; the script is written for it,
  as M10's was.
- **(c) "tasklist lists no spps.exe"** is checked on the gate's private copy of the verified solvers
  (no process runs from `<work>\solvers\spps.exe`), and the machine-wide `tasklist` count is
  printed beside it, not asserted (PLAN F8, T9): other sessions may run up to 4 solvers. It read 0
  in every run today.
- **The `-FocusSayNo` live proof was not run.** It opens a window of the gate's own that takes the
  foreground on purpose, which is what Burhan asked the tests never to do to him (11:26). The
  watcher's live ability to catch a steal was proven instead by a real one: in the first run it
  flagged the WebView2 console in 5 of 5 sessions (F-21), and after the fix it saw none. The
  judge's logic is proven by its 14 synthetic cases.
- **`m11-h` "after m11-a, after m11-c, after m11-e"** (PLAN 4.2) runs as one scan of the three
  projects after all of them, which reads the same pages; the integration added the mid-run read,
  which the plan's order could not give.
- **Occlusion** (PLAN 4.3, the first smoke): not a gate check, and not measured with the window
  deliberately covered. The frame and ping numbers above were taken with the test window wherever
  Windows placed it, unfocused, while Burhan used other windows.
- **`emulation.ts`** (PLAN 4.3): not built; no spec needed page-focus emulation (FOUNDATION F-10).

## What the first runs found, and what changed (commit `acdf1e6`)

1. **The test windows took focus, through a console (FOUNDATION F-21).** Every session's
   WebView2 browser allocated a console window (`ConsoleWindowClass`, titled with
   `msedgewebview2.exe`'s path), which Windows activated and which lived for the whole session: the
   driver adds `--enable-logging` to WebView2's switches. `m11-smoke` had passed it because it
   compared the foreground pid with `app.exe`'s alone. Both e2e configs now pass msedgedriver's own
   capabilities with `excludeSwitches: ['enable-logging']` (tauri-driver cannot carry it in
   `tauri:options`), and `m11-smoke` checks the app's whole process tree. The windows themselves
   are unchanged: visible, and built unfocused.
2. **`m11-r22-a3`: the app named an imported `.proj` "New project"**, the `.proj`'s own default,
   while `simpa import-proj` names it after the file, so the two saves differed at byte 84. The
   rule is now one core function, `geometry::import::name_after_file`, called by both, with a unit
   test.
3. **`m11-r22-m1`: the spec was wrong, not the code.** It expected `f64::from(0.3f32)`
   (0.30000001192092896); the core's `widen_f32` is the shortest decimal that reads back to the
   same f32, 0.3, which is also what a `.proj` import gives "30% absorbing". The spec now checks the
   library value against `widen_f32` recomputed in Node and against tutorial 1's import.
4. **Progress text in the step bar and the status bar** was SPPS's raw 4-significant-digit text,
   which below 10 % ("1.667e-05", "0.0006667") breaks the `progress_pct` grammar. Both now show
   the simulate package's `progressDisplay`, in diagnostic spans, and `m11-h`'s mid-run read
   proves them.
5. **The Semi-diffuse note** said SPPS reflects it specularly; this app's core refuses law 6 at
   export (`export_failed`), and the note now says that. Blocking Run before meshing is backlog 22.
6. **Runs rows** align their cells to the first line of the Check column, so a failed run's long
   reasons no longer push its number and status out of view.

## Decisions and deferrals

Decision-log rows 24-27 record M11's calls: the plan's T1-T19, the product questions PQ1-PQ7
with the defaults built, the packages' user-visible calls with their defaults (simulate P1-P7,
dock D1-D2, project P1), and the technical calls, F-21 among them. Backlog rows 19-22 hold what
was deferred: the Law column's keyboard path, per-band law editing, the import dialog's key
blocking, and a semi-diffuse material blocking Run before meshing.

## Still open (not deferred past v1)

- A leftover `msedgedriver` after the close and kill specs (the foundation's open item 4): not
  seen. The after spec, and every spec after it, started cleanly in every run, and `tasklist`
  showed no driver after any run.
- M11 is not merged into `rebuild`; that is Burhan's push.

## Screens

`screens/` (5 PNG, taken by `m11.ps1 -Only e2e -Spec screens` at 22:11:50 on the build of
`7b20504`; that run's `m11-focus` also PASS): `1-simulate-mid-run.png` (the hall solving: the running block, the step bar's
and status bar's percentage, the Console's live PROGRESS line), `2-console-box-run.png`,
`3-runs-box-run.png` (the row opened: per-band loss, hashes, verified solvers, line counts),
`4-runs-mesh-failure.png`, `5-results-refused.png`.

## Files

- **On C:**, the full run's work folder `C:\tmp\nm-target\gates\m11\20260929-214420`: 321 files,
  17.1 MB. Every M11 run today has its own folder beside it. C: free space at the end: 23 GB (24.4 GB at the start of the session).
- **On B:**, the gate left 0 files in the repository. Added by this integration: this file, the 5
  screenshots, and `app/e2e/specs/m11.screens.e2e.ts`.
- **Session record.** This page is the integration session's summary (21:25-22:15). No separate
  file was written under `session-logs/`: the integrating agent's harness refuses summary files,
  so the live handoff, `session-logs/HANDOFF-2026-09-29.md`, predates M11's integration and this
  page supersedes it for M11's state.
