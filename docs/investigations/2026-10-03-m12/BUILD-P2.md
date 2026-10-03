# M12 P2 build: the Acoustics tab (2026-10-03)

Branch `m12-acoustics` off `m12` @ `b9a60cd`, worktree `.claude/worktrees/m12-acoustics`. Spec: `PLAN.md` beside this
file, P2, gate (a), (b), (e), (f), MQ1-MQ6 at their defaults. Receipts (red and green logs, gate logs, the progress
log): `B:\data\m12\p2\`.

## What it means

**The Results step now shows numbers, in the Acoustics tab of the dock, and only the ones the bed passed.** For the
selected run: a table of all receivers (one band at a time, or the bands summed) with every parameter whose status
in `report.bed` is PASS, each value with its range (`lo`–`hi`) and `ok`/`wide`, or its refusal code and kind; the
reverberation time per band for one receiver, drawn in uPlot against the DIN 18041 target (group chosen, A3 by
default) with the same numbers in a table; that receiver's decay curve; a Sabine/Eyring table; and the absorption
by surface group per band with its totals. Every number on the tab is read from the report `simpa results --json`
prints, by its JSON path, and the e2e compares all of them. The words are MQ2's; "validated" appears nowhere.

As the evidence stands on this branch (`beds/summary.json` at `b9a60cd`), **T30, EDT, G and dB(A) have no element
on screen**; SPL, T20, C50, C80, D50, Ts and STI are shown. The UI holds no list of statuses: when the m12 branch's
summary records T30 PASS (decision 46), T30 appears with no UI change. EDT's two marks (row 37 (1)-(2)) and the
per-value "unchecked" note are built and unit-tested, but they cannot be seen until EDT passes.

**What Burhan should look at:** the core change below (results version 12, `room`), and two choices made for the
screen that are not in the plan's text: D50 is shown as a fraction (0.00, not %), and the DIN band is drawn as a
fifth either side of `T_soll` (the design's simplification; the standard's own band per frequency is not drawn,
and the tab says so).

## Commits

| Commit | What |
|---|---|
| `87e4017` | P2.1, core: `results::room` and `RoomReport` (volume, area, DIN 18041 A1-A5 targets, absorption by solver material id, band totals), `sabine_s` in an SPPS reference band; results version 12, required-fields pin, schema, `results-json.md`, bindings |
| `19a6ad9` | P2.2: the e2e spec `app/e2e/specs/m12.acoustics.e2e.ts` and `lib/acoustics.ts` (+ test), written before the tab; red |
| `8283687` | P2.3, app: `run_report` adds `surface_groups` (each material id's group names from the open project, `SolverIds::assign`) |
| `720f2e6` | P2.4, UI: `features/acoustics/` (model, pane, css), `reportStore`/`reportFor`, the dock mounts it, the variant follow; the spec moved onto m11.ps1's harness as P3's is; m11-sim-numbers and m11-sim-link narrowed; m12.ps1's P2 checks |
| `73c897f` | P2.5: e2e green; option checks in m12-a; the Kuttruff column left out where refused in every band |

## Why the core changed (not only the UI)

Gate (a) holds every number on the Acoustics tab to `simpa results --json`. The DIN target, the absorption by group
and a Sabine time for SPPS runs were not in the report, so computing them in the UI would put numbers on screen the
gate cannot check. They are now in the report (`room`, results version 12), computed from the run's own inputs as
TCR's analytic references read them, for both solvers. Group names are not numbers and are not in the run folder;
the app maps material ids to the open project's groups (the label says the names are from the project as it is open
now).

## Tests, red then green

| Test | Red (receipt in `B:\data\m12\p2\`) | Green |
|---|---|---|
| `cli_results.rs` `m12_the_report_carries_the_rooms_din_targets_absorption_by_group_and_sabine` (seats SPPS and TCR): version 12; volume 180, area 216; DIN A1-A4 against `a·lg V + b` written out, A5 refused `params_din_out_of_range`, A3 reads 0.55; ids 21/22/25 with faces 2/8/2, areas 60/96/60, `α` as f32, `S·α`; band totals 43.2 and equal to TCR's own `A_Sabine`; Sabine against `K·V/(4mV + ΣSα)` | `red-room-cli.txt` (version 11) | `green-room-cli.txt`: cli_results 26 passed |
| `report.rs` required-fields pin (backlog 47) | digest changed | pinned at (12, `667ef5d3…`) |
| app `results_data::tests::the_surface_groups_name_the_reports_material_ids` | `red-app-names.txt` (no `group_names`) | `green-app-names.txt`; app 69 |
| UI `features/acoustics/model.test.ts` (10): only `report.bed` PASS shown (and the other set when the bed says so), a status other than `PASS` hides; value with range and status as paths at its decimals; refusal as code and kind; a value with no range not shown alone; STI's note; EDT's unchecked note; RT series as values with gaps; DIN by group with volume; bands as numbers of `bands_hz` (kHz scaled); absorption, classical table, decay; the variant's newest OK run | `red-ui-model.txt` (module not found) | `green-ui-model.txt`: 10 passed; UI 169 |
| harness `e2e/lib/acoustics.test.ts` (7): number at precision (and a digit off, the wrong precision, a refusal read as a number, a unit inside, a missing scale say no), strings, stray digits, labels as words, series path for path | written with the rules | 7 passed; harness 52 |
| e2e `m12.acoustics` (m12-a, b, e, f) | `red-e2e-acoustics.txt`: the before hook found no `acousticsView`, 4 not passed | `gate-e2e-5.log`: 4 of 4 |

## The gate's ids, as run (`m11.ps1 -Only e2e -Spec m12.acoustics`, `gate-e2e-5.log`)

- **(a)** box run (`box_run.simpa`, SPPS, 150,000 particles): **1,916 numbers and 532 strings compared over every
  band (and the bands summed), receiver and DIN group, 0 mismatches**; no digit anywhere else on the tab; every
  option of the three controls is the report's; 21 receiver-parameter cells, each with its range and status or its
  refusal; MQ2's words exactly; "validated" in no text or tooltip on the page. Control: a planted wrong digit is
  caught by the same comparison.
- **(b)** not PASS: `edt_s`, `t30_s`, `g_db`, `dba`: 0 `[data-param]` elements and their names in neither the tab's
  nor the Results panel's text, under every selection, and not drawn; the report's `bed` equals the file. Control:
  all seven PASS parameters have elements.
- **(e)** `0.55` s for A3, from `room.din18041.2.target_s.value`, at `180` m³. Control: A1 reads its own `1.08`.
- **(f)** baseline run and a "Curtain" variant (rear wall in a 0.6 curtain): the tab follows the variant switch to
  the newest OK run of that variant; the T20 series drawn and every number shown are that run's JSON, 0 mismatches,
  baseline → Curtain → baseline. Control: the two series differ (1.78 s vs 1.01 s at 125 Hz).

## m12.ps1, m11.ps1

- The spec runs on m11.ps1's harness, as P3's does (`m11.ps1 -Only e2e -Spec m12.acoustics`): P3's dotted-name
  change to `m11.conf.ts` and `m11.ps1` is applied here byte for byte, with one more line in `$specIds`. m12.ps1 gets
  a P2 block in P3's shape (static: `cli_results m12_`, `the_surface_groups`, the two node suites; e2e: the run and
  one check per id read from its verdict lines). P4 merges the two blocks.
- `m11-sim-numbers` and `m11-sim-link` (P1's open item): the Results panel's text and `[data-result]` count are read
  with the run label and `[data-results-region]` removed, so a number may sit only in a Results region; on the
  Simulate step the Acoustics tab and every Results region are asserted digit-free, so a number off the Results step
  still fails. The Results panel itself still holds no digit (the values are in the dock).

## Suite

- Workspace (`cargo test --workspace --no-fail-fast`, 2 test threads, `suite-1.log`, 13:40-13:51, at `73c897f`):
  **1103 passed, 0 failed, 40 ignored** in 159 binaries. P1's 1101 plus the two added here (cli_results' room test,
  app's group names); the core's lib count is unchanged (the pin test was updated, not added).
- UI `npm test`: 169 passed (159 + 10). Harness `node --test e2e/lib/*.test.ts`: 52 passed (+7). `npm run typecheck`,
  the e2e `tsc`, `cargo fmt --check`, both clippys `-D warnings`: clean.
- **Final `m12.ps1` in full** (`-TargetDir C:\tmp\nm-target-h -BedData B:\data`, `gate-final-m12.log`,
  13:51-14:07, at `0a7c8bc`, this file committed before it): P1's six checks, P2's three static checks, the P2 e2e
  and **m12-a, m12-b, m12-e, m12-f PASS**; prior gate **M11 PASSED** (core selection 919 passed / 0 failed / 36
  ignored in 86 binaries; e2e 37 of 37 required ids, the narrowed m11-sim-numbers and m11-sim-link among them; m10
  and "M9 PASSED"; m11-focus); 0 files left in the repository. It prints **"M12 FAILED: 2 check(s)"**: m12-c and
  m12-d NOT BUILT, which are P3's, on `m12-viewport`; P4's merge brings them.

## Left open

- **T30, EDT, G, dB(A)** stay hidden here by this branch's summary; T30 follows decision 46 once the m12 branch's
  summary is merged. EDT's marks are visible only when EDT passes.
- **STI** on the gate's box (octaves 125 Hz-4 kHz) is refused (no 8 kHz band), so MQ3's note is seen in the header
  mark and the unit test, not on a value, in this e2e.
- **Group names** come from the project as it is open now; a project whose groups were reordered or renamed after a
  run names that run's rows by the new project. The ids and numbers are the run's.
- **RT per band is one receiver's** (chosen), not an average: an average is a number the report does not hold.
- **TCR runs**: the tab shows TCR's own Sabine/Eyring (A, T, L) and every receiver parameter refused
  `no_time_series`; not covered by the e2e.
- The CLI's text mode does not print `room`.
- Layout: the dock is 250 px high, so the five cards scroll sideways; style is still deferred (design README).
