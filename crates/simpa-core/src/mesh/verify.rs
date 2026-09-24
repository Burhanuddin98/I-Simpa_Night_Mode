//! Mesh verification: the `.mbin` invariants and the checks on a folder of TetGen output that
//! `simpa mesh-verify` reports. See `docs/m5-m6-design.md`, decision 6 and "Interfaces between
//! pieces".
//!
//! [`verify_mesh`] checks a mesh against the scene its markers index, in O(n log n) for n
//! tetrahedra: one pass over the tetrahedra, one sort of the face keys, one pass over the faces.
//! [`verify_dir`] adds what only the folder shows: TetGen's skipped facets, a missing `.neigh`, a
//! partial output set, and a manifest whose `.mbin` hash is not the file's.
//!
//! Every count is a number of offending items and has a reason code spelled as its field. A mesh
//! passes exactly when every count is 0.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::formats::{FormatError, cbin, mbin};

mod dir;
mod geometry;

/// Face `i` of a tetrahedron `(A, B, C, D)`, as corner positions: `B D C`, `C D A`, `A D B`,
/// `B C A`. Face `i` is opposite corner `i` and its neighbour is the tetrahedron across it
/// (`Objet3D_maillage.cpp:182-185`, `docs/formats/mbin.md`). `(A, B, C, D)` is a `.ele` row in
/// upstream's order, `(d, c, b, a)` ([`crate::mesh::upstream_order`]); every check here holds in
/// either order, the permutation being even.
pub const FACE_CORNERS: [[usize; 3]; 4] = [[1, 3, 2], [2, 3, 0], [0, 3, 1], [1, 2, 0]];

/// How many uncovered scene faces [`VerifyReport::uncovered_scene_faces_first`] lists.
pub const UNCOVERED_LISTED: usize = 20;

/// What [`verify_mesh`] accepts as volume ids: TetGen's region numbering, which upstream's GUI
/// writes into the `.mbin` unchanged, and so does this crate's builder (`docs/m5-m6-design.md`,
/// decision 1).
///
/// TetGen's `-A` gives each seeded region its seed's attribute, a fitting zone's solver id, and
/// each region no seed reaches the next number above the largest seed, one per region
/// (`tetgen.cxx:22403-22436`): the room is 1 without fitting zones, and its parts are
/// `room, room + 1, ...` (tutorial 3's room is three regions, 2084 to 2086, above its fittings
/// 1930 and 2083). An id is known when it is a fitting's or at least `room`; anything else,
/// such as a room written 0 beside TetGen's numbering, is `unknown_volume_ids`.
///
/// The default is a mesh without fitting zones: room 1 ([`VolumeIds::tetgen`]).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct VolumeIds {
    /// The room's first id; its further parts carry the ids above it.
    pub room: i32,
    /// The solver ids of the enabled fitting zones.
    pub fittings: Vec<i32>,
}

impl VolumeIds {
    /// TetGen's numbering for regions seeded with `fittings`: the room starts one above the
    /// largest seed, or at 1 without one (`maxattr` starts at 0, `tetgen.cxx:22357, 22404`).
    pub fn tetgen(fittings: Vec<i32>) -> Self {
        let room = fittings.iter().copied().max().unwrap_or(0).max(0) + 1;
        VolumeIds { room, fittings }
    }

    /// Whether a tetrahedron may carry `id`: a fitting's id, or a room part's.
    pub fn knows(&self, id: i32) -> bool {
        self.fittings.contains(&id) || id >= self.room
    }
}

impl Default for VolumeIds {
    fn default() -> Self {
        VolumeIds::tetgen(Vec::new())
    }
}

/// The result of [`verify_mesh`]. Every count is a number of offending items; `codes` lists the
/// reason code of each non-zero count, spelled as its field, and is empty exactly when the mesh
/// passes.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct VerifyReport {
    pub tetrahedra: usize,
    pub nodes: usize,
    /// Scene faces in the `.cbin` the markers index.
    pub scene_faces: usize,
    /// Corner, face-vertex or neighbour indices out of range, one per index (`index_errors`). A
    /// tetrahedron with a corner out of range gets no geometric check.
    pub index_errors: usize,
    /// Tetrahedra with a repeated corner, or with `|(A−D)·((B−D)×(C−D))|` at or below the `f32`
    /// noise floor of their corners (`degenerate_tets`; `geometry::volume_floor`). They get no
    /// orientation or face-order check.
    pub degenerate_tets: usize,
    /// Tetrahedra with `(A−D)·((B−D)×(C−D)) > 0`, against the `.mbin` convention
    /// (`inverted_tets`). With the degenerate ones this is every tetrahedron not `< 0`.
    pub inverted_tets: usize,
    /// Faces that are not the face opposite their slot's corner, wound as [`FACE_CORNERS`] winds
    /// it; any rotation of that triple is accepted (`misordered_faces`). The solver takes a face's
    /// normal from this winding (`coreTypes.cpp:233`).
    pub misordered_faces: usize,
    /// Faces with no neighbour and a marker below 0 (`unmarked_boundary_faces`).
    pub unmarked_boundary_faces: usize,
    /// Markers at or above `scene_faces` (`marker_out_of_range`).
    pub marker_out_of_range: usize,
    /// Faces whose neighbour does not name them back across the same three nodes, and faces with
    /// no neighbour whose three nodes another face also holds (`nonmutual_neighbors`).
    pub nonmutual_neighbors: usize,
    /// Marked faces with a neighbour whose shared face carries a different marker
    /// (`asymmetric_internal_faces`). Internal facets are marked on both sides (decision 6).
    pub asymmetric_internal_faces: usize,
    /// Marked faces with a node farther than `marker_tolerance_m` from the scene face their
    /// marker names, its plane and its edges both counting (`marker_geometry_mismatches`).
    pub marker_geometry_mismatches: usize,
    /// Scene faces no tetrahedron face carries (`uncovered_scene_faces`): the count, and the
    /// first 20 face indices.
    pub uncovered_scene_faces: usize,
    pub uncovered_scene_faces_first: Vec<u32>,
    /// Tetrahedra whose `idVolume` is neither a fitting's nor a room part's, that is below
    /// [`VolumeIds::room`] and no fitting's (`unknown_volume_ids`).
    pub unknown_volume_ids: usize,
    /// Total volume in cubic metres per `idVolume`.
    pub volume_by_id: BTreeMap<i32, f64>,
    /// The distance used for `marker_geometry_mismatches`, `16 · 2^-24 · r` for a scene whose
    /// largest |coordinate| is `r` (`geometry::marker_tolerance` derives it).
    pub marker_tolerance_m: f64,
    /// The largest finite distance from a marked face's node to the scene face its in-range
    /// marker names: how close the mesh comes to `marker_tolerance_m`.
    pub max_marker_distance_m: f64,
    pub codes: Vec<String>,
}

impl VerifyReport {
    pub fn passed(&self) -> bool {
        self.codes.is_empty()
    }

    /// Each count with its reason code, in report order.
    fn counts(&self) -> [(&'static str, usize); 11] {
        [
            ("index_errors", self.index_errors),
            ("degenerate_tets", self.degenerate_tets),
            ("inverted_tets", self.inverted_tets),
            ("misordered_faces", self.misordered_faces),
            ("unmarked_boundary_faces", self.unmarked_boundary_faces),
            ("marker_out_of_range", self.marker_out_of_range),
            ("nonmutual_neighbors", self.nonmutual_neighbors),
            ("asymmetric_internal_faces", self.asymmetric_internal_faces),
            (
                "marker_geometry_mismatches",
                self.marker_geometry_mismatches,
            ),
            ("uncovered_scene_faces", self.uncovered_scene_faces),
            ("unknown_volume_ids", self.unknown_volume_ids),
        ]
    }
}

/// The three node indices of a face, sorted: the key two faces share when they are the same face.
fn face_key(face: &mbin::TetraFace) -> [i32; 3] {
    let mut k = face.vertices;
    k.sort_unstable();
    k
}

/// True when `face` is `expected` or one of its rotations: the same face, wound the same way.
fn same_winding(face: [i32; 3], expected: [i32; 3]) -> bool {
    let [a, b, c] = expected;
    face == [a, b, c] || face == [b, c, a] || face == [c, a, b]
}

/// Checks `mesh` against the `.mbin` invariants (`docs/m5-m6-design.md`, decision 6) and against
/// the scene its markers index.
pub fn verify_mesh(mesh: &mbin::Mesh, scene: &cbin::Model, ids: &VolumeIds) -> VerifyReport {
    let n_tets = mesh.tetrahedra.len();
    let n_scene = scene.faces.len();
    let mut r = VerifyReport {
        tetrahedra: n_tets,
        nodes: mesh.nodes.len(),
        scene_faces: n_scene,
        ..VerifyReport::default()
    };
    let node = |v: i32| {
        usize::try_from(v)
            .ok()
            .and_then(|i| mesh.nodes.get(i))
            .map(|p| p.map(f64::from))
    };
    // Pass 1, per tetrahedron: indices, volume ids, degeneracy, orientation, face order.
    for tet in &mesh.tetrahedra {
        if !ids.knows(tet.id_volume) {
            r.unknown_volume_ids += 1;
        }
        for face in &tet.faces {
            r.index_errors += face.vertices.iter().filter(|&&v| node(v).is_none()).count();
            if usize::try_from(face.neighbor).is_ok_and(|n| n >= n_tets) {
                r.index_errors += 1;
            }
        }
        let [Some(a), Some(b), Some(c), Some(d)] = tet.vertices.map(node) else {
            r.index_errors += tet.vertices.iter().filter(|&&v| node(v).is_none()).count();
            continue;
        };
        let p = [a, b, c, d];
        let six_v = geometry::orient(p);
        if six_v.is_finite() {
            *r.volume_by_id.entry(tet.id_volume).or_default() += six_v.abs() / 6.0;
        }
        let v = tet.vertices;
        let distinct = (0..4).all(|i| (i + 1..4).all(|j| v[i] != v[j]));
        let floor = geometry::volume_floor(geometry::max_abs(p), geometry::gradient_sum(p));
        // False for a NaN volume or floor as well.
        let resolved = six_v.abs() > floor;
        if !distinct || !resolved {
            r.degenerate_tets += 1;
            continue;
        }
        if six_v > 0.0 {
            r.inverted_tets += 1;
        }
        for (face, corners) in tet.faces.iter().zip(FACE_CORNERS) {
            if !same_winding(face.vertices, corners.map(|k| v[k])) {
                r.misordered_faces += 1;
            }
        }
    }

    // Pass 2: which faces share their three nodes with another face. One sort, O(n log n).
    let mut keys: Vec<([i32; 3], usize)> = Vec::with_capacity(4 * n_tets);
    for (t, tet) in mesh.tetrahedra.iter().enumerate() {
        for (i, face) in tet.faces.iter().enumerate() {
            keys.push((face_key(face), 4 * t + i));
        }
    }
    keys.sort_unstable();
    let mut shared = vec![false; 4 * n_tets];
    for run in keys.chunk_by(|x, y| x.0 == y.0) {
        if run.len() > 1 {
            for &(_, slot) in run {
                shared[slot] = true;
            }
        }
    }
    drop(keys);

    // Pass 3, per face: neighbours, markers, geometry, coverage.
    let tolerance = geometry::marker_tolerance(scene);
    r.marker_tolerance_m = tolerance;
    let triangles: Vec<Option<[geometry::P; 3]>> = scene
        .faces
        .iter()
        .map(|f| {
            let v = |i: u32| {
                scene
                    .vertices
                    .get(i as usize)
                    .map(|v| [v.x, v.y, v.z].map(f64::from))
            };
            Some([v(f.a)?, v(f.b)?, v(f.c)?])
        })
        .collect();
    let mut covered = vec![false; n_scene];
    for (t, tet) in mesh.tetrahedra.iter().enumerate() {
        for (i, face) in tet.faces.iter().enumerate() {
            match usize::try_from(face.neighbor) {
                Ok(n) if n < n_tets => {
                    let key = face_key(face);
                    let twin = if n == t {
                        None
                    } else {
                        mesh.tetrahedra[n].faces.iter().find(|g| face_key(g) == key)
                    };
                    match twin {
                        Some(g) if usize::try_from(g.neighbor).is_ok_and(|b| b == t) => {
                            if face.marker >= 0 && g.marker != face.marker {
                                r.asymmetric_internal_faces += 1;
                            }
                        }
                        _ => r.nonmutual_neighbors += 1,
                    }
                }
                // Out of range: counted in index_errors.
                Ok(_) => {}
                // No neighbour (-2 by convention; the solver treats any negative as none,
                // coreTypes.cpp:230-231).
                Err(_) => {
                    if shared[4 * t + i] {
                        r.nonmutual_neighbors += 1;
                    }
                    if face.marker < 0 {
                        r.unmarked_boundary_faces += 1;
                    }
                }
            }
            let Ok(m) = usize::try_from(face.marker) else {
                continue;
            };
            if m >= n_scene {
                r.marker_out_of_range += 1;
                continue;
            }
            covered[m] = true;
            let mut worst = 0.0f64;
            for &v in &face.vertices {
                let d = match (node(v), triangles[m]) {
                    (Some(p), Some(tri)) => geometry::point_triangle_distance(p, tri),
                    _ => f64::INFINITY,
                };
                // NaN counts as infinitely far.
                worst = if d.is_nan() {
                    f64::INFINITY
                } else {
                    worst.max(d)
                };
            }
            if worst.is_finite() {
                r.max_marker_distance_m = r.max_marker_distance_m.max(worst);
            }
            if worst > tolerance {
                r.marker_geometry_mismatches += 1;
            }
        }
    }
    for (s, _) in covered.iter().enumerate().filter(|(_, c)| !**c) {
        r.uncovered_scene_faces += 1;
        if r.uncovered_scene_faces_first.len() < UNCOVERED_LISTED {
            r.uncovered_scene_faces_first.push(s as u32);
        }
    }

    r.codes = r
        .counts()
        .iter()
        .filter(|(_, n)| *n > 0)
        .map(|(code, _)| code.to_string())
        .collect();
    r
}

/// The result of [`verify_dir`].
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct DirReport {
    pub dir: PathBuf,
    /// The TetGen basename found (`scene_mesh`, `model`, ...), if any.
    pub basename: Option<String>,
    /// Rows in `<base>_skipped.face`, and their markers (TetGen's facet markers, `.cbin` face
    /// indices when the `.poly` carried them; -1 in every row when it did not).
    pub skipped_facets: usize,
    pub skipped_markers: Vec<i32>,
    /// The `.mbin` checked: `tetramesh.mbin`, or the folder's only `.mbin`.
    pub mbin_file: Option<PathBuf>,
    /// The `.cbin` it was checked against: `mesh.cbin`, or the folder's only `.cbin`. Looked for
    /// only when there is a `.mbin`.
    pub cbin_file: Option<PathBuf>,
    /// Lowercase hex sha256 of that `.mbin`.
    pub mbin_sha256: Option<String>,
    /// The `.mbin` check, when the folder holds a `.mbin` and the `.cbin` it indexes.
    pub mesh: Option<VerifyReport>,
    /// Reason codes: the folder-level ones (`tetgen_skipped_facets`, `neigh_missing`,
    /// `tetgen_output_missing`, `cbin_missing`, `manifest_mismatch`, `nothing_to_verify`)
    /// followed by the mesh report's.
    pub codes: Vec<String>,
}

impl DirReport {
    pub fn passed(&self) -> bool {
        self.codes.is_empty()
    }
}

/// Checks a folder holding TetGen output, a `.mbin` and its `.cbin`, a mesh manifest, or any
/// subset of them, whatever the TetGen basename. Folder-level codes:
/// - `tetgen_skipped_facets`: a `<base>_skipped.face` exists (TetGen stopped on intersecting
///   facets, `tetgen.cxx:36038-36085`);
/// - `neigh_missing`: TetGen output without `<base>.1.neigh` (TetGen writes none when it stops,
///   `tetgen.cxx:36225-36240`, and upstream then solves on default neighbours);
/// - `tetgen_output_missing`: TetGen output without all of `.1.node`, `.1.ele` and `.1.face`;
/// - `cbin_missing`: a `.mbin` with no `.cbin` to check its markers against;
/// - `manifest_mismatch`: `mesh.json` records a `.mbin` sha256 that is not the `.mbin`'s, or
///   one for a `.mbin` the folder lacks, or records no `.mbin` beside one. Its records are the
///   mesher's `files.mbin` (`docs/formats/mesh-manifest.md`; null when it wrote none) and a
///   top-level `mbin_sha256` (null: not recorded); an absent record is not checked;
/// - `nothing_to_verify`: neither TetGen output nor a `.mbin`.
///
/// Unreadable files, a folder with TetGen output under two basenames, several `.mbin` files
/// without `tetramesh.mbin` among them, and, beside a `.mbin`, several `.cbin` files without
/// `mesh.cbin` among them, are errors.
pub fn verify_dir(dir: &Path, ids: &VolumeIds) -> Result<DirReport, FormatError> {
    dir::verify(dir, ids)
}
