//! The committed room fixtures, `tests/fixtures/rooms/`, hold M4 gate items (b) and (c) without
//! upstream's checkout (CI-portable). That they are exactly the import of upstream's tutorials,
//! and that the import's groups are the `.finfo` lists face for face, is checked on Grace by
//! `geometry_import_proj.rs` (`room_fixtures_are_the_import_of_upstreams_tutorials`,
//! `gate_b_...`, `gate_c_...`).

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::Path;

use simpa_core::schema::{self, Directivity, Project, SurfaceReceiverShape};
use simpa_core::validate;

fn room(name: &str) -> Project {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/rooms")
        .join(name);
    schema::load(&path).unwrap()
}

fn corners(p: &Project, f: usize) -> [[f64; 3]; 3] {
    p.geometry.faces[f]
        .vertices
        .map(|v| p.geometry.vertices[v as usize].to_array())
}

fn area(p: &Project) -> f64 {
    (0..p.geometry.faces.len())
        .map(|f| {
            let [a, b, c] = corners(p, f);
            let u = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
            let v = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
            let n = [
                u[1] * v[2] - u[2] * v[1],
                u[2] * v[0] - u[0] * v[2],
                u[0] * v[1] - u[1] * v[0],
            ];
            0.5 * (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt()
        })
        .sum()
}

fn signed_volume(p: &Project) -> f64 {
    (0..p.geometry.faces.len())
        .map(|f| {
            let [a, b, c] = corners(p, f);
            (a[0] * (b[1] * c[2] - b[2] * c[1]) - a[1] * (b[0] * c[2] - b[2] * c[0])
                + a[2] * (b[0] * c[1] - b[1] * c[0]))
                / 6.0
        })
        .sum()
}

/// Undirected edges and how many faces use each.
fn edge_uses(p: &Project) -> HashMap<(u32, u32), usize> {
    let mut uses = HashMap::new();
    for f in &p.geometry.faces {
        let t = f.vertices;
        for k in 0..3 {
            let (a, b) = (t[k], t[(k + 1) % 3]);
            *uses.entry((a.min(b), a.max(b))).or_insert(0) += 1;
        }
    }
    uses
}

fn faces_by_group(p: &Project) -> BTreeMap<String, BTreeSet<usize>> {
    let mut out: BTreeMap<String, BTreeSet<usize>> = BTreeMap::new();
    for (i, f) in p.geometry.faces.iter().enumerate() {
        out.entry(p.group(f.group).unwrap().name.clone())
            .or_default()
            .insert(i);
    }
    out
}

/// Every source and point receiver lies strictly inside the room: the positions and the mesh are
/// in the same frame (world metres, Z up).
fn assert_all_inside(p: &Project) {
    let points = p
        .sources
        .iter()
        .map(|s| (&s.name, s.position))
        .chain(p.point_receivers.iter().map(|r| (&r.name, r.position)));
    for (name, at) in points {
        let loc = validate::locate_point(&p.geometry, at);
        assert!(
            matches!(loc, validate::PointLocation::Inside { .. }),
            "{name} at {at:?}: {loc:?}"
        );
    }
}

fn print_issues(name: &str, p: &Project) -> Vec<validate::Issue> {
    let issues = validate::validate(p);
    for i in &issues {
        println!("{name}: {i:?}");
    }
    issues
}

#[test]
fn gate_c_tutorial1_box_fixture() {
    let p = room("tutorial1_box.simpa");
    p.check_integrity().unwrap();
    assert_eq!(p.geometry.vertices.len(), 8);
    assert_eq!(p.geometry.faces.len(), 12);
    let want: BTreeMap<String, BTreeSet<usize>> = [
        ("Ceiling", vec![10, 11]),
        ("Floor", vec![0, 1]),
        ("Walls", (2..=9).collect()),
    ]
    .into_iter()
    .map(|(n, f)| (n.to_string(), f.into_iter().collect()))
    .collect();
    assert_eq!(faces_by_group(&p), want);
    for (name, idmat, absorption) in [("Ceiling", 21, 0.3), ("Floor", 25, 0.1), ("Walls", 22, 0.2)]
    {
        let g = p.surface_groups.iter().find(|g| g.name == name).unwrap();
        let m = p.material(g.material).unwrap();
        assert_eq!(m.solver_id, Some(idmat), "{name}");
        assert!(m.absorption.iter().all(|a| a.get() == absorption), "{name}");
    }
    assert_eq!(p.surface_receivers.len(), 1);
    let rs = &p.surface_receivers[0];
    assert_eq!(rs.name, "Receiver");
    let SurfaceReceiverShape::Scene { groups } = &rs.shape else {
        panic!("not a scene receiver");
    };
    let covered: BTreeSet<usize> = (0..12)
        .filter(|&i| groups.contains(&p.geometry.faces[i].group))
        .collect();
    assert_eq!(covered, BTreeSet::from([0, 1]));

    let mut lo = [f64::INFINITY; 3];
    let mut hi = [f64::NEG_INFINITY; 3];
    for v in &p.geometry.vertices {
        for (k, c) in v.to_array().into_iter().enumerate() {
            lo[k] = lo[k].min(c);
            hi[k] = hi[k].max(c);
        }
    }
    assert_eq!(
        (lo, hi),
        ([0.0; 3], [6.0, 10.0, 3.0]),
        "a 6 x 10 x 3 m box, Z up"
    );
    let v = signed_volume(&p);
    println!("box: volume {v:.6} m3, area {:.6} m2", area(&p));
    assert_eq!(format!("{v:.3}"), "180.000");
    assert!(v > 0.0, "outward-wound");
    assert!(edge_uses(&p).values().all(|&n| n == 2));

    assert_eq!(p.sources.len(), 1);
    assert_eq!(p.sources[0].position.to_array(), [3.0, 5.0, 1.8]);
    assert_eq!(p.sources[0].directivity, Directivity::Omni);
    let mut receivers: Vec<[f64; 3]> = p
        .point_receivers
        .iter()
        .map(|r| r.position.to_array())
        .collect();
    receivers.sort_by(|a, b| a.partial_cmp(b).unwrap());
    assert_eq!(receivers, vec![[1.0, 1.0, 1.8], [3.0, 7.0, 1.8]]);
    assert_all_inside(&p);

    let issues = print_issues("box", &p);
    assert!(!validate::has_errors(&issues), "{issues:?}");
}

#[test]
fn gate_b_elmia_corrected_fixture() {
    let p = room("elmia_corrected.simpa");
    p.check_integrity().unwrap();
    assert_eq!(p.geometry.vertices.len(), 3926);
    assert_eq!(p.geometry.faces.len(), 7860);
    assert_eq!(p.surface_groups.len(), 10);
    // Faces and material id per group, as tutorial_2.proj's .finfo lists and idmat give them.
    let counts: BTreeMap<String, usize> = faces_by_group(&p)
        .into_iter()
        .map(|(n, f)| (n, f.len()))
        .collect();
    let want: BTreeMap<String, usize> = [
        ("audience", 1568),
        ("ceiling", 1114),
        ("exteriorwalls", 628),
        ("floor", 397),
        ("interiorwalls", 276),
        ("panelwalls", 1301),
        ("reflectors", 1085),
        ("sidereflector", 1170),
        ("stage", 112),
        ("stairs", 209),
    ]
    .into_iter()
    .map(|(n, c)| (n.to_string(), c))
    .collect();
    assert_eq!(counts, want);
    for (name, idmat) in [
        ("ceiling", 115),
        ("floor", 119),
        ("stairs", 121),
        ("stage", 114),
        ("sidereflector", 109),
        ("reflectors", 113),
        ("panelwalls", 108),
        ("interiorwalls", 120),
        ("exteriorwalls", 122),
        ("audience", 110),
    ] {
        let g = p.surface_groups.iter().find(|g| g.name == name).unwrap();
        assert_eq!(
            p.material(g.material).unwrap().solver_id,
            Some(idmat),
            "{name}"
        );
    }
    let a = area(&p);
    let v = signed_volume(&p);
    let uses = edge_uses(&p);
    println!(
        "elmia: area {a:.4} m2, signed volume {v:.3} m3, {} edges, all used twice: {}",
        uses.len(),
        uses.values().all(|&n| n == 2)
    );
    assert!((a - 4001.8).abs() <= 0.1, "area {a}");
    assert!(v > 0.0, "signed volume {v}");
    assert_eq!(uses.len(), 11_790);
    assert!(uses.values().all(|&n| n == 2));
    assert_eq!(p.sources.len(), 3);
    assert_eq!(p.point_receivers.len(), 6);
    assert_all_inside(&p);

    let issues = print_issues("elmia", &p);
    assert!(!validate::has_errors(&issues), "{issues:?}");
}
