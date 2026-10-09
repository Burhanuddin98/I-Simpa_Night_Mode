//! Parameter maps (parity R42 with R73's time-valued ranges, decision 77): T30, EDT, C80 and D50
//! on every face of an SPPS surface receiver or cutting plane, as upstream's "Calculate acoustic
//! parameters" on a `.csbin` does (`projet_calculation.cpp`,
//! `OnMenuRecepteurSurfDoAcousticParametersComputation`), **by the code every point receiver's
//! value comes from**: each face's records are its energy series, built by the report's own
//! `series_of` (the band's completeness, the solver's floor, the particles left alive), and read
//! by `params::decay::evaluate` (T30, C80, D50) and EDT v2.1 (`params::edt::analyse`, checked
//! against the energy the series can lack as a receiver's is). The direct sound's arrival at a
//! face is the source's emission plus the distance to the face's centroid over `c`, spread over
//! half the spread of its vertices' distances (a plane is crossed, not a ball: no receiver
//! radius).
//!
//! What a map is not, and says so:
//! - **No Monte-Carlo noise range** for T30, C80 and D50: the receivers' noise model is calibrated
//!   on spheres, not on faces, which a particle crosses at any angle (`noise_note`). EDT keeps
//!   the range EDT v2.1 reads from the series itself.
//! - **Per band only**: the `Global` file sums the bands into one series no band's settings
//!   describe (refused, `map_global`).
//! - **SPL maps only** (`sound_map` "spl", `surf_receiv_method` 1): an intensity map counts the
//!   energy crossing a face, not the energy density there, so its C80 and D50 are not ISO 3382's
//!   (refused, `map_intensity`).
//! - **One source**: ISO 3382-1 defines these per source and receiver; with more than one source
//!   emitting in the band the map is refused, `several_sources`, as a receiver's values are.
//!
//! The bed (`results::extra_bed::r42`) holds a map's face to the point receiver whose centre it
//! holds, on a real run.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::report::{Evaluated, face_edt, series_of};
use super::spps::{LOST_SHARE_REFUSED, SppsResults};
use super::{RunResults, SolverResults};
use crate::formats::csbin;
use crate::params::decay::{self, Arrival};
use crate::params::{NotEvaluable, ParamError};

/// A parameter a map shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum MapParameter {
    #[serde(rename = "t30_s")]
    T30,
    #[serde(rename = "edt_s")]
    Edt,
    #[serde(rename = "c80_db")]
    C80,
    #[serde(rename = "d50")]
    D50,
}

impl MapParameter {
    pub const ALL: [MapParameter; 4] = [Self::T30, Self::Edt, Self::C80, Self::D50];

    /// Its JSON name, as the receivers' (`t30_s`, `edt_s`, `c80_db`, `d50`).
    pub fn name(self) -> &'static str {
        match self {
            Self::T30 => "t30_s",
            Self::Edt => "edt_s",
            Self::C80 => "c80_db",
            Self::D50 => "d50",
        }
    }

    /// Its unit: s, dB, or none (a fraction).
    pub fn unit(self) -> &'static str {
        match self {
            Self::T30 | Self::Edt => "s",
            Self::C80 => "dB",
            Self::D50 => "",
        }
    }
}

/// Why a map cannot be computed at all.
#[derive(Clone, Debug, PartialEq, Serialize, JsonSchema)]
pub struct MapRefusal {
    /// `map_not_spps`, `map_not_found`, `map_global`, `map_intensity`, `several_sources`,
    /// `map_time_step`, `map_unreadable`.
    pub code: String,
    pub message: String,
}

fn refuse(code: &str, message: String) -> MapRefusal {
    MapRefusal {
        code: code.into(),
        message,
    }
}

/// [`MapParameter`] on every face of one `.csbin`, faces in file order (every receiver's, as
/// SMAP orders them).
#[derive(Clone, Debug, PartialEq, Serialize, JsonSchema)]
pub struct ParameterMap {
    pub parameter: MapParameter,
    /// The file, relative to `solve/`.
    pub path: String,
    pub band_hz: i32,
    pub cutting_plane: bool,
    pub unit: String,
    /// Per face: the value, or `null` where it is refused (`why` says why).
    pub values: Vec<Option<f64>>,
    /// EDT only: the range EDT v2.1 reads, per face (`null` where refused); empty otherwise.
    pub lo: Vec<Option<f64>>,
    pub hi: Vec<Option<f64>>,
    /// Per face: `null` for a value, else the refusal's kind (`range_not_reached`, `truncated`,
    /// `no_energy`, ...).
    pub why: Vec<Option<String>>,
    /// Faces with a value.
    pub shown: usize,
    /// Refused faces, by kind.
    pub refused: BTreeMap<String, usize>,
    /// The smallest and largest value shown; `null` when none is.
    pub min: Option<f64>,
    pub max: Option<f64>,
    /// What a map's values carry and what they do not ([`NOISE_NOTE`] or [`EDT_NOTE`]).
    pub note: String,
    /// The band's lost share when it is a warning (0.3 % to 1 %), shown beside the map.
    pub lost_share_warning: Option<f64>,
}

/// T30's, C80's and D50's note.
pub const NOISE_NOTE: &str = "noise range not computed: the receivers' Monte-Carlo noise model is calibrated on spheres, not on faces";
/// EDT's note.
pub const EDT_NOTE: &str = "the range is EDT v2.1's own, read from each face's series";

/// The kind of a refusal, as a face's `why` names it.
fn kind(e: &ParamError) -> String {
    match e.not_evaluable() {
        Some(n) => serde_json::to_value(n)
            .ok()
            .and_then(|v| v.get("why").and_then(|w| w.as_str()).map(String::from))
            .unwrap_or_else(|| e.code().to_string()),
        None => e.code().to_string(),
    }
}

/// The arrival at a face with vertices `v`: emission plus the centroid's distance over `c`,
/// spread over half the spread of the vertices' distances.
fn face_arrival(s: &SppsResults, source: Option<(&[f64; 3], f64)>, v: [[f64; 3]; 3]) -> Arrival {
    let Some((q, emission)) = source.filter(|_| !s.celerity_gradient) else {
        return Arrival::Detected;
    };
    let d = |p: [f64; 3]| {
        ((p[0] - q[0]).powi(2) + (p[1] - q[1]).powi(2) + (p[2] - q[2]).powi(2)).sqrt()
    };
    let centroid = [0, 1, 2].map(|i| (v[0][i] + v[1][i] + v[2][i]) / 3.0);
    let ds = v.map(d);
    let spread = ds.iter().copied().fold(f64::NEG_INFINITY, f64::max)
        - ds.iter().copied().fold(f64::INFINITY, f64::min);
    let c = s.speed_of_sound_m_s;
    Arrival::spread(emission + d(centroid) / c, 0.5 * spread / c)
}

/// One face's value of `p` from its dense series `energy` (Pa²-like, per bin of `dt`) in band
/// `index` (`freq_hz`): the value with EDT's range, or the refusal.
pub(crate) fn face_value(
    s: &SppsResults,
    index: usize,
    freq_hz: i32,
    energy: &[f64],
    arrival: Arrival,
    p: MapParameter,
) -> Result<(f64, Option<(f64, f64)>), ParamError> {
    let series = series_of(s, index, freq_hz, energy, arrival)?;
    match p {
        MapParameter::Edt => {
            let h = match arrival {
                Arrival::Known { half_width_s, .. } => half_width_s,
                Arrival::Detected => 0.0,
            };
            match face_edt(s, &series, arrival, h) {
                Evaluated::Value { value, lo, hi, .. } => Ok((value, lo.zip(hi))),
                Evaluated::NotEvaluable { not_evaluable } => Err(not_evaluable.error),
            }
        }
        _ => {
            let b = decay::evaluate(&series, arrival);
            match p {
                MapParameter::T30 => b.t30.map(|f| (f.t_s, None)),
                MapParameter::C80 => b.c80_db.map(|v| (v, None)),
                _ => b.d50.map(|v| (v, None)),
            }
        }
    }
}

/// [`ParameterMap`] of `p` on the run's `.csbin` at `path` (as `run_data` lists it).
pub fn parameter_map(
    r: &RunResults,
    path: &str,
    p: MapParameter,
) -> Result<ParameterMap, MapRefusal> {
    let SolverResults::Spps(s) = &r.data else {
        return Err(refuse(
            "map_not_spps",
            "parameter maps are read from SPPS's time series; TCR writes none".into(),
        ));
    };
    let file = s.surfaces.iter().find(|f| f.path == path).ok_or_else(|| {
        refuse(
            "map_not_found",
            format!("the run has no surface map '{path}'"),
        )
    })?;
    let Some(freq_hz) = file.band_hz else {
        return Err(refuse(
            "map_global",
            "the Global map sums the bands: a parameter is read per band (turn on maps per band and run again)".into(),
        ));
    };
    if s.surf_receiv_method != 1 {
        return Err(refuse(
            "map_intensity",
            "this run's maps hold intensity, the energy crossing each face, not the sound level there: C80, D50 and the decay times are read from sound-level maps (set the map quantity to SPL and run again)".into(),
        ));
    }
    let index = r
        .bands_hz
        .iter()
        .position(|&f| f == freq_hz)
        .ok_or_else(|| {
            refuse(
                "map_not_found",
                format!("{freq_hz} Hz is not a band of this run"),
            )
        })?;
    let emitting: Vec<&super::spps::SourcePoint> = s
        .sources
        .iter()
        .filter(|x| x.band_power_w.get(index).is_some_and(|w| *w > 0.0))
        .collect();
    if emitting.len() > 1 {
        return Err(refuse(
            "several_sources",
            format!(
                "{} sources emit at {freq_hz} Hz: EDT, T30, C80 and D50 are defined per source and receiver (ISO 3382-1), and a map holds every source together",
                emitting.len()
            ),
        ));
    }
    if (f64::from(file.time_step) - s.time_step_s).abs() > 1e-9 * s.time_step_s {
        return Err(refuse(
            "map_time_step",
            format!(
                "the map is stored in bins of {} s, the run's series in {} s",
                file.time_step, s.time_step_s
            ),
        ));
    }
    let data = csbin::read_file(
        &r.folder
            .join(crate::run::manager::SOLVE_DIR)
            .join(&file.path),
    )
    .map_err(|e| refuse("map_unreadable", format!("{path} does not read now: {e}")))?;
    let source = emitting
        .first()
        .and_then(|x| x.position_m.as_ref().map(|q| (q, x.emission_s)));
    let lost = s.lost_share(freq_hz);
    let steps = data.time_step_count as usize;
    let edt_map = p == MapParameter::Edt;
    let mut out = ParameterMap {
        parameter: p,
        path: path.into(),
        band_hz: freq_hz,
        cutting_plane: file.cutting_plane,
        unit: p.unit().into(),
        values: Vec::new(),
        lo: Vec::new(),
        hi: Vec::new(),
        why: Vec::new(),
        shown: 0,
        refused: BTreeMap::new(),
        min: None,
        max: None,
        note: if edt_map { EDT_NOTE } else { NOISE_NOTE }.into(),
        lost_share_warning: lost
            .filter(|l| *l >= super::spps::LOST_SHARE_WARNING && *l < LOST_SHARE_REFUSED),
    };
    let node = |i: u32| data.nodes[i as usize].map(f64::from);
    for rx in &data.receivers {
        for face in &rx.faces {
            let mut energy = vec![0.0; steps];
            for rec in face.records.iter() {
                if let Some(e) = energy.get_mut(usize::from(rec.time_step)) {
                    *e += f64::from(rec.energy);
                }
            }
            let v = face.vertices.map(node);
            let arrival = face_arrival(s, source, v);
            let got = if lost.is_some_and(|l| l >= LOST_SHARE_REFUSED) {
                Err(crate::params::not_evaluable(
                    crate::params::Quantity::T30,
                    NotEvaluable::LostParticles {
                        share: lost.unwrap_or(0.0),
                        limit: LOST_SHARE_REFUSED,
                    },
                ))
            } else {
                face_value(s, index, freq_hz, &energy, arrival, p)
            };
            match got {
                Ok((value, range)) => {
                    out.shown += 1;
                    out.min = Some(out.min.map_or(value, |m| m.min(value)));
                    out.max = Some(out.max.map_or(value, |m| m.max(value)));
                    out.values.push(Some(value));
                    out.why.push(None);
                    if edt_map {
                        out.lo.push(range.map(|x| x.0));
                        out.hi.push(range.map(|x| x.1));
                    }
                }
                Err(e) => {
                    let k = kind(&e);
                    *out.refused.entry(k.clone()).or_default() += 1;
                    out.values.push(None);
                    out.why.push(Some(k));
                    if edt_map {
                        out.lo.push(None);
                        out.hi.push(None);
                    }
                }
            }
        }
    }
    Ok(out)
}

/// The faces of `path` (file order) whose triangle holds `point` within `tol_m` of its plane.
pub fn faces_at(r: &RunResults, path: &str, point: [f64; 3], tol_m: f64) -> Vec<usize> {
    let SolverResults::Spps(s) = &r.data else {
        return Vec::new();
    };
    let Some(file) = s.surfaces.iter().find(|f| f.path == path) else {
        return Vec::new();
    };
    let Ok(data) = csbin::read_file(
        &r.folder
            .join(crate::run::manager::SOLVE_DIR)
            .join(&file.path),
    ) else {
        return Vec::new();
    };
    let node = |i: u32| data.nodes[i as usize].map(f64::from);
    let mut out = Vec::new();
    let mut k = 0;
    for rx in &data.receivers {
        for face in &rx.faces {
            let [a, b, c] = face.vertices.map(node);
            if in_triangle(point, a, b, c, tol_m) {
                out.push(k);
            }
            k += 1;
        }
    }
    out
}

fn in_triangle(p: [f64; 3], a: [f64; 3], b: [f64; 3], c: [f64; 3], tol: f64) -> bool {
    let sub = |x: [f64; 3], y: [f64; 3]| [x[0] - y[0], x[1] - y[1], x[2] - y[2]];
    let dot = |x: [f64; 3], y: [f64; 3]| x[0] * y[0] + x[1] * y[1] + x[2] * y[2];
    let cross = |x: [f64; 3], y: [f64; 3]| {
        [
            x[1] * y[2] - x[2] * y[1],
            x[2] * y[0] - x[0] * y[2],
            x[0] * y[1] - x[1] * y[0],
        ]
    };
    let (ab, ac, ap) = (sub(b, a), sub(c, a), sub(p, a));
    let n = cross(ab, ac);
    let nn = dot(n, n).sqrt();
    if nn == 0.0 || (dot(ap, n) / nn).abs() > tol {
        return false;
    }
    let (d00, d01, d11, d20, d21) = (
        dot(ab, ab),
        dot(ab, ac),
        dot(ac, ac),
        dot(ap, ab),
        dot(ap, ac),
    );
    let den = d00 * d11 - d01 * d01;
    let v = (d11 * d20 - d01 * d21) / den;
    let w = (d00 * d21 - d01 * d20) / den;
    let e = 1e-9;
    v >= -e && w >= -e && v + w <= 1.0 + e
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_point_is_on_a_face_only_inside_it_and_near_its_plane() {
        let (a, b, c) = ([0.0, 7.0, 0.0], [1.0, 7.0, 0.0], [0.0, 7.0, 1.0]);
        assert!(in_triangle([0.25, 7.0, 0.25], a, b, c, 0.02));
        assert!(
            in_triangle([0.5, 7.0, 0.5], a, b, c, 0.02),
            "on the diagonal edge"
        );
        assert!(
            !in_triangle([0.6, 7.0, 0.6], a, b, c, 0.02),
            "past the diagonal"
        );
        assert!(
            !in_triangle([0.25, 7.05, 0.25], a, b, c, 0.02),
            "off the plane"
        );
        assert!(in_triangle([0.25, 7.01, 0.25], a, b, c, 0.02));
    }

    #[test]
    fn the_parameters_are_named_as_the_receivers_are() {
        let names: Vec<&str> = MapParameter::ALL.iter().map(|p| p.name()).collect();
        assert_eq!(names, ["t30_s", "edt_s", "c80_db", "d50"]);
        for p in MapParameter::ALL {
            assert_eq!(serde_json::to_value(p).unwrap(), p.name());
        }
        assert_eq!(MapParameter::C80.unit(), "dB");
        assert_eq!(MapParameter::D50.unit(), "");
    }
}
