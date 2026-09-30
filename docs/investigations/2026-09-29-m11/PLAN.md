# M11 plan: Concept B, Simulate, Console, Runs

2026-09-29, branch `m11`, cut from `rebuild` at `d7ecbda` (M10 and M8a merged). **Planning only.
Nothing is built yet.** The plan covers:
- the scope, with decision row 22's items;
- the IPC contract and the core changes;
- the UI state and component tree;
- the gate, its added checks and the e2e harness;
- the fixtures;
- the hanging core test and the two `cli_run` tests;
- M10's minor findings;
- one sequential foundation and three parallel packages;
- the calls made here and the product questions, each with the default the build follows.

The technical calls are Jarvis's under decision row 13. Burhan can overrule any of them; section 10
lists them. Burhan, 11:29: "Just keep at it as you were". So no question here stops the build: every
product question has a recommended default, and the build follows it.

## 0. The spec, verbatim (`rebuild-plan-raw-2026-09-23.json`, plan.milestones, M11)

> **Deliverable.**
> - The Simulate step drives core::run asynchronously, with live '#' progress.
> - The Console shows classified lines with FAIL / WARN / INFO / OK text labels, not colour alone.
> - The Runs tab lists run history from the manifests: status, reason, per-band particle loss %,
>   solver hashes.
> - Cancel is wired to the Job Object.
> - Results of failed or cancelled runs are refused.
> - The updater, if shipped, is blocked while a run is active.
>
> **Gate.** `pwsh tools/gates/m11.ps1` exits 0.
> (a) Run the box from the UI. The Runs row shows status OK and the text 'Particles lost 0.00 %'.
> The Console's per-class line counts equal those in the core's run manifest for the same folder.
> (b) During the corrected-hall solve, 200 IPC pings have p99 < 100 ms, and a synthetic orbit drag
> has frame time p95 < 33 ms. Both thresholds are proposed; they test 'the app stays usable'.
> (c) Cancel sets the row to 'Cancelled', and 2 s later tasklist lists no spps.exe.
> (d) Closing the window mid-solve, and separately Stop-Process on app.exe, both leave no spps.exe
> 2 s later.
> (e) A forced mesh-failure fixture shows a Runs row FAIL with reason MESH_TETGEN_SKIPPED, and the
> Results step displays no numbers for it.

Standing rules for this milestone:
- **No solver-computed acoustic number is shown** (T30, EDT, SPL, C, D, STI, ...). Only M12 may show
  one. Run diagnostics are allowed in the Runs tab and the Console: status, reason, particles lost %,
  solver hashes, timings. Section 4.2 designs the allowance so that it cannot hide an acoustic number.
- **The test windows stay visible and never take keyboard focus.** Burhan, 11:26, verbatim: "i like
  seeing the fucking test windfows, i really do". They are never hidden, minimised or moved off the
  screen. Only the focus-stealing is fixed.
- **Existing gates keep passing:** `tools/gates/m10.ps1`, `tools/gates/m9.ps1`, and the core crates' tests.
- **Builds.** Every cargo call uses `CARGO_TARGET_DIR=C:/tmp/nm-target`, `CARGO_BUILD_JOBS=16` and
  `CARGO_INCREMENTAL=0`. Nothing builds into a `target/` on B:. The verified solver build is
  `C:/tmp/nm-m8a-solvers`.
- **Limits.** At most 4 solver processes at once, and at most 2 `python.exe`.
- **exFAT.** Anything generated on B: gets a file count, with no one-file-per-item outputs.
- **The v1 filter (row 5).** Anything deferred goes to `docs/v1.1-backlog.md` in the same commit.

## Receipts gathered for this plan (2026-09-29, 18:55-19:21, Grace)

| What | Result |
|---|---|
| `mesh_project::every_failure_code_fires_on_its_input`, built into `C:/tmp/nm-target` (`cargo test --release -p simpa-core --test mesh_project --no-run`, 20.5 s), then run alone under `timeout 600`, with scratch on C: (NTFS) in this session's scratchpad and `SIMPA_SOLVERS_DIR=C:/tmp/nm-m8a-solvers` | **Hung and was killed at 600 s (exit 124), 18:59:27 to 19:09:27.** At once it printed `panicked at crates\simpa-core\tests\mesh_project.rs:665:25: the mesher must not run` and 12 `scratch kept for debugging` lines, then nothing more. Its `fake-readonly` folder holds a fresh 96-byte `scene_mesh.var` and a new `scene_mesh.poly` and `mesh.cbin`: the 5-byte read-only stale file was deleted, so the mesher went on and called the fake that must not run. Section 6 has the cause |
| `crates/simpa-core/src/mesh.rs:1234-1263`, `call()` | The watch thread loops `while !done.load(..)` (1240). It ends only on the caller's cancel, or at the program's time limit (1245): `Timeouts::TETGEN` is 3,600 s, `PREPROCESS` 600 s. `done.store(true)` (1262) sits after `mesher.run(..)`, so a panic inside the mesher skips it. `std::thread::scope` then waits for the watch thread before the panic can leave |
| `sha256sum C:/tmp/nm-m8a-solvers/*.exe` | The raw sha256s are not `solvers/manifest.json`'s `sha256`, which depend on link times. The build is verified by `code_sha256`, the executable with its link times zeroed, which `simpa_core::bed::pe::check_solvers` computes against the embedded manifest (`bed::SOLVER_MANIFEST`) |
| tao 0.35.3, `platform_impl/windows/window.rs:1147` and `window_state.rs:329-333` | `focused: false` sets `MARKER_DONT_FOCUS`, and the first show is then `SW_SHOWNOACTIVATE`, not `SW_SHOW`. `force_window_active` (`SetForegroundWindow`, plus a synthetic Alt press) runs only from `set_focus` (184) and at creation of a fullscreen window (1340) |
| wry 0.55.1, `webview2/mod.rs:546-547` and `1255-1256` | The webview calls `MoveFocus` at creation only `if attributes.focused`, and after that only on `WM_SETFOCUS`, the window getting focus |
| tauri 2.11.6 `webview_window.rs:535-538`; tauri-utils 2.9.3 `config.rs:1936, 2016` | `WebviewWindowBuilder::focused(f)` sets both the window and the webview. `WindowConfig` has `create` (default true) and `focus` (default true) |
| `app/src-tauri/tauri.conf.json` | The main window is created from the config, focused by default. Nothing in the app asks for focus. So the focus-stealing is Windows activating a new window of a process that may take the foreground (the chain starts at Burhan's terminal) |
| `tools/gates/m10.ps1:137` and `app/e2e/specs/m10.scene.e2e.ts:103` | M10's gate hard-codes `$attrs.Count -eq 28`, and `m10-a-run`'s control requires the teaching room's blockers to be exactly `M11_PENDING`. Both become false the moment M11 adds a command or wires Run. Section 4.5 changes them without loosening either |
| `crates/simpa-core/src/mesh.rs:103-114`; `docs/m5-m6-design.md:83-88` | `tetgen_skipped_facets` fires only when `<base>_skipped.face` has rows. TetGen 1.5.0, the verified mesher, never writes one: it stops at the first self-intersection (`tetgen_self_intersection`, exit 3) |
| `crates/simpa-core/src/run/manager.rs:602-616` | A run checks the geometry before it meshes. A self-intersecting model is refused `geometry_refused` at the geometry stage, and never reaches TetGen. So no honest input makes a real run fail with `tetgen_skipped_facets` (section 5) |
| `tests/fixtures/rooms/PROVENANCE.md:54`; `docs/rebuild-plan.md` (M6) | The seeded tutorial box (10,000 particles) loses 1 particle of 10,000 at 2 kHz, as upstream's own mesh of it does. `tutorial1_box.simpa` computes 27 bands at 150,000 particles |
| `tests/fixtures/ui/teaching_room.simpa` | 10 × 6 × 3 m, 180 m³, octave bands 125 Hz-4 kHz (6), SPPS 150,000 particles, random, seed 0, one source, three receivers, `preserve_boundary` on |
| `crates/simpa-core/src/geometry/import.rs:686-705` | `.proj` import ids come from an FNV digest of the file: the same `.proj` always gives the same project, byte for byte |
| Disk | C: 33 GB free. `C:/tmp/nm-target` was fresh at 18:49 (release only, no `debug/`) |

## Findings that shape the plan

- **F1. `core::run` reads a project file, not a project in memory.** `run_project(path, ..)` records
  the file's sha256. The app must therefore run the file on disk, and the file must be what the user
  sees. The default (PQ1): Run saves first, as upstream's GUI does (`projet.cpp:782-785`, parity C35).
- **F2. The run folder is created inside `run_project`, and nothing tells the caller its name
  until the run ends.** The UI needs it at once: for the Runs row "Running", for Cancel's row, and
  for gate (c). Core change C6 adds `RunEvent::Started`.
- **F3. `MESH_TETGEN_SKIPPED` cannot come out of a real run with the verified TetGen** (receipts).
  Gate (e) is met by a fixture run that the core's real run manager makes with a stand-in TetGen.
  The stand-in plays TetGen 1.6.0's output (section 5). This is the one place a fake enters, and it
  is named as such.
- **F4. The run manager does not check the solvers against `solvers/manifest.json`.** Row 17 (5)
  and scope.md's engine item 5 ask for it on every run. The CLI and the bed (the bed checks its own)
  run without it. Core change C7 adds it as an option, which the app always turns on.
- **F5. The hang is test-triggered, but its mechanism is a core defect on the Run path** (section 6).
  M11 is the first code that puts app code, the event sink, inside the mesher call. The foundation
  fixes both the core and the test.
- **F6. Gate text and core codes differ, as in M10 (row 22 (3)).** `MESH_TETGEN_SKIPPED` is
  `tetgen_skipped_facets`. The app maps core codes to UI codes with one exhaustive table and shows
  both (2.7).
- **F7. M10's `m10-h` cannot see a unitless acoustic number** (MINOR A-2). STI is in v1 (row 19),
  and the Runs tab now brings numbers with units onto the screen. The continuation `m11-h` closes
  A-2. It allows diagnostics only when each one is proven to be a manifest value (4.2).
- **F8. tasklist is machine-wide, and other sessions run solvers.** An M8b bed may run up to 4 at
  once. A literal "tasklist lists no spps.exe" would fail on someone else's run. The gate runs a
  private copy of the verified build and checks that no process runs from that copy's path. It
  prints the machine-wide count beside it (4.1).
- **F9. Keyboard focus and page focus are different things.** A window opened without activation
  leaves the WebView's page unfocused. Chromium delivers WebDriver's (CDP) key and mouse events to
  an unfocused page, but it may hold back `focus` events. M10's specs rely on focus: the materials
  grid's focused cell and Ctrl+Z. The foundation measures this at its first smoke run. The
  fallback is page-level focus emulation (CDP `Emulation.setFocusEmulationEnabled`) from the
  harness, which never touches OS focus (4.3).
- **F10. `enclosed_volume_m3` is 0 for a refused model** (`scene.rs` `check_summary`). The Geometry
  panel prints it as "Volume 0 m³" (MINOR B-18): a wrong number reaching a user. So the fix is v1,
  at the source (2.9, A5).

---

## 1. Scope

### 1.1 The deliverable and gate (a)-(e)

These are verbatim in section 0. Section 4.1 maps each gate check to its test, and 3.3 maps each
deliverable line to its component.

### 1.2 Decision row 22: M11 builds these

| Item | What exists | What M11 builds | Owner | Check |
|---|---|---|---|---|
| **A9, save prompt before New, Open and Exit** | Nothing. `Session::replace` discards silently | Before New, Open (every path: menu, Ctrl+O, a mesh or `.proj` import) and Exit (the window's close button, Alt+F4, `WM_CLOSE`), a dirty project prompts "Save changes to <name>?", with **Save**, **Don't save** and **Cancel** (upstream's Yes, No and Cancel, `main.cpp:1015-1045, 1080-1114`). Save on a project never saved opens Save as; if that is cancelled, the action is cancelled. The flow and the close interception are the foundation's (2.8, 3.2). The dialog is the project package's | foundation (flow), project (dialog) | `m11-r22-a9` (New and Open), `m11-d-close` (Exit, through a real `WM_CLOSE`) |
| **A3, opening an upstream `.proj`** | `import_proj_file` in core, and the CLI `import-proj` | File › Open… accepts `.proj`, with no unit dialog (a `.proj` carries its own units). A new command `proj_import` (2.2). The report's notes become INFO lines, a refusal becomes a FAIL line with its code, and the project is left unchanged. The project is not saved (Save as follows) | foundation (command, action), project (menu and filter) | `m11-r22-a3` |
| **G42, reset camera** | View › Frame model (M10). Framing on load | A toolbar button in the 3D view, `data-tool="frame"`, titled "Frame model (Home)", and the Home key (App.tsx key map) | project (button), foundation (key) | `m11-r22-g42` |
| **M26, source on/off** | `set_source_enabled` op in core, and the validator's `source_none` | A switch per source in the Sources panel and the scene list, through the checked apply. Switching off the only enabled source is refused `SOURCE_NONE` inline, and the project is left unchanged. A disabled source shows "off" as text | project | `m11-r22-m26` |
| **M5, the reflection-law column** | `Material.reflection_law` (`All` or `PerBand`) | A "Law" column in the materials grid, as a select: Specular, Uniform, Lambert, W2, W3, W4 and Semi-diffuse (`ReflectionLaw::ALL`). A per-band law, from a `.proj`, reads "per band", and choosing a law sets `All`. The edit is `replace_material`, one undo step | project | `m11-r22-m5` |
| **M1, the reference material library** | `REFERENCE_MATERIALS` in core (12, id 0 `Default` first) | "Add from library" in the Materials panel lists upstream's 11 reference materials, all but `Default`. It adds `add_material` with the core's exact values, the `f32`-widened α in every band of the project, as a `.proj` import makes them. The values come from Rust (`material_library`), never retyped in TypeScript | foundation (command, op builder), project (menu) | `m11-r22-m1` |
| **The core refuses the `Default` placeholder at Run** | The app blocks Run with `MATERIALS_UNASSIGNED`. The core accepts the placeholder (M10 finding F1) | A new validator rule, `material_placeholder` (project, error), fires for each surface group whose effective material is upstream's placeholder. `run_project` then refuses the run at the validate stage, exit class 2, with no solver launched. The app's counter and the core rule share one predicate (C5) | foundation (core) | `m11-r22-default` (e2e and CLI), and core unit tests |

**Not in M11 (row 22):** face regrouping (G19) and grouped receivers on `.proj` import (M37). They
have their own piece.

### 1.3 The before-v1 parity items in the run/simulate, console and runs areas

From `docs/investigations/2026-09-25-parity-audit/parity-matrix.md` (sections 2.3, 2.4 R1-R6, 2.5
A24-A27, 3b, 3c, 3d). "Before v1" is the audit's recommendation. Row 21 made those items v1.

| # | Feature | Audit | M11 |
|---|---|---|---|
| C1 | Run SPPS from the project | Planned | **Built** (`run_start`) |
| C2 | TCR | Planned | **Built**: the design's SPPS/TCR choice in the Simulate panel |
| C32 | Pre-run checks | Planned | **Built**: Run's blockers, and the design's "Before running" list, each row with an OK or FAIL text label |
| C33, C34 | Mesh on each run; a dated run folder | Ready | Kept (`run_project` does both) |
| C36 | Progress with elapsed and remaining time | Planned | **Progress and elapsed time built.** A remaining-time estimate is not (backlog 18) |
| C37 | Cancel | Planned | **Built**: `run_cancel` sets the core's `CancelToken`, and `process::run` terminates the Job Object |
| C38 | Solver output in the Console | Planned | **Built**: classified lines |
| C41 | Particle fate per band | Planned | **Built**: the Runs row, from the manifest |
| C39 | Run time in the Runs row | v1.x | **Built**: the task allows timings among the diagnostics |
| R1 | Run list | Planned | **Built**: the Runs tab, from the manifests |
| R6 | A per-run record of inputs | Ready | Shown: the solver and mesh hashes from `run.json` |
| A24 | Updater blocked while a run is active | Planned, conditional | **No updater is shipped in M11** (an open product decision, scope.md). The app keeps one `run_active()` for M13 to check. Nothing else is built |
| A25 | Console with levels and times | Ready | Kept, with the run lines added |
| C7, C10, C11, C25, C27 | Particles, length, step, bands, air | Design only | **Shown read-only, as drawn** (inside `[data-input]`), with the project's true values and labels. Editing them is **PQ3** (section 10): a v1 piece of its own before M12, recorded in scope.md |
| C8, C12, C21, C22, C26 | Particles saved, method, per-band maps, echogram per source, band presets | Core only, or absent | Not built in M11. They are the settings editor of PQ3. C8 and C22 matter to M12's playback and multi-source runs (audit risks 3 and 4) |
| A26, A27, R2-R5, C35, C40 | Console export and clear; label, delete, open and refresh a run; project copy at run time; job list | v1.x | Not in M11. They are listed by ID in the parity matrix, which the backlog does not repeat |

### 1.4 What M11 does not do

- **No acoustic number.** The Results step shows whether a run's results verify, never a value (3.3).
- **Not the settings editor** (PQ3). Not G19 or M37 (row 22). Not the other before-v1 items of M10's
  PLAN 7.2 outside row 22: G7, G8, G11, G18, M6, M20, M22, M40, M41 and M42. They stay tracked in
  the parity matrix and scope.md, untouched here.

---

## 2. The IPC contract

### 2.1 Principles, unchanged from M9 and M10

- Every command is an `async fn` whose body runs through `guard::blocking`. The one exception stays
  M9's `panic_probe_unguarded`.
- Every command is registered in four places, which must agree: the attributes,
  `generate_handler!`, `build.rs`, and `capabilities/default.json`'s `allow-<command>`.
- Schema values travel as JSON text through the core's exact reader.
- **No new plugin and no new permission.** In particular there is no `core:event` permission:
  app-to-UI events travel on `ipc::Channel`s the UI hands over, as M9's run events already do.
- Only `actions.ts` calls `backend`. The M11 commands are called from `actions.ts` only, and a lint
  enforces it.

### 2.2 New commands (9). In `commands.rs`, backed by a new `src/runs.rs` and `AppState` additions

| Command | Rust signature | Returns (TS) | What it does |
|---|---|---|---|
| `run_start` | `pub async fn run_start(state: State<'_, AppState>, solver: String, on_event: Channel<RunStreamBatch>) -> CmdResult<RunStarted>` | `RunStarted` | Checks, under the session lock: a project is open, it has a path, it is not dirty (the UI saved it first, PQ1), no run is active, `run_blockers` is empty, and the solvers are found and verified. Then it spawns the run thread and returns at once. The thread calls `run_project(path, active variant id, MeshChoice::Build { tetgen, preprocess }, opts with verify = Some(manifest), token, sink)`, catches a panic, and streams everything as `RunStreamEvent`s through a `Batcher` (50 ms) into `on_event`. The last batch is `last: true` |
| `run_cancel` | `pub async fn run_cancel(state: State<'_, AppState>) -> CmdResult<bool>` | `boolean` | Sets the active run's `CancelToken`. `true` when a run was active. The kill is the core's: `process::run` terminates the Job Object and returns only once no process of the tree is alive. No PID is looked up and nothing is swept |
| `runs_list` | `pub async fn runs_list(state: State<'_, AppState>) -> CmdResult<RunsView>` | `RunsView` | Every run folder under the project's runs root, `<project folder>/runs` (the CLI's default). A folder is this project's when its `run.json` `source.path`, made absolute against the project's folder, is the open project's path (case-insensitive). Its `run.json` is read with the core's `RunManifest`. A folder named like a run with no `run.json` is **Interrupted**. Other projects' runs are counted, not listed. The active run is `Running`. Rows are numbered 1, 2, ... by start time |
| `run_results` | `pub async fn run_results(state: State<'_, AppState>, run: String) -> CmdResult<ResultsState>` | `ResultsState` | `simpa_core::results::load(<root>/<run>)`, reduced to verified, or refused with its code and message. **It returns no number.** `run` must be a bare folder name of this project's runs (`^\d{8}-\d{6}-\d{3}-(spps\|tcr)(-\d+)?$`). Anything else is `RUN_NOT_FOUND`, so no path can climb out of the root |
| `proj_import` | `pub async fn proj_import(state: State<'_, AppState>, path: String) -> CmdResult<SceneState>` | `SceneState` | `import_proj_file(path)`, then `Session::replace` (unsaved, named after the file). The report goes into `lines` |
| `material_library` | `pub async fn material_library() -> CmdResult<Vec<LibraryMaterial>>` | `LibraryMaterial[]` | `REFERENCE_MATERIALS[1..]`, with each α `f32`-widened (`widen_f32`), exactly as a `.proj` import makes it |
| `solvers_status` | `pub async fn solvers_status(state: State<'_, AppState>) -> CmdResult<SolversStatus>` | `SolversStatus` | Finds `spps.exe`, `classicalTheory.exe`, `tetgen.exe` and `preprocess.exe` with `ExeSearch::from_env(None)`, and checks each with `bed::pe::check_solvers` against `bed::SOLVER_MANIFEST`. Cached by (path, size, mtime) |
| `app_events` | `pub async fn app_events(state: State<'_, AppState>, on_event: Channel<AppEvent>) -> CmdResult<()>` | `null` | The UI registers its app-event channel once at boot. Until it has, a close request is let through |
| `app_quit` | `pub async fn app_quit(app: AppHandle, state: State<'_, AppState>) -> CmdResult<()>` | `null` | The UI's answer to a close request, once it has dealt with the save prompt. Cancels any active run, waits up to 3 s for its thread (and so its `run.json`), then `app.exit(0)` |

**Other changes to existing code:**
- **`StartupInfo`** gains `pid: u32`, which the gate needs for `WM_CLOSE` and Stop-Process.
- **`main.rs`**: the window. `tauri.conf.json` sets the main window to `"create": false`. `setup`
  builds it with `WebviewWindowBuilder::from_config(app, &app.config().app.windows[0])?`, then
  `.focused(!(args.e2e || args.selftest.is_some()))`, then `.build()`. Test windows open visible
  and unfocused, and a normal launch is unchanged. `on_window_event(CloseRequested)` does two things:
  - if the UI channel is registered, it calls `api.prevent_close()` and sends `CloseRequested`;
  - if a second close request arrives within 5 s with no `app_quit` in between (a hung UI), it
    cancels any run and exits.
- **`run_blockers`** (`scene.rs`) loses `M11_PENDING`. `MATERIALS_UNASSIGNED` now also arrives as
  the core's `material_placeholder`, mapped to the same UI code and listed once. The solver
  blockers are app-level, not the project's: `SOLVER_NOT_FOUND` and `SOLVER_UNVERIFIED` come from
  `SolversStatus`, and `RUN_ACTIVE` from the run slot. The UI joins the lists (3.2).

### 2.3 Rust types (`src/runs.rs` and `src/events.rs`; each `Serialize, JsonSchema`, added to `bindings.rs`'s dump)

```rust
pub struct RunStarted { pub solver: String, pub variant: Option<String>, pub project_path: String, pub runs_root: String }

/// One event of a run, in order. `seq` is 0, 1, 2, ... per run, so the UI can prove it lost nothing.
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RunStreamEvent {
    /// The run folder exists (core `RunEvent::Started`, C6). Always the first event.
    Started { seq: u64, t_ms: f64, run: String /* folder name */, folder: String },
    /// A stage begins: geometry | validate | mesh | export | pre_launch | solve | solvers (C7).
    Stage   { seq: u64, t_ms: f64, stage: String },
    /// A classified solver line (core `Classified`), or a TetGen line (`source: mesh`, INFO).
    Line    { seq: u64, t_ms: f64, source: LineSource, stream: Stream, class: RunLineClass,
              rule: String /* the contract's row id, "" for mesh lines */,
              solver_seq: Option<u64> /* Classified::seq */, progress: Option<f64>,
              continuation: bool, text: String },
    /// `run.json` is written. `row` is what `runs_list` would list for it.
    Ended   { seq: u64, t_ms: f64, row: RunRow },
    /// No run folder with its run.json could be made (core `RunError`), or the run thread panicked.
    Failed  { seq: u64, t_ms: f64, error: CmdError /* RUN_PROJECT | RUN_IO | RUN_PANIC */ },
}
pub enum LineSource { Solver, Mesh }                        // "solver" | "mesh"
pub enum RunLineClass { Progress, Info, Ok, Warn, Fail }    // "PROGRESS" | "INFO" | "OK" | "WARN" | "FAIL", the core's classes
pub struct RunStreamBatch { pub batch: u64, pub events: Vec<RunStreamEvent>, pub last: bool }

pub struct RunsView { pub root: String, pub rows: Vec<RunRow>, pub other_projects: usize, pub active: Option<String> }
pub struct RunRow {
    pub run: String,              // folder name, the row's key
    pub number: u32,              // 1-based, by start time among this project's runs
    pub started: Option<String>,  // run.json `started` (RFC 3339)
    pub status: RunStatusUi,      // OK | FAIL | CRASH | CANCELLED | RUNNING | INTERRUPTED
    pub stage: Option<String>,
    pub solver: Option<String>,   // spps | tcr
    pub variant: Option<String>,  // run.json source.variant (an id), None = the base project
    pub reasons: Vec<ReasonUi>,   // the verdict's reasons, in order
    pub warnings: Vec<ReasonUi>,
    pub loss: Option<LossUi>,     // SPPS with its statistics table read
    pub lines: Option<LineCounts>,// run.json `lines`: progress, info, ok, warn, fail, unclassified
    pub exe: Option<FileRefUi>,   // run.json `exe`: path and sha256
    pub mesh_sha256: Option<String>,
    pub solvers: Option<Vec<SolverCheck>>, // run.json `solvers` (C7); None for a CLI or older run
    pub elapsed_s: Option<String>,// outcome.elapsed_ms as seconds, one decimal (below)
    pub exit_code: Option<u32>,
    pub manifest_error: Option<String>, // run.json present but unreadable
}
pub struct ReasonUi { pub code: String /* core */, pub ui_code: String, pub detail: String }
pub struct LossUi { pub worst_pct: String, pub worst_band_hz: i32, pub limit_pct: String, pub bands: Vec<BandLossUi> }
pub struct BandLossUi { pub freq_hz: i32, pub lost: u64, pub total: u32, pub pct: String }
pub struct ResultsState { pub run: String, pub verified: bool, pub refusal: Option<ReasonUi> } // never a number
pub struct LibraryMaterial { pub reference_id: u32, pub name: String, pub absorption: F64, pub color: Rgb }
pub struct SolversStatus { pub checks: Vec<SolverCheck>, pub blockers: Vec<String> }
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AppEvent { CloseRequested { dirty: bool, run_active: bool } }
```

**The numbers are exact and formatted in Rust.** The UI prints strings, never floats it formatted
itself:
- **A band's loss.** `pct` is the loss in hundredths of a percent, `(lost·10000·2 + total) / (2·total)`
  in `u64`, which rounds half up, printed as `x.yz`. `worst_pct` is the largest band's. That is the
  quantity the verdict holds against the limit (`particle_loss_excess`, per band). It is also what
  "Particles lost … / 1 % limit" means on the design (PQ5).
- **The limit.** `limit_pct` is `loss_limit × 100` with no trailing zeros.
- **Elapsed time.** `elapsed_s` is `floor(elapsed_ms / 100 + 0.5) / 10`. The e2e recomputes each of
  these in Node, with BigInt or the same IEEE operations (4.2).

**`Batcher`** (`events.rs`) becomes generic over its event type. M9's `run_events_probe` keeps
`RunEvent` and `RunEventBatch` exactly as they are, so M9's self-test and its check "run events:
batched at 50 ms" do not change.

### 2.4 TypeScript, as the generator will emit it (`ui/src/bindings/ipc.ts`; fields alphabetical)

```ts
export type RunStreamEvent =
  | { kind: 'started'; folder: string; run: string; seq: number; t_ms: number }
  | { kind: 'stage'; seq: number; stage: string; t_ms: number }
  | { kind: 'line'; class: RunLineClass; continuation: boolean; progress?: number | null; rule: string;
      seq: number; solver_seq?: number | null; source: LineSource; stream: Stream; t_ms: number; text: string }
  | { kind: 'ended'; row: RunRow; seq: number; t_ms: number }
  | { kind: 'failed'; error: CmdError; seq: number; t_ms: number };
export type RunLineClass = 'PROGRESS' | 'INFO' | 'OK' | 'WARN' | 'FAIL';
export interface RunStreamBatch { batch: number; events: RunStreamEvent[]; last: boolean; }
export interface RunRow { elapsed_s?: string | null; exe?: FileRefUi | null; exit_code?: number | null;
  lines?: LineCounts | null; loss?: LossUi | null; manifest_error?: string | null; mesh_sha256?: string | null;
  number: number; reasons: ReasonUi[]; run: string; solver?: string | null; solvers?: SolverCheck[] | null;
  stage?: string | null; started?: string | null; status: RunStatusUi; variant?: string | null; warnings: ReasonUi[]; }
export type RunStatusUi = 'OK' | 'FAIL' | 'CRASH' | 'CANCELLED' | 'RUNNING' | 'INTERRUPTED';
export interface ResultsState { refusal?: ReasonUi | null; run: string; verified: boolean; }
export type AppEvent = { kind: 'close_requested'; dirty: boolean; run_active: boolean };
// StartupInfo gains: pid. CheckSummary.enclosed_volume_m3 becomes `number | null` (A5).
```

### 2.5 The event stream

- **One `Batcher` per run, flushed every 50 ms,** as M9 measured it. The run thread pushes
  events from the core's `on_event` callback, which runs inside `process::run`'s line loop, so the
  push is one `mpsc::send` and never blocks on the UI.
- **Every core event becomes one stream event:**
  - `Stage` becomes `stage`;
  - `MeshLine` becomes a `line` with source `mesh`, class INFO;
  - `SolverLine` becomes a `line` with source `solver`, the core's class and row id,
    `solver_seq = Classified::seq`, and `progress` when the row is `progress`;
  - `Started` becomes `started`.
- **When `run_project` returns,** the thread writes `ended` with the row (it reads the `run.json`
  just written, as `runs_list` does), or `failed`, then finishes the batcher.
- **PROGRESS lines are counted, never rendered one by one.** SPPS prints one each time its
  4-significant-digit percentage changes, thousands per run. The UI keeps each run's counts per
  class, its latest percentage, and the set of `solver_seq` values received (3.1).
- **Panics.** The sink never panics: no `unwrap` on a send, and a closed channel is counted in
  `BatchStats.send_failures`. Even so, the run body runs in `catch_unwind`. A panic ends as a `failed`
  event with `RUN_PANIC`, and a folder without `run.json` then lists as Interrupted.

### 2.6 Error codes (`CmdError.code`), besides the guard's `PANIC`, `STATE_POISONED` and `TASK_FAILED`

| Command | Codes |
|---|---|
| `run_start` | `NO_PROJECT`; `SAVE_NO_PATH` (never saved; the UI saves first); `RUN_DIRTY` (unsaved changes; the UI saves first); `RUN_ACTIVE`; `RUN_BLOCKED` (message lists the UI codes); `SOLVER_UNKNOWN` (not `spps` or `tcr`); `SOLVER_NOT_FOUND` (the message lists the paths tried); `SOLVER_UNVERIFIED` (the message names the file and both hashes) |
| `run_cancel` | none |
| `runs_list` | `NO_PROJECT`; `RUNS_IO` (the root exists but cannot be read; a missing root is an empty list) |
| `run_results` | `NO_PROJECT`; `RUN_NOT_FOUND` |
| `proj_import` | `IMPORT_IO`, `IMPORT_*` (the importer's `ImportError`), `PROJ_*` (the `.proj` codes upper-cased, for example `PROJ_VOLUMES_UNSUPPORTED`) |
| `material_library`, `solvers_status`, `app_events`, `app_quit` | none |
| Stream `failed` events | `RUN_PROJECT`, `RUN_IO` (core `RunError`), `RUN_PANIC` |

### 2.7 UI codes for a run's reasons (`runs.rs::run_ui_code`, exhaustive, unit-tested)

| Core code | UI code |
|---|---|
| `tetgen_skipped_facets` | **`MESH_TETGEN_SKIPPED`** (gate (e)) |
| every other mesher code (`mesh::codes::ALL`, added by C8) and `mesh::verify`'s codes, in a run whose stage is `mesh` | `MESH_` + the code upper-cased (`tetgen_self_intersection` → `MESH_TETGEN_SELF_INTERSECTION`; `mesh_invalid` stays `MESH_INVALID`) |
| every validator code (`validate::RULES`, `STRUCTURAL_CODES`) | M10's `scene::ui_code` table: `receiver_outside_volume` → `RECEIVER_OUTSIDE`, ... Plus **`material_placeholder` → `MATERIALS_UNASSIGNED`**, M10's own app code, so the blocker is one code wherever it comes from |
| `verdict::codes::ALL`, the new `solver_unverified`, `results::codes::ALL` | the code upper-cased (`particle_loss_excess` → `PARTICLE_LOSS_EXCESS`, `results_run_failed` → `RESULTS_RUN_FAILED`) |

The UI shows both the UI code and the core code (row 22 (3)). The unit test walks every list above,
and fails on a code with no UI code, or on a stale key in the named map.

### 2.8 `AppState` and the run slot

```rust
pub struct AppState { /* M10's fields */
    pub run: Arc<Mutex<RunSlot>>,                         // the active run, if any
    pub ui_events: Arc<Mutex<Option<Channel<AppEvent>>>>, // registered by app_events
    pub solvers: Arc<Mutex<SolversCache>>,
    pub close: Arc<Mutex<CloseState>>,                    // the first unanswered close request's time
}
pub struct RunSlot { active: Option<ActiveRun> }
struct ActiveRun { run: Option<String>, token: CancelToken, thread: JoinHandle<()>, started: Instant }
```

- **The session lock is held only in `run_start`'s checks.** The run thread reads the project file
  from disk. An edit during a run changes the session, not the running file.
- **Stop-Process needs no code.** The core's Job Object is created with `KILL_ON_JOB_CLOSE`, and its
  only handle is not inheritable (`winproc.rs:5-9`). When `app.exe` dies, the kernel closes that
  handle, and the tree dies with it. M6(f) proved this for `simpa.exe`; gate (d) proves it for
  `app.exe`.

### 2.9 Core changes, each with its unit test (the foundation's; section 6 and 7 hold the test fixes)

| # | Change | Where | Unit test (fails before the change) |
|---|---|---|---|
| C1 | **The mesher's watch thread ends when the mesher panics.** A drop guard stores `done = true` whatever way `mesher.run` leaves, so `std::thread::scope` joins at once and the panic propagates | `crates/simpa-core/src/mesh.rs` `call()` (1234-1263) | `mesh_project.rs`: `a_mesher_that_panics_is_not_held_for_its_time_limit`. A `Fake` whose `act` panics, run under `Timeouts { tetgen: 120 s, .. }` inside `catch_unwind`, must return its panic in under 5 s. Before C1 it takes 120 s |
| C2 | (the test fix of section 6) | `tests/mesh_project.rs` | the test itself, on NTFS and on exFAT |
| C3 | `logs_that_cannot_be_created_are_launch_failed` writes under `$SIMPA_TEST_SCRATCH_ROOT`, else `std::env::temp_dir()`, never `<repo>/target/tmp` | `run/manager.rs` tests | itself, run by `m11.ps1` with no skip |
| C4 | (the `cli_run` fix of section 7) | `crates/simpa/tests/cli_run.rs` | the two tests, with `CARGO_TARGET_DIR` outside the repo |
| C5 | **`material_placeholder`**, a project error. It fires for each surface group whose effective material under the active variant has name `REFERENCE_MATERIALS[0].name` (`Default`) and every absorption and scattering value exactly 0: upstream's placeholder for "no material chosen". One issue per group, at `/surface_groups/<i>/material`. `validate::is_placeholder_material(&Material)` is the one predicate, and `scene.rs::groups_assigned` calls it instead of its own copy. A row goes into `docs/solver-contract.md` Part A, Materials, after `material_unassigned`. `RULES` becomes 43 | `validate.rs`, `validate/project.rs`, `docs/solver-contract.md` | `validate_projects.rs`: (1) the hall imported from `testdata/elmia_corrected.ply` gives 10 `material_placeholder` issues; (2) `tutorial1_box.simpa` and `teaching_room.simpa` give none; (3) a variant that overrides one group to `Default` gives 1, only while it is active; (4) a material named `Default` with α 0.1 is not a placeholder. `run_manager.rs`: that hall, with a source and a receiver, run with the stub solver, is refused at the validate stage with exit class 2 and `material_placeholder`, and the stub is never launched. `validate_contract_docs` and `reason_codes_docs` pass with the new row |
| C6 | **`RunEvent::Started(&Path)`**, sent by `run_project` and `run_folder` right after `create_run_folder`. The CLI's `print_event` gains an arm (it prints nothing new) | `run/manager.rs`, `crates/simpa/src/mesh_run.rs` | `run_manager.rs`: in a run that is OK, one refused at each stage, and one `run_folder`, the first event is `Started`, and its folder is the one holding `run.json` |
| C7 | **Solver verification, as an option.** `RunOptions.verify: Option<SolverManifest>` (None in the CLI, the bed and tests). When `Some`, a new first stage, `Stage::Solvers`, checks the solver exe and, with `MeshChoice::Build`, `tetgen.exe` and `preprocess.exe`, with `bed::pe::check_solvers`. A mismatch is refused `solver_unverified`, and `ExitClass::of(Solvers, Fail)` is `Usage` (2). `RunManifest` gains `solvers: Option<Vec<SolverCheck>>` (`#[serde(default, skip_serializing_if = "Option::is_none")]`), so the CLI's manifests stay byte-identical and old ones still read. `MANIFEST_VERSION` stays 1, since no field changes meaning. A row goes into `docs/solver-contract.md` Part B | `run/manager.rs`, `run/manifest.rs`, `run/verdict.rs`, contract | `run_manager.rs`: (1) a manifest built from the stub's own code sha256 passes, and `run.json` records the checks; (2) one with a wrong sha refuses at stage `solvers` with `solver_unverified`, with no mesh and no launch; (3) `verify: None` writes no `solvers` key. `run_manifest.rs`: a manifest without the key reads as `None`. `exit_classes_follow_the_stage_and_status` gains `Solvers` |
| C8 | `mesh::codes::ALL`, the mesher's codes as a list, so the app's UI-code test can be exhaustive | `mesh.rs` | `reason_codes_docs.rs` is unchanged. A new assertion checks that `ALL` holds every `const` of `mod codes` (the same source walk) |
| C9 | **Solver processes run at `BELOW_NORMAL_PRIORITY_CLASS`** (`winproc` creation flags). The design says "Runs in the background · the app stays usable", and gate (b) measures exactly that. Priority does not change results | `process/winproc.rs` | `process_job.rs`: a child's `GetPriorityClass` is `BELOW_NORMAL_PRIORITY_CLASS` |

Every change keeps the core's public API additive, except for the new `Stage` variant and the
`RunOptions` field. There are 4 constructions of `RunOptions`, all updated: `bed/run.rs`,
`tests/run_manager.rs` and `simpa/src/mesh_run.rs`.

### 2.10 `backend.ts` (foundation)

```ts
runStart:   (solver: 'spps' | 'tcr', onEvent: Channel<RunStreamBatch>) => invoke<RunStarted>('run_start', { solver, on_event: onEvent }),
runCancel:  () => invoke<boolean>('run_cancel'),
runsList:   () => invoke<RunsView>('runs_list'),
runResults: (run: string) => invoke<ResultsState>('run_results', { run }),
projImport: (path: string) => invoke<SceneState>('proj_import', { path }),
materialLibrary: () => invoke<LibraryMaterial[]>('material_library'),
solversStatus:   () => invoke<SolversStatus>('solvers_status'),
appEvents:  (onEvent: Channel<AppEvent>) => invoke<null>('app_events', { on_event: onEvent }),
appQuit:    () => invoke<null>('app_quit'),
```

---

## 3. UI state model and component tree

### 3.1 Stores (`store.ts`, foundation; M9's islands rule and M10's stores kept)

| Store | Type | Written by | Notes |
|---|---|---|---|
| `runStore` | `ActiveRun \| null`: `{ run?: string; solver; variant; stage; progress: number \| null; progressText: string; startedAt; status: 'starting' \| 'running' \| 'cancelling' }` | actions (the stream handler) | `progressText` is the last PROGRESS line's text after its `#`, exactly as SPPS printed it |
| `runLinesStore` | `Map<run, RunLog>`: `{ counts: {PROGRESS, INFO, OK, WARN, FAIL}; seqs: Set<number>; dupes: number; lines: ConsoleLine[] (non-PROGRESS) }` | actions | Counts come only from the `line` events received with source `solver`. The Console renders it, and the e2e reads it (hook) |
| `consoleStore` | `ConsoleLine[]` (M9), gaining `source?: 'app' \| 'solver' \| 'mesh'`, `run?: string` and `tag` including `PROGRESS` | actions | Run lines, except PROGRESS, are appended with their run |
| `runsStore` | `RunsView \| null` | actions | Refreshed on project open, save and import, on `ended`, and when the Runs tab is shown |
| `selectedRunStore` | `string \| null` | dock (a row click), simulate (the "Run n" link) | Defaults to the newest run. The Results step shows it |
| `resultsStore` | `Map<run, ResultsState>` | actions | Fetched when a run is selected on the Results step |
| `solverStore` | `'spps' \| 'tcr'` | simulate | Session state, default SPPS. It is not saved in the project |
| `solversStatusStore` | `SolversStatus \| null` | actions | At boot, and before each run |
| `promptStore` | `{ name: string; resolve: (c: 'save' \| 'discard' \| 'cancel') => void } \| null` | actions (opens it), project (the dialog answers it) | The A9 prompt |
| `libraryStore` | `LibraryMaterial[]` | actions (at boot) | M1 |

### 3.2 Actions and flows (`actions.ts`, foundation: still the only caller of `backend`)

- **`runStart()`** takes three steps:
  1. **The save-first flow (PQ1).** With no path, `saveAs()` opens the native dialog; if it is
     cancelled, nothing runs. With a path and unsaved changes, `save()`. The Console gets
     "Saved <path> before the run".
  2. `solversStatus()`.
  3. `backend.runStart(solverStore, channel)`.

  The channel's handler is the one writer of `runStore`, `runLinesStore` and run lines in
  `consoleStore`. It checks `seq` continuity per run. On `ended` it refreshes `runsStore`, clears
  `runStore`, logs one app line and selects the run.

  The app line is OK for an OK run, FAIL for FAIL or CRASH, and INFO for Cancelled. It reads
  "Run #n · SPPS finished · OK · Particles lost 0.00 % (limit 1 %)", or the reasons' UI codes. Its
  numbers sit in diagnostic spans (4.2).
- **`runCancel()`** sets `runStore.status = 'cancelling'`, then calls `backend.runCancel()`.
- **`refreshRuns()`, `selectRun(run)` and `resultsFor(run)`.**
- **The discard guard.** `newProject`, `openDialog` and `openPath` (mesh, `.simpa`, `.proj`) first
  `await confirmDiscard()`:
  - when `sceneStore.info.dirty`, it opens `promptStore`;
  - `save` runs `save()` (or `saveAs()`), and proceeds only if that saved;
  - `discard` proceeds;
  - `cancel` stops.

  While a run is active, these actions are disabled (PQ4).
- **Close.** At boot, `backend.appEvents(channel)`. On `close_requested`, the flow runs
  `confirmDiscard()`, then `backend.appQuit()` unless the answer is Cancel.
- **`importProj(path)`, `addFromLibrary(entry)`, `setSourceEnabled(id, on)` and
  `setLaw(material, law)`.** The last three go through `apply()` with `ops.ts` builders.
- **`ops.ts` gains** `setSourceEnabled`, `replaceMaterial`, `withLaw(material, law)` and
  `libraryMaterial(entry, id, bandCount)`, each unit-tested (`ops.test.ts`).

### 3.3 Component tree against Concept B

| Region (design line) | Component | File | Owner |
|---|---|---|---|
| menu bar Run button (design:43) | `RunButton`: label "Run SPPS" or "Run TCR" (the design's `runLabel`); while running, "Running <p> %", disabled. `data-part="run"`, and `data-blockers` joins the project's blockers, the solver blockers and `RUN_ACTIVE`. F5 runs it | `chrome/RunButton.tsx` | simulate |
| step bar, Simulate sub (design:617) | "<p> %" while running, else "run <n> <status>" (for example `run 3 OK`), else empty | `chrome/StepBar.tsx` calls `features/simulate/model.ts#simulateSub` | foundation (slot), simulate (function) |
| properties, Simulate step (design:386-426) | `SimulatePanel`, top to bottom. **Head:** "Simulation · Runs in the background · the app stays usable". **Solver choice:** SPPS or TCR radios, `data-solver`, `aria-checked`. **Settings, read-only as drawn,** each value inside `[data-input]`: particles per source, duration, time step, bands, air. **"Before running"**, with a text label OK or FAIL per row: every surface has a material (`MATERIALS_UNASSIGNED`); model closed, no self-intersections (`GEOMETRY_REFUSED`); source inside the room (`SOURCE_*`); all receivers inside (`RECEIVER_*`); air absorption set (`ATMOSPHERE_INVALID`, `ABSATMO_INVALID`); solvers are the verified build (`SOLVER_*`). **Running:** "Meshing…" or "Solving · <p> %"; the bar; elapsed m:ss; **Cancel** (`data-part="cancel-run"`, PQ2); "Solver output is read line by line in the Console tab." **Idle:** "Run <n> · <variant>" and its status as text; "Particles lost <x> % / <l> % limit"; "Solver warnings <n>"; the big Run button | `features/simulate/SimulatePanel.tsx` | simulate |
| properties, Results step (design:428-470) | `ResultsPanel`, for the selected run: the "Run <n> · <variant>" label (`data-run-label`). `data-results-state="verified"`: "Results verified: run.json, inputs and outputs re-checked. Values appear here once the physics checks behind them pass." `"refused"`: "FAIL · Results refused" with its UI and core codes, then the run's reasons' codes. `"none"`: no run. **No number and no `[data-result]` element in M11** | `features/simulate/ResultsPanel.tsx` | simulate |
| dock tabs (design:177-183) | `Dock`. Console badge: "live" during a run, else "<n> fail". Runs badge: the row count | `features/dock/Dock.tsx` | dock |
| Console (design:247-257) | `ConsolePane`. **Lines:** each has its time, a text tag (FAIL, WARN, INFO, OK or PROGRESS; never colour alone), its text, `data-run` and `data-source`. **Per run:** one live PROGRESS line updated in place, and a counts strip `[data-run-counts=<run>]` with `[data-count=PROGRESS\|INFO\|OK\|WARN\|FAIL]`, which prints the counts in `runLinesStore`. **Verbatim.** Solver and TetGen text is shown as it came, marked `data-verbatim`. M10's follow-the-bottom behaviour is kept | `features/dock/ConsolePane.tsx` | dock |
| Runs (design:259-274) | `RunsPane`. **Columns** as drawn: Run (`#n`), Variant, Solver, Status, Check. **Status** is text: `OK`, `FAIL`, `CRASH`, `Cancelled`, `Running`, `Interrupted` (PQ5), with `data-status`. **Check:** "Particles lost <x> %" (worst band) with the limit; each reason as UI code plus core code (`data-reason-code`); "<n> warnings"; solver time "<t> s". **The expanded row adds:** the per-band loss, the exe sha256 (first 12 hex digits, the whole value in the title), whether the solvers were verified (from `solvers`: verified, not verified, or not recorded), the mesh sha256, the line counts, the exit code and the folder. `data-run-row=<run>`. A click selects the run | `features/dock/RunsPane.tsx` | dock |
| status bar (design:474-481) | "Simulating · <p> %" while running (the percentage in a diagnostic span) | `chrome/StatusBar.tsx` | foundation |
| row-22 items (1.2) | Save prompt dialog; File › Open… with `.proj`; Frame model button; source switches; Law column; Add from library; the B-18 Geometry panel | `chrome/SavePrompt.tsx`, `MenuBar.tsx`, `SourcesPanel.tsx`, `ScenePanel.tsx`, `GeometryPanel.tsx`, `features/materials/**`, `features/viewport/**` | project |

**Where the design is silent, and the choice is a product question:** Cancel (PQ2), status words
(PQ5), and what the Results step shows in M11 (the rule forbids numbers, so only its state).

### 3.4 Rules every package follows

M10's PLAN 2.4 rules 1-9 stand. Added:
1. **Diagnostics.** A number next to a unit outside `[data-input]` and `[data-geometry]` goes only
   in a leaf `<span data-diagnostic="<field>" data-run="<run>">`, whose text is exactly one field's
   grammar (4.2). `<field>` is one of:
   - `loss_pct`, `loss_limit_pct` and `elapsed_s`, which also carry `data-band` where per band;
   - `progress_pct`, which carries `data-run` only.

   Only the Runs tab, the Console, the Simulate panel, the Simulate sub, the status bar and the Run
   button may hold one.
2. **Verbatim text.** Solver and TetGen text goes in `data-verbatim="<run>:<source>"` elements,
   unchanged.
3. **The Results step shows no value.**
4. **No package asks for window focus, hides, minimises or moves the window.** The static lint of
   4.4 enforces it.
5. **CSS.** Packages use their own CSS files. `theme.css` stays frozen at M10's blob. `chrome.css`
   belongs to the project package after the foundation moves the Run and dock rules out of it.

### 3.5 Test hooks (on M10's `window.__m10` registry, installed with `--e2e`)

| Hook | Owner | Returns |
|---|---|---|
| `runStart(solver)`, `runCancel()` | foundation | The same actions the buttons call |
| `runState()` | foundation | `runStore` as plain data |
| `waitRun(run \| null, until: 'started' \| 'solve' \| 'progress' \| 'ended', ms)` | foundation | The run's name once the condition holds. `null` means the next run |
| `runLog(run)` | foundation | `{ counts, n: seqs.size, min, max, dupes }` from `runLinesStore` |
| `runsRows()`, `selectRun(run)`, `resultsState(run)` | foundation | |
| `pingStats(n)` | foundation | n sequential `ping` latencies in ms, measured in the page with `performance.now()` around each `invoke` |
| `frameStats.begin()`, `frameStats.end()` | foundation | The intervals between `requestAnimationFrame` callbacks from begin to end |
| `pid()`, `solversStatus()`, `openProj(path)`, `promptOpen()` | foundation | |
| `cameraState()`, `frame()` (M10) | project (viewport) | Kept |

### 3.6 The DOM contract the gate spec reads

The foundation writes the gate specs against these, and the packages make them true:
- `[data-part="run"][data-blockers]`, `[data-solver]`, `[data-part="cancel-run"]`;
- `[data-run-row=<run>][data-status]`, with `[data-part="status"]`, `[data-part="loss"]`,
  `[data-reason-code]`, `[data-part="elapsed"]` and `[data-part="hashes"]`;
- `.console-line.<CLASS>[data-run][data-source]` and `[data-run-counts=<run>] [data-count=<CLASS>]`;
- `[data-props-step="results"] [data-results-state]` and `[data-run-label]`;
- `[data-diagnostic]` and `[data-verbatim]`;
- `[data-prompt] [data-choice="save|discard|cancel"]`;
- `[data-tool="frame"]`, `[data-source-toggle=<id>]`, `[data-law=<material>]`,
  `[data-library-add=<reference id>]`.

---

## 4. The gate

### 4.1 How each of (a)-(e) is met

Every test title starts with its id. `m11.ps1` requires every id to have passed, and none to have
been skipped, with no test failing. The projects are copied into the run's work folder on C:
(`<work>\p\<name>\`) before the e2e, so their runs never land on B:.

| Id | Gate text | Fixture and action | Evidence read | Control (so the check can fail) | Spec |
|---|---|---|---|---|---|
| `m11-a` | (a) the box from the UI: row OK, 'Particles lost 0.00 %', Console counts = manifest | Open `<work>\p\box\box_run.simpa` (section 5). **Click** `[data-part="run"]` (a WebDriver click, not a hook). `waitRun(null, 'ended', 180 s)` | **(1)** The row's `data-status` is `OK` and its status text is `OK`. **(2)** `[data-part="loss"]` reads exactly `Particles lost 0.00 %`. Node recomputes the worst band from `<run>\run.json` with BigInt, and the displayed digits must equal it (and `0.00`). **(3)** Each `[data-count=<C>]` of `[data-run-counts=<run>]`, and `runLog(run).counts`, equal `run.json` `lines.progress, .info, .ok, .warn, .fail`. `runLog.n` equals their total, `min` is 0, `max` is n−1, and `dupes` is 0 | The manifest's counts must not be trivial: `progress > 0` and `ok == 1` (`spps_end_of_calculation`), so an empty Console against an empty manifest cannot pass. Before the run, the Run button's `data-blockers` is empty; on the raw hall it is not | gate (foundation) |
| `m11-b-ipc` | (b) 200 pings p99 < 100 ms during the hall solve | Open `<work>\p\hall\hall_run.simpa`. `runStart('spps')`, then `waitRun(run, 'progress')`: the solve stage has begun, and SPPS has printed a `#` line | `pingStats(200)`. p99 is the nearest rank, the 198th of 200 sorted. It must be under 100 ms. The distribution (min, median, p95, p99, max) is printed | The run must still be `running` after the pings, and the private `spps.exe` must be alive, or the measurement was not "during the solve" | gate |
| `m11-b-frames` | (b) orbit drag frame p95 < 33 ms | Same run, still solving. `frameStats.begin()`, then a real WebDriver pointer action on the canvas: down at the centre, 60 moves of 4 px and 33 ms each, up. Then `frameStats.end()` | p95 (nearest rank) under 33 ms, with n ≥ 50 intervals. The distribution is printed | `cameraState()` differs before and after, so the drag did orbit. The run is still solving | gate |
| `m11-c` | (c) Cancel → 'Cancelled'; 2 s later no spps.exe | Same run. Before: some process runs from `<work>\solvers\spps.exe` (the positive control). **Click** `[data-part="cancel-run"]` at t0 | By t0 + 2 s, no process has `ExecutablePath` equal to `<work>\solvers\spps.exe` (checked with `Get-CimInstance Win32_Process`). The row's `data-status` is `CANCELLED`, its text `Cancelled`. `run.json` has verdict `CANCELLED`, `outcome.cancelled` true and `exit_code` null. The machine-wide `tasklist /FI "IMAGENAME eq spps.exe"` count is printed, not asserted (F8) | The private spps must have been running before the click. `files.present < files.expected`: the solver stopped mid-run | gate |
| `m11-d-close` | (d) close mid-solve → no spps.exe | A fresh session. Open `<work>\p\long\box_long.simpa`. Make one edit, and send `WM_CLOSE` to `pid()`'s main window (`(Get-Process -Id pid).CloseMainWindow()`): **the A9 prompt must appear, and Cancel keeps the app alive.** Undo, save, `runStart`, `waitRun(run, 'progress')`, then `WM_CLOSE` again at t0 | `app.exe` exits within 5 s. By t0 + 2 s no process runs from the private `spps.exe`. `run.json` exists and says `CANCELLED`, because the close path cancelled and waited | The private spps ran before t0. With the project dirty, the first `WM_CLOSE` did **not** close the window, so the close request is intercepted and not ignored | close (foundation) |
| `m11-d-kill` | (d) Stop-Process → no spps.exe | A fresh session: the long box, run, `waitRun(run, 'progress')`, then `Stop-Process -Id pid -Force` at t0 | By t0 + 2 s no process runs from the private `spps.exe`. The folder has no `run.json`: nothing in the app ran after the kill, so only the job's `KILL_ON_JOB_CLOSE` can have ended the solver | The private spps ran before t0 | kill (foundation) |
| `m11-d-after` | (not in the text) the killed run is listed honestly | A fresh session. Open the long box and show the Runs tab | The killed run's row reads `Interrupted`, with its reason "no run.json" | The close run's row reads `Cancelled` | after (foundation) |
| `m11-e-row` | (e) the forced mesh failure: row FAIL, MESH_TETGEN_SKIPPED | Open `<work>\p\meshfail\box_run.simpa`. Its runs folder holds the fixture run (section 5) | The row's `data-status` is `FAIL` and its text is `FAIL`. It has a `[data-reason-code="MESH_TETGEN_SKIPPED"]` whose text also shows `tetgen_skipped_facets`. `run.json` has stage `mesh` and `tetgen_skipped_facets` among its reasons | The OK box run's row in `m11-a` has no reason code | gate |
| `m11-e-results` | (e) the Results step shows no numbers for it | Select that run, then open the Results step | `data-results-state="refused"`, with `RESULTS_RUN_FAILED` shown. `[data-props-step="results"]` has 0 `[data-result]`. Its text with `[data-run-label]` removed has no digit | The same panel on `m11-a`'s run reads `verified`, so the panel does tell the two apart | gate |

**Why the gate uses a private copy of the solvers (F8).** At startup the gate copies the verified
build into `<work>\solvers\`: 4 files, whose code sha256s equal the manifest's. It sets
`SIMPA_SOLVERS_DIR` to that folder for the app. "No spps.exe" then means no process running from
that path. A solver another session runs from `C:\tmp\nm-m8a-solvers` or `target\solvers\bin` is
neither counted nor touched.

### 4.2 Added checks, each able to fail

**`m11-h`: no solver-computed acoustic number, with the diagnostic allowance.** It runs on the box
(after `m11-a`), the hall (after `m11-c`) and the mesh-failure project (after `m11-e`), on every
step and every dock tab. It fails on any of these:
1. **A number next to a unit outside the allowed places.** The page's text, with three things
   hidden, has a match of M10's `ACOUSTIC_NUMBER` (unchanged):
   - `[data-input]` and `[data-geometry]` (as in M10);
   - the diagnostic spans that passed rule 3;
   - the `[data-verbatim]` elements that passed rule 4.
2. **A parameter name followed by a number, anywhere, nothing hidden (MINOR A-2).** It matches
   `\b(T15|T20|T30|EDT|RT|C50|C80|D50|Ts|STI|SPL|LF|LFC|G)\b\s*[:=]?\s*[-+]?\d`. The Acoustics
   panel still has no digit at all.
3. **A diagnostic span that fails its proof.** Every `[data-diagnostic]` must pass all four of these:
   - **(i)** its field is one of `loss_pct`, `loss_limit_pct`, `elapsed_s`, `progress_pct`;
   - **(ii)** it sits inside one of the allowed regions (3.4 rule 1);
   - **(iii)** its text matches the field's grammar exactly: `^\d{1,3}\.\d{2} %$`, `^\d+(\.\d+)? %$`,
     `^\d+\.\d s$`, `^\d{1,3}(\.\d{1,2})? %$`;
   - **(iv)** its value is the manifest's. Node recomputes it from `<run>\run.json` with the rules of
     2.3: the band's or the worst band's loss, the limit, the elapsed time. A `progress_pct` span must
     equal, at its displayed digits, a `#` line of that run's `solver.stdout.txt`, or read `100 %`
     after `spps_end_of_calculation`.
4. **A verbatim element that is not verbatim.** Its text must equal a line of that run's logs:
   `solver.stdout.txt`, `solver.stderr.txt`, `mesh/tetgen.stdout.txt` or `mesh/tetgen.stderr.txt`.

**Why this cannot hide an acoustic number.**
- An exempt number must be the manifest's own diagnostic field, at the display rounding, in a
  diagnostic region. A T30 put in a `data-diagnostic="elapsed_s"` span fails (iv). An exempt line
  must be the solver's own output, and the solvers print no acoustic value (contract Part B's table:
  banners, steps, `#` progress, end, loss warning).
- Rule 2 applies to every element, exempt or not, with no attribute hiding anything.

**Say-NO inside the test,** before the real scan, on a planted DOM that is removed afterwards. The
checker must flag each of these:
- `T30 1.8 s` in the status bar (H1, as in M10);
- `STI 0.62 · D50 0.45` (H2, which passed M10's `m10-h`);
- a `data-diagnostic="elapsed_s"` span reading `1.8 s` for a run whose manifest says otherwise;
- a `loss_pct` span inside the Acoustics panel;
- a `data-verbatim` line that is not in the logs.

If it fails to flag any one, the test fails. `m10-h` gains rule 2 too (tightening, not loosening),
so M10's own H2 mutation now fails it.

**Row 22, one id each** (project spec, except `m11-r22-default`):

| Id | Action | Evidence | Control |
|---|---|---|---|
| `m11-r22-a9` | Teaching room: one edit (dirty), then File › New project | `[data-prompt]` shows the project's name. **Cancel**: the project, `dirty` and `undoDepth` are unchanged. **Don't save**: a new project, and the file on disk is unchanged (sha256). Again with **Save**: the file changed, then the new project | With the project clean, New shows no prompt. The same prompt comes from File › Open… (through the `openPath` hook) |
| `m11-r22-a3` | `openProj(<upstream>\tutorial 1\tutorial_1.proj)`, then `saveAs(<work>\t1_app.simpa)`. The gate has run `simpa import-proj` on the same file into `<work>\t1_cli.simpa` | The two files are byte-identical (the import is deterministic, receipts). The step subs show the `.proj`'s group and source counts | `openProj(<work>\bad.proj)` (8 bytes, not a zip) gives a FAIL line with its code, and `projectJson()` is unchanged |
| `m11-r22-g42` | Box: `cameraState()` after load = S0. Orbit with a real pointer drag: S1 ≠ S0. Click `[data-tool="frame"]` | The state equals S0 within 1e-9 m, the tolerance M10 measured for OrbitControls | Without the click, S1 stays |
| `m11-r22-m26` | Teaching room: switch S1 off | Refused: `[data-issue-code="SOURCE_NONE"]` is shown, and the project is unchanged | Add S2 and switch S1 off: applied. `projectJson` has `enabled: false`, and the list shows "off". Undo restores it |
| `m11-r22-m5` | Rear wall's material: law Lambert | `projectJson`'s material has `reflection_law` Lambert (`All`). `undoDepth` went up by one | Undo gives back the original bytes |
| `m11-r22-m1` | Add from library "30% absorbing", then assign it to Floor | The new material's absorption in all 6 bands is exactly `material_library()`'s value for id 21. That is the core's `widen_f32(0.3)`, compared as the JSON number text | Its name, colour and law equal the core's entry. The Materials sub stays `6 / 6` |
| `m11-r22-default` | Import `testdata/elmia_corrected.ply` (m, z); place a source and a receiver through the hooks; save as `<work>\ph.simpa` | The Run button's `data-blockers` is exactly `MATERIALS_UNASSIGNED`, now held by the core's rule: the issues list has `material_placeholder`. **The core itself refuses:** the gate runs `simpa run <work>\ph.simpa --solver spps --json`, which exits 2 with `run.json` stage `validate` and reason `material_placeholder`, and no `solve/` output | Assign a material to all 10 groups: the blockers are empty, and `simpa validate` has no `material_placeholder` |

**`m11-b18` (MINOR B-18), project spec.** Import the raw hall. The Geometry panel's volume row holds
no digit, and reads that a refused model encloses no volume. The three dimensions share one number
of decimals. Control: the corrected hall shows its volume (10,389.096 m³ by `simpa check`) at that
precision, inside `[data-geometry]`, from `CheckSummary.enclosed_volume_m3 = Some(..)`.

**`m11-focus`: the test windows are visible and never take keyboard focus.** It is two static
checks and one runtime watcher, over every window the gate opens: M11's specs, M10's specs through
`m10.ps1`, and M9's `--selftest`.

1. **Static lint** (fails on a hit):
   - `app/src-tauri/src/**` has no `set_focus`, `SetForegroundWindow`, `SetFocus(`,
     `BringWindowToTop`, `SwitchToThisWindow`, `AllowSetForegroundWindow`, `.focused(true)`,
     `always_on_top`, `.minimize(`, `.hide(` or `visible(false)`;
   - the window built in `setup` uses `.focused(!(e2e || selftest))`;
   - `app/ui/src/**` has no `setFocus(`, `.minimize(`, `.hide(`, `setAlwaysOnTop`, `setPosition(`
     or `setVisible(false)`;
   - `app/e2e/**` has no `setWindowSize`, `setWindowRect`, `maximizeWindow`, `minimizeWindow`,
     `fullscreenWindow`, `switchWindow`, `switchToWindow` or `closeWindow`;
   - `tauri.conf.json` has no window with `visible: false`, `focus: true` or `alwaysOnTop`.
2. **The runtime watcher,** `tools/gates/focus-watch.ps1`, started before the first test window
   and stopped after the last. It is C# through `Add-Type`, on its own thread with a message loop:
   - `SetWinEventHook(EVENT_SYSTEM_FOREGROUND)`, out of context, so **every** foreground change
     arrives, with its time: there is no sampling gap;
   - low-level mouse and keyboard hooks (`WH_MOUSE_LL`, `WH_KEYBOARD_LL`) record the **time**, the
     injected flag, and for a button-down the screen point of each real input. **No key code is
     recorded.** WebDriver's events are CDP events inside the renderer, so they never pass through
     these hooks. What the hooks see is a person;
   - every 250 ms, each top-level window of an `app.exe` in the gate's process tree is sampled:
     `IsWindowVisible`, `IsIconic`, and `MonitorFromWindow(.., MONITOR_DEFAULTTONULL)`.

   It writes JSON lines into `<work>\focus.jsonl`.
3. **The judge** (`app/e2e/lib/focus.ts`, pure, with a `node --test` suite) fails when either holds:
   - **A foreground change to a window of a process descended from the gate** (`app.exe`,
     `msedgewebview2.exe`, `msedgedriver`, `tauri-driver`, `node` or a console host) with **no real
     input in the 1,000 ms before it**. A real button-down inside that window's rectangle, or a real
     key, within 1,000 ms excuses it: that is Burhan clicking or Alt+Tabbing to it himself. Each
     excused change is printed;
   - **A gate `app.exe` window, once first seen visible, sampled invisible, minimised, or on no
     monitor, or no app window ever seen visible in a session.**

   The suite feeds synthetic logs, and each must get its verdict: a steal with no input fails; a
   click inside the window 200 ms before passes; a click elsewhere 200 ms before fails; a key 200 ms
   before passes; a minimised sample fails.

**What `m11-focus` proves:**
- no test window took the foreground without a person's input just before it;
- the app's code asks for no focus;
- every test window was on screen, visible and not minimised at every 250 ms sample.

**What it cannot prove:**
- **A steal within 1 s of Burhan's own key press** (anywhere) is excused. A key cannot be placed on
  the screen, so this is a false-negative window.
- **A hide or minimise shorter than 250 ms** between samples.
- **That the watcher was live in a run where no foreground change happened at all.** The judge's
  logic is proven by its suite. The live proof is opt-in: `m11.ps1 -FocusSayNo` opens one small
  window of the gate's own that calls `Activate()`, and requires the judge to fail on it. It
  deliberately steals focus once, so it never runs by default.

**Prior gates.** `m11-prior-m9`: `m9.ps1 -TargetDir C:\tmp\nm-target` in full, exit 0 and
"M9 PASSED". `m11-prior-m10`: `m10.ps1 -SkipCore`, exit 0. Its core step is a subset of M11's core
step, which runs the same targets plus the two it skipped. At integration, `m10.ps1` also runs
bare, in full, once, and GATE.md records "M10 PASSED".

### 4.3 The e2e harness

- **`app/e2e/m11.conf.ts`** has M10's shape, with `M11_*` variables and specs `m11.<name>.e2e.ts`.
  `m10.conf.ts` stays as it is.
- **Specs**, in this order:
  - foundation: `smoke`, `gate` (a, b, c, e, h, r22-default), `close` (d-close), `kill` (d-kill),
    `after` (d-after);
  - packages: `simulate`, `dock`, `project`.

  Each spec file is its own session and app launch. Close and kill end their app on purpose, and
  their `afterSession` tolerates a dead session.
- **`app/e2e/lib/`** (foundation):
  - `runs.ts`: read `run.json`; the BigInt loss and elapsed formulas; the log lines;
  - `procs.ts`: through `powershell -NoProfile -Command`: processes running from a path;
    `CloseMainWindow`; `Stop-Process`;
  - `acoustic.ts`: the `m11-h` checker. It is a pure function over a DOM snapshot, run in the page,
    and its grammar and proof helpers have a `node --test` suite;
  - `focus.ts`: the judge;
  - `emulation.ts`: the F9 fallback (below).
- **Page focus (F9).**
  - **Measure first.** The foundation's first smoke runs M10's full spec set against the new
    unfocused window.
  - **If a spec fails only on page focus,** `emulation.ts` switches on CDP
    `Emulation.setFocusEmulationEnabled` at the start of every session. It goes through the
    driver's CDP endpoint, `POST /session/:id/ms/cdp/execute` (msedgedriver), else chromium's
    `send_command_and_get_result`, and tauri-driver proxies it. It changes only what the page
    believes, never OS focus.
  - **If neither endpoint works,** the foundation stops and reports. The alternative, activating the
    window, is exactly what Burhan asked us not to do.
- **Occlusion.** WebView2 renders while its host is covered, since the host controls visibility.
  The first smoke confirms this with the app window behind another window: `m11-b-frames` and
  `idle()` must still run. If the page is throttled when covered, that finding stops the foundation.
  It is reported to Burhan with the one fix that does not steal focus: e2e-only `always_on_top` of
  the test window.

### 4.4 `tools/gates/m11.ps1` (Windows PowerShell 5.1, M10's shape)

**Parameters:**
- `-TargetDir C:\tmp\nm-target`;
- `-E2eHome C:\tmp\nm-e2e`;
- `-Only all|static|e2e`;
- `-Spec smoke,gate,close,kill,after,simulate,dock,project`;
- `-SolversDir C:\tmp\nm-m8a-solvers`;
- `-SkipCore`, `-SkipPrior`, `-FocusSayNo`, `-FetchDriver`.

A partial run never prints "M11 PASSED". The work folder is `<TargetDir>\gates\m11\<stamp>`.

1. **Static:**
   - `m10.ps1 -Only static` (it runs M9's static checks);
   - the command inventory, 37: the same set in the four places;
   - lints:
     - `backend.` only in `actions.ts`, `selftest.ts` and `App.tsx`;
     - the run commands only in `actions.ts`;
     - the focus lint (4.2);
     - no PID-based kill in `app/src-tauri` (`taskkill`, `TerminateProcess`, `OpenProcess` with
       terminate, `sysinfo`);
     - `theme.css` still has M10's frozen blob;
   - `npm run typecheck`, and `npm test` (all `node --test` suites, the harness libs' included).
2. **Rust:**
   - `cargo test -p app`, clippy `-D warnings`, `fmt --check`;
   - `cargo test -p simpa-core --test ui_fixtures`;
   - unless `-SkipCore`, the core crates' tests with `RUST_TEST_THREADS=4` (so no more than 4 test
     solver processes run at once), scratch on C:, `SIMPA_SOLVERS_DIR = -SolversDir`, and **no
     staging** of `<target>\target\solvers\bin`, which section 7 makes unnecessary. **Not run** are
     only the targets that write under `<repo>\target`, printed on every run (backlog 6). The hang
     and the `run::manager` unit test are no longer skipped.
3. **Build:** under the e2e lock, `npx tauri build --no-bundle` (custom protocol), as in M10.
4. **Prerequisites:**
   - tauri-driver, the msedgedriver of the live WebView2 `pv`, and the WebdriverIO install, as in M10;
   - `<work>\solvers\` staged (4 exe) and checked against the manifest by code sha256;
   - the projects copied into `<work>\p\`;
   - **the mesh-failure fixture run** (section 5), which must exit 4 with `tetgen_skipped_facets`
     among its reasons, or the check fails before the e2e;
   - the raw hall and upstream's `tutorial_1.proj` present.
5. **Focus watcher** started.
6. **e2e:** `wdio run app/e2e/m11.conf.ts`. The verdict parses the junit files, and every required id
   must pass: `m11-smoke`, `m11-a`, `m11-b-ipc`, `m11-b-frames`, `m11-c`, `m11-d-close`,
   `m11-d-kill`, `m11-d-after`, `m11-e-row`, `m11-e-results`, `m11-h`, `m11-r22-a9`, `m11-r22-a3`,
   `m11-r22-g42`, `m11-r22-m26`, `m11-r22-m5`, `m11-r22-m1`, `m11-r22-default`, `m11-b18`. There
   must be 0 failures and 0 skips. The line is named "e2e: wdio ran (verdict below)" (MINOR A-4).
   Then the lock is released.
7. **Prior gates** (unless `-SkipPrior`): `m10.ps1 -SkipCore`, then `m9.ps1`. Both still run under
   the watcher.
8. **Focus:** the watcher stopped; `m11-focus` judged; the excused changes printed.
9. **File counts:** every file under the work folder (C:), and every file this run left on B: (the
   gate writes none there).

### 4.5 Changes to existing gates. None loosens a criterion

| File | Change | Why |
|---|---|---|
| `tools/gates/m10.ps1:137` | `$attrs.Count -eq 28` becomes "the 28 commands M10 named are all present, and the four sets are equal". The M10 list is written in the script | The count was a snapshot. The substance, every command registered in all four places, is unchanged, and M10's commands must still exist |
| `app/e2e/specs/m10.scene.e2e.ts:103` | The teaching room's control changes from exactly `M11_PENDING` to empty blockers **and** Run enabled | The control existed to show "disabled" is not vacuous. With Run wired, "enabled on a clean project" is the stronger form |
| `m10.ps1` core step | The skip of `every_failure_code_fires_on_its_input` and of `logs_that_cannot_be_created_are_launch_failed` is removed | Both are fixed (section 6, C3) |
| `m10.ps1` e2e step | "PASS e2e: wdio run" becomes "e2e: wdio ran (verdict below)", and it passes only on exit 0 | MINOR A-4 |
| `app/e2e/specs/m10.shell.e2e.ts` (`m10-h`) | Adds rule 2 of 4.2, the parameter name followed by a number | MINOR A-2. A tightening |
| `m9.ps1` | none | M9's selftest window becomes unfocused through `main.rs`, and its checks read nothing about focus |

---

## 5. Fixtures (produced by the core; `tests/fixtures/ui/`, `tests/fixtures/** -text` keeps the bytes)

The recipe is `crates/simpa-core/tests/ui_fixtures.rs` (M10's), extended.
`SIMPA_WRITE_FIXTURES=1` writes the files; otherwise every committed file must equal its recipe's
output. **4 new files on B:,** plus PROVENANCE.md updated.

| File | Recipe | Asserted by the recipe |
|---|---|---|
| `box_run.simpa` ("the box" of gate (a)) | `teaching_room.simpa` with SPPS `random_seed` set to a fixed value, so the run is reproducible single-threaded. **The seed is chosen by measurement:** the foundation runs the box through `simpa run` with the verified solvers, one seed at a time (≤ 4 solver processes), and takes the first whose statistics table shows 0 lost particles in all 6 bands. PROVENANCE records every seed tried, with its per-band counts. The particle count stays 150,000 unless the run takes over 60 s, in which case it is lowered and the change recorded | M4 check ok; `validate()` 0 errors and 0 warnings; `run_blockers` empty. The gate (not the recipe) checks the loss, since a recipe cannot run a solver: if a future solver change loses a particle, `m11-a` fails with the numbers, which is right |
| `box_long.simpa` (d) | `box_run.simpa` with `particles_per_source` raised until the solve lasts at least 60 s (measured; recorded) | The same checks |
| `hall_run.simpa` (b, c) | Imports **`testdata/elmia_corrected.ply`** with the core importer (m, z). Every group gets reference material id 22, "20% absorbing", added as the library adds it. S1 and R1-R2 are taken from `tests/fixtures/rooms/elmia_corrected.simpa`'s first source and first two receivers (read with `schema::load`). SPPS energetic, 1,000,000 particles, the 6 octave bands, fixed seed. It must still be solving 60 s after its first `#` line; the gate always cancels it | M4 check ok (7,860 faces, 0 open edges); `validate()` 0 errors; `run_blockers` empty. The PLY's wrong grouping (CLAUDE.md) does not matter here: the run tests the app's responsiveness and cancel, not the physics |
| `tetgen_skips.bat` (e) | A stand-in `tetgen.exe`. It writes `scene_mesh_skipped.face` (`2 1` / `1 1 2 3 8` / `2 1 3 4 9`, the rows `mesh_project.rs`'s `Fake` writes) into its working folder and exits 3, as TetGen 1.6.0 does when it skips self-intersecting facets. Committed with CRLF line ends | The recipe checks its bytes |

**The mesh-failure run (gate (e)) is made at gate time by the core's own run manager:**
`simpa run <work>\p\meshfail\box_run.simpa --solver spps --tetgen tests\fixtures\ui\tetgen_skips.bat --runs <work>\p\meshfail\runs --json`.

**What the core does with it.** The real mesher writes the `.poly` and the `.var`, runs the stand-in
(and its `-d` follow-up in `diag/`), reads the `_skipped.face`, and maps markers 8 and 9 to the
box's Walls. It writes `mesh.json` and the run's `run.json`: stage `mesh`, status FAIL, exit class
4, with reasons `tetgen_exit_nonzero`, `tetgen_skipped_facets`, `tetgen_output_missing` and
`neigh_missing`. The gate asserts these before the e2e starts.

**Why a stand-in, and why that is honest.** TetGen 1.5.0 never writes `_skipped.face`, and the
geometry check keeps self-intersecting input from ever reaching TetGen. So the only way a Runs row
can show `MESH_TETGEN_SKIPPED` is through a mesher that skips facets. The fault is in the tool, as
in the core's own mesher tests. Everything the app shows is read from the core's real records.

- **Why the CLI and not the app.** The app verifies every solver before a run (C7), and would
  refuse the stand-in: that is the point of C7.
- **Why that meets gate (e).** Its text asks for a Runs row and a Results step, not for a launch
  from the UI.

**Also made at gate time:**
- `<work>\t1_cli.simpa`, by `simpa import-proj` (for `m11-r22-a3`);
- `<work>\bad.proj`, 8 bytes;
- **the Interrupted control,** an empty run folder `20260101-000000-000-spps\solve\` in the long box's
  runs root, for the dock spec.

---

## 6. The hanging test: `mesh_project::every_failure_code_fires_on_its_input`

**Measured today** (receipts): it hangs. It was killed at 600 s.

**Cause, in two parts.**
1. **The trigger is the test's.** It makes `scene_mesh.var` read-only and expects `delete_stale` to
   fail on it (`STALE_DELETE_FAILED`). On NTFS, with this toolchain (rustc 1.98.1),
   `std::fs::remove_file` deletes a read-only file. The folder shows it replaced. On B:'s exFAT the
   deletion still fails, which is why the test passed where it was written. The mesher therefore
   goes on and calls the `never` fake, which panics.
2. **The hang is the core's.** In `mesh.rs::call()`, the panic leaves `mesher.run(..)` before
   `done.store(true)` (1262). The watch thread keeps looping until the caller cancels or the
   program's time limit passes: TetGen 3,600 s, `preprocess.exe` 600 s. `std::thread::scope` must
   join it before the panic can propagate.

**Is it reachable from a user's Run? The trigger is not. The mechanism is.**
- **The path.** Every app Run meshes through `run_project`, `mesh_project_with` and this `call()`.
  `mesher.run` is `process::run` with a line callback that ends in the caller's `on_event`, and in
  M11 that is app code: the event sink.
- **A panic there,** or in `process::run`, blocks the run thread for up to an hour. The Simulate
  step would show "Meshing…" with nothing moving. A Cancel would release it, because the watch
  thread checks the caller's token.
- **Verdict.** Test-triggered, but a latent defect on M11's Run path, which M11 is the first to
  expose. **The foundation fixes both:**
  - **C1 (core):** a drop guard sets `done` on every exit from `call()`'s body, with the unit test of
    2.9;
  - **C2 (test):** the stale file is held open with `OpenOptionsExt::share_mode(0)` (no
    `FILE_SHARE_DELETE`), so the deletion fails with a sharing violation on NTFS and on exFAT alike.
    The read-only attribute is dropped: it no longer proves anything on NTFS. On non-Windows the
    test keeps the read-only file.

**Gate (e)'s mesh failure does not pass through the panic.** The stand-in exits 3 normally. But the
core suite, which the gate requires, now runs the whole of `mesh_project` on C: with no skip.

## 7. The `cli_run` tests with `CARGO_TARGET_DIR` outside the repo

- **What fails.** Only
  `a_passing_test_leaves_no_scratch_behind_and_a_failing_one_keeps_its_folder` (M10 GATE.md, item
  2; FOUNDATION.md's second name was the child run's output, caught by a grep).
- **Why.** Its failing arm points `$SIMPA_SOLVERS_DIR` at an empty folder, yet expects the run to get
  as far as the preprocess lookup. That needs `classicalTheory.exe` and `tetgen.exe` from the CLI's
  last fallback, `<ancestor of simpa.exe>\target\solvers\bin`. When cargo builds into
  `C:\tmp\nm-target`, that folder does not exist, and M10's gate staged a copy there to make the
  test pass.
- **The fix (C4).** The failing arm makes `<outer>\no-preprocess\` with **copies of
  `classicalTheory.exe` and `tetgen.exe` only**, from `solver_exe()`, which is `$SIMPA_SOLVERS_DIR`.
  It points `$SIMPA_SOLVERS_DIR` there. The fault stays the one the test names, a build without
  `preprocess.exe`, and the dev-tree fallback is no longer needed. The folder is inside the test's
  own scratch and removed with it.
- **The other test.** `a_run_whose_preprocess_gives_up_records_the_warning` takes its solvers from
  `$SIMPA_SOLVERS_DIR` through `solver_exe()`, and is checked to pass alone with the target on C:.
- **The proof.** `m11.ps1` runs the core crates **without** staging `<target>\target\solvers\bin`
  (4.4 step 2). M10's staging stays in `m10.ps1`, harmless.

## 8. M10's MINOR.md: where each item goes

The criterion: an item goes into M11 if it lets a wrong or misleading number reach a user, or it
breaks a gate's honesty. Anything else goes to `docs/v1.1-backlog.md` in this commit, with its
receipt and its "done when". MINOR.md now points to each.

| Item | Where | Why |
|---|---|---|
| A-2, `m10-h` blind to unitless numbers | **M11, foundation** (`m10-h` and `m11-h`, rule 2) | Gate honesty. STI is in v1, and M11 puts numbers on screen |
| A-4, "PASS e2e: wdio run" on exit 1 | **M11, foundation** (`m10.ps1` and `m11.ps1`) | Gate honesty: a PASS line that is not one |
| A-3, the wall control in `m10-d` | Kept, no action | A note, not a defect |
| A-5, invisible key constants in the screens spec | Backlog 7 | Not a gate spec. No number |
| B-18, "Volume 0 m³" on the refused hall, and mixed precision | **M11: foundation** (`enclosed_volume_m3: Option<f64>`, `None` when refused) **and project** (Geometry panel); check `m11-b18` | A wrong number reaching a user |
| B-2, B-5, B-6, B-9, B-11, B-17, B-23, B-24 | Backlog 8-13, 15, 16 | Design fidelity. No wrong number |
| B-20, the Materials panel layout | Backlog 14, Burhan's to decide | M11's law column and library button go inside the existing grid and panel, and do not decide it |
| B-25, the scrollbar hover | Backlog 17, Burhan's to decide | `theme.css` stays frozen in M11 |

---

## 9. Packages

### 9.0 The foundation: sequential, before the packages. It owns every shared file

**Order, each step committed:**
1. **Core C1-C9,** with their tests and docs rows. Then the core crates' tests, on C:, with no skip
   but the `<repo>\target` writers.
2. **The fixtures (section 5):**
   - measure the box seed and the long box's particles;
   - the recipes;
   - PROVENANCE.md.
3. **App Rust:**
   - `main.rs` (window and close), `commands.rs` (9 commands), `runs.rs`, `events.rs` (generic
     `Batcher`), `bridge.rs`, `scene.rs`, `bindings.rs`, `build.rs`,
     `capabilities/default.json`, `tauri.conf.json`;
   - unit tests:
     - the UI-code table exhaustive (2.7);
     - `runs_list` on the fixture folders: OK, FAIL, CANCELLED, Interrupted, other project's;
     - the loss and elapsed formulas on edge values, including exact halves (1 lost of 800);
     - `run_results` refusing a path outside the root;
     - the stream order on a stub-solver run: `started` first, `solver_seq` 0..n−1, `ended` last;
     - `material_library` equal to `REFERENCE_MATERIALS[1..]` widened.
4. **UI shared:**
   - `store.ts`, `actions.ts`, `ops.ts` (and its test), `backend.ts`, `testhooks.ts`, `bindings/**`;
   - `App.tsx`: boot (`appEvents`, `solversStatus`, `materialLibrary`) and the keys F5 (Run) and
     Home (Frame model);
   - `chrome/StepBar.tsx`, `StatusBar.tsx`, `PropertiesPanel.tsx`, `sceneModel.ts` (and its test,
     M11_PENDING removed).
5. **Moves and stubs,** each moved unchanged and compiling:
   - `chrome/Dock.tsx`, `ConsolePane.tsx` and `RunsPane.tsx` go to `features/dock/`, with their rules
     from `chrome.css` into `features/dock/dock.css`;
   - the Run button's rules go into `features/simulate/simulate.css`;
   - stubs: `features/simulate/SimulatePanel.tsx`, `ResultsPanel.tsx`, and `model.ts` (whose
     `simulateSub` returns `''`);
   - `chrome/SavePrompt.tsx`: a minimal working dialog, which the project package then owns.
6. **The harness and gates:**
   - `app/e2e/m11.conf.ts` and `lib/**`;
   - the specs `m11.{smoke,gate,close,kill,after}.e2e.ts`;
   - spec shells `m11.{simulate,dock,project}.e2e.ts`, each with one failing `it` per owned id;
   - `tools/gates/m11.ps1` and `focus-watch.ps1`;
   - the `m10.ps1`, `m10.scene` and `m10.shell` changes of 4.5.
7. **The first smoke** (F9 and occlusion): `m10.ps1 -SkipCore` under the watcher, with the app
   window behind another window. Stop and report if focus is still taken, or the page throttles.

**Done when:**
- the core crates' tests pass, with the two former skips run;
- `m9.ps1` passes in full;
- `m10.ps1 -SkipCore` passes;
- `m11.ps1 -Spec smoke,gate,close,kill,after` passes every foundation-only id: `m11-smoke`,
  `m11-b-ipc`, `m11-b-frames`, `m11-d-close`, `m11-d-kill` and `m11-r22-default`, and the
  `m11-focus` judgement over them (`m11-d-after` reads the dock's Runs row, so it waits for the dock);
- every package-owned id fails as pending, and nothing else fails;
- the bindings regenerate clean.

### 9.1 Package `simulate`

- **Owns:**
  - `app/ui/src/features/simulate/**` (`SimulatePanel.tsx`, `ResultsPanel.tsx`, `model.ts` and its
    `model.test.ts`, `simulate.css`);
  - `app/ui/src/chrome/RunButton.tsx`;
  - `app/e2e/specs/m11.simulate.e2e.ts`.
- **Builds:** the Simulate step, the Run button and the Results step, all as in 3.3.
- **Pure logic in `model.ts`:** the simulate sub, the Run label, the "Before running" rows from
  blockers and issues, and the elapsed m:ss.
- **Extra checks in its spec:**
  - the radio switches the label to "Run TCR", and a TCR run of the box ends OK with no loss line;
  - the "Before running" rows read FAIL on the placeholder hall and OK on the box;
  - progress shows "Solving · <p> %" during the hall run;
  - Run is disabled with `RUN_ACTIVE` during a run;
  - the "Run <n>" link selects the run on the Results step.
- **The gate's DOM it makes true:**
  - `m11-a` (the Run click);
  - `m11-c` (the Cancel click);
  - `m11-e-results`;
  - its share of `m11-h`.
- **Done when:**
  - its spec passes;
  - `m11.ps1 -Only e2e -Spec gate` passes `m11-e-results`, with `m11-a` and `m11-c` failing only on
    the dock's part, if the dock is not in yet;
  - its unit tests pass;
  - there are no type errors in its files.

### 9.2 Package `dock`

- **Owns:**
  - `app/ui/src/features/dock/**` (`Dock.tsx`, `ConsolePane.tsx`, `RunsPane.tsx`, `dock.css`, and
    any pure modules with tests);
  - `app/e2e/specs/m11.dock.e2e.ts`.
- **Builds:** the dock tabs and badges, the Console and the Runs tab, all as in 3.3.
- **Console rendering stays bounded:** PROGRESS lines are counted, not rendered.
- **Extra checks in its spec:**
  - every per-band value in an expanded row equals the manifest's (BigInt);
  - the hash shown is the manifest's `exe.sha256` prefix, and the verified mark follows `solvers`;
  - the Interrupted control folder lists as Interrupted;
  - "<n> fail" and "live" badges;
  - the Console stays at the bottom during a run, and stays put when scrolled up.
- **The gate's DOM it makes true:**
  - `m11-a` (the row and the counts);
  - `m11-c` (the row "Cancelled");
  - `m11-d-after`;
  - `m11-e-row`;
  - its share of `m11-h`.
- **Done when:**
  - its spec passes;
  - `-Spec gate` passes `m11-e-row`, with `m11-a` failing only on the simulate part, if simulate is
    not in yet;
  - its unit tests pass;
  - there are no type errors in its files.

### 9.3 Package `project`

- **Owns:**
  - `app/ui/src/chrome/MenuBar.tsx`, `SourcesPanel.tsx`, `ScenePanel.tsx`, `GeometryPanel.tsx`,
    `ImportDialog.tsx`, `SavePrompt.tsx` and `chrome.css`;
  - `app/ui/src/features/materials/**` and `app/ui/src/features/viewport/**`;
  - `app/e2e/specs/m11.project.e2e.ts`.
- **Builds:**
  - the row-22 UI of 1.2: the prompt dialog, `.proj` in File › Open…, the Frame button, the source
    switches, the Law column, Add from library;
  - B-18 in the Geometry panel.
- **Extra checks:** as in the 4.2 table.
- **It must keep M10's viewport, materials and scene specs passing without editing them.** They are
  M10's gate.
- **Done when:**
  - `m11-r22-a9`, `m11-r22-a3`, `m11-r22-g42`, `m11-r22-m26`, `m11-r22-m5`, `m11-r22-m1` and
    `m11-b18` pass;
  - `m10.ps1 -SkipCore` passes;
  - its unit tests pass;
  - there are no type errors in its files.

### 9.4 Rules for building in parallel

- **M10's PLAN 6.4 rules stand:**
  - no package edits a file it does not own; a missing shared piece is asked of the foundation,
    not patched around;
  - every write leaves the file compiling;
  - every build and e2e run goes through `m11.ps1 -Only e2e -Spec <pkg>`, which holds
    `C:\tmp\nm-e2e\e2e.lock`;
  - nothing but the gates writes into `C:\tmp\nm-target`;
  - no new npm dependency.
- **Packages run only their own spec plus `gate`,** never the full gate, and never `m9.ps1`. Its
  build does not take the lock.
- **Integration**, after all three:
  - `m11.ps1` in full, and `m10.ps1` bare in full;
  - a critic;
  - decision-log rows for section 10's calls; scope.md; `rebuild-plan.md`;
  - GATE.md;
  - a session summary before the push.

---

## 10. Calls made here, and product questions

**Technical calls** (Jarvis, row 13; recorded in the decision log at the merge):

| # | Call | Why |
|---|---|---|
| T1 | The runs root is `<project folder>/runs`, the CLI's default. A run belongs to a project by its `run.json` `source.path` | CLI runs and app runs of one project show together. Several projects may share a folder |
| T2 | `RunEvent::Started` in core (C6) | The UI needs the run folder's name at once (F2). There is no other honest source |
| T3 | Solver verification in core, off by default, always on in the app (C7), and recorded in `run.json` | Row 17 (5). The Runs tab's "solver hashes" mean something only if the check is part of the run's own record |
| T4 | Solvers run at below-normal priority (C9) | "The app stays usable" (the design, gate (b)). Results do not change |
| T5 | The mesher's panic safety (C1), and the test fix by an exclusive handle (C2) | Section 6 |
| T6 | `material_placeholder` is a new code, not an extension of `material_unassigned` | Codes are the validator's API and never change meaning. `material_unassigned` means "the solver crashes or exits -1", the placeholder means "no material was chosen". The UI shows both as `MATERIALS_UNASSIGNED` |
| T7 | `tetgen_skipped_facets` is shown as `MESH_TETGEN_SKIPPED`; the other mesh codes as `MESH_` plus the code | The gate's name, and row 22 (3)'s precedent |
| T8 | The mesh-failure fixture is a CLI run with a stand-in TetGen | Section 5, F3 |
| T9 | Gate (c) and (d) check for no process running from a private copy of the solvers, not a machine-wide tasklist, which is printed beside it | F8. Other sessions may run up to 4 solvers |
| T10 | Test windows (`--e2e`, `--selftest`) are built with `focused(false)`; a normal launch is unchanged | Burhan's words (11:26), and the receipts on tao and wry |
| T11 | Page-focus emulation through CDP, only if the first smoke shows a page-focus failure | F9. It never changes OS focus |
| T12 | "The box" of gate (a) is the teaching room with a measured seed, not `tutorial1_box` | `tutorial1_box` computes 27 bands and loses about 1 particle in 10,000 at 2 kHz (0.01 %), so 'Particles lost 0.00 %' would not be true of it. The teaching room is the design's own 180 m³ box, and M10's |
| T13 | "The corrected hall" is built from `testdata/elmia_corrected.ply` by the core importer, with library materials and tutorial 2's first source and receivers | The task names the PLY. The grouping error does not matter to a responsiveness test |
| T14 | Percentages and times are formatted in Rust with integer rules, and the e2e recomputes them with BigInt | Rust's and JavaScript's rounding differ at exact halves. The check must not depend on either |
| T15 | Progress lines are counted, never rendered one by one | SPPS prints thousands. Gate (a) compares counts, and gate (b) the frame time |
| T16 | Close interception goes through a UI-registered `Channel`, not a Tauri event | No new permission (M9 (f)) |
| T17 | M11 ships no updater. `run_active()` exists for M13 | The updater is an open product decision (scope.md). The deliverable is conditional |
| T18 | `RUST_TEST_THREADS=4` for the core tests in the gate | The session's limit of 4 solver processes |
| T19 | Prior gates are run by `m11.ps1` (`m9.ps1` full, `m10.ps1 -SkipCore`), and `m10.ps1` bare once at integration | "m10.ps1 and m9.ps1 still pass", without running the 11-minute core suite twice |

**Product questions.** Each has a recommended default, and **the build follows the default** until
Burhan says otherwise:

| # | Question | Recommended default (built) | Why |
|---|---|---|---|
| PQ1 | What does Run do with unsaved changes? | **Run saves the project first,** as upstream's GUI does. A project never saved asks for Save as. The Console says "Saved <path> before the run" | The run must be of what is on screen, and `core::run` runs the file (F1). Blocking Run until the user saves is the alternative, at one click more |
| PQ2 | Where is Cancel? The design draws none | **In the Simulate panel's running block, under the bar ("Cancel run"), and in Simulate › Cancel run.** The Run button reads "Running <p> %" and is disabled, as drawn | The deliverable needs Cancel, and this keeps the design's Run button as drawn |
| PQ3 | Are the Simulate settings (particles, length, step, bands, air, method, particles saved, per-band maps, echogram per source) editable in M11? | **No: shown read-only, as drawn, with their true values.** The editor is a v1 piece of its own, before M12, recorded in scope.md | M11's deliverable and row 22 do not name it. It is an M-sized panel (parity 3b). C8 and C22 affect M12, so it must land before M12 |
| PQ4 | Closing the window during a run, and New, Open or Import during a run | **Close:** no extra prompt. The run is cancelled, recorded as Cancelled, and the app closes (the save prompt still comes first if the project is dirty). **New, Open, Import:** disabled while a run is active, with the tooltip "A run is active: cancel it first" | Gate (d) needs a clean close. A run left orphaned by switching projects would confuse the Runs tab |
| PQ5 | The words and the number on a Runs row | **Status as the gate spells it:** OK, FAIL, CRASH, Cancelled, Running, Interrupted, not the design's "Valid" and "Refused". **"Particles lost" is the worst band's loss** against the per-band limit. Each band is shown on the expanded row | The gate's words. The verdict's limit is per band, so the worst band is the number that decides OK |
| PQ6 | The reference library's content | **Upstream's 11 reference materials** ("100% absorbing" to "0% absorbing"), not `Default` | Parity M1. It is what the tutorials pick ("30% absorbing") |

**Open for Burhan and not blocking: MINOR B-20 and B-25** (backlog 14 and 17).

## 11. Traps (paid for elsewhere, or found while planning)

- **The M10 gate breaks the moment Run is wired:** the hard-coded 28 and the `M11_PENDING` control
  (4.5). Fix both in the foundation's first commit, not at integration.
- **`std::thread::scope` joins every thread before a panic leaves it.** A watch thread that waits on
  a flag the panicking code never sets holds the panic for as long as the thread lives (section 6).
- **exFAT and NTFS differ in more than cluster size.** A read-only file is deletable on NTFS with
  this toolchain, and not on exFAT. A test written on B: can hang on C:.
- **Runs write many files.** A run folder holds tens of files for the box, and more for the hall.
  **Never run from a project inside the repo:** the e2e copies its projects to C: first, and so
  must anyone trying the app by hand.
- **Stop-Process leaves a run folder with no `run.json`.** It is listed as Interrupted, never
  dropped and never guessed.
- **msedgedriver passes the environment on to `app.exe`,** which is how `SIMPA_SOLVERS_DIR` reaches
  the app. Check it at the first smoke with `solversStatus()`.
- **The first `ShowWindow` of a process can be overridden** by its parent's `STARTUPINFO`. If the
  watcher still sees a steal with `focused(false)`, look there first, and at msedgedriver's
  `Page.bringToFront`.
- **WebDriver's key and mouse events are CDP events,** not OS input. They need no OS focus, and the
  low-level hooks never see them. The page may still hold back `focus` events while unfocused (F9).
- **An LL keyboard hook is sensitive.** The watcher records only times and the injected flag, never
  key codes, and runs only while the gate runs.
- **Solvers run below normal priority from M11 on.** A bed's wall time can grow under other load;
  its results cannot change.
- **Windows PowerShell 5.1:** call native programs through `cmd /c`; write `"${var}:"`, not
  `"$var:"`.
- **`/compact` or an interrupt can kill a running workflow silently.** Check the agents' journals
  before assuming a package is still being built.
- **Still owed from M10:** one manual paste from Excel at hand-over (M10 PLAN 9).

## Files

- **This plan:** `docs/investigations/2026-09-29-m11/PLAN.md` (1 file on B:).
- **Changed in the same commit:**
  - `docs/v1.1-backlog.md` (rows 6-18);
  - `docs/investigations/2026-09-29-m10/MINOR.md` (where each item went);
  - `docs/scope.md` (the M11 plan, the v1 pieces outside the milestone specs, the product
    questions, the backlog count).
- **On C:, from the hang measurement,** in this session's scratchpad: 55 files in `mesh-hang\`, and
  the `mesh_project` test binary in `C:\tmp\nm-target\release\deps`. Nothing else was built, and
  nothing on B: was generated.
