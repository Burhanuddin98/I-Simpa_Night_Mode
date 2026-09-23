//! Golden values and negative cases for the `.poly` reader and writer (docs/formats/poly.md).
mod common;

use std::path::{Path, PathBuf};
use std::process::Command;

use simpa_core::formats::FormatError;
use simpa_core::formats::poly::{self, Face, Model, Region};

const IMPORT1: &str = "upstream/lib_interface/test_import1.poly";
const SCENE: &str = "upstream/tutorial1/tetgen/scene_mesh.poly";
/// scene_mesh.poly's user facet list header (`0  1` alone also matches inside `1  0  1`).
const USER_HEAD: &str = "# Part 5 - user facet list\r\n0  1\r\n";

fn face(a: u32, b: u32, c: u32, face_index: u32) -> Face {
    Face {
        vertices: [a, b, c],
        face_index,
    }
}

fn fixture_bytes(rel: &str) -> Vec<u8> {
    std::fs::read(common::fixture(rel)).expect("fixture")
}

/// The first field of the first content line after the comment `marker`: a section's count as
/// it stands in the file, for the sections the model does not keep (holes).
fn section_header(bytes: &[u8], marker: &str) -> String {
    let text = String::from_utf8_lossy(bytes);
    let mut lines = text.lines().skip_while(|l| !l.starts_with(marker)).skip(1);
    let line = lines
        .find(|l| !l.trim().is_empty())
        .expect("section header");
    line.split_whitespace().next().expect("count").to_string()
}

fn assert_bits(v: [f64; 3], want: [f64; 3], what: &str) {
    assert_eq!(
        v.map(f64::to_bits),
        want.map(f64::to_bits),
        "{what}: {v:?} vs {want:?}"
    );
}

/// test_import1.poly's nodes as the file spells them (upstream io_test.cpp writes `0` where the
/// file has `-0`; the reader keeps the sign).
const IMPORT1_VERTICES: [[f64; 3]; 19] = [
    [5.5, -0.0, 0.0],
    [0.0, -0.0, 0.0],
    [0.0, 8.1000004, 0.0],
    [5.5, 8.1000004, 0.0],
    [0.0, 8.1000004, 3.2],
    [5.5, 8.1000004, 3.2],
    [0.0, -0.0, 3.2],
    [5.5, -0.0, 3.2],
    [3.7532663, 2.4895077, 0.0],
    [1.7356972, 2.4895077, 0.0],
    [1.7356972, 5.795608, 0.0],
    [3.7532663, 5.795608, 0.0],
    [1.7356972, 5.795608, 1.1],
    [3.7532663, 5.795608, 1.1],
    [1.7356972, 2.4895077, 1.1],
    [3.7532663, 2.4895077, 1.1],
    [3.7532661, 2.5724626, 0.0],
    [1.7356973, 5.5437918, 0.0],
    [1.7356972, 5.5054293, 0.0],
];

/// Upstream io_test.cpp `read_poly_test1`: the 39 faces, `t_face(a, b, c, faceIndex)`.
const IMPORT1_FACES: [[u32; 4]; 39] = [
    [0, 1, 9, 0],
    [0, 16, 3, 1],
    [2, 4, 5, 2],
    [2, 5, 3, 3],
    [2, 6, 4, 4],
    [2, 1, 6, 5],
    [1, 0, 7, 6],
    [6, 1, 7, 7],
    [0, 3, 5, 8],
    [7, 0, 5, 9],
    [7, 5, 4, 10],
    [6, 7, 4, 11],
    [10, 12, 13, 14],
    [10, 13, 11, 15],
    [10, 14, 12, 16],
    [10, 17, 14, 17],
    [9, 8, 15, 18],
    [14, 9, 15, 19],
    [8, 16, 13, 20],
    [15, 8, 13, 21],
    [15, 13, 12, 22],
    [14, 15, 12, 23],
    [0, 9, 8, 0],
    [9, 1, 2, 0],
    [16, 17, 10, 1],
    [16, 10, 11, 1],
    [10, 2, 3, 1],
    [16, 11, 3, 1],
    [11, 10, 3, 1],
    [17, 18, 14, 17],
    [0, 8, 16, 0],
    [8, 9, 18, 0],
    [16, 11, 13, 20],
    [16, 8, 17, 0],
    [17, 2, 10, 1],
    [17, 8, 18, 0],
    [18, 9, 14, 17],
    [18, 9, 2, 0],
    [18, 2, 17, 0],
];

fn import1_faces() -> Vec<Face> {
    IMPORT1_FACES
        .iter()
        .map(|&[a, b, c, i]| face(a, b, c, i))
        .collect()
}

#[test]
fn test_import1_parses_as_upstream_io_test_expects() {
    // test_import1.poly: 19 nodes, 39 facets (all triangles), 0 holes, 1 region, 0 user facets.
    let bytes = fixture_bytes(IMPORT1);
    let m = poly::read(&bytes).expect("test_import1.poly");
    assert_eq!(m.model_vertices.len(), 19, "nodes");
    assert_eq!(m.model_faces.len(), 39, "facets");
    assert_eq!(section_header(&bytes, "# Part 3"), "0", "holes");
    assert_eq!(m.model_regions.len(), 1, "regions");
    assert_eq!(m.user_defined_faces.len(), 0, "user facets");
    assert!(m.save_face_index, "facet list header is '39  1'");

    for (i, (&v, &want)) in m
        .model_vertices
        .iter()
        .zip(IMPORT1_VERTICES.iter())
        .enumerate()
    {
        assert_bits(v, want, &format!("node {}", i + 1));
    }
    assert_eq!(m.model_faces, import1_faces());
    assert_eq!(
        m.model_regions[0],
        Region {
            region_index: 3119,
            dot_in_region: [3.7530646, 5.7952776, 1.09989],
            region_refinement: -1.0,
        }
    );
}

/// scene_mesh.poly through upstream's ImportPOLY (the oracle printed exactly this).
const SCENE_DUMP: &str = "poly
save_face_index 1
user_faces 0
faces 12
0 1 2 0
0 2 3 1
2 4 5 2
2 5 3 3
2 6 4 4
2 1 6 5
1 0 7 6
6 1 7 7
0 3 5 8
7 0 5 9
7 5 4 10
6 7 4 11
vertices 8
4018000000000000 8000000000000000 0000000000000000
0000000000000000 8000000000000000 0000000000000000
0000000000000000 4024000000000000 0000000000000000
4018000000000000 4024000000000000 0000000000000000
0000000000000000 4024000000000000 4008000000000000
4018000000000000 4024000000000000 4008000000000000
0000000000000000 8000000000000000 4008000000000000
4018000000000000 8000000000000000 4008000000000000
regions 0
";

#[test]
fn scene_mesh_parses_and_dumps() {
    // scene_mesh.poly: 8 nodes, 12 facets (all triangles), 0 holes, 0 regions, 0 user facets.
    let bytes = fixture_bytes(SCENE);
    let m = poly::read(&bytes).expect("scene_mesh.poly");
    assert_eq!(m.model_vertices.len(), 8, "nodes");
    assert_eq!(m.model_faces.len(), 12, "facets");
    assert_eq!(section_header(&bytes, "# Part 3"), "0", "holes");
    assert_eq!(m.model_regions.len(), 0, "regions");
    assert_eq!(m.user_defined_faces.len(), 0, "user facets");
    assert!(m.save_face_index);
    assert_bits(m.model_vertices[0], [6.0, -0.0, 0.0], "node 1 keeps -0");
    assert_eq!(m.model_faces[11], face(6, 7, 4, 11));
    assert_eq!(poly::dump(&m), SCENE_DUMP);
    assert_eq!(poly::dump_file(&common::fixture(SCENE)), SCENE_DUMP);
}

#[test]
fn writer_reproduces_upstream_export_byte_for_byte() {
    // scene_mesh.poly was written by upstream's GUI through ExportPOLY on Windows.
    let bytes = fixture_bytes(SCENE);
    let m = poly::read(&bytes).unwrap();
    let out = poly::write(&m);
    assert_eq!(
        String::from_utf8_lossy(&out),
        String::from_utf8_lossy(&bytes)
    );
    assert_eq!(out, bytes);
}

#[test]
fn test_import1_rewrites_to_the_same_model() {
    // test_import1.poly comes from an older writer (double spaces), so only the model must match.
    let m = poly::read(&fixture_bytes(IMPORT1)).unwrap();
    let again = poly::read(&poly::write(&m)).unwrap();
    assert_eq!(poly::dump(&again), poly::dump(&m));
}

#[test]
fn upstream_write_read_poly_test1_round_trips() {
    // Port of io_test.cpp write_read_poly_test1: the model built in code, written, read back.
    let mut vertices = IMPORT1_VERTICES;
    for v in &mut vertices {
        *v = v.map(|c| if c == 0.0 { 0.0 } else { c }); // C++ `-0` there is the int 0
    }
    let model = Model {
        save_face_index: false,
        user_defined_faces: Vec::new(),
        model_faces: import1_faces(),
        model_vertices: vertices.to_vec(),
        model_regions: vec![Region {
            region_index: 3119,
            dot_in_region: [3.7530646, 5.7952776, 1.09989],
            region_refinement: -1.0,
        }],
    };
    let back = poly::read(&poly::write(&model)).unwrap();
    assert_eq!(back, model);
    assert_eq!(poly::dump(&back), poly::dump(&model));
}

/// `ostream << double` at precision 17 in the classic locale, as MSVC 19.4x / UCRT print it
/// (receipts from a probe built with the oracle's compiler, 2026-09-23; same expressions).
fn msvc_doubles() -> Vec<(f64, &'static str)> {
    vec![
        (2f64.powi(-25), "2.9802322387695312e-08"), // an exact tie at 17 digits, rounded to even
        (1e15 + 0.25, "1000000000000000.2"),        // tie
        (1e15 + 0.75, "1000000000000000.8"),        // tie
        (0.5, "0.5"),
        (2.5, "2.5"),
        (1e-5, "1.0000000000000001e-05"),
        (1e20, "1e+20"),
        (1e100, "1e+100"),
        (123456789012345678.0, "1.2345678901234568e+17"),
        (-0.0, "-0"),
        (1e-300, "1e-300"),
        (5e-324, "4.9406564584124654e-324"),
        (0.1, "0.10000000000000001"),
        (1.0 / 3.0, "0.33333333333333331"),
        (1e16, "10000000000000000"),
        (1e17, "1e+17"),
        (0.0001, "0.0001"),
        (0.00001234, "1.234e-05"),
        (12345678901234567.0, "12345678901234568"),
        (3.7530646324157715, "3.7530646324157715"),
        (-1.0, "-1"),
        (3.0 * 2f64.powi(-26), "4.4703483581542969e-08"),
        (5.0 * 2f64.powi(-25), "1.4901161193847656e-07"),
        (f64::from_bits(0x7ff0_0000_0000_0000), "inf"),
        (f64::from_bits(0xfff0_0000_0000_0000), "-inf"),
        (f64::from_bits(0x7ff8_0000_0000_0000), "nan"),
        (f64::from_bits(0xfff8_0000_0000_0000), "-nan(ind)"),
        (f64::from_bits(0x7ff0_0000_0000_0001), "nan(snan)"),
        (f64::from_bits(0xfff8_0000_0000_0001), "-nan"),
        (f64::from_bits(0x7ff8_0000_0000_0123), "nan"),
    ]
}

/// `ostream << float` (promoted to double) at precision 17, same probe.
const MSVC_FLOATS: &[(f32, &str)] = &[
    (0.1, "0.10000000149011612"),
    (3.7530646, "3.7530646324157715"),
    (-1.0, "-1"),
    (1e-8, "9.9999999392252903e-09"),
    (1e30, "1.0000000150474662e+30"),
];

fn node_line(v: f64) -> String {
    let m = Model {
        model_vertices: vec![[v, 0.0, 0.0]],
        ..Model::default()
    };
    let text = String::from_utf8(poly::write(&m)).unwrap();
    text.lines().nth(2).unwrap().to_string()
}

#[test]
fn float_spelling_matches_msvc() {
    for (v, want) in msvc_doubles() {
        assert_eq!(
            node_line(v),
            format!("1 {want} 0 0"),
            "bits {:016x}",
            v.to_bits()
        );
    }
    for &(v, want) in MSVC_FLOATS {
        let m = Model {
            model_regions: vec![Region {
                region_index: 7,
                dot_in_region: [v, 0.0, -0.0],
                region_refinement: v,
            }],
            ..Model::default()
        };
        let text = String::from_utf8(poly::write(&m)).unwrap();
        let line = text.lines().find(|l| l.starts_with("1  ")).unwrap();
        assert_eq!(line, format!("1  {want}  0  -0  7  {want}"), "float {v:e}");
    }
}

#[test]
fn quadrilateral_becomes_two_triangles() {
    let text = "4 3 0 0\n1 0 0 0\n2 1 0 0\n3 1 1 0\n4 0 1 0\n1 0\n1 0 5\n4 1 2 3 4\n0\n";
    let m = poly::read(text.as_bytes()).unwrap();
    // poly.cpp ImportPOLY: (a,b,c) then (c,d,a), same marker.
    assert_eq!(m.model_faces, vec![face(0, 1, 2, 5), face(2, 3, 0, 5)]);
    assert!(!m.save_face_index);
    assert!(m.model_regions.is_empty() && m.user_defined_faces.is_empty());
}

#[test]
fn user_facets_keep_their_own_markers() {
    // Upstream's ImportPOLY gives every user facet after the first the first one's marker
    // (poly.cpp compares against the main facet count); this reader does not.
    let text = "3 3 0 0\n1 0 0 0\n2 1 0 0\n3 0 1 0\n0 1\n0\n0\n2  1\n1  0  7\n3  1  2  3\n\
                1  0  9\n3  3  2  1\n";
    let m = poly::read(text.as_bytes()).unwrap();
    assert_eq!(
        m.user_defined_faces,
        vec![face(0, 1, 2, 7), face(2, 1, 0, 9)]
    );
    // The oracle (upstream ImportPOLY) printed "0 1 2 7" / "2 1 0 7" for this file, 2026-09-23.
    assert_eq!(
        poly::upstream_import_view(&m).user_defined_faces,
        vec![face(0, 1, 2, 7), face(2, 1, 0, 7)]
    );
    let mut view = m.clone();
    view.user_defined_faces.clear();
    assert_eq!(
        poly::upstream_import_view(&view),
        view,
        "no user facets, no change"
    );
}

#[test]
fn layout_variations_read_the_same_model() {
    let crlf = fixture_bytes(SCENE);
    let want = poly::dump(&poly::read(&crlf).unwrap());
    let lf: Vec<u8> = crlf.iter().copied().filter(|&b| b != b'\r').collect();
    assert_eq!(poly::dump(&poly::read(&lf).unwrap()), want, "LF line ends");
    let text = String::from_utf8(lf).unwrap();
    let spaced = text
        .replace(' ', " \t ")
        .replace('\n', "\n\n# comment\n   \n");
    assert_eq!(
        poly::dump(&poly::read(spaced.as_bytes()).unwrap()),
        want,
        "tabs, blanks, comments"
    );
    let no_final_newline = text.trim_end();
    assert_eq!(
        poly::dump(&poly::read(no_final_newline.as_bytes()).unwrap()),
        want
    );
    let holes = text.replace(
        "# Part 3 - hole list\n0\n",
        "# Part 3 - hole list\n2\n1 1 1 1\n2 +2 .5 5.\n",
    );
    assert_eq!(
        poly::dump(&poly::read(holes.as_bytes()).unwrap()),
        want,
        "holes are dropped"
    );
    let short = &text[..text.find("# Part 4").unwrap()];
    let m = poly::read(short.as_bytes()).expect("region and user facet lists are optional");
    assert!(m.model_regions.is_empty() && m.user_defined_faces.is_empty());
    assert_eq!(m.model_faces.len(), 12);
}

#[test]
fn number_spellings_cpp_accepts() {
    let text = "3 3 0 0\n1 +5 5. .5\n2 1.e0 1E0 -1e-310\n+3 00012 0.0 -.25e+1\n+2 +0\n\
                1 0 +4294967295\n3 +1 2 3\n1 0 0\n3 3 2 1\n0\n1\n1 2e-45 -0 1 -2147483648 +1.5\n";
    let m = poly::read(text.as_bytes()).unwrap();
    assert_bits(
        m.model_vertices[0],
        [5.0, 5.0, 0.5],
        "signs and bare points",
    );
    assert_bits(
        m.model_vertices[1],
        [1.0, 1.0, -1e-310],
        "exponents and a subnormal",
    );
    assert_bits(m.model_vertices[2], [12.0, 0.0, -2.5], "leading zeros");
    assert_eq!(
        m.model_faces,
        vec![face(0, 1, 2, u32::MAX), face(2, 1, 0, 0)]
    );
    let r = m.model_regions[0];
    assert_eq!(
        r.dot_in_region.map(f32::to_bits),
        [1, 0x8000_0000, 1f32.to_bits()]
    );
    assert_eq!((r.region_index, r.region_refinement), (i32::MIN, 1.5));
}

fn read_err(bytes: &[u8]) -> FormatError {
    match poly::read(bytes) {
        Ok(m) => panic!("expected an error, read {m:?}"),
        Err(e) => e,
    }
}

/// scene_mesh.poly with the first `from` replaced by `to`.
fn scene_with(from: &str, to: &str) -> Vec<u8> {
    let text = String::from_utf8(fixture_bytes(SCENE)).unwrap();
    assert!(text.contains(from), "{from:?} not in scene_mesh.poly");
    text.replacen(from, to, 1).into_bytes()
}

#[test]
fn malformed_numbers_are_invalid() {
    let cases: &[(&str, &str)] = &[
        ("1 6 -0 0", "1 6x -0 0"),
        ("1 6 -0 0", "1 6 -0 0.0.0"),
        ("1 6 -0 0", "1 6 nan 0"),
        ("1 6 -0 0", "1 6 inf 0"),
        ("1 6 -0 0", "1 0x6 -0 0"),   // MSVC's >> double would read 6
        ("1 6 -0 0", "1 6 1e999 0"),  // overflows
        ("1 6 -0 0", "1 6 1e-999 0"), // underflows to zero
        ("1 6 -0 0", "1 6 - 0"),
        ("1 6 -0 0", "1 6 1e 0"),
        ("1 6 -0 0", "1 6 . 0"),
        ("1 6 -0 0", "1 6 \u{e9} 0"),
        ("1 6 -0 0", "1.0 6 -0 0"),
        ("3  1 2 3", "3  1 2 3.5"),
        ("1  0  0\r\n3  1 2 3", "1  0  -1\r\n3  1 2 3"), // MSVC's >> unsigned would wrap
        ("1  0  0\r\n3  1 2 3", "1  0  4294967296\r\n3  1 2 3"),
        ("12 1", "12 x"),
        ("12 1", "12 -1"),
        ("8  3  0  0", "8  3.0  0  0"),
        ("8  3  0  0", "99999999999  3  0  0"),
    ];
    for &(from, to) in cases {
        let e = read_err(&scene_with(from, to));
        assert!(matches!(e, FormatError::Invalid(_)), "{to:?}: {e}");
    }
}

#[test]
fn malformed_structure_is_invalid() {
    let cases: &[(&str, &str)] = &[
        ("3  1 2 3", "3  1 2 9"),   // no node 9
        ("3  1 2 3", "3  1 2 0"),   // nodes are 1-based
        ("3  1 2 3", "3  1 2"),     // too few corners
        ("3  1 2 3", "3  1 2 3 4"), // too many corners
        ("3  1 3 4", "5  1 3 4 5 6"),
        ("1  0  1\r\n", "2  0  1\r\n"), // two polygons in one facet
        ("1  0  1\r\n", "1  1  1\r\n"), // a facet hole
        ("1  0  1\r\n", "1  0\r\n"),
        ("12 1", "12 2"),
        ("12 1", "12"),
        ("8  3  0  0", "8  2  0  0"),
        ("8  3  0  0", "8  3  1  0"),
        ("8  3  0  0", "8  3  0  1"),
        ("8  3  0  0", "8  3"),
        ("8  3  0  0", "8  3  0  0 # nodes"), // ImportPOLY would drop this header whole
        ("2 0 -0 0", "3 0 -0 0"),             // node numbering must count from 1
        ("2 0 -0 0", "2 0 -0 0 1"),
        (
            "# Part 4 - region list\r\n0",
            "# Part 4 - region list\r\n-1",
        ),
        (
            "# Part 4 - region list\r\n0",
            "# Part 4 - region list\r\n1\r\n2 0 0 0 1 -1",
        ),
        (
            "# Part 4 - region list\r\n0",
            "# Part 4 - region list\r\n1\r\n1 0 0 0 1.5 -1",
        ),
        (
            "# Part 4 - region list\r\n0",
            "# Part 4 - region list\r\n1\r\n1 0 0 0 1",
        ),
        (USER_HEAD, "# Part 5 - user facet list\r\n0  2\r\n"),
        (
            USER_HEAD,
            "# Part 5 - user facet list\r\n0  1\r\nleftover\r\n",
        ),
    ];
    for &(from, to) in cases {
        let e = read_err(&scene_with(from, to));
        assert!(matches!(e, FormatError::Invalid(_)), "{to:?}: {e}");
    }
}

#[test]
fn truncated_files_are_truncated() {
    let bytes = fixture_bytes(SCENE);
    let text = String::from_utf8(bytes.clone()).unwrap();
    for cut in [
        "2 0 -0 0",
        "# Part 2",
        "1  0  5",
        "3  3 2 7",
        "# Part 3",
        "0\r\n\r\n# Part 4",
    ] {
        let at = text.find(cut).unwrap();
        let e = read_err(&bytes[..at]);
        assert!(
            matches!(e, FormatError::Truncated { .. }),
            "cut before {cut:?}: {e}"
        );
    }
    for short in ["", "\n\n", "# only a comment\n", "8  3  0  0\n"] {
        let e = read_err(short.as_bytes());
        assert!(matches!(e, FormatError::Truncated { .. }), "{short:?}: {e}");
    }
    let regions = scene_with(
        "# Part 4 - region list\r\n0",
        "# Part 4 - region list\r\n2\r\n1 0 0 0 1 -1",
    );
    let at = regions.len() - "\r\n# Part 5 - user facet list\r\n0  1\r\n".len();
    assert!(
        matches!(read_err(&regions[..at]), FormatError::Truncated { .. }),
        "region list cut"
    );
    let users = scene_with(
        USER_HEAD,
        "# Part 5 - user facet list\r\n1  1\r\n1  0  3\r\n",
    );
    assert!(
        matches!(read_err(&users), FormatError::Truncated { .. }),
        "user facet cut"
    );
}

fn tmp(name: &str) -> PathBuf {
    Path::new(env!("CARGO_TARGET_TMPDIR")).join(name)
}

#[test]
fn missing_file_is_not_found() {
    let path = tmp("poly_golden_no_such_file.poly");
    let _ = std::fs::remove_file(&path);
    assert!(matches!(
        poly::read_file(&path),
        Err(FormatError::NotFound(_))
    ));
    assert_eq!(poly::dump_file(&path), "error notfound\n");
}

#[test]
fn dump_file_reports_error_kinds() {
    let path = tmp("poly_golden_malformed.poly");
    std::fs::write(&path, scene_with("1 6 -0 0", "1 6x -0 0")).unwrap();
    assert_eq!(poly::dump_file(&path), "error invalid\n");
    std::fs::write(&path, b"8  3  0  0\r\n").unwrap();
    assert_eq!(poly::dump_file(&path), "error truncated\n");
}

#[test]
fn generated_models_round_trip() {
    let mut dumps = std::collections::HashSet::new();
    for seed in 0..200u64 {
        let m = poly::generate(seed);
        assert_eq!(
            poly::dump(&poly::generate(seed)),
            poly::dump(&m),
            "seed {seed} is deterministic"
        );
        let bytes = poly::write(&m);
        let back = poly::read(&bytes).unwrap_or_else(|e| panic!("seed {seed}: {e}"));
        assert_eq!(poly::dump(&back), poly::dump(&m), "seed {seed}");
        assert_eq!(
            poly::write(&back),
            bytes,
            "seed {seed} rewrites identically"
        );
        assert!(!m.model_vertices.is_empty() && !m.model_faces.is_empty());
        dumps.insert(poly::dump(&m));
    }
    assert_eq!(dumps.len(), 200, "every seed gives a different model");
}

#[test]
fn write_file_then_read_file() {
    let path = tmp("poly_golden_written.poly");
    let m = poly::generate(7);
    poly::write_file(&m, &path).unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), poly::write(&m));
    assert_eq!(poly::dump_file(&path), poly::dump(&m));
}

// ---------------------------------------------------------------------------------------------
// The spellings refused because a reader upstream would see something else (docs/formats/poly.md,
// "Lines" and "Numbers").

#[test]
fn ctrl_z_is_invalid_anywhere() {
    // Text-mode reads on Windows (ImportPOLY's ifstream, TetGen's fopen "r") end the file at
    // Ctrl-Z; the oracle read nothing of scene_mesh.poly with one in its first comment.
    let cases: &[(&str, &str)] = &[
        ("# Part 1 - node list", "# Part 1 - node list\u{1a}"),
        ("# Part 1 - node list", "#\u{1a}Part 1 - node list"),
        ("# Part 4 - region list", "# Part 4 - regi\u{1a}on list"),
        ("1 6 -0 0", "1 6 -0 0\u{1a}"),
        ("1 6 -0 0", "1 6 \u{1a} 0"),
        (USER_HEAD, "# Part 5 - user facet list\r\n0  1\r\n\u{1a}"), // a DOS end-of-file mark
    ];
    for &(from, to) in cases {
        let e = read_err(&scene_with(from, to));
        assert!(matches!(e, FormatError::Invalid(_)), "{to:?}: {e}");
    }
}

#[test]
fn separators_other_than_space_and_tab_are_invalid() {
    // ImportPOLY's >> skips every isspace byte; TetGen's findnextnumber (tetgen.cxx:2932) ends
    // a field only at space, tab, comma or '#', so it would read "1\v6" as the single field 1.
    let cases: &[(&str, &str)] = &[
        ("1 6 -0 0", "1\u{b}6 -0 0"),
        ("1 6 -0 0", "1 6\u{c}-0 0"),
        ("1 6 -0 0", "1 6\r-0 0"),
        ("1 6 -0 0\r\n", "1 6 -0 0\r\r\n"),
        ("\r\n\r\n# Part 2", "\r\n\u{c}\r\n# Part 2"), // a "blank" line of a form feed
        ("1 6 -0 0", "1,6,-0,0"),                      // TetGen's separator, not ImportPOLY's
    ];
    for &(from, to) in cases {
        let e = read_err(&scene_with(from, to));
        assert!(matches!(e, FormatError::Invalid(_)), "{to:?}: {e}");
    }
}

#[test]
fn integers_with_leading_zeros_are_invalid() {
    // ImportPOLY reads them as decimal; TetGen's strtol(.., 0) reads 010 as octal 8, 08 as 0.
    let cases: &[(&str, &str)] = &[
        ("3  1 2 3", "3  1 2 03"),
        ("3  1 2 3", "3  1 2 010"),
        ("3  1 2 3", "3  1 2 08"),
        ("3  1 2 3", "03  1 2 3"),
        ("2 0 -0 0", "02 0 -0 0"),
        ("1  0  1\r\n", "1  0  01\r\n"),
        ("1  0  1\r\n", "01  0  1\r\n"),
        ("1  0  1\r\n", "1  00  1\r\n"),
        ("8  3  0  0", "08  3  0  0"),
        ("8  3  0  0", "8  03  0  0"),
        ("12 1", "012 1"),
        ("12 1", "12 +01"),
        ("# Part 3 - hole list\r\n0", "# Part 3 - hole list\r\n00"),
        (
            "# Part 4 - region list\r\n0",
            "# Part 4 - region list\r\n01\r\n1 0 0 0 7 -1",
        ),
        (
            "# Part 4 - region list\r\n0",
            "# Part 4 - region list\r\n1\r\n1 0 0 0 -010 -1",
        ),
        (USER_HEAD, "# Part 5 - user facet list\r\n00  1\r\n"),
    ];
    for &(from, to) in cases {
        let e = read_err(&scene_with(from, to));
        assert!(matches!(e, FormatError::Invalid(_)), "{to:?}: {e}");
    }
    // A zero alone, signed or not, is fine.
    let m = poly::read(&scene_with("12 1", "+12 +1")).unwrap();
    assert!(m.save_face_index);
    poly::read(&scene_with("1  0  0\r\n", "+1  -0  +0\r\n")).unwrap();
}

#[test]
fn lines_longer_than_tetgen_reads_are_invalid() {
    // TetGen's fgets reads 2047 bytes of a line (INPUTLINESIZE 2048) and the rest as a new line.
    let node = "1 6 -0 0";
    let fits = format!("{node}{}", " ".repeat(2047 - node.len()));
    let long = format!("{fits} ");
    let m = poly::read(&scene_with(node, &fits)).expect("a 2047-byte line");
    assert_eq!(m.model_vertices.len(), 8);
    let e = read_err(&scene_with(node, &long));
    assert!(matches!(e, FormatError::Invalid(_)), "2048-byte line: {e}");

    let comment = "# Part 2 - facet list";
    let fits = format!("{comment}{}", "9".repeat(2047 - comment.len()));
    poly::read(&scene_with(comment, &fits)).expect("a 2047-byte comment");
    let e = read_err(&scene_with(comment, &format!("{fits}9")));
    assert!(
        matches!(e, FormatError::Invalid(_)),
        "2048-byte comment: {e}"
    );
}

#[test]
fn comment_lines_are_the_ones_both_readers_skip() {
    // A line holding '#' is a comment when nothing before the first '#' can start a number:
    // ImportPOLY skips every line with '#', TetGen skips these. A UTF-8 byte order mark before
    // the first comment is such a prefix.
    let want = poly::dump(&poly::read(&fixture_bytes(SCENE)).unwrap());
    for (from, to) in [
        ("# Part 1 - node list", "\u{feff}# Part 1 - node list"),
        ("# Part 2 - facet list", "  * see below # Part 2"),
        ("# Part 2 - facet list", "\u{b}\u{c}\r#"),
        ("# Part 3 - hole list", "#\u{0} 1 2 3 #"),
    ] {
        let m = poly::read(&scene_with(from, to)).unwrap_or_else(|e| panic!("{to:?}: {e}"));
        assert_eq!(poly::dump(&m), want, "{to:?}");
    }
    // A number before the '#': TetGen reads it, ImportPOLY drops the line.
    for (from, to) in [
        ("8  3  0  0", "\u{feff}8  3  0  0"), // ImportPOLY cannot read the header, TetGen can
        ("# Part 2 - facet list", "Part 2 # facet list"),
        ("# Part 2 - facet list", "x-y # facet list"),
        ("# Part 2 - facet list", ". # facet list"),
    ] {
        let e = read_err(&scene_with(from, to));
        assert!(matches!(e, FormatError::Invalid(_)), "{to:?}: {e}");
    }
}

// ---------------------------------------------------------------------------------------------
// Upstream's own reader (`oracle/dump_poly.cpp` over `CPoly::ImportPOLY`).

/// The oracle, built into `target/oracle/poly` on demand by `common/paths.rs` when it is missing
/// or older than its sources. A failed build fails every test that needs it, so these checks
/// never pass without running.
fn oracle() -> PathBuf {
    common::paths::oracle("poly")
}

/// The oracle's dump of `path`, line ends normalised.
fn oracle_dump(path: &Path) -> String {
    let out = Command::new(oracle())
        .args(["dump", "poly"])
        .arg(path)
        .output()
        .expect("run the oracle");
    String::from_utf8_lossy(&out.stdout).replace("\r\n", "\n")
}

#[test]
fn oracle_agrees_on_fixtures() {
    for rel in [IMPORT1, SCENE] {
        let path = common::fixture(rel);
        assert_eq!(poly::dump_file(&path), oracle_dump(&path), "{rel}");
    }
}

#[test]
fn oracle_reads_generated_files_as_modelled() {
    // Upstream's ImportPOLY must read what we write as `upstream_import_view` says: the model,
    // with the user-facet marker bug and nothing else. Also counts how often the bug shows.
    let mut bug_visible = 0;
    for seed in 0..200u64 {
        let m = poly::generate(seed);
        let path = tmp(&format!("poly_golden_gen_{seed}.poly"));
        poly::write_file(&m, &path).unwrap();
        let theirs = oracle_dump(&path);
        assert_eq!(
            poly::dump(&poly::upstream_import_view(&m)),
            theirs,
            "seed {seed}"
        );
        if theirs != poly::dump(&m) {
            bug_visible += 1;
        }
    }
    eprintln!("upstream user-facet marker bug visible in {bug_visible} of 200 seeds");
    assert!(bug_visible > 0, "generate() exercises the user-facet bug");
}

#[test]
fn oracle_agrees_on_every_accepted_variant() {
    // Hand-made files at the edges of what the reader accepts. Each must be accepted, and
    // upstream must read it as `upstream_import_view` of our model says.
    let lf = String::from_utf8(fixture_bytes(SCENE))
        .unwrap()
        .replace("\r\n", "\n");
    let fits = format!("1 6 -0 0{}", " ".repeat(2047 - 8));
    let long_comment = format!("# Part 2 - facet list{}", "7".repeat(2047 - 21));
    let variants: Vec<(&str, Vec<u8>)> = vec![
        ("lf", lf.clone().into_bytes()),
        (
            "bom then comment",
            scene_with("# Part 1", "\u{feff}# Part 1"),
        ),
        (
            "junk before '#'",
            scene_with("# Part 2 - facet list", "  * see # 1 2 3"),
        ),
        (
            "control bytes before '#'",
            scene_with("# Part 2 - facet list", "\u{b}\u{c}\r\u{0}# 5"),
        ),
        (
            "no final newline, lone cr",
            format!("{}\r", lf.trim_end()).into_bytes(),
        ),
        (
            "tabs",
            lf.replace("  ", "\t").replace(' ', " \t ").into_bytes(),
        ),
        ("2047-byte line", scene_with("1 6 -0 0", &fits)),
        (
            "2047-byte comment",
            scene_with("# Part 2 - facet list", &long_comment),
        ),
        (
            "signs, zeros, real spellings",
            "4 3 0 0\n+1 +5 5. .5\n2 1.e0 1E0 -1e-310\n+3 00012 0.0 -.25e+1\n4 -0 +0 0e5\n\
             +2 +1\n1 -0 +4294967295\n3 +1 2 3\n1 0 0\n4 4 3 2 1\n+1\n1 0 0 0\n\
             1\n1 2e-45 -0 1 -2147483648 +1.5\n"
                .as_bytes()
                .to_vec(),
        ),
        (
            "holes, regions, quads and user facets",
            "4 3 0 0\n1 0 0 0\n2 1 0 0\n3 1 1 0\n4 0 1 0\n2 0\n1 0 5\n4 1 2 3 4\n1 0 6\n\
             3 4 3 2\n2\n1 0.5 0.5 0.5\n2 1 1 1\n2\n1 0.25 0.25 0.25 3119 -1\n\
             2 0.1 0.2 0.3 -7 0.001\n3 1\n1 0 7\n3 1 2 3\n1 0 9\n4 1 2 3 4\n1 0 11\n3 3 2 1\n"
                .as_bytes()
                .to_vec(),
        ),
        (
            "no region or user facet list",
            lf.as_bytes()[..lf.find("# Part 4").unwrap()].to_vec(),
        ),
    ];
    for (name, bytes) in variants {
        let m = poly::read(&bytes).unwrap_or_else(|e| panic!("{name}: {e}"));
        let path = tmp(&format!(
            "poly_golden_variant_{}.poly",
            name.replace(' ', "_")
        ));
        std::fs::write(&path, &bytes).unwrap();
        assert_eq!(
            poly::dump(&poly::upstream_import_view(&m)),
            oracle_dump(&path),
            "{name}"
        );
    }
}

#[test]
fn oracle_reads_a_ctrl_z_comment_as_the_end_of_the_file() {
    // Why Ctrl-Z is refused: with one in scene_mesh.poly's first comment, upstream keeps nothing.
    let path = tmp("poly_golden_ctrl_z.poly");
    std::fs::write(
        &path,
        scene_with("# Part 1 - node list", "# Part 1 - node list\u{1a}"),
    )
    .unwrap();
    assert_eq!(
        oracle_dump(&path),
        poly::dump(&Model::default()),
        "ImportPOLY stops at Ctrl-Z"
    );
    assert!(matches!(
        poly::read_file(&path),
        Err(FormatError::Invalid(_))
    ));
}
