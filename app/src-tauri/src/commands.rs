//! Every command the UI can invoke. All are `async fn` (a sync command runs on the main thread
//! and freezes the window; gate g counts them) and every body runs through
//! [`guard::blocking`], so a panic returns as an error. The one exception is
//! `panic_probe_unguarded`, which exists to measure what happens without the guard.
//!
//! Arguments keep their snake_case names on the JS side (`rename_all = "snake_case"`). Schema
//! values arrive as JSON text (see `bridge`).

use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use schemars::JsonSchema;
use serde::Serialize;
use tauri::ipc::{Channel, Response};
use tauri::{AppHandle, State};

use crate::bench::{BenchStore, Prepared};
use crate::bridge::{self, FloatProbe, ProjectInfo, Session};
use crate::events::{
    AppEvent, BATCH_PERIOD, BatchStats, Batcher, LineClass, RunEvent, RunEventBatch, Stream,
};
use crate::guard::{self, CmdError, CmdResult, lock};
use crate::results_data::{self, EchogramView, ReportView, RunDataIndex};
use crate::runs::{
    self, LibraryMaterial, ResultsState, RunSlot, RunStarted, RunStreamBatch, RunsView,
    SolversCache, SolversStatus,
};
use crate::scene::{EditOutcome, SceneState};
use crate::selftest::Selftest;
use crate::webview2::{self, WebviewInfo};

/// Shared state. Each part is behind its own lock, cloned into the blocking closure.
pub struct AppState {
    pub session: Arc<Mutex<Session>>,
    pub bench: Arc<Mutex<BenchStore>>,
    pub selftest: Option<Arc<Selftest>>,
    /// Why `--project` could not be opened, shown once in the Console.
    pub startup_error: Option<CmdError>,
    /// `--e2e`: the UI installs its test hooks (PLAN.md 2.5).
    pub e2e: bool,
    // ---- M11 (docs/investigations/2026-09-29-m11/PLAN.md 2.8) ----
    /// The active run, if any.
    pub run: Arc<Mutex<RunSlot>>,
    /// The UI's app-event channel, registered once at boot (`app_events`).
    pub ui_events: Arc<Mutex<Option<Channel<AppEvent>>>>,
    /// The solver checks, by (path, size, modification time).
    pub solvers: Arc<Mutex<SolversCache>>,
    /// The first close request the UI has not answered yet.
    pub close: Arc<Mutex<CloseState>>,
}

/// The oldest close request the UI was told of and has not acknowledged, and when. Requests
/// while it is unanswered are held: a page busy with other work acknowledges late. Only once it
/// has gone unanswered for [`CloseState::HUNG_UI`] does the next request close the window after
/// all (the run is cancelled first). The UI acknowledges each request as soon as its page runs,
/// by registering a fresh channel (`app_events`), and `app_quit` clears it too, so a live UI's
/// save prompt is skipped only if its page stays blocked for 5 s and the user asks again.
#[derive(Default)]
pub struct CloseState {
    pub requested: Option<Instant>,
}

impl CloseState {
    pub const HUNG_UI: Duration = Duration::from_secs(5);
}

#[derive(Clone, Debug, Serialize, JsonSchema)]
pub struct StartupInfo {
    /// This process's id: the M11 gate sends `WM_CLOSE` to its window and stops it by it.
    pub pid: u32,
    pub selftest: bool,
    /// Started with `--e2e`: the UI installs its test hooks.
    pub e2e: bool,
    pub project: Option<ProjectInfo>,
    pub project_error: Option<CmdError>,
    pub app_version: &'static str,
    pub tauri_version: &'static str,
    pub webview: WebviewInfo,
}

#[tauri::command(rename_all = "snake_case")]
pub async fn app_startup(state: State<'_, AppState>) -> CmdResult<StartupInfo> {
    let session = state.session.clone();
    let selftest = state.selftest.is_some();
    let e2e = state.e2e;
    let project_error = state.startup_error.clone();
    guard::blocking("app_startup", move || {
        Ok(StartupInfo {
            pid: std::process::id(),
            selftest,
            e2e,
            project: lock(&session, "project")?.info(),
            project_error,
            app_version: env!("CARGO_PKG_VERSION"),
            tauri_version: tauri::VERSION,
            webview: webview2::info(),
        })
    })
    .await
}

#[tauri::command(rename_all = "snake_case")]
pub async fn ping() -> CmdResult<String> {
    guard::blocking("ping", || Ok("pong".to_string())).await
}

/// Panics inside the guarded body; the UI must get `{code: "PANIC"}` back.
#[tauri::command(rename_all = "snake_case")]
pub async fn panic_probe() -> CmdResult<()> {
    guard::blocking("panic_probe", || -> CmdResult<()> {
        panic!("panic probe: a deliberate panic inside a command")
    })
    .await
}

/// How many times the body of `panic_probe_unguarded` has started.
static UNGUARDED_RUNS: AtomicU32 = AtomicU32::new(0);

/// Panics with no guard, to record what Tauri itself does with it (docs/decisions/ipc.md): the
/// reply is dropped, the page's custom-protocol request fails, and Tauri's IPC script marks the
/// protocol failed for the rest of the page's life and sends the same call again over
/// postMessage, so this body runs twice. The self-test calls it last.
#[tauri::command(rename_all = "snake_case")]
pub async fn panic_probe_unguarded() -> CmdResult<()> {
    UNGUARDED_RUNS.fetch_add(1, Ordering::SeqCst);
    panic!("unguarded panic probe: a deliberate panic with no boundary")
}

/// How many times the body of `panic_probe_unguarded` has started, for the self-test.
#[tauri::command(rename_all = "snake_case")]
pub async fn unguarded_panic_runs() -> CmdResult<u32> {
    guard::blocking("unguarded_panic_runs", || {
        Ok(UNGUARDED_RUNS.load(Ordering::SeqCst))
    })
    .await
}

#[tauri::command(rename_all = "snake_case")]
pub async fn bench_prepare(
    state: State<'_, AppState>,
    bytes: u64,
    seed: u64,
) -> CmdResult<Prepared> {
    let bench = state.bench.clone();
    guard::blocking("bench_prepare", move || {
        lock(&bench, "benchmark")?.prepare(bytes, seed)
    })
    .await
}

/// The prepared bytes as a raw `ipc::Response`: an ArrayBuffer in JS, no JSON step.
#[tauri::command(rename_all = "snake_case")]
pub async fn bench_take(state: State<'_, AppState>, token: u64) -> CmdResult<Response> {
    let bench = state.bench.clone();
    guard::blocking("bench_take", move || {
        let data = lock(&bench, "benchmark")?.take(token)?;
        Ok(Response::new(data))
    })
    .await
}

#[tauri::command(rename_all = "snake_case")]
pub async fn bench_clear(state: State<'_, AppState>) -> CmdResult<u64> {
    let bench = state.bench.clone();
    guard::blocking(
        "bench_clear",
        move || Ok(lock(&bench, "benchmark")?.clear()),
    )
    .await
}

#[derive(Clone, Debug, Serialize, JsonSchema)]
pub struct EventsProbeReport {
    pub lines: u64,
    pub elapsed_ms: f64,
    pub batch_period_ms: f64,
    pub stats: BatchStats,
}

/// Streams `lines` synthetic run events, one every `spacing_us`, through the batching channel.
#[tauri::command(rename_all = "snake_case")]
pub async fn run_events_probe(
    on_event: Channel<RunEventBatch>,
    lines: u32,
    spacing_us: u32,
) -> CmdResult<EventsProbeReport> {
    guard::blocking("run_events_probe", move || {
        let total_us = u64::from(lines) * u64::from(spacing_us);
        if lines > 100_000 || total_us > 10_000_000 {
            return Err(CmdError::new(
                "PROBE_TOO_LONG",
                format!("{lines} lines every {spacing_us} us: at most 100,000 lines and 10 s"),
            ));
        }
        let batcher = Batcher::spawn(BATCH_PERIOD, move |batch, events, last| {
            on_event
                .send(RunEventBatch {
                    batch,
                    events,
                    last,
                })
                .is_ok()
        });
        let start = Instant::now();
        let spacing = Duration::from_micros(u64::from(spacing_us));
        const CLASSES: [LineClass; 4] = [
            LineClass::Info,
            LineClass::Ok,
            LineClass::Warn,
            LineClass::Fail,
        ];
        for i in 0..u64::from(lines) {
            let due = start + spacing * i as u32;
            std::thread::sleep(due.saturating_duration_since(Instant::now()));
            batcher.push(RunEvent {
                seq: i,
                t_ms: start.elapsed().as_secs_f64() * 1e3,
                stream: if i % 3 == 2 {
                    Stream::Stderr
                } else {
                    Stream::Stdout
                },
                class: CLASSES[(i % 4) as usize],
                text: format!("probe line {i}"),
            });
        }
        let stats = batcher.finish();
        Ok(EventsProbeReport {
            lines: u64::from(lines),
            elapsed_ms: start.elapsed().as_secs_f64() * 1e3,
            batch_period_ms: BATCH_PERIOD.as_secs_f64() * 1e3,
            stats,
        })
    })
    .await
}

#[tauri::command(rename_all = "snake_case")]
pub async fn exact_float_probe(text: String) -> CmdResult<FloatProbe> {
    guard::blocking("exact_float_probe", move || {
        bridge::exact_float_probe(&text)
    })
    .await
}

#[tauri::command(rename_all = "snake_case")]
pub async fn project_new(state: State<'_, AppState>, name: String) -> CmdResult<ProjectInfo> {
    let session = state.session.clone();
    guard::blocking("project_new", move || {
        Ok(lock(&session, "project")?.new_project(&name))
    })
    .await
}

/// `text` is a whole `.simpa` document.
#[tauri::command(rename_all = "snake_case")]
pub async fn project_load_text(state: State<'_, AppState>, text: String) -> CmdResult<ProjectInfo> {
    let session = state.session.clone();
    guard::blocking("project_load_text", move || {
        lock(&session, "project")?.load_text(&text)
    })
    .await
}

/// `path` comes from the dialog plugin; the core reads the file, the webview never does.
#[tauri::command(rename_all = "snake_case")]
pub async fn project_open(state: State<'_, AppState>, path: String) -> CmdResult<ProjectInfo> {
    let session = state.session.clone();
    guard::blocking("project_open", move || {
        lock(&session, "project")?.open(&PathBuf::from(path))
    })
    .await
}

#[tauri::command(rename_all = "snake_case")]
pub async fn project_info(state: State<'_, AppState>) -> CmdResult<Option<ProjectInfo>> {
    let session = state.session.clone();
    guard::blocking(
        "project_info",
        move || Ok(lock(&session, "project")?.info()),
    )
    .await
}

#[tauri::command(rename_all = "snake_case")]
pub async fn project_json(state: State<'_, AppState>) -> CmdResult<String> {
    let session = state.session.clone();
    guard::blocking("project_json", move || lock(&session, "project")?.json()).await
}

/// `op` is one `Op` as JSON text, read with `Op::from_json`.
#[tauri::command(rename_all = "snake_case")]
pub async fn project_apply(state: State<'_, AppState>, op: String) -> CmdResult<ProjectInfo> {
    let session = state.session.clone();
    guard::blocking("project_apply", move || {
        lock(&session, "project")?.apply(&op)
    })
    .await
}

#[tauri::command(rename_all = "snake_case")]
pub async fn project_undo(state: State<'_, AppState>) -> CmdResult<ProjectInfo> {
    let session = state.session.clone();
    guard::blocking("project_undo", move || lock(&session, "project")?.undo()).await
}

#[tauri::command(rename_all = "snake_case")]
pub async fn project_redo(state: State<'_, AppState>) -> CmdResult<ProjectInfo> {
    let session = state.session.clone();
    guard::blocking("project_redo", move || lock(&session, "project")?.redo()).await
}

// ---- M10 (docs/investigations/2026-09-29-m10/PLAN.md, section 1) ------------------------------
// Every M10 command returns the whole `SceneState` (or the mesh bytes), so the UI replaces its
// copy with the backend's truth after each call. Check and import lines travel in `lines`.

/// The current state, or `None` with no project open. Takes any pending Console lines, such as
/// the check lines of a `--project` opened at startup.
#[tauri::command(rename_all = "snake_case")]
pub async fn scene_state(state: State<'_, AppState>) -> CmdResult<Option<SceneState>> {
    let session = state.session.clone();
    guard::blocking("scene_state", move || {
        Ok(lock(&session, "project")?.scene_state())
    })
    .await
}

/// A new empty project. The history is cleared.
#[tauri::command(rename_all = "snake_case")]
pub async fn scene_new(state: State<'_, AppState>, name: String) -> CmdResult<SceneState> {
    let (session, slot) = (state.session.clone(), state.run.clone());
    guard::blocking("scene_new", move || {
        let mut s = lock(&session, "project")?;
        runs::refuse_while_running(&slot, "New project")?;
        s.scene_new(&name)
    })
    .await
}

/// Opens a `.simpa` file (`schema::load`), then runs the model check and the validator.
#[tauri::command(rename_all = "snake_case")]
pub async fn scene_open(state: State<'_, AppState>, path: String) -> CmdResult<SceneState> {
    let (session, slot) = (state.session.clone(), state.run.clone());
    guard::blocking("scene_open", move || {
        let mut s = lock(&session, "project")?;
        runs::refuse_while_running(&slot, "Open")?;
        s.scene_open(&PathBuf::from(path))
    })
    .await
}

/// Imports a PLY, OBJ or STL file as a new project. `unit` is m, cm, mm, ft or in; `up` is y or
/// z. A geometry the check refuses is loaded, its faces highlighted and Run blocked.
#[tauri::command(rename_all = "snake_case")]
pub async fn model_import(
    state: State<'_, AppState>,
    path: String,
    unit: String,
    up: String,
) -> CmdResult<SceneState> {
    let (session, slot) = (state.session.clone(), state.run.clone());
    guard::blocking("model_import", move || {
        let mut s = lock(&session, "project")?;
        runs::refuse_while_running(&slot, "Import")?;
        s.model_import(&PathBuf::from(path), &unit, &up)
    })
    .await
}

/// Saves atomically: `None` to the session's path, `Some` as Save As.
#[tauri::command(rename_all = "snake_case")]
pub async fn project_save(
    state: State<'_, AppState>,
    path: Option<String>,
) -> CmdResult<SceneState> {
    let session = state.session.clone();
    guard::blocking("project_save", move || {
        lock(&session, "project")?.save(path.map(PathBuf::from).as_deref())
    })
    .await
}

/// The checked apply: `op` is one `Op` as JSON text. A validator refusal is `applied: false`,
/// not an error.
#[tauri::command(rename_all = "snake_case")]
pub async fn edit_apply(state: State<'_, AppState>, op: String) -> CmdResult<EditOutcome> {
    let session = state.session.clone();
    guard::blocking("edit_apply", move || {
        lock(&session, "project")?.edit_apply(&op)
    })
    .await
}

/// A band preset (PQ3): every band of `kind` (`octave` or `third_octave`) from `lowest_hz` to
/// `highest_hz`, per-band values from the nearest current band, as one undoable checked apply.
#[tauri::command(rename_all = "snake_case")]
pub async fn edit_reband(
    state: State<'_, AppState>,
    kind: String,
    lowest_hz: u32,
    highest_hz: u32,
) -> CmdResult<EditOutcome> {
    let session = state.session.clone();
    guard::blocking("edit_reband", move || {
        lock(&session, "project")?.edit_reband(&kind, lowest_hz, highest_hz)
    })
    .await
}

/// New group from selection (scope row 15 (1), G19): `faces` (indices into the mesh) sent to a
/// new surface group, as one undoable checked apply (`Session::edit_regroup`).
#[tauri::command(rename_all = "snake_case")]
pub async fn edit_regroup(state: State<'_, AppState>, faces: Vec<u32>) -> CmdResult<EditOutcome> {
    let session = state.session.clone();
    guard::blocking("edit_regroup", move || {
        lock(&session, "project")?.edit_regroup(&faces)
    })
    .await
}

#[tauri::command(rename_all = "snake_case")]
pub async fn edit_undo(state: State<'_, AppState>) -> CmdResult<SceneState> {
    let session = state.session.clone();
    guard::blocking("edit_undo", move || lock(&session, "project")?.edit_undo()).await
}

#[tauri::command(rename_all = "snake_case")]
pub async fn edit_redo(state: State<'_, AppState>) -> CmdResult<SceneState> {
    let session = state.session.clone();
    guard::blocking("edit_redo", move || lock(&session, "project")?.edit_redo()).await
}

/// The geometry as raw bytes (`scene::mesh_bytes`): an ArrayBuffer in JS, no JSON step.
#[tauri::command(rename_all = "snake_case")]
pub async fn scene_mesh(state: State<'_, AppState>) -> CmdResult<Response> {
    let session = state.session.clone();
    guard::blocking("scene_mesh", move || {
        Ok(Response::new(lock(&session, "project")?.mesh()?))
    })
    .await
}

// ---- M11 (docs/investigations/2026-09-29-m11/PLAN.md 2.2) --------------------------------------

/// Starts a run of the open project, saved and unblocked, with `solver` (`spps` or `tcr`), and
/// returns at once. The run streams its events into `on_event`, batched, `last: true` at the end.
#[tauri::command(rename_all = "snake_case")]
pub async fn run_start(
    state: State<'_, AppState>,
    solver: String,
    on_event: Channel<RunStreamBatch>,
) -> CmdResult<RunStarted> {
    let (session, slot, solvers) = (
        state.session.clone(),
        state.run.clone(),
        state.solvers.clone(),
    );
    guard::blocking("run_start", move || {
        runs::start(&session, &slot, &solvers, &solver, on_event)
    })
    .await
}

/// Cancels the active run through its token; the core's process layer ends the Job Object.
/// `true` when a run was active.
#[tauri::command(rename_all = "snake_case")]
pub async fn run_cancel(state: State<'_, AppState>) -> CmdResult<bool> {
    let slot = state.run.clone();
    guard::blocking("run_cancel", move || Ok(lock(&slot, "run")?.cancel())).await
}

/// The open project's file, or why there is none.
fn project_path(session: &Mutex<Session>) -> CmdResult<Option<PathBuf>> {
    let s = lock(session, "project")?;
    if s.info().is_none() {
        return Err(CmdError::new("NO_PROJECT", "no project is open"));
    }
    Ok(s.path().map(std::path::Path::to_path_buf))
}

/// Every run of the open project, from the `run.json` files under `<project folder>/runs`.
#[tauri::command(rename_all = "snake_case")]
pub async fn runs_list(state: State<'_, AppState>) -> CmdResult<RunsView> {
    let (session, slot) = (state.session.clone(), state.run.clone());
    guard::blocking("runs_list", move || {
        let Some(path) = project_path(&session)? else {
            // Never saved: no runs root yet, so no runs.
            return Ok(RunsView {
                root: String::new(),
                rows: Vec::new(),
                other_projects: 0,
                active: None,
            });
        };
        let active = lock(&slot, "run")?.active_run().map(str::to_string);
        runs::list(&runs::runs_root(&path), &path, active.as_deref())
    })
    .await
}

/// Whether the run `run` (a bare run-folder name) has results that verify. Never a value.
#[tauri::command(rename_all = "snake_case")]
pub async fn run_results(state: State<'_, AppState>, run: String) -> CmdResult<ResultsState> {
    let session = state.session.clone();
    guard::blocking("run_results", move || {
        let path = project_path(&session)?.ok_or_else(|| {
            CmdError::new(
                "RUN_NOT_FOUND",
                format!("no run '{run}': the project has no runs"),
            )
        })?;
        runs::results_state(&runs::runs_root(&path), &run)
    })
    .await
}

// ---- M12 (docs/investigations/2026-10-03-m12/PLAN.md, P1 item 3): the Results step's reads ------

/// The runs root of the open project, or `RUN_NOT_FOUND` for `run` when it was never saved.
fn runs_root_for(session: &Mutex<Session>, run: &str) -> CmdResult<PathBuf> {
    let path = project_path(session)?.ok_or_else(|| {
        CmdError::new(
            "RUN_NOT_FOUND",
            format!("no run '{run}': the project has no runs"),
        )
    })?;
    Ok(runs::runs_root(&path))
}

/// The run's results state and, when its results load, the report `simpa results <run> --json`
/// prints, with each parameter's bed status (`report.bed`).
#[tauri::command(rename_all = "snake_case")]
pub async fn run_report(state: State<'_, AppState>, run: String) -> CmdResult<ReportView> {
    let session = state.session.clone();
    guard::blocking("run_report", move || {
        let mut view = results_data::report_view(&runs_root_for(&session, &run)?, &run)?;
        if view.report.is_some() {
            let s = lock(&session, "project")?;
            view.surface_groups = s
                .project()
                .map(results_data::group_names)
                .unwrap_or_default();
        }
        Ok(view)
    })
    .await
}

/// Which surface maps, particle files and echograms the run holds.
#[tauri::command(rename_all = "snake_case")]
pub async fn run_data(state: State<'_, AppState>, run: String) -> CmdResult<RunDataIndex> {
    let session = state.session.clone();
    guard::blocking("run_data", move || {
        results_data::data_index(&runs_root_for(&session, &run)?, &run)
    })
    .await
}

/// One surface map (`path` as `run_data` lists it) as SMAP bytes (`results_data`): an
/// ArrayBuffer in JS, the `.csbin`'s float32 values bit for bit.
#[tauri::command(rename_all = "snake_case")]
pub async fn run_surface_map(
    state: State<'_, AppState>,
    run: String,
    path: String,
) -> CmdResult<Response> {
    let session = state.session.clone();
    guard::blocking("run_surface_map", move || {
        results_data::surface_map_bytes(&runs_root_for(&session, &run)?, &run, &path)
            .map(Response::new)
    })
    .await
}

/// One band's saved particles as PART bytes (`results_data`): an ArrayBuffer in JS, the
/// `.pbin`'s positions and energies bit for bit.
#[tauri::command(rename_all = "snake_case")]
pub async fn run_particles(
    state: State<'_, AppState>,
    run: String,
    band_hz: i32,
) -> CmdResult<Response> {
    let session = state.session.clone();
    guard::blocking("run_particles", move || {
        results_data::particles_bytes(&runs_root_for(&session, &run)?, &run, band_hz)
            .map(Response::new)
    })
    .await
}

/// One SPPS point receiver's echogram per band, and each source's own when the run wrote them.
#[tauri::command(rename_all = "snake_case")]
pub async fn run_echogram(
    state: State<'_, AppState>,
    run: String,
    receiver: String,
) -> CmdResult<EchogramView> {
    let session = state.session.clone();
    guard::blocking("run_echogram", move || {
        results_data::echogram(&runs_root_for(&session, &run)?, &run, &receiver)
    })
    .await
}

/// Opens an upstream I-Simpa `.proj` as a new, unsaved project.
#[tauri::command(rename_all = "snake_case")]
pub async fn proj_import(state: State<'_, AppState>, path: String) -> CmdResult<SceneState> {
    let (session, slot) = (state.session.clone(), state.run.clone());
    guard::blocking("proj_import", move || {
        let mut s = lock(&session, "project")?;
        runs::refuse_while_running(&slot, "Open")?;
        s.proj_import(&PathBuf::from(path))
    })
    .await
}

/// Upstream's reference materials but the placeholder, with the core's exact values.
#[tauri::command(rename_all = "snake_case")]
pub async fn material_library() -> CmdResult<Vec<LibraryMaterial>> {
    guard::blocking("material_library", || Ok(runs::material_library())).await
}

/// The four executables a run needs, found and checked against the verified build.
#[tauri::command(rename_all = "snake_case")]
pub async fn solvers_status(state: State<'_, AppState>) -> CmdResult<SolversStatus> {
    let solvers = state.solvers.clone();
    guard::blocking("solvers_status", move || runs::solvers_status(&solvers)).await
}

/// Registers the UI's app-event channel (the close request). Until it is registered, a close
/// request is let through. The UI registers a fresh channel as soon as a close request reaches
/// it: that is its acknowledgement, so the request no longer counts as unanswered, and the next
/// close goes to the UI (the save prompt) instead of closing past it. A hung UI acknowledges
/// nothing, and its window closes on a request made once the first has gone unanswered for
/// [`CloseState::HUNG_UI`].
#[tauri::command(rename_all = "snake_case")]
pub async fn app_events(state: State<'_, AppState>, on_event: Channel<AppEvent>) -> CmdResult<()> {
    let (slot, close) = (state.ui_events.clone(), state.close.clone());
    guard::blocking("app_events", move || {
        *lock(&slot, "app events")? = Some(on_event);
        lock(&close, "close")?.requested = None;
        Ok(())
    })
    .await
}

/// The UI's answer to a close request, once the save prompt is dealt with: cancels any active
/// run, waits up to 3 s for its `run.json`, then exits.
#[tauri::command(rename_all = "snake_case")]
pub async fn app_quit(app: AppHandle, state: State<'_, AppState>) -> CmdResult<()> {
    let (slot, close) = (state.run.clone(), state.close.clone());
    guard::blocking("app_quit", move || {
        runs::cancel_and_wait(&slot, runs::QUIT_WAIT);
        lock(&close, "close")?.requested = None;
        Ok(())
    })
    .await?;
    app.exit(0);
    Ok(())
}

/// The UI's self-test result as JSON text. Written to the `--selftest` path, then the app exits
/// with 0 (every check passed) or 1.
#[tauri::command(rename_all = "snake_case")]
pub async fn selftest_report(
    app: AppHandle,
    state: State<'_, AppState>,
    report: String,
) -> CmdResult<bool> {
    let selftest = state
        .selftest
        .clone()
        .ok_or_else(|| CmdError::new("SELFTEST_OFF", "the app was not started with --selftest"))?;
    let ok = guard::blocking("selftest_report", move || selftest.report(&report)).await?;
    app.exit(if ok { 0 } else { 1 });
    Ok(ok)
}

/// W9 export: writes the request's raw bytes to the path the save dialog returned. The path
/// (`encodeURIComponent`) and the kind (`csv`, `json`, `png`) ride in the `x-export-path` and
/// `x-export-kind` headers; `export::write` refuses a path without the kind's extension and
/// bytes that are not the kind, and writes the file whole or not at all. Returns the bytes written.
#[tauri::command(rename_all = "snake_case")]
pub async fn export_write(request: tauri::ipc::Request<'_>) -> CmdResult<u64> {
    let header = |name: &str| -> CmdResult<String> {
        let v = request
            .headers()
            .get(name)
            .ok_or_else(|| CmdError::new("EXPORT_PATH", format!("no {name} header")))?;
        v.to_str()
            .map(str::to_owned)
            .map_err(|_| CmdError::new("EXPORT_PATH", format!("the {name} header is not text")))
    };
    let path = crate::export::percent_decode(&header("x-export-path")?)?;
    let kind = header("x-export-kind")?;
    let bytes = match request.body() {
        tauri::ipc::InvokeBody::Raw(b) => b.clone(),
        tauri::ipc::InvokeBody::Json(_) => {
            return Err(CmdError::new(
                "EXPORT_CONTENT",
                "the export's bytes must be sent raw, not as JSON",
            ));
        }
    };
    guard::blocking("export_write", move || {
        crate::export::write(&kind, &path, &bytes)
    })
    .await
}
