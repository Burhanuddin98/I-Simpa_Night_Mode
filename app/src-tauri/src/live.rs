//! B3 (decision 71): the running GPU solve on screen. `spps-gpu` writes each saved particle's
//! trajectory to `solve/spps-gpu.pstream` as it re-traces it (its layout is in
//! `solvers/spps-gpu/src/output.h`, `LiveStream`); while a run of SPPS on the GPU is live this
//! module tails that file, decodes whole frames and sends them to the UI on the run's live channel,
//! in the PART v1 layout the viewport already decodes (`results_data.rs`), behind a 32-byte
//! envelope that says how far the run has got:
//!
//! ```text
//! LIVE v1 (one batch of one band's arriving particles)
//!   0   u32 magic 0x4556494C ("LIVE")      4   u32 version 1
//!   8   u32 particles arrived so far in the run, this batch included
//!   12  u32 particles the run saves in all (per band x bands)
//!   16  u32 the batch's band, its position among the computed bands
//!   20  u32 bands computed                24  u32 0      28  u32 0
//!   32  PART v1 of the batch's particles (`encode_particles`), the band in its header
//! ```
//!
//! The file is polled every [`POLL`] (no file watcher in the app), so at most about ten batches a
//! second reach the UI, one per band present in a poll. A torn tail (a frame the solver is writing)
//! stays in the buffer until it is whole; it is never an error. Nothing here is an acoustic value:
//! these are the saved sample's positions and energies, the same records the Results step replays.

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;
use std::time::Duration;

use simpa_core::formats::pbin;

use crate::results_data::encode_particles;

/// The stream's file, in the run's `solve/` folder.
pub const STREAM_FILE: &str = "spps-gpu.pstream";
pub const STREAM_MAGIC: u32 = 0x4D54_5350;
pub const STREAM_VERSION: u32 = 1;
pub const LIVE_MAGIC: u32 = 0x4556_494C;
pub const LIVE_VERSION: u32 = 1;
pub const LIVE_HEADER: usize = 32;
/// How often the file is read.
pub const POLL: Duration = Duration::from_millis(100);
/// A frame longer than this is a corrupt length, not a particle (10 s at 1 ms is 160 kB).
const MAX_FRAME: u32 = 256 << 20;
/// More computed bands than this is a corrupt header.
const MAX_BANDS: u32 = 4096;

#[derive(Clone, Debug, PartialEq)]
pub struct StreamHeader {
    pub time_step: f32,
    pub steps: u32,
    /// Particles saved per band (the `.pbin` header's count).
    pub per_band: u32,
    /// The computed bands, Hz, in computation order.
    pub bands: Vec<i32>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct LiveFrame {
    pub band_hz: i32,
    /// The particle's index in its band's `.pbin`.
    pub index: u32,
    /// Its first time step, the `.pbin`'s u16.
    pub first_step: u32,
    pub positions: Vec<[f32; 3]>,
    pub energies: Vec<f32>,
}

fn u32_at(b: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([b[at], b[at + 1], b[at + 2], b[at + 3]])
}
fn f32_at(b: &[u8], at: usize) -> f32 {
    f32::from_bits(u32_at(b, at))
}

/// Decodes the stream incrementally: feed it what the file grew by, take the whole frames.
#[derive(Default)]
pub struct Decoder {
    buf: Vec<u8>,
    header: Option<StreamHeader>,
}

impl Decoder {
    pub fn header(&self) -> Option<&StreamHeader> {
        self.header.as_ref()
    }

    /// Bytes held back: a torn tail waiting for the rest of its frame.
    #[cfg(test)]
    pub fn pending(&self) -> usize {
        self.buf.len()
    }

    /// Appends `bytes` and returns every frame now whole. An error is a stream that is not this
    /// layout (a wrong magic or version, an impossible length): the caller stops reading it.
    pub fn feed(&mut self, bytes: &[u8]) -> Result<Vec<LiveFrame>, String> {
        self.buf.extend_from_slice(bytes);
        let mut at = 0usize;
        if self.header.is_none() {
            if self.buf.len() < 24 {
                return Ok(Vec::new());
            }
            let b = &self.buf;
            let magic = u32_at(b, 0);
            if magic != STREAM_MAGIC {
                return Err(format!("not a live stream: magic {magic:#010x}"));
            }
            let version = u32_at(b, 4);
            if version != STREAM_VERSION {
                return Err(format!("live stream version {version}, this app reads {STREAM_VERSION}"));
            }
            let nb = u32_at(b, 20);
            if nb > MAX_BANDS {
                return Err(format!("live stream header names {nb} bands"));
            }
            let len = 24 + 4 * nb as usize;
            if b.len() < len {
                return Ok(Vec::new());
            }
            self.header = Some(StreamHeader {
                time_step: f32_at(b, 8),
                steps: u32_at(b, 12),
                per_band: u32_at(b, 16),
                bands: (0..nb as usize).map(|i| u32_at(b, 24 + 4 * i) as i32).collect(),
            });
            at = len;
        }
        let mut out = Vec::new();
        loop {
            let b = &self.buf;
            if b.len() < at + 4 {
                break;
            }
            let len = u32_at(b, at);
            if !(16..=MAX_FRAME).contains(&len) || (len - 16) % 16 != 0 {
                return Err(format!("live stream frame of {len} bytes"));
            }
            if b.len() < at + 4 + len as usize {
                break; // a torn tail: wait for the rest
            }
            let f = at + 4;
            let n = u32_at(b, f + 12);
            if 16 + 16 * n as u64 != u64::from(len) {
                return Err(format!("live stream frame of {len} bytes holds {n} records"));
            }
            let n = n as usize;
            let pos = f + 16;
            let en = pos + 12 * n;
            out.push(LiveFrame {
                band_hz: u32_at(b, f) as i32,
                index: u32_at(b, f + 4),
                first_step: u32_at(b, f + 8),
                positions: (0..n)
                    .map(|k| [f32_at(b, pos + 12 * k), f32_at(b, pos + 12 * k + 4), f32_at(b, pos + 12 * k + 8)])
                    .collect(),
                energies: (0..n).map(|k| f32_at(b, en + 4 * k)).collect(),
            });
            at += 4 + len as usize;
        }
        self.buf.drain(..at);
        Ok(out)
    }
}

/// One band's frames as a LIVE v1 batch: the envelope, then PART v1.
pub fn encode_batch(h: &StreamHeader, frames: &[LiveFrame], so_far: u32) -> Result<Vec<u8>, String> {
    let band_hz = frames.first().map_or(0, |f| f.band_hz);
    let file = pbin::ParticleFile {
        header: pbin::FileHeader {
            nb_particles: frames.len() as u32,
            format_version: pbin::FORMAT_VERSION,
            file_info_length: pbin::FILE_HEADER_SIZE as u32,
            particle_info_length: pbin::TIME_STEP_SIZE as u32,
            particle_header_info_length: pbin::PARTICLE_HEADER_SIZE as u32,
            nb_time_step_max: h.steps,
            time_step: h.time_step,
        },
        particles: frames
            .iter()
            .map(|f| pbin::ParticleHeader {
                nb_time_step: f.energies.len() as u32,
                first_time_step: f.first_step as u16,
            })
            .collect(),
        steps: frames
            .iter()
            .flat_map(|f| f.positions.iter().zip(&f.energies).map(|(p, e)| pbin::TimeStep { position: *p, energy: *e }))
            .collect(),
    };
    let part = encode_particles(&file, band_hz).map_err(|e| e.message.clone())?;
    let band_pos = h.bands.iter().position(|&b| b == band_hz).map_or(u32::MAX, |p| p as u32);
    let mut b = Vec::with_capacity(LIVE_HEADER + part.len());
    for v in [
        LIVE_MAGIC,
        LIVE_VERSION,
        so_far,
        h.per_band.saturating_mul(h.bands.len() as u32),
        band_pos,
        h.bands.len() as u32,
        0,
        0,
    ] {
        b.extend_from_slice(&v.to_le_bytes());
    }
    b.extend_from_slice(&part);
    Ok(b)
}

/// Splits frames into runs of one band, in order (a poll can straddle a band's end).
pub fn by_band(frames: Vec<LiveFrame>) -> Vec<Vec<LiveFrame>> {
    let mut out: Vec<Vec<LiveFrame>> = Vec::new();
    for f in frames {
        match out.last_mut() {
            Some(g) if g[0].band_hz == f.band_hz => g.push(f),
            _ => out.push(vec![f]),
        }
    }
    out
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct TailStats {
    pub frames: u64,
    pub batches: u64,
    pub bytes: u64,
    /// Times the file at the path turned out to be another stream and reading started again.
    pub resets: u64,
    /// Why reading stopped early, if it did.
    pub error: Option<String>,
}

/// The stream file of a run folder.
pub fn stream_path(run_dir: &Path) -> PathBuf {
    run_dir.join(simpa_core::run::manager::SOLVE_DIR).join(STREAM_FILE)
}

/// Reads what `file` grew by since `at`.
fn read_more(file: &mut File, at: &mut u64) -> std::io::Result<Vec<u8>> {
    file.seek(SeekFrom::Start(*at))?;
    let mut more = Vec::new();
    file.read_to_end(&mut more)?;
    *at += more.len() as u64;
    Ok(more)
}

/// How many bytes of the stream's start identify it (the header and the start of its first frame).
const PREFIX: usize = 64;

/// Whether the file now at the path is still the stream read so far: not shorter than what was read,
/// and starting with the same bytes. A stream truncated and rewritten, or replaced by another file
/// (a reused folder, an old stream the solver then rewrites), is not.
fn same_stream(file: &mut File, len: u64, at: u64, prefix: &[u8]) -> bool {
    if len < at {
        return false;
    }
    let mut head = vec![0u8; prefix.len()];
    file.seek(SeekFrom::Start(0)).is_ok() && file.read_exact(&mut head).is_ok() && head == prefix
}

/// Tails `path` every `poll` until `stop` is set, sending each band's new particles to `sink` as a
/// LIVE v1 batch. The file may not exist yet (the solver has not started): it is waited for. The path
/// is opened afresh at every poll and checked against what was read (`same_stream`); when it is
/// another stream the reading starts again from its first byte, the count with it. When the path is
/// gone (the solver removes it at its end) the last handle keeps reading what was written.
pub fn spawn_tail<S>(path: PathBuf, poll: Duration, stop: Arc<AtomicBool>, mut sink: S) -> std::io::Result<JoinHandle<TailStats>>
where
    S: FnMut(Vec<u8>) -> bool + Send + 'static,
{
    std::thread::Builder::new().name("live-tail".to_string()).spawn(move || {
        let mut stats = TailStats::default();
        let mut dec = Decoder::default();
        let mut file: Option<File> = None;
        let mut at = 0u64;
        let mut so_far = 0u32;
        let mut prefix: Vec<u8> = Vec::new();
        loop {
            let stopping = stop.load(Ordering::SeqCst);
            if let Ok(mut now) = File::open(&path) {
                let len = now.metadata().map(|m| m.len()).unwrap_or(0);
                if file.is_some() && !same_stream(&mut now, len, at, &prefix) {
                    dec = Decoder::default();
                    at = 0;
                    so_far = 0;
                    prefix.clear();
                    stats.resets += 1;
                }
                file = Some(now);
            }
            if let Some(f) = file.as_mut() {
                let more = match read_more(f, &mut at) {
                    Ok(m) => m,
                    Err(e) => {
                        stats.error = Some(format!("reading {}: {e}", path.display()));
                        break;
                    }
                };
                if prefix.len() < PREFIX {
                    let take = (PREFIX - prefix.len()).min(more.len());
                    prefix.extend_from_slice(&more[..take]);
                }
                stats.bytes += more.len() as u64;
                match dec.feed(&more) {
                    Ok(frames) if !frames.is_empty() => {
                        let h = dec.header().cloned().expect("frames follow the header");
                        for group in by_band(frames) {
                            so_far = so_far.saturating_add(group.len() as u32);
                            stats.frames += group.len() as u64;
                            match encode_batch(&h, &group, so_far) {
                                Ok(b) => {
                                    stats.batches += 1;
                                    sink(b);
                                }
                                Err(e) => stats.error = Some(e),
                            }
                        }
                    }
                    Ok(_) => {}
                    Err(e) => {
                        stats.error = Some(e);
                        break;
                    }
                }
            }
            if stopping {
                break;
            }
            // Sleep in short slices so a stop is seen within a few milliseconds.
            let mut slept = Duration::ZERO;
            while slept < poll && !stop.load(Ordering::SeqCst) {
                let d = Duration::from_millis(10).min(poll - slept);
                std::thread::sleep(d);
                slept += d;
            }
        }
        stats
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::sync::Mutex;

    fn header(bands: &[i32]) -> Vec<u8> {
        let mut b = Vec::new();
        for v in [STREAM_MAGIC, STREAM_VERSION, 0.001f32.to_bits(), 10_000, 400, bands.len() as u32] {
            b.extend_from_slice(&v.to_le_bytes());
        }
        for &f in bands {
            b.extend_from_slice(&f.to_le_bytes());
        }
        b
    }

    fn frame(band: i32, index: u32, first: u32, n: u32) -> Vec<u8> {
        let mut b = Vec::new();
        for v in [16 + 16 * n, band as u32, index, first, n] {
            b.extend_from_slice(&v.to_le_bytes());
        }
        for k in 0..n {
            for c in 0..3 {
                b.extend_from_slice(&(index as f32 + k as f32 * 0.5 + c as f32 * 0.25).to_le_bytes());
            }
        }
        for k in 0..n {
            b.extend_from_slice(&(1.0f32 / (k + 1) as f32).to_le_bytes());
        }
        b
    }

    fn stream() -> Vec<u8> {
        let mut s = header(&[500, 1000]);
        s.extend(frame(500, 0, 3, 4));
        s.extend(frame(500, 1, 0, 1));
        s.extend(frame(1000, 0, 7, 3));
        s
    }

    #[test]
    fn whole_frames_decode_bit_for_bit() {
        let mut d = Decoder::default();
        let f = d.feed(&stream()).unwrap();
        assert_eq!(d.header().unwrap().bands, vec![500, 1000]);
        assert_eq!(d.header().unwrap().per_band, 400);
        assert_eq!(f.len(), 3);
        assert_eq!((f[0].band_hz, f[0].index, f[0].first_step, f[0].energies.len()), (500, 0, 3, 4));
        assert_eq!(f[0].positions[1], [0.5, 0.75, 1.0]);
        assert_eq!(f[0].energies[3].to_bits(), 0.25f32.to_bits());
        assert_eq!((f[2].band_hz, f[2].first_step), (1000, 7));
        assert_eq!(d.pending(), 0);
    }

    #[test]
    fn a_torn_tail_is_held_not_an_error_at_every_cut() {
        let s = stream();
        let whole = Decoder::default().feed(&s).unwrap();
        for cut in 0..=s.len() {
            let mut d = Decoder::default();
            let mut got = d.feed(&s[..cut]).unwrap();
            assert!(got.len() <= whole.len());
            got.extend(d.feed(&s[cut..]).unwrap());
            assert_eq!(got, whole, "cut at {cut}");
            assert_eq!(d.pending(), 0);
        }
        // fed byte by byte
        let mut d = Decoder::default();
        let mut got = Vec::new();
        for b in &s {
            got.extend(d.feed(std::slice::from_ref(b)).unwrap());
        }
        assert_eq!(got, whole);
    }

    #[test]
    fn a_stream_that_is_not_this_layout_is_refused() {
        let mut s = stream();
        s[0] ^= 1;
        assert!(Decoder::default().feed(&s).unwrap_err().contains("magic"));
        let mut s = stream();
        s[4] = 2;
        assert!(Decoder::default().feed(&s).unwrap_err().contains("version"));
        // a frame whose length disagrees with its record count
        let mut s = header(&[500]);
        let mut f = frame(500, 0, 0, 2);
        f[16] = 3;
        s.extend(f);
        assert!(Decoder::default().feed(&s).unwrap_err().contains("records"));
        // an impossible length
        let mut s = header(&[500]);
        s.extend(17u32.to_le_bytes());
        assert!(Decoder::default().feed(&s).is_err());
    }

    #[test]
    fn a_batch_is_the_envelope_then_part_v1() {
        let mut d = Decoder::default();
        let f = d.feed(&stream()).unwrap();
        let h = d.header().unwrap().clone();
        let groups = by_band(f);
        assert_eq!(groups.iter().map(Vec::len).collect::<Vec<_>>(), vec![2, 1]);
        let b = encode_batch(&h, &groups[0], 2).unwrap();
        assert_eq!(u32_at(&b, 0), LIVE_MAGIC);
        assert_eq!(u32_at(&b, 8), 2);
        assert_eq!(u32_at(&b, 12), 800);
        assert_eq!(u32_at(&b, 16), 0);
        assert_eq!(u32_at(&b, 20), 2);
        let p = &b[LIVE_HEADER..];
        assert_eq!(u32_at(p, 0), crate::results_data::PART_MAGIC);
        assert_eq!(u32_at(p, 8), 2); // particles
        assert_eq!(u32_at(p, 12), 5); // records
        assert_eq!(u32_at(p, 16), 10_000);
        assert_eq!(f32_at(p, 20), 0.001);
        assert_eq!(u32_at(p, 24) as i32, 500);
        assert_eq!((u32_at(p, 32), u32_at(p, 36)), (3, 0)); // first steps
        assert_eq!((u32_at(p, 40), u32_at(p, 44), u32_at(p, 48)), (0, 4, 5)); // offsets
        let e0 = 52 + 12 * 5;
        assert_eq!(f32_at(p, e0 + 12), 0.25);
        assert_eq!(p.len(), 52 + 16 * 5);
        let b2 = encode_batch(&h, &groups[1], 3).unwrap();
        assert_eq!(u32_at(&b2, 16), 1);
    }

    #[test]
    fn the_tail_follows_a_growing_file_and_stops() {
        let dir = std::env::temp_dir().join(format!("nm-live-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(STREAM_FILE);
        let _ = std::fs::remove_file(&path);
        let got: Arc<Mutex<Vec<Vec<u8>>>> = Arc::default();
        let sink_got = got.clone();
        let stop = Arc::new(AtomicBool::new(false));
        let h = spawn_tail(path.clone(), Duration::from_millis(5), stop.clone(), move |b| {
            sink_got.lock().unwrap().push(b);
            true
        })
        .unwrap();
        std::thread::sleep(Duration::from_millis(20)); // no file yet: waited for
        let s = stream();
        let mut f = File::create(&path).unwrap();
        let cut = header(&[500, 1000]).len() + 30; // inside the first frame
        f.write_all(&s[..cut]).unwrap();
        f.flush().unwrap();
        std::thread::sleep(Duration::from_millis(40));
        assert!(got.lock().unwrap().is_empty(), "a torn frame is not sent");
        f.write_all(&s[cut..]).unwrap();
        f.flush().unwrap();
        drop(f);
        let t0 = std::time::Instant::now();
        while got.lock().unwrap().len() < 2 && t0.elapsed() < Duration::from_secs(5) {
            std::thread::sleep(Duration::from_millis(5));
        }
        stop.store(true, Ordering::SeqCst);
        let stats = h.join().unwrap();
        assert_eq!(stats.frames, 3);
        assert_eq!(stats.error, None);
        let got = got.lock().unwrap();
        assert_eq!(got.len(), 2);
        assert_eq!(u32_at(&got[1], 8), 3, "so far counts the run");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A stream replaced while it is tailed (deleted and written again with another header), and one
    /// truncated and rewritten shorter: each time the tail starts again from the new file's first
    /// byte and counts afresh; nothing of the old stream is decoded as the new one.
    #[test]
    fn a_replaced_or_truncated_stream_is_read_again_from_its_start() {
        let dir = std::env::temp_dir().join(format!("nm-live-replace-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(STREAM_FILE);
        // an old stream already there when the tail starts: three frames of band 500
        let mut old = header(&[500]);
        for i in 0..3 {
            old.extend(frame(500, i, 0, 6));
        }
        std::fs::write(&path, &old).unwrap();
        let got: Arc<Mutex<Vec<Vec<u8>>>> = Arc::default();
        let sink_got = got.clone();
        let stop = Arc::new(AtomicBool::new(false));
        let h = spawn_tail(path.clone(), Duration::from_millis(5), stop.clone(), move |b| {
            sink_got.lock().unwrap().push(b);
            true
        })
        .unwrap();
        let wait = |n: usize| {
            let t0 = std::time::Instant::now();
            while got.lock().unwrap().len() < n && t0.elapsed() < Duration::from_secs(5) {
                std::thread::sleep(Duration::from_millis(5));
            }
        };
        wait(1);
        // replaced: removed, then a new stream of band 1000 with one short frame (shorter than the old)
        std::fs::remove_file(&path).unwrap();
        let mut new = header(&[1000]);
        new.extend(frame(1000, 0, 2, 1));
        std::fs::write(&path, &new).unwrap();
        wait(2);
        // truncated in place and rewritten: band 2000, one frame
        let mut third = header(&[2000]);
        third.extend(frame(2000, 0, 0, 1));
        {
            let mut f = std::fs::OpenOptions::new().write(true).truncate(true).open(&path).unwrap();
            f.write_all(&third).unwrap();
        }
        wait(3);
        stop.store(true, Ordering::SeqCst);
        let stats = h.join().unwrap();
        assert_eq!(stats.error, None);
        assert_eq!(stats.resets, 2, "{stats:?}");
        let got = got.lock().unwrap();
        let bands: Vec<i32> = got.iter().map(|b| u32_at(&b[LIVE_HEADER..], 24) as i32).collect();
        assert_eq!(bands, vec![500, 1000, 2000]);
        let so_far: Vec<u32> = got.iter().map(|b| u32_at(b, 8)).collect();
        assert_eq!(so_far, vec![3, 1, 1], "the count starts again with each stream");
        assert_eq!(u32_at(&got[1][LIVE_HEADER..], 32), 2, "the new stream's own first step");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
