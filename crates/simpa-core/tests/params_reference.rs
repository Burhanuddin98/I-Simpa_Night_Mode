//! The analytic reference M8 compares SPPS's T30 with, and what each open choice in it costs
//! against M8's 5 % budget (`docs/params.md`, "The reference M8 compares against"). Information
//! for Burhan's decision, computed on tutorial 1's box; the numbers the documentation quotes are
//! asserted here.
//!
//! Two choices:
//! - **The constant.** `T = K·V/(A + 4mV)`. TCR uses `K = 0.163` (`TC_CalculationCore.cpp:138`).
//!   A diffuse field whose energy falls as `exp(−c·(A + 4mV)·t/(4V))` has `K = 24·ln(10)/c`
//!   exactly; SPPS moves its particles at `c = 343.2·√(T/293.15)` (`Celerite_du_son.cpp:46`),
//!   343.2 m/s at 20 °C, so `K` = 0.16102 for SPPS's own physics.
//! - **The air term.** SPPS and TCR take `m` from upstream's formula at the nominal band
//!   frequency (`base_core_configuration.cpp:114`; `params::air::solver_air_absorption_per_m`);
//!   ISO 9613-1 tabulates α at the exact midband frequency (equation (6)).
//!
//! Says no: the same comparison at 80 kPa, where upstream's humidity lacks the pressure factor,
//! moves the high bands by far more than at sea level.

use simpa_core::params::air::{
    self, Atmosphere, energy_attenuation_per_m, exact_midband_hz, solver_air_absorption_per_m,
};
use simpa_core::params::room::{RtConstant, Surface, eyring_rt};

/// Tutorial 1's box: floor 60 m² at α 0.1, ceiling 60 m² at 0.3, walls 96 m² at 0.2; 180 m³.
const VOLUME: f64 = 180.0;
fn tutorial_surfaces() -> [Surface; 3] {
    [
        Surface {
            area_m2: 60.0,
            absorption: 0.1,
        },
        Surface {
            area_m2: 60.0,
            absorption: 0.3,
        },
        Surface {
            area_m2: 96.0,
            absorption: 0.2,
        },
    ]
}

/// The third-octave bands from 50 Hz to 8 kHz, M8's air-absorption range.
const BANDS: [f64; 23] = [
    50.0, 63.0, 80.0, 100.0, 125.0, 160.0, 200.0, 250.0, 315.0, 400.0, 500.0, 630.0, 800.0, 1000.0,
    1250.0, 1600.0, 2000.0, 2500.0, 3150.0, 4000.0, 5000.0, 6300.0, 8000.0,
];

/// Per band: T_Eyring with TCR's constant and the solver's air term (what TCR prints), and the
/// relative change from switching the constant to SPPS's physical one, the air term to ISO's
/// exact midband value, and both.
fn table(air: &Atmosphere) -> Vec<(f64, f64, f64, f64, f64)> {
    table_of(VOLUME, &tutorial_surfaces(), air)
}

/// [`table`] for any room.
fn table_of(volume: f64, s: &[Surface], air: &Atmosphere) -> Vec<(f64, f64, f64, f64, f64)> {
    let physical = RtConstant::Physical {
        speed_of_sound: f64::from(air::speed_of_sound(air.temperature_c) as f32),
    };
    BANDS
        .iter()
        .map(|&f| {
            let nominal = solver_air_absorption_per_m(f, air).unwrap();
            let exact = energy_attenuation_per_m(
                air::attenuation_db_per_m(exact_midband_hz(f).unwrap(), air).unwrap(),
            );
            let t = |m: f64, k: RtConstant| eyring_rt(volume, s, Some(m), k).unwrap();
            let tcr = t(nominal, RtConstant::Tcr);
            let rel = |x: f64| x / tcr - 1.0;
            (
                f,
                tcr,
                rel(t(nominal, physical)),
                rel(t(exact, RtConstant::Tcr)),
                rel(t(exact, physical)),
            )
        })
        .collect()
}

fn print(label: &str, rows: &[(f64, f64, f64, f64, f64)]) {
    println!(
        "{label}\n{:>6} | {:>8} | {:>12} | {:>12} | {:>12}",
        "band", "T (TCR)", "K physical", "m exact", "both"
    );
    for (f, t, k, m, both) in rows {
        println!(
            "{f:>6} | {t:>8.4} | {:>+11.3}% | {:>+11.3}% | {:>+11.3}%",
            100.0 * k,
            100.0 * m,
            100.0 * both
        );
    }
}

#[test]
fn what_the_constant_and_the_air_term_move_on_tutorial_1() {
    let sea = Atmosphere::at_reference_pressure(20.0, 50.0);
    let rows = table(&sea);
    print("tutorial 1, 20 °C, 50 %, 101.325 kPa", &rows);
    // The constant moves every band by the same share: 0.16102 against 0.163, 1.215 % shorter
    // (0.163 is 1.23 % above 0.16102).
    for (f, _, k, _, _) in &rows {
        assert!((k * 100.0 + 1.2147).abs() < 0.001, "{f}: {k}");
    }
    // The air term's form moves a band by at most 0.36 %, at 8 kHz, where air is a quarter of
    // the absorption: within a tenth of the 5 % budget in every band up to 8 kHz.
    let worst = rows.iter().map(|r| r.3.abs()).fold(0.0f64, f64::max);
    let at = rows.iter().find(|r| r.3.abs() == worst).unwrap().0;
    println!(
        "the air term's form: at most {:.3} % ({at} Hz)",
        100.0 * worst
    );
    assert!(worst < 0.005, "{worst}");
    assert!(worst > 0.001, "{worst}");
    // Both together stay within 1.6 % of TCR's value in every band.
    for (f, _, _, _, both) in &rows {
        assert!(both.abs() < 0.016, "{f}: {both}");
    }
}

#[test]
fn what_the_air_term_moves_in_m8s_rooms() {
    // M8's second table: its rooms with every surface at α and air on (20 °C, 50 %). The less the
    // surfaces absorb, the larger air's share at 8 kHz, and the more its form moves T_Eyring
    // (review: tutorial 1's 0.36 % is not M8's).
    let sea = Atmosphere::at_reference_pressure(20.0, 50.0);
    let mut worst_all = (0.0f64, "", 0.0, 0.0);
    for (name, [a, b, c]) in [("6x10x3", [6.0, 10.0, 3.0]), ("5x4x3", [5.0, 4.0, 3.0])] {
        let volume = a * b * c;
        let area = 2.0 * (a * b + b * c + a * c);
        for alpha in [0.05, 0.1, 0.2, 0.4] {
            let s = [Surface {
                area_m2: area,
                absorption: alpha,
            }];
            let rows = table_of(volume, &s, &sea);
            let (f, m) = rows
                .iter()
                .map(|r| (r.0, r.3))
                .max_by(|x, y| x.1.abs().total_cmp(&y.1.abs()))
                .unwrap();
            println!(
                "{name}, alpha {alpha}: the air term's form moves T_Eyring by at most {:+.3} % \
                 ({f} Hz)",
                100.0 * m
            );
            if m.abs() > worst_all.0.abs() {
                worst_all = (m, name, alpha, f);
            }
        }
    }
    println!(
        "worst: {:+.3} % ({} alpha {} at {} Hz)",
        100.0 * worst_all.0,
        worst_all.1,
        worst_all.2,
        worst_all.3
    );
    // Within a fifth of M8's 5 % budget everywhere, and larger than on tutorial 1.
    assert!(worst_all.0.abs() < 0.01, "{worst_all:?}");
    assert!(worst_all.0.abs() > 0.005, "{worst_all:?}");
}

#[test]
fn at_80_kpa_the_air_terms_form_moves_the_high_bands_far_more() {
    // Upstream's humidity without the pressure factor: the solver's m is off ISO's by up to
    // about 25 % in some band at 80 kPa (docs/params.md, "ISO 9613-1"), and T_Eyring with it.
    let high = Atmosphere {
        temperature_c: 20.0,
        relative_humidity_percent: 50.0,
        pressure_pa: 80_000.0,
    };
    let rows = table(&high);
    print("tutorial 1, 20 °C, 50 %, 80 kPa", &rows);
    let worst = rows.iter().map(|r| r.3.abs()).fold(0.0f64, f64::max);
    println!(
        "the air term's form at 80 kPa: at most {:.2} %",
        100.0 * worst
    );
    assert!(worst > 0.01, "{worst}");
}
