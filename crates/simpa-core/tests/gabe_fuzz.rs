//! Robustness gate for the GABE reader: arbitrary bytes and mutations of real tables never
//! panic, and never make the reader allocate more than `budget(len)`.
//! Run with `--test-threads=1`: the allocation counters are process-wide.
mod common;

#[global_allocator]
static A: common::Tracking = common::Tracking;

use std::sync::OnceLock;

use common::{budget, fixture, peak_since_reset, reset_peak};
use proptest::prelude::*;
use simpa_core::formats::gabe;

const CASES: u32 = 10_000;

fn config() -> ProptestConfig {
    ProptestConfig {
        cases: CASES,
        failure_persistence: None,
        ..ProptestConfig::default()
    }
}

/// Reads `bytes` and checks the allocation peak against the budget. Also checks that a
/// successful read's dump can be produced (outside the measured window).
fn check(bytes: &[u8]) -> Result<(), TestCaseError> {
    let base = reset_peak();
    let res = gabe::read_prefix(bytes);
    let peak = peak_since_reset(base);
    prop_assert!(
        peak <= budget(bytes.len()),
        "peak {} bytes > budget {} for {} input bytes",
        peak,
        budget(bytes.len()),
        bytes.len()
    );
    if let Ok((g, used)) = res {
        prop_assert!(used <= bytes.len());
        prop_assert_eq!(g.version, gabe::VERSION);
        let _ = gabe::dump(&g);
    }
    Ok(())
}

fn seeds() -> &'static [Vec<u8>] {
    static SEEDS: OnceLock<Vec<Vec<u8>>> = OnceLock::new();
    SEEDS.get_or_init(|| {
        [
            "solver-outputs/nightmode-2026-09-08/spps_stats.gabe",
            "solver-outputs/nightmode-2026-09-08/tcr_main_results.gabe",
            "upstream/python_bindings/retrocompat_test.gabe",
        ]
        .iter()
        .map(|rel| std::fs::read(fixture(rel)).unwrap())
        .collect()
    })
}

#[derive(Debug, Clone)]
enum Mutation {
    /// XOR one byte.
    Flip(usize, u8),
    /// Overwrite 4 bytes with a little-endian integer (hits counts, sizes and versions).
    Word(usize, i32),
    /// Overwrite 8 bytes with a little-endian integer (hits `sizeofCol`).
    Long(usize, i64),
    /// Cut the file.
    Truncate(usize),
    /// Insert bytes.
    Insert(usize, Vec<u8>),
}

fn interesting_i32() -> impl Strategy<Value = i32> {
    prop_oneof![
        any::<i32>(),
        Just(-1),
        Just(0),
        Just(1),
        Just(2),
        Just(i32::MAX),
        Just(i32::MIN),
        50..53i32,
        0..100_000i32,
    ]
}

fn mutation() -> impl Strategy<Value = Mutation> {
    prop_oneof![
        (any::<usize>(), 1..=255u8).prop_map(|(i, x)| Mutation::Flip(i, x)),
        (any::<usize>(), interesting_i32()).prop_map(|(i, v)| Mutation::Word(i, v)),
        (
            any::<usize>(),
            prop_oneof![any::<i64>(), -1..1000i64, Just(i64::MAX)]
        )
            .prop_map(|(i, v)| Mutation::Long(i, v)),
        any::<usize>().prop_map(Mutation::Truncate),
        (any::<usize>(), prop::collection::vec(any::<u8>(), 1..64))
            .prop_map(|(i, v)| Mutation::Insert(i, v)),
    ]
}

fn apply(b: &mut Vec<u8>, m: &Mutation) {
    let n = b.len();
    match m {
        Mutation::Flip(i, x) => {
            if n > 0 {
                b[i % n] ^= x;
            }
        }
        Mutation::Word(i, v) => {
            if n >= 4 {
                let at = i % (n - 3);
                b[at..at + 4].copy_from_slice(&v.to_le_bytes());
            }
        }
        Mutation::Long(i, v) => {
            if n >= 8 {
                let at = i % (n - 7);
                b[at..at + 8].copy_from_slice(&v.to_le_bytes());
            }
        }
        Mutation::Truncate(i) => b.truncate(i % (n + 1)),
        Mutation::Insert(i, v) => {
            let at = i % (n + 1);
            b.splice(at..at, v.iter().copied());
        }
    }
}

/// A random table header followed by random column headers and bytes, so that the structural
/// checks (counts, types, sizes) are reached far more often than by fully random bytes.
fn structured() -> impl Strategy<Value = Vec<u8>> {
    let col = (
        prop_oneof![50..53u16, any::<u16>()],
        interesting_i32(),
        prop_oneof![any::<i64>(), 0..400i64],
        prop::collection::vec(any::<u8>(), 0..300),
    );
    (interesting_i32(), 0..=1u8, prop::collection::vec(col, 0..6)).prop_map(|(cols, ro, v)| {
        let mut b = Vec::new();
        for x in [2i32, 0, 0, cols] {
            b.extend_from_slice(&x.to_le_bytes());
        }
        b.extend_from_slice(&[ro, 0, 0, 0]);
        for (ty, rows, size, tail) in v {
            b.extend_from_slice(&ty.to_le_bytes());
            b.extend_from_slice(&[0, 0]);
            b.extend_from_slice(&rows.to_le_bytes());
            b.extend_from_slice(&size.to_le_bytes());
            b.extend_from_slice(&1i64.to_le_bytes());
            b.extend_from_slice(&[b'c'; 255]);
            b.push(0);
            b.extend_from_slice(&tail);
        }
        b
    })
}

proptest! {
    #![proptest_config(config())]

    #[test]
    fn arbitrary_bytes(bytes in prop::collection::vec(any::<u8>(), 0..4096)) {
        check(&bytes)?;
    }

    #[test]
    fn arbitrary_bytes_behind_a_v2_header(tail in prop::collection::vec(any::<u8>(), 0..4096)) {
        let mut bytes = 2i32.to_le_bytes().to_vec();
        bytes.extend_from_slice(&tail);
        check(&bytes)?;
    }

    #[test]
    fn mutated_fixtures(which in 0..3usize, ms in prop::collection::vec(mutation(), 1..6)) {
        let mut bytes = seeds()[which].clone();
        for m in &ms {
            apply(&mut bytes, m);
        }
        check(&bytes)?;
    }

    #[test]
    fn structured_tables(bytes in structured()) {
        check(&bytes)?;
    }
}

#[test]
fn fixtures_fit_the_budget() {
    for b in seeds() {
        let base = reset_peak();
        let g = gabe::read(b).unwrap();
        let peak = peak_since_reset(base);
        // A zero peak would mean the tracking allocator is not counting, and every budget
        // check above would pass vacuously.
        assert!(peak > 0, "tracking allocator saw no allocation");
        assert!(
            peak <= budget(b.len()),
            "peak {peak} > budget {}",
            budget(b.len())
        );
        drop(g);
    }
}
