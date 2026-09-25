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
//! below `W/N` (`CalculationCore.cpp:57, 141, 259, 284`), so the same `d̄` bounds each *deposit*
//! from above. That bounds neither mode's *variance*: crossings are not independent, since one
//! particle crosses a receiver at several times (its crossings share its lifetime in random mode,
//! its energy in energetic mode), so SPPS's spread can exceed the structure in either mode (it did,
//! by up to 1.43 times, in 11 of 13 random-mode cells of round 1). Only the calibration, measured
//! on SPPS's own seeds, says how the structure compares, and only where it was measured. (A
//! deposit scaled step by step by the particles' mean energy, from the room table, was the other
//! structure measured for energetic mode; calibrated, it overstated the seeds' spread more,
//! because the particles' energies spread apart as they are absorbed:
//! `docs/investigations/2026-09-25-noise-calibration/`.)
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
//! energies), so each quantity's standard deviation is multiplied by `factor·√(1 + kappa·n)`, per
//! computation method (and for energetic T20 and T30 per kind of band), `n` the run's crossings
//! of the receiver per particle ([`NoiseModel::multi_crossing`]): measured on real SPPS runs over
//! ten seeds per cell and validated on cells held out of the measurement
//! (`docs/investigations/2026-09-25-noise-calibration/`). The factor is the largest one-sided 95 %
//! upper bound of the ratio of the seeds' spread to the corrected model over the calibration
//! cells, so the calibrated value never claims less noise than SPPS showed there.
//!
//! **The domain.** A calibration holds where it was measured. A run's value is given only with at
//! least the particles per source and at most the crossings per particle the quantity was
//! calibrated at; outside, it is refused, `noise_uncalibrated`, naming the particles to run or how
//! far to shrink the receiver radius. What the code cannot see is stated instead
//! (`docs/params.md`, "Monte-Carlo noise": box rooms, the listed wall kinds, no transmission, no
//! fitting zones, steps of 1 and 10 ms).
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
/// (`docs/investigations/2026-09-25-noise-calibration/`, round 3): per computation method and
/// quantity, in [`QUANTITY_NAMES`]' order (SPL, EDT, T20, T30, C50, C80, D50, Ts), the bootstrap's
/// standard deviation is multiplied by `factor · √(1 + kappa·n)`, `n` the run's crossings of the
/// receiver per particle ([`variable`]), and a value is given only inside the domain the quantity
/// was measured on. Every number is what the pre-registered rules give on the committed receipt,
/// which the suite re-derives (`tests/params_noise_calibration.rs`).
pub mod calibration {
    use super::Method;
    use schemars::JsonSchema;
    use serde::Serialize;

    /// What `n`, the multi-crossing variable, is (rule R3-2, per method).
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, JsonSchema)]
    #[serde(rename_all = "snake_case")]
    pub enum Variable {
        /// The crossings of the receiver per particle, `n1`
        /// ([`super::crossings_per_particle`]).
        CrossingsPerParticle,
        /// `n1` times the spread of the particles' lifetimes, `Var L/(E L)²`
        /// ([`super::lifetime_cv2`]): a particle alive late has more crossings ahead of it when
        /// the decay has a long second slope.
        CrossingsTimesLifetimeSpread,
    }

    /// One quantity's calibration under one method (and, for energetic T20 and T30, one kind of
    /// band).
    #[derive(Clone, Copy, Debug, PartialEq, Serialize, JsonSchema)]
    pub struct Entry {
        /// `k`: the largest one-sided 95 % upper bound, over the calibration cells, of the ratio of
        /// the seeds' spread to `√(1 + kappa·n)` times the bootstrap, rounded up to two digits
        /// (rule R3-1).
        pub factor: f64,
        /// `κ`: how much a particle's crossings of one receiver at several times add (R3-1).
        pub kappa: f64,
        /// The domain (R3-5): the fewest particles per source the quantity was calibrated at ...
        pub min_particles: u32,
        /// ... and the most crossings per particle, `n`. Outside it the value is refused,
        /// `noise_uncalibrated`.
        pub max_crossings_per_particle: f64,
        /// A refusal names the particle count at which the calibrated standard deviation would be
        /// the limit over this margin: one run's own estimate scatters from seed to seed, so a
        /// count named from one run can fall short (rule 5b over round 3's calibration cells).
        pub margin: f64,
        /// Whether a refusal names a count: the seeds' spread did not fall slower than `1/√N` on
        /// any pair of cells that differ only in `N` (rule R3-6, one-sided).
        pub root_n_confirmed: bool,
    }

    #[allow(clippy::too_many_arguments)]
    const fn e(
        factor: f64,
        kappa: f64,
        min_particles: u32,
        max_crossings_per_particle: f64,
        margin: f64,
        root_n_confirmed: bool,
    ) -> Entry {
        Entry {
            factor,
            kappa,
            min_particles,
            max_crossings_per_particle,
            margin,
            root_n_confirmed,
        }
    }

    /// What the calibration was measured on beyond what [`entry`]'s domain checks, said beside
    /// every report's calibration (`monte_carlo.measured_on`).
    pub const MEASURED_ON: &str = "box rooms of 60 to 1,000 m3 (tutorial 1's mesh scaled); \
        Lambert, specular and partly scattering walls, uniform absorption or absorption on one \
        surface, every surface absorbing 1 (the direct field alone) and dead walls; one omni \
        source; no transmission, no fitting zones, no directivity; air absorption on or off; \
        steps of 1 and 10 ms; 5,000 to 15,000,000 particles and receiver radii of 0.31 to 1.4 m \
        (checked, per quantity, as particles and crossings per particle). Coupled volumes, long \
        specular tunnels, transmission and fittings were not measured";
    /// Random mode's variable (R3-2: it overstated least, 1.357 against 1.607 for `n1` alone).
    pub const RANDOM_VARIABLE: Variable = Variable::CrossingsTimesLifetimeSpread;
    /// Energetic mode's variable (R3-2: 1.924 against 1.942).
    pub const ENERGETIC_VARIABLE: Variable = Variable::CrossingsTimesLifetimeSpread;

    /// The largest `n` of random mode's calibration rows (C3-R1: a 5 × 4 × 3 m room at α 0.05
    /// with receivers of 0.9 m).
    const RANDOM_MAX_N: f64 = 2.1641858528999998;
    /// The largest `n` of energetic mode's calibration rows (C3-E1, the same room and receivers).
    const ENERGETIC_MAX_N: f64 = 2.12938788744;
    /// The largest `n` of the energetic Lambert rows with unequal absorption (all at receivers of
    /// 0.31 m).
    const ENERGETIC_LAMBERT_MAX_N: f64 = 0.0319601939142;
    /// The largest `n` of the energetic rows not Lambert throughout (C3-E4, specular, 0.9 m).
    const ENERGETIC_OTHER_MAX_N: f64 = 0.81943475966;

    /// Random mode (round 3, 22 calibration cells): every factor carries a correction for a
    /// particle's crossings of one receiver at several times, largest for the decay times; the
    /// decay times and Ts are calibrated from 50,000 particles, SPL, C50, C80 and D50 from 5,000.
    pub const RANDOM: [Entry; 8] = [
        e(1.2, 1.75, 5_000, RANDOM_MAX_N, 1.1, true),
        e(1.3, 2.0, 50_000, RANDOM_MAX_N, 1.2, true),
        e(1.5, 2.75, 50_000, RANDOM_MAX_N, 1.5, true),
        e(1.6, 3.0, 50_000, RANDOM_MAX_N, 1.5, true),
        e(1.2, 1.25, 5_000, RANDOM_MAX_N, 1.2, true),
        e(1.4, 1.0, 5_000, RANDOM_MAX_N, 1.2, true),
        e(1.2, 1.25, 5_000, RANDOM_MAX_N, 1.2, true),
        e(1.4, 3.0, 50_000, RANDOM_MAX_N, 1.1, true),
    ];
    /// Energetic mode (round 3, 29 calibration cells); its T20 and T30 here are those of bands
    /// not every face of which reflects by Lambert's law with scattering 1
    /// ([`super::Walls::Other`]): M7's structure, factor 1, as rounds 1 and 2 left them. SPL is
    /// above 1 since the direct field alone (every surface absorbing 1) and a specular room with
    /// 0.9 m receivers are among the cells.
    pub const ENERGETIC: [Entry; 8] = [
        e(1.1, 0.0, 5_000, ENERGETIC_MAX_N, 1.1, true),
        e(0.86, 0.0, 50_000, ENERGETIC_MAX_N, 1.2, true),
        e(1.0, 0.0, 15_000, ENERGETIC_OTHER_MAX_N, 1.2, true),
        e(1.0, 0.0, 150_000, ENERGETIC_OTHER_MAX_N, 1.3, true),
        e(0.96, 0.25, 5_000, ENERGETIC_MAX_N, 1.1, true),
        e(0.86, 0.5, 5_000, ENERGETIC_MAX_N, 1.1, true),
        e(0.96, 0.25, 5_000, ENERGETIC_MAX_N, 1.1, true),
        e(0.84, 0.5, 50_000, ENERGETIC_MAX_N, 1.1, true),
    ];
    /// Energetic T20 and T30 in bands whose every face reflects by Lambert's law with
    /// scattering 1 and not every face has the same absorption ([`super::Walls::Lambert`]): the
    /// particles' energies spread apart as in specular rooms when the absorption is concentrated
    /// (0.63 of M7's structure for T30 with the floor at 0.9 and the rest at 0.02). Measured with
    /// receivers of 0.31 m only, so their domain stops at few crossings per particle. T20's
    /// fitted 0.52 failed its validation (a 6 × 10 × 3 m room, floor 0.9, the rest 0.02, 1 ms
    /// steps: 1.12 times the prediction, lower bound 1.04), so by F1 it takes the other bands'
    /// factor 1 and kappa 0, in its own domain.
    pub const ENERGETIC_LAMBERT: [Entry; 2] = [
        e(1.0, 0.0, 150_000, ENERGETIC_LAMBERT_MAX_N, 1.3, true),
        e(0.73, 2.0, 150_000, ENERGETIC_LAMBERT_MAX_N, 1.3, true),
    ];
    /// Energetic T20 and T30 in bands whose every face reflects by Lambert's law with
    /// scattering 1 and has the same absorption, at most
    /// [`UNIFORM_LAMBERT_MAX_MEAN_ABSORPTION`] ([`super::Walls::UniformLambert`]; M8's rooms):
    /// every particle meets the same absorption at every reflection, so their energies spread
    /// apart only as their reflection counts do (T30 0.027 to 0.047 of M7's structure in ten
    /// cells).
    pub const ENERGETIC_UNIFORM_LAMBERT: [Entry; 2] = [
        e(0.098, 0.0, 5_000, ENERGETIC_MAX_N, 1.2, true),
        e(0.052, 0.0, 50_000, ENERGETIC_MAX_N, 1.3, true),
    ];
    /// The largest mean absorption the uniform-Lambert entries were calibrated at (0.4, as SPPS
    /// reads it in `f32`): the particles' energies spread apart faster the more each reflection
    /// absorbs, so a uniform-Lambert band above it takes the Lambert entries.
    pub const UNIFORM_LAMBERT_MAX_MEAN_ABSORPTION: f64 = 0.4000000059604645;

    /// The variable `method`'s correction and domain take.
    pub fn variable(method: Method) -> Variable {
        match method {
            Method::Random => RANDOM_VARIABLE,
            Method::Energetic => ENERGETIC_VARIABLE,
        }
    }

    /// `n` under `method` from a series' crossings per particle and its lifetimes' spread.
    pub fn multi_crossing(method: Method, crossings_per_particle: f64, lifetime_cv2: f64) -> f64 {
        match variable(method) {
            Variable::CrossingsPerParticle => crossings_per_particle,
            Variable::CrossingsTimesLifetimeSpread => crossings_per_particle * lifetime_cv2,
        }
    }

    /// Quantity `i`'s entry under `method`, for a band of faces `walls` (only energetic T20 and
    /// T30 differ). A test build can scale every factor through a fault seam
    /// (`crate::faults::Fault::NoiseCalibrationScaled`), the say-NO of the validation.
    pub fn entry(method: Method, i: usize, walls: super::Walls) -> Entry {
        use super::Walls;
        let mut e = match (method, i, walls) {
            (Method::Random, _, _) => RANDOM[i],
            (Method::Energetic, 2 | 3, Walls::UniformLambert) => ENERGETIC_UNIFORM_LAMBERT[i - 2],
            (Method::Energetic, 2 | 3, Walls::Lambert) => ENERGETIC_LAMBERT[i - 2],
            (Method::Energetic, _, _) => ENERGETIC[i],
        };
        if let Some(crate::faults::Fault::NoiseCalibrationScaled { by }) = crate::faults::active() {
            e.factor *= by;
        }
        e
    }

    /// Quantity `i`'s factor under `method`, outside Lambert bands.
    pub fn factor(method: Method, i: usize) -> f64 {
        entry(method, i, super::Walls::Other).factor
    }

    /// Quantity `i`'s margin under `method`, outside Lambert bands.
    pub fn margin(method: Method, i: usize) -> f64 {
        entry(method, i, super::Walls::Other).margin
    }

    /// Whether a refusal of quantity `i` under `method` names a particle count, outside Lambert
    /// bands.
    pub fn root_n_confirmed(method: Method, i: usize) -> bool {
        entry(method, i, super::Walls::Other).root_n_confirmed
    }
}

/// The eight quantities' names in the JSON, in the order of [`Parameters`] and of
/// [`calibration`]'s tables.
pub const QUANTITY_NAMES: [&str; 8] = [
    "spl_db", "edt_s", "t20_s", "t30_s", "c50_db", "c80_db", "d50", "ts_s",
];

/// SPPS's computation method (`computation_method`): how a crossing's deposit is modelled, and
/// which of [`calibration`]'s tables apply.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Method {
    /// Code 0: a particle keeps its start energy until it is absorbed whole.
    Random,
    /// Code 1: a particle's energy falls at every reflection.
    Energetic,
}

/// A band's faces, as energetic T20 and T30's calibration tells them apart (round 3, A2): how far
/// the particles' energies spread apart late in a decay depends on whether every face scatters
/// diffusely and absorbs alike.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Walls {
    /// Not every face reflects by Lambert's law with scattering 1.
    Other,
    /// Every face reflects by Lambert's law with scattering 1, and not every face has the same
    /// absorption (or it is above [`calibration::UNIFORM_LAMBERT_MAX_MEAN_ABSORPTION`]).
    Lambert,
    /// Every face reflects by Lambert's law with scattering 1 and has the same absorption, at most
    /// [`calibration::UNIFORM_LAMBERT_MAX_MEAN_ABSORPTION`].
    UniformLambert,
}

/// What the calibration's correction and domain need from the run behind a series.
#[derive(Clone, Debug, PartialEq, Serialize, JsonSchema)]
pub struct RunNoise {
    /// Particles per source (`nbparticules`).
    pub particles: u32,
    /// The smallest of the contributing sources' mean deposits: crossings per particle counted
    /// at it are at least each source's own.
    pub least_deposit: f64,
    /// The spread of the particles' lifetimes, [`lifetime_cv2`] of the band's room table (the
    /// largest over the bands of an aggregate).
    pub lifetime_cv2: f64,
    /// Every face reflects by Lambert's law with scattering 1 in the band (in every band of an
    /// aggregate); false when the run's reference was not computed.
    pub lambert_walls: bool,
    /// Every face has the same absorption in the band (in every band of an aggregate); false
    /// when the run's reference was not computed.
    pub uniform_absorption: bool,
    /// The faces' mean absorption in the band, `ᾱ` (the largest over an aggregate's bands).
    pub mean_absorption: f64,
    /// The bands summed into the series: 1 for a band. Each band has its own particles.
    pub bands: u32,
}

impl RunNoise {
    /// The band's faces as the calibration tells them apart.
    pub fn walls(&self) -> Walls {
        if !self.lambert_walls {
            Walls::Other
        } else if self.uniform_absorption
            && self.mean_absorption <= calibration::UNIFORM_LAMBERT_MAX_MEAN_ABSORPTION
        {
            Walls::UniformLambert
        } else {
            Walls::Lambert
        }
    }
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
        /// The run behind the series, for the calibration's correction and domain. `null` only
        /// for a series that is no run's (a synthetic one in the tests): then `n` is taken as 0
        /// and no domain is checked. Every model `core::results` makes from a run carries it.
        run: Option<RunNoise>,
    },
    /// Nothing bounds the noise; every value is refused, `noise_unknown`.
    Unknown { detail: String },
}

impl NoiseModel {
    /// Crossings of mean deposit `mean_deposit` under `method`, `particles` per source when known,
    /// for a series that is no run's (no correction, no domain: see [`NoiseModel::of_run`]);
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
            run: None,
        })
    }

    /// Crossings of a run's series: [`NoiseModel::crossings`] with what the calibration's
    /// correction and domain need; refused, `params_bad_noise_input`, when the least deposit or
    /// the lifetime spread is not a finite number above 0 (at least 0), no particle is run, or no
    /// band is summed.
    pub fn of_run(mean_deposit: f64, method: Method, run: RunNoise) -> Result<Self, ParamError> {
        let bad = |field: &str, value: f64| {
            Err(ParamError::BadNoiseInput {
                field: field.into(),
                value,
            })
        };
        if !run.least_deposit.is_finite()
            || run.least_deposit <= 0.0
            || run.least_deposit > mean_deposit
        {
            return bad("least_deposit", run.least_deposit);
        }
        if !run.lifetime_cv2.is_finite() || run.lifetime_cv2 < 0.0 {
            return bad("lifetime_cv2", run.lifetime_cv2);
        }
        if !(0.0..=1.0).contains(&run.mean_absorption) {
            return bad("mean_absorption", run.mean_absorption);
        }
        if run.particles == 0 {
            return bad("particles", 0.0);
        }
        if run.bands == 0 {
            return bad("bands", 0.0);
        }
        match NoiseModel::crossings(mean_deposit, method, Some(run.particles))? {
            NoiseModel::Crossings {
                mean_deposit,
                method,
                particles,
                ..
            } => Ok(NoiseModel::Crossings {
                mean_deposit,
                method,
                particles,
                run: Some(run),
            }),
            u @ NoiseModel::Unknown { .. } => Ok(u),
        }
    }

    /// `n`, the multi-crossing variable of `series` under this model
    /// ([`calibration::multi_crossing`]); `None` for an unknown model or one that is no run's.
    pub fn multi_crossing(&self, series: &EnergySeries) -> Option<f64> {
        let NoiseModel::Crossings {
            method,
            run: Some(run),
            ..
        } = self
        else {
            return None;
        };
        let total: f64 = series.values().iter().sum();
        let n1 = crossings_per_particle(
            total,
            run.least_deposit,
            f64::from(run.particles) * f64::from(run.bands),
        );
        Some(calibration::multi_crossing(*method, n1, run.lifetime_cv2))
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
    /// `n`, the series' crossings of the receiver per particle as the calibration measures them
    /// ([`NoiseModel::multi_crossing`]); `None` when the model is unknown or no run's, or the
    /// series is refused.
    pub crossings_per_particle: Option<f64>,
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
                crossings_per_particle: None,
            };
        }
    };
    let (base, onset, decay_arrival) = values(series, arrival);
    let samples = if base.iter().any(Result::is_ok) {
        resamples(series, arrival, model)
    } else {
        Vec::new()
    };
    let n = model.multi_crossing(series);
    let mut out: Vec<Result<Estimate, ParamError>> = Vec::with_capacity(8);
    for (i, b) in base.into_iter().enumerate() {
        out.push(b.and_then(|value| {
            let got: Vec<f64> = samples.iter().filter_map(|s| s[i]).collect();
            judge_one(
                model,
                i,
                value,
                standard_deviation(&got),
                RESAMPLES - got.len(),
                n,
            )
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
            let factor = calibrated_factor(model, 2, n)
                .unwrap_or(1.0)
                .max(calibrated_factor(model, 3, n).unwrap_or(1.0));
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
        crossings_per_particle: n,
    }
}

/// The factor quantity `i`'s bootstrap standard deviation is multiplied by under `model`, for a
/// series whose multi-crossing variable is `n` (`None`: no run's, taken as 0):
/// `factor · √(1 + kappa·n)` of its [`calibration::entry`]. `None` for an unknown model.
pub fn calibrated_factor(model: &NoiseModel, i: usize, n: Option<f64>) -> Option<f64> {
    let NoiseModel::Crossings { method, run, .. } = model else {
        return None;
    };
    let walls = run.as_ref().map_or(Walls::Other, RunNoise::walls);
    let e = calibration::entry(*method, i, walls);
    Some(e.factor * (1.0 + e.kappa * n.unwrap_or(0.0)).sqrt())
}

/// How [`evaluate`] judges quantity `i` ([`QUANTITY_NAMES`]) of value `value` under `model`, from
/// the bootstrap's standard deviation before calibration (`raw_sd`, in the quantity's unit), the
/// resamples that refused the quantity and the series' multi-crossing variable `n`
/// ([`NoiseModel::multi_crossing`]): the value with its calibrated standard deviation, or its
/// refusal. Public so that the calibration's evidence judges exactly as a report does.
pub fn judge_one(
    model: &NoiseModel,
    i: usize,
    value: f64,
    raw_sd: Option<f64>,
    refused: usize,
    n: Option<f64>,
) -> Result<Estimate, ParamError> {
    let (quantity, limit, relative) = QUANTITIES[i];
    let (method, particles, run) = match model {
        NoiseModel::Unknown { detail } => {
            return Err(not_evaluable(
                quantity,
                NotEvaluable::NoiseUnknown {
                    value,
                    detail: detail.clone(),
                },
            ));
        }
        NoiseModel::Crossings {
            method,
            particles,
            run,
            ..
        } => (*method, *particles, run),
    };
    let e = calibration::entry(
        method,
        i,
        run.as_ref().map_or(Walls::Other, RunNoise::walls),
    );
    // The domain: a run's value outside what its quantity was calibrated on is refused whatever
    // its noise reads (rule R3-5). A run's model always has its `n`.
    if let Some(run) = run {
        let n = n.unwrap_or(f64::INFINITY);
        let few = run.particles < e.min_particles;
        // A NaN is outside too.
        let many = n.is_nan() || n > e.max_crossings_per_particle;
        if few || many {
            return Err(not_evaluable(
                quantity,
                NotEvaluable::NoiseUncalibrated {
                    value,
                    particles: run.particles,
                    crossings_per_particle: n,
                    min_particles: e.min_particles,
                    max_crossings_per_particle: e.max_crossings_per_particle,
                    particles_at_least: few.then_some(e.min_particles),
                    receiver_radius_scale_at_most: many
                        .then(|| (e.max_crossings_per_particle / n).sqrt()),
                },
            ));
        }
    }
    judge(
        quantity,
        value,
        raw_sd,
        refused,
        Judged {
            limit,
            relative,
            factor: calibrated_factor(model, i, n).unwrap_or(e.factor),
            margin: e.root_n_confirmed.then_some(e.margin),
            particles,
        },
    )
}

/// How one quantity is judged: its limit (relative for the decay times), the calibration factor
/// with its correction, the margin a named particle count takes (`None`: no count is named, its
/// `1/√N` fall not confirmed), and the run's particles per source when known.
struct Judged {
    limit: f64,
    relative: bool,
    factor: f64,
    margin: Option<f64>,
    particles: Option<u32>,
}

/// A value against its bootstrap standard deviation before calibration (`raw_sd`) and the number
/// of resamples that refused it.
fn judge(
    quantity: Quantity,
    value: f64,
    raw_sd: Option<f64>,
    refused: usize,
    j: Judged,
) -> Result<Estimate, ParamError> {
    let sd = raw_sd.map(|s| s * j.factor);
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

/// The squared coefficient of variation of a particle's lifetime, `Var L / (E L)²`, from the share
/// of the band's emitted energy still in the room at the end of each step (`alive`, the room
/// table over the sources' power; 1 at time 0), by the trapezoid rule: `E L = ∫S`,
/// `E L² = ∫2t·S`. In random mode the share alive is the share of particles alive, so this is
/// their lifetimes' spread: 1 for an exponential decay, more for a double slope. `None` when the
/// shares are not finite numbers of at least 0, or nothing is alive after time 0.
pub fn lifetime_cv2(alive: &[f64], dt: f64) -> Option<f64> {
    if !(dt.is_finite() && dt > 0.0) || alive.iter().any(|a| !(a.is_finite() && *a >= 0.0)) {
        return None;
    }
    let (mut first, mut second) = (0.0, 0.0);
    let mut prev = (0.0, 1.0);
    for (k, &s) in alive.iter().enumerate() {
        let t = (k + 1) as f64 * dt;
        first += dt * (prev.1 + s) / 2.0;
        second += dt * (2.0 * prev.0 * prev.1 + 2.0 * t * s) / 2.0;
        prev = (t, s);
    }
    (first > 0.0).then(|| (second / (first * first) - 1.0).max(0.0))
}

/// The crossings of a receiver per particle behind a series holding `total` in all, each crossing
/// adding `least_deposit` at most on average: `total / (least_deposit · particles)`. Exact in
/// random mode with one source; with several, taken at the smallest source's deposit, it is at
/// least each source's own; in energetic mode it counts crossings weighted by the particles'
/// energies.
pub fn crossings_per_particle(total: f64, least_deposit: f64, particles: f64) -> f64 {
    total / (least_deposit * particles)
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
    fn the_lifetime_spread_is_one_for_an_exponential_decay_and_more_for_a_double_slope() {
        // Survival e^(−t/τ) at the end of each 1 ms step, τ 0.1 s, run to 3 s: E L = τ,
        // E L² = 2τ², so Var L / (E L)² = 1 (to the trapezoid's error).
        let dt = 0.001;
        let tau = 0.1;
        let one: Vec<f64> = (1..=3000).map(|k| (-(k as f64) * dt / tau).exp()).collect();
        let cv2 = lifetime_cv2(&one, dt).unwrap();
        assert!((cv2 - 1.0).abs() < 1e-3, "{cv2}");
        // Half the particles at τ and half at 10τ: E L = 5.5τ, E L² = 101τ², so 101/30.25 − 1.
        let two: Vec<f64> = (1..=30000)
            .map(|k| {
                let t = k as f64 * dt;
                0.5 * (-t / tau).exp() + 0.5 * (-t / (10.0 * tau)).exp()
            })
            .collect();
        let want = 101.0 / 30.25 - 1.0;
        let cv2 = lifetime_cv2(&two, dt).unwrap();
        assert!((cv2 / want - 1.0).abs() < 2e-3, "{cv2} vs {want}");
        // Says no: a share that is not a finite number of at least 0, a step that is not above 0,
        // or nothing alive after time 0.
        assert_eq!(lifetime_cv2(&[0.5, f64::NAN], dt), None);
        assert_eq!(lifetime_cv2(&[0.5, -0.1], dt), None);
        assert_eq!(lifetime_cv2(&one, 0.0), None);
        assert_eq!(lifetime_cv2(&[], dt), None);
    }

    #[test]
    fn crossings_per_particle_are_the_total_over_the_deposit_and_the_particles() {
        assert_eq!(crossings_per_particle(30.0, 2.0, 5.0), 3.0);
        // A run's model computes it from the series, over its bands' particles together.
        let run = RunNoise {
            particles: 5,
            least_deposit: 2.0,
            lifetime_cv2: 1.5,
            lambert_walls: false,
            uniform_absorption: false,
            mean_absorption: 0.1,
            bands: 2,
        };
        let m = NoiseModel::of_run(4.0, Method::Random, run).unwrap();
        let s = EnergySeries::complete(0.01, vec![10.0, 5.0, 5.0]).unwrap();
        let n1 = 20.0 / (2.0 * 5.0 * 2.0);
        assert_eq!(
            m.multi_crossing(&s),
            Some(calibration::multi_crossing(Method::Random, n1, 1.5))
        );
        // A model that is no run's has none.
        assert_eq!(random(1.0).multi_crossing(&s), None);
    }

    #[test]
    fn a_runs_model_refuses_a_least_deposit_above_the_largest_and_other_bad_inputs() {
        let ok = RunNoise {
            particles: 5,
            least_deposit: 1.0,
            lifetime_cv2: 1.0,
            lambert_walls: true,
            uniform_absorption: true,
            mean_absorption: 0.1,
            bands: 1,
        };
        assert!(NoiseModel::of_run(1.0, Method::Energetic, ok.clone()).is_ok());
        for bad in [
            RunNoise {
                least_deposit: 2.0,
                ..ok.clone()
            },
            RunNoise {
                least_deposit: f64::NAN,
                ..ok.clone()
            },
            RunNoise {
                lifetime_cv2: -1.0,
                ..ok.clone()
            },
            RunNoise {
                particles: 0,
                ..ok.clone()
            },
            RunNoise {
                bands: 0,
                ..ok.clone()
            },
            RunNoise {
                mean_absorption: 1.5,
                ..ok.clone()
            },
        ] {
            assert_eq!(
                NoiseModel::of_run(1.0, Method::Energetic, bad)
                    .unwrap_err()
                    .code(),
                super::super::codes::BAD_NOISE_INPUT
            );
        }
    }

    /// A run's model for [`judge_one`]: deposit 1, one band, `particles` per source, faces
    /// `walls` (uniform ones absorbing 0.1).
    fn run_model(method: Method, particles: u32, walls: Walls) -> NoiseModel {
        NoiseModel::of_run(
            1.0,
            method,
            RunNoise {
                particles,
                least_deposit: 1.0,
                lifetime_cv2: 1.0,
                lambert_walls: walls != Walls::Other,
                uniform_absorption: walls == Walls::UniformLambert,
                mean_absorption: 0.1f64.min(calibration::UNIFORM_LAMBERT_MAX_MEAN_ABSORPTION),
                bands: 1,
            },
        )
        .unwrap()
    }

    #[test]
    fn a_value_inside_the_domain_carries_the_corrected_factor_and_one_outside_is_refused() {
        for method in [Method::Random, Method::Energetic] {
            for i in 0..8 {
                for lambert in [Walls::Other, Walls::Lambert, Walls::UniformLambert] {
                    let e = calibration::entry(method, i, lambert);
                    let value = 1.0;
                    // A tiny raw standard deviation: within every limit.
                    let raw = Some(1e-6);
                    let inside_n = e.max_crossings_per_particle / 2.0;
                    let n_particles = e.min_particles.max(1);
                    let m = run_model(method, n_particles, lambert);
                    let got = judge_one(&m, i, value, raw, 0, Some(inside_n)).unwrap();
                    let want = 1e-6 * e.factor * (1.0 + e.kappa * inside_n).sqrt();
                    assert!(
                        (got.sd - want).abs() <= 1e-12 * want,
                        "{method:?} {i} {lambert:?}"
                    );
                    // Says no: more crossings per particle than the calibration measured.
                    let many = 2.0 * e.max_crossings_per_particle.max(1e-3);
                    if e.max_crossings_per_particle < f64::MAX / 4.0 {
                        let r = judge_one(&m, i, value, raw, 0, Some(many)).unwrap_err();
                        match r.not_evaluable() {
                            Some(NotEvaluable::NoiseUncalibrated {
                                particles_at_least: None,
                                receiver_radius_scale_at_most: Some(s),
                                ..
                            }) => assert!((s - (0.5f64).sqrt()).abs() < 1e-12, "{s}"),
                            other => panic!("{method:?} {i}: {other:?}"),
                        }
                    }
                    // Says no: fewer particles than the calibration measured.
                    if e.min_particles > 1 {
                        let few = run_model(method, e.min_particles - 1, lambert);
                        let r = judge_one(&few, i, value, raw, 0, Some(inside_n)).unwrap_err();
                        match r.not_evaluable() {
                            Some(NotEvaluable::NoiseUncalibrated {
                                particles_at_least: Some(n),
                                ..
                            }) => assert_eq!(*n, e.min_particles),
                            other => panic!("{method:?} {i}: {other:?}"),
                        }
                    }
                    // A run's model without its n is refused, never judged as if n were 0.
                    assert!(judge_one(&m, i, value, raw, 0, None).is_err());
                }
            }
        }
    }

    #[test]
    fn a_quantity_whose_fall_as_one_over_root_n_is_not_confirmed_names_no_count() {
        // Every flag of round 3 is on; the path stays for a calibration that turns one off.
        let judged = |margin| {
            judge(
                Quantity::T30,
                1.0,
                Some(0.1),
                0,
                Judged {
                    limit: limits::DECAY_RELATIVE,
                    relative: true,
                    factor: 1.0,
                    margin,
                    particles: Some(150_000),
                },
            )
            .unwrap_err()
        };
        let off = judged(None);
        match off.not_evaluable() {
            Some(NotEvaluable::MonteCarloNoise {
                particle_count: ParticleCount::ScalingNotConfirmed,
                ..
            }) => {}
            other => panic!("{other:?}"),
        }
        assert!(off.to_string().contains("did not confirm"), "{off}");
        // Says no: with the flag on, the same value names a count, (1.2 · 0.1 / 0.025)² = 23.04
        // times the run's particles, rounded up to two digits.
        match judged(Some(1.2)).not_evaluable() {
            Some(NotEvaluable::MonteCarloNoise {
                particle_count:
                    ParticleCount::Named {
                        particles: Some(n), ..
                    },
                ..
            }) => assert_eq!(*n, 3_500_000),
            other => panic!("{other:?}"),
        }
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
