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
    // The room as TetGen numbers it, 1 without fitting zones, written unchanged (decision 1).
    assert!(mesh.tetrahedra.iter().all(|t| t.id_volume == 1));
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

/// Whether a mesh passes gate M5(a)'s refinement check as `docs/m5-m6-design.md` states it: more
/// than 2 tetrahedron faces on the receiver's faces, none above 0.1 m² × (1 + 1e-4).
fn floor_refined(mesh: &mbin::Mesh) -> bool {
    let (floor, largest) = floor_faces(mesh);
    floor > 2 && largest <= 0.1 * (1.0 + 1e-4)
}

/// Gate M5(a)'s refinement check on the box meshed with its own settings: TetGen 1.5.0, the mesher
/// since decision 3, honours the `.var` (0.1 m² on faces 0 and 1): its `checkfac4split` splits a
/// subface whose area is above the facet's bound (`third_party/tetgen-1.5.0/tetgen.cxx`, 24580 ff.).
/// The mesh is upstream's own 2019 tutorial mesh: 732 nodes, 2,257 tetrahedra and 934 floor faces
/// of at most 0.0998 m² (`docs/investigations/2026-09-23-upstream-meshing/synthesise.md` §4). The
/// input that makes it say no is the next test's: the same box without its `.var`.
#[test]
fn the_var_refines_the_receiver_faces() {
    let (dir, m) = box_mesh();
    assert!(m.is_ok(), "{m:#?}");
    let mesh = mbin::read_file(&dir.join("tetramesh.mbin")).unwrap();
    let (floor, largest) = floor_faces(&mesh);
    println!(
        "box with its .var: {} nodes, {} tetrahedra, {floor} floor faces, largest {largest} m²",
        mesh.nodes.len(),
        mesh.tetrahedra.len()
    );
    assert!(
        floor_refined(&mesh),
        "{floor} floor faces, largest {largest} m²"
    );
    assert_eq!(
        (mesh.nodes.len(), mesh.tetrahedra.len(), floor),
        (732, 2257, 934)
    );
    assert!(largest <= 0.0998, "largest floor face {largest} m²");
}

/// What the `.var` does, measured against the same box without it: TetGen 1.5.0 opens it and
/// refines the floor to the bound, and without it the floor stays coarse, so the refinement check
/// refuses that mesh. With the pinned TetGen 1.6.0 both meshes were the same 6 tetrahedra, with
/// the floor 2 faces of 30 m² (decision 3); this test fails on that TetGen.
#[test]
fn without_the_var_the_floor_is_not_refined() {
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
    assert_ne!(a, b);
    assert!(floor_refined(&a));
    assert!(
        !floor_refined(&b),
        "without the .var the floor is refined anyway"
    );
    assert!(lb > 0.1, "largest floor face without the .var {lb} m²");
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
            destination: None,
        },
        absorption: vec![F64::new(0.1); n],
        mean_free_path_m: vec![F64::new(2.0); n],
        diffusion_law: vec![DiffusionLaw::Uniform; n],
        solver_id: None,
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
    assert_eq!(volumes.keys().copied().collect::<Vec<_>>(), [2, 3]);
    let zone = volumes[&2];
    assert!(((zone - 1.0) / 1.0).abs() <= 1e-9, "zone volume {zone} m³");
    assert!(((volumes[&3] + zone - 180.0) / 180.0).abs() <= 1e-9);
    // The zone's triangles (markers 12..24) are plain tet-to-tet transitions.
    assert!(
        mesh.tetrahedra
            .iter()
            .flat_map(|t| t.faces.iter())
            .all(|f| f.marker < 12)
    );
    let stats = m.counts.build.as_ref().unwrap();
    assert!(stats.zone_tet_faces >= 24, "{stats:?}");
    // TetGen gave the unseeded room the next attribute, 3, and the builder wrote it unchanged, as
    // upstream does; the verifier took the room from 3 (VolumeIds::tetgen).
    let attrs: Vec<(i64, i32)> = stats
        .attributes
        .iter()
        .map(|a| (a.attribute, a.id_volume))
        .collect();
    assert_eq!(attrs, [(2, 2), (3, 3)]);
    assert_eq!(m.volume_ids.room, 3);
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
    // With no model.poly beside the output, the regions are held to the project's own .poly:
    // never skipped.
    assert_eq!(e.geometry.as_ref().unwrap().checked, "project");
    let v = e.verify.as_ref().unwrap();
    assert!(v.regions_checked, "{v:#?}");
    assert_eq!(v.regions.len(), 1);
    let cell = v.regions[0].cell_volume_m3.unwrap();
    assert!((cell - 180.0).abs() < 1e-9, "{v:#?}");
    // Says NO: every other tetrahedron given a second region, 2, in the room's one cell.
    let d = scratch("external-split");
    for ext in ["node", "face", "neigh"] {
        std::fs::copy(
            c.join(format!("model.1.{ext}")),
            d.join(format!("model.1.{ext}")),
        )
        .unwrap();
    }
    let ele = std::fs::read_to_string(c.join("model.1.ele")).unwrap();
    let split: String = ele
        .lines()
        .enumerate()
        .map(|(i, l)| {
            let mut cols: Vec<&str> = l.split_whitespace().collect();
            if i > 0 && i % 2 == 0 && cols.len() == 6 && !l.starts_with('#') {
                cols[5] = "2";
                format!("{}\n", cols.join(" "))
            } else {
                format!("{l}\n")
            }
        })
        .collect();
    std::fs::write(d.join("model.1.ele"), split).unwrap();
    let e = mesh_from_tetgen(&p, &d, None, &d.join("out")).unwrap();
    assert_eq!(
        e.codes,
        [codes::MESH_INVALID, "region_volume_mismatch"],
        "{:#?}",
        e.verify.as_ref().map(|v| &v.regions)
    );
    assert!(!d.join("out/tetramesh.mbin").exists());
    // Says NO: a project whose own .poly the geometry check refuses (a face gone, so the room
    // is open) cannot stand in for the missing one: geometry_refused, nothing built.
    let mut open = p.clone();
    open.geometry.faces.pop();
    let e = mesh_from_tetgen(&open, &c, None, &c.join("out-open")).unwrap();
    assert_eq!(e.codes, [codes::GEOMETRY_REFUSED], "{e:#?}");
    assert!(!c.join("out-open/tetramesh.mbin").exists());

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

/// A mesher that answers from a script: the main call prints `main` on stdout and exits
/// `main_exit`; the `-d` follow-up (in `diag/`) writes `diag_face` as `scene_mesh.1.face` when
/// given, prints `diag` and exits 0, as TetGen 1.5.0's does.
struct Scripted {
    main: &'static str,
    main_exit: u32,
    diag: &'static str,
    diag_face: Option<&'static str>,
}

impl Mesher for Scripted {
    fn program(&self) -> Option<&Path> {
        None
    }

    fn run(
        &self,
        dir: &Path,
        args: &[String],
        _cancel: &CancelToken,
        on_line: &mut dyn FnMut(&Line),
    ) -> io::Result<Outcome> {
        let follow_up = args.first().is_some_and(|a| a == "-d");
        let (text, exit) = if follow_up {
            if let Some(face) = self.diag_face {
                std::fs::write(dir.join("scene_mesh.1.face"), face)?;
            }
            (self.diag, 0)
        } else {
            (self.main, self.main_exit)
        };
        for l in text.lines() {
            on_line(&Line {
                stream: simpa_core::process::Stream::Stdout,
                t_ms: 0.0,
                text: l.to_string(),
                terminated: true,
            });
        }
        Ok(exited(exit))
    }
}

/// TetGen 1.5.0's stop, read from its stdout and exit code by the mesher: `tetgen_self_intersection`
/// fires exactly on exit 3 with 1.5.0's `A self-intersection was detected. Program stopped.`, and
/// the facets named are mapped to scene faces and groups. The inputs that make it say no: the
/// same stdout with exit 0, and exit 3 with TetGen 1.6.0's own stop line.
#[test]
fn a_self_intersection_stop_is_read_from_tetgen_1_5s_stdout() {
    let p = load_room("tutorial1_box.simpa");
    // Facet #10 is the box's scene face 9 (a wall), facet #2 its face 1 (the floor).
    const STOP: &str = "Recovering boundaries...
Found two facets intersect each other.
  1st: [3, 5, 6] #10
  2nd: [3, 5, 1] #2
A self-intersection was detected. Program stopped.
Hint: use -d option to detect all self-intersections.";
    const DIAG: &str = "Detecting self-intersecting facets...
  Facet #10 intersects facet #2 at triangles:
    (   3,    5,    6) and (   3,    5,    1)
  Facet #10 intersects facet #3 at triangles:
    (   3,    5,    6) and (   2,    3,    4)
  Facet #10 intersects facet #2 at triangles:
    (   3,    5,    6) and (   3,    5,    1)

!! Found 3 pairs of faces are intersecting.";
    let stopped = Scripted {
        main: STOP,
        main_exit: 3,
        diag: DIAG,
        diag_face: Some("3  1\n1 3 5 6 9\n2 3 5 1 1\n3 2 3 4 2\n"),
    };
    let (dir, m) = fake_run("scripted-stop", &p, &stopped);
    assert_eq!(
        m.codes,
        [
            codes::TETGEN_EXIT_NONZERO,
            codes::TETGEN_SELF_INTERSECTION,
            codes::TETGEN_OUTPUT_MISSING,
            codes::NEIGH_MISSING
        ],
        "{m:#?}"
    );
    let si = m.self_intersection.as_ref().unwrap();
    let stop = si.stop.as_ref().unwrap();
    assert_eq!(stop.message, "Found two facets intersect each other.");
    assert_eq!(stop.first.markers, [9]);
    assert_eq!(stop.second.as_ref().unwrap().markers, [1]);
    assert_eq!(si.pairs, [[1, 9], [2, 9]]);
    let named: Vec<(i64, Option<u32>, Option<&str>)> = si
        .facets
        .iter()
        .map(|f| (f.marker, f.scene_face, f.group.as_deref()))
        .collect();
    let groups: Vec<Option<String>> = [1usize, 2, 9]
        .iter()
        .map(|&k| {
            let gid = p.geometry.faces[k].group;
            p.surface_groups
                .iter()
                .find(|g| g.id == gid)
                .map(|g| g.name.clone())
        })
        .collect();
    assert_eq!(
        named,
        [
            (1, Some(1), groups[0].as_deref()),
            (2, Some(2), groups[1].as_deref()),
            (9, Some(9), groups[2].as_deref())
        ]
    );
    assert_eq!(groups[2].as_deref(), Some("Walls"));
    let d = m.diagnosis.as_ref().unwrap();
    assert_eq!(d.face_markers, [9, 1, 2]);
    assert_eq!(d.intersections.len(), 2, "the repeated pair is read once");
    assert!(dir.join("diag/scene_mesh.poly").is_file());
    // No pair named, and a follow-up that names nothing: the code stands on the stop alone.
    let bare = Scripted {
        main: "A self-intersection was detected. Program stopped.",
        main_exit: 3,
        diag: "No faces are intersecting.",
        diag_face: None,
    };
    let (_, m) = fake_run("scripted-bare", &p, &bare);
    assert!(has(&m, codes::TETGEN_SELF_INTERSECTION), "{m:#?}");
    let si = m.self_intersection.as_ref().unwrap();
    assert!(si.stop.is_none() && si.pairs.is_empty() && si.facets.is_empty());
    assert!(m.messages.iter().any(|s| s.contains("it named no pair")));

    // Says no: the same stdout with exit 0 is no stop (and no follow-up runs).
    let exit0 = Scripted {
        main_exit: 0,
        ..stopped
    };
    let (_, m) = fake_run("scripted-exit0", &p, &exit0);
    assert_eq!(
        m.codes,
        [codes::TETGEN_OUTPUT_MISSING, codes::NEIGH_MISSING],
        "{m:#?}"
    );
    assert!(m.self_intersection.is_none() && m.diagnosis.is_none());
    // Says no: exit 3 with TetGen 1.6.0's stop line, and no _skipped.face, is a nonzero exit only.
    let v16 = Scripted {
        main: "The input surface mesh contain self-intersections. Program stopped.",
        main_exit: 3,
        diag: "",
        diag_face: None,
    };
    let (_, m) = fake_run("scripted-16", &p, &v16);
    assert_eq!(
        m.codes,
        [
            codes::TETGEN_EXIT_NONZERO,
            codes::TETGEN_OUTPUT_MISSING,
            codes::NEIGH_MISSING
        ],
        "{m:#?}"
    );
    assert!(m.self_intersection.is_none() && m.diagnosis.is_none());
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
    let scene = mesh::project_input(&load_room("tutorial1_box.simpa"))
        .unwrap()
        .scene;
    let unitize = mesh::Unitize::of_scene(&scene).unwrap();
    let (built, _) = mesh::build_mbin(&out, 12, &unitize).unwrap();
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
    // `fake_run` also asserts that no `tetramesh.mbin` is in the folder. TetGen 1.5.0 meshes the
    // box to 2,257 tetrahedra, so each scene face is carried by many tetrahedron faces and the
    // one that lost its row leaves its scene face covered. (TetGen 1.6.0's 6-tetrahedron box
    // carried each scene face on one tetrahedron face, and the same drop also gave
    // `uncovered_scene_faces`.)
    let (_, m) = fake_run("fake-unmarked", &p, &fake);
    assert_eq!(
        m.codes,
        [codes::MESH_INVALID, "unmarked_boundary_faces"],
        "{m:#?}"
    );
    let report = m.verify.as_ref().unwrap();
    assert_eq!(report.unmarked_boundary_faces, 1, "{report:#?}");
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
        solver_id: None,
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
    // The zone, 2, and the room as TetGen numbered it, 3.
    assert_eq!(volumes.keys().copied().collect::<Vec<_>>(), [2, 3]);
    assert!((volumes[&2] - 1.0).abs() <= 1e-9, "{volumes:?}");
    assert!((volumes[&3] - 179.0).abs() / 179.0 <= 1e-9, "{volumes:?}");

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

/// A self-intersecting project with the real TetGen (1.5.0): the stop and the `-d` follow-up's
/// facets are named by scene face and surface group, and a box fitting zone's triangles by the
/// zone.
#[test]
fn self_intersecting_facets_are_named_by_group_and_fitting_zone() {
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
    let m = run(&p, &scratch("stopped-baffle"));
    let si = m.self_intersection.as_ref();
    println!(
        "baffle: codes {:?}, stop {:?}, pairs {:?}, facets {:?}, -d face markers {:?}",
        m.codes,
        si.and_then(|s| s.stop.as_ref()),
        si.map(|s| &s.pairs),
        si.map(|s| &s.facets),
        m.diagnosis.as_ref().map(|d| &d.face_markers)
    );
    assert!(has(&m, codes::TETGEN_SELF_INTERSECTION), "{m:#?}");
    assert!(!has(&m, codes::TETGEN_SKIPPED_FACETS), "{m:#?}");
    let si = si.unwrap();
    let named: Vec<(i64, Option<u32>, Option<&str>)> = si
        .facets
        .iter()
        .map(|s| (s.marker, s.scene_face, s.group.as_deref()))
        .collect();
    assert_eq!(
        named,
        [(9, Some(9), Some("Walls")), (12, Some(12), Some("Baffle"))]
    );
    assert!(si.facets.iter().all(|s| s.fitting_zone.is_none()));
    assert_eq!(si.pairs, [[9, 12]]);

    // Two box zones that overlap: the facets named are zone triangles (markers past the 12 scene
    // faces), each named by its zone, with no scene face or group.
    let mut p = load_room("tutorial1_box.simpa");
    with_named_box_zone(&mut p, "Zone 1", 1, [1.0, 1.0, 0.5], [2.0, 2.0, 1.5]);
    with_named_box_zone(&mut p, "Zone 2", 2, [1.5, 1.5, 1.0], [2.5, 2.5, 2.0]);
    let m = run(&p, &scratch("stopped-zones"));
    let si = m.self_intersection.as_ref();
    println!(
        "overlapping zones: codes {:?}, zone facets {:?}, stop {:?}, pairs {:?}, facets {:?}",
        m.codes,
        m.zone_facets,
        si.and_then(|s| s.stop.as_ref()),
        si.map(|s| &s.pairs),
        si.map(|s| &s.facets)
    );
    assert!(has(&m, codes::TETGEN_SELF_INTERSECTION), "{m:#?}");
    let si = si.unwrap();
    assert!(!si.facets.is_empty());
    let mut zones = BTreeSet::new();
    for s in &si.facets {
        let zone = match s.marker {
            12..24 => "Zone 1",
            24..36 => "Zone 2",
            _ => panic!("{s:?} is not a zone triangle"),
        };
        assert_eq!((s.scene_face, s.group.as_deref()), (None, None), "{s:?}");
        assert_eq!(s.fitting_zone.as_deref(), Some(zone), "{s:?}");
        zones.insert(zone);
    }
    assert_eq!(zones.len(), 2, "both zones are named");
    // Every pair is one triangle of each zone.
    assert!(
        si.pairs
            .iter()
            .all(|&[a, b]| (12..24).contains(&a) && (24..36).contains(&b)),
        "{:?}",
        si.pairs
    );
}

/// A stand-in for `preprocess.exe`: it applies `edit` to the folder's `scene_mesh.poly`, prints
/// `lines` on stdout and exits `exit`.
struct FakePreprocess {
    lines: &'static str,
    exit: u32,
    edit: fn(&Path),
}

impl Mesher for FakePreprocess {
    fn program(&self) -> Option<&Path> {
        None
    }

    fn run(
        &self,
        dir: &Path,
        args: &[String],
        _cancel: &CancelToken,
        on_line: &mut dyn FnMut(&Line),
    ) -> io::Result<Outcome> {
        assert_eq!(args, ["scene_mesh.poly"], "preprocess.exe's one argument");
        (self.edit)(dir);
        for l in self.lines.lines() {
            on_line(&Line {
                stream: simpa_core::process::Stream::Stdout,
                t_ms: 0.0,
                text: l.to_string(),
                terminated: true,
            });
        }
        Ok(exited(self.exit))
    }
}

/// `preprocess.exe`'s statistics as it prints them when it saves.
const SAVED: &str = "Coplanar correction step and vertices merging step has finished.\n\
                     Remesh status : \nVertices merged : 0\nCoplanar Faces destroyed : 0\n\
                     Face splitted : 0\n";
const SAVED_ONE_DESTROYED: &str = "Remesh status : \nVertices merged : 0\n\
                                   Coplanar Faces destroyed : 1\nFace splitted : 0\n";

/// The folder's `.poly` with `f` applied to its model.
fn edit_poly(dir: &Path, f: impl FnOnce(&mut poly::Model)) {
    let path = dir.join("scene_mesh.poly");
    let mut m = poly::read_file(&path).unwrap();
    f(&mut m);
    poly::write_file(&m, &path).unwrap();
}

/// A `preprocess.exe` that never finishes on its own: it waits for its cancel, for at most 20 s.
struct HungPreprocess;

impl Mesher for HungPreprocess {
    fn program(&self) -> Option<&Path> {
        None
    }

    fn run(
        &self,
        _dir: &Path,
        _args: &[String],
        cancel: &CancelToken,
        _on_line: &mut dyn FnMut(&Line),
    ) -> io::Result<Outcome> {
        let start = std::time::Instant::now();
        while !cancel.is_cancelled() && start.elapsed().as_secs() < 20 {
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        Ok(Outcome {
            exit_code: None,
            cancelled: cancel.is_cancelled(),
            elapsed_ms: start.elapsed().as_secs_f64() * 1e3,
        })
    }
}

/// Upstream's scene correction in the mesher (`docs/m5-m6-design.md`, decision 12), each of its
/// codes on the input that fires it, with `preprocess.exe` played by [`FakePreprocess`] and the
/// real TetGen after it. `preprocess.exe` exits 0 whatever it did, so its lines and the file it
/// saved are what the mesher reads. When it saves nothing (it gives up, cannot read the file, or
/// prints no statistics), the `.poly` as written is meshed, as upstream's GUI meshes it, the abort
/// recorded, and the geometry check on that `.poly` is the gate.
#[test]
fn every_preprocess_failure_code_fires_on_its_input() {
    let mut p = load_room("tutorial1_box.simpa");
    p.solvers.meshing.preprocess = true;
    let tetgen = tetgen();
    let run = |p: &simpa_core::schema::Project,
               label: &str,
               pre: Option<&dyn Mesher>,
               timeouts: mesh::Timeouts|
     -> (PathBuf, MeshManifest) {
        let dir = scratch(label);
        let tools = mesh::MeshTools {
            tetgen: &tetgen,
            preprocess: pre,
            markers: mesh::Markers::Restored,
            timeouts,
        };
        let m = mesh::mesh_project_with(p, &dir, &tools, &CancelToken::new(), &mut |_: &Line| {})
            .unwrap();
        (dir, m)
    };
    let go = |label: &str, pre: Option<&dyn Mesher>| run(&p, label, pre, mesh::Timeouts::default());
    let failed = |dir: &Path, m: &MeshManifest, codes_: &[&str]| {
        assert_eq!(m.codes, codes_, "{m:#?}");
        assert_eq!(m.status, MeshStatus::Fail);
        assert!(!dir.join("tetramesh.mbin").exists());
        assert!(m.tetgen.is_none(), "TetGen ran: {m:#?}");
    };

    // Control: it saves the file unchanged and prints its statistics: the box meshes.
    let same = FakePreprocess {
        lines: SAVED,
        exit: 0,
        edit: |_| {},
    };
    let (dir, m) = go("pre-ok", Some(&same));
    assert!(m.is_ok(), "{m:#?}");
    let report = m.preprocess.as_ref().unwrap();
    assert_eq!(report.input_sha256, report.output_sha256.clone().unwrap());
    assert_eq!(report.outcome, Some(mesh::PreprocessOutcome::Corrected));
    assert!(dir.join("scene_mesh.input.poly").is_file());
    assert!(m.verify.as_ref().unwrap().regions_checked);
    // The limits each call ran under are recorded: the defaults.
    assert_eq!(
        (
            m.tetgen.as_ref().unwrap().timeout_ms,
            report.call.timeout_ms
        ),
        (
            Some(mesh::Timeouts::TETGEN.as_secs_f64() * 1e3),
            Some(mesh::Timeouts::PREPROCESS.as_secs_f64() * 1e3)
        )
    );

    // None given.
    let (dir, m) = go("pre-none", None);
    failed(&dir, &m, &[codes::PREPROCESS_LAUNCH_FAILED]);

    // It gives up: the abort line and no statistics, the file untouched, exit 0.
    let aborted = FakePreprocess {
        lines: "Split Triangle for the 104 times. [41.1;-0.65;8.55]\nMesh reparation has been \
                aborted. The algorithm enter into an infinite loop. Try to stick coplanar faces \
                or destroy manually.\n",
        exit: 0,
        edit: |_| {},
    };
    // Each of the three: the .poly as written is meshed, OK, and the abort is recorded.
    let meshed_as_written = |dir: &Path, m: &MeshManifest, why: &str| {
        assert!(m.is_ok(), "{m:#?}");
        let report = m.preprocess.as_ref().unwrap();
        assert_eq!(report.outcome, Some(mesh::PreprocessOutcome::Aborted));
        assert!(
            report.aborted_reason.as_deref().unwrap().contains(why),
            "{report:#?}"
        );
        assert!(
            m.messages.iter().any(|l| l.contains("uncorrected")),
            "{m:#?}"
        );
        // TetGen read the .poly as written, byte for byte.
        let written = std::fs::read(dir.join("scene_mesh.input.poly")).unwrap();
        assert!(std::fs::read(dir.join("scene_mesh.poly")).unwrap() == written);
        assert_eq!(m.files.poly.as_deref(), Some(report.input_sha256.as_str()));
        assert_eq!(m.geometry.as_ref().unwrap().verdict, "ok");
        assert!(dir.join("tetramesh.mbin").is_file());
    };
    let (dir, m) = go("pre-aborted", Some(&aborted));
    meshed_as_written(&dir, &m, "gave up");
    // It cannot read the file.
    let not_found = FakePreprocess {
        lines: "The mesh file cant be found !\n",
        exit: 0,
        edit: |_| {},
    };
    let (dir, m) = go("pre-not-found", Some(&not_found));
    meshed_as_written(&dir, &m, "could not read");
    // It prints no statistics at all.
    let silent = FakePreprocess {
        lines: "",
        exit: 0,
        edit: |_| {},
    };
    let (dir, m) = go("pre-silent", Some(&silent));
    meshed_as_written(&dir, &m, "no statistics");
    // It gives up after leaving something else in the file: the .poly as written is put back.
    let aborted_after_edit = FakePreprocess {
        lines: "Mesh reparation has been aborted. The algorithm enter into an infinite loop.\n",
        exit: 0,
        edit: |d| {
            edit_poly(d, |m| {
                m.model_faces.remove(3);
            })
        },
    };
    let (dir, m) = go("pre-aborted-edited", Some(&aborted_after_edit));
    meshed_as_written(&dir, &m, "gave up");
    // Says no: it gives up on a .poly the geometry check refuses (a wall face gone, so the room
    // is open), and the check refuses it, before TetGen runs.
    let mut open_room = p.clone();
    open_room.geometry.faces.remove(3);
    let (dir, m) = run(
        &open_room,
        "pre-aborted-open",
        Some(&aborted),
        mesh::Timeouts::default(),
    );
    failed(&dir, &m, &[codes::GEOMETRY_REFUSED]);
    assert_eq!(
        m.preprocess.as_ref().unwrap().outcome,
        Some(mesh::PreprocessOutcome::Aborted)
    );
    let gate = m.geometry.as_ref().unwrap();
    assert_eq!(
        (gate.checked.as_str(), gate.verdict.as_str()),
        ("written, preprocess.exe having given up", "refused")
    );
    assert!(
        gate.reasons.iter().any(|r| r.code == "open_boundary"),
        "{gate:#?}"
    );
    // It saves a user facet it never merged: what it saved cannot be accounted for.
    let unmerged = FakePreprocess {
        lines: SAVED,
        exit: 0,
        edit: |d| {
            edit_poly(d, |m| {
                let f = m.model_faces[0];
                m.user_defined_faces.push(f);
            })
        },
    };
    let (dir, m) = go("pre-unmerged", Some(&unmerged));
    failed(&dir, &m, &[codes::PREPROCESS_OUTPUT_INVALID]);
    assert!(m.messages[0].contains("never merged"), "{m:#?}");

    // It never finishes: stopped at the mesher's limit, nothing meshed; the control above ran
    // under the default limit.
    let limit = mesh::Timeouts {
        preprocess: std::time::Duration::from_millis(50),
        ..mesh::Timeouts::default()
    };
    let (dir, m) = run(&p, "pre-hung", Some(&HungPreprocess), limit);
    failed(&dir, &m, &[codes::PREPROCESS_TIMEOUT]);
    let call = &m.preprocess.as_ref().unwrap().call;
    assert!(
        call.timed_out && call.cancelled && call.exit_code.is_none(),
        "{call:#?}"
    );
    assert!(call.elapsed_ms < 5_000.0, "{call:#?}");

    // Its exit code: a crash, and any other nonzero code.
    let crash = FakePreprocess {
        lines: "",
        exit: 0xC000_0005,
        edit: |_| {},
    };
    let (dir, m) = go("pre-crash", Some(&crash));
    failed(
        &dir,
        &m,
        &[codes::PREPROCESS_CRASH, codes::PREPROCESS_EXIT_NONZERO],
    );
    let one = FakePreprocess {
        lines: SAVED,
        exit: 1,
        edit: |_| {},
    };
    let (dir, m) = go("pre-exit-1", Some(&one));
    failed(&dir, &m, &[codes::PREPROCESS_EXIT_NONZERO]);

    // What it saved cannot be accounted for: a facet gone that it did not count, and a facet
    // moved off the one it came from.
    let lost = FakePreprocess {
        lines: SAVED,
        exit: 0,
        edit: |d| {
            edit_poly(d, |m| {
                m.model_faces.remove(3);
            })
        },
    };
    let (dir, m) = go("pre-lost", Some(&lost));
    failed(&dir, &m, &[codes::PREPROCESS_OUTPUT_INVALID]);
    assert!(m.messages[0].contains("counted 0 destroyed"), "{m:#?}");
    let moved = FakePreprocess {
        lines: SAVED,
        exit: 0,
        edit: |d| {
            edit_poly(d, |m| {
                m.model_faces[0].face_index = 5;
            })
        },
    };
    let (dir, m) = go("pre-moved", Some(&moved));
    failed(&dir, &m, &[codes::PREPROCESS_OUTPUT_INVALID]);

    // Accounted for, and refused by the geometry check before TetGen runs: a wall facet deleted,
    // and counted, leaves the room open.
    let open = FakePreprocess {
        lines: SAVED_ONE_DESTROYED,
        exit: 0,
        edit: |d| {
            edit_poly(d, |m| {
                m.model_faces.remove(3);
            })
        },
    };
    let (dir, m) = go("pre-open", Some(&open));
    failed(&dir, &m, &[codes::GEOMETRY_REFUSED]);
    let gate = m.geometry.as_ref().unwrap();
    assert_eq!(
        (gate.checked.as_str(), gate.verdict.as_str()),
        ("preprocessed", "refused")
    );
    assert!(
        gate.reasons.iter().any(|r| r.code == "open_boundary"),
        "{gate:#?}"
    );
    assert_eq!(
        m.preprocess
            .as_ref()
            .unwrap()
            .accounting
            .as_ref()
            .unwrap()
            .deleted,
        [3]
    );
}

/// TetGen still running at the mesher's limit is stopped, its process tree killed, and the mesh
/// fails by name (`tetgen_timeout`), with no `.1.ele` and no `.mbin`: the Elmia hall, which TetGen
/// meshes in seconds, under a 100 ms limit. The limit is recorded in the call. (The control, a
/// mesh under the default limit, is every other test here; `every_preprocess_failure_code_...`
/// asserts the default is what the manifest records.)
#[test]
fn tetgen_is_stopped_at_the_meshers_limit() {
    let p = load_room("elmia_corrected.simpa");
    let tetgen = tetgen();
    let tools = mesh::MeshTools {
        tetgen: &tetgen,
        preprocess: None,
        markers: mesh::Markers::Restored,
        timeouts: mesh::Timeouts {
            tetgen: std::time::Duration::from_millis(100),
            ..mesh::Timeouts::default()
        },
    };
    let dir = scratch("tetgen-timeout");
    let m =
        mesh::mesh_project_with(&p, &dir, &tools, &CancelToken::new(), &mut |_: &Line| {}).unwrap();
    assert_eq!(m.codes, [codes::TETGEN_TIMEOUT], "{m:#?}");
    assert_eq!(m.status, MeshStatus::Fail);
    let call = m.tetgen.as_ref().unwrap();
    assert!(
        call.timed_out && call.cancelled && call.exit_code.is_none(),
        "{call:#?}"
    );
    assert_eq!(call.timeout_ms, Some(100.0));
    assert!(call.elapsed_ms < 10_000.0, "{call:#?}");
    assert!(!dir.join("scene_mesh.1.ele").exists());
    assert!(!dir.join("tetramesh.mbin").exists());
}
