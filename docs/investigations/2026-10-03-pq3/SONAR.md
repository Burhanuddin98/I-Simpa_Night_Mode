# SONAR map: Simulate settings editor (PQ3), 2026-10-03

The seat could not write this file (heredoc quoting failed); its returned map, saved verbatim by the main thread.

- **Settings types** (`crates/simpa-core/src/schema/model.rs`): SolverSettings :892 (for_bands :900), SppsSettings
  :915 (for_bands :970, defaults :972-990: extinction_exponent 7.0, particles 150000, saved 0, 10 s, 1 ms, Energetic,
  per-band maps true, echogram_per_source false, bands_computed all true), TcrSettings :998 (for_bands :1006),
  Environment :822 (Default :838), ComputationMethod :857. `config_xml/write.rs:233` (SPPS attrs :303-325, TCR :343,
  docalc :357, atmosphere :366). `config_xml/import.rs:414,430`. `geometry/import/proj.rs` read_solvers :2662 (legacy
  fallbacks 2 s / 10 ms / random / eps 5 at :2731).
- **Parity rows:** `parity-matrix.md:223-243` (C7, C8, C10, C11, C12, C21, C22, C25, C26, C27; all S). Before v1:
  C7, C10, C11, C25, C27 (:601-605); C8, C12, C21, C22 (:558-561); C26 (:410).
- **UI panel:** `app/ui/src/features/simulate/SimulatePanel.tsx:280` (read-only `.sim-settings` block :314, title
  "Read-only in this version"; useProjectSettings ~:84 reads backend.projectJson). Rows from `simulate/model.ts`:
  settingsRows :343, projectSettings :276 (particles, duration, time step, bands, air only).
- **Edit IPC:** `edit_apply` (`app/src-tauri/src/commands.rs:387` -> `bridge.rs:521`; UI `backend.ts:103` editApply;
  wrapper `actions.ts` apply(op, fieldKey) ~:217) returns {applied, refusals, state}. Undo/redo `edit_undo`/`edit_redo`
  (commands.rs:396/402). Dirty = sceneStore info.dirty (actions.ts:321). Ops exist in `schema/ops.rs`:
  SetSolverSettings :216 (whole SolverSettings, integrity::check_solver_settings), SetBandComputed, SetEnvironment
  :213. Nothing in `app/ui/src` uses them yet.
- **Upstream limits:** nbparticules min 1 (e_core_sppscore.h:158; spps core_configuration.cpp:21 clamps <1 to 1,
  nbparticules_rendu <0 to 0); duree_simulation min 0.0001, default 2 (e_core_core_config.h:73); pasdetemps default
  0.01, no min (:74); rayon_recepteurp min EPSILON, default .31 (sppscore.h:54); computation_method 0/1 (:163). Core
  refuses steps >= 65,536 (validate/project.rs:557, MAX_TIME_STEPS), receiver radius <= 0 (:321-371), extinction
  exponent not > 0 and finite (:611). Integers capped at i32::MAX (SOLVER_INT_MAX, model.rs:27; integrity.rs:103).
- **Gaps:** (1) no UI inputs or op builders for SetSolverSettings / SetBandComputed / SetEnvironment (no ops.ts
  builder); (2) no band picker or presets (C25/C26), no "auto" particle count (C7), no "all bands off" refusal, no
  default-on echogram for 2+ sources (C22); (3) temperature, humidity, pressure unchecked; time-step/steps is a
  validate issue, not an apply-time refusal (apply only runs integrity).
- **Tests/gates:** unit tests `*.test.ts` beside source (`simulate/model.test.ts`); e2e
  `app/e2e/specs/m11.simulate.e2e.ts` with `m11.conf.ts`; fixtures `tests/fixtures/ui/*.simpa`; gate
  `tools/gates/m11.ps1` (-Only static|e2e, -Spec simulate, -SkipCore, -SkipPrior).

Main-thread additions: no IPC exposes `Project::rebanded` (`schema/ops.rs:638`; only tests call it). Upstream's band
presets (`e_core_core_bfreqselection.h:90-140`): none, all; octave all 63-16000, octave Building/Road 125-4000;
third-octave all 50-20000, third-octave Building/Road 100-5000. Upstream's `.pbin` size warning (`e_core_sppscore.h`,
Modified): saved x (duration / step) x 16 bytes x active sources, per band.
