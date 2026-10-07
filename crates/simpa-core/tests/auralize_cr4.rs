//! C5's bed on a real run (done-when 1, items 2 and 3; the audit of 2026-10-08): BRAS CR4's
//! receivers, octave and third-octave runs.
//!
//! For every receiver and every source (the source-receiver pairs ISO 3382-1 defines; the app's
//! results refuse EDT and T30 for the sources summed), the response is synthesised with three
//! noise seeds, analysed back through each of the bed's two banks (`auralize::analyse_butterworth`,
//! independent of the synthesis's filters: the IEC 61260 class-1-like bank, a third-order
//! Butterworth prototype and sixth-order band-pass, which the headline uses; and a steeper one, a
//! sixth-order prototype and twelfth-order band-pass), and the per-step energies of each band put
//! in place of that source's echogram in a copy of the run. **Both sides' EDT and T30 are the app's
//! own**: `results::report`, the code `simpa results` prints, on the run and on the copy.
//!
//! Every value outside the JND (5 %) must be one of [`KNOWN`] (backlog 99), by bank, set, seed,
//! receiver, source, band, quantity and sign, and none may pass [`GUARD`] (6 %): a new miss, or a
//! known one growing, fails. The 5 % tolerance is not loosened; the known misses are listed.
//!
//! Also printed: the module's own Schroeder estimator (`auralize::decay_times`) against the report
//! on the echograms, and the early part of each pair's echogram (the direct sound's share, the
//! time to −10 dB) for why EDT runs short at MP4. Writes `receipt.json` and, for the first seed,
//! the summed responses and MP1's per-source ones as WAVs under `$SIMPA_AURAL_OUT/<set>/`.
//!
//! ```text
//! $env:SIMPA_AURAL_RUNS = "<octave run>;<third-octave run>"; $env:SIMPA_AURAL_OUT = "<dir>"
//! cargo test -p simpa-core --release --test auralize_cr4 -- --ignored --nocapture
//! ```

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde_json::json;
use simpa_core::auralize::{
    self, IEC_PROTOTYPE_ORDER, Pick, SAMPLE_RATE, SEED, STEEP_PROTOTYPE_ORDER, wav,
};
use simpa_core::results::report::{Evaluated, Report, report};
use simpa_core::results::{self, SolverResults};

const JND: f64 = 0.05;
/// A known miss may not grow past this.
const GUARD: f64 = 0.06;
const SEEDS: [u64; 3] = [SEED, SEED + 1, SEED + 2];
/// The analysed response's numerical floor, dB below a band's largest step: 30 dB under SPPS's own
/// (`trans_epsilon` 7, −70 dB below a particle's start), so nothing the echogram resolves is
/// touched, and far over the FFT's round-off (about −300 dB).
const FLOOR_DB: f64 = -100.0;
const BANKS: [(&str, i32); 2] = [
    ("iec", IEC_PROTOTYPE_ORDER),
    ("steep", STEEP_PROTOTYPE_ORDER),
];

/// A value outside the JND the bed knows of (backlog 99): bank, set, seed index, receiver,
/// source, band, quantity, the sign of the response's error, and its size as recorded.
type Miss = (
    &'static str,
    &'static str,
    usize,
    &'static str,
    &'static str,
    i32,
    &'static str,
    i8,
    f64,
);

/// Backlog 99: every miss of the bed of 2026-10-08 01:18 (`.out/aural/bed/receipt.json`), 24 of
/// 2,858 measured values: 23 through the IEC-like bank (22 T30, every one longer, at the 500 Hz,
/// 2 and 4 kHz octaves and the 400 Hz third; one EDT) and one through the steep bank (MP4/LS1
/// 160 Hz EDT, shorter). The synthesis and the report are deterministic, so a listed miss is held
/// to [`GUARD`] or, when it already lies past it, to its recorded size (+0.1 point for arithmetic):
/// growing fails, as does any miss not listed.
const KNOWN: &[Miss] = &[
    ("iec", "octave", 0, "MP1", "LS1", 500, "T30", 1, 0.0526),
    ("iec", "octave", 0, "MP1", "LS1", 4000, "T30", 1, 0.0579),
    ("iec", "octave", 0, "MP1", "LS2", 500, "T30", 1, 0.0614),
    ("iec", "octave", 0, "MP2", "LS1", 500, "T30", 1, 0.0694),
    ("iec", "octave", 0, "MP3", "LS1", 500, "T30", 1, 0.0886),
    ("iec", "octave", 0, "MP3", "LS2", 4000, "T30", 1, 0.0563),
    ("iec", "octave", 0, "MP5", "LS2", 2000, "T30", 1, 0.0641),
    ("iec", "octave", 1, "MP1", "LS2", 500, "T30", 1, 0.0683),
    ("iec", "octave", 1, "MP3", "LS1", 500, "T30", 1, 0.0635),
    ("iec", "octave", 1, "MP3", "LS1", 4000, "T30", 1, 0.0700),
    ("iec", "octave", 1, "MP3", "LS2", 4000, "T30", 1, 0.0758),
    ("iec", "octave", 1, "MP4", "LS2", 500, "T30", 1, 0.0701),
    ("iec", "octave", 1, "MP4", "LS2", 4000, "T30", 1, 0.0519),
    ("iec", "octave", 1, "MP5", "LS2", 500, "T30", 1, 0.0655),
    ("iec", "octave", 2, "MP3", "LS1", 500, "T30", 1, 0.0573),
    ("iec", "octave", 2, "MP3", "LS2", 4000, "T30", 1, 0.0799),
    ("iec", "octave", 2, "MP5", "LS2", 2000, "T30", 1, 0.0662),
    ("iec", "third", 0, "MP1", "LS2", 400, "T30", 1, 0.0509),
    ("iec", "third", 0, "MP2", "LS2", 400, "T30", 1, 0.0637),
    ("iec", "third", 0, "MP4", "LS2", 400, "T30", 1, 0.0620),
    ("iec", "third", 2, "MP1", "LS2", 400, "T30", 1, 0.0573),
    ("iec", "third", 2, "MP3", "LS2", 400, "EDT", 1, 0.0620),
    ("iec", "third", 2, "MP4", "LS2", 400, "T30", 1, 0.0722),
    ("steep", "third", 2, "MP4", "LS1", 160, "EDT", -1, 0.0508),
];

/// (receiver, source, band) to the report's (EDT, T30), each its value or its refusal's code.
type Values = BTreeMap<(String, String, i32), (Result<f64, String>, Result<f64, String>)>;

fn value(e: &Evaluated) -> Result<f64, String> {
    match e {
        Evaluated::Value { value, .. } => Ok(*value),
        Evaluated::NotEvaluable { not_evaluable } => Err(not_evaluable.message.clone()),
    }
}

fn values(rep: &Report) -> Values {
    let mut out = Values::new();
    for r in &rep.spps.as_ref().expect("an SPPS report").point_receivers {
        for s in &r.per_source {
            for b in &s.bands {
                out.insert(
                    (r.label.clone(), s.source.clone(), b.freq_hz),
                    (value(&b.parameters.edt_s), value(&b.parameters.t30_s)),
                );
            }
        }
    }
    out
}

#[test]
#[ignore = "needs CR4 runs: SIMPA_AURAL_RUNS (octave;third) and SIMPA_AURAL_OUT; run with --ignored"]
fn cr4_responses_keep_the_echograms_edt_and_t30_within_the_jnd_but_the_known_misses() {
    let runs =
        std::env::var("SIMPA_AURAL_RUNS").expect("SIMPA_AURAL_RUNS: <octave run>;<third run>");
    let out = PathBuf::from(std::env::var("SIMPA_AURAL_OUT").expect("SIMPA_AURAL_OUT"));
    let mut receipt = Vec::new();
    let mut all_misses = Vec::new();
    let mut guard_failures = Vec::new();
    for dir in runs.split(';').filter(|s| !s.is_empty()) {
        let run = results::load(&PathBuf::from(dir)).unwrap_or_else(|e| panic!("{dir}: {e}"));
        let filters = auralize::run_filters(&run).unwrap();
        let set = format!("{:?}", filters.fraction).to_lowercase();
        let spps = run.spps().unwrap();
        let dt = spps.time_step_s;
        let bands: Vec<i32> = run.bands_hz.clone();
        let orig = values(&report(&run));

        // The module's estimator against the report, on the echograms themselves.
        let (mut agree_edt, mut agree_t30, mut agree_n) = (0.0f64, 0.0f64, 0);
        // The early part of each pair's echogram.
        let mut early = Vec::new();
        for r in &spps.point_receivers {
            for e in &r.echograms {
                for (b, &hz) in bands.iter().enumerate() {
                    let (edt_m, t30_m) = auralize::decay_times(&e.energy[b], dt);
                    let (edt_r, t30_r) = &orig[&(r.label.clone(), e.source.clone(), hz)];
                    let (edt_r, t30_r) = (edt_r.clone().ok(), t30_r.clone().ok());
                    if let (Some(a), Some(b)) = (edt_m, edt_r) {
                        agree_edt = agree_edt.max((a / b - 1.0).abs());
                    }
                    if let (Some(a), Some(b)) = (t30_m, t30_r) {
                        agree_t30 = agree_t30.max((a / b - 1.0).abs());
                    }
                    agree_n += 1;
                    let total: f64 = e.energy[b].iter().sum();
                    let max = e.energy[b].iter().copied().fold(0.0, f64::max);
                    let onset = e.energy[b]
                        .iter()
                        .position(|&v| v >= 0.01 * max)
                        .unwrap_or(0);
                    let direct: f64 = e.energy[b][onset..(onset + 3).min(e.energy[b].len())]
                        .iter()
                        .sum();
                    let mut tail = total;
                    let mut t10 = None;
                    for (k, v) in e.energy[b].iter().enumerate().skip(onset) {
                        if tail <= 0.1 * total {
                            t10 = Some((k - onset) as f64 * dt);
                            break;
                        }
                        tail -= v;
                    }
                    early.push(json!({
                        "receiver": r.label, "source": e.source, "band_hz": hz,
                        "onset_s": onset as f64 * dt,
                        "direct_3_steps_share": direct / total,
                        "to_minus_10_db_s": t10,
                        "edt_report_s": edt_r, "edt_schroeder_s": edt_m,
                    }));
                }
            }
        }
        println!(
            "{set}: auralize::decay_times against results::report on the echograms ({agree_n} pair-bands): \
             EDT up to {:.2} %, T30 up to {:.2} % apart",
            100.0 * agree_edt,
            100.0 * agree_t30
        );
        // Each receiver's direct share and time to -10 dB, the pairs' worst, for MP4's question.
        for r in &spps.point_receivers {
            let rows: Vec<&serde_json::Value> =
                early.iter().filter(|x| x["receiver"] == r.label).collect();
            let share = rows
                .iter()
                .map(|x| x["direct_3_steps_share"].as_f64().unwrap())
                .fold(0.0, f64::max);
            let t10 = rows
                .iter()
                .filter_map(|x| x["to_minus_10_db_s"].as_f64())
                .fold(f64::INFINITY, f64::min);
            println!(
                "  {set} {}: the direct sound (onset and 2 steps) holds up to {:.1} % of a pair-band's energy; \
                 the shortest time to -10 dB {:.3} s",
                r.label,
                100.0 * share,
                t10
            );
        }

        let bounds = |n: usize| auralize::step_bounds(n, dt, SAMPLE_RATE);
        let mut per_bank = BTreeMap::new();
        let nseeds: usize = std::env::var("SIMPA_AURAL_SEEDS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(SEEDS.len());
        for (si, &seed) in SEEDS.iter().enumerate().take(nseeds) {
            // The per-source responses, once per seed.
            let mut synth = Vec::new();
            for (ri, r) in spps.point_receivers.iter().enumerate() {
                for (ei, e) in r.echograms.iter().enumerate() {
                    let resp =
                        auralize::response(&run, &r.label, Pick::Source(&e.source), seed).unwrap();
                    synth.push((ri, ei, resp));
                }
            }
            if si == 0 {
                let wav_dir = out.join(&set);
                std::fs::create_dir_all(&wav_dir).unwrap();
                let write = |name: String, resp: &auralize::Response| {
                    let mut x = resp.synthesis.samples.clone();
                    let g = auralize::normalise(&mut x);
                    let bytes = wav::write(
                        &x,
                        SAMPLE_RATE,
                        wav::SampleFormat::F32,
                        &resp.comment(g, "impulse response"),
                    )
                    .unwrap();
                    std::fs::write(wav_dir.join(name), bytes).unwrap();
                };
                for r in &spps.point_receivers {
                    let resp = auralize::response(&run, &r.label, Pick::Summed, seed).unwrap();
                    write(format!("{}-summed-ir.wav", r.label), &resp);
                }
                for (ri, ei, resp) in &synth {
                    if *ri == 0 {
                        let r = &spps.point_receivers[*ri];
                        write(
                            format!("{}-{}-ir.wav", r.label, r.echograms[*ei].source),
                            resp,
                        );
                    }
                }
            }
            for (bank, order) in BANKS {
                let mut copy = run.clone();
                let SolverResults::Spps(s) = &mut copy.data else {
                    unreachable!()
                };
                for (ri, ei, resp) in &synth {
                    let x = &resp.synthesis.samples;
                    let anal = auralize::analyse_butterworth(x, &filters, SAMPLE_RATE, order);
                    let e = &mut s.point_receivers[*ri].echograms[*ei];
                    let steps = e.energy[0].len();
                    for (b, a) in anal.iter().enumerate() {
                        let mut en = auralize::step_energies(a, &bounds(steps));
                        // The FFT's round-off leaves a floor some 300 dB down where the response
                        // is silent; the echogram has exact zeros there, and EDT v2.1 refuses a
                        // series still holding energy at the run's end. Steps more than
                        // FLOOR_DB below the band's largest are that floor: zeroed.
                        let max = en.iter().copied().fold(0.0, f64::max);
                        for v in &mut en {
                            if *v < max * 10f64.powf(FLOOR_DB / 10.0) {
                                *v = 0.0;
                            }
                        }
                        e.energy[b] = en;
                    }
                }
                let got = values(&report(&copy));
                let entry = per_bank.entry(bank).or_insert_with(|| {
                    json!({"seeds": [], "worst_edt": vec![0.0; bands.len()], "worst_t30": vec![0.0; bands.len()]})
                });
                let (mut within, mut total, mut unmeasured) = (0, 0, Vec::new());
                for (key, (e0, t0)) in &orig {
                    let (e1, t1) = &got[key];
                    let b = bands.iter().position(|&h| h == key.2).unwrap();
                    for (q, a, z) in [("EDT", e0, e1), ("T30", t0, t1)] {
                        let (Ok(a), Ok(z)) = (a, z) else {
                            unmeasured.push(format!(
                                "{} {} {} Hz {q}: echogram {a:?}, response {z:?}",
                                key.0, key.1, key.2
                            ));
                            continue;
                        };
                        let d = z / a - 1.0;
                        total += 1;
                        let worst = if q == "EDT" { "worst_edt" } else { "worst_t30" };
                        let w = &mut entry[worst][b];
                        if d.abs() > w.as_f64().unwrap() {
                            *w = json!(d.abs());
                        }
                        if d.abs() <= JND {
                            within += 1;
                            continue;
                        }
                        let sign: i8 = if d < 0.0 { -1 } else { 1 };
                        let line = format!(
                            "{bank} {set} seed{si} {} {} {} Hz {q} {:+.2} %",
                            key.0,
                            key.1,
                            key.2,
                            100.0 * d
                        );
                        let known = KNOWN.iter().find(|m| {
                            (m.0, m.1, m.2, m.3, m.4, m.5, m.6, m.7)
                                == (
                                    bank,
                                    set.as_str(),
                                    si,
                                    key.0.as_str(),
                                    key.1.as_str(),
                                    key.2,
                                    q,
                                    sign,
                                )
                        });
                        match known {
                            None => guard_failures.push(format!("new miss: {line}")),
                            Some(m) if d.abs() > GUARD.max(m.8 + 0.001) => guard_failures.push(
                                format!("known miss grown past {:.4}: {line}", GUARD.max(m.8)),
                            ),
                            Some(_) => {}
                        }
                        all_misses.push(line);
                    }
                }
                println!(
                    "{bank} {set} seed{si} ({seed:#x}): {within} of {total} within 5 %; {} not measured",
                    unmeasured.len()
                );
                entry["seeds"].as_array_mut().unwrap().push(json!({
                    "seed": format!("{seed:#018x}"), "within": within, "of": total,
                    "unmeasured": unmeasured,
                }));
            }
        }
        for (bank, e) in &per_bank {
            let fmt = |k: &str| -> String {
                bands
                    .iter()
                    .zip(e[k].as_array().unwrap())
                    .map(|(h, v)| format!("{h}: {:.2}", 100.0 * v.as_f64().unwrap()))
                    .collect::<Vec<_>>()
                    .join(" · ")
            };
            println!(
                "  {bank} {set} worst |dEDT| % per band: {}",
                fmt("worst_edt")
            );
            println!(
                "  {bank} {set} worst |dT30| % per band: {}",
                fmt("worst_t30")
            );
        }
        receipt.push(json!({
            "set": set, "run": run.folder,
            "bands": filters.bands.iter().map(|b| json!({"centre_hz": b.centre_hz, "lo_hz": b.lo_hz, "hi_hz": b.hi_hz})).collect::<Vec<_>>(),
            "estimator": "results::report (simpa results): EDT v2.1 and T30, per source-receiver pair",
            "agreement_decay_times_vs_report": {"edt_max_rel": agree_edt, "t30_max_rel": agree_t30},
            "early": early,
            "banks": per_bank,
        }));
    }
    println!("{} outside the JND:", all_misses.len());
    for m in &all_misses {
        println!("  {m}");
    }
    std::fs::write(
        out.join("receipt.json"),
        serde_json::to_string_pretty(&json!({
            "jnd": JND, "guard": GUARD, "backlog": 99,
            "banks": {"iec": format!("Butterworth prototype order {IEC_PROTOTYPE_ORDER}, band-pass order {}", 2 * IEC_PROTOTYPE_ORDER),
                      "steep": format!("Butterworth prototype order {STEEP_PROTOTYPE_ORDER}, band-pass order {}", 2 * STEEP_PROTOTYPE_ORDER)},
            "sets": receipt, "misses": all_misses, "guard_failures": guard_failures,
        }))
        .unwrap(),
    )
    .unwrap();
    assert!(
        guard_failures.is_empty(),
        "backlog 99: {} outside what is known:\n{}",
        guard_failures.len(),
        guard_failures.join("\n")
    );
}
