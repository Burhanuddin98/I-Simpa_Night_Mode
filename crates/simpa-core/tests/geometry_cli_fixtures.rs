//! The two fault fixtures the M4 gate drives through the CLI, built from the tutorial box with
//! the same recipes as the library tests (geometry_repair.rs m4d, geometry_check.rs m4e).
//! Regenerate with SIMPA_WRITE_FIXTURES=1; otherwise the committed files must match exactly.
use std::path::PathBuf;

use simpa_core::schema::{self, Project, Vec3};

fn repo(rel: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(rel)
}

fn the_box() -> Project {
    schema::load(&repo("tests/fixtures/rooms/tutorial1_box.simpa")).expect("tutorial1_box.simpa")
}

/// A duplicate vertex, a flipped face and a zero-area face: repair must log exactly these three.
fn three_faults() -> Project {
    let mut p = the_box();
    let clean = p.geometry.clone();
    let g = &mut p.geometry;
    let copied = g.faces[2].vertices[2];
    g.vertices.push(g.vertices[copied as usize]);
    g.faces[2].vertices[2] = 8;
    g.faces[7].vertices.swap(1, 2);
    let [a, b, _] = clean.faces[0].vertices;
    let [pa, pb] = [a, b].map(|v| clean.vertices[v as usize].to_array());
    g.vertices
        .push(Vec3::from([0, 1, 2].map(|k| 0.5 * (pa[k] + pb[k]))));
    let mut degenerate = clean.faces[0];
    degenerate.vertices = [a, 9, b];
    g.faces.push(degenerate);
    p.name = "box with three faults".to_string();
    p
}

/// The box and a copy of itself shifted half its size along every axis: they interpenetrate.
fn two_boxes() -> Project {
    let mut p = the_box();
    let clean = p.geometry.clone();
    let base = clean.vertices.len() as u32;
    for v in &clean.vertices {
        let [x, y, z] = v.to_array();
        p.geometry
            .vertices
            .push(Vec3::from([x + 3.0, y + 5.0, z + 1.5]));
    }
    for f in &clean.faces {
        let mut copy = *f;
        copy.vertices = f.vertices.map(|v| v + base);
        p.geometry.faces.push(copy);
    }
    p.name = "two interpenetrating boxes".to_string();
    p
}

fn check_or_write(rel: &str, project: &Project) {
    let path = repo(rel);
    let text = schema::to_json(project);
    if std::env::var_os("SIMPA_WRITE_FIXTURES").is_some() {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, &text).unwrap();
    }
    let committed = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{rel}: {e}"));
    assert_eq!(
        committed, text,
        "{rel} differs from its recipe; regenerate with SIMPA_WRITE_FIXTURES=1"
    );
}

#[test]
fn box_three_faults_fixture_matches_its_recipe() {
    check_or_write(
        "tests/fixtures/geometry/box_three_faults.simpa",
        &three_faults(),
    );
}

#[test]
fn two_boxes_fixture_matches_its_recipe() {
    check_or_write(
        "tests/fixtures/geometry/two_boxes_interpenetrating.simpa",
        &two_boxes(),
    );
}
