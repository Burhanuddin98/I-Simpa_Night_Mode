//! Exact geometric predicates on `f64` points.
//!
//! Every decision here is the **sign** of an orientation determinant, computed by Shewchuk's
//! adaptive-precision `orient2d` / `orient3d` (the `robust` crate). The signs are exact for any
//! finite input that does not underflow, so "touching", "coplanar" and "collinear" are decided
//! without a tolerance. Magnitudes are only ever used to pick a projection axis or to order hits
//! along a ray, never to decide whether two things meet.
//!
//! Inputs must be finite: the check excludes faces with a non-finite vertex before any predicate
//! sees them.

use robust::{Coord, Coord3D};

pub(crate) type P3 = [f64; 3];
type P2 = [f64; 2];

fn c3(p: P3) -> Coord3D<f64> {
    Coord3D {
        x: p[0],
        y: p[1],
        z: p[2],
    }
}

fn c2(p: P2) -> Coord<f64> {
    Coord { x: p[0], y: p[1] }
}

fn sign(v: f64) -> i8 {
    if v > 0.0 {
        1
    } else if v < 0.0 {
        -1
    } else {
        0
    }
}

/// Shewchuk's `orient3d`: positive when `d` lies on the **back** side of the plane through `a, b,
/// c` (the side the normal `(b - a) x (c - a)` points away from), negative on the front side,
/// zero when the four points are coplanar. The sign is exact; the magnitude approximates six times
/// the signed volume of the tetrahedron.
pub(crate) fn orient3d_value(a: P3, b: P3, c: P3, d: P3) -> f64 {
    robust::orient3d(c3(a), c3(b), c3(c), c3(d))
}

/// The exact sign of [`orient3d_value`].
pub(crate) fn orient3d(a: P3, b: P3, c: P3, d: P3) -> i8 {
    sign(orient3d_value(a, b, c, d))
}

/// Exact sign of the 2D orientation: +1 when `a, b, c` turn counter-clockwise.
fn orient2d(a: P2, b: P2, c: P2) -> i8 {
    sign(robust::orient2d(c2(a), c2(b), c2(c)))
}

/// `p` with coordinate `axis` dropped. Axis 0 keeps (y, z), 1 keeps (z, x), 2 keeps (x, y): the
/// signed area of a projected triangle is then exactly the `axis` component of its normal.
fn project(p: P3, axis: usize) -> P2 {
    match axis {
        0 => [p[1], p[2]],
        1 => [p[2], p[0]],
        _ => [p[0], p[1]],
    }
}

/// The coordinate axis to drop so that the triangle keeps the largest projected area, or `None`
/// when all three projections are exactly collinear, which is exactly when the triangle has zero
/// area (the three projected areas are the three components of its normal).
///
/// Dropping an axis along which the plane's normal has a non-zero component is an affine
/// bijection of the plane onto the coordinate plane, so for points **exactly** coplanar with the
/// triangle every 2D orientation keeps its truth up to one global sign; the 2D tests below only
/// compare signs with each other.
pub(crate) fn projection_axis(t: &[P3; 3]) -> Option<usize> {
    let mut best = None;
    let mut best_magnitude = 0.0;
    for axis in 0..3 {
        let [a, b, c] = t.map(|p| project(p, axis));
        let magnitude = robust::orient2d(c2(a), c2(b), c2(c)).abs();
        if magnitude > best_magnitude {
            best_magnitude = magnitude;
            best = Some(axis);
        }
    }
    best
}

/// Whether the triangle has exactly zero area (collinear or coincident vertices).
pub(crate) fn is_zero_area(t: &[P3; 3]) -> bool {
    projection_axis(t).is_none()
}

/// Whether collinear point `r` lies on the closed segment `p q`.
fn within_box_2d(p: P2, q: P2, r: P2) -> bool {
    (0..2).all(|k| p[k].min(q[k]) <= r[k] && r[k] <= p[k].max(q[k]))
}

/// Whether the closed segments `p1 p2` and `q1 q2` meet (Cormen et al., 33.1).
fn segments_meet_2d(p1: P2, p2: P2, q1: P2, q2: P2) -> bool {
    let d1 = orient2d(q1, q2, p1);
    let d2 = orient2d(q1, q2, p2);
    let d3 = orient2d(p1, p2, q1);
    let d4 = orient2d(p1, p2, q2);
    if d1 * d2 < 0 && d3 * d4 < 0 {
        return true;
    }
    (d1 == 0 && within_box_2d(q1, q2, p1))
        || (d2 == 0 && within_box_2d(q1, q2, p2))
        || (d3 == 0 && within_box_2d(p1, p2, q1))
        || (d4 == 0 && within_box_2d(p1, p2, q2))
}

/// Whether `p` lies in the closed, non-degenerate triangle `t`.
fn point_in_triangle_2d(p: P2, t: [P2; 3]) -> bool {
    let o = orient2d(t[0], t[1], t[2]);
    [
        orient2d(t[0], t[1], p),
        orient2d(t[1], t[2], p),
        orient2d(t[2], t[0], p),
    ]
    .iter()
    .all(|&s| s == 0 || s == o)
}

/// Whether the closed segment `s0 s1` meets the closed, non-degenerate triangle `t`, all in 2D.
fn segment_meets_triangle_2d(s0: P2, s1: P2, t: [P2; 3]) -> bool {
    point_in_triangle_2d(s0, t)
        || point_in_triangle_2d(s1, t)
        || (0..3).any(|k| segments_meet_2d(s0, s1, t[k], t[(k + 1) % 3]))
}

/// Whether the closed segment `s0 s1` meets the closed, non-degenerate triangle `t`.
pub(crate) fn segment_meets_triangle(s0: P3, s1: P3, t: &[P3; 3]) -> bool {
    let [a, b, c] = *t;
    let o0 = orient3d(a, b, c, s0);
    let o1 = orient3d(a, b, c, s1);
    if o0 == o1 && o0 != 0 {
        return false;
    }
    if o0 == 0 && o1 == 0 {
        let Some(axis) = projection_axis(t) else {
            return false;
        };
        return segment_meets_triangle_2d(
            project(s0, axis),
            project(s1, axis),
            t.map(|p| project(p, axis)),
        );
    }
    // The segment reaches the plane at exactly one point; the line through it passes through the
    // closed triangle iff the three edge orientations never disagree in sign.
    let e = [
        orient3d(s0, s1, a, b),
        orient3d(s0, s1, b, c),
        orient3d(s0, s1, c, a),
    ];
    !(e.contains(&1) && e.contains(&-1))
}

fn edges(t: &[P3; 3]) -> [(P3, P3); 3] {
    [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])]
}

fn strictly_one_side(t: &[P3; 3], of: &[P3; 3]) -> bool {
    let s = of.map(|p| orient3d(t[0], t[1], t[2], p));
    s.iter().all(|&x| x == 1) || s.iter().all(|&x| x == -1)
}

/// Whether two closed, non-degenerate triangles that share **no** vertex position meet at all.
///
/// Their intersection, when not empty, is convex and compact; each of its extreme points lies on
/// the boundary of one of the triangles, so some edge of one triangle meets the other.
pub(crate) fn triangles_meet(t1: &[P3; 3], t2: &[P3; 3]) -> bool {
    if strictly_one_side(t1, t2) || strictly_one_side(t2, t1) {
        return false;
    }
    edges(t1)
        .iter()
        .any(|&(p, q)| segment_meets_triangle(p, q, t2))
        || edges(t2)
            .iter()
            .any(|&(p, q)| segment_meets_triangle(p, q, t1))
}

/// Two non-degenerate triangles `p q a` and `p q b` that share the edge `p q` (and no third
/// position) meet beyond that edge only when they are coplanar with `a` and `b` on the same side
/// of it, that is, when they overlap.
pub(crate) fn edge_neighbours_overlap(p: P3, q: P3, a: P3, b: P3) -> bool {
    if orient3d(p, q, a, b) != 0 {
        return false;
    }
    let Some(axis) = projection_axis(&[p, q, a]) else {
        return false;
    };
    let (p2, q2) = (project(p, axis), project(q, axis));
    orient2d(p2, q2, project(a, axis)) == orient2d(p2, q2, project(b, axis))
}

/// Whether the segment from `t[0]` towards `x` enters the closed triangle `t` beyond `t[0]`.
/// Only possible when `x` is coplanar with `t` and the direction lies in the triangle's corner.
fn enters_corner(x: P3, t: &[P3; 3]) -> bool {
    let [p, c, d] = *t;
    if orient3d(p, c, d, x) != 0 {
        return false;
    }
    let Some(axis) = projection_axis(t) else {
        return false;
    };
    let [p2, c2, d2, x2] = [p, c, d, x].map(|v| project(v, axis));
    let r = orient2d(p2, c2, d2);
    let s1 = orient2d(p2, c2, x2);
    let s2 = orient2d(p2, d2, x2);
    (s1 == 0 || s1 == r) && (s2 == 0 || s2 == -r)
}

/// Two non-degenerate triangles `p a b` and `p c d` that share the vertex position `p` (and no
/// other) meet anywhere besides `p`.
///
/// If they do, their intersection is a convex set holding `p` and some other point; its far
/// boundary lies on an edge of one triangle. Either that is the edge opposite `p` (which then
/// meets the other triangle), or an edge through `p` runs into the other triangle.
pub(crate) fn vertex_neighbours_meet(p: P3, a: P3, b: P3, c: P3, d: P3) -> bool {
    let t1 = [p, a, b];
    let t2 = [p, c, d];
    segment_meets_triangle(a, b, &t2)
        || segment_meets_triangle(c, d, &t1)
        || enters_corner(a, &t2)
        || enters_corner(b, &t2)
        || enters_corner(c, &t1)
        || enters_corner(d, &t1)
}

/// How a segment `p q` meets a triangle, for ray casting.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Crossing {
    Miss,
    /// Crosses the triangle's interior at parameter `t` in (0, 1). `from_front`: `p` is on the
    /// triangle's front (normal) side; `to_front`: `q` is.
    Hit {
        t: f64,
        from_front: bool,
        to_front: bool,
    },
    /// Passes exactly through an edge or a vertex, starts on the triangle, or runs in its plane
    /// across it. The caller picks another ray.
    Degenerate,
}

/// Classifies the segment `p q` against the non-degenerate triangle `t`. `q` must lie outside the
/// model's bounding box, so it is never on a triangle.
pub(crate) fn segment_crossing(p: P3, q: P3, t: &[P3; 3]) -> Crossing {
    let [a, b, c] = *t;
    let vp = orient3d_value(a, b, c, p);
    let vq = orient3d_value(a, b, c, q);
    let (sp, sq) = (sign(vp), sign(vq));
    if sp == sq && sp != 0 {
        return Crossing::Miss;
    }
    if sp == 0 && sq == 0 {
        return if segment_meets_triangle(p, q, t) {
            Crossing::Degenerate
        } else {
            Crossing::Miss
        };
    }
    if sq == 0 {
        // Touches the plane only at `q`, which is outside every triangle.
        return Crossing::Miss;
    }
    let e = [
        orient3d(p, q, a, b),
        orient3d(p, q, b, c),
        orient3d(p, q, c, a),
    ];
    let mixed = e.contains(&1) && e.contains(&-1);
    if mixed {
        return Crossing::Miss;
    }
    if sp == 0 || e.contains(&0) {
        // Starts on the triangle, or passes through its boundary.
        return Crossing::Degenerate;
    }
    Crossing::Hit {
        t: vp / (vp - vq),
        from_front: sp < 0,
        to_front: sq < 0,
    }
}

/// Where `w` lies in angle around the directed axis `u -> v`, relative to the reference `r`,
/// as a half-turn class: 0 on `r`'s half-plane, 1 in the half-turn after it (right-hand rotation
/// about `u -> v`), 2 on the opposite half-plane, 3 in the last half-turn.
///
/// `u, v, r` must be a non-degenerate triangle and `w` must not lie on the axis line.
pub(crate) fn half_turn(u: P3, v: P3, r: P3, w: P3) -> u8 {
    match orient3d(u, v, r, w) {
        -1 => 1,
        1 => 3,
        _ => {
            // Coplanar with the axis and r: same side of the axis as r, or the other side.
            let axis = projection_axis(&[u, v, r]).unwrap_or(2);
            let [u2, v2, r2, w2] = [u, v, r, w].map(|p| project(p, axis));
            if orient2d(u2, v2, w2) == orient2d(u2, v2, r2) {
                0
            } else {
                2
            }
        }
    }
}

/// Angular order of `a` and `b` about the directed axis `u -> v`, for two points in the same open
/// half-turn: `Less` when `b` lies after `a` in right-hand rotation, `Equal` when both lie on one
/// half-plane.
pub(crate) fn angular_order(u: P3, v: P3, a: P3, b: P3) -> std::cmp::Ordering {
    match orient3d(u, v, a, b) {
        -1 => std::cmp::Ordering::Less,
        1 => std::cmp::Ordering::Greater,
        _ => std::cmp::Ordering::Equal,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    /// Separating-axis reference for closed triangles, exact on small dyadic coordinates (every
    /// product and sum below is then exact in f64).
    fn sat_meet(t1: &[P3; 3], t2: &[P3; 3]) -> bool {
        fn sub(a: P3, b: P3) -> P3 {
            [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
        }
        fn cross(a: P3, b: P3) -> P3 {
            [
                a[1] * b[2] - a[2] * b[1],
                a[2] * b[0] - a[0] * b[2],
                a[0] * b[1] - a[1] * b[0],
            ]
        }
        fn dot(a: P3, b: P3) -> f64 {
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
        // In-plane edge normals, taken with both normals: when one argument is a segment or a
        // point (normal zero), its edges' in-plane normals come from the other triangle's normal.
        for n in [n1, n2] {
            for e in e1.iter().chain(e2.iter()) {
                axes.push(cross(n, *e));
            }
        }
        for axis in axes {
            if axis == [0.0; 3] {
                continue;
            }
            let p1 = t1.map(|p| dot(p, axis));
            let p2 = t2.map(|p| dot(p, axis));
            let (lo1, hi1) = (
                p1.iter().copied().fold(f64::INFINITY, f64::min),
                p1.iter().copied().fold(f64::NEG_INFINITY, f64::max),
            );
            let (lo2, hi2) = (
                p2.iter().copied().fold(f64::INFINITY, f64::min),
                p2.iter().copied().fold(f64::NEG_INFINITY, f64::max),
            );
            if hi1 < lo2 || hi2 < lo1 {
                return false;
            }
        }
        true
    }

    fn dyadic() -> impl Strategy<Value = f64> {
        (-4i32..=4).prop_map(|k| f64::from(k) * 0.5)
    }

    fn point() -> impl Strategy<Value = P3> {
        [dyadic(), dyadic(), dyadic()]
    }

    fn triangle() -> impl Strategy<Value = [P3; 3]> {
        [point(), point(), point()].prop_filter("non-degenerate", |t| !is_zero_area(t))
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(4000))]

        /// On a small integer lattice (many coplanar, collinear and touching configurations) the
        /// exact test agrees with the separating-axis reference whenever no position is shared.
        #[test]
        fn triangles_meet_matches_sat(t1 in triangle(), t2 in triangle()) {
            prop_assume!(t1.iter().all(|p| !t2.contains(p)));
            prop_assert_eq!(triangles_meet(&t1, &t2), sat_meet(&t1, &t2));
        }

        /// Sharing one position `p`: two convex sets that both hold `p` meet somewhere else iff
        /// their corner cones at `p` share a ray, iff the segment `a b` (translated so `p` is the
        /// origin) meets the cone spanned by `c - p` and `d - p`. That cone is replaced by the
        /// triangle `0, K(c - p), K(d - p)`: on this lattice a point of `a b` in the cone has
        /// cone coordinates below |x| |d| / |c x d| <= 8 * 7 / 0.25 = 224 < K, and every
        /// coordinate stays dyadic, so the separating-axis reference is exact.
        #[test]
        fn shared_vertex_matches_cone_reference(p in point(), a in point(), b in point(), c in point(), d in point()) {
            let t1 = [p, a, b];
            let t2 = [p, c, d];
            prop_assume!(!is_zero_area(&t1) && !is_zero_area(&t2));
            prop_assume!(![a, b].contains(&c) && ![a, b].contains(&d));
            const K: f64 = 1024.0;
            let rel = |x: P3, s: f64| [s * (x[0] - p[0]), s * (x[1] - p[1]), s * (x[2] - p[2])];
            let (ra, rb) = (rel(a, 1.0), rel(b, 1.0));
            let cone = [[0.0; 3], rel(c, K), rel(d, K)];
            let reference = sat_meet(&[ra, rb, rb], &cone);
            prop_assert_eq!(vertex_neighbours_meet(p, a, b, c, d), reference);
        }

        /// Sharing an edge: they overlap beyond it iff the second triangle's points just off the
        /// middle of the edge lie in the first. Reference: is `y = m + (b - m) / 1024` (`m` the
        /// edge's midpoint) in the closed triangle `p q a`? On this lattice `y` is within the
        /// first triangle's extent whenever it is on its half-plane (its barycentric weight on
        /// `a` is at most 7 / (1024 * 0.036) < 1 and on `p`, `q` at least 1/2 - 14/1024), and
        /// every coordinate stays dyadic, so the separating-axis test is exact.
        #[test]
        fn shared_edge_matches_point_reference(p in point(), q in point(), a in point(), b in point()) {
            prop_assume!(!is_zero_area(&[p, q, a]) && !is_zero_area(&[p, q, b]) && a != b);
            let m = [(p[0] + q[0]) / 2.0, (p[1] + q[1]) / 2.0, (p[2] + q[2]) / 2.0];
            let y = [m[0] + (b[0] - m[0]) / 1024.0, m[1] + (b[1] - m[1]) / 1024.0, m[2] + (b[2] - m[2]) / 1024.0];
            prop_assert_eq!(edge_neighbours_overlap(p, q, a, b), sat_meet(&[p, q, a], &[y, y, y]));
        }
    }

    /// A deterministic sweep over the same lattice, counting what it covers: the agreement is
    /// only evidence if many pairs meet, many are coplanar and many merely touch.
    #[test]
    fn sweep_agrees_with_sat_and_covers_the_hard_cases() {
        let mut state: u64 = 0x9e37_79b9_7f4a_7c15;
        let mut next = || {
            state = state
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            f64::from(((state >> 33) % 9) as u8) * 0.5 - 2.0
        };
        let (mut pairs, mut meet, mut coplanar, mut touching) = (0, 0, 0, 0);
        let mut draws = 0u32;
        while pairs < 30_000 {
            draws += 1;
            // One draw in three puts all six points on z = 1/2, one in three on the slanted
            // plane z = x - y (exact on the lattice), the rest anywhere.
            let mode = draws % 3;
            let mut p = || {
                let (x, y, z) = (next(), next(), next());
                match mode {
                    0 => [x, y, z],
                    1 => [x, y, 0.5],
                    _ => [x, y, x - y],
                }
            };
            let t1 = [p(), p(), p()];
            let t2 = [p(), p(), p()];
            if is_zero_area(&t1) || is_zero_area(&t2) || t1.iter().any(|v| t2.contains(v)) {
                continue;
            }
            pairs += 1;
            let exact = triangles_meet(&t1, &t2);
            assert_eq!(exact, sat_meet(&t1, &t2), "{t1:?} {t2:?}");
            meet += usize::from(exact);
            if t2.iter().all(|&v| orient3d(t1[0], t1[1], t1[2], v) == 0) {
                coplanar += 1;
            }
            // Touching: they meet, but moving t2 by a tiny step along the separating direction
            // of some axis would part them; approximated by a vertex exactly on the other's plane.
            if exact
                && (t2.iter().any(|&v| orient3d(t1[0], t1[1], t1[2], v) == 0)
                    || t1.iter().any(|&v| orient3d(t2[0], t2[1], t2[2], v) == 0))
            {
                touching += 1;
            }
        }
        eprintln!(
            "sweep: {pairs} pairs agree with SAT; {meet} meet, {coplanar} coplanar, {touching} \
             meet with a vertex on the other's plane"
        );
        assert!(meet > 2_000, "only {meet} of {pairs} pairs meet");
        assert!(coplanar > 50, "only {coplanar} coplanar pairs");
        assert!(
            touching > 500,
            "only {touching} pairs with a vertex on the other's plane"
        );
    }

    /// The shared-vertex and shared-edge tests against their references (see the proptests),
    /// on a deterministic sweep that counts how many cases come out each way.
    #[test]
    fn shared_sweeps_agree_and_cover_both_outcomes() {
        let mut state: u64 = 0x2545_f491_4f6c_dd1d;
        let mut draws = 0u32;
        let mut next = || {
            state = state
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            f64::from(((state >> 33) % 9) as u8) * 0.5 - 2.0
        };
        let (mut vertex_cases, mut vertex_meet, mut edge_cases, mut edge_meet) = (0, 0, 0, 0);
        while vertex_cases < 20_000 || edge_cases < 20_000 {
            draws += 1;
            let planar = draws.is_multiple_of(2);
            let mut p = || {
                let (x, y, z) = (next(), next(), next());
                if planar { [x, y, x - y] } else { [x, y, z] }
            };
            let (s, a, b, c, d) = (p(), p(), p(), p(), p());
            if vertex_cases < 20_000
                && !is_zero_area(&[s, a, b])
                && !is_zero_area(&[s, c, d])
                && ![a, b].contains(&c)
                && ![a, b].contains(&d)
            {
                vertex_cases += 1;
                const K: f64 = 1024.0;
                let rel = |x: P3, k: f64| [k * (x[0] - s[0]), k * (x[1] - s[1]), k * (x[2] - s[2])];
                let reference = sat_meet(
                    &[rel(a, 1.0), rel(b, 1.0), rel(b, 1.0)],
                    &[[0.0; 3], rel(c, K), rel(d, K)],
                );
                let exact = vertex_neighbours_meet(s, a, b, c, d);
                assert_eq!(exact, reference, "{s:?} {a:?} {b:?} {c:?} {d:?}");
                vertex_meet += usize::from(exact);
            }
            // Edge s a shared; b and c the third vertices.
            if edge_cases < 20_000
                && !is_zero_area(&[s, a, b])
                && !is_zero_area(&[s, a, c])
                && b != c
            {
                edge_cases += 1;
                let m = [
                    (s[0] + a[0]) / 2.0,
                    (s[1] + a[1]) / 2.0,
                    (s[2] + a[2]) / 2.0,
                ];
                let y = [
                    m[0] + (c[0] - m[0]) / 1024.0,
                    m[1] + (c[1] - m[1]) / 1024.0,
                    m[2] + (c[2] - m[2]) / 1024.0,
                ];
                let exact = edge_neighbours_overlap(s, a, b, c);
                assert_eq!(
                    exact,
                    sat_meet(&[s, a, b], &[y, y, y]),
                    "{s:?} {a:?} {b:?} {c:?}"
                );
                edge_meet += usize::from(exact);
            }
        }
        eprintln!(
            "shared sweeps: vertex {vertex_meet} of {vertex_cases} meet; edge {edge_meet} of \
             {edge_cases} overlap"
        );
        assert!(vertex_meet > 1_000 && vertex_cases - vertex_meet > 1_000);
        assert!(edge_meet > 1_000 && edge_cases - edge_meet > 1_000);
    }

    #[test]
    fn shared_edge_cases() {
        let p = [0.0, 0.0, 0.0];
        let q = [1.0, 0.0, 0.0];
        // Coplanar, opposite sides: a flat continuation.
        assert!(!edge_neighbours_overlap(
            p,
            q,
            [0.0, 1.0, 0.0],
            [0.0, -1.0, 0.0]
        ));
        // Coplanar, same side: folded onto each other.
        assert!(edge_neighbours_overlap(
            p,
            q,
            [0.0, 1.0, 0.0],
            [0.5, 2.0, 0.0]
        ));
        // Any dihedral angle other than 0 or 180 degrees.
        assert!(!edge_neighbours_overlap(
            p,
            q,
            [0.0, 1.0, 0.0],
            [0.0, 1.0, 1e-300]
        ));
    }

    #[test]
    fn shared_vertex_cases() {
        let p = [0.0, 0.0, 0.0];
        // Two faces of a box corner: touch only at p.
        assert!(!vertex_neighbours_meet(
            p,
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.0, 0.0, 1.0],
            [0.0, -1.0, 1.0]
        ));
        // Coplanar fan triangles on either side of a line through p.
        assert!(!vertex_neighbours_meet(
            p,
            [1.0, 0.0, 0.0],
            [1.0, 1.0, 0.0],
            [-1.0, 0.0, 0.0],
            [-1.0, -1.0, 0.0]
        ));
        // Coplanar and overlapping near p.
        assert!(vertex_neighbours_meet(
            p,
            [2.0, 0.0, 0.0],
            [0.0, 2.0, 0.0],
            [1.0, 1.0, 0.0],
            [2.0, 2.0, 0.0]
        ));
        // A fin through the middle of the other triangle.
        assert!(vertex_neighbours_meet(
            p,
            [2.0, 0.0, 0.0],
            [0.0, 2.0, 0.0],
            [0.5, 0.5, -1.0],
            [0.5, 0.5, 1.0]
        ));
        // An edge through p lying along the other's edge, same direction: they share a segment.
        assert!(vertex_neighbours_meet(
            p,
            [2.0, 0.0, 0.0],
            [0.0, 2.0, 0.0],
            [1.0, 0.0, 0.0],
            [0.0, -1.0, 0.0]
        ));
        // Same line, opposite direction: only p.
        assert!(!vertex_neighbours_meet(
            p,
            [2.0, 0.0, 0.0],
            [0.0, 2.0, 0.0],
            [-1.0, 0.0, 0.0],
            [0.0, -1.0, 0.0]
        ));
    }

    #[test]
    fn crossing_classification() {
        let t = [[0.0, 0.0, 0.0], [4.0, 0.0, 0.0], [0.0, 4.0, 0.0]];
        let hit = segment_crossing([1.0, 1.0, -1.0], [1.0, 1.0, 3.0], &t);
        assert_eq!(
            hit,
            Crossing::Hit {
                t: 0.25,
                from_front: false,
                to_front: true
            }
        );
        assert_eq!(
            segment_crossing([2.0, 0.0, -1.0], [2.0, 0.0, 1.0], &t),
            Crossing::Degenerate
        );
        assert_eq!(
            segment_crossing([5.0, 5.0, -1.0], [5.0, 5.0, 1.0], &t),
            Crossing::Miss
        );
        assert_eq!(
            segment_crossing([1.0, 1.0, 0.0], [1.0, 1.0, 1.0], &t),
            Crossing::Degenerate
        );
        assert_eq!(
            segment_crossing([-1.0, 1.0, 0.0], [-1.0, 1.0, 1.0], &t),
            Crossing::Miss
        );
    }

    #[test]
    fn half_turns_follow_the_right_hand_rule() {
        let (u, v) = ([0.0, 0.0, 0.0], [0.0, 0.0, 1.0]);
        let r = [1.0, 0.0, 0.5];
        assert_eq!(half_turn(u, v, r, [2.0, 0.0, 0.3]), 0);
        assert_eq!(half_turn(u, v, r, [0.0, 1.0, 0.3]), 1);
        assert_eq!(half_turn(u, v, r, [-1.0, 0.0, 0.3]), 2);
        assert_eq!(half_turn(u, v, r, [0.0, -1.0, 0.3]), 3);
        assert_eq!(
            angular_order(u, v, [1.0, 1.0, 0.0], [0.0, 1.0, 0.0]),
            std::cmp::Ordering::Less
        );
    }

    #[test]
    fn zero_area_is_exact() {
        assert!(is_zero_area(&[
            [0.0, 0.0, 0.0],
            [1.0, 1.0, 1.0],
            [3.0, 3.0, 3.0]
        ]));
        assert!(is_zero_area(&[
            [0.1, 0.2, 0.3],
            [0.1, 0.2, 0.3],
            [1.0, 0.0, 0.0]
        ]));
        // 0.1 + 0.2 != 0.3 in binary: these three are not exactly collinear.
        assert!(!is_zero_area(&[
            [0.0, 0.0, 0.0],
            [0.1, 0.1, 0.1],
            [0.3, 0.3, 0.1 + 0.2]
        ]));
    }
}
