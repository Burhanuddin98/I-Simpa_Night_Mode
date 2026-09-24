//! M7 gate (a): exact exponential decays `E(t) = E0·exp(−6·ln10·t/T)` (the gate writes the
//! constant as 13.8155) with T in {0.3, 1.0, 3.0} s and dt in {0.001, 0.01} s. T20, T30 and EDT
//! must be within 0.5 % of T, C80 and C50 within 0.01 dB and D50 within 0.1 percentage points of
//! their closed forms, and Ts equal to the histogram's closed form (`docs/params.md`).
//!
//! Every check has a partner that makes it say no:
//! - a decay with T off by 1 % fails every bound; a decay off only where one parameter looks fails
//!   that parameter and no other;
//! - a series truncated before −35 dB gives `range_not_reached` for T30, not a number, although
//!   the truncated curve itself passes −35 dB; a series truncated at −40 dB gives `truncated`;
//! - a double-slope decay is flagged by the T20/T30 disagreement, a single slope is not.
//!
//! The series are bin integrals of the exponential, so bin `k` holds the energy of
//! `[k·dt, (k+1)·dt)` exactly, as SPPS's histogram does.

use std::f64::consts::LN_10;

use simpa_core::params::decay::{
    self, BandParameters, DecayRange, P_REF_SQUARED, evaluate, schroeder_db,
};
use simpa_core::params::{EnergySeries, NotEvaluable, codes};

/// `6·ln(10)` = 13.815510…, the gate's 13.8155.
const K60: f64 = 6.0 * LN_10;
const TS: [f64; 3] = [0.3, 1.0, 3.0];
const DTS: [f64; 2] = [0.001, 0.01];
/// Energy scale, Pa²: 1 Pa²·s of total energy is about 94 dB.
const E0: f64 = 1.0;

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

// The closed forms.
fn clarity_closed(te: f64, t: f64) -> f64 {
    10.0 * ((te / tau(t)).exp() - 1.0).log10()
}
fn definition_closed(te: f64, t: f64) -> f64 {
    1.0 - (-te / tau(t)).exp()
}
/// Ts of the histogram, each bin's energy at its midpoint: `dt·(1/(e^x − 1) + 1/2)`, `x = dt/τ`.
fn centre_time_histogram(t: f64, dt: f64) -> f64 {
    let x = dt / tau(t);
    dt * (1.0 / x.exp_m1() + 0.5)
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

/// Whether each gated quantity of `p` meets its bound against the closed forms at `t`:
/// `[EDT, T20, T30, C50, C80, D50]`. A refused quantity does not meet it.
fn gate_checks(p: &BandParameters, t: f64) -> [bool; 6] {
    let d = |r: &Result<decay::DecayFit, _>| r.as_ref().is_ok_and(|f| decay_ok(f.t_s, t));
    [
        d(&p.edt),
        d(&p.t20),
        d(&p.t30),
        p.c50_db
            .as_ref()
            .is_ok_and(|&c| clarity_ok(c, clarity_closed(0.05, t))),
        p.c80_db
            .as_ref()
            .is_ok_and(|&c| clarity_ok(c, clarity_closed(0.08, t))),
        p.d50
            .as_ref()
            .is_ok_and(|&x| definition_ok(x, definition_closed(0.05, t))),
    ]
}

#[test]
fn exact_decays_meet_every_bound() {
    for t in TS {
        for dt in DTS {
            let s = exact_decay(t, dt, 150.0);
            let p = evaluate(&s);
            let (edt, t20, t30) = (
                p.edt.clone().unwrap(),
                p.t20.clone().unwrap(),
                p.t30.clone().unwrap(),
            );
            let (c50, c80, d50) = (
                p.c50_db.clone().unwrap(),
                p.c80_db.clone().unwrap(),
                p.d50.clone().unwrap(),
            );
            println!(
                "T {t} s, dt {dt} s: EDT {:+.2e}, T20 {:+.2e}, T30 {:+.2e} (relative); \
                 C50 {:+.1e} dB, C80 {:+.1e} dB, D50 {:+.1e} points",
                edt.t_s / t - 1.0,
                t20.t_s / t - 1.0,
                t30.t_s / t - 1.0,
                c50 - clarity_closed(0.05, t),
                c80 - clarity_closed(0.08, t),
                100.0 * (d50 - definition_closed(0.05, t)),
            );
            assert_eq!(gate_checks(&p, t), [true; 6], "T {t}, dt {dt}");
            // Tighter than the gate: the regression on an exact exponential is exact.
            for f in [&edt, &t20, &t30] {
                assert!((f.t_s / t - 1.0).abs() < 1e-9, "{f:?}");
            }
            assert_eq!(p.onset.index, 0);
            // Ts: the histogram's closed form, and its midpoint bias against τ is (dt/τ)²/12.
            let ts = p.ts_s.clone().unwrap();
            let want = centre_time_histogram(t, dt);
            assert!((ts / want - 1.0).abs() < 1e-9, "Ts {ts} vs {want}");
            let x = dt / tau(t);
            let bias = ts / tau(t) - 1.0;
            assert!(
                (bias - x * x / 12.0).abs() <= 0.02 * x * x / 12.0 + 1e-12,
                "Ts bias {bias} vs (dt/τ)²/12 = {}",
                x * x / 12.0
            );
            // SPL: the sum over p0².
            let spl = p.spl_db.clone().unwrap();
            assert!((spl - 10.0 * (s.total() / P_REF_SQUARED).log10()).abs() < 1e-9);
            // A single slope is not curved.
            let c = p.curvature.clone().unwrap();
            assert!(!c.curved && c.percent.abs() < 1e-6, "{c:?}");
        }
    }
}

#[test]
fn a_decay_one_percent_off_fails_every_bound() {
    for t in TS {
        for dt in DTS {
            let p = evaluate(&exact_decay(1.01 * t, dt, 150.0));
            // Every quantity is computed (none refused), and every one misses its bound.
            for r in [&p.edt, &p.t20, &p.t30] {
                assert!(r.is_ok());
            }
            assert!(p.c50_db.is_ok() && p.c80_db.is_ok() && p.d50.is_ok());
            assert_eq!(gate_checks(&p, t), [false; 6], "T {t}, dt {dt}");
        }
    }
}

#[test]
fn a_decay_off_only_where_one_parameter_looks_fails_that_parameter_only() {
    for t in TS {
        let dt = 0.001;
        let n = (150.0 * t / 60.0 / dt).round() as usize;
        let slope = -60.0 / t;
        // 5 % faster above −5 dB: only EDT sees it. T20 and T30 fit points at or below −5 dB,
        // where the slope is exact.
        let t5 = -5.0 / (1.05 * slope);
        let early = prescribed_curve(dt, n, |x| {
            if x <= t5 {
                1.05 * slope * x
            } else {
                -5.0 + slope * (x - t5)
            }
        });
        let [edt, t20, t30, ..] = gate_checks(&evaluate(&early), t);
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
        let [edt, t20, t30, ..] = gate_checks(&evaluate(&late), t);
        assert_eq!([edt, t20, t30], [true, true, false], "late kink, T {t}");
    }
}

/// T30 by a least-squares line through the truncated curve's points in −5 … −35 dB, nothing
/// added for the tail: what upstream's GUI does, bar its one extra point
/// (`projet_calculation.cpp:143-170`). `None` when the curve never reaches −35 dB.
fn naive_t30(s: &EnergySeries) -> Option<f64> {
    let pts: Vec<(f64, f64)> = schroeder_db(s)
        .into_iter()
        .filter(|&(_, l)| (-35.0..=-5.0).contains(&l))
        .collect();
    if schroeder_db(s).iter().all(|&(_, l)| l > -35.0) || pts.len() < 2 {
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
            let e = decay::decay_time(&s, DecayRange::T30).unwrap_err();
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
            // 30 dB is 0.36 %, inside its 0.5 % limit).
            let p = evaluate(&s);
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
            let e = decay::decay_time(&exact_decay(t, dt, 40.0), DecayRange::T30).unwrap_err();
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
            let fit = decay::decay_time(&exact_decay(t, dt, 60.0), DecayRange::T30).unwrap();
            assert!(decay_ok(fit.t_s, t), "{fit:?}");
        }
    }
}

/// `docs/params.md`'s table: T30 of the truncated curve with nothing added, `T = 1 s`,
/// `dt = 1 ms`, against the depth the true decay reached; and the depth from which our bound
/// lets T30 through.
#[test]
fn the_documented_truncation_table_holds() {
    let table = [
        (30.0, -12.2),
        (35.0, -6.1),
        (40.0, -2.4),
        (45.0, -0.85),
        (50.0, -0.28),
        (55.0, -0.09),
        (60.0, -0.03),
    ];
    for (depth, percent) in table {
        let n = naive_t30(&exact_decay(1.0, 0.001, depth)).unwrap();
        let got = 100.0 * (n - 1.0);
        // Two significant figures, as printed.
        assert!(
            (got - percent).abs() <= 0.051 * percent.abs().max(1.0),
            "D {depth}: {got:.3} % vs {percent} %"
        );
    }
    let first_pass = (30..=80)
        .find(|&d| decay::decay_time(&exact_decay(1.0, 0.001, d as f64), DecayRange::T30).is_ok())
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
        let p = evaluate(&EnergySeries::new(dt, v).unwrap());
        let c = p.curvature.clone().unwrap();
        println!(
            "dt {dt}: T20 {:.3} s, T30 {:.3} s, curvature {:.1} %",
            p.t20.as_ref().unwrap().t_s,
            p.t30.as_ref().unwrap().t_s,
            c.percent
        );
        assert!(c.curved && c.percent > 10.0, "{c:?}");
        // The single slope at the same resolution is not flagged (exact_decays_meet_every_bound).
        let single = evaluate(&exact_decay(t1, dt, 150.0)).curvature.unwrap();
        assert!(!single.curved);
    }
}
