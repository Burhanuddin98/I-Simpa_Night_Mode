//! `simpa run` end to end through the built binary, on the seeded tutorial box
//! (`tests/fixtures/rooms/tutorial1_box_seeded.simpa`: seed 1, 10,000 particles, M1's reference
//! configuration), with the M1 solvers and TetGen:
//! - TCR is OK with every expected file (gate M6(b)), twice into two distinct folders (M6(h)),
//!   and under a path with `Ł` (M6(g));
//! - SPPS locates a source lying exactly on an internal facet of TetGen 1.6.0's 6-tetrahedron box
//!   because the `.mbin` carries upstream's corner order; in TetGen's order the run is refused
//!   before launch (`source_unlocatable`, exit 5), and `spps.exe` launched on those same inputs
//!   crashes. The seeded box itself, meshed by TetGen 1.5.0 as upstream's 2019 mesh, is OK and
//!   loses per band no more particles than upstream's own mesh of the box, same seed (gate M6(a),
//!   as Burhan judged it on 2026-09-24); the box with its source 5 cm away is OK too, its losses
//!   measured and pinned;
//! - `--mesh <dir>` reuses a mesh, and refuses a stale one (`mesh_out_of_date`, M5(d1)) or one
//!   whose re-mesh was cancelled (`mesh_missing`, M5(d2)) before any solver starts;
//! - a cancel exits 130 with the solver killed mid-run, and `simpa` itself killed mid-run leaves
//!   no solver running 2 s later (M6(f));
//! - a passing test leaves nothing in its scratch root, and a failing one keeps its folder and
//!   names it (the disk emergency of 2026-09-25).

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
    let tetgen_order = simpa_core::mesh::in_tetgen_order(&mesh);
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

/// Per band of a run manifest: `(freq_hz, lost_by_meshing_problems, lost_by_infinite_loops)`,
/// after checking that every band ran all 10,000 particles.
fn losses(m: &Value) -> Vec<(u64, u64, u64)> {
    let bands = m["particles"]["bands"].as_array().unwrap();
    assert_eq!(bands.len(), 27);
    bands
        .iter()
        .map(|b| {
            assert_eq!(b["total"], 10_000, "{b}");
            let n = |k: &str| b[k].as_u64().unwrap();
            (
                n("freq_hz"),
                n("lost_by_meshing_problems"),
                n("lost_by_infinite_loops"),
            )
        })
        .collect()
}

/// The bands of `l` that lost any particle.
fn lossy(l: &[(u64, u64, u64)]) -> Vec<(u64, u64, u64)> {
    l.iter().copied().filter(|&(_, a, b)| a + b > 0).collect()
}

/// `Err` naming each band in which `ours` loses more particles than `reference`, by meshing or by
/// infinite loops: gate M6(a)'s test ("no worse than upstream"), band by band, both runs having
/// the same bands.
fn no_worse(ours: &[(u64, u64, u64)], reference: &[(u64, u64, u64)]) -> Result<(), String> {
    let bands = |l: &[(u64, u64, u64)]| l.iter().map(|b| b.0).collect::<Vec<_>>();
    if bands(ours) != bands(reference) {
        return Err(format!(
            "the runs computed different bands: {:?} and {:?}",
            bands(ours),
            bands(reference)
        ));
    }
    let worse: Vec<String> = ours
        .iter()
        .zip(reference)
        .filter(|(o, r)| o.1 > r.1 || o.2 > r.2)
        .map(|(o, r)| {
            format!(
                "{} Hz: ours loses {} by meshing and {} by loops, the reference {} and {}",
                o.0, o.1, o.2, r.1, r.2
            )
        })
        .collect();
    if worse.is_empty() {
        Ok(())
    } else {
        Err(worse.join("; "))
    }
}

/// A mesh folder built by `mesh::mesh_from_tetgen` from the TetGen output set `base` in the
/// fixture folder `tetgen_rel`, against the seeded box, for `simpa run --mesh`.
fn box_mesh_from(root: &Path, name: &str, tetgen_rel: &str, base: &str) -> PathBuf {
    let project = simpa_core::schema::load(&fixture(BOX)).unwrap();
    let dir = root.join(name);
    let mm = simpa_core::mesh::mesh_from_tetgen(&project, &fixture(tetgen_rel), Some(base), &dir)
        .unwrap();
    assert!(mm.is_ok(), "{mm:#?}");
    dir
}

/// Gate M6(a), as Burhan judged it on 2026-09-24 at 05:13 ("No worse than upstream",
/// `docs/investigations/2026-09-23-upstream-meshing/DECISIONS.md`): the seeded box with SPPS runs
/// OK, and in no band loses more particles than upstream's own mesh of the same room, with the
/// same seed and configuration. M6(c) judges the hall the same way.
///
/// - **Ours:** `simpa run` meshes the box with TetGen 1.5.0, the mesher since decision 3, into
///   2,257 tetrahedra in upstream's corner order; the tutorial's source is inside it by SPPS's own
///   test (`run::locate`), and the run launches, exits 0 and writes all 65 files.
/// - **Upstream's own mesh:** the TetGen output its GUI meshed tutorial 1 with on 2019-06-07
///   (`tests/fixtures/upstream/tutorial1/tetgen`), built into a mesh folder whose `.mbin` must be
///   the 2019 `tetramesh.mbin` byte for byte (sha256 `8a6b3943…`), then `simpa run --mesh` on it:
///   the same project, seed 1 and 10,000 particles, 27 bands.
/// - **Says no:** the same comparison against the 6-tetrahedron mesh TetGen 1.6.0 makes of the box
///   (`tests/fixtures/solver-outputs/tutorial1/tetgen_scene_mesh.1.*`), which loses none: ours,
///   which loses one particle at 2 kHz, is refused.
///
/// Measured on 2026-09-24: both 2,257-tetrahedron runs lose one particle by meshing at 2 kHz and
/// none elsewhere.
#[test]
fn spps_runs_the_seeded_box_ok() {
    let root = scratch("run-spps-gate");
    let o = run(&fixture(BOX), "spps", &root, &[]);
    let m = json(&o);
    summary("box SPPS, gate M6(a)", &o, &m);
    assert_eq!(o.code, 0, "{o:#?}");
    assert_eq!(m["verdict"]["status"], "OK");
    assert_eq!(m["stage"], "solve");
    assert_eq!(codes(&m), Vec::<String>::new());
    assert_eq!(m["lines"]["fail"], 0);
    assert_eq!(m["files"]["expected"], 65);
    assert_eq!(m["files"]["present"], 65);
    let mm: Value =
        serde_json::from_str(&std::fs::read_to_string(run_dir(&m).join("mesh/mesh.json")).unwrap())
            .unwrap();
    assert_eq!(mm["counts"]["build"]["tetrahedra"], 2257);
    let ours = losses(&m);

    // Upstream's own mesh of the box, the same project.
    let upstream_dir = box_mesh_from(
        &root,
        "upstream-2019",
        "upstream/tutorial1/tetgen",
        "scene_mesh",
    );
    let upstream_mbin = std::fs::read(upstream_dir.join("tetramesh.mbin")).unwrap();
    assert_eq!(
        upstream_mbin,
        std::fs::read(fixture("upstream/tutorial1/spps/tetramesh.mbin")).unwrap(),
        "the reference mesh is upstream's 2019 tetramesh.mbin"
    );
    let arg = upstream_dir.display().to_string();
    let u = run(&fixture(BOX), "spps", &root, &["--mesh", &arg]);
    let um = json(&u);
    summary("box SPPS on upstream's 2019 mesh", &u, &um);
    assert_eq!(u.code, 0, "{u:#?}");
    let reference = losses(&um);
    println!(
        "M6(a) bands with a loss (Hz, meshing, loops): ours {:?}, upstream's own mesh {:?}",
        lossy(&ours),
        lossy(&reference)
    );
    no_worse(&ours, &reference).unwrap();

    // Says no: judged against a mesh of the box that loses fewer, ours is refused.
    let six_dir = box_mesh_from(
        &root,
        "six-tetrahedra",
        "solver-outputs/tutorial1",
        "tetgen_scene_mesh",
    );
    let arg = six_dir.display().to_string();
    let s = run(&fixture(BOX), "spps", &root, &["--mesh", &arg]);
    assert_eq!(s.code, 0, "{s:#?}");
    let six = losses(&json(&s));
    assert!(lossy(&six).is_empty(), "{six:?}");
    let refused = no_worse(&ours, &six).unwrap_err();
    println!("M6(a) against the 6-tetrahedron mesh: {refused}");
    assert!(
        refused.starts_with("2000 Hz: ours loses 1 by meshing"),
        "{refused}"
    );
    // And bands that do not line up are refused, not compared.
    assert!(no_worse(&ours, &six[1..]).is_err());
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
    // On TetGen 1.6.0's 6-tetrahedron box none was lost. On TetGen 1.5.0's mesh, upstream's own
    // 2019 one, SPPS loses one particle by meshing at 500 Hz and one at 1250 Hz (measured
    // 2026-09-24, seed 1), as it loses one at 2 kHz with the tutorial's own source (gate M6(a)):
    // far inside the 1 % run limit, and pinned so that a change shows.
    let l = losses(&m);
    println!(
        "moved source: bands with a loss (Hz, meshing, loops): {:?}",
        lossy(&l)
    );
    assert_eq!(lossy(&l), [(500, 1, 0), (1250, 1, 0)]);
    let solve = run_dir(&m).join("solve");
    assert!(
        solve
            .join("Surface receiver/Global/Sound level.csbin")
            .is_file()
    );
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

/// `mesh.json` in `dir` with `edit` applied, written back as the mesher writes it.
fn edit_mesh_json(dir: &Path, edit: impl FnOnce(&mut Value)) {
    let path = dir.join("mesh.json");
    let mut v: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    edit(&mut v);
    std::fs::write(&path, serde_json::to_string_pretty(&v).unwrap() + "\n").unwrap();
}

/// `simpa mesh <project> --out <dir> --json [extra]`.
fn mesh_into(project: &Path, dir: &Path, extra: &[&str]) -> Out {
    let mut args = vec![
        "mesh".to_string(),
        project.display().to_string(),
        "--out".into(),
        dir.display().to_string(),
        "--json".into(),
    ];
    args.extend(extra.iter().map(|s| s.to_string()));
    simpa_run(&args)
}

/// A refusal before launch: the exit code, stage and codes, and no solver started.
fn refused_before_launch(o: &Out, exit: i32, stage: &str, want: &[&str]) {
    assert_eq!(o.code, exit, "{o:#?}");
    let m = json(o);
    assert_eq!(m["stage"], stage, "{m:#}");
    assert_eq!(m["verdict"]["status"], "FAIL", "{m:#}");
    assert_eq!(codes(&m), want, "{m:#}");
    assert_eq!(m["outcome"], Value::Null);
    assert!(!run_dir(&m).join("solver.stdout.txt").exists());
}

/// Burhan's decision 2 of 2026-09-24: a run on a parity-mode mesh is never OK. `run --mesh` reads
/// its `mesh.json`, which is not signed, so a hand-edited one must not make such a mesh runnable
/// (the tutorial-3 follow-ups' critic). The box with a fitting zone, meshed through
/// `preprocess.exe` in parity mode, keeps the marker `preprocess.exe` gives the zone's facets and
/// fails `mesh::verify`:
/// - as written (status FAIL, parity true): exit 4, `mesh_missing` and `mesh_parity`;
/// - its status edited to OK: exit 4, `mesh_parity`;
/// - its status edited to OK and its parity to false: the mesh is held to the geometry before
///   launch, as `run-folder` holds a folder's: exit 5, `mesh_invalid` and the verifier's codes.
///
/// Says no, each check lets a good mesh through: the same project meshed by default runs OK from
/// its folder; the box without a fitting zone, meshed in parity mode, is OK with the right
/// markers, refused for its parity alone, and runs once that flag is edited away (the check
/// before launch finds nothing wrong with it). And no `mesh.json` stands in for the regions: a
/// mesh folder whose `.poly` gives no cells is refused before launch, where `run-folder` would
/// take the manifest's word (`run_manager.rs`).
#[test]
fn a_parity_mesh_is_never_run_whatever_its_mesh_json_says() {
    let root = scratch("run-parity-mesh");
    let with_preprocess = |rel: &str, name: &str| {
        let text = std::fs::read_to_string(fixture(rel)).unwrap();
        assert_eq!(text.matches("\"preprocess\": false").count(), 1, "{rel}");
        let path = root.join(name);
        std::fs::write(
            &path,
            text.replace("\"preprocess\": false", "\"preprocess\": true"),
        )
        .unwrap();
        path
    };
    let fitting = with_preprocess("rooms/tutorial1_box_fitting.simpa", "fitting.simpa");
    let tcr = |project: &Path, dir: &Path| {
        run(
            project,
            "tcr",
            &root.join("runs"),
            &["--mesh", &dir.display().to_string()],
        )
    };

    let parity = root.join("fitting-parity");
    let pm = mesh_into(&fitting, &parity, &["--parity"]);
    assert_eq!(pm.code, 4, "{pm:#?}");
    let pm = json(&pm);
    assert_eq!(
        (pm["status"].as_str(), &pm["parity"]),
        (Some("FAIL"), &Value::Bool(true))
    );
    assert!(strings_of(&pm["codes"]).contains(&"marker_geometry_mismatches".to_string()));
    assert!(
        parity.join("tetramesh.mbin").is_file(),
        "parity mode writes its .mbin"
    );
    refused_before_launch(
        &tcr(&fitting, &parity),
        4,
        "mesh",
        &["mesh_missing", "mesh_parity"],
    );

    edit_mesh_json(&parity, |v| {
        v["status"] = "OK".into();
        v["codes"] = serde_json::json!([]);
    });
    refused_before_launch(&tcr(&fitting, &parity), 4, "mesh", &["mesh_parity"]);

    edit_mesh_json(&parity, |v| v["parity"] = false.into());
    let o = tcr(&fitting, &parity);
    let m = json(&o);
    let found = codes(&m);
    assert_eq!(found[0], "mesh_invalid", "{m:#}");
    assert!(
        found.contains(&"marker_geometry_mismatches".to_string()),
        "{m:#}"
    );
    let found: Vec<&str> = found.iter().map(String::as_str).collect();
    refused_before_launch(&o, 5, "pre_launch", &found);

    // Says no: the default mesh of the same project runs from its folder.
    let default = root.join("fitting-default");
    let dm = mesh_into(&fitting, &default, &[]);
    assert_eq!(dm.code, 0, "{dm:#?}");
    let o = tcr(&fitting, &default);
    assert_eq!(o.code, 0, "{o:#?}");
    assert_eq!(json(&o)["verdict"]["status"], "OK");

    // Says no: the box without a fitting zone in parity mode is OK, with the right markers.
    let plain = with_preprocess(BOX, "box.simpa");
    let box_parity = root.join("box-parity");
    let bm = mesh_into(&plain, &box_parity, &["--parity"]);
    assert_eq!(bm.code, 0, "{bm:#?}");
    let bm = json(&bm);
    assert_eq!(
        (bm["status"].as_str(), &bm["parity"]),
        (Some("OK"), &Value::Bool(true))
    );
    refused_before_launch(&tcr(&plain, &box_parity), 4, "mesh", &["mesh_parity"]);
    edit_mesh_json(&box_parity, |v| v["parity"] = false.into());
    let o = tcr(&plain, &box_parity);
    assert_eq!(o.code, 0, "{o:#?}");
    assert_eq!(json(&o)["verdict"]["status"], "OK");

    // No mesh.json stands in for the regions: the box's default mesh with an open .poly in its
    // folder holds its regions to no cells, and is refused before launch although its manifest
    // is the mesher's record of this .mbin with its regions checked.
    let box_default = root.join("box-open-poly");
    let o = mesh_into(&plain, &box_default, &[]);
    assert_eq!(o.code, 0, "{o:#?}");
    let poly_path = box_default.join("scene_mesh.poly");
    let mut model = simpa_core::formats::poly::read_file(&poly_path).unwrap();
    model.model_faces.pop();
    simpa_core::formats::poly::write_file(&model, &poly_path).unwrap();
    refused_before_launch(
        &tcr(&plain, &box_default),
        5,
        "pre_launch",
        &["mesh_invalid", "regions_unchecked"],
    );
}

/// When `preprocess.exe` gives up, `run` meshes the `.poly` as written, as upstream's GUI does,
/// and records `preprocess_aborted` as a warning in `run.json` (the tutorial-3 follow-ups'
/// decision (d); `docs/solver-contract.md`, "Reason codes"). The tutorial-3 follow-ups' critic
/// found no test that runs a project whose `preprocess.exe` gives up, so the warning could stop
/// being recorded unseen. Here `preprocess.exe` is played by a batch file that prints the line
/// upstream's prints when it gives up and saves nothing, exit 0 (`Preprocess.cpp:100-106`).
/// Says no: the real `preprocess.exe`, which corrects the box, gives no warning.
#[test]
fn a_run_whose_preprocess_gives_up_records_the_warning() {
    let root = scratch("run-preprocess-aborted");
    let text = std::fs::read_to_string(fixture(BOX)).unwrap();
    assert_eq!(text.matches("\"preprocess\": false").count(), 1);
    let project = root.join("box_preprocess.simpa");
    std::fs::write(
        &project,
        text.replace("\"preprocess\": false", "\"preprocess\": true"),
    )
    .unwrap();
    let fake = root.join("gives_up.bat");
    std::fs::write(
        &fake,
        "@echo off\r\necho Mesh reparation has been aborted. The algorithm enter into an \
         infinite loop. Try to stick coplanar faces or destroy manually.\r\nexit /b 0\r\n",
    )
    .unwrap();
    let warnings = |m: &Value| -> Vec<String> {
        m["verdict"]["warnings"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|w| w["code"].as_str().unwrap().to_string())
            .collect()
    };

    let o = run(
        &project,
        "tcr",
        &root,
        &["--preprocess", &fake.display().to_string()],
    );
    assert_eq!(o.code, 0, "{o:#?}");
    let m = json(&o);
    assert_eq!(m["verdict"]["status"], "OK", "{m:#}");
    assert_eq!(warnings(&m), ["preprocess_aborted"], "{m:#}");
    let mm: Value =
        serde_json::from_str(&std::fs::read_to_string(run_dir(&m).join("mesh/mesh.json")).unwrap())
            .unwrap();
    assert_eq!(mm["preprocess"]["outcome"], "aborted", "{mm:#}");
    assert_eq!(
        mm["geometry"]["checked"],
        "written, preprocess.exe having given up"
    );

    // Says no: the real preprocess.exe corrects the box, and no warning is recorded.
    solver_exe("preprocess.exe");
    let o = run(&project, "tcr", &root, &[]);
    assert_eq!(o.code, 0, "{o:#?}");
    let m = json(&o);
    assert_eq!(warnings(&m), Vec::<String>::new(), "{m:#}");
    let mm: Value =
        serde_json::from_str(&std::fs::read_to_string(run_dir(&m).join("mesh/mesh.json")).unwrap())
            .unwrap();
    assert_eq!(mm["preprocess"]["outcome"], "corrected", "{mm:#}");
}

/// Every file and folder below the group folders of a scratch root (`<root>/<group>/...`), which
/// is what the tests that used it left behind.
fn left_below(root: &Path) -> Vec<PathBuf> {
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        for e in std::fs::read_dir(dir).unwrap() {
            let p = e.unwrap().path();
            out.push(p.clone());
            if p.is_dir() {
                walk(&p, out);
            }
        }
    }
    let mut out = Vec::new();
    for group in std::fs::read_dir(root).unwrap() {
        walk(&group.unwrap().path(), &mut out);
    }
    out
}

/// The disk emergency of 2026-09-25: no test removed its scratch folders, and 368,032 files,
/// about 47 GB of B:'s exFAT clusters, had piled up under `target/tmp` and `target/test-runs`.
/// A test's folders now go when it passes and stay, named in its output, when it fails
/// (`crates/simpa-core/tests/common/scratch.rs`). Checked on a representative CLI test, the one
/// above (a project meshed through `preprocess.exe` and TetGen, two TCR runs and their run
/// folders), run again by itself in a child process with its scratch root moved into a folder of
/// this test's (removed with it), so that nothing else running changes the count:
/// - passing: the root holds as many entries after the run as before, none, its group folder
///   `cli/` aside (which shows the child's folder went there);
/// - says no, the cleanup switched off (`$SIMPA_KEEP_SCRATCH=1`, the fault in the code): the
///   count rises by the run's files, so the count sees a folder left behind;
/// - failing (`$SIMPA_SOLVERS_DIR` at an empty folder, the fault in the input: the test's second
///   half finds no `preprocess.exe`): the child fails, its folder is kept with its run folders,
///   and its output names it.
#[test]
fn a_passing_test_leaves_no_scratch_behind_and_a_failing_one_keeps_its_folder() {
    const CHILD: &str = "a_run_whose_preprocess_gives_up_records_the_warning";
    let outer = scratch("scratch-count");
    let no_solvers = outer.join("no-solvers");
    std::fs::create_dir(&no_solvers).unwrap();
    let child = |arm: &str, env: &[(&str, &std::ffi::OsStr)]| {
        // Short, beside the group folders: the run folders nest deep, and a longer root takes
        // the solvers' output paths past MAX_PATH (`output_path_too_long`).
        let root = scratch_files::root().join(format!("sc-{}-{arm}", std::process::id()));
        scratch_files::own(&root);
        std::fs::create_dir(&root).unwrap();
        let before = left_below(&root).len();
        let mut cmd = std::process::Command::new(std::env::current_exe().unwrap());
        cmd.args([CHILD, "--exact", "--test-threads=1"])
            .env(scratch_files::ROOT_ENV, &root)
            .env_remove(scratch_files::KEEP_ENV);
        for (k, v) in env {
            cmd.env(k, v);
        }
        let out = cmd.output().unwrap();
        let text = format!(
            "{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
        let after = left_below(&root);
        println!(
            "{arm}: exit {:?}; entries below {} before {before}, after {}",
            out.status.code(),
            root.display(),
            after.len()
        );
        (out.status.success(), text, root, before, after)
    };
    let is_run_json = |p: &PathBuf| p.file_name().is_some_and(|n| n == "run.json");

    let (passed, text, root, before, after) = child("pass", &[]);
    assert!(passed && text.contains("1 passed"), "{text}");
    assert!(
        root.join("cli").is_dir(),
        "the child's scratch root was not {}",
        root.display()
    );
    assert_eq!((before, after), (0, Vec::new()), "{text}");

    let (passed, text, _, before, after) =
        child("keep", &[(scratch_files::KEEP_ENV, "1".as_ref())]);
    assert!(passed && text.contains("1 passed"), "{text}");
    assert_eq!(before, 0);
    assert!(after.iter().any(is_run_json), "{after:#?}");

    let (passed, text, root, before, after) =
        child("fail", &[("SIMPA_SOLVERS_DIR", no_solvers.as_os_str())]);
    assert!(
        !passed && text.contains("preprocess.exe is missing"),
        "{text}"
    );
    let kept: Vec<PathBuf> = text
        .lines()
        .filter_map(|l| l.trim().strip_prefix(scratch_files::KEPT))
        .map(|p| PathBuf::from(p.trim()))
        .collect();
    assert_eq!(kept.len(), 1, "{text}");
    assert!(kept[0].starts_with(root.join("cli")), "{kept:?}");
    assert_eq!(before, 0);
    assert!(after.contains(&kept[0]), "{after:#?}");
    assert!(
        after
            .iter()
            .filter(|p| p.starts_with(&kept[0]))
            .any(is_run_json),
        "{after:#?}"
    );
    assert!(after.iter().all(|p| p.starts_with(&kept[0])), "{after:#?}");
}

fn strings_of(v: &Value) -> Vec<String> {
    v.as_array()
        .into_iter()
        .flatten()
        .map(|c| c.as_str().unwrap().to_string())
        .collect()
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

/// The inputs of the run `m` (its `solve/` folder), copied into `folder` with the run's working
/// directory written `__RUNDIR__`, as `run-folder` takes a folder of inputs.
fn inputs_copied(m: &Value, folder: &Path) -> PathBuf {
    let solve = run_dir(m).join("solve");
    std::fs::create_dir_all(folder).unwrap();
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
    folder.to_path_buf()
}

/// A project with a box fitting zone. Its run's `mesh.cbin` carries the box's 12 triangles after
/// the room's 12 faces, as upstream's GUI writes them (`idMat` 0, `idRs` -1, `idEn` the zone's
/// id, 2), and no `.mbin` marker names them (`docs/m5-m6-design.md`, decision 5):
/// `mesh::verify` takes them for a drawn zone's triangles, so the run's own folder passes
/// `run-folder`'s pre-launch check, and `mesh-verify` with the zone declared. Before 2026-09-24's
/// fix both refused it (`uncovered_scene_faces`, 12). The say-nos: `mesh-verify` without the
/// zone declared, and the folder with one triangle given a material, refused before launch.
#[test]
fn a_box_fitting_zones_run_folder_passes_its_own_checks() {
    let root = scratch("run-box-fitting");
    let project = fixture("rooms/tutorial1_box_fitting.simpa");
    let o = run(&project, "tcr", &root, &[]);
    assert_eq!(o.code, 0, "{o:#?}");
    let m = json(&o);
    let solve = run_dir(&m).join("solve");
    let scene = simpa_core::formats::cbin::read_file(&solve.join("mesh.cbin")).unwrap();
    assert_eq!(scene.faces.len(), 24);
    assert!(
        scene.faces[12..]
            .iter()
            .all(|f| (f.id_mat, f.id_rs, f.id_en) == (0, -1, 2)),
        "{:?}",
        &scene.faces[12..]
    );

    let verify = |dir: &Path, extra: &[&str]| {
        let mut args = vec!["mesh-verify".to_string(), dir.display().to_string()];
        args.extend(extra.iter().map(|s| s.to_string()));
        simpa_run(&args)
    };
    let declared = verify(&solve, &["--fittings", "2"]);
    assert_eq!(declared.code, 0, "{declared:#?}");
    // Undeclared, the zone's triangles are scene faces no marker names, and TetGen's numbering
    // starts the room at 1: the fitting's 2 and the room's 3 are then no ids it gives.
    let undeclared = verify(&solve, &[]);
    assert_eq!(undeclared.code, 4, "{undeclared:#?}");
    assert!(
        undeclared
            .stdout
            .starts_with("FAIL uncovered_scene_faces, unknown_volume_ids:"),
        "{}",
        undeclared.stdout
    );

    let folder_run = |folder: &Path| {
        simpa_run(&[
            "run-folder".to_string(),
            folder.display().to_string(),
            "--solver".into(),
            "tcr".into(),
            "--runs".into(),
            root.display().to_string(),
            "--json".into(),
        ])
    };
    let folder = inputs_copied(&m, &root.join("as-run"));
    let ok = folder_run(&folder);
    let mo = json(&ok);
    summary("box fitting, run-folder on its own inputs", &ok, &mo);
    assert_eq!(ok.code, 0, "{ok:#?}");
    assert_eq!(mo["verdict"]["status"], "OK");

    // One box triangle given a material: no longer a drawn zone's, so 12 scene faces no marker
    // names. Refused before launch.
    let bad = inputs_copied(&m, &root.join("triangle-with-material"));
    let mut edited = scene.clone();
    edited.faces[17].id_mat = 5;
    simpa_core::formats::cbin::write_file(&edited, &bad.join("mesh.cbin")).unwrap();
    let refused = folder_run(&bad);
    let mr = json(&refused);
    summary("box fitting, one triangle with a material", &refused, &mr);
    assert_eq!(refused.code, 5, "{refused:#?}");
    assert_eq!(mr["stage"], "pre_launch");
    assert_eq!(codes(&mr), ["mesh_invalid", "uncovered_scene_faces"]);
    assert_eq!(mr["outcome"], Value::Null);
}
