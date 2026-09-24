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
mod folder;
mod geometry;
mod regions;

pub use dir::{FolderPoly, folder_poly, manifest_proves_regions, read_manifest_json};
pub use folder::{FolderRegions, verify_with_folder_geometry};
pub(crate) use geometry::point_triangle_distance;
pub use regions::{CellCheck, Reference, RegionCheck, ZoneCell, seed_inside, zone_cell};

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
/// each region no seed reaches the next number above the largest seed, one per region, counting
/// up by one with no gap (`attr++`, `tetgen.cxx:22403-22436`): the room is 1 without fitting
/// zones, and its parts are `room, room + 1, ...` (tutorial 3's room is three regions, 2084 to
/// 2086, above its fittings 1930 and 2083). An id is known when it is a fitting's, or one of the
/// room's parts: `room + k` where the mesh also carries every id from `room` to it. Anything else,
/// such as a room written 0 beside TetGen's numbering, or a part numbered past a gap, is
/// `unknown_volume_ids`.
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

    /// Whether a tetrahedron may carry `id`, on its own: a fitting's id, or `room` and above.
    /// [`verify_mesh`] also requires the room's parts to be numbered without a gap
    /// ([`VolumeIds::known_ids`]).
    pub fn knows(&self, id: i32) -> bool {
        self.fittings.contains(&id) || id >= self.room
    }

    /// Of the distinct ids a mesh carries (ascending), those TetGen's numbering allows: the
    /// fittings', and the room's parts from `room` up to the first missing number.
    pub fn known_ids(&self, present: &[i32]) -> Vec<i32> {
        let mut next = self.room;
        let mut known = Vec::new();
        for &id in present {
            if self.fittings.contains(&id) {
                known.push(id);
            } else if id == next {
                known.push(id);
                next = next.saturating_add(1);
            }
        }
        known
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
    /// first 20 face indices. A drawn fitting zone's triangles are not counted
    /// ([`VerifyReport::drawn_zone_faces`]).
    pub uncovered_scene_faces: usize,
    pub uncovered_scene_faces_first: Vec<u32>,
    /// Scene faces that are drawn fitting zones' triangles ([`drawn_zone_faces`]), which no marker
    /// has to name. Not an offence: a count of what the coverage check left out.
    #[serde(default)]
    pub drawn_zone_faces: usize,
    /// Tetrahedra whose `idVolume` is neither a fitting's nor a room part's: below
    /// [`VolumeIds::room`] and no fitting's, or a room part numbered past a gap
    /// ([`VolumeIds::known_ids`]) (`unknown_volume_ids`).
    pub unknown_volume_ids: usize,
    /// Total volume in cubic metres per `idVolume`.
    pub volume_by_id: BTreeMap<i32, f64>,
    /// The distance used for `marker_geometry_mismatches`, `16 · 2^-24 · r` for a scene whose
    /// largest |coordinate| is `r` (`geometry::marker_tolerance` derives it).
    pub marker_tolerance_m: f64,
    /// The largest finite distance from a marked face's node to the scene face its in-range
    /// marker names: how close the mesh comes to `marker_tolerance_m`.
    pub max_marker_distance_m: f64,
    /// Regions whose volume is not the volume of the cell of the meshed geometry they fill, or
    /// that fill no cell, lie in the exterior, or share a cell (`region_volume_mismatch`). Only
    /// checked against a [`Reference`] ([`verify_mesh_with`]).
    #[serde(default)]
    pub region_volume_mismatch: usize,
    /// Cells of the meshed geometry no region fills (`unmeshed_cells`).
    #[serde(default)]
    pub unmeshed_cells: usize,
    /// Fitting zones whose id is not on their zone's cell, or on no tetrahedron
    /// (`fitting_region_misplaced`).
    #[serde(default)]
    pub fitting_region_misplaced: usize,
    /// Fitting zones whose seed lies on facets between cells that the zone's own faces do not
    /// tell apart (`fitting_seed_ambiguous`).
    #[serde(default)]
    pub fitting_seed_ambiguous: usize,
    /// Whether the region volume check ran: only against a [`Reference`].
    #[serde(default)]
    pub regions_checked: bool,
    /// Each region against its cell, ascending by id.
    #[serde(default)]
    pub regions: Vec<RegionCheck>,
    /// Each cell of the meshed geometry, and the regions found in it.
    #[serde(default)]
    pub cells: Vec<CellCheck>,
    /// The scene faces [`Expectations::unmeshed_faces`] named: left out of the coverage check
    /// in place of [`drawn_zone_faces`]' recognition.
    #[serde(default)]
    pub expected_unmeshed_faces: Vec<u32>,
    pub codes: Vec<String>,
}

impl VerifyReport {
    pub fn passed(&self) -> bool {
        self.codes.is_empty()
    }

    /// Each count with its reason code, in report order.
    fn counts(&self) -> [(&'static str, usize); 15] {
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
            ("region_volume_mismatch", self.region_volume_mismatch),
            ("unmeshed_cells", self.unmeshed_cells),
            ("fitting_region_misplaced", self.fitting_region_misplaced),
            ("fitting_seed_ambiguous", self.fitting_seed_ambiguous),
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

/// What a caller that built the mesh knows beyond the `.mbin` and the `.cbin`.
#[derive(Clone, Copy, Debug, Default)]
pub struct Expectations<'a> {
    /// The scene faces no tetrahedron face need carry, in place of [`drawn_zone_faces`]'
    /// recognition: the facets `preprocess.exe` deleted (a box zone's bottom lying on the floor).
    /// Every other scene face must be covered.
    pub unmeshed_faces: Option<&'a [u32]>,
    /// The geometry TetGen was given and its cells: the region volume check runs against it.
    pub reference: Option<Reference<'a>>,
}

/// Checks `mesh` against the `.mbin` invariants (`docs/m5-m6-design.md`, decision 6) and against
/// the scene its markers index: [`verify_mesh_with`] with no [`Expectations`], so a drawn zone's
/// triangles are recognised ([`drawn_zone_faces`]) and no region volume is checked.
pub fn verify_mesh(mesh: &mbin::Mesh, scene: &cbin::Model, ids: &VolumeIds) -> VerifyReport {
    verify_mesh_with(mesh, scene, ids, &Expectations::default())
}

/// [`verify_mesh`], and what `expect` adds: its list of faces that need no marker in place of
/// [`drawn_zone_faces`], and the region volume check against its [`Reference`]
/// (`region_volume_mismatch`, `unmeshed_cells`, `fitting_region_misplaced`,
/// `fitting_seed_ambiguous`; `mesh/verify/regions.rs`).
pub fn verify_mesh_with(
    mesh: &mbin::Mesh,
    scene: &cbin::Model,
    ids: &VolumeIds,
    expect: &Expectations,
) -> VerifyReport {
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
    let mut tets_by_id: BTreeMap<i32, usize> = BTreeMap::new();
    for tet in &mesh.tetrahedra {
        *tets_by_id.entry(tet.id_volume).or_default() += 1;
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
    let present: Vec<i32> = tets_by_id.keys().copied().collect();
    let known = ids.known_ids(&present);
    r.unknown_volume_ids = tets_by_id
        .iter()
        .filter(|(id, _)| !known.contains(id))
        .map(|(_, n)| n)
        .sum();
    let mut covered = vec![false; n_scene];
    let drawn = match expect.unmeshed_faces {
        Some(list) => {
            let mut d = vec![false; n_scene];
            for &f in list {
                if let Some(x) = d.get_mut(f as usize) {
                    *x = true;
                }
            }
            r.expected_unmeshed_faces = list.to_vec();
            d
        }
        None => {
            let d = drawn_zone_faces(scene, ids);
            r.drawn_zone_faces = d.iter().filter(|x| **x).count();
            d
        }
    };
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
    for (s, _) in covered
        .iter()
        .enumerate()
        .filter(|&(s, c)| !*c && !drawn[s])
    {
        r.uncovered_scene_faces += 1;
        if r.uncovered_scene_faces_first.len() < UNCOVERED_LISTED {
            r.uncovered_scene_faces_first.push(s as u32);
        }
    }

    if let Some(reference) = &expect.reference {
        let o = regions::check(mesh, reference, tolerance);
        r.region_volume_mismatch = o.region_volume_mismatch;
        r.unmeshed_cells = o.unmeshed_cells;
        r.fitting_region_misplaced = o.fitting_region_misplaced;
        r.fitting_seed_ambiguous = o.fitting_seed_ambiguous;
        r.regions = o.regions;
        r.cells = o.cells;
        r.regions_checked = true;
    }

    r.codes = r
        .counts()
        .iter()
        .filter(|(_, n)| *n > 0)
        .map(|(code, _)| code.to_string())
        .collect();
    r
}

/// How many triangles one drawn fitting zone gives the `.cbin`.
pub const DRAWN_ZONE_TRIANGLES: usize = 12;

/// Which scene faces are drawn fitting zones' triangles, which no `.mbin` marker has to name.
///
/// Upstream's GUI appends a rectangular fitting zone's 12 triangles to the `.cbin` after the
/// room's faces, each with three vertices of its own, `idMat` 0, `idRs` -1 and `idEn` the zone's
/// id (`CObjet3D::ToCBINFormat`, `Objet3D_maillage.cpp:783-816`), and `config_xml::scene_mesh`
/// writes a run's `.cbin` the same way; this crate's mesher gives them no marker
/// (`docs/m5-m6-design.md`, decision 5), so without this every run folder of a project with a box
/// zone would fail `uncovered_scene_faces`.
///
/// A face is one when it lies in a run of [`DRAWN_ZONE_TRIANGLES`] faces at the end of the file
/// (runs counted back from the last face, stopping at the first run that is not one) whose faces
/// all have `idMat` 0, `idRs` -1 and the same `idEn`, one of `ids.fittings`, use 36 vertices that
/// no other face uses, and make one axis-aligned box with a volume: every vertex a corner of the
/// run's bounding box, each triangle three distinct corners of one side, each side two triangles
/// that share its diagonal. Winding is not checked. Anything else is a scene face like any
/// other, and uncovered when no marker names it.
pub fn drawn_zone_faces(scene: &cbin::Model, ids: &VolumeIds) -> Vec<bool> {
    let n = scene.faces.len();
    let mut drawn = vec![false; n];
    let mut uses = vec![0u32; scene.vertices.len()];
    for f in &scene.faces {
        for v in [f.a, f.b, f.c] {
            if let Some(u) = uses.get_mut(v as usize) {
                *u += 1;
            }
        }
    }
    let mut end = n;
    while end >= DRAWN_ZONE_TRIANGLES {
        let start = end - DRAWN_ZONE_TRIANGLES;
        if !is_drawn_box(&scene.faces[start..end], scene, &uses, ids) {
            break;
        }
        drawn[start..end].fill(true);
        end = start;
    }
    drawn
}

/// One run of [`drawn_zone_faces`].
fn is_drawn_box(faces: &[cbin::Face], scene: &cbin::Model, uses: &[u32], ids: &VolumeIds) -> bool {
    let zone = faces[0].id_en;
    if !ids.fittings.contains(&zone) {
        return false;
    }
    let mut triangles: Vec<[[f32; 3]; 3]> = Vec::with_capacity(faces.len());
    for f in faces {
        if f.id_mat != crate::config_xml::DRAWN_ZONE_MATERIAL_ID || f.id_rs != -1 || f.id_en != zone
        {
            return false;
        }
        let mut t = [[0.0f32; 3]; 3];
        for (k, v) in [f.a, f.b, f.c].into_iter().enumerate() {
            if uses.get(v as usize) != Some(&1) {
                return false;
            }
            let p = &scene.vertices[v as usize];
            t[k] = [p.x, p.y, p.z];
        }
        triangles.push(t);
    }
    let mut lo = [f32::INFINITY; 3];
    let mut hi = [f32::NEG_INFINITY; 3];
    for p in triangles.iter().flatten() {
        for k in 0..3 {
            lo[k] = lo[k].min(p[k]);
            hi[k] = hi[k].max(p[k]);
        }
    }
    // False for a NaN bound as well.
    if !(0..3).all(|k| lo[k] < hi[k]) {
        return false;
    }
    // A corner as three bits, bit k set on the upper bound of axis k.
    let corner = |p: &[f32; 3]| -> Option<u8> {
        let mut c = 0u8;
        for k in 0..3 {
            if p[k] == hi[k] {
                c |= 1 << k;
            } else if p[k] != lo[k] {
                return None;
            }
        }
        Some(c)
    };
    // Per side (axis k, bound b, index 2k + b): its triangles' corner sets.
    let mut sides: [Vec<[u8; 3]>; 6] = Default::default();
    for t in &triangles {
        let Some(c) = t.iter().map(corner).collect::<Option<Vec<u8>>>() else {
            return false;
        };
        let c = [c[0], c[1], c[2]];
        if c[0] == c[1] || c[1] == c[2] || c[0] == c[2] {
            return false;
        }
        // Three distinct corners share the bit of at most one axis: that is their side.
        let Some(k) = (0..3).find(|&k| c.iter().all(|&x| (x >> k) & 1 == (c[0] >> k) & 1)) else {
            return false;
        };
        sides[2 * k + usize::from((c[0] >> k) & 1)].push(c);
    }
    sides.iter().all(|side| {
        let [a, b] = side.as_slice() else {
            return false;
        };
        let shared: Vec<u8> = a.iter().copied().filter(|x| b.contains(x)).collect();
        // Two corners in common, opposite on the side: its diagonal.
        shared.len() == 2 && (shared[0] ^ shared[1]).count_ones() == 2
    })
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
    /// How its regions were held to the folder's own geometry (`folder.rs`). Read as the default
    /// when absent.
    #[serde(default)]
    pub regions: FolderRegions,
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
/// - `regions_unchecked`: the `.mbin`'s regions could not be held to cells, the folder's
///   geometry (its `.poly`, else its `.cbin`) being refused by the geometry check
///   ([`verify_with_folder_geometry`]), and its `mesh.json` does not prove that the mesher held
///   them when it built this `.mbin` ([`FolderRegions::proven_by_manifest`]);
/// - `nothing_to_verify`: neither TetGen output nor a `.mbin`.
///
/// The `.mbin` is checked with [`verify_with_folder_geometry`]: its regions are held to the
/// cells of the folder's `.poly` (TetGen's basename's, `scene_mesh.poly`, or the only one) or,
/// without one, of its `.cbin`.
///
/// Unreadable files, a folder with TetGen output under two basenames, several `.mbin` files
/// without `tetramesh.mbin` among them, and, beside a `.mbin`, several `.cbin` files without
/// `mesh.cbin` among them, are errors.
pub fn verify_dir(dir: &Path, ids: &VolumeIds) -> Result<DirReport, FormatError> {
    dir::verify(dir, ids)
}

/// The largest |coordinate| of `vertices`, the `r` of the verifier's distance tolerance.
pub(crate) fn geometry_max_abs(vertices: &[[f64; 3]]) -> f64 {
    geometry::max_abs(vertices.iter().copied())
}
