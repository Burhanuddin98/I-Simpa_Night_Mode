//! M7 gate (a): exact exponential decays `E(t) = E0·exp(−6·ln10·t/T)` (the gate writes the
//! constant as 13.8155) with T in {0.3, 1.0, 3.0} s and dt in {0.001, 0.01} s. T20, T30 and EDT
//! must be within 0.5 % of T, C80 and C50 within 0.01 dB and D50 within 0.1 percentage points of
//! their closed forms. Ts must be within 0.5 % of its closed form, τ (the gate names no Ts bound;
//! this is the decay times' bound). See `docs/params.md`.
//!
//! The gate's decays start at t = 0 on a bin edge. Real receivers do not: the direct sound
//! arrives inside a bin, `r/c` after the source starts. The same bounds are asserted on a direct
//! sound plus an exponential decay arriving at five places inside a bin, measured from the given
//! arrival.
//!
//! Every check has a partner that makes it say no:
//! - a decay with T off by 1 % fails every bound, Ts's included; a decay off only where one
//!   parameter looks fails that parameter and no other;
//! - a series truncated before −35 dB gives `range_not_reached` for T30, not a number, although
//!   the truncated curve itself passes −35 dB; a series truncated at −40 dB gives `truncated`;
//! - a double-slope decay is flagged by the T20/T30 disagreement, a single slope is not;
//! - a direct sound measured from the start of its bin, as the first version of this module did,
//!   fails C, D or Ts; its EDT by the first version's regression fails EDT's bound; with the
//!   arrival not given, C, D and Ts at dt = 10 ms are refused as `unresolved`, bracketing the
//!   closed form, never returned as numbers.
//!
//! The series are bin integrals of the exponential, so bin `k` holds the energy of
//! `[k·dt, (k+1)·dt)` exactly, as SPPS's histogram does.

use std::f64::consts::LN_10;

use simpa_core::params::decay::{
    self, Arrival, BandParameters, DecayRange, P_REF_SQUARED, evaluate, schroeder_db,
};
use simpa_core::params::{EnergySeries, NotEvaluable, codes};

/// `6·ln(10)` = 13.815510…, the gate's 13.8155.
const K60: f64 = 6.0 * LN_10;
const TS: [f64; 3] = [0.3, 1.0, 3.0];
const DTS: [f64; 2] = [0.001, 0.01];
/// Energy scale, Pa²: 1 Pa²·s of total energy is about 94 dB.
const E0: f64 = 1.0;
/// The gate's decays start at t = 0.
const AT_ZERO: Arrival = Arrival::at(0.0);

fn tau(t: f64) -> f64 {
    t / K60
}

/// Bin integrals of `E0·exp(−t/τ)` down to `depth_db` of decay.
fn exact_decay(t: f64, dt: f64, depth_db: f64) -> EnergySeries {
    let tau = tau(t);
    let n = (depth_db * t / 60.0 / dt).round() as usize;
    let v = (0..n)
        .map(|k| {
            let (a, b) = (k as f64 * dt, (k + 1) as f64 * dt);
            E0 * tau * ((-a / tau).exp() - (-b / tau).exp())
        })
        .collect();
    EnergySeries::new(dt, v).unwrap()
}

/// A decay whose Schroeder curve is `level(t)` dB exactly at every bin edge: bin `k` holds
/// `S(k·dt) − S((k+1)·dt)`.
fn prescribed_curve(dt: f64, n: usize, level: impl Fn(f64) -> f64) -> EnergySeries {
    let s = |k: usize| 10f64.powf(level(k as f64 * dt) / 10.0);
    EnergySeries::new(dt, (0..n).map(|k| s(k) - s(k + 1)).collect()).unwrap()
}

// The closed forms of an exponential decay from t = 0.
fn clarity_closed(te: f64, t: f64) -> f64 {
    10.0 * ((te / tau(t)).exp() - 1.0).log10()
}
fn definition_closed(te: f64, t: f64) -> f64 {
    1.0 - (-te / tau(t)).exp()
}

// The gate's bounds, one predicate each, shared by the tests that must pass and those that must
// fail.
fn decay_ok(got: f64, t: f64) -> bool {
    (got / t - 1.0).abs() <= 0.005
}
fn clarity_ok(got: f64, closed: f64) -> bool {
    (got - closed).abs() <= 0.01
}
fn definition_ok(got: f64, closed: f64) -> bool {
    100.0 * (got - closed).abs() <= 0.1
}
fn centre_ok(got: f64, closed: f64) -> bool {
    (got / closed - 1.0).abs() <= 0.005
}

/// The closed forms a band is checked against.
struct Closed {
    t: f64,
    c50: f64,
    c80: f64,
    d50: f64,
    ts: f64,
}

impl Closed {
    /// An exponential decay of reverberation time `t` from t = 0.
    fn decay(t: f64) -> Closed {
        Closed {
            t,
            c50: clarity_closed(0.05, t),
            c80: clarity_closed(0.08, t),
            d50: definition_closed(0.05, t),
            ts: tau(t),
        }
    }

    /// A direct sound of energy `direct` followed by a decay of total energy 1, measured from
    /// the direct sound: early energy `direct + 1 − e^(−te/τ)`, late `e^(−te/τ)`, and
    /// `Ts = τ/(direct + 1)`.
    fn direct_and_decay(t: f64, direct: f64) -> Closed {
        let late = |te: f64| (-te / tau(t)).exp();
        let early = |te: f64| direct + 1.0 - late(te);
        Closed {
            t,
            c50: 10.0 * (early(0.05) / late(0.05)).log10(),
            c80: 10.0 * (early(0.08) / late(0.08)).log10(),
            d50: early(0.05) / (direct + 1.0),
            ts: tau(t) / (direct + 1.0),
        }
    }
}

/// Whether each gated quantity of `p` meets its bound against `c`:
/// `[EDT, T20, T30, C50, C80, D50, Ts]`. A refused quantity does not meet it.
fn gate_checks(p: &BandParameters, c: &Closed) -> [bool; 7] {
    let d = |r: &Result<decay::DecayFit, _>| r.as_ref().is_ok_and(|f| decay_ok(f.t_s, c.t));
    [
        d(&p.edt),
        d(&p.t20),
        d(&p.t30),
        p.c50_db.as_ref().is_ok_and(|&x| clarity_ok(x, c.c50)),
        p.c80_db.as_ref().is_ok_and(|&x| clarity_ok(x, c.c80)),
        p.d50.as_ref().is_ok_and(|&x| definition_ok(x, c.d50)),
        p.ts_s.as_ref().is_ok_and(|&x| centre_ok(x, c.ts)),
    ]
}

/// The largest deviation of `p` from `c` in each gated unit: relative for the times, dB for C,
/// percentage points for D50.
fn deviations(p: &BandParameters, c: &Closed) -> [f64; 7] {
    let t = |r: &Result<decay::DecayFit, _>| (r.clone().unwrap().t_s / c.t - 1.0).abs();
    [
        t(&p.edt),
        t(&p.t20),
        t(&p.t30),
        (p.c50_db.clone().unwrap() - c.c50).abs(),
        (p.c80_db.clone().unwrap() - c.c80).abs(),
        100.0 * (p.d50.clone().unwrap() - c.d50).abs(),
        (p.ts_s.clone().unwrap() / c.ts - 1.0).abs(),
    ]
}

#[test]
fn exact_decays_meet_every_bound() {
    for t in TS {
        for dt in DTS {
            let s = exact_decay(t, dt, 150.0);
            let p = evaluate(&s, AT_ZERO);
            let closed = Closed::decay(t);
            let dev = deviations(&p, &closed);
            println!(
                "T {t} s, dt {dt} s: EDT {:.1e}, T20 {:.1e}, T30 {:.1e} (relative); C50 {:.1e} dB, \
                 C80 {:.1e} dB, D50 {:.1e} points; Ts {:.1e} (relative)",
                dev[0], dev[1], dev[2], dev[3], dev[4], dev[5], dev[6]
            );
            assert_eq!(gate_checks(&p, &closed), [true; 7], "T {t}, dt {dt}");
            // Tighter than the gate: the model is exact for an exponential, Ts included.
            for d in dev {
                assert!(d < 1e-9, "T {t}, dt {dt}: {dev:?}");
            }
            assert_eq!(p.onset.index, 0);
            // SPL: the sum over p0².
            let spl = p.spl_db.clone().unwrap();
            assert!((spl - 10.0 * (s.total() / P_REF_SQUARED).log10()).abs() < 1e-9);
            // A single slope is not curved.
            let c = p.curvature.clone().unwrap();
            assert!(!c.curved && c.percent.abs() < 1e-6, "{c:?}");
        }
    }
}

/// What gate (a)'s six decays give with the arrival detected, not given: per quantity of
/// `[EDT, T20, T30, C50, C80, D50, Ts]`, `Some(true)` when it meets its bound, `None` when it is
/// refused `unresolved` with the closed form between the two ends of the onset bin; anything else
/// panics (a value outside its bound, another refusal, or an `unresolved` that misses the closed
/// form).
fn detected_outcome(p: &BandParameters, c: &Closed) -> [Option<bool>; 7] {
    let ok = gate_checks(p, c);
    // The truth is one end of the bin, so it may lie a rounding step outside the pair.
    let bracketed = |e: &simpa_core::params::ParamError, want: f64| match e.not_evaluable() {
        Some(NotEvaluable::Unresolved { low, high, .. }) => {
            let tol = 1e-9 * want.abs().max(1.0);
            *low - tol <= want && want <= *high + tol
        }
        _ => false,
    };
    let times = [&p.edt, &p.t20, &p.t30];
    let others = [
        (&p.c50_db, c.c50),
        (&p.c80_db, c.c80),
        (&p.d50, c.d50),
        (&p.ts_s, c.ts),
    ];
    let mut out = [None; 7];
    for (i, r) in times.into_iter().enumerate() {
        match r {
            Ok(_) if ok[i] => out[i] = Some(true),
            Err(e) if bracketed(e, c.t) => {}
            other => panic!("quantity {i}: {other:?}"),
        }
    }
    for (i, (r, want)) in others.into_iter().enumerate() {
        match r {
            Ok(_) if ok[3 + i] => out[3 + i] = Some(true),
            Err(e) if bracketed(e, want) => {}
            other => panic!("quantity {}: {other:?} against {want}", 3 + i),
        }
    }
    out
}

#[test]
fn gate_a_decays_with_the_arrival_detected() {
    // M7 follow-ups (the M7 critic): gate (a) passes with the true arrival given, `Arrival::at(0)`.
    // The plan's direct-arrival detection is `Arrival::Detected`: the arrival somewhere in the
    // onset bin, every onset-relative quantity computed from both ends of it, and refused
    // `unresolved` when the two differ by more than its limit. Run on the gate's six decays.
    let mut table = Vec::new();
    for t in TS {
        for dt in DTS {
            let s = exact_decay(t, dt, 150.0);
            let p = evaluate(&s, Arrival::Detected);
            let got = detected_outcome(&p, &Closed::decay(t));
            let cell = |x: Option<bool>| if x.is_some() { "pass" } else { "unresolved" };
            println!(
                "detected, T {t} s, dt {dt} s: EDT {}, T20 {}, T30 {}, C50 {}, C80 {}, D50 {}, \
                 Ts {}",
                cell(got[0]),
                cell(got[1]),
                cell(got[2]),
                cell(got[3]),
                cell(got[4]),
                cell(got[5]),
                cell(got[6])
            );
            table.push(((t, dt), got.map(|x| x.is_some())));
        }
    }
    // The decay times do not depend on where in the bin time starts: they pass everywhere. C50,
    // C80 and D50 are refused in all six, the truth (the bin's start) one end of the bracket; Ts
    // comes through only at T = 3 s, dt = 1 ms, where the two ends of the bin give Ts within its
    // limit of each other. Gate (a)'s C80 and D50 therefore need the arrival given
    // (`docs/params.md`).
    let pinned = [
        ((0.3, 0.001), [true, true, true, false, false, false, false]),
        ((0.3, 0.01), [true, true, true, false, false, false, false]),
        ((1.0, 0.001), [true, true, true, false, false, false, false]),
        ((1.0, 0.01), [true, true, true, false, false, false, false]),
        ((3.0, 0.001), [true, true, true, false, false, false, true]),
        ((3.0, 0.01), [true, true, true, false, false, false, false]),
    ];
    assert_eq!(table, pinned);
    // Says no: the same six decays 1 % off, detected, miss the decay times' bound.
    for t in TS {
        for dt in DTS {
            let p = evaluate(&exact_decay(1.01 * t, dt, 150.0), Arrival::Detected);
            assert_eq!(
                gate_checks(&p, &Closed::decay(t))[..3],
                [false; 3],
                "T {t}, dt {dt}"
            );
        }
    }
}

#[test]
fn a_decay_one_percent_off_fails_every_bound() {
    for t in TS {
        for dt in DTS {
            let p = evaluate(&exact_decay(1.01 * t, dt, 150.0), AT_ZERO);
            // Every quantity is computed (none refused), and every one misses its bound.
            for r in [&p.edt, &p.t20, &p.t30] {
                assert!(r.is_ok());
            }
            assert!(p.c50_db.is_ok() && p.c80_db.is_ok() && p.d50.is_ok() && p.ts_s.is_ok());
            assert_eq!(
                gate_checks(&p, &Closed::decay(t)),
                [false; 7],
                "T {t}, dt {dt}"
            );
        }
    }
}

#[test]
fn a_decay_off_only_where_one_parameter_looks_fails_that_parameter_only() {
    for t in TS {
        let dt = 0.001;
        let n = (150.0 * t / 60.0 / dt).round() as usize;
        let slope = -60.0 / t;
        // 5 % faster above −5 dB: only EDT sees it. T20 and T30 fit the curve at or below −5 dB,
        // where the slope is exact.
        let t5 = -5.0 / (1.05 * slope);
        let early = prescribed_curve(dt, n, |x| {
            if x <= t5 {
                1.05 * slope * x
            } else {
                -5.0 + slope * (x - t5)
            }
        });
        let [edt, t20, t30, ..] = gate_checks(&evaluate(&early, AT_ZERO), &Closed::decay(t));
        println!("early kink, T {t} s: {}", kink_line([edt, t20, t30]));
        assert_eq!([edt, t20, t30], [false, true, true], "early kink, T {t}");
        // 5 % slower below −25 dB: only T30 sees it.
        let t25 = -25.0 / slope;
        let late = prescribed_curve(dt, n, |x| {
            if x <= t25 {
                slope * x
            } else {
                -25.0 + slope / 1.05 * (x - t25)
            }
        });
        let [edt, t20, t30, ..] = gate_checks(&evaluate(&late, AT_ZERO), &Closed::decay(t));
        println!("late kink, T {t} s: {}", kink_line([edt, t20, t30]));
        assert_eq!([edt, t20, t30], [true, true, false], "late kink, T {t}");
    }
}

/// `EDT within, T20 within, T30 OUTSIDE`: which of the three meet gate (a)'s 0.5 %, for the gate
/// script to read.
fn kink_line(within: [bool; 3]) -> String {
    let w = |b: bool| if b { "within" } else { "OUTSIDE" };
    format!(
        "EDT {}, T20 {}, T30 {}",
        w(within[0]),
        w(within[1]),
        w(within[2])
    )
}

/// T30 by a least-squares line through the truncated curve's bin-edge points in −5 … −35 dB,
/// nothing added for the tail: what upstream's GUI does, bar its one extra point
/// (`projet_calculation.cpp:143-170`). `None` when the curve never reaches −35 dB.
fn naive_t30(s: &EnergySeries) -> Option<f64> {
    naive_fit(s, -5.0, -35.0)
}

/// A least-squares line through the raw bin-edge points of the Schroeder curve from the onset
/// bin's start, within `top` … `bottom` dB; `None` when the curve never reaches `bottom`.
fn naive_fit(s: &EnergySeries, top: f64, bottom: f64) -> Option<f64> {
    let curve = schroeder_db(s);
    let pts: Vec<(f64, f64)> = curve
        .iter()
        .copied()
        .filter(|&(_, l)| (bottom..=top).contains(&l))
        .collect();
    if curve.iter().all(|&(_, l)| l > bottom) || pts.len() < 2 {
        return None;
    }
    let n = pts.len() as f64;
    let mt = pts.iter().map(|p| p.0).sum::<f64>() / n;
    let ml = pts.iter().map(|p| p.1).sum::<f64>() / n;
    let stt: f64 = pts.iter().map(|p| (p.0 - mt).powi(2)).sum();
    let stl: f64 = pts.iter().map(|p| (p.0 - mt) * (p.1 - ml)).sum();
    Some(-60.0 / (stl / stt))
}

#[test]
fn a_series_cut_before_minus_35_db_gives_not_evaluable_not_a_number() {
    let mut naive_numbers = 0;
    for t in TS {
        for dt in DTS {
            let s = exact_decay(t, dt, 30.0);
            let e = decay::decay_time(&s, AT_ZERO, DecayRange::T30).unwrap_err();
            assert_eq!(e.code(), codes::NOT_EVALUABLE);
            match e.not_evaluable() {
                Some(NotEvaluable::RangeNotReached {
                    needed_db,
                    reached_db,
                }) => {
                    assert_eq!(*needed_db, -35.0);
                    assert!((reached_db + 30.0).abs() < 1e-6, "reached {reached_db}");
                }
                other => panic!("T {t}, dt {dt}: {other:?}"),
            }
            // The whole band: T30 and T20 are refused, EDT survives (its truncation bias at
            // 30 dB is inside its 0.5 % limit).
            let p = evaluate(&s, AT_ZERO);
            assert!(p.t30.is_err());
            assert!(
                matches!(
                    p.t20.as_ref().unwrap_err().not_evaluable(),
                    Some(NotEvaluable::Truncated { .. })
                ),
                "{:?}",
                p.t20
            );
            assert!(p.edt.is_ok(), "{:?}", p.edt);
            // Without the tail bound the truncated curve still passes −35 dB here, and a fit
            // gives a number about 12 % short.
            if let Some(n) = naive_t30(&s) {
                naive_numbers += 1;
                println!(
                    "T {t}, dt {dt}: the naive T30 is {:+.1} %",
                    100.0 * (n / t - 1.0)
                );
                assert!(n / t - 1.0 < -0.1, "naive {n} vs {t}");
            }
        }
    }
    // Five of the six cases (all but T = 0.3 s at dt = 10 ms, docs/params.md).
    assert_eq!(naive_numbers, 5);
}

#[test]
fn a_series_cut_at_minus_40_db_is_refused_as_truncated_and_at_60_db_is_not() {
    for t in TS {
        for dt in DTS {
            let e =
                decay::decay_time(&exact_decay(t, dt, 40.0), AT_ZERO, DecayRange::T30).unwrap_err();
            match e.not_evaluable() {
                Some(NotEvaluable::Truncated {
                    value,
                    with_tail: Some(w),
                    limit,
                }) => {
                    assert_eq!(*limit, 0.005);
                    // The value from the series alone is short; the tail restores T.
                    assert!(value / t - 1.0 < -0.01, "{value}");
                    assert!((w / t - 1.0).abs() < 1e-6, "{w}");
                }
                other => panic!("T {t}, dt {dt}: {other:?}"),
            }
            let fit =
                decay::decay_time(&exact_decay(t, dt, 60.0), AT_ZERO, DecayRange::T30).unwrap();
            assert!(decay_ok(fit.t_s, t), "{fit:?}");
        }
    }
}

/// `docs/params.md`'s table: T30 of the truncated curve with nothing added, `T = 1 s`,
/// `dt = 1 ms`, against the depth the true decay reached; and the depth from which our bound
/// lets T30 through.
#[test]
fn the_documented_truncation_table_holds() {
    // (depth, percent as printed, half a unit of its last printed digit)
    let table = [
        (30.0, -12.2, 0.05),
        (35.0, -6.1, 0.05),
        (40.0, -2.4, 0.05),
        (45.0, -0.85, 0.005),
        (50.0, -0.28, 0.005),
        (55.0, -0.09, 0.005),
        (60.0, -0.03, 0.005),
    ];
    for (depth, percent, half_unit) in table {
        let n = naive_t30(&exact_decay(1.0, 0.001, depth)).unwrap();
        let got = 100.0 * (n - 1.0);
        println!("D {depth} dB: {got:.4} %");
        assert!(
            (got - percent).abs() <= half_unit,
            "D {depth}: {got:.4} % does not print as {percent} %"
        );
    }
    let first_pass = (30..=80)
        .find(|&d| {
            decay::decay_time(&exact_decay(1.0, 0.001, d as f64), AT_ZERO, DecayRange::T30).is_ok()
        })
        .unwrap();
    println!("T30 passes from {first_pass} dB of true decay");
    assert_eq!(first_pass, 48);
}

#[test]
fn a_double_slope_decay_is_flagged_and_a_single_slope_is_not() {
    for dt in DTS {
        // Two exponentials, T1 = 1 s and T2 = 3 s, the slow one 20 dB down in the Schroeder
        // curve at the start: a coupled room.
        let (t1, t2) = (1.0, 3.0);
        let (tau1, tau2) = (tau(t1), tau(t2));
        let a = 0.01 * tau1 / tau2;
        let n = (8.0 / dt) as usize;
        let bin = |tau: f64, k: usize| {
            let (x, y) = (k as f64 * dt, (k + 1) as f64 * dt);
            tau * ((-x / tau).exp() - (-y / tau).exp())
        };
        let v = (0..n).map(|k| bin(tau1, k) + a * bin(tau2, k)).collect();
        let p = evaluate(&EnergySeries::new(dt, v).unwrap(), AT_ZERO);
        let c = p.curvature.clone().unwrap();
        println!(
            "dt {dt}: T20 {:.3} s, T30 {:.3} s, curvature {:.1} %",
            p.t20.as_ref().unwrap().t_s,
            p.t30.as_ref().unwrap().t_s,
            c.percent
        );
        assert!(c.curved && c.percent > 10.0, "{c:?}");
        // The single slope at the same resolution is not flagged (exact_decays_meet_every_bound).
        let single = evaluate(&exact_decay(t1, dt, 150.0), AT_ZERO)
            .curvature
            .unwrap();
        assert!(!single.curved);
    }
}

/// Where inside its bin the direct sound arrives, as a fraction of the bin.
const OFFSETS: [f64; 5] = [0.05, 0.3, 0.5, 0.75, 0.95];
/// Direct energy over the decay's: equal, as at a receiver near the critical distance.
const DIRECT: f64 = 1.0;

/// A direct sound of energy `direct` at `(k + f)·dt`, `k` the bin 29 ms falls in (10 m at
/// 343.2 m/s), then `exp(−(t − t_a)/τ)` of total energy 1, as bin integrals down to 150 dB.
/// Returns the series and the arrival time.
fn direct_and_decay(t: f64, dt: f64, f: f64, direct: f64) -> (EnergySeries, f64) {
    let tau = tau(t);
    let ka = (0.029 / dt).floor() as usize;
    let t_a = (ka as f64 + f) * dt;
    let n = ka + 1 + (150.0 * t / 60.0 / dt).round() as usize;
    let v = (0..n)
        .map(|k| {
            let (a, b) = (k as f64 * dt, (k + 1) as f64 * dt);
            let decay = if k < ka {
                0.0
            } else {
                (-(a.max(t_a) - t_a) / tau).exp() - (-(b - t_a) / tau).exp()
            };
            decay + if k == ka { direct } else { 0.0 }
        })
        .collect();
    (EnergySeries::new(dt, v).unwrap(), t_a)
}

#[test]
fn a_direct_sound_inside_a_bin_meets_every_bound_from_its_arrival() {
    let mut worst = [0.0_f64; 7];
    for t in TS {
        for dt in DTS {
            for f in OFFSETS {
                let (s, t_a) = direct_and_decay(t, dt, f, DIRECT);
                let p = evaluate(&s, Arrival::at(t_a));
                assert_eq!(p.onset.index, (0.029 / dt).floor() as usize);
                let closed = Closed::direct_and_decay(t, DIRECT);
                assert_eq!(
                    gate_checks(&p, &closed),
                    [true; 7],
                    "T {t}, dt {dt}, offset {f}: {p:?}"
                );
                for (w, d) in worst.iter_mut().zip(deviations(&p, &closed)) {
                    *w = w.max(d);
                }
            }
        }
    }
    println!("worst deviation over 30 cases: {worst:?}");
    // The model is exact for a direct sound followed by an exponential.
    for d in worst {
        assert!(d < 1e-9, "{worst:?}");
    }
}

#[test]
fn a_direct_sound_measured_from_the_start_of_its_bin_fails() {
    // The first version of this module took the arrival as the start of the onset bin. At
    // dt = 10 ms that misses C, D or Ts at every offset tried, from 0.05 of a bin.
    for t in TS {
        for f in OFFSETS {
            let dt = 0.01;
            let (s, t_a) = direct_and_decay(t, dt, f, DIRECT);
            let start = decay::onset(&s).bin_start_s;
            let p = evaluate(&s, Arrival::at(start));
            let closed = Closed::direct_and_decay(t, DIRECT);
            let ok = gate_checks(&p, &closed);
            println!(
                "T {t}, arrival {:.1} ms, from {start} s: C50 {:+.3} dB, C80 {:+.3} dB, D50 \
                 {:+.2} points, Ts {:+.2} ms; checks {ok:?}",
                1000.0 * t_a,
                p.c50_db.clone().unwrap() - closed.c50,
                p.c80_db.clone().unwrap() - closed.c80,
                100.0 * (p.d50.clone().unwrap() - closed.d50),
                1000.0 * (p.ts_s.clone().unwrap() - closed.ts),
            );
            // Decay times do not depend on where the time axis starts.
            assert_eq!(ok[..3], [true; 3]);
            assert!(ok[3..].contains(&false), "T {t}, offset {f}: {ok:?}");
        }
    }
}

#[test]
fn the_first_versions_edt_regression_fails_with_a_direct_sound() {
    // The first version fitted EDT to the bin-edge points of the curve from the onset bin's
    // start, the 0 dB point included, which then stands for a whole bin at 0 dB. With a direct
    // sound as strong as the decay, at dt = 10 ms, it reads short by several per cent.
    for t in TS {
        for f in OFFSETS {
            let (s, t_a) = direct_and_decay(t, 0.01, f, DIRECT);
            let naive = naive_fit(&s, 0.0, -10.0).unwrap();
            let ours = decay::decay_time(&s, Arrival::at(t_a), DecayRange::Edt)
                .unwrap()
                .t_s;
            println!(
                "T {t}, offset {f}: EDT by the first version {:+.1} %, now {:+.1e}",
                100.0 * (naive / t - 1.0),
                ours / t - 1.0
            );
            assert!(!decay_ok(naive, t), "T {t}, offset {f}: {naive}");
            assert!(decay_ok(ours, t));
        }
    }
}

#[test]
fn an_arrival_not_given_leaves_c_d_and_ts_unresolved() {
    for t in TS {
        for dt in DTS {
            for f in OFFSETS {
                let (s, _) = direct_and_decay(t, dt, f, DIRECT);
                let p = evaluate(&s, Arrival::Detected);
                let closed = Closed::direct_and_decay(t, DIRECT);
                let ok = gate_checks(&p, &closed);
                // The decay times are the same from either end of the onset bin.
                assert_eq!(ok[..3], [true; 3], "T {t}, dt {dt}, offset {f}");
                // At 10 ms, C, D and Ts are all refused. At 1 ms, C still is: where in the bin
                // the direct sound arrived moves it by more than 0.01 dB. D50 and Ts come through
                // at T = 3 s, and then meet their bounds. Where refused, the two ends bracket the
                // closed form.
                let mut refused = [false; 4];
                for (i, (r, want)) in [
                    (&p.c50_db, closed.c50),
                    (&p.c80_db, closed.c80),
                    (&p.d50, closed.d50),
                    (&p.ts_s, closed.ts),
                ]
                .into_iter()
                .enumerate()
                {
                    match r {
                        Ok(_) => assert!(ok[3 + i], "T {t}, dt {dt}, offset {f}: {r:?}"),
                        Err(e) => match e.not_evaluable() {
                            Some(NotEvaluable::Unresolved { low, high, .. }) => {
                                assert!(*low <= want && want <= *high, "{low} {want} {high}");
                                refused[i] = true;
                            }
                            other => panic!("T {t}, dt {dt}, offset {f}: {other:?}"),
                        },
                    }
                }
                println!("T {t}, dt {dt}, offset {f}: unresolved [C50, C80, D50, Ts] {refused:?}");
                let d_and_ts = !(dt == 0.001 && t == 3.0);
                assert_eq!(
                    refused,
                    [true, true, d_and_ts, d_and_ts],
                    "T {t}, dt {dt}, offset {f}"
                );
            }
        }
    }
}

#[test]
fn an_arrival_not_given_is_resolved_on_a_fine_enough_step() {
    // The check says yes when it should: at dt = 0.1 ms and T = 3 s, where in its bin the direct
    // sound arrived moves nothing past its limit, and every quantity meets the gate's bound.
    let (t, dt) = (3.0, 1e-4);
    for f in OFFSETS {
        let (s, _) = direct_and_decay(t, dt, f, DIRECT);
        let p = evaluate(&s, Arrival::Detected);
        assert_eq!(
            gate_checks(&p, &Closed::direct_and_decay(t, DIRECT)),
            [true; 7],
            "offset {f}: {p:?}"
        );
    }
}

#[test]
fn a_given_arrival_outside_its_onset_bin_refuses_c_d_and_ts_but_not_the_decay_times() {
    // A bin early, a bin late, and at 0 s: C50, C80, D50 and Ts are refused. The decay times do
    // not depend on where time starts; with an impulse, nothing before the arrival can be its
    // direct sound, so they are measured as if no arrival were given, and meet gate (a)'s bound.
    for t in TS {
        let (s, t_a) = direct_and_decay(t, 0.01, 0.5, DIRECT);
        for wrong in [t_a - 0.01, t_a + 0.01, 0.0] {
            let p = evaluate(&s, Arrival::at(wrong));
            for r in [&p.c50_db, &p.c80_db, &p.d50, &p.ts_s] {
                let e = r.as_ref().unwrap_err();
                assert_eq!(e.code(), codes::BAD_ARRIVAL, "{wrong}: {e}");
            }
            assert_eq!(p.decay_arrival, Arrival::Detected);
            let ok = gate_checks(&p, &Closed::direct_and_decay(t, DIRECT));
            assert_eq!(
                ok,
                [true, true, true, false, false, false, false],
                "T {t}, {wrong}"
            );
        }
        // Says no: the same series from its own arrival refuses nothing.
        let p = evaluate(&s, Arrival::at(t_a));
        assert_eq!(
            gate_checks(&p, &Closed::direct_and_decay(t, DIRECT)),
            [true; 7]
        );
    }
}
