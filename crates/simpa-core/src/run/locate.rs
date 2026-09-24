//! Where SPPS puts a source or a point receiver in the tetrahedral mesh, emulated bit for bit, so
//! that a run SPPS would crash on, or finish with a silently empty receiver, is refused before
//! launch (`docs/solver-contract.md` Part B, "The run manager").
//!
//! SPPS walks the tetrahedra in file order and takes the first one for which no face has
//! `(node[a] - p) . normal > 0`, all in `f32` (`coreinitialisation.cpp:71-95` for the sources,
//! `178-212` for the point receivers; upstream at `SOLVER_COMMIT`). A point that lies exactly on
//! an internal facet can round to `> 0` on both sides of it and so lie in no tetrahedron: the
//! seeded tutorial box's source (3, 5, 1.8) gets +2.4e-7 from both tetrahedra that share the
//! facet x/6 + y/10 = 1. Then:
//! - **a source** keeps `currentVolume` NULL (`coreTypes.h:512`), and
//!   `TranslateSourceAtTetrahedronVertex` dereferences it (`sppsInitialisation.cpp:20`, called
//!   at `sppsNantes.cpp:321`) before any particle runs: exit `0xC0000005` with no message. The
//!   guard at `sppsNantes.cpp:58-65` comes later and is never reached;
//! - **a point receiver** is linked to no tetrahedron, and its `indexTetra` is never written
//!   (declared at `coreTypes.h:425`, not set by the constructor at `coreTypes.cpp:36-44`).
//!   `ExpandPunctualReceiverTetrahedronLocalisation` starts from whatever that field holds
//!   (`sppsInitialisation.cpp:82-90`), and a receiver collects energy only in the tetrahedra
//!   linked to it (`spps/input_output/reportmanager.cpp:196-201`). The run ends OK with the
//!   receiver's levels wrong: exactly 0 on a refined box (`tests/run_locate.rs`), as for a
//!   receiver outside the room (Part A's `receiver_outside_volume`, VERIFIED P2 `rcv_out`).
//!
//! TCR loads the mesh through the same `initTetraMesh` (`main_tc.cpp:78`), so the same test runs,
//! but TCR never reads `currentVolume` nor a tetrahedron's receiver links (no use in `src/ctr/`;
//! its `linkedRecepteurP` is its own receiver record, `tcTypes.h:91`), and it computes from the
//! positions themselves (`TC_CalculationCore.cpp:166-168`). The check is SPPS's only.
//!
//! **The emulation, step by step.** Every step is `f32` (`typedef float decimal`,
//! `Core/mathlib.h:54`), built by MSVC x64 with the default `/fp:precise` and no `/arch`, so SSE2
//! scalar operations: no FMA contraction, no extended precision, no reassociation. Rust `f32`
//! arithmetic without `mul_add` is the same IEEE operations in the same order.
//! 1. **The position** is `ToFloat` of the `x`, `y`, `z` attributes
//!    (`base_core_configuration.cpp:131` for a source, `:244` for a point receiver): the first `,`
//!    becomes `.`, then `atof`, then the `float` it is stored in (`coreString.cpp:89-105`). A missing attribute reads as "" and so
//!    as 0 (`cxml.cpp:108-118`). The elements are every child of `<sources>` and of
//!    `<recepteursp>`, in order ([`super::expect`]'s `items`).
//! 2. **The nodes** are the `.mbin`'s `f32` triples, copied as they are (`coreTypes.cpp:193`).
//! 3. **Each face's normal** is computed once, at load (`coreTypes.cpp:233`), as
//!    `FaceNormal(n[a], n[b], n[c])` with `(a, b, c)` the face's vertices in file order
//!    (`coreTypes.cpp:227-229`): `Cross_r(Vector_r(n[a], n[b]), Vector_r(n[b], n[c]))`, where
//!    `Vector_r(p, q) = p - q` (`mathlib.h:368-371`) and `Cross_r(u, v) = (u.y*v.z - u.z*v.y,
//!    u.z*v.x - u.x*v.z, u.x*v.y - u.y*v.x)` (`mathlib.h:344-346`), then `normalize()`
//!    (`mathlib.h:391-397`): `l = sqrtf(x*x + y*y + z*z)` (`:148`); when `l < EPSILON`
//!    (`(float)1e-6`, `:56`) the vector is left as it is (`:151`); otherwise `inv = 1.0f / l`
//!    and each component is multiplied by `inv` (`:152-155`).
//! 4. **The test** is `pSrcA = n[a] - p` (`coreinitialisation.cpp:84`, `mathlib.h:120`) and
//!    `pSrcA.dot(normal) > 0` (`:85`), with `dot = (x*x' + y*y') + z*z'` (`mathlib.h:173`). It is
//!    strict: 0 is inside, and NaN compares false, so a NaN position is "inside" the first
//!    tetrahedron.
//! 5. **The first tetrahedron that passes wins** (`coreinitialisation.cpp:77, 91-93` and
//!    `198-209`). Only whether there is one matters here.

use std::fmt::Write as _;
use std::path::Path;

use roxmltree::Document;

use super::expect;
use super::verdict::{Reason, codes};
use crate::config_xml::names;
use crate::formats::mbin;

/// A point or a vector as SPPS holds it: `vec3`, three `float`s.
pub type Vec3 = [f32; 3];

/// `Vector_r(p, q)`, and `vec3::operator-`: `p - q` per component.
fn sub(p: Vec3, q: Vec3) -> Vec3 {
    [p[0] - q[0], p[1] - q[1], p[2] - q[2]]
}

/// `Cross_r(u, v)`.
fn cross(u: Vec3, v: Vec3) -> Vec3 {
    [
        u[1] * v[2] - u[2] * v[1],
        u[2] * v[0] - u[0] * v[2],
        u[0] * v[1] - u[1] * v[0],
    ]
}

/// `vec3::dot`: `(x*x' + y*y') + z*z'`.
fn dot(u: Vec3, v: Vec3) -> f32 {
    u[0] * v[0] + u[1] * v[1] + u[2] * v[2]
}

/// `EPSILON` of `Core/mathlib.h:56`, `(decimal)0.000001`.
const EPSILON: f32 = 0.000001;

/// `FaceNormal(p1, p2, p3)`: the normal SPPS stores for a face whose vertices are `p1, p2, p3` in
/// file order, normalised unless its length is below `EPSILON`.
pub fn face_normal(p1: Vec3, p2: Vec3, p3: Vec3) -> Vec3 {
    let v = cross(sub(p1, p2), sub(p2, p3));
    let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    if l < EPSILON {
        return v;
    }
    let inv = 1.0f32 / l;
    [v[0] * inv, v[1] * inv, v[2] * inv]
}

/// Whether SPPS counts `p` outside the face whose first vertex is `a` and whose stored normal is
/// `normal`: `(a - p) . normal > 0`.
pub fn outside_face(a: Vec3, normal: Vec3, p: Vec3) -> bool {
    dot(sub(a, p), normal) > 0.0
}

/// A face whose vertex index is not a node of the mesh: SPPS would read outside its node array,
/// so nothing can be emulated.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IndexError {
    pub tetrahedron: usize,
    pub face: usize,
    pub index: i32,
}

impl std::fmt::Display for IndexError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "tetrahedron {} face {} names node {}, which the mesh does not have",
            self.tetrahedron, self.face, self.index
        )
    }
}

impl std::error::Error for IndexError {}

/// A mesh as SPPS's point test sees it: per tetrahedron, each face's first node and the normal
/// SPPS computed for it at load.
#[derive(Clone, Debug)]
pub struct TetraTest {
    faces: Vec<[(Vec3, Vec3); 4]>,
}

impl TetraTest {
    /// The faces of `mesh`, with their normals, as `t_TetraMesh::LoadFile` makes them.
    pub fn new(mesh: &mbin::Mesh) -> Result<Self, IndexError> {
        let node = |t: usize, f: usize, i: i32| {
            usize::try_from(i)
                .ok()
                .and_then(|k| mesh.nodes.get(k).copied())
                .ok_or(IndexError {
                    tetrahedron: t,
                    face: f,
                    index: i,
                })
        };
        let mut faces = Vec::with_capacity(mesh.tetrahedra.len());
        for (t, tet) in mesh.tetrahedra.iter().enumerate() {
            let mut four = [([0.0; 3], [0.0; 3]); 4];
            for (f, face) in tet.faces.iter().enumerate() {
                let [a, b, c] = face.vertices;
                let (a, b, c) = (node(t, f, a)?, node(t, f, b)?, node(t, f, c)?);
                four[f] = (a, face_normal(a, b, c));
            }
            faces.push(four);
        }
        Ok(TetraTest { faces })
    }

    /// How many tetrahedra the mesh has.
    pub fn len(&self) -> usize {
        self.faces.len()
    }

    /// Whether the mesh has no tetrahedron.
    pub fn is_empty(&self) -> bool {
        self.faces.is_empty()
    }

    /// Whether tetrahedron `t` holds `p` for SPPS: no face has `p` outside it.
    pub fn holds(&self, t: usize, p: Vec3) -> bool {
        self.faces[t].iter().all(|&(a, n)| !outside_face(a, n, p))
    }

    /// The tetrahedron SPPS puts `p` in: the first that holds it, `None` for none.
    pub fn locate(&self, p: Vec3) -> Option<usize> {
        (0..self.faces.len()).find(|&t| self.holds(t, p))
    }
}

/// C's `atof` (`strtod` with no end pointer), as MSVC's UCRT reads a decimal: leading C spaces,
/// an optional sign, then the longest prefix that is digits with an optional `.` and an optional
/// exponent, correctly rounded; `inf`, `infinity` and `nan` in any case; 0 when nothing converts.
/// `None` for a hexadecimal float (`0x...`), which is not emulated.
pub fn atof(text: &str) -> Option<f64> {
    let s = text.trim_start_matches([' ', '\t', '\n', '\x0b', '\x0c', '\r']);
    let b = s.as_bytes();
    let mut i = usize::from(matches!(b.first(), Some(b'+' | b'-')));
    let negative = b.first() == Some(&b'-');
    let rest = &b[i..];
    let starts =
        |w: &str| rest.len() >= w.len() && rest[..w.len()].eq_ignore_ascii_case(w.as_bytes());
    if starts("inf") {
        return Some(if negative {
            f64::NEG_INFINITY
        } else {
            f64::INFINITY
        });
    }
    if starts("nan") {
        return Some(f64::NAN);
    }
    let digits = |from: usize| b[from..].iter().take_while(|c| c.is_ascii_digit()).count();
    let int = digits(i);
    let hex_mark = matches!(b.get(i + 1), Some(b'x' | b'X'));
    if int == 1 && b[i] == b'0' && hex_mark {
        let after = b.get(i + 2).copied().unwrap_or(0);
        let frac_hex = after == b'.' && b.get(i + 3).is_some_and(u8::is_ascii_hexdigit);
        if after.is_ascii_hexdigit() || frac_hex {
            return None;
        }
    }
    i += int;
    let mut frac = 0;
    if b.get(i) == Some(&b'.') {
        frac = digits(i + 1);
        if int + frac > 0 {
            i += 1 + frac;
        }
    }
    if int + frac == 0 {
        return Some(0.0);
    }
    if matches!(b.get(i), Some(b'e' | b'E')) {
        let mut j = i + 1;
        if matches!(b.get(j), Some(b'+' | b'-')) {
            j += 1;
        }
        let exp = digits(j);
        if exp > 0 {
            i = j + exp;
        }
    }
    s[..i].parse::<f64>().ok()
}

/// `CoreString::ToFloat` (`coreString.cpp:89-105`): the first `,` becomes `.`, then [`atof`],
/// then the `float` it is stored in. `None` where [`atof`] is.
pub fn to_float(text: &str) -> Option<f32> {
    match text.find(',') {
        Some(i) => atof(&format!("{}.{}", &text[..i], &text[i + 1..])),
        None => atof(text),
    }
    .map(|v| v as f32)
}

/// What SPPS locates in the mesh.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// A child of `<sources>`.
    Source,
    /// A child of `<recepteursp>`.
    PointReceiver,
}

/// A source or point receiver as SPPS reads it from `config.xml`.
#[derive(Clone, Debug, PartialEq)]
pub struct Point {
    pub kind: Kind,
    /// Its place among its kind's elements in the project's order, from 1: `config.xml` lists
    /// them newest first, as upstream's GUI writes them and `config_xml::write` does
    /// (`crates/simpa-core/tests/parity_inputs.rs`), so the file's last element is number 1 and a
    /// receiver's number goes with its name ("point receiver 2 "Receiver 2"").
    pub number: usize,
    /// `source@name` or `recepteur_ponctuel@lbl`.
    pub label: String,
    /// The `x`, `y` and `z` attributes as written; "" when missing.
    pub text: [String; 3],
    /// The position SPPS stores; `None` when a coordinate is a hexadecimal float ([`atof`]).
    pub position: Option<Vec3>,
}

impl Point {
    /// `source 1 "Source 1" at (3, 5, 1.8)`: the position as SPPS stores it.
    fn describe(&self) -> String {
        let what = match self.kind {
            Kind::Source => "source",
            Kind::PointReceiver => "point receiver",
        };
        let mut s = format!("{what} {} \"{}\"", self.number, self.label);
        if let Some([x, y, z]) = self.position {
            let _ = write!(s, " at ({x}, {y}, {z})");
        }
        s
    }
}

/// Every source and point receiver of `doc`, in the order SPPS reads them.
pub fn points(doc: &Document) -> Vec<Point> {
    let root = doc.root_element();
    let mut out = Vec::new();
    for (kind, list, label) in [
        (Kind::Source, "sources", "name"),
        (Kind::PointReceiver, "recepteursp", "lbl"),
    ] {
        let Some(node) = expect::child(root, list) else {
            continue;
        };
        let count = expect::items(node).count();
        for (i, e) in expect::items(node).enumerate() {
            let text = ["x", "y", "z"].map(|a| e.attribute(a).unwrap_or("").to_string());
            let position = match text.each_ref().map(|t| to_float(t)) {
                [Some(x), Some(y), Some(z)] => Some([x, y, z]),
                _ => None,
            };
            out.push(Point {
                kind,
                number: count - i,
                label: e.attribute(label).unwrap_or("").to_string(),
                text,
                position,
            });
        }
    }
    out
}

/// The sources and point receivers of `doc` that SPPS would locate in no tetrahedron of `mesh`,
/// in order. A point whose position is not emulated is left out.
pub fn unlocated(doc: &Document, test: &TetraTest) -> Vec<Point> {
    points(doc)
        .into_iter()
        .filter(|p| p.position.is_some_and(|x| test.locate(x).is_none()))
        .collect()
}

/// The refusals for `doc` on `mesh` (named `mbin_name` in the details): `source_unlocatable`
/// for the sources SPPS would locate in no tetrahedron, `receiver_unlocatable` for the point
/// receivers, one reason each at most, listing every such point. Nothing when the mesh names a
/// node it does not have: nothing can be emulated then, and `pre_launch`'s mesh check refuses it.
pub fn check(doc: &Document, mesh: &mbin::Mesh, mbin_name: &str) -> Vec<Reason> {
    let Ok(test) = TetraTest::new(mesh) else {
        return Vec::new();
    };
    let lost = unlocated(doc, &test);
    let list = |kind: Kind| {
        let v: Vec<String> = lost
            .iter()
            .filter(|p| p.kind == kind)
            .map(Point::describe)
            .collect();
        (!v.is_empty()).then(|| v.join("; "))
    };
    let n = test.len();
    let mut out = Vec::new();
    if let Some(l) = list(Kind::Source) {
        out.push(Reason::new(
            codes::SOURCE_UNLOCATABLE,
            format!(
                "{l}: SPPS's f32 point test puts it in none of the {n} tetrahedra of {mbin_name} \
                 (coreinitialisation.cpp:71-95), and then dereferences the missing tetrahedron \
                 and crashes with 0xC0000005 before any particle runs \
                 (sppsInitialisation.cpp:20). A source exactly on an internal facet does this; \
                 move it off the facet or refine the mesh"
            ),
        ));
    }
    if let Some(l) = list(Kind::PointReceiver) {
        out.push(Reason::new(
            codes::RECEIVER_UNLOCATABLE,
            format!(
                "{l}: SPPS's f32 point test links it to none of the {n} tetrahedra of \
                 {mbin_name} (coreinitialisation.cpp:178-212), so it collects energy only from \
                 where an uninitialised tetrahedron index leads (sppsInitialisation.cpp:82-90), \
                 so its levels are not its own position's, zero when that index leads nowhere \
                 near it, with no message. Move it off the facet or refine the mesh"
            ),
        ));
    }
    out
}

/// [`check`] on the working folder `solve`: its `config.xml` and the `.mbin` its
/// `tetrameshFileName` names. Nothing when either cannot be read: `pre_launch` (for `run_folder`)
/// and the export checks (for `run_project`) refuse those.
pub fn check_folder(solve: &Path) -> Vec<Reason> {
    let Ok(bytes) = std::fs::read(solve.join(names::CONFIG)) else {
        return Vec::new();
    };
    let text = String::from_utf8_lossy(bytes.strip_prefix(b"\xef\xbb\xbf").unwrap_or(&bytes));
    let Ok(doc) = Document::parse(&text) else {
        return Vec::new();
    };
    let mbin_name = expect::child(doc.root_element(), "simulation")
        .and_then(|s| s.attribute("tetrameshFileName"))
        .unwrap_or("");
    if mbin_name.is_empty() {
        return Vec::new();
    }
    match mbin::read_file(&solve.join(mbin_name)) {
        Ok(mesh) => check(&doc, &mesh, mbin_name),
        Err(_) => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn atof_reads_the_prefix_c_reads() {
        let cases: [(&str, f64); 16] = [
            ("3", 3.0),
            ("1.8", 1.8),
            ("  \t-2.5e1", -25.0),
            ("+.5", 0.5),
            ("5.", 5.0),
            ("1e", 1.0),
            ("1e+", 1.0),
            ("2E-1x", 0.2),
            ("7abc", 7.0),
            ("", 0.0),
            ("abc", 0.0),
            (".", 0.0),
            ("-", 0.0),
            ("1.2.3", 1.2),
            ("0", 0.0),
            ("00012", 12.0),
        ];
        for (text, want) in cases {
            assert_eq!(atof(text), Some(want), "{text:?}");
        }
        assert_eq!(atof("-INF"), Some(f64::NEG_INFINITY));
        assert_eq!(atof("infinity"), Some(f64::INFINITY));
        assert!(atof("NaN").unwrap().is_nan());
        assert_eq!(atof("1e999"), Some(f64::INFINITY));
        // Hexadecimal floats are not emulated; a bare "0x" is the 0 before it.
        assert_eq!(atof("0x1p3"), None);
        assert_eq!(atof("0x.8"), None);
        assert_eq!(atof("0xg"), Some(0.0));
        // ToFloat: the first comma is the decimal point; then the float.
        assert_eq!(to_float("1,8"), Some(1.8f32));
        assert_eq!(to_float("1,8,5"), Some(1.8f32));
        assert_eq!(to_float("0.1"), Some(0.1f32));
        assert_eq!(
            to_float("0.1").unwrap().to_bits(),
            (0.1f64 as f32).to_bits()
        );
    }

    #[test]
    fn the_face_normal_is_normalised_unless_tiny() {
        let n = face_normal([0.0, 10.0, 0.0], [0.0, 10.0, 3.0], [6.0, 0.0, 3.0]);
        // (0,0,-3) x (-6,10,0) = (30, 18, 0), normalised.
        let l = (30f32 * 30.0 + 18.0 * 18.0).sqrt();
        assert_eq!(n, [30.0 * (1.0 / l), 18.0 * (1.0 / l), 0.0]);
        // Below EPSILON the cross product is kept as it is.
        let tiny = face_normal([0.0; 3], [1e-4, 0.0, 0.0], [1e-4, 1e-4, 0.0]);
        assert_eq!(tiny, [0.0, 0.0, 1e-4f32 * 1e-4f32]);
        // The seeded box's facet x/6 + y/10 = 1 as its two tetrahedra store it: opposite normals,
        // but each tests from its own first vertex, so each product rounds on its own. For the
        // box's source both round to +2.4e-7: outside both.
        let a = face_normal([0.0, 10.0, 0.0], [0.0, 10.0, 3.0], [6.0, 0.0, 3.0]);
        let b = face_normal([6.0, 0.0, 3.0], [0.0, 10.0, 3.0], [0.0, 10.0, 0.0]);
        assert_eq!(a.map(|x| -x), b);
        assert!(outside_face([0.0, 10.0, 0.0], a, [3.0, 5.0, 1.8]));
        assert!(outside_face([6.0, 0.0, 3.0], b, [3.0, 5.0, 1.8]));
    }
}
