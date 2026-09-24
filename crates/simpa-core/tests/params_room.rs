//! Sabine and Eyring as TCR computes them, and M7 gate (f), the DIN 18041 A3 target
//! (`docs/params.md`). Each check has its say-no partner.

use std::f64::consts::LN_10;

use simpa_core::params::codes;
use simpa_core::params::din18041::{Group, target_s};
use simpa_core::params::room::{
    RtConstant, Surface, TCR_CONSTANT, eyring_absorption_area, eyring_rt, reverberation_time,
    sabine_absorption_area, sabine_rt,
};

#[allow(dead_code)]
#[path = "common/paths.rs"]
mod paths;

/// A 10 m cube, every face α.
fn cube(alpha: f64) -> Vec<Surface> {
    vec![
        Surface {
            area_m2: 100.0,
            absorption: alpha,
        };
        6
    ]
}

fn close(a: f64, b: f64, rel: f64) -> bool {
    (a / b - 1.0).abs() <= rel
}

#[test]
fn sabine_and_eyring_match_their_closed_forms() {
    let s = cube(0.1);
    assert!((sabine_absorption_area(&s).unwrap() - 60.0).abs() < 1e-12);
    let a_eyring = -600.0 * 0.9f64.ln();
    assert!((eyring_absorption_area(&s).unwrap() - a_eyring).abs() < 1e-12);
    // TCR's constant, air off and on (m = 0.01 per metre adds 4·m·V = 40 m²).
    let tcr = RtConstant::Tcr;
    assert!((sabine_rt(1000.0, &s, None, tcr).unwrap() - 0.163 * 1000.0 / 60.0).abs() < 1e-12);
    assert!((eyring_rt(1000.0, &s, None, tcr).unwrap() - 163.0 / a_eyring).abs() < 1e-12);
    assert!((sabine_rt(1000.0, &s, Some(0.01), tcr).unwrap() - 163.0 / 100.0).abs() < 1e-12);
    assert!(
        (eyring_rt(1000.0, &s, Some(0.01), tcr).unwrap() - 163.0 / (40.0 + a_eyring)).abs() < 1e-12
    );
    // Mixed absorption: ᾱ is area-weighted before the logarithm.
    let mixed = [
        Surface {
            area_m2: 100.0,
            absorption: 0.1,
        },
        Surface {
            area_m2: 200.0,
            absorption: 0.5,
        },
    ];
    let abar: f64 = (10.0 + 100.0) / 300.0;
    assert!((eyring_absorption_area(&mixed).unwrap() + 300.0 * (1.0 - abar).ln()).abs() < 1e-12);
    // Eyring's area always exceeds Sabine's for α > 0; equal as α → 0.
    assert!(eyring_absorption_area(&s).unwrap() > sabine_absorption_area(&s).unwrap());
    let small = cube(1e-9);
    assert!(close(
        eyring_absorption_area(&small).unwrap(),
        sabine_absorption_area(&small).unwrap(),
        1e-8
    ));
    // A fully absorbing room has an Eyring time of 0 s.
    assert_eq!(eyring_rt(1000.0, &cube(1.0), None, tcr).unwrap(), 0.0);
}

#[test]
fn tcrs_constant_is_not_the_physical_one() {
    // 24·ln10/c at TCR's c = 343.2 m/s is 0.16102; TCR's 0.163 is 1.23 % above it. A gate that
    // compared TCR with the physical constant at 0.5 % would fail on a correct solver.
    let physical = RtConstant::Physical {
        speed_of_sound: 343.2,
    };
    assert!((physical.value() - 24.0 * LN_10 / 343.2).abs() < 1e-15);
    assert!((physical.value() - 0.161_020).abs() < 1e-6);
    let s = cube(0.1);
    let tcr = sabine_rt(1000.0, &s, None, RtConstant::Tcr).unwrap();
    let phys = sabine_rt(1000.0, &s, None, physical).unwrap();
    assert!(!close(phys, tcr, 0.005), "{phys} vs {tcr}");
    assert!((tcr / phys - 1.0 - 0.0123).abs() < 5e-5, "{}", tcr / phys);
    assert_eq!(TCR_CONSTANT, 0.163);
}

#[test]
fn bad_rooms_are_refused() {
    let tcr = RtConstant::Tcr;
    let s = cube(0.1);
    let code = |r: Result<f64, simpa_core::params::ParamError>| r.unwrap_err().code();
    for alpha in [-0.1, 1.2, f64::NAN] {
        assert_eq!(
            code(sabine_rt(1000.0, &cube(alpha), None, tcr)),
            codes::BAD_ROOM
        );
        assert_eq!(
            code(eyring_rt(1000.0, &cube(alpha), None, tcr)),
            codes::BAD_ROOM
        );
    }
    let neg = [Surface {
        area_m2: -1.0,
        absorption: 0.1,
    }];
    assert_eq!(code(sabine_rt(1000.0, &neg, None, tcr)), codes::BAD_ROOM);
    assert_eq!(code(sabine_rt(1000.0, &[], None, tcr)), codes::BAD_ROOM);
    for v in [0.0, -1.0, f64::NAN, f64::INFINITY] {
        assert_eq!(code(sabine_rt(v, &s, None, tcr)), codes::BAD_ROOM);
    }
    for m in [-0.001, f64::NAN, f64::INFINITY] {
        assert_eq!(code(sabine_rt(1000.0, &s, Some(m), tcr)), codes::BAD_ROOM);
    }
    let bad_c = RtConstant::Physical {
        speed_of_sound: 0.0,
    };
    assert_eq!(code(sabine_rt(1000.0, &s, None, bad_c)), codes::BAD_ROOM);
    // No absorption at all: the time is infinite, refused rather than returned.
    assert_eq!(
        code(sabine_rt(1000.0, &cube(0.0), None, tcr)),
        codes::NO_ABSORPTION
    );
    assert_eq!(
        code(reverberation_time(1000.0, 0.0, Some(0.0), tcr)),
        codes::NO_ABSORPTION
    );
    // Air alone is absorption.
    assert!(sabine_rt(1000.0, &cube(0.0), Some(0.01), tcr).is_ok());
}

/// One line of an upstream source file, lossily decoded.
fn upstream_line(rel: &str, line: usize) -> String {
    let bytes = std::fs::read(paths::upstream_file(rel)).unwrap();
    String::from_utf8_lossy(&bytes)
        .lines()
        .nth(line - 1)
        .unwrap_or_else(|| panic!("{rel} has no line {line}"))
        .trim()
        .to_string()
}

/// `params::room` cites TCR's lines; they say what we transcribed.
#[test]
fn tcrs_lines_say_what_we_transcribed() {
    let tcr = "src/ctr/TC_CalculationCore.cpp";
    for (line, text) in [
        (
            13,
            "if(faceInfo.faceEncombrement==NULL || faceInfo.faceMaterial->matSpectrumProperty[0].tau==1)",
        ),
        (
            98,
            "A_Sabine+=aireSurface*this->sceneMesh->pfaces[idface].faceMaterial->matSpectrumProperty[idFreq].absorption;",
        ),
        (
            129,
            "A_Moyen+=(aireSurface*this->sceneMesh->pfaces[idface].faceMaterial->matSpectrumProperty[idFreq].absorption)/aireTot;",
        ),
        (132, "return -aireTot*log(1-A_Moyen);"),
        (
            138,
            "return 0.163 * this->mainData.sceneVolume / (4*this->configurationTool->freqList[idFreq]->absorption_atmospherique*this->mainData.sceneVolume + A);",
        ),
        (140, "return 0.163 * this->mainData.sceneVolume / (A);"),
        (223, "float AireAbsorptionEquivalente;"),
    ] {
        assert_eq!(upstream_line(tcr, line), text, "{tcr}:{line}");
    }
}

#[test]
fn gate_f_din_18041_a3_at_180_m3_is_0_552_s() {
    let within = |t: f64| (t - 0.552).abs() <= 0.001;
    let t = target_s(Group::A3, 180.0).unwrap();
    println!("A3, 180 m³: {t:.4} s");
    assert!(within(t), "{t}");
    // Say-no partners: the neighbouring group, and the natural logarithm for lg, both miss.
    assert!(!within(target_s(Group::A2, 180.0).unwrap()));
    assert!(!within(target_s(Group::A4, 180.0).unwrap()));
    assert!(!within(0.32 * 180f64.ln() - 0.17));
    // The design README's 0.55 s to two decimals.
    assert_eq!(format!("{t:.2}"), "0.55");
}

#[test]
fn din_18041_groups_and_ranges() {
    let at = |g, v| target_s(g, v).unwrap();
    // The sourced formulas where lg V is round (100 m³, 1000 m³).
    for (g, v, want) in [
        (Group::A1, 1000.0, 1.42),
        (Group::A2, 1000.0, 0.97),
        (Group::A3, 1000.0, 0.79),
        (Group::A4, 100.0, 0.38),
        (Group::A5, 1000.0, 1.25),
    ] {
        assert!((at(g, v) - want).abs() < 1e-12, "{g:?}");
    }
    // A5: the formula meets 2.0 s at 10 000 m³ and stays there to 30 000 m³.
    assert!((at(Group::A5, 10_000.0) - 2.0).abs() < 1e-12);
    assert_eq!(at(Group::A5, 20_000.0), 2.0);
    assert_eq!(at(Group::A5, 30_000.0), 2.0);
    // Each group's range, both ends accepted, and just outside each end refused. The ranges are
    // the solid stretches of the standard's figure (Nocke 2016, Bild 2): A4 stops at 500 m³.
    let code = |g, v| target_s(g, v).unwrap_err().code();
    for (g, lo, hi) in [
        (Group::A1, 30.0, 1000.0),
        (Group::A2, 50.0, 5000.0),
        (Group::A3, 30.0, 5000.0),
        (Group::A4, 30.0, 500.0),
        (Group::A5, 200.0, 30_000.0),
    ] {
        assert_eq!(g.volume_range_m3(), (lo, hi));
        assert!(target_s(g, lo).is_ok() && target_s(g, hi).is_ok(), "{g:?}");
        assert_eq!(code(g, lo * 0.999), codes::DIN_OUT_OF_RANGE, "{g:?}");
        assert_eq!(code(g, hi * 1.001), codes::DIN_OUT_OF_RANGE, "{g:?}");
    }
    // The first version accepted A4 up to 5000 m³ and A3 down to 4 m³; both are refused now.
    assert_eq!(code(Group::A4, 1000.0), codes::DIN_OUT_OF_RANGE);
    assert_eq!(code(Group::A3, 4.0), codes::DIN_OUT_OF_RANGE);
    // Every target in range is positive.
    for g in [Group::A1, Group::A2, Group::A3, Group::A4, Group::A5] {
        assert!(at(g, g.volume_range_m3().0) > 0.2, "{g:?}");
    }
    // A volume that is not a number.
    for v in [0.0, -1.0, f64::NAN, f64::INFINITY] {
        assert_eq!(code(Group::A3, v), codes::DIN_OUT_OF_RANGE);
    }
}
