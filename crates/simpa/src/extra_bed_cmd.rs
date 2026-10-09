//! `simpa bed-extra r15|r20|r27 [--out <file>]`: M12c's closed-form beds of the numbers parity
//! added (`simpa_core::results::extra_bed`). The bed's cases go to stdout as JSON and, with
//! `--out`, to the file, with the commit they ran at and whether the tree was edited
//! (`hashes.head`, `hashes.src_uncommitted`: the shape `tools/bed/summary.py` reads a set's
//! commit from). Exit 0 only when every case holds; 8 when one does not; 2 usage.

use std::path::Path;
use std::process::ExitCode;

use simpa_core::results::extra_bed::{self, BedRun};

use crate::fail;

const EXIT_NOT_PASSED: u8 = 8;

/// `git rev-parse HEAD` and the last line of `git diff --stat HEAD` (empty for a clean tree) in
/// the folder the command runs from.
fn git() -> (Option<String>, Option<String>) {
    let run = |args: &[&str]| {
        std::process::Command::new("git")
            .args(args)
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
    };
    let head = run(&["rev-parse", "HEAD"]);
    let diff = run(&["diff", "--stat", "HEAD"]).map(|d| d.lines().last().unwrap_or("").to_string());
    (head, diff)
}

pub fn extra_bed_cmd(args: &[&str]) -> ExitCode {
    let mut which = None;
    let mut out = None;
    let mut it = args.iter().copied();
    while let Some(a) = it.next() {
        match a {
            "--out" => match it.next() {
                Some(p) => out = Some(p),
                None => return fail("--out needs a file"),
            },
            _ if a.starts_with("--") => return fail(&format!("unknown option '{a}'")),
            _ if which.is_none() => which = Some(a),
            _ => return fail(&format!("unexpected argument '{a}'")),
        }
    }
    let bed: BedRun = match which {
        Some("r15") => extra_bed::r15(),
        Some("r20") => extra_bed::r20(),
        Some(other) => return fail(&format!("no bed '{other}': r15, r20")),
        None => return fail("bed-extra needs a bed: simpa bed-extra r15 [--out <file>]"),
    };
    let (head, diff) = git();
    let mut v = serde_json::to_value(&bed).expect("a bed serialises");
    v["hashes"] = serde_json::json!({ "head": head, "src_uncommitted": diff });
    let text = serde_json::to_string_pretty(&v).expect("JSON") + "\n";
    print!("{text}");
    if let Some(p) = out {
        if let Some(dir) = Path::new(p).parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Err(e) = std::fs::write(p, &text) {
            return fail(&format!("{p}: {e}"));
        }
    }
    if bed.pass {
        ExitCode::SUCCESS
    } else {
        eprintln!(
            "simpa: bed {} did not pass: {} of {} cases do not hold",
            bed.bed,
            bed.failures,
            bed.cases.len()
        );
        ExitCode::from(EXIT_NOT_PASSED)
    }
}
