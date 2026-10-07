//! Auralization: a pressure impulse response synthesised from an SPPS energy echogram, and its
//! convolution with an anechoic recording (C5, `docs/investigations/2026-10-07-auralization/`;
//! decisions 67 and 75).
//!
//! **What it is.** SPPS counts energy: a point receiver's `.recp` holds Pa² per time step in each
//! band, with no phase. The impulse response made here carries the echogram's decay and spectrum
//! per band and a random fine structure; it is not a measured or wave-based impulse response, and
//! the app says so where it plays one. The method is the one energy-based simulators use (noise
//! shaped per band by the echogram), with two refinements the CR4 bed needed (REPORT.md):
//!
//! 1. **Bands** ([`Filters`]): exactly the bands the run computed, each from the geometric
//!    midpoint with its lower neighbour in the run's set to the midpoint with its upper one; an
//!    outer edge (or one beside a band the run skipped) lies half a band out, half an octave for
//!    octaves and a sixth of one for thirds (Burhan, 2026-10-07 22:39: "the solver's own bands",
//!    not the nominal IEC edges). Each filter is zero-phase, its power response `w_b(f)` 1 inside
//!    the band and crossing to its neighbour as `cos²`/`sin²` of log-frequency, so neighbours'
//!    powers sum to exactly 1: a bank sums flat across the band set ([`Filters::ripple_db`]). Two
//!    banks on the same edges: the noise's, with a narrow crossover ([`NOISE_TRANSITION`]), and a
//!    smooth one ([`SMOOTH_TRANSITION`]) whose impulse responses are short.
//! 2. **Noise**: for each band, Gaussian noise drawn in the frequency domain on the band (so it is
//!    stationary, with no filter transient), its envelope flattened by alternating projections
//!    ([`FLATTEN_PASSES`]: to unit modulus, back onto the band), shaped by the noise bank, and
//!    scaled to unit mean power. Flattening keeps the band's energy over a window from swinging
//!    with the noise's own Rayleigh envelope. The generator is seeded from [`SEED`] (or a seed the
//!    caller gives) and a stream (0 for the sources summed, `i + 1` for source `i`); the seed is
//!    written into the WAV, and the same run gives the same samples bit for bit.
//! 3. **Envelope**: the band's energy per step, as power per sample, interpolated linearly between
//!    step centres and rescaled inside each step so the step's samples hold exactly the step's
//!    energy (a step with no energy stays silent; a lone step stays inside itself).
//! 4. **Synthesis**: per band, the noise times the square root of the envelope, band-limited again
//!    through the smooth bank (the echogram's step-to-step Monte-Carlo noise modulates the noise
//!    at 1 ms and would otherwise spread energy into the neighbouring bands: a T30 bias of up to
//!    +18 % at 500 Hz on CR4 without it), and scaled so the band holds the echogram's total energy.
//!    The bands are summed.
//! 5. **Correction** ([`CORRECTION_PASSES`]): the sum is analysed back through the smooth bank;
//!    where a band's energy, summed over its own time resolution ([`correction_windows`], `2/B`),
//!    differs from the echogram's, its envelope is scaled by the ratio, and step 4 is repeated
//!    with the same noise. The result's absolute scale is the run's (a step's samples squared and
//!    summed are about its energy in Pa²); [`normalise`] scales it for playback and the gain is
//!    recorded.
//!
//! Convolution with an anechoic recording ([`auralize`]) is a linear FFT convolution after the
//! recording is read ([`wav::read`]) and resampled to [`SAMPLE_RATE`] ([`resample`]).

use std::fmt;

use crate::results::{RunResults, spps::SppsResults};

pub mod fft;
pub mod resample;
pub mod wav;

use fft::{C, fft as transform, next_pow2};

/// The synthesis rate (Burhan, 2026-10-07 22:39: "48 kHz").
pub const SAMPLE_RATE: u32 = 48_000;

/// The noise seed every response is synthesised with unless the caller gives another: fixed, so a
/// run's response is the same bit for bit each time, and written into each WAV.
pub const SEED: u64 = 0x4E4D_4155_5241_4C31;

/// The peak level a response or an auralization is normalised to, dBFS.
pub const PEAK_DBFS: f64 = -1.0;

/// The longest anechoic recording taken, s: its convolution with a 10 s response fits an FFT of
/// 2²³ points.
pub const MAX_ANECHOIC_S: f64 = 120.0;

/// Why an auralization was refused: a stable code and what was found.
#[derive(Clone, Debug, PartialEq)]
pub struct AuralError {
    code: &'static str,
    detail: String,
}

impl AuralError {
    pub fn new(code: &'static str, detail: impl Into<String>) -> Self {
        AuralError {
            code,
            detail: detail.into(),
        }
    }

    /// `aural_not_spps`, `aural_no_receiver`, `aural_no_source`, `aural_no_echograms`,
    /// `aural_no_energy`, `aural_bands`, `aural_time_step`, `aural_wav_invalid`,
    /// `aural_wav_too_long`, `aural_would_clip`, `aural_bad_sample`.
    pub fn code(&self) -> &'static str {
        self.code
    }

    pub fn detail(&self) -> &str {
        &self.detail
    }
}

impl fmt::Display for AuralError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.code, self.detail)
    }
}

impl std::error::Error for AuralError {}

// ---- bands -------------------------------------------------------------------------------------

/// The band set a run's bands belong to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fraction {
    Octave,
    Third,
}

impl Fraction {
    /// A band's width, octaves.
    pub fn width_octaves(self) -> f64 {
        match self {
            Fraction::Octave => 1.0,
            Fraction::Third => 1.0 / 3.0,
        }
    }

    /// The set of `set_hz` (ascending centres), from the median spacing of neighbours: octaves
    /// when it is above 2/3 of an octave, thirds otherwise. `None` for fewer than two centres.
    pub fn infer(set_hz: &[f64]) -> Option<Fraction> {
        let mut steps: Vec<f64> = set_hz
            .windows(2)
            .map(|w| (w[1] / w[0]).log2())
            .filter(|s| s.is_finite() && *s > 0.0)
            .collect();
        if steps.is_empty() {
            return None;
        }
        steps.sort_by(f64::total_cmp);
        let median = steps[steps.len() / 2];
        Some(if median > 2.0 / 3.0 {
            Fraction::Octave
        } else {
            Fraction::Third
        })
    }
}

/// One band's edges, Hz.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Band {
    pub centre_hz: f64,
    pub lo_hz: f64,
    pub hi_hz: f64,
}

/// The filter bank on a run's bands (module docs, step 1).
#[derive(Clone, Debug, PartialEq)]
pub struct Filters {
    pub fraction: Fraction,
    pub bands: Vec<Band>,
    /// The crossover's width, octaves, centred on each edge: half the narrowest band.
    pub transition_octaves: f64,
}

impl Filters {
    /// The bank on `computed_hz` (the bands the run computed, ascending), each band's neighbours
    /// taken from `set_hz` (every band of the run's set, computed or not, ascending). When the set
    /// has one band, `fraction` names it; otherwise it is inferred and `fraction` is ignored.
    pub fn new(
        computed_hz: &[f64],
        set_hz: &[f64],
        fraction: Option<Fraction>,
    ) -> Result<Filters, AuralError> {
        if computed_hz.is_empty() {
            return Err(AuralError::new("aural_bands", "the run computed no band"));
        }
        if computed_hz.iter().any(|f| f.is_nan() || *f <= 0.0)
            || computed_hz
                .windows(2)
                .any(|w| w[1].partial_cmp(&w[0]) != Some(std::cmp::Ordering::Greater))
        {
            return Err(AuralError::new(
                "aural_bands",
                format!("the bands {computed_hz:?} are not positive and ascending"),
            ));
        }
        let fraction = Fraction::infer(set_hz)
            .or_else(|| Fraction::infer(computed_hz))
            .or(fraction)
            .ok_or_else(|| {
                AuralError::new(
                    "aural_bands",
                    format!(
                        "one band ({} Hz) does not say whether it is an octave or a third",
                        computed_hz[0]
                    ),
                )
            })?;
        let w = fraction.width_octaves();
        // A neighbour in the set is at most 1.5 band widths away; a computed band further off has
        // a skipped band between.
        let adjacent = |a: f64, b: f64| (b / a).log2() < 1.5 * w;
        let mut bands = Vec::with_capacity(computed_hz.len());
        for (i, &f) in computed_hz.iter().enumerate() {
            let half = 2f64.powf(w / 2.0);
            let lo = match i.checked_sub(1).map(|j| computed_hz[j]) {
                Some(p) if adjacent(p, f) => (p * f).sqrt(),
                _ => f / half,
            };
            let hi = match computed_hz.get(i + 1) {
                Some(&n) if adjacent(f, n) => (f * n).sqrt(),
                _ => f * half,
            };
            bands.push(Band {
                centre_hz: f,
                lo_hz: lo,
                hi_hz: hi,
            });
        }
        let narrowest = bands
            .iter()
            .map(|b| (b.hi_hz / b.lo_hz).log2())
            .fold(f64::INFINITY, f64::min);
        let filters = Filters {
            fraction,
            bands,
            transition_octaves: NOISE_TRANSITION * narrowest,
        };
        let top = filters.bands.last().expect("one band at least").hi_hz;
        if top >= 0.5 * f64::from(SAMPLE_RATE) {
            return Err(AuralError::new(
                "aural_bands",
                format!(
                    "the top band's upper edge, {top:.0} Hz, is at or above half the {SAMPLE_RATE} Hz rate"
                ),
            ));
        }
        Ok(filters)
    }

    /// Band `b`'s power response at `f` Hz, in [0, 1] (module docs, step 1).
    pub fn with_transition(&self, fraction: f64) -> Filters {
        let narrowest = self
            .bands
            .iter()
            .map(|b| (b.hi_hz / b.lo_hz).log2())
            .fold(f64::INFINITY, f64::min);
        Filters {
            transition_octaves: fraction * narrowest,
            ..self.clone()
        }
    }

    pub fn power(&self, b: usize, f: f64) -> f64 {
        if f.is_nan() || f <= 0.0 {
            return 0.0;
        }
        let x = f.log2();
        let t = self.transition_octaves;
        let band = &self.bands[b];
        let ramp = |edge: f64| -> f64 {
            let s = ((x - (edge - t / 2.0)) / t).clamp(0.0, 1.0);
            (std::f64::consts::FRAC_PI_2 * s).sin().powi(2)
        };
        ramp(band.lo_hz.log2()) * (1.0 - ramp(band.hi_hz.log2()))
    }

    /// Band `b`'s amplitude response: the square root of [`Filters::power`].
    pub fn amplitude(&self, b: usize, f: f64) -> f64 {
        self.power(b, f).sqrt()
    }

    /// Where the bank is flat: from the lowest edge plus half a transition to the highest edge
    /// minus half a transition, Hz.
    pub fn flat_range_hz(&self) -> (f64, f64) {
        let h = 2f64.powf(self.transition_octaves / 2.0);
        (
            self.bands[0].lo_hz * h,
            self.bands.last().expect("one band").hi_hz / h,
        )
    }

    /// The reconstruction ripple: the largest `|10·lg Σ_b w_b(f)|` over `points` frequencies spaced
    /// evenly in log-frequency across [`Filters::flat_range_hz`], dB.
    pub fn ripple_db(&self, points: usize) -> f64 {
        let (a, b) = self.flat_range_hz();
        (0..points)
            .map(|i| {
                let f = a * (b / a).powf(i as f64 / (points - 1).max(1) as f64);
                let sum: f64 = (0..self.bands.len()).map(|k| self.power(k, f)).sum();
                (10.0 * sum.log10()).abs()
            })
            .fold(0.0, f64::max)
    }
}

// ---- the generator -----------------------------------------------------------------------------

/// SplitMix64: seeds the generator.
fn splitmix(x: &mut u64) -> u64 {
    *x = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *x;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// xoshiro256** with Box–Muller normals: small, fast and the same on every platform.
struct Rng {
    s: [u64; 4],
    spare: Option<f64>,
}

impl Rng {
    fn new(seed: u64, stream: u64, part: u64) -> Rng {
        let mut x = seed
            ^ stream.wrapping_mul(0xD1B5_4A32_D192_ED03)
            ^ part.wrapping_mul(0x8CB9_2BA7_2F3D_8DD7);
        let s = [
            splitmix(&mut x),
            splitmix(&mut x),
            splitmix(&mut x),
            splitmix(&mut x),
        ];
        Rng { s, spare: None }
    }

    fn next(&mut self) -> u64 {
        let r = self.s[1].wrapping_mul(5).rotate_left(7).wrapping_mul(9);
        let t = self.s[1] << 17;
        self.s[2] ^= self.s[0];
        self.s[3] ^= self.s[1];
        self.s[1] ^= self.s[2];
        self.s[0] ^= self.s[3];
        self.s[2] ^= t;
        self.s[3] = self.s[3].rotate_left(45);
        r
    }

    /// Uniform in (0, 1].
    fn uniform(&mut self) -> f64 {
        ((self.next() >> 11) as f64 + 1.0) / (1u64 << 53) as f64
    }

    fn normal(&mut self) -> f64 {
        if let Some(v) = self.spare.take() {
            return v;
        }
        let r = (-2.0 * self.uniform().ln()).sqrt();
        let a = 2.0 * std::f64::consts::PI * self.uniform();
        self.spare = Some(r * a.sin());
        r * a.cos()
    }
}

// ---- synthesis ---------------------------------------------------------------------------------

/// An energy echogram: Pa² per time step, one series per band of the [`Filters`] it goes with.
#[derive(Clone, Debug, PartialEq)]
pub struct Echogram {
    pub dt_s: f64,
    pub energy: Vec<Vec<f64>>,
}

impl Echogram {
    pub fn steps(&self) -> usize {
        self.energy.first().map_or(0, Vec::len)
    }
}

/// A synthesised impulse response.
#[derive(Clone, Debug, PartialEq)]
pub struct Synthesis {
    /// Absolute: a step's samples squared and summed are its energy in Pa², in expectation.
    pub samples: Vec<f64>,
    pub rate: u32,
    pub seed: u64,
    pub stream: u64,
    pub filters: Filters,
    pub dt_s: f64,
}

/// The first sample of each step, and the end: `round(k·dt·rate)` for `k` in `0..=steps`.
pub fn step_bounds(steps: usize, dt_s: f64, rate: u32) -> Vec<usize> {
    (0..=steps)
        .map(|k| (k as f64 * dt_s * f64::from(rate)).round() as usize)
        .collect()
}

/// A band's power per sample (module docs, step 3): the step's energy over its samples at each
/// step centre, linear between centres, held before the first and after the last, then rescaled
/// inside each step to hold exactly the step's energy.
pub fn envelope(energy: &[f64], bounds: &[usize]) -> Vec<f64> {
    let n = *bounds.last().unwrap_or(&0);
    let steps = energy.len();
    let mut p = vec![0.0; n];
    let width = |k: usize| (bounds[k + 1] - bounds[k]).max(1) as f64;
    let centre = |k: usize| 0.5 * (bounds[k] + bounds[k + 1]) as f64 - 0.5;
    let density = |k: usize| energy[k] / width(k);
    for k in 0..steps {
        let (a, b) = (bounds[k], bounds[k + 1]);
        if a == b || energy[k] <= 0.0 {
            continue;
        }
        let mut sum = 0.0;
        for (i, slot) in p.iter_mut().enumerate().take(b).skip(a) {
            let t = i as f64;
            let c = centre(k);
            let v = if t < c {
                if k == 0 {
                    density(k)
                } else {
                    let (c0, d0) = (centre(k - 1), density(k - 1));
                    d0 + (density(k) - d0) * (t - c0) / (c - c0)
                }
            } else if k + 1 == steps {
                density(k)
            } else {
                let (c1, d1) = (centre(k + 1), density(k + 1));
                density(k) + (d1 - density(k)) * (t - c) / (c1 - c)
            };
            *slot = v;
            sum += v;
        }
        let scale = energy[k] / sum;
        for v in &mut p[a..b] {
            *v *= scale;
        }
    }
    p
}

/// The band components of the response (module docs): one per band, `n` samples each, before
/// they are summed. [`synthesise`] sums them; the tests and the bed read them apart.
pub fn components(
    e: &Echogram,
    filters: &Filters,
    rate: u32,
    seed: u64,
    stream: u64,
) -> Result<Vec<Vec<f64>>, AuralError> {
    let mut out = Vec::with_capacity(filters.bands.len());
    synthesise_with(e, filters, rate, seed, stream, |_, c| out.push(c.to_vec()))?;
    Ok(out)
}

/// The response (module docs).
pub fn synthesise(
    e: &Echogram,
    filters: &Filters,
    rate: u32,
    seed: u64,
    stream: u64,
) -> Result<Synthesis, AuralError> {
    let mut sum: Vec<f64> = Vec::new();
    synthesise_with(e, filters, rate, seed, stream, |_, c| {
        if sum.is_empty() {
            sum = vec![0.0; c.len()];
        }
        for (s, v) in sum.iter_mut().zip(c) {
            *s += v;
        }
    })?;
    Ok(Synthesis {
        samples: sum,
        rate,
        seed,
        stream,
        filters: filters.clone(),
        dt_s: e.dt_s,
    })
}

fn synthesise_with(
    e: &Echogram,
    filters: &Filters,
    rate: u32,
    seed: u64,
    stream: u64,
    mut each: impl FnMut(usize, &[f64]),
) -> Result<(), AuralError> {
    let nb = filters.bands.len();
    if e.energy.len() != nb {
        return Err(AuralError::new(
            "aural_bands",
            format!("{} series for {nb} bands", e.energy.len()),
        ));
    }
    let steps = e.steps();
    if e.energy.iter().any(|s| s.len() != steps) || steps == 0 {
        return Err(AuralError::new(
            "aural_no_energy",
            "the bands' series are empty or differ in length",
        ));
    }
    if let Some(v) = e
        .energy
        .iter()
        .flatten()
        .find(|v| !v.is_finite() || **v < 0.0)
    {
        return Err(AuralError::new(
            "aural_no_energy",
            format!("an energy of {v} Pa²"),
        ));
    }
    if !e.energy.iter().flatten().any(|v| *v > 0.0) {
        return Err(AuralError::new(
            "aural_no_energy",
            "every step of every band is 0",
        ));
    }
    if !(e.dt_s.is_finite() && e.dt_s * f64::from(rate) >= 1.0) {
        return Err(AuralError::new(
            "aural_time_step",
            format!(
                "a time step of {} s is shorter than one sample at {rate} Hz",
                e.dt_s
            ),
        ));
    }
    let bounds = step_bounds(steps, e.dt_s, rate);
    let n = *bounds.last().expect("steps >= 1");
    let nfft = next_pow2(n);
    let smooth = filters.with_transition(SMOOTH_TRANSITION);
    let live: Vec<bool> = e
        .energy
        .iter()
        .map(|s| s.iter().any(|v| *v > 0.0))
        .collect();
    // The noise, once per band, scaled to unit mean power over the response.
    let noises: Vec<Vec<f64>> = (0..nb)
        .map(|k| {
            if !live[k] {
                return Vec::new();
            }
            let mut rng = Rng::new(seed, stream, k as u64);
            let mut v = band_noise(filters, k, nfft, rate, &mut rng);
            v.truncate(n);
            let power = v.iter().map(|x| x * x).sum::<f64>() / n as f64;
            let g = 1.0 / power.sqrt();
            v.iter_mut().for_each(|x| *x *= g);
            v
        })
        .collect();
    let mut envs: Vec<Vec<f64>> = (0..nb)
        .map(|k| {
            if live[k] {
                envelope(&e.energy[k], &bounds)
            } else {
                Vec::new()
            }
        })
        .collect();
    let windows = correction_windows(&smooth, e.dt_s);
    let targets: Vec<Vec<f64>> = (0..nb)
        .map(|k| moving_sum(&e.energy[k], windows[k]))
        .collect();
    let totals: Vec<f64> = e.energy.iter().map(|s| s.iter().sum()).collect();
    let mut comps: Vec<Vec<f64>> = vec![Vec::new(); nb];
    for pass in 0..=CORRECTION_PASSES {
        let modulated: Vec<Vec<f64>> = (0..nb)
            .map(|k| {
                if live[k] {
                    noises[k]
                        .iter()
                        .zip(&envs[k])
                        .map(|(v, q)| v * q.sqrt())
                        .collect()
                } else {
                    vec![0.0; n]
                }
            })
            .collect();
        comps = band_limit(&modulated, &smooth, rate);
        for k in 0..nb {
            let got: f64 = comps[k].iter().map(|v| v * v).sum();
            if live[k] && got > 0.0 {
                let g = (totals[k] / got).sqrt();
                comps[k].iter_mut().for_each(|v| *v *= g);
            }
        }
        if pass == CORRECTION_PASSES {
            break;
        }
        let sum: Vec<f64> = (0..n).map(|i| comps.iter().map(|c| c[i]).sum()).collect();
        let anal = analyse(&sum, &smooth, rate);
        for k in (0..nb).filter(|&k| live[k]) {
            let got = moving_sum(&step_energies(&anal[k], &bounds), windows[k]);
            for j in 0..steps {
                let f = if got[j] > 0.0 && targets[k][j] > 0.0 {
                    (targets[k][j] / got[j]).clamp(0.25, 4.0)
                } else {
                    1.0
                };
                for v in &mut envs[k][bounds[j]..bounds[j + 1]] {
                    *v *= f;
                }
            }
        }
    }
    for (k, c) in comps.iter().enumerate() {
        each(k, c);
    }
    Ok(())
}

/// The noise's crossover, as a fraction of the narrowest band: narrow, so the flattened noise
/// keeps its flat envelope through the band's shaping (module docs, step 2).
pub const NOISE_TRANSITION: f64 = 0.1;

/// The crossover of the bank the modulated noise is band-limited with and analysed back through:
/// smooth, so its impulse response is short (module docs, steps 4 and 5).
pub const SMOOTH_TRANSITION: f64 = 0.5;

/// How many times the envelope is corrected (module docs, step 5).
pub const CORRECTION_PASSES: usize = 3;

/// The correction's time resolution in a band, in cycles of its bandwidth: `2 / B` seconds.
pub const CORRECTION_CYCLES: f64 = 2.0;

/// Each band's correction window, steps: [`CORRECTION_CYCLES`] over the band's width in Hz, at
/// least one step.
pub fn correction_windows(filters: &Filters, dt_s: f64) -> Vec<usize> {
    filters
        .bands
        .iter()
        .map(|b| ((CORRECTION_CYCLES / ((b.hi_hz - b.lo_hz) * dt_s)).round() as usize).max(1))
        .collect()
}

/// `x` summed over a centred window of `w` steps (clipped at the ends).
pub fn moving_sum(x: &[f64], w: usize) -> Vec<f64> {
    let mut prefix = vec![0.0; x.len() + 1];
    for (i, v) in x.iter().enumerate() {
        prefix[i + 1] = prefix[i] + v;
    }
    let half = w / 2;
    (0..x.len())
        .map(|j| {
            let a = j.saturating_sub(half);
            let b = (a + w).min(x.len());
            (prefix[b] - prefix[a]).max(0.0)
        })
        .collect()
}

/// Each of `xs` through its band's filter of `filters` (zero-phase, two bands to a transform),
/// padded by at least a quarter second so a filter's ringing does not wrap.
fn band_limit(xs: &[Vec<f64>], filters: &Filters, rate: u32) -> Vec<Vec<f64>> {
    let n = xs.first().map_or(0, Vec::len);
    let nfft = next_pow2(n + rate as usize / 4);
    let df = f64::from(rate) / nfft as f64;
    let nb = xs.len();
    let mut out = vec![Vec::new(); nb];
    let mut z = vec![C::ZERO; nfft];
    for pair in 0..nb.div_ceil(2) {
        let (a, b) = (2 * pair, 2 * pair + 1);
        z.iter_mut().for_each(|v| *v = C::ZERO);
        for (i, v) in xs[a].iter().enumerate() {
            z[i].re = *v;
        }
        if b < nb {
            for (i, v) in xs[b].iter().enumerate() {
                z[i].im = *v;
            }
        }
        transform(&mut z, false);
        let spec = z.clone();
        for k in 0..nfft {
            let zk = spec[k];
            let zn = spec[(nfft - k) % nfft].conj();
            let xa = (zk + zn).scale(0.5);
            let d = (zk - zn).scale(0.5);
            let xb = C::new(d.im, -d.re);
            let f = k.min(nfft - k) as f64 * df;
            let ya = xa.scale(filters.amplitude(a, f));
            let yb = if b < nb {
                xb.scale(filters.amplitude(b, f))
            } else {
                C::ZERO
            };
            z[k] = ya + C::new(-yb.im, yb.re);
        }
        transform(&mut z, true);
        out[a] = z[..n].iter().map(|c| c.re).collect();
        if b < nb {
            out[b] = z[..n].iter().map(|c| c.im).collect();
        }
    }
    out
}

/// How many times [`band_noise`] flattens the noise's envelope.
pub const FLATTEN_PASSES: usize = 4;

/// Band `k`'s noise, `nfft` samples, periodic (module docs, step 2): Gaussian noise drawn in the
/// frequency domain on the band's support, as an analytic signal; its envelope flattened by
/// [`FLATTEN_PASSES`] alternating projections (to unit modulus, then back onto the band's
/// support); then shaped by the band's amplitude response. The real part is returned.
fn band_noise(filters: &Filters, k: usize, nfft: usize, rate: u32, rng: &mut Rng) -> Vec<f64> {
    let df = f64::from(rate) / nfft as f64;
    let h: Vec<f64> = (0..nfft / 2)
        .map(|i| filters.amplitude(k, i as f64 * df))
        .collect();
    let mut z = vec![C::ZERO; nfft];
    for i in 1..nfft / 2 {
        if h[i] > 0.0 {
            z[i] = C::new(rng.normal(), rng.normal());
        }
    }
    transform(&mut z, true);
    for _ in 0..FLATTEN_PASSES {
        for v in z.iter_mut() {
            let m = v.norm_sqr().sqrt();
            *v = if m > 0.0 { v.scale(1.0 / m) } else { C::ZERO };
        }
        transform(&mut z, false);
        z[0] = C::ZERO;
        for (i, v) in z.iter_mut().enumerate().skip(1) {
            if i >= nfft / 2 || h[i] == 0.0 {
                *v = C::ZERO;
            }
        }
        transform(&mut z, true);
    }
    transform(&mut z, false);
    for (i, v) in z.iter_mut().enumerate() {
        *v = if i < nfft / 2 { v.scale(h[i]) } else { C::ZERO };
    }
    transform(&mut z, true);
    z.into_iter().map(|c| c.re).collect()
}

// ---- analysis (the tests' and the bed's) -------------------------------------------------------

/// `x` through every band's filter (zero-phase, linear: padded so nothing wraps), one signal per
/// band, each as long as `x`.
pub fn analyse(x: &[f64], filters: &Filters, rate: u32) -> Vec<Vec<f64>> {
    analyse_by(x, filters.bands.len(), rate, |b, f| filters.amplitude(b, f))
}

/// The order of the bed's Butterworth prototype ([`analyse_butterworth`]).
pub const BED_ORDER: i32 = 6;

/// The bed's own bank, independent of the synthesis's: on the same edges, a twelfth-order
/// Butterworth band-pass magnitude ([`BED_ORDER`] prototype), `|H|² = 1 / (1 + ((f² − f₀²) /
/// (f·(f_hi − f_lo)))¹²)` with `f₀ = √(f_lo·f_hi)`, applied zero-phase. The EDT and T30 the bed holds
/// to the JND are measured through it, so the correction (module docs, step 5), which works through
/// [`analyse`], is not graded by its own instrument.
pub fn analyse_butterworth(x: &[f64], filters: &Filters, rate: u32) -> Vec<Vec<f64>> {
    analyse_by(x, filters.bands.len(), rate, |b, f| {
        let band = &filters.bands[b];
        if f.is_nan() || f <= 0.0 {
            return 0.0;
        }
        let f0sq = band.lo_hz * band.hi_hz;
        let r = (f * f - f0sq) / (f * (band.hi_hz - band.lo_hz));
        (1.0 / (1.0 + r.powi(2 * BED_ORDER))).sqrt()
    })
}

fn analyse_by(
    x: &[f64],
    nb: usize,
    rate: u32,
    amplitude: impl Fn(usize, f64) -> f64,
) -> Vec<Vec<f64>> {
    let n = x.len();
    let nfft = next_pow2(2 * n.max(1));
    let df = f64::from(rate) / nfft as f64;
    let mut spec = vec![C::ZERO; nfft];
    for (s, &v) in spec.iter_mut().zip(x) {
        s.re = v;
    }
    transform(&mut spec, false);
    let mut out = vec![Vec::new(); nb];
    let mut buf = vec![C::ZERO; nfft];
    for pair in 0..nb.div_ceil(2) {
        let (a, b) = (2 * pair, 2 * pair + 1);
        for k in 0..nfft {
            // The frequency of bin k, folded: the responses are even.
            let f = k.min(nfft - k) as f64 * df;
            let ha = amplitude(a, f);
            let hb = if b < nb { amplitude(b, f) } else { 0.0 };
            let s = spec[k];
            let ya = s.scale(ha);
            let yb = s.scale(hb);
            buf[k] = ya + C::new(-yb.im, yb.re);
        }
        transform(&mut buf, true);
        out[a] = buf[..n].iter().map(|c| c.re).collect();
        if b < nb {
            out[b] = buf[..n].iter().map(|c| c.im).collect();
        }
    }
    out
}

/// The energy of `x` in each step: its samples squared and summed between [`step_bounds`].
pub fn step_energies(x: &[f64], bounds: &[usize]) -> Vec<f64> {
    bounds
        .windows(2)
        .map(|w| {
            x[w[0].min(x.len())..w[1].min(x.len())]
                .iter()
                .map(|v| v * v)
                .sum()
        })
        .collect()
}

/// EDT and T30 of an energy series by ISO 3382-1's method, written plainly so the echogram and
/// the synthesised response are measured by one estimator: Schroeder's backward integral from the
/// onset (the first step within 20 dB of the largest), then the least-squares slope of its level
/// over 0 to −10 dB (EDT) and −5 to −35 dB (T30), each `−60 / slope`. `None` when the curve does
/// not reach the range's bottom or fewer than two steps lie in it.
pub fn decay_times(energy: &[f64], dt_s: f64) -> (Option<f64>, Option<f64>) {
    let max = energy.iter().copied().fold(0.0, f64::max);
    if max <= 0.0 {
        return (None, None);
    }
    let onset = energy.iter().position(|&e| e >= max * 0.01).unwrap_or(0);
    let mut s = vec![0.0; energy.len() + 1];
    for k in (0..energy.len()).rev() {
        s[k] = s[k + 1] + energy[k];
    }
    let total = s[onset];
    let level: Vec<(f64, f64)> = (onset..energy.len())
        .filter(|&k| s[k] > 0.0)
        .map(|k| (k as f64 * dt_s, 10.0 * (s[k] / total).log10()))
        .collect();
    let fit = |top: f64, bottom: f64| -> Option<f64> {
        if !level.iter().any(|&(_, l)| l <= bottom) {
            return None;
        }
        let pts: Vec<(f64, f64)> = level
            .iter()
            .copied()
            .filter(|&(_, l)| l <= top && l >= bottom)
            .collect();
        if pts.len() < 2 {
            return None;
        }
        let m = pts.len() as f64;
        let (mt, ml) = (
            pts.iter().map(|p| p.0).sum::<f64>() / m,
            pts.iter().map(|p| p.1).sum::<f64>() / m,
        );
        let sxy: f64 = pts.iter().map(|p| (p.0 - mt) * (p.1 - ml)).sum();
        let sxx: f64 = pts.iter().map(|p| (p.0 - mt).powi(2)).sum();
        let slope = sxy / sxx;
        (slope < 0.0).then(|| -60.0 / slope)
    };
    (fit(0.0, -10.0), fit(-5.0, -35.0))
}

// ---- normalising and convolving ----------------------------------------------------------------

/// Scales `x` so its largest magnitude is [`PEAK_DBFS`]; returns the gain applied (linear). A
/// silent `x` is left alone, gain 1.
pub fn normalise(x: &mut [f64]) -> f64 {
    let peak = x.iter().fold(0.0_f64, |m, v| m.max(v.abs()));
    if peak <= 0.0 {
        return 1.0;
    }
    let g = 10f64.powf(PEAK_DBFS / 20.0) / peak;
    for v in x.iter_mut() {
        *v *= g;
    }
    g
}

/// The anechoic recording `bytes` (a WAV), mixed to mono and resampled to [`SAMPLE_RATE`]
/// ([`resample`]); refused when it is not a WAV this reads or is longer than
/// [`MAX_ANECHOIC_S`].
pub fn anechoic(bytes: &[u8]) -> Result<(wav::Wav, Vec<f64>), AuralError> {
    let w = wav::read(bytes)?;
    let secs = w.samples.len() as f64 / f64::from(w.rate);
    if secs > MAX_ANECHOIC_S {
        return Err(AuralError::new(
            "aural_wav_too_long",
            format!("{secs:.1} s, the longest taken is {MAX_ANECHOIC_S} s: trim it first"),
        ));
    }
    if w.samples.is_empty() {
        return Err(AuralError::new("aural_wav_invalid", "no samples"));
    }
    let x = resample::resample(&w.samples, w.rate, SAMPLE_RATE);
    Ok((w, x))
}

/// `dry` convolved with the response `ir` (both at [`SAMPLE_RATE`]), the trailing samples more
/// than 120 dB below the peak cut off (never shorter than `dry`).
pub fn auralize(ir: &[f64], dry: &[f64]) -> Vec<f64> {
    let mut y = fft::convolve(dry, ir);
    let peak = y.iter().fold(0.0_f64, |m, v| m.max(v.abs()));
    let floor = peak * 1e-6;
    let end = y
        .iter()
        .rposition(|v| v.abs() > floor)
        .map_or(0, |i| i + 1)
        .max(dry.len().min(y.len()));
    y.truncate(end);
    y
}

// ---- a run's echograms -------------------------------------------------------------------------

/// Which echogram of a receiver: every source summed (the `.recp`), or one source's own.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pick<'a> {
    Summed,
    Source(&'a str),
}

fn spps_of(run: &RunResults) -> Result<&SppsResults, AuralError> {
    run.spps().ok_or_else(|| {
        AuralError::new(
            "aural_not_spps",
            "a TCR run has no echogram: auralization reads SPPS's",
        )
    })
}

/// The bank on a run's bands: the computed bands within the config's whole set.
pub fn run_filters(run: &RunResults) -> Result<Filters, AuralError> {
    let computed: Vec<f64> = run.bands_hz.iter().map(|&f| f64::from(f)).collect();
    let set: Vec<f64> = run
        .expectation
        .bands
        .iter()
        .map(|b| f64::from(b.freq_hz))
        .collect();
    Filters::new(&computed, &set, None)
}

/// A receiver's echogram, and the stream its noise is drawn from (0 summed, `i + 1` source `i`).
pub fn run_echogram(
    run: &RunResults,
    receiver: &str,
    pick: Pick<'_>,
) -> Result<(Echogram, u64), AuralError> {
    let s = spps_of(run)?;
    let r = s.point_receiver(receiver).ok_or_else(|| {
        AuralError::new(
            "aural_no_receiver",
            format!(
                "no point receiver '{receiver}'; the run has {:?}",
                s.point_receivers
                    .iter()
                    .map(|p| p.label.as_str())
                    .collect::<Vec<_>>()
            ),
        )
    })?;
    let (energy, stream) = match pick {
        Pick::Summed => (
            r.bands.iter().map(|b| b.energy.clone()).collect::<Vec<_>>(),
            0,
        ),
        Pick::Source(name) => {
            let index = s
                .sources
                .iter()
                .position(|x| x.name == name)
                .ok_or_else(|| {
                    AuralError::new(
                        "aural_no_source",
                        format!(
                            "no source '{name}'; the run has {:?}",
                            s.sources
                                .iter()
                                .map(|x| x.name.as_str())
                                .collect::<Vec<_>>()
                        ),
                    )
                })?;
            let e = r.echograms.iter().find(|e| e.source == name).ok_or_else(|| {
                AuralError::new(
                    "aural_no_echograms",
                    format!(
                        "the run wrote no echogram per source (output_recp_bysource is off), so \
                         '{name}' cannot be heard alone; the sources summed can"
                    ),
                )
            })?;
            (e.energy.clone(), index as u64 + 1)
        }
    };
    Ok((
        Echogram {
            dt_s: s.time_step_s,
            energy,
        },
        stream,
    ))
}

/// A run's response for one receiver, synthesised (module docs), with what the WAV records.
#[derive(Clone, Debug, PartialEq)]
pub struct Response {
    pub synthesis: Synthesis,
    pub receiver: String,
    /// `None`: the sources summed.
    pub source: Option<String>,
}

impl Response {
    /// The WAV's comment for this response, scaled by `gain` (what [`normalise`] returned).
    pub fn comment(&self, gain: f64, what: &str) -> String {
        let s = &self.synthesis;
        let edges: Vec<String> = s
            .filters
            .bands
            .iter()
            .map(|b| format!("{:.1}-{:.1}", b.lo_hz, b.hi_hz))
            .collect();
        format!(
            "I-Simpa Night Mode {what}: synthesised from the SPPS energy echogram of receiver {} \
             ({}); the room's decay and spectrum per band with a random fine structure, not a \
             measured or wave-based impulse response. {} Hz; noise seed {:#018x}, stream {}; \
             {:?} bands, edges Hz {}; time step {} s; samples are the response times {gain:.6e} \
             (the response in Pa per step-energy root: divide by it for the run's absolute scale)",
            self.receiver,
            self.source
                .as_deref()
                .map_or("all sources summed".to_string(), |x| format!("source {x}")),
            s.rate,
            s.seed,
            s.stream,
            s.filters.fraction,
            edges.join(", "),
            s.dt_s,
        )
    }
}

/// The response of `receiver` in `run` for `pick`, at [`SAMPLE_RATE`] with `seed`.
pub fn response(
    run: &RunResults,
    receiver: &str,
    pick: Pick<'_>,
    seed: u64,
) -> Result<Response, AuralError> {
    let filters = run_filters(run)?;
    let (e, stream) = run_echogram(run, receiver, pick)?;
    let synthesis = synthesise(&e, &filters, SAMPLE_RATE, seed, stream)?;
    Ok(Response {
        synthesis,
        receiver: receiver.to_string(),
        source: match pick {
            Pick::Summed => None,
            Pick::Source(s) => Some(s.to_string()),
        },
    })
}

#[cfg(test)]
mod tests;
