//! Acoustic parameters over one band's energy histogram, and the analytic references the later
//! gates compare against. Milestone M7; the definitions, sources and every difference from
//! upstream are in `docs/params.md`.
//!
//! - [`decay`]: onset, Schroeder integration, EDT, T20, T30, C50, C80, D50, Ts and SPL, each with
//!   a bound on what the unseen tail after the series' end could change, on what energy the solver
//!   dropped below its floor could change ([`EnergySeries::with_solver_floor`]), and, when the
//!   direct sound's arrival is not given, on what its place inside the onset bin could change.
//! - [`noise`]: the Monte-Carlo noise of each of those values, estimated from the number of
//!   receiver crossings behind every bin, and the refusal of a value whose noise is too large.
//! - [`air`]: ISO 9613-1 attenuation, and the value SPPS and TCR actually use.
//! - [`room`]: Sabine and Eyring as TCR computes them, and Kuttruff's correction of Eyring with the
//!   free paths' relative variance `γ²`.
//! - [`lambert`]: a diffuse (Lambert) ray transport written from scratch: the mean free path and
//!   `γ²` of a room from its geometry alone, and its decay, for M8's reference and cross-check.
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
pub mod lambert;
pub mod noise;
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
    /// A given direct-arrival time is not a time, or lies outside the series' onset bin.
    pub const BAD_ARRIVAL: &str = "params_bad_arrival";
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
    /// A solver floor, or a Monte-Carlo deposit, that is not a finite number in its domain.
    pub const BAD_NOISE_INPUT: &str = "params_bad_noise_input";
    /// The diffuse ray transport (`params::lambert`) cannot run, or refuses its own result.
    pub const TRANSPORT_REFUSED: &str = "params_transport_refused";
    /// Kuttruff's reference is not computed for a band whose faces do not all reflect by
    /// Lambert's law with scattering 1, the only walls the transport's `γ²` describes; and the
    /// transport is not run when no computed band has such walls.
    pub const REFERENCE_NOT_APPLICABLE: &str = "params_reference_not_applicable";

    /// Every code, in the order of the documentation table.
    pub const ALL: [&str; 14] = [
        BAD_TIME_STEP,
        SERIES_TOO_SHORT,
        BAD_ENERGY,
        NO_ENERGY,
        BAD_ARRIVAL,
        NOT_EVALUABLE,
        SERIES_MISMATCH,
        BAD_AIR,
        BAD_ROOM,
        NO_ABSORPTION,
        DIN_OUT_OF_RANGE,
        BAD_NOISE_INPUT,
        TRANSPORT_REFUSED,
        REFERENCE_NOT_APPLICABLE,
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

/// The particle count a refusal for Monte-Carlo noise names, or why it names none.
#[derive(Clone, Debug, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case", tag = "count")]
pub enum ParticleCount {
    /// `factor` times the run's particles would bring the calibrated standard deviation to the
    /// limit over `margin`, from its fall as `1/√N`: `(margin·sd/limit)²`
    /// ([`noise::calibration::margin`]). `particles` is that many per source, rounded up to two
    /// significant digits, when the run's count is known.
    Named {
        factor: f64,
        margin: f64,
        particles: Option<u64>,
    },
    /// More than [`noise::REFUSED_RESAMPLES_ALLOWED`] resamples refuse the value (rule R4-3):
    /// `multiple` times the run's particles, the smallest of [`noise::RESAMPLED_MULTIPLES`] at which
    /// the model's own resamples of the series, every deposit over `multiple`, refuse it at most
    /// [`noise::RESAMPLED_REFUSALS_NAMED`] times and its calibrated standard deviation times
    /// `margin` is within the limit. `particles` is that many per source, when the run's count is
    /// known.
    Resampled {
        multiple: u32,
        margin: f64,
        particles: Option<u64>,
    },
    /// More than [`noise::REFUSED_RESAMPLES_ALLOWED`] resamples refuse the value, and they still
    /// do, or its standard deviation is still above the limit, at `multiple` times the run's
    /// particles, the largest tried (R4-3): no count is named, and more particles may not help.
    BeyondResampled { multiple: u32 },
    /// More than [`noise::REFUSED_RESAMPLES_ALLOWED`] resamples refuse the value, and for this
    /// quantity in this computation method the counts its resamples named were not borne out at
    /// a higher count (round 4, F8): no count is named.
    ResampledNotConfirmed,
    /// The fall of the seeds' spread as `1/√N` was not confirmed for this quantity in the run's
    /// computation method ([`noise::calibration::root_n_confirmed`]), so no count is named.
    ScalingNotConfirmed,
    /// Fewer than two resamples give a value: there is no standard deviation to scale.
    NoStandardDeviation,
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
        /// series was not decaying at its end), or when the decay curve with the tail spends too
        /// little time in the range to fit.
        with_tail: Option<f64>,
        /// The largest difference allowed, in the quantity's unit (relative for decay times).
        limit: f64,
    },
    /// The arrival was not given, and where in the onset bin it lies moves the value by more
    /// than `limit`: `low` and `high` are the values with the arrival at the two ends of the bin.
    Unresolved {
        /// Midway between `low` and `high`. Not reported as the quantity.
        value: f64,
        low: f64,
        high: f64,
        /// The largest distance allowed from `value` to either end, in the quantity's unit
        /// (relative for decay times).
        limit: f64,
    },
    /// The decay curve spends less time inside the range than a regression needs.
    RangeTooShort { span_s: f64, needed_s: f64 },
    /// The fitted slope is not negative.
    NotDecaying,
    /// A window the quantity divides by holds no energy.
    EmptyWindow { from_s: f64 },
    /// Energy is missing from the series: the solver dropped each particle once its energy fell to
    /// `floor_db` below its start ([`EnergySeries::with_solver_floor`]), or lost particles
    /// mid-path ([`EnergySeries::with_lost_share`]). With the most energy that can have cost
    /// added, the decay is not shown to fall to the bottom of the range.
    MissingNotCleared {
        floor_db: Option<f64>,
        lost_share: Option<f64>,
        /// The bottom of the evaluation range, dB.
        needed_db: f64,
        /// How far the decay had fallen, the unseen tail and the missing energy included, dB.
        reached_db: f64,
    },
    /// With the most energy the solver's floor or its lost particles can have cost added, the
    /// value moves by more than `limit`.
    MissingMoves {
        floor_db: Option<f64>,
        lost_share: Option<f64>,
        /// The value from the series. Not reported as the quantity.
        value: f64,
        /// The value with the missing energy added; `None` when that curve cannot be fitted.
        /// When the lost share follows the decay, the value moved by the most that share can
        /// move it.
        with_missing: Option<f64>,
        limit: f64,
    },
    /// The series' early reverberation is not resolved
    /// ([`EnergySeries::with_early_reverberation_unresolved`]), and where it begins moves the
    /// value by more than `limit`: `continued` is the value with the reverberation beginning at
    /// the arrival (the decay of the first bin wholly after the direct sound continued back to
    /// it), and `low` and `high` the lowest and highest over that and the reverberation beginning
    /// at the start or at the end of that bin (`None` when one of those curves gives no value).
    EarlyUnresolved {
        /// Midway between `low` and `high`. Not reported as the quantity.
        value: f64,
        continued: f64,
        low: Option<f64>,
        high: Option<f64>,
        /// The largest distance allowed from `value` to either, in the quantity's unit (relative
        /// for decay times).
        limit: f64,
    },
    /// The value's Monte-Carlo standard deviation, estimated from the receiver crossings behind
    /// each bin and calibrated against SPPS's own seed-to-seed spread ([`noise`]), is above
    /// `limit`; or more than [`noise::REFUSED_RESAMPLES_ALLOWED`] of the resampled series refuse
    /// the quantity themselves.
    MonteCarloNoise {
        /// The value from the series. Not reported as the quantity.
        value: f64,
        /// Calibrated, in the quantity's unit. `None` when fewer than two resampled series give
        /// a value.
        sd: Option<f64>,
        /// In the quantity's unit (relative for decay times).
        limit: f64,
        resamples: usize,
        refused_resamples: usize,
        /// The particle count that would bring the value within its limit, or why none is named.
        particle_count: ParticleCount,
    },
    /// The series' Monte-Carlo noise cannot be estimated, so nothing bounds it.
    NoiseUnknown {
        /// The value from the series. Not reported as the quantity.
        value: f64,
        detail: String,
    },
    /// The run lies outside what the noise model's calibration measured for this quantity
    /// ([`noise::calibration`]): fewer particles per source than its fewest, or more crossings of
    /// a receiver per particle than its most. The model's standard deviation is not known to
    /// bound the noise there, so the value is refused whatever it reads.
    NoiseUncalibrated {
        /// The value from the series. Not reported as the quantity.
        value: f64,
        /// The run's particles per source.
        particles: u32,
        /// The run's crossings of the receiver per particle, as the calibration measures them
        /// ([`noise::calibration::variable`]).
        crossings_per_particle: f64,
        /// The fewest particles per source the calibration measured this quantity at.
        min_particles: u32,
        /// The most crossings per particle it measured this quantity at.
        max_crossings_per_particle: f64,
        /// When the run has too few particles: `min_particles`, the count to run at least.
        particles_at_least: Option<u32>,
        /// When the run has too many crossings per particle: the most the receiver radius may be,
        /// as a multiple of the run's (crossings per particle grow as its square):
        /// `√(max_crossings_per_particle / crossings_per_particle)`.
        receiver_radius_scale_at_most: Option<f64>,
    },
    /// More than one source contributes to the series. ISO 3382-1 defines the onset-relative
    /// quantities per source–receiver pair, and a sum of several sources' responses is not one.
    /// Made by `core::results`, which knows the sources; `params` never sees them.
    SeveralSources { sources: Vec<String> },
    /// The solver wrote no energy time series for the receiver, so there is nothing to compute the
    /// quantity from. Made by `core::results` for TCR, whose receiver tables hold steady-state
    /// levels only; `params` never sees such a receiver.
    NoTimeSeries {
        /// The solver, and where its own values for the receiver are.
        detail: String,
    },
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
            NotEvaluable::Unresolved {
                value,
                low,
                high,
                limit,
            } => write!(
                f,
                "unresolved: {low} to {high} with the arrival at either end of the onset bin \
                 (midway {value}); the limit is {limit}. Give the arrival time, or use a finer \
                 time step"
            ),
            NotEvaluable::RangeTooShort { span_s, needed_s } => write!(
                f,
                "range_too_short: the curve spends {span_s} s in the range, at least {needed_s} s \
                 needed"
            ),
            NotEvaluable::NotDecaying => {
                write!(f, "not_decaying: the fitted slope is not negative")
            }
            NotEvaluable::EmptyWindow { from_s } => {
                write!(f, "empty_window: no energy after {from_s} s")
            }
            NotEvaluable::MissingNotCleared {
                floor_db,
                lost_share,
                needed_db,
                reached_db,
            } => write!(
                f,
                "missing_not_cleared: with what {} can have cost added, the decay reached \
                 {reached_db:.1} dB, the range needs {needed_db:.1} dB",
                missing_cause(*floor_db, *lost_share)
            ),
            NotEvaluable::MissingMoves {
                floor_db,
                lost_share,
                value,
                with_missing,
                limit,
            } => {
                let cause = missing_cause(*floor_db, *lost_share);
                match with_missing {
                    Some(w) => write!(
                        f,
                        "missing_moves: {value} from the series, {w} with what {cause} can have \
                         cost added; the limit is {limit}"
                    ),
                    None => write!(
                        f,
                        "missing_moves: {value} from the series, and with what {cause} can have \
                         cost added the curve cannot be fitted"
                    ),
                }
            }
            NotEvaluable::EarlyUnresolved {
                value,
                continued,
                low,
                high,
                limit,
            } => {
                write!(
                    f,
                    "early_unresolved: {continued} with the reverberation beginning at the \
                     arrival; "
                )?;
                match (low, high) {
                    (Some(l), Some(h)) => write!(
                        f,
                        "{l} to {h} with it beginning at the arrival, at the first bin wholly \
                         after the direct sound or at that bin's end (midway {value})"
                    )?,
                    _ => write!(
                        f,
                        "with it beginning later, at the first bin wholly after the direct \
                         sound or at that bin's end, the curve gives no value"
                    )?,
                }
                write!(f, "; the limit is {limit}. Use a finer time step")
            }
            NotEvaluable::MonteCarloNoise {
                value,
                sd,
                limit,
                resamples,
                refused_resamples,
                particle_count,
            } => {
                write!(f, "monte_carlo_noise: {value} from the series, ")?;
                match sd {
                    Some(sd) => write!(
                        f,
                        "standard deviation {sd} (calibrated) over {resamples} resamples"
                    )?,
                    None => write!(f, "no standard deviation from {resamples} resamples")?,
                }
                write!(
                    f,
                    ", {refused_resamples} of which refuse it; the limit is {limit}."
                )?;
                match particle_count {
                    ParticleCount::Named {
                        particles: Some(n), ..
                    } => write!(
                        f,
                        " Run at least {n} particles per source to bring it within its limit"
                    ),
                    ParticleCount::Named { factor, .. } => write!(
                        f,
                        " Run at least {factor:.3} times the particles to bring it within its limit"
                    ),
                    ParticleCount::Resampled {
                        particles: Some(n), ..
                    } => write!(
                        f,
                        " Run at least {n} particles per source: there its resamples would refuse \
                         it seldom enough and its noise would be within its limit"
                    ),
                    ParticleCount::Resampled { multiple, .. } => write!(
                        f,
                        " Run at least {multiple} times the particles: there its resamples would \
                         refuse it seldom enough and its noise would be within its limit"
                    ),
                    ParticleCount::BeyondResampled { multiple } => write!(
                        f,
                        " No particle count is named: at {multiple} times the particles its \
                         resamples would still refuse it, or its noise would still be above its \
                         limit, so more particles may not help"
                    ),
                    ParticleCount::ResampledNotConfirmed => write!(
                        f,
                        " No particle count is named: for this quantity the counts its resamples                          named were not borne out at a higher count"
                    ),
                    ParticleCount::ScalingNotConfirmed => write!(
                        f,
                        " No particle count is named: the calibration did not confirm that this \
                         quantity's spread falls as 1/√N in this computation method"
                    ),
                    ParticleCount::NoStandardDeviation => write!(
                        f,
                        " With no standard deviation, no particle count can be named"
                    ),
                }
            }
            NotEvaluable::NoiseUnknown { value, detail } => write!(
                f,
                "noise_unknown: {value} from the series, but its Monte-Carlo noise cannot be \
                 estimated: {detail}"
            ),
            NotEvaluable::NoiseUncalibrated {
                value,
                particles,
                crossings_per_particle,
                min_particles,
                max_crossings_per_particle,
                particles_at_least,
                receiver_radius_scale_at_most,
            } => {
                write!(
                    f,
                    "noise_uncalibrated: {value} from the series, but the noise model was \
                     calibrated for this quantity at {min_particles} particles per source or \
                     more and {max_crossings_per_particle} crossings of a receiver per particle \
                     or fewer; this run has {particles} and {crossings_per_particle}."
                )?;
                if let Some(n) = particles_at_least {
                    write!(f, " Run at least {n} particles per source.")?;
                }
                if let Some(s) = receiver_radius_scale_at_most {
                    write!(
                        f,
                        " Make the receiver radius at most {s:.3} times this run's: crossings per \
                         particle grow as its square, and more particles do not lower them."
                    )?;
                }
                Ok(())
            }
            NotEvaluable::SeveralSources { sources } => write!(
                f,
                "several_sources: {sources:?} all contribute; the quantity is defined per source \
                 and receiver. Turn on the echogram per source"
            ),
            NotEvaluable::NoTimeSeries { detail } => {
                write!(f, "no_time_series: the solver wrote none: {detail}")
            }
        }
    }
}

/// What made energy go missing, in words.
fn missing_cause(floor_db: Option<f64>, lost_share: Option<f64>) -> String {
    let floor = floor_db.map(|db| format!("the solver's floor ({db} dB below a particle's start)"));
    let lost = lost_share.map(|s| format!("its lost particles (share {s:.2e})"));
    match (floor, lost) {
        (Some(a), Some(b)) => format!("{a} and {b}"),
        (Some(a), None) | (None, Some(a)) => a,
        (None, None) => "nothing".into(),
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
    BadArrival {
        time_s: f64,
        detail: String,
    },
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
    BadNoiseInput {
        field: String,
        value: f64,
    },
    /// The diffuse ray transport (`params::lambert`) cannot run with its inputs, a ray left the
    /// enclosure, its mean free path is not `4V/S` within its statistical error, or `γ²`'s
    /// standard error is above its limit.
    TransportRefused {
        detail: String,
    },
    /// Kuttruff's reference does not describe this band: not every face reflects by Lambert's
    /// law with scattering 1 in it (`results::reference`); or no computed band has such walls, so
    /// the transport was not run.
    ReferenceNotApplicable {
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
            ParamError::BadArrival { .. } => codes::BAD_ARRIVAL,
            ParamError::NotEvaluable { .. } => codes::NOT_EVALUABLE,
            ParamError::SeriesMismatch { .. } => codes::SERIES_MISMATCH,
            ParamError::BadAir { .. } => codes::BAD_AIR,
            ParamError::BadRoom { .. } => codes::BAD_ROOM,
            ParamError::NoAbsorption => codes::NO_ABSORPTION,
            ParamError::DinOutOfRange { .. } => codes::DIN_OUT_OF_RANGE,
            ParamError::BadNoiseInput { .. } => codes::BAD_NOISE_INPUT,
            ParamError::TransportRefused { .. } => codes::TRANSPORT_REFUSED,
            ParamError::ReferenceNotApplicable { .. } => codes::REFERENCE_NOT_APPLICABLE,
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
            ParamError::BadArrival { time_s, detail } => {
                write!(f, "the arrival at {time_s} s: {detail}")
            }
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
            ParamError::BadNoiseInput { field, value } => write!(f, "{field} = {value}"),
            ParamError::TransportRefused { detail } => write!(f, "{detail}"),
            ParamError::ReferenceNotApplicable { detail } => write!(f, "{detail}"),
        }
    }
}

impl std::error::Error for ParamError {}

pub(crate) fn not_evaluable(quantity: Quantity, why: NotEvaluable) -> ParamError {
    ParamError::NotEvaluable { quantity, why }
}

/// The JSON pattern of a seed as [`serialize_seed`] writes it.
pub(crate) const SEED_PATTERN: &str = "^0x[0-9a-f]{16}$";

/// Writes a 64-bit seed as the string `0x` and 16 lowercase hex digits. A JSON number above 2⁵³
/// is read as a different value by every reader that parses numbers as doubles (JavaScript, and
/// many JSON libraries): the transport's seed, `0x6c61_6d62_6572_7431`, would be read as
/// 7809643498213372928, not 7809643498213372977 (pre-M8 review).
pub(crate) fn serialize_seed<S: serde::Serializer>(seed: &u64, s: S) -> Result<S::Ok, S::Error> {
    s.serialize_str(&format!("{seed:#018x}"))
}

/// A solver's floor: it drops each particle once its energy falls `-db` dB below its start
/// ([`EnergySeries::with_solver_floor`]).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SolverFloor {
    /// Negative: SPPS's `-10·trans_epsilon`.
    pub db: f64,
    /// The share of the emitted energy the room still held when the direct sound arrived, above
    /// 0. What the dropped particles would still have brought is at most `10^{db/10}/alive_share`
    /// of the energy the receiver gets from the arrival on.
    pub alive_share: f64,
}

/// One band's energy histogram: bin `k` holds the energy that arrived in `[k·dt, (k+1)·dt)`,
/// in Pa² as SPPS writes a `.recp` value (`docs/params.md`, "The input").
#[derive(Clone, Debug, PartialEq)]
pub struct EnergySeries {
    dt: f64,
    values: Vec<f64>,
    /// The caller knows that no energy arrives after the last bin ([`EnergySeries::complete`]).
    complete: bool,
    /// The solver dropped each particle once its energy fell below a floor
    /// ([`EnergySeries::with_solver_floor`]).
    floor: Option<SolverFloor>,
    /// The share of the energy after the onset that lost particles can have taken with them
    /// ([`EnergySeries::with_lost_share`]).
    lost_share: Option<f64>,
    /// The lost share bounds the energy from every time on, not only from the onset: what the
    /// lost particles would still have brought falls with the decay
    /// ([`EnergySeries::with_lost_share_following_decay`]).
    lost_follows_decay: bool,
    /// How the reverberation ran before the first bin wholly after the direct sound is not known
    /// ([`EnergySeries::with_early_reverberation_unresolved`]).
    early_unresolved: bool,
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
        Ok(EnergySeries {
            dt,
            values,
            complete: false,
            floor: None,
            lost_share: None,
            lost_follows_decay: false,
            early_unresolved: false,
        })
    }

    /// The series of a solver whose reverberation begins with the first reflection and builds up,
    /// not with the direct sound: SPPS. In the bins the direct sound reaches, the histogram cannot
    /// tell it from reverberation that arrived with it, nor, in those bins and the first one after
    /// them, when the reverberation began. [`decay`] then reads the curve three ways, the
    /// reverberation beginning at the arrival (the first bin wholly after the direct sound's
    /// decay continued back to it: the module's model, exact when the reverberation runs from the
    /// arrival), at the start of that bin, and at its end; reports each quantity midway between the
    /// lowest and highest reading; and refuses one they spread further than its limit from that,
    /// `early_unresolved` (`docs/params.md`, "The curve between bin edges";
    /// `tests/params_early.rs`).
    pub fn with_early_reverberation_unresolved(mut self) -> Self {
        self.early_unresolved = true;
        self
    }

    /// Whether [`EnergySeries::with_early_reverberation_unresolved`] was set.
    pub fn early_reverberation_unresolved(&self) -> bool {
        self.early_unresolved
    }

    /// The series of a solver that lost some particles mid-path, whose unfinished paths can have
    /// taken up to `share` of the energy after the onset (`S(onset)`) with them. SPPS counts them
    /// (`partLoop`, `partLost`); `core::results` gives the share (`docs/results.md`, "Lost
    /// particles"). Every quantity it could move beyond its limit is refused, as for a floor; the
    /// series keeps its completeness, since nothing arrives after its end but what was lost.
    /// Refused, `params_bad_noise_input`, when `share` is not a finite number of at least 0.
    pub fn with_lost_share(mut self, share: f64) -> Result<Self, ParamError> {
        if !share.is_finite() || share < 0.0 {
            return Err(ParamError::BadNoiseInput {
                field: "lost_share".into(),
                value: share,
            });
        }
        self.lost_share = (share > 0.0).then_some(share);
        self.lost_follows_decay = false;
        Ok(self)
    }

    /// The series of a solver whose lost particles would still have brought at most `share` of
    /// the energy the receiver gets from any time on, `share·S(u)` from every `u`: SPPS in
    /// energetic mode, where every particle's energy falls with the room's, so a particle lost at
    /// `t` carries about the mean energy of the particles then, and what it would still have
    /// brought falls with the decay (`docs/params.md`, "Missing energy"; `core::results` gives
    /// the share). Such an addition scales the curve by at most `1 + share` at every time, so
    /// every quantity is held to the most that scaling can move it. Refused as
    /// [`EnergySeries::with_lost_share`] is.
    pub fn with_lost_share_following_decay(self, share: f64) -> Result<Self, ParamError> {
        let mut s = self.with_lost_share(share)?;
        s.lost_follows_decay = s.lost_share.is_some();
        Ok(s)
    }

    /// The share [`EnergySeries::with_lost_share`] or
    /// [`EnergySeries::with_lost_share_following_decay`] set.
    pub fn lost_share(&self) -> Option<f64> {
        self.lost_share
    }

    /// Whether the lost share follows the decay
    /// ([`EnergySeries::with_lost_share_following_decay`]).
    pub fn lost_follows_decay(&self) -> bool {
        self.lost_follows_decay
    }

    /// The series of a solver that drops each particle once its energy falls `-floor_db` dB below
    /// its start: SPPS in energetic mode drops it at `10^-trans_epsilon` of its start
    /// (`spps/sppsNantes.cpp:75`; `CalculationCore.cpp:57-60, 305`), so `floor_db` is
    /// `-10·trans_epsilon`. `alive_share` is the share of the emitted energy the room still held
    /// when the direct sound arrived ([`SolverFloor`]). What the dropped particles would still
    /// have brought is bounded, `10^{floor_db/10}/alive_share` of `S(onset)`, and every quantity it
    /// could move beyond its limit is refused (`docs/params.md`, "Missing energy"). A floor means
    /// energy is missing, so the series is no longer complete. Refused, `params_bad_noise_input`,
    /// when `floor_db` is not a finite number or `alive_share` not a finite positive one.
    pub fn with_solver_floor(
        mut self,
        floor_db: f64,
        alive_share: f64,
    ) -> Result<Self, ParamError> {
        if !floor_db.is_finite() {
            return Err(ParamError::BadNoiseInput {
                field: "floor_db".into(),
                value: floor_db,
            });
        }
        if !alive_share.is_finite() || alive_share <= 0.0 {
            return Err(ParamError::BadNoiseInput {
                field: "alive_share".into(),
                value: alive_share,
            });
        }
        self.floor = Some(SolverFloor {
            db: floor_db,
            alive_share,
        });
        self.complete = false;
        Ok(self)
    }

    /// The floor [`EnergySeries::with_solver_floor`] set.
    pub fn floor(&self) -> Option<SolverFloor> {
        self.floor
    }

    /// A series whose caller knows that no energy arrives after its last bin, refused as
    /// [`EnergySeries::new`]. Its tail is [`decay::Tail::Complete`]: nothing is estimated, added
    /// or refused for the energy after the end (`docs/params.md`, "Truncation"). `core::results`
    /// claims it for SPPS in random mode when the run's statistics count at most one particle in a
    /// million remaining at the end of the calculation (`results::spps::REMAINING_UNFINISHED_SHARE`;
    /// those few are bounded with the lost ones, as unfinished paths); the claim changes numbers,
    /// so it needs such evidence.
    pub fn complete(dt: f64, values: Vec<f64>) -> Result<Self, ParamError> {
        let mut s = Self::new(dt, values)?;
        s.complete = true;
        Ok(s)
    }

    /// Whether the series was made with [`EnergySeries::complete`].
    pub fn is_complete(&self) -> bool {
        self.complete
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
/// `Global` row, `projet_calculation.cpp:898`). It is complete when every band is, and carries
/// the highest floor of any band. Refused when the series differ in `dt` or length, or when there
/// are none.
///
/// **Not ISO 3382-1's single-number value.** ISO's single numbers are arithmetic means of octave
/// band values (Annex A, as commonly quoted: 500 Hz and 1 kHz for EDT and C80); this is one decay
/// of all bands' energy together, weighted by the source spectrum (`docs/params.md`).
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
    let summed = if bands.iter().all(|b| b.complete) {
        EnergySeries::complete(first.dt, values)?
    } else {
        EnergySeries::new(first.dt, values)?
    };
    // The highest floor, the smallest share alive and the largest lost share of any band: those
    // that can have cost most.
    let summed = match bands
        .iter()
        .filter_map(|b| b.floor)
        .reduce(|a, b| SolverFloor {
            db: a.db.max(b.db),
            alive_share: a.alive_share.min(b.alive_share),
        }) {
        Some(f) => summed.with_solver_floor(f.db, f.alive_share)?,
        None => summed,
    };
    // The early reverberation is unresolved when it is in any band.
    let summed = if bands.iter().any(|b| b.early_unresolved) {
        summed.with_early_reverberation_unresolved()
    } else {
        summed
    };
    // The largest lost share; it follows the decay only when every band's that has one does.
    let follows = bands
        .iter()
        .filter(|b| b.lost_share.is_some())
        .all(|b| b.lost_follows_decay);
    match bands.iter().filter_map(|b| b.lost_share).reduce(f64::max) {
        Some(share) if follows => summed.with_lost_share_following_decay(share),
        Some(share) => summed.with_lost_share(share),
        None => Ok(summed),
    }
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
            ParamError::BadArrival {
                time_s: -1.0,
                detail: "x".into(),
            },
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
            ParamError::BadNoiseInput {
                field: "floor_db".into(),
                value: f64::NAN,
            },
            ParamError::TransportRefused { detail: "x".into() },
            ParamError::ReferenceNotApplicable { detail: "x".into() },
        ];
        let got: Vec<&str> = errors.iter().map(ParamError::code).collect();
        assert_eq!(got, codes::ALL);
        for e in &errors {
            assert!(e.to_string().starts_with(e.code()), "{e}");
        }
    }
}
