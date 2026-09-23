//! Keep-on-re-import (`core::geometry::import::reassign`): M4 gate item (g). A model re-exported
//! from a modelling tool comes back with its vertices moved by rounding, its faces in another
//! order and its groups gone (an STL has none); each face must still get the surface group, and
//! so the material and the surface receiver, it had. Reads only the committed room fixtures.

use std::fmt::Write as _;
use std::path::Path;

use simpa_core::geometry::import::{
    DEFAULT_REASSIGN_TOLERANCE_M, ImportOptions, ImportedModel, Unit, Up, read_stl, reassign,
};
use simpa_core::schema::{self, Project, SurfaceReceiverShape};

fn room(name: &str) -> Project {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/rooms")
        .join(name);
    schema::load(&path).unwrap()
}

/// A deterministic displacement of length `r` for vertex `i` (splitmix64 directions).
fn jitter(i: usize, r: f64) -> [f64; 3] {
    let mut s = 0x9E37_79B9_7F4A_7C15u64.wrapping_mul(i as u64 + 1);
    let mut next = || {
        s = s.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = s;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^= z >> 31;
        (z >> 11) as f64 / (1u64 << 53) as f64 * 2.0 - 1.0
    };
    loop {
        let d = [next(), next(), next()];
        let n = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
        if n > 0.1 && n <= 1.0 {
            return d.map(|c| c / n * r);
        }
    }
}

/// The project's model as an ASCII STL written by a modelling tool: every vertex moved by `r`
/// metres (the same way wherever it is used, so the facets still meet), coordinates to 17
/// significant digits, the facets in reverse order, one solid.
fn jittered_stl(p: &Project, r: f64) -> String {
    let moved: Vec<[f64; 3]> = p
        .geometry
        .vertices
        .iter()
        .enumerate()
        .map(|(i, v)| {
            let d = jitter(i, r);
            let a = v.to_array();
            [a[0] + d[0], a[1] + d[1], a[2] + d[2]]
        })
        .collect();
    let mut s = String::from("solid remodelled\n");
    for f in p.geometry.faces.iter().rev() {
        s.push_str("facet normal 0 0 0\nouter loop\n");
        for v in f.vertices {
            let [x, y, z] = moved[v as usize];
            writeln!(s, "vertex {x:.17e} {y:.17e} {z:.17e}").unwrap();
        }
        s.push_str("endloop\nendfacet\n");
    }
    s.push_str("endsolid remodelled\n");
    s
}

fn stl(text: &str) -> ImportedModel {
    read_stl(text.as_bytes(), &ImportOptions::new(Unit::Metre, Up::Z)).unwrap()
}

/// For each new face (in reverse order of the old ones), whether it kept the old face's
/// material: `(kept, of)`.
fn materials_kept(old: &Project, new: &Project) -> (usize, usize) {
    let n = old.geometry.faces.len();
    assert_eq!(new.geometry.faces.len(), n);
    let kept = (0..n)
        .filter(|&i| {
            let before = old.group(old.geometry.faces[i].group).unwrap().material;
            let after = new
                .group(new.geometry.faces[n - 1 - i].group)
                .unwrap()
                .material;
            before == after
        })
        .count();
    (kept, n)
}

#[test]
fn gate_g_box_reimported_with_1e_4_m_jitter_keeps_12_of_12_materials() {
    let old = room("tutorial1_box.simpa");
    let model = stl(&jittered_stl(&old, 1e-4));
    assert_eq!(model.vertices.len(), 8, "the jittered facets still weld");
    // Every vertex really moved by 1e-4 m.
    for (i, v) in old.geometry.vertices.iter().enumerate() {
        let d = jitter(i, 1e-4);
        assert!(((d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt() - 1e-4).abs() < 1e-15);
        assert!(
            model.vertices.iter().all(|w| w.to_array() != v.to_array()),
            "vertex {i} did not move"
        );
    }
    let r = reassign(&old, &model, DEFAULT_REASSIGN_TOLERANCE_M).unwrap();
    let (kept, of) = materials_kept(&old, &r.project);
    println!(
        "box: {kept}/{of} material assignments kept; matched {}, unmatched {}, new groups {}",
        r.matched_faces,
        r.unmatched_faces,
        r.new_groups.len()
    );
    assert_eq!((kept, of), (12, 12));
    assert_eq!(r.matched_faces, 12);
    assert!(r.new_groups.is_empty());
    assert!(r.empty_groups.is_empty());
    // Groups too, not only materials; and the surface receiver still covers the floor.
    for i in 0..12 {
        assert_eq!(
            r.project.geometry.faces[11 - i].group,
            old.geometry.faces[i].group
        );
    }
    let SurfaceReceiverShape::Scene { groups } = &r.project.surface_receivers[0].shape else {
        panic!("tutorial 1's receiver is a scene receiver");
    };
    let covered: Vec<usize> = (0..12)
        .filter(|&i| groups.contains(&r.project.geometry.faces[i].group))
        .collect();
    assert_eq!(
        covered,
        vec![10, 11],
        "the old floor faces 0 and 1, now last"
    );
    // The geometry is the new one; everything else is the old project's.
    assert_eq!(r.project.geometry.vertices, model.vertices);
    assert_eq!(r.project.materials, old.materials);
    assert_eq!(r.project.sources, old.sources);
    assert_eq!(r.project.point_receivers, old.point_receivers);
    r.project.check_integrity().unwrap();
}

#[test]
fn a_face_beyond_the_tolerance_goes_to_a_new_group_with_the_default_material() {
    // The x = 6 wall pushed out 5 cm, beyond the 1 cm tolerance: its two faces lose their
    // group, the other ten (whose corners moved within their own planes) keep theirs.
    let old = room("tutorial1_box.simpa");
    let mut moved = old.clone();
    for v in &mut moved.geometry.vertices {
        let [x, y, z] = v.to_array();
        if x == 6.0 {
            *v = simpa_core::schema::Vec3::new(6.05, y, z);
        }
    }
    let model = stl(&jittered_stl(&moved, 0.0));
    let r = reassign(&old, &model, DEFAULT_REASSIGN_TOLERANCE_M).unwrap();
    assert_eq!((r.matched_faces, r.unmatched_faces), (10, 2));
    assert_eq!(r.new_groups.len(), 1);
    let new_group = r.project.group(r.new_groups[0]).unwrap();
    assert_eq!(new_group.name, "remodelled");
    let m = r.project.material(new_group.material).unwrap();
    assert_eq!(m.name, "Default");
    assert!(m.absorption.iter().all(|a| a.get() == 0.0));
    let (kept, _) = materials_kept(&old, &r.project);
    assert_eq!(kept, 10);
    // With a 6 cm tolerance the wall is found again.
    let r = reassign(&old, &model, 0.06).unwrap();
    assert_eq!(materials_kept(&old, &r.project), (12, 12));
}

#[test]
fn elmia_reimported_with_1e_4_m_jitter_keeps_every_material() {
    let old = room("elmia_corrected.simpa");
    let model = stl(&jittered_stl(&old, 1e-4));
    assert_eq!(model.vertices.len(), old.geometry.vertices.len());
    let t = std::time::Instant::now();
    let r = reassign(&old, &model, DEFAULT_REASSIGN_TOLERANCE_M).unwrap();
    let elapsed = t.elapsed();
    let (kept, of) = materials_kept(&old, &r.project);
    let groups_kept = (0..of)
        .filter(|&i| r.project.geometry.faces[of - 1 - i].group == old.geometry.faces[i].group)
        .count();
    println!(
        "elmia: {kept}/{of} materials and {groups_kept}/{of} groups kept, in {elapsed:?} (debug)"
    );
    assert_eq!((kept, of), (7860, 7860));
    assert_eq!(groups_kept, 7860);
}

#[test]
fn reassign_refuses_a_bad_tolerance() {
    let old = room("tutorial1_box.simpa");
    let model = stl(&jittered_stl(&old, 0.0));
    for tol in [-1.0, f64::NAN, f64::INFINITY] {
        let e = reassign(&old, &model, tol).unwrap_err();
        assert_eq!(e.code(), "invalid");
    }
}
