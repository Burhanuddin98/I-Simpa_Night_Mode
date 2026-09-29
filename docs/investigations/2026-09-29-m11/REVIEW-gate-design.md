# M11 review: gate honesty and design fidelity

2026-09-29, Grace, branch `m11` at **`ca96476`** (GATE.md present). The review started after the
integrator finished. No code was committed or kept: every mutation was a temporary, uncommitted
edit, run through `tools/gates/m11.ps1` and reverted with `git checkout -- <file>`. After each
revert `git status --porcelain` was empty. After each run `tasklist` and CIM listed no `spps.exe`,
`app.exe`, `tauri-driver.exe` or `msedgedriver.exe`. The mutations were live in the worktree from
**22:20 to 23:03**. Any other run built from this worktree in that window built mutated code.

## Verdict

- **A. Gate honesty.** I made 48 mutations: 33 through the e2e specs and 15 through the static
  checks. **43 failed the check they targeted**, each with its own message. One (M06) passed
  its target, m11-c, but failed the core suite, which the full gate runs. **Four passed
  everything they should have failed**:
  1. **Major.** A hard-coded `Particles lost 0.00 %` passes. The problem is the fixtures, not
     the code.
  2. **Major.** m11-h does not look inside `[data-geometry]` or `[data-input]`, anywhere on the
     page. An acoustic number wrapped in either attribute passes m11-h and m11-sim-numbers.
  3. **Minor.** A progress readout that stops updating passes.
  4. **Minor.** A data-flow change that gives the test windows focus passes the focus lint and
     the app's unit tests. Only the live watcher would catch it, and that was not run live on
     purpose (see A.4).

  M18 is not counted: it was confounded by a chained failure and rerun as M19.
- **B. Design fidelity.** Region by region, the chrome matches Concept B exactly: sizes to the
  pixel, and the colour tokens as measured on the screenshots. Every deviation from the design
  in the three screens is listed below. Most are recorded product calls (decision-log rows 25
  and 26). One is a visual defect: the selected Runs row's red bar covers the run number. Four
  are small, unrecorded choices.

## A. Gate honesty: every check, its mutation, the result

The runs were `m11.ps1 -Only e2e -Spec <spec>` or `-Only static -SkipCore`, with
`-SolversDir C:\tmp\nm-m8a-solvers`, `CARGO_TARGET_DIR=C:/tmp/nm-target`, 16 jobs and
incremental off. Logs are in the session scratchpad. Each run's work folder is under
`C:\tmp\nm-target\gates\m11\2026092922*`/`230*`.

### A.1 The gate text's ids (gate, close, kill, after, smoke)

| # | Check | Mutation (file) | Run | Result |
|---|---|---|---|---|
| M01 | m11-a, "Particles lost 0.00 %" | the Runs row's loss span prints a literal `0.00 %` (`dock/RunsPane.tsx`) | gate,dock | **PASSED 13 of 13: the check cannot fail. See finding F1** |
| M02 | m11-a, the Console's counts equal run.json | the `[data-count]` of INFO shows +1 (`dock/ConsolePane.tsx`) | gate | FAIL: Console `INFO 3` against run.json `2` |
| M03 | m11-b-ipc, p99 < 100 ms | `ping` sleeps 150 ms (`commands.rs`) | gate | FAIL: p99 156.00 ms |
| M04 | m11-b-frames, p95 < 33 ms | each viewport render busy-waits 40 ms (`viewport/engine.ts`) | gate | FAIL: p95 50.00 ms |
| M05 | m11-c, Cancel | `RunSlot::cancel` returns true and never sets the token (`runs.rs`) | gate | FAIL: pid 15788 was still running at t0 + 2,010 ms. m11-h and r22-default then failed too, because the run stayed active |
| M06 | m11-c, "cancel skips the Job Object" | `kill_all` kills the direct child only (`simpa-core/process/winproc.rs`) | gate | **m11-c PASSES.** spps.exe has no children, so killing the child and ending the job look the same from outside. The **core suite catches it**: `process_job` had 3 FAILED (`cancel_from_another_thread_kills_the_grandchild`, `cancel_from_inside_on_line_on_the_first_line`, `the_tree_ends_with_its_root`). Honest at full-gate level. A `-SkipCore` run loses this guard |
| M07 | m11-d-kill | the job without `KILL_ON_JOB_CLOSE` (`winproc.rs`) | kill | FAIL: spps pid 13468 alive 2 s after Stop-Process. The core's `process_job` also had 2 FAILED (`killing_the_parent_kills_the_tree`, `a_panic_in_on_line_kills_the_tree`) |
| M08 | m11-d-close | `app_quit` skips `cancel_and_wait` (`commands.rs`) | close | FAIL: "the closed run has no run.json". The gate text's own clause, no spps.exe at 2 s, **still passed**: the Job Object ended the solver on exit. The spec's extra run.json assertion is what catches it |
| M09 | m11-d-after, m11-dock-interrupted | `runs::list` drops a folder with no run.json (`runs.rs`) | close,kill,after,dock | FAIL, both ids: "no Runs row …" |
| M10 | m11-e-row | `tetgen_skipped_facets` maps to `MESH_FAILED` (`runs.rs`) | gate | FAIL: no `[data-reason-code="MESH_TETGEN_SKIPPED"]` |
| M11 | m11-e-results | the refused panel reads "The run's 4 reasons" (`ResultsPanel.tsx`) | gate (with M10) | FAIL: "a digit on the Results step outside [data-run-label]" |
| M12 | m11-h (and m11-dock-h) | the solver time printed without its `data-diagnostic` span (`RunsPane.tsx`) | gate,dock | FAIL: rule 1, "solver time 1.4 s". dock-h also failed |
| M13 | m11-h, m11-sim-numbers | the Simulate step's last-run block shows "Reverberation time · Sabine 1.52 s" inside a `data-geometry` span (`SimulatePanel.tsx`) | gate,simulate | **PASSED 14 of 14: the check cannot see it. See finding F2** |
| M14 | m11-r22-default | the core's `placeholders()` returns early (`simpa-core/validate/project.rs`) | gate | FAIL: "0 material_placeholder issue(s)". The `data-blockers` still read MATERIALS_UNASSIGNED from the app's own count, so the issue count is what catches it |
| M15 | m11-smoke, and the harness's "private solver copy" check | one hex digit of spps.exe's `code_sha256` in `solvers/manifest.json` | smoke | FAIL, both: the harness says "DIFFERS"; smoke says "spps.exe NOT VERIFIED" |
| M34 | the harness's mesh-failure run, and "files left on B:" | `tetgen_skips.bat` stops writing the skipped `.face` file; a probe file was dropped into the investigation folder mid-run, then deleted | smoke | FAIL, both: the reasons lack `tetgen_skipped_facets`, and "files this run left in the repository (B:): 1" |

### A.2 The packages' ids (simulate, dock, project)

The simulate spec's tests are chained: they share a box run, and the solver radio's state
carries from one test to the next. My first combined simulate run (M16, M17, M18) cascaded.
M18 was confounded, because the failed TCR test left TCR selected, so I reran it alone as M19.
After that, only independent mutations were combined in one run, and each failure was matched
to its own message.

| # | Check | Mutation | Result |
|---|---|---|---|
| M16 | m11-sim-preflight | the materials row is never FAIL (`simulate/model.ts`) | FAIL: "the materials row fails, as text" |
| M17 | m11-sim-tcr | the Run label stays "Run SPPS" for TCR | FAIL: "the Run label did not become Run TCR" |
| M19 | m11-sim-running | the running head reads "Solving: " | FAIL: `'Solving: 0.02 %'` against `'Solving · 0.02 %'` |
| M20 | m11-sim-link | the "Run n" link no longer selects its run | FAIL: the Results step showed the control run |
| M21 | m11-sim-last-run | "Solver warnings" shows `lines.info` | FAIL: `'2' !== '0'` |
| M22 | m11-sim-numbers | the panel's sub gains "· 2 s per update" | FAIL: "2 s" outside [data-input] and the diagnostics |
| M23 | m11-sim-running, m11-dock-live, m11-h mid-run | the progress text is kept at its first value (`actions.ts`) | **PASSED 19 of 19 (gate, simulate, dock). See finding F3** |
| M24 | m11-dock-row | the opened record's OK count shows INFO | FAIL: "run.json lines OK" |
| M25 | m11-dock-meshfail | the Check column lists only the first 2 reasons | FAIL: "every reason is listed" |
| M26 | m11-dock-live | the Console never follows the bottom | FAIL: "did not follow the bottom during the run". dock-row and dock-h then failed too: the run was left active |
| M27-M33 | the 7 project ids, one mutation each, in one run | a9: Cancel on the prompt proceeds; a3: the app's `name_after_file` call removed; g42: `frameModel` does nothing; m26: the refusal line loses its "FAIL " label; m5: Lambert is set as specular; m1: the library value becomes `f64::from(f32)`; b18: a refused model shows its box volume | **All 7 FAIL, each with its own message**: "Cancel changed the project"; byte 84 `"New project"`; "Frame model did not restore S0"; the text no longer starts with `FAIL SOURCE_NONE`; `'specular'`; "0.30000001192092896 is not widen_f32's 0.3"; "Volume box 20595.2 m3" |

### A.3 The static and Rust checks

Two runs of `-Only static -SkipCore` covered these. Each mutation targeted a different check.

| Check | Mutation | Result |
|---|---|---|
| M10's static checks (these run M9's) | S-A's theme, backend and inventory changes; S-B's clippy and unit-test changes | FAIL in both runs |
| The command inventory (37) | `allow-app-quit` removed from `capabilities/default.json` | FAIL: capabilities 36 |
| Lint: the M11 commands only from actions.ts | `backend.runCancel` referenced in RunsPane | FAIL: "CALLS A RUN COMMAND" |
| Lint (m11-focus) | `.focused(focus)` becomes `.focused(!focus)` | FAIL |
| Lint (m11-focus) | `let focus = focus_on_open(&args) \|\| args.e2e;`: the test windows would take focus | **PASSED, and the app's unit tests PASSED too. See finding F4** |
| Lint: no PID-based kill | `let _probe = "taskkill";` in `RunSlot::cancel` | FAIL |
| Lint: the theme.css blob | a comment appended | FAIL |
| tsc | a string assigned to a number in `numbers.ts` | FAIL (TS2322) |
| npm test | `progressPct` drops the decimals | FAIL: 1 test |
| node --test app/e2e/lib | acoustic.ts rule 4 disabled | FAIL: 2 tests (rule 4 and the say-NO) |
| focus-watch.ps1 -CheckOnly | the probe is rooted at pid 4 | FAIL: `"root":4` |
| The app crate's unit tests | `loss_pct` prints `h % 10` | FAIL: 2 tests. The Rust formatter of the loss **is** guarded |
| clippy on the app / on the core | `len() == 0` in `bench.rs` / in `validate.rs` | FAIL / FAIL, each on its own file |
| rustfmt | `HashMap ;` | FAIL |
| The fixtures' recipe (ui_fixtures) | a byte of `box_run.simpa` | FAIL |
| The core crates' tests | see M06 and M07 (`process_job`, run alone rather than the 9-minute suite) | FAIL |

### A.4 What was not mutated, and why

- **m11-focus, live, and the unfocused-window assertion of m11-smoke.** Mutating these live
  means building a test window that takes the foreground. That takes Burhan's keyboard focus,
  which he asked the tests never to do (11:26). GATE.md made the same call for `-FocusSayNo`.
  The check was proven another way instead. **The current judge** (`focus.ts` and
  `focus-judge.ts` are unchanged since `f80c21b`, 21:28) was re-run on the real steal of the
  21:29 run (F-21, `C:\tmp\nm-target\gates\m11\20260929-212915\focus.jsonl`): **FAIL, 5
  FOCUS TAKEN**. On the passing 21:44 log it gives PASS, with 14 sessions.
- **The verdict's "skipped" branch.** An `it.skip` mutation was refused by the permission
  system. It was not retried by any other route. The missing-id and failure branches are
  exercised by every FAIL above: the verdict FAILED each time, with the id named.
- **Prerequisite checks.** Not mutated: the e2e lock, the build, tauri-driver, the
  msedgedriver version, the WebdriverIO install, the e2e typecheck, the gate-time inputs, and
  the full m9.ps1 as a prior gate. These fail on a missing tool or a compile error, and none of
  them can pass a wrong number.

### A.5 Findings

**F1, major: no fixture has a non-zero particle loss, so no check can tell a real loss from a
hard-coded `0.00`.** Every SPPS run in the gate loses 0 of 150,000 in every band. That is the
box, the long box, and the simulate and dock copies; the hall runs are cancelled and write no
statistics. So m11-a, m11-dock-row, m11-dock-h, m11-sim-last-run, m11-sim-numbers and m11-h
all compare `0.00` against a recomputed `0.00`.

This is exactly the regression this project has already paid for: arc item 12, a broken solve
in which about 99.997 % of particles died while every gate reported success. The Rust formatter
is guarded (A.3: `loss_pct` mutated makes 2 unit tests fail). The UI's display of it is not,
and neither is the Simulate panel's.

Fix, cheap, in the pattern of the Interrupted control: `m11.ps1` stages one synthetic run folder
whose `run.json` has a band with a known non-zero loss, for example 1,500 of 150,000 = `1.00`,
with the worst band not the first. m11-a (or m11-dock-row) then reads that row's
`[data-part="loss"]` and `[data-part="worst-band"]`, and m11-sim-last-run reads it in the
Simulate panel.

v1 filter: this is a gate that would let a wrong number reach a user, so v1. If it is deferred,
it goes in `docs/v1.1-backlog.md` with this receipt: M01, 13 of 13 passed.

**F2, major: m11-h's exemption for `[data-geometry]` and `[data-input]` covers the whole
page.** Rule 1 hides every such element, wherever it is. Rule 2 only matches the parameter
names on its list: `RT60`, `T60`, "Sabine", "Eyring" and "Reverberation time" are not on it.
M13 put "Reverberation time · Sabine 1.52 s" inside a `data-geometry` span in the Simulate
panel. It passed m11-h and m11-sim-numbers (14 of 14). M11's own code does not misuse the
attribute; I checked every use in `app/ui/src`.

This is the "loosening it globally" the no-acoustic-number rule forbids, inherited from M10 and
not scoped in M11. M12 is the milestone that adds acoustic numbers, so it matters most there.

Fix: give these two exemptions a region allow-list, as rule 3 (ii) has for diagnostics:
- `data-geometry` only in the Geometry panel's fact sections, the status bar's model fact, the
  viewport's dimensions and the Materials sub;
- `data-input` only in the Simulate settings, the Sources panel and the Materials grid.

An exempt element outside those regions is then a violation. Also add
`RT60|T60|Sabine|Eyring|reverberation` to `PARAMETER_NUMBER`, and plant a say-NO case for each
exemption.

**F3, minor: a frozen progress readout passes.** M23 kept the progress text at the first `#`
line. In the dock-live receipt, the live line read `#0.01` in all 8 samples while PROGRESS lines
kept arriving, 37 of them at the first sample and more at each one after. The sim-running
samples read `0.01, 0.01, 0.01, 0.01, 0.01`. The proof accepts any `#` line the run ever
printed, not the latest one.

Fix: m11-sim-running and m11-dock-live require the sampled value to change while the counted
PROGRESS lines grow. Better still, the value at each read should be one of the last few `#` lines
received by then.

**F4, minor: the static focus lint is a text match.**
`let focus = focus_on_open(&args) || args.e2e;` keeps both patterns the lint looks for, so it
passes the lint and the `focus_on_open` unit test. The windows would then take focus under
`--e2e`. Only the live watcher and m11-smoke's foreground check stand in the way, and those were
not exercised live here (A.4).

Fix: the lint requires the argument of `.focused(` to be exactly `focus_on_open(&args)` or a
binding assigned only from it. Or factor the builder input into a function the unit test calls.

**Notes, not defects:**
- Gate (c) and gate (d)'s literal clause, no spps.exe 2 s later, passes even when cancel skips
  the Job Object (M06) and when the close path skips its cancel (M08). The core's `process_job`
  tests and the close spec's run.json assertion are what catch them. Both run in the full gate.
  `-SkipCore` loses the first.
- m11-r22-default's `data-blockers` half cannot fail on its own, because the app counts
  placeholders itself (M14). The core-issue count and the `simpa run` exit 2 carry that id.

## B. Design fidelity: Simulate, Console, Runs

The screens are in `review-screens/`. They were taken by `m11.ps1 -Only e2e -Spec screens` at
23:04 on the clean tree: the built `ca96476`, 1440 × 900, which is the design's own size.

| File | What it shows |
|---|---|
| `1-simulate-mid-run.png` | The corrected hall solving at 1.03 % |
| `2-console-box-run.png` | The box after its run, on the Simulate step |
| `3-runs-box-run.png` | The row opened |
| `4-runs-mesh-failure.png` | The forced mesh failure's row |
| `5-results-refused.png` | The Results step for that run |
| `6-zoom-selected-row-bar.png` | A zoom of the selected row's bar, from screen 3 |
| `7-zoom-console-counts.png` | A zoom of the Console's counts strip, from screen 2 |

m11-focus passed on that run.

**Measured from the pixels, matching the design exactly:**
- **Sizes:** header 36 px, step bar 44 px, scene list 248 + 1 px, properties 344 + 1 px, dock
  250 + 1 px with a 34 + 1 px tab bar, footer 24 px.
- **Runs columns:** 70 / 170 / 90 / 90 px, measured at x = 265, 335, 505, 595 and 685.
- **Colours:** panels `#0F0F11`, main `#09090B`, rules `#1D1D21`. `#E0202E` on the Run
  buttons, the active step's underline and badge, the dock tab's underline and the selection.
  The SPPS radio's wash is `#1A0B0E`.
- **Brand rules:** no logo or wordmark. No serif and no italic: `fonts.css` has only
  `font-style: normal`. Every failure carries a FAIL text label: the Runs status, the Console
  tag, the preflight rows, and the Results step's "FAIL Results refused".

### Deviations, region by region

Decisions are cited as "row NN, <item>". **Defect** means fix; **unrecorded** means a small
choice nobody wrote down.

- **Header** (design:28-44): no deviation.
- **Step bar** (46-65):
  1. The Simulate sub reads `run 1 OK` where the design reads `run 3 valid`. Row 25, PQ5.
  2. A `+` button sits outside the variant control. M10.
- **Scene list** (69-102):
  3. Each source row has an on/off switch. Row 22, item M26.
  4. There is no "Audience grid" row. The feature is not built, and not in M11's scope.
- **Dock tab bar** (176-182):
  5. The caption "Live · Sabine and Eyring, 1/1 octave" is absent. It announces acoustic
     numbers, and M11 shows none: consistent.
  6. The Console badge is empty with 0 FAIL lines, where the design shows "1 fail". This
     follows the badge's logic: consistent.
- **Console** (247-257): the padding, the 12 px mono, the 14 px gap, the tag colours and the
  FAIL wash all match. The deviations:
  7. The tag column is 8ch, about 58 px, against the design's 40 px, so PROGRESS fits.
     Recorded in `dock.css`.
  8. PROGRESS and WARN classes are added, along with one live PROGRESS line with its note, and
     the per-run counts strip. T15, gate (a), row 26 D2.
  9. Solver and TetGen lines keep their own leading spaces, so the text column is ragged.
     Verbatim rule; deliberate.
  10. **Unrecorded.** The counts strip paints the word FAIL red and WARN amber even at count 0
      (`dock.css` `.count.FAIL .k`), so "FAIL 0" looks like a failure. Recommended default:
      colour the class word only when its count is above 0, and grey otherwise.
- **Runs** (259-274): the header row, the columns, the row type and the colours match. The
  deviations:
  11. The status words are OK, FAIL, Cancelled and Interrupted, against Valid and Refused. Row
      25, PQ5.
  12. The Check column reads "Particles lost 0.00 % / 1 % limit · 0 warnings · solver time
      1.5 s", against "no particles lost · 0 warnings". Row 25, PQ5.
  13. The opened record (per-band loss, hashes, line counts, meta) and the "Runs folder …"
      footer are additions. PLAN 3.3.
  14. The cells align to the first line of the Check column, not centred. GATE.md item 6.
  15. **Defect.** The selected row's 2 px red bar (`box-shadow: inset 2px 0 0`, with no left
      padding on the row) covers the first 2 px of the run number: the "#" of "#1" sits under
      it (`6-zoom-selected-row-bar.png`). Fix: `padding-left` of 2 to 4 px on `.run-row`, or
      the bar outside the cell.
  16. **Unrecorded.** A failed row's Check column shows the core's whole reason prose. For the
      mesh failure that is about 9 wrapped lines, with absolute paths and the file list, and it
      runs past the dock's 216 px body, so the row's end is below the fold (screens 4 and 5).
      The design gives a one-line reason ("stopped before meshing: 2 self-intersecting faces
      on the ceiling"). Row 26 D1 covers only details that quote units. Recommended default:
      one line per reason (UI code plus core code) in the Check column, with the prose in the
      opened record.
- **Properties, the Simulate step** (386-426): the head, the solver radio, the settings rows
  (30 px) and the idle block (the "Run n · variant" head with its status on the right,
  "Particles lost 0.00 % / 1 % limit", "Solver warnings", the 38 px red Run button) all match.
  The deviations:
  17. The settings show the project file's true values and the label "Particles per source and
      band 150,000", against "Particles per band: auto". Row 25 PQ3, row 26 P2.
  18. "Before running" shows an `OK`/`FAIL` text label per row where the design draws a green
      check. A sixth row, "Solvers are the verified build", is added. Row 26 P1.
  19. The running block adds "Elapsed m:ss" and a "Cancel run" button. Row 25 PQ2. The band
      ("Solving 250 Hz") is not shown. Row 26 P5.
  20. **Unrecorded.** The running head's percentage sits inline after "Solving ·". The design
      right-aligns it (`justify-content: space-between`, design:412). Recommended default:
      right-align it, as drawn.
- **Footer** (474-481): no deviation. It reads "Simulating · 1.03 %" in red during the run and
  "Ready" in green after.

The Results step (screen 5) shows no map and no receiver values. That is M11's rule; only M12
may show them.

## Files and space

| Where | What |
|---|---|
| **B:**, new and not committed | This file and `review-screens/`: 7 PNG, 1.2 MB. Nothing else was added. The probe file of M34 was made and deleted by me |
| **C:**, gate work folders | 25 of them, `C:\tmp\nm-target\gates\m11\20260929-222037` to `-230358`: 2,207 files, 168.7 MB |
| **C:**, the session scratchpad | 146 files, 4.3 MB: the logs, `mut.ps1`, the crops |
| **C:**, core test scratch | `C:\tmp\nm-target\test-scratch`: 4 files |

**C: free: 22.0 GB** at the end (22.2 GB at 22:18).
