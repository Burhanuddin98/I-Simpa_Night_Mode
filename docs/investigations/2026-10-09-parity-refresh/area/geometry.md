# Parity 2.1 Geometry, scene and 3D view: status at 95ec062

R = B:\repos\I-Simpa_Night_Mode\.claude\worktrees\parity-ro. Paths below are relative to R. `ui/` = app/ui/src, `tauri/` = app/src-tauri/src, `core/` = crates/simpa-core/src. Checked in code (read, not run). No row is Not porting: no decision refuses any of these; 3DS and remesh are "product decisions for Burhan" (docs/scope.md:70), still open.

## 1. Table

| ID | Feature | Imp. | 09-25 | Now | Receipt | Note |
|---|---|---|---|---|---|---|
| G1 | Import PLY, layers to groups | core | Planned | Ready | ui/chrome/ImportDialog.tsx:1-130; tauri/commands.rs:384-397; tauri/bridge.rs:469-505; core/geometry/import/ply.rs | File > Open and "Import model..." (GeometryPanel.tsx:29-42) open the dialog. Import replaces the project |
| G2 | Import STL | core | Planned | Ready | same dialog; core/geometry/import/stl.rs | |
| G3 | Import 3DS | common | Absent | Absent | docs/scope.md:70 (open product decision); no 3ds reader in core/geometry/import/ | Matrix: v1.x |
| G4 | Import TetGen .poly as scene | niche | Absent | Absent | dispatch is ply/obj/stl only (bridge.rs:469-505) | |
| G5 | Import upstream .bin scene | niche | Absent | Absent | read only inside a .proj (core/geometry/import/proj.rs) | |
| G6 | Import units | core | Planned | Ready | ui/chrome/ImportDialog.tsx:17-18, 98-99 (m cm mm ft in; z or y up) | Chosen by the user, never guessed. Design's "m (detected)" not built, by decision |
| G7 | Keep groups on re-import | common | Design only | Core only | crates/simpa/src/main.rs:20, 506-508 (`--keep-groups`); core/geometry/import/reassign.rs | App has no "Replace model, keep groups": model_import (commands.rs:384) takes path, unit, up only. Re-import in the GUI loses groups and materials |
| G8 | Repair model at import | core | Core only | Core only | crates/simpa/src/main.rs:99 (`simpa repair`); core/geometry/repair.rs | GUI import never repairs. A refused model loads with its faces highlighted and Run blocked (commands.rs:381-382; engine.ts:1689-1699). No Repair button |
| G9 | Average-model remesh (VMR) | common | Absent | Absent | docs/scope.md:70 (open decision) | |
| G10 | Surface meshing at import | niche | Absent | Absent | no import option | |
| G11 | New scene: box room | common | Absent | Absent | ui/chrome/Landing.tsx:51-82 (examples, "New project" = empty project via actions.ts:122); only fixture box core/schema/generate.rs | Matrix said "before v1". Not built. Landing offers shipped examples instead (tauri/examples.rs) |
| G12 | Export scene as PLY / .mat.ply | niche | Absent | Absent | tauri/export.rs and ui/features/export/ do PNG, CSV, JSON only | |
| G13 | Export .cbin / .poly | niche | Core only | Core only | core/formats/cbin.rs, poly.rs; written into run and mesh folders | No menu item |
| G14 | Export .mesh, .nff, .bin, .asc | niche | Absent | Absent | none | |
| G15 | Save as upstream .proj | niche | Absent | Absent | project is .simpa; .proj is read-only (core/geometry/import/proj.rs) | |
| G16 | Scene tree | core | Planned | Partial | ui/chrome/ScenePanel.tsx:1-30 (filter; Surfaces, Sources, Receivers, surface receivers; issue labels; rename, merge, delete) | Missing vs upstream: volumes, fitting zones, environment/display nodes. Cutting planes live in the Sources step (PlanesSection.tsx), not the list |
| G17 | Surface groups from the file | core | Planned | Ready | core/geometry/import.rs; ScenePanel.tsx (per-group face count `data-group-faces`) | Ungrouped file gets a note (GeometryPanel.tsx:152-160) |
| G18 | Add a group; refuse delete of non-empty | core | Core only | Partial | core/schema/ops.rs:128-135 (AddSurfaceGroup, RemoveSurfaceGroup); ui/ops.ts has no builder for either | A group is made only from picked faces (G19). No "add empty group" and no "delete group" in the GUI. Merge (G20) removes a group |
| G19 | Faces to a new group | core | Deferred | Ready | ui/actions.ts:310-326; ui/features/viewport/Viewport.tsx:159-195 (right-click menu); ui/chrome/MenuBar.tsx Edit > New group from selection; tauri/commands.rs:455-461 (edit_regroup); core/schema/ops.rs:153, 756-800 | Refused when it would split a surface receiver or fitting zone |
| G20 | Move faces to a group; merge groups | common | Absent | Ready | ui/actions.ts:327-343 (moveSelectionToGroup), 345-362 (mergeSelectedGroups); Viewport.tsx:190-200 "Move to group"; MenuBar Edit > Merge groups; core/schema/ops.rs:167 (MoveFaces) | Menu and Ctrl+click, not drag-and-drop. Also Rename group (F2) |
| G21 | Face list per group; tree and 3D in sync | niche | Absent | Partial | ui/features/materials/MaterialsPanel.tsx:48-93 (picked faces resolve to their groups); ScenePanel group click | No expandable face list per group. Group-click highlight in 3D not run (selectedFaces() in engine.ts not read in full) |
| G22 | Model and project statistics | common | Design only | Ready | ui/chrome/GeometryPanel.tsx:107-148 (dimensions, air volume, surface area, face and group counts); StatusBar.tsx; MaterialsPanel.tsx:65,93 (area of picked groups) | Volume is the air's (backlog 85) |
| G23 | Invert face normals by hand | niche | Absent | Absent | no op; repair orients (CLI only) | |
| G24 | Internal partitions, coupled rooms | core | Ready | Ready | core/geometry/check.rs (partitions classified, not refused) | Unchanged |
| G25 | Volumes | niche | Absent | Absent | refused on .proj import (core/geometry/import/proj.rs) | |
| G26 | Volume auto-detect | niche | Absent | Absent | none | |
| G27 | Convert a volume to a fitting zone | niche | Absent | Absent | none | |
| G28 | Fitting zones reach the solver | common | Core only | Core only | core/schema/model.rs:672 (FittingZone); import/proj.rs | From a .proj only. No display, list or edit in the GUI (grep "fitting" in ui/chrome and viewport: none). Silent to the user |
| G29 | Create and edit a box fitting zone | common | Absent | Core only | core/schema/ops.rs:245 (AddFittingZone) | Op exists; no CLI verb, no screen. Matrix called it Absent; same in effect for a user |
| G30 | Scene-fitted zone from selected faces | niche | Absent | Absent | FittingShape::Surfaces takes whole groups (model.rs) | G19 now exists, so unblocked, not built |
| G31 | Fitting-zone display | niche | Absent | Absent | none | |
| G32 | Mesh on demand | common | Core only | Core only | crates/simpa/src/main.rs:101-102 (`mesh`, `mesh-verify`) | tauri `scene_mesh` (commands.rs:477) is the display mesh, not TetGen. In the app the mesh is made inside Run |
| G33 | Mesh quality settings | niche | Core only | Partial | ui/features/simulate/SettingsEditor.tsx:367, 475 ("Preserve walls when meshing (-Y)"); settings.ts:205 | Radius/edge ratio and max tet volume not in the GUI (core/schema/model.rs:1039-1062) |
| G34 | Surface-receiver area constraint | common | Core only | Core only | core/schema/model.rs:1046-1050 | PlanesSection.tsx edits plane resolution only |
| G35 | Free-form TetGen parameters | niche | Absent | Absent | refused on import | |
| G36 | Scene correction (preprocess.exe) | common | Core only | Core only | core/schema/model.rs:1061 (`preprocess`); core/mesh/preprocess.rs | No toggle in the GUI |
| G37 | Test mesh topology | common | Planned | Partial | core check `highlight_faces` (ipc.ts:3001) drawn by ui/features/viewport/engine.ts:1689-1699; GeometryPanel check rows | Check faces are highlighted. TetGen-diagnosed faces from a failed mesh not highlighted in the view (dock shows reasons only; not traced in full) |
| G38 | Mesh settings per solver | niche | Absent | Absent | one MeshSettings per project | |
| G39 | Mesh preview with slice | common | Deferred | Deferred | docs/ui-parity-backlog.md:62 only; no row in docs/v1.1-backlog.md | .mbin reader exists; nothing draws tets. Upstream "Hide meshing" toggle goes with it (section 3) |
| G40 | Orbit, zoom, pan | core | Planned | Ready | ui/features/viewport/engine.ts:75, 875-878 (OrbitControls, screen-space pan); Viewport.tsx:78 toolbar Orbit tool | Wheel zoom works (upstream discards it) |
| G41 | First-person camera | niche | Absent | Absent | none | |
| G42 | Reset camera | common | Absent | Ready | ui/App.tsx:90-95 (Home); ui/chrome/MenuBar.tsx:156 View > Frame model; engine.ts:2471 `frameModel`; toolbar Frame tool (Viewport.tsx:68) | Frames the model rather than a stored default camera |
| G43 | Face display inside / outside / none | core | Planned | Partial | ui/features/viewport/ViewStyleMenu.tsx:12-17 (Colour, Grey, See-through with near-wall opacity, Wireframe) | No "none". Outside view is by See-through; no outside-only mode |
| G44 | Lines all / contour / none | common | Planned | Partial | ViewStyleMenu.tsx:18-21 (Every triangle, Features only at 20 degrees) | No "none" |
| G45 | Colour by material or CAD colours | common | Deferred | Partial | ViewStyleMenu.tsx:12 "Colour" = material colour; "Grey" | Material colour is Ready. Original CAD colours absent (importers keep none) |
| G46 | Axis arrows; XY/XZ/YZ grids | common | Deferred | Partial | axis gizmo live: engine.ts:1439-1445, Viewport.tsx:100; floor grid: ViewStyleMenu.tsx:100-110, features/viewport/ground.ts | Horizontal floor grid only (XY), on/off. No XZ or YZ grid, no scale or colour. v1.1-backlog row 11 ("No floor grid") is stale: the grid is drawn |
| G47 | Face selection | core | Planned | Partial | engine.ts:2205-2224 (click one face; double-click takes the coplanar flat surface); floodfill.ts; pick.ts | No Ctrl multi-select, no drag-select; an empty click clears. A source/receiver marker click selects the marker |
| G48 | Pick a position on the model | core | Planned | Partial | engine.ts:2226-2250; Viewport.tsx:79-80 (Place receiver, Place source); ui/actions.ts:400-410 (placeAt) | Floors only (surfaces facing up), at fixed heights. Sources and receivers; plane corners, orientation points and zone corners cannot be picked. Numeric entry also exists (SourcesPanel.tsx) |

## 2. Counts

48 rows. Ready 10, Partial 11, Core only 8, Deferred 1, Absent 18, Not porting 0, Unverified 0 (G21 and G37 carry a stated sub-claim not fully traced).

Importance x status now:

| Importance | n | Ready | Partial | Core only | Deferred | Absent |
|---|---|---|---|---|---|---|
| core | 13 | 7 (G1 G2 G6 G17 G19 G24 G40) | 5 (G16 G18 G43 G47 G48) | 1 (G8) | 0 | 0 |
| common | 17 | 3 (G20 G22 G42) | 4 (G37 G44 G45 G46) | 6 (G7 G28 G29 G32 G34 G36) | 1 (G39) | 3 (G3 G9 G11) |
| niche | 18 | 0 | 2 (G21 G33) | 1 (G13) | 0 | 15 |

Core rows not Ready: G8 G16 G18 G43 G47 G48.
Moved since 09-25: G19 Deferred to Ready; G20 Absent to Ready; G22 Design only to Ready; G42 Absent to Ready; G1 G2 G6 G17 G40 Planned to Ready. G11 (matrix: "before v1") and G7 are still open.

## 3. Missed by the matrix (upstream user-facing, src/isimpa)

1. Hide meshing: View menu check item that toggles the drawn mesh (main/i_simpa_main.cpp:260; handler :768). Common; tied to G39.
2. Source and receiver display properties: colour and "Show name" per element (data_manager/tree_scene/e_scene_sources_source_rendu.h:46-48; e_scene_recepteursp_recepteur_rendu.h:46-48). Common. The app labels markers but has no colour or label toggle.
3. Axis and grid appearance: axis arrow colours, length, width, "Show XYZ axis", grid colour and grid scale (tree_scene/e_scene_projet_rendu_origine.h:46-66). Niche; G46 mentions arrows and grids but not these fields.
4. Copy 3D view to the clipboard (main/i_simpa_main.cpp:289, handler :88). Niche; the app has Export view as PNG file only (MenuBar.tsx).
5. Dock or float the 3D view window, and "Reinitialize interface" (window layout reset) (main/i_simpa_main.cpp:235, :287-288). Niche; the app has a fixed layout with fold buttons.
6. Cutting-plane display options: "Show the grid", "Show vertices name" (tree_scene/e_scene_recepteurss_recepteurcoupe_rendu.h:57-58). Niche; app draws plane outline and grid without toggles (engine.ts:553-556).
7. Volume "Domain colour" display (tree_scene/e_scene_volumes_volume_rendu.h:53). Niche; goes with G25.

## 4. In the app, not in upstream

- Orthographic Plan view with a plan inset (View menu; engine.ts:1962 setView). The matrix said upstream has none.
- See-through walls with opacity, distance fade, darker corners, ground shadow, on-model dimensions (ViewStyleMenu.tsx).
- WebGPU renderer with WebGL2 fallback (engine.ts:855-905); present mode and turntable (Viewport.tsx).
- Validator FAIL/WARN labels on scene rows and on model faces; Run blocked until the check passes (ScenePanel.tsx, GeometryPanel.tsx).
- Unlimited exact undo/redo of every edit; atomic save; save prompt before New/Open.
- Group rename (F2), merge, Move to group, with per-edit undo (G19, G20).
- Explicit unit and up axis at import; binary STL read correctly; OBJ import; .proj opened with its own units.
- Results drawn in the 3D view (surface maps, particle playback, live view, rays) and Export view as PNG with the map legend.
- Run-size forecast and run-quality advisor with Apply, scene filter, shipped example projects on the landing page.
