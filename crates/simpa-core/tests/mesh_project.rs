//! `mesh::mesh_project` and `mesh::mesh_from_tetgen` on tutorial 1's box, with the real
//! `tetgen.exe` (the M1 build; see `mesh_support.rs`), and every failure code the pipeline can
//! give, each driven by an input that makes it fire.

#[path = "mesh_support.rs"]
mod support;

use std::collections::BTreeSet;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use simpa_core::formats::{cbin, mbin, poly};
use simpa_core::mesh::{
    self, MeshManifest, MeshStatus, Mesher, TetgenMesher, codes, mesh_from_tetgen, mesh_project,
    read_manifest, sha256_file,
};
use simpa_core::process::{CancelToken, Line, Outcome};
use simpa_core::schema::{
    DiffusionLaw, F64, Face, FittingShape, FittingZone, FittingZoneId, GroupId, Project,
    SurfaceGroup, Vec3,
};
use support::{face_area, fixture, invariants, load_room, scratch, tetgen_exe, volume_by_id};

fn tetgen() -> TetgenMesher {
    TetgenMesher::new(tetgen_exe())
}

fn run(p: &Project, dir: &Path) -> MeshManifest {
    mesh_project(p, dir, &tetgen(), &CancelToken::new(), &mut |_: &Line| {}).unwrap()
}

fn has(m: &MeshManifest, code: &str) -> bool {
    m.codes.iter().any(|c| c == code)
}

/// The box meshed once with its own settings, shared by the tests that only read it.
fn box_mesh() -> &'static (PathBuf, MeshManifest) {
    static BOX: OnceLock<(PathBuf, MeshManifest)> = OnceLock::new();
    BOX.get_or_init(|| {
        let dir = scratch("box");
        let m = run(&load_room("tutorial1_box.simpa"), &dir);
        (dir, m)
    })
}

/// The scene-face markers the `.mbin` carries, and how many faces carry each.
fn marker_counts(mesh: &mbin::Mesh) -> std::collections::BTreeMap<i32, usize> {
    let mut out = std::collections::BTreeMap::new();
    for t in &mesh.tetrahedra {
        for f in &t.faces {
            if f.marker >= 0 {
                *out.entry(f.marker).or_insert(0) += 1;
            }
        }
    }
    out
}

#[test]
fn the_box_meshes_with_its_own_settings() {
    let (dir, m) = box_mesh();
    assert_eq!(m.status, MeshStatus::Ok, "{m:#?}");
    assert!(m.codes.is_empty());
    let call = m.tetgen.as_ref().unwrap();
    assert_eq!(call.argv, ["-pq2", "-A", "-n", "scene_mesh.poly"]);
    assert_eq!(call.exit_code, Some(0));
    assert_eq!(
        call.program_sha256.as_deref(),
        Some(sha256_file(&tetgen_exe()).unwrap().as_str())
    );

    // The .var is upstream's tutorial-1 file, byte for byte.
    let var = std::fs::read(dir.join("scene_mesh.var")).unwrap();
    let upstream = std::fs::read(fixture("upstream/tutorial1/tetgen/scene_mesh.var")).unwrap();
    assert_eq!(var, upstream);
    assert_eq!(m.counts.var_constraints, 2);

    let mesh = mbin::read_file(&dir.join("tetramesh.mbin")).unwrap();
    assert_eq!(invariants(&mesh), Vec::<String>::new());
    assert!(mesh.tetrahedra.iter().all(|t| t.id_volume == 0));
    let markers = marker_counts(&mesh);
    assert_eq!(
        markers.keys().copied().collect::<Vec<_>>(),
        (0..12).collect::<Vec<i32>>(),
        "markers cover exactly the 12 scene faces"
    );

    // TetGen found and read the .var (see the_var_refines_the_receiver_faces for what it did).
    let log = std::fs::read_to_string(dir.join("tetgen.stdout.txt")).unwrap();
    assert!(log.contains("Opening scene_mesh.var."), "{log}");
    let (floor, largest) = floor_faces(&mesh);
    println!(
        "box: {} nodes, {} tetrahedra, {floor} floor faces, largest {largest:.6} m², TetGen \
         {:.0} ms, whole call {:.0} ms",
        mesh.nodes.len(),
        mesh.tetrahedra.len(),
        call.elapsed_ms,
        m.elapsed_ms
    );

    // The .poly holds exactly the .cbin's f32 vertices.
    let poly = poly::read_file(&dir.join("scene_mesh.poly")).unwrap();
    let scene = cbin::read_file(&dir.join("mesh.cbin")).unwrap();
    let from_cbin: Vec<[f64; 3]> = scene
        .vertices
        .iter()
        .map(|v| [v.x, v.y, v.z].map(f64::from))
        .collect();
    assert_eq!(poly.model_vertices, from_cbin);
    assert!(poly.save_face_index);
    assert!(
        poly.model_faces
            .iter()
            .enumerate()
            .all(|(i, f)| f.face_index as usize == i)
    );

    // The manifest on disk is the one returned, and its hashes are the files'.
    assert_eq!(&read_manifest(dir).unwrap(), m);
    assert_eq!(
        m.files.mbin.as_deref(),
        Some(sha256_file(&dir.join("tetramesh.mbin")).unwrap().as_str())
    );
    assert_eq!(
        m.files.cbin.as_deref(),
        Some(sha256_file(&dir.join("mesh.cbin")).unwrap().as_str())
    );
    let p = load_room("tutorial1_box.simpa");
    assert_eq!(
        m.mesh_input_hash.as_deref(),
        Some(simpa_core::validate::mesh_input_hash(&p).as_str())
    );
}

/// The tetrahedron faces on the receiver's scene faces (0 and 1, the floor): how many, and the
/// largest area.
fn floor_faces(mesh: &mbin::Mesh) -> (usize, f64) {
    let areas: Vec<f64> = mesh
        .tetrahedra
        .iter()
        .flat_map(|t| t.faces.iter())
        .filter(|f| f.marker == 0 || f.marker == 1)
        .map(|f| face_area(mesh, f))
        .collect();
    (areas.len(), areas.iter().copied().fold(0.0, f64::max))
}

/// Gate M5(a)'s refinement check as `docs/m5-m6-design.md` states it: more than 2 tetrahedron
/// faces on the receiver's faces, none above 0.1 m². It cannot pass with the pinned TetGen
/// (1.6.0 at upstream 929a5c8), which reads the `.var` but never consults a facet's area bound
/// when it decides a split (`tetgen.cxx:27347-27388`; `areabound` is read only to copy it,
/// `:12362, 12607, 12727, 16539`). See `docs/formats/var.md`, and the measured behaviour in
/// `the_pinned_tetgen_reads_the_var_and_refines_nothing`.
#[test]
#[ignore = "fails with the pinned TetGen 1.6.0, which ignores .var area bounds: docs/formats/var.md"]
fn the_var_refines_the_receiver_faces() {
    let (dir, m) = box_mesh();
    assert!(m.is_ok(), "{m:#?}");
    let mesh = mbin::read_file(&dir.join("tetramesh.mbin")).unwrap();
    let (floor, largest) = floor_faces(&mesh);
    assert!(floor > 2, "{floor} floor faces");
    assert!(
        largest <= 0.1 * (1.0 + 1e-4),
        "largest floor face {largest} m²"
    );
}

/// What the pinned TetGen does with the box's `.var` (0.1 m² on faces 0 and 1), measured: it
/// opens it and refines nothing, so the mesh is the one it makes with no `.var` at all. If this
/// fails, TetGen has changed: rerun `the_var_refines_the_receiver_faces` and revisit
/// `docs/formats/var.md`.
#[test]
fn the_pinned_tetgen_reads_the_var_and_refines_nothing() {
    let (with_dir, with) = box_mesh();
    assert!(with.is_ok(), "{with:#?}");
    let mut p = load_room("tutorial1_box.simpa");
    p.solvers.meshing.surface_receiver_max_area_m2 = None;
    let without_dir = scratch("box-novar");
    let without = run(&p, &without_dir);
    assert!(without.is_ok(), "{without:#?}");
    assert!(!without_dir.join("scene_mesh.var").exists());
    let a = mbin::read_file(&with_dir.join("tetramesh.mbin")).unwrap();
    let b = mbin::read_file(&without_dir.join("tetramesh.mbin")).unwrap();
    let (fa, la) = floor_faces(&a);
    let (fb, lb) = floor_faces(&b);
    println!(
        "box -pq2 -A -n: with the .var {} tetrahedra, {fa} floor faces, largest {la} m²; \
         without {} tetrahedra, {fb} floor faces, largest {lb} m²",
        a.tetrahedra.len(),
        b.tetrahedra.len()
    );
    assert_eq!(a, b);
    assert_eq!((fa, la), (2, 30.0));
}

fn with_box_zone(p: &mut Project, min: [f64; 3], max: [f64; 3]) {
    with_named_box_zone(p, "Zone 1", 1, min, max);
}

/// Adds an enabled box fitting zone named `name`, id number `k`.
fn with_named_box_zone(p: &mut Project, name: &str, k: u128, min: [f64; 3], max: [f64; 3]) {
    let n = p.bands.frequencies_hz.len();
    p.fitting_zones.push(FittingZone {
        id: FittingZoneId::from_u128(0x0f17_0000_0000_4000_8000_0000_0000_0000 + k),
        name: name.to_string(),
        enabled: true,
        shape: FittingShape::Box {
            min: Vec3::from(min),
            max: Vec3::from(max),
        },
        absorption: vec![F64::new(0.1); n],
        mean_free_path_m: vec![F64::new(2.0); n],
        diffusion_law: vec![DiffusionLaw::Uniform; n],
    });
}

#[test]
fn a_box_fitting_zone_is_its_own_region() {
    let mut p = load_room("tutorial1_box.simpa");
    with_box_zone(&mut p, [1.0, 1.0, 0.5], [2.0, 2.0, 1.5]);
    let dir = scratch("box-zone");
    let m = run(&p, &dir);
    assert!(m.is_ok(), "{m:#?}");
    assert_eq!(
        m.volume_ids.fittings,
        [2],
        "the first fitting zone is solver id 2"
    );
    assert_eq!(m.zone_facets.len(), 1);
    assert_eq!(m.zone_facets[0].first_marker, 12);
    assert_eq!(m.counts.poly_facets, 24);
    assert_eq!(m.counts.regions, 1);

    let mesh = mbin::read_file(&dir.join("tetramesh.mbin")).unwrap();
    assert_eq!(invariants(&mesh), Vec::<String>::new());
    let volumes = volume_by_id(&mesh);
    println!("box with a zone: volume per idVolume {volumes:?}");
    assert_eq!(volumes.keys().copied().collect::<Vec<_>>(), [0, 2]);
    let zone = volumes[&2];
    assert!(((zone - 1.0) / 1.0).abs() <= 1e-9, "zone volume {zone} m³");
    assert!(((volumes[&0] + zone - 180.0) / 180.0).abs() <= 1e-9);
    // The zone's triangles (markers 12..24) are plain tet-to-tet transitions.
    assert!(
        mesh.tetrahedra
            .iter()
            .flat_map(|t| t.faces.iter())
            .all(|f| f.marker < 12)
    );
    let stats = m.counts.build.as_ref().unwrap();
    assert!(stats.zone_tet_faces >= 24, "{stats:?}");
    // TetGen gave the unseeded room the next attribute, 3; the builder wrote it as 0.
    let attrs: Vec<(i64, i32)> = stats
        .attributes
        .iter()
        .map(|a| (a.attribute, a.id_volume))
        .collect();
    assert_eq!(attrs, [(2, 2), (3, 0)]);
}

#[test]
fn stale_files_are_deleted_before_meshing() {
    let dir = scratch("stale");
    // A leftover .var that would refine every face, and a junk .ele.
    let stale_var = mesh::var_bytes(&(0..12).collect::<Vec<u32>>(), 0.05);
    std::fs::write(dir.join("scene_mesh.var"), &stale_var).unwrap();
    std::fs::write(dir.join("scene_mesh.1.ele"), "junk").unwrap();
    std::fs::write(dir.join("tetramesh.mbin"), "junk").unwrap();
    std::fs::create_dir_all(dir.join("diag")).unwrap();
    std::fs::write(dir.join("diag/scene_mesh.poly"), "junk").unwrap();
    std::fs::write(dir.join("notes.txt"), "kept").unwrap();
    let mut p = load_room("tutorial1_box.simpa");
    p.solvers.meshing.surface_receiver_max_area_m2 = None;
    let m = run(&p, &dir);
    assert!(m.is_ok(), "{m:#?}");
    assert!(
        !dir.join("scene_mesh.var").exists(),
        "the stale .var is gone"
    );
    assert!(!dir.join("diag").exists());
    assert_ne!(
        std::fs::read(dir.join("scene_mesh.1.ele")).unwrap(),
        b"junk"
    );
    assert_eq!(std::fs::read(dir.join("notes.txt")).unwrap(), b"kept");
    let fresh = mbin::read_file(&dir.join("tetramesh.mbin")).unwrap();

    assert!(!fresh.tetrahedra.is_empty());
    let log = std::fs::read_to_string(dir.join("tetgen.stdout.txt")).unwrap();
    assert!(!log.contains("scene_mesh.var"), "{log}");

    // Why it matters: TetGen opens a .var it finds beside the .poly (tetgen.cxx:2446-2449).
    // The same .poly with the stale .var left in place, run as the mesher runs it:
    let control = scratch("stale-control");
    std::fs::copy(dir.join("scene_mesh.poly"), control.join("scene_mesh.poly")).unwrap();
    std::fs::write(control.join("scene_mesh.var"), &stale_var).unwrap();
    let out = std::process::Command::new(tetgen_exe())
        .args(["-pq2", "-A", "-n", "scene_mesh.poly"])
        .current_dir(&control)
        .output()
        .unwrap();
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("Opening scene_mesh.var."), "{stdout}");
}

#[test]
fn a_mesh_rebuilt_from_its_tetgen_output_is_identical() {
    let (a, m) = box_mesh();
    assert!(m.is_ok(), "{m:#?}");
    let p = load_room("tutorial1_box.simpa");
    let original = std::fs::read(a.join("tetramesh.mbin")).unwrap();

    // Same basename, another folder, rebuilt in place.
    let b = scratch("external");
    for ext in ["node", "ele", "face", "neigh", "edge"] {
        let name = format!("scene_mesh.1.{ext}");
        std::fs::copy(a.join(&name), b.join(&name)).unwrap();
    }
    let e = mesh_from_tetgen(&p, &b, None, &b).unwrap();
    assert!(e.is_ok(), "{e:#?}");
    assert_eq!(e.source, mesh::MeshSource::External);
    assert_eq!(std::fs::read(b.join("tetramesh.mbin")).unwrap(), original);
    let call = e.tetgen.as_ref().unwrap();
    assert_eq!(call.argv, ["-pq2", "-A", "-n", "scene_mesh.poly"]);
    assert!(call.program.as_deref().unwrap().ends_with("tetgen.exe"));
    assert_eq!(e.mesh_input_hash, m.mesh_input_hash);
    assert_eq!(read_manifest(&b).unwrap(), e);

    // Any basename, and a separate output folder.
    let c = scratch("external-model");
    for ext in ["node", "ele", "face", "neigh"] {
        std::fs::copy(
            a.join(format!("scene_mesh.1.{ext}")),
            c.join(format!("model.1.{ext}")),
        )
        .unwrap();
    }
    let out = c.join("out");
    let e = mesh_from_tetgen(&p, &c, None, &out).unwrap();
    assert!(e.is_ok(), "{e:#?}");
    assert_eq!(std::fs::read(out.join("tetramesh.mbin")).unwrap(), original);

    // No .neigh: refused, never recomputed.
    std::fs::remove_file(c.join("model.1.neigh")).unwrap();
    let e = mesh_from_tetgen(&p, &c, None, &out).unwrap();
    assert_eq!(e.codes, [codes::NEIGH_MISSING]);
    assert!(!out.join("tetramesh.mbin").exists());

    // No TetGen output at all: every missing file has its code.
    let empty = scratch("external-empty");
    let e = mesh_from_tetgen(&p, &empty, None, &empty).unwrap();
    assert_eq!(
        e.codes,
        [codes::TETGEN_OUTPUT_MISSING, codes::NEIGH_MISSING]
    );
    assert!(!empty.join("tetramesh.mbin").exists());
    assert_eq!(read_manifest(&empty).unwrap(), e);

    // Two output sets and no basename: which one is meant is not guessed.
    let two = scratch("external-two");
    for base in ["model", "scene_mesh"] {
        for ext in ["node", "ele", "face", "neigh"] {
            std::fs::copy(
                a.join(format!("scene_mesh.1.{ext}")),
                two.join(format!("{base}.1.{ext}")),
            )
            .unwrap();
        }
    }
    let e = mesh_from_tetgen(&p, &two, None, &two.join("out")).unwrap();
    assert_eq!(e.codes, [codes::INPUT_INVALID], "{e:#?}");
    let e = mesh_from_tetgen(&p, &two, Some("model"), &two.join("out")).unwrap();
    assert!(e.is_ok(), "{e:#?}");
    assert_eq!(
        std::fs::read(two.join("out/tetramesh.mbin")).unwrap(),
        original
    );
}

/// A mesher that runs nothing: it calls `act` on the folder and returns `outcome`.
struct Fake<F: Fn(&Path)> {
    act: F,
    outcome: Outcome,
}

impl<F: Fn(&Path)> Mesher for Fake<F> {
    fn program(&self) -> Option<&Path> {
        None
    }

    fn run(
        &self,
        dir: &Path,
        _args: &[String],
        _cancel: &CancelToken,
        _on_line: &mut dyn FnMut(&Line),
    ) -> io::Result<Outcome> {
        (self.act)(dir);
        Ok(self.outcome.clone())
    }
}

fn exited(code: u32) -> Outcome {
    Outcome {
        exit_code: Some(code),
        cancelled: false,
        elapsed_ms: 1.0,
    }
}

/// Copies the real box mesh's TetGen output into `dir`, less the extensions in `skip`.
fn copy_box_output(dir: &Path, skip: &[&str]) {
    let (from, m) = box_mesh();
    assert!(m.is_ok(), "{m:#?}");
    for ext in ["node", "ele", "face", "neigh"] {
        if !skip.contains(&ext) {
            let name = format!("scene_mesh.1.{ext}");
            std::fs::copy(from.join(&name), dir.join(&name)).unwrap();
        }
    }
}

fn fake_run(label: &str, p: &Project, mesher: &dyn Mesher) -> (PathBuf, MeshManifest) {
    let dir = scratch(label);
    let m = mesh_project(p, &dir, mesher, &CancelToken::new(), &mut |_: &Line| {}).unwrap();
    assert!(
        !dir.join("tetramesh.mbin").exists(),
        "{label}: a failed mesh leaves no .mbin"
    );
    assert_eq!(
        read_manifest(&dir).unwrap(),
        m,
        "{label}: the manifest is written"
    );
    (dir, m)
}

#[test]
// Clearing the read-only attribute is the point on Windows.
#[allow(clippy::permissions_set_readonly_false)]
fn every_failure_code_fires_on_its_input() {
    let p = load_room("tutorial1_box.simpa");
    let set = |m: &MeshManifest| m.codes.iter().cloned().collect::<BTreeSet<String>>();
    let expect = |xs: &[&str]| {
        xs.iter()
            .map(|s| s.to_string())
            .collect::<BTreeSet<String>>()
    };

    // The fake reproduces the real run exactly: the pipeline accepts it.
    let fake = Fake {
        act: |d: &Path| copy_box_output(d, &[]),
        outcome: exited(0),
    };
    let dir = scratch("fake-ok");
    let m = mesh_project(&p, &dir, &fake, &CancelToken::new(), &mut |_: &Line| {}).unwrap();
    assert!(m.is_ok(), "{m:#?}");

    // An access violation: crash and nonzero, and whatever files are missing.
    let crash = Fake {
        act: |_: &Path| {},
        outcome: exited(0xC000_0005),
    };
    let (_, m) = fake_run("fake-crash", &p, &crash);
    assert_eq!(
        set(&m),
        expect(&[
            codes::TETGEN_CRASH,
            codes::TETGEN_EXIT_NONZERO,
            codes::TETGEN_OUTPUT_MISSING,
            codes::NEIGH_MISSING
        ])
    );
    assert!(m.messages.iter().any(|s| s.contains("0xC0000005")));

    // Exit 0 and no files.
    let silent = Fake {
        act: |_: &Path| {},
        outcome: exited(0),
    };
    let (_, m) = fake_run("fake-silent", &p, &silent);
    assert_eq!(
        set(&m),
        expect(&[codes::TETGEN_OUTPUT_MISSING, codes::NEIGH_MISSING])
    );

    // Everything but the .neigh: no fallback.
    let no_neigh = Fake {
        act: |d: &Path| copy_box_output(d, &["neigh"]),
        outcome: exited(0),
    };
    let (_, m) = fake_run("fake-noneigh", &p, &no_neigh);
    assert_eq!(m.codes, [codes::NEIGH_MISSING]);

    // A .neigh with a row missing: unreadable, so invalid.
    let short_neigh = Fake {
        act: |d: &Path| {
            copy_box_output(d, &[]);
            let path = d.join("scene_mesh.1.neigh");
            let text = std::fs::read_to_string(&path).unwrap();
            let mut lines: Vec<&str> = text.lines().collect();
            let last_data = lines.iter().rposition(|l| !l.starts_with('#')).unwrap();
            lines.remove(last_data);
            std::fs::write(&path, lines.join("\n")).unwrap();
        },
        outcome: exited(0),
    };
    let (_, m) = fake_run("fake-shortneigh", &p, &short_neigh);
    assert_eq!(m.codes, [codes::TETGEN_OUTPUT_INVALID], "{m:#?}");

    // A program that is not there.
    let missing = TetgenMesher::new(support::scratch("no-exe").join("tetgen.exe"));
    let (_, m) = fake_run("fake-noexe", &p, &missing);
    assert_eq!(m.codes, [codes::TETGEN_LAUNCH_FAILED]);

    // Settings that conflict: refused before anything runs.
    let never = Fake {
        act: |_: &Path| panic!("the mesher must not run"),
        outcome: exited(0),
    };
    let mut conflict = p.clone();
    conflict.solvers.meshing.preserve_boundary = true;
    let (dir, m) = fake_run("fake-conflict", &conflict, &never);
    assert_eq!(m.codes, [codes::MESH_SETTINGS_CONFLICT]);
    assert!(!dir.join("scene_mesh.poly").exists());

    // A box zone with no volume.
    let mut flat = p.clone();
    with_box_zone(&mut flat, [1.0, 1.0, 0.5], [2.0, 1.0, 1.5]);
    let (_, m) = fake_run("fake-flatzone", &flat, &never);
    assert_eq!(m.codes, [codes::INPUT_INVALID]);

    // A .poly that cannot be written (a folder stands where it goes): nothing runs, and the
    // manifest says why.
    let dir = scratch("fake-polydir");
    std::fs::create_dir(dir.join("scene_mesh.poly")).unwrap();
    let m = mesh_project(&p, &dir, &never, &CancelToken::new(), &mut |_: &Line| {}).unwrap();
    assert_eq!(m.codes, [codes::INPUT_WRITE_FAILED], "{m:#?}");
    assert_eq!(read_manifest(&dir).unwrap(), m);

    // A stale file that will not be deleted (read-only): nothing is meshed over it.
    let dir = scratch("fake-readonly");
    let stale = dir.join("scene_mesh.var");
    std::fs::write(&stale, "stale").unwrap();
    let mut perms = std::fs::metadata(&stale).unwrap().permissions();
    perms.set_readonly(true);
    std::fs::set_permissions(&stale, perms.clone()).unwrap();
    let m = mesh_project(&p, &dir, &never, &CancelToken::new(), &mut |_: &Line| {}).unwrap();
    assert_eq!(m.codes, [codes::STALE_DELETE_FAILED], "{m:#?}");
    assert_eq!(read_manifest(&dir).unwrap(), m);
    assert!(!dir.join("scene_mesh.poly").exists());
    perms.set_readonly(false);
    std::fs::set_permissions(&stale, perms).unwrap();

    // A _skipped.face that does not read: the skip cannot be mapped, so the output is invalid.
    let garbled = Fake {
        act: |d: &Path| {
            copy_box_output(d, &[]);
            std::fs::write(d.join("scene_mesh_skipped.face"), "not a face file\n").unwrap();
        },
        outcome: exited(0),
    };
    let (_, m) = fake_run("fake-garbledskip", &p, &garbled);
    assert_eq!(m.codes, [codes::TETGEN_OUTPUT_INVALID], "{m:#?}");
    assert!(
        m.messages
            .iter()
            .any(|s| s.contains("scene_mesh_skipped.face"))
    );

    // Skipped facets, and a file named diag where the follow-up's folder goes: the failure is
    // reported as it is, and the follow-up that could not be set up is a message.
    let skips = Fake {
        act: |d: &Path| {
            std::fs::write(
                d.join("scene_mesh_skipped.face"),
                "2 1\n1 1 2 3 8\n2 1 3 4 9\n",
            )
            .unwrap();
        },
        outcome: exited(3),
    };
    let dir = scratch("fake-diagfile");
    std::fs::write(dir.join("diag"), "a file, not a folder").unwrap();
    let m = mesh_project(&p, &dir, &skips, &CancelToken::new(), &mut |_: &Line| {}).unwrap();
    assert_eq!(
        set(&m),
        expect(&[
            codes::TETGEN_EXIT_NONZERO,
            codes::TETGEN_SKIPPED_FACETS,
            codes::TETGEN_OUTPUT_MISSING,
            codes::NEIGH_MISSING
        ])
    );
    let skipped: Vec<(i64, Option<String>)> = m
        .skipped_facets
        .iter()
        .map(|s| (s.marker, s.group.clone()))
        .collect();
    assert_eq!(
        skipped,
        [
            (8, Some("Walls".to_string())),
            (9, Some("Walls".to_string()))
        ]
    );
    assert_eq!(m.diagnosis, None);
    assert!(
        m.messages
            .iter()
            .any(|s| s.starts_with("the tetgen -d follow-up could not be set up"))
    );
    assert_eq!(read_manifest(&dir).unwrap(), m);

    // Cancelled before TetGen starts.
    let dir = scratch("fake-precancel");
    let cancel = CancelToken::new();
    cancel.cancel();
    let m = mesh_project(&p, &dir, &never, &cancel, &mut |_: &Line| {}).unwrap();
    assert_eq!(m.status, MeshStatus::Cancelled);
    assert!(has(&m, codes::CANCELLED));
    assert!(!dir.join("tetramesh.mbin").exists());

    // Cancelled while TetGen runs: the outcome says so.
    let killed = Fake {
        act: |d: &Path| copy_box_output(d, &[]),
        outcome: Outcome {
            exit_code: None,
            cancelled: true,
            elapsed_ms: 1.0,
        },
    };
    let (_, m) = fake_run("fake-killed", &p, &killed);
    assert_eq!(m.status, MeshStatus::Cancelled);
    assert_eq!(m.codes, [codes::CANCELLED]);
}

#[test]
fn invariant_checker_says_no() {
    // The test-side checker used above must be able to fail.
    let (dir, m) = box_mesh();
    assert!(m.is_ok(), "{m:#?}");
    let good = mbin::read_file(&dir.join("tetramesh.mbin")).unwrap();
    let mut bad = good.clone();
    let t = bad
        .tetrahedra
        .iter()
        .position(|t| t.faces.iter().any(|f| f.neighbor >= 0))
        .unwrap();
    let k = bad.tetrahedra[t]
        .faces
        .iter()
        .position(|f| f.neighbor >= 0)
        .unwrap();
    bad.tetrahedra[t].faces[k].neighbor = -2;
    assert!(!invariants(&bad).is_empty());
    // TetGen's own hull value, -1, left in place of -2.
    let mut raw_hull = good.clone();
    let t = raw_hull
        .tetrahedra
        .iter()
        .position(|t| t.faces.iter().any(|f| f.neighbor == -2))
        .unwrap();
    for f in &mut raw_hull.tetrahedra[t].faces {
        if f.neighbor == -2 {
            f.neighbor = -1;
        }
    }
    assert!(
        invariants(&raw_hull)
            .iter()
            .any(|s| s.starts_with("neighbour value")),
        "{:?}",
        invariants(&raw_hull)
    );
    let mut flipped = good.clone();
    flipped.tetrahedra[0].vertices.swap(0, 1);
    assert!(
        invariants(&flipped)
            .iter()
            .any(|s| s.starts_with("winding"))
    );
}

/// The box's TetGen output with the last `.face` row dropped: one hull triangle loses its
/// marker, so the `.mbin` built from it has an unmarked boundary face.
fn copy_box_output_less_one_face_row(dir: &Path) {
    copy_box_output(dir, &[]);
    let path = dir.join("scene_mesh.1.face");
    let text = std::fs::read_to_string(&path).unwrap();
    let mut lines: Vec<String> = text.lines().map(str::to_string).collect();
    let mut header = lines[0].split_whitespace();
    let count: usize = header.next().unwrap().parse().unwrap();
    let flag = header.next().unwrap().to_string();
    lines[0] = format!("{}  {flag}", count - 1);
    let last = lines.iter().rposition(|l| !l.starts_with('#')).unwrap();
    lines.remove(last);
    std::fs::write(&path, lines.join("\n") + "\n").unwrap();
}

#[test]
fn a_face_row_short_gives_a_mesh_the_invariants_refuse() {
    // The input of the next test is bad by the test-side checker: build_mbin accepts it, and
    // the mesh it builds has an unmarked hull face.
    let dir = scratch("unmarked-hull");
    copy_box_output_less_one_face_row(&dir);
    let out = mesh::TetgenOutput::read(&mesh::OutputPaths::new(&dir, "scene_mesh")).unwrap();
    let (built, _) = mesh::build_mbin(&out, 12, &[]).unwrap();
    let bad = invariants(&built);
    assert!(
        bad.iter().any(|s| s.starts_with("unmarked hull")),
        "{bad:?}"
    );
}

/// Decision 5 of the pipeline: a mesh the real `verify_mesh` refuses is never written. The input
/// is the box's TetGen output with its last `.face` row dropped, so one hull face of the `.mbin`
/// built from it carries no marker. It fails when the verifier passes that mesh (a pass-all
/// verifier gives no `mesh_invalid`), or when the pipeline writes the `.mbin` anyway.
#[test]
fn a_mesh_that_fails_verification_is_not_written() {
    let p = load_room("tutorial1_box.simpa");
    let fake = Fake {
        act: |d: &Path| copy_box_output_less_one_face_row(d),
        outcome: exited(0),
    };
    // `fake_run` also asserts that no `tetramesh.mbin` is in the folder. The box's 6-tetrahedron
    // mesh carries each scene face on exactly one tetrahedron face, so the face that lost its
    // row also leaves its scene face uncovered.
    let (_, m) = fake_run("fake-unmarked", &p, &fake);
    assert_eq!(
        m.codes,
        [
            codes::MESH_INVALID,
            "unmarked_boundary_faces",
            "uncovered_scene_faces"
        ],
        "{m:#?}"
    );
    assert_eq!(m.status, MeshStatus::Fail);
    assert_eq!(m.files.mbin, None);
    assert!(m.verify.as_ref().is_some_and(|r| !r.passed()));
}

/// Adds a surface group named `name` (the walls' material) and returns its id.
fn add_group(p: &mut Project, name: &str, id: u128) -> GroupId {
    let gid = GroupId::from_u128(id);
    let material = p.surface_groups[0].material;
    p.surface_groups.push(SurfaceGroup {
        id: gid,
        name: name.to_string(),
        material,
    });
    gid
}

/// Adds the closed box `min`-`max` to the scene as 12 faces of group `gid`, wound outward.
fn add_box_faces(p: &mut Project, gid: GroupId, min: [f64; 3], max: [f64; 3]) {
    let base = p.geometry.vertices.len() as u32;
    for k in 0..8usize {
        let at_max = [matches!(k & 3, 1 | 2), matches!(k & 3, 2 | 3), k >= 4];
        p.geometry.vertices.push(Vec3::from(
            [0, 1, 2].map(|a| if at_max[a] { max[a] } else { min[a] }),
        ));
    }
    for t in [
        [0, 2, 1],
        [0, 3, 2],
        [4, 5, 6],
        [4, 6, 7],
        [0, 1, 5],
        [0, 5, 4],
        [1, 2, 6],
        [1, 6, 5],
        [2, 3, 7],
        [2, 7, 6],
        [3, 0, 4],
        [3, 4, 7],
    ] {
        p.geometry.faces.push(Face {
            vertices: t.map(|c: u32| base + c),
            group: gid,
        });
    }
}

/// Decision 6 end to end: a `Surfaces` fitting zone whose surfaces are scene faces inside the
/// room. Those faces are internal facets, so each of their triangles is marked on both sides.
#[test]
fn a_surfaces_zone_marks_its_internal_facets_on_both_sides() {
    let mut p = load_room("tutorial1_box.simpa");
    let gid = add_group(
        &mut p,
        "Fittings",
        0x0f17_0000_0000_4000_8000_0000_0000_0002,
    );
    add_box_faces(&mut p, gid, [1.0, 1.0, 0.5], [2.0, 2.0, 1.5]);
    let n = p.bands.frequencies_hz.len();
    p.fitting_zones.push(FittingZone {
        id: FittingZoneId::from_u128(0x0f17_0000_0000_4000_8000_0000_0000_0003),
        name: "Shelf".to_string(),
        enabled: true,
        shape: FittingShape::Surfaces {
            groups: vec![gid],
            inside_point: Vec3::from([1.5, 1.5, 1.0]),
        },
        absorption: vec![F64::new(0.1); n],
        mean_free_path_m: vec![F64::new(2.0); n],
        diffusion_law: vec![DiffusionLaw::Uniform; n],
    });
    let dir = scratch("surfaces-zone");
    let m = run(&p, &dir);
    assert!(m.is_ok(), "{m:#?}");
    assert_eq!(m.volume_ids.fittings, [2]);
    assert!(m.zone_facets.is_empty(), "a Surfaces zone adds no facets");
    assert_eq!((m.counts.scene_faces, m.counts.poly_facets), (24, 24));

    let mesh = mbin::read_file(&dir.join("tetramesh.mbin")).unwrap();
    assert_eq!(invariants(&mesh), Vec::<String>::new());
    let volumes = volume_by_id(&mesh);
    println!("Surfaces zone: volume per idVolume {volumes:?}");
    assert_eq!(volumes.keys().copied().collect::<Vec<_>>(), [0, 2]);
    assert!((volumes[&2] - 1.0).abs() <= 1e-9, "{volumes:?}");
    assert!((volumes[&0] - 179.0).abs() / 179.0 <= 1e-9, "{volumes:?}");

    // Per internal marker (12..24): as many .face rows as triangles, each on two tetrahedron
    // faces, and every one of those faces has a neighbour.
    let face = match simpa_core::formats::tetgen::read_file(&dir.join("scene_mesh.1.face")) {
        Ok(simpa_core::formats::tetgen::TetgenFile::Face(f)) => f,
        other => panic!("{other:?}"),
    };
    let mut rows = std::collections::BTreeMap::<i32, usize>::new();
    for &k in face.markers.as_ref().unwrap() {
        *rows.entry(k).or_insert(0) += 1;
    }
    let on_tets = marker_counts(&mesh);
    assert_eq!(
        on_tets.keys().copied().collect::<Vec<_>>(),
        (0..24).collect::<Vec<i32>>()
    );
    for k in 12..24 {
        assert_eq!(on_tets[&k], 2 * rows[&k], "marker {k}: both sides");
    }
    for k in 0..12 {
        assert_eq!(on_tets[&k], rows[&k], "marker {k}: hull, one side");
    }
    let internal_on_hull = mesh
        .tetrahedra
        .iter()
        .flat_map(|t| t.faces.iter())
        .filter(|f| f.marker >= 12 && f.neighbor < 0)
        .count();
    assert_eq!(internal_on_hull, 0);
    let stats = m.counts.build.as_ref().unwrap();
    let internal_rows: usize = (12..24).map(|k| rows[&k]).sum();
    assert_eq!(
        stats.marked_tet_faces,
        stats.hull_tet_faces + 2 * internal_rows
    );
}

/// Skipped facets of a project are named by scene face and surface group, and a box fitting
/// zone's triangles by the zone.
#[test]
fn skipped_facets_are_named_by_group_and_fitting_zone() {
    // A baffle (its own group) that pierces the x = 6 wall inside wall face 9: the segment it
    // cuts runs (6, 3, 1.25)-(6, 3, 1.75), above that wall's diagonal (z = 0.3 y).
    let mut p = load_room("tutorial1_box.simpa");
    let gid = add_group(&mut p, "Baffle", 0x0f17_0000_0000_4000_8000_0000_0000_0004);
    let v = p.geometry.vertices.len() as u32;
    for c in [[5.0, 3.0, 1.5], [7.0, 3.0, 1.0], [7.0, 3.0, 2.0]] {
        p.geometry.vertices.push(Vec3::from(c));
    }
    p.geometry.faces.push(Face {
        vertices: [v, v + 1, v + 2],
        group: gid,
    });
    let m = run(&p, &scratch("skipped-baffle"));
    println!(
        "baffle: codes {:?}, skipped {:?}, diagnosis pairs {:?}",
        m.codes,
        m.skipped_facets,
        m.diagnosis.as_ref().map(|d| &d.intersections)
    );
    assert!(has(&m, codes::TETGEN_SKIPPED_FACETS), "{m:#?}");
    // Measured: TetGen skips the pierced wall face, not the baffle; the -d follow-up names the
    // baffle's edge as what pierces it.
    let named: Vec<(i64, Option<u32>, Option<&str>)> = m
        .skipped_facets
        .iter()
        .map(|s| (s.marker, s.scene_face, s.group.as_deref()))
        .collect();
    assert_eq!(named, [(9, Some(9), Some("Walls"))]);
    assert!(m.skipped_facets.iter().all(|s| s.fitting_zone.is_none()));
    let d = m.diagnosis.as_ref().expect("a diagnosis");
    assert!(
        d.intersections.iter().any(|i| {
            i.first.markers == [12] && i.second.as_ref().is_some_and(|s| s.markers == [9])
        }),
        "{d:#?}"
    );

    // Two box zones that overlap: TetGen skips zone triangles (markers past the 12 scene
    // faces), which are named by their zone and carry no scene face or group.
    let mut p = load_room("tutorial1_box.simpa");
    with_named_box_zone(&mut p, "Zone 1", 1, [1.0, 1.0, 0.5], [2.0, 2.0, 1.5]);
    with_named_box_zone(&mut p, "Zone 2", 2, [1.5, 1.5, 1.0], [2.5, 2.5, 2.0]);
    let m = run(&p, &scratch("skipped-zones"));
    println!(
        "overlapping zones: codes {:?}, zone facets {:?}, skipped {:?}",
        m.codes, m.zone_facets, m.skipped_facets
    );
    assert!(has(&m, codes::TETGEN_SKIPPED_FACETS), "{m:#?}");
    assert!(!m.skipped_facets.is_empty());
    for s in &m.skipped_facets {
        let zone = match s.marker {
            12..24 => "Zone 1",
            24..36 => "Zone 2",
            _ => panic!("{s:?} is not a zone triangle"),
        };
        assert_eq!((s.scene_face, s.group.as_deref()), (None, None), "{s:?}");
        assert_eq!(s.fitting_zone.as_deref(), Some(zone), "{s:?}");
    }
}
