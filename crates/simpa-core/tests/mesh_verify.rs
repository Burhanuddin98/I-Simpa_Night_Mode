//! `mesh::verify`: upstream's own meshes pass, every check fails on a mesh made to fail it and
//! on nothing else, and the committed broken TetGen sets report what they are.
//!
//! Mutations are made on upstream's tutorial-1 box mesh (2,257 tetrahedra), so a scene face is
//! covered by several tetrahedron faces and one mutation touches one invariant only.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Instant;

use simpa_core::formats::cbin::{self, Model};
use simpa_core::formats::mbin::{self, Mesh, TetraFace, Tetrahedron};
use simpa_core::formats::{FormatError, tetgen};
use simpa_core::mesh::verify::{FACE_CORNERS, VerifyReport, VolumeIds, verify_dir, verify_mesh};

/// Upstream's meshes carry the room as TetGen numbers it, 1 without fitting zones
/// (`docs/m5-m6-design.md`, decision 1): the default ids, which this crate's mesher uses too.
fn upstream() -> VolumeIds {
    let ids = VolumeIds::default();
    assert_eq!(
        ids,
        VolumeIds {
            room: 1,
            fittings: vec![]
        }
    );
    ids
}

/// The ids of a mesh whose room is 0: upstream's Python-binding test mesh and Night Mode's broken
/// hall, built by other tools than TetGen's numbering.
fn room_0() -> VolumeIds {
    VolumeIds {
        room: 0,
        fittings: vec![],
    }
}

#[allow(dead_code)]
#[path = "common/paths.rs"]
mod paths;
use paths::fixture;

fn load(mbin_rel: &str, cbin_rel: &str) -> (Mesh, Model) {
    let mesh = mbin::read_file(&fixture(mbin_rel)).unwrap_or_else(|e| panic!("{mbin_rel}: {e}"));
    let scene = cbin::read_file(&fixture(cbin_rel)).unwrap_or_else(|e| panic!("{cbin_rel}: {e}"));
    (mesh, scene)
}

/// Upstream's tutorial-1 box: its SPPS run's mesh and the `.cbin` beside it.
fn tutorial1() -> (Mesh, Model) {
    load(
        "upstream/tutorial1/spps/tetramesh.mbin",
        "upstream/tutorial1/spps/mesh.cbin",
    )
}

/// A fresh, empty scratch folder for one test.
fn scratch(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("mesh_verify")
        .join(name);
    if dir.exists() {
        std::fs::remove_dir_all(&dir).unwrap();
    }
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn copy(from: &Path, to_dir: &Path, name: &str) {
    std::fs::copy(from, to_dir.join(name))
        .unwrap_or_else(|e| panic!("copy {} -> {name}: {e}", from.display()));
}

fn codes(r: &VerifyReport) -> Vec<&str> {
    r.codes.iter().map(String::as_str).collect()
}

/// The one line per report the measurements quote.
fn summary(label: &str, r: &VerifyReport) -> String {
    format!(
        "{label}: tets {} nodes {} scene_faces {} | index_errors {} degenerate_tets {} \
         inverted_tets {} misordered_faces {} unmarked_boundary_faces {} marker_out_of_range {} \
         nonmutual_neighbors {} asymmetric_internal_faces {} marker_geometry_mismatches {} \
         uncovered_scene_faces {} (first {:?}) unknown_volume_ids {} | volume_by_id {:?} | \
         tolerance {:.3e} m, max marker distance {:.3e} m | codes {:?}",
        r.tetrahedra,
        r.nodes,
        r.scene_faces,
        r.index_errors,
        r.degenerate_tets,
        r.inverted_tets,
        r.misordered_faces,
        r.unmarked_boundary_faces,
        r.marker_out_of_range,
        r.nonmutual_neighbors,
        r.asymmetric_internal_faces,
        r.marker_geometry_mismatches,
        r.uncovered_scene_faces,
        r.uncovered_scene_faces_first,
        r.unknown_volume_ids,
        r.volume_by_id,
        r.marker_tolerance_m,
        r.max_marker_distance_m,
        r.codes
    )
}

// ---------------------------------------------------------------------------------------------
// Upstream's meshes pass.

#[test]
fn upstream_cube_passes() {
    let (mesh, scene) = load(
        "upstream/lib_interface/cube_mesh.mbin",
        "upstream/lib_interface/cube.cbin",
    );
    let r = verify_mesh(&mesh, &scene, &upstream());
    println!("{}", summary("cube", &r));
    assert!(r.passed(), "{}", summary("cube", &r));
    assert_eq!((r.tetrahedra, r.nodes, r.scene_faces), (6, 8, 12));
    let v = r.volume_by_id[&1];
    assert!((v - 125.0).abs() < 1e-9, "cube volume {v}");
    assert_eq!(r.volume_by_id.len(), 1);
}

#[test]
fn upstream_tutorial1_passes() {
    let (mesh, scene) = tutorial1();
    let r = verify_mesh(&mesh, &scene, &upstream());
    println!("{}", summary("tutorial1", &r));
    assert!(r.passed(), "{}", summary("tutorial1", &r));
    assert_eq!((r.tetrahedra, r.nodes), (2257, 732));
    let v = r.volume_by_id[&1];
    assert!((v - 180.0).abs() < 1e-6, "box volume {v}");
    // Upstream's GUI carries every node through f32 GL coordinates and back. On this mesh that
    // moves marked nodes off their scene faces by less than one unit roundoff of the model size
    // (measured 0.69 · 2^-24 · 10 m = 4.1e-7 m), a sixteenth of the tolerance.
    let u_r = r.marker_tolerance_m / 16.0;
    assert!(r.max_marker_distance_m > 0.0);
    assert!(r.max_marker_distance_m < u_r, "{}", r.max_marker_distance_m);
}

#[test]
fn upstream_python_bindings_mesh_passes_with_room_0() {
    // Upstream's python-binding test mesh carries its room as 0, not 1.
    let (mesh, scene) = load(
        "upstream/python_bindings/tetramesh.mbin",
        "upstream/python_bindings/mesh.cbin",
    );
    let r = verify_mesh(&mesh, &scene, &room_0());
    println!("{}", summary("python_bindings", &r));
    assert!(r.passed(), "{}", summary("python_bindings", &r));
    assert_eq!((r.tetrahedra, r.nodes, r.scene_faces), (102, 46, 76));
    // Under TetGen's numbering its 0 is no room part: every tetrahedron is unknown.
    let r = verify_mesh(&mesh, &scene, &VolumeIds::default());
    assert_eq!(codes(&r), ["unknown_volume_ids"]);
    assert_eq!(r.unknown_volume_ids, 102);
}

#[test]
fn a_room_written_0_fails_under_tetgens_numbering() {
    // Upstream's tutorial-1 mesh with its room written 0, as this crate's builder wrote it before
    // decision 1 was reversed (2026-09-24): every tetrahedron is an unknown volume, and nothing
    // else. Read with the room from 0 it passes.
    let (mut mesh, scene) = tutorial1();
    for t in &mut mesh.tetrahedra {
        t.id_volume = 0;
    }
    let r = verify_mesh(&mesh, &scene, &VolumeIds::default());
    assert_eq!(codes(&r), ["unknown_volume_ids"]);
    assert_eq!(r.unknown_volume_ids, 2257);
    assert!(verify_mesh(&mesh, &scene, &room_0()).passed());
    // With one fitting zone, TetGen's room starts above it: upstream's 1 is then a fitting's id,
    // known, and a room at 1 beside a fitting declared as 2 is not.
    let (mesh, scene) = tutorial1();
    let with_fitting = VolumeIds::tetgen(vec![2]);
    assert_eq!(with_fitting.room, 3);
    let r = verify_mesh(&mesh, &scene, &with_fitting);
    assert_eq!(codes(&r), ["unknown_volume_ids"]);
    assert_eq!(r.unknown_volume_ids, 2257);
    assert!(verify_mesh(&mesh, &scene, &VolumeIds::tetgen(vec![1])).passed());
}

// ---------------------------------------------------------------------------------------------
// One mutation per check, each yielding exactly its own code.

/// How many tetrahedron faces carry each marker.
fn marker_uses(mesh: &Mesh) -> BTreeMap<i32, usize> {
    let mut uses = BTreeMap::new();
    for t in &mesh.tetrahedra {
        for f in &t.faces {
            *uses.entry(f.marker).or_insert(0) += 1;
        }
    }
    uses
}

/// A boundary face whose scene face other tetrahedron faces also carry: (tet, face).
fn boundary_face_on_shared_scene_face(mesh: &Mesh) -> (usize, usize) {
    let uses = marker_uses(mesh);
    for (t, tet) in mesh.tetrahedra.iter().enumerate() {
        for (i, f) in tet.faces.iter().enumerate() {
            if f.neighbor < 0 && f.marker >= 0 && uses[&f.marker] >= 2 {
                return (t, i);
            }
        }
    }
    panic!("no scene face is carried twice");
}

/// An interior face and its twin: ((tet, face), (neighbour, face)).
fn internal_face(mesh: &Mesh) -> ((usize, usize), (usize, usize)) {
    for (t, tet) in mesh.tetrahedra.iter().enumerate() {
        for (i, f) in tet.faces.iter().enumerate() {
            if f.neighbor >= 0 {
                let n = f.neighbor as usize;
                let mut key = f.vertices;
                key.sort_unstable();
                let k = mesh.tetrahedra[n]
                    .faces
                    .iter()
                    .position(|g| {
                        let mut gk = g.vertices;
                        gk.sort_unstable();
                        gk == key
                    })
                    .unwrap();
                return ((t, i), (n, k));
            }
        }
    }
    panic!("no interior face");
}

fn node_f64(mesh: &Mesh, v: i32) -> [f64; 3] {
    mesh.nodes[v as usize].map(f64::from)
}

/// Adds a scene face with the coordinates of `face`'s three nodes; returns its index.
fn add_scene_face_at(scene: &mut Model, mesh: &Mesh, face: &TetraFace) -> i32 {
    let base = scene.vertices.len() as u32;
    for &v in &face.vertices {
        let [x, y, z] = mesh.nodes[v as usize];
        scene.vertices.push(cbin::Vertex { x, y, z });
    }
    scene.faces.push(cbin::Face {
        a: base,
        b: base + 1,
        c: base + 2,
        id_mat: 0,
        id_rs: -1,
        id_en: -1,
    });
    (scene.faces.len() - 1) as i32
}

fn assert_only(r: &VerifyReport, code: &str) {
    assert_eq!(codes(r), [code], "{}", summary("mutated", r));
}

#[test]
fn flipped_winding_is_inverted_tets_only() {
    let (mut mesh, scene) = tutorial1();
    // Mirror tetrahedron 100: swap corners A and B, carry each face's marker and neighbour to the
    // slot of the corner it is opposite, and rewind every face by the table. Every face keeps its
    // three nodes, so only the orientation changes.
    let tet = &mut mesh.tetrahedra[100];
    tet.vertices.swap(0, 1);
    tet.faces.swap(0, 1);
    let v = tet.vertices;
    for (face, corners) in tet.faces.iter_mut().zip(FACE_CORNERS) {
        face.vertices = corners.map(|k| v[k]);
    }
    let r = verify_mesh(&mesh, &scene, &upstream());
    assert_only(&r, "inverted_tets");
    assert_eq!(r.inverted_tets, 1);
}

#[test]
fn unmarked_boundary_face_is_unmarked_boundary_faces_only() {
    let (mut mesh, scene) = tutorial1();
    let (t, i) = boundary_face_on_shared_scene_face(&mesh);
    mesh.tetrahedra[t].faces[i].marker = -1;
    let r = verify_mesh(&mesh, &scene, &upstream());
    assert_only(&r, "unmarked_boundary_faces");
    assert_eq!(r.unmarked_boundary_faces, 1);
}

#[test]
fn broken_neighbour_link_is_nonmutual_neighbors_only() {
    let (mut mesh, scene) = tutorial1();
    let ((t, i), (n, _)) = internal_face(&mesh);
    // Point the link at a tetrahedron that does not hold the face. Both ends now disagree:
    // t names a stranger, and n names t across a face t says belongs to the stranger.
    let stranger = (0..mesh.tetrahedra.len())
        .find(|&u| u != t && u != n)
        .unwrap();
    mesh.tetrahedra[t].faces[i].neighbor = stranger as i32;
    let r = verify_mesh(&mesh, &scene, &upstream());
    assert_only(&r, "nonmutual_neighbors");
    assert_eq!(r.nonmutual_neighbors, 2);
}

#[test]
fn neighbours_in_the_wrong_face_slots_are_nonmutual_neighbors_only() {
    // A mesher that writes `.neigh` columns into the wrong face slots: one tetrahedron's
    // neighbours across faces 0 and 1 trade places. Each link still names a tetrahedron that
    // links back to it, so a check of the back-link alone passes this; only comparing the three
    // nodes on both sides of the link catches it.
    let (mut mesh, scene) = tutorial1();
    let t = mesh
        .tetrahedra
        .iter()
        .position(|tet| tet.faces.iter().all(|f| f.neighbor >= 0 && f.marker < 0))
        .expect("an interior tetrahedron");
    let faces = &mut mesh.tetrahedra[t].faces;
    let (n0, n1) = (faces[0].neighbor, faces[1].neighbor);
    assert_ne!(n0, n1);
    faces[0].neighbor = n1;
    faces[1].neighbor = n0;
    let r = verify_mesh(&mesh, &scene, &upstream());
    assert_only(&r, "nonmutual_neighbors");
    // t's two swapped faces, and the face each former neighbour shares with t.
    assert_eq!(r.nonmutual_neighbors, 4);
}

#[test]
fn shared_face_without_a_link_is_nonmutual_neighbors_only() {
    // Both sides of an interior face lose their link, and both are marked with a scene face
    // placed exactly there, so they are not unmarked boundary faces: the face is still shared.
    let (mut mesh, mut scene) = tutorial1();
    let ((t, i), (n, k)) = internal_face(&mesh);
    let s = add_scene_face_at(&mut scene, &mesh, &mesh.tetrahedra[t].faces[i].clone());
    for (a, b) in [(t, i), (n, k)] {
        mesh.tetrahedra[a].faces[b].neighbor = -2;
        mesh.tetrahedra[a].faces[b].marker = s;
    }
    let r = verify_mesh(&mesh, &scene, &upstream());
    assert_only(&r, "nonmutual_neighbors");
    assert_eq!(r.nonmutual_neighbors, 2);
}

#[test]
fn marker_past_the_scene_is_marker_out_of_range_only() {
    let (mut mesh, scene) = tutorial1();
    let (t, i) = boundary_face_on_shared_scene_face(&mesh);
    mesh.tetrahedra[t].faces[i].marker = scene.faces.len() as i32;
    let r = verify_mesh(&mesh, &scene, &upstream());
    assert_only(&r, "marker_out_of_range");
    assert_eq!(r.marker_out_of_range, 1);
}

#[test]
fn marker_on_a_distant_scene_face_is_marker_geometry_mismatches_only() {
    let (mut mesh, scene) = tutorial1();
    let (t, i) = boundary_face_on_shared_scene_face(&mesh);
    let face = mesh.tetrahedra[t].faces[i];
    // The scene face whose plane lies farthest from this face's first node.
    let p = node_f64(&mesh, face.vertices[0]);
    let far = (0..scene.faces.len())
        .max_by(|&a, &b| {
            let d = |s: usize| {
                let v = scene.vertices[scene.faces[s].a as usize];
                let q = [v.x, v.y, v.z].map(f64::from);
                (0..3).map(|k| (p[k] - q[k]).abs()).fold(0.0, f64::max)
            };
            d(a).total_cmp(&d(b))
        })
        .unwrap();
    assert_ne!(far as i32, face.marker);
    mesh.tetrahedra[t].faces[i].marker = far as i32;
    let r = verify_mesh(&mesh, &scene, &upstream());
    assert_only(&r, "marker_geometry_mismatches");
    assert_eq!(r.marker_geometry_mismatches, 1);
}

/// A scene face's corners as `f64`.
fn scene_triangle(scene: &Model, s: usize) -> [[f64; 3]; 3] {
    let f = scene.faces[s];
    [f.a, f.b, f.c].map(|i| {
        let v = scene.vertices[i as usize];
        [v.x, v.y, v.z].map(f64::from)
    })
}

/// Distance from `p` to the plane through `tri`.
fn plane_distance(p: [f64; 3], tri: [[f64; 3]; 3]) -> f64 {
    let sub = |a: [f64; 3], b: [f64; 3]| [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
    let (u, v, w) = (sub(tri[1], tri[0]), sub(tri[2], tri[0]), sub(p, tri[0]));
    let n = [
        u[1] * v[2] - u[2] * v[1],
        u[2] * v[0] - u[0] * v[2],
        u[0] * v[1] - u[1] * v[0],
    ];
    let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
    (n[0] * w[0] + n[1] * w[1] + n[2] * w[2]).abs() / len
}

#[test]
fn marker_on_the_coplanar_neighbouring_triangle_is_marker_geometry_mismatches_only() {
    // Each wall of the box is two triangles. A face on one, marked with the other, lies in the
    // marker's plane but outside its triangle: only the "inside it" half of the check sees it.
    let (mut mesh, scene) = tutorial1();
    let tol = verify_mesh(&mesh, &scene, &upstream()).marker_tolerance_m;
    let uses = marker_uses(&mesh);
    let centroid =
        |pts: [[f64; 3]; 3]| [0, 1, 2].map(|k| (pts[0][k] + pts[1][k] + pts[2][k]) / 3.0);
    let gap = |a: [f64; 3], b: [f64; 3]| (0..3).map(|k| (a[k] - b[k]).powi(2)).sum::<f64>().sqrt();
    // Of the boundary faces whose scene face others also carry, the one farthest from a
    // coplanar other scene face: (distance between centroids, tet, face, that scene face).
    let mut best: Option<(f64, usize, usize, usize)> = None;
    for (t, tet) in mesh.tetrahedra.iter().enumerate() {
        for (i, f) in tet.faces.iter().enumerate() {
            if f.neighbor >= 0 || f.marker < 0 || uses[&f.marker] < 2 {
                continue;
            }
            let pts = f.vertices.map(|v| node_f64(&mesh, v));
            for s in (0..scene.faces.len()).filter(|&s| s as i32 != f.marker) {
                let tri = scene_triangle(&scene, s);
                if pts.iter().all(|&p| plane_distance(p, tri) <= tol / 16.0) {
                    let d = gap(centroid(pts), centroid(tri));
                    if best.is_none_or(|b| d > b.0) {
                        best = Some((d, t, i, s));
                    }
                }
            }
        }
    }
    let (_, t, i, s) = best.expect("a wall of two coplanar triangles");
    mesh.tetrahedra[t].faces[i].marker = s as i32;
    let r = verify_mesh(&mesh, &scene, &upstream());
    assert_only(&r, "marker_geometry_mismatches");
    assert_eq!(r.marker_geometry_mismatches, 1);
    // In the plane to within a sixteenth of the tolerance, yet metres outside the triangle.
    println!(
        "coplanar marker: tet {t} face {i} -> scene face {s}, {:.3} m outside it",
        r.max_marker_distance_m
    );
    assert!(r.max_marker_distance_m > 0.1, "{}", r.max_marker_distance_m);
}

#[test]
fn scene_face_with_no_markers_is_uncovered_scene_faces_only() {
    // Every tetrahedron face that carried scene face 5 moves to a copy of it appended to the
    // scene: they stay marked and on their face, and face 5 has no marker left.
    let (mut mesh, mut scene) = tutorial1();
    let copy = scene.faces[5];
    scene.faces.push(copy);
    let new = (scene.faces.len() - 1) as i32;
    let mut moved = 0;
    for tet in &mut mesh.tetrahedra {
        for f in &mut tet.faces {
            if f.marker == 5 {
                f.marker = new;
                moved += 1;
            }
        }
    }
    assert!(moved > 0);
    let r = verify_mesh(&mesh, &scene, &upstream());
    assert_only(&r, "uncovered_scene_faces");
    assert_eq!(r.uncovered_scene_faces, 1);
    assert_eq!(r.uncovered_scene_faces_first, [5]);

    // The literal mutation, every marker of face 5 removed (set to -1), leaves those boundary
    // faces unmarked as well, so it gives exactly those two codes.
    let (mut mesh, scene) = tutorial1();
    let mut cleared = 0;
    for tet in &mut mesh.tetrahedra {
        for f in &mut tet.faces {
            if f.marker == 5 {
                f.marker = -1;
                cleared += 1;
            }
        }
    }
    assert_eq!(cleared, moved);
    let r = verify_mesh(&mesh, &scene, &upstream());
    assert_eq!(
        codes(&r),
        ["unmarked_boundary_faces", "uncovered_scene_faces"],
        "{}",
        summary("face 5 unmarked", &r)
    );
    assert_eq!(r.unmarked_boundary_faces, cleared);
    assert_eq!(r.uncovered_scene_faces_first, [5]);
}

#[test]
fn volume_id_without_a_fitting_is_unknown_volume_ids_only() {
    // Fittings 7 and 9 seeded: TetGen's room starts at 10. One tetrahedron carrying 8, below the
    // room and no fitting's, is unknown.
    let (mut mesh, scene) = tutorial1();
    for t in &mut mesh.tetrahedra {
        t.id_volume = 10;
    }
    mesh.tetrahedra[3].id_volume = 7;
    mesh.tetrahedra[7].id_volume = 8;
    let ids = VolumeIds::tetgen(vec![7, 9]);
    let r = verify_mesh(&mesh, &scene, &ids);
    assert_only(&r, "unknown_volume_ids");
    assert_eq!(r.unknown_volume_ids, 1);
    assert!(r.volume_by_id.contains_key(&8));
    // Declaring 8 as a fitting makes it known.
    assert!(verify_mesh(&mesh, &scene, &VolumeIds::tetgen(vec![7, 8, 9])).passed());
    // An id above the room is a further room part, as TetGen numbers a second unseeded region
    // (tutorial 3's room is 2084 to 2086).
    mesh.tetrahedra[7].id_volume = 11;
    assert!(verify_mesh(&mesh, &scene, &ids).passed());
}

#[test]
fn repeated_corner_is_degenerate_tets_only() {
    let (mut mesh, scene) = tutorial1();
    let tet = &mut mesh.tetrahedra[42];
    tet.vertices[3] = tet.vertices[0];
    let r = verify_mesh(&mesh, &scene, &upstream());
    assert_only(&r, "degenerate_tets");
    assert_eq!(r.degenerate_tets, 1);
}

#[test]
fn corner_out_of_range_is_index_errors_only() {
    let (mut mesh, scene) = tutorial1();
    mesh.tetrahedra[42].vertices[2] = mesh.nodes.len() as i32;
    let r = verify_mesh(&mesh, &scene, &upstream());
    assert_only(&r, "index_errors");
    assert_eq!(r.index_errors, 1);
}

#[test]
fn neighbour_and_face_vertex_out_of_range_are_index_errors() {
    let (mut mesh, scene) = tutorial1();
    let (t, i) = boundary_face_on_shared_scene_face(&mesh);
    // A boundary face given a neighbour past the end: an index error, and no longer a
    // no-neighbour face.
    mesh.tetrahedra[t].faces[i].neighbor = mesh.tetrahedra.len() as i32;
    let r = verify_mesh(&mesh, &scene, &upstream());
    assert_only(&r, "index_errors");
    assert_eq!(r.index_errors, 1);

    // A face vertex of -5 on an interior face. One index error; the face is then also not the
    // face opposite its slot's corner, and no longer the face its neighbour holds, seen from both
    // ends. Each of the three is true of the mutated mesh, so all three are reported.
    let (mut mesh, scene) = tutorial1();
    let ((t, i), _) = internal_face(&mesh);
    mesh.tetrahedra[t].faces[i].vertices[0] = -5;
    let r = verify_mesh(&mesh, &scene, &upstream());
    assert_eq!(
        codes(&r),
        ["index_errors", "misordered_faces", "nonmutual_neighbors"],
        "{}",
        summary("face vertex -5", &r)
    );
    assert_eq!(
        (r.index_errors, r.misordered_faces, r.nonmutual_neighbors),
        (1, 1, 2)
    );
}

#[test]
fn reversed_face_is_misordered_faces_only() {
    let (mut mesh, scene) = tutorial1();
    mesh.tetrahedra[42].faces[2].vertices.swap(1, 2);
    let r = verify_mesh(&mesh, &scene, &upstream());
    assert_only(&r, "misordered_faces");
    assert_eq!(r.misordered_faces, 1);
    // A rotation of the table's triple is the same face, wound the same way.
    let (mut mesh, scene) = tutorial1();
    mesh.tetrahedra[42].faces[2].vertices.rotate_left(1);
    assert!(verify_mesh(&mesh, &scene, &upstream()).passed());
}

#[test]
fn internal_facet_marked_on_both_sides_passes_and_on_one_side_is_asymmetric() {
    let (mut mesh, mut scene) = tutorial1();
    let ((t, i), (n, k)) = internal_face(&mesh);
    let s = add_scene_face_at(&mut scene, &mesh, &mesh.tetrahedra[t].faces[i].clone());
    // Both sides: the invariant of decision 6.
    mesh.tetrahedra[t].faces[i].marker = s;
    mesh.tetrahedra[n].faces[k].marker = s;
    let r = verify_mesh(&mesh, &scene, &upstream());
    assert!(r.passed(), "{}", summary("both sides", &r));
    // One side only.
    mesh.tetrahedra[n].faces[k].marker = -1;
    let r = verify_mesh(&mesh, &scene, &upstream());
    assert_only(&r, "asymmetric_internal_faces");
    assert_eq!(r.asymmetric_internal_faces, 1);
}

/// One tetrahedron over the four triangles of its own surface: corners (0,0,0), (1,0,0),
/// (0,1,0) and (0.3, 0.3, h), face `i` marked `i`.
fn single_tet(h: f32) -> (Mesh, Model) {
    let nodes = vec![
        [0.0, 0.0, 0.0],
        [1.0, 0.0, 0.0],
        [0.0, 1.0, 0.0],
        [0.3, 0.3, h],
    ];
    let v = [0, 1, 2, 3];
    let faces = std::array::from_fn(|i| TetraFace {
        vertices: FACE_CORNERS[i].map(|k| v[k]),
        marker: i as i32,
        neighbor: mbin::NO_NEIGHBOR,
    });
    let mesh = Mesh {
        tetrahedra: vec![Tetrahedron {
            vertices: v,
            id_volume: 1,
            faces,
        }],
        nodes: nodes.clone(),
    };
    let scene = Model {
        vertices: nodes
            .iter()
            .map(|&[x, y, z]| cbin::Vertex { x, y, z })
            .collect(),
        faces: FACE_CORNERS
            .iter()
            .map(|c| cbin::Face {
                a: c[0] as u32,
                b: c[1] as u32,
                c: c[2] as u32,
                id_mat: 0,
                id_rs: -1,
                id_en: -1,
            })
            .collect(),
    };
    (mesh, scene)
}

#[test]
fn flat_tetrahedron_is_degenerate_tets_only() {
    // h = 1e-3: |6V| = 1e-3, about 2,400 times the f32 floor (4.1e-7 here). Passes.
    let (mesh, scene) = single_tet(1e-3);
    let r = verify_mesh(&mesh, &scene, &VolumeIds::default());
    assert!(r.passed(), "{}", summary("h=1e-3", &r));
    // h = 1e-7: |6V| = 1e-7, below the floor. Its sign is still right, but f32 cannot resolve it.
    let (mesh, scene) = single_tet(1e-7);
    let r = verify_mesh(&mesh, &scene, &VolumeIds::default());
    assert_only(&r, "degenerate_tets");
    // Upside down, it is inverted instead.
    let (mesh, scene) = single_tet(-1e-3);
    let r = verify_mesh(&mesh, &scene, &VolumeIds::default());
    assert_only(&r, "inverted_tets");
}

#[test]
fn marker_geometry_tolerance_says_yes_at_f32_noise_and_no_just_past_it() {
    // Scene 16 m from the origin on x: tolerance 16 * 2^-24 * 17 = 1.6e-5 m.
    let (mut mesh, mut scene) = single_tet(1.0);
    for p in &mut mesh.nodes {
        p[0] += 16.0;
    }
    for v in &mut scene.vertices {
        v.x += 16.0;
    }
    let r = verify_mesh(&mesh, &scene, &VolumeIds::default());
    assert!(r.passed(), "{}", summary("shifted", &r));
    let tol = r.marker_tolerance_m;
    assert!((tol - 16.0 * 17.0 / 16_777_216.0).abs() < 1e-12, "{tol}");
    // Node 3 lies on faces 0, 1 and 2. Moving it 2 ulps along x (at 16..32, 1 ulp = 2^-19 m)
    // keeps it within tolerance of all three.
    let shifted = mesh.clone();
    mesh.nodes[3][0] += 2.0 * 2f32.powi(-19);
    assert!(verify_mesh(&mesh, &scene, &VolumeIds::default()).passed());
    // Node 0 lies on faces 1, 2 and 3; the scene keeps its old position. Moved down by 0.9 of
    // the tolerance it is still on all three.
    let mut mesh = shifted.clone();
    mesh.nodes[0][2] -= (0.9 * tol) as f32;
    let r = verify_mesh(&mesh, &scene, &VolumeIds::default());
    assert!(r.passed(), "{}", summary("0.9 tol", &r));
    // Moved down by 4 times the tolerance it is off all three: 4 tol from the floor (face 3), and
    // 4 tol times sin 16.7° = 1.15 tol from the plane of face 2, the steepest.
    let mut mesh = shifted;
    mesh.nodes[0][2] -= (4.0 * tol) as f32;
    let r = verify_mesh(&mesh, &scene, &VolumeIds::default());
    assert_only(&r, "marker_geometry_mismatches");
    assert_eq!(r.marker_geometry_mismatches, 3);
}

// ---------------------------------------------------------------------------------------------
// Folders.

#[test]
fn broken_hall_folder() {
    let dir = fixture("meshes/broken_hall");
    // Night Mode's own .mbin, which writes the room as 0.
    let r = verify_dir(&dir, &room_0()).unwrap();
    let mesh = r
        .mesh
        .as_ref()
        .expect("the folder holds tetramesh.mbin and model.cbin");
    println!("{}", summary("broken hall", mesh));
    println!(
        "broken hall dir: basename {:?} skipped {} markers {:?} mbin {:?} cbin {:?} codes {:?}",
        r.basename,
        r.skipped_facets,
        r.skipped_markers
            .iter()
            .collect::<std::collections::BTreeSet<_>>(),
        r.mbin_file,
        r.cbin_file,
        r.codes
    );
    assert_eq!(r.basename.as_deref(), Some("model"));
    assert!(r.mbin_file.as_ref().unwrap().ends_with("tetramesh.mbin"));
    assert!(r.cbin_file.as_ref().unwrap().ends_with("model.cbin"));
    // The .poly carried no facet markers, so TetGen's skipped rows carry -1.
    assert_eq!(r.skipped_facets, 535);
    assert_eq!(r.skipped_markers, vec![-1; 535]);
    assert_eq!(
        r.codes,
        [
            "tetgen_skipped_facets",
            "neigh_missing",
            "regions_unchecked",
            "degenerate_tets",
            "unmarked_boundary_faces",
            "uncovered_scene_faces"
        ]
    );
    // Its model.poly does not read with this crate's reader, and is not replaced by the .cbin:
    // its regions are held to no cells (decision 15).
    let why = r.regions.unchecked.as_deref().unwrap();
    assert!(why.contains("model.poly does not read"), "{why}");
    // Arc item 12: faces with neither a marker nor a neighbour, where SPPS destroys particles,
    // and scene faces the mesh never reaches.
    assert_eq!(
        (mesh.tetrahedra, mesh.nodes, mesh.scene_faces),
        (6294, 1067, 1086)
    );
    assert_eq!(mesh.degenerate_tets, 67);
    assert_eq!(mesh.unmarked_boundary_faces, 267);
    assert_eq!(mesh.uncovered_scene_faces, 338);
    assert_eq!(
        mesh.uncovered_scene_faces_first,
        [
            0, 1, 2, 12, 13, 14, 25, 26, 27, 30, 32, 33, 44, 45, 52, 53, 56, 57, 58, 59
        ]
    );
    // Night Mode rebuilt the neighbours from the tetrahedra and matched markers by vertex
    // triple, so these hold.
    let others = [
        mesh.index_errors,
        mesh.inverted_tets,
        mesh.misordered_faces,
        mesh.marker_out_of_range,
        mesh.nonmutual_neighbors,
        mesh.asymmetric_internal_faces,
        mesh.marker_geometry_mismatches,
        mesh.unknown_volume_ids,
    ];
    assert_eq!(others, [0; 8]);
    assert_eq!(mesh.volume_by_id.keys().copied().collect::<Vec<_>>(), [0]);
}

#[test]
fn tg_bad_folder() {
    let dir = fixture("meshes/tg_bad");
    let r = verify_dir(&dir, &VolumeIds::default()).unwrap();
    assert_eq!(r.basename.as_deref(), Some("scene_mesh"));
    assert_eq!(r.skipped_facets, 3);
    assert_eq!(r.skipped_markers, [8, 9, 12]);
    assert_eq!(r.codes, ["tetgen_skipped_facets", "neigh_missing"]);
    assert!(r.mesh.is_none());
}

#[test]
fn complete_tetgen_set_passes_and_each_missing_file_is_reported() {
    // tg_bad's .1.* files plus a .1.neigh (verify_dir checks presence, not content), no skipped.
    let src = fixture("meshes/tg_bad");
    let dir = scratch("complete_set");
    for ext in ["1.node", "1.ele", "1.face"] {
        copy(
            &src.join(format!("scene_mesh.{ext}")),
            &dir,
            &format!("Scene_Mesh.{ext}"),
        );
    }
    std::fs::write(dir.join("scene_mesh.1.neigh"), b"placeholder").unwrap();
    let r = verify_dir(&dir, &VolumeIds::default()).unwrap();
    assert!(r.passed(), "{r:?}");
    assert_eq!(r.basename.as_deref(), Some("Scene_Mesh"));

    std::fs::remove_file(dir.join("scene_mesh.1.neigh")).unwrap();
    let r = verify_dir(&dir, &VolumeIds::default()).unwrap();
    assert_eq!(r.codes, ["neigh_missing"]);

    std::fs::write(dir.join("scene_mesh.1.neigh"), b"placeholder").unwrap();
    std::fs::remove_file(dir.join("Scene_Mesh.1.ele")).unwrap();
    let r = verify_dir(&dir, &VolumeIds::default()).unwrap();
    assert_eq!(r.codes, ["tetgen_output_missing"]);
}

#[test]
fn skipped_facets_without_any_mesh_output() {
    let dir = scratch("skipped_only");
    copy(
        &fixture("meshes/tg_bad/scene_mesh_skipped.face"),
        &dir,
        "scene_mesh_skipped.face",
    );
    let r = verify_dir(&dir, &VolumeIds::default()).unwrap();
    assert_eq!(
        r.codes,
        [
            "tetgen_skipped_facets",
            "neigh_missing",
            "tetgen_output_missing"
        ]
    );
}

/// A folder as the mesher writes it on success, from upstream's cube: `tetramesh.mbin`,
/// `mesh.cbin` and a manifest recording the `.mbin`'s sha256 (or `manifest` verbatim).
fn cube_folder(name: &str, manifest: Option<&str>) -> PathBuf {
    let dir = scratch(name);
    copy(
        &fixture("upstream/lib_interface/cube_mesh.mbin"),
        &dir,
        "tetramesh.mbin",
    );
    copy(
        &fixture("upstream/lib_interface/cube.cbin"),
        &dir,
        "mesh.cbin",
    );
    if let Some(m) = manifest {
        std::fs::write(dir.join("mesh.json"), m).unwrap();
    }
    dir
}

/// sha256 of upstream's `cube_mesh.mbin`, computed independently with `sha256sum`.
const CUBE_MBIN_SHA256: &str = "d24ae2ac53cebb26d8ad3275df27f7ec6759535868ce34af6fcca0fd1cff80cd";

#[test]
fn manifest_hash_is_checked_when_present() {
    let good = format!("{{\"stamp\": \"x\", \"mbin_sha256\": \"{CUBE_MBIN_SHA256}\"}}");
    let dir = cube_folder("manifest_good", Some(&good));
    let r = verify_dir(&dir, &upstream()).unwrap();
    assert!(r.passed(), "{r:?}");
    assert_eq!(r.mbin_sha256.as_deref(), Some(CUBE_MBIN_SHA256));
    assert!(r.mesh.as_ref().unwrap().passed());

    // Upper-case hex is the same hash.
    let upper = good.replace(CUBE_MBIN_SHA256, &CUBE_MBIN_SHA256.to_uppercase());
    let dir = cube_folder("manifest_upper", Some(&upper));
    assert!(verify_dir(&dir, &upstream()).unwrap().passed());

    // A UTF-8 BOM before the JSON is tolerated, and the hash is still checked behind it.
    let dir = cube_folder("manifest_bom", Some(&format!("\u{feff}{good}")));
    assert!(verify_dir(&dir, &upstream()).unwrap().passed());
    let bad = format!("\u{feff}{{\"mbin_sha256\": \"{}\"}}", "0".repeat(64));
    let dir = cube_folder("manifest_bom_bad", Some(&bad));
    assert_eq!(
        verify_dir(&dir, &upstream()).unwrap().codes,
        ["manifest_mismatch"]
    );

    // One digit off.
    let mut wrong = CUBE_MBIN_SHA256.to_string();
    wrong.replace_range(0..1, if wrong.starts_with('0') { "1" } else { "0" });
    let dir = cube_folder(
        "manifest_wrong",
        Some(&format!("{{\"mbin_sha256\": \"{wrong}\"}}")),
    );
    assert_eq!(
        verify_dir(&dir, &upstream()).unwrap().codes,
        ["manifest_mismatch"]
    );

    // Absent or null: not checked.
    for (name, m) in [
        ("manifest_absent", "{\"stamp\": \"x\"}"),
        ("manifest_null", "{\"mbin_sha256\": null}"),
    ] {
        let dir = cube_folder(name, Some(m));
        assert!(verify_dir(&dir, &upstream()).unwrap().passed(), "{name}");
    }

    // Not a string.
    let dir = cube_folder("manifest_number", Some("{\"mbin_sha256\": 12}"));
    assert_eq!(
        verify_dir(&dir, &upstream()).unwrap().codes,
        ["manifest_mismatch"]
    );

    // A hash for a .mbin the folder does not hold.
    let dir = cube_folder("manifest_no_mbin", Some(&good));
    std::fs::remove_file(dir.join("tetramesh.mbin")).unwrap();
    let r = verify_dir(&dir, &upstream()).unwrap();
    assert_eq!(r.codes, ["manifest_mismatch", "nothing_to_verify"]);

    // Not JSON: an error, not a verdict.
    let dir = cube_folder("manifest_garbage", Some("{not json"));
    assert!(matches!(
        verify_dir(&dir, &upstream()),
        Err(FormatError::Invalid(_))
    ));
}

/// A manifest in the mesher's layout (`docs/formats/mesh-manifest.md`, "Layout"), trimmed to the
/// keys around the one read: `files.mbin` is `mbin`, verbatim JSON.
fn mesher_manifest(mbin: &str) -> String {
    format!(
        "{{\n  \"manifest_version\": 1,\n  \"source\": \"project\",\n  \"status\": \"OK\",\n  \
         \"codes\": [],\n  \"files\": {{\n    \"poly\": \"{p}\",\n    \"var\": null,\n    \
         \"cbin\": \"{p}\",\n    \"mbin\": {mbin}\n  }},\n  \"skipped_rows\": 0\n}}\n",
        p = "a".repeat(64)
    )
}

#[test]
fn mesher_manifest_files_mbin_is_checked() {
    let quoted = |h: &str| format!("\"{h}\"");
    let dir = cube_folder(
        "files_good",
        Some(&mesher_manifest(&quoted(CUBE_MBIN_SHA256))),
    );
    let r = verify_dir(&dir, &upstream()).unwrap();
    assert!(r.passed(), "{r:?}");

    // Negative: one digit off.
    let mut wrong = CUBE_MBIN_SHA256.to_string();
    wrong.replace_range(0..1, if wrong.starts_with('0') { "1" } else { "0" });
    let dir = cube_folder("files_wrong", Some(&mesher_manifest(&quoted(&wrong))));
    assert_eq!(
        verify_dir(&dir, &upstream()).unwrap().codes,
        ["manifest_mismatch"]
    );

    // Negative: null says the mesher wrote no .mbin, so the one in the folder is not its own (a
    // partial write, or a leftover).
    let dir = cube_folder("files_null_beside_mbin", Some(&mesher_manifest("null")));
    assert_eq!(
        verify_dir(&dir, &upstream()).unwrap().codes,
        ["manifest_mismatch"]
    );

    // Negative: a record of the wrong type, for `files.mbin` and for `files` itself.
    for (name, m) in [
        ("files_mbin_number", mesher_manifest("7")),
        ("files_not_object", "{\"files\": \"x\"}".to_string()),
    ] {
        let dir = cube_folder(name, Some(&m));
        assert_eq!(
            verify_dir(&dir, &upstream()).unwrap().codes,
            ["manifest_mismatch"],
            "{name}"
        );
    }

    // No `files`, or `files` without `mbin`: not checked.
    for (name, m) in [
        ("files_absent", "{\"manifest_version\": 1}"),
        ("files_without_mbin", "{\"files\": {\"cbin\": null}}"),
    ] {
        let dir = cube_folder(name, Some(m));
        assert!(verify_dir(&dir, &upstream()).unwrap().passed(), "{name}");
    }

    // Both records kept: each is checked.
    let both = |files: &str, top: &str| {
        mesher_manifest(&quoted(files)).replacen(
            '{',
            &format!("{{\n  \"mbin_sha256\": \"{top}\","),
            1,
        )
    };
    let dir = cube_folder("both_good", Some(&both(CUBE_MBIN_SHA256, CUBE_MBIN_SHA256)));
    assert!(verify_dir(&dir, &upstream()).unwrap().passed());
    for (name, files, top) in [
        ("both_files_wrong", wrong.as_str(), CUBE_MBIN_SHA256),
        ("both_top_wrong", CUBE_MBIN_SHA256, wrong.as_str()),
    ] {
        let dir = cube_folder(name, Some(&both(files, top)));
        assert_eq!(
            verify_dir(&dir, &upstream()).unwrap().codes,
            ["manifest_mismatch"],
            "{name}"
        );
    }

    // A failed meshing: TetGen skipped facets and no .mbin was written. Null is then right, and a
    // hash is not.
    let src = fixture("meshes/tg_bad");
    for (name, mbin, expected) in [
        (
            "failed_null",
            "null".to_string(),
            &["tetgen_skipped_facets", "neigh_missing"][..],
        ),
        (
            "failed_hash",
            quoted(CUBE_MBIN_SHA256),
            &[
                "tetgen_skipped_facets",
                "neigh_missing",
                "manifest_mismatch",
            ][..],
        ),
    ] {
        let dir = scratch(name);
        for ext in [
            "1.node",
            "1.ele",
            "1.face",
            "_skipped.face",
            "_skipped.node",
        ] {
            let file = if ext.starts_with('_') {
                format!("scene_mesh{ext}")
            } else {
                format!("scene_mesh.{ext}")
            };
            copy(&src.join(&file), &dir, &file);
        }
        std::fs::write(dir.join("mesh.json"), mesher_manifest(&mbin)).unwrap();
        let r = verify_dir(&dir, &VolumeIds::default()).unwrap();
        assert_eq!(r.codes, expected, "{name}");
    }
}

#[test]
fn mesh_codes_follow_the_folder_codes() {
    // The cube (room 1) read with the ids of a project with one fitting zone, 2: TetGen would put
    // that room at 3, so the mesh check fails, and its code reaches the folder. The region check
    // against the folder's .cbin adds that no tetrahedron carries the fitting's id.
    let dir = cube_folder("cube_room1_fitting2", None);
    let r = verify_dir(&dir, &VolumeIds::tetgen(vec![2])).unwrap();
    assert_eq!(r.codes, ["unknown_volume_ids", "fitting_region_misplaced"]);
    assert_eq!(r.mesh.as_ref().unwrap().unknown_volume_ids, 6);
    // With the default ids it passes.
    assert!(verify_dir(&dir, &VolumeIds::default()).unwrap().passed());
}

#[test]
fn folder_without_anything_to_verify_fails() {
    let dir = scratch("empty");
    let r = verify_dir(&dir, &VolumeIds::default()).unwrap();
    assert_eq!(r.codes, ["nothing_to_verify"]);
    // A .cbin, a .poly and logs are inputs, not a mesh.
    copy(
        &fixture("upstream/lib_interface/cube.cbin"),
        &dir,
        "mesh.cbin",
    );
    copy(
        &fixture("meshes/tg_bad/scene_mesh.poly"),
        &dir,
        "scene_mesh.poly",
    );
    let r = verify_dir(&dir, &VolumeIds::default()).unwrap();
    assert_eq!(r.codes, ["nothing_to_verify"]);
}

#[test]
fn mbin_without_cbin_is_cbin_missing() {
    let dir = cube_folder("no_cbin", None);
    std::fs::remove_file(dir.join("mesh.cbin")).unwrap();
    let r = verify_dir(&dir, &upstream()).unwrap();
    assert_eq!(r.codes, ["cbin_missing"]);
    assert!(r.mesh.is_none());
}

#[test]
fn ambiguous_or_missing_folders_are_errors() {
    assert!(matches!(
        verify_dir(&fixture("meshes/no_such_folder"), &VolumeIds::default()),
        Err(FormatError::NotFound(_))
    ));

    // Two .mbin files, neither named tetramesh.mbin.
    let dir = cube_folder("two_mbin", None);
    std::fs::rename(dir.join("tetramesh.mbin"), dir.join("a.mbin")).unwrap();
    copy(&dir.join("a.mbin"), &dir, "b.mbin");
    assert!(matches!(
        verify_dir(&dir, &upstream()),
        Err(FormatError::Invalid(_))
    ));
    // With tetramesh.mbin among them, that one is taken.
    copy(&dir.join("a.mbin"), &dir, "tetramesh.mbin");
    let r = verify_dir(&dir, &upstream()).unwrap();
    assert!(r.passed(), "{r:?}");
    assert!(r.mbin_file.unwrap().ends_with("tetramesh.mbin"));

    // Two .cbin files, neither named mesh.cbin: without a .mbin none is needed, so the folder
    // gets a verdict; beside a .mbin they are ambiguous.
    let dir = scratch("two_cbin");
    let src = fixture("meshes/tg_bad");
    for ext in ["1.node", "1.ele", "1.face"] {
        copy(
            &src.join(format!("scene_mesh.{ext}")),
            &dir,
            &format!("scene_mesh.{ext}"),
        );
    }
    std::fs::write(dir.join("scene_mesh.1.neigh"), b"placeholder").unwrap();
    for name in ["a.cbin", "b.cbin"] {
        copy(&fixture("upstream/lib_interface/cube.cbin"), &dir, name);
    }
    let r = verify_dir(&dir, &upstream()).unwrap();
    assert!(r.passed(), "{r:?}");
    assert!(r.cbin_file.is_none());
    copy(
        &fixture("upstream/lib_interface/cube_mesh.mbin"),
        &dir,
        "tetramesh.mbin",
    );
    assert!(matches!(
        verify_dir(&dir, &upstream()),
        Err(FormatError::Invalid(_))
    ));

    // TetGen output under two basenames.
    let dir = scratch("two_bases");
    let src = fixture("meshes/tg_bad");
    copy(&src.join("scene_mesh.1.node"), &dir, "scene_mesh.1.node");
    copy(&src.join("scene_mesh.1.node"), &dir, "model.1.node");
    assert!(matches!(
        verify_dir(&dir, &VolumeIds::default()),
        Err(FormatError::Invalid(_))
    ));

    // A truncated _skipped.face is unreadable, not a count.
    let dir = scratch("bad_skipped");
    std::fs::write(dir.join("scene_mesh_skipped.face"), b"3 1\n1  1 4 6  8\n").unwrap();
    assert!(matches!(
        verify_dir(&dir, &VolumeIds::default()),
        Err(FormatError::Truncated { .. })
    ));
}

#[test]
fn skipped_face_markers_match_the_file() {
    // Independent read of the committed file: the markers verify_dir reports are its last column.
    let bytes = std::fs::read(fixture("meshes/tg_bad/scene_mesh_skipped.face")).unwrap();
    let f = tetgen::read_face(&bytes).unwrap();
    assert_eq!(f.markers.unwrap(), [8, 9, 12]);
    let text = String::from_utf8(bytes).unwrap();
    let last: Vec<i32> = text
        .lines()
        .skip(1)
        .map(|l| l.split_whitespace().last().unwrap().parse().unwrap())
        .collect();
    assert_eq!(last, [8, 9, 12]);
}

// ---------------------------------------------------------------------------------------------
// Performance.

/// A cube of side `size` split into `n³` cells of 6 Freudenthal tetrahedra each, over a scene of
/// 12 triangles (two per wall, split along the wall's (0,0)-(1,1) diagonal, the way every
/// Freudenthal cell face is split). Built to pass [`verify_mesh`].
fn grid(n: usize, size: f32) -> (Mesh, Model) {
    let g = n + 1;
    let idx = |x: usize, y: usize, z: usize| (x + g * (y + g * z)) as i32;
    let step = size / n as f32;
    let mut nodes = Vec::with_capacity(g * g * g);
    for z in 0..g {
        for y in 0..g {
            for x in 0..g {
                nodes.push([x as f32 * step, y as f32 * step, z as f32 * step]);
            }
        }
    }
    const ORDERS: [[usize; 3]; 6] = [
        [0, 1, 2],
        [0, 2, 1],
        [1, 0, 2],
        [1, 2, 0],
        [2, 0, 1],
        [2, 1, 0],
    ];
    let mut corners: Vec<[i32; 4]> = Vec::with_capacity(6 * n * n * n);
    for z in 0..n {
        for y in 0..n {
            for x in 0..n {
                for order in ORDERS {
                    let mut at = [x, y, z];
                    let mut tet = [idx(x, y, z), 0, 0, 0];
                    for (k, &axis) in order.iter().enumerate() {
                        at[axis] += 1;
                        tet[k + 1] = idx(at[0], at[1], at[2]);
                    }
                    let p = tet.map(|v| nodes[v as usize].map(f64::from));
                    let d = |a: [f64; 3], b: [f64; 3]| [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
                    let (a, b, c) = (d(p[0], p[3]), d(p[1], p[3]), d(p[2], p[3]));
                    let o = a[0] * (b[1] * c[2] - b[2] * c[1])
                        + a[1] * (b[2] * c[0] - b[0] * c[2])
                        + a[2] * (b[0] * c[1] - b[1] * c[0]);
                    if o > 0.0 {
                        tet.swap(0, 1);
                    }
                    corners.push(tet);
                }
            }
        }
    }
    // Neighbours: faces sorted by their node triple, pairs linked.
    let mut keys: Vec<([i32; 3], usize)> = Vec::with_capacity(4 * corners.len());
    for (t, tet) in corners.iter().enumerate() {
        for (i, fc) in FACE_CORNERS.iter().enumerate() {
            let mut k = fc.map(|c| tet[c]);
            k.sort_unstable();
            keys.push((k, 4 * t + i));
        }
    }
    keys.sort_unstable();
    let mut neighbor = vec![mbin::NO_NEIGHBOR; 4 * corners.len()];
    for run in keys.chunk_by(|a, b| a.0 == b.0) {
        if let [(_, s), (_, u)] = run {
            neighbor[*s] = (u / 4) as i32;
            neighbor[*u] = (s / 4) as i32;
        }
    }
    // Scene: wall w = 2 * axis + side, triangles 2w (u >= v) and 2w + 1 (v >= u).
    let corner = |bits: [usize; 3]| (bits[0] + 2 * bits[1] + 4 * bits[2]) as u32;
    let vertices: Vec<cbin::Vertex> = (0..8)
        .map(|c| cbin::Vertex {
            x: (c & 1) as f32 * size,
            y: ((c >> 1) & 1) as f32 * size,
            z: ((c >> 2) & 1) as f32 * size,
        })
        .collect();
    let mut faces = Vec::new();
    for axis in 0..3 {
        let (u, v) = match axis {
            0 => (1, 2),
            1 => (0, 2),
            _ => (0, 1),
        };
        for side in 0..2 {
            let at = |bu: usize, bv: usize| {
                let mut bits = [0; 3];
                bits[axis] = side;
                bits[u] = bu;
                bits[v] = bv;
                corner(bits)
            };
            for tri in [
                [at(0, 0), at(1, 0), at(1, 1)],
                [at(0, 0), at(1, 1), at(0, 1)],
            ] {
                faces.push(cbin::Face {
                    a: tri[0],
                    b: tri[1],
                    c: tri[2],
                    id_mat: 0,
                    id_rs: -1,
                    id_en: -1,
                });
            }
        }
    }
    let coord = |v: i32| {
        let v = v as usize;
        [v % g, (v / g) % g, v / (g * g)]
    };
    let tetrahedra = corners
        .iter()
        .enumerate()
        .map(|(t, &tet)| Tetrahedron {
            vertices: tet,
            id_volume: 1,
            faces: std::array::from_fn(|i| {
                let vertices = FACE_CORNERS[i].map(|c| tet[c]);
                let nb = neighbor[4 * t + i];
                let mut marker = -1;
                if nb < 0 {
                    let cs = vertices.map(coord);
                    let axis = (0..3)
                        .find(|&a| {
                            cs.iter().all(|c| c[a] == cs[0][a]) && (cs[0][a] == 0 || cs[0][a] == n)
                        })
                        .expect("a hull face lies on a wall");
                    let side = usize::from(cs[0][axis] == n);
                    let (u, v) = match axis {
                        0 => (1, 2),
                        1 => (0, 2),
                        _ => (0, 1),
                    };
                    let lean: i64 = cs.iter().map(|c| c[u] as i64 - c[v] as i64).sum();
                    marker = (2 * (2 * axis + side) + usize::from(lean < 0)) as i32;
                }
                TetraFace {
                    vertices,
                    marker,
                    neighbor: nb,
                }
            }),
        })
        .collect();
    (Mesh { tetrahedra, nodes }, Model { faces, vertices })
}

#[test]
fn hall_sized_grid_verifies_in_under_two_seconds() {
    // 30³ cells × 6 = 162,000 tetrahedra, the size of a hall mesh.
    let (mesh, scene) = grid(30, 30.0);
    let start = Instant::now();
    let r = verify_mesh(&mesh, &scene, &VolumeIds::default());
    let elapsed = start.elapsed();
    let profile = if cfg!(debug_assertions) {
        "debug"
    } else {
        "release"
    };
    println!(
        "grid: {} tetrahedra verified in {:.3} s ({profile})",
        r.tetrahedra,
        elapsed.as_secs_f64()
    );
    assert!(r.passed(), "{}", summary("grid", &r));
    assert_eq!(r.tetrahedra, 162_000);
    assert!((r.volume_by_id[&1] - 27_000.0).abs() < 1e-6);
    assert!(elapsed.as_secs_f64() < 2.0, "{elapsed:?} ({profile})");
}

#[test]
fn broken_hall_verifies_in_under_two_seconds() {
    let (mesh, scene) = load(
        "meshes/broken_hall/tetramesh.mbin",
        "meshes/broken_hall/model.cbin",
    );
    let start = Instant::now();
    let r = verify_mesh(&mesh, &scene, &room_0());
    let elapsed = start.elapsed();
    let profile = if cfg!(debug_assertions) {
        "debug"
    } else {
        "release"
    };
    println!(
        "broken hall: {} tetrahedra verified in {:.3} s ({profile})",
        r.tetrahedra,
        elapsed.as_secs_f64()
    );
    assert!(elapsed.as_secs_f64() < 2.0, "{elapsed:?} ({profile})");
}

// ---------------------------------------------------------------------------------------------
// A drawn fitting zone's triangles in the `.cbin`.

/// Upstream's tutorial-1 mesh and scene, the scene with a drawn box zone's 12 triangles appended
/// as upstream's GUI and `config_xml::scene_mesh` append them (`Objet3D_maillage.cpp:783-816`):
/// three vertices of their own each, `idMat` 0, `idRs` -1, `idEn` 2. No marker names them.
fn tutorial1_with_a_drawn_box() -> (Mesh, Model) {
    let (mesh, mut scene) = tutorial1();
    let frame = simpa_core::config_xml::GlFrame::of_vertices(
        scene.vertices.iter().map(|v| [v.x, v.y, v.z]),
    );
    let triangles = simpa_core::config_xml::upstream_box_triangles(
        frame.as_ref(),
        [1.0, 1.0, 0.5],
        [2.0, 2.5, 1.5],
    );
    assert_eq!(triangles.len(), 12);
    for t in triangles {
        let base = scene.vertices.len() as u32;
        for [x, y, z] in t {
            scene.vertices.push(cbin::Vertex { x, y, z });
        }
        scene.faces.push(cbin::Face {
            a: base,
            b: base + 1,
            c: base + 2,
            id_mat: 0,
            id_rs: -1,
            id_en: 2,
        });
    }
    (mesh, scene)
}

/// The ids of tutorial 1's mesh with the box declared: the room stays upstream's 1.
fn with_box_zone() -> VolumeIds {
    VolumeIds {
        room: 1,
        fittings: vec![2],
    }
}

/// A drawn box zone's triangles need no marker; each thing that makes them one, undone, makes
/// them 12 uncovered scene faces again, and a room face left without a marker is still uncovered.
#[test]
fn a_drawn_zones_triangles_need_no_marker_and_nothing_else_does() {
    let (mesh, scene) = tutorial1_with_a_drawn_box();
    let r = verify_mesh(&mesh, &scene, &with_box_zone());
    println!("{}", summary("tutorial 1 with a drawn box", &r));
    assert!(r.passed(), "{}", summary("drawn box", &r));
    assert_eq!((r.scene_faces, r.drawn_zone_faces), (24, 12));

    // Each partner: the scene edited, and the triangles are scene faces no marker names.
    let uncovered_box = |label: &str, scene: &Model, ids: &VolumeIds| {
        let r = verify_mesh(&mesh, scene, ids);
        println!("{}", summary(label, &r));
        assert_eq!(codes(&r), ["uncovered_scene_faces"], "{label}");
        assert_eq!(r.drawn_zone_faces, 0, "{label}");
        r.uncovered_scene_faces
    };
    // The zone not declared.
    assert_eq!(
        uncovered_box("no fitting declared", &scene, &upstream()),
        12
    );
    let edited = |edit: &dyn Fn(&mut Model)| {
        let mut s = scene.clone();
        edit(&mut s);
        s
    };
    let ids = with_box_zone();
    for (label, s) in [
        (
            "one triangle with a material",
            edited(&|s| s.faces[17].id_mat = 5),
        ),
        (
            "one triangle on a receiver",
            edited(&|s| s.faces[17].id_rs = 0),
        ),
        (
            "one triangle of another zone",
            edited(&|s| s.faces[17].id_en = 3),
        ),
        (
            "one vertex off the box's corners",
            edited(&|s| s.vertices[s.faces[17].a as usize].x += 0.25),
        ),
        (
            "one vertex shared with the room",
            edited(&|s| s.faces[17].a = s.faces[0].a),
        ),
        (
            "the last triangle moved onto the first's side",
            edited(&|s| {
                let (from, to) = (s.faces[12], s.faces[23]);
                for (f, t) in [(from.a, to.a), (from.b, to.b), (from.c, to.c)] {
                    s.vertices[t as usize] = s.vertices[f as usize];
                }
            }),
        ),
    ] {
        assert_eq!(uncovered_box(label, &s, &ids), 12, "{label}");
    }
    // The triangles in another order within the box are still the box.
    let reordered = edited(&|s| s.faces.swap(12, 23));
    assert!(verify_mesh(&mesh, &reordered, &ids).passed());
    // A triangle left out: the last 12 faces now hold a room face, which is covered, and the 11
    // triangles are not.
    let eleven = edited(&|s| {
        s.faces.pop();
    });
    assert_eq!(uncovered_box("11 triangles", &eleven, &ids), 11);
    // A face after the box: the box is no longer at the end, and that face is uncovered too.
    let after = edited(&|s| {
        let f = s.faces[0];
        s.faces.push(f);
    });
    let r = verify_mesh(&mesh, &after, &ids);
    assert_eq!(codes(&r), ["uncovered_scene_faces"]);
    assert_eq!((r.uncovered_scene_faces, r.drawn_zone_faces), (13, 0));
    // A room face no marker names is uncovered with the box beside it.
    let (mut unmarked, _) = tutorial1_with_a_drawn_box();
    for t in &mut unmarked.tetrahedra {
        for f in &mut t.faces {
            if f.marker == 3 {
                f.marker = 4;
            }
        }
    }
    let r = verify_mesh(&unmarked, &scene, &ids);
    println!("{}", summary("room face 3 unmarked", &r));
    assert!(
        r.codes.contains(&"uncovered_scene_faces".to_string()),
        "{:?}",
        r.codes
    );
    assert_eq!(r.uncovered_scene_faces_first, [3]);
    assert_eq!(r.drawn_zone_faces, 12);
}

/// Upstream's own run folders of tutorial 3 carry the box's 12 triangles as faces 88 to 99, and
/// they are what `drawn_zone_faces` takes for a drawn zone, with the box's id declared.
#[test]
fn upstreams_tutorial3_scene_carries_its_box_as_a_drawn_zone() {
    use simpa_core::geometry::import::zip::Archive;
    let proj =
        paths::upstream_file(r"src/isimpa/resources/doc/tutorial/tutorial 3/tutorial_3.proj");
    let bytes = std::fs::read(&proj).unwrap();
    let archive = Archive::parse(&bytes).unwrap();
    let mut runs = 0;
    for e in archive.entries() {
        if !(e.name.contains("/report/") && e.name.ends_with("/mesh.cbin")) {
            continue;
        }
        runs += 1;
        let scene = cbin::read(&archive.read(&e.name).unwrap()).unwrap();
        let ids = VolumeIds::tetgen(vec![1930, 2083]);
        let drawn = simpa_core::mesh::verify::drawn_zone_faces(&scene, &ids);
        let which: Vec<usize> = (0..drawn.len()).filter(|&i| drawn[i]).collect();
        assert_eq!(which, (88..100).collect::<Vec<_>>(), "{}", e.name);
        // Without the box's id declared, none.
        let only_model = VolumeIds::tetgen(vec![1930]);
        let drawn = simpa_core::mesh::verify::drawn_zone_faces(&scene, &only_model);
        assert!(drawn.iter().all(|d| !d), "{}", e.name);
    }
    assert_eq!(runs, 3);
}

// ---------------------------------------------------------------------------------------------
// The room's parts, and the region volume check.

/// TetGen numbers the room's parts up by one from the first (`tetgen.cxx:22403-22436`), so a part
/// past a gap is no id it gives.
#[test]
fn the_rooms_parts_are_numbered_without_a_gap() {
    let (mesh, scene) = tutorial1();
    assert!(verify_mesh(&mesh, &scene, &upstream()).passed());
    // Half the tetrahedra made a second part, 2: TetGen's numbering, so no unknown id.
    let mut two = mesh.clone();
    for (i, t) in two.tetrahedra.iter_mut().enumerate() {
        if i % 2 == 0 {
            t.id_volume = 2;
        }
    }
    let r = verify_mesh(&two, &scene, &upstream());
    assert_eq!(r.unknown_volume_ids, 0, "{}", summary("parts 1 and 2", &r));
    // Says no: numbered 3, past the missing 2.
    let mut gap = mesh.clone();
    for (i, t) in gap.tetrahedra.iter_mut().enumerate() {
        if i % 2 == 0 {
            t.id_volume = 3;
        }
    }
    let r = verify_mesh(&gap, &scene, &upstream());
    assert_eq!(
        r.unknown_volume_ids,
        mesh.tetrahedra.len().div_ceil(2),
        "{}",
        summary("parts 1 and 3", &r)
    );
    assert_eq!(codes(&r), ["unknown_volume_ids"]);
}

/// Three tetrahedra round the edge from the origin to (0, 0, 1), each a cell of its own: the
/// first (the zone, id 2), the second and the third (the room's two parts, 3 and 4). Facets: the
/// 8 outer ones, wound outward, then the first's wall with the second (8) and the second's with
/// the third (9); a facet's marker is its position.
struct Wedges {
    nodes: Vec<[f64; 3]>,
    facets: Vec<[u32; 3]>,
    tets: [[i32; 4]; 3],
}

fn wedges() -> Wedges {
    let nodes = vec![
        [0.0, 0.0, 0.0],
        [0.0, 0.0, 1.0],
        [0.0, 1.0, 0.5],
        [1.0, 0.0, 0.5],
        [-1.0, 0.0, 0.5],
        [0.0, -1.0, 0.5],
    ];
    let tets = [[0, 1, 3, 2], [0, 1, 2, 4], [0, 1, 4, 5]];
    let outward = |f: [u32; 3], away_from: usize| -> [u32; 3] {
        let p = |i: u32| nodes[i as usize];
        let [a, b, c] = f.map(p);
        let d = nodes[away_from];
        let u = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
        let v = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
        let n = [
            u[1] * v[2] - u[2] * v[1],
            u[2] * v[0] - u[0] * v[2],
            u[0] * v[1] - u[1] * v[0],
        ];
        let w = [d[0] - a[0], d[1] - a[1], d[2] - a[2]];
        if n[0] * w[0] + n[1] * w[1] + n[2] * w[2] > 0.0 {
            [f[0], f[2], f[1]]
        } else {
            f
        }
    };
    let facets = vec![
        outward([0, 1, 3], 2),
        outward([0, 3, 2], 1),
        outward([1, 3, 2], 0),
        outward([0, 2, 4], 1),
        outward([1, 2, 4], 0),
        outward([0, 4, 5], 1),
        outward([1, 4, 5], 0),
        outward([0, 1, 5], 4),
        [0, 1, 2],
        [0, 1, 4],
    ];
    Wedges {
        nodes,
        facets,
        tets,
    }
}

impl Wedges {
    fn check(&self) -> simpa_core::geometry::check::CheckReport {
        let group = simpa_core::schema::GroupId::from_u128(0);
        let g = simpa_core::schema::Geometry {
            vertices: self
                .nodes
                .iter()
                .map(|&v| simpa_core::schema::Vec3::from(v))
                .collect(),
            faces: self
                .facets
                .iter()
                .map(|&vertices| simpa_core::schema::Face { vertices, group })
                .collect(),
        };
        let r = simpa_core::geometry::check::check(&g);
        assert!(r.is_ok(), "{:?}", r.reasons);
        assert_eq!(r.cells.len(), 3);
        r
    }

    /// The three tetrahedra with these ids (only the corners and ids matter to the region check).
    fn mesh(&self, ids: [i32; 3]) -> Mesh {
        let face = TetraFace {
            vertices: [0, 0, 0],
            marker: -1,
            neighbor: -2,
        };
        Mesh {
            nodes: self.nodes.iter().map(|p| p.map(|c| c as f32)).collect(),
            tetrahedra: (0..3)
                .map(|t| Tetrahedron {
                    vertices: self.tets[t],
                    id_volume: ids[t],
                    faces: [face; 4],
                })
                .collect(),
        }
    }
}

/// The zone of the wedges: id 2, seeded at `seed`, its faces the facets `faces`.
fn zone(seed: [f32; 3], faces: Vec<u32>) -> simpa_core::mesh::FittingRegion {
    simpa_core::mesh::FittingRegion {
        zone: "zone".to_string(),
        solver_id: 2,
        seed,
        box_centre: None,
        box_volume_m3: None,
        faces,
    }
}

/// Every region fills one cell with its volume, each fitting's id is on its zone's cell, and each
/// of the four region codes says no to the mesh made to fail it.
#[test]
fn regions_are_held_to_the_cells_of_the_meshed_geometry() {
    use simpa_core::mesh::verify::{
        Expectations, Reference, seed_inside, verify_mesh_with, zone_cell,
    };
    let w = wedges();
    let check = w.check();
    let markers: Vec<u32> = (0..w.facets.len() as u32).collect();
    let cell_of = |t: usize| {
        let reference = Reference {
            vertices: &w.nodes,
            facets: &w.facets,
            markers: &markers,
            check: &check,
            fittings: &[],
        };
        let c = [0, 1, 2, 3].map(|k| w.nodes[w.tets[t][k] as usize]);
        let centroid = [0, 1, 2].map(|k| ((c[0][k] + c[1][k] + c[2][k] + c[3][k]) / 4.0) as f32);
        zone_cell(&reference, &zone(centroid, vec![]), 1e-9)
            .zone_cell
            .unwrap()
    };
    let (first, second) = (cell_of(0), cell_of(1));
    let scene = cbin::Model {
        vertices: Vec::new(),
        faces: Vec::new(),
    };
    let ids = VolumeIds::tetgen(vec![2]);
    // The seed on the wall between the first and the second, the zone's own face (facet 8): the
    // first is closed by it and the outer shell, the second is not (the wall behind it has the
    // third cell behind it), so the zone is the first.
    let on_wall = [0.0f32, 1.0 / 3.0, 0.5];
    let fittings = [zone(on_wall, vec![8])];
    let run = |ids_of: [i32; 3], fittings: &[simpa_core::mesh::FittingRegion]| {
        let reference = Reference {
            vertices: &w.nodes,
            facets: &w.facets,
            markers: &markers,
            check: &check,
            fittings,
        };
        verify_mesh_with(
            &w.mesh(ids_of),
            &scene,
            &ids,
            &Expectations {
                unmeshed_faces: None,
                reference: Some(reference),
            },
        )
    };
    let region_codes = |r: &VerifyReport| -> Vec<String> {
        r.codes
            .iter()
            .filter(|c| {
                [
                    "region_volume_mismatch",
                    "unmeshed_cells",
                    "fitting_region_misplaced",
                    "fitting_seed_ambiguous",
                    "unknown_volume_ids",
                ]
                .contains(&c.as_str())
            })
            .cloned()
            .collect()
    };
    let ok = run([2, 3, 4], &fittings);
    assert!(region_codes(&ok).is_empty(), "{:#?}", ok.regions);
    assert!(ok.regions_checked);
    assert_eq!(ok.regions.len(), 3);
    let zone_region = ok.regions.iter().find(|r| r.id == 2).unwrap();
    assert_eq!(
        (zone_region.cell, zone_region.zone_cell),
        (Some(first), Some(first))
    );
    assert_eq!(zone_region.seed_on_facets, [8]);
    for r in &ok.regions {
        assert!((r.volume_m3 - 1.0 / 6.0).abs() < 1e-12, "{r:?}");
    }

    // Says no: the zone's id on the second (TetGen's other choice of side), the room's first part
    // on the first.
    let r = run([3, 2, 4], &fittings);
    assert_eq!(
        region_codes(&r),
        ["fitting_region_misplaced"],
        "{:#?}",
        r.regions
    );
    // Says no: the zone's faces not given, so neither side is closed by them.
    let r = run([2, 3, 4], &[zone(on_wall, vec![])]);
    assert_eq!(
        region_codes(&r),
        ["fitting_seed_ambiguous"],
        "{:#?}",
        r.regions
    );
    // Says no: the second and the third one region, so it is not the second's volume and the
    // third cell is meshed by none.
    let r = run([2, 3, 3], &fittings);
    assert_eq!(
        region_codes(&r),
        ["region_volume_mismatch", "unmeshed_cells"],
        "{:#?}",
        r.regions
    );
    // Says no: no tetrahedron carries the zone's id.
    let r = run([3, 3, 3], &fittings);
    assert!(
        region_codes(&r).contains(&"fitting_region_misplaced".to_string()),
        "{:#?}",
        r.regions
    );
    // Says no: a node moved, so the first region's volume is not its cell's.
    let mut moved = w.mesh([2, 3, 4]);
    moved.nodes[3][0] = 1.5;
    let reference = Reference {
        vertices: &w.nodes,
        facets: &w.facets,
        markers: &markers,
        check: &check,
        fittings: &fittings,
    };
    let r = verify_mesh_with(
        &moved,
        &scene,
        &ids,
        &Expectations {
            unmeshed_faces: None,
            reference: Some(reference),
        },
    );
    assert!(r.region_volume_mismatch >= 1, "{:#?}", r.regions);

    // The mesher's move: the seed off the wall, strictly inside the first cell.
    let inside = seed_inside(&reference, &fittings[0], 1e-9).expect("a point inside the zone");
    let moved_zone = zone_cell(&reference, &zone(inside, vec![8]), 1e-9);
    assert!(moved_zone.on_facets.is_empty(), "{moved_zone:?}");
    assert_eq!(moved_zone.candidates, [first]);
    assert_ne!(first, second);
    // Nothing to move a seed already inside.
    assert!(seed_inside(&reference, &zone(inside, vec![8]), 1e-9).is_none());
}

/// The region volume tolerance is `2 · A · 16 · 2⁻²⁴ · R` (`mesh/verify/regions.rs`), from the
/// scene's own extent `R` and the cell's boundary area `A`: a region half a tolerance off its
/// cell's volume passes, one two tolerances off fails. The test above has an empty scene, so its
/// tolerance is 0 and it cannot see a wrong scale; this one can.
#[test]
fn the_region_volume_tolerance_says_yes_at_half_and_no_at_twice() {
    use simpa_core::mesh::verify::{Expectations, Reference, verify_mesh_with};
    let w = wedges();
    let check = w.check();
    let markers: Vec<u32> = (0..w.facets.len() as u32).collect();
    // The wedges' nodes as the scene: R = 1, so the distance tolerance is 16 · 2⁻²⁴ m.
    let scene = cbin::Model {
        vertices: w
            .nodes
            .iter()
            .map(|p| cbin::Vertex {
                x: p[0] as f32,
                y: p[1] as f32,
                z: p[2] as f32,
            })
            .collect(),
        faces: Vec::new(),
    };
    let distance = 16.0 * 2f64.powi(-24);
    // The first cell is the first tetrahedron: its boundary is its four faces.
    let area = |a: usize, b: usize, c: usize| {
        let (p, q, r) = (w.nodes[a], w.nodes[b], w.nodes[c]);
        let u = [q[0] - p[0], q[1] - p[1], q[2] - p[2]];
        let v = [r[0] - p[0], r[1] - p[1], r[2] - p[2]];
        let n = [
            u[1] * v[2] - u[2] * v[1],
            u[2] * v[0] - u[0] * v[2],
            u[0] * v[1] - u[1] * v[0],
        ];
        0.5 * (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt()
    };
    let [a, b, c, d] = w.tets[0].map(|i| i as usize);
    let boundary = area(a, b, c) + area(a, b, d) + area(a, c, d) + area(b, c, d);
    let tolerance = 2.0 * boundary * distance;
    // The room's three parts, 1 to 3; no fitting.
    let ids = VolumeIds::tetgen(Vec::new());
    let run = |mesh: &Mesh| {
        let reference = Reference {
            vertices: &w.nodes,
            facets: &w.facets,
            markers: &markers,
            check: &check,
            fittings: &[],
        };
        verify_mesh_with(
            mesh,
            &scene,
            &ids,
            &Expectations {
                unmeshed_faces: None,
                reference: Some(reference),
            },
        )
    };
    let ok = run(&w.mesh([1, 2, 3]));
    assert!(ok.regions_checked);
    assert_eq!(ok.region_volume_mismatch, 0, "{:#?}", ok.regions);
    let first = ok.regions.iter().find(|r| r.id == 1).unwrap();
    assert!(
        (first.tolerance_m3 - tolerance).abs() <= 1e-12 * tolerance,
        "tolerance {:e} m³, expected 2 · {boundary} m² · {distance:e} m = {tolerance:e} m³",
        first.tolerance_m3
    );
    // Node 3 belongs to the first tetrahedron alone, whose volume is its x / 6: moved along x by
    // 6 · k · tolerance, the region is k tolerances off its cell.
    let off_by = |k: f64| {
        let mut mesh = w.mesh([1, 2, 3]);
        let x = w.nodes[3][0] + 6.0 * k * tolerance;
        mesh.nodes[3][0] = x as f32;
        let r = run(&mesh);
        let first = r.regions.iter().find(|r| r.id == 1).unwrap().clone();
        let off = (first.volume_m3 - first.cell_volume_m3.unwrap()).abs() / tolerance;
        (r.region_volume_mismatch, off)
    };
    let (mismatch, off) = off_by(0.5);
    assert!((0.4..0.6).contains(&off), "{off}");
    assert_eq!(mismatch, 0, "half a tolerance off ({off:.3}) passes");
    let (mismatch, off) = off_by(2.0);
    assert!((1.9..2.1).contains(&off), "{off}");
    assert_eq!(mismatch, 1, "two tolerances off ({off:.3}) fails");
}

/// A folder's regions are held to the cells of the geometry it holds (decision 15): its `.poly`
/// when it has one, else its `.cbin`. Without that, any run of room ids without a gap passed.
/// Says no: the cube's tetrahedra split between two ids; a folder whose `.poly` the geometry
/// check refuses, unless its `mesh.json` proves the mesher checked this `.mbin`'s regions; a
/// manifest proving nothing.
#[test]
fn a_folders_regions_are_held_to_its_own_geometry() {
    let dir = cube_folder("regions_cbin", None);
    let r = verify_dir(&dir, &upstream()).unwrap();
    assert!(r.passed(), "{r:?}");
    assert_eq!(r.regions.reference.as_deref(), Some("mesh.cbin"));
    let m = r.mesh.as_ref().unwrap();
    assert!(m.regions_checked);
    assert_eq!(m.regions.len(), 1, "{:?}", m.regions);

    // Says no: half the cube's tetrahedra given the room's next id.
    let dir = cube_folder("regions_split", None);
    let path = dir.join("tetramesh.mbin");
    let mut mesh = mbin::read_file(&path).unwrap();
    let half = mesh.tetrahedra.len() / 2;
    for t in &mut mesh.tetrahedra[..half] {
        t.id_volume += 1;
    }
    mbin::write_file(&mesh, &path).unwrap();
    let r = verify_dir(&dir, &upstream()).unwrap();
    assert_eq!(r.codes, ["region_volume_mismatch"], "{r:?}");

    // Says no: a .poly the check refuses (two facets, open) takes precedence over the .cbin, and
    // gives no cells.
    let open = |dir: &Path| {
        use simpa_core::formats::poly;
        let face = |vertices: [u32; 3], face_index: u32| poly::Face {
            vertices,
            face_index,
        };
        let model = poly::Model {
            save_face_index: true,
            user_defined_faces: Vec::new(),
            model_faces: vec![face([0, 2, 1], 0), face([0, 1, 3], 1)],
            model_vertices: vec![
                [0.0, 0.0, 0.0],
                [1.0, 0.0, 0.0],
                [0.0, 1.0, 0.0],
                [0.0, 0.0, 1.0],
            ],
            model_regions: Vec::new(),
        };
        poly::write_file(&model, &dir.join("scene_mesh.poly")).unwrap();
    };
    let dir = cube_folder("regions_unchecked", None);
    open(&dir);
    let r = verify_dir(&dir, &upstream()).unwrap();
    assert_eq!(r.codes, ["regions_unchecked"], "{r:?}");
    assert_eq!(r.regions.reference, None);
    assert!(
        r.regions
            .unchecked
            .as_deref()
            .unwrap()
            .contains("scene_mesh.poly"),
        "{:?}",
        r.regions
    );
    // The mesher's manifest of this .mbin, regions checked: proven.
    let proof = |sha: &str, checked: bool| {
        format!(
            "{{\"status\": \"OK\", \"files\": {{\"mbin\": \"{sha}\"}}, \"verify\": \
             {{\"regions_checked\": {checked}}}}}"
        )
    };
    let dir = cube_folder("regions_proven", Some(&proof(CUBE_MBIN_SHA256, true)));
    open(&dir);
    let r = verify_dir(&dir, &upstream()).unwrap();
    assert!(r.passed(), "{r:?}");
    assert!(r.regions.proven_by_manifest);
    // Says no: a manifest whose regions were not checked.
    let dir = cube_folder("regions_not_proven", Some(&proof(CUBE_MBIN_SHA256, false)));
    open(&dir);
    assert_eq!(
        verify_dir(&dir, &upstream()).unwrap().codes,
        ["regions_unchecked"]
    );
    // Says no: the mesher's OK record, regions checked, of another .mbin (one hex digit off). The
    // folder's hash check names it, and it proves nothing about this .mbin's regions.
    let mut other = CUBE_MBIN_SHA256.to_string();
    let last = if other.ends_with('0') { "1" } else { "0" };
    other.replace_range(other.len() - 1.., last);
    let dir = cube_folder("regions_other_mbin", Some(&proof(&other, true)));
    open(&dir);
    let r = verify_dir(&dir, &upstream()).unwrap();
    assert!(!r.regions.proven_by_manifest, "{:?}", r.regions);
    assert!(
        r.codes.contains(&"regions_unchecked".to_string())
            && r.codes.contains(&"manifest_mismatch".to_string()),
        "{:?}",
        r.codes
    );
    // Says no: a record that is not OK.
    let dir = cube_folder(
        "regions_not_ok",
        Some(&proof(CUBE_MBIN_SHA256, true).replace("\"OK\"", "\"FAIL\"")),
    );
    open(&dir);
    let r = verify_dir(&dir, &upstream()).unwrap();
    assert!(!r.regions.proven_by_manifest, "{:?}", r.regions);
    assert!(
        r.codes.contains(&"regions_unchecked".to_string()),
        "{:?}",
        r.codes
    );
}
