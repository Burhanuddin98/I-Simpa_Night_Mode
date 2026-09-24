//! `simpa run` end to end through the built binary, on the seeded tutorial box
//! (`tests/fixtures/rooms/tutorial1_box_seeded.simpa`: seed 1, 10,000 particles, M1's reference
//! configuration), with the M1 solvers and TetGen:
//! - TCR is OK with every expected file (gate M6(b)), twice into two distinct folders (M6(h)),
//!   and under a path with `Ł` (M6(g));
//! - SPPS locates a source lying exactly on an internal facet of TetGen 1.6.0's 6-tetrahedron box
//!   because the `.mbin` carries upstream's corner order; in TetGen's order the run is refused
//!   before launch (`source_unlocatable`, exit 5), and `spps.exe` launched on those same inputs
//!   crashes. With the source 5 cm away it is OK with 10,000 particles per band and none lost;
//! - gate M6(a): the seeded box itself, meshed by TetGen 1.5.0, is OK;
//! - `--mesh <dir>` reuses a mesh, and refuses a stale one (`mesh_out_of_date`, M5(d1)) or one
//!   whose re-mesh was cancelled (`mesh_missing`, M5(d2)) before any solver starts;
//! - a cancel exits 130 with the solver killed mid-run, and `simpa` itself killed mid-run leaves
//!   no solver running 2 s later (M6(f)).

mod support;

use std::path::{Path, PathBuf};

use serde_json::Value;
use support::*;

const BOX: &str = "rooms/tutorial1_box_seeded.simpa";

fn run(project: &Path, solver: &str, root: &Path, extra: &[&str]) -> Out {
    let mut args: Vec<String> = vec![
        "run".into(),
        project.display().to_string(),
        "--solver".into(),
        solver.into(),
        "--runs".into(),
        root.display().to_string(),
        "--json".into(),
    ];
    args.extend(extra.iter().map(|s| s.to_string()));
    simpa_run(&args)
}

/// A copy of the seeded box with `edit` applied to its JSON, written into `dir`.
fn edited_box(dir: &Path, name: &str, edit: impl FnOnce(&mut Value)) -> PathBuf {
    let text = std::fs::read_to_string(fixture(BOX)).unwrap();
    let mut v: Value = serde_json::from_str(&text).unwrap();
    edit(&mut v);
    let path = dir.join(name);
    std::fs::write(&path, serde_json::to_string_pretty(&v).unwrap()).unwrap();
    path
}

/// The box with its source moved 5 cm in x and 10 cm in y, off every internal facet of its mesh.
fn moved_source_box(dir: &Path) -> PathBuf {
    edited_box(dir, "box_source_moved.simpa", |v| {
        v["sources"][0]["position"] = serde_json::json!([3.05, 5.1, 1.8]);
    })
}

fn summary(label: &str, o: &Out, m: &Value) {
    println!(
        "{label}: {} {:?} exit {}; solver {:.0} ms, simpa run {:.0} ms; files {}/{}",
        m["verdict"]["status"],
        codes(m),
        o.code,
        m["outcome"]["elapsed_ms"].as_f64().unwrap_or(f64::NAN),
        o.ms,
        m["files"]["present"],
        m["files"]["expected"]
    );
}

#[test]
fn tcr_runs_the_box_ok_twice_into_two_folders() {
    let root = scratch("run-tcr");
    let a = run(&fixture(BOX), "tcr", &root, &[]);
    assert_eq!(a.code, 0, "{a:#?}");
    let m = json(&a);
    summary("box TCR", &a, &m);
    assert_eq!(m["verdict"]["status"], "OK");
    assert_eq!(m["stage"], "solve");
    assert_eq!(m["files"]["expected"], 87);
    assert_eq!(m["files"]["present"], 87);
    let solve = run_dir(&m).join("solve");
    assert!(solve.join("Main results.gabe").is_file());
    for lbl in ["Receiver 1", "Receiver 2"] {
        assert!(
            solve
                .join(format!("Punctual receivers/{lbl}.gabe"))
                .is_file()
        );
    }
    // The run folder: mesh/ built for it, logs and run.json beside solve/.
    let dir = run_dir(&m);
    assert!(dir.join("mesh/mesh.json").is_file());
    assert!(dir.join("solver.stdout.txt").is_file());
    // Every classified line went to stderr as CLASS  text.
    assert!(
        a.stderr
            .lines()
            .any(|l| l.starts_with("INFO  Classical Theory")),
        "{}",
        a.stderr
    );

    // A second run: its own folder, and no receiver folder with a suffix (M6(h)).
    let b = run(&fixture(BOX), "tcr", &root, &[]);
    assert_eq!(b.code, 0, "{b:#?}");
    let mb = json(&b);
    assert_ne!(run_dir(&m), run_dir(&mb));
    let mut labels: Vec<String> = std::fs::read_dir(run_dir(&mb).join("solve/Punctual receivers"))
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    labels.sort();
    assert_eq!(labels, ["Receiver 1.gabe", "Receiver 2.gabe"]);

    // Without --json: one line naming the verdict, the exit code and the folder.
    let plain = simpa_run(&[
        "run".to_string(),
        fixture(BOX).display().to_string(),
        "--solver".into(),
        "tcr".into(),
        "--runs".into(),
        root.display().to_string(),
    ]);
    assert_eq!(plain.code, 0);
    assert!(
        plain.stdout.starts_with("OK - exit 0: "),
        "{}",
        plain.stdout
    );
    assert!(
        Path::new(plain.stdout.trim().rsplit(": ").next().unwrap())
            .join("run.json")
            .is_file()
    );
}

#[test]
fn a_run_folder_under_a_non_ascii_path_is_ok() {
    let root = scratch("run-utf8").join("Łódź runs");
    let o = run(&fixture(BOX), "tcr", &root, &[]);
    assert_eq!(o.code, 0, "{o:#?}");
    let m = json(&o);
    summary("box TCR under Ł", &o, &m);
    assert_eq!(m["files"]["present"], 87);
    assert!(m["cwd"].as_str().unwrap().contains("Łódź"));
}

/// SPPS with its source on an internal facet, where the corner order decides whether it finds
/// the source. The mesh is the 6-tetrahedron box TetGen 1.6.0 makes of tutorial 1
/// (`tests/fixtures/solver-outputs/tutorial1/tetgen_scene_mesh.1.*`, the same six tetrahedra the
/// pinned 1.6.0 build gave before decision 3 moved the mesher to 1.5.0), built into a mesh folder
/// by `mesh::mesh_from_tetgen`, so the test holds whichever TetGen the solver build carries. The
/// tutorial's source (3, 5, 1.8) lies exactly on its internal facet x/6 + y/10 = 1.
/// `InitSourcesTetraLocalisation` (`lib_interface/coreinitialisation.cpp:71-95`) counts a point
/// as outside a tetrahedron when `(node - source) . normal > 0` for any face, in `f32`, with the
/// normals built from each face's winding (`coreTypes.cpp:233`). On this facet that product is
/// about 2.4e-7, and its sign follows the corner order:
/// - in upstream's order, `(d,c,b,a)` of each `.ele` row as the GUI writes it
///   (`mesh::upstream_order`), it is negative from both sides: the source is inside both
///   tetrahedra, SPPS takes the first, and `simpa run --mesh` on that folder is OK with 10,000
///   particles per band and none lost;
/// - in TetGen's own order, which the builder wrote until upstream's was adopted, it is positive
///   from both sides: neither tetrahedron takes the source, `currentVolume` stays NULL, and
///   `TranslateSourceAtTetrahedronVertex` (`spps/sppsInitialisation.cpp:20`, called at
///   `sppsNantes.cpp:321`) dereferences it before any particle runs: an access violation.
///
/// The second half runs the first run's own inputs with only the `.mbin`'s corners put back in
/// TetGen's order, through `run-folder`. `run::locate` emulates SPPS's test, so the run manager
/// refuses that folder before launch: `source_unlocatable`, exit 5, no solver started, `run.json`
/// written. Then `spps.exe` is launched on the refused folder's own inputs by hand, and crashes
/// with `0xC0000005`: the refusal stands for a real crash, and that folder is the input that
/// makes this test say no. The emulation agrees with `spps.exe` on 233 points on and near this
/// mesh's facets (`crates/simpa-core/tests/run_locate.rs`).
///
/// `mesh::verify` passes both meshes. A margin of 2.4e-7 is why the pre-launch source-location
/// check stays (decision 3's notes).
#[test]
fn spps_finds_a_source_on_an_internal_facet_only_in_upstreams_corner_order() {
    let root = scratch("run-spps-facet");
    let project = simpa_core::schema::load(&fixture(BOX)).unwrap();
    let mesh_dir = root.join("six-tetrahedra");
    let mm = simpa_core::mesh::mesh_from_tetgen(
        &project,
        &fixture("solver-outputs/tutorial1"),
        Some("tetgen_scene_mesh"),
        &mesh_dir,
    )
    .unwrap();
    assert!(mm.is_ok(), "{mm:#?}");
    let mesh = simpa_core::formats::mbin::read_file(&mesh_dir.join("tetramesh.mbin")).unwrap();
    assert_eq!(mesh.tetrahedra.len(), 6);
    let on = internal_faces_holding(&mesh, [3.0, 5.0, 1.8]);
    assert_eq!(
        on, 2,
        "tetrahedron faces holding the source (both sides of one facet)"
    );
    // The moved source of the test below is on none.
    assert_eq!(internal_faces_holding(&mesh, [3.05, 5.1, 1.8]), 0);

    let mesh_arg = mesh_dir.display().to_string();
    let o = run(&fixture(BOX), "spps", &root, &["--mesh", &mesh_arg]);
    let m = json(&o);
    summary("box SPPS, 6 tetrahedra, upstream's corner order", &o, &m);
    assert_eq!(o.code, 0, "{o:#?}");
    assert_eq!(m["verdict"]["status"], "OK");
    let bands = m["particles"]["bands"].as_array().unwrap();
    assert_eq!(bands.len(), 27);
    for b in bands {
        assert_eq!(b["total"], 10_000, "{b}");
        assert_eq!(b["lost_by_meshing_problems"], 0, "{b}");
    }
    let solve = run_dir(&m).join("solve");
    assert_eq!(
        std::fs::read(solve.join("tetramesh.mbin")).unwrap(),
        std::fs::read(mesh_dir.join("tetramesh.mbin")).unwrap(),
        "the run solved the 6-tetrahedron mesh"
    );

    // The same inputs, the .mbin in TetGen's corner order: refused before launch.
    let folder = root.join("tetgen-order");
    std::fs::create_dir_all(&folder).unwrap();
    for input in m["inputs"].as_array().unwrap() {
        let name = input["path"].as_str().unwrap();
        std::fs::copy(solve.join(name), folder.join(name)).unwrap();
    }
    let config = std::fs::read_to_string(folder.join("config.xml")).unwrap();
    let solve_dir = format!("{}\\", solve.display());
    assert!(config.contains(&solve_dir), "{config}");
    std::fs::write(
        folder.join("config.xml"),
        config.replace(&solve_dir, "__RUNDIR__"),
    )
    .unwrap();
    let tetgen_order = in_tetgen_order(&mesh);
    simpa_core::formats::mbin::write_file(&tetgen_order, &folder.join("tetramesh.mbin")).unwrap();
    let folder_run = |json_out: bool| {
        let mut args = vec![
            "run-folder".to_string(),
            folder.display().to_string(),
            "--solver".into(),
            "spps".into(),
            "--runs".into(),
            root.display().to_string(),
        ];
        if json_out {
            args.push("--json".into());
        }
        simpa_run(&args)
    };
    let o = folder_run(true);
    let m = json(&o);
    summary("box SPPS, .mbin in TetGen's corner order", &o, &m);
    assert_eq!(o.code, 5, "{o:#?}");
    assert_eq!(m["verdict"]["status"], "FAIL");
    assert_eq!(m["stage"], "pre_launch");
    assert_eq!(m["exit_class"], 5);
    assert_eq!(codes(&m), ["source_unlocatable"]);
    let detail = m["verdict"]["reasons"][0]["detail"].as_str().unwrap();
    assert!(
        detail.starts_with("source 1 \"Source 1\" at (3, 5, 1.8): "),
        "{detail}"
    );
    // Not launched: no outcome, no solver logs, no solver output; run.json written.
    let dir = run_dir(&m);
    assert_eq!(m["outcome"], Value::Null);
    assert!(dir.join("run.json").is_file());
    assert!(!dir.join("solver.stdout.txt").exists());
    assert!(!dir.join("solver.stderr.txt").exists());
    assert!(!dir.join("solve/SPPS particle statistics.gabe").exists());
    // Without --json: the one verdict line names the refusal.
    let plain = folder_run(false);
    assert_eq!(plain.code, 5);
    assert!(
        plain.stdout.starts_with("FAIL source_unlocatable exit 5: "),
        "{}",
        plain.stdout
    );
    // What the refusal spares: spps.exe on the refused folder's own inputs, launched by hand.
    let refused = dir.join("solve");
    let crashed = std::process::Command::new(solver_exe("spps.exe"))
        .arg("config.xml")
        .current_dir(&refused)
        .stdin(std::process::Stdio::null())
        .output()
        .expect("spps.exe starts");
    let exit = crashed.status.code().map(|c| c as u32);
    println!(
        "spps.exe by hand on the refused inputs: exit {:?}",
        exit.map(|c| format!("0x{c:08X}"))
    );
    assert_eq!(
        exit,
        Some(0xC000_0005),
        "SPPS did not crash on the inputs the check refused: {}",
        String::from_utf8_lossy(&crashed.stdout)
    );
}

/// `mesh` with each tetrahedron's corners in TetGen's `.ele` order: upstream's `(d,c,b,a)`
/// reversed, each face rebuilt from the reversed corners and keeping the marker and neighbour of
/// the face opposite the same node, as the builder wrote it before it took upstream's order.
fn in_tetgen_order(mesh: &simpa_core::formats::mbin::Mesh) -> simpa_core::formats::mbin::Mesh {
    use simpa_core::mesh::{FACE_CORNERS, UPSTREAM_CORNERS};
    let mut out = mesh.clone();
    for t in &mut out.tetrahedra {
        let old = *t;
        t.vertices = UPSTREAM_CORNERS.map(|k| old.vertices[k]);
        t.faces = std::array::from_fn(|k| simpa_core::formats::mbin::TetraFace {
            vertices: FACE_CORNERS[k].map(|c| t.vertices[c]),
            ..old.faces[UPSTREAM_CORNERS[k]]
        });
    }
    out
}

/// How many internal tetrahedron faces (with a neighbour) hold `p`: on the face's plane to
/// 1e-9 m and inside its triangle.
fn internal_faces_holding(mesh: &simpa_core::formats::mbin::Mesh, p: [f64; 3]) -> usize {
    let sub = |a: [f64; 3], b: [f64; 3]| [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
    let cross = |a: [f64; 3], b: [f64; 3]| {
        [
            a[1] * b[2] - a[2] * b[1],
            a[2] * b[0] - a[0] * b[2],
            a[0] * b[1] - a[1] * b[0],
        ]
    };
    let dot = |a: [f64; 3], b: [f64; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
    let node = |i: i32| mesh.nodes[i as usize].map(f64::from);
    let mut n = 0;
    for t in &mesh.tetrahedra {
        for f in t.faces.iter().filter(|f| f.neighbor >= 0) {
            let [a, b, c] = f.vertices.map(node);
            let normal = cross(sub(b, a), sub(c, a));
            let len = dot(normal, normal).sqrt();
            if (dot(normal, sub(p, a)) / len).abs() > 1e-9 {
                continue;
            }
            // Inside: p is on the inner side of each edge, within the plane.
            let inside = [(a, b), (b, c), (c, a)]
                .iter()
                .all(|&(u, v)| dot(cross(sub(v, u), sub(p, u)), normal) >= -1e-12);
            n += usize::from(inside);
        }
    }
    n
}

/// Gate M6(a): the seeded box with SPPS exits 0 with status OK. With TetGen 1.5.0, the mesher
/// since decision 3, the box is upstream's own 2019 mesh (`simpa-core/tests/mesh_mbin_parity.rs`),
/// in upstream's corner order, and the tutorial's source is on no internal facet of it, so
/// `run::locate` finds it and the run launches. Before, TetGen 1.6.0 left the box at 6
/// tetrahedra with the source on an internal facet (the test above).
#[test]
fn spps_runs_the_seeded_box_ok() {
    let o = run(&fixture(BOX), "spps", &scratch("run-spps-gate"), &[]);
    assert_eq!(o.code, 0, "{}", o.stdout);
}

#[test]
fn spps_runs_the_box_ok_with_its_source_off_the_internal_facets() {
    let root = scratch("run-spps-moved");
    let project = moved_source_box(&root);
    let o = run(&project, "spps", &root, &[]);
    let m = json(&o);
    summary("box SPPS, source moved 5 cm", &o, &m);
    assert_eq!(o.code, 0, "{o:#?}");
    assert_eq!(m["verdict"]["status"], "OK");
    assert_eq!(m["lines"]["fail"], 0);
    assert_eq!(m["lines"]["warn"], 0);
    assert_eq!(m["files"]["expected"], 65);
    assert_eq!(m["files"]["present"], 65);
    let bands = m["particles"]["bands"].as_array().unwrap();
    assert_eq!(bands.len(), 27);
    for b in bands {
        assert_eq!(b["total"], 10_000, "{b}");
        assert_eq!(b["lost_by_meshing_problems"], 0, "{b}");
        assert_eq!(b["lost_by_infinite_loops"], 0, "{b}");
    }
    let solve = run_dir(&m).join("solve");
    assert!(
        solve
            .join("Surface receiver/Global/Sound level.csbin")
            .is_file()
    );
    assert!(!o.stderr.contains("particles has been in error"));
}

#[test]
fn a_mesh_folder_is_reused_and_refused_when_stale_or_cancelled() {
    let root = scratch("run-reuse");
    let mesh_dir = root.join("box-mesh");
    let meshed = simpa_run(&[
        "mesh".to_string(),
        fixture(BOX).display().to_string(),
        "--out".into(),
        mesh_dir.display().to_string(),
    ]);
    assert_eq!(meshed.code, 0, "{meshed:#?}");
    let mesh_arg = mesh_dir.display().to_string();

    // Reused: no mesh/ in the run folder, and the manifest names the mesh it used.
    let o = run(&fixture(BOX), "tcr", &root, &["--mesh", &mesh_arg]);
    assert_eq!(o.code, 0, "{o:#?}");
    let m = json(&o);
    assert!(!run_dir(&m).join("mesh").exists());
    assert_eq!(
        m["mesh"]["manifest"],
        mesh_dir.join("mesh.json").display().to_string()
    );

    // M5(d1): a vertex moved after meshing: mesh_out_of_date, exit 4, no solver started.
    let moved = edited_box(&root, "box_vertex_moved.simpa", |v| {
        v["geometry"]["vertices"][0][2] = serde_json::json!(0.25);
    });
    let o = run(&moved, "tcr", &root, &["--mesh", &mesh_arg]);
    assert_eq!(o.code, 4, "{o:#?}");
    let m = json(&o);
    assert_eq!(codes(&m), ["mesh_out_of_date"]);
    assert_eq!(m["outcome"], Value::Null);
    assert_eq!(m["stage"], "mesh");
    assert!(!run_dir(&m).join("solver.stdout.txt").exists());
    assert!(!run_dir(&m).join("solve").exists());

    // M5(d2): a re-mesh cancelled 1 ms into TetGen leaves no .mbin: mesh_missing, exit 4.
    let cancelled = simpa_run(&[
        "mesh".to_string(),
        fixture(BOX).display().to_string(),
        "--out".into(),
        mesh_arg.clone(),
        "--cancel-after-ms".into(),
        "1".into(),
    ]);
    assert_eq!(cancelled.code, 130, "{cancelled:#?}");
    assert!(!mesh_dir.join("tetramesh.mbin").exists());
    let o = run(&fixture(BOX), "tcr", &root, &["--mesh", &mesh_arg]);
    assert_eq!(o.code, 4, "{o:#?}");
    let m = json(&o);
    assert_eq!(codes(&m), ["mesh_missing"]);
    assert!(!run_dir(&m).join("solver.stdout.txt").exists());
}

/// Particles per source of [`long_box`]: enough that SPPS, left alone, runs for many seconds
/// (measured on Grace, debug `simpa`: 17.1 s and 65/65 files, against 0.5 s for the box's
/// 10,000), so a solver that was not killed shows as one that finished or still runs, never as
/// one that happened to end in time.
const LONG_RUN_PARTICLES: u64 = 1_000_000;

/// The moved-source box with [`LONG_RUN_PARTICLES`] per source: the run the cancel and kill
/// tests stop.
fn long_box(dir: &Path) -> PathBuf {
    edited_box(dir, "box_long_run.simpa", |v| {
        v["sources"][0]["position"] = serde_json::json!([3.05, 5.1, 1.8]);
        v["solvers"]["spps"]["particles_per_source"] = serde_json::json!(LONG_RUN_PARTICLES);
    })
}

/// M6(f), first clause. The cancel must stop a running SPPS, not merely be reported: the solver
/// ends long before it could have finished, with its outputs incomplete. Each assertion fails
/// when the tree is not killed (the solver then runs to the end, writing every file) or when
/// the cancel is not passed on (exit 0, OK).
#[test]
fn a_cancelled_run_exits_130_and_leaves_no_solver_running() {
    let root = scratch("run-cancel");
    let project = long_box(&root);
    for (label, extra) in [
        ("progress", ["--cancel-after-progress", "1"]),
        ("time", ["--cancel-after-ms", "150"]),
    ] {
        let image = format!("spps-cancel-{label}-{}.exe", std::process::id());
        let exe = private_copy(&solver_exe("spps.exe"), &root, &image);
        let exe_arg = exe.display().to_string();
        let mut args: Vec<&str> = vec!["--solver-exe", &exe_arg];
        args.extend(extra);
        let o = run(&project, "spps", &root, &args);
        assert_eq!(o.code, 130, "{label}: {o:#?}");
        let m = json(&o);
        summary(&format!("box SPPS cancelled by {label}"), &o, &m);
        assert_eq!(m["verdict"]["status"], "CANCELLED");
        assert_eq!(codes(&m), ["cancelled"]);
        assert_eq!(m["outcome"]["cancelled"], true);
        assert_eq!(m["outcome"]["exit_code"], Value::Null, "killed, not exited");
        // Killed mid-run: the outputs are incomplete, and the solver stopped far sooner than the
        // run's 17 s (measured: 0/65 files after 0.1 s, for both cancels).
        let (present, expected) = (
            m["files"]["present"].as_u64().unwrap(),
            m["files"]["expected"].as_u64().unwrap(),
        );
        assert!(
            present < expected,
            "{label}: {present}/{expected} files: SPPS ran to its end"
        );
        let solver_ms = m["outcome"]["elapsed_ms"].as_f64().unwrap();
        assert!(
            solver_ms < 5_000.0,
            "{label}: SPPS ran {solver_ms:.0} ms after a cancel at 1 % or 150 ms"
        );
        assert!(!image_running(&image), "{image} still runs");
        std::fs::remove_file(&exe).expect("the SPPS copy is no longer running");
    }
}

/// M6(f), second clause: `simpa.exe` killed mid-run (`Stop-Process` is `TerminateProcess`, as
/// `Child::kill` is) leaves no solver running 2 s later. Nothing in `simpa` runs after the kill:
/// only the job's `KILL_ON_JOB_CLOSE`, applied when the kernel closes the dead process's job
/// handle, can end the solver. Fails when that flag is missing: SPPS then runs on for the rest
/// of its 17 s.
#[test]
fn killing_simpa_mid_run_leaves_no_solver_running() {
    use std::io::{BufRead, BufReader};
    use std::process::{Command, Stdio};
    use std::time::{Duration, Instant};

    let root = scratch("run-killed");
    let project = long_box(&root);
    let image = format!("spps-killed-{}.exe", std::process::id());
    let exe = private_copy(&solver_exe("spps.exe"), &root, &image);
    let mut child = Command::new(simpa())
        .arg("run")
        .arg(&project)
        .args(["--solver", "spps", "--runs"])
        .arg(&root)
        .arg("--solver-exe")
        .arg(&exe)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .expect("simpa starts");
    // The first PROGRESS line: the solver has been launched and is propagating particles.
    let (tx, rx) = std::sync::mpsc::channel();
    let stderr = child.stderr.take().unwrap();
    let reader = std::thread::spawn(move || {
        for line in BufReader::new(stderr).lines() {
            let Ok(line) = line else { break };
            if line.starts_with("PROGRESS  ") {
                let _ = tx.send(line);
            }
        }
    });
    let first = rx.recv_timeout(Duration::from_secs(120));
    let running = image_running(&image);
    child.kill().expect("simpa is killed");
    child.wait().unwrap();
    let killed = Instant::now();
    let first = first.expect("simpa printed no PROGRESS line within 120 s");
    assert!(
        running,
        "{image} did not run when simpa was killed ({first})"
    );
    // M6(f): 2 s later, no solver.
    std::thread::sleep(Duration::from_secs(2).saturating_sub(killed.elapsed()));
    let after_2s = image_running(&image);
    reader.join().unwrap();
    assert!(
        !after_2s,
        "{image} still runs 2 s after simpa was killed at {first}"
    );
    std::fs::remove_file(&exe).expect("the SPPS copy is no longer running");
    println!("simpa killed at {first}: no {image} 2 s later");
}

#[test]
fn bad_options_and_refused_projects_have_their_exit_codes() {
    let root = scratch("run-refused");
    for bad in ["NaN", "-0.5"] {
        let o = run(&fixture(BOX), "spps", &root, &["--loss-limit", bad]);
        assert_eq!(o.code, 2, "{bad}: {o:#?}");
        assert!(o.stderr.contains("--loss-limit"), "{}", o.stderr);
    }
    let o = run(&fixture(BOX), "fdtd", &root, &[]);
    assert_eq!(o.code, 2);
    // Geometry refused: exit 3, with a run folder that says so.
    let o = run(
        &fixture("geometry/two_boxes_interpenetrating.simpa"),
        "tcr",
        &root,
        &[],
    );
    assert_eq!(o.code, 3, "{o:#?}");
    assert_eq!(codes(&json(&o)), ["geometry_refused"]);
    // A project rule broken: exit 2.
    let o = run(
        &fixture("negative/schema/source_none.simpa"),
        "tcr",
        &root,
        &[],
    );
    assert_eq!(o.code, 2, "{o:#?}");
    assert_eq!(codes(&json(&o)), ["source_none"]);
}
