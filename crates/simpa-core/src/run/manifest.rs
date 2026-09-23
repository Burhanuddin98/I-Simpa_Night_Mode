//! `run.json`: the record of one solver run, written beside the run's `solve/` folder.
//!
//! It records what ran (source, solver, executable and its hash, argv, cwd), on what (the input
//! files' hashes and the mesh), what happened (the process outcome and the lines per class) and
//! the verdict, with the loss limit it was judged by. Nothing here reads a clock: the start time
//! is whatever the caller passes. Unknown fields are refused on reading, so a manifest from a
//! different layout fails loudly instead of losing fields.

use std::io::{self, Read};
use std::path::Path;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::classify::ClassCounts;
use super::expect::normalize;
use super::verdict::{Outputs, Verdict};
use crate::process::Outcome;
use crate::schema::SolverKind;

/// The file name, beside `solve/`.
pub const FILE_NAME: &str = "run.json";

/// The layout of [`RunManifest`]; bumped when a field changes meaning.
pub const MANIFEST_VERSION: u32 = 1;

/// A file and its sha256.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FileRef {
    /// As the run saw it: absolute for the executable, relative to `solve/` for an input.
    pub path: String,
    /// Lower-case hex.
    pub sha256: String,
}

/// What was run.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum RunSource {
    /// `simpa run <project>`: the project file, the variant written, and the project's hash.
    Project {
        path: String,
        sha256: String,
        variant: Option<String>,
    },
    /// `simpa run-folder <fixture-dir>`: a folder copied in as it is, validator bypassed.
    Fixture { path: String },
}

/// The mesh the run used.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MeshRef {
    /// The mesh manifest (`mesh.json`) the `.mbin` came from; `None` for a fixture's own mesh.
    pub manifest: Option<String>,
    /// The mesh input stamp (`validate::mesh_input_hash`) that manifest records.
    pub mesh_input_hash: Option<String>,
    /// The sha256 of the `.mbin` in `solve/`, checked against the manifest before launch.
    pub mbin_sha256: String,
}

/// Files in `solve/` after the run.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FileCounts {
    /// Every file under `solve/`, inputs included.
    pub total: usize,
    /// The files the config asked for (`Expectation::expected_files`).
    pub expected: usize,
    /// Of those, the ones present and non-empty.
    pub present: usize,
}

impl FileCounts {
    /// Counts `outputs` against the `expected` list
    /// ([`Expectation::expected_files`](super::expect::Expectation::expected_files)).
    pub fn of(expected: &[String], outputs: &Outputs) -> Self {
        FileCounts {
            total: outputs.files.len(),
            expected: expected.len(),
            present: expected
                .iter()
                .filter(|p| outputs.files.get(*p).is_some_and(|&n| n > 0))
                .count(),
        }
    }
}

/// Every file under `dir` with its sha256, keyed by its [`normalize`]d relative path, sorted:
/// the run's inputs, hashed in `solve/` immediately before launch.
pub fn hash_folder(dir: &Path) -> io::Result<Vec<FileRef>> {
    fn walk(root: &Path, dir: &Path, out: &mut Vec<FileRef>) -> io::Result<()> {
        for e in std::fs::read_dir(dir)? {
            let e = e?;
            let path = e.path();
            let kind = e.file_type()?;
            if kind.is_dir() {
                walk(root, &path, out)?;
            } else if kind.is_file() {
                let rel = path.strip_prefix(root).unwrap_or(&path);
                out.push(FileRef {
                    path: normalize(&rel.to_string_lossy()),
                    sha256: sha256_file(&path)?,
                });
            }
        }
        Ok(())
    }
    let mut out = Vec::new();
    walk(dir, dir, &mut out)?;
    out.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(out)
}

/// `run.json`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunManifest {
    /// [`MANIFEST_VERSION`].
    pub manifest_version: u32,
    /// `simpa_core::VERSION`.
    pub core_version: String,
    /// The upstream commit the solvers are built from (`simpa_core::SOLVER_COMMIT`).
    pub solver_commit: String,
    pub source: RunSource,
    pub solver: SolverKind,
    /// The solver executable, absolute, and its sha256.
    pub exe: FileRef,
    /// The arguments after the program name: always `["config.xml"]` (contract Part B,
    /// "Launch").
    pub argv: Vec<String>,
    /// The child's working directory, absolute: the run's `solve/` folder.
    pub cwd: String,
    /// When the child was started, as the caller gives it (RFC 3339, local time with offset).
    pub started: String,
    /// The input files in `solve/` before launch (`config.xml`, the `.cbin`, the `.mbin`,
    /// directivity files), relative to `solve/`, with their hashes.
    pub inputs: Vec<FileRef>,
    pub mesh: Option<MeshRef>,
    pub outcome: Outcome,
    /// Lines per class, both streams together.
    pub lines: ClassCounts,
    pub files: FileCounts,
    /// The loss limit the verdict used (`docs/m5-m6-design.md`, decision 9).
    pub loss_limit: f64,
    pub verdict: Verdict,
}

impl RunManifest {
    /// Pretty JSON with a final newline.
    pub fn to_json(&self) -> String {
        let mut s = serde_json::to_string_pretty(self).expect("a manifest always serialises");
        s.push('\n');
        s
    }

    pub fn from_json(text: &str) -> serde_json::Result<RunManifest> {
        serde_json::from_str(text)
    }
}

/// The sha256 of a file, in lower-case hex.
pub fn sha256_file(path: &Path) -> io::Result<String> {
    let mut f = std::fs::File::open(path)?;
    let mut h = Sha256::new();
    let mut buf = vec![0u8; 1 << 16];
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Ok(hex(&h.finalize()))
}

/// The sha256 of bytes, in lower-case hex.
pub fn sha256_bytes(bytes: &[u8]) -> String {
    hex(&Sha256::digest(bytes))
}

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write;
    bytes.iter().fold(String::with_capacity(64), |mut s, b| {
        let _ = write!(s, "{b:02x}");
        s
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha256_known_answers() {
        assert_eq!(
            sha256_bytes(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            sha256_bytes(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }
}
