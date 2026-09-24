//! Per-band parameters from an energy histogram: onset, Schroeder curve, EDT, T20, T30, C50,
//! C80, D50, Ts and SPL (`docs/params.md`).
//!
//! **The tail bound.** A series ends; the energy after its end is unseen. Every parameter is
//! computed from the series alone, and again with an estimate of that tail added after the end
//! ([`Tail`]). When the two differ by more than the quantity's limit ([`limits`]), the quantity is
//! refused as `truncated`. The value with the tail is never reported as the quantity.

use schemars::JsonSchema;
use serde::Serialize;

use super::{EnergySeries, NotEvaluable, ParamError, Quantity, not_evaluable};

/// `p₀² = (20 µPa)²`, Pa².
pub const P_REF_SQUARED: f64 = 20e-6 * 20e-6;

/// The onset is the first bin within this many dB of the largest (ISO 3382-1's 20 dB rule, as
/// commonly stated).
pub const ONSET_THRESHOLD_DB: f64 = 20.0;

/// A decay regression needs at least this many points in its range.
pub const MIN_REGRESSION_POINTS: usize = 3;

/// `|100·(T30/T20 − 1)|` above this marks a curved decay (ISO 3382-2's 10 %, as commonly stated).
pub const CURVATURE_LIMIT_PERCENT: f64 = 10.0;

/// How far the unseen tail may move each quantity before it is refused: 1/10 of the difference
/// limens commonly quoted from ISO 3382-1 Annex A (`docs/params.md`, "Truncation").
pub mod limits {
    /// EDT, T20, T30: relative.
    pub const DECAY_RELATIVE: f64 = 0.005;
    /// C50, C80: dB.
    pub const CLARITY_DB: f64 = 0.1;
    /// D50: fraction (0.5 percentage points).
    pub const DEFINITION: f64 = 0.005;
    /// Ts: s.
    pub const CENTRE_TIME_S: f64 = 0.001;
    /// SPL: dB.
    pub const SPL_DB: f64 = 0.1;
}

/// The direct-sound arrival: the first bin whose energy is within [`ONSET_THRESHOLD_DB`] of the
/// largest bin, and the start time of that bin.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, JsonSchema)]
pub struct Onset {
    pub index: usize,
    pub time_s: f64,
}

/// The direct-sound arrival of `series` (`docs/params.md`, "Direct-arrival detection").
pub fn onset(series: &EnergySeries) -> Onset {
    let v = series.values();
    let max = v.iter().copied().fold(0.0_f64, f64::max);
    let threshold = max / 10f64.powf(ONSET_THRESHOLD_DB / 10.0);
    // A series holds energy (EnergySeries::new), so some bin reaches the threshold.
    let index = v.iter().position(|&e| e >= threshold && e > 0.0).unwrap();
    Onset {
        index,
        time_s: index as f64 * series.dt(),
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
}

impl Tail {
    fn energy(&self) -> Option<f64> {
        match *self {
            Tail::Bounded { energy, .. } => Some(energy),
            Tail::Unbounded => None,
        }
    }
}

/// The tail estimate of `series`: two windows of `w` bins ending at the last bin with energy,
/// `w` a tenth of the bins from the onset to there and at least 1 (`docs/params.md`,
/// "Truncation"). Refused when fewer than 2 bins from the onset hold energy up to the last.
pub fn tail(series: &EnergySeries) -> Result<Tail, ParamError> {
    tail_after(series, onset(series))
}

/// One past the last bin with energy.
fn energy_end(v: &[f64]) -> usize {
    // A series holds energy (EnergySeries::new).
    v.iter().rposition(|&e| e > 0.0).unwrap() + 1
}

fn tail_after(series: &EnergySeries, o: Onset) -> Result<Tail, ParamError> {
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

/// The Schroeder decay curve from the onset: `(k·dt, 10·lg(S_k / S_{k₀}))` for every bin from
/// the onset to the last, with nothing added for the unseen tail. A level after the last energy
/// is `-inf`.
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
    /// The line's level at `t = 0`, dB.
    pub intercept_db: f64,
    /// Points in the range.
    pub points: usize,
    /// The same fit on the curve with the unseen tail added. A check, never the value.
    pub with_tail_s: f64,
}

/// A decay time of `series` (`docs/params.md`, "Decay times").
pub fn decay_time(series: &EnergySeries, range: DecayRange) -> Result<DecayFit, ParamError> {
    Analysis::new(series).decay_time(range)
}

/// C_te in dB, `te` in seconds (C50: 0.05, C80: 0.08). A `te` that is not a positive number is
/// refused as `params_bad_time_step`.
pub fn clarity_db(series: &EnergySeries, te_s: f64) -> Result<f64, ParamError> {
    Analysis::new(series).clarity_db(te_s)
}

/// D_te as a fraction, `te` in seconds (D50: 0.05).
pub fn definition(series: &EnergySeries, te_s: f64) -> Result<f64, ParamError> {
    Analysis::new(series).definition(te_s)
}

/// The centre time Ts from the onset, s.
pub fn centre_time_s(series: &EnergySeries) -> Result<f64, ParamError> {
    Analysis::new(series).centre_time_s()
}

/// The sound pressure level of every bin summed, dB re 20 µPa.
pub fn spl_db(series: &EnergySeries) -> Result<f64, ParamError> {
    Analysis::new(series).spl_db()
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
pub fn evaluate(series: &EnergySeries) -> BandParameters {
    let a = Analysis::new(series);
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

/// What every parameter shares: the onset, the tail and the backward sums.
struct Analysis<'a> {
    series: &'a EnergySeries,
    onset: Onset,
    tail: Result<Tail, ParamError>,
    sums: Vec<f64>,
    /// One past the last bin with energy: where the tail is added.
    energy_end: usize,
}

/// A least-squares line through `(t, level)` points.
struct Line {
    slope: f64,
    intercept: f64,
    points: usize,
}

fn fit(points: &[(f64, f64)]) -> Option<Line> {
    let n = points.len();
    if n < 2 {
        return None;
    }
    let nf = n as f64;
    let mt = points.iter().map(|p| p.0).sum::<f64>() / nf;
    let ml = points.iter().map(|p| p.1).sum::<f64>() / nf;
    let (mut stt, mut stl) = (0.0, 0.0);
    for &(t, l) in points {
        stt += (t - mt) * (t - mt);
        stl += (t - mt) * (l - ml);
    }
    if stt == 0.0 {
        return None;
    }
    let slope = stl / stt;
    Some(Line {
        slope,
        intercept: ml - slope * mt,
        points: n,
    })
}

impl<'a> Analysis<'a> {
    fn new(series: &'a EnergySeries) -> Self {
        let onset = onset(series);
        Analysis {
            series,
            onset,
            tail: tail_after(series, onset),
            sums: backward_sums(series.values()),
            energy_end: energy_end(series.values()),
        }
    }

    fn dt(&self) -> f64 {
        self.series.dt()
    }

    /// The tail's energy, or the error that stops every quantity from being bounded.
    fn tail_energy(&self) -> Result<Option<f64>, ParamError> {
        match &self.tail {
            Ok(t) => Ok(t.energy()),
            Err(e) => Err(e.clone()),
        }
    }

    /// `value` against `with_tail` under `limit`: the value, or `truncated`.
    fn bounded(
        quantity: Quantity,
        value: f64,
        with_tail: Option<f64>,
        limit: f64,
        differs: impl Fn(f64, f64) -> bool,
    ) -> Result<f64, ParamError> {
        match with_tail {
            Some(t) if !differs(value, t) => Ok(value),
            _ => Err(not_evaluable(
                quantity,
                NotEvaluable::Truncated {
                    value,
                    with_tail,
                    limit,
                },
            )),
        }
    }

    /// The points of the curve `sums + extra` in `range`, from the onset. With `extra`, the
    /// tail, added after the last bin with energy, the curve ends at that bin's end.
    fn range_points(&self, range: DecayRange, extra: f64) -> Vec<(f64, f64)> {
        let k0 = self.onset.index;
        let s0 = self.sums[k0] + extra;
        let last = if extra > 0.0 {
            self.energy_end
        } else {
            self.energy_end - 1
        };
        (k0..=last)
            .filter_map(|k| {
                let l = 10.0 * ((self.sums[k] + extra) / s0).log10();
                (l <= range.top_db() && l >= range.bottom_db()).then_some((k as f64 * self.dt(), l))
            })
            .collect()
    }

    fn line(&self, range: DecayRange, extra: f64) -> Result<Line, ParamError> {
        let points = self.range_points(range, extra);
        if points.len() < MIN_REGRESSION_POINTS {
            return Err(not_evaluable(
                range.quantity(),
                NotEvaluable::TooFewPoints {
                    points: points.len(),
                    needed: MIN_REGRESSION_POINTS,
                },
            ));
        }
        match fit(&points) {
            Some(line) if line.slope < 0.0 => Ok(line),
            _ => Err(not_evaluable(range.quantity(), NotEvaluable::NotDecaying)),
        }
    }

    fn decay_time(&self, range: DecayRange) -> Result<DecayFit, ParamError> {
        let q = range.quantity();
        let tail = self.tail_energy()?;
        if let Some(m) = tail {
            // How far the decay had fallen when the series ended, the tail included.
            let reached = 10.0 * (m / (self.sums[self.onset.index] + m)).log10();
            if reached > range.bottom_db() {
                return Err(not_evaluable(
                    q,
                    NotEvaluable::RangeNotReached {
                        needed_db: range.bottom_db(),
                        reached_db: reached,
                    },
                ));
            }
        }
        let own = self.line(range, 0.0)?;
        let t = -60.0 / own.slope;
        let with_tail = match tail {
            Some(m) => self.line(range, m).ok().map(|l| -60.0 / l.slope),
            None => None,
        };
        let t = Self::bounded(q, t, with_tail, limits::DECAY_RELATIVE, |a, b| {
            (a / b - 1.0).abs() > limits::DECAY_RELATIVE
        })?;
        Ok(DecayFit {
            range,
            t_s: t,
            slope_db_per_s: own.slope,
            intercept_db: own.intercept,
            points: own.points,
            // `bounded` passed, so the tail's fit exists.
            with_tail_s: with_tail.unwrap(),
        })
    }

    /// Energy in `[from, to)` seconds, a bin split in proportion when an edge falls inside it.
    fn energy_in(&self, from: f64, to: f64) -> f64 {
        let dt = self.dt();
        self.series
            .values()
            .iter()
            .enumerate()
            .map(|(k, &e)| {
                let (a, b) = (k as f64 * dt, (k + 1) as f64 * dt);
                let overlap = (b.min(to) - a.max(from)).max(0.0);
                if overlap >= dt { e } else { e * overlap / dt }
            })
            .sum()
    }

    /// The window edge `t₀ + te`, refused when `te` is not a positive number or the series ends
    /// at or before it.
    fn window_end(&self, quantity: Quantity, te_s: f64) -> Result<f64, ParamError> {
        if !te_s.is_finite() || te_s <= 0.0 {
            return Err(ParamError::BadTimeStep { dt: te_s });
        }
        let edge = self.onset.time_s + te_s;
        let available = self.series.duration();
        if available - edge <= 1e-9 * self.dt() {
            return Err(ParamError::SeriesTooShort {
                what: quantity.to_string(),
                needed_s: edge,
                available_s: available,
            });
        }
        Ok(edge)
    }

    fn clarity_db(&self, te_s: f64) -> Result<f64, ParamError> {
        let q = Quantity::Clarity { te_s };
        let edge = self.window_end(q, te_s)?;
        let early = self.energy_in(self.onset.time_s, edge);
        let late = self.energy_in(edge, f64::INFINITY);
        if late <= 0.0 {
            return Err(not_evaluable(q, NotEvaluable::EmptyWindow { from_s: edge }));
        }
        let c = 10.0 * (early / late).log10();
        let with_tail = self
            .tail_energy()?
            .map(|m| 10.0 * (early / (late + m)).log10());
        Self::bounded(q, c, with_tail, limits::CLARITY_DB, |a, b| {
            (a - b).abs() > limits::CLARITY_DB
        })
    }

    fn definition(&self, te_s: f64) -> Result<f64, ParamError> {
        let q = Quantity::Definition { te_s };
        let edge = self.window_end(q, te_s)?;
        let early = self.energy_in(self.onset.time_s, edge);
        let total = self.sums[self.onset.index];
        let d = early / total;
        let with_tail = self.tail_energy()?.map(|m| early / (total + m));
        Self::bounded(q, d, with_tail, limits::DEFINITION, |a, b| {
            (a - b).abs() > limits::DEFINITION
        })
    }

    fn centre_time_s(&self) -> Result<f64, ParamError> {
        let q = Quantity::CentreTime;
        let dt = self.dt();
        let (k0, t0) = (self.onset.index, self.onset.time_s);
        let total = self.sums[k0];
        // Each bin's energy sits at the bin's midpoint.
        let moment: f64 = self.series.values()[k0..]
            .iter()
            .enumerate()
            .map(|(j, &e)| (((k0 + j) as f64 + 0.5) * dt - t0) * e)
            .sum();
        let ts = moment / total;
        let with_tail = match &self.tail {
            Err(e) => return Err(e.clone()),
            Ok(Tail::Unbounded) => None,
            Ok(Tail::Bounded {
                energy,
                ratio_per_bin,
                from_s,
            }) => {
                // The tail's bins continue geometrically from `from_s`, each at its midpoint.
                let r = *ratio_per_bin;
                let tail_centre = from_s - t0 + dt * (r / (1.0 - r) + 0.5);
                Some((moment + energy * tail_centre) / (total + energy))
            }
        };
        Self::bounded(q, ts, with_tail, limits::CENTRE_TIME_S, |a, b| {
            (a - b).abs() > limits::CENTRE_TIME_S
        })
    }

    fn spl_db(&self) -> Result<f64, ParamError> {
        let total = self.series.total();
        let level = |e: f64| 10.0 * (e / P_REF_SQUARED).log10();
        let with_tail = self.tail_energy()?.map(|m| level(total + m));
        Self::bounded(
            Quantity::Spl,
            level(total),
            with_tail,
            limits::SPL_DB,
            |a, b| (a - b).abs() > limits::SPL_DB,
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

    #[test]
    fn onset_is_the_first_bin_within_20_db_of_the_largest() {
        // 1/100 of the maximum is inside; just below it is not.
        let s = series(0.001, vec![0.0, 0.0099, 0.01, 1.0, 0.5]);
        assert_eq!(
            onset(&s),
            Onset {
                index: 2,
                time_s: 0.002
            }
        );
        let s = series(0.001, vec![0.0, 0.0099, 0.009_999, 1.0, 0.5]);
        assert_eq!(onset(&s).index, 3);
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
        let e = decay_time(&series(0.001, v), DecayRange::T30).unwrap_err();
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
        let p = evaluate(&series(0.001, v));
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

    #[test]
    fn a_curve_that_jumps_over_a_range_has_too_few_points() {
        // A direct pulse far above a short tail: the curve drops from 0 dB to about −35 dB in
        // one bin, so T20's range holds no point.
        let mut v = vec![1.0];
        v.extend((0..200).map(|k| 1e-5 * 0.97f64.powi(k)));
        v.extend([0.0; 50]);
        let e = decay_time(&series(0.001, v), DecayRange::T20).unwrap_err();
        assert!(
            matches!(
                e.not_evaluable(),
                Some(NotEvaluable::TooFewPoints { needed: 3, .. })
            ),
            "{e}"
        );
    }

    #[test]
    fn clarity_needs_the_series_to_pass_its_window_and_late_energy() {
        // 50 bins of 1 ms: the series ends at 50 ms, before C80's edge.
        let v: Vec<f64> = (0..50).map(|k| 0.9f64.powi(k)).collect();
        let e = clarity_db(&series(0.001, v.clone()), 0.08).unwrap_err();
        assert_eq!(e.code(), codes::SERIES_TOO_SHORT);
        // Energy only before 80 ms, then silence: no late energy.
        let mut w = v;
        w.extend([0.0; 100]);
        let e = clarity_db(&series(0.001, w), 0.08).unwrap_err();
        assert!(
            matches!(e.not_evaluable(), Some(NotEvaluable::EmptyWindow { .. })),
            "{e}"
        );
        // A window length that is not a positive number.
        let v: Vec<f64> = (0..500).map(|k| 0.99f64.powi(k)).collect();
        for te in [0.0, -0.05, f64::NAN] {
            assert_eq!(
                clarity_db(&series(0.001, v.clone()), te)
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
        let p = evaluate(&series(0.001, v));
        let json = serde_json::to_value(&p).unwrap();
        assert!(json["edt"]["Ok"]["t_s"].is_f64(), "{json}");
        assert_eq!(json["t30"]["Err"]["kind"], "not_evaluable");
        assert_eq!(json["t30"]["Err"]["quantity"]["quantity"], "t30");
        assert_eq!(json["t30"]["Err"]["why"]["why"], "range_not_reached");
    }

    #[test]
    fn a_window_edge_inside_a_bin_splits_it_in_proportion() {
        // Bins of 10 ms holding 1 and 1, then a steep fall so the tail is negligible; edge at
        // 5 ms after an onset at 0: early 0.5, late 1.5.
        let mut v = vec![1.0, 1.0, 1e-9, 1e-18];
        v.extend([0.0; 16]);
        let s = series(0.01, v);
        let c = clarity_db(&s, 0.005).unwrap();
        assert!((c - 10.0 * (0.5f64 / 1.5).log10()).abs() < 1e-8, "{c}");
        let d = definition(&s, 0.005).unwrap();
        assert!((d - 0.25).abs() < 1e-8, "{d}");
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
