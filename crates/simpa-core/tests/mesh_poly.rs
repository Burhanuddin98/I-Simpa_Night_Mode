//! `mesh::mesh_poly` with the real `tetgen.exe`: the survey's self-intersecting cube
//! (`target/scr-libif-backup/mkpolybad.py`) is refused with its skipped facets named, and the
//! same cube without the piercing facet meshes.

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

#[test]
fn a_self_intersecting_poly_is_refused_with_its_facets_named() {
    let dir = scratch("poly-bad");
    let input = dir.join("tg_bad.poly");
    poly::write_file(&cube(true), &input).unwrap();
    let out = dir.join("mesh");
    let m = mesh(&input, &out);
    println!("{}", serde_json::to_string_pretty(&m).unwrap());

    assert_eq!(m.status, MeshStatus::Fail);
    for code in [
        codes::TETGEN_EXIT_NONZERO,
        codes::TETGEN_SKIPPED_FACETS,
        codes::NEIGH_MISSING,
    ] {
        assert!(m.codes.iter().any(|c| c == code), "{code} in {:?}", m.codes);
    }
    assert_eq!(m.tetgen.as_ref().unwrap().exit_code, Some(3));
    assert_eq!(m.mesh_input_hash, None);
    let markers: Vec<i64> = m.skipped_facets.iter().map(|s| s.marker).collect();
    assert_eq!(markers, [8, 9, 12]);
    assert_eq!(m.skipped_rows, 3);
    assert!(
        m.skipped_facets
            .iter()
            .all(|s| s.scene_face == Some(s.marker as u32))
    );
    assert!(!out.join("tetramesh.mbin").exists());

    // The tetgen -d follow-up names the pairs.
    let d = m
        .diagnosis
        .as_ref()
        .expect("a diagnosis after skipped facets");
    assert_eq!(d.call.argv, ["-d", "scene_mesh.poly"]);
    assert_eq!(d.call.exit_code, Some(3));
    assert_eq!(d.skipped_markers, [8, 9, 12]);
    let pairs: Vec<(Vec<u32>, Vec<u32>)> = d
        .intersections
        .iter()
        .map(|i| {
            (
                i.first.markers.clone(),
                i.second
                    .as_ref()
                    .map(|s| s.markers.clone())
                    .unwrap_or_default(),
            )
        })
        .collect();
    assert_eq!(
        pairs,
        [
            (vec![12], vec![8]),
            (vec![12], vec![9]),
            (vec![8, 9], vec![12])
        ]
    );
    assert!(out.join("diag/scene_mesh_skipped.face").is_file());
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
