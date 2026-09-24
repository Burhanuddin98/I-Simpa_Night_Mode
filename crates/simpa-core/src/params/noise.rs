//! Monte-Carlo noise: how far each value of [`decay`] would move between two runs that differ only
//! in their random numbers, estimated from the receiver crossings behind every bin; and the
//! refusal of a value whose noise is too large (`docs/params.md`, "Monte-Carlo noise").
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
//! below `W/N` (`CalculationCore.cpp:57, 141, 259, 284`), so the same `d̄` bounds each deposit and
//! the variance from above: the model is exact in random mode and conservative in energetic mode.
//!
//! **The estimate.** A parametric bootstrap: [`RESAMPLES`] series are drawn from the model around
//! the series, each bin a compound Poisson sum of `E/d̄` expected crossings with chord-distributed
//! deposits (a normal draw of the same mean and variance when more than 30 are expected); every
//! quantity is evaluated on each, taken as it is (complete, nothing missing: the tail, the floor
//! and the lost particles are judged once, on the series itself); the standard deviation over
//! them is the value's estimated noise. A fixed seed ([`SEED`]) makes it repeatable.
//!
//! **The refusal.** A value whose standard deviation is above its limit ([`limits`]), or that more
//! than [`REFUSED_RESAMPLES_ALLOWED`] of the resamples refuse themselves, is refused as
//! `monte_carlo_noise`, with both numbers. A value whose noise has no model ([`NoiseModel::Unknown`])
//! is refused as `noise_unknown`. No value is ever reported with noise nothing bounds.

use schemars::JsonSchema;
use serde::Serialize;

use super::decay::{self, Arrival, Onset};
use super::{EnergySeries, NotEvaluable, ParamError, Quantity, not_evaluable};

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

/// What a series' noise is estimated from.
#[derive(Clone, Debug, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case", tag = "model")]
pub enum NoiseModel {
    /// Receiver crossings, each adding `mean_deposit` on average, in the series' unit; with
    /// several sources, the largest of theirs.
    Crossings { mean_deposit: f64 },
    /// Nothing bounds the noise; every value is refused, `noise_unknown`.
    Unknown { detail: String },
}

impl NoiseModel {
    /// Crossings of mean deposit `mean_deposit`, refused, `params_bad_noise_input`, when it is not
    /// a finite positive number.
    pub fn crossings(mean_deposit: f64) -> Result<Self, ParamError> {
        if !mean_deposit.is_finite() || mean_deposit <= 0.0 {
            return Err(ParamError::BadNoiseInput {
                field: "mean_deposit".into(),
                value: mean_deposit,
            });
        }
        Ok(NoiseModel::Crossings { mean_deposit })
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

/// The eight parameters of `series` from `arrival`, each with its noise under `model`, or its
/// refusal ([module docs](self)). A series `decay` refuses outright (all zero, for instance)
/// refuses all eight with that error.
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
            };
        }
    };
    let (base, onset, decay_arrival) = values(series, arrival);
    // Each resample's eight values; `None` where the resample refuses the quantity.
    let mut samples: Vec<[Option<f64>; 8]> = Vec::new();
    if let NoiseModel::Crossings { mean_deposit } = model
        && base.iter().any(Result::is_ok)
    {
        let mut rng = Rng::new(SEED);
        samples.reserve(RESAMPLES);
        for _ in 0..RESAMPLES {
            let drawn = resample(series.values(), *mean_deposit, &mut rng);
            // A resample is a stand-in for another run's series. The tail, the floor and the lost
            // particles were judged on the series itself; here only the value's spread is wanted,
            // so the resample is taken as it is: complete, nothing missing. (Judging the tail of
            // a resample again would refuse energetic mode for the ragged ends of the random-mode
            // stand-ins, not for its own noise.) Its early reverberation is read as the series'
            // is, so that the resamples give the value the series gives.
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
    }
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
            NoiseModel::Crossings { .. } => {
                let got: Vec<f64> = samples.iter().filter_map(|s| s[i]).collect();
                judge(quantity, value, &got, limit, relative)
            }
        }));
    }
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
    }
}

/// A value against the values its resamples gave (`got`, one per resample that did not refuse).
fn judge(
    quantity: Quantity,
    value: f64,
    got: &[f64],
    limit: f64,
    relative: bool,
) -> Result<Estimate, ParamError> {
    let refused = RESAMPLES - got.len();
    let sd = standard_deviation(got);
    let measure = sd.map(|s| if relative { s / value.abs() } else { s });
    match (sd, measure) {
        (Some(sd), Some(m)) if refused <= REFUSED_RESAMPLES_ALLOWED && m <= limit => {
            Ok(Estimate { value, sd })
        }
        _ => Err(not_evaluable(
            quantity,
            NotEvaluable::MonteCarloNoise {
                value,
                sd,
                limit,
                resamples: RESAMPLES,
                refused_resamples: refused,
            },
        )),
    }
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

/// One series drawn from the model around `values`: bin by bin, a compound Poisson sum of
/// `E/d̄` expected crossings, each depositing `d̄·(3/2)·√u` (a chord over its mean), or above
/// [`NORMAL_ABOVE`] a normal draw of mean `E` and variance `(9/8)·d̄·E`, held at 0.
pub fn resample(values: &[f64], mean_deposit: f64, rng: &mut Rng) -> Vec<f64> {
    values
        .iter()
        .map(|&e| {
            if e <= 0.0 {
                return 0.0;
            }
            let lambda = e / mean_deposit;
            if lambda > NORMAL_ABOVE {
                (e + (CHORD_FACTOR * mean_deposit * e).sqrt() * rng.normal()).max(0.0)
            } else {
                let n = rng.poisson(lambda);
                (0..n)
                    .map(|_| mean_deposit * 1.5 * rng.uniform().sqrt())
                    .sum()
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
            let draws: Vec<f64> = (0..100_000).map(|_| resample(&[e], d, &mut r)[0]).collect();
            let mean = draws.iter().sum::<f64>() / draws.len() as f64;
            let var = standard_deviation(&draws).unwrap().powi(2);
            assert!((mean / e - 1.0).abs() < 0.01, "{e}: mean {mean}");
            let want = CHORD_FACTOR * d * e;
            assert!((var / want - 1.0).abs() < 0.03, "{e}: var {var} vs {want}");
        }
        // An empty bin stays empty.
        assert_eq!(resample(&[0.0], d, &mut r), [0.0]);
    }

    #[test]
    fn a_deposit_that_is_not_a_positive_number_is_refused() {
        for d in [0.0, -1.0, f64::NAN, f64::INFINITY] {
            assert_eq!(
                NoiseModel::crossings(d).unwrap_err().code(),
                super::super::codes::BAD_NOISE_INPUT
            );
        }
        assert!(NoiseModel::crossings(1e-9).is_ok());
    }
}
