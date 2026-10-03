# PQ3 assay (83a6ad8..5a287de), read-only audit, no suite rerun

VERDICT: SHIP-WITH-FIXES (0 HIGH, 0 MED, 3 LOW)

1. 7317732: only adds `echogram_per_source = false` to cube_project, teaching_room, hall_run (schema_roundtrip.rs:1598, ui_fixtures.rs:146,426). No assertion loosened or deleted. OK.
2. b1c5a83 / d15854d: params_sti.rs is two assert! reflows (same args); report.rs drops two `.clone()` of a Copy type. No behaviour change. OK.
3. Round trip (tests/settings_editor.rs:259-352): covers C7 nbparticules, C8 nbparticules_rendu, C10 duree_simulation, C11 pasdetemps, C12 computation_method, C21 output_recs_byfreq, C22 output_recp_bysource, C25 docalc per solver, C26 reband (freqs + per-band), C27 temperature/humidite/pression, as JSON text via Op::from_json, bit-for-bit f64, save/load, undo to original bytes. All table rows present. write.rs:325 attr matches.
4. Refusals: no_band_computed only via Context.solver / solver_issues(chosen); run_project passes opts.solver; app plan() merges per-solver blockers. Air refusals (T > -273.15 with f32 guard, h 0..=100, p > 0) are errors; formula_range is a W. saved<=n, steps>=65536 refused (existing, project.rs:583). No silent pass found.
5. edit_reband: via Project::rebanded -> edit_apply; app test asserts project == original and json bytes equal after undo, redo, all five presets. UI has a confirm (reband-confirm / reband-apply) before actions.reband.
6. Imports: proj.rs reads output_recp_bysource, absent -> false (test covers on/off/absent); config_xml/import.rs:434 defaults false. Editor writes nothing on open.
7. SettingsEditor uses CommitInput (<input>), no [data-input]; the unused EXEMPT_REGIONS entry in dom.ts is harmless.
8. -20 C edge: real but LOW. ISO 9613-1 clause 7 range is inclusive of -20 C; air.rs:158 compares 253.15 <= k and kelvin is 253.14999999999998, so exactly -20 C raises a false "outside range" warning (stated_accuracy is shared; fix there with a tolerance or compare in C).

LOW-1 project.rs:115 and :766: message strings carry a ~14-space run mid-sentence (continuation backslash missing); shows in user-facing text and contract fixtures.
LOW-2 `simpa validate` names no solver, so no_band_computed never appears there (documented by builder).
LOW-3 -20 C warning edge above.

EXCESS: none
