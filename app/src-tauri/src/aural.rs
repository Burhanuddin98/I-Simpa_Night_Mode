//! C5: the Results step's listening (decision 75; `docs/investigations/2026-10-07-auralization/`).
//! [`auralize_bytes`] answers `run_auralize`: a receiver's impulse response synthesised from the
//! run's SPPS energy echogram (`simpa_core::auralize`), or a dry recording convolved with it,
//! as a WAV the UI plays and saves: mono, 48 kHz, 32-bit float, peak −1 dBFS, with the seed and the
//! gain to the run's scale in its comment.
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

/// What the UI asked to hear: the response alone, a shipped clip through it, or a file.
pub enum Dry<'a> {
    None,
    Clip(&'a str),
    File(&'a str),
}

/// The WAV for `run`'s receiver `receiver`, the sources summed or `source` alone (module docs).
pub fn auralize_bytes(
    root: &Path,
    run: &str,
    receiver: &str,
    source: Option<&str>,
    dry: Dry<'_>,
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
    let (mut samples, what, note) = match dry {
        Dry::None => (resp.synthesis.samples.clone(), "impulse response", None),
        Dry::Clip(id) => {
            let clip = CLIPS
                .iter()
                .find(|c| c.id == id)
                .ok_or_else(|| CmdError::new("CLIP_UNKNOWN", format!("no bundled clip '{id}'")))?;
            let (_, x) = auralize::dry_recording(clip.bytes).map_err(refused)?;
            (
                auralize::auralize(&resp.synthesis.samples, &x),
                "auralization",
                Some(format!("convolved with the shipped clip {id}")),
            )
        }
        Dry::File(path) => {
            let bytes = std::fs::read(path)
                .map_err(|e| CmdError::new("WAV_READ", format!("could not read '{path}': {e}")))?;
            let (w, x) = auralize::dry_recording(&bytes).map_err(refused)?;
            let name = Path::new(path)
                .file_name()
                .map_or_else(|| path.to_string(), |n| n.to_string_lossy().into_owned());
            (
                auralize::auralize(&resp.synthesis.samples, &x),
                "auralization",
                Some(format!(
                    "convolved with {name} ({} channel(s) at {} Hz, mixed to mono and resampled to {SAMPLE_RATE} Hz)",
                    w.channels, w.rate
                )),
            )
        }
    };
    let gain = auralize::normalise(&mut samples);
    let mut comment = resp.comment(gain, what);
    if let Some(n) = note {
        comment.push_str("; ");
        comment.push_str(&n);
    }
    wav::write(&samples, SAMPLE_RATE, wav::SampleFormat::F32, &comment).map_err(refused)
}

#[cfg(test)]
mod tests {
    use super::*;

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
                &["Licence: GPL-3.0", "sha256", "seed", "Dryness: anechoic by construction"]
            } else {
                &["Licence", "sha256", "Source page", "Date fetched", "NOT anechoic"]
            };
            for word in words {
                assert!(prov.contains(word), "{}: no {word}", c.id);
            }
        }
        assert!(total < 5 * 1024 * 1024, "{total} bytes");
    }
}
