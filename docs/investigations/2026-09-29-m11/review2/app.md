# M11 review 2: the app and UI

Adversarial review of `git diff d7ecbda...m11` (m11 at `2642cff`), area: the app and UI. No
product code was edited. 2026-09-30 00:06-00:35, Grace.

**Verdict: 4 majors, 4 minors, no blocker proven.** One major lost unsaved work in a real app
(R6), but only with the page's main thread busy longer than the gap between two close requests,
which no ordinary operation produced on Grace (R7). That is why it is graded major, not blocker.

| # | Sev | What | Evidence |
|---|---|---|---|
| 1 | major | A busy page is taken for a hung one: two close clicks close a dirty project with no prompt, and the edit is lost | R6, R5; controls R1, R2, R7 |
| 2 | major | PQ4 is held only in page memory. After a page reload mid-run there is no Cancel, Run is enabled, and New replaced the project while spps.exe ran | R4 |
| 3 | major | After File › New, or File › Open of a `.proj`, the Results step shows the previous project's run as "Results verified" | R10 |
| 4 | major | m11-h, m11-e-results and m11-sim-numbers read text only. Three components put the core's raw details, numbers and units included, into `title` tooltips. The dock withholds the same detail | code + `detailView` run |
| 5 | minor | A run.json limit of 0.07 is shown as "7.000000000000001 % limit". The m11-h judge accepts that and would reject "7 %" | R9 + judge run |
| 6 | minor | `name_after_file` knows only the English "New project". Upstream's default is translated | upstream source + `.po` |
| 7 | minor | m11-h's patterns pass "85 dBA", "1.8 sec", "1.8 seconds", "Clarity +2.1", "Definition 0.45" | pattern run |
| 8 | minor | Another project's run folder with no run.json is listed, and numbered, as this project's "Interrupted" run | code |

## How it was tested

- **Harness.** A scratch WebdriverIO harness in the session scratchpad,
  `C:\Users\Burhan\AppData\Local\Temp\claude\b--repos-I-Simpa-Night-Mode\2944fcd7-7038-44bd-9fc4-e0055382b531\scratchpad\e2e\`,
  holding `run.ps1`, `review.conf.mjs`, `lib.mjs`, `close-case.mjs` and `specs/r1..r10.e2e.mjs`.
  Receipts are in `out\receipts.txt`, with one wdio log per run. It uses m11.conf.ts's shape: `--e2e`, so
  the window opens visible and unfocused, and `excludeSwitches: ['enable-logging']`.
- **App under test.** The gates' release build, `C:\tmp\nm-target\release\app.exe`: sha256
  `A1AFFABF…97FD`, written 00:03:11 by the M9 gate run on this tree. `414ad5c`..`2642cff` changes
  only GATE.md.
- **Isolation.** Every run held `C:\tmp\nm-e2e\e2e.lock`. Before each run, no `app.exe`, `tauri-driver` or
  `msedgedriver` was running. `SIMPA_SOLVERS_DIR=C:\tmp\nm-m8a-solvers`. Projects were copied into the
  scratchpad on C:. After every run, nothing was left running: `left running: none` for app,
  tauri-driver, msedgedriver and spps.

## 1. Major: the hung-UI rule closes a busy, live page past the save prompt (A9)

`main.rs:142-179` `on_close_requested`: a close request unacknowledged for less than 5 s makes the next
one close the window (`let_through`, lines 151-160). The UI acknowledges only once its page runs the
channel's handler (`actions.ts:481-489`). A page whose main thread is busy therefore cannot
acknowledge before a second click. The docs claim the opposite: `main.rs:20-24` says "A live UI's
prompt is never skipped that way, however fast the close button is clicked again", and
`commands.rs:508-510` says the same.

Receipts (dirty project: one receiver renamed; two `WM_CLOSE` posted by `Process.CloseMainWindow`):

| Case | Page | Gap | Result |
|---|---|---|---|
| R1 | idle | 2.1 ms (1.27 → 3.38 ms) | alive, prompt "Teaching room", file unchanged |
| R2 | idle | 189 ms | alive, prompt, file unchanged |
| **R6** | busy 600 ms | 153 ms | **"app.exe EXITED 194 ms after the first WM_CLOSE, with no save; project file sha256 3d38961c3150189a (unchanged); the edit 'R1 edited' on disk: false"** |
| R5 | busy 3,000 ms | 1,008 ms | exited 1,046 ms after the first WM_CLOSE, edit not on disk |
| R7 | ordinary work on the hall (open, step changes, edits, undo) | – | no long task over 50 ms |

**Why the gate cannot catch it.** `m11.close.e2e.ts:55-59` sends the second `WM_CLOSE` only after
`waitPrompt`, so the acknowledgement has always landed by then.

**Severity.** The trigger needs a main-thread stall longer than the click gap, about 150 ms for a
double click. None was measured on Grace. Zeph, and larger models, were not measured.

**Fix direction.** Do not treat a request as unanswered until an acknowledgement deadline has
passed, for example the first request older than 1-2 s. Or have the page acknowledge from a
worker. Test with a page stall, as R6 does.

## 2. Major: PQ4 exists only in page memory; a reload mid-run orphans the run

**R4.** The long box, SPPS solving (spps pid 17780), then a page reload. After it:
- `runState null`; the Run button's `data-blockers ''`, enabled; Cancel shown `false`;
  `runs_list` row `RUNNING`; spps 17780 still running.
- Then New: the project went from 'Teaching room' (6 groups) to 'Untitled' (0 groups) while spps
  17780 ran.
- Cleanup: `WM_CLOSE` exited the app in 2,229 ms and left no spps.exe. No child process survived.

**Why.** `refuseDuringRun` reads `runStore` (`actions.ts:254-264`), which lives in page memory.
`scene_new`, `scene_open`, `model_import` and `proj_import` (`commands.rs:329, 339, 350, 485`) never
look at the run slot.

**Reachability.** WebView2's browser accelerator keys are on: wry-0.55.1 `src/lib.rs:1687`,
`browser_accelerator_keys: true, // This is WebView2's default behavior`. Tauri 2.11.6 never
changes it, and in WebView2 that setting covers "Ctrl+R and F5 for Reload". The app's key map
leaves both through:
- `App.tsx:68` returns before `preventDefault` whenever a text field has focus. So F5, the app's own
  Run key, reloads the page from the scene filter, a coordinate field or a materials cell editor.
- `App.tsx:88-96` never handles Ctrl+R, so Ctrl+R reloads it anywhere outside the materials grid.

**Not proven: a real key press.** It needs OS focus, which the test windows must not take. CDP key
events do not trigger WebView2's accelerators: R8, F5 in `<INPUT>` and Ctrl+R, page kept.

## 3. Major: the Results step shows a previous project's run as verified

**R10.** The box, with a real OK run copied from the gate's work folder, reads `verified`. Then:
- **After File › New:** "project 'Untitled' with 0 runs listed; Results step state 'verified' for run
  '20260929-234927-813-spps': '20260929-234927-813-spps Run · … OK Results verified Results verified:
  run.json, inputs and outputs re-checked…'"
- **After File › Open of upstream's `tutorial_1.proj`:** the same: 'tutorial_1', 0 runs, `verified`
  for the box's run.

**Why.**
- `newProject` (`actions.ts:108-115`) and `importProj` (`438-443`) reset neither `selectedRunStore`
  nor `resultsStore`. Only `openProject` resets both (121-122); `importModel` resets the selection only.
- `saveAs` (166-178) moves the runs root and keeps the selection.
- `resultsFor` (328-336) caches a verdict per run name for the whole session, so "re-checked" can
  be stale.

## 4. Major: the no-number checks cannot see tooltips; raw core details are put in them

**Where the raw detail goes.**
- `SimulatePanel.tsx:237` (the last run's reasons) and `:268` (its warnings): `title={r.detail}` and
  `title={w.detail}`.
- `ResultsPanel.tsx:22`: `title={r.detail}`. Its header comment, lines 7-9, says a detail "may hold
  numbers, so it goes in a title, never in the text (gate (e), m11-e-results)". That moves it out of
  the check's sight, not out of the user's.

**What the checks read.** `collectSnapshot` (`e2e/lib/acoustic.ts:156-201`) and m11-e-results
(`m11.gate.e2e.ts:300-309`) read only `textContent` and `innerText`. Nothing in a `title` can fail
them.

**What the details hold.**
- The verdict's `particle_loss_excess` quotes unproven four-decimal losses with units:
  `verdict.rs` `"{} Hz: {} of {} ({:.4} %)"` and "…lost more than {} % …".
- The dock withholds exactly that string. `detailView('1 band(s) lost more than 1 % … 500 Hz: 1600
  of 150000 (1.0667 %)')` returns `{"kind":"withheld"}` (run in Node), so the same text is hidden
  in the Runs tab and shown on hover in the Simulate and Results steps.
- On the Results step, a `results_value_invalid` refusal quotes a solver-computed value:
  `results/spps.rs:653-676` "`{what} value {i} is {x}, not a finite energy of at least 0`", with a
  negative energy (`tests/results_load.rs:678-694` plants -1.0). That is a solver-computed value on
  the step that "shows no value" (PLAN 3.4 rule 3).

**Not driven through the UI:** no fixture run carries either detail. The evidence is the code and
the `detailView` run.

## 5. Minor: the loss limit is printed from a float product, and the check mirrors it

`runs.rs:303-305`: `format!("{}", limit * 100.0)`. **R9**, a run.json with `loss_limit: 0.07`, which
is what `simpa run --loss-limit 0.07` records (`mesh_run.rs:99-106`). The app lists CLI runs (T1).
- Runs tab: "Particles lost 0.82 % at 500 Hz / 7.000000000000001 % limit".
- Simulate step: "Particles lost 0.82 % / 7.000000000000001 % limit".

The judge's `limitPct` (`e2e/lib/runs.ts:102-110`) does the same product. Run on these spans, it
returns `[]` for "7.000000000000001 %" and `3iv run.json gives '7.000000000000001 %'` for "7 %".
Node: 0.035 gives 3.5000000000000004, 0.29 gives 28.999999999999996, 0.14 gives 14.000000000000002.

## 6. Minor: `name_after_file` recognises only the English default name (A3)

`proj.rs:356-367` replaces a name only when it equals `"New project"`. Upstream stores
`wxGetTranslation("New project")` (`e_scene_projet_userconfiguration.h:50`), and the translation
table has "Nouveau projet" (`fr.po:1973-1974`), "Nowy projekt" (pl) and "Novo projeto" (pt_BR).
A `.proj` saved by a non-English upstream keeps that name, against the rule's stated intent.

**Odd file names.** `Path::file_stem`, compiled and run: `tutorial_1.proj` gives `tutorial_1`,
`.proj` gives `.proj`, `..proj` gives `.`, `a.b.c.proj` gives `a.b.c`, `CON.proj` gives `CON`,
`Salle é 房间.proj` gives `Salle é 房间`, `name .proj` gives `name ` (trailing space), and
`x.PROJ` gives `x`. These are cosmetic only: the project name is never validated and never reaches
a solver, since `validate/project.rs` `names()` checks sources and receivers only. The CLI and the
app pass the same path to the same function, so they agree.

## 7. Minor: m11-h's patterns miss common spellings

`ACOUSTIC_NUMBER` and `PARAMETER_NUMBER` (`e2e/lib/dom.ts`), run in Node, both answer false for
"85 dBA", "Leq 85 dBA", "1.8 sec", "reverberation 1.8 seconds", "Clarity +2.1", "Definition 0.45"
and "Level 85 dBZ". No such text is in M11's UI today (grep). A future one would pass m11-h, m10-h
and the dock's `detailView`.

## 8. Minor: other projects' interrupted or live runs are listed as this project's

`runs.rs:588-600` lists every run-named folder without a `run.json` as this project's
`Interrupted` row, numbered among the project's own runs. Without a `run.json` it cannot check
`belongs`. Two consequences follow:
- **Ownership.** T1 allows projects to share a folder, and PLAN 2.2 says other projects' runs are
  "counted, not listed". A killed run of project B is listed in A's Runs tab.
- **False status.** A run that another app instance or the CLI is solving right now reads
  "Interrupted · RESULTS_MANIFEST_MISSING".

## Checked, no finding

- **Commands.** All 9 new commands are `async fn`. In `commands.rs`: 37 async commands, 36
  `guard::blocking` calls; `panic_probe_unguarded` is M9's exception. `app_quit` calls `app.exit(0)`
  after the guard. Errors reach the UI typed: `CmdError {code, message}`, `asCmdError`, and stream
  `failed` events carry a `CmdError`.
- **Capabilities and network.** `capabilities/default.json` holds `dialog:allow-open/save/message`
  and `allow-<command>` only: no `shell:` or `fs:`. M11 changes no `Cargo.toml` or `package.json`, so
  no plugin was added. `tauri.conf.json`'s only change is `"create": false`, and the CSP is
  unchanged. No `http(s)`, `ws`, `fetch` or `WebSocket` in `ui/src` or `src-tauri/src`, apart from the
  JSON-schema `$schema` string in `bindings.rs`.
- **The save prompt itself.**
  - New, Open, a `.proj` and a mesh all go through `confirmDiscard`.
  - Save failing or Save as cancelled keeps the app open.
  - The prompt holds every key; its backdrop, z-index 20, is over the menu bar (5) and the dropdowns (10).
  - A close request with the prompt already open is ignored, not obeyed (R1, R2).
  - Close mid-run cancels the run and exits, leaving no spps.exe (R4 cleanup).
- **M5, M26, G42.** No defect found in the code read: the Law select issues one `replace_material`,
  a per-band law reads "per band", the source switch is `set_source_enabled` with its refusal
  inline, and the Frame button and the Home key call `frameModel`. m11-r22-g42 clicks the button
  only; the Home key path is untested.

## The m11-r22-m1 change in `acdf1e6`: independent, or a mirror of the code?

**Partly independent.**
- **Leg (a) can fail.** The asserted value, `0.3`, comes from the literal 0.3 through a separate JS
  implementation of "the shortest decimal that reads back to the same f32" (a `toPrecision` loop,
  `m11.project.e2e.ts:59-65`). It does not call `widen_f32`, and it would fail a core that widened
  with `f64::from`, which gives 0.30000001192092896.
- **Leg (b) cannot fail independently.** "Equals tutorial 1's `.proj` import" compares the core
  with itself: since `e25f6eb`, the importer and the library share `library_material`
  (`git log -S` on proj.rs).
- **The rest.** The colour and the law are compared with the library entry the code under test
  produced, not with upstream's `[178,178,178]`. The Rust unit test compares with `widen_f32` itself
  (`runs.rs:1339-1342`); only its `== 0.3` at 1347 is independent.
- **No solver input can change.** Node: for all 11 reference values, the widened value and the plain
  `f64` widening read back to the same f32, and SPPS reads an f32.
- **Conclusion.** The expectation was corrected, not bent: the old spec misread `widen_f32`. The
  commit's word "independently" holds for leg (a) only.

## Not verified

- **A physical F5 or Ctrl+R reload.** It needs OS focus; CDP keys do not trigger it (R8).
- **Stall lengths on Zeph or on models larger than the hall.**
- **The tooltip text through the UI.** No fixture run has `particle_loss_excess` or
  `results_value_invalid`.
- **A non-English `.proj`.** Only the translation table was read.
- **A9 at Windows logoff or shutdown** (`WM_QUERYENDSESSION`). Not examined.
- **The Law column's whole-material `replace_material` against an edit still in flight.** Whether
  it can overwrite that edit was not reproduced.
- **The core's semi-diffuse export refusal and the Job coverage of TetGen and preprocess.** Other
  reviewers' areas.
