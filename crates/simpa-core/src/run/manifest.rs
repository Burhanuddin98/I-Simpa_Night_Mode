//! `run.json`: the record of one solver run, written beside the run's `solve/` folder.
//!
//! It records what ran (source, solver, executable and its hash, argv, cwd), on what (the input
//! files' hashes and the mesh), how far it got (the stage it ended in and the CLI exit class),
//! what happened (the process outcome, the lines per class, SPPS's particle statistics) and the
//! verdict, with the loss limit it was judged by. The run manager
//! ([`super::manager`]) writes one for every run folder it creates, refused or launched. Nothing
//! here reads a clock: the start time is whatever the caller passes. Unknown fields are refused
//! on reading, so a manifest from a different layout fails loudly instead of losing fields.

use std::io::{self, Read};
use std::path::Path;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::classify::ClassCounts;
use super::expect::normalize;
use super::manager::{ExitClass, Stage};
use super::stats::ParticleStats;
use super::verdict::{Outputs, Verdict};
use crate::bed::pe::SolverCheck;
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
        /// The project file, as the command line gave it.
        path: String,
        /// The project file's sha256, lower-case hex, read when the run started.
        sha256: String,
        /// The variant whose `config.xml` was written; `None` for the project's own settings.
        variant: Option<String>,
    },
    /// `simpa run-folder <fixture-dir>`: a folder copied in as it is, validator bypassed.
    Fixture {
        /// The fixture folder, as the command line gave it.
        path: String,
    },
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

/// A file a run left in its folder: its path relative to the run folder, `/`-separated, its size
/// in bytes and its sha256 (lower-case hex).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OutputRef {
    pub path: String,
    pub size: u64,
    pub sha256: String,
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

/// Every file under `dir`, recursively, except the top-level entries named in `except`, with its
/// size and sha256, keyed by its path relative to `dir` with `/` between its parts, sorted by
/// path: what a run left ([`RunManifest::outputs`]), and what the M8 bed hashes again to hold a
/// run to it. Anything that is neither a file nor a folder (a link, say), and a name that is not
/// UTF-8, is an error, never skipped: a file that cannot be hashed must not go unbound.
pub fn hash_tree(dir: &Path, except: &[&str]) -> io::Result<Vec<OutputRef>> {
    fn walk(root: &Path, dir: &Path, except: &[&str], out: &mut Vec<OutputRef>) -> io::Result<()> {
        for e in std::fs::read_dir(dir)? {
            let e = e?;
            let path = e.path();
            let rel = path.strip_prefix(root).unwrap_or(&path);
            let rel = rel.to_str().ok_or_else(|| {
                io::Error::other(format!("{}: the name is not UTF-8", path.display()))
            })?;
            if dir == root && except.contains(&rel) {
                continue;
            }
            let kind = e.file_type()?;
            if kind.is_dir() {
                walk(root, &path, except, out)?;
            } else if kind.is_file() {
                let (sha256, size) = sha256_and_size(&path)?;
                out.push(OutputRef {
                    path: rel.split(['\\', '/']).collect::<Vec<_>>().join("/"),
                    size,
                    sha256,
                });
            } else {
                return Err(io::Error::other(format!(
                    "{}: neither a file nor a folder",
                    path.display()
                )));
            }
        }
        Ok(())
    }
    let mut out = Vec::new();
    walk(dir, dir, except, &mut out)?;
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
    /// What was run: a project, or a fixture folder with the validator bypassed.
    pub source: RunSource,
    /// Which solver: `spps` or `tcr`.
    pub solver: SolverKind,
    /// The solver executable, absolute, and its sha256.
    pub exe: FileRef,
    /// The executables checked against the verified build (`solvers/manifest.json`) before the
    /// run, when the caller asked for it (`RunOptions::verify`; the desktop app always does):
    /// the solver, and with a mesh built in the run, `tetgen.exe` and `preprocess.exe`. `None`
    /// when no check was asked for (the CLI, the bed, a manifest written before M11): absent from
    /// the file, so those manifests are byte for byte what they were.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub solvers: Option<Vec<SolverCheck>>,
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
    /// The mesh the run used; `None` when the run ended before one was chosen.
    pub mesh: Option<MeshRef>,
    /// The stage the run ended in: `solve` once the solver was launched, otherwise the stage
    /// that refused it.
    pub stage: Stage,
    /// The CLI's exit code for this run (`docs/m5-m6-design.md`, "Exit codes"): 0, 2, 3, 4, 5
    /// or 130.
    pub exit_class: ExitClass,
    /// How the process ended: exit code as a raw `u32` (`None` when there was none), whether it
    /// was cancelled, and its wall time in ms. `None` when the solver was never launched.
    pub outcome: Option<Outcome>,
    /// Lines per class, both streams together.
    pub lines: ClassCounts,
    /// Files in `solve/` after the run, against the expected list.
    pub files: FileCounts,
    /// Every file in the run folder when the run ended but `run.json` itself ([`hash_tree`]): the
    /// solver's outputs, its inputs in `solve/` as they then stood, its logs, and for a run that
    /// meshed, `mesh/` (TetGen's outputs and `mesh.json`), each with its size and sha256. The M8
    /// bed reads a run only when every file in its folder still has these, no file gone and none
    /// added (`bed::bind`). `None` in a manifest written before M8b, which has no such key and so
    /// reads and writes back byte for byte, and when the folder could not be read at the end.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub outputs: Option<Vec<OutputRef>>,
    /// SPPS's particle statistics per band, when the table was read.
    pub particles: Option<ParticleStats>,
    /// The loss limit the verdict used (`docs/m5-m6-design.md`, decision 9). JSON has no NaN or
    /// infinity, so those are written as the strings `"NaN"`, `"inf"` and `"-inf"`.
    #[serde(with = "any_f64")]
    pub loss_limit: f64,
    /// The judgement: status, reasons and the WARN lines ([`Verdict`]).
    pub verdict: Verdict,
}

/// An `f64` as a JSON number when finite, otherwise as `"NaN"`, `"inf"` or `"-inf"`. serde_json
/// writes a non-finite `f64` as `null`, which does not read back into an `f64`.
mod any_f64 {
    use serde::de::Error as _;
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(v: &f64, s: S) -> Result<S::Ok, S::Error> {
        if v.is_finite() {
            s.serialize_f64(*v)
        } else if v.is_nan() {
            s.serialize_str("NaN")
        } else if *v > 0.0 {
            s.serialize_str("inf")
        } else {
            s.serialize_str("-inf")
        }
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<f64, D::Error> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Repr {
            Number(f64),
            Text(String),
        }
        match Repr::deserialize(d)? {
            Repr::Number(v) => Ok(v),
            Repr::Text(t) => match t.as_str() {
                "NaN" => Ok(f64::NAN),
                "inf" => Ok(f64::INFINITY),
                "-inf" => Ok(f64::NEG_INFINITY),
                other => Err(D::Error::custom(format!(
                    "{other:?} is not a number, \"NaN\", \"inf\" or \"-inf\""
                ))),
            },
        }
    }
}

impl RunManifest {
    /// Pretty JSON with a final newline.
    pub fn to_json(&self) -> String {
        let mut s = serde_json::to_string_pretty(self).expect("a manifest always serialises");
        s.push('\n');
        s
    }

    /// Reads a manifest with the crate's exact JSON reader ([`crate::schema::parse_json`]):
    /// serde_json's own reader is not correctly rounded, so a time would not read back as
    /// written.
    pub fn from_json(text: &str) -> serde_json::Result<RunManifest> {
        use serde::de::Error as _;
        let value = crate::schema::parse_json(text)
            .map_err(|e| serde_json::Error::custom(e.to_string()))?;
        serde_json::from_value(value)
    }
}

/// The sha256 of a file, in lower-case hex.
pub fn sha256_file(path: &Path) -> io::Result<String> {
    sha256_and_size(path).map(|(h, _)| h)
}

/// The sha256 of a file, in lower-case hex, and the number of bytes hashed.
pub fn sha256_and_size(path: &Path) -> io::Result<(String, u64)> {
    let mut f = std::fs::File::open(path)?;
    let mut h = Sha256::new();
    let mut buf = vec![0u8; 1 << 16];
    let mut size = 0u64;
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
        size += n as u64;
    }
    Ok((hex(&h.finalize()), size))
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

    /// Every file under the folder, nested ones included, with its size; a top-level name in
    /// `except` alone left out, the same name deeper kept.
    #[test]
    fn a_tree_is_hashed_file_by_file() {
        let dir = std::env::temp_dir().join(format!("simpa-hash-tree-{}", std::process::id()));
        for (p, text) in [
            ("run.json", "{}"),
            ("solve/run.json", "deeper"),
            ("solve/Punctual receivers/R000/Sound level.recp", "abc"),
            ("empty.txt", ""),
        ] {
            let path = dir.join(p);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, text).unwrap();
        }
        let t = hash_tree(&dir, &["run.json"]).unwrap();
        let got: Vec<(&str, u64)> = t.iter().map(|f| (f.path.as_str(), f.size)).collect();
        assert_eq!(
            got,
            [
                ("empty.txt", 0),
                ("solve/Punctual receivers/R000/Sound level.recp", 3),
                ("solve/run.json", 6),
            ]
        );
        assert_eq!(t[1].sha256, sha256_bytes(b"abc"));
        assert_eq!(t[0].sha256, sha256_bytes(b""));
        assert_eq!(hash_tree(&dir, &[]).unwrap().len(), 4);
        std::fs::remove_dir_all(&dir).unwrap();
    }

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
