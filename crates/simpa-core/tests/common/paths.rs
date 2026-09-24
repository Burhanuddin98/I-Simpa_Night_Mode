// Where the tests find what the repository does not commit: the M1 solver build, an upstream
// source tree at the pinned commit, and the oracle built from that tree. Every test target that
// needs one of them goes through this file:
// - `mod common;` targets as `common::paths`;
// - the `*_support.rs` helpers with `#[path = "common/paths.rs"] mod paths;`;
// - unit tests under `src/` with `include!(concat!(env!("CARGO_MANIFEST_DIR"),
//   "/tests/common/paths.rs"))`, which is why this file has no inner attributes or `//!` docs.
//
// The locations:
// - `$SIMPA_SOLVERS_DIR`: the folder holding `spps.exe`, `classicalTheory.exe`, `tetgen.exe` and
//   `preprocess.exe`.
//   Default `<repo>/target/solvers/bin`, where `solvers/build.ps1` puts the M1 build.
// - `$SIMPA_UPSTREAM`: an upstream source tree at `simpa_core::SOLVER_COMMIT`, the folder holding
//   `src/`. Default `<repo>/target/solvers/src-929a5c8`, the archive `solvers/build.ps1`
//   extracts. A checkout of that commit (`B:\repos\I-Simpa-upstream`) serves as well.
// - the oracle: `<repo>/target/oracle/<format>/oracle.exe`, built on demand from that tree.
//
// Nothing here skips. A missing file panics, naming where it looked and how to provide it, so a
// test that needs one never passes without running.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;

/// The repository root: an absolute path with no `..` in it.
pub fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crates/simpa-core sits two levels below the root")
        .to_path_buf()
}

/// A path relative to the repository root.
pub fn repo_file(rel: &str) -> PathBuf {
    repo_root().join(rel)
}

/// A file under `tests/fixtures/`.
pub fn fixture(rel: &str) -> PathBuf {
    repo_root().join("tests/fixtures").join(rel)
}

/// `$SIMPA_SOLVERS_DIR`, else `<repo>/target/solvers/bin`. Not checked: see [`solver_exe`].
pub fn solvers_dir() -> PathBuf {
    match std::env::var_os("SIMPA_SOLVERS_DIR") {
        Some(d) if !d.is_empty() => PathBuf::from(d),
        _ => repo_root().join("target/solvers/bin"),
    }
}

/// `<solvers_dir>/<name>` from the M1 build. Panics, naming where it looked, when it is absent.
pub fn solver_exe(name: &str) -> PathBuf {
    let p = solvers_dir().join(name);
    assert!(
        p.is_file(),
        "{} is missing: this test runs the M1 solver build. Build it with \
         `powershell -File solvers/build.ps1` (into <repo>/target/solvers/bin), or set \
         SIMPA_SOLVERS_DIR to a folder holding spps.exe, classicalTheory.exe, tetgen.exe and \
         preprocess.exe",
        p.display()
    );
    p
}

/// `tetgen.exe` from the M1 build ([`solver_exe`]).
pub fn tetgen_exe() -> PathBuf {
    solver_exe("tetgen.exe")
}

/// Upstream's own TetGen 1.6.0, the pinned commit's `tetgen` target, which `solvers/build.ps1`
/// builds beside ours as the reference the parity bed's refusals need (never the mesher):
/// `$SIMPA_TETGEN160`, else `<solvers_dir>/../build/src/tetgen/Release/tetgen.exe`, the place
/// `tools/gates/parity.ps1` takes it from. Panics, naming where it looked, when it is absent, and
/// when its sha256 is not `solvers/manifest.json`'s `tetgen.upstream_160_reference_sha256`.
pub fn tetgen160_exe() -> PathBuf {
    let p = match std::env::var_os("SIMPA_TETGEN160") {
        Some(d) if !d.is_empty() => PathBuf::from(d),
        _ => solvers_dir()
            .parent()
            .map(|d| d.join("build/src/tetgen/Release/tetgen.exe"))
            .unwrap_or_default(),
    };
    assert!(
        p.is_file(),
        "{} is missing: this test runs upstream's TetGen 1.6.0 build as a refusal. Build it with \
         `powershell -File solvers/build.ps1`, or set SIMPA_TETGEN160 to it",
        p.display()
    );
    let manifest = std::fs::read_to_string(repo_file("solvers/manifest.json")).unwrap();
    let want = manifest
        .split("\"upstream_160_reference_sha256\"")
        .nth(1)
        .and_then(|s| s.split('"').nth(1))
        .expect("solvers/manifest.json names the TetGen 1.6.0 reference's sha256")
        .to_string();
    let out = Command::new("certutil")
        .args(["-hashfile", &p.display().to_string(), "SHA256"])
        .output()
        .expect("certutil runs");
    let text = String::from_utf8_lossy(&out.stdout).to_lowercase();
    assert!(
        text.lines().any(|l| l.trim().replace(' ', "") == want),
        "{} is not solvers/manifest.json's TetGen 1.6.0 reference (sha256 {want}): {text}",
        p.display()
    );
    p
}

/// The upstream source tree: `$SIMPA_UPSTREAM`, else `<repo>/target/solvers/src-929a5c8`. Panics
/// when it has no `src/lib_interface` folder.
pub fn upstream_root() -> PathBuf {
    let root = match std::env::var_os("SIMPA_UPSTREAM") {
        Some(d) if !d.is_empty() => PathBuf::from(d),
        _ => repo_root().join("target/solvers/src-929a5c8"),
    };
    assert!(
        root.join("src/lib_interface").is_dir(),
        "{} is not an upstream source tree (no src/lib_interface): this test reads upstream's \
         sources at the pinned commit. Extract them with `powershell -File solvers/build.ps1`, \
         or set SIMPA_UPSTREAM to a checkout of that commit (for example \
         B:\\repos\\I-Simpa-upstream)",
        root.display()
    );
    root
}

/// A file of the upstream source tree, `rel` from its root (`src/spps/...`). Panics when it is
/// absent.
pub fn upstream_file(rel: &str) -> PathBuf {
    let p = upstream_root().join(rel);
    assert!(
        p.is_file(),
        "{} is missing from the upstream source tree",
        p.display()
    );
    p
}

/// The oracle of `format` (`oracle/dump_<format>.cpp` over upstream's own reader), built into
/// `<repo>/target/oracle/<format>/` by `oracle/build.ps1 -Only <format> -UpstreamSrc <src>`
/// when it is missing, older than its sources, or does not list `format`. Panics when it cannot
/// be built, with the build's output.
pub fn oracle(format: &str) -> PathBuf {
    static BUILT: Mutex<Vec<(String, PathBuf)>> = Mutex::new(Vec::new());
    // Held across the build: two tests of one binary never build the same oracle at once.
    let mut built = BUILT.lock().unwrap_or_else(|e| e.into_inner());
    if let Some((_, exe)) = built.iter().find(|(f, _)| f == format) {
        return exe.clone();
    }
    let repo = repo_root();
    let exe = repo.join(format!("target/oracle/{format}/oracle.exe"));
    let modified = |p: &Path| std::fs::metadata(p).and_then(|m| m.modified()).ok();
    let sources = [
        format!("oracle/dump_{format}.cpp"),
        "oracle/common.hpp".to_string(),
        "oracle/main.cpp".to_string(),
        "oracle/std_tools_nob.cpp".to_string(),
        "oracle/build.ps1".to_string(),
    ];
    assert!(
        repo.join(&sources[0]).is_file(),
        "no {} to build an oracle for '{format}' from",
        sources[0]
    );
    let stale = modified(&exe).is_none_or(|built| {
        sources
            .iter()
            .any(|s| modified(&repo.join(s)).is_none_or(|t| t > built))
    });
    let lists_format = || {
        Command::new(&exe).arg("list").output().is_ok_and(|o| {
            String::from_utf8_lossy(&o.stdout)
                .lines()
                .any(|l| l.trim() == format)
        })
    };
    if stale || !lists_format() {
        let src = upstream_root().join("src");
        let out = Command::new("powershell")
            .args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-File"])
            .arg(repo.join("oracle/build.ps1"))
            .args(["-Only", format, "-UpstreamSrc"])
            .arg(&src)
            .output()
            .unwrap_or_else(|e| panic!("cannot run oracle/build.ps1: {e}"));
        assert!(
            out.status.success(),
            "oracle/build.ps1 -Only {format} -UpstreamSrc {} failed ({}):\n{}{}",
            src.display(),
            out.status,
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
    }
    assert!(
        lists_format(),
        "{} does not list '{format}' after its build",
        exe.display()
    );
    built.push((format.to_string(), exe.clone()));
    exe
}
