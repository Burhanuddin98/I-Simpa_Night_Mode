//! The desktop app: a Tauri 2 shell that links `simpa-core` directly (rebuild plan, option B).
//!
//! ```text
//! app.exe                          the window
//! app.exe --project <file.simpa>   open a project at startup
//! app.exe --selftest <out.json>    measure, write <out.json>, exit (0 pass, 1 fail, 3 timeout)
//! app.exe --dump-schema <dir>     write schema.json and ipc.json, the UI's TypeScript sources
//! ```

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod bench;
mod bindings;
mod bridge;
mod commands;
mod events;
mod guard;
mod selftest;
mod webview2;

use std::ffi::OsString;
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::{Arc, Mutex};

use commands::AppState;

#[derive(Debug, Default, PartialEq, Eq)]
struct GuiArgs {
    selftest: Option<PathBuf>,
    project: Option<PathBuf>,
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

fn run(args: GuiArgs) -> ExitCode {
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
        ])
        .setup(move |_app| {
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
            }))
        );
        assert_eq!(
            parse(&["--dump-schema", "s.json"]),
            Ok(Mode::DumpSchema("s.json".into()))
        );
        assert!(parse(&["--selftest"]).is_err());
        assert!(parse(&["--selftest", "a", "--selftest", "b"]).is_err());
        assert!(parse(&["--dump-schema", "s", "--selftest", "o"]).is_err());
        assert!(parse(&["--frobnicate"]).is_err());
    }
}
