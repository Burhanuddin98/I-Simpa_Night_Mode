//! Shared helpers of the CLI's integration tests: the built `simpa.exe` and
//! `simpa-stub-solver.exe`, fresh scratch folders, and `simpa-core`'s one finder of the solver
//! build and the fixtures (`crates/simpa-core/tests/common/paths.rs`), so a missing solver build
//! panics here too, naming where it looked.
#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

#[path = "../../../simpa-core/tests/common/paths.rs"]
pub mod paths;

#[allow(unused_imports)]
pub use paths::{fixture, solver_exe};

/// The `simpa.exe` cargo built for these tests.
pub fn simpa() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_simpa"))
}

/// The stub solver cargo built for these tests.
pub fn stub() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_simpa-stub-solver"))
}

/// A fresh, empty folder for one test under cargo's test scratch space. Never reused.
pub fn scratch(label: &str) -> PathBuf {
    static N: AtomicUsize = AtomicUsize::new(0);
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("cli")
        .join(format!(
            "{label}-{}-{}-{stamp}",
            std::process::id(),
            N.fetch_add(1, Ordering::SeqCst)
        ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// What one CLI call did.
#[derive(Debug)]
pub struct Out {
    pub code: i32,
    pub stdout: String,
    pub stderr: String,
    pub ms: f64,
}

/// Runs `simpa <args>` to completion.
pub fn simpa_run<S: AsRef<std::ffi::OsStr>>(args: &[S]) -> Out {
    let t0 = std::time::Instant::now();
    let out = Command::new(simpa())
        .args(args)
        .output()
        .unwrap_or_else(|e| panic!("{}: {e}", simpa().display()));
    Out {
        code: out.status.code().expect("simpa exited with a code"),
        stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
        ms: t0.elapsed().as_secs_f64() * 1e3,
    }
}

/// Parses `--json` stdout, with the call in the message when it is not JSON.
pub fn json(o: &Out) -> serde_json::Value {
    serde_json::from_str(&o.stdout).unwrap_or_else(|e| panic!("stdout is not JSON ({e}): {o:#?}"))
}

/// The run folder a `run`/`run-folder` manifest names: `cwd` is its `solve/`.
pub fn run_dir(manifest: &serde_json::Value) -> PathBuf {
    let cwd = manifest["cwd"].as_str().expect("cwd");
    Path::new(cwd).parent().unwrap().to_path_buf()
}

/// The verdict's reason codes.
pub fn codes(manifest: &serde_json::Value) -> Vec<String> {
    manifest["verdict"]["reasons"]
        .as_array()
        .expect("reasons")
        .iter()
        .map(|r| r["code"].as_str().unwrap().to_string())
        .collect()
}

/// Whether a process runs from the image `name` (`tasklist`, on every Windows).
pub fn image_running(name: &str) -> bool {
    let out = Command::new("tasklist")
        .args(["/FI", &format!("IMAGENAME eq {name}"), "/NH"])
        .output()
        .expect("tasklist runs");
    String::from_utf8_lossy(&out.stdout)
        .to_ascii_lowercase()
        .contains(&name.to_ascii_lowercase())
}

/// A copy of `exe` under a name no other process uses, so that a check for a leftover process
/// cannot be confused by a sibling test's solver.
pub fn private_copy(exe: &Path, dir: &Path, name: &str) -> PathBuf {
    let copy = dir.join(name);
    std::fs::copy(exe, &copy).unwrap();
    copy
}
