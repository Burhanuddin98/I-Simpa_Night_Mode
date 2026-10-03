//! The JSON `simpa results --json` prints: a [`RunResults`] and, per point receiver and band,
//! `core::params`' SPL, EDT, T20, T30, C50, C80, D50 and Ts, with G beside them and dB(A) per
//! receiver (`params::level`), and STI per receiver (`params::sti`), each a value with its estimated
//! Monte-Carlo standard deviation, or the reason it is not evaluable (for a TCR receiver every
//! one, `no_time_series`: TCR writes no series to compute them from). The shape is documented in
//! `docs/formats/results-json.md` and generated as a JSON Schema by [`report_schema`]
//! (`docs/formats/results-json.schema.json`), which M12 reads.
//!
//! **`validated_by_bed` is false**: no number here may be shown to a user before M8's physics bed
//! passes (`docs/rebuild-plan.md`, M12).

use schemars::JsonSchema;
use serde::Serialize;

use super::reference::{REFERENCE_LABEL, Reference};
use super::spps::{
    BandEnergy, ParticleFileSummary, PointReceiver, SourcePoint, SourceTotals, SppsResults,
};
use super::tcr::{self, MainBand, TcrResults};
use super::{Refusal, RunResults, SolverBuild, SolverResults, SurfaceFile, value_invalid};
use crate::params::decay::{self, Arrival, Onset};
use crate::params::edt;
use crate::params::lambert::FreePaths;
use crate::params::level;
use crate::params::noise::{self, NoiseModel};
use crate::params::sti;
use crate::params::{self, EnergySeries, NotEvaluable, ParamError, Quantity};
use crate::run::stats::ParticleStats;
use crate::run::verdict::Status;
use crate::schema::SolverKind;

/// The layout of [`Report`]; bumped when a field changes meaning. 2: values carry `mc_sd`, and
/// the Monte-Carlo, floor and per-source fields were added. 3: TCR point receivers carry
/// `parameters` per band and an `aggregate`, every value refused `no_time_series`. 4 (the M7
/// follow-ups): SPPS bands carry `lost_follows_decay`, and in energetic mode `lost_share` bounds
/// the energy from every time on; a given arrival outside the onset bin refuses C50, C80, D50 and
/// Ts only; bands carry the `arrival` and `decay_arrival` they were measured from, with the direct
/// sound's spread, and `early_reverberation_unresolved`: each value is midway between the lowest
/// and highest of three readings of the early reverberation (continued back to the arrival, or
/// beginning at the start or at the end of the first bin wholly after the direct sound), or
/// refused `early_unresolved`; bands and aggregates carry
/// `curvature` and `decay_curve`; a TCR receiver's `Global` row is the labelled object `global`, and
/// its `aggregate` says it sums nothing; surface files carry `aggregate` and each receiver its `id`.
/// Version 4 has not been merged yet, so these are 4 as well. 5 (pre-M8): an SPPS run carries
/// `reference`, Kuttruff's corrected Eyring with `γ²` from the room's geometry and plain Eyring,
/// labelled and not validated; seeds (`monte_carlo.seed`, the transport's) are hex strings; and
/// every `mc_sd` is calibrated against SPPS's own seed-to-seed spread per computation method
/// (`monte_carlo.method` and `.calibration`, `noise_model.method` and `.particles`), and a
/// refusal for noise carries `particle_count`. 6 (M8b): `edt_s` is EDT v2.1 (`params::edt`), the
/// held-out-tested method, on the raw histogram: a value with its guaranteed range, or refused
/// `edt_refused`; SPPS parameters carry `edt` (status, range, reason, validated). 7 (M8b,
/// decision-log rows 37 (3) and 39 (3)): a value of the eight parameters carries `status`, `lo` and
/// `hi`, its range (`params::noise::range`; EDT's from `edt`), `ok` or `wide`; a value refused
/// `monte_carlo_noise` for its standard deviation alone is shown so, `wide`, instead of refused.
/// 8 (M8b): bands, a source's bands and TCR's bands carry `g_db`, sound strength G; aggregates
/// carry `dba`, the A-weighted level of the bands' SPL. No other field changes. 9 (M8b, the bed's
/// findings): a value refused `monte_carlo_noise` for its resamples alone, which its stand-ins
/// resample within the allowed refusals, is shown `wide` with `refused_resamples` and the
/// stand-ins' range (`params::noise`, "The stand-ins"); a C50, C80 or D50 whose bin straddling te
/// can move it beyond its limit is `wide` with `straddle` and a range covering it
/// (`params::decay::Straddle`). 10 (M8b): a new project computes the octaves 125 Hz to 8 kHz
/// (decision-log row 43), and an SPPS point receiver carries `sti`, the speech transmission index
/// (IEC 60268-16:2011, `params::sti`): male (shown) and female, the MTF and MTI per band, or
/// refused. No other field changes.
pub const REPORT_VERSION: u32 = 10;

/// A quantity's value, or why it has none.
#[derive(Clone, Debug, PartialEq, Serialize, JsonSchema)]
#[serde(untagged)]
pub enum Evaluated {
    /// In the quantity's unit (the field name says it).
    Value {
        value: f64,
        /// The estimated Monte-Carlo standard deviation, in the same unit (`params::noise`);
        /// `null` for a value that does not come from a Monte-Carlo histogram, such as TCR's
        /// analytic references.
        mc_sd: Option<f64>,
        /// One of the eight parameters of an SPPS band, aggregate or per-source band
        /// (decision-log rows 37 (3) and 39 (3)): `ok` when its range, `lo` to `hi`, is within
        /// the quantity's difference limen (`params::noise::jnd`: 5 % for the decay times, 1 dB
        /// for SPL, C50 and C80, 0.05 for D50, 10 ms for Ts), `wide` when it is not. A `wide` value
        /// is shown with its range rather than refused; a consumer that shows the value shows the
        /// range beside it. Also `wide`, whatever the range's width, with `refused_resamples` or
        /// `straddle` (results version 9). Absent for every other value.
        #[serde(skip_serializing_if = "Option::is_none")]
        status: Option<noise::RangeStatus>,
        /// The range's lower end, in the same unit: `value − 2.5·mc_sd`
        /// (`params::noise::RANGE_Z`), or EDT's own (`edt.lo_s`). Present with `status`.
        #[serde(skip_serializing_if = "Option::is_none")]
        lo: Option<f64>,
        /// The range's upper end: `value + 2.5·mc_sd`, or EDT's own (`edt.hi_s`). Present with
        /// `status`.
        #[serde(skip_serializing_if = "Option::is_none")]
        hi: Option<f64>,
        /// Present only on a value `params::noise::evaluate` refused `monte_carlo_noise` because
        /// more than 10 of its 200 resamples refused it, but which at most 10 refuse when the same
        /// resamples are judged with their decay range on the series (the stand-ins,
        /// `params::noise`, "The stand-ins"): how many refused it as judged. Shown `wide`, `mc_sd`
        /// the judged standard deviation (the stand-ins' when the judged resamples gave none), and
        /// `lo`/`hi` `value ∓ 2.5·sd` with `sd` the larger of the stand-ins' and `mc_sd`.
        #[serde(skip_serializing_if = "Option::is_none")]
        refused_resamples: Option<usize>,
        /// Present only on a C50, C80 or D50 whose bin straddling te, wholly late or wholly early,
        /// moves it beyond its limit (0.1 dB, 0.005; `params::decay::Straddle`): `[lo, hi]`, the
        /// value with that bin each way. The value is `wide`, and `lo`/`hi` cover the bracket,
        /// widened by `2.5·mc_sd` each way.
        #[serde(skip_serializing_if = "Option::is_none")]
        straddle: Option<[f64; 2]>,
    },
    /// `core::params` refused it.
    NotEvaluable { not_evaluable: Refused },
}

/// A `core::params` refusal.
#[derive(Clone, Debug, PartialEq, Serialize, JsonSchema)]
pub struct Refused {
    /// One of `params::codes::ALL` (`docs/solver-contract.md`, "Parameter refusals").
    pub code: String,
    /// The refusal in words.
    pub message: String,
    /// The typed refusal.
    pub error: ParamError,
}

impl Evaluated {
    fn refused(e: ParamError) -> Self {
        Evaluated::NotEvaluable {
            not_evaluable: Refused {
                code: e.code().to_string(),
                message: e.to_string(),
                error: e,
            },
        }
    }

    /// A value with no range.
    pub fn bare(value: f64, mc_sd: Option<f64>) -> Self {
        Evaluated::Value {
            value,
            mc_sd,
            status: None,
            lo: None,
            hi: None,
            refused_resamples: None,
            straddle: None,
        }
    }

    /// A value that is not from a Monte-Carlo histogram, or its refusal.
    fn of(r: Result<f64, ParamError>) -> Self {
        match r {
            Ok(value) => Evaluated::bare(value, None),
            Err(e) => Self::refused(e),
        }
    }

    /// A Monte-Carlo value with its standard deviation and no range, or its refusal: the
    /// curvature, which has no limen.
    fn of_estimate(r: Result<noise::Estimate, ParamError>) -> Self {
        match r {
            Ok(e) => Evaluated::bare(e.value, Some(e.sd)),
            Err(e) => Self::refused(e),
        }
    }

    /// Parameter `i` (`params::noise::QUANTITY_NAMES`) as the product shows it
    /// (`params::noise::shown`): a value with its standard deviation and its range, `ok` or `wide`,
    /// a refusal for its standard deviation alone included; any other refusal as it is.
    pub fn of_parameter(i: usize, r: Result<noise::Estimate, ParamError>) -> Self {
        Self::of_parameter_with(i, r, &noise::Widen::default())
    }

    /// [`Evaluated::of_parameter`] with what widens its range (`params::noise::shown_with`).
    pub fn of_parameter_with(
        i: usize,
        r: Result<noise::Estimate, ParamError>,
        w: &noise::Widen,
    ) -> Self {
        match noise::shown_with(i, r, w) {
            Ok(s) => Evaluated::Value {
                value: s.value,
                mc_sd: Some(s.sd),
                status: Some(s.status),
                lo: Some(s.lo),
                hi: Some(s.hi),
                refused_resamples: s.refused_resamples,
                straddle: s.straddle.map(|(lo, hi)| [lo, hi]),
            },
            Err(e) => Self::refused(e),
        }
    }

    /// The range's status, for a value that has one.
    pub fn status(&self) -> Option<noise::RangeStatus> {
        match self {
            Evaluated::Value { status, .. } => *status,
            Evaluated::NotEvaluable { .. } => None,
        }
    }

    /// The value, if there is one.
    pub fn value(&self) -> Option<f64> {
        match self {
            Evaluated::Value { value, .. } => Some(*value),
            Evaluated::NotEvaluable { .. } => None,
        }
    }

    /// The refusal, if it is one.
    pub fn refusal(&self) -> Option<&Refused> {
        match self {
            Evaluated::Value { .. } => None,
            Evaluated::NotEvaluable { not_evaluable } => Some(not_evaluable),
        }
    }
}

/// EDT as EDT v2.1 ([`edt`]) read it from the band's raw histogram: the value with its
/// guaranteed range, or the reason it declined. `Parameters::edt_s` carries the same value, or the
/// refusal as `edt_refused`.
#[derive(Clone, Debug, PartialEq, Serialize, JsonSchema)]
pub struct EdtReport {
    /// [`edt::METHOD`].
    pub method: String,
    /// `ok` (the range is inside the 5 % JND), `wide` (shown with its range, decision-log row 9)
    /// or `refused`.
    pub status: edt::Status,
    /// The value, s; absent when refused.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value_s: Option<f64>,
    /// The range's lower end, s; absent when refused.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lo_s: Option<f64>,
    /// The range's upper end, s; absent when refused.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hi_s: Option<f64>,
    /// A refusal's code (`params::edt::REFUSAL_REASONS`), or the method's own detail
    /// (`hw=…;fit=…;noise=…;tail=…;n=…`).
    pub reason: String,
    /// The direct sound's arrival the method was given, s, **the source's emission delay
    /// included** (rounded up to the next whole step); absent when not computed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arrival_s: Option<f64>,
    /// Whether the method has passed its held-out test for what this EDT was read from: a single
    /// band, in either mode, with a receiver up to 1 m that the direct sound reached
    /// (`docs/investigations/2026-09-27-edt-heldout/VERDICT-2.md`; energetic mode's one H3
    /// failure was a receiver with no direct sound, decision-log row 38), and an aggregate never
    /// (the test read single bands).
    pub validated: bool,
    /// Why `validated` is false.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub validation_note: Option<String>,
}

/// The note an EDT carries, in every mode, when no direct sound reached the receiver
/// ([`edt::Outcome::no_direct_path`]): energetic mode's only failure in the held-out test was
/// such a receiver (VERDICT-2 H3, G4 R007), read 5-9 % low in both modes (decision-log row 38).
/// Burhan's wording (2026-10-02 15:41): EDT does not need line of sight; its start time does.
pub const EDT_NO_DIRECT_PATH_NOT_VALIDATED: &str = "not yet validated: start time uncertain, no \
direct path from the source (the first arrival is estimated from the first recorded hit; \
VERDICT-2 H3, G4 R007)";

/// The note a summed-bands (broadband) aggregate's EDT carries in every mode: the held-out test
/// read single bands only.
pub const EDT_BROADBAND_NOT_VALIDATED: &str = "not yet validated: broadband EDT is not covered by the held-out test (VERDICT-2 tested single bands only)";

/// The note an EDT carries when the receiver is larger than [`edt::VALIDATED_MAX_RADIUS_M`], in
/// every mode (decision 37).
pub const EDT_LARGE_RECEIVER_NOT_VALIDATED: &str = "not yet validated: receivers over 1 m read EDT up to 7 % low in the held-out test (VERDICT-2, ruling 1)";

/// The eight parameters of one band (or of the aggregate).
#[derive(Clone, Debug, PartialEq, Serialize, JsonSchema)]
pub struct Parameters {
    /// dB re 20 µPa.
    pub spl_db: Evaluated,
    /// s.
    pub edt_s: Evaluated,
    /// s.
    pub t20_s: Evaluated,
    /// s.
    pub t30_s: Evaluated,
    /// dB.
    pub c50_db: Evaluated,
    /// dB.
    pub c80_db: Evaluated,
    /// A fraction, 0 to 1 (shown as a percentage).
    pub d50: Evaluated,
    /// s.
    pub ts_s: Evaluated,
    /// EDT's status, range and validation (below `edt_s`'s value). Absent where EDT is not
    /// computed from a histogram: TCR, and the several-sources refusal.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub edt: Option<EdtReport>,
    /// Whether `edt_s` is a tested number: `edt.validated`, and false where there is no `edt`.
    /// `edt_s` itself stays a bare value for the consumers that read it, so this is its marker:
    /// a consumer that prints or exports `edt_s` carries "not yet validated" wherever this is false
    /// (decision-log row 20).
    pub edt_validated: bool,
}

impl Parameters {
    /// The eight in their order, with their JSON names.
    pub fn named(&self) -> [(&'static str, &Evaluated); 8] {
        [
            ("spl_db", &self.spl_db),
            ("edt_s", &self.edt_s),
            ("t20_s", &self.t20_s),
            ("t30_s", &self.t30_s),
            ("c50_db", &self.c50_db),
            ("c80_db", &self.c80_db),
            ("d50", &self.d50),
            ("ts_s", &self.ts_s),
        ]
    }

    /// The seven onset-relative quantities refused as `several_sources`: ISO 3382-1 defines them
    /// per source–receiver pair. SPL, the level of every source together, stays.
    fn several_sources(&mut self, sources: &[&str]) {
        let why = || NotEvaluable::SeveralSources {
            sources: sources.iter().map(|s| s.to_string()).collect(),
        };
        let refuse = |q: Quantity| Evaluated::refused(params::not_evaluable(q, why()));
        self.edt_s = refuse(Quantity::Edt);
        self.edt = None;
        self.edt_validated = false;
        self.t20_s = refuse(Quantity::T20);
        self.t30_s = refuse(Quantity::T30);
        self.c50_db = refuse(Quantity::Clarity { te_s: 0.05 });
        self.c80_db = refuse(Quantity::Clarity { te_s: 0.08 });
        self.d50 = refuse(Quantity::Definition { te_s: 0.05 });
        self.ts_s = refuse(Quantity::CentreTime);
    }

    /// All eight refused as `no_time_series`: the solver wrote no series to compute them from
    /// (TCR). `detail` says where the solver's own values are.
    fn no_time_series(detail: &str) -> Self {
        let refuse = |q: Quantity| {
            Evaluated::refused(params::not_evaluable(
                q,
                NotEvaluable::NoTimeSeries {
                    detail: detail.to_string(),
                },
            ))
        };
        Parameters {
            spl_db: refuse(Quantity::Spl),
            edt_s: refuse(Quantity::Edt),
            t20_s: refuse(Quantity::T20),
            t30_s: refuse(Quantity::T30),
            c50_db: refuse(Quantity::Clarity { te_s: 0.05 }),
            c80_db: refuse(Quantity::Clarity { te_s: 0.08 }),
            d50: refuse(Quantity::Definition { te_s: 0.05 }),
            ts_s: refuse(Quantity::CentreTime),
            edt: None,
            edt_validated: false,
        }
    }
}

/// `core::params` on one series, measured from `arrival`, each value with its noise under `model`
/// (`params::noise::evaluate`). The series itself may be refused (all zero, for instance): then
/// every parameter carries that refusal. SPL does not depend on the arrival, so a given arrival
/// that `params` refuses (`params_bad_arrival`) refuses the other seven only.
pub fn parameters(
    series: &Result<EnergySeries, ParamError>,
    arrival: Arrival,
    model: &NoiseModel,
) -> (Parameters, Option<Onset>) {
    let e = evaluated(series, arrival, model);
    (e.parameters, e.onset)
}

/// The curvature of a decay (`params::decay::curvature`), for M8 and M12 to decide what a curved
/// decay may show: its value with its Monte-Carlo standard deviation, and the flag.
#[derive(Clone, Debug, PartialEq, Serialize, JsonSchema)]
pub struct CurvatureReport {
    /// `100·(T30/T20 − 1)`, %, from the reported T20 and T30, with its standard deviation over
    /// the Monte-Carlo resamples; refused, with T30's refusal or else T20's, when either is.
    pub percent: Evaluated,
    /// `|percent| > limit_percent`: a curved (double-slope) decay. `null` when `percent` is
    /// refused.
    pub curved: Option<bool>,
    /// 10 %, ISO 3382-2's, as commonly stated (`params::decay::CURVATURE_LIMIT_PERCENT`).
    pub limit_percent: f64,
}

impl CurvatureReport {
    fn of(r: Result<noise::Estimate, ParamError>) -> Self {
        let percent = Evaluated::of_estimate(r);
        CurvatureReport {
            curved: percent
                .value()
                .map(|p| p.abs() > decay::CURVATURE_LIMIT_PERCENT),
            percent,
            limit_percent: decay::CURVATURE_LIMIT_PERCENT,
        }
    }
}

/// One series through `core::params`: the eight parameters, the curvature, the onset, what the
/// decay times were measured from, and the Schroeder curve they were fitted to.
struct Evaluation {
    parameters: Parameters,
    crossings_per_particle: Option<f64>,
    curvature: CurvatureReport,
    onset: Option<Onset>,
    decay_arrival: Option<Arrival>,
    decay_curve: Option<decay::DecayCurve>,
}

impl Evaluation {
    /// The seven onset-relative quantities refused as `several_sources` ([`Parameters`]), the
    /// curvature with them, and no decay curve: the decay of several sources' sum is no
    /// source–receiver pair's.
    fn several_sources(&mut self, sources: &[&str]) {
        self.parameters.several_sources(sources);
        self.curvature = CurvatureReport::of(Err(params::not_evaluable(
            Quantity::Curvature,
            NotEvaluable::SeveralSources {
                sources: sources.iter().map(|s| s.to_string()).collect(),
            },
        )));
        self.decay_curve = None;
    }
}

/// [`parameters`], with the curvature, what the decay times were measured from
/// (`params::decay::BandParameters::decay_arrival`) and the curve they were fitted to.
fn evaluated(
    series: &Result<EnergySeries, ParamError>,
    arrival: Arrival,
    model: &NoiseModel,
) -> Evaluation {
    let p = noise::evaluate(series, arrival, model);
    Evaluation {
        parameters: Parameters {
            spl_db: Evaluated::of_parameter_with(0, p.spl_db, &p.widen[0]),
            edt_s: Evaluated::of_parameter_with(1, p.edt_s, &p.widen[1]),
            t20_s: Evaluated::of_parameter_with(2, p.t20_s, &p.widen[2]),
            t30_s: Evaluated::of_parameter_with(3, p.t30_s, &p.widen[3]),
            c50_db: Evaluated::of_parameter_with(4, p.c50_db, &p.widen[4]),
            c80_db: Evaluated::of_parameter_with(5, p.c80_db, &p.widen[5]),
            d50: Evaluated::of_parameter_with(6, p.d50, &p.widen[6]),
            ts_s: Evaluated::of_parameter_with(7, p.ts_s, &p.widen[7]),
            edt: None,
            edt_validated: false,
        },
        curvature: CurvatureReport::of(p.curvature_percent),
        crossings_per_particle: p.crossings_per_particle,
        onset: p.onset,
        decay_arrival: p.decay_arrival,
        decay_curve: series.as_ref().ok().map(|s| decay::decay_curve(s, arrival)),
    }
}

/// One band of an SPPS point receiver.
#[derive(Clone, Debug, PartialEq, Serialize, JsonSchema)]
pub struct ReceiverBandReport {
    pub freq_hz: i32,
    /// SPPS's statistics show that at most one particle in a million was still alive when the
    /// steps ran out in random mode, none in energetic mode (`trans_epsilon` above 0:
    /// `spps::SppsResults::band_complete`), so the series is given to `params` as complete and no
    /// tail after its end is bounded; in energetic mode its floor still is (`floor_db`).
    /// Otherwise `params` bounds that tail. **Lost particles, and those few left alive, do not
    /// make a band incomplete:** the energy their unfinished paths would have brought is bounded
    /// separately, `lost_share`.
    pub complete: bool,
    /// The level below a particle's start at which SPPS drops it, dB, when that can cost the
    /// histogram energy (energetic mode: `-10·trans_epsilon`); `params` bounds what it can have
    /// dropped. `null` otherwise.
    pub floor_db: Option<f64>,
    /// The share of the energy from the arrival on that unfinished particles (lost, or in a
    /// complete band left alive at the end) can have taken with them
    /// (`spps::SppsResults::lost_share`), or, when `lost_follows_decay`, that lost particles can
    /// have taken of the energy from every time on (`spps::SppsResults::lost_share_following_
    /// decay`); `params` bounds what it can move. `null` when there are none.
    pub lost_share: Option<f64>,
    /// Energetic mode: what the lost particles would still have brought falls with the decay, so
    /// `lost_share` bounds the energy from every time on, not a lump added at the end.
    pub lost_follows_decay: bool,
    /// Always true for SPPS, whose reverberation begins with the first reflection: how it ran
    /// between the arrival and the first bin wholly after the direct sound is not known, so each
    /// value is read three ways, with the reverberation beginning at the arrival (the decay of
    /// that bin continued back), at that bin's start and at its end, is taken midway between the
    /// lowest and highest of the three, and is refused, `early_unresolved`, when either lies
    /// further than its limit from that midpoint
    /// (`params::EnergySeries::with_early_reverberation_unresolved`). `decay_curve` shows the
    /// first reading only.
    pub early_reverberation_unresolved: bool,
    /// The arrival C50, C80, D50 and Ts are measured from: the direct sound at the receiver's
    /// centre, `arrival_s`, spread over `±R/c` (`params::decay::Arrival::Known`), or `detected`.
    pub arrival: Arrival,
    /// What EDT, T20 and T30 are measured from (`params::decay::BandParameters::decay_arrival`):
    /// `arrival` when it fits the onset bin, or follows it within the direct sound's spread;
    /// otherwise `detected`. `null` when the series is refused.
    pub decay_arrival: Option<Arrival>,
    /// The sources whose energy reaches the receiver in this band (their `.recps` total is above
    /// 0). With more than one, the seven onset-relative parameters are refused,
    /// `several_sources`.
    pub contributing_sources: Vec<String>,
    /// What the values' Monte-Carlo noise is estimated from (`params::noise`).
    pub noise_model: NoiseModel,
    /// The receiver crossings behind the series, estimated as its total over the mean deposit;
    /// `null` when the noise has no model.
    pub crossings: Option<f64>,
    /// `n`: the crossings of the receiver per particle as the noise calibration measures them
    /// (`monte_carlo.crossings_variable`), which its correction and domain take
    /// (`params::noise::NoiseModel::multi_crossing`); `null` when the noise has no model or the
    /// series is refused.
    pub crossings_per_particle: Option<f64>,
    /// The `.recp` column, Pa² per time step.
    pub energy_pa2: Vec<f64>,
    /// Their sum.
    pub total_pa2: f64,
    /// The sources' power in this band times `ρ·c` (the `.gap`), Pa²·m²: the free field's
    /// mean-square pressure at distance `r` is this over `4πr²`.
    pub source_power_rho_c: f64,
    /// The receiver's background noise, dB (the `.gap`).
    pub background_noise_db: f64,
    /// The first bin within 20 dB of the largest; `null` when the series is refused.
    pub onset: Option<Onset>,
    pub parameters: Parameters,
    /// Sound strength G, dB ([`strength`]): `parameters.spl_db` less the free field's level at
    /// 10 m of the same sources, `source_power_rho_c` over `4π·100 m²` against the same `p₀²`, so
    /// SPPS's `ρc` cancels (ISO 3382-1:2009 A.2.1, Eqs. A.1-A.3; not Eq. A.9's `+31 dB`, which
    /// assumes `ρc ≈ 400`). With several sources, every source's energy at the receiver against
    /// every source's free field, both summed. SPL's `mc_sd`, `status` and range, shifted by the
    /// same constant; refused as SPL is.
    pub g_db: Evaluated,
    /// T20 against T30: the curved-decay flag.
    pub curvature: CurvatureReport,
    /// The Schroeder curve EDT, T20 and T30 were fitted to, thinned for display
    /// (`params::decay::DecayCurve`); `null` when the series is refused, or when several sources
    /// contribute (the decay of their sum is no source–receiver pair's).
    pub decay_curve: Option<decay::DecayCurve>,
}

/// The label of an SPPS aggregate.
pub const AGGREGATE_BANDS_SUMMED: &str = "all computed bands summed bin by bin";
/// The label of a TCR receiver's aggregate, which sums nothing.
pub const AGGREGATE_NO_SERIES: &str = "none: TCR writes no series to sum";
/// The label of the energetic sums over bands that TCR writes as its `Global` rows.
pub const AGGREGATE_ENERGETIC_SUM: &str = "energetic sum of the band levels";
/// The label of a surface receiver's or cutting plane's `Global` file.
pub const AGGREGATE_GLOBAL_FILE: &str = "all computed bands: the solver's Global file";

/// All bands of a receiver summed bin by bin (`params::aggregate`): **an aggregate, not a band,
/// and not ISO 3382-1's single-number value** (the arithmetic mean of band values), since its
/// decay is weighted by the source spectrum.
#[derive(Clone, Debug, PartialEq, Serialize, JsonSchema)]
pub struct AggregateReport {
    /// [`AGGREGATE_BANDS_SUMMED`] for SPPS; [`AGGREGATE_NO_SERIES`] for a TCR receiver.
    pub aggregate: String,
    /// The bands summed: those whose series `params` accepts. Empty for TCR.
    pub bands_hz: Vec<i32>,
    /// As for a band: the aggregate's own (its bands' particles together, counted at the least
    /// deposit of any band, with the largest lifetime spread of any); `null` for TCR.
    pub crossings_per_particle: Option<f64>,
    pub parameters: Parameters,
    /// The A-weighted level of the receiver's (or the source's) bands, from their SPL.
    pub dba: DbaReport,
    /// As for a band.
    pub curvature: CurvatureReport,
    /// As for a band.
    pub decay_curve: Option<decay::DecayCurve>,
}

/// The label of [`DbaReport`]'s sum.
pub const DBA_METHOD: &str = "energy sum over the computed octave bands of SPL plus the IEC 61672-1 A-weighting at the octave centre";

/// The A-weighted level of a receiver's computed bands (`params::level::a_weighted`): the energy
/// sum of each band's SPL plus its A-weighting. An aggregate of band levels, not of a series.
#[derive(Clone, Debug, PartialEq, Serialize, JsonSchema)]
pub struct DbaReport {
    /// [`DBA_METHOD`].
    pub method: String,
    /// dB(A) re 20 µPa. Its `mc_sd` is the bands' propagated to first order, the bands
    /// independent (SPPS runs each band's particles on its own): `√Σ (wᵢ·sdᵢ)²`, `wᵢ` band `i`'s
    /// share of the weighted energy; `status` and range as SPL's (±2.5 `mc_sd`, `ok` within 1 dB).
    /// Refused, with that band's refusal, when any band's SPL is; `no_a_weight` when a band is not
    /// an octave centre from 125 Hz to 8 kHz, the bands whose weighting is pinned.
    pub level_db: Evaluated,
    /// The bands summed (when refused, the bands it would have summed): every computed band
    /// with an A-weighting pinned.
    pub bands_hz: Vec<i32>,
    /// Their A-weighting, dB, in `bands_hz`' order.
    pub weights_db: Vec<f64>,
    /// The computed bands with no A-weighting pinned (not an octave centre from 125 Hz to 8 kHz):
    /// when there is one, `level_db` is refused `no_a_weight`. Usually empty.
    pub unweighted_hz: Vec<i32>,
}

/// The label of [`StiReport`]'s method.
pub const STI_METHOD: &str = "IEC 60268-16:2011 (edition 4), indirect method: per octave 125 Hz \
to 8 kHz, the MTF by the Schroeder equation (cl. 6.1) from the band's predicted energy response, \
from the direct sound's arrival, at the 14 modulation frequencies 0.63-12.5 Hz (A.2.2); the speech \
spectrum of Table A.4 at 60 dB(A) at 1 m on axis (J.3), carried to the receiver by the band's SPL \
less the source's free-field level at 1 m; masking by band k-1's level (Table A.1; 125 Hz \
unmasked), the reception threshold (Table A.2) and the background noise in the correction \
(A.5.3); SNR_eff within +/-15 dB, TI, MTI, and the weights of Table A.3; truncated at 1.0";

/// The weighting [`StiReport`] shows, with what cl. 8.3 asks a predicted STI to state.
pub const STI_WEIGHTING: &str = "male (IEC 60268-16:2011 Table A.3; A.3.4: male speech assesses \
a channel), female computed beside it; calculated from an MTF derived from a predicted impulse \
response (cl. 8.3)";

/// [`StiReport::noise`] when the receiver has a background noise.
pub const STI_NOISE_RECEIVER: &str = "the receiver's background noise per band (the .gap), in the \
correction's denominator (A.5.3 note 2) and in band k-1's masking level (Table A.1)";

/// [`StiReport::noise`] when it has none.
pub const STI_NOISE_NONE: &str = "none: the receiver has no background noise (0 dB in every band \
of the .gap, as config.xml writes a receiver without one), so no noise term is applied";

/// [`StiReport::monte_carlo`]: STI has no noise model yet.
pub const STI_NOISE_NOT_MODELLED: &str = "not modelled: STI carries no Monte-Carlo standard \
deviation or range; its noise is not estimated";

/// The speech transmission index at a point receiver (`params::sti`): an aggregate of its octave
/// bands 125 Hz to 8 kHz, not a band.
#[derive(Clone, Debug, PartialEq, Serialize, JsonSchema)]
pub struct StiReport {
    /// [`STI_METHOD`].
    pub method: String,
    /// [`STI_WEIGHTING`]: male is the value shown, and the STI is from a predicted response.
    pub weighting: String,
    /// The speech whose STI is shown: male.
    pub shown: sti::Gender,
    /// Male speech's STI, 0 to 1, with no `mc_sd` (`monte_carlo`); or why it is refused: a band
    /// 125 Hz to 8 kHz missing (`band_missing`) or unreadable (`band_refused`), bands that are not
    /// octaves (`not_octave_bands`), several sources (`several_sources`), or a response shorter
    /// than 1.6 s or half the reverberation time (`params_series_too_short`).
    pub male: Evaluated,
    /// Female speech's, refused as male's but needing 250 Hz to 8 kHz only.
    pub female: Evaluated,
    /// The test speech level, dB(A) at 1 m on axis (J.3).
    pub speech_level_dba_at_1m: f64,
    /// The noise applied: [`STI_NOISE_RECEIVER`] or [`STI_NOISE_NONE`].
    pub noise: String,
    /// [`STI_NOISE_NOT_MODELLED`].
    pub monte_carlo: String,
    /// The modulation frequencies, Hz, the order of every band's `mtf`.
    pub modulation_hz: Vec<f64>,
    /// One per octave band of the run from 125 Hz to 8 kHz: the room's MTF, the transfer, the
    /// speech and noise levels, and each speech's MTI.
    pub bands: Vec<sti::BandSti>,
}

/// Sound strength G of a band ([`ReceiverBandReport::g_db`]): `spl` less the free field's level at
/// 10 m of the sources whose power times `ρc` is `power_rho_c` (`params::level`). A refused SPL
/// refuses G with the same refusal; a value keeps SPL's standard deviation, status and range.
fn strength(spl: &Evaluated, power_rho_c: f64) -> Evaluated {
    let Evaluated::Value {
        value,
        mc_sd,
        status,
        lo,
        hi,
        refused_resamples,
        straddle,
    } = spl
    else {
        return spl.clone();
    };
    match level::free_field_level_db(power_rho_c) {
        Ok(free) => Evaluated::Value {
            value: value - free,
            mc_sd: *mc_sd,
            status: *status,
            lo: lo.map(|x| x - free),
            hi: hi.map(|x| x - free),
            refused_resamples: *refused_resamples,
            straddle: straddle.map(|[a, b]| [a - free, b - free]),
        },
        Err(e) => Evaluated::refused(e),
    }
}

/// [`DbaReport`] of `bands`, `(freq_hz, SPL)`: refused with the first refused band's refusal,
/// its message naming the band; otherwise `params::level::a_weighted`, with SPL's range and
/// status on the propagated standard deviation when every band has one.
fn dba_report(bands: &[(i32, &Evaluated)]) -> DbaReport {
    let weighted: Vec<(i32, f64)> = bands
        .iter()
        .filter_map(|(f, _)| level::a_weight_db(*f).map(|w| (*f, w)))
        .collect();
    let refused = bands.iter().find_map(|(f, e)| e.refusal().map(|r| (f, r)));
    let level_db = match refused {
        Some((f, r)) => Evaluated::NotEvaluable {
            not_evaluable: Refused {
                code: r.code.clone(),
                message: format!("SPL at {f} Hz is refused: {}", r.message),
                error: r.error.clone(),
            },
        },
        None => {
            let levels: Vec<(i32, f64, Option<f64>)> = bands
                .iter()
                .filter_map(|(f, e)| match e {
                    Evaluated::Value { value, mc_sd, .. } => Some((*f, *value, *mc_sd)),
                    Evaluated::NotEvaluable { .. } => None,
                })
                .collect();
            match level::a_weighted(&levels) {
                Ok((value, Some(sd))) => {
                    let r = noise::range(0, value, sd);
                    Evaluated::Value {
                        value,
                        mc_sd: Some(sd),
                        status: Some(r.status),
                        lo: Some(r.lo),
                        hi: Some(r.hi),
                        refused_resamples: None,
                        straddle: None,
                    }
                }
                Ok((value, None)) => Evaluated::bare(value, None),
                Err(e) => Evaluated::refused(e),
            }
        }
    };
    DbaReport {
        method: DBA_METHOD.into(),
        level_db,
        bands_hz: weighted.iter().map(|(f, _)| *f).collect(),
        weights_db: weighted.iter().map(|(_, w)| *w).collect(),
        unweighted_hz: bands
            .iter()
            .map(|(f, _)| *f)
            .filter(|f| level::a_weight_db(*f).is_none())
            .collect(),
    }
}

/// One source's own echogram at a receiver, one band.
#[derive(Clone, Debug, PartialEq, Serialize, JsonSchema)]
pub struct SourceBandReport {
    pub freq_hz: i32,
    /// As for the receiver's band, from this source's own arrival.
    pub arrival: Arrival,
    /// As for the receiver's band.
    pub decay_arrival: Option<Arrival>,
    pub noise_model: NoiseModel,
    pub crossings: Option<f64>,
    /// As for the receiver's band.
    pub crossings_per_particle: Option<f64>,
    /// The source's `.recp` column, Pa² per time step.
    pub energy_pa2: Vec<f64>,
    pub total_pa2: f64,
    pub onset: Option<Onset>,
    pub parameters: Parameters,
    /// As for the receiver's band, against this source's own free field at 10 m: the band's
    /// `source_power_rho_c` times this source's share of the sources' power in the band.
    pub g_db: Evaluated,
    /// As for the receiver's band.
    pub curvature: CurvatureReport,
    /// As for the receiver's band.
    pub decay_curve: Option<decay::DecayCurve>,
}

/// One source's own echogram at a receiver (`output_recp_bysource`): the parameters of that
/// source–receiver pair, as ISO 3382-1 defines them.
#[derive(Clone, Debug, PartialEq, Serialize, JsonSchema)]
pub struct SourceReceiverReport {
    pub source: String,
    /// Relative to `solve/`.
    pub file: String,
    /// The direct sound's arrival from this source, as for the receiver.
    pub arrival_s: Option<f64>,
    pub bands: Vec<SourceBandReport>,
    pub aggregate: AggregateReport,
}

/// An SPPS point receiver.
#[derive(Clone, Debug, PartialEq, Serialize, JsonSchema)]
pub struct SppsReceiverReport {
    pub label: String,
    /// Relative to `solve/`.
    pub folder: String,
    pub position_m: Option<[f64; 3]>,
    /// The direct sound's arrival at the receiver's centre, which every onset-relative parameter
    /// is measured from (`params::decay::Arrival::Known`): the earliest over the sources;
    /// `null` when it is not computed (a celerity gradient, or a position not read), and the
    /// parameters then use `Arrival::Detected`.
    pub arrival_s: Option<f64>,
    pub bands: Vec<ReceiverBandReport>,
    pub aggregate: AggregateReport,
    /// The speech transmission index at the receiver (results version 10).
    pub sti: StiReport,
    /// Each source's total per band, Pa² (`.recps`), in `config.xml`'s order.
    pub by_source: Vec<SourceTotals>,
    /// Each source's own echogram and its parameters, when `output_recp_bysource` is on; empty
    /// otherwise.
    pub per_source: Vec<SourceReceiverReport>,
}

/// One surface-receiver or cutting-plane file, summarised (the values stay in the file; M12 reads
/// it with the `.csbin` reader).
#[derive(Clone, Debug, PartialEq, Serialize, JsonSchema)]
pub struct SurfaceSummary {
    /// Relative to `solve/`.
    pub path: String,
    /// TCR's field; `null` for SPPS.
    pub field: Option<String>,
    /// The band; `null` for the `Global` file, **an aggregate of all bands**.
    pub band_hz: Option<i32>,
    /// [`AGGREGATE_GLOBAL_FILE`] for the `Global` file; `null` for a band's.
    pub aggregate: Option<String>,
    /// The cutting-plane file (`rs_cut.csbin`), not the surface receivers' (`Sound level.csbin`):
    /// told apart by the file's name, and each holds exactly the receivers `config.xml` gives it.
    pub cutting_plane: bool,
    /// `recordType` (`docs/formats/csbin.md`).
    pub record_type: String,
    pub time_steps: u32,
    pub time_step_s: f64,
    pub receivers: Vec<SurfaceReceiverSummary>,
}

/// One receiver of a surface file.
#[derive(Clone, Debug, PartialEq, Serialize, JsonSchema)]
pub struct SurfaceReceiverSummary {
    /// `xmlIndex`: its id in `config.xml`, of a `recepteur_surfacique` in a receivers' file and of
    /// a `recepteur_surfacique_coupe` in a cutting planes' file.
    pub id: i32,
    pub name: String,
    pub faces: usize,
    pub records: usize,
    /// The sum of every stored value.
    pub value_sum: f64,
}

impl SurfaceSummary {
    fn of(s: &SurfaceFile) -> Self {
        SurfaceSummary {
            path: s.path.clone(),
            field: s.field.clone(),
            band_hz: s.band_hz,
            aggregate: s
                .band_hz
                .is_none()
                .then(|| AGGREGATE_GLOBAL_FILE.to_string()),
            cutting_plane: s.cutting_plane,
            record_type: format!("{:?}", s.data.record_type),
            time_steps: s.data.time_step_count,
            time_step_s: f64::from(s.data.time_step),
            receivers: s
                .data
                .receivers
                .iter()
                .map(|r| SurfaceReceiverSummary {
                    id: r.xml_index,
                    name: r.name_lossy().into_owned(),
                    faces: r.faces.len(),
                    records: r.faces.iter().map(|f| f.records.len()).sum(),
                    value_sum: r
                        .faces
                        .iter()
                        .flat_map(|f| f.records.iter())
                        .map(|v| f64::from(v.energy))
                        .sum(),
                })
                .collect(),
        }
    }
}

/// How every SPPS value's Monte-Carlo noise was estimated and judged (`params::noise`).
#[derive(Clone, Debug, PartialEq, Serialize, JsonSchema)]
pub struct MonteCarloReport {
    /// Resampled series per estimate.
    pub resamples: usize,
    /// Resamples that may refuse a quantity before its value is refused.
    pub refused_resamples_allowed: usize,
    /// The bootstrap's seed, as `0x` and 16 hex digits: as a JSON number above 2⁵³ it would not
    /// survive a reader that parses numbers as doubles.
    #[serde(serialize_with = "crate::params::serialize_seed")]
    #[schemars(with = "String", pattern(crate::params::SEED_PATTERN))]
    pub seed: u64,
    /// The largest standard deviation a value may carry: EDT, T20 and T30 relative, C50 and C80
    /// in dB, D50 as a fraction, Ts in s, SPL in dB.
    pub limit_decay_relative: f64,
    pub limit_clarity_db: f64,
    pub limit_definition: f64,
    pub limit_centre_time_s: f64,
    pub limit_spl_db: f64,
    /// The run's computation method, which picks the calibration.
    pub method: noise::Method,
    /// What `n`, the crossings of a receiver per particle each band's `crossings_per_particle`
    /// gives, is for this method (`params::noise::calibration::variable`).
    pub crossings_variable: noise::calibration::Variable,
    /// What the calibration was measured on beyond what the code checks (particles and crossings
    /// per particle, in `calibration`): [`noise::calibration::MEASURED_ON`].
    pub measured_on: String,
    /// Energetic T20 and T30: a band whose every face is Lambert with scattering 1 and has the
    /// same absorption takes `uniform_lambert_walls` up to this mean absorption, and the other
    /// bands' entry above it.
    pub uniform_lambert_max_mean_absorption: f64,
    /// Per quantity, by name, for this computation method (`params::noise::calibration`;
    /// `docs/investigations/2026-09-25-noise-calibration/`): each value's standard deviation is
    /// the bootstrap's times `factor·√(1 + kappa·n)`, and a value is given only with at least
    /// `min_particles` particles per source and `n` at most `max_crossings_per_particle`.
    pub calibration: std::collections::BTreeMap<String, QuantityCalibration>,
}

/// How one quantity's noise is calibrated, where the calibration holds, and how a refusal names a
/// particle count (`params::noise::calibration::Entry`).
#[derive(Clone, Debug, PartialEq, Serialize, JsonSchema)]
pub struct QuantityCalibration {
    /// For bands not every face of which reflects by Lambert's law with scattering 1 (and for
    /// every band, when the two below are `null`), with the structure its resamples are drawn
    /// with (`structure`: `constant`, or since round 4 `roughness` for energetic T20 and T30).
    #[serde(flatten)]
    pub entry: noise::calibration::Entry,
    /// Energetic T20 and T30 only: the entry for bands whose every face reflects by Lambert's law
    /// with scattering 1 and not every face has the same absorption (or the absorption is above
    /// `uniform_lambert_max_mean_absorption`), the same as `entry` since round 4; `null`
    /// otherwise.
    pub lambert_walls: Option<noise::calibration::Entry>,
    /// Energetic T20 and T30 only: the entry for bands whose every face reflects by Lambert's law
    /// with scattering 1 and has the same absorption, at most
    /// `uniform_lambert_max_mean_absorption`; `null` otherwise.
    pub uniform_lambert_walls: Option<noise::calibration::Entry>,
}

impl MonteCarloReport {
    fn current(method: noise::Method) -> Self {
        use noise::limits;
        MonteCarloReport {
            resamples: noise::RESAMPLES,
            refused_resamples_allowed: noise::REFUSED_RESAMPLES_ALLOWED,
            seed: noise::SEED,
            limit_decay_relative: limits::DECAY_RELATIVE,
            limit_clarity_db: limits::CLARITY_DB,
            limit_definition: limits::DEFINITION,
            limit_centre_time_s: limits::CENTRE_TIME_S,
            limit_spl_db: limits::SPL_DB,
            method,
            crossings_variable: noise::calibration::variable(method),
            measured_on: noise::calibration::MEASURED_ON.into(),
            uniform_lambert_max_mean_absorption:
                noise::calibration::UNIFORM_LAMBERT_MAX_MEAN_ABSORPTION,
            calibration: noise::QUANTITY_NAMES
                .iter()
                .enumerate()
                .map(|(i, q)| {
                    use noise::Walls;
                    let entry = noise::calibration::entry(method, i, Walls::Other);
                    let lambert = noise::calibration::entry(method, i, Walls::Lambert);
                    let uniform = noise::calibration::entry(method, i, Walls::UniformLambert);
                    let split = method == noise::Method::Energetic && (i == 2 || i == 3);
                    (
                        q.to_string(),
                        QuantityCalibration {
                            entry,
                            lambert_walls: split.then_some(lambert),
                            uniform_lambert_walls: split.then_some(uniform),
                        },
                    )
                })
                .collect(),
        }
    }
}

/// An SPPS run.
#[derive(Clone, Debug, PartialEq, Serialize, JsonSchema)]
pub struct SppsReport {
    pub time_step_s: f64,
    pub duration_s: f64,
    pub steps: usize,
    pub speed_of_sound_m_s: f64,
    pub receiver_radius_m: f64,
    /// `2R/c`: the direct sound is spread over this long at a receiver.
    pub receiver_crossing_s: f64,
    pub celerity_gradient: bool,
    /// `computation_method`: 0 random, 1 energetic.
    pub computation_method: i32,
    /// `nbparticules`: particles per source and band.
    pub particles_per_source: u32,
    /// `trans_epsilon` as SPPS reads it.
    pub trans_epsilon: f64,
    /// `output_recp_bysource`.
    pub echogram_per_source: bool,
    pub monte_carlo: MonteCarloReport,
    pub sources: Vec<SourcePoint>,
    pub particles: ParticleStats,
    /// The energy in the room per band and step (`<cumul_filename>`).
    pub total_energy: Vec<BandEnergy>,
    pub point_receivers: Vec<SppsReceiverReport>,
    pub surfaces: Vec<SurfaceSummary>,
    pub particle_files: Vec<ParticleFileSummary>,
    /// The analytic reference on the run's own inputs (`results::reference`): Kuttruff's
    /// corrected Eyring with `γ²` from the room's geometry, M8's reference, and plain Eyring,
    /// reported only. **Not validated.**
    pub reference: ReferenceReport,
}

/// One band of [`ReferenceReport`].
#[derive(Clone, Debug, PartialEq, Serialize, JsonSchema)]
pub struct ReferenceBandReport {
    pub freq_hz: i32,
    /// The energy attenuation the solver applies, added as `4·m·V`, 1/m; `null` with air
    /// absorption off.
    pub air_m_per_metre: Option<f64>,
    /// `ᾱ = Σ Sᵢ·αᵢ / S` over the room's faces.
    pub mean_absorption: f64,
    /// Every face reflects by Lambert's law with scattering 1 in this band: the only walls the
    /// transport's `γ²` describes. When false, neither time describes the run's field.
    pub lambert_walls: bool,
    /// Every face has the same absorption in this band.
    pub uniform_absorption: bool,
    /// Plain Eyring, `K·V/(4·m·V − S·ln(1 − ᾱ))`, s, with SPPS's `K`: **reported only**.
    pub eyring_s: Evaluated,
    /// Kuttruff's corrected Eyring, `K·V/(4·m·V + A_K)`, s, with `γ²` from the room's geometry:
    /// **M8's reference**. `mc_sd` is only the standard deviation it inherits from the transport's
    /// `γ²`, not its total uncertainty: it leaves out the formula's own error against a diffuse
    /// room (−0.41 % to +0.59 % in M8's cells, `docs/params.md`, "Kuttruff's reference"; not
    /// measured in other rooms). Refused with the transport's own refusal when the transport
    /// refused, and `params_reference_not_applicable` where `lambert_walls` is false: it is
    /// computed only where it describes the band.
    pub kuttruff_s: Evaluated,
}

/// The analytic reference of an SPPS run, or why there is none (`results::reference`).
#[derive(Clone, Debug, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case", tag = "status")]
pub enum ReferenceReport {
    Computed {
        /// Always [`crate::results::reference::REFERENCE_LABEL`].
        label: String,
        /// The `.mbin`'s volume, m³.
        volume_m3: f64,
        /// The `.cbin` faces' total area, m².
        area_m2: f64,
        /// SPPS's speed of sound, m/s.
        speed_of_sound_m_s: f64,
        /// `K = 24·ln 10/c`, s/m.
        constant_s_per_m: f64,
        /// The diffuse transport's free paths in the room: the mean free path, `γ²`, their
        /// standard errors, `4V/S`, and the transport's settings. `null` when the transport
        /// refused, every band with Lambert walls then carrying that refusal in `kuttruff_s`
        /// (the others refused `params_reference_not_applicable`, as always); and when no computed
        /// band has Lambert walls, so that it was not run, every `kuttruff_s` then refused
        /// `params_reference_not_applicable`.
        free_paths: Option<FreePaths>,
        bands: Vec<ReferenceBandReport>,
    },
    NotComputed {
        why: String,
    },
}

impl ReferenceReport {
    fn of(r: &Reference) -> Self {
        match r {
            Reference::Computed {
                volume_m3,
                area_m2,
                speed_of_sound_m_s,
                constant_s_per_m,
                free_paths,
                bands,
            } => ReferenceReport::Computed {
                label: REFERENCE_LABEL.into(),
                volume_m3: *volume_m3,
                area_m2: *area_m2,
                speed_of_sound_m_s: *speed_of_sound_m_s,
                constant_s_per_m: *constant_s_per_m,
                free_paths: free_paths.as_ref().ok().cloned(),
                bands: bands
                    .iter()
                    .map(|b| ReferenceBandReport {
                        freq_hz: b.freq_hz,
                        air_m_per_metre: b.air_m_per_metre,
                        mean_absorption: b.mean_absorption,
                        lambert_walls: b.lambert_walls,
                        uniform_absorption: b.uniform_absorption,
                        eyring_s: Evaluated::of(b.eyring_s.clone()),
                        kuttruff_s: match &b.kuttruff_s {
                            Ok((value, sd)) => Evaluated::bare(*value, Some(*sd)),
                            Err(e) => Evaluated::refused(e.clone()),
                        },
                    })
                    .collect(),
            },
            Reference::NotComputed { why } => ReferenceReport::NotComputed { why: why.clone() },
        }
    }
}

/// TCR's `Global` row: the energetic sum of the band levels, **an aggregate**.
#[derive(Clone, Debug, PartialEq, Serialize, JsonSchema)]
pub struct TcrGlobal {
    /// Always [`AGGREGATE_ENERGETIC_SUM`].
    pub aggregate: String,
    pub sabine_level_db: f64,
    pub eyring_level_db: f64,
}

/// A TCR receiver's `Global` row: each column's energetic sum over the bands, **an aggregate**
/// (`ctr/input_output/reportmanager.cpp:131-143`).
#[derive(Clone, Debug, PartialEq, Serialize, JsonSchema)]
pub struct TcrReceiverGlobal {
    /// Always [`AGGREGATE_ENERGETIC_SUM`].
    pub aggregate: String,
    pub direct_db: f64,
    pub total_sabine_db: f64,
    pub total_eyring_db: f64,
}

/// `core::params`' Sabine and Eyring times for one band, on the run's inputs.
#[derive(Clone, Debug, PartialEq, Serialize, JsonSchema)]
pub struct AnalyticBandReport {
    pub freq_hz: i32,
    /// The energy attenuation TCR adds as `4·m·V`, 1/m; `null` with air absorption off.
    pub air_m_per_metre: Option<f64>,
    /// s, with TCR's constant 0.163.
    pub sabine_s: Evaluated,
    /// s, with TCR's constant 0.163.
    pub eyring_s: Evaluated,
}

/// The analytic references on the run's own inputs (`tcr::analytic`), or why there are none.
#[derive(Clone, Debug, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case", tag = "status")]
pub enum AnalyticReport {
    Computed {
        volume_m3: f64,
        area_m2: f64,
        bands: Vec<AnalyticBandReport>,
    },
    NotComputed {
        why: String,
    },
}

/// One band of a TCR point receiver: TCR's own levels, dB, and the eight parameters, every one
/// refused `no_time_series`.
#[derive(Clone, Debug, PartialEq, Serialize, JsonSchema)]
pub struct TcrReceiverBandReport {
    pub freq_hz: i32,
    pub direct_db: f64,
    pub total_sabine_db: f64,
    pub total_eyring_db: f64,
    /// TCR writes steady-state levels, not an energy time series, so `core::params` has nothing
    /// to compute from: each of the eight is `params_not_evaluable`, `no_time_series`. Its SPL
    /// too, because TCR gives two totals, Sabine's and Eyring's, and neither is `params`' SPL of a
    /// series; they are `total_sabine_db` and `total_eyring_db`.
    pub parameters: Parameters,
    /// Sound strength G, refused as SPL is (`no_time_series`).
    pub g_db: Evaluated,
    /// Refused as the parameters are.
    pub curvature: CurvatureReport,
    /// Always `null`: no series, no curve.
    pub decay_curve: Option<decay::DecayCurve>,
}

/// A TCR point receiver, in the same shape as an SPPS one where the two meet: `label`, `bands[]`
/// with `freq_hz`, `parameters`, `curvature` and `decay_curve`, and `aggregate`.
#[derive(Clone, Debug, PartialEq, Serialize, JsonSchema)]
pub struct TcrReceiverReport {
    /// The file's name without `.gabe`: exactly one `recepteur_ponctuel@lbl`.
    pub label: String,
    /// Relative to `solve/`.
    pub file: String,
    pub bands: Vec<TcrReceiverBandReport>,
    /// The `Global` row, **an aggregate**, labelled.
    pub global: TcrReceiverGlobal,
    /// SPPS's shape, labelled [`AGGREGATE_NO_SERIES`]: no band summed, every parameter refused
    /// `no_time_series`.
    pub aggregate: AggregateReport,
}

impl TcrReceiverReport {
    fn of(r: &tcr::PointReceiver) -> Self {
        let detail = format!(
            "TCR writes steady-state levels only; its own for this receiver are direct_db, \
             total_sabine_db and total_eyring_db ({})",
            r.file
        );
        let parameters = Parameters::no_time_series(&detail);
        let curvature = CurvatureReport::of(Err(params::not_evaluable(
            Quantity::Curvature,
            NotEvaluable::NoTimeSeries { detail },
        )));
        TcrReceiverReport {
            label: r.label.clone(),
            file: r.file.clone(),
            bands: r
                .bands
                .iter()
                .map(|b| TcrReceiverBandReport {
                    freq_hz: b.freq_hz,
                    direct_db: b.direct_db,
                    total_sabine_db: b.total_sabine_db,
                    total_eyring_db: b.total_eyring_db,
                    // Refused as SPL is: `strength` of a refusal is that refusal.
                    g_db: strength(&parameters.spl_db, f64::NAN),
                    parameters: parameters.clone(),
                    curvature: curvature.clone(),
                    decay_curve: None,
                })
                .collect(),
            global: TcrReceiverGlobal {
                aggregate: AGGREGATE_ENERGETIC_SUM.into(),
                direct_db: r.global_direct_db,
                total_sabine_db: r.global_total_sabine_db,
                total_eyring_db: r.global_total_eyring_db,
            },
            aggregate: AggregateReport {
                aggregate: AGGREGATE_NO_SERIES.into(),
                bands_hz: Vec::new(),
                crossings_per_particle: None,
                dba: dba_report(
                    &r.bands
                        .iter()
                        .map(|b| (b.freq_hz, &parameters.spl_db))
                        .collect::<Vec<_>>(),
                ),
                parameters,
                curvature,
                decay_curve: None,
            },
        }
    }
}

/// A TCR run: its values as TCR computed them, and `core::params`' analytic Sabine and Eyring
/// times on the same inputs. TCR writes no time series, so every per-receiver parameter is
/// refused, `no_time_series`.
#[derive(Clone, Debug, PartialEq, Serialize, JsonSchema)]
pub struct TcrReport {
    pub bands: Vec<MainBand>,
    pub global: TcrGlobal,
    pub point_receivers: Vec<TcrReceiverReport>,
    pub surfaces: Vec<SurfaceSummary>,
    pub analytic: AnalyticReport,
}

/// What `simpa results <run> --json` prints.
#[derive(Clone, Debug, PartialEq, Serialize, JsonSchema)]
pub struct Report {
    /// [`REPORT_VERSION`].
    pub results_version: u32,
    /// False until M8's physics bed passes: no number here may be shown to a user.
    pub validated_by_bed: bool,
    /// The run folder, as given.
    pub run_folder: String,
    pub solver: SolverKind,
    /// Always OK: any other run is refused.
    pub status: Status,
    /// Whether the run's solver build was verified (backlog 38, [`super::solver_build`]): it marks
    /// the run and refuses nothing.
    pub solver_build: SolverBuild,
    /// `run.json`'s `started`.
    pub started: String,
    /// The computed bands, ascending.
    pub bands_hz: Vec<i32>,
    /// Present for an SPPS run.
    pub spps: Option<SppsReport>,
    /// Present for a TCR run.
    pub tcr: Option<TcrReport>,
}

/// A band's series as `params` gets it: complete when the statistics say so, with the run's floor
/// when it has one, and the share its lost particles can have taken from the arrival's step on
/// (the onset bin's, when the arrival is not known).
fn series_of(
    s: &SppsResults,
    index: usize,
    freq_hz: i32,
    energy: &[f64],
    arrival: Arrival,
) -> Result<EnergySeries, ParamError> {
    // SPPS's reverberation begins with the first reflection, not with the direct sound.
    let base = if s.band_complete(freq_hz) {
        EnergySeries::complete(s.time_step_s, energy.to_vec())
    } else {
        EnergySeries::new(s.time_step_s, energy.to_vec())
    }?
    .with_early_reverberation_unresolved();
    let bin = match arrival {
        Arrival::Known { time_s, .. } => (time_s / s.time_step_s).floor().max(0.0) as usize,
        Arrival::Detected => decay::onset(&base).index,
    };
    let base = match s.floor_db() {
        // The share alive the floor divides by: the smallest over the decay above the floor, from
        // the room table bin by bin (`params::floor_alive_share`: in coupled rooms the particles
        // the floor drops bring more per unit of energy than those alive at the arrival). Nothing
        // known alive: the smallest share, so that nothing bounds what was dropped.
        Some(floor) => {
            let alive: Vec<f64> = (0..energy.len())
                .map(|k| s.alive_share(index, k).unwrap_or(f64::NAN))
                .collect();
            base.with_solver_floor(
                floor,
                params::floor_alive_share(energy, &alive, bin, floor)
                    .filter(|a| *a > 0.0)
                    .unwrap_or(f64::MIN_POSITIVE),
            )?
        }
        None => base,
    };
    // Energetic mode: what the lost particles would still have brought follows the decay.
    if let Some(share) = s.lost_share_following_decay(freq_hz) {
        return base.with_lost_share_following_decay(share);
    }
    match s.lost_share(index, freq_hz, bin) {
        Some(share) => base.with_lost_share(share),
        None => Ok(base),
    }
}

/// An estimate of the most energy band `index`'s series (`freq_hz`, `energy`) can lack, as a share
/// of what it holds from bin `from` on, for STI (`sti::ReceiverBand::unseen_share`), taken as the
/// decay quantities take it (`series_of`); `None` when there is nothing to estimate it from. Three
/// parts, summed:
/// - **Particles alive at the run's end**, when the band is not complete
///   ([`SppsResults::band_complete`]): the room table at the last step holds their energy, a
///   share `alive_end` of the emitted. **A heuristic, not a proof:** what a particle alive at the
///   end will still bring per unit of its energy is assumed to be no more than what the particles
///   alive brought per unit of theirs over the decay above the floor, as the floor's estimate
///   assumes (`params::floor_alive_share` from `from`), giving `alive_end / share` of the energy
///   from `from` on. Nothing guarantees it: a particle alive late in a coupled space, or near the
///   receiver, can bring more per unit than the average did.
/// - **The floor**, `10^{floor/10} / share` (`EnergySeries::with_solver_floor`).
/// - **Lost particles**, their share (`SppsResults::lost_share_following_decay` in energetic mode,
///   `SppsResults::lost_share` otherwise).
///
/// A run whose `trans_epsilon` is not above 0 drops every particle at its first surface: `None`.
fn sti_unseen_share(
    s: &SppsResults,
    index: usize,
    freq_hz: i32,
    energy: &[f64],
    from: usize,
) -> Option<f64> {
    if s.trans_epsilon.is_nan() || s.trans_epsilon <= 0.0 {
        return None;
    }
    let floor = s.floor_db();
    let share = || {
        let alive: Vec<f64> = (0..energy.len())
            .map(|k| s.alive_share(index, k).unwrap_or(f64::NAN))
            .collect();
        params::floor_alive_share(energy, &alive, from, floor.unwrap_or(f64::NEG_INFINITY))
            .filter(|a| a.is_finite() && *a > 0.0)
    };
    let mut x = 0.0;
    if !s.band_complete(freq_hz) {
        let end = s.alive_share(index, energy.len().checked_sub(1)?)?;
        if !(end.is_finite() && end >= 0.0) {
            return None;
        }
        if end > 0.0 {
            x += end / share()?;
        }
    }
    if let Some(db) = floor {
        x += 10f64.powf(db / 10.0) / share()?;
    }
    match s.lost_share_following_decay(freq_hz) {
        Some(l) => x += l,
        None => x += s.lost_share(index, freq_hz, from).unwrap_or(0.0),
    }
    x.is_finite().then_some(x)
}

/// The receiver crossings behind `total` under `model`.
fn crossings(model: &NoiseModel, total: f64) -> Option<f64> {
    match model {
        NoiseModel::Crossings { mean_deposit, .. } => Some(total / mean_deposit),
        NoiseModel::Unknown { .. } => None,
    }
}

/// The aggregate's noise model: crossings of the largest band deposit, under the bands' method
/// and particle count; its run the bands' together (the least deposit of any band, the largest
/// lifetime spread, Lambert only when every band is, and the bands counted, each with its own
/// particles); unknown when any band's is, or a band's model is no run's while another's is.
fn aggregate_model(models: &[&NoiseModel]) -> NoiseModel {
    let mut largest: Option<(f64, noise::Method, Option<u32>)> = None;
    let mut run: Option<noise::RunNoise> = None;
    let mut runless = false;
    for m in models {
        match m {
            NoiseModel::Crossings {
                mean_deposit,
                method,
                particles,
                run: r,
            } => {
                if largest.is_none_or(|(l, _, _)| *mean_deposit > l) {
                    largest = Some((*mean_deposit, *method, *particles));
                }
                match (r, &mut run) {
                    (None, _) => runless = true,
                    (Some(r), None) => run = Some(r.clone()),
                    (Some(r), Some(a)) => {
                        a.least_deposit = a.least_deposit.min(r.least_deposit);
                        a.lifetime_cv2 = a.lifetime_cv2.max(r.lifetime_cv2);
                        a.lambert_walls &= r.lambert_walls;
                        a.uniform_absorption &= r.uniform_absorption;
                        a.mean_absorption = a.mean_absorption.max(r.mean_absorption);
                        a.particles = a.particles.min(r.particles);
                        a.bands += r.bands;
                    }
                }
            }
            NoiseModel::Unknown { detail } => {
                return NoiseModel::Unknown {
                    detail: detail.clone(),
                };
            }
        }
    }
    let Some((mean_deposit, method, particles)) = largest else {
        return NoiseModel::Unknown {
            detail: "no band to aggregate".into(),
        };
    };
    match (run, runless) {
        (Some(_), true) => NoiseModel::Unknown {
            detail: "the bands' noise models are not all from the run".into(),
        },
        (Some(run), false) => {
            NoiseModel::of_run(mean_deposit, method, run).unwrap_or_else(|e| NoiseModel::Unknown {
                detail: e.to_string(),
            })
        }
        (None, _) => NoiseModel::Crossings {
            mean_deposit,
            method,
            particles,
            run: None,
        },
    }
}

/// The aggregate of `series` (one per band, `bands_hz`), with its noise model from `models`.
fn aggregate_report(
    s: &SppsResults,
    bands_hz: &[i32],
    series: &[Result<EnergySeries, ParamError>],
    models: &[NoiseModel],
    arrival: Arrival,
    contributing: &[&str],
    dba: DbaReport,
) -> AggregateReport {
    let mut valid = Vec::new();
    let mut summed = Vec::new();
    let mut used: Vec<&NoiseModel> = Vec::new();
    for ((&f, se), m) in bands_hz.iter().zip(series).zip(models) {
        if let Ok(x) = se {
            valid.push(f);
            summed.push(x.clone());
            used.push(m);
        }
    }
    let aggregate = params::aggregate(&summed).map_err(|e| {
        // No band holds energy: say so with the series' own refusal.
        if summed.is_empty() {
            ParamError::NoEnergy
        } else {
            e
        }
    });
    let mut e = evaluated(&aggregate, arrival, &aggregate_model(&used));
    e.set_edt(edt_report(s, &aggregate, arrival, true));
    if contributing.len() > 1 {
        e.several_sources(contributing);
    }
    AggregateReport {
        aggregate: AGGREGATE_BANDS_SUMMED.into(),
        bands_hz: valid,
        crossings_per_particle: e.crossings_per_particle,
        parameters: e.parameters,
        dba,
        curvature: e.curvature,
        decay_curve: e.decay_curve,
    }
}

/// Source `name`'s power in band `index` times `ρc`: `total_rho_c`, the `.gap`'s sources' power
/// times `ρc`, times its share of the sources' power in the band (as [`SppsResults::mean_deposit`]
/// takes it). 0 when no source emits in the band; NaN when the source's power is not known, which
/// G refuses.
fn source_power_rho_c(s: &SppsResults, index: usize, total_rho_c: f64, name: &str) -> f64 {
    let total: f64 = s
        .sources
        .iter()
        .filter_map(|x| x.band_power_w.get(index))
        .sum();
    match s
        .sources
        .iter()
        .find(|x| x.name == name)
        .and_then(|x| x.band_power_w.get(index))
    {
        Some(w) if total > 0.0 => total_rho_c * w / total,
        Some(_) => 0.0,
        None => f64::NAN,
    }
}

/// [`dba_report`] of band reports' SPL.
fn dba_of<'a>(bands: impl Iterator<Item = (i32, &'a Parameters)>) -> DbaReport {
    dba_report(&bands.map(|(f, p)| (f, &p.spl_db)).collect::<Vec<_>>())
}

/// The arrival `params` measures from: at the receiver's centre, `t`, with the direct sound spread
/// over the time a particle takes to cross the receiver ball, `t ± R/c`
/// ([`SppsResults::receiver_crossing_s`]); [`Arrival::Detected`] when `t` is not known.
fn known_arrival(s: &SppsResults, t: Option<f64>) -> Arrival {
    t.map_or(Arrival::Detected, |t| {
        Arrival::spread(t, s.receiver_crossing_s() / 2.0)
    })
}

/// The direct sound's arrival `a` holds, s; `None` when it is to be detected.
fn arrival_time(a: Arrival) -> Option<f64> {
    match a {
        Arrival::Known { time_s, .. } => Some(time_s),
        Arrival::Detected => None,
    }
}

/// EDT v2.1 ([`edt`]) on `series`' histogram as the solver wrote it, with `arrival`'s time, which
/// is `SppsResults::arrival_from`'s: the source's emission delay included, rounded up to the next
/// whole step (`spps::emission_s`). Without that delay the method reads the decay from before
/// the sound has left the source (VERDICT-2, "A port requirement from the attack"). `None` when
/// the series itself is refused: every parameter then carries that refusal.
fn edt_report(
    s: &SppsResults,
    series: &Result<EnergySeries, ParamError>,
    arrival: Arrival,
    broadband: bool,
) -> Option<EdtReport> {
    let bins = series.as_ref().ok()?.values();
    let t_arrival = arrival_time(arrival);
    let o = edt::analyse(
        bins,
        s.time_step_s,
        t_arrival,
        Some(s.receiver_crossing_s() / 2.0),
    );
    // The held-out test passed single bands with receivers up to 1 m that the direct sound
    // reached, in either mode; outside any of these the EDT is not validated, and the note names
    // every reason that applies.
    let reasons: Vec<&str> = [
        (broadband, EDT_BROADBAND_NOT_VALIDATED),
        (o.no_direct_path, EDT_NO_DIRECT_PATH_NOT_VALIDATED),
        (
            s.receiver_radius_m > edt::VALIDATED_MAX_RADIUS_M,
            EDT_LARGE_RECEIVER_NOT_VALIDATED,
        ),
    ]
    .into_iter()
    .filter_map(|(applies, note)| applies.then_some(note))
    .collect();
    let validated = reasons.is_empty();
    Some(EdtReport {
        method: edt::METHOD.into(),
        status: o.status,
        value_s: o.edt,
        lo_s: o.edt_lo,
        hi_s: o.edt_hi,
        reason: o.reason,
        arrival_s: t_arrival,
        validated,
        validation_note: (!validated).then(|| reasons.join("; ")),
    })
}

impl EdtReport {
    /// `Parameters::edt_s`: the value with the method's range and status, or the refusal as
    /// `edt_refused`.
    fn evaluated(&self) -> Evaluated {
        match self.value_s {
            Some(value) if self.status != edt::Status::Refused => Evaluated::Value {
                value,
                mc_sd: None,
                status: Some(if self.status == edt::Status::Ok {
                    noise::RangeStatus::Ok
                } else {
                    noise::RangeStatus::Wide
                }),
                lo: self.lo_s,
                hi: self.hi_s,
                refused_resamples: None,
                straddle: None,
            },
            _ => Evaluated::refused(params::not_evaluable(
                Quantity::Edt,
                NotEvaluable::EdtRefused {
                    reason: self.reason.clone(),
                },
            )),
        }
    }
}

impl Evaluation {
    /// EDT is the port's ([`edt_report`]); the old estimate stays in `params` for the noise
    /// calibration's own evidence and does not reach the report. `None` leaves what the series'
    /// refusal gave.
    fn set_edt(&mut self, r: Option<EdtReport>) {
        if let Some(r) = &r {
            self.parameters.edt_s = r.evaluated();
        }
        self.parameters.edt_validated = r.as_ref().is_some_and(|r| r.validated);
        self.parameters.edt = r;
    }
}

/// [`StiReport`] of a receiver's bands, refused `several_sources` when `contributing` names more
/// than one source: STI is a talker's, one source's.
fn sti_report(
    s: &SppsResults,
    inputs: &[sti::ReceiverBand<'_>],
    octave: bool,
    contributing: &[&str],
) -> StiReport {
    let r = sti::receiver_sti(s.time_step_s, inputs, octave);
    let shown = |v: Result<sti::StiValue, ParamError>| match v {
        _ if contributing.len() > 1 => Evaluated::refused(params::not_evaluable(
            Quantity::Sti,
            NotEvaluable::SeveralSources {
                sources: contributing.iter().map(|c| c.to_string()).collect(),
            },
        )),
        Ok(v) => Evaluated::bare(v.value, None),
        Err(e) => Evaluated::refused(e),
    };
    let noisy = inputs.iter().any(|b| b.noise_db.is_some());
    StiReport {
        method: STI_METHOD.into(),
        weighting: STI_WEIGHTING.into(),
        shown: sti::Gender::Male,
        male: shown(r.male),
        female: shown(r.female),
        speech_level_dba_at_1m: sti::SPEECH_LEVEL_DBA_AT_1M,
        noise: if noisy {
            STI_NOISE_RECEIVER
        } else {
            STI_NOISE_NONE
        }
        .into(),
        monte_carlo: STI_NOISE_NOT_MODELLED.into(),
        modulation_hz: sti::MODULATION_HZ.to_vec(),
        bands: r.bands,
    }
}

/// The value of an evaluated quantity, or its refusal in words.
fn value_or_why(e: &Evaluated) -> Result<f64, String> {
    match e {
        Evaluated::Value { value, .. } => Ok(*value),
        Evaluated::NotEvaluable { not_evaluable } => Err(not_evaluable.message.clone()),
    }
}

fn receiver_report(
    bands_hz: &[i32],
    s: &SppsResults,
    r: &PointReceiver,
    octave: bool,
) -> SppsReceiverReport {
    let arrival_s = s.arrival_s(r);
    let arrival = known_arrival(s, arrival_s);
    let mut sti_inputs: Vec<sti::ReceiverBand<'_>> = Vec::with_capacity(r.bands.len());
    let mut series: Vec<Result<EnergySeries, ParamError>> = Vec::with_capacity(r.bands.len());
    let mut models = Vec::with_capacity(r.bands.len());
    let mut all_contributing: Vec<&str> = Vec::new();
    let mut bands = Vec::with_capacity(r.bands.len());
    for (i, b) in r.bands.iter().enumerate() {
        let contributing = r.contributing(i);
        for c in &contributing {
            if !all_contributing.contains(c) {
                all_contributing.push(c);
            }
        }
        // The largest deposit of the sources that reach the receiver; of all of them when none
        // does (the series is then refused anyway).
        let names: Vec<&str> = if contributing.is_empty() {
            s.sources.iter().map(|x| x.name.as_str()).collect()
        } else {
            contributing.clone()
        };
        let model = s.noise_model(i, &names);
        let arrival = if contributing.is_empty() {
            arrival
        } else {
            known_arrival(s, s.arrival_from(r, &contributing))
        };
        let se = series_of(s, i, b.freq_hz, &b.energy, arrival);
        let mut e = evaluated(&se, arrival, &model);
        e.set_edt(edt_report(s, &se, arrival, false));
        if contributing.len() > 1 {
            e.several_sources(&contributing);
        }
        let total_pa2: f64 = b.energy.iter().sum();
        let g_db = strength(&e.parameters.spl_db, b.source_power_rho_c);
        // The bin of the direct sound's leading edge (nothing reaches the receiver before it), or
        // the onset bin when the arrival is not known.
        let from = match arrival {
            Arrival::Known {
                time_s,
                half_width_s,
            } => ((time_s - half_width_s) / s.time_step_s).floor().max(0.0) as usize,
            Arrival::Detected => e.onset.map_or(0, |o| o.index),
        };
        let unseen = sti_unseen_share(s, i, b.freq_hz, &b.energy, from);
        sti_inputs.push(sti::ReceiverBand {
            freq_hz: b.freq_hz,
            energy: &b.energy,
            from,
            spl_db: value_or_why(&e.parameters.spl_db),
            power_rho_c: b.source_power_rho_c,
            // A receiver with noise in some bands is written 0 dB in the others: no noise there
            // (backlog 67).
            noise_db: (b.background_noise_db != 0.0).then_some(b.background_noise_db),
            reverberation_s: [
                &e.parameters.t30_s,
                &e.parameters.t20_s,
                &e.parameters.edt_s,
            ]
            .into_iter()
            .find_map(Evaluated::value),
            unusable: match (&se, unseen) {
                (Err(err), _) => Some(format!("its series is refused: {err}")),
                (Ok(_), None) if !s.band_complete(b.freq_hz) => Some(
                    "its series is not complete: particles were still alive when the run \
                     ended, and nothing bounds what they would still bring"
                        .into(),
                ),
                (Ok(_), None) => Some(
                    "nothing bounds the energy its series lacks (dropped at the solver's floor, \
                     or lost)"
                        .into(),
                ),
                (Ok(_), Some(_)) => None,
            },
            unseen_share: unseen.unwrap_or(0.0),
        });
        bands.push(ReceiverBandReport {
            freq_hz: b.freq_hz,
            complete: s.band_complete(b.freq_hz),
            floor_db: s.floor_db(),
            lost_share: se.as_ref().ok().and_then(EnergySeries::lost_share),
            lost_follows_decay: se
                .as_ref()
                .ok()
                .is_some_and(EnergySeries::lost_follows_decay),
            early_reverberation_unresolved: se
                .as_ref()
                .ok()
                .is_some_and(EnergySeries::early_reverberation_unresolved),
            arrival,
            decay_arrival: e.decay_arrival,
            contributing_sources: contributing.iter().map(|c| c.to_string()).collect(),
            crossings: crossings(&model, total_pa2),
            crossings_per_particle: e.crossings_per_particle,
            noise_model: model.clone(),
            energy_pa2: b.energy.clone(),
            total_pa2,
            source_power_rho_c: b.source_power_rho_c,
            background_noise_db: b.background_noise_db,
            onset: e.onset,
            parameters: e.parameters,
            g_db,
            curvature: e.curvature,
            decay_curve: e.decay_curve,
        });
        series.push(se);
        models.push(model);
    }
    let dba = dba_of(bands.iter().map(|b| (b.freq_hz, &b.parameters)));
    let sti = sti_report(s, &sti_inputs, octave, &all_contributing);
    let aggregate = aggregate_report(
        s,
        bands_hz,
        &series,
        &models,
        arrival,
        &all_contributing,
        dba,
    );
    let per_source = r
        .echograms
        .iter()
        .map(|e| {
            let name = [e.source.as_str()];
            let arrival_s = s.arrival_from(r, &name);
            let arrival = known_arrival(s, arrival_s);
            let series: Vec<Result<EnergySeries, ParamError>> = r
                .bands
                .iter()
                .zip(&e.energy)
                .enumerate()
                .map(|(i, (b, energy))| series_of(s, i, b.freq_hz, energy, arrival))
                .collect();
            let models: Vec<NoiseModel> = (0..r.bands.len())
                .map(|i| s.noise_model(i, &name))
                .collect();
            let bands: Vec<SourceBandReport> = r
                .bands
                .iter()
                .zip(&e.energy)
                .zip(&series)
                .zip(&models)
                .enumerate()
                .map(|(i, (((b, energy), se), model))| {
                    let mut e = evaluated(se, arrival, model);
                    e.set_edt(edt_report(s, se, arrival, false));
                    let total_pa2: f64 = energy.iter().sum();
                    let g_db = strength(
                        &e.parameters.spl_db,
                        source_power_rho_c(s, i, b.source_power_rho_c, name[0]),
                    );
                    SourceBandReport {
                        freq_hz: b.freq_hz,
                        arrival,
                        decay_arrival: e.decay_arrival,
                        noise_model: model.clone(),
                        crossings: crossings(model, total_pa2),
                        crossings_per_particle: e.crossings_per_particle,
                        energy_pa2: energy.clone(),
                        total_pa2,
                        onset: e.onset,
                        parameters: e.parameters,
                        g_db,
                        curvature: e.curvature,
                        decay_curve: e.decay_curve,
                    }
                })
                .collect();
            let dba = dba_of(bands.iter().map(|b| (b.freq_hz, &b.parameters)));
            SourceReceiverReport {
                source: e.source.clone(),
                file: e.file.clone(),
                arrival_s,
                bands,
                aggregate: aggregate_report(s, bands_hz, &series, &models, arrival, &name, dba),
            }
        })
        .collect();
    SppsReceiverReport {
        label: r.label.clone(),
        folder: r.folder.clone(),
        position_m: r.position_m,
        arrival_s,
        bands,
        aggregate,
        sti,
        by_source: r.by_source.clone(),
        per_source,
    }
}

/// `octave`: whether the run's bands are octave bands (STI is defined on octaves).
fn spps_report(bands_hz: &[i32], s: &SppsResults, octave: bool) -> SppsReport {
    SppsReport {
        time_step_s: s.time_step_s,
        duration_s: s.duration_s,
        steps: s.steps,
        speed_of_sound_m_s: s.speed_of_sound_m_s,
        receiver_radius_m: s.receiver_radius_m,
        receiver_crossing_s: s.receiver_crossing_s(),
        celerity_gradient: s.celerity_gradient,
        computation_method: s.computation_method,
        particles_per_source: s.particles_per_source,
        trans_epsilon: s.trans_epsilon,
        echogram_per_source: s.echogram_per_source,
        monte_carlo: MonteCarloReport::current(s.noise_method()),
        sources: s.sources.clone(),
        particles: s.particles.clone(),
        total_energy: s.total_energy.clone(),
        point_receivers: s
            .point_receivers
            .iter()
            .map(|r| receiver_report(bands_hz, s, r, octave))
            .collect(),
        surfaces: s.surfaces.iter().map(SurfaceSummary::of).collect(),
        particle_files: s.particle_files.clone(),
        reference: ReferenceReport::of(&s.reference),
    }
}

fn tcr_report(t: &TcrResults) -> TcrReport {
    TcrReport {
        bands: t.bands.clone(),
        global: TcrGlobal {
            aggregate: AGGREGATE_ENERGETIC_SUM.into(),
            sabine_level_db: t.global_sabine_level_db,
            eyring_level_db: t.global_eyring_level_db,
        },
        point_receivers: t
            .point_receivers
            .iter()
            .map(TcrReceiverReport::of)
            .collect(),
        surfaces: t.surfaces.iter().map(SurfaceSummary::of).collect(),
        analytic: match &t.analytic {
            tcr::Analytic::Computed {
                volume_m3,
                area_m2,
                bands,
            } => AnalyticReport::Computed {
                volume_m3: *volume_m3,
                area_m2: *area_m2,
                bands: bands
                    .iter()
                    .map(|b| AnalyticBandReport {
                        freq_hz: b.freq_hz,
                        air_m_per_metre: b.air_m_per_metre,
                        sabine_s: Evaluated::of(b.sabine_s.clone()),
                        eyring_s: Evaluated::of(b.eyring_s.clone()),
                    })
                    .collect(),
            },
            tcr::Analytic::NotComputed { why } => AnalyticReport::NotComputed { why: why.clone() },
        },
    }
}

/// The report of a verified run. [`checked_report`] also refuses one holding a number that is
/// not finite.
pub fn report(r: &RunResults) -> Report {
    let (spps, tcr) = match &r.data {
        SolverResults::Spps(s) => {
            // Octave bands when every `freq_enum` item is an octave nominal, as
            // `config_xml::import` reads the band kind.
            let octave = r.expectation.bands.iter().all(|b| {
                u32::try_from(b.freq_hz).is_ok_and(|f| {
                    crate::schema::BandKind::Octave
                        .nominal_frequencies()
                        .contains(&f)
                })
            });
            (Some(spps_report(&r.bands_hz, s, octave)), None)
        }
        SolverResults::Tcr(t) => (None, Some(tcr_report(t))),
    };
    Report {
        results_version: REPORT_VERSION,
        validated_by_bed: false,
        run_folder: r.folder.display().to_string(),
        solver: r.manifest.solver,
        status: r.manifest.verdict.status,
        solver_build: super::solver_build(&r.manifest),
        started: r.manifest.started.clone(),
        bands_hz: r.bands_hz.clone(),
        spps,
        tcr,
    }
}

/// [`report`], refused, `results_value_invalid`, when any number in it is NaN or infinite.
/// serde_json prints such a number as `null`, which a reader cannot tell from a value that is
/// absent, so the report is walked before it is printed ([`non_finite`]).
pub fn checked_report(r: &RunResults) -> Result<Report, Refusal> {
    let rep = report(r);
    let bad = non_finite(&rep);
    if bad.is_empty() {
        Ok(rep)
    } else {
        Err(value_invalid(
            "the report",
            format!(
                "{} numbers are not finite, first {}",
                bad.len(),
                bad.first().map_or("", String::as_str)
            ),
        ))
    }
}

/// The JSON Schema of [`Report`], committed as `docs/formats/results-json.schema.json`.
pub fn report_schema() -> schemars::Schema {
    schemars::schema_for!(Report)
}

/// The JSON Schema of the refusal `simpa results --json` prints on exit 5 or 6.
pub fn refusal_schema() -> schemars::Schema {
    schemars::schema_for!(RefusalReport)
}

/// What `simpa results <run> --json` prints when it refuses a run.
#[derive(Clone, Debug, PartialEq, Serialize, JsonSchema)]
pub struct RefusalReport {
    /// [`REPORT_VERSION`].
    pub results_version: u32,
    pub run_folder: String,
    pub refused: super::Refusal,
    /// The CLI's exit code: 5 for a failed or cancelled run, 6 for results that do not verify.
    pub exit_code: u8,
}

impl RefusalReport {
    pub fn new(folder: &std::path::Path, refused: super::Refusal) -> Self {
        RefusalReport {
            results_version: REPORT_VERSION,
            run_folder: folder.display().to_string(),
            exit_code: refused.exit_code(),
            refused,
        }
    }
}

// --- finiteness -------------------------------------------------------------------------------

/// The path of every `f32` or `f64` in `value` that is NaN or infinite, as `.field[index]`.
pub fn non_finite(value: &impl Serialize) -> Vec<String> {
    let mut f = Finder::default();
    // The walker never fails: every method returns Ok.
    let _ = value.serialize(&mut f);
    f.found
}

#[derive(Default)]
struct Finder {
    path: Vec<String>,
    /// One counter per open sequence.
    counters: Vec<usize>,
    found: Vec<String>,
}

impl Finder {
    fn check(&mut self, v: f64) -> Result<(), Never> {
        if !v.is_finite() {
            self.found.push(self.path.concat());
        }
        Ok(())
    }

    fn element<T: ?Sized + Serialize>(&mut self, v: &T) -> Result<(), Never> {
        let i = self.counters.last_mut().map_or(0, |c| {
            *c += 1;
            *c - 1
        });
        self.path.push(format!("[{i}]"));
        v.serialize(&mut *self)?;
        self.path.pop();
        Ok(())
    }

    fn field<T: ?Sized + Serialize>(&mut self, key: &str, v: &T) -> Result<(), Never> {
        self.path.push(format!(".{key}"));
        v.serialize(&mut *self)?;
        self.path.pop();
        Ok(())
    }
}

#[derive(Debug)]
struct Never;

impl std::fmt::Display for Never {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "never")
    }
}

impl std::error::Error for Never {}

impl serde::ser::Error for Never {
    fn custom<T: std::fmt::Display>(_: T) -> Self {
        Never
    }
}

macro_rules! ignore {
    ($($name:ident: $t:ty),*) => {
        $(fn $name(self, _: $t) -> Result<(), Never> { Ok(()) })*
    };
}

impl serde::Serializer for &mut Finder {
    type Ok = ();
    type Error = Never;
    type SerializeSeq = Self;
    type SerializeTuple = Self;
    type SerializeTupleStruct = Self;
    type SerializeTupleVariant = Self;
    type SerializeMap = Self;
    type SerializeStruct = Self;
    type SerializeStructVariant = Self;

    ignore!(serialize_bool: bool, serialize_i8: i8, serialize_i16: i16, serialize_i32: i32,
        serialize_i64: i64, serialize_u8: u8, serialize_u16: u16, serialize_u32: u32,
        serialize_u64: u64, serialize_char: char, serialize_str: &str, serialize_bytes: &[u8],
        serialize_unit_struct: &'static str);

    fn serialize_f32(self, v: f32) -> Result<(), Never> {
        self.check(f64::from(v))
    }
    fn serialize_f64(self, v: f64) -> Result<(), Never> {
        self.check(v)
    }
    fn serialize_none(self) -> Result<(), Never> {
        Ok(())
    }
    fn serialize_some<T: ?Sized + Serialize>(self, v: &T) -> Result<(), Never> {
        v.serialize(self)
    }
    fn serialize_unit(self) -> Result<(), Never> {
        Ok(())
    }
    fn serialize_unit_variant(self, _: &'static str, _: u32, _: &'static str) -> Result<(), Never> {
        Ok(())
    }
    fn serialize_newtype_struct<T: ?Sized + Serialize>(
        self,
        _: &'static str,
        v: &T,
    ) -> Result<(), Never> {
        v.serialize(self)
    }
    fn serialize_newtype_variant<T: ?Sized + Serialize>(
        self,
        _: &'static str,
        _: u32,
        variant: &'static str,
        v: &T,
    ) -> Result<(), Never> {
        self.field(variant, v)
    }
    fn serialize_seq(self, _: Option<usize>) -> Result<Self, Never> {
        self.counters.push(0);
        Ok(self)
    }
    fn serialize_tuple(self, _: usize) -> Result<Self, Never> {
        self.counters.push(0);
        Ok(self)
    }
    fn serialize_tuple_struct(self, _: &'static str, _: usize) -> Result<Self, Never> {
        self.counters.push(0);
        Ok(self)
    }
    fn serialize_tuple_variant(
        self,
        _: &'static str,
        _: u32,
        variant: &'static str,
        _: usize,
    ) -> Result<Self, Never> {
        self.path.push(format!(".{variant}"));
        self.counters.push(0);
        Ok(self)
    }
    fn serialize_map(self, _: Option<usize>) -> Result<Self, Never> {
        Ok(self)
    }
    fn serialize_struct(self, _: &'static str, _: usize) -> Result<Self, Never> {
        Ok(self)
    }
    fn serialize_struct_variant(
        self,
        _: &'static str,
        _: u32,
        variant: &'static str,
        _: usize,
    ) -> Result<Self, Never> {
        self.path.push(format!(".{variant}"));
        Ok(self)
    }
}

impl serde::ser::SerializeSeq for &mut Finder {
    type Ok = ();
    type Error = Never;
    fn serialize_element<T: ?Sized + Serialize>(&mut self, v: &T) -> Result<(), Never> {
        self.element(v)
    }
    fn end(self) -> Result<(), Never> {
        self.counters.pop();
        Ok(())
    }
}

impl serde::ser::SerializeTuple for &mut Finder {
    type Ok = ();
    type Error = Never;
    fn serialize_element<T: ?Sized + Serialize>(&mut self, v: &T) -> Result<(), Never> {
        self.element(v)
    }
    fn end(self) -> Result<(), Never> {
        self.counters.pop();
        Ok(())
    }
}

impl serde::ser::SerializeTupleStruct for &mut Finder {
    type Ok = ();
    type Error = Never;
    fn serialize_field<T: ?Sized + Serialize>(&mut self, v: &T) -> Result<(), Never> {
        self.element(v)
    }
    fn end(self) -> Result<(), Never> {
        self.counters.pop();
        Ok(())
    }
}

impl serde::ser::SerializeTupleVariant for &mut Finder {
    type Ok = ();
    type Error = Never;
    fn serialize_field<T: ?Sized + Serialize>(&mut self, v: &T) -> Result<(), Never> {
        self.element(v)
    }
    fn end(self) -> Result<(), Never> {
        self.counters.pop();
        self.path.pop();
        Ok(())
    }
}

impl serde::ser::SerializeMap for &mut Finder {
    type Ok = ();
    type Error = Never;
    fn serialize_key<T: ?Sized + Serialize>(&mut self, k: &T) -> Result<(), Never> {
        self.field("{key}", k)
    }
    fn serialize_value<T: ?Sized + Serialize>(&mut self, v: &T) -> Result<(), Never> {
        self.field("{value}", v)
    }
    fn end(self) -> Result<(), Never> {
        Ok(())
    }
}

impl serde::ser::SerializeStruct for &mut Finder {
    type Ok = ();
    type Error = Never;
    fn serialize_field<T: ?Sized + Serialize>(
        &mut self,
        key: &'static str,
        v: &T,
    ) -> Result<(), Never> {
        self.field(key, v)
    }
    fn end(self) -> Result<(), Never> {
        Ok(())
    }
}

impl serde::ser::SerializeStructVariant for &mut Finder {
    type Ok = ();
    type Error = Never;
    fn serialize_field<T: ?Sized + Serialize>(
        &mut self,
        key: &'static str,
        v: &T,
    ) -> Result<(), Never> {
        self.field(key, v)
    }
    fn end(self) -> Result<(), Never> {
        self.path.pop();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    #[derive(Serialize)]
    struct Inner {
        a: f64,
        b: Option<f32>,
    }

    #[derive(Serialize)]
    enum Kind {
        One(f64),
        Two { x: Vec<f64> },
    }

    #[derive(Serialize)]
    struct Outer {
        inner: Vec<Inner>,
        kinds: Vec<Kind>,
        pair: (f64, f64),
        text: String,
    }

    #[test]
    fn the_walker_finds_every_number_that_is_not_finite_and_nothing_else() {
        let clean = Outer {
            inner: vec![Inner { a: 1.0, b: None }],
            kinds: vec![Kind::One(2.0), Kind::Two { x: vec![3.0] }],
            pair: (4.0, 5.0),
            text: "NaN".into(),
        };
        assert!(non_finite(&clean).is_empty());
        let spoiled = Outer {
            inner: vec![
                Inner { a: 1.0, b: None },
                Inner {
                    a: f64::NAN,
                    b: Some(f32::INFINITY),
                },
            ],
            kinds: vec![
                Kind::One(f64::NEG_INFINITY),
                Kind::Two {
                    x: vec![0.0, f64::NAN],
                },
            ],
            pair: (4.0, f64::NAN),
            text: String::new(),
        };
        assert_eq!(
            non_finite(&spoiled),
            [
                ".inner[1].a",
                ".inner[1].b",
                ".kinds[0].One",
                ".kinds[1].Two.x[1]",
                ".pair[1]"
            ]
        );
        // serde_json prints each of the five as null, the same null as the one absent `b`.
        let text = serde_json::to_string(&spoiled).unwrap();
        assert_eq!(text.matches("null").count(), 6, "{text}");
    }

    #[test]
    fn a_value_carries_its_noise_and_a_refusal_its_reason() {
        let v = Evaluated::of_estimate(Ok(noise::Estimate {
            value: 1.5,
            sd: 0.01,
        }));
        assert_eq!(
            serde_json::to_value(&v).unwrap(),
            serde_json::json!({"value": 1.5, "mc_sd": 0.01})
        );
        let a = Evaluated::of(Ok(0.6));
        assert_eq!(
            serde_json::to_value(&a).unwrap(),
            serde_json::json!({"value": 0.6, "mc_sd": null})
        );
        let r = Evaluated::of(Err(ParamError::NoEnergy));
        let j = serde_json::to_value(&r).unwrap();
        assert_eq!(j["not_evaluable"]["code"], "params_no_energy");
        assert_eq!(r.value(), None);
        assert_eq!(r.refusal().unwrap().code, crate::params::codes::NO_ENERGY);
    }

    #[test]
    fn the_curvature_flag_follows_its_value_and_is_null_when_refused() {
        let at = |value: f64| CurvatureReport::of(Ok(noise::Estimate { value, sd: 0.5 }));
        assert_eq!(at(12.0).curved, Some(true));
        assert_eq!(at(-10.5).curved, Some(true));
        assert_eq!(at(10.0).curved, Some(false));
        assert_eq!(at(-3.0).curved, Some(false));
        assert_eq!(at(4.0).percent.value(), Some(4.0));
        let refused = CurvatureReport::of(Err(ParamError::NoEnergy));
        assert_eq!(refused.curved, None);
        assert_eq!(refused.limit_percent, 10.0);
        assert_eq!(
            refused.percent.refusal().unwrap().code,
            crate::params::codes::NO_ENERGY
        );
        // Several sources withhold the curve with the curvature.
        let s = EnergySeries::new(0.01, (0..100).map(|k| 0.9f64.powi(k)).collect()).unwrap();
        let model = NoiseModel::crossings(1e-6, noise::Method::Random, None).unwrap();
        let mut e = evaluated(&Ok(s), Arrival::at(0.0), &model);
        assert!(e.decay_curve.is_some());
        e.several_sources(&["A", "B"]);
        assert!(e.decay_curve.is_none());
        assert!(matches!(
            e.curvature.percent.refusal().unwrap().error.not_evaluable(),
            Some(NotEvaluable::SeveralSources { .. })
        ));
    }

    #[test]
    fn several_sources_refuse_the_seven_onset_relative_quantities_and_keep_spl() {
        let v = || Evaluated::of(Ok(1.0));
        let mut p = Parameters {
            spl_db: v(),
            edt_s: v(),
            t20_s: v(),
            t30_s: v(),
            c50_db: v(),
            c80_db: v(),
            d50: v(),
            ts_s: v(),
            edt: None,
            edt_validated: false,
        };
        p.several_sources(&["A", "B"]);
        assert_eq!(p.spl_db.value(), Some(1.0));
        for (name, e) in p.named().into_iter().skip(1) {
            let r = e.refusal().unwrap_or_else(|| panic!("{name}"));
            assert!(
                matches!(
                    r.error.not_evaluable(),
                    Some(NotEvaluable::SeveralSources { sources }) if sources == &["A", "B"]
                ),
                "{name}: {}",
                r.message
            );
        }
    }

    #[test]
    fn a_refused_transport_leaves_free_paths_null_and_refuses_kuttruff_only() {
        use crate::results::reference::ReferenceBand;
        let refusal = ParamError::TransportRefused {
            detail: "a ray left the enclosure".into(),
        };
        let r = Reference::Computed {
            volume_m3: 180.0,
            area_m2: 216.0,
            speed_of_sound_m_s: 343.2,
            constant_s_per_m: 0.161,
            free_paths: Err(refusal.clone()),
            bands: vec![ReferenceBand {
                freq_hz: 500,
                air_m_per_metre: None,
                mean_absorption: 0.2,
                lambert_walls: true,
                uniform_absorption: true,
                eyring_s: Ok(0.6),
                kuttruff_s: Err(refusal),
            }],
        };
        let ReferenceReport::Computed {
            label,
            free_paths,
            bands,
            ..
        } = ReferenceReport::of(&r)
        else {
            panic!("computed");
        };
        assert_eq!(label, REFERENCE_LABEL);
        assert!(free_paths.is_none());
        assert_eq!(bands[0].eyring_s.value(), Some(0.6));
        assert_eq!(
            bands[0].kuttruff_s.refusal().unwrap().code,
            crate::params::codes::TRANSPORT_REFUSED
        );
        // A value carries the standard deviation it inherits from γ².
        let Reference::Computed { mut bands, .. } = r else {
            unreachable!()
        };
        bands[0].kuttruff_s = Ok((0.62, 5e-5));
        let r = Reference::Computed {
            volume_m3: 180.0,
            area_m2: 216.0,
            speed_of_sound_m_s: 343.2,
            constant_s_per_m: 0.161,
            free_paths: Err(ParamError::NoAbsorption),
            bands,
        };
        let ReferenceReport::Computed { bands, .. } = ReferenceReport::of(&r) else {
            panic!("computed");
        };
        assert_eq!(bands[0].kuttruff_s, Evaluated::bare(0.62, Some(5e-5)));
    }

    // --- EDT v2.1 (M8b port; docs/investigations/2026-10-02-edt-port/PORT.md) --------------------

    use crate::results::spps::{ReceiverBand, SourceTotals, emission_s};
    use crate::run::stats::BandStats;

    const DT: f32 = 0.001;
    const C: f64 = 343.2;
    const RADIUS_M: f64 = 0.31;
    const DISTANCE_M: f64 = 3.432; // 10 ms from the source

    /// A decay of `t60` s from `arrival_s`, the direct sound in the bin it falls in, with a
    /// deterministic ragged scatter standing in for the hits' noise.
    fn decay(n: usize, arrival_s: f64, t60: f64) -> Vec<f64> {
        let dt = f64::from(DT);
        let mut seed = 12345u64;
        (0..n)
            .map(|k| {
                seed = seed
                    .wrapping_mul(6364136223846793005)
                    .wrapping_add(1442695040888963407);
                let jitter = 0.9 + 0.2 * ((seed >> 33) as f64 / f64::from(1u32 << 31));
                let t = (k as f64 + 0.5) * dt;
                // The direct sound crosses the ball over [arrival - h, arrival + h].
                let h = RADIUS_M / C;
                let lo = (k as f64 * dt).max(arrival_s - h);
                let hi = ((k as f64 + 1.0) * dt).min(arrival_s + h);
                let direct = (hi - lo).max(0.0) / (2.0 * h);
                if t < arrival_s - h {
                    0.0
                } else {
                    direct
                        + if t >= arrival_s {
                            1e-2 * jitter * 10f64.powf(-6.0 * (t - arrival_s) / t60)
                        } else {
                            0.0
                        }
                }
            })
            .collect()
    }

    /// One source (delayed by `delay_s`) and one receiver, one band, `bins` as the receiver's
    /// series.
    fn edt_run(method: i32, delay_s: f32, bins: Vec<f64>) -> SppsResults {
        let n = bins.len();
        SppsResults {
            time_step_s: f64::from(DT),
            duration_s: n as f64 * f64::from(DT),
            steps: n,
            speed_of_sound_m_s: C,
            receiver_radius_m: RADIUS_M,
            celerity_gradient: false,
            computation_method: method,
            particles_per_source: 100_000,
            trans_epsilon: 5.0,
            echogram_per_source: false,
            sources: vec![SourcePoint {
                name: "S".into(),
                position_m: Some([0.0, 0.0, 0.0]),
                emission_s: emission_s(delay_s, DT),
                band_power_w: vec![1.0],
                balloon: false,
            }],
            point_receivers: vec![PointReceiver {
                label: "R".into(),
                folder: "Punctual receivers/R".into(),
                position_m: Some([DISTANCE_M, 0.0, 0.0]),
                bands: vec![ReceiverBand {
                    freq_hz: 500,
                    energy: bins.clone(),
                    lateral_cos2: Ok(vec![0.0; n]),
                    lateral_abs_cos: Ok(vec![0.0; n]),
                    intensity: Default::default(),
                    source_power_rho_c: 2.0,
                    background_noise_db: 0.0,
                }],
                by_source: vec![SourceTotals {
                    source: "S".into(),
                    energy: vec![bins.iter().sum()],
                }],
                echograms: Vec::new(),
            }],
            total_energy: Vec::new(),
            particles: ParticleStats {
                bands: vec![BandStats {
                    freq_hz: 500,
                    absorbed_by_atmosphere: 0,
                    absorbed_by_materials: 100_000,
                    absorbed_by_fittings: 0,
                    lost_by_infinite_loops: 0,
                    lost_by_meshing_problems: 0,
                    remaining: 0,
                    total: 100_000,
                }],
            },
            surfaces: Vec::new(),
            particle_files: Vec::new(),
            reference: Box::new(Reference::NotComputed {
                why: "a unit test's run".into(),
            }),
        }
    }

    fn band_edt(s: &SppsResults) -> EdtReport {
        let rep = receiver_report(&[500], s, &s.point_receivers[0], true);
        rep.bands[0].parameters.edt.clone().expect("an EDT report")
    }

    #[test]
    fn the_method_gets_the_arrival_with_the_source_delay_rounded_up_to_the_next_whole_step() {
        let h = RADIUS_M / C;
        let direct_s = DISTANCE_M / C;
        // 15.3 steps of delay: the particles start at step 16.
        let delay_s = 0.0153f32;
        let arrival_s = emission_s(delay_s, DT) + direct_s;
        assert!((arrival_s - (0.016 + direct_s)).abs() < 1e-6);
        let bins = decay(1500, arrival_s, 0.6);
        let s = edt_run(0, delay_s, bins.clone());
        let r = receiver_report(&[500], &s, &s.point_receivers[0], true);
        assert_eq!(r.arrival_s, Some(arrival_s));
        let got = r.bands[0].parameters.edt.clone().unwrap();
        assert_eq!(got.arrival_s, Some(arrival_s));

        let with_delay = edt::analyse(&bins, f64::from(DT), Some(arrival_s), Some(h));
        let delay_dropped = edt::analyse(&bins, f64::from(DT), Some(direct_s), Some(h));
        // The test has power only where dropping the delay changes the answer.
        assert_ne!(
            with_delay, delay_dropped,
            "this series cannot tell a dropped delay"
        );
        // Rounded up, not the delay as written (0.7 ms apart: the arrival itself says so).
        assert_ne!(r.arrival_s, Some(f64::from(delay_s) + direct_s));
        assert_eq!(got.value_s, with_delay.edt);
        assert_eq!(got.lo_s, with_delay.edt_lo);
        assert_eq!(got.hi_s, with_delay.edt_hi);
        assert_eq!(got.reason, with_delay.reason);
        assert_ne!(got.reason, delay_dropped.reason);
    }

    #[test]
    fn edt_s_is_the_methods_value_or_its_refusal_as_edt_refused() {
        let arrival_s = emission_s(0.0, DT) + DISTANCE_M / C;
        // A decay the run covers: a value, in `edt_s` and in `edt`.
        let s = edt_run(0, 0.0, decay(1500, arrival_s, 0.6));
        let rep = receiver_report(&[500], &s, &s.point_receivers[0], true);
        let p = &rep.bands[0].parameters;
        let e = p.edt.as_ref().unwrap();
        assert_ne!(e.status, edt::Status::Refused, "{e:?}");
        assert_eq!(p.edt_s.value(), e.value_s);
        assert!(e.lo_s.unwrap() < e.value_s.unwrap() && e.value_s.unwrap() < e.hi_s.unwrap());
        // A run that ends before the decay does: refused, with the method's reason.
        let s = edt_run(0, 0.0, decay(60, arrival_s, 0.6));
        let rep = receiver_report(&[500], &s, &s.point_receivers[0], true);
        let p = &rep.bands[0].parameters;
        let e = p.edt.as_ref().unwrap();
        assert_eq!(e.status, edt::Status::Refused);
        assert_eq!(e.value_s, None);
        let why = p
            .edt_s
            .refusal()
            .expect("refused")
            .error
            .not_evaluable()
            .cloned();
        assert_eq!(
            why,
            Some(NotEvaluable::EdtRefused {
                reason: e.reason.clone()
            })
        );
    }

    /// A receiver the direct sound never reaches: nothing until `onset_s` after the geometric
    /// arrival `arrival_s`, then the reverberant decay of `t60` s, with [`decay`]'s scatter.
    fn blocked(n: usize, arrival_s: f64, onset_s: f64, t60: f64) -> Vec<f64> {
        let first = arrival_s + onset_s;
        let mut seed = 12345u64;
        (0..n)
            .map(|k| {
                seed = seed
                    .wrapping_mul(6364136223846793005)
                    .wrapping_add(1442695040888963407);
                let jitter = 0.9 + 0.2 * ((seed >> 33) as f64 / f64::from(1u32 << 31));
                let t = (k as f64 + 0.5) * f64::from(DT);
                if t < first {
                    0.0
                } else {
                    1e-2 * jitter * 10f64.powf(-6.0 * (t - first) / t60)
                }
            })
            .collect()
    }

    /// Decision-log row 38: energetic mode's one held-out failure was G4's blocked far receiver,
    /// so the mode itself no longer marks EDT; a single band with line of sight is validated in
    /// either mode, with the same value and range.
    #[test]
    fn a_single_band_with_line_of_sight_is_validated_in_either_mode() {
        let bins = decay(1500, DISTANCE_M / C, 0.6);
        let random = band_edt(&edt_run(0, 0.0, bins.clone()));
        let energetic = band_edt(&edt_run(1, 0.0, bins.clone()));
        for (name, e) in [("random", &random), ("energetic", &energetic)] {
            assert_ne!(e.status, edt::Status::Refused, "{name}: {e:?}");
            assert!(e.validated, "{name}: {e:?}");
            assert_eq!(e.validation_note, None, "{name}");
        }
        assert_eq!(random.value_s, energetic.value_s);
        assert_eq!((random.lo_s, random.hi_s), (energetic.lo_s, energetic.hi_s));
        let s = edt_run(1, 0.0, bins);
        let rep = receiver_report(&[500], &s, &s.point_receivers[0], true);
        assert!(rep.bands[0].parameters.edt_validated);
        let json = serde_json::to_value(&rep).unwrap();
        assert_eq!(json["bands"][0]["parameters"]["edt"]["validated"], true);
        assert!(json["bands"][0]["parameters"]["edt"]["validation_note"].is_null());
    }

    /// Decision-log row 38: with no direct sound the method puts 0 dB at the run's first recorded
    /// energy, which read EDT 5-9 % low at G4 R007 in both modes, so such a receiver's EDT is
    /// not validated in either mode. The value is still the method's, shown with its range.
    #[test]
    fn a_receiver_the_direct_sound_never_reached_is_not_validated_in_either_mode() {
        let arrival_s = DISTANCE_M / C;
        let h = RADIUS_M / C;
        let bins = blocked(1500, arrival_s, 0.010, 0.6);
        // The series is what the branch reads: nothing from the ball's front to one step past
        // its back, energy after.
        let o = edt::analyse(&bins, f64::from(DT), Some(arrival_s), Some(h));
        assert!(o.no_direct_path, "{o:?}");
        assert_ne!(o.status, edt::Status::Refused, "{o:?}");
        let seen = edt::analyse(
            &decay(1500, arrival_s, 0.6),
            f64::from(DT),
            Some(arrival_s),
            Some(h),
        );
        assert!(!seen.no_direct_path, "line of sight: {seen:?}");
        for method in [0, 1] {
            let s = edt_run(method, 0.0, bins.clone());
            let e = band_edt(&s);
            assert!(!e.validated, "mode {method}: {e:?}");
            assert_eq!(
                e.validation_note.as_deref(),
                Some(EDT_NO_DIRECT_PATH_NOT_VALIDATED),
                "mode {method}"
            );
            assert_eq!(e.value_s, o.edt, "mode {method}: the method's value");
            let rep = receiver_report(&[500], &s, &s.point_receivers[0], true);
            assert!(!rep.bands[0].parameters.edt_validated, "mode {method}");
            // The aggregate names both of its reasons.
            let agg = rep
                .aggregate
                .parameters
                .edt
                .as_ref()
                .expect("aggregate EDT");
            let note = agg.validation_note.as_deref().expect("a note");
            assert!(note.contains(EDT_BROADBAND_NOT_VALIDATED), "{note}");
            assert!(note.contains(EDT_NO_DIRECT_PATH_NOT_VALIDATED), "{note}");
            let json = serde_json::to_value(&rep).unwrap();
            assert_eq!(
                json["bands"][0]["parameters"]["edt"]["validation_note"],
                EDT_NO_DIRECT_PATH_NOT_VALIDATED
            );
        }
        // No arrival to measure from (a celerity gradient): the method anchors at the first
        // energy by design, and its blocked-path branch is not what ran.
        let none = edt::analyse(&bins, f64::from(DT), None, Some(h));
        assert!(!none.no_direct_path, "{none:?}");
    }
    /// Decision 37 (1): a receiver over 1 m is outside what the held-out test passed (Synth
    /// spheres over 1 m read 5-7 % low, VERDICT-2 ruling 1), so its EDT is not validated in any
    /// mode, and a run with several reasons names each of them.
    #[test]
    fn a_receiver_over_one_metre_marks_edt_not_validated() {
        let bins = decay(1500, DISTANCE_M / C, 0.6);
        let at = |method: i32, radius_m: f64| {
            let mut s = edt_run(method, 0.0, bins.clone());
            s.receiver_radius_m = radius_m;
            band_edt(&s)
        };
        assert!(at(0, 1.0).validated, "1 m is inside");
        let big = at(0, 1.01);
        assert!(!big.validated, "over 1 m");
        assert_eq!(
            big.validation_note.as_deref(),
            Some(EDT_LARGE_RECEIVER_NOT_VALIDATED)
        );
        // Decision-log row 38: the mode is no longer a reason, so an energetic run's large
        // receiver carries the radius note alone.
        assert_eq!(
            at(1, 1.5).validation_note.as_deref(),
            Some(EDT_LARGE_RECEIVER_NOT_VALIDATED)
        );
        // Several reasons: each is named.
        let mut s = edt_run(1, 0.0, blocked(1500, DISTANCE_M / C, 0.010, 0.6));
        s.receiver_radius_m = 1.5;
        let both = band_edt(&s).validation_note.expect("a note");
        assert!(both.contains(EDT_NO_DIRECT_PATH_NOT_VALIDATED), "{both}");
        assert!(both.contains(EDT_LARGE_RECEIVER_NOT_VALIDATED), "{both}");
        // The marker beside edt_s follows it.
        let mut s = edt_run(0, 0.0, bins.clone());
        s.receiver_radius_m = 1.5;
        let rep = receiver_report(&[500], &s, &s.point_receivers[0], true);
        assert!(!rep.bands[0].parameters.edt_validated);
    }

    /// Finding 1 (assay): the summed-bands EDT was never in the held-out test, which read single
    /// bands, so it is "not yet validated" in every mode, Random included.
    #[test]
    fn the_aggregate_edt_is_not_validated_whatever_the_mode() {
        let arrival_s = DISTANCE_M / C;
        let bins = decay(1500, arrival_s, 0.6);
        for method in [0, 1] {
            let s = edt_run(method, 0.0, bins.clone());
            let rep = receiver_report(&[500], &s, &s.point_receivers[0], true);
            let agg = rep
                .aggregate
                .parameters
                .edt
                .as_ref()
                .expect("aggregate EDT");
            assert!(!agg.validated, "mode {method}");
            let note = agg.validation_note.as_deref().expect("a note");
            assert!(note.contains("broadband EDT is not covered by the held-out test"));
            // A single band keeps its own validation in either mode (decision-log row 38), and
            // says nothing of broadband.
            let band = rep.bands[0].parameters.edt.as_ref().unwrap();
            assert!(band.validated, "mode {method}");
            assert_eq!(band.validation_note, None, "mode {method}");
            let json = serde_json::to_value(&rep).unwrap();
            assert_eq!(json["aggregate"]["parameters"]["edt"]["validated"], false);
            for src in json["per_source"].as_array().into_iter().flatten() {
                assert_eq!(src["aggregate"]["parameters"]["edt"]["validated"], false);
            }
        }
    }

    /// Finding 2 (assay): `edt_s` is a bare number, so the marker rides beside it as
    /// `edt_validated`, on every Parameters that holds an EDT; true only where the held-out test
    /// covered it (a single band the direct sound reached, in either mode since decision-log
    /// row 38).
    #[test]
    fn edt_s_carries_its_marker_beside_it_on_every_surface() {
        let bins = decay(1500, DISTANCE_M / C, 0.6);
        let marker = |method: i32, at: &dyn Fn(&Value) -> Value| {
            let s = edt_run(method, 0.0, bins.clone());
            let rep = receiver_report(&[500], &s, &s.point_receivers[0], true);
            at(&serde_json::to_value(&rep).unwrap())
        };
        let band = |j: &Value| j["bands"][0]["parameters"]["edt_validated"].clone();
        let agg = |j: &Value| j["aggregate"]["parameters"]["edt_validated"].clone();
        assert_eq!(marker(0, &band), true, "Random band");
        assert_eq!(marker(1, &band), true, "Energetic band");
        assert_eq!(marker(0, &agg), false, "Random aggregate");
        assert_eq!(marker(1, &agg), false, "Energetic aggregate");
        // Never true without an EDT object: TCR's refusal and the several-sources refusal.
        let tcr = Parameters::no_time_series("x");
        assert!(!tcr.edt_validated);
        let mut p = tcr;
        p.several_sources(&["A", "B"]);
        assert!(!p.edt_validated);
    }

    // --- A noise-limited value shows its range (decision-log rows 37 (3), 39 (3)) ----------------

    use crate::params::noise::RangeStatus;

    /// A clean exponential decay, 0.46 dB per 10 ms bin over 1 s (T60 about 1.3 s): T20 and T30
    /// reach their ranges, and its noise is whatever the model's deposit makes it.
    fn exponential(bins: i32) -> EnergySeries {
        EnergySeries::new(0.01, (0..bins).map(|k| 0.9f64.powi(k)).collect()).unwrap()
    }

    fn random_model(deposit: f64) -> NoiseModel {
        NoiseModel::crossings(deposit, noise::Method::Random, None).unwrap()
    }

    /// The first deposit at which `noise::evaluate` refuses T20 for its standard deviation alone
    /// (what a 150 k random-mode run did to every T20 in RESULT-3B), with that refusal's value and
    /// standard deviation.
    fn noisy_t20() -> (f64, f64, f64) {
        for d in [1e-4, 2e-4, 5e-4, 1e-3, 2e-3, 5e-3] {
            let p = noise::evaluate(&Ok(exponential(100)), Arrival::at(0.0), &random_model(d));
            if let Err(e) = &p.t20_s
                && let Some(NotEvaluable::MonteCarloNoise {
                    value,
                    sd: Some(sd),
                    refused_resamples,
                    ..
                }) = e.not_evaluable()
                && *refused_resamples <= noise::REFUSED_RESAMPLES_ALLOWED
            {
                return (d, *value, *sd);
            }
        }
        panic!("no deposit tried refuses T20 for its standard deviation alone");
    }

    #[test]
    fn a_band_refused_for_its_noise_shows_its_value_and_range_marked_wide() {
        let (d, value, sd) = noisy_t20();
        let e = evaluated(&Ok(exponential(100)), Arrival::at(0.0), &random_model(d));
        let half = 2.5 * sd;
        assert_eq!(
            e.parameters.t20_s,
            Evaluated::Value {
                value,
                mc_sd: Some(sd),
                status: Some(RangeStatus::Wide),
                lo: Some(value - half),
                hi: Some(value + half),
                refused_resamples: None,
                straddle: None,
            },
            "deposit {d}"
        );
        // Wide: the range is past the 5 % limen.
        assert!(half / value > 0.05, "{half} / {value}");
        // The curvature still needs T20 and T30 as judged: with T20 wide it is refused, not
        // computed from a wide value.
        assert!(e.curvature.percent.refusal().is_some());
    }

    #[test]
    fn a_quiet_band_shows_every_value_ok_with_its_range() {
        // 200 bins, 92 dB: no unseen tail moves T30.
        let e = evaluated(&Ok(exponential(200)), Arrival::at(0.0), &random_model(1e-9));
        for (i, (name, q)) in e.parameters.named().into_iter().enumerate() {
            let Evaluated::Value {
                value,
                mc_sd: Some(sd),
                status,
                lo,
                hi,
                refused_resamples: None,
                straddle,
            } = q
            else {
                panic!("{name}: {q:?}")
            };
            // From an arrival at 0 every window edge is a bin edge: no bin straddles te.
            assert_eq!(*straddle, None, "{name}");
            assert_eq!(*status, Some(RangeStatus::Ok), "{name}: {q:?}");
            assert_eq!(*lo, Some(value - 2.5 * sd), "{name}");
            assert_eq!(*hi, Some(value + 2.5 * sd), "{name}");
            assert!(2.5 * sd <= noise::jnd(i) * if noise::relative(i) { value.abs() } else { 1.0 });
        }
    }

    #[test]
    fn a_refusal_not_about_noise_stays_a_refusal_however_noisy_the_band() {
        let (d, _, _) = noisy_t20();
        // 30 bins decay 13.7 dB: T20 needs 25.
        let e = evaluated(&Ok(exponential(30)), Arrival::at(0.0), &random_model(d));
        for q in [&e.parameters.t20_s, &e.parameters.t30_s] {
            let r = q.refusal().unwrap_or_else(|| panic!("{q:?}"));
            assert!(
                matches!(
                    r.error.not_evaluable(),
                    Some(NotEvaluable::RangeNotReached { .. })
                ),
                "{}",
                r.message
            );
            assert_eq!(q.status(), None);
        }
        // A series refused outright refuses all eight.
        let e = evaluated(
            &Err(ParamError::NoEnergy),
            Arrival::at(0.0),
            &random_model(d),
        );
        for (name, q) in e.parameters.named() {
            assert!(q.refusal().is_some(), "{name}");
        }
        // A model with no noise estimate refuses, noise_unknown: nothing bounds it.
        let unknown = NoiseModel::Unknown {
            detail: "a balloon".into(),
        };
        let e = evaluated(&Ok(exponential(100)), Arrival::at(0.0), &unknown);
        assert!(matches!(
            e.parameters.t20_s.refusal().unwrap().error.not_evaluable(),
            Some(NotEvaluable::NoiseUnknown { .. })
        ));
    }

    /// The JSON: `status`, `lo` and `hi` beside `value` and `mc_sd`; a consumer reading them can
    /// tell `ok` from `wide` and re-derive both from `value` and `mc_sd`; a value with no range
    /// prints as before.
    #[test]
    fn the_json_carries_status_and_range_beside_the_value() {
        let (d, value, sd) = noisy_t20();
        let e = evaluated(&Ok(exponential(100)), Arrival::at(0.0), &random_model(d));
        let j = serde_json::to_value(&e.parameters).unwrap();
        let t20 = &j["t20_s"];
        assert_eq!(t20["status"], "wide");
        assert_eq!(t20["value"].as_f64(), Some(value));
        assert_eq!(t20["mc_sd"].as_f64(), Some(sd));
        assert_eq!(t20["lo"].as_f64(), Some(value - 2.5 * sd));
        assert_eq!(t20["hi"].as_f64(), Some(value + 2.5 * sd));
        assert!(t20.get("not_evaluable").is_none());
        for (i, name) in noise::QUANTITY_NAMES.iter().enumerate().skip(2) {
            let q = &j[*name];
            let Some(v) = q["value"].as_f64() else {
                continue;
            };
            let s = q["mc_sd"].as_f64().unwrap();
            let unit = if noise::relative(i) { v.abs() } else { 1.0 };
            let want = if 2.5 * s <= noise::jnd(i) * unit {
                "ok"
            } else {
                "wide"
            };
            assert_eq!(q["status"], want, "{name}: {q}");
        }
        // A value with no range prints as it always did.
        assert_eq!(
            serde_json::to_value(Evaluated::bare(0.6, None)).unwrap(),
            serde_json::json!({"value": 0.6, "mc_sd": null})
        );
        // The schema names both statuses.
        let schema = serde_json::to_string(&report_schema()).unwrap();
        assert!(schema.contains("\"wide\""), "{schema}");
    }

    /// EDT's value carries the method's own status and range.
    #[test]
    fn edt_s_carries_the_methods_status_and_range() {
        let bins = decay(1500, DISTANCE_M / C, 0.6);
        let s = edt_run(0, 0.0, bins);
        let rep = receiver_report(&[500], &s, &s.point_receivers[0], true);
        let p = &rep.bands[0].parameters;
        let edt = p.edt.as_ref().unwrap();
        let Evaluated::Value { status, lo, hi, .. } = &p.edt_s else {
            panic!("{:?}", p.edt_s)
        };
        assert_eq!(*lo, edt.lo_s);
        assert_eq!(*hi, edt.hi_s);
        let want = match edt.status {
            edt::Status::Ok => RangeStatus::Ok,
            _ => RangeStatus::Wide,
        };
        assert_eq!(*status, Some(want));
    }

    // --- The bed's findings: T30's stand-ins, the bin straddling te (results version 9) ----------

    /// A deposit at which `noise::evaluate` refuses T30 for its resamples (more than 10 of 200
    /// fall short of -35 dB, their last bins a few whole deposits) and its stand-ins give it, with
    /// that evaluation. The bed's G2 refused T30 so at every receiver-band.
    fn t30_refused_by_its_resamples() -> (f64, noise::Parameters) {
        for d in [1e-3, 2e-3, 3e-3, 4e-3, 6e-3, 8e-3] {
            let p = noise::evaluate(&Ok(exponential(200)), Arrival::at(0.0), &random_model(d));
            if let Err(e) = &p.t30_s
                && let Some(NotEvaluable::MonteCarloNoise {
                    refused_resamples, ..
                }) = e.not_evaluable()
                && *refused_resamples > noise::REFUSED_RESAMPLES_ALLOWED
                && p.widen[3].stand_in.is_some()
            {
                return (d, p);
            }
        }
        panic!("no deposit tried refuses T30 for its resamples with stand-ins that give it");
    }

    #[test]
    fn t30_refused_by_its_resamples_alone_is_shown_wide_from_its_stand_ins() {
        let (d, p) = t30_refused_by_its_resamples();
        let Some(NotEvaluable::MonteCarloNoise {
            value,
            sd,
            refused_resamples,
            ..
        }) = p.t30_s.as_ref().unwrap_err().not_evaluable().cloned()
        else {
            unreachable!()
        };
        let st = p.widen[3].stand_in.unwrap();
        assert_eq!(st.refused_resamples, refused_resamples);
        if let Some(sd) = sd {
            assert!(st.sd >= sd, "{st:?} {sd}");
        }
        let e = evaluated(&Ok(exponential(200)), Arrival::at(0.0), &random_model(d));
        let half = 2.5 * st.sd;
        assert_eq!(
            e.parameters.t30_s,
            Evaluated::Value {
                value,
                mc_sd: Some(sd.unwrap_or(st.sd)),
                status: Some(RangeStatus::Wide),
                lo: Some(value - half),
                hi: Some(value + half),
                refused_resamples: Some(refused_resamples),
                straddle: None,
            },
            "deposit {d}"
        );
        // The judgement is the resamples' as judged: the curvature still refuses with T30.
        assert!(e.curvature.percent.refusal().is_some());
        let j = serde_json::to_value(&e.parameters).unwrap();
        assert_eq!(j["t30_s"]["refused_resamples"], refused_resamples);
        assert!(
            j["t20_s"].get("refused_resamples").is_none(),
            "{}",
            j["t20_s"]
        );
        // Says no: without its stand-ins the same refusal is shown as it was, refused.
        assert!(
            Evaluated::of_parameter(3, p.t30_s.clone())
                .refusal()
                .is_some()
        );
    }

    /// The bed's `s2|B26|rec1|R0.1|8000Hz|10ms` in miniature (`tests/params_straddle.rs`): C50 from
    /// the in-bin decay is 0.48 dB off the truth; shown `wide` with a range that holds it.
    #[test]
    fn c50_whose_straddling_bin_can_move_it_beyond_its_limit_is_wide_with_the_bracket() {
        let (ta, k, direct, t_r, refl) = (0.0311, 6.0 * std::f64::consts::LN_10, 0.3, 0.087, 0.3);
        let energy = |a: f64, b: f64| {
            let rev = |t: f64| 1.0 - (-k * (t - ta).max(0.0)).exp();
            let mut e = rev(b) - rev(a);
            if (a..b).contains(&ta) {
                e += direct;
            }
            if (a..b).contains(&t_r) {
                e += refl;
            }
            e
        };
        let dt = 0.01;
        let bins: Vec<f64> = (0..300)
            .map(|i| energy(i as f64 * dt, (i + 1) as f64 * dt))
            .collect();
        let early = energy(0.0, ta + 0.05);
        let truth = 10.0 * (early / (1.0 + direct + refl - early)).log10();
        let s = EnergySeries::complete(dt, bins).unwrap();
        let e = evaluated(&Ok(s), Arrival::at(ta), &random_model(1e-12));
        let Evaluated::Value {
            value,
            status,
            lo: Some(lo),
            hi: Some(hi),
            straddle: Some([s_lo, s_hi]),
            mc_sd: Some(sd),
            ..
        } = e.parameters.c50_db
        else {
            panic!("{:?}", e.parameters.c50_db)
        };
        assert!((value - truth).abs() > 0.1, "{value} vs {truth}");
        assert_eq!(status, Some(RangeStatus::Wide));
        assert!(s_lo <= truth && truth <= s_hi, "{truth} [{s_lo}, {s_hi}]");
        assert!(lo <= s_lo - 2.5 * sd + 1e-12 && hi >= s_hi + 2.5 * sd - 1e-12);
        // Without it the same value would have been ok: its noise is nothing.
        assert!(2.5 * sd < 1.0);
        let j = serde_json::to_value(&e.parameters).unwrap();
        assert_eq!(j["c50_db"]["straddle"][0].as_f64(), Some(s_lo));
        assert_eq!(j["c50_db"]["status"], "wide");
    }

    // --- G and dB(A) (M8b; PLAN.md "SPL (and dB(A), G on top)") ---------------------------------

    use crate::params::decay::P_REF_SQUARED;
    use crate::params::level;
    use crate::results::spps::SourceEchogram;

    /// SPL's value with a range, as `params::noise::shown` gives it.
    fn spl_value(value: f64, sd: f64) -> Evaluated {
        let s = noise::range(0, value, sd);
        Evaluated::Value {
            value,
            mc_sd: Some(sd),
            status: Some(s.status),
            lo: Some(s.lo),
            hi: Some(s.hi),
            refused_resamples: None,
            straddle: None,
        }
    }

    #[test]
    fn g_is_spl_less_the_free_field_at_ten_metres_with_the_same_range() {
        // A band whose energy is exactly the free field at 10 m of its sources: G is 0.
        let power_rho_c = 2.0 * 413.25;
        let free = 10.0 * (level::free_field_pa2(power_rho_c, 10.0) / P_REF_SQUARED).log10();
        let g = strength(&spl_value(free, 0.3), power_rho_c);
        let Evaluated::Value {
            value,
            mc_sd,
            status,
            lo,
            hi,
            ..
        } = g
        else {
            panic!("{g:?}")
        };
        assert!(value.abs() < 1e-9, "{value}");
        // SPL minus a constant: the same standard deviation, status and range width.
        assert_eq!(mc_sd, Some(0.3));
        assert_eq!(status, Some(RangeStatus::Ok));
        assert!((lo.unwrap() - (value - 0.75)).abs() < 1e-9);
        assert!((hi.unwrap() - (value + 0.75)).abs() < 1e-9);
        // A wide SPL gives a wide G.
        assert_eq!(
            strength(&spl_value(free, 1.0), power_rho_c).status(),
            Some(RangeStatus::Wide)
        );
        // A bare SPL gives a bare G.
        let bare = strength(&Evaluated::bare(free + 3.0, None), power_rho_c);
        assert!((bare.value().unwrap() - 3.0).abs() < 1e-9);
        assert_eq!(bare.status(), None);
    }

    #[test]
    fn a_refused_spl_refuses_g_with_the_same_reason_and_no_power_refuses_g() {
        let spl = Evaluated::refused(ParamError::NoEnergy);
        assert_eq!(strength(&spl, 826.5), spl);
        let noisy = Evaluated::refused(params::not_evaluable(
            Quantity::Spl,
            NotEvaluable::SeveralSources {
                sources: vec!["x".into()],
            },
        ));
        assert_eq!(strength(&noisy, 826.5), noisy);
        // No source emits in the band.
        assert_eq!(
            strength(&spl_value(70.0, 0.1), 0.0).refusal().unwrap().code,
            crate::params::codes::NO_ENERGY
        );
    }

    #[test]
    fn dba_sums_the_computed_bands_with_their_weights_and_says_which() {
        let a = spl_value(70.0, 0.2);
        let b = spl_value(65.0, 0.4);
        let d = dba_report(&[(1000, &a), (2000, &b)]);
        assert_eq!(d.bands_hz, vec![1000, 2000]);
        assert_eq!(d.weights_db, vec![0.0, 1.2]);
        assert!(d.unweighted_hz.is_empty());
        let (want, sd) =
            level::a_weighted(&[(1000, 70.0, Some(0.2)), (2000, 65.0, Some(0.4))]).unwrap();
        assert_eq!(d.level_db, spl_value(want, sd.unwrap()));
        // A single band: its SPL plus its weight.
        let one = dba_report(&[(125, &a)]);
        assert!((one.level_db.value().unwrap() - (70.0 - 16.1)).abs() < 1e-9);
        assert!(d.method.contains("IEC 61672-1"));
    }

    #[test]
    fn dba_is_refused_when_any_band_spl_is_refused_or_has_no_weight() {
        let ok = spl_value(70.0, 0.2);
        let refused = Evaluated::refused(params::not_evaluable(
            Quantity::Spl,
            NotEvaluable::NoTimeSeries {
                detail: "TCR".into(),
            },
        ));
        let d = dba_report(&[(500, &ok), (1000, &refused)]);
        let r = d.level_db.refusal().expect("refused");
        let spl = refused.refusal().unwrap();
        assert_eq!(r.code, spl.code);
        assert_eq!(r.error, spl.error);
        assert!(r.message.contains("1000 Hz"), "{}", r.message);
        // The bands it would have summed are still named.
        assert_eq!(d.bands_hz, vec![500, 1000]);
        // A third-octave band has no pinned weight.
        let d = dba_report(&[(500, &ok), (630, &ok)]);
        assert_eq!((d.bands_hz, d.weights_db), (vec![500], vec![-3.2]));
        assert_eq!(d.unweighted_hz, vec![630]);
        assert_eq!(
            d.level_db.refusal().unwrap().error,
            params::not_evaluable(
                Quantity::AWeighted,
                NotEvaluable::NoAWeight { freq_hz: 630 }
            )
        );
    }

    /// Two sources, 1 W and 3 W in each of two bands, the second bringing half the first's energy
    /// to the receiver, echograms on.
    fn two_source_run() -> SppsResults {
        let bins = decay(1500, DISTANCE_M / C, 0.6);
        let mut s = edt_run(0, 0.0, bins.clone());
        let rho_c = 413.25;
        s.sources[0].band_power_w = vec![1.0, 1.0];
        let second = SourcePoint {
            name: "T".into(),
            band_power_w: vec![3.0, 3.0],
            ..s.sources[0].clone()
        };
        s.sources.push(second);
        s.echogram_per_source = true;
        let half: Vec<f64> = bins.iter().map(|e| 0.5 * e).collect();
        let sum: Vec<f64> = bins.iter().zip(&half).map(|(a, b)| a + b).collect();
        let total: f64 = bins.iter().sum();
        let r = &mut s.point_receivers[0];
        let band = r.bands[0].clone();
        r.bands = [500, 1000]
            .into_iter()
            .map(|f| ReceiverBand {
                freq_hz: f,
                energy: sum.clone(),
                source_power_rho_c: 4.0 * rho_c,
                ..band.clone()
            })
            .collect();
        r.by_source = vec![
            SourceTotals {
                source: "S".into(),
                energy: vec![total, total],
            },
            SourceTotals {
                source: "T".into(),
                energy: vec![0.5 * total, 0.5 * total],
            },
        ];
        r.echograms = vec![
            SourceEchogram {
                source: "S".into(),
                file: "S/x.recp".into(),
                energy: vec![bins.clone(), bins.clone()],
            },
            SourceEchogram {
                source: "T".into(),
                file: "T/x.recp".into(),
                energy: vec![half.clone(), half],
            },
        ];
        // The room table, the particles' lifetimes the noise model's correction needs: the room's
        // energy falling 60 dB in 0.6 s from the sources' power times rho c.
        s.total_energy = [500, 1000]
            .into_iter()
            .map(|f| BandEnergy {
                freq_hz: f,
                energy: (0..1500)
                    .map(|k| 4.0 * rho_c * 10f64.powf(-6.0 * k as f64 * f64::from(DT) / 0.6))
                    .collect(),
            })
            .collect();
        let stats = s.particles.bands[0];
        s.particles.bands = [500, 1000]
            .into_iter()
            .map(|f| BandStats {
                freq_hz: f,
                ..stats
            })
            .collect();
        s
    }

    #[test]
    fn several_sources_g_sums_their_energies_against_their_free_fields_summed() {
        let s = two_source_run();
        let rep = receiver_report(&[500, 1000], &s, &s.point_receivers[0], true);
        let free = |p: f64| level::free_field_pa2(p, 10.0);
        let rho_c = 413.25;
        for (i, b) in rep.bands.iter().enumerate() {
            let spl = b
                .parameters
                .spl_db
                .value()
                .unwrap_or_else(|| panic!("{:?}", b.parameters.spl_db));
            // The receiver's G: both sources' energy against both free fields, 1 W + 3 W.
            let want = 10.0 * (b.total_pa2 / free(4.0 * rho_c)).log10();
            let g = b.g_db.value().expect("G");
            assert!((g - want).abs() < 1e-6, "band {i}: {g} vs {want}");
            assert!((spl - g - 10.0 * (free(4.0 * rho_c) / P_REF_SQUARED).log10()).abs() < 1e-9);
            // Each source's own G: its energy against its own free field.
            for (ps, w) in rep.per_source.iter().zip([1.0, 3.0]) {
                let sb = &ps.bands[i];
                let want = 10.0 * (sb.total_pa2 / free(w * rho_c)).log10();
                let g = sb.g_db.value().expect("source G");
                assert!(
                    (g - want).abs() < 1e-6,
                    "{} band {i}: {g} vs {want}",
                    ps.source
                );
            }
        }
        // S is 1 W and brings 2/3 of the energy, T 3 W and 1/3: T's G is 10 lg 6 lower.
        let gs = rep.per_source[0].bands[0].g_db.value().unwrap();
        let gt = rep.per_source[1].bands[0].g_db.value().unwrap();
        assert!((gs - gt - 10.0 * 6f64.log10()).abs() < 1e-6, "{gs} {gt}");
        // dB(A) per receiver, and per source, over the two bands.
        let d = &rep.aggregate.dba;
        assert_eq!(d.bands_hz, vec![500, 1000]);
        let l: Vec<f64> = rep
            .bands
            .iter()
            .map(|b| b.parameters.spl_db.value().unwrap())
            .collect();
        let (want, _) = level::a_weighted(&[(500, l[0], None), (1000, l[1], None)]).unwrap();
        assert!((d.level_db.value().unwrap() - want).abs() < 1e-9);
        assert!(rep.per_source[1].aggregate.dba.level_db.value().is_some());
        // The JSON names them.
        let j = serde_json::to_value(&rep).unwrap();
        assert!(
            j["bands"][0]["g_db"]["value"].is_number(),
            "{}",
            j["bands"][0]["g_db"]
        );
        assert!(j["aggregate"]["dba"]["level_db"]["value"].is_number());
        assert_eq!(
            j["aggregate"]["dba"]["bands_hz"],
            serde_json::json!([500, 1000])
        );
        assert!(j["per_source"][0]["bands"][0]["g_db"]["value"].is_number());
    }

    #[test]
    fn a_tcr_receiver_refuses_g_and_dba_as_its_spl() {
        let r = tcr::PointReceiver {
            label: "R".into(),
            file: "R.gabe".into(),
            bands: vec![tcr::ReceiverBand {
                freq_hz: 1000,
                direct_db: 60.0,
                total_sabine_db: 70.0,
                total_eyring_db: 69.0,
            }],
            global_direct_db: 60.0,
            global_total_sabine_db: 70.0,
            global_total_eyring_db: 69.0,
        };
        let rep = TcrReceiverReport::of(&r);
        let spl = rep.bands[0].parameters.spl_db.refusal().unwrap().clone();
        assert_eq!(rep.bands[0].g_db.refusal(), Some(&spl));
        let d = rep.aggregate.dba.level_db.refusal().unwrap();
        assert_eq!((d.code.as_str(), &d.error), (spl.code.as_str(), &spl.error));
    }

    // --- STI (results version 10) ---------------------------------------------------------------

    /// [`edt_run`] in random mode (no floor to bound) with its one band copied onto every octave
    /// of `freqs`.
    fn octave_run(freqs: &[i32], bins: Vec<f64>, noise_db: f64) -> SppsResults {
        let mut s = edt_run(0, 0.0, bins);
        let band = s.point_receivers[0].bands[0].clone();
        let stats = s.particles.bands[0];
        s.point_receivers[0].bands = freqs
            .iter()
            .map(|&f| ReceiverBand {
                freq_hz: f,
                background_noise_db: noise_db,
                ..band.clone()
            })
            .collect();
        s.particles.bands = freqs
            .iter()
            .map(|&f| BandStats {
                freq_hz: f,
                ..stats
            })
            .collect();
        s.sources[0].band_power_w = vec![1.0; freqs.len()];
        s.point_receivers[0].by_source[0].energy = vec![1.0; freqs.len()];
        // The room table, whose lifetimes the noise model needs (as `two_source_run`'s).
        s.total_energy = freqs
            .iter()
            .map(|&f| BandEnergy {
                freq_hz: f,
                energy: (0..1500)
                    .map(|k| 2.0 * 10f64.powf(-6.0 * k as f64 * f64::from(DT) / 1.0))
                    .collect(),
            })
            .collect();
        s
    }

    #[test]
    fn a_receiver_with_the_seven_octaves_carries_sti_male_shown_and_female_beside_it() {
        let octaves = [125, 250, 500, 1000, 2000, 4000, 8000];
        let s = octave_run(&octaves, decay(3000, DISTANCE_M / C, 1.0), 0.0);
        let rep = receiver_report(&octaves, &s, &s.point_receivers[0], true);
        let sti = &rep.sti;
        assert_eq!(sti.shown, sti::Gender::Male);
        assert!(
            sti.weighting.contains("predicted impulse response"),
            "{}",
            sti.weighting
        );
        assert!(sti.weighting.contains("male"));
        assert_eq!(sti.noise, STI_NOISE_NONE);
        assert_eq!(sti.monte_carlo, STI_NOISE_NOT_MODELLED);
        assert_eq!(sti.speech_level_dba_at_1m, 60.0);
        let male = sti.male.value().unwrap_or_else(|| panic!("{:?}", sti.male));
        let female = sti.female.value().expect("a female STI");
        assert!((0.0..=1.0).contains(&male) && (0.0..=1.0).contains(&female));
        // No Monte-Carlo range: a bare value.
        assert_eq!(sti.male, Evaluated::bare(male, None));
        assert_eq!(sti.bands.len(), 7);
        assert!(
            sti.bands
                .iter()
                .all(|b| b.mtf.as_ref().is_some_and(|m| m.len() == 14))
        );
        assert_eq!(sti.bands[0].mti_female, None);
        assert!(
            sti.bands
                .iter()
                .all(|b| b.mti_male.is_some() && b.noise_db.is_none())
        );
        // The same with a background noise of 45 dB in every band: lower, and said so.
        let noisy = octave_run(&octaves, decay(3000, DISTANCE_M / C, 1.0), 45.0);
        let rep = receiver_report(&octaves, &noisy, &noisy.point_receivers[0], true);
        assert_eq!(rep.sti.noise, STI_NOISE_RECEIVER);
        assert!(rep.sti.male.value().unwrap() < male - 0.01);
        // Third-octave bands refuse.
        let rep = receiver_report(&octaves, &s, &s.point_receivers[0], false);
        assert!(matches!(
            rep.sti.male.refusal().unwrap().error.not_evaluable(),
            Some(NotEvaluable::NotOctaveBands { .. })
        ));
    }

    #[test]
    fn a_run_without_the_seven_octaves_refuses_sti_naming_the_band() {
        let six = [125, 250, 500, 1000, 2000, 4000];
        let s = octave_run(&six, decay(3000, DISTANCE_M / C, 1.0), 0.0);
        let rep = receiver_report(&six, &s, &s.point_receivers[0], true);
        for e in [&rep.sti.male, &rep.sti.female] {
            let r = e.refusal().expect("refused");
            assert_eq!(r.code, crate::params::codes::NOT_EVALUABLE);
            assert!(
                r.message.contains("STI: band_missing: the 8000 Hz octave"),
                "{}",
                r.message
            );
        }
        // What the bands it has gave is still reported.
        assert_eq!(rep.sti.bands.len(), 6);
        assert!(rep.sti.bands.iter().all(|b| b.mtf.is_some()));
    }

    /// [`octave_run`] in energetic mode (floor −50 dB), its room table the decay's own to the
    /// series' end, with `remaining` particles of the 125 Hz band alive at the end holding
    /// `end_share` of the emitted energy.
    fn energetic_octave_run(freqs: &[i32], remaining: u32, end_share: f64) -> SppsResults {
        let n = 3000;
        let mut s = octave_run(freqs, decay(n, DISTANCE_M / C, 1.0), 0.0);
        s.computation_method = 1;
        let power = s.point_receivers[0].bands[0].source_power_rho_c;
        let dt = f64::from(DT);
        for e in &mut s.total_energy {
            e.energy = (0..n)
                .map(|k| power * 10f64.powf(-6.0 * k as f64 * dt))
                .collect();
        }
        s.total_energy[0].energy[n - 1] = power * end_share;
        s.particles.bands[0].remaining = remaining;
        s.particles.bands[0].absorbed_by_materials -= remaining;
        s
    }

    /// The STI bed's set C7 (ADDENDUM-4): one particle of 150,000 alive at 10 s left the 125 Hz band
    /// incomplete and refused male STI at every receiver, although the energy it can still bring is
    /// bounded and negligible. STI now refuses only when that energy can move it by more than
    /// [`sti::UNSEEN_LIMIT`].
    #[test]
    fn a_band_with_particles_alive_at_the_end_gives_sti_when_what_they_carry_cannot_move_it() {
        let octaves = [125, 250, 500, 1000, 2000, 4000, 8000];
        let ended = energetic_octave_run(&octaves, 0, 0.0);
        let want = receiver_report(&octaves, &ended, &ended.point_receivers[0], true)
            .sti
            .male
            .value()
            .expect("male STI of the complete run");
        // One particle alive at the end, holding 1e-12 of the emitted energy: the same STI.
        let one = energetic_octave_run(&octaves, 1, 1e-12);
        assert!(!one.band_complete(125));
        let rep = receiver_report(&octaves, &one, &one.point_receivers[0], true);
        assert_eq!(rep.sti.male.value(), Some(want), "{:?}", rep.sti.male);
        // Particles alive at the end holding half of it: refused, saying why.
        let half = energetic_octave_run(&octaves, 1, 0.5);
        let rep = receiver_report(&octaves, &half, &half.point_receivers[0], true);
        let r = rep.sti.male.refusal().expect("refused");
        assert!(
            matches!(
                r.error.not_evaluable(),
                Some(NotEvaluable::BandRefused { freq_hz: 125, .. })
            ),
            "{}",
            r.message
        );
        assert!(r.message.contains("can move the STI"), "{}", r.message);
        // Female speech has no 125 Hz band, and the receiver no noise: answered.
        assert!(rep.sti.female.value().is_some());
    }

    /// Backlog 67: a receiver with noise in some bands is written 0 dB in the others, which is no
    /// noise, not 0 dB SPL of it.
    #[test]
    fn a_band_at_0_db_noise_contributes_no_noise_to_sti() {
        let octaves = [125, 250, 500, 1000, 2000, 4000, 8000];
        let mut s = octave_run(&octaves, decay(3000, DISTANCE_M / C, 1.0), 0.0);
        s.point_receivers[0].bands[3].background_noise_db = 40.0;
        let rep = receiver_report(&octaves, &s, &s.point_receivers[0], true);
        assert_eq!(rep.sti.noise, STI_NOISE_RECEIVER);
        for b in &rep.sti.bands {
            assert_eq!(
                b.noise_db,
                (b.freq_hz == 1000).then_some(40.0),
                "{}",
                b.freq_hz
            );
        }
    }

    #[test]
    fn several_sources_refuse_sti() {
        let s = two_source_run();
        let rep = receiver_report(&[500, 1000], &s, &s.point_receivers[0], true);
        for e in [&rep.sti.male, &rep.sti.female] {
            assert!(
                matches!(
                    e.refusal().unwrap().error.not_evaluable(),
                    Some(NotEvaluable::SeveralSources { .. })
                ),
                "{e:?}"
            );
        }
    }
}
