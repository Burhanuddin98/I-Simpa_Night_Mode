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
    let needed = gender.bands_hz();
    let find = |f: i32| bands.iter().find(|b| b.freq_hz == f);
    for &f in needed {
        if find(f).and_then(|b| b.speech_db).is_none() {
            return Err(not_evaluable(
                Quantity::Sti,
                NotEvaluable::BandMissing {
                    freq_hz: f,
                    needed_hz: needed.to_vec(),
                },
            ));
        }
    }
    let mut mti = Vec::with_capacity(needed.len());
    for &f in needed {
        let k = octave_index(f).expect("an octave of OCTAVES_HZ");
        let b = find(f).expect("checked above");
        let i_s = intensity(b.speech_db);
        let i_n = intensity(b.noise_db);
        let i_rt = 10f64.powf(RECEPTION_THRESHOLD_DB[k] / 10.0);
        // Masking by band k-1's total level, speech and noise (Table A.1); 125 Hz is not masked,
        // and a band k-1 not given (female's 125 Hz, when the run has none) masks nothing.
        let i_am = match k.checked_sub(1).and_then(|p| find(OCTAVES_HZ[p])) {
            Some(prev) => {
                let i_prev = intensity(prev.speech_db) + intensity(prev.noise_db);
                if i_prev > 0.0 {
                    i_prev * 10f64.powf(masking_db(10.0 * i_prev.log10()) / 10.0)
                } else {
                    0.0
                }
            }
            None => 0.0,
        };
        let factor = i_s / (i_s + i_n + i_am + i_rt);
        let sum: f64 = b.mtf.iter().map(|m| transmission_index(m * factor)).sum();
        mti.push((f, sum / MODULATION_HZ.len() as f64));
    }
    let (alpha, beta) = (gender.alpha(), gender.beta());
    let mut raw = 0.0;
    for (j, &(f, m)) in mti.iter().enumerate() {
        let k = octave_index(f).expect("an octave");
        raw += alpha[k].expect("a band of this speech") * m;
        if let Some(&(_, next)) = mti.get(j + 1) {
            raw -= beta[k].expect("a pair of this speech") * (m * next).sqrt();
        }
    }
    Ok(StiValue {
        value: raw.min(1.0),
        untruncated: raw,
        mti,
    })
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
    /// Why the band's series cannot be read honestly (refused, or not complete), if it cannot.
    pub unusable: Option<String>,
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
/// refused or not complete, its SPL is refused, no source emits in it, or it has no reverberation
/// time to check the run's length against); and `params_series_too_short` when the response,
/// from the arrival, is shorter than [`MIN_RESPONSE_S`] or half the longest reverberation time of
/// its bands (cl. 6.2 b, 8.3 a).
pub fn receiver_sti(dt: f64, bands: &[ReceiverBand<'_>], octave: bool) -> ReceiverSti {
    // What each octave band of the run gives: its MTF and transfer, or why it gives none.
    struct Read {
        freq_hz: i32,
        k: usize,
        got: Result<([f64; 14], f64), String>,
        noise_db: Option<f64>,
        reverberation_s: Option<f64>,
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
        let mut longest: f64 = 0.0;
        for r in &used {
            match r.reverberation_s {
                Some(t) if t.is_finite() && t > 0.0 => longest = longest.max(t),
                _ => {
                    return refuse(NotEvaluable::BandRefused {
                        freq_hz: r.freq_hz,
                        detail: "it has no reverberation time (T30, T20 or EDT) to check the \
                                 run's length against (IEC 60268-16:2011 cl. 8.3 a)"
                            .into(),
                    });
                }
            }
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
                     the reverberation time, {longest:.3} s, from the direct sound)"
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
        sti(&levels, gender)
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
