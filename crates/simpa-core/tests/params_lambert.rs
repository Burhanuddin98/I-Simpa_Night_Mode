//! The diffuse transport of `params::lambert` against known answers (`docs/params.md`,
//! "Kuttruff's reference"; the pre-M8 piece, Burhan 2026-09-24 23:14: `γ²` from the geometry,
//! never fitted to SPPS). Each check has its say-NO partner.
//!
//! **The known answers.** Lambert reflection keeps a uniform, isotropic field in a closed room, so
//! the free paths between reflections have the mean `4V/S` in any room, convex or not. In a convex
//! room they are then the chords of lines uniform and isotropic in space, whose mean square is
//!
//! `⟨ℓ²⟩ = I₂/I₀`, `I₀ = π·S/2`, `I₂ = ∫∫ |x − y|⁻² dx dy` over pairs of points in the room,
//!
//! by the Blaschke–Petkantschin formula (`dx dy = |t₁ − t₂|²·dt₁ dt₂ dG` for two points on a line
//! `G`, so `∫∫ |x − y|⁻² dx dy = ∫ σ(G)² dG`, `σ` the chord; `∫ dG = π·S/2` over the lines meeting
//! the room: Cauchy). Derived here; the chord power integrals are Santaló's, *Integral Geometry and
//! Geometric Probability* (1976), not opened here. For a box the six-dimensional integral folds into
//! three smooth two-dimensional ones (`lambert_exact::box_i2`); for the unit cube it is Bailey,
//! Borwein and Crandall's box integral `Δ₃(−2)`, whose closed form they give ("Advances in the
//! theory of box integrals", Math. Comp. 79 (2010) 1839-1866, Table 6; read in the authors' copy,
//! `davidhbailey.com/dhbpapers/BoxII.pdf`, and their eq. (69) is `box_i2`'s integrand). For a
//! sphere the chord is `2R·cos θ` with Lambert's `θ`, so `γ²` = 1/8 exactly.
//!
//! **What is tested where.** `free_paths` takes the room alone, at fixed settings: it is what
//! `params::room::kuttruff_rt` takes, and the checks that bear on the reference use it. The
//! transport's behaviour at other settings and starting points (a finer run for tighter known
//! answers, the rays' start, the correlation of successive paths, a seed search) is studied
//! through `free_path_study`, which exists in test builds only and gives a `FreePathStudy`, which
//! no function of `params::room` takes. The two share every line of the transport:
//! `free_paths` equals `free_path_study` at the same settings to the bit.

mod lambert_exact;

use std::f64::consts::PI;

use lambert_exact::{V3, box_gamma2, box_i2, gauss_legendre};
use simpa_core::faults::{self, Fault};
use simpa_core::params::codes;
use simpa_core::params::lambert::{
    Enclosure, FreePathSettings, FreePathStudy, FreePaths, GAMMA2_SE_LIMIT, free_path_study,
    free_paths,
};
use simpa_core::params::room::{RtConstant, Surface, kuttruff_rt};

/// More rays than `STANDARD`, so that `γ²`'s standard error is about 0.0002.
const FINE: FreePathSettings = FreePathSettings {
    replicas: 16,
    rays_per_replica: 8192,
    burn_in_paths: 32,
    paths_per_ray: 64,
    seed: 0x6b6e_6f77_6e5f_616e,
};

/// Catalan's constant `G` (OEIS A006752).
const CATALAN: f64 = 0.915_965_594_177_219_015_054_603_514_932_384_110_774;

/// The inverse tangent integral `Ti₂(x) = ∫₀ˣ arctan(t)/t dt = Σ (−1)ᵏ x²ᵏ⁺¹/(2k + 1)²`, `|x| < 1`.
fn ti2(x: f64) -> f64 {
    let mut sum = 0.0;
    let mut p = x;
    for k in 0..200 {
        let term = p / ((2 * k + 1) as f64).powi(2);
        sum += if k % 2 == 0 { term } else { -term };
        if term.abs() < 1e-18 {
            break;
        }
        p *= x * x;
    }
    sum
}

/// Bailey, Borwein and Crandall's `Δ₃(−2)`, the mean of `|x − y|⁻²` over two points of the unit
/// cube (Table 6): `2π − 12G + 12·Ti₂(3 − 2√2) + 6π·ln(1 + √2) + 2·ln 2 − (5/2)·ln 3 −
/// 8√2·arctan(1/√2)`.
fn bbc_delta3_minus2() -> f64 {
    let r2 = 2f64.sqrt();
    2.0 * PI - 12.0 * CATALAN
        + 12.0 * ti2(3.0 - 2.0 * r2)
        + 6.0 * PI * (1.0 + r2).ln()
        + 2.0 * 2f64.ln()
        - 2.5 * 3f64.ln()
        - 8.0 * r2 * (1.0 / r2).atan()
}

/// M8's rooms and the cube, with their exact `γ²`.
fn rooms() -> [(&'static str, V3, f64); 3] {
    [
        ("cube", [1.0, 1.0, 1.0], box_gamma2([1.0, 1.0, 1.0])),
        ("6x10x3", [6.0, 10.0, 3.0], box_gamma2([6.0, 10.0, 3.0])),
        ("5x4x3", [5.0, 4.0, 3.0], box_gamma2([5.0, 4.0, 3.0])),
    ]
}

fn study(room: &Enclosure, settings: &FreePathSettings) -> FreePathStudy {
    free_path_study(room, settings, None).unwrap()
}

fn assert_refused(r: Result<FreePaths, simpa_core::params::ParamError>, needle: &str) {
    match r {
        Ok(p) => panic!("accepted: {p:?}"),
        Err(e) => {
            assert_eq!(e.code(), codes::TRANSPORT_REFUSED, "{e}");
            assert!(e.to_string().contains(needle), "{e}");
        }
    }
}

#[test]
fn the_box_integral_reproduces_bailey_borwein_crandalls_cube() {
    let closed = bbc_delta3_minus2();
    // Their value to the digits the closed form gives in double precision (mpmath, 30 digits:
    // 5.6337151581361093110).
    assert!((closed - 5.633_715_158_136_109).abs() < 1e-13, "{closed}");
    let quad = box_i2([1.0, 1.0, 1.0]);
    assert!((quad - closed).abs() < 1e-12, "{quad} against {closed}");
    // So the cube's γ² is 3·Δ₃(−2)/(4π) − 1.
    let g = box_gamma2([1.0, 1.0, 1.0]);
    assert!((g - (3.0 * closed / (4.0 * PI) - 1.0)).abs() < 1e-12);
    assert!((g - 0.344_950_423_083_65).abs() < 1e-12, "{g}");
    // M8's rooms, by the same quadrature (numpy's Gauss–Legendre at 100 to 800 nodes agrees with
    // these to 1e-15).
    assert!((box_gamma2([6.0, 10.0, 3.0]) - 0.388_874_084_782_36).abs() < 1e-11);
    assert!((box_gamma2([5.0, 4.0, 3.0]) - 0.352_401_468_954_72).abs() < 1e-11);
    // Says no: the kernel's power, or a face left out, moves it far off.
    let one_face = 8.0 * {
        let gl = gauss_legendre(96);
        let mut sum = 0.0;
        for &(u, wu) in &gl {
            let y = 0.5 * (u + 1.0);
            for &(v, wv) in &gl {
                let z = 0.5 * (v + 1.0);
                let j = 0.5 - (z + y) / 6.0 + y * z / 12.0;
                sum += 0.25 * wu * wv / (1.0 + y * y + z * z) * j;
            }
        }
        sum
    };
    assert!((one_face - closed).abs() > 1.0, "{one_face}");
}

#[test]
fn free_paths_have_the_mean_4v_over_s_and_the_exact_gamma2_of_the_cube_and_m8s_rooms() {
    for (name, size, exact) in rooms() {
        let room = Enclosure::shoebox(size).unwrap();
        let p = study(&room, &FINE);
        let mfp_off = p.mean_free_path_off_se();
        let g_off = (p.gamma2 - exact) / p.gamma2_se;
        println!(
            "{name}: mean free path {:.5} ± {:.5} m against 4V/S {:.5} m ({mfp_off:+.2} SE); \
             gamma^2 {:.5} ± {:.5} against {exact:.6} ({g_off:+.2} SE); {} paths",
            p.mean_free_path_m,
            p.mean_free_path_se_m,
            room.four_v_over_s_m(),
            p.gamma2,
            p.gamma2_se,
            p.paths
        );
        // The errors are small enough to mean something, and the values are within 3 of them.
        assert!(
            p.mean_free_path_se_m < 5e-4 * room.four_v_over_s_m(),
            "{name}"
        );
        assert!(p.gamma2_se < 4e-4, "{name}: {}", p.gamma2_se);
        assert!(mfp_off.abs() < 3.0, "{name}: {mfp_off}");
        assert!(g_off.abs() < 3.0, "{name}: {g_off}");
        // What core::results uses, the room alone at the fixed settings, gives the same within its
        // larger error, and is the transport studied here at those settings, to the bit.
        let s = free_paths(&room).unwrap();
        println!(
            "{name}: free_paths gamma^2 {:.5} ± {:.5} ({:+.2} SE)",
            s.gamma2(),
            s.gamma2_se(),
            (s.gamma2() - exact) / s.gamma2_se()
        );
        assert!(
            ((s.gamma2() - exact) / s.gamma2_se()).abs() < 3.0,
            "{name}: {s:?}"
        );
        let same = study(&room, &FreePathSettings::STANDARD);
        assert_eq!(
            (
                s.mean_free_path_m(),
                s.mean_free_path_se_m(),
                s.gamma2(),
                s.gamma2_se(),
                s.paths()
            ),
            (
                same.mean_free_path_m,
                same.mean_free_path_se_m,
                same.gamma2,
                same.gamma2_se,
                same.paths
            ),
            "{name}"
        );
    }
    // Says no: the rooms' γ² differ from one another by tens of standard errors, so a transport
    // that gave one room's value for another would fail above.
    let [cube, wide, small] = rooms();
    assert!((wide.2 - small.2).abs() > 0.03 && (small.2 - cube.2).abs() > 0.007);
}

#[test]
fn published_values_of_gamma2() {
    // Bies and Hansen's fit for rectangular rooms, as Arup's Strutt help quotes it
    // (`strutt.arup.com/help/Building_Acoustics/RTInsert.htm`, citing Engineering Noise Control,
    // 4th ed., eq. 7.64; the book not opened): γ = √(0.0179·(L + W)/H − 0.0001·(L − W)/H −
    // 0.0011·((L − W)/H)² + 0.3025), H the height. A fit whose accuracy the page does not state:
    // the exact values sit within 0.007 of it, the bound below ours.
    let fit = |l: f64, w: f64, h: f64| {
        0.0179 * (l + w) / h - 0.0001 * (l - w) / h - 0.0011 * ((l - w) / h).powi(2) + 0.3025
    };
    for (name, size, exact) in rooms() {
        let [x, y, h] = size;
        let f = fit(x.max(y), x.min(y), h);
        println!("{name}: exact {exact:.4}, Bies and Hansen's fit {f:.4}");
        assert!((exact - f).abs() < 0.01, "{name}: {exact} against {f}");
    }
    // Stephenson (ICA 2016, paper ICA2016-556's longer version, section 6.3): "for typical
    // proportions from 1:1:1 to 1:10:10, the variance γ² is in the order of 0.4…0.6 (as found by
    // numerical experiments)". The exact values run from 0.345 to 0.653: the same order, and no
    // check. Recorded, not asserted beyond that.
    let flat = box_gamma2([10.0, 10.0, 1.0]);
    assert!((0.6..0.7).contains(&flat), "{flat}");
}

/// The M7 follow-ups' `tests/lambert_box.rs` counted, at α 0.05, the paths of rays started at the
/// source from each ray's first reflection until its time ran out: 1.6·1.12·T_Eyring, 482.7 mean
/// free paths of travel in both rooms, so about 482 paths a ray.
const LAMBERT_BOX_PATHS: u32 = 482;

#[test]
fn lambert_boxs_values_are_its_counting_and_the_exact_ones_are_the_diffuse_fields() {
    // lambert_box.rs reported γ² 0.388 (6×10×3 m) and 0.352 (5×4×3 m), to three decimals, at
    // α 0.05, with no stated uncertainty. The exact values are 0.38887 and 0.35240: 0.388 lies
    // outside the exact value's rounding. Run the way lambert_box.rs counted (every ray from the
    // source, every path from the first reflection, 482 a ray), this transport gives lambert_box's
    // values to their rounding; run as `free_paths` runs (starts spread over the volume, 32 paths
    // left out), the exact ones. So the difference is the counting's, not the transport's: a
    // ray's first reflections remember where it started.
    assert!((box_gamma2([6.0, 10.0, 3.0]) - 0.388).abs() > 0.0005);
    let rooms = [
        ("6x10x3", [6.0, 10.0, 3.0], [3.0, 5.0, 1.8], 0.388),
        ("5x4x3", [5.0, 4.0, 3.0], [2.52, 1.97, 1.53], 0.352),
    ];
    let as_lambert_box = FreePathSettings {
        replicas: 16,
        rays_per_replica: 4096,
        burn_in_paths: 0,
        paths_per_ray: LAMBERT_BOX_PATHS,
        seed: 0x6c62_6f78_5f63_6f75,
    };
    for (name, size, source, reported) in rooms {
        let exact = box_gamma2(size);
        let room = Enclosure::shoebox(size).unwrap();
        let counted = free_path_study(&room, &as_lambert_box, Some(source)).unwrap();
        let diffuse = study(&room, &FINE);
        println!(
            "{name}: lambert_box {reported}; this transport counted as lambert_box counted \
             {:.5} ± {:.5} (mean free path {:+.2} SE from 4V/S), as free_paths counts {:.5} ± \
             {:.5}; exact {exact:.5}",
            counted.gamma2,
            counted.gamma2_se,
            counted.mean_free_path_off_se(),
            diffuse.gamma2,
            diffuse.gamma2_se
        );
        // lambert_box's value to its rounding, within this run's error.
        assert!(
            (counted.gamma2 - reported).abs() < 0.0005 + 3.0 * counted.gamma2_se,
            "{name}: {counted:?}"
        );
        // Says no: its counting reads measurably below the exact value, and the diffuse field's
        // own counting does not.
        assert!(
            exact - counted.gamma2 > 3.0 * counted.gamma2_se,
            "{name}: {counted:?}"
        );
        assert!(
            (diffuse.gamma2 - exact).abs() < 3.0 * diffuse.gamma2_se,
            "{name}: {diffuse:?}"
        );
        // Within one unit of the third decimal, lambert_box and the exact value agree.
        assert!((exact - reported).abs() < 0.001, "{name}");
    }
}

#[test]
fn the_first_paths_are_left_out_because_they_remember_the_start() {
    // Even from starts drawn evenly from the volume, a ray's first reflection is not yet the
    // diffuse field's: it falls where long paths end, weighted by their length. Counted from it
    // (0 left out), γ² reads low by several standard errors in the box and the mean free path
    // sits several standard errors from 4V/S in the L-shaped room; with 16 or 64 left out they are
    // the exact value and 4V/S. The study checks nothing itself, so every run is asserted here.
    let (l_tris, l_cells) = l_room();
    let rooms = [
        (
            "6x10x3",
            Enclosure::shoebox([6.0, 10.0, 3.0]).unwrap(),
            Some(box_gamma2([6.0, 10.0, 3.0])),
        ),
        ("L", Enclosure::from_mesh(&l_tris, &l_cells).unwrap(), None),
    ];
    for (name, room, exact) in &rooms {
        for burn_in_paths in [0, 16, 64] {
            let p = study(
                room,
                &FreePathSettings {
                    burn_in_paths,
                    ..FINE
                },
            );
            println!(
                "{name}, {burn_in_paths} left out: mean free path {:+.2} SE from 4V/S, gamma^2 \
                 {:.5} ± {:.5}",
                p.mean_free_path_off_se(),
                p.gamma2,
                p.gamma2_se
            );
            match (burn_in_paths, exact) {
                (0, Some(exact)) => {
                    assert!(exact - p.gamma2 > 3.0 * p.gamma2_se, "{name}: {p:?}")
                }
                (0, None) => assert!(p.mean_free_path_off_se().abs() > 3.0, "{name}: {p:?}"),
                (_, exact) => {
                    assert!(
                        p.mean_free_path_off_se().abs() < 3.0,
                        "{name}, {burn_in_paths}"
                    );
                    if let Some(exact) = exact {
                        assert!(
                            (p.gamma2 - exact).abs() < 3.0 * p.gamma2_se,
                            "{name}, {burn_in_paths}"
                        );
                    }
                }
            }
        }
    }
}

/// An icosahedron subdivided `level` times, its vertices on the unit sphere.
fn icosphere(level: u32) -> Vec<[V3; 3]> {
    let t = (1.0 + 5f64.sqrt()) / 2.0;
    let unit = |p: V3| {
        let n = (p[0] * p[0] + p[1] * p[1] + p[2] * p[2]).sqrt();
        [p[0] / n, p[1] / n, p[2] / n]
    };
    let v: Vec<V3> = [
        [-1.0, t, 0.0],
        [1.0, t, 0.0],
        [-1.0, -t, 0.0],
        [1.0, -t, 0.0],
        [0.0, -1.0, t],
        [0.0, 1.0, t],
        [0.0, -1.0, -t],
        [0.0, 1.0, -t],
        [t, 0.0, -1.0],
        [t, 0.0, 1.0],
        [-t, 0.0, -1.0],
        [-t, 0.0, 1.0],
    ]
    .map(unit)
    .to_vec();
    let f: [[usize; 3]; 20] = [
        [0, 11, 5],
        [0, 5, 1],
        [0, 1, 7],
        [0, 7, 10],
        [0, 10, 11],
        [1, 5, 9],
        [5, 11, 4],
        [11, 10, 2],
        [10, 7, 6],
        [7, 1, 8],
        [3, 9, 4],
        [3, 4, 2],
        [3, 2, 6],
        [3, 6, 8],
        [3, 8, 9],
        [4, 9, 5],
        [2, 4, 11],
        [6, 2, 10],
        [8, 6, 7],
        [9, 8, 1],
    ];
    let mut tris: Vec<[V3; 3]> = f.iter().map(|t| [v[t[0]], v[t[1]], v[t[2]]]).collect();
    let mid = |a: V3, b: V3| unit([a[0] + b[0], a[1] + b[1], a[2] + b[2]]);
    for _ in 0..level {
        tris = tris
            .iter()
            .flat_map(|&[a, b, c]| {
                let (ab, bc, ca) = (mid(a, b), mid(b, c), mid(c, a));
                [[a, ab, ca], [b, bc, ab], [c, ca, bc], [ab, bc, ca]]
            })
            .collect();
    }
    tris
}

/// The tetrahedra from the origin to each face: they fill a room that is star-shaped about the
/// origin, as the sphere is.
fn fan(tris: &[[V3; 3]]) -> Vec<[V3; 4]> {
    tris.iter().map(|&[a, b, c]| [[0.0; 3], a, b, c]).collect()
}

#[test]
fn a_sphere_of_triangles_gives_one_eighth_whatever_its_faces_orientation() {
    // 20,480 faces: the polyhedron's γ² lies above 1/8 by its facets, measured to fall fourfold a
    // subdivision (+0.0172, +0.0044, +0.0011, +0.00025, +0.00004 from 80 to 20,480 faces).
    let tris = icosphere(5);
    let settings = FreePathSettings {
        rays_per_replica: 4096,
        ..FreePathSettings::STANDARD
    };
    let room = Enclosure::from_mesh(&tris, &fan(&tris)).unwrap();
    let p = study(&room, &settings);
    println!(
        "sphere, {} faces: mean free path {:+.2} SE from 4V/S; gamma^2 {:.5} ± {:.5}",
        tris.len(),
        p.mean_free_path_off_se(),
        p.gamma2,
        p.gamma2_se
    );
    assert!(p.mean_free_path_off_se().abs() < 3.0);
    assert!((p.gamma2 - 0.125).abs() < 1e-4 + 3.0 * p.gamma2_se, "{p:?}");
    // What core::results would give it: accepted, within its error.
    let r = free_paths(&room).unwrap();
    println!(
        "sphere, free_paths: gamma^2 {:.5} ± {:.5}",
        r.gamma2(),
        r.gamma2_se()
    );
    assert!(
        (r.gamma2() - 0.125).abs() < 1e-4 + 3.0 * r.gamma2_se(),
        "{r:?}"
    );
    // Every other face wound the other way: the transport reflects to the side a ray came from, so
    // it traces the same room; the triangles' own arithmetic differs in its last bits, so the
    // paths part after a while and agree within their errors, not to the bit.
    let flipped: Vec<[V3; 3]> = tris
        .iter()
        .enumerate()
        .map(|(i, &[a, b, c])| if i % 2 == 0 { [a, b, c] } else { [a, c, b] })
        .collect();
    let q = study(
        &Enclosure::from_mesh(&flipped, &fan(&tris)).unwrap(),
        &settings,
    );
    assert!(
        (q.gamma2 - p.gamma2).abs() < 3.0 * q.gamma2_se.hypot(p.gamma2_se),
        "{q:?}"
    );
    assert!(q.mean_free_path_off_se().abs() < 3.0);
    // Says no: the sphere's 1/8 is far from any box's, and far from what the coarsest sphere of
    // 80 faces gives.
    let coarse = icosphere(1);
    let c = study(
        &Enclosure::from_mesh(&coarse, &fan(&coarse)).unwrap(),
        &settings,
    );
    assert!((c.gamma2 - 0.125).abs() > 0.01, "{c:?}");
}

#[test]
fn successive_free_paths_are_correlated_in_a_box_and_independent_in_a_sphere() {
    // Kuttruff's formula takes the number of reflections in a time to have the variance `γ²` times
    // its mean, which holds for independent free paths. In a box a long path tends to end where
    // the next is long too; in a sphere every chord is 2R·cos θ with a fresh θ, independent of
    // the last. Counted over 256 paths a ray, the variance of a ray's length over 256·⟨ℓ⟩²
    // (`effective_gamma2`) is γ² for independent paths and larger for positively correlated ones.
    let settings = FreePathSettings {
        rays_per_replica: 1024,
        paths_per_ray: 256,
        ..FreePathSettings::STANDARD
    };
    for size in [[6.0, 10.0, 3.0], [5.0, 4.0, 3.0]] {
        let p = study(&Enclosure::shoebox(size).unwrap(), &settings);
        println!(
            "{size:?}: gamma^2 {:.4} ± {:.4}; lag-1 correlation {:.4} ± {:.4}; effective gamma^2 \
             over 256 paths {:.4} ± {:.4} ({:+.1} % of gamma^2)",
            p.gamma2,
            p.gamma2_se,
            p.lag1_correlation,
            p.lag1_correlation_se,
            p.effective_gamma2,
            p.effective_gamma2_se,
            100.0 * (p.effective_gamma2 / p.gamma2 - 1.0)
        );
        assert!(
            p.lag1_correlation > 0.03 && p.lag1_correlation > 10.0 * p.lag1_correlation_se,
            "{p:?}"
        );
        assert!(
            p.effective_gamma2 - p.gamma2 > 0.1 * p.gamma2
                && p.effective_gamma2 - p.gamma2 > 5.0 * p.effective_gamma2_se,
            "{p:?}"
        );
    }
    // Says no: in the sphere the same estimators read no correlation and no excess.
    let tris = icosphere(4);
    let p = study(
        &Enclosure::from_mesh(&tris, &fan(&tris)).unwrap(),
        &settings,
    );
    println!(
        "sphere, {} faces: gamma^2 {:.4} ± {:.4}; lag-1 correlation {:.4} ± {:.4}; effective \
         gamma^2 {:.4} ± {:.4}",
        tris.len(),
        p.gamma2,
        p.gamma2_se,
        p.lag1_correlation,
        p.lag1_correlation_se,
        p.effective_gamma2,
        p.effective_gamma2_se
    );
    assert!(
        p.lag1_correlation.abs() < 3.0 * p.lag1_correlation_se + 0.002,
        "{p:?}"
    );
    assert!(
        (p.effective_gamma2 - p.gamma2).abs() < 3.0 * p.effective_gamma2_se + 0.002,
        "{p:?}"
    );
}

/// Kuhn's six tetrahedra of the box `lo..hi`, built here apart from `params::lambert`'s own.
fn cells(lo: V3, hi: V3) -> Vec<[V3; 4]> {
    let d = [hi[0] - lo[0], hi[1] - lo[1], hi[2] - lo[2]];
    let mut out = Vec::new();
    for order in [
        [0, 1, 2],
        [0, 2, 1],
        [1, 0, 2],
        [1, 2, 0],
        [2, 0, 1],
        [2, 1, 0],
    ] {
        let mut p = lo;
        let mut t = [lo; 4];
        for (n, &k) in order.iter().enumerate() {
            p[k] += d[k];
            t[n + 1] = p;
        }
        out.push(t);
    }
    out
}

/// An L-shaped room, `[0, 6]×[0, 4] ∪ [0, 2]×[4, 8]`, 3 m high: not convex. Its surface and
/// the tetrahedra of its two boxes.
fn l_room() -> (Vec<[V3; 3]>, Vec<[V3; 4]>) {
    let outline: [[f64; 2]; 6] = [
        [0.0, 0.0],
        [6.0, 0.0],
        [6.0, 4.0],
        [2.0, 4.0],
        [2.0, 8.0],
        [0.0, 8.0],
    ];
    let h = 3.0;
    let mut tris = Vec::new();
    // The floor and the ceiling: two rectangles each.
    for z in [0.0, h] {
        for [x0, y0, x1, y1] in [[0.0, 0.0, 6.0, 4.0], [0.0, 4.0, 2.0, 8.0]] {
            let (a, b, c, d) = ([x0, y0, z], [x1, y0, z], [x1, y1, z], [x0, y1, z]);
            tris.push([a, b, c]);
            tris.push([a, c, d]);
        }
    }
    // The walls, one rectangle an edge of the outline.
    for i in 0..6 {
        let [p, q] = [outline[i], outline[(i + 1) % 6]];
        let (a, b, c, d) = (
            [p[0], p[1], 0.0],
            [q[0], q[1], 0.0],
            [q[0], q[1], h],
            [p[0], p[1], h],
        );
        tris.push([a, b, c]);
        tris.push([a, c, d]);
    }
    let mut tets = cells([0.0; 3], [6.0, 4.0, h]);
    tets.extend(cells([0.0, 4.0, 0.0], [2.0, 8.0, h]));
    (tris, tets)
}

#[test]
fn a_room_that_is_not_convex_still_gives_4v_over_s() {
    let (tris, tets) = l_room();
    let room = Enclosure::from_mesh(&tris, &tets).unwrap();
    assert!((room.area_m2() - 148.0).abs() < 1e-9);
    assert!((room.volume_m3() - 96.0).abs() < 1e-9);
    let p = study(&room, &FINE);
    println!(
        "L room: mean free path {:.5} ± {:.5} m against 4V/S {:.5} m; gamma^2 {:.4} ± {:.4}",
        p.mean_free_path_m,
        p.mean_free_path_se_m,
        room.four_v_over_s_m(),
        p.gamma2,
        p.gamma2_se
    );
    assert!(p.mean_free_path_off_se().abs() < 3.0);
    // What core::results would give it: accepted, and γ² the finer run's within its error.
    let r = free_paths(&room).unwrap();
    println!(
        "L room, free_paths: gamma^2 {:.4} ± {:.4}",
        r.gamma2(),
        r.gamma2_se()
    );
    assert!((r.gamma2() - p.gamma2).abs() < 3.0 * r.gamma2_se().hypot(p.gamma2_se));
    // The partner: rays started in the narrow wing only (its tetrahedra alone, which also make the
    // volume a quarter of the room's) are refused, as a room whose volume is not the surface's.
    let wing = cells([0.0, 4.0, 0.0], [2.0, 8.0, 3.0]);
    assert_refused(
        free_paths(&Enclosure::from_mesh(&tris, &wing).unwrap()),
        "4V/S",
    );
}

#[test]
fn a_wrong_mean_free_path_is_refused() {
    let size = [6.0, 10.0, 3.0];
    let room = Enclosure::shoebox(size).unwrap();
    assert!(free_paths(&room).is_ok());
    // Reflection that is not Lambert's, through the code: uniform over the hemisphere keeps no
    // diffuse field, and the mean free path falls to 2.97 m against 4V/S's 3.33 m (measured with
    // six seeds, 115 to 175 standard errors).
    assert_refused(
        faults::with(Fault::LambertUniformReflection, || free_paths(&room)),
        "4V/S",
    );
    let tris = box_triangles(size);
    let tets = cells([0.0; 3], size);
    // The same triangles filled by their own box are accepted: the refusals below are the
    // inputs'.
    assert!(free_paths(&Enclosure::from_mesh(&tris, &tets).unwrap()).is_ok());
    // A volume that is not the surface's: 1 % too small (the tetrahedra of a box 1 % lower), and
    // 1 % too large (a tetrahedron of 1.8 m³ inside it counted twice).
    let low = cells([0.0; 3], [6.0, 10.0, 2.97]);
    assert_refused(
        free_paths(&Enclosure::from_mesh(&tris, &low).unwrap()),
        "4V/S",
    );
    let mut twice = tets.clone();
    twice.push([
        [1.0, 1.0, 1.0],
        [2.2, 1.0, 1.0],
        [1.0, 7.0, 1.0],
        [1.0, 1.0, 2.5],
    ]);
    let e = Enclosure::from_mesh(&tris, &twice).unwrap();
    assert!((e.volume_m3() / 180.0 - 1.01).abs() < 1e-9);
    assert_refused(free_paths(&e), "4V/S");
    // A surface that is not closed: one triangle of the floor missing.
    let open: Vec<[V3; 3]> = tris
        .iter()
        .enumerate()
        .filter(|(i, _)| *i != 8)
        .map(|(_, t)| *t)
        .collect();
    assert_refused(
        free_paths(&Enclosure::from_mesh(&open, &tets).unwrap()),
        "left the enclosure",
    );
    // Rays started outside the room: its tetrahedra moved below the floor.
    let below = cells([0.0, 0.0, -4.0], [6.0, 10.0, -1.0]);
    assert_refused(
        free_paths(&Enclosure::from_mesh(&tris, &below).unwrap()),
        "left the enclosure",
    );
}

/// A box as twelve triangles, built here apart from `Enclosure::shoebox`.
fn box_triangles([x, y, z]: V3) -> Vec<[V3; 3]> {
    let p = |i: usize| {
        [
            if i & 1 != 0 { x } else { 0.0 },
            if i & 2 != 0 { y } else { 0.0 },
            if i & 4 != 0 { z } else { 0.0 },
        ]
    };
    [
        [0, 2, 6, 4],
        [1, 3, 7, 5],
        [0, 1, 5, 4],
        [2, 3, 7, 6],
        [0, 1, 3, 2],
        [4, 5, 7, 6],
    ]
    .iter()
    .flat_map(|s| [[p(s[0]), p(s[1]), p(s[2])], [p(s[0]), p(s[2]), p(s[3])]])
    .collect()
}

#[test]
fn bad_inputs_are_refused_and_runs_are_deterministic() {
    let tris = box_triangles([6.0, 10.0, 3.0]);
    let tets = cells([0.0; 3], [6.0, 10.0, 3.0]);
    let refused = |r: Result<Enclosure, simpa_core::params::ParamError>| {
        assert_eq!(r.unwrap_err().code(), codes::TRANSPORT_REFUSED);
    };
    refused(Enclosure::shoebox([6.0, 0.0, 3.0]));
    refused(Enclosure::shoebox([6.0, f64::NAN, 3.0]));
    refused(Enclosure::from_mesh(&[], &tets));
    refused(Enclosure::from_mesh(&tris, &[]));
    refused(Enclosure::from_mesh(&tris, &[[[1.0; 3]; 4]]));
    let mut bad = tris.clone();
    bad[3][1][2] = f64::INFINITY;
    refused(Enclosure::from_mesh(&bad, &tets));
    let mut bad = tets.clone();
    bad[2][3][0] = f64::NAN;
    refused(Enclosure::from_mesh(&tris, &bad));
    let room = Enclosure::shoebox([6.0, 10.0, 3.0]).unwrap();
    // A study's settings: too few replicas, rays or paths, or a start point not finite.
    for (s, start) in [
        (
            FreePathSettings {
                replicas: 1,
                ..FreePathSettings::STANDARD
            },
            None,
        ),
        (
            FreePathSettings {
                rays_per_replica: 0,
                ..FreePathSettings::STANDARD
            },
            None,
        ),
        (
            FreePathSettings {
                paths_per_ray: 1,
                ..FreePathSettings::STANDARD
            },
            None,
        ),
        (FreePathSettings::STANDARD, Some([1.0, f64::NAN, 1.0])),
    ] {
        let e = free_path_study(&room, &s, start).unwrap_err();
        assert_eq!(e.code(), codes::TRANSPORT_REFUSED, "{e}");
        assert!(
            e.to_string().contains("at least 2 replicas") || e.to_string().contains("not finite"),
            "{e}"
        );
    }
    // A study's rays started outside the room leave it.
    let e = free_path_study(&room, &FreePathSettings::STANDARD, Some([3.0, 5.0, 4.0])).unwrap_err();
    assert!(e.to_string().contains("left the enclosure"), "{e}");
    // The same room gives the same numbers to the bit, whatever the threads did; another seed, in
    // a study, others within their errors.
    let a = free_paths(&room).unwrap();
    let b = free_paths(&room).unwrap();
    assert_eq!(a, b);
    let c = study(
        &room,
        &FreePathSettings {
            seed: 99,
            ..FreePathSettings::STANDARD
        },
    );
    assert_ne!(a.gamma2(), c.gamma2);
    assert!((a.gamma2() - c.gamma2).abs() < 4.0 * a.gamma2_se().hypot(c.gamma2_se));
}

/// `p` turned by `angle` about the vertical through `(3, 5, 0)` and moved by `shift`.
fn turned(p: V3, angle: f64, shift: V3) -> V3 {
    let (s, c) = angle.sin_cos();
    let (x, y) = (p[0] - 3.0, p[1] - 5.0);
    [
        3.0 + c * x - s * y + shift[0],
        5.0 + s * x + c * y + shift[1],
        p[2] + shift[2],
    ]
}

#[test]
fn gamma2_cannot_be_fitted_through_what_reaches_kuttruff() {
    // The pre-M8 review fished for γ² by trying seeds and ray counts until the transport gave a
    // wanted value, and found 0.10 to 0.60 for the 6×10×3 m room (exact 0.3889). Reproduced here
    // through the test-only study, which still takes settings: the search has power.
    let room = Enclosure::shoebox([6.0, 10.0, 3.0]).unwrap();
    let exact = box_gamma2([6.0, 10.0, 3.0]);
    let tiny = FreePathSettings {
        replicas: 2,
        rays_per_replica: 1,
        burn_in_paths: 0,
        paths_per_ray: 3,
        seed: 0,
    };
    let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
    for seed in 0..2000 {
        let g = study(&room, &FreePathSettings { seed, ..tiny }).gamma2;
        lo = lo.min(g);
        hi = hi.max(g);
    }
    println!("a seed search at 2 x 1 rays of 3 paths: gamma^2 from {lo:.3} to {hi:.3}");
    assert!(lo < 0.2 && hi > 0.55, "{lo} {hi}");
    // What reaches kuttruff_rt is free_paths, which takes the room alone. The only freedom left is
    // how the same room is described: turned and moved, each description draws the transport's
    // randomness afresh. Over 32 descriptions γ² stays within 4.5 standard errors of the exact
    // value, and Kuttruff's time at α 0.4 within 0.15 %: a search can pick among the room's own
    // statistical scatter and nothing more.
    let walls = |e: &Enclosure| {
        [Surface {
            area_m2: e.area_m2(),
            absorption: 0.4,
        }]
    };
    let k = RtConstant::Physical {
        speed_of_sound: 343.2,
    };
    let tris = box_triangles([6.0, 10.0, 3.0]);
    let tets = cells([0.0; 3], [6.0, 10.0, 3.0]);
    let (mut values, mut times) = (Vec::new(), Vec::new());
    for i in 0..32 {
        let angle = 0.173 * i as f64;
        let shift = [0.37 * i as f64, -0.11 * i as f64, 0.013 * i as f64];
        let t: Vec<[V3; 3]> = tris
            .iter()
            .map(|f| f.map(|p| turned(p, angle, shift)))
            .collect();
        let c: Vec<[V3; 4]> = tets
            .iter()
            .map(|f| f.map(|p| turned(p, angle, shift)))
            .collect();
        let e = Enclosure::from_mesh(&t, &c).unwrap();
        let p = free_paths(&e).unwrap();
        assert!(p.gamma2_se() <= GAMMA2_SE_LIMIT);
        assert!(
            (p.gamma2() - exact).abs() < 4.5 * p.gamma2_se(),
            "description {i}: {p:?}"
        );
        values.push(p.gamma2());
        times.push(kuttruff_rt(&p, &walls(&e), None, k).unwrap());
    }
    let spread = |v: &[f64]| {
        v.iter().cloned().fold(f64::NEG_INFINITY, f64::max)
            - v.iter().cloned().fold(f64::INFINITY, f64::min)
    };
    let t_mean = times.iter().sum::<f64>() / times.len() as f64;
    println!(
        "32 descriptions of the room through free_paths: gamma^2 from {:.5} to {:.5}; Kuttruff at \
         alpha 0.4 spread {:.3} %",
        values.iter().cloned().fold(f64::INFINITY, f64::min),
        values.iter().cloned().fold(f64::NEG_INFINITY, f64::max),
        100.0 * spread(&times) / t_mean
    );
    assert!(spread(&times) / t_mean < 0.0015, "{times:?}");
    // The descriptions are real redraws, not one value 32 times.
    let mut distinct = values.clone();
    distinct.sort_by(f64::total_cmp);
    distinct.dedup();
    assert!(distinct.len() >= 30, "{values:?}");
}
