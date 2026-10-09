# Upstream completeness sweep vs parity-matrix section 2 (250 rows)

Method: read-only. Counts are feature-level (duplicates inside one source merged), sources overlap, so
the totals are not de-duplicated across sources. A feature is "covered" only if a matrix row names it.
Verdict: the matrix is close to complete. 11 small misses, none core, none common.

## 1. Per source: found / covered / missed

| # | Source | Found | Covered | Missed |
|---|---|---|---|---|
| 1 | Menus, toolbars, popup menus (i_simpa_main.cpp menu bar + toolbars; element.cpp, tree_*, uiTreeCtrl, GabeDataGrid, PropGrid, simpleGraphManager popups; Python-registered menus) | 118 | 113 | 5 |
| 2 | Tree properties by element type (tree_core, tree_scene, tree_rapport, generic_element, preferences tree) | 62 groups | 61 | 1 |
| 3 | SystemScript plugins + shipped script samples (8 UI plugins, 3 libs graphy/uilocale/new_element_witness, SppsReportSample, recp_res_norm, user_core, 2 experimental report scripts) | 16 | 14 | 2 |
| 4 | Solver config keys + solver CLI (spps, ctr, lib_interface base keys, preprocess, VolumetricMeshRepair) | 34 | 32 | 2 |
| 5 | Result file types + import/export formats (gabe gap recp recps rpi pbin csbin, unknown/OS, csv/gabe save, png/jpg/bmp, 5 scene imports, 8 scene exports, CATT/Odeon, chart export) | 36 | 36 | 0 |
| 6 | Docs/ and CHANGELOG (menus, toolbars, tabs, tutorials, validation, install, FreeCAD workflow) | 24 | 23 | 1 |
| | Total (with cross-source overlap) | 290 | 279 | 11 |

Plugin check: check_version->A24, job_tool->C40, moveto_vertex->G20, preceiv_sourceTracker->M34, recp_res_tool->R65,
recp_tool->M38/M39, source_tools->M28/M29, sample->R66/A45, SppsReportSample->R68/R69, recp_res_norm->R70, user_core->C5.
Solver keys: all SPPS keys map to C7-C21 (incl. save_*_intersection->C9, random_seed->C19); TCR keys to C2/C6/C23; common
keys (humidite, pression, temperature, alog, blin, absatmo, directivity_file, side_material, resolution, loi_diff) to C27-C31, M8, M23, M41, G29.

## 2. MISSED

| Feature | Area | Importance | Upstream file:line | One line |
|---|---|---|---|---|
| Hide / show meshing toggle | geometry | niche | main/i_simpa_main.cpp:98, :260, :438 | View menu + toolbar toggle that hides the tetra mesh; G32/G39 name show and slice, not hide. |
| Reinitialize interface (reset window layout) | project-app | niche | main/i_simpa_main.cpp:58, :235; Docs/menu_windows.rst | Restores the default panel layout; A28 names docking only. |
| Delete the animation (toolbar) | results | niche | main/i_simpa_main.cpp:66, :421; Docs/toolbar_simulation.rst | Frees loaded animation data from memory; R55 names play/pause/step only. |
| Results spreadsheet structure editing | results | niche | IHM/GabeDataGrid.cpp:511-551 | Insert/delete line or column, edit line/column properties, Save data in place; R61 names only the editable copy. |
| Python console: save and clear | project-app | niche | data_manager/projet.h:467-472 (OnSaveShellToFile, OnClearShell) | A14 names the console, A26/A27 name only the message console. |
| Material display colour (mat_color) | materials | niche | data_manager/tree_scene/e_scene_bdd_materiaux_rendermateriau.h:56 | Per-material colour used by "colour by material"; G45 names the view mode, not the editable colour. |
| Experimental script: energy density per receiver | results | niche | currentRelease/ExperimentalScript/density_report_recp_tool/__init__.py | Builds density-vs-position table from receiver levels; not installed by default. |
| Experimental script: intensity of all receivers | results | niche | currentRelease/ExperimentalScript/intensity_all_report_recp_tool/__init__.py | Same family, intensity; a todo.txt describes a level-vs-area distribution. |
| Round_Robin_3 benchmark data | project-app | niche | src/isimpa/resources/doc/tutorial/Round_Robin_3 (readme.txt, results_p3_public.xls, studio_coordinates_p3.xls) | Fourth bundled data set; A43 says 3 tutorials, A44 names only the validation folder. |
| TCR do_angular_weighting key | calculation | niche | src/ctr/data_manager/core_configuration.cpp:33; TC_CalculationCore.cpp:449,467 | Cosine weighting of energy on surface receivers in external-core mode; C6 names the mode, not the switch. |
| SPPS -v verbose flag and >5 % lost-particle console warning | calculation | niche | src/spps/sppsNantes.cpp:33, :270 | Only user-visible diagnostics outside the GUI options; C41 names the stats file. |

Counts by area: geometry 1, materials 1, calculation 2, results 4, project-app 3.

Looked for and found covered (spot checks that could have been misses): Compare surface receivers wizard (R52), Copy 3D view (R56),
Data extraction mode (R51), Open folder / Refresh / Delete folder (R4/R5/R3), Load animation Particles/Rays/Intensity (R53/R54/R33),
band preselection submenu (C26), job-list submenu (C40), CATT/Odeon import (M13/M14), Iso level list / smooth level (R48),
Surface area per face (G22), recent projects (A7), single-instance + old-session recovery (A34/A35), app-data folder (A36).

## 3. Matrix rows with no real upstream counterpart

- A17 "Headless and scripted operation": its own receipt says "Upstream: Python scripts only". Upstream has no headless mode
  (the GUI binary needs wx; only the solver exes and libsimpa run without it). This is a rebuild goal, not an upstream feature.
  Real upstream counterparts are A14/A15/A18 already.
- Weak, not false (the upstream code exists but is unreachable or interop-only):
  - C3 TLM: E_Core_Tlm loads from an old project file only (tree_core/e_core.h:75), never created in a new project.
  - C4 md_octave: currentRelease/ExperimentalCore only, not in the CMake install list.
  - R67 "Open results of upstream runs": projet.cpp:2300 loads a report folder from a .proj; it is import interop, not a standalone feature.
  - A24 update check: code exists (SystemScript/check_version) but the matrix itself records it never runs.
  - A42 file association: Linux .desktop file only.
- Doc-only items not in code: Docs/menu_help.rst lists a 'License' item that the menu bar in i_simpa_main.cpp:299-305 does not register. The matrix correctly omits it.
