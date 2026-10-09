# Parity 2.5 Project, app and workflow: status now (R = worktree parity-ro @ 95ec062)

Paths: ui/ = R/app/ui/src, tauri/ = R/app/src-tauri/src. Code read, not docs. Two claims in the brief do NOT hold in the code:
- No command palette: the "Commands / Ctrl K" button is disabled (ui/chrome/MenuBar.tsx:222-225; v1.1-backlog row 8). Model, Results and Help menus are also disabled stubs ("not built yet", MenuBar.tsx:166-171).
- No recent files anywhere (grep "recent" over app/ and crates/simpa: only an unrelated particles test).

## (1) Table
Imp = importance (core/common/niche).

| ID | Feature | Imp | 09-25 | Now | Receipt | Note |
|---|---|---|---|---|---|---|
| A1 | New project | core | Ready | Ready | MenuBar.tsx:104-110; actions.ts:122; commands::scene_new | Save prompt now applies (A9); disabled during a run |
| A2 | Open project | core | Ready | Ready | MenuBar.tsx:111-118; actions.ts:194 openDialog, :184 openPath; commands::scene_open | Dialog takes .simpa, .proj and PLY/OBJ/STL |
| A3 | Open upstream .proj | core | Core only | Ready | actions.ts:189,811 importProj; commands::proj_import; tauri/main.rs generate_handler | Opens as unsaved project, no unit dialog. Importer refusals unchanged (volumes, balloons, ...) |
| A4 | Save | core | Planned | Ready | MenuBar.tsx:120; App.tsx:118; actions.ts:227; commands::project_save | Ctrl+S, atomic |
| A5 | Save as | core | Planned | Ready | MenuBar.tsx:121; App.tsx:117; actions.ts:210 | Ctrl+Shift+S |
| A6 | Save a copy | common | Deferred | Deferred | no UI; docs/ui-parity-backlog.md:62 | Not in v1.1-backlog.md; only the old-GUI backlog line |
| A7 | Recent projects | common | Absent | Absent | none in app/ or crates | Landing lists 6 bundled examples only (chrome/Landing.tsx:41-55), not recents |
| A8 | Unsaved-changes marker | core | Design only | Ready | MenuBar.tsx:228-230 dirty-dot; tauri/bridge.rs:56,316 (serial-based dirty) | |
| A9 | Save prompt before New/Open/Exit | core | Absent | Ready | chrome/SavePrompt.tsx; actions.ts:472 confirmDiscard, :841 onCloseRequested; tauri/main.rs:172 on_close_requested | Close cancels an active run (3 s wait); hung-UI fallback 5 s |
| A10 | Undo/redo | core | Planned | Ready | MenuBar.tsx:133-134; App.tsx:114-116; commands::project_undo/redo, edit_undo/redo | Ctrl+Z, Ctrl+Y, Ctrl+Shift+Z |
| A11 | Undo on/off pref | niche | Not porting | Not porting | op-based undo, core ops.rs | unchanged |
| A12 | Preferences | common | Deferred | Deferred | no dialog; ui-parity-backlog.md:62; per-viewer localStorage only: dock height (features/dock/Dock.tsx:51), panel folds (chrome/fold.tsx:11), view style (viewport/engine.ts:160-200) | Upstream prefs: colours, render rate, legend font (projet.cpp:2341-2461); theme fixed by design |
| A13 | Language / translations | common | Deferred | Deferred | strings hard-coded English; ui-parity-backlog.md:62 | |
| A14 | Embedded Python console | niche | Absent | Deferred | v1.1-backlog.md V2-5 (:145); scope.md:71 "embedded Python" not in v1; decision 21 | Decided to v2 since 09-25 |
| A15 | Python plugin API/hooks | niche | Absent | Deferred | as A14; V2-5, V2-7 (contrib/plugin mechanism) | |
| A16 | Formulas in number fields | niche | Absent | Absent | no expression parser in ui/numbers.ts | |
| A17 | Headless/scripted operation | common | Ready | Ready | crates/simpa/src/main.rs:12-60 (import, import-proj, check, repair, mesh, run, results, advise, reband, aural) | Upstream has none; beyond parity |
| A18 | libsimpa Python module | niche | Absent | Deferred | V2-5; `simpa dump`, `results --json` partial substitute | |
| A19 | Help > Website | niche | Absent | Absent | Help menu disabled stub MenuBar.tsx:166-171 | |
| A20 | Help > Online docs | common | Absent | Absent | as A19 | |
| A21 | Help > Offline PDF | common | Absent | Absent | as A19 | |
| A22 | User manual | common | Absent | Absent | docs/ is developer material | V2-13 (teaching mode, tutorials) is v2 |
| A23 | About dialog | common | Absent | Absent | no component; only status-bar string "Solvers: I-Simpa 1.4.0 ..." chrome/StatusBar.tsx:57; V2-12 licence page is v2 | |
| A24 | Update check | common | Planned | Absent | tauri.conf.json bundle.active=false, no updater plugin (main.rs registers only dialog); scope.md:71 lists "the updater" as an open product decision | Not decided, nothing built |
| A25 | Console log with levels/times | core | Ready | Ready | features/dock/ConsolePane.tsx (Dock Console tab) | Moved from Panels.tsx into the dock |
| A26 | Export console to file | niche | Absent | Absent | no export/copy in ConsolePane.tsx | |
| A27 | Clear console | niche | Absent | Absent | none in ConsolePane.tsx | |
| A28 | Dockable panels, floating 3D | niche | Not porting | Not porting | docs/design/README.md:19-24; dock is drag-resizable only (Dock.tsx, decision 74) | Panels float over a full-window 3D view (decision 50), not dockable |
| A29 | Rename and delete any element | core | Core only | Partial | Sources/receivers: F2 rename, Del remove (chrome/SourcesPanel.tsx:7,167-200, sceneUi.ts removeSelected); groups: F2 rename, merge (MenuBar.tsx:146-169, ScenePanel.tsx:136-154) | Missing: delete a surface group; rename/delete of materials unverified (not found in MaterialsPanel) |
| A30 | Multi-select and drag in tree | niche | Absent | Absent | Ctrl+click multi-pick only for groups (merge); no drag | |
| A31 | Project name and description | common | Core only | Core only | ops.rs:109-114 SetProjectName/SetDescription; tab shows name (MenuBar.tsx:226); no ui/ caller | No field to edit either |
| A32 | Project author and date | niche | Absent | Absent | no fields in model | |
| A33 | Reopen last project at startup | common | Absent | Absent | tauri/main.rs opens only `--project`; landing card otherwise | |
| A34 | Crash and session recovery | common | Absent | Absent | no autosave; only the close guard | |
| A35 | Several instances | niche | Ready | UNVERIFIED | no single-instance plugin so it should work; runs per project folder | Not run, not tested |
| A36 | Data folder choice | niche | Absent | Absent | runs root = `<project folder>/runs` (decision 24); CLI `--runs <root>` only | |
| A37 | Open from command line | common | Ready | Ready | tauri/main.rs:40-69 (`--project`) | Only the flag; bare path argument unverified |
| A38 | Drop a file on window | common | Absent | Absent | tauri.conf.json dragDropEnabled true, no handler in app/ui/src | |
| A39 | Keyboard shortcuts | common | Absent | Partial | App.tsx:81-128: Ctrl+O/S/Shift+S/Z/Y/Shift+Z, F5, Home; F2 rename, Del | Missing: Ctrl+N, Ctrl+C/V (copy/paste elements), Ctrl+K (palette not built) |
| A40 | Windows installer | core | Planned | Absent | tauri.conf.json: "bundle":{"active":false}; only tools/devtools/package-zeph.ps1 (portable folder app.exe + solvers\) | No NSIS build exists |
| A41 | macOS/Linux packages | common | Absent | Deferred | V2-10 (v1.1-backlog.md) | |
| A42 | File association | common | Absent | Absent | no bundle config | |
| A43 | Bundled tutorial projects | common | Absent | Partial | tauri/examples.rs:38-66 (Elmia hall, Industrial hall, BRAS CR1-4); landing cards; commands::example_open | Six shipped examples as fresh copies. Not upstream's 3 tutorials, no tutorial text; V2-13 |
| A44 | Bundled validation projects | niche | Absent | Absent | none | BRAS rooms are examples, not the air-absorption/clarity set |
| A45 | Scripting samples | niche | Absent | Deferred | V2-5 | |
| A46 | Project format version/provenance | core | Ready | Ready | model.rs:18; json.rs:104-116; run manifest | v1-to-v2 migration code unverified |
| A47 | Project archive with runs | common | Absent | Absent | scope.md:71 open decision; runs in separate folders | |

## (2) Counts
ROWS 47. Now: READY 12 | PARTIAL 3 (A29, A39, A43) | CORE-ONLY 1 (A31) | DEFERRED 8 (A6, A12, A13, A14, A15, A18, A41, A45) | ABSENT 20 | NOT-PORTING 2 (A11, A28) | UNVERIFIED 1 (A35).
Absent: A7 A16 A19 A20 A21 A22 A23 A24 A26 A27 A30 A32 A33 A34 A36 A38 A40 A42 A44 A47.

| Importance | Ready | Partial | Core-only | Deferred | Absent | Not porting | Unverified | Total |
|---|---|---|---|---|---|---|---|---|
| core | 10 | 1 (A29) | 0 | 0 | 1 (A40) | 0 | 0 | 12 |
| common | 2 (A17, A37) | 2 (A39, A43) | 1 (A31) | 4 (A6, A12, A13, A41) | 11 | 0 | 0 | 20 |
| niche | 0 | 0 | 0 | 4 (A14, A15, A18, A45) | 8 | 2 | 1 | 15 |

Movement: M10-M12 resolved A3, A4, A5, A8, A9, A10 to Ready. A24 and A40 (both "Planned") were NOT delivered. Python (A14, A15, A18, A45) moved Absent to Deferred (v2). Core gaps: A29 (partial), A40 (no installer).

## (3) Features the matrix missed (upstream src/isimpa/)
1. Pause/resume a running simulation: main/i_simpa_main.cpp:62 (EVT_MENU), :219 (menu), :418 (toolbar), :817-820. Matrix has Cancel (C37) and animation pause (R55) only. Current app: Cancel only.
2. Copy 3D view to clipboard: i_simpa_main.cpp:88, :289 (CopyGlToFile). Current app: Export view as PNG file (MenuBar.tsx:122-128), no clipboard.
3. Hide-meshing display toggle: i_simpa_main.cpp:98, :260, :768 (ID_ShowHideMailler). Current app: unverified whether any mesh view exists.
4. Reinitialize interface (reset panel layout): i_simpa_main.cpp:58, :235. Tied to A28; current app has no reset command (dock height persisted only).
5. Preferences contents folded into A12 (3D colours for lines/selection/background/default model, render rate Hz, particle and surface-receiver display, label colour, legend font/colours/transparent background, iso-line colour): data_manager/projet.cpp:2341-2461.
6. SystemScript helper libraries `graphy` (bar/line/pie charts for scripts) and `uilocale` (gettext for script UIs): resources/SystemScript/graphy/, uilocale/__init__.py:8-20. Part of A14/A15.
Checked and already covered: change application data folder = A36; Compare surface receivers = R52; Previous/Next/Delete simulation (main.cpp:64-66) = R55, R3; New/Import/Export scene = G11-G13; Console export/clear = A26/A27; per-element script menus (job_tool, source_tools, recp_tool, recp_res_tool, moveto_vertex, preceiv_sourceTracker) = C40, M28, M29, M38, G20, M34.

## (4) Current app has, upstream lacks
- Full headless CLI (A17): validate, advise (run-quality advisor), reband, repair, bed, aural (crates/simpa/src/main.rs).
- Exact unlimited op-based undo; atomic save; close request that cancels a run and shows one save prompt (tauri/main.rs:172).
- Landing page with six bundled examples opened as fresh copies, never overwriting (examples.rs:105-132).
- Run guard: New/Open disabled with the reason during a run (MenuBar.tsx:52).
- Edit > New group from selection, Merge groups; Export view PNG, parameters CSV/JSON (features/export); Listen (auralization) window with WAV save.
- Plan view toggle, Frame model (Home), variant switch, run-size/blocker forecast on Run, per-run provenance and solver-build verification.
- Persisted per-viewer panel folds, dock height and view style (localStorage).
