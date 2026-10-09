//! What the person does to one of the open project's run folders from the Runs tab (parity
//! M12c P2; upstream's result tree, `tree_rapport/e_report_file.cpp`):
//! - **R2, a label**: a name of the person's own beside `#n`, kept in `notes.json` beside
//!   `run.json`, never by renaming the folder (the folder's name is the run's key everywhere, and
//!   `is_run_name` is what keeps a path from climbing out of the runs root).
//! - **R4, open the folder**: Explorer on the run's folder, started from here, so the webview keeps
//!   no shell permission (capabilities/default.json: "No shell").
//! - **R3, delete**: the folder moved to the Recycle Bin, so a delete can be undone there, after
//!   the Runs tab's confirm. The files exported from the run (`export_write` with the run named)
//!   are noted beside it, so the confirm says which exported reports cite the run before it goes.
//!   What Windows cannot recycle (a network or removable drive, a bin turned off, a run larger
//!   than the bin holds) is refused with the run left in place, never deleted outright
//!   (`recycle`).
//!
//! `notes.json` is the person's, not the run's: `results::load` re-checks `run.json` and `solve/`
//! only, so a note never changes whether a run's results verify. A run is acted on only when it is
//! a row of the open project's Runs tab (`runs::list`), and changed never while it is the active
//! run.

use std::path::{Path, PathBuf};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::guard::{CmdError, CmdResult};
use crate::runs::{self, RunRow, RunStatusUi, RunsView};

/// The person's notes on a run, beside `run.json`.
pub const NOTES_FILE: &str = "notes.json";
/// The longest label, in characters.
pub const LABEL_MAX_CHARS: usize = 80;

/// R3: a file exported from a run (`export_write` with the run named): a report that cites it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RunExport {
    pub path: String,
    /// `csv`, `json`, `png` or `wav`.
    pub kind: String,
    /// When it was written, RFC 3339 in local time.
    pub at: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
struct Notes {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    label: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    exports: Vec<RunExport>,
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

/// Held across every read-modify-write of a `notes.json`: two exports noted at once (each
/// `export_write` runs on its own thread) would otherwise each read the old notes and the later
/// write would drop the other's note.
static NOTES_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn notes_lock() -> std::sync::MutexGuard<'static, ()> {
    // A panic while held leaves no half-written notes (`write_notes` renames whole files).
    NOTES_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Writes `notes` whole or not at all: a temporary file beside it, named for this write alone
/// (so no other write, another instance of the app's too, can rename it away), then renamed over
/// it.
fn write_notes(dir: &Path, notes: &Notes) -> CmdResult<()> {
    static WRITES: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let n = WRITES.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let path = dir.join(NOTES_FILE);
    let tmp = dir.join(format!("{NOTES_FILE}.{}-{n}.tmp", std::process::id()));
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

/// A row's label and exports from its folder's notes (`runs::list` calls this for every row).
pub fn annotate(row: &mut RunRow, dir: &Path) {
    match read_notes(dir) {
        Ok(n) => {
            row.label = n.label;
            row.exports = n.exports;
        }
        Err(e) => row.notes_error = Some(e),
    }
}

/// R3: refused unless `run` is one of the open project's runs: `export_write` asks before it
/// writes the file, so an export never names a run it cannot be noted beside.
pub fn check_export_run(
    root: &Path,
    project: &Path,
    active: Option<&str>,
    run: &str,
) -> CmdResult<()> {
    let view = runs::list(root, project, active)?;
    row_of(&view, run, "Note the export").map(drop)
}

/// R3: notes that `path` (of `kind`) was exported from `run`, one of the open project's runs: the
/// export a delete's confirm names. A file exported again replaces its older note.
pub fn record_export(
    root: &Path,
    project: &Path,
    active: Option<&str>,
    run: &str,
    path: &str,
    kind: &str,
) -> CmdResult<()> {
    check_export_run(root, project, active, run)?;
    let dir = root.join(run);
    let _held = notes_lock();
    let mut notes = read_notes(&dir).map_err(|e| {
        CmdError::new(
            "RUN_NOTES_INVALID",
            format!("the run's notes do not read, so they are not overwritten: {e}"),
        )
    })?;
    notes.exports.retain(|x| !x.path.eq_ignore_ascii_case(path));
    notes.exports.push(RunExport {
        path: path.to_string(),
        kind: kind.to_string(),
        at: simpa_core::run::clock::rfc3339(std::time::SystemTime::now()),
    });
    write_notes(&dir, &notes)
}

/// R3: moves `run`, one of the open project's runs and not the active one, to the Recycle Bin, and
/// answers the runs as listed after it. The Runs tab asks first and names what cites the run.
pub fn delete(root: &Path, project: &Path, active: Option<&str>, run: &str) -> CmdResult<RunsView> {
    let view = runs::list(root, project, active)?;
    listed(&view, run, "Delete")?;
    let dir = root.join(run);
    recycle(&std::path::absolute(&dir).unwrap_or(dir))?;
    runs::list(root, project, active)
}

/// Moves the folder `dir` to the Recycle Bin (`recycle`), or refuses with the run left in place:
/// never a permanent delete, whatever the drive or the size of the run.
fn recycle(dir: &Path) -> CmdResult<()> {
    match crate::recycle::to_recycle_bin(dir) {
        Ok(()) => Ok(()),
        Err(crate::recycle::Refusal::NotRecyclable(why)) => Err(CmdError::new(
            "RUN_NO_RECYCLE_BIN",
            format!(
                "nothing was deleted: Windows cannot put {} in the Recycle Bin ({why}), so it is \
                 left where it is; delete it by hand if it should go",
                dir.display()
            ),
        )),
        Err(crate::recycle::Refusal::Declined) => Err(CmdError::new(
            "RUN_DELETE_DECLINED",
            format!(
                "nothing was deleted: Windows asked whether to delete {} and the answer was No",
                dir.display()
            ),
        )),
        Err(crate::recycle::Refusal::Failed(why)) => Err(CmdError::new(
            "RUN_DELETE_FAILED",
            format!("the move to the Recycle Bin was not confirmed: {why}"),
        )),
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
    let held = notes_lock();
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
    drop(held);
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
        assert!(
            std::fs::read_dir(root.join(run)).unwrap().all(|e| !e
                .unwrap()
                .file_name()
                .to_string_lossy()
                .ends_with(".tmp")),
            "no temporary file left"
        );
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
    fn r3_an_export_is_noted_beside_its_run_once_per_file_and_listed() {
        let (dir, project, root) = bed("exports");
        let run = "20260101-000001-000-spps";
        set_label(&root, &project, None, run, "Hall").unwrap();
        record_export(&root, &project, None, run, "C:\\out\\a.csv", "csv").unwrap();
        record_export(&root, &project, None, run, "C:\\out\\b.png", "png").unwrap();
        // The same file again (another case): one note, the newer.
        record_export(&root, &project, None, run, "c:\\OUT\\A.csv", "csv").unwrap();
        let view = runs::list(&root, &project, None).unwrap();
        let row = view.rows.iter().find(|r| r.run == run).unwrap();
        let paths: Vec<&str> = row.exports.iter().map(|x| x.path.as_str()).collect();
        assert_eq!(paths, ["C:\\out\\b.png", "c:\\OUT\\A.csv"]);
        assert_eq!(
            row.label.as_deref(),
            Some("Hall"),
            "the label kept beside the exports"
        );
        assert!(
            row.exports.iter().all(|x| x.at.len() >= 23),
            "{:?}",
            row.exports
        );
        assert_eq!(
            record_export(&root, &project, None, "notes", "C:\\x.csv", "csv")
                .unwrap_err()
                .code,
            "RUN_NOT_FOUND"
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn r3_exports_noted_at_once_are_all_kept() {
        let (dir, project, root) = bed("exports-at-once");
        let run = "20260101-000001-000-spps";
        let paths: Vec<String> = (0..16)
            .map(|i| format!("C:\\out\\at-once-{i}.csv"))
            .collect();
        std::thread::scope(|s| {
            for p in &paths {
                let (root, project) = (&root, &project);
                s.spawn(move || record_export(root, project, None, run, p, "csv").unwrap());
            }
        });
        let view = runs::list(&root, &project, None).unwrap();
        let row = view.rows.iter().find(|r| r.run == run).unwrap();
        let mut got: Vec<&str> = row.exports.iter().map(|x| x.path.as_str()).collect();
        got.sort_unstable();
        let mut want: Vec<&str> = paths.iter().map(String::as_str).collect();
        want.sort_unstable();
        assert_eq!(got, want, "no note lost");
        let stray: Vec<_> = std::fs::read_dir(root.join(run))
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .filter(|n| n.ends_with(".tmp"))
            .collect();
        assert!(stray.is_empty(), "{stray:?}");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn r3_an_export_naming_a_run_that_is_not_listed_is_refused_before_it_is_written() {
        let (dir, project, root) = bed("export-check");
        assert_eq!(
            check_export_run(&root, &project, None, "notes")
                .unwrap_err()
                .code,
            "RUN_NOT_FOUND"
        );
        check_export_run(&root, &project, None, "20260101-000001-000-spps").unwrap();
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// R3 on a drive Windows cannot recycle on: the same bed reached through the drive's
    /// administrative share (`\\localhost\X$`, a network path, no Recycle Bin). Refused, and the
    /// run left whole.
    #[cfg(windows)]
    #[test]
    fn r3_delete_where_windows_cannot_recycle_is_refused_with_the_run_intact() {
        let (dir, _, _) = bed("delete-unc");
        let s = dir.to_str().unwrap();
        let (drive, rest) = s.split_once(":\\").unwrap();
        let unc = std::path::PathBuf::from(format!("\\\\localhost\\{drive}$\\{rest}"));
        assert!(
            unc.is_dir(),
            "{} is not reachable, so the refusal cannot be shown here",
            unc.display()
        );
        let project = unc.join("room.simpa");
        let root = runs::runs_root(&project);
        let run = "20260101-000001-000-spps";
        std::fs::write(root.join(run).join("kept.txt"), "kept").unwrap();
        let err = delete(&root, &project, None, run).unwrap_err();
        assert_eq!(err.code, "RUN_NO_RECYCLE_BIN", "{}", err.message);
        assert!(
            err.message.starts_with("nothing was deleted"),
            "{}",
            err.message
        );
        assert_eq!(
            std::fs::read_to_string(
                dir.join(root.strip_prefix(&unc).unwrap())
                    .join(run)
                    .join("kept.txt")
            )
            .unwrap(),
            "kept"
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn r3_delete_refuses_what_is_not_a_listed_run_and_the_active_run() {
        let (dir, project, root) = bed("delete");
        let code = |r: CmdResult<RunsView>| r.unwrap_err().code;
        for bad in ["notes", "..", "20260101-000009-000-spps"] {
            assert_eq!(
                code(delete(&root, &project, None, bad)),
                "RUN_NOT_FOUND",
                "{bad}"
            );
        }
        let run = "20260101-000001-000-spps";
        assert_eq!(code(delete(&root, &project, Some(run), run)), "RUN_ACTIVE");
        assert!(
            root.join(run).is_dir() && root.join("notes").is_dir(),
            "nothing moved"
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
