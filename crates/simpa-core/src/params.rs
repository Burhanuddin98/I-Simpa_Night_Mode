//! Acoustic parameters over one band's energy histogram, and the analytic references the later
//! gates compare against. Milestone M7; the definitions, sources and every difference from
//! upstream are in `docs/params.md`.
//!
//! - [`decay`]: onset, Schroeder integration, EDT, T20, T30, C50, C80, D50, Ts and SPL, each with
//!   a bound on what the unseen tail after the series' end could change.
//! - [`air`]: ISO 9613-1 attenuation, and the value SPPS and TCR actually use.
//! - [`room`]: Sabine and Eyring as TCR computes them.
//! - [`din18041`]: the group-A target reverberation times.
//!
//! Everything is a pure function. A value that cannot be computed honestly is a typed
//! [`ParamError`] with a stable code, never a number and never a warning. Nothing here is shown
//! to a user before M8's bed passes.

use std::fmt;

use schemars::JsonSchema;
use serde::Serialize;

pub mod air;
pub mod decay;
pub mod din18041;
pub mod room;

/// The refusal codes. Each is a row of `docs/solver-contract.md`, Part B, "Parameter refusals"
/// (`tests/reason_codes_docs.rs`).
pub mod codes {
    /// `dt` is not finite or not positive.
    pub const BAD_TIME_STEP: &str = "params_bad_time_step";
    /// The series is empty, or ends before a window the quantity needs.
    pub const SERIES_TOO_SHORT: &str = "params_series_too_short";
    /// A value is NaN, infinite or negative.
    pub const BAD_ENERGY: &str = "params_bad_energy";
    /// Every value is zero.
    pub const NO_ENERGY: &str = "params_no_energy";
    /// The series is valid but this quantity cannot be read from it honestly.
    pub const NOT_EVALUABLE: &str = "params_not_evaluable";
    /// Series to be aggregated differ in `dt` or length.
    pub const SERIES_MISMATCH: &str = "params_series_mismatch";
    /// An ISO 9613-1 input is out of its physical domain.
    pub const BAD_AIR: &str = "params_bad_air";
    /// A Sabine or Eyring input is out of its physical domain.
    pub const BAD_ROOM: &str = "params_bad_room";
    /// The absorption area plus the air term is zero: the reverberation time is infinite.
    pub const NO_ABSORPTION: &str = "params_no_absorption";
    /// A DIN 18041 volume outside the range the sourced formula covers.
    pub const DIN_OUT_OF_RANGE: &str = "params_din_out_of_range";

    /// Every code, in the order of the documentation table.
    pub const ALL: [&str; 10] = [
        BAD_TIME_STEP,
        SERIES_TOO_SHORT,
        BAD_ENERGY,
        NO_ENERGY,
        NOT_EVALUABLE,
        SERIES_MISMATCH,
        BAD_AIR,
        BAD_ROOM,
        NO_ABSORPTION,
        DIN_OUT_OF_RANGE,
    ];
}

/// What a refusal is about.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case", tag = "quantity")]
pub enum Quantity {
    Spl,
    Edt,
    T20,
    T30,
    /// C_te, `te` in seconds.
    Clarity {
        te_s: f64,
    },
    /// D_te, `te` in seconds.
    Definition {
        te_s: f64,
    },
    CentreTime,
    /// `100·(T30/T20 − 1)`.
    Curvature,
}

impl fmt::Display for Quantity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Quantity::Spl => write!(f, "SPL"),
            Quantity::Edt => write!(f, "EDT"),
            Quantity::T20 => write!(f, "T20"),
            Quantity::T30 => write!(f, "T30"),
            Quantity::Clarity { te_s } => write!(f, "C{}", ms(*te_s)),
            Quantity::Definition { te_s } => write!(f, "D{}", ms(*te_s)),
            Quantity::CentreTime => write!(f, "Ts"),
            Quantity::Curvature => write!(f, "curvature"),
        }
    }
}

fn ms(s: f64) -> String {
    let v = s * 1000.0;
    if (v - v.round()).abs() < 1e-9 {
        format!("{}", v.round())
    } else {
        format!("{v}")
    }
}

/// Why a quantity cannot be read from a valid series.
#[derive(Clone, Debug, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case", tag = "why")]
pub enum NotEvaluable {
    /// The decay, with the unseen tail added, never fell to the bottom of the range.
    RangeNotReached {
        /// The bottom of the evaluation range, dB (negative).
        needed_db: f64,
        /// How far the decay had fallen when the series ended, dB.
        reached_db: f64,
    },
    /// The energy after the series' end could move the value by more than `limit`.
    Truncated {
        /// The value from the series alone. Not reported as the quantity.
        value: f64,
        /// The value with the estimated tail added; `None` when the tail is unbounded (the
        /// series was not decaying at its end), or when the decay curve with the tail has too
        /// few points in the range to fit.
        with_tail: Option<f64>,
        /// The largest difference allowed, in the quantity's unit (relative for decay times).
        limit: f64,
    },
    /// Fewer points than a regression needs lie inside the range.
    TooFewPoints { points: usize, needed: usize },
    /// The fitted slope is not negative.
    NotDecaying,
    /// A window the quantity divides by holds no energy.
    EmptyWindow { from_s: f64 },
}

impl fmt::Display for NotEvaluable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NotEvaluable::RangeNotReached {
                needed_db,
                reached_db,
            } => write!(
                f,
                "range_not_reached: the decay reached {reached_db:.1} dB, the range needs \
                 {needed_db:.1} dB"
            ),
            NotEvaluable::Truncated {
                value,
                with_tail: Some(t),
                limit,
            } => write!(
                f,
                "truncated: {value} from the series, {t} with the unseen tail added; the limit \
                 is {limit}"
            ),
            NotEvaluable::Truncated {
                value,
                with_tail: None,
                ..
            } => write!(
                f,
                "truncated: {value} from the series, and the series is not decaying at its end, \
                 so the tail after it has no bound"
            ),
            NotEvaluable::TooFewPoints { points, needed } => write!(
                f,
                "too_few_points: {points} in the range, at least {needed} needed"
            ),
            NotEvaluable::NotDecaying => {
                write!(f, "not_decaying: the fitted slope is not negative")
            }
            NotEvaluable::EmptyWindow { from_s } => {
                write!(f, "empty_window: no energy after {from_s} s")
            }
        }
    }
}

/// A refusal, with its code ([`ParamError::code`]) and what caused it.
#[derive(Clone, Debug, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum ParamError {
    BadTimeStep {
        dt: f64,
    },
    SeriesTooShort {
        what: String,
        needed_s: f64,
        available_s: f64,
    },
    BadEnergy {
        index: usize,
        value: f64,
    },
    NoEnergy,
    NotEvaluable {
        quantity: Quantity,
        why: NotEvaluable,
    },
    SeriesMismatch {
        detail: String,
    },
    BadAir {
        field: String,
        value: f64,
    },
    BadRoom {
        field: String,
        value: f64,
    },
    NoAbsorption,
    DinOutOfRange {
        group: String,
        volume_m3: f64,
        detail: String,
    },
}

impl ParamError {
    /// The stable code, one of [`codes::ALL`].
    pub fn code(&self) -> &'static str {
        match self {
            ParamError::BadTimeStep { .. } => codes::BAD_TIME_STEP,
            ParamError::SeriesTooShort { .. } => codes::SERIES_TOO_SHORT,
            ParamError::BadEnergy { .. } => codes::BAD_ENERGY,
            ParamError::NoEnergy => codes::NO_ENERGY,
            ParamError::NotEvaluable { .. } => codes::NOT_EVALUABLE,
            ParamError::SeriesMismatch { .. } => codes::SERIES_MISMATCH,
            ParamError::BadAir { .. } => codes::BAD_AIR,
            ParamError::BadRoom { .. } => codes::BAD_ROOM,
            ParamError::NoAbsorption => codes::NO_ABSORPTION,
            ParamError::DinOutOfRange { .. } => codes::DIN_OUT_OF_RANGE,
        }
    }

    /// The reason a quantity was not evaluable, if that is what this is.
    pub fn not_evaluable(&self) -> Option<&NotEvaluable> {
        match self {
            ParamError::NotEvaluable { why, .. } => Some(why),
            _ => None,
        }
    }
}

impl fmt::Display for ParamError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: ", self.code())?;
        match self {
            ParamError::BadTimeStep { dt } => write!(f, "dt = {dt} s is not a positive number"),
            ParamError::SeriesTooShort {
                what,
                needed_s,
                available_s,
            } => write!(
                f,
                "{what} needs {needed_s} s of series, it has {available_s} s"
            ),
            ParamError::BadEnergy { index, value } => {
                write!(
                    f,
                    "value {index} is {value}, not a finite non-negative energy"
                )
            }
            ParamError::NoEnergy => write!(f, "every value is zero"),
            ParamError::NotEvaluable { quantity, why } => write!(f, "{quantity}: {why}"),
            ParamError::SeriesMismatch { detail } => write!(f, "{detail}"),
            ParamError::BadAir { field, value } => write!(f, "{field} = {value}"),
            ParamError::BadRoom { field, value } => write!(f, "{field} = {value}"),
            ParamError::NoAbsorption => write!(
                f,
                "the absorption area plus the air term is zero, so the reverberation time is \
                 infinite"
            ),
            ParamError::DinOutOfRange {
                group,
                volume_m3,
                detail,
            } => write!(f, "{group} at {volume_m3} m³: {detail}"),
        }
    }
}

impl std::error::Error for ParamError {}

pub(crate) fn not_evaluable(quantity: Quantity, why: NotEvaluable) -> ParamError {
    ParamError::NotEvaluable { quantity, why }
}

/// One band's energy histogram: bin `k` holds the energy that arrived in `[k·dt, (k+1)·dt)`,
/// in Pa² as SPPS writes a `.recp` value (`docs/params.md`, "The input").
#[derive(Clone, Debug, PartialEq)]
pub struct EnergySeries {
    dt: f64,
    values: Vec<f64>,
}

impl EnergySeries {
    /// A series, refused when `dt` is not a positive number, `values` is empty, a value is NaN,
    /// infinite or negative, or every value is zero.
    pub fn new(dt: f64, values: Vec<f64>) -> Result<Self, ParamError> {
        if !dt.is_finite() || dt <= 0.0 {
            return Err(ParamError::BadTimeStep { dt });
        }
        if values.is_empty() {
            return Err(ParamError::SeriesTooShort {
                what: "any quantity".into(),
                needed_s: dt,
                available_s: 0.0,
            });
        }
        if let Some((index, &value)) = values
            .iter()
            .enumerate()
            .find(|(_, v)| !v.is_finite() || **v < 0.0)
        {
            return Err(ParamError::BadEnergy { index, value });
        }
        if values.iter().all(|&v| v == 0.0) {
            return Err(ParamError::NoEnergy);
        }
        Ok(EnergySeries { dt, values })
    }

    /// The bin width, s.
    pub fn dt(&self) -> f64 {
        self.dt
    }

    /// The bins.
    pub fn values(&self) -> &[f64] {
        &self.values
    }

    /// Number of bins.
    pub fn len(&self) -> usize {
        self.values.len()
    }

    /// Never true: an empty series is refused.
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    /// The time the series covers, `len·dt`, s.
    pub fn duration(&self) -> f64 {
        self.values.len() as f64 * self.dt
    }

    /// The sum of every bin.
    pub fn total(&self) -> f64 {
        self.values.iter().sum()
    }
}

/// Bands summed bin by bin into one series, labelled by its caller as an aggregate (upstream's
/// `Global` row, `projet_calculation.cpp:898`). Refused when the series differ in `dt` or length,
/// or when there are none.
pub fn aggregate(bands: &[EnergySeries]) -> Result<EnergySeries, ParamError> {
    let Some(first) = bands.first() else {
        return Err(ParamError::SeriesMismatch {
            detail: "no series to aggregate".into(),
        });
    };
    for (i, b) in bands.iter().enumerate().skip(1) {
        if b.dt != first.dt || b.len() != first.len() {
            return Err(ParamError::SeriesMismatch {
                detail: format!(
                    "series {i} has dt = {} s and {} bins, series 0 has dt = {} s and {} bins",
                    b.dt,
                    b.len(),
                    first.dt,
                    first.len()
                ),
            });
        }
    }
    let mut values = vec![0.0; first.len()];
    for b in bands {
        for (acc, v) in values.iter_mut().zip(&b.values) {
            *acc += v;
        }
    }
    EnergySeries::new(first.dt, values)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_series_is_refused_for_each_bad_input() {
        let code = |r: Result<EnergySeries, ParamError>| r.unwrap_err().code();
        assert_eq!(
            code(EnergySeries::new(0.0, vec![1.0])),
            codes::BAD_TIME_STEP
        );
        assert_eq!(
            code(EnergySeries::new(-0.01, vec![1.0])),
            codes::BAD_TIME_STEP
        );
        assert_eq!(
            code(EnergySeries::new(f64::NAN, vec![1.0])),
            codes::BAD_TIME_STEP
        );
        assert_eq!(
            code(EnergySeries::new(f64::INFINITY, vec![1.0])),
            codes::BAD_TIME_STEP
        );
        assert_eq!(
            code(EnergySeries::new(0.01, vec![])),
            codes::SERIES_TOO_SHORT
        );
        // The first bad value is named; NaN never equals NaN, so match on it.
        match EnergySeries::new(0.01, vec![1.0, f64::NAN, -1.0]).unwrap_err() {
            ParamError::BadEnergy { index: 1, value } => assert!(value.is_nan()),
            other => panic!("{other}"),
        }
        assert_eq!(
            EnergySeries::new(0.01, vec![1.0, 2.0, -1e-30]).unwrap_err(),
            ParamError::BadEnergy {
                index: 2,
                value: -1e-30
            }
        );
        assert_eq!(
            code(EnergySeries::new(0.01, vec![1.0, f64::INFINITY])),
            codes::BAD_ENERGY
        );
        assert_eq!(
            code(EnergySeries::new(0.01, vec![0.0; 5])),
            codes::NO_ENERGY
        );
        // And the good one.
        let s = EnergySeries::new(0.01, vec![0.0, 1.0, 0.5]).unwrap();
        assert_eq!((s.len(), s.total()), (3, 1.5));
        assert!((s.duration() - 0.03).abs() < 1e-15);
    }

    #[test]
    fn aggregate_sums_bands_and_refuses_mismatches() {
        let a = EnergySeries::new(0.01, vec![1.0, 2.0]).unwrap();
        let b = EnergySeries::new(0.01, vec![3.0, 0.0]).unwrap();
        assert_eq!(aggregate(&[a.clone(), b]).unwrap().values(), &[4.0, 2.0]);
        let c = EnergySeries::new(0.001, vec![3.0, 0.0]).unwrap();
        assert_eq!(
            aggregate(&[a.clone(), c]).unwrap_err().code(),
            codes::SERIES_MISMATCH
        );
        let d = EnergySeries::new(0.01, vec![3.0]).unwrap();
        assert_eq!(
            aggregate(&[a, d]).unwrap_err().code(),
            codes::SERIES_MISMATCH
        );
        assert_eq!(aggregate(&[]).unwrap_err().code(), codes::SERIES_MISMATCH);
    }

    #[test]
    fn every_error_has_its_own_code_and_a_message() {
        let errors = [
            ParamError::BadTimeStep { dt: 0.0 },
            ParamError::SeriesTooShort {
                what: "C80".into(),
                needed_s: 0.08,
                available_s: 0.05,
            },
            ParamError::BadEnergy {
                index: 0,
                value: -1.0,
            },
            ParamError::NoEnergy,
            not_evaluable(Quantity::T30, NotEvaluable::NotDecaying),
            ParamError::SeriesMismatch { detail: "x".into() },
            ParamError::BadAir {
                field: "f".into(),
                value: 0.0,
            },
            ParamError::BadRoom {
                field: "volume".into(),
                value: 0.0,
            },
            ParamError::NoAbsorption,
            ParamError::DinOutOfRange {
                group: "A5".into(),
                volume_m3: 10.0,
                detail: "x".into(),
            },
        ];
        let got: Vec<&str> = errors.iter().map(ParamError::code).collect();
        assert_eq!(got, codes::ALL);
        for e in &errors {
            assert!(e.to_string().starts_with(e.code()), "{e}");
        }
    }
}
