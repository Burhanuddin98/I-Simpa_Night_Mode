// The noise calibration's statistics and rules, as pre-registered
// (`docs/investigations/2026-09-25-noise-calibration/PREREGISTER.txt`), shared by the evidence
// harness that writes the receipt (`crates/simpa/tests/noise_calibration.rs`) and the suite's
// check of the committed receipt (`tests/params_noise_calibration.rs`), each with
// `#[path = ".../common/noise_calibration.rs"] mod calibration;`.
//
// The receipt (`calibration.json`): `{"cells": [{"cell": {...its configuration...}, "quantities":
// {"spl_db": [row, ...], ...}}]}`, one row per receiver-band where every seed gives a value:
// `{"receiver", "freq_hz", "mean", "observed_sd", "predicted_sd": {"constant": …,
// "mean_energy": …}}`. The decay times' standard deviations are relative; the predicted ones are
// the root-mean-square over the seeds of the model's, before calibration.

#![allow(dead_code)]

use serde_json::Value;

/// The receipt's eight quantities, in `params::noise::QUANTITY_NAMES`' order.
pub const QUANTITIES: [&str; 8] = [
    "spl_db", "edt_s", "t20_s", "t30_s", "c50_db", "c80_db", "d50", "ts_s",
];

/// The model's structure a method's calibration uses, as the receipt names it: M7's constant
/// deposit in both, for energetic mode chosen by rule 1 over the mean-energy candidate on the
/// calibration cells (geometric-mean overstatement 1.653 against 2.231).
pub fn structure(method: &str) -> &'static str {
    match method {
        "random" => "constant",
        _ => ENERGETIC_STRUCTURE,
    }
}

/// Energetic mode's structure (rule 1).
pub const ENERGETIC_STRUCTURE: &str = "constant";

/// The roles of the cells a method's quantity is calibrated on: round 1's calibration cells, and
/// for energetic T20 and T30, whose round-1 factors failed their validation, round 2's: every
/// energetic cell of round 1 (`PREREGISTER.txt`, "ROUND 2").
pub fn calibration_roles(method: &str, quantity: &str) -> &'static [&'static str] {
    if round_two(method, quantity) {
        &["calibration", "validation"]
    } else {
        &["calibration"]
    }
}

/// The roles of the cells that validate a method's quantity: round 1's validation cells and round
/// 2's, which validate every quantity; for energetic T20 and T30 round 2's alone.
pub fn validation_roles(method: &str, quantity: &str) -> &'static [&'static str] {
    if round_two(method, quantity) {
        &["validation2"]
    } else {
        &["validation", "validation2"]
    }
}

/// Energetic T20 and T30: re-calibrated in round 2.
pub fn round_two(method: &str, quantity: &str) -> bool {
    method == "energetic" && matches!(quantity, "t20_s" | "t30_s")
}

/// The cells of `receipt` in `roles` for `method`.
pub fn cells_in<'a>(receipt: &'a Value, method: &str, roles: &[&str]) -> Vec<&'a Value> {
    receipt["cells"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|c| c["cell"]["method"] == method && roles.iter().any(|r| c["cell"]["role"] == *r))
        .collect()
}

/// The pairs of cells that differ only in their particle count (rule 4), the lower count first.
pub const PAIRS: [(&str, &str); 6] = [
    ("C-R1", "C-R2"),
    ("C-R3", "C-R4"),
    ("C-R5", "V-R4"),
    ("C-E1", "C-E2"),
    ("C-E3", "C-E4"),
    ("C-E5", "V-E4"),
];

/// One receiver-band: the seeds' spread and the model's standard deviation.
#[derive(Clone, Copy, Debug)]
pub struct Row {
    pub observed: f64,
    pub predicted: f64,
}

/// The rows of `cell`'s `quantity` under `structure` (those that give it: the mean-energy
/// candidate is energetic mode's only).
pub fn rows(cell: &Value, quantity: &str, structure: &str) -> Vec<Row> {
    cell["quantities"][quantity]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|r| {
                    Some(Row {
                        observed: r["observed_sd"].as_f64().unwrap(),
                        predicted: r["predicted_sd"][structure].as_f64()?,
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

/// The seeds of `cell`.
pub fn seeds(cell: &Value) -> usize {
    cell["cell"]["seeds"].as_array().unwrap().len()
}

/// A cell's pooled ratio of the seeds' spread to the calibrated model, `√(Σ observed² / Σ (k ·
/// predicted)²)`, with its one-sided 95 % bounds from the chi-square distribution of
/// `receiver-bands × (seeds − 1)` degrees of freedom.
#[derive(Clone, Copy, Debug)]
pub struct Pooled {
    pub ratio: f64,
    pub dof: f64,
    pub lower: f64,
    pub upper: f64,
}

pub fn pooled(rows: &[Row], factor: f64, seeds: usize) -> Option<Pooled> {
    if rows.is_empty() {
        return None;
    }
    let o: f64 = rows.iter().map(|r| r.observed * r.observed).sum();
    let p: f64 = rows.iter().map(|r| (factor * r.predicted).powi(2)).sum();
    let ratio = (o / p).sqrt();
    let dof = (rows.len() * (seeds - 1)) as f64;
    Some(Pooled {
        ratio,
        dof,
        lower: ratio * (dof / chi2_quantile(dof, 0.95)).sqrt(),
        upper: ratio * (dof / chi2_quantile(dof, 0.05)).sqrt(),
    })
}

/// Rule 2: a method's factor for `quantity`, the largest one-sided 95 % upper bound of the pooled
/// ratio (at factor 1) over the calibration cells, rounded up to two significant digits; with the
/// cell that sets it. `None` when no calibration cell has a receiver-band with a value in every
/// seed.
pub fn factor(cells: &[&Value], quantity: &str) -> Option<(f64, String)> {
    let mut best: Option<(f64, String)> = None;
    for c in cells {
        let method = c["cell"]["method"].as_str().unwrap();
        let Some(p) = pooled(&rows(c, quantity, structure(method)), 1.0, seeds(c)) else {
            continue;
        };
        if best.as_ref().is_none_or(|(b, _)| p.upper > *b) {
            best = Some((p.upper, c["cell"]["id"].as_str().unwrap().to_string()));
        }
    }
    best.map(|(u, id)| (round_up_two_digits(u), id))
}

/// Rule 5b: a method's margin for `quantity`, `1 + 1.28·s`, `s` the largest over the calibration
/// cells of the median over their receiver-bands of one seed's scatter (`predicted_scatter`),
/// rounded up to two significant digits and never below 1.1; with the cell that sets `s`. `None`
/// when no calibration cell has the quantity.
pub fn margin(cells: &[&Value], quantity: &str) -> Option<(f64, String)> {
    let mut best: Option<(f64, String)> = None;
    for c in cells {
        let mut sc: Vec<f64> = c["quantities"][quantity]
            .as_array()
            .map(|a| {
                a.iter()
                    .map(|r| r["predicted_scatter"].as_f64().unwrap())
                    .collect()
            })
            .unwrap_or_default();
        if sc.is_empty() {
            continue;
        }
        sc.sort_by(f64::total_cmp);
        let median = sc[sc.len() / 2];
        if best.as_ref().is_none_or(|(b, _)| median > *b) {
            best = Some((median, c["cell"]["id"].as_str().unwrap().to_string()));
        }
    }
    best.map(|(s, id)| (round_up_two_digits(1.0 + 1.28 * s).max(1.1), id))
}

/// Rule 4 for a method and quantity: every pair of [`PAIRS`] of that method in `receipt` whose
/// cells both have the quantity agrees within two joint standard errors; with the pairs that do
/// not. Only pairs whose cells are both in the receipt are read.
pub fn root_n_confirmed(receipt: &Value, method: &str, quantity: &str) -> (bool, Vec<String>) {
    let find = |id: &str| {
        receipt["cells"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["cell"]["id"] == id)
    };
    let mut differ = Vec::new();
    for (a, b) in PAIRS {
        let (Some(ca), Some(cb)) = (find(a), find(b)) else {
            continue;
        };
        if ca["cell"]["method"] != method {
            continue;
        }
        if let Some((x, y, ok)) = root_n(ca, cb, quantity)
            && !ok
        {
            differ.push(format!(
                "{a}/{b}: {:.4} vs {:.4} ({:+.1} se)",
                x.0,
                y.0,
                (y.0 - x.0) / x.1.hypot(y.1)
            ));
        }
    }
    (differ.is_empty(), differ)
}

/// `x` rounded up to two significant digits, as the nearest double to the decimal (1.4, not
/// 1.4000000000000001).
pub fn round_up_two_digits(x: f64) -> f64 {
    let e = x.log10().floor() as i32 - 1;
    // A hair's tolerance, so that a value already on the grid is not rounded up a step.
    let m = (x / 10f64.powi(e) - 1e-9).ceil();
    if e < 0 {
        m / 10f64.powi(-e)
    } else {
        m * 10f64.powi(e)
    }
}

/// Rule 3: a validation cell's quantity passes when the one-sided 95 % lower bound of the pooled
/// ratio of the seeds' spread to the calibrated model is at most 1. `None` when the cell has no
/// receiver-band with a value in every seed.
pub fn validates(cell: &Value, quantity: &str, factor: f64) -> Option<(bool, Pooled)> {
    let method = cell["cell"]["method"].as_str().unwrap();
    pooled(
        &rows(cell, quantity, structure(method)),
        factor,
        seeds(cell),
    )
    .map(|p| (p.lower <= 1.0, p))
}

/// The seeds' spread of a cell's quantity, root-mean-square over its receiver-bands, times
/// `√N`, with its standard error (`1/√(2·dof)` relative): rule 4's comparison between cells that
/// differ only in `N`.
pub fn spread_times_root_n(cell: &Value, quantity: &str) -> Option<(f64, f64)> {
    let method = cell["cell"]["method"].as_str().unwrap();
    let r = rows(cell, quantity, structure(method));
    if r.is_empty() {
        return None;
    }
    let n = cell["cell"]["particles_per_source"].as_f64().unwrap();
    let rms = (r.iter().map(|x| x.observed * x.observed).sum::<f64>() / r.len() as f64).sqrt();
    let dof = (r.len() * (seeds(cell) - 1)) as f64;
    let v = rms * n.sqrt();
    Some((v, v / (2.0 * dof).sqrt()))
}

/// The receiver-bands two cells have in common for `quantity` (same receiver and band, a value in
/// every seed of both): rule 4 compares the spread over the same receiver-bands.
pub fn common_rows(a: &Value, b: &Value, quantity: &str) -> (Vec<f64>, Vec<f64>) {
    let key = |r: &Value| {
        (
            r["receiver"].as_u64().unwrap(),
            r["freq_hz"].as_i64().unwrap(),
        )
    };
    let rows_a = a["quantities"][quantity]
        .as_array()
        .cloned()
        .unwrap_or_default();
    let rows_b = b["quantities"][quantity]
        .as_array()
        .cloned()
        .unwrap_or_default();
    let mut oa = Vec::new();
    let mut ob = Vec::new();
    for ra in &rows_a {
        if let Some(rb) = rows_b.iter().find(|rb| key(rb) == key(ra)) {
            oa.push(ra["observed_sd"].as_f64().unwrap());
            ob.push(rb["observed_sd"].as_f64().unwrap());
        }
    }
    (oa, ob)
}

/// A cell's spread times `√N` and its standard error.
pub type Scaled = (f64, f64);

/// Rule 4 on one pair of cells that differ only in `N`: the seeds' spread times `√N` over their
/// common receiver-bands, `(value, its standard error)` for each, and whether they agree within
/// two joint standard errors. `None` with no common receiver-band.
pub fn root_n(a: &Value, b: &Value, quantity: &str) -> Option<(Scaled, Scaled, bool)> {
    let (oa, ob) = common_rows(a, b, quantity);
    if oa.is_empty() {
        return None;
    }
    let at = |o: &[f64], c: &Value| {
        let n = c["cell"]["particles_per_source"].as_f64().unwrap();
        let rms = (o.iter().map(|x| x * x).sum::<f64>() / o.len() as f64).sqrt();
        let dof = (o.len() * (seeds(c) - 1)) as f64;
        let v = rms * n.sqrt();
        (v, v / (2.0 * dof).sqrt())
    };
    let (x, y) = (at(&oa, a), at(&ob, b));
    let ok = (x.0 - y.0).abs() <= 2.0 * x.1.hypot(y.1);
    Some((x, y, ok))
}

/// The inverse of the chi-square distribution's CDF at `p` with `dof` degrees of freedom
/// (Wilson–Hilferty; within 0.1 % above 30 degrees of freedom).
pub fn chi2_quantile(dof: f64, p: f64) -> f64 {
    let z = normal_quantile(p);
    let a = 2.0 / (9.0 * dof);
    dof * (1.0 - a + z * a.sqrt()).powi(3)
}

/// The standard normal quantile (Acklam's rational approximation, 1.15e-9 relative).
pub fn normal_quantile(p: f64) -> f64 {
    const A: [f64; 6] = [
        -3.969_683_028_665_376e1,
        2.209_460_984_245_205e2,
        -2.759_285_104_469_687e2,
        1.383_577_518_672_69e2,
        -3.066_479_806_614_716e1,
        2.506_628_277_459_239,
    ];
    const B: [f64; 5] = [
        -5.447_609_879_822_406e1,
        1.615_858_368_580_409e2,
        -1.556_989_798_598_866e2,
        6.680_131_188_771_972e1,
        -1.328_068_155_288_572e1,
    ];
    const C: [f64; 6] = [
        -7.784_894_002_430_293e-3,
        -3.223_964_580_411_365e-1,
        -2.400_758_277_161_838,
        -2.549_732_539_343_734,
        4.374_664_141_464_968,
        2.938_163_982_698_783,
    ];
    const D: [f64; 4] = [
        7.784_695_709_041_462e-3,
        3.224_671_290_700_398e-1,
        2.445_134_137_142_996,
        3.754_408_661_907_416,
    ];
    let pl = 0.024_25;
    if p < pl {
        let q = (-2.0 * p.ln()).sqrt();
        (((((C[0] * q + C[1]) * q + C[2]) * q + C[3]) * q + C[4]) * q + C[5])
            / ((((D[0] * q + D[1]) * q + D[2]) * q + D[3]) * q + 1.0)
    } else if p <= 1.0 - pl {
        let q = p - 0.5;
        let r = q * q;
        (((((A[0] * r + A[1]) * r + A[2]) * r + A[3]) * r + A[4]) * r + A[5]) * q
            / (((((B[0] * r + B[1]) * r + B[2]) * r + B[3]) * r + B[4]) * r + 1.0)
    } else {
        -normal_quantile(1.0 - p)
    }
}
