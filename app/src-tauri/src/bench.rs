//! Synthetic datasets for the IPC benchmark (M9 gate c).
//!
//! The UI asks for a buffer of a given size ([`BenchStore::prepare`]), then times only the
//! transfer ([`BenchStore::take`] handed to `ipc::Response`), so generating the bytes is not
//! counted. The sizes the UI asks for are the corrected Elmia solve's real `rs_cut.csbin` files;
//! the content is a seeded pseudo-random stream with a checksum the UI recomputes, so a truncated
//! or reordered transfer cannot pass.

use std::collections::HashMap;
use std::time::Instant;

use schemars::JsonSchema;
use serde::Serialize;

use crate::guard::{CmdError, CmdResult};

/// The largest single buffer: above the 16.5 MB Global surface map with room to spare.
pub const MAX_BUFFER: u64 = 64 * 1024 * 1024;
/// The most held at once: the 98.4 MB all-band set plus headroom.
pub const MAX_HELD: u64 = 192 * 1024 * 1024;

/// A prepared buffer: take it with `token`; `checksum` is [`checksum`] of its bytes.
#[derive(Clone, Debug, PartialEq, Serialize, JsonSchema)]
pub struct Prepared {
    pub token: u64,
    pub bytes: u64,
    pub checksum: u32,
    pub prepare_ms: f64,
}

#[derive(Debug, Default)]
pub struct BenchStore {
    next: u64,
    held: HashMap<u64, Vec<u8>>,
    held_bytes: u64,
}

impl BenchStore {
    pub fn prepare(&mut self, bytes: u64, seed: u64) -> CmdResult<Prepared> {
        if bytes > MAX_BUFFER {
            return Err(CmdError::new(
                "BENCH_TOO_LARGE",
                format!("{bytes} bytes asked, at most {MAX_BUFFER} per buffer"),
            ));
        }
        if self.held_bytes + bytes > MAX_HELD {
            return Err(CmdError::new(
                "BENCH_TOO_MUCH_HELD",
                format!(
                    "{} bytes already held; {bytes} more would pass {MAX_HELD}",
                    self.held_bytes
                ),
            ));
        }
        let t0 = Instant::now();
        let data = fill(bytes as usize, seed);
        let sum = checksum(&data);
        let prepare_ms = t0.elapsed().as_secs_f64() * 1e3;
        self.next += 1;
        self.held_bytes += bytes;
        self.held.insert(self.next, data);
        Ok(Prepared {
            token: self.next,
            bytes,
            checksum: sum,
            prepare_ms,
        })
    }

    /// Hands the buffer over; the store forgets it, so the transfer moves it without a copy here.
    pub fn take(&mut self, token: u64) -> CmdResult<Vec<u8>> {
        let data = self.held.remove(&token).ok_or_else(|| {
            CmdError::new("BENCH_UNKNOWN_TOKEN", format!("no prepared buffer {token}"))
        })?;
        self.held_bytes -= data.len() as u64;
        Ok(data)
    }

    /// Drops every held buffer; returns how many bytes were released.
    pub fn clear(&mut self) -> u64 {
        let released = self.held_bytes;
        self.held.clear();
        self.held_bytes = 0;
        released
    }
}

/// `bytes` bytes of splitmix64 output for `seed`, little-endian.
pub fn fill(bytes: usize, seed: u64) -> Vec<u8> {
    let mut out = Vec::with_capacity(bytes);
    let mut state = seed;
    while out.len() < bytes {
        state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^= z >> 31;
        let take = (bytes - out.len()).min(8);
        out.extend_from_slice(&z.to_le_bytes()[..take]);
    }
    out
}

/// The checksum the UI recomputes: the wrapping sum of `w * (2k + 1)` over each little-endian
/// u32 word `w` at word index `k`, then of `b * (2k + 1)` over each trailing byte `b`, `k`
/// continuing the count. The odd, position-dependent weights make a reordered or shifted
/// transfer fail it.
pub fn checksum(data: &[u8]) -> u32 {
    let (words, tail) = data.as_chunks::<4>();
    let mut sum = 0u32;
    for (k, w) in words.iter().enumerate() {
        sum = sum.wrapping_add(u32::from_le_bytes(*w).wrapping_mul(weight(k)));
    }
    for (j, &b) in tail.iter().enumerate() {
        sum = sum.wrapping_add(u32::from(b).wrapping_mul(weight(words.len() + j)));
    }
    sum
}

fn weight(k: usize) -> u32 {
    (k as u32).wrapping_mul(2).wrapping_add(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fill_is_deterministic_and_sized() {
        for n in [0, 1, 7, 8, 9, 4099] {
            let a = fill(n, 42);
            assert_eq!(a.len(), n);
            assert_eq!(a, fill(n, 42));
        }
        assert_ne!(fill(64, 1), fill(64, 2));
    }

    #[test]
    fn checksum_sees_order_and_tail() {
        let a = fill(1027, 5);
        let mut b = a.clone();
        b.swap(0, 4);
        assert_ne!(checksum(&a), checksum(&b));
        let mut c = a.clone();
        c[1026] ^= 1;
        assert_ne!(checksum(&a), checksum(&c));
        let mut d = a.clone();
        d.swap(0, 1);
        assert_ne!(checksum(&a), checksum(&d));
        assert_eq!(checksum(&[1, 0, 0, 0, 2, 0, 0, 0, 9]), 1 + 2 * 3 + 9 * 5);
    }

    /// The known answers the UI's implementation (ui/src/checksum.ts) is held to as well, by
    /// `npm test` and by the app's self-test. The file's values come from a third implementation.
    #[test]
    fn checksum_known_answers_shared_with_the_ui() {
        let kat: serde_json::Value =
            serde_json::from_str(include_str!("../../ui/src/checksum-kat.json")).unwrap();
        let cases = kat["cases"].as_array().unwrap();
        assert!(cases.len() >= 6, "{} cases", cases.len());
        for case in cases {
            let hex = case["hex"].as_str().unwrap();
            let bytes: Vec<u8> = (0..hex.len())
                .step_by(2)
                .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
                .collect();
            let want = case["checksum"].as_u64().unwrap();
            assert_eq!(u64::from(checksum(&bytes)), want, "{}", case["name"]);
        }
    }

    #[test]
    fn prepare_take_clear_account_for_bytes() {
        let mut s = BenchStore::default();
        let p = s.prepare(1000, 3).unwrap();
        assert_eq!(p.checksum, checksum(&fill(1000, 3)));
        let q = s.prepare(10, 4).unwrap();
        assert_eq!(s.take(p.token).unwrap().len(), 1000);
        assert_eq!(s.take(p.token).unwrap_err().code, "BENCH_UNKNOWN_TOKEN");
        assert_eq!(s.clear(), 10);
        assert_eq!(s.take(q.token).unwrap_err().code, "BENCH_UNKNOWN_TOKEN");
        assert_eq!(
            s.prepare(MAX_BUFFER + 1, 0).unwrap_err().code,
            "BENCH_TOO_LARGE"
        );
    }

    #[test]
    fn the_held_total_is_bounded() {
        let mut s = BenchStore {
            held_bytes: MAX_HELD - 5,
            ..BenchStore::default()
        };
        assert_eq!(s.prepare(6, 0).unwrap_err().code, "BENCH_TOO_MUCH_HELD");
        assert!(s.prepare(5, 0).is_ok());
    }
}
