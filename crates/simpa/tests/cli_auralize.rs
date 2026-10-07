//! `simpa auralize` (C5, done-when 2) on the committed SPPS fixtures: `results/sources2_spps`
//! (two sources, an echogram per source, octaves 500 and 1000 Hz) and `results/seats_spps` (no
//! echogram per source). The fixtures are only read; every WAV goes to a scratch folder.

mod support;

use std::path::{Path, PathBuf};

use simpa_core::auralize::{SAMPLE_RATE, SEED, wav};
use support::*;

fn fixture(rel: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/results")
        .join(rel)
}

fn auralize(run: &Path, extra: &[&str]) -> Out {
    let mut args = vec!["auralize".to_string(), run.display().to_string()];
    args.extend(extra.iter().map(|s| s.to_string()));
    simpa_run(&args)
}

fn read(p: &Path) -> wav::Wav {
    wav::read(&std::fs::read(p).unwrap()).unwrap()
}

#[test]
fn the_impulse_response_is_48_khz_the_runs_length_normalised_and_the_same_each_time() {
    let dir = scratch("aural-ir");
    let run = fixture("sources2_spps");
    let a = dir.join("a.wav");
    let o = auralize(&run, &["--receiver", "Seat", "--out", a.to_str().unwrap()]);
    assert_eq!(o.code, 0, "{o:#?}");
    assert!(o.stdout.starts_with("impulse response: "), "{}", o.stdout);
    let w = read(&a);
    assert_eq!(
        (w.rate, w.channels, w.bits, w.float),
        (SAMPLE_RATE, 1, 32, true)
    );
    // The run's duration at 48 kHz: duree_simulation / pasdetemps steps of pasdetemps · 48 kHz samples.
    let cfg = std::fs::read_to_string(run.join("solve/config.xml")).unwrap();
    let attr = |n: &str| -> f64 {
        let i = cfg.find(&format!("{n}=\"")).unwrap() + n.len() + 2;
        cfg[i..i + cfg[i..].find('"').unwrap()].parse().unwrap()
    };
    let steps = (attr("duree_simulation") / attr("pasdetemps")).round() as usize;
    let per_step = (attr("pasdetemps") * f64::from(SAMPLE_RATE)).round() as usize;
    assert_eq!(w.samples.len(), steps * per_step);
    let peak = w.samples.iter().fold(0.0f64, |m, v| m.max(v.abs()));
    assert_eq!(peak, f64::from(10f64.powf(-1.0 / 20.0) as f32));
    let comment = w.comment.unwrap();
    assert!(comment.contains(&format!("{SEED:#018x}")), "{comment}");
    assert!(comment.contains("not a measured or wave-based impulse response"));
    assert!(comment.contains("all sources summed"));
    // The same run, the same bytes.
    let b = dir.join("b.wav");
    assert_eq!(
        auralize(
            &run,
            &[
                "--receiver",
                "Seat",
                "--summed",
                "--out",
                b.to_str().unwrap()
            ]
        )
        .code,
        0
    );
    assert_eq!(std::fs::read(&a).unwrap(), std::fs::read(&b).unwrap());
    // One source alone is another response; 24-bit on request.
    let c = dir.join("c.wav");
    let o = auralize(
        &run,
        &[
            "--receiver",
            "Seat",
            "--source",
            "Source 1",
            "--pcm24",
            "--out",
            c.to_str().unwrap(),
        ],
    );
    assert_eq!(o.code, 0, "{o:#?}");
    let wc = read(&c);
    assert_eq!((wc.bits, wc.float), (24, false));
    assert!(wc.comment.unwrap().contains("source Source 1"));
    assert_ne!(wc.samples.len(), 0);
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn an_anechoic_recording_at_44_1_khz_is_resampled_and_convolved() {
    let dir = scratch("aural-conv");
    let run = fixture("sources2_spps");
    let dry: Vec<f64> = (0..22_050)
        .map(|n| 0.5 * (2.0 * std::f64::consts::PI * 700.0 * n as f64 / 44_100.0).sin())
        .collect();
    let dry_path = dir.join("dry.wav");
    std::fs::write(
        &dry_path,
        wav::write(&dry, 44_100, wav::SampleFormat::Pcm24, "").unwrap(),
    )
    .unwrap();
    let out = dir.join("wet.wav");
    let o = auralize(
        &run,
        &[
            "--receiver",
            "Seat2",
            "--anechoic",
            dry_path.to_str().unwrap(),
            "--out",
            out.to_str().unwrap(),
        ],
    );
    assert_eq!(o.code, 0, "{o:#?}");
    assert!(o.stdout.starts_with("auralization: "), "{}", o.stdout);
    let w = read(&out);
    assert_eq!(w.rate, SAMPLE_RATE);
    // At least the dry signal's 0.5 s at 48 kHz, at most that plus the response.
    assert!(w.samples.len() >= 24_000, "{}", w.samples.len());
    let ir = dir.join("ir.wav");
    auralize(
        &run,
        &["--receiver", "Seat2", "--out", ir.to_str().unwrap()],
    );
    assert!(w.samples.len() < 24_000 + read(&ir).samples.len());
    let peak = w.samples.iter().fold(0.0f64, |m, v| m.max(v.abs()));
    assert_eq!(peak, f64::from(10f64.powf(-1.0 / 20.0) as f32));
    assert!(w.comment.unwrap().contains("at 44100 Hz"));
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn refusals_and_usage() {
    let dir = scratch("aural-no");
    let out = dir.join("x.wav");
    let o = out.to_str().unwrap();
    let r = auralize(
        &fixture("sources2_spps"),
        &["--receiver", "Nowhere", "--out", o],
    );
    assert_eq!(r.code, 2);
    assert!(r.stderr.contains("aural_no_receiver"), "{}", r.stderr);
    let r = auralize(
        &fixture("seats_spps"),
        &["--receiver", "Seat", "--source", "Source", "--out", o],
    );
    assert_eq!(r.code, 2);
    assert!(
        r.stderr.contains("aural_no_echograms") || r.stderr.contains("aural_no_source"),
        "{}",
        r.stderr
    );
    let r = auralize(&fixture("sources2_spps"), &["--receiver", "Seat"]);
    assert_eq!(r.code, 2, "no --out");
    let r = auralize(
        &fixture("sources2_spps"),
        &[
            "--receiver",
            "Seat",
            "--summed",
            "--source",
            "Source 1",
            "--out",
            o,
        ],
    );
    assert_eq!(r.code, 2);
    let r = auralize(&fixture("seats_tcr"), &["--receiver", "Seat", "--out", o]);
    assert_eq!(r.code, 2);
    assert!(r.stderr.contains("aural_not_spps"), "{}", r.stderr);
    assert!(!out.exists(), "nothing is written on a refusal");
    std::fs::remove_dir_all(&dir).unwrap();
}
