//! What the person does to one of the open project's run folders from the Runs tab (parity
//! M12c P2; upstream's result tree, `tree_rapport/e_report_file.cpp`):
//! - **R2, a label**: a name of the person's own beside `#n`, kept in `notes.json` beside
//!   `run.json`, never by renaming the folder (the folder's name is the run's key everywhere, and
//!   `is_run_name` is what keeps a path from climbing out of the runs root).
//! - **R4, open the folder**: Explorer on the run's folder, started from here, so the webview keeps
//!   no shell permission (capabilities/default.json: "No shell").
//!
//! `notes.json` is the person's, not the run's: `results::load` re-checks `run.json` and `solve/`
//! only, so a note never changes whether a run's results verify. A run is acted on only when it is
//! a row of the open project's Runs tab (`runs::list`), and changed never while it is the active
//! run.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::guard::{CmdError, CmdResult};
use crate::runs::{self, RunRow, RunStatusUi, RunsView};

/// The person's notes on a run, beside `run.json`.
pub const NOTES_FILE: &str = "notes.json";
/// The longest label, in characters.
pub const LABEL_MAX_CHARS: usize = 80;

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
struct Notes {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    label: Option<String>,
}

/// The notes in run folder `dir`: none when there is no `notes.json`, an error when it does not
/// read (never taken as no notes: a note that does not read is said, not dropped).
fn read_notes(dir: &Path) -> Result<Notes, String> {
    let path = dir.join(NOTES_FILE);
    match std::fs::read_to_string(&path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Notes::default()),
        Err(e) => Err(format!("{}: {e}", path.display())),
        Ok(text) => serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display())),
    }
}

/// Writes `notes` whole or not at all: a temporary file beside it, then renamed over it.
fn write_notes(dir: &Path, notes: &Notes) -> CmdResult<()> {
    let path = dir.join(NOTES_FILE);
    let tmp = dir.join(format!("{NOTES_FILE}.tmp"));
    let text = serde_json::to_string_pretty(notes)
        .map_err(|e| CmdError::new("RUN_NOTES_IO", format!("the notes did not encode: {e}")))?;
    std::fs::write(&tmp, text)
        .and_then(|()| std::fs::rename(&tmp, &path))
        .map_err(|e| {
            let _ = std::fs::remove_file(&tmp);
            CmdError::new(
                "RUN_NOTES_IO",
                format!("could not write {}: {e}", path.display()),
            )
        })
}

/// A row's label from its folder's notes (`runs::list` calls this for every row it lists).
pub fn annotate(row: &mut RunRow, dir: &Path) {
    match read_notes(dir) {
        Ok(n) => row.label = n.label,
        Err(e) => row.notes_error = Some(e),
    }
}

/// The label as stored: trimmed, at most [`LABEL_MAX_CHARS`] characters, no control character;
/// `None` for an empty one (the label is cleared).
pub fn checked_label(label: &str) -> CmdResult<Option<String>> {
    let t = label.trim();
    if t.chars().any(char::is_control) {
        return Err(CmdError::new(
            "RUN_LABEL_INVALID",
            "a run's label is one line, without control characters",
        ));
    }
    let n = t.chars().count();
    if n > LABEL_MAX_CHARS {
        return Err(CmdError::new(
            "RUN_LABEL_INVALID",
            format!("a run's label is at most {LABEL_MAX_CHARS} characters; this one is {n}"),
        ));
    }
    Ok((!t.is_empty()).then(|| t.to_string()))
}

/// The row of `run` in the open project's runs, refused unless it is one.
fn row_of(view: &RunsView, run: &str, what: &str) -> CmdResult<RunRow> {
    view.rows
        .iter()
        .find(|r| r.run == run)
        .cloned()
        .ok_or_else(|| {
            CmdError::new(
                "RUN_NOT_FOUND",
                format!("{what}: '{run}' is not a run of the open project"),
            )
        })
}

/// The row of `run` in the open project's runs, refused unless it is one and is not running.
fn listed(view: &RunsView, run: &str, what: &str) -> CmdResult<RunRow> {
    let row = row_of(view, run, what)?;
    if row.status == RunStatusUi::Running || view.active.as_deref() == Some(run) {
        return Err(CmdError::new(
            "RUN_ACTIVE",
            format!("{what}: run {} is still running", row.number),
        ));
    }
    Ok(row)
}

/// R2: sets (or, empty, clears) the label of `run`, one of the project at `project`'s runs under
/// `root`, and answers the runs as listed after it.
pub fn set_label(
    root: &Path,
    project: &Path,
    active: Option<&str>,
    run: &str,
    label: &str,
) -> CmdResult<RunsView> {
    let label = checked_label(label)?;
    let view = runs::list(root, project, active)?;
    listed(&view, run, "Label")?;
    let dir = root.join(run);
    let mut notes = read_notes(&dir).map_err(|e| {
        CmdError::new(
            "RUN_NOTES_INVALID",
            format!("the run's notes do not read, so they are not overwritten: {e}"),
        )
    })?;
    if notes.label == label {
        return Ok(view);
    }
    notes.label = label;
    write_notes(&dir, &notes)?;
    runs::list(root, project, active)
}

/// R4: the folder of `run`, one of the open project's runs (a running one too), that Explorer
/// is to open; refused for anything else, so no path but a listed run folder reaches Explorer.
pub fn folder_to_open(
    root: &Path,
    project: &Path,
    active: Option<&str>,
    run: &str,
) -> CmdResult<PathBuf> {
    let view = runs::list(root, project, active)?;
    row_of(&view, run, "Open folder")?;
    let dir = root.join(run);
    if !dir.is_dir() {
        return Err(CmdError::new(
            "RUN_NOT_FOUND",
            format!("Open folder: {} is not there", dir.display()),
        ));
    }
    Ok(std::path::absolute(&dir).unwrap_or(dir))
}

/// R4: opens `dir` in Explorer. Explorer answers 1 even when it opened the window, so only a
/// failure to start it is an error.
pub fn open_in_explorer(dir: &Path) -> CmdResult<()> {
    if !cfg!(windows) {
        return Err(CmdError::new(
            "OPEN_FOLDER_UNSUPPORTED",
            "opening a folder is done with Explorer, on Windows only",
        ));
    }
    std::process::Command::new("explorer.exe")
        .arg(dir)
        .spawn()
        .map(drop)
        .map_err(|e| {
            CmdError::new(
                "OPEN_FOLDER_FAILED",
                format!("Explorer did not start for {}: {e}", dir.display()),
            )
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(tag: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("nm-run-files-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    /// A project folder with two run folders that have no `run.json` (listed as Interrupted, so
    /// as this project's), one that is not a run, and the project's path.
    fn bed(tag: &str) -> (std::path::PathBuf, std::path::PathBuf, std::path::PathBuf) {
        let dir = scratch(tag);
        let project = dir.join("room.simpa");
        let root = runs::runs_root(&project);
        for name in [
            "20260101-000001-000-spps",
            "20260101-000002-000-tcr",
            "notes",
        ] {
            std::fs::create_dir_all(root.join(name)).unwrap();
        }
        (dir, project, root)
    }

    #[test]
    fn r2_a_label_is_kept_beside_run_json_and_listed_cleared_when_empty() {
        let (dir, project, root) = bed("label");
        let run = "20260101-000002-000-tcr";
        let view = set_label(&root, &project, None, run, "  Curtains closed  ").unwrap();
        let row = view.rows.iter().find(|r| r.run == run).unwrap();
        assert_eq!(row.label.as_deref(), Some("Curtains closed"));
        assert_eq!(
            view.rows.iter().find(|r| r.run != run).unwrap().label,
            None,
            "only the run labelled"
        );
        // The folder keeps its name; the note is a file beside where run.json goes.
        assert!(root.join(run).join(NOTES_FILE).is_file());
        assert!(!root.join(run).join(format!("{NOTES_FILE}.tmp")).exists());
        let view = set_label(&root, &project, None, run, " ").unwrap();
        assert_eq!(view.rows.iter().find(|r| r.run == run).unwrap().label, None);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn r2_a_label_is_refused_for_what_is_not_a_listed_run_or_is_running() {
        let (dir, project, root) = bed("refuse");
        let long = "x".repeat(LABEL_MAX_CHARS + 1);
        let code = |r: CmdResult<RunsView>| r.unwrap_err().code;
        assert_eq!(
            code(set_label(
                &root,
                &project,
                None,
                "20260101-000001-000-spps",
                &long
            )),
            "RUN_LABEL_INVALID"
        );
        assert_eq!(
            code(set_label(
                &root,
                &project,
                None,
                "20260101-000001-000-spps",
                "a\nb"
            )),
            "RUN_LABEL_INVALID"
        );
        assert_eq!(
            code(set_label(&root, &project, None, "notes", "x")),
            "RUN_NOT_FOUND"
        );
        assert_eq!(
            code(set_label(&root, &project, None, "..", "x")),
            "RUN_NOT_FOUND"
        );
        assert_eq!(
            code(set_label(
                &root,
                &project,
                Some("20260101-000001-000-spps"),
                "20260101-000001-000-spps",
                "x"
            )),
            "RUN_ACTIVE"
        );
        // A note that does not read is said on the row, and never overwritten.
        let run = "20260101-000002-000-tcr";
        std::fs::write(root.join(run).join(NOTES_FILE), "{").unwrap();
        let view = runs::list(&root, &project, None).unwrap();
        let row = view.rows.iter().find(|r| r.run == run).unwrap();
        assert!(
            row.notes_error
                .as_deref()
                .is_some_and(|e| e.contains(NOTES_FILE)),
            "{row:?}"
        );
        assert_eq!(
            code(set_label(&root, &project, None, run, "x")),
            "RUN_NOTES_INVALID"
        );
        assert_eq!(
            std::fs::read_to_string(root.join(run).join(NOTES_FILE)).unwrap(),
            "{"
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn r4_only_a_listed_run_folder_is_opened_a_running_one_too() {
        let (dir, project, root) = bed("open");
        let run = "20260101-000001-000-spps";
        let got = folder_to_open(&root, &project, Some(run), run).unwrap();
        assert!(got.is_absolute() && got.ends_with(run), "{got:?}");
        for bad in [
            "notes",
            "..",
            "20260101-000009-000-spps",
            "20260101-000001-000-spps/..",
        ] {
            assert_eq!(
                folder_to_open(&root, &project, None, bad).unwrap_err().code,
                "RUN_NOT_FOUND",
                "{bad}"
            );
        }
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
