//! `simpa auralize <run-folder> --receiver <name> [--source <name> | --summed]
//! [--source-audio <wav>] --out <wav> [--pcm24] [--seed <n>]` (C5, `simpa_core::auralize`).
//!
//! Without `--source-audio` (`--anechoic` is kept as an alias) it writes the receiver's impulse response, synthesised from the SPPS
//! energy echogram; with it, that recording (best dry or anechoic) convolved with that response (resampled to
//! 48 kHz first when it is at another rate). Either is mono, 48 kHz, peak-normalised to −1 dBFS,
//! 32-bit float (or 24-bit PCM with `--pcm24`), and carries in its `INFO/ICMT` comment what it is,
//! the noise seed and the gain back to the run's absolute scale. The sources summed unless
//! `--source` names one. One line on stdout says what was written.
//!
//! Exit codes: 0 written; 2 usage, or an auralization refused (`simpa: auralize refused: <code>:
//! <detail>`); 5 and 6 as `simpa results` for a run that did not succeed or does not verify.

use std::path::Path;
use std::process::ExitCode;

use simpa_core::auralize::{self, Pick, SAMPLE_RATE, SEED, wav};
use simpa_core::results;

use crate::fail;

pub fn auralize_cmd(args: &[&str]) -> ExitCode {
    let mut folder = None;
    let mut receiver = None;
    let mut source = None;
    let mut summed = false;
    let mut dry_audio = None;
    let mut out = None;
    let mut pcm24 = false;
    let mut seed = SEED;
    let mut it = args.iter().copied();
    while let Some(a) = it.next() {
        let mut value = |name: &str| it.next().ok_or_else(|| format!("{name} needs a value"));
        let r = match a {
            "--receiver" => value(a).map(|v| receiver = Some(v)),
            "--source" => value(a).map(|v| source = Some(v)),
            "--summed" => {
                summed = true;
                Ok(())
            }
            "--source-audio" | "--anechoic" => value(a).map(|v| dry_audio = Some(v)),
            "--out" => value(a).map(|v| out = Some(v)),
            "--pcm24" => {
                pcm24 = true;
                Ok(())
            }
            "--seed" => value(a).and_then(|v| {
                let t = v.trim_start_matches("0x");
                let parsed = if v.starts_with("0x") {
                    u64::from_str_radix(t, 16)
                } else {
                    v.parse()
                };
                parsed
                    .map(|s| seed = s)
                    .map_err(|_| format!("--seed '{v}' is not an unsigned integer"))
            }),
            _ if a.starts_with("--") => Err(format!("unknown option '{a}'")),
            _ if folder.is_none() => {
                folder = Some(a);
                Ok(())
            }
            _ => Err(format!("unexpected argument '{a}'")),
        };
        if let Err(e) = r {
            return fail(&e);
        }
    }
    let usage = "auralize needs: simpa auralize <run-folder> --receiver <name> [--source <name> | \
                 --summed] [--source-audio <wav>] --out <wav> [--pcm24] [--seed <n>]";
    let (Some(folder), Some(receiver), Some(out)) = (folder, receiver, out) else {
        return fail(usage);
    };
    if summed && source.is_some() {
        return fail("--source and --summed are exclusive");
    }
    let folder = Path::new(folder);
    if !folder.is_dir() {
        return fail(&format!("{} is not a folder", folder.display()));
    }
    let run = match results::load(folder) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("simpa: results refused: {e}");
            return ExitCode::from(e.exit_code());
        }
    };
    let refused = |e: auralize::AuralError| {
        eprintln!("simpa: auralize refused: {e}");
        ExitCode::from(2)
    };
    let pick = source.map_or(Pick::Summed, Pick::Source);
    let resp = match auralize::response(&run, receiver, pick, seed) {
        Ok(r) => r,
        Err(e) => return refused(e),
    };
    let (mut samples, what, dry) = match dry_audio {
        None => (resp.synthesis.samples.clone(), "impulse response", None),
        Some(path) => {
            let bytes = match std::fs::read(path) {
                Ok(b) => b,
                Err(e) => return fail(&format!("{path}: {e}")),
            };
            let (w, x) = match auralize::dry_recording(&bytes) {
                Ok(v) => v,
                Err(e) => return refused(e),
            };
            (
                auralize::auralize(&resp.synthesis.samples, &x),
                "auralization",
                Some((path, w.rate, w.channels, x.len())),
            )
        }
    };
    let gain = auralize::normalise(&mut samples);
    let mut comment = resp.comment(gain, what);
    if let Some((path, rate, channels, _)) = dry {
        let name = Path::new(path)
            .file_name()
            .map_or_else(|| path.to_string(), |n| n.to_string_lossy().into_owned());
        comment.push_str(&format!(
            "; convolved with {name} ({channels} channel(s) at {rate} Hz, mixed to mono and resampled to {SAMPLE_RATE} Hz), the gain applied after the convolution"
        ));
    }
    let format = if pcm24 {
        wav::SampleFormat::Pcm24
    } else {
        wav::SampleFormat::F32
    };
    let bytes = match wav::write(&samples, SAMPLE_RATE, format, &comment) {
        Ok(b) => b,
        Err(e) => return refused(e),
    };
    if let Err(e) = std::fs::write(out, &bytes) {
        return fail(&format!("{out}: {e}"));
    }
    println!(
        "{what}: {out}, {} samples ({:.3} s) at {SAMPLE_RATE} Hz, {}, receiver {receiver}, {}, seed {seed:#018x}, gain {gain:.6e}",
        samples.len(),
        samples.len() as f64 / f64::from(SAMPLE_RATE),
        if pcm24 { "24-bit PCM" } else { "32-bit float" },
        source.map_or("all sources summed".to_string(), |s| format!("source {s}")),
    );
    ExitCode::SUCCESS
}
