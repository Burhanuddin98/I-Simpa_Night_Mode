//! The example projects shipped with the app (Burhan, 2026-10-06 01:24: "already set defaults
//! ready to run for each brass room and also the elmia hall so users can know what something
//! that is ready to simulate looks like"). The landing page offers them when no project is open.
//!
//! Each is a whole `.simpa` file embedded in the binary with `include_bytes!`, as the core embeds
//! the solver manifest: the app ships as one executable plus `solvers/`, with no bundle and so no
//! resource folder to find. Their sources and licences are in `examples/ATTRIBUTION.md`.
//!
//! Opening one never touches the embedded original and never overwrites a file: it writes a fresh
//! copy to `Documents\Night Mode\Examples\<name>.simpa` (`<name> (2).simpa`, `(3)`… when that
//! name is taken), then opens the copy exactly as File › Open opens a `.simpa`
//! ([`Session::scene_open`]). The copy is the user's: they may edit it, save it and run it, and
//! its runs land beside it.
//!
//! To add a room (BRAS CR1 and CR3 are next): put its `.simpa` in `examples/`, add a row to
//! [`EXAMPLES`] and to the landing page's list (`ui/src/chrome/landingModel.ts`), whose test holds
//! the two lists and the files to each other.

use std::fs::OpenOptions;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use crate::bridge::Session;
use crate::guard::{CmdError, CmdResult};
use crate::scene::SceneState;

/// One shipped example.
pub struct Example {
    /// The id the UI asks for (`example_open`).
    pub id: &'static str,
    /// The copy's file name, without `.simpa`.
    pub file_stem: &'static str,
    /// The `.simpa` file as shipped.
    pub bytes: &'static [u8],
}

/// Every shipped example, in the landing page's order.
pub const EXAMPLES: &[Example] = &[
    Example {
        id: "elmia",
        file_stem: "Elmia hall",
        bytes: include_bytes!("../examples/elmia_hall.simpa"),
    },
    Example {
        id: "bras-cr2",
        file_stem: "BRAS CR2",
        bytes: include_bytes!("../examples/bras_cr2.simpa"),
    },
    Example {
        id: "bras-cr4",
        file_stem: "BRAS CR4",
        bytes: include_bytes!("../examples/bras_cr4.simpa"),
    },
];

/// Unknown example ids are refused with this code.
pub const EXAMPLE_UNKNOWN: &str = "EXAMPLE_UNKNOWN";
/// The copy could not be written.
pub const EXAMPLE_WRITE: &str = "EXAMPLE_WRITE";

/// How many numbered names are tried before giving up: far more copies than anyone keeps.
const MAX_COPIES: u32 = 999;

pub fn find(id: &str) -> CmdResult<&'static Example> {
    EXAMPLES.iter().find(|e| e.id == id).ok_or_else(|| {
        CmdError::new(
            EXAMPLE_UNKNOWN,
            format!("no example '{id}' ships with this version"),
        )
    })
}

/// Where the copies go: `<Documents>\Night Mode\Examples`.
pub fn examples_dir(documents: &Path) -> PathBuf {
    documents.join("Night Mode").join("Examples")
}

/// The `n`th name for a copy: `<stem>.simpa`, then `<stem> (2).simpa`, `<stem> (3).simpa`…
pub fn copy_name(stem: &str, n: u32) -> String {
    if n <= 1 {
        format!("{stem}.simpa")
    } else {
        format!("{stem} ({n}).simpa")
    }
}

/// Writes `bytes` to the first free name in `dir` (created if missing) and returns its path. A
/// name is taken with `create_new`, so an existing file is never opened for writing, even one
/// that appears between two attempts. A copy that fails half-written is removed.
pub fn write_fresh_copy(dir: &Path, stem: &str, bytes: &[u8]) -> io::Result<PathBuf> {
    std::fs::create_dir_all(dir)?;
    for n in 1..=MAX_COPIES {
        let path = dir.join(copy_name(stem, n));
        let mut file = match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(f) => f,
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e),
        };
        if let Err(e) = file.write_all(bytes).and_then(|()| file.sync_all()) {
            drop(file);
            let _ = std::fs::remove_file(&path);
            return Err(e);
        }
        return Ok(path);
    }
    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        format!(
            "{MAX_COPIES} copies named {} already exist",
            copy_name(stem, 1)
        ),
    ))
}

/// The command's body: a fresh copy of example `id` under `documents`, opened in `session` as
/// File › Open opens it.
pub fn open_copy(session: &mut Session, documents: &Path, id: &str) -> CmdResult<SceneState> {
    let example = find(id)?;
    let dir = examples_dir(documents);
    let path = write_fresh_copy(&dir, example.file_stem, example.bytes).map_err(|e| {
        CmdError::new(
            EXAMPLE_WRITE,
            format!(
                "could not write a copy of the example into {}: {e}",
                dir.display()
            ),
        )
    })?;
    session.scene_open(&path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::IssueSeverity;

    /// A folder on the system temp drive (never the exFAT repo drive), removed by the caller.
    fn scratch(tag: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("simpa-app-examples-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn copy_names_count_up_from_the_bare_name() {
        assert_eq!(copy_name("BRAS CR4", 1), "BRAS CR4.simpa");
        assert_eq!(copy_name("BRAS CR4", 2), "BRAS CR4 (2).simpa");
        assert_eq!(copy_name("BRAS CR4", 13), "BRAS CR4 (13).simpa");
    }

    #[test]
    fn a_copy_never_overwrites_an_existing_file() {
        let dir = scratch("names");
        let first = write_fresh_copy(&dir, "Room", b"one").unwrap();
        assert_eq!(first, dir.join("Room.simpa"));
        // A file of the user's under the next name is skipped, not replaced.
        std::fs::write(dir.join("Room (2).simpa"), b"mine").unwrap();
        let third = write_fresh_copy(&dir, "Room", b"two").unwrap();
        assert_eq!(third, dir.join("Room (3).simpa"));
        assert_eq!(std::fs::read(&first).unwrap(), b"one");
        assert_eq!(std::fs::read(dir.join("Room (2).simpa")).unwrap(), b"mine");
        assert_eq!(std::fs::read(&third).unwrap(), b"two");
        // A gap left by a deleted copy is reused; nothing else moves.
        std::fs::remove_file(&first).unwrap();
        assert_eq!(write_fresh_copy(&dir, "Room", b"three").unwrap(), first);
        assert_eq!(std::fs::read(&third).unwrap(), b"two");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn the_folder_is_made_when_missing() {
        let docs = scratch("mkdir");
        let dir = examples_dir(&docs);
        assert!(!dir.exists());
        let p = write_fresh_copy(&dir, "Room", b"x").unwrap();
        assert_eq!(
            p,
            docs.join("Night Mode").join("Examples").join("Room.simpa")
        );
        std::fs::remove_dir_all(&docs).unwrap();
    }

    #[test]
    fn an_unknown_example_is_refused_and_writes_nothing() {
        let docs = scratch("unknown");
        let mut s = Session::default();
        let err = open_copy(&mut s, &docs, "cr9").unwrap_err();
        assert_eq!(err.code, EXAMPLE_UNKNOWN);
        assert!(!examples_dir(&docs).exists());
        assert!(s.info().is_none());
        std::fs::remove_dir_all(&docs).unwrap();
    }

    #[test]
    fn ids_and_names_are_unique() {
        for (i, a) in EXAMPLES.iter().enumerate() {
            for b in &EXAMPLES[i + 1..] {
                assert_ne!(a.id, b.id);
                assert_ne!(a.file_stem.to_lowercase(), b.file_stem.to_lowercase());
            }
        }
    }

    /// Each example opens through the command's path into a copy equal to what ships, and is
    /// ready to run: the model check passes, the validator reports no error, nothing blocks Run,
    /// and neither solver has an error of its own. Opening it again writes `(2)` and leaves the
    /// first copy as it was.
    #[test]
    fn every_example_opens_as_a_fresh_copy_ready_to_run() {
        let docs = scratch("open");
        for ex in EXAMPLES {
            let mut s = Session::default();
            let st = open_copy(&mut s, &docs, ex.id).unwrap();
            let first = examples_dir(&docs).join(copy_name(ex.file_stem, 1));
            assert_eq!(
                st.info.path.as_deref(),
                Some(first.display().to_string().as_str())
            );
            assert_eq!(std::fs::read(&first).unwrap(), ex.bytes, "{}", ex.id);
            assert!(st.check.is_some(), "{}: no model check", ex.id);
            let errors: Vec<_> = st
                .issues
                .iter()
                .filter(|i| i.severity == IssueSeverity::Error)
                .map(|i| &i.code)
                .collect();
            assert!(errors.is_empty(), "{}: validator errors {errors:?}", ex.id);
            assert!(
                st.run_blockers.is_empty(),
                "{}: {:?}",
                ex.id,
                st.run_blockers
            );
            for i in st.solver_issues.spps.iter().chain(&st.solver_issues.tcr) {
                assert_ne!(i.severity, IssueSeverity::Error, "{}: {}", ex.id, i.code);
            }
            assert!(
                st.info.sources > 0 && st.info.point_receivers > 0,
                "{}",
                ex.id
            );

            let again = open_copy(&mut s, &docs, ex.id).unwrap();
            let second = examples_dir(&docs).join(copy_name(ex.file_stem, 2));
            assert_eq!(
                again.info.path.as_deref(),
                Some(second.display().to_string().as_str())
            );
            assert_eq!(std::fs::read(&first).unwrap(), ex.bytes);
            assert_eq!(std::fs::read(&second).unwrap(), ex.bytes);
        }
        std::fs::remove_dir_all(&docs).unwrap();
    }
}
