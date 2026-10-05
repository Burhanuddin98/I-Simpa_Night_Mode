//! The advisor after a run: each refused, `wide` or lost-particle-warned value of a report, its
//! cause and the setting that addresses it (PLAN.md, "The refusal table", adapted to decision 56:
//! lost particles are a warning from 0.3 % and a refusal from 1 %, no longer the
//! `ENERGETIC_LOST_ENERGY_RATIO` bound behind `missing_moves`).
//!
//! The mapping is an exhaustive `match` on `ParamError` and `NotEvaluable`, with no `_` arm: a new
//! refusal kind does not compile until it has advice. EDT's reasons are strings
//! (`edt::REFUSAL_REASONS`); a test holds every one of them to its own advice.
//!
//! Items are grouped by cause and fix: one item per distinct (code, cause, fix), carrying every
//! value it explains, so the Results screen shows each cause once with a count.

use std::path::Path;

use super::{
    Advice, FINEST_TIME_STEP_S, Fix, NEW_PROJECT_EXTINCTION, RADIUS_MAX_M, RADIUS_STEP_M,
    RadiusBound, Setting, SettingValue, code, longer_duration, radius_below, step_count,
};
use crate::params::noise::{self, Method, RangeStatus, Walls};
use crate::params::{NotEvaluable, ParamError, ParticleCount};
use crate::results::RunResults;
use crate::results::report::{Evaluated, Parameters, Report, RoomReport, SppsReport};

/// How the run was meshed, from `<run>/mesh/mesh.json`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Meshing {
    /// TetGen's argv was read: whether it had `-Y`, and whether a surface-receiver refinement
    /// (the `.var`) was meshed with it.
    Known {
        preserve_boundary: bool,
        refinement: bool,
    },
    /// No mesh record in the run (a reused or fixture mesh).
    Unknown,
}

/// What the advice needs from a run beyond its report.
#[derive(Clone, Debug, PartialEq)]
pub struct RunFacts {
    /// The smallest distance from a point receiver's centre to a face of the run's own `.cbin`,
    /// m; `None` when it cannot be read or a receiver is not strictly inside.
    pub min_clearance_m: Option<f64>,
    pub meshing: Meshing,
}

impl RunFacts {
    /// Read from the run: the clearances from its `.cbin` against `report`'s receiver positions,
    /// the meshing from `mesh/mesh.json`.
    pub fn read(r: &RunResults, report: &Report) -> Self {
        let positions: Option<Vec<[f64; 3]>> = report
            .spps
            .as_ref()
            .map(|s| s.point_receivers.iter().map(|p| p.position_m).collect())
            .unwrap_or(None);
        let min_clearance_m = positions.and_then(|ps| {
            let tri = crate::results::tcr::scene_triangles(&r.solve_dir(), &r.expectation).ok()?;
            min_clearance(&tri, &ps)
        });
        RunFacts {
            min_clearance_m,
            meshing: read_meshing(&r.folder),
        }
    }
}

/// The smallest clearance of `points` from `triangles`; `None` when one is not strictly inside.
pub fn min_clearance(triangles: &[[[f64; 3]; 3]], points: &[[f64; 3]]) -> Option<f64> {
    let mut min: Option<f64> = None;
    for p in points {
        match crate::validate::locate_in(triangles, *p) {
            crate::validate::PointLocation::Inside { clearance_m } => {
                min = Some(min.map_or(clearance_m, |m| m.min(clearance_m)));
            }
            _ => return None,
        }
    }
    min
}

/// `<run>/mesh/mesh.json`'s TetGen argv and `.var` file; [`Meshing::Unknown`] without one.
pub fn read_meshing(run: &Path) -> Meshing {
    let path = run.join(crate::run::manager::MESH_DIR).join("mesh.json");
    let Ok(text) = std::fs::read_to_string(path) else {
        return Meshing::Unknown;
    };
    let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) else {
        return Meshing::Unknown;
    };
    let Some(argv) = v.pointer("/tetgen/argv").and_then(|a| a.as_array()) else {
        return Meshing::Unknown;
    };
    let preserve_boundary = argv.iter().any(|a| a.as_str() == Some("-Y"));
    let refinement = v.pointer("/files/var").is_some_and(|x| !x.is_null());
    Meshing::Known {
        preserve_boundary,
        refinement,
    }
}

/// The run's settings the advice reads and proposes from (the report's `spps`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct RunSettings {
    pub radius_m: f64,
    pub particles: f64,
    pub duration_s: f64,
    pub time_step_s: f64,
    pub trans_epsilon: f64,
    pub echogram_per_source: bool,
}

impl RunSettings {
    fn of(s: &SppsReport) -> Self {
        RunSettings {
            radius_m: s.receiver_radius_m,
            particles: f64::from(s.particles_per_source),
            duration_s: s.duration_s,
            time_step_s: s.time_step_s,
            trans_epsilon: s.trans_epsilon,
            echogram_per_source: s.echogram_per_source,
        }
    }
}

/// The run's settings and bounds every value's advice reads.
pub(super) struct Ctx<'a> {
    /// `None` for TCR.
    pub settings: Option<RunSettings>,
    pub facts: &'a RunFacts,
    /// The largest radius proposed, and the bound that set it; `None` when the clearance is not
    /// known (a radius could then cross a wall).
    pub radius_cap: Option<(f64, RadiusBound)>,
    /// The room's air volume, m3, when the report computed it.
    pub volume_m3: Option<f64>,
}

impl Ctx<'_> {
    fn number(&self, f: impl Fn(&RunSettings) -> f64) -> Option<f64> {
        self.settings.as_ref().map(f)
    }
}

/// The calibration's bound on the radius: crossings per particle grow as its square, so `r` times
/// `√(max_n / n)`, the smallest calibrated `max_n` under the run's method over the largest `n` of
/// any band of any receiver. `None` when no band has an `n`.
fn calibration_radius(s: &SppsReport) -> Option<f64> {
    let method = match s.computation_method {
        0 => Method::Random,
        _ => Method::Energetic,
    };
    let max_n = (0..noise::QUANTITY_NAMES.len())
        .flat_map(|i| {
            [Walls::Other, Walls::Lambert, Walls::UniformLambert]
                .map(|w| noise::calibration::entry(method, i, w).max_crossings_per_particle)
        })
        .fold(f64::INFINITY, f64::min);
    let n = s
        .point_receivers
        .iter()
        .flat_map(|r| r.bands.iter().filter_map(|b| b.crossings_per_particle))
        .filter(|n| n.is_finite() && *n > 0.0)
        .fold(None, |m: Option<f64>, n| Some(m.map_or(n, |m| m.max(n))))?;
    Some(s.receiver_radius_m * (max_n / n).sqrt())
}

pub(super) fn radius_cap(s: Option<&SppsReport>, facts: &RunFacts) -> Option<(f64, RadiusBound)> {
    let s = s?;
    let clearance = radius_below(facts.min_clearance_m?)?;
    let mut cap = (clearance, RadiusBound::Clearance);
    if RADIUS_MAX_M < cap.0 {
        cap = (RADIUS_MAX_M, RadiusBound::EdtChecked);
    }
    if let Some(c) = calibration_radius(s) {
        let c = (c / RADIUS_STEP_M + 1e-9).floor() * RADIUS_STEP_M;
        if c < cap.0 {
            cap = (c, RadiusBound::Calibration);
        }
    }
    Some(cap)
}

/// The advice for `report`'s values, grouped by cause and fix, in [`super::CODES`]' order.
pub fn after(report: &Report, facts: &RunFacts) -> Vec<Advice> {
    let spps = report.spps.as_ref();
    let ctx = Ctx {
        settings: spps.map(RunSettings::of),
        facts,
        radius_cap: radius_cap(spps, facts),
        volume_m3: match &report.room {
            RoomReport::Computed { volume_m3, .. } => Some(*volume_m3),
            RoomReport::NotComputed { .. } => None,
        },
    };
    let mut out: Vec<Advice> = Vec::new();
    for (path, e) in values(report) {
        for (code, cause, fix) in advise_value(&ctx, e) {
            match out
                .iter_mut()
                .find(|a| a.code == code && a.cause == cause && a.fix == fix)
            {
                Some(a) => {
                    if !a.values.contains(&path) {
                        a.values.push(path.clone());
                    }
                }
                None => out.push(Advice {
                    code: code.into(),
                    cause,
                    fix,
                    values: vec![path.clone()],
                }),
            }
        }
    }
    let rank = |c: &str| {
        super::CODES
            .iter()
            .position(|x| *x == c)
            .unwrap_or(usize::MAX)
    };
    out.sort_by_key(|a| rank(&a.code));
    out
}

fn push_params<'a>(out: &mut Vec<(String, &'a Evaluated)>, base: &str, p: &'a Parameters) {
    for (name, e) in p.named() {
        out.push((format!("{base}.parameters.{name}"), e));
    }
}

/// Every value the Results screen can show, with its dot path, in report order: per point
/// receiver its bands' eight parameters and G, its bands summed and their dB(A), its STI, and
/// the same per source.
pub fn values(report: &Report) -> Vec<(String, &Evaluated)> {
    let mut out = Vec::new();
    if let Some(s) = &report.spps {
        for (i, r) in s.point_receivers.iter().enumerate() {
            let rx = format!("spps.point_receivers.{i}");
            for (j, b) in r.bands.iter().enumerate() {
                let base = format!("{rx}.bands.{j}");
                push_params(&mut out, &base, &b.parameters);
                out.push((format!("{base}.g_db"), &b.g_db));
            }
            push_params(
                &mut out,
                &format!("{rx}.aggregate"),
                &r.aggregate.parameters,
            );
            out.push((
                format!("{rx}.aggregate.dba.level_db"),
                &r.aggregate.dba.level_db,
            ));
            out.push((format!("{rx}.sti.male"), &r.sti.male));
            for (k, ps) in r.per_source.iter().enumerate() {
                let src = format!("{rx}.per_source.{k}");
                for (j, b) in ps.bands.iter().enumerate() {
                    let base = format!("{src}.bands.{j}");
                    push_params(&mut out, &base, &b.parameters);
                    out.push((format!("{base}.g_db"), &b.g_db));
                }
                push_params(
                    &mut out,
                    &format!("{src}.aggregate"),
                    &ps.aggregate.parameters,
                );
            }
        }
    }
    if let Some(t) = &report.tcr {
        for (i, r) in t.point_receivers.iter().enumerate() {
            let rx = format!("tcr.point_receivers.{i}");
            for (j, b) in r.bands.iter().enumerate() {
                push_params(&mut out, &format!("{rx}.bands.{j}"), &b.parameters);
            }
        }
    }
    out
}

pub(super) type Item = (&'static str, String, Fix);

fn item(code: &'static str, cause: &str, fix: Fix) -> Item {
    (code, cause.to_string(), fix)
}

/// The advice for one value: none for an `ok` value without a lost-particle warning.
pub(super) fn advise_value(ctx: &Ctx, e: &Evaluated) -> Vec<Item> {
    match e {
        Evaluated::Value {
            status,
            straddle,
            lost_share_warning,
            ..
        } => {
            let mut out = Vec::new();
            if lost_share_warning.is_some() {
                out.push(item(
                    code::LOST_SHARE_WARNING,
                    "Some particles were lost through the mesh in this band: the value is \
                     shown, but its late decay may hold slightly too little energy.",
                    lost_fix(ctx),
                ));
            }
            if *status == Some(RangeStatus::Wide) {
                if straddle.is_some() {
                    out.push(item(
                        code::STRADDLE,
                        "The time bin at the early/late limit could move this value: it is shown \
                         with a wide range.",
                        time_step_fix(ctx),
                    ));
                } else {
                    out.push(item(
                        code::WIDE,
                        "Monte-Carlo noise: the value's range is wider than a listener could \
                         tell apart, so it is shown with its range.",
                        noise_fix(ctx, None),
                    ));
                }
            }
            out
        }
        Evaluated::NotEvaluable { not_evaluable } => refused(ctx, &not_evaluable.error),
    }
}

fn refused(ctx: &Ctx, e: &ParamError) -> Vec<Item> {
    let input = || {
        vec![item(
            code::INPUT_FAULT,
            "The value cannot be computed from this run's inputs.",
            Fix::none(
                "No run setting addresses this; the message beside the value names the input.",
                "no setting of the run addresses it.",
            ),
        )]
    };
    match e {
        ParamError::NotEvaluable { why, .. } => not_evaluable(ctx, why),
        ParamError::SeriesTooShort { .. } => vec![item(
            code::RUN_TOO_SHORT,
            "The run ended before the decay did: the response is too short for this value.",
            duration_fix(ctx),
        )],
        ParamError::BadTimeStep { .. }
        | ParamError::BadEnergy { .. }
        | ParamError::NoEnergy
        | ParamError::BadArrival { .. }
        | ParamError::SeriesMismatch { .. }
        | ParamError::BadAir { .. }
        | ParamError::BadRoom { .. }
        | ParamError::NoAbsorption
        | ParamError::DinOutOfRange { .. }
        | ParamError::BadNoiseInput { .. }
        | ParamError::TransportRefused { .. }
        | ParamError::ReferenceNotApplicable { .. } => input(),
    }
}

fn not_evaluable(ctx: &Ctx, why: &NotEvaluable) -> Vec<Item> {
    match why {
        NotEvaluable::MonteCarloNoise { particle_count, .. } => vec![item(
            code::MONTE_CARLO_NOISE,
            "Monte-Carlo noise: too few particle crossings of this receiver for the value to \
             be within its limit.",
            noise_fix(ctx, Some(particle_count)),
        )],
        NotEvaluable::RangeBelowZero { .. } => vec![item(
            code::RANGE_BELOW_ZERO,
            "Monte-Carlo noise: the noise is so large that the value's range reaches below \
             zero, so the value means nothing.",
            noise_fix(ctx, None),
        )],
        NotEvaluable::NoiseUncalibrated {
            particles_at_least,
            receiver_radius_scale_at_most,
            ..
        } => {
            let cause = "The run is outside what the noise model was measured on, so its noise \
                         cannot be judged.";
            let mut out = Vec::new();
            if let Some(n) = particles_at_least
                && let Some(now) = ctx.number(|s| s.particles)
            {
                out.push(item(
                    code::NOISE_UNCALIBRATED,
                    cause,
                    Fix::apply(
                        "Use the fewest particles the noise model was measured with (Particles \
                         per source and band): a calibration minimum, not a forecast.",
                        Setting::ParticlesPerSource,
                        SettingValue::Number(now),
                        SettingValue::Number(f64::from(*n)),
                    ),
                ));
            }
            if let Some(scale) = receiver_radius_scale_at_most
                && let Some(r) = ctx.number(|s| s.radius_m)
            {
                let to = (scale * r * 100.0).floor() / 100.0;
                let fix = if to > 0.0 && to < r {
                    let mut f = Fix::apply(
                        "Use smaller receivers (Receiver radius): each particle crosses them too \
                         often for the noise model.",
                        Setting::ReceiverRadius,
                        SettingValue::Number(r),
                        SettingValue::Number(to),
                    );
                    f.bound = Some(RadiusBound::Calibration);
                    f
                } else {
                    Fix::named(
                        "Use smaller receivers (Receiver radius).",
                        Setting::ReceiverRadius,
                        Some(SettingValue::Number(r)),
                        "the radius the noise model allows rounds to nothing.",
                    )
                };
                out.push(item(code::NOISE_UNCALIBRATED, cause, fix));
            }
            if out.is_empty() {
                out.push(item(
                    code::NOISE_UNCALIBRATED,
                    cause,
                    Fix::none("No setting is named.", "the refusal names no setting."),
                ));
            }
            out
        }
        NotEvaluable::NoiseUnknown { .. } => vec![item(
            code::NOISE_UNKNOWN,
            "The value's Monte-Carlo noise cannot be estimated, so nothing bounds it.",
            Fix::none(
                "No setting is known to address this.",
                "the noise cannot be estimated from this series.",
            ),
        )],
        NotEvaluable::MissingNotCleared { floor_db, .. }
        | NotEvaluable::MissingMoves { floor_db, .. } => {
            if floor_db.is_some() {
                vec![item(
                    code::SOLVER_FLOOR,
                    "The solver dropped particles once their energy fell to its extinction \
                     floor, and the energy they would have brought can move this value.",
                    extinction_fix(ctx),
                )]
            } else {
                vec![item(
                    code::PARTICLES_LEFT_ALIVE,
                    "Particles were still alive when the run ended, and the energy they would \
                     have brought can move this value.",
                    duration_fix(ctx),
                )]
            }
        }
        NotEvaluable::LostParticles { .. } => vec![item(
            code::LOST_PARTICLES,
            "Too many particles were lost through the mesh in this band: the model likely has \
             holes, or the meshing split the walls, so the value is refused.",
            lost_fix(ctx),
        )],
        NotEvaluable::RangeNotReached { .. } | NotEvaluable::Truncated { .. } => vec![item(
            code::RUN_TOO_SHORT,
            "The run ended before the decay did.",
            duration_fix(ctx),
        )],
        NotEvaluable::Unresolved { .. } | NotEvaluable::EarlyUnresolved { .. } => vec![item(
            code::ONSET_TOO_COARSE,
            "The time step is too coarse to place the sound's arrival in it.",
            time_step_fix(ctx),
        )],
        NotEvaluable::RangeTooShort { .. }
        | NotEvaluable::NotDecaying
        | NotEvaluable::EmptyWindow { .. } => vec![item(
            code::DECAY_NOT_FITTED,
            "The decay cannot be fitted at this receiver in this band.",
            Fix::none(
                "No setting is known to address this.",
                "the decay itself does not allow the value here.",
            ),
        )],
        NotEvaluable::SeveralSources { .. } => vec![item(
            code::SEVERAL_SOURCES,
            "Several sources are summed here; the standard defines this value per source and \
             receiver.",
            several_sources_fix(ctx),
        )],
        NotEvaluable::NoTimeSeries { .. } => vec![item(
            code::NO_TIME_SERIES,
            "This solver writes no energy time series, so there is nothing to compute the value \
             from.",
            Fix::none(
                "Use SPPS, the particle solver, for this value.",
                "the solver is a choice, not a setting.",
            ),
        )],
        NotEvaluable::EdtRefused { reason } => edt_refused(ctx, reason),
        NotEvaluable::NoAWeight { .. }
        | NotEvaluable::BandMissing { .. }
        | NotEvaluable::NotOctaveBands { .. } => vec![item(
            code::BANDS_NOT_COVERED,
            "The run's bands do not cover this value.",
            Fix::none(
                "Change the bands to the new-project octaves (Change bands… in the Simulate step).",
                "changing the bands remaps every per-band value, so it is not applied from here.",
            ),
        )],
        NotEvaluable::BandRefused { .. } => vec![item(
            code::BAND_REFUSED,
            "A band this value needs is itself refused.",
            Fix::none(
                "See that band's own values and their advice.",
                "the band's own advice applies.",
            ),
        )],
    }
}

/// EDT v2.1's refusal reasons ([`crate::params::edt::REFUSAL_REASONS`]) to their advice.
fn edt_refused(ctx: &Ctx, reason: &str) -> Vec<Item> {
    match reason {
        "run_too_short" | "not_decaying_at_run_end" => vec![item(
            code::RUN_TOO_SHORT,
            "The run ended before the decay did.",
            duration_fix(ctx),
        )],
        "step_too_coarse" => vec![item(
            code::ONSET_TOO_COARSE,
            "The time step is too coarse for the early decay.",
            time_step_fix(ctx),
        )],
        "too_few_particles" => vec![item(
            code::EDT_TOO_FEW_PARTICLES,
            "Too few particles reached this receiver for the early decay.",
            Fix::named(
                "Use more particles (Particles per source and band), or larger receivers.",
                Setting::ParticlesPerSource,
                ctx.number(|s| s.particles).map(SettingValue::Number),
                "the EDT method names no particle count.",
            ),
        )],
        "receiver_too_large" => {
            let r = ctx.number(|s| s.radius_m);
            let to = ctx
                .radius_cap
                .map(|(c, _)| c.min(RADIUS_MAX_M))
                .unwrap_or(RADIUS_MAX_M);
            let fix = match r {
                Some(r) if to < r => {
                    let mut f = Fix::apply(
                        "Use smaller receivers (Receiver radius): EDT is checked up to a metre.",
                        Setting::ReceiverRadius,
                        SettingValue::Number(r),
                        SettingValue::Number(to),
                    );
                    f.bound = Some(RadiusBound::EdtChecked);
                    f
                }
                _ => Fix::named(
                    "Use smaller receivers (Receiver radius).",
                    Setting::ReceiverRadius,
                    r.map(SettingValue::Number),
                    "the radius is already within what EDT is checked for.",
                ),
            };
            vec![item(
                code::EDT_RECEIVER_TOO_LARGE,
                "The receivers are larger than EDT is checked for.",
                fix,
            )]
        }
        _ => vec![item(
            code::EDT_OUTSIDE_METHOD,
            "The response is outside what the EDT method reads (no energy after the arrival, \
             the direct sound alone, or no decay).",
            Fix::none(
                "No setting is known to address this.",
                "the response itself does not allow EDT here.",
            ),
        )],
    }
}

/// Monte-Carlo noise: a larger receiver up to the cap first, else the count the noise model
/// names (Q1: a setting, not a forecast), else particles named with no count.
fn noise_fix(ctx: &Ctx, count: Option<&ParticleCount>) -> Fix {
    let r = ctx.number(|s| s.radius_m);
    let note = ctx.volume_m3.and_then(super::before::volume_note);
    if let (Some(r), Some((cap, bound))) = (r, ctx.radius_cap)
        && cap > r + 1e-9
    {
        let mut f = Fix::apply(
            "Use larger receivers (Receiver radius): more particles cross them. The radius \
             proposed keeps every receiver's sphere inside the walls.",
            Setting::ReceiverRadius,
            SettingValue::Number(r),
            SettingValue::Number(cap),
        );
        f.bound = Some(bound);
        f.note = note;
        return f;
    }
    let now = ctx.number(|s| s.particles);
    let named = match count {
        Some(
            ParticleCount::Named { particles, .. } | ParticleCount::Resampled { particles, .. },
        ) => *particles,
        Some(
            ParticleCount::BeyondResampled { .. }
            | ParticleCount::ResampledNotConfirmed
            | ParticleCount::ScalingNotConfirmed
            | ParticleCount::NoStandardDeviation,
        )
        | None => None,
    };
    let mut f = match (now, named) {
        (Some(now), Some(n)) if (n as f64) > now => Fix::apply(
            "Use more particles (Particles per source and band): the count the noise model \
             names, a setting, not a forecast.",
            Setting::ParticlesPerSource,
            SettingValue::Number(now),
            SettingValue::Number(n as f64),
        ),
        _ => Fix::named(
            "Use more particles (Particles per source and band).",
            Setting::ParticlesPerSource,
            now.map(SettingValue::Number),
            if ctx.radius_cap.is_none() {
                "the receivers' distance to the walls is not known, so no larger radius is \
                 proposed, and the noise model names no particle count here."
            } else {
                "the receivers cannot grow without crossing a wall, and the noise model names no \
                 particle count here: more particles may not help."
            },
        ),
    };
    f.note = note;
    f
}

/// Lost particles (decision 56): `-Y` when the run was meshed without it; otherwise no setting,
/// and never more particles (the lost share does not fall with more).
fn lost_fix(ctx: &Ctx) -> Fix {
    let words = "Turn on Preserve walls when meshing (-Y), and check the model for holes, gaps \
                 or overlapping faces.";
    match ctx.facts.meshing {
        Meshing::Known {
            preserve_boundary: false,
            refinement: false,
        } => Fix::apply(
            words,
            Setting::PreserveBoundary,
            SettingValue::Bool(false),
            SettingValue::Bool(true),
        ),
        Meshing::Known {
            preserve_boundary: false,
            refinement: true,
        } => Fix::named(
            words,
            Setting::PreserveBoundary,
            Some(SettingValue::Bool(false)),
            "a surface receiver was refined when meshing, which splits its faces; -Y forbids \
             that, so the two cannot be on together.",
        ),
        Meshing::Known {
            preserve_boundary: true,
            ..
        } => Fix::none(
            "Check the model for holes, gaps or overlapping faces.",
            "the walls were already preserved when meshing, and more particles do not lower the \
             share lost: no setting addresses it.",
        ),
        Meshing::Unknown => Fix::named(
            words,
            Setting::PreserveBoundary,
            None,
            "how this run was meshed is not known (it has no mesh record).",
        ),
    }
}

/// A longer run: the new-project duration, or what the step limit allows.
fn duration_fix(ctx: &Ctx) -> Fix {
    let Some((now, dt)) = ctx.settings.map(|s| (s.duration_s, s.time_step_s)) else {
        return Fix::none("Run longer.", "this solver has no duration setting.");
    };
    match longer_duration(now, dt, None) {
        Some(to) => Fix::apply(
            "Run longer (Duration).",
            Setting::Duration,
            SettingValue::Number(now),
            SettingValue::Number(to),
        ),
        None => Fix::named(
            "Run longer (Duration).",
            Setting::Duration,
            Some(SettingValue::Number(now)),
            "the run is already as long as the solver's step counter allows at this time step.",
        ),
    }
}

/// A finer time step, down to 1 ms (decision 11), when the duration still fits the step counter.
fn time_step_fix(ctx: &Ctx) -> Fix {
    let Some((dur, dt)) = ctx.settings.map(|s| (s.duration_s, s.time_step_s)) else {
        return Fix::none("Use a finer time step.", "this solver has no time step.");
    };
    let words = "Use a finer time step (Time step).";
    // The run's step is the solver's `f32`: 1 ms reads 0.0010000000474974513.
    if (dt as f32) <= (FINEST_TIME_STEP_S as f32) {
        return Fix::named(
            words,
            Setting::TimeStep,
            Some(SettingValue::Number(dt)),
            "the time step is already the finest the values are checked at.",
        );
    }
    if step_count(dur, FINEST_TIME_STEP_S) >= f64::from(crate::validate::MAX_TIME_STEPS) {
        return Fix::named(
            words,
            Setting::TimeStep,
            Some(SettingValue::Number(dt)),
            "a finer step would pass the solver's step counter at this duration.",
        );
    }
    Fix::apply(
        words,
        Setting::TimeStep,
        SettingValue::Number(dt),
        SettingValue::Number(FINEST_TIME_STEP_S),
    )
}

/// The extinction floor: up to the new-project exponent (decision 41).
fn extinction_fix(ctx: &Ctx) -> Fix {
    let words = "Let particles live longer (Particle extinction).";
    let Some(eps) = ctx.number(|s| s.trans_epsilon) else {
        return Fix::none(words, "this solver has no extinction setting.");
    };
    if eps < NEW_PROJECT_EXTINCTION {
        Fix::apply(
            words,
            Setting::ExtinctionExponent,
            SettingValue::Number(eps),
            SettingValue::Number(NEW_PROJECT_EXTINCTION),
        )
    } else {
        Fix::named(
            words,
            Setting::ExtinctionExponent,
            Some(SettingValue::Number(eps)),
            "the extinction is already the new-project value; nothing longer is proposed.",
        )
    }
}

/// Several sources summed: echogram per source on, or, when it is on, a choice of source.
fn several_sources_fix(ctx: &Ctx) -> Fix {
    match ctx.settings.map(|s| s.echogram_per_source) {
        Some(false) => Fix::apply(
            "Turn on Echogram per source: each source's values are then computed.",
            Setting::EchogramPerSource,
            SettingValue::Bool(false),
            SettingValue::Bool(true),
        ),
        Some(true) => Fix::none(
            "Choose a source in the Source picker: each source's values are there.",
            "this view sums the sources; choosing one is a view, not a setting.",
        ),
        None => Fix::none(
            "Use SPPS with Echogram per source on.",
            "this solver computes no per-source values.",
        ),
    }
}
