//! The solver fingerprint the tests hold a build to: the code sha256 (`solvers/pe-fingerprint.ps1`,
//! the sha256 with the link timestamps zeroed), through `common/paths.rs`. The parity bed finds
//! upstream's TetGen 1.6.0 reference by it ([`paths::tetgen160_exe`]); these tests show that the
//! check keeps accepting the reference once relinked, as every rebuild relinks it, and says no to
//! our TetGen, to one changed code byte and to a file it defines no code sha256 for.
//!
//! Needs the M1 build and its TetGen 1.6.0 reference (`common/paths.rs`); missing, they fail.

#[allow(dead_code)]
#[path = "common/paths.rs"]
mod paths;
#[allow(dead_code)]
#[path = "common/scratch.rs"]
mod scratch;

use std::path::{Path, PathBuf};
use std::process::Command;

/// A copy of `from` made by a `solvers/pe-fingerprint.ps1` function (`Copy-PeRestamped`,
/// `Copy-PeFlippedText`), at `<tmp>/solver_fingerprint/<label>-<unique>/<file name>`: removed
/// when the test passes, kept when it fails (`common/scratch.rs`).
fn ps_copy(from: &Path, label: &str, function: &str, extra: &str) -> PathBuf {
    let dir = scratch::fresh("solver_fingerprint", label);
    let to = dir.join(from.file_name().unwrap());
    let quote = |p: &Path| format!("'{}'", p.display().to_string().replace('\'', "''"));
    let out = Command::new("powershell")
        .args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-Command"])
        .arg(format!(
            "$ErrorActionPreference = 'Stop'; . {}; $null = {function} {} {} {extra}",
            quote(&paths::repo_file("solvers/pe-fingerprint.ps1")),
            quote(from),
            quote(&to)
        ))
        .output()
        .expect("powershell runs");
    assert!(
        out.status.success(),
        "{function} {} failed: {}",
        from.display(),
        String::from_utf8_lossy(&out.stderr)
    );
    to
}

#[test]
fn the_tetgen_160_reference_is_found_and_our_tetgen_is_refused() {
    let reference = paths::tetgen160_exe();
    assert_eq!(paths::tetgen160_mismatch(&reference), None);
    let ours = paths::tetgen_exe();
    let why = paths::tetgen160_mismatch(&ours).expect("our TetGen 1.5.0 is refused");
    assert!(
        why.contains("is not solvers/manifest.json's TetGen 1.6.0 reference"),
        "{why}"
    );
}

#[test]
fn a_relinked_reference_is_still_the_reference_and_one_changed_code_byte_is_not() {
    let reference = paths::tetgen160_exe();
    // Every link-time field set to 2000-01-01 00:00:00 UTC: the same code, linked at another time.
    let relinked = ps_copy(&reference, "relinked", "Copy-PeRestamped", "946684800");
    assert_ne!(
        std::fs::read(&relinked).unwrap(),
        std::fs::read(&reference).unwrap(),
        "the relinked copy has other bytes"
    );
    assert_eq!(paths::tetgen160_mismatch(&relinked), None);
    assert_eq!(
        paths::code_sha256(&relinked).unwrap(),
        paths::manifest_string("upstream_160_reference_code_sha256")
    );
    let flipped = ps_copy(&reference, "flipped", "Copy-PeFlippedText", "");
    let why = paths::tetgen160_mismatch(&flipped).expect("a changed code byte is refused");
    assert!(why.contains("code sha256"), "{why}");
}

#[test]
fn code_sha256_refuses_a_file_that_is_not_a_pe_executable() {
    let not_pe = paths::repo_file("solvers/manifest.json");
    let why = paths::code_sha256(&not_pe).expect_err("a JSON file has no code sha256");
    assert!(why.contains("no MZ signature"), "{why}");
    assert!(paths::tetgen160_mismatch(&not_pe).is_some());
}
