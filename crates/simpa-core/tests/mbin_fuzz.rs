mod common;

#[global_allocator]
static A: common::Tracking = common::Tracking;

// The allocation counters are per thread (see common/mod.rs), so tests may run in parallel.

use std::sync::OnceLock;
use std::sync::atomic::{AtomicU32, Ordering};

use proptest::prelude::*;
use proptest::test_runner::{Config, TestRunner};
use simpa_core::formats::FormatError;
use simpa_core::formats::mbin;

const CASES: u32 = 10_000;

fn fixture_bytes(rel: &'static str, cell: &'static OnceLock<Vec<u8>>) -> Vec<u8> {
    cell.get_or_init(|| std::fs::read(common::fixture(rel)).unwrap())
        .clone()
}

fn cube() -> Vec<u8> {
    static CELL: OnceLock<Vec<u8>> = OnceLock::new();
    fixture_bytes("upstream/lib_interface/cube_mesh.mbin", &CELL)
}

fn tutorial() -> Vec<u8> {
    static CELL: OnceLock<Vec<u8>> = OnceLock::new();
    fixture_bytes("upstream/tutorial1/spps/tetramesh.mbin", &CELL)
}

/// Runs `read` under the allocation meter; checks the budget, and that a successful read
/// consumed exactly `8 + 12n + 100t` bytes, re-encodes to that prefix and dumps without panic.
fn check(bytes: &[u8]) -> Result<(), TestCaseError> {
    let base = common::reset_peak();
    let r = mbin::read(bytes);
    let peak = common::peak_since_reset(base);
    prop_assert!(
        peak <= common::budget(bytes.len()),
        "peak {peak} > budget {} for {} bytes",
        common::budget(bytes.len()),
        bytes.len()
    );
    match r {
        Ok(mesh) => {
            let used = 8 + 12 * mesh.nodes.len() + 100 * mesh.tetrahedra.len();
            prop_assert!(used <= bytes.len());
            prop_assert_eq!(&mbin::write(&mesh)[..], &bytes[..used]);
            let dump = mbin::dump(&mesh);
            prop_assert_eq!(
                dump.lines().count(),
                3 + mesh.nodes.len() + mesh.tetrahedra.len()
            );
        }
        Err(FormatError::Truncated { .. }) => {}
        Err(e) => prop_assert!(false, "unexpected error kind: {e}"),
    }
    Ok(())
}

/// A header with small counts in front of arbitrary bytes, so the success path is exercised too.
fn small_header_then_bytes() -> impl Strategy<Value = Vec<u8>> {
    (
        0u32..12,
        0u32..24,
        prop::collection::vec(any::<u8>(), 0..1600),
    )
        .prop_map(|(t, n, body)| {
            let mut b = Vec::with_capacity(8 + body.len());
            b.extend_from_slice(&t.to_le_bytes());
            b.extend_from_slice(&n.to_le_bytes());
            b.extend_from_slice(&body);
            b
        })
}

#[derive(Debug, Clone)]
enum Mutation {
    Flip { at: usize, xor: u8 },
    Set { at: usize, byte: u8 },
    Truncate { len: usize },
    Extend { bytes: Vec<u8> },
    Header { tetra: u32, nodes: u32 },
    Splice { at: usize, bytes: Vec<u8> },
}

fn mutation() -> impl Strategy<Value = Mutation> {
    prop_oneof![
        (any::<usize>(), 1u8..=255).prop_map(|(at, xor)| Mutation::Flip { at, xor }),
        (any::<usize>(), any::<u8>()).prop_map(|(at, byte)| Mutation::Set { at, byte }),
        any::<usize>().prop_map(|len| Mutation::Truncate { len }),
        prop::collection::vec(any::<u8>(), 1..300).prop_map(|bytes| Mutation::Extend { bytes }),
        (any::<u32>(), any::<u32>()).prop_map(|(tetra, nodes)| Mutation::Header { tetra, nodes }),
        (0u32..16, 0u32..16).prop_map(|(tetra, nodes)| Mutation::Header { tetra, nodes }),
        (any::<usize>(), prop::collection::vec(any::<u8>(), 1..64))
            .prop_map(|(at, bytes)| Mutation::Splice { at, bytes }),
    ]
}

fn apply(mut b: Vec<u8>, muts: &[Mutation]) -> Vec<u8> {
    for m in muts {
        match m {
            Mutation::Flip { at, xor } if !b.is_empty() => {
                let i = at % b.len();
                b[i] ^= xor;
            }
            Mutation::Set { at, byte } if !b.is_empty() => {
                let i = at % b.len();
                b[i] = *byte;
            }
            Mutation::Truncate { len } => {
                let keep = len % (b.len() + 1);
                b.truncate(keep);
            }
            Mutation::Extend { bytes } => b.extend_from_slice(bytes),
            Mutation::Header { tetra, nodes } if b.len() >= 8 => {
                b[0..4].copy_from_slice(&tetra.to_le_bytes());
                b[4..8].copy_from_slice(&nodes.to_le_bytes());
            }
            Mutation::Splice { at, bytes } => {
                let i = at % (b.len() + 1);
                b.splice(i..i, bytes.iter().copied());
            }
            _ => {}
        }
    }
    b
}

fn config() -> Config {
    Config {
        cases: CASES,
        failure_persistence: None,
        ..Config::default()
    }
}

/// Runs `CASES` cases of `strategy` through `check` and proves that many actually ran.
fn run<S: Strategy>(strategy: S, to_bytes: impl Fn(S::Value) -> Vec<u8>) {
    let ran = AtomicU32::new(0);
    let mut runner = TestRunner::new(config());
    let result = runner.run(&strategy, |value| {
        ran.fetch_add(1, Ordering::Relaxed);
        check(&to_bytes(value))
    });
    if let Err(e) = result {
        panic!("{e}");
    }
    let ran = ran.load(Ordering::Relaxed);
    assert!(ran >= CASES, "only {ran} of {CASES} cases ran");
    eprintln!("{ran} cases passed");
}

#[test]
fn arbitrary_bytes_never_panic_or_overallocate() {
    run(
        prop_oneof![
            prop::collection::vec(any::<u8>(), 0..2048),
            small_header_then_bytes(),
        ],
        |bytes| bytes,
    );
}

#[test]
fn mutated_cube_never_panics_or_overallocates() {
    run(prop::collection::vec(mutation(), 1..6), |muts| {
        apply(cube(), &muts)
    });
}

#[test]
fn mutated_tutorial_mesh_never_panics_or_overallocates() {
    run(prop::collection::vec(mutation(), 1..4), |muts| {
        apply(tutorial(), &muts)
    });
}

#[test]
fn every_prefix_of_every_fixture() {
    for rel in [
        "upstream/lib_interface/cube_mesh.mbin",
        "upstream/python_bindings/tetramesh.mbin",
    ] {
        let bytes = std::fs::read(common::fixture(rel)).unwrap();
        for n in 0..=bytes.len() {
            check(&bytes[..n]).unwrap();
        }
    }
}

#[test]
fn huge_counts_stay_within_budget() {
    for (t, n) in [
        (u32::MAX, u32::MAX),
        (u32::MAX, 0),
        (0, u32::MAX),
        (42_949_673, 8),
        (1 << 24, 1 << 24),
    ] {
        for body in [0usize, 1, 99, 100, 4096] {
            let mut b = Vec::new();
            b.extend_from_slice(&t.to_le_bytes());
            b.extend_from_slice(&n.to_le_bytes());
            b.resize(8 + body, 0x5a);
            check(&b).unwrap();
            assert!(mbin::read(&b).is_err(), "t={t} n={n} body={body}");
        }
    }
}
