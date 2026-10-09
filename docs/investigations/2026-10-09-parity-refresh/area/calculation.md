# Parity: 2.3 Calculation, status at 95ec062 (2026-10-09)

R = B:\repos\I-Simpa_Night_Mode\.claude\worktrees\parity-ro. Paths are relative to R.
UI = app/ui/src. SE = UI/features/simulate/SettingsEditor.tsx. SP = UI/features/simulate/SimulatePanel.tsx. CM = crates/simpa-core/src/schema/model.rs.
Method: read SettingsEditor.tsx in full, the settings structs in model.rs, grep of UI and tauri for every SPPS/TCR/mesh field name, validate/project.rs, run/manager.rs, RunsPane.tsx, all of v1.1-backlog.md, decision-log, scope.md. Nothing was run (read-only), so "Ready" means the path is wired in the UI by reading the code.
Rows C1-C41 are matrix lines 213-258.

## 1. Table

| ID | Feature | Imp. | 09-25 | Now | Receipt | Note |
|---|---|---|---|---|---|---|
| C1 | Run SPPS from project | core | Planned | Ready | UI/chrome/RunButton.tsx; MenuBar Simulate > Run, F5; app/src-tauri/src/commands.rs:493 `run_start`; SP solver choice | Run is enabled now (matrix said disabled) |
| C2 | TCR classical theory | common | Planned | Ready | SE:321-335 (TCR block: method, bands, air); SP solver choice `tcr` | Method row is read-only text "Sabine · Eyring" |
| C3 | TLM entry | niche | Not porting | Not porting | CM SolverKind Spps/Tcr only (ops.rs:61-63) | No decision-log row names TLM; reason is "no upstream solver exists" (matrix) |
| C4 | Diffusion core md_octave | niche | Absent | Absent | none; scope.md:71 lists embedded Python as not in v1 | Not installed by upstream's Windows build |
| C5 | Plugin calculation cores | niche | Absent | Absent | ops.rs:61-63; backlog V2-5 (scripting API, v2) | |
| C6 | TCR external-core mode | niche | Absent | Absent | write.rs writes GUI mode only | Not reachable from upstream GUI |
| C7 | Particles per source | core | Design only | Ready | SE:376-386 (parseCount, refusals inline); CM:926 | "auto" never built: a number, default 150,000 |
| C8 | Particles saved | common | Core only | Ready | SE:387-401 with .pbin size readout (SE:349-356) | Default still 0 (CM `for_bands`), user must set it for playback |
| C9 | Collision statistics CSVs | niche | Ready | Core only | CM `save_surface_intersections`, `save_receiver_intersections` (default true); no UI hit | Matrix called it Ready (core); no toggle, not in upstream GUI either |
| C10 | Simulation length | core | Design only | Ready | SE:402-413 | |
| C11 | Time step | core | Design only | Ready | SE:414-429, ms entry + step count | Plus a sound-map time step field SE:430-449 (new) |
| C12 | Method Random/Energetic | common | Core only | Ready | SE:481-501 | Default Energetic |
| C13 | Particle extinction limit | niche | Core only | Ready | SE:462-473 (`extinction_exponent` = upstream `trans_epsilon`) | |
| C14 | SPPS air absorption on/off | common | Core only | Core only | CM `spps.air_absorption`; read-only in UI/features/simulate/model.ts:425 | No toggle |
| C15 | Fitting-zone scattering on/off | niche | Core only | Core only | CM `spps.fittings` (no UI hit) | |
| C16 | Direct field only | niche | Core only | Core only | CM `direct_field_only` (no UI hit) | |
| C17 | Transmission on/off (solver switch) | common | Core only | Core only | CM `spps.transmission` (no UI hit) | Per-material transmission is editable (Materials grid) but not the solver switch |
| C18 | Receiver sphere radius | common | Core only | Ready | SE:450-461; advisor sets it too (advise.rs `Setting::ReceiverRadius`) | |
| C19 | Random seed | niche | Core only | Core only | CM `random_seed` (no UI hit) | |
| C20 | Map quantity intensity/SPL | common | Core only | Core only | CM `sound_map` (no UI hit) | |
| C21 | Maps per band | common | Core only | Ready | SE:502-505 | Default on |
| C22 | Echogram per source | common | Core only | Ready | SE:506-509 | Default now ON (CM), closing the matrix's multi-source trap |
| C23 | TCR air absorption on/off | common | Core only | Core only | CM `tcr.air_absorption`; read-only in model.ts:415 | TCR block of SE has no toggle |
| C24 | TCR per-band map switch | niche | Not porting | Not porting | no TcrSettings field (CM:1020-1024) | Switch has no effect in TCR (matrix) |
| C25 | Frequency band selection | core | Design only | Ready | SE:165-263 `BandsEditor`: per-band checkboxes per solver | No "select all / none" buttons (see 3a) |
| C26 | Band presets | common | Absent | Ready | UI/features/simulate/settings.ts:172-178 (5 presets incl. Building/Road, octave and third-octave); SE:225-259; commands.rs:439 `edit_reband` | Carries per-band values to the new bands (upstream has none) |
| C27 | Air conditions | core | Design only | Ready | SE:273-299 temperature, humidity, pressure | |
| C28 | User-defined air absorption | niche | Core only | Core only | CM:751-816; shown read-only model.ts:392-394 | No editor |
| C29 | Sound-speed gradient, ground roughness | niche | Core only | Core only | CM `ground_roughness_m`, `celerity_gradient_*`; write.rs:386 | No UI hit |
| C30 | Meteorological presets | niche | Absent | Absent | none | |
| C31 | Ground-type presets | niche | Absent | Absent | none | |
| C32 | Pre-run checks | core | Planned | Ready | crates/simpa-core/src/validate/project.rs:99 (`no_band_computed`), SOURCE_NONE; SP issues list; flow.ts | Matrix gap "no refusal when every band is off" is closed |
| C33 | Mesh on each run | core | Ready | Ready | run/manager.rs:789 `Stage::Mesh` inside the run | Matrix line numbers have moved |
| C34 | Dated run folder | core | Ready | Ready | run/manager.rs:412 `create_run_folder`; Runs tab lists them | |
| C35 | Save project at run time | niche | Absent | Absent | run.json keeps sha256 only | |
| C36 | Progress, elapsed, remaining | core | Planned | Partial | SP:269-275 bar + elapsed clock; remaining time missing = v1.1-backlog 18 | |
| C37 | Cancel a run | core | Planned | Ready | SP `cancel-run`; MenuBar Simulate > Cancel run; commands.rs:526 `run_cancel`; runs.rs:941 | Kills the solver tree |
| C38 | Solver output in console | core | Planned | Ready | UI/features/dock/ConsolePane.tsx | |
| C39 | Calculation time | niche | Core only | Ready | UI/features/dock/RunsPane.tsx:101-105 `elapsed_s` | |
| C40 | Job list | common | Absent | Absent | none (CLI can be looped); matrix says v1.x but no v1.1-backlog row exists | |
| C41 | Particle fate per band | common | Planned | Ready | RunsPane.tsx:83-90 (loss %, worst band); SP:345 `last-loss`; decision-log 56 | |

### 1b. Meshing rows (matrix Geometry section, same area; not counted in section 2)

| ID | Feature | Imp. | Now | Receipt / note |
|---|---|---|---|---|
| G32 | Mesh on demand | common | Core only | CLI `simpa mesh`; no GUI button. `scene_mesh` is the surface mesh, not TetGen |
| G33 | Mesh quality (ratio, max volume, -Y) | niche | Partial | Only -Y ("Preserve walls when meshing", SE:474-480). `min_radius_edge_ratio`, `max_volume_m3` (CM:1039-1045) have no UI |
| G34 | Surface-receiver area constraint | common | Core only | CM `surface_receiver_max_area_m2`; read only in bridge.rs:1811 |
| G35 | Free TetGen params | niche | Absent | refused on import |
| G36 | preprocess toggle | common | Core only | CM `preprocess` (default false); runs.rs:1112 finds preprocess.exe; no UI |
| G37 | Test mesh topology | common | Partial (viewport highlight unverified) | automatic tetgen diagnosis (core mesh/diag.rs); console wording reasonWords.ts:60 |
| G38 | Mesh settings per solver | niche | Absent | one MeshSettings per project |
| G39 | Mesh preview with slice | common | Absent in practice | no v1.1-backlog row (matrix cites old-GUI backlog:62) |

## 2. Counts (41 C rows)

READY 21 | PARTIAL 1 | CORE-ONLY 10 | DEFERRED 0 | ABSENT 7 | NOT-PORTING 2 | UNVERIFIED 0

- Ready: C1 C2 C7 C8 C10 C11 C12 C13 C18 C21 C22 C25 C26 C27 C32 C33 C34 C37 C38 C39 C41
- Partial: C36
- Core only: C9 C14 C15 C16 C17 C19 C20 C23 C28 C29
- Absent: C4 C5 C6 C30 C31 C35 C40
- Not porting: C3 C24

| Importance | Total | Ready | Partial | Core only | Absent | Not porting |
|---|---|---|---|---|---|---|
| core | 12 | 11 | 1 | 0 | 0 | 0 |
| common | 13 | 8 | 0 | 4 | 1 | 0 |
| niche | 16 | 2 | 0 | 6 | 6 | 2 |
| all | 41 | 21 | 1 | 10 | 7 | 2 |

(core: C1 C7 C10 C11 C25 C27 C32 C33 C34 C36 C37 C38; common: C2 C8 C12 C14 C17 C18 C20 C21 C22 C23 C26 C40 C41.)

Core-importance rows not Ready: C36 (progress and elapsed are Ready; remaining time is not).

Of the 10 Core-only rows, 5 are common (C14 C17 C20 C23 and none else) or niche; these are the "one settings panel" leftovers: nothing a user can reach for them, and each defaults to the upstream value.

## 3. Missed by the 09-25 matrix (upstream receipts under B:\repos\I-Simpa-upstream\src)

Config keys: every key SPPS parses (spps/data_manager/core_configuration.cpp:18-68) is already covered by C7-C22/C41, and the rebuild's writer emits all of them (write.rs: nbparticules, nbparticules_rendu, abs_atmo_calc, output_recp_bysource, random_seed, save_surface_intersection, save_receivers_intersection, direct_calc, enc_calc, computation_method, rayon_recepteurp, trans_epsilon, trans_calc, output_recs_byfreq, surf_receiv_method, pasdetemps, duree_simulation). Gaps found:

a. Band "Select all / Unselect all" (common, S). isimpa/data_manager/tree_core/e_core_core_bfreqselection.h, "Automatic selection" menu, `IDEVENT_BFREQ_PRESELECTION_NONE`/`_ALL`. Rebuild has per-band checkboxes and 5 presets, no all/none.
b. TCR external-core keys `output_folder` and `do_angular_weighting` (niche). ctr/data_manager/core_configuration.cpp:21-37; angular weighting forced on in GUI mode at line 22. Belongs with C6.
c. Separate enable flags for TetGen constraints: `ismaxvol`, `isareaconstraint`, plus `userdefineparams` and `appendparams` default "-Y" (niche to common). isimpa/data_manager/tree_core/e_core_core_tetconf.h:99-109. Rebuild models "off" as `None`; the user-defined string is G35.
d. 3D view paused during a calculation (niche). isimpa/data_manager/projet.cpp:693 `GlFrame->PauseSimulation()`. Rebuild leaves the viewport live.
e. Batch acoustic-parameter compute over every receiver (niche). SystemScript/recp_res_tool/__init__.py:21-31 sends IDEVENT_RECP_COMPUTE_ACOUSTIC_PARAMETERS (TR 15;30, EDT, D). The rebuild computes the report automatically (commands.rs:592 `run_report`), so no GUI action is needed.
f. The GUI-side acoustic-parameter cores, listed for completeness (they are Results rows R12-R33, not new): isimpa/data_manager/projet_calculation.cpp:792 (point receiver), :1001 (surface), :1108 (STI map), :1234 (advanced); formulas TR/ST/C/D/Ts/LEF/LF/LFC/LG/G/STI at :190-586.

Calculation cores registered in the GUI tree (tree_core): SPPS (e_core_sppscore.h), TCR (e_core_tccore.h), TLM (e_core_tlmcore.h, C3), shared config (e_core_core_config.h: duree_simulation, pasdetemps only), band selection, TetGen config. No other core. SystemScript: no Python-driven calculation or diffusion/MD model; job_tool (C40), preceiv_sourceTracker (M34), recp_tool and source_tools (M29/M38), moveto_vertex (G20), sample/parttocsv.py (R66). The only diffusion code is currentRelease/ExperimentalCore/md_octave (C4) with ExperimentalScript/density_report_recp_tool and intensity_all_report_recp_tool, not installed.

Uncovered items: a, b, c, d (4); e and f are covered elsewhere or by design. Total missed 6 listed, 4 genuinely new.

## 4. What the app has that upstream lacks (calculation side)

- SPPS on the GPU (spps-gpu.exe) as a per-run device choice (SP solver choice; run/gpu.rs; decision 70).
- Run-quality advisor with Apply (advise.rs; SP AdviceList) and a sound-map memory forecast that refuses oversize runs (UI/features/simulate/runSize.ts; SE:200-215).
- Sound-map time step (patch 0001) and sparse sound-map series (patch 0002) in the patched SPPS build; solver build verified against solvers/manifest.json on each run.
- Pre-run validator with an inline refusal under each settings field, strict decimal entry, exact undo of every settings edit.
- Re-banding that carries materials, sources and fittings values to the new bands; step count and .pbin size readouts while typing.
- Particle loss as a per-band pass/fail verdict, run provenance in run.json (solver sha, device), variants, headless CLI, run cancel that kills the solver tree.
- Auralization from the echogram (commands.rs:672 `run_auralize`), particle playback, validated T20/T30/EDT/C/D/STI with noise ranges.

Unverified: G37's viewport highlight; nothing was executed.
