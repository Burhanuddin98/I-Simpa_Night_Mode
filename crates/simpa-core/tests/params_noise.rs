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

use simpa_core::params::decay::{self, Arrival};
use simpa_core::params::noise::{self, NoiseModel, limits, standard_deviation};
use simpa_core::params::{EnergySeries, NotEvaluable, ParamError, codes};

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

const AT: Arrival = Arrival::Known { time_s: ARRIVAL };

/// The eight values from the series alone, `None` where refused.
fn plain(v: &[f64]) -> [Option<f64>; 8] {
    let s = series(v.to_vec()).unwrap();
    let p = decay::evaluate(&s, AT).unwrap();
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
        &NoiseModel::crossings(1.0).unwrap(),
    );
    [
        p.spl_db, p.edt_s, p.t20_s, p.t30_s, p.c50_db, p.c80_db, p.d50, p.ts_s,
    ]
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
        let estimates: Vec<[Option<f64>; 8]> = (0..5)
            .map(|_| noisy(&realise(&lambda, &mut g)).each_ref().map(sd_of))
            .collect();
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
        &NoiseModel::crossings(1.0).unwrap(),
    );
    assert_eq!(p.spl_db.unwrap_err().code(), codes::NO_ENERGY);
    assert!(p.onset.is_none());
}
