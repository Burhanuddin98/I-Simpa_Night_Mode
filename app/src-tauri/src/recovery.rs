//! Parity A34, crash recovery: the open project's unsaved changes kept beside the app while it
//! runs, and offered back when it did not close.
//!
//! Upstream keeps each running instance's project in a cache folder of its own and, at the next
//! start, asks whether to retrieve an old session (i_simpa_main.h:474-524). Here the UI asks every
//! 30 s (`recovery_save`, its timer in App.tsx): while the project has unsaved changes, and they
//! moved since the last ask, it is written to `<key>.simpa` in the recovery folder, whole or not at
//! all, with `<key>.json` saying what it is (the project's own file, its name, when). Saved,
//! undone back to the file, replaced by New or Open, or let go at the save prompt on quitting, the
//! copy is removed. A copy is never the project's own file: restoring opens it as unsaved changes
//! to that file, and only Save writes there.
//!
//! Each instance holds `<key>.lock` open with no sharing for as long as it runs, so another
//! instance's copy is never offered while it runs (several instances, A35). A copy whose lock
//! opens is one whose instance ended without closing: crashed, killed, or the machine went off.
//! Those are listed for the landing page (`recovery_list`), to restore or discard.
//!
//! The folder is `SIMPA_RECOVERY_DIR` when set, else the app's local data folder's `recovery`;
//! under `--e2e` without `SIMPA_RECOVERY_DIR` there is none, so a test session neither writes into
//! the person's folder nor finds their copies.

use std::fs::{File, OpenOptions};
use std::path::{Path, PathBuf};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use simpa_core::schema;

use crate::bridge::Session;
use crate::guard::{CmdError, CmdResult};

/// What `<key>.json` says about a copy.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct Meta {
    /// The project's own file, `None` for a project never saved.
    path: Option<String>,
    name: String,
    /// When it was written, RFC 3339 in local time.
    saved_at: String,
}

/// A copy left by an instance that did not close, as the landing page offers it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, JsonSchema)]
pub struct RecoveryEntry {
    /// The copy's key (its file names); `recovery_restore` and `recovery_discard` take it.
    pub key: String,
    pub name: String,
    /// The project's own file, or none for a project never saved.
    pub path: Option<String>,
    /// When it was last written, RFC 3339 in local time.
    pub saved_at: String,
}

/// What one ask did.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum AutosaveStatus {
    /// No recovery folder (a test session): nothing kept.
    Off,
    /// Nothing unsaved: no copy (one there was removed).
    Clean,
    /// Unsaved changes, already kept as they are.
    Kept,
    /// Unsaved changes, written now.
    Written,
}

/// This instance's recovery folder, its key and its held lock.
pub struct Recovery {
    dir: Option<PathBuf>,
    key: String,
    _lock: Option<File>,
    /// The session's state serial and path the copy was last written for.
    written: Option<(u64, Option<PathBuf>)>,
}

/// `path` opened for writing with no sharing: held, nobody else can open it.
fn hold(path: &Path) -> std::io::Result<File> {
    let mut o = OpenOptions::new();
    o.create(true).truncate(false).write(true);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        o.share_mode(0);
    }
    o.open(path)
}

fn io_error(what: &str, path: &Path, e: &std::io::Error) -> CmdError {
    CmdError::new("RECOVERY_IO", format!("{what} {}: {e}", path.display()))
}

impl Recovery {
    /// The folder `dir` (made if missing), or none. A folder that cannot be made is none too, with
    /// the reason returned for the Console: recovery is a safety net, never a reason not to start.
    pub fn new(dir: Option<PathBuf>) -> (Recovery, Option<String>) {
        static MADE: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let key = format!(
            "{}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_millis()),
            MADE.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        );
        let off = |why: Option<String>| {
            (
                Recovery {
                    dir: None,
                    key: key.clone(),
                    _lock: None,
                    written: None,
                },
                why,
            )
        };
        let Some(dir) = dir else { return off(None) };
        if let Err(e) = std::fs::create_dir_all(&dir) {
            return off(Some(format!(
                "crash recovery is off: {} could not be made: {e}",
                dir.display()
            )));
        }
        match hold(&dir.join(format!("{key}.lock"))) {
            Ok(lock) => (
                Recovery {
                    dir: Some(dir),
                    key,
                    _lock: Some(lock),
                    written: None,
                },
                None,
            ),
            Err(e) => off(Some(format!(
                "crash recovery is off: its lock in {} could not be made: {e}",
                dir.display()
            ))),
        }
    }

    fn file(&self, dir: &Path, key: &str, ext: &str) -> PathBuf {
        let _ = self;
        dir.join(format!("{key}.{ext}"))
    }

    /// Removes this instance's copy, if any.
    fn remove_own(&mut self, dir: &Path) {
        for ext in ["simpa", "json"] {
            let _ = std::fs::remove_file(self.file(dir, &self.key, ext));
        }
        self.written = None;
    }

    /// The UI's ask: keeps the session's unsaved changes, or removes the copy when there are none.
    pub fn autosave(&mut self, session: &Session) -> CmdResult<AutosaveStatus> {
        let Some(dir) = self.dir.clone() else {
            return Ok(AutosaveStatus::Off);
        };
        // The save prompt's rule (flow.ts `needsSavePrompt`): unsaved changes to a file, or edits
        // or restored work in a project never saved; a new empty project or a fresh import is none.
        let Some(info) = session
            .info()
            .filter(|i| i.dirty && (i.path.is_some() || i.undo_depth > 0 || i.restored))
        else {
            self.remove_own(&dir);
            return Ok(AutosaveStatus::Clean);
        };
        let now = (
            session.state_serial(),
            session.path().map(Path::to_path_buf),
        );
        if self.written.as_ref() == Some(&now) {
            return Ok(AutosaveStatus::Kept);
        }
        let project = session
            .project()
            .ok_or_else(|| CmdError::new("NO_PROJECT", "no project is open"))?;
        let copy = self.file(&dir, &self.key, "simpa");
        schema::save(project, &copy).map_err(|e| {
            CmdError::new(
                "RECOVERY_IO",
                format!(
                    "could not keep the unsaved changes in {}: {e}",
                    copy.display()
                ),
            )
        })?;
        let meta = Meta {
            path: info.path.clone(),
            name: info.name.clone(),
            saved_at: simpa_core::run::clock::rfc3339(std::time::SystemTime::now()),
        };
        let path = self.file(&dir, &self.key, "json");
        let tmp = self.file(&dir, &self.key, "json.tmp");
        let text = serde_json::to_string_pretty(&meta)
            .map_err(|e| CmdError::new("RECOVERY_IO", format!("the note did not encode: {e}")))?;
        std::fs::write(&tmp, text)
            .and_then(|()| std::fs::rename(&tmp, &path))
            .map_err(|e| io_error("could not write", &path, &e))?;
        self.written = Some(now);
        Ok(AutosaveStatus::Written)
    }

    /// The copies of instances that ended without closing, newest first. A copy whose instance
    /// still runs (its lock held) is not one; a lock left with no copy is cleared.
    pub fn list(&self) -> Vec<RecoveryEntry> {
        let Some(dir) = &self.dir else {
            return Vec::new();
        };
        let Ok(read) = std::fs::read_dir(dir) else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for e in read.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            let Some(key) = name.strip_suffix(".lock") else {
                continue;
            };
            if key == self.key || !Self::ended(dir, key) {
                continue;
            }
            let meta = std::fs::read_to_string(self.file(dir, key, "json"))
                .ok()
                .and_then(|t| serde_json::from_str::<Meta>(&t).ok());
            match meta {
                Some(m) if self.file(dir, key, "simpa").is_file() => out.push(RecoveryEntry {
                    key: key.to_string(),
                    name: m.name,
                    path: m.path,
                    saved_at: m.saved_at,
                }),
                // A lock with no copy beside it: an instance that ended with nothing unsaved.
                _ if !self.file(dir, key, "simpa").exists() => {
                    let _ = std::fs::remove_file(e.path());
                    let _ = std::fs::remove_file(self.file(dir, key, "json"));
                }
                _ => {}
            }
        }
        out.sort_by(|a, b| b.saved_at.cmp(&a.saved_at));
        out
    }

    /// Whether the instance that holds `<key>.lock` has ended: its lock opens.
    fn ended(dir: &Path, key: &str) -> bool {
        hold(&dir.join(format!("{key}.lock"))).is_ok()
    }

    /// `key`, refused unless it is a copy [`Recovery::list`] offers.
    fn offered(&self, key: &str) -> CmdResult<(PathBuf, RecoveryEntry)> {
        let dir = self
            .dir
            .clone()
            .ok_or_else(|| CmdError::new("RECOVERY_NOT_FOUND", "crash recovery is off here"))?;
        let entry = self
            .list()
            .into_iter()
            .find(|e| e.key == key)
            .ok_or_else(|| {
                CmdError::new(
                    "RECOVERY_NOT_FOUND",
                    format!("no unsaved work '{key}' is waiting to be restored"),
                )
            })?;
        Ok((dir, entry))
    }

    /// Opens the copy `key` in `session` as unsaved changes to its project's own file, then
    /// removes the copy (this instance keeps the changes from here on).
    pub fn restore(&mut self, key: &str, session: &mut Session) -> CmdResult<()> {
        let (dir, entry) = self.offered(key)?;
        let copy = self.file(&dir, key, "simpa");
        let project = schema::load(&copy).map_err(|e| crate::bridge::load_error(&e))?;
        session.restore_recovered(
            project,
            entry.path.as_deref().map(PathBuf::from),
            &entry.saved_at,
        );
        for ext in ["simpa", "json", "lock"] {
            let _ = std::fs::remove_file(self.file(&dir, key, ext));
        }
        Ok(())
    }

    /// Removes the copy `key`: the person let it go.
    pub fn discard(&mut self, key: &str) -> CmdResult<()> {
        let (dir, _) = self.offered(key)?;
        for ext in ["simpa", "json"] {
            let p = self.file(&dir, key, ext);
            if let Err(e) = std::fs::remove_file(&p)
                && e.kind() != std::io::ErrorKind::NotFound
            {
                return Err(io_error("could not remove", &p, &e));
            }
        }
        let _ = std::fs::remove_file(self.file(&dir, key, "lock"));
        Ok(())
    }

    /// At a quit the person answered (saved, or let the changes go): this instance's copy goes.
    pub fn clear_own(&mut self) {
        if let Some(dir) = self.dir.clone() {
            self.remove_own(&dir);
        }
    }
}

/// The recovery folder: `SIMPA_RECOVERY_DIR`, else `<local data>/recovery`, none under `--e2e`
/// without the variable.
pub fn folder(
    env: Option<std::ffi::OsString>,
    e2e: bool,
    local_data: Option<PathBuf>,
) -> Option<PathBuf> {
    match env.filter(|v| !v.is_empty()) {
        Some(v) => Some(PathBuf::from(v)),
        None if e2e => None,
        None => local_data.map(|d| d.join("recovery")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use simpa_core::schema::{Op, Vec3};

    fn scratch(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("nm-recovery-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    fn fixture() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/rooms/tutorial1_box.simpa")
    }

    /// A session on a saved copy of the fixture in `dir`, one receiver moved: unsaved changes.
    fn edited(dir: &Path) -> (Session, PathBuf) {
        let own = dir.join("room.simpa");
        let mut s = Session::default();
        s.scene_open(&fixture()).unwrap();
        s.save(Some(&own)).unwrap();
        let r1 = s.project().unwrap().point_receivers[0].id;
        let op = Op::MovePointReceiver {
            id: r1,
            position: Vec3::new(4.0, 2.0, 1.8),
        };
        assert!(s.edit_apply(&op.to_json()).unwrap().applied);
        (s, own)
    }

    #[test]
    fn the_folder_is_the_variable_then_local_data_and_none_under_test_without_it() {
        let local = Some(PathBuf::from("L"));
        assert_eq!(
            folder(Some("X".into()), true, local.clone()),
            Some(PathBuf::from("X"))
        );
        assert_eq!(
            folder(None, false, local.clone()),
            Some(PathBuf::from("L").join("recovery"))
        );
        assert_eq!(folder(None, true, local.clone()), None);
        assert_eq!(folder(Some("".into()), true, local), None);
    }

    #[test]
    fn unsaved_changes_are_kept_once_and_go_when_saved() {
        let dir = scratch("keep");
        std::fs::create_dir_all(&dir).unwrap();
        let (mut s, own) = edited(&dir);
        let rdir = dir.join("recovery");
        let (mut r, why) = Recovery::new(Some(rdir.clone()));
        assert_eq!(why, None);
        assert_eq!(r.autosave(&s).unwrap(), AutosaveStatus::Written);
        assert_eq!(
            r.autosave(&s).unwrap(),
            AutosaveStatus::Kept,
            "nothing moved"
        );
        let copy = rdir.join(format!("{}.simpa", r.key));
        assert_eq!(
            schema::to_json(&schema::load(&copy).unwrap()),
            s.json().unwrap()
        );
        let meta: Meta = serde_json::from_str(
            &std::fs::read_to_string(rdir.join(format!("{}.json", r.key))).unwrap(),
        )
        .unwrap();
        assert_eq!(
            meta.path.as_deref(),
            Some(own.display().to_string().as_str())
        );
        assert!(
            r.list().is_empty(),
            "this instance's own copy is never offered"
        );
        s.save(None).unwrap();
        assert_eq!(r.autosave(&s).unwrap(), AutosaveStatus::Clean);
        assert!(!copy.exists(), "saved: the copy is gone");
        let (mut off, _) = Recovery::new(None);
        assert_eq!(off.autosave(&s).unwrap(), AutosaveStatus::Off);
        // A new empty project holds nothing of the person's (the save prompt does not ask either).
        let mut blank = Session::default();
        blank.scene_new("Untitled").unwrap();
        assert_eq!(r.autosave(&blank).unwrap(), AutosaveStatus::Clean);
        drop(r);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// Close project, the save prompt answered: the copy goes as at a quit (`scene_close`), and a
    /// session with nothing open keeps none.
    #[test]
    fn a_project_closed_keeps_no_copy() {
        let dir = scratch("close");
        std::fs::create_dir_all(&dir).unwrap();
        let (mut s, _) = edited(&dir);
        let rdir = dir.join("recovery");
        let (mut r, _) = Recovery::new(Some(rdir.clone()));
        assert_eq!(r.autosave(&s).unwrap(), AutosaveStatus::Written);
        let copy = rdir.join(format!("{}.simpa", r.key));
        assert!(copy.is_file());
        s.scene_close().unwrap();
        r.clear_own();
        assert!(!copy.exists(), "closed: the copy is gone");
        assert_eq!(r.autosave(&s).unwrap(), AutosaveStatus::Clean);
        assert!(!copy.exists());
        drop(r);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_copy_left_by_an_instance_that_ended_is_offered_restored_as_unsaved_changes_and_removed() {
        let dir = scratch("restore");
        std::fs::create_dir_all(&dir).unwrap();
        let (s, own) = edited(&dir);
        let rdir = dir.join("recovery");
        let edited_json = s.json().unwrap();
        let on_disk = std::fs::read(&own).unwrap();
        // The first instance writes its copy, and ends without closing (its lock let go, the copy left).
        let (mut first, _) = Recovery::new(Some(rdir.clone()));
        first.autosave(&s).unwrap();
        let key = first.key.clone();
        // While it runs, a second instance does not offer its copy.
        let (mut second, _) = Recovery::new(Some(rdir.clone()));
        assert!(
            second.list().is_empty(),
            "a running instance's copy is never offered"
        );
        drop(first);
        let offered = second.list();
        assert_eq!(offered.len(), 1, "{offered:?}");
        assert_eq!(offered[0].key, key);
        assert_eq!(
            offered[0].path.as_deref(),
            Some(own.display().to_string().as_str())
        );
        let mut fresh = Session::default();
        second.restore(&key, &mut fresh).unwrap();
        let info = fresh.info().unwrap();
        assert!(info.dirty && info.restored, "restored as unsaved changes");
        assert_eq!(
            info.path.as_deref(),
            Some(own.display().to_string().as_str())
        );
        assert_eq!(
            fresh.json().unwrap(),
            edited_json,
            "the changes back as they were"
        );
        assert_eq!(
            std::fs::read(&own).unwrap(),
            on_disk,
            "the project's own file untouched"
        );
        assert!(second.list().is_empty(), "restored: no longer offered");
        assert!(!rdir.join(format!("{key}.simpa")).exists());
        assert_eq!(
            second.restore(&key, &mut fresh).unwrap_err().code,
            "RECOVERY_NOT_FOUND"
        );
        drop(second);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_copy_let_go_is_removed_and_a_running_instances_copy_cannot_be() {
        let dir = scratch("discard");
        std::fs::create_dir_all(&dir).unwrap();
        let (s, _) = edited(&dir);
        let rdir = dir.join("recovery");
        let (mut first, _) = Recovery::new(Some(rdir.clone()));
        first.autosave(&s).unwrap();
        let key = first.key.clone();
        let (mut second, _) = Recovery::new(Some(rdir.clone()));
        assert_eq!(second.discard(&key).unwrap_err().code, "RECOVERY_NOT_FOUND");
        assert!(
            rdir.join(format!("{key}.simpa")).is_file(),
            "a running instance's copy stays"
        );
        drop(first);
        second.discard(&key).unwrap();
        assert!(!rdir.join(format!("{key}.simpa")).exists());
        assert!(second.list().is_empty());
        second.clear_own();
        drop(second);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
