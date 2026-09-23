mod common;

use std::path::{Path, PathBuf};

use common::fixture;
use simpa_core::formats::FormatError;
use simpa_core::formats::gabe::{self, ColumnData, Gabe};

/// Every GABE-family fixture: tables, receiver series and upstream's retro-compatibility file.
const FIXTURES: &[&str] = &[
    "solver-outputs/nightmode-2026-09-08/spps_stats.gabe",
    "solver-outputs/nightmode-2026-09-08/tcr_main_results.gabe",
    "solver-outputs/tutorial1/spps_stats.gabe",
    "solver-outputs/tutorial1/tcr_main_results.gabe",
    "solver-outputs/tutorial1/spps_punctual.recp",
    "solver-outputs/tutorial1/spps_total_energy.recp",
    "upstream/python_bindings/retrocompat_test.gabe",
];

fn bytes(rel: &str) -> Vec<u8> {
    std::fs::read(fixture(rel)).unwrap()
}

fn load(rel: &str) -> Gabe {
    gabe::read_file(&fixture(rel)).unwrap_or_else(|e| panic!("{rel}: {e}"))
}

fn int_row(g: &Gabe, row: usize) -> Vec<i32> {
    g.columns
        .iter()
        .filter_map(|c| c.ints())
        .map(|v| v[row])
        .collect()
}

fn round3(v: &[f32]) -> Vec<f64> {
    v.iter()
        .map(|&x| (f64::from(x) * 1000.0).round() / 1000.0)
        .collect()
}

#[test]
fn every_fixture_reads_and_consumes_every_byte() {
    for rel in FIXTURES {
        let b = bytes(rel);
        let (g, used) = gabe::read_prefix(&b).unwrap_or_else(|e| panic!("{rel}: {e}"));
        assert_eq!(used, b.len(), "{rel}: consumed {used} of {} bytes", b.len());
        assert_eq!(g.version, 2, "{rel}");
        assert!(!g.columns.is_empty(), "{rel}");
    }
}

#[test]
fn nightmode_spps_stats() {
    let rel = "solver-outputs/nightmode-2026-09-08/spps_stats.gabe";
    let b = bytes(rel);
    let (g, used) = gabe::read_prefix(&b).unwrap();
    assert_eq!((used, b.len()), (2505, 2505));
    assert!(g.read_only);
    assert_eq!(g.columns.len(), 7);
    let labels: Vec<&[u8]> = g.columns.iter().map(|c| c.label.as_slice()).collect();
    assert_eq!(
        labels,
        [
            &b""[..],
            b"125 Hz",
            b"250 Hz",
            b"500 Hz",
            b"1000 Hz",
            b"2000 Hz",
            b"4000 Hz"
        ]
    );
    let rows = g.row_labels().unwrap();
    assert_eq!(rows.len(), 7);
    assert_eq!(rows[6], b"Total");
    let meshing = g.row_index(b"Particles lost by meshing problems").unwrap();
    assert_eq!(meshing, 4);
    assert_eq!(int_row(&g, meshing), [38, 54, 30, 42, 50, 48]);
    assert_eq!(int_row(&g, 6), [100_000; 6]);
    // Each band's rows sum to its total.
    for col in &g.columns[1..] {
        let v = col.ints().unwrap();
        assert_eq!(v[..6].iter().sum::<i32>(), v[6], "{:?}", col.label);
    }
}

#[test]
fn nightmode_tcr_main_results() {
    let g = load("solver-outputs/nightmode-2026-09-08/tcr_main_results.gabe");
    assert_eq!(g.columns.len(), 7);
    assert_eq!(g.columns[0].label, b"TC");
    let rows = g.row_labels().unwrap();
    let rows: Vec<&[u8]> = rows.iter().map(Vec::as_slice).collect();
    assert_eq!(
        rows,
        [
            &b"125 Hz"[..],
            b"250 Hz",
            b"500 Hz",
            b"1000 Hz",
            b"2000 Hz",
            b"4000 Hz",
            b"Global"
        ]
    );

    let tr = g.column(b"TR_Sabine").unwrap();
    assert_eq!(tr.label, b"TR_Sabine\rs");
    assert_eq!(tr.unit(), Some(&b"s"[..]));
    assert!(matches!(
        tr.data,
        ColumnData::Float {
            num_of_digits: 2,
            ..
        }
    ));

    let s = g.band_series(b"TR_Sabine").unwrap();
    assert_eq!(round3(&s.bands), [5.591, 2.698, 1.645, 1.298, 1.252, 1.126]);
    let global = s.global.expect("TR_Sabine has a Global row");
    assert!(
        global.is_nan(),
        "Global TR_Sabine is NaN by design, got {global}"
    );

    // Energetic columns do have a Global value; the unit is UTF-8 in this file.
    let l = g.band_series(b"L_Sabine").unwrap();
    assert_eq!(l.bands.len(), 6);
    assert!(l.global.unwrap().is_finite());
    assert_eq!(
        g.column(b"A_Sabine").unwrap().unit(),
        Some("m\u{b2}".as_bytes())
    );
}

#[test]
fn tutorial1_tables() {
    let stats = load("solver-outputs/tutorial1/spps_stats.gabe");
    assert!(stats.read_only);
    assert_eq!(stats.columns.len(), 28);
    assert_eq!(stats.columns[1].label, b"50 Hz");
    assert_eq!(stats.columns[27].label, b"20000 Hz");
    assert_eq!(int_row(&stats, 6), [10_000; 27]);

    let tcr = load("solver-outputs/tutorial1/tcr_main_results.gabe");
    assert_eq!(tcr.columns.len(), 7);
    assert_eq!(tcr.row_labels().unwrap().len(), 28);
    let s = tcr.band_series(b"TR_Eyring").unwrap();
    assert_eq!(s.bands.len(), 27);
    assert!(s.global.unwrap().is_nan());
}

#[test]
fn tutorial1_receivers() {
    let punctual = load("solver-outputs/tutorial1/spps_punctual.recp");
    let total = load("solver-outputs/tutorial1/spps_total_energy.recp");
    assert!(punctual.read_only);
    assert!(!total.read_only);
    for g in [&punctual, &total] {
        assert_eq!(g.columns.len(), 28);
        assert_eq!(g.columns[0].label, b"SPL");
        assert_eq!(g.row_labels().unwrap().len(), 200);
        for col in &g.columns[1..] {
            match &col.data {
                ColumnData::Float {
                    num_of_digits,
                    values,
                } => {
                    assert_eq!(*num_of_digits, 12);
                    assert_eq!(values.len(), 200);
                }
                other => panic!("expected a float column, got type {}", other.type_code()),
            }
        }
    }
    assert_eq!(punctual.row_labels().unwrap()[0], b"10.0 ms");
    assert_eq!(total.row_labels().unwrap()[4], b"49 ms");
}

#[test]
fn upstream_retrocompat_file() {
    // Written by the old struct-dumping writer: header lengths 20 and 280, and garbage in the
    // padding bytes, all of which upstream ignores. Expected values are upstream's own
    // (src/python_bindings/tests/check_retrocompat_gabe.py).
    let rel = "upstream/python_bindings/retrocompat_test.gabe";
    let b = bytes(rel);
    assert_eq!(&b[4..8], &20i32.to_le_bytes());
    assert_eq!(&b[17..20], &[0x9b, 0x48, 0x00]);
    let g = gabe::read(&b).unwrap();
    assert!(g.read_only);
    assert_eq!(g.columns.len(), 2);
    assert_eq!(g.columns[0].label, b"");
    let rows: Vec<&[u8]> = g.columns[0]
        .strings()
        .unwrap()
        .iter()
        .map(Vec::as_slice)
        .collect();
    assert_eq!(
        rows,
        [
            &b"Particules absorb\xe9es par l'atmosph\xe8re"[..],
            b"Particules absorb\xe9es par les mat\xe9riaux",
            b"Particules perdues d\xfb aux boucles infinies",
            b"Particules perdues d\xfb au maillage incorrect",
            b"Particules restantes",
            b"Total",
        ]
    );
    assert_eq!(g.columns[1].label, b"50 Hz");
    assert_eq!(
        g.columns[1].ints().unwrap(),
        [0, 19_999_966, 0, 2, 32, 20_000_000]
    );
}

#[test]
fn dump_shape() {
    let g = load("solver-outputs/nightmode-2026-09-08/tcr_main_results.gabe");
    let d = gabe::dump(&g);
    let lines: Vec<&str> = d.lines().collect();
    assert_eq!(
        &lines[..5],
        [
            "gabe 2",
            "readonly 1",
            "columns 7",
            "column shortstring TC",
            "rows 7"
        ]
    );
    assert_eq!(lines[5], "125\\x20Hz");
    assert!(d.contains("column float TR_Sabine\\x0ds\ndigits 2\nrows 7\n"));
    assert!(
        d.contains("\n7fc00000\n"),
        "the NaN Global value is dumped by its bits"
    );
    assert!(d.ends_with('\n') && !d.contains(" \n"));
}

#[test]
fn missing_file_is_not_found() {
    let p = fixture("solver-outputs/nightmode-2026-09-08/no_such_table.gabe");
    assert!(matches!(gabe::read_file(&p), Err(FormatError::NotFound(_))));
    assert_eq!(gabe::dump_file(&p), "error notfound\n");
}

#[test]
fn version_3_is_rejected() {
    for rel in FIXTURES {
        let mut b = bytes(rel);
        b[0] = 3;
        assert!(
            matches!(gabe::read(&b), Err(FormatError::Version { .. })),
            "{rel}"
        );
    }
    // Upstream reads any version <= 2 with the v2 layout; only 2 has that layout.
    let mut b = bytes(FIXTURES[0]);
    for v in [1i32, 0, -1, i32::MAX] {
        b[..4].copy_from_slice(&v.to_le_bytes());
        assert!(
            matches!(gabe::read(&b), Err(FormatError::Version { .. })),
            "version {v}"
        );
    }
    // A bare future-version header is a version error, not a truncation.
    assert!(matches!(
        gabe::read(&3i32.to_le_bytes()),
        Err(FormatError::Version { .. })
    ));
}

#[test]
fn every_truncation_is_truncated() {
    // Upstream's Load returns true on most of these, with zero-filled or stale columns.
    for rel in FIXTURES {
        let b = bytes(rel);
        for len in 0..b.len() {
            match gabe::read(&b[..len]) {
                Err(FormatError::Truncated { .. }) => {}
                other => panic!("{rel} cut to {len} of {} bytes: {other:?}", b.len()),
            }
        }
    }
}

#[test]
fn trailing_bytes_are_ignored_as_upstream_ignores_them() {
    let mut b = bytes(FIXTURES[0]);
    let n = b.len();
    let before = gabe::read(&b).unwrap();
    b.extend_from_slice(b"trailing");
    let (after, used) = gabe::read_prefix(&b).unwrap();
    assert_eq!(used, n);
    assert_eq!(after, before);
}

/// A hand-built column: (type, rows, sizeofCol, label, payload after the label's padding byte).
type RawCol<'a> = (u16, i32, i64, &'a [u8], Vec<u8>);

/// Builds a v2 table by hand.
fn table(read_only: u8, cols: &[RawCol]) -> Vec<u8> {
    let mut b = Vec::new();
    for v in [2i32, 0, 0, cols.len() as i32] {
        b.extend_from_slice(&v.to_le_bytes());
    }
    b.extend_from_slice(&[read_only, 0, 0, 0]);
    for (ty, rows, size, label, payload) in cols {
        b.extend_from_slice(&ty.to_le_bytes());
        b.extend_from_slice(&[0, 0]);
        b.extend_from_slice(&rows.to_le_bytes());
        b.extend_from_slice(&size.to_le_bytes());
        b.extend_from_slice(&1i64.to_le_bytes());
        let mut l = label.to_vec();
        l.resize(gabe::LABEL_LEN, 0);
        b.extend_from_slice(&l);
        b.push(0);
        b.extend_from_slice(payload);
    }
    b
}

#[test]
fn edge_cases_upstreams_writer_produces() {
    // No columns: the writer's trailing 3-byte seek is never materialised, leaving 17 bytes.
    let empty = table(0, &[]);
    let g = gabe::read(&empty[..17]).unwrap();
    assert!(g.columns.is_empty());
    assert_eq!(gabe::read_prefix(&empty[..17]).unwrap().1, 17);
    assert!(matches!(
        gabe::read(&empty[..16]),
        Err(FormatError::Truncated { .. })
    ));

    // A last integer column with no rows ends before its two padding bytes.
    let b = table(1, &[(51, 0, 0, b"n", vec![0])]);
    for cut in [0, 1, 2] {
        let g = gabe::read(&b[..b.len() - cut]).unwrap();
        assert_eq!(g.columns[0].data, ColumnData::Int(vec![]));
    }
    assert!(matches!(
        gabe::read(&b[..b.len() - 3]),
        Err(FormatError::Truncated { .. })
    ));

    // An empty float column still carries its 4-byte digits header.
    let b = table(0, &[(50, 0, 0, b"f", 7i32.to_le_bytes().to_vec())]);
    let g = gabe::read(&b).unwrap();
    assert_eq!(
        g.columns[0].data,
        ColumnData::Float {
            num_of_digits: 7,
            values: vec![]
        }
    );
    assert!(matches!(
        gabe::read(&b[..b.len() - 1]),
        Err(FormatError::Truncated { .. })
    ));
}

#[test]
fn unknown_columns_are_skipped_by_size() {
    let ints = [0u8, 5, 0, 0, 0];
    let b = table(
        0,
        &[
            (7, 99, 3, b"user", vec![0xaa, 0xbb, 0xcc]),
            (51, 1, 4, b"i", ints.to_vec()),
        ],
    );
    let g = gabe::read(&b).unwrap();
    assert_eq!(g.columns.len(), 1);
    assert_eq!(g.columns[0].label, b"i");
    assert_eq!(g.columns[0].ints().unwrap(), [5]);
    // A negative size is invalid; one past the end is truncated.
    let neg = table(0, &[(7, 0, -1, b"user", vec![])]);
    assert!(matches!(gabe::read(&neg), Err(FormatError::Invalid(_))));
    let long = table(0, &[(7, 0, 4, b"user", vec![1, 2, 3])]);
    assert!(matches!(
        gabe::read(&long),
        Err(FormatError::Truncated { .. })
    ));
}

#[test]
fn bad_counts_are_rejected_before_reserving() {
    let mut b = table(0, &[(51, 1, 4, b"i", vec![0, 1, 0, 0, 0])]);
    b[12..16].copy_from_slice(&(-1i32).to_le_bytes());
    assert!(matches!(gabe::read(&b), Err(FormatError::Invalid(_))));
    b[12..16].copy_from_slice(&i32::MAX.to_le_bytes());
    assert!(matches!(gabe::read(&b), Err(FormatError::Truncated { .. })));

    for ty in [50u16, 51, 52] {
        let mut b = table(0, &[(ty, 1, 4, b"c", vec![0; 60])]);
        b[24..28].copy_from_slice(&(-3i32).to_le_bytes());
        assert!(
            matches!(gabe::read(&b), Err(FormatError::Invalid(_))),
            "type {ty}"
        );
        b[24..28].copy_from_slice(&i32::MAX.to_le_bytes());
        assert!(
            matches!(gabe::read(&b), Err(FormatError::Truncated { .. })),
            "type {ty}"
        );
    }

    let mut b = table(0, &[]);
    b[16] = 2;
    assert!(matches!(gabe::read(&b), Err(FormatError::Invalid(_))));
}

#[test]
fn strings_and_labels_stop_at_nul_or_their_field() {
    let mut cell = vec![b'x'; 50];
    let mut cell2 = b"ab\0garbage".to_vec();
    cell2.resize(50, b'z');
    let mut payload = vec![0u8];
    payload.append(&mut cell);
    payload.append(&mut cell2);
    let label = [b'L'; 255];
    let b = table(0, &[(52, 2, 100, &label, payload)]);
    let g = gabe::read(&b).unwrap();
    assert_eq!(g.columns[0].label.len(), 255);
    let s = g.columns[0].strings().unwrap();
    assert_eq!(s[0], vec![b'x'; 50]);
    assert_eq!(s[1], b"ab");
}

/// An oracle build that has a `gabe` dumper, if one exists.
fn oracle_exe() -> Option<PathBuf> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/oracle");
    ["gabe", "all"]
        .iter()
        .map(|d| root.join(d).join("oracle.exe"))
        .find(|p| {
            std::process::Command::new(p)
                .arg("list")
                .output()
                .is_ok_and(|o| {
                    String::from_utf8_lossy(&o.stdout)
                        .lines()
                        .any(|l| l.trim() == "gabe")
                })
        })
}

#[test]
fn oracle_agrees_on_every_fixture_when_built() {
    let Some(exe) = oracle_exe() else {
        eprintln!("oracle not built; skipping cross-check");
        return;
    };
    for rel in FIXTURES {
        let path = fixture(rel);
        let out = std::process::Command::new(&exe)
            .arg("dump")
            .arg("gabe")
            .arg(&path)
            .output()
            .unwrap();
        let oracle = String::from_utf8_lossy(&out.stdout).replace("\r\n", "\n");
        assert_eq!(oracle, gabe::dump_file(&path), "{rel}");
    }
}
