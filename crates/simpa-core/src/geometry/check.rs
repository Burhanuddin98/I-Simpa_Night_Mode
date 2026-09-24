//! `core::geometry::check`: can this triangle mesh be meshed and run as a room?
//!
//! [`check`] reads a [`Geometry`] as given (vertex indices, `f64` metres) and returns a
//! [`CheckReport`]: a verdict, reason codes, counts and every finding **by face index**. It never
//! changes the geometry; [`super::repair`] does the safe fixes.
//!
//! # What is checked
//!
//! - **Edge census.** Every distinct edge (unordered vertex-index pair) of every face whose indices
//!   are in range, with how many faces use it: once is **open**, exactly twice **manifold**, three
//!   or more **non-manifold** ([`Counts::edge_uses`]). The census is by index, as the solver sees
//!   the mesh: two vertices at the same position are two vertices here (see
//!   [`Counts::coincident_vertices`]; [`super::repair`] welds them).
//! - **Invalid, degenerate and duplicate faces.** A face with a vertex index out of range or a
//!   non-finite coordinate is invalid. A face with a repeated vertex, or whose three vertices are
//!   exactly collinear, is degenerate (zero area, decided with exact predicates). A face with the
//!   same three vertex positions as an earlier face, in either orientation, is a duplicate of it.
//!   None of these is analysed further; everything below is about the remaining faces, the
//!   **analysed** ones.
//! - **Self-intersections**, found with a bounding-box tree and decided with Shewchuk's exact
//!   `orient3d` / `orient2d`: two faces may share an edge or a vertex (by position) and nothing
//!   else. Touching counts: a vertex on another face, an edge along another face, coplanar
//!   overlap. TetGen refuses all of them.
//! - **Cells and orientation.** The sides of the faces are joined into cells, the volumes of air
//!   they bound (see `check/cells.rs`): cell 0 is the exterior, the room is depth 1, a fitting zone
//!   floating in it depth 2. Each analysed face is then classified ([`FaceClass`]):
//!   - [`FaceClass::Boundary`]: the exterior on one side, a room on the other: the outer shell.
//!   - [`FaceClass::Partition`]: two different rooms of the same depth: a wall between two
//!     volumes that both touch the outer shell.
//!   - [`FaceClass::NestedShell`]: two cells of different depth, neither the exterior: the
//!     surface of a closed volume inside the room (a fitting zone).
//!   - [`FaceClass::Sheet`]: the same room on both sides: a hanging reflector, a baffle, a shelf
//!     resting on a wall, a partition with a doorway.
//!   - [`FaceClass::Exterior`]: the exterior on both sides: a hole in the outer shell, or
//!     something outside the room.
//!
//!   A face is **inverted** when its normal points into the deeper of its two cells: every
//!   boundary face must point out of the room, every nested shell out of its zone. Partition and
//!   sheet faces may point either way.
//!
//! # What is refused
//!
//! Partitions, sheets, nested shells and the non-manifold edges where they meet the outer shell
//! are **accepted and classified**: SPPS needs internal faces to model transmission, and TetGen
//! meshes them. The verdict is [`Verdict::Refused`] when any [`ReasonCode`] applies:
//! no faces, invalid faces, degenerate or duplicate faces, self-intersections, an outer shell that
//! is not closed (an open edge or a face with the exterior on both sides), no enclosed volume at
//! all, inverted faces, or a component whose placement could not be decided.
//!
//! # Limits
//!
//! - The analysis trusts the self-intersection test: with intersecting faces the cells, and so
//!   the classification, orientation and volumes, can be wrong. [`CheckReport::topology_reliable`]
//!   says so; the verdict is then refused anyway.
//! - Components are placed with rays whose hit order uses rounded parameters: two components
//!   closer than about 1e-12 of the model's size along a ray can be ordered wrongly.
//! - Coordinates must not underflow the exact predicates (values below about 1e-150 m).

mod bvh;
mod cells;
mod intersect;
pub(crate) mod predicates;

use std::collections::{BTreeMap, HashMap};

use serde::Serialize;

use self::bvh::{Aabb, Bvh};
use self::cells::{EdgeTopo, NONE, Sides, UnionFind, Use};
use self::predicates::P3;
use crate::schema::Geometry;

/// The overall outcome.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
    /// No reason applies: the geometry can go to meshing.
    Ok,
    /// At least one reason applies; see [`CheckReport::reasons`].
    Refused,
}

/// Why a geometry is refused. The codes (their snake_case spelling) are stable API.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReasonCode {
    /// The geometry has no faces.
    EmptyGeometry,
    /// Faces with a vertex index out of range or a non-finite vertex coordinate.
    InvalidFaces,
    /// Faces of zero area (repeated vertex or exactly collinear vertices). Repairable: removed.
    DegenerateFaces,
    /// Faces repeating an earlier face's three vertex positions. Repairable: removed.
    DuplicateFaces,
    /// Faces meeting other than along a shared edge or at a shared vertex.
    SelfIntersections,
    /// The outer shell is not closed: open edges and faces with the exterior on both sides.
    OpenBoundary,
    /// No face bounds an enclosed volume.
    NoEnclosedVolume,
    /// Faces whose normal points into the volume they bound. Repairable: flipped.
    InvertedFaces,
    /// A face-connected component could not be placed relative to the others.
    UnresolvedTopology,
}

impl ReasonCode {
    /// The stable code, as serialised.
    pub fn as_str(self) -> &'static str {
        match self {
            ReasonCode::EmptyGeometry => "empty_geometry",
            ReasonCode::InvalidFaces => "invalid_faces",
            ReasonCode::DegenerateFaces => "degenerate_faces",
            ReasonCode::DuplicateFaces => "duplicate_faces",
            ReasonCode::SelfIntersections => "self_intersections",
            ReasonCode::OpenBoundary => "open_boundary",
            ReasonCode::NoEnclosedVolume => "no_enclosed_volume",
            ReasonCode::InvertedFaces => "inverted_faces",
            ReasonCode::UnresolvedTopology => "unresolved_topology",
        }
    }

    /// Whether [`super::repair`] fixes this reason by itself.
    pub fn repairable(self) -> bool {
        matches!(
            self,
            ReasonCode::DegenerateFaces | ReasonCode::DuplicateFaces | ReasonCode::InvertedFaces
        )
    }
}

impl std::fmt::Display for ReasonCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// One reason for refusal.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Reason {
    pub code: ReasonCode,
    /// How many findings (faces, pairs or edges, as the message says).
    pub count: usize,
    /// Whether [`super::repair`] fixes it.
    pub repairable: bool,
    /// The faces involved, ascending.
    pub faces: Vec<u32>,
    pub message: String,
}

/// Why a face is not analysed at all.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum InvalidCause {
    IndexOutOfRange,
    NonFiniteVertex,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize)]
pub struct InvalidFace {
    pub face: u32,
    pub cause: InvalidCause,
}

/// Why a face has zero area.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DegenerateKind {
    /// Two of its vertex indices are equal.
    RepeatedVertex,
    /// Three distinct vertices, exactly collinear (or coincident).
    ZeroArea,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize)]
pub struct DegenerateFace {
    pub face: u32,
    pub kind: DegenerateKind,
}

/// A face with the same three vertex positions as an earlier one.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize)]
pub struct DuplicateFace {
    pub face: u32,
    /// The first face with these positions (the one kept by repair).
    pub duplicate_of: u32,
    /// Same cyclic order, hence the same normal.
    pub same_orientation: bool,
    /// Same surface group, hence the same material.
    pub same_group: bool,
}

/// What an analysed face separates. See the module docs.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FaceClass {
    Boundary,
    Partition,
    NestedShell,
    Sheet,
    Exterior,
}

/// An analysed face that is not on the outer shell.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize)]
pub struct InternalFace {
    pub face: u32,
    pub class: FaceClass,
    /// The cell on the face's front (normal) side and on its back; 0 is the exterior.
    pub front_cell: u32,
    pub back_cell: u32,
}

/// What an edge that is not plainly manifold is, in the analysed mesh.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EdgeClass {
    /// Used once, by a face with the exterior on both sides: a hole in the outer shell (refused).
    BoundaryOpen,
    /// Used once, by a sheet inside a room: the free edge of a reflector (accepted).
    InternalFree,
    /// Used 3+ times, with the exterior in exactly one wedge: internal faces meeting the outer
    /// shell, such as a partition's edge on a wall (accepted).
    BoundaryJunction,
    /// Used 3+ times, with the exterior in no wedge (accepted).
    InternalJunction,
    /// Used 3+ times, with the exterior in two or more wedges: shells touching along an edge,
    /// or a fin outside the room (accepted; a fin's faces are refused as exterior).
    PinchedBoundary,
    /// Used twice by analysed faces; the other uses are by faces not analysed.
    Manifold,
    /// Every face using it is not analysed (invalid, degenerate or duplicate).
    Unanalysed,
}

/// An edge whose use count is not 2, in the census or in the analysed mesh.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize)]
pub struct EdgeFinding {
    /// Vertex indices, ascending.
    pub vertices: [u32; 2],
    /// Uses in the census (every face with in-range indices).
    pub uses: u32,
    /// Uses by analysed faces.
    pub analysed_uses: u32,
    /// Every face using it, ascending.
    pub faces: Vec<u32>,
    pub class: EdgeClass,
}

/// An enclosed volume.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct CellReport {
    /// 1, 2, ... (0 is the exterior and is not listed).
    pub id: u32,
    /// Faces to cross from the exterior: 1 for a room, 2 for a zone floating in it.
    pub depth: u32,
    /// Its volume, from the faces bounding it (independent of their orientation).
    pub volume_m3: f64,
    /// Faces with this cell on exactly one side.
    pub faces: usize,
}

/// Sizes and measures.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Counts {
    pub vertices: usize,
    pub faces: usize,
    /// Faces neither invalid, degenerate nor duplicate.
    pub analysed_faces: usize,
    /// Distinct edges in the census.
    pub edges: usize,
    /// Census edges used exactly once.
    pub open_edges: usize,
    /// Census edges used exactly twice.
    pub manifold_edges: usize,
    /// Census edges used three times or more.
    pub nonmanifold_edges: usize,
    /// Census histogram: uses -> number of edges.
    pub edge_uses: BTreeMap<u32, usize>,
    pub invalid_faces: usize,
    pub degenerate_faces: usize,
    pub duplicate_faces: usize,
    pub self_intersecting_pairs: usize,
    pub self_intersecting_faces: usize,
    pub boundary_faces: usize,
    pub partition_faces: usize,
    pub nested_shell_faces: usize,
    pub sheet_faces: usize,
    pub exterior_faces: usize,
    pub inverted_faces: usize,
    /// Analysed edges of class [`EdgeClass::BoundaryOpen`].
    pub boundary_open_edges: usize,
    /// Analysed edges of class [`EdgeClass::InternalFree`].
    pub internal_free_edges: usize,
    /// Analysed edges used three times or more.
    pub junction_edges: usize,
    /// Edges used by exactly two analysed faces that run along it the same way (a local
    /// orientation clash; informational, the verdict uses [`Counts::inverted_faces`]).
    pub orientation_conflict_edges: usize,
    /// Face-connected components of the analysed faces.
    pub components: usize,
    /// Enclosed volumes (cells other than the exterior).
    pub cells: usize,
    /// Vertices at exactly the position of a lower-numbered vertex.
    pub coincident_vertices: usize,
    /// Vertices no face uses (informational).
    pub unreferenced_vertices: usize,
    pub non_finite_vertices: usize,
}

/// Areas and volumes of the analysed faces.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct Measures {
    pub area_m2: f64,
    /// The sum of the signed volumes of the analysed faces as oriented: the enclosed volume for
    /// a closed shell with outward normals, negative when they point inwards. Internal faces
    /// contribute too; the verdict relies on [`Counts::inverted_faces`], not on this sign.
    pub signed_volume_m3: f64,
    /// The sum of the enclosed cells' volumes, independent of orientation.
    pub enclosed_volume_m3: f64,
}

/// The result of [`check`]. Face indices are indices into `Geometry::faces`, vertex indices
/// into `Geometry::vertices`. Every list is in ascending order.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct CheckReport {
    pub verdict: Verdict,
    /// Empty exactly when the verdict is ok; in [`ReasonCode`] order.
    pub reasons: Vec<Reason>,
    pub counts: Counts,
    pub measures: Measures,
    /// False when faces intersect or placement failed: the cells, classes, orientation and
    /// volumes may then be wrong (the verdict is refused in that case anyway).
    pub topology_reliable: bool,
    /// Edges used other than twice (in the census or by the analysed faces), classified.
    pub edges: Vec<EdgeFinding>,
    pub invalid_faces: Vec<InvalidFace>,
    pub degenerate_faces: Vec<DegenerateFace>,
    pub duplicate_faces: Vec<DuplicateFace>,
    /// Intersecting face pairs `[i, j]`, `i < j`.
    pub self_intersections: Vec<[u32; 2]>,
    pub inverted_faces: Vec<u32>,
    /// Every analysed face that is not [`FaceClass::Boundary`].
    pub internal_faces: Vec<InternalFace>,
    /// The enclosed cells.
    pub cells: Vec<CellReport>,
    /// Per face, the cell on its front (normal) side and on its back, `[front, back]`; 0 is the
    /// exterior, and `u32::MAX` marks a face that was not analysed. Not serialised: it is for
    /// callers that locate points in the cells (`mesh::verify`'s region volume check).
    #[serde(skip)]
    pub face_cells: Vec<[u32; 2]>,
}

impl CheckReport {
    pub fn is_ok(&self) -> bool {
        self.verdict == Verdict::Ok
    }

    /// The reason with this code, if it applies.
    pub fn reason(&self, code: ReasonCode) -> Option<&Reason> {
        self.reasons.iter().find(|r| r.code == code)
    }
}

/// The geometry as the predicates see it.
pub(crate) struct Mesh {
    pub pos: Vec<P3>,
    pub tris: Vec<[u32; 3]>,
    /// Per vertex: the lowest index of a vertex at exactly the same position.
    pub pid: Vec<u32>,
}

impl Mesh {
    fn tri(&self, f: u32) -> [P3; 3] {
        self.tris[f as usize].map(|i| self.pos[i as usize])
    }

    fn pids(&self, f: u32) -> [u32; 3] {
        self.tris[f as usize].map(|i| self.pid[i as usize])
    }
}

/// Exact-position key; `-0.0` and `0.0` are one position.
fn position_key(p: &P3) -> [u64; 3] {
    p.map(|c| if c == 0.0 { 0 } else { c.to_bits() })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Status {
    Invalid(InvalidCause),
    Degenerate(DegenerateKind),
    Duplicate(u32),
    Analysed,
}

/// Whether two cyclic triples list the same cycle (same orientation).
fn same_cycle(a: [u32; 3], b: [u32; 3]) -> bool {
    (0..3).any(|r| (0..3).all(|k| a[k] == b[(k + r) % 3]))
}

/// Signed volume of the tetrahedron `origin, t` (positive when `t` faces away from `origin`).
fn signed_volume(t: &[P3; 3], origin: P3) -> f64 {
    let [a, b, c] = t.map(|p| [p[0] - origin[0], p[1] - origin[1], p[2] - origin[2]]);
    (a[0] * (b[1] * c[2] - b[2] * c[1]) - a[1] * (b[0] * c[2] - b[2] * c[0])
        + a[2] * (b[0] * c[1] - b[1] * c[0]))
        / 6.0
}

fn area(t: &[P3; 3]) -> f64 {
    let u = [0, 1, 2].map(|k| t[1][k] - t[0][k]);
    let v = [0, 1, 2].map(|k| t[2][k] - t[0][k]);
    let n = [
        u[1] * v[2] - u[2] * v[1],
        u[2] * v[0] - u[0] * v[2],
        u[0] * v[1] - u[1] * v[0],
    ];
    0.5 * (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt()
}

fn plural(n: usize, one: &str, many: &str) -> String {
    if n == 1 {
        format!("{n} {one}")
    } else {
        format!("{n} {many}")
    }
}

/// Checks `geometry`. See the module docs.
pub fn check(geometry: &Geometry) -> CheckReport {
    let nv = geometry.vertices.len();
    let pos: Vec<P3> = geometry.vertices.iter().map(|v| v.to_array()).collect();
    let finite: Vec<bool> = pos
        .iter()
        .map(|p| p.iter().all(|c| c.is_finite()))
        .collect();
    let mut first_at: HashMap<[u64; 3], u32> = HashMap::with_capacity(nv);
    let pid: Vec<u32> = pos
        .iter()
        .enumerate()
        .map(|(i, p)| {
            if finite[i] {
                *first_at.entry(position_key(p)).or_insert(i as u32)
            } else {
                i as u32
            }
        })
        .collect();
    let coincident_vertices = pid
        .iter()
        .enumerate()
        .filter(|&(i, &p)| p != i as u32)
        .count();
    let non_finite_vertices = finite.iter().filter(|f| !**f).count();
    let tris: Vec<[u32; 3]> = geometry.faces.iter().map(|f| f.vertices).collect();
    let mesh = Mesh { pos, tris, pid };
    let nf = mesh.tris.len();

    // Face status.
    let mut referenced = vec![false; nv];
    let mut status = Vec::with_capacity(nf);
    let mut duplicates = Vec::new();
    let mut first_face: HashMap<[u32; 3], u32> = HashMap::with_capacity(nf);
    for (i, t) in mesh.tris.iter().enumerate() {
        let i = i as u32;
        if t.iter().any(|&v| v as usize >= nv) {
            status.push(Status::Invalid(InvalidCause::IndexOutOfRange));
            continue;
        }
        for &v in t {
            referenced[v as usize] = true;
        }
        let s = if t.iter().any(|&v| !finite[v as usize]) {
            Status::Invalid(InvalidCause::NonFiniteVertex)
        } else if t[0] == t[1] || t[1] == t[2] || t[0] == t[2] {
            Status::Degenerate(DegenerateKind::RepeatedVertex)
        } else if predicates::is_zero_area(&mesh.tri(i)) {
            Status::Degenerate(DegenerateKind::ZeroArea)
        } else {
            let p = mesh.pids(i);
            let mut key = p;
            key.sort_unstable();
            match first_face.get(&key) {
                Some(&first) => {
                    duplicates.push(DuplicateFace {
                        face: i,
                        duplicate_of: first,
                        same_orientation: same_cycle(p, mesh.pids(first)),
                        same_group: geometry.faces[i as usize].group
                            == geometry.faces[first as usize].group,
                    });
                    Status::Duplicate(first)
                }
                None => {
                    first_face.insert(key, i);
                    Status::Analysed
                }
            }
        };
        status.push(s);
    }
    let unreferenced_vertices = referenced.iter().filter(|r| !**r).count();
    let analysed: Vec<u32> = (0..nf as u32)
        .filter(|&f| status[f as usize] == Status::Analysed)
        .collect();
    let invalid_faces: Vec<InvalidFace> = status
        .iter()
        .enumerate()
        .filter_map(|(f, s)| match s {
            Status::Invalid(cause) => Some(InvalidFace {
                face: f as u32,
                cause: *cause,
            }),
            _ => None,
        })
        .collect();
    let degenerate_faces: Vec<DegenerateFace> = status
        .iter()
        .enumerate()
        .filter_map(|(f, s)| match s {
            Status::Degenerate(kind) => Some(DegenerateFace {
                face: f as u32,
                kind: *kind,
            }),
            _ => None,
        })
        .collect();

    // Bounding boxes and the tree over the analysed faces.
    let mut boxes = vec![Aabb::EMPTY; nf];
    for &f in &analysed {
        boxes[f as usize] = Aabb::of_points(&mesh.tri(f));
    }
    let bbox = analysed
        .iter()
        .fold(Aabb::EMPTY, |acc, &f| acc.union(boxes[f as usize]));
    let bvh = Bvh::build(analysed.iter().map(|&f| (f, boxes[f as usize])).collect());

    // Edge census, side joins and components, one edge at a time. A face uses each of its
    // distinct edges once: `[a, a, b]` uses `a b` once, not twice.
    let mut half_edges: Vec<(u32, u32, u32, bool, u32)> = Vec::with_capacity(3 * nf);
    for (f, t) in mesh.tris.iter().enumerate() {
        if status[f] == Status::Invalid(InvalidCause::IndexOutOfRange) {
            continue;
        }
        let key = |k: usize| {
            let (a, b) = (t[k], t[(k + 1) % 3]);
            (a.min(b), a.max(b))
        };
        for k in 0..3 {
            let (a, b, c) = (t[k], t[(k + 1) % 3], t[(k + 2) % 3]);
            if a != b && !(0..k).any(|j| key(j) == key(k)) {
                half_edges.push((a.min(b), a.max(b), f as u32, a < b, c));
            }
        }
    }
    half_edges.sort_unstable();
    let mut sides = Sides::new(nf);
    let mut components = UnionFind::new(nf);
    let mut edge_uses: BTreeMap<u32, usize> = BTreeMap::new();
    let mut pending: Vec<(EdgeFinding, EdgeTopo)> = Vec::new();
    let mut junction_edges = 0;
    let mut distinct_edges = 0;
    let mut uses: Vec<Use> = Vec::new();
    for group in half_edges.chunk_by(|a, b| (a.0, a.1) == (b.0, b.1)) {
        distinct_edges += 1;
        let (u, v) = (group[0].0, group[0].1);
        let raw = group.len() as u32;
        *edge_uses.entry(raw).or_insert(0) += 1;
        uses.clear();
        uses.extend(
            group
                .iter()
                .filter(|h| status[h.2 as usize] == Status::Analysed)
                .map(|h| Use {
                    face: h.2,
                    fwd: h.3,
                    third: h.4,
                }),
        );
        let k = uses.len() as u32;
        for w in uses.windows(2) {
            components.union(w[0].face, w[1].face);
        }
        if k >= 3 {
            junction_edges += 1;
        }
        let topo = sides.join_edge(&mesh.pos, u, v, &mut uses);
        if raw == 2 && (k == 2 || k == 0) {
            continue;
        }
        let mut faces: Vec<u32> = group.iter().map(|h| h.2).collect();
        faces.dedup();
        pending.push((
            EdgeFinding {
                vertices: [u, v],
                uses: raw,
                analysed_uses: k,
                faces,
                class: EdgeClass::Unanalysed,
            },
            if k == 0 { EdgeTopo::Manifold } else { topo },
        ));
    }

    let (mut cells, ties, conflicts) =
        cells::build(&mesh, &analysed, sides, components, &bvh, &bbox);
    let self_intersections = intersect::pairs(&mesh, &analysed, &boxes, &bvh);

    // Classify faces, find inverted ones, measure.
    let origin = if analysed.is_empty() {
        [0.0; 3]
    } else {
        [0, 1, 2].map(|k| 0.5 * (bbox.min[k] + bbox.max[k]))
    };
    let n_cells = cells.depth.len();
    let mut cell_volume = vec![0.0f64; n_cells];
    let mut cell_faces = vec![0usize; n_cells];
    let mut internal_faces = Vec::new();
    let mut inverted_faces = Vec::new();
    let mut class_counts: HashMap<FaceClass, usize> = HashMap::new();
    let mut area_m2 = 0.0;
    let mut signed_volume_m3 = 0.0;
    let mut unreachable = false;
    for &f in &analysed {
        let t = mesh.tri(f);
        let vol = signed_volume(&t, origin);
        area_m2 += area(&t);
        signed_volume_m3 += vol;
        let (cf, cb) = (cells.front_cell[f as usize], cells.back_cell[f as usize]);
        let (df, db) = (cells.depth[cf as usize], cells.depth[cb as usize]);
        unreachable |= df == NONE || db == NONE;
        let class = if cf == cb {
            if cf == 0 {
                FaceClass::Exterior
            } else {
                FaceClass::Sheet
            }
        } else if cf == 0 || cb == 0 {
            FaceClass::Boundary
        } else if df == db {
            FaceClass::Partition
        } else {
            FaceClass::NestedShell
        };
        *class_counts.entry(class).or_insert(0) += 1;
        if cf != cb {
            cell_volume[cb as usize] += vol;
            cell_volume[cf as usize] -= vol;
            cell_faces[cb as usize] += 1;
            cell_faces[cf as usize] += 1;
            if df != NONE && db != NONE && df > db {
                inverted_faces.push(f);
            }
        }
        if class != FaceClass::Boundary {
            internal_faces.push(InternalFace {
                face: f,
                class,
                front_cell: cf,
                back_cell: cb,
            });
        }
    }
    let cell_reports: Vec<CellReport> = (1..n_cells)
        .map(|c| CellReport {
            id: c as u32,
            depth: cells.depth[c],
            volume_m3: cell_volume[c],
            faces: cell_faces[c],
        })
        .collect();
    // A fold from +0.0: `Sum` for f64 starts at -0.0, which would print as "-0.0" with no cells.
    let enclosed_volume_m3 = cell_reports.iter().fold(0.0, |acc, c| acc + c.volume_m3);

    // Classify the listed edges.
    let mut edges = Vec::with_capacity(pending.len());
    let mut boundary_open_edges = 0;
    let mut internal_free_edges = 0;
    for (mut finding, topo) in pending {
        finding.class = match topo {
            _ if finding.analysed_uses == 0 => EdgeClass::Unanalysed,
            EdgeTopo::Open(node) => {
                if cells.cell_of(node) == 0 {
                    boundary_open_edges += 1;
                    EdgeClass::BoundaryOpen
                } else {
                    internal_free_edges += 1;
                    EdgeClass::InternalFree
                }
            }
            EdgeTopo::Manifold => EdgeClass::Manifold,
            EdgeTopo::Junction(nodes) => {
                let exterior = nodes.iter().filter(|&&n| cells.cell_of(n) == 0).count();
                match exterior {
                    0 => EdgeClass::InternalJunction,
                    1 => EdgeClass::BoundaryJunction,
                    _ => EdgeClass::PinchedBoundary,
                }
            }
        };
        edges.push(finding);
    }

    let count_of = |c: FaceClass| class_counts.get(&c).copied().unwrap_or(0);
    let mut intersecting: Vec<u32> = self_intersections.iter().flatten().copied().collect();
    intersecting.sort_unstable();
    intersecting.dedup();
    let exterior_faces: Vec<u32> = internal_faces
        .iter()
        .filter(|i| i.class == FaceClass::Exterior)
        .map(|i| i.face)
        .collect();
    let topology_reliable = self_intersections.is_empty()
        && cells.unresolved == 0
        && cells.ambiguous == 0
        && ties == 0
        && !unreachable;

    // Reasons, in code order.
    let mut reasons = Vec::new();
    let mut reason = |code: ReasonCode, count: usize, faces: Vec<u32>, message: String| {
        reasons.push(Reason {
            code,
            count,
            repairable: code.repairable(),
            faces,
            message,
        });
    };
    if nf == 0 {
        reason(
            ReasonCode::EmptyGeometry,
            0,
            Vec::new(),
            "the geometry has no faces".to_string(),
        );
    }
    if !invalid_faces.is_empty() {
        reason(
            ReasonCode::InvalidFaces,
            invalid_faces.len(),
            invalid_faces.iter().map(|f| f.face).collect(),
            format!(
                "{} a vertex index out of range or a non-finite vertex coordinate",
                plural(invalid_faces.len(), "face has", "faces have")
            ),
        );
    }
    if !degenerate_faces.is_empty() {
        reason(
            ReasonCode::DegenerateFaces,
            degenerate_faces.len(),
            degenerate_faces.iter().map(|f| f.face).collect(),
            format!(
                "{} zero area (a repeated vertex, or three collinear vertices)",
                plural(degenerate_faces.len(), "face has", "faces have")
            ),
        );
    }
    if !duplicates.is_empty() {
        reason(
            ReasonCode::DuplicateFaces,
            duplicates.len(),
            duplicates.iter().map(|d| d.face).collect(),
            format!(
                "{} the three vertex positions of an earlier face",
                plural(duplicates.len(), "face repeats", "faces repeat")
            ),
        );
    }
    if !self_intersections.is_empty() {
        reason(
            ReasonCode::SelfIntersections,
            self_intersections.len(),
            intersecting.clone(),
            format!(
                "{} of faces meet other than along a shared edge or at a shared vertex ({})",
                plural(self_intersections.len(), "pair", "pairs"),
                plural(intersecting.len(), "face", "faces"),
            ),
        );
    }
    if boundary_open_edges > 0 || !exterior_faces.is_empty() {
        let mut message = format!(
            "the outer shell is not closed: {} the exterior, and {} the exterior on both sides",
            plural(boundary_open_edges, "open edge faces", "open edges face"),
            plural(exterior_faces.len(), "face has", "faces have"),
        );
        if coincident_vertices > 0 {
            message.push_str(&format!(
                "; {} at the position of another vertex, and welding them (repair) closes the \
                 edges that are only unwelded seams",
                plural(coincident_vertices, "vertex is", "vertices are"),
            ));
        }
        reason(
            ReasonCode::OpenBoundary,
            boundary_open_edges,
            exterior_faces.clone(),
            message,
        );
    }
    if nf > 0 && n_cells <= 1 {
        reason(
            ReasonCode::NoEnclosedVolume,
            0,
            Vec::new(),
            "no face bounds an enclosed volume".to_string(),
        );
    }
    if !inverted_faces.is_empty() {
        reason(
            ReasonCode::InvertedFaces,
            inverted_faces.len(),
            inverted_faces.clone(),
            format!(
                "{} into the volume it bounds (normals must point out of the room, and out of a \
                 nested shell)",
                plural(inverted_faces.len(), "face points", "faces point")
            ),
        );
    }
    if cells.unresolved > 0 || cells.ambiguous > 0 {
        reason(
            ReasonCode::UnresolvedTopology,
            cells.unresolved + cells.ambiguous,
            Vec::new(),
            format!(
                "the placement of {} relative to the others could not be decided",
                plural(
                    cells.unresolved + cells.ambiguous,
                    "face-connected component",
                    "face-connected components"
                )
            ),
        );
    }

    let counts = Counts {
        vertices: nv,
        faces: nf,
        analysed_faces: analysed.len(),
        edges: distinct_edges,
        open_edges: edge_uses.get(&1).copied().unwrap_or(0),
        manifold_edges: edge_uses.get(&2).copied().unwrap_or(0),
        nonmanifold_edges: edge_uses.range(3..).map(|(_, n)| n).sum(),
        edge_uses,
        invalid_faces: invalid_faces.len(),
        degenerate_faces: degenerate_faces.len(),
        duplicate_faces: duplicates.len(),
        self_intersecting_pairs: self_intersections.len(),
        self_intersecting_faces: intersecting.len(),
        boundary_faces: count_of(FaceClass::Boundary),
        partition_faces: count_of(FaceClass::Partition),
        nested_shell_faces: count_of(FaceClass::NestedShell),
        sheet_faces: count_of(FaceClass::Sheet),
        exterior_faces: count_of(FaceClass::Exterior),
        inverted_faces: inverted_faces.len(),
        boundary_open_edges,
        internal_free_edges,
        junction_edges,
        orientation_conflict_edges: conflicts,
        components: cells.components,
        cells: n_cells.saturating_sub(1),
        coincident_vertices,
        unreferenced_vertices,
        non_finite_vertices,
    };
    CheckReport {
        verdict: if reasons.is_empty() {
            Verdict::Ok
        } else {
            Verdict::Refused
        },
        reasons,
        counts,
        measures: Measures {
            area_m2,
            signed_volume_m3,
            enclosed_volume_m3,
        },
        topology_reliable,
        edges,
        invalid_faces,
        degenerate_faces,
        duplicate_faces: duplicates,
        self_intersections,
        inverted_faces,
        internal_faces,
        cells: cell_reports,
        face_cells: (0..nf)
            .map(|f| [cells.front_cell[f], cells.back_cell[f]])
            .collect(),
    }
}
