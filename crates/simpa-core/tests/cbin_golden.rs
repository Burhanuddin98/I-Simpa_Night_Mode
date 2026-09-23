//! Golden values and negative cases for `.cbin` (docs/formats/cbin.md).
//!
//! Expected values come from upstream, never from this reader: `lib_interface/tests/io_test.cpp`
//! (cube.cbin), `python_bindings/tests/check_retrocompat.py` (mesh.cbin), and tutorial 1's own
//! config.xml and room (tutorial1/*/mesh.cbin).
mod common;

use common::fixture;
use simpa_core::formats::FormatError;
use simpa_core::formats::cbin::{self, Face, Model, Vertex};

fn face(a: u32, b: u32, c: u32, id_mat: u32, id_rs: i32, id_en: i32) -> Face {
    Face {
        a,
        b,
        c,
        id_mat,
        id_rs,
        id_en,
    }
}

fn v(x: f32, y: f32, z: f32) -> Vertex {
    Vertex { x, y, z }
}

fn bytes(rel: &str) -> Vec<u8> {
    std::fs::read(fixture(rel)).unwrap()
}

fn put_u32(buf: &mut [u8], at: usize, value: u32) {
    buf[at..at + 4].copy_from_slice(&value.to_le_bytes());
}

fn put_u16(buf: &mut [u8], at: usize, value: u16) {
    buf[at..at + 2].copy_from_slice(&value.to_le_bytes());
}

/// A scratch directory under cargo's target tmp directory, one per test, reused across runs.
fn scratch(name: &str) -> std::path::PathBuf {
    let dir = std::path::Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("cbin_golden")
        .join(name);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// The eight vertices of io_test.cpp:24-31 and :96-103.
const CUBE_VERTICES: [[f32; 3]; 8] = [
    [5.0, 0.0, 0.0],
    [0.0, 0.0, 0.0],
    [0.0, 5.0, 0.0],
    [5.0, 5.0, 0.0],
    [0.0, 5.0, 5.0],
    [5.0, 5.0, 5.0],
    [0.0, 0.0, 5.0],
    [5.0, 0.0, 5.0],
];

/// The triangles of io_test.cpp:33-44 and :106-117.
const CUBE_TRIANGLES: [[u32; 3]; 12] = [
    [0, 1, 2],
    [0, 2, 3],
    [2, 4, 5],
    [2, 5, 3],
    [2, 6, 4],
    [2, 1, 6],
    [1, 0, 7],
    [6, 1, 7],
    [0, 3, 5],
    [7, 0, 5],
    [7, 5, 4],
    [6, 7, 4],
];

// Byte offsets in cube.cbin (8 vertices): header 0, vertices node 8, nbVertex 20, vertex data 24,
// group node 120, group name 132, nbFace 388, face data 392, end 680.
const CUBE_LEN: usize = 680;
const CUBE_NB_VERTEX: usize = 20;
const CUBE_GROUP_NODE: usize = 120;
const CUBE_NB_FACE: usize = 388;
const CUBE_FACES: usize = 392;

#[test]
fn cube_matches_upstream_retrocompat_test() {
    // io_test.cpp:83-118, retrocompat_test1.
    let m = cbin::read_file(&fixture("upstream/lib_interface/cube.cbin")).unwrap();
    assert_eq!(m.vertices.len(), 8);
    assert_eq!(m.faces.len(), 12);
    assert_eq!(m.faces[0], face(0, 1, 2, 0, -1, -1));
    for (got, want) in m.vertices.iter().zip(CUBE_VERTICES) {
        assert_eq!(*got, v(want[0], want[1], want[2]));
    }
    for (got, t) in m.faces.iter().zip(CUBE_TRIANGLES) {
        assert_eq!(*got, face(t[0], t[1], t[2], 0, -1, -1));
    }
}

#[test]
fn cube_dump_keeps_the_sign_of_zero() {
    // The file stores y = -0.0 on vertex 0 (bytes 28..32 are 00 00 00 80); upstream's test compares
    // with a tolerance, the dump compares bits.
    let d = cbin::dump_file(&fixture("upstream/lib_interface/cube.cbin"));
    let lines: Vec<&str> = d.lines().collect();
    assert_eq!(lines[0], "cbin 1 0");
    assert_eq!(lines[1], "vertices 8");
    assert_eq!(lines[2], "40a00000 80000000 00000000");
    assert_eq!(lines[10], "faces 12");
    assert_eq!(lines[11], "0 1 2 0 -1 -1");
    assert_eq!(lines.len(), 1 + 1 + 8 + 1 + 12);
    assert!(d.ends_with("6 7 4 0 -1 -1\n"));
}

/// check_retrocompat.py:7, each entry [a, b, c, idEn, idMat, idRs] (the script's own order, :17).
const PY_EXPECTED: [[i64; 6]; 76] = [
    [16, 27, 22, -1, 101, -1],
    [19, 27, 16, -1, 101, -1],
    [4, 11, 2, -1, 100, -1],
    [11, 10, 2, -1, 100, -1],
    [19, 1, 37, -1, 100, -1],
    [14, 9, 18, -1, 100, -1],
    [0, 41, 13, -1, 100, -1],
    [7, 12, 41, -1, 100, -1],
    [15, 40, 8, -1, 100, -1],
    [23, 40, 15, -1, 100, -1],
    [18, 28, 14, -1, 100, -1],
    [33, 22, 10, -1, 101, -1],
    [24, 23, 41, -1, 100, -1],
    [33, 32, 22, -1, 101, -1],
    [24, 35, 16, -1, 101, -1],
    [20, 14, 4, -1, 100, -1],
    [24, 41, 0, -1, 100, -1],
    [10, 11, 31, -1, 100, -1],
    [10, 31, 33, -1, 100, -1],
    [9, 19, 18, -1, 100, -1],
    [18, 37, 6, -1, 100, -1],
    [12, 37, 13, -1, 100, -1],
    [13, 37, 1, -1, 100, -1],
    [10, 22, 20, -1, 101, -1],
    [16, 22, 8, -1, 101, -1],
    [9, 14, 20, -1, 100, -1],
    [21, 15, 30, -1, 100, -1],
    [23, 7, 41, -1, 100, -1],
    [12, 13, 41, -1, 100, -1],
    [5, 39, 29, -1, 100, -1],
    [16, 8, 24, -1, 101, -1],
    [12, 6, 37, -1, 100, -1],
    [0, 35, 24, -1, 101, -1],
    [18, 26, 17, -1, 100, -1],
    [13, 35, 0, -1, 101, -1],
    [15, 17, 23, -1, 100, -1],
    [2, 20, 4, -1, 100, -1],
    [10, 20, 2, -1, 101, -1],
    [15, 21, 17, -1, 100, -1],
    [31, 11, 21, -1, 100, -1],
    [31, 21, 30, -1, 100, -1],
    [8, 40, 24, -1, 100, -1],
    [33, 31, 29, -1, 100, -1],
    [17, 28, 18, -1, 100, -1],
    [21, 28, 17, -1, 100, -1],
    [18, 19, 37, -1, 100, -1],
    [16, 25, 19, -1, 101, -1],
    [1, 25, 13, -1, 101, -1],
    [25, 1, 19, -1, 101, -1],
    [6, 26, 18, -1, 100, -1],
    [26, 6, 12, -1, 100, -1],
    [9, 27, 19, -1, 101, -1],
    [20, 27, 9, -1, 101, -1],
    [22, 27, 20, -1, 101, -1],
    [14, 28, 4, -1, 100, -1],
    [11, 28, 21, -1, 100, -1],
    [4, 28, 11, -1, 100, -1],
    [5, 31, 30, -1, 100, -1],
    [29, 31, 5, -1, 100, -1],
    [24, 40, 23, -1, 100, -1],
    [32, 8, 22, -1, 101, -1],
    [32, 33, 3, -1, 101, -1],
    [3, 33, 29, -1, 100, -1],
    [17, 34, 23, -1, 100, -1],
    [26, 34, 17, -1, 100, -1],
    [12, 34, 26, -1, 100, -1],
    [7, 34, 12, -1, 100, -1],
    [34, 7, 23, -1, 100, -1],
    [25, 35, 13, -1, 101, -1],
    [16, 35, 25, -1, 101, -1],
    [30, 39, 5, -1, 100, -1],
    [15, 39, 30, -1, 100, -1],
    [3, 39, 32, -1, 100, -1],
    [29, 39, 3, -1, 100, -1],
    [8, 39, 15, -1, 100, -1],
    [32, 39, 8, -1, 100, -1],
];

#[test]
fn python_bindings_mesh_matches_check_retrocompat() {
    let m = cbin::read_file(&fixture("upstream/python_bindings/mesh.cbin")).unwrap();
    // check_retrocompat.py:18-26: the same faces in any order, and every model face expected.
    assert_eq!(m.faces.len(), PY_EXPECTED.len());
    let mut got: Vec<[i64; 6]> = m
        .faces
        .iter()
        .map(|f| {
            [
                f.a.into(),
                f.b.into(),
                f.c.into(),
                f.id_en.into(),
                f.id_mat.into(),
                f.id_rs.into(),
            ]
        })
        .collect();
    let mut want = PY_EXPECTED.to_vec();
    got.sort_unstable();
    want.sort_unstable();
    assert_eq!(got, want);
    // check_retrocompat.py:27-39: 42 vertices spanning [0, 20] x [0, 30] x [0, 10].
    assert_eq!(m.vertices.len(), 42);
    let axis = |f: fn(&Vertex) -> f32| {
        let vals: Vec<f32> = m.vertices.iter().map(f).collect();
        let lo = vals.iter().copied().fold(f32::INFINITY, f32::min);
        let hi = vals.iter().copied().fold(f32::NEG_INFINITY, f32::max);
        (lo, hi)
    };
    for ((lo, hi), want_hi) in [axis(|p| p.x), axis(|p| p.y), axis(|p| p.z)]
        .into_iter()
        .zip([20.0, 30.0, 10.0])
    {
        assert!(lo.abs() < 1e-5, "min {lo}");
        assert!((hi - want_hi).abs() < 1e-5, "max {hi} vs {want_hi}");
    }
}

#[test]
fn tutorial1_mesh_is_the_tutorial_shoebox() {
    let spps = cbin::read_file(&fixture("upstream/tutorial1/spps/mesh.cbin")).unwrap();
    let tcr = cbin::read_file(&fixture("upstream/tutorial1/tcr/mesh.cbin")).unwrap();
    assert_eq!(spps, tcr);
    // Tutorial 1's room is a 6 x 10 x 3 m box: 12 triangles, each with three vertices of its own.
    assert_eq!(spps.vertices.len(), 36);
    assert_eq!(spps.faces.len(), 12);
    for (i, f) in spps.faces.iter().enumerate() {
        let i = i as u32;
        assert_eq!((f.a, f.b, f.c), (3 * i, 3 * i + 1, 3 * i + 2));
        assert_eq!(f.id_en, -1);
    }
    for p in &spps.vertices {
        assert!(
            [0.0, 6.0].contains(&p.x) && [0.0, 10.0].contains(&p.y) && [0.0, 3.0].contains(&p.z)
        );
    }
    // Material and receiver ids must be ones the project's config.xml declares.
    let config = std::fs::read_to_string(fixture("upstream/tutorial1/spps/config.xml")).unwrap();
    for f in &spps.faces {
        assert!(
            config.contains(&format!("<type_surface id=\"{}\"", f.id_mat)),
            "idMat {}",
            f.id_mat
        );
        if f.id_rs != -1 {
            assert!(config.contains(&format!("<recepteur_surfacique id=\"{}\"", f.id_rs)));
        }
    }
    let mats: Vec<u32> = spps.faces.iter().map(|f| f.id_mat).collect();
    assert_eq!(mats, [25, 25, 22, 22, 22, 22, 22, 22, 22, 22, 21, 21]);
    let receivers: Vec<i32> = spps.faces.iter().map(|f| f.id_rs).collect();
    assert_eq!(
        receivers,
        [3503, 3503, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1]
    );
}

#[test]
fn writer_reproduces_an_upstream_file_byte_for_byte() {
    // tutorial1's mesh.cbin was written by upstream's ExportBIN with zeroed padding and group name.
    let raw = bytes("upstream/tutorial1/spps/mesh.cbin");
    assert_eq!(cbin::write(&cbin::read(&raw).unwrap()), raw);
}

#[test]
fn every_fixture_survives_a_write_read_round_trip() {
    // cube.cbin and python mesh.cbin carry stack garbage in padding, firtSon and the group name
    // (an older writer); none of it is part of the model, so the round trip keeps the model only.
    for rel in [
        "upstream/lib_interface/cube.cbin",
        "upstream/python_bindings/mesh.cbin",
        "upstream/tutorial1/spps/mesh.cbin",
        "upstream/tutorial1/tcr/mesh.cbin",
    ] {
        let m = cbin::read(&bytes(rel)).unwrap();
        let again = cbin::read(&cbin::write(&m)).unwrap();
        assert_eq!(cbin::dump(&again), cbin::dump(&m), "{rel}");
    }
}

#[test]
fn upstream_constructor_test_round_trips() {
    // io_test.cpp:20-79: ExportBIN then ImportBIN, with idMat -1 (stored as 0xFFFFFFFF) and
    // receiver ids 66 and 100.
    let rs = [66, 66, 100, 100, 100, 100, 100, 100, 100, 100, 66, 66];
    let model = Model {
        faces: CUBE_TRIANGLES
            .iter()
            .zip(rs)
            .map(|(t, r)| face(t[0], t[1], t[2], u32::MAX, r, -1))
            .collect(),
        vertices: CUBE_VERTICES.iter().map(|p| v(p[0], p[1], p[2])).collect(),
    };
    let path = scratch("ctor").join("test_io_wr.cbin");
    cbin::write_file(&model, &path).unwrap();
    assert_eq!(std::fs::metadata(&path).unwrap().len() as usize, CUBE_LEN);
    let back = cbin::read_file(&path).unwrap();
    assert_eq!(back, model);
    assert_eq!(back.faces[0], face(0, 1, 2, u32::MAX, 66, -1));
}

#[test]
fn generated_models_round_trip_and_differ_per_seed() {
    let mut seen = std::collections::HashSet::new();
    for seed in 0..200u64 {
        let m = cbin::generate(seed);
        assert!(!m.vertices.is_empty());
        let back = cbin::read(&cbin::write(&m)).unwrap_or_else(|e| panic!("seed {seed}: {e}"));
        let d = cbin::dump(&m);
        assert_eq!(cbin::dump(&back), d, "seed {seed}");
        assert!(seen.insert(d), "seed {seed} repeats an earlier model");
    }
}

// Upstream's node walk (bin.cpp:236-271), on files it reads without touching undefined values.

/// The cube with the group node's `nextBrother` set, and a 16-byte gap of junk before a second
/// vertices node, which the first node's `nextBrother` jumps over.
fn cube_with_extra_nodes() -> Vec<u8> {
    let mut b = bytes("upstream/lib_interface/cube.cbin");
    let second_at = (CUBE_LEN + 16) as u32;
    put_u32(&mut b, CUBE_GROUP_NODE + 8, second_at);
    b.extend_from_slice(&[0xAB; 16]);
    // Vertices node: type 0, padding, firstSon, nextBrother 0, one vertex.
    b.extend_from_slice(&0u16.to_le_bytes());
    b.extend_from_slice(&[0xCD, 0xEF]);
    b.extend_from_slice(&7u32.to_le_bytes());
    b.extend_from_slice(&0u32.to_le_bytes());
    b.extend_from_slice(&1u32.to_le_bytes());
    for x in [1.5f32, 2.5, 3.5] {
        b.extend_from_slice(&x.to_le_bytes());
    }
    b
}

#[test]
fn nodes_append_and_forward_jumps_skip_junk() {
    let m = cbin::read(&cube_with_extra_nodes()).unwrap();
    assert_eq!(m.vertices.len(), 9);
    assert_eq!(m.vertices[8], v(1.5, 2.5, 3.5));
    assert_eq!(m.faces.len(), 12);
}

#[test]
fn a_backward_next_brother_continues_where_the_node_ended() {
    // Point the group node's nextBrother back at the header; upstream ignores a backward link and
    // reads the next node right after the group, here a second vertices node.
    let mut b = cube_with_extra_nodes();
    put_u32(&mut b, CUBE_GROUP_NODE + 8, 8);
    b.drain(CUBE_LEN..CUBE_LEN + 16);
    let m = cbin::read(&b).unwrap();
    assert_eq!(m.vertices.len(), 9);
}

#[test]
fn bytes_after_the_last_node_are_ignored() {
    let mut b = bytes("upstream/lib_interface/cube.cbin");
    let clean = cbin::read(&b).unwrap();
    b.extend_from_slice(b"trailing bytes are never read");
    assert_eq!(cbin::read(&b).unwrap(), clean);
}

// Negative cases.

#[test]
fn missing_file_is_not_found() {
    let path = fixture("upstream/lib_interface/no_such_file.cbin");
    assert!(matches!(
        cbin::read_file(&path),
        Err(FormatError::NotFound(_))
    ));
    assert_eq!(cbin::dump_file(&path), "error notfound\n");
}

#[test]
fn a_version_other_than_1_0_is_refused() {
    for (major, minor) in [(2, 0), (1, 1), (0, 0), (0, 1), (u32::MAX, 0)] {
        let mut b = bytes("upstream/lib_interface/cube.cbin");
        put_u32(&mut b, 0, major);
        put_u32(&mut b, 4, minor);
        match cbin::read(&b) {
            Err(FormatError::Version { found, .. }) => {
                assert_eq!(found, format!("{major}.{minor}"))
            }
            other => panic!("{major}.{minor}: {other:?}"),
        }
    }
    let dir = scratch("version");
    let path = dir.join("v2.cbin");
    let mut b = bytes("upstream/lib_interface/cube.cbin");
    put_u32(&mut b, 0, 2);
    std::fs::write(&path, &b).unwrap();
    assert_eq!(cbin::dump_file(&path), "error version\n");
}

#[test]
fn every_truncation_is_truncated() {
    for rel in [
        "upstream/lib_interface/cube.cbin",
        "upstream/tutorial1/spps/mesh.cbin",
    ] {
        let b = bytes(rel);
        for len in 0..b.len() {
            assert!(
                matches!(cbin::read(&b[..len]), Err(FormatError::Truncated { .. })),
                "{rel} cut to {len} bytes: {:?}",
                cbin::read(&b[..len]).map(|m| (m.vertices.len(), m.faces.len()))
            );
        }
    }
    let path = scratch("trunc").join("cut.cbin");
    std::fs::write(&path, &bytes("upstream/lib_interface/cube.cbin")[..500]).unwrap();
    assert_eq!(cbin::dump_file(&path), "error truncated\n");
}

#[test]
fn a_count_larger_than_the_file_is_truncated_not_allocated() {
    // 55 vertices need 660 bytes where 656 remain; 13 faces need 312 where 288 remain.
    for (at, count) in [
        (CUBE_NB_VERTEX, u32::MAX),
        (CUBE_NB_VERTEX, 55),
        (CUBE_NB_FACE, u32::MAX),
        (CUBE_NB_FACE, 13),
    ] {
        let mut b = bytes("upstream/lib_interface/cube.cbin");
        put_u32(&mut b, at, count);
        assert!(
            matches!(cbin::read(&b), Err(FormatError::Truncated { .. })),
            "count {count} at {at}"
        );
    }
}

#[test]
fn a_next_brother_past_the_end_is_truncated() {
    for target in [CUBE_LEN as u32, CUBE_LEN as u32 + 1, u32::MAX] {
        let mut b = bytes("upstream/lib_interface/cube.cbin");
        put_u32(&mut b, CUBE_GROUP_NODE + 8, target);
        assert!(
            matches!(cbin::read(&b), Err(FormatError::Truncated { .. })),
            "nextBrother {target}"
        );
    }
}

#[test]
fn an_unknown_node_type_is_invalid() {
    for t in [2u16, 0xFFFF] {
        let mut b = bytes("upstream/lib_interface/cube.cbin");
        put_u16(&mut b, CUBE_GROUP_NODE, t);
        assert!(
            matches!(cbin::read(&b), Err(FormatError::Invalid(_))),
            "type {t}"
        );
    }
}

#[test]
fn a_face_outside_the_vertex_list_is_invalid() {
    let mut b = bytes("upstream/lib_interface/cube.cbin");
    put_u32(&mut b, CUBE_FACES + 24 * 5 + 8, 8); // face 5, c = 8 with 8 vertices
    assert!(matches!(cbin::read(&b), Err(FormatError::Invalid(_))));

    let mut m = cbin::read(&bytes("upstream/lib_interface/cube.cbin")).unwrap();
    m.faces[3].a = 8;
    let path = scratch("badindex").join("never_written.cbin");
    assert!(matches!(
        cbin::write_file(&m, &path),
        Err(FormatError::Invalid(_))
    ));
    assert!(!path.exists(), "write_file created a file it refused");
}
