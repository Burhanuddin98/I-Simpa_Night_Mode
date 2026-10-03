# M12 P4 build: integration, and gate (b) made to prove something (2026-10-03)

Branch `m12` (worktree `.claude/worktrees/m12`) at `e8acdc5` (P1.1: all eleven parameters PASS, T30 by decision
46), with `m12-viewport` @ `7217bde` (P3) and `m12-acoustics` @ `95873cc` (P2) merged in. Spec: `PLAN.md` beside
this file, P4. Receipts (red and green logs, the suite, the gate logs, the progress log): `B:\data\m12\p4\`.

## What it means

**The three packages are one branch, and gate (b) now shows that a parameter without a PASS is hidden, not just
that nothing happened to need hiding.** After P1.1 every parameter in `beds/summary.json` is PASS, so P2's m12-b
compared an empty list and passed whatever the screen did. It is now two ids:

- **m12-b** (real summary): every parameter is rendered, each cell with its range and status or its refusal code;
  T30 has 81 cells with a range and is drawn against the DIN band like T20 and EDT; EDT carries both row 37 marks.
- **m12-b-plant** (new spec `app/e2e/specs/m12.bedplant.e2e.ts`): the app and `simpa results` run with C80 planted
  FAIL, read by core at runtime. C80 has 0 elements on the page under every band, receiver and DIN selection, its
  name is in neither the tab nor the Results panel, it is not drawn; the other ten are rendered. Before the core
  change the same test found C80 on screen (red, below).

**What Burhan may want to know:** core gained one test-only lever, `$SIMPA_BED_DEMOTE`, the second of its kind
after `$SIMPA_SOLVER_MANIFEST`. It can only hide a number, never show one: a parameter is PASS only where both the
compiled-in summary and the named file say PASS, and the report names the file in `bed.summary` and in each
demoted parameter's reasons. Nothing else about the product changed in P4.

## Merges

| Commit | What | Conflicts and how they were resolved |
|---|---|---|
| `e25fef4` | `origin/m12-viewport` (P3) into `m12`, `--no-ff` | none |
| `2057897` | `origin/m12-acoustics` (P2) into `m12`, `--no-ff` | `tools/gates/m11.ps1` `$specIds`: both inserted a line after the same entry; kept both (`m12.viewport`, `m12.acoustics`). `tools/gates/m12.ps1`: both inserted a block after P1's checks; kept both whole, P2's (static checks, its e2e, a b e f) before P3's (c d) so the gate reads in id order; each block removes its own ids from the NOT BUILT list. `app/e2e/m11.conf.ts` merged clean (P2 had applied P3's dotted-spec change byte for byte). |

P1.1 (`e8acdc5`) touched none of P2's or P3's files. No report version or IPC conflict: P2 holds results version
12 and its pin (`667ef5d3...`); P1.1 changed the summary's content, not the report's fields; P3 added no command,
so m11's inventory stays at P1's 44.

One integration fault, found by the first full gate run (`gate-final-m12-1.log`, 14:34-14:49, at `741b76a`):
M10's lint "backend is called only from actions.ts, selftest.ts and App.tsx" failed on P3's
`features/viewport/resultsView.ts` (`backend.runData` x2, `backend.runSurfaceMap`, `backend.runParticles`). P3
had not run the prior gates (BUILD-P3, Regression). Fixed in `actions.ts` with three pass-through actions
(`runData`, `runSurfaceMap`, `runParticles`; no busy line or Console entry, so the viewport's behaviour is
unchanged) that `resultsView.ts` now calls. Every other check of that run passed: P1's six, P2's three static, the
P2+plant e2e and m12-a, b, b-plant, e, f, the P3 e2e and m12-c, d, mq4, p3-maps; in M11, only this M10 lint (and
the m10 call it sits in) failed; M9 PASSED; 0 files left in the repository.

One environment fault on the first e2e try, not a code fault: the worktree's `app/node_modules` predated P2's
`uplot` dependency, so `tsc` read uPlot's callbacks as `any` (`red-e2e-bedplant.log`). `npm ci` from the
lockfile fixed it (`npm-ci.log`).

## The (b) change

| Piece | Where |
|---|---|
| The plant | `tests/fixtures/beds/summary-c80-fail.json`: the summary's shape, C80 FAIL with a reason saying it is a plant, the rest PASS; row in `tests/fixtures/README.md` |
| Core | `crates/simpa-core/src/results/bed.rs`: `DEMOTE_ENV`, `demote` (PASS only where both PASS; demoted reasons prefixed `demoted by $SIMPA_BED_DEMOTE (<path>)`), `build` (an unreadable or malformed file fails every parameter, naming why; `summary_sha256` stays the compiled-in file's); `report()` reads the variable once. Documented in `docs/formats/results-json.md` |
| Harness | `app/e2e/m11.conf.ts` `beforeSession`: the bedplant session alone gets the variable (the app inherits it through tauri-driver and msedgedriver; the spec's `simpa results` calls inherit it from the worker); every other session has it removed |
| Specs | `app/e2e/lib/acousticsTab.ts`: P2's tab helpers moved out of `m12.acoustics.e2e.ts` unchanged, plus `bedSweep` (gate (b) over every selection) and `assertPassRendered`; m12-b rewritten on them; `m12.bedplant.e2e.ts` new; `EDT_MARKS` in `lib/acoustics.ts` |
| Gates | `m11.ps1` `$specIds`: `m12.bedplant` = `m12-b-plant`; `m12.ps1`: P2's e2e call runs `-Spec m12.acoustics,m12.bedplant` and requires m12-b and m12-b-plant |

A rule loosened on the way, on evidence: the first green attempt also required every PASS parameter but STI to
show at least one value with a range. The gate's box refuses dB(A) in all 45 cells (its 250 Hz SPL is truncated,
`params_not_evaluable`), which is that run's honest status, shown with its code and held to the JSON by m12-a. The
range requirement now names the reverberation times (EDT, T20, T30); every other cell must still carry a range
and status or a refusal code.

### Red, then green

| Test | Red | Green |
|---|---|---|
| e2e `m12-b-plant` | `red-e2e-bedplant-2.log`: `band 0: c80_db (FAIL) has elements`, 5 elements (the app showed C80; nothing in core read the plant) | `green-e2e-bedplant.log`: passed; receipt: `report.bed c80_db FAIL (demoted by $SIMPA_BED_DEMOTE (...summary-c80-fail.json): planted ...)`, elements for the other ten, C80 none |
| core `results::bed` (3 new: demote only demotes, never promotes and keeps both reasons; unset or empty is the compiled-in summary; the variable's file demotes and is named, an unreadable or malformed file fails all eleven) | `red-core-demote.txt`: does not compile (`build`, `demote` missing) | `green-core-demote.txt`: 7 passed |
| `cli_results` `m12_the_bed_demote_plant_fails_c80_in_the_cli_report_and_nothing_else` | written after the core change (the e2e is the red for the mechanism) | `green-cli-demote.txt`: passed with `gate_e_seat...` and P2's `m12_` |

### Step 3, T30 and EDT now that they are PASS (`green-e2e-bedplant.log`, m12-b receipt)

- **T30**: 135 cells, 81 with a range and status, 54 refused with their codes; drawn (series `edt_s`, `t20_s`,
  `t30_s`). m12-f's series now carry three curves per run, 0 mismatches.
- **EDT**: 135 cells, 115 with a range; marks shown: "EDT unchecked for receivers larger than one metre in radius",
  "EDT unchecked in energetic mode".
- **m12-a** with all eleven: 2,546 numbers and 688 strings compared over every selection, 0 mismatches (P2's run
  with seven: 1,916 and 532).
- STI and dB(A) are refused in every cell of the box (no 8 kHz band; a truncated 250 Hz SPL), shown with codes.

## Suite

- Workspace (`cargo test --workspace --no-fail-fast -- --test-threads 2`, `suite-1.log`, 14:22-14:33, at
  `5e10ec9`): **1110 passed, 0 failed, 40 ignored**. P2's 1103, plus P1.1's 3 (`bed_summary` 4 -> 7), plus P4's 4
  (core 3, `cli_results` 1).
- UI `npm test`: 180 passed (159 + P2's 10 + P3's 11). Harness `node --test e2e/lib/*.test.ts`: 56 passed (45 + 7 +
  4). `npm run typecheck`, `cargo fmt --check`, core clippy `-D warnings`: clean.

## Gate

**Final `m12.ps1` in full: M12 PASSED** (`-TargetDir C:\tmp\nm-target-h -BedData B:\data`,
`gate-final-m12-2.log`, 14:50-15:04, at `1c280c5`, this file committed and pushed before it): 21 of 21 checks.
P1's six; P2's three static; the P2 e2e (`-Spec m12.acoustics,m12.bedplant`) and **m12-a, m12-b, m12-b-plant,
m12-e, m12-f PASS**; the P3 e2e and **m12-c, m12-d, m12-mq4, m12-p3-maps PASS**; no id NOT BUILT; prior gate
**M11 PASSED** (32 of 32 checks; inventory 44 commands; core selection 926 passed / 0 failed / 36 ignored in 86
binaries; e2e 37 of 37 required ids, 0 failures, 0 skipped; m10 `-SkipCore` exit 0 with the backend lint
passing; **M9 PASSED**); 0 files left in the repository. The first run (`gate-final-m12-1.log`) failed on the M10
lint above.

## Left open

- **Assay and the merge into `rebuild`** (PLAN.md P4's last two words) are the coordinator's, not done here.
- The CLI's text banner (`results_cmd.rs:186`, "M8's physics bed has not passed") still contradicts
  `validated_by_bed: true`; P1's open item, backlog 74 / MQ2.
- P3's map controls sit over the viewport, not in the design's right-hand panel; P3's open items stand.
- `app/src-tauri/Cargo.toml` shows modified after a build (line endings only); not committed.

## Assay fixes (2026-10-03, after `2833b4a`)

The assay's verdict was **SHIP-WITH-FIXES**: two MED findings and four LOW, recorded with their resolutions in
`ASSAY.md` beside this file. Five are fixed in three commits; the sixth (H6 `n_classes == 11`) fails safe and is
left. Receipts: `B:\data\m12\p4\` (`red-assay*`, `green-assay*`, `suite-assay.log`, the gate log).

**What it means for Burhan:** no number on screen changed. The word "validated" is gone from the Simulate step's
Method hint (it now says "EDT unchecked in energetic mode", the Acoustics tab's own mark), and a test fails if it
appears in any UI string again. T30 now carries a small mark beside the table: "Ranges on noise-limited T30
values may be slightly narrow: 1 of 833 checked values fell 1.5 ms outside its range." The RT chart draws each
value's range as a whisker, and only the values the tables show. Gate (a) now also proves each number sits under
the right receiver, parameter and band labels, not only that it equals its JSON path.

| Commit | Finding | Change |
|---|---|---|
| `a5ef89b` | 1 (MED): "marked not validated" in `SettingsEditor.tsx:403` | Hint uses `EDT_MARKS[1]`; `ui/src/wording.test.ts` parses every UI source file's strings (literals, templates, JSX text, CSS) and fails on "validated"; snake_case field names and comments are not words on screen |
| `b90e7d5` | 2 (MED): gate (a) never tied a cell's visible labels to its path | `scan` reads each cell's row head, column head and its card's Band/Receiver selection from the DOM with every `data-json` inside; `cellLabelMismatch` requires them to name the paths' receiver, parameter and band; m12-a checks every cell under every selection, and a control swaps two rendered cells' paths |
| `b90e7d5` | 3 (LOW): RT chart drew raw `.value` outside `cell()` | `rtSeries` built from `cell()` (PASS only, value only with its range and `ok`/`wide`, gaps where refused), with `lo`/`hi` drawn as whiskers; gate (f) compares the range too |
| `b90e7d5` | 4 (LOW): T30 shown by decision 46 with no mark | `T30_MARK` beside the receivers table while T30 is shown (`paramMarks`, which also carries EDT's two marks and STI's note); marks are `[data-label="mark"]`, checked word for word by m12-a, T30's required by m12-b |
| `ccff1d3` | 5 (LOW): count mode skipped the draw's code after the step test | One `kept()` gate for both modes, count branch directly after it, no cull after; shaders moved to `particles.ts` so `particles.test.ts` holds that shape |
| none | 6 (LOW): H6 `n_classes == 11` | Fails safe: any other class count demotes EDT, never promotes; left as is |

### Why count mode and the draw still differ (finding 5)

The draw had no energy cull: a zero-energy record is drawn at the ramp's floor, so the two modes already kept the
same records. Now both go through `kept()` (a record at its own step, the `.pbin`'s definition of alive, which
`aliveCounts` counts), and nothing after the count branch may drop a record. Two things differ by design, and gate
(d) depends on both: **the camera**, because count mode puts every kept record on one pixel, so the count is of
the particles alive at that step, not of those the current view happens to frame (a particle behind a wall or
off-screen is still alive and still drawn when the view turns to it); and **the round sprite**, whose
corner discard in the fragment shader never removes a point's centre pixels, so it cannot hide a particle. Gate
(d) still compares the GPU's count with alive in the `.pbin` at 5 steps.

### Red, then green

| Test | Red | Green |
|---|---|---|
| UI `wording.test.ts` (2) | `red-assay1-wording.txt`: one hit, `features\simulate\SettingsEditor.tsx:403` | `green-assay1-wording.txt`: 2 passed |
| UI `model.test.ts` (+2: RT chart through `cell()` with range; marks) | `red-assay34-model.txt`: no export `T30_MARK`; `red-assay3-model.txt`: T20 with no range drawn (`[0.555, 0.555]`, want `[null, 0.555]`) | `green-assay34-model.txt`: 12 passed |
| harness `acoustics.test.ts` (+3: series filter and range, labels against paths with deliberately mislabelled cases and a swap, marks) | `red-assay2-harness.txt`: 3 failed against stubs (`cellLabelMismatch` returning null; the old raw-value series) | `green-assay2-harness.txt`: 10 passed |
| UI `particles.test.ts` (+1: one `kept()` gate, count branch after it, no cull after) | `red-assay5-particles.txt`: no `kept()` | `green-assay5-particles.txt`: 4 passed |
| e2e m12-a, m12-b, m12-f, m12-d | the in-run controls (a planted digit; two cells' paths swapped) | the final gate below |

UI `npm test`: 185 passed (180 + 5). Harness `node --test e2e/lib/*.test.ts`: 59 passed (56 + 3). `npm run
typecheck` and the e2e `tsc`: clean. `m12.ps1` gains one static check (the wording and particle suites) and the
m12-a/m12-b descriptions name the new checks.

### Suite

Workspace (`cargo test --workspace --no-fail-fast -- --test-threads 2`, `suite-assay.log`, 15:18-15:28, at
`b90e7d5`): **1110 passed, 0 failed, 40 ignored**, as before: the fixes touched no Rust.

An e2e pre-run before the gate (`m11.ps1 -Only e2e -Spec m12.acoustics,m12.bedplant,m12.viewport`,
`pre-e2e-assay.log`, 15:28-15:29, under `B:\data\m12\e2e.lock`): m12-a, b, b-plant, e, f, c, d, mq4, p3-maps
passed. m12-a: 2,546 numbers and 688 strings compared, **738 cells' visible labels held to their paths, 0
mismatches**; the swap control caught `receivers-table "R1" x "SPL"` carrying R2's G path ("the receiver shown is
R1, the path's is R2"). m12-b: T30's mark seen word for word; T30 135 cells, 81 with a range. m12-f: the drawn
series, ranges included, 0 mismatches both ways. m12-d: drawn 33, 2, 1, 0, 27 against alive 33, 2, 1, 0, 27.

### Gate

The full `m12.ps1` (`-TargetDir C:\tmp\nm-target-h -BedData B:\data`) ran after this file was committed and
pushed (`d6f5d61`): **M12 PASSED**, 22 of 22 checks (`gate-final-m12-assay.log`, 15:30-15:45, under
`B:\data\m12\e2e.lock`): P1's six, P2's three static and the new assay check, m12-a (738 cells' labels held to their
paths, 0 mismatches, the swap control caught), m12-b, m12-b-plant, m12-e, m12-f, m12-c, m12-d, m12-mq4,
m12-p3-maps; prior gate **M11 PASSED** (with **M9 PASSED**); 0 files left in the repository.
