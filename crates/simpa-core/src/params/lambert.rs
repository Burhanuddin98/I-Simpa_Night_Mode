//! A diffuse (Lambert) ray transport of a closed room, written from scratch for M8's reference
//! (`docs/params.md`, "Kuttruff's reference"; Burhan, 2026-09-24 23:14: Kuttruff with `γ²` from
//! the geometry, never fitted to SPPS, and the independent transport as a cross-check).
//!
//! Promoted from the M7 follow-ups' evidence test `tests/lambert_box.rs`, and extended from a box
//! to any closed surface of triangles. **Nothing of SPPS is used**: no mesh walking, no time
//! stepping, no random generator of its, and no output of any run. A ray moves in a straight line
//! at `c` to the nearest face; there its energy is multiplied by `1 − α` of the face and it is
//! reflected by Lambert's law: a new direction on the side it came from, drawn with a density
//! proportional to the cosine from the face's normal. The room's volume is given as tetrahedra
//! that fill it; they say only where [`free_paths`]' rays may start (anywhere in the room,
//! evenly), and rays move against the surface alone.
//!
//! Two things are computed:
//! - [`free_paths`]: the mean free path between reflections and its relative variance `γ²`
//!   (Kuttruff's; `γ² = (E[ℓ²] − E[ℓ]²)/E[ℓ]²`). **It takes the room and nothing else**: its rays,
//!   paths and seed are fixed ([`FreePathSettings::STANDARD`]), and [`FreePaths`] has no public
//!   constructor and no fields a caller can set. So a `γ²` fitted to a solver's numbers cannot be
//!   given to [`super::room::kuttruff_rt`], by construction, and neither can one fished for by
//!   trying seeds or ray counts until the transport happens to give the wanted value. What is left
//!   to a caller is the description of the room (its placement, its faces, its tetrahedra), which
//!   draws the transport's randomness otherwise; [`GAMMA2_SE_LIMIT`] bounds what that can move.
//!   It refuses its own result when the mean free path it measured is not Kosten's `4V/S` within
//!   its statistical error ([`MEAN_FREE_PATH_TOLERANCE_SE`]): a room that is not closed, a volume
//!   that is not the surface's, a face with the room on both its sides (it reflects on both, so
//!   the field sees its area twice), parts of a room that exchange too little sound to mix within
//!   the rays' paths, or a transport that does not reflect by Lambert's law; and when `γ²`'s
//!   standard error is above [`GAMMA2_SE_LIMIT`].
//! - [`decay`]: the energy of the room, and of receiver balls as SPPS's receivers collect it (the
//!   energy times the length of the path inside the ball, per time bin), from a point source in a
//!   room of given absorption and air, for M8's cross-check against SPPS.
//!
//! **Studies of the transport** (`free_path_study`, other settings and starting points, and the
//! correlation of successive paths) exist in test builds only, behind the `transport-study`
//! feature, and give a `FreePathStudy`, which no function of `params::room` takes.
//!
//! Every result is deterministic: replica `r` of a run with seed `s` draws from its own SplitMix64
//! stream, and replicas are combined in their order, whatever the threads did. The spread over the
//! replicas is the statistical error; successive free paths of one ray are correlated, so the
//! spread of independent replicas, not the count of paths, gives it.
//!
//! **Known answers** (`tests/params_lambert.rs`; `docs/params.md`, "Kuttruff's reference"). In a
//! closed room, Lambert reflection keeps a uniform, isotropic field, so the free paths between
//! reflections have the mean `4V/S` in any room. In a convex room they are the chords of lines
//! that are uniform and isotropic in space, whose mean square integral geometry gives as
//! `⟨ℓ²⟩ = (2/(π·S))·∫∫ |x − y|⁻² dx dy` over pairs of points in the room: `γ²` = 1/8 for a
//! sphere, 0.344950 for a cube (from Bailey, Borwein and Crandall's closed form of that integral),
//! 0.388874 for M8's 6×10×3 m room and 0.352401 for its 5×4×3 m one. The transport reproduces each
//! within its statistical error. **Where rays start matters**: a field begun at one point is not
//! yet diffuse, and a ray's first reflections remember it. The M7 follow-ups' `lambert_box.rs`
//! started every ray at the source and counted from the first reflection; measured with this
//! transport, that reads `γ²` 0.0035 to 0.005 low and the mean free path 0.16 to 0.27 % long at
//! 64 paths a ray, and, over the 482 paths a ray it counted at α 0.05, gives its 0.388 and 0.352
//! (0.38812 and 0.35187, `tests/params_lambert.rs`); rays started in the narrow wing of an
//! L-shaped room give a mean
//! free path 0.3 % short after 16 reflections. So [`free_paths`] starts each ray at a point drawn
//! evenly from the room's volume, and leaves out its first paths as well
//! ([`FreePathSettings::burn_in_paths`]).
//!
//! **Successive free paths are not independent** in a room with flat walls: a long path tends to
//! end where the next is long too. Kuttruff's formula takes them as independent
//! ([`super::room`]); how far that matters is measured in `docs/params.md`, "Kuttruff's
//! reference". In a sphere they are independent (each chord is `2R·cos θ` with a fresh `θ`).

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

/// The largest standard error of `γ²` [`free_paths`] accepts; above it, it refuses its own
/// result. At its fixed settings a box's is 0.0006 to 0.0009 (0.0009 for tutorial 1's box as
/// TetGen meshed it), an L-shaped room's 0.0007 and a sphere's 0.0002 (`tests/params_lambert.rs`,
/// `simpa results` on the committed fixture), so only a room whose free paths the fixed rays
/// cannot pin down is refused. It bounds what redescribing the same room can do to `γ²`, the one
/// freedom [`free_paths`] leaves a caller: every description is a fresh draw of the transport's
/// randomness, and the largest of many draws lies a few standard errors from the room's value
/// (32 descriptions of the 6×10×3 m room, turned and moved: −2.4 to +2.2 standard errors,
/// Kuttruff's time at `ᾱ` 0.4 within 0.08 %). At `γ²` ± 0.002 Kuttruff's time moves by 0.06 %
/// at `ᾱ` 0.4 and 0.2 % at 0.8 (`∂T/T` per unit `γ²` is
/// `−ln(1 − ᾱ)/(2·(1 + (γ²/2)·ln(1 − ᾱ)))`, air aside).
pub const GAMMA2_SE_LIMIT: f64 = 0.002;

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

/// A closed room: a surface of triangles, and tetrahedra that fill its volume.
///
/// Built from geometry only ([`Enclosure::shoebox`], [`Enclosure::from_mesh`]). The surface need
/// not be convex, and its faces need not be oriented: a ray is reflected to the side it came from.
/// It must be closed: a ray that leaves it refuses the transport. The tetrahedra give the volume
/// and where [`free_paths`]' rays start; a volume that is not the surface's fails its
/// mean-free-path check.
#[derive(Clone, Debug)]
pub struct Enclosure {
    faces: Vec<Face>,
    nodes: Vec<Node>,
    order: Vec<u32>,
    /// The tetrahedra, and the running sum of their volumes, m³.
    cells: Vec<[V3; 4]>,
    cumulative_m3: Vec<f64>,
    volume_m3: f64,
    area_m2: f64,
    /// The diagonal of the bounding box, m.
    size_m: f64,
}

/// A tetrahedron's volume, m³.
fn tet_volume(t: &[V3; 4]) -> f64 {
    dot(sub(t[1], t[0]), cross(sub(t[2], t[0]), sub(t[3], t[0]))).abs() / 6.0
}

/// The six tetrahedra of the box `[0, x] × [0, y] × [0, z]` around its diagonal (Kuhn's
/// triangulation): one per order of the three axes.
fn box_cells([x, y, z]: V3) -> Vec<[V3; 4]> {
    let e = [[x, 0.0, 0.0], [0.0, y, 0.0], [0.0, 0.0, z]];
    [
        [0, 1, 2],
        [0, 2, 1],
        [1, 0, 2],
        [1, 2, 0],
        [2, 0, 1],
        [2, 1, 0],
    ]
    .iter()
    .map(|&[i, j, k]| {
        let a = [0.0; 3];
        let b = add_scaled(a, e[i], 1.0);
        let c = add_scaled(b, e[j], 1.0);
        let d = add_scaled(c, e[k], 1.0);
        [a, b, c, d]
    })
    .collect()
}

impl Enclosure {
    /// A box `[0, x] × [0, y] × [0, z]`, each side as two triangles in the order −x, +x, −y, +y,
    /// −z, +z, filled by six tetrahedra. Refused, `params_transport_refused`, when a side is not a
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
        Enclosure::from_mesh(&triangles, &box_cells(size_m))
    }

    /// A room bounded by `triangles` and filled by `tetrahedra`. Refused,
    /// `params_transport_refused`, for no triangles or no tetrahedra, a coordinate that is not
    /// finite, no area, or no volume. A surface that is not closed, or tetrahedra that reach
    /// outside it, are found by the transport itself: its rays leave; tetrahedra that do not fill
    /// it, by its mean-free-path check.
    pub fn from_mesh(
        triangles: &[[V3; 3]],
        tetrahedra: &[[V3; 4]],
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
        if tetrahedra.is_empty() {
            return Err(refused("the enclosure has no tetrahedra to fill it"));
        }
        if let Some((i, t)) = tetrahedra
            .iter()
            .enumerate()
            .find(|(_, t)| t.iter().flatten().any(|c| !c.is_finite()))
        {
            return Err(refused(format!(
                "tetrahedron {i} has a coordinate that is not finite: {t:?}"
            )));
        }
        let mut cumulative_m3 = Vec::with_capacity(tetrahedra.len());
        let mut volume_m3 = 0.0;
        for t in tetrahedra {
            volume_m3 += tet_volume(t);
            cumulative_m3.push(volume_m3);
        }
        if !(volume_m3 > 0.0 && volume_m3.is_finite()) {
            return Err(refused(format!(
                "the tetrahedra's volume is {volume_m3} m³, not a positive number"
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
            cells: tetrahedra.to_vec(),
            cumulative_m3,
            volume_m3,
            area_m2,
            size_m,
        })
    }

    /// The tetrahedra's volume, m³.
    pub fn volume_m3(&self) -> f64 {
        self.volume_m3
    }

    /// The faces' total area, m².
    pub fn area_m2(&self) -> f64 {
        self.area_m2
    }

    /// A point drawn evenly from the tetrahedra: one by its share of the volume, then a point in
    /// it by C. Rocchini and P. Cignoni's folding of the unit cube ("Generating random points in a
    /// tetrahedron", J. Graphics Tools 5(4), 2000).
    fn start_point(&self, rng: &mut Rng) -> V3 {
        let u = rng.uniform() * self.volume_m3;
        let i = self
            .cumulative_m3
            .partition_point(|&c| c <= u)
            .min(self.cells.len() - 1);
        let [p0, p1, p2, p3] = self.cells[i];
        let (mut s, mut t, mut v) = (rng.uniform(), rng.uniform(), rng.uniform());
        if s + t > 1.0 {
            (s, t) = (1.0 - s, 1.0 - t);
        }
        if t + v > 1.0 {
            (t, v) = (1.0 - v, 1.0 - s - t);
        } else if s + t + v > 1.0 {
            (s, v) = (1.0 - t - v, s + t + v - 1.0);
        }
        let a = 1.0 - s - t - v;
        [0, 1, 2].map(|k| a * p0[k] + s * p1[k] + t * p2[k] + v * p3[k])
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
/// `0 ≤ t < limit`. Conservative: a box it might meet is kept, which costs time and never a hit.
fn slab(n: &Node, o: V3, inv: V3, limit: f64) -> bool {
    let (mut t0, mut t1) = (0.0f64, limit);
    for k in 0..3 {
        let a = (n.lo[k] - o[k]) * inv[k];
        let b = (n.hi[k] - o[k]) * inv[k];
        // A direction component of 0 gives ±inf, and 0·inf = NaN when the origin lies on the
        // slab's plane. The ray is then parallel to the slab and on its edge: keep the slab open
        // on this axis rather than let one NaN operand of min or max close it.
        if a.is_nan() || b.is_nan() {
            continue;
        }
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
/// `cos θ/π` per steradian (`cos θ·sin θ/π` per `dθ·dψ`). With `uniform` (a test fault only)
/// `cos θ = u`: uniform over the hemisphere, which is not Lambert's law.
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
                 {:?}, last reflected by face {}: the surface is not closed there, or the ray \
                 started outside it",
                self.p,
                self.d,
                if self.face == u32::MAX {
                    "none".to_string()
                } else {
                    self.face.to_string()
                }
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

/// The settings the transport traced with: its own, nothing of any solver. [`free_paths`] always
/// uses [`FreePathSettings::STANDARD`] and takes no settings from its caller; others are for
/// studies of the transport in test builds.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, JsonSchema)]
pub struct FreePathSettings {
    /// Independent replicas, each its own random stream, traced on as many threads as the
    /// machine has. At least 2. The statistical errors are over the rays (`PathStats`).
    pub replicas: u32,
    /// Rays per replica, each started at a point drawn evenly from the enclosure's volume, in an
    /// isotropic direction.
    pub rays_per_replica: u32,
    /// Free paths of each ray left out after its first (from its start to a face, never
    /// counted), while the ray forgets where it started.
    pub burn_in_paths: u32,
    /// Free paths counted per ray after the burn-in. At least 2.
    pub paths_per_ray: u32,
    /// The seed of the replicas' random streams. In JSON, `0x` and 16 hex digits: a number above
    /// 2⁵³ would not survive a reader that parses numbers as doubles.
    #[serde(serialize_with = "super::serialize_seed")]
    #[schemars(with = "String", pattern(super::SEED_PATTERN))]
    pub seed: u64,
}

impl FreePathSettings {
    /// What [`free_paths`], and so `core::results`, uses: 16 × 4096 rays of 1 + 32 + 512 paths,
    /// 33,554,432 paths counted, and a fixed seed (the one the first settings had).
    ///
    /// **Its precision is set by a stated target, not by where a draw landed** (pre-M8 review,
    /// round 2: the first settings, 16 × 1024 rays of 64 paths, put the shipped reference 0.005
    /// points above the 0.6 % in one cell, and changing them until a draw fell below would have
    /// been the fishing `free_paths` exists to close). The target: in every one of M8's cells,
    /// three of the standard deviations Kuttruff's time inherits from `γ²` fit between the
    /// formula's own error there and the 0.6 % the formula is held to. The formula's error, with
    /// the exact `γ²` against the transport at high counts, is at most +0.587 % (5×4×3 m, `ᾱ`
    /// 0.4, on the receivers; +0.585 % on the room energy), so `3·σ(T)/T ≤ 0.013 %`; there
    /// `∂T/T` is 0.2807 per unit `γ²`, so `σ(γ²) ≤ 0.000154`. Measured at another seed (never
    /// this one, so that no candidate's draw was seen), `σ(γ²)·√paths` is 0.72 in that room and
    /// 0.83 in 6×10×3 m, at 64 or 512 paths a ray alike. So at least 21.7 M paths counted; the
    /// smallest power-of-two multiple of the first settings' 1,048,576 that reaches it is 32
    /// (`σ(γ²)` about 0.000124, `3σ(T)/T` 0.0105 %). The seed was kept, and the draw these
    /// settings give is the reference whatever it is.
    ///
    /// 512 paths a ray rather than 64 spends 6 % of the tracing on the 33 paths left out instead
    /// of 34 %: the same precision in three quarters of the time (0.55 s against 0.74 s for a
    /// box in a release build, 28 threads). 32 left out: in an L-shaped room the mean free path
    /// sits 1.8 standard errors from 4V/S with 16 left out and 0.2 with 64
    /// (`tests/params_lambert.rs`).
    pub const STANDARD: FreePathSettings = FreePathSettings {
        replicas: 16,
        rays_per_replica: 4096,
        burn_in_paths: 32,
        paths_per_ray: 512,
        seed: 0x6c61_6d62_6572_7431,
    };
}

/// The free paths between reflections of a diffuse (Lambert) transport in one enclosure: their
/// mean and relative variance `γ²`, with their statistical errors, and the enclosure's `V` and `S`.
///
/// **Made only by [`free_paths`]**, from an [`Enclosure`] alone, at its fixed settings. Its fields
/// are private and it has no other constructor, so no `γ²` fitted to a solver's output, and none
/// fished for through the transport's seed or counts, can reach [`super::room::kuttruff_rt`]:
///
/// ```
/// use simpa_core::params::lambert::{free_paths, Enclosure, FreePathSettings};
/// use simpa_core::params::room::{kuttruff_rt, RtConstant, Surface};
///
/// let room = Enclosure::shoebox([5.0, 4.0, 3.0]).unwrap();
/// let paths = free_paths(&room).unwrap();
/// assert_eq!(paths.settings(), FreePathSettings::STANDARD);
/// let walls = [Surface { area_m2: room.area_m2(), absorption: 0.2 }];
/// let t = kuttruff_rt(&paths, &walls, None, RtConstant::Physical { speed_of_sound: 343.2 }).unwrap();
/// assert!(t > 0.0);
/// ```
///
/// A caller cannot choose the transport's settings: [`free_paths`] takes the room alone, and the
/// same call with settings, which the example above compiles in every other part, does not
/// compile:
///
/// ```compile_fail
/// use simpa_core::params::lambert::{free_paths, Enclosure, FreePathSettings};
///
/// let room = Enclosure::shoebox([5.0, 4.0, 3.0]).unwrap();
/// let paths = free_paths(&room, &FreePathSettings::STANDARD).unwrap();
/// ```
///
/// A `γ²` set by hand does not compile outside this module:
///
/// ```compile_fail
/// use simpa_core::params::lambert::{FreePathSettings, FreePaths};
///
/// let fitted = FreePaths {
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
/// ```
///
/// Stable rustdoc does not check a `compile_fail` example's error code, so the example holds
/// nothing but the literal, and the module's own test `the_hand_made_literal_is_well_formed`
/// compiles the same literal inside the module: the example fails for the fields' privacy alone.
/// Made public, the fields would let it compile, and the doctest would fail.
#[derive(Clone, Debug, PartialEq, Serialize, JsonSchema)]
#[schemars(
    description = "The free paths between reflections of a diffuse (Lambert) ray transport in the \
                   room, computed from its geometry alone at fixed settings \
                   (params::lambert::free_paths): their mean and relative variance gamma^2, with \
                   their standard errors over the transport's replicas, and the room's 4V/S, V \
                   and S."
)]
pub struct FreePaths {
    /// The mean free path, m.
    mean_free_path_m: f64,
    /// Its standard error over the replicas, m.
    mean_free_path_se_m: f64,
    /// `γ²`, the free paths' relative variance `(⟨ℓ²⟩ − ⟨ℓ⟩²)/⟨ℓ⟩²`.
    gamma2: f64,
    /// Its standard error over the replicas.
    gamma2_se: f64,
    /// Kosten's `4V/S`, m, which the mean free path was checked against.
    four_v_over_s_m: f64,
    /// The room's volume (its tetrahedra's), m³.
    volume_m3: f64,
    /// The room's surface area, m².
    area_m2: f64,
    /// The free paths counted.
    paths: u64,
    /// The transport's settings.
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

/// One replica's sums over its counted free paths.
#[derive(Clone, Copy, Debug, Default)]
struct PathSums {
    /// Paths counted, `Σ ℓ` and `Σ ℓ²`.
    n: u64,
    s1: f64,
    s2: f64,
    /// Pairs of successive counted paths of one ray, and `Σ ℓₖ·ℓₖ₊₁` over them.
    pairs: u64,
    s11: f64,
    /// Rays, and the sums over them of each ray's counted length `Lᵣ = Σ ℓ` and of `Lᵣ²`, of `Qᵣ²`
    /// for its counted `Qᵣ = Σ ℓ²` (whose sum is `s2`), and of `Lᵣ·Qᵣ`.
    rays: u64,
    r1: f64,
    r2: f64,
    q2: f64,
    rq: f64,
}

/// What the replicas' sums give. The mean free path and `γ²` are pooled over every counted path,
/// and their standard errors are over the rays (below); the correlation and the effective `γ²`,
/// read by studies only, are pooled too, with the standard error of the replicas' own values.
///
/// **The standard errors are over the rays**, each ray's counted paths summed first: rays are
/// independent, the paths of one ray are not (successive paths are correlated), so a ray is the
/// unit. With `R` rays of `K` counted paths, `xᵣ = Lᵣ/K` and `yᵣ = Qᵣ/K`, the mean free path's is
/// `√(var x/(R − 1))`, and `γ² = ȳ/x̄² − 1`'s the delta method's,
/// `√((var y/x̄⁴ − 4ȳ·cov(x, y)/x̄⁵ + 4ȳ²·var x/x̄⁶)/(R − 1))`. Over tens of thousands of rays each
/// is itself known to a fraction of a per cent, where 16 replicas' spread knew it only to about a
/// fifth, so a room's refusal for its mean free path no longer turns on how that spread fell (the
/// pre-M8 review: a volume 0.4 % off was refused and one 0.5 % off accepted).
#[derive(Clone, Copy, Debug)]
#[cfg_attr(not(feature = "transport-study"), allow(dead_code))]
struct PathStats {
    paths: u64,
    mean: f64,
    mean_se: f64,
    gamma2: f64,
    gamma2_se: f64,
    lag1: f64,
    lag1_se: f64,
    effective_gamma2: f64,
    effective_gamma2_se: f64,
}

impl PathSums {
    /// `(mean, γ², lag-1 correlation, effective γ²)` of these sums.
    fn values(&self) -> [f64; 4] {
        let m = self.s1 / self.n as f64;
        let second = self.s2 / self.n as f64;
        let gamma2 = second / (m * m) - 1.0;
        let var = second - m * m;
        let lag1 = (self.s11 / self.pairs as f64 - m * m) / var;
        let per_ray = self.n as f64 / self.rays as f64;
        let ray_mean = self.r1 / self.rays as f64;
        let ray_var = self.r2 / self.rays as f64 - ray_mean * ray_mean;
        [m, gamma2, lag1, ray_var / (per_ray * m * m)]
    }

    fn add(&mut self, o: &PathSums) {
        self.n += o.n;
        self.s1 += o.s1;
        self.s2 += o.s2;
        self.pairs += o.pairs;
        self.s11 += o.s11;
        self.rays += o.rays;
        self.r1 += o.r1;
        self.r2 += o.r2;
        self.q2 += o.q2;
        self.rq += o.rq;
    }

    /// The standard errors of the mean free path and of `γ²` over the rays ([`PathStats`]). NaN
    /// for fewer than 2 rays.
    fn ray_errors(&self) -> (f64, f64) {
        let r = self.rays as f64;
        let k = self.n as f64 / r;
        let (x, y) = (self.s1 / self.n as f64, self.s2 / self.n as f64);
        let rk2 = r * k * k;
        // Population moments of xᵣ = Lᵣ/K and yᵣ = Qᵣ/K; the (R − 1) below makes them the
        // samples'.
        let var_x = self.r2 / rk2 - x * x;
        let var_y = self.q2 / rk2 - y * y;
        let cov = self.rq / rk2 - x * y;
        let mean_se = (var_x / (r - 1.0)).sqrt();
        let var_g = var_y / x.powi(4) - 4.0 * y * cov / x.powi(5) + 4.0 * y * y * var_x / x.powi(6);
        (mean_se, (var_g / (r - 1.0)).sqrt())
    }
}

fn path_stats(sums: &[PathSums]) -> PathStats {
    let mut all = PathSums::default();
    let mut per: [Vec<f64>; 4] = Default::default();
    for s in sums {
        all.add(s);
        for (k, v) in s.values().into_iter().enumerate() {
            per[k].push(v);
        }
    }
    let [mean, gamma2, lag1, effective_gamma2] = all.values();
    let (mean_error, gamma2_error) = all.ray_errors();
    let se = |k: usize| mean_se(&per[k]).1;
    PathStats {
        paths: all.n,
        mean,
        mean_se: mean_error,
        gamma2,
        gamma2_se: gamma2_error,
        lag1,
        lag1_se: se(2),
        effective_gamma2,
        effective_gamma2_se: se(3),
    }
}

/// Refuses settings the transport cannot give an error with: fewer than 2 replicas, no ray, or
/// fewer than 2 paths a ray.
fn check_settings(s: &FreePathSettings) -> Result<(), ParamError> {
    if s.replicas < 2 || s.rays_per_replica == 0 || s.paths_per_ray < 2 {
        return Err(refused(format!(
            "the free-path settings need at least 2 replicas, a ray and 2 paths a ray: {s:?}"
        )));
    }
    Ok(())
}

/// Traces `settings`' rays in `enclosure`, each from `start_m` or, when `None`, from a point drawn
/// evenly from its volume, and sums each replica's counted paths. Refused when a ray leaves.
fn trace(
    enclosure: &Enclosure,
    settings: &FreePathSettings,
    start_m: Option<V3>,
) -> Result<Vec<PathSums>, ParamError> {
    let s = *settings;
    let uniform = uniform_fault();
    per_replica(s.replicas, |r| {
        let mut rng = Rng::new(replica_seed(s.seed, r));
        let mut sums = PathSums::default();
        let mut reflections = 0u64;
        for _ in 0..s.rays_per_replica {
            let start = match start_m {
                Some(p) => p,
                None => enclosure.start_point(&mut rng),
            };
            let mut ray = Ray::from_source(start, &mut rng);
            let (mut length, mut square, mut previous) = (0.0f64, 0.0f64, None::<f64>);
            // Path 0 runs from the start point; paths 1 to burn_in are left out too.
            for k in 0..=u64::from(s.burn_in_paths) + u64::from(s.paths_per_ray) {
                let (run, face) = ray.run(enclosure, reflections)?;
                if k > u64::from(s.burn_in_paths) {
                    sums.n += 1;
                    sums.s1 += run;
                    sums.s2 += run * run;
                    if let Some(p) = previous {
                        sums.pairs += 1;
                        sums.s11 += p * run;
                    }
                    previous = Some(run);
                    length += run;
                    square += run * run;
                }
                ray.reflect(enclosure, run, face, &mut rng, uniform);
                reflections += 1;
            }
            sums.rays += 1;
            sums.r1 += length;
            sums.r2 += length * length;
            sums.q2 += square * square;
            sums.rq += length * square;
        }
        Ok(sums)
    })
}

/// The free paths of a diffuse (Lambert) transport in `enclosure` ([`FreePaths`]), traced at
/// [`FreePathSettings::STANDARD`]: **the room is its only input**. Refused,
/// `params_transport_refused`, for a ray that leaves the enclosure, a mean free path further from
/// `4V/S` than [`MEAN_FREE_PATH_TOLERANCE_SE`] standard errors, or a `γ²` whose standard error is
/// above [`GAMMA2_SE_LIMIT`].
pub fn free_paths(enclosure: &Enclosure) -> Result<FreePaths, ParamError> {
    free_paths_with(enclosure, &FreePathSettings::STANDARD)
}

/// [`free_paths`] at `settings`. Private: this module's tests use it for the say-NO of
/// [`GAMMA2_SE_LIMIT`]; nothing outside can choose the settings.
fn free_paths_with(
    enclosure: &Enclosure,
    settings: &FreePathSettings,
) -> Result<FreePaths, ParamError> {
    check_settings(settings)?;
    let st = path_stats(&trace(enclosure, settings, None)?);
    let (mean, mean_error) = (st.mean, st.mean_se);
    let four_v_over_s_m = enclosure.four_v_over_s_m();
    let off = (mean - four_v_over_s_m).abs();
    if off > MEAN_FREE_PATH_TOLERANCE_SE * mean_error + 1e-9 * four_v_over_s_m {
        return Err(refused(format!(
            "the mean free path is {mean} ± {mean_error} m, and 4V/S is {four_v_over_s_m} m: {:.1} \
             standard errors apart, more than {MEAN_FREE_PATH_TOLERANCE_SE}. Diffuse reflection \
             in a closed room of this volume gives 4V/S; the surface is not closed, the volume is \
             not the surface's, a face has the room on both its sides, the room's parts do not \
             mix within the rays' paths, or the reflection is not Lambert's",
            off / mean_error.max(f64::MIN_POSITIVE)
        )));
    }
    if st.gamma2_se.is_nan() || st.gamma2_se > GAMMA2_SE_LIMIT {
        return Err(refused(format!(
            "gamma^2 is {} ± {}, a standard error above the limit {GAMMA2_SE_LIMIT}: the \
             transport's fixed rays do not pin this room's free paths down",
            st.gamma2, st.gamma2_se
        )));
    }
    Ok(FreePaths {
        mean_free_path_m: mean,
        mean_free_path_se_m: mean_error,
        gamma2: st.gamma2,
        gamma2_se: st.gamma2_se,
        four_v_over_s_m,
        volume_m3: enclosure.volume_m3,
        area_m2: enclosure.area_m2,
        paths: st.paths,
        settings: *settings,
    })
}

/// A study of the transport's free paths at any settings and starting point: evidence and tests
/// only (the `transport-study` feature, which only test builds turn on). **Not a [`FreePaths`]**:
/// no function of `params::room` takes it, so nothing studied here can become a reference.
#[cfg(feature = "transport-study")]
#[derive(Clone, Debug, PartialEq)]
pub struct FreePathStudy {
    /// The mean free path and its standard error over the replicas, m.
    pub mean_free_path_m: f64,
    pub mean_free_path_se_m: f64,
    /// `γ²` and its standard error.
    pub gamma2: f64,
    pub gamma2_se: f64,
    /// The correlation of successive counted paths of one ray, `(⟨ℓₖ·ℓₖ₊₁⟩ − ⟨ℓ⟩²)/var(ℓ)`, and its
    /// standard error.
    pub lag1_correlation: f64,
    pub lag1_correlation_se: f64,
    /// The variance of a ray's counted length over `K = paths_per_ray` paths, divided by
    /// `K·⟨ℓ⟩²`, and its standard error: `γ²` for independent paths, larger when successive
    /// paths are positively correlated.
    pub effective_gamma2: f64,
    pub effective_gamma2_se: f64,
    /// Kosten's `4V/S`, m.
    pub four_v_over_s_m: f64,
    /// The paths counted.
    pub paths: u64,
    pub settings: FreePathSettings,
    /// Where every ray started, or `None`: evenly in the volume.
    pub start_m: Option<V3>,
}

#[cfg(feature = "transport-study")]
impl FreePathStudy {
    /// How far the mean free path lies from `4V/S`, in its standard errors (signed).
    pub fn mean_free_path_off_se(&self) -> f64 {
        (self.mean_free_path_m - self.four_v_over_s_m) / self.mean_free_path_se_m
    }
}

/// The transport's free paths in `enclosure` at `settings`, every ray started at `start_m`, or
/// evenly in the volume when `None`. Test builds only (`transport-study`). It does not check the
/// mean free path against `4V/S` ([`FreePathStudy::mean_free_path_off_se`] says how far it is).
/// Refused, `params_transport_refused`, for fewer than 2 replicas, no ray or fewer than 2 paths a
/// ray, a start point that is not finite, or a ray that leaves the enclosure.
#[cfg(feature = "transport-study")]
pub fn free_path_study(
    enclosure: &Enclosure,
    settings: &FreePathSettings,
    start_m: Option<V3>,
) -> Result<FreePathStudy, ParamError> {
    check_settings(settings)?;
    if let Some(p) = start_m
        && p.iter().any(|c| !c.is_finite())
    {
        return Err(refused(format!("the start point {p:?} is not finite")));
    }
    let st = path_stats(&trace(enclosure, settings, start_m)?);
    Ok(FreePathStudy {
        mean_free_path_m: st.mean,
        mean_free_path_se_m: st.mean_se,
        gamma2: st.gamma2,
        gamma2_se: st.gamma2_se,
        lag1_correlation: st.lag1,
        lag1_correlation_se: st.lag1_se,
        effective_gamma2: st.effective_gamma2,
        effective_gamma2_se: st.effective_gamma2_se,
        four_v_over_s_m: enclosure.four_v_over_s_m(),
        paths: st.paths,
        settings: *settings,
        start_m,
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

#[cfg(test)]
mod tests {
    use super::*;

    /// The literal of [`FreePaths`]' `compile_fail` example, character for character: it compiles
    /// here, inside the module, so the example fails outside it only because the fields are
    /// private.
    #[test]
    fn the_hand_made_literal_is_well_formed() {
        let fitted = FreePaths {
            mean_free_path_m: 2.553,
            mean_free_path_se_m: 0.0,
            gamma2: 0.4,
            gamma2_se: 0.0,
            four_v_over_s_m: 2.553,
            volume_m3: 60.0,
            area_m2: 94.0,
            paths: 1,
            settings: FreePathSettings::STANDARD,
        };
        assert_eq!(fitted.gamma2(), 0.4);
    }

    /// The say-NO of [`GAMMA2_SE_LIMIT`], through the private [`free_paths_with`]: the settings a
    /// search over seeds used to fish for `γ²` in the pre-M8 review (2 replicas of 1 ray and 3
    /// paths, which gave 0.599 at seed 1265 and 0.101 at seed 84; 16 × 16 rays of 32 + 8 paths,
    /// 0.450 at seed 903), each accepted by the mean-free-path check before this limit, are
    /// refused for their standard error. The fixed settings are accepted, well inside it.
    #[test]
    fn a_gamma2_too_loose_to_describe_the_room_is_refused() {
        let room = Enclosure::shoebox([6.0, 10.0, 3.0]).unwrap();
        let tiny = FreePathSettings {
            replicas: 2,
            rays_per_replica: 1,
            burn_in_paths: 0,
            paths_per_ray: 3,
            seed: 0,
        };
        let small = FreePathSettings {
            replicas: 16,
            rays_per_replica: 16,
            burn_in_paths: 32,
            paths_per_ray: 8,
            seed: 0,
        };
        for (settings, seed) in [(tiny, 1265), (tiny, 84), (small, 903)] {
            let e = free_paths_with(&room, &FreePathSettings { seed, ..settings }).unwrap_err();
            assert_eq!(e.code(), crate::params::codes::TRANSPORT_REFUSED, "{e}");
            assert!(e.to_string().contains("above the limit"), "{seed}: {e}");
        }
        let p = free_paths(&room).unwrap();
        assert_eq!(p.settings(), FreePathSettings::STANDARD);
        assert!(p.gamma2_se() < GAMMA2_SE_LIMIT / 2.0, "{p:?}");
    }

    /// Start points fill the room evenly: in the box's six tetrahedra, the share of points in the
    /// lowest third of each axis is a third, and the mean is the centre. The second input is the
    /// partner for the choice of tetrahedron: two cells of volumes 1 to 27, where a choice by
    /// count instead of volume would put half the points in the small one, not 1/28.
    #[test]
    fn start_points_fill_the_room_evenly() {
        let size = [6.0, 10.0, 3.0];
        let e = Enclosure::shoebox(size).unwrap();
        assert_eq!(e.cells.len(), 6);
        assert!((e.volume_m3 - 180.0).abs() < 1e-9);
        let mut rng = Rng::new(11);
        let n = 200_000;
        let (mut mean, mut low) = ([0.0; 3], [0usize; 3]);
        for _ in 0..n {
            let p = e.start_point(&mut rng);
            for k in 0..3 {
                assert!((0.0..=size[k]).contains(&p[k]), "{p:?}");
                mean[k] += p[k] / n as f64;
                if p[k] < size[k] / 3.0 {
                    low[k] += 1;
                }
            }
        }
        // Binomial standard error of a third over 200,000: 0.0011; of the mean, size/√(12·n).
        for k in 0..3 {
            let share = low[k] as f64 / n as f64;
            assert!((share - 1.0 / 3.0).abs() < 0.005, "axis {k}: {share}");
            let se = size[k] / (12.0 * n as f64).sqrt();
            assert!(
                (mean[k] - size[k] / 2.0).abs() < 4.0 * se,
                "axis {k}: {}",
                mean[k]
            );
        }
        // Says no: one tetrahedron of unequal cells chosen evenly by count, not by volume, puts
        // too many points in the small one.
        let cells = [
            [[0.0; 3], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
            [[0.0; 3], [3.0, 0.0, 0.0], [0.0, 3.0, 0.0], [0.0, 0.0, -3.0]],
        ];
        let (v0, v1) = (tet_volume(&cells[0]), tet_volume(&cells[1]));
        assert!((v1 / v0 - 27.0).abs() < 1e-9);
        let tris = [[[0.0; 3], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]]];
        let e = Enclosure::from_mesh(&tris, &cells).unwrap();
        let above = (0..n).filter(|_| e.start_point(&mut rng)[2] > 0.0).count() as f64 / n as f64;
        assert!((above - 1.0 / 28.0).abs() < 0.003, "{above}");
    }
}
