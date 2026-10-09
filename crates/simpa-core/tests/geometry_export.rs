//! `core::geometry::export`: the OBJ the app's Repair writes (parity G8) reads back as the same
//! model: every vertex bit for bit, every face in order with its winding, every face's group.

use std::path::PathBuf;

use simpa_core::geometry::export::obj_text;
use simpa_core::geometry::import::{self, ImportOptions, ImportedModel, Unit, Up};
use simpa_core::geometry::repair::{self, RepairOptions};
use simpa_core::schema::Project;

fn repo(rel: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join(rel)
}

fn metres() -> ImportOptions {
    ImportOptions::new(Unit::Metre, Up::Z)
}

/// Writes `p` as OBJ and reads it back.
fn round_trip(p: &Project) -> ImportedModel {
    let text = obj_text(p, &["round trip".to_string()]);
    import::read_obj(text.as_bytes(), &metres()).expect("the written OBJ reads back")
}

fn assert_same(p: &Project, back: &ImportedModel) {
    let g = &p.geometry;
    assert_eq!(back.vertices.len(), g.vertices.len());
    for (a, b) in g.vertices.iter().zip(&back.vertices) {
        let (a, b) = (a.to_array(), b.to_array());
        for k in 0..3 {
            assert_eq!(a[k].to_bits(), b[k].to_bits(), "{a:?} vs {b:?}");
        }
    }
    let faces: Vec<[u32; 3]> = g.faces.iter().map(|f| f.vertices).collect();
    assert_eq!(back.triangles, faces, "faces in order, winding kept");
    for (i, f) in g.faces.iter().enumerate() {
        let want = &p.group(f.group).expect("a face's group exists").name;
        assert_eq!(
            &back.group_names[back.face_groups[i] as usize], want,
            "face {i}"
        );
    }
}

#[test]
fn the_corrected_hall_reads_back_as_written() {
    let path = repo("testdata/elmia_corrected.ply");
    let model = import::import_file(&path, &metres()).expect("the corrected hall imports");
    let p = model.to_project("elmia_corrected");
    assert!(p.geometry.faces.len() > 1000);
    assert_same(&p, &round_trip(&p));
}

#[test]
fn a_repaired_geometry_keeps_its_flips_and_loses_its_removed_faces() {
    let path = repo("testdata/elmia_corrected.ply");
    let mut p = import::import_file(&path, &metres())
        .expect("the corrected hall imports")
        .to_project("hall");
    // Damage it the way repair undoes: one face flipped, one face doubled.
    let f0 = p.geometry.faces[0];
    p.geometry.faces[3].vertices.swap(1, 2);
    p.geometry.faces.push(f0);
    let outcome = repair::repair(&p.geometry, &RepairOptions::default()).expect("repair runs");
    assert!(outcome.changes.len() >= 2, "{:?}", outcome.changes);
    let mut fixed = p.clone();
    fixed.geometry = outcome.geometry.clone();
    assert_eq!(fixed.geometry.faces.len(), p.geometry.faces.len() - 1);
    assert_same(&fixed, &round_trip(&fixed));
}

#[test]
fn control_characters_in_a_group_name_become_spaces() {
    let path = repo("testdata/elmia_corrected.ply");
    let mut p = import::import_file(&path, &metres())
        .expect("the corrected hall imports")
        .to_project("hall");
    p.surface_groups[0].name = "Stage\nfloor\tleft".into();
    let text = obj_text(&p, &[]);
    assert!(text.contains("\ng Stage floor left\n"));
    assert!(text.starts_with("# metres, Z up\n"));
}
