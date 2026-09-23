//! Where a point lies relative to the project's triangles.
//!
//! # Method
//!
//! - **Clearance:** the exact Euclidean distance from the point to the nearest triangle (the
//!   closest-point construction of Ericson, *Real-Time Collision Detection*, 5.1.5), taken over
//!   every triangle. A degenerate triangle is measured as its three edges.
//! - **Inside or outside:** the generalised winding number `w`, the sum of the signed solid angles
//!   the triangles subtend at the point (Van Oosterom and Strackee's formula) divided by 4π. The
//!   point is inside when `|w| >= 1/2`.
//!
//! For a closed, consistently oriented shell `w` is exactly +1 or -1 inside (by the shell's
//! orientation, outward or inward) and 0 outside, up to rounding of a few ulps per triangle, so
//! the threshold 1/2 is never close for a point that is not on the surface; a point on or within
//! [`ON_SURFACE_M`] of a face is reported as [`PointLocation::OnSurface`]
//! before the winding number is consulted.
//!
//! # Limits
//!
//! - **The outer shell must be closed and consistently oriented.** An open shell gives a
//!   fractional `w` near the hole, and a shell whose triangles disagree in orientation cancels
//!   itself. Checking and repairing shells is the geometry stage's job (M4); this test trusts it.
//! - **Internal facets** (partitions, hanging reflectors) are tolerated: a planar sheet subtends
//!   less than a hemisphere, so it moves `w` by less than 1/2 and cannot flip the result on its
//!   own. Several large internal sheets close to the point can add up past 1/2; a curved sheet
//!   that nearly surrounds the point can too.
//! - **A closed internal shell** (a fitting zone made of surface groups) must share the outer
//!   shell's orientation. A point inside it then has `|w| = 2`; with the opposite orientation
//!   `w` cancels to 0 and the point is reported outside.
//! - **Arithmetic is `f64`.** The solver works in `f32` (`coreString.h:41`); a point within about
//!   1e-7 of the room's size from a face can land on the other side once rounded. The rules that
//!   use this module keep a clearance ([`SOURCE_CLEARANCE_M`](super::SOURCE_CLEARANCE_M), the
//!   receiver radius) far larger than that.
//! - **Non-finite coordinates** make every point outside.
//! - **Cost** is one pass over the triangles per point: fine for the tens of points a project
//!   has, not meant for millions.

use std::f64::consts::PI;

use super::ON_SURFACE_M;
use crate::schema::{Geometry, Vec3};

type P3 = [f64; 3];

/// Where a point lies relative to the project's triangles.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PointLocation {
    /// Outside the closed volume, or the geometry or point is not finite.
    Outside { distance_m: f64 },
    /// On a face: within [`ON_SURFACE_M`] of one.
    OnSurface { distance_m: f64 },
    /// Strictly inside; `clearance_m` is the distance to the nearest face.
    Inside { clearance_m: f64 },
}

/// The triangles of `geometry`, skipping faces whose vertex indices are out of range (a
/// structural fault reported elsewhere).
pub(super) fn triangles(geometry: &Geometry) -> Vec<[P3; 3]> {
    let v = &geometry.vertices;
    geometry
        .faces
        .iter()
        .filter_map(|f| {
            let [a, b, c] = f.vertices.map(|i| v.get(i as usize).map(|p| p.to_array()));
            Some([a?, b?, c?])
        })
        .collect()
}

/// Classifies `point` against every face of `geometry`. See the module docs for the method and
/// its limits.
pub fn locate_point(geometry: &Geometry, point: Vec3) -> PointLocation {
    locate(&triangles(geometry), point.to_array())
}

pub(super) fn locate(triangles: &[[P3; 3]], p: P3) -> PointLocation {
    if !p.iter().all(|c| c.is_finite()) {
        return PointLocation::Outside {
            distance_m: f64::NAN,
        };
    }
    let mut distance = f64::INFINITY;
    let mut finite = true;
    for t in triangles {
        let d = distance_to_triangle(p, t);
        finite &= d.is_finite();
        distance = distance.min(d);
    }
    if !finite {
        return PointLocation::Outside {
            distance_m: f64::NAN,
        };
    }
    if distance <= ON_SURFACE_M {
        return PointLocation::OnSurface {
            distance_m: distance,
        };
    }
    if winding_number(triangles, p).abs() >= 0.5 {
        PointLocation::Inside {
            clearance_m: distance,
        }
    } else {
        PointLocation::Outside {
            distance_m: distance,
        }
    }
}

/// The generalised winding number of `triangles` at `p`: their total signed solid angle over 4π.
pub(super) fn winding_number(triangles: &[[P3; 3]], p: P3) -> f64 {
    triangles.iter().map(|t| solid_angle(p, t)).sum::<f64>() / (4.0 * PI)
}

fn sub(a: P3, b: P3) -> P3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn add(a: P3, b: P3) -> P3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

fn scale(a: P3, s: f64) -> P3 {
    [a[0] * s, a[1] * s, a[2] * s]
}

fn dot(a: P3, b: P3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

pub(super) fn cross(a: P3, b: P3) -> P3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

pub(super) fn norm(a: P3) -> f64 {
    dot(a, a).sqrt()
}

/// Signed solid angle of triangle `t` seen from `p` (Van Oosterom and Strackee, 1983):
/// `tan(Ω/2) = a·(b×c) / (|a||b||c| + (a·b)|c| + (a·c)|b| + (b·c)|a|)`.
fn solid_angle(p: P3, t: &[P3; 3]) -> f64 {
    let a = sub(t[0], p);
    let b = sub(t[1], p);
    let c = sub(t[2], p);
    let (la, lb, lc) = (norm(a), norm(b), norm(c));
    let numerator = dot(a, cross(b, c));
    let denominator = la * lb * lc + dot(a, b) * lc + dot(a, c) * lb + dot(b, c) * la;
    2.0 * numerator.atan2(denominator)
}

fn distance_to_segment(p: P3, a: P3, b: P3) -> f64 {
    let ab = sub(b, a);
    let len2 = dot(ab, ab);
    let t = if len2 > 0.0 {
        (dot(sub(p, a), ab) / len2).clamp(0.0, 1.0)
    } else {
        0.0
    };
    norm(sub(p, add(a, scale(ab, t))))
}

/// Exact distance from `p` to the closed triangle `t`.
pub(super) fn distance_to_triangle(p: P3, t: &[P3; 3]) -> f64 {
    let [a, b, c] = *t;
    let ab = sub(b, a);
    let ac = sub(c, a);
    if norm(cross(ab, ac)) == 0.0 {
        // Degenerate: a segment or a point.
        return distance_to_segment(p, a, b)
            .min(distance_to_segment(p, b, c))
            .min(distance_to_segment(p, c, a));
    }
    let closest = {
        let ap = sub(p, a);
        let d1 = dot(ab, ap);
        let d2 = dot(ac, ap);
        let bp = sub(p, b);
        let d3 = dot(ab, bp);
        let d4 = dot(ac, bp);
        let cp = sub(p, c);
        let d5 = dot(ab, cp);
        let d6 = dot(ac, cp);
        let vc = d1 * d4 - d3 * d2;
        let vb = d5 * d2 - d1 * d6;
        let va = d3 * d6 - d5 * d4;
        if d1 <= 0.0 && d2 <= 0.0 {
            a
        } else if d3 >= 0.0 && d4 <= d3 {
            b
        } else if vc <= 0.0 && d1 >= 0.0 && d3 <= 0.0 {
            add(a, scale(ab, d1 / (d1 - d3)))
        } else if d6 >= 0.0 && d5 <= d6 {
            c
        } else if vb <= 0.0 && d2 >= 0.0 && d6 <= 0.0 {
            add(a, scale(ac, d2 / (d2 - d6)))
        } else if va <= 0.0 && (d4 - d3) >= 0.0 && (d5 - d6) >= 0.0 {
            add(b, scale(sub(c, b), (d4 - d3) / ((d4 - d3) + (d5 - d6))))
        } else {
            let denom = 1.0 / (va + vb + vc);
            add(a, add(scale(ab, vb * denom), scale(ac, vc * denom)))
        }
    };
    norm(sub(p, closest))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The 12 outward triangles of the box [0, 5]^3, as in upstream's cube.cbin.
    fn cube(flip: bool) -> Vec<[P3; 3]> {
        let v: [P3; 8] = [
            [5.0, 0.0, 0.0],
            [0.0, 0.0, 0.0],
            [0.0, 5.0, 0.0],
            [5.0, 5.0, 0.0],
            [0.0, 5.0, 5.0],
            [5.0, 5.0, 5.0],
            [0.0, 0.0, 5.0],
            [5.0, 0.0, 5.0],
        ];
        let f: [[usize; 3]; 12] = [
            [0, 1, 2],
            [0, 2, 3],
            [2, 4, 5],
            [2, 5, 3],
            [2, 6, 4],
            [2, 1, 6],
            [1, 0, 7],
            [6, 1, 7],
            [0, 3, 5],
            [7, 0, 5],
            [7, 5, 4],
            [6, 7, 4],
        ];
        f.iter()
            .map(|&[a, b, c]| {
                if flip {
                    [v[a], v[c], v[b]]
                } else {
                    [v[a], v[b], v[c]]
                }
            })
            .collect()
    }

    #[test]
    fn winding_number_is_one_inside_and_zero_outside_either_orientation() {
        for flip in [false, true] {
            let t = cube(flip);
            let w = winding_number(&t, [1.0, 2.0, 3.0]);
            assert!((w.abs() - 1.0).abs() < 1e-12, "{w}");
            let w = winding_number(&t, [6.0, 2.0, 3.0]);
            assert!(w.abs() < 1e-12, "{w}");
            let w = winding_number(&t, [2.5, 2.5, 5.0 + 1e-6]);
            assert!(w.abs() < 1e-6, "{w}");
            let w = winding_number(&t, [2.5, 2.5, 5.0 - 1e-6]);
            assert!((w.abs() - 1.0).abs() < 1e-6, "{w}");
        }
    }

    #[test]
    fn locate_reports_clearance_and_surface() {
        let t = cube(false);
        assert_eq!(
            locate(&t, [3.0, 1.25, 2.5]),
            PointLocation::Inside { clearance_m: 1.25 }
        );
        assert!(matches!(
            locate(&t, [3.0, 1.25, 0.0]),
            PointLocation::OnSurface { .. }
        ));
        assert!(matches!(
            locate(&t, [5.0, 5.0, 5.0]),
            PointLocation::OnSurface { .. }
        ));
        match locate(&t, [6.0, 1.25, 2.5]) {
            PointLocation::Outside { distance_m } => assert_eq!(distance_m, 1.0),
            other => panic!("{other:?}"),
        }
        match locate(&t, [6.0, 6.0, 6.0]) {
            PointLocation::Outside { distance_m } => {
                assert!((distance_m - 3f64.sqrt()).abs() < 1e-15)
            }
            other => panic!("{other:?}"),
        }
        assert!(matches!(
            locate(&t, [f64::NAN, 1.0, 1.0]),
            PointLocation::Outside { .. }
        ));
        assert!(matches!(
            locate(&[], [1.0, 1.0, 1.0]),
            PointLocation::Outside { .. }
        ));
    }

    #[test]
    fn an_internal_partition_does_not_flip_the_result() {
        // The cube split by the plane x = 2.5, one sheet of two triangles.
        let mut t = cube(false);
        t.push([[2.5, 0.0, 0.0], [2.5, 5.0, 0.0], [2.5, 5.0, 5.0]]);
        t.push([[2.5, 0.0, 0.0], [2.5, 5.0, 5.0], [2.5, 0.0, 5.0]]);
        for x in [0.3, 2.0, 2.4, 2.6, 3.0, 4.7] {
            let w = winding_number(&t, [x, 2.5, 2.5]).abs();
            assert!((0.5..1.5).contains(&w), "x = {x}: {w}");
        }
        for x in [-1.0, 6.0] {
            let w = winding_number(&t, [x, 2.5, 2.5]).abs();
            assert!(w < 0.5, "x = {x}: {w}");
        }
    }

    #[test]
    fn distance_to_a_degenerate_triangle_is_the_distance_to_its_edges() {
        let t = [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [2.0, 0.0, 0.0]];
        assert_eq!(distance_to_triangle([1.0, 1.0, 0.0], &t), 1.0);
        let t = [[1.0, 1.0, 1.0]; 3];
        assert_eq!(distance_to_triangle([1.0, 1.0, 3.0], &t), 2.0);
    }
}
