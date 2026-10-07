//! WAV files: written mono, 32-bit float or 24-bit PCM, with a `LIST/INFO` comment that records
//! what the file is (the seed, the run, the gain to pascals); read as PCM 8/16/24/32-bit or IEEE
//! float 32/64-bit, plain or `WAVE_FORMAT_EXTENSIBLE`, any channel count, mixed down to mono.

use super::AuralError;

/// How the samples are stored.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SampleFormat {
    /// IEEE float, 32-bit (`WAVE_FORMAT_IEEE_FLOAT`, with the `fact` chunk the format asks for).
    F32,
    /// Signed integer PCM, 24-bit. A sample outside [âˆ’1, 1] is refused, never clipped.
    Pcm24,
}

const FORMAT_PCM: u16 = 1;
const FORMAT_FLOAT: u16 = 3;
const FORMAT_EXTENSIBLE: u16 = 0xFFFE;

fn chunk(out: &mut Vec<u8>, id: &[u8; 4], body: &[u8]) {
    out.extend_from_slice(id);
    out.extend_from_slice(&(body.len() as u32).to_le_bytes());
    out.extend_from_slice(body);
    if body.len() % 2 == 1 {
        out.push(0);
    }
}

/// A mono WAV of `samples` at `rate`, with `comment` as its `INFO/ICMT` (none when empty).
/// [`SampleFormat::Pcm24`] refuses a sample that is not finite or lies outside [âˆ’1, 1]
/// (`aural_would_clip`); [`SampleFormat::F32`] refuses one that is not finite.
pub fn write(
    samples: &[f64],
    rate: u32,
    format: SampleFormat,
    comment: &str,
) -> Result<Vec<u8>, AuralError> {
    if let Some(i) = samples.iter().position(|v| !v.is_finite()) {
        return Err(AuralError::new(
            "aural_bad_sample",
            format!("sample {i} is {}", samples[i]),
        ));
    }
    if format == SampleFormat::Pcm24
        && let Some(i) = samples.iter().position(|v| v.abs() > 1.0)
    {
        return Err(AuralError::new(
            "aural_would_clip",
            format!(
                "sample {i} is {}, outside [-1, 1]: 24-bit PCM would clip it; normalise first",
                samples[i]
            ),
        ));
    }
    let (tag, bits): (u16, u16) = match format {
        SampleFormat::F32 => (FORMAT_FLOAT, 32),
        SampleFormat::Pcm24 => (FORMAT_PCM, 24),
    };
    let block = bits / 8;
    let mut fmt = Vec::with_capacity(18);
    fmt.extend_from_slice(&tag.to_le_bytes());
    fmt.extend_from_slice(&1u16.to_le_bytes());
    fmt.extend_from_slice(&rate.to_le_bytes());
    fmt.extend_from_slice(&(rate * u32::from(block)).to_le_bytes());
    fmt.extend_from_slice(&block.to_le_bytes());
    fmt.extend_from_slice(&bits.to_le_bytes());
    if format == SampleFormat::F32 {
        fmt.extend_from_slice(&0u16.to_le_bytes());
    }
    let mut data = Vec::with_capacity(samples.len() * usize::from(block));
    for &v in samples {
        match format {
            SampleFormat::F32 => data.extend_from_slice(&(v as f32).to_le_bytes()),
            SampleFormat::Pcm24 => {
                let q = (v * 8_388_608.0).round().clamp(-8_388_608.0, 8_388_607.0) as i32;
                data.extend_from_slice(&q.to_le_bytes()[..3]);
            }
        }
    }
    let mut body = Vec::with_capacity(data.len() + 256);
    body.extend_from_slice(b"WAVE");
    chunk(&mut body, b"fmt ", &fmt);
    if format == SampleFormat::F32 {
        chunk(&mut body, b"fact", &(samples.len() as u32).to_le_bytes());
    }
    if !comment.is_empty() {
        let mut text = comment.as_bytes().to_vec();
        text.push(0);
        let mut info = b"INFO".to_vec();
        chunk(&mut info, b"ICMT", &text);
        chunk(&mut body, b"LIST", &info);
    }
    chunk(&mut body, b"data", &data);
    let mut out = Vec::with_capacity(body.len() + 8);
    chunk(&mut out, b"RIFF", &body);
    Ok(out)
}

/// A WAV as read: its rate, its channel count and bit depth as stored, and its samples mixed down
/// to mono (the mean of the channels), in [âˆ’1, 1] for integer PCM.
#[derive(Clone, Debug, PartialEq)]
pub struct Wav {
    pub rate: u32,
    pub channels: u16,
    pub bits: u16,
    pub float: bool,
    pub samples: Vec<f64>,
    /// `INFO/ICMT`, when the file has one.
    pub comment: Option<String>,
}

fn bad(what: impl Into<String>) -> AuralError {
    AuralError::new("aural_wav_invalid", what.into())
}

fn u16_at(b: &[u8], o: usize) -> u16 {
    u16::from_le_bytes([b[o], b[o + 1]])
}

fn u32_at(b: &[u8], o: usize) -> u32 {
    u32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]])
}

/// Reads a RIFF/WAVE file (module docs); refuses anything else, `aural_wav_invalid`.
pub fn read(bytes: &[u8]) -> Result<Wav, AuralError> {
    if bytes.len() < 12 || &bytes[..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return Err(bad("not a RIFF/WAVE file"));
    }
    let mut o = 12;
    let mut fmt: Option<(u16, u16, u32, u16, u16)> = None;
    let mut data: Option<&[u8]> = None;
    let mut comment = None;
    while o + 8 <= bytes.len() {
        let id = &bytes[o..o + 4];
        let len = u32_at(bytes, o + 4) as usize;
        let start = o + 8;
        // A data chunk whose stated length runs past the file (a writer that never went back to
        // fill it in) is read to the end of the file.
        let end = start.saturating_add(len).min(bytes.len());
        let body = &bytes[start..end];
        match id {
            b"fmt " => {
                if body.len() < 16 {
                    return Err(bad("the fmt chunk is shorter than 16 bytes"));
                }
                let mut tag = u16_at(body, 0);
                let channels = u16_at(body, 2);
                let rate = u32_at(body, 4);
                let align = u16_at(body, 12);
                let bits = u16_at(body, 14);
                if tag == FORMAT_EXTENSIBLE {
                    if body.len() < 40 {
                        return Err(bad("an extensible fmt chunk shorter than 40 bytes"));
                    }
                    tag = u16_at(body, 24);
                }
                fmt = Some((tag, channels, rate, align, bits));
            }
            b"data" => data = Some(body),
            b"LIST" if body.len() >= 4 && &body[..4] == b"INFO" => {
                let mut p = 4;
                while p + 8 <= body.len() {
                    let l = u32_at(body, p + 4) as usize;
                    let s = (p + 8).min(body.len());
                    let e = (p + 8 + l).min(body.len());
                    if &body[p..p + 4] == b"ICMT" {
                        let t = String::from_utf8_lossy(&body[s..e]);
                        comment = Some(t.trim_end_matches('\0').to_string());
                    }
                    p = p + 8 + l + (l % 2);
                }
            }
            _ => {}
        }
        o = start.saturating_add(len).saturating_add(len % 2);
    }
    let (tag, channels, rate, align, bits) = fmt.ok_or_else(|| bad("no fmt chunk"))?;
    let data = data.ok_or_else(|| bad("no data chunk"))?;
    if channels == 0 || rate == 0 {
        return Err(bad(format!("{channels} channels at {rate} Hz")));
    }
    let float = match (tag, bits) {
        (FORMAT_PCM, 8 | 16 | 24 | 32) => false,
        (FORMAT_FLOAT, 32 | 64) => true,
        _ => {
            return Err(bad(format!(
                "format {tag} at {bits} bits: only PCM 8/16/24/32-bit and float 32/64-bit are read"
            )));
        }
    };
    let width = usize::from(bits / 8);
    let frame = width * usize::from(channels);
    if usize::from(align) != frame {
        return Err(bad(format!(
            "block align {align}, {channels} channels of {bits} bits need {frame}"
        )));
    }
    let frames = data.len() / frame;
    let mut samples = Vec::with_capacity(frames);
    for f in 0..frames {
        let mut sum = 0.0;
        for c in 0..usize::from(channels) {
            let s = &data[f * frame + c * width..f * frame + (c + 1) * width];
            let v = match (float, bits) {
                (false, 8) => (f64::from(s[0]) - 128.0) / 128.0,
                (false, 16) => f64::from(i16::from_le_bytes([s[0], s[1]])) / 32_768.0,
                (false, 24) => {
                    f64::from(i32::from_le_bytes([0, s[0], s[1], s[2]]) >> 8) / 8_388_608.0
                }
                (false, 32) => {
                    f64::from(i32::from_le_bytes([s[0], s[1], s[2], s[3]])) / 2_147_483_648.0
                }
                (true, 32) => f64::from(f32::from_le_bytes([s[0], s[1], s[2], s[3]])),
                (true, 64) => f64::from_le_bytes(s.try_into().expect("8 bytes")),
                _ => unreachable!("checked above"),
            };
            sum += v;
        }
        samples.push(sum / f64::from(channels));
    }
    if let Some(i) = samples.iter().position(|v| !v.is_finite()) {
        return Err(bad(format!("sample {i} is not finite")));
    }
    Ok(Wav {
        rate,
        channels,
        bits,
        float,
        samples,
        comment,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn float_round_trips_bit_for_bit_with_its_comment() {
        let x: Vec<f64> = (0..1001).map(|i| ((i as f64) * 0.01).sin() * 0.7).collect();
        let bytes = write(&x, 48_000, SampleFormat::F32, "seed 7").unwrap();
        assert_eq!(&bytes[..4], b"RIFF");
        assert_eq!(u32_at(&bytes, 4) as usize, bytes.len() - 8);
        let w = read(&bytes).unwrap();
        assert_eq!((w.rate, w.channels, w.bits, w.float), (48_000, 1, 32, true));
        assert_eq!(w.comment.as_deref(), Some("seed 7"));
        assert_eq!(w.samples.len(), x.len());
        for (a, b) in w.samples.iter().zip(&x) {
            assert_eq!(*a, f64::from(*b as f32));
        }
    }

    #[test]
    fn pcm24_round_trips_to_its_quantum_and_refuses_to_clip() {
        let x: Vec<f64> = (0..500).map(|i| ((i as f64) * 0.03).cos()).collect();
        let w = read(&write(&x, 48_000, SampleFormat::Pcm24, "").unwrap()).unwrap();
        assert_eq!((w.bits, w.float, w.comment.clone()), (24, false, None));
        let err = w
            .samples
            .iter()
            .zip(&x)
            .map(|(a, b)| (a - b).abs())
            .fold(0.0, f64::max);
        // Half a quantum, except +1.0 itself, which is held at the largest code.
        assert!(err <= 1.0 / 8_388_608.0, "{err}");
        let e = write(&[0.5, 1.0001], 48_000, SampleFormat::Pcm24, "").unwrap_err();
        assert_eq!(e.code(), "aural_would_clip");
        assert_eq!(
            write(&[f64::NAN], 48_000, SampleFormat::F32, "")
                .unwrap_err()
                .code(),
            "aural_bad_sample"
        );
    }

    #[test]
    fn a_stereo_16_bit_file_is_mixed_down() {
        // Built by hand: 2 channels, 16-bit, 44.1 kHz, frames (1000, -1000), (32767, 32767).
        let mut fmt = Vec::new();
        for v in [1u16, 2] {
            fmt.extend_from_slice(&v.to_le_bytes());
        }
        fmt.extend_from_slice(&44_100u32.to_le_bytes());
        fmt.extend_from_slice(&(44_100u32 * 4).to_le_bytes());
        fmt.extend_from_slice(&4u16.to_le_bytes());
        fmt.extend_from_slice(&16u16.to_le_bytes());
        let mut data = Vec::new();
        for v in [1000i16, -1000, 32767, 32767] {
            data.extend_from_slice(&v.to_le_bytes());
        }
        let mut body = b"WAVE".to_vec();
        chunk(&mut body, b"fmt ", &fmt);
        chunk(&mut body, b"data", &data);
        let mut file = Vec::new();
        chunk(&mut file, b"RIFF", &body);
        let w = read(&file).unwrap();
        assert_eq!((w.rate, w.channels, w.bits), (44_100, 2, 16));
        assert_eq!(w.samples, vec![0.0, 32767.0 / 32768.0]);
        assert_eq!(
            read(b"RIFF....WAVEjunk").unwrap_err().code(),
            "aural_wav_invalid"
        );
        assert_eq!(read(b"not a wav").unwrap_err().code(), "aural_wav_invalid");
    }
}
