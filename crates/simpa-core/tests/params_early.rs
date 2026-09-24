//! The early reverberation SPPS records: it begins with the first reflection, not with the direct
//! sound (M7 follow-ups, second review; `docs/params.md`, "The curve between bin edges").
//!
//! **What went wrong.** Between the arrival and the first bin wholly after the direct sound, the
//! curve took the reverberation to be that bin's decay continued back to the arrival. That is exact
//! for the synthetic decays of gate (a), which start at the arrival. SPPS's do not: its
//! reverberation starts with the first reflection, a few milliseconds after the direct sound, and
//! builds up. At a step of 10 ms that stretch is up to 11 ms, a quarter of EDT's 10 dB range in a
//! room of `T` = 0.22 s, and on SPPS runs EDT read 3.8 to 4.8 % short of EDT at a step of 0.2 ms
//! (`docs/results.md`, "What M8 needs"), returned as numbers.
//!
//! **Now.** A series made with `EnergySeries::with_early_reverberation_unresolved`, as
//! `core::results` makes every SPPS series, is read three ways: the reverberation beginning at the
//! arrival (that bin's decay continued back, the most a decay from the arrival would put there), at
//! the start of the first bin wholly after the direct sound (none before it), and at that bin's end
//! (its energy arriving late, as a reverberation building up through it brings it). Each quantity
//! is reported midway between the lowest and highest reading and refused, `early_unresolved`, when
//! they lie further than its limit from it.
//!
//! Two readings were not enough: with the reverberation beginning at the arrival or at the first
//! bin only, a build-up reaching into that bin put an accepted EDT 1.6 % off the truth at 10 ms
//! (`T` = 0.25 s, `D/R` 0.3, the arrival 0.71 of the way into its bin).
//!
//! The series here are a direct sound spread over the receiver ball, as SPPS records it, and
//! reverberation that starts `DELAY` after the arrival and builds up linearly over `RAMP` to an
//! exponential decay, as bin integrals in closed form. The truth is computed apart from `params`,
//! on a 10 µs grid: the least-squares line through the curve's level over the time it spends inside
//! each range, the direct sound a step at the arrival.
//!
//! Every check says no:
//! - the same series without the mark, at a step of 10 ms: EDT is accepted outside its limit of the
//!   truth (the defect);
//! - with the mark every value that comes out is within its limit of the truth, at 10 ms and 1 ms,
//!   and the truth lies between the lowest and highest reading wherever EDT is refused.

use std::f64::consts::LN_10;

use simpa_core::params::decay::{Arrival, BandParameters, evaluate, onset};
use simpa_core::params::{EnergySeries, NotEvaluable};

const K60: f64 = 6.0 * LN_10;
/// SPPS's speed of sound at 20 °C.
const C: f64 = 343.2;
/// Upstream's receiver radius.
const RADIUS: f64 = 0.31;
/// From the arrival to the first reflection, s.
const DELAY: f64 = 0.003;
/// Over which the reverberation builds up to its decay, s.
const RAMP: f64 = 0.008;
const TS: [f64; 3] = [0.25, 0.6, 1.5];
/// Direct over reverberant energy.
const DIRECTS: [f64; 2] = [0.3, 1.0];
const OFFSETS: usize = 12;

/// The reverberation's energy flux from `x` = 0, the arrival: 0 before `DELAY`, then rising
/// linearly over `RAMP` to `e^(−x/τ)`.
struct Reverb {
    tau: f64,
}

impl Reverb {
    /// `∫₀^x` of the flux.
    fn cumulative(&self, x: f64) -> f64 {
        let tau = self.tau;
        let e = |x: f64| (-x / tau).exp();
        if x <= DELAY {
            0.0
        } else if x <= DELAY + RAMP {
            tau / RAMP * (e(x) * (DELAY - x - tau) + tau * e(DELAY))
        } else {
            self.cumulative(DELAY + RAMP) + tau * (e(DELAY + RAMP) - e(x))
        }
    }

    fn total(&self) -> f64 {
        self.cumulative(DELAY + RAMP) + self.tau * (-(DELAY + RAMP) / self.tau).exp()
    }

    /// Energy after `x`.
    fn after(&self, x: f64) -> f64 {
        self.total() - self.cumulative(x.max(0.0))
    }
}

/// A direct sound of `direct` times the reverberation's energy, spread over the ball as SPPS
/// records it, and the reverberation from `r/c`, as bin integrals down to 150 dB.
fn series(t: f64, dt: f64, r: f64, direct: f64) -> (EnergySeries, Reverb) {
    let rev = Reverb { tau: t / K60 };
    let d = direct * rev.total();
    let t_a = r / C;
    let (t1, t2) = ((r - RADIUS) / C, (r + RADIUS) / C);
    let prim = |t: f64| {
        let u = C * t.clamp(t1, t2) - r;
        RADIUS * RADIUS * u - u * u * u / 3.0
    };
    let whole = 4.0 * RADIUS.powi(3) / 3.0;
    let n = (t_a / dt) as usize + 2 + (150.0 * t / 60.0 / dt).round() as usize;
    let v = (0..n)
        .map(|k| {
            let (a, b) = (k as f64 * dt, (k + 1) as f64 * dt);
            d * (prim(b) - prim(a)) / whole + rev.after(a - t_a) - rev.after(b - t_a)
        })
        .collect();
    (EnergySeries::complete(dt, v).unwrap(), rev)
}

/// The truth, apart from `params`: `(EDT, T30, Ts)` of the direct sound as a step at the arrival
/// followed by the reverberation, on a 10 µs grid.
fn truth(rev: &Reverb, direct: f64) -> (f64, f64, f64) {
    let top = direct * rev.total() + rev.total();
    let h = 1e-5;
    let level = |u: f64| 10.0 * (rev.after(u) / top).log10();
    let fit = |hi: f64, lo: f64| {
        let (mut n, mut su, mut sl, mut suu, mut sul) = (0.0, 0.0, 0.0, 0.0, 0.0);
        let mut k = 0usize;
        loop {
            let u = (k as f64 + 0.5) * h;
            let l = level(u);
            if l < lo {
                break;
            }
            if l <= hi {
                n += 1.0;
                su += u;
                sl += l;
                suu += u * u;
                sul += u * l;
            }
            k += 1;
        }
        let slope = (n * sul - su * sl) / (n * suu - su * su);
        -60.0 / slope
    };
    // Ts: ∫₀^∞ S(u) du over S(0), the direct sound adding nothing.
    let mut moment = 0.0;
    let mut u = 0.0;
    while rev.after(u) > 1e-16 * top {
        moment += h * rev.after(u + 0.5 * h);
        u += h;
    }
    (fit(0.0, -10.0), fit(-5.0, -35.0), moment / top)
}

/// The source–receiver distance putting `r/c` a fraction `f` into the bin that holds 29 ms.
fn distance(dt: f64, f: f64) -> f64 {
    ((0.029 / dt).floor() + f) * dt * C
}

/// `(low, high)` of an `early_unresolved` refusal, when every reading gives a value.
fn readings(r: &Result<f64, simpa_core::params::ParamError>) -> Option<(f64, Option<f64>)> {
    match r.as_ref().err()?.not_evaluable()? {
        NotEvaluable::EarlyUnresolved { low, high, .. } => Some(((*low)?, *high)),
        _ => None,
    }
}

struct Tally {
    edt_accepted: usize,
    edt_refused: usize,
    ts_accepted: usize,
    worst_edt: f64,
    worst_t30: f64,
    worst_ts: f64,
    /// Where EDT is refused: how far the truth lies outside the two readings, relative.
    worst_outside: f64,
}

/// Every case at step `dt`, marked or not; with `check`, asserts each bound.
fn sweep(dt: f64, marked: bool, check: bool) -> Tally {
    let mut tally = Tally {
        edt_accepted: 0,
        edt_refused: 0,
        ts_accepted: 0,
        worst_edt: 0.0,
        worst_t30: 0.0,
        worst_ts: 0.0,
        worst_outside: 0.0,
    };
    for t in TS {
        for direct in DIRECTS {
            for i in 0..OFFSETS {
                let f = (i as f64 + 0.5) / OFFSETS as f64;
                let r = distance(dt, f);
                let (s, rev) = series(t, dt, r, direct);
                let s = if marked {
                    s.with_early_reverberation_unresolved()
                } else {
                    s
                };
                let (edt, t30, ts) = truth(&rev, direct);
                let p: BandParameters = evaluate(&s, Arrival::spread(r / C, RADIUS / C));
                let case = format!("dt {dt}, T {t}, D/R {direct}, offset {f:.3}");
                let edt_r = p.edt.as_ref().map(|x| x.t_s).map_err(Clone::clone);
                match &edt_r {
                    Ok(x) => {
                        tally.edt_accepted += 1;
                        let d = (x / edt - 1.0).abs();
                        tally.worst_edt = tally.worst_edt.max(d);
                        if check {
                            assert!(d <= 0.005, "{case}: EDT {x} against {edt}");
                        }
                    }
                    Err(e) => {
                        if let Some((cont, Some(flat))) = readings(&edt_r) {
                            tally.edt_refused += 1;
                            // How far the truth lies outside the lowest and highest reading,
                            // relative: 0 between them.
                            let (lo, hi) = (cont.min(flat), cont.max(flat));
                            let out = (lo / edt - 1.0).max(1.0 - hi / edt).max(0.0);
                            tally.worst_outside = tally.worst_outside.max(out);
                            if check {
                                assert!(
                                    out <= 0.005,
                                    "{case}: EDT truth {edt} outside [{lo}, {hi}]"
                                );
                            }
                        } else if check {
                            assert!(
                                matches!(
                                    e.not_evaluable(),
                                    Some(NotEvaluable::RangeTooShort { .. })
                                ),
                                "{case}: EDT refused otherwise: {e}"
                            );
                        }
                    }
                }
                if let Ok(x) = &p.t30 {
                    let d = (x.t_s / t30 - 1.0).abs();
                    tally.worst_t30 = tally.worst_t30.max(d);
                    if check {
                        assert!(d <= 0.005, "{case}: T30 {} against {t30}", x.t_s);
                    }
                } else if check {
                    panic!("{case}: T30 refused: {:?}", p.t30);
                }
                // Ts is measured from the arrival: only where it lies in the onset bin.
                if r / C < onset(&s).bin_end_s
                    && let Ok(x) = &p.ts_s
                {
                    tally.ts_accepted += 1;
                    let d = (x - ts).abs();
                    tally.worst_ts = tally.worst_ts.max(d / ts);
                    if check {
                        assert!(d <= (0.005 * ts).min(0.001), "{case}: Ts {x} against {ts}");
                    }
                }
            }
        }
    }
    tally
}

#[test]
fn marked_every_value_that_comes_out_is_within_its_limit_of_the_truth() {
    for dt in [0.01, 0.001] {
        let t = sweep(dt, true, true);
        println!(
            "marked, dt {dt}: EDT accepted {}, refused early_unresolved {}; Ts accepted {}; worst \
             accepted EDT {:.2} %, T30 {:.3} %, Ts {:.3} %; where EDT is refused the truth lies at \
             most {:.2} % outside its readings",
            t.edt_accepted,
            t.edt_refused,
            t.ts_accepted,
            100.0 * t.worst_edt,
            100.0 * t.worst_t30,
            100.0 * t.worst_ts,
            100.0 * t.worst_outside
        );
        if dt == 0.01 {
            // At 10 ms the stretch is too long for the short decays: EDT is refused there.
            assert!(t.edt_refused > 0);
        } else {
            // At 1 ms it is short enough for EDT to come out everywhere.
            assert_eq!(t.edt_refused, 0);
        }
        assert!(t.edt_accepted > 0);
    }
}

#[test]
fn unmarked_the_decay_continued_back_gives_edt_outside_its_limit() {
    // Says no: the model alone, the reverberation continued back to the arrival, at 10 ms.
    let t = sweep(0.01, false, false);
    println!(
        "unmarked, dt 0.01: EDT accepted {}, worst {:.2} % off the truth",
        t.edt_accepted,
        100.0 * t.worst_edt
    );
    assert!(t.worst_edt > 0.005, "{}", t.worst_edt);
}
