//! What the bed reads from each run: `simpa results`' report (`results::load` and
//! `results::checked_report`, in process), cut down to what M8a judges and reports, so that 400
//! runs' reports, several MB each, never sit in memory at once.
//!
//! **The value each seed gives** (`docs/investigations/2026-09-29-m8a/SPEC.md`, section 4): per
//! receiver and band, T30 is read from `spps.point_receivers[r].bands[b].parameters.t30_s`: a
//! value (with its `mc_sd`); or, refused `params_not_evaluable` for `monte_carlo_noise` or
//! `noise_uncalibrated`, the refusal's own value ("the value from the series", and its `sd` when
//! there is one): a seed's own noise refusal does not refuse the batch (D6). Any other refusal is
//! kept as its reason, and makes the cell not judged (E6, SB-3). T20 is read from
//! `parameters.t20_s` by the same rules, for its own gate C (the T20 twin).
//!
//! Since results version 7 a value refused for its standard deviation alone is shown instead,
//! `wide`, with the same value and `mc_sd` (decision-log row 37 (3)): such a value is read as the
//! refusal it was, `monte_carlo_noise` (`params::noise::over_limit`), so the numbers and sources
//! the bed reads, and its verdicts, do not change. Since version 9 a value refused for its
//! resamples alone can be shown from its stand-ins, `wide`, with `refused_resamples` and the judged
//! `mc_sd` (`params::noise`, "The stand-ins"): read as that refusal too.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::params::noise::{self, RangeStatus};
use crate::params::{NotEvaluable, ParamError};
use crate::results::report::{Evaluated, ReferenceReport, Report};
use crate::run::manager::{MESH_DIR, SOLVE_DIR};
use crate::run::manifest::{FILE_NAME as RUN_JSON, RunManifest, RunSource};

/// A run folder's record: `run.json` and `mesh/mesh.json`, and what the folder holds.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct RunInfo {
    pub folder: String,
    /// `OK`, `FAIL`, `CRASH` or `CANCELLED`.
    pub status: String,
    pub reasons: Vec<String>,
    pub exit_class: u8,
    /// The solver process's wall time, s.
    pub solver_wall_s: Option<f64>,
    /// The solver's CPU time: **not recorded** by the run manager (`process::Outcome` has none),
    /// so always `null`.
    pub solver_cpu_s: Option<f64>,
    /// `run.json`'s `exe`: the solver as the run saw it, and its raw sha256.
    pub exe: String,
    pub exe_sha256: String,
    /// `run.json`'s `source.sha256`: the project file the run started from, as the run manager
    /// hashed it. `None` for a run of a fixture folder, which has no project. The bed matches it
    /// with the project it would write for the run's cell and seed (`run::check_planned`).
    #[serde(default)]
    pub project_sha256: Option<String>,
    /// `mesh.json`'s TetGen sha256, when the run meshed.
    pub tetgen_sha256: Option<String>,
    /// Files and bytes under the run folder.
    pub files: u64,
    pub bytes: u64,
}

/// Files and bytes under `dir`, recursively.
pub fn count_files(dir: &Path) -> (u64, u64) {
    let (mut files, mut bytes) = (0, 0);
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&d) else {
            continue;
        };
        for e in rd.flatten() {
            match e.file_type() {
                Ok(t) if t.is_dir() => stack.push(e.path()),
                Ok(t) if t.is_file() => {
                    files += 1;
                    bytes += e.metadata().map_or(0, |m| m.len());
                }
                _ => {}
            }
        }
    }
    (files, bytes)
}

/// The one run folder the run manager made in `dir` (the folder with a `run.json`).
pub fn find_run_folder(dir: &Path) -> Option<PathBuf> {
    let mut found: Vec<PathBuf> = std::fs::read_dir(dir)
        .ok()?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.join(RUN_JSON).is_file())
        .collect();
    found.sort();
    found.pop()
}

/// `run.json` and `mesh/mesh.json` of `folder`.
pub fn run_info(folder: &Path) -> Result<RunInfo, String> {
    let path = folder.join(RUN_JSON);
    let text = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    let m = RunManifest::from_json(&text).map_err(|e| format!("{}: {e}", path.display()))?;
    let tetgen_sha256 = crate::mesh::read_manifest(&folder.join(MESH_DIR))
        .ok()
        .and_then(|mm| mm.tetgen.and_then(|t| t.program_sha256));
    let (files, bytes) = count_files(folder);
    let status = serde_json::to_value(m.verdict.status)
        .ok()
        .and_then(|v| v.as_str().map(str::to_string))
        .unwrap_or_default();
    let project_sha256 = match &m.source {
        RunSource::Project { sha256, .. } => Some(sha256.clone()),
        RunSource::Fixture { .. } => None,
    };
    Ok(RunInfo {
        folder: folder.display().to_string(),
        status,
        reasons: m.verdict.codes().iter().map(|c| c.to_string()).collect(),
        exit_class: m.exit_class.code(),
        solver_wall_s: m.outcome.map(|o| o.elapsed_ms / 1000.0),
        solver_cpu_s: None,
        exe: m.exe.path,
        exe_sha256: m.exe.sha256,
        project_sha256,
        tetgen_sha256,
        files,
        bytes,
    })
}

/// One seed's T30 in one receiver-band (section 4).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct T30 {
    /// The value, or the noise refusal's value from the series; `None` for any other refusal.
    pub t: Option<f64>,
    /// Its calibrated Monte-Carlo standard deviation, when it has one.
    pub mc_sd: Option<f64>,
    /// `value`, `monte_carlo_noise` or `noise_uncalibrated`, which are judged; anything else is
    /// the refusal's reason (`truncated`, `missing_moves`, ...) or its code, and is not.
    pub source: String,
}

/// One seed's T20 in one receiver-band, read from `parameters.t20_s` by exactly section 4's
/// rules for T30 (the T20 twin of gate C).
pub type T20 = T30;

/// T30's index in `params::noise::QUANTITY_NAMES` (T20's, 2, has the same limit).
const T30_INDEX: usize = 3;

/// The sources of a judged value.
pub const JUDGED_SOURCES: [&str; 3] = ["value", "monte_carlo_noise", "noise_uncalibrated"];

impl T30 {
    /// A value or a noise refusal, with its value (E6).
    pub fn judged(&self) -> bool {
        self.t.is_some() && JUDGED_SOURCES.contains(&self.source.as_str())
    }

    /// Reads `t30_s`.
    pub fn of(e: &Evaluated) -> T30 {
        match e {
            Evaluated::Value {
                value,
                mc_sd,
                status,
                refused_resamples,
                ..
            } => T30 {
                t: Some(*value),
                mc_sd: *mc_sd,
                // Shown `wide` where `params::noise::evaluate` refused it for its noise, or for its
                // resamples (shown from its stand-ins): read as that refusal. T20 and T30 share
                // their limit.
                source: match (status, mc_sd) {
                    (Some(RangeStatus::Wide), Some(_)) if refused_resamples.is_some() => {
                        "monte_carlo_noise"
                    }
                    (Some(RangeStatus::Wide), Some(sd))
                        if noise::over_limit(T30_INDEX, *value, *sd) =>
                    {
                        "monte_carlo_noise"
                    }
                    _ => "value",
                }
                .into(),
            },
            Evaluated::NotEvaluable { not_evaluable: r } => match &r.error {
                ParamError::NotEvaluable {
                    why: NotEvaluable::MonteCarloNoise { value, sd, .. },
                    ..
                } => T30 {
                    t: Some(*value),
                    mc_sd: *sd,
                    source: "monte_carlo_noise".into(),
                },
                ParamError::NotEvaluable {
                    why: NotEvaluable::NoiseUncalibrated { value, .. },
                    ..
                } => T30 {
                    t: Some(*value),
                    mc_sd: None,
                    source: "noise_uncalibrated".into(),
                },
                ParamError::NotEvaluable { why, .. } => T30 {
                    t: None,
                    mc_sd: None,
                    source: serde_json::to_value(why)
                        .ok()
                        .and_then(|v| v["why"].as_str().map(str::to_string))
                        .unwrap_or_else(|| r.code.clone()),
                },
                _ => T30 {
                    t: None,
                    mc_sd: None,
                    source: r.code.clone(),
                },
            },
        }
    }
}

/// One band of an SPPS run's analytic reference (`spps.reference.bands[b]`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct RefBand {
    pub freq_hz: i32,
    pub air_m_per_metre: Option<f64>,
    pub mean_absorption: f64,
    pub lambert_walls: bool,
    pub uniform_absorption: bool,
    /// Kuttruff's time and the standard deviation it inherits from `γ²`; or its refusal's code.
    pub kuttruff_s: Option<f64>,
    pub kuttruff_mc_sd: Option<f64>,
    pub kuttruff_refused: Option<String>,
    pub eyring_s: Option<f64>,
    pub eyring_refused: Option<String>,
}

/// An SPPS run's analytic reference (`spps.reference`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Reference {
    /// `None` when computed; why not, otherwise.
    pub not_computed: Option<String>,
    pub volume_m3: f64,
    pub area_m2: f64,
    pub speed_of_sound_m_s: f64,
    pub constant_s_per_m: f64,
    /// `γ²` and its standard error, `None` when the transport refused or did not run.
    pub gamma2: Option<f64>,
    pub gamma2_se: Option<f64>,
    pub mean_free_path_m: Option<f64>,
    pub bands: Vec<RefBand>,
}

impl Reference {
    fn of(r: &ReferenceReport) -> Reference {
        match r {
            ReferenceReport::Computed {
                volume_m3,
                area_m2,
                speed_of_sound_m_s,
                constant_s_per_m,
                free_paths,
                bands,
                ..
            } => Reference {
                not_computed: None,
                volume_m3: *volume_m3,
                area_m2: *area_m2,
                speed_of_sound_m_s: *speed_of_sound_m_s,
                constant_s_per_m: *constant_s_per_m,
                gamma2: free_paths.as_ref().map(|p| p.gamma2()),
                gamma2_se: free_paths.as_ref().map(|p| p.gamma2_se()),
                mean_free_path_m: free_paths.as_ref().map(|p| p.mean_free_path_m()),
                bands: bands
                    .iter()
                    .map(|b| RefBand {
                        freq_hz: b.freq_hz,
                        air_m_per_metre: b.air_m_per_metre,
                        mean_absorption: b.mean_absorption,
                        lambert_walls: b.lambert_walls,
                        uniform_absorption: b.uniform_absorption,
                        kuttruff_s: b.kuttruff_s.value(),
                        kuttruff_mc_sd: match &b.kuttruff_s {
                            Evaluated::Value { mc_sd, .. } => *mc_sd,
                            _ => None,
                        },
                        kuttruff_refused: b.kuttruff_s.refusal().map(|r| r.code.clone()),
                        eyring_s: b.eyring_s.value(),
                        eyring_refused: b.eyring_s.refusal().map(|r| r.code.clone()),
                    })
                    .collect(),
            },
            ReferenceReport::NotComputed { why } => Reference {
                not_computed: Some(why.clone()),
                volume_m3: f64::NAN,
                area_m2: f64::NAN,
                speed_of_sound_m_s: f64::NAN,
                constant_s_per_m: f64::NAN,
                gamma2: None,
                gamma2_se: None,
                mean_free_path_m: None,
                bands: Vec::new(),
            },
        }
    }
}

/// A decay curve: `[u, dB]` from `from_s` (`params::decay::DecayCurve`).
#[derive(Clone, Debug, PartialEq)]
pub struct Curve {
    pub from_s: f64,
    pub points: Vec<[f64; 2]>,
}

/// What the bed keeps of an SPPS run.
#[derive(Clone, Debug, PartialEq)]
pub struct SppsRead {
    pub info: RunInfo,
    pub bands_hz: Vec<i32>,
    pub particles_per_source: u32,
    pub computation_method: i32,
    /// `trans_epsilon` and the step as SPPS reads them (`f32`).
    pub trans_epsilon: f64,
    pub time_step_s: f64,
    /// `config.xml`'s `random_seed` as SPPS reads it (0 when absent); `None` when the config
    /// has no SPPS settings.
    pub random_seed: Option<i32>,
    /// `[receiver][band]`, receivers in the bed's order (`R000`, `R001`, ...).
    pub t30: Vec<Vec<T30>>,
    pub reference: Reference,
    /// `[receiver][band]`.
    pub curves: Vec<Vec<Option<Curve>>>,
    /// `[receiver][band]`: `parameters.t20_s`, by section 4's rules.
    pub t20: Vec<Vec<T20>>,
}

/// One band of a TCR run: TCR's own times and `core::params`' analytic ones (TCR's 0.163).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct TcrBand {
    pub freq_hz: i32,
    pub sabine_s: f64,
    pub eyring_s: f64,
    pub analytic_sabine_s: Option<f64>,
    pub analytic_eyring_s: Option<f64>,
    /// Why the analytic times are missing, when they are.
    pub analytic_refused: Option<String>,
    pub air_m_per_metre: Option<f64>,
}

/// What the bed keeps of a TCR run.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct TcrRead {
    pub info: RunInfo,
    pub bands: Vec<TcrBand>,
}

/// The run's report, as `simpa results --json` prints it, and its `config.xml`'s SPPS
/// `random_seed` (which the report does not carry); its refusal in words otherwise.
fn report_of(folder: &Path) -> Result<(Report, Option<i32>), String> {
    let refused = |r: crate::results::Refusal| {
        format!("simpa results refuses the run: {}: {}", r.code, r.detail)
    };
    let r = crate::results::load(folder).map_err(refused)?;
    let seed = r.expectation.spps.as_ref().map(|s| s.random_seed);
    let rep = crate::results::checked_report(&r).map_err(refused)?;
    Ok((rep, seed))
}

/// Reads an SPPS run whose point receivers are the bed's `receivers` (`R000`, ...); with `None`,
/// every point receiver in the report's order (upstream's validation project names its own).
pub fn read_spps(folder: &Path, receivers: Option<usize>) -> Result<SppsRead, String> {
    let info = run_info(folder)?;
    let (rep, random_seed) = report_of(folder)?;
    let s = rep
        .spps
        .as_ref()
        .ok_or_else(|| format!("{}: not an SPPS run", folder.display()))?;
    let picked: Vec<&crate::results::report::SppsReceiverReport> = match receivers {
        Some(n) => (0..n)
            .map(|i| {
                let name = super::file::receiver_name(i);
                s.point_receivers
                    .iter()
                    .find(|r| r.label == name)
                    .ok_or_else(|| format!("{}: no receiver {name}", folder.display()))
            })
            .collect::<Result<_, _>>()?,
        None => s.point_receivers.iter().collect(),
    };
    let mut t30 = Vec::with_capacity(picked.len());
    let mut t20 = Vec::with_capacity(picked.len());
    let mut curves = Vec::with_capacity(picked.len());
    for r in picked {
        let name = &r.label;
        if r.bands.len() != rep.bands_hz.len() {
            return Err(format!(
                "{}: receiver {name} has {} bands, the run {}",
                folder.display(),
                r.bands.len(),
                rep.bands_hz.len()
            ));
        }
        t30.push(
            r.bands
                .iter()
                .map(|b| T30::of(&b.parameters.t30_s))
                .collect(),
        );
        t20.push(
            r.bands
                .iter()
                .map(|b| T20::of(&b.parameters.t20_s))
                .collect(),
        );
        curves.push(
            r.bands
                .iter()
                .map(|b| {
                    b.decay_curve.as_ref().map(|c| Curve {
                        from_s: c.from_s,
                        points: c.points.clone(),
                    })
                })
                .collect(),
        );
    }
    Ok(SppsRead {
        info,
        bands_hz: rep.bands_hz.clone(),
        particles_per_source: s.particles_per_source,
        computation_method: s.computation_method,
        trans_epsilon: s.trans_epsilon,
        time_step_s: s.time_step_s,
        random_seed,
        t30,
        reference: Reference::of(&s.reference),
        curves,
        t20,
    })
}

/// Reads a TCR run.
pub fn read_tcr(folder: &Path) -> Result<TcrRead, String> {
    use crate::results::report::AnalyticReport;
    let info = run_info(folder)?;
    let (rep, _) = report_of(folder)?;
    let t = rep
        .tcr
        .as_ref()
        .ok_or_else(|| format!("{}: not a TCR run", folder.display()))?;
    let bands = t
        .bands
        .iter()
        .map(|b| {
            let (sab, eyr, air, refused) = match &t.analytic {
                AnalyticReport::Computed { bands, .. } => {
                    match bands.iter().find(|a| a.freq_hz == b.freq_hz) {
                        Some(a) => (
                            a.sabine_s.value(),
                            a.eyring_s.value(),
                            a.air_m_per_metre,
                            a.eyring_s
                                .refusal()
                                .or(a.sabine_s.refusal())
                                .map(|r| r.code.clone()),
                        ),
                        None => (None, None, None, Some("no analytic band".to_string())),
                    }
                }
                AnalyticReport::NotComputed { why } => (None, None, None, Some(why.clone())),
            };
            TcrBand {
                freq_hz: b.freq_hz,
                sabine_s: b.sabine.reverberation_time_s,
                eyring_s: b.eyring.reverberation_time_s,
                analytic_sabine_s: sab,
                analytic_eyring_s: eyr,
                analytic_refused: refused,
                air_m_per_metre: air,
            }
        })
        .collect();
    Ok(TcrRead { info, bands })
}

/// The solve folder of a run folder.
pub fn solve_dir(folder: &Path) -> PathBuf {
    folder.join(SOLVE_DIR)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::params::{ParticleCount, Quantity};
    use crate::results::report::Refused;

    fn refused(why: NotEvaluable) -> Evaluated {
        let e = ParamError::NotEvaluable {
            quantity: Quantity::T30,
            why,
        };
        Evaluated::NotEvaluable {
            not_evaluable: Refused {
                code: e.code().to_string(),
                message: e.to_string(),
                error: e,
            },
        }
    }

    #[test]
    fn every_seeds_value_is_read_refused_for_noise_or_not() {
        let v = T30::of(&Evaluated::bare(1.2, Some(0.01)));
        assert!(v.judged() && v.t == Some(1.2) && v.mc_sd == Some(0.01));
        let noise = T30::of(&refused(NotEvaluable::MonteCarloNoise {
            value: 1.3,
            sd: Some(0.05),
            limit: 0.025,
            resamples: 200,
            refused_resamples: 0,
            particle_count: ParticleCount::NoStandardDeviation,
        }));
        assert!(noise.judged());
        assert_eq!((noise.t, noise.mc_sd), (Some(1.3), Some(0.05)));
        assert_eq!(noise.source, "monte_carlo_noise");
        let uncal = T30::of(&refused(NotEvaluable::NoiseUncalibrated {
            value: 1.4,
            particles: 31_000,
            crossings_per_particle: 1.0,
            min_particles: 50_000,
            max_crossings_per_particle: 2.0,
            particles_at_least: Some(50_000),
            receiver_radius_scale_at_most: None,
        }));
        assert!(uncal.judged() && uncal.t == Some(1.4) && uncal.mc_sd.is_none());
        // Says no: any other refusal is not judged, and names its reason.
        let truncated = T30::of(&refused(NotEvaluable::Truncated {
            value: 1.0,
            with_tail: Some(1.1),
            limit: 0.01,
        }));
        assert!(!truncated.judged());
        assert_eq!(truncated.source, "truncated");
        assert_eq!(truncated.t, None);
    }

    fn refused_t20(why: NotEvaluable) -> Evaluated {
        let e = ParamError::NotEvaluable {
            quantity: Quantity::T20,
            why,
        };
        Evaluated::NotEvaluable {
            not_evaluable: Refused {
                code: e.code().to_string(),
                message: e.to_string(),
                error: e,
            },
        }
    }

    /// `parameters.t20_s` is read by section 4's rules, as `t30_s` is: a noise refusal keeps its
    /// value from the series; `range_not_reached` (or any other refusal) keeps none, and names
    /// its reason.
    #[test]
    fn t20_is_read_by_section_4s_rules() {
        let noise = T20::of(&refused_t20(NotEvaluable::MonteCarloNoise {
            value: 0.47,
            sd: Some(0.02),
            limit: 0.025,
            resamples: 200,
            refused_resamples: 0,
            particle_count: ParticleCount::NoStandardDeviation,
        }));
        assert!(noise.judged());
        assert_eq!((noise.t, noise.mc_sd), (Some(0.47), Some(0.02)));
        assert_eq!(noise.source, "monte_carlo_noise");
        let short = T20::of(&refused_t20(NotEvaluable::RangeNotReached {
            needed_db: -25.0,
            reached_db: -21.3,
        }));
        assert!(!short.judged());
        assert_eq!(short.t, None);
        assert_eq!(short.source, "range_not_reached");
    }

    /// Results version 7 shows a value refused for its standard deviation alone, `wide`, with the
    /// same value and `mc_sd` (decision-log row 37 (3)). The bed reads it exactly as it read the
    /// refusal: the same number, standard deviation and source, so its verdicts do not move. A
    /// value `params` gave stays `value`, `wide` or not.
    #[test]
    fn a_noise_refusal_shown_wide_is_read_as_the_refusal_it_was() {
        for (q, i, refused_as, read) in [
            (
                Quantity::T30,
                3,
                refused as fn(NotEvaluable) -> Evaluated,
                T30::of as fn(&Evaluated) -> T30,
            ),
            (Quantity::T20, 2, refused_t20, T20::of),
        ] {
            let why = NotEvaluable::MonteCarloNoise {
                value: 1.3,
                sd: Some(0.05),
                limit: 0.025,
                resamples: 200,
                refused_resamples: 0,
                particle_count: ParticleCount::NoStandardDeviation,
            };
            let before = read(&refused_as(why.clone()));
            let shown =
                Evaluated::of_parameter(i, Err(ParamError::NotEvaluable { quantity: q, why }));
            assert_eq!(shown.status(), Some(RangeStatus::Wide), "{q}");
            assert_eq!(read(&shown), before, "{q}");
            assert_eq!(before.source, "monte_carlo_noise");
            // A value params gave within its limit (2.2 % of it) is shown wide (2.5 · 2.2 % > 5 %)
            // and read as a value, as it was.
            let given = Evaluated::of_parameter(
                i,
                Ok(crate::params::noise::Estimate {
                    value: 1.0,
                    sd: 0.022,
                }),
            );
            assert_eq!(given.status(), Some(RangeStatus::Wide));
            let r = read(&given);
            assert_eq!(
                (r.t, r.mc_sd, r.source.as_str()),
                (Some(1.0), Some(0.022), "value")
            );
            // A refusal by its resamples is still a refusal, read as before.
            let resampled = NotEvaluable::MonteCarloNoise {
                value: 1.3,
                sd: Some(0.01),
                limit: 0.025,
                resamples: 200,
                refused_resamples: 40,
                particle_count: ParticleCount::NoStandardDeviation,
            };
            let shown = Evaluated::of_parameter(
                i,
                Err(ParamError::NotEvaluable {
                    quantity: q,
                    why: resampled.clone(),
                }),
            );
            assert!(shown.refusal().is_some());
            assert_eq!(read(&shown), read(&refused_as(resampled.clone())));
            // Shown from its stand-ins (results version 9): read as the same refusal, the same
            // value and judged standard deviation.
            let stand_in = Evaluated::of_parameter_with(
                i,
                Err(ParamError::NotEvaluable {
                    quantity: q,
                    why: resampled.clone(),
                }),
                &noise::Widen {
                    straddle: None,
                    stand_in: Some(noise::StandIn {
                        sd: 0.03,
                        refused_resamples: 40,
                    }),
                },
            );
            assert_eq!(stand_in.status(), Some(RangeStatus::Wide));
            assert_eq!(read(&stand_in), read(&refused_as(resampled)));
        }
    }
}
