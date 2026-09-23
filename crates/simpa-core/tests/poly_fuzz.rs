//! The `.poly` reader under hostile input: no panic, and never more than `budget(len)` bytes
//! allocated, for arbitrary bytes, for mutations of real files and of generated ones, for token
//! soups that reach deep into the grammar, and for the worst cases of the size argument in
//! docs/formats/poly.md. Run with `--test-threads=1`: the allocation counters are process-wide.
mod common;

#[global_allocator]
static A: common::Tracking = common::Tracking;

use std::sync::OnceLock;

use proptest::prelude::*;
use simpa_core::formats::poly;

/// Reads `bytes`, failing the case if the peak allocation exceeds the budget.
fn read_within_budget(bytes: &[u8]) -> std::result::Result<(), TestCaseError> {
    let base = common::reset_peak();
    let result = poly::read(bytes);
    let peak = common::peak_since_reset(base);
    drop(result);
    let budget = common::budget(bytes.len());
    prop_assert!(
        peak <= budget,
        "peak {} > budget {} for {} bytes",
        peak,
        budget,
        bytes.len()
    );
    Ok(())
}

fn config(cases: u32) -> ProptestConfig {
    ProptestConfig {
        cases,
        failure_persistence: None,
        ..ProptestConfig::default()
    }
}

/// Real files to mutate: both fixtures and a few generated models.
fn seeds() -> &'static [Vec<u8>] {
    static SEEDS: OnceLock<Vec<Vec<u8>>> = OnceLock::new();
    SEEDS.get_or_init(|| {
        let mut v = vec![
            std::fs::read(common::fixture("upstream/lib_interface/test_import1.poly")).unwrap(),
            std::fs::read(common::fixture("upstream/tutorial1/tetgen/scene_mesh.poly")).unwrap(),
        ];
        v.extend((0..6).map(|s| poly::write(&poly::generate(s))));
        v
    })
}

const INTERESTING: &[u8] = b" \t\r\n#-+.eE0123456789x\x0b\x0c\0\x1a,\xef";

fn mutate(mut bytes: Vec<u8>, edits: &[(u32, u8, u8)]) -> Vec<u8> {
    for &(pos, kind, byte) in edits {
        let at = pos as usize % (bytes.len() + 1);
        let pick = INTERESTING[byte as usize % INTERESTING.len()];
        match kind {
            0 if at < bytes.len() => bytes[at] = byte,
            1 if at < bytes.len() => bytes[at] = pick,
            2 => bytes.insert(at, pick),
            3 if at < bytes.len() => {
                bytes.remove(at);
            }
            4 => bytes.truncate(at),
            5 => {
                let len = (byte as usize % 24).min(bytes.len() - at);
                let copy: Vec<u8> = bytes[at..at + len].to_vec();
                let to = (pos as usize / 7) % (bytes.len() + 1);
                bytes.splice(to..to, copy);
            }
            _ => bytes.push(pick),
        }
    }
    bytes
}

const VOCAB: &[&str] = &[
    "0",
    "1",
    "2",
    "3",
    "4",
    "8",
    "12",
    "-0",
    "-1",
    "+1",
    "1.5",
    "1e5",
    "1e999",
    "1e-999",
    ".5",
    "#",
    "# c",
    "x",
    "nan",
    "0x1",
    "4294967295",
    "4294967296",
    "2147483647",
    "-2147483648",
    "99999999999",
    "3  1 2 3",
    "4 1 2 3 4",
    "1  0  0",
    "0  1",
    "8  3  0  0",
    "1 0 0 0",
];

const SEPARATORS: &[&str] = &[" ", "\n", "\r\n", "\t", "  ", "\n\n"];

proptest! {
    #![proptest_config(config(10_000))]

    #[test]
    fn arbitrary_bytes(bytes in proptest::collection::vec(any::<u8>(), 0..2048)) {
        read_within_budget(&bytes)?;
    }

    #[test]
    fn mutated_real_files(
        which in 0usize..8,
        edits in proptest::collection::vec((any::<u32>(), 0u8..7, any::<u8>()), 1..10),
    ) {
        let seeds = seeds();
        let bytes = mutate(seeds[which % seeds.len()].clone(), &edits);
        read_within_budget(&bytes)?;
    }
}

proptest! {
    #![proptest_config(config(5_000))]

    /// A real prefix (cut anywhere) followed by a soup of plausible tokens.
    #[test]
    fn prefix_then_token_soup(
        which in 0usize..8,
        cut in any::<u32>(),
        soup in proptest::collection::vec((0usize..VOCAB.len(), 0usize..SEPARATORS.len()), 0..120),
    ) {
        let seeds = seeds();
        let seed = &seeds[which % seeds.len()];
        let mut bytes = seed[..cut as usize % (seed.len() + 1)].to_vec();
        for (t, s) in soup {
            bytes.extend_from_slice(VOCAB[t].as_bytes());
            bytes.extend_from_slice(SEPARATORS[s].as_bytes());
        }
        read_within_budget(&bytes)?;
    }
}

fn check(bytes: &[u8]) -> poly::Model {
    let base = common::reset_peak();
    let result = poly::read(bytes);
    let peak = common::peak_since_reset(base);
    let budget = common::budget(bytes.len());
    assert!(
        peak <= budget,
        "peak {peak} > budget {budget} for {} bytes",
        bytes.len()
    );
    result.expect("worst-case input is valid")
}

#[test]
fn minimal_node_lines_stay_within_budget() {
    // 24 bytes of model per node line of digits(k) + 7 bytes: the worst ratio in the format.
    for n in [9usize, 99, 999, 9_999, 99_999, 150_000] {
        let mut text = format!("{n} 3 0 0\n");
        for k in 1..=n {
            text.push_str(&format!("{k} 0 0 0\n"));
        }
        text.push_str("0 0\n0\n");
        let m = check(text.as_bytes());
        assert_eq!(m.model_vertices.len(), n);
    }
}

#[test]
fn minimal_quads_stay_within_budget() {
    // "1 0 0\n4 1 1 1 1\n" is 16 bytes and yields two 16-byte triangles.
    for f in [1usize, 1_000, 100_000] {
        let mut text = format!("1 3 0 0\n1 0 0 0\n{f} 0\n");
        for _ in 0..f {
            text.push_str("1 0 0\n4 1 1 1 1\n");
        }
        text.push_str("0\n0\n");
        text.push_str(&format!("{f}\n"));
        for _ in 0..f {
            text.push_str("1 0 0\n4 1 1 1 1\n");
        }
        let m = check(text.as_bytes());
        assert_eq!(
            (m.model_faces.len(), m.user_defined_faces.len()),
            (2 * f, 2 * f)
        );
    }
}

#[test]
fn minimal_regions_stay_within_budget() {
    let n = 50_000usize;
    let mut text = String::from("0 3 0 0\n0 0\n0\n");
    text.push_str(&format!("{n}\n"));
    for k in 1..=n {
        text.push_str(&format!("{k} 0 0 0 0 0\n"));
    }
    let m = check(text.as_bytes());
    assert_eq!(m.model_regions.len(), n);
}

#[test]
fn lying_headers_allocate_nothing() {
    let cases = [
        "4294967295  3  0  0\n1 0 0 0\n",
        "0 3 0 0\n4294967295 1\n1 0 0\n",
        "0 3 0 0\n0 1\n2147483647\n1 0 0 0\n",
        "0 3 0 0\n0 1\n0\n2147483647\n1 0 0 0 0 -1\n",
        "0 3 0 0\n0 1\n0\n0\n2147483647 1\n1 0 0\n",
    ];
    for text in cases {
        let base = common::reset_peak();
        let result = poly::read(text.as_bytes());
        let peak = common::peak_since_reset(base);
        assert!(result.is_err(), "{text:?}");
        drop(result);
        assert!(peak <= common::budget(text.len()), "{text:?}: peak {peak}");
    }
}

#[test]
fn huge_fields_stay_within_budget() {
    // 1 MiB lines are refused for their length; fields just short of the 2047-byte line limit
    // reach the number parsers.
    let digits = "1".repeat(1 << 20);
    let controls = "\x01".repeat(1 << 20);
    let long_digits = "1".repeat(2030);
    let long_zeros = format!("{}1", "0".repeat(2029));
    for field in [&digits, &controls, &long_digits, &long_zeros] {
        for text in [
            format!("1 3 0 0\n1 {field} 0 0\n0 0\n0\n"),
            format!("{field} 3 0 0\n"),
            format!("1 3 0 0\n1 0.{field} 0 0\n0 0\n0\n"),
        ] {
            let base = common::reset_peak();
            let result = poly::read(text.as_bytes());
            let peak = common::peak_since_reset(base);
            drop(result);
            assert!(
                peak <= common::budget(text.len()),
                "peak {peak} for {} bytes",
                text.len()
            );
        }
    }
}
