//! Fuzz for `core::geometry::import`: no input panics a reader, and none makes it allocate more
//! than its budget, which is linear in the input. The mesh readers get
//! [`MESH_BYTES_PER_INPUT_BYTE`] per input byte; the `.proj` pieces are measured the same way,
//! except inflation, which deflate lets reach 1,032 output bytes per input byte. The allocation
//! counters are per thread (see common/mod.rs), so tests may run in parallel.
mod common;

#[global_allocator]
static A: common::Tracking = common::Tracking;

use proptest::prelude::*;
use proptest::test_runner::TestCaseError;
use simpa_core::geometry::import::proj::{read_finfo, read_scene_mesh};
use simpa_core::geometry::import::zip::{Archive, crc32, inflate};
use simpa_core::geometry::import::{
    ImportOptions, Unit, Up, Weld, import_proj, read_obj, read_ply, read_stl,
};

const CASES: u32 = 4_000;

/// Allocation allowed per input byte. The costliest inputs measured are binary PLY vertices of
/// three `uchar`s each, used by `list uchar uint` faces, and minimal ASCII PLY/OBJ rows; see
/// `worst_case_inputs_stay_within_budget`, which prints the measured ratios.
const MESH_BYTES_PER_INPUT_BYTE: usize = 64;
const BASE_BYTES: usize = 1 << 20;

fn mesh_budget(len: usize) -> usize {
    MESH_BYTES_PER_INPUT_BYTE * len + BASE_BYTES
}

/// Deflate's ceiling (1,032:1), doubled for the output vector's growth.
fn inflate_budget(len: usize) -> usize {
    2 * 1032 * len + BASE_BYTES
}

fn config(cases: u32) -> ProptestConfig {
    ProptestConfig {
        cases,
        failure_persistence: None,
        ..ProptestConfig::default()
    }
}

#[derive(Clone, Copy, Debug)]
enum Reader {
    Ply,
    Obj,
    Stl,
}

const READERS: [Reader; 3] = [Reader::Ply, Reader::Obj, Reader::Stl];

fn options(weld: bool) -> ImportOptions {
    let mut o = ImportOptions::new(Unit::Millimetre, Up::Y);
    o.weld = if weld { Weld::Exact } else { Weld::Off };
    o
}

/// Reads `bytes` with `reader`, measuring the peak allocation of the read.
fn check(reader: Reader, bytes: &[u8], weld: bool) -> Result<usize, TestCaseError> {
    let o = options(weld);
    let base = common::reset_peak();
    let result = match reader {
        Reader::Ply => read_ply(bytes, &o),
        Reader::Obj => read_obj(bytes, &o),
        Reader::Stl => read_stl(bytes, &o),
    };
    let peak = common::peak_since_reset(base);
    let ok = result.is_ok();
    drop(result);
    prop_assert!(
        peak <= mesh_budget(bytes.len()),
        "{reader:?}: peak {peak} bytes > budget {} for {} input bytes (ok = {ok})",
        mesh_budget(bytes.len()),
        bytes.len()
    );
    Ok(peak)
}

const FIXTURES: [(&str, Reader); 8] = [
    ("box.ply", Reader::Ply),
    ("box_be.ply", Reader::Ply),
    ("box_le.ply", Reader::Ply),
    ("box_mixed.ply", Reader::Ply),
    ("box_mixed_be.ply", Reader::Ply),
    ("box_yup.obj", Reader::Obj),
    ("box.stl", Reader::Stl),
    ("box_binary.stl", Reader::Stl),
];

fn fixture_bytes(name: &str) -> Vec<u8> {
    std::fs::read(common::fixture(&format!("geometry/{name}"))).unwrap()
}

/// Edits applied to a real file: bytes, aligned 32-bit words (binary counts and indices), cuts,
/// splices, and whole text tokens replaced by hostile numbers (text counts and indices).
#[derive(Debug, Clone)]
enum Edit {
    Byte(usize, u8),
    Word(usize, u32),
    Cut(usize),
    Insert(usize, Vec<u8>),
    Remove(usize, usize),
    Token(usize, &'static str),
}

const HOSTILE: [&str; 16] = [
    "0",
    "-1",
    "1",
    "3",
    "255",
    "65535",
    "4294967295",
    "4294967296",
    "18446744073709551615",
    "-9223372036854775808",
    "1e309",
    "nan",
    "inf",
    "-0",
    "0x10",
    "",
];

fn edit() -> impl Strategy<Value = Edit> {
    let word = prop_oneof![
        Just(0u32),
        Just(1),
        Just(3),
        Just(0x7FFF_FFFF),
        Just(u32::MAX),
        0u32..1024,
        any::<u32>(),
    ];
    prop_oneof![
        (any::<usize>(), any::<u8>()).prop_map(|(i, b)| Edit::Byte(i, b)),
        (any::<usize>(), word).prop_map(|(i, w)| Edit::Word(i, w)),
        any::<usize>().prop_map(Edit::Cut),
        (
            any::<usize>(),
            proptest::collection::vec(any::<u8>(), 1..32)
        )
            .prop_map(|(i, v)| Edit::Insert(i, v)),
        (any::<usize>(), 1usize..32).prop_map(|(i, n)| Edit::Remove(i, n)),
        (any::<usize>(), 0..HOSTILE.len()).prop_map(|(i, k)| Edit::Token(i, HOSTILE[k])),
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
            Edit::Token(i, t) => {
                // The i-th maximal run of non-whitespace bytes.
                let mut runs = Vec::new();
                let mut start = None;
                for (k, c) in b.iter().enumerate() {
                    match (c.is_ascii_whitespace(), start) {
                        (false, None) => start = Some(k),
                        (true, Some(s)) => {
                            runs.push((s, k));
                            start = None;
                        }
                        _ => {}
                    }
                }
                if let Some(s) = start {
                    runs.push((s, len));
                }
                if !runs.is_empty() {
                    let (s, e) = runs[i % runs.len()];
                    b.splice(s..e, t.bytes());
                }
            }
            _ => {}
        }
    }
    b
}

proptest! {
    #![proptest_config(config(CASES))]

    #[test]
    fn arbitrary_bytes(which in 0..3usize, weld in any::<bool>(), bytes in proptest::collection::vec(any::<u8>(), 0..2048)) {
        check(READERS[which], &bytes, weld)?;
    }

    #[test]
    fn arbitrary_bytes_after_a_valid_start(
        which in 0..5usize,
        weld in any::<bool>(),
        tail in proptest::collection::vec(any::<u8>(), 0..2048),
    ) {
        let (reader, head): (Reader, &[u8]) = [
            (Reader::Ply, &b"ply\nformat binary_big_endian 1.0\nelement vertex 3\nproperty float x\nproperty float y\nproperty float z\nelement face 1\nproperty list uchar int vertex_indices\nend_header\n"[..]),
            (Reader::Ply, &b"ply\nformat ascii 1.0\nelement vertex 3\nproperty int x\nproperty int y\nproperty int z\nelement face 1\nproperty list uint uint vertex_indices\nproperty int layer_id\nend_header\n"[..]),
            (Reader::Obj, &b"v 0 0 0\nv 1 0 0\nv 0 1 0\nf "[..]),
            (Reader::Stl, &b"solid a\nfacet normal 0 0 1\nouter loop\nvertex "[..]),
            (Reader::Stl, &[0u8; 80][..]),
        ][which];
        let mut bytes = head.to_vec();
        bytes.extend_from_slice(&tail);
        check(reader, &bytes, weld)?;
    }

    #[test]
    fn mutations_of_the_fixtures(
        which in 0..FIXTURES.len(),
        weld in any::<bool>(),
        edits in proptest::collection::vec(edit(), 1..6),
    ) {
        let (name, reader) = FIXTURES[which];
        let bytes = apply(fixture_bytes(name), &edits);
        check(reader, &bytes, weld)?;
    }

    #[test]
    fn proj_pieces_on_arbitrary_bytes(bytes in proptest::collection::vec(any::<u8>(), 0..2048)) {
        let base = common::reset_peak();
        let _ = read_scene_mesh(&bytes);
        let _ = read_finfo(&bytes);
        if let Ok(a) = Archive::parse(&bytes) {
            for e in a.entries() {
                let _ = a.read_entry(e);
            }
        }
        let peak = common::peak_since_reset(base);
        prop_assert!(peak <= inflate_budget(bytes.len()), "peak {peak} for {} bytes", bytes.len());
        let base = common::reset_peak();
        let _ = inflate(&bytes, 1 << 16);
        let peak = common::peak_since_reset(base);
        prop_assert!(peak <= (1 << 17) + BASE_BYTES, "inflate peak {peak}");
    }
}

#[test]
fn maximal_counts_stay_within_budget() {
    // Every 32-bit word of each binary fixture set to u32::MAX, and every text token set to each
    // hostile number, one at a time.
    let mut cases = 0usize;
    for (name, reader) in FIXTURES {
        let raw = fixture_bytes(name);
        for at in (0..raw.len() / 4).map(|i| i * 4) {
            let bytes = apply(raw.clone(), &[Edit::Word(at, u32::MAX)]);
            check(reader, &bytes, true).unwrap_or_else(|e| panic!("{name} word at {at}: {e}"));
            cases += 1;
        }
        let tokens = raw
            .split(|c| c.is_ascii_whitespace())
            .filter(|t| !t.is_empty())
            .count();
        for i in 0..tokens {
            for t in HOSTILE {
                let bytes = apply(raw.clone(), &[Edit::Token(i, t)]);
                check(reader, &bytes, false)
                    .unwrap_or_else(|e| panic!("{name} token {i} = {t:?}: {e}"));
                cases += 1;
            }
        }
    }
    println!("{cases} single-field edits, each within budget");
}

/// The costliest inputs per byte that were found: the measured peak per input byte of each.
#[test]
fn worst_case_inputs_stay_within_budget() {
    let n = 20_000u32;
    // Binary PLY: 3-byte vertices, every one used by a 13-byte triangle, one vertex per face.
    let mut ply = format!(
        "ply\nformat binary_little_endian 1.0\nelement vertex {}\nproperty uchar x\n\
         property uchar y\nproperty uchar z\nelement face {n}\n\
         property list uchar uint vertex_indices\nend_header\n",
        n + 2
    )
    .into_bytes();
    for i in 0..n + 2 {
        ply.extend_from_slice(&[(i % 256) as u8, (i / 256 % 256) as u8, (i / 65536) as u8]);
    }
    for i in 0..n {
        ply.push(3);
        for v in [i, i + 1, i + 2] {
            ply.extend_from_slice(&v.to_le_bytes());
        }
    }
    // ASCII PLY: short rows.
    let mut ascii = format!(
        "ply\nformat ascii 1.0\nelement vertex {}\nproperty uchar x\nproperty uchar y\n\
         property uchar z\nelement face {n}\nproperty list uchar uint vertex_indices\nend_header\n",
        n + 2
    );
    for i in 0..n + 2 {
        ascii.push_str(&format!("{} {} {}\n", i % 256, i / 256 % 256, i / 65536));
    }
    for i in 0..n {
        ascii.push_str(&format!("3 {} {} {}\n", i, i + 1, i + 2));
    }
    // OBJ: short rows, one face per vertex.
    let mut obj = String::new();
    for i in 0..n + 2 {
        obj.push_str(&format!("v {} {} {}\n", i % 10, i / 10 % 10, i / 100));
    }
    for i in 1..=n {
        obj.push_str(&format!("f {} {} {}\n", i, i + 1, i + 2));
    }
    // OBJ: one long polygon.
    let mut fan = String::from("v 0 0 0\nv 1 0 0\nv 0 1 0\nf");
    for i in 0..n {
        fan.push_str(&format!(" {}", i % 3 + 1));
    }
    fan.push('\n');
    // Binary STL: 50-byte facets, all distinct points.
    let mut stl = vec![0u8; 80];
    stl.extend_from_slice(&n.to_le_bytes());
    for i in 0..n {
        stl.extend_from_slice(&[0u8; 12]);
        for k in 0..9u32 {
            stl.extend_from_slice(&((i * 9 + k) as f32).to_le_bytes());
        }
        stl.extend_from_slice(&[0u8; 2]);
    }
    for (name, reader, bytes) in [
        ("binary PLY", Reader::Ply, ply),
        ("ASCII PLY", Reader::Ply, ascii.into_bytes()),
        ("OBJ", Reader::Obj, obj.into_bytes()),
        ("OBJ fan", Reader::Obj, fan.into_bytes()),
        ("binary STL", Reader::Stl, stl),
    ] {
        for weld in [false, true] {
            let peak = check(reader, &bytes, weld).unwrap_or_else(|e| panic!("{name}: {e}"));
            println!(
                "{name} (weld {weld}): {} bytes in, peak {peak} bytes, {:.1} per input byte \
                 (budget {MESH_BYTES_PER_INPUT_BYTE} + {BASE_BYTES} fixed)",
                bytes.len(),
                peak as f64 / bytes.len() as f64
            );
        }
    }
}

/// A stored (uncompressed) zip of `entries`, as the `.proj` reader reads it.
fn stored_zip(entries: &[(String, Vec<u8>)]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut central = Vec::new();
    for (name, data) in entries {
        let offset = out.len() as u32;
        let crc = crc32(data);
        let fields = |sig: u32, v: &mut Vec<u8>| {
            v.extend_from_slice(&sig.to_le_bytes());
        };
        fields(0x0403_4b50, &mut out);
        out.extend_from_slice(&[20, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
        out.extend_from_slice(&crc.to_le_bytes());
        out.extend_from_slice(&(data.len() as u32).to_le_bytes());
        out.extend_from_slice(&(data.len() as u32).to_le_bytes());
        out.extend_from_slice(&(name.len() as u16).to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(name.as_bytes());
        out.extend_from_slice(data);
        fields(0x0201_4b50, &mut central);
        central.extend_from_slice(&[20, 0, 20, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
        central.extend_from_slice(&crc.to_le_bytes());
        central.extend_from_slice(&(data.len() as u32).to_le_bytes());
        central.extend_from_slice(&(data.len() as u32).to_le_bytes());
        central.extend_from_slice(&(name.len() as u16).to_le_bytes());
        central.extend_from_slice(&[0u8; 12]);
        central.extend_from_slice(&offset.to_le_bytes());
        central.extend_from_slice(name.as_bytes());
    }
    let cd_offset = out.len() as u32;
    out.extend_from_slice(&central);
    out.extend_from_slice(&0x0605_4b50u32.to_le_bytes());
    out.extend_from_slice(&[0, 0, 0, 0]);
    out.extend_from_slice(&(entries.len() as u16).to_le_bytes());
    out.extend_from_slice(&(entries.len() as u16).to_le_bytes());
    out.extend_from_slice(&(central.len() as u32).to_le_bytes());
    out.extend_from_slice(&cd_offset.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out
}

/// Tutorial 1's project folder (projet_config.xml, sceneMesh.bin, the .finfo lists), read from
/// upstream's checkout; `None` when it is absent and not required.
fn tutorial1_entries() -> Option<Vec<(String, Vec<u8>)>> {
    let root = std::env::var_os("SIMPA_UPSTREAM")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from(r"B:\repos\I-Simpa-upstream"));
    let path = root.join(r"src/isimpa/resources/doc/tutorial/tutorial 1/tutorial_1.proj");
    if !path.exists() {
        if std::env::var_os("SIMPA_REQUIRE_UPSTREAM").is_some() {
            panic!(
                "SIMPA_REQUIRE_UPSTREAM is set but {} is missing",
                path.display()
            );
        }
        eprintln!("upstream checkout not found ({}); skipping", path.display());
        return None;
    }
    let bytes = std::fs::read(path).unwrap();
    let archive = Archive::parse(&bytes).unwrap();
    let entries: Vec<(String, Vec<u8>)> = archive
        .entries()
        .iter()
        .filter(|e| e.name.starts_with("instance2/") && e.name.matches('/').count() == 1)
        .map(|e| (e.name.clone(), archive.read_entry(e).unwrap()))
        .collect();
    Some(entries)
}

#[test]
fn stored_zip_of_tutorial1_imports_like_the_original() {
    let Some(entries) = tutorial1_entries() else {
        return;
    };
    let p = import_proj(&stored_zip(&entries)).unwrap().project;
    assert_eq!(p.geometry.vertices.len(), 8);
    assert_eq!(p.geometry.faces.len(), 12);
}

proptest! {
    #![proptest_config(config(256))]

    /// Mutations of tutorial 1's scene mesh, face lists and element tree, rebuilt into an
    /// archive: `import_proj` returns, it does not panic.
    #[test]
    fn proj_mutations_do_not_panic(
        which in 0..6usize,
        edits in proptest::collection::vec(edit(), 1..4),
    ) {
        let Some(mut entries) = tutorial1_entries() else {
            return Ok(());
        };
        let k = which % entries.len();
        let mutated = apply(std::mem::take(&mut entries[k].1), &edits);
        entries[k].1 = mutated;
        let _ = import_proj(&stored_zip(&entries));
    }
}
