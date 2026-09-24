//! The direct sound as SPPS records it, and a given arrival outside the onset bin (M7 follow-up,
//! `docs/params.md`, "Direct-arrival detection" and "The direct sound's spread").
//!
//! **What SPPS records.** A particle crossing a receiver ball of radius `R` adds its energy times
//! the length of its path inside the ball, to the time step in which it travelled that path
//! (`spps/input_output/reportmanager.cpp:198-225`). The direct sound of a source `r` away therefore
//! reaches the receiver over `[(r − R)/c, (r + R)/c]`, with the density of a plane front sweeping a
//! ball, `R² − (ct − r)²`: 1.8 ms at upstream's `R` = 0.31 m. The series here are that direct
//! sound plus an exponential decay from `r/c`, as bin integrals.
//!
//! **What went wrong.** The first model took the direct sound as an impulse at `r/c` inside one
//! bin. When the spread straddles a bin edge, the next bin holds part of the direct sound, which
//! the model read as decay: EDT came out up to 26 % short and Ts up to 21 % off, as numbers
//! (`the_direct_sound_taken_as_an_impulse_misses_where_it_straddles_a_bin_edge`). And when `r/c`
//! fell in the bin after the one the leading edge of the ball reached, the onset bin (the cap),
//! every onset-relative quantity was refused, T30 included, although decay times do not depend
//! on where time starts.
//!
//! **Now.** Given the spread, the curve is the histogram's own from the first bin wholly after the
//! direct sound; before it, the decay is continued back to the arrival and the rest is the direct
//! sound, at the arrival. A given arrival outside the onset bin refuses C50, C80, D50 and Ts only
//! (`params_bad_arrival`); the decay times come from the arrival when the onset bin holds the
//! leading edge of its spread, and otherwise as if no arrival were given.
//!
//! Every check says no:
//! - the spread given as 0 (an impulse), on the same series: EDT and Ts miss their bounds;
//! - the arrival outside the onset bin: C80 refused; inside it, the same geometry gives C80;
//! - the decay 1 % off: T30 misses gate (a)'s bound;
//! - the decay times taken as if no arrival were given, which the first fix proposed: EDT misses
//!   its bound where the onset bin holds only the cap.

use std::f64::consts::LN_10;

use simpa_core::params::decay::{Arrival, BandParameters, evaluate, onset};
use simpa_core::params::{EnergySeries, NotEvaluable, codes};

const K60: f64 = 6.0 * LN_10;
/// SPPS's speed of sound at 20 °C.
const C: f64 = 343.2;
/// Upstream's receiver radius.
const RADIUS: f64 = 0.31;
const TS: [f64; 3] = [0.3, 1.0, 3.0];
const DTS: [f64; 2] = [0.01, 0.001];
/// Direct over reverberant energy: a receiver well beyond, near and inside the critical distance.
const DIRECTS: [f64; 3] = [0.12, 1.0, 3.0];
/// Arrivals per bin, spread evenly through it.
const OFFSETS: usize = 40;

/// Direct sound of energy `direct` spread over `[(r − R)/c, (r + R)/c]` with the density of a plane
/// front sweeping a ball, then an exponential of reverberation time `t` and total energy 1 from
/// `r/c`, as bin integrals down to 150 dB.
fn spps_like(t: f64, dt: f64, r: f64, direct: f64) -> EnergySeries {
    let tau = t / K60;
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
            let d = direct * (prim(b) - prim(a)) / whole;
            let e = if b <= t_a {
                0.0
            } else {
                (-(a.max(t_a) - t_a) / tau).exp() - (-(b - t_a) / tau).exp()
            };
            d + e
        })
        .collect();
    EnergySeries::new(dt, v).unwrap()
}

/// The source–receiver distance putting `r/c` a fraction `f` into the bin that holds 29 ms.
fn distance(dt: f64, f: f64) -> f64 {
    ((0.029 / dt).floor() + f) * dt * C
}

/// The closed forms of a direct sound plus an exponential, from the arrival: C50, C80, D50, Ts.
fn closed(t: f64, direct: f64) -> [f64; 4] {
    let tau = t / K60;
    let late = |te: f64| (-te / tau).exp();
    let early = |te: f64| direct + 1.0 - late(te);
    [
        10.0 * (early(0.05) / late(0.05)).log10(),
        10.0 * (early(0.08) / late(0.08)).log10(),
        early(0.05) / (direct + 1.0),
        tau / (direct + 1.0),
    ]
}

/// Each quantity's deviation from the truth in gate (a)'s unit, or `None` when refused: EDT, T20,
/// T30 and Ts relative, C50 and C80 in dB, D50 in percentage points.
fn deviations(p: &BandParameters, t: f64, direct: f64) -> [Option<f64>; 7] {
    let c = closed(t, direct);
    let d = |r: &Result<simpa_core::params::decay::DecayFit, _>| {
        r.as_ref().ok().map(|f| f.t_s / t - 1.0)
    };
    [
        d(&p.edt),
        d(&p.t20),
        d(&p.t30),
        p.c50_db.as_ref().ok().map(|x| x - c[0]),
        p.c80_db.as_ref().ok().map(|x| x - c[1]),
        p.d50.as_ref().ok().map(|x| 100.0 * (x - c[2])),
        p.ts_s.as_ref().ok().map(|x| x / c[3] - 1.0),
    ]
}

/// Gate (a)'s bounds in the same order: 0.5 % for the times, 0.01 dB for C, 0.1 points for D50.
const BOUNDS: [f64; 7] = [0.005, 0.005, 0.005, 0.01, 0.01, 0.1, 0.005];

/// Whether `r/c` lies after the onset bin of `s`.
fn after_onset_bin(s: &EnergySeries, t_a: f64) -> bool {
    t_a >= onset(s).bin_end_s
}

/// Whether EDT was refused because the direct sound's step leaves less than 2 bins of its 10 dB
/// range: at `D/R` = 3 the step is 6 dB, and at `T` = 0.3 s the 4 dB left last 20 ms. Any other
/// refusal of EDT fails.
fn edt_or_its_short_range(p: &BandParameters, dev: Option<f64>) -> bool {
    if dev.is_some() {
        return false;
    }
    match p.edt.as_ref().unwrap_err().not_evaluable() {
        Some(NotEvaluable::RangeTooShort { .. }) => true,
        other => panic!("EDT refused otherwise: {other:?}"),
    }
}

#[test]
fn a_spread_direct_sound_meets_every_bound_from_its_arrival_and_spread() {
    let mut after = 0;
    let mut short_edt = 0;
    let mut worst = [0.0f64; 7];
    for dt in DTS {
        for t in TS {
            for direct in DIRECTS {
                for i in 0..OFFSETS {
                    let f = (i as f64 + 0.5) / OFFSETS as f64;
                    let r = distance(dt, f);
                    let s = spps_like(t, dt, r, direct);
                    let t_a = r / C;
                    let p = evaluate(&s, Arrival::spread(t_a, RADIUS / C));
                    let dev = deviations(&p, t, direct);
                    let case = format!("dt {dt}, T {t}, D/R {direct}, offset {f}");
                    // T20 and T30 always come out, within gate (a)'s bound; EDT too, unless the
                    // direct sound's step leaves less than 2 bins of its range.
                    if edt_or_its_short_range(&p, dev[0]) {
                        short_edt += 1;
                    }
                    for j in 0..3 {
                        let Some(d) = dev[j] else { continue };
                        assert!(d.abs() <= BOUNDS[j], "{case}: quantity {j} off by {d}");
                        worst[j] = worst[j].max(d.abs());
                    }
                    assert!(dev[1].is_some() && dev[2].is_some(), "{case}: {p:?}");
                    if after_onset_bin(&s, t_a) {
                        // Only the cap of the ball reached the onset bin: C, D and Ts refused.
                        after += 1;
                        for r in [&p.c50_db, &p.c80_db, &p.d50, &p.ts_s] {
                            assert_eq!(r.as_ref().unwrap_err().code(), codes::BAD_ARRIVAL);
                        }
                        continue;
                    }
                    for j in 3..7 {
                        let d = dev[j].unwrap_or_else(|| panic!("{case}: {j} refused {p:?}"));
                        assert!(d.abs() <= BOUNDS[j], "{case}: quantity {j} off by {d}");
                        worst[j] = worst[j].max(d.abs());
                    }
                }
            }
        }
    }
    println!(
        "{} cases, {after} with r/c after the onset bin, {short_edt} EDTs refused for a range \
         under 2 bins; worst: EDT {:.1e}, T20 {:.1e}, T30 {:.1e}, C50 {:.1e} dB, C80 {:.1e} dB, \
         D50 {:.1e} points, Ts {:.1e}",
        DTS.len() * TS.len() * DIRECTS.len() * OFFSETS,
        worst[0],
        worst[1],
        worst[2],
        worst[3],
        worst[4],
        worst[5],
        worst[6]
    );
    assert!(after > 0, "no case put r/c after the onset bin");
}

#[test]
fn the_direct_sound_taken_as_an_impulse_misses_where_it_straddles_a_bin_edge() {
    // The same series with the spread given as 0: where the direct sound straddles a bin edge,
    // the next bin holds part of it, which the curve reads as decay.
    let mut worst = [0.0f64; 7];
    for dt in DTS {
        for t in TS {
            for direct in DIRECTS {
                for i in 0..OFFSETS {
                    let f = (i as f64 + 0.5) / OFFSETS as f64;
                    let r = distance(dt, f);
                    let s = spps_like(t, dt, r, direct);
                    let p = evaluate(&s, Arrival::at(r / C));
                    for (w, d) in worst.iter_mut().zip(deviations(&p, t, direct)) {
                        if let Some(d) = d {
                            *w = w.max(d.abs());
                        }
                    }
                }
            }
        }
    }
    println!(
        "taken as an impulse, worst: EDT {:.2} %, T20 {:.2} %, T30 {:.2} %, C50 {:.3} dB, C80 \
         {:.3} dB, D50 {:.3} points, Ts {:.2} %",
        100.0 * worst[0],
        100.0 * worst[1],
        100.0 * worst[2],
        worst[3],
        worst[4],
        worst[5],
        100.0 * worst[6]
    );
    for j in [0, 6] {
        assert!(worst[j] > BOUNDS[j], "quantity {j}: {worst:?}");
    }
}

/// Upstream's defaults, `dt` = 10 ms and `R` = 0.31 m, with `r/c` 0.03 of the way into its bin: the
/// leading 0.9 ms of the ball's crossing falls in the bin before, and at these energies it holds
/// more than 1 % of the largest bin, so it is the onset bin.
fn cap_in_the_bin_before(t: f64, direct: f64) -> (EnergySeries, f64) {
    let r = distance(0.01, 0.03);
    let s = spps_like(t, 0.01, r, direct);
    assert!(
        after_onset_bin(&s, r / C),
        "T {t}, D/R {direct}: not the cap case"
    );
    (s, r / C)
}

#[test]
fn an_arrival_after_the_onset_bin_gives_t30_and_refuses_c80() {
    for t in TS {
        for direct in DIRECTS {
            let (s, t_a) = cap_in_the_bin_before(t, direct);
            let p = evaluate(&s, Arrival::spread(t_a, RADIUS / C));
            let dev = deviations(&p, t, direct);
            println!(
                "T {t}, D/R {direct}: T30 {:+.1e}, EDT {:?}; C80 {:?}",
                dev[2].unwrap(),
                dev[0],
                p.c80_db.as_ref().map_err(|e| e.code())
            );
            edt_or_its_short_range(&p, dev[0]);
            assert!(dev[2].unwrap().abs() <= 0.005, "T {t}, D/R {direct}: {p:?}");
            assert_eq!(p.c80_db.unwrap_err().code(), codes::BAD_ARRIVAL);
            assert_eq!(p.decay_arrival, Arrival::spread(t_a, RADIUS / C));

            // Says no: 1 % off in T, T30 misses the bound.
            let (off, _) = cap_in_the_bin_before(1.01 * t, direct);
            let q = evaluate(&off, Arrival::spread(t_a, RADIUS / C));
            assert!(deviations(&q, t, direct)[2].unwrap().abs() > 0.005);

            // Says no: the arrival 0.3 of the way into its bin, where the whole ball's crossing
            // lies in one bin, gives C80 within its bound.
            let r = distance(0.01, 0.3);
            let inside = spps_like(t, 0.01, r, direct);
            let q = evaluate(&inside, Arrival::spread(r / C, RADIUS / C));
            assert!(deviations(&q, t, direct)[4].unwrap().abs() <= 0.01, "{q:?}");
        }
    }
}

#[test]
fn decay_times_taken_as_if_no_arrival_were_given_miss_where_only_the_cap_is_in_the_onset_bin() {
    // The first proposal for the misfit: evaluate the decay times as with the arrival detected,
    // from the two ends of the onset bin. The onset bin then holds only the ball's cap, and the
    // direct sound lies smeared across the next bin: EDT is biased, and not always refused.
    let mut accepted_outside = 0;
    let mut worst = 0.0f64;
    for dt in DTS {
        for t in TS {
            for direct in [0.12, 0.5, 1.0] {
                let r = distance(dt, 0.01);
                let s = spps_like(t, dt, r, direct);
                if !after_onset_bin(&s, r / C) {
                    continue;
                }
                let p = evaluate(&s, Arrival::Detected);
                if let Ok(f) = &p.edt {
                    let d = (f.t_s / t - 1.0).abs();
                    worst = worst.max(d);
                    if d > 0.005 {
                        accepted_outside += 1;
                    }
                }
                // From the arrival and its spread the same series is within the bound.
                let q = evaluate(&s, Arrival::spread(r / C, RADIUS / C));
                assert!((q.edt.unwrap().t_s / t - 1.0).abs() <= 0.005);
            }
        }
    }
    println!(
        "as if detected: {accepted_outside} EDTs accepted outside 0.5 %, the worst {:.2} % off",
        100.0 * worst
    );
    assert!(accepted_outside > 0);
}
