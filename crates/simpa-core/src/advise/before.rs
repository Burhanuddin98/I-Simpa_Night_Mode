//! The advisor before a run: four checks on a project (PLAN.md, "Pre-run rules"). None blocks
//! the run; each names a setting and offers the value Apply sets.

use super::{
    Advice, CALIBRATED_VOLUME_M3, Fix, RADIUS_MAX_M, RECEIVERS_SMALL_K, RadiusBound, Setting,
    SettingValue, code, longer_duration, radius_below,
};
use crate::geometry::check::{self, Verdict};
use crate::params::noise::{self, Method, Walls};
use crate::params::room::{self, RtConstant, Surface};
use crate::schema::{ComputationMethod, Project};
use crate::validate::{PointLocation, locate_point};

/// [`before_with`], the air's volume from the geometry check (`air_volume_m3`), `None` when the
/// check refuses the model or it has no faces.
pub fn before(p: &Project) -> Vec<Advice> {
    let volume = (!p.geometry.faces.is_empty())
        .then(|| check::check(&p.geometry))
        .filter(|r| r.verdict == Verdict::Ok)
        .map(|r| r.measures.air_volume_m3);
    before_with(p, volume)
}

/// The checks, in [`super::CODES`]' order, on `p` with the air's volume `air_volume_m3` (the
/// check's; the app passes its cached one). A check that needs the volume is skipped without it,
/// never guessed.
pub fn before_with(p: &Project, air_volume_m3: Option<f64>) -> Vec<Advice> {
    let volume = air_volume_m3.filter(|v| v.is_finite() && *v > 0.0);
    let mut out = Vec::new();
    out.extend(mesh_splits_walls(p));
    out.extend(volume.and_then(|v| receivers_small(p, v)));
    out.extend(volume.and_then(|v| run_short(p, v)));
    out.extend(particles_few(p));
    out
}

/// B1: `-Y` off. Q3: with a surface-receiver refinement set, `-Y` cannot be on (the validator's
/// `mesh_settings_conflict`), so the conflict is named and no Apply is offered.
fn mesh_splits_walls(p: &Project) -> Option<Advice> {
    let m = &p.solvers.meshing;
    if m.preserve_boundary {
        return None;
    }
    let setting = Setting::PreserveBoundary;
    let words =
        "Turn on Preserve walls when meshing (-Y): TetGen then adds no points on the walls.";
    let fix = if m.surface_receiver_max_area_m2.is_some() {
        Fix::named(
            words,
            setting,
            Some(SettingValue::Bool(false)),
            "a surface receiver is refined when meshing, which splits its faces; -Y forbids \
             that, so the two cannot be on together. Remove the refinement first to use -Y.",
        )
    } else {
        Fix::apply(
            words,
            setting,
            SettingValue::Bool(false),
            SettingValue::Bool(true),
        )
    };
    Some(Advice {
        code: code::MESH_SPLITS_WALLS.into(),
        cause: "This meshing splits the walls; expect lost particles. TetGen may add points on \
                the room's surfaces, and particles are lost where the split faces meet."
            .into(),
        fix,
        values: vec![setting.pointer().into()],
    })
}

/// Every point receiver's distance to its nearest face, the smallest; `None` when a receiver is
/// not strictly inside (the validator reports that receiver) or there is none.
pub(super) fn min_clearance(p: &Project) -> Option<f64> {
    let mut min: Option<f64> = None;
    for r in &p.point_receivers {
        match locate_point(&p.geometry, r.position) {
            PointLocation::Inside { clearance_m } => {
                min = Some(min.map_or(clearance_m, |m| m.min(clearance_m)));
            }
            _ => return None,
        }
    }
    min
}

/// The note a noise fix carries when the room is outside the calibration's volumes.
pub(super) fn volume_note(volume_m3: f64) -> Option<String> {
    let (lo, hi) = CALIBRATED_VOLUME_M3;
    (volume_m3 < lo || volume_m3 > hi).then(|| {
        "This room's volume is outside the rooms the noise model was measured on, so this fix is \
         not checked here."
            .to_string()
    })
}

/// B2: `N·r²/V` below [`RECEIVERS_SMALL_K`]. The fix is the largest radius below every
/// receiver's clearance and at most 1 m when that is above the radius, else more particles
/// (named, no count: the advisor names only the core's own counts).
fn receivers_small(p: &Project, volume: f64) -> Option<Advice> {
    if p.point_receivers.is_empty() {
        return None;
    }
    let s = &p.solvers.spps;
    let r = s.receiver_radius_m.get();
    let n = f64::from(s.particles_per_source);
    if !(r.is_finite() && r > 0.0) {
        return None;
    }
    if n * r * r / volume >= RECEIVERS_SMALL_K {
        return None;
    }
    let cap = min_clearance(p)
        .and_then(radius_below)
        .map(|c| c.min(RADIUS_MAX_M));
    let mut fix = match cap {
        Some(c) if c > r => {
            let mut f = Fix::apply(
                "Use larger receivers (Receiver radius). The radius proposed keeps every \
                 receiver's sphere inside the walls and within the range EDT is checked for.",
                Setting::ReceiverRadius,
                SettingValue::Number(r),
                SettingValue::Number(c),
            );
            f.bound = Some(if c >= RADIUS_MAX_M {
                RadiusBound::EdtChecked
            } else {
                RadiusBound::Clearance
            });
            f
        }
        _ => Fix::named(
            "Use more particles (Particles per source and band).",
            Setting::ParticlesPerSource,
            Some(SettingValue::Number(n)),
            "a larger receiver would cross a wall here, and the advisor names no particle count \
             before a run: it does not forecast the noise.",
        ),
    };
    fix.note = volume_note(volume);
    Some(Advice {
        code: code::RECEIVERS_SMALL.into(),
        cause: "The receivers are small for this room's volume: few particles cross them, so \
                decay times will be noisy, or refused."
            .into(),
        fix,
        values: vec![
            Setting::ReceiverRadius.pointer().into(),
            Setting::ParticlesPerSource.pointer().into(),
        ],
    })
}

/// The slowest computed SPPS band's Sabine time, s: Sabine's `T`, the larger of Sabine and
/// Eyring, without the air (which only shortens it), at `24·ln 10/c` with `c` from the project's
/// temperature. `None` when any computed band's Sabine is refused (no absorption, an `α` outside
/// [0, 1]) or no band is computed: not guessed.
fn slowest_sabine_s(p: &Project, volume: f64) -> Option<f64> {
    let v = &p.geometry.vertices;
    let faces: Vec<(f64, Option<&crate::schema::Material>)> = p
        .geometry
        .faces
        .iter()
        .map(|f| {
            let [a, b, c] = f.vertices.map(|i| v.get(i as usize).map(|x| x.to_array()));
            let area = match (a, b, c) {
                (Some(a), Some(b), Some(c)) => {
                    let u = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
                    let w = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
                    let x = [
                        u[1] * w[2] - u[2] * w[1],
                        u[2] * w[0] - u[0] * w[2],
                        u[0] * w[1] - u[1] * w[0],
                    ];
                    0.5 * (x[0] * x[0] + x[1] * x[1] + x[2] * x[2]).sqrt()
                }
                _ => 0.0,
            };
            let mat = p.active_material(f.group).and_then(|m| p.material(m));
            (area, mat)
        })
        .collect();
    let c = crate::params::air::speed_of_sound(p.environment.temperature_c.get());
    let constant = RtConstant::Physical { speed_of_sound: c };
    let mut slowest: Option<f64> = None;
    for (band, on) in p.solvers.spps.bands_computed.iter().enumerate() {
        if !on {
            continue;
        }
        let mut surfaces = Vec::with_capacity(faces.len());
        for (area, mat) in &faces {
            let alpha = (*mat)?.absorption.get(band)?.get();
            surfaces.push(Surface {
                area_m2: *area,
                absorption: alpha,
            });
        }
        let t = room::sabine_rt(volume, &surfaces, None, constant).ok()?;
        if !t.is_finite() {
            return None;
        }
        slowest = Some(slowest.map_or(t, |s| s.max(t)));
    }
    slowest
}

/// B3: the duration below the slowest computed band's 60 dB decay (Sabine, the larger of Sabine
/// and Eyring). The fix is the new-project duration, or the longest the step limit allows.
fn run_short(p: &Project, volume: f64) -> Option<Advice> {
    let s = &p.solvers.spps;
    let (dur, dt) = (s.duration_s.get(), s.time_step_s.get());
    if !(dur.is_finite() && dur > 0.0) {
        return None;
    }
    let rt60 = slowest_sabine_s(p, volume)?;
    if dur >= rt60 {
        return None;
    }
    let fix = match longer_duration(dur, dt, Some(rt60)) {
        Some(to) => Fix::apply(
            "Run longer (Duration).",
            Setting::Duration,
            SettingValue::Number(dur),
            SettingValue::Number(to),
        ),
        None => Fix::named(
            "Run longer (Duration).",
            Setting::Duration,
            Some(SettingValue::Number(dur)),
            "the run is already as long as the solver's step counter allows at this time step; \
             a larger time step allows a longer run.",
        ),
    };
    Some(Advice {
        code: code::RUN_SHORT.into(),
        cause: "The run ends before the room's sound has decayed: the slowest band's \
                reverberation (Sabine, from the materials) is longer than the duration, so its \
                decay times may be refused."
            .into(),
        fix,
        values: vec![Setting::Duration.pointer().into()],
    })
}

/// The most particles per source any quantity's calibration needs under `method`: below it some
/// value is refused `noise_uncalibrated`. 150,000 for energetic T30, 50,000 in random mode.
pub(super) fn calibration_minimum(method: Method) -> u32 {
    (0..noise::QUANTITY_NAMES.len())
        .map(|i| noise::calibration::entry(method, i, Walls::Other).min_particles)
        .max()
        .unwrap_or(0)
}

/// B4: fewer particles per source than the calibration's minimum for some quantity. The fix is
/// that minimum: a calibration domain, not a forecast.
fn particles_few(p: &Project) -> Option<Advice> {
    let s = &p.solvers.spps;
    let method = match s.method {
        ComputationMethod::Random => Method::Random,
        ComputationMethod::Energetic => Method::Energetic,
    };
    let min = calibration_minimum(method);
    if s.particles_per_source >= min {
        return None;
    }
    Some(Advice {
        code: code::PARTICLES_FEW.into(),
        cause: "Fewer particles than the noise model was measured with: some values will be \
                refused because their noise cannot be judged."
            .into(),
        fix: Fix::apply(
            "Use the fewest particles the noise model was measured with (Particles per source \
             and band): a calibration minimum, not a forecast.",
            Setting::ParticlesPerSource,
            SettingValue::Number(f64::from(s.particles_per_source)),
            SettingValue::Number(f64::from(min)),
        ),
        values: vec![Setting::ParticlesPerSource.pointer().into()],
    })
}
