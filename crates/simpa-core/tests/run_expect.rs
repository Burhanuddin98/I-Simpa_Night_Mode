//! The expectation of a run folder, from tutorial 1's configs as upstream's GUI wrote them.
//!
//! `docs/solver-contract.md` Part B states the file counts of P2's base runs of these configs:
//! 65 for SPPS and 87 for TCR. `run_solvers.rs` checks the lists against real runs, file for file.

#[path = "run_support.rs"]
mod support;

use simpa_core::formats::cbin;
use simpa_core::run::{ExpectError, Expectation};
use simpa_core::schema::SolverKind;
use support::*;

fn edited(solver: SolverKind, from: &str, to: &str) -> Expectation {
    let text = tutorial1_config(solver);
    assert!(text.contains(from), "{from}");
    Expectation::from_config(&text.replace(from, to), solver)
        .unwrap()
        .with_scene(&tutorial1_scene())
}

#[test]
fn tutorial1_spps_asks_for_65_files() {
    let dir = stage_tutorial1("expect-spps", SolverKind::Spps, |t| t);
    let e = Expectation::read(&dir, SolverKind::Spps).unwrap();
    assert_eq!(e.bands.len(), 27);
    assert!(e.bands.iter().all(|b| b.computed && b.requested));
    assert_eq!(e.requested_bands().first(), Some(&50));
    assert_eq!(e.sources, ["Source 1"]);
    assert_eq!(e.point_receivers, ["Receiver 2", "Receiver 1"]);
    assert_eq!(e.surface_receivers, [3503]);
    assert_eq!(e.first_surface_receiver_has_faces, Some(true));
    assert_eq!(e.cutting_planes, 0);
    assert!(e.output_recs_byfreq);
    let spps = e.spps.as_ref().unwrap();
    assert_eq!(
        (
            spps.nbparticules,
            spps.nbparticules_rendu,
            spps.computation_method,
            spps.random_seed
        ),
        (10_000, 0, 0, 1)
    );
    assert!(e.working_directory.ends_with('\\'));

    let files = e.expected_files();
    // 2 tables, 4 files x 2 receivers, 27 intensity files, 27 + 1 surface-receiver files.
    assert_eq!(files.len(), 65, "{files:#?}");
    for f in [
        "SPPS particle statistics.gabe",
        "Total energy.recp",
        "Punctual receivers/Receiver 1/Sound level.recp",
        "Punctual receivers/Receiver 2/Advanced sound level.gap",
        "Punctual receivers/Receiver 2/Punctual receiver intensity.gabe",
        "Punctual receivers/Receiver 1/Sound level per source.recps",
        "Intensity animation/20000 Hz/Intensity.rpi",
        "Surface receiver/50 Hz/Sound level.csbin",
        "Surface receiver/Global/Sound level.csbin",
    ] {
        assert!(files.iter().any(|p| p == f), "{f} not in {files:#?}");
    }
    assert!(e.result_tables().is_empty());
}

#[test]
fn tutorial1_tcr_asks_for_87_files() {
    let dir = stage_tutorial1("expect-tcr", SolverKind::Tcr, |t| t);
    let e = Expectation::read(&dir, SolverKind::Tcr).unwrap();
    assert!(e.spps.is_none());
    let files = e.expected_files();
    // Main results, 2 receivers, 3 prefixes x (27 bands + Global).
    assert_eq!(files.len(), 87, "{files:#?}");
    for f in [
        "Main results.gabe",
        "Punctual receivers/Receiver 2.gabe",
        "Direct field/Surface receiver/125 Hz/Sound level.csbin",
        "Total field (Sabine)/Surface receiver/Global/Sound level.csbin",
        "Total field (Eyring)/Surface receiver/20000 Hz/Sound level.csbin",
    ] {
        assert!(files.iter().any(|p| p == f), "{f} not in {files:#?}");
    }
    assert_eq!(
        e.result_tables(),
        [
            "Main results.gabe",
            "Punctual receivers/Receiver 2.gabe",
            "Punctual receivers/Receiver 1.gabe"
        ]
    );
}

#[test]
fn settings_change_the_file_list_as_the_solvers_do() {
    let spps = |from: &str, to: &str| edited(SolverKind::Spps, from, to).expected_files().len();
    let tcr = |from: &str, to: &str| edited(SolverKind::Tcr, from, to).expected_files().len();
    // P2 recs_byfreq0: only the Global surface-receiver file is left.
    assert_eq!(
        spps(r#"output_recs_byfreq="1""#, r#"output_recs_byfreq="0""#),
        65 - 27
    );
    // TCR writes every band whatever output_recs_byfreq says.
    assert_eq!(
        tcr(r#"output_recs_byfreq="1""#, r#"output_recs_byfreq="0""#),
        87
    );
    // Saved particles: a .pbin and the two collision tables per band.
    assert_eq!(
        spps(r#"nbparticules_rendu="0""#, r#"nbparticules_rendu="100""#),
        65 + 27 * 3
    );
    // One file per receiver per source.
    assert_eq!(
        spps(r#"output_recp_bysource="0""#, r#"output_recp_bysource="1""#),
        65 + 2
    );
    // A band not computed drops its files.
    assert_eq!(
        spps(r#"freq="630" docalc="1""#, r#"freq="630" docalc="0""#),
        65 - 2
    );
    assert_eq!(
        tcr(r#"freq="630" docalc="1""#, r#"freq="630" docalc="0""#),
        87 - 3
    );
}

#[test]
fn a_receiver_without_faces_writes_no_receiver_files() {
    let mut scene = tutorial1_scene();
    for f in &mut scene.faces {
        f.id_rs = -1;
    }
    for (solver, n) in [(SolverKind::Spps, 65 - 28), (SolverKind::Tcr, 87 - 84)] {
        let e = Expectation::from_config(&tutorial1_config(solver), solver)
            .unwrap()
            .with_scene(&scene);
        assert_eq!(e.first_surface_receiver_has_faces, Some(false));
        assert_eq!(e.expected_files().len(), n, "{solver:?}");
    }
}

#[test]
fn an_unreadable_scene_leaves_the_receiver_files_expected() {
    let dir = stage_tutorial1("expect-nocbin", SolverKind::Spps, |t| t);
    std::fs::write(dir.join("mesh.cbin"), b"not a cbin").unwrap();
    let e = Expectation::read(&dir, SolverKind::Spps).unwrap();
    assert_eq!(e.first_surface_receiver_has_faces, None);
    assert_eq!(e.expected_files().len(), 65);
    // And a scene read from disk agrees with the committed one.
    let e = Expectation::read(
        &stage_tutorial1("expect-cbin", SolverKind::Spps, |t| t),
        SolverKind::Spps,
    )
    .unwrap();
    let rs = cbin::read_file(&fixture("upstream/tutorial1/spps/mesh.cbin"))
        .unwrap()
        .faces
        .iter()
        .filter(|f| f.id_rs == 3503)
        .count();
    assert!(rs > 0);
    assert_eq!(e.first_surface_receiver_has_faces, Some(true));
}

#[test]
fn a_config_that_cannot_be_read_gives_no_expectation() {
    let dir = fresh_dir("expect-bad");
    assert!(matches!(
        Expectation::read(&dir, SolverKind::Spps),
        Err(ExpectError::Unreadable(_))
    ));
    std::fs::write(dir.join("config.xml"), "<configuration><simulation>").unwrap();
    assert!(matches!(
        Expectation::read(&dir, SolverKind::Spps),
        Err(ExpectError::NotXml(_))
    ));
    std::fs::write(dir.join("config.xml"), b"\xff\xfe<").unwrap();
    assert!(matches!(
        Expectation::read(&dir, SolverKind::Spps),
        Err(ExpectError::NotXml(_))
    ));
    std::fs::write(dir.join("config.xml"), "<project/>").unwrap();
    assert!(matches!(
        Expectation::read(&dir, SolverKind::Spps),
        Err(ExpectError::NotAConfiguration(_))
    ));
}
