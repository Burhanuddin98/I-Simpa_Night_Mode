//! The speech transmission index, STI, from a predicted energy response: IEC 60268-16:2011
//! (edition 4), the indirect method of cl. 6.1 as cl. 8.3 asks for a simulated impulse response
//! (`docs/params.md`, "STI"). Written from the standard's text; not a port of upstream's
//! `Compute_STI_Param`, which uses band k's own level for its masking where Table A.1 uses band
//! k-1's, and whose `if(gen='F')` is an assignment, so every STI it gives has female weights.
//!
//! Per octave band k, 125 Hz to 8 kHz:
//!
//! 1. **MTF** ([`mtf`]), the Schroeder equation of cl. 6.1 on the band's energy response `E(t)`,
//!    the series the solver writes: `m_k(F) = |Σ E(t)·e^{-j2πFt}| / Σ E(t)`, from the bin of the
//!    direct sound's arrival, at the 14 modulation frequencies of A.2.2 ([`MODULATION_HZ`]). The
//!    noise factor of cl. 6.1 is applied in step 3, as A.5.3 note 2 adds noise to the masking
//!    correction's denominator; the two are the same factor, applied once.
//! 2. **Levels.** The speech signal in band k at the receiver is Table A.4's spectrum at
//!    [`SPEECH_LEVEL_DBA_AT_1M`] (J.3: 60 dB(A) at 1 m, on axis), carried to the receiver by the
//!    room's own transfer: the band's SPL at the receiver less the same source's free-field level
//!    at 1 m ([`free_field_level_at_db`], `params::level`'s free field, as G uses it). The noise is
//!    the receiver's background noise in the band, when it has one.
//! 3. **Corrections** (A.3, A.5.3): `m'_k(F) = m_k(F)·I_k / (I_k + I_n,k + I_am,k + I_rt,k)`, with
//!    `I_k` the speech intensity, `I_n,k` the noise's, `I_rt,k` the reception threshold's (Table
//!    A.2, [`RECEPTION_THRESHOLD_DB`]) and `I_am,k = I_{k-1}·10^{amdB/10}`, the masking by **band
//!    k-1's** total level, speech and noise (Table A.1, [`masking_db`]). The 125 Hz band is not
//!    masked.
//! 4. `SNR_eff = 10·lg(m'/(1-m'))` limited to ±15 dB (A.5.4), `TI = (SNR_eff + 15)/30` (A.5.5),
//!    `MTI_k` the mean of the 14 TIs, and `STI = Σ α_k·MTI_k − Σ β_k·√(MTI_k·MTI_{k+1})`
//!    (A.5.6, Table A.3, [`Gender`]), truncated at 1.0 (Table A.3, note).
//!
//! Male and female are both computed (A.3.4: male is the one used to assess a channel, and the
//! one shown); female speech has no 125 Hz band (Tables A.3 and A.4).

use schemars::JsonSchema;
use serde::Serialize;

use super::{NotEvaluable, ParamError, Quantity, not_evaluable};

/// The 14 modulation frequencies, Hz (IEC 60268-16:2011 A.2.2).
pub const MODULATION_HZ: [f64; 14] = [
    0.63, 0.8, 1.0, 1.25, 1.6, 2.0, 2.5, 3.15, 4.0, 5.0, 6.3, 8.0, 10.0, 12.5,
];

/// The seven octave bands STI is defined on, Hz.
pub const OCTAVES_HZ: [i32; 7] = [125, 250, 500, 1000, 2000, 4000, 8000];

/// The absolute speech reception threshold per octave, dB SPL (Table A.2).
pub const RECEPTION_THRESHOLD_DB: [f64; 7] = [46.0, 27.0, 12.0, 6.5, 7.5, 8.0, 12.0];

/// The test speech level, dB(A) at 1 m on the talker's axis (Annex J.3; 70 for raised effort).
pub const SPEECH_LEVEL_DBA_AT_1M: f64 = 60.0;

/// The shortest response the MTF may be read from, s (cl. 6.2 b, 8.3 a): the period of the lowest
/// modulation frequency, 0.63 Hz. The response must also be at least half the reverberation time.
pub const MIN_RESPONSE_S: f64 = 1.6;

/// How far the energy a band's series may lack ([`ReceiverBand::unseen_share`]) may move the STI
/// before it is refused: 1/10 of 0.03, the tolerance the STI bed holds it to (IEC 60268-16:2011
/// gives no limen; edition 4's repeatability is 0.02 and its rating bands are 0.04 wide), the same
/// tenth the decay quantities hold their unknowns to (`decay::limits`).
pub const UNSEEN_LIMIT: f64 = 0.003;

/// A response whose level at its end lies this far below its loudest stretch has decayed through a
/// whole reverberation time's range, dB ([`end_decay`]); one that has not is taken as cut while
/// still decaying, and the decay its end shows is one the length check must meet.
pub const END_DECAY_DB: f64 = 60.0;

/// How many standard deviations of the windows' own count noise a difference between two windows
/// at a response's end must exceed to be read as a change of level ([`end_decay`]): 2, a one-sided
/// chance of about 2.3% that noise alone reads as one. A series with no scatter in its windows (a
/// noise-free one) has no margin: any rise or tie at its end is not decaying.
pub const END_NOISE_SIGMAS: f64 = 2.0;

/// The distance the speech level is given at, m (Annex J.3).
pub const SPEECH_DISTANCE_M: f64 = 1.0;

/// The talker: male or female speech (A.3.4, A.3.5).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Gender {
    Male,
    Female,
}

impl Gender {
    /// The weighting factors `α_k` on [`OCTAVES_HZ`] (Table A.3); `None` where the band is not
    /// part of this speech (female 125 Hz).
    pub fn alpha(self) -> [Option<f64>; 7] {
        match self {
            Gender::Male => [0.085, 0.127, 0.230, 0.233, 0.309, 0.224, 0.173].map(Some),
            Gender::Female => [
                None,
                Some(0.117),
                Some(0.223),
                Some(0.216),
                Some(0.328),
                Some(0.250),
                Some(0.194),
            ],
        }
    }

    /// The redundancy factors `β_k` between [`OCTAVES_HZ`]`[k]` and `[k+1]` (Table A.3).
    pub fn beta(self) -> [Option<f64>; 6] {
        match self {
            Gender::Male => [0.085, 0.078, 0.065, 0.011, 0.047, 0.095].map(Some),
            Gender::Female => [
                None,
                Some(0.099),
                Some(0.066),
                Some(0.062),
                Some(0.025),
                Some(0.076),
            ],
        }
    }

    /// The speech spectrum on [`OCTAVES_HZ`], dB relative to the A-weighted speech level (Table
    /// A.4); `None` where this speech has no band.
    pub fn spectrum_db(self) -> [Option<f64>; 7] {
        match self {
            Gender::Male => [2.9, 2.9, -0.8, -6.8, -12.8, -18.8, -24.8].map(Some),
            Gender::Female => [
                None,
                Some(5.3),
                Some(-1.9),
                Some(-9.1),
                Some(-15.8),
                Some(-16.7),
                Some(-18.0),
            ],
        }
    }

    /// The octave bands this speech needs: 125 Hz to 8 kHz, female 250 Hz to 8 kHz.
    pub fn bands_hz(self) -> &'static [i32] {
        match self {
            Gender::Male => &OCTAVES_HZ,
            Gender::Female => &OCTAVES_HZ[1..],
        }
    }

    /// The speech level in the octave `freq_hz` at the receiver, dB SPL: Table A.4's band level at
    /// [`SPEECH_LEVEL_DBA_AT_1M`], plus the room's transfer from the free field at 1 m,
    /// `transfer_db`. `None` for a band this speech does not have.
    pub fn speech_db(self, freq_hz: i32, transfer_db: f64) -> Option<f64> {
        let k = octave_index(freq_hz)?;
        self.spectrum_db()[k].map(|s| SPEECH_LEVEL_DBA_AT_1M + s + transfer_db)
    }
}

/// The position of `freq_hz` in [`OCTAVES_HZ`].
pub fn octave_index(freq_hz: i32) -> Option<usize> {
    OCTAVES_HZ.iter().position(|&f| f == freq_hz)
}

/// The free field's level at `r_m` of the source whose power times `ρc` is `power_rho_c`, dB
/// against the reference SPL divides by (`params::level::free_field_pa2`). STI's transfer is the
/// band's SPL less this at [`SPEECH_DISTANCE_M`]. Refused as [`super::level::free_field_level_db`]
/// is: `params_no_energy` for no power, `params_bad_energy` for a negative or non-finite one.
pub fn free_field_level_at_db(power_rho_c: f64, r_m: f64) -> Result<f64, ParamError> {
    // The same checks and reference as G's free field, moved from 10 m to `r_m`.
    let at_10 = super::level::free_field_level_db(power_rho_c)?;
    Ok(at_10 + 20.0 * (super::level::G_DISTANCE_M / r_m).log10())
}

/// The modulation transfer function of one band's energy response (cl. 6.1, the Schroeder
/// equation's first factor): `m(F) = |Σ_i E_i·e^{-j2πF·t_i}| / Σ_i E_i` over the bins from `from`
/// on, `t_i = (i − from)·dt`, at each of [`MODULATION_HZ`]. Each bin's energy is placed at its
/// start; a constant shift of every `t_i` does not change `|·|`, and spreading each bin over its
/// width would multiply every `m(F)` by `sinc(πF·dt)`, 0.9997 at 12.5 Hz and 1 ms, which is left
/// out. Refused `params_bad_time_step`, `params_bad_energy` (a value not finite or negative), or
/// `params_no_energy` (nothing from `from` on).
pub fn mtf(energy: &[f64], dt: f64, from: usize) -> Result<[f64; 14], ParamError> {
    if !dt.is_finite() || dt <= 0.0 {
        return Err(ParamError::BadTimeStep { dt });
    }
    if let Some((index, &value)) = energy
        .iter()
        .enumerate()
        .find(|(_, v)| !v.is_finite() || **v < 0.0)
    {
        return Err(ParamError::BadEnergy { index, value });
    }
    let tail = energy.get(from..).unwrap_or(&[]);
    let total: f64 = tail.iter().sum();
    if total <= 0.0 {
        return Err(ParamError::NoEnergy);
    }
    Ok(MODULATION_HZ.map(|f| {
        let w = 2.0 * std::f64::consts::PI * f * dt;
        let (mut re, mut im) = (0.0, 0.0);
        for (i, e) in tail.iter().enumerate() {
            if *e > 0.0 {
                let (s, c) = (w * i as f64).sin_cos();
                re += e * c;
                im -= e * s;
            }
        }
        re.hypot(im) / total
    }))
}

/// The auditory masking of band k by band k-1, dB, from band k-1's total level `l_db` (Table
/// A.1): `0.5·L − 65` below 63 dB, `1.8·L − 146.9` from 63 to 67, `0.5·L − 59.8` from 67 to 100,
/// and −10 from 100 on.
pub fn masking_db(l_db: f64) -> f64 {
    if l_db < 63.0 {
        0.5 * l_db - 65.0
    } else if l_db < 67.0 {
        1.8 * l_db - 146.9
    } else if l_db < 100.0 {
        0.5 * l_db - 59.8
    } else {
        -10.0
    }
}

/// One octave band's inputs to [`sti`].
#[derive(Clone, Debug, PartialEq)]
pub struct LevelBand {
    pub freq_hz: i32,
    /// The room's MTF ([`mtf`]), before the level corrections.
    pub mtf: [f64; 14],
    /// The speech level at the receiver, dB SPL; `None` where the speech has no band (female
    /// 125 Hz).
    pub speech_db: Option<f64>,
    /// The background noise at the receiver, dB SPL; `None` for no noise.
    pub noise_db: Option<f64>,
}

/// An STI and the MTI it was summed from.
#[derive(Clone, Debug, PartialEq, Serialize, JsonSchema)]
pub struct StiValue {
    /// The STI, truncated at 1.0.
    pub value: f64,
    /// `Σα·MTI − Σβ·√(MTI·MTI)` before the truncation.
    pub untruncated: f64,
    /// `(freq_hz, MTI_k)`, one per band of the speech, ascending.
    pub mti: Vec<(i32, f64)>,
}

/// `10^(db/10)`, 0 for no level.
fn intensity(db: Option<f64>) -> f64 {
    db.map_or(0.0, |l| 10f64.powf(l / 10.0))
}

/// The transmission index of a corrected modulation transfer ratio (A.5.4, A.5.5).
fn transmission_index(m: f64) -> f64 {
    let snr = if m >= 1.0 {
        15.0
    } else if m <= 0.0 {
        -15.0
    } else {
        (10.0 * (m / (1.0 - m)).log10()).clamp(-15.0, 15.0)
    };
    (snr + 15.0) / 30.0
}

/// The STI of `gender`'s speech from `bands` (steps 3 and 4 of the module's text). Refused
/// `params_not_evaluable` (`band_missing`) when a band the speech needs is not in `bands`, or has
/// no speech level.
pub fn sti(bands: &[LevelBand], gender: Gender) -> Result<StiValue, ParamError> {
    check_bands(bands, gender)?;
    let mti = mtis(bands, gender, &|_| 0.0, Reading::Given);
    let raw = combine(&mti, &mti, gender);
    Ok(StiValue {
        value: raw.min(1.0),
        untruncated: raw,
        mti,
    })
}

/// The lowest and highest STI (each truncated at 1.0) that `bands` can give when each band's
/// series may lack energy, `unseen(freq_hz)` of what it holds from the direct sound on (0 for
/// none): energy the solver did not record (particles alive at the run's end, dropped at its
/// floor, or lost), arriving at any time. Such an addition `X = x·Σ E` changes the Schroeder sum
/// by a phasor of modulus at most `X`, so every `m(F)` lies in `[(m − x)/(1 + x), (m + x)/(1 + x)]`,
/// at 0.63 Hz as at every modulation frequency; the band's speech level rises by at most
/// `10·lg(1 + x)`, which raises its own correction factor and band k+1's masking. Each `MTI_k` is
/// then taken at both ends, and `Σα·MTI − Σβ·√(MTI·MTI)` bounded term by term: its lowest with
/// every `α` term low and every `β` term high, its highest the other way. The range is exact for
/// the shares it is given; the shares a run gives are estimates ([`ReceiverBand::unseen_share`]).
/// Refused as [`sti`] is.
pub fn sti_unseen_range(
    bands: &[LevelBand],
    gender: Gender,
    unseen: &dyn Fn(i32) -> f64,
) -> Result<(f64, f64), ParamError> {
    check_bands(bands, gender)?;
    let lo = mtis(bands, gender, unseen, Reading::Lowest);
    let hi = mtis(bands, gender, unseen, Reading::Highest);
    Ok((
        combine(&lo, &hi, gender).min(1.0),
        combine(&hi, &lo, gender).min(1.0),
    ))
}

/// `band_missing` when a band `gender`'s speech needs is not in `bands`, or has no speech level.
fn check_bands(bands: &[LevelBand], gender: Gender) -> Result<(), ParamError> {
    let needed = gender.bands_hz();
    for &f in needed {
        if bands
            .iter()
            .find(|b| b.freq_hz == f)
            .and_then(|b| b.speech_db)
            .is_none()
        {
            return Err(not_evaluable(
                Quantity::Sti,
                NotEvaluable::BandMissing {
                    freq_hz: f,
                    needed_hz: needed.to_vec(),
                },
            ));
        }
    }
    Ok(())
}

/// How [`mtis`] reads the bands: as given, or at the lowest or highest the energy each band's
/// series may lack can take its MTI ([`sti_unseen_range`]).
#[derive(Clone, Copy, PartialEq, Eq)]
enum Reading {
    Given,
    Lowest,
    Highest,
}

/// `(freq_hz, MTI_k)` of every band `gender`'s speech needs, ascending, read as `reading` says.
/// The bands are checked ([`check_bands`]).
fn mtis(
    bands: &[LevelBand],
    gender: Gender,
    unseen: &dyn Fn(i32) -> f64,
    reading: Reading,
) -> Vec<(i32, f64)> {
    let find = |f: i32| bands.iter().find(|b| b.freq_hz == f);
    let lack = |f: i32| {
        if reading == Reading::Given {
            0.0
        } else {
            unseen(f)
        }
    };
    let mut mti = Vec::with_capacity(gender.bands_hz().len());
    for &f in gender.bands_hz() {
        let k = octave_index(f).expect("an octave of OCTAVES_HZ");
        let b = find(f).expect("checked by check_bands");
        let x = lack(f);
        let i_s = intensity(b.speech_db);
        let i_n = intensity(b.noise_db);
        let i_rt = 10f64.powf(RECEPTION_THRESHOLD_DB[k] / 10.0);
        // Masking by band k-1's total level, speech and noise (Table A.1); 125 Hz is not masked,
        // and a band k-1 not given (female's 125 Hz, when the run has none) masks nothing. Band
        // k-1's unseen energy can raise its speech level; the masking is taken at its least and
        // most over that range ([`masking_range`]).
        let (am_given, am_least, am_most) = match k.checked_sub(1).and_then(|p| find(OCTAVES_HZ[p]))
        {
            Some(prev) => {
                let i_noise = intensity(prev.noise_db);
                let i_speech = intensity(prev.speech_db);
                let given = i_speech + i_noise;
                let (least, most) =
                    masking_range(given, i_speech * (1.0 + lack(prev.freq_hz)) + i_noise);
                (masking_intensity(given), least, most)
            }
            None => (0.0, 0.0, 0.0),
        };
        let factor = match reading {
            Reading::Given => i_s / (i_s + i_n + am_given + i_rt),
            Reading::Lowest => i_s / (i_s + i_n + am_most + i_rt),
            Reading::Highest => {
                let lifted = i_s * (1.0 + x);
                lifted / (lifted + i_n + am_least + i_rt)
            }
        };
        let sum: f64 = b
            .mtf
            .iter()
            .map(|&m| {
                let m = match reading {
                    Reading::Given => m,
                    Reading::Lowest => ((m - x) / (1.0 + x)).max(0.0),
                    Reading::Highest => ((m + x) / (1.0 + x)).min(1.0),
                };
                transmission_index(m * factor)
            })
            .sum();
        mti.push((f, sum / MODULATION_HZ.len() as f64));
    }
    mti
}

/// `I_am`, the masking intensity band k-1 at total intensity `i_prev` puts on band k (Table A.1):
/// `I_{k-1}·10^{amdB/10}`, 0 for no level.
fn masking_intensity(i_prev: f64) -> f64 {
    if i_prev > 0.0 {
        i_prev * 10f64.powf(masking_db(10.0 * i_prev.log10()) / 10.0)
    } else {
        0.0
    }
}

/// The least and the most masking intensity ([`masking_intensity`]) band k-1 can put on band k
/// while its total intensity lies anywhere from `i_given` to `i_lifted` (`i_lifted ≥ i_given`).
/// `I_am` rises with the level everywhere but at 100 dB, where Table A.1 steps down from −9.8 to
/// −10 dB: across that step it is most just below 100 dB and least at 100 dB, neither of them an
/// end, so a range straddling 100 dB takes both.
fn masking_range(i_given: f64, i_lifted: f64) -> (f64, f64) {
    let (a, b) = (masking_intensity(i_given), masking_intensity(i_lifted));
    let (mut least, mut most) = (a.min(b), a.max(b));
    let level = |i: f64| {
        if i > 0.0 {
            10.0 * i.log10()
        } else {
            f64::NEG_INFINITY
        }
    };
    const STEP_DB: f64 = 100.0;
    if level(i_given) < STEP_DB && level(i_lifted) >= STEP_DB {
        let at_step = 10f64.powf(STEP_DB / 10.0);
        // The limit from below, 0.5·100 − 59.8 = −9.8 dB, and the value at 100 dB, −10 dB.
        most = most.max(at_step * 10f64.powf((0.5 * STEP_DB - 59.8) / 10.0));
        least = least.min(at_step * 10f64.powf(masking_db(STEP_DB) / 10.0));
    }
    (least, most)
}

/// `Σ α_k·plus_k − Σ β_k·√(minus_k·minus_{k+1})` (A.5.6, Table A.3), untruncated: `plus` and
/// `minus` the same MTIs for the STI itself, opposite ends of their ranges for its bounds.
fn combine(plus: &[(i32, f64)], minus: &[(i32, f64)], gender: Gender) -> f64 {
    let (alpha, beta) = (gender.alpha(), gender.beta());
    let mut raw = 0.0;
    for (j, &(f, m)) in plus.iter().enumerate() {
        let k = octave_index(f).expect("an octave");
        raw += alpha[k].expect("a band of this speech") * m;
        if let (Some(&(_, a)), Some(&(_, b))) = (minus.get(j), minus.get(j + 1)) {
            raw -= beta[k].expect("a pair of this speech") * (a * b).sqrt();
        }
    }
    raw
}

/// What the end of a band's response shows about its decay ([`end_decay`]).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum EndDecay {
    /// The response has ended: its last window holds nothing, or lies at least
    /// [`END_DECAY_DB`] below its loudest.
    Ended,
    /// Still within [`END_DECAY_DB`] of its loudest at its end, and decaying there with this
    /// reverberation time, s, read over the shortest span its noise resolves; `drop_db` is how far
    /// the last window lies below the loudest.
    Decaying { t_s: f64, drop_db: f64 },
    /// Still within [`END_DECAY_DB`] of its loudest at its end, and not decaying there: its last
    /// window level with or above the one before beyond their count noise (any tie, for a
    /// noise-free series), or no earlier window louder than it beyond that noise.
    NotDecaying { drop_db: f64 },
}

/// The decay at the end of a band's response, `energy` from bin `from` on, for cl. 6.2 b and
/// 8.3 a's length rule. The windows are the tail estimate's (`decay::tail`): `w`, a tenth of the
/// bins from `from` to the series' end, at least 1; but they end at the series' end, not at its
/// last bin with energy, so that a response that has ended shows it. The level of the last window
/// is compared with the loudest `w`-bin window from `from` on; within [`END_DECAY_DB`] of it, the
/// decay rate at the end is read against the series' own count noise ([`window_variance`],
/// [`END_NOISE_SIGMAS`] `= z`): when the last window `w₂` exceeds the one before, `w₁`, by at
/// least `z·σ(w₁ − w₂)`, the response is not decaying; otherwise the rate is read over `k`
/// windows, `T = 60·k·w·dt / (10·lg(a_k/w₂))`, `a_k` the nearest earlier window that exceeds `w₂`
/// by more than `z·σ(a_k − w₂)`, and none is not decaying. A noise-free series reads as it did
/// before the margin: `k = 1` whenever `w₁ > w₂`, not decaying otherwise. Sparse late particles
/// can tie or swap the last two windows by count noise alone, 40 to 55 dB down, which a strict
/// `w₂ ≥ w₁` read as not decaying; a series cut short (a window empty or [`END_DECAY_DB`] down
/// still counts as ended) or flat at its end is read as before. `None` when fewer than two bins
/// follow `from`.
pub fn end_decay(energy: &[f64], dt: f64, from: usize) -> Option<EndDecay> {
    let v = energy.get(from..)?;
    let n = v.len();
    if n < 2 {
        return None;
    }
    let w = (n / 10).max(1);
    let mut acc: f64 = v[..w].iter().sum();
    let mut loudest = acc;
    for j in w..n {
        acc += v[j] - v[j - w];
        loudest = loudest.max(acc);
    }
    // The window of `w` bins ending `k` windows before the series' end: its sum, and the
    // variance of that sum from its own scatter ([`window_variance`]).
    let window = |k: usize| {
        let s = &v[n - (k + 1) * w..n - k * w];
        (s.iter().sum::<f64>(), window_variance(s))
    };
    let (w2, var2) = window(0);
    if w2 <= 0.0 {
        return Some(EndDecay::Ended);
    }
    let drop_db = 10.0 * (loudest / w2).log10();
    if drop_db >= END_DECAY_DB {
        return Some(EndDecay::Ended);
    }
    let (w1, var1) = window(1);
    let z = END_NOISE_SIGMAS;
    // Level or rising at the end beyond what its own noise explains: not decaying.
    if w2 - w1 >= z * (var1 + var2).sqrt() {
        return Some(EndDecay::NotDecaying { drop_db });
    }
    // The decay over the shortest span ending at the series' end that its noise resolves: the
    // nearest earlier window louder than the last by more than `z` of their difference's noise.
    // A noise-free series resolves at the window before the last, as it always has.
    for k in 1..n / w {
        let (a, var_a) = window(k);
        if a - w2 > z * (var_a + var2).sqrt() {
            return Some(EndDecay::Decaying {
                t_s: 60.0 * (k * w) as f64 * dt / (10.0 * (a / w2).log10()),
                drop_db,
            });
        }
    }
    Some(EndDecay::NotDecaying { drop_db })
}

/// The variance of the sum of the bins `s`, from their own scatter: `|s|` times the bins'
/// variance, estimated from successive differences, `Σ(v_i − v_{i−1})² / (2·(|s| − 1))` (the
/// von Neumann estimate, which a smooth decay over the window barely enters, where the scatter
/// about the window's mean would count the decay itself as noise). Bins of a particle series hold
/// independent counts of deposits; a window of sparse deposits of one size gives, for `c` of them
/// isolated, about `c·d²`, the Poisson variance. 0 for a window of one bin.
fn window_variance(s: &[f64]) -> f64 {
    if s.len() < 2 {
        return 0.0;
    }
    let ss: f64 = s.windows(2).map(|p| (p[1] - p[0]).powi(2)).sum();
    s.len() as f64 * ss / (2.0 * (s.len() - 1) as f64)
}

/// One octave band of a receiver, as [`receiver_sti`] takes it.
#[derive(Clone, Debug, PartialEq)]
pub struct ReceiverBand<'a> {
    pub freq_hz: i32,
    /// The band's energy series, Pa² per step, as the solver wrote it.
    pub energy: &'a [f64],
    /// The bin of the direct sound's arrival: the MTF is read from it on.
    pub from: usize,
    /// The band's SPL at the receiver, dB; or its refusal, in words.
    pub spl_db: Result<f64, String>,
    /// The sources' power in the band times `ρc`, Pa²·m² (the `.gap`), whose free field at 1 m the
    /// SPL is taken against.
    pub power_rho_c: f64,
    /// The receiver's background noise in the band, dB SPL; `None` for none.
    pub noise_db: Option<f64>,
    /// The band's reverberation time, s, that the run's length is checked against (cl. 8.3 a);
    /// `None` when the band has none.
    pub reverberation_s: Option<f64>,
    /// Why the band's series cannot be read honestly (refused, or not complete with nothing to
    /// bound what it lacks), if it cannot.
    pub unusable: Option<String>,
    /// An estimate of the most energy the band's series can lack, as a share of what it holds from
    /// `from` on: what the particles alive at the run's end can still bring, what the solver's
    /// floor dropped, and what lost particles took (`results::report` estimates each as the decay
    /// quantities do; the alive particles' part assumes they bring no more per unit of energy than
    /// the particles alive over the decay did, a heuristic, not a proof). 0 for none. STI is
    /// refused when it can move the value by more than [`UNSEEN_LIMIT`] ([`sti_unseen_range`]); a
    /// share that is not a finite number of at least 0 makes the band unusable.
    pub unseen_share: f64,
}

/// What [`receiver_sti`] read from one band.
#[derive(Clone, Debug, PartialEq, Serialize, JsonSchema)]
pub struct BandSti {
    pub freq_hz: i32,
    /// The room's MTF at [`MODULATION_HZ`], before the level corrections; `null` when the band
    /// cannot give one.
    pub mtf: Option<Vec<f64>>,
    /// The band's SPL less the same sources' free-field level at 1 m, dB: what carries the speech
    /// from 1 m on axis to the receiver. `null` when the band cannot give one.
    pub transfer_db: Option<f64>,
    /// The male speech level at the receiver, dB SPL (Table A.4 at 60 dB(A) at 1 m, plus
    /// `transfer_db`).
    pub speech_male_db: Option<f64>,
    /// The female speech level at the receiver, dB SPL; `null` at 125 Hz, which female speech has
    /// not.
    pub speech_female_db: Option<f64>,
    /// The background noise applied, dB SPL; `null` for none.
    pub noise_db: Option<f64>,
    /// `MTI_k` of male speech; `null` when the male STI is refused.
    pub mti_male: Option<f64>,
    /// `MTI_k` of female speech; `null` at 125 Hz, or when the female STI is refused.
    pub mti_female: Option<f64>,
}

/// A receiver's STI, both speeches, and what each band gave.
#[derive(Clone, Debug, PartialEq)]
pub struct ReceiverSti {
    pub male: Result<StiValue, ParamError>,
    pub female: Result<StiValue, ParamError>,
    /// One per octave of [`OCTAVES_HZ`] the run computed, ascending.
    pub bands: Vec<BandSti>,
}

/// The STI of a receiver from its bands (the module's text, steps 1 to 4), male and female. `dt`
/// is the series' time step; `octave` says whether the run's bands are octave bands (a
/// third-octave band at 125 Hz is not the octave STI needs). Each speech is refused
/// `params_not_evaluable` when the bands are not octaves (`not_octave_bands`), when a band it
/// needs is not in the run (`band_missing`), or cannot be read (`band_refused`: its series is
/// refused or not complete with nothing to bound what it lacks, its SPL is refused, no source
/// emits in it, it has no reverberation time to check the run's length against, its response is
/// not decaying at its end, or what its series can lack moves the STI by more than
/// [`UNSEEN_LIMIT`]); and `params_series_too_short` when the response, from the arrival, is
/// shorter than [`MIN_RESPONSE_S`] or half the longest reverberation time of its bands (cl. 6.2 b,
/// 8.3 a), each band's taken as the larger of its T30 (else T20, else EDT) and the decay time its
/// response's end shows ([`end_decay`]). Female speech is also refused when the run's 125 Hz band
/// cannot be read and the receiver has noise in it: that noise masks 250 Hz (Table A.1).
pub fn receiver_sti(dt: f64, bands: &[ReceiverBand<'_>], octave: bool) -> ReceiverSti {
    // What each octave band of the run gives: its MTF and transfer, or why it gives none.
    struct Read {
        freq_hz: i32,
        k: usize,
        got: Result<([f64; 14], f64), String>,
        noise_db: Option<f64>,
        reverberation_s: Option<f64>,
        end: Option<EndDecay>,
        unseen_share: f64,
        length_s: f64,
    }
    let mut read: Vec<Read> = Vec::new();
    for b in bands {
        let Some(k) = octave_index(b.freq_hz) else {
            continue;
        };
        let got = (|| {
            if let Some(why) = &b.unusable {
                return Err(why.clone());
            }
            if !(b.unseen_share.is_finite() && b.unseen_share >= 0.0) {
                return Err(format!(
                    "nothing bounds the energy its series can lack (share {})",
                    b.unseen_share
                ));
            }
            let spl = b
                .spl_db
                .clone()
                .map_err(|e| format!("its SPL is refused: {e}"))?;
            let free = free_field_level_at_db(b.power_rho_c, SPEECH_DISTANCE_M)
                .map_err(|e| format!("no free field to take its SPL against: {e}"))?;
            let m = mtf(b.energy, dt, b.from).map_err(|e| e.to_string())?;
            Ok((m, spl - free))
        })();
        read.push(Read {
            freq_hz: b.freq_hz,
            k,
            got,
            noise_db: b.noise_db,
            reverberation_s: b.reverberation_s,
            end: end_decay(b.energy, dt, b.from),
            unseen_share: b.unseen_share,
            length_s: b.energy.len().saturating_sub(b.from) as f64 * dt,
        });
    }
    read.sort_by_key(|r| r.k);

    let evaluate = |gender: Gender| -> Result<StiValue, ParamError> {
        let refuse = |why: NotEvaluable| Err(not_evaluable(Quantity::Sti, why));
        if !octave {
            return refuse(NotEvaluable::NotOctaveBands {
                bands_hz: bands.iter().map(|b| b.freq_hz).collect(),
            });
        }
        let needed = gender.bands_hz();
        for &f in needed {
            if !read.iter().any(|r| r.freq_hz == f) {
                return refuse(NotEvaluable::BandMissing {
                    freq_hz: f,
                    needed_hz: needed.to_vec(),
                });
            }
        }
        let used: Vec<&Read> = read
            .iter()
            .filter(|r| needed.contains(&r.freq_hz))
            .collect();
        for r in &used {
            if let Err(detail) = &r.got {
                return refuse(NotEvaluable::BandRefused {
                    freq_hz: r.freq_hz,
                    detail: detail.clone(),
                });
            }
        }
        // A band k-1 outside the speech (female's 125 Hz) masks band k with its noise: when it
        // cannot be read and has noise, band k cannot be masked honestly (backlog 66).
        for r in &used {
            let Some(prev) =
                r.k.checked_sub(1)
                    .and_then(|p| read.iter().find(|q| q.k == p))
            else {
                continue;
            };
            if let (false, Err(detail), Some(_)) =
                (needed.contains(&prev.freq_hz), &prev.got, prev.noise_db)
            {
                return refuse(NotEvaluable::BandRefused {
                    freq_hz: prev.freq_hz,
                    detail: format!(
                        "{detail}; its background noise masks the {} Hz band (IEC 60268-16:2011 \
                         Table A.1), which is not taken unmasked",
                        r.freq_hz
                    ),
                });
            }
        }
        // The length (cl. 6.2 b, 8.3 a): each band's reverberation time is the larger of its T30
        // (else T20, else EDT) and the decay its response's end shows, since a response cut while
        // it still decays bends its own Schroeder curve down and reads a T30 too short.
        let mut longest: f64 = 0.0;
        for r in &used {
            let t = match r.reverberation_s {
                Some(t) if t.is_finite() && t > 0.0 => t,
                _ => {
                    return refuse(NotEvaluable::BandRefused {
                        freq_hz: r.freq_hz,
                        detail: "it has no reverberation time (T30, T20 or EDT) to check the \
                                 run's length against (IEC 60268-16:2011 cl. 8.3 a)"
                            .into(),
                    });
                }
            };
            let t = match r.end {
                Some(EndDecay::Decaying { t_s, .. }) => t.max(t_s),
                Some(EndDecay::NotDecaying { drop_db }) => {
                    return refuse(NotEvaluable::BandRefused {
                        freq_hz: r.freq_hz,
                        detail: format!(
                            "its response is not decaying at its end, {drop_db:.1} dB below its \
                             loudest (less than {END_DECAY_DB} dB): no reverberation time says \
                             how long it must be (IEC 60268-16:2011 cl. 6.2 b, 8.3 a)"
                        ),
                    });
                }
                Some(EndDecay::Ended) | None => t,
            };
            longest = longest.max(t);
        }
        let needed_s = MIN_RESPONSE_S.max(longest / 2.0);
        let available_s = used
            .iter()
            .map(|r| r.length_s)
            .fold(f64::INFINITY, f64::min);
        if available_s < needed_s {
            return Err(ParamError::SeriesTooShort {
                what: format!(
                    "STI (IEC 60268-16:2011 cl. 6.2 b, 8.3 a: at least {MIN_RESPONSE_S} s and half \
                     the reverberation time, {longest:.3} s, the larger of each band's T30, T20 or \
                     EDT and the decay its response's end shows, from the direct sound)"
                ),
                needed_s,
                available_s,
            });
        }
        // Every octave of the run, so that a band k-1 outside the speech (female's 125 Hz) still
        // masks band k with its noise.
        let levels: Vec<LevelBand> = read
            .iter()
            .filter_map(|r| {
                let (m, transfer) = r.got.as_ref().ok()?;
                Some(LevelBand {
                    freq_hz: r.freq_hz,
                    mtf: *m,
                    speech_db: gender.speech_db(r.freq_hz, *transfer),
                    noise_db: r.noise_db,
                })
            })
            .collect();
        let value = sti(&levels, gender)?;
        // What the series can lack (particles alive at the end, dropped at the floor, lost),
        // bounded to its worst effect on the STI.
        let lack = |f: i32| {
            read.iter()
                .find(|r| r.freq_hz == f)
                .map_or(0.0, |r| r.unseen_share)
        };
        if levels.iter().any(|b| lack(b.freq_hz) > 0.0) {
            let (lo, hi) = sti_unseen_range(&levels, gender, &lack)?;
            let moves = (value.value - lo).max(hi - value.value);
            if moves > UNSEEN_LIMIT {
                let worst = *needed
                    .iter()
                    .max_by(|a, b| lack(**a).total_cmp(&lack(**b)))
                    .expect("a speech has bands");
                return refuse(NotEvaluable::BandRefused {
                    freq_hz: worst,
                    detail: format!(
                        "the energy its series can lack (particles alive when the run ended, \
                         dropped at the solver's floor, or lost), estimated at up to {:.3e} of \
                         what it holds from the direct sound on, can move the STI by up to \
                         {moves:.4}, more than {UNSEEN_LIMIT} (1/10 of the 0.03 STI is held \
                         to); the estimate is a heuristic, not a proof: it assumes particles \
                         alive at the end bring no more per unit of their energy than those \
                         alive over the decay did",
                        lack(worst)
                    ),
                });
            }
        }
        Ok(value)
    };
    let male = evaluate(Gender::Male);
    let female = evaluate(Gender::Female);
    let mti_of = |s: &Result<StiValue, ParamError>, f: i32| {
        s.as_ref()
            .ok()
            .and_then(|v| v.mti.iter().find(|(g, _)| *g == f).map(|(_, m)| *m))
    };
    let bands = read
        .iter()
        .map(|r| {
            let ok = r.got.as_ref().ok();
            let transfer = ok.map(|(_, t)| *t);
            BandSti {
                freq_hz: r.freq_hz,
                mtf: ok.map(|(m, _)| m.to_vec()),
                transfer_db: transfer,
                speech_male_db: transfer.and_then(|t| Gender::Male.speech_db(r.freq_hz, t)),
                speech_female_db: transfer.and_then(|t| Gender::Female.speech_db(r.freq_hz, t)),
                noise_db: r.noise_db,
                mti_male: mti_of(&male, r.freq_hz),
                mti_female: mti_of(&female, r.freq_hz),
            }
        })
        .collect();
    ReceiverSti {
        male,
        female,
        bands,
    }
}
