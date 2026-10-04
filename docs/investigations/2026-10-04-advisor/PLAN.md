# Backlog 80 plan: the run-quality advisor (2026-10-04)

Spec: `docs/v1.1-backlog.md` row 80, decision 49, with decision 48 (`range_below_zero`) and rows 81-86 beside it.
Receipts: `B:\data\m12\b78-mesh\FINDINGS.md` (arms A-H) and `B:\data\m12\pearl-geom\cr2-solve\comparison.md` (CR2: T30
refused `missing_moves` from 500 Hz at 0.04 % lost). Branch `b80` off `glass`. Outputs go to `B:\data\m12\advisor\`.
Status: PLAN, not started; Burhan reads it first (his 11:45 "Start the advisor plan").

## What "done" means

Row 80: tutorial 2's Elmia as upstream ships it gets both warnings before its run: "this meshing splits the walls;
expect lost particles" (`-q2` without `-Y`) and "receivers small for this room" (0.31 m). After a run, a
`range_below_zero` T30 names its cause (Monte-Carlo noise) and its fix: a larger receiver radius that keeps every
sphere inside the walls. "Apply and re-run" makes that edit as one undo step, and the re-run shows the T30. An e2e.

In every case the advisor names a cause and a setting, never a value the run will produce: the noise fell 9x (B to D)
and 15x (B to E) where the square-root rule forecast 3.2x.

## Where the advice is computed, and why

In the core, a new module `crates/simpa-core/src/advise.rs` (`advise/before.rs`, `advise/after.rs`), not in the UI:
1. Gate M12 (a): every number on the Results screen is a path into `simpa results --json`. A proposed radius,
   particle count or duration is a number, so it lives in the report (`report.advice`, results version 14).
2. The CLI gets the same advice: `simpa results <run> --json` after a run, a new `simpa advise <project> [--json]`
   before it. `simpa validate --json` stays as it is.
3. The mapping is an exhaustive `match` on `NotEvaluable` (params.rs) and `edt::REFUSAL_REASONS`, with no `_ =>`
   arm: a new refusal kind fails to compile until it has advice.

Not a validate rule: `validate::RULES` is the solver contract (what reaches the solver, codes that are API). The
advisor's checks are quality judgements with their own stable list, `advise::CODES`.

## The refusal table

Kinds in `crates/simpa-core/src/params.rs`. "No Apply": the cause and setting are named, no op is offered.

| kind | emitted at | cause in plain words | fix (setting) | Apply |
|---|---|---|---|---|
| `monte_carlo_noise` | noise.rs | Monte-Carlo noise: too few particle crossings of this receiver | 1) receiver radius up to the cap; 2) particles: `particle_count` Named/Resampled is the core's count; BeyondResampled / ResampledNotConfirmed / ScalingNotConfirmed / NoStandardDeviation name none ("more particles may not help") | radius yes; particles Q1 |
| `range_below_zero` | noise.rs; EDT report.rs | noise so large the range crosses zero | as `monte_carlo_noise`, radius first | yes |
| `wide` status | noise::shown_with | noise wider than the difference limen; `straddle` (C50/C80/D50) is the bin at te | noise: radius, then particles; straddle: finer time step | yes |
| `noise_uncalibrated` | noise.rs | the run is outside what the noise model was measured on | `particles_at_least` (a calibration minimum, not a forecast); `receiver_radius_scale_at_most` (a smaller radius) | yes |
| `noise_unknown` | noise.rs | the noise cannot be estimated | none | no |
| `missing_moves`, `missing_not_cleared` | decay.rs | `floor_db`: particles dropped at the extinction floor; `lost_share`: particles lost by meshing (cap 10) | floor: trans_epsilon up to 7 (decision 41); lost and meshed without `-Y` (`<run>/mesh/mesh.json` argv): `-Y`; lost with `-Y` already: no setting fixes it in this build (backlog 82) | floor/-Y yes; else no |
| `truncated`, `range_not_reached` | decay.rs | the run ended before the decay did | duration up to the new-project 10 s, or what the step limit allows (< 65,536 steps) | yes |
| `unresolved`, `early_unresolved` | decay.rs | the onset bin is too coarse | time step down to 1 ms (decision 11) | yes |
| `range_too_short`, `not_decaying`, `empty_window` | decay.rs | the decay cannot be fitted here | none | no |
| `several_sources` | report.rs | several sources summed; ISO 3382-1 is per source | echogram per source on; if on: "choose a source in the Source picker" | yes / no |
| `no_time_series` | report.rs | TCR writes no echogram | "use SPPS" (a choice, not an op) | no |
| `edt_refused` (reasons edt.rs) | report.rs | `run_too_short`/`not_decaying_at_run_end`: truncated; `step_too_coarse`; `too_few_particles`; `receiver_too_large`; others none | duration; time step; particles; radius <= 1 m; none | per reason |
| `no_a_weight`, `band_missing`, `not_octave_bands` | level.rs; sti.rs | the run's bands do not cover the quantity | octaves 125 Hz to 8 kHz | name only (`actions.reband`) |
| `band_refused` | sti.rs | inherits its band's refusal | that band's advice | as that |
| other ParamError codes | | input faults validation stops | none | no |

Advice is grouped by cause and fix: each item carries `code`, `cause`, `fix {setting pointer, from, to | null,
why_no_apply}` and `values` (the report paths it explains), so a refused cell points to its item and the screen shows
each cause once with a count.

The radius cap is the minimum of three bounds, each with a receipt:
- below the smallest receiver-to-face clearance (the `geometry::locate` that `receiver_sphere_crosses_surface` uses,
  validate/project.rs), rounded down to 0.05 m (Elmia: 0.647 to 0.6, the fixture's value);
- at most 1.0 m (EDT is unchecked above 1 m);
- at most sqrt(max_crossings_per_particle / n) x r (noise.rs calibration domain).

After a run the clearance comes from the run's own `.cbin` and `config.xml` (as `results/room.rs` reads them), so
the CLI needs no project; before a run, from the project.

## Pre-run rules (`advise::before(&Project)`)

1. `mesh_splits_walls`: `meshing.preserve_boundary` false. "this meshing splits the walls; expect lost particles".
   Fix: `preserve_boundary` true. Receipt: arms A vs B/C (119-171 vs 28-54 lost per band).
2. `receivers_small`: crossings measure `N r^2 / V` below K, V from the geometry check (backlog 85's 0.2 % noted).
   Fix: the radius cap when above r, else particles. K is set in T0. Known: B 1.39 (bad); D 13.9, E 14.4, H 34.7
   (good); tutorial 2 as shipped 9.25 (unmeasured).
3. `run_short`: duration below the longest computed band's 60 dB decay (the larger of Sabine and Eyring from the
   project). Fix: duration.
4. `particles_few`: particles per source below the calibration minimum for a shown quantity (150,000 for energetic
   T30). Fix: that count.

`receiver_sphere_crosses_surface` stays a validate warning carried into `run.json`. The advisor never proposes a
radius that would raise it. Today the Simulate preflight row lists it but never fails on it (only errors reach
`run_blockers`); the advisor row shows it.

## The "Apply" ops

Every fix is a field of `SolverSettings` (schema/model.rs, meshing included), so Apply is one
`Op::SetSolverSettings` (UI `ops.ts setSolverSettings`) built from the current settings with one field patched,
through the checked apply (`actions.apply`): one undo step, and validation refuses an edit that introduces an error
(`step_count_overflow`, `mesh_settings_conflict`). Bands go through `actions.reband` (name only). Ops travel as text
through `Op::from_json` (PQ3 trap).

A new Tauri command `advice_apply(code, run | null)` patches the current project, refusing with a reason when the
project's value is no longer the run's ("the project changed since this run"), then `runStart`. Each run meshes again
in `<run>/mesh`, so a `-Y` edit needs no extra step.

## UI surfaces

- Simulate step (`features/simulate/SimulatePanel.tsx` PreflightList, `model.ts` PREFLIGHT/preflightRows): a "Run
  quality" list under "Before running": words and the setting's name, never blocks Run, an Apply per item.
- `SettingsEditor.tsx` gets receiver radius, trans_epsilon and "Preserve walls when meshing (-Y)", so every value an
  Apply sets is visible and editable (today it shows none of the three).
- Results (`features/acoustics/AcousticsPane.tsx`, `model.ts`): a "Why values are missing" card from
  `report.advice`, numbers `[data-num][data-json]` per gate (a); each refused or `wide` cell gets
  `data-advice=<code>`; the card holds "Apply and re-run". `ResultsPanel.tsx` gets a one-line pointer.
- Backend: `scene.rs` (SceneState `advice`, beside `issues`), `commands.rs run_report` (report v14),
  `results_data.rs`, bindings.

## Must NOT

- Auto-tune: one click is one edit and one run, never a loop or a search.
- Change a value silently: every Apply shows from -> to, is one undo step, goes through the checked apply; no setting
  changes without the click; new-project defaults untouched (backlog 83 stays open).
- Predict a value: no "T30 will be +-x %", no forecast noise or run time, no extrapolation from sqrt(N). The only
  counts it names are the core's own (calibration minimums, or ParticleCount per Q1).
- Hide or soften a refusal: advice sits beside it; a `wide` value keeps its range.
- Offer a fix outside a checked domain (radius above 1 m or past a wall, steps over 65,535, a setting the validator
  refuses), or "more particles" for lost-particle shares (the share does not change with N).
- Edit geometry, materials, sources or receiver positions.

## Gates (what makes each check say NO)

- B1 `mesh_splits_walls`: absent with `-Y`; with a surface-receiver refinement set, Apply is not offered (Q3) and the
  conflict is named.
- B2 `receivers_small`: absent at or above K; never proposes a radius >= any receiver's clearance or > 1 m (Elmia
  <= 0.6); when the cap <= r, the fix is particles.
- B3 `run_short`: absent when duration >= RT60; absent, not guessed, when Sabine is refused.
- B4 `particles_few`: absent at or above the calibration minimum.
- A1: an `ok` value carries no advice; a refusal with no setting fix carries its cause and no Apply; lost-share
  `missing_moves` under `-Y` names backlog 82, with no particles and no `-Y` advice.
- A2: every number in an advice item is a report path (the m12 gate (a) checker); the match is exhaustive.
- A3: Apply is refused when the project changed since the run; undo restores the run's settings exactly.
- UI: the m10-h/m11-h scanner (`app/e2e/lib/acoustic.ts`, `dom.ts ACOUSTIC_NUMBER`) passes with the advice on the
  Simulate step: no "<n> s/ms/dB/%" and no parameter name followed by a number in pre-run text ("0.31 m" is allowed).

## Order of work (tests first; each test fails before its code)

- T0 Receipts, no product code: a script under `tools/advisor/` tabulates `N r^2 / V`, duration/RT60, -Y and
  min clearance against T30 ok/wide/refused from arms A-H and the teaching-room runs; one new run (tutorial 2's 1M
  particles and 0.31 m with corrected meshing, 10 s, 1 ms, eps 7: arm I, about 100 s). K from this table, written to
  `B:\data\m12\advisor\`.
- T1 `advise` types and `CODES` (serialisation, code stability).
- T2 Pre-run rules B1-B4, say-NO cases first, on tutorial 2 as shipped, the Elmia fixture, the teaching room.
- T3 Post-run mapping, exhaustive: one test per table row on synthetic refusals, plus arm B's stored report (9 T30
  `range_below_zero` -> radius 0.6).
- T4 Report v14: `advice`, `REQUIRED_FIELDS_PIN`, history line, `docs/formats/results-json.md` and schema,
  `cli_results.rs`, `results_data.rs`, `acoustics/model.test.ts`.
- T5 CLI `simpa advise <project> [--json]`, USAGE, a CLI test.
- T6 SettingsEditor: receiver radius, trans_epsilon, "Preserve walls when meshing (-Y)", with `settings.test.ts`.
- T7 Tauri: SceneState `advice`, `advice_apply` (registered in main.rs), bindings, Rust tests incl. refused-when-changed.
- T8 UI pre-run: `adviceRows` in simulate/model.ts with tests, then SimulatePanel.
- T9 UI post-run: `adviceCards` in acoustics/model.ts with tests (paths, `data-advice`), then AcousticsPane,
  ResultsPanel, actions.ts.
- T10 e2e `app/e2e/specs/b80.advisor.e2e.ts`: tutorial 2 as shipped shows `mesh_splits_walls` and
  `receivers_small` before any run; the arm-B fixture (`tests/fixtures/rooms/`, PROVENANCE row): run, the
  `range_below_zero` T30 card names the receiver radius, Apply and re-run, the T30 is shown; undo restores; the m11-h
  scanner stays green. Added to `tools/gates/m12.ps1`.
- T11 Docs: backlog 80 Done with its receipt; a decision-log row for K and Q1-Q2; backlog 83 noted.

## Size

12 tasks, about 27 files (6 new: advise.rs, advise/before.rs, advise/after.rs, the tools/advisor script, the b80 e2e
spec, the Elmia arm-B fixture). Roughly two to three working sessions. The longest single wait is T0's run plus the
e2e runs (Elmia arm B 17 s per run, meshing extra).

## Product questions (each has a default; the build follows it until Burhan says otherwise)

| # | Question | Default |
|---|---|---|
| Q1 | Should Apply set ParticleCount::Named's count, which comes from 1/sqrt(N) and overshoots per the 10-04 runs? | Named and applied, labelled "the count the noise model names" (a setting, not a forecast); no count when the core names none |
| Q2 | The e2e "after" project, since tutorial 2 as shipped refuses T30 `truncated`/`missing_moves`, not `range_below_zero` | The arm-B project (corrected meshing, S01 alone, 150,000, 0.31 m) as a fixture |
| Q3 | Apply -Y when a surface-receiver refinement is set | Not offered; the conflict is named |
| Q4 | K, the "receivers small" threshold | Set in T0 from measured arms; if tutorial 2 (9.25) cannot be separated from D/E (about 14), ask |
| Q5 | Does the advisor cover backlog 83? | Left open; the advisor is shown, the default radius unchanged |

## Traps

- The advice must not trip m10-h/m11-h: on the Simulate step no "10 s", "Sabine 2.07" or "34 %"; show the
  setting's name, the value lives in its `[data-input]` field after Apply.
- No Apply for a setting the user cannot see: T6 comes before T8 and T9.
- Lost-particle `missing_moves` under -Y (backlog 82) has no fix; more particles leaves the share the same.
- The radius is one global SPPS setting: one receiver near a wall caps all of them (Elmia R03 0.647 m). At 1.0 m four
  Elmia spheres crossed walls and their levels read low.
- EDT is unchecked above 1 m; `noise_uncalibrated` can ask for a smaller radius, so the fixes can point opposite
  ways: take the minimum and say which bound bound it.
- `several_sources` stays on the summed view even with echogram per source on: "choose a source", not a setting.
- The calibration covers rooms of 60-1,000 m3 only (`noise::calibration::MEASURED_ON`); Elmia's 10,389 m3 is outside,
  and the advisor must not claim the fix is checked there.
- The run's meshing comes from `<run>/mesh/mesh.json`; a reused or fixture mesh may have none: "meshing not known".
- Report version 14: pin, history line, schema, cli_results, results_data and bindings together. Beds (backlog 81)
  were scored at 13.
- The pre-run volume counts obstacle interiors (backlog 85, about 0.2 %), which matters near K.
- The usual: ops as text through `Op::from_json`; one cargo builder per target dir; one e2e at a time
  (`B:\data\m12\e2e.lock`); suite timeout 5400000 ms; the gate fails on untracked files.
