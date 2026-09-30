# Backlog 38 and 39: the review

Step 3 of `PLAN.md` (pass bar 6), on branch `b38-39` at `ec3f97e`. Four reviewers ran from
2026-09-30 23:46 to 2026-10-01 00:04. Each had its own lens, and none of them wrote the code:

- **contract (C):** both decisions traced through every consumer, against C1-C7;
- **tests (T):** whether each test in the plan proves its claim, and its RED against the RED commits;
- **off-axis (O):** the bed, other callers, runs without `--variant`, the docs, the shelved
  `m8b-tamper` branch, and the trees;
- **gate receipts (S):** `GATES.md` against the gates' consoles and work folders.

The review was read-only. No reviewer wrote a file into either tree or re-ran a gate. This report
was written from the reviewers' results, and every finding below was re-read at its cited lines on
`ec3f97e` before it went in.

## Verdict

**MERGE-READY.** The lenses raised 23 findings: 0 blockers, 0 majors and 23 minors. Pass bar 6 sends
only surviving blockers and majors to a fix round, so plan step 4 does not run. Step 5 is next.

No skeptic ran. Pass bar 6 says a skeptic checks each finding, but this review sent skeptics only to
blockers and majors, and there were none. Each minor therefore rests on its own lens's reproduction
and on the re-read for this report, which contradicted none of them. Two citation slips are noted
under "Refuted".

The other pass bars, on the lenses' receipts:

1. **RED.** The tests lens, an agent other than the writer, matched every RED.md failure line to an
   assertion in the RED commits (`fbe5371`, and `910bb8f` for T38-8) and read the stubs.
2. **GREEN.** Every test in the plan passes at `ec3f97e`. The tests lens re-ran them, and `m11-b38`
   comes from the M11 gate's log. No assertion was weakened after RED.
3. **fmt and clippy `-D warnings`** were clean in M11 and M10.
4. **The gates.** M11 (32 of 32 required ids, 0 core failures, 0 foreground changes), M10 and M9
   passed, in that order. The gate-receipts lens re-derived each count from the raw logs.
5. **Nothing under `crates/simpa-core/src/bed/` changed**, and `load`, `check_status` and
   `checked_report` are unedited. On the real M8a bed, one run from each of the 64 cells gives the
   same report from HEAD's build as from M8a's, apart from the new field.
7. **No new untracked file in either tree.** C: free space was recorded: 18.48 GB before and
   18.14 GB after.

## Surviving findings

None. No blocker or major was raised, so no finding needs a fix round or a failing-first test.

## Refuted

None. The re-reads found two citation slips. Neither changes its finding:

- S1 says `c9edf1e` is absent from the file it cites. `c9edf1e` is the commit that recorded that file
  ("GATE.md: M11, M10 and M9 pass after the second review's fixes"). The runs themselves were on
  `0f5cb12` (`2026-09-29-m11/GATE.md:38`). S1's actual point, 14m45s against 13m45s, stands.
- O3 quotes the call at `main.rs:210` as `config_xml::write(&project, solver, None, ..)`. It is
  `config_xml::write_file(&project, solver, variant, ..)`, with `variant` still `None`, so the
  behaviour is the same. (C2 puts the same call at `:209`.)

## Minors: proposed v1.1 backlog rows

Findings that duplicate each other across lenses are merged: 20 of the 23 findings become the 11 rows
below. The other three are settled in the next section. The rows start at 46, not 43, because the
shelved `m8b-tamper` (`21efe03`) already adds rows 43-45 after row 42, the same place step 5 appends
(O2). Step 5 adds the rows to `docs/v1.1-backlog.md`. This commit changes no other file. Line
numbers in the rows are as re-read on `ec3f97e`. Where a lens was a line or two off, the row uses the
re-read line.

| # | Item | Why it waits | Origin (receipt) | Done when | Status |
|---|---|---|---|---|---|
| 46 | **A run made with `simpa run --variant <name>` is labelled "unknown variant" on its Runs row and "Deleted variant" on its Simulate and Results labels. `simpa run` also reads the project file twice** (b38-39 review, C3 and C9). `run_project` records `source.variant` as the selector it was given (`manager.rs:674`), and the app matches labels by id only (`dock/model.ts:202`, `simulate/model.ts:367`). The new default passes the id, so only an explicit name is affected. The flaw predates this step, and T39-2 now pins it (`cli_run.rs:1316` expects `"uniform"`). The default also reads the file to pick the active variant (`mesh_run.rs:594-599`) before `run_project` reads it again to hash and export it (`manager.rs:640`). If the file is saved in between with another variant active, the run exports the old variant under the new bytes' sha256. | It is a label, not a number, and only on the explicit-name path. The race needs a save within milliseconds of a CLI start. PLAN C3 kept `run_project`'s semantics for this step. | `docs/investigations/2026-09-30-b38-39/REVIEW.md` C3, C9; `manager.rs:640-675`; `mesh_run.rs:569-599`; `runs.rs:513-516` | `run_project` resolves the variant (the active one, the base, or a named one) from the bytes it hashes and records the resolved id. T39-2 expects the id. A `run_manager` test shows `source.variant` is the id of the active variant in the bytes hashed, and a model test labels a run made with `--variant <name>` by that name. | open |
| 47 | **`results_version` stays 5 although `solver_build` joined the report's required fields** (b38-39 review, C1 and O5). `REPORT_VERSION` is 5 (`report.rs:46`). Neither version history names `solver_build` (`report.rs:28-45`, `results-json.md:89-109`), but the committed schema requires it (`results-json.schema.json:3154`). A v5 report from any build up to `bc1b3f1`, including the pushed `rebuild` head, lacks the field and fails the committed v5 schema. `results-json.md:21-22` tells a reader to ignore fields it does not know, so a v5 reader written earlier would skip the one field that says whether exit-0 results are verified. Every earlier bump, 2 to 5, carried added fields. | No code reads `results_version` (only tests do). M12, its first reader, is not built. No committed JSON outside `docs/formats` carries `"results_version": 5`. The version rule is not among the decisions or C1-C7, so this could not block (pass bar 6). It is cheapest before M12. | `2026-09-30-b38-39/REVIEW.md` C1, O5; `GREEN.md` ("The review may prefer a bump") | `REPORT_VERSION` is 6, with a history line naming `solver_build` in `report.rs` and in `results-json.md` (or, on Burhan's word, the addition is recorded under 5 in both). A test pins the report's required fields to its version, so a required field added without a bump fails it. | open |
| 48 | **`simpa export-config` still exports the base by default. On a file with an active variant it writes a different `config.xml` from the one `simpa run` runs** (b38-39 review, C2 and O3). `variant` starts as `None` (`main.rs:178`) and reaches `config_xml::write_file` (`main.rs:210`), and `resolve_variant(None)` is the base (`config_xml/write.rs:127-129`). The usage line ("write config.xml and mesh.cbin for a run", `main.rs:16-17`) does not state this default. It is the one CLI path still on the pre-39 default, and together with `run-folder` it is a documented route to a run. | Decision 39 names `simpa run` only, and GREEN.md left this alone on purpose. No gate, test or tool runs `export-config` on a file with an active variant. It is a product call, as 39 was. | `2026-09-30-b38-39/REVIEW.md` C2, O3; `GREEN.md`, "Left for step 5" | Burhan picks the default. Either it matches `simpa run` (the active variant, with `--base` for the base) or the usage states that it exports the base. A `cli_run` test on T39-1's box checks what `export-config` writes without `--variant`. | open, Burhan's |
| 49 | **The pre-38 verdict rule survives in `solversMark`'s `kind` and in `m11-dock-row`'s oracle, and no test tells it apart from the core's rule on a Runs row** (b38-39 review, C4, T1, T2). `solversMark` (`dock/model.ts:258-262`) still answers `verified` for checks that cover only TetGen, and `dock/model.test.ts:194-203` pins that. `buildMark` uses only its names. `m11.dock.e2e.ts:342` computes the expected `data-verified` with `m.solvers.every((c) => c.matches)`. In every T38-8 UI case the verdict agrees with the checks under C5, so a `buildMark` that re-derived C5 in TypeScript would pass. The row's only new DOM check (`m11-b38`) is on a CLI run, where both rules give `unrecorded`. A mutant `RunsPane.tsx:123` that takes `solversMark`'s kind passes every test. | The two rules agree on every row this code writes. An app run checks its own solver first (`manager.rs:572-573`), a mismatch is refused at stage solvers, and a CLI or bed run records no checks. They differ only on a hand-edited or foreign `run.json`. | `2026-09-30-b38-39/REVIEW.md` C4, T1, T2 | `solversMark` returns only the names. `m11-dock-row` takes its expected mark from `RunRow.solver_build`, or requires the solver's own check. A dock model test gives matching checks with an unverified `solver_build` and expects `unverified`. An e2e plants a `run.json` whose checks cover only TetGen (as `plant-loss.ts` rewrites `particles`) and requires `data-verified="no"` and `SOLVER_BUILD_UNCHECKED` on its row. | open |
| 50 | **The Runs row and the Results step show the same unverified build differently, and no test pins the Results step's label or colour** (b38-39 review, C5 and T4). For `solver_build_unchecked` and `solver_build_mismatch`, the row shows a red `FAIL` flag (`RunsPane.tsx:161-167`), while the Results step shows `UNVERIFIED · Results unverified` in the warn colour (`ResultsPanel.tsx:107-118`). For `solver_build_unrecorded`, the row shows prose, and its code appears only in `data-build-code` (`RunsPane.tsx:157, 169`), although row 29 asks for codes, not prose. `m11-b38` rules out only the exact phrase "Results verified" (`m11.gate.e2e.ts:373`). A verdict line rendered as `res-verdict ok` with the label `OK` passes it, and T38-7 as well. | The verdicts agree: both come from the core, and T38-8 and `m11-b38` pin them. What differs is what a user sees, which is Burhan's call, and a change of style was out of scope for this step. | `2026-09-30-b38-39/REVIEW.md` C5, T4 | Burhan picks one label and colour for an unverified build (keeping FAIL for a mismatch if he wants it). The row shows the unrecorded code as text. `m11-b38` asserts the verdict's label and class on both the Results step and the row, so the `ok`/`OK` mutant fails it. | open, Burhan's |
| 51 | **The CLI half of 38-39 is only loosely pinned by its tests** (b38-39 review, C7, T3, T5). No test reaches the verified arm of `simpa results`, in text or JSON (`results_cmd.rs:168-171`). No committed run folder records checks, so that arm could print "UNVERIFIED" and every test would still pass. T38-6's JSON half checks only that the code appears (`cli_results.rs:2663`), not `solver_build.status` or `solver_build.reason.code`. T39-5 accepts any "active variant" in the run usage (`cli_run.rs:1398`), so a help text that names it only as what `--variant` overrides would pass. | Each branch is right today. The verified arm was run by hand on the M11 gate's SPPS and TCR app runs (exit 0, `{"status":"verified"}`, and its text line), and the help states the default. This is test strength, not a false claim. | `2026-09-30-b38-39/REVIEW.md` C7, T3, T5 | A `cli_results` test copies a committed run folder to scratch, records matching checks in its `run.json`, and requires the verified line and `{"status":"verified"}`, validated against the committed schema. T38-6 asserts `solver_build.status` and `.reason.code`. T39-5 asserts the sentence that states the default. Each test fails on its mutant above. | open |
| 52 | **The help, docs and comments still use "verified" in `load`'s sense, and three places state the pre-39 CLI rule** (b38-39 review, C6 and O4). The `simpa results` usage reads "a verified run's results ... exit 0; ... 6 its results do not verify" (`main.rs:46-47`). It names neither the verdict line nor that exit 0 covers an unverified build. `results-json.md:3` and `report.rs:1266` use "verified" as `load` means it. `ResultsPanel.tsx:102` explains "Results verified" as "run.json, inputs and outputs re-checked" and leaves out the solver build, which that state now requires. The old CLI rule is stated at `run_manager.rs:867-869` ("the CLI passes `--variant` as given, or nothing") and in `m5-m6-design.md:541-546`, which has no `--base` and no default, and also misses the older `--preprocess`. `results-json.md:21` says the schema forbids no extra fields, but `Reason` now does (`results-json.schema.json:1960`), as three objects already did (lines 5, 277, 1754). | This is wording: the code and the run usage are right. Step 5 may close part of it in its docs pass, since GREEN.md left `m5-m6-design.md` to that step. | `2026-09-30-b38-39/REVIEW.md` C6, O4 | `git grep` finds no statement of the pre-39 rule. `m5-m6-design.md` lists `--base`, the default and `--preprocess`. The `simpa results` usage names the verdict line and says that exit 0 covers an unverified build. "Verified" in `load`'s sense is reworded wherever it appears, and `results-json.md:21` names the objects that forbid extra fields. | open |
| 53 | **The predicate matches the solver's check by name only** (b38-39 review, C8). `solver_build` looks for `c.name == solver_exe_name(m.solver)` (`results.rs:504-514`) and never reads `m.exe`, the file the run launched (its path and sha256, `manifest.rs:35-40`). So "no check covers the solver the run executed" in practice means "no check is named `spps.exe` or `classicalTheory.exe`". | `run_project` hashes and checks the same `opts.solver_exe` (`manager.rs:573`; `exe_ref` at `:920-927`), so no real run disagrees. Only a hand-edited `run.json` could, and that is the tamper work of rows 41-42. | `2026-09-30-b38-39/REVIEW.md` C8 | The solver's check must also be of the executed file: its `raw_sha256` equals `m.exe.sha256`, and both are the file's raw sha256. A `results_solver_build` test whose `spps.exe` check carries another file's hash reads `solver_build_unchecked`. | open |
| 54 | **No CLI or bed run can read "verified"**, not even M8a's 433 runs, whose executables the bed itself checked at E1 (b38-39 review, O7). `run_options` sets `verify: None` (`mesh_run.rs:520`), and so does the bed's `run_one` (`bed/run.rs:364`). `simpa results` on one run from each of M8a's 64 cells reads `solver_build_unrecorded` 64 of 64. | This is decision 38 as written, and `solver-contract.md`'s new table says so. The bed half is a change under `bed/`, which C2 forbade for this step. | `2026-09-30-b38-39/REVIEW.md` O7 | Burhan picks whether `simpa run` verifies its executables on request (`--verify`) or by default, as the app does. A `cli_run` test's run then reads verified in `simpa results`. The bed records E1's checks in each run's `run.json`, with a bed test. | open, Burhan's |
| 55 | **Merging the shelved `m8b-tamper` after this step conflicts in `run/manager.rs`, and either one-sided resolution is wrong** (b38-39 review, O1). `21efe03` moved `run_project`'s variant handling into `pub fn project_as_run`, which `bed/check.rs:1210` and `bed/run.rs:551` call. This step rewrote the same doc comment and body comment. `git merge-tree` finds two conflict blocks, both in `manager.rs`; `docs/solver-contract.md` merges clean. Taking this step's side drops `project_as_run` and breaks the build. Taking `m8b-tamper`'s side restores the pre-39 rule in the comments. In addition, `21efe03`'s `analytic_of_project` (`results/tcr.rs`) exports with `None`, the base, from a project that `project_as_run` may have switched to a variant. | The branch is shelved as the head start for rows 41-42 (`HANDOFF-2026-09-30.md`, "Next", item 0). | `2026-09-30-b38-39/REVIEW.md` O1; `git merge-tree 5d46aa7 b38-39 21efe03` | The merge keeps `project_as_run` and its call in `run_project`, keeps this step's `run_project` doc, and moves the rewritten body comment into `project_as_run`. The merge builds, and `manager.rs` no longer states the pre-39 rule. T39-1 to T39-5 and `m8b-tamper`'s own tests pass on it. `analytic_of_project` exports the variant that `project_as_run` chose, with a test that passes one. | open |
| 56 | **`app/src-tauri/Cargo.toml` reads as modified after the first tauri build in a new checkout, although its content is unchanged** (b38-39 review, O6; `GATES.md`'s finding). `core.autocrlf` is true in the system and user gitconfig, and no `.gitattributes` rule covers the file. A checkout therefore writes it with CRLF line endings (668 B, the size the index caches), and `npx tauri build --no-bundle` rewrites it with LF (648 B, the HEAD blob `116d9e0`). Worktrees where a tauri build ran have LF; the others have CRLF. | It is cosmetic: `git diff` is empty, `git add` clears it with the same blob, and the gate counts only untracked files. It comes back after any checkout that writes the file with CRLF. | `2026-09-30-b38-39/REVIEW.md` O6; `GATES.md`, "Before and after" | `.gitattributes` holds `app/src-tauri/Cargo.toml text eol=lf`, and a new worktree followed by `tauri build --no-bundle` leaves `git status --porcelain` empty. | open |

## Settled here, not rows

- **O2, the row numbers.** Applied above: the rows start at 46.
- **S1, an erratum to `PLAN.md:100` and `GATES.md:15`.** These give M11's baseline as "14m45s on
  `c9edf1e`". The cited row, `2026-09-29-m11/GATE.md:42`, reads 01:54:24-02:08:09, which is 13m45s.
  That run was on `0f5cb12`; `c9edf1e` recorded it. M10's 9m31s and M9's 45s match their rows. So M11
  here took 13m49s, 4 s over its baseline rather than 56 s under it. The times only paced the
  check-ins and judged no pass bar. PLAN.md's bar does not move and GATES.md is a record, so the
  correction is made here, not in those files.
- **S2, the refused count.** `GATES.md:118-120` does not say what the refused count checked. Nothing
  rests on it: the gate-receipts lens re-derived every count GATES.md reports from the raw logs
  (below). That lens also hit the same false positive of the hook's drive-root guard with another
  command, so the explanation GATES.md gives is real.

## What the lenses checked and found clean

### Contract

- **The predicate.** `results_solver_build` (4 passed) and `reason_codes_docs` (3 passed, the
  scanner's say-no test among them). A missing `solvers` key or an empty list reads
  `solver_build_unrecorded`, and `load` stays Ok on both committed fixtures. A failing check, the
  solver's or TetGen's, reads `solver_build_mismatch`, which is tested before unchecked. Checks that
  leave out the run's own solver read `solver_build_unchecked`: TetGen only, or a TCR run checked
  only as `spps.exe`. All checks matching, the solver's among them, reads verified. `run_exes` always
  puts the solver first (`manager.rs:572-581`), so an app run cannot be TetGen-only.
- **C1.** `load`, `check_status` and `read_manifest` are untouched. Every `results.rs` change comes
  after `load` (from `:415`). `checked_report` errs only on a non-finite number, and `SolverBuild`
  holds only strings. Its callers are `results_cmd.rs:59`, `bed/read.rs:321` and tests, and the bed
  reads named fields and serialises no `Report`.
- **C2.** The diff under `crates/simpa-core/src/bed/` is empty. The bed calls
  `run_project(&project, None, ..)` directly (`bed/run.rs:370-377`), so the new default cannot
  reach it.
- **C3.** `manager.rs` changes comments only, and its doc states the new rule. The app still passes
  the active variant's id (`bridge.rs:268`, `runs.rs:977`).
- **C4.** `resolve_variant` has no spelling for the base (`config_xml/write.rs:121-143`), so `--base`
  is new. Combined with `--variant`, in either order, it gives exit 2 with nothing on stdout. The
  parser refuses a repeated `--variant`. The usage states the default and the base.
  `cli_run t39_`: 5 passed.
- **The default's edge cases.** `--base` with no active variant, and a file with no variants, both
  run the base as before. A dangling active variant is refused with `variant_reference_invalid` at
  stage validate, exit 2, as before; the only difference is that `source.variant` now records the
  dangling id instead of null. A reused mesh is unaffected, because `mesh_input_hash` hashes no
  materials. No fixture under `tests/fixtures/ui` or `tests/fixtures/rooms` sets an active variant,
  so the gates' CLI runs export what they did before.
- **The app.** `results_state` is verified only when `load` is Ok and the build is verified. It
  carries the reason when the build is not verified, and leaves the reason out on a refusal
  (`runs.rs:699-722`). 3 app tests passed (T38-5, T38-8 and the UI-code table). Every Runs row with a
  `run.json` goes through `row_from_manifest` (`runs.rs:544`), and only the app's own `RunOptions`
  sets `verify` (`runs.rs:1057-1064`). `resultsStateName` answers verified only when
  `results.verified` is true, and no other UI surface reads that field. Both model test files pass,
  33 of 33 (12 dock and 21 simulate; the lens printed "12 and 33").
- **The real app.** M11's `wdio.log:26-28` shows `m11-b38` reading UNVERIFIED with both codes and no
  digit. The control run reads `OK` and `Results verified` (`wdio.log:25`).
- **`simpa results`.** An unverified build is exit 0, with a verdict line in the text and
  `solver_build` in `--json`. Exits 5 and 6 are unchanged, and the schema tests pass.
  `docs/solver-contract.md` Part B has one row per build code, and the scanner reads them.
- **No scope creep.** Every change belongs to decision 38 or 39, to their tests, or to their docs.

### Tests

- **Every test in the plan passes at `ec3f97e`, re-run:** T38-1 to T38-4 (4), T38-5 and T38-8's Rust
  half (2), T38-6 (1), T39-1 to T39-5 (5), T38-7 and T38-8's UI half (1 each), and
  `reason_codes_docs` (3). `m11-b38` (T38-9) passed in the M11 gate on `8f6f77e`, which differs from
  HEAD only in `GATES.md` (`wdio.log:122`; `junit-0-1.xml:368`, 0.164 s).
- **RED.** Each RED.md failure line sits on an assertion at `fbe5371`, and T38-8's failure lines
  sit on its first assertions at `910bb8f` (`runs.rs:1965`, `dock/model.test.ts:266`). The writer's
  logs are `C:\tmp\b38-39\t38-8-red-rust.log` and `t38-8-red-ts.log`. The stubs reproduce the old
  behaviour. T39-5's second assertion was red too: the usage at `bc1b3f1` had neither "active
  variant" nor `--base`.
- **No assertion weakened after RED.** `git diff fbe5371..HEAD` is empty for
  `results_solver_build.rs`, `cli_results.rs`, `cli_run.rs` and `simulate/model.test.ts`, and
  `git diff 910bb8f..HEAD` is empty for `dock/model.test.ts`. The tests in `runs.rs` only gain lines.
  The scanner is generalised and still reads `mod codes`. The e2e library's type is only widened.
- **`m11-b38`** uses a real run with no solver record, asserted from `run.json` on disk, and is a
  required id (`m11.ps1:82`). Its RED follows from T38-5.
- **The Runs row's verdict comes from the core, through the IPC:** `runs.rs:544`, then
  `RunRow.solver_build`, then `buildMark`, then `RunsPane.tsx:123`. No app code decides it from the
  checks. T38-8 asserts that none of its five manifests is refused, the mismatching one included, so
  `load` does not refuse on the record (C1).
- **The app's verified path is pinned end to end.** `m11-smoke` asserts that the app run's three
  checks match and that its state is verified.
- **Every test can fail.** T38-4, T39-2 and T39-4 fail on over-refusal or on the wrong variant.
  T39-3's combination half fails if the combination is accepted. T38-6's JSON half fails without the
  field.

### Off-axis

- **C2.** The diff over `bed.rs`, `bed/`, `bed_cmd.rs`, `bed_m8a.rs`, `m8a.ps1`, `beds/` and
  `tools/bed/` is empty. Nothing under `bed/` serialises, hashes or compares the results `Report`.
- **C1 on the real M8a bed.** The lens ran `simpa results --json` from HEAD's release build and from
  the M8a build (29/09 17:24) on one run from each of the 64 cells under
  `C:\tmp\nm-m8a-bed\20260929T093134Z\runs`. Both builds exit 0 on 64 of 64, and with `solver_build`
  removed, all 64 reports are identical (163 s). This step adds nothing to `simpa bed --schema`, and
  `simpa bed --canonical` is byte-identical between the two builds.
- **`m8b-tamper`'s seal.** It hashes run folders and the bed's own report, never a `simpa results`
  report, and this step writes nothing into run folders.
- **The verified branch on real app runs.** On the SPPS and TCR runs in M11's work folder:
  exit 0, `{"status":"verified"}` and the verified line. On the CLI loss run: exit 0 and
  `solver_build_unrecorded`.
- **The new default touches no existing run.** No gate, e2e, test or tool runs `simpa run` without
  `--variant` on a file with an active variant. Of the worktree's 56 `.simpa` files, only
  `variant_reference_invalid.simpa` sets one, and nothing runs it. Importers never set one, and the
  bed passes `None`.
- **Consumers.** Every consumer of the changed IPC types is updated. `m11-sim-link` accepts only
  verified or refused, and the checks of its app runs match. Every reader of `simpa results` output
  reads `--json` by field name.
- **The trees.** After the checks, the worktree showed only ` M app/src-tauri/Cargo.toml` and the
  main checkout only its 3 LEAVINGS files. C: had 18.2 GB free.

### Gate receipts

- **Core counts,** recomputed from M11's and M10's raw cargo logs: 64 + 9 binaries, 720 + 61 passed,
  12 + 19 ignored, 0 failed, and no `FAILED` line.
- **e2e:** M11's junit has 32 test cases and M10's has 27, with 0 failure elements.
- **`m11-focus`:** 0 foreground changes, 748 samples and 15 sessions, from `focus-judge.log`.
- **Files left on B::** 0, from the untracked snapshots taken before and after (the main checkout
  had the same 3 both times, the worktree none).
- **Work folders:** the counts and sizes (381 files and 17.8 MB, 21 files and 0.4 MB, 8 files and
  0.2 MB) and every nested folder name are exact. The PASS lines number 88, 27 and 23. No FAIL line
  appears at any indentation, and every skip, warning and refusal token is an expected fixture or
  count.
- **The rest.** The planted-loss bands match the run's `run.json`. The gates ran in order, without
  overlap. M9's `selftest.json` has 22 checks and none failed. The +1 test binary and +15 passed tests
  against `0f5cb12` come from `results_solver_build`. Free space on C: fell steadily from 18.48 to
  18.14 GB.

## Not checked

- No reviewer re-ran a gate. The gates rest on their own logs, which the gate-receipts lens
  re-derived.
- No test was re-run at its RED commit. RED rests on the stubs, the positions of the failure lines
  and the writer's logs.
- The mutants in T1 to T5, and C1's claim that an old binary's report fails the schema, were
  reasoned, not run. C9's race and the CLI path for a dangling active variant were traced, not
  reproduced.
- No reviewer re-ran fmt, clippy, the typecheck or the e2e typecheck; the gates ran them.
- The bed's own ignored tests and `simpa bed --from` were not run, because C2 and the plan exclude
  them. The 64-cell comparison covers one run per cell, not all 433, and its baseline is the M8a
  build, not a build of `bc1b3f1`.
- The claim that the tauri build rewrites `Cargo.toml` is inferred from timestamps, the index's
  cached size and the hashes. The `m8b-tamper` merge was previewed with `git merge-tree`, not built.
- The CLI's verified branch was checked by hand on one SPPS and one TCR app run. No committed test
  covers it (row 51).
- The window between `exe_ref` and `verify_solvers` in `run_project`, in which the executable could be
  replaced between hashing and checking, belongs to rows 41-42 and was not examined.
- The M9 bindings' blob hashes were not re-derived.
