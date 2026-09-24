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
//! **Two unknowns, bounded.** The energy after the series' end ([`Tail`]); and, when the arrival
//! is not given, where in the onset bin it lies. Each quantity is computed with and without the
//! tail, and with the arrival at each end of the onset bin. When either moves it by more than
//! its limit ([`limits`]), it is refused, as `truncated` or `unresolved`. No alternative value is
//! ever reported as the quantity. A series its caller knows to be complete
//! ([`EnergySeries::complete`]) has no tail: [`Tail::Complete`] adds nothing and refuses nothing.

use schemars::JsonSchema;
use serde::Serialize;

use super::{EnergySeries, NotEvaluable, ParamError, Quantity, not_evaluable};

/// `p₀² = (20 µPa)²`, Pa².
pub const P_REF_SQUARED: f64 = 20e-6 * 20e-6;

/// The onset is the first bin within this many dB of the largest (ISO 3382-1's 20 dB rule, as
/// commonly stated).
pub const ONSET_THRESHOLD_DB: f64 = 20.0;

/// A decay regression needs the curve to spend at least this many bin widths inside its range.
pub const MIN_REGRESSION_BINS: f64 = 2.0;

/// `|100·(T30/T20 − 1)|` above this marks a curved decay (ISO 3382-2's 10 %, as commonly stated).
pub const CURVATURE_LIMIT_PERCENT: f64 = 10.0;

/// How far an unknown the histogram cannot show (the tail after its end, the arrival inside the
/// onset bin) may move each quantity before it is refused: the tighter of 1/10 of the difference
/// limen commonly quoted from ISO 3382-1 Annex A and gate (a)'s bound (`docs/params.md`,
/// "Truncation").
pub mod limits {
    /// EDT, T20, T30: relative. 1/10 of the 5 % limen, and gate (a)'s bound.
    pub const DECAY_RELATIVE: f64 = 0.005;
    /// C50, C80: dB. Gate (a)'s bound, tighter than 1/10 of the 1 dB limen.
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
    /// Given, such as the source–receiver distance over the speed of sound. It must lie in the
    /// onset bin; otherwise the call is refused, `params_bad_arrival`.
    Known { time_s: f64 },
}

impl Arrival {
    /// A given time, checked against the onset bin `o` of `series`.
    fn check(self, series: &EnergySeries, o: Onset) -> Result<Arrival, ParamError> {
        let Arrival::Known { time_s } = self else {
            return Ok(self);
        };
        let bad = |detail: String| Err(ParamError::BadArrival { time_s, detail });
        if !time_s.is_finite() || time_s < 0.0 {
            return bad("not a finite time at or after 0 s".into());
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
        })
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

/// A decay time of `series` (`docs/params.md`, "Decay times").
pub fn decay_time(
    series: &EnergySeries,
    arrival: Arrival,
    range: DecayRange,
) -> Result<DecayFit, ParamError> {
    Analysis::new(series, arrival)?.decay_time(range)
}

/// C_te in dB, `te` in seconds (C50: 0.05, C80: 0.08). A `te` that is not a positive number is
/// refused as `params_bad_time_step`.
pub fn clarity_db(series: &EnergySeries, arrival: Arrival, te_s: f64) -> Result<f64, ParamError> {
    Analysis::new(series, arrival)?.clarity_db(te_s)
}

/// D_te as a fraction, `te` in seconds (D50: 0.05).
pub fn definition(series: &EnergySeries, arrival: Arrival, te_s: f64) -> Result<f64, ParamError> {
    Analysis::new(series, arrival)?.definition(te_s)
}

/// The centre time Ts from the arrival, s.
pub fn centre_time_s(series: &EnergySeries, arrival: Arrival) -> Result<f64, ParamError> {
    Analysis::new(series, arrival)?.centre_time_s()
}

/// The sound pressure level of every bin summed, dB re 20 µPa. It does not depend on the arrival.
pub fn spl_db(series: &EnergySeries) -> Result<f64, ParamError> {
    Analysis::new(series, Arrival::Detected)?.spl_db()
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
    /// The arrival the onset-relative quantities are measured from.
    pub arrival: Arrival,
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

/// Every parameter of one band's series. Refused as a whole only for a given arrival that does
/// not fit the series (`params_bad_arrival`).
pub fn evaluate(series: &EnergySeries, arrival: Arrival) -> Result<BandParameters, ParamError> {
    let a = Analysis::new(series, arrival)?;
    let t20 = a.decay_time(DecayRange::T20);
    let t30 = a.decay_time(DecayRange::T30);
    let curvature = match (&t20, &t30) {
        (Ok(a), Ok(b)) => Ok(curvature(a, b)),
        (Err(e), _) | (_, Err(e)) => Err(match e {
            ParamError::NotEvaluable { why, .. } => not_evaluable(Quantity::Curvature, why.clone()),
            other => other.clone(),
        }),
    };
    Ok(BandParameters {
        onset: a.onset,
        arrival: a.arrival,
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
    })
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

impl Curve {
    /// The curve of the backward sums `sums` with the arrival at `t_a` in onset bin `k0`, up to
    /// `end`, one past the last bin with energy; with `tail`, `M` is added to every sum and the
    /// curve goes on past `end`.
    fn new(
        sums: &[f64],
        dt: f64,
        k0: usize,
        end: usize,
        t_a: f64,
        tail: Option<(f64, f64)>,
    ) -> Curve {
        let m = tail.map_or(0.0, |t| t.0);
        let s = |k: usize| sums[k] + m;
        let shape = |s1: f64| {
            if s1 > 0.0 {
                Shape::LogLinear
            } else {
                Shape::Linear
            }
        };
        let top = s(k0);
        let mut pieces = Vec::with_capacity(end.saturating_sub(k0));
        // The rest of the onset bin after the arrival: the next bin's decay continued back to the
        // arrival, at most the whole bin; the rest of the bin is the direct sound, at u = 0.
        let u1 = (k0 + 1) as f64 * dt - t_a;
        if u1 > 0.0 {
            let next = s(k0 + 1);
            let continued = if k0 + 2 <= end && next > 0.0 && s(k0 + 2) > 0.0 {
                next * ((next / s(k0 + 2)).powf(u1 / dt) - 1.0)
            } else {
                f64::INFINITY
            };
            pieces.push(Piece {
                u0: 0.0,
                u1,
                s0: next + continued.min(top - next),
                s1: next,
                shape: shape(next),
            });
        }
        for k in k0 + 1..end {
            pieces.push(Piece {
                u0: k as f64 * dt - t_a,
                u1: (k + 1) as f64 * dt - t_a,
                s0: s(k),
                s1: s(k + 1),
                shape: shape(s(k + 1)),
            });
        }
        Curve { top, pieces, tail }
    }

    fn end(&self) -> f64 {
        self.pieces.last().map_or(0.0, |p| p.u1)
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

/// The curve from one arrival, and the same with the unseen tail added.
struct View {
    arrival_s: f64,
    plain: Curve,
    /// `None` when the tail is unbounded.
    with_tail: Option<Curve>,
}

/// What every parameter shares: the onset, the arrival, the tail and the curves.
struct Analysis<'a> {
    series: &'a EnergySeries,
    onset: Onset,
    arrival: Arrival,
    tail: Result<Tail, ParamError>,
    /// One per arrival: the given one, or the two ends of the onset bin.
    views: Vec<View>,
}

fn relative(a: f64, b: f64) -> f64 {
    (a / b - 1.0).abs()
}

fn absolute(a: f64, b: f64) -> f64 {
    (a - b).abs()
}

impl<'a> Analysis<'a> {
    fn new(series: &'a EnergySeries, arrival: Arrival) -> Result<Self, ParamError> {
        let dt = series.dt();
        let onset = onset(series);
        let arrival = arrival.check(series, onset)?;
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
        let times = match arrival {
            Arrival::Known { time_s } => vec![time_s],
            Arrival::Detected => vec![onset.bin_start_s, onset.bin_end_s],
        };
        let views = times
            .into_iter()
            .map(|t| View {
                arrival_s: t,
                plain: Curve::new(&sums, dt, onset.index, end, t, None),
                with_tail: added.map(|a| Curve::new(&sums, dt, onset.index, end, t, a)),
            })
            .collect();
        Ok(Analysis {
            series,
            onset,
            arrival,
            tail,
            views,
        })
    }

    fn dt(&self) -> f64 {
        self.series.dt()
    }

    /// The error that stops every quantity from being bounded, if the tail could not be
    /// estimated.
    fn tail_ok(&self) -> Result<(), ParamError> {
        self.tail.as_ref().map(|_| ()).map_err(Clone::clone)
    }

    /// The reported value from one `(value, value with the tail)` per arrival: their mean, or a
    /// refusal. `truncated` when the tail moves the mean by more than `limit`; `unresolved` when
    /// an end of the onset bin lies further than `limit` from it.
    fn settle(
        quantity: Quantity,
        evals: &[(f64, Option<f64>)],
        limit: f64,
        distance: fn(f64, f64) -> f64,
    ) -> Result<f64, ParamError> {
        let n = evals.len() as f64;
        let value = evals.iter().map(|e| e.0).sum::<f64>() / n;
        let with_tail = evals
            .iter()
            .map(|e| e.1)
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
        let low = evals.iter().map(|e| e.0).fold(f64::INFINITY, f64::min);
        let high = evals.iter().map(|e| e.0).fold(f64::NEG_INFINITY, f64::max);
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
        Ok(value)
    }

    fn decay_time(&self, range: DecayRange) -> Result<DecayFit, ParamError> {
        let q = range.quantity();
        self.tail_ok()?;
        // The level at the arrival is the same from every arrival.
        let top = self.views[0].plain.top;
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
        let mut evals = Vec::with_capacity(self.views.len());
        let mut span = f64::INFINITY;
        for v in &self.views {
            let own = v
                .plain
                .line(range, self.dt())
                .map_err(|why| not_evaluable(q, why))?;
            let with_tail = v
                .with_tail
                .as_ref()
                .and_then(|c| c.line(range, self.dt()).ok())
                .map(|l| -60.0 / l.slope);
            evals.push((-60.0 / own.slope, with_tail));
            span = span.min(own.span);
        }
        let t = Self::settle(q, &evals, limits::DECAY_RELATIVE, relative)?;
        // `settle` passed, so every arrival's fit with the tail exists.
        let with_tail_s = evals.iter().map(|e| e.1.unwrap()).sum::<f64>() / evals.len() as f64;
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
        self.check_window(q, te_s)?;
        let c = |curve: &Curve| {
            let (early, total) = Self::split(curve, te_s);
            10.0 * (early / (total - early)).log10()
        };
        let mut evals = Vec::with_capacity(self.views.len());
        for v in &self.views {
            if v.plain.at(te_s) <= 0.0 {
                return Err(not_evaluable(
                    q,
                    NotEvaluable::EmptyWindow {
                        from_s: v.arrival_s + te_s,
                    },
                ));
            }
            evals.push((c(&v.plain), v.with_tail.as_ref().map(c)));
        }
        self.tail_ok()?;
        Self::settle(q, &evals, limits::CLARITY_DB, absolute)
    }

    fn definition(&self, te_s: f64) -> Result<f64, ParamError> {
        let q = Quantity::Definition { te_s };
        self.check_window(q, te_s)?;
        self.tail_ok()?;
        let d = |curve: &Curve| {
            let (early, total) = Self::split(curve, te_s);
            early / total
        };
        let evals: Vec<_> = self
            .views
            .iter()
            .map(|v| (d(&v.plain), v.with_tail.as_ref().map(d)))
            .collect();
        Self::settle(q, &evals, limits::DEFINITION, absolute)
    }

    fn centre_time_s(&self) -> Result<f64, ParamError> {
        self.tail_ok()?;
        let ts = |curve: &Curve| curve.moment() / curve.top;
        let evals: Vec<_> = self
            .views
            .iter()
            .map(|v| (ts(&v.plain), v.with_tail.as_ref().map(ts)))
            .collect();
        let value = evals.iter().map(|e| e.0).sum::<f64>() / evals.len() as f64;
        let limit = limits::CENTRE_TIME_S.min(limits::CENTRE_TIME_RELATIVE * value);
        Self::settle(Quantity::CentreTime, &evals, limit, absolute)
    }

    fn spl_db(&self) -> Result<f64, ParamError> {
        self.tail_ok()?;
        let total = self.series.total();
        let level = |e: f64| 10.0 * (e / P_REF_SQUARED).log10();
        let with_tail = match self.tail {
            Ok(Tail::Bounded { energy, .. }) => Some(level(total + energy)),
            Ok(Tail::Complete) => Some(level(total)),
            _ => None,
        };
        Self::settle(
            Quantity::Spl,
            &[(level(total), with_tail)],
            limits::SPL_DB,
            absolute,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::params::codes;

    fn series(dt: f64, v: Vec<f64>) -> EnergySeries {
        EnergySeries::new(dt, v).unwrap()
    }

    const AT_ZERO: Arrival = Arrival::Known { time_s: 0.0 };

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
    fn a_given_arrival_must_lie_in_the_onset_bin() {
        // Onset bin 3, [30, 40) ms.
        let mut v = vec![0.0, 0.0, 0.0, 1.0];
        v.extend((1..400).map(|k| 0.1 * 0.98f64.powi(k)));
        let s = series(0.01, v);
        let used = |t: f64| match evaluate(&s, Arrival::Known { time_s: t }).unwrap().arrival {
            Arrival::Known { time_s } => time_s,
            Arrival::Detected => panic!("the given time was dropped"),
        };
        for t in [0.0345, 0.039_999] {
            assert_eq!(used(t), t);
        }
        // The bin's start, and a rounding step before it, are its start.
        let start = onset(&s).bin_start_s;
        assert_eq!(used(start), start);
        assert_eq!(used(start - 1e-15), start);
        for t in [0.04, 0.05, 0.029, 0.0, -0.01, f64::NAN, f64::INFINITY] {
            let e = evaluate(&s, Arrival::Known { time_s: t }).unwrap_err();
            assert_eq!(e.code(), codes::BAD_ARRIVAL, "{t}: {e}");
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
            let p = evaluate(&series(0.001, v.clone()), arrival).unwrap();
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
        let p = evaluate(&series(0.001, v), AT_ZERO).unwrap();
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
        let a = Analysis::new(&s, Arrival::Known { time_s: 0.025 }).unwrap();
        let c = &a.views[0].plain;
        let next = 2.0 - 0.5f64.powi(59);
        assert!((c.top - (3.0 + continued + next)).abs() < 1e-12);
        assert!((c.at(1e-12) - (continued + next)).abs() < 1e-9);
        // The decay after the impulse is the same slope throughout: EDT from it is exact.
        let t = -60.0 / (10.0 * 0.5f64.log10() / dt);
        let edt = decay_time(&s, Arrival::Known { time_s: 0.025 }, DecayRange::Edt).unwrap();
        assert!((edt.t_s / t - 1.0).abs() < 1e-9, "{} vs {t}", edt.t_s);
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
