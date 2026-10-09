//! Parity A20: the Help menu's destinations. The UI names a topic; the core turns it into a web
//! address and hands it to the shell, as R4 opens a run's folder (`run_files::open_in_explorer`),
//! so the webview holds no shell permission and no web address sits in the UI's bundle (m9 (f):
//! the dist carries no `http(s)://` outside its listed exceptions).
//!
//! Upstream's Help menu opens its online documentation and its website
//! (`docs/investigations/2026-10-09-parity-refresh/area/project-app.md`, rows A19 and A20); the
//! same two are here, with this app's own source, which the GPL asks to be within reach.
//!
//! Parity A22: the user manual (`manual/manual.html`, A21) ships inside app.exe ([`PAGES`]) and
//! opens the same way, written first to a folder of the app's on the temp drive ([`pages_dir`]).
//!
//! `$SIMPA_HELP_OPEN_LOG`, when set, names a file each target is appended to instead of being
//! opened: the e2e harness and the beds check what would open without a browser taking the
//! screen of whoever is at the machine.

use std::io::Write;
use std::path::{Path, PathBuf};

use crate::guard::{CmdError, CmdResult};

/// A topic the UI asked for that this build does not know.
pub const HELP_UNKNOWN: &str = "HELP_UNKNOWN";
/// The shell could not be started for the target.
pub const HELP_OPEN_FAILED: &str = "HELP_OPEN_FAILED";

/// The environment variable that turns opening into appending to a log (see the module docs).
pub const OPEN_LOG_ENV: &str = "SIMPA_HELP_OPEN_LOG";

/// Each web destination: the topic id the UI sends, and the address.
pub const LINKS: &[(&str, &str)] = &[
    // Upstream's user guide, as its own index names it (`Docs/index.rst`, the note on the
    // offline documentation).
    (
        "upstream-guide",
        "https://i-simpa-wiki.readthedocs.io/en/latest/",
    ),
    // Upstream's website (`Docs/index.rst`, "visit the offical I-Simpa website").
    ("upstream-site", "https://i-simpa.univ-gustave-eiffel.fr/"),
    // This app's source, GPL-3.0.
    (
        "source",
        "https://github.com/Burhanuddin98/I-Simpa_Night_Mode",
    ),
];

/// One page shipped inside app.exe (parity A22): the topic that opens it, its file name, its text.
pub struct Page {
    pub topic: &'static str,
    pub file: &'static str,
    pub text: &'static str,
}

/// The pages shipped with the app, embedded as the examples are (`examples.rs`): the app ships
/// as one executable plus `solvers/`, with no resource folder to find. Opening any of them
/// writes them all, so a link from one page to another resolves beside it.
pub const PAGES: &[Page] = &[
    Page {
        topic: "manual",
        file: "manual.html",
        text: include_str!("../manual/manual.html"),
    },
    // Parity A23: About's two texts. The GPL-3.0, this app's licence (the repository's LICENSE).
    Page {
        topic: "licence",
        file: "LICENSE.txt",
        text: include_str!("../../../LICENSE"),
    },
    // The work of others that app.exe and the solvers carry, written by
    // tools/devtools/third_party_notices.py.
    Page {
        topic: "notices",
        file: "THIRD-PARTY-NOTICES.txt",
        text: include_str!("../about/THIRD-PARTY-NOTICES.txt"),
    },
    // Parity A43: upstream's tutorials 1 to 3, each the text beside its example project
    // (examples.rs `tutorial-N`): what is set, what to do here, what upstream expects.
    Page {
        topic: "tutorial-1",
        file: "tutorial-1.html",
        text: include_str!("../manual/tutorial-1.html"),
    },
    Page {
        topic: "tutorial-2",
        file: "tutorial-2.html",
        text: include_str!("../manual/tutorial-2.html"),
    },
    Page {
        topic: "tutorial-3",
        file: "tutorial-3.html",
        text: include_str!("../manual/tutorial-3.html"),
    },
];

/// The pages could not be written where the browser is to read them.
pub const HELP_WRITE: &str = "HELP_WRITE";

/// What a topic opens.
#[derive(Debug, PartialEq, Eq)]
pub enum Target {
    Web(&'static str),
    /// A page of [`PAGES`], by its file name.
    Page(&'static str),
}

/// The target of `topic`, or HELP_UNKNOWN.
pub fn resolve(topic: &str) -> CmdResult<Target> {
    if let Some((_, url)) = LINKS.iter().find(|(id, _)| *id == topic) {
        return Ok(Target::Web(url));
    }
    if let Some(p) = PAGES.iter().find(|p| p.topic == topic) {
        return Ok(Target::Page(p.file));
    }
    Err(CmdError::new(
        HELP_UNKNOWN,
        format!("Help has no topic '{topic}' in this version"),
    ))
}

/// Where the pages are written for the browser: a folder of this app's on the temp drive,
/// rewritten on every open, so what opens is always this build's text.
pub fn pages_dir() -> PathBuf {
    std::env::temp_dir()
        .join("I-Simpa Night Mode")
        .join("manual")
}

/// Writes every page of [`PAGES`] into `dir` (made if missing), each through a temporary file
/// renamed over the old one, so a browser never reads half a page.
pub fn write_pages(dir: &Path) -> CmdResult<()> {
    let fail = |what: &Path, e: std::io::Error| {
        CmdError::new(
            HELP_WRITE,
            format!("could not write the manual's page {}: {e}", what.display()),
        )
    };
    std::fs::create_dir_all(dir).map_err(|e| fail(dir, e))?;
    for p in PAGES {
        let path = dir.join(p.file);
        let tmp = dir.join(format!("{}.tmp", p.file));
        std::fs::write(&tmp, p.text).map_err(|e| fail(&tmp, e))?;
        std::fs::rename(&tmp, &path).map_err(|e| {
            let _ = std::fs::remove_file(&tmp);
            fail(&path, e)
        })?;
    }
    Ok(())
}

/// Opens `topic`: the default browser on its address or its page (the pages written first), or,
/// with `$SIMPA_HELP_OPEN_LOG` set, a line appended to that file. Answers what was opened.
pub fn open(topic: &str) -> CmdResult<String> {
    let text = match resolve(topic)? {
        Target::Web(url) => url.to_string(),
        Target::Page(file) => {
            let dir = pages_dir();
            write_pages(&dir)?;
            dir.join(file).display().to_string()
        }
    };
    match std::env::var_os(OPEN_LOG_ENV) {
        Some(log) if !log.is_empty() => append_line(Path::new(&log), &text)?,
        _ => launch(&text)?,
    }
    Ok(text)
}

fn append_line(log: &Path, text: &str) -> CmdResult<()> {
    std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(log)
        .and_then(|mut f| writeln!(f, "{text}"))
        .map_err(|e| {
            CmdError::new(
                HELP_OPEN_FAILED,
                format!("could not record {text} in {}: {e}", log.display()),
            )
        })
}

/// The shell's own open: Explorer given an address or a file opens it with the program Windows
/// has for it (the default browser for a web page). Explorer answers 1 even when it opened it, so
/// only a failure to start it is an error.
fn launch(text: &str) -> CmdResult<()> {
    if !cfg!(windows) {
        return Err(CmdError::new(
            HELP_OPEN_FAILED,
            "Help opens pages through Explorer, on Windows only",
        ));
    }
    std::process::Command::new("explorer.exe")
        .arg(text)
        .spawn()
        .map(drop)
        .map_err(|e| {
            CmdError::new(
                HELP_OPEN_FAILED,
                format!("Explorer did not start for {text}: {e}"),
            )
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_link_resolves_to_its_address_and_ids_are_unique() {
        for (i, (id, url)) in LINKS.iter().enumerate() {
            assert_eq!(resolve(id).unwrap(), Target::Web(url));
            assert!(url.starts_with("https://"), "{id}: {url}");
            assert!(LINKS[i + 1..].iter().all(|(other, _)| other != id), "{id}");
        }
    }

    #[test]
    fn an_unknown_topic_is_refused_by_name() {
        let err = resolve("cheat-sheet").unwrap_err();
        assert_eq!(err.code, HELP_UNKNOWN);
        assert!(err.message.contains("cheat-sheet"), "{}", err.message);
    }

    #[test]
    fn each_page_resolves_and_is_a_whole_html_page_or_text() {
        for (i, p) in PAGES.iter().enumerate() {
            assert_eq!(resolve(p.topic).unwrap(), Target::Page(p.file));
            assert!(LINKS.iter().all(|(id, _)| *id != p.topic), "{}", p.topic);
            assert!(
                PAGES[i + 1..]
                    .iter()
                    .all(|o| o.topic != p.topic && o.file != p.file)
            );
            if p.file.ends_with(".html") {
                assert!(
                    p.text.trim_start().starts_with("<!doctype html>"),
                    "{}",
                    p.file
                );
                assert!(p.text.trim_end().ends_with("</html>"), "{}", p.file);
                // Nothing fetched: the page reads the same offline.
                for fetch in ["<script", "<img", "<link", "@import", "url("] {
                    assert!(!p.text.contains(fetch), "{} has {fetch}", p.file);
                }
            }
        }
        assert_eq!(resolve("manual").unwrap(), Target::Page("manual.html"));
    }

    /// A23: the licence is the GPL-3.0, and the notices name TetGen's AGPL-3.0 with its text and the
    /// solvers' GPL, as the repository's rules require of a release (CLAUDE.md, TetGen).
    #[test]
    fn about_texts_carry_the_licences_a_release_needs() {
        let page = |t: &str| PAGES.iter().find(|p| p.topic == t).unwrap().text;
        assert!(page("licence").starts_with("GNU GENERAL PUBLIC LICENSE"));
        assert!(page("licence").contains("Version 3, 29 June 2007"));
        let n = page("notices");
        for must in [
            "TetGen 1.5.0",
            "AGPL-3.0",
            "GNU AFFERO GENERAL PUBLIC LICENSE",
            "I-Simpa solvers",
            "GPL-3.0-or-later",
            "CC-BY-SA-4.0",
            "SIL Open Font License",
            "TinyXML-2",
            "2. Rust crates compiled into app.exe",
            "3. JavaScript packages in the interface",
        ] {
            assert!(n.contains(must), "THIRD-PARTY-NOTICES.txt lacks {must}");
        }
    }

    /// Every page is written whole, and a second write replaces the first in place.
    #[test]
    fn the_pages_are_written_and_rewritten_whole() {
        let dir = std::env::temp_dir().join(format!("nm-help-pages-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        write_pages(&dir).unwrap();
        std::fs::write(dir.join("manual.html"), "stale").unwrap();
        // Written as shipped: the same bytes as the embedded text.
        write_pages(&dir).unwrap();
        for p in PAGES {
            assert_eq!(std::fs::read_to_string(dir.join(p.file)).unwrap(), p.text);
        }
        let mut names: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .collect();
        names.sort();
        let mut want: Vec<_> = PAGES.iter().map(|p| p.file.to_string()).collect();
        want.sort();
        assert_eq!(names, want, "no temporary file is left");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// Every link between the pages names a page that ships, or a place on the same page.
    #[test]
    fn links_between_pages_resolve() {
        for p in PAGES.iter().filter(|p| p.file.ends_with(".html")) {
            for href in p.text.split("href=\"").skip(1) {
                let href = &href[..href.find('"').unwrap()];
                assert!(
                    !href.starts_with("http"),
                    "{}: {href} leaves the app",
                    p.file
                );
                let (file, anchor) = href.split_once('#').unwrap_or((href, ""));
                let target = if file.is_empty() {
                    p
                } else {
                    PAGES
                        .iter()
                        .find(|o| o.file == file)
                        .unwrap_or_else(|| panic!("{}: link to {href}", p.file))
                };
                if !anchor.is_empty() {
                    assert!(
                        target.text.contains(&format!("id=\"{anchor}\"")),
                        "{}: no #{anchor} in {}",
                        p.file,
                        target.file
                    );
                }
            }
        }
    }

    /// The log stands in for the shell: one line per open, in order, nothing launched.
    #[test]
    fn the_open_log_records_each_target() {
        let dir = std::env::temp_dir().join(format!("nm-help-log-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let log = dir.join("opened.txt");
        append_line(&log, "https://a.example/").unwrap();
        append_line(&log, "https://b.example/").unwrap();
        assert_eq!(
            std::fs::read_to_string(&log).unwrap(),
            "https://a.example/\nhttps://b.example/\n"
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
