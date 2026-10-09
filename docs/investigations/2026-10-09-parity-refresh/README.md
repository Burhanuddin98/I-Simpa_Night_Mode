# Upstream feature parity, refreshed (2026-10-09)

Read-only audit of the code at `95ec062` (`gpu` = `rebuild`, the newest on GitHub) against I-Simpa 1.4.0
(`B:/repos/I-Simpa-upstream`, tag `v1.4.0_snapshot_14_01_2026`). It refreshes the 2026-09-25 matrix
(`../2026-09-25-parity-audit/parity-matrix.md`), whose statuses predate M10-M12b, the settings editor, C1 blank
geometry, the dock, auralization and the GPU solver.

**Burhan, 2026-10-09 03:01, verbatim:** "SURE, BANK IT, AND LETS make sure v1 is like proper" (to: bank the refresh,
and choose between a parity milestone before the hybrid engine or the hybrid first).

My reading: v1 gets a parity milestone, M12c, before M13 (the installer). Section 4 proposes its cut. **The cut is a
proposal until Burhan confirms it**, and so are M12c's place before M13 and its order against the hybrid engine;
the decision-log row 76 records his words and that all three are open.

## 1. The answer

Upstream has **about 260 user-facing features**: the 250 of the 09-25 matrix, less one that is not an upstream
feature (A17), plus 11 the systematic sweep found (section 3). Of the 250 audited rows:

| Status now | 09-25 | 2026-10-09 |
|---|---|---|
| Ready: a GUI user can do it today | 14 | **90** |
| Partial: reachable, a named part missing | (not a status) | 26 |
| Core only: in the core or CLI, no control in the app | 44 | 32 |
| Deferred: a v1.1-backlog row or decision | 18 | 20 |
| Absent: in no plan | 106 | **73** |
| Not porting: dropped with a reason | 8 | 8 |
| Unverified | - | 1 (A35, several instances) |

The 09-25 "Planned" (45) and "Design only" (15) are gone: M10-M12b are closed, so every such row resolved to what
was built. Counts per area (Ready / Partial / Core only / Deferred / Absent / Not porting / Unverified):

| Area | Rows | R | P | C | D | A | N | U |
|---|---|---|---|---|---|---|---|---|
| Geometry, scene, 3D view | 48 | 10 | 11 | 8 | 1 | 18 | 0 | 0 |
| Materials, sources, receivers | 44 | 14 | 5 | 7 | 4 | 13 | 1 | 0 |
| Calculation | 41 | 21 | 1 | 10 | 0 | 7 | 2 | 0 |
| Results | 70 | 33 | 6 | 6 | 7 | 15 | 3 | 0 |
| Project, app, workflow | 47 | 12 | 3 | 1 | 8 | 20 | 2 | 1 |

By importance, as the area readers graded it: **core 46 of 56 Ready** (the 09-25 matrix counted 55 core rows; one
reader's grading differs), common 38 of 114, niche 6 of 81. All 11 core Results rows are Ready.

## 2. The 10 core rows not Ready

| ID | Feature | Now | What is missing (receipts in `area/`) |
|---|---|---|---|
| G8 | Repair model at import | Core only | The GUI import never repairs; a refused model loads with faces highlighted and Run blocked |
| G16 | Scene tree | Partial | No volumes, fitting zones, environment or display nodes |
| G18 | Add a group; refuse deleting a non-empty one | Partial | A group is made only from picked faces (G19); no empty group, no delete |
| G43 | Faces inside / outside / none | Partial | No "none"; outside only through See-through |
| G47 | Face selection | Partial | No Ctrl multi-select, no drag-select |
| G48 | Pick a position on the model | Partial | Floors only (faces facing up), at fixed heights |
| M32 | Point receiver: position, label, orientation | Partial | Orientation not editable |
| C36 | Progress, elapsed, remaining | Partial | No remaining time (v1.1-backlog 18) |
| A29 | Rename and delete any element | Partial | A surface group cannot be deleted; material rename/delete unverified |
| A40 | Windows installer | Absent | No NSIS build: milestone M13 |

## 3. Found by this sweep: features the 09-25 matrix missed

The completeness reader swept six sources mechanically (every menu and popup item, every tree property, every
SystemScript plugin, every solver config key, every result and export format, Docs and CHANGELOG): 290 found, 279
covered, 11 missed, none core (`area/upstream-completeness.md`). The area readers found more, some of them parts of
existing rows. Merged, with new IDs where a row is new:

| ID | Feature | Area | Importance | Upstream receipt (under `src/isimpa/` unless stated) | Kind |
|---|---|---|---|---|---|
| G49 | Hide / show the tetrahedral mesh (View menu, toolbar) | geometry | niche (one reader: common) | main/i_simpa_main.cpp:98, :260, :768 | new |
| G50 | Per-element display: colour and "Show name" for sources, receivers, fitting zones | geometry | common | data_manager/tree_scene/e_scene_sources_source_rendu.h:46-48; e_scene_recepteursp_recepteur_rendu.h:46-48 | new (M30 named it in a cell only) |
| G51 | Axis and grid appearance (arrow colours, length, width; grid colour, scale) | geometry | niche | tree_scene/e_scene_projet_rendu_origine.h:46-66 | part of G46 |
| G52 | Cutting-plane display options (grid, vertex names, cut colour) | geometry | niche | tree_scene/e_scene_recepteurss_recepteurcoupe_rendu.h:54-58 | new |
| G53 | Volume domain colour | geometry | niche | tree_scene/e_scene_volumes_volume_rendu.h:53 | part of G25 |
| M45 | Editable material display colour | materials | common (sweep: niche) | tree_scene/e_scene_bdd_materiaux_rendermateriau.h:56 | new; the app stores `color` (model.rs:429) and shows a read-only swatch |
| M46 | Spectrum editor's Global row (dB, dB(A), attenuation, Lw, linked) and per-band dB(A) | materials | common | generic_element/e_property_freq.cpp:89-110, :181-190 | new |
| M47 | Descriptions on surface receivers and cutting planes | materials | niche | tree_scene/e_scene_recepteurss_recepteur_proprietes.h:50; ..._recepteurcoupe_proprietes.h:50 | part of M31 |
| M48 | A source's spectrum stays linked to its library entry | materials | niche | generic_element/e_property_freq.cpp:151-175 | part of M17 |
| C26+ | Bands: Select all / Unselect all | calculation | common | data_manager/tree_core/e_core_core_bfreqselection.h (IDEVENT_BFREQ_PRESELECTION_NONE/_ALL) | part of C26 (the app has 5 presets, no all/none) |
| C42 | TetGen constraint enable flags (ismaxvol, isareaconstraint), default "-Y" | calculation | niche | data_manager/tree_core/e_core_core_tetconf.h:99-109 | part of G33 |
| C43 | TCR `do_angular_weighting` and `output_folder` | calculation | niche | src/ctr/data_manager/core_configuration.cpp:21-37 | part of C6 |
| C44 | Pause and resume a running simulation | calculation | common | main/i_simpa_main.cpp:62, :219, :418, :817-820 | new (the app has Cancel only) |
| C45 | 3D view paused while a calculation runs | calculation | niche | data_manager/projet.cpp:693 | new; the app keeps the view live on purpose (live run view, B3) |
| C46 | SPPS `-v` verbose flag and its >5 % lost-particle console warning | calculation | niche | src/spps/sppsNantes.cpp:33, :270 | covered in spirit by decision 56 (loss reported from 0.3 %, refused from 1 %) |
| R71 | Report script's RT30 CSV per band per receiver | results | common | resources/doc/tutorial/script_tutorial/SppsReportSample/__init__.py:~100-117 | part of R68; R59's CSV holds the data in another layout |
| R72 | Chart axis scale and tick spacing by hand | results | common | IHM/simpleGraphDialogs.cpp:548-553 | new |
| R73 | Map record types incl. time-valued (RT-style) maps | results | common | data_manager/tree_rapport/e_report_recepteurssvisualisation.h:~85-105 | part of R42 (v1.1-backlog 76) |
| R74 | Per-curve chart styling (line, marker, labels, fill, background) | results | niche | IHM/simpleGraphDialogs.cpp:281-282, :533-554, :632-663 | part of R64 |
| R75 | Results spreadsheet editing (insert/delete rows and columns, save .gabe) | results | niche | IHM/GabeDataGrid.cpp:342, :396-445, :511-551 | new |
| R76 | Per-source parameter dialog (clarity ms, definition ms, decay list, NC curve) | results | common | data_manager/tree_rapport/e_report_gabe_recps.cpp:49; data_manager/projet_calculation.cpp:796-830 | part of R15/R20/R23 |
| R77 | Delete the loaded animation (toolbar) | results | niche | main/i_simpa_main.cpp:66, :421 | new |
| R78 | Experimental scripts: energy density and intensity of all receivers | results | niche | currentRelease/ExperimentalScript/ (not installed by default) | new, experimental |
| R79 | "Sum only" surface receivers as a static map | results | niche | e_report_recepteurssvisualisation.h:~79-82 | unverified in the app |
| A48 | Reinitialize interface (reset the layout) | project-app | niche | main/i_simpa_main.cpp:58, :235 | new |
| A49 | Python console: save and clear | project-app | niche | data_manager/projet.h:467-472 | part of A14 |
| A50 | Round Robin 3 benchmark data (PTB studio) | project-app | niche | src/isimpa/resources/doc/tutorial/Round_Robin_3/ | new |
| A51 | Script libraries `graphy` and `uilocale` | project-app | niche | resources/SystemScript/graphy/, uilocale/__init__.py:8-20 | part of A14/A15 |
| A52 | Preference contents (3D colours, render rate, legend font, iso-line colour) | project-app | common | data_manager/projet.cpp:2341-2461 | part of A12 |

Two readers disagree on importance (G49, M45): the sweep graded strictly by how buried the control is, the area
readers by how often a user meets it. Both gradings are kept.

## 4. v1 parity: the proposed cut for M12c (Burhan to confirm)

Rule used for the proposal: an upstream user must not hit a wall in the first hour of ordinary work, and nothing
in the cut may put a wrong number in front of a user. Sizes are not estimated here; M12c's planner sizes them.

**P0, the core rows (9; A40 is M13 itself):** G8, G16, G18, G43, G47, G48, M32, C36, A29.

**P1, built in the engine, add the control (15 common rows; G28 and G29 are two):** G7 keep groups on re-import, G28/G29 fitting zones
shown and edited, G32 mesh on demand, G34 surface-receiver area constraint, G36 preprocess toggle, M27 source
groups, M40 scene surface receivers, M43 enable/disable planes, surface receivers and zones, C14 SPPS air absorption,
C17 transmission switch, C20 map quantity (intensity/SPL), C23 TCR air absorption, R36 TCR receiver levels, A31
project name and description.

**P2, first-hour tools (common, absent or partial):**
- Runs and results: R2 label a run, R3 delete a run, R4 open a run's folder, R7 receiver level table, R9 spectrum
  chart, R15 T15 and chosen decay ranges, R20 custom C/D limits, R27 Schroeder table, R45 map opacity, R52 difference
  map with a run picker, R58 copy table cells, R62 chart zoom and series hide, R63 export a chart as an image, R72
  chart axes by hand, and **R42 maps of T30, EDT, C80 and D50 with a test bed** (with R73's time-valued map
  ranges; added 03:15 on Burhan's answer "v1, with a test bed", decision 77; STI maps stay v1.1).
- Scene, sources, receivers: G11 new box room, G37 mesh-diagnosed faces highlighted, G44 lines none, G46 XZ/YZ grids
  (v1.1-backlog 11), G50 per-element colour and names, M5 per-band reflection law (v1.1-backlog 20), M17 user
  spectrum library, M18 and M46 dB(A) entry with the linked Global row, M30 markers, M37 receiver groups created and
  edited, M41 free cutting planes, M45 material colour.
- Calculation: C26+ select all / none, C40 job list, C44 pause and resume.
- App: A6 save a copy, A7 recent projects, A20-A23 help, offline PDF, manual and About, A33 reopen the last project,
  A34 crash recovery, A38 drop a file on the window, A39 the missing shortcuts, A42 file association (with M13), A43
  upstream's tutorials 1-3 with expected results.

**Stays v1.1 or later (a v1.1-backlog row, 105):** every niche row; and these common ones, each needing a bed, a
format or a product call first: G3 3DS import, G9 average-model remesh, G39 mesh preview (with G49), G45 CAD colours,
M3 material folders, M12 copy/paste across projects, M13/M14 CATT and Odeon import, M28/M38/M39 group actions and
the receiver grid, R28 LF/LFC, R44 STI maps (v1.1-backlog 76), R49 palettes, R60 charts from a
selection, R68/R71 the report document, A12/A52 preferences, A13 languages, A24 update check, A41 macOS/Linux, A47
project archive.

**The cut counts 67 items:** P0 9, P1 15, P2 43 (15 results, 13 scene/sources/receivers, 3 calculation, 12 app);
it was 66 until R42 joined at 03:15.
**03:14, Burhan, verbatim: "parity, first, like let me know what those final features for v1 are, we continue
building a hubrid engine in this chat".** Parity goes before the hybrid engine.

**Open for Burhan:** (1) confirm or change the cut, and M12c's place before M13; (2) R42, maps of T30/EDT/C80/D50,
is what an I-Simpa user expects on a map and is v1.1 only because those map numbers have no bed: pull it into v1 with a
bed, or keep it out; (3) the order of the two arcs: he was asked at 03:00 "parity first or the hybrid engine first" and
his answer did not choose. The hybrid arc itself is open too: at 02:28 he asked whether to "think it over and maybe
redesign the whole thing" (the options and four product questions were put to him, unanswered; Zeph's evidence is on
branch `fdtd-probe`).

## 5. Corrections to earlier statements

- My dispatch brief to the readers (02:33) said the app has a command palette and recent files. It has neither: the Ctrl K
  button is disabled, and the Model, Results and Help menus are disabled stubs (`area/project-app.md`).
- C44 "Pause and resume a running simulation" (section 3, P2) is not a pause of a solver (checked 2026-10-09
  14:43 while building P2). Upstream's Simulation › Pause and its toolbar button, beside Previous and Next time
  step, call `OpenGlViewer::PauseSimulation` (3dengine/OpenGlViewer.cpp:415-418), which only sets
  `simulationIsRunning` false; that flag gates the particle animation's `Tic()` (:219-224), so it pauses the
  animation playback, which the app has as R55 (Ready). No upstream code pauses a running SPPS or TCR, and SPPS
  reads nothing from its console while it runs (src/spps: no stdin read), so there is no pause to offer and none
  is built; suspending the solver's process would be a pause upstream does not have. The P2 count is 42 builds,
  C44 answered by R55.
- A17 "Headless and scripted operation" is not an upstream feature: its own receipt says "Python scripts only", and
  upstream's GUI needs wx. It is a rebuild goal (the CLI). Weak but real rows: C3 TLM (loads from old projects only),
  C4 md_octave (ExperimentalCore, not installed), R67 (interop), A24 (code present, never runs), A42 (Linux .desktop
  only).

## 6. Method and files

Six read-only readers (Sonnet), 2026-10-09 02:33-02:40, on a detached checkout of `95ec062`: one per area (rows
checked against code, a file:line receipt for every Ready and Core only, "unverified" where code could not confirm)
and one completeness sweep over upstream. Their full reports, with every receipt, are in `area/`:
`geometry.md`, `materials.md`, `calculation.md` (its section 1b re-checks the mesh rows G32-G39, not counted twice
in section 1 above), `results.md`, `project-app.md`, `upstream-completeness.md`. The readers' counts were not
re-verified line by line by the main thread; a row's status is the reader's, with its receipt.
