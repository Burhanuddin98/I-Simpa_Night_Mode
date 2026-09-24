//! `mesh::mesh_poly` with the real `tetgen.exe` (TetGen 1.5.0, the M1 build): the survey's
//! self-intersecting cube (`target/scr-libif-backup/mkpolybad.py`) is refused with its
//! intersecting facets named, and the same cube without the piercing facet meshes.

#[path = "mesh_support.rs"]
mod support;

use std::path::Path;

use simpa_core::formats::{mbin, poly};
use simpa_core::mesh::{MeshStatus, TetgenMesher, codes, mesh_poly, read_manifest};
use simpa_core::process::{CancelToken, Line};
use simpa_core::schema::MeshSettings;
use support::{invariants, scratch, tetgen_exe};

/// Upstream's 5 m cube as `CPoly::ExportPOLY` writes it (the survey's `tg_ok/scene_mesh.poly`),
/// and, with `pierce`, `mkpolybad.py`'s additions: nodes 9 (2.5, 2.5, 2.5), 10 (7.5, 2.5, 2.0)
/// and 11 (7.5, 2.5, 3.0), and facet 12 = (9, 10, 11), which pierces the x = 5 wall (facets 8
/// and 9).
fn cube(pierce: bool) -> poly::Model {
    let mut model_vertices = vec![
        [5.0, -0.0, 0.0],
        [0.0, -0.0, 0.0],
        [0.0, 5.0, 0.0],
        [5.0, 5.0, 0.0],
        [0.0, 5.0, 5.0],
        [5.0, 5.0, 5.0],
        [0.0, -0.0, 5.0],
        [5.0, -0.0, 5.0],
    ];
    let mut facets: Vec<[u32; 3]> = vec![
        [1, 2, 3],
        [1, 3, 4],
        [3, 5, 6],
        [3, 6, 4],
        [3, 7, 5],
        [3, 2, 7],
        [2, 1, 8],
        [7, 2, 8],
        [1, 4, 6],
        [8, 1, 6],
        [8, 6, 5],
        [7, 8, 5],
    ];
    if pierce {
        model_vertices.extend([[2.5, 2.5, 2.5], [7.5, 2.5, 2.0], [7.5, 2.5, 3.0]]);
        facets.push([9, 10, 11]);
    }
    poly::Model {
        save_face_index: true,
        user_defined_faces: Vec::new(),
        model_faces: facets
            .iter()
            .enumerate()
            .map(|(i, f)| poly::Face {
                vertices: f.map(|v| v - 1),
                face_index: i as u32,
            })
            .collect(),
        model_vertices,
        model_regions: Vec::new(),
    }
}

fn mesh(poly_path: &Path, out: &Path) -> simpa_core::mesh::MeshManifest {
    mesh_poly(
        poly_path,
        &MeshSettings::default(),
        out,
        &TetgenMesher::new(tetgen_exe()),
        &CancelToken::new(),
        &mut |_: &Line| {},
    )
    .unwrap()
}

/// TetGen 1.5.0, the mesher since decision 3, stops at the first self-intersection it meets (exit
/// 3, `A self-intersection was detected. Program stopped.`) and writes no `_skipped.face`: the
/// piercing facet's edge [9, 10] against facet #9, the wall's triangle (1, 4, 6), marker 8. The
/// `-d` follow-up exits 0 and names every intersecting facet, in its pairs and in the markers of
/// the `.1.face` it writes: 8, 9 and 12, each a scene face of this raw `.poly`. The control that
/// makes this say no is the next test: the same cube without facet 12 meshes, with no
/// `self_intersection`.
#[test]
fn a_self_intersecting_poly_is_refused_with_its_facets_named() {
    let dir = scratch("poly-bad");
    let input = dir.join("tg_bad.poly");
    poly::write_file(&cube(true), &input).unwrap();
    let out = dir.join("mesh");
    let m = mesh(&input, &out);
    println!("{}", serde_json::to_string_pretty(&m).unwrap());

    assert_eq!(m.status, MeshStatus::Fail);
    assert_eq!(
        m.codes,
        [
            codes::TETGEN_EXIT_NONZERO,
            codes::TETGEN_SELF_INTERSECTION,
            codes::TETGEN_OUTPUT_MISSING,
            codes::NEIGH_MISSING,
        ]
    );
    assert_eq!(m.tetgen.as_ref().unwrap().exit_code, Some(3));
    assert_eq!(m.mesh_input_hash, None);
    // No _skipped.face from 1.5.0.
    assert_eq!((m.skipped_rows, m.skipped_facets.len()), (0, 0));
    assert!(!out.join("scene_mesh_skipped.face").exists());
    assert!(!out.join("tetramesh.mbin").exists());

    // The stop names the pair: the segment [9, 10], an edge of facet 12, and facet #9 (marker 8).
    let si = m
        .self_intersection
        .as_ref()
        .expect("a self-intersection stop");
    let stop = si.stop.as_ref().expect("the stop names its pair");
    assert_eq!(stop.message, "Found a segment and a subface intersect.");
    assert_eq!(
        (stop.first.kind.as_str(), &stop.first.points),
        ("segment", &vec![9, 10])
    );
    assert_eq!(stop.first.markers, [12]);
    let second = stop.second.as_ref().unwrap();
    assert_eq!(
        (second.kind.as_str(), &second.points),
        ("facet", &vec![1, 4, 6])
    );
    assert_eq!(second.markers, [8]);

    // The tetgen -d follow-up exits 0 and names every intersecting facet.
    let d = m.diagnosis.as_ref().expect("a diagnosis after the stop");
    assert_eq!(d.call.argv, ["-d", "scene_mesh.poly"]);
    assert_eq!(d.call.exit_code, Some(0));
    assert_eq!(d.skipped_markers, Vec::<i64>::new());
    assert_eq!(d.face_markers, [8, 9, 12]);
    assert!(out.join("diag/scene_mesh.1.face").is_file());
    assert_eq!(
        simpa_core::mesh::marker_pairs(&d.intersections),
        [[8, 12], [9, 12]]
    );
    assert_eq!(si.pairs, [[8, 12], [9, 12]]);
    let named: Vec<(i64, Option<u32>)> =
        si.facets.iter().map(|f| (f.marker, f.scene_face)).collect();
    assert_eq!(named, [(8, Some(8)), (9, Some(9)), (12, Some(12))]);
    assert!(
        m.messages
            .iter()
            .any(|s| s.contains("2 pairs over 3 facets, markers [8, 9, 12]")),
        "{:?}",
        m.messages
    );
    assert_eq!(read_manifest(&out).unwrap(), m);
}

#[test]
fn the_same_cube_without_the_piercing_facet_meshes() {
    let dir = scratch("poly-ok");
    let input = dir.join("tg_ok.poly");
    poly::write_file(&cube(false), &input).unwrap();
    let out = dir.join("mesh");
    let m = mesh(&input, &out);
    assert!(m.is_ok(), "{m:#?}");
    assert_eq!(
        m.tetgen.as_ref().unwrap().argv,
        ["-pq5", "-A", "-n", "-Y", "scene_mesh.poly"]
    );
    assert!(m.diagnosis.is_none());
    assert!(m.self_intersection.is_none());
    let mesh = mbin::read_file(&out.join("tetramesh.mbin")).unwrap();
    assert_eq!(invariants(&mesh), Vec::<String>::new());
    let mut markers: Vec<i32> = mesh
        .tetrahedra
        .iter()
        .flat_map(|t| t.faces.iter())
        .map(|f| f.marker)
        .filter(|&m| m >= 0)
        .collect();
    markers.sort_unstable();
    markers.dedup();
    assert_eq!(markers, (0..12).collect::<Vec<i32>>());
    assert!(mesh.tetrahedra.iter().all(|t| t.id_volume == 0));
}

#[test]
fn a_poly_that_does_not_parse_is_input_invalid() {
    let dir = scratch("poly-garbage");
    let input = dir.join("garbage.poly");
    std::fs::write(&input, "not a poly\n").unwrap();
    let m = mesh(&input, &dir.join("mesh"));
    assert_eq!(m.codes, [codes::INPUT_INVALID]);
    assert!(m.tetgen.is_none(), "TetGen never ran");
}
