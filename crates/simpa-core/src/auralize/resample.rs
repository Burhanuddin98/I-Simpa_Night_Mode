//! Sample-rate conversion of a dry recording to the synthesis rate: band-limited
//! interpolation with a Kaiser-windowed sinc (Smith's method), low-passed at 45 % of the lower of
//! the two rates, 48 zero crossings a side, β = 9 (about 90 dB of stopband). For a rational ratio
//! `L/M` with `L` at most [`MAX_PHASES`] the kernel is tabulated once per phase; otherwise it is
//! evaluated per output sample. Output sample `n` is the input at time `n·from/to`.

use std::f64::consts::PI;

/// Zero crossings of the sinc on each side of the centre.
const ZEROS: f64 = 48.0;
/// The Kaiser window's β.
const BETA: f64 = 9.0;
/// The cutoff, as a fraction of the lower rate.
const CUTOFF: f64 = 0.45;
/// The most phases tabulated.
pub const MAX_PHASES: u64 = 4096;

fn bessel_i0(x: f64) -> f64 {
    let mut sum = 1.0;
    let mut term = 1.0;
    let q = x * x / 4.0;
    for k in 1..200 {
        term *= q / (k * k) as f64;
        sum += term;
        if term < sum * 1e-17 {
            break;
        }
    }
    sum
}

fn gcd(a: u64, b: u64) -> u64 {
    if b == 0 { a } else { gcd(b, a % b) }
}

/// The resampling kernel at `t` input samples from the output instant: `fc` is the cutoff in
/// cycles per input sample, `half` the half-length in input samples.
fn kernel(t: f64, fc: f64, half: f64, i0b: f64) -> f64 {
    if t.abs() >= half {
        return 0.0;
    }
    let x = 2.0 * fc * t;
    let sinc = if x.abs() < 1e-12 {
        1.0
    } else {
        (PI * x).sin() / (PI * x)
    };
    let r = t / half;
    let w = bessel_i0(BETA * (1.0 - r * r).max(0.0).sqrt()) / i0b;
    2.0 * fc * sinc * w
}

/// The number of output samples for `len` input samples: `ceil(len·to/from)`.
pub fn output_len(len: usize, from: u32, to: u32) -> usize {
    ((len as u128 * u128::from(to)).div_ceil(u128::from(from))) as usize
}

/// `x` at `from` Hz, resampled to `to` Hz (module docs). The same rate returns `x` unchanged.
pub fn resample(x: &[f64], from: u32, to: u32) -> Vec<f64> {
    assert!(from > 0 && to > 0, "resample: rates {from}, {to}");
    if from == to || x.is_empty() {
        return x.to_vec();
    }
    let fc = CUTOFF * f64::from(from.min(to)) / f64::from(from);
    let half = ZEROS / (2.0 * fc);
    let i0b = bessel_i0(BETA);
    let n_out = output_len(x.len(), from, to);
    let g = gcd(u64::from(from), u64::from(to));
    let (l, m) = (u64::from(to) / g, u64::from(from) / g);
    let reach = half.ceil() as i64;
    let taps = (2 * reach + 1) as usize;
    let sample = |j: i64| -> f64 {
        if j < 0 || j as usize >= x.len() {
            0.0
        } else {
            x[j as usize]
        }
    };
    let mut out = Vec::with_capacity(n_out);
    if l <= MAX_PHASES {
        // Output n sits at input time n·m/l = base + phase/l.
        let table: Vec<Vec<f64>> = (0..l)
            .map(|p| {
                let frac = p as f64 / l as f64;
                (0..taps)
                    .map(|i| kernel(frac - (i as i64 - reach) as f64, fc, half, i0b))
                    .collect()
            })
            .collect();
        for n in 0..n_out as u64 {
            let pos = n * m;
            let base = (pos / l) as i64;
            let row = &table[(pos % l) as usize];
            let mut acc = 0.0;
            for (i, &k) in row.iter().enumerate() {
                acc += k * sample(base + i as i64 - reach);
            }
            out.push(acc);
        }
    } else {
        let step = f64::from(from) / f64::from(to);
        for n in 0..n_out {
            let t = n as f64 * step;
            let base = t.floor() as i64;
            let frac = t - base as f64;
            let mut acc = 0.0;
            for i in 0..taps as i64 {
                let j = base + i - reach;
                acc += kernel(frac - (i - reach) as f64, fc, half, i0b) * sample(j);
            }
            out.push(acc);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tone(f: f64, rate: u32, len: usize) -> Vec<f64> {
        (0..len)
            .map(|n| (2.0 * PI * f * n as f64 / f64::from(rate)).sin())
            .collect()
    }

    fn rms(x: &[f64]) -> f64 {
        (x.iter().map(|v| v * v).sum::<f64>() / x.len() as f64).sqrt()
    }

    /// Measured numbers, quoted in the auralization report.
    #[test]
    fn a_tone_at_44_1_khz_becomes_the_same_tone_at_48_khz() {
        let x = tone(1000.0, 44_100, 44_100);
        let y = resample(&x, 44_100, 48_000);
        assert_eq!(y.len(), 48_000);
        let want = tone(1000.0, 48_000, 48_000);
        // Away from the ends, where the kernel runs off the input.
        let mid = 2000..46_000;
        let err: Vec<f64> = mid.clone().map(|n| y[n] - want[n]).collect();
        let db = 20.0 * (rms(&err) / rms(&want[mid])).log10();
        println!("44.1 -> 48 kHz, 1 kHz tone: error {db:.1} dB re the tone");
        assert!(db < -80.0, "{db} dB");
        // A 15 kHz tone too (passband edge region still inside the cutoff, 19.8 kHz).
        let x = tone(15_000.0, 44_100, 44_100);
        let y = resample(&x, 44_100, 48_000);
        let want = tone(15_000.0, 48_000, 48_000);
        let err: Vec<f64> = (2000..46_000).map(|n| y[n] - want[n]).collect();
        let db = 20.0 * (rms(&err) / rms(&want[2000..46_000])).log10();
        println!("44.1 -> 48 kHz, 15 kHz tone: error {db:.1} dB re the tone");
        assert!(db < -60.0, "{db} dB");
    }

    #[test]
    fn downsampling_from_96_khz_removes_what_48_khz_cannot_hold() {
        let x = tone(30_000.0, 96_000, 96_000);
        let y = resample(&x, 96_000, 48_000);
        assert_eq!(y.len(), 48_000);
        let db = 20.0 * (rms(&y[2000..46_000]) / rms(&x)).log10();
        println!("96 -> 48 kHz, 30 kHz tone: {db:.1} dB left");
        assert!(db < -80.0, "a 30 kHz tone aliased at {db} dB");
        let x = tone(1000.0, 96_000, 96_000);
        let y = resample(&x, 96_000, 48_000);
        let want = tone(1000.0, 48_000, 48_000);
        let err: Vec<f64> = (2000..46_000).map(|n| y[n] - want[n]).collect();
        assert!(20.0 * (rms(&err) / rms(&want)).log10() < -80.0);
    }

    #[test]
    fn same_rate_is_untouched_and_lengths_round_up() {
        let x = vec![0.1, 0.2, 0.3];
        assert_eq!(resample(&x, 48_000, 48_000), x);
        assert_eq!(output_len(3, 44_100, 48_000), 4);
        assert_eq!(output_len(44_100, 44_100, 48_000), 48_000);
        // An awkward ratio takes the untabulated path and still holds a tone.
        let x = tone(500.0, 44_057, 8000);
        let y = resample(&x, 44_057, 48_000);
        let want = tone(500.0, 48_000, y.len());
        let err: Vec<f64> = (1500..y.len() - 1500).map(|n| y[n] - want[n]).collect();
        assert!(20.0 * (rms(&err) / rms(&want)).log10() < -80.0);
    }
}
