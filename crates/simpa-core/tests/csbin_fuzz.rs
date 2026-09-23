mod common;

#[global_allocator]
static A: common::Tracking = common::Tracking;

use std::sync::OnceLock;

use proptest::prelude::*;
use proptest::sample::Index;
use proptest::test_runner::TestCaseError;
use simpa_core::formats::csbin;

const CASES: u32 = 10_000;

fn config() -> ProptestConfig {
    // No regression files: a failure is reported with its minimal input instead.
    ProptestConfig {
        cases: CASES,
        failure_persistence: None,
        ..ProptestConfig::default()
    }
}

fn fixture_bytes() -> &'static [u8] {
    static B: OnceLock<Vec<u8>> = OnceLock::new();
    B.get_or_init(|| std::fs::read(common::fixture("upstream/lib_interface/rs_cut.csbin")).unwrap())
}

/// Reads `bytes`, requiring no panic (proptest fails the case on one) and a peak allocation
/// within budget. Successful reads are also dumped, outside the measurement.
fn check(bytes: &[u8]) -> Result<(), TestCaseError> {
    let base = common::reset_peak();
    let result = csbin::read(bytes);
    let peak = common::peak_since_reset(base);
    prop_assert!(
        peak <= common::budget(bytes.len()),
        "peak {peak} bytes over budget {} for {} input bytes",
        common::budget(bytes.len()),
        bytes.len()
    );
    if let Ok(m) = result {
        let d = csbin::dump(&m);
        prop_assert!(d.starts_with("csbin 3\n"));
    }
    Ok(())
}

/// A version-3 header with the native struct lengths and chosen counts.
fn header(nodes: i32, receivers: i32, steps: i32, record_type: i32) -> Vec<u8> {
    let mut h = Vec::with_capacity(44);
    for v in [3i32, 44, 12, 264, 16, 8, nodes, receivers, steps] {
        h.extend_from_slice(&v.to_le_bytes());
    }
    h.extend_from_slice(&0.01f32.to_le_bytes());
    h.extend_from_slice(&record_type.to_le_bytes());
    h
}

fn interesting_i32() -> impl Strategy<Value = i32> {
    prop_oneof![
        Just(0),
        Just(-1),
        Just(i32::MIN),
        Just(i32::MAX),
        Just(0x4000_0000),
        0..64i32,
        any::<i32>(),
    ]
}

proptest! {
    #![proptest_config(config())]

    /// Arbitrary bytes.
    #[test]
    fn arbitrary_bytes(bytes in prop::collection::vec(any::<u8>(), 0..4096)) {
        check(&bytes)?;
    }

    /// Arbitrary bytes behind a valid version field, so the header and body are reached.
    #[test]
    fn arbitrary_after_version(tail in prop::collection::vec(any::<u8>(), 0..4096)) {
        let mut bytes = 3i32.to_le_bytes().to_vec();
        bytes.extend_from_slice(&tail);
        check(&bytes)?;
    }

    /// A plausible header with counts from small to absurd, then arbitrary bytes.
    #[test]
    fn header_then_arbitrary(
        nodes in interesting_i32(),
        receivers in interesting_i32(),
        steps in interesting_i32(),
        record_type in -2..12i32,
        tail in prop::collection::vec(any::<u8>(), 0..8192),
    ) {
        let mut bytes = header(nodes, receivers, steps, record_type);
        bytes.extend_from_slice(&tail);
        check(&bytes)?;
    }

    /// Mutations of the upstream fixture: byte overwrites, i32 overwrites at aligned offsets
    /// (every count and index is an aligned i32), an optional truncation and an optional tail.
    #[test]
    fn fixture_mutations(
        bytes_set in prop::collection::vec((any::<Index>(), any::<u8>()), 0..8),
        words_set in prop::collection::vec((any::<Index>(), interesting_i32()), 0..4),
        cut in prop::option::of(any::<Index>()),
        tail in prop::collection::vec(any::<u8>(), 0..64),
    ) {
        let mut b = fixture_bytes().to_vec();
        for (i, v) in bytes_set {
            let at = i.index(b.len());
            b[at] = v;
        }
        for (i, v) in words_set {
            let at = i.index(b.len() / 4) * 4;
            b[at..at + 4].copy_from_slice(&v.to_le_bytes());
        }
        if let Some(i) = cut {
            b.truncate(i.index(b.len() + 1));
        }
        b.extend_from_slice(&tail);
        check(&b)?;
    }
}

/// Every byte of the fixture set to 0x00, 0x7F, 0x80 and 0xFF in turn.
#[test]
fn every_single_byte_mutation() {
    let base = fixture_bytes();
    let mut n = 0usize;
    for at in 0..base.len() {
        for v in [0x00u8, 0x7f, 0x80, 0xff] {
            let mut b = base.to_vec();
            b[at] = v;
            check(&b).unwrap();
            n += 1;
        }
    }
    assert_eq!(n, 4 * base.len());
}

/// A header whose counts are backed by just enough bytes to pass the count checks, which is
/// where an allocation-per-claimed-record reader would be at its worst.
#[test]
fn claimed_counts_at_the_limit() {
    // 1000 receivers claimed, 1000 * 264 bytes present, but the first receiver claims every face.
    let mut b = header(0, 1000, 5, 0);
    let mut rec = vec![0u8; 264];
    let faces: i32 = (1000 * 264 - 264) / 16;
    rec[4..8].copy_from_slice(&faces.to_le_bytes());
    b.extend_from_slice(&rec);
    b.resize(44 + 1000 * 264, 0);
    check(&b).unwrap();
    // As many zero-record faces as fit: the densest allocation per input byte the model has.
    let mut b = header(3, 1, 1, 0);
    b.extend_from_slice(&[0u8; 36]);
    let faces = 20_000usize;
    let mut rec = vec![0u8; 264];
    rec[4..8].copy_from_slice(&(faces as i32).to_le_bytes());
    b.extend_from_slice(&rec);
    for _ in 0..faces {
        b.extend_from_slice(&[0, 0, 0, 0, 1, 0, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0]);
    }
    let base = common::reset_peak();
    let m = csbin::read(&b).unwrap();
    let peak = common::peak_since_reset(base);
    assert_eq!(m.receivers[0].faces.len(), faces);
    assert!(
        peak <= common::budget(b.len()),
        "peak {peak} over budget {}",
        common::budget(b.len())
    );
}

/// Real files stay within budget too.
#[test]
fn fixtures_within_budget() {
    for rel in [
        "upstream/lib_interface/rs_cut.csbin",
        "upstream/python_bindings/rs_cut.csbin",
        "solver-outputs/tutorial1/spps_1000hz.csbin",
    ] {
        let b = std::fs::read(common::fixture(rel)).unwrap();
        let base = common::reset_peak();
        let m = csbin::read(&b).unwrap();
        let peak = common::peak_since_reset(base);
        assert!(peak <= common::budget(b.len()), "{rel}: peak {peak}");
        drop(m);
    }
}
