//! The Monte-Carlo noise model's calibration against SPPS's own seed-to-seed spread, checked on the
//! committed receipt (`docs/investigations/2026-09-25-noise-calibration/calibration.json`; the
//! rules are pre-registered in `PREREGISTER.txt` beside it, and the numbers come from real SPPS
//! runs, `crates/simpa/tests/noise_calibration.rs`, run on purpose).
//!
//! Round 4 (what the code holds for energetic T20 and T30 outside uniform Lambert rooms, and the
//! resample counts), each check saying no:
//! - the roughness entries are what rules R4-1 and R4-2 derive from the cells of rounds 1 to 3, and
//!   every `1/√N` flag is R3-6 over every pair, round 4's included; the factors moved through the
//!   code, or the cell that sets T30's factor read noisier, and they no longer are;
//! - on every V4 cell, method, quantity and split, the calibrated prediction is at or above the
//!   seeds' spread within its uncertainty (R4-4); a model twice too optimistic fails every V4 cell,
//!   and one quantity's spread raised above its prediction fails that check alone;
//! - the uniform-Lambert entries fail two long rooms at a mean absorption of 0.4, so their bound
//!   is 0.2 (F6);
//! - on every pair a count the higher count reaches brings the value through there (R4-5), from
//!   the standard deviation and from the resamples, and the counts that fell short are withdrawn
//!   (F8); the higher count's judgements read from the lower count fail it.
//!
//! Round 3 (what the code still holds from it), each check saying no:
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
        Split::All | Split::Other | Split::NotUniformLambert => Walls::Other,
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

/// What round 3 shipped (`50695f6`) for energetic T20 and T30 in the bands round 4 gives the
/// roughness structure (Lambert with unequal absorption, and not Lambert), which the code no
/// longer holds: `(quantity index, split, k, κ, fewest particles, largest n, margin)`, each flag of
/// `1/√N` on.
const R3_REPLACED: [(usize, Split, f64, f64, f64, f64, f64); 4] = [
    (2, Split::Lambert, 1.0, 0.0, 150_000.0, 0.0319601939142, 1.3),
    (
        3,
        Split::Lambert,
        0.73,
        2.0,
        150_000.0,
        0.0319601939142,
        1.3,
    ),
    (2, Split::Other, 1.0, 0.0, 15_000.0, 0.81943475966, 1.2),
    (3, Split::Other, 1.0, 0.0, 150_000.0, 0.81943475966, 1.3),
];

/// Round 3's numbers for a method, quantity and split: the code's, or what round 3 shipped where
/// round 4 replaced it ([`R3_REPLACED`]).
fn r3_numbers(m: Method, i: usize, s: Split) -> Numbers {
    if m == Method::Energetic
        && let Some(&(_, _, k, kappa, min_particles, max_n, _)) =
            R3_REPLACED.iter().find(|x| x.0 == i && x.1 == s)
    {
        return Numbers {
            k,
            kappa,
            min_particles,
            max_n,
        };
    }
    code_numbers(m, i, s)
}

/// Round 3's entry, as [`r3_numbers`], with its margin and `1/√N` flag.
fn r3_entry(m: Method, i: usize, s: Split) -> noise::calibration::Entry {
    let e = noise::calibration::entry(m, i, walls(s));
    match R3_REPLACED.iter().find(|x| x.0 == i && x.1 == s) {
        Some(&(_, _, factor, kappa, min_particles, max_n, margin)) if m == Method::Energetic => {
            noise::calibration::Entry {
                factor,
                kappa,
                min_particles: min_particles as u32,
                max_crossings_per_particle: max_n,
                margin,
                root_n_confirmed: true,
                structure: noise::Structure::Constant,
                resampled_confirmed: true,
            }
        }
        _ => e,
    }
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
            // R3-6 over every pair of the receipt, round 4's included: a pair can only withdraw.
            let (confirmed, _, unsafe_) =
                calibration::root_n_confirmed_over(r, name, q, &all, &calibration::PAIRS4);
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
                let e = r3_entry(m, i, s);
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
                    // The uniform split's absorption bound: its calibration rows' largest (A2),
                    // or 0.2 where its entries fail a V4 cell above it (F6, round 4). The receipt
                    // keeps six digits, the code the f32 SPPS reads.
                    let want = if f6_failures(r).is_empty() {
                        dom.max_mean_absorption
                    } else {
                        0.2
                    };
                    let a = noise::calibration::UNIFORM_LAMBERT_MAX_MEAN_ABSORPTION;
                    if (want - a).abs() > 1e-6 {
                        out.push(format!("{tag} mean absorption {want} vs {a}"));
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
    // Says no through the code: every factor the code still holds from round 3 moved by 10 %: 8
    // random, and 8 energetic with T20 and T30 in uniform Lambert bands (their other bands are
    // round 4's now, `R3_REPLACED`).
    let m = faults::with(Fault::NoiseCalibrationScaled { by: 1.1 }, || {
        round3_mismatches(&r)
    });
    assert_eq!(
        m.iter().filter(|x| x.contains(" factor ")).count(),
        16,
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
    let v = validation3(&r, r3_numbers);
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
        validation3(&r, r3_numbers)
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
        let x = r3_numbers(m, qi, s);
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
        let v = validation3(&r, r3_numbers);
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
        ..r3_numbers(m, i, s)
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
                            ..r3_numbers(m, i, s)
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
        let x = r3_numbers(m, i, s);
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
    // 94 cells over ten seeds each, their roles as pre-registered: round 1's calibration and
    // validation cells, round 2's validation cells, round 3's calibration and validation cells and
    // round 4's; every one of their receiver-bands with its round-3 fields, and for every cell of a
    // pair how the report judged each receiver-band seed (R4-5).
    let r = receipt();
    let all = r["cells"].as_array().unwrap();
    assert_eq!(all.len(), 94);
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
            "V4" => "validation4",
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
            let paired = calibration::PAIRS4
                .iter()
                .any(|(a, b)| *a == id || *b == id);
            if paired {
                assert_eq!(c["judged"][q].as_array().unwrap().len(), 360, "{id} {q}");
            }
        }
    }
}

// --- round 4 -------------------------------------------------------------------------------------

/// F6's test: the V4 cells at a mean absorption of 0.3 to 0.4 on which the uniform-Lambert entries,
/// held to round 3's bound of 0.4, fail (R4-4), as `cell quantity`.
fn f6_failures(r: &Value) -> Vec<String> {
    let mut out = Vec::new();
    for c in calibration::validation4_cells(r, "energetic") {
        for (i, q) in [(2, "t20_s"), (3, "t30_s")] {
            let x = code_numbers(Method::Energetic, i, Split::UniformLambert);
            let rows: Vec<calibration::Row3> = calibration::rows_with(
                c,
                q,
                Var::N1Cv2,
                Split::UniformLambert,
                x.max_n,
                calibration::CONSTANT_SD,
            )
            .into_iter()
            .filter(|row| row.mean_absorption > 0.25 && row.mean_absorption < 0.41)
            .collect();
            if c["cell"]["particles_per_source"].as_f64().unwrap() < x.min_particles {
                continue;
            }
            if let Some(p) = calibration::pooled3(&rows, x.k, x.kappa, calibration::seeds(c))
                && p.lower > 1.0
            {
                out.push(format!("{} {q}", c["cell"]["id"].as_str().unwrap()));
            }
        }
    }
    out
}

#[test]
fn f6_raises_energetic_t20s_roughness_factor_to_cover_the_room_it_fails() {
    // Once F6 sends uniform Lambert bands above 0.2 to the roughness entries, the 5 x 4 x 3 m room at
    // alpha 0.4 with 1 ms steps (C-E3) claims less noise than it shows for T20 at the fitted 1.2, so
    // T20 ships 1.4 (its upper bound, rounded up); T30 needs no cover.
    let r = receipt();
    let cal = calibration::calibration4_cells(&r, "energetic");
    let s = Split::NotUniformLambert;
    let key = calibration::ROUGH_SD;
    let mut got = Vec::new();
    for q in ["t20_s", "t30_s"] {
        let fit =
            calibration::fit_with(&cal, q, Var::N1Cv2, s, &calibration::kappas4(), key).unwrap();
        let dom = calibration::domain_with(&cal, q, Var::N1Cv2, s, key).unwrap();
        got.push((q, fit.k, f6_cover(&r, q, fit.k, fit.kappa, dom)));
    }
    assert_eq!(
        got,
        [
            ("t20_s", 1.2, Some((1.4, "C-E3".to_string()))),
            ("t30_s", 1.3, None)
        ]
    );
}

#[test]
fn the_uniform_lambert_entries_fail_long_rooms_at_0_4_so_their_bound_is_0_2() {
    // F6: at round 3's bound (0.4) the uniform-Lambert entries (0.098 and 0.052 of M7's structure)
    // fail two held-out long rooms, V4-E1 (20 x 4 x 3 m, 1 ms) and V4-E4 (30 x 4 x 3 m); the cube and
    // the hall at 0.3 and 0.35 pass. So their bound is 0.2, and those bands take the roughness.
    let r = receipt();
    assert_eq!(
        f6_failures(&r),
        ["V4-E1 t30_s", "V4-E4 t20_s", "V4-E4 t30_s"]
    );
    let a = noise::calibration::UNIFORM_LAMBERT_MAX_MEAN_ABSORPTION;
    assert!((a - 0.2).abs() < 1e-6, "{a}");
}

/// The V4 cells in which round 4's validation checks something (R4-4).
const ROUND4_CELLS_CHECKED: usize = 24;
/// Round 4's coverage shortfalls (R4-4): method, quantity and split with fewer than two V4 cells of
/// six rows inside the domain, reported and not made up. After F6 the uniform-Lambert entries hold up to a mean absorption of 0.2, and one V4 cell has six
/// rows there (V4-E17); round 3 checked them on V3-E1, V3-E5 and V3-E9.
const ROUND4_SHORTFALL: [&str; 2] = [
    "energetic t20_s uniform_lambert: 1",
    "energetic t30_s uniform_lambert: 1",
];
/// A V4 cell whose energetic T30 is judged under the roughness structure with six rows or more.
const ROUND4_ROUGH_CELL: &str = "V4-E5";

/// The code's numbers for a method, quantity and round-4 split, with the receipt's key of the
/// structure the code draws that quantity's resamples with.
fn code4(m: Method, i: usize, s: Split) -> (Numbers, &'static str) {
    let e = noise::calibration::entry(m, i, walls(s));
    let key = match e.structure {
        noise::Structure::Constant => calibration::CONSTANT_SD,
        noise::Structure::Roughness => calibration::ROUGH_SD,
    };
    (code_numbers(m, i, s), key)
}

/// Every cell id of the receipt.
fn ids(r: &Value) -> Vec<&str> {
    r["cells"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["cell"]["id"].as_str().unwrap())
        .collect()
}

/// F6's consequence on the cells of rounds 1 to 3: their uniform Lambert rows above a mean absorption
/// of 0.2, which F6 sends to the roughness entries. A cell whose rows there claim less noise than
/// they show at the fitted `k` (lower bound above 1) raises `k` to cover its upper bound (F3's
/// remedy, applied after round 4's validation): the raised `k` and the cell, or `None`.
fn f6_cover(
    r: &Value,
    q: &str,
    k: f64,
    kappa: f64,
    dom: calibration::Domain,
) -> Option<(f64, String)> {
    let mut best: Option<(f64, String)> = None;
    for c in calibration::calibration4_cells(r, "energetic") {
        if c["cell"]["particles_per_source"].as_f64().unwrap() < dom.min_particles {
            continue;
        }
        let rows: Vec<calibration::Row3> = calibration::rows_with(
            c,
            q,
            Var::N1Cv2,
            Split::UniformLambert,
            dom.max_n,
            calibration::ROUGH_SD,
        )
        .into_iter()
        .filter(|row| row.mean_absorption > 0.2000001)
        .collect();
        let Some(p) = calibration::pooled3(&rows, k, kappa, calibration::seeds(c)) else {
            continue;
        };
        if p.lower > 1.0 {
            let cover = calibration::round_up_two_digits(p.upper * k);
            if best.as_ref().is_none_or(|(b, _)| cover > *b) {
                best = Some((cover, c["cell"]["id"].as_str().unwrap().to_string()));
            }
        }
    }
    best
}

/// Every number of energetic T20 and T30 in bands not uniform Lambert that differs from what rules
/// R4-1 and R4-2 derive from the receipt's cells of rounds 1 to 3 (with F6's cover, [`f6_cover`]) (both kinds of such band take
/// the same entry, under the roughness structure), and every `1/√N` flag of the code that differs
/// from R3-6 over every pair of the receipt, round 4's included (F9), with both.
fn round4_mismatches(r: &Value) -> Vec<String> {
    let mut out = Vec::new();
    let all = ids(r);
    let cal = calibration::calibration4_cells(r, "energetic");
    let s = Split::NotUniformLambert;
    let key = calibration::ROUGH_SD;
    let close = |a: f64, b: f64| (a - b).abs() <= 1e-12 * a.abs().max(b.abs());
    for (i, q) in [(2, "t20_s"), (3, "t30_s")] {
        let fit = calibration::fit_with(&cal, q, Var::N1Cv2, s, &calibration::kappas4(), key)
            .unwrap_or_else(|| panic!("{q}: no fit"));
        let dom = calibration::domain_with(&cal, q, Var::N1Cv2, s, key).unwrap();
        let (margin, _) = calibration::margin_with(&cal, q, s, key).unwrap();
        let (confirmed, _, _) =
            calibration::root_n_confirmed_over(r, "energetic", q, &all, &calibration::PAIRS4);
        let cover = f6_cover(r, q, fit.k, fit.kappa, dom);
        let k = cover.as_ref().map_or(fit.k, |(c, _)| c.max(fit.k));
        println!(
            "energetic {q} not uniform Lambert: k {} kappa {} (by {}), N >= {}, n <= {}, margin \
             {margin}, 1/sqrt(N) {confirmed}; F6 cover {cover:?}",
            fit.k, fit.kappa, fit.by, dom.min_particles, dom.max_n
        );
        for w in [Walls::Other, Walls::Lambert] {
            let e = noise::calibration::entry(Method::Energetic, i, w);
            let tag = format!("energetic {q} {w:?}");
            if e.structure != noise::Structure::Roughness {
                out.push(format!("{tag} structure {:?}", e.structure));
            }
            if !close(k, e.factor) {
                out.push(format!("{tag} factor {k} vs {}", e.factor));
            }
            if !close(fit.kappa, e.kappa) {
                out.push(format!("{tag} kappa {} vs {}", fit.kappa, e.kappa));
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
        }
    }
    for name in METHODS {
        let m = method(name);
        for (i, q) in calibration::QUANTITIES.iter().enumerate() {
            let (confirmed, _, _) =
                calibration::root_n_confirmed_over(r, name, q, &all, &calibration::PAIRS4);
            for w in [Walls::Other, Walls::Lambert, Walls::UniformLambert] {
                let e = noise::calibration::entry(m, i, w);
                if confirmed != e.root_n_confirmed {
                    out.push(format!(
                        "{name} {q} {w:?} 1/sqrt(N) {confirmed} vs {}",
                        e.root_n_confirmed
                    ));
                }
            }
        }
    }
    out
}

#[test]
fn the_codes_round_four_numbers_are_the_rules_on_the_receipt() {
    let r = receipt();
    assert_eq!(round4_mismatches(&r), Vec::<String>::new());
    // Says no through the code: the factors moved by 10 % move both kinds of band of both
    // quantities.
    let m = faults::with(Fault::NoiseCalibrationScaled { by: 1.1 }, || {
        round4_mismatches(&r)
    });
    assert_eq!(
        m.iter().filter(|x| x.contains(" factor ")).count(),
        4,
        "{m:?}"
    );
    // Says no through the input: the cell that sets energetic T30's factor read with its spread
    // 1.5 times what it was moves it.
    let cal = calibration::calibration4_cells(&r, "energetic");
    let by = calibration::fit_with(
        &cal,
        "t30_s",
        Var::N1Cv2,
        Split::NotUniformLambert,
        &calibration::kappas4(),
        calibration::ROUGH_SD,
    )
    .unwrap()
    .by;
    let mut r2 = receipt();
    for row in cell_mut(&mut r2, &by)["quantities"]["t30_s"]
        .as_array_mut()
        .unwrap()
    {
        let o = row["observed_sd"].as_f64().unwrap();
        row["observed_sd"] = (1.5 * o).into();
    }
    let m = round4_mismatches(&r2);
    assert!(
        m.iter()
            .any(|x| x.starts_with("energetic t30_s Other factor")),
        "{m:?}"
    );
}

fn validation4(
    r: &Value,
    numbers: impl Fn(Method, usize, Split) -> (Numbers, &'static str),
) -> Validation {
    let mut v = Validation {
        fails: Vec::new(),
        checked: Vec::new(),
        outside: 0,
    };
    for name in METHODS {
        let m = method(name);
        let var = code_var(m);
        for c in calibration::validation4_cells(r, name) {
            let id = c["cell"]["id"].as_str().unwrap().to_string();
            for (i, q) in calibration::QUANTITIES.iter().enumerate() {
                for (s, _) in calibration::splits4(name, q) {
                    let (x, key) = numbers(m, i, s);
                    let max_a = noise::calibration::UNIFORM_LAMBERT_MAX_MEAN_ABSORPTION;
                    let (p, inside, all) = calibration::validates4(c, q, var, s, x, max_a, key);
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
fn every_round_four_validation_cell_is_at_or_below_its_prediction() {
    let r = receipt();
    let v = validation4(&r, code4);
    println!(
        "checked {} (cell, quantity) pairs; {} rows outside the domain",
        v.checked.len(),
        v.outside
    );
    assert_eq!(v.fails, Vec::<String>::new());
    let mut cells: Vec<&str> = v.checked.iter().map(|(c, _, _)| c.as_str()).collect();
    cells.dedup();
    assert_eq!(cells.len(), ROUND4_CELLS_CHECKED, "{cells:?}");
    // Coverage (R4-4): every method, quantity and split in at least two V4 cells with six rows
    // inside the domain, but for those listed, reported and not made up.
    let mut short = Vec::new();
    for name in METHODS {
        for q in calibration::QUANTITIES {
            for (s, _) in calibration::splits4(name, q) {
                let what = format!("{q} {}", calibration::split_name(s));
                let n = v
                    .checked
                    .iter()
                    .filter(|(c, w, k)| {
                        *w == what
                            && *k >= 6
                            && c.starts_with(if name == "random" { "V4-R" } else { "V4-E" })
                    })
                    .count();
                if n < 2 {
                    short.push(format!("{name} {what}: {n}"));
                }
            }
        }
    }
    assert_eq!(short, ROUND4_SHORTFALL.to_vec());
    // The uniform-Lambert rooms above a mean absorption of 0.2 in rooms other than 5 x 4 x 3 and
    // 6 x 10 x 3 m (R4-4: V4-E1 to V4-E4) are judged, since F6, by the other bands' entries: six
    // rows each.
    for id in ["V4-E1", "V4-E2", "V4-E3", "V4-E4"] {
        for q in ["t20_s", "t30_s"] {
            let what = format!("{q} not_uniform_lambert");
            assert!(
                v.checked
                    .iter()
                    .any(|(c, w, k)| c == id && *w == what && *k >= 6),
                "{id} {what}"
            );
        }
    }
    // Says no through the code: a model twice as optimistic fails in every V4 cell.
    let halved = faults::with(Fault::NoiseCalibrationScaled { by: 0.5 }, || {
        validation4(&r, code4)
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
fn a_round_four_quantity_noisier_than_its_prediction_fails_its_check_alone() {
    // Says no through the input: in one V4 cell, one quantity's observed spread raised to 1.5
    // times its calibrated prediction in every receiver-band: energetic T30 under the roughness
    // structure, and under the uniform-Lambert entries (a mean absorption of 0.2).
    for (id, q, s) in [
        (ROUND4_ROUGH_CELL, "t30_s", Split::NotUniformLambert),
        ("V4-E17", "t30_s", Split::UniformLambert),
    ] {
        let mut r = receipt();
        let m = Method::Energetic;
        let qi = calibration::QUANTITIES
            .iter()
            .position(|x| *x == q)
            .unwrap();
        let (x, key) = code4(m, qi, s);
        let cell = r["cells"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["cell"]["id"] == id)
            .unwrap()
            .clone();
        let rows = calibration::rows_with(&cell, q, code_var(m), s, f64::INFINITY, key);
        assert!(rows.len() >= 6, "{id} {q}: {} rows", rows.len());
        for row in cell_mut(&mut r, id)["quantities"][q]
            .as_array_mut()
            .unwrap()
        {
            if let Some(r3) = rows.iter().find(|x| {
                x.receiver == row["receiver"].as_u64().unwrap()
                    && x.freq_hz == row["freq_hz"].as_i64().unwrap()
            }) {
                row["observed_sd"] = (1.5 * x.k * r3.predicted_var(x.kappa).sqrt()).into();
            }
        }
        let v = validation4(&r, code4);
        println!("{id} {q}: {:?}", v.fails);
        assert_eq!(v.fails.len(), 1, "{id}: {:?}", v.fails);
        assert!(
            v.fails[0].starts_with(&format!("{id} {q}")),
            "{:?}",
            v.fails
        );
    }
}

/// R4-5 on the receipt: per pair, quantity and kind of count (`n` from the standard deviation,
/// `r` from the resamples), the lower count's refusals whose named count the higher count reaches,
/// and there the mean share of the higher count's seeds that give the value (as registered) and
/// that are not refused for their noise (given, or refused for another reason, which no particle
/// count cures). `swap` reads the higher count's judgements from the lower count's cell (the
/// say-NO).
fn named_counts(r: &Value, swap: bool) -> Vec<(String, usize, f64, f64)> {
    let find = |id: &str| {
        r["cells"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["cell"]["id"] == id)
    };
    let noise = |c: &str| c.starts_with('n') || c.starts_with('r') || c == "b" || c == "x";
    let mut out = Vec::new();
    for (a, b) in calibration::PAIRS4 {
        let (Some(ca), Some(cb)) = (find(a), find(b)) else {
            continue;
        };
        let high = cb["cell"]["particles_per_source"].as_u64().unwrap();
        let seeds = calibration::seeds(ca);
        for q in calibration::QUANTITIES {
            let codes = |c: &Value| -> Vec<String> {
                c["judged"][q]
                    .as_array()
                    .unwrap_or_else(|| panic!("{}: no judgements", c["cell"]["id"]))
                    .iter()
                    .map(|x| x.as_str().unwrap().to_string())
                    .collect()
            };
            let lo = codes(ca);
            let hi = codes(if swap { ca } else { cb });
            for kind in ['n', 'r'] {
                let (mut within, mut given, mut quiet) = (0usize, 0.0, 0.0);
                for (k, code) in lo.iter().enumerate() {
                    let Some(n) = code.strip_prefix(kind).and_then(|n| n.parse::<u64>().ok())
                    else {
                        continue;
                    };
                    if n > high {
                        continue;
                    }
                    within += 1;
                    let rb = k / seeds;
                    let at = &hi[rb * seeds..(rb + 1) * seeds];
                    given += at.iter().filter(|c| *c == "g").count() as f64 / seeds as f64;
                    quiet += at.iter().filter(|c| !noise(c)).count() as f64 / seeds as f64;
                }
                if within > 0 {
                    out.push((
                        format!("{a}/{b} {q} {kind}"),
                        within,
                        given / within as f64,
                        quiet / within as f64,
                    ));
                }
            }
        }
    }
    out
}

/// The pairs, quantities and kinds of count where fewer than 80 % of the higher count's seeds give
/// the value (R4-5 as registered) although at least 80 % are not refused for their noise: the
/// higher count refuses them for their tail or their missing energy, which no particle count cures.
const R4_5_NOT_NOISE: [&str; 2] = ["W-E3/V4-E15 edt_s n", "C-R7/V4-R5 edt_s n"];

#[test]
fn a_named_count_brings_the_value_through_at_the_higher_count() {
    // R4-5: on every pair with at least 10 refusals whose named count the higher count reaches, at
    // least 80 % of the higher count's seeds are not refused for their noise there, for counts
    // named from the standard deviation and from the resamples alike (counts under the roughness
    // structure named from M7's structure; energetic C50, C80, D50 and Ts naming none from their
    // resamples, F8).
    let r = receipt();
    let got = named_counts(&r, false);
    for (what, within, given, quiet) in &got {
        println!(
            "{what}: {within} within reach; there {:.1} % given, {:.1} % not refused for noise",
            100.0 * given,
            100.0 * quiet
        );
    }
    let short: Vec<&(String, usize, f64, f64)> = got
        .iter()
        .filter(|(_, within, _, quiet)| *within >= 10 && *quiet < 0.8)
        .collect();
    assert!(short.is_empty(), "{short:?}");
    let registered: Vec<&str> = got
        .iter()
        .filter(|(_, within, given, _)| *within >= 10 && *given < 0.8)
        .map(|(w, ..)| w.as_str())
        .collect();
    assert_eq!(registered, R4_5_NOT_NOISE);
    // Both kinds of count are exercised at a higher count.
    for kind in [" n", " r"] {
        assert!(
            got.iter().any(|(w, n, ..)| w.ends_with(kind) && *n >= 10),
            "{kind}: {got:?}"
        );
    }
    // Says no through the input: the higher count's judgements read from the lower count, where
    // those values were refused for their noise, fail the target wherever the refusals are many
    // (with few, a receiver-band's other seeds at the lower count can pass).
    let swapped = named_counts(&r, true);
    let dense: Vec<&(String, usize, f64, f64)> = swapped
        .iter()
        .filter(|(_, within, ..)| *within >= 50)
        .collect();
    assert!(dense.len() >= 10, "{swapped:?}");
    assert!(dense.iter().all(|(.., quiet)| *quiet < 0.8), "{dense:?}");
}

#[test]
fn the_counts_the_resamples_name_are_withdrawn_where_they_fell_short() {
    // F8 (`validation-round4.txt`): on V-E2 against V4-E16 the counts energetic C50, C80, D50 and Ts
    // took from their resamples gave the value in 64.7 % of the higher count's seeds (17 within
    // reach), so they name none; every other quantity names them.
    for (m, name) in [(Method::Random, "random"), (Method::Energetic, "energetic")] {
        for (i, q) in calibration::QUANTITIES.iter().enumerate() {
            let want =
                !(m == Method::Energetic && matches!(*q, "c50_db" | "c80_db" | "d50" | "ts_s"));
            for w in [Walls::Other, Walls::Lambert, Walls::UniformLambert] {
                let e = noise::calibration::entry(m, i, w);
                assert_eq!(e.resampled_confirmed, want, "{name} {q} {w:?}");
            }
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

/// What `docs/params.md` ("Monte-Carlo noise") gives as tutorial 1's reverberation times, each
/// with its source: Sabine's 0.67 s and Eyring's 0.60 s from the room (`params::room`, with SPPS's
/// `K`), and SPPS's own T30 in that specular box from the receipt (not a converged value: the
/// cells differ by up to 2.6 % at 125 Hz, `docs/params.md`), the mean over six receivers of each
/// one's mean over ten seeds: C-R6 (random, 1.5 M, `dt` 1 ms) 0.98 s at 125 Hz to 0.79 s at
/// 4 kHz, V4-R2 (random, 6 M) and V4-E14 (energetic, 1.2 M) 0.95 s to 0.78 s. Says no:
/// Sabine's 0.67 s is not SPPS's time, which lies above it by more than 15 % in every band of
/// every cell, so the first version's "the room's time is 0.67 s" fails.
#[test]
fn tutorial_ones_t30_is_spps_own_not_sabines() {
    use simpa_core::params::room::{RtConstant, Surface, eyring_rt, sabine_rt};
    let k = RtConstant::Physical {
        speed_of_sound: 343.2,
    };
    let walls = [Surface {
        area_m2: 216.0,
        absorption: 0.2,
    }];
    let sabine = sabine_rt(180.0, &walls, None, k).unwrap();
    let eyring = eyring_rt(180.0, &walls, None, k).unwrap();
    assert!((sabine - 0.6709).abs() < 1e-4, "{sabine}");
    assert!((eyring - 0.6013).abs() < 1e-4, "{eyring}");

    let r = receipt();
    let t30 = |id: &str| -> Vec<(i64, f64)> {
        let c = r["cells"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["cell"]["id"] == id)
            .unwrap_or_else(|| panic!("{id}"));
        assert!(
            c["cell"]["walls"]
                .as_str()
                .unwrap()
                .starts_with("tutorial 1's materials"),
            "{id}"
        );
        // Tutorial 1's walls are specular.
        assert!(
            c["bands"]
                .as_array()
                .unwrap()
                .iter()
                .all(|b| b["lambert"] == false)
        );
        let mut by: std::collections::BTreeMap<i64, Vec<f64>> = Default::default();
        for row in c["quantities"]["t30_s"].as_array().unwrap() {
            by.entry(row["freq_hz"].as_i64().unwrap())
                .or_default()
                .push(row["mean"].as_f64().unwrap());
        }
        by.into_iter()
            .map(|(f, v)| {
                assert_eq!(v.len(), 6, "{id} {f} Hz: six receivers");
                (f, v.iter().sum::<f64>() / 6.0)
            })
            .collect()
    };
    let near = |got: f64, want: f64| (got - want).abs() <= 0.005;
    for (id, low, high) in [
        ("C-R6", 0.98, 0.79),
        ("V4-R2", 0.95, 0.78),
        ("V4-E14", 0.95, 0.78),
    ] {
        let bands = t30(id);
        println!("{id}: T30 per band {bands:?}; Sabine {sabine:.4} s, Eyring {eyring:.4} s");
        assert_eq!(bands.len(), 6, "{id}");
        assert!(near(bands[0].1, low), "{id} at 125 Hz: {}", bands[0].1);
        assert!(near(bands[5].1, high), "{id} at 4 kHz: {}", bands[5].1);
        for (f, t) in &bands {
            assert!(*t > 1.15 * sabine, "{id} at {f} Hz: {t} is Sabine's time");
        }
    }

    // Why the doc calls none of them converged (the review of piece C): at 125 Hz, each cell's
    // mean over the six receivers per seed, its mean and standard error over the ten seeds.
    let at_125 = |id: &str| -> (f64, f64, Vec<Vec<f64>>) {
        let c = r["cells"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["cell"]["id"] == id)
            .unwrap();
        let seeds: Vec<Vec<f64>> = c["quantities"]["t30_s"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|row| row["freq_hz"] == 125)
            .map(|row| {
                row["seed_values"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|v| v.as_f64().unwrap())
                    .collect()
            })
            .collect();
        assert_eq!(seeds.len(), 6, "{id}");
        let per_seed: Vec<f64> = (0..10)
            .map(|i| seeds.iter().map(|s| s[i]).sum::<f64>() / 6.0)
            .collect();
        let mean = per_seed.iter().sum::<f64>() / 10.0;
        let var = per_seed.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / 9.0;
        (mean, (var / 10.0).sqrt(), seeds)
    };
    let (c_r6, c_r6_se, c_r6_seeds) = at_125("C-R6");
    let (v4_r2, v4_r2_se, _) = at_125("V4-R2");
    let (v4_e14, v4_e14_se, _) = at_125("V4-E14");
    println!(
        "125 Hz: C-R6 {c_r6:.4} ± {c_r6_se:.4}, V4-R2 {v4_r2:.4} ± {v4_r2_se:.4}, V4-E14 \
         {v4_e14:.4} ± {v4_e14_se:.4}"
    );
    let above = c_r6 / v4_r2 - 1.0;
    assert!((above - 0.026).abs() < 0.001, "{above}");
    assert!(((c_r6 - v4_r2) / c_r6_se - 2.5).abs() < 0.1);
    assert!((c_r6_se - 0.010).abs() < 0.0005, "{c_r6_se}");
    assert!((v4_r2_se - 0.005).abs() < 0.0005, "{v4_r2_se}");
    assert!((v4_e14 - 0.9545).abs() < 0.00005 && v4_e14_se < 0.0006);
    let first = &c_r6_seeds[0];
    let (lo, hi) = first
        .iter()
        .fold((f64::MAX, f64::MIN), |(lo, hi), &x| (lo.min(x), hi.max(x)));
    assert!(
        (lo - 0.935).abs() < 0.001 && (hi - 1.188).abs() < 0.001,
        "{lo} {hi}"
    );
    let cv: Vec<f64> = c_r6_seeds
        .iter()
        .map(|s| {
            let m = s.iter().sum::<f64>() / 10.0;
            (s.iter().map(|x| (x - m).powi(2)).sum::<f64>() / 9.0).sqrt() / m
        })
        .collect();
    let (cv_lo, cv_hi) = cv
        .iter()
        .fold((f64::MAX, f64::MIN), |(lo, hi), &x| (lo.min(x), hi.max(x)));
    assert!(
        (0.04..0.05).contains(&cv_lo) && (0.07..0.08).contains(&cv_hi),
        "{cv:?}"
    );
    // Says no: the cells agreeing within C-R6's error would have made "converged" defensible.
    assert!(c_r6 - v4_r2 > 2.0 * c_r6_se);
}
