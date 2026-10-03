# PQ3 build: the Simulate settings editor (2026-10-03)

Branch `pq3-settings` off `rebuild` @ `83a6ad8`, worktree `.claude/worktrees/pq3-settings`. Spec: `PLAN.md` beside
this file. Progress log and every receipt below: `B:\data\pq3\`.

## What it means

The Simulate panel's settings are now fields. Every value the editor lets through goes into `config.xml` as typed
(the round-trip test, and a real run read back in the e2e), and the values the solver would mishandle silently are
refused under the field before Run: zero particles, saved above computed, a step count past 65,535, non-physical air,
and, new, a solver with every band off. Air outside ISO 9613-1's stated range is warned, not refused. Call 3's probe
came back bit-identical, so new projects now write an echogram per source.

## Commits

| Commit | What |
|---|---|
| `74598be` | Core: `no_band_computed` (error, judged only for the solver launched: `Context::solver`, `validate::solver_issues`), `atmosphere_outside_formula_range` (warning), the run manager passes its solver, contract page rows (45 rules: 38 project, 41 errors, 4 warnings), two negative fixtures, `tests/settings_editor.rs` |
| `e8b8f05` | App: `SceneState::solver_issues` (per solver), `Session::solver_blockers`, `run_start` checks them, `edit_reband` (38th command; m11.ps1 inventory updated), bindings regenerated |
| `fd95b95` | Call 3: `echogram_per_source` on in `SppsSettings::for_bands`; a `.proj` with no `output_recp_bysource` keeps upstream's off |
| `b1c5a83` | rustfmt of `params_sti.rs` (pre-existing, `d268327`): m11.ps1's `rustfmt --check` failed on `rebuild` as it stood |
| `939cac0` | rustfmt of the PQ3 tests |
| `7317732` | `cube.simpa` and the UI run fixtures pinned to echogram per source off, as for rows 11, 36, 38, 41 |
| `c71d218` | UI: `SettingsEditor.tsx`, `settings.ts`, op builders, `actions.reband`, `flow.projectBlockers` used by Run, F5, the menu and the hooks |
| `7c47e0d` | e2e: `m11.settings.e2e.ts` (3 ids) in m11.ps1; `m11-sim-preflight` reads input values instead of `[data-input]` text |
| `d15854d` | clippy: two `.clone()` of a `Copy` type in a `report.rs` test (pre-existing, `ceaf1375`): m11.ps1's core clippy failed on `rebuild` |

## Tests added, red then green

| Test | Red (receipt in `B:\data\pq3\`) | Green |
|---|---|---|
| `crates/simpa-core/tests/settings_editor.rs` (7): every band off blocks only the chosen solver; one band on runs; non-physical air refused; air outside the formula's range warned; **the round trip** (C7, C8, C10, C11, C12, C21, C22, C25, C26, C27 through their ops, as JSON text through `Op::from_json`, each the `config.xml` attribute, save/load bit for bit, undo to the original bytes); an import keeps its own values; a new project's defaults | `red-step1-settings_editor.txt`: 6 compile errors, the API did not exist | 7 passed |
| `run_manager.rs` `every_band_off_refuses_that_solver_s_run_and_not_the_other_s` | `red-step1-run_manager.txt`: FAILED (the SPPS run was not refused before the manager passed its solver) | passed |
| `proj.rs` `an_imported_project_keeps_its_own_echogram_per_source_not_night_mode_s_default` | `red-call3.txt`: FAILED | passed |
| app `bridge::pq3_tests` (4): every band off is applied and blocks only that solver; air refused inline, out-of-range a warning; a preset is one edit and one undo restores it bit for bit (and all five presets apply); a bad range is refused | `red-step1-app.txt`: 15 compile errors | 4 passed (app crate 59) |
| UI `settings.test.ts` (7): `parseCount`, `moveDecimalPoint`, ms to s bit for bit (`2.1 / 1000` is not 0.0021; the text shift is), step count, `.pbin` size, presets, ops | `red-step3-ui.txt`: module not found | 7 passed |
| UI `flow.test.ts` `projectBlockers` | SyntaxError: no export | passed (UI 154 of 154) |
| e2e `m11.settings.e2e.ts`: `pq3-settings-edit`, `pq3-settings-bands-off`, `pq3-settings-preset` | written after the UI | 3 of 3 (below) |

Deviation: the UI unit tests are in `simulate/settings.test.ts` beside a new `settings.ts`, not in `model.test.ts`;
the same `npm test` glob runs them. The round-trip test passed the first time it compiled (it tests ops and a writer
that already existed); its red is the shared compile failure above.

## Probe for call 3 (echogram per source)

`tests/fixtures/rooms/tutorial1_box_seeded.simpa` (the teaching room, one source, two receivers, seed 1, random,
10,000 particles, 27 third-octave bands), run with `simpa run --solver spps`, the flag off and on (the only byte
changed in the project). Folders: `B:\data\pq3\probe-c22\runs-{off,on}`; comparison: `probe-c22-compare.txt`.

- `simpa results --json`: **27,031 leaf values compared, 0 differ** (skipped only the run folder, the start time, the
  flag itself and the new `per_source` entries: 0 per receiver off, 1 on).
- Files: 83 off, 85 on (the two added are `Receiver n/Source 1/Sound level.recp`, each byte-identical to that
  receiver's total). 52 common files byte-identical; the 28 `.csbin` differ only in uninitialised struct padding
  (`docs/formats/README.md:47`) and their decoded dumps are identical (28 of 28); `config.xml` differs only in
  `output_recp_bysource`; `run.json`/`mesh.json` only in paths, hashes, file counts and elapsed time. Verdict OK both,
  line counts identical.

Bit-identical, so the default was built (`fd95b95`). Legacy `.proj` imports and `config.xml` imports keep off.

## Gates

- Full core suite (`cargo test -p simpa-core -p simpa`, nothing skipped, 4 threads): **1011 passed, 0 failed, 40
  ignored** (`full-run2.log`). With the app crate's **59**: 1070. The baseline 1057 is 1002 core + 55 app; the
  difference is the 9 core and 4 app tests added here.
- `tools/gates/m11.ps1 -TargetDir C:	mp
m-target-h`, in full (`m11-full-1.log`, 09:18-09:31): all 32 checks PASS
  (static and lints, app 59, both clippys, rustfmt, UI fixtures, the gate's core selection 896 passed / 0 failed / 36
  ignored in 84 binaries, release build, harness, e2e, prior gates m10 `-SkipCore` exit 0 and m9 "M9 PASSED",
  m11-focus), and it printed "M11 FAILED: 1 check(s)" for the last one: this BUILD.md, untracked while the gate ran,
  was a file left in the repository. Rerun with it committed: RERUN_RESULT
- e2e, inside that gate: **35 of 35 required ids passed**, 0 failures, 0 skipped (all ten specs; `settings` adds
  3). A first `-Only e2e -Spec simulate,settings` run (`m11-e2e-1.log`) passed 9 of 9.

Receipts from the e2e: particles 150000 -> 12000, time step 0.01 -> 0.005 s, band 250 Hz off; the run's
`config.xml` read `nbparticules=12000 pasdetemps=0.005` and docalc `1,0,1,1,1,1`. Every band off: Run (panel and
top) carried `NO_BAND_COMPUTED`, on the FAIL row and under the bands; TCR was not blocked. The third-octave
100 Hz–5 kHz preset gave 18 bands and one undo gave back the project text exactly.

## Left open

- Two gate checks failed on `rebuild` before this work (rustfmt `d268327`, clippy `ceaf1375`); fixed here as
  separate commits, formatting and a no-op clone only. Merging `rebuild` elsewhere without them keeps m11.ps1 red.
- `-20 °C` exactly is warned as outside ISO 9613-1's range: `params::air::stated_accuracy` converts to kelvin in
  f64, and -20 + 273.15 is 253.14999999999998 K. The test uses -19.5 °C and says why. Not changed: `stated_accuracy`
  is shared with the parameters.
- `simpa validate` names no solver, so it does not report `no_band_computed`; a run, the app's Run and the app's
  per-solver blockers do.
- The energetic-EDT "not validated" mark is not shown anywhere in the UI today (M12 shows results); the method
  field's hint says it, so it is not lost when the method changes.
- `EXEMPT_REGIONS['data-input']['simulate-settings']` (`app/e2e/lib/dom.ts`) is now unused: the settings block has
  no `[data-input]`. Left in place.
- Calls 1-5 go into `docs/decision-log.md` at the merge (PLAN.md), not done here.
