//! The `.mbin` builder against upstream's own. On 2019-06-07 upstream's GUI meshed tutorial 1 and
//! kept both TetGen's output (`tests/fixtures/upstream/tutorial1/tetgen/scene_mesh.1.*`) and the
//! `tetramesh.mbin` it wrote from it (`upstream/tutorial1/spps/tetramesh.mbin`, the same bytes as
//! `tcr/`). `mesh::mesh_from_tetgen` on the first, with the tutorial's own project, gives the
//! second byte for byte, with one documented exception: `idVolume`.
//!
//! **`idVolume`.** Decision 1 of `docs/m5-m6-design.md` writes the room as 0; upstream writes
//! TetGen's region attribute unchanged (`Objet3D_maillage.cpp:165, 868`). The evidence for how
//! the comparison treats it: every one of the fixture's 2,257 tetrahedra has `idVolume` 1, and
//! every row of its `.ele` has attribute 1, the one region TetGen found. So the fixture is
//! compared with those fields mapped 1 -> 0, the map decision 1 prescribes, rather than with a
//! builder switched to upstream's `room: 1`: the comparison then covers the code path every
//! mesh takes, and the test also checks that those 2,257 fields are the **only** bytes in which
//! the two files differ.
//!
//! **Each of the builder's two upstream conversions is needed.** The builder's output with one
//! of them undone, and nothing else changed, fails the comparison:
//! - TetGen's own corner order (the reversal undone, anchored to the `.ele` rows): all 2,257
//!   tetrahedron records differ;
//! - no GL round trip (the nodes as the `.node` gives them, narrowed to `f32`): 230 of the 732
//!   nodes differ, 198 of them in value by up to 4.77e-7 m, the rest in the sign of a zero.
//!
//! Neither of those meshes breaks an invariant `mesh::verify` checks: only the byte comparison can
//! tell them from upstream's. Those two tests show that the comparison **can say no**: each
//! undoes one conversion itself, anchored to TetGen's own files (the `.ele` rows, the `.node`
//! values), and checks the rest against upstream's bytes. So neither can see its own conversion
//! missing from the builder (with the round trip taken out of `build.rs`, the round-trip test
//! still passes); what holds the builder to upstream is the byte-identity test, which fails when
//! either conversion is taken out (at tetrahedron 0, field 0, and at node 9, coordinate 1). The
//! tutorial-1 tests need nothing outside the repository.
//!
//! **The frame.** The round trip's `UnitizeVar` is fitted to a list of scene vertices, the last
//! one left out (`mesh::Unitize`). Upstream's list is the one its GUI wrote into the run's
//! `.cbin`: tutorial 1's has 36 vertices, a copy per face corner, where ours has the 8 points. Both
//! fit `(3, 1.5, -5, 0.2)`, and so do tutorial 3's two; a frame one `f32` step off, or fitted in
//! the scene's axes rather than the GUI's, fails the comparison. No upstream file here has a last
//! vertex that decides the frame, so that rule is held to `Objet3D.cpp:542` by `build.rs`'s unit
//! tests only.
//!
//! **From the project, through TetGen 1.5.0.** Tutorial 1's project meshed by the TetGen of the
//! solver build (`$SIMPA_SOLVERS_DIR`), which is 1.5.0 by Burhan's decision ("Our own build",
//! `docs/investigations/2026-09-23-upstream-meshing/DECISIONS.md`), gives TetGen's 2019 files
//! and then upstream's 2019 `.mbin`, byte for byte (`idVolume` as above). Against a TetGen 1.6.0
//! build that test fails, as it should: 1.6.0 meshes the box into 6 tetrahedra.
//!
//! **The code under test is this tree's.** The runs of this project share one cargo build folder
//! between checkouts, and cargo judges a path package up to date by file times alone, so it can
//! link another checkout's `simpa-core`. The first test compares the folder the library was
//! compiled from (`mesh::COMPILED_FROM`) with the package cargo runs the tests for.
//!
//! **A second, independent case: tutorial 3**, read from the upstream checkout
//! (`$SIMPA_UPSTREAM`, `tests/common/paths.rs`; missing, the test fails, it never skips). A scene
//! of 40 vertices and a fitting zone in five TetGen regions (upstream's ids 1930 and 2083-2086;
//! the fitting zone is why `import_proj` refuses the project), meshed on 2019-06-18 into 3,285
//! tetrahedra: `build_mbin` gives its `.mbin` byte for byte, told upstream's own ids, with the
//! frame fitted to the scene part of upstream's `.cbin`, which is not a box of integers.

#[path = "mesh_support.rs"]
mod support;

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use simpa_core::formats::{cbin, mbin, tetgen};
use simpa_core::geometry::import::zip::Archive;
use simpa_core::mesh::verify::{VolumeIds, verify_mesh};
use simpa_core::mesh::{
    self, FACE_CORNERS, MeshManifest, MeshStatus, TetgenMesher, UPSTREAM_CORNERS, Unitize,
    mesh_from_tetgen, mesh_project, sha256_hex,
};
use simpa_core::process::{CancelToken, Line};
use support::{fixture, invariants, load_room, scratch, tetgen_exe, upstream_file};

/// The TetGen output set upstream meshed tutorial 1 with.
const TETGEN_DIR: &str = "upstream/tutorial1/tetgen";
/// The `.mbin` upstream's GUI wrote from it.
const UPSTREAM_MBIN: &str = "upstream/tutorial1/spps/tetramesh.mbin";
/// Its sha256 (`tests/fixtures/upstream/tutorial1/PROVENANCE.md`).
const UPSTREAM_SHA256: &str = "8a6b3943dd47126bb875d0c92979a7b1f8c54d7a7f277c8d5cfcb52d6aca00dc";
/// Its counts.
const TETS: usize = 2257;
const NODES: usize = 732;

/// The tutorial's TetGen output built by `mesh_from_tetgen` against the tutorial's project, once:
/// the output folder and its manifest.
fn built() -> &'static (PathBuf, MeshManifest) {
    static BUILT: OnceLock<(PathBuf, MeshManifest)> = OnceLock::new();
    BUILT.get_or_init(|| {
        let out = scratch("mbin-parity");
        let p = load_room("tutorial1_box.simpa");
        let m = mesh_from_tetgen(&p, &fixture(TETGEN_DIR), None, &out).unwrap();
        (out, m)
    })
}

fn ours_bytes() -> Vec<u8> {
    let (out, m) = built();
    assert_eq!(m.status, MeshStatus::Ok, "{m:#?}");
    std::fs::read(out.join("tetramesh.mbin")).unwrap()
}

fn ours() -> mbin::Mesh {
    mbin::read(&ours_bytes()).unwrap()
}

fn upstream_bytes() -> Vec<u8> {
    std::fs::read(fixture(UPSTREAM_MBIN)).unwrap()
}

/// Byte offset of tetrahedron `t`'s `idVolume` in a `.mbin` of `nodes` nodes
/// (`docs/formats/mbin.md`, byte layout).
fn id_volume_offset(nodes: usize, t: usize) -> usize {
    8 + 12 * nodes + 100 * t + 16
}

/// Upstream's bytes with each room `idVolume`, 1, written as decision 1 writes it, 0. Panics if a
/// tetrahedron carries anything else: then the map would be a guess.
fn with_room_as_zero(upstream: &[u8]) -> Vec<u8> {
    let mut expected = upstream.to_vec();
    for t in 0..TETS {
        let at = id_volume_offset(NODES, t);
        let field: [u8; 4] = expected[at..at + 4].try_into().unwrap();
        assert_eq!(i32::from_le_bytes(field), 1, "tetrahedron {t}'s idVolume");
        expected[at..at + 4].copy_from_slice(&0i32.to_le_bytes());
    }
    expected
}

/// Where two `.mbin` files first differ, in words.
fn first_difference(a: &[u8], b: &[u8], nodes: usize) -> Option<String> {
    if a.len() != b.len() {
        return Some(format!("lengths {} and {}", a.len(), b.len()));
    }
    let at = a.iter().zip(b).position(|(x, y)| x != y)?;
    let body = 8 + 12 * nodes;
    Some(if at < 8 {
        format!("header byte {at}")
    } else if at < body {
        format!("node {} coordinate {}", (at - 8) / 12, (at - 8) % 12 / 4)
    } else {
        let t = (at - body) / 100;
        let field = (at - body) % 100 / 4;
        format!("tetrahedron {t}, i32 field {field} of 25 (byte {at})")
    })
}

/// `mesh` with its corners permuted: new corner `k` is old corner `perm[k]`. Faces are rebuilt
/// from the new corners in [`FACE_CORNERS`] order; each keeps the marker and neighbour of the face
/// opposite the same node. This is what the builder writes when it takes a `.ele` row in the
/// order `perm` gives it.
fn permuted(mesh: &mbin::Mesh, perm: [usize; 4]) -> mbin::Mesh {
    let mut out = mesh.clone();
    for t in &mut out.tetrahedra {
        let old = *t;
        t.vertices = perm.map(|k| old.vertices[k]);
        t.faces = std::array::from_fn(|k| mbin::TetraFace {
            vertices: FACE_CORNERS[k].map(|c| t.vertices[c]),
            ..old.faces[perm[k]]
        });
    }
    out
}

/// The tutorial's `.node` points, narrowed to `f32` with nothing else done to them.
fn plain_nodes() -> Vec<[f32; 3]> {
    let path = fixture(&format!("{TETGEN_DIR}/scene_mesh.1.node"));
    match tetgen::read_file(&path).unwrap() {
        tetgen::TetgenFile::Node(n) => n.points.iter().map(|p| p.map(|c| c as f32)).collect(),
        other => panic!("{other:?}"),
    }
}

/// The scene part of a `.cbin` upstream's GUI wrote, `_pVertices` verbatim: every vertex before
/// the fitting zones' triangles, which `ToCBINFormat` appends last, each with three fresh
/// vertices, `idMat` 0, `idRs` -1 and the zone's `idEn` (`Objet3D_maillage.cpp:775-815`).
fn upstream_scene_vertices(model: &cbin::Model) -> Vec<[f32; 3]> {
    let mut n = model.vertices.len();
    for f in model.faces.iter().rev() {
        let fresh = n >= 3 && [f.a, f.b, f.c] == [n - 3, n - 2, n - 1].map(|i| i as u32);
        if !(fresh && f.id_mat == 0 && f.id_rs == -1 && f.id_en != -1) {
            break;
        }
        n -= 3;
    }
    model.vertices[..n]
        .iter()
        .map(|v| [v.x, v.y, v.z])
        .collect()
}

/// A list of vertices as a set of points, a zero's sign ignored.
fn points(vertices: &[[f32; 3]]) -> BTreeSet<[u32; 3]> {
    vertices
        .iter()
        .map(|v| v.map(|c| (c + 0.0).to_bits()))
        .collect()
}

/// A TetGen output file's rows with its `#` lines left out (the trailer names the command line
/// and the folder TetGen ran in), its columns single-spaced, and each `-0` read as `0`.
fn tetgen_rows(path: &Path) -> Vec<String> {
    std::fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("{}: {e}", path.display()))
        .lines()
        .filter(|l| !l.trim_start().starts_with('#'))
        .map(|l| {
            l.split_whitespace()
                .map(|t| if t == "-0" { "0" } else { t })
                .collect::<Vec<_>>()
                .join(" ")
        })
        .collect()
}

/// `Err`, naming both, unless `compiled_from`, the package folder some code was compiled from, is
/// `running_for`, the package cargo runs the tests for.
fn compiled_from_this_package(
    what: &str,
    compiled_from: &Path,
    running_for: &Path,
) -> Result<(), String> {
    if compiled_from == running_for {
        Ok(())
    } else {
        Err(format!(
            "{what} was compiled from {}, but cargo runs the tests of {}: the shared build \
             folder handed this tree another checkout's build. Touch this tree's sources (their \
             content unchanged) and run again",
            compiled_from.display(),
            running_for.display()
        ))
    }
}

#[test]
fn the_builder_under_test_was_compiled_from_this_tree() {
    let running_for = PathBuf::from(
        std::env::var_os("CARGO_MANIFEST_DIR").expect("cargo test sets CARGO_MANIFEST_DIR"),
    );
    compiled_from_this_package("simpa-core", Path::new(mesh::COMPILED_FROM), &running_for).unwrap();
    compiled_from_this_package(
        "this test",
        Path::new(env!("CARGO_MANIFEST_DIR")),
        &running_for,
    )
    .unwrap();
    // The input that makes it say no: another checkout's package folder.
    let elsewhere = running_for
        .parent()
        .unwrap()
        .join("../../other-checkout/crates/simpa-core");
    let e = compiled_from_this_package("simpa-core", &elsewhere, &running_for).unwrap_err();
    assert!(e.contains("other-checkout"), "{e}");
}

#[test]
fn the_2019_tutorial_mesh_is_rebuilt_byte_for_byte() {
    let (out, m) = built();
    assert!(m.is_ok(), "{m:#?}");
    let upstream = upstream_bytes();
    assert_eq!(sha256_hex(&upstream), UPSTREAM_SHA256);
    assert_eq!(
        upstream,
        std::fs::read(fixture("upstream/tutorial1/tcr/tetramesh.mbin")).unwrap(),
        "upstream's SPPS and TCR runs got the same .mbin"
    );
    let ours = ours_bytes();
    assert_eq!(m.files.mbin.as_deref(), Some(sha256_hex(&ours).as_str()));

    let expected = with_room_as_zero(&upstream);
    assert!(
        ours == expected,
        "the rebuilt .mbin differs from upstream's (room mapped to 0) at {}",
        first_difference(&ours, &expected, NODES).unwrap()
    );

    // The only bytes in which ours and upstream's raw file differ: the low byte of each
    // tetrahedron's idVolume.
    let differing: Vec<usize> = ours
        .iter()
        .zip(&upstream)
        .enumerate()
        .filter(|(_, (a, b))| a != b)
        .map(|(i, _)| i)
        .collect();
    let id_volumes: Vec<usize> = (0..TETS).map(|t| id_volume_offset(NODES, t)).collect();
    assert_eq!(differing, id_volumes);

    // The builder saw one region, attribute 1, and wrote it as the room.
    let stats = m.counts.build.as_ref().unwrap();
    let attrs: Vec<(i64, i32, usize)> = stats
        .attributes
        .iter()
        .map(|a| (a.attribute, a.id_volume, a.tetrahedra))
        .collect();
    assert_eq!(attrs, [(1, 0, TETS)]);
    assert_eq!((stats.nodes, stats.tetrahedra), (NODES, TETS));
    // The TetGen call is the one the .face trailer records.
    let call = m.tetgen.as_ref().unwrap();
    assert_eq!(&call.argv[..3], ["-pq2", "-A", "-n"]);
    assert!(out.join("mesh.cbin").is_file());
}

#[test]
fn the_frame_fitted_to_our_list_is_the_one_upstreams_list_gives() {
    // Upstream's list, as its run's .cbin carries it: a copy of each vertex per face corner.
    let theirs_model = cbin::read_file(&fixture("upstream/tutorial1/spps/mesh.cbin")).unwrap();
    let theirs = upstream_scene_vertices(&theirs_model);
    assert_eq!((theirs.len(), theirs_model.faces.len()), (36, 12));
    for (f, face) in theirs_model.faces.iter().enumerate() {
        let k = 3 * f as u32;
        assert_eq!([face.a, face.b, face.c], [k, k + 1, k + 2], "face {f}");
    }
    let upstream = Unitize::fit(&theirs).unwrap();
    assert_eq!(upstream.centre, [3.0, 1.5, -5.0]);
    assert_eq!(upstream.scale.to_bits(), 0.2f32.to_bits());

    // Ours, the .cbin the build wrote and the project the run meshes: the same 8 points, once
    // each, and the same frame.
    let ours_model = cbin::read_file(&built().0.join("mesh.cbin")).unwrap();
    let ours: Vec<[f32; 3]> = ours_model
        .vertices
        .iter()
        .map(|v| [v.x, v.y, v.z])
        .collect();
    assert_eq!(ours.len(), 8);
    assert_eq!(points(&ours), points(&theirs));
    assert_eq!(Unitize::of_scene(&ours_model).unwrap(), upstream);
    let p = load_room("tutorial1_box.simpa");
    let scene = mesh::project_input(&p).unwrap().scene;
    assert_eq!(scene, ours_model);

    // Why the last vertex left out decides neither frame here. In upstream's list every point
    // appears 3 to 6 times, so its last copy is not missed; in ours, as in any box, no vertex is
    // alone at a minimum or maximum, so whichever comes last, the frame is the same.
    let mut copies: BTreeMap<[u32; 3], usize> = BTreeMap::new();
    for p in points(&theirs) {
        copies.insert(p, 0);
    }
    for v in &theirs {
        *copies.get_mut(&v.map(|c| (c + 0.0).to_bits())).unwrap() += 1;
    }
    assert!(copies.values().all(|&n| (3..=6).contains(&n)), "{copies:?}");
    for k in 0..ours.len() {
        let mut rotated = ours.clone();
        rotated.rotate_left(k);
        assert_eq!(Unitize::fit(&rotated).unwrap(), upstream, "vertex {k} last");
    }
}

#[test]
fn without_the_corner_reversal_the_comparison_fails() {
    let ours = ours();
    let tetgen_order = permuted(&ours, UPSTREAM_CORNERS);

    // Anchor: that is TetGen's order exactly, every .ele row minus its base of 1.
    let ele = match tetgen::read_file(&fixture(&format!("{TETGEN_DIR}/scene_mesh.1.ele"))) {
        Ok(tetgen::TetgenFile::Ele(e)) => e,
        other => panic!("{other:?}"),
    };
    assert_eq!(ele.len(), TETS);
    for (t, tet) in tetgen_order.tetrahedra.iter().enumerate() {
        let row: Vec<i32> = ele.tet(t).iter().map(|v| v - 1).collect();
        assert_eq!(tet.vertices.as_slice(), row.as_slice(), "tetrahedron {t}");
    }
    // A valid mesh all the same: the invariants cannot tell the two orders apart.
    assert_eq!(invariants(&tetgen_order), Vec::<String>::new());

    let bytes = mbin::write(&tetgen_order);
    let expected = with_room_as_zero(&upstream_bytes());
    assert_ne!(bytes, expected);
    let body = 8 + 12 * NODES;
    assert_eq!(bytes[..body], expected[..body], "the nodes are untouched");
    let records_differing = bytes[body..]
        .chunks(100)
        .zip(expected[body..].chunks(100))
        .filter(|(a, b)| a != b)
        .count();
    assert_eq!(records_differing, TETS, "every tetrahedron record differs");
    let fields_differing = bytes[body..]
        .chunks(4)
        .zip(expected[body..].chunks(4))
        .filter(|(a, b)| a != b)
        .count();
    assert_eq!(fields_differing, 47_524, "of {} i32 fields", 25 * TETS);
    // The reversal is its own inverse: applied again, it gives back upstream's bytes.
    assert_eq!(
        mbin::write(&permuted(&tetgen_order, UPSTREAM_CORNERS)),
        expected
    );
}

#[test]
fn without_the_round_trip_the_comparison_fails() {
    let ours = ours();
    let plain = plain_nodes();
    assert_eq!(plain.len(), NODES);
    let mut unconverted = ours.clone();
    unconverted.nodes = plain;
    assert_eq!(invariants(&unconverted), Vec::<String>::new());

    let bytes = mbin::write(&unconverted);
    let expected = with_room_as_zero(&upstream_bytes());
    assert_ne!(bytes, expected);
    let body = 8 + 12 * NODES;
    assert_eq!(
        bytes[body..],
        expected[body..],
        "the tetrahedra are untouched"
    );

    // Measured: 230 nodes (268 coordinates) differ in their bits, 198 of them (229 coordinates)
    // in value, by at most 2^-21 = 4.77e-7 m; the other 32 nodes differ only in a zero's sign
    // (a 0 in y comes back from the round trip as -0.0).
    let upstream = mbin::read(&expected).unwrap();
    let (mut bit_nodes, mut bit_coords, mut value_nodes, mut value_coords) = (0, 0, 0, 0);
    let mut largest = 0.0f64;
    for (p, q) in unconverted.nodes.iter().zip(&upstream.nodes) {
        let bits = (0..3).filter(|&k| p[k].to_bits() != q[k].to_bits()).count();
        let values = (0..3).filter(|&k| p[k] != q[k]).count();
        bit_nodes += usize::from(bits > 0);
        bit_coords += bits;
        value_nodes += usize::from(values > 0);
        value_coords += values;
        for k in 0..3 {
            largest = largest.max((f64::from(p[k]) - f64::from(q[k])).abs());
        }
    }
    assert_eq!((bit_nodes, bit_coords), (230, 268));
    assert_eq!((value_nodes, value_coords), (198, 229));
    assert_eq!(largest, 2f64.powi(-21));
}

#[test]
fn a_frame_one_ulp_off_or_in_the_scenes_axes_fails_the_comparison() {
    // The frame is part of the result: the scale one f32 step above upstream's 0.2 moves nodes,
    // and so does the box's centre taken in the scene's axes, (3, 5, 1.5), not the GUI's
    // (x, z, -y).
    let out = mesh::TetgenOutput::read(&mesh::OutputPaths::new(&fixture(TETGEN_DIR), "scene_mesh"))
        .unwrap();
    let expected = with_room_as_zero(&upstream_bytes());
    let right = Unitize {
        centre: [3.0, 1.5, -5.0],
        scale: 0.2,
    };
    let (mesh, _) = mesh::build_mbin(&out, 12, &[], &right).unwrap();
    assert!(mbin::write(&mesh) == expected, "control: upstream's frame");
    let off = Unitize {
        scale: f32::from_bits(0.2f32.to_bits() + 1),
        ..right
    };
    let (mesh, _) = mesh::build_mbin(&out, 12, &[], &off).unwrap();
    let bytes = mbin::write(&mesh);
    assert_ne!(bytes, expected);
    println!(
        "scale one ulp off: first difference at {}",
        first_difference(&bytes, &expected, NODES).unwrap()
    );
    let scene_axes = Unitize {
        centre: [3.0, 5.0, 1.5],
        ..right
    };
    let (mesh, _) = mesh::build_mbin(&out, 12, &[], &scene_axes).unwrap();
    let bytes = mbin::write(&mesh);
    assert_ne!(bytes, expected);
    println!(
        "centre in the scene's axes: first difference at {}",
        first_difference(&bytes, &expected, NODES).unwrap()
    );
}

#[test]
fn the_tutorial_project_meshed_by_tetgen_150_gives_upstreams_2019_mbin() {
    let exe = tetgen_exe();
    let dir = scratch("tutorial1-tetgen150");
    let p = load_room("tutorial1_box.simpa");
    let m = mesh_project(
        &p,
        &dir,
        &TetgenMesher::new(exe.clone()),
        &CancelToken::new(),
        &mut |_: &Line| {},
    )
    .unwrap();
    assert!(m.is_ok(), "{m:#?}");

    // TetGen's output is its 2019 output, the trailer apart. So is the .node, but for the sign
    // of the zeros in y: our .poly writes the box's y = 0 as 0, upstream's as -0 (its vertices
    // come back from the GL frame, `_SavePOLY`, Objet3D_maillage.cpp:941-942), and TetGen copies
    // what it reads. The round trip writes both as -0.0 in the .mbin.
    for ext in ["ele", "face", "neigh", "node"] {
        let name = format!("scene_mesh.1.{ext}");
        let ours = tetgen_rows(&dir.join(&name));
        let theirs = tetgen_rows(&fixture(&format!("{TETGEN_DIR}/{name}")));
        assert!(
            ours == theirs,
            "{}: TetGen's .{ext} is not upstream's 2019 one ({} rows against {}; header {:?} \
             against {:?}). The mesher is TetGen 1.5.0 (DECISIONS.md, 'Our own build'); a 1.6.0 \
             build meshes this box into 6 tetrahedra",
            exe.display(),
            ours.len(),
            theirs.len(),
            ours.first(),
            theirs.first()
        );
    }

    let bytes = std::fs::read(dir.join("tetramesh.mbin")).unwrap();
    let expected = with_room_as_zero(&upstream_bytes());
    assert!(
        bytes == expected,
        "the .mbin meshed from the project differs from upstream's (room mapped to 0) at {}",
        first_difference(&bytes, &expected, NODES).unwrap()
    );
}

#[test]
fn the_invariants_hold_in_upstreams_corner_order() {
    let (out, _) = built();
    let ours = ours();
    let scene = cbin::read_file(&out.join("mesh.cbin")).unwrap();

    // The verifier and the test-side checker both pass the rebuilt mesh: orientation, face i
    // opposite corner i in FACE_CORNERS' winding, mutual neighbours, marked hull.
    let report = verify_mesh(&ours, &scene, &VolumeIds::default());
    assert!(report.passed(), "{report:#?}");
    assert_eq!(invariants(&ours), Vec::<String>::new());
    // So does upstream's own file, with its room id and against its own .cbin.
    let upstream = mbin::read(&upstream_bytes()).unwrap();
    let upstream_scene = cbin::read_file(&fixture("upstream/tutorial1/spps/mesh.cbin")).unwrap();
    let ids = VolumeIds {
        room: 1,
        fittings: Vec::new(),
    };
    let report = verify_mesh(&upstream, &upstream_scene, &ids);
    assert!(report.passed(), "{report:#?}");

    // The orientation sign, exactly (Shewchuk's orient3d), in both corner orders: (d,c,b,a) is
    // an even permutation of (a,b,c,d), so every tetrahedron keeps the .mbin's negative sign.
    let sign = |mesh: &mbin::Mesh, t: &mbin::Tetrahedron| {
        let [a, b, c, d] = t.vertices.map(|v| {
            let p = mesh.nodes[v as usize].map(f64::from);
            robust::Coord3D {
                x: p[0],
                y: p[1],
                z: p[2],
            }
        });
        let o = robust::orient3d(a, b, c, d);
        i8::from(o > 0.0) - i8::from(o < 0.0)
    };
    let tetgen_order = permuted(&ours, UPSTREAM_CORNERS);
    for (t, (u, g)) in ours
        .tetrahedra
        .iter()
        .zip(&tetgen_order.tetrahedra)
        .enumerate()
    {
        assert_eq!(sign(&ours, u), -1, "tetrahedron {t}, upstream's order");
        assert_eq!(
            sign(&tetgen_order, g),
            -1,
            "tetrahedron {t}, TetGen's order"
        );
    }

    // One swap is an odd permutation: every sign flips, and the verifier says so for every
    // tetrahedron. Its faces are rebuilt from the swapped corners, so orientation is the only
    // thing it can object to.
    let swapped = permuted(&ours, [1, 0, 2, 3]);
    assert!(swapped.tetrahedra.iter().all(|t| sign(&swapped, t) == 1));
    let report = verify_mesh(&swapped, &scene, &VolumeIds::default());
    assert_eq!(report.inverted_tets, TETS, "{report:#?}");
    assert_eq!(report.codes, ["inverted_tets"]);
}

/// Upstream's tutorial 3 in the pinned checkout.
const TUTORIAL3: &str = "src/isimpa/resources/doc/tutorial/tutorial 3/tutorial_3.proj";
/// Its last SPPS run; the two before it hold the same `.mbin` and `.cbin`.
const TUTORIAL3_RUNS: [&str; 3] = [
    "instance1/report/SPPS/2019-06-18_14h27m51s/",
    "instance1/report/SPPS/2019-06-18_14h28m18s/",
    "instance1/report/SPPS/2019-06-18_14h31m31s/",
];

#[test]
fn tutorial_3s_mesh_is_rebuilt_byte_for_byte_from_the_upstream_checkout() {
    let proj = std::fs::read(upstream_file(TUTORIAL3)).unwrap();
    let zip = Archive::parse(&proj).unwrap();
    let dir = scratch("tutorial3");
    for ext in ["node", "ele", "face", "neigh"] {
        let member = format!("instance1/temp/scene_mesh.1.{ext}");
        std::fs::write(
            dir.join(format!("scene_mesh.1.{ext}")),
            zip.read(&member).unwrap(),
        )
        .unwrap();
    }
    let expected = zip
        .read(&format!("{}tetramesh.mbin", TUTORIAL3_RUNS[2]))
        .unwrap();
    for run in &TUTORIAL3_RUNS[..2] {
        assert_eq!(zip.read(&format!("{run}tetramesh.mbin")).unwrap(), expected);
    }
    let scene = cbin::read(
        &zip.read(&format!("{}mesh.cbin", TUTORIAL3_RUNS[2]))
            .unwrap(),
    )
    .unwrap();

    // The frame, fitted to upstream's list as its .cbin carries it: 40 scene vertices, one copy of
    // each point, before the fitting zone's 12 triangles. (9.548741, 5, -4) and 0.10472585,
    // nothing like tutorial 1's round numbers.
    let vertices = upstream_scene_vertices(&scene);
    assert_eq!((vertices.len(), scene.vertices.len()), (40, 76));
    assert_eq!(points(&vertices).len(), 40);
    let unitize = Unitize::fit(&vertices).unwrap();
    assert_eq!(unitize.centre[1..], [5.0, -4.0]);
    assert!((unitize.centre[0] - 9.548_741).abs() < 1e-5, "{unitize:?}");
    assert!((unitize.scale - 0.104_725_85).abs() < 1e-7, "{unitize:?}");

    // Upstream's ids unchanged: every region attribute is listed, so none is written as 0.
    let out = mesh::TetgenOutput::read(&mesh::OutputPaths::new(&dir, "scene_mesh")).unwrap();
    let attributes: BTreeSet<i32> = (0..out.ele.len())
        .map(|t| *out.ele.tet_attributes(t).last().unwrap() as i32)
        .collect();
    let ids: Vec<i32> = attributes.into_iter().collect();
    assert_eq!(ids, [1930, 2083, 2084, 2085, 2086]);
    let (built, stats) = mesh::build_mbin(&out, scene.faces.len(), &ids, &unitize).unwrap();
    assert_eq!((stats.tetrahedra, stats.nodes), (3285, 835));
    assert_eq!(stats.zone_tet_faces, 0, "every marker indexes a .cbin face");
    let bytes = mbin::write(&built);
    assert!(
        bytes == expected,
        "tutorial 3's rebuilt .mbin differs from upstream's at {}",
        first_difference(&bytes, &expected, 835).unwrap()
    );

    // And it says no to each conversion undone.
    let tetgen_order = mbin::write(&permuted(&built, UPSTREAM_CORNERS));
    assert_ne!(tetgen_order, expected);
    let mut unconverted = built.clone();
    unconverted.nodes = out
        .node
        .points
        .iter()
        .map(|p| p.map(|c| c as f32))
        .collect();
    assert_ne!(mbin::write(&unconverted), expected);
}
