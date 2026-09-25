//! The Monte-Carlo noise model's calibration against SPPS's own seed-to-seed spread, checked on the
//! committed receipt (`docs/investigations/2026-09-25-noise-calibration/calibration.json`; the
//! rules are pre-registered in `PREREGISTER.txt` beside it, and the numbers come from real SPPS
//! runs, `crates/simpa/tests/noise_calibration.rs`, run on purpose).
//!
//! Round 3 (what the code holds), each check saying no:
//! - the code's variable, factors, corrections, domains, margins and `1/√N` flags are the ones
//!   rules R3-1 to R3-7 (with the addendum's A1 and A2, and F1 where round 3's validation failed)
//!   derive from the receipt's calibration cells; every factor moved through the code (the fault
//!   seam), a calibration cell's spread raised, or a cell read at another particle count, and they
//!   no longer are;
//! - on every round-3 validation cell, method, quantity and kind of wall, over the rows inside the
//!   domain, the calibrated prediction is at or above the seeds' spread within its statistical
//!   uncertainty (R3-8); the factors halved through the code fail it in every validation cell, one
//!   quantity's observed spread raised above its prediction fails that quantity's check alone, the
//!   correction for repeated crossings dropped fails where receivers were large, and the fitted
//!   0.52 for energetic T20 in Lambert rooms fails where it failed (V3-E8), which is why F1 ships 1.
//!
//! Rounds 1 and 2 (history, `7132e42`): their rules on the same receipt still give the numbers
//! they shipped, and both rounds' energetic T20 and T30 factors fail where they failed.

#[path = "common/noise_calibration.rs"]
mod calibration;

use calibration::{Numbers, Split, Var};
use serde_json::Value;
use simpa_core::faults::{self, Fault};
use simpa_core::params::noise::{self, Method, Walls};

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

fn cell_mut<'a>(r: &'a mut Value, id: &str) -> &'a mut Value {
    r["cells"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|c| c["cell"]["id"] == id)
        .unwrap_or_else(|| panic!("{id}"))
}

// --- round 3 -------------------------------------------------------------------------------------

/// The code's variable for a method, as the rules name it.
fn code_var(m: Method) -> Var {
    match noise::calibration::variable(m) {
        noise::calibration::Variable::CrossingsPerParticle => Var::N1,
        noise::calibration::Variable::CrossingsTimesLifetimeSpread => Var::N1Cv2,
    }
}

/// The code's walls for a split.
fn walls(s: Split) -> Walls {
    match s {
        Split::UniformLambert => Walls::UniformLambert,
        Split::Lambert => Walls::Lambert,
        Split::All | Split::Other => Walls::Other,
    }
}

/// The code's numbers for a method, quantity and split.
fn code_numbers(m: Method, i: usize, s: Split) -> Numbers {
    let e = noise::calibration::entry(m, i, walls(s));
    Numbers {
        k: e.factor,
        kappa: e.kappa,
        min_particles: f64::from(e.min_particles),
        max_n: e.max_crossings_per_particle,
    }
}

/// F1 (`PREREGISTER.txt`, round 3): energetic T20 in Lambert bands with unequal absorption failed
/// V3-E8 at its fitted factor, so it ships the other bands' factor and correction, in its own
/// domain and with its own margin.
fn fell_back(name: &str, q: &str, s: Split) -> bool {
    name == "energetic" && q == "t20_s" && s == Split::Lambert
}

/// Every method, quantity and split whose code numbers differ from what round 3's rules derive
/// from the receipt, with both.
fn round3_mismatches(r: &Value) -> Vec<String> {
    let mut out = Vec::new();
    let all: Vec<&str> = r["cells"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["cell"]["id"].as_str().unwrap())
        .collect();
    for name in METHODS {
        let m = method(name);
        let cal = calibration::calibration3_cells(r, name);
        let (var, gms) = calibration::choose_var(r, name);
        println!("{name}: variable {} {gms:?}", calibration::var_name(var));
        if var != code_var(m) {
            out.push(format!("{name} variable {var:?} vs {:?}", code_var(m)));
        }
        for (i, q) in calibration::QUANTITIES.iter().enumerate() {
            // R3-6 over every pair of the receipt: the validation pairs can only withdraw.
            let (confirmed, _, unsafe_) = calibration::root_n3_confirmed(r, name, q, &all);
            for &s in calibration::splits(name, q) {
                let fit = calibration::fit(&cal, q, var, s)
                    .unwrap_or_else(|| panic!("{name} {q} {s:?}: no fit"));
                let dom = calibration::domain(&cal, q, var, s).unwrap();
                let (margin, _) = calibration::margin3(&cal, q, s).unwrap();
                let (k, kappa) = if fell_back(name, q, s) {
                    let other = calibration::fit(&cal, q, var, Split::Other).unwrap();
                    (other.k, other.kappa)
                } else {
                    (fit.k, fit.kappa)
                };
                let e = noise::calibration::entry(m, i, walls(s));
                let tag = format!("{name} {q} {}", calibration::split_name(s));
                println!(
                    "{tag}: k {k} kappa {kappa} (by {}), N >= {}, n <= {}, margin {margin}, \
                     1/sqrt(N) {confirmed} {unsafe_:?}; code {e:?}",
                    fit.by, dom.min_particles, dom.max_n
                );
                let close = |a: f64, b: f64| (a - b).abs() <= 1e-12 * a.abs().max(b.abs());
                if !close(k, e.factor) {
                    out.push(format!("{tag} factor {k} vs {}", e.factor));
                }
                if !close(kappa, e.kappa) {
                    out.push(format!("{tag} kappa {kappa} vs {}", e.kappa));
                }
                if dom.min_particles != f64::from(e.min_particles) {
                    out.push(format!(
                        "{tag} min_particles {} vs {}",
                        dom.min_particles, e.min_particles
                    ));
                }
                if !close(dom.max_n, e.max_crossings_per_particle) {
                    out.push(format!(
                        "{tag} max_n {} vs {}",
                        dom.max_n, e.max_crossings_per_particle
                    ));
                }
                if !close(margin, e.margin) {
                    out.push(format!("{tag} margin {margin} vs {}", e.margin));
                }
                if confirmed != e.root_n_confirmed {
                    out.push(format!(
                        "{tag} 1/sqrt(N) {confirmed} vs {}",
                        e.root_n_confirmed
                    ));
                }
                if s == Split::UniformLambert {
                    // The uniform split's absorption bound: the receipt keeps six digits, the
                    // code the f32 SPPS reads.
                    let a = noise::calibration::UNIFORM_LAMBERT_MAX_MEAN_ABSORPTION;
                    if (dom.max_mean_absorption - a).abs() > 1e-6 {
                        out.push(format!(
                            "{tag} mean absorption {} vs {a}",
                            dom.max_mean_absorption
                        ));
                    }
                }
            }
        }
    }
    out
}

#[test]
fn the_codes_round_three_numbers_are_the_rules_on_the_receipt() {
    let r = receipt();
    assert_eq!(round3_mismatches(&r), Vec::<String>::new());
    // Says no through the code: every factor moved by 10 %: 8 random, 8 energetic, and energetic
    // T20 and T30's two other kinds of wall.
    let m = faults::with(Fault::NoiseCalibrationScaled { by: 1.1 }, || {
        round3_mismatches(&r)
    });
    assert_eq!(
        m.iter().filter(|x| x.contains(" factor ")).count(),
        20,
        "{m:?}"
    );
    // Says no through the input: C3-R3's T30 spread 1.5 times what it was moves random T30's
    // factor (it sets it).
    let mut r2 = receipt();
    for row in cell_mut(&mut r2, "C3-R3")["quantities"]["t30_s"]
        .as_array_mut()
        .unwrap()
    {
        let o = row["observed_sd"].as_f64().unwrap();
        row["observed_sd"] = (1.5 * o).into();
    }
    let m = round3_mismatches(&r2);
    assert!(
        m.iter().any(|x| x.starts_with("random t30_s all factor")),
        "{m:?}"
    );
    // Says no through the input: C3-R9 read as 40,000 particles moves random T20's domain, whose
    // fewest particles it sets.
    let mut r3 = receipt();
    cell_mut(&mut r3, "C3-R9")["cell"]["particles_per_source"] = 40_000.into();
    let m = round3_mismatches(&r3);
    assert!(
        m.iter()
            .any(|x| x.starts_with("random t20_s all min_particles 40000")),
        "{m:?}"
    );
}

/// Every round-3 validation cell, method, quantity and split that fails R3-8 with `numbers`, the
/// checks made (cell and what was checked, with the rows inside the domain), and the rows outside
/// it.
struct Validation {
    fails: Vec<String>,
    checked: Vec<(String, String, usize)>,
    outside: usize,
}

fn validation3(r: &Value, numbers: impl Fn(Method, usize, Split) -> Numbers) -> Validation {
    let mut v = Validation {
        fails: Vec::new(),
        checked: Vec::new(),
        outside: 0,
    };
    for name in METHODS {
        let m = method(name);
        let var = code_var(m);
        for c in calibration::validation3_cells(r, name) {
            let id = c["cell"]["id"].as_str().unwrap().to_string();
            for (i, q) in calibration::QUANTITIES.iter().enumerate() {
                for &s in calibration::splits(name, q) {
                    let x = numbers(m, i, s);
                    let max_a = noise::calibration::UNIFORM_LAMBERT_MAX_MEAN_ABSORPTION;
                    let (p, inside, all) = calibration::validates3(c, q, var, s, x, max_a);
                    v.outside += all - inside;
                    let Some((ok, p)) = p else { continue };
                    let what = format!("{q} {}", calibration::split_name(s));
                    v.checked.push((id.clone(), what.clone(), inside));
                    if !ok {
                        v.fails.push(format!(
                            "{id} {what}: {:.3} [{:.3}, {:.3}]",
                            p.ratio, p.lower, p.upper
                        ));
                    }
                }
            }
        }
    }
    v
}

#[test]
fn every_round_three_validation_cell_is_at_or_below_its_prediction() {
    let r = receipt();
    let v = validation3(&r, code_numbers);
    println!(
        "checked {} (cell, quantity) pairs; {} rows outside the domain",
        v.checked.len(),
        v.outside
    );
    assert_eq!(v.fails, Vec::<String>::new());
    // Every V3 cell checks something.
    let mut cells: Vec<&str> = v.checked.iter().map(|(c, _, _)| c.as_str()).collect();
    cells.dedup();
    assert_eq!(cells.len(), 19, "{cells:?}");
    // Coverage (R3-8): every method, quantity and split in at least two V3 cells with six rows
    // inside the domain, but for energetic T20 and T30 outside Lambert bands, which have one
    // (V3-E10: V3-E4's larger receivers are outside the domain): a shortfall reported, not made
    // up (rounds 1 and 2 checked that split on nineteen cells).
    let mut short = Vec::new();
    for name in METHODS {
        for q in calibration::QUANTITIES {
            for &s in calibration::splits(name, q) {
                let what = format!("{q} {}", calibration::split_name(s));
                let n = v
                    .checked
                    .iter()
                    .filter(|(c, w, k)| {
                        *w == what
                            && *k >= 6
                            && c.starts_with(if name == "random" { "V3-R" } else { "V3-E" })
                    })
                    .count();
                if n < 2 {
                    short.push(format!("{name} {what}: {n}"));
                }
            }
        }
    }
    assert_eq!(
        short,
        vec![
            "energetic t20_s other: 1".to_string(),
            "energetic t30_s other: 1".to_string()
        ]
    );
    // Says no through the code: a model twice as optimistic fails in every V3 cell.
    let halved = faults::with(Fault::NoiseCalibrationScaled { by: 0.5 }, || {
        validation3(&r, code_numbers)
    });
    for c in &cells {
        assert!(
            halved.fails.iter().any(|f| f.starts_with(&format!("{c} "))),
            "{c} passes at half: {:#?}",
            halved.fails
        );
    }
}

#[test]
fn a_round_three_quantity_noisier_than_its_prediction_fails_its_check_alone() {
    // Says no through the input: in one V3 cell of each method, one quantity's observed spread
    // raised to 1.5 times its calibrated prediction in every receiver-band.
    for (name, id, q, s) in [
        ("random", "V3-R1", "t30_s", Split::All),
        ("energetic", "V3-E9", "t30_s", Split::UniformLambert),
    ] {
        let mut r = receipt();
        let m = method(name);
        let qi = calibration::QUANTITIES
            .iter()
            .position(|x| *x == q)
            .unwrap();
        let x = code_numbers(m, qi, s);
        let var = code_var(m);
        let rows = calibration::rows3(
            r["cells"]
                .as_array()
                .unwrap()
                .iter()
                .find(|c| c["cell"]["id"] == id)
                .unwrap(),
            q,
            var,
            s,
            f64::INFINITY,
        );
        for (row, r3) in cell_mut(&mut r, id)["quantities"][q]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .zip(&rows)
        {
            assert_eq!(row["freq_hz"].as_i64().unwrap(), r3.freq_hz);
            row["observed_sd"] = (1.5 * x.k * r3.predicted_var(x.kappa).sqrt()).into();
        }
        let v = validation3(&r, code_numbers);
        println!("{name}: {:?}", v.fails);
        assert_eq!(v.fails.len(), 1, "{name}: {:?}", v.fails);
        assert!(
            v.fails[0].starts_with(&format!("{id} {q}")),
            "{:?}",
            v.fails
        );
    }
}

#[test]
fn without_the_correction_for_repeated_crossings_a_large_receiver_cell_fails() {
    // Says no through the numbers: every kappa set to 0, the factors kept, and the random-mode
    // cells with large receivers claim less noise than they show.
    let r = receipt();
    let v = validation3(&r, |m, i, s| Numbers {
        kappa: 0.0,
        ..code_numbers(m, i, s)
    });
    let cal = {
        // The calibration cells too: round 3's factors were fitted with the correction.
        let mut fails = Vec::new();
        for name in METHODS {
            let m = method(name);
            for c in calibration::calibration3_cells(&r, name) {
                for (i, q) in calibration::QUANTITIES.iter().enumerate() {
                    for &s in calibration::splits(name, q) {
                        let x = Numbers {
                            kappa: 0.0,
                            ..code_numbers(m, i, s)
                        };
                        let max_a = noise::calibration::UNIFORM_LAMBERT_MAX_MEAN_ABSORPTION;
                        if let (Some((false, _)), _, _) =
                            calibration::validates3(c, q, code_var(m), s, x, max_a)
                        {
                            fails.push(format!("{} {q}", c["cell"]["id"].as_str().unwrap()));
                        }
                    }
                }
            }
        }
        fails
    };
    println!("kappa 0: validation {:?}; calibration {cal:?}", v.fails);
    assert!(
        cal.iter()
            .chain(&v.fails)
            .any(|f| f.starts_with("C3-R1 ") || f.starts_with("C3-R3 ")),
        "{cal:?} {:?}",
        v.fails
    );
}

#[test]
fn energetic_t20_in_lambert_rooms_fails_v3_e8_at_its_fitted_factor() {
    // F1: the fit for energetic T20 in Lambert bands with unequal absorption, 0.52, fails V3-E8
    // (floor 0.9, the rest 0.02, 1 ms steps); the factor 1 the code ships passes it.
    let r = receipt();
    let fitted = |m: Method, i: usize, s: Split| {
        let x = code_numbers(m, i, s);
        if m == Method::Energetic && i == 2 && s == Split::Lambert {
            Numbers { k: 0.52, ..x }
        } else {
            x
        }
    };
    let v = validation3(&r, fitted);
    assert_eq!(v.fails.len(), 1, "{:?}", v.fails);
    assert!(
        v.fails[0].starts_with("V3-E8 t20_s lambert"),
        "{:?}",
        v.fails
    );
    let cal = calibration::calibration3_cells(&r, "energetic");
    let fit = calibration::fit(&cal, "t20_s", Var::N1Cv2, Split::Lambert).unwrap();
    assert_eq!(fit.k, 0.52);
}

#[test]
fn the_receipt_is_the_pre_registered_design() {
    // 70 cells over ten seeds each, their roles as pre-registered: round 1's calibration and
    // validation cells, round 2's validation cells, and round 3's calibration and validation
    // cells; every one of their receiver-bands with its round-3 fields.
    let r = receipt();
    let all = r["cells"].as_array().unwrap();
    assert_eq!(all.len(), 70);
    let pre = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../docs/investigations/2026-09-25-noise-calibration/PREREGISTER.txt"),
    )
    .unwrap();
    for c in all {
        let id = c["cell"]["id"].as_str().unwrap();
        assert_eq!(calibration::seeds(c), 10, "{id}");
        let role = match id.split('-').next().unwrap() {
            "C" => "calibration",
            "V" => "validation",
            "W" => "validation2",
            "C3" => "calibration3",
            "V3" => "validation3",
            other => panic!("{other}"),
        };
        assert_eq!(c["cell"]["role"], role, "{id}");
        assert!(
            pre.contains(&format!(" {id} ")),
            "{id} is not pre-registered"
        );
        assert_eq!(c["bands"].as_array().unwrap().len(), 36, "{id}");
        // Every receiver-band of every quantity is in the receipt: complete, or kept apart with
        // each seed's value or null.
        for q in calibration::QUANTITIES {
            let n = c["quantities"][q].as_array().unwrap().len()
                + c["incomplete"][q].as_array().unwrap().len();
            assert_eq!(n, 36, "{id} {q}");
        }
    }
}

// --- rounds 1 and 2, as history --------------------------------------------------------------

/// One method's numbers as rounds 1 and 2 shipped them: its name, then factors, margins and
/// `1/√N` flags in `QUANTITIES`' order.
type Shipped = (&'static str, [f64; 8], [f64; 8], [bool; 8]);

/// What rounds 1 and 2 shipped (`7132e42`).
const R12: [Shipped; 2] = [
    (
        "random",
        [1.3, 1.4, 1.4, 1.6, 1.2, 1.2, 1.2, 1.4],
        [1.1, 1.1, 1.4, 1.5, 1.1, 1.1, 1.1, 1.1],
        [true, true, true, false, true, true, true, true],
    ),
    (
        "energetic",
        [0.91, 0.59, 1.0, 1.0, 0.85, 0.78, 0.85, 0.62],
        [1.1, 1.1, 1.2, 1.3, 1.1, 1.1, 1.1, 1.1],
        [true, false, true, true, true, true, true, true],
    ),
];

/// Every quantity whose round-1/2 number differs from what rules 2, 5b and 4 (and R2-4) derive
/// from the receipt, with both.
fn r12_mismatches(r: &Value) -> Vec<String> {
    let mut out = Vec::new();
    for (name, factors, margins, flags) in R12 {
        for (i, q) in calibration::QUANTITIES.iter().enumerate() {
            let cal = calibration::cells_in(r, name, calibration::calibration_roles(name, q));
            let (factor, _) = calibration::shipped_factor(r, name, q);
            let (margin, _) = calibration::margin(&cal, q).unwrap();
            let (confirmed, _) = calibration::root_n_confirmed(r, name, q);
            if (factor - factors[i]).abs() > 1e-12 * factor {
                out.push(format!("{name} {q} factor {factor} vs {}", factors[i]));
            }
            if (margin - margins[i]).abs() > 1e-12 * margin {
                out.push(format!("{name} {q} margin {margin} vs {}", margins[i]));
            }
            if confirmed != flags[i] {
                out.push(format!("{name} {q} 1/sqrt(N) {confirmed} vs {}", flags[i]));
            }
        }
    }
    out
}

#[test]
fn rounds_one_and_two_numbers_are_their_rules_on_the_receipt() {
    let r = receipt();
    assert_eq!(r12_mismatches(&r), Vec::<String>::new());
    // Says no through the input: with W-E4's T30 spread a third of what it was, round 2's T30
    // factor passes its validation and would have shipped.
    let mut r4 = receipt();
    for row in cell_mut(&mut r4, "W-E4")["quantities"]["t30_s"]
        .as_array_mut()
        .unwrap()
    {
        let o = row["observed_sd"].as_f64().unwrap();
        row["observed_sd"] = (o / 3.0).into();
    }
    assert_eq!(
        r12_mismatches(&r4),
        vec!["energetic t30_s factor 0.31 vs 1".to_string()]
    );
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
    let m = r12_mismatches(&r2);
    assert_eq!(m.len(), 1, "{m:?}");
    assert!(m[0].starts_with("random t30_s margin"), "{m:?}");
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
    // structure, factor 1, holds on every energetic cell of rounds 1 and 2.
    let r = receipt();
    let f1 = energetic_decay_failures(&r, &["validation"], 0.26, 0.15);
    assert_eq!(f1.len(), 2, "{f1:?}");
    assert!(f1.iter().all(|f| f.starts_with("V-E6 ")), "{f1:?}");
    let f2 = energetic_decay_failures(&r, &["validation2"], 0.43, 0.31);
    assert_eq!(f2.len(), 2, "{f2:?}");
    assert!(f2.iter().all(|f| f.starts_with("W-E4 ")), "{f2:?}");
    let all = ["calibration", "validation", "validation2"];
    assert_eq!(
        energetic_decay_failures(&r, &all, 1.0, 1.0),
        Vec::<String>::new()
    );
    // Says no: half M7's structure fails W-E4's T30.
    let half = energetic_decay_failures(&r, &all, 0.5, 0.5);
    assert!(half.iter().any(|f| f.starts_with("W-E4 t30_s")), "{half:?}");
}
