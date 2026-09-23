//! Helpers shared by `geometry_check.rs` and `geometry_repair.rs` (included with `#[path]`).
//! Compiled on its own too, as an empty test target.
#![allow(dead_code)]

use std::path::{Path, PathBuf};

#[path = "common/paths.rs"]
mod paths;

use simpa_core::schema::{self, Face, Geometry, GroupId, Vec3};

pub fn repo(rel: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(rel)
}

/// Upstream's raw tutorial hall, relative to the upstream source tree.
pub const RAW_ELMIA: &str = "src/isimpa/resources/doc/tutorial/tutorial 2/elmia.ply";

/// The raw hall, from the upstream source tree (`common/paths.rs`: `$SIMPA_UPSTREAM`, else the
/// tree `solvers/build.ps1` extracts). Panics, naming where it looked, when it is absent.
pub fn raw_elmia() -> PathBuf {
    paths::upstream_file(RAW_ELMIA)
}

/// A committed fixture a gate item needs. Panics when it is missing: the test must run.
pub fn committed(rel: &str) -> PathBuf {
    let path = repo(rel);
    assert!(
        path.is_file(),
        "{} is missing: this gate item runs on it",
        path.display()
    );
    path
}

/// The corrected hall: `tests/fixtures/rooms/elmia_corrected.simpa` once the importer writes it,
/// else Night Mode's `testdata/elmia_corrected.ply`, whose geometry is the corrected hall's (its
/// surface groups are wrong, which the check does not look at).
pub fn corrected_elmia() -> (Geometry, String) {
    let simpa = repo("tests/fixtures/rooms/elmia_corrected.simpa");
    if simpa.is_file() {
        let project = schema::load(&simpa).expect("elmia_corrected.simpa loads");
        return (project.geometry, simpa.display().to_string());
    }
    let ply = repo("testdata/elmia_corrected.ply");
    (read_ply(&ply), ply.display().to_string())
}

/// A minimal ASCII PLY reader for the tests: `x y z` vertices, polygon faces fan-triangulated
/// from their first vertex (as the harvest census did), an optional per-face `layer_id` as the
/// surface group. Vertices are not welded.
pub fn read_ply(path: &Path) -> Geometry {
    let text = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let mut lines = text.lines().map(str::trim_end);
    assert_eq!(lines.next(), Some("ply"));
    let mut elements: Vec<(String, usize, Vec<String>)> = Vec::new();
    for line in lines.by_ref() {
        let words: Vec<&str> = line.split_whitespace().collect();
        match words.as_slice() {
            ["format", "ascii", _] => {}
            ["format", other, _] => panic!("{}: only ASCII PLY, not {other}", path.display()),
            ["element", name, count] => {
                elements.push((name.to_string(), count.parse().unwrap(), Vec::new()))
            }
            ["property", .., name] => elements.last_mut().unwrap().2.push(name.to_string()),
            ["end_header"] => break,
            _ => {}
        }
    }
    let mut vertices = Vec::new();
    let mut faces = Vec::new();
    for (name, count, properties) in &elements {
        for _ in 0..*count {
            let line = lines.next().expect("element row");
            let values: Vec<&str> = line.split_whitespace().collect();
            match name.as_str() {
                "vertex" => {
                    assert_eq!(&properties[..3], ["x", "y", "z"]);
                    let c: Vec<f64> = values[..3].iter().map(|v| v.parse().unwrap()).collect();
                    vertices.push(Vec3::new(c[0], c[1], c[2]));
                }
                "face" => {
                    let n: usize = values[0].parse().unwrap();
                    let idx: Vec<u32> = values[1..=n].iter().map(|v| v.parse().unwrap()).collect();
                    let layer: u128 = values.get(n + 1).map_or(0, |v| v.parse().unwrap());
                    let group = GroupId::from_u128(layer + 1);
                    for k in 1..n - 1 {
                        faces.push(Face {
                            vertices: [idx[0], idx[k], idx[k + 1]],
                            group,
                        });
                    }
                }
                _ => {}
            }
        }
    }
    Geometry { vertices, faces }
}

pub fn face(vertices: [u32; 3], group: GroupId) -> Face {
    Face { vertices, group }
}

/// An axis-aligned box with outward normals. Vertices: 0 (x0 y0 z0), 1 (x1 y0 z0), 2 (x1 y1 z0),
/// 3 (x0 y1 z0), then 4..7 the same at z1. Faces: 0-1 floor, 2-3 ceiling, 4-5 y0, 6-7 x1, 8-9 y1,
/// 10-11 x0.
pub fn box_geometry(lo: [f64; 3], hi: [f64; 3], group: GroupId) -> Geometry {
    let [x0, y0, z0] = lo;
    let [x1, y1, z1] = hi;
    let vertices = [
        [x0, y0, z0],
        [x1, y0, z0],
        [x1, y1, z0],
        [x0, y1, z0],
        [x0, y0, z1],
        [x1, y0, z1],
        [x1, y1, z1],
        [x0, y1, z1],
    ]
    .map(Vec3::from)
    .to_vec();
    let faces = [
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
        [0, 4, 7],
        [0, 7, 3],
    ]
    .map(|f| face(f, group))
    .to_vec();
    Geometry { vertices, faces }
}

/// Two triangles `a b c`, `a c d`.
pub fn quad(a: [f64; 3], b: [f64; 3], c: [f64; 3], d: [f64; 3], group: GroupId) -> Geometry {
    Geometry {
        vertices: [a, b, c, d].map(Vec3::from).to_vec(),
        faces: vec![face([0, 1, 2], group), face([0, 2, 3], group)],
    }
}

/// Appends `other` to `g`, renumbering its vertices.
pub fn append(g: &mut Geometry, other: &Geometry) {
    let base = g.vertices.len() as u32;
    g.vertices.extend_from_slice(&other.vertices);
    g.faces.extend(other.faces.iter().map(|f| Face {
        vertices: f.vertices.map(|v| v + base),
        group: f.group,
    }));
}

/// The box [0, 2] x [0, 1] x [0, 1] split at x = 1 by a partition, conforming: 12 vertices,
/// 20 outer faces with outward normals (0..20), then the two partition faces (20, 21).
pub fn partitioned_box() -> Geometry {
    let g = GroupId::from_u128(1);
    let p = GroupId::from_u128(2);
    // Vertex (i, j, k) at x = i, y = j, z = k, numbered i + 3 (j + 2 k).
    let v = |i: u32, j: u32, k: u32| i + 3 * (j + 2 * k);
    let mut vertices = Vec::new();
    for k in 0..2 {
        for j in 0..2 {
            for i in 0..3 {
                vertices.push(Vec3::new(f64::from(i), f64::from(j), f64::from(k)));
            }
        }
    }
    let mut faces = Vec::new();
    for i in 0..2 {
        // Floor (normal -z) and ceiling (+z) of each half.
        faces.push(face([v(i, 0, 0), v(i + 1, 1, 0), v(i + 1, 0, 0)], g));
        faces.push(face([v(i, 0, 0), v(i, 1, 0), v(i + 1, 1, 0)], g));
        faces.push(face([v(i, 0, 1), v(i + 1, 0, 1), v(i + 1, 1, 1)], g));
        faces.push(face([v(i, 0, 1), v(i + 1, 1, 1), v(i, 1, 1)], g));
        // Front y = 0 (-y) and back y = 1 (+y) of each half.
        faces.push(face([v(i, 0, 0), v(i + 1, 0, 0), v(i + 1, 0, 1)], g));
        faces.push(face([v(i, 0, 0), v(i + 1, 0, 1), v(i, 0, 1)], g));
        faces.push(face([v(i, 1, 0), v(i + 1, 1, 1), v(i + 1, 1, 0)], g));
        faces.push(face([v(i, 1, 0), v(i, 1, 1), v(i + 1, 1, 1)], g));
    }
    // Ends: x = 0 (-x), x = 2 (+x).
    faces.push(face([v(0, 0, 0), v(0, 0, 1), v(0, 1, 1)], g));
    faces.push(face([v(0, 0, 0), v(0, 1, 1), v(0, 1, 0)], g));
    faces.push(face([v(2, 0, 0), v(2, 1, 1), v(2, 0, 1)], g));
    faces.push(face([v(2, 0, 0), v(2, 1, 0), v(2, 1, 1)], g));
    // The partition at x = 1.
    faces.push(face([v(1, 0, 0), v(1, 0, 1), v(1, 1, 1)], p));
    faces.push(face([v(1, 0, 0), v(1, 1, 1), v(1, 1, 0)], p));
    Geometry { vertices, faces }
}

pub fn tri(g: &Geometry, f: u32) -> [[f64; 3]; 3] {
    g.faces[f as usize]
        .vertices
        .map(|v| g.vertices[v as usize].to_array())
}

/// Separating-axis test for closed triangles; exact when every product and sum of coordinates
/// is exact in f64 (small dyadic coordinates).
pub fn sat_meet(t1: &[[f64; 3]; 3], t2: &[[f64; 3]; 3]) -> bool {
    type P = [f64; 3];
    fn sub(a: P, b: P) -> P {
        [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
    }
    fn cross(a: P, b: P) -> P {
        [
            a[1] * b[2] - a[2] * b[1],
            a[2] * b[0] - a[0] * b[2],
            a[0] * b[1] - a[1] * b[0],
        ]
    }
    fn dot(a: P, b: P) -> f64 {
        a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
    }
    let e1 = [sub(t1[1], t1[0]), sub(t1[2], t1[1]), sub(t1[0], t1[2])];
    let e2 = [sub(t2[1], t2[0]), sub(t2[2], t2[1]), sub(t2[0], t2[2])];
    let n1 = cross(e1[0], e1[1]);
    let n2 = cross(e2[0], e2[1]);
    let mut axes = vec![n1, n2];
    for a in e1 {
        for b in e2 {
            axes.push(cross(a, b));
        }
    }
    for n in [n1, n2] {
        for e in e1.iter().chain(e2.iter()) {
            axes.push(cross(n, *e));
        }
    }
    let range = |t: &[P; 3], axis: P| {
        let d = t.map(|p| dot(p, axis));
        (d[0].min(d[1]).min(d[2]), d[0].max(d[1]).max(d[2]))
    };
    axes.into_iter().filter(|a| *a != [0.0; 3]).all(|axis| {
        let (lo1, hi1) = range(t1, axis);
        let (lo2, hi2) = range(t2, axis);
        hi1 >= lo2 && hi2 >= lo1
    })
}
