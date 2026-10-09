//! Parity A20: the Help menu's destinations. The UI names a topic; the core turns it into a web
//! address and hands it to the shell, as R4 opens a run's folder (`run_files::open_in_explorer`),
//! so the webview holds no shell permission and no web address sits in the UI's bundle (m9 (f):
//! the dist carries no `http(s)://` outside its listed exceptions).
//!
//! Upstream's Help menu opens its online documentation and its website
//! (`docs/investigations/2026-10-09-parity-refresh/area/project-app.md`, rows A19 and A20); the
//! same two are here, with this app's own source, which the GPL asks to be within reach.
//!
//! `$SIMPA_HELP_OPEN_LOG`, when set, names a file each target is appended to instead of being
//! opened: the e2e harness and the beds check what would open without a browser taking the
//! screen of whoever is at the machine.

use std::io::Write;
use std::path::Path;

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

/// What a topic opens.
#[derive(Debug, PartialEq, Eq)]
pub enum Target {
    Web(&'static str),
}

impl Target {
    /// What the shell is given, and what the command answers.
    pub fn text(&self) -> String {
        match self {
            Target::Web(url) => (*url).to_string(),
        }
    }
}

/// The target of `topic`, or HELP_UNKNOWN.
pub fn resolve(topic: &str) -> CmdResult<Target> {
    if let Some((_, url)) = LINKS.iter().find(|(id, _)| *id == topic) {
        return Ok(Target::Web(url));
    }
    Err(CmdError::new(
        HELP_UNKNOWN,
        format!("Help has no topic '{topic}' in this version"),
    ))
}

/// Opens `topic`: the default browser on its address, or, with `$SIMPA_HELP_OPEN_LOG` set, a
/// line appended to that file. Answers what was opened.
pub fn open(topic: &str) -> CmdResult<String> {
    let target = resolve(topic)?;
    let text = target.text();
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
