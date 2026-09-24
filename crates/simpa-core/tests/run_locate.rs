//! `run::locate` against SPPS itself. The emulation of SPPS's `f32` point-in-tetrahedron test is
//! checked on meshes made by the real TetGen of the M1 build (`$SIMPA_SOLVERS_DIR`, see
//! `common/paths.rs`; a missing build panics, nothing skips), on a hand-made two-tetrahedron
//! mesh, and then against `spps.exe` on points on and near every internal facet of the seeded
//! tutorial box: the emulation says a source is in no tetrahedron exactly when SPPS crashes with
//! `0xC0000005`. Last, the bed behind `receiver_unlocatable`: on a refined box, a point receiver
//! the emulation locates nowhere reads exactly zero in a run SPPS calls finished, while the same
//! receiver 1 mm away does not.

#[path = "run_support.rs"]
mod support;

use std::path::{Path, PathBuf};

use roxmltree::Document;
use simpa_core::config_xml::{self, names};
use simpa_core::formats::gabe::{self, ColumnData};
use simpa_core::formats::{cbin, mbin};
use simpa_core::mesh::{self, TetgenMesher};
use simpa_core::process::{self, CancelToken, Line, Spec, Stream};
use simpa_core::run::locate::{self, Kind, TetraTest};
use simpa_core::run::verdict::codes;
use simpa_core::schema::{self, Project, SolverKind, Vec3};
use support::*;

const BOX: &str = "rooms/tutorial1_box_seeded.simpa";
/// The one band the solver runs below: 1000 Hz, entry 13 of the box's 27 third octaves.
const BAND: usize = 13;
const ACCESS_VIOLATION: u32 = 0xC000_0005;

fn box_project() -> Project {
    schema::load(&fixture(BOX)).unwrap()
}

/// A position as SPPS reads it from attribute texts (`ToFloat`).
fn at(x: &str, y: &str, z: &str) -> locate::Vec3 {
    [x, y, z].map(|t| locate::to_float(t).unwrap())
}

/// `project` meshed with the M1 TetGen into a fresh folder, and its `.mbin`.
fn meshed(project: &Project, label: &str) -> (PathBuf, mbin::Mesh) {
    let dir = fresh_dir(label);
    let m = mesh::mesh_project(
        project,
        &dir,
        &TetgenMesher::new(solver_exe("tetgen.exe")),
        &CancelToken::new(),
        &mut |_| {},
    )
    .unwrap();
    assert!(m.is_ok(), "{m:#?}");
    let mesh = mbin::read_file(&dir.join(names::TETRA_MESH)).unwrap();
    (dir, mesh)
}

/// A run folder for SPPS in the new folder `dir`: `project`'s config, its scene mesh, and the
/// `.mbin` at `mbin`, as the run manager exports them.
fn stage(project: &Project, mbin: &Path, dir: &Path) {
    std::fs::create_dir_all(dir).unwrap();
    config_xml::write_file(project, SolverKind::Spps, None, dir).unwrap();
    let scene = config_xml::scene_mesh(project).unwrap();
    cbin::write_file(&scene, &dir.join(names::SCENE_MESH)).unwrap();
    std::fs::copy(mbin, dir.join(names::TETRA_MESH)).unwrap();
}

/// What SPPS did in `dir`: its exit code and whether it printed `End of calculation.`.
fn spps(dir: &Path) -> (Option<u32>, bool) {
    let spec = Spec {
        program: solver_exe("spps.exe"),
        args: vec!["config.xml".into()],
        cwd: dir.to_path_buf(),
    };
    let mut end = false;
    let outcome = process::run(&spec, &CancelToken::new(), &mut |l: &Line| {
        end |= l.stream == Stream::Stdout && l.text == "End of calculation.";
    })
    .unwrap();
    (outcome.exit_code, end)
}

/// The one source's position in `dir/config.xml` as SPPS stores it, and the tetrahedron SPPS
/// puts it in, by the emulation.
fn emulated(dir: &Path, test: &TetraTest) -> (locate::Vec3, Option<usize>) {
    let text = read_text(&dir.join(names::CONFIG));
    let doc = Document::parse(&text).unwrap();
    let p = locate::points(&doc)
        .into_iter()
        .find(|p| p.kind == Kind::Source)
        .unwrap()
        .position
        .unwrap();
    (p, test.locate(p))
}

fn set_source(project: &mut Project, p: [f64; 3]) {
    project.sources[0].position = Vec3::from(p);
}

/// Few particles, one band: SPPS reaches its source location in milliseconds.
fn short_runs(project: &mut Project, particles: u32) {
    let spps = &mut project.solvers.spps;
    spps.particles_per_source = particles;
    spps.bands_computed = (0..spps.bands_computed.len()).map(|i| i == BAND).collect();
}

fn node(mesh: &mbin::Mesh, i: i32) -> [f64; 3] {
    mesh.nodes[i as usize].map(f64::from)
}

/// Each internal facet once: (the tetrahedron, the one across, its three corners), from the
/// lower-numbered side, in file order.
fn internal_facets(mesh: &mbin::Mesh) -> Vec<(usize, usize, [[f64; 3]; 3])> {
    let mut out = Vec::new();
    for (t, tet) in mesh.tetrahedra.iter().enumerate() {
        for f in &tet.faces {
            if usize::try_from(f.neighbor).is_ok_and(|n| n > t) {
                out.push((t, f.neighbor as usize, f.vertices.map(|v| node(mesh, v))));
            }
        }
    }
    out
}

/// `a + s (b - a) + t (c - a)`.
fn on_facet([a, b, c]: [[f64; 3]; 3], s: f64, t: f64) -> [f64; 3] {
    [0, 1, 2].map(|k| a[k] + s * (b[k] - a[k]) + t * (c[k] - a[k]))
}

fn unit_normal([a, b, c]: [[f64; 3]; 3]) -> [f64; 3] {
    let (u, v) = (
        [b[0] - a[0], b[1] - a[1], b[2] - a[2]],
        [c[0] - a[0], c[1] - a[1], c[2] - a[2]],
    );
    let n = [
        u[1] * v[2] - u[2] * v[1],
        u[2] * v[0] - u[0] * v[2],
        u[0] * v[1] - u[1] * v[0],
    ];
    let l = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
    n.map(|x| x / l)
}

// ---------------------------------------------------------------------------------------------
// The emulation alone

#[test]
fn the_seeded_boxs_source_is_in_no_tetrahedron_and_its_neighbours_are() {
    let (_, mesh) = meshed(&box_project(), "locate-box");
    assert_eq!(mesh.tetrahedra.len(), 6);
    let test = TetraTest::new(&mesh).unwrap();
    // The tutorial's source, on the internal facet x/6 + y/10 = 1: outside both tetrahedra that
    // share it, and outside the other four.
    assert_eq!(test.locate(at("3", "5", "1.8")), None);
    // 5 cm and 10 cm off it, and 1 mm off it: inside one.
    assert!(test.locate(at("3.05", "5.1", "1.8")).is_some());
    assert!(test.locate(at("3", "5.001", "1.8")).is_some());
    // The source is on the facet in exact arithmetic: it is the f32 rounding that loses it.
    let a = at("3", "5", "1.8").map(f64::from);
    assert!((a[0] / 6.0 + a[1] / 10.0 - 1.0).abs() < 1e-12);
}

/// The box's two tetrahedra across the facet that holds its source, rebuilt by hand with the
/// corners and face orders the pinned TetGen gives them (tetrahedra 0 and 4 of the box).
fn two_tetrahedra() -> mbin::Mesh {
    let face = |v: [i32; 3], neighbor: i32| mbin::TetraFace {
        vertices: v,
        marker: if neighbor < 0 { 0 } else { -1 },
        neighbor,
    };
    mbin::Mesh {
        nodes: vec![
            [0.0, 10.0, 0.0],
            [6.0, 0.0, 3.0],
            [6.0, 10.0, 3.0],
            [0.0, 10.0, 3.0],
            [0.0, 0.0, 3.0],
        ],
        tetrahedra: vec![
            mbin::Tetrahedron {
                vertices: [0, 1, 2, 3],
                id_volume: 0,
                faces: [
                    face([1, 3, 2], -2),
                    face([2, 3, 0], -2),
                    face([0, 3, 1], 1),
                    face([1, 2, 0], -2),
                ],
            },
            mbin::Tetrahedron {
                vertices: [4, 1, 0, 3],
                id_volume: 0,
                faces: [
                    face([1, 3, 0], 0),
                    face([0, 3, 4], -2),
                    face([4, 3, 1], -2),
                    face([1, 0, 4], -2),
                ],
            },
        ],
    }
}

#[test]
fn a_hand_made_two_tetrahedron_mesh_says_no_on_its_shared_face_and_yes_inside() {
    let mesh = two_tetrahedra();
    mbin::validate(&mesh).unwrap();
    let test = TetraTest::new(&mesh).unwrap();
    let on = at("3", "5", "1.8");
    assert!(!test.holds(0, on) && !test.holds(1, on));
    assert_eq!(test.locate(on), None);
    // Clearly inside each: the centroids.
    let centroid = |t: usize| {
        let v = mesh.tetrahedra[t].vertices.map(|i| node(&mesh, i));
        [0, 1, 2].map(|k| ((v[0][k] + v[1][k] + v[2][k] + v[3][k]) / 4.0) as f32)
    };
    assert_eq!(test.locate(centroid(0)), Some(0));
    assert_eq!(test.locate(centroid(1)), Some(1));
    // 1 mm off the face on either side: the tetrahedron on that side.
    assert_eq!(test.locate(at("3.001", "5", "1.8")), Some(0));
    assert_eq!(test.locate(at("2.999", "5", "1.8")), Some(1));
    // Outside both: nowhere.
    assert_eq!(test.locate(at("3", "5", "3.5")), None);
    // Other points of the shared face round every way. Exactly 0 on both sides: the strict test
    // puts it in both, and the first wins. Or in one of the two only, either one.
    let both = at("1.2", "8", "2.4");
    assert!(test.holds(0, both) && test.holds(1, both));
    assert_eq!(test.locate(both), Some(0));
    let second = at("2.1", "6.5", "2.2");
    assert!(!test.holds(0, second) && test.holds(1, second));
    assert_eq!(test.locate(second), Some(1));
    let first = at("2.4", "6", "2.4");
    assert!(test.holds(0, first) && !test.holds(1, first));
    assert_eq!(test.locate(first), Some(0));
    assert_eq!(test.locate(at("1.5", "7.5", "2.25")), None);
    // A face that names a node the mesh does not have: nothing to emulate.
    let mut bad = mesh.clone();
    bad.tetrahedra[1].faces[2].vertices[0] = 9;
    let e = TetraTest::new(&bad).unwrap_err();
    assert_eq!((e.tetrahedron, e.face, e.index), (1, 2, 9));
}

#[test]
fn the_check_names_each_unlocatable_source_and_receiver_and_passes_the_rest() {
    let mut project = box_project();
    let (mesh_dir, mesh) = meshed(&project, "locate-check");
    let mbin_path = mesh_dir.join(names::TETRA_MESH);
    let reasons = |project: &Project, label: &str| {
        let dir = fresh_dir(label);
        stage(project, &mbin_path, &dir);
        locate::check_folder(&dir)
    };
    // The box as it is: its source is lost, its two receivers are found.
    let r = reasons(&project, "check-box");
    assert_eq!(r.len(), 1, "{r:#?}");
    assert_eq!(r[0].code, codes::SOURCE_UNLOCATABLE);
    assert!(
        r[0].detail
            .starts_with("source 1 \"Source 1\" at (3, 5, 1.8): "),
        "{}",
        r[0].detail
    );
    assert!(
        r[0].detail
            .contains("none of the 6 tetrahedra of tetramesh.mbin")
    );
    // Receiver 2 moved onto the source's spot: both refused, each once, naming only what is lost.
    project.point_receivers[1].position = Vec3::new(3.0, 5.0, 1.8);
    let r = reasons(&project, "check-both");
    let got: Vec<&str> = r.iter().map(|r| r.code.as_str()).collect();
    assert_eq!(
        got,
        [codes::SOURCE_UNLOCATABLE, codes::RECEIVER_UNLOCATABLE]
    );
    assert!(
        r[1].detail
            .starts_with("point receiver 2 \"Receiver 2\" at (3, 5, 1.8): "),
        "{}",
        r[1].detail
    );
    assert!(!r[1].detail.contains("Receiver 1"));
    // Both moved 5 cm and 10 cm off the facet: nothing.
    set_source(&mut project, [3.05, 5.1, 1.8]);
    project.point_receivers[1].position = Vec3::new(3.05, 5.1, 1.8);
    assert_eq!(reasons(&project, "check-off"), []);

    // The config as SPPS reads it: every child in order, a missing attribute as 0, a comma as the
    // decimal point.
    let doc = Document::parse(
        r#"<configuration><sources><source name="A" x="3" y="5,0" z=" 1.8"/>
        <source name="B" x="1" z="2"/></sources>
        <recepteursp><anything lbl="R" x="0x1p1" y="1" z="1"/></recepteursp></configuration>"#,
    )
    .unwrap();
    let pts = locate::points(&doc);
    assert_eq!(pts.len(), 3);
    assert_eq!(pts[0].position, Some(at("3", "5", "1.8")));
    assert_eq!(pts[1].position, Some([1.0, 0.0, 2.0]));
    assert_eq!((pts[2].kind, pts[2].number), (Kind::PointReceiver, 1));
    assert_eq!(pts[2].position, None, "a hexadecimal float is not emulated");
    let test = TetraTest::new(&mesh).unwrap();
    let lost = locate::unlocated(&doc, &test);
    assert_eq!(
        lost.iter().map(|p| p.label.as_str()).collect::<Vec<_>>(),
        ["A"],
        "(1, 0, 2) is on the wall y = 0, where the product is exactly 0 and the strict test \
         holds it; the receiver is not judged"
    );
    // A mesh that names a node it does not have: no finding, the mesh check refuses it.
    let mut bad = mesh.clone();
    bad.tetrahedra[0].faces[0].vertices[0] = -1;
    assert_eq!(locate::check(&doc, &bad, "bad.mbin"), []);
}

// ---------------------------------------------------------------------------------------------
// Against SPPS

/// One point of the agreement table.
struct Row {
    what: String,
    text: [String; 3],
    tetrahedron: Option<usize>,
    refused: bool,
    exit: Option<u32>,
    end: bool,
}

#[test]
fn the_emulation_agrees_with_spps_on_and_near_the_boxs_internal_facets() {
    let mut project = box_project();
    short_runs(&mut project, 10);
    let (mesh_dir, mesh) = meshed(&project, "agree-mesh");
    let mbin_path = mesh_dir.join(names::TETRA_MESH);
    let test = TetraTest::new(&mesh).unwrap();

    // The three points known from the box's own run, two outside the room, then per internal
    // facet: a barycentric grid of 36 points on it (s, t in tenths, both at least 0.1 and at
    // most 0.9 together), and its centroid 1 mm off on either side.
    let mut points: Vec<(String, [f64; 3])> = vec![
        ("known: the tutorial's source".into(), [3.0, 5.0, 1.8]),
        ("known: 5 cm, 10 cm off".into(), [3.05, 5.1, 1.8]),
        ("known: 1 mm off".into(), [3.0, 5.001, 1.8]),
        ("outside, x > 6".into(), [7.0, 5.0, 1.5]),
        ("outside, below the floor".into(), [3.0, 5.0, -0.5]),
    ];
    let facets = internal_facets(&mesh);
    assert_eq!(
        facets.len(),
        6,
        "the 6-tetrahedron box has 6 internal facets"
    );
    for &(t, n, corners) in &facets {
        for i in 1..=8 {
            for j in 1..=(9 - i) {
                let (s, u) = (f64::from(i) / 10.0, f64::from(j) / 10.0);
                points.push((format!("on {t}|{n} ({s}, {u})"), on_facet(corners, s, u)));
            }
        }
        let c = on_facet(corners, 1.0 / 3.0, 1.0 / 3.0);
        let nrm = unit_normal(corners);
        for (side, sign) in [("+", 1.0), ("-", -1.0)] {
            points.push((
                format!("near {t}|{n}, 1 mm {side}"),
                [0, 1, 2].map(|k| c[k] + sign * 1e-3 * nrm[k]),
            ));
        }
    }

    // Each point its own run folder, and SPPS on each, eight at a time (about 1 s each, the
    // crashes 2 s).
    let root = fresh_dir("agree-runs");
    let dirs: Vec<PathBuf> = points
        .iter()
        .enumerate()
        .map(|(k, (_, p))| {
            let mut pr = project.clone();
            set_source(&mut pr, *p);
            let d = root.join(format!("pt{k:03}"));
            stage(&pr, &mbin_path, &d);
            d
        })
        .collect();
    let started = std::time::Instant::now();
    let mut ran: Vec<(Option<u32>, bool)> = vec![(None, false); dirs.len()];
    for (chunk, out) in dirs.chunks(8).zip(ran.chunks_mut(8)) {
        std::thread::scope(|s| {
            let handles: Vec<_> = chunk.iter().map(|d| s.spawn(move || spps(d))).collect();
            for (h, o) in handles.into_iter().zip(out.iter_mut()) {
                *o = h.join().unwrap();
            }
        });
    }
    let secs = started.elapsed().as_secs_f64();

    let rows: Vec<Row> = points
        .iter()
        .zip(&dirs)
        .zip(&ran)
        .map(|(((what, _), dir), &(exit, end))| {
            let (_, tetrahedron) = emulated(dir, &test);
            let refused = locate::check_folder(dir)
                .iter()
                .any(|r| r.code == codes::SOURCE_UNLOCATABLE);
            let doc_text = read_text(&dir.join(names::CONFIG));
            let doc = Document::parse(&doc_text).unwrap();
            let text = locate::points(&doc)[0].text.clone();
            Row {
                what: what.clone(),
                text,
                tetrahedron,
                refused,
                exit,
                end,
            }
        })
        .collect();

    println!(
        "{:<34} {:<52} {:>6} {:>10} end",
        "point", "x, y, z", "tet", "exit"
    );
    for r in &rows {
        println!(
            "{:<34} {:<52} {:>6} {:>10} {}",
            r.what,
            r.text.join(", "),
            r.tetrahedron.map_or("none".into(), |t| t.to_string()),
            r.exit.map_or("none".into(), |e| format!("0x{e:08X}")),
            if r.end { "yes" } else { "no" }
        );
    }
    let crashed = |r: &Row| r.exit == Some(ACCESS_VIOLATION);
    let finished = |r: &Row| r.exit == Some(0) && r.end;
    let count = |f: &dyn Fn(&Row) -> bool| rows.iter().filter(|r| f(r)).count();
    let (un_crash, un_fin) = (
        count(&|r| r.tetrahedron.is_none() && crashed(r)),
        count(&|r| r.tetrahedron.is_none() && finished(r)),
    );
    let (lo_crash, lo_fin) = (
        count(&|r| r.tetrahedron.is_some() && crashed(r)),
        count(&|r| r.tetrahedron.is_some() && finished(r)),
    );
    println!(
        "{} points, SPPS on each in {secs:.1} s (8 at a time, 10 particles, 1 band)\n\
         emulation \\ SPPS   crash 0xC0000005   End of calculation\n\
         unlocated          {un_crash:>17}   {un_fin:>18}\n\
         located            {lo_crash:>17}   {lo_fin:>18}",
        rows.len()
    );

    // Every point: SPPS crashes exactly where the emulation locates the source nowhere, and
    // finishes everywhere else; the manager's check refuses exactly those.
    let bad: Vec<String> = rows
        .iter()
        .filter(|r| {
            let unlocated = r.tetrahedron.is_none();
            (unlocated && !crashed(r)) || (!unlocated && !finished(r)) || r.refused != unlocated
        })
        .map(|r| format!("{} ({})", r.what, r.text.join(", ")))
        .collect();
    assert!(bad.is_empty(), "disagreements:\n{}", bad.join("\n"));
    // Both answers occur often enough to be tested: the facet points that round out of both
    // tetrahedra, the outside points, and the rest.
    assert!(un_crash >= 10, "only {un_crash} unlocated points");
    assert!(lo_fin >= 200, "only {lo_fin} located points");
    assert_eq!(rows.len(), 5 + 6 * (36 + 2));
    assert_eq!(rows[0].tetrahedron, None);
    assert!(rows[1].tetrahedron.is_some() && rows[2].tetrahedron.is_some());
}

/// The receiver bed. The box refined to at most 0.5 m^3 per tetrahedron, one source off every
/// facet, Receiver 2 on an internal facet far from tetrahedron 0 at a point the emulation locates
/// nowhere, then the same with Receiver 2 1 mm away. SPPS finishes both runs. The receiver on the
/// facet reads exactly 0 at 1000 Hz, as nothing links it to a tetrahedron (its `indexTetra` is
/// never written, and on this heap it led nowhere near it); 1 mm away it reads a level. Receiver 1,
/// untouched, reads the same bits in both: the particles are the same (seed 1).
#[test]
fn a_receiver_the_emulation_locates_nowhere_reads_zero_in_a_finished_run() {
    let mut project = box_project();
    short_runs(&mut project, 10_000);
    project.solvers.meshing.max_volume_m3 = Some(0.5.into());
    set_source(&mut project, [3.05, 5.1, 1.8]);
    let (mesh_dir, mesh) = meshed(&project, "rcv-mesh");
    let mbin_path = mesh_dir.join(names::TETRA_MESH);
    let test = TetraTest::new(&mesh).unwrap();
    let centroid0 = {
        let v = mesh.tetrahedra[0].vertices.map(|i| node(&mesh, i));
        [0, 1, 2].map(|k| (v[0][k] + v[1][k] + v[2][k] + v[3][k]) / 4.0)
    };
    let far = |p: [f64; 3]| {
        let d = [0, 1, 2].map(|k| p[k] - centroid0[k]);
        (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt() > 2.5
            && p[0] > 0.6
            && p[0] < 5.4
            && p[1] > 0.6
            && p[1] < 9.4
            && p[2] > 0.6
            && p[2] < 2.4
    };
    // The first grid point of an internal facet, in file order, that SPPS would lose, taken as
    // the f32 SPPS stores (written as that f32's exact value).
    let on = internal_facets(&mesh)
        .into_iter()
        .flat_map(|(_, _, c)| {
            (1..=8).flat_map(move |i| {
                (1..=(9 - i)).map(move |j| on_facet(c, f64::from(i) / 10.0, f64::from(j) / 10.0))
            })
        })
        .map(|p| p.map(|x| f64::from(x as f32)))
        .find(|&p| far(p) && test.locate(p.map(|x| x as f32)).is_none())
        .expect("no unlocatable grid point far from tetrahedron 0");
    let off = [on[0], on[1] + 0.001, on[2]];
    assert!(test.locate(off.map(|x| x as f32)).is_some());

    let level = |p: [f64; 3], label: &str| {
        let mut pr = project.clone();
        pr.point_receivers[1].position = Vec3::from(p);
        let dir = fresh_dir(label);
        stage(&pr, &mbin_path, &dir);
        let refused: Vec<String> = locate::check_folder(&dir)
            .into_iter()
            .map(|r| r.code)
            .collect();
        let (exit, end) = spps(&dir);
        assert_eq!((exit, end), (Some(0), true), "{label}: SPPS did not finish");
        let read = |rcv: &str| {
            let g = gabe::read_file(&dir.join(format!(
                "{}/{rcv}/Sound level per source.recps",
                names::POINT_RECEIVER_DIR
            )))
            .unwrap();
            match &g.columns[1].data {
                ColumnData::Float { values, .. } => values[0],
                other => panic!("{other:?}"),
            }
        };
        (refused, read("Receiver 1"), read("Receiver 2"))
    };
    let (refused_on, r1_on, r2_on) = level(on, "rcv-on");
    let (refused_off, r1_off, r2_off) = level(off, "rcv-off");
    println!(
        "{} tetrahedra; Receiver 2 at {on:?}: {r2_on:e} ({:#010x}), refused {refused_on:?}; \
         1 mm off: {r2_off:e} ({:#010x}), refused {refused_off:?}; Receiver 1: {r1_on:e} and \
         {r1_off:e}",
        mesh.tetrahedra.len(),
        r2_on.to_bits(),
        r2_off.to_bits()
    );
    assert_eq!(refused_on, [codes::RECEIVER_UNLOCATABLE]);
    assert_eq!(refused_off, Vec::<String>::new());
    assert_eq!(r2_on.to_bits(), 0, "the lost receiver read {r2_on:e}");
    assert!(r2_off > 0.0, "the found receiver read {r2_off:e}");
    assert_eq!(
        r1_on.to_bits(),
        r1_off.to_bits(),
        "the control receiver moved"
    );
}
