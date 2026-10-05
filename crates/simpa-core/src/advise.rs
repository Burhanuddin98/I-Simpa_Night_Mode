//! The run-quality advisor (backlog 80, decisions 49, 54, 56 and 57;
//! `docs/investigations/2026-10-04-advisor/PLAN.md`).
//!
//! Before a run, [`before`] checks a project for what makes its values noisy or refused: meshing
//! that splits the walls, receivers small for the room's volume, a run shorter than the room's
//! decay, fewer particles than the noise model was calibrated at. After a run, [`after`] reads a
//! report and gives each refused, `wide` or lost-particle-warned value its cause and the setting
//! that addresses it. Each item names a **setting**, and the value Apply would set it to, never
//! a value the run will produce: the noise fell 9x and 15x where the square-root rule forecast
//! 3.2x (`B:\data\m12\b78-mesh\FINDINGS.md`), so nothing here extrapolates.
//!
//! These are quality judgements, not the solver contract: [`CODES`] is their own stable list, apart
//! from `validate::RULES`, and nothing here blocks a run or hides a value.
//!
//! **Every fix is a field of `SolverSettings`**, so Apply is one `Op::SetSolverSettings` with one
//! field changed ([`apply_op`]), through the app's checked apply: one undo step, refused by the
//! validator like any other edit, and refused here when the project's value is no longer the one
//! the advice was given for (the project changed since the run).
//!
//! The wording is for an acoustician, not a programmer, and holds no number: the numbers an item
//! carries are its `from` and `to`, each at a path of the report (`advice.<i>.fix.to`), which the
//! Results screen shows with that path (M12 gate (a)), and before a run the Simulate step shows
//! in its setting's own field.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::schema::{F64, Op, Project, SolverSettings};

mod after;
mod before;

pub use after::{Meshing, RunFacts, after};
pub use before::{before, before_with};

/// The advisor's codes, stable API: each [`Advice::code`] is one of these. Not reason codes of the
/// solver contract (`docs/solver-contract.md`): quality judgements with their own list, documented
/// in `docs/results.md`, "The run-quality advisor".
pub mod code {
    // ---- before a run ----
    /// `-Y` off: TetGen may split the room's boundary, and SPPS loses particles at the splits.
    pub const MESH_SPLITS_WALLS: &str = "mesh_splits_walls";
    /// Particles times the receiver's cross-section over the volume below [`super::RECEIVERS_SMALL_K`].
    pub const RECEIVERS_SMALL: &str = "receivers_small";
    /// The duration is shorter than the slowest computed band's 60 dB decay by Sabine.
    pub const RUN_SHORT: &str = "run_short";
    /// Fewer particles per source than the noise model was calibrated at for some quantity.
    pub const PARTICLES_FEW: &str = "particles_few";
    // ---- after a run ----
    /// `monte_carlo_noise`: the value's noise is above its limit.
    pub const MONTE_CARLO_NOISE: &str = "monte_carlo_noise";
    /// `range_below_zero`: the noise is so large the range crosses zero.
    pub const RANGE_BELOW_ZERO: &str = "range_below_zero";
    /// A `wide` value: shown with a range wider than the difference limen.
    pub const WIDE: &str = "wide";
    /// A `wide` C50, C80 or D50 whose bin at te could move it (`straddle`).
    pub const STRADDLE: &str = "straddle";
    /// `noise_uncalibrated`: the run is outside what the noise model was measured on.
    pub const NOISE_UNCALIBRATED: &str = "noise_uncalibrated";
    /// `noise_unknown`: the noise cannot be estimated.
    pub const NOISE_UNKNOWN: &str = "noise_unknown";
    /// `missing_moves` / `missing_not_cleared` from the solver's extinction floor.
    pub const SOLVER_FLOOR: &str = "solver_floor";
    /// `missing_moves` / `missing_not_cleared` from particles still alive when the run ended.
    pub const PARTICLES_LEFT_ALIVE: &str = "particles_left_alive";
    /// `lost_particles`: from 1 % of the band's particles lost (decision 56).
    pub const LOST_PARTICLES: &str = "lost_particles";
    /// A value shown with `lost_share_warning`: from 0.3 % lost (decision 56).
    pub const LOST_SHARE_WARNING: &str = "lost_share_warning";
    /// `truncated`, `range_not_reached`, EDT's `run_too_short` / `not_decaying_at_run_end`.
    pub const RUN_TOO_SHORT: &str = "run_too_short";
    /// `unresolved`, `early_unresolved`, EDT's `step_too_coarse`.
    pub const ONSET_TOO_COARSE: &str = "onset_too_coarse";
    /// `range_too_short`, `not_decaying`, `empty_window`: the decay cannot be fitted here.
    pub const DECAY_NOT_FITTED: &str = "decay_not_fitted";
    /// `several_sources`.
    pub const SEVERAL_SOURCES: &str = "several_sources";
    /// `no_time_series` (TCR).
    pub const NO_TIME_SERIES: &str = "no_time_series";
    /// EDT's `too_few_particles`.
    pub const EDT_TOO_FEW_PARTICLES: &str = "edt_too_few_particles";
    /// EDT's `receiver_too_large`.
    pub const EDT_RECEIVER_TOO_LARGE: &str = "edt_receiver_too_large";
    /// Any other EDT refusal: the histogram is outside the method's premises.
    pub const EDT_OUTSIDE_METHOD: &str = "edt_outside_method";
    /// `no_a_weight`, `band_missing`, `not_octave_bands`: the run's bands do not cover it.
    pub const BANDS_NOT_COVERED: &str = "bands_not_covered";
    /// `band_refused`: a band STI needs is itself refused.
    pub const BAND_REFUSED: &str = "band_refused";
    /// Any `ParamError` that is not a `NotEvaluable`: an input fault, no setting fixes it here.
    pub const INPUT_FAULT: &str = "input_fault";
}

/// Every advisor code, in the order items are listed.
pub const CODES: [&str; 25] = [
    code::MESH_SPLITS_WALLS,
    code::RECEIVERS_SMALL,
    code::RUN_SHORT,
    code::PARTICLES_FEW,
    code::RANGE_BELOW_ZERO,
    code::MONTE_CARLO_NOISE,
    code::WIDE,
    code::STRADDLE,
    code::NOISE_UNCALIBRATED,
    code::NOISE_UNKNOWN,
    code::LOST_PARTICLES,
    code::LOST_SHARE_WARNING,
    code::SOLVER_FLOOR,
    code::PARTICLES_LEFT_ALIVE,
    code::RUN_TOO_SHORT,
    code::ONSET_TOO_COARSE,
    code::DECAY_NOT_FITTED,
    code::SEVERAL_SOURCES,
    code::NO_TIME_SERIES,
    code::EDT_TOO_FEW_PARTICLES,
    code::EDT_RECEIVER_TOO_LARGE,
    code::EDT_OUTSIDE_METHOD,
    code::BANDS_NOT_COVERED,
    code::BAND_REFUSED,
    code::INPUT_FAULT,
];

/// `receivers_small`'s threshold `K` on `N·r²/V` (particles per source times the receiver radius
/// squared over the air's volume, m⁻¹): arm D's 13.88, the lowest of the measured Elmia arms whose
/// T30 had no range below zero (B 1.39: 9 of 36 below zero, median noise 34 % of the value; D
/// 13.88 and E 14.44: none, 3.7 % and 2.3 %; H 34.65: none, 1.2 %), rounded down so D itself
/// passes (`B:\data\m12\b78-mesh\FINDINGS.md`). Tutorial 2 as upstream ships it, 9.25, lies
/// between B and D and was not measured (the plan's arm I): it is warned because it is below every
/// measured arm that was good, not because a run at 9.25 was seen to fail.
pub const RECEIVERS_SMALL_K: f64 = 13.8;

/// The receiver radius above which EDT is unchecked (`params::edt::VALIDATED_MAX_RADIUS_M`,
/// decision 37): the advisor never proposes more.
pub const RADIUS_MAX_M: f64 = crate::params::edt::VALIDATED_MAX_RADIUS_M;

/// A proposed radius is rounded down to this step, m (Elmia's nearest receiver, 0.647 m from a
/// face, gives 0.6, the fixture's value; decision 49).
pub const RADIUS_STEP_M: f64 = 0.05;

/// The new-project duration, s (`SppsSettings::for_bands`, decision 36): what `run_short` and
/// `run_too_short` propose, unless the step limit allows less.
pub const NEW_PROJECT_DURATION_S: f64 = 10.0;

/// The finest time step the parameters are validated at, s (decision 11).
pub const FINEST_TIME_STEP_S: f64 = 0.001;

/// The new-project extinction exponent (decision 41): what `solver_floor` proposes.
pub const NEW_PROJECT_EXTINCTION: f64 = 7.0;

/// The room volumes, m³, the noise model was calibrated on (`noise::calibration::MEASURED_ON`,
/// "box rooms of 60 to 1,000 m3"): outside them a fix for noise is not checked.
pub const CALIBRATED_VOLUME_M3: (f64, f64) = (60.0, 1000.0);

/// A setting a fix changes: every one a field of `SolverSettings`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Setting {
    /// `spps.receiver_radius_m`.
    ReceiverRadius,
    /// `spps.particles_per_source`.
    ParticlesPerSource,
    /// `spps.duration_s`.
    Duration,
    /// `spps.time_step_s`.
    TimeStep,
    /// `spps.extinction_exponent`.
    ExtinctionExponent,
    /// `meshing.preserve_boundary` (TetGen's `-Y`).
    PreserveBoundary,
    /// `spps.echogram_per_source`.
    EchogramPerSource,
}

impl Setting {
    /// The project's JSON pointer to the setting.
    pub fn pointer(self) -> &'static str {
        match self {
            Setting::ReceiverRadius => "/solvers/spps/receiver_radius_m",
            Setting::ParticlesPerSource => "/solvers/spps/particles_per_source",
            Setting::Duration => "/solvers/spps/duration_s",
            Setting::TimeStep => "/solvers/spps/time_step_s",
            Setting::ExtinctionExponent => "/solvers/spps/extinction_exponent",
            Setting::PreserveBoundary => "/solvers/meshing/preserve_boundary",
            Setting::EchogramPerSource => "/solvers/spps/echogram_per_source",
        }
    }

    /// The setting's name as the Simulate step labels its field.
    pub fn label(self) -> &'static str {
        match self {
            Setting::ReceiverRadius => "Receiver radius",
            Setting::ParticlesPerSource => "Particles per source and band",
            Setting::Duration => "Duration",
            Setting::TimeStep => "Time step",
            Setting::ExtinctionExponent => "Particle extinction",
            Setting::PreserveBoundary => "Preserve walls when meshing (-Y)",
            Setting::EchogramPerSource => "Echogram per source",
        }
    }

    /// The setting's value in `s`.
    pub fn get(self, s: &SolverSettings) -> SettingValue {
        match self {
            Setting::ReceiverRadius => SettingValue::Number(s.spps.receiver_radius_m.get()),
            Setting::ParticlesPerSource => {
                SettingValue::Number(f64::from(s.spps.particles_per_source))
            }
            Setting::Duration => SettingValue::Number(s.spps.duration_s.get()),
            Setting::TimeStep => SettingValue::Number(s.spps.time_step_s.get()),
            Setting::ExtinctionExponent => SettingValue::Number(s.spps.extinction_exponent.get()),
            Setting::PreserveBoundary => SettingValue::Bool(s.meshing.preserve_boundary),
            Setting::EchogramPerSource => SettingValue::Bool(s.spps.echogram_per_source),
        }
    }

    /// `s` with the setting at `v`; refused when `v` is not of the setting's kind.
    pub fn set(self, s: &SolverSettings, v: SettingValue) -> Result<SolverSettings, String> {
        let mut out = s.clone();
        let number = |v: SettingValue| match v {
            SettingValue::Number(x) if x.is_finite() => Ok(x),
            _ => Err(format!("{} takes a number", self.label())),
        };
        match self {
            Setting::ReceiverRadius => out.spps.receiver_radius_m = F64::new(number(v)?),
            Setting::ParticlesPerSource => {
                let x = number(v)?;
                if x.fract() != 0.0 || !(1.0..=f64::from(u32::MAX)).contains(&x) {
                    return Err(format!("{} takes a whole count", self.label()));
                }
                out.spps.particles_per_source = x as u32;
            }
            Setting::Duration => out.spps.duration_s = F64::new(number(v)?),
            Setting::TimeStep => out.spps.time_step_s = F64::new(number(v)?),
            Setting::ExtinctionExponent => out.spps.extinction_exponent = F64::new(number(v)?),
            Setting::PreserveBoundary | Setting::EchogramPerSource => {
                let SettingValue::Bool(b) = v else {
                    return Err(format!("{} is on or off", self.label()));
                };
                if self == Setting::PreserveBoundary {
                    out.meshing.preserve_boundary = b;
                } else {
                    out.spps.echogram_per_source = b;
                }
            }
        }
        Ok(out)
    }
}

/// A setting's value: a number in the setting's own unit (m, s, a count), or on and off.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum SettingValue {
    Bool(bool),
    Number(f64),
}

/// Which bound set a proposed receiver radius: the smallest of three.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RadiusBound {
    /// Below the nearest receiver's distance to a face, rounded down to [`RADIUS_STEP_M`]: no
    /// sphere crosses a wall (`receiver_sphere_crosses_surface`).
    Clearance,
    /// [`RADIUS_MAX_M`]: EDT is unchecked above it.
    EdtChecked,
    /// The noise model's calibrated crossings per particle (`noise_uncalibrated` would refuse
    /// above it).
    Calibration,
}

/// What addresses an advice item's cause.
#[derive(Clone, Debug, PartialEq, Serialize, JsonSchema)]
pub struct Fix {
    /// The fix in plain words, naming the setting; no number.
    pub words: String,
    /// The setting it changes; `null` when no setting addresses the cause.
    pub setting: Option<Setting>,
    /// The setting's project pointer ([`Setting::pointer`]); `null` with `setting`.
    pub pointer: Option<String>,
    /// The setting's name as its field is labelled ([`Setting::label`]); `null` with `setting`.
    pub label: Option<String>,
    /// The setting's value now: the project's before a run, the run's after one; `null` when it
    /// is not known (the run's meshing, with no `mesh/mesh.json`).
    pub from: Option<SettingValue>,
    /// The value "Apply" sets; `null` when no Apply is offered (`why_no_apply` says why).
    pub to: Option<SettingValue>,
    /// Why no Apply is offered, in plain words; `null` when `to` is given.
    pub why_no_apply: Option<String>,
    /// For a radius: the bound that set `to`.
    pub bound: Option<RadiusBound>,
    /// A caveat on the fix, in plain words (the room is outside what the noise model was
    /// measured on); `null` when none.
    pub note: Option<String>,
}

impl Fix {
    /// A fix that changes `setting` from `from` to `to`.
    pub fn apply(words: &str, setting: Setting, from: SettingValue, to: SettingValue) -> Self {
        Fix {
            words: words.into(),
            setting: Some(setting),
            pointer: Some(setting.pointer().into()),
            label: Some(setting.label().into()),
            from: Some(from),
            to: Some(to),
            why_no_apply: None,
            bound: None,
            note: None,
        }
    }

    /// A fix that names `setting` but offers no Apply, with why.
    pub fn named(words: &str, setting: Setting, from: Option<SettingValue>, why: &str) -> Self {
        Fix {
            words: words.into(),
            setting: Some(setting),
            pointer: Some(setting.pointer().into()),
            label: Some(setting.label().into()),
            from,
            to: None,
            why_no_apply: Some(why.into()),
            bound: None,
            note: None,
        }
    }

    /// No setting addresses the cause.
    pub fn none(words: &str, why: &str) -> Self {
        Fix {
            words: words.into(),
            setting: None,
            pointer: None,
            label: None,
            from: None,
            to: None,
            why_no_apply: Some(why.into()),
            bound: None,
            note: None,
        }
    }

    /// Whether Apply is offered.
    pub fn applies(&self) -> bool {
        self.setting.is_some() && self.to.is_some()
    }
}

/// One advice item: a cause and its fix, with the values it explains.
#[derive(Clone, Debug, PartialEq, Serialize, JsonSchema)]
pub struct Advice {
    /// One of [`CODES`].
    pub code: String,
    /// The cause, in plain words; no number.
    pub cause: String,
    pub fix: Fix,
    /// After a run: the report's dot paths of the values it explains
    /// (`spps.point_receivers.0.bands.3.parameters.t30_s`), each once, in report order. Before a
    /// run: the project pointers it read.
    pub values: Vec<String>,
}

/// What "Apply" sends (the app's `advice_apply`): the setting, the value the advice was given
/// for, and the value it sets. It travels as JSON text, read with the core's exact reader
/// (`schema::from_json_exact`), as every edit does (PQ3).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ApplyArgs {
    pub setting: Setting,
    pub from: SettingValue,
    pub to: SettingValue,
}

/// Why [`apply_op`] refused.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ApplyRefusal {
    /// The advice offers no Apply.
    NoApply,
    /// The project's value is no longer the one the advice was given for.
    ProjectChanged { setting: Setting },
    /// The value is not of the setting's kind.
    BadValue(String),
}

impl ApplyRefusal {
    /// A stable code for the UI.
    pub fn code(&self) -> &'static str {
        match self {
            ApplyRefusal::NoApply => "ADVICE_NO_APPLY",
            ApplyRefusal::ProjectChanged { .. } => "ADVICE_PROJECT_CHANGED",
            ApplyRefusal::BadValue(_) => "ADVICE_BAD_VALUE",
        }
    }
}

impl std::fmt::Display for ApplyRefusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ApplyRefusal::NoApply => write!(f, "this advice offers no setting to apply"),
            ApplyRefusal::ProjectChanged { setting } => write!(
                f,
                "the project changed since this run: {} is no longer what the run used, so the \
                 advice may not apply; check the setting in the Simulate step",
                setting.label()
            ),
            ApplyRefusal::BadValue(why) => write!(f, "{why}"),
        }
    }
}

/// The edit that applies `fix` to `p`: `Op::SetSolverSettings`, the project's settings with the
/// one field changed. Refused when the fix offers no Apply, or when the project's value is not
/// `fix.from` (the project changed since the run the advice came from). The app sends the op
/// through its checked apply, so the validator still refuses an edit that introduces an error.
pub fn apply_op(
    p: &Project,
    setting: Setting,
    from: SettingValue,
    to: SettingValue,
) -> Result<Op, ApplyRefusal> {
    if !same(setting.get(&p.solvers), from) {
        return Err(ApplyRefusal::ProjectChanged { setting });
    }
    let settings = setting
        .set(&p.solvers, to)
        .map_err(ApplyRefusal::BadValue)?;
    Ok(Op::SetSolverSettings { settings })
}

/// Whether a project's value is the one a run used: numbers as the solver reads them, in `f32`
/// (a run reports a 0.31 m radius as 0.3100000023841858).
pub fn same(a: SettingValue, b: SettingValue) -> bool {
    match (a, b) {
        (SettingValue::Number(x), SettingValue::Number(y)) => x == y || (x as f32) == (y as f32),
        (SettingValue::Bool(x), SettingValue::Bool(y)) => x == y,
        _ => false,
    }
}

/// The largest radius below `clearance_m` on [`RADIUS_STEP_M`]'s grid (0.647 and 0.65 both give
/// 0.6), or `None` when that is not above 0.
pub fn radius_below(clearance_m: f64) -> Option<f64> {
    if !clearance_m.is_finite() {
        return None;
    }
    let steps = (clearance_m / RADIUS_STEP_M - 1e-9).ceil() - 1.0;
    (steps >= 1.0).then(|| steps / (1.0 / RADIUS_STEP_M))
}

/// The step count the solver computes for `duration` in steps of `dt`, in `f32` and `f64`, the
/// larger (`validate`'s `step_count_overflow` rule).
pub fn step_count(duration: f64, dt: f64) -> f64 {
    let steps32 = f64::from(((duration as f32) / (dt as f32)).ceil());
    (duration / dt).ceil().max(steps32)
}

/// The longest duration, on `dt`'s grid, whose step count stays below the 16-bit step counter.
pub fn longest_duration(dt: f64) -> Option<f64> {
    if !(dt.is_finite() && dt > 0.0) {
        return None;
    }
    let max = f64::from(crate::validate::MAX_TIME_STEPS);
    let mut n = max - 1.0;
    while n > 0.0 {
        let d = n * dt;
        if step_count(d, dt) < max {
            return Some(d);
        }
        n -= 1.0;
    }
    None
}

/// What `run_short` and `run_too_short` propose: the new-project duration when it is longer than
/// `now` and reaches `needed` (when one is known), else the longest the step limit allows; `None`
/// when that is not longer than `now`.
pub fn longer_duration(now: f64, dt: f64, needed: Option<f64>) -> Option<f64> {
    let cap = longest_duration(dt)?;
    let new = NEW_PROJECT_DURATION_S.min(cap);
    let to = if new > now && needed.is_none_or(|n| new >= n) {
        new
    } else {
        cap
    };
    (to > now).then_some(to)
}

#[cfg(test)]
mod tests;
