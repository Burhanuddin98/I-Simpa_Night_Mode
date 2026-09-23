//! The f64 geometry behind [`super::verify_mesh`]: orientation, the `f32` noise floor of a
//! tetrahedron's volume, point-to-triangle distance and the marker tolerance.
//!
//! Every input is an `f32` widened to f64, so differences and products carry about 29 more bits
//! than the data; the rounding of these f64 computations is negligible against the `f32`
//! tolerances below.

use crate::formats::cbin;

/// A point in f64.
pub type P = [f64; 3];

/// The unit roundoff of `f32`, 2^-24: a correctly rounded `f32` result lies within `U32 * |x|` of
/// the exact value `x`, and one `f32` ulp at `x` is at most `2 * U32 * |x|`.
pub const U32: f64 = f32::EPSILON as f64 / 2.0;

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

fn norm(a: P) -> f64 {
    dot(a, a).sqrt()
}

/// `(A−D)·((B−D)×(C−D))`, six times the signed volume. The `.mbin` convention is `< 0`
/// (`docs/formats/mbin.md`, "Conventions of upstream's meshes").
pub fn orient(p: [P; 4]) -> f64 {
    let d = p[3];
    dot(sub(p[0], d), cross(sub(p[1], d), sub(p[2], d)))
}

/// The sum over the four corners of |∂orient/∂corner|. The gradient with respect to a corner is
/// the cross product of two edges of the opposite face, so this is twice the surface area.
pub fn gradient_sum(p: [P; 4]) -> f64 {
    let twice_area = |a: P, b: P, c: P| norm(cross(sub(b, a), sub(c, a)));
    twice_area(p[1], p[2], p[3])
        + twice_area(p[0], p[2], p[3])
        + twice_area(p[0], p[1], p[3])
        + twice_area(p[0], p[1], p[2])
}

/// The largest |coordinate| of `points`, ignoring non-finite values.
pub fn max_abs(points: impl IntoIterator<Item = P>) -> f64 {
    points
        .into_iter()
        .flatten()
        .map(f64::abs)
        .filter(|x| x.is_finite())
        .fold(0.0, f64::max)
}

/// The `f32` noise floor of [`orient`] for a tetrahedron whose corners lie within `r` of the
/// origin on every axis and whose [`gradient_sum`] is `g`.
///
/// Moving each coordinate by one `f32` ulp, at most `2 * U32 * r`, moves a corner by at most
/// `sqrt(3) * 2 * U32 * r`, and to first order moves `orient` by at most that distance times `g`.
/// A tetrahedron with `|orient|` at or below this bound has an orientation, and a volume sign,
/// that its own stored coordinates cannot resolve.
pub fn volume_floor(r: f64, g: f64) -> f64 {
    2.0 * 3f64.sqrt() * U32 * r * g
}

/// How far a marked face's node may lie from the scene face its marker names, for a scene whose
/// largest |coordinate| is `r`: `16 * U32 * r`.
///
/// The `.cbin` vertices are the `f32` values TetGen was given (upstream writes the `.poly` and the
/// `.cbin` from the same `f32` vertices, `Objet3D_maillage.cpp:777, 941-942`; our mesher narrows
/// with `c as f32`, `config_xml/ids.rs:191`), and TetGen places a node on a facet in f64, exact to
/// f64 rounding. What then moves a node off its facet, per coordinate:
/// - narrowing TetGen's f64 node to `f32`: at most `U32 * r`;
/// - upstream's GUI only: `LoadNodeFile` carries every node through `CommonCoordsToGlCoords`
///   (`Objet3D_maillage.cpp:98`) and `GetTetraMesh` back through `GlCoordsToCommonCoords`
///   (`Objet3D_maillage.cpp:857`), `(x - c) * w` then `g / w + c` in `f32` (`Mathlib.h:50-67`).
///   The model centre `c` lies within `r`, so the subtraction, the multiplication and the
///   division each round a value of at most `2r` (in model units) and the final addition one of
///   at most `r`: `(2 + 2 + 2 + 1) * U32 * r`.
///
/// That is at most `8 * U32 * r` per coordinate and `sqrt(3) * 8 * U32 * r ≈ 13.9 * U32 * r` in
/// distance; the bound rounds it up to 16. For the hall, `r ≈ 40 m`, it is 38 µm, far below the
/// millimetres at least that separate a node from any other scene face it could wrongly name.
/// Upstream's `preprocess` merges vertices closer than 1e-4 (`computations.cpp:652-705`,
/// `mathlib.h:145`), which this bound does not cover: a mesh made from merged vertices fails.
pub fn marker_tolerance(scene: &cbin::Model) -> f64 {
    let r = max_abs(
        scene
            .vertices
            .iter()
            .map(|v| [v.x, v.y, v.z].map(f64::from)),
    );
    16.0 * U32 * r
}

/// Distance from `p` to segment `ab`.
fn point_segment_distance(p: P, a: P, b: P) -> f64 {
    let ab = sub(b, a);
    let len2 = dot(ab, ab);
    let t = if len2 > 0.0 {
        (dot(sub(p, a), ab) / len2).clamp(0.0, 1.0)
    } else {
        0.0
    };
    norm(sub(
        p,
        [a[0] + t * ab[0], a[1] + t * ab[1], a[2] + t * ab[2]],
    ))
}

/// Distance from `p` to the closed triangle `t`: the distance to its plane when `p` projects inside
/// it, otherwise the distance to its nearest edge. A degenerate triangle is its edges. NaN in,
/// NaN out.
pub fn point_triangle_distance(p: P, t: [P; 3]) -> f64 {
    let [a, b, c] = t;
    let n = cross(sub(b, a), sub(c, a));
    let nn = dot(n, n);
    if nn > 0.0 {
        // p projects inside when it lies on the inner side of all three edges.
        let inside = dot(cross(sub(b, a), sub(p, a)), n) >= 0.0
            && dot(cross(sub(c, b), sub(p, b)), n) >= 0.0
            && dot(cross(sub(a, c), sub(p, c)), n) >= 0.0;
        if inside {
            return dot(sub(p, a), n).abs() / nn.sqrt();
        }
    }
    let d = [
        point_segment_distance(p, a, b),
        point_segment_distance(p, b, c),
        point_segment_distance(p, c, a),
    ];
    if d.iter().any(|x| x.is_nan()) {
        return f64::NAN;
    }
    d[0].min(d[1]).min(d[2])
}

#[cfg(test)]
mod tests {
    use super::*;

    const T: [P; 3] = [[0.0, 0.0, 0.0], [2.0, 0.0, 0.0], [0.0, 2.0, 0.0]];

    #[test]
    fn distance_inside_is_the_plane_distance() {
        assert_eq!(point_triangle_distance([0.5, 0.5, 3.0], T), 3.0);
        assert_eq!(point_triangle_distance([0.5, 0.5, -3.0], T), 3.0);
        assert_eq!(point_triangle_distance([1.0, 1.0, 0.0], T), 0.0);
    }

    #[test]
    fn distance_outside_is_the_edge_or_corner_distance() {
        // Beyond the hypotenuse, in the plane: distance to the edge (2,0)-(0,2).
        let d = point_triangle_distance([2.0, 2.0, 0.0], T);
        assert!((d - 2f64.sqrt()).abs() < 1e-15, "{d}");
        // Beyond a corner, off the plane.
        let d = point_triangle_distance([-3.0, -4.0, 12.0], T);
        assert!((d - 13.0).abs() < 1e-15, "{d}");
    }

    #[test]
    fn degenerate_triangle_is_its_edges() {
        let flat = [[0.0, 0.0, 0.0], [2.0, 0.0, 0.0], [4.0, 0.0, 0.0]];
        assert_eq!(point_triangle_distance([1.0, 3.0, 0.0], flat), 3.0);
        let point = [[1.0, 1.0, 1.0]; 3];
        assert_eq!(point_triangle_distance([1.0, 1.0, 3.0], point), 2.0);
        assert!(point_triangle_distance([f64::NAN, 0.0, 0.0], T).is_nan());
    }

    #[test]
    fn orient_follows_the_mbin_convention() {
        let p = [
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.0, 0.0, 1.0],
        ];
        assert_eq!(orient(p), -1.0);
        assert_eq!(orient([p[1], p[0], p[2], p[3]]), 1.0);
        // Four faces of areas 1/2, 1/2, 1/2 and sqrt(3)/2.
        assert!((gradient_sum(p) - (3.0 + 3f64.sqrt())).abs() < 1e-15);
    }
}
