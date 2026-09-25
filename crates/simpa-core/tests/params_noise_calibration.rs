//! The Monte-Carlo noise model's calibration against SPPS's own seed-to-seed spread, checked on the
//! committed receipt (`docs/investigations/2026-09-25-noise-calibration/calibration.json`; the
//! rules are pre-registered in `PREREGISTER.txt` beside it, and the numbers come from real SPPS
//! runs, `crates/simpa/tests/noise_calibration.rs`, run on purpose).
//!
//! Each check says no:
//! - the code's factors are the ones the rule derives from the calibration cells; a factor moved
//!   through the code (the fault seam) no longer is;
//! - on every validation cell and quantity the calibrated prediction is at or above the seeds'
//!   spread within its statistical uncertainty; the factors halved through the code (a model twice
//!   too optimistic) fail it, and one quantity's observed spread raised above its prediction in the
//!   input fails that quantity's check;
//! - the seeds' spread falls as `1/√N`, which the particle count a refusal names rests on; a pair
//!   whose count is misread fails it.

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

fn cells<'a>(r: &'a Value, role: &str, method: &str) -> Vec<&'a Value> {
    r["cells"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|c| c["cell"]["role"] == role && c["cell"]["method"] == method)
        .collect()
}

fn method(name: &str) -> Method {
    match name {
        "random" => Method::Random,
        _ => Method::Energetic,
    }
}

/// Every quantity of `method` whose code factor differs from the one the rule derives, with both.
fn factor_mismatches(r: &Value, name: &str) -> Vec<(String, f64, f64)> {
    let cal = cells(r, "calibration", name);
    let mut out = Vec::new();
    for (i, q) in calibration::QUANTITIES.iter().enumerate() {
        let (derived, id) = calibration::factor(&cal, q)
            .unwrap_or_else(|| panic!("{name} {q}: no calibration cell gives it"));
        let code = noise::calibration::factor(method(name), i);
        println!("{name} {q}: derived {derived} (set by {id}), code {code}");
        if (derived - code).abs() > 1e-12 * derived {
            out.push((q.to_string(), derived, code));
        }
    }
    out
}

#[test]
fn the_codes_factors_are_the_calibration_cells_upper_bounds() {
    let r = receipt();
    for name in ["random", "energetic"] {
        assert!(cells(&r, "calibration", name).len() >= 5, "{name}");
        assert_eq!(factor_mismatches(&r, name), vec![], "{name}");
    }
    // Says no through the code: every factor moved by 10 %.
    faults::with(Fault::NoiseCalibrationScaled { by: 1.1 }, || {
        for name in ["random", "energetic"] {
            assert_eq!(factor_mismatches(&r, name).len(), 8, "{name}");
        }
    });
}

/// Every validation cell and quantity that fails rule 3 under the code's factors.
fn validation_failures(r: &Value) -> Vec<String> {
    let mut fails = Vec::new();
    for name in ["random", "energetic"] {
        let val = cells(r, "validation", name);
        assert!(val.len() >= 5, "{name}: {} validation cells", val.len());
        for c in val {
            let id = c["cell"]["id"].as_str().unwrap();
            let mut checked = 0;
            for (i, q) in calibration::QUANTITIES.iter().enumerate() {
                let Some((ok, p)) =
                    calibration::validates(c, q, noise::calibration::factor(method(name), i))
                else {
                    continue;
                };
                checked += 1;
                if !ok {
                    fails.push(format!(
                        "{id} {q}: {:.3} [{:.3}, {:.3}]",
                        p.ratio, p.lower, p.upper
                    ));
                }
            }
            // No cell passes for having nothing to check: the decay times and SPL are in each.
            assert!(checked >= 4, "{id}: {checked} quantities checked");
        }
    }
    fails
}

#[test]
fn every_validation_cell_is_at_or_below_its_calibrated_prediction() {
    let r = receipt();
    assert_eq!(validation_failures(&r), Vec::<String>::new());
    // Says no through the code: a model twice as optimistic fails the validation.
    let fails = faults::with(Fault::NoiseCalibrationScaled { by: 0.5 }, || {
        validation_failures(&r)
    });
    println!("factors halved: {} failures: {fails:#?}", fails.len());
    assert!(!fails.is_empty());
    for name in ["random", "energetic"] {
        let val = cells(&r, "validation", name);
        let failed_cells = val
            .iter()
            .filter(|c| {
                let id = c["cell"]["id"].as_str().unwrap();
                fails.iter().any(|f| f.starts_with(&format!("{id} ")))
            })
            .count();
        assert_eq!(failed_cells, val.len(), "{name}: every cell fails at half");
    }
}

#[test]
fn a_quantity_noisier_than_its_prediction_fails_its_check() {
    // Says no through the input: in one validation cell of each method, one quantity's observed
    // spread raised to 1.5 times its calibrated prediction in every receiver-band.
    for (name, q) in [("random", "t30_s"), ("energetic", "edt_s")] {
        let mut r = receipt();
        let qi = calibration::QUANTITIES
            .iter()
            .position(|x| *x == q)
            .unwrap();
        let k = noise::calibration::factor(method(name), qi);
        let id = cells(&r, "validation", name)[0]["cell"]["id"]
            .as_str()
            .unwrap()
            .to_string();
        let structure = calibration::structure(name);
        let cell = r["cells"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|c| c["cell"]["id"] == id.as_str())
            .unwrap();
        for row in cell["quantities"][q].as_array_mut().unwrap() {
            let p = row["predicted_sd"][structure].as_f64().unwrap();
            row["observed_sd"] = (1.5 * k * p).into();
        }
        let fails = validation_failures(&r);
        println!("{name}: {fails:?}");
        assert_eq!(fails.len(), 1, "{name}: {fails:?}");
        assert!(fails[0].starts_with(&format!("{id} {q}:")), "{fails:?}");
    }
}

fn root_n_failures(r: &Value) -> Vec<String> {
    let find = |id: &str| {
        r["cells"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["cell"]["id"] == id)
            .unwrap_or_else(|| panic!("{id}"))
    };
    let mut fails = Vec::new();
    for (a, b) in calibration::PAIRS {
        let mut checked = 0;
        for q in calibration::QUANTITIES {
            if let Some((x, y, ok)) = calibration::root_n(find(a), find(b), q) {
                checked += 1;
                if !ok {
                    fails.push(format!("{a}/{b} {q}: {:.4} vs {:.4}", x.0, y.0));
                }
            }
        }
        assert!(checked >= 4, "{a}/{b}: {checked}");
    }
    fails
}

#[test]
fn the_spread_falls_as_one_over_the_root_of_the_particle_count() {
    let r = receipt();
    assert_eq!(root_n_failures(&r), Vec::<String>::new());
    // Says no through the input: C-R2 read as twice its particle count.
    let mut r = receipt();
    let c = r["cells"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|c| c["cell"]["id"] == "C-R2")
        .unwrap();
    let n = c["cell"]["particles_per_source"].as_f64().unwrap();
    c["cell"]["particles_per_source"] = (2.0 * n).into();
    let fails = root_n_failures(&r);
    println!("{fails:?}");
    assert!(
        fails.iter().any(|f| f.starts_with("C-R1/C-R2 ")),
        "{fails:?}"
    );
}

#[test]
fn the_receipt_is_the_pre_registered_design() {
    // 26 cells over ten seeds each, their roles as pre-registered.
    let r = receipt();
    let all = r["cells"].as_array().unwrap();
    assert_eq!(all.len(), 26);
    let pre = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../docs/investigations/2026-09-25-noise-calibration/PREREGISTER.txt"),
    )
    .unwrap();
    for c in all {
        let id = c["cell"]["id"].as_str().unwrap();
        assert_eq!(calibration::seeds(c), 10, "{id}");
        let role = if id.starts_with("C-") {
            "calibration"
        } else {
            "validation"
        };
        assert_eq!(c["cell"]["role"], role, "{id}");
        assert!(
            pre.contains(&format!(" {id} ")),
            "{id} is not pre-registered"
        );
    }
}

/// Every quantity of `method` whose code margin or 1/√N flag differs from what rules 5b and 4
/// derive from the receipt, with both.
fn margin_and_flag_mismatches(r: &Value, name: &str) -> Vec<String> {
    let cal = cells(r, "calibration", name);
    let mut out = Vec::new();
    for (i, q) in calibration::QUANTITIES.iter().enumerate() {
        let (derived, id) = calibration::margin(&cal, q)
            .unwrap_or_else(|| panic!("{name} {q}: no calibration cell gives it"));
        let code = noise::calibration::margin(method(name), i);
        let (confirmed, differ) = calibration::root_n_confirmed(r, name, q);
        let flag = noise::calibration::root_n_confirmed(method(name), i);
        println!(
            "{name} {q}: margin derived {derived} (set by {id}), code {code}; 1/sqrt(N) \
             confirmed {confirmed} {differ:?}, code {flag}"
        );
        if (derived - code).abs() > 1e-12 * derived {
            out.push(format!("{q} margin {derived} vs {code}"));
        }
        if confirmed != flag {
            out.push(format!("{q} 1/sqrt(N) {confirmed} vs {flag}"));
        }
    }
    out
}

#[test]
fn the_codes_margins_and_scaling_flags_are_the_rules_on_the_receipt() {
    let r = receipt();
    for name in ["random", "energetic"] {
        assert_eq!(
            margin_and_flag_mismatches(&r, name),
            Vec::<String>::new(),
            "{name}"
        );
    }
    // Says no through the input: one calibration cell's T30 estimates scattering 0.2 more moves
    // random mode's T30 margin away from the code's.
    let mut r2 = receipt();
    let c = r2["cells"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|c| c["cell"]["id"] == "C-R1")
        .unwrap();
    for row in c["quantities"]["t30_s"].as_array_mut().unwrap() {
        let s = row["predicted_scatter"].as_f64().unwrap();
        row["predicted_scatter"] = (s + 0.2).into();
    }
    let m = margin_and_flag_mismatches(&r2, "random");
    assert!(m.iter().any(|x| x.starts_with("t30_s margin")), "{m:?}");
    // And a pair read at the wrong count flips a flag: C-E2 read as twice its particles.
    let mut r3 = receipt();
    let c = r3["cells"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|c| c["cell"]["id"] == "C-E2")
        .unwrap();
    let n = c["cell"]["particles_per_source"].as_f64().unwrap();
    c["cell"]["particles_per_source"] = (2.0 * n).into();
    let m = margin_and_flag_mismatches(&r3, "energetic");
    assert!(m.iter().any(|x| x.contains("1/sqrt(N)")), "{m:?}");
}
