//! The JSON `simpa results --json` prints: a [`RunResults`] and, per SPPS point receiver and band,
//! `core::params`' SPL, EDT, T20, T30, C50, C80, D50 and Ts, each a value or the reason it is not
//! evaluable. The shape is documented in `docs/formats/results-json.md` and generated as a JSON
//! Schema by [`report_schema`] (`docs/formats/results-json.schema.json`), which M12 reads.
//!
//! **`validated_by_bed` is false**: no number here may be shown to a user before M8's physics bed
//! passes (`docs/rebuild-plan.md`, M12).

use schemars::JsonSchema;
use serde::Serialize;

use super::spps::{BandEnergy, ParticleFileSummary, SourcePoint, SourceTotals, SppsResults};
use super::tcr::{self, MainBand, TcrResults};
use super::{RunResults, SolverResults, SurfaceFile};
use crate::params::decay::{self, Arrival, Onset};
use crate::params::{self, EnergySeries, ParamError};
use crate::run::stats::ParticleStats;
use crate::run::verdict::Status;
use crate::schema::SolverKind;

/// The layout of [`Report`]; bumped when a field changes meaning.
pub const REPORT_VERSION: u32 = 1;

/// A quantity's value, or why it has none.
#[derive(Clone, Debug, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Evaluated {
    /// In the quantity's unit (the field name says it).
    Value(f64),
    /// `core::params` refused it.
    NotEvaluable(Refused),
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
    fn of(r: Result<f64, ParamError>) -> Self {
        match r {
            Ok(v) => Evaluated::Value(v),
            Err(e) => Evaluated::NotEvaluable(Refused {
                code: e.code().to_string(),
                message: e.to_string(),
                error: e,
            }),
        }
    }

    /// The value, if there is one.
    pub fn value(&self) -> Option<f64> {
        match self {
            Evaluated::Value(v) => Some(*v),
            Evaluated::NotEvaluable(_) => None,
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
    fn refused(e: &ParamError) -> Self {
        let r = || Evaluated::of(Err(e.clone()));
        Parameters {
            spl_db: r(),
            edt_s: r(),
            t20_s: r(),
            t30_s: r(),
            c50_db: r(),
            c80_db: r(),
            d50: r(),
            ts_s: r(),
        }
    }

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
}

/// `core::params` on one series, measured from `arrival`. The series itself may be refused (all
/// zero, for instance): then every parameter carries that refusal. SPL does not depend on the
/// arrival, so a given arrival that `params` refuses (`params_bad_arrival`) refuses the other
/// seven only.
pub fn parameters(
    series: &Result<EnergySeries, ParamError>,
    arrival: Arrival,
) -> (Parameters, Option<Onset>) {
    let s = match series {
        Ok(s) => s,
        Err(e) => return (Parameters::refused(e), None),
    };
    let spl_db = Evaluated::of(decay::spl_db(s));
    match decay::evaluate(s, arrival) {
        Ok(p) => (
            Parameters {
                spl_db,
                edt_s: Evaluated::of(p.edt.map(|f| f.t_s)),
                t20_s: Evaluated::of(p.t20.map(|f| f.t_s)),
                t30_s: Evaluated::of(p.t30.map(|f| f.t_s)),
                c50_db: Evaluated::of(p.c50_db),
                c80_db: Evaluated::of(p.c80_db),
                d50: Evaluated::of(p.d50),
                ts_s: Evaluated::of(p.ts_s),
            },
            Some(p.onset),
        ),
        Err(e) => {
            let mut all = Parameters::refused(&e);
            all.spl_db = spl_db;
            (all, Some(decay::onset(s)))
        }
    }
}

/// One band of an SPPS point receiver.
#[derive(Clone, Debug, PartialEq, Serialize, JsonSchema)]
pub struct ReceiverBandReport {
    pub freq_hz: i32,
    /// SPPS's statistics show that nothing arrives after the series' end (random mode, no
    /// particle remaining: `spps::SppsResults::band_complete`), so the series is given to
    /// `params` as complete and no tail is bounded. Otherwise `params` bounds the tail.
    pub complete: bool,
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

/// All bands of a receiver summed bin by bin (`params::aggregate`): **an aggregate, not a band**.
#[derive(Clone, Debug, PartialEq, Serialize, JsonSchema)]
pub struct AggregateReport {
    /// Always `"all computed bands summed bin by bin"`.
    pub aggregate: String,
    /// The bands summed: those whose series `params` accepts.
    pub bands_hz: Vec<i32>,
    pub parameters: Parameters,
}

/// An SPPS point receiver.
#[derive(Clone, Debug, PartialEq, Serialize, JsonSchema)]
pub struct SppsReceiverReport {
    pub label: String,
    /// Relative to `solve/`.
    pub folder: String,
    pub position_m: Option<[f64; 3]>,
    /// The direct sound's arrival at the receiver's centre, which every onset-relative parameter
    /// is measured from (`params::decay::Arrival::Known`); `null` when it is not computed
    /// (a celerity gradient, or a position not read), and the parameters then use
    /// `Arrival::Detected`.
    pub arrival_s: Option<f64>,
    pub bands: Vec<ReceiverBandReport>,
    pub aggregate: AggregateReport,
    /// Each source's total per band, Pa² (`.recps`), in `config.xml`'s order.
    pub by_source: Vec<SourceTotals>,
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

fn spps_report(bands_hz: &[i32], s: &SppsResults) -> SppsReport {
    let point_receivers = s
        .point_receivers
        .iter()
        .map(|r| {
            let arrival_s = s.arrival_s(r);
            let arrival = arrival_s.map_or(Arrival::Detected, |t| Arrival::Known { time_s: t });
            let series: Vec<Result<EnergySeries, ParamError>> = r
                .bands
                .iter()
                .map(|b| {
                    if s.band_complete(b.freq_hz) {
                        EnergySeries::complete(s.time_step_s, b.energy.clone())
                    } else {
                        EnergySeries::new(s.time_step_s, b.energy.clone())
                    }
                })
                .collect();
            let bands = r
                .bands
                .iter()
                .zip(&series)
                .map(|(b, se)| {
                    let (parameters, onset) = parameters(se, arrival);
                    ReceiverBandReport {
                        freq_hz: b.freq_hz,
                        complete: s.band_complete(b.freq_hz),
                        energy_pa2: b.energy.clone(),
                        total_pa2: b.energy.iter().sum(),
                        source_power_rho_c: b.source_power_rho_c,
                        background_noise_db: b.background_noise_db,
                        onset,
                        parameters,
                    }
                })
                .collect();
            let (valid, summed): (Vec<i32>, Vec<EnergySeries>) = bands_hz
                .iter()
                .zip(&series)
                .filter_map(|(&f, se)| se.as_ref().ok().map(|x| (f, x.clone())))
                .unzip();
            let aggregate = params::aggregate(&summed).map_err(|e| {
                // No band holds energy: say so with the series' own refusal.
                if summed.is_empty() {
                    ParamError::NoEnergy
                } else {
                    e
                }
            });
            let (agg_params, _) = parameters(&aggregate, arrival);
            SppsReceiverReport {
                label: r.label.clone(),
                folder: r.folder.clone(),
                position_m: r.position_m,
                arrival_s,
                bands,
                aggregate: AggregateReport {
                    aggregate: "all computed bands summed bin by bin".into(),
                    bands_hz: valid,
                    parameters: agg_params,
                },
                by_source: r.by_source.clone(),
            }
        })
        .collect();
    SppsReport {
        time_step_s: s.time_step_s,
        duration_s: s.duration_s,
        steps: s.steps,
        speed_of_sound_m_s: s.speed_of_sound_m_s,
        receiver_radius_m: s.receiver_radius_m,
        receiver_crossing_s: s.receiver_crossing_s(),
        celerity_gradient: s.celerity_gradient,
        sources: s.sources.clone(),
        particles: s.particles.clone(),
        total_energy: s.total_energy.clone(),
        point_receivers,
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

/// The report of a verified run.
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
