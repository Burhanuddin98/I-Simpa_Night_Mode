//! M8a's checks and what it reports (`docs/investigations/2026-09-29-m8a/SPEC.md`, section 5).
//!
//! Every interval is two-sided 95 %, Student's t with k − 1 degrees of freedom over the k
//! per-seed values (row 17 (2): "with the seed spread as its uncertainty"). With `T_{s,r,b}` seed
//! `s`'s T30 at receiver `r` in band `b` (`read::T30`), `T_K,b` Kuttruff's time and `T_tr,b` the
//! transport's:
//! - **A**, per band: `x_s = mean_r T_{s,r,b} / T_K,b − 1`; the interval of its mean inside ±5 %.
//! - **B**: `m_s = mean_{r,b} T_{s,r,b} / T_K,b`; `(max_s m_s − min_s m_s)/mean_s m_s ≤ 2 %`, on
//!   seeds 1 to 10 only.
//! - **C**: `y_s = mean_{r,b} T_{s,r,b} / T_tr,b − 1`, `d` its mean, `SE² = var_s(y)/k + se_tr²`
//!   (`se_tr` the transport's relative standard error averaged over the bands, as if fully
//!   correlated): PASS when `|d| + t·SE ≤ 0.5 %`; FAIL when the interval excludes 0 and `|d| >
//!   0.5 %`; otherwise INCONCLUSIVE, which does not pass, and which may be extended once by seeds
//!   11 to 20 and judged again on all twenty.
//! - **D**, per band of each TCR run: `|T_TCR,Eyring / T_analytic,Eyring − 1| ≤ 0.5 %`.
//!
//! A mean over a subset of seeds or receiver-bands is never formed (SB-3): a cell in which any
//! seed's T30 is refused for more than its noise is not judged (E6).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::file::{BedFile, Cell, Method, Room, TcrCell};
use super::limits;
use super::read::{SppsRead, T30, TcrRead};
use super::stats::{self, Interval};
use super::transport::{self, Transports};
use crate::params::air::{Atmosphere, solver_air_absorption_per_m};
use crate::params::room::{RtConstant, Surface, sabine_rt};

/// A check's outcome.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
    Pass,
    Fail,
    /// Gate C only: neither pass nor fail. It does not pass.
    Inconclusive,
    /// A precondition failed, so the check was not made. It does not pass.
    NotJudged,
    /// Computed and shown, never gated.
    Reported,
}

/// One band's statistic `x_s = mean_r T_{s,r,b} / T_ref,b − 1` over the seeds, against a limit.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct BandCheck {
    pub freq_hz: i32,
    /// `None`: the band has no reference, and is not judged.
    pub reference_s: Option<f64>,
    pub interval: Option<Interval>,
    pub limit: f64,
    pub verdict: Verdict,
}

/// Check A (or, reported, R1 and R2 against plain Eyring and Sabine).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct CheckA {
    /// `kuttruff_s`, or what stands in its place (`eyring_s`, `sabine_s`).
    pub reference: String,
    pub bands: Vec<BandCheck>,
    pub verdict: Verdict,
}

/// Check B.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct CheckB {
    pub seeds: Vec<u32>,
    /// `m_s`, per seed.
    pub cell_means: Vec<f64>,
    /// `(max − min)/mean`.
    pub spread: f64,
    pub limit: f64,
    pub verdict: Verdict,
}

/// Check C.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct CheckC {
    pub seeds: Vec<u32>,
    /// `y_s`, per seed.
    pub per_seed: Vec<f64>,
    /// The transport's relative standard error, averaged over the bands.
    pub se_transport: f64,
    /// `d`, with `SE² = var_s(y)/k + se_tr²` and `t₀.₉₇₅,ₖ₋₁`.
    pub interval: Interval,
    pub limit: f64,
    pub verdict: Verdict,
}

/// Check D on one TCR run.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct CheckD {
    pub bands: Vec<BandD>,
    pub limit: f64,
    pub verdict: Verdict,
}

/// One band of check D.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct BandD {
    pub freq_hz: i32,
    pub tcr_eyring_s: f64,
    pub analytic_eyring_s: Option<f64>,
    pub deviation: Option<f64>,
    pub verdict: Verdict,
}

/// A precondition: holds or not, and what broke it.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Precondition {
    pub holds: bool,
    pub problems: Vec<String>,
}

impl Precondition {
    fn of(problems: Vec<String>) -> Precondition {
        Precondition {
            holds: problems.is_empty(),
            problems,
        }
    }
}

/// `x_s` in band `b`.
fn band_values(values: &[Vec<Vec<f64>>], b: usize, reference: f64) -> Vec<f64> {
    values
        .iter()
        .map(|seed| stats::mean(&seed.iter().map(|r| r[b]).collect::<Vec<_>>()) / reference - 1.0)
        .collect()
}

/// Check A, or its reported partners: per band, the interval of `x_s` inside ±`limit`.
/// `values[s][r][b]`, `references[b]`; a band without a reference fails.
pub fn check_a(
    values: &[Vec<Vec<f64>>],
    freqs: &[i32],
    references: &[Option<f64>],
    reference: &str,
    limit: f64,
) -> CheckA {
    let bands: Vec<BandCheck> = freqs
        .iter()
        .zip(references)
        .enumerate()
        .map(|(b, (&freq_hz, r))| match r {
            Some(r) => {
                let interval = Interval::of(&band_values(values, b, *r));
                let verdict = if interval.reach() <= limit {
                    Verdict::Pass
                } else {
                    Verdict::Fail
                };
                BandCheck {
                    freq_hz,
                    reference_s: Some(*r),
                    interval: Some(interval),
                    limit,
                    verdict,
                }
            }
            None => BandCheck {
                freq_hz,
                reference_s: None,
                interval: None,
                limit,
                verdict: Verdict::NotJudged,
            },
        })
        .collect();
    let verdict = if !bands.is_empty() && bands.iter().all(|b| b.verdict == Verdict::Pass) {
        Verdict::Pass
    } else {
        Verdict::Fail
    };
    CheckA {
        reference: reference.to_string(),
        bands,
        verdict,
    }
}

/// `mean_{r,b} T_{s,r,b} / ref_b` for one seed.
fn cell_mean(seed: &[Vec<f64>], references: &[f64]) -> f64 {
    let v: Vec<f64> = seed
        .iter()
        .flat_map(|r| r.iter().zip(references).map(|(t, k)| t / k))
        .collect();
    stats::mean(&v)
}

/// Check B over `seeds` (`values[s]` is seed `seeds[s]`'s).
pub fn check_b(seeds: &[u32], values: &[Vec<Vec<f64>>], kuttruff: &[f64], limit: f64) -> CheckB {
    let cell_means: Vec<f64> = values.iter().map(|s| cell_mean(s, kuttruff)).collect();
    let spread = stats::relative_range(&cell_means);
    CheckB {
        seeds: seeds.to_vec(),
        cell_means,
        spread,
        limit,
        verdict: if spread <= limit {
            Verdict::Pass
        } else {
            Verdict::Fail
        },
    }
}

/// Gate C's three outcomes for its interval.
pub fn judge_c(interval: &Interval, limit: f64) -> Verdict {
    if interval.reach() <= limit {
        Verdict::Pass
    } else if interval.excludes_zero() && interval.mean.abs() > limit {
        Verdict::Fail
    } else {
        Verdict::Inconclusive
    }
}

/// Check C over `seeds`, with the transport's `(T, se)` per band.
pub fn check_c(
    seeds: &[u32],
    values: &[Vec<Vec<f64>>],
    transport: &[(f64, f64)],
    limit: f64,
) -> CheckC {
    let t: Vec<f64> = transport.iter().map(|x| x.0).collect();
    let per_seed: Vec<f64> = values.iter().map(|s| cell_mean(s, &t) - 1.0).collect();
    let se_transport = stats::mean(&transport.iter().map(|(t, se)| se / t).collect::<Vec<_>>());
    let k = per_seed.len();
    let (d, sd) = (stats::mean(&per_seed), stats::sd(&per_seed));
    let se = (sd * sd / k as f64 + se_transport * se_transport).sqrt();
    let interval = Interval::with_se(k, d, sd, se);
    CheckC {
        seeds: seeds.to_vec(),
        verdict: judge_c(&interval, limit),
        per_seed,
        se_transport,
        interval,
        limit,
    }
}

/// Check D: TCR's Eyring time against the analytic one, per band.
pub fn check_d(bands: &[(i32, f64, Option<f64>)], limit: f64) -> CheckD {
    let bands: Vec<BandD> = bands
        .iter()
        .map(|&(freq_hz, tcr, analytic)| {
            let deviation = analytic.map(|a| tcr / a - 1.0);
            BandD {
                freq_hz,
                tcr_eyring_s: tcr,
                analytic_eyring_s: analytic,
                deviation,
                verdict: match deviation {
                    Some(d) if d.abs() <= limit => Verdict::Pass,
                    Some(_) => Verdict::Fail,
                    None => Verdict::NotJudged,
                },
            }
        })
        .collect();
    let verdict = if !bands.is_empty() && bands.iter().all(|b| b.verdict == Verdict::Pass) {
        Verdict::Pass
    } else {
        Verdict::Fail
    };
    CheckD {
        bands,
        limit,
        verdict,
    }
}

// ---------------------------------------------------------------------------------------------
// What is read, and the cells judged on it

/// Everything the bed read: by cell id and seed, by TCR id; the atmospheric validation's and
/// the say-NO runs'. A run that is missing, or that `simpa results` refused, is an `Err` with
/// why.
#[derive(Clone, Debug, Default)]
pub struct Reads {
    pub spps: BTreeMap<String, BTreeMap<u32, Result<SppsRead, String>>>,
    pub tcr: BTreeMap<String, Result<TcrRead, String>>,
    pub atmospheric: AtmosphericReads,
    pub n5: Option<Result<SppsRead, String>>,
    pub n6: Option<Result<SppsRead, String>>,
}

/// The atmospheric validation's runs, or why they were not run.
#[derive(Clone, Debug, Default)]
pub struct AtmosphericReads {
    pub not_run: Option<String>,
    pub spps: BTreeMap<u32, Result<SppsRead, String>>,
    pub tcr: Option<Result<TcrRead, String>>,
}

/// One seed's run in a cell.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SeedRun {
    pub seed: u32,
    /// The run's record (R12: wall and CPU time, files and bytes); `None` when there is no run.
    pub run: Option<super::read::RunInfo>,
    /// Why the run could not be read, when it could not.
    pub error: Option<String>,
    /// `[receiver][band]`.
    pub t30: Vec<Vec<T30>>,
}

/// One band of a cell's references.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ReferenceBand {
    pub freq_hz: i32,
    pub kuttruff_s: Option<f64>,
    pub kuttruff_mc_sd: Option<f64>,
    pub eyring_s: Option<f64>,
    /// Sabine with the same `K`, `V`, `S`, `ᾱ` and `m`, computed by the bed.
    pub sabine_s: Option<f64>,
    /// The run's `m`, and the bed's (`air::solver_air_absorption_per_m`), which the transport
    /// is traced with.
    pub air_m_per_metre: Option<f64>,
    pub bed_air_m_per_metre: Option<f64>,
    pub lambert_walls: bool,
    /// The transport's T30 and its standard error; the room energy's.
    pub transport_t: Option<f64>,
    pub transport_se: Option<f64>,
    pub transport_room_t: Option<f64>,
    pub transport_room_se: Option<f64>,
    /// Why there is no transport, when there is none.
    pub transport_error: Option<String>,
    /// R3: Kuttruff against the transport, `T_K/T_tr − 1`, and the transport's relative se.
    pub kuttruff_against_transport: Option<f64>,
}

/// A cell's references, read from its first seed and required bitwise equal in the others (E3).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct CellReference {
    pub volume_m3: Option<f64>,
    pub area_m2: Option<f64>,
    pub gamma2: Option<f64>,
    pub gamma2_se: Option<f64>,
    pub bands: Vec<ReferenceBand>,
}

/// R4 and R5 of one receiver-band.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ReceiverBand {
    pub receiver: usize,
    pub freq_hz: i32,
    /// R4: the seeds' mean, relative standard deviation and relative range.
    pub mean_s: f64,
    pub relative_sd: f64,
    pub relative_range: f64,
    /// R5: the seeds' standard deviation `s₁₀`, the root mean square of their calibrated
    /// `mc_sd`, their ratio and its 90 % interval (χ², k − 1 degrees of freedom).
    pub seeds_sd_s: f64,
    pub model_rms_sd_s: Option<f64>,
    pub ratio: Option<f64>,
    pub ratio_90: Option<[f64; 2]>,
    /// Seeds with an `mc_sd`, of them how many reach `s₁₀`, and seeds refused
    /// `noise_uncalibrated` (no `sd`).
    pub with_sd: usize,
    pub sd_at_least_spread: usize,
    pub uncalibrated: usize,
}

/// What a cell reports and never gates (section 5.4).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct CellReported {
    /// R1, R2: A's statistic against plain Eyring and Sabine.
    pub eyring: Option<CheckA>,
    pub sabine: Option<CheckA>,
    /// R4, R5 per receiver-band.
    pub receiver_bands: Vec<ReceiverBand>,
    /// R5 per cell: the share of receiver-band-seeds whose `mc_sd` ≥ the seeds' spread, beside
    /// the 90 % D7 of the follow-up spec asks of single-run coverage.
    pub coverage: Option<f64>,
    /// R6: `T_{s,r,b}` per source (`value`, `monte_carlo_noise`, ..., or the refusal), and the
    /// share taken from a noise refusal.
    pub sources: BTreeMap<String, usize>,
    pub noise_refusal_share: Option<f64>,
    /// R9: `mean(T/T_K) − 1` over the values that came through (`value`) only, against over all.
    pub mean_values_only: Option<f64>,
    pub mean_all: Option<f64>,
}

/// Gate C's extension (seeds 11 to 20), when it was made.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Extension {
    pub seeds: Vec<SeedRun>,
    /// C on seeds 1 to 10, which was INCONCLUSIVE.
    pub c_first: CheckC,
}

/// One SPPS cell.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct CellReport {
    pub id: String,
    pub room: String,
    pub alpha: f64,
    pub method: Method,
    pub air: bool,
    pub gated: bool,
    pub particles_per_source: u32,
    pub duration_s: f64,
    pub trans_epsilon: f64,
    pub bands_hz: Vec<i32>,
    pub seeds: Vec<SeedRun>,
    pub reference: Option<CellReference>,
    /// E2 (every run OK and read), E3 (the reference), E6 (every value judged), for this cell.
    pub e2: Precondition,
    pub e3: Precondition,
    pub e6: Precondition,
    pub a: Option<CheckA>,
    pub b: Option<CheckB>,
    pub c: Option<CheckC>,
    pub extension: Option<Extension>,
    pub reported: CellReported,
    /// `pass` exactly when A, B and C pass and E2, E3 and E6 hold; `reported` for a cell that is
    /// not gated.
    pub verdict: Verdict,
    /// Every failed or unjudged check, with its numbers.
    pub failures: Vec<String>,
}

/// One TCR run.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct TcrReport {
    pub id: String,
    pub project_of: String,
    pub gated: bool,
    pub run: Option<super::read::RunInfo>,
    pub error: Option<String>,
    pub d: Option<CheckD>,
    /// R8: TCR's Sabine against the analytic Sabine (0.163), and its Eyring against Kuttruff
    /// (from the cell `project_of`), per band.
    pub sabine_against_analytic: Vec<Option<f64>>,
    pub eyring_against_kuttruff: Vec<Option<f64>>,
    pub verdict: Verdict,
    pub failures: Vec<String>,
}

/// The bed's `m` for a cell's band.
pub fn bed_air(bed: &BedFile, air: bool, freq_hz: f64) -> Option<f64> {
    if !air {
        return None;
    }
    let a = Atmosphere {
        temperature_c: bed.atmosphere.temperature_c,
        relative_humidity_percent: bed.atmosphere.relative_humidity_percent,
        pressure_pa: bed.atmosphere.pressure_pa,
    };
    solver_air_absorption_per_m(freq_hz, &a).ok()
}

/// The transports a cell needs, one per band (the same one in every band with the air off).
pub fn cell_transport_keys(bed: &BedFile, cell: &Cell) -> Vec<(f64, Option<f64>)> {
    bed.bands_hz(cell.air)
        .iter()
        .map(|&f| (cell.alpha, bed_air(bed, cell.air, f64::from(f))))
        .collect()
}

/// The runs of one cell: `(seed, read)` in seed order; a missing run is an `Err`.
fn runs_of<'a>(
    reads: &'a BTreeMap<u32, Result<SppsRead, String>>,
    seeds: &[u32],
    missing: &'a Result<SppsRead, String>,
) -> Vec<(u32, &'a Result<SppsRead, String>)> {
    seeds
        .iter()
        .map(|s| (*s, reads.get(s).unwrap_or(missing)))
        .collect()
}

fn seed_run(seed: u32, r: &Result<SppsRead, String>) -> SeedRun {
    match r {
        Ok(read) => SeedRun {
            seed,
            run: Some(read.info.clone()),
            error: None,
            t30: read.t30.clone(),
        },
        Err(e) => SeedRun {
            seed,
            run: None,
            error: Some(e.clone()),
            t30: Vec::new(),
        },
    }
}

/// E2 on a cell's runs: each read, OK.
fn e2_of(runs: &[(u32, &Result<SppsRead, String>)]) -> Vec<String> {
    runs.iter()
        .filter_map(|(s, r)| match r {
            Err(e) => Some(format!("seed {s}: {e}")),
            Ok(read) if read.info.status != "OK" => Some(format!(
                "seed {s}: run {} is {} ({})",
                read.info.folder,
                read.info.status,
                read.info.reasons.join(", ")
            )),
            Ok(_) => None,
        })
        .collect()
}

/// E3 on a cell's runs: the reference computed, a Kuttruff value in every band with Lambert
/// walls, bitwise equal in every seed; the bands and receivers the bed's.
fn e3_of(runs: &[(u32, &SppsRead)], freqs: &[i32], receivers: usize) -> Vec<String> {
    let mut out = Vec::new();
    let Some((_, first)) = runs.first() else {
        return vec!["no run".into()];
    };
    for (s, r) in runs {
        if r.bands_hz != freqs {
            out.push(format!(
                "seed {s}: bands {:?}, the bed's {freqs:?}",
                r.bands_hz
            ));
            continue;
        }
        if r.t30.len() != receivers {
            out.push(format!(
                "seed {s}: {} receivers, the bed's {receivers}",
                r.t30.len()
            ));
        }
        let rf = &r.reference;
        if let Some(why) = &rf.not_computed {
            out.push(format!("seed {s}: no reference: {why}"));
            continue;
        }
        if rf.gamma2.is_none() {
            out.push(format!("seed {s}: free_paths not computed"));
        }
        for (b, band) in rf.bands.iter().enumerate() {
            if !band.lambert_walls {
                out.push(format!(
                    "seed {s}, {} Hz: lambert_walls false",
                    band.freq_hz
                ));
            }
            match band.kuttruff_s {
                None => out.push(format!(
                    "seed {s}, {} Hz: kuttruff_s refused {}",
                    band.freq_hz,
                    band.kuttruff_refused.as_deref().unwrap_or("?")
                )),
                Some(k) => {
                    let k0 = first.reference.bands.get(b).and_then(|x| x.kuttruff_s);
                    if k0.map(f64::to_bits) != Some(k.to_bits()) {
                        out.push(format!(
                            "seed {s}, {} Hz: kuttruff_s {k} is not seed {}'s {k0:?} bitwise",
                            band.freq_hz, runs[0].0
                        ));
                    }
                }
            }
        }
        if rf.bands.len() != freqs.len() {
            out.push(format!(
                "seed {s}: the reference has {} bands",
                rf.bands.len()
            ));
        }
    }
    out
}

/// E6 on a cell's runs: every seed's T30 a value or a noise refusal.
fn e6_of(runs: &[(u32, &SppsRead)], freqs: &[i32]) -> Vec<String> {
    let mut out = Vec::new();
    for (s, r) in runs {
        for (ri, rec) in r.t30.iter().enumerate() {
            for (bi, t) in rec.iter().enumerate() {
                if !t.judged() {
                    out.push(format!(
                        "seed {s}, receiver R{ri:03}, {} Hz: T30 refused {}",
                        freqs.get(bi).copied().unwrap_or(0),
                        t.source
                    ));
                }
            }
        }
    }
    out
}

/// `values[s][r][b]`: the judged values.
fn values_of(runs: &[(u32, &SppsRead)]) -> Vec<Vec<Vec<f64>>> {
    runs.iter()
        .map(|(_, r)| {
            r.t30
                .iter()
                .map(|rec| rec.iter().map(|t| t.t.unwrap_or(f64::NAN)).collect())
                .collect()
        })
        .collect()
}

/// R4, R5 per receiver-band, and R5's coverage share.
fn receiver_bands(runs: &[(u32, &SppsRead)], freqs: &[i32]) -> (Vec<ReceiverBand>, Option<f64>) {
    let k = runs.len();
    let dof = (k - 1) as f64;
    let (lo_q, hi_q) = (
        stats::chi2_quantile(0.95, dof),
        stats::chi2_quantile(0.05, dof),
    );
    let (mut covered, mut with_sd_all) = (0usize, 0usize);
    let mut out = Vec::new();
    let receivers = runs[0].1.t30.len();
    for r in 0..receivers {
        for (b, &freq_hz) in freqs.iter().enumerate() {
            let ts: Vec<&T30> = runs.iter().map(|(_, x)| &x.t30[r][b]).collect();
            let v: Vec<f64> = ts.iter().filter_map(|t| t.t).collect();
            let (m, s) = (stats::mean(&v), stats::sd(&v));
            let sds: Vec<f64> = ts.iter().filter_map(|t| t.mc_sd).collect();
            let rms = (!sds.is_empty())
                .then(|| (sds.iter().map(|x| x * x).sum::<f64>() / sds.len() as f64).sqrt());
            let ratio = rms.map(|q| s / q);
            let at_least = sds.iter().filter(|q| **q >= s).count();
            covered += at_least;
            with_sd_all += sds.len();
            out.push(ReceiverBand {
                receiver: r,
                freq_hz,
                mean_s: m,
                relative_sd: s / m,
                relative_range: stats::relative_range(&v),
                seeds_sd_s: s,
                model_rms_sd_s: rms,
                ratio,
                ratio_90: ratio.map(|q| [q * (dof / lo_q).sqrt(), q * (dof / hi_q).sqrt()]),
                with_sd: sds.len(),
                sd_at_least_spread: at_least,
                uncalibrated: ts
                    .iter()
                    .filter(|t| t.source == "noise_uncalibrated")
                    .count(),
            });
        }
    }
    let coverage = (with_sd_all > 0).then(|| covered as f64 / with_sd_all as f64);
    (out, coverage)
}

/// R6: the sources of every `T_{s,r,b}` read.
fn sources_of(runs: &[(u32, &Result<SppsRead, String>)]) -> (BTreeMap<String, usize>, Option<f64>) {
    let mut out: BTreeMap<String, usize> = BTreeMap::new();
    for (_, r) in runs {
        if let Ok(r) = r {
            for t in r.t30.iter().flatten() {
                *out.entry(t.source.clone()).or_default() += 1;
            }
        }
    }
    let total: usize = out.values().sum();
    let noise = out.get("monte_carlo_noise").copied().unwrap_or(0)
        + out.get("noise_uncalibrated").copied().unwrap_or(0);
    (out, (total > 0).then(|| noise as f64 / total as f64))
}

/// Inputs every cell's evaluation shares.
pub struct CellContext<'a> {
    pub bed: &'a BedFile,
    pub transports: &'a Transports,
}

/// Judges one cell on the runs of `bed.seeds` in `reads` (and its extension's, when C is
/// INCONCLUSIVE on them and they are there). A read of `replace` stands in for the seed it names
/// (the say-NO N5).
pub fn evaluate_cell(
    ctx: &CellContext,
    cell: &Cell,
    reads: Option<&BTreeMap<u32, Result<SppsRead, String>>>,
    replace: Option<(u32, &Result<SppsRead, String>)>,
) -> CellReport {
    let bed = ctx.bed;
    let empty = BTreeMap::new();
    let reads = reads.unwrap_or(&empty);
    let missing: Result<SppsRead, String> = Err("no run".into());
    let freqs: Vec<i32> = bed.bands_hz(cell.air).iter().map(|&f| f as i32).collect();
    let room: Option<&Room> = bed.room(&cell.room);
    let receivers = room.map_or(0, |r| r.receivers_m.len());
    let mut runs = runs_of(reads, &bed.seeds, &missing);
    if let Some((seed, r)) = replace {
        for x in &mut runs {
            if x.0 == seed {
                x.1 = r;
            }
        }
    }
    let mut report = CellReport {
        id: cell.id.clone(),
        room: cell.room.clone(),
        alpha: cell.alpha,
        method: cell.method,
        air: cell.air,
        gated: cell.gated,
        particles_per_source: cell.particles_per_source,
        duration_s: cell.duration_s,
        trans_epsilon: cell.trans_epsilon,
        bands_hz: freqs.clone(),
        seeds: runs.iter().map(|(s, r)| seed_run(*s, r)).collect(),
        reference: None,
        e2: Precondition::of(e2_of(&runs)),
        e3: Precondition::default(),
        e6: Precondition::default(),
        a: None,
        b: None,
        c: None,
        extension: None,
        reported: CellReported::default(),
        verdict: Verdict::NotJudged,
        failures: Vec::new(),
    };
    let (sources, share) = sources_of(&runs);
    report.reported.sources = sources;
    report.reported.noise_refusal_share = share;
    let ok: Vec<(u32, &SppsRead)> = runs
        .iter()
        .filter_map(|(s, r)| r.as_ref().ok().map(|r| (*s, r)))
        .collect();
    if ok.is_empty() || room.is_none() {
        report.failures.push(format!(
            "{}: not judged: no run could be read (E2: {})",
            cell.id,
            report.e2.problems.join("; ")
        ));
        return finish(report);
    }
    report.e3 = Precondition::of(e3_of(&ok, &freqs, receivers));
    report.e6 = Precondition::of(e6_of(&ok, &freqs));
    report.reference = Some(reference_of(
        ctx,
        cell,
        room.expect("checked"),
        ok[0].1,
        &freqs,
    ));
    if !(report.e2.holds && report.e3.holds && report.e6.holds) {
        for (name, p) in [("E2", &report.e2), ("E3", &report.e3), ("E6", &report.e6)] {
            if !p.holds {
                report.failures.push(format!(
                    "{}: not judged: {name}: {}",
                    cell.id,
                    p.problems.join("; ")
                ));
            }
        }
        return finish(report);
    }
    let seeds: Vec<u32> = ok.iter().map(|x| x.0).collect();
    let values = values_of(&ok);
    let reference = report.reference.as_ref().expect("set above");
    let kuttruff: Vec<Option<f64>> = reference.bands.iter().map(|b| b.kuttruff_s).collect();
    let eyring: Vec<Option<f64>> = reference.bands.iter().map(|b| b.eyring_s).collect();
    let sabine: Vec<Option<f64>> = reference.bands.iter().map(|b| b.sabine_s).collect();
    // A: against Kuttruff; the say-NO N3 puts plain Eyring in its place, through the code.
    let a = match crate::faults::active() {
        Some(crate::faults::Fault::BedEyringReference) => {
            check_a(&values, &freqs, &eyring, "eyring_s", limits::KUTTRUFF)
        }
        _ => check_a(&values, &freqs, &kuttruff, "kuttruff_s", limits::KUTTRUFF),
    };
    let k: Vec<f64> = kuttruff.iter().map(|x| x.unwrap_or(f64::NAN)).collect();
    let b = check_b(&seeds, &values, &k, limits::SEED_SPREAD);
    report.reported.eyring = Some(reported(check_a(
        &values,
        &freqs,
        &eyring,
        "eyring_s",
        limits::KUTTRUFF,
    )));
    report.reported.sabine = Some(reported(check_a(
        &values,
        &freqs,
        &sabine,
        "sabine_s",
        limits::KUTTRUFF,
    )));
    let (rb, coverage) = receiver_bands(&ok, &freqs);
    report.reported.receiver_bands = rb;
    report.reported.coverage = coverage;
    // R9: the values that came through, against all.
    let mut only = Vec::new();
    let mut all = Vec::new();
    for (_, r) in &ok {
        for rec in &r.t30 {
            for (bi, t) in rec.iter().enumerate() {
                let x = t.t.unwrap_or(f64::NAN) / k[bi] - 1.0;
                all.push(x);
                if t.source == "value" {
                    only.push(x);
                }
            }
        }
    }
    report.reported.mean_all = Some(stats::mean(&all));
    report.reported.mean_values_only = (!only.is_empty()).then(|| stats::mean(&only));
    // C: against the transport.
    let transport: Option<Vec<(f64, f64)>> = reference
        .bands
        .iter()
        .map(|b| b.transport_t.zip(b.transport_se))
        .collect();
    let c = transport
        .as_ref()
        .map(|tr| check_c(&seeds, &values, tr, limits::TRANSPORT));
    if transport.is_none() {
        report.failures.push(format!(
            "{}: C not judged: no transport ({})",
            cell.id,
            reference
                .bands
                .iter()
                .filter_map(|b| b.transport_error.clone())
                .collect::<Vec<_>>()
                .join("; ")
        ));
    }
    // The one extension of an INCONCLUSIVE C, when its runs are there.
    if let (Some(c10), Some(tr)) = (&c, &transport)
        && c10.verdict == Verdict::Inconclusive
        && !bed.extension_seeds.is_empty()
        && bed.extension_seeds.iter().any(|s| reads.contains_key(s))
    {
        let ext = runs_of(reads, &bed.extension_seeds, &missing);
        let mut problems = e2_of(&ext);
        let ext_ok: Vec<(u32, &SppsRead)> = ext
            .iter()
            .filter_map(|(s, r)| r.as_ref().ok().map(|r| (*s, r)))
            .collect();
        problems.extend(e6_of(&ext_ok, &freqs));
        let mut all_runs: Vec<(u32, &SppsRead)> = ok.clone();
        all_runs.extend(ext_ok.iter().copied());
        problems.extend(e3_of(&all_runs, &freqs, receivers));
        let seeds20: Vec<u32> = all_runs.iter().map(|x| x.0).collect();
        let c20 = if problems.is_empty() {
            check_c(&seeds20, &values_of(&all_runs), tr, limits::TRANSPORT)
        } else {
            report.failures.push(format!(
                "{}: C's extension not judged: {}",
                cell.id,
                problems.join("; ")
            ));
            let mut c = c10.clone();
            c.verdict = Verdict::NotJudged;
            c
        };
        report.extension = Some(Extension {
            seeds: ext.iter().map(|(s, r)| seed_run(*s, r)).collect(),
            c_first: c10.clone(),
        });
        report.c = Some(c20);
    } else {
        report.c = c;
    }
    report.a = Some(a);
    report.b = Some(b);
    finish(report)
}

/// A check made for show only.
fn reported(mut a: CheckA) -> CheckA {
    a.verdict = Verdict::Reported;
    for b in &mut a.bands {
        b.verdict = Verdict::Reported;
    }
    a
}

/// A cell's verdict and failures from its checks.
fn finish(mut r: CellReport) -> CellReport {
    let judged = r.e2.holds && r.e3.holds && r.e6.holds && r.a.is_some();
    let verdict = if !judged {
        Verdict::NotJudged
    } else {
        let a = r.a.as_ref().map(|a| a.verdict);
        let b = r.b.as_ref().map(|b| b.verdict);
        let c = r.c.as_ref().map(|c| c.verdict);
        if let Some(a) = &r.a {
            for band in a.bands.iter().filter(|b| b.verdict != Verdict::Pass) {
                r.failures.push(match &band.interval {
                    Some(i) => format!(
                        "{}: A {} Hz {:?}: T30/{} − 1 = {:+.3} %, interval [{:+.3}, {:+.3}] %, \
                         limit ±{:.1} %",
                        r.id,
                        band.freq_hz,
                        band.verdict,
                        a.reference,
                        100.0 * i.mean,
                        100.0 * i.lo,
                        100.0 * i.hi,
                        100.0 * band.limit
                    ),
                    None => format!(
                        "{}: A {} Hz not judged: no {}",
                        r.id, band.freq_hz, a.reference
                    ),
                });
            }
        }
        if let Some(b) = r.b.as_ref().filter(|b| b.verdict != Verdict::Pass) {
            r.failures.push(format!(
                "{}: B {:?}: seed spread of the cell mean {:.3} %, limit {:.1} %",
                r.id,
                b.verdict,
                100.0 * b.spread,
                100.0 * b.limit
            ));
        }
        if let Some(c) = r.c.as_ref().filter(|c| c.verdict != Verdict::Pass) {
            r.failures.push(format!(
                "{}: C {:?} on {} seeds: d = {:+.3} %, interval [{:+.3}, {:+.3}] %, limit ±{:.1} %",
                r.id,
                c.verdict,
                c.seeds.len(),
                100.0 * c.interval.mean,
                100.0 * c.interval.lo,
                100.0 * c.interval.hi,
                100.0 * c.limit
            ));
        }
        match (a, b, c) {
            (Some(Verdict::Pass), Some(Verdict::Pass), Some(Verdict::Pass)) => Verdict::Pass,
            (_, _, Some(Verdict::Inconclusive))
                if a == Some(Verdict::Pass) && b == Some(Verdict::Pass) =>
            {
                Verdict::Inconclusive
            }
            (_, _, None) => Verdict::NotJudged,
            _ => Verdict::Fail,
        }
    };
    r.verdict = if r.gated { verdict } else { Verdict::Reported };
    if !r.gated {
        // A reported cell's failures are information, not failures of the bed.
        r.failures
            .iter_mut()
            .for_each(|f| f.insert_str(0, "(reported) "));
    }
    r
}

/// A cell's references from `first`'s run, with the bed's air term and the transport.
fn reference_of(
    ctx: &CellContext,
    cell: &Cell,
    room: &Room,
    first: &SppsRead,
    freqs: &[i32],
) -> CellReference {
    let rf = &first.reference;
    let bands = freqs
        .iter()
        .enumerate()
        .map(|(i, &f)| {
            let band = rf.bands.iter().find(|b| b.freq_hz == f);
            let bed_air = bed_air(ctx.bed, cell.air, f64::from(f));
            let sabine = band.and_then(|b| {
                sabine_rt(
                    rf.volume_m3,
                    &[Surface {
                        area_m2: rf.area_m2,
                        absorption: b.mean_absorption,
                    }],
                    b.air_m_per_metre,
                    RtConstant::Physical {
                        speed_of_sound: rf.speed_of_sound_m_s,
                    },
                )
                .ok()
            });
            let tr = ctx.transports.get(&room.name, cell.alpha, bed_air);
            let (t, t_err) = match tr {
                Some(Ok(t)) => (Some(t), None),
                Some(Err(e)) => (None, Some(e.clone())),
                None => (None, Some(format!("not traced (band {i})"))),
            };
            let kuttruff = band.and_then(|b| b.kuttruff_s);
            ReferenceBand {
                freq_hz: f,
                kuttruff_s: kuttruff,
                kuttruff_mc_sd: band.and_then(|b| b.kuttruff_mc_sd),
                eyring_s: band.and_then(|b| b.eyring_s),
                sabine_s: sabine,
                air_m_per_metre: band.and_then(|b| b.air_m_per_metre),
                bed_air_m_per_metre: bed_air,
                lambert_walls: band.is_some_and(|b| b.lambert_walls),
                transport_t: t.map(|t| t.t),
                transport_se: t.map(|t| t.se),
                transport_room_t: t.map(|t| t.room_t),
                transport_room_se: t.map(|t| t.room_se),
                transport_error: t_err,
                kuttruff_against_transport: kuttruff.zip(t).map(|(k, t)| k / t.t - 1.0),
            }
        })
        .collect();
    CellReference {
        volume_m3: rf.volume_m3.is_finite().then_some(rf.volume_m3),
        area_m2: rf.area_m2.is_finite().then_some(rf.area_m2),
        gamma2: rf.gamma2,
        gamma2_se: rf.gamma2_se,
        bands,
    }
}

/// Judges one TCR run with check D, and reports R8 against the cell `project_of`'s Kuttruff.
pub fn evaluate_tcr(
    t: &TcrCell,
    read: Option<&Result<TcrRead, String>>,
    kuttruff: Option<&CellReference>,
) -> TcrReport {
    let mut out = TcrReport {
        id: t.id.clone(),
        project_of: t.project_of.clone(),
        gated: t.gated,
        run: None,
        error: None,
        d: None,
        sabine_against_analytic: Vec::new(),
        eyring_against_kuttruff: Vec::new(),
        verdict: Verdict::NotJudged,
        failures: Vec::new(),
    };
    let r = match read {
        Some(Ok(r)) => r,
        Some(Err(e)) => {
            out.error = Some(e.clone());
            out.failures.push(format!("{}: not judged: {e}", t.id));
            return out;
        }
        None => {
            out.error = Some("no run".into());
            out.failures.push(format!("{}: not judged: no run", t.id));
            return out;
        }
    };
    out.run = Some(r.info.clone());
    if r.info.status != "OK" {
        out.failures.push(format!(
            "{}: run {} is {}",
            t.id, r.info.folder, r.info.status
        ));
        return out;
    }
    let d = check_d(
        &r.bands
            .iter()
            .map(|b| (b.freq_hz, b.eyring_s, b.analytic_eyring_s))
            .collect::<Vec<_>>(),
        limits::TCR_EYRING,
    );
    for b in d.bands.iter().filter(|b| b.verdict != Verdict::Pass) {
        out.failures.push(format!(
            "{}: D {} Hz {:?}: TCR Eyring {:.6} s against analytic {:?} s ({:+.4} %), limit \
             ±{:.1} %",
            t.id,
            b.freq_hz,
            b.verdict,
            b.tcr_eyring_s,
            b.analytic_eyring_s,
            100.0 * b.deviation.unwrap_or(f64::NAN),
            100.0 * d.limit
        ));
    }
    out.sabine_against_analytic = r
        .bands
        .iter()
        .map(|b| b.analytic_sabine_s.map(|a| b.sabine_s / a - 1.0))
        .collect();
    out.eyring_against_kuttruff = r
        .bands
        .iter()
        .map(|b| {
            kuttruff
                .and_then(|k| k.bands.iter().find(|x| x.freq_hz == b.freq_hz))
                .and_then(|x| x.kuttruff_s)
                .map(|k| b.eyring_s / k - 1.0)
        })
        .collect();
    out.verdict = if t.gated {
        d.verdict
    } else {
        Verdict::Reported
    };
    if !t.gated {
        out.failures
            .iter_mut()
            .for_each(|f| f.insert_str(0, "(reported) "));
    }
    out.d = Some(d);
    out
}

/// R7: random against energetic, per (room, α, air): the difference of the two cells' means of
/// `T/T_K` over their seeds, with its Welch interval.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct RandomAgainstEnergetic {
    pub room: String,
    pub alpha: f64,
    pub air: bool,
    /// `mean(m_s, random) − mean(m_s, energetic)`.
    pub difference: f64,
    pub se: f64,
    pub dof: f64,
    pub lo: f64,
    pub hi: f64,
}

/// R7 over the cells.
pub fn random_against_energetic(cells: &[CellReport]) -> Vec<RandomAgainstEnergetic> {
    let mut out = Vec::new();
    for r in cells.iter().filter(|c| c.method == Method::Random) {
        let Some(e) = cells.iter().find(|e| {
            e.method == Method::Energetic
                && e.room == r.room
                && e.alpha == r.alpha
                && e.air == r.air
        }) else {
            continue;
        };
        let (Some(rb), Some(eb)) = (&r.b, &e.b) else {
            continue;
        };
        let (x, y) = (&rb.cell_means, &eb.cell_means);
        let (vx, vy) = (
            stats::sd(x).powi(2) / x.len() as f64,
            stats::sd(y).powi(2) / y.len() as f64,
        );
        let se = (vx + vy).sqrt();
        let welch = vx * vx / (x.len() - 1) as f64 + vy * vy / (y.len() - 1) as f64;
        let dof = if welch > 0.0 {
            (vx + vy).powi(2) / welch
        } else {
            (x.len() + y.len() - 2) as f64
        };
        let t = stats::t_quantile(0.975, dof);
        let difference = stats::mean(x) - stats::mean(y);
        out.push(RandomAgainstEnergetic {
            room: r.room.clone(),
            alpha: r.alpha,
            air: r.air,
            difference,
            se,
            dof,
            lo: difference - t * se,
            hi: difference + t * se,
        });
    }
    out
}

/// The transports every cell of `bed` needs, as `(room, α, m)`, each once.
pub fn needed_transports(bed: &BedFile) -> Vec<(Room, f64, Option<f64>)> {
    let mut seen = std::collections::BTreeSet::new();
    let mut out = Vec::new();
    for cell in &bed.cells {
        let Some(room) = bed.room(&cell.room) else {
            continue;
        };
        for (alpha, air) in cell_transport_keys(bed, cell) {
            if seen.insert(transport::key(&room.name, alpha, air)) {
                out.push((room.clone(), alpha, air));
            }
        }
    }
    out
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// `values[s][r][b]` = `truth[b]·(1 + noise)` with a fixed small pattern per seed and
    /// receiver, `seeds` seeds, 3 receivers.
    pub(crate) fn synthetic(truth: &[f64], seeds: usize, spread: f64) -> Vec<Vec<Vec<f64>>> {
        (0..seeds)
            .map(|s| {
                (0..3)
                    .map(|r| {
                        truth
                            .iter()
                            .enumerate()
                            .map(|(b, t)| {
                                let u = ((s * 7 + r * 3 + b * 5) % 11) as f64 / 10.0 - 0.5;
                                t * (1.0 + spread * u)
                            })
                            .collect()
                    })
                    .collect()
            })
            .collect()
    }

    #[test]
    fn a_passes_inside_five_percent_and_fails_a_band_outside() {
        let truth = [2.0, 2.0, 1.5];
        let v = synthetic(&truth, 10, 0.01);
        let freqs = [125, 250, 500];
        let refs: Vec<Option<f64>> = truth.iter().map(|t| Some(*t)).collect();
        let a = check_a(&v, &freqs, &refs, "kuttruff_s", limits::KUTTRUFF);
        assert_eq!(a.verdict, Verdict::Pass, "{a:?}");
        // One band's reference 6 % low: that band fails, and with it A.
        let mut off = refs.clone();
        off[2] = Some(1.5 / 1.06);
        let a = check_a(&v, &freqs, &off, "kuttruff_s", limits::KUTTRUFF);
        assert_eq!(a.verdict, Verdict::Fail);
        assert_eq!(a.bands[2].verdict, Verdict::Fail);
        assert_eq!(a.bands[0].verdict, Verdict::Pass);
        assert!(
            (a.bands[2].interval.unwrap().mean - 0.06).abs() < 0.003,
            "{:?}",
            a.bands[2]
        );
        // The interval counts, not only the mean: a mean of +4.9 % whose interval reaches past
        // 5 % fails.
        let noisy = synthetic(&truth, 10, 0.2);
        let near: Vec<Option<f64>> = truth.iter().map(|t| Some(t / 1.049)).collect();
        let a = check_a(&noisy, &freqs, &near, "kuttruff_s", limits::KUTTRUFF);
        assert!(
            a.bands
                .iter()
                .all(|b| b.interval.unwrap().mean.abs() < 0.05)
        );
        assert_eq!(a.verdict, Verdict::Fail, "{a:?}");
        // A missing reference is not judged, and fails.
        let a = check_a(
            &v,
            &freqs,
            &[Some(2.0), None, Some(1.5)],
            "kuttruff_s",
            0.05,
        );
        assert_eq!(a.bands[1].verdict, Verdict::NotJudged);
        assert_eq!(a.verdict, Verdict::Fail);
    }

    #[test]
    fn b_is_the_range_of_the_cell_means() {
        let truth = [1.0, 1.0];
        let mut v = synthetic(&truth, 10, 0.004);
        let b = check_b(
            &(1..=10).collect::<Vec<_>>(),
            &v,
            &truth,
            limits::SEED_SPREAD,
        );
        assert_eq!(b.verdict, Verdict::Pass, "{b:?}");
        // One seed 2.5 % long everywhere: the range passes 2 %.
        for r in &mut v[9] {
            for t in r.iter_mut() {
                *t *= 1.025;
            }
        }
        let b = check_b(
            &(1..=10).collect::<Vec<_>>(),
            &v,
            &truth,
            limits::SEED_SPREAD,
        );
        assert_eq!(b.verdict, Verdict::Fail);
        assert!(b.spread > 0.02, "{b:?}");
    }

    #[test]
    fn c_has_three_outcomes() {
        let truth = [1.0, 0.8];
        let seeds: Vec<u32> = (1..=10).collect();
        let tr: Vec<(f64, f64)> = truth.iter().map(|t| (*t, t * 0.0002)).collect();
        // Tight and centred: PASS.
        let v = synthetic(&truth, 10, 0.004);
        let c = check_c(&seeds, &v, &tr, limits::TRANSPORT);
        assert_eq!(c.verdict, Verdict::Pass, "{c:?}");
        // Tight and 1 % off: FAIL.
        let off: Vec<(f64, f64)> = truth.iter().map(|t| (t / 1.01, t * 0.0002)).collect();
        let c = check_c(&seeds, &v, &off, limits::TRANSPORT);
        assert_eq!(c.verdict, Verdict::Fail, "{c:?}");
        // Centred but too noisy to tell: INCONCLUSIVE.
        let noisy = synthetic(&truth, 10, 0.08);
        let c = check_c(&seeds, &noisy, &tr, limits::TRANSPORT);
        assert_eq!(c.verdict, Verdict::Inconclusive, "{c:?}");
        // The transport's own error counts: tight seeds, a loose transport.
        let loose: Vec<(f64, f64)> = truth.iter().map(|t| (*t, t * 0.004)).collect();
        let c = check_c(&seeds, &v, &loose, limits::TRANSPORT);
        assert!(
            c.interval.se >= 0.004 && c.verdict != Verdict::Pass,
            "{c:?}"
        );
        // judge_c on its own: an interval that excludes 0 but centres inside 0.5 % is not FAIL.
        let i = Interval::with_se(10, 0.004, 0.001, 0.001);
        assert!(i.excludes_zero());
        assert_eq!(judge_c(&i, 0.005), Verdict::Inconclusive);
    }

    #[test]
    fn d_holds_tcr_to_its_analytic_value() {
        let d = check_d(&[(125, 1.0, Some(1.004)), (250, 0.5, Some(0.5))], 0.005);
        assert_eq!(d.verdict, Verdict::Pass);
        // Says no: TCR's 0.163 against the physical K (+1.23 %).
        let k = 24.0 * std::f64::consts::LN_10 / 343.2;
        let d = check_d(&[(125, 1.0, Some(k / 0.163))], 0.005);
        assert_eq!(d.verdict, Verdict::Fail);
        assert!(
            (d.bands[0].deviation.unwrap() - 0.0123).abs() < 0.0002,
            "{d:?}"
        );
        assert_eq!(check_d(&[(125, 1.0, None)], 0.005).verdict, Verdict::Fail);
    }
}
