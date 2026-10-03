# PQ3 plan: the Simulate settings editor (2026-10-03)

The v1 piece named in `docs/scope.md` row 15 (2) and M11 PLAN section 10, PQ3: the Simulate panel's settings become
editable. Receipts: `KEEL.md`, `SONAR.md` beside this file. Branch `pq3-settings` off `rebuild`, worktree
`.claude/worktrees/pq3-settings`.

## What "done" means (the load-bearing question, KEEL)

Every value the editor lets through reaches `config.xml` as a value the solver honours, and every value the solver
would mishandle without saying so is refused inline before Run. "The fields are editable" is not the bar.

## Scope

In v1 (parity rows, `parity-matrix.md:223-243`):

| Item | Field | Op | Rule |
|---|---|---|---|
| C7 | Particles per source and band | SetSolverSettings | integer 1..=2,147,483,647. The true number, never "auto": the design's "auto" (design:668, review deviation 17) is dropped |
| C8 | Particles saved for playback | SetSolverSettings | 0..=particles (validator exists). Shows the file size before it is turned on (upstream's formula: saved x steps x 16 B x active sources, per band) |
| C10 | Duration | SetSolverSettings | > 0 and steps < 65,536 (validator exists) |
| C11 | Time step | SetSolverSettings | > 0 and steps < 65,536; the step count is shown |
| C12 | Method: Energetic or Random | SetSolverSettings | One line from upstream's docs: Random for drafts, Energetic for final results. The energetic-EDT "not validated" mark (row 37 (2)) stays wherever it is shown today |
| C21 | Sound maps per band | SetSolverSettings | checkbox; shows the stored value, never flips it |
| C22 | Echogram per source | SetSolverSettings | checkbox; name-collision validator exists (validate/project.rs:772) |
| C25 | Bands computed, per solver | SetBandComputed | **new refusal `no_band_computed`** when the chosen solver computes no band: Run is blocked |
| C26 | Band presets | new IPC `edit_reband` over `Project::rebanded` | upstream's four (octave 63-16000, octave 125-4000, third-octave 50-20000, third-octave 100-5000) plus ours, octave 125-8000 (decision 43). A confirm first: per-band material, source and fitting values are taken from the nearest current band; one undo restores |
| C27 | Air: temperature, humidity, pressure | SetEnvironment | refused when non-physical (humidity outside 0-100 %, pressure <= 0, temperature <= -273.15 °C); a warning, not a refusal, outside the range of the air-absorption formula the solver uses (builder reads upstream's implementation for that range) |

Not in v1 (parity has them v1.x): trans_epsilon (C13), the other SPPS switches (C9, C14-C20), TCR air on/off (C23).

## Calls (Jarvis; into `docs/decision-log.md` at the merge)

1. **trans_epsilon is not shown.** Parity defers it (C13, :580); a low value reads T30 short (eps 3: 11.8 %,
   params.md:371). New projects keep 7, imports their own.
2. **Particles saved stays 0 for new projects.** At the defaults 1,000 saved particles is about 160 MB per band per
   source (1.1 GB a run at 7 bands), by upstream's own formula. M12's playback asks the user to turn it on and shows
   the size; it does not get a silent default.
3. **Echogram per source on for new projects**, if a probe shows it changes nothing else: one seeded run of the
   teaching room with one source, the flag on and off, total echograms and every report value bit-identical. If not
   identical, it stays off and this call is reported, not built.
4. **Imports keep their own values.** The editor shows the project's values and never writes a default over them
   (rows 11, 41, 43).
5. **Band presets go through core.** The UI cannot build `BandData`; `edit_reband(kind, lowest_hz, highest_hz)`
   computes `Project::rebanded` in core and applies it as one undoable op.

## Order of work (tests first, each red before green)

1. Core: `no_band_computed` validator code (+ test: every band off for the chosen solver blocks Run, the other
   solver's state does not); air-condition refusals and the range warning (+ tests); `edit_reband` command (+ test:
   apply then undo is `==` the original project, bit for bit). Default change of call 3 after its probe.
2. Core round trip, the load-bearing test: for every editable field, a value set by its op is the attribute written
   to `config.xml` (`config_xml/write.rs`), and save/load keeps it bit for bit.
3. UI: op builders in `ops.ts`; the settings block in `SimulatePanel.tsx` becomes inputs (strict decimal parse,
   `numbers.ts`; refusals inline via `fieldKey`, the SourcesPanel pattern); edits go through `actions.apply`, mark
   dirty, undo. Unit tests in `simulate/model.test.ts` (parsing, step count, `.pbin` size, preset list).
   Editable fields must not sit inside the m11-h `[data-input]` exemption (GATE.md F2).
4. e2e (`app/e2e/specs/`, new spec beside `m11.simulate.e2e.ts`): edit particles, time step and one band off; Run
   (PQ1 saves first); the run's `config.xml` carries the edited values; all bands off disables Run with the refusal
   shown; a preset applies and one undo restores.
5. Gates: `tools/gates/m11.ps1` (it runs m10 and m9) passes; the full core suite passes (baseline 1057).

## Traps (paid for)

Ops travel as text through `Op::from_json` (serde_json's reader is not correctly rounded). Builds go to `C:\tmp\nm-target-h`
(disposable, reused), `CARGO_INCREMENTAL=0`, no debuginfo. Test env: `SIMPA_SOLVERS_DIR=C:\tmp\nm-m8a-solvers`,
`SIMPA_UPSTREAM=B:\repos\I-Simpa-upstream`, `SIMPA_TETGEN160=C:\tmp\nm-m10-solvers\build\src\tetgen\Release\tetgen.exe`.
A full suite needs a Bash timeout above 600000 ms (use 5400000). tauri-driver's port 4444 is one e2e at a time.
Run outputs go to `B:\data\`, never C:.
