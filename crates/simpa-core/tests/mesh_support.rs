//! Shared helpers for the `mesh_*` tests, included by each with `#[path]`. Built on its own it is
//! an empty test target.
//!
//! - [`tetgen_exe`] finds the M1 build of `tetgen.exe` through `common/paths.rs`:
//!   `$SIMPA_SOLVERS_DIR`, or `<repo>/target/solvers/bin`. It panics when neither has one: these
//!   tests never skip.
//! - [`scratch`] gives each test a fresh folder under cargo's test scratch space.
//! - [`invariants`] checks a `.mbin` against `docs/m5-m6-design.md` decision 6 on its own, so the
//!   mesher's tests do not lean on `mesh::verify`, which is built separately.
#![allow(dead_code)]

use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use simpa_core::formats::mbin;
use simpa_core::schema::{self, Project};

#[path = "common/paths.rs"]
mod paths;
#[allow(unused_imports)]
pub use paths::{fixture, repo_root, solvers_dir, tetgen_exe};

static N: AtomicUsize = AtomicUsize::new(0);

/// A fresh, empty folder for one test. Nothing is removed afterwards: `target/` is build output.
pub fn scratch(label: &str) -> PathBuf {
    let n = N.fetch_add(1, Ordering::SeqCst);
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join("mesh")
        .join(format!("{label}-{}-{n}-{stamp}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

pub fn load_room(name: &str) -> Project {
    let path = fixture(&format!("rooms/{name}"));
    schema::load(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

pub fn node(mesh: &mbin::Mesh, i: i32) -> [f64; 3] {
    mesh.nodes[i as usize].map(f64::from)
}

/// Area of a tetrahedron face, from the `.mbin`'s `f32` nodes.
pub fn face_area(mesh: &mbin::Mesh, f: &mbin::TetraFace) -> f64 {
    let [a, b, c] = f.vertices.map(|v| node(mesh, v));
    let n = cross(sub(b, a), sub(c, a));
    0.5 * dot(n, n).sqrt()
}

/// `(A−D)·((B−D)×(C−D))`, six times the signed volume.
pub fn orient(mesh: &mbin::Mesh, t: &mbin::Tetrahedron) -> f64 {
    let [a, b, c, d] = t.vertices.map(|v| node(mesh, v));
    dot(sub(a, d), cross(sub(b, d), sub(c, d)))
}

/// Volume in m³ per `idVolume`.
pub fn volume_by_id(mesh: &mbin::Mesh) -> std::collections::BTreeMap<i32, f64> {
    let mut out = std::collections::BTreeMap::new();
    for t in &mesh.tetrahedra {
        *out.entry(t.id_volume).or_insert(0.0) += orient(mesh, t).abs() / 6.0;
    }
    out
}

/// Every violation of the `.mbin` invariants of `docs/m5-m6-design.md` decision 6, plus the face
/// convention of `docs/formats/mbin.md`, as messages (the first few of each kind):
/// - face `i` is `FACE_CORNERS[i]` of the corners;
/// - a neighbour is -2 (none) or a tetrahedron index: TetGen's -1 never reaches the `.mbin`;
/// - a face with neighbour -2 has a marker ≥ 0;
/// - a face with a marker ≥ 0 and a neighbour has a twin carrying the same marker;
/// - neighbours are mutual and share the face;
/// - every tetrahedron has `(A−D)·((B−D)×(C−D)) < 0`.
pub fn invariants(mesh: &mbin::Mesh) -> Vec<String> {
    const FACE_CORNERS: [[usize; 3]; 4] = [[1, 3, 2], [2, 3, 0], [0, 3, 1], [1, 2, 0]];
    let mut bad = Vec::new();
    let mut note = |kind: &str, msg: String| {
        if bad.iter().filter(|b: &&String| b.starts_with(kind)).count() < 3 {
            bad.push(format!("{kind}: {msg}"));
        }
    };
    let key = |mut f: [i32; 3]| {
        f.sort_unstable();
        f
    };
    for (i, t) in mesh.tetrahedra.iter().enumerate() {
        if orient(mesh, t) >= 0.0 {
            note("winding", format!("tetrahedron {i}"));
        }
        for (k, f) in t.faces.iter().enumerate() {
            if f.vertices != FACE_CORNERS[k].map(|c| t.vertices[c]) {
                note("face order", format!("tetrahedron {i} face {k}"));
            }
            if f.neighbor == mbin::NO_NEIGHBOR {
                if f.marker < 0 {
                    note("unmarked hull", format!("tetrahedron {i} face {k}"));
                }
                continue;
            }
            if f.neighbor < 0 {
                // TetGen's -1 is written as -2; no other negative value is a neighbour.
                note(
                    "neighbour value",
                    format!("tetrahedron {i} face {k}: {}", f.neighbor),
                );
                continue;
            }
            let Some(other) = mesh.tetrahedra.get(f.neighbor as usize) else {
                note("neighbour range", format!("tetrahedron {i} face {k}"));
                continue;
            };
            let back = other
                .faces
                .iter()
                .find(|g| g.neighbor == i as i32 && key(g.vertices) == key(f.vertices));
            match back {
                None => note("not mutual", format!("tetrahedron {i} face {k}")),
                Some(g) if g.marker != f.marker => note(
                    "asymmetric marker",
                    format!("tetrahedron {i} face {k}: {} vs {}", f.marker, g.marker),
                ),
                Some(_) => {}
            }
        }
    }
    bad
}

/// Whether a process runs from an image named `image` (`tasklist`, which every Windows has).
pub fn process_running(image: &str) -> bool {
    let out = std::process::Command::new("tasklist")
        .args(["/FI", &format!("IMAGENAME eq {image}"), "/NH"])
        .output()
        .expect("tasklist runs");
    String::from_utf8_lossy(&out.stdout)
        .to_ascii_lowercase()
        .contains(&image.to_ascii_lowercase())
}
