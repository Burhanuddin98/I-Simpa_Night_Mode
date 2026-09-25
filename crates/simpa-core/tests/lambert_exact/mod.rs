//! The exact `γ²` of a box's diffuse free paths, shared by `params_lambert.rs` (which checks it
//! against Bailey, Borwein and Crandall's cube) and `params_kuttruff.rs` (which uses it to measure
//! Kuttruff's formula apart from the transport's noise in `γ²`).
//!
//! Lambert reflection keeps a uniform, isotropic field in a closed room, so in a convex room the
//! free paths are the chords of lines uniform and isotropic in space, whose mean square is
//! `⟨ℓ²⟩ = I₂/I₀`, `I₀ = π·S/2`, `I₂ = ∫∫ |x − y|⁻² dx dy` over pairs of points in the room, by
//! the Blaschke–Petkantschin formula (`params_lambert.rs`'s module docs), and the mean is `4V/S`.

use std::f64::consts::PI;

pub type V3 = [f64; 3];

/// Gauss–Legendre nodes and weights on [−1, 1], by Newton's method on `P_n`.
pub fn gauss_legendre(n: usize) -> Vec<(f64, f64)> {
    (0..n)
        .map(|i| {
            let mut x = (PI * (i as f64 + 0.75) / (n as f64 + 0.5)).cos();
            let mut dp = 0.0;
            for _ in 0..100 {
                let (mut p0, mut p1) = (1.0, x);
                for k in 2..=n {
                    let k = k as f64;
                    (p0, p1) = (p1, ((2.0 * k - 1.0) * x * p1 - (k - 1.0) * p0) / k);
                }
                dp = n as f64 * (x * p1 - p0) / (x * x - 1.0);
                let dx = p1 / dp;
                x -= dx;
                if dx.abs() < 1e-16 {
                    break;
                }
            }
            (x, 2.0 / ((1.0 - x * x) * dp * dp))
        })
        .collect()
}

/// `∫∫ |x − y|⁻² dx dy` over the box `a × b × c`: `8·∫ Π(Lₖ − rₖ)/|r|² dr` over `[0, L]³`, and the
/// directions from the corner split by the far face each leaves through. Through `x = a` at
/// `(a, y, z)`, with `ρ² = a² + y² + z²`, the radial integral is exact and `dΩ = a·dy dz/ρ³`:
/// `∫∫ a²/ρ²·[bc/2 − (b·z + c·y)/6 + y·z/12] dy dz`, smooth on the face's rectangle.
pub fn box_i2([a, b, c]: V3) -> f64 {
    let gl = gauss_legendre(96);
    let face = |a: f64, b: f64, c: f64| -> f64 {
        let mut sum = 0.0;
        for &(u, wu) in &gl {
            let y = 0.5 * b * (u + 1.0);
            for &(v, wv) in &gl {
                let z = 0.5 * c * (v + 1.0);
                let j = b * c / 2.0 - (b * z + c * y) / 6.0 + y * z / 12.0;
                sum += 0.25 * b * c * wu * wv * a * a / (a * a + y * y + z * z) * j;
            }
        }
        sum
    };
    8.0 * (face(a, b, c) + face(b, c, a) + face(c, a, b))
}

/// The exact `γ²` of the free paths of a Lambert box.
pub fn box_gamma2(size: V3) -> f64 {
    let [a, b, c] = size;
    let (v, s) = (a * b * c, 2.0 * (a * b + b * c + a * c));
    let mean_square = 2.0 * box_i2(size) / (PI * s);
    mean_square / (4.0 * v / s).powi(2) - 1.0
}
