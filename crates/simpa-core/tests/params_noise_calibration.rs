//! The Monte-Carlo noise model's calibration against SPPS's own seed-to-seed spread, checked on the
//! committed receipt (`docs/investigations/2026-09-25-noise-calibration/calibration.json`; the
//! rules are pre-registered in `PREREGISTER.txt` beside it, and the numbers come from real SPPS
//! runs, `crates/simpa/tests/noise_calibration.rs`, run on purpose).
//!
//! Each check says no:
//! - the code's factors, margins and `1/√N` flags are the ones the rules derive from the receipt's
//!   calibration cells; a factor moved through the code (the fault seam), a cell's estimates
//!   scattering more, or a pair read at the wrong count, and they no longer are;
//! - on every validation cell and quantity the calibrated prediction is at or above the seeds'
//!   spread within its statistical uncertainty; the factors halved through the code (a model twice
//!   too optimistic) fail it in every validation cell, and one quantity's observed spread raised
//!   above its prediction in the input fails that quantity's check alone;
//! - round 1's energetic T20 and T30 factors, which failed their validation, fail it here too:
//!   the check that passes round 2 is the one that caught round 1.

#[path = "common/noise_calibration.rs"]
mod calibration;

use serde_json::Value;
use simpa_core::faults::{self, Fault};
use simpa_core::params::noise::{self, Method};

fn receipt() -> Value {
    let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../docs/investigations/2026-09-25-noise-calibration/calibration.json");
    serde_json::from_str(
        &std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display())),
    )
    .unwrap()
}

fn method(name: &str) -> Method {
    match name {
        "random" => Method::Random,
        _ => Method::Energetic,
    }
}

const METHODS: [&str; 2] = ["random", "energetic"];

/// Every quantity whose code factor, margin or `1/√N` flag differs from what rules 2, 5b and 4
/// derive from the receipt, with both.
fn rule_mismatches(r: &Value) -> Vec<String> {
    let mut out = Vec::new();
    for name in METHODS {
        for (i, q) in calibration::QUANTITIES.iter().enumerate() {
            let cal = calibration::cells_in(r, name, calibration::calibration_roles(name, q));
            assert!(
                cal.len() >= 7,
                "{name} {q}: {} calibration cells",
                cal.len()
            );
            let (factor, by) = calibration::shipped_factor(r, name, q);
            let (margin, scattered) = calibration::margin(&cal, q).unwrap();
            let (confirmed, differ) = calibration::root_n_confirmed(r, name, q);
            let code = (
                noise::calibration::factor(method(name), i),
                noise::calibration::margin(method(name), i),
                noise::calibration::root_n_confirmed(method(name), i),
            );
            println!(
                "{name} {q}: factor {factor} (set by {by}), margin {margin} (by {scattered}), \
                 1/sqrt(N) {confirmed} {differ:?}; code {code:?}"
            );
            if (factor - code.0).abs() > 1e-12 * factor {
                out.push(format!("{name} {q} factor {factor} vs {}", code.0));
            }
            if (margin - code.1).abs() > 1e-12 * margin {
                out.push(format!("{name} {q} margin {margin} vs {}", code.1));
            }
            if confirmed != code.2 {
                out.push(format!("{name} {q} 1/sqrt(N) {confirmed} vs {}", code.2));
            }
        }
    }
    out
}

#[test]
fn the_codes_factors_margins_and_scaling_flags_are_the_rules_on_the_receipt() {
    let r = receipt();
    assert_eq!(rule_mismatches(&r), Vec::<String>::new());
    // Energetic T20 and T30 ship at M7's bound: round 2's factors failed a round-2 room.
    for q in ["t20_s", "t30_s"] {
        let (k, why) = calibration::shipped_factor(&r, "energetic", q);
        println!("energetic {q}: {k}, {why}");
        assert_eq!(k, 1.0, "{why}");
        assert!(why.contains("W-E4"), "{why}");
    }
    // Says no through the code: every factor moved by 10 %.
    let m = faults::with(Fault::NoiseCalibrationScaled { by: 1.1 }, || {
        rule_mismatches(&r)
    });
    assert_eq!(
        m.iter().filter(|x| x.contains(" factor ")).count(),
        16,
        "{m:?}"
    );
    // Says no through the input: with W-E4's T30 spread a third of what it was, round 2's T30
    // factor passes its validation and would ship; the code's 1 no longer matches.
    let mut r4 = receipt();
    for row in cell_mut(&mut r4, "W-E4")["quantities"]["t30_s"]
        .as_array_mut()
        .unwrap()
    {
        let o = row["observed_sd"].as_f64().unwrap();
        row["observed_sd"] = (o / 3.0).into();
    }
    let m = rule_mismatches(&r4);
    assert_eq!(m, vec!["energetic t30_s factor 0.31 vs 1".to_string()]);
    // Says no through the input: one calibration cell's T30 estimates scattering 0.2 more moves
    // random mode's T30 margin.
    let mut r2 = receipt();
    for row in cell_mut(&mut r2, "C-R1")["quantities"]["t30_s"]
        .as_array_mut()
        .unwrap()
    {
        let s = row["predicted_scatter"].as_f64().unwrap();
        row["predicted_scatter"] = (s + 0.2).into();
    }
    let m = rule_mismatches(&r2);
    assert_eq!(m.len(), 1, "{m:?}");
    assert!(m[0].starts_with("random t30_s margin"), "{m:?}");
    // And a pair read at the wrong count flips flags: C-E2 read as twice its particles.
    let mut r3 = receipt();
    let c = cell_mut(&mut r3, "C-E2");
    let n = c["cell"]["particles_per_source"].as_f64().unwrap();
    c["cell"]["particles_per_source"] = (2.0 * n).into();
    let m = rule_mismatches(&r3);
    assert!(
        m.iter()
            .any(|x| x.starts_with("energetic") && x.contains("1/sqrt(N)")),
        "{m:?}"
    );
}

fn cell_mut<'a>(r: &'a mut Value, id: &str) -> &'a mut Value {
    r["cells"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|c| c["cell"]["id"] == id)
        .unwrap_or_else(|| panic!("{id}"))
}

/// Every validation cell and quantity that fails rule 3 with `factor(method, i)`, and how many
/// quantities each validation cell had checked.
fn validation_failures(
    r: &Value,
    factor: impl Fn(Method, usize) -> f64,
) -> (Vec<String>, Vec<(String, usize)>) {
    let mut fails = Vec::new();
    let mut checked: Vec<(String, usize)> = Vec::new();
    for name in METHODS {
        for (i, q) in calibration::QUANTITIES.iter().enumerate() {
            let val = calibration::cells_in(r, name, calibration::validation_roles(name, q));
            assert!(val.len() >= 6, "{name} {q}: {} validation cells", val.len());
            for c in val {
                let id = c["cell"]["id"].as_str().unwrap().to_string();
                let Some((ok, p)) = calibration::validates(c, q, factor(method(name), i)) else {
                    continue;
                };
                match checked.iter_mut().find(|(x, _)| *x == id) {
                    Some((_, k)) => *k += 1,
                    None => checked.push((id.clone(), 1)),
                }
                if !ok {
                    fails.push(format!(
                        "{id} {q}: {:.3} [{:.3}, {:.3}]",
                        p.ratio, p.lower, p.upper
                    ));
                }
            }
        }
    }
    (fails, checked)
}

fn code_factor(m: Method, i: usize) -> f64 {
    noise::calibration::factor(m, i)
}

#[test]
fn every_validation_cell_is_at_or_below_its_calibrated_prediction() {
    let r = receipt();
    let (fails, checked) = validation_failures(&r, code_factor);
    println!("checked: {checked:?}");
    assert_eq!(fails, Vec::<String>::new());
    // No cell passes for having nothing to check: two quantities at least are in each. The 25
    // are round 1's and round 2's validation cells, and the energetic calibration cells, which
    // check energetic T20 and T30 at M7's bound.
    assert_eq!(checked.len(), 25);
    assert!(checked.iter().all(|(_, k)| *k >= 2), "{checked:?}");
    // Says no through the code: a model twice as optimistic fails the validation, in every
    // validation cell of both rounds.
    let (fails, _) = faults::with(Fault::NoiseCalibrationScaled { by: 0.5 }, || {
        validation_failures(&r, code_factor)
    });
    println!("factors halved: {} failures", fails.len());
    let held_out: Vec<&String> = checked
        .iter()
        .map(|(id, _)| id)
        .filter(|id| !id.starts_with("C-"))
        .collect();
    assert_eq!(held_out.len(), 18);
    for id in held_out {
        assert!(
            fails.iter().any(|f| f.starts_with(&format!("{id} "))),
            "{id} passes at half: {fails:#?}"
        );
    }
}

/// The energetic T20 and T30 checks of `roles`' cells that fail with factors `t20` and `t30`.
fn energetic_decay_failures(r: &Value, roles: &[&str], t20: f64, t30: f64) -> Vec<String> {
    let mut fails = Vec::new();
    for c in calibration::cells_in(r, "energetic", roles) {
        for (q, k) in [("t20_s", t20), ("t30_s", t30)] {
            if let Some((false, p)) = calibration::validates(c, q, k) {
                fails.push(format!(
                    "{} {q}: {:.3} [{:.3}]",
                    c["cell"]["id"].as_str().unwrap(),
                    p.ratio,
                    p.lower
                ));
            }
        }
    }
    fails
}

#[test]
fn both_rounds_energetic_t20_and_t30_fail_where_they_failed() {
    // Round 1 set energetic T20 and T30 at 0.26 and 0.15 from its seven calibration cells, and its
    // validation caught them in V-E6; round 2 set them at 0.43 and 0.31 from all thirteen, and its
    // validation caught them in W-E4. The same check on the same cells says so again, and M7's
    // bound, which ships, holds on every energetic cell.
    let r = receipt();
    let f1 = energetic_decay_failures(&r, &["validation"], 0.26, 0.15);
    println!("round 1: {f1:?}");
    assert_eq!(f1.len(), 2, "{f1:?}");
    assert!(f1.iter().all(|f| f.starts_with("V-E6 ")), "{f1:?}");
    let f2 = energetic_decay_failures(&r, &["validation2"], 0.43, 0.31);
    println!("round 2: {f2:?}");
    assert_eq!(f2.len(), 2, "{f2:?}");
    assert!(f2.iter().all(|f| f.starts_with("W-E4 ")), "{f2:?}");
    let all = ["calibration", "validation", "validation2"];
    assert_eq!(
        energetic_decay_failures(&r, &all, 1.0, 1.0),
        Vec::<String>::new()
    );
    // Says no: half M7's bound fails W-E4's T30.
    let half = energetic_decay_failures(&r, &all, 0.5, 0.5);
    assert!(half.iter().any(|f| f.starts_with("W-E4 t30_s")), "{half:?}");
}

#[test]
fn a_quantity_noisier_than_its_prediction_fails_its_check() {
    // Says no through the input: in one validation cell of each method, one quantity's observed
    // spread raised to 1.5 times its calibrated prediction in every receiver-band.
    for (name, id, q) in [("random", "V-R1", "t30_s"), ("energetic", "W-E1", "c80_db")] {
        let mut r = receipt();
        let qi = calibration::QUANTITIES
            .iter()
            .position(|x| *x == q)
            .unwrap();
        let k = noise::calibration::factor(method(name), qi);
        let structure = calibration::structure(name);
        for row in cell_mut(&mut r, id)["quantities"][q]
            .as_array_mut()
            .unwrap()
        {
            let p = row["predicted_sd"][structure].as_f64().unwrap();
            row["observed_sd"] = (1.5 * k * p).into();
        }
        let (fails, _) = validation_failures(&r, code_factor);
        println!("{name}: {fails:?}");
        assert_eq!(fails.len(), 1, "{name}: {fails:?}");
        assert!(fails[0].starts_with(&format!("{id} {q}:")), "{fails:?}");
    }
}

#[test]
fn the_receipt_is_the_pre_registered_design() {
    // 32 cells over ten seeds each, their roles as pre-registered: round 1's calibration and
    // validation cells, and round 2's validation cells.
    let r = receipt();
    let all = r["cells"].as_array().unwrap();
    assert_eq!(all.len(), 32);
    let pre = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../docs/investigations/2026-09-25-noise-calibration/PREREGISTER.txt"),
    )
    .unwrap();
    for c in all {
        let id = c["cell"]["id"].as_str().unwrap();
        assert_eq!(calibration::seeds(c), 10, "{id}");
        let role = match &id[..2] {
            "C-" => "calibration",
            "V-" => "validation",
            _ => "validation2",
        };
        assert_eq!(c["cell"]["role"], role, "{id}");
        assert!(
            pre.contains(&format!(" {id} ")),
            "{id} is not pre-registered"
        );
    }
}
