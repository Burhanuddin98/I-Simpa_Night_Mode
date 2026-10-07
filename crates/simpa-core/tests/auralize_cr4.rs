//! C5's bed on a real run (done-when 1, items 2 and 3): BRAS CR4's receivers, octave and
//! third-octave runs. For every receiver and every echogram (the sources summed, and each source's
//! own), the response is synthesised, analysed back through the bed's own band-pass bank
//! (`auralize::analyse_butterworth`, independent of the synthesis's filters), and its EDT and T30
//! (Schroeder, `auralize::decay_times`) are held to the echogram's within the JND, 5 %. The band
//! leakage (each band's energy, so analysed, against the echogram's, in total and after 50 ms) is
//! measured and recorded. Writes `receipt.json` and the summed responses as WAVs (48 kHz float,
//! peak −1 dBFS) under `$SIMPA_AURAL_OUT/<set>/`.
//!
//! ```text
//! $env:SIMPA_AURAL_RUNS = "<octave run>;<third-octave run>"; $env:SIMPA_AURAL_OUT = "<dir>"
//! cargo test -p simpa-core --release --test auralize_cr4 -- --ignored --nocapture
//! ```

use std::path::PathBuf;

use serde_json::json;
use simpa_core::auralize::{self, Pick, SAMPLE_RATE, SEED, wav};
use simpa_core::results;

const JND: f64 = 0.05;

#[test]
#[ignore = "needs CR4 runs: SIMPA_AURAL_RUNS (octave;third) and SIMPA_AURAL_OUT; run with --ignored"]
fn cr4_responses_keep_the_echograms_edt_and_t30_within_the_jnd() {
    let runs =
        std::env::var("SIMPA_AURAL_RUNS").expect("SIMPA_AURAL_RUNS: <octave run>;<third run>");
    let out = PathBuf::from(std::env::var("SIMPA_AURAL_OUT").expect("SIMPA_AURAL_OUT"));
    let mut receipt = Vec::new();
    let mut failures = Vec::new();
    for dir in runs.split(';').filter(|s| !s.is_empty()) {
        let run = results::load(&PathBuf::from(dir)).unwrap_or_else(|e| panic!("{dir}: {e}"));
        let filters = auralize::run_filters(&run).unwrap();
        let set = format!("{:?}", filters.fraction).to_lowercase();
        let wav_dir = out.join(&set);
        std::fs::create_dir_all(&wav_dir).unwrap();
        let spps = run.spps().unwrap();
        let mut picks = vec![None];
        picks.extend(spps.sources.iter().map(|s| Some(s.name.clone())));
        let nb = filters.bands.len();
        let mut worst_edt = vec![0.0f64; nb];
        let mut worst_t30 = vec![0.0f64; nb];
        let mut leak_total = vec![(f64::INFINITY, f64::NEG_INFINITY); nb];
        let mut leak_late = vec![(f64::INFINITY, f64::NEG_INFINITY); nb];
        let mut rows = Vec::new();
        for r in &spps.point_receivers {
            for pick in &picks {
                let p = pick.as_deref().map_or(Pick::Summed, Pick::Source);
                let resp = auralize::response(&run, &r.label, p, SEED).unwrap();
                let (e, _) = auralize::run_echogram(&run, &r.label, p).unwrap();
                let s = &resp.synthesis;
                let bounds = auralize::step_bounds(e.steps(), e.dt_s, SAMPLE_RATE);
                let anal = auralize::analyse_butterworth(&s.samples, &filters, SAMPLE_RATE);
                let late = (0.05 / e.dt_s).round() as usize;
                for b in 0..nb {
                    let ea = auralize::step_energies(&anal[b], &bounds);
                    let (edt0, t300) = auralize::decay_times(&e.energy[b], e.dt_s);
                    let (edt1, t301) = auralize::decay_times(&ea, e.dt_s);
                    let rel = |a: Option<f64>, b: Option<f64>| match (a, b) {
                        (Some(a), Some(b)) => Some(b / a - 1.0),
                        _ => None,
                    };
                    let (de, dt) = (rel(edt0, edt1), rel(t300, t301));
                    let tot0: f64 = e.energy[b].iter().sum();
                    let tot1: f64 = ea.iter().sum();
                    let late0: f64 = e.energy[b][late..].iter().sum();
                    let late1: f64 = ea[late..].iter().sum();
                    let lt = 10.0 * (tot1 / tot0).log10();
                    let ll = 10.0 * (late1 / late0).log10();
                    leak_total[b] = (leak_total[b].0.min(lt), leak_total[b].1.max(lt));
                    leak_late[b] = (leak_late[b].0.min(ll), leak_late[b].1.max(ll));
                    let who = format!(
                        "{set} {} {} {:.0} Hz",
                        r.label,
                        pick.as_deref().unwrap_or("summed"),
                        filters.bands[b].centre_hz
                    );
                    match (de, dt) {
                        (Some(de), Some(dt)) => {
                            worst_edt[b] = worst_edt[b].max(de.abs());
                            worst_t30[b] = worst_t30[b].max(dt.abs());
                            if de.abs() > JND || dt.abs() > JND {
                                failures.push(format!(
                                    "{who}: EDT {:+.2} %, T30 {:+.2} %",
                                    100.0 * de,
                                    100.0 * dt
                                ));
                            }
                        }
                        _ => failures.push(format!(
                            "{who}: not measured (echogram EDT {edt0:?} T30 {t300:?}, response EDT {edt1:?} T30 {t301:?})"
                        )),
                    }
                    rows.push(json!({
                        "receiver": r.label, "source": pick, "band_hz": filters.bands[b].centre_hz,
                        "edt_echogram_s": edt0, "edt_response_s": edt1,
                        "t30_echogram_s": t300, "t30_response_s": t301,
                        "leak_total_db": lt, "leak_after_50ms_db": ll,
                    }));
                }
                if pick.is_none() || r.label == spps.point_receivers[0].label {
                    let mut x = s.samples.clone();
                    let g = auralize::normalise(&mut x);
                    let name =
                        format!("{}-{}-ir.wav", r.label, pick.as_deref().unwrap_or("summed"));
                    let bytes = wav::write(
                        &x,
                        SAMPLE_RATE,
                        wav::SampleFormat::F32,
                        &resp.comment(g, "impulse response"),
                    )
                    .unwrap();
                    std::fs::write(wav_dir.join(name), bytes).unwrap();
                }
            }
        }
        println!(
            "{set}: {} receivers x {} echograms, {nb} bands, run {}",
            spps.point_receivers.len(),
            picks.len(),
            run.folder.display()
        );
        for b in 0..nb {
            println!(
                "  {:>6.0} Hz ({:.1}-{:.1}): worst |dEDT| {:.2} %, worst |dT30| {:.2} %; leakage total {:+.2}..{:+.2} dB, after 50 ms {:+.2}..{:+.2} dB",
                filters.bands[b].centre_hz,
                filters.bands[b].lo_hz,
                filters.bands[b].hi_hz,
                100.0 * worst_edt[b],
                100.0 * worst_t30[b],
                leak_total[b].0,
                leak_total[b].1,
                leak_late[b].0,
                leak_late[b].1
            );
        }
        receipt.push(json!({
            "set": set, "run": run.folder, "seed": format!("{SEED:#018x}"),
            "bands": filters.bands.iter().map(|b| json!({"centre_hz": b.centre_hz, "lo_hz": b.lo_hz, "hi_hz": b.hi_hz})).collect::<Vec<_>>(),
            "transition_octaves": filters.transition_octaves,
            "ripple_db": filters.ripple_db(20_000),
            "worst_edt": worst_edt, "worst_t30": worst_t30,
            "rows": rows,
        }));
    }
    std::fs::write(
        out.join("receipt.json"),
        serde_json::to_string_pretty(&json!({"jnd": JND, "sets": receipt, "failures": failures}))
            .unwrap(),
    )
    .unwrap();
    assert!(
        failures.is_empty(),
        "{} outside the JND:\n{}",
        failures.len(),
        failures.join("\n")
    );
}
