//! The desktop app: a Tauri 2 shell that links `simpa-core` directly (rebuild plan, option B).
//!
//! ```text
//! app.exe                          the window
//! app.exe --project <file.simpa>   open a project at startup
//! app.exe --e2e                    install the UI's test hooks (the M10 and M11 e2e harness)
//! app.exe --selftest <out.json>    measure, write <out.json>, exit (0 pass, 1 fail, 3 timeout)
//! app.exe --dump-schema <dir>     write schema.json and ipc.json, the UI's TypeScript sources
//! ```
//!
//! **The window.** `tauri.conf.json` declares it with `"create": false`, and `setup` builds it
//! from that config: focused on a normal launch, as before, and **unfocused** under `--e2e` and
//! `--selftest`, so a test window opens visible, on screen, without taking keyboard focus from
//! whatever the person at the machine is using (Burhan, 2026-09-29 11:26: the test windows stay
//! visible; only the focus-stealing is fixed). With `focused(false)` tao shows the window with
//! `SW_SHOWNOACTIVATE`, and wry does not move focus into the webview. Nothing in the app asks for
//! focus afterwards.
//!
//! **Closing.** Once the UI has registered its app-event channel, a close request (the close
//! button, Alt+F4, `WM_CLOSE`) is held and passed to the UI, which acknowledges it at once (it
//! registers a fresh channel), asks to save a dirty project and answers with `app_quit`. A page
//! busy with other work acknowledges late, not never: further requests while one is unanswered
//! are held, and only a request that has gone unanswered for 5 s (what Windows itself calls a
//! hung window) means the UI cannot answer, so the next one closes the window after all (M11
//! review 2, app 1: two clicks 153 ms apart on a page busy for 600 ms closed a dirty project
//! with no prompt). Whatever closes it, an active run is cancelled first and
//! given up to 3 s to write its `run.json`; if the process is killed instead, the Job Object's
//! `KILL_ON_JOB_CLOSE` ends the solver with it, even one killed in the milliseconds between its
//! creation and its own job (the core's `process::winproc`, "the spawn window").

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod aural;
mod bench;
mod bindings;
mod bridge;
mod commands;
mod events;
mod examples;
mod export;
mod guard;
mod live;
mod mesh_now;
mod results_data;
mod run_files;
mod runs;
mod scene;
mod selftest;
mod webview2;

use std::ffi::OsString;
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use commands::{AppState, CloseState};
use events::AppEvent;
use tauri::Manager;

#[derive(Debug, Default, PartialEq, Eq)]
struct GuiArgs {
    selftest: Option<PathBuf>,
    project: Option<PathBuf>,
    e2e: bool,
}

#[derive(Debug, PartialEq, Eq)]
enum Mode {
    Gui(GuiArgs),
    DumpSchema(PathBuf),
}

fn parse_args(args: impl IntoIterator<Item = OsString>) -> Result<Mode, String> {
    let mut gui = GuiArgs::default();
    let mut dump = None;
    let mut it = args.into_iter();
    while let Some(arg) = it.next() {
        let flag = arg.to_string_lossy().into_owned();
        if flag == "--e2e" {
            if gui.e2e {
                return Err("--e2e given twice".to_string());
            }
            gui.e2e = true;
            continue;
        }
        let mut value = |name: &str| {
            it.next()
                .map(PathBuf::from)
                .ok_or_else(|| format!("{name} needs a path"))
        };
        let slot = match flag.as_str() {
            "--selftest" => &mut gui.selftest,
            "--project" => &mut gui.project,
            "--dump-schema" => &mut dump,
            other => return Err(format!("unknown argument '{other}'")),
        };
        if slot.is_some() {
            return Err(format!("{flag} given twice"));
        }
        *slot = Some(value(&flag)?);
    }
    match dump {
        Some(_) if gui != GuiArgs::default() => {
            Err("--dump-schema takes no other argument".to_string())
        }
        Some(path) => Ok(Mode::DumpSchema(path)),
        None => Ok(Mode::Gui(gui)),
    }
}

/// Resolves `path` against the working directory at startup, before anything can change it.
fn absolute(path: PathBuf) -> PathBuf {
    std::path::absolute(&path).unwrap_or(path)
}

fn main() -> ExitCode {
    match parse_args(std::env::args_os().skip(1)) {
        Ok(Mode::DumpSchema(dir)) => match dump_schemas(&dir) {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => fail(&e),
        },
        Ok(Mode::Gui(args)) => run(args),
        Err(e) => fail(&e),
    }
}

fn dump_schemas(dir: &std::path::Path) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("could not create {}: {e}", dir.display()))?;
    for (file, text) in bindings::dump_texts()? {
        let path = dir.join(file);
        std::fs::write(&path, text)
            .map_err(|e| format!("could not write {}: {e}", path.display()))?;
    }
    Ok(())
}

fn fail(message: &str) -> ExitCode {
    // A release build has no console; the message still reaches a redirected stderr.
    eprintln!("app: {message}");
    ExitCode::from(2)
}

/// Whether the window may take focus when it opens: never for a test window (`--e2e`,
/// `--selftest`).
fn focus_on_open(args: &GuiArgs) -> bool {
    !(args.e2e || args.selftest.is_some())
}

/// What a close request does, given when the UI was told of the oldest request it has not
/// acknowledged (`None`: it has acknowledged every one).
#[derive(Debug, PartialEq, Eq)]
enum CloseAction {
    /// Tell the UI and hold the window: the save prompt decides.
    AskUi,
    /// The UI has a request it has not answered yet, for less than [`CloseState::HUNG_UI`]: a
    /// page busy with other work, not a hung one. Hold the window; the UI answers the request it
    /// already has.
    Hold,
    /// The UI has not answered for [`CloseState::HUNG_UI`]: it cannot, so the window closes.
    LetThrough,
}

fn close_action(unanswered_since: Option<Instant>, now: Instant) -> CloseAction {
    match unanswered_since {
        None => CloseAction::AskUi,
        Some(t) if now.duration_since(t) >= CloseState::HUNG_UI => CloseAction::LetThrough,
        Some(_) => CloseAction::Hold,
    }
}

/// A close request (PLAN.md 2.2): held and passed to the UI once it has registered its channel,
/// let through otherwise, with any active run cancelled and given up to 3 s for its `run.json`.
fn on_close_requested(state: &AppState, api: &tauri::CloseRequestApi) {
    let let_through = |state: &AppState| {
        runs::cancel_and_wait(&state.run, runs::QUIT_WAIT);
    };
    let channel = state.ui_events.lock().ok().and_then(|c| c.clone());
    let Some(channel) = channel else {
        return let_through(state);
    };
    let now = Instant::now();
    let unanswered_since = state.close.lock().ok().and_then(|c| c.requested);
    match close_action(unanswered_since, now) {
        CloseAction::AskUi => {}
        CloseAction::Hold => return api.prevent_close(),
        // The UI did not answer for 5 s: it cannot, so the window closes.
        CloseAction::LetThrough => return let_through(state),
    }
    // Never block the window's thread on a busy session: a busy one counts as dirty.
    let dirty = state
        .session
        .try_lock()
        .ok()
        .and_then(|s| s.info())
        .is_none_or(|i| i.dirty);
    let run_active = state.run.lock().map(|r| r.run_active()).unwrap_or(false);
    if channel
        .send(AppEvent::CloseRequested { dirty, run_active })
        .is_err()
    {
        return let_through(state);
    }
    if let Ok(mut c) = state.close.lock() {
        c.requested = Some(now);
    }
    api.prevent_close();
}

fn run(args: GuiArgs) -> ExitCode {
    let focus = focus_on_open(&args);
    let selftest = args
        .selftest
        .map(|out| Arc::new(selftest::Selftest::new(absolute(out))));
    let mut session = bridge::Session::default();
    let startup_error = args
        .project
        .map(absolute)
        .and_then(|path| session.open(&path).err());
    let state = AppState {
        session: Arc::new(Mutex::new(session)),
        bench: Arc::new(Mutex::new(bench::BenchStore::default())),
        selftest: selftest.clone(),
        startup_error,
        e2e: args.e2e,
        run: Arc::new(Mutex::new(runs::RunSlot::default())),
        ui_events: Arc::new(Mutex::new(None)),
        solvers: Arc::new(Mutex::new(runs::SolversCache::default())),
        gpu: Arc::new(Mutex::new(runs::GpuCache::default())),
        close: Arc::new(Mutex::new(CloseState::default())),
    };
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(state)
        .invoke_handler(tauri::generate_handler![
            commands::app_startup,
            commands::ping,
            commands::panic_probe,
            commands::panic_probe_unguarded,
            commands::unguarded_panic_runs,
            commands::bench_prepare,
            commands::bench_take,
            commands::bench_clear,
            commands::run_events_probe,
            commands::exact_float_probe,
            commands::project_new,
            commands::project_load_text,
            commands::project_open,
            commands::project_info,
            commands::project_json,
            commands::project_apply,
            commands::project_undo,
            commands::project_redo,
            commands::selftest_report,
            commands::scene_state,
            commands::scene_new,
            commands::scene_open,
            commands::example_open,
            commands::model_import,
            commands::model_repair,
            commands::model_reimport,
            commands::mesh_now,
            commands::project_save,
            commands::edit_apply,
            commands::advice_apply,
            commands::edit_reband,
            commands::edit_regroup,
            commands::edit_add_group,
            commands::edit_undo,
            commands::edit_redo,
            commands::scene_mesh,
            commands::run_start,
            commands::run_cancel,
            commands::runs_list,
            commands::run_label,
            commands::run_open_folder,
            commands::run_delete,
            commands::run_results,
            commands::run_report,
            commands::run_data,
            commands::run_surface_map,
            commands::run_particles,
            commands::run_echogram,
            commands::run_auralize,
            commands::proj_import,
            commands::material_library,
            commands::spectrum_library,
            commands::solvers_status,
            commands::spps_gpu_status,
            commands::app_events,
            commands::app_quit,
            commands::export_write,
        ])
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                on_close_requested(&window.state::<AppState>(), api);
            }
        })
        .setup(move |app| {
            // The one window, from tauri.conf.json ("create": false there), unfocused for tests.
            let config = app
                .config()
                .app
                .windows
                .first()
                .cloned()
                .ok_or("tauri.conf.json declares no window")?;
            tauri::WebviewWindowBuilder::from_config(app.handle(), &config)?
                .focused(focus)
                .build()?;
            if let Some(st) = &selftest {
                st.start_watchdog(selftest::TIMEOUT);
            }
            Ok(())
        })
        .build(tauri::generate_context!());
    match app {
        Ok(app) => {
            let code = app.run_return(|_, _| {});
            ExitCode::from(u8::try_from(code).unwrap_or(1))
        }
        Err(e) => fail(&format!("could not start: {e}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Result<Mode, String> {
        parse_args(args.iter().map(OsString::from))
    }

    #[test]
    fn arguments() {
        assert_eq!(parse(&[]), Ok(Mode::Gui(GuiArgs::default())));
        assert_eq!(
            parse(&["--selftest", "o.json", "--project", "p.simpa"]),
            Ok(Mode::Gui(GuiArgs {
                selftest: Some("o.json".into()),
                project: Some("p.simpa".into()),
                e2e: false,
            }))
        );
        assert_eq!(
            parse(&["--e2e", "--project", "p.simpa"]),
            Ok(Mode::Gui(GuiArgs {
                selftest: None,
                project: Some("p.simpa".into()),
                e2e: true,
            }))
        );
        assert!(parse(&["--e2e", "--e2e"]).is_err());
        assert!(parse(&["--dump-schema", "s", "--e2e"]).is_err());
        assert_eq!(
            parse(&["--dump-schema", "s.json"]),
            Ok(Mode::DumpSchema("s.json".into()))
        );
        assert!(parse(&["--selftest"]).is_err());
        assert!(parse(&["--selftest", "a", "--selftest", "b"]).is_err());
        assert!(parse(&["--dump-schema", "s", "--selftest", "o"]).is_err());
        assert!(parse(&["--frobnicate"]).is_err());
    }

    use std::time::Duration;

    /// M11 review 2, app 1: a page busy for 600 ms got two close requests 153 ms apart, could
    /// not acknowledge the first in between, and the second closed a dirty project with no
    /// prompt. A request unanswered for less than HUNG_UI is a busy page: the next is held.
    #[test]
    fn a_busy_page_is_not_a_hung_one() {
        let now = Instant::now();
        let ago = |ms: u64| now - Duration::from_millis(ms);
        assert_eq!(close_action(None, now), CloseAction::AskUi);
        for ms in [0, 2, 153, 1_008, 4_999] {
            assert_eq!(
                close_action(Some(ago(ms)), now),
                CloseAction::Hold,
                "a request unanswered for {ms} ms"
            );
        }
        for ms in [5_000, 5_001, 60_000] {
            assert_eq!(
                close_action(Some(ago(ms)), now),
                CloseAction::LetThrough,
                "a request unanswered for {ms} ms: a hung UI"
            );
        }
    }

    #[test]
    fn test_windows_open_without_focus_and_a_normal_launch_is_unchanged() {
        let gui = |args: &[&str]| match parse(args).unwrap() {
            Mode::Gui(g) => g,
            Mode::DumpSchema(_) => unreachable!(),
        };
        assert!(focus_on_open(&gui(&[])));
        assert!(focus_on_open(&gui(&["--project", "p.simpa"])));
        assert!(!focus_on_open(&gui(&["--e2e"])));
        assert!(!focus_on_open(&gui(&["--e2e", "--project", "p.simpa"])));
        assert!(!focus_on_open(&gui(&["--selftest", "o.json"])));
    }

    /// The window is built in `setup` from the config, so the config must not create it too
    /// (that window would take focus), nor hide it, nor ask for focus or to stay on top.
    #[test]
    fn the_config_leaves_the_window_to_setup() {
        let conf: serde_json::Value =
            serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
        let windows = conf["app"]["windows"].as_array().unwrap();
        assert_eq!(windows.len(), 1);
        let w = &windows[0];
        assert_eq!(w["label"], "main");
        assert_eq!(w["create"], false);
        assert_ne!(w["visible"], false);
        assert_ne!(w["focus"], true);
        assert!(w.get("alwaysOnTop").is_none());
    }
}
