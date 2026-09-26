# Upstream feature parity: the rebuild against I-Simpa 1.4.0

2026-09-25. Read-only audit. Upstream is `B:/repos/I-Simpa-upstream` (tag `v1.4.0_snapshot_14_01_2026`). The rebuild is branch `rebuild` in the main checkout (the GUI is still the M9 shell: `app/ui/src` holds App, MenuBar, Panels, StepBar). Inputs: five area readers (geometry, materials and sources, calculation, results, app) and one completeness critic. This file merges them, applies the critic's corrections, and adds a few of its own (listed under Method).

---

## 1. The answer

Once the five readers' lists are merged and duplicates removed, upstream I-Simpa has **250 user-facing features**. If M10 to M13 deliver exactly what their deliverables and gates name, a v1 user gets **59** of them: 14 work today and 45 are planned. **15 more** are drawn on the approved Concept B screens, but no deliverable or gate names them, so they exist only if the screens are built as drawn. **44** are built in the core or CLI but have no GUI anywhere in the plan, so a GUI user cannot reach them. **18** are deferred to after M12, **106** are in no plan, and **8** are deliberately not ported. By importance: of the **55 core** features, 38 are covered, 9 are on the design only, 5 are core-only, 1 is deferred and 2 are absent. Of the **114 common** features, 19 are covered, 6 are on the design only, 24 are core-only, 15 are deferred, 48 are absent and 2 are not ported. Of the **81 niche** features, 2 are covered, 15 are core-only, 2 are deferred, 56 are absent and 6 are not ported. The engine is close to parity: the schema, config export, import and results core already hold most of upstream's settings and outputs, and in several places they are stricter or more correct than upstream. The GUI plan is where the distance is. M10 to M12 cover the main path (import, materials grid, placing sources and receivers, run, T30/EDT, maps, particles). They do not cover the settings, the editing verbs or the ways results leave the tool, and those are what a user who knows upstream reaches for in the first hour. Of the 124 absent or deferred features, 80 are small (S), 35 medium, 6 large and 3 extra-large.

| Importance | Total | Ready | Planned | Design only | Core only (no GUI) | Deferred | Absent | Not porting |
|---|---|---|---|---|---|---|---|---|
| core | 55 | 7 | 31 | 9 | 5 | 1 | 2 | 0 |
| common | 114 | 5 | 14 | 6 | 24 | 15 | 48 | 2 |
| niche | 81 | 2 | 0 | 0 | 15 | 2 | 56 | 6 |
| **all** | **250** | **14** | **45** | **15** | **44** | **18** | **106** | **8** |

| Area | Total | Ready | Planned | Design only | Core only | Deferred | Absent | Not porting |
|---|---|---|---|---|---|---|---|---|
| Geometry, scene, 3D view | 48 | 1 | 11 | 2 | 8 | 4 | 22 | 0 |
| Materials, sources, receivers | 44 | 1 | 8 | 3 | 9 | 6 | 16 | 1 |
| Calculation | 41 | 3 | 7 | 5 | 16 | 0 | 8 | 2 |
| Results | 70 | 2 | 14 | 4 | 8 | 5 | 34 | 3 |
| Project and app | 47 | 7 | 5 | 1 | 3 | 3 | 26 | 2 |

**Four risks inside the planned items.** These are not missing features, but each one can leave a planned screen empty:

1. **No T30 or EDT on tutorial 1 at upstream's defaults.** At 150k particles, all 540 receiver-bands refuse T30 and EDT in both modes (handoff:358-364, commit 8a8f8e8). A user running upstream's own tutorial would see no reverberation time.
2. **M12(b) hides every parameter without a PASS bed** (raw:219-220). The 00:20 M8 decisions gate only T30 and EDT. C80 and D50 are "reported, not gated" (handoff:466-470), and C50 and Ts have no bed at all. As planned, those four do not appear, even though C80 and D50 are on the design's map selector (design:438-443). Whether SPL is gated is not stated.
3. **Particle playback has nothing to play by default.** M12 gate (d) plays `.pbin` particles, but `particles_saved` defaults to 0 (core/schema/model.rs:963) and no GUI field sets it.
4. **Multi-source projects get no decay parameters by default.** The rebuild refuses EDT, T30, C and D on a summed multi-source echogram (docs/results.md:469-473), and `echogram_per_source` defaults to off (model.rs:975).

**Where the rebuild already beats upstream**, so none of this reads as all loss: binary STL read correctly, OBJ import, explicit units and up axis, partitions classified instead of refused, a validator that refuses bad runs before launch, a cancel that really kills the solver, atomic save, exact and unlimited undo, particle loss as a pass/fail verdict, per-run provenance, and a full headless CLI (upstream has none).

---

## Method, statuses and receipts

**Statuses.** A GUI part counts as planned only when an M10 to M13 deliverable line or gate names it. This is the critic's rule, and it is stricter than some readers applied.

| Status | Meaning |
|---|---|
| **Ready** | Works today, or needs no GUI work (automatic behaviour, or already in the shell). |
| **Planned** | Named in an M10 to M13 deliverable or gate, or in rebuild-plan.md's M12 text. The Plan column says whether the core part is built. |
| **Design only** | Drawn on the approved Concept B screen for a step M10 to M12 builds, with no deliverable or gate naming it. |
| **Core only** | Built in the core or CLI. No milestone and no design screen puts it in the GUI. |
| **Deferred** | After M12: rebuild-plan.md:133-134, "STI comes later" (plan.md:141), or an open row of ui-parity-backlog.md (the post-M12 seed, raw:257). |
| **Absent** | In no plan, and not deliberately refused. Open decisions with no answer count as absent. |
| **Not porting** | Deliberately left out, with a reason in the plan or in the evidence. |

**Critic corrections applied:**
1. Six features the readers missed are added: M43, M44, R23, R69, R70 and A47.
2. **Report export is an upstream feature.** The `SppsReportSample` script adds "Make report" (`resources/doc/tutorial/script_tutorial/SppsReportSample/__init__.py:163-164`, re-read for this audit). It is R68, merged with the app reader's "HTML report generation".
3. **Source groups:** import and persistence are done (model.rs:553-560). Only creating and editing groups is deferred (M27).
4. **preprocess.exe is done in core** (core/mesh/preprocess.rs, m56 decision 12). It is not "not shipped" (G36).
5. **Reference materials and spectra:** the tables are in core (appconst.rs:36-61). The material library UI is deferred (M1); the spectrum library is absent (M16).
6. **3DS is undecided, not dropped.** raw:351 is a proposal. raw:263 "[drop] The 3DS loader" refers to Night Mode's own old loader, not to the feature (re-read for this audit).
7. **GUI for fitting zones, surface receivers, cutting planes, environment, solver settings, bands, spectra and directivities** is in no M10 to M12 deliverable or gate. Each is Core only, or Design only where a screen draws it.

**Corrections of my own:**
8. **Landed rows of the old GUI's backlog are not deferred items.** ui-parity-backlog.md splits its rows into "Landed" (:27-40, code that existed in the old ImGui GUI) and "Still open" (:55-62). Only the open rows describe gaps, so only they count as the post-M12 seed. Band presets, ground presets, the mesh-quality UI, face display modes, axis arrows, iso-contours and drag-drop open were "Landed" in the old GUI and are **Absent** from the rebuild plan. Two readers had filed some of them as deferred.
9. **Average model remesh** is an open decision with "proposal: no" and no reason (raw:360). It counts as Absent, not Not porting.
10. **Bundled validation projects:** M8 imports the air-absorption one internally, "reported, not gated". Nothing ships to users, so the status is Absent.

**Merging.** The readers overlapped: the same feature was often seen from two areas, for example "export 3D view" three times and "echogram per source" three times. Each feature appears once, in its home area. The "Calculation settings editor" roll-up row is not counted; its fields are counted one by one (C7 to C31, G33 to G36).

**Not counted, because upstream does not have them:** measurement tool, vertex editing, model transforms, per-group hide, a section plane through the room, orthographic plan view (grep receipts in the geometry reader: no such code in src/isimpa; projection is gluPerspective only, 3dengine/OpenGLApp.cpp:542), directivity balloon display (no directivity code in 3dengine), "Listen at R1" auralisation, and the Ctrl K command palette. AUDIT_COMPARISON.md:58 ("Vertex editing: Y") and :171 ("Measurement tool: Y") are wrong about upstream.

**Receipts.** Upstream paths are relative to `B:/repos/I-Simpa-upstream/src/isimpa/` unless they start with `Docs/`, `src/`, `currentRelease/` or `org.`. Abbreviations:

- `main.cpp` / `main.h`: main/i_simpa_main.*
- `projet.cpp`: data_manager/projet.cpp
- `calc.cpp`: data_manager/projet_calculation.cpp
- `projet_maillage.cpp`: data_manager/projet_maillage.cpp
- `tree_scene/`, `tree_core/`, `tree_rapport/`, `generic_element/`, `python_interface/`: under data_manager/
- `Objet3D*.cpp`, `Recepteurs_surfacique.cpp`, `Particules.cpp`, `stl.cpp`: 3dengine/Core/
- `SystemScript/`: resources/SystemScript/

Plan receipts are relative to `B:/repos/I-Simpa_Night_Mode/`:

- `raw:N`: docs/rebuild-plan-raw-2026-09-23.json line N. M10 is raw:197-204, M11 raw:207-214, M12 raw:217-225, M13 raw:228-235, open decisions raw:348-360.
- `plan.md`: docs/rebuild-plan.md
- `design`: docs/design/concept-b-approved.dc.html
- `backlog`: docs/ui-parity-backlog.md
- `m56`: docs/m5-m6-design.md
- `contract`: docs/solver-contract.md
- `handoff`: session-logs/HANDOFF-2026-09-23.md
- `core/`: crates/simpa-core/src/
- `model.rs`, `ops.rs`, `bands.rs`: core/schema/
- `proj.rs`, `appconst.rs`, `reassign.rs`: core/geometry/import/
- `cli`: crates/simpa/src/main.rs
- `ui/`: app/ui/src/
- `tauri/`: app/src-tauri/src/

This audit re-read the following. Other receipts are the area readers' own and were not re-opened:

- M9 to M13 (raw:190-235), the harvest (raw:238-263) and the open decisions (raw:348-360)
- plan.md:120-149
- all of backlog
- design:290-470
- handoff:352-372 and :455-480
- the SppsReportSample menu
- the `app/ui/src` file list

Sizes follow the readers: S is about a day or one field, M a few days, L a week or more, XL a new subsystem.

---

## 2. The matrix

### 2.1 Geometry, scene and 3D view

| # | Feature | What it does upstream | Imp. | Status | Upstream receipt | Plan / code receipt | Size | Note |
|---|---|---|---|---|---|---|---|---|
| G1 | Import PLY, layers become groups | ASCII or binary .ply; each layer becomes a surface group | core | Planned | main.cpp:187, :881; Objet3D.cpp:1083-1157 | core/geometry/import/ply.rs:1-17; M4 gate (f); M10 "import wired to the M4 check", gate (a) (raw:199-200) | S | CLI today; the GUI File menu opens .simpa only (ui/chrome/MenuBar.tsx:9-14) |
| G2 | Import STL | .stl into one group | core | Planned | Objet3D.cpp:1006-1044; stl.cpp:316-319 | core/geometry/import/stl.rs:1-20; M4 gate (f); M10 import | S | Upstream's binary STL reader scrambles triangles; the rebuild's does not |
| G3 | Import 3DS | Each object becomes a group; CAD colours and textures kept | common | Absent | main.cpp:881; Objet3D.cpp:1265-1330; Docs/import_file_recommandations.rst step 2 | Open decision raw:351 ("Proposed: ... 3DS dropped"), not settled in plan.md:11-19; no reader in core/geometry/import/; design:307 lists 3DS | M | Upstream's own docs tell users to export 3DS |
| G4 | Import a TetGen .poly as the scene | .poly as the model, one group | niche | Absent | Objet3D.cpp:1045-1082 | core/formats/poly.rs:92 reads .poly; core/geometry/import.rs:218-235 dispatches ply/obj/stl only | S | |
| G5 | Import I-Simpa .bin scene | Upstream's scene binary with groups | niche | Absent | Objet3D.cpp:1160-1264 | Read only inside a .proj (proj.rs:9-18) | S | |
| G6 | Import units (m, cm, mm, ft, in) | Scales the file to metres | core | Planned | IHM/loadingSceneDialog.cpp:111-121; Objet3D.cpp:431-438 | core/geometry/import.rs:71-115; cli `--unit` required; M10 import | S | Design shows "m (detected)" (design:665) but the plan never guesses units |
| G7 | Keep groups on re-import | Re-importing a changed model keeps groups and materials (0.01 m match) | common | Design only | loadingSceneDialog.cpp:144-151; projet.cpp:3018-3110 | reassign.rs:1-27; cli `import --keep-groups`; M4 gate (g); design:306 "Replace model..." | S | |
| G8 | Repair model at import | Default-on fix of duplicate vertices and orientation | core | Core only | loadingSceneDialog.cpp:128-131; projet.cpp:3199-3240 | core/geometry/repair.rs:1-30 (`simpa repair`); M4 gate (d). M10 wires import to the check, not to repair (raw:199) | S | Flipped faces are refused by the check, and the GUI has no repair |
| G9 | Average model remesh (VMR wizard) | Rebuilds a broken model by marching cubes, keeps one volume | common | Absent | loadingSceneDialog.cpp:123-126; data_manager/projet_remesh.cpp:49-80; IHM/WizardRemeshModel.cpp:116-200; Docs/tutorial_Elmia_hall.rst:77-84 | Open decision raw:360 ("proposal: no", no reason) | L | The corrected Elmia removes the tutorial's need for it |
| G10 | Surface meshing at import | Re-triangulates faces with TetGen | niche | Absent | loadingSceneDialog.cpp:133-141; projet_maillage.cpp:91-145 | Not in any milestone | S | |
| G11 | New scene: box room | W x L x H shoebox | common | Absent | main.cpp:189, :891-931; Docs/tutorial_teaching_room.rst:42 | Box exists as a test fixture only (raw:250; core/schema/generate.rs:429) | S | Tutorial 1 starts here; the rebuild's box has 6 groups already |
| G12 | Export scene as PLY or .mat.ply | Groups or materials as layers | niche | Absent | main.cpp:188, :933-943; Objet3D.cpp:905-1004 | No PLY writer in core/formats | S | |
| G13 | Export scene as .cbin or .poly | SPPS or TetGen scene file | niche | Core only | main.cpp:935; Objet3D_maillage.cpp:821, :931 | core/formats/cbin.rs:287, poly.rs:779; written into run and mesh folders | S | |
| G14 | Export .mesh, .nff, .bin, .asc | Legacy formats | niche | Absent | main.cpp:935; Objet3D.cpp:880-903 | Not planned | S | NFF is offered but broken upstream (Objet3D.cpp:489-525) |
| G15 | Save as upstream .proj | Hand a project back to I-Simpa | niche | Absent | main.cpp:191-193; projet.cpp:2558-2610 | Project is JSON .simpa (model.rs:17-21); import-proj is read-only (proj.rs:1) | L | |
| G16 | Scene tree | Find and edit every scene object | core | Planned | main.cpp:347-360; Docs/tab_scene.rst | M10 "the scene list and the properties panel" (raw:199) | M | The design's list has no fitting zones or volumes (design:80-100) |
| G17 | Surface groups from the file | Each file group or layer becomes a surface group | core | Planned | projet.cpp:3033-3056 | core/geometry/import.rs:5-8; M4 gate (b); M10 import | S | |
| G18 | Add a surface group; refuse deleting a non-empty one | "Add a group" | core | Core only | tree_scene/e_scene_groupesurfaces.h:106, :199; projet.cpp:851-861 | ops.rs:115-134. No M10 line or gate names these verbs | S | |
| G19 | Send selected faces to a new group | Right-click the selection, new group | core | Deferred | tree_scene/e_scene_groupesurfaces_groupe_vertex.h:76; projet.cpp:1638-1651; Docs/tutorial_teaching_room.rst:51-57 | backlog:62 "surface group from selection". No regroup op (ops.rs:124-127 has SetGeometry only) | M | Also blocks scene-fitted zones (G30) |
| G20 | Move faces to an existing group; merge groups | Drag faces or groups onto a group | common | Absent | tree_scene/e_scene_groupesurfaces_groupe.cpp:376-434; SystemScript/moveto_vertex/__init__.py:14-27 | Not in any milestone or the backlog | M | Shares G19's op |
| G21 | Face list per group, tree and 3D in sync | Expand a group; selection mirrors both ways | niche | Absent | tree_scene/e_scene_groupesurfaces_groupe_vertex.h:47-60; projet.cpp:1708-1800, :2820-2869 | M10 names picking and flood-fill only | M | |
| G22 | Model and project statistics | Face count, group and scene area, volume, element counts | common | Design only | tree_scene/e_scene_groupesurfaces_groupe.cpp:103-111; tree_scene/e_scene_projet_informations.h:65-81; Objet3D.cpp:1677-1722 | Counts shown today (ui/chrome/Panels.tsx:28-36); area and volume in core/geometry/check.rs:250-322; drawn at design:297-305, :316, :476 | S | Volume comes from the surface before meshing |
| G23 | Invert face normals by hand | Flip a group or a selection | niche | Absent | tree_scene/e_scene_groupesurfaces_groupe.cpp:455; Objet3D.cpp:1724-1740 | Repair orients automatically (repair.rs:17-22) | S | |
| G24 | Internal partitions, reflectors, coupled rooms | Keeps internal facets so transmission works | common | Ready | Objet3D_maillage.cpp:369-381; Docs/tutorial_industrial_hall.rst:38 | core/geometry/check.rs:23-46; plan.md:123-126 | S | |
| G25 | Volumes | Faces plus an inside point; TetGen regions | niche | Absent | tree_scene/e_scene_volumes.cpp:96; tree_scene/e_scene_volumes_volume.h:104-106 | Refused on import (proj.rs:82, :243-245; contract:677) | M | Upstream's Industrial.proj is refused for its three volumes |
| G26 | Volume auto-detect | One volume per closed region | niche | Absent | tree_scene/e_scene_volumes.cpp:97; projet_maillage.cpp:310-361 | Cells found by check.rs:250-261, not exposed | M | |
| G27 | Convert a volume to a fitting zone | | niche | Absent | tree_scene/e_scene_volumes_volume.h:194; projet_maillage.cpp:285-309 | No volumes in the schema | S | |
| G28 | Fitting zones reach the solver | Box and scene-fitted zones meshed and exported | common | Core only | tree_scene/e_scene_encombrements_encombrement_cuboide.h:104-105; Objet3D_maillage.cpp:1002-1036 | model.rs:661-749; proj.rs:53-80; tutorial 3 parity (plan.md:206-212) | S | Zones come only from an imported .proj or edited JSON |
| G29 | Create and edit a box fitting zone | Corners, per-band absorption, mean free path, diffusion law | common | Absent | tree_scene/e_scene_encombrements.h:109; projet.cpp:901-916; generic_element/e_gammeabsorption.cpp:34-60; Docs/tutorial_industrial_hall.rst:127-134 | Core ops exist (ops.rs:197-212); no CLI command, no screen in M10 to M12 | M | |
| G30 | Scene-fitted zone from selected faces | CAD shell plus an inside point | niche | Absent | tree_scene/e_scene_encombrements.h:108; tree_scene/e_scene_encombrements_encombrement_model.h:122-125 | FittingShape::Surfaces takes whole groups (model.rs:709-713) | M | Needs G19 |
| G31 | Fitting-zone display | Colour, opacity, borders, label | niche | Absent | tree_scene/e_scene_encombrements_encombrement_rendu.h:44-66 | No display fields (model.rs:661-685) | S | |
| G32 | Mesh on demand | "Meshing" button to check the mesh | common | Core only | main.cpp:222, :437; projet.cpp:2730-2755 | cli `simpa mesh`, `mesh-verify`; M5 (plan.md:225) | S | The geometry check covers most of the need |
| G33 | Mesh quality settings | Radius/edge ratio, max tet volume, -Y | niche | Core only | tree_core/e_core_core_tetconf.h:95-104 | model.rs:1004-1042; core/mesh/flags.rs:1-25. Old GUI had a UI (backlog:35, Landed) | S | |
| G34 | Surface-receiver area constraint (.var) | Map resolution on surface receivers | common | Core only | tree_core/e_core_core_tetconf.h:100-101; projet_maillage.cpp:217-224 | model.rs:1014-1018; m56 decision 4 | S | |
| G35 | Free-form TetGen parameters | Extra flags or a full command line | niche | Absent | tree_core/e_core_core_tetconf.h:98-99; projet_maillage.cpp:163-178 | Refused on import (proj.rs:2531-2560) | M | Needs a policy: free flags bypass the mesh verifier |
| G36 | Scene correction before meshing (preprocess.exe) | Runs preprocess.exe before TetGen | common | Core only | tree_core/e_core_core_tetconf.h:103; projet_maillage.cpp:57, :206-213 | core/mesh/preprocess.rs:1-30; model.rs:1021-1029; m56:232 decision 12 | S | Default off where upstream's is on; open for Burhan (m56:244-245) |
| G37 | Test mesh topology | Finds faces TetGen rejects and highlights them | common | Planned | tree_core/e_core_core_tetconf.h:104; projet_maillage.cpp:242-275 | Automatic `tetgen -d` (core/mesh/diag.rs:1-25); M10 gate (a) highlights the check's faces (raw:200) | S | TetGen-diagnosed faces are not in a gate |
| G38 | Mesh settings per solver | SPPS and TCR each keep their own | niche | Absent | projet_maillage.cpp:40-58 | One MeshSettings per project (model.rs:889-896) | S | |
| G39 | Mesh preview with x/y/z slice | Tets drawn; a slider shows a slab | common | Deferred | main.cpp:260, :437-444; Objet3D_maillage.cpp:542-680; Docs/toolbar_meshing.rst | backlog:62 "mesh preview in the viewport" | M | .mbin reader exists (core/formats/mbin.rs:117) |
| G40 | Orbit, zoom, pan | Basic navigation | core | Planned | main.cpp:267; 3dengine/Camera.cpp:69-153 | M10 three.js viewport (raw:199); M11 gate (b) orbit drag | M | Upstream discards the mouse wheel (Camera.cpp:182-185) |
| G41 | First-person camera | Walk-through with arrow keys | niche | Absent | main.cpp:266; 3dengine/Camera.cpp:155-174 | Not in M10 or the design | S | |
| G42 | Reset camera | Back to the default view | common | Absent | main.cpp:264, :600-603 | Not in M10; model.rs:1094-1111 stores an optional camera | S | |
| G43 | Face display: inside, outside, none | How a room is seen | core | Planned | main.cpp:237-242; projet.cpp:2664-2693 | M10 "inside view" (raw:199) | S | Outside/none not planned (backlog:38, Landed in the old GUI) |
| G44 | Lines: all, contour, none | Edge modes | common | Planned | main.cpp:251-256; Objet3D.cpp:1459-1495 | M10 "outlines" (raw:199) | S | The all/contour/none switch is deferred (backlog:58) |
| G45 | Colour by material or by CAD colours | Visual check of assignments | common | Deferred | main.cpp:245-248; projet.cpp:2711-2728; Docs/tutorial_industrial_hall.rst:259 | backlog:57 row 15 | S | "Original" colours need importers to keep file colours (M) |
| G46 | Axis arrows; XY/XZ/YZ grids | Orientation aids | common | Deferred | main.cpp:274-282; tree_scene/e_scene_projet_rendu_origine.h:49-59 | Grids backlog:59; axis gizmo on design:172, drawn static (Panels.tsx:78) | S | |
| G47 | Face selection | Click, Ctrl multi-select, drag, double-click a flat surface | core | Planned | main.cpp:429; 3dengine/OpenGlViewer.cpp:311-373; Objet3D.cpp:1427-1457; Docs/surface_selection.rst | M10 "BVH picking, iterative coplanar flood-fill", gate (d) | M | Multi-select, drag-select, tree sync not named |
| G48 | Pick a position on the model | Click to set a position | core | Planned | data_manager/e_position.h:104; projet.cpp:1805-1831; Docs/define_position.rst | M10 "source and receiver placement"; design:142, :373 | M | Sources and receivers only; not plane corners, orientation points, zone corners |

### 2.2 Materials, sources and receivers

| # | Feature | What it does upstream | Imp. | Status | Upstream receipt | Plan / code receipt | Size | Note |
|---|---|---|---|---|---|---|---|---|
| M1 | Reference material library | 12 read-only materials, "Default" and 0 to 100 % absorbing | common | Deferred | resources/appconst.xml `<appmaterials>`; tree_scene/e_scene_bdd_materiaux_app.h; Docs/tutorial_industrial_hall.rst:247 | plan.md:133-134; data in appconst.rs:36-47, used by import only | S | Data done, browsing deferred |
| M2 | User materials: create, rename, delete, edit | | core | Planned | tree_scene/e_scene_bdd_materiaux_user.h:142; projet.cpp:1341 | model.rs:421-454; ops.rs:139-149; M10 materials grid (raw:199) | M | |
| M3 | Material folders and categories | Nested folders, drag to re-parent | common | Deferred | tree_scene/e_scene_bdd_materiaux_user.h:143; tree_scene/e_scene_bdd_materiaux_user_group.h | plan.md:133-134; backlog:62 "material categories"; flat list (model.rs:73-74) | M | Schema change |
| M4 | Per-band absorption and scattering | | core | Planned | data_manager/e_data_row_materiau.h InitPropMat; data_manager/appconfig.cpp:101-104 | model.rs:429-433; M10 grid, gate (c); design:336-345 | M | Band set is chosen per project (bands.rs:19-25); upstream always uses 27 third-octaves |
| M5 | Per-band reflection law (7 laws) | Specular, uniform, Lambert, W2 to W4, semi-diffuse | common | Core only | data_manager/appconfig.cpp:110-117; e_data_row_materiau.h:156 | model.rs:321-419, :434-435; not in the M10 text or the design's panel | S | Semi-diffuse falls to specular in SPPS (model.rs:319-320) |
| M6 | Per-band transmission switch and loss | | common | Design only | e_data_row_materiau.h:98-106; Docs/tutorial_industrial_hall.rst:143-199 | model.rs:436-444; design:346 "Transmission off" | S | |
| M7 | Material consistency rules | alpha, scattering, tau limits | core | Planned | e_data_row_materiau.h:114-205 | contract:63-65; core/validate/project.rs:97-203; M10 "inline validator messages" | S | Reports instead of silently rewriting |
| M8 | Side of material effect | One- or two-sided | niche | Core only | tree_scene/e_scene_bdd_materiaux_propmateriau.h | model.rs:445-446 | S | |
| M9 | Material metadata | Description, reference, density, resistivity | niche | Absent | tree_scene/e_scene_bdd_materiaux_propmateriau.h | No fields (model.rs:421-454) | S | A new field means format version 2 (core/schema/json.rs:104-116) |
| M10 | Assign a material to a surface group | | core | Planned | tree_scene/e_scene_groupesurfaces_groupe.cpp:205-220 | model.rs:248-256; ops.rs:135; M10 gates (b), (d); design:319-333 | M | |
| M11 | Spreadsheet editing of band tables | TSV copy and paste with Excel, fill a row or column | common | Planned | IHM/PropGrid.cpp:44-47, :134-277 | M10 grid "TSV paste and copy, row fill", gate (c) | M | Materials only; no grid for source spectra or zone bands |
| M12 | Copy and paste elements | Materials, sources, receivers and groups through the clipboard, across projects | common | Absent | projet.cpp:1364-1412; IHM/uiTreeCtrl.cpp:108-113; Docs/project_database.rst | Not in M10 to M13 or the deferred list | M | Upstream's only way to reuse materials between projects |
| M13 | CATT-Acoustic material import | | common | Deferred | projet.cpp:918-1196; Docs/tutorial_Elmia_hall.rst:130-136 | plan.md:133-134; design:348 | M | CATT materials already inside a .proj do import (proj.rs:111-116) |
| M14 | Odeon .li8 import | | common | Deferred | projet.cpp:1197-1292 | plan.md:133-134 | S | |
| M15 | Unassigned faces | Upstream silently uses "Default" | core | Ready | resources/appconst.xml (id 0); tree_scene/e_scene_groupesurfaces_groupe.cpp:270-271 | Run refused, `material_unassigned` (contract:62) | S | Deliberately stricter (section 5) |
| M16 | Reference spectra | White, pink, six road-traffic | niche | Absent | resources/appconst.xml `<appspectrums>`; tree_scene/e_scene_bdd_spectrums_app.h | Pink and white exist as shapes (bands.rs:142-152); traffic spectra only in the import table (appconst.rs:53-62) | S | |
| M17 | User spectrum library | Named, reusable spectra | common | Absent | tree_scene/e_scene_bdd_spectrums_user.h:71; generic_element/e_gammefrequence.cpp | Per-source custom shapes only (bands.rs:151, :175-184) | M | |
| M18 | dB(A) entry for spectra and power | | common | Absent | data_manager/appconfig.cpp:105-108; data_manager/e_data_row_bandefreq.h | 0 hits for dB(A) or A-weight in plan.md and raw | S | |
| M19 | Source position | Typed x, y, z | core | Planned | tree_scene/e_scene_sources_source.h | model.rs:545-547; ops.rs:155-165; M10 placement; design:357-364 | S | A source outside the room is refused before launch |
| M20 | Source sound power and spectrum shape | Global Lw plus a spectrum | core | Design only | generic_element/e_property_freq.cpp; Docs/using_spectrum.rst | model.rs:548-549; bands.rs:175-218; design:368-369 | S | |
| M21 | Attenuation on a spectrum | Per-band offset for a silencer | niche | Absent | data_manager/e_data_row_ext_bandefreq.h | Folded into custom levels on import (proj.rs:2258-2293) | S | |
| M22 | Analytic directivities | Omni, unidirectional, XY/YZ/XZ plane | common | Design only | tree_scene/e_scene_sources_source_properties.h:59-72 | model.rs:463-505; design:370 "Directivity Omni" | S | TCR treats every source as omni (docs/formats/config_xml.md:277) |
| M23 | Balloon directivity sources | Measured loudspeaker files | niche | Core only | tree_scene/e_scene_sources_source_properties.h:51-53, :93-110; Docs/using_directivity.rst | model.rs:477-480; core/validate/directivity.rs; core/config_xml/write.rs:186-225 | M | import-proj refuses balloon sources |
| M24 | Directivity database | 3 reference speakers plus user entries | niche | Absent | tree_scene/e_scene_bdd_directivities_user.h:123; resources/Directivities/ | Balloon is a file path only (model.rs:477-479) | M | |
| M25 | Source time delay | | niche | Core only | tree_scene/e_scene_sources_source_properties.h:73 | model.rs:551-552; contract:94 | S | |
| M26 | Enable or disable a source | | common | Core only | tree_scene/e_scene_sources_source_properties.h:49; projet.cpp:649-653 | model.rs:543-544; ops.rs:169; contract:71 | S | Fixes upstream's all-disabled guard bug |
| M27 | Source groups | Nested folders of sources | common | Deferred | tree_scene/e_scene_sources.h:221-222; Docs/tutorial_industrial_hall.rst:57-79 | plan.md:133-134; imported groups kept (model.rs:553-560; plan.md:206-208) | M | New groups cannot be made |
| M28 | Source-group actions | Enable/disable all, rotate, translate | common | Absent | SystemScript/source_tools/__init__.py:44-169 | Not named by the deferred "source groups" item | S | Tutorial 3 translates a copied machine by (5, -2, 0) |
| M29 | Line of sources | Start, count, step | niche | Absent | SystemScript/source_tools/__init__.py:13-35, :58-81 | Not planned | S | |
| M30 | Source and receiver markers in 3D | Point, label, direction arrow | common | Planned | tree_scene/e_scene_sources_source.h:145-176; tree_scene/e_scene_recepteursp_recepteur.h:150-179 | M10 placement; design markers S1, R1 to R3 | S | Per-item colour, show-name, arrows not planned |
| M31 | Descriptions on sources and receivers | Free text | niche | Absent | tree_scene/e_scene_sources_source_properties.h:58; tree_scene/e_scene_recepteursp_recepteur_proprietes.h:55 | No field (model.rs:536-613) | S | |
| M32 | Point receiver: position, label, orientation | | core | Planned | tree_scene/e_scene_recepteursp.h:192; tree_scene/e_scene_recepteursp_recepteur_proprietes.h | model.rs:572-594; M10 gate (e) RECEIVER_OUTSIDE, LABEL_UNSAFE | S | Orientation not on the design |
| M33 | Receiver orientation point | Aim a receiver at a point | niche | Absent | tree_scene/e_scene_recepteursp_recepteur.h:53-56, :198-219 | Vector only (model.rs:580-582) | S | |
| M34 | Link a receiver to a source | Follows or aims at a moving source | niche | Absent | SystemScript/preceiv_sourceTracker/__init__.py | Not planned | M | |
| M35 | Receiver background noise | Spectrum used by STI | niche | Core only | tree_scene/e_scene_recepteursp_recepteur.h:98-100 | model.rs:583-586 | S | |
| M36 | Receiver directivity | One option, "Omnidirectional" | niche | Not porting | tree_scene/e_scene_recepteursp_recepteur_proprietes.h:52-54; not read by src/lib_interface/data_manager/base_core_configuration.cpp:229-258 | No field (model.rs:572-594) | - | Nothing to port |
| M37 | Point-receiver groups | Nested folders | common | Absent | tree_scene/e_scene_recepteursp.h:193; projet.cpp:1437 | No group field; import-proj refuses nested receiver groups (proj.rs:910-916) | M | Such .proj files fail to import |
| M38 | Receiver grid generator | Rows x columns of receivers | common | Deferred | SystemScript/recp_tool/__init__.py:13-94; Docs/tutorial_industrial_hall.rst:270-310 | plan.md:133-134; design:98 "Audience grid" | M | |
| M39 | Receiver-group actions | Rotate, translate, orient all at a point | common | Absent | SystemScript/recp_tool/__init__.py:95-178 | Not planned | S | Needs M37 |
| M40 | Scene surface receiver | Map on chosen model faces | common | Core only | tree_scene/e_scene_recepteurss.h:136; Docs/tab_scene.rst:284-298 | model.rs:596-630; M6 gate (a) (plan.md:192); no screen creates one | M | M12 draws maps, but a new model cannot have one |
| M41 | Cutting-plane receiver | Map on a plane: A, B, C, resolution | common | Core only | tree_scene/e_scene_recepteurss.h:137; tree_scene/e_scene_recepteurss_recepteurcoupe.h:89-104; Docs/tutorial_Elmia_hall.rst:112-125 | model.rs:622-629; contract:123; proj.rs:142-143. The design's "Section plane" is a view tool (design:143) | M | |
| M42 | Cutting-plane display before a run | Draws the plane grid and corners | common | Absent | tree_scene/e_scene_recepteurss_recepteurcoupe.h:118-195 | Not in M10; M12 draws maps after a run | S | |
| M43 | Enable or disable zones, surface receivers, planes | Keep an item but leave it out of the run | common | Core only | tree_scene/e_scene_encombrements_encombrement_proprietes.h:54; tree_scene/e_scene_recepteurss_recepteur_proprietes.h:46; tree_scene/e_scene_recepteurss_recepteurcoupe_proprietes.h:46 | model.rs:604, :667; core/config_xml/write.rs:490, :523 | S | From the critic |
| M44 | "Clean" a face list | Empty a receiver's or zone's faces | niche | Absent | tree_scene/e_scene_groupesurfaces_groupe.cpp:448-453; projet.cpp:1653-1656 | No GUI edits these lists | S | From the critic |

### 2.3 Calculation

| # | Feature | What it does upstream | Imp. | Status | Upstream receipt | Plan / code receipt | Size | Note |
|---|---|---|---|---|---|---|---|---|
| C1 | Run SPPS from the project | Run command, solver choice, run folder, launch | core | Planned | tree_core/e_core_sppscore.h:47-173; tree_core/e_core_core.h:113-118; projet.cpp:643-847 | core/run/manager.rs:1-24; cli `simpa run`; M6 (plan.md:190-203); M11 (raw:209); design:154-157 | S | Run is disabled today (ui/chrome/MenuBar.tsx:89-91) |
| C2 | TCR classical theory | Sabine and Eyring RT, direct and total fields | common | Planned | tree_core/e_core_tccore.h:43-109; src/ctr/TC_CalculationCore.cpp | core/config_xml/write.rs:343-345; M6 gate (b); M11; M12 Sabine/Eyring table | S | |
| C3 | TLM solver entry | Legacy node | niche | Not porting | tree_core/e_core_tlmcore.h:43-78; no TLM in the install list (CMakeLists.txt:784) | Not in the plan; backlog:62 names it for the old GUI | XL | No upstream solver exists |
| C4 | Diffusion-equation core (md_octave) | Experimental Python and Octave solver | niche | Absent | currentRelease/ExperimentalCore/md_octave/; currentRelease/ExperimentalScript/md_octave/ | Not planned | XL | Not installed by upstream's Windows build |
| C5 | Plugin calculation cores | Python-defined solvers | niche | Absent | projet.cpp:801-808; resources/doc/tutorial/script_tutorial/user_core/ | SolverKind is Spps or Tcr only (ops.rs:61-63) | L | |
| C6 | TCR external-core mode | TCR as a helper for another solver | niche | Absent | src/ctr/data_manager/core_configuration.cpp:13-34 | Writer uses GUI mode only (write.rs:343-345) | S | Not reachable from upstream's GUI |
| C7 | Particles per source | Accuracy against run time | core | Design only | tree_core/e_core_sppscore.h:158 | model.rs:916-918; contract:101; design:668 "Particles per band: auto" | S | "auto" has no counterpart in core (raw critique) |
| C8 | Particles saved for animation | How many particles go to .pbin | common | Core only | tree_core/e_core_sppscore.h:159, :186-203 | model.rs:919-921, default 0 at :963 | S | M12 playback needs this above 0 |
| C9 | Collision statistics CSVs | Surface and receiver hit logs | niche | Ready | src/spps/data_manager/core_configuration.cpp:49-58 (not in upstream's GUI) | model.rs:950-953 | S | More than upstream's GUI offers |
| C10 | Simulation length | | core | Design only | tree_core/e_core_core_config.h:73 | model.rs:922-923; design:669 | S | |
| C11 | Time step | | core | Design only | tree_core/e_core_core_config.h:74 | model.rs:924-925; design:670 | S | |
| C12 | Method: Random or Energetic | | common | Core only | tree_core/e_core_sppscore.h:38-42, :153-163; Docs/code_configuration_SPPS.rst:51-58 | model.rs:853-869, :930 | S | Upstream's docs: Random for drafts, Energetic for final results |
| C13 | Particle extinction limit | | niche | Core only | tree_core/e_core_sppscore.h:52 | model.rs:939-941 | S | |
| C14 | SPPS air absorption on/off | | common | Core only | tree_core/e_core_sppscore.h:160 | model.rs:931-932 | S | |
| C15 | Fitting-zone scattering on/off | | niche | Core only | tree_core/e_core_sppscore.h:161 | model.rs:933-934 | S | |
| C16 | Direct field only | | niche | Core only | tree_core/e_core_sppscore.h:162 | model.rs:935-936 | S | |
| C17 | Transmission on/off | | common | Core only | tree_core/e_core_sppscore.h:53 | model.rs:937-938 | S | |
| C18 | Receiver sphere radius | | common | Core only | tree_core/e_core_sppscore.h:54 | model.rs:942-943; contract:76-77 | S | The rebuild warns when a wall cuts the sphere |
| C19 | Random seed | Reproducible single-thread runs | niche | Core only | tree_core/e_core_sppscore.h:84-86 | model.rs:926-929 | S | |
| C20 | Map quantity: intensity or SPL | | common | Core only | tree_core/e_core_sppscore.h:56-65 | model.rs:871-887, :944-945 | S | |
| C21 | Maps per frequency band | | common | Core only | tree_core/e_core_sppscore.h:77-80 | model.rs:946-947 | S | Without it the map band selector (R41) has only Global |
| C22 | Echogram per source | Each source's echogram at each receiver | common | Core only | tree_core/e_core_sppscore.h:81-83 | model.rs:948-949, default off (:975) | S | Multi-source projects get no decay parameters while it is off |
| C23 | TCR air absorption on/off | | common | Core only | tree_core/e_core_tccore.h:57 | model.rs:988-990 | S | |
| C24 | TCR per-band map switch | | niche | Not porting | tree_core/e_core_tccore.h:46-50 | model.rs:984-985 | - | The switch has no effect in TCR |
| C25 | Frequency band selection | Which bands each solver computes | core | Design only | tree_core/e_core_core_bfreqselection.h:41-165; projet.cpp:670-692 | bands.rs; model.rs:954-955, :991-992; design:671 shows "Bands 1/1 oct · 125–4k" read-only | S | No picker is drawn. A refusal when every band is off was not found |
| C26 | Band presets | Octave or third-octave, "Building/Road" ranges | common | Absent | tree_core/e_core_core_bfreqselection.h:90-140; Docs/code_configuration_frequency_bands.rst:8-32 | 0 hits for "preset" in raw; old GUI had them (backlog:36, Landed) | S | |
| C27 | Air conditions | Temperature, humidity, pressure | core | Design only | tree_scene/e_scene_projet_environnement.h:123-127 | model.rs:818-851; design:672 "Air absorption 20 °C · 50 %" | S | Pressure not shown; editing not specified |
| C28 | User-defined air absorption | One value for all bands | niche | Core only | tree_scene/e_scene_projet_environnement.h:169-174 | model.rs:751-816 | S | Explicit unit required, unlike upstream |
| C29 | Sound-speed gradient and ground roughness | Outdoor refraction | niche | Core only | tree_scene/e_scene_projet_environnement.h:117-121; src/lib_interface/coreinitialisation.cpp:133-150 | model.rs:830-835 | S | |
| C30 | Meteorological presets | Very favourable to very unfavourable | niche | Absent | tree_scene/e_scene_projet_environnement.h:43-86, :129-142 | Raw values only (model.rs:818-836) | S | |
| C31 | Ground-type presets | Water to dense urban | niche | Absent | tree_scene/e_scene_projet_environnement.h:52-114, :144-165 | Not planned | S | |
| C32 | Pre-run checks | No source, no faces, no band, no solver: refuse | core | Planned | projet.cpp:649-659, :670-692, :740-747 | contract:53-149; core/run/manager.rs:209-290; M10 inline validator; design:683 | S | Much stronger than upstream |
| C33 | Mesh automatically on each run | | core | Ready | projet.cpp:751-755 | core/run/manager.rs:641-669; contract:131 | S | |
| C34 | Dated run folder per run | | core | Ready | projet.cpp:716-729 | core/run/manager.rs:375-391; M6 gate (h) | S | |
| C35 | Save the project at run time | Autosave, and a project copy in the run folder | niche | Absent | projet.cpp:782-785, :816-820 | run.json keeps the project's sha256 (core/run/manifest.rs:45-53), not a copy | S | |
| C36 | Progress with elapsed and remaining time | | core | Planned | manager/processManager.cpp:56-75; projet.cpp:795 | M11 "live '#' progress" (raw:209); core/run/classify.rs:336-372 | S | Elapsed and remaining time not named |
| C37 | Cancel a run | | core | Planned | manager/processManager.cpp:102-135 | Job Object (m56:636-637); M11 gates (c), (d) | S | Upstream's cancel is reported failing (#69, #181) |
| C38 | Solver output in the console | Warnings and errors | core | Planned | manager/processManager.cpp:41-100 | core/run/classify.rs; contract:345-366; M11 | S | |
| C39 | Calculation time | | niche | Core only | projet.cpp:799-814 | core/run/manifest.rs:159-161; the M11 Runs columns omit it | S | |
| C40 | Job list (batch runs) | Queue runs across projects | common | Absent | SystemScript/job_tool/__init__.py:38-153; Docs/code_configuration_job.rst | Not planned; the CLI can be looped | M | Upstream's is broken on Python 3.10+ (job_tool/__init__.py:102) |
| C41 | Particle fate statistics per band | Absorbed, lost, alive | common | Planned | src/spps/input_output/reportmanager.cpp:482-513 | core/run/stats.rs; contract:458-459; M11 "per-band particle loss %", gate (a) | S | Now a pass/fail verdict |

### 2.4 Results

| # | Feature | What it does upstream | Imp. | Status | Upstream receipt | Plan / code receipt | Size | Note |
|---|---|---|---|---|---|---|---|---|
| R1 | Run list (results browser) | Every run and its results | core | Planned | tree_rapport/e_report_file.cpp:242-350; projet.cpp:822-842; Docs/tab_results.rst | M11 Runs tab "run history from the manifests" (raw:209); core/run/manifest.rs | M | Placeholder only (Panels.tsx:132-141) |
| R2 | Label a run | Rename the run | common | Absent | tree_rapport/e_report_file.cpp:106-151 | M11 lists status, reason, loss, hashes only | S | Store as metadata, not a folder rename |
| R3 | Delete a run | With confirmation | common | Absent | tree_rapport/e_report_file.cpp:351-369; projet.cpp:1658-1668 | Not in M11 or M12 | S | |
| R4 | Open a run folder or file in the OS | Explorer, default app | common | Absent | tree_rapport/e_report_folder.h:81-110; tree_rapport/e_report_unknown_file.cpp:52-107 | M9 gate (f) allows no "shell:" capability (raw:191) | S | Needs a Rust-side command |
| R5 | Refresh a results folder | | niche | Absent | tree_rapport/e_report_file.cpp:370-378 | Not planned | S | An app-managed run list may not need it |
| R6 | Per-run record of inputs | config.xml and provenance with each run | common | Ready | projet.cpp:816-820 | core/run/manifest.rs:1-9, :131-174 | S | No project copy (C35) |
| R7 | Receiver level table | dB per band per time step | common | Absent | tree_rapport/e_report_gabe_recp.cpp:53-150 | Data in the JSON (core/results/report.rs:326-329); no view | S | |
| R8 | Echogram and Schroeder curve chart | | core | Planned | tree_rapport/e_report_gabe_recp.cpp:230-270 | M12 "Decay charts in uPlot" (raw:219) | M | |
| R9 | Spectrum chart at a receiver | | common | Absent | tree_rapport/e_report_gabe_recp.cpp:160-200 | Not in M12 | S | |
| R10 | Level per source (.recps) | | niche | Absent | tree_rapport/e_report_gabe_recps.cpp:53-213 | Data read (core/results/spps.rs) | S | |
| R11 | Per-source echograms and parameters | | common | Core only | src/spps/input_output/reportmanager.cpp:620-656 | core/results/report.rs:396-424; docs/results.md:474-480 | S | The switch is C22 |
| R12 | SPL per band at receivers | | core | Planned (at risk) | calc.cpp:220-240 | core/params/decay.rs:419; M7 gate (c); M8 covers SPL (plan.md:137); M12(b) hides it without a PASS bed | S | The 00:20 M8 decisions name only T30 and EDT as gated |
| R13 | A-weighted level dB(A) | | common | Absent | calc.cpp:264-306 | No hits in crates/ | S | |
| R14 | T30 (and T20) | | core | Planned (at risk) | calc.cpp:190-218 | core/params/decay.rs:335-400; M8 gate (handoff:468); M12 "RT per band" | S | Tutorial 1 at 150k: 540 of 540 refused (handoff:358-364) |
| R15 | T15 and chosen decay ranges | | common | Absent | calc.cpp:803-823 | docs/params.md:549 ("which we do not", no reason) | S | Upstream's default list is "15;30" |
| R16 | EDT | | core | Planned (at risk) | calc.cpp:936 | core/params/decay.rs; M8 gate | S | Same tutorial-1 refusals (early_unresolved) |
| R17 | C50 and C80 | | core | Core only | calc.cpp:350-384 | core/params/decay.rs:403, :575-576; C80 reported not gated (handoff:469-470), C50 no bed; M12(b) hides both | S | C80 is on the design's map selector |
| R18 | D50 | | common | Core only | calc.cpp:386-416 | core/params/decay.rs:408, :577; as R17 | S | |
| R19 | Centre time Ts | | common | Core only | calc.cpp:418-443 | core/params/decay.rs:414; no bed | S | |
| R20 | Custom time limits for C, D, LF | e.g. C35, D80 | common | Absent | calc.cpp:806-823 | Limits hard-coded (core/params/decay.rs:575-577) | S | |
| R21 | Early stage support ST_early | | niche | Absent | calc.cpp:313-348 | Not in core/params.rs:73-90 | S | |
| R22 | STI at receivers | | common | Deferred | calc.cpp:586-790; nc_curves.cpp | plan.md:141; raw:348 | M | Upstream bug: every STI uses female weights (calc.cpp:748) |
| R23 | NC-curve background for STI | | niche | Deferred | calc.cpp:620-633, :812 | With STI | S | From the critic |
| R24 | "Global" all-band row | | common | Ready | calc.cpp:895-901 | core/results/report.rs:355-370 | S | |
| R25 | "Average" row across bands | | common | Not porting | calc.cpp:902 | docs/params.md:65-70 | S | ISO single numbers (500 Hz to 1 kHz means) are not computed either |
| R26 | Decay parameters on a multi-source sum | | common | Not porting | calc.cpp:860-898 | docs/results.md:469-473 | S | See C22 |
| R27 | Schroeder curve table | | common | Core only | calc.cpp:961-990 | core/results/report.rs:341-344 | S | Thinned, not one point per step |
| R28 | Lateral fractions LF and LFC | | common | Absent | calc.cpp:446-516, :1234-1300 | Data read (core/results/spps.rs:100-106); raw:348 | M | Upstream swaps the two column labels |
| R29 | Sound strength G | | common | Absent | calc.cpp:554-584 | Not in core/params.rs; source power read (report.rs:333) | M | |
| R30 | Late lateral level LG | | niche | Absent | calc.cpp:518-552 | Not planned | S | |
| R31 | .gap table view | | niche | Absent | tree_rapport/e_report_gabe_gap.cpp:54-122 | Data read (core/results/spps.rs:95-116) | S | |
| R32 | Receiver intensity table | | niche | Absent | src/spps/input_output/reportmanager.cpp:658-758 | Read (spps.rs:107-108), not emitted | S | |
| R33 | Intensity vector animation | | niche | Deferred | tree_rapport/e_report_rpi.h:70-74; 3dengine/Core/Recepteurs_ponctuel_intensity.cpp | plan.md:133-134 | M | |
| R34 | Total room energy decay | | niche | Core only | src/spps/input_output/reportmanager.cpp:515-538 | core/results/report.rs:554 | S | |
| R35 | TCR main results | A, RT and L per band, Sabine and Eyring | common | Planned | src/ctr/main_tc.cpp:112-143 | core/results/tcr.rs:40-55; M12 "a Sabine/Eyring table" | S | |
| R36 | TCR receiver levels | Direct, Sabine and Eyring totals | common | Core only | src/ctr/input_output/reportmanager.cpp:106-152 | core/results/tcr.rs:57-64; no view named | S | |
| R37 | TCR maps | | common | Planned | src/ctr/TC_CalculationCore.cpp:318-420 | M12 surface maps; core/results.rs:426-437 | S | M12 does not say which solvers |
| R38 | SPPS level map (summed) | | core | Planned | tree_rapport/e_report_recepteurssvisualisation.h:175-197; Recepteurs_surfacique.cpp:206-540 | M12 maps, gate (c) (raw:219-220); core/formats/csbin.rs | L | |
| R39 | Time-step map animation | | common | Planned | tree_rapport/e_report_recepteurssvisualisation.h:172 | M12 "faces x steps", Animator timeline | M | |
| R40 | Cumulative map animation | | niche | Absent | tree_rapport/e_report_recepteurssvisualisation.h:173 | Not named | S | |
| R41 | Band or Global choice for maps | | core | Design only | Docs/code_configuration_SPPS.rst:63-64 | design:445 "Band 1 kHz"; not in the M12 text | S | |
| R42 | Parameter maps: T30, EDT, C80, D50 | Per-face parameters | common | Planned | tree_rapport/e_report_recepteurssvisualisation.h:184-195; calc.cpp:1001-1106 | plan.md:132 "map selector ... in M12"; design:438-443; not in the raw M12 deliverable or gates | L | No per-face parameter code yet; M12(b) hides non-PASS |
| R43 | Other parameter maps | RT15, C50, Ts, ST | niche | Absent | calc.cpp:1048-1069 | Design selector omits them | S | |
| R44 | STI map | | common | Deferred | calc.cpp:1108-1231 | plan.md:141; on design:443 | M | |
| R45 | Map opacity, front/back | | common | Absent | tree_rapport/e_report_recepteurssvisualisation.h:63-65 | Not planned | S | |
| R46 | Smooth or flat map colouring | | common | Absent | tree_rapport/e_report_recepteurssvisualisation.h:101 | Not planned | S | |
| R47 | Fixed colour range | Min/max to compare maps | common | Absent | tree_rapport/e_report_recepteurssvisualisation.h:102-103 | Not planned | S | |
| R48 | Iso-contour lines | | common | Absent | tree_rapport/e_report_recepteurssvisualisation.h:137-142; Recepteurs_surfacique.cpp:490-600 | Not planned; old GUI had them (backlog:29, Landed) | M | |
| R49 | Palette choice | 7 palettes incl. NF S 31-130 | common | Absent | resources/Bitmaps/iso/*.gpl; tree_userpref/e_userprefitemisotemplate.hpp:57-100 | M12 fixes black-red-yellow (raw:219; docs/design/README.md:17) | S | |
| R50 | Map legend | Colour bar with units | core | Design only | Recepteurs_surfacique.cpp:759-830 | design:164-168, :743; not in the M12 text | S | |
| R51 | Value probe on maps | Click a face, read its value | common | Absent | main.cpp:430; projet.cpp:1816-1821 | Not in M12 | S | |
| R52 | Difference map between runs | | common | Planned | IHM/RecepteurSOperationDialog.cpp; 3dengine/tools/recepteursurf_difference.cpp:128-190; Docs/tutorial_industrial_hall.rst:381-450 | plan.md:132; design:446-447; M12 gate (f) variant switch | M | Upstream compares any runs, several at once, and saves the result |
| R53 | Particle animation | | common | Planned | tree_rapport/e_report_partvisualisation.h:78-86; Particules.cpp:320-365 | M12 particle playback, gate (d) | M | Needs C8 above 0 |
| R54 | Particle trails ("Rays") | | niche | Absent | Particules.cpp:366-386 | Not named | S | |
| R55 | Animation transport controls | Play, pause, step | common | Planned | main.cpp:218-220, :416-421 | M12 "one shared Animator timeline" | S | |
| R56 | Export the 3D view as an image | PNG, JPG, BMP | common | Absent | main.cpp:289; projet.cpp:364-383 | Not in M10 to M13 | S | |
| R57 | Parameter tables | Per receiver and band | core | Planned | projet.cpp:3159-3196; IHM/GabeDataGrid.cpp | M12 Acoustics tab, gate (a) | M | No generic viewer for other tables |
| R58 | Copy table cells (TSV) | | common | Absent | IHM/PropGrid.cpp:170-205 | Not in M12 | S | |
| R59 | Save a table as CSV | | core | Absent | IHM/GabeDataGrid.cpp:338-352, :612-648; Docs/tutorial_teaching_room.rst:132 | Only `results --json` (crates/simpa/src/results_cmd.rs:54-140) | S | |
| R60 | Chart from a table selection | "New diagram" | common | Absent | IHM/GabeDataGrid.cpp:488-522; IHM/DialogDiagramCreator.cpp | Not planned | M | |
| R61 | Editable spreadsheet from a selection | | niche | Absent | IHM/GabeDataGrid.cpp:362-449 | Not planned | M | |
| R62 | Chart zoom, show/hide, legend | | common | Absent | IHM/simpleGraph.cpp:1295-1345 | Not specified; uPlot has it built in | S | |
| R63 | Export a chart as an image | | common | Deferred | IHM/simpleGraphManager.cpp:84-119 | backlog:62 "graph export" | S | |
| R64 | Chart display styles | | niche | Absent | IHM/simpleGraphDialogs.cpp:533-700 | Not planned | M | |
| R65 | All receivers in one table | "Merge point receivers" | common | Design only | SystemScript/recp_res_tool/__init__.py:8-66 | cli `results --json` holds every receiver; design Results list "Receivers · SPL 1 kHz" | S | Design shows SPL at 1 kHz only |
| R66 | Particle paths to CSV | | niche | Core only | SystemScript/sample/parttocsv.py | `simpa dump pbin` prints hex floats (core/formats/pbin.rs:215-256) | S | |
| R67 | Open results of upstream runs | report/ inside a .proj | niche | Not porting | Docs/tutorial_teaching_room.rst:24; projet.cpp:2300-2338 | proj.rs:5-6; M7 "verified runs only" | M | |
| R68 | Report document ("Make report") | HTML report of a run | common | Design only | resources/doc/tutorial/script_tutorial/SppsReportSample/__init__.py:117-176 | design:466 "Export report"; in no milestone | M | Upstream ships it as a sample script (Python 2 idioms) |
| R69 | Area by level of a map | m² per level class | niche | Absent | SppsReportSample/recsurf_report_stats.py:22-80 | Not planned | S | From the critic |
| R70 | Normalise SPL to a reference receiver | | niche | Absent | resources/doc/tutorial/script_tutorial/recp_res_norm/__init__.py:22-102 | Not planned | S | From the critic; likely dead upstream |

### 2.5 Project, app and workflow

| # | Feature | What it does upstream | Imp. | Status | Upstream receipt | Plan / code receipt | Size | Note |
|---|---|---|---|---|---|---|---|---|
| A1 | New project | | core | Ready | main.cpp:184, :636-644 | ui/chrome/MenuBar.tsx:26-33; tauri/commands.rs:201-208 | S | No save prompt (A9) |
| A2 | Open project | | core | Ready | main.cpp:185, :946-963 | MenuBar.tsx:9-24; commands.rs:220-228; core/schema/json.rs:135-146 | S | .simpa only |
| A3 | Open an upstream .proj | Tutorials and existing projects | core | Core only | main.cpp:948; projet.cpp:2507-2535 | cli `import-proj`; proj.rs:1-145; M4 (plan.md:206-212). M10 says "import"; its gates test only .ply | S | Refuses volumes, balloons, grouped receivers, extra TetGen flags; drops stored runs |
| A4 | Save project | | core | Planned | main.cpp:191; projet.cpp:2558-2608 | M10 "save, open, and undo/redo" (raw:199); json.rs:147-171 (atomic) | S | |
| A5 | Save as | | core | Planned | main.cpp:192, :965-976 | Implied by M10 "save"; dialog:allow-save already granted | S | Name it in the M10 gate |
| A6 | Save a copy | | common | Deferred | main.cpp:193; projet.cpp:2578-2581 | backlog:62 "save-a-copy" | S | |
| A7 | Recent projects | Last 5 | common | Absent | main.cpp:195-201, :534-543 | Not planned (MenuBar.tsx:70-79) | S | |
| A8 | Unsaved-changes marker | | core | Design only | projet.cpp:2536-2556 | design:37-40 dot; no dirty flag (tauri/bridge.rs:20-35) | S | |
| A9 | Save prompt before New, Open, Exit | Yes, No, Cancel | core | Absent | main.cpp:1015-1045, :1080-1114 | No close handler in app/; bridge.rs:70-75 replaces silently | S | |
| A10 | Undo and redo | | core | Planned | main.cpp:213-215; data_manager/projet_undo_redo.cpp | M10 gate (f); ops.rs:1169-1244; commands.rs:256-266 | S | Unlimited and exact; upstream keeps 5 snapshots |
| A11 | Undo on/off preference | | niche | Not porting | projet.cpp:2393-2395; Docs/menu_edition.rst | Op-based undo (ops.rs:1169-1244) | S | Exists only because upstream's undo is slow |
| A12 | Preferences | Animation rate, display toggles, legend, colours | common | Deferred | projet.cpp:2341-2461 | backlog:62 "preferences dialog"; theme fixed (docs/design/README.md:15-25) | M | |
| A13 | Language selection and translations | French, Polish, Portuguese, Chinese | common | Deferred | IHM/languageSelection.cpp; lang/*.po | backlog:62 "language selection"; strings hard-coded | L | |
| A14 | Embedded Python console | | niche | Absent | main.cpp:330-335; python_interface/pythonshell.cpp | No decision in the plan (only the old GUI's backlog:80) | XL | Needs a decision |
| A15 | Python plugin API and hooks | Menus, events, custom elements | niche | Absent | python_interface/py_ui_module/Application.cpp:61-142; element_pywrap.cpp:167-268 | As A14 | XL | Needs a decision |
| A16 | Formulas in number fields | "3*4" | niche | Absent | data_manager/e_data_entier.h:205-218 | Not planned | S | |
| A17 | Headless and scripted operation | Import, run, results without the GUI | common | Ready | Upstream: Python scripts only (docs/upstream-laydown.md:123-126) | cli USAGE: import, import-proj, check, repair, validate, mesh, run, results, all `--json` | S | Beyond upstream |
| A18 | libsimpa Python module | Solver files from Python | niche | Absent | src/python_bindings/libsimpa.i:195-206 | `simpa dump` and `results --json` partly substitute | M | |
| A19 | Help > Website | | niche | Absent | main.cpp:300 | Help menu disabled (MenuBar.tsx:64-68) | S | |
| A20 | Help > Online documentation | | common | Absent | main.cpp:301 | As A19 | S | |
| A21 | Help > Offline PDF | | common | Absent | main.cpp:302; resources/doc/documentation.pdf | M13 bundles no docs | S | |
| A22 | User manual | About 70 pages, 3 tutorials | common | Absent | Docs/*.rst | docs/ holds developer specs only | L | contract and docs/params.md could seed it |
| A23 | About dialog | Versions, credits, licence | common | Absent | main.cpp:304; IHM/AboutDialog.cpp:37-103 | Versions in the status bar (ui/App.tsx:45) and `simpa --version` | S | |
| A24 | Update check | | common | Planned | SystemScript/check_version/__init__.py:15-25 (never runs: python_interface/pythonshell.cpp:140-141) | M11 "The updater, if shipped" (raw:209); open decision raw:357 | S | Conditional |
| A25 | Console log with levels and times | | core | Ready | main.h:336-390 | ui/chrome/Panels.tsx:90-103 | S | |
| A26 | Export the console to a file | | niche | Absent | main.cpp:292, :523-533 | Not planned | S | |
| A27 | Clear the console | | niche | Absent | main.cpp:294 | Not planned | S | |
| A28 | Dockable panels, floating 3D view | | niche | Not porting | main.cpp:450-496, :605-629 | raw surveys[3] recommended_stack[7] (fixed layout per approved design); docs/design/README.md:19-24 | L | Loses a second-monitor 3D view |
| A29 | Rename and delete any element | F2, Del | core | Core only | IHM/uiTreeCtrl.cpp:108-124; projet.cpp:1479-1522 | ops.rs:115-231; not named in M10 | S | |
| A30 | Multi-select and drag in the tree | | niche | Absent | IHM/uiTreeCtrl.cpp:699-745 | Not planned | M | |
| A31 | Project name and description | | common | Core only | tree_scene/e_scene_projet_userconfiguration.h:50-51 | model.rs:66-67; ops.rs:109-114; tab shows the name (MenuBar.tsx:81-83) | S | |
| A32 | Project author and date | | niche | Absent | tree_scene/e_scene_projet_userconfiguration.h:52-55 | No fields (model.rs:62-87) | S | Format version 2 |
| A33 | Reopen the last project at startup | | common | Absent | projet.cpp:1986-2000 | Starts empty (tauri/main.rs:103-117) | S | |
| A34 | Crash and session recovery | Autosave, offer recovery | common | Absent | main.h:474-524 | raw risks cover panics only; no autosave | M | |
| A35 | Several instances at once | | niche | Ready | main.h:476-486 | No single-instance plugin; own session per process (bridge.rs:43-49) | S | Not tested |
| A36 | Choice of data folder | | niche | Absent | main.h:466-472; main.cpp:977-995 | Runs go beside the project (cli `--runs`) | S | A runs-root setting would help with big runs |
| A37 | Open a project from the command line | | common | Ready | main.h:549-554 | tauri/main.rs:40-69, :107-111 (`--project`) | S | |
| A38 | Drop a file on the window to open it | | common | Absent | main.h:605-614; main.cpp:1131 | tauri.conf.json dragDropEnabled true, no handler in app/ui/src | S | Old GUI had it (backlog:40, Landed) |
| A39 | Keyboard shortcuts | Ctrl+N/O/S/Z/Y, Ctrl+C/V, Del, F2 | common | Absent | main.cpp:184-215; IHM/uiTreeCtrl.cpp:108-124 | No key handling in app/ui/src; design shows F5 and Ctrl K only | S | |
| A40 | Windows installer | | core | Planned | CMakeLists.txt:751-831 | M13 (raw:228-231) | M | |
| A41 | macOS and Linux packages | | common | Absent | CMakeLists.txt:789-829; org.noise_planet.i-simpa.yml | Open decision raw:359 | L | |
| A42 | File association | Double-click a project | common | Absent | org.noise_planet.i-simpa.desktop (Linux only) | Not in M13 or tauri.conf.json | S | |
| A43 | Bundled tutorial projects | 3 tutorials and their files | common | Absent | resources/doc/tutorial/; Docs/tutorial_*.rst | Test fixtures only; M13 ships solvers and licences | M | |
| A44 | Bundled validation projects | Air absorption and clarity, with spreadsheets | niche | Absent | resources/doc/validation/ | M8 imports the air one internally; nothing ships | S | |
| A45 | Scripting samples | | niche | Absent | resources/doc/tutorial/script_tutorial/; SystemScript/sample/ | No scripting API planned | S | |
| A46 | Project format version and provenance | | core | Ready | projet.cpp:3128-3132 | model.rs:18; json.rs:104-116; core/run/manifest.rs | S | No v1-to-v2 migration code yet |
| A47 | Project archive with its runs | One file carries a study and its results | common | Absent | projet.cpp:2583-2607, :2284-2299 | Runs in separate folders (core/run/manager.rs:6-9); open decision raw:352 | M | From the critic |

---

## 3. The gap list

Every feature that is not covered, with the critic's corrections applied. Order: importance (core, common, niche), then size (S, M, L, XL). The "When" column is the recommendation from section 4.

### 3a. Absent or deferred (124)

**Core (3)**

| # | Feature | Size | Status | When | Why / what it takes |
|---|---|---|---|---|---|
| A9 | Save prompt before New, Open, Exit | S | Absent | Before v1 | Once M10 brings editing, New and Open throw work away silently (bridge.rs:70-75). A close-requested handler and one dialog. |
| R59 | Save a result table as CSV | S | Absent | Before v1 | Today numbers leave the app only as `results --json`. Serialise the report tables. |
| G19 | Send selected faces to a new group | M | Deferred | Before v1 | Any CAD layer that mixes materials is stuck without it, and scene-fitted zones need it. One invertible regroup op, which G20 reuses. |

**Common, S (37)**

| # | Feature | Status | When | Why / what it takes |
|---|---|---|---|---|
| G11 | New scene: box room | Absent | Before v1 | Tutorial 1 starts here. The 6-group box generator exists as a fixture. |
| G42 | Reset camera | Absent | Before v1 | A lost view has no way back. Frame the model on demand. |
| M1 | Reference material library | Deferred | Before v1 | Otherwise every material is typed in; the tutorials pick "30 % absorbing". The 12 materials are already in code. |
| M42 | Cutting-plane display before a run | Absent | Before v1 | Goes with M41: the only check that the plane is where intended. |
| C26 | Band presets | Absent | Before v1 | Goes with the band picker (C25). Six fixed lists over BandSet::range. |
| R58 | Copy table cells (TSV) | Absent | Before v1 | Same data as R59; a clipboard write. |
| R62 | Chart zoom, show/hide, legend | Absent | Before v1 | uPlot has it built in; configuration only. |
| A23 | About dialog | Absent | Before v1 | Version for bug reports; licence notice inside the app. |
| A39 | Keyboard shortcuts | Absent | Before v1 | Ctrl+S, Ctrl+Z, Del, F2 are expected. A key map over existing commands. |
| A42 | File association for .simpa | Absent | Before v1 | Ships with the M13 installer; `--project` already works. |
| G45 | Colour by material | Deferred | v1.x | The tutorials check assignments this way; scene-list swatches partly cover it. |
| G46 | Axis arrows and grids | Deferred | v1.x | Axis gizmo is on the design; grids later. |
| M14 | Odeon import | Deferred | v1.x | Simple line format. |
| M18 | dB(A) entry | Absent | v1.x | Noise-control users think in dB(A). |
| M28 | Source-group actions | Absent | v1.x | With M27. |
| M39 | Receiver-group actions | Absent | v1.x | With M37. |
| R2 | Label a run | Absent | v1.x | A label in run.json. |
| R3 | Delete a run | Absent | v1.x | Runs are about 100 MB on Elmia; confirm dialog, Rust command. |
| R4 | Open a run folder in Explorer | Absent | v1.x | Rust-side, so the webview keeps no shell permission. |
| R7 | Level table per time step | Absent | v1.x | Data already in the JSON. |
| R9 | Spectrum chart | Absent | v1.x | uPlot bars over spl_db. |
| R13 | dB(A) result | Absent | v1.x | Weighting and sum over SPL per band. |
| R15 | T15 and chosen decay ranges | Absent | v1.x | One more DecayRange and a bed row. |
| R20 | Custom C, D, LF limits | Absent | v1.x | Thread a list through report.rs. |
| R45 | Map opacity, front/back | Absent | v1.x | Shader uniforms. |
| R46 | Smooth or flat colouring | Absent | v1.x | Interpolate the value, not RGB. |
| R47 | Fixed colour range | Absent | v1.x | The difference map covers the main comparison. |
| R49 | Palette choice | Absent | v1.x | NF S 31-130 for French regulatory maps. |
| R51 | Value probe on maps | Absent | v1.x | Reuses M10's picking. |
| R56 | Export the 3D view as an image | Absent | v1.x | canvas.toBlob plus the dialog plugin. |
| R63 | Export a chart | Deferred | v1.x | |
| A6 | Save a copy | Deferred | v1.x | Variants cover part of the need. |
| A7 | Recent projects | Absent | v1.x | |
| A20 | Help link to online docs | Absent | v1.x | Needs A22. |
| A33 | Reopen the last project | Absent | v1.x | |
| A38 | Drop a file to open it | Absent | v1.x | A handler over the open and import paths. |
| A21 | Offline PDF | Absent | Later | Once a manual exists. |

**Common, M (22)**

| # | Feature | Status | When | Why / what it takes |
|---|---|---|---|---|
| M37 | Point-receiver groups | Absent | Before v1 (import side) | import-proj refuses a .proj with grouped receivers (proj.rs:910-916). Flatten them, or keep a group path as sources do. GUI groups in v1.x. |
| A43 | Bundled tutorial projects | Absent | Before v1 (samples) | Ship tutorials 1 to 3 as .simpa, plus the corrected Elmia. The step-by-step text can follow in v1.x. |
| G3 | Import 3DS | Absent | v1.x | Close the open decision (raw:351) before v1 and make the design's format list (design:307) match it. |
| G20 | Move faces to an existing group | Absent | v1.x | Small once G19's op exists. |
| G29 | Create and edit a box fitting zone | Absent | v1.x | Industrial halls. Imported zones already run. |
| G39 | Mesh preview with slice | Deferred | v1.x | |
| M3 | Material categories | Deferred | v1.x | Needed once CATT and Odeon libraries arrive. |
| M12 | Copy and paste elements | Absent | v1.x | Fresh ids, remapped references, batched undo. |
| M13 | CATT import | Deferred | v1.x | |
| M17 | User spectrum library | Absent | v1.x | |
| M27 | Source groups (create, edit) | Deferred | v1.x | Imported groups already survive. |
| M38 | Receiver grid generator | Deferred | v1.x | The design already draws "Audience grid". |
| C40 | Job list | Absent | v1.x | A queue over variants would beat upstream's broken one. |
| R22 | STI | Deferred | v1.x | Needs a bed against IEC 60268-16 worked examples. |
| R28 | LF and LFC | Absent | v1.x | Needs a bed. |
| R29 | Sound strength G | Absent | v1.x | M7's free-field calibration is most of the bed. |
| R44 | STI map | Deferred | v1.x | After STI and per-face parameters. |
| R48 | Iso-contour lines | Absent | v1.x | |
| A12 | Preferences | Deferred | v1.x | Animation rate and display toggles matter. |
| A34 | Crash recovery | Absent | v1.x | Periodic autosave. A9 comes first. |
| A47 | Project archive with its runs | Absent | v1.x | An explicit "export with results", for passing studies between Burhan and Michael. |
| R60 | Chart from a table selection | Absent | Later | Fixed charts cover most uses. |

**Common, L (4)**

| # | Feature | Status | When | Why / what it takes |
|---|---|---|---|---|
| A22 | User manual | Absent | v1.x | A getting-started page before v1; contract reason codes and params.md seed the rest. |
| G9 | Average model remesh | Absent | Later | Decide raw:360 first. The corrected Elmia removes the tutorial's need. |
| A13 | Language selection | Deferred | Later | String extraction across React and reason codes. |
| A41 | macOS and Linux | Absent | Later | Open decision raw:359; .pbin layout differs on LP64. |

**Niche (58)**

| # | Feature | Size | Status | When |
|---|---|---|---|---|
| G12 | Export scene as PLY | S | Absent | v1.x |
| G31 | Fitting-zone display | S | Absent | v1.x (with G29) |
| M9 | Material metadata | S | Absent | v1.x (with CATT import, which fills Description and Reference) |
| M16 | Reference spectra library | S | Absent | v1.x |
| M29 | Line of sources | S | Absent | v1.x (with M27) |
| M44 | "Clean" a face list | S | Absent | v1.x (with M40) |
| C30 | Meteorological presets | S | Absent | v1.x (with the settings panel) |
| C31 | Ground-type presets | S | Absent | v1.x (with the settings panel) |
| C35 | Save the project at run time | S | Absent | v1.x |
| R23 | NC-curve background for STI | S | Deferred | v1.x (with STI) |
| R43 | Other parameter maps (C50 etc.) | S | Absent | v1.x |
| R69 | Area by level of a map | S | Absent | v1.x |
| A19 | Help > Website | S | Absent | v1.x |
| A26 | Export the console | S | Absent | v1.x |
| A27 | Clear the console | S | Absent | v1.x |
| A36 | Runs-root setting | S | Absent | v1.x |
| A44 | Bundled validation projects | S | Absent | v1.x (ship the M8 bed report as the validation) |
| G4 | Import .poly as the scene | S | Absent | Later |
| G5 | Import .bin scene | S | Absent | Later |
| G10 | Surface meshing at import | S | Absent | Later |
| G14 | Export .mesh, .nff, .bin, .asc | S | Absent | Later |
| G23 | Invert normals by hand | S | Absent | Later |
| G27 | Convert volume to fitting zone | S | Absent | Later |
| G38 | Mesh settings per solver | S | Absent | Later |
| G41 | First-person camera | S | Absent | Later |
| M21 | Attenuation on a spectrum | S | Absent | Later |
| M31 | Descriptions | S | Absent | Later |
| M33 | Receiver orientation point | S | Absent | Later |
| C6 | TCR external-core mode | S | Absent | Later or never (no user entry upstream) |
| R5 | Refresh a results folder | S | Absent | Later or never (app-managed list) |
| R10 | Level per source table | S | Absent | Later |
| R21 | ST_early | S | Absent | Later |
| R30 | LG | S | Absent | Later |
| R31 | .gap table | S | Absent | Later |
| R32 | Intensity table | S | Absent | Later |
| R40 | Cumulative map animation | S | Absent | Later |
| R54 | Particle trails | S | Absent | Later |
| R70 | Normalise SPL to a reference | S | Absent | Later |
| A16 | Formulas in number fields | S | Absent | Later |
| A32 | Project author and date | S | Absent | Later (with the first format bump) |
| A45 | Scripting samples | S | Absent | Later |
| G21 | Face list per group | M | Absent | Later |
| G25 | Volumes | M | Absent | Later |
| G26 | Volume auto-detect | M | Absent | Later |
| G30 | Scene-fitted zone from faces | M | Absent | Later |
| G35 | Free-form TetGen parameters | M | Absent | Later (needs a policy) |
| M24 | Directivity database | M | Absent | Later |
| M34 | Link a receiver to a source | M | Absent | Later |
| R33 | Intensity vector animation | M | Deferred | Later |
| R61 | Editable spreadsheet | M | Absent | Later or never (CSV instead) |
| R64 | Chart display styles | M | Absent | Later |
| A18 | libsimpa Python module | M | Absent | Later |
| A30 | Multi-select and drag in the tree | M | Absent | Later |
| G15 | Save as upstream .proj | L | Absent | Later |
| C5 | Plugin calculation cores | L | Absent | Later |
| C4 | Diffusion-equation core | XL | Absent | Later or never |
| A14 | Embedded Python console | XL | Absent | Later (decide first) |
| A15 | Python plugin API | XL | Absent | Later (decide first) |

### 3b. Built in core, no GUI anywhere in the plan (44)

A GUI user cannot reach these. Most are one field each. One settings panel (about 25 fields with inline validator messages, over SetSolverSettings, SetEnvironment, SetBands and SetBandComputed, ops.rs:213-223 and commands.rs:247-254) would clear C8 to C29 as one M-sized piece of work.

| # | Feature | Imp. | Size | When | What it takes |
|---|---|---|---|---|---|
| A3 | Open an upstream .proj | core | S | Before v1 | A File > Import entry over `import-proj`, showing its notes and refusals. Name it in M10's gate. |
| G8 | Repair at import | core | S | Before v1 | Run the safe repairs during GUI import, or offer Repair after a refusal. |
| G18 | Add a surface group | core | S | Before v1 | A verb in the scene list over existing ops. |
| A29 | Rename and delete any element | core | S | Before v1 | Same. |
| R17 | C50 and C80 | core | S (plus a bed) | Before v1 (decide) | Hidden by M12(b). C80 is on the design's map selector: choose the reference and gate it, or take it off the design. |
| M40 | Scene surface receiver | common | M | Before v1 | M12 draws maps, but only imported projects have a receiver to draw. Pick groups into a receiver. |
| M41 | Cutting-plane receiver | common | M | Before v1 | The most common map output. A three-point gizmo with a default at floor + 1.6 m, as upstream does. |
| M5 | Reflection law column | common | S | Before v1 | A dropdown column in the M10 grid. |
| M26 | Source on/off | common | S | Before v1 | A checkbox. |
| C8 | Particles saved | common | S | Before v1 | Default 0 leaves M12 playback empty. A field, or a non-zero default. |
| C12 | Random or Energetic | common | S | Before v1 | Upstream's docs tell users to switch for final results. |
| C21 | Maps per band | common | S | Before v1 | Needed for the design's map band selector (R41). |
| C22 | Echogram per source | common | S | Before v1 | Default it on with two or more sources, or multi-source projects get no decay parameters. |
| R18 | D50 | common | S (plus a bed) | Before v1 (decide) | As R17. |
| G28 | Show imported fitting zones | common | S | v1.x | List them in the scene list, with G29. |
| G32 | Mesh on demand | common | S | v1.x | |
| G34 | Surface-receiver area constraint | common | S | v1.x | Needs a sensible default when M40 ships. |
| G36 | preprocess.exe toggle | common | S | v1.x | Decide the default before v1 (m56:244-245). |
| M43 | Enable or disable zones, receivers, planes | common | S | v1.x | |
| C14 | SPPS air absorption on/off | common | S | v1.x | |
| C17 | Transmission on/off | common | S | v1.x | |
| C18 | Receiver sphere radius | common | S | v1.x | |
| C20 | Map quantity | common | S | v1.x | |
| C23 | TCR air absorption on/off | common | S | v1.x | |
| R11 | Per-source results view | common | S | v1.x | |
| R19 | Ts | common | S (plus a bed) | v1.x | |
| R27 | Schroeder curve table | common | S | v1.x | Comes with CSV. |
| R36 | TCR receiver levels | common | S | v1.x | |
| A31 | Project name and description | common | S | v1.x | |
| G33 | Mesh quality settings | niche | S | v1.x | |
| M35 | Receiver background noise | niche | S | v1.x (with STI) | |
| C13 | Extinction limit | niche | S | v1.x (with C12) | |
| C15 | Fitting scattering on/off | niche | S | v1.x | |
| C19 | Random seed | niche | S | v1.x | |
| C39 | Run time in the Runs row | niche | S | v1.x | |
| G13 | Export .cbin or .poly | niche | S | Later | |
| M8 | Side of material effect | niche | S | Later | |
| M25 | Source time delay | niche | S | Later | |
| C16 | Direct field only | niche | S | Later | |
| C28 | User-defined air absorption | niche | S | Later | |
| C29 | Sound-speed gradient, ground roughness | niche | S | Later | |
| R34 | Total room energy decay | niche | S | Later | |
| R66 | Particle paths to CSV | niche | S | Later | |
| M23 | Balloon directivity sources | niche | M | Later | Also fix import-proj's refusal. |

### 3c. On the approved design, but in no deliverable or gate (15)

These will exist only if M10 to M12 build the screens as drawn. The simplest fix is to name them in the M10, M11 and M12 gates.

| # | Feature | Imp. | Size | When | Design receipt |
|---|---|---|---|---|---|
| M20 | Source power and spectrum | core | S | Before v1 | design:368-369 |
| C7 | Particles per source | core | S | Before v1 | design:668 (define what "auto" means) |
| C10 | Simulation length | core | S | Before v1 | design:669 |
| C11 | Time step | core | S | Before v1 | design:670 |
| C25 | Band selection | core | S | Before v1 | design:671 shows it read-only; it needs a picker |
| C27 | Air conditions | core | S | Before v1 | design:672 |
| R41 | Band choice for maps | core | S | Before v1 | design:445 |
| R50 | Map legend | core | S | Before v1 | design:164-168, :743 |
| A8 | Unsaved-changes marker | core | S | Before v1 | design:37-40 |
| G7 | Replace model, keeping groups | common | S | Before v1 | design:306 |
| G22 | Area and volume | common | S | Before v1 | design:297-305, :476 |
| M6 | Transmission per band | common | S | Before v1 | design:346 |
| M22 | Directivity choice | common | S | Before v1 | design:370 |
| R65 | All receivers in one table | common | S | Before v1 | design Results list |
| R68 | Report export | common | M | v1.x | design:466. Hide the button until it works |

### 3d. Partial gaps inside planned features

These features are Planned, but part of upstream's version is missing:

- **G43:** outside and no-face display modes.
- **G44:** the all/contour/none switch (deferred).
- **G47:** multi-select, drag-select, and tree/3D selection sync.
- **G48:** picking plane corners, orientation points and zone corners.
- **G37:** highlighting the faces TetGen rejects.
- **M11:** no band grids for source spectra or fitting zones.
- **M30:** per-item colour, name toggle and direction arrows.
- **M32:** the orientation vector field.
- **C32:** no refusal when every band is off.
- **C36:** elapsed and remaining time.
- **R1:** label, delete and open-folder on runs.
- **R52:** comparing any two runs and saving the result.
- **R57:** a viewer for the other tables (statistics, total energy).
- **A5:** "Save as" is not named in a gate.
- **A24:** the updater is conditional.

---

## 4. When each gap belongs

**Rules used.** Most early items are small, so size alone would not order them. Each gap was placed by how early a user hits it:

- **Before v1:** a user who follows upstream's tutorials, or brings an ordinary project, hits it in the first session and is stuck. Items that risk silent loss of work, and planned screens that would otherwise be empty, also go here.
- **v1.x:** common, but there is a workaround or it is off the first-session path.
- **Later:** niche, expert, very large, or waiting on a decision. "Later or never" marks items with no user value that the evidence can show.

**Totals.** Before v1: **43** items (15 absent or deferred, 14 core-only, 14 design-only), 38 of them S and 5 M. v1.x: **85**. Later: **55**.

**The before-v1 bill, grouped:**

1. **Project safety:**
   - A9: save prompt.
   - A8: unsaved marker.
   - A39: shortcuts.
   - A23: About dialog.
   - A42: file association.
2. **Getting a model in and organised:**
   - A3: open .proj in the GUI.
   - G8: repair at import.
   - G11: box room.
   - G18 and A29: add, rename and delete.
   - G19: send faces to a new group.
   - G7 and G22: replace model; area and volume.
   - G42: reset camera.
   - M37: import side only; stop refusing grouped receivers.
   - A43: sample projects.
3. **Materials and sources:**
   - M1: reference library.
   - M5: reflection-law column.
   - M6: transmission.
   - M20: source power and spectrum.
   - M22: directivity.
   - M26: source on/off.
4. **The Simulate settings:**
   - C7, C10, C11: particles, length, time step.
   - C25 and C26: band picker and presets.
   - C27: air.
   - C8: particles saved.
   - C12: method.
   - C21: per-band maps.
   - C22: echogram per source default.
5. **Maps you can set up:**
   - M40: surface receivers.
   - M41 and M42: cutting planes, drawn before the run.
   - R41: map band choice.
   - R50: legend.
6. **Numbers you can use:**
   - R59 and R58: CSV and TSV.
   - R62: chart zoom.
   - R65: all receivers in one table.
   - R17 and R18: decide C80, C50 and D50.
   - The four risks in section 1: tutorial-1 T30 and EDT, M12(b) hiding, particles-saved default, multi-source default. These are fixes to planned items, not new features, but a v1 without them fails the same test.

**Things a user who knows upstream would say "it can't even do X" about, in the order they would hit them:**

1. "It can't even open my .proj." (A3: CLI only.)
2. "It can't even make a shoebox." (G11)
3. "It can't even fix my model's flipped faces." (G8)
4. "It can't even split these faces into their own group." (G19)
5. "It can't even give me a 30 % absorbing material to pick." (M1)
6. "It can't even switch a source off." (M26)
7. "It can't even let me choose the frequency bands." (C25, C26)
8. "It can't even set Random or Energetic, the receiver radius, or transmission." (C12, C18, C17)
9. "It can't even add a cutting plane, so my new model gets no map." (M40, M41)
10. "It can't even play the particles unless I edit the file." (C8)
11. "It can't even give a T30 for tutorial 1." (R14 risk)
12. "It can't even show C80 or D50." (R17, R18)
13. "It can't even give decay parameters with two sources." (C22 default)
14. "It can't even save the table for Excel." (R59, R58)
15. "It can't even ask before throwing away my changes." (A9)

**Open decisions this audit runs into.** None of these has an answer in the plan, and each changes a row above:

- import formats and 3DS: raw:351 and design:307 disagree
- VolumetricMeshRepair: raw:360, "proposal: no" with no reason
- the preprocess.exe default: m56:244-245
- which parameters ship in the v1 GUI: raw:348, together with the 00:20 M8 gating
- embedded Python and plugins: no decision anywhere in the rebuild plan
- the updater: raw:357
- Mac and Linux timing: raw:359
- project archive format: raw:352

---

## 5. What not to copy

These are only the items where the plan or the upstream evidence gives a reason.

| What upstream has | Why not | Receipt |
|---|---|---|
| TLM solver entry (C3) | No TLM solver exists upstream; nothing to wrap | CMakeLists.txt:784 installs spps, vmr, classicalTheory, tetgen and preprocess only; e_core_tlmcore.h is not in the tree_core sources (CMakeLists.txt:248-254) |
| TCR per-band map switch (C24) | Does nothing: TCR tests whether the attribute is present, not its value | model.rs:984-985; docs/formats/config_xml.md (verified P2) |
| Receiver directivity list (M36) | One option, and no solver reads it | e_scene_recepteursp_recepteur_proprietes.h:52-54; base_core_configuration.cpp:229-258 |
| "Average" row across bands (R25) | Not a standard quantity. ISO single numbers are 500 Hz and 1 kHz means, and those are not computed either, which is worth adding | docs/params.md:65-70 |
| Decay parameters on a summed multi-source echogram (R26) | ISO 3382-1 defines them per source-receiver pair. This depends on fixing C22's default, or multi-source projects get none | docs/results.md:469-473 |
| Opening results of upstream runs (R67) | Only verified runs are read; upstream never judged its solver exits | proj.rs:5-6; M7 "typed RunResults built from verified runs only" |
| Undo on/off preference (A11) | Exists only because upstream's snapshot undo is slow; op-based undo has no such cost | Docs/menu_edition.rst; ops.rs:1169-1244 |
| Docking and a floating 3D view (A28) | The fixed layout of the approved design. This is the weakest reason here ("dockview only if free docking is ever wanted"), and the cost is a second-monitor 3D view | raw surveys[3] recommended_stack[7]; docs/design/README.md:19-24 |
| Silent "Default" material on unassigned faces (M15) | Refused before launch instead of substituted | contract:62 |
| Zipping every run into the project on each save | Produced 2.5 GB .proj files and lost work. Offer an explicit "export with results" instead (A47) | docs/upstream-laydown.md:95-100 |
| Upstream bugs that a port should not carry | STI uses female weights always (`if(gen='F')`, calc.cpp:748). The .gap viewer swaps the two lateral labels (e_report_gabe_gap.cpp:80-81). Binary STL is read scrambled (stl.cpp:316-319). NFF export does nothing (Objet3D.cpp:489-525). job_tool breaks on Python 3.10+ (job_tool/__init__.py:102). The update check never runs (python_interface/pythonshell.cpp:140-141) | as cited |

The plan does not rule out Embedded Python (A14, A15), 3DS (G3), VolumetricMeshRepair (G9) or free-form TetGen flags (G35): they are open, not refused. The "deliberately not" line for Python in ui-parity-backlog.md:80 belongs to the old GUI and is not the rebuild's decision.
