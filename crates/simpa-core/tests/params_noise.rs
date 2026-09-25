//! Monte-Carlo noise (`params::noise`, `docs/params.md`, "Monte-Carlo noise").
//!
//! **The defect** (M7 review, 2026-09-24). A random-mode series that SPPS's statistics show to be
//! complete has no tail to bound, and nothing else bounded its noise: on tutorial 1 at 150,000
//! particles T30 came out anywhere from 0.8 to 2.4 s where the room's time is 0.67 s, and was
//! reported as a number.
//!
//! **The model the tests are held to**: what SPPS does at a receiver in random mode. An exponential
//! decay with `T = 0.67 s` (tutorial 1's 1 kHz Sabine time) and a direct sound, in 10 ms bins from
//! the arrival at 13 ms; in each bin a Poisson number of crossings, each depositing a chord of a
//! sphere over its mean (`(3/2)·√u`). The generator here is its own (xorshift64* and Knuth's
//! Poisson), not `params::noise`'s.
//!
//! Each test says no:
//! - at 4,000 crossings, as tutorial 1 has at 150,000 particles, the series alone gives T30s more
//!   than 5 % off; every one is refused for its noise;
//! - the estimated standard deviation of each of the eight values is the spread of 60 independent
//!   runs to within a factor 1.4 either way, at 40,000 and 400,000 crossings;
//! - at 400,000 crossings all eight pass; at 4,000, T20 and T30 do not;
//! - a series with no noise model has every value refused.

use std::f64::consts::LN_10;

use simpa_core::faults::{self, Fault};
use simpa_core::params::decay::{self, Arrival};
use simpa_core::params::noise::{self, NoiseModel, limits, standard_deviation};
use simpa_core::params::{EnergySeries, NotEvaluable, ParamError, ParticleCount, codes};

const T: f64 = 0.67;
const DT: f64 = 0.01;
const ARRIVAL: f64 = 0.013;
const DURATION: f64 = 2.0;
/// The direct sound, relative to the reverberant energy.
const DIRECT: f64 = 0.05;

/// Expected crossings per bin for `total` crossings in all.
fn expected(total: f64) -> Vec<f64> {
    let tau = T / (6.0 * LN_10);
    let n = (DURATION / DT).round() as usize;
    let mut rev: Vec<f64> = (0..n)
        .map(|k| {
            let lo = (k as f64 * DT).max(ARRIVAL);
            let hi = ((k + 1) as f64 * DT).max(ARRIVAL);
            tau * ((-(lo - ARRIVAL) / tau).exp() - (-(hi - ARRIVAL) / tau).exp())
        })
        .collect();
    let sum: f64 = rev.iter().sum();
    for x in &mut rev {
        *x *= total / (1.0 + DIRECT) / sum;
    }
    rev[(ARRIVAL / DT) as usize] += total * DIRECT / (1.0 + DIRECT);
    rev
}

/// xorshift64*.
struct Gen(u64);

impl Gen {
    fn uniform(&mut self) -> f64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        (self.0.wrapping_mul(0x2545_f491_4f6c_dd1d) >> 11) as f64 / (1u64 << 53) as f64
    }

    /// Knuth's method, in pieces of mean at most 20 so that `e^-λ` does not underflow.
    fn poisson(&mut self, mut lambda: f64) -> u64 {
        let mut k = 0;
        while lambda > 0.0 {
            let piece = lambda.min(20.0);
            lambda -= piece;
            let limit = (-piece).exp();
            let mut p = self.uniform();
            while p > limit {
                k += 1;
                p *= self.uniform();
            }
        }
        k
    }
}

/// One run: per bin, its crossings' deposits, each a chord over its mean; mean deposit 1.
fn realise(lambda: &[f64], g: &mut Gen) -> Vec<f64> {
    lambda
        .iter()
        .map(|&l| {
            (0..g.poisson(l))
                .map(|_| 1.5 * g.uniform().sqrt())
                .sum::<f64>()
        })
        .collect()
}

fn series(v: Vec<f64>) -> Result<EnergySeries, ParamError> {
    EnergySeries::complete(DT, v)
}

const AT: Arrival = Arrival::at(ARRIVAL);

/// The eight values from the series alone, `None` where refused.
fn plain(v: &[f64]) -> [Option<f64>; 8] {
    let s = series(v.to_vec()).unwrap();
    let p = decay::evaluate(&s, AT);
    [
        p.spl_db.ok(),
        p.edt.ok().map(|f| f.t_s),
        p.t20.ok().map(|f| f.t_s),
        p.t30.ok().map(|f| f.t_s),
        p.c50_db.ok(),
        p.c80_db.ok(),
        p.d50.ok(),
        p.ts_s.ok(),
    ]
}

/// The eight with their noise, in the same order.
fn noisy(v: &[f64]) -> [Result<noise::Estimate, ParamError>; 8] {
    let p = noise::evaluate(
        &series(v.to_vec()),
        AT,
        &NoiseModel::crossings(1.0, noise::Method::Random, None).unwrap(),
    );
    [
        p.spl_db, p.edt_s, p.t20_s, p.t30_s, p.c50_db, p.c80_db, p.d50, p.ts_s,
    ]
}

/// The bootstrap's standard deviations before calibration (`noise::bootstrap`).
fn raw_sd(v: &[f64]) -> [Option<f64>; 8] {
    noise::bootstrap(
        &series(v.to_vec()).unwrap(),
        AT,
        &NoiseModel::crossings(1.0, noise::Method::Random, None).unwrap(),
    )
    .map(|(sd, _)| sd)
}

const NAMES: [&str; 8] = ["SPL", "EDT", "T20", "T30", "C50", "C80", "D50", "Ts"];

/// The estimated standard deviation, whether the value passed or was refused for it.
fn sd_of(r: &Result<noise::Estimate, ParamError>) -> Option<f64> {
    match r {
        Ok(e) => Some(e.sd),
        Err(e) => match e.not_evaluable() {
            Some(NotEvaluable::MonteCarloNoise { sd, .. }) => *sd,
            _ => None,
        },
    }
}

#[test]
fn at_tutorial_ones_particle_count_t30_is_noise_and_is_refused() {
    let lambda = expected(4_000.0);
    let mut g = Gen(0x9e37_79b9_7f4a_7c15);
    let mut off = 0;
    for run in 0..10 {
        let v = realise(&lambda, &mut g);
        // The defect: the series alone gives a T30 more than 5 % off, as a number.
        if let Some(t30) = plain(&v)[3]
            && (t30 / T - 1.0).abs() > 0.05
        {
            off += 1;
        }
        let n = noisy(&v);
        let e = n[3].clone().unwrap_err();
        assert_eq!(e.code(), codes::NOT_EVALUABLE, "run {run}");
        println!("run {run}: {e}");
        // SPL rests on all 4,000 crossings: it passes.
        assert!(n[0].is_ok(), "run {run}: {:?}", n[0]);
    }
    println!("{off} of 10 runs give a T30 more than 5 % off from the series alone");
    assert!(off >= 2, "{off}");
}

#[test]
fn the_estimate_is_the_spread_of_independent_runs() {
    for total in [40_000.0, 400_000.0] {
        let lambda = expected(total);
        let mut g = Gen(0x0123_4567_89ab_cdef ^ total as u64);
        let runs: Vec<[Option<f64>; 8]> =
            (0..60).map(|_| plain(&realise(&lambda, &mut g))).collect();
        // The bootstrap itself, before calibration: the model's own runs are what it models.
        let estimates: Vec<[Option<f64>; 8]> =
            (0..5).map(|_| raw_sd(&realise(&lambda, &mut g))).collect();
        for (i, name) in NAMES.iter().enumerate() {
            let got: Vec<f64> = runs.iter().filter_map(|r| r[i]).collect();
            assert!(got.len() >= 57, "{total} {name}: {} of 60 runs", got.len());
            let spread = standard_deviation(&got).unwrap();
            let est: Vec<f64> = estimates.iter().filter_map(|e| e[i]).collect();
            assert_eq!(est.len(), 5, "{total} {name}");
            let mean = est.iter().sum::<f64>() / 5.0;
            let ratio = mean / spread;
            println!(
                "{total} crossings, {name}: spread of 60 runs {spread:.3e}, estimate {mean:.3e} \
                 (ratio {ratio:.2})"
            );
            assert!(
                (1.0 / 1.4..=1.4).contains(&ratio),
                "{total} {name}: {ratio}"
            );
        }
    }
}

#[test]
fn enough_crossings_pass_every_value_and_too_few_do_not() {
    let mut g = Gen(0x5eed_5eed_5eed_5eed);
    let v = realise(&expected(400_000.0), &mut g);
    for (name, r) in NAMES.iter().zip(noisy(&v)) {
        let e = r.unwrap_or_else(|e| panic!("{name}: {e}"));
        println!("400,000 crossings, {name}: {} ± {}", e.value, e.sd);
    }
    let v = realise(&expected(4_000.0), &mut g);
    let n = noisy(&v);
    for i in [2, 3] {
        match n[i].as_ref().unwrap_err().not_evaluable() {
            Some(NotEvaluable::MonteCarloNoise { sd, limit, .. }) => {
                assert_eq!(*limit, limits::DECAY_RELATIVE);
                println!("4,000 crossings, {}: refused, sd {sd:?}", NAMES[i]);
            }
            other => panic!("{}: {other:?}", NAMES[i]),
        }
    }
}

#[test]
fn the_curvature_carries_its_noise_and_its_refusals_and_flags_a_double_slope() {
    // The curvature `simpa results` reports (M7 follow-ups: it had been computed and dropped):
    // `100·(T30/T20 − 1)` from the reported T20 and T30, its standard deviation over the
    // resamples, refused with T30's refusal (or T20's) when either is.
    let mut g = Gen(0x0c0f_fee0_0000_0001);
    let v = realise(&expected(400_000.0), &mut g);
    let p = noise::evaluate(
        &series(v.clone()),
        AT,
        &NoiseModel::crossings(1.0, noise::Method::Random, None).unwrap(),
    );
    let (t20, t30) = (p.t20_s.unwrap(), p.t30_s.unwrap());
    let c = p.curvature_percent.unwrap();
    assert_eq!(c.value, 100.0 * (t30.value / t20.value - 1.0));
    // A single slope: within its own noise of 0, far from the 10 % flag; its noise is about the
    // two decay times' together.
    println!("single slope: curvature {:.3} ± {:.3} %", c.value, c.sd);
    assert!(c.value.abs() < 4.0 * c.sd && c.value.abs() < decay::CURVATURE_LIMIT_PERCENT);
    let both = 100.0 * (t30.sd / t30.value).hypot(t20.sd / t20.value);
    assert!(c.sd > 0.1 * both && c.sd < 1.5 * both, "{} vs {both}", c.sd);

    // A double slope: the decay after −20 dB twice as slow. T30 reads it, T20 barely: flagged.
    let tau = T / (6.0 * LN_10);
    let knee = 20.0 / 60.0 * T;
    let n = (DURATION / DT).round() as usize;
    let level = |t: f64| {
        let u = (t - ARRIVAL).max(0.0);
        if u < knee {
            (-u / tau).exp()
        } else {
            (-knee / tau).exp() * (-(u - knee) / (2.0 * tau)).exp()
        }
    };
    let dbl: Vec<f64> = (0..n)
        .map(|k| 4.0e7 * (level(k as f64 * DT) - level((k + 1) as f64 * DT)))
        .collect();
    let p = noise::evaluate(
        &series(dbl),
        AT,
        &NoiseModel::crossings(1.0, noise::Method::Random, None).unwrap(),
    );
    let c = p.curvature_percent.unwrap();
    println!("double slope: curvature {:.2} ± {:.2} %", c.value, c.sd);
    assert!(c.value > decay::CURVATURE_LIMIT_PERCENT, "{c:?}");

    // Refused with its decay times: at 4,000 crossings T30 is noise, and so is the curvature.
    let v = realise(&expected(4_000.0), &mut g);
    let p = noise::evaluate(
        &series(v),
        AT,
        &NoiseModel::crossings(1.0, noise::Method::Random, None).unwrap(),
    );
    let e = p.curvature_percent.unwrap_err();
    assert!(
        matches!(
            e,
            ParamError::NotEvaluable {
                quantity: simpa_core::params::Quantity::Curvature,
                why: NotEvaluable::MonteCarloNoise { .. }
            }
        ),
        "{e:?}"
    );
    // And with no model at all.
    let p = noise::evaluate(
        &series(realise(&expected(400_000.0), &mut g)),
        AT,
        &NoiseModel::Unknown { detail: "x".into() },
    );
    assert!(matches!(
        p.curvature_percent.unwrap_err().not_evaluable(),
        Some(NotEvaluable::NoiseUnknown { .. })
    ));
}

#[test]
fn without_a_noise_model_every_value_is_refused() {
    let v = realise(&expected(400_000.0), &mut Gen(77));
    let p = noise::evaluate(
        &series(v),
        AT,
        &NoiseModel::Unknown {
            detail: "a directivity balloon".into(),
        },
    );
    for (name, r) in NAMES.iter().zip([
        p.spl_db, p.edt_s, p.t20_s, p.t30_s, p.c50_db, p.c80_db, p.d50, p.ts_s,
    ]) {
        assert!(
            matches!(
                r.as_ref().unwrap_err().not_evaluable(),
                Some(NotEvaluable::NoiseUnknown { .. })
            ),
            "{name}: {r:?}"
        );
    }
    // A series refused outright carries its own refusal.
    let p = noise::evaluate(
        &EnergySeries::new(DT, vec![0.0; 5]),
        AT,
        &NoiseModel::crossings(1.0, noise::Method::Random, None).unwrap(),
    );
    assert_eq!(p.spl_db.unwrap_err().code(), codes::NO_ENERGY);
    assert!(p.onset.is_none());
}

#[test]
fn every_standard_deviation_is_the_bootstrap_times_its_methods_calibration_factor() {
    // What `evaluate` reports and judges is the bootstrap's standard deviation times the factor of
    // the run's computation method (`noise::calibration`), quantity by quantity.
    let v = realise(&expected(400_000.0), &mut Gen(0x00ca_11b7_0000_0001));
    let s = series(v).unwrap();
    for method in [noise::Method::Random, noise::Method::Energetic] {
        let model = NoiseModel::crossings(1.0, method, None).unwrap();
        let raw = noise::bootstrap(&s, AT, &model);
        let p = noise::evaluate(&Ok(s.clone()), AT, &model);
        let got = [
            &p.spl_db, &p.edt_s, &p.t20_s, &p.t30_s, &p.c50_db, &p.c80_db, &p.d50, &p.ts_s,
        ];
        for i in 0..8 {
            let sd = sd_of(got[i]).unwrap_or_else(|| panic!("{method:?} {}", NAMES[i]));
            let want = noise::calibration::factor(method, i) * raw[i].0.unwrap();
            assert!(
                (sd - want).abs() <= 1e-12 * want,
                "{method:?} {}: {sd} vs {want}",
                NAMES[i]
            );
        }
        // Says no: through the code, a factor scaled twice over scales every reported value's
        // standard deviation, so the factor is what `evaluate` applies.
        let doubled = faults::with(Fault::NoiseCalibrationScaled { by: 2.0 }, || {
            noise::evaluate(&Ok(s.clone()), AT, &model)
        });
        let d = [
            &doubled.spl_db,
            &doubled.edt_s,
            &doubled.t20_s,
            &doubled.t30_s,
            &doubled.c50_db,
            &doubled.c80_db,
            &doubled.d50,
            &doubled.ts_s,
        ];
        for i in 0..8 {
            let (a, b) = (sd_of(got[i]).unwrap(), sd_of(d[i]).unwrap());
            assert!((b - 2.0 * a).abs() <= 1e-12 * b, "{method:?} {}", NAMES[i]);
        }
    }
}

/// The particle count a refusal for noise names, its factor over the run's count and its margin.
fn named(r: &Result<noise::Estimate, ParamError>) -> (f64, u64, f64) {
    match r.as_ref().unwrap_err().not_evaluable() {
        Some(NotEvaluable::MonteCarloNoise {
            particle_count:
                ParticleCount::Named {
                    factor,
                    margin,
                    particles: Some(n),
                },
            ..
        }) => (*factor, *n, *margin),
        other => panic!("{other:?}"),
    }
}

/// Crossings behind a series of [`BASE`] particles in the particle-count test.
const BASE_CROSSINGS: f64 = 10_000.0;
const BASE: f64 = 150_000.0;

/// A run of `particles` particles where [`BASE`] particles give [`BASE_CROSSINGS`]: its crossings
/// scale with the count, each depositing proportionally less, so the room's energy stays the same.
fn at_count(particles: f64, g: &mut Gen) -> (Vec<f64>, NoiseModel) {
    let scale = particles / BASE;
    let v = realise(&expected(BASE_CROSSINGS * scale), g)
        .into_iter()
        .map(|x| x / scale)
        .collect();
    let model = NoiseModel::crossings(
        1.0 / scale,
        noise::Method::Random,
        Some(particles.round() as u32),
    )
    .unwrap();
    (v, model)
}

#[test]
fn the_particle_count_a_refusal_names_brings_the_value_within_its_limit() {
    // 10,000 crossings: T20 is refused for its noise, and the refusal names the count at which
    // the calibrated standard deviation, falling as 1/√N, is its margin below the limit
    // (`noise::calibration::margin`: random-mode T20's one-run estimate scatters by up to 30 %,
    // so its margin is 1.5). Ten new runs at that count pass; at a sixth of it, where the
    // calibrated standard deviation is √6/1.5 = 1.63 times the limit, most are refused again. (A
    // quarter, 1.33 times the limit with round 3's margin, lets a run whose own estimate reads
    // low through about 4 times in 10.)
    let mut g = Gen(0x0c0f_fee0_0000_0077);
    let (v, model) = at_count(BASE, &mut g);
    let p = noise::evaluate(&series(v), AT, &model);
    let (factor, needed, margin) = named(&p.t20_s);
    assert_eq!(margin, noise::calibration::margin(noise::Method::Random, 2));
    let Some(NotEvaluable::MonteCarloNoise {
        sd: Some(sd),
        value,
        limit,
        ..
    }) = p.t20_s.as_ref().unwrap_err().not_evaluable()
    else {
        panic!()
    };
    let want = (margin * sd / value / limit).powi(2);
    assert!((factor - want).abs() <= 1e-12 * want, "{factor} vs {want}");
    assert_eq!(needed, noise::round_up_two_digits(factor * BASE));
    assert!(needed as f64 >= factor * BASE);
    println!(
        "T20 {value:.3} s ± {:.2} %: run at least {needed} particles",
        100.0 * sd / value
    );
    let (mut pass_at, mut pass_sixth) = (0, 0);
    for _ in 0..10 {
        let (v, m) = at_count(needed as f64, &mut g);
        let r = noise::evaluate(&series(v), AT, &m).t20_s;
        match &r {
            Ok(e) => println!(
                "  at the count: {:.3} ± {:.2} %",
                e.value,
                100.0 * e.sd / e.value
            ),
            Err(e) => println!("  at the count: {e}"),
        }
        pass_at += usize::from(r.is_ok());
        let (v, m) = at_count(needed as f64 / 6.0, &mut g);
        pass_sixth += usize::from(noise::evaluate(&series(v), AT, &m).t20_s.is_ok());
    }
    println!("at the named count {pass_at} of 10 pass; at a sixth of it {pass_sixth}");
    assert!(pass_at >= 9, "{pass_at}");
    // Says no: a sixth of the named count is not enough.
    assert!(pass_sixth <= 3, "{pass_sixth}");
    // Random-mode T30's spread did not fall slower than 1/√N on any pair of SPPS cells (round 3's
    // one-sided rule), so its refusal names a count too, with its own margin: from its standard
    // deviation, or, when more than 10 of its resamples refuse it (here), from the first multiple
    // of the particles at which they would not (R4-3); a quantity whose flag is off names none
    // (`params::noise`'s unit tests).
    let margin = match p.t30_s.as_ref().unwrap_err().not_evaluable() {
        Some(NotEvaluable::MonteCarloNoise {
            particle_count:
                ParticleCount::Named { margin, .. } | ParticleCount::Resampled { margin, .. },
            ..
        }) => *margin,
        other => panic!("{other:?}"),
    };
    assert_eq!(margin, noise::calibration::margin(noise::Method::Random, 3));
    assert!(noise::calibration::root_n_confirmed(
        noise::Method::Random,
        3
    ));
    // A value its resamples refuse names the multiple at which they would not (R4-3, `params::
    // noise`'s unit tests), or says that none tried clears them.
    let e = ParamError::NotEvaluable {
        quantity: simpa_core::params::Quantity::T30,
        why: NotEvaluable::MonteCarloNoise {
            value: 1.0,
            sd: Some(0.01),
            limit: 0.025,
            resamples: 200,
            refused_resamples: 50,
            particle_count: ParticleCount::BeyondResampled { multiple: 64 },
        },
    };
    assert!(e.to_string().contains("No particle count is named"), "{e}");
}

/// Each quantity's spread over `runs` independent runs of `total` crossings (relative for the
/// decay times), with how many runs gave it, and the bootstrap's mean estimate over `estimates`
/// more runs (relative likewise).
fn spread_at(total: f64, runs: usize, estimates: usize, g: &mut Gen) -> [(f64, usize, f64); 8] {
    let lambda = expected(total);
    let got: Vec<[Option<f64>; 8]> = (0..runs).map(|_| plain(&realise(&lambda, g))).collect();
    let est: Vec<[Option<f64>; 8]> = (0..estimates)
        .map(|_| raw_sd(&realise(&lambda, g)))
        .collect();
    std::array::from_fn(|i| {
        let v: Vec<f64> = got.iter().filter_map(|r| r[i]).collect();
        let e: Vec<f64> = est.iter().filter_map(|r| r[i]).collect();
        let m = v.iter().sum::<f64>() / v.len().max(1) as f64;
        let rel = if (1..=3).contains(&i) { m } else { 1.0 };
        let sd = standard_deviation(&v).unwrap_or(f64::NAN) / rel;
        let em = e.iter().sum::<f64>() / e.len().max(1) as f64 / rel;
        if std::env::var_os("PROBE_SCATTER").is_some() && e.len() > 2 {
            let mut sorted = e.clone();
            sorted.sort_by(f64::total_cmp);
            println!(
                "  {total} {}: estimate scatter {:.3} relative; 10th percentile {:.3} of the mean",
                NAMES[i],
                standard_deviation(&e).unwrap() / (em * rel),
                sorted[sorted.len() / 10] / (em * rel)
            );
        }
        (sd, v.len(), em)
    })
}

#[test]
fn the_spread_falls_as_one_over_the_root_of_the_crossings() {
    // The particle count a refusal names rests on this: the spread of every value over independent
    // runs of the model falls as 1/√N. Measured over 400 runs at 10,000, 40,000 and 160,000
    // crossings: the spread times √N agrees between every two counts within three of their joint
    // standard errors (each spread's relative standard error is 1/√(2·399)), for every quantity.
    let mut g = Gen(0x0000_5ca1_e000_0001);
    let totals = [10_000.0, 40_000.0, 160_000.0];
    let at: Vec<[(f64, usize, f64); 8]> = totals
        .iter()
        .map(|t| spread_at(*t, 400, 0, &mut g))
        .collect();
    let check = |labels: &[f64]| -> Vec<String> {
        let mut off = Vec::new();
        for (i, name) in NAMES.iter().enumerate() {
            let scaled: Vec<(f64, f64)> = at
                .iter()
                .zip(labels)
                .map(|(a, t)| {
                    let v = a[i].0 * t.sqrt();
                    (v, v / (2.0 * (a[i].1 as f64 - 1.0)).sqrt())
                })
                .collect();
            let mut worst = 0.0f64;
            for a in 0..scaled.len() {
                for b in a + 1..scaled.len() {
                    let z = (scaled[a].0 - scaled[b].0).abs() / scaled[a].1.hypot(scaled[b].1);
                    worst = worst.max(z);
                }
            }
            println!("{name}: spread x sqrt(N) {scaled:.4?}, worst pair {worst:.1} se");
            if worst > 3.0 {
                off.push(name.to_string());
            }
        }
        off
    };
    assert_eq!(check(&totals), Vec::<String>::new());
    // Says no through the input: the runs of 160,000 crossings read as 80,000.
    let off = check(&[10_000.0, 40_000.0, 80_000.0]);
    assert_eq!(off.len(), 8, "{off:?}");
}

#[test]
#[ignore = "information for docs/params.md: the bootstrap's estimate against the true spread at low             crossing counts; run on purpose"]
fn the_bootstrap_against_the_true_spread_at_low_counts() {
    let mut g = Gen(0x0123_4567_89ab_cdef);
    for total in [4_000.0, 10_000.0, 20_000.0, 40_000.0, 100_000.0, 400_000.0] {
        let a = spread_at(total, 200, 100, &mut g);
        let line: Vec<String> = NAMES
            .iter()
            .zip(a)
            .map(|(n, (sd, k, e))| format!("{n} true {sd:.4} ({k}/200) est {e:.4} ({:.2})", e / sd))
            .collect();
        println!("{total:>8} crossings: {}", line.join("; "));
    }
}
