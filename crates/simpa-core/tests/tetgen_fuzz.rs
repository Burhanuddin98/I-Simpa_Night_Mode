mod common;

#[global_allocator]
static A: common::Tracking = common::Tracking;

use proptest::prelude::*;
use proptest::test_runner::TestCaseError;
use simpa_core::formats::FormatError;
use simpa_core::formats::tetgen::{self, Kind};

/// Reads `bytes` as every kind of TetGen file; none may panic or outgrow the budget.
fn read_all_kinds(bytes: &[u8]) -> Result<(), TestCaseError> {
    for kind in Kind::ALL {
        let base = common::reset_peak();
        let result = tetgen::read(kind, bytes);
        let peak = common::peak_since_reset(base);
        drop(result);
        let budget = common::budget(bytes.len());
        prop_assert!(
            peak <= budget,
            "{kind:?}: peak {peak} > budget {budget} for {} bytes",
            bytes.len()
        );
    }
    Ok(())
}

fn fixture_bytes() -> Vec<Vec<u8>> {
    [
        "solver-outputs/tutorial1/tetgen_scene_mesh.1.node",
        "solver-outputs/tutorial1/tetgen_scene_mesh.1.ele",
        "solver-outputs/tutorial1/tetgen_scene_mesh.1.face",
        "solver-outputs/tutorial1/tetgen_scene_mesh.1.neigh",
        "solver-outputs/nightmode-2026-09-08/tcr_model_skipped.face",
        "solver-outputs/nightmode-2026-09-08/tcr_model_skipped.node",
    ]
    .iter()
    .map(|rel| std::fs::read(common::fixture(rel)).unwrap())
    .collect()
}

/// One edit to a fixture's bytes.
#[derive(Clone, Debug)]
enum Edit {
    Flip(usize, u8),
    Insert(usize, u8),
    Delete(usize, usize),
    Truncate(usize),
    DigitSwap(usize, u8),
    DuplicateLine(usize),
    DeleteLine(usize),
    InsertText(usize, &'static str),
}

fn edit() -> impl Strategy<Value = Edit> {
    let text = prop::sample::select(vec![
        "-1",
        "0",
        "4294967296",
        "2147483647",
        "1e999",
        "#",
        ",",
        "\r",
        "\n",
        "\0",
        "\u{1a}",
        " 9 ",
        "rbox",
        "0x1",
        "010",
        ".",
        "-",
        "nan",
        "\t",
        "99999999999",
    ]);
    prop_oneof![
        (any::<usize>(), any::<u8>()).prop_map(|(i, b)| Edit::Flip(i, b)),
        (any::<usize>(), any::<u8>()).prop_map(|(i, b)| Edit::Insert(i, b)),
        (any::<usize>(), 1usize..64).prop_map(|(i, n)| Edit::Delete(i, n)),
        any::<usize>().prop_map(Edit::Truncate),
        (any::<usize>(), b'0'..=b'9').prop_map(|(i, d)| Edit::DigitSwap(i, d)),
        any::<usize>().prop_map(Edit::DuplicateLine),
        any::<usize>().prop_map(Edit::DeleteLine),
        (any::<usize>(), text).prop_map(|(i, t)| Edit::InsertText(i, t)),
    ]
}

fn line_span(b: &[u8], at: usize) -> (usize, usize) {
    let start = b[..at]
        .iter()
        .rposition(|&c| c == b'\n')
        .map_or(0, |p| p + 1);
    let end = b[at..]
        .iter()
        .position(|&c| c == b'\n')
        .map_or(b.len(), |p| at + p + 1);
    (start, end)
}

fn apply(b: &mut Vec<u8>, e: &Edit) {
    if b.is_empty() {
        return;
    }
    let at = |i: usize| i % b.len();
    match *e {
        Edit::Flip(i, v) => {
            let i = at(i);
            b[i] = v;
        }
        Edit::Insert(i, v) => {
            let i = at(i);
            b.insert(i, v);
        }
        Edit::Delete(i, n) => {
            let i = at(i);
            let end = (i + n).min(b.len());
            b.drain(i..end);
        }
        Edit::Truncate(i) => {
            let i = at(i);
            b.truncate(i);
        }
        Edit::DigitSwap(i, d) => {
            // Replace the next digit at or after i: counts, indices and coordinates change.
            let i = at(i);
            if let Some(p) = b[i..].iter().position(u8::is_ascii_digit) {
                b[i + p] = d;
            }
        }
        Edit::DuplicateLine(i) => {
            let (s, t) = line_span(b, at(i));
            let line = b[s..t].to_vec();
            b.splice(t..t, line);
        }
        Edit::DeleteLine(i) => {
            let (s, t) = line_span(b, at(i));
            b.drain(s..t);
        }
        Edit::InsertText(i, text) => {
            let i = at(i);
            b.splice(i..i, text.bytes());
        }
    }
}

/// Text in TetGen's alphabet, so cases get past the header into the record parsers.
fn tetgen_like_text() -> impl Strategy<Value = Vec<u8>> {
    let token = prop_oneof![
        4 => (-2i64..12).prop_map(|v| v.to_string()),
        1 => any::<i32>().prop_map(|v| v.to_string()),
        1 => any::<f64>().prop_map(|v| format!("{v:e}")),
        1 => "[0-9.eE+-]{1,5}",
        1 => prop::sample::select(vec!["#", "rbox", "-0", "1e999", "0x1", "010", ",", "\u{1a}", "\0"])
            .prop_map(str::to_string),
    ];
    let sep = prop::sample::select(vec![" ", "  ", "\t", ",", " , "]);
    let line = prop::collection::vec((token, sep), 0..8).prop_map(|parts| {
        parts
            .into_iter()
            .map(|(t, s)| format!("{t}{s}"))
            .collect::<String>()
    });
    let eol = prop::sample::select(vec!["\n", "\r\n", "\r"]);
    prop::collection::vec((line, eol), 0..40).prop_map(|lines| {
        lines
            .into_iter()
            .map(|(l, e)| format!("{l}{e}"))
            .collect::<String>()
            .into_bytes()
    })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(10_000))]

    #[test]
    fn arbitrary_bytes_never_panic_or_overallocate(bytes in prop::collection::vec(any::<u8>(), 0..2048)) {
        read_all_kinds(&bytes)?;
    }

    #[test]
    fn tetgen_like_text_never_panics_or_overallocates(bytes in tetgen_like_text()) {
        read_all_kinds(&bytes)?;
    }

    #[test]
    fn mutated_fixtures_never_panic_or_overallocate(
        which in 0usize..6,
        edits in prop::collection::vec(edit(), 1..6),
    ) {
        thread_local! { static FIXTURES: Vec<Vec<u8>> = fixture_bytes(); }
        let mut bytes = FIXTURES.with(|f| f[which].clone());
        for e in &edits {
            apply(&mut bytes, e);
        }
        read_all_kinds(&bytes)?;
    }
}

// Deterministic worst cases for the budget: the densest files each kind allows.

fn peak_of(kind: Kind, bytes: &[u8]) -> (usize, Result<(), String>) {
    let base = common::reset_peak();
    let r = tetgen::read(kind, bytes);
    let peak = common::peak_since_reset(base);
    (peak, r.map(|_| ()).map_err(|e| e.to_string()))
}

#[test]
fn densest_valid_files_stay_inside_the_budget() {
    // Points, tetrahedra, faces and neighbours with one-character values: the most records per
    // byte each layout allows. All are valid and must be read, within budget.
    let n = 300_000;
    let mut node = format!("{n} 3 0 0\n").into_bytes();
    let mut ele = format!("{n} 4 1\n").into_bytes();
    let mut face = format!("{n} 1\n").into_bytes();
    let mut neigh = format!("{n} 4\n").into_bytes();
    for i in 0..n {
        let i = i + 1;
        node.extend(format!("{i} 0 0 0\n").bytes());
        ele.extend(format!("{i} 1 1 1 1 0\n").bytes());
        face.extend(format!("{i} 1 1 1 0\n").bytes());
        neigh.extend(format!("{i} -1 1 1 1\n").bytes());
    }
    for (kind, bytes) in [
        (Kind::Node, &node),
        (Kind::Ele, &ele),
        (Kind::Face, &face),
        (Kind::Neigh, &neigh),
    ] {
        let (peak, r) = peak_of(kind, bytes);
        assert!(r.is_ok(), "{kind:?}: {r:?}");
        let budget = common::budget(bytes.len());
        assert!(peak <= budget, "{kind:?}: peak {peak} > budget {budget}");
    }
}

#[test]
fn attribute_heavy_files_are_refused_inside_the_budget() {
    // 20 one-character attributes per point: 184 decoded bytes per ~52-byte line. Refused
    // before anything is reserved, as the spec says.
    let n = 50_000;
    let mut node = format!("{n} 3 20 0\n").into_bytes();
    for i in 1..=n {
        node.extend(format!("{i} 0 0 0{}\n", " 0".repeat(20)).bytes());
    }
    let (peak, r) = peak_of(Kind::Node, &node);
    assert!(r.as_ref().is_err_and(|m| m.contains("budget")), "{r:?}");
    assert!(peak <= common::budget(node.len()));
}

#[test]
fn huge_count_headers_allocate_nothing_large() {
    let cases: [(Kind, &[u8]); 4] = [
        (Kind::Node, b"2147483647 3 100 1\n1 0 0 0\n"),
        (Kind::Ele, b"2147483647 10 100\n1 1 1 1 1 1 1 1 1 1 1\n"),
        (Kind::Face, b"2147483647 1\n1 1 2 3 0\n"),
        (Kind::Neigh, b"2147483647 4\n1 -1 -1 -1 -1\n"),
    ];
    for (kind, bytes) in cases {
        let base = common::reset_peak();
        let r = tetgen::read(kind, bytes);
        let peak = common::peak_since_reset(base);
        assert!(
            matches!(r, Err(FormatError::Truncated { .. })),
            "{kind:?}: {r:?}"
        );
        assert!(
            peak <= 4096,
            "{kind:?}: {peak} bytes for a {}-byte file",
            bytes.len()
        );
    }
}
