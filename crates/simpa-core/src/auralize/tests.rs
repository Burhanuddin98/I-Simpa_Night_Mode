//! The auralization's tests (C5's done-when, item 1). Each prints the numbers it measures; the
//! report quotes them (`docs/investigations/2026-10-07-auralization/REPORT.md`). The CR4 bed on a
//! real run is `tests/auralize_cr4.rs`.

use super::*;

const OCTAVES: [f64; 6] = [125.0, 250.0, 500.0, 1000.0, 2000.0, 4000.0];
const THIRDS: [f64; 18] = [
    100.0, 125.0, 160.0, 200.0, 250.0, 315.0, 400.0, 500.0, 630.0, 800.0, 1000.0, 1250.0, 1600.0,
    2000.0, 2500.0, 3150.0, 4000.0, 5000.0,
];

fn close(a: f64, b: f64, tol: f64) -> bool {
    (a - b).abs() <= tol * b.abs().max(1e-300)
}

#[test]
fn the_bands_are_the_runs_own_edges_at_geometric_midpoints() {
    let f = Filters::new(&OCTAVES, &OCTAVES, None).unwrap();
    assert_eq!(f.fraction, Fraction::Octave);
    assert!(close(f.bands[0].lo_hz, 125.0 / 2f64.sqrt(), 1e-12));
    assert!(close(f.bands[0].hi_hz, (125.0f64 * 250.0).sqrt(), 1e-12));
    assert_eq!(f.bands[0].hi_hz, f.bands[1].lo_hz);
    assert!(close(f.bands[5].hi_hz, 4000.0 * 2f64.sqrt(), 1e-12));
    assert!(close(f.transition_octaves, NOISE_TRANSITION, 1e-12));

    let t = Filters::new(&THIRDS, &THIRDS, None).unwrap();
    assert_eq!(t.fraction, Fraction::Third);
    assert!(close(t.bands[0].lo_hz, 100.0 / 2f64.powf(1.0 / 6.0), 1e-12));
    assert!(close(t.bands[0].hi_hz, (100.0f64 * 125.0).sqrt(), 1e-12));
    assert!(close(
        t.bands[17].hi_hz,
        5000.0 * 2f64.powf(1.0 / 6.0),
        1e-12
    ));
    // Nominal centres are not exact thirds: the narrowest band sets the transition.
    let narrowest = t
        .bands
        .iter()
        .map(|b| (b.hi_hz / b.lo_hz).log2())
        .fold(f64::INFINITY, f64::min);
    assert!(close(
        t.transition_octaves,
        NOISE_TRANSITION * narrowest,
        1e-12
    ));
    assert!(close(
        t.with_transition(SMOOTH_TRANSITION).transition_octaves,
        SMOOTH_TRANSITION * narrowest,
        1e-12
    ));
    assert!(
        narrowest > 0.3 && narrowest < 1.0 / 3.0 + 1e-9,
        "{narrowest}"
    );

    // A band the run skipped: its neighbours take half a band out on that side.
    let g = Filters::new(&[125.0, 1000.0], &OCTAVES, None).unwrap();
    assert!(close(g.bands[0].hi_hz, 125.0 * 2f64.sqrt(), 1e-12));
    assert!(close(g.bands[1].lo_hz, 1000.0 / 2f64.sqrt(), 1e-12));
    // One band: its set tells the fraction; alone, the caller must.
    let one = Filters::new(&[1000.0], &OCTAVES, None).unwrap();
    assert!(close(one.bands[0].lo_hz, 1000.0 / 2f64.sqrt(), 1e-12));
    assert_eq!(
        Filters::new(&[1000.0], &[1000.0], None).unwrap_err().code(),
        "aural_bands"
    );
    assert!(Filters::new(&[1000.0], &[1000.0], Some(Fraction::Third)).is_ok());
    // A top edge past the Nyquist frequency is refused.
    assert_eq!(
        Filters::new(&[16000.0, 32000.0], &[16000.0, 32000.0], None)
            .unwrap_err()
            .code(),
        "aural_bands"
    );
}

/// The reconstruction ripple: each bank's powers summed across the band set (done-when 1, item 4),
/// analytically on a fine grid, and as measured on a synthesised "white" response.
#[test]
fn the_filters_sum_flat_across_the_band_set() {
    for (name, set) in [("octaves", &OCTAVES[..]), ("thirds", &THIRDS[..])] {
        let noise = Filters::new(set, set, None).unwrap();
        for (bank, f) in [
            ("noise", noise.clone()),
            ("smooth", noise.with_transition(SMOOTH_TRANSITION)),
        ] {
            let ripple = f.ripple_db(20_000);
            let (a, b) = f.flat_range_hz();
            println!(
                "ripple, {name}, {bank} bank: {ripple:.2e} dB over {a:.1} to {b:.1} Hz ({} bands, transition {:.4} oct)",
                f.bands.len(),
                f.transition_octaves
            );
            assert!(ripple < 1e-9, "{name} {bank}: {ripple} dB");
            for (k, band) in f.bands.iter().enumerate() {
                let mid = (band.lo_hz * band.hi_hz).sqrt();
                assert_eq!(f.power(k, mid), 1.0);
            }
        }
    }
    // Measured: an echogram whose band energies are proportional to each band's width in Hz is
    // white; its synthesised response's spectrum, averaged in sixth-octave bins across the flat
    // range, should be flat within the estimator's own spread.
    let f = Filters::new(&THIRDS, &THIRDS, None).unwrap();
    let steps = 2000;
    let energy: Vec<Vec<f64>> = f
        .bands
        .iter()
        .map(|b| vec![1e-6 * (b.hi_hz - b.lo_hz); steps])
        .collect();
    let e = Echogram {
        dt_s: 0.001,
        energy,
    };
    // Eight seeds' periodograms averaged, so the estimator's own spread is small.
    let seeds = 8u64;
    let mut power: Vec<f64> = Vec::new();
    let mut n = 0;
    for seed in 0..seeds {
        let s = synthesise(&e, &f, SAMPLE_RATE, SEED + seed, 0).unwrap();
        n = next_pow2(s.samples.len());
        let mut spec = vec![C::ZERO; n];
        for (c, &v) in spec.iter_mut().zip(&s.samples) {
            c.re = v;
        }
        transform(&mut spec, false);
        if power.is_empty() {
            power = vec![0.0; n];
        }
        for (p, c) in power.iter_mut().zip(&spec) {
            *p += c.norm_sqr() / seeds as f64;
        }
    }
    let df = f64::from(SAMPLE_RATE) / n as f64;
    let (a, b) = f.with_transition(SMOOTH_TRANSITION).flat_range_hz();
    let mut levels = Vec::new();
    let mut lo = a;
    while lo * 2f64.powf(1.0 / 6.0) <= b {
        let hi = lo * 2f64.powf(1.0 / 6.0);
        let (k0, k1) = ((lo / df).ceil() as usize, (hi / df).floor() as usize);
        let p: f64 = power[k0..=k1].iter().sum::<f64>() / (k1 + 1 - k0) as f64;
        levels.push((lo, k1 + 1 - k0, 10.0 * p.log10()));
        lo = hi;
    }
    let mean = levels.iter().map(|l| l.2).sum::<f64>() / levels.len() as f64;
    let dev = levels
        .iter()
        .map(|l| (l.2 - mean).abs())
        .fold(0.0, f64::max);
    // The estimator's spread: a periodogram line is exponential, so a mean of m lines over 8 seeds
    // has a relative standard deviation of 1/sqrt(8m); three of those, in dB, at the fewest lines.
    let fewest = levels.iter().map(|l| l.1).min().unwrap();
    let spread = 3.0 * 10.0 * (1.0 + 1.0 / ((seeds as usize * fewest) as f64).sqrt()).log10();
    println!(
        "white echogram, thirds: the response's sixth-octave levels within ±{dev:.3} dB of their mean \
         over {a:.0} to {b:.0} Hz ({} bins, 8 seeds; the estimator's 3-sigma spread at {fewest} lines: \
         {spread:.3} dB)",
        levels.len()
    );
    assert!(dev < spread, "{dev} dB");
}

#[test]
fn a_single_band_single_step_echogram_gives_one_band_limited_click_at_the_right_time() {
    for centre in [1000.0, 4000.0] {
        let f = Filters::new(&[centre], &OCTAVES, None).unwrap();
        let steps = 200;
        let k = 50;
        let mut series = vec![0.0; steps];
        series[k] = 1e-3;
        let e = Echogram {
            dt_s: 0.001,
            energy: vec![series],
        };
        let bounds = step_bounds(steps, e.dt_s, SAMPLE_RATE);
        let (a, b) = (bounds[k], bounds[k + 1]);
        let s = synthesise(&e, &f, SAMPLE_RATE, SEED, 0).unwrap();
        assert_eq!(s.samples.len(), 9600);
        let energy: f64 = s.samples.iter().map(|v| v * v).sum();
        // Where its energy lies in time.
        let centroid = s
            .samples
            .iter()
            .enumerate()
            .map(|(i, v)| i as f64 * v * v)
            .sum::<f64>()
            / energy;
        let centre_sample = 0.5 * (a + b) as f64 - 0.5;
        let peak = s
            .samples
            .iter()
            .enumerate()
            .fold(
                (0, 0.0),
                |m, (i, v)| if v.abs() > m.1 { (i, v.abs()) } else { m },
            )
            .0;
        let mut within = 0;
        while s.samples[a.saturating_sub(within)..(b + within).min(9600)]
            .iter()
            .map(|v| v * v)
            .sum::<f64>()
            < 0.99 * energy
        {
            within += 1;
        }
        // Where it lies in frequency.
        let n = next_pow2(4 * 9600);
        let df = f64::from(SAMPLE_RATE) / n as f64;
        let mut spec = vec![C::ZERO; n];
        for (c, &v) in spec.iter_mut().zip(&s.samples) {
            c.re = v;
        }
        transform(&mut spec, false);
        let (mut inb, mut tot) = (0.0, 0.0);
        let smooth = f.with_transition(SMOOTH_TRANSITION);
        let (lo, hi) = (
            f.bands[0].lo_hz / 2f64.powf(smooth.transition_octaves / 2.0),
            f.bands[0].hi_hz * 2f64.powf(smooth.transition_octaves / 2.0),
        );
        for (i, c) in spec.iter().enumerate().take(n / 2) {
            tot += c.norm_sqr();
            if (lo..=hi).contains(&(i as f64 * df)) {
                inb += c.norm_sqr();
            }
        }
        println!(
            "click, {centre:.0} Hz octave, one 1 ms step of 1e-3 Pa² at step {k}: energy {:.6} of the \
             step's; centroid {:+.3} ms from the step's centre; peak at sample {peak} (step {a}..{b}); 99 % \
             of the energy within {:.2} ms of the step; {:.3} % of it inside the band's support {lo:.0}-{hi:.0} Hz",
            energy / 1e-3,
            (centroid - centre_sample) / 48.0,
            within as f64 / 48.0,
            100.0 * inb / tot
        );
        assert!((energy / 1e-3 - 1.0).abs() < 1e-9);
        assert!(
            (centroid - centre_sample).abs() < 24.0,
            "centroid off by more than half a step"
        );
        assert!((a..b).contains(&peak), "the peak outside the step");
        assert!(inb / tot > 0.999, "{}", inb / tot);
    }
}

#[test]
fn the_envelope_holds_each_steps_energy_exactly() {
    let bounds = step_bounds(6, 0.001, SAMPLE_RATE);
    let energy = [0.0, 2.0, 1.0, 0.0, 0.0, 5.0];
    let p = envelope(&energy, &bounds);
    let got = step_energies(&p.iter().map(|v| v.sqrt()).collect::<Vec<_>>(), &bounds);
    for (g, w) in got.iter().zip(energy) {
        assert!((g - w).abs() <= 1e-12 * w.max(1.0), "{g} vs {w}");
    }
    assert!(p[..48].iter().all(|&v| v == 0.0));
    assert!(p[48..96].iter().all(|&v| v > 0.0));
}

/// A synthetic room's echogram: a direct sound, three reflections and an exponential tail with
/// a decay time per band, at 1 ms.
fn synthetic(f: &Filters, steps: usize) -> Echogram {
    let energy = f
        .bands
        .iter()
        .enumerate()
        .map(|(b, _)| {
            let t60 = 1.8 - 0.2 * b as f64 / (f.bands.len() as f64 / 6.0);
            (0..steps)
                .map(|k| {
                    let t = k as f64 * 0.001;
                    let direct = if k == 12 { 4e-5 } else { 0.0 };
                    let refl = if [19, 27, 33].contains(&k) { 1e-5 } else { 0.0 };
                    let tail = if k >= 15 {
                        2e-6 * 10f64.powf(-6.0 * (t - 0.015) / t60)
                    } else {
                        0.0
                    };
                    direct + refl + tail
                })
                .collect()
        })
        .collect();
    Echogram {
        dt_s: 0.001,
        energy,
    }
}

/// Done-when 1, item 2: the synthesised response's band energies per step against the echogram's,
/// within the filters' leakage. The response is analysed back through the bank it was band-limited
/// with ([`analyse`]); per band: the band's total against the echogram's (the leakage, energy that
/// crossed into a neighbour or came from one), the energy-weighted error of the per-step energies
/// summed over the band's correction window (its time resolution, `2/B`), and the same per step
/// (the random fine structure, which a real room's late field has too).
#[test]
fn band_energies_per_step_equal_the_echograms_within_the_leakage() {
    for (name, set) in [("octaves", &OCTAVES[..]), ("thirds", &THIRDS[..])] {
        let f = Filters::new(set, set, None).unwrap();
        let steps = 1500;
        let e = synthetic(&f, steps);
        let bounds = step_bounds(steps, e.dt_s, SAMPLE_RATE);
        let smooth = f.with_transition(SMOOTH_TRANSITION);
        let s = synthesise(&e, &f, SAMPLE_RATE, SEED, 0).unwrap();
        let anal = analyse(&s.samples, &smooth, SAMPLE_RATE);
        let windows = correction_windows(&smooth, e.dt_s);
        let (mut worst_total, mut worst_window, mut worst_step) = (0.0f64, 0.0f64, 0.0f64);
        for b in 0..f.bands.len() {
            let got = step_energies(&anal[b], &bounds);
            let total: f64 = e.energy[b].iter().sum();
            let leak_db = 10.0 * (got.iter().sum::<f64>() / total).log10();
            let (we, wg) = (
                moving_sum(&e.energy[b], windows[b]),
                moving_sum(&got, windows[b]),
            );
            let wsum: f64 = we.iter().sum();
            let win_err: f64 = we.iter().zip(&wg).map(|(x, y)| (y - x).abs()).sum::<f64>() / wsum;
            let step_err: f64 = e.energy[b]
                .iter()
                .zip(&got)
                .map(|(x, y)| (y - x).abs())
                .sum::<f64>()
                / total;
            println!(
                "{name} {:>6.0} Hz: total analysed back {leak_db:+.3} dB; per step summed over {} ms off by {:.2} %; \
                 per 1 ms step {:.1} %",
                f.bands[b].centre_hz,
                windows[b],
                100.0 * win_err,
                100.0 * step_err
            );
            worst_total = worst_total.max(leak_db.abs());
            worst_window = worst_window.max(win_err);
            worst_step = worst_step.max(step_err);
        }
        println!(
            "{name}: leakage at most {worst_total:.3} dB in a band's total, {:.2} % per step at the band's \
             resolution, {:.1} % per 1 ms step",
            100.0 * worst_window,
            100.0 * worst_step
        );
        // The bounds are the first round numbers above what was measured (0.24 / 0.85 dB and 13 / 16 %,
        // 2026-10-07): they hold the synthesis to what it does, they are not a physical claim.
        assert!(worst_total < 1.0, "{name}: {worst_total} dB");
        assert!(worst_window < 0.2, "{name}: {worst_window}");
    }
}

#[test]
fn edt_and_t30_of_a_synthetic_decay_survive_synthesis() {
    let f = Filters::new(&OCTAVES, &OCTAVES, None).unwrap();
    let steps = 2500;
    let e = synthetic(&f, steps);
    let s = synthesise(&e, &f, SAMPLE_RATE, SEED, 0).unwrap();
    let bounds = step_bounds(steps, e.dt_s, SAMPLE_RATE);
    let anal = analyse_butterworth(&s.samples, &f, SAMPLE_RATE);
    for (b, band) in anal.iter().enumerate() {
        let (edt0, t300) = decay_times(&e.energy[b], e.dt_s);
        let (edt1, t301) = decay_times(&step_energies(band, &bounds), e.dt_s);
        let (edt0, t300, edt1, t301) = (edt0.unwrap(), t300.unwrap(), edt1.unwrap(), t301.unwrap());
        println!(
            "synthetic {:.0} Hz: EDT {edt0:.3} -> {edt1:.3} s ({:+.2} %), T30 {t300:.3} -> {t301:.3} s ({:+.2} %)",
            f.bands[b].centre_hz,
            100.0 * (edt1 / edt0 - 1.0),
            100.0 * (t301 / t300 - 1.0)
        );
        assert!((edt1 / edt0 - 1.0).abs() < 0.05);
        assert!((t301 / t300 - 1.0).abs() < 0.05);
    }
}

#[test]
fn the_same_inputs_give_the_same_samples_bit_for_bit() {
    let f = Filters::new(&OCTAVES, &OCTAVES, None).unwrap();
    let e = synthetic(&f, 300);
    let a = synthesise(&e, &f, SAMPLE_RATE, SEED, 0).unwrap();
    let b = synthesise(&e, &f, SAMPLE_RATE, SEED, 0).unwrap();
    assert!(
        a.samples
            .iter()
            .zip(&b.samples)
            .all(|(x, y)| x.to_bits() == y.to_bits())
    );
    let c = synthesise(&e, &f, SAMPLE_RATE, SEED, 1).unwrap();
    assert!(a.samples != c.samples, "another stream, another noise");
    let d = synthesise(&e, &f, SAMPLE_RATE, SEED + 1, 0).unwrap();
    assert!(a.samples != d.samples, "another seed, another noise");
    assert_eq!(a.samples.len(), 14_400);
}

#[test]
fn normalising_and_convolving() {
    let mut x = vec![0.0, -2.0, 1.0];
    let g = normalise(&mut x);
    assert!(close(g, 10f64.powf(-1.0 / 20.0) / 2.0, 1e-12));
    assert!(close(x[1], -(10f64.powf(-1.0 / 20.0)), 1e-12));
    let mut z = vec![0.0; 3];
    assert_eq!(normalise(&mut z), 1.0);
    // A unit impulse response leaves the dry signal as it is; a tail of zeros is cut.
    let dry = vec![0.5, -0.25, 0.125];
    let mut ir = vec![0.0; 100];
    ir[0] = 1.0;
    let y = auralize(&ir, &dry);
    assert_eq!(y.len(), 3);
    for (a, b) in y.iter().zip(&dry) {
        assert!((a - b).abs() < 1e-12);
    }
    // A delayed impulse delays it.
    let mut ir = vec![0.0; 10];
    ir[4] = 2.0;
    let y = auralize(&ir, &dry);
    assert_eq!(y.len(), 7);
    assert!((y[4] - 1.0).abs() < 1e-12 && (y[6] - 0.25).abs() < 1e-12);
}

#[test]
fn a_dry_file_at_44_1_khz_is_read_and_resampled() {
    let x: Vec<f64> = (0..44_100)
        .map(|n| 0.5 * (2.0 * std::f64::consts::PI * 440.0 * n as f64 / 44_100.0).sin())
        .collect();
    let bytes = wav::write(&x, 44_100, wav::SampleFormat::Pcm24, "").unwrap();
    let (w, y) = dry_recording(&bytes).unwrap();
    assert_eq!(w.rate, 44_100);
    assert_eq!(y.len(), 48_000);
    let err = (2000..46_000)
        .map(|n| {
            (y[n] - 0.5 * (2.0 * std::f64::consts::PI * 440.0 * n as f64 / 48_000.0).sin()).abs()
        })
        .fold(0.0, f64::max);
    println!(
        "dry recording, 44.1 kHz 440 Hz tone at 48 kHz: largest error {err:.2e} of a 0.5 peak"
    );
    assert!(err < 1e-4, "{err}");
    let long = wav::write(&vec![0.0; 121 * 8000], 8000, wav::SampleFormat::F32, "").unwrap();
    assert_eq!(
        dry_recording(&long).unwrap_err().code(),
        "aural_wav_too_long"
    );
}
