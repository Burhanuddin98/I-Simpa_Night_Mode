//! A diffuse (Lambert) ray transport of a closed room, written from scratch for M8's reference
//! (`docs/params.md`, "Kuttruff's reference"; Burhan, 2026-09-24 23:14: Kuttruff with `γ²` from
//! the geometry, never fitted to SPPS, and the independent transport as a cross-check).
//!
//! Promoted from the M7 follow-ups' evidence test `tests/lambert_box.rs`, and extended from a box
//! to any closed surface of triangles. **Nothing of SPPS is used**: no tetrahedral mesh, no time
//! stepping, no random generator of its, and no output of any run. A ray moves in a straight line
//! at `c` to the nearest face; there its energy is multiplied by `1 − α` of the face and it is
//! reflected by Lambert's law: a new direction on the side it came from, drawn with a density
//! proportional to the cosine from the face's normal.
//!
//! Two things are computed:
//! - [`free_paths`]: the mean free path between reflections and its relative variance `γ²`
//!   (Kuttruff's; `γ² = (E[ℓ²] − E[ℓ]²)/E[ℓ]²`). **It takes the geometry and the transport's own
//!   settings and nothing else**, and [`FreePaths`] has no public constructor and no fields a
//!   caller can set: a `γ²` fitted to a solver's numbers cannot be given to
//!   [`super::room::kuttruff_rt`], by construction. It refuses its own result when the mean free
//!   path it measured is not Kosten's `4V/S` within its statistical error
//!   ([`MEAN_FREE_PATH_TOLERANCE_SE`]): a room that is not closed, a volume that is not the
//!   surface's, or a transport that does not reflect by Lambert's law.
//! - [`decay`]: the energy of the room, and of receiver balls as SPPS's receivers collect it (the
//!   energy times the length of the path inside the ball, per time bin), from a point source in a
//!   room of given absorption and air, for M8's cross-check against SPPS.
//!
//! Every result is deterministic: replica `r` of a run with seed `s` draws from its own SplitMix64
//! stream, and replicas are combined in their order, whatever the threads did. The spread over the
//! replicas is the statistical error; successive free paths of one ray are correlated, so the
//! spread of independent replicas, not the count of paths, gives it.

use std::f64::consts::PI;

use schemars::JsonSchema;
use serde::Serialize;

use super::decay::{self, Arrival, DecayRange};
use super::noise::Rng;
use super::{EnergySeries, ParamError};

type V3 = [f64; 3];

fn sub(a: V3, b: V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn add_scaled(a: V3, d: V3, s: f64) -> V3 {
    [a[0] + s * d[0], a[1] + s * d[1], a[2] + s * d[2]]
}

fn dot(a: V3, b: V3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn cross(a: V3, b: V3) -> V3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn norm(a: V3) -> f64 {
    dot(a, a).sqrt()
}

fn refused(detail: impl Into<String>) -> ParamError {
    ParamError::TransportRefused {
        detail: detail.into(),
    }
}

/// How far the measured mean free path may lie from `4V/S`, in its standard errors, before
/// [`free_paths`] refuses its own result. With 16 replicas the standard error is itself known to
/// about a fifth, so 6 leaves a correct transport a chance of about 10⁻⁴ of being refused, and
/// since every run is deterministic, a given room is either always accepted or always refused.
pub const MEAN_FREE_PATH_TOLERANCE_SE: f64 = 6.0;

/// Barycentric slack of the ray–triangle test: a ray through a shared edge hits both faces, so no
/// ray slips between two faces of a closed surface.
const EDGE_SLACK: f64 = 1e-9;

/// After each reflection the ray starts this far (times the room's size) inside the face it
/// left, so that rounding never starts it outside the room or on a neighbouring face of the same
/// plane. It shortens nothing measurably: 10⁻¹⁰ of the room's size per path.
const NUDGE: f64 = 1e-10;

/// One face, ready for the ray test.
#[derive(Clone, Copy, Debug)]
struct Face {
    a: V3,
    e1: V3,
    e2: V3,
    /// Unit normal, either side.
    n: V3,
    /// `|e1 × e2|`, twice the area.
    area2: f64,
}

/// A node of the bounding-volume hierarchy: a leaf holds `count > 0` faces from `start` in
/// [`Enclosure::order`]; an inner node has its left child next to it and its right at `right`.
#[derive(Clone, Copy, Debug)]
struct Node {
    lo: V3,
    hi: V3,
    start: u32,
    count: u32,
    right: u32,
}

const LEAF_SIZE: usize = 4;

/// A closed room: a surface of triangles, its volume, and a point inside it where rays start.
///
/// Built from geometry only ([`Enclosure::shoebox`], [`Enclosure::from_triangles`]). The surface
/// need not be convex, and its faces need not be oriented: a ray is reflected to the side it came
/// from. It must be closed: a ray that leaves it refuses the transport.
#[derive(Clone, Debug)]
pub struct Enclosure {
    faces: Vec<Face>,
    nodes: Vec<Node>,
    order: Vec<u32>,
    volume_m3: f64,
    area_m2: f64,
    interior_m: V3,
    /// The diagonal of the bounding box, m.
    size_m: f64,
}

impl Enclosure {
    /// A box `[0, x] × [0, y] × [0, z]`, each side as two triangles in the order −x, +x, −y, +y,
    /// −z, +z; rays start at its centre. Refused, `params_transport_refused`, when a side is not a
    /// positive finite length.
    pub fn shoebox(size_m: [f64; 3]) -> Result<Enclosure, ParamError> {
        if size_m.iter().any(|s| !s.is_finite() || *s <= 0.0) {
            return Err(refused(format!(
                "a box's sides must be positive lengths, not {size_m:?}"
            )));
        }
        let [x, y, z] = size_m;
        let corner = |i: usize| -> V3 {
            [
                if i & 1 != 0 { x } else { 0.0 },
                if i & 2 != 0 { y } else { 0.0 },
                if i & 4 != 0 { z } else { 0.0 },
            ]
        };
        // Each side's four corners, in order around it.
        let sides: [[usize; 4]; 6] = [
            [0, 2, 6, 4],
            [1, 3, 7, 5],
            [0, 1, 5, 4],
            [2, 3, 7, 6],
            [0, 1, 3, 2],
            [4, 5, 7, 6],
        ];
        let mut triangles = Vec::with_capacity(12);
        for s in sides {
            let [a, b, c, d] = s.map(corner);
            triangles.push([a, b, c]);
            triangles.push([a, c, d]);
        }
        Enclosure::from_triangles(&triangles, x * y * z, [x / 2.0, y / 2.0, z / 2.0])
    }

    /// A room bounded by `triangles`, of `volume_m3`, with rays starting at `interior_m`, which
    /// must lie inside it. Refused, `params_transport_refused`, for no triangles, a coordinate
    /// that is not finite, no area, a volume that is not a positive number, or an interior point
    /// that is not finite. A surface that is not closed, or a point that is not inside it, is
    /// found by the transport itself: its rays leave.
    pub fn from_triangles(
        triangles: &[[V3; 3]],
        volume_m3: f64,
        interior_m: V3,
    ) -> Result<Enclosure, ParamError> {
        if triangles.is_empty() {
            return Err(refused("the enclosure has no faces"));
        }
        if let Some((i, t)) = triangles
            .iter()
            .enumerate()
            .find(|(_, t)| t.iter().flatten().any(|c| !c.is_finite()))
        {
            return Err(refused(format!(
                "face {i} has a coordinate that is not finite: {t:?}"
            )));
        }
        if !volume_m3.is_finite() || volume_m3 <= 0.0 {
            return Err(refused(format!(
                "the volume {volume_m3} m³ is not a positive number"
            )));
        }
        if interior_m.iter().any(|c| !c.is_finite()) {
            return Err(refused(format!(
                "the interior point {interior_m:?} is not finite"
            )));
        }
        let faces: Vec<Face> = triangles
            .iter()
            .map(|&[a, b, c]| {
                let (e1, e2) = (sub(b, a), sub(c, a));
                let n = cross(e1, e2);
                let area2 = norm(n);
                let n = if area2 > 0.0 {
                    n.map(|v| v / area2)
                } else {
                    [0.0; 3]
                };
                Face {
                    a,
                    e1,
                    e2,
                    n,
                    area2,
                }
            })
            .collect();
        let area_m2: f64 = faces.iter().map(|f| 0.5 * f.area2).sum();
        if !(area_m2 > 0.0 && area_m2.is_finite()) {
            return Err(refused(format!(
                "the enclosure's area is {area_m2} m², not a positive number"
            )));
        }
        let (mut lo, mut hi) = ([f64::INFINITY; 3], [f64::NEG_INFINITY; 3]);
        for p in triangles.iter().flatten() {
            for k in 0..3 {
                lo[k] = lo[k].min(p[k]);
                hi[k] = hi[k].max(p[k]);
            }
        }
        let size_m = norm(sub(hi, lo));
        let mut order: Vec<u32> = (0..faces.len() as u32).collect();
        let mut nodes = Vec::with_capacity(2 * faces.len() / LEAF_SIZE + 1);
        build(&faces, &mut order, 0, faces.len(), &mut nodes, size_m);
        Ok(Enclosure {
            faces,
            nodes,
            order,
            volume_m3,
            area_m2,
            interior_m,
            size_m,
        })
    }

    /// The volume it was given, m³.
    pub fn volume_m3(&self) -> f64 {
        self.volume_m3
    }

    /// The faces' total area, m².
    pub fn area_m2(&self) -> f64 {
        self.area_m2
    }

    /// Where rays start.
    pub fn interior_m(&self) -> V3 {
        self.interior_m
    }

    /// The number of faces.
    pub fn face_count(&self) -> usize {
        self.faces.len()
    }

    /// Face `i`'s area, m².
    pub fn face_area_m2(&self, i: usize) -> f64 {
        0.5 * self.faces[i].area2
    }

    /// Kosten's mean free path of a diffuse field, `4V/S`, m.
    pub fn four_v_over_s_m(&self) -> f64 {
        4.0 * self.volume_m3 / self.area_m2
    }

    /// The nearest face along `o + t·d`, `t > 0`, other than `skip`: `(t, face)`.
    fn nearest(&self, o: V3, d: V3, skip: u32) -> Option<(f64, u32)> {
        let inv = d.map(|v| 1.0 / v);
        let mut best: Option<(f64, u32)> = None;
        let mut stack = [0u32; 64];
        let mut top = 1;
        while top > 0 {
            top -= 1;
            let index = stack[top];
            let node = self.nodes[index as usize];
            let limit = best.map_or(f64::INFINITY, |b| b.0);
            if !slab(&node, o, inv, limit) {
                continue;
            }
            if node.count > 0 {
                for &f in &self.order[node.start as usize..(node.start + node.count) as usize] {
                    if f == skip {
                        continue;
                    }
                    if let Some(t) = intersect(&self.faces[f as usize], o, d)
                        && t > 0.0
                        && best.is_none_or(|b| t < b.0)
                    {
                        best = Some((t, f));
                    }
                }
            } else {
                // Both children: the left is the node right after this one ([`build`]). The stack
                // holds at most the tree's depth plus one.
                stack[top] = node.right;
                stack[top + 1] = index + 1;
                top += 2;
            }
        }
        best
    }
}

/// Builds the hierarchy over `order[start..end]`, splitting at the median centroid along the
/// longest axis; returns the node's index.
fn build(
    faces: &[Face],
    order: &mut [u32],
    start: usize,
    end: usize,
    nodes: &mut Vec<Node>,
    size_m: f64,
) -> u32 {
    let pad = 1e-9 * size_m.max(f64::MIN_POSITIVE);
    let (mut lo, mut hi) = ([f64::INFINITY; 3], [f64::NEG_INFINITY; 3]);
    let (mut clo, mut chi) = ([f64::INFINITY; 3], [f64::NEG_INFINITY; 3]);
    for &f in &order[start..end] {
        let f = &faces[f as usize];
        let pts = [f.a, add_scaled(f.a, f.e1, 1.0), add_scaled(f.a, f.e2, 1.0)];
        for p in pts {
            for k in 0..3 {
                lo[k] = lo[k].min(p[k]);
                hi[k] = hi[k].max(p[k]);
            }
        }
        let c = centroid(f);
        for k in 0..3 {
            clo[k] = clo[k].min(c[k]);
            chi[k] = chi[k].max(c[k]);
        }
    }
    let lo = lo.map(|v| v - pad);
    let hi = hi.map(|v| v + pad);
    let index = nodes.len() as u32;
    let count = end - start;
    let axis = (0..3)
        .max_by(|&a, &b| (chi[a] - clo[a]).total_cmp(&(chi[b] - clo[b])))
        .unwrap_or(0);
    if count <= LEAF_SIZE || chi[axis] - clo[axis] <= 0.0 {
        nodes.push(Node {
            lo,
            hi,
            start: start as u32,
            count: count as u32,
            right: 0,
        });
        return index;
    }
    let mid = start + count / 2;
    order[start..end].select_nth_unstable_by(count / 2, |&a, &b| {
        centroid(&faces[a as usize])[axis].total_cmp(&centroid(&faces[b as usize])[axis])
    });
    nodes.push(Node {
        lo,
        hi,
        start: 0,
        count: 0,
        right: 0,
    });
    build(faces, order, start, mid, nodes, size_m);
    let right = build(faces, order, mid, end, nodes, size_m);
    nodes[index as usize].right = right;
    index
}

fn centroid(f: &Face) -> V3 {
    [
        f.a[0] + (f.e1[0] + f.e2[0]) / 3.0,
        f.a[1] + (f.e1[1] + f.e2[1]) / 3.0,
        f.a[2] + (f.e1[2] + f.e2[2]) / 3.0,
    ]
}

/// Whether the ray `o + t·d` (`inv` = `1/d` componentwise) meets the node's box for some
/// `0 ≤ t < limit`.
fn slab(n: &Node, o: V3, inv: V3, limit: f64) -> bool {
    let (mut t0, mut t1) = (0.0f64, limit);
    for k in 0..3 {
        let a = (n.lo[k] - o[k]) * inv[k];
        let b = (n.hi[k] - o[k]) * inv[k];
        // A component of 0 gives ±inf (or NaN when the origin is on the slab's face): f64::min and
        // max return the other operand for NaN, which keeps the slab open, as it is.
        t0 = t0.max(a.min(b));
        t1 = t1.min(a.max(b));
    }
    t0 <= t1
}

/// Möller–Trumbore, with [`EDGE_SLACK`] on the barycentric bounds: the ray parameter `t`, or
/// `None` when the ray is parallel to the face or misses it.
fn intersect(f: &Face, o: V3, d: V3) -> Option<f64> {
    let p = cross(d, f.e2);
    let det = dot(f.e1, p);
    if det.abs() <= 1e-14 * f.area2 {
        return None;
    }
    let inv = 1.0 / det;
    let s = sub(o, f.a);
    let u = dot(s, p) * inv;
    if !(-EDGE_SLACK..=1.0 + EDGE_SLACK).contains(&u) {
        return None;
    }
    let q = cross(s, f.e1);
    let v = dot(d, q) * inv;
    if v < -EDGE_SLACK || u + v > 1.0 + EDGE_SLACK {
        return None;
    }
    Some(dot(f.e2, q) * inv)
}

/// An isotropic direction.
fn isotropic(rng: &mut Rng) -> V3 {
    let z = 2.0 * rng.uniform() - 1.0;
    let phi = 2.0 * PI * rng.uniform();
    let s = (1.0 - z * z).max(0.0).sqrt();
    [s * phi.cos(), s * phi.sin(), z]
}

/// A direction on the side of `n` (a unit normal), by Lambert's law: `cos θ = √u`, the density
/// `cos θ·sin θ/π` per steradian. With `uniform` (a test fault only) `cos θ = u`: uniform over the
/// hemisphere, which is not Lambert's law.
fn lambert(n: V3, rng: &mut Rng, uniform: bool) -> V3 {
    let u = rng.uniform();
    let cos_t = if uniform { u } else { u.sqrt() };
    let sin_t = (1.0 - cos_t * cos_t).max(0.0).sqrt();
    let psi = 2.0 * PI * rng.uniform();
    // An orthonormal basis about n (Duff et al., "Building an orthonormal basis, revisited",
    // JCGT 6(1), 2017).
    let sign = 1.0f64.copysign(n[2]);
    let a = -1.0 / (sign + n[2]);
    let b = n[0] * n[1] * a;
    let t1 = [1.0 + sign * n[0] * n[0] * a, sign * b, -sign * n[0]];
    let t2 = [b, sign + n[1] * n[1] * a, -n[1]];
    let (c, s) = (sin_t * psi.cos(), sin_t * psi.sin());
    [
        cos_t * n[0] + c * t1[0] + s * t2[0],
        cos_t * n[1] + c * t1[1] + s * t2[1],
        cos_t * n[2] + c * t1[2] + s * t2[2],
    ]
}

/// A ray in the room: where it is, where it goes, and the face it left.
struct Ray {
    p: V3,
    d: V3,
    face: u32,
}

impl Ray {
    fn from_source(p: V3, rng: &mut Rng) -> Ray {
        Ray {
            p,
            d: isotropic(rng),
            face: u32::MAX,
        }
    }

    /// The distance to the next face, and the face; a refusal when the ray leaves the room.
    fn run(&self, e: &Enclosure, reflections: u64) -> Result<(f64, u32), ParamError> {
        e.nearest(self.p, self.d, self.face).ok_or_else(|| {
            refused(format!(
                "a ray left the enclosure after {reflections} reflections, from {:?} towards \
                 {:?}: the surface is not closed there, or the start {:?} is not inside it",
                self.p, self.d, e.interior_m
            ))
        })
    }

    /// Moves `run` to `face` and reflects there by Lambert's law.
    fn reflect(&mut self, e: &Enclosure, run: f64, face: u32, rng: &mut Rng, uniform: bool) {
        let n = e.faces[face as usize].n;
        // The side the ray came from.
        let inward = if dot(self.d, n) < 0.0 {
            n
        } else {
            n.map(|v| -v)
        };
        let hit = add_scaled(self.p, self.d, run);
        self.p = add_scaled(hit, inward, NUDGE * e.size_m);
        self.d = lambert(inward, rng, uniform);
        self.face = face;
    }
}

/// The settings of [`free_paths`]: the transport's own, nothing of any solver.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, JsonSchema)]
pub struct FreePathSettings {
    /// Independent replicas; their spread is the statistical error. At least 2.
    pub replicas: u32,
    /// Rays per replica, each started at the enclosure's interior point in an isotropic direction.
    pub rays_per_replica: u32,
    /// Free paths of each ray left out before counting, the first from the start point among
    /// them, while the ray forgets where it started.
    pub burn_in_paths: u32,
    /// Free paths counted per ray after the burn-in.
    pub paths_per_ray: u32,
    pub seed: u64,
}

impl FreePathSettings {
    /// What `core::results` uses: 16 × 1024 rays of 16 + 64 paths, 1,048,576 paths counted.
    pub const STANDARD: FreePathSettings = FreePathSettings {
        replicas: 16,
        rays_per_replica: 1024,
        burn_in_paths: 16,
        paths_per_ray: 64,
        seed: 0x6c61_6d62_6572_7431,
    };
}

/// The free paths between reflections of a diffuse (Lambert) transport in one enclosure: their
/// mean and relative variance `γ²`, with their statistical errors, and the enclosure's `V` and `S`.
///
/// **Made only by [`free_paths`]**, from an [`Enclosure`] and [`FreePathSettings`]. Its fields
/// are private and it has no other constructor, so no `γ²` fitted to a solver's output can reach
/// [`super::room::kuttruff_rt`]:
///
/// ```
/// use simpa_core::params::lambert::{free_paths, Enclosure, FreePathSettings};
/// use simpa_core::params::room::{kuttruff_rt, RtConstant, Surface};
///
/// let room = Enclosure::shoebox([5.0, 4.0, 3.0]).unwrap();
/// let paths = free_paths(&room, &FreePathSettings { rays_per_replica: 64, ..FreePathSettings::STANDARD }).unwrap();
/// let walls = [Surface { area_m2: room.area_m2(), absorption: 0.2 }];
/// let t = kuttruff_rt(&paths, &walls, None, RtConstant::Physical { speed_of_sound: 343.2 }).unwrap();
/// assert!(t > 0.0);
/// ```
///
/// The same with a `γ²` set by hand does not compile:
///
/// ```compile_fail
/// use simpa_core::params::lambert::{FreePathSettings, FreePaths};
/// use simpa_core::params::room::{kuttruff_rt, RtConstant, Surface};
///
/// let paths = FreePaths {
///     mean_free_path_m: 2.553,
///     mean_free_path_se_m: 0.0,
///     gamma2: 0.4,
///     gamma2_se: 0.0,
///     four_v_over_s_m: 2.553,
///     volume_m3: 60.0,
///     area_m2: 94.0,
///     paths: 1,
///     settings: FreePathSettings::STANDARD,
/// };
/// let walls = [Surface { area_m2: 94.0, absorption: 0.2 }];
/// let t = kuttruff_rt(&paths, &walls, None, RtConstant::Physical { speed_of_sound: 343.2 }).unwrap();
/// ```
#[derive(Clone, Debug, PartialEq, Serialize, JsonSchema)]
pub struct FreePaths {
    mean_free_path_m: f64,
    mean_free_path_se_m: f64,
    gamma2: f64,
    gamma2_se: f64,
    four_v_over_s_m: f64,
    volume_m3: f64,
    area_m2: f64,
    paths: u64,
    settings: FreePathSettings,
}

impl FreePaths {
    /// The mean free path, m.
    pub fn mean_free_path_m(&self) -> f64 {
        self.mean_free_path_m
    }

    /// Its standard error over the replicas, m.
    pub fn mean_free_path_se_m(&self) -> f64 {
        self.mean_free_path_se_m
    }

    /// `γ²`, the free paths' relative variance.
    pub fn gamma2(&self) -> f64 {
        self.gamma2
    }

    /// Its standard error over the replicas.
    pub fn gamma2_se(&self) -> f64 {
        self.gamma2_se
    }

    /// Kosten's `4V/S` of the enclosure, m.
    pub fn four_v_over_s_m(&self) -> f64 {
        self.four_v_over_s_m
    }

    /// The enclosure's volume, m³.
    pub fn volume_m3(&self) -> f64 {
        self.volume_m3
    }

    /// The enclosure's area, m².
    pub fn area_m2(&self) -> f64 {
        self.area_m2
    }

    /// The free paths counted.
    pub fn paths(&self) -> u64 {
        self.paths
    }

    /// The settings they were traced with.
    pub fn settings(&self) -> FreePathSettings {
        self.settings
    }
}

/// Whether this thread runs the transport with a test fault (`crate::faults`): read on the
/// calling thread and handed to the replicas' threads, which do not share its fault.
fn uniform_fault() -> bool {
    matches!(
        crate::faults::active(),
        Some(crate::faults::Fault::LambertUniformReflection)
    )
}

/// The seed of replica `r`: a SplitMix64 step from the run's seed and the replica's number.
fn replica_seed(seed: u64, r: u32) -> u64 {
    Rng::new(seed ^ u64::from(r).wrapping_mul(0xd1b5_4a32_d192_ed03)).next_u64()
}

/// Runs `work` for replicas `0..replicas` on as many threads as the machine offers, and returns
/// their results in replica order.
fn per_replica<T: Send>(
    replicas: u32,
    work: impl Fn(u32) -> Result<T, ParamError> + Sync,
) -> Result<Vec<T>, ParamError> {
    let threads = std::thread::available_parallelism()
        .map_or(1, |n| n.get())
        .clamp(1, replicas.max(1) as usize);
    let mut out: Vec<Option<Result<T, ParamError>>> = (0..replicas).map(|_| None).collect();
    std::thread::scope(|scope| {
        let work = &work;
        let handles: Vec<_> = (0..threads)
            .map(|w| {
                scope.spawn(move || {
                    (0..replicas)
                        .filter(|r| *r as usize % threads == w)
                        .map(|r| (r, work(r)))
                        .collect::<Vec<_>>()
                })
            })
            .collect();
        for h in handles {
            for (r, res) in h.join().expect("a transport thread panicked") {
                out[r as usize] = Some(res);
            }
        }
    });
    out.into_iter()
        .map(|r| r.expect("every replica ran"))
        .collect()
}

/// Mean and standard error of the mean of `v`.
fn mean_se(v: &[f64]) -> (f64, f64) {
    let n = v.len() as f64;
    let m = v.iter().sum::<f64>() / n;
    let var = v.iter().map(|x| (x - m).powi(2)).sum::<f64>() / (n - 1.0);
    (m, (var / n).sqrt())
}

/// The free paths of a diffuse (Lambert) transport in `enclosure` ([`FreePaths`]). Refused,
/// `params_transport_refused`, for settings with fewer than 2 replicas or no ray or no path, a ray
/// that leaves the enclosure, or a mean free path further from `4V/S` than
/// [`MEAN_FREE_PATH_TOLERANCE_SE`] standard errors.
pub fn free_paths(
    enclosure: &Enclosure,
    settings: &FreePathSettings,
) -> Result<FreePaths, ParamError> {
    let s = *settings;
    if s.replicas < 2 || s.rays_per_replica == 0 || s.paths_per_ray == 0 {
        return Err(refused(format!(
            "the free-path settings need at least 2 replicas, a ray and a path: {s:?}"
        )));
    }
    let uniform = uniform_fault();
    let sums = per_replica(s.replicas, |r| {
        let mut rng = Rng::new(replica_seed(s.seed, r));
        let (mut n, mut s1, mut s2) = (0u64, 0.0f64, 0.0f64);
        let mut reflections = 0u64;
        for _ in 0..s.rays_per_replica {
            let mut ray = Ray::from_source(enclosure.interior_m, &mut rng);
            // Path 0 runs from the start point; paths 1 to burn_in are left out too.
            for k in 0..=u64::from(s.burn_in_paths) + u64::from(s.paths_per_ray) {
                let (run, face) = ray.run(enclosure, reflections)?;
                if k > u64::from(s.burn_in_paths) {
                    n += 1;
                    s1 += run;
                    s2 += run * run;
                }
                ray.reflect(enclosure, run, face, &mut rng, uniform);
                reflections += 1;
            }
        }
        Ok((n, s1, s2))
    })?;
    let (mut n, mut s1, mut s2) = (0u64, 0.0, 0.0);
    let (mut means, mut gammas) = (Vec::new(), Vec::new());
    for &(rn, r1, r2) in &sums {
        n += rn;
        s1 += r1;
        s2 += r2;
        let m = r1 / rn as f64;
        means.push(m);
        gammas.push((r2 / rn as f64) / (m * m) - 1.0);
    }
    let mean = s1 / n as f64;
    let gamma2 = (s2 / n as f64) / (mean * mean) - 1.0;
    let (_, mean_error) = mean_se(&means);
    let (_, gamma2_se) = mean_se(&gammas);
    let four_v_over_s_m = enclosure.four_v_over_s_m();
    let off = (mean - four_v_over_s_m).abs();
    if off > MEAN_FREE_PATH_TOLERANCE_SE * mean_error + 1e-9 * four_v_over_s_m {
        return Err(refused(format!(
            "the mean free path is {mean} ± {mean_error} m, and 4V/S is {four_v_over_s_m} m: {:.1} \
             standard errors apart, more than {MEAN_FREE_PATH_TOLERANCE_SE}. Diffuse reflection \
             in a closed room of this volume gives 4V/S; the surface is not closed, the volume is \
             not the surface's, or the reflection is not Lambert's",
            off / mean_error.max(f64::MIN_POSITIVE)
        )));
    }
    Ok(FreePaths {
        mean_free_path_m: mean,
        mean_free_path_se_m: mean_error,
        gamma2,
        gamma2_se,
        four_v_over_s_m,
        volume_m3: enclosure.volume_m3,
        area_m2: enclosure.area_m2,
        paths: n,
        settings: s,
    })
}

/// The settings of [`decay`].
#[derive(Clone, Debug, PartialEq, Serialize, JsonSchema)]
pub struct DecaySettings {
    /// The point source, inside the room.
    pub source_m: V3,
    /// Receiver balls' centres.
    pub receivers_m: Vec<V3>,
    pub receiver_radius_m: f64,
    pub speed_of_sound_m_s: f64,
    /// The width of a time bin, s.
    pub time_step_s: f64,
    /// Rays are followed to this time, s.
    pub duration_s: f64,
    /// The energy attenuation `m` of the air, 1/m: every ray's energy falls as `e^(−m·c·t)`.
    /// `None` for none.
    pub air_m_per_metre: Option<f64>,
    /// Independent replicas, at least 2.
    pub replicas: u32,
    pub rays_per_replica: u32,
    pub seed: u64,
}

/// One replica of [`decay`]: per time bin, the room's energy (the sum over the rays of their
/// energy times the time they spent in the bin), and each receiver's (the sum over the rays of
/// their energy times the length of path inside the ball).
#[derive(Clone, Debug, PartialEq)]
pub struct DecayReplica {
    pub room: Vec<f64>,
    pub receivers: Vec<Vec<f64>>,
}

/// What [`decay`] traced.
#[derive(Clone, Debug, PartialEq)]
pub struct Decay {
    pub time_step_s: f64,
    pub replicas: Vec<DecayReplica>,
}

/// A value over replicas: their mean, its standard error, and each replica's.
#[derive(Clone, Debug, PartialEq, Serialize, JsonSchema)]
pub struct OverReplicas {
    pub mean: f64,
    pub se: f64,
    pub values: Vec<f64>,
}

impl Decay {
    /// The T30 of the room's energy, read through `params::decay` as a series from `t = 0` (the
    /// start of the source), over the replicas. Refused as `params::decay` refuses one replica's.
    pub fn room_t30(&self) -> Result<OverReplicas, ParamError> {
        self.over_replicas(|r| &r.room)
    }

    /// The T30 of receiver `i`, over the replicas, with the direct sound at `arrival` (the
    /// distance from the source over `c`, spread over `R/c`).
    pub fn receiver_t30(&self, i: usize, arrival: Arrival) -> Result<OverReplicas, ParamError> {
        let mut values = Vec::with_capacity(self.replicas.len());
        for r in &self.replicas {
            let s = EnergySeries::new(self.time_step_s, r.receivers[i].clone())?;
            values.push(decay::decay_time(&s, arrival, DecayRange::T30)?.t_s);
        }
        let (mean, se) = mean_se(&values);
        Ok(OverReplicas { mean, se, values })
    }

    fn over_replicas(
        &self,
        series: impl Fn(&DecayReplica) -> &Vec<f64>,
    ) -> Result<OverReplicas, ParamError> {
        let mut values = Vec::with_capacity(self.replicas.len());
        for r in &self.replicas {
            let s = EnergySeries::new(self.time_step_s, series(r).clone())?;
            values.push(decay::decay_time(&s, Arrival::at(0.0), DecayRange::T30)?.t_s);
        }
        let (mean, se) = mean_se(&values);
        Ok(OverReplicas { mean, se, values })
    }
}

/// Adds `value` spread evenly over `[t0, t1)` into `bins` of width `dt`.
fn spread(bins: &mut [f64], dt: f64, t0: f64, t1: f64, value: f64) {
    if t1 <= t0 {
        return;
    }
    let rate = value / (t1 - t0);
    // The bin index advances by itself: a time at a bin edge, divided by dt, can round down to the
    // bin before.
    let (mut t, mut k) = (t0, (t0 / dt) as usize);
    while t < t1 && k < bins.len() {
        let edge = ((k + 1) as f64 * dt).min(t1);
        if edge > t {
            bins[k] += rate * (edge - t);
            t = edge;
        }
        k += 1;
    }
}

/// The energy of the room and of its receivers after an impulse from a point source, traced by a
/// diffuse (Lambert) transport in `enclosure` with face `i` absorbing `absorption[i]`
/// ([module docs](self)). Refused, `params_transport_refused`, for an absorption list that is not
/// one α in [0, 1] per face, settings that are not finite and positive (a radius of at least 0,
/// air at least 0, at least 2 replicas and a ray), or a ray that leaves the enclosure.
///
/// Air is applied per bin: every ray's energy falls as `e^(−m·c·t)` whatever its path, so bin `k`
/// is multiplied by that factor's mean over the bin, `e^(−m·c·k·dt)·(1 − e^(−m·c·dt))/(m·c·dt)`,
/// exact for energy spread evenly in the bin.
pub fn decay(
    enclosure: &Enclosure,
    absorption: &[f64],
    settings: &DecaySettings,
) -> Result<Decay, ParamError> {
    let s = settings;
    if absorption.len() != enclosure.faces.len() {
        return Err(refused(format!(
            "{} absorption coefficients for {} faces",
            absorption.len(),
            enclosure.faces.len()
        )));
    }
    if let Some((i, a)) = absorption
        .iter()
        .enumerate()
        .find(|(_, a)| !(0.0..=1.0).contains(*a))
    {
        return Err(refused(format!("face {i} has α = {a}, not in [0, 1]")));
    }
    let positive = |v: f64| v.is_finite() && v > 0.0;
    let air = s.air_m_per_metre.unwrap_or(0.0);
    if !positive(s.speed_of_sound_m_s)
        || !positive(s.time_step_s)
        || !positive(s.duration_s)
        || !(s.receiver_radius_m.is_finite() && s.receiver_radius_m >= 0.0)
        || !(air.is_finite() && air >= 0.0)
        || s.replicas < 2
        || s.rays_per_replica == 0
        || s.source_m.iter().any(|c| !c.is_finite())
        || s.receivers_m.iter().flatten().any(|c| !c.is_finite())
    {
        return Err(refused(format!("the decay settings are not usable: {s:?}")));
    }
    let bins = (s.duration_s / s.time_step_s).ceil() as usize;
    let c = s.speed_of_sound_m_s;
    let r2 = s.receiver_radius_m * s.receiver_radius_m;
    let uniform = uniform_fault();
    let mut replicas = per_replica(s.replicas, |r| {
        let mut rng = Rng::new(replica_seed(s.seed, r));
        let mut room = vec![0.0; bins];
        let mut receivers = vec![vec![0.0; bins]; s.receivers_m.len()];
        let mut reflections = 0u64;
        for _ in 0..s.rays_per_replica {
            let mut ray = Ray::from_source(s.source_m, &mut rng);
            let (mut t, mut w) = (0.0f64, 1.0f64);
            while t < s.duration_s && w > 0.0 {
                let (run, face) = ray.run(enclosure, reflections)?;
                let t1 = t + run / c;
                spread(&mut room, s.time_step_s, t, t1, w * (t1 - t));
                for (ri, q) in s.receivers_m.iter().enumerate() {
                    let m = sub(ray.p, *q);
                    let b = dot(m, ray.d);
                    let disc = b * b - (dot(m, m) - r2);
                    if disc <= 0.0 {
                        continue;
                    }
                    let root = disc.sqrt();
                    let (x0, x1) = ((-b - root).max(0.0), (-b + root).min(run));
                    if x1 > x0 {
                        spread(
                            &mut receivers[ri],
                            s.time_step_s,
                            t + x0 / c,
                            t + x1 / c,
                            w * (x1 - x0),
                        );
                    }
                }
                w *= 1.0 - absorption[face as usize];
                ray.reflect(enclosure, run, face, &mut rng, uniform);
                reflections += 1;
                t = t1;
            }
        }
        Ok(DecayReplica { room, receivers })
    })?;
    if air > 0.0 {
        let x = air * c * s.time_step_s;
        let mean = (1.0 - (-x).exp()) / x;
        for r in &mut replicas {
            for series in std::iter::once(&mut r.room).chain(r.receivers.iter_mut()) {
                for (k, v) in series.iter_mut().enumerate() {
                    *v *= (-x * k as f64).exp() * mean;
                }
            }
        }
    }
    Ok(Decay {
        time_step_s: s.time_step_s,
        replicas,
    })
}
