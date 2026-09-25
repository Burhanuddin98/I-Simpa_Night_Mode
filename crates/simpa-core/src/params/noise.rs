//! Monte-Carlo noise: how far each value of [`decay`] would move between two runs that differ only
//! in their random numbers, estimated from the receiver crossings behind every bin and calibrated
//! against SPPS's own seed-to-seed spread; and the refusal of a value whose noise is too large
//! (`docs/params.md`, "Monte-Carlo noise").
//!
//! **The model.** SPPS adds, for every particle that crosses a receiver sphere of radius `R`, the
//! particle's energy times its chord through the sphere (`spps/input_output/reportmanager.cpp:
//! 223-224`), and writes the sum times `ρc/V` (`baseReportManager.cpp:183`). In random mode a
//! particle carries its start energy `W/N` (`sppsNantes.cpp:73`) until it is absorbed whole
//! (`CalculationCore.cpp:62-67, 147-155, 288-300`), so one crossing adds `W/N · ℓ · ρc/V`. A beam
//! crossing a sphere uniformly gives chords of density `ℓ/(2R²)` on `[0, 2R]`: mean `4R/3` and
//! `E[ℓ²]/E[ℓ]² = 9/8` ([`CHORD_FACTOR`]). So the mean deposit of one crossing is
//! `d̄ = W·ρc/(N·πR²)`, a bin holding `E` holds about `E/d̄` crossings, and, crossings being
//! Poisson, its variance is `(9/8)·d̄·E`. In energetic mode a particle's energy only ever falls
//! below `W/N` (`CalculationCore.cpp:57, 141, 259, 284`), so the same `d̄` bounds each deposit:
//! the structure is exact in random mode and conservative in energetic mode, where the calibration
//! brings it down to what SPPS shows. (A deposit scaled step by step by the particles' mean energy,
//! from the room table, was the other structure measured for energetic mode; calibrated, it
//! overstated the seeds' spread more, because the particles' energies spread apart as they are
//! absorbed: `docs/investigations/2026-09-25-noise-calibration/`.)
//!
//! **The estimate.** A parametric bootstrap ([`bootstrap`]): [`RESAMPLES`] series are drawn from
//! the model around the series, each bin a compound Poisson sum of `E/d̄` expected crossings with
//! chord-distributed deposits (a normal draw of the same mean and variance when more than 30 are
//! expected); every quantity is evaluated on each, taken as it is (complete, nothing missing: the
//! tail, the floor and the lost particles are judged once, on the series itself); the standard
//! deviation over them is the model's. A fixed seed ([`SEED`]) makes it repeatable.
//!
//! **The calibration** ([`calibration`]). The model leaves out what SPPS does beyond it (a
//! particle's crossings at several times, and in energetic mode the spread of the particles'
//! energies), so each quantity's standard deviation is multiplied by a factor per computation
//! method, measured on real SPPS runs over ten seeds per cell and validated on cells held out of
//! the measurement (`docs/investigations/2026-09-25-noise-calibration/`). The factor is the largest
//! one-sided 95 % upper bound of the ratio of the seeds' spread to the model over the calibration
//! cells, so the calibrated value never claims less noise than SPPS showed there.
//!
//! **The refusal.** A value whose calibrated standard deviation is above its limit ([`limits`]),
//! or that more than [`REFUSED_RESAMPLES_ALLOWED`] of the resamples refuse themselves, is refused
//! as `monte_carlo_noise`, with both numbers and, when the standard deviation is the reason, the
//! particle count that would bring it within its limit from its fall as `1/√N` (measured,
//! `docs/investigations/2026-09-25-noise-calibration/`). A value whose noise has no model
//! ([`NoiseModel::Unknown`]) is refused as `noise_unknown`. No value is ever reported with noise
//! nothing bounds.

use schemars::JsonSchema;
use serde::Serialize;

use super::decay::{self, Arrival, Onset};
use super::{EnergySeries, NotEvaluable, ParamError, ParticleCount, Quantity, not_evaluable};

/// Resampled series per estimate.
pub const RESAMPLES: usize = 200;
/// Resamples that may refuse a quantity before the value is refused: 5 %.
pub const REFUSED_RESAMPLES_ALLOWED: usize = RESAMPLES / 20;
/// `E[ℓ²]/E[ℓ]²` for the chords of a sphere crossed by a uniform beam.
pub const CHORD_FACTOR: f64 = 9.0 / 8.0;
/// The bootstrap's seed: the same series gives the same estimate.
pub const SEED: u64 = 0x4d37_5eed_0000_0001;
/// Above this many expected crossings a bin is drawn from a normal of the same mean and variance.
const NORMAL_ABOVE: f64 = 30.0;
/// The most noise a reported value may carry, as a standard deviation: half the difference limen
/// commonly quoted from ISO 3382-1 Annex A, so that twice it, about a 95 % interval, stays within
/// one limen. T20, T30 and C50 have no limen of their own there; they take EDT's and C80's.
pub mod limits {
    /// EDT, T20, T30: relative, half of 5 %.
    pub const DECAY_RELATIVE: f64 = 0.025;
    /// C50, C80: dB, half of 1 dB.
    pub const CLARITY_DB: f64 = 0.5;
    /// D50: fraction, half of 0.05.
    pub const DEFINITION: f64 = 0.025;
    /// Ts: s, half of 10 ms.
    pub const CENTRE_TIME_S: f64 = 0.005;
    /// SPL: dB, half of the 1 dB quoted for G.
    pub const SPL_DB: f64 = 0.5;
}

/// The model's calibration against SPPS's own seed-to-seed spread
/// (`docs/investigations/2026-09-25-noise-calibration/`): per computation method, the factor each
/// quantity's bootstrap standard deviation is multiplied by, in [`QUANTITY_NAMES`]' order (SPL,
/// EDT, T20, T30, C50, C80, D50, Ts). Each is the largest one-sided 95 % upper bound, over its
/// calibration cells (ten seeds each), of the ratio of the seeds' spread to the model, rounded up
/// to two digits; the pre-registered rule, which the suite re-derives from the committed receipt
/// (`tests/params_noise_calibration.rs`). Round 1 calibrated on seven cells per method and
/// validated on six more; its energetic T20 and T30 failed that validation in a 20 m corridor
/// whose absorption is on its floor, and round 2 re-calibrated those two on all thirteen energetic
/// cells, validated on six new rooms (`PREREGISTER.txt`, "ROUND 2").
pub mod calibration {
    use super::Method;

    /// Random mode: the model is exact in structure, and SPPS's spread is up to 1.43 times it
    /// (T30 in a room whose absorption is on its floor alone, specular: a particle's crossings at
    /// several times), so every factor is above 1. Set by the dead-floor cell (SPL, EDT, T20, C80,
    /// Ts), tutorial 1's materials (T30 at 150,000 particles, C50) and the Lambert box (D50).
    pub const RANDOM: [f64; 8] = [1.3, 1.4, 1.4, 1.6, 1.2, 1.2, 1.2, 1.4];
    /// Energetic mode: the model bounds each deposit by a particle's start energy, and SPPS's
    /// spread is 0.03 to 0.84 times it; set by the dead-floor room (SPL, EDT), whose particles'
    /// energies spread apart the most, the Lambert box at 600,000 particles (C50, C80, D50, Ts),
    /// and, in round 2, the dead-floor corridor (T20, T30).
    pub const ENERGETIC: [f64; 8] = [0.91, 0.59, 0.43, 0.31, 0.85, 0.78, 0.85, 0.62];

    /// The factor for quantity `i` ([`super::QUANTITY_NAMES`]) under `method`. A test build can
    /// scale every factor through a fault seam (`crate::faults::Fault::NoiseCalibrationScaled`),
    /// the say-NO of the validation.
    pub fn factor(method: Method, i: usize) -> f64 {
        let k = match method {
            Method::Random => RANDOM[i],
            Method::Energetic => ENERGETIC[i],
        };
        match crate::faults::active() {
            Some(crate::faults::Fault::NoiseCalibrationScaled { by }) => k * by,
            _ => k,
        }
    }

    /// A refusal names the particle count at which the calibrated standard deviation would be the
    /// limit over this margin. One run's estimate scatters from seed to seed, by about 5 % for
    /// most quantities but by up to 36 % (median over a cell's receiver-bands) for random-mode T20
    /// and T30, so a count named from one run can fall short; the margin is `1 + 1.28·s`, `s` the
    /// largest such median scatter over the calibration cells, rounded up to two digits and never
    /// below 1.1: about a 90 % chance that the estimate the count was named from was not too low
    /// (pre-registered before the validation, rule 5b).
    pub const RANDOM_MARGIN: [f64; 8] = [1.1, 1.1, 1.4, 1.5, 1.1, 1.1, 1.1, 1.1];
    /// Energetic mode's margins, by the same rule (T20 and T30 over round 2's thirteen cells).
    pub const ENERGETIC_MARGIN: [f64; 8] = [1.1, 1.1, 1.2, 1.3, 1.1, 1.1, 1.1, 1.1];

    /// The margin for quantity `i` under `method`.
    pub fn margin(method: Method, i: usize) -> f64 {
        match method {
            Method::Random => RANDOM_MARGIN[i],
            Method::Energetic => ENERGETIC_MARGIN[i],
        }
    }

    /// Whether the seeds' spread of quantity `i` fell as `1/√N` between every pair of cells that
    /// differ only in their particle count, within two joint standard errors (rule 4). Where it
    /// did not, a refusal names no count: the count would rest on a scaling the data did not
    /// confirm.
    pub const RANDOM_ROOT_N: [bool; 8] = [true, true, true, false, true, true, true, true];
    /// Energetic mode's.
    pub const ENERGETIC_ROOT_N: [bool; 8] = [true, false, true, true, true, true, true, true];

    /// Whether a refusal of quantity `i` under `method` names a particle count.
    pub fn root_n_confirmed(method: Method, i: usize) -> bool {
        match method {
            Method::Random => RANDOM_ROOT_N[i],
            Method::Energetic => ENERGETIC_ROOT_N[i],
        }
    }
}

/// The eight quantities' names in the JSON, in the order of [`Parameters`] and of
/// [`calibration`]'s factors.
pub const QUANTITY_NAMES: [&str; 8] = [
    "spl_db", "edt_s", "t20_s", "t30_s", "c50_db", "c80_db", "d50", "ts_s",
];

/// SPPS's computation method (`computation_method`): how a crossing's deposit is modelled, and
/// which of [`calibration`]'s factors apply.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Method {
    /// Code 0: a particle keeps its start energy until it is absorbed whole.
    Random,
    /// Code 1: a particle's energy falls at every reflection.
    Energetic,
}

/// What a series' noise is estimated from.
#[derive(Clone, Debug, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case", tag = "model")]
pub enum NoiseModel {
    /// Receiver crossings, each adding `mean_deposit` on average, in the series' unit (with
    /// several sources, the largest of theirs).
    Crossings {
        mean_deposit: f64,
        /// Which calibration applies.
        method: Method,
        /// Particles per source (`nbparticules`), from which a refusal names the count that
        /// would bring its value within its limit; `null` when not known.
        particles: Option<u32>,
    },
    /// Nothing bounds the noise; every value is refused, `noise_unknown`.
    Unknown { detail: String },
}

impl NoiseModel {
    /// Crossings of mean deposit `mean_deposit` under `method`, `particles` per source when known;
    /// refused, `params_bad_noise_input`, when the deposit is not a finite positive number.
    pub fn crossings(
        mean_deposit: f64,
        method: Method,
        particles: Option<u32>,
    ) -> Result<Self, ParamError> {
        if !mean_deposit.is_finite() || mean_deposit <= 0.0 {
            return Err(ParamError::BadNoiseInput {
                field: "mean_deposit".into(),
                value: mean_deposit,
            });
        }
        Ok(NoiseModel::Crossings {
            mean_deposit,
            method,
            particles,
        })
    }
}

/// A value and its estimated Monte-Carlo standard deviation, both in the quantity's unit.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, JsonSchema)]
pub struct Estimate {
    pub value: f64,
    pub sd: f64,
}

/// SPL, EDT, T20, T30, C50, C80, D50 and Ts of one series, each with its noise or its refusal.
#[derive(Clone, Debug, PartialEq)]
pub struct Parameters {
    /// The onset bin; `None` when the series itself is refused.
    pub onset: Option<Onset>,
    /// What EDT, T20 and T30 were measured from (`decay::BandParameters::decay_arrival`); `None`
    /// when the series itself is refused.
    pub decay_arrival: Option<Arrival>,
    pub spl_db: Result<Estimate, ParamError>,
    pub edt_s: Result<Estimate, ParamError>,
    pub t20_s: Result<Estimate, ParamError>,
    pub t30_s: Result<Estimate, ParamError>,
    pub c50_db: Result<Estimate, ParamError>,
    pub c80_db: Result<Estimate, ParamError>,
    /// A fraction.
    pub d50: Result<Estimate, ParamError>,
    pub ts_s: Result<Estimate, ParamError>,
    /// `100·(T30/T20 − 1)`, %, from the reported T20 and T30, its standard deviation over the
    /// resamples that give both, calibrated with the larger of T20's and T30's factors; refused,
    /// with T30's refusal or else T20's, when either is ([`decay::curvature`]). No limit of its
    /// own: its noise follows from theirs.
    pub curvature_percent: Result<Estimate, ParamError>,
}

/// The eight quantities in [`Parameters`]' order, with their limits: `(quantity, limit,
/// relative)`.
const QUANTITIES: [(Quantity, f64, bool); 8] = [
    (Quantity::Spl, limits::SPL_DB, false),
    (Quantity::Edt, limits::DECAY_RELATIVE, true),
    (Quantity::T20, limits::DECAY_RELATIVE, true),
    (Quantity::T30, limits::DECAY_RELATIVE, true),
    (Quantity::Clarity { te_s: 0.05 }, limits::CLARITY_DB, false),
    (Quantity::Clarity { te_s: 0.08 }, limits::CLARITY_DB, false),
    (
        Quantity::Definition { te_s: 0.05 },
        limits::DEFINITION,
        false,
    ),
    (Quantity::CentreTime, limits::CENTRE_TIME_S, false),
];

/// Whether quantity `i` ([`QUANTITY_NAMES`]) has a relative standard deviation (the decay times).
pub fn relative(i: usize) -> bool {
    QUANTITIES[i].2
}

/// The eight values of `decay` on one series, in [`QUANTITIES`]' order. A given arrival that does
/// not fit the onset bin refuses C50, C80, D50 and Ts only (`params_bad_arrival`,
/// [`decay::evaluate`]).
fn values(
    series: &EnergySeries,
    arrival: Arrival,
) -> ([Result<f64, ParamError>; 8], Onset, Arrival) {
    let p = decay::evaluate(series, arrival);
    (
        [
            p.spl_db,
            p.edt.map(|f| f.t_s),
            p.t20.map(|f| f.t_s),
            p.t30.map(|f| f.t_s),
            p.c50_db,
            p.c80_db,
            p.d50,
            p.ts_s,
        ],
        p.onset,
        p.decay_arrival,
    )
}

/// The model's own resamples of `series`: each one's eight values, `None` where the resample
/// refuses the quantity. Empty for an unknown model.
fn resamples(series: &EnergySeries, arrival: Arrival, model: &NoiseModel) -> Vec<[Option<f64>; 8]> {
    let mut samples = Vec::new();
    if let NoiseModel::Unknown { .. } = model {
        return samples;
    }
    let mut rng = Rng::new(SEED);
    samples.reserve(RESAMPLES);
    for _ in 0..RESAMPLES {
        let drawn = resample(series.values(), model, &mut rng);
        // A resample is a stand-in for another run's series. The tail, the floor and the lost
        // particles were judged on the series itself; here only the value's spread is wanted, so
        // the resample is taken as it is: complete, nothing missing. (Judging the tail of a
        // resample again would refuse energetic mode for the ragged ends of the random-mode
        // stand-ins, not for its own noise.) Its early reverberation is read as the series' is,
        // so that the resamples give the value the series gives.
        let resampled = EnergySeries::complete(series.dt(), drawn).map(|s| {
            if series.early_reverberation_unresolved() {
                s.with_early_reverberation_unresolved()
            } else {
                s
            }
        });
        samples.push(match resampled {
            Ok(s) => values(&s, arrival).0.map(|r| r.ok()),
            Err(_) => [None; 8],
        });
    }
    samples
}

/// The model's standard deviation of each of the eight quantities, before calibration, in their
/// unit (not relative), with how many resamples refused each: what [`evaluate`] calibrates and
/// judges. `None` where fewer than two resamples give a value, and for every quantity of an
/// unknown model.
pub fn bootstrap(
    series: &EnergySeries,
    arrival: Arrival,
    model: &NoiseModel,
) -> [(Option<f64>, usize); 8] {
    let samples = resamples(series, arrival, model);
    std::array::from_fn(|i| {
        let got: Vec<f64> = samples.iter().filter_map(|s| s[i]).collect();
        (standard_deviation(&got), RESAMPLES - got.len())
    })
}

/// The eight parameters of `series` from `arrival`, each with its calibrated noise under `model`,
/// or its refusal ([module docs](self)). A series `decay` refuses outright (all zero, for
/// instance) refuses all eight with that error.
pub fn evaluate(
    series: &Result<EnergySeries, ParamError>,
    arrival: Arrival,
    model: &NoiseModel,
) -> Parameters {
    let series = match series {
        Ok(s) => s,
        Err(e) => {
            let r = || Err(e.clone());
            return Parameters {
                onset: None,
                decay_arrival: None,
                spl_db: r(),
                edt_s: r(),
                t20_s: r(),
                t30_s: r(),
                c50_db: r(),
                c80_db: r(),
                d50: r(),
                ts_s: r(),
                curvature_percent: r(),
            };
        }
    };
    let (base, onset, decay_arrival) = values(series, arrival);
    let samples = if base.iter().any(Result::is_ok) {
        resamples(series, arrival, model)
    } else {
        Vec::new()
    };
    let mut out: Vec<Result<Estimate, ParamError>> = Vec::with_capacity(8);
    for (i, (b, (quantity, limit, relative))) in base.into_iter().zip(QUANTITIES).enumerate() {
        out.push(b.and_then(|value| match model {
            NoiseModel::Unknown { detail } => Err(not_evaluable(
                quantity,
                NotEvaluable::NoiseUnknown {
                    value,
                    detail: detail.clone(),
                },
            )),
            NoiseModel::Crossings {
                method, particles, ..
            } => {
                let got: Vec<f64> = samples.iter().filter_map(|s| s[i]).collect();
                judge(
                    quantity,
                    value,
                    &got,
                    Judged {
                        limit,
                        relative,
                        factor: calibration::factor(*method, i),
                        margin: calibration::root_n_confirmed(*method, i)
                            .then(|| calibration::margin(*method, i)),
                        particles: *particles,
                    },
                )
            }
        }));
    }
    // The curvature of the reported T20 and T30, with its spread over the resamples giving both.
    let curvature_percent = match (&out[2], &out[3]) {
        // T30's refusal first, then T20's.
        (_, Err(e)) | (Err(e), _) => Err(match e {
            ParamError::NotEvaluable { why, .. } => not_evaluable(Quantity::Curvature, why.clone()),
            other => other.clone(),
        }),
        (Ok(t20), Ok(t30)) => {
            let percent = |t20: f64, t30: f64| 100.0 * (t30 / t20 - 1.0);
            let got: Vec<f64> = samples
                .iter()
                .filter_map(|s| Some(percent(s[2]?, s[3]?)))
                .collect();
            let value = percent(t20.value, t30.value);
            let factor = match model {
                NoiseModel::Crossings { method, .. } => {
                    calibration::factor(*method, 2).max(calibration::factor(*method, 3))
                }
                NoiseModel::Unknown { .. } => 1.0,
            };
            // Both passed their own judgement, so at least 180 resamples give both. Were there
            // fewer than two, the two (calibrated) spreads added as if independent would stand
            // in.
            let sd = standard_deviation(&got).map_or_else(
                || 100.0 * (t30.value / t20.value) * (t30.sd / t30.value).hypot(t20.sd / t20.value),
                |s| s * factor,
            );
            Ok(Estimate { value, sd })
        }
    };
    let mut it = out.into_iter();
    let mut next = || it.next().expect("eight quantities");
    Parameters {
        onset: Some(onset),
        decay_arrival: Some(decay_arrival),
        spl_db: next(),
        edt_s: next(),
        t20_s: next(),
        t30_s: next(),
        c50_db: next(),
        c80_db: next(),
        d50: next(),
        ts_s: next(),
        curvature_percent,
    }
}

/// How one quantity is judged: its limit (relative for the decay times), the calibration factor,
/// the margin a named particle count takes (`None`: no count is named, its `1/√N` fall not
/// confirmed), and the run's particles per source when known.
struct Judged {
    limit: f64,
    relative: bool,
    factor: f64,
    margin: Option<f64>,
    particles: Option<u32>,
}

/// A value against the values its resamples gave (`got`, one per resample that did not refuse).
fn judge(quantity: Quantity, value: f64, got: &[f64], j: Judged) -> Result<Estimate, ParamError> {
    let refused = RESAMPLES - got.len();
    let sd = standard_deviation(got).map(|s| s * j.factor);
    let measure = sd.map(|s| if j.relative { s / value.abs() } else { s });
    match (sd, measure) {
        (Some(sd), Some(m)) if refused <= REFUSED_RESAMPLES_ALLOWED && m <= j.limit => {
            Ok(Estimate { value, sd })
        }
        _ => {
            let particle_count = match (measure, j.margin) {
                (None, _) => ParticleCount::NoStandardDeviation,
                (Some(m), _) if m <= j.limit => ParticleCount::WithinLimit,
                (Some(_), None) => ParticleCount::ScalingNotConfirmed,
                (Some(m), Some(margin)) => {
                    let factor = (margin * m / j.limit).powi(2);
                    ParticleCount::Named {
                        factor,
                        margin,
                        particles: j
                            .particles
                            .map(|n| round_up_two_digits(factor * f64::from(n))),
                    }
                }
            };
            Err(not_evaluable(
                quantity,
                NotEvaluable::MonteCarloNoise {
                    value,
                    sd,
                    limit: j.limit,
                    resamples: RESAMPLES,
                    refused_resamples: refused,
                    particle_count,
                },
            ))
        }
    }
}

/// `x` rounded up to two significant digits: 1,234,567 to 1,300,000.
pub fn round_up_two_digits(x: f64) -> u64 {
    if !x.is_finite() || x <= 0.0 {
        return 0;
    }
    let step = 10f64.powi((x.log10().floor() as i32 - 1).max(0));
    ((x / step).ceil() * step).min(u64::MAX as f64) as u64
}

/// The sample standard deviation (`n − 1`); `None` for fewer than two values.
pub fn standard_deviation(v: &[f64]) -> Option<f64> {
    if v.len() < 2 {
        return None;
    }
    let n = v.len() as f64;
    let mean = v.iter().sum::<f64>() / n;
    Some((v.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / (n - 1.0)).sqrt())
}

/// One series drawn from `model` around `values`: bin by bin, a compound Poisson sum of `E/d̄`
/// expected crossings of mean deposit `d̄`, each depositing `d̄·(3/2)·√u` (a chord over its mean),
/// or above [`NORMAL_ABOVE`] a normal draw of mean `E` and variance `(9/8)·d̄·E`, held at 0. An
/// unknown model returns the series unchanged.
pub fn resample(values: &[f64], model: &NoiseModel, rng: &mut Rng) -> Vec<f64> {
    let NoiseModel::Crossings {
        mean_deposit: d, ..
    } = *model
    else {
        return values.to_vec();
    };
    values
        .iter()
        .map(|&e| {
            if e <= 0.0 {
                return 0.0;
            }
            let lambda = e / d;
            if lambda > NORMAL_ABOVE {
                (e + (CHORD_FACTOR * d * e).sqrt() * rng.normal()).max(0.0)
            } else {
                let n = rng.poisson(lambda);
                (0..n).map(|_| d * 1.5 * rng.uniform().sqrt()).sum()
            }
        })
        .collect()
}

/// SplitMix64: small, fast and reproducible; the bootstrap needs no more.
#[derive(Clone, Debug)]
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Rng(seed)
    }

    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    /// Uniform on `[0, 1)`.
    pub fn uniform(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }

    /// Standard normal (Box–Muller).
    pub fn normal(&mut self) -> f64 {
        let u1 = 1.0 - self.uniform();
        let u2 = self.uniform();
        (-2.0 * u1.ln()).sqrt() * (2.0 * std::f64::consts::PI * u2).cos()
    }

    /// Poisson of mean `lambda` (Knuth's product method; meant for `lambda` up to about 30).
    pub fn poisson(&mut self, lambda: f64) -> u64 {
        let limit = (-lambda).exp();
        let mut k = 0;
        let mut p = self.uniform();
        while p > limit {
            k += 1;
            p *= self.uniform();
        }
        k
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn random(d: f64) -> NoiseModel {
        NoiseModel::crossings(d, Method::Random, None).unwrap()
    }

    #[test]
    fn the_generators_have_their_moments() {
        let mut r = Rng::new(7);
        let n = 200_000;
        let u: Vec<f64> = (0..n).map(|_| r.uniform()).collect();
        let m = u.iter().sum::<f64>() / n as f64;
        assert!((m - 0.5).abs() < 0.005, "{m}");
        let z: Vec<f64> = (0..n).map(|_| r.normal()).collect();
        let sd = standard_deviation(&z).unwrap();
        assert!((sd - 1.0).abs() < 0.01, "{sd}");
        for lambda in [0.3, 4.0, 25.0] {
            let k: Vec<f64> = (0..n).map(|_| r.poisson(lambda) as f64).collect();
            let mean = k.iter().sum::<f64>() / n as f64;
            let var = standard_deviation(&k).unwrap().powi(2);
            assert!((mean / lambda - 1.0).abs() < 0.02, "{lambda}: mean {mean}");
            assert!((var / lambda - 1.0).abs() < 0.03, "{lambda}: var {var}");
        }
    }

    #[test]
    fn a_resampled_bin_has_the_models_mean_and_variance() {
        // One bin of 5 and one of 500 expected crossings: the Poisson and the normal branch.
        let d = 2.0;
        let mut r = Rng::new(11);
        for e in [10.0, 1000.0] {
            let draws: Vec<f64> = (0..100_000)
                .map(|_| resample(&[e], &random(d), &mut r)[0])
                .collect();
            let mean = draws.iter().sum::<f64>() / draws.len() as f64;
            let var = standard_deviation(&draws).unwrap().powi(2);
            assert!((mean / e - 1.0).abs() < 0.01, "{e}: mean {mean}");
            let want = CHORD_FACTOR * d * e;
            assert!((var / want - 1.0).abs() < 0.03, "{e}: var {var} vs {want}");
        }
        // An empty bin stays empty.
        assert_eq!(resample(&[0.0], &random(d), &mut r), [0.0]);
    }

    #[test]
    fn a_deposit_that_is_not_a_positive_number_is_refused() {
        for d in [0.0, -1.0, f64::NAN, f64::INFINITY] {
            assert_eq!(
                NoiseModel::crossings(d, Method::Random, None)
                    .unwrap_err()
                    .code(),
                super::super::codes::BAD_NOISE_INPUT
            );
        }
        assert!(NoiseModel::crossings(1e-9, Method::Energetic, Some(5)).is_ok());
    }

    #[test]
    fn two_significant_digits_round_up() {
        assert_eq!(round_up_two_digits(1_234_567.0), 1_300_000);
        assert_eq!(round_up_two_digits(1_200_000.0), 1_200_000);
        assert_eq!(round_up_two_digits(150_001.0), 160_000);
        assert_eq!(round_up_two_digits(99.2), 100);
        assert_eq!(round_up_two_digits(7.1), 8);
        assert_eq!(round_up_two_digits(0.0), 0);
    }
}
