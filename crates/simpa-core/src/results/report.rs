//! The JSON `simpa results --json` prints: a [`RunResults`] and, per SPPS point receiver and band,
//! `core::params`' SPL, EDT, T20, T30, C50, C80, D50 and Ts, each a value with its estimated
//! Monte-Carlo standard deviation, or the reason it is not evaluable. The shape is documented in
//! `docs/formats/results-json.md` and generated as a JSON Schema by [`report_schema`]
//! (`docs/formats/results-json.schema.json`), which M12 reads.
//!
//! **`validated_by_bed` is false**: no number here may be shown to a user before M8's physics bed
//! passes (`docs/rebuild-plan.md`, M12).

use schemars::JsonSchema;
use serde::Serialize;

use super::spps::{
    BandEnergy, ParticleFileSummary, PointReceiver, SourcePoint, SourceTotals, SppsResults,
};
use super::tcr::{self, MainBand, TcrResults};
use super::{Refusal, RunResults, SolverResults, SurfaceFile, value_invalid};
use crate::params::decay::{self, Arrival, Onset};
use crate::params::noise::{self, NoiseModel};
use crate::params::{self, EnergySeries, NotEvaluable, ParamError, Quantity};
use crate::run::stats::ParticleStats;
use crate::run::verdict::Status;
use crate::schema::SolverKind;

/// The layout of [`Report`]; bumped when a field changes meaning. 2: values carry `mc_sd`, and
/// the Monte-Carlo, floor and per-source fields were added.
pub const REPORT_VERSION: u32 = 2;

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

    /// A value that is not from a Monte-Carlo histogram, or its refusal.
    fn of(r: Result<f64, ParamError>) -> Self {
        match r {
            Ok(value) => Evaluated::Value { value, mc_sd: None },
            Err(e) => Self::refused(e),
        }
    }

    /// A Monte-Carlo value with its standard deviation, or its refusal.
    fn of_estimate(r: Result<noise::Estimate, ParamError>) -> Self {
        match r {
            Ok(e) => Evaluated::Value {
                value: e.value,
                mc_sd: Some(e.sd),
            },
            Err(e) => Self::refused(e),
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
        self.t20_s = refuse(Quantity::T20);
        self.t30_s = refuse(Quantity::T30);
        self.c50_db = refuse(Quantity::Clarity { te_s: 0.05 });
        self.c80_db = refuse(Quantity::Clarity { te_s: 0.08 });
        self.d50 = refuse(Quantity::Definition { te_s: 0.05 });
        self.ts_s = refuse(Quantity::CentreTime);
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
    let p = noise::evaluate(series, arrival, model);
    (
        Parameters {
            spl_db: Evaluated::of_estimate(p.spl_db),
            edt_s: Evaluated::of_estimate(p.edt_s),
            t20_s: Evaluated::of_estimate(p.t20_s),
            t30_s: Evaluated::of_estimate(p.t30_s),
            c50_db: Evaluated::of_estimate(p.c50_db),
            c80_db: Evaluated::of_estimate(p.c80_db),
            d50: Evaluated::of_estimate(p.d50),
            ts_s: Evaluated::of_estimate(p.ts_s),
        },
        p.onset,
    )
}

/// One band of an SPPS point receiver.
#[derive(Clone, Debug, PartialEq, Serialize, JsonSchema)]
pub struct ReceiverBandReport {
    pub freq_hz: i32,
    /// SPPS's statistics show that nothing arrives after the series' end (random mode, no
    /// particle remaining, none lost: `spps::SppsResults::band_complete`), so the series is given
    /// to `params` as complete and no tail is bounded. Otherwise `params` bounds the tail.
    pub complete: bool,
    /// The level below a particle's start at which SPPS drops it, dB, when that can cost the
    /// histogram energy (energetic mode: `-10·trans_epsilon`); `params` bounds what it can have
    /// dropped. `null` otherwise.
    pub floor_db: Option<f64>,
    /// The share of the energy from the arrival on that lost particles can have taken with them
    /// (`spps::SppsResults::lost_share`); `params` bounds what it can move. `null` when none was
    /// lost.
    pub lost_share: Option<f64>,
    /// The sources whose energy reaches the receiver in this band (their `.recps` total is above
    /// 0). With more than one, the seven onset-relative parameters are refused,
    /// `several_sources`.
    pub contributing_sources: Vec<String>,
    /// What the values' Monte-Carlo noise is estimated from (`params::noise`).
    pub noise_model: NoiseModel,
    /// The receiver crossings behind the series, estimated as its total over the mean deposit;
    /// `null` when the noise has no model.
    pub crossings: Option<f64>,
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
}

/// All bands of a receiver summed bin by bin (`params::aggregate`): **an aggregate, not a band,
/// and not ISO 3382-1's single-number value** (the arithmetic mean of band values), since its
/// decay is weighted by the source spectrum.
#[derive(Clone, Debug, PartialEq, Serialize, JsonSchema)]
pub struct AggregateReport {
    /// Always `"all computed bands summed bin by bin"`.
    pub aggregate: String,
    /// The bands summed: those whose series `params` accepts.
    pub bands_hz: Vec<i32>,
    pub parameters: Parameters,
}

/// One source's own echogram at a receiver, one band.
#[derive(Clone, Debug, PartialEq, Serialize, JsonSchema)]
pub struct SourceBandReport {
    pub freq_hz: i32,
    pub noise_model: NoiseModel,
    pub crossings: Option<f64>,
    /// The source's `.recp` column, Pa² per time step.
    pub energy_pa2: Vec<f64>,
    pub total_pa2: f64,
    pub onset: Option<Onset>,
    pub parameters: Parameters,
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
            cutting_plane: s.cutting_plane,
            record_type: format!("{:?}", s.data.record_type),
            time_steps: s.data.time_step_count,
            time_step_s: f64::from(s.data.time_step),
            receivers: s
                .data
                .receivers
                .iter()
                .map(|r| SurfaceReceiverSummary {
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
    /// The bootstrap's seed.
    pub seed: u64,
    /// The largest standard deviation a value may carry: EDT, T20 and T30 relative, C50 and C80
    /// in dB, D50 as a fraction, Ts in s, SPL in dB.
    pub limit_decay_relative: f64,
    pub limit_clarity_db: f64,
    pub limit_definition: f64,
    pub limit_centre_time_s: f64,
    pub limit_spl_db: f64,
}

impl MonteCarloReport {
    fn current() -> Self {
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
}

/// TCR's `Global` row: the energetic sum of the band levels, **an aggregate**.
#[derive(Clone, Debug, PartialEq, Serialize, JsonSchema)]
pub struct TcrGlobal {
    /// Always `"energetic sum of the band levels"`.
    pub aggregate: String,
    pub sabine_level_db: f64,
    pub eyring_level_db: f64,
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

/// A TCR run: its values as TCR computed them, and `core::params`' analytic Sabine and Eyring
/// times on the same inputs. TCR writes no time series, so there are no per-receiver parameters.
#[derive(Clone, Debug, PartialEq, Serialize, JsonSchema)]
pub struct TcrReport {
    pub bands: Vec<MainBand>,
    pub global: TcrGlobal,
    pub point_receivers: Vec<tcr::PointReceiver>,
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
    let base = if s.band_complete(freq_hz) {
        EnergySeries::complete(s.time_step_s, energy.to_vec())
    } else {
        EnergySeries::new(s.time_step_s, energy.to_vec())
    }?;
    let bin = match arrival {
        Arrival::Known { time_s } => (time_s / s.time_step_s).floor().max(0.0) as usize,
        Arrival::Detected => decay::onset(&base).index,
    };
    let base = match s.floor_db() {
        // Nothing known alive: the smallest share, so that nothing bounds what was dropped.
        Some(floor) => base.with_solver_floor(
            floor,
            s.alive_share(index, bin)
                .filter(|a| *a > 0.0)
                .unwrap_or(f64::MIN_POSITIVE),
        )?,
        None => base,
    };
    match s.lost_share(index, freq_hz, bin) {
        Some(share) => base.with_lost_share(share),
        None => Ok(base),
    }
}

/// The receiver crossings behind `total` under `model`.
fn crossings(model: &NoiseModel, total: f64) -> Option<f64> {
    match model {
        NoiseModel::Crossings { mean_deposit } => Some(total / mean_deposit),
        NoiseModel::Unknown { .. } => None,
    }
}

/// The aggregate's noise model: crossings of the largest band deposit, unknown when any band's is.
fn aggregate_model(models: &[&NoiseModel]) -> NoiseModel {
    let mut largest: Option<f64> = None;
    for m in models {
        match m {
            NoiseModel::Crossings { mean_deposit } => {
                largest = Some(largest.map_or(*mean_deposit, |l: f64| l.max(*mean_deposit)));
            }
            NoiseModel::Unknown { detail } => {
                return NoiseModel::Unknown {
                    detail: detail.clone(),
                };
            }
        }
    }
    match largest {
        Some(d) => NoiseModel::Crossings { mean_deposit: d },
        None => NoiseModel::Unknown {
            detail: "no band to aggregate".into(),
        },
    }
}

/// The aggregate of `series` (one per band, `bands_hz`), with its noise model from `models`.
fn aggregate_report(
    bands_hz: &[i32],
    series: &[Result<EnergySeries, ParamError>],
    models: &[NoiseModel],
    arrival: Arrival,
    contributing: &[&str],
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
    let (mut p, _) = parameters(&aggregate, arrival, &aggregate_model(&used));
    if contributing.len() > 1 {
        p.several_sources(contributing);
    }
    AggregateReport {
        aggregate: "all computed bands summed bin by bin".into(),
        bands_hz: valid,
        parameters: p,
    }
}

fn receiver_report(bands_hz: &[i32], s: &SppsResults, r: &PointReceiver) -> SppsReceiverReport {
    let arrival_s = s.arrival_s(r);
    let arrival = arrival_s.map_or(Arrival::Detected, |t| Arrival::Known { time_s: t });
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
            s.arrival_from(r, &contributing)
                .map_or(Arrival::Detected, |t| Arrival::Known { time_s: t })
        };
        let se = series_of(s, i, b.freq_hz, &b.energy, arrival);
        let (mut parameters, onset) = parameters(&se, arrival, &model);
        if contributing.len() > 1 {
            parameters.several_sources(&contributing);
        }
        let total_pa2: f64 = b.energy.iter().sum();
        bands.push(ReceiverBandReport {
            freq_hz: b.freq_hz,
            complete: s.band_complete(b.freq_hz),
            floor_db: s.floor_db(),
            lost_share: se.as_ref().ok().and_then(EnergySeries::lost_share),
            contributing_sources: contributing.iter().map(|c| c.to_string()).collect(),
            crossings: crossings(&model, total_pa2),
            noise_model: model.clone(),
            energy_pa2: b.energy.clone(),
            total_pa2,
            source_power_rho_c: b.source_power_rho_c,
            background_noise_db: b.background_noise_db,
            onset,
            parameters,
        });
        series.push(se);
        models.push(model);
    }
    let aggregate = aggregate_report(bands_hz, &series, &models, arrival, &all_contributing);
    let per_source = r
        .echograms
        .iter()
        .map(|e| {
            let name = [e.source.as_str()];
            let arrival_s = s.arrival_from(r, &name);
            let arrival = arrival_s.map_or(Arrival::Detected, |t| Arrival::Known { time_s: t });
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
            let bands = r
                .bands
                .iter()
                .zip(&e.energy)
                .zip(&series)
                .zip(&models)
                .map(|(((b, energy), se), model)| {
                    let (parameters, onset) = parameters(se, arrival, model);
                    let total_pa2: f64 = energy.iter().sum();
                    SourceBandReport {
                        freq_hz: b.freq_hz,
                        noise_model: model.clone(),
                        crossings: crossings(model, total_pa2),
                        energy_pa2: energy.clone(),
                        total_pa2,
                        onset,
                        parameters,
                    }
                })
                .collect();
            SourceReceiverReport {
                source: e.source.clone(),
                file: e.file.clone(),
                arrival_s,
                bands,
                aggregate: aggregate_report(bands_hz, &series, &models, arrival, &name),
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
        by_source: r.by_source.clone(),
        per_source,
    }
}

fn spps_report(bands_hz: &[i32], s: &SppsResults) -> SppsReport {
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
        monte_carlo: MonteCarloReport::current(),
        sources: s.sources.clone(),
        particles: s.particles.clone(),
        total_energy: s.total_energy.clone(),
        point_receivers: s
            .point_receivers
            .iter()
            .map(|r| receiver_report(bands_hz, s, r))
            .collect(),
        surfaces: s.surfaces.iter().map(SurfaceSummary::of).collect(),
        particle_files: s.particle_files.clone(),
    }
}

fn tcr_report(t: &TcrResults) -> TcrReport {
    TcrReport {
        bands: t.bands.clone(),
        global: TcrGlobal {
            aggregate: "energetic sum of the band levels".into(),
            sabine_level_db: t.global_sabine_level_db,
            eyring_level_db: t.global_eyring_level_db,
        },
        point_receivers: t.point_receivers.clone(),
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
        SolverResults::Spps(s) => (Some(spps_report(&r.bands_hz, s)), None),
        SolverResults::Tcr(t) => (None, Some(tcr_report(t))),
    };
    Report {
        results_version: REPORT_VERSION,
        validated_by_bed: false,
        run_folder: r.folder.display().to_string(),
        solver: r.manifest.solver,
        status: r.manifest.verdict.status,
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
}
