//! Per-band parameters from an energy histogram: onset, Schroeder curve, EDT, T20, T30, C50,
//! C80, D50, Ts and SPL (`docs/params.md`).
//!
//! **The curve between bin edges.** A histogram gives each bin's energy, not where in the bin it
//! arrived. Every onset-relative quantity is read from one continuous Schroeder curve `S(u)`,
//! `u` the time since the arrival ([`Arrival`]):
//! - at every bin edge it is the histogram's own backward sum, exactly;
//! - between two edges it is log-linear, as an exponential decay inside the bin makes it;
//! - in the onset bin it is the direct sound, an impulse at the arrival, plus the decay of the
//!   next bin continued back to the arrival.
//!
//! The model is exact for an exponential decay and for an impulse followed by one, wherever in
//! its bin the arrival falls.
//!
//! **Four unknowns, bounded.** The energy after the series' end ([`Tail`]); the energy missing
//! from it because the solver dropped particles below a floor or lost them mid-path
//! ([`EnergySeries::with_solver_floor`], [`EnergySeries::with_lost_share`]); when the arrival is
//! not given, where in the onset bin it lies; and, for a solver whose reverberation begins with
//! the first reflection ([`EnergySeries::with_early_reverberation_unresolved`]), where it begins.
//! Each quantity is computed with and without the tail, with the missing energy added too, with the
//! arrival at each end of the onset bin, and with the reverberation beginning at the arrival, at
//! the first bin wholly after the direct sound and at that bin's end. When any of them moves it by
//! more than its limit ([`limits`]), it is refused, as `truncated`, `missing_moves`, `unresolved`
//! or `early_unresolved`. No alternative value is ever reported as the quantity; a value with two
//! or three readings is reported midway between them. A series its caller knows to be complete
//! ([`EnergySeries::complete`]) has no tail: [`Tail::Complete`] adds nothing and refuses nothing.

use schemars::JsonSchema;
use serde::Serialize;

use super::{EnergySeries, NotEvaluable, ParamError, Quantity, not_evaluable};

/// `p₀² = (20 µPa)²`, Pa².
pub const P_REF_SQUARED: f64 = 20e-6 * 20e-6;

/// The reference SPL divides the energy by: [`P_REF_SQUARED`], or in a test build the one a
/// [`crate::faults::Fault::LevelReference`] sets, so that gate M7(c)'s say-NO runs through this
/// code path.
fn level_reference_pa2() -> f64 {
    match crate::faults::active() {
        Some(crate::faults::Fault::LevelReference { pa2 }) => pa2,
        _ => P_REF_SQUARED,
    }
}

/// The onset is the first bin within this many dB of the largest (ISO 3382-1's 20 dB rule, as
/// commonly stated).
pub const ONSET_THRESHOLD_DB: f64 = 20.0;

/// A decay regression needs the curve to spend at least this many bin widths inside its range.
pub const MIN_REGRESSION_BINS: f64 = 2.0;

/// `|100·(T30/T20 − 1)|` above this marks a curved decay (ISO 3382-2's 10 %, as commonly stated).
pub const CURVATURE_LIMIT_PERCENT: f64 = 10.0;

/// How far an unknown the histogram cannot show (the tail after its end, the energy missing from
/// it, the arrival inside the onset bin) may move each quantity before it is refused: the tighter
/// of 1/10 of a difference limen and gate (a)'s bound (`docs/params.md`, "Truncation"). ISO
/// 3382-1's Table A.1, as commonly reproduced, gives limens for EDT, C80, D50, Ts and G only; T20
/// and T30 take EDT's, C50 takes C80's.
pub mod limits {
    /// EDT, T20, T30: relative. 1/10 of EDT's 5 % limen, and gate (a)'s bound.
    pub const DECAY_RELATIVE: f64 = 0.005;
    /// C50, C80: dB. Gate (a)'s bound, tighter than 1/10 of C80's 1 dB limen.
    pub const CLARITY_DB: f64 = 0.01;
    /// D50: fraction (0.1 percentage points). Gate (a)'s bound, tighter than 1/10 of the 0.05
    /// limen.
    pub const DEFINITION: f64 = 0.001;
    /// Ts: s, 1/10 of the 10 ms limen; or [`CENTRE_TIME_RELATIVE`] of Ts, whichever is tighter.
    pub const CENTRE_TIME_S: f64 = 0.001;
    /// Ts: relative, the bound gate (a)'s test holds Ts to (the gate names none; this is the
    /// decay times' bound).
    pub const CENTRE_TIME_RELATIVE: f64 = 0.005;
    /// SPL: dB. 1/10 of the 1 dB limen quoted for G; gate (c)'s ±0.5 dB is looser.
    pub const SPL_DB: f64 = 0.1;
}

/// The onset bin: the first bin whose energy is within [`ONSET_THRESHOLD_DB`] of the largest.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, JsonSchema)]
pub struct Onset {
    pub index: usize,
    /// Where the bin starts, s.
    pub bin_start_s: f64,
    /// Where it ends, s.
    pub bin_end_s: f64,
}

/// The onset bin of `series` (`docs/params.md`, "Direct-arrival detection").
pub fn onset(series: &EnergySeries) -> Onset {
    let v = series.values();
    let max = v.iter().copied().fold(0.0_f64, f64::max);
    let threshold = max / 10f64.powf(ONSET_THRESHOLD_DB / 10.0);
    // A series holds energy (EnergySeries::new), so some bin reaches the threshold.
    let index = v.iter().position(|&e| e >= threshold && e > 0.0).unwrap();
    Onset {
        index,
        bin_start_s: index as f64 * series.dt(),
        bin_end_s: (index + 1) as f64 * series.dt(),
    }
}

/// When the direct sound arrived, which every onset-relative quantity is measured from.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case", tag = "arrival")]
pub enum Arrival {
    /// Not given: somewhere in the onset bin. Each quantity is computed with the arrival at both
    /// ends of the bin, and refused as `unresolved` when the two differ by more than its limit.
    Detected,
    /// Given, such as the source–receiver distance over the speed of sound. For C50, C80, D50 and
    /// Ts it must lie in the onset bin; otherwise those four are refused, `params_bad_arrival`.
    /// The decay times are computed whatever the arrival ([`evaluate`]).
    Known {
        time_s: f64,
        /// The direct sound reaches the receiver over `[time_s − half_width_s, time_s +
        /// half_width_s]`, not at one instant: a ball of radius `R` crossed at `c` takes `2R/c`
        /// (`docs/params.md`, "The direct sound's spread"). 0 for an impulse.
        half_width_s: f64,
    },
}

impl Arrival {
    /// A direct sound that is an impulse at `time_s`.
    pub const fn at(time_s: f64) -> Arrival {
        Arrival::Known {
            time_s,
            half_width_s: 0.0,
        }
    }

    /// A direct sound centred on `time_s` and spread over `half_width_s` either side of it, as a
    /// receiver ball of radius `R` sees it at speed `c`: `half_width_s = R/c`.
    pub const fn spread(time_s: f64, half_width_s: f64) -> Arrival {
        Arrival::Known {
            time_s,
            half_width_s,
        }
    }

    /// A given time, checked against the onset bin `o` of `series`.
    fn check(self, series: &EnergySeries, o: Onset) -> Result<Arrival, ParamError> {
        let Arrival::Known {
            time_s,
            half_width_s,
        } = self
        else {
            return Ok(self);
        };
        let bad = |detail: String| Err(ParamError::BadArrival { time_s, detail });
        if !time_s.is_finite() || time_s < 0.0 {
            return bad("not a finite time at or after 0 s".into());
        }
        if !half_width_s.is_finite() || half_width_s < 0.0 {
            return bad(format!(
                "the direct sound's half-width {half_width_s} s is not a finite time of at \
                 least 0 s"
            ));
        }
        // A time a rounding step before the bin's start is its start.
        if time_s < o.bin_start_s - 1e-9 * series.dt() {
            return bad(format!(
                "before the onset bin [{}, {}) s. The energy there is more than {ONSET_THRESHOLD_DB} \
                 dB below the largest bin, so the given time does not place the onset: evaluate \
                 with the arrival detected",
                o.bin_start_s, o.bin_end_s
            ));
        }
        if time_s >= o.bin_end_s {
            return bad(format!(
                "after the onset bin [{}, {}) s: energy within {ONSET_THRESHOLD_DB} dB of the \
                 largest bin arrived before it",
                o.bin_start_s, o.bin_end_s
            ));
        }
        Ok(Arrival::Known {
            time_s: time_s.max(o.bin_start_s),
            half_width_s,
        })
    }
}

/// Where one Schroeder curve starts, and which bins it reads as the histogram gives them.
#[derive(Clone, Copy, Debug)]
struct Start {
    /// The arrival, s: `u = 0`.
    time_s: f64,
    /// The curve's top is the backward sum from this bin: the onset bin.
    top_bin: usize,
    /// The first bin wholly after the direct sound. From its start on the curve is the
    /// histogram's own; before it, back to the arrival, the reverberation is its decay continued
    /// back, and the rest is the direct sound, at `u = 0`.
    exact_bin: usize,
}

impl Start {
    /// From a given arrival `t` with the direct sound over `[t − h, t + h]`, the onset bin `k0`
    /// being the first to hold energy within 20 dB of the largest. The top is the sum from the
    /// onset bin, or from the bin the direct sound starts in when that is earlier: a spread
    /// direct sound's leading edge is weak, and can fall more than 20 dB below the largest bin,
    /// but it is the direct sound all the same. The first bin wholly after the direct sound is at
    /// least the one after the arrival's.
    fn known(t: f64, h: f64, k0: usize, dt: f64) -> Start {
        let arrival_bin = (t / dt).floor() as usize;
        let leading_bin = ((t - h) / dt).floor().max(0.0) as usize;
        // A bin that starts a rounding step before the direct sound ends is after it.
        let after = ((t + h) / dt - 1e-9).ceil().max(0.0) as usize;
        Start {
            time_s: t,
            top_bin: k0.min(leading_bin),
            exact_bin: after.max(arrival_bin + 1).max(k0 + 1),
        }
    }

    /// The two ends of the onset bin `k0`, when the arrival is not given.
    fn detected(o: Onset) -> [Start; 2] {
        [o.bin_start_s, o.bin_end_s].map(|t| Start {
            time_s: t,
            top_bin: o.index,
            exact_bin: o.index + 1,
        })
    }
}

/// What the decay times are measured from when a given arrival does not fit the onset bin
/// (`docs/params.md`, "Direct-arrival detection"). They do not depend on where time starts, only
/// on where in the histogram the direct sound is taken to be:
/// - the arrival after the onset bin, with the onset bin reaching into the direct sound's spread
///   (`(k₀+1)·dt > t − h`): the onset bin holds the leading part of the direct sound, which a
///   receiver ball starts to catch `R/c` before its centre. The curve is read from the arrival,
///   with everything from the onset bin up to it counted in the direct sound;
/// - any other misfit (the arrival before the onset bin, so more than 20 dB below the largest
///   bin; after it by more than the spread; not a time): as if no arrival were given, from the two
///   ends of the onset bin ([`Arrival::Detected`]).
fn decay_starts(arrival: Arrival, o: Onset, dt: f64) -> Vec<Start> {
    match arrival {
        Arrival::Known {
            time_s,
            half_width_s,
        } if time_s.is_finite()
            && half_width_s.is_finite()
            && half_width_s >= 0.0
            && time_s >= o.bin_end_s
            && o.bin_end_s > time_s - half_width_s =>
        {
            vec![Start::known(time_s, half_width_s, o.index, dt)]
        }
        _ => Start::detected(o).to_vec(),
    }
}

/// The energy estimated to arrive after the last bin with energy.
///
/// Trailing empty bins are not taken as proof that the decay ended: a histogram that stops
/// abruptly is bounded from the energy before it stopped, so the stop is refused when it matters.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case", tag = "tail")]
pub enum Tail {
    /// The series decays over its last two windows; `energy` is what follows `from_s`, the end
    /// of the last bin with energy, if the decay goes on at `ratio_per_bin`.
    Bounded {
        energy: f64,
        ratio_per_bin: f64,
        from_s: f64,
    },
    /// The series is not decaying at its end: nothing bounds what follows.
    Unbounded,
    /// The caller knows that nothing follows ([`EnergySeries::complete`]): nothing is added, and
    /// no quantity is refused for the tail.
    Complete,
}

/// The tail estimate of `series`: two windows of `w` bins ending at the last bin with energy,
/// `w` a tenth of the bins from the onset to there and at least 1 (`docs/params.md`,
/// "Truncation"). Refused when fewer than 2 bins from the onset hold energy up to the last.
/// [`Tail::Complete`] for a series made with [`EnergySeries::complete`].
pub fn tail(series: &EnergySeries) -> Result<Tail, ParamError> {
    tail_after(series, onset(series))
}

/// One past the last bin with energy.
fn energy_end(v: &[f64]) -> usize {
    // A series holds energy (EnergySeries::new).
    v.iter().rposition(|&e| e > 0.0).unwrap() + 1
}

fn tail_after(series: &EnergySeries, o: Onset) -> Result<Tail, ParamError> {
    if series.is_complete() {
        return Ok(Tail::Complete);
    }
    let v = series.values();
    let n = energy_end(v);
    let post = n - o.index;
    if post < 2 {
        return Err(ParamError::SeriesTooShort {
            what: "the tail estimate (2 bins from the onset to the last with energy)".into(),
            needed_s: (o.index + 2) as f64 * series.dt(),
            available_s: n as f64 * series.dt(),
        });
    }
    let w = (post / 10).max(1);
    let w2: f64 = v[n - w..n].iter().sum();
    let w1: f64 = v[n - 2 * w..n - w].iter().sum();
    Ok(if w1 == 0.0 || w2 >= w1 {
        Tail::Unbounded
    } else {
        let r = w2 / w1;
        Tail::Bounded {
            energy: w2 * r / (1.0 - r),
            ratio_per_bin: r.powf(1.0 / w as f64),
            from_s: n as f64 * series.dt(),
        }
    })
}

/// `S_k = Σ_{j≥k} B_j` for `k = 0..=n`, summed from the end; `S_n = 0`.
fn backward_sums(v: &[f64]) -> Vec<f64> {
    let mut s = vec![0.0; v.len() + 1];
    for k in (0..v.len()).rev() {
        s[k] = s[k + 1] + v[k];
    }
    s
}

/// The raw Schroeder curve at the bin edges from the onset bin's start:
/// `(k·dt, 10·lg(S_k / S_{k₀}))`, nothing added for the unseen tail and nothing placed inside a
/// bin. A level after the last energy is `-inf`. The parameters do not use it; it is what
/// upstream's GUI fits (`projet_calculation.cpp:68-83`).
pub fn schroeder_db(series: &EnergySeries) -> Vec<(f64, f64)> {
    let o = onset(series);
    let s = backward_sums(series.values());
    let s0 = s[o.index];
    (o.index..series.len())
        .map(|k| (k as f64 * series.dt(), 10.0 * (s[k] / s0).log10()))
        .collect()
}

/// A decay-time evaluation range.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DecayRange {
    /// 0 dB to −10 dB.
    Edt,
    /// −5 dB to −25 dB.
    T20,
    /// −5 dB to −35 dB.
    T30,
}

impl DecayRange {
    /// The upper limit, dB.
    pub fn top_db(self) -> f64 {
        match self {
            DecayRange::Edt => 0.0,
            DecayRange::T20 | DecayRange::T30 => -5.0,
        }
    }

    /// The lower limit, dB.
    pub fn bottom_db(self) -> f64 {
        match self {
            DecayRange::Edt => -10.0,
            DecayRange::T20 => -25.0,
            DecayRange::T30 => -35.0,
        }
    }

    fn quantity(self) -> Quantity {
        match self {
            DecayRange::Edt => Quantity::Edt,
            DecayRange::T20 => Quantity::T20,
            DecayRange::T30 => Quantity::T30,
        }
    }
}

/// A decay time and the regression it came from.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, JsonSchema)]
pub struct DecayFit {
    pub range: DecayRange,
    /// `−60 / slope`, s.
    pub t_s: f64,
    /// dB/s.
    pub slope_db_per_s: f64,
    /// How long the curve spends inside the range, s (the shorter, when the arrival is detected).
    pub span_s: f64,
    /// The same fit on the curve with the unseen tail added. A check, never the value.
    pub with_tail_s: f64,
}

/// A decay time of `series` (`docs/params.md`, "Decay times"). It is computed whatever the
/// arrival: a given arrival that does not fit the onset bin changes only where the direct sound is
/// taken to be ([`evaluate`]).
pub fn decay_time(
    series: &EnergySeries,
    arrival: Arrival,
    range: DecayRange,
) -> Result<DecayFit, ParamError> {
    Analysis::new(series, arrival).decay_time(range)
}

/// C_te in dB, `te` in seconds (C50: 0.05, C80: 0.08). A `te` that is not a positive number is
/// refused as `params_bad_time_step`; a given arrival outside the onset bin as
/// `params_bad_arrival`.
pub fn clarity_db(series: &EnergySeries, arrival: Arrival, te_s: f64) -> Result<f64, ParamError> {
    Analysis::new(series, arrival).clarity_db(te_s)
}

/// D_te as a fraction, `te` in seconds (D50: 0.05). Refused as [`clarity_db`] is.
pub fn definition(series: &EnergySeries, arrival: Arrival, te_s: f64) -> Result<f64, ParamError> {
    Analysis::new(series, arrival).definition(te_s)
}

/// The centre time Ts from the arrival, s. A given arrival outside the onset bin is refused,
/// `params_bad_arrival`.
pub fn centre_time_s(series: &EnergySeries, arrival: Arrival) -> Result<f64, ParamError> {
    Analysis::new(series, arrival).centre_time_s()
}

/// The sound pressure level of every bin summed, dB re 20 µPa. It does not depend on the arrival.
pub fn spl_db(series: &EnergySeries) -> Result<f64, ParamError> {
    Analysis::new(series, Arrival::Detected).spl_db()
}

/// The largest distance, dB, between the Schroeder curve and the straight lines through the points
/// [`decay_curve`] keeps of it.
pub const CURVE_TOLERANCE_DB: f64 = 0.01;

/// The Schroeder curve EDT, T20 and T30 are fitted to, for display (`docs/formats/results-json.md`,
/// "The decay curve"): the same curve, from the same code, not a second computation of it.
#[derive(Clone, Debug, PartialEq, Serialize, JsonSchema)]
pub struct DecayCurve {
    /// The time, s, of the curve's `u = 0`: the arrival the decay times are measured from
    /// ([`BandParameters::decay_arrival`]); with the arrival detected, the start of the onset bin,
    /// the first of the two ends the decay times are read from.
    pub from_s: f64,
    /// `[u, level]`: `u` in s since `from_s`, the level in dB re the curve's value at `u = 0`,
    /// which includes the direct sound. Joined by straight lines they are the curve within
    /// `tolerance_db`: the curve is straight in dB between its knots, and a knot is left out only
    /// when the line through the points kept either side of it passes within `tolerance_db` of it
    /// and of every other knot left out between them. The first point is `[0, 0]`; the second,
    /// also at `u = 0`, is the level just after the direct sound (the curve steps down by it at
    /// the arrival). The last is the start of the last bin with energy: inside that bin the curve
    /// falls to nothing, which a level cannot show, and the fits leave it out.
    pub points: Vec<[f64; 2]>,
    /// [`CURVE_TOLERANCE_DB`].
    pub tolerance_db: f64,
    /// The knots of the whole curve, before any is left out.
    pub knots: usize,
    /// From this `u` on, every knot is the histogram's own backward sum at a bin edge. Before it
    /// the curve is the model's reading: the direct sound a step at `u = 0`, then the decay of the
    /// first bin wholly after the direct sound continued back to the arrival
    /// (`docs/params.md`, "The curve between bin edges"). When the series' early reverberation is
    /// unresolved, the values are read two more ways over this stretch
    /// ([`EnergySeries::with_early_reverberation_unresolved`]); the curve shown is this reading.
    pub histogram_from_s: f64,
}

/// The Schroeder curve of `series` that EDT, T20 and T30 are read from, from `arrival` as
/// [`evaluate`] takes it, thinned for display ([`DecayCurve`]). Nothing is added for the unseen
/// tail, the floor or lost particles: this is the curve the values come from, and those bounds
/// judge them.
pub fn decay_curve(series: &EnergySeries, arrival: Arrival) -> DecayCurve {
    let a = Analysis::new(series, arrival);
    let view = &a.decay_views()[0];
    let knots = view.plain.knots();
    DecayCurve {
        from_s: view.arrival_s,
        knots: knots.len(),
        points: thin(&knots, CURVE_TOLERANCE_DB),
        tolerance_db: CURVE_TOLERANCE_DB,
        histogram_from_s: view.histogram_from_s,
    }
}

/// The points of the polyline `knots` that straight lines through them keep within `tol` of every
/// knot: from each point kept, the furthest knot the line to which passes within `tol` of every
/// knot between them (the slopes each of those allows, intersected as the line is extended).
/// `u` increases strictly, except at the first two knots, a step at `u = 0`, both kept.
fn thin(knots: &[[f64; 2]], tol: f64) -> Vec<[f64; 2]> {
    if knots.len() <= 2 {
        return knots.to_vec();
    }
    let lead = if knots[1][0] == knots[0][0] { 2 } else { 1 };
    let mut kept: Vec<[f64; 2]> = knots[..lead].to_vec();
    let mut a = lead - 1;
    let (mut lo, mut hi) = (f64::NEG_INFINITY, f64::INFINITY);
    let mut k = a + 1;
    while k < knots.len() {
        let du = knots[k][0] - knots[a][0];
        let slope = (knots[k][1] - knots[a][1]) / du;
        if slope >= lo && slope <= hi {
            lo = lo.max((knots[k][1] - tol - knots[a][1]) / du);
            hi = hi.min((knots[k][1] + tol - knots[a][1]) / du);
            k += 1;
        } else {
            // The line cannot reach k: the knot before it is kept, and the next line starts there
            // (the knot right after an anchor is always reachable, so this moves on).
            a = k - 1;
            kept.push(knots[a]);
            lo = f64::NEG_INFINITY;
            hi = f64::INFINITY;
        }
    }
    let last = knots[knots.len() - 1];
    if kept.last() != Some(&last) {
        kept.push(last);
    }
    kept
}

/// `100·(T30/T20 − 1)`, and whether it marks a curved decay.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, JsonSchema)]
pub struct Curvature {
    pub percent: f64,
    /// `|percent| > CURVATURE_LIMIT_PERCENT`.
    pub curved: bool,
}

/// The curvature of a decay from its T20 and T30.
pub fn curvature(t20: &DecayFit, t30: &DecayFit) -> Curvature {
    let percent = 100.0 * (t30.t_s / t20.t_s - 1.0);
    Curvature {
        percent,
        curved: percent.abs() > CURVATURE_LIMIT_PERCENT,
    }
}

/// Every parameter of one band. Each is its value or its refusal; one refusal never hides
/// another quantity.
#[derive(Clone, Debug, PartialEq, Serialize, JsonSchema)]
pub struct BandParameters {
    pub onset: Onset,
    /// The arrival C50, C80, D50 and Ts are measured from: the given one, or
    /// [`Arrival::Detected`]. When a given arrival does not fit the onset bin those four are
    /// refused, `params_bad_arrival`, and this is the arrival as given.
    pub arrival: Arrival,
    /// What EDT, T20 and T30 are measured from: `arrival` when it fits the onset bin, or follows
    /// it within the direct sound's spread; otherwise [`Arrival::Detected`] (`docs/params.md`,
    /// "Direct-arrival detection"). Decay times are never refused `params_bad_arrival`; measured
    /// as detected, they can be refused `unresolved`.
    pub decay_arrival: Arrival,
    pub tail: Result<Tail, ParamError>,
    pub spl_db: Result<f64, ParamError>,
    pub edt: Result<DecayFit, ParamError>,
    pub t20: Result<DecayFit, ParamError>,
    pub t30: Result<DecayFit, ParamError>,
    pub c50_db: Result<f64, ParamError>,
    pub c80_db: Result<f64, ParamError>,
    /// A fraction.
    pub d50: Result<f64, ParamError>,
    pub ts_s: Result<f64, ParamError>,
    pub curvature: Result<Curvature, ParamError>,
}

/// Every parameter of one band's series.
///
/// **A given arrival that does not fit the onset bin** refuses C50, C80, D50 and Ts only,
/// `params_bad_arrival`: they are measured from the arrival. SPL does not depend on it, and EDT,
/// T20 and T30 do not depend on where time starts, only on where in the histogram the direct
/// sound is taken to be: they are computed from the arrival when it follows the onset bin within
/// the direct sound's spread, and otherwise as if no arrival were given (`BandParameters::
/// decay_arrival`).
pub fn evaluate(series: &EnergySeries, arrival: Arrival) -> BandParameters {
    let a = Analysis::new(series, arrival);
    let t20 = a.decay_time(DecayRange::T20);
    let t30 = a.decay_time(DecayRange::T30);
    let curvature = match (&t20, &t30) {
        (Ok(a), Ok(b)) => Ok(curvature(a, b)),
        (Err(e), _) | (_, Err(e)) => Err(match e {
            ParamError::NotEvaluable { why, .. } => not_evaluable(Quantity::Curvature, why.clone()),
            other => other.clone(),
        }),
    };
    BandParameters {
        onset: a.onset,
        arrival: a.arrival,
        decay_arrival: a.decay_arrival,
        tail: a.tail.clone(),
        spl_db: a.spl_db(),
        edt: a.decay_time(DecayRange::Edt),
        t20,
        t30,
        c50_db: a.clarity_db(0.05),
        c80_db: a.clarity_db(0.08),
        d50: a.definition(0.05),
        ts_s: a.centre_time_s(),
        curvature,
    }
}

/// How the curve runs between two neighbouring knots.
#[derive(Clone, Copy, Debug)]
enum Shape {
    /// `S` exponential in time: the level falls linearly.
    LogLinear,
    /// `S` falls linearly to 0: the last bin with energy, when nothing follows it.
    Linear,
}

/// One stretch of the curve, `u` in seconds since the arrival.
#[derive(Clone, Copy, Debug)]
struct Piece {
    u0: f64,
    u1: f64,
    s0: f64,
    s1: f64,
    shape: Shape,
}

impl Piece {
    fn at(&self, u: f64) -> f64 {
        let x = ((u - self.u0) / (self.u1 - self.u0)).clamp(0.0, 1.0);
        match self.shape {
            Shape::LogLinear => self.s0 * (self.s1 / self.s0).powf(x),
            Shape::Linear => self.s0 + (self.s1 - self.s0) * x,
        }
    }

    /// `∫ S du` over the piece.
    fn integral(&self) -> f64 {
        let h = self.u1 - self.u0;
        match self.shape {
            Shape::LogLinear if self.s0 - self.s1 > 1e-12 * self.s0 => {
                h * (self.s0 - self.s1) / (self.s0 / self.s1).ln()
            }
            _ => h * (self.s0 + self.s1) / 2.0,
        }
    }
}

/// The Schroeder curve from one arrival (the module's model).
struct Curve {
    /// `S` at the arrival, the direct sound included: 0 dB.
    top: f64,
    /// Contiguous from `u = 0`.
    pieces: Vec<Piece>,
    /// Beyond the last piece, when the tail is added: `M·e^(−μ·(u − end))`, as `(M, μ)`.
    tail: Option<(f64, f64)>,
}

/// Where the reverberation begins, when the series leaves it unresolved
/// ([`EnergySeries::with_early_reverberation_unresolved`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Early {
    /// At the arrival: the first bin wholly after the direct sound's decay continued back to it
    /// (the module's model, and the only reading of a series that does not leave it unresolved).
    Arrival,
    /// At the start of the first bin wholly after the direct sound: everything before it is the
    /// direct sound, and the curve stays at that bin's sum from the arrival.
    FirstBin,
    /// At the end of that bin: the curve stays at its sum through it too, its energy arriving at
    /// its end, as a reverberation that builds up through the bin brings it late.
    AfterFirstBin,
}

impl Curve {
    /// The curve of the backward sums `sums` from `start`, up to `end`, one past the last bin with
    /// energy; with `tail`, `M` is added to every sum and the curve goes on past `end`; the
    /// reverberation beginning where `early` says.
    fn new(
        sums: &[f64],
        dt: f64,
        start: Start,
        end: usize,
        tail: Option<(f64, f64)>,
        early: Early,
    ) -> Curve {
        let m = tail.map_or(0.0, |t| t.0);
        // Past the end every sum is 0.
        let s = |k: usize| sums.get(k).copied().unwrap_or(0.0) + m;
        let shape = |s1: f64| {
            if s1 > 0.0 {
                Shape::LogLinear
            } else {
                Shape::Linear
            }
        };
        let t_a = start.time_s;
        let top = s(start.top_bin);
        let first = start.exact_bin;
        let mut pieces = Vec::with_capacity(end.saturating_sub(first) + 1);
        // From the arrival to the first bin wholly after the direct sound: that bin's decay
        // continued back to the arrival, at most everything since the onset; the rest is the
        // direct sound, at u = 0. With the direct sound an impulse inside the onset bin, `first`
        // is the next bin and this is the rest of the onset bin.
        let u1 = first as f64 * dt - t_a;
        if u1 > 0.0 {
            let next = s(first);
            let continued = if early != Early::Arrival {
                next
            } else if first < end && next > 0.0 && s(first + 1) > 0.0 {
                next * (next / s(first + 1)).powf(u1 / dt)
            } else {
                f64::INFINITY
            };
            pieces.push(Piece {
                u0: 0.0,
                u1,
                s0: continued.min(top),
                s1: next,
                shape: shape(next),
            });
        }
        for k in first..end {
            // After the first bin, that bin holds its energy at its end: the curve drops there, to
            // the next sum.
            let s1 = if early == Early::AfterFirstBin && k == first {
                s(k)
            } else {
                s(k + 1)
            };
            pieces.push(Piece {
                u0: k as f64 * dt - t_a,
                u1: (k + 1) as f64 * dt - t_a,
                s0: s(k),
                s1,
                shape: shape(s1),
            });
        }
        Curve { top, pieces, tail }
    }

    fn end(&self) -> f64 {
        self.pieces.last().map_or(0.0, |p| p.u1)
    }

    /// `[u, level dB]` at every knot the fits see: `[0, 0]`, the level just after the direct
    /// sound when it steps the curve down, and the end of every log-linear piece. The last piece,
    /// where the curve falls linearly to nothing, is left out, as the fits leave it out.
    fn knots(&self) -> Vec<[f64; 2]> {
        let db = |s: f64| 10.0 * (s / self.top).log10();
        let mut knots: Vec<[f64; 2]> = vec![[0.0, 0.0]];
        let mut shown = self
            .pieces
            .iter()
            .filter(|p| matches!(p.shape, Shape::LogLinear))
            .peekable();
        if let Some(first) = shown.peek() {
            // A step of rounding size is no direct sound.
            let after = db(first.s0);
            if after < -1e-9 {
                knots.push([0.0, after]);
            }
        }
        knots.extend(shown.map(|p| [p.u1, db(p.s1)]));
        knots
    }

    /// `S(u)`; at `u = 0` the direct sound is included.
    fn at(&self, u: f64) -> f64 {
        if u <= 0.0 {
            return self.top;
        }
        if let Some(p) = self.pieces.iter().find(|p| u <= p.u1) {
            return p.at(u);
        }
        match self.tail {
            Some((m, mu)) => m * (-mu * (u - self.end())).exp(),
            None => 0.0,
        }
    }

    /// `∫₀^∞ S du`, which is `∫ u·E du`: the direct sound, at `u = 0`, adds nothing.
    fn moment(&self) -> f64 {
        self.pieces.iter().map(Piece::integral).sum::<f64>()
            + self.tail.map_or(0.0, |(m, mu)| m / mu)
    }

    /// The least-squares line through the level `10·lg(S(u)/S(0))` over all the time it spends
    /// inside `range`, up to the end of the series. A `Linear` piece, where the curve dives to
    /// nothing, is left out.
    fn line(&self, range: DecayRange, dt: f64) -> Result<Line, NotEvaluable> {
        let (top, bottom) = (range.top_db(), range.bottom_db());
        let db = |s: f64| 10.0 * (s / self.top).log10();
        // (width, midpoint, level at the midpoint, slope) of each stretch inside the range.
        let mut parts = Vec::new();
        for p in self
            .pieces
            .iter()
            .filter(|p| matches!(p.shape, Shape::LogLinear))
        {
            let (l0, l1) = (db(p.s0), db(p.s1));
            if l1 > top || l0 < bottom {
                continue;
            }
            let q = (l1 - l0) / (p.u1 - p.u0);
            // The level only falls, so q < 0 wherever the piece crosses a limit.
            let ua = if l0 > top {
                p.u0 + (top - l0) / q
            } else {
                p.u0
            };
            let ub = if l1 < bottom {
                p.u0 + (bottom - l0) / q
            } else {
                p.u1
            };
            if ub > ua {
                let mid = 0.5 * (ua + ub);
                parts.push((ub - ua, mid, l0 + q * (mid - p.u0), q));
            }
        }
        let span: f64 = parts.iter().map(|p| p.0).sum();
        let needed = MIN_REGRESSION_BINS * dt;
        if span < needed * (1.0 - 1e-9) {
            return Err(NotEvaluable::RangeTooShort {
                span_s: span,
                needed_s: needed,
            });
        }
        let mean = parts.iter().map(|p| p.0 * p.1).sum::<f64>() / span;
        let (mut suu, mut sul) = (0.0, 0.0);
        for &(w, mid, level, q) in &parts {
            let inner = w * w * w / 12.0;
            suu += w * (mid - mean).powi(2) + inner;
            sul += w * (mid - mean) * level + q * inner;
        }
        let slope = sul / suu;
        if slope < 0.0 {
            Ok(Line { slope, span })
        } else {
            Err(NotEvaluable::NotDecaying)
        }
    }
}

struct Line {
    slope: f64,
    span: f64,
}

/// The curve from one arrival, the same with the unseen tail added, and with the missing energy
/// added as well.
struct View {
    arrival_s: f64,
    /// `u` of the start of the first bin wholly after the direct sound: from there on the curve is
    /// the histogram's own.
    histogram_from_s: f64,
    plain: Curve,
    /// `None` when the tail is unbounded.
    with_tail: Option<Curve>,
    /// `None` when nothing is missing, or the missing energy has no finite bound.
    with_missing: Option<Curve>,
    /// The same three with the reverberation beginning later ([`Early::FirstBin`],
    /// [`Early::AfterFirstBin`]), when the series' early reverberation is unresolved
    /// ([`EnergySeries::with_early_reverberation_unresolved`]); empty otherwise.
    later: Vec<View>,
}

/// One quantity from one arrival: from the curve as modelled, and from the curves with the
/// reverberation beginning later ([`View::later`]); a `plain` that is NaN where that curve gives no
/// value.
#[derive(Clone, Debug)]
struct Pair {
    cont: Eval,
    later: Vec<Eval>,
}

impl View {
    /// `f` on this view, and on each of its later ones.
    fn pair(&self, f: impl Fn(&View) -> Eval) -> Pair {
        Pair {
            cont: f(self),
            later: self.later.iter().map(&f).collect(),
        }
    }
}

impl Pair {
    /// The lowest and highest reading, of `plain`, `tail` or `missing` (`pick`), over the
    /// continued curve and the later ones: `None` when a reading has none.
    fn span(&self, pick: impl Fn(&Eval) -> Option<f64>) -> Option<(f64, f64)> {
        let mut lo = pick(&self.cont)?;
        let mut hi = lo;
        for e in &self.later {
            let x = pick(e)?;
            lo = lo.min(x);
            hi = hi.max(x);
        }
        Some((lo, hi))
    }
}

/// Energy missing from the series ([`EnergySeries::with_solver_floor`],
/// [`EnergySeries::with_lost_share`]), bounded (`docs/params.md`, "Missing energy").
#[derive(Clone, Copy, Debug)]
struct Missing {
    floor_db: Option<f64>,
    lost_share: Option<f64>,
    /// The most energy the dropped and lost particles would still have brought to the receiver:
    /// `10^{floor/10} / alive_share · S(onset)` for the floor, `share · S(onset)` for the lost
    /// particles, unless their share follows the decay.
    energy: f64,
    /// The lost particles' share when it follows the decay
    /// ([`EnergySeries::with_lost_share_following_decay`]): at most `share·S(u)` from every `u`.
    following: Option<f64>,
}

/// The most a share `s` of missing energy that follows the decay can move each quantity: the
/// true curve is `S(u)·(1 + a(u))` with `a(u)` anywhere in `[0, s]`, so every level moves by at
/// most `Δ = 10·lg(1 + s)` dB (`docs/params.md`, "Missing energy").
mod following {
    /// Level change, dB.
    pub fn level_db(s: f64) -> f64 {
        10.0 * (1.0 + s).log10()
    }

    /// A decay time, relative, fitted over a range the curve covers `range_db` of: a least-squares
    /// slope over a span `L` moves by at most `3Δ/L` for a level perturbation within `±Δ`, and the
    /// range's ends, moving by `Δ` in level, by at most `6Δ/L` more; relative to a slope that
    /// covers `range_db` over `L`, `9Δ/range_db`.
    pub fn decay_relative(s: f64, range_db: f64) -> f64 {
        9.0 * level_db(s) / range_db
    }

    /// C, dB, from the ratio `r = S(0)/S(te)`: `C = 10·lg(r − 1)`, and `r` scales by a factor in
    /// `[1/(1+s), 1+s]`.
    pub fn clarity_db(s: f64, r: f64) -> f64 {
        let c = |r: f64| 10.0 * (r - 1.0).log10();
        (c(r * (1.0 + s)) - c(r)).max(c(r) - c(r / (1.0 + s)))
    }

    /// D, a fraction: `D = 1 − S(te)/S(0)`, the ratio scaling as above: at most `s·(1 − D)`.
    pub fn definition(s: f64, d: f64) -> f64 {
        s * (1.0 - d)
    }

    /// Ts, s: `∫S du / S(0)` scales by a factor in `[1/(1+s), 1+s]`: at most `s·Ts`.
    pub fn centre_time_s(s: f64, ts: f64) -> f64 {
        s * ts
    }
}

/// A decay so fast that energy added after the series' end sits at its end: a lump there.
const LUMP_RATE: f64 = 1e12;

/// What every parameter shares: the onset, the arrival, the tail and the curves.
struct Analysis<'a> {
    series: &'a EnergySeries,
    onset: Onset,
    /// As given, or checked against the onset bin when it fits it.
    arrival: Arrival,
    /// A given arrival that does not fit the onset bin: C50, C80, D50 and Ts are refused with it.
    arrival_error: Option<ParamError>,
    /// What the decay times are measured from ([`decay_starts`]).
    decay_arrival: Arrival,
    tail: Result<Tail, ParamError>,
    /// `Some` when energy is missing from the series.
    missing: Option<Missing>,
    /// For C50, C80, D50 and Ts: one per arrival, the given one or the two ends of the onset bin.
    /// Empty when the given arrival does not fit.
    views: Vec<View>,
    /// For EDT, T20 and T30, when they are measured from elsewhere than `views`.
    decay_views: Option<Vec<View>>,
}

/// One quantity from one arrival: from the series, with the tail, and with the missing energy as
/// well (`None` when that curve gives no value).
#[derive(Clone, Copy, Debug)]
struct Eval {
    plain: f64,
    tail: Option<f64>,
    missing: Option<f64>,
}

fn relative(a: f64, b: f64) -> f64 {
    (a / b - 1.0).abs()
}

fn absolute(a: f64, b: f64) -> f64 {
    (a - b).abs()
}

impl<'a> Analysis<'a> {
    fn new(series: &'a EnergySeries, given: Arrival) -> Self {
        let dt = series.dt();
        let onset = onset(series);
        let (arrival, arrival_error) = match given.check(series, onset) {
            Ok(a) => (a, None),
            Err(e) => (given, Some(e)),
        };
        let tail = tail_after(series, onset);
        let sums = backward_sums(series.values());
        let end = energy_end(series.values());
        // The curve "with the tail": `Some(None)` for a complete series, the same curve.
        let added = match tail {
            Ok(Tail::Bounded {
                energy,
                ratio_per_bin,
                ..
            }) => Some(Some((energy, -ratio_per_bin.ln() / dt))),
            Ok(Tail::Complete) => Some(None),
            _ => None,
        };
        // Where the curves start: for C, D and Ts the given arrival, which must fit the onset bin,
        // or the two ends of the onset bin; for the decay times the same when it fits, and
        // otherwise `decay_starts`.
        let starts = match (arrival, &arrival_error) {
            (_, Some(_)) => Vec::new(),
            (
                Arrival::Known {
                    time_s,
                    half_width_s,
                },
                None,
            ) => vec![Start::known(time_s, half_width_s, onset.index, dt)],
            (Arrival::Detected, None) => Start::detected(onset).to_vec(),
        };
        let (decay_arrival, decay_start_list) = if arrival_error.is_some() {
            let s = decay_starts(given, onset, dt);
            let a = if s.len() == 1 {
                given
            } else {
                Arrival::Detected
            };
            (a, Some(s))
        } else {
            (arrival, None)
        };
        // The floor's dropped energy: each particle is dropped with at most `10^{db/10}` of its
        // start energy, and what it would still have brought is, on average, what that much energy
        // brings from any particle alive then. From the arrival on the receiver gets `S(onset)`
        // from the `alive_share` of the emitted energy the room still held, so the dropped
        // particles, all together, would have brought at most `10^{db/10}/alive_share` of it.
        // The lost particles' share is given whole: a lump of `share·S(onset)`, or, when it
        // follows the decay, bounded apart (`following`).
        let s0 = sums[onset.index];
        let floor_energy = series
            .floor()
            .map(|f| 10f64.powf(f.db / 10.0) / f.alive_share * s0);
        let follows = series.lost_follows_decay();
        let lost_energy = series
            .lost_share()
            .filter(|_| !follows)
            .map(|share| share * s0);
        let missing = (floor_energy.is_some() || series.lost_share().is_some()).then(|| Missing {
            floor_db: series.floor().map(|f| f.db),
            lost_share: series.lost_share(),
            energy: floor_energy.unwrap_or(0.0) + lost_energy.unwrap_or(0.0),
            following: series.lost_share().filter(|_| follows),
        });
        // The curve with the missing energy: added to every sum, and after the end continued at
        // the tail's rate, or, for a complete series, a lump at its end, the latest it can be.
        let with_missing = match (missing, &tail) {
            (
                Some(m),
                Ok(Tail::Bounded {
                    energy,
                    ratio_per_bin,
                    ..
                }),
            ) if m.energy.is_finite() => Some((energy + m.energy, -ratio_per_bin.ln() / dt)),
            (Some(m), Ok(Tail::Complete)) if m.energy.is_finite() => Some((m.energy, LUMP_RATE)),
            _ => None,
        };
        let curves = |start: Start, early: Early| View {
            arrival_s: start.time_s,
            histogram_from_s: (start.exact_bin as f64 * dt - start.time_s).max(0.0),
            plain: Curve::new(&sums, dt, start, end, None, early),
            with_tail: added.map(|a| Curve::new(&sums, dt, start, end, a, early)),
            with_missing: with_missing.map(|a| Curve::new(&sums, dt, start, end, Some(a), early)),
            later: Vec::new(),
        };
        // With the early reverberation unresolved, each start is read three ways.
        let view = |start: Start| View {
            later: if series.early_reverberation_unresolved() {
                vec![
                    curves(start, Early::FirstBin),
                    curves(start, Early::AfterFirstBin),
                ]
            } else {
                Vec::new()
            },
            ..curves(start, Early::Arrival)
        };
        let views = starts.into_iter().map(view).collect();
        let decay_views = decay_start_list.map(|s| s.into_iter().map(view).collect());
        Analysis {
            series,
            onset,
            arrival,
            arrival_error,
            decay_arrival,
            tail,
            missing,
            views,
            decay_views,
        }
    }

    /// The curves the decay times are read from.
    fn decay_views(&self) -> &[View] {
        self.decay_views.as_deref().unwrap_or(&self.views)
    }

    /// Refuses an onset-relative quantity when the given arrival does not fit the onset bin.
    fn arrival_ok(&self) -> Result<(), ParamError> {
        match &self.arrival_error {
            Some(e) => Err(e.clone()),
            None => Ok(()),
        }
    }

    fn dt(&self) -> f64 {
        self.series.dt()
    }

    /// The error that stops every quantity from being bounded, if the tail could not be
    /// estimated.
    fn tail_ok(&self) -> Result<(), ParamError> {
        self.tail.as_ref().map(|_| ()).map_err(Clone::clone)
    }

    /// The reported value from one [`Pair`] per arrival: the mean over the arrivals of each one's
    /// value, which is midway between its lowest and highest reading when the early reverberation
    /// is unresolved; or a refusal. `truncated` when the tail moves the mean by more than `limit`;
    /// `missing_moves` when the energy missing from the series does; `unresolved` when an end of
    /// the onset bin lies further than `limit` from it; `early_unresolved` when an arrival's lowest
    /// or highest reading does.
    ///
    /// `follows` is the most a lost share that follows the decay can move the value, in the
    /// limit's unit ([`following`]). It is added to what the floor's missing energy moves it by,
    /// and the two together are held to the limit: each is bounded apart, and both can be missing
    /// at once.
    fn settle(
        &self,
        quantity: Quantity,
        pairs: &[Pair],
        limit: f64,
        distance: fn(f64, f64) -> f64,
        follows: Option<f64>,
    ) -> Result<f64, ParamError> {
        let early = |value: f64, continued: f64, span: Option<(f64, f64)>| {
            not_evaluable(
                quantity,
                NotEvaluable::EarlyUnresolved {
                    value,
                    continued,
                    low: span.map(|s| s.0),
                    high: span.map(|s| s.1),
                    limit,
                },
            )
        };
        let finite = |x: f64| x.is_finite().then_some(x);
        let mid = |s: Option<(f64, f64)>| s.map(|(lo, hi)| 0.5 * (lo + hi));
        let mut evals = Vec::with_capacity(pairs.len());
        for p in pairs {
            if p.later.is_empty() {
                evals.push(p.cont);
                continue;
            }
            let Some(span) = p.span(|e| finite(e.plain)) else {
                return Err(early(p.cont.plain, p.cont.plain, None));
            };
            evals.push(Eval {
                plain: 0.5 * (span.0 + span.1),
                tail: mid(p.span(|e| e.tail)),
                missing: mid(p.span(|e| e.missing)),
            });
        }
        let relative = matches!(quantity, Quantity::Edt | Quantity::T20 | Quantity::T30);
        let n = evals.len() as f64;
        let value = evals.iter().map(|e| e.plain).sum::<f64>() / n;
        let with_tail = evals
            .iter()
            .map(|e| e.tail)
            .sum::<Option<f64>>()
            .map(|s| s / n);
        match with_tail {
            Some(w) if distance(value, w) <= limit => {}
            _ => {
                return Err(not_evaluable(
                    quantity,
                    NotEvaluable::Truncated {
                        value,
                        with_tail,
                        limit,
                    },
                ));
            }
        }
        if let Some(m) = self.missing {
            let with_missing = evals
                .iter()
                .map(|e| e.missing)
                .sum::<Option<f64>>()
                .map(|s| s / n);
            // What the floor (and a lost share added as a lump) moves the value by, with the tail,
            // and what a lost share that follows the decay can move it by on top: together.
            let b = follows.unwrap_or(0.0);
            let moved = with_missing.map(|w| distance(value, w) + b);
            match moved {
                Some(d) if d <= limit => {}
                _ => {
                    // The value moved by both, in the direction the floor moves it.
                    let reported = with_missing.zip(moved).map(|(w, d)| {
                        if b == 0.0 {
                            return w;
                        }
                        let s = if w < value { -1.0 } else { 1.0 };
                        if relative {
                            value * (1.0 + s * d)
                        } else {
                            value + s * d
                        }
                    });
                    return Err(not_evaluable(
                        quantity,
                        NotEvaluable::MissingMoves {
                            floor_db: m.floor_db,
                            lost_share: m.lost_share,
                            value,
                            with_missing: reported,
                            limit,
                        },
                    ));
                }
            }
        }
        let low = evals.iter().map(|e| e.plain).fold(f64::INFINITY, f64::min);
        let high = evals
            .iter()
            .map(|e| e.plain)
            .fold(f64::NEG_INFINITY, f64::max);
        if distance(value, low).max(distance(value, high)) > limit {
            return Err(not_evaluable(
                quantity,
                NotEvaluable::Unresolved {
                    value,
                    low,
                    high,
                    limit,
                },
            ));
        }
        // Each arrival's lowest and highest reading, against the value midway between them.
        for (p, e) in pairs.iter().zip(&evals) {
            if p.later.is_empty() {
                continue;
            }
            let span = p.span(|e| finite(e.plain));
            if let Some((lo, hi)) = span
                && distance(e.plain, lo).max(distance(e.plain, hi)) > limit
            {
                return Err(early(e.plain, p.cont.plain, span));
            }
        }
        Ok(value)
    }

    fn decay_time(&self, range: DecayRange) -> Result<DecayFit, ParamError> {
        let q = range.quantity();
        self.tail_ok()?;
        let views = self.decay_views();
        // The level at the arrival is the same from every arrival: the sum from the onset bin.
        let top = views[0].plain.top;
        let reached = match self.tail {
            // How far the decay had fallen when the series ended, the tail included.
            Ok(Tail::Bounded { energy: m, .. }) => Some(10.0 * (m / (top + m)).log10()),
            // Where the curve stops having a shape: the start of the last bin with energy, after
            // which it falls to nothing inside one bin and the fit leaves it out. Without this a
            // range whose bottom lies in that bin would be fitted over its upper part only.
            Ok(Tail::Complete) => {
                let v = self.series.values();
                Some(10.0 * (v[energy_end(v) - 1] / top).log10())
            }
            _ => None,
        };
        if let Some(reached) = reached
            && reached > range.bottom_db()
        {
            return Err(not_evaluable(
                q,
                NotEvaluable::RangeNotReached {
                    needed_db: range.bottom_db(),
                    reached_db: reached,
                },
            ));
        }
        // With energy missing, the decay must also be shown to pass the bottom with all of it and
        // the tail added.
        if let Some(m) = self.missing {
            let unseen = m.energy
                + match self.tail {
                    Ok(Tail::Bounded { energy, .. }) => energy,
                    _ => 0.0,
                };
            let reached = if unseen.is_finite() {
                10.0 * (unseen / (top + unseen)).log10()
            } else {
                0.0
            };
            if reached > range.bottom_db() {
                return Err(not_evaluable(
                    q,
                    NotEvaluable::MissingNotCleared {
                        floor_db: m.floor_db,
                        lost_share: m.lost_share,
                        needed_db: range.bottom_db(),
                        reached_db: reached,
                    },
                ));
            }
        }
        let mut pairs = Vec::with_capacity(views.len());
        let mut span = f64::INFINITY;
        // The level range the fits cover, the smallest from any arrival.
        let mut covered_db = f64::INFINITY;
        let fit = |c: &Option<Curve>| {
            c.as_ref()
                .and_then(|c| c.line(range, self.dt()).ok())
                .map(|l| -60.0 / l.slope)
        };
        // A flat early stretch whose curve gives no value: NaN, refused by `settle`.
        let eval = |v: &View| Eval {
            plain: v
                .plain
                .line(range, self.dt())
                .map_or(f64::NAN, |l| -60.0 / l.slope),
            tail: fit(&v.with_tail),
            missing: fit(&v.with_missing),
        };
        for v in views {
            let own = v
                .plain
                .line(range, self.dt())
                .map_err(|why| not_evaluable(q, why))?;
            pairs.push(v.pair(eval));
            span = span.min(own.span);
            covered_db = covered_db.min(-own.slope * own.span);
        }
        let follows = self
            .following()
            .map(|s| following::decay_relative(s, covered_db));
        let t = self.settle(q, &pairs, limits::DECAY_RELATIVE, relative, follows)?;
        // `settle` passed, so every arrival's fit with the tail exists, from every reading.
        let with_tail_s = pairs
            .iter()
            .map(|p| {
                let (lo, hi) = p.span(|e| e.tail).unwrap();
                0.5 * (lo + hi)
            })
            .sum::<f64>()
            / pairs.len() as f64;
        Ok(DecayFit {
            range,
            t_s: t,
            slope_db_per_s: -60.0 / t,
            span_s: span,
            with_tail_s,
        })
    }

    /// Refused when `te` is not a positive number, or the series ends at or before the window
    /// edge from some arrival.
    fn check_window(&self, quantity: Quantity, te_s: f64) -> Result<(), ParamError> {
        if !te_s.is_finite() || te_s <= 0.0 {
            return Err(ParamError::BadTimeStep { dt: te_s });
        }
        let available = self.series.duration();
        for v in &self.views {
            let edge = v.arrival_s + te_s;
            if available - edge <= 1e-9 * self.dt() {
                return Err(ParamError::SeriesTooShort {
                    what: quantity.to_string(),
                    needed_s: edge,
                    available_s: available,
                });
            }
        }
        Ok(())
    }

    /// `(early, total)` energy of `curve` for a window `te` long from its arrival.
    fn split(curve: &Curve, te_s: f64) -> (f64, f64) {
        (curve.top - curve.at(te_s), curve.top)
    }

    fn clarity_db(&self, te_s: f64) -> Result<f64, ParamError> {
        let q = Quantity::Clarity { te_s };
        self.arrival_ok()?;
        self.check_window(q, te_s)?;
        let c = |curve: &Curve| {
            let (early, total) = Self::split(curve, te_s);
            10.0 * (early / (total - early)).log10()
        };
        let mut pairs = Vec::with_capacity(self.views.len());
        let eval = |v: &View| Eval {
            plain: c(&v.plain),
            tail: v.with_tail.as_ref().map(c),
            missing: v.with_missing.as_ref().map(c),
        };
        for v in &self.views {
            if v.plain.at(te_s) <= 0.0 {
                return Err(not_evaluable(
                    q,
                    NotEvaluable::EmptyWindow {
                        from_s: v.arrival_s + te_s,
                    },
                ));
            }
            pairs.push(v.pair(eval));
        }
        self.tail_ok()?;
        let follows = self.following().map(|s| {
            self.views
                .iter()
                .map(|v| following::clarity_db(s, v.plain.top / v.plain.at(te_s)))
                .fold(0.0, f64::max)
        });
        self.settle(q, &pairs, limits::CLARITY_DB, absolute, follows)
    }

    fn definition(&self, te_s: f64) -> Result<f64, ParamError> {
        let q = Quantity::Definition { te_s };
        self.arrival_ok()?;
        self.check_window(q, te_s)?;
        self.tail_ok()?;
        let d = |curve: &Curve| {
            let (early, total) = Self::split(curve, te_s);
            early / total
        };
        let pairs: Vec<Pair> = self
            .views
            .iter()
            .map(|v| {
                v.pair(|v| Eval {
                    plain: d(&v.plain),
                    tail: v.with_tail.as_ref().map(d),
                    missing: v.with_missing.as_ref().map(d),
                })
            })
            .collect();
        let follows = self.following().map(|s| {
            pairs
                .iter()
                .map(|p| following::definition(s, p.cont.plain))
                .fold(0.0, f64::max)
        });
        self.settle(q, &pairs, limits::DEFINITION, absolute, follows)
    }

    fn centre_time_s(&self) -> Result<f64, ParamError> {
        self.arrival_ok()?;
        self.tail_ok()?;
        let ts = |curve: &Curve| curve.moment() / curve.top;
        let pairs: Vec<Pair> = self
            .views
            .iter()
            .map(|v| {
                v.pair(|v| Eval {
                    plain: ts(&v.plain),
                    tail: v.with_tail.as_ref().map(ts),
                    missing: v.with_missing.as_ref().map(ts),
                })
            })
            .collect();
        // The limit is relative to the value `settle` reports: midway between the readings.
        let value = pairs
            .iter()
            .map(|p| {
                p.span(|e| Some(e.plain))
                    .map_or(p.cont.plain, |s| 0.5 * (s.0 + s.1))
            })
            .sum::<f64>()
            / pairs.len() as f64;
        let limit = limits::CENTRE_TIME_S.min(limits::CENTRE_TIME_RELATIVE * value);
        let follows = self.following().map(|s| {
            pairs
                .iter()
                .map(|p| following::centre_time_s(s, p.cont.plain))
                .fold(0.0, f64::max)
        });
        self.settle(Quantity::CentreTime, &pairs, limit, absolute, follows)
    }

    fn spl_db(&self) -> Result<f64, ParamError> {
        self.tail_ok()?;
        let total = self.series.total();
        let reference = level_reference_pa2();
        let level = |e: f64| 10.0 * (e / reference).log10();
        let tail_energy = match self.tail {
            Ok(Tail::Bounded { energy, .. }) => Some(energy),
            Ok(Tail::Complete) => Some(0.0),
            _ => None,
        };
        let missing = match (tail_energy, self.missing) {
            (Some(t), Some(m)) if m.energy.is_finite() => Some(level(total + t + m.energy)),
            _ => None,
        };
        self.settle(
            Quantity::Spl,
            &[Pair {
                cont: Eval {
                    plain: level(total),
                    tail: tail_energy.map(|t| level(total + t)),
                    missing,
                },
                later: Vec::new(),
            }],
            limits::SPL_DB,
            absolute,
            self.following().map(following::level_db),
        )
    }

    /// The lost share, when it follows the decay.
    fn following(&self) -> Option<f64> {
        self.missing.and_then(|m| m.following)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::params::codes;

    fn series(dt: f64, v: Vec<f64>) -> EnergySeries {
        EnergySeries::new(dt, v).unwrap()
    }

    const AT_ZERO: Arrival = Arrival::Known {
        time_s: 0.0,
        half_width_s: 0.0,
    };

    #[test]
    fn onset_is_the_first_bin_within_20_db_of_the_largest() {
        // 1/100 of the maximum is inside; just below it is not.
        let s = series(0.001, vec![0.0, 0.0099, 0.01, 1.0, 0.5]);
        assert_eq!(
            onset(&s),
            Onset {
                index: 2,
                bin_start_s: 0.002,
                bin_end_s: 0.003,
            }
        );
        let s = series(0.001, vec![0.0, 0.0099, 0.009_999, 1.0, 0.5]);
        assert_eq!(onset(&s).index, 3);
    }

    #[test]
    fn a_given_arrival_must_lie_in_the_onset_bin_for_c_d_and_ts_only() {
        // Onset bin 3, [30, 40) ms.
        let mut v = vec![0.0, 0.0, 0.0, 1.0];
        v.extend((1..400).map(|k| 0.1 * 0.98f64.powi(k)));
        let s = series(0.01, v);
        let used = |t: f64| {
            let p = evaluate(&s, Arrival::at(t));
            assert_eq!(p.arrival, p.decay_arrival);
            match p.arrival {
                Arrival::Known { time_s, .. } => time_s,
                Arrival::Detected => panic!("the given time was dropped"),
            }
        };
        for t in [0.0345, 0.039_999] {
            assert_eq!(used(t), t);
        }
        // The bin's start, and a rounding step before it, are its start.
        let start = onset(&s).bin_start_s;
        assert_eq!(used(start), start);
        assert_eq!(used(start - 1e-15), start);
        // Outside it, or not a time: C50, C80, D50 and Ts are refused; SPL and EDT are not, and
        // with an impulse (no spread) the decay times are measured as if no arrival were given.
        let half_nan = Arrival::spread(0.0345, f64::NAN);
        let half_negative = Arrival::spread(0.0345, -1e-3);
        let wrong = [0.04, 0.05, 0.029, 0.0, -0.01, f64::NAN, f64::INFINITY]
            .map(Arrival::at)
            .into_iter()
            .chain([half_nan, half_negative]);
        for a in wrong {
            let p = evaluate(&s, a);
            for r in [&p.c50_db, &p.c80_db, &p.d50, &p.ts_s] {
                assert_eq!(r.as_ref().unwrap_err().code(), codes::BAD_ARRIVAL, "{a:?}");
            }
            assert!(p.spl_db.is_ok() && p.edt.is_ok(), "{a:?}: {p:?}");
            // The arrival is kept as given (a NaN never equals itself, so compare the text).
            assert_eq!(format!("{:?}", p.arrival), format!("{a:?}"));
            assert_eq!(p.decay_arrival, Arrival::Detected, "{a:?}");
        }
    }

    #[test]
    fn the_tail_is_exact_for_a_geometric_series_and_says_no_otherwise() {
        let q: f64 = 0.9;
        let n = 40;
        let v: Vec<f64> = (0..n).map(|k| q.powi(k)).collect();
        let Tail::Bounded {
            energy,
            ratio_per_bin,
            from_s,
        } = tail(&series(0.01, v)).unwrap()
        else {
            panic!("a decaying series has a bounded tail");
        };
        let exact = q.powi(n) / (1.0 - q);
        assert!((energy / exact - 1.0).abs() < 1e-12, "{energy} vs {exact}");
        assert!((ratio_per_bin - q).abs() < 1e-12);
        assert!((from_s - 0.4).abs() < 1e-12);
        // Trailing empty bins: the tail is estimated from the bins before them, as if the
        // series ended with its last energy.
        let mut v: Vec<f64> = (0..20).map(|k| q.powi(k)).collect();
        v.extend([0.0; 20]);
        let Tail::Bounded { energy, from_s, .. } = tail(&series(0.01, v)).unwrap() else {
            panic!("bounded");
        };
        assert!((energy / (q.powi(20) / (1.0 - q)) - 1.0).abs() < 1e-12);
        assert!((from_s - 0.2).abs() < 1e-12);
        // Not decaying at the end: unbounded.
        let mut v: Vec<f64> = (0..20).map(|k| q.powi(k)).collect();
        v.extend([0.5; 20]);
        assert_eq!(tail(&series(0.01, v)).unwrap(), Tail::Unbounded);
        // One bin after the onset is too short to say anything.
        let e = tail(&series(0.01, vec![0.0, 0.0, 1.0])).unwrap_err();
        assert_eq!(e.code(), codes::SERIES_TOO_SHORT);
    }

    #[test]
    fn a_histogram_that_stops_abruptly_is_refused_not_trusted() {
        // A 1 s decay that stops at −20 dB and is padded with empty bins. Taking the empty bins
        // at their word would put the curve at −∞ and give a T30 from its dive.
        let tau = 1.0 / (6.0 * std::f64::consts::LN_10);
        let mut v: Vec<f64> = (0..333)
            .map(|k| (-(k as f64) * 0.001 / tau).exp())
            .collect();
        v.extend([0.0; 2000]);
        let e = decay_time(&series(0.001, v), AT_ZERO, DecayRange::T30).unwrap_err();
        match e.not_evaluable() {
            Some(NotEvaluable::RangeNotReached { reached_db, .. }) => {
                assert!((reached_db + 20.0).abs() < 0.1, "{reached_db}");
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn an_unbounded_tail_refuses_every_quantity_that_depends_on_it() {
        let q: f64 = 0.95;
        let mut v: Vec<f64> = (0..400).map(|k| q.powi(k)).collect();
        v.extend([0.3; 100]);
        for arrival in [AT_ZERO, Arrival::Detected] {
            let p = evaluate(&series(0.001, v.clone()), arrival);
            assert_eq!(p.tail, Ok(Tail::Unbounded));
            for r in [&p.spl_db, &p.c50_db, &p.c80_db, &p.d50, &p.ts_s] {
                match r.as_ref().unwrap_err().not_evaluable() {
                    Some(NotEvaluable::Truncated {
                        with_tail: None, ..
                    }) => {}
                    other => panic!("{other:?}"),
                }
            }
            for r in [&p.edt, &p.t20, &p.t30] {
                assert_eq!(r.as_ref().unwrap_err().code(), codes::NOT_EVALUABLE);
            }
        }
    }

    #[test]
    fn a_curve_that_jumps_over_a_range_is_too_short_for_it() {
        // A direct pulse far above a short tail: the curve drops from 0 dB to about −35 dB at
        // the arrival, so it spends no time in T20's range.
        let mut v = vec![1.0];
        v.extend((0..200).map(|k| 1e-5 * 0.97f64.powi(k)));
        v.extend([0.0; 50]);
        let e = decay_time(&series(0.001, v), AT_ZERO, DecayRange::T20).unwrap_err();
        assert!(
            matches!(
                e.not_evaluable(),
                Some(NotEvaluable::RangeTooShort { span_s, needed_s })
                    if *span_s == 0.0 && (*needed_s - 0.002).abs() < 1e-15
            ),
            "{e}"
        );
    }

    #[test]
    fn clarity_needs_the_series_to_pass_its_window_and_late_energy() {
        // 50 bins of 1 ms: the series ends at 50 ms, before C80's edge.
        let v: Vec<f64> = (0..50).map(|k| 0.9f64.powi(k)).collect();
        let e = clarity_db(&series(0.001, v.clone()), AT_ZERO, 0.08).unwrap_err();
        assert_eq!(e.code(), codes::SERIES_TOO_SHORT);
        // Energy only before 80 ms, then silence: no late energy.
        let mut w = v;
        w.extend([0.0; 100]);
        let e = clarity_db(&series(0.001, w), AT_ZERO, 0.08).unwrap_err();
        assert!(
            matches!(e.not_evaluable(), Some(NotEvaluable::EmptyWindow { .. })),
            "{e}"
        );
        // A window length that is not a positive number.
        let v: Vec<f64> = (0..500).map(|k| 0.99f64.powi(k)).collect();
        for te in [0.0, -0.05, f64::NAN] {
            assert_eq!(
                clarity_db(&series(0.001, v.clone()), AT_ZERO, te)
                    .unwrap_err()
                    .code(),
                codes::BAD_TIME_STEP
            );
        }
    }

    #[test]
    fn a_band_serialises_values_and_refusals_alike() {
        // A 1 s decay cut at 30 dB: T30 refused, EDT a value.
        let tau = 1.0 / (6.0 * std::f64::consts::LN_10);
        let v: Vec<f64> = (0..500)
            .map(|k| (-(k as f64) * 0.001 / tau).exp())
            .collect();
        let p = evaluate(&series(0.001, v), AT_ZERO);
        let json = serde_json::to_value(&p).unwrap();
        assert!(json["edt"]["Ok"]["t_s"].is_f64(), "{json}");
        assert_eq!(json["arrival"]["arrival"], "known");
        assert_eq!(json["t30"]["Err"]["kind"], "not_evaluable");
        assert_eq!(json["t30"]["Err"]["quantity"]["quantity"], "t30");
        assert_eq!(json["t30"]["Err"]["why"]["why"], "range_not_reached");
    }

    #[test]
    fn a_window_edge_inside_a_bin_splits_it_as_the_curve_runs() {
        // Bins of 10 ms holding 1 and 1, then a steep fall so the tail is negligible. The next
        // bin's decay continued back would be far more than bin 0 holds, so bin 0 holds no direct
        // sound: its curve runs log-linear from S = 2 to S = 1, and at 5 ms S = √2.
        let mut v = vec![1.0, 1.0, 1e-9, 1e-18];
        v.extend([0.0; 16]);
        let s = series(0.01, v);
        let early = 2.0 - 2f64.sqrt();
        let c = clarity_db(&s, AT_ZERO, 0.005).unwrap();
        assert!(
            (c - 10.0 * (early / 2f64.sqrt()).log10()).abs() < 1e-8,
            "{c}"
        );
        let d = definition(&s, AT_ZERO, 0.005).unwrap();
        assert!((d - early / 2.0).abs() < 1e-8, "{d}");
        // Detected, the arrival could be anywhere in bin 0, and the 5 ms window with it.
        let e = clarity_db(&s, Arrival::Detected, 0.005).unwrap_err();
        assert!(
            matches!(e.not_evaluable(), Some(NotEvaluable::Unresolved { .. })),
            "{e}"
        );
    }

    #[test]
    fn a_direct_sound_is_an_impulse_and_the_rest_of_its_bin_continues_the_decay() {
        // An impulse of 3 at 25 ms, then bins falling by half: the bin of the impulse holds the
        // impulse plus the decay continued back (1 for half a bin at this rate: 2^0.5 − 1 of the
        // next sum). The curve right after the arrival sits below its top by the impulse.
        let dt = 0.01;
        let mut v = vec![0.0, 0.0];
        let continued = 2.0 * (2f64.sqrt() - 1.0);
        v.push(3.0 + continued);
        v.extend((0..60).map(|k| 0.5f64.powi(k)));
        let s = series(dt, v);
        let a = Analysis::new(&s, Arrival::at(0.025));
        let c = &a.views[0].plain;
        let next = 2.0 - 0.5f64.powi(59);
        assert!((c.top - (3.0 + continued + next)).abs() < 1e-12);
        assert!((c.at(1e-12) - (continued + next)).abs() < 1e-9);
        // The decay after the impulse is the same slope throughout: EDT from it is exact.
        let t = -60.0 / (10.0 * 0.5f64.log10() / dt);
        let edt = decay_time(&s, Arrival::at(0.025), DecayRange::Edt).unwrap();
        assert!((edt.t_s / t - 1.0).abs() < 1e-9, "{} vs {t}", edt.t_s);
    }

    #[test]
    fn a_direct_sound_spread_past_its_bin_is_taken_whole_as_the_impulse() {
        // The same impulse of 3, now at 29.5 ms and spread over ±1 ms: 2/3 of it falls in bin 2,
        // 1/3 in bin 3. Bin 3 then holds 1 of direct sound and 1 of decay, and bins 4 on fall by
        // half. Given the spread, the curve from bin 4 on is the histogram's and before it the
        // decay continued back: EDT is exact. Taken as an impulse (no spread), bin 3 reads as a
        // decay twice as steep and EDT comes out short.
        let dt = 0.01;
        let decay_in = |a: f64, b: f64| 2.0 * (0.5f64.powf(a / dt) - 0.5f64.powf(b / dt));
        let t_a = 0.0295;
        let mut v = vec![0.0, 0.0, 2.0 + decay_in(0.0, 0.0005)];
        v.push(1.0 + decay_in(0.0005, 0.0105));
        v.extend((0..60).map(|k| decay_in(0.0105 + k as f64 * dt, 0.0205 + k as f64 * dt)));
        let s = series(dt, v);
        let t = -60.0 / (10.0 * 0.5f64.log10() / dt);
        let spread = decay_time(&s, Arrival::spread(t_a, 0.001), DecayRange::Edt).unwrap();
        assert!((spread.t_s / t - 1.0).abs() < 1e-9, "{} vs {t}", spread.t_s);
        let impulse = decay_time(&s, Arrival::at(t_a), DecayRange::Edt).unwrap();
        assert!(impulse.t_s / t - 1.0 < -0.05, "{} vs {t}", impulse.t_s);
    }

    /// The thinned polyline's level at `u`, straight between its points.
    fn level_on(points: &[[f64; 2]], u: f64) -> f64 {
        let i = points.iter().rposition(|p| p[0] <= u).unwrap();
        match points.get(i + 1) {
            Some(q) if q[0] > points[i][0] => {
                let p = points[i];
                p[1] + (q[1] - p[1]) * (u - p[0]) / (q[0] - p[0])
            }
            _ => points[i][1],
        }
    }

    #[test]
    fn the_decay_curve_is_the_fitted_curve_thinned_within_its_tolerance() {
        // A direct sound of 1 in bin 2 at 25 ms (an impulse), then a decay halving every bin: the
        // curve steps down at the arrival and then falls in one straight line, bending only in
        // its last bins, below −150 dB, where the series' end cuts the backward sums short. So
        // one line stands for 150 dB of it.
        let dt = 0.01;
        let mut v = vec![0.0, 0.0];
        let continued = 2.0 * (2f64.sqrt() - 1.0);
        v.push(1.0 + continued);
        v.extend((0..60).map(|k| 0.5f64.powi(k)));
        let s = series(dt, v);
        let c = decay_curve(&s, Arrival::at(0.025));
        assert_eq!(c.from_s, 0.025);
        assert_eq!(c.tolerance_db, CURVE_TOLERANCE_DB);
        assert!((c.histogram_from_s - 0.005).abs() < 1e-12);
        assert_eq!(c.points[0], [0.0, 0.0]);
        assert_eq!(c.points[1][0], 0.0);
        // The step is the direct sound: 10·lg(S just after / S at 0).
        let top = 1.0 + continued + (2.0 - 0.5f64.powi(59));
        let after = continued + (2.0 - 0.5f64.powi(59));
        assert!((c.points[1][1] - 10.0 * (after / top).log10()).abs() < 1e-9);
        // One line to below −150 dB, whose slope is the decay the fits read: EDT from it is EDT.
        assert!(c.points[2][1] < -150.0, "{:?}", c.points);
        let slope = (c.points[2][1] - c.points[1][1]) / (c.points[2][0] - c.points[1][0]);
        let edt = decay_time(&s, Arrival::at(0.025), DecayRange::Edt).unwrap();
        assert!((-60.0 / slope / edt.t_s - 1.0).abs() < 1e-3, "{slope}");
        // It ends at the start of the last bin with energy.
        let last_start = (s.len() - 1) as f64 * dt - 0.025;
        assert!((c.points.last().unwrap()[0] - last_start).abs() < 1e-12);
        assert_eq!(c.knots, 2 + 60);
        assert!(c.points.len() < 15, "{:?}", c.points);

        // A double slope: every knot of the whole curve lies within the tolerance of the thinned
        // line, and the thinning keeps the knee. Says no: with no tolerance at all the curved
        // stretch keeps its knots, and a knot moved by twice the tolerance is off the line.
        let knee = 0.3;
        let w: Vec<f64> = (0..400)
            .map(|k| {
                let t = k as f64 * 0.001;
                if t < knee {
                    (-t / 0.02).exp()
                } else {
                    (-knee / 0.02).exp() * (-(t - knee) / 0.1).exp()
                }
            })
            .collect();
        let s = series(0.001, w);
        let a = Analysis::new(&s, Arrival::at(0.0));
        let knots = a.decay_views()[0].plain.knots();
        let c = decay_curve(&s, Arrival::at(0.0));
        assert_eq!(c.knots, knots.len());
        assert!(
            c.points.len() < knots.len() / 4,
            "{} points",
            c.points.len()
        );
        for k in &knots {
            let d = (level_on(&c.points, k[0]) - k[1]).abs();
            assert!(d <= CURVE_TOLERANCE_DB * (1.0 + 1e-9), "{k:?}: {d}");
        }
        assert!(
            c.points.iter().any(|p| (p[0] - knee).abs() < 0.01),
            "the knee is kept: {:?}",
            c.points
        );
        assert!(thin(&knots, 0.0).len() > 2 * c.points.len());
        let mut moved = c.points.clone();
        let mid = moved.len() / 2;
        moved[mid][1] += 2.0 * CURVE_TOLERANCE_DB;
        assert!(
            knots
                .iter()
                .any(|k| (level_on(&moved, k[0]) - k[1]).abs() > CURVE_TOLERANCE_DB)
        );
    }

    #[test]
    fn spl_is_the_sum_over_p0_squared() {
        // 4e-10 Pa² in total is 0 dB; 4e-8 is 20 dB. The last bins fall steeply, so the tail
        // is negligible.
        let mut v = vec![3e-8, 0.99e-8, 0.01e-8, 1e-20];
        v.extend([0.0; 10]);
        let spl = spl_db(&series(0.01, v)).unwrap();
        assert!((spl - 20.0).abs() < 1e-9, "{spl}");
        // Night Mode's +26 dB bug, dividing by 1e-12 instead: the gap is 26.02 dB.
        assert!((10.0 * (P_REF_SQUARED / 1e-12).log10() - 26.0206).abs() < 1e-4);
    }
}
