mod common;

#[global_allocator]
static A: common::Tracking = common::Tracking;

use proptest::prelude::*;
use proptest::test_runner::TestCaseError;

use simpa_core::formats::pbin::{self, FILE_HEADER_SIZE, PARTICLE_HEADER_SIZE, TIME_STEP_SIZE};

const CASES: u32 = 10_000;

fn config() -> ProptestConfig {
    // No failure persistence: a failing case is printed, never written into the source tree.
    ProptestConfig {
        cases: CASES,
        failure_persistence: None,
        ..ProptestConfig::default()
    }
}

fn fixture() -> &'static [u8] {
    static BYTES: std::sync::OnceLock<Vec<u8>> = std::sync::OnceLock::new();
    BYTES.get_or_init(|| {
        std::fs::read(common::fixture("solver-outputs/tutorial1/spps_50hz.pbin")).unwrap()
    })
}

/// Reads `bytes` under the allocation budget; a successful read must describe exactly `bytes`.
fn check(bytes: &[u8]) -> Result<(), TestCaseError> {
    let base = common::reset_peak();
    let r = pbin::read(bytes);
    let peak = common::peak_since_reset(base);
    prop_assert!(
        peak <= common::budget(bytes.len()),
        "peak {} > budget {} for {} bytes",
        peak,
        common::budget(bytes.len()),
        bytes.len()
    );
    if let Ok(f) = r {
        prop_assert_eq!(f.header.format_version, pbin::FORMAT_VERSION);
        prop_assert_eq!(f.particles.len(), f.header.nb_particles as usize);
        let total: usize = f.particles.iter().map(|p| p.nb_time_step as usize).sum();
        prop_assert_eq!(total, f.steps.len());
        prop_assert_eq!(
            FILE_HEADER_SIZE + f.particles.len() * PARTICLE_HEADER_SIZE + total * TIME_STEP_SIZE,
            bytes.len()
        );
        // The dump never panics and has one line per record.
        let d = pbin::dump(&f);
        prop_assert_eq!(d.lines().count(), 3 + f.particles.len() + f.steps.len());
    }
    Ok(())
}

/// Byte-level edits applied to the fixture.
#[derive(Debug, Clone)]
enum Edit {
    Byte(usize, u8),
    /// Overwrite the `u32` at a 4-aligned offset (header fields and step counts live there).
    Word(usize, u32),
    Truncate(usize),
    Append(Vec<u8>),
}

fn edit() -> impl Strategy<Value = Edit> {
    prop_oneof![
        4 => (0usize..2048, any::<u8>()).prop_map(|(i, b)| Edit::Byte(i, b)),
        3 => (0usize..512, prop_oneof![any::<u32>(), 0u32..64]).prop_map(|(i, w)| Edit::Word(i * 4, w)),
        2 => (0usize..2048).prop_map(Edit::Truncate),
        1 => proptest::collection::vec(any::<u8>(), 1..64).prop_map(Edit::Append),
    ]
}

fn apply(mut bytes: Vec<u8>, edits: &[Edit]) -> Vec<u8> {
    for e in edits {
        match e {
            Edit::Byte(i, b) if *i < bytes.len() => bytes[*i] = *b,
            Edit::Word(i, w) if i + 4 <= bytes.len() => {
                bytes[*i..*i + 4].copy_from_slice(&w.to_le_bytes())
            }
            Edit::Truncate(n) => bytes.truncate(*n),
            Edit::Append(tail) => bytes.extend_from_slice(tail),
            _ => {}
        }
    }
    bytes
}

/// A version-1 file built from parts, some consistent and some not: arbitrary header fields,
/// particle headers whose step counts may lie, and arbitrary step bytes.
fn structured() -> impl Strategy<Value = Vec<u8>> {
    let header = (
        // None: the true particle count.
        prop_oneof![
            2 => Just(None),
            1 => (0u32..12).prop_map(Some),
            1 => any::<u32>().prop_map(Some)
        ],
        // Struct sizes: mostly the Windows layout's, which is all the reader accepts.
        prop_oneof![7 => Just([28u32, 16, 8]), 1 => any::<[u32; 3]>()],
        any::<u32>(), // nbTimeStepMax
        any::<u32>(), // timeStep bits
    );
    let particle = (
        prop_oneof![7 => 0u32..6, 1 => any::<u32>()],
        any::<u16>(),
        any::<u16>(),
        // None: exactly the step bytes the count declares.
        prop_oneof![7 => Just(None), 1 => (0usize..96).prop_map(Some)],
    );
    let tail = prop_oneof![
        3 => Just(Vec::new()),
        1 => proptest::collection::vec(any::<u8>(), 1..24)
    ];
    (
        header,
        proptest::collection::vec(particle, 0..12),
        tail,
        any::<u64>(),
    )
        .prop_map(|((nb, [fil, pil, phil], max, dt), parts, tail, seed)| {
            let nb = nb.unwrap_or(parts.len() as u32);
            let mut b = Vec::new();
            for v in [nb, 1, fil, pil, phil, max, dt] {
                b.extend_from_slice(&v.to_le_bytes());
            }
            let mut x = seed | 1;
            for (count, first, pad, step_bytes) in parts {
                b.extend_from_slice(&count.to_le_bytes());
                b.extend_from_slice(&first.to_le_bytes());
                b.extend_from_slice(&pad.to_le_bytes());
                // Honest step data for small counts, a lie of arbitrary length otherwise.
                let n = step_bytes.unwrap_or((count as usize).min(64) * TIME_STEP_SIZE);
                for _ in 0..n {
                    x ^= x << 13;
                    x ^= x >> 7;
                    x ^= x << 17;
                    b.push(x as u8);
                }
            }
            b.extend_from_slice(&tail);
            b
        })
}

proptest! {
    #![proptest_config(config())]

    #[test]
    fn arbitrary_bytes(bytes in proptest::collection::vec(any::<u8>(), 0..2048)) {
        check(&bytes)?;
    }

    /// Arbitrary bytes after a particle count, the version and the struct sizes, so the random
    /// part reaches the count checks instead of stopping at the version.
    #[test]
    fn arbitrary_bytes_behind_a_valid_prefix(
        nb in prop_oneof![0u32..40, any::<u32>()],
        rest in proptest::collection::vec(any::<u8>(), 0..2048),
    ) {
        let mut bytes = Vec::with_capacity(20 + rest.len());
        for v in [nb, 1, 28, 16, 8] {
            bytes.extend_from_slice(&v.to_le_bytes());
        }
        bytes.extend_from_slice(&rest);
        check(&bytes)?;
    }

    #[test]
    fn fixture_mutations(edits in proptest::collection::vec(edit(), 1..8)) {
        check(&apply(fixture().to_vec(), &edits))?;
    }

    #[test]
    fn structured_version_1_files(bytes in structured()) {
        check(&bytes)?;
    }
}

/// The structured generator must reach the success path often enough to matter, and the
/// failure paths too; otherwise its 10,000 cases prove little.
#[test]
fn structured_generator_reaches_both_outcomes() {
    use proptest::strategy::ValueTree;
    let mut runner = proptest::test_runner::TestRunner::deterministic();
    let strategy = structured();
    let (mut ok, mut err) = (0, 0);
    for _ in 0..1000 {
        let bytes = strategy.new_tree(&mut runner).unwrap().current();
        if pbin::read(&bytes).is_ok() {
            ok += 1;
        } else {
            err += 1;
        }
    }
    assert!(ok >= 100 && err >= 100, "ok {ok}, err {err}");
}

#[test]
fn fixture_reads_within_budget() {
    check(fixture()).unwrap();
    assert!(pbin::read(fixture()).is_ok());
}

#[test]
fn huge_counts_reserve_nothing() {
    for (at, v) in [
        (0usize, u32::MAX),
        (0, 1 << 29),
        (28, u32::MAX),
        (28, 1 << 30),
    ] {
        let mut bytes = fixture().to_vec();
        bytes[at..at + 4].copy_from_slice(&v.to_le_bytes());
        let base = common::reset_peak();
        let r = pbin::read(&bytes);
        let peak = common::peak_since_reset(base);
        assert!(r.is_err(), "count {v} at {at} accepted");
        assert!(
            peak <= common::budget(bytes.len()),
            "count {v} at {at}: peak {peak}"
        );
    }
}
