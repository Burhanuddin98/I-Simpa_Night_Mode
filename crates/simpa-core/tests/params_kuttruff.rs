//! Kuttruff's corrected Eyring (`params::room::kuttruff_rt`) with `γ²` from the geometry, M8's
//! reference (Burhan, 2026-09-24 23:14), against its closed form and against the diffuse
//! transport's decay in M8's cells (`docs/params.md`, "Kuttruff's reference"). Each check has its
//! say-NO partner, through the code (`simpa_core::faults`) or the input.
//!
//! **What "the transport's T30" is here**: the T30 of the energy the transport's receiver balls
//! collect (radius 0.31 m, SPPS's), read through `params` from the arrival, at M8's source and
//! three receivers per room and `dt` 1 ms, averaged over the three receivers in each of 16
//! replicas; the standard error is that of the 16 replica means. It is what M8 compares SPPS's
//! T30 with. And the T30 of the room's total energy, which in M8's rooms decays alike (the
//! paired difference within 1.4 standard errors, −0.032 % to +0.017 %, in every cell at high
//! counts) and is several times less noisy for the same rays; in the elongated room of the
//! evidence run it does not (up to 0.8 % apart, 31 standard errors).
//!
//! **Two questions, kept apart.** Kuttruff's *formula* against the transport is measured with the
//! box's exact `γ²` (`lambert_exact`), so that only the transport's noise is in the comparison.
//! The *reference as shipped*, `kuttruff_rt` with `free_paths`' `γ²`, is held to the same 0.6 %,
//! and to the formula within the standard deviation it reports (`kuttruff_rt_sd`).
//!
//! **The bare 0.6 % in the suite.** A debug build cannot trace enough rays to judge 0.6 % at the
//! point estimate (the worst cell is 0.013 points inside it). So the transport's T30 at high
//! counts is committed here ([`HIGH`]), from the release run
//! `kuttruff_against_the_transport_at_high_counts`, which re-derives every value and requires it
//! equal. The suite then (a) requires its own
//! transport, at its lower counts, to reproduce those values within its noise, so they are this
//! code's and not a number typed in; and (b) holds the formula and the shipped reference to the
//! bare 0.6 % of them, which is arithmetic, not a draw.

mod lambert_exact;

use std::f64::consts::LN_10;

use lambert_exact::box_gamma2;
use simpa_core::faults::{self, Fault};
use simpa_core::params::air::{Atmosphere, solver_air_absorption_per_m};
use simpa_core::params::codes;
use simpa_core::params::decay::Arrival;
use simpa_core::params::lambert::{
    DecaySettings, Enclosure, FreePathSettings, decay, free_path_study, free_paths,
};
use simpa_core::params::room::{
    RtConstant, Surface, eyring_rt, kuttruff_absorption_area, kuttruff_rt, kuttruff_rt_sd,
};

/// SPPS's speed of sound at 20 °C (`Celerite_du_son.cpp:46`).
const C: f64 = 343.2;
const K: RtConstant = RtConstant::Physical { speed_of_sound: C };
const RADIUS: f64 = 0.31;

/// One room with its source and three receivers.
struct Room {
    name: &'static str,
    size: [f64; 3],
    source: [f64; 3],
    receivers: [[f64; 3]; 3],
}

/// M8's rooms (`crates/simpa/tests/m8_evidence.rs`).
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

/// One of M8's cells at high counts: the room (an index into [`ROOMS`]), `α`, the receivers' T30
/// and its standard error, and the room energy's and its, s.
struct HighCell {
    room: usize,
    alpha: f64,
    t: f64,
    se: f64,
    room_t: f64,
    room_se: f64,
}

/// The transport's T30 in M8's eight cells at 16 times the suite's rays (4 M to 34 M a cell),
/// from `kuttruff_against_the_transport_at_high_counts` in a release build, which re-derives each
/// and requires it equal (to 10⁻⁹): the transport and its settings are deterministic. Regenerate
/// from that test's `HighCell` lines only after a reviewed change to the transport or to
/// `params::decay`.
const HIGH: [HighCell; 8] = [
    HighCell {
        room: 0,
        alpha: 0.05,
        t: 2.647681680,
        se: 0.000411370,
        room_t: 2.647489334,
        room_se: 0.000061000,
    },
    HighCell {
        room: 0,
        alpha: 0.1,
        t: 1.304672095,
        se: 0.000242639,
        room_t: 1.304451001,
        room_se: 0.000035608,
    },
    HighCell {
        room: 0,
        alpha: 0.2,
        t: 0.631223921,
        se: 0.000109630,
        room_t: 0.631200878,
        room_se: 0.000023486,
    },
    HighCell {
        room: 0,
        alpha: 0.4,
        t: 0.290834217,
        se: 0.000068707,
        room_t: 0.290927533,
        room_se: 0.000011009,
    },
    HighCell {
        room: 1,
        alpha: 0.05,
        t: 2.025626686,
        se: 0.000196265,
        room_t: 2.025813879,
        room_se: 0.000050207,
    },
    HighCell {
        room: 1,
        alpha: 0.1,
        t: 0.996933379,
        se: 0.000121952,
        room_t: 0.996993117,
        room_se: 0.000030028,
    },
    HighCell {
        room: 1,
        alpha: 0.2,
        t: 0.481140657,
        se: 0.000059798,
        room_t: 0.481062394,
        room_se: 0.000016347,
    },
    HighCell {
        room: 1,
        alpha: 0.4,
        t: 0.219812161,
        se: 0.000026423,
        room_t: 0.219815591,
        room_se: 0.000004383,
    },
];

/// The limit Kuttruff is held to against the transport in M8's cells (spec, pre-M8 piece A).
const LIMIT: f64 = 0.006;

/// `∂T/T` per unit `γ²` of Kuttruff's time with air off: `−ln(1 − α)/(2·(1 + (γ²/2)·ln(1 − α)))`.
fn relative_slope(alpha: f64, gamma2: f64) -> f64 {
    let ln = (1.0 - alpha).ln();
    -ln / (2.0 * (1.0 + 0.5 * gamma2 * ln))
}

/// Every wall of `e` at `alpha`, as one surface.
fn walls(e: &Enclosure, alpha: f64) -> [Surface; 1] {
    [Surface {
        area_m2: e.area_m2(),
        absorption: alpha,
    }]
}

/// Kuttruff's formula `K·V/(4·m·V − S·ln(1 − α)·[1 + (γ²/2)·ln(1 − α)])` with a given `γ²`, for
/// the exact `γ²` of a box. `kuttruff_rt` is this formula to 10⁻¹²
/// (`kuttruff_is_its_closed_form_and_refuses_what_it_does_not_describe`).
fn kuttruff_formula(e: &Enclosure, alpha: f64, gamma2: f64, air: Option<f64>) -> f64 {
    let (v, s) = (e.volume_m3(), e.area_m2());
    let ln = (1.0 - alpha).ln();
    K.value() * v / (4.0 * air.unwrap_or(0.0) * v - s * ln * (1.0 + 0.5 * gamma2 * ln))
}

/// The transport's T30s in one cell, s.
struct Transport {
    /// The receivers': the mean over replicas of the three receivers' mean, and its standard
    /// error.
    t: f64,
    se: f64,
    /// The room energy's, and its standard error over the replicas.
    room: f64,
    room_se: f64,
    /// The standard error of the receivers' T30 minus the room energy's, over the replicas'
    /// paired differences: both come from the same rays, so this is well below either's
    /// standard errors combined as if independent.
    diff_se: f64,
}

/// Mean and standard error of the mean.
fn mean_se(v: &[f64]) -> (f64, f64) {
    let n = v.len() as f64;
    let m = v.iter().sum::<f64>() / n;
    let var = v.iter().map(|x| (x - m).powi(2)).sum::<f64>() / (n - 1.0);
    (m, (var / n).sqrt())
}

fn transport_t30(room: &Room, e: &Enclosure, alpha: f64, air: Option<f64>, rays: u32) -> Transport {
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
            // 1.4 of Eyring's time: 84 dB of Eyring's decay, 76 dB of the slowest cell's (α 0.4,
            // 11 % longer), past the −35 dB T30 needs.
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
    let (t, se) = mean_se(&means);
    let room_t30 = d.room_t30().unwrap();
    let diffs: Vec<f64> = means
        .iter()
        .zip(&room_t30.values)
        .map(|(a, b)| a - b)
        .collect();
    Transport {
        t,
        se,
        room: room_t30.mean,
        room_se: room_t30.se,
        diff_se: mean_se(&diffs).1,
    }
}

#[test]
fn kuttruff_is_its_closed_form_and_refuses_what_it_does_not_describe() {
    let e = Enclosure::shoebox([6.0, 10.0, 3.0]).unwrap();
    let p = free_paths(&e).unwrap();
    let (v, s, g) = (e.volume_m3(), e.area_m2(), p.gamma2());
    let k = 24.0 * LN_10 / C;
    for alpha in [0.05, 0.2, 0.4, 0.9] {
        let ln = (1.0f64 - alpha).ln();
        let a = -s * ln * (1.0 + 0.5 * g * ln);
        let w = walls(&e, alpha);
        assert!((kuttruff_absorption_area(&p, &w).unwrap() - a).abs() < 1e-10 * a);
        assert!((kuttruff_rt(&p, &w, None, K).unwrap() - k * v / a).abs() < 1e-12);
        assert!(
            (kuttruff_rt(&p, &w, None, K).unwrap() - kuttruff_formula(&e, alpha, g, None)).abs()
                < 1e-12
        );
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

/// `|T_K/T − 1| ≤ 0.6 % + 3·SE`, with the comparison's relative standard error `rel_se`.
fn within(t_k: f64, t: f64, rel_se: f64) -> bool {
    (t_k / t - 1.0).abs() <= LIMIT + 3.0 * rel_se
}

#[test]
fn kuttruff_reproduces_the_transports_t30_in_m8s_cells() {
    // At the suite's counts (0.26 M to 2.1 M rays a cell, a debug build) the receivers' T30
    // carries up to about 0.1 % of noise and the room energy's under 0.02 %: too much to judge a
    // cell 0.013 points inside 0.6 % by. So, in every cell:
    // (a) the suite's transport reproduces the committed high-count T30s (`HIGH`) within 4 of
    //     their combined standard errors (4, not 3: sixteen comparisons, so that a correct
    //     transport fails one with a chance of about 0.1 %, not 4 %);
    // (b) Kuttruff's formula with the exact γ² is within the bare 0.6 % of both high-count T30s;
    // (c) the reference as shipped, kuttruff_rt with free_paths' γ², is too, and is the formula
    //     within 3 of the standard deviations it reports;
    // (d) the fixed settings meet their precision target: 3 of those deviations fit between the
    //     formula's error and 0.6 % (`FreePathSettings::STANDARD`).
    let mut rows = Vec::new();
    let (mut eyring_fails, mut fault_fails) = (0, 0);
    for (ri, room) in ROOMS.iter().enumerate() {
        let e = Enclosure::shoebox(room.size).unwrap();
        let p = free_paths(&e).unwrap();
        let exact = box_gamma2(room.size);
        for h in HIGH.iter().filter(|h| h.room == ri) {
            let alpha = h.alpha;
            // More rays where the decay is short and noisier, about the same work in every cell.
            let rays = (16384.0 * alpha / 0.05) as u32;
            let tr = transport_t30(room, &e, alpha, None, rays);
            let w = walls(&e, alpha);
            let t_x = kuttruff_formula(&e, alpha, exact, None);
            let t_k = kuttruff_rt(&p, &w, None, K).unwrap();
            let sd_k = kuttruff_rt_sd(&p, &w, None, K).unwrap();
            let t_e = eyring_rt(e.volume_m3(), &w, None, K).unwrap();
            let t_fault = faults::with(Fault::KuttruffFullVariance, || {
                kuttruff_rt(&p, &w, None, K).unwrap()
            });
            let (rel, rel_room) = (tr.se / tr.t, tr.room_se / tr.room);
            let off = (tr.t - h.t) / tr.se.hypot(h.se);
            let off_room = (tr.room - h.room_t) / tr.room_se.hypot(h.room_se);
            let (d_x, d_x_room) = (t_x / h.t - 1.0, t_x / h.room_t - 1.0);
            let (d_k, d_k_room) = (t_k / h.t - 1.0, t_k / h.room_t - 1.0);
            let margin = LIMIT - d_x.abs().max(d_x_room.abs());
            rows.push(format!(
                "{} α {alpha}: suite transport {:.5} ± {:.5} s ({off:+.2} SE from the high \
                 counts), room energy {:.5} ± {:.5} s ({off_room:+.2} SE); against the high \
                 counts: Kuttruff (exact γ²) {:+.3} % of the receivers, {:+.3} % of the room \
                 energy; as shipped {:+.3} % and {:+.3} %, 3·mc_sd {:.4} % against a margin of \
                 {:.4} %; Eyring {:+.2} %, Kuttruff without the ½ {:+.2} % of the room energy",
                room.name,
                tr.t,
                tr.se,
                tr.room,
                tr.room_se,
                100.0 * d_x,
                100.0 * d_x_room,
                100.0 * d_k,
                100.0 * d_k_room,
                300.0 * sd_k / t_k,
                100.0 * margin,
                100.0 * (t_e / h.room_t - 1.0),
                100.0 * (t_fault / h.room_t - 1.0),
            ));
            let row = rows.last().unwrap();
            println!("{row}");
            // (a), with errors small enough for it to catch a change of the transport that
            // matters: 4 SE is at most 0.08 % on the room energy.
            assert!(rel < 0.0012 && rel_room < 0.0002, "{row}");
            assert!(off.abs() < 4.0 && off_room.abs() < 4.0, "(a) {row}");
            // (b) and (c): the bare 0.6 %.
            assert!(d_x.abs() <= LIMIT && d_x_room.abs() <= LIMIT, "(b) {row}");
            assert!(d_k.abs() <= LIMIT && d_k_room.abs() <= LIMIT, "(c) {row}");
            assert!((t_k - t_x).abs() <= 3.0 * sd_k, "(c) {row}");
            // (d)
            assert!(3.0 * sd_k / t_k <= margin, "(d) {row}");
            eyring_fails += usize::from((t_e / h.room_t - 1.0).abs() > LIMIT);
            fault_fails += usize::from((t_fault / h.room_t - 1.0).abs() > LIMIT);
        }
    }
    // Says no to (b) and (c): plain Eyring misses in every cell, and Kuttruff's own formula with
    // its ½ dropped (a fault through the code, with free_paths' γ²) too.
    assert_eq!(eyring_fails, 8, "{}", rows.join("\n"));
    assert_eq!(fault_fails, 8, "{}", rows.join("\n"));

    // Says no to (a): the transport reflecting evenly over the hemisphere, not by Lambert's law
    // (a fault through the code), in the worst cell, is far from the committed values.
    let h = &HIGH[7];
    let room = &ROOMS[h.room];
    let e = Enclosure::shoebox(room.size).unwrap();
    let rays = (16384.0 * h.alpha / 0.05) as u32;
    let bad = faults::with(Fault::LambertUniformReflection, || {
        transport_t30(room, &e, h.alpha, None, rays)
    });
    let off_room = (bad.room - h.room_t) / bad.room_se.hypot(h.room_se);
    println!(
        "{} α {}, reflection uniform over the hemisphere: room energy {:.5} s, {off_room:+.1} SE \
         from the high counts",
        room.name, h.alpha, bad.room
    );
    assert!(off_room.abs() > 4.0, "{off_room}");

    // Says no to (d): the first fixed settings (16 × 1024 rays of 64 paths), with the standard
    // error they give in the worst cell, miss the target.
    let first = FreePathSettings {
        rays_per_replica: 1024,
        paths_per_ray: 64,
        ..FreePathSettings::STANDARD
    };
    let st = free_path_study(&e, &first, None).unwrap();
    let t_x = kuttruff_formula(&e, h.alpha, box_gamma2(room.size), None);
    let margin = LIMIT - (t_x / h.t - 1.0).abs().max((t_x / h.room_t - 1.0).abs());
    let three_sd = 3.0 * relative_slope(h.alpha, st.gamma2) * st.gamma2_se;
    println!(
        "the first settings: gamma^2 standard error {:.5}, 3·mc_sd {:.4} % against a margin of \
         {:.4} %",
        st.gamma2_se,
        100.0 * three_sd,
        100.0 * margin
    );
    assert!(three_sd > margin, "{three_sd} {margin}");
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
        let p = free_paths(&e).unwrap();
        let tr = transport_t30(room, &e, alpha, Some(m), 131_072);
        let w = walls(&e, alpha);
        let t_k = kuttruff_rt(&p, &w, Some(m), K).unwrap();
        let t_inside = faults::with(Fault::KuttruffAirInsideMean, || {
            kuttruff_rt(&p, &w, Some(m), K).unwrap()
        });
        let t_no_air = kuttruff_rt(&p, &w, None, K).unwrap();
        println!(
            "{} α {alpha}, 8 kHz (m {m:.5} /m, 4mV/A {:.2}): transport {:.5} ± {:.5} s; room \
             energy {:.5} ± {:.5} s; Kuttruff + 4mV {:+.3} % of the receivers; air folded into ᾱ \
             {:+.2} %; no air {:+.1} %",
            room.name,
            4.0 * m * e.volume_m3() / kuttruff_absorption_area(&p, &w).unwrap(),
            tr.t,
            tr.se,
            tr.room,
            tr.room_se,
            100.0 * (t_k / tr.t - 1.0),
            100.0 * (t_inside / tr.t - 1.0),
            100.0 * (t_no_air / tr.t - 1.0),
        );
        let rel = tr.se / tr.t;
        assert!(rel < 0.002);
        assert!(within(t_k, tr.t, rel), "{}", room.name);
        // Says no: the other form, through the code, misses.
        assert!(!within(t_inside, tr.t, rel), "{}", room.name);
    }
}

/// A room longer than M8's, for information only: how far Kuttruff's formula is from the
/// transport where the room is elongated (the M12 question; M8's gate does not use it).
const LONG_ROOM: Room = Room {
    name: "20x4x3",
    size: [20.0, 4.0, 3.0],
    source: [5.0, 2.1, 1.6],
    receivers: [[2.0, 1.0, 1.2], [11.0, 3.0, 1.8], [17.0, 2.0, 1.0]],
};

#[test]
#[ignore = "evidence for M8 and the source of HIGH, not a gate: M8's cells and an elongated room at \
            16 times the suite's rays, about five minutes in a release build; run on purpose with \
            --release"]
fn kuttruff_against_the_transport_at_high_counts() {
    // At 4 M to 34 M rays a cell, in every one of M8's cells:
    // - the T30s are `HIGH`'s to 10⁻⁹ s (the lines to regenerate it are printed);
    // - the receivers' and the room energy's T30 are one decay within the noise of their paired
    //   difference (4 SE, eight comparisons); the elongated room, where they are not, is this
    //   check's partner;
    // - Kuttruff's formula with the exact γ² is within the bare 0.6 % of both at the point
    //   estimate, and of the room energy also at one-sided 95 % (point + 1.645 SE); the
    //   receivers' bound is printed (their noise is six times the room energy's);
    // - the reference as shipped, with free_paths' γ², is within the bare 0.6 % of both;
    // - plain Eyring and the formula with its ½ dropped miss by more than their noise.
    // Then the elongated room, printed only.
    let mut worst: f64 = 0.0;
    let mut mismatches = Vec::new();
    let mut long_apart = 0;
    for (ri, room, gated) in [
        (0, &ROOMS[0], true),
        (1, &ROOMS[1], true),
        (2, &LONG_ROOM, false),
    ] {
        let e = Enclosure::shoebox(room.size).unwrap();
        let p = free_paths(&e).unwrap();
        let exact = box_gamma2(room.size);
        println!(
            "{}: exact gamma^2 {exact:.5}; free_paths {:.5} ± {:.5} ({:+.2} SE)",
            room.name,
            p.gamma2(),
            p.gamma2_se(),
            (p.gamma2() - exact) / p.gamma2_se()
        );
        for alpha in [0.05, 0.1, 0.2, 0.4] {
            let rays = (16.0 * 16384.0 * alpha / 0.05) as u32;
            let tr = transport_t30(room, &e, alpha, None, rays);
            let w = walls(&e, alpha);
            let t_x = kuttruff_formula(&e, alpha, exact, None);
            let t_k = kuttruff_rt(&p, &w, None, K).unwrap();
            let sd_k = kuttruff_rt_sd(&p, &w, None, K).unwrap();
            let t_e = eyring_rt(e.volume_m3(), &w, None, K).unwrap();
            let (d, d_room) = (t_x / tr.t - 1.0, t_x / tr.room - 1.0);
            let (d_k, d_k_room) = (t_k / tr.t - 1.0, t_k / tr.room - 1.0);
            let (rel, rel_room) = (tr.se / tr.t, tr.room_se / tr.room);
            let apart = (tr.t - tr.room) / tr.diff_se;
            println!(
                "{} α {alpha}: {} rays; transport T30 {:.5} ± {:.5} s ({:+.3} % vs Eyring), room \
                 energy {:.5} ± {:.5} s ({:+.3} %), receivers minus room energy {:+.3} % \
                 ({apart:+.2} SE, paired); Kuttruff (exact γ²) {:+.3} ± {:.3} % of the \
                 receivers (95 % bound {:.3} %), {:+.3} ± {:.4} % of the room energy (95 % bound \
                 {:.3} %); as shipped (free_paths' γ²) {:+.3} % of the receivers, {:+.3} % of the \
                 room energy, mc_sd {:.4} %",
                room.name,
                16 * rays,
                tr.t,
                tr.se,
                100.0 * (tr.t / t_e - 1.0),
                tr.room,
                tr.room_se,
                100.0 * (tr.room / t_e - 1.0),
                100.0 * (tr.t / tr.room - 1.0),
                100.0 * d,
                100.0 * rel,
                100.0 * (d.abs() + 1.645 * rel),
                100.0 * d_room,
                100.0 * rel_room,
                100.0 * (d_room.abs() + 1.645 * rel_room),
                100.0 * d_k,
                100.0 * d_k_room,
                100.0 * sd_k / t_k,
            );
            if !gated {
                long_apart += usize::from(apart.abs() > 4.0);
                continue;
            }
            println!(
                "    HighCell {{ room: {ri}, alpha: {alpha}, t: {:.9}, se: {:.9}, room_t: {:.9}, \
                 room_se: {:.9} }},",
                tr.t, tr.se, tr.room, tr.room_se
            );
            let h = HIGH
                .iter()
                .find(|h| h.room == ri && h.alpha == alpha)
                .unwrap();
            for (what, got, want) in [
                ("t", tr.t, h.t),
                ("se", tr.se, h.se),
                ("room_t", tr.room, h.room_t),
                ("room_se", tr.room_se, h.room_se),
            ] {
                if (got - want).abs() > 1e-9 {
                    mismatches.push(format!(
                        "{} α {alpha} {what}: {got} against {want}",
                        room.name
                    ));
                }
            }
            // Precise enough that the bare bound means something.
            assert!(
                rel < 0.0003 && rel_room < 0.00006,
                "{} α {alpha}",
                room.name
            );
            assert!(apart.abs() < 4.0, "{} α {alpha}: {apart}", room.name);
            assert!(d.abs() <= LIMIT, "{} α {alpha}: {d}", room.name);
            assert!(d_room.abs() <= LIMIT, "{} α {alpha}: {d_room}", room.name);
            assert!(
                d_room.abs() + 1.645 * rel_room <= LIMIT,
                "{} α {alpha}: {d_room}",
                room.name
            );
            assert!(
                d_k.abs() <= LIMIT && d_k_room.abs() <= LIMIT,
                "{} α {alpha}: as shipped {d_k} {d_k_room}",
                room.name
            );
            // Says no: plain Eyring, and the formula with its ½ dropped (through the code),
            // miss by more than their noise.
            let t_fault = faults::with(Fault::KuttruffFullVariance, || {
                kuttruff_rt(&p, &w, None, K).unwrap()
            });
            for (what, t) in [("Eyring", t_e), ("the ½ dropped", t_fault)] {
                assert!(
                    (t / tr.room - 1.0).abs() > LIMIT + 3.0 * rel_room.hypot(sd_k / t_k),
                    "{} α {alpha}: {what}",
                    room.name
                );
            }
            worst = worst.max(d.abs()).max(d_room.abs());
        }
    }
    println!("worst of M8's cells: {:.3} %", 100.0 * worst);
    // Says no to the receivers and the room energy being one decay: in the elongated room they
    // are apart.
    assert!(
        long_apart > 0,
        "the elongated room's receivers and room energy agree"
    );
    assert!(
        mismatches.is_empty(),
        "HIGH is not what this transport gives; after a reviewed change, regenerate it from the \
         HighCell lines above:\n{}",
        mismatches.join("\n")
    );
}
