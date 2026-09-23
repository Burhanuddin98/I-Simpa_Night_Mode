//! Fuzz for the `.cbin` reader: no input panics it, and no input makes it allocate more than
//! `budget(len)`. Run with `--test-threads=1`: the allocation counters are process-wide.
mod common;

#[global_allocator]
static A: common::Tracking = common::Tracking;

use proptest::prelude::*;
use proptest::test_runner::TestCaseError;
use simpa_core::formats::cbin;

const CASES: u32 = 10_000;

/// The real files the mutation cases start from.
const FILES: [&str; 3] = [
    "upstream/lib_interface/cube.cbin",
    "upstream/python_bindings/mesh.cbin",
    "upstream/tutorial1/spps/mesh.cbin",
];

fn config() -> ProptestConfig {
    // No regression files: a failure is reported with its minimal input instead.
    ProptestConfig {
        cases: CASES,
        failure_persistence: None,
        ..ProptestConfig::default()
    }
}

/// Reads `bytes` once, measuring the peak allocation the read makes.
fn check(bytes: &[u8]) -> Result<(), TestCaseError> {
    let base = common::reset_peak();
    let result = cbin::read(bytes);
    let peak = common::peak_since_reset(base);
    let ok = result.is_ok();
    drop(result);
    prop_assert!(
        peak <= common::budget(bytes.len()),
        "peak {} bytes > budget {} for {} input bytes (ok = {})",
        peak,
        common::budget(bytes.len()),
        bytes.len(),
        ok
    );
    Ok(())
}

fn fixture_bytes(rel: &str) -> Vec<u8> {
    std::fs::read(common::fixture(rel)).unwrap()
}

/// Edits applied to a real file: overwrite bytes, overwrite an aligned 32-bit field with an
/// extreme or random value (counts, links and indices are all 32-bit), cut, or splice.
#[derive(Debug, Clone)]
enum Edit {
    Byte(usize, u8),
    Word(usize, u32),
    Cut(usize),
    Insert(usize, Vec<u8>),
    Remove(usize, usize),
}

fn edit() -> impl Strategy<Value = Edit> {
    let word = prop_oneof![
        Just(0u32),
        Just(1),
        Just(2),
        Just(8),
        Just(0x7FFF_FFFF),
        Just(u32::MAX),
        Just(u32::MAX - 1),
        0u32..1024,
        any::<u32>(),
    ];
    prop_oneof![
        (any::<usize>(), any::<u8>()).prop_map(|(i, b)| Edit::Byte(i, b)),
        (any::<usize>(), word).prop_map(|(i, w)| Edit::Word(i, w)),
        any::<usize>().prop_map(Edit::Cut),
        (
            any::<usize>(),
            proptest::collection::vec(any::<u8>(), 1..64)
        )
            .prop_map(|(i, v)| Edit::Insert(i, v)),
        (any::<usize>(), 1usize..64).prop_map(|(i, n)| Edit::Remove(i, n)),
    ]
}

fn apply(mut b: Vec<u8>, edits: &[Edit]) -> Vec<u8> {
    for e in edits {
        let len = b.len();
        match e {
            Edit::Byte(i, v) if len > 0 => b[i % len] = *v,
            Edit::Word(i, w) if len >= 4 => {
                let at = (i % (len / 4)) * 4;
                b[at..at + 4].copy_from_slice(&w.to_le_bytes());
            }
            Edit::Cut(i) => b.truncate(i % (len + 1)),
            Edit::Insert(i, v) => {
                let at = i % (len + 1);
                b.splice(at..at, v.iter().copied());
            }
            Edit::Remove(i, n) if len > 0 => {
                let at = i % len;
                b.drain(at..(at + n).min(len));
            }
            _ => {}
        }
    }
    b
}

proptest! {
    #![proptest_config(config())]

    #[test]
    fn arbitrary_bytes(bytes in proptest::collection::vec(any::<u8>(), 0..4096)) {
        check(&bytes)?;
    }

    #[test]
    fn arbitrary_bytes_after_a_valid_header(tail in proptest::collection::vec(any::<u8>(), 0..4096)) {
        let mut bytes = Vec::with_capacity(8 + tail.len());
        bytes.extend_from_slice(&1u32.to_le_bytes());
        bytes.extend_from_slice(&0u32.to_le_bytes());
        bytes.extend_from_slice(&tail);
        check(&bytes)?;
    }

    #[test]
    fn mutations_of_real_files(which in 0..FILES.len(), edits in proptest::collection::vec(edit(), 1..6)) {
        let bytes = apply(fixture_bytes(FILES[which]), &edits);
        check(&bytes)?;
    }
}

#[test]
fn maximal_counts_stay_within_budget() {
    // Every 32-bit field of each file set to u32::MAX, one at a time.
    for rel in FILES {
        let raw = fixture_bytes(rel);
        for at in (0..raw.len() / 4).map(|i| i * 4) {
            let bytes = apply(raw.clone(), &[Edit::Word(at, u32::MAX)]);
            check(&bytes).unwrap_or_else(|e| panic!("{rel} field at {at}: {e}"));
        }
    }
}
