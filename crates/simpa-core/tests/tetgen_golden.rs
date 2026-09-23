mod common;

use std::path::{Path, PathBuf};

use simpa_core::formats::FormatError;
use simpa_core::formats::tetgen::{
    self, EleFile, FaceFile, HULL, Kind, NeighFile, NodeFile, TetgenFile,
};

fn tutorial(ext: &str) -> PathBuf {
    common::fixture(&format!(
        "solver-outputs/tutorial1/tetgen_scene_mesh.1.{ext}"
    ))
}

fn skipped(ext: &str) -> PathBuf {
    common::fixture(&format!(
        "solver-outputs/nightmode-2026-09-08/tcr_model_skipped.{ext}"
    ))
}

/// Every TetGen fixture under tests/fixtures.
fn fixtures() -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = ["node", "ele", "face", "neigh"]
        .iter()
        .map(|e| tutorial(e))
        .collect();
    v.push(skipped("face"));
    v.push(skipped("node"));
    v
}

fn node(p: &Path) -> NodeFile {
    match tetgen::read_file(p).unwrap() {
        TetgenFile::Node(n) => n,
        other => panic!("{} read as {:?}", p.display(), other.kind()),
    }
}

fn ele(p: &Path) -> EleFile {
    match tetgen::read_file(p).unwrap() {
        TetgenFile::Ele(e) => e,
        other => panic!("{} read as {:?}", p.display(), other.kind()),
    }
}

fn face(p: &Path) -> FaceFile {
    match tetgen::read_file(p).unwrap() {
        TetgenFile::Face(f) => f,
        other => panic!("{} read as {:?}", p.display(), other.kind()),
    }
}

fn neigh(p: &Path) -> NeighFile {
    match tetgen::read_file(p).unwrap() {
        TetgenFile::Neigh(n) => n,
        other => panic!("{} read as {:?}", p.display(), other.kind()),
    }
}

// Canonical dumps of tutorial 1's mesh, as the oracle (TetGen's own loaders) prints them.
const TUTORIAL_NODE: &str = "tetgen node\nfirst 1\ndim 3\nattributes 0\nmarkers 0\npoints 8\n\
4018000000000000 8000000000000000 0000000000000000\n\
0000000000000000 8000000000000000 0000000000000000\n\
0000000000000000 4024000000000000 0000000000000000\n\
4018000000000000 4024000000000000 0000000000000000\n\
0000000000000000 4024000000000000 4008000000000000\n\
4018000000000000 4024000000000000 4008000000000000\n\
0000000000000000 8000000000000000 4008000000000000\n\
4018000000000000 8000000000000000 4008000000000000\n";

const TUTORIAL_ELE: &str = "tetgen ele\nfirst 1\ncorners 4\nattributes 1\ntets 6\n\
3 8 6 5 3ff0000000000000\n2 8 3 7 3ff0000000000000\n8 2 3 1 3ff0000000000000\n\
4 3 1 6 3ff0000000000000\n7 8 3 5 3ff0000000000000\n3 8 1 6 3ff0000000000000\n";

const TUTORIAL_FACE: &str = "tetgen face\nfirst 1\nmarkers 1\nfaces 12\n\
2 1 3 0\n3 1 4 1\n5 3 6 2\n6 3 4 3\n7 3 5 4\n2 3 7 5\n1 2 8 6\n2 7 8 7\n4 1 6 8\n1 8 6 9\n\
6 8 5 10\n8 7 5 11\n";

const TUTORIAL_NEIGH: &str = "tetgen neigh\nfirst 1\ntets 6\n\
-1 -1 5 6\n5 -1 -1 3\n-1 6 -1 2\n6 -1 -1 -1\n1 -1 -1 2\n-1 4 1 3\n";

#[test]
fn tutorial1_mesh_reads_with_consistent_counts() {
    let n = node(&tutorial("node"));
    let e = ele(&tutorial("ele"));
    let f = face(&tutorial("face"));
    let g = neigh(&tutorial("neigh"));

    // The 6 x 10 x 3 m box of tutorial 1: 8 corners, 6 tetrahedra, 12 hull triangles.
    assert_eq!((n.first, n.dim, n.points.len()), (1, 3, 8));
    assert_eq!((n.attributes_per_point, n.markers.is_some()), (0, false));
    assert_eq!(
        (e.first, e.corners, e.len(), e.attributes_per_tet),
        (1, 4, 6, 1)
    );
    assert_eq!((f.first, f.faces.len()), (1, 12));
    assert_eq!((g.first, g.neighbors.len()), (1, 6));

    // Point 1 is (6, -0, 0): TetGen wrote "-0" and the sign bit survives.
    assert_eq!(
        n.points[0].map(f64::to_bits),
        [6f64.to_bits(), (-0f64).to_bits(), 0]
    );
    assert_eq!(n.points[7], [6.0, -0.0, 3.0]);

    // Every .ele index is a point: 1-based, so index - 1 < node count.
    let count = n.points.len() as i32;
    assert!(
        e.tets.iter().all(|&c| c >= 1 && c - 1 < count),
        "{:?}",
        e.tets
    );
    assert_eq!(e.tet(0), [3, 8, 6, 5]);
    assert_eq!(e.tet(5), [3, 8, 1, 6]);
    // -A: one attribute per tetrahedron, the region number, 1 for the single room.
    assert!(e.attributes.iter().all(|&a| a == 1.0));

    // .face: 1-based points, markers are the .poly facet numbers 0..11.
    assert!(f.faces.iter().flatten().all(|&c| c >= 1 && c - 1 < count));
    assert_eq!(f.faces[0], [2, 1, 3]);
    assert_eq!(
        f.markers.as_deref(),
        Some(&(0..12).collect::<Vec<i32>>()[..])
    );

    // .neigh: -1 is the hull, anything else is a tetrahedron number 1..=6.
    assert_eq!(g.neighbors[0], [HULL, HULL, 5, 6]);
    let ntets = e.len() as i32;
    assert!(
        g.neighbors
            .iter()
            .flatten()
            .all(|&t| t == HULL || (1..=ntets).contains(&t))
    );
    // A closed box: each of the 12 hull triangles is one -1, every other face is shared.
    let hull = g.neighbors.iter().flatten().filter(|&&t| t == HULL).count();
    assert_eq!(hull, f.faces.len());
    assert_eq!((4 * e.len() - hull) % 2, 0);
    // Neighbour j lies across the face opposite corner j: it holds the other three corners.
    for t in 0..e.len() {
        for (j, &u) in g.neighbors[t].iter().enumerate() {
            if u == HULL {
                continue;
            }
            let theirs = e.tet((u - g.first) as usize);
            let shared: Vec<i32> = (0..4).filter(|&k| k != j).map(|k| e.tet(t)[k]).collect();
            assert!(
                shared.iter().all(|c| theirs.contains(c)),
                "tet {t} neighbour {j}"
            );
            assert!(!theirs.contains(&e.tet(t)[j]), "tet {t} neighbour {j}");
        }
    }

    tetgen::check_mesh(&n, &e, Some(&f), Some(&g)).unwrap();
}

#[test]
fn tutorial1_dumps_are_pinned() {
    assert_eq!(tetgen::dump_file(&tutorial("node")), TUTORIAL_NODE);
    assert_eq!(tetgen::dump_file(&tutorial("ele")), TUTORIAL_ELE);
    assert_eq!(tetgen::dump_file(&tutorial("face")), TUTORIAL_FACE);
    assert_eq!(tetgen::dump_file(&tutorial("neigh")), TUTORIAL_NEIGH);
}

#[test]
fn skipped_face_parses_as_535_skipped_facets() {
    let f = face(&skipped("face"));
    assert_eq!(f.first, 1);
    assert_eq!(f.faces.len(), 535);
    assert_eq!(f.faces[0], [838, 840, 841]);
    assert_eq!(f.faces[534], [338, 337, 1067]);
    // The fifth column is (int) badface::key, -1 on every row of this run; upstream's GUI reads
    // it as a 1-based facet number (projet_maillage.cpp:256), so it can highlight none of them.
    let markers = f.markers.as_ref().expect("header flag 1");
    assert_eq!(markers.len(), 535);
    assert!(markers.iter().all(|&m| m == -1));

    // The skipped facets index the _skipped.node TetGen wrote beside them.
    let n = node(&skipped("node"));
    assert_eq!((n.first, n.points.len()), (1, 1067));
    // "0.34999999999999998  -1  5.2000000000000002" and the last point, bit for bit.
    let bits = |p: [f64; 3]| p.map(f64::to_bits);
    assert_eq!(
        bits(n.points[0]),
        [0x3fd6666666666666, 0xbff0000000000000, 0x4014cccccccccccd]
    );
    assert_eq!(
        bits(n.points[1066]),
        [0x40337762472e01e3, 0xc02cf38071ba6902, 0x4027cccccccccccd]
    );
    let count = n.points.len() as i32;
    assert!(f.faces.iter().flatten().all(|&c| c >= 1 && c <= count));
}

/// Runs the oracle (TetGen's own loaders) over every fixture. It is built on demand
/// (`common/paths.rs`): an oracle that cannot be built fails this test, never skips it.
#[test]
fn oracle_agrees_on_every_fixture() {
    let exe = common::paths::oracle("tetgen");
    for path in fixtures() {
        let out = std::process::Command::new(&exe)
            .arg("dump")
            .arg("tetgen")
            .arg(&path)
            .output()
            .unwrap();
        let oracle = String::from_utf8_lossy(&out.stdout).replace("\r\n", "\n");
        assert!(
            !oracle.starts_with("error"),
            "oracle failed on {}",
            path.display()
        );
        assert_eq!(tetgen::dump_file(&path), oracle, "{}", path.display());
    }
}

// ---------------------------------------------------------------------------------------------
// Negative cases.

fn kind_of(r: Result<TetgenFile, FormatError>) -> &'static str {
    match r {
        Ok(_) => "ok",
        Err(FormatError::NotFound(_)) => "notfound",
        Err(FormatError::Truncated { .. }) => "truncated",
        Err(FormatError::Version { .. }) => "version",
        Err(FormatError::Invalid(_)) => "invalid",
        Err(FormatError::Io(_)) => "io",
    }
}

fn read(kind: Kind, text: &str) -> &'static str {
    kind_of(tetgen::read(kind, text.as_bytes()))
}

#[test]
fn missing_file_is_notfound() {
    let p = common::fixture("solver-outputs/tutorial1/no_such_mesh.1.node");
    assert!(matches!(
        tetgen::read_file(&p),
        Err(FormatError::NotFound(_))
    ));
    assert_eq!(tetgen::dump_file(&p), "error notfound\n");
}

#[test]
fn count_header_larger_than_the_lines_is_truncated() {
    // The declared counts are huge; the reader must refuse before reserving anything.
    assert_eq!(
        read(Kind::Node, "2000000000  3  0  0\n1 0 0 0\n"),
        "truncated"
    );
    assert_eq!(
        read(Kind::Ele, "2147483647  4  0\n1 1 2 3 4\n"),
        "truncated"
    );
    assert_eq!(read(Kind::Face, "1500000000  1\n1 1 2 3 0\n"), "truncated");
    assert_eq!(
        read(Kind::Neigh, "999999999  4\n1 -1 -1 -1 -1\n"),
        "truncated"
    );
    // One record short, and a header with nothing after it.
    assert_eq!(
        read(Kind::Node, "3 3 0 0\n1 0 0 0\n2 1 0 0\n# Generated by\n"),
        "truncated"
    );
    assert_eq!(read(Kind::Face, "4 0\n"), "truncated");
    // Nothing at all, or comments only.
    assert_eq!(read(Kind::Node, ""), "truncated");
    assert_eq!(read(Kind::Ele, "# only a comment\n\n"), "truncated");

    // A real fixture whose header claims more records than it holds.
    let text = std::fs::read_to_string(tutorial("ele")).unwrap();
    let bumped = text.replacen("6  4  1", "60000000  4  1", 1);
    assert_ne!(bumped, text);
    assert_eq!(read(Kind::Ele, &bumped), "truncated");
}

#[test]
fn file_kind_comes_from_the_extension() {
    assert_eq!(
        Kind::from_path(Path::new("a/scene_mesh.1.node")),
        Some(Kind::Node)
    );
    assert_eq!(
        Kind::from_path(Path::new("model_skipped.face")),
        Some(Kind::Face)
    );
    assert_eq!(Kind::from_path(Path::new("X.1.NEIGH")), Some(Kind::Neigh));
    assert_eq!(Kind::from_path(Path::new("scene_mesh.poly")), None);
    assert_eq!(Kind::from_path(Path::new(".node")), None);
    // An existing file with another extension is not guessed at.
    let poly = common::fixture("upstream/tutorial1/tetgen/scene_mesh.poly");
    assert_eq!(kind_of(tetgen::read_file(&poly)), "invalid");
    assert_eq!(tetgen::dump_file(&poly), "error invalid\n");
}

#[test]
fn malformed_files_are_refused() {
    let cases: &[(Kind, &str, &str)] = &[
        // Tokens TetGen would read as something other than what they say.
        (
            Kind::Node,
            "1 3 0 0\n1 0 0 abc\n",
            "letters where z belongs",
        ),
        (Kind::Node, "1 3 0 0\n1 0 0 0x10\n", "hexadecimal"),
        (Kind::Node, "1 3 0 0\n1 0 0 -inf\n", "infinity"),
        (
            Kind::Node,
            "1 3 0 0\n1 0 0 1e999\n",
            "overflows to infinity",
        ),
        (Kind::Node, "1 3 0 0\n1 0 0 1e\n", "exponent without digits"),
        (
            Kind::Ele,
            "1 4 0\n1 1 2 3 010\n",
            "leading zero (strtol base 0 reads octal)",
        ),
        (
            Kind::Ele,
            "1 4 0\n1 1 2 3 4.0\n",
            "real number for a corner",
        ),
        (
            Kind::Ele,
            "1 4 0\n1 1 2 3 3000000000\n",
            "corner beyond a C long",
        ),
        (
            Kind::Node,
            ",1 3 0 0\n1 0 0 0\n",
            "leading comma (load_node reads the header from col 0)",
        ),
        (Kind::Node, "\u{feff}1 3 0 0\n1 0 0 0\n", "byte-order mark"),
        (
            Kind::Node,
            "1 3 0 0\n1 0 0 0\r\r\n",
            "stray carriage return",
        ),
        // Fields missing or extra (TetGen defaults the one and ignores the other).
        (Kind::Node, "1 3 1 0\n1 0 0 0\n", "missing point attribute"),
        (Kind::Node, "1 3 0 1\n1 0 0 0\n", "missing point marker"),
        (Kind::Node, "1 3 0 0\n1 0 0 0 7\n", "extra field"),
        (Kind::Node, "1 3 0 0\n1 0 0\n", "missing z"),
        (Kind::Face, "1 1\n1 1 2 3\n", "missing face marker"),
        (Kind::Ele, "1 4 0 9\n1 1 2 3 4\n", "extra header field"),
        (Kind::Face, "1 1\n1 1 2 3 0 7 8\n", "-nn columns"),
        // Records.
        (
            Kind::Node,
            "2 3 0 0\n1 0 0 0\n1 1 1 1\n",
            "index out of sequence",
        ),
        (
            Kind::Node,
            "1 3 0 0\n2 0 0 0\n",
            "first index neither 0 nor 1",
        ),
        (
            Kind::Node,
            "1 3 0 0\n1 0 0 0\n2 1 1 1\n",
            "more records than declared",
        ),
        (
            Kind::Ele,
            "1 4 0\n1 0 2 3 4\n",
            "corner 0 in a 1-based file",
        ),
        (
            Kind::Ele,
            "1 4 0\n0 -1 1 2 3\n",
            "negative corner in a 0-based file",
        ),
        (
            Kind::Neigh,
            "2 4\n1 -1 -1 2 3\n2 -1 -1 -1 1\n",
            "neighbour past the last tetrahedron",
        ),
        (Kind::Neigh, "1 4\n1 -2 -1 -1 -1\n", "neighbour -2"),
        (
            Kind::Neigh,
            "1 4\n0 -1 -1 -1 1\n",
            "neighbour 1 in a 0-based single-tet file",
        ),
        // Headers.
        (Kind::Node, "-1 3 0 0\n", "negative point count"),
        (Kind::Node, "1 2 0 0\n1 0 0\n", "2-D node file"),
        (
            Kind::Node,
            "1 3 -1 0\n1 0 0 0\n",
            "negative attribute count",
        ),
        (Kind::Node, "1 3 0 0 1\n1 0 0 0\n", "uv flag"),
        (
            Kind::Node,
            "1 3 0 0 # rbox\n1 0 0 0\n",
            "rbox in the header switches load_node's layout",
        ),
        (Kind::Ele, "0 4 0\n", "zero tetrahedra (load_tet refuses)"),
        (Kind::Ele, "1 5 0\n1 1 2 3 4 5\n", "five corners"),
        (Kind::Face, "-3 1\n", "negative face count"),
        (Kind::Neigh, "1 3\n1 -1 -1 -1\n", "three neighbours"),
        (Kind::Neigh, "0 4\n", "zero tetrahedra"),
        // Bytes a text-mode C read treats specially.
        (Kind::Node, "1 3 0 0\n1 0 0 0\0\n", "NUL"),
        (Kind::Node, "1 3 0 0\n1 0 0 0\n\u{1a}", "Ctrl-Z"),
    ];
    for (kind, text, why) in cases {
        assert_eq!(read(*kind, text), "invalid", "{kind:?} {why}: {text:?}");
    }

    // MSVC's strtod keeps 768 significant digits and drops the rest unrounded: 1 + 2^-53 (exactly
    // halfway to the next double) plus a 1 in digit 769 is 1.0 in TetGen and 1 + 2^-52 here.
    let half = "1.00000000000000011102230246251565404236316680908203125"; // 54 digits
    let node_with = |x: String| format!("1 3 0 0\n1 {x} 0 0\n");
    let at_limit = node_with(format!("{half}{}1", "0".repeat(713)));
    let n = tetgen::read_node(at_limit.as_bytes()).unwrap();
    assert_eq!(n.points[0][0].to_bits(), 0x3ff0000000000001);
    let past_limit = node_with(format!("{half}{}1", "0".repeat(714)));
    assert_eq!(read(Kind::Node, &past_limit), "invalid");
    // Leading zeros do not count, as in MSVC.
    let lead = node_with(format!(
        "0.{}{}{}1e101",
        "0".repeat(100),
        half.replace('.', ""),
        "0".repeat(713)
    ));
    assert_eq!(read(Kind::Node, &lead), "ok");

    // A line TetGen's 2 048-byte buffer would split, even a comment.
    let long_comment = format!("1 3 0 0\n# {}\n1 0 0 0\n", "x".repeat(tetgen::MAX_LINE));
    assert_eq!(read(Kind::Node, &long_comment), "invalid");
    let at_limit = format!("1 3 0 0\n#{}\n1 0 0 0\n", "x".repeat(tetgen::MAX_LINE - 1));
    assert_eq!(read(Kind::Node, &at_limit), "ok");
}

#[test]
fn layout_variants_tetgen_accepts_read_the_same() {
    // CRLF and LF, commas, comments, blank lines, tabs and trailing text read identically.
    let lf = "8  3  0  0\n   1    6  -0  0\n   2    0  -0  0\n   3    0  10  0\n   4    6  10  0\n   5    0  10  3\n   6    6  10  3\n   7    0  -0  3\n   8    6  -0  3\n# Generated by tetgen\n";
    let crlf = lf.replace('\n', "\r\n");
    let odd = "# leading comment\n\n8,3,0,0 # header\n1\t6\t-0\t0\n2 0.0 -0.0 0e0\n3 ,0, 1e1, .0e5\n\
               \n# between records\n4 +6 10 0#comment\n5 0 10 3\n6 6 10 3\n7 0 -0 3\n8 6 -0 3\n";
    let a = tetgen::dump(&tetgen::read(Kind::Node, lf.as_bytes()).unwrap());
    assert_eq!(a, TUTORIAL_NODE);
    assert_eq!(
        tetgen::dump(&tetgen::read(Kind::Node, crlf.as_bytes()).unwrap()),
        a
    );
    assert_eq!(
        tetgen::dump(&tetgen::read(Kind::Node, odd.as_bytes()).unwrap()),
        a
    );

    // Optional header fields take TetGen's defaults.
    let n = tetgen::read_node(b"1\n0 1 2 3\n").unwrap();
    assert_eq!(
        (n.first, n.dim, n.attributes_per_point, n.markers.is_some()),
        (0, 3, 0, false)
    );
    let e = tetgen::read_ele(b"1\n0 0 1 2 3\n").unwrap();
    assert_eq!((e.first, e.corners, e.attributes_per_tet), (0, 4, 0));

    // Attributes and markers, 0-based.
    let n = tetgen::read_node(b"2 3 2 1\n0 0 0 0 0.5 -2 7\n1 1 1 1 1.5 -3 -7\n").unwrap();
    assert_eq!(n.attributes, [0.5, -2.0, 1.5, -3.0]);
    assert_eq!(n.markers, Some(vec![7, -7]));

    // Second-order tetrahedra carry ten corners.
    let e = tetgen::read_ele(b"1 10 1\n1 1 2 3 4 5 6 7 8 9 10 2.5\n").unwrap();
    assert_eq!(
        (e.len(), e.tet(0).len(), e.tet_attributes(0)),
        (1, 10, &[2.5][..])
    );

    // An empty .face is valid; TetGen then allocates no marker list, whatever the header says.
    let f = tetgen::read_face(b"0  1\n# Generated by tetgen\n").unwrap();
    assert_eq!((f.first, f.faces.len(), f.markers.is_none()), (0, 0, true));
    assert_eq!(
        tetgen::dump(&TetgenFile::Face(f)),
        "tetgen face\nfirst 0\nmarkers 0\nfaces 0\n"
    );
    // ...while an empty .node keeps its marker list.
    let n = tetgen::read_node(b"0 3 0 1\n").unwrap();
    assert_eq!(n.markers, Some(vec![]));
}

#[test]
fn check_mesh_refuses_files_that_disagree() {
    let n = node(&tutorial("node"));
    let e = ele(&tutorial("ele"));
    let f = face(&tutorial("face"));
    let g = neigh(&tutorial("neigh"));

    let mut bad = e.clone();
    bad.tets[3] = 9; // one past the 8 points
    assert!(matches!(
        tetgen::check_mesh(&n, &bad, None, None),
        Err(FormatError::Invalid(_))
    ));

    let mut bad = f.clone();
    bad.faces[11][2] = 9;
    assert!(matches!(
        tetgen::check_mesh(&n, &e, Some(&bad), None),
        Err(FormatError::Invalid(_))
    ));

    let mut bad = g.clone();
    bad.neighbors[0] = [HULL, HULL, HULL, 6]; // 5 still lists 1
    assert!(matches!(
        tetgen::check_mesh(&n, &e, None, Some(&bad)),
        Err(FormatError::Invalid(_))
    ));

    let mut bad = g.clone();
    bad.neighbors.pop();
    assert!(matches!(
        tetgen::check_mesh(&n, &e, None, Some(&bad)),
        Err(FormatError::Invalid(_))
    ));

    let zero_based = tetgen::read_ele(b"1 4 0\n0 0 1 2 3\n").unwrap();
    assert!(matches!(
        tetgen::check_mesh(&n, &zero_based, None, None),
        Err(FormatError::Invalid(_))
    ));
}
