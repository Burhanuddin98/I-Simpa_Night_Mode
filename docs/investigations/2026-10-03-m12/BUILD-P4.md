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
