//! A series known to be complete: `EnergySeries::complete` (M7 piece B's one change to piece A).
//!
//! **The defect.** `params` bounds the energy after a series' end from the series' last two
//! windows, and refuses everything that depends on it when the series is not decaying there
//! (`docs/params.md`, "Truncation"). SPPS in random mode, upstream's default, ends its histograms
//! with a few scattered particles, so its last window is often not decaying. On the M6 gate's
//! seeded tutorial box (10,000 particles, 27 bands) that refused all eight of SPL, EDT, T20, T30,
//! C50, C80, D50 and Ts in 22 of 27 bands at Receiver 1 and 20 of 27 at Receiver 2, although the
//! run's statistics say that no particle was alive when the calculation ended in any band:
//! nothing arrives after the series, and there is no tail to bound.
//!
//! **The fix.** A caller with that evidence says so with `EnergySeries::complete`; the tail is
//! then `Tail::Complete`, nothing is added and nothing is refused for it. `core::results` makes
//! the claim only for SPPS in random mode, band by band, when the statistics count 0 particles
//! remaining (`docs/results.md`).
//!
//! A complete series has no tail to bound, but its curve still has no shape inside its last bin
//! with energy, where it falls to nothing: a decay time whose range ends there is refused as
//! `range_not_reached`, with the level at the start of that bin as the depth reached.
//!
//! Each test says no:
//! - the same ragged series without the evidence is refused, every quantity;
//! - the evidence claimed for a series that is in truth cut short gives a C80 more than 0.01 dB
//!   off, where the honest series is refused: the claim changes numbers, so it is only made on the
//!   solver's own statistics;
//! - a complete series whose last bin starts at −30 dB gives T20, and refuses T30.

use std::f64::consts::LN_10;

use simpa_core::params::decay::{self, Arrival, P_REF_SQUARED, Tail, evaluate};
use simpa_core::params::{self, EnergySeries, NotEvaluable, codes};

const K60: f64 = 6.0 * LN_10;
const AT_ZERO: Arrival = Arrival::Known { time_s: 0.0 };

/// Bin integrals of `exp(−t/τ)` over `depth_db` of decay, then, far below, one stray bin such as
/// a last particle leaves in random mode: `stray_db` below the first bin, 5 bins later.
fn ragged(t: f64, dt: f64, depth_db: f64, stray_db: f64) -> Vec<f64> {
    let tau = t / K60;
    let n = (depth_db * t / 60.0 / dt).round() as usize;
    let mut v: Vec<f64> = (0..n)
        .map(|k| {
            let (a, b) = (k as f64 * dt, (k + 1) as f64 * dt);
            tau * ((-a / tau).exp() - (-b / tau).exp())
        })
        .collect();
    let stray = v[0] * 10f64.powf(-stray_db / 10.0);
    v.extend([0.0, 0.0, 0.0, 0.0, stray]);
    v
}

fn within(got: f64, want: f64, rel: f64) -> bool {
    (got / want - 1.0).abs() <= rel
}

#[test]
fn a_complete_series_with_a_ragged_end_is_refused_without_its_evidence() {
    let s = EnergySeries::new(0.01, ragged(1.0, 0.01, 70.0, 50.0)).unwrap();
    assert!(!s.is_complete());
    assert_eq!(decay::tail(&s).unwrap(), Tail::Unbounded);
    let p = evaluate(&s, AT_ZERO).unwrap();
    for (name, r) in [
        ("spl", p.spl_db.clone()),
        ("c50", p.c50_db.clone()),
        ("c80", p.c80_db.clone()),
        ("d50", p.d50.clone()),
        ("ts", p.ts_s.clone()),
    ] {
        let e = r.unwrap_err();
        assert!(
            matches!(
                e.not_evaluable(),
                Some(NotEvaluable::Truncated {
                    with_tail: None,
                    ..
                })
            ),
            "{name}: {e}"
        );
    }
    for r in [&p.edt, &p.t20, &p.t30] {
        assert_eq!(r.as_ref().unwrap_err().code(), codes::NOT_EVALUABLE);
    }
}

#[test]
fn with_its_evidence_it_meets_every_bound_of_gate_a() {
    for (t, dt) in [(0.3, 0.001), (1.0, 0.01), (3.0, 0.01)] {
        let v = ragged(t, dt, 70.0, 50.0);
        let total: f64 = v.iter().sum();
        let s = EnergySeries::complete(dt, v).unwrap();
        assert!(s.is_complete());
        assert_eq!(decay::tail(&s).unwrap(), Tail::Complete);
        let p = evaluate(&s, AT_ZERO).unwrap();
        // SPL is the sum, nothing added.
        let spl = p.spl_db.clone().unwrap();
        assert!(
            (spl - 10.0 * (total / P_REF_SQUARED).log10()).abs() < 1e-12,
            "{spl}"
        );
        let tau = t / K60;
        for (name, got) in [
            ("EDT", p.edt.clone().unwrap().t_s),
            ("T20", p.t20.clone().unwrap().t_s),
            ("T30", p.t30.clone().unwrap().t_s),
        ] {
            assert!(within(got, t, 0.005), "T = {t}, dt = {dt}: {name} {got}");
        }
        let c = |te: f64| 10.0 * ((te / tau).exp() - 1.0).log10();
        assert!((p.c50_db.clone().unwrap() - c(0.05)).abs() <= 0.01);
        assert!((p.c80_db.clone().unwrap() - c(0.08)).abs() <= 0.01);
        let d50 = 1.0 - (-0.05 / tau).exp();
        assert!(100.0 * (p.d50.clone().unwrap() - d50).abs() <= 0.1);
        assert!(within(p.ts_s.clone().unwrap(), tau, 0.005));
    }
}

/// Bin integrals of `exp(−t/τ)` over `depth_db` of decay, cut there.
fn cut(t: f64, dt: f64, depth_db: f64) -> Vec<f64> {
    let tau = t / K60;
    let n = (depth_db * t / 60.0 / dt).round() as usize;
    (0..n)
        .map(|k| {
            let (a, b) = (k as f64 * dt, (k + 1) as f64 * dt);
            tau * ((-a / tau).exp() - (-b / tau).exp())
        })
        .collect()
}

#[test]
fn the_evidence_claimed_for_a_series_cut_short_gives_a_wrong_c80() {
    // An exponential stopped at −20 dB: 1 % of its energy is missing, all of it late.
    let (t, dt) = (1.0, 0.001);
    let tau = t / K60;
    let c80 = 10.0 * ((0.08 / tau).exp() - 1.0).log10();
    let v = cut(t, dt, 20.0);
    let lie = evaluate(&EnergySeries::complete(dt, v.clone()).unwrap(), AT_ZERO).unwrap();
    let got = lie.c80_db.clone().unwrap();
    assert!((got - c80).abs() > 0.01, "C80 {got} vs {c80}");
    // Without the claim the same series is refused, not given a number.
    let honest = evaluate(&EnergySeries::new(dt, v).unwrap(), AT_ZERO).unwrap();
    assert!(
        matches!(
            honest.c80_db.unwrap_err().not_evaluable(),
            Some(NotEvaluable::Truncated { .. })
        ),
        "the honest C80 is refused as truncated"
    );
}

#[test]
fn a_complete_series_is_fitted_only_down_to_its_last_bin() {
    // An exponential to −30 dB, then one last bin holding all the energy after it, as a last
    // particle would: the curve is exact down to −30 dB and has no shape below. T20's range ends
    // above −30 dB; T30's, at −35 dB, ends inside the last bin, which the fit leaves out, so its
    // regression would cover −5 to −30 dB only. It is refused. (Before this check T30 came out
    // equal to T20 to every digit on the committed Seat run, 500 Hz.)
    let (t, dt) = (1.0, 0.01);
    let tau = t / K60;
    let mut v = cut(t, dt, 30.0);
    v.push(tau * (-(v.len() as f64) * dt / tau).exp());
    let p = evaluate(&EnergySeries::complete(dt, v).unwrap(), AT_ZERO).unwrap();
    let t20 = p.t20.clone().unwrap().t_s;
    assert!(within(t20, t, 0.005), "T20 {t20}");
    match p.t30.clone().unwrap_err().not_evaluable() {
        Some(NotEvaluable::RangeNotReached {
            needed_db,
            reached_db,
        }) => {
            assert_eq!(*needed_db, -35.0);
            assert!((reached_db + 30.0).abs() < 0.01, "{reached_db}");
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn an_aggregate_is_complete_only_when_every_band_is() {
    let a = EnergySeries::complete(0.01, vec![1.0, 0.5, 0.25]).unwrap();
    let b = EnergySeries::complete(0.01, vec![2.0, 1.0, 0.0]).unwrap();
    let c = EnergySeries::new(0.01, vec![2.0, 1.0, 0.0]).unwrap();
    assert!(params::aggregate(&[a.clone(), b]).unwrap().is_complete());
    assert!(!params::aggregate(&[a, c]).unwrap().is_complete());
}
