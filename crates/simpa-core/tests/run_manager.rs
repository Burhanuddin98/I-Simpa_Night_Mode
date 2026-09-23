//! `run::manager` on real folders: the run folder and its `run.json`, every refusal before
//! launch with the exit class it gives, the pre-launch checks of `run_folder`, and the launch
//! with the M1 solvers (`$SIMPA_SOLVERS_DIR`, see `common/paths.rs`; a missing build panics).
//! The CLI's own tests (`crates/simpa/tests/`) drive the same functions through `simpa.exe`.

#[path = "run_support.rs"]
mod support;

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use simpa_core::process::CancelToken;
use simpa_core::run::manager::{STDERR_LOG, STDOUT_LOG};
use simpa_core::run::verdict::codes;
use simpa_core::run::{
    DEFAULT_LOSS_LIMIT, ExeSearch, ExitClass, MeshChoice, RunEvent, RunManifest, RunOptions,
    RunReport, Stage, Status, check_mesh_dir, create_run_folder, pre_launch, run_folder,
    run_project,
};
use simpa_core::schema::SolverKind;
use support::*;

fn options(label: &str, solver: SolverKind, exe: PathBuf) -> RunOptions {
    RunOptions {
        solver,
        solver_exe: exe,
        runs_root: fresh_dir(label),
        loss_limit: DEFAULT_LOSS_LIMIT,
        cancel_after_ms: None,
        cancel_after_progress: None,
    }
}

fn tetgen() -> MeshChoice {
    MeshChoice::Build {
        tetgen: solver_exe("tetgen.exe"),
    }
}

/// Runs a fixture folder, collecting the stages and solver lines it reports.
fn folder(case: &str, opts: &RunOptions) -> (RunReport, Vec<Stage>, usize) {
    let mut stages = Vec::new();
    let mut lines = 0;
    let r = run_folder(
        &fixture(&format!("runs/{case}")),
        opts,
        &CancelToken::new(),
        &mut |e: &RunEvent| match e {
            RunEvent::Stage(s) => stages.push(*s),
            RunEvent::SolverLine(_) => lines += 1,
            RunEvent::MeshLine(_) => {}
        },
    )
    .unwrap_or_else(|e| panic!("{case}: {e}"));
    (r, stages, lines)
}

fn project(label: &str, file: &Path, mesh: &MeshChoice, opts: &RunOptions) -> RunReport {
    run_project(
        file,
        None,
        mesh,
        opts,
        &CancelToken::new(),
        &mut |_: &RunEvent| {},
    )
    .unwrap_or_else(|e| panic!("{label}: {e}"))
}

/// `run.json` as written, which must be the report's manifest.
fn written(r: &RunReport) -> RunManifest {
    let m = RunManifest::from_json(&read_text(&r.dir.join("run.json"))).unwrap();
    assert_eq!(m, r.manifest);
    m
}

fn codes_of(r: &RunReport) -> Vec<&str> {
    r.manifest.verdict.codes()
}

/// A refusal before launch: the stage, status FAIL, the codes, the exit class, and no launch.
fn assert_refused(r: &RunReport, stage: Stage, codes: &[&str], exit: ExitClass) {
    let m = written(r);
    assert_eq!(
        (m.stage, m.verdict.status, m.exit_class),
        (stage, Status::Fail, exit),
        "{m:#?}"
    );
    for c in codes {
        assert!(m.verdict.codes().contains(c), "{c} missing: {m:#?}");
    }
    assert_eq!(m.outcome, None);
    assert!(!r.dir.join(STDOUT_LOG).exists(), "the solver was launched");
}

#[test]
fn a_fixture_runs_in_a_fresh_folder_with_its_logs_beside_solve() {
    let opts = options("mgr-ok", SolverKind::Spps, exe_for(SolverKind::Spps));
    let (r, stages, lines) = folder("spps_ok", &opts);
    let m = written(&r);
    println!(
        "spps_ok: {:?} {:?} in {:.0} ms, {} lines, {:?}",
        m.verdict.status,
        m.verdict.codes(),
        m.outcome.as_ref().unwrap().elapsed_ms,
        lines,
        m.particles
    );
    assert_eq!(m.verdict.status, Status::Ok, "{m:#?}");
    assert_eq!((m.stage, m.exit_class), (Stage::Solve, ExitClass::Ok));
    assert_eq!(stages, [Stage::PreLaunch, Stage::Solve]);
    assert!(lines > 0);
    // The folder: named for the time and solver, solve/ inside, logs and run.json beside it.
    let name = r.dir.file_name().unwrap().to_string_lossy().into_owned();
    assert!(name.ends_with("-spps") && name.len() == "yyyyMMdd-HHmmss-fff-spps".len());
    let solve = r.dir.join("solve");
    assert_eq!(m.cwd, solve.display().to_string());
    for f in ["run.json", STDOUT_LOG, STDERR_LOG] {
        assert!(r.dir.join(f).is_file(), "{f}");
        assert!(!solve.join(f).exists(), "{f} is in solve/");
    }
    assert!(!solve.join("expected.json").exists());
    // __RUNDIR__ became the absolute solve folder plus a backslash.
    let config = read_text(&solve.join("config.xml"));
    assert!(!config.contains("__RUNDIR__"));
    assert!(config.contains(&format!("workingdirectory=\"{}\\\"", solve.display())));
    assert_eq!(m.argv, ["config.xml"]);
    assert_eq!(m.exe.sha256.len(), 64);
    assert_eq!(
        m.inputs.iter().map(|f| f.path.as_str()).collect::<Vec<_>>(),
        ["config.xml", "mesh.cbin", "tetramesh.mbin"]
    );
    assert_eq!(m.files.expected, m.files.present);
    let stats = m.particles.as_ref().unwrap();
    assert!(stats.bands.iter().all(|b| b.total == 2000), "{stats:?}");
    // The log holds what the solver printed.
    assert!(read_text(&r.dir.join(STDOUT_LOG)).contains("End of calculation."));
}

#[test]
fn run_folder_refuses_before_launch_what_no_signal_after_it_catches() {
    let opts = options("mgr-pre", SolverKind::Spps, exe_for(SolverKind::Spps));
    // A short source spectrum: exit 0 and every file written if launched (decision 11).
    let (r, stages, _) = folder("spps_oneband", &opts);
    assert_refused(
        &r,
        Stage::PreLaunch,
        &["band_set_mismatch"],
        ExitClass::Solver,
    );
    assert_eq!(stages, [Stage::PreLaunch]);
    // No .mbin, and an unreadable .cbin: mesh_invalid.
    for case in ["spps_nomesh", "spps_unreadable_mesh"] {
        let (r, _, _) = folder(case, &opts);
        assert_refused(&r, Stage::PreLaunch, &["mesh_invalid"], ExitClass::Solver);
    }
    // A mesh mesh::verify refuses: mesh_invalid, then the verifier's codes.
    let (r, _, _) = folder("spps_lossy", &opts);
    assert_refused(
        &r,
        Stage::PreLaunch,
        &["mesh_invalid", "unmarked_boundary_faces"],
        ExitClass::Solver,
    );
    assert_eq!(codes_of(&r)[0], "mesh_invalid");
}

#[test]
fn the_pre_launch_checks_pass_a_good_folder_and_say_no_to_each_fault() {
    // The good folder, and the same folder with one thing broken each time.
    let stage = |label: &str, edit: &dyn Fn(&Path)| {
        let dir = fresh_dir(label);
        for f in ["config.xml", "mesh.cbin", "tetramesh.mbin"] {
            std::fs::copy(fixture(&format!("runs/spps_ok/{f}")), dir.join(f)).unwrap();
        }
        edit(&dir);
        pre_launch(&dir)
    };
    let good = stage("pre-good", &|_| {});
    assert_eq!(good.reasons, [], "{good:#?}");
    assert!(good.verify.as_ref().is_some_and(|v| v.passed()));
    assert_eq!(good.mesh.as_ref().unwrap().mbin_sha256.len(), 64);

    let codes = |p: &simpa_core::run::PreLaunch| -> Vec<String> {
        p.reasons.iter().map(|r| r.code.clone()).collect()
    };
    // The spectrum of source 1 cut to one entry.
    let short = stage("pre-short", &|d| {
        let c = d.join("config.xml");
        let t = read_text(&c).replacen(r#"<bfreq freq="500" db="90"/>"#, "", 1);
        std::fs::write(&c, t).unwrap();
    });
    assert_eq!(codes(&short), ["band_set_mismatch"], "{short:#?}");
    // A config that does not parse.
    let bad = stage("pre-xml", &|d| {
        std::fs::write(d.join("config.xml"), "<configuration>").unwrap()
    });
    assert_eq!(codes(&bad), [codes::CONFIG_ATTRIBUTE_MISSING]);
    // A tetrahedron with a repeated corner: spps_degenerate's .mbin, which the writer refuses
    // to make.
    let broken = stage("pre-degenerate", &|d| {
        let from = fixture("runs/spps_degenerate/tetramesh.mbin");
        std::fs::copy(from, d.join("tetramesh.mbin")).unwrap();
    });
    assert_eq!(
        codes(&broken),
        ["mesh_invalid", "degenerate_tets"],
        "{broken:#?}"
    );
}

#[test]
fn a_solver_that_cannot_start_is_launch_failed() {
    let dir = fresh_dir("not-an-exe");
    let fake = dir.join("spps.exe");
    std::fs::write(&fake, "this is not a program").unwrap();
    let opts = options("mgr-launch", SolverKind::Spps, fake);
    let (r, _, _) = folder("spps_ok", &opts);
    let m = written(&r);
    assert_eq!(
        (m.stage, m.verdict.status, m.exit_class),
        (Stage::Solve, Status::Fail, ExitClass::Solver)
    );
    assert_eq!(m.verdict.codes(), [codes::LAUNCH_FAILED], "{m:#?}");
    assert_eq!(m.outcome, None);
}

#[test]
fn run_project_refuses_at_each_stage_with_its_exit_class() {
    let spps = exe_for(SolverKind::Spps);
    let opts = options("mgr-refuse", SolverKind::Spps, spps);
    // Geometry: two boxes that interpenetrate.
    let r = project(
        "geometry",
        &fixture("geometry/two_boxes_interpenetrating.simpa"),
        &tetgen(),
        &opts,
    );
    assert_refused(
        &r,
        Stage::Geometry,
        &[codes::GEOMETRY_REFUSED],
        ExitClass::Geometry,
    );
    assert!(
        r.manifest.verdict.reasons[0]
            .detail
            .contains("self_intersections"),
        "{:#?}",
        r.manifest.verdict
    );
    // Validation: a project with no source.
    let r = project(
        "validate",
        &fixture("negative/schema/source_none.simpa"),
        &tetgen(),
        &opts,
    );
    assert_refused(&r, Stage::Validate, &["source_none"], ExitClass::Usage);
    assert!(!r.dir.join("mesh").exists());
    // The mesh: a folder with nothing in it.
    let empty = fresh_dir("empty-mesh");
    let box_ = fixture("rooms/tutorial1_box_seeded.simpa");
    let r = project("mesh", &box_, &MeshChoice::Reuse(empty), &opts);
    assert_refused(&r, Stage::Mesh, &[codes::MESH_MISSING], ExitClass::Mesh);
    // The export: a variant the project does not have.
    let r = run_project(
        &box_,
        Some("no such variant"),
        &tetgen(),
        &opts,
        &CancelToken::new(),
        &mut |_: &RunEvent| {},
    )
    .unwrap();
    assert_refused(&r, Stage::Export, &[codes::EXPORT_FAILED], ExitClass::Usage);
    assert!(
        r.manifest.verdict.reasons[0]
            .detail
            .starts_with("variant_not_found"),
        "{:#?}",
        r.manifest.verdict
    );
    // A cancel before anything runs: CANCELLED, exit class 130, nothing launched.
    let cancel = CancelToken::new();
    cancel.cancel();
    let r = run_project(
        &box_,
        None,
        &tetgen(),
        &opts,
        &cancel,
        &mut |_: &RunEvent| {},
    )
    .unwrap();
    let m = written(&r);
    assert_eq!(
        (m.verdict.status, m.exit_class, m.outcome),
        (Status::Cancelled, ExitClass::Cancelled, None)
    );
    // A project that is not a project: no run folder at all.
    let e = run_project(
        &fixture("geometry/box.ply"),
        None,
        &tetgen(),
        &opts,
        &CancelToken::new(),
        &mut |_: &RunEvent| {},
    )
    .unwrap_err();
    assert_eq!(e.exit_class(), ExitClass::Usage);
}

#[test]
fn a_mesh_folder_is_checked_against_the_project_and_its_mbin() {
    let box_ = fixture("rooms/tutorial1_box_seeded.simpa");
    let project = simpa_core::schema::load(&box_).unwrap();
    let hash = simpa_core::validate::mesh_input_hash(&project);
    let dir = fresh_dir("mesh-check");
    let m = simpa_core::mesh::mesh_project(
        &project,
        &dir,
        &simpa_core::mesh::TetgenMesher::new(solver_exe("tetgen.exe")),
        &CancelToken::new(),
        &mut |_| {},
    )
    .unwrap();
    assert!(m.is_ok(), "{m:#?}");
    let ok = check_mesh_dir(&dir, &hash).unwrap();
    assert_eq!(Some(ok.mbin_sha256.clone()), m.files.mbin);
    // Another project stamp: out of date.
    let other = "0".repeat(32);
    let r = check_mesh_dir(&dir, &other).unwrap_err();
    assert_eq!(
        r.iter().map(|r| r.code.as_str()).collect::<Vec<_>>(),
        ["mesh_out_of_date"]
    );
    // A .mbin that is not the one the manifest records: manifest_mismatch.
    let mbin = dir.join("tetramesh.mbin");
    let mut bytes = std::fs::read(&mbin).unwrap();
    let last = bytes.len() - 1;
    bytes[last] ^= 1;
    std::fs::write(&mbin, &bytes).unwrap();
    let r = check_mesh_dir(&dir, &hash).unwrap_err();
    assert_eq!(
        r.iter().map(|r| r.code.as_str()).collect::<Vec<_>>(),
        ["manifest_mismatch"]
    );
    // No .mbin, and no manifest: mesh_missing.
    std::fs::remove_file(&mbin).unwrap();
    let r = check_mesh_dir(&dir, &hash).unwrap_err();
    assert_eq!(r[0].code, codes::MESH_MISSING);
    std::fs::remove_file(dir.join("mesh.json")).unwrap();
    let r = check_mesh_dir(&dir, &hash).unwrap_err();
    assert_eq!(r[0].code, codes::MESH_MISSING);
}

#[test]
fn run_folders_are_never_reused() {
    let root = fresh_dir("collide");
    let at = SystemTime::UNIX_EPOCH + Duration::from_millis(1_790_189_551_123);
    let a = create_run_folder(&root, SolverKind::Tcr, at).unwrap();
    let b = create_run_folder(&root, SolverKind::Tcr, at).unwrap();
    let c = create_run_folder(&root, SolverKind::Tcr, at).unwrap();
    let names: Vec<String> = [&a, &b, &c]
        .iter()
        .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
        .collect();
    // The stamp is local time (`run::clock`, tested against .NET there).
    let stamp = simpa_core::run::manager::folder_stamp(at);
    assert!(stamp.ends_with("-123"), "{stamp}");
    assert_eq!(
        names,
        [
            format!("{stamp}-tcr"),
            format!("{stamp}-tcr-2"),
            format!("{stamp}-tcr-3")
        ]
    );
    assert!(a.is_absolute());
}

#[test]
fn executables_are_found_in_the_designs_order() {
    let root = fresh_dir("exe-search");
    let exe_dir = root.join("repo/target/debug");
    let env_dir = root.join("env");
    for d in [&exe_dir, &env_dir, &exe_dir.join("solvers")] {
        std::fs::create_dir_all(d).unwrap();
    }
    let dev = root.join("repo/target/solvers/bin");
    std::fs::create_dir_all(&dev).unwrap();
    let touch = |p: PathBuf| {
        std::fs::write(&p, b"x").unwrap();
        p
    };
    // A name the real dev tree above this folder does not have: its target/solvers/bin would
    // otherwise be found through the ancestors.
    let search = ExeSearch {
        explicit: None,
        solvers_dir: Some(env_dir.clone()),
        exe_dir: Some(exe_dir.clone()),
    };
    // Nothing anywhere: the error lists every candidate, in order.
    let e = search.find("probe.exe").unwrap_err();
    assert_eq!(
        e.tried,
        [
            env_dir.join("probe.exe"),
            exe_dir.join("solvers/probe.exe"),
            exe_dir.join("probe.exe")
        ]
    );
    // Each place wins over the ones after it.
    let in_dev = touch(dev.join("probe.exe"));
    assert_eq!(search.find("probe.exe").unwrap(), in_dev);
    let beside = touch(exe_dir.join("probe.exe"));
    assert_eq!(search.find("probe.exe").unwrap(), beside);
    let in_solvers = touch(exe_dir.join("solvers/probe.exe"));
    assert_eq!(search.find("probe.exe").unwrap(), in_solvers);
    let in_env = touch(env_dir.join("probe.exe"));
    assert_eq!(search.find("probe.exe").unwrap(), in_env);
    // An explicit path is the only candidate, even when it is missing.
    let explicit = ExeSearch {
        explicit: Some(root.join("nope.exe")),
        ..search.clone()
    };
    assert_eq!(
        explicit.find("probe.exe").unwrap_err().tried,
        [root.join("nope.exe")]
    );
}
