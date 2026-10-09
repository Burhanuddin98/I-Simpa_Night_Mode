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

/// The hall written to a file and imported as the app's Repair hands it back (unit m, up z): the
/// same coordinates bit for bit (no unit scale or axis turn crept in), and the same groups, by
/// name, one each.
#[test]
fn a_written_file_imports_in_metres_with_its_groups() {
    let path = repo("testdata/elmia_corrected.ply");
    let p = import::import_file(&path, &metres())
        .expect("the corrected hall imports")
        .to_project("hall");
    let dir = std::env::temp_dir().join(format!("simpa-export-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("hall_repaired.obj");
    std::fs::write(&file, obj_text(&p, &[])).unwrap();
    let back = import::import_file(&file, &metres())
        .expect("the written file imports")
        .to_project("hall_repaired");
    std::fs::remove_dir_all(&dir).unwrap();
    assert_eq!(back.geometry.vertices.len(), p.geometry.vertices.len());
    for (a, b) in p.geometry.vertices.iter().zip(&back.geometry.vertices) {
        let (a, b) = (a.to_array(), b.to_array());
        assert!(
            (0..3).all(|k| a[k].to_bits() == b[k].to_bits()),
            "metres, Z up: {a:?} vs {b:?}"
        );
    }
    let used = |q: &Project| {
        let mut names: Vec<String> = Vec::new();
        for f in &q.geometry.faces {
            let n = &q.group(f.group).unwrap().name;
            if !names.contains(n) {
                names.push(n.clone());
            }
        }
        names
    };
    assert_eq!(used(&back), used(&p), "the same groups, in the same order");
    assert_eq!(back.surface_groups.len(), used(&p).len(), "one group each");
}

/// Names that differ only in what an OBJ `g` line cannot hold (runs of spaces, a tab, the ends,
/// a `#`) stay groups of their own on re-import, instead of merging into one.
#[test]
fn group_names_differing_only_in_whitespace_stay_distinct() {
    let path = repo("testdata/elmia_corrected.ply");
    let mut p = import::import_file(&path, &metres())
        .expect("the corrected hall imports")
        .to_project("hall");
    assert!(p.surface_groups.len() >= 5, "{}", p.surface_groups.len());
    let names = [
        "Stage floor",
        "Stage  floor",
        "Stage\tfloor ",
        "Stage floor #",
        "   ",
    ];
    for (g, n) in p.surface_groups.iter_mut().zip(names) {
        g.name = n.to_string();
    }
    let back = round_trip(&p);
    let ids: Vec<_> = p.surface_groups.iter().take(5).map(|g| g.id).collect();
    let mut seen = Vec::new();
    for (i, f) in p.geometry.faces.iter().enumerate() {
        if let Some(k) = ids.iter().position(|id| *id == f.group) {
            let read = back.group_names[back.face_groups[i] as usize].clone();
            if !seen.iter().any(|(j, _)| *j == k) {
                seen.push((k, read));
            } else {
                assert_eq!(
                    seen.iter().find(|(j, _)| *j == k).unwrap().1,
                    read,
                    "face {i}"
                );
            }
        }
    }
    assert_eq!(seen.len(), 5, "every renamed group has faces: {seen:?}");
    let mut distinct: Vec<&String> = seen.iter().map(|(_, n)| n).collect();
    distinct.sort();
    distinct.dedup();
    assert_eq!(distinct.len(), 5, "five groups, five names: {seen:?}");
    let mut read: Vec<&str> = seen.iter().map(|(_, n)| n.as_str()).collect();
    read.sort();
    assert_eq!(
        read,
        [
            "Stage floor",
            "Stage floor (2)",
            "Stage floor (3)",
            "Stage floor (4)",
            "default"
        ]
    );
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
