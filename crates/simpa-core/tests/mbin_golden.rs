mod common;

use std::sync::{Mutex, MutexGuard};

use simpa_core::formats::FormatError;
use simpa_core::formats::mbin::{self, Mesh, TetraFace, Tetrahedron};

// The negative case measures allocation. The counters are per thread now (common/mod.rs), so
// the lock every test here holds is no longer required; it is kept as harmless belt and braces.
#[global_allocator]
static A: common::Tracking = common::Tracking;
static SERIAL: Mutex<()> = Mutex::new(());

fn serial() -> MutexGuard<'static, ()> {
    SERIAL.lock().unwrap_or_else(|e| e.into_inner())
}

fn load(rel: &str) -> Mesh {
    mbin::read_file(&common::fixture(rel)).unwrap_or_else(|e| panic!("{rel}: {e}"))
}

/// Sum of |tetrahedron volume| in f64 (upstream's `CMBIN::ComputeVolume`, mbin.cpp:220).
fn volume(mesh: &Mesh) -> f64 {
    mesh.tetrahedra
        .iter()
        .map(|t| {
            let p = t.vertices.map(|v| mesh.nodes[v as usize].map(f64::from));
            let d = |a: [f64; 3], b: [f64; 3]| [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
            let (a, b, c) = (d(p[0], p[3]), d(p[1], p[3]), d(p[2], p[3]));
            let cross = [
                b[1] * c[2] - b[2] * c[1],
                b[2] * c[0] - b[0] * c[2],
                b[0] * c[1] - b[1] * c[0],
            ];
            ((a[0] * cross[0] + a[1] * cross[1] + a[2] * cross[2]) / 6.0).abs()
        })
        .sum()
}

/// Extent of the nodes on each axis.
fn extent(mesh: &Mesh) -> [f64; 3] {
    let mut out = [0.0; 3];
    for (a, o) in out.iter_mut().enumerate() {
        let lo = mesh
            .nodes
            .iter()
            .map(|n| f64::from(n[a]))
            .fold(f64::INFINITY, f64::min);
        let hi = mesh
            .nodes
            .iter()
            .map(|n| f64::from(n[a]))
            .fold(f64::NEG_INFINITY, f64::max);
        *o = hi - lo;
    }
    out
}

/// Properties every upstream mesh in the fixtures has: face i is opposite vertex i, neighbours
/// are mutual, a face has a marker exactly when it has no neighbour, and indices are in range.
fn check_upstream_conventions(mesh: &Mesh) {
    mbin::validate(mesh).unwrap();
    for (t, tet) in mesh.tetrahedra.iter().enumerate() {
        for (i, face) in tet.faces.iter().enumerate() {
            let mut expect: Vec<i32> = (0..4)
                .filter(|&k| k != i)
                .map(|k| tet.vertices[k])
                .collect();
            let mut got = face.vertices.to_vec();
            expect.sort_unstable();
            got.sort_unstable();
            assert_eq!(got, expect, "tetra {t} face {i} is not opposite vertex {i}");
            if face.neighbor >= 0 {
                assert_eq!(face.marker, -1, "tetra {t} face {i}");
                let other = &mesh.tetrahedra[face.neighbor as usize];
                assert!(
                    other.faces.iter().any(|f| f.neighbor == t as i32),
                    "tetra {t} face {i}: neighbour {} does not point back",
                    face.neighbor
                );
            } else {
                assert_eq!(face.neighbor, -2, "tetra {t} face {i}");
                assert!(
                    face.marker >= 0,
                    "tetra {t} face {i}: boundary face without marker"
                );
            }
        }
    }
}

fn tetra(v: [i32; 4], id_volume: i32, f: [[i32; 5]; 4]) -> Tetrahedron {
    Tetrahedron {
        vertices: v,
        id_volume,
        faces: f.map(|f| TetraFace {
            vertices: [f[0], f[1], f[2]],
            marker: f[3],
            neighbor: f[4],
        }),
    }
}

#[test]
fn cube_mesh_matches_upstream_io_test() {
    let _g = serial();
    let mesh = load("upstream/lib_interface/cube_mesh.mbin");

    // Expected content from upstream's own test, lib_interface/tests/io_test.cpp:123-169.
    let nodes: [[f32; 3]; 8] = [
        [5., 0., 0.],
        [0., 0., 0.],
        [0., 5., 0.],
        [5., 5., 0.],
        [0., 5., 5.],
        [5., 5., 5.],
        [0., 0., 5.],
        [5., 0., 5.],
    ];
    assert_eq!(mesh.nodes, nodes); // compares as floats: the file stores -0.0 in some y's
    let expected = [
        tetra(
            [4, 5, 7, 2],
            1,
            [
                [5, 2, 7, -1, 5],
                [7, 2, 4, -1, 4],
                [4, 2, 5, 2, -2],
                [5, 7, 4, 10, -2],
            ],
        ),
        tetra(
            [6, 2, 7, 1],
            1,
            [
                [2, 1, 7, -1, 2],
                [7, 1, 6, 7, -2],
                [6, 1, 2, 5, -2],
                [2, 7, 6, -1, 4],
            ],
        ),
        tetra(
            [0, 2, 1, 7],
            1,
            [
                [2, 7, 1, -1, 1],
                [1, 7, 0, 6, -2],
                [0, 7, 2, -1, 5],
                [2, 1, 0, 0, -2],
            ],
        ),
        tetra(
            [0, 3, 2, 5],
            1,
            [
                [3, 5, 2, 3, -2],
                [2, 5, 0, -1, 5],
                [0, 5, 3, 8, -2],
                [3, 2, 0, 1, -2],
            ],
        ),
        tetra(
            [4, 2, 7, 6],
            1,
            [
                [2, 6, 7, -1, 1],
                [7, 6, 4, 11, -2],
                [4, 6, 2, 4, -2],
                [2, 7, 4, -1, 0],
            ],
        ),
        tetra(
            [5, 0, 7, 2],
            1,
            [
                [0, 2, 7, -1, 2],
                [7, 2, 5, -1, 0],
                [5, 2, 0, -1, 3],
                [0, 7, 5, 9, -2],
            ],
        ),
    ];
    assert_eq!(mesh.tetrahedra, expected);

    // 6 tetrahedra filling a 5 m cube: 125.000 m3.
    assert_eq!(mesh.tetrahedra.len(), 6);
    let v = volume(&mesh);
    assert!((v - 125.0).abs() < 1e-3, "cube volume {v}");
    check_upstream_conventions(&mesh);

    // Exact bits, -0.0 included, in the canonical dump.
    let dump = mbin::dump(&mesh);
    assert!(
        dump.starts_with("mbin\nnodes 8\nnode 40a00000 80000000 00000000\n"),
        "{dump}"
    );
    assert!(dump.contains(
        "\ntetrahedra 6\ntetra 4 5 7 2 1 face 5 2 7 -1 5 face 7 2 4 -1 4 face 4 2 5 2 -2 face 5 7 4 10 -2\n"
    ));
    assert_eq!(dump.lines().count(), 1 + 1 + 8 + 1 + 6);
}

#[test]
fn python_bindings_tetramesh() {
    let _g = serial();
    let mesh = load("upstream/python_bindings/tetramesh.mbin");
    assert_eq!(mesh.tetrahedra.len(), 102);
    assert_eq!(mesh.nodes.len(), 46);
    // 8 + 12*46 + 100*102 = 10760, the file's size: nothing is left over.
    assert_eq!(
        std::fs::metadata(common::fixture("upstream/python_bindings/tetramesh.mbin"))
            .unwrap()
            .len(),
        10760
    );
    assert!(mesh.tetrahedra.iter().all(|t| t.id_volume == 0));
    // A 20 x 30 x 10 m box: the tetrahedra fill it (f32 nodes leave ~5e-4 m3 of rounding).
    let e = extent(&mesh);
    assert!(
        (e[0] - 20.0).abs() < 1e-5 && (e[1] - 30.0).abs() < 1e-5 && (e[2] - 10.0).abs() < 1e-5,
        "{e:?}"
    );
    let v = volume(&mesh);
    assert!((v - 6000.0).abs() < 1e-2, "volume {v}");
    check_upstream_conventions(&mesh);
    assert_eq!(
        mesh.tetrahedra[0],
        tetra(
            [13, 36, 34, 35],
            0,
            [
                [36, 35, 34, -1, 37],
                [34, 35, 13, -1, 60],
                [13, 35, 36, -1, 83],
                [36, 34, 13, -1, 85]
            ]
        )
    );
}

#[test]
fn tutorial1_tetramesh_spps_and_tcr() {
    let _g = serial();
    let spps = load("upstream/tutorial1/spps/tetramesh.mbin");
    let tcr = load("upstream/tutorial1/tcr/tetramesh.mbin");
    assert_eq!(mbin::dump(&spps), mbin::dump(&tcr));
    assert_eq!(spps.tetrahedra.len(), 2257);
    assert_eq!(spps.nodes.len(), 732);
    assert!(spps.tetrahedra.iter().all(|t| t.id_volume == 1));
    // A 6 x 10 x 3 m room: 180 m3.
    let e = extent(&spps);
    assert!(
        (e[0] - 6.0).abs() < 1e-6 && (e[1] - 10.0).abs() < 1e-6 && (e[2] - 3.0).abs() < 1e-6,
        "{e:?}"
    );
    let v = volume(&spps);
    assert!((v - 180.0).abs() < 1e-3, "volume {v}");
    check_upstream_conventions(&spps);
    assert_eq!(
        spps.tetrahedra[0],
        tetra(
            [671, 574, 21, 9],
            1,
            [
                [574, 9, 21, -1, 92],
                [21, 9, 671, -1, 1900],
                [671, 9, 574, -1, 1433],
                [574, 21, 671, -1, 1497]
            ]
        )
    );
    let boundary = spps
        .tetrahedra
        .iter()
        .flat_map(|t| t.faces.iter())
        .filter(|f| f.neighbor < 0)
        .count();
    assert_eq!(boundary, 1270);
}

#[test]
fn every_fixture_round_trips_byte_exact() {
    let _g = serial();
    for rel in [
        "upstream/lib_interface/cube_mesh.mbin",
        "upstream/python_bindings/tetramesh.mbin",
        "upstream/tutorial1/spps/tetramesh.mbin",
        "upstream/tutorial1/tcr/tetramesh.mbin",
    ] {
        let bytes = std::fs::read(common::fixture(rel)).unwrap();
        let mesh = mbin::read(&bytes).unwrap();
        assert_eq!(mbin::write(&mesh), bytes, "{rel}");
    }
}

#[test]
fn missing_file_is_not_found() {
    let _g = serial();
    let path = common::fixture("upstream/lib_interface/no_such_mesh.mbin");
    assert!(matches!(
        mbin::read_file(&path),
        Err(FormatError::NotFound(_))
    ));
    assert_eq!(mbin::dump_file(&path), "error notfound\n");
}

#[test]
fn cube_truncated_to_300_bytes_is_truncated() {
    let _g = serial();
    let bytes = std::fs::read(common::fixture("upstream/lib_interface/cube_mesh.mbin")).unwrap();
    assert_eq!(bytes.len(), 704);
    assert!(matches!(
        mbin::read(&bytes[..300]),
        Err(FormatError::Truncated { .. })
    ));
    // Every shorter prefix fails the same way; the full file and anything longer read.
    for n in 0..bytes.len() {
        assert!(
            matches!(mbin::read(&bytes[..n]), Err(FormatError::Truncated { .. })),
            "{n} bytes"
        );
    }
    let mut longer = bytes.clone();
    longer.extend_from_slice(&[0xab; 7]);
    assert_eq!(mbin::read(&longer).unwrap(), mbin::read(&bytes).unwrap());
}

#[test]
fn counts_larger_than_the_file_are_truncated_without_allocating() {
    let _g = serial();
    let cube = std::fs::read(common::fixture("upstream/lib_interface/cube_mesh.mbin")).unwrap();
    let with_header = |tetra: u32, nodes: u32| {
        let mut b = cube.clone();
        b[0..4].copy_from_slice(&tetra.to_le_bytes());
        b[4..8].copy_from_slice(&nodes.to_le_bytes());
        b
    };
    let cases = [
        with_header(u32::MAX, u32::MAX),
        with_header(u32::MAX, 8),
        with_header(6, u32::MAX),
        with_header(1_000_000, 8), // 100 MB of tetrahedra claimed by a 704-byte file
        with_header(6, 1_000_000),
        with_header(7, 8),          // one tetrahedron too many
        with_header(6, 9),          // one node too many
        with_header(42_949_673, 8), // 100 * t = 2^32 + 4: a u32 size sum would wrap to 100 bytes
    ];
    for bytes in &cases {
        let base = common::reset_peak();
        let r = mbin::read(bytes);
        let peak = common::peak_since_reset(base);
        assert!(
            matches!(r, Err(FormatError::Truncated { .. })),
            "header {:?}: {r:?}",
            &bytes[..8]
        );
        assert!(
            peak <= common::budget(bytes.len()),
            "header {:?}: peak {peak}",
            &bytes[..8]
        );
    }
    // Fewer than the file holds is legal: upstream reads the prefix and ignores the rest.
    let fewer = mbin::read(&with_header(5, 8)).unwrap();
    assert_eq!(fewer.tetrahedra.len(), 5);
}

#[test]
fn validate_rejects_what_the_solver_cannot_index() {
    let _g = serial();
    let good = load("upstream/lib_interface/cube_mesh.mbin");
    let mutate = |f: &dyn Fn(&mut Mesh)| {
        let mut m = good.clone();
        f(&mut m);
        mbin::validate(&m)
    };
    assert!(mutate(&|_| {}).is_ok());
    assert!(matches!(
        mutate(&|m| m.tetrahedra[0].vertices[1] = 8),
        Err(FormatError::Invalid(_))
    ));
    assert!(matches!(
        mutate(&|m| m.tetrahedra[0].vertices[1] = -1),
        Err(FormatError::Invalid(_))
    ));
    assert!(matches!(
        mutate(&|m| m.tetrahedra[0].vertices[1] = 4),
        Err(FormatError::Invalid(_))
    ));
    assert!(matches!(
        mutate(&|m| m.tetrahedra[2].faces[3].vertices[0] = 99),
        Err(FormatError::Invalid(_))
    ));
    assert!(matches!(
        mutate(&|m| m.tetrahedra[2].faces[3].neighbor = 6),
        Err(FormatError::Invalid(_))
    ));
    assert!(mutate(&|m| m.tetrahedra[2].faces[3].neighbor = -1).is_ok());
    assert!(mutate(&|m| m.tetrahedra[2].faces[3].marker = 12345).is_ok());

    // write_file refuses an invalid mesh and writes nothing.
    let dir = common::scratch::fresh("mbin_golden", "write");
    let path = dir.join("bad.mbin");
    let mut bad = good.clone();
    bad.tetrahedra[0].vertices[1] = bad.tetrahedra[0].vertices[0];
    assert!(matches!(
        mbin::write_file(&bad, &path),
        Err(FormatError::Invalid(_))
    ));
    assert!(!path.exists());
    let ok = dir.join("good.mbin");
    mbin::write_file(&good, &ok).unwrap();
    assert_eq!(mbin::read_file(&ok).unwrap(), good);
}

#[test]
fn generate_is_valid_conforming_and_seed_dependent() {
    let _g = serial();
    let mut dumps = std::collections::HashSet::new();
    for seed in 0..200 {
        let mesh = mbin::generate(seed);
        assert!(!mesh.tetrahedra.is_empty());
        check_upstream_conventions(&mesh);
        assert_eq!(
            mbin::generate(seed),
            mesh,
            "seed {seed} is not deterministic"
        );
        let bytes = mbin::write(&mesh);
        assert_eq!(
            bytes.len(),
            8 + 12 * mesh.nodes.len() + 100 * mesh.tetrahedra.len()
        );
        assert_eq!(mbin::read(&bytes).unwrap(), mesh);
        assert!(
            dumps.insert(mbin::dump(&mesh)),
            "seed {seed} repeats an earlier mesh"
        );
    }
}
