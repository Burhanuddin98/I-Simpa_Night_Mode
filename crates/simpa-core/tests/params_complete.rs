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
const AT_ZERO: Arrival = Arrival::at(0.0);

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
    let p = evaluate(&s, AT_ZERO);
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
        let p = evaluate(&s, AT_ZERO);
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
    let lie = evaluate(&EnergySeries::complete(dt, v.clone()).unwrap(), AT_ZERO);
    let got = lie.c80_db.clone().unwrap();
    assert!((got - c80).abs() > 0.01, "C80 {got} vs {c80}");
    // Without the claim the same series is refused, not given a number.
    let honest = evaluate(&EnergySeries::new(dt, v).unwrap(), AT_ZERO);
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
    let p = evaluate(&EnergySeries::complete(dt, v).unwrap(), AT_ZERO);
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

/// Random mode with `n` of `n_total` particles lost at `t_lost`: every particle deposits at the
/// same rate while alive, and particles are absorbed at rate `1/τ`, so the receiver's histogram is
/// `N·e^{−t/τ}` per unit time and each lost particle takes `e^{−(t − t_lost)/τ}` with it. Bin
/// integrals, the arrival at 0.
fn with_lost(t: f64, dt: f64, n: f64, n_total: f64, t_lost: f64) -> (Vec<f64>, Vec<f64>) {
    let tau = t / K60;
    let bins = (80.0 * t / 60.0 / dt).round() as usize;
    let bin = |a: f64, b: f64, from: f64| -> f64 {
        let (a, b) = (a.max(from), b.max(from));
        tau * ((-(a - from) / tau).exp() - (-(b - from) / tau).exp())
    };
    let truth: Vec<f64> = (0..bins)
        .map(|k| n_total * bin(k as f64 * dt, (k + 1) as f64 * dt, 0.0))
        .collect();
    let lost: Vec<f64> = (0..bins)
        .map(|k| {
            let (a, b) = (k as f64 * dt, (k + 1) as f64 * dt);
            n_total * bin(a, b, 0.0) - n * bin(a, b, t_lost)
        })
        .collect();
    (truth, lost)
}

#[test]
fn lost_particles_move_no_accepted_value_beyond_its_limit() {
    // The share core::results gives: n / (N·f) with f the share alive at the arrival, 1 here.
    let (t, dt, n_total) = (1.0, 0.01, 150_000.0);
    let tau = t / K60;
    let mut refused_with = 0;
    for t_lost in [0.1, 0.5] {
        // Lost particles must have been alive: at most N·e^{−t/τ} of them.
        let alive = n_total * (-t_lost / tau).exp();
        for n in [1.0, 10.0, 100.0] {
            if n > alive {
                continue;
            }
            let (truth, lost) = with_lost(t, dt, n, n_total, t_lost);
            let want = evaluate(&EnergySeries::complete(dt, truth).unwrap(), AT_ZERO);
            let plain = evaluate(&EnergySeries::complete(dt, lost.clone()).unwrap(), AT_ZERO);
            let s = EnergySeries::complete(dt, lost)
                .unwrap()
                .with_lost_share(n / n_total)
                .unwrap();
            assert!(s.is_complete(), "a lost share keeps the series complete");
            let got = evaluate(&s, AT_ZERO);
            let pairs = [
                (
                    "T20",
                    want.t20.clone().map(|f| f.t_s),
                    plain.t20.clone().map(|f| f.t_s),
                    got.t20.clone().map(|f| f.t_s),
                    0.005,
                    true,
                ),
                (
                    "T30",
                    want.t30.clone().map(|f| f.t_s),
                    plain.t30.clone().map(|f| f.t_s),
                    got.t30.clone().map(|f| f.t_s),
                    0.005,
                    true,
                ),
                (
                    "C80",
                    want.c80_db.clone(),
                    plain.c80_db.clone(),
                    got.c80_db.clone(),
                    0.01,
                    false,
                ),
                (
                    "D50",
                    want.d50.clone(),
                    plain.d50.clone(),
                    got.d50.clone(),
                    0.001,
                    false,
                ),
                (
                    "SPL",
                    want.spl_db.clone(),
                    plain.spl_db.clone(),
                    got.spl_db.clone(),
                    0.1,
                    false,
                ),
            ];
            for (name, w, p, g, limit, rel) in pairs {
                let w = w.unwrap();
                let off = |x: f64| {
                    if rel {
                        (x / w - 1.0).abs()
                    } else {
                        (x - w).abs()
                    }
                };
                match g {
                    Ok(g) => assert!(off(g) <= limit, "t_lost {t_lost} n {n}: {name} {g} vs {w}"),
                    Err(e) => {
                        assert!(
                            matches!(
                                e.not_evaluable(),
                                Some(
                                    NotEvaluable::MissingMoves {
                                        lost_share: Some(_),
                                        ..
                                    } | NotEvaluable::MissingNotCleared {
                                        lost_share: Some(_),
                                        ..
                                    }
                                )
                            ),
                            "{name}: {e}"
                        );
                        // The value the series alone gives is wrong: the refusal is earned.
                        if let Ok(p) = p
                            && off(p) > limit
                        {
                            refused_with += 1;
                        }
                    }
                }
            }
        }
    }
    // The defect it closes: 100 particles lost at 0.5 s, two thirds of what was then alive, make
    // T30 wrong from the series alone, and the share refuses it.
    assert!(refused_with >= 1, "{refused_with}");
    // One lost particle of 150,000 moves nothing: T30 from 1 s of decay passes.
    let (_, lost) = with_lost(t, dt, 1.0, n_total, 0.5);
    let s = EnergySeries::complete(dt, lost)
        .unwrap()
        .with_lost_share(1.0 / n_total)
        .unwrap();
    assert!(evaluate(&s, AT_ZERO).t30.is_ok());
    // A share that is not a number of at least 0 is refused.
    for bad in [-1e-6, f64::NAN, f64::INFINITY] {
        assert_eq!(
            EnergySeries::new(dt, vec![1.0, 0.5])
                .unwrap()
                .with_lost_share(bad)
                .unwrap_err()
                .code(),
            codes::BAD_NOISE_INPUT
        );
    }
}

/// Energetic mode: every particle carries energy that falls with the room's, so `n` particles lost
/// at `t_lost` with `ratio` times the mean energy then take `ratio·n/N` of the energy from
/// `t_lost` on. Bin integrals of `e^{−t/τ}`, the arrival at 0: the truth and the series with the
/// loss.
fn energetic_lost(t: f64, dt: f64, share: f64, t_lost: f64) -> (Vec<f64>, Vec<f64>) {
    let tau = t / K60;
    let bins = (80.0 * t / 60.0 / dt).round() as usize;
    let s = |u: f64| (-u / tau).exp();
    let truth: Vec<f64> = (0..bins)
        .map(|k| s(k as f64 * dt) - s((k + 1) as f64 * dt))
        .collect();
    let lost = truth
        .iter()
        .enumerate()
        .map(|(k, v)| {
            let (a, b) = (k as f64 * dt, (k + 1) as f64 * dt);
            let gone = share * (s(a.max(t_lost)) - s(b.max(t_lost)));
            v - gone
        })
        .collect();
    (truth, lost)
}

#[test]
fn an_energetic_lost_share_that_follows_the_decay_is_bounded_by_it() {
    let (t, dt) = (1.0, 0.01);
    let all = |p: &simpa_core::params::decay::BandParameters| {
        [
            p.spl_db.is_ok(),
            p.edt.is_ok(),
            p.t20.is_ok(),
            p.t30.is_ok(),
            p.c50_db.is_ok(),
            p.c80_db.is_ok(),
            p.d50.is_ok(),
            p.ts_s.is_ok(),
        ]
    };
    // The share core::results gives for tutorial 1 in energetic mode: 10 × 4/150,000, the most lost
    // in a band there. Whenever the particles were lost, every quantity comes out, within its
    // limit of the truth.
    let share = 10.0 * 4.0 / 150_000.0;
    for t_lost in [0.02, 0.2, 0.5] {
        let (truth, lost) = energetic_lost(t, dt, share, t_lost);
        let want = evaluate(&EnergySeries::complete(dt, truth).unwrap(), AT_ZERO);
        let s = EnergySeries::complete(dt, lost.clone())
            .unwrap()
            .with_lost_share_following_decay(share)
            .unwrap();
        assert!(s.lost_follows_decay());
        let got = evaluate(&s, AT_ZERO);
        assert_eq!(all(&got), [true; 8], "t_lost {t_lost}: {got:?}");
        let t30 = got.t30.as_ref().unwrap().t_s;
        assert!(within(t30, want.t30.unwrap().t_s, 0.005), "t_lost {t_lost}");
        let c80 = got.c80_db.clone().unwrap();
        assert!((c80 - want.c80_db.clone().unwrap()).abs() <= 0.01);

        // Says no: the same share as a lump added at the end, random mode's bound, refuses T30.
        let lump = EnergySeries::complete(dt, lost)
            .unwrap()
            .with_lost_share(share)
            .unwrap();
        let e = evaluate(&lump, AT_ZERO).t30.unwrap_err();
        assert!(
            matches!(e.not_evaluable(), Some(NotEvaluable::MissingMoves { .. })),
            "t_lost {t_lost}: {e}"
        );
    }
    // Says no: a share that follows the decay but is large enough to move a decay time beyond
    // 0.5 %, 3 %, refuses it and C80.
    let (_, lost) = energetic_lost(t, dt, 0.03, 0.2);
    let s = EnergySeries::complete(dt, lost)
        .unwrap()
        .with_lost_share_following_decay(0.03)
        .unwrap();
    let p = evaluate(&s, AT_ZERO);
    for r in [p.t30.as_ref().map(|_| ()), p.c80_db.as_ref().map(|_| ())] {
        assert!(
            matches!(
                r.unwrap_err().not_evaluable(),
                Some(NotEvaluable::MissingMoves { .. })
            ),
            "{p:?}"
        );
    }
    // An aggregate follows the decay only when every band's lost share does.
    let a = EnergySeries::complete(dt, vec![1.0, 0.5, 0.25])
        .unwrap()
        .with_lost_share_following_decay(1e-4)
        .unwrap();
    let b = EnergySeries::complete(dt, vec![1.0, 0.5, 0.25])
        .unwrap()
        .with_lost_share(1e-5)
        .unwrap();
    let both = params::aggregate(&[a.clone(), a.clone()]).unwrap();
    assert!(both.lost_follows_decay() && both.lost_share() == Some(1e-4));
    let mixed = params::aggregate(&[a, b]).unwrap();
    assert!(!mixed.lost_follows_decay() && mixed.lost_share() == Some(1e-4));
}

#[test]
fn the_floor_and_a_lost_share_that_follows_the_decay_are_bounded_together() {
    // Energetic mode has both: the floor's dropped energy, bounded as a lump, and the lost
    // particles' share, bounded as a scaling of the curve. Each alone within the limit is not
    // enough: both can be missing at once, so what the two can move a value by is added.
    // (Review finding: the follow-ups checked each against the full limit alone.)
    let (t, dt) = (1.0, 0.01);
    let tau = t / K60;
    let v: Vec<f64> = (0..(90.0 * t / 60.0 / dt).round() as usize)
        .map(|k| {
            let (a, b) = (k as f64 * dt, (k + 1) as f64 * dt);
            tau * ((-a / tau).exp() - (-b / tau).exp())
        })
        .collect();
    let series = |alive: Option<f64>, share: Option<f64>| {
        let mut s = EnergySeries::new(dt, v.clone()).unwrap();
        if let Some(a) = alive {
            s = s.with_solver_floor(-55.0, a).unwrap();
        }
        if let Some(x) = share {
            s = s.with_lost_share_following_decay(x).unwrap();
        }
        evaluate(&s, AT_ZERO).t30
    };
    // Nothing missing: T30 is T.
    assert!(within(series(None, None).unwrap().t_s, t, 1e-6));
    // The edges: the floor's alive share, and the lost share, at which each alone moves T30 by
    // exactly the limit. Accepted on one side, refused on the other.
    let edge = |ok: &dyn Fn(f64) -> bool, mut lo: f64, mut hi: f64| {
        // `ok(lo)` is false, `ok(hi)` true.
        assert!(!ok(lo) && ok(hi));
        for _ in 0..60 {
            let mid = (lo * hi).sqrt();
            if ok(mid) {
                hi = mid;
            } else {
                lo = mid;
            }
        }
        hi
    };
    let alive_edge = edge(&|a| series(Some(a), None).is_ok(), 1e-3, 1.0);
    let share_edge = 1.0 / edge(&|inv| series(None, Some(1.0 / inv)).is_ok(), 1.0, 1e6);
    println!("alone at the limit: alive share {alive_edge:.4}, lost share {share_edge:.3e}");
    // Each at a little more than half the limit: each alone passes.
    let (alive, share) = (alive_edge * 1.8, share_edge * 0.55);
    assert!(
        alive * 4.0 / 1.8 < 1.0,
        "an alive share is at most 1: {alive}"
    );
    assert!(series(Some(alive), None).is_ok());
    assert!(series(None, Some(share)).is_ok());
    // Together they are refused, missing_moves, with T30 moved past the limit.
    let e = series(Some(alive), Some(share)).unwrap_err();
    match e.not_evaluable() {
        Some(NotEvaluable::MissingMoves {
            value,
            with_missing: Some(w),
            limit,
            ..
        }) => assert!((w / value - 1.0).abs() > *limit, "{e}"),
        other => panic!("{other:?}"),
    }
    // Well inside the limit together, they pass.
    assert!(series(Some(alive_edge * 4.0), Some(share_edge * 0.2)).is_ok());
}

#[test]
fn an_aggregate_is_complete_only_when_every_band_is() {
    let a = EnergySeries::complete(0.01, vec![1.0, 0.5, 0.25]).unwrap();
    let b = EnergySeries::complete(0.01, vec![2.0, 1.0, 0.0]).unwrap();
    let c = EnergySeries::new(0.01, vec![2.0, 1.0, 0.0]).unwrap();
    assert!(params::aggregate(&[a.clone(), b]).unwrap().is_complete());
    assert!(!params::aggregate(&[a, c]).unwrap().is_complete());
}
