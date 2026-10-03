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
        verify: None,
    }
}

fn tetgen() -> MeshChoice {
    MeshChoice::Build {
        tetgen: solver_exe("tetgen.exe"),
        preprocess: Some(solver_exe("preprocess.exe")),
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
            RunEvent::MeshLine(_) | RunEvent::Started(_) => {}
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
    // Its region, the cube's, then lacks the collapsed tetrahedron's volume too.
    assert_eq!(
        codes(&broken),
        ["mesh_invalid", "degenerate_tets", "region_volume_mismatch"],
        "{broken:#?}"
    );

    // The regions are held to the cells of the folder's own geometry (decision 15): the good
    // folder's, from its mesh.cbin.
    let v = good.verify.as_ref().unwrap();
    assert!(v.regions_checked, "{v:#?}");
    assert_eq!(v.regions.len(), 1, "{:#?}", v.regions);
    // Says no: the room's tetrahedra split between two ids without a gap, which TetGen's numbering
    // alone accepted: two regions in one cell, neither its volume.
    let split = stage("pre-split-room", &|d| {
        let path = d.join("tetramesh.mbin");
        let mut m = simpa_core::formats::mbin::read_file(&path).unwrap();
        let first = m.tetrahedra.iter().map(|t| t.id_volume).min().unwrap();
        let half = m.tetrahedra.len() / 2;
        for t in &mut m.tetrahedra[..half] {
            t.id_volume = first + 1;
        }
        simpa_core::formats::mbin::write_file(&m, &path).unwrap();
    });
    assert_eq!(
        codes(&split),
        ["mesh_invalid", "region_volume_mismatch"],
        "{split:#?}"
    );
    assert!(
        !split
            .verify
            .as_ref()
            .unwrap()
            .codes
            .contains(&"unknown_volume_ids".to_string()),
        "the numbering alone accepts it"
    );
    // Says no: a folder whose geometry the check refuses (its .poly, which takes precedence over
    // the .cbin, with a facet gone, so open) holds its regions to no cells: refused...
    let open_poly = |d: &Path| {
        let model = simpa_core::formats::poly::Model {
            save_face_index: true,
            user_defined_faces: Vec::new(),
            model_faces: vec![
                simpa_core::formats::poly::Face {
                    vertices: [0, 2, 1],
                    face_index: 0,
                },
                simpa_core::formats::poly::Face {
                    vertices: [0, 1, 3],
                    face_index: 1,
                },
            ],
            model_vertices: vec![
                [0.0, 0.0, 0.0],
                [1.0, 0.0, 0.0],
                [0.0, 1.0, 0.0],
                [0.0, 0.0, 1.0],
            ],
            model_regions: Vec::new(),
        };
        simpa_core::formats::poly::write_file(&model, &d.join("scene_mesh.poly")).unwrap();
    };
    let unchecked = stage("pre-unchecked", &|d| open_poly(d));
    assert_eq!(
        codes(&unchecked),
        ["mesh_invalid", "regions_unchecked"],
        "{unchecked:#?}"
    );
    assert!(!unchecked.verify.as_ref().unwrap().regions_checked);
    // ... unless the folder's mesh.json is the mesher's record of this .mbin, regions checked.
    let proven = |sha_ok: bool, checked: bool| {
        move |d: &Path| {
            open_poly(d);
            let sha = simpa_core::mesh::sha256_file(&d.join("tetramesh.mbin")).unwrap();
            let manifest = serde_json::json!({
                "status": "OK",
                "files": { "mbin": if sha_ok { sha } else { "0".repeat(64) } },
                "verify": { "regions_checked": checked },
            });
            std::fs::write(d.join("mesh.json"), manifest.to_string()).unwrap();
        }
    };
    let accepted = stage("pre-proven", &proven(true, true));
    assert_eq!(accepted.reasons, [], "{accepted:#?}");
    // Says no: a manifest of another .mbin, or one whose regions were not checked.
    for (label, sha_ok, checked) in [
        ("pre-other-mbin", false, true),
        ("pre-not-checked", true, false),
    ] {
        let refused = stage(label, &proven(sha_ok, checked));
        assert_eq!(
            codes(&refused),
            ["mesh_invalid", "regions_unchecked"],
            "{label}: {refused:#?}"
        );
    }
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
    // A manifest that records parity mode is refused whatever its status (Burhan's decision 2 of
    // 2026-09-24), after mesh_missing when it is not OK either. The manifest is edited by hand,
    // as nothing stops a user from doing.
    let json_path = dir.join("mesh.json");
    let original = std::fs::read_to_string(&json_path).unwrap();
    let edited = |parity: bool, status: &str| {
        let mut v: serde_json::Value = serde_json::from_str(&original).unwrap();
        v["parity"] = parity.into();
        v["status"] = status.into();
        std::fs::write(&json_path, serde_json::to_string_pretty(&v).unwrap()).unwrap();
        let got = check_mesh_dir(&dir, &hash)
            .map(|_| Vec::new())
            .unwrap_or_else(|r| r.into_iter().map(|r| r.code).collect::<Vec<_>>());
        std::fs::write(&json_path, &original).unwrap();
        got
    };
    assert_eq!(edited(true, "OK"), [codes::MESH_PARITY]);
    assert_eq!(
        edited(true, "FAIL"),
        [codes::MESH_MISSING, codes::MESH_PARITY]
    );
    assert_eq!(edited(false, "FAIL"), [codes::MESH_MISSING]);
    assert_eq!(edited(false, "OK"), Vec::<String>::new());
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

/// A project whose mesh settings ask for upstream's scene correction is checked on what
/// `preprocess.exe` saves, by the mesher (`docs/m5-m6-design.md`, decision 12), not on its scene
/// before meshing: the two interpenetrating boxes the geometry stage refuses (above) go on to the
/// mesh stage then. Without a `preprocess.exe` the mesh stage refuses such a project by name.
#[test]
fn a_project_meshed_through_preprocess_is_checked_by_the_mesher() {
    let spps = exe_for(SolverKind::Spps);
    let opts = options("mgr-preprocess", SolverKind::Spps, spps);
    let dir = fresh_dir("preprocess-projects");
    let switched = |rel: &str| {
        let text = std::fs::read_to_string(fixture(rel)).unwrap();
        assert_eq!(text.matches("\"preprocess\": false").count(), 1, "{rel}");
        let path = dir.join(Path::new(rel).file_name().unwrap());
        std::fs::write(
            &path,
            text.replace("\"preprocess\": false", "\"preprocess\": true"),
        )
        .unwrap();
        path
    };
    let no_preprocess = MeshChoice::Build {
        tetgen: solver_exe("tetgen.exe"),
        preprocess: None,
    };
    let box_ = switched("rooms/tutorial1_box_seeded.simpa");
    let r = project("no-preprocess", &box_, &no_preprocess, &opts);
    assert_refused(
        &r,
        Stage::Mesh,
        &["preprocess_launch_failed"],
        ExitClass::Mesh,
    );
    // The scene check is the mesher's then: the geometry stage lets the boxes through.
    let boxes = switched("geometry/two_boxes_interpenetrating.simpa");
    let r = project("boxes-no-preprocess", &boxes, &no_preprocess, &opts);
    let m = written(&r);
    assert_ne!(m.stage, Stage::Geometry, "{m:#?}");
    assert!(
        !m.verdict.codes().contains(&codes::GEOMETRY_REFUSED),
        "{m:#?}"
    );
}

// ---- M11 (docs/investigations/2026-09-29-m11/PLAN.md 2.9: C5, C6, C7) -------------------------

/// What a run reported, owned: `Started`'s folder, stage names, and counts of the line events.
#[derive(Debug, Default)]
struct Events {
    order: Vec<String>,
    started: Vec<PathBuf>,
}

impl Events {
    fn sink(&mut self) -> impl FnMut(&RunEvent) + '_ {
        |e: &RunEvent| match e {
            RunEvent::Started(dir) => {
                self.order.push("started".into());
                self.started.push(dir.to_path_buf());
            }
            RunEvent::Stage(s) => self.order.push(format!("stage {s:?}")),
            RunEvent::MeshLine(_) => self.order.push("mesh line".into()),
            RunEvent::SolverLine(_) => self.order.push("solver line".into()),
        }
    }

    /// `Started` came first, once, and named the folder that holds `run.json`.
    fn assert_started_first(&self, r: &RunReport, label: &str) {
        assert_eq!(
            self.order.first().map(String::as_str),
            Some("started"),
            "{label}: {:?}",
            self.order
        );
        assert_eq!(self.started, std::slice::from_ref(&r.dir), "{label}");
        assert!(self.started[0].join("run.json").is_file(), "{label}");
        assert!(self.started[0].is_absolute(), "{label}");
    }
}

/// A manifest of the verified build made from the files themselves: whatever build the tests
/// run with, its own code sha256s are the "verified" ones.
fn manifest_of(names: &[&str]) -> simpa_core::bed::pe::SolverManifest {
    let mut code = std::collections::BTreeMap::new();
    let mut raw = std::collections::BTreeMap::new();
    for name in names {
        let (c, r) = simpa_core::bed::pe::file_hashes(&solver_exe(name)).unwrap();
        code.insert(name.to_string(), c);
        raw.insert(name.to_string(), r);
    }
    simpa_core::bed::pe::SolverManifest {
        code_sha256: code,
        sha256: raw,
        source: simpa_core::bed::pe::ManifestSource::Embedded,
        file_sha256: String::new(),
    }
}

fn run_with_events(
    file: &Path,
    variant: Option<&str>,
    mesh: &MeshChoice,
    opts: &RunOptions,
) -> (RunReport, Events) {
    let mut events = Events::default();
    let r = run_project(
        file,
        variant,
        mesh,
        opts,
        &CancelToken::new(),
        &mut events.sink(),
    )
    .unwrap_or_else(|e| panic!("{}: {e}", file.display()));
    (r, events)
}

/// C6: `RunEvent::Started` is the first event of every run that gets a folder, refused at any
/// stage, launched, or a fixture folder, and it names the folder `run.json` is written into.
#[test]
fn the_first_event_names_the_run_folder() {
    let spps = options("m11-started", SolverKind::Spps, exe_for(SolverKind::Spps));
    let box_ = fixture("rooms/tutorial1_box_seeded.simpa");
    let empty = fresh_dir("m11-started-empty-mesh");
    for (label, file, variant, mesh, stage) in [
        (
            "geometry",
            fixture("geometry/two_boxes_interpenetrating.simpa"),
            None,
            tetgen(),
            Stage::Geometry,
        ),
        (
            "validate",
            fixture("negative/schema/source_none.simpa"),
            None,
            tetgen(),
            Stage::Validate,
        ),
        (
            "mesh",
            box_.clone(),
            None,
            MeshChoice::Reuse(empty.clone()),
            Stage::Mesh,
        ),
        (
            "export",
            box_.clone(),
            Some("no such variant"),
            tetgen(),
            Stage::Export,
        ),
    ] {
        let (r, events) = run_with_events(&file, variant, &mesh, &spps);
        assert_eq!(r.manifest.stage, stage, "{label}");
        events.assert_started_first(&r, label);
    }
    // A launched run, OK.
    let tcr = options("m11-started-tcr", SolverKind::Tcr, exe_for(SolverKind::Tcr));
    let (r, events) = run_with_events(&box_, None, &tetgen(), &tcr);
    assert_eq!(r.manifest.verdict.status, Status::Ok, "{:#?}", r.manifest);
    events.assert_started_first(&r, "tcr ok");
    assert!(events.order.iter().any(|e| e == "solver line"));
    // A fixture folder.
    let mut events = Events::default();
    let r = run_folder(
        &fixture("runs/spps_ok"),
        &spps,
        &CancelToken::new(),
        &mut events.sink(),
    )
    .unwrap();
    events.assert_started_first(&r, "run_folder");
    // No folder, no event: a file that is not a project.
    let mut events = Events::default();
    let e = run_project(
        &fixture("geometry/box.ply"),
        None,
        &tetgen(),
        &spps,
        &CancelToken::new(),
        &mut events.sink(),
    );
    assert!(e.is_err());
    assert!(events.order.is_empty(), "{:?}", events.order);
}

/// C7: with `verify`, the executables are checked first, stage `solvers`. The build's own hashes
/// pass and are recorded; a wrong one refuses the run with `solver_unverified`, exit class 2,
/// with nothing meshed or launched; without `verify` no `solvers` key is written.
#[test]
fn a_run_asked_to_verify_its_solvers_checks_them_first() {
    let box_ = fixture("rooms/tutorial1_box_seeded.simpa");
    let names = ["classicalTheory.exe", "tetgen.exe", "preprocess.exe"];

    // (1) The build's own hashes: verified, recorded, and the run goes on to its verdict.
    let mut opts = options("m11-verify-ok", SolverKind::Tcr, exe_for(SolverKind::Tcr));
    opts.verify = Some(manifest_of(&names));
    let (r, events) = run_with_events(&box_, None, &tetgen(), &opts);
    let m = written(&r);
    assert_eq!(m.verdict.status, Status::Ok, "{m:#?}");
    assert_eq!(
        events.order.get(1).map(String::as_str),
        Some("stage Solvers"),
        "{:?}",
        events.order
    );
    let checks = m.solvers.as_ref().expect("the checks are recorded");
    assert_eq!(
        checks.iter().map(|c| c.name.as_str()).collect::<Vec<_>>(),
        names
    );
    assert!(checks.iter().all(|c| c.matches), "{checks:#?}");
    assert!(read_text(&r.dir.join("run.json")).contains("\"solvers\": ["));

    // (2) One wrong hash: refused at stage solvers, exit class 2, no mesh and no launch.
    let mut wrong = manifest_of(&names);
    wrong
        .code_sha256
        .insert("tetgen.exe".into(), "0".repeat(64));
    let mut opts = options("m11-verify-bad", SolverKind::Tcr, exe_for(SolverKind::Tcr));
    opts.verify = Some(wrong);
    let (r, events) = run_with_events(&box_, None, &tetgen(), &opts);
    assert_refused(
        &r,
        Stage::Solvers,
        &[codes::SOLVER_UNVERIFIED],
        ExitClass::Usage,
    );
    let m = written(&r);
    assert_eq!(m.verdict.codes(), [codes::SOLVER_UNVERIFIED]);
    assert!(m.verdict.reasons[0].detail.contains("tetgen.exe"), "{m:#?}");
    assert!(!r.dir.join("mesh").exists(), "nothing was meshed");
    assert!(!r.dir.join("solve").exists(), "nothing was exported");
    assert_eq!(
        events.order,
        ["started", "stage Solvers"],
        "no stage after the refusal"
    );
    let checks = m.solvers.as_ref().unwrap();
    assert_eq!(
        checks.iter().filter(|c| !c.matches).count(),
        1,
        "{checks:#?}"
    );

    // (3) No verify: no stage, and no key in the file.
    let opts = options(
        "m11-verify-none",
        SolverKind::Spps,
        exe_for(SolverKind::Spps),
    );
    let (r, events) = run_with_events(
        &fixture("negative/schema/source_none.simpa"),
        None,
        &tetgen(),
        &opts,
    );
    assert!(!events.order.iter().any(|e| e == "stage Solvers"));
    assert_eq!(written(&r).solvers, None);
    assert!(!read_text(&r.dir.join("run.json")).contains("\"solvers\""));
}

/// C5: the hall imported from PLY, every group on upstream's placeholder, with a source and a
/// receiver: the run is refused at the validate stage with `material_placeholder`, exit class 2,
/// and the solver is never launched.
#[test]
fn a_project_on_the_placeholder_material_is_refused_before_meshing() {
    use simpa_core::geometry::import::{ImportOptions, Unit, Up, import_file};
    let mut hall = import_file(
        &repo_root().join("testdata/elmia_corrected.ply"),
        &ImportOptions::new(Unit::Metre, Up::Z),
    )
    .unwrap()
    .to_project("elmia_corrected");
    let reference = simpa_core::schema::load(&fixture("rooms/elmia_corrected.simpa")).unwrap();
    let mut source = reference.sources[0].clone();
    source.solver_id = None;
    let mut receiver = reference.point_receivers[0].clone();
    receiver.solver_id = None;
    hall.sources.push(source);
    hall.point_receivers.push(receiver);
    let dir = fresh_dir("m11-placeholder");
    let file = dir.join("placeholder_hall.simpa");
    simpa_core::schema::save(&hall, &file).unwrap();
    // The placeholder is the only error: the control that the refusal is its.
    let errors: Vec<&str> = simpa_core::validate::validate(&hall)
        .iter()
        .filter(|i| i.severity == simpa_core::validate::Severity::Error)
        .map(|i| i.code)
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect();
    assert_eq!(errors, ["material_placeholder"]);

    let opts = options(
        "m11-placeholder-run",
        SolverKind::Spps,
        exe_for(SolverKind::Spps),
    );
    let r = project("placeholder", &file, &tetgen(), &opts);
    assert_refused(
        &r,
        Stage::Validate,
        &["material_placeholder"],
        ExitClass::Usage,
    );
    let m = written(&r);
    assert_eq!(m.verdict.codes(), ["material_placeholder"]);
    assert!(
        m.verdict.reasons[0].detail.contains("(and 9 more)"),
        "one reason per code, the other nine groups counted: {}",
        m.verdict.reasons[0].detail
    );
    assert!(!r.dir.join("mesh").exists(), "nothing was meshed");
}

/// C5 on every path a run takes: the run validates the materials it exports. `run_project`
/// exports `variant`'s materials (`None`: the base project); the file's active variant is what
/// the app shows and passes, but the CLI passes `--variant` as given, or nothing. So the rules
/// must be judged under the variant exported, not the one the file has active (M11 review 2,
/// core finding 1). An empty mesh folder stands in for the mesh: a run the validator lets through
/// is refused at the mesh stage instead, and no solver is launched either way.
#[test]
fn the_materials_validated_are_the_materials_exported() {
    use simpa_core::geometry::import::{REFERENCE_MATERIALS, library_material};
    use simpa_core::schema::{MaterialId, Variant, VariantId};
    let base = simpa_core::schema::load(&fixture("rooms/tutorial1_box_seeded.simpa")).unwrap();
    let n = base.bands.frequencies_hz.len();
    let placeholder = library_material(&REFERENCE_MATERIALS[0], MaterialId::from_u128(0xd0), n);
    assert!(simpa_core::validate::is_placeholder_material(&placeholder));
    let dir = fresh_dir("m11-variant-exported");
    let empty = fresh_dir("m11-variant-exported-mesh");
    let opts = options(
        "m11-variant-exported-runs",
        SolverKind::Tcr,
        exe_for(SolverKind::Tcr),
    );
    let run = |file: &Path, variant: Option<&str>| {
        run_project(
            file,
            variant,
            &MeshChoice::Reuse(empty.clone()),
            &opts,
            &CancelToken::new(),
            &mut |_: &RunEvent| {},
        )
        .unwrap()
    };
    let validates = |p: &simpa_core::schema::Project| {
        !simpa_core::validate::has_errors(&simpa_core::validate::validate(p))
    };

    // (1) The base on the placeholder, the active variant "real" choosing every group's own
    // material: what the app shows validates, but a run with no variant exports the base.
    let mut p = base.clone();
    p.materials.push(placeholder.clone());
    let mut real = Variant {
        id: VariantId::from_u128(0xd1),
        name: "real".to_string(),
        overrides: Vec::new(),
    };
    for g in &mut p.surface_groups {
        real.set_override(g.id, Some(g.material));
        g.material = placeholder.id;
    }
    p.variants.push(real);
    p.active_variant = Some(VariantId::from_u128(0xd1));
    assert!(validates(&p), "the control: the active variant validates");
    let file = dir.join("base_placeholder.simpa");
    simpa_core::schema::save(&p, &file).unwrap();
    let r = run(&file, None);
    assert_refused(
        &r,
        Stage::Validate,
        &["material_placeholder"],
        ExitClass::Usage,
    );
    // The control: the variant with the chosen materials passes the validator.
    let r = run(&file, Some("real"));
    assert_refused(&r, Stage::Mesh, &[codes::MESH_MISSING], ExitClass::Mesh);

    // (2) The reverse: the base chooses every material, no variant is active, and the variant
    // "cli" puts every group on the placeholder.
    let mut p = base.clone();
    p.materials.push(placeholder.clone());
    let mut cli = Variant {
        id: VariantId::from_u128(0xd2),
        name: "cli".to_string(),
        overrides: Vec::new(),
    };
    for g in &p.surface_groups {
        cli.set_override(g.id, Some(placeholder.id));
    }
    p.variants.push(cli);
    p.active_variant = None;
    assert!(validates(&p), "the control: the base validates");
    let file = dir.join("variant_placeholder.simpa");
    simpa_core::schema::save(&p, &file).unwrap();
    let r = run(&file, Some("cli"));
    assert_refused(
        &r,
        Stage::Validate,
        &["material_placeholder"],
        ExitClass::Usage,
    );
    let r = run(&file, None);
    assert_refused(&r, Stage::Mesh, &[codes::MESH_MISSING], ExitClass::Mesh);

    // (3) A file whose active variant does not exist is still refused as it stands, whatever
    // the run exports.
    let mut p = base.clone();
    p.active_variant = Some(VariantId::from_u128(0xd3));
    let file = dir.join("active_dangling.simpa");
    std::fs::write(&file, simpa_core::schema::to_json(&p)).unwrap();
    let r = run(&file, None);
    assert_refused(
        &r,
        Stage::Validate,
        &["variant_reference_invalid"],
        ExitClass::Usage,
    );
}

/// `no_band_computed` is judged for the solver the run launches (PQ3 PLAN.md, C25): SPPS with
/// every band off is refused at the validate stage; TCR, which computes every band of the same
/// file, passes it (an empty mesh folder stands in for the mesh, so nothing is launched).
#[test]
fn every_band_off_refuses_that_solver_s_run_and_not_the_other_s() {
    let file = fixture("negative/schema/no_band_computed.simpa");
    let p = simpa_core::schema::load(&file).unwrap();
    assert!(p.solvers.spps.bands_computed.iter().all(|&on| !on));
    assert!(p.solvers.tcr.bands_computed.iter().all(|&on| on));
    let empty = fresh_dir("pq3-no-band-mesh");

    let opts = options("pq3-no-band-spps", SolverKind::Spps, exe_for(SolverKind::Spps));
    let r = project("spps", &file, &MeshChoice::Reuse(empty.clone()), &opts);
    assert_refused(&r, Stage::Validate, &["no_band_computed"], ExitClass::Usage);
    assert_eq!(written(&r).verdict.codes(), ["no_band_computed"]);

    let opts = options("pq3-no-band-tcr", SolverKind::Tcr, exe_for(SolverKind::Tcr));
    let r = project("tcr", &file, &MeshChoice::Reuse(empty), &opts);
    assert_refused(&r, Stage::Mesh, &[codes::MESH_MISSING], ExitClass::Mesh);
    assert!(!written(&r).verdict.codes().contains(&"no_band_computed"));
}
