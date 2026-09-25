//! Kuttruff's corrected Eyring (`params::room::kuttruff_rt`) with `γ²` from the geometry, M8's
//! reference (Burhan, 2026-09-24 23:14), against its closed form and against the diffuse
//! transport's decay in M8's cells (`docs/params.md`, "Kuttruff's reference"). Each check has its
//! say-NO partner, through the code (`simpa_core::faults`) or the input.
//!
//! **What "the transport's T30" is here**: the T30 of the energy the transport's receiver balls
//! collect (radius 0.31 m, SPPS's), read through `params` from the arrival, at M8's source and
//! three receivers per room and `dt` 1 ms, averaged over the three receivers in each of 16
//! replicas; the standard error is that of the 16 replica means. It is what M8 compares SPPS's
//! T30 with. The room's total energy decays almost alike (printed): at 4 M to 34 M rays a cell
//! (`kuttruff_against_the_transport_at_high_counts`), within 0.04 % of the receivers in every
//! cell, and Kuttruff +0.59 % of both at α 0.4 in the 5×4×3 m room, its worst (`docs/params.md`).

use std::f64::consts::LN_10;

use simpa_core::faults::{self, Fault};
use simpa_core::params::air::{Atmosphere, solver_air_absorption_per_m};
use simpa_core::params::codes;
use simpa_core::params::decay::Arrival;
use simpa_core::params::lambert::{
    DecaySettings, Enclosure, FreePathSettings, FreePaths, decay, free_paths,
};
use simpa_core::params::room::{
    RtConstant, Surface, eyring_rt, kuttruff_absorption_area, kuttruff_rt, kuttruff_rt_sd,
};

/// SPPS's speed of sound at 20 °C (`Celerite_du_son.cpp:46`).
const C: f64 = 343.2;
const K: RtConstant = RtConstant::Physical { speed_of_sound: C };
const RADIUS: f64 = 0.31;

/// One of M8's rooms with its source and three receivers (`crates/simpa/tests/m8_evidence.rs`).
struct Room {
    name: &'static str,
    size: [f64; 3],
    source: [f64; 3],
    receivers: [[f64; 3]; 3],
}

const ROOMS: [Room; 2] = [
    Room {
        name: "6x10x3",
        size: [6.0, 10.0, 3.0],
        source: [3.0, 5.0, 1.8],
        receivers: [[1.0, 1.0, 1.8], [3.0, 7.0, 1.8], [5.0, 8.5, 1.2]],
    },
    Room {
        name: "5x4x3",
        size: [5.0, 4.0, 3.0],
        source: [2.52, 1.97, 1.53],
        receivers: [[1.0, 1.0, 1.0], [4.0, 3.0, 2.0], [1.0, 3.0, 1.9]],
    },
];

/// Every wall of `e` at `alpha`, as one surface.
fn walls(e: &Enclosure, alpha: f64) -> [Surface; 1] {
    [Surface {
        area_m2: e.area_m2(),
        absorption: alpha,
    }]
}

/// The transport's T30 in one cell: `(mean, standard error)` of the replicas' receiver means, s,
/// and the room energy's T30, s.
fn transport_t30(
    room: &Room,
    e: &Enclosure,
    alpha: f64,
    air: Option<f64>,
    rays: u32,
) -> (f64, f64, f64) {
    let t_eyring = eyring_rt(e.volume_m3(), &walls(e, alpha), air, K).unwrap();
    let d = decay(
        e,
        &vec![alpha; e.face_count()],
        &DecaySettings {
            source_m: room.source,
            receivers_m: room.receivers.to_vec(),
            receiver_radius_m: RADIUS,
            speed_of_sound_m_s: C,
            time_step_s: 0.001,
            // 76 dB of Eyring's decay, 68 of the slowest (α 0.4, +11 %).
            duration_s: 1.4 * t_eyring,
            air_m_per_metre: air,
            replicas: 16,
            rays_per_replica: rays,
            seed: 0x6d38_6365_6c6c_0000 + (alpha * 1000.0) as u64,
        },
    )
    .unwrap();
    let per_receiver: Vec<Vec<f64>> = room
        .receivers
        .iter()
        .enumerate()
        .map(|(i, q)| {
            let r = ((q[0] - room.source[0]).powi(2)
                + (q[1] - room.source[1]).powi(2)
                + (q[2] - room.source[2]).powi(2))
            .sqrt();
            d.receiver_t30(i, Arrival::spread(r / C, RADIUS / C))
                .unwrap_or_else(|e| panic!("{} α {alpha}, receiver {i}: {e}", room.name))
                .values
        })
        .collect();
    let means: Vec<f64> = (0..16)
        .map(|k| per_receiver.iter().map(|v| v[k]).sum::<f64>() / 3.0)
        .collect();
    let m = means.iter().sum::<f64>() / 16.0;
    let var = means.iter().map(|x| (x - m).powi(2)).sum::<f64>() / 15.0;
    (m, (var / 16.0).sqrt(), d.room_t30().unwrap().mean)
}

#[test]
fn kuttruff_is_its_closed_form_and_refuses_what_it_does_not_describe() {
    let e = Enclosure::shoebox([6.0, 10.0, 3.0]).unwrap();
    let p = free_paths(&e, &FreePathSettings::STANDARD).unwrap();
    let (v, s, g) = (e.volume_m3(), e.area_m2(), p.gamma2());
    let k = 24.0 * LN_10 / C;
    for alpha in [0.05, 0.2, 0.4, 0.9] {
        let ln = (1.0f64 - alpha).ln();
        let a = -s * ln * (1.0 + 0.5 * g * ln);
        let w = walls(&e, alpha);
        assert!((kuttruff_absorption_area(&p, &w).unwrap() - a).abs() < 1e-10 * a);
        assert!((kuttruff_rt(&p, &w, None, K).unwrap() - k * v / a).abs() < 1e-12);
        // The air term is added outside the wall term.
        let m = 0.01;
        assert!(
            (kuttruff_rt(&p, &w, Some(m), K).unwrap() - k * v / (4.0 * m * v + a)).abs() < 1e-12
        );
        // Longer than Eyring's whenever anything absorbs.
        assert!(kuttruff_rt(&p, &w, None, K).unwrap() > eyring_rt(v, &w, None, K).unwrap());
        // Its standard deviation is ∂T/∂γ² times γ²'s standard error: the derivative by hand.
        let t_of = |g: f64| k * v / (-s * ln * (1.0 + 0.5 * g * ln));
        let h = 1e-6;
        let slope = (t_of(g + h) - t_of(g - h)) / (2.0 * h);
        let sd = kuttruff_rt_sd(&p, &w, None, K).unwrap();
        assert!(
            (sd - slope.abs() * p.gamma2_se()).abs() < 1e-6 * sd,
            "{sd} {slope}"
        );
    }
    // Mixed walls: ᾱ is area-weighted before the logarithm, as Eyring's.
    let mixed = [
        Surface {
            area_m2: 60.0,
            absorption: 0.1,
        },
        Surface {
            area_m2: 156.0,
            absorption: 0.3,
        },
    ];
    let abar: f64 = (6.0 + 46.8) / 216.0;
    let ln = (1.0 - abar).ln();
    let a = -216.0 * ln * (1.0 + 0.5 * g * ln);
    assert!((kuttruff_absorption_area(&p, &mixed).unwrap() - a).abs() < 1e-10 * a);

    let bad = |r: Result<f64, simpa_core::params::ParamError>, field: &str| {
        let e = r.unwrap_err();
        assert_eq!(e.code(), codes::BAD_ROOM, "{e}");
        assert!(e.to_string().contains(field), "{e}");
    };
    // γ² of another room: the 5×4×3 m room's walls with the 6×10×3 m room's free paths.
    let other = Enclosure::shoebox([5.0, 4.0, 3.0]).unwrap();
    bad(
        kuttruff_rt(&p, &walls(&other, 0.2), None, K),
        "total_area_m2",
    );
    // Everything absorbed, and absorption past where the correction still grows with it:
    // 1 + γ²·ln(1 − ᾱ) is 0.10 at ᾱ 0.9 and −0.17 at 0.95 for γ² ≈ 0.389.
    bad(kuttruff_rt(&p, &walls(&e, 1.0), None, K), "mean_absorption");
    bad(
        kuttruff_rt(&p, &walls(&e, 0.95), None, K),
        "kuttruff_monotone",
    );
    assert!(kuttruff_rt(&p, &walls(&e, 0.9), None, K).is_ok());
    bad(kuttruff_rt(&p, &walls(&e, 1.2), None, K), "absorption");
    bad(
        kuttruff_rt(&p, &walls(&e, 0.2), Some(-1.0), K),
        "air_m_per_metre",
    );

    // The faults, through the code: the ½ dropped, and the air folded into ᾱ.
    let w = walls(&e, 0.4);
    let ln = 0.6f64.ln();
    let full = faults::with(Fault::KuttruffFullVariance, || {
        kuttruff_absorption_area(&p, &w).unwrap()
    });
    assert!((full - (-s * ln * (1.0 + g * ln))).abs() < 1e-10 * full);
    let m = 0.02;
    let inside = faults::with(Fault::KuttruffAirInsideMean, || {
        kuttruff_rt(&p, &w, Some(m), K).unwrap()
    });
    let ln_in = (1.0 - (0.4 * s + 4.0 * m * v) / s).ln();
    assert!((inside - k * v / (-s * ln_in * (1.0 + 0.5 * g * ln_in))).abs() < 1e-12);
    // Without air the second fault changes nothing.
    assert_eq!(
        faults::with(Fault::KuttruffAirInsideMean, || kuttruff_rt(
            &p, &w, None, K
        )),
        kuttruff_rt(&p, &w, None, K)
    );
}

/// `|T_K/T − 1| ≤ 0.6 % + 3·SE`, with `T`'s relative standard error `rel_se`.
fn within(t_k: f64, t: f64, rel_se: f64) -> bool {
    (t_k / t - 1.0).abs() <= 0.006 + 3.0 * rel_se
}

#[test]
fn kuttruff_reproduces_the_transports_t30_in_m8s_cells() {
    let mut rows = Vec::new();
    let (mut eyring_fails, mut fault_fails) = (0, 0);
    for room in &ROOMS {
        let e = Enclosure::shoebox(room.size).unwrap();
        let p: FreePaths = free_paths(&e, &FreePathSettings::STANDARD).unwrap();
        for alpha in [0.05, 0.1, 0.2, 0.4] {
            // More rays where the decay is short and noisier, about the same work in every cell.
            let rays = (16384.0 * alpha / 0.05) as u32;
            let (t, se, t_room) = transport_t30(room, &e, alpha, None, rays);
            let w = walls(&e, alpha);
            let t_k = kuttruff_rt(&p, &w, None, K).unwrap();
            let t_e = eyring_rt(e.volume_m3(), &w, None, K).unwrap();
            let t_fault = faults::with(Fault::KuttruffFullVariance, || {
                kuttruff_rt(&p, &w, None, K).unwrap()
            });
            let rel = se / t;
            rows.push(format!(
                "{} α {alpha}: transport {t:.5} ± {se:.5} s ({:+.2} % vs Eyring; room energy \
                 {:+.2} %); Kuttruff {:+.3} % of it; Eyring {:+.2} %; Kuttruff without the ½ \
                 {:+.2} %",
                room.name,
                100.0 * (t / t_e - 1.0),
                100.0 * (t_room / t_e - 1.0),
                100.0 * (t_k / t - 1.0),
                100.0 * (t_e / t - 1.0),
                100.0 * (t_fault / t - 1.0),
            ));
            // The error is small enough for the bound to mean something.
            assert!(rel < 0.002, "{}", rows.last().unwrap());
            assert!(within(t_k, t, rel), "{}", rows.last().unwrap());
            eyring_fails += usize::from(!within(t_e, t, rel));
            fault_fails += usize::from(!within(t_fault, t, rel));
        }
    }
    println!("{}", rows.join("\n"));
    // Says no: plain Eyring misses in every cell, and Kuttruff's own formula with its ½ dropped
    // (a fault through the code) misses in most.
    assert_eq!(eyring_fails, 8, "{}", rows.join("\n"));
    assert!(fault_fails >= 6, "{fault_fails}: {}", rows.join("\n"));
}

#[test]
fn the_air_term_is_added_outside_as_the_transport_decays() {
    // M8's second table: air on at 20 °C, 50 %, 101.325 kPa, at 8 kHz where it absorbs most, with
    // the solver's own m (nominal band frequency, upstream's form).
    let atmosphere = Atmosphere {
        temperature_c: 20.0,
        relative_humidity_percent: 50.0,
        pressure_pa: 101_325.0,
    };
    let m = solver_air_absorption_per_m(8000.0, &atmosphere).unwrap();
    for (room, alpha) in [(&ROOMS[0], 0.05), (&ROOMS[1], 0.1)] {
        let e = Enclosure::shoebox(room.size).unwrap();
        let p = free_paths(&e, &FreePathSettings::STANDARD).unwrap();
        let (t, se, _) = transport_t30(room, &e, alpha, Some(m), 131_072);
        let w = walls(&e, alpha);
        let t_k = kuttruff_rt(&p, &w, Some(m), K).unwrap();
        let t_inside = faults::with(Fault::KuttruffAirInsideMean, || {
            kuttruff_rt(&p, &w, Some(m), K).unwrap()
        });
        let t_no_air = kuttruff_rt(&p, &w, None, K).unwrap();
        println!(
            "{} α {alpha}, 8 kHz (m {m:.5} /m, 4mV/A {:.2}): transport {t:.5} ± {se:.5} s; \
             Kuttruff + 4mV {:+.3} %; air folded into ᾱ {:+.2} %; no air {:+.1} %",
            room.name,
            4.0 * m * e.volume_m3() / kuttruff_absorption_area(&p, &w).unwrap(),
            100.0 * (t_k / t - 1.0),
            100.0 * (t_inside / t - 1.0),
            100.0 * (t_no_air / t - 1.0),
        );
        let rel = se / t;
        assert!(rel < 0.002);
        assert!(within(t_k, t, rel), "{}", room.name);
        // Says no: the other form, through the code, misses.
        assert!(!within(t_inside, t, rel), "{}", room.name);
    }
}

#[test]
#[ignore = "evidence for M8, not a gate: M8's cells at 16 times the gate's rays, about two minutes \
            in a release build; run on purpose with --release"]
fn kuttruff_against_the_transport_at_high_counts() {
    for room in &ROOMS {
        let e = Enclosure::shoebox(room.size).unwrap();
        let p = free_paths(
            &e,
            &FreePathSettings {
                rays_per_replica: 16384,
                ..FreePathSettings::STANDARD
            },
        )
        .unwrap();
        for alpha in [0.05, 0.1, 0.2, 0.4] {
            let rays = (16.0 * 16384.0 * alpha / 0.05) as u32;
            let (t, se, t_room) = transport_t30(room, &e, alpha, None, rays);
            let w = walls(&e, alpha);
            let t_k = kuttruff_rt(&p, &w, None, K).unwrap();
            let t_e = eyring_rt(e.volume_m3(), &w, None, K).unwrap();
            println!(
                "{} α {alpha}: {} rays, gamma^2 {:.5} ± {:.5}; transport T30 {t:.5} ± {se:.5} s \
                 ({:+.3} % vs Eyring), room energy T30 {:+.3} %; Kuttruff {:+.3} % ± {:.3} of \
                 the transport, {:+.3} % of the room energy",
                room.name,
                16 * rays,
                p.gamma2(),
                p.gamma2_se(),
                100.0 * (t / t_e - 1.0),
                100.0 * (t_room / t_e - 1.0),
                100.0 * (t_k / t - 1.0),
                100.0 * se / t,
                100.0 * (t_k / t_room - 1.0),
            );
        }
    }
}
