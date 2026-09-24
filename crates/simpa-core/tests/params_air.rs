//! M7 gate (b): our ISO 9613-1 attenuation against tabulated values, within 1 % relative, at
//! 101.325 kPa (`docs/params.md`, "ISO 9613-1").
//!
//! **Where each number comes from.** Values in dB/km, one per one-third-octave band from 50 Hz
//! to 10 kHz, compared with our α at the band's exact midband frequency, as the standard computes
//! its table (ISO 9613-1:1993, clause 6.4, note 5, page 3).
//! - [`GOST_20C_50RH`]: **20 °C, 50 % RH**, the gate's condition. GOST 31295.1-2005 (ISO
//!   9613-1:1993, MOD), Table 1 (i), printed page 13 (PDF page 18 of
//!   `https://meganorm.ru/Data2/1/4293849/4293849418.pdf`, sha256 `03f11462d7aa3e48…`). This is the
//!   Russian adoption, **not ISO's own page**: ISO's 20 °C table is on a page the preview we could
//!   open does not include.
//! - [`ISO_15C_50RH`] and [`ISO_10C_50RH`]: ISO 9613-1:1993(E) itself, Table 1 (h) and (g), page 8,
//!   read in the iTeh preview (`https://cdn.standards.iteh.ai/samples/17426/3a2d69b767024b74805b83b063a91445/ISO-9613-1-1993.pdf`,
//!   sha256 `cb0e28c6a1dda1d3…`). The preview's watermark lies across the 15 °C column's rows 50 to
//!   200 Hz; they were read through it, and GOST's 15 °C column agrees.
//! - [`GOST_15C_50RH`]: GOST's Table 1 (h), printed page 12. Equal to ISO's page 8 in all 24 bands,
//!   the evidence that GOST reproduces ISO's table.

use simpa_core::params::air::{
    self, Atmosphere, attenuation_db_per_m, energy_attenuation_per_m, exact_midband_hz,
    molar_concentration_percent, solver_air_absorption_per_m, upstream_attenuation_db_per_m,
};
use simpa_core::params::codes;

#[allow(dead_code)]
#[path = "common/paths.rs"]
mod paths;

/// The preferred (nominal) frequencies of Table 1, Hz.
const NOMINAL: [f64; 24] = [
    50.0, 63.0, 80.0, 100.0, 125.0, 160.0, 200.0, 250.0, 315.0, 400.0, 500.0, 630.0, 800.0, 1000.0,
    1250.0, 1600.0, 2000.0, 2500.0, 3150.0, 4000.0, 5000.0, 6300.0, 8000.0, 10000.0,
];

/// GOST 31295.1-2005, Table 1 (i), 20 °C, column "50", printed page 13. dB/km.
const GOST_20C_50RH: [f64; 24] = [
    7.84e-2, 1.23e-1, 1.91e-1, 2.94e-1, 4.45e-1, 6.60e-1, 9.50e-1, 1.32, 1.75, 2.23, 2.73, 3.27,
    3.89, 4.66, 5.75, 7.37, 9.86, 13.7, 19.8, 29.4, 44.4, 67.8, 104.0, 159.0,
];

/// ISO 9613-1:1993, Table 1 (h), 15 °C, column "50", page 8. dB/km.
const ISO_15C_50RH: [f64; 24] = [
    9.14e-2, 1.42e-1, 2.17e-1, 3.26e-1, 4.79e-1, 6.81e-1, 9.30e-1, 1.22, 1.53, 1.87, 2.24, 2.69,
    3.29, 4.16, 5.49, 7.55, 10.8, 15.9, 23.8, 36.2, 55.4, 84.7, 129.0, 192.0,
];

/// GOST 31295.1-2005, Table 1 (h), 15 °C, column "50", printed page 12. dB/km.
const GOST_15C_50RH: [f64; 24] = [
    9.14e-2, 1.42e-1, 2.17e-1, 3.26e-1, 4.79e-1, 6.81e-1, 9.30e-1, 1.22, 1.53, 1.87, 2.24, 2.69,
    3.29, 4.16, 5.49, 7.55, 10.8, 15.9, 23.8, 36.2, 55.4, 84.7, 129.0, 192.0,
];

/// ISO 9613-1:1993, Table 1 (g), 10 °C, column "50", page 8. dB/km.
const ISO_10C_50RH: [f64; 24] = [
    1.05e-1, 1.60e-1, 2.39e-1, 3.47e-1, 4.86e-1, 6.53e-1, 8.43e-1, 1.05, 1.28, 1.55, 1.90, 2.39,
    3.13, 4.26, 6.04, 8.83, 13.2, 20.0, 30.6, 46.7, 70.8, 106.0, 155.0, 220.0,
];

/// Our α at each band's exact midband frequency, dB/km.
fn ours(temperature_c: f64, rh: f64) -> Vec<f64> {
    let air = Atmosphere::at_reference_pressure(temperature_c, rh);
    NOMINAL
        .iter()
        .map(|&n| 1000.0 * attenuation_db_per_m(exact_midband_hz(n).unwrap(), &air).unwrap())
        .collect()
}

/// `got / want − 1` per band.
fn relative(got: &[f64], want: &[f64]) -> Vec<f64> {
    got.iter().zip(want).map(|(g, w)| g / w - 1.0).collect()
}

/// The bands outside 1 %, as `(nominal Hz, relative error)`.
fn outside_one_percent(got: &[f64], want: &[f64]) -> Vec<(f64, f64)> {
    NOMINAL
        .iter()
        .zip(relative(got, want))
        .filter(|(_, r)| r.abs() > 0.01)
        .map(|(&n, r)| (n, r))
        .collect()
}

fn worst(r: &[f64]) -> f64 {
    r.iter().fold(0.0_f64, |m, x| m.max(x.abs()))
}

#[test]
fn the_gate_20c_50rh_matches_the_table_within_one_percent() {
    let got = ours(20.0, 50.0);
    let r = relative(&got, &GOST_20C_50RH);
    for ((n, g), (w, e)) in NOMINAL.iter().zip(&got).zip(GOST_20C_50RH.iter().zip(&r)) {
        println!(
            "{n:>6} Hz: ours {g:>10.4} dB/km, table {w:>8} dB/km, {:+.3} %",
            100.0 * e
        );
    }
    assert_eq!(outside_one_percent(&got, &GOST_20C_50RH), vec![]);
    // As documented: the worst band is within 0.29 %.
    assert!(worst(&r) < 0.0029, "worst {}", worst(&r));
}

#[test]
fn the_standards_own_pages_match_within_one_percent() {
    // The two transcriptions of the 15 °C column agree: GOST reproduces ISO's table.
    assert_eq!(ISO_15C_50RH, GOST_15C_50RH);
    for (t, table, bound) in [(15.0, &ISO_15C_50RH, 0.0034), (10.0, &ISO_10C_50RH, 0.0029)] {
        let got = ours(t, 50.0);
        assert_eq!(outside_one_percent(&got, table), vec![], "{t} °C");
        let w = worst(&relative(&got, table));
        println!("{t} °C, 50 %: worst band {:.3} %", 100.0 * w);
        assert!(w < bound, "{t} °C: worst {w}");
    }
}

#[test]
fn one_degree_or_five_percent_humidity_off_fails() {
    for (t, rh) in [(19.0, 50.0), (21.0, 50.0), (20.0, 45.0), (20.0, 55.0)] {
        let bad = outside_one_percent(&ours(t, rh), &GOST_20C_50RH);
        println!("{t} °C, {rh} %: {} of 24 bands outside 1 %", bad.len());
        assert!(bad.len() >= 20, "{t} °C, {rh} %: only {bad:?}");
    }
}

#[test]
fn the_nominal_frequencies_miss_the_table() {
    // The table is computed at the exact midband frequencies. Evaluated at the nominal ones, as
    // SPPS and TCR do, the same equations miss it by more than 1 % in several bands.
    let air = Atmosphere::at_reference_pressure(20.0, 50.0);
    let nominal: Vec<f64> = NOMINAL
        .iter()
        .map(|&n| 1000.0 * attenuation_db_per_m(n, &air).unwrap())
        .collect();
    let bad = outside_one_percent(&nominal, &GOST_20C_50RH);
    println!("at the nominal frequencies: {bad:?}");
    assert!(bad.iter().any(|&(n, r)| n == 160.0 && r > 0.015), "{bad:?}");
    assert!(worst(&relative(&nominal, &GOST_20C_50RH)) < 0.017);
}

#[test]
fn upstreams_form_agrees_at_the_reference_pressure_only() {
    let at = |p: f64| Atmosphere {
        temperature_c: 20.0,
        relative_humidity_percent: 50.0,
        pressure_pa: p,
    };
    let diff = |p: f64| {
        NOMINAL
            .iter()
            .map(|&n| {
                let f = exact_midband_hz(n).unwrap();
                let (u, o) = (
                    upstream_attenuation_db_per_m(f, &at(p)).unwrap(),
                    attenuation_db_per_m(f, &at(p)).unwrap(),
                );
                (u / o - 1.0).abs()
            })
            .fold(0.0_f64, f64::max)
    };
    // Algebraically the same equations: 0.035 % at 101.325 kPa.
    assert!(diff(101_325.0) < 0.00035, "{}", diff(101_325.0));
    // Its `h` lacks the pressure factor (Coef_Att_Atmos.cpp:56). At 80 kPa, about 2000 m up, that
    // is a 25 % difference in some band.
    println!("at 80 kPa: {:.1} %", 100.0 * diff(80_000.0));
    assert!(diff(80_000.0) > 0.2);
    // The factor is the standard's: h is the partial pressure of water vapour over the ambient
    // pressure (ISO 9613-1, clause 6.1, note 3, page 2), so halving the pressure doubles h.
    let h = |p| molar_concentration_percent(&at(p)).unwrap();
    assert!((h(50_662.5) / h(101_325.0) - 2.0).abs() < 1e-12);
}

#[test]
fn the_energy_attenuation_is_alpha_times_ln10_over_10() {
    // 1 dB/m is 0.2303 per metre of energy: after 1/m metres the intensity is down 4.34 dB.
    let m = energy_attenuation_per_m(1.0);
    assert!((m - 0.230_258_509_299_404_6).abs() < 1e-15);
    assert!((10.0 * (1.0f64).exp().log10() - 1.0 / m).abs() < 1e-12);
    // The table's 1 kHz value over 1 km: 4.66 dB of intensity loss.
    let m = energy_attenuation_per_m(4.66e-3);
    assert!((-10.0 * (-m * 1000.0).exp().log10() - 4.66).abs() < 1e-9);
    // What SPPS and TCR use at 1000 Hz nominal, 20 °C, 50 %: upstream's α there, converted.
    let air = Atmosphere::at_reference_pressure(20.0, 50.0);
    let solver = solver_air_absorption_per_m(1000.0, &air).unwrap();
    let alpha = upstream_attenuation_db_per_m(1000.0, &air).unwrap();
    assert_eq!(solver, alpha * std::f64::consts::LN_10 / 10.0);
    println!("m(1 kHz) = {solver:.6e} per metre");
    assert!((solver / energy_attenuation_per_m(4.66e-3) - 1.0).abs() < 0.002);
    // Refusals pass through.
    assert_eq!(
        solver_air_absorption_per_m(-1.0, &air).unwrap_err().code(),
        codes::BAD_AIR
    );
}

/// Clause 7 (printed page 4 of the preview): ±10 % for `h` from 0.05 % to 5 % between −20 °C
/// and +50 °C, ±20 % for `h` from 0.005 % to 0.05 % or above 5 % in the same temperatures, ±50 %
/// for `h` below 0.005 % above 200 K; each below 200 kPa and at 4·10⁻⁴ to 10 Hz/Pa.
#[test]
fn the_stated_accuracy_follows_clause_7_and_says_none_outside_it() {
    use air::StatedAccuracy::{FiftyPercent, TenPercent, TwentyPercent};
    let at = |t: f64, rh: f64, p: f64, f: f64| {
        let air = Atmosphere {
            temperature_c: t,
            relative_humidity_percent: rh,
            pressure_pa: p,
        };
        let h = molar_concentration_percent(&air).unwrap();
        let a = air::attenuation(f, &air).unwrap();
        assert_eq!(a.db_per_m, attenuation_db_per_m(f, &air).unwrap());
        (h, a.stated_accuracy)
    };
    let sea = 101_325.0;
    // Gate (b)'s air, in every band it checks.
    for n in NOMINAL {
        assert_eq!(
            at(20.0, 50.0, sea, exact_midband_hz(n).unwrap()).1,
            Some(TenPercent)
        );
    }
    // Drier air steps down a class at h = 0.05 % and again at 0.005 %.
    for (rh, class) in [
        (50.0, TenPercent),
        (1.0, TwentyPercent),
        (0.1, FiftyPercent),
        (0.0, FiftyPercent),
    ] {
        let (h, got) = at(20.0, rh, sea, 1000.0);
        println!("20 °C, {rh} % RH: h = {h:.4} %, {got:?}");
        assert_eq!(got, Some(class), "{rh} % RH, h {h}");
    }
    // Outside every class: below 40.5 Hz at sea level, above 200 kPa, and humid air above 50 °C
    // or below −20 °C.
    assert_eq!(at(20.0, 50.0, sea, 25.0).1, None);
    assert_eq!(at(20.0, 50.0, sea, 50.0).1, Some(TenPercent));
    assert_eq!(at(20.0, 50.0, 250_000.0, 1000.0).1, None);
    for t in [60.0, -30.0] {
        let (h, got) = at(t, 50.0, sea, 1000.0);
        assert!(h >= 0.005, "{t} °C: h {h}");
        assert_eq!(got, None, "{t} °C");
    }
    // Clause 7.3 has no upper temperature: bone-dry air at 60 °C is ±50 %.
    assert_eq!(at(60.0, 0.0, sea, 1000.0).1, Some(FiftyPercent));
}

/// One line of an upstream source file, lossily decoded (the sources hold Latin-1 comments).
fn upstream_line(rel: &str, line: usize) -> String {
    let bytes = std::fs::read(paths::upstream_file(rel)).unwrap();
    let text = String::from_utf8_lossy(&bytes).into_owned();
    text.lines()
        .nth(line - 1)
        .unwrap_or_else(|| panic!("{rel} has no line {line}"))
        .to_string()
}

/// The citations in `params::air` and `docs/params.md` point at what they claim.
#[test]
fn the_upstream_lines_we_cite_say_what_we_transcribed() {
    let aa = "src/lib_interface/data_manager/data_calculation/Coef_Att_Atmos.cpp";
    for (line, text) in [
        (54, "double C = -6.8346 * pow( K01 / K , 1.261) + 4.6151;"),
        (56, "double hmol = H * Ps / Pref;"),
        (
            59,
            "double Acr = (Pref/P) * (1.60E-10) * sqrt( K / Kref) * pow(F,2);",
        ),
        (
            62,
            "double Fr = (P/Pref) * (24. + 4.04E4 * hmol * (0.02 + hmol) / (0.391 + hmol) );",
        ),
        (
            63,
            "double Am = 1.559 * FmolO * exp(-KvibO/K) * pow(KvibO/K,2);",
        ),
        (
            67,
            "Fr = (P/Pref) * sqrt(Kref/K) * (9. + 280. * hmol * exp( -4.170 * (pow(K/Kref,-1./3.)-1)));",
        ),
    ] {
        assert_eq!(upstream_line(aa, line).trim(), text, "{aa}:{line}");
    }
    let consts = "src/lib_interface/data_manager/data_calculation/calculsPropagation.h";
    for (line, text) in [
        (48, "const double Pref(101325);"),
        (49, "const double Kref(293.15);"),
        (50, "const double FmolO(0.209);"),
        (51, "const double FmolN(0.781);"),
        (52, "const double KvibO(2239.1);"),
        (53, "const double KvibN(3352.0);"),
        (54, "const double K01(273.16);"),
    ] {
        assert!(
            upstream_line(consts, line).trim().starts_with(text),
            "{consts}:{line}"
        );
    }
    assert_eq!(
        upstream_line(
            "src/lib_interface/data_manager/data_calculation/Celerite_du_son.cpp",
            46
        )
        .trim(),
        "double c = 343.2 * sqrt(K/Kref);"
    );
    let conf = upstream_line(
        "src/lib_interface/data_manager/base_core_configuration.cpp",
        114,
    );
    assert!(
        conf.contains("Coef_Att_Atmos((*nvFreq).freqValue,")
            && conf.ends_with("*(logf(10.f)/10.f);"),
        "{conf}"
    );
    // The nominal frequency is an integer read from config.xml.
    assert!(
        upstream_line(
            "src/lib_interface/data_manager/base_core_configuration.cpp",
            109
        )
        .contains("GetProperty(\"freq\").ToInt()")
    );
    assert_eq!(air::speed_of_sound(20.0), 343.2);
    // SPPS applies the per-step factor to a particle's energy in energetic mode and as a survival
    // probability in random mode.
    let spps = "src/spps/CalculationCore.cpp";
    assert_eq!(
        upstream_line(spps, 57).trim(),
        "configurationP.energie*=densite_proba_absorption_atmospherique;"
    );
    assert_eq!(
        upstream_line(spps, 64).trim(),
        "if(GetRandValue()>=densite_proba_absorption_atmospherique)"
    );
}
