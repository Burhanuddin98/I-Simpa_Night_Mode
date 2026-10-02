//! STI (`params::sti`) against what IEC 60268-16:2011 fixes exactly: the MTF of an exponential
//! decay, the limits m = 1 and m = 0, the masking of band k by band k-1 (Table A.1), female speech
//! without 125 Hz, the truncation at 1.0 (Table A.3, note), and the refusals of cl. 8.3.

use simpa_core::params::sti::{
    Gender, LevelBand, MODULATION_HZ, OCTAVES_HZ, ReceiverBand, masking_db, mtf, receiver_sti, sti,
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
    // Longer than 1.6 s, shorter than T/2 (T = 5 s: 2.5 s needed).
    let r = receiver(&room(&OCTAVES_HZ, 5.0, dt, 2.0), dt, true);
    let e = r.female.unwrap_err();
    assert!(
        matches!(e, ParamError::SeriesTooShort { needed_s, available_s, .. }
            if (needed_s - 2.5).abs() < 1e-12 && (available_s - 2.0).abs() < 1e-9),
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
            })
            .collect();
        receiver_sti(dt, &rb, true).male.unwrap().value
    };
    let quiet = with(None);
    let noisy = with(Some(50.0));
    assert!(noisy < quiet - 0.05, "{quiet} then {noisy}");
}
