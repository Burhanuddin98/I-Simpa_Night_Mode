//! The solver's floor: SPPS in energetic mode drops each particle once its energy falls to
//! `10^-trans_epsilon` of its start (`spps/sppsNantes.cpp:75`; `CalculationCore.cpp:57-60,
//! 305-310`), so a histogram loses its late energy in a cliff (`EnergySeries::with_solver_floor`,
//! `docs/params.md`, "The solver's floor").
//!
//! **The defect** (M7 review, 2026-09-24). Inside the cliff the last window falls steeply, the
//! tail estimate comes out near 0, and the decay reads as deep as it likes: with
//! `trans_epsilon` 3 or 4, T20 and T30 came out short by 2 to 13 % and were accepted.
//!
//! **The model** the tests are held to (the reviewer's, in closed form): tutorial 1's box
//! (V = 180 m³, S = 216 m²), reflections a Poisson process at `c/(4V/S)`, every surface absorbing
//! `α`, so a particle's energy after `N` reflections is `(1 − α)^N`, dropped once that is at or
//! below `10^-ε`. The expected histogram at a receiver `r` metres from the source is
//! `E[(1 − α)^N·1(kept)]` from `r/c` on, in 10 ms bins, plus a direct sound of
//! `S·α/(16πr²)` of the reverberant energy at `r/c`; a bin where fewer than one particle in 10⁵
//! is still kept is empty, as a finite run's would be. The reference is the same without the drop.
//!
//! Each test says no:
//! - without the floor, the cliff's T30 is accepted and 10 % short; with it, refused;
//! - over α from 0.05 to 0.9, ε from 1 to 7 and r of 2 and 8 m, no value the floor accepts is
//!   further from the reference than its limit, and the cases the model makes wrong are refused;
//! - at upstream's default ε = 5 in tutorial 1's room, T30 is still accepted: the rule is not a
//!   blanket refusal.

use simpa_core::params::decay::{self, Arrival, DecayRange, limits};
use simpa_core::params::{self, EnergySeries, NotEvaluable, codes};

const C: f64 = 343.2;
const V: f64 = 180.0;
const S: f64 = 216.0;
const DT: f64 = 0.01;

/// `ln Γ(x)`, Lanczos (g = 7, 9 terms).
fn ln_gamma(x: f64) -> f64 {
    const G: [f64; 9] = [
        0.999_999_999_999_809_9,
        676.520_368_121_885_1,
        -1_259.139_216_722_402_8,
        771.323_428_777_653_1,
        -176.615_029_162_140_6,
        12.507_343_278_686_905,
        -0.138_571_095_265_720_12,
        9.984_369_578_019_572e-6,
        1.505_632_735_149_311_6e-7,
    ];
    let x = x - 1.0;
    let mut a = G[0];
    let t = x + 7.5;
    for (i, g) in G.iter().enumerate().skip(1) {
        a += g / (x + i as f64);
    }
    0.5 * (2.0 * std::f64::consts::PI).ln() + (x + 0.5) * t.ln() - t + a.ln()
}

/// `P(N < n)` for `N` Poisson of mean `mu`: the regularised upper incomplete gamma `Q(n, mu)`.
fn poisson_below(n: u32, mu: f64) -> f64 {
    if n == 0 {
        return 0.0;
    }
    if mu <= 0.0 {
        return 1.0;
    }
    let a = f64::from(n);
    let front = (-mu + a * mu.ln() - ln_gamma(a)).exp();
    if mu < a + 1.0 {
        // The series for P(a, mu), then Q = 1 − P.
        let (mut sum, mut term, mut ap) = (1.0 / a, 1.0 / a, a);
        for _ in 0..10_000 {
            ap += 1.0;
            term *= mu / ap;
            sum += term;
            if term < sum * 1e-16 {
                break;
            }
        }
        (1.0 - sum * front).max(0.0)
    } else {
        // The continued fraction for Q (modified Lentz).
        let tiny = 1e-300;
        let mut b = mu + 1.0 - a;
        let mut c = 1.0 / tiny;
        let mut d = 1.0 / b;
        let mut h = d;
        for i in 1..10_000 {
            let an = -(i as f64) * (i as f64 - a);
            b += 2.0;
            d = an * d + b;
            if d.abs() < tiny {
                d = tiny;
            }
            c = b + an / c;
            if c.abs() < tiny {
                c = tiny;
            }
            d = 1.0 / d;
            let del = d * c;
            h *= del;
            if (del - 1.0).abs() < 1e-16 {
                break;
            }
        }
        (front * h).clamp(0.0, 1.0)
    }
}

/// The model's histogram and the arrival `r/c`; `eps = None` for the reference without the drop.
fn histogram(alpha: f64, eps: Option<f64>, r: f64, duration: f64) -> (Vec<f64>, f64) {
    let lam = C / (4.0 * V / S);
    let t_d = r / C;
    let n_bins = (duration / DT).round() as usize;
    let sub = 8;
    let ncut = eps.map(|e| ((-e * std::f64::consts::LN_10) / (1.0 - alpha).ln() - 1e-12).ceil());
    let mut out = vec![0.0; n_bins];
    for (k, bin) in out.iter_mut().enumerate() {
        let mut sum = 0.0;
        for j in 0..sub {
            let t = (k as f64 + (j as f64 + 0.5) / sub as f64) * DT;
            if t < t_d {
                continue;
            }
            let mu = lam * t;
            let mean = (-mu * alpha).exp();
            sum += match ncut {
                None => mean,
                Some(n) => {
                    let n = n.max(0.0) as u32;
                    if poisson_below(n, mu) * 1e5 < 1.0 {
                        0.0
                    } else {
                        mean * poisson_below(n, mu * (1.0 - alpha))
                    }
                }
            };
        }
        *bin = sum / sub as f64 * DT;
    }
    let reverberant: f64 = out.iter().sum();
    let k0 = (t_d / DT) as usize;
    out[k0] += S * alpha / (16.0 * std::f64::consts::PI * r * r) * reverberant;
    (out, t_d)
}

/// The share of the emitted energy the room still holds at the arrival `t_a`: `E[(1 − α)^N]`,
/// what SPPS's room table over the sources' power gives (`results::spps::SppsResults::alive_share`).
fn alive(alpha: f64, t_a: f64) -> f64 {
    (-(C / (4.0 * V / S)) * alpha * t_a).exp()
}

/// The eight values of `series` from `arrival`, `None` where refused, with the refusals' reasons.
fn values(series: &EnergySeries, arrival: f64) -> Vec<(&'static str, Result<f64, String>)> {
    let p = decay::evaluate(series, Arrival::at(arrival));
    let why = |e: simpa_core::params::ParamError| e.to_string();
    vec![
        ("SPL", p.spl_db.map_err(why)),
        ("EDT", p.edt.map(|f| f.t_s).map_err(why)),
        ("T20", p.t20.map(|f| f.t_s).map_err(why)),
        ("T30", p.t30.map(|f| f.t_s).map_err(why)),
        ("C50", p.c50_db.map_err(why)),
        ("C80", p.c80_db.map_err(why)),
        ("D50", p.d50.map_err(why)),
        ("Ts", p.ts_s.map_err(why)),
    ]
}

/// Whether `got` is within the quantity's limit of `want` (`params::decay::limits`).
fn within(name: &str, got: f64, want: f64) -> bool {
    match name {
        "EDT" | "T20" | "T30" => (got / want - 1.0).abs() <= limits::DECAY_RELATIVE,
        "C50" | "C80" => (got - want).abs() <= limits::CLARITY_DB,
        "D50" => (got - want).abs() <= limits::DEFINITION,
        "Ts" => {
            (got - want).abs() <= limits::CENTRE_TIME_S.min(limits::CENTRE_TIME_RELATIVE * want)
        }
        "SPL" => (got - want).abs() <= limits::SPL_DB,
        _ => unreachable!(),
    }
}

fn duration(alpha: f64) -> f64 {
    if alpha >= 0.1 { 3.0 } else { 8.0 }
}

#[test]
fn the_cliff_gives_a_short_t30_without_the_floor_and_a_refusal_with_it() {
    let (alpha, eps, r) = (0.2, 3.0, 4.472);
    let (reference, t_a) = histogram(alpha, None, r, 3.0);
    let (cliff, _) = histogram(alpha, Some(eps), r, 3.0);
    let t30_ref = decay::decay_time(
        &EnergySeries::new(DT, reference).unwrap(),
        Arrival::at(t_a),
        DecayRange::T30,
    )
    .unwrap()
    .t_s;
    // The defect: nothing in the series itself refuses the cliff.
    let plain = EnergySeries::new(DT, cliff.clone()).unwrap();
    let t30 = decay::decay_time(&plain, Arrival::at(t_a), DecayRange::T30)
        .unwrap()
        .t_s;
    println!("without the floor: T30 {t30:.4} s against {t30_ref:.4} s");
    assert!(t30 / t30_ref - 1.0 < -0.10, "{t30} vs {t30_ref}");
    // The fix: the same series with the solver's floor at -30 dB.
    let floored = plain
        .with_solver_floor(-10.0 * eps, alive(alpha, t_a))
        .unwrap();
    for range in [DecayRange::T20, DecayRange::T30] {
        let e = decay::decay_time(&floored, Arrival::at(t_a), range).unwrap_err();
        assert_eq!(e.code(), codes::NOT_EVALUABLE);
        assert!(
            matches!(
                e.not_evaluable(),
                Some(
                    NotEvaluable::MissingNotCleared {
                        floor_db: Some(_),
                        ..
                    } | NotEvaluable::MissingMoves {
                        floor_db: Some(_),
                        ..
                    }
                )
            ),
            "{range:?}: {e}"
        );
        println!("with the floor, {range:?}: {e}");
    }
}

#[test]
fn nothing_the_floor_accepts_is_further_than_its_limit_from_the_reference() {
    let mut wrong = Vec::new();
    let (mut accepted, mut refused, mut caught) = (0, 0, 0);
    for alpha in [0.05, 0.1, 0.2, 0.4, 0.7, 0.9] {
        for r in [2.0, 8.0] {
            let (reference, t_a) = histogram(alpha, None, r, duration(alpha));
            let want = values(&EnergySeries::new(DT, reference).unwrap(), t_a);
            for eps in [1.0, 2.0, 3.0, 3.5, 4.0, 4.5, 5.0, 6.0, 7.0] {
                let (cliff, _) = histogram(alpha, Some(eps), r, duration(alpha));
                let plain = values(&EnergySeries::new(DT, cliff.clone()).unwrap(), t_a);
                let floored = values(
                    &EnergySeries::new(DT, cliff)
                        .unwrap()
                        .with_solver_floor(-10.0 * eps, alive(alpha, t_a))
                        .unwrap(),
                    t_a,
                );
                for (((name, w), (_, p)), (_, f)) in want.iter().zip(&plain).zip(&floored) {
                    let Ok(w) = w else { continue };
                    match f {
                        Ok(f) => {
                            accepted += 1;
                            if !within(name, *f, *w) {
                                wrong.push(format!("α {alpha} ε {eps} r {r}: {name} {f} vs {w}"));
                            }
                        }
                        Err(_) => {
                            refused += 1;
                            // A value the series alone would give, wrong: the floor caught it.
                            if let Ok(p) = p
                                && !within(name, *p, *w)
                            {
                                caught += 1;
                            }
                        }
                    }
                }
            }
        }
    }
    println!(
        "{accepted} values accepted with the floor, {refused} refused, of which {caught} would \
         have been wrong without it"
    );
    assert!(wrong.is_empty(), "{wrong:#?}");
    // The grid does hold cases the floor must refuse.
    assert!(caught > 50, "{caught}");
}

#[test]
fn at_upstreams_default_epsilon_the_floor_still_accepts_t30() {
    let (alpha, r) = (0.2, 4.472);
    let (reference, t_a) = histogram(alpha, None, r, 3.0);
    let (cliff, _) = histogram(alpha, Some(5.0), r, 3.0);
    let s = EnergySeries::new(DT, cliff)
        .unwrap()
        .with_solver_floor(-50.0, alive(alpha, t_a))
        .unwrap();
    let r = EnergySeries::new(DT, reference).unwrap();
    for range in [DecayRange::Edt, DecayRange::T20, DecayRange::T30] {
        let got = decay::decay_time(&s, Arrival::at(t_a), range)
            .unwrap_or_else(|e| panic!("{range:?}: {e}"))
            .t_s;
        let want = decay::decay_time(&r, Arrival::at(t_a), range).unwrap().t_s;
        println!("{range:?}: {got:.4} s against {want:.4} s");
        assert!((got / want - 1.0).abs() <= limits::DECAY_RELATIVE);
    }
}

#[test]
fn a_floor_that_is_not_a_number_is_refused() {
    let s = EnergySeries::new(DT, vec![1.0, 0.5, 0.25]).unwrap();
    for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert_eq!(
            s.clone().with_solver_floor(bad, 1.0).unwrap_err().code(),
            codes::BAD_NOISE_INPUT
        );
    }
    for bad in [0.0, -0.5, f64::NAN, f64::INFINITY] {
        assert_eq!(
            s.clone().with_solver_floor(-50.0, bad).unwrap_err().code(),
            codes::BAD_NOISE_INPUT
        );
    }
    // A floor keeps a series' completeness, as a lost share does: nothing arrives after its end
    // but what the floor dropped, and that is bounded as missing energy. A series not known to be
    // complete stays incomplete.
    let c = EnergySeries::complete(DT, vec![1.0, 0.5, 0.25]).unwrap();
    let c = c.with_solver_floor(-50.0, 1.0).unwrap();
    assert!(c.is_complete());
    assert_eq!(c.floor().map(|f| f.db), Some(-50.0));
    assert!(!s.with_solver_floor(-50.0, 1.0).unwrap().is_complete());
}

// A series that ended at the floor: SPPS in energetic mode with every particle dropped, absorbed or
// lost before the steps ran out (`results::spps::SppsResults::band_complete`). The histogram's
// last bins then hold the last few particles above the floor, not a decay, and the tail estimate
// read from them said "not decaying" at random (round 2's G5, `docs/investigations/
// 2026-10-02-t20/RESULT-3B.md`, "Run length in G5"). Nothing arrives after such a series' end but
// what the floor dropped, which the floor bounds.

/// 1 ms bins, as round 2's runs.
const DT_FINE: f64 = 0.001;

/// A decay of `t60` s in 1 ms bins of 10 s, from 1 at bin 0, down to `until_db` and zero after.
fn decay_to(t60: f64, until_db: f64) -> Vec<f64> {
    let per_bin = 10f64.powf(-6.0 * DT_FINE / t60);
    let mut v = vec![0.0; 10_000];
    let mut e: f64 = 1.0;
    for b in v.iter_mut() {
        if 10.0 * e.log10() < until_db {
            break;
        }
        *b = e;
        e *= per_bin;
    }
    v
}

/// [`decay_to`] at 1 s down to −51 dB, then the last particles above the floor: a few bins 66 to
/// 76 dB down, more of them late than early, as in G5's die-off; then nothing for the rest of
/// the 10 s run. `first_window_empty` leaves the first of the tail estimate's two windows empty,
/// as at G5 R000 250 Hz.
fn ended_with_stragglers(first_window_empty: bool) -> Vec<f64> {
    let mut v = decay_to(1.0, -51.0);
    let mut put = |k: usize, db: f64| v[k] = 10f64.powf(db / 10.0);
    put(960, -70.0);
    put(1_020, -72.0);
    if !first_window_empty {
        put(1_200, -76.0);
    }
    put(1_300, -66.0);
    put(1_350, -70.0);
    put(1_399, -68.0);
    v
}

fn t(series: &EnergySeries, range: DecayRange) -> Result<f64, simpa_core::params::ParamError> {
    decay::decay_time(series, Arrival::Detected, range).map(|f| f.t_s)
}

fn why(e: &simpa_core::params::ParamError) -> &NotEvaluable {
    e.not_evaluable()
        .unwrap_or_else(|| panic!("not a not-evaluable refusal: {e}"))
}

#[test]
fn a_decay_that_ended_at_the_floor_is_not_refused_for_its_last_particles() {
    // The same decay without the stragglers, its tail bounded from the series: exact for an
    // exponential, so T20 is answered.
    let clean = EnergySeries::new(DT_FINE, decay_to(1.0, -51.0)).unwrap();
    let want = t(&clean, DecayRange::T20).unwrap();
    assert!((want / 1.0 - 1.0).abs() < limits::DECAY_RELATIVE, "{want}");
    for first_window_empty in [true, false] {
        let v = ended_with_stragglers(first_window_empty);
        // The defect: bounded from its last windows, the stragglers read as not decaying.
        let open = EnergySeries::new(DT_FINE, v.clone())
            .unwrap()
            .with_solver_floor(-50.0, 1.0)
            .unwrap();
        assert_eq!(decay::tail(&open).unwrap(), decay::Tail::Unbounded);
        let e = t(&open, DecayRange::T20).unwrap_err();
        assert!(
            matches!(
                why(&e),
                NotEvaluable::Truncated {
                    with_tail: None,
                    ..
                }
            ),
            "{e}"
        );
        // Ended at the floor: answered, from the same series, so the same value as the
        // refusal carried; and within 1e-4 of the decay without the stragglers.
        let ended = EnergySeries::complete(DT_FINE, v)
            .unwrap()
            .with_solver_floor(-50.0, 1.0)
            .unwrap();
        let got = t(&ended, DecayRange::T20).unwrap_or_else(|e| panic!("{e}"));
        let NotEvaluable::Truncated { value, .. } = why(&e) else {
            unreachable!()
        };
        assert_eq!(got, *value);
        assert!((got / want - 1.0).abs() < 1e-4, "{got} vs {want}");
        // T30 and SPL are not refused for a tail either; the floor may still refuse them.
        for (name, r) in [
            ("T30", t(&ended, DecayRange::T30).map(|_| ())),
            ("SPL", decay::spl_db(&ended).map(|_| ())),
        ] {
            if let Err(e) = r {
                assert!(
                    !matches!(why(&e), NotEvaluable::Truncated { .. }),
                    "{name}: {e}"
                );
            }
        }
    }
}

#[test]
fn a_run_cut_while_still_decaying_is_still_refused() {
    // Cut at −15 dB, particles still alive: not complete, its tail bounded from the series. T20
    // never reaches its range with the tail added; SPL is truncated.
    let cut = EnergySeries::new(DT_FINE, decay_to(1.0, -15.0))
        .unwrap()
        .with_solver_floor(-50.0, 1.0)
        .unwrap();
    let e = t(&cut, DecayRange::T20).unwrap_err();
    let NotEvaluable::RangeNotReached { reached_db, .. } = why(&e) else {
        panic!("{e}")
    };
    assert!((reached_db + 15.0).abs() < 0.1, "{reached_db}");
    let e = decay::spl_db(&cut).unwrap_err();
    assert!(
        matches!(
            why(&e),
            NotEvaluable::Truncated {
                with_tail: Some(_),
                ..
            }
        ),
        "{e}"
    );
    // Cut at −30 dB: T20 reaches its range, and the tail moves it: truncated.
    let cut = EnergySeries::new(DT_FINE, decay_to(1.0, -30.0))
        .unwrap()
        .with_solver_floor(-50.0, 1.0)
        .unwrap();
    let e = t(&cut, DecayRange::T20).unwrap_err();
    assert!(
        matches!(
            why(&e),
            NotEvaluable::Truncated {
                with_tail: Some(_),
                ..
            }
        ),
        "{e}"
    );
    // A series that ended at its floor, every particle dropped, but the floor only 15 dB down:
    // refused, by the floor or, deeper than the last bin, for the range. (Completeness is the caller's claim, and `results` makes it only
    // from SPPS's statistics; a decay that stops at −15 dB with a −50 dB floor and no particle
    // left cannot come from SPPS, and is not this module's to catch.)
    let ended = EnergySeries::complete(DT_FINE, decay_to(1.0, -15.0))
        .unwrap()
        .with_solver_floor(-15.0, 1.0)
        .unwrap();
    for range in [DecayRange::T20, DecayRange::T30] {
        let e = t(&ended, range).unwrap_err();
        assert!(
            matches!(
                why(&e),
                NotEvaluable::MissingNotCleared {
                    floor_db: Some(_),
                    ..
                } | NotEvaluable::RangeNotReached { .. }
            ),
            "{range:?}: {e}"
        );
    }
}

#[test]
fn a_decay_that_stops_falling_before_the_run_ends_is_still_refused() {
    // A coupled room's late plateau: down 20 dB at 1 s, then level to the end of the run, with
    // particles still alive at the end (not complete). The tail is unbounded: refused.
    let mut v = decay_to(1.0, -20.0);
    let last = v.iter().rposition(|&e| e > 0.0).unwrap();
    let level = v[last];
    for b in &mut v[last..] {
        *b = level;
    }
    let plateau = EnergySeries::new(DT_FINE, v)
        .unwrap()
        .with_solver_floor(-50.0, 1.0)
        .unwrap();
    for range in [DecayRange::T20, DecayRange::T30] {
        let e = t(&plateau, range).unwrap_err();
        assert!(
            matches!(
                why(&e),
                NotEvaluable::Truncated {
                    with_tail: None,
                    ..
                }
            ),
            "{range:?}: {e}"
        );
    }
}

#[test]
fn nothing_a_series_ended_at_the_floor_accepts_is_further_than_its_limit_from_the_reference() {
    // The cliff model's histograms end in empty bins once fewer than one particle in 10⁵ is kept:
    // every particle dropped, as a run that ended at its floor. Claimed complete, with the floor.
    let mut wrong = Vec::new();
    let (mut ended, mut accepted, mut caught) = (0, 0, 0);
    for alpha in [0.05, 0.1, 0.2, 0.4, 0.7, 0.9] {
        for r in [2.0, 8.0] {
            let (reference, t_a) = histogram(alpha, None, r, duration(alpha));
            let want = values(&EnergySeries::new(DT, reference).unwrap(), t_a);
            for eps in [1.0, 2.0, 3.0, 3.5, 4.0, 4.5, 5.0, 6.0, 7.0] {
                let (cliff, _) = histogram(alpha, Some(eps), r, duration(alpha));
                if *cliff.last().unwrap() != 0.0 {
                    continue;
                }
                ended += 1;
                let plain = values(&EnergySeries::new(DT, cliff.clone()).unwrap(), t_a);
                let floored = values(
                    &EnergySeries::complete(DT, cliff)
                        .unwrap()
                        .with_solver_floor(-10.0 * eps, alive(alpha, t_a))
                        .unwrap(),
                    t_a,
                );
                for (((name, w), (_, p)), (_, f)) in want.iter().zip(&plain).zip(&floored) {
                    let Ok(w) = w else { continue };
                    match f {
                        Ok(f) => {
                            accepted += 1;
                            if !within(name, *f, *w) {
                                wrong.push(format!("α {alpha} ε {eps} r {r}: {name} {f} vs {w}"));
                            }
                        }
                        Err(_) => {
                            if let Ok(p) = p
                                && !within(name, *p, *w)
                            {
                                caught += 1;
                            }
                        }
                    }
                }
            }
        }
    }
    println!(
        "{ended} cases ended at the floor: {accepted} values accepted, {caught} refused that \
         would have been wrong"
    );
    assert!(ended >= 80, "{ended}");
    assert!(wrong.is_empty(), "{wrong:#?}");
    assert!(caught > 50, "{caught}");
}

// Coupled rooms (B2 assay): the floor's bound takes a dropped particle's future to be an average
// particle's at the arrival, which holds when every particle decays alike. In two coupled rooms it
// does not: late in the run the particles left are those in the slow room, and a particle dropped
// there would still have brought far more than an average one. The model: two rooms, a particle
// reflecting 100 times a second (with probability 0.1 per 1 ms step), losing `m` tenths of a dB at
// each reflection in the room it is in, and passing to the other room at a reflection with
// probability `p`. The source is in room A. Its energy is held on a 0.1 dB grid, so the expected
// histogram, with and without the drop at −50 dB, is computed exactly by stepping the distribution
// of (room, level). A bin in which fewer than one particle in 10⁵ is still kept is empty, as a
// finite run's would be. The reference is the same without the drop, run to −150 dB.

/// A coupled pair of rooms: the loss per reflection in tenths of a dB, the chance of passing to
/// the other room at a reflection, and the room the receiver is in (0 = A, the source's).
#[derive(Clone, Copy, Debug)]
struct Coupled {
    m: [usize; 2],
    p: [f64; 2],
    receiver: usize,
}

/// Reflections per 1 ms step.
const COUPLED_Q: f64 = 0.1;
/// Levels to the floor: 500 tenths of a dB, −50 dB, `trans_epsilon` 5.
const COUPLED_FLOOR: usize = 500;

/// The receiver's expected histogram in 10 ms bins (`DT`), `bins` long: with the drop at −50 dB
/// per particle (and empty once fewer than 10⁻⁵ of the particles are kept), or without. With it,
/// also the share of the emitted energy alive at the end of each bin, SPPS's room table over its
/// sources' power (`results::spps::SppsResults::alive_share`).
fn coupled(c: Coupled, drop: bool, bins: usize) -> (Vec<f64>, Vec<f64>) {
    let sub = (DT / 0.001).round() as usize;
    let level = |k: usize| 10f64.powf(-(k as f64) / 100.0);
    let mut out = vec![0.0; bins];
    if !drop {
        // The expected energy in each room: the sum over levels, exactly.
        let a = [level(c.m[0]), level(c.m[1])];
        let mut e = [1.0, 0.0];
        for bin in out.iter_mut() {
            for _ in 0..sub {
                let stay =
                    |r: usize| (1.0 - COUPLED_Q) * e[r] + COUPLED_Q * (1.0 - c.p[r]) * a[r] * e[r];
                let come = |r: usize| COUPLED_Q * c.p[1 - r] * a[1 - r] * e[1 - r];
                e = [stay(0) + come(0), stay(1) + come(1)];
                *bin += e[c.receiver] * 0.001;
            }
        }
        return (out, Vec::new());
    }
    let energy: Vec<f64> = (0..COUPLED_FLOOR).map(level).collect();
    let mut p = [vec![0.0; COUPLED_FLOOR], vec![0.0; COUPLED_FLOOR]];
    p[0][0] = 1.0;
    let mut alive = vec![0.0; bins];
    for (bin, held) in out.iter_mut().zip(alive.iter_mut()) {
        for _ in 0..sub {
            let mut next = [vec![0.0; COUPLED_FLOOR], vec![0.0; COUPLED_FLOOR]];
            for r in 0..2 {
                for k in 0..COUPLED_FLOOR {
                    let x = p[r][k];
                    if x == 0.0 {
                        continue;
                    }
                    next[r][k] += (1.0 - COUPLED_Q) * x;
                    let k2 = k + c.m[r];
                    // At or below 10^-5 of its start: dropped (`CalculationCore.cpp:305`).
                    if k2 >= COUPLED_FLOOR {
                        continue;
                    }
                    next[r][k2] += COUPLED_Q * (1.0 - c.p[r]) * x;
                    next[1 - r][k2] += COUPLED_Q * c.p[r] * x;
                }
            }
            p = next;
            let at: f64 = p[c.receiver].iter().zip(&energy).map(|(x, e)| x * e).sum();
            *bin += at * 0.001;
        }
        let kept: f64 = p.iter().flatten().sum();
        if kept < 1e-5 {
            *bin = 0.0;
            break;
        }
        *held = p
            .iter()
            .map(|q| q.iter().zip(&energy).map(|(x, e)| x * e).sum::<f64>())
            .sum();
    }
    (out, alive)
}

/// How far below the fast decay's start the slow one starts, dB: the reference's level from
/// −70 to −90 dB fitted with a line and continued back to 0 s.
fn slow_intercept_db(reference: &[f64]) -> f64 {
    let top = reference.iter().copied().fold(0.0, f64::max);
    let pts: Vec<(f64, f64)> = reference
        .iter()
        .enumerate()
        .map(|(k, &e)| (k as f64 * DT, 10.0 * (e / top).log10()))
        .filter(|&(_, l)| (-90.0..=-70.0).contains(&l))
        .collect();
    let n = pts.len() as f64;
    let (mt, ml) = pts
        .iter()
        .fold((0.0, 0.0), |(a, b), &(t, l)| (a + t / n, b + l / n));
    let (sxy, sxx) = pts.iter().fold((0.0, 0.0), |(a, b), &(t, l)| {
        (a + (t - mt) * (l - ml), b + (t - mt) * (t - mt))
    });
    ml - sxy / sxx * mt
}

/// The coupled cases: the fast room A at T60 0.5 s (1.2 dB a reflection), the slow room B 2 to 12
/// times slower, a passage between them from rare to frequent, the receiver in either room.
fn coupled_cases() -> Vec<Coupled> {
    let mut cases = Vec::new();
    for mb in [6, 4, 3, 2, 1] {
        for p in [0.002, 0.005, 0.01, 0.02, 0.03, 0.05, 0.1] {
            for receiver in [0, 1] {
                cases.push(Coupled {
                    m: [12, mb],
                    p: [p, p],
                    receiver,
                });
            }
        }
    }
    cases
}

/// One coupled case's five quantities against the reference's, with the floor dividing by
/// `share`: how far each accepted one is off (`!` past its limit), `[reason]` for a refusal; and
/// how many were accepted past their limit and accepted at all.
fn judge(
    want: &[(&str, Result<f64, String>)],
    cliff: &[f64],
    share: f64,
) -> (String, usize, usize) {
    let got = values(
        &EnergySeries::complete(DT, cliff.to_vec())
            .unwrap()
            .with_solver_floor(-50.0, share)
            .unwrap(),
        0.0,
    );
    let (mut line, mut wrong, mut accepted) = (String::new(), 0, 0);
    for ((name, w), (_, f)) in want.iter().zip(&got) {
        if !matches!(*name, "T20" | "T30" | "SPL" | "C80" | "Ts") {
            continue;
        }
        let Ok(w) = w else { continue };
        match f {
            Ok(f) => {
                accepted += 1;
                line += &match *name {
                    "T20" | "T30" => format!(" {name} {:+.2}%", 100.0 * (f / w - 1.0)),
                    "Ts" => format!(" {name} {:+.4}s", f - w),
                    _ => format!(" {name} {:+.3}dB", f - w),
                };
                if !within(name, *f, *w) {
                    line += "!";
                    wrong += 1;
                }
            }
            Err(e) => line += &format!(" {name} [{}]", e.split(':').nth(2).unwrap_or("?").trim()),
        }
    }
    (line, wrong, accepted)
}

#[test]
fn a_coupled_room_ended_at_the_floor_is_bounded_by_its_slowest_particles() {
    let (mut ended, mut in_band) = (0, 0);
    let (mut wrong_at_arrival, mut wrong, mut accepted) = (0, 0, 0);
    for c in coupled_cases() {
        let (reference, _) = coupled(c, false, 3_000);
        let slow = slow_intercept_db(&reference);
        let want = values(&EnergySeries::complete(DT, reference).unwrap(), 0.0);
        let (cliff, alive) = coupled(c, true, 1_500);
        let end = cliff.iter().rposition(|&e| e > 0.0).unwrap() + 1;
        assert!(end < cliff.len(), "{c:?}: not ended in 15 s");
        ended += 1;
        if c.receiver == 0 && (-20.0..=-5.0).contains(&slow) {
            in_band += 1;
        }
        // The bound as it was: the share alive at the end of the arrival's bin.
        let (line0, w0, _) = judge(&want, &cliff, alive[0]);
        wrong_at_arrival += w0;
        if w0 > 0 {
            println!("{c:?} with the share at the arrival:{line0}");
        }
        // As `results` gives it: the smallest over the decay above the floor.
        let share = params::floor_alive_share(&cliff, &alive, 0, -50.0).unwrap();
        assert!(share <= alive[0]);
        let (line, w, a) = judge(&want, &cliff, share);
        wrong += w;
        accepted += a;
        println!(
            "T60 B/A {:>4.1}, p {:<5}, receiver {}, slow {slow:>6.1} dB, ends {:.2} s, share \
             {:>5.1} dB (arrival {:>5.1}):{line}",
            12.0 / c.m[1] as f64,
            c.p[0],
            ["A", "B"][c.receiver],
            end as f64 * DT,
            10.0 * share.log10(),
            10.0 * alive[0].log10(),
        );
    }
    println!(
        "{ended} coupled cases ended at the floor, {in_band} with the receiver in the source's \
         room and the slow decay starting 5 to 20 dB below: {accepted} values accepted, {wrong} \
         past their limit; with the share at the arrival, {wrong_at_arrival} past their limit"
    );
    assert!(in_band >= 10, "{in_band}");
    // Says no: the bound as it was accepts values past their limit here.
    assert!(wrong_at_arrival >= 10, "{wrong_at_arrival}");
    assert_eq!(wrong, 0);
    // And it is not a blanket refusal.
    assert!(accepted >= 150, "{accepted}");
}

/// Round 2's G5 (specular low hall, energetic, 150,000 particles, 1 ms, 10 s): two receiver-bands
/// refused T20 `truncated`, "not decaying at its end", whose series end 1.97 and 2.08 s into the
/// run with every particle dropped (`tests/fixtures/params/g5_energetic_ended.json`).
#[test]
fn round_2s_g5_ended_at_its_floor_answers_t20_with_the_value_its_refusal_carried() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/params/g5_energetic_ended.json");
    let fixture: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    for row in fixture["rows"].as_array().unwrap() {
        let id = row["id"].as_str().unwrap();
        let f = |k: &str| row[k].as_f64().unwrap();
        let mut bins: Vec<f64> = row["bins"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_f64().unwrap())
            .collect();
        bins.resize(row["steps"].as_u64().unwrap() as usize, 0.0);
        // As `results::report` builds an SPPS band's series.
        let build = |complete: bool| {
            let s = if complete {
                EnergySeries::complete(f("dt"), bins.clone())
            } else {
                EnergySeries::new(f("dt"), bins.clone())
            };
            s.unwrap()
                .with_early_reverberation_unresolved()
                .with_solver_floor(f("floor_db"), f("alive_share"))
                .unwrap()
                .with_lost_share_following_decay(f("lost_share_following_decay"))
                .unwrap()
        };
        let arrival = Arrival::spread(f("arrival_s"), f("half_width_s"));
        let before = decay::evaluate(&build(false), arrival);
        let after = decay::evaluate(&build(true), arrival);
        for (name, b, a, reported) in [
            ("T20", &before.t20, &after.t20, f("reported_t20_s")),
            ("T30", &before.t30, &after.t30, f("reported_t30_s")),
        ] {
            let e = b.as_ref().unwrap_err();
            let NotEvaluable::Truncated {
                value,
                with_tail: None,
                ..
            } = why(e)
            else {
                panic!("{id} {name} before: {e}")
            };
            // The fixture rebuilt here gives the value the run's report carried.
            assert!((value / reported - 1.0).abs() < 1e-12, "{id} {name}");
            match a {
                Ok(fit) => {
                    println!("{id} {name}: {} s (refused before)", fit.t_s);
                    assert_eq!(fit.t_s, *value, "{id} {name}");
                }
                // The floor may refuse T30; a tail may not.
                Err(e) => {
                    println!("{id} {name}: {e}");
                    assert!(name == "T30", "{id} {name}: {e}");
                    assert!(
                        matches!(
                            why(e),
                            NotEvaluable::MissingMoves { .. }
                                | NotEvaluable::MissingNotCleared { .. }
                        ),
                        "{id} {name}: {e}"
                    );
                }
            }
        }
    }
}

#[test]
fn the_poisson_distribution_function_is_right() {
    // P(N < 1) = e^-mu; P(N < 3) at mu = 2 = e^-2 (1 + 2 + 2).
    assert!((poisson_below(1, 0.7) - (-0.7f64).exp()).abs() < 1e-12);
    assert!((poisson_below(3, 2.0) - 5.0 * (-2.0f64).exp()).abs() < 1e-12);
    // Far above and below the mean, and on the continued fraction's side.
    let direct = |n: u32, mu: f64| -> f64 {
        let mut term = (-mu).exp();
        let mut sum = 0.0;
        for k in 0..n {
            sum += term;
            term *= mu / f64::from(k + 1);
        }
        sum
    };
    for (n, mu) in [(10, 3.0), (10, 30.0), (40, 35.0), (5, 0.1), (60, 20.0)] {
        let (a, b) = (poisson_below(n, mu), direct(n, mu));
        assert!(
            (a - b).abs() < 1e-10 * b.max(1e-300) + 1e-14,
            "{n} {mu}: {a} vs {b}"
        );
    }
}
