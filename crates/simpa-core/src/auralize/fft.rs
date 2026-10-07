//! An iterative radix-2 complex FFT in `f64`, and the FFT convolution the auralization uses. Written
//! here rather than taken from a crate: the synthesis needs only power-of-two transforms, and the
//! test against a direct DFT below holds it.

use std::f64::consts::PI;

/// A complex number.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct C {
    pub re: f64,
    pub im: f64,
}

impl C {
    pub const ZERO: C = C { re: 0.0, im: 0.0 };

    pub const fn new(re: f64, im: f64) -> C {
        C { re, im }
    }

    pub fn conj(self) -> C {
        C::new(self.re, -self.im)
    }

    pub fn scale(self, s: f64) -> C {
        C::new(self.re * s, self.im * s)
    }

    pub fn norm_sqr(self) -> f64 {
        self.re * self.re + self.im * self.im
    }
}

impl std::ops::Add for C {
    type Output = C;
    fn add(self, o: C) -> C {
        C::new(self.re + o.re, self.im + o.im)
    }
}

impl std::ops::Sub for C {
    type Output = C;
    fn sub(self, o: C) -> C {
        C::new(self.re - o.re, self.im - o.im)
    }
}

impl std::ops::Mul for C {
    type Output = C;
    fn mul(self, o: C) -> C {
        C::new(
            self.re * o.re - self.im * o.im,
            self.re * o.im + self.im * o.re,
        )
    }
}

/// The smallest power of two at least `n` (1 for 0).
pub fn next_pow2(n: usize) -> usize {
    n.max(1).next_power_of_two()
}

/// In place: the forward DFT `X[k] = Σ x[n]·e^(−2πikn/N)`, or with `inverse` the inverse
/// `x[n] = (1/N)·Σ X[k]·e^(2πikn/N)`. Panics unless the length is a power of two.
pub fn fft(buf: &mut [C], inverse: bool) {
    let n = buf.len();
    assert!(n.is_power_of_two(), "fft: length {n} is not a power of two");
    if n == 1 {
        return;
    }
    // Bit reversal.
    let bits = n.trailing_zeros();
    for i in 0..n {
        let j = i.reverse_bits() >> (usize::BITS - bits);
        if j > i {
            buf.swap(i, j);
        }
    }
    // Twiddles for the largest stage, `e^(∓2πik/N)`, k < N/2; a stage of length `len` takes every
    // `N/len`-th.
    let sign = if inverse { 1.0 } else { -1.0 };
    let tw: Vec<C> = (0..n / 2)
        .map(|k| {
            let a = sign * 2.0 * PI * k as f64 / n as f64;
            C::new(a.cos(), a.sin())
        })
        .collect();
    let mut len = 2;
    while len <= n {
        let half = len / 2;
        let stride = n / len;
        for start in (0..n).step_by(len) {
            for k in 0..half {
                let w = tw[k * stride];
                let a = buf[start + k];
                let b = buf[start + k + half] * w;
                buf[start + k] = a + b;
                buf[start + k + half] = a - b;
            }
        }
        len <<= 1;
    }
    if inverse {
        let s = 1.0 / n as f64;
        for v in buf.iter_mut() {
            *v = v.scale(s);
        }
    }
}

/// The linear convolution of two real sequences, `len(x) + len(h) − 1` long, through one forward
/// and one inverse FFT: `x` rides in the real part and `h` in the imaginary part, and the two
/// spectra are separated by their symmetry.
pub fn convolve(x: &[f64], h: &[f64]) -> Vec<f64> {
    if x.is_empty() || h.is_empty() {
        return Vec::new();
    }
    let out_len = x.len() + h.len() - 1;
    let n = next_pow2(out_len);
    let mut z = vec![C::ZERO; n];
    for (i, &v) in x.iter().enumerate() {
        z[i].re = v;
    }
    for (i, &v) in h.iter().enumerate() {
        z[i].im = v;
    }
    fft(&mut z, false);
    // X[k] = (Z[k] + conj Z[N−k]) / 2, H[k] = (Z[k] − conj Z[N−k]) / 2i; Y = X·H.
    let mut y = vec![C::ZERO; n];
    for k in 0..n {
        let zk = z[k];
        let zn = z[(n - k) % n].conj();
        let xk = (zk + zn).scale(0.5);
        let d = (zk - zn).scale(0.5);
        let hk = C::new(d.im, -d.re);
        y[k] = xk * hk;
    }
    fft(&mut y, true);
    y.truncate(out_len);
    y.into_iter().map(|v| v.re).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dft(x: &[C]) -> Vec<C> {
        let n = x.len();
        (0..n)
            .map(|k| {
                x.iter().enumerate().fold(C::ZERO, |acc, (j, &v)| {
                    let a = -2.0 * PI * (k * j) as f64 / n as f64;
                    acc + v * C::new(a.cos(), a.sin())
                })
            })
            .collect()
    }

    #[test]
    fn fft_matches_a_direct_dft_and_inverts() {
        for n in [1usize, 2, 4, 8, 64, 256] {
            let x: Vec<C> = (0..n)
                .map(|i| {
                    C::new(
                        (i as f64 * 0.7).sin() + 0.1 * i as f64,
                        (i as f64 * 1.3).cos(),
                    )
                })
                .collect();
            let want = dft(&x);
            let mut got = x.clone();
            fft(&mut got, false);
            let err = got
                .iter()
                .zip(&want)
                .map(|(a, b)| (*a - *b).norm_sqr().sqrt())
                .fold(0.0, f64::max);
            assert!(err < 1e-9 * n as f64, "n {n}: {err}");
            fft(&mut got, true);
            let back = got
                .iter()
                .zip(&x)
                .map(|(a, b)| (*a - *b).norm_sqr().sqrt())
                .fold(0.0, f64::max);
            assert!(back < 1e-12, "n {n}: inverse {back}");
        }
    }

    #[test]
    fn convolution_matches_the_direct_sum() {
        let x: Vec<f64> = (0..37).map(|i| ((i * 7 % 11) as f64) - 5.0).collect();
        let h: Vec<f64> = (0..13).map(|i| (i as f64 * 0.4).cos()).collect();
        let got = convolve(&x, &h);
        assert_eq!(got.len(), 49);
        for (n, &g) in got.iter().enumerate() {
            let want: f64 = (0..h.len())
                .filter(|&k| n >= k && n - k < x.len())
                .map(|k| h[k] * x[n - k])
                .sum();
            assert!((g - want).abs() < 1e-10, "n {n}: {g} vs {want}");
        }
    }
}
