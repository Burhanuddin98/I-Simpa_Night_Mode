//! Shared helpers for the `run_*` tests, included by each with `#[path]`. Built on its own it is
//! an empty test target.
//!
//! - [`solver_exe`] finds an M1 solver build: `$SIMPA_SOLVERS_DIR`, else
//!   `<repo>/target/solvers/bin`. A missing executable panics; these tests never skip.
//! - [`stage_tutorial1`] copies upstream's tutorial-1 run folder with `__RUNDIR__` filled in.
//! - [`good_outputs`] builds, in memory, the outputs of a run that did everything its
//!   expectation asks, which each verdict test then breaks in exactly one way.
#![allow(dead_code)]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use simpa_core::config_xml;
use simpa_core::formats::cbin;
use simpa_core::formats::gabe::{Column, ColumnData, Gabe};
use simpa_core::process::{Line, Stream};
use simpa_core::run::{BandStats, Expectation, Outputs, ParticleStats};
use simpa_core::schema::SolverKind;

/// The repository root: an absolute path with no `..` in it.
pub fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crates/simpa-core sits two levels below the root")
        .to_path_buf()
}

/// A file under `tests/fixtures/`.
pub fn fixture(rel: &str) -> PathBuf {
    repo_root().join("tests/fixtures").join(rel)
}

pub fn read_text(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

/// An M1 solver build: `$SIMPA_SOLVERS_DIR/<name>`, else `<repo>/target/solvers/bin/<name>`.
pub fn solver_exe(name: &str) -> PathBuf {
    let dir = std::env::var_os("SIMPA_SOLVERS_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| repo_root().join("target/solvers/bin"));
    let p = dir.join(name);
    assert!(
        p.is_file(),
        "{} is missing: set SIMPA_SOLVERS_DIR to the folder of the M1 solver build \
         (tools/gates/m1.ps1), or build it into <repo>/target/solvers/bin",
        p.display()
    );
    p
}

pub fn exe_for(solver: SolverKind) -> PathBuf {
    solver_exe(match solver {
        SolverKind::Spps => "spps.exe",
        SolverKind::Tcr => "classicalTheory.exe",
    })
}

/// A new, empty folder under the test target's scratch folder. Never reused.
pub fn fresh_dir(label: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("run_tests")
        .join(format!("{label}-{nanos}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

pub fn kind(solver: SolverKind) -> &'static str {
    match solver {
        SolverKind::Spps => "spps",
        SolverKind::Tcr => "tcr",
    }
}

/// Upstream's tutorial-1 `config.xml` for `solver`, with `__RUNDIR__` still in it.
pub fn tutorial1_config(solver: SolverKind) -> String {
    read_text(&fixture(&format!(
        "upstream/tutorial1/{}/config.xml",
        kind(solver)
    )))
}

pub fn tutorial1_scene() -> cbin::Model {
    cbin::read_file(&fixture("upstream/tutorial1/spps/mesh.cbin")).unwrap()
}

/// A fresh run folder holding tutorial 1's inputs for `solver`, with `edit` applied to the
/// config text and `__RUNDIR__` replaced by the folder, as the gates do.
pub fn stage_tutorial1(
    label: &str,
    solver: SolverKind,
    edit: impl FnOnce(String) -> String,
) -> PathBuf {
    let dir = fresh_dir(label);
    let wd = config_xml::working_directory(&dir).unwrap();
    let text = edit(tutorial1_config(solver)).replace("__RUNDIR__", &wd);
    std::fs::write(dir.join("config.xml"), text).unwrap();
    for f in ["mesh.cbin", "tetramesh.mbin"] {
        std::fs::copy(
            fixture(&format!("upstream/tutorial1/{}/{f}", kind(solver))),
            dir.join(f),
        )
        .unwrap();
    }
    dir
}

/// Tutorial 1's expectation, without touching the disk.
pub fn tutorial1_expectation(solver: SolverKind) -> Expectation {
    Expectation::from_config(&tutorial1_config(solver), solver)
        .unwrap()
        .with_scene(&tutorial1_scene())
}

pub fn line(stream: Stream, text: &str) -> Line {
    Line {
        stream,
        t_ms: 0.0,
        text: text.to_string(),
        terminated: true,
    }
}

/// Statistics in which every requested band ran exactly its particles and lost none.
pub fn clean_stats(exp: &Expectation) -> ParticleStats {
    let per_band = exp.spps.as_ref().unwrap().nbparticules * exp.sources.len() as u32;
    ParticleStats {
        bands: exp
            .requested_bands()
            .into_iter()
            .map(|freq_hz| BandStats {
                freq_hz,
                absorbed_by_atmosphere: 0,
                absorbed_by_materials: per_band / 2,
                absorbed_by_fittings: 0,
                lost_by_infinite_loops: 0,
                lost_by_meshing_problems: 0,
                remaining: per_band - per_band / 2,
                total: per_band,
            })
            .collect(),
    }
}

/// A TCR-shaped table: a label column with every band and `Global`, then float columns whose
/// `Global` row is NaN, as TCR writes the non-energetic ones.
pub fn band_table(exp: &Expectation, columns: usize) -> Gabe {
    let mut labels: Vec<Vec<u8>> = exp
        .bands
        .iter()
        .map(|b| format!("{} Hz", b.freq_hz).into_bytes())
        .collect();
    labels.push(b"Global".to_vec());
    let n = labels.len();
    let mut cols = vec![Column {
        label: b"TC".to_vec(),
        data: ColumnData::ShortString(labels),
    }];
    for k in 0..columns {
        let mut values = vec![1.5f32 + k as f32; n];
        values[n - 1] = f32::NAN;
        cols.push(Column {
            label: format!("C{k}\rs").into_bytes(),
            data: ColumnData::Float {
                num_of_digits: 2,
                values,
            },
        });
    }
    Gabe {
        version: 2,
        read_only: true,
        columns: cols,
    }
}

/// The outputs of a run that wrote every expected file (100 bytes each), clean statistics
/// (SPPS) and finite result tables (TCR).
pub fn good_outputs(exp: &Expectation) -> Outputs {
    let files: BTreeMap<String, u64> = exp.expected_files().into_iter().map(|p| (p, 100)).collect();
    let stats = exp.spps.as_ref().map(|_| Ok(clean_stats(exp)));
    let tables = exp
        .result_tables()
        .into_iter()
        .map(|t| (t, Ok(band_table(exp, 6))))
        .collect();
    Outputs {
        files,
        stats,
        tables,
    }
}

/// The lines of a clean SPPS run.
pub fn clean_spps_lines() -> Vec<Line> {
    vec![
        line(Stream::Stdout, "SPPS version 2.2.1"),
        line(Stream::Stdout, "#50"),
        line(Stream::Stdout, "#100"),
        line(Stream::Stdout, "End of calculation."),
        line(Stream::Stdout, "Output results files."),
    ]
}

/// The lines of a clean TCR run (our writer emits `directivities_directory`).
pub fn clean_tcr_lines() -> Vec<Line> {
    [
        "XML configuration file is currently loading...",
        "config.xml",
        "XML configuration file has been loaded.",
        "Classical Theory of Reverberation version 1.0.1",
        "Step 1/3 : Main calculation.",
        "Step 2/3 : Punctual receivers calculation.",
        "Step 3/3 : Surface receivers calculation.",
        "#100",
    ]
    .into_iter()
    .map(|t| line(Stream::Stdout, t))
    .collect()
}
