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
    BATCH_PERIOD, BatchStats, Batcher, LineClass, RunEvent, RunEventBatch, Stream,
};
use crate::guard::{self, CmdError, CmdResult, lock};
use crate::selftest::Selftest;
use crate::webview2::{self, WebviewInfo};

/// Shared state. Each part is behind its own lock, cloned into the blocking closure.
pub struct AppState {
    pub session: Arc<Mutex<Session>>,
    pub bench: Arc<Mutex<BenchStore>>,
    pub selftest: Option<Arc<Selftest>>,
    /// Why `--project` could not be opened, shown once in the Console.
    pub startup_error: Option<CmdError>,
}

#[derive(Clone, Debug, Serialize, JsonSchema)]
pub struct StartupInfo {
    pub selftest: bool,
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
    let project_error = state.startup_error.clone();
    guard::blocking("app_startup", move || {
        Ok(StartupInfo {
            selftest,
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
        let batcher = Batcher::spawn(BATCH_PERIOD, move |batch| on_event.send(batch).is_ok());
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
