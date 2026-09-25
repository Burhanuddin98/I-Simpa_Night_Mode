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
/// 2's, which validate every quantity. For energetic T20 and T30, whose round-2 factor failed its
/// validation too and which fall back to factor 1 (R2-4), every energetic cell: no cell set that
/// factor.
pub fn validation_roles(method: &str, quantity: &str) -> &'static [&'static str] {
    if round_two(method, quantity) {
        &["calibration", "validation", "validation2"]
    } else {
        &["validation", "validation2"]
    }
}

/// Rules 2 and R2-4 for a method's quantity: the factor rule 2 gives on its calibration cells;
/// for energetic T20 and T30, 1 (M7's bound) when that factor fails any of round 2's validation
/// cells. With the cell that set the factor, or the round-2 cell and pooled ratio that failed it.
pub fn shipped_factor(receipt: &Value, method: &str, quantity: &str) -> (f64, String) {
    let cal = cells_in(receipt, method, calibration_roles(method, quantity));
    let (k, by) = factor(&cal, quantity)
        .unwrap_or_else(|| panic!("{method} {quantity}: no calibration cell gives it"));
    if !round_two(method, quantity) {
        return (k, by);
    }
    for c in cells_in(receipt, method, &["validation2"]) {
        if let Some((false, p)) = validates(c, quantity, k) {
            return (
                1.0,
                format!(
                    "M7's bound: round 2's {k} failed {} ({:.3}, lower bound {:.3})",
                    c["cell"]["id"].as_str().unwrap(),
                    p.ratio,
                    p.lower
                ),
            );
        }
    }
    (k, by)
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

// --- Round 3 (`PREREGISTER.txt`, "ROUND 3") -----------------------------------------------------
//
// The prediction for a receiver-band is `k · √(1 + κ·n) · sd`: `sd` the bootstrap's (before
// calibration), `n` the run's crossings per particle (`n1`) or that times its particles' lifetime
// spread (`n1·cv2`), whichever rule R3-2 chooses per method; `k` and `κ` per method and quantity,
// and for energetic T20 and T30 per kind of band (every face Lambert with scattering 1, or not).
// The bounds take effective degrees of freedom (R3-3). A value is given only inside the domain
// its quantity was calibrated on: at least the fewest particles and at most the most crossings per
// particle of its calibration rows (R3-5).

/// The grid `κ` is chosen from (R3-1): 0 to 3 in steps of 0.25.
pub const KAPPAS: [f64; 13] = [
    0.0, 0.25, 0.5, 0.75, 1.0, 1.25, 1.5, 1.75, 2.0, 2.25, 2.5, 2.75, 3.0,
];

/// The multi-crossing variable (R3-2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Var {
    /// Crossings per particle.
    N1,
    /// Crossings per particle times the lifetimes' `Var L / (E L)²`.
    N1Cv2,
}

pub const VARS: [Var; 2] = [Var::N1, Var::N1Cv2];

pub fn var_name(v: Var) -> &'static str {
    match v {
        Var::N1 => "n1",
        Var::N1Cv2 => "n1*cv2",
    }
}

/// Which receiver-bands a factor covers: all, or for energetic T20 and T30 those whose every face
/// is Lambert with scattering 1 and the others (R3-4).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Split {
    All,
    Lambert,
    Other,
}

pub fn split_name(s: Split) -> &'static str {
    match s {
        Split::All => "all",
        Split::Lambert => "lambert",
        Split::Other => "other",
    }
}

/// The splits of a method's quantity.
pub fn splits(method: &str, quantity: &str) -> &'static [Split] {
    if round_two(method, quantity) {
        &[Split::Lambert, Split::Other]
    } else {
        &[Split::All]
    }
}

/// The factor a split is held to rather than fitted (R3-4): energetic T20 and T30 outside Lambert
/// bands keep M7's structure, factor 1, as rounds 1 and 2 left them.
pub fn fixed_factor(method: &str, quantity: &str, split: Split) -> Option<f64> {
    (round_two(method, quantity) && split == Split::Other).then_some(1.0)
}

/// One receiver-band for round 3.
#[derive(Clone, Debug)]
pub struct Row3 {
    pub receiver: u64,
    pub freq_hz: i64,
    pub observed: f64,
    /// Each seed's model standard deviation before calibration.
    pub seed_sd: Vec<f64>,
    /// Each seed's multi-crossing variable.
    pub seed_n: Vec<f64>,
    pub seed_values: Vec<f64>,
    pub lambert: bool,
}

impl Row3 {
    /// The predicted variance with correction `κ`: the mean over the seeds of `sd²·(1 + κ·n)`.
    pub fn predicted_var(&self, kappa: f64) -> f64 {
        self.seed_sd
            .iter()
            .zip(&self.seed_n)
            .map(|(s, n)| s * s * (1.0 + kappa * n))
            .sum::<f64>()
            / self.seed_sd.len() as f64
    }

    /// The largest of the seeds' multi-crossing variable.
    pub fn n_max(&self) -> f64 {
        self.seed_n.iter().copied().fold(0.0, f64::max)
    }
}

fn floats(v: &Value) -> Vec<f64> {
    v.as_array()
        .unwrap()
        .iter()
        .map(|x| x.as_f64().unwrap())
        .collect()
}

/// A cell's rows of `quantity` in `split`, with `var`, whose largest `n` is at most `n_max`.
pub fn rows3(cell: &Value, quantity: &str, var: Var, split: Split, n_max: f64) -> Vec<Row3> {
    cell["quantities"][quantity]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|r| {
                    let lambert = r["lambert"].as_bool().unwrap();
                    match split {
                        Split::All => {}
                        Split::Lambert if lambert => {}
                        Split::Other if !lambert => {}
                        _ => return None,
                    }
                    let n1 = floats(&r["seed_n1"]);
                    let cv2 = floats(&r["seed_cv2"]);
                    let seed_n = match var {
                        Var::N1 => n1,
                        Var::N1Cv2 => n1.iter().zip(&cv2).map(|(a, b)| a * b).collect(),
                    };
                    let row = Row3 {
                        receiver: r["receiver"].as_u64().unwrap(),
                        freq_hz: r["freq_hz"].as_i64().unwrap(),
                        observed: r["observed_sd"].as_f64().unwrap(),
                        seed_sd: floats(&r["seed_model_sd"]),
                        seed_n,
                        seed_values: floats(&r["seed_values"]),
                        lambert,
                    };
                    (row.n_max() <= n_max).then_some(row)
                })
                .collect()
        })
        .unwrap_or_default()
}

/// The Pearson correlation of two equal-length series.
fn correlation(a: &[f64], b: &[f64]) -> f64 {
    let n = a.len() as f64;
    let (ma, mb) = (a.iter().sum::<f64>() / n, b.iter().sum::<f64>() / n);
    let (mut sab, mut saa, mut sbb) = (0.0, 0.0, 0.0);
    for (x, y) in a.iter().zip(b) {
        sab += (x - ma) * (y - mb);
        saa += (x - ma) * (x - ma);
        sbb += (y - mb) * (y - mb);
    }
    if saa > 0.0 && sbb > 0.0 {
        sab / (saa * sbb).sqrt()
    } else {
        0.0
    }
}

/// R3-3: the effective degrees of freedom of a pooled ratio: `rows × (seeds − 1)` over the
/// dispersion `φ` of the rows' ratios² against what the chi-square allows (at least 1) and the
/// design effect of the receivers of one band sharing its particles, `1 + (m − 1)·ρ²`, `ρ²` the
/// mean squared correlation of the seeds' values between receivers of one band, less its bias
/// `1/(seeds − 1)`, and `m` the mean rows per band.
pub fn dof_eff(rows: &[Row3], kappa: f64, seeds: usize) -> f64 {
    let m = rows.len();
    let nu = (seeds - 1) as f64;
    let x: Vec<f64> = rows
        .iter()
        .map(|r| r.observed * r.observed / r.predicted_var(kappa))
        .collect();
    let phi = if m > 1 {
        let mean = x.iter().sum::<f64>() / m as f64;
        let var = x.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / (m - 1) as f64;
        (var / (2.0 / nu * mean * mean)).max(1.0)
    } else {
        1.0
    };
    let (mut sum_r2, mut pairs) = (0.0, 0usize);
    let mut bands: Vec<i64> = rows.iter().map(|r| r.freq_hz).collect();
    bands.sort_unstable();
    bands.dedup();
    for f in &bands {
        let group: Vec<&Row3> = rows.iter().filter(|r| r.freq_hz == *f).collect();
        for i in 0..group.len() {
            for j in i + 1..group.len() {
                sum_r2 += correlation(&group[i].seed_values, &group[j].seed_values).powi(2);
                pairs += 1;
            }
        }
    }
    let deff = if pairs > 0 {
        let bias = 1.0 / nu;
        let rho2 = ((sum_r2 / pairs as f64 - bias) / (1.0 - bias)).max(0.0);
        let per_band = m as f64 / bands.len() as f64;
        1.0 + (per_band - 1.0) * rho2
    } else {
        1.0
    };
    m as f64 * nu / (phi * deff)
}

/// A cell's pooled ratio of the seeds' spread to `k · √(1 + κ·n) · sd`, with one-sided 95 % bounds
/// on R3-3's degrees of freedom. `None` without rows.
pub fn pooled3(rows: &[Row3], k: f64, kappa: f64, seeds: usize) -> Option<Pooled> {
    if rows.is_empty() {
        return None;
    }
    let o: f64 = rows.iter().map(|r| r.observed * r.observed).sum();
    let p: f64 = rows.iter().map(|r| k * k * r.predicted_var(kappa)).sum();
    let ratio = (o / p).sqrt();
    let dof = dof_eff(rows, kappa, seeds);
    Some(Pooled {
        ratio,
        dof,
        lower: ratio * (dof / chi2_quantile(dof, 0.95)).sqrt(),
        upper: ratio * (dof / chi2_quantile(dof, 0.05)).sqrt(),
    })
}

/// Round 3's calibration cells of a method: every cell of rounds 1 and 2, and round 3's own.
pub fn calibration3_cells<'a>(receipt: &'a Value, method: &str) -> Vec<&'a Value> {
    cells_in(
        receipt,
        method,
        &["calibration", "validation", "validation2", "calibration3"],
    )
}

/// Round 3's validation cells of a method.
pub fn validation3_cells<'a>(receipt: &'a Value, method: &str) -> Vec<&'a Value> {
    cells_in(receipt, method, &["validation3"])
}

/// R3-1: a quantity's (and split's) factor and correction on its calibration cells.
#[derive(Clone, Debug, PartialEq)]
pub struct Fit {
    pub k: f64,
    pub kappa: f64,
    /// The cell whose upper bound sets `k` (or, for a fixed factor, the largest bound under it).
    pub by: String,
    /// The geometric mean over the cells of `k·√(1 + κ·n)` over the seeds' spread.
    pub overstates: f64,
    /// Cells with rows.
    pub cells: usize,
    /// `ln(k / ratio)` per cell, for R3-2.
    pub logs: Vec<f64>,
}

/// R3-1 for one `κ`: `k` the largest one-sided 95 % upper bound over `cells` of the pooled ratio
/// at factor 1 with correction `κ`, rounded up to two significant digits (or the fixed factor,
/// when every bound is at most it; `None` when one is above it or no cell has rows).
pub fn fit_at(
    cells: &[&Value],
    quantity: &str,
    var: Var,
    split: Split,
    kappa: f64,
    fixed: Option<f64>,
) -> Option<Fit> {
    let mut best: Option<(f64, String)> = None;
    let mut ratios = Vec::new();
    for c in cells {
        let rows = rows3(c, quantity, var, split, f64::INFINITY);
        let Some(p) = pooled3(&rows, 1.0, kappa, seeds(c)) else {
            continue;
        };
        ratios.push(p.ratio);
        if best.as_ref().is_none_or(|(u, _)| p.upper > *u) {
            best = Some((p.upper, c["cell"]["id"].as_str().unwrap().to_string()));
        }
    }
    let (upper, by) = best?;
    let k = match fixed {
        Some(f) if upper <= f => f,
        Some(_) => return None,
        None => round_up_two_digits(upper),
    };
    let logs: Vec<f64> = ratios.iter().map(|r| (k / r).ln()).collect();
    Some(Fit {
        k,
        kappa,
        by,
        overstates: (logs.iter().sum::<f64>() / logs.len() as f64).exp(),
        cells: ratios.len(),
        logs,
    })
}

/// R3-1: the `κ` of [`KAPPAS`] whose fit overstates least (the geometric mean), the smaller on a
/// tie; for a fixed factor, the smallest `κ` that keeps it (`None` when none does).
pub fn fit(cells: &[&Value], quantity: &str, var: Var, split: Split) -> Option<Fit> {
    let method = cells.first()?["cell"]["method"].as_str().unwrap();
    let fixed = fixed_factor(method, quantity, split);
    let mut best: Option<Fit> = None;
    for kappa in KAPPAS {
        let Some(f) = fit_at(cells, quantity, var, split, kappa, fixed) else {
            continue;
        };
        if fixed.is_some() {
            return Some(f);
        }
        if best.as_ref().is_none_or(|b| f.overstates < b.overstates) {
            best = Some(f);
        }
    }
    best
}

/// R3-2: a method's variable, the one whose fits overstate least over every quantity, split and
/// calibration cell together (geometric mean); `n1` on a tie. With the geometric means.
pub fn choose_var(receipt: &Value, method: &str) -> (Var, Vec<(Var, f64)>) {
    let cells = calibration3_cells(receipt, method);
    let mut out = Vec::new();
    for var in VARS {
        let mut logs = Vec::new();
        for q in QUANTITIES {
            for &s in splits(method, q) {
                if let Some(f) = fit(&cells, q, var, s) {
                    logs.extend(f.logs);
                }
            }
        }
        out.push((var, (logs.iter().sum::<f64>() / logs.len() as f64).exp()));
    }
    let best = if out[1].1 < out[0].1 {
        out[1].0
    } else {
        out[0].0
    };
    (best, out)
}

/// R3-5: the domain of a quantity's (and split's) calibration: the fewest particles of a
/// calibration cell with a row of it, and the largest `n` of any seed of those rows.
pub fn domain(cells: &[&Value], quantity: &str, var: Var, split: Split) -> Option<(f64, f64)> {
    let mut least: Option<f64> = None;
    let mut most = 0.0f64;
    for c in cells {
        let rows = rows3(c, quantity, var, split, f64::INFINITY);
        if rows.is_empty() {
            continue;
        }
        let n = c["cell"]["particles_per_source"].as_f64().unwrap();
        least = Some(least.map_or(n, |l| l.min(n)));
        for r in &rows {
            most = most.max(r.n_max());
        }
    }
    least.map(|l| (l, most))
}

/// Rule 5b over round 3's calibration cells, for a quantity and split.
pub fn margin3(cells: &[&Value], quantity: &str, split: Split) -> Option<(f64, String)> {
    let mut best: Option<(f64, String)> = None;
    for c in cells {
        let mut sc: Vec<f64> = c["quantities"][quantity]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter(|r| match split {
                        Split::All => true,
                        Split::Lambert => r["lambert"] == true,
                        Split::Other => r["lambert"] == false,
                    })
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

/// The pairs of cells that differ only in their particle count, round 3's added to rounds 1 and
/// 2's (R3-6), the lower count first.
pub const PAIRS3: [(&str, &str); 12] = [
    ("C-R1", "C-R2"),
    ("C-R3", "C-R4"),
    ("C-R5", "V-R4"),
    ("C-E1", "C-E2"),
    ("C-E3", "C-E4"),
    ("C-E5", "V-E4"),
    ("C3-R5", "V3-R8"),
    ("C3-R6", "C-R5"),
    ("C3-R9", "V3-R9"),
    ("C3-E5", "V3-E5"),
    ("C3-E6", "C-E5"),
    ("C3-E10", "V3-E9"),
];

/// R3-6 on one pair: the seeds' spread times `√N` over the receiver-bands both have, each with
/// its standard error on R3-3's degrees of freedom, and `z` = (higher count's − lower count's) /
/// joint standard error. The named count is safe unless the spread falls slower than `1/√N`,
/// `z` above 2.
pub fn root_n3(a: &Value, b: &Value, quantity: &str) -> Option<(Scaled, Scaled, f64)> {
    let key = |r: &Row3| (r.receiver, r.freq_hz);
    let ra = rows3(a, quantity, Var::N1, Split::All, f64::INFINITY);
    let rb = rows3(b, quantity, Var::N1, Split::All, f64::INFINITY);
    let ca: Vec<Row3> = ra
        .iter()
        .filter(|x| rb.iter().any(|y| key(y) == key(x)))
        .cloned()
        .collect();
    let cb: Vec<Row3> = rb
        .iter()
        .filter(|y| ca.iter().any(|x| key(x) == key(y)))
        .cloned()
        .collect();
    if ca.is_empty() {
        return None;
    }
    let at = |rows: &[Row3], c: &Value| {
        let n = c["cell"]["particles_per_source"].as_f64().unwrap();
        let rms =
            (rows.iter().map(|x| x.observed * x.observed).sum::<f64>() / rows.len() as f64).sqrt();
        let dof = dof_eff(rows, 0.0, seeds(c));
        let v = rms * n.sqrt();
        (v, v / (2.0 * dof).sqrt())
    };
    let (x, y) = (at(&ca, a), at(&cb, b));
    Some((x, y, (y.0 - x.0) / x.1.hypot(y.1)))
}

/// R3-6 for a method and quantity over the pairs whose both cells are among `ids` (the cells read
/// so far): confirmed unless a pair's `z` is above 2; with the pairs read and those that fail.
pub fn root_n3_confirmed(
    receipt: &Value,
    method: &str,
    quantity: &str,
    ids: &[&str],
) -> (bool, Vec<String>, Vec<String>) {
    let find = |id: &str| {
        receipt["cells"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["cell"]["id"] == id)
    };
    let (mut read, mut unsafe_) = (Vec::new(), Vec::new());
    for (a, b) in PAIRS3 {
        if !ids.contains(&a) || !ids.contains(&b) {
            continue;
        }
        let (Some(ca), Some(cb)) = (find(a), find(b)) else {
            continue;
        };
        if ca["cell"]["method"] != method {
            continue;
        }
        if let Some((_, _, z)) = root_n3(ca, cb, quantity) {
            let line = format!("{a}/{b} {z:+.2}");
            if z > 2.0 {
                unsafe_.push(line.clone());
            }
            read.push(line);
        }
    }
    (unsafe_.is_empty(), read, unsafe_)
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
