//! The classifier, the expectation and the verdict on real runs of our M1 solver builds, on
//! upstream's tutorial-1 run folders (SPPS seeded at 10,000 particles). These need the solver
//! executables (`$SIMPA_SOLVERS_DIR`, see `run_support.rs`) and panic without them.
//!
//! What they prove beyond the unit tests: every line the solvers print on these runs is
//! classified by a documented row (no `unclassified_line`); the expected-file lists are exactly
//! the files the solvers write, no more and no fewer; and the verdict says OK on a clean run and
//! names the cause on a broken one.

#[path = "run_support.rs"]
mod support;

use std::collections::BTreeSet;
use std::path::Path;

use simpa_core::process::{self, CancelToken, Spec};
use simpa_core::run::verdict::codes::*;
use simpa_core::run::{
    ClassCounts, Classified, Classifier, DEFAULT_LOSS_LIMIT, Evidence, Expectation, Outputs,
    Status, Verdict, judge,
};
use simpa_core::schema::SolverKind;
use support::*;

struct Run {
    lines: Vec<Classified>,
    outcome: process::Outcome,
    verdict: Verdict,
    expected: Vec<String>,
    outputs: Outputs,
}

/// Runs `solver` in `dir` the contract's way (cwd = the folder, one argument `config.xml`),
/// classifying lines as they arrive, then judges it.
fn run(dir: &Path, solver: SolverKind) -> Run {
    let spec = Spec {
        program: exe_for(solver),
        args: vec!["config.xml".into()],
        cwd: dir.to_path_buf(),
    };
    let mut classifier = Classifier::new();
    let mut lines = Vec::new();
    let outcome = process::run(&spec, &CancelToken::new(), &mut |l: &process::Line| {
        lines.extend(classifier.push(l))
    })
    .unwrap();
    lines.extend(classifier.finish());
    let exp = Expectation::read(dir, solver);
    let outputs = match &exp {
        Ok(e) => Outputs::read(dir, e),
        Err(_) => Outputs::default(),
    };
    let verdict = judge(&Evidence {
        solver,
        outcome: &outcome,
        lines: &lines,
        expectation: exp.as_ref(),
        outputs: &outputs,
        loss_limit: DEFAULT_LOSS_LIMIT,
    });
    let expected = exp.map(|e| e.expected_files()).unwrap_or_default();
    println!(
        "{solver:?} in {}: exit {:?} after {:.0} ms, lines {:?}, {} files, verdict {:?} {:?}",
        dir.display(),
        outcome.exit_code,
        outcome.elapsed_ms,
        ClassCounts::of(&lines),
        outputs.files.len(),
        verdict.status,
        verdict.codes()
    );
    Run {
        lines,
        outcome,
        verdict,
        expected,
        outputs,
    }
}

const INPUTS: [&str; 3] = ["config.xml", "mesh.cbin", "tetramesh.mbin"];

/// The files the solver wrote: everything in the folder but its inputs.
fn written(r: &Run) -> BTreeSet<String> {
    r.outputs
        .files
        .keys()
        .filter(|p| !INPUTS.contains(&p.as_str()))
        .cloned()
        .collect()
}

fn ids<'a>(r: &'a Run, id: &str) -> Vec<&'a Classified> {
    r.lines.iter().filter(|l| l.id == id).collect()
}

#[test]
fn spps_tutorial1_is_ok_and_writes_exactly_the_expected_files() {
    let dir = stage_tutorial1("spps-t1", SolverKind::Spps, |t| t);
    let r = run(&dir, SolverKind::Spps);
    assert_eq!(r.outcome.exit_code, Some(0));
    let n = ClassCounts::of(&r.lines);
    assert_eq!(
        (n.unclassified, n.warn, n.fail, n.ok, n.info),
        (0, 0, 0, 1, 2),
        "{:#?}",
        r.lines
            .iter()
            .filter(|l| l.id != "progress")
            .collect::<Vec<_>>()
    );
    assert_eq!(ids(&r, "spps_banner").len(), 1);
    assert_eq!(ids(&r, "spps_output_start").len(), 1);
    // Progress climbs in steps of 0.01 %; the last line printed is #99.99, never #100.
    let progress: Vec<f64> = r.lines.iter().filter_map(|l| l.progress).collect();
    assert_eq!(progress.len(), 9_999);
    assert!(progress.windows(2).all(|w| w[0] < w[1]));
    assert_eq!(progress.last(), Some(&99.99));

    let expected: BTreeSet<String> = r.expected.iter().cloned().collect();
    assert_eq!(expected.len(), 65);
    assert_eq!(written(&r), expected, "files written vs files expected");

    assert_eq!(r.verdict.status, Status::Ok, "{:#?}", r.verdict);
    assert!(r.verdict.reasons.is_empty() && r.verdict.warnings.is_empty());
    let st = r.outputs.stats.as_ref().unwrap().as_ref().unwrap();
    assert_eq!(st.bands.len(), 27);
    assert!(st.bands.iter().all(|b| b.total == 10_000));
    // Seeded, so the table is the committed one from M1's run of this config, to the particle:
    // upstream's own box mesh loses 1 particle in 10,000 to meshing problems, at 2000 Hz.
    let committed =
        simpa_core::run::stats::read_file(&fixture("solver-outputs/tutorial1/spps_stats.gabe"))
            .unwrap();
    assert_eq!(st, &committed);
    let lost: Vec<(i32, u64)> = st
        .bands
        .iter()
        .filter(|b| b.lost() > 0)
        .map(|b| (b.freq_hz, b.lost()))
        .collect();
    assert_eq!(lost, [(2000, 1)]);
}

#[test]
fn tcr_tutorial1_as_upstreams_gui_wrote_it_fails_on_its_xml_property_line() {
    // Upstream's GUI writes no directivities_directory for TCR (contract Part B, corrections),
    // and the solver says so: a FAIL line although it exits 0 with every file written.
    let dir = stage_tutorial1("tcr-t1-upstream", SolverKind::Tcr, |t| t);
    let r = run(&dir, SolverKind::Tcr);
    assert_eq!(r.outcome.exit_code, Some(0));
    assert_eq!(r.verdict.status, Status::Fail);
    assert_eq!(r.verdict.codes(), ["xml_property_missing"]);
    assert_eq!(
        r.verdict.reasons[0].detail,
        "Xml Property directivities_directory doesn't exist !"
    );
    let expected: BTreeSet<String> = r.expected.iter().cloned().collect();
    assert_eq!(written(&r), expected);
}

#[test]
fn tcr_tutorial1_with_every_attribute_is_ok_and_writes_exactly_the_expected_files() {
    let dir = stage_tutorial1("tcr-t1", SolverKind::Tcr, |t| {
        t.replacen(
            r#"modelName="mesh.cbin""#,
            r#"directivities_directory="loudspeakers\" modelName="mesh.cbin""#,
            1,
        )
    });
    let r = run(&dir, SolverKind::Tcr);
    let n = ClassCounts::of(&r.lines);
    assert_eq!((n.unclassified, n.warn, n.fail), (0, 0, 0));
    for (id, count) in [
        ("tcr_loading", 2),
        ("tcr_config_echo", 1),
        ("tcr_banner", 1),
        ("tcr_step", 3),
    ] {
        assert_eq!(ids(&r, id).len(), count, "{id}");
    }
    let expected: BTreeSet<String> = r.expected.iter().cloned().collect();
    assert_eq!(expected.len(), 87);
    assert_eq!(written(&r), expected, "files written vs files expected");
    assert_eq!(r.verdict.status, Status::Ok, "{:#?}", r.verdict);
    assert!(r.verdict.reasons.is_empty() && r.verdict.warnings.is_empty());
    assert_eq!(r.outputs.tables.len(), 3);
}

#[test]
fn spps_without_its_tetrahedral_mesh_exits_0_and_is_refused() {
    // Contract Part B: SPPS returns 0 whatever happened. Here it cannot read the .mbin.
    let dir = stage_tutorial1("spps-t1-nombin", SolverKind::Spps, |t| t);
    std::fs::remove_file(dir.join("tetramesh.mbin")).unwrap();
    let r = run(&dir, SolverKind::Spps);
    assert_eq!(r.outcome.exit_code, Some(0));
    assert_eq!(r.verdict.status, Status::Fail);
    let codes = r.verdict.codes();
    assert_eq!(
        codes[..2],
        [END_OF_CALCULATION_MISSING, "tetra_mesh_unreadable"],
        "{:#?}",
        r.verdict
    );
    assert!(codes.contains(&STATS_UNREADABLE));
    assert!(codes.contains(&EXPECTED_FILE_MISSING));
    assert_eq!(ClassCounts::of(&r.lines).unclassified, 0);
}

#[test]
fn tcr_without_its_tetrahedral_mesh_exits_1() {
    // S tcr_nomesh.
    let dir = stage_tutorial1("tcr-t1-nombin", SolverKind::Tcr, |t| {
        t.replacen(
            r#"modelName="mesh.cbin""#,
            r#"directivities_directory="loudspeakers\" modelName="mesh.cbin""#,
            1,
        )
    });
    std::fs::remove_file(dir.join("tetramesh.mbin")).unwrap();
    let r = run(&dir, SolverKind::Tcr);
    assert_eq!(r.outcome.exit_code, Some(1));
    assert_eq!(r.verdict.status, Status::Fail);
    assert_eq!(
        r.verdict.codes(),
        [EXIT_NONZERO, "tetra_mesh_unreadable"],
        "{:#?}",
        r.verdict
    );
}
