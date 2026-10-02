//! STI (`params::sti`) against what IEC 60268-16:2011 fixes exactly: the MTF of an exponential
//! decay, the limits m = 1 and m = 0, the masking of band k by band k-1 (Table A.1), female speech
//! without 125 Hz, the truncation at 1.0 (Table A.3, note), and the refusals of cl. 8.3.

use simpa_core::params::sti::{
    END_DECAY_DB, EndDecay, Gender, LevelBand, MODULATION_HZ, OCTAVES_HZ, ReceiverBand,
    UNSEEN_LIMIT, end_decay, masking_db, mtf, receiver_sti, sti, sti_unseen_range,
};
use simpa_core::params::{NotEvaluable, ParamError, codes, level};

/// `n` bins of an energy decay `e^{-13.8·t/T}` sampled every `dt`.
fn exponential(t60: f64, dt: f64, n: usize) -> Vec<f64> {
    (0..n)
        .map(|i| (-13.8 * i as f64 * dt / t60).exp())
        .collect()
}

#[test]
fn an_exponential_decay_has_the_closed_form_mtf() {
    // m(F) = 1/sqrt(1 + (2πF·T/13.8)²), exact for a continuous exponential decay; a 10 µs step
    // brings the sampled sum within 1e-6 of it.
    let dt = 1e-5;
    for t60 in [0.3, 1.0, 2.5] {
        let n = (8.0 * t60 / dt) as usize;
        let m = mtf(&exponential(t60, dt, n), dt, 0).unwrap();
        for (j, &f) in MODULATION_HZ.iter().enumerate() {
            let x = 2.0 * std::f64::consts::PI * f * t60 / 13.8;
            let exact = 1.0 / (1.0 + x * x).sqrt();
            assert!(
                (m[j] - exact).abs() < 1e-6,
                "T {t60} s, F {f} Hz: {} vs {exact}",
                m[j]
            );
        }
    }
}

#[test]
fn the_mtf_is_read_from_the_arrival_and_a_delay_does_not_change_it() {
    let dt = 1e-4;
    let decay = exponential(1.0, dt, 30_000);
    let mut delayed = vec![0.0; 137];
    delayed.extend_from_slice(&decay);
    let a = mtf(&decay, dt, 0).unwrap();
    let b = mtf(&delayed, dt, 137).unwrap();
    let c = mtf(&delayed, dt, 0).unwrap();
    for j in 0..14 {
        assert!((a[j] - b[j]).abs() < 1e-12 && (a[j] - c[j]).abs() < 1e-9);
    }
    // A single impulse transfers every modulation fully.
    let mut impulse = vec![0.0; 1000];
    impulse[10] = 3.0;
    assert!(
        mtf(&impulse, dt, 0)
            .unwrap()
            .iter()
            .all(|m| (m - 1.0).abs() < 1e-15)
    );
    // Refusals.
    assert_eq!(mtf(&[0.0; 5], dt, 0), Err(ParamError::NoEnergy));
    assert_eq!(mtf(&[1.0, 0.0], dt, 1), Err(ParamError::NoEnergy));
    assert_eq!(
        mtf(&[1.0], 0.0, 0).unwrap_err().code(),
        codes::BAD_TIME_STEP
    );
    assert_eq!(
        mtf(&[1.0, -1.0], dt, 0).unwrap_err().code(),
        codes::BAD_ENERGY
    );
}

/// Every octave at `level_db` with the same MTF `m` at every modulation frequency, no noise.
fn flat(level_db: f64, m: f64) -> Vec<LevelBand> {
    OCTAVES_HZ
        .iter()
        .map(|&f| LevelBand {
            freq_hz: f,
            mtf: [m; 14],
            speech_db: Some(level_db),
            noise_db: None,
        })
        .collect()
}

fn for_female(mut bands: Vec<LevelBand>) -> Vec<LevelBand> {
    bands[0].speech_db = None;
    bands
}

#[test]
fn full_modulation_at_a_high_level_gives_one_for_both_speeches() {
    // 80 dB in every band: masking by band k-1 is -19.8 dB, so m' = 0.9896 and SNR_eff = 19.8 dB,
    // clipped to 15: every TI is 1, and STI = Σα − Σβ = 1 (male 1.381 − 0.381, female 1.328 − 0.328).
    let male = sti(&flat(80.0, 1.0), Gender::Male).unwrap();
    assert!((male.value - 1.0).abs() < 1e-9, "{male:?}");
    assert!(
        (male.untruncated - 1.0).abs() < 1e-9,
        "not the truncation: {male:?}"
    );
    assert_eq!(male.mti.len(), 7);
    assert!(male.mti.iter().all(|(_, m)| (m - 1.0).abs() < 1e-12));
    let female = sti(&for_female(flat(80.0, 1.0)), Gender::Female).unwrap();
    assert!((female.value - 1.0).abs() < 1e-9, "{female:?}");
    assert!((female.untruncated - 1.0).abs() < 1e-9);
    // The tables themselves.
    let sum = |v: &[Option<f64>]| v.iter().flatten().sum::<f64>();
    assert!((sum(&Gender::Male.alpha()) - 1.381).abs() < 1e-12);
    assert!((sum(&Gender::Male.beta()) - 0.381).abs() < 1e-12);
    assert!((sum(&Gender::Female.alpha()) - 1.328).abs() < 1e-12);
    assert!((sum(&Gender::Female.beta()) - 0.328).abs() < 1e-12);
}

#[test]
fn no_modulation_gives_zero() {
    for gender in [Gender::Male, Gender::Female] {
        let mut bands = flat(80.0, 0.0);
        if gender == Gender::Female {
            bands = for_female(bands);
        }
        let s = sti(&bands, gender).unwrap();
        assert!(s.value.abs() < 1e-12, "{gender:?}: {s:?}");
    }
}

#[test]
fn band_k_is_masked_by_band_k_minus_1_s_level_not_its_own() {
    // 500 Hz at 100 dB masks 1 kHz at 40 dB: amdB = -10 at 100 dB, so I_am = 10^9 against
    // I = 10^4, m' ~ 1e-5 and TI = 0. With 1 kHz's own 40 dB the masking would be -45 dB and TI 1.
    let mut bands = flat(80.0, 1.0);
    bands[2].speech_db = Some(100.0);
    bands[3].speech_db = Some(40.0);
    let s = sti(&bands, Gender::Male).unwrap();
    let mti = |f: i32| s.mti.iter().find(|(g, _)| *g == f).unwrap().1;
    assert!(
        mti(1000) < 1e-9,
        "1 kHz under a 100 dB 500 Hz band: {}",
        mti(1000)
    );
    // 2 kHz at 80 dB above a 40 dB 1 kHz band is unmasked.
    assert!((mti(2000) - 1.0).abs() < 1e-12);

    // An intermediate case by hand: band k-1 at 70 dB, band k at 50 dB, m = 1.
    let mut bands = flat(80.0, 1.0);
    bands[2].speech_db = Some(70.0);
    bands[3].speech_db = Some(50.0);
    let s = sti(&bands, Gender::Male).unwrap();
    let mti = s.mti.iter().find(|(g, _)| *g == 1000).unwrap().1;
    let i_am = 1e7 * 10f64.powf((0.5 * 70.0 - 59.8) / 10.0);
    let i_rt = 10f64.powf(0.65);
    let m = 1e5 / (1e5 + i_am + i_rt);
    let ti = (10.0 * (m / (1.0 - m)).log10() + 15.0) / 30.0;
    assert!((mti - ti).abs() < 1e-12, "{mti} vs {ti}");
    assert!((ti - 0.65997).abs() < 1e-4, "{ti}");
}

#[test]
fn the_masking_table_is_followed_as_printed() {
    assert_eq!(masking_db(40.0), -45.0);
    assert!((masking_db(63.0) - (1.8 * 63.0 - 146.9)).abs() < 1e-12);
    assert!((masking_db(70.0) - -24.8).abs() < 1e-12);
    assert_eq!(masking_db(100.0), -10.0);
    assert_eq!(masking_db(130.0), -10.0);
    for edge in [63.0, 67.0] {
        let jump = masking_db(edge) - masking_db(edge - 1e-9);
        assert!(jump.abs() < 1e-6, "{edge} dB: {jump}");
    }
    // At 100 dB the table as printed steps by 0.2 dB (0.5·100 − 59.8 = −9.8, then −10), though
    // its note 2 calls the scheme continuous; the table is followed as printed.
    let jump = masking_db(100.0) - masking_db(100.0 - 1e-9);
    assert!((jump - -0.2).abs() < 1e-6, "{jump}");
}

#[test]
fn the_125_hz_band_is_not_masked() {
    // A 125 Hz band at the reception threshold's level plus 40 dB reads the same TI whatever
    // the other bands' levels: nothing below it masks it.
    let mut quiet = flat(30.0, 0.7);
    let mut loud = flat(110.0, 0.7);
    quiet[0].speech_db = Some(86.0);
    loud[0].speech_db = Some(86.0);
    let a = sti(&quiet, Gender::Male).unwrap().mti[0];
    let b = sti(&loud, Gender::Male).unwrap().mti[0];
    assert_eq!(a, b);
    assert_eq!(a.0, 125);
}

#[test]
fn female_speech_has_no_125_hz_term() {
    // Every band at m = 0.6 except 125 Hz: the male STI moves with 125 Hz's MTF, the female does
    // not, and needs no 125 Hz band at all.
    let at = |m125: f64| {
        let mut b = flat(75.0, 0.6);
        b[0].mtf = [m125; 14];
        b
    };
    let male0 = sti(&at(0.0), Gender::Male).unwrap().value;
    let male1 = sti(&at(1.0), Gender::Male).unwrap().value;
    assert!((male1 - male0).abs() > 0.01, "{male0} {male1}");
    let f0 = sti(&for_female(at(0.0)), Gender::Female).unwrap();
    let f1 = sti(&for_female(at(1.0)), Gender::Female).unwrap();
    let without: Vec<LevelBand> = for_female(at(0.0)).into_iter().skip(1).collect();
    let f2 = sti(&without, Gender::Female).unwrap();
    assert_eq!(f0.value, f1.value);
    assert_eq!(f0.value, f2.value);
    assert_eq!(f0.mti.len(), 6);
    assert_eq!(f0.mti[0].0, 250);
    // Female speech has no 125 Hz level at all (Table A.4).
    assert_eq!(Gender::Female.speech_db(125, 0.0), None);
    assert_eq!(Gender::Male.speech_db(125, 0.0), Some(62.9));
    assert_eq!(Gender::Female.speech_db(250, -3.0), Some(62.3));
}

#[test]
fn male_sti_is_truncated_at_one() {
    // Table A.3's note: the 250 Hz band at TI <= 0.15 with every other at 1 gives 1.036.
    let mut bands = flat(80.0, 1.0);
    bands[1].mtf = [0.0; 14];
    let s = sti(&bands, Gender::Male).unwrap();
    let expect = 1.381 - 0.127 - (0.381 - 0.085 - 0.078);
    assert!((s.untruncated - expect).abs() < 1e-9, "{s:?}");
    assert!(s.untruncated > 1.03);
    assert_eq!(s.value, 1.0);
    // The female factors in the same corner stay below 1 (1.328 − 0.117 − (0.328 − 0.099)).
    let f = sti(&for_female(bands), Gender::Female).unwrap();
    assert!((f.untruncated - 0.982).abs() < 1e-9, "{f:?}");
    assert_eq!(f.value, f.untruncated);
}

#[test]
fn a_band_the_speech_needs_missing_refuses() {
    let six: Vec<LevelBand> = flat(80.0, 1.0).into_iter().take(6).collect();
    for gender in [Gender::Male, Gender::Female] {
        let e = sti(&six, gender).unwrap_err();
        assert_eq!(e.code(), codes::NOT_EVALUABLE);
        assert!(
            matches!(
                e.not_evaluable(),
                Some(NotEvaluable::BandMissing { freq_hz: 8000, .. })
            ),
            "{e}"
        );
        assert!(e.to_string().contains("8000"), "{e}");
    }
    // Male needs 125 Hz; female does not.
    let no125: Vec<LevelBand> = flat(80.0, 1.0).into_iter().skip(1).collect();
    assert!(matches!(
        sti(&no125, Gender::Male).unwrap_err().not_evaluable(),
        Some(NotEvaluable::BandMissing { freq_hz: 125, .. })
    ));
    assert!(sti(&no125, Gender::Female).is_ok());
}

// --- a receiver's bands -------------------------------------------------------------------------

/// `ρc` times a source power such that the free field at 1 m reads `level_db`.
fn power_for(level_db: f64) -> f64 {
    let at_one = simpa_core::params::sti::free_field_level_at_db(1.0, 1.0).unwrap();
    10f64.powf((level_db - at_one) / 10.0)
}

struct Band {
    freq: i32,
    energy: Vec<f64>,
    unusable: Option<String>,
    t: Option<f64>,
}

fn receiver(bands: &[Band], dt: f64, octave: bool) -> simpa_core::params::sti::ReceiverSti {
    let rb: Vec<ReceiverBand> = bands
        .iter()
        .map(|b| ReceiverBand {
            freq_hz: b.freq,
            energy: &b.energy,
            from: 0,
            spl_db: Ok(70.0),
            power_rho_c: power_for(70.0),
            noise_db: None,
            reverberation_s: b.t,
            unusable: b.unusable.clone(),
            unseen_share: 0.0,
        })
        .collect();
    receiver_sti(dt, &rb, octave)
}

fn room(freqs: &[i32], t60: f64, dt: f64, length_s: f64) -> Vec<Band> {
    freqs
        .iter()
        .map(|&f| Band {
            freq: f,
            energy: exponential(t60, dt, (length_s / dt).round() as usize),
            unusable: None,
            t: Some(t60),
        })
        .collect()
}

#[test]
fn a_receiver_with_every_octave_and_a_long_enough_response_gives_both_speeches() {
    let dt = 1e-3;
    let r = receiver(&room(&OCTAVES_HZ, 1.0, dt, 3.0), dt, true);
    let male = r.male.unwrap();
    let female = r.female.unwrap();
    // The transfer is 0 dB (SPL equals the free field at 1 m): the speech is Table A.4 at
    // 60 dB(A), and the STI is set by the reverberation alone; T = 1 s gives about 0.6.
    assert!(male.value > 0.5 && male.value < 0.7, "{male:?}");
    assert!(female.value > 0.5 && female.value < 0.75, "{female:?}");
    assert_eq!(r.bands.len(), 7);
    assert!((r.bands[0].transfer_db.unwrap()).abs() < 1e-9);
    assert!((r.bands[0].speech_male_db.unwrap() - 62.9).abs() < 1e-9);
    assert_eq!(r.bands[0].speech_female_db, None);
    assert_eq!(r.bands[0].mti_female, None);
    assert!(r.bands[6].mti_male.is_some() && r.bands[6].mti_female.is_some());
    // The free field at 1 m is 20 dB above the one G uses at 10 m.
    let p = 3.7;
    let d = simpa_core::params::sti::free_field_level_at_db(p, 1.0).unwrap()
        - level::free_field_level_db(p).unwrap();
    assert!((d - 20.0).abs() < 1e-12);
}

#[test]
fn a_run_without_8_khz_refuses_both_speeches() {
    let dt = 1e-3;
    let r = receiver(&room(&OCTAVES_HZ[..6], 1.0, dt, 3.0), dt, true);
    for s in [&r.male, &r.female] {
        let e = s.as_ref().unwrap_err();
        assert!(
            matches!(
                e.not_evaluable(),
                Some(NotEvaluable::BandMissing { freq_hz: 8000, .. })
            ),
            "{e}"
        );
    }
    // The bands it has are still reported, with their MTF.
    assert_eq!(r.bands.len(), 6);
    assert!(
        r.bands
            .iter()
            .all(|b| b.mtf.is_some() && b.mti_male.is_none())
    );
}

#[test]
fn a_short_response_refuses() {
    let dt = 1e-3;
    // Shorter than 1.6 s.
    let r = receiver(&room(&OCTAVES_HZ, 0.5, dt, 1.5), dt, true);
    let e = r.male.unwrap_err();
    assert_eq!(e.code(), codes::SERIES_TOO_SHORT, "{e}");
    assert!(matches!(e, ParamError::SeriesTooShort { needed_s, .. } if needed_s == 1.6));
    // Longer than 1.6 s, shorter than T/2 (T = 5 s: 2.5 s needed). The series still decays at its
    // end, so the decay its end shows counts too: with `exponential`'s 13.8 that is
    // 5·6·ln 10/13.8 = 5.006 s.
    let t_end = 5.0 * 6.0 * 10f64.ln() / 13.8;
    let r = receiver(&room(&OCTAVES_HZ, 5.0, dt, 2.0), dt, true);
    let e = r.female.unwrap_err();
    assert!(
        matches!(e, ParamError::SeriesTooShort { needed_s, available_s, .. }
            if (needed_s - t_end / 2.0).abs() < 1e-9 && (available_s - 2.0).abs() < 1e-9),
        "{e}"
    );
    // Long enough for both.
    assert!(
        receiver(&room(&OCTAVES_HZ, 5.0, dt, 2.6), dt, true)
            .male
            .is_ok()
    );
}

#[test]
fn a_band_whose_series_is_refused_or_has_no_reverberation_time_refuses() {
    let dt = 1e-3;
    let mut bands = room(&OCTAVES_HZ, 1.0, dt, 3.0);
    bands[3].unusable = Some("the series is not complete".into());
    let r = receiver(&bands, dt, true);
    let e = r.male.unwrap_err();
    assert!(
        matches!(
            e.not_evaluable(),
            Some(NotEvaluable::BandRefused { freq_hz: 1000, .. })
        ),
        "{e}"
    );
    assert!(e.to_string().contains("not complete"), "{e}");
    let mut bands = room(&OCTAVES_HZ, 1.0, dt, 3.0);
    bands[5].t = None;
    let e = receiver(&bands, dt, true).male.unwrap_err();
    assert!(
        matches!(
            e.not_evaluable(),
            Some(NotEvaluable::BandRefused { freq_hz: 4000, .. })
        ),
        "{e}"
    );
    // A 125 Hz band that cannot be read refuses male speech only.
    let mut bands = room(&OCTAVES_HZ, 1.0, dt, 3.0);
    bands[0].unusable = Some("refused".into());
    let r = receiver(&bands, dt, true);
    assert!(r.male.is_err() && r.female.is_ok());
}

#[test]
fn third_octave_bands_refuse() {
    let dt = 1e-3;
    let r = receiver(&room(&OCTAVES_HZ, 1.0, dt, 3.0), dt, false);
    assert!(matches!(
        r.male.unwrap_err().not_evaluable(),
        Some(NotEvaluable::NotOctaveBands { .. })
    ));
}

#[test]
fn background_noise_lowers_the_sti() {
    let dt = 1e-3;
    let bands = room(&OCTAVES_HZ, 0.8, dt, 3.0);
    let with = |noise: Option<f64>| {
        let rb: Vec<ReceiverBand> = bands
            .iter()
            .map(|b| ReceiverBand {
                freq_hz: b.freq,
                energy: &b.energy,
                from: 0,
                spl_db: Ok(70.0),
                power_rho_c: power_for(70.0),
                noise_db: noise,
                reverberation_s: b.t,
                unusable: None,
                unseen_share: 0.0,
            })
            .collect();
        receiver_sti(dt, &rb, true).male.unwrap().value
    };
    let quiet = with(None);
    let noisy = with(Some(50.0));
    assert!(noisy < quiet - 0.05, "{quiet} then {noisy}");
}

// --- build H: the STI bed's findings (docs/investigations/2026-10-02-bed/ADDENDUM-4.md) ----------

/// Set A's S1 row `s1|1.5|042|double|R0.1|1ms|short` re-created: a double-slope decay (T60 8.08 s
/// and 12.12 s, the late slope 18.9 dB down, DRR −16.3 dB, a 38.3 ms gap) cut 3.28 s after its
/// arrival, `synth_fresh.py`'s closed form integrated over 1 ms bins. Its reverberation time is
/// 8.26 s, so cl. 8.3 a needs 4.13 s; the product read T30 6.34 s on the cut series, which it was
/// told is complete (the shim's claim, ADDENDUM-3 item 6), and answered.
#[test]
fn a_series_cut_while_it_still_decays_refuses_for_length_whatever_t_it_reads() {
    use simpa_core::params::EnergySeries;
    use simpa_core::params::decay::{self, Arrival};
    let dt = 1e-3;
    let (t_arr, hw, gap) = (0.099_740_423, 0.1 / 343.2, 0.038_308_512);
    let (a, k) = ([1.0, 0.008_540_111], [1.709_725_006, 1.139_816_671]);
    let ed = 0.013_899_316;
    let series = |n: usize| -> Vec<f64> {
        let t0 = t_arr + gap;
        (0..n)
            .map(|i| {
                let (lo, hi) = (i as f64 * dt, (i + 1) as f64 * dt);
                let direct = if (t_arr / dt).floor() as usize == i {
                    ed
                } else {
                    0.0
                };
                let rev: f64 = if hi <= t0 {
                    0.0
                } else {
                    let lo = lo.max(t0);
                    (0..2)
                        .map(|j| {
                            a[j] / k[j] * ((-k[j] * (lo - t0)).exp() - (-k[j] * (hi - t0)).exp())
                        })
                        .sum()
                };
                direct + rev
            })
            .collect()
    };
    let t_of = |bins: &[f64]| {
        let p = decay::evaluate(
            &EnergySeries::complete(dt, bins.to_vec()).unwrap(),
            Arrival::spread(t_arr, hw),
        );
        p.t30.as_ref().map(|x| x.t_s).unwrap()
    };
    // The true reverberation time, on the decay run to 2.5 times its slow T60.
    let truth = t_of(&series(30_400));
    assert!((truth - 8.26).abs() < 0.02, "{truth}");
    let cut = series(3379);
    let from = ((t_arr - hw) / dt).floor() as usize;
    let length_s = (cut.len() - from) as f64 * dt;
    assert!(length_s < truth / 2.0, "{length_s} vs {truth}");
    // The cut series reads a T30 short enough to pass the T/2 rule: the defect.
    let read = t_of(&cut);
    assert!(read / 2.0 < length_s && read < 6.5, "{read}");
    let bands: Vec<ReceiverBand> = OCTAVES_HZ
        .iter()
        .map(|&f| ReceiverBand {
            freq_hz: f,
            energy: &cut,
            from,
            spl_db: Ok(70.0),
            power_rho_c: power_for(70.0),
            noise_db: None,
            reverberation_s: Some(read),
            unusable: None,
            unseen_share: 0.0,
        })
        .collect();
    let r = receiver_sti(dt, &bands, true);
    for s in [&r.male, &r.female] {
        let e = s.as_ref().unwrap_err();
        assert_eq!(e.code(), codes::SERIES_TOO_SHORT, "{e}");
        let ParamError::SeriesTooShort { needed_s, .. } = e else {
            unreachable!()
        };
        // What its end decays at, not the T30 the cut bends down.
        assert!(
            *needed_s > length_s && *needed_s >= truth / 2.0 - 0.1,
            "{e}"
        );
    }
}

/// Backlog 66: female speech has no 125 Hz band, but 125 Hz's noise masks 250 Hz (Table A.1).
/// A noisy receiver whose 125 Hz band cannot be read must not give female STI with 250 Hz
/// unmasked.
#[test]
fn a_noisy_receiver_whose_125_hz_band_cannot_be_read_refuses_female_speech() {
    let dt = 1e-3;
    let mut bands = room(&OCTAVES_HZ, 1.0, dt, 3.0);
    bands[0].unusable = Some("its series is refused".into());
    let with = |noise: Option<f64>| {
        let rb: Vec<ReceiverBand> = bands
            .iter()
            .map(|b| ReceiverBand {
                freq_hz: b.freq,
                energy: &b.energy,
                from: 0,
                spl_db: Ok(70.0),
                power_rho_c: power_for(70.0),
                noise_db: noise,
                reverberation_s: b.t,
                unusable: b.unusable.clone(),
                unseen_share: 0.0,
            })
            .collect();
        receiver_sti(dt, &rb, true)
    };
    let e = with(Some(55.0)).female.unwrap_err();
    assert!(
        matches!(
            e.not_evaluable(),
            Some(NotEvaluable::BandRefused { freq_hz: 125, .. })
        ),
        "{e}"
    );
    assert!(e.to_string().contains("250 Hz"), "{e}");
    // Without noise 125 Hz masks nothing in female speech: answered.
    assert!(with(None).female.is_ok());
}

/// The energy a band's series can lack, `x` of what it holds, bounds the STI between the ends
/// `sti_unseen_range` gives: checked by adding `x` of it after the end of the 250 Hz band (which
/// also masks 500 Hz) at 40 delays across a period of 0.63 Hz, its speech level raised with it.
#[test]
fn the_unseen_energy_bounds_the_sti_and_refuses_only_beyond_its_limit() {
    let dt = 1e-3;
    let t60 = 1.2;
    let base = exponential(t60, dt, 3000);
    let total: f64 = base.iter().sum();
    let levels = |extra: Option<(usize, f64)>| -> Vec<LevelBand> {
        OCTAVES_HZ
            .iter()
            .map(|&f| {
                let mut e = base.clone();
                let mut transfer = 0.0;
                if let (250, Some((at, x))) = (f, extra) {
                    e.resize(at + 1, 0.0);
                    e[at] += x * total;
                    transfer = 10.0 * (1.0 + x).log10();
                }
                LevelBand {
                    freq_hz: f,
                    mtf: mtf(&e, dt, 0).unwrap(),
                    speech_db: Gender::Male.speech_db(f, 10.0 + transfer),
                    noise_db: Some(30.0),
                }
            })
            .collect()
    };
    for x in [1e-4, 1e-3, 1e-2, 0.1] {
        let lack = |f: i32| if f == 250 { x } else { 0.0 };
        let (lo, hi) = sti_unseen_range(&levels(None), Gender::Male, &lack).unwrap();
        let given = sti(&levels(None), Gender::Male).unwrap().value;
        assert!(lo <= given && given <= hi, "{lo} {given} {hi}");
        for j in 0..40 {
            let at = 3000 + (f64::from(j) * 1.587 / 40.0 / dt) as usize;
            let v = sti(&levels(Some((at, x))), Gender::Male).unwrap().value;
            assert!(
                lo - 1e-12 <= v && v <= hi + 1e-12,
                "x {x}, bin {at}: {lo} <= {v} <= {hi}"
            );
        }
    }
    // Through a receiver: a negligible share gives the same STI, a large one refuses.
    let bands = room(&OCTAVES_HZ, t60, dt, 3.0);
    let with = |x: f64| {
        let rb: Vec<ReceiverBand> = bands
            .iter()
            .map(|b| ReceiverBand {
                freq_hz: b.freq,
                energy: &b.energy,
                from: 0,
                spl_db: Ok(70.0),
                power_rho_c: power_for(70.0),
                noise_db: None,
                reverberation_s: b.t,
                unusable: None,
                unseen_share: if b.freq == 250 { x } else { 0.0 },
            })
            .collect();
        receiver_sti(dt, &rb, true)
    };
    let clean = with(0.0).male.unwrap().value;
    assert_eq!(with(1e-6).male.unwrap().value, clean);
    let e = with(0.05).male.unwrap_err();
    assert!(
        matches!(
            e.not_evaluable(),
            Some(NotEvaluable::BandRefused { freq_hz: 250, .. })
        ),
        "{e}"
    );
    assert!(e.to_string().contains("can move the STI"), "{e}");
    assert!(e.to_string().contains(&UNSEEN_LIMIT.to_string()), "{e}");
    // A share that is not a number makes the band unusable.
    assert!(with(f64::NAN).male.is_err());
}

#[test]
fn the_end_of_a_response_says_whether_it_has_ended_or_how_it_still_decays() {
    let dt = 1e-3;
    // Cut 2 s into a 2 s decay: decaying there at its own T60.
    match end_decay(&exponential(2.0, dt, 2000), dt, 0).unwrap() {
        EndDecay::Decaying { t_s, drop_db } => {
            assert!((t_s - 2.0 * 6.0 * 10f64.ln() / 13.8).abs() < 1e-6, "{t_s}");
            assert!(drop_db < END_DECAY_DB, "{drop_db}");
        }
        other => panic!("{other:?}"),
    }
    // Run to 3 T60: more than 60 dB down, ended.
    assert_eq!(
        end_decay(&exponential(1.0, dt, 3000), dt, 0),
        Some(EndDecay::Ended)
    );
    // Nothing in its last window: ended.
    let mut e = exponential(5.0, dt, 1000);
    e.resize(3000, 0.0);
    assert_eq!(end_decay(&e, dt, 0), Some(EndDecay::Ended));
    // Flat at its end: not decaying.
    let mut e = exponential(1.0, dt, 1000);
    e.extend(std::iter::repeat_n(1e-3, 1000));
    assert!(matches!(
        end_decay(&e, dt, 0),
        Some(EndDecay::NotDecaying { .. })
    ));
    // Fewer than two bins from `from`: nothing to read.
    assert_eq!(end_decay(&[1.0], dt, 0), None);
}
