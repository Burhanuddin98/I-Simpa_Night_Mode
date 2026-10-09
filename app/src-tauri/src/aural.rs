//! C5: the Results step's listening (decision 75; `docs/investigations/2026-10-07-auralization/`).
//! [`auralize_bytes`] answers `run_auralize`: a receiver's impulse response synthesised from the
//! run's SPPS energy echogram (`simpa_core::auralize`), peak −1 dBFS; or a dry clip through the
//! room (convolved with it) or the same clip without it, as a WAV the UI plays and saves: mono,
//! 48 kHz, 32-bit float, with the seed and the gain to the run's scale in its comment.
//!
//! Dry and Room share one loudness reference (decision 84, [`matched_gains`]), so the Listen
//! window's switch between them changes what the room adds, not the level: the room version is
//! scaled to carry the dry clip's total energy, and both by one gain that puts the louder peak of
//! the two at −1 dBFS. The convolution itself is `simpa_core::auralize::auralize`, unchanged.
//!
//! The clips offered are [`CLIPS`] (decision 84): three made by this repository's own code
//! (`tools/clips/make_dry_clips.py`: claps and rim clicks, a plucked-string melody, a drum groove),
//! anechoic by construction and GPL-3.0 like the app, each event's own tail measured in its
//! provenance; and one recording, speech, dry and close-miked but not anechoic, so the room it was
//! recorded in adds a little. They are embedded with `include_bytes!` as the examples are
//! (`examples.rs`); each has a provenance file beside it in `examples/clips/` (how it was made, or
//! source, author and licence quoted; its sha256), and `examples/ATTRIBUTION.md` lists them. The UI's list
//! (`ui/src/features/acoustics/aural.ts`) is held to this one by its test. "Open your own WAV…"
//! passes a path instead.
//!
//! The last response is kept (run, receiver, source), so Auralize after Play does not synthesise
//! it again.

use std::path::Path;
use std::sync::Mutex;

use simpa_core::auralize::{self, AuralError, Pick, SAMPLE_RATE, SEED, wav};

use crate::guard::{CmdError, CmdResult};

/// One shipped clip: generated (anechoic by construction) or a dry recording (module docs).
pub struct Clip {
    /// The id the UI asks for.
    pub id: &'static str,
    /// The WAV as shipped: mono, 48 kHz, 16-bit.
    pub bytes: &'static [u8],
}

/// Every shipped recording, in the UI's order.
pub const CLIPS: &[Clip] = &[
    Clip {
        id: "clap-pattern",
        bytes: include_bytes!("../examples/clips/clap-pattern.wav"),
    },
    Clip {
        id: "pluck-melody",
        bytes: include_bytes!("../examples/clips/pluck-melody.wav"),
    },
    Clip {
        id: "drum-groove",
        bytes: include_bytes!("../examples/clips/drum-groove.wav"),
    },
    Clip {
        id: "speech-lv-hislastbow",
        bytes: include_bytes!("../examples/clips/speech-lv-hislastbow.wav"),
    },
];

type Key = (String, String, String, Option<String>);
static LAST: Mutex<Option<(Key, auralize::Response)>> = Mutex::new(None);

fn refused(e: AuralError) -> CmdError {
    CmdError::new(e.code().to_ascii_uppercase(), e.detail().to_string())
}

/// What the UI asked to hear: the response alone, a shipped clip, or a file; the clip and the
/// file through the room or without it (`room`).
pub enum Dry<'a> {
    None,
    Clip(&'a str),
    File(&'a str),
}

/// The gains that put a dry clip `x` and its room version `y` on one loudness reference (module
/// docs): `(g, g k)`, with `k` scaling `y` to `x`'s total energy and `g` putting the louder peak
/// of `x` and `k y` at [`auralize::PEAK_DBFS`]. A silent `y` gets `k` 1, a silent pair gain 1.
pub fn matched_gains(x: &[f64], y: &[f64]) -> (f64, f64) {
    let energy = |v: &[f64]| v.iter().map(|s| s * s).sum::<f64>();
    let peak = |v: &[f64]| v.iter().fold(0.0_f64, |m, s| m.max(s.abs()));
    let ey = energy(y);
    let k = if ey > 0.0 {
        (energy(x) / ey).sqrt()
    } else {
        1.0
    };
    let top = peak(x).max(k * peak(y));
    let g = if top > 0.0 {
        10f64.powf(auralize::PEAK_DBFS / 20.0) / top
    } else {
        1.0
    };
    (g, g * k)
}

/// The words the WAV's comment ends with for a clip or a file at the shared reference.
const MATCHED: &str = "Dry and Room carry the same energy: the room version is scaled to the dry clip's                        total energy, and both by one gain that puts the louder peak of the two at -1 dBFS";

/// The WAV for `run`'s receiver `receiver`, the sources summed or `source` alone (module docs);
/// for a clip or a file, through the room when `room`, else the clip alone at the shared level.
pub fn auralize_bytes(
    root: &Path,
    run: &str,
    receiver: &str,
    source: Option<&str>,
    dry: Dry<'_>,
    room: bool,
) -> CmdResult<Vec<u8>> {
    let key: Key = (
        root.display().to_string(),
        run.to_string(),
        receiver.to_string(),
        source.map(str::to_string),
    );
    let cached = LAST.lock().ok().and_then(|g| {
        g.as_ref()
            .filter(|(k, _)| *k == key)
            .map(|(_, r)| r.clone())
    });
    let resp = match cached {
        Some(r) => r,
        None => {
            let results = crate::results_data::loaded(root, run)?;
            let pick = source.map_or(Pick::Summed, Pick::Source);
            let r = auralize::response(&results, receiver, pick, SEED).map_err(refused)?;
            if let Ok(mut g) = LAST.lock() {
                *g = Some((key, r.clone()));
            }
            r
        }
    };
    let (x, what) = match dry {
        Dry::None => {
            let mut samples = resp.synthesis.samples.clone();
            let gain = auralize::normalise(&mut samples);
            let comment = resp.comment(gain, "impulse response");
            return wav::write(&samples, SAMPLE_RATE, wav::SampleFormat::F32, &comment)
                .map_err(refused);
        }
        Dry::Clip(id) => {
            let clip = CLIPS
                .iter()
                .find(|c| c.id == id)
                .ok_or_else(|| CmdError::new("CLIP_UNKNOWN", format!("no bundled clip '{id}'")))?;
            let (_, x) = auralize::dry_recording(clip.bytes).map_err(refused)?;
            (x, format!("the shipped clip {id}"))
        }
        Dry::File(path) => {
            let bytes = std::fs::read(path)
                .map_err(|e| CmdError::new("WAV_READ", format!("could not read '{path}': {e}")))?;
            let (w, x) = auralize::dry_recording(&bytes).map_err(refused)?;
            let name = Path::new(path)
                .file_name()
                .map_or_else(|| path.to_string(), |n| n.to_string_lossy().into_owned());
            (
                x,
                format!(
                    "{name} ({} channel(s) at {} Hz, mixed to mono and resampled to {SAMPLE_RATE} Hz)",
                    w.channels, w.rate
                ),
            )
        }
    };
    let y = auralize::auralize(&resp.synthesis.samples, &x);
    let (g_dry, g_room) = matched_gains(&x, &y);
    let (samples, comment) = if room {
        let comment = format!(
            "{}; convolved with {what}; {MATCHED}",
            resp.comment(g_room, "auralization")
        );
        (y.iter().map(|v| v * g_room).collect::<Vec<_>>(), comment)
    } else {
        let comment = format!(
            "I-Simpa Night Mode dry clip: {what}, not through the room; samples are the clip times              {g_dry:.6e}, the level it shares with its room version at receiver {} ({}); {MATCHED}",
            resp.receiver,
            resp.source
                .as_deref()
                .map_or("all sources summed".to_string(), |s| format!("source {s}")),
        );
        (x.iter().map(|v| v * g_dry).collect::<Vec<_>>(), comment)
    };
    wav::write(&samples, SAMPLE_RATE, wav::SampleFormat::F32, &comment).map_err(refused)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dry_and_room_share_one_energy_and_the_louder_peak_is_minus_1_dbfs() {
        let x: Vec<f64> = (0..4800)
            .map(|i| if i % 1200 < 48 { 0.5 } else { 0.0 })
            .collect();
        // A decaying "room": the convolution spreads each burst, lowering its peak.
        let h: Vec<f64> = (0..9600)
            .map(|i| 0.9 * (-(i as f64) / 1500.0).exp() * if i % 7 == 0 { 1.0 } else { -0.3 })
            .collect();
        let y = auralize::auralize(&h, &x);
        let (gd, gr) = matched_gains(&x, &y);
        let e = |v: &[f64], g: f64| v.iter().map(|s| (s * g).powi(2)).sum::<f64>();
        assert!(
            (e(&x, gd) / e(&y, gr) - 1.0).abs() < 1e-12,
            "the same energy"
        );
        let p = |v: &[f64], g: f64| v.iter().fold(0.0_f64, |m, s| m.max((s * g).abs()));
        let top = p(&x, gd).max(p(&y, gr));
        assert!(
            (20.0 * top.log10() - auralize::PEAK_DBFS).abs() < 1e-9,
            "{top}"
        );
        assert!(p(&x, gd) <= top && p(&y, gr) <= top);
        // Control: a silent room keeps the dry gain finite and the pair on the dry clip's peak.
        let (gd0, gr0) = matched_gains(&x, &[0.0; 10]);
        assert!((20.0 * (0.5 * gd0).log10() - auralize::PEAK_DBFS).abs() < 1e-9);
        assert!(gr0.is_finite());
        assert_eq!(matched_gains(&[0.0; 4], &[0.0; 4]), (1.0, 1.0));
    }

    #[test]
    fn every_clip_is_a_short_mono_48_khz_wav_and_together_under_5_mb() {
        let mut total = 0;
        for c in CLIPS {
            let w = wav::read(c.bytes).unwrap_or_else(|e| panic!("{}: {e}", c.id));
            assert_eq!((w.rate, w.channels), (SAMPLE_RATE, 1), "{}", c.id);
            let secs = w.samples.len() as f64 / f64::from(w.rate);
            assert!((2.0..=20.0).contains(&secs), "{}: {secs} s", c.id);
            total += c.bytes.len();
            let prov = std::fs::read_to_string(
                Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("examples/clips")
                    .join(format!("{}.provenance.md", c.id)),
            )
            .unwrap();
            // Generated: the script and its measured dryness; recorded: where it came from.
            let words: &[&str] = if prov.contains("- Made by: tools/clips/make_dry_clips.py") {
                &[
                    "Licence: GPL-3.0",
                    "sha256",
                    "seed",
                    "Dryness: anechoic by construction",
                ]
            } else {
                &[
                    "Licence",
                    "sha256",
                    "Source page",
                    "Date fetched",
                    "NOT anechoic",
                ]
            };
            for word in words {
                assert!(prov.contains(word), "{}: no {word}", c.id);
            }
        }
        assert!(total < 5 * 1024 * 1024, "{total} bytes");
    }
}
