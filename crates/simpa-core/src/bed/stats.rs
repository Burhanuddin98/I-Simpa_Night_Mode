//! The statistics M8a judges with (`docs/investigations/2026-09-29-m8a/SPEC.md`, section 5): the
//! mean and sample standard deviation of per-seed values, Student's t and the χ² quantiles.
//!
//! The quantiles are computed, not tabulated: the regularized incomplete beta and gamma functions
//! (continued fractions, W. H. Press et al., *Numerical Recipes*, 3rd ed., sections 6.2 and 6.4)
//! inverted by bisection. The tests hold them to the published values the spec quotes
//! (`t₀.₉₇₅,₉` = 2.262, `t₀.₉₇₅,₁₉` = 2.093).

/// The mean of `x`; NaN for no values.
pub fn mean(x: &[f64]) -> f64 {
    x.iter().sum::<f64>() / x.len() as f64
}

/// The sample standard deviation of `x` (`n − 1` in the denominator); NaN for fewer than two.
pub fn sd(x: &[f64]) -> f64 {
    if x.len() < 2 {
        return f64::NAN;
    }
    let m = mean(x);
    (x.iter().map(|v| (v - m).powi(2)).sum::<f64>() / (x.len() - 1) as f64).sqrt()
}

/// `(max − min) / mean` of `x`.
pub fn relative_range(x: &[f64]) -> f64 {
    let hi = x.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let lo = x.iter().copied().fold(f64::INFINITY, f64::min);
    (hi - lo) / mean(x)
}

/// `ln Γ(x)` for `x > 0` (Lanczos, g = 7, nine coefficients; about 15 digits).
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
    if x < 0.5 {
        // Reflection.
        let pi = std::f64::consts::PI;
        return (pi / (pi * x).sin()).ln() - ln_gamma(1.0 - x);
    }
    let x = x - 1.0;
    let mut a = G[0];
    let t = x + 7.5;
    for (i, g) in G.iter().enumerate().skip(1) {
        a += g / (x + i as f64);
    }
    0.5 * (2.0 * std::f64::consts::PI).ln() + (x + 0.5) * t.ln() - t + a.ln()
}

/// The continued fraction of the incomplete beta function (modified Lentz).
fn beta_cf(a: f64, b: f64, x: f64) -> f64 {
    const TINY: f64 = 1e-300;
    let (qab, qap, qam) = (a + b, a + 1.0, a - 1.0);
    let mut c = 1.0;
    let mut d = 1.0 - qab * x / qap;
    if d.abs() < TINY {
        d = TINY;
    }
    d = 1.0 / d;
    let mut h = d;
    for m in 1..=400 {
        let m = f64::from(m);
        let m2 = 2.0 * m;
        let aa = m * (b - m) * x / ((qam + m2) * (a + m2));
        d = 1.0 + aa * d;
        if d.abs() < TINY {
            d = TINY;
        }
        c = 1.0 + aa / c;
        if c.abs() < TINY {
            c = TINY;
        }
        d = 1.0 / d;
        h *= d * c;
        let aa = -(a + m) * (qab + m) * x / ((a + m2) * (qap + m2));
        d = 1.0 + aa * d;
        if d.abs() < TINY {
            d = TINY;
        }
        c = 1.0 + aa / c;
        if c.abs() < TINY {
            c = TINY;
        }
        d = 1.0 / d;
        let del = d * c;
        h *= del;
        if (del - 1.0).abs() < 1e-15 {
            break;
        }
    }
    h
}

/// The regularized incomplete beta function `I_x(a, b)`.
fn inc_beta(a: f64, b: f64, x: f64) -> f64 {
    if x <= 0.0 {
        return 0.0;
    }
    if x >= 1.0 {
        return 1.0;
    }
    let front =
        (ln_gamma(a + b) - ln_gamma(a) - ln_gamma(b) + a * x.ln() + b * (1.0 - x).ln()).exp();
    if x < (a + 1.0) / (a + b + 2.0) {
        front * beta_cf(a, b, x) / a
    } else {
        1.0 - front * beta_cf(b, a, 1.0 - x) / b
    }
}

/// The regularized lower incomplete gamma function `P(a, x)`.
fn inc_gamma_p(a: f64, x: f64) -> f64 {
    if x <= 0.0 {
        return 0.0;
    }
    if x < a + 1.0 {
        // Series.
        let (mut ap, mut sum) = (a, 1.0 / a);
        let mut del = sum;
        for _ in 0..1000 {
            ap += 1.0;
            del *= x / ap;
            sum += del;
            if del.abs() < sum.abs() * 1e-16 {
                break;
            }
        }
        sum * (-x + a * x.ln() - ln_gamma(a)).exp()
    } else {
        // Continued fraction for Q, modified Lentz.
        const TINY: f64 = 1e-300;
        let mut b = x + 1.0 - a;
        let mut c = 1.0 / TINY;
        let mut d = 1.0 / b;
        let mut h = d;
        for i in 1..1000 {
            let an = -f64::from(i) * (f64::from(i) - a);
            b += 2.0;
            d = an * d + b;
            if d.abs() < TINY {
                d = TINY;
            }
            c = b + an / c;
            if c.abs() < TINY {
                c = TINY;
            }
            d = 1.0 / d;
            let del = d * c;
            h *= del;
            if (del - 1.0).abs() < 1e-16 {
                break;
            }
        }
        1.0 - (-x + a * x.ln() - ln_gamma(a)).exp() * h
    }
}

/// Student's t distribution function at `t` with `dof` degrees of freedom.
pub fn t_cdf(t: f64, dof: f64) -> f64 {
    let tail = 0.5 * inc_beta(0.5 * dof, 0.5, dof / (dof + t * t));
    if t >= 0.0 { 1.0 - tail } else { tail }
}

/// The χ² distribution function at `x` with `dof` degrees of freedom.
pub fn chi2_cdf(x: f64, dof: f64) -> f64 {
    inc_gamma_p(0.5 * dof, 0.5 * x)
}

/// The `x` in `[lo, hi]` where the increasing `f` reaches `p`, by bisection to the last bit.
fn invert(f: impl Fn(f64) -> f64, p: f64, mut lo: f64, mut hi: f64) -> f64 {
    for _ in 0..200 {
        let mid = 0.5 * (lo + hi);
        if mid <= lo || mid >= hi {
            break;
        }
        if f(mid) < p {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    0.5 * (lo + hi)
}

/// The `p`-quantile of Student's t with `dof` degrees of freedom, `0 < p < 1`.
pub fn t_quantile(p: f64, dof: f64) -> f64 {
    if p == 0.5 {
        return 0.0;
    }
    if p < 0.5 {
        return -t_quantile(1.0 - p, dof);
    }
    let mut hi = 1.0;
    while t_cdf(hi, dof) < p && hi < 1e12 {
        hi *= 2.0;
    }
    invert(|t| t_cdf(t, dof), p, 0.0, hi)
}

/// The `p`-quantile of χ² with `dof` degrees of freedom, `0 < p < 1`.
pub fn chi2_quantile(p: f64, dof: f64) -> f64 {
    let mut hi = dof.max(1.0);
    while chi2_cdf(hi, dof) < p && hi < 1e12 {
        hi *= 2.0;
    }
    invert(|x| chi2_cdf(x, dof), p, 0.0, hi)
}

/// A mean with its two-sided 95 % Student's t interval over `n` values.
#[derive(
    Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
pub struct Interval {
    pub n: usize,
    pub mean: f64,
    /// The sample standard deviation.
    pub sd: f64,
    /// `sd/√n`, or the given combined standard error.
    pub se: f64,
    /// `t₀.₉₇₅,ₙ₋₁`.
    pub t: f64,
    pub lo: f64,
    pub hi: f64,
}

impl Interval {
    /// The mean of `x` and its interval, `mean ± t₀.₉₇₅,ₙ₋₁·sd/√n`.
    pub fn of(x: &[f64]) -> Interval {
        let n = x.len();
        let (m, s) = (mean(x), sd(x));
        Interval::with_se(n, m, s, s / (n as f64).sqrt())
    }

    /// `mean ± t₀.₉₇₅,ₙ₋₁·se` with a standard error of the caller's.
    pub fn with_se(n: usize, mean: f64, sd: f64, se: f64) -> Interval {
        let t = if n >= 2 {
            t_quantile(0.975, (n - 1) as f64)
        } else {
            f64::NAN
        };
        Interval {
            n,
            mean,
            sd,
            se,
            t,
            lo: mean - t * se,
            hi: mean + t * se,
        }
    }

    /// The largest distance of the interval from 0: `|mean| + t·se`.
    pub fn reach(&self) -> f64 {
        self.mean.abs() + self.t * self.se
    }

    /// The interval does not contain 0.
    pub fn excludes_zero(&self) -> bool {
        self.lo > 0.0 || self.hi < 0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quantiles_are_the_published_values() {
        // Student's t, two-sided 95 %: 1, 9, 19 and 30 degrees of freedom.
        for (dof, want) in [
            (1.0, 12.706_204_736),
            (9.0, 2.262_157_163),
            (19.0, 2.093_024_054),
            (30.0, 2.042_272_456),
        ] {
            let got = t_quantile(0.975, dof);
            assert!((got - want).abs() < 1e-7, "t {dof}: {got}");
        }
        assert!((t_quantile(0.025, 9.0) + 2.262_157_163).abs() < 1e-7);
        // χ² with 9 degrees of freedom, the 5 % and 95 % points.
        assert!((chi2_quantile(0.05, 9.0) - 3.325_112_843).abs() < 1e-7);
        assert!((chi2_quantile(0.95, 9.0) - 16.918_977_605).abs() < 1e-7);
        // The spec's three-figure values.
        assert_eq!(format!("{:.3}", t_quantile(0.975, 9.0)), "2.262");
        assert_eq!(format!("{:.3}", t_quantile(0.975, 19.0)), "2.093");
    }

    #[test]
    fn mean_sd_and_range() {
        let x = [1.0, 2.0, 3.0, 4.0];
        assert_eq!(mean(&x), 2.5);
        assert!((sd(&x) - (5.0f64 / 3.0).sqrt()).abs() < 1e-15);
        assert!((relative_range(&x) - 3.0 / 2.5).abs() < 1e-15);
        assert!(sd(&[1.0]).is_nan());
        let i = Interval::of(&x);
        assert_eq!(i.n, 4);
        assert!((i.hi - i.lo - 2.0 * i.t * i.se).abs() < 1e-12);
        assert!(i.excludes_zero());
        assert!(!Interval::of(&[-1.0, 1.0, 0.5]).excludes_zero());
    }
}
