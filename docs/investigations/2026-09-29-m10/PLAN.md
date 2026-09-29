# M10 plan: Concept B, Geometry, Materials, Sources & receivers

2026-09-29, branch `m10` at `6a2b411`. **Planning only. Nothing is built yet.** The plan covers:
- the IPC contract;
- the UI state and component tree;
- how each gate check is met;
- the e2e harness and the fixtures;
- one sequential foundation and three parallel packages;
- the parity items M10 must hold.

The technical calls here are Jarvis's under decision row 13, and Burhan can overrule any of them.
Section 8 lists them.

## 0. The spec, verbatim (`rebuild-plan-raw-2026-09-23.json`, plan.milestones, M10)

> **Deliverable.** The Concept B chrome, following concept-b-approved.dc.html: a menu bar with the
> project as a tab, and a step bar with the variant switch; the scene list and the properties
> panel; a bottom dock with Acoustics, Console and Runs tabs; themed scrollbars.
> First three steps: import wired to the M4 check, feeding the Console and the view highlighting;
> a three.js viewport: inside view, outlines, BVH picking, iterative coplanar flood-fill, plan
> inset in one context; a materials grid with TSV paste and copy, row fill, natural band sort and
> inline validator messages; source and receiver placement with inline validator errors; save,
> open, and undo/redo through core operations.
>
> **Gate.** `pwsh tools/gates/m10.ps1`, which runs `npx wdio app/e2e/m10.conf.ts`, exits 0.
> (a) Open raw elmia.ply: the Console shows a FAIL line containing 'open' and '955'; the viewport
> test hook reports more than 0 highlighted faces; the Run control is disabled.
> (b) Open the corrected hall. The Console shows the INFO line 'Closed volume, 0
> self-intersections', and the step bar shows Materials '0 / 10'.
> (c) Paste tests/fixtures/ui/materials_6x6.tsv. The canonical JSON of the saved project equals
> materials_6x6.expected.json byte for byte.
> (d) Double-click the box ceiling. The selection is 2 faces, in group Ceiling.
> (e) Invalid placement: a receiver placed outside the room shows RECEIVER_OUTSIDE and leaves the
> project unchanged; the label 'a/b' is rejected with LABEL_UNSAFE.
> (f) 50 scripted edits followed by 50 undos leave the saved file byte-identical to the original.
> (g) document.querySelectorAll('canvas').length == 1.

Standing rules for this milestone:
- **No acoustic number computed by a solver is shown.** Only M12 may show one, after M8. Geometry
  facts such as volume, area and counts are allowed.
- **M9 must keep passing** (`tools/gates/m9.ps1`), and so must the core crates' tests.
- Rust builds use `CARGO_TARGET_DIR=C:/tmp/nm-target`. Nothing builds into a `target/` on B:.
- Anything generated on B: gets a file count.

## Receipts gathered for this plan (2026-09-29, 08:12-08:31)

| What | Result |
|---|---|
| `simpa check` (built into `C:/tmp/nm-target`) on upstream `tutorial 2/elmia.ply`, `--unit m --up z` | exit 3, **refused**. Reasons: `degenerate_faces` 1; `self_intersections` 1,395 pairs (896 faces); `open_boundary` ("953 open edges face the exterior, and 1085 faces have the exterior on both sides; 7 vertices are at the position of another vertex"); `no_enclosed_volume`. **Census `counts.open_edges` = 955**, `nonmanifold_edges` 9, 1,086 faces. |
| `simpa check testdata/elmia_corrected.ply` | exit 0, **ok**: 7,860 faces, 0 open edges, 0 self-intersections, one cell, area 4,001.809 m², volume 10,389.096 m³ |
| `simpa check tests/fixtures/rooms/tutorial1_box.simpa` | ok, 12 faces, area 216 m², volume 180 m³. Groups: Ceiling 2 faces, Floor 2, Walls 8 (four walls in one group) |
| `simpa import testdata/elmia_corrected.ply` | 3,926 vertices, 7,860 faces, **10 groups**, and **one material, `Default`** (α 0, `#ff0000`), octave 125 Hz-4 kHz bands. Import took about 40 ms |
| `simpa validate` on the hall imported from PLY | one issue, `source_none`. **The `Default` placeholder passes the validator** (finding F1) |
| `simpa validate tests/fixtures/rooms/elmia_corrected.simpa` | 0 issues, 47 ms including the load |
| WebView2 runtime (registry `pv`) | **154.0.4258.37**. The runtime folder also still holds 153.0.4234.48. M9's "msedgedriver 153" is stale |
| tauri-driver, msedgedriver, pwsh | none installed. Windows PowerShell is 5.1.26100; Node is v24.15.0 |
| Disk | C: 27.7 GB free; B: 93.9 GB free. `C:/tmp/nm-target` is 1.7 GB |
| Main checkout `app/node_modules` | 3,473 files, 520 MB on B:. The m10 worktree has none |

## Findings that shape the plan

- **F1. An imported model's groups all get `Default`, a real material with α 0, and the core
  validator accepts it.** Parity row M15 says the rebuild refuses unassigned faces. For imported
  meshes it does not. For M10:
  - the Materials counter counts `Default` as unassigned, which is how gate (b) reads '0 / 10';
  - the app adds a Run blocker, `MATERIALS_UNASSIGNED`.

  Whether the core refuses it is left for M11 and Burhan (section 8).
- **F2. Gate (b)'s '0 / 10' is reachable only by importing a mesh.** The `.simpa` hall
  (`tests/fixtures/rooms/elmia_corrected.simpa`) has all 10 groups assigned, so it would read
  '10 / 10'. Gate (b) therefore imports `testdata/elmia_corrected.ply`, which is verified closed
  with 10 layers. Its wrong grouping (CLAUDE.md) does not affect counts. When Michael's clean-up
  of `main` deletes `testdata/`, the file moves to `tests/fixtures/ui/`.
- **F3. The gate's codes are not the core's.** `RECEIVER_OUTSIDE` is `receiver_outside_volume`,
  and `LABEL_UNSAFE` is `name_not_filename_safe`. The core codes are stable API and stay as they
  are. The app maps them to UI codes with one exhaustive table (1.7). The UI shows both, the UI
  code and the rule.
- **F4. The raw hall's '955' is the census count** (`counts.open_edges`). The `open_boundary`
  reason itself says 953. The FAIL line prints both, each labelled (1.11), so the number is honest
  and the gate's '955' is present.
- **F5. `m9.ps1` cannot run against `C:/tmp/nm-target` today.**
  - `Join-Path $repo $TargetDir` breaks on an absolute path.
  - Its work folder is `<repo>\target\gates\m9` on B:.

  The foundation fixes both, which leaves the gate's criteria unchanged (4.5).
- **F6. three-mesh-bvh rearranges the geometry's index buffer unless the BVH is built with
  `indirect: true`.** Picked `faceIndex` must equal the project's face index, or gate (d) reads the
  wrong group.

---

## 1. The IPC contract

### 1.1 Principles, unchanged from M9

- Every command is an `async fn` whose body runs through `guard::blocking`. M9 check (g) and the
  guard check keep counting them.
- Every command is registered in three places: `generate_handler!`, `build.rs`'s ACL list, and
  an `allow-<command>` in `capabilities/default.json`. M9 (g) requires the attributes, the handler
  and `build.rs` to hold the same set. `m10.ps1` adds the capabilities file to that comparison,
  because M9 does not check it and a missing `allow-` fails only at run time.
- Schema values arrive as JSON text and are read with the core's exact reader (`Op::from_json`).
  No argument is typed as a schema struct.
- Datasets travel as raw `ipc::Response` bytes.
- No new plugin and no `fs:` or `shell:` permission. The core reads and writes by path.
- **M9's commands stay as they are.** The self-test uses `project_new`, `project_open`,
  `project_apply`, `project_undo` and `project_redo`. M10's UI calls only the M10 commands below;
  a lint in `m10.ps1` fails any `backend.projectApply` call outside `selftest.ts`.

### 1.2 New commands (9). All live in `commands.rs`, backed by a new `src/scene.rs` and an extended `bridge::Session`

Every body has the form `guard::blocking("<name>", move || lock(&session, "project")?.<method>(..)).await`.

| Command | Rust signature | Returns (TS) | What it does |
|---|---|---|---|
| `scene_state` | `pub async fn scene_state(state: State<'_, AppState>) -> CmdResult<Option<SceneState>>` | `SceneState \| null` | The current state. It drains any pending lines, including the check lines of a `--project` opened at startup |
| `scene_new` | `pub async fn scene_new(state: State<'_, AppState>, name: String) -> CmdResult<SceneState>` | `SceneState` | New empty project. History cleared |
| `scene_open` | `pub async fn scene_open(state: State<'_, AppState>, path: String) -> CmdResult<SceneState>` | `SceneState` | `schema::load`, then the M4 check and the validator. Check lines go to `lines` |
| `model_import` | `pub async fn model_import(state: State<'_, AppState>, path: String, unit: String, up: String) -> CmdResult<SceneState>` | `SceneState` | `geometry::import::import_file(path, ImportOptions::new(unit, up))`, then `to_project(file stem)` replaces the session. The import report becomes INFO lines, the M4 check becomes FAIL or INFO lines. **A refused geometry is loaded, not rejected:** `check.verdict = "refused"`, its faces are highlighted and Run is blocked |
| `project_save` | `pub async fn project_save(state: State<'_, AppState>, path: Option<String>) -> CmdResult<SceneState>` | `SceneState` | `schema::save(project, path)` (atomic). `None` saves to the session's path; `Some` is Save As, which sets the path. It **never** changes the project's name. Pushes an OK line: "Saved <path>" |
| `edit_apply` | `pub async fn edit_apply(state: State<'_, AppState>, op: String) -> CmdResult<EditOutcome>` | `EditOutcome` | The **checked apply** (1.8). A validator refusal is `Ok(applied: false)`, not an error |
| `edit_undo` | `pub async fn edit_undo(state: State<'_, AppState>) -> CmdResult<SceneState>` | `SceneState` | `History::undo`. Nothing to undo is `Ok` with the state unchanged |
| `edit_redo` | `pub async fn edit_redo(state: State<'_, AppState>) -> CmdResult<SceneState>` | `SceneState` | `History::redo`. Same rules as undo |
| `scene_mesh` | `pub async fn scene_mesh(state: State<'_, AppState>) -> CmdResult<Response>` | `ArrayBuffer` | The geometry as a binary buffer (1.9). The UI fetches it only when `info.geometry_rev` changes |

Other changes to existing code:
- **`StartupInfo`** gains `e2e: bool`.
- **`main.rs`** accepts a value-less `--e2e` flag, which installs the test hooks (2.5).
- **Risk to check at the first harness smoke run:** msedgedriver may append arguments when it
  launches the app. If it does, `parse_args` (which refuses unknown arguments) must ignore
  `--remote-debugging-*` style arguments, and only when `--e2e` is present.

### 1.3 Rust types (`src/scene.rs`, each `#[derive(Clone, Debug, Serialize, JsonSchema)]`, added to `bindings.rs`'s `ipc.json` dump)

```rust
pub struct SceneState {
    pub info: ProjectInfo,            // M9 type, extended (below)
    pub view: ProjectView,            // the project without its mesh
    pub groups: Vec<GroupStats>,      // one per surface group, in project order
    pub check: Option<CheckSummary>,  // None when the project has no faces
    pub issues: Vec<UiIssue>,         // validate_with(project, ctx) on the current project
    pub run_blockers: Vec<String>,    // UI codes; empty never happens in M10 (M11_PENDING)
    pub lines: Vec<LogLine>,          // Console lines produced since the last state was returned
}
pub struct ProjectView {
    pub name: String, pub description: String, pub bands: BandSet,
    pub surface_groups: Vec<SurfaceGroup>, pub materials: Vec<Material>,
    pub sources: Vec<Source>, pub point_receivers: Vec<PointReceiver>,
    pub surface_receivers: Vec<SurfaceReceiver>,
    pub variants: Vec<Variant>, pub active_variant: Option<VariantId>,
}
pub struct GroupStats { pub id: GroupId, pub faces: u32, pub area_m2: f64, pub assigned: bool }
pub struct CheckSummary {
    pub verdict: CheckVerdict,        // "ok" | "refused"
    pub reasons: Vec<CheckReasonOut>, // code, count, repairable, faces (count), message
    pub counts: CheckCounts,          // vertices, faces, edges, open_edges, nonmanifold_edges,
                                      // self_intersecting_pairs, self_intersecting_faces,
                                      // degenerate_faces, duplicate_faces, invalid_faces,
                                      // inverted_faces, exterior_faces, boundary_open_edges,
                                      // cells, components, coincident_vertices
    pub area_m2: f64, pub enclosed_volume_m3: f64,
    pub bbox_min: [f64; 3], pub extents_m: [f64; 3],
    pub highlight_faces: Vec<u32>,    // union of every reason's faces, ascending, unique
}
pub struct UiIssue {
    pub code: String,                 // UI code (1.7), e.g. RECEIVER_OUTSIDE
    pub rule: String,                 // core code, e.g. receiver_outside_volume
    pub severity: IssueSeverity,      // "error" | "warning"
    pub path: String,                 // the validator's JSON pointer
    pub entity: Option<EntityRef>,    // resolved from `path` in its own project
    pub field: String,                // path after the entity, e.g. "position", "absorption/3"
    pub message: String,
}
pub struct LogLine { pub class: LineClass, pub text: String }   // LineClass from events.rs
pub struct EditOutcome { pub applied: bool, pub refusals: Vec<UiIssue>, pub state: SceneState }
```

**`ProjectInfo`** (M9) gains these fields. It is additive, so the self-test is unaffected.

| Field | Meaning |
|---|---|
| `undo_depth: usize`, `redo_depth: usize` | The history stacks |
| `dirty: bool` | See 1.10 |
| `geometry_rev: u64` | Changes whenever the geometry can have changed |
| `groups_assigned: usize` | Groups whose effective material is not the placeholder (1.10) |

The bindings test's name list (`the_ipc_dump_holds_every_command_result`) gains every new type.

**The core types in `ProjectView`** (`Material`, `Source`, ...) are also emitted into `ipc.json`,
so `ipc.ts` re-declares them. The declarations are structurally identical to `schema.ts`, so
either import works. UI code imports the entity types from `schema.ts` and the IPC wrappers from
`ipc.ts`.

### 1.4 TypeScript, as the generator will emit it (`ui/src/bindings/ipc.ts`)

The generator output is authoritative. Field order is alphabetical, and an `Option` field becomes
`?: T | null`.

```ts
export interface SceneState { check?: CheckSummary | null; groups: GroupStats[]; info: ProjectInfo;
  issues: UiIssue[]; lines: LogLine[]; run_blockers: string[]; view: ProjectView; }
export interface EditOutcome { applied: boolean; refusals: UiIssue[]; state: SceneState; }
export interface UiIssue { code: string; entity?: EntityRef | null; field: string; message: string;
  path: string; rule: string; severity: IssueSeverity; }
export type IssueSeverity = 'error' | 'warning';
export interface LogLine { class: LineClass; text: string; }
export interface CheckSummary { area_m2: number; bbox_min: [number, number, number]; counts: CheckCounts;
  enclosed_volume_m3: number; extents_m: [number, number, number]; highlight_faces: number[];
  reasons: CheckReasonOut[]; verdict: CheckVerdict; }
export type CheckVerdict = 'ok' | 'refused';
export interface GroupStats { area_m2: number; assigned: boolean; faces: number; id: string; }
// ProjectInfo gains: dirty, geometry_rev, groups_assigned, redo_depth, undo_depth.
// StartupInfo gains: e2e.
```

Plain `f64` fields (areas, volumes) serialise a non-finite value as `null`. The UI shows `—` for
it. Entity numbers are `F64` (`number | string`): non-finite values arrive as strings.

### 1.5 `backend.ts` (foundation)

```ts
sceneState: () => invoke<SceneState | null>('scene_state'),
sceneNew:   (name: string) => invoke<SceneState>('scene_new', { name }),
sceneOpen:  (path: string) => invoke<SceneState>('scene_open', { path }),
modelImport:(path: string, unit: Unit, up: Up) => invoke<SceneState>('model_import', { path, unit, up }),
projectSave:(path: string | null) => invoke<SceneState>('project_save', { path }),
editApply:  (op: Op) => invoke<EditOutcome>('edit_apply', { op: opText(op) }),
editUndo:   () => invoke<SceneState>('edit_undo'),
editRedo:   () => invoke<SceneState>('edit_redo'),
sceneMesh:  () => invoke<ArrayBuffer>('scene_mesh'),
// type Unit = 'm' | 'cm' | 'mm' | 'ft' | 'in'; type Up = 'y' | 'z'
```

**`opText(op)` is not `JSON.stringify`.** Two JavaScript behaviours would corrupt a value:
- `JSON.stringify(-0)` is `"0"`, which loses the sign that the core's writer keeps as `-0.0`;
- `JSON.stringify(NaN)` is `null`.

`opText` writes −0 as `-0.0` and throws on a non-finite number. Its unit test pins both.

### 1.6 Error codes (`CmdError.code`)

| Command | Codes, besides the guard's `PANIC`, `STATE_POISONED` and `TASK_FAILED` |
|---|---|
| `scene_state`, `scene_new` | none |
| `scene_open` | `LOAD_NOT_FOUND`, `LOAD_IO`, `LOAD_UTF8`, `LOAD_SYNTAX`, `LOAD_MISSING_VERSION`, `LOAD_VERSION`, `LOAD_SCHEMA`, `LOAD_INTEGRITY` |
| `model_import` | `IMPORT_UNIT`, `IMPORT_UP` (option text not recognised); `IMPORT_` plus `ImportError::code` upper-cased: `IMPORT_IO`, `IMPORT_UNKNOWN_FORMAT`, `IMPORT_SYNTAX`, `IMPORT_TRUNCATED`, `IMPORT_INVALID`, `IMPORT_UNSUPPORTED`, `IMPORT_EMPTY`, `IMPORT_INTEGRITY` |
| `project_save` | `NO_PROJECT`, `SAVE_NO_PATH` (plain Save of a project never saved), `SAVE_IO` (the OS message in `message`) |
| `edit_apply` | `NO_PROJECT`; `LOAD_SYNTAX` and `LOAD_SCHEMA` (op text); `OP_NOT_FOUND`, `OP_DUPLICATE_ID`, `OP_INDEX`, `OP_BAND`, `OP_IN_USE`, `OP_NO_TRANSMISSION`, `OP_ACTIVE_VARIANT`, `OP_BAND_DATA`, `OP_INTEGRITY`, `OP_BATCH`. The project is unchanged on every error |
| `edit_undo`, `edit_redo` | `NO_PROJECT`, `OP_*`. An inverse that fails is a bug, and it surfaces rather than being swallowed |
| `scene_mesh` | `NO_PROJECT` |

### 1.7 UI codes (`scene.rs::ui_code`). The table is exhaustive over `validate::RULES` and `STRUCTURAL_CODES`, and a unit test fails if a core code is missing

| Core rule | UI code |
|---|---|
| `receiver_outside_volume` | **`RECEIVER_OUTSIDE`** |
| `receiver_on_surface` | `RECEIVER_ON_SURFACE` |
| `receiver_sphere_crosses_surface` (warning) | `RECEIVER_SPHERE_CROSSES` |
| `source_outside_volume` | `SOURCE_OUTSIDE` |
| `source_near_surface` | `SOURCE_NEAR_SURFACE` |
| `name_not_filename_safe` | **`LABEL_UNSAFE`** |
| `name_too_long` | `LABEL_TOO_LONG` |
| `name_duplicate` | `LABEL_DUPLICATE` |
| every other rule and structural code | the core code, upper-cased (`MATERIAL_VALUE_OUT_OF_RANGE`, `SOURCE_NONE`, ...) |

Codes that only the app produces:
- **Run blockers:** `GEOMETRY_REFUSED` (the check refused), `MATERIALS_UNASSIGNED` (F1), and
  `M11_PENDING` (always present in M10, because Run is wired in M11).
- **Produced in the UI, never by Rust:** `NOT_A_NUMBER`, `PASTE_SHAPE` and `PASTE_HEADER`.

### 1.8 The checked apply (`edit_apply`)

1. Read `op` with `Op::from_json`. Take `before`, the cached issues of the current project.
2. Clone the project and apply `op` to the clone. On `OpError`, return `Err(OP_*)`.
3. Run `after = validate_with(&clone, ctx)`, where `ctx = Context::for_project_file(path)`, or the
   default when the project has no path.
4. **Identity of an issue is `(rule, entity kind + id, field)`, resolved in its own project**, or
   `(rule, path)` when there is no entity. It is not the path alone: an insertion shifts list
   indices, and every existing issue below it would otherwise look new.
5. A **new error** is one in `after` whose identity is not in `before`. If there are any, return
   `Ok(EditOutcome { applied: false, refusals: new errors, state: current })` and push one FAIL
   line per refusal. **The project and the history are unchanged.**
6. Otherwise, `History::apply(project, op)` applies the same op to the real project. The issues
   cache becomes `after`, and each **new warning** becomes a WARN line and is applied.
   `applied: true`.

A pre-existing error, such as `source_none` on a fresh import, does not block edits. An edit that
fixes an issue is accepted. One rule covers every M10 edit: placement, rename, material values,
paste and assignment.

**Known limit.** The core validates only the materials in use. An α of 1.5 in an unused library
material is therefore accepted without a message. Assigning that material to a group is then
refused with `MATERIAL_VALUE_OUT_OF_RANGE`. This is listed, not patched: fixing it in the core is
out of M10's scope.

### 1.9 `scene_mesh` layout (little-endian)

| Offset | Type | Field |
|---|---|---|
| 0 | u32 | magic `0x4853454D` ("MESH") |
| 4 | u32 | version 1 |
| 8 | u32 | `nv` vertices |
| 12 | u32 | `nf` faces |
| 16 | u64 | `geometry_rev` |
| 24 | f64 × 3nv | positions, world metres, bit-exact from the project |
| 24 + 24nv | u32 × 3nf | vertex indices, in project face order |
| 24 + 24nv + 12nf | u32 × nf | group index (position in `view.surface_groups`) |

The positions start 8-aligned, so the UI reads them with `new Float64Array(buf, 24, 3*nv)` and
no copy. The UI checks the magic and the version. The hall is about 220 KB.

### 1.10 Session additions (`bridge.rs`)

- **Dirty flag.** A serial is pushed with every applied op, and undo and redo move serials between
  two stacks that mirror `History`'s. `dirty = path.is_none() || top_serial != saved_serial`. Save,
  open, new and import set `saved_serial`. A project that was imported or created and never saved
  is dirty, so the design's unsaved dot shows.
- **Geometry revision.** `geometry_rev` is bumped by replace (open, import, new) and by any
  applied, undone or redone op that holds a `set_geometry`, including inside a `batch`.
- **Caches.**
  - `check`: the `CheckSummary` for the current `geometry_rev`, computed once;
  - `issues`: the validator output for the current state, recomputed after each change (47 ms on
    the hall, measured).
- **Pending lines.** A queue of `LogLine`s drained into the next `SceneState`.
- **The placeholder rule, for `groups_assigned` and `GroupStats.assigned`.** A group is unassigned
  when all three hold for its effective material under the active variant:
  - its name is exactly `REFERENCE_MATERIALS[0].name` (`"Default"`);
  - every absorption value is 0;
  - every scattering value is 0.

  That is upstream's reference material 0, which upstream gives a face "that no surface group
  holds". Unit tests:
  - the hall imported from PLY gives 0 of 10;
  - `teaching_room.simpa` gives 6 of 6;
  - `tutorial1_box.simpa` gives 3 of 3.

### 1.11 Console wording from the check (Rust, `scene.rs::check_lines`, one place)

- **OK.** Exactly one line starts with the literal **`Closed volume, 0 self-intersections`**:
  `INFO Closed volume, 0 self-intersections · 7860 faces, 10 surface groups`. It is followed by the
  import report's INFO lines: polygons split, welded or unused vertices, dropped empty groups, and
  the notes.
- **Refused.** There is one FAIL line per reason, in `ReasonCode` order. Each is "Model check
  refused <file>: <code>: <summary>", followed by the core's message verbatim. The
  `open_boundary` line carries the census count as well:
  `FAIL Model check refused elmia.ply: open_boundary: 955 open edges in the census (953 on the
  outer shell after analysis), 1085 faces with the exterior on both sides. <core message> Run
  refused; faces highlighted in the view.`
- Unit tests:
  - the box gives the exact INFO prefix;
  - the box with one face removed gives a FAIL `open_boundary` line containing "open" and the
    census count.

  The raw hall is covered by the e2e.

---

## 2. UI state model and component tree

### 2.1 Stores (`ui/src/store.ts`, foundation)

These keep M9's islands rule: state lives outside React, and `useStore` subscribes to it.

| Store | Type | Written by | Notes |
|---|---|---|---|
| `sceneStore` | `SceneState \| null` | actions only | The backend's truth, replaced on every response |
| `meshStore` | `SceneMesh \| null` | actions only | Decoded `scene_mesh`, refetched when `info.geometry_rev` changes |
| `consoleStore` | `ConsoleLine[]` (M9) | actions, UI | `SceneState.lines` are appended with a clock |
| `statusStore` | M9 | actions | |
| `stepStore` | `StepKey` | scene (step bar), hooks | Moved out of `App`'s `useState` |
| `selectionStore` | `Selection` | viewport, scene list, materials | Values below |
| `toolStore` | `'select' \| 'orbit' \| 'place-receiver' \| 'place-source'` | viewport toolbar, scene ("Place in view") | |
| `refusalStore` | `Map<fieldKey, UiIssue[]>` | actions | Refusals by field; cleared by the next accepted edit of that field |
| `busyStore` | number of in-flight commands | backend wrapper | Feeds the `idle()` hook |

```ts
type Selection =
  | { kind: 'none' }
  | { kind: 'faces'; faces: number[]; groups: GroupId[] }  // viewport pick or flood-fill
  | { kind: 'group'; id: GroupId } | { kind: 'material'; id: MaterialId }
  | { kind: 'source'; id: SourceId } | { kind: 'receiver'; id: PointReceiverId };
// fieldKey = `${kind}:${id}:${field}`, e.g. `point_receiver:<uuid>:position`, `material:<uuid>:absorption/3`
```

### 2.2 Actions and op builders (foundation)

**`actions.ts`** holds the only code that calls `backend`:
- `newProject`;
- `openPath(path)`, which sends `.simpa` to `scene_open` and a mesh to the import dialog;
- `importModel(path, unit, up)`, `save()`, `saveAs(path?)`;
- `apply(op, fieldKey?)`, which returns `EditOutcome` and records refusals under `fieldKey`;
- `undo()`, `redo()`;
- `placeAt(kind, point)`, which builds `add_point_receiver` or `add_source` at the list's end with
  the next free name `R<n>` or `S<n>` and calls `apply`.

**`ops.ts`** has pure builders over the generated `Op` type:
- `rename`, `moveReceiver`, `moveSource`, `addReceiver`, `addSource`, `removeReceiver`,
  `removeSource`;
- `setMaterialBand`, `addMaterial`, `removeMaterial`;
- `assignMaterial(group, material)`: `set_group_material` on the base, or `set_variant_override`
  when a variant is active;
- `addVariant`, `setActiveVariant`, `batch`, `opText`.

New entity ids come from `crypto.randomUUID()` (the app runs on `http://tauri.localhost`, a secure
context).

**`numbers.ts`** has `parseStrictDecimal(text)`, which accepts only
`^[+-]?(\d+(\.\d*)?|\.\d+)([eE][+-]?\d+)?$`, so `0,5`, `NaN`, `Infinity` and the empty string are
refused as `NOT_A_NUMBER`. Both the grid and the position fields use it.

**`mesh.ts`** decodes the 1.9 layout. **`issues.ts`** gives issues by entity and by field.

### 2.3 Component tree against the Concept B regions

| Region (design line) | Component | File | Owner |
|---|---|---|---|
| whole window | `App`: composition, boot, the global key map | `ui/src/App.tsx` | foundation |
| menu bar, 36 px (design:28-44) | `MenuBar`. Menus File, Edit, View, then Model, Simulate, Results and Help, which stay disabled | `chrome/MenuBar.tsx` | scene |
| ↳ project tab and dirty dot (design:37-40) | `ProjectTab` | `chrome/MenuBar.tsx` | scene |
| ↳ Run button with F5 (design:43) | `RunButton`: `disabled`, `data-part="run"`, `data-blockers` | `chrome/RunButton.tsx` | scene |
| step bar, 44 px (design:46-65) | `StepBar`: five steps and their subs (`data-part="sub"`) | `chrome/StepBar.tsx` | scene |
| ↳ variant switch (design:57-64) | `VariantSwitch`: Baseline plus the variants | `chrome/VariantSwitch.tsx` | scene |
| scene list, 248 px (design:69-102) | `ScenePanel`: filter, Surfaces, Sources, Receivers | `chrome/ScenePanel.tsx` | scene |
| 3D view (design:106-173) | `Viewport`: the one canvas, view tabs, tools, plan inset frame, legend chip, axis gizmo | `features/viewport/Viewport.tsx` and siblings | viewport |
| dock, 250 px (design:175-275) | `Dock`: Acoustics (empty state), `ConsolePane`, `RunsPane` (empty) | `chrome/Dock.tsx`, `ConsolePane.tsx`, `RunsPane.tsx` | scene |
| properties, 344 px (design:278-471) | `PropertiesPanel`, which switches by step | `chrome/PropertiesPanel.tsx` | scene |
| ↳ Geometry step (design:280-310) | `GeometryPanel`: model check, dimensions, volume, surface, Import model… | `chrome/GeometryPanel.tsx` | scene |
| ↳ Materials step (design:312-350) | `MaterialsPanel`: group header, material list, the grid | `features/materials/MaterialsPanel.tsx` and siblings | materials |
| ↳ Sources & receivers step (design:352-384) | `SourcesPanel`: position fields, emission (read-only), receivers, Place in view | `chrome/SourcesPanel.tsx` | scene |
| ↳ Simulate and Results | M9's hint text, kept for M11 and M12 | `chrome/PropertiesPanel.tsx` | scene |
| status bar, 24 px (design:474-481) | `StatusBar` (moved out of `App.tsx`) | `chrome/StatusBar.tsx` | scene |
| (modal) | `ImportDialog`: unit and up axis, `m` and `z` preselected and shown, confirmed by the user | `chrome/ImportDialog.tsx` | scene |

**Where the materials grid goes.** It is the one interpretation of the layout in this plan. The
design draws "Absorption α · editable" as six band cells for the selected material, in the
properties panel. The grid generalises that block to every library material:
- one row per material and one column per band, in the design's cell style;
- a segmented Absorption | Scattering switch;
- horizontal scrolling for third-octave projects, with the themed scrollbar.

This adds no new region.

### 2.4 Rules every package follows

1. **Design.** Use the tokens in `theme.css`, which follows the design's own values. There is no
   logo, no wordmark, no serif and no italic. A failure always carries a text label (`FAIL`, or
   the UI code), never colour alone. That includes the 3D view: its check-highlight overlay
   carries a DOM chip reading "FAIL · <n> faces highlighted".
2. **No solver-computed acoustic number.**
   - The Acoustics tab keeps M9's empty state.
   - Inputs that carry units, such as source power, sit inside `[data-input]`.
   - Geometry facts sit inside `[data-geometry]`.
   - Check (h) enforces this (section 3).
3. **One canvas.** It is created once for the app's lifetime by the viewport and never recreated.
   - Labels and legends are DOM.
   - Sprites use `DataTexture`, never `CanvasTexture`.
   - M9's `probeWebGL` creates an unattached canvas, which `querySelectorAll` does not count.
4. **CSS ownership.**
   - `theme.css` keeps the tokens, the base rules and the scrollbar rules; it belongs to the
     foundation and is frozen after it.
   - The M9 region rules move to `chrome/chrome.css` (scene) and
     `features/viewport/viewport.css` (viewport). Materials adds
     `features/materials/materials.css`.
   - Packages never edit `theme.css`. They use its variables.
5. **Selectors the M9 self-test reads must stay:** `[data-step]` with `[data-part="name"]`,
   `[data-menu]`, `[data-dock-tab]`, and `[data-part="viewport"]` on the viewport root.
6. **The startup screen with no project shows no digit next to a unit.** M9 check (h) runs its
   ACOUSTIC regex on it.
7. **Only `actions.ts` calls `backend`.** Packages import actions, ops and stores; they never
   invoke directly.
8. **Pure logic in pure modules** (TSV, bands, flood-fill, numbers). Such a module imports
   siblings with a `.ts` extension, uses only erasable TypeScript, and has a `*.test.ts` beside it
   run by `node --test`. Node 24 strips types. A pure module must not import `store.ts`, whose
   `Store` class uses a parameter property, which Node cannot strip.
9. **The camera is session state.** No `set_camera` op is ever issued in M10. A saved file must
   not change because the user orbited, which gates (c) and (f) depend on.

### 2.5 Test hooks, installed only when `StartupInfo.e2e`

`testhooks.ts` (foundation) provides `registerHook(name, fn)`. `window.__m10` exposes the
registered hooks. Each package registers its own hooks when its component mounts. **What a gate
says is "shown" is read from the DOM**; hooks cover what the DOM cannot show.

| Hook | Owner | Returns |
|---|---|---|
| `ready(names)` | foundation | `true` when every named hook is registered |
| `idle()` | foundation | Resolves when `busyStore` is 0 and the viewport has drawn the latest `geometry_rev` |
| `openProject(path)`, `importModel(path, unit, up)`, `saveAs(path)` | foundation | The same actions the menu calls, minus the native dialog, which WebDriver cannot drive |
| `edit(op)`, `undo()`, `redo()`, `undoDepth()`, `projectJson()`, `runBlockers()`, `dirty()`, `setStep(key)` | foundation | |
| `highlightedFaceCount()` | viewport | Faces actually uploaded to the check-highlight overlay |
| `selection()` | viewport | `{ faces: number[]; groups: string[] }`: group names, sorted, unique |
| `facesOfGroup(name)` | viewport | `number[]` |
| `aimAtFace(face)` | viewport | Puts the camera on the face's visible (interior) side and returns the client `{x, y}` of its centroid. Returns `null` unless the BVH's first hit at that point is that face |
| `frame()` | viewport | Frames the model |
| `faceClientPoint(face)` | viewport (added by the package) | The client point of a face's centroid under the current camera, without moving it; `null` unless a click there takes that face |
| `cameraState()` | viewport (added by the package) | `{ view, projection, position, direction, target }` of the main view's camera |
| `highlightPixels()` | viewport (added after the review, finding 1) | `{ pixels, changed, warn }`: the main view drawn as the app draws it and again with the check-highlight overlay hidden, read back in one task; the pixels that differ, and those moved towards the warn colour |
| `materialsGrid()` | materials (added by the package) | The grid as shown: quantity, sort, band columns, rows with their exact values, cursor and extent |
| `materialsCopy()` | materials (added by the package) | The TSV text Ctrl+C would copy for the current selection |
| `openImportDialog(path)` | scene (added by the package) | Opens the import dialog for a mesh path, as File › Open… does after the native dialog |

---

## 3. How each gate check is met

Every test title starts with its id. `m10.ps1` requires every id below to have passed, not been
skipped, with no test failing.

| Id | Gate text | Fixture and action | Evidence read | Negative or positive control | Spec (owner) |
|---|---|---|---|---|---|
| `m10-a-console` | (a) FAIL line with 'open' and '955' | `importModel(<upstream>/tutorial 2/elmia.ply, 'm', 'z')` | DOM `.console-line.FAIL .text`: one line contains `open` and `955` | Teaching room: 0 FAIL lines | `m10.shell.e2e.ts` (foundation) |
| `m10-a-highlight` | (a) highlighted faces > 0 | same | `__m10.highlightedFaceCount() > 0`; `highlightPixels()` has at least 1024 warn pixels, and they are at least half of the changed ones; the chip "FAIL · n faces" is visible | Teaching room: `highlightedFaceCount() == 0`, and `highlightPixels()` changes 0 pixels | `m10.viewport.e2e.ts` |
| `m10-a-run` | (a) Run disabled | same | `[data-part=run]` has `disabled`, and `data-blockers` holds `GEOMETRY_REFUSED` | **Teaching room: `data-blockers` is exactly `M11_PENDING`.** Without this, "disabled" is vacuous, because Run is unwired in M10 | `m10.scene.e2e.ts` |
| `m10-b-console` | (b) INFO 'Closed volume, 0 self-intersections' | `importModel(testdata/elmia_corrected.ply, 'm', 'z')` | DOM `.console-line.INFO .text` starts with that literal | Raw hall: no such line | shell (foundation) |
| `m10-b-materials` | (b) Materials '0 / 10' | same | `[data-step="materials"] [data-part="sub"]` text is exactly `0 / 10` | `tutorial1_box.simpa` reads `3 / 3` | scene |
| `m10-c` | (c) pasted 6x6 saves byte-identical | `openProject(tests/fixtures/ui/teaching_room.simpa)`, Materials step, click `[data-grid-cell="0:0"]`, then dispatch a `paste` `ClipboardEvent` whose `DataTransfer` holds the `.tsv` text read by Node, bytes kept, then `saveAs(<work>/c.simpa)` | Node compares the saved bytes with `materials_6x6.expected.json`, byte for byte | The committed start file differs from the expected file, asserted first, so the comparison can fail | `m10.materials.e2e.ts` |
| `m10-d` | (d) double-click the box ceiling | `openProject(tests/fixtures/rooms/tutorial1_box.simpa)`, `f = facesOfGroup('Ceiling')[0]`, `p = aimAtFace(f)`, a WebDriver double-click at `p` on the canvas | `selection()` is 2 faces, `[10, 11]`, groups `['Ceiling']` | A double-click on a wall face selects 2, not the Walls group's 8: the fill follows the plane, not the group | viewport |
| `m10-e-outside` | (e) RECEIVER_OUTSIDE, project unchanged | Teaching room: select R1 in the list, type `20` in `[data-field="position.x"]`, press Enter | `[data-issue-code="RECEIVER_OUTSIDE"]` is visible with that text; `projectJson()` and `undoDepth()` are equal before and after | Moving R1 to x = 4 is accepted and `undoDepth` goes up by 1 | scene |
| `m10-e-label` | (e) 'a/b' → LABEL_UNSAFE | Teaching room: R1's `[data-field="name"]` set to `a/b`, Enter | `[data-issue-code="LABEL_UNSAFE"]` is visible; the project is unchanged, as above | Renaming to `Front row` is accepted | scene |
| `m10-f` | (f) 50 edits, 50 undos, byte-identical | Teaching room: 50 edits through `__m10.edit`, covering every op kind the M10 UI issues. Each must be `applied: true`, and `undoDepth` goes up by one per edit, ending at 50. One refused edit (receiver outside) is inserted and must not move `undoDepth`. Then 50 × **Ctrl+Z** as real key events, then `saveAs(<work>/f.simpa)` | The saved bytes equal the committed `teaching_room.simpa`; `undoDepth() == 0`; `dirty() == false` | After 49 undos the saved bytes differ | shell (foundation) |
| `m10-g` | (g) one canvas | Load the hall, visit all five steps, both view tabs (Perspective and Plan), all three dock tabs, then load the box | `document.querySelectorAll('canvas').length === 1` | none | viewport |
| `m10-h` | (not in the spec) no solver-computed acoustic number | Teaching room and the hall, every step and dock tab | `[data-dock-panel="acoustics"]` has no digit. The body text with `[data-input]` and `[data-geometry]` removed has 0 matches of `/\d\s*(dB\|s\|ms\|%)(?![\p{L}\p{N}])/u` | none | shell (foundation) |

`m10-h` is added by this plan from the milestone rules; the gate text does not name it. Say so
when reporting.

**Other checks each package's spec carries** must pass, but are not gate ids:
- **viewport:**
  - a placement click adds a receiver 1.2 m above a floor hit;
  - the Plan tab shows a top orthographic view;
  - a pick after the camera orbits maps to the right project face.
- **materials:**
  - the copy round trip is exact;
  - row fill;
  - `1.5` on a used material is refused inline with `MATERIAL_VALUE_OUT_OF_RANGE`, and `0,5`
    with `NOT_A_NUMBER`;
  - the band headers of a third-octave project (`tutorial1_box`) read 50 … 20k in ascending
    order;
  - a paste with a header row maps its columns by frequency.
- **scene:**
  - the dirty dot;
  - adding and switching a variant;
  - themed scrollbars: every scroll container's computed `scrollbar-color` equals a probe
    element's, and none is `auto`;
  - the step subs;
  - the scene filter.

---

## 4. The e2e harness

### 4.1 Versions and install locations (all on C:)

| Piece | Where | How |
|---|---|---|
| tauri-driver | `%USERPROFILE%\.cargo\bin\tauri-driver.exe` | `cargo install tauri-driver --locked`, 2.x, pinned at install and printed by the gate |
| msedgedriver | `C:\tmp\nm-e2e\msedgedriver\<pv>\msedgedriver.exe` | Must match the WebView2 runtime exactly: **154.0.4258.37 today**. The gate reads `HKLM:\SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}` `pv` on every run. `-FetchDriver` downloads `https://msedgedriver.microsoft.com/<pv>/edgedriver_win64.zip`. Without the switch, a missing driver is a FAIL that prints the URL. The runtime auto-updates (153 at M9, 154 now), which is why the version is never hard-coded |
| WebdriverIO | `C:\tmp\nm-e2e\node\node_modules` | `app/e2e/package.json` and `package-lock.json`, committed and pinned exactly: `@wdio/cli`, `@wdio/local-runner`, `@wdio/mocha-framework`, `@wdio/spec-reporter`, `@wdio/junit-reporter`, v9. The gate copies both files there and runs `npm ci` only when the lock's sha256 changes. **Nothing from wdio lands on B:** (exFAT, 128 KB clusters) |
| three.js, three-mesh-bvh, @types/three | `app/node_modules` (B:) | Foundation adds them to `app/package.json` pinned exactly, and the lock with them. A worktree `npm ci` on B: installs about 3,500 files today, plus these packages. **Count and report the files** before and after |

### 4.2 `app/e2e/m10.conf.ts`: no bare-specifier runtime import

wdio resolves its runner, framework and reporters from its own install on C:. The config and the
specs import only `node:` built-ins and relative files. The globals (`browser`, `$`, `expect`)
are injected.

```ts
import { spawn, type ChildProcess } from 'node:child_process';
import path from 'node:path';
const need = (k: string) => { const v = process.env[k]; if (!v) throw new Error(`${k} unset: run tools/gates/m10.ps1`); return v; };
let driver: ChildProcess | undefined;
export const config = {
  runner: 'local', maxInstances: 1,
  specs: [path.join(import.meta.dirname, 'specs', `m10.${process.env.M10_SPEC ?? '*'}.e2e.ts`)],
  hostname: '127.0.0.1', port: 4444,
  capabilities: [{ maxInstances: 1, 'tauri:options': { application: need('M10_APP'), args: ['--e2e'] } }],
  framework: 'mocha', mochaOpts: { ui: 'bdd', timeout: 180_000 },
  reporters: ['spec', ['junit', { outputDir: need('M10_WORK'), outputFileFormat: (o: { cid: string }) => `junit-${o.cid}.xml` }]],
  beforeSession: () => { driver = spawn(need('M10_TAURI_DRIVER'), ['--native-driver', need('M10_NATIVE_DRIVER')], { stdio: ['ignore', 'inherit', 'inherit'] }); },
  afterSession: () => { driver?.kill(); },
};
```

- Each spec file gets its own worker and session, so the app is relaunched and state is isolated
  per package.
- Environment passed by the gate:
  - `M10_APP`, `M10_WORK`, `M10_REPO`, `M10_TAURI_DRIVER`, `M10_NATIVE_DRIVER`;
  - `M10_ELMIA_RAW`, the upstream path; missing is a FAIL, never a skip;
  - `M10_SPEC`, optional: `shell`, `viewport`, `materials` or `scene`.
- `app/e2e/lib/` (foundation) holds `hooks.ts` (typed `__m10` calls with `idle()` after each),
  `files.ts` (byte compare with the first differing offset), and `dom.ts`.

### 4.3 How the app is built for it

1. `npm ci` in the worktree's `app/`, once, on B:. Report the file count.
2. From `app/`, with `CARGO_TARGET_DIR=C:/tmp/nm-target` (forward slashes; Git Bash mangles
   backslashes), run `npx --no-install tauri build --no-bundle`.
   - This produces `C:/tmp/nm-target/release/app.exe`, a custom-protocol build that embeds
     `ui/dist`.
   - **A debug or `cargo build` app would load the dev URL.**
3. The gate checks that `app.exe` is newer than the build's start, as M9 (b) does.

### 4.4 `tools/gates/m10.ps1`

It runs on **Windows PowerShell 5.1**: `pwsh` is not installed, although the gate text says
`pwsh`. It has the same shape as M9: `Check` blocks, a `-Only` for partial runs that never print
"M10 PASSED", and native programs called through `cmd /c`.

- **Parameters:**
  - `-TargetDir C:\tmp\nm-target` (absolute);
  - `-E2eHome C:\tmp\nm-e2e`;
  - `-Only all|static|e2e`;
  - `-Spec shell,viewport,materials,scene`;
  - `-FetchDriver`;
  - `-SkipCore`, for iteration only.
- **Work folder:** `<TargetDir>\gates\m10\<stamp>`, on C:.

1. **Static:**
   - `m9.ps1 -Only static -TargetDir <same>`;
   - the command inventory: 28 commands, equal across the attributes, the handler, `build.rs` and
     `capabilities/default.json` `allow-*`;
   - lint:
     - `backend.` is called only from `actions.ts`, `selftest.ts` and `App.tsx` boot;
     - `projectApply` only from `selftest.ts`;
     - `<canvas` and `createElement('canvas')` only under `features/viewport/` and in `gpu.ts`;
     - `theme.css` unchanged since the foundation commit, compared by hash;
   - `npm run typecheck`;
   - `npm test`: M9's known answers and `node --test "ui/src/**/*.test.ts"`.
2. **Rust:**
   - `cargo test -p app`, clippy `-D warnings`, `fmt --check`;
   - `cargo test -p simpa-core --test ui_fixtures`, which checks the fixtures against their
     recipes;
   - unless `-SkipCore`, `cargo test -p simpa-core -p simpa`, the core crates' tests.
3. **Build:** 4.3.
4. **Harness prerequisites:**
   - tauri-driver present, and its version printed;
   - the WebView2 `pv` read, and the msedgedriver of exactly that version present, checked with
     `msedgedriver --version`;
   - the e2e node dependencies current, by the lock hash;
   - the raw hall present.
5. **The e2e lock.** One lock serialises every build-and-e2e step:
   `[IO.File]::Open("C:\tmp\nm-e2e\e2e.lock", 'OpenOrCreate', 'ReadWrite', 'None')`, held for the
   build and the e2e together, with a wait of up to 30 min. The reasons:
   - port 4444 is fixed;
   - parallel packages share one `app.exe` and one `ui/dist`.
6. **Run:** `npx --prefix C:\tmp\nm-e2e\node wdio run app/e2e/m10.conf.ts`, from the repo root,
   with the environment of 4.2.
7. **Verdict:**
   - parse every `junit-*.xml`;
   - each required id (section 3) must be present and passed;
   - failures and skips must both be 0;
   - print each test with its time.
8. Count the files created under the work folder, then print "M10 PASSED" and exit 0.

### 4.5 The M9 fix, owned by the foundation

These are harness fixes. **M9's criteria do not change.**
- `m9.ps1` accepts an absolute `-TargetDir` (`[IO.Path]::IsPathRooted`).
- Its work folder moves to `<target>\gates\m9\<stamp>`, off B:.
- It clears the variable with `$env:CARGO_TARGET_DIR = $null`, not `Remove-Item Env:`, which the
  no-delete guard misreads.

If three.js brings a URL string into `dist`, M9 (f) lists it the way it lists the React
exceptions, with its reason, and only after reading the string. The foundation adds three.js first
and runs M9 (f) before the packages start.

---

## 5. Fixtures (`tests/fixtures/ui/`: 4 files; `tests/fixtures/** -text` keeps the bytes)

**Where they come from.** One recipe, `crates/simpa-core/tests/ui_fixtures.rs`, written in the
style of `geometry_cli_fixtures.rs`. With `SIMPA_WRITE_FIXTURES=1` it writes the files. Without
it, each committed file must equal its recipe's output. It runs as part of the core crates' tests.
**Nothing in the recipe imports app or UI code.**

| File | What | How it is produced |
|---|---|---|
| `teaching_room.simpa` | The design's room | Built from core types. The ids are fixed (`Uuid::from_u128`) |
| `materials_6x6.tsv` | 6 rows (the six materials, in project order) × 6 bands of absorption | Written by the recipe |
| `materials_6x6.expected.json` | The start project with the TSV applied | **Computed from the committed `.tsv` bytes**, not from the recipe's values (below) |
| `PROVENANCE.md` | Recipe, values, sources | Hand-written |

**`teaching_room.simpa`** follows the design:
- **Room.** A 10 × 6 × 3 m box: x 0-10, y 0-6, z 0-3. 8 vertices and 12 outward triangles.
  Octave bands, 125 Hz-4 kHz.
- **Groups.** Floor, Ceiling, Left wall, Right wall, Front wall, Rear wall.
- **Six materials,** every value 0.10 before the paste:
  - Linoleum on concrete, used by Floor;
  - Mineral-fibre tile, used by Ceiling;
  - Painted plaster, used by Left, Right and Front;
  - Slotted wood panel, used by Rear;
  - Heavy velour curtain, unused;
  - Glass window, unused.
- **Source.** S1 at (2, 3, 1.5): omni, pink, 85 dB.
- **Receivers.** R1 at (5, 2, 1.2), R2 at (8, 4, 1.2), R3 at (3.5, 4.5, 1.2).
- **Asserted by the recipe:**
  - the M4 check is ok, with volume 180 m³ and area 216 m²;
  - `validate()` returns **0 errors and 0 warnings**, so Run's blockers are exactly
    `M11_PENDING`;
  - the placeholder rule gives 6 of 6.

**`materials_6x6.tsv`** looks like a spreadsheet copy:
- CRLF line endings and a trailing CRLF, no header, no name column;
- textbook values: the design's for lino, tile, plaster, panel and curtain, and a published glass
  row;
- **one cell holds a 17-significant-digit decimal in (0.05, 0.95) that `serde_json::from_str`
  misreads in this build.** It is found by a seeded search and asserted in the recipe, so a lossy
  path anywhere between the paste and the save changes the bytes. The recipe also asserts that
  every cell matches `parseStrictDecimal`'s grammar.

**`materials_6x6.expected.json`**, which is the honest part:
1. The recipe reads the **committed `.tsv` bytes** with its own reader: split on `\r\n` then on
   `\t`, and convert each cell with `str::parse::<f64>`, which is correctly rounded, as
   JavaScript's `Number()` is.
2. It loads the **committed `teaching_room.simpa`** with `schema::load`.
3. It applies one `SetMaterialBand { material: materials[r].id, quantity: Absorption, band: b,
   value }` per cell, through `History::apply`.
4. It writes `schema::to_json`.

The expected file is therefore the core's output for that input. The UI must reach the same bytes
by its own path: its TSV parser, `opText`, `Op::from_json`, `History`, `schema::save`. It is never
typed to match what the UI produces.

**Pinned so the two paths agree:**
- the grid's default row order is project order;
- a paste lands at the focused cell, all or nothing;
- one paste is one `batch` op and one undo step.

**Other inputs:**
- `tests/fixtures/rooms/tutorial1_box.simpa` ("the box", for (d));
- `testdata/elmia_corrected.ply` (for (b));
- the upstream `resources/doc/tutorial/tutorial 2/elmia.ply`, read-only, on Grace (for (a)).

---

## 6. Packages

### 6.0 The foundation: sequential, before the packages; it owns every shared file

- **Rust:**
  - `app/src-tauri/**`: `commands.rs`; the new `scene.rs`; `bridge.rs` with the 1.10 additions;
    `bindings.rs`; `main.rs` (`--e2e`); `build.rs`; `capabilities/default.json`;
  - unit tests:
    - checked apply: refusal, acceptance, and the index-shift identity;
    - `ui_code` exhaustive;
    - the placeholder rule on three projects;
    - the check-line wording;
    - the mesh buffer round trip;
    - dirty serials across edit, undo, redo and save.
- **Fixtures:** `crates/simpa-core/tests/ui_fixtures.rs` and `tests/fixtures/ui/*`.
- **UI shared files:**
  - `App.tsx`: composition, boot (`scene_state` after startup), and the global key map
    (Ctrl+Z, Ctrl+Y, Ctrl+Shift+Z, Ctrl+S, Ctrl+Shift+S, Ctrl+O), ignored while focus is in an
    input;
  - `store.ts`, `actions.ts`, `ops.ts`, `numbers.ts` (and its test), `mesh.ts`, `issues.ts`,
    `testhooks.ts`, `backend.ts`, `bindings/**`;
  - `steps.ts`, `main.tsx`, `gpu.ts`, `selftest.ts`, `checksum.ts`, `fonts.css`, `theme.css`,
    `chrome/icons.tsx`;
  - `ui/tsconfig.json`: `allowImportingTsExtensions` and `erasableSyntaxOnly`;
  - `vite.config.ts`, `app/package.json` and its lock (three, three-mesh-bvh, @types/three;
    `test` runs `node --test`), `ui/scripts/**`.
- **Hand-over stubs.** Created by the foundation; each package owns its stub from then on:
  - split M9's `chrome/Panels.tsx` into the `chrome/*` files of 2.3, moving the working M9 code
    unchanged, and delete `Panels.tsx`;
  - move the region rules out of `theme.css` into `chrome/chrome.css` and
    `features/viewport/viewport.css`;
  - `features/viewport/Viewport.tsx` renders M9's empty viewport, with `data-part="viewport"`;
  - `features/materials/MaterialsPanel.tsx` is a placeholder;
  - three spec shells, `app/e2e/specs/m10.{viewport,materials,scene}.e2e.ts`, each with one
    failing `it` per owned id. A pending test fails the gate, so nothing passes by being absent.
- **Harness and gates:**
  - `app/e2e/{package.json, package-lock.json, tsconfig.json, m10.conf.ts, lib/**}` and
    `specs/m10.shell.e2e.ts` (ids `a-console`, `b-console`, `f`, `h`);
  - `tools/gates/m10.ps1`, and the `m9.ps1` fix;
  - install tauri-driver, the msedgedriver of the current `pv`, and the e2e node dependencies.
- **Docs:** `docs/investigations/2026-09-29-m10/**`.
- **Done when:**
  - `m9.ps1` passes in full against `C:/tmp/nm-target`;
  - the core crates' tests pass;
  - the bindings regenerate clean;
  - `m10.ps1` passes every foundation id;
  - every package id fails as pending, and nothing else fails.

### 6.1 Package `viewport`

- **Owns:**
  - `app/ui/src/features/viewport/**`: `Viewport.tsx`, `viewport.css`, and any modules and tests
    inside it;
  - `app/e2e/specs/m10.viewport.e2e.ts`.
- **Builds:**
  - **One `WebGLRenderer`** and one canvas, created on mount and kept for the app's lifetime.
    Rendering is on demand, not in a loop. Context loss is handled.
  - **The mesh.** Built from `meshStore`:
    - positions from the f64 buffer, as f32 for the GPU;
    - an index in project face order;
    - shading neutral, as in the design.
  - **Inside view.** Faces are drawn `BackSide` (normals point out of the room), so the near
    walls drop away. Raycasts honour it.
  - **Outlines.** Feature edges from `EdgesGeometry` at 1°, in the design's light line colour.
  - **Picking.** three-mesh-bvh with **`indirect: true`** (F6), so `faceIndex` equals the project
    face index. A click selects a face.
  - **Double-click flood-fill** follows upstream's `GetAllCoplanarFaces` rule
    (`Objet3D.cpp:1397-1444`), made **iterative**:
    - neighbours are faces sharing an exact vertex position;
    - the criterion is `|n_src · n_face| > 0.998` against the source face's normal;
    - it is not limited to the group;
    - an explicit stack and a visited `Uint8Array` replace upstream's recursion.

    Unit tests: a plane in a grid; a riser that stops the fill; a 1° tilt accepted and a 5° tilt
    refused.
  - **Overlays:**
    - the check highlight (`check.highlight_faces`) in the warn colour, with the DOM chip
      "FAIL · n faces highlighted";
    - the selection, in the design's red wash and outline, for faces or a whole group.
  - **Markers.**
    - Sources: a red dot with a glow from a `DataTexture`.
    - Receivers: white rings.
    - Stems down to the floor.
    - DOM labels projected each frame.
  - **Plan inset.** A second pass in the **same renderer**, with an orthographic top camera and a
    scissor rectangle under the DOM frame at bottom left. It draws the room outline, the selected
    group's edges in red, and the markers. The label is "PLAN · L × W m", a geometry fact.
  - **Plan tab.** The main view in the same orthographic top camera.
  - **Section tab** and the Section and Measure tools stay disabled. Upstream has neither.
  - **Orbit, zoom and pan** use OrbitControls. The model is framed on load.
  - **Tools:** Select, Orbit, and Place receiver / Place source, which write `toolStore`.
    - In a placement mode, a click on a floor-like face (normal within 45° of vertical) calls
      `actions.placeAt(kind, hit + (0, 0, 1.2))`. A source uses +1.5 m.
    - A refusal is shown by the scene package.
  - **Axis gizmo** as SVG, following the camera.
  - **Hooks** from 2.5.
- **Reads:**
  - `meshStore`;
  - `sceneStore`: `groups`, `view.sources`, `view.point_receivers`, `check`;
  - `selectionStore`, `toolStore`, `stepStore`.

  It writes `selectionStore`.
- **Done when:**
  - ids `a-highlight`, `d` and `g` pass (`m10.ps1 -Only e2e -Spec viewport`), with the extra
    checks in section 3;
  - its unit tests pass;
  - no type errors in its files.

### 6.2 Package `materials`

- **Owns:**
  - `app/ui/src/features/materials/**`: `MaterialsPanel.tsx`, `materials.css`, and any modules
    and tests inside it;
  - `app/e2e/specs/m10.materials.e2e.ts`.
- **Builds:**
  - **The design's Materials panel:**
    - the selected group's header: name, "Surface group · area m² · n faces";
    - the material radio list with α mini-bars. Picking a material calls
      `ops.assignMaterial`, which writes the base or the active variant's override;
    - Scattering, through the grid's quantity switch;
    - Transmission, as a read-only "off" or "on in n bands";
    - the hint "Double-click a face to take its whole flat surface."
  - **The grid** (2.3):
    - one row per library material, in project order by default, with a swatch, a name cell
      (rename) and "used by n";
    - band columns in ascending frequency, labelled `125 · 250 · 500 · 1k · 2k · 4k` and in
      thirds `50 … 1.25k … 20k`;
    - a focus cell and a rectangular selection; arrows, Tab, Enter or F2 to edit, Esc to cancel;
    - add a row (`add_material` at the end) and delete a row (`OP_IN_USE` is shown inline).
  - **TSV:**
    - **Ctrl+C** writes the selected block as TSV of the exact shortest round-trip values,
      whatever the display rounding. The display never rounds without marking it.
    - **Ctrl+V** parses the block (CRLF or LF, a trailing newline, Excel's quoted cells). It then
      detects an optional header row and an optional leading name column. A header row counts
      only if every band cell is a label with a unit or a `k` suffix (`125 Hz`, `1k`, `1 kHz`),
      or the first cell is text. A header maps columns **by frequency**; that is the natural band
      sort. A name column maps rows by exact name.
    - **All or nothing.** A shape that does not fit gives `PASTE_SHAPE`, an unknown band gives
      `PASTE_HEADER`, a cell that fails the grammar gives `NOT_A_NUMBER`. Each is refused before
      any op, and the bad cells are named inline.
    - **One paste is one `batch`.**
  - **Row fill** (Ctrl+R and a "Fill row" button) copies the focused cell across its row, as one
    `batch`.
  - **Natural sort of rows** by name (`Material 2` before `Material 10`) or by a band column's
    value, on a header click. Default order is project order.
  - **Inline validator messages:**
    - the cell's issues from `sceneStore.issues`, whose field is `absorption/<b>` or
      `scattering/<b>`;
    - refusals from `refusalStore`, shown as "FAIL `<CODE>` — message" under the grid, with the
      cell outlined.

  **Pure modules** for TSV, bands and fill, each with a `node --test` suite.
- **Reads:** `sceneStore` (`view.materials`, `view.surface_groups`, `groups`, `view.bands`,
  `issues`), `selectionStore`, `refusalStore`.
- **Writes:** only through `actions.apply` with `ops`.
- **Done when:**
  - id `c` passes, with the extra checks in section 3;
  - its unit tests pass;
  - no type errors in its files.

### 6.3 Package `scene`

- **Owns:**
  - `app/ui/src/chrome/**` except `icons.tsx`: `MenuBar.tsx`, `RunButton.tsx`, `StepBar.tsx`,
    `VariantSwitch.tsx`, `ScenePanel.tsx`, `PropertiesPanel.tsx`, `GeometryPanel.tsx`,
    `SourcesPanel.tsx`, `Dock.tsx`, `ConsolePane.tsx`, `RunsPane.tsx`, `StatusBar.tsx`,
    `ImportDialog.tsx`, `chrome.css`;
  - `app/e2e/specs/m10.scene.e2e.ts`.
- **Builds:**
  - **Menu bar:**
    - File: New project, Open… (`.simpa .ply .obj .stl`; a mesh opens `ImportDialog`), Save,
      Save as…;
    - Edit: Undo, Redo, with their key hints;
    - View: Perspective, Plan, Frame model;
    - the rest disabled;
    - the project tab with the dirty dot (`aria-label="Unsaved changes"`);
    - Commands disabled;
    - **`RunButton`**: always disabled in M10, with `data-blockers` and a tooltip listing each
      blocker as text.
  - **Step bar:**
    - the subs: Geometry `closed` / `refused` / `no model`; Materials
      `groups_assigned / surface_groups`; Sources & receivers `<enabled sources> · <receivers>`;
      Simulate and Results empty;
    - **`VariantSwitch`**: Baseline plus the variants, where a click is `set_active_variant`. A
      "+" is `batch(add_variant, set_active_variant)`, and double-click renames (see 7.5).
  - **Scene list** (design:69-102):
    - the filter;
    - Surfaces: the swatch and short name of the effective material, and a FAIL label when an
      issue points at the group;
    - Sources: the enabled state and "Omni · 85 dB" inside `[data-input]`;
    - Receivers: "x, y" coordinates and a FAIL label for issues;
    - a row sets `selectionStore`. `[data-entity="<kind>:<id>"]`.
  - **Properties shell:**
    - **`GeometryPanel`:**
      - "Room model · <file> · imported, checked" or "… refused";
      - the model check list with text status: Closed volume yes/no, Self-intersections n,
        Flipped normals n, Open edges n, Units "m (chosen at import)";
      - Dimensions (the bounding-box extents), Volume and Surface, inside `[data-geometry]`;
      - an **Import model…** button, and the format line `PLY · OBJ · STL`. Only those are real
        (the design's `3DS · glTF · IFC` and "Replace model…" are listed in 7.2);
    - **`SourcesPanel`:**
      - the selected source or receiver: name (`data-field="name"`), and position X, Y and Z
        (`data-field="position.x|y|z"`, committed on Enter or blur through `parseStrictDecimal`
        and a move op);
      - inline `[data-issue-code]` elements from `refusalStore` and `issues`;
      - emission read-only: Sound power, Spectrum, Directivity, inside `[data-input]`;
      - a receivers list with inside, or a FAIL label and code;
      - Place in view (`toolStore`) and + Source / + Receiver;
      - Del to remove the selected entity, F2 to rename it (see 7.5);
      - the hint: "Receivers snap to 1.2 m ear height. One placed outside the room is refused and
        the project is left unchanged";
    - Simulate and Results keep M9's hints.
  - **Dock:**
    - Acoustics keeps M9's empty state, with `data-dock-panel="acoustics"`;
    - Console shows FAIL, WARN, INFO and OK as text labels, with a "n fail" badge;
    - Runs is M9's empty table.
  - **Status bar** (design:474-481): "Model closed · n surfaces · V m³" as a geometry fact, the
    variant, the solvers, the units.
  - **`ImportDialog`:** unit (m, cm, mm, ft, in) and up axis (z, y), with `m` and `z` preselected
    and shown, and Import or Cancel.
  - **Themed scrollbars** on every scroll container: the scene list, the properties, the
    console, the dialog.
- **Reads:** every store. **Writes:** through actions.
- **Done when:**
  - ids `a-run`, `b-materials`, `e-outside` and `e-label` pass, with the extra checks in
    section 3;
  - no type errors in its files.

### 6.4 Rules for building in parallel

- **No package edits a file it does not own.** If a package needs a shared change (a store field,
  a new op builder, a hook in `actions.ts`), it asks the foundation owner and does not patch
  around the gap.
- **If all three work in the one `m10` worktree at the same time:**
  - every write leaves the file compiling (stub first, whole-file writes);
  - `tsc` errors are judged per owned path;
  - every `tauri build` and e2e run goes through `m10.ps1 -Only e2e -Spec <pkg>`, which holds the
    lock of 4.4, so no one builds over another's `app.exe` or `ui/dist`;
  - nothing but `m10.ps1` writes into `C:/tmp/nm-target`.
- **No new npm dependency.** The foundation pins every one.
- **Integration**, after all three:
  - `m10.ps1` in full;
  - `m9.ps1` in full;
  - the core crates' tests;
  - a critic;
  - update `docs/rebuild-plan.md`, `docs/scope.md`, and decision-log rows for section 8's
    technical calls;
  - a session summary before any push.

---

## 7. Parity: the before-v1 features M10 must hold

This covers the geometry, materials, and sources & receivers areas of
`docs/investigations/2026-09-25-parity-audit/parity-matrix.md`. The audit's "Before v1" is its
recommendation. **Folding those items into M10 is Burhan's call**, and it was still pending in
the handoff ("fold the 43 before-v1 items into M10-M13 specs with Burhan before M10 starts").

### 7.1 Named by M10's spec: built and held

| # | Feature | Package |
|---|---|---|
| G1, G2 (and OBJ) | Import PLY with layers as groups; STL | foundation (`model_import`), scene (menu, dialog) |
| G6 | Import units, explicit | scene (`ImportDialog`), foundation |
| G16 | Scene tree | scene. The design's list has no fitting zones or volumes |
| G17 | Surface groups from the file | foundation |
| G37 | Faces the check refuses, highlighted (gate a) | foundation (`highlight_faces`), viewport |
| G40 | Orbit, zoom, pan | viewport |
| G43 | Inside view | viewport |
| G44 | Outlines | viewport |
| G47 | Face selection: click, and double-click for a flat surface (gate d) | viewport |
| G48 | Pick a position, for sources and receivers | viewport (pick), scene (panel), foundation (`placeAt`) |
| M2 | User materials: create, rename, delete, edit | materials |
| M4 | Absorption and scattering per band | materials |
| M7 | Consistency rules as inline messages | materials, foundation (checked apply) |
| M10 | Assign a material to a group, as base or variant override | materials, foundation (`ops.assignMaterial`) |
| M11 | Spreadsheet editing: TSV copy and paste, row fill (gate c) | materials |
| M19 | Source position, typed | scene |
| M30 | Source and receiver markers in 3D | viewport |
| M32 | Point receiver position and label (gate e). Orientation is not on the design | scene |
| M15 | Unassigned faces refused | foundation: the `MATERIALS_UNASSIGNED` blocker and the counter. **F1: the core does not refuse `Default` today.** See section 8 |
| A1, A2, A4, A5, A10 | New, Open, Save, Save as, Undo and redo | foundation (actions, key map), scene (menus) |

### 7.2 Before v1 in these three areas and **not named by M10's spec**: listed here, not built

| # | Feature | Status in the audit | What it would take | Package if added |
|---|---|---|---|---|
| G7 | Replace model, keeping groups (the design's "Replace model…") | design only | a `model_reimport` command over `geometry::import::reassign` | foundation and scene |
| G8 | Repair at import | core only | a `model_repair` command over `geometry::repair`, and a Repair button after a refusal | foundation and scene |
| G11 | New scene: box room | absent | a box generator command and a dialog | foundation and scene |
| G18 | Add a surface group | core only | a verb in the scene list over `add_surface_group` | scene |
| G19 | Send selected faces to a new group (M) | deferred | **a new core op (there is no regroup op in `ops.rs`): a core change** | core, then viewport and scene |
| G42 | Reset camera | absent | View › Frame model and a button (framing on load is built anyway) | viewport, scene |
| M1 | Reference material library ("30 % absorbing") | deferred | "Add from library" over `REFERENCE_MATERIALS` | materials |
| M5 | Reflection-law column | core only | a law column in the grid | materials |
| M6 | Transmission per band (the design's "Transmission off") | design only | **read-only in M10**; editing is not named | materials |
| M20 | Source power and spectrum | design only | **read-only in M10**; editing is not named | scene |
| M22 | Directivity choice | design only | **read-only in M10**; editing is not named | scene |
| M26 | Source on or off | core only | a checkbox over `set_source_enabled`. Disabling the only source is refused `SOURCE_NONE` by the checked apply | scene |
| M37 | Point-receiver groups, on the import side | absent | a core change to `import-proj`, not GUI work | core |
| M40 | Scene surface receiver | core only | create one from groups | scene, viewport |
| M41 | Cutting-plane receiver | core only | a three-point gizmo | viewport, scene |
| M42 | Cutting-plane display before a run | absent | draw the plane | viewport |

G22 (area and volume, design only, before v1) **is built**: the design's Geometry panel draws it,
and geometry facts are allowed. It is flagged here so that it is not taken as silent scope.

### 7.3 Outside the three areas but on M10's surface (before v1)

- **A3, open an upstream `.proj`** (core only). M10 says "import"; its gates test only `.ply`.
  One extra branch in `model_import` over `import_proj_file`. Listed.
- **A8, the unsaved-changes marker** (design only). **Built**: the design draws it (the
  `dirty` flag, 1.10).
- **A9, a save prompt before New, Open and Exit** (absent). **Listed, and worth adding:** M10
  brings editing, so New, Open and closing the window now discard work without a word
  (`Session::replace`). It is S-sized: a close-requested handler and one dialog.
- **A29, rename and delete any element.** Built only where M10 needs it (7.5).
- **A39, keyboard shortcuts.** Only the named verbs get keys (7.5).

### 7.4 Partial gaps inside named features (from the audit's section 3d): not built, listed

- **G43:** the outside and no-face display modes.
- **G44:** the all, contour and none switch.
- **G47:** Ctrl multi-select, drag-select, and a face list per group.
- **G48:** picking plane corners, orientation points and zone corners.
- **G37:** TetGen-diagnosed faces (`mesh::diag`) are not highlighted.
- **M11:** band grids for source spectra and zones.
- **M30:** per-item colour, a name toggle, direction arrows.
- **M32:** the orientation field.

### 7.5 Interpretations this plan makes, each flagged

1. **Placement includes removal** of sources and receivers (Del) and renaming (F2). Gate (e)
   renames, and placement without removal leaves misplaced points stuck. A29 is covered for
   sources, receivers, materials and variants; groups get no rename or delete.
2. **The variant switch includes adding and renaming a variant.** Without them the switch shows
   only Baseline on every new project. Deleting a variant is `remove_variant`, which the core
   refuses while the variant is active.
3. **Keys.** Ctrl+Z, Ctrl+Y, Ctrl+Shift+Z, Ctrl+S, Ctrl+Shift+S and Ctrl+O belong to the named
   verbs (save, open, undo, redo). The rest of A39 (F2, Del outside the scene panel, a full key
   map) is not built.
4. **The Plan tab** as a main view is drawn on the design. Upstream does not have it. It reuses
   the inset's camera.
5. **The grid lives in the properties panel** (2.3).
6. **Emission, transmission and directivity are displayed read-only**, as drawn.

---

## 8. Calls made here, and questions for Burhan

**Technical calls** (Jarvis, row 13). Each is recorded in the decision log at the merge.

| Call | Why |
|---|---|
| **The checked apply:** refuse only the new errors an edit introduces; apply warnings; identity by entity and field (1.8) | One rule for every edit; a pre-existing error never locks the user out; gate (e)'s "unchanged" holds by construction |
| **UI codes** are mapped from the core's, which stay unchanged (1.7). The UI shows both | The core codes are stable API; the gate names the UI codes |
| **The placeholder rule** for "assigned" (1.10), and the `MATERIALS_UNASSIGNED` Run blocker | Upstream's `Default` means no material was chosen; this is how gate (b) reads '0 / 10' |
| The camera is never written to the project in M10 | Gates (c) and (f) need saves that do not change when the user orbits |
| three-mesh-bvh with `indirect: true`; flood-fill by upstream's rule, made iterative | F6; parity with upstream's double-click |
| The grid's default order is project order; a paste is all-or-nothing and one undo step | Gate (c) is deterministic and undo stays sane |
| The e2e's node dependencies live on C:; msedgedriver follows the live WebView2 `pv` | exFAT file counts; the runtime auto-updated from 153 to 154 |
| `m10.ps1` runs on Windows PowerShell 5.1 | `pwsh` is not installed. M9 is run the same way |

**Questions for Burhan:**

1. **Which of 7.2 and 7.3 fold into M10?** The recommendation, all S-sized:
   - A9, the save prompt, which protects work M10 now makes editable;
   - A3, `.proj` open;
   - G42, reset camera;
   - M26, source on and off;
   - M5, the law column;
   - M1, the reference library.

   G19 and M37 need core changes and belong in their own piece.
2. **Should the core refuse the `Default` placeholder (F1)?** It would be a new contract rule
   before M11's Run. M10 blocks Run in the app either way.
3. **Accept the gate reading of 'RECEIVER_OUTSIDE' and 'LABEL_UNSAFE' as UI codes** mapped from
   the core's (1.7), or rename the gate text to the core codes?
4. **Accept the added check `m10-h`** (no solver-computed acoustic number on screen) as part of
   the M10 gate?

---

## 9. Traps (paid for elsewhere, or found while planning)

- **B: is exFAT with 128 KB clusters.**
  - `npm ci` in the worktree installs about 3,500 files on B:, plus three.js. Count them and
    report the count.
  - Nothing from wdio or msedgedriver goes on B:.
  - Gate work folders go under `C:/tmp/nm-target/gates/`.
- **Build output.** Forward slashes in `CARGO_TARGET_DIR`: Git Bash has mangled backslashes into
  a 5 GB folder in the repo root before. Never build into a `target/` on B:, because B:'s old one
  is corrupt (os error 1392).
- **Windows PowerShell 5.1** turns a native program's stderr into a terminating error under
  `Stop`: use `cmd /c`. And `"$var:"` is a parse error: write `"${var}:"`.
- **The dialog plugin's native dialogs cannot be driven by WebDriver.** Every e2e path goes
  through a hook that calls the same action as the menu.
- **JavaScript numbers.** `JSON.stringify(-0)` is `"0"` and `JSON.stringify(NaN)` is `null`
  (1.5). JavaScript's `Number()` and Rust's `str::parse::<f64>` are both correctly rounded, so the
  TSV path agrees with the recipe. **serde_json's reader is not**, so ops always go as text
  through `Op::from_json`.
- **three-mesh-bvh** rearranges the index unless `indirect: true` (F6).
- **Raycasts honour `BackSide`**, so from outside and above, the ceiling is not pickable. That is
  why `aimAtFace` exists.
- **React `StrictMode`** mounts twice in dev. The renderer must survive dispose and re-create in
  dev. The release build, which the e2e uses, mounts once.
- **A synthetic `paste` event** exercises the grid's handler, not the OS clipboard. Ask Burhan to
  do one manual paste from Excel at hand-over.
- **tauri-driver's port 4444 is fixed,** and one `app.exe` and one `ui/dist` are shared: every
  build-and-e2e run takes the lock (4.4).
- **msedgedriver may append arguments** to the app's command line (1.2): check it at the first
  smoke run.
- **Only the materials in use are validated by the core** (1.8, known limit).
- **`testdata/` is queued for deletion** with Michael's clean-up of `main` (F2): move
  `elmia_corrected.ply` to `tests/fixtures/ui/` at that point.
- **An interrupt or `/compact` can kill a running workflow silently.** Check the agents'
  journals before assuming a package is still being built.
