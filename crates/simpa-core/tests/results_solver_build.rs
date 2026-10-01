//! Backlog 38: "Results verified" only when the run's solver build was verified
//! (`docs/investigations/2026-09-30-b38-39/PLAN.md`, tests T38-1 to T38-4).
//!
//! `results::solver_build` decides it from the manifest alone, beside `results::load`, which keeps
//! its contract (C1): the committed run folders under `tests/fixtures/results/` were written by
//! the CLI before M11, so their `run.json` has no `solvers` key, and they still load. The other
//! cases edit a parsed copy of one of those manifests in memory; no file is written and no solver
//! runs.

mod common;

use simpa_core::bed::pe::{ManifestSource, SolverCheck};
use simpa_core::results::{self, SolverBuild, build_codes};
use simpa_core::run::manager::{TETGEN_EXE_NAME, solver_exe_name};
use simpa_core::run::{RunManifest, SolverManifestRecord};

const SPPS: &str = "results/seats_spps";
const TCR: &str = "results/seats_tcr";

/// The committed TCR run's manifest, parsed.
fn tcr_manifest() -> RunManifest {
    let path = common::fixture(TCR).join("run.json");
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    RunManifest::from_json(&text).unwrap()
}

/// One check of `name`, matching the verified build or not.
fn check(name: &str, matches: bool) -> SolverCheck {
    SolverCheck {
        name: name.to_string(),
        path: format!(r"C:\solvers\{name}"),
        code_sha256: Some("a".repeat(64)),
        raw_sha256: Some("b".repeat(64)),
        manifest_code_sha256: Some(if matches { "a" } else { "c" }.repeat(64)),
        matches,
        detail: (!matches).then(|| "not the verified build".to_string()),
    }
}

/// The code of an unverified build; panics, naming the case, on a verified one.
fn unverified_code(case: &str, b: &SolverBuild) -> String {
    match b {
        SolverBuild::Unverified { reason } => reason.code.clone(),
        SolverBuild::Verified => panic!("{case}: the solver build is verified, want unverified"),
    }
}

/// T38-1: a manifest with no `solvers` key still loads (C1 pinned), and its build is unverified
/// with the reason code for no record.
#[test]
fn t38_1_a_run_without_a_solver_record_loads_and_its_build_is_unverified() {
    for name in [SPPS, TCR] {
        let text = std::fs::read_to_string(common::fixture(name).join("run.json")).unwrap();
        assert!(
            !text.contains("\"solvers\""),
            "{name}: the fixture must have no solvers key"
        );
        let r = results::load(&common::fixture(name))
            .unwrap_or_else(|e| panic!("{name}: load must stay Ok (C1): {e}"));
        assert_eq!(r.manifest.solvers, None, "{name}");
        let b = results::solver_build(&r.manifest);
        assert!(!b.is_verified(), "{name}: {b:?}");
        assert_eq!(
            unverified_code(name, &b),
            build_codes::UNRECORDED,
            "{name}: {b:?}"
        );
    }
}

/// T38-2: `solvers` recorded with one check that does not match: unverified, with the mismatch
/// code, whether the mismatch is the solver's own check or the mesher's.
#[test]
fn t38_2_a_recorded_check_that_does_not_match_leaves_the_build_unverified() {
    let base = tcr_manifest();
    let solver = solver_exe_name(base.solver);
    for (case, checks) in [
        ("the solver's own check", vec![check(solver, false)]),
        (
            "the mesher's check",
            vec![check(solver, true), check(TETGEN_EXE_NAME, false)],
        ),
    ] {
        let m = RunManifest {
            solvers: Some(checks),
            ..base.clone()
        };
        let b = results::solver_build(&m);
        assert!(!b.is_verified(), "{case}: {b:?}");
        assert_eq!(unverified_code(case, &b), build_codes::MISMATCH, "{case}");
    }
}

/// T38-3: `solvers` recorded as an empty list: unverified, as a run with no record is. And (C5)
/// checks that cover other executables but not the solver the run executed leave it unverified
/// too.
#[test]
fn t38_3_an_empty_solver_record_leaves_the_build_unverified() {
    let base = tcr_manifest();
    let empty = RunManifest {
        solvers: Some(Vec::new()),
        ..base.clone()
    };
    let b = results::solver_build(&empty);
    assert!(!b.is_verified(), "an empty list: {b:?}");
    assert_eq!(
        unverified_code("an empty list", &b),
        build_codes::UNRECORDED
    );

    let other = RunManifest {
        solvers: Some(vec![check(TETGEN_EXE_NAME, true)]),
        ..base
    };
    let b = results::solver_build(&other);
    assert!(!b.is_verified(), "no check covers the solver: {b:?}");
    assert_eq!(
        unverified_code("no check covers the solver", &b),
        build_codes::UNCHECKED
    );
}

/// T38-4, control: `solvers` recorded and every check matching, the solver's among them:
/// verified, with no reason.
#[test]
fn t38_4_every_recorded_check_matching_verifies_the_build() {
    let base = tcr_manifest();
    let solver = solver_exe_name(base.solver);
    for checks in [
        vec![check(solver, true)],
        vec![check(solver, true), check(TETGEN_EXE_NAME, true)],
    ] {
        let m = RunManifest {
            solvers: Some(checks.clone()),
            ..base.clone()
        };
        let b = results::solver_build(&m);
        assert_eq!(b, SolverBuild::Verified, "{checks:?}");
        assert!(b.is_verified());
        assert!(b.reason().is_none());
    }
}

/// T54-override (M8b): `$SIMPA_SOLVER_MANIFEST` lets a run proceed against a stand-in whose own
/// hash the override manifest holds, so its checks all match trivially. That must never read as
/// verified: `solver_manifest.source == override` forces `solver_manifest_override`, whatever the
/// checks say, and the embedded source (or no record at all) is unaffected.
#[test]
fn t54_override_a_run_checked_against_the_override_manifest_never_verifies() {
    let base = tcr_manifest();
    let solver = solver_exe_name(base.solver);
    // Every check matches, the solver's among them (T38-4's exact verified case) -- but the
    // manifest they were checked against was the override, so this must still be unverified.
    let m = RunManifest {
        solvers: Some(vec![check(solver, true), check(TETGEN_EXE_NAME, true)]),
        solver_manifest: Some(SolverManifestRecord {
            source: ManifestSource::Override,
            sha256: "d".repeat(64),
        }),
        ..base.clone()
    };
    let b = results::solver_build(&m);
    assert!(!b.is_verified(), "an override run must never verify: {b:?}");
    assert_eq!(
        unverified_code("override, all matching", &b),
        build_codes::OVERRIDE
    );

    // The same checks, recorded against the embedded manifest (or no `solver_manifest` at all,
    // the M11-era shape): still verified. The override marker, not the checks, is what changed.
    let embedded = RunManifest {
        solvers: Some(vec![check(solver, true), check(TETGEN_EXE_NAME, true)]),
        solver_manifest: Some(SolverManifestRecord {
            source: ManifestSource::Embedded,
            sha256: "d".repeat(64),
        }),
        ..base.clone()
    };
    assert_eq!(results::solver_build(&embedded), SolverBuild::Verified);
    let no_record = RunManifest {
        solvers: Some(vec![check(solver, true), check(TETGEN_EXE_NAME, true)]),
        solver_manifest: None,
        ..base
    };
    assert_eq!(results::solver_build(&no_record), SolverBuild::Verified);
}
