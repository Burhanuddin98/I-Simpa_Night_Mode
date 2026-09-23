//! `core::geometry::import`'s mesh readers on the hand-made fixtures in
//! `tests/fixtures/geometry/` (written by `make_fixtures.py` there): M4 gate item (f), and the
//! Night Mode reader defects each fix answers. Needs nothing outside the repo, except the last
//! test, which reads upstream's raw `elmia.ply` from the upstream source tree (`common/paths.rs`)
//! and panics when that tree is absent.

use std::path::{Path, PathBuf};

use simpa_core::geometry::import::{
    ImportError, ImportOptions, ImportedModel, MeshFormat, ObjGroups, Unit, Up, Weld, import_file,
    read_obj, read_ply, read_stl,
};
use simpa_core::schema::Vec3;

#[allow(dead_code)]
#[path = "common/paths.rs"]
mod paths;

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/geometry")
        .join(name)
}

fn bytes(name: &str) -> Vec<u8> {
    std::fs::read(fixture(name)).unwrap()
}

fn metres_z_up() -> ImportOptions {
    ImportOptions::new(Unit::Metre, Up::Z)
}

fn bbox(m: &ImportedModel) -> ([f64; 3], [f64; 3]) {
    let mut lo = [f64::INFINITY; 3];
    let mut hi = [f64::NEG_INFINITY; 3];
    for v in &m.vertices {
        for (k, c) in v.to_array().into_iter().enumerate() {
            lo[k] = lo[k].min(c);
            hi[k] = hi[k].max(c);
        }
    }
    (lo, hi)
}

fn corners(m: &ImportedModel, t: [u32; 3]) -> [[f64; 3]; 3] {
    t.map(|i| m.vertices[i as usize].to_array())
}

fn signed_volume(m: &ImportedModel) -> f64 {
    m.triangles
        .iter()
        .map(|&t| {
            let [a, b, c] = corners(m, t);
            (a[0] * (b[1] * c[2] - b[2] * c[1]) - a[1] * (b[0] * c[2] - b[2] * c[0])
                + a[2] * (b[0] * c[1] - b[1] * c[0]))
                / 6.0
        })
        .sum()
}

fn area(m: &ImportedModel) -> f64 {
    m.triangles
        .iter()
        .map(|&t| {
            let [a, b, c] = corners(m, t);
            let u = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
            let v = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
            let n = [
                u[1] * v[2] - u[2] * v[1],
                u[2] * v[0] - u[0] * v[2],
                u[0] * v[1] - u[1] * v[0],
            ];
            0.5 * (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt()
        })
        .sum()
}

/// Every edge of a closed 2-manifold is used exactly twice, once each way.
fn assert_closed_and_consistently_wound(m: &ImportedModel) {
    let mut directed = std::collections::HashMap::new();
    for t in &m.triangles {
        for k in 0..3 {
            *directed.entry((t[k], t[(k + 1) % 3])).or_insert(0) += 1;
        }
    }
    for (&(a, b), &n) in &directed {
        assert_eq!(n, 1, "edge {a}->{b} used {n} times the same way");
        assert_eq!(directed.get(&(b, a)), Some(&1), "edge {a}->{b} has no twin");
    }
}

/// Names of the groups of the triangles, in triangle order.
fn group_of_each(m: &ImportedModel) -> Vec<&str> {
    m.face_groups
        .iter()
        .map(|&g| m.group_names[g as usize].as_str())
        .collect()
}

// ---------------------------------------------------------------------------------------------
// STL.

#[test]
fn gate_f_ascii_stl_box_welds_to_8_vertices() {
    let m = read_stl(&bytes("box.stl"), &metres_z_up()).unwrap();
    println!("{:?}", m.report);
    assert_eq!(m.format, MeshFormat::Stl);
    assert_eq!(
        m.report.source_vertices, 36,
        "36 vertex records in the file"
    );
    assert_eq!(m.vertices.len(), 8, "welded to 8 vertices");
    assert_eq!(m.report.welded_vertices, 28);
    assert_eq!(m.triangles.len(), 12);
    assert_eq!(m.group_names, vec!["box".to_string()]);
    assert_closed_and_consistently_wound(&m);
    assert_eq!(signed_volume(&m), 180.0);
    assert_eq!(area(&m), 216.0);
    assert_eq!(bbox(&m), ([0.0, 0.0, 0.0], [6.0, 10.0, 3.0]));
}

#[test]
fn binary_stl_reads_every_triangle_and_equals_its_ascii_twin() {
    // Upstream's binary reader gets every triangle but the first wrong (laydown); ours must give
    // the same mesh as the ASCII file, although its header starts with `solid`.
    let ascii = read_stl(&bytes("box.stl"), &metres_z_up()).unwrap();
    let binary = read_stl(&bytes("box_binary.stl"), &metres_z_up()).unwrap();
    assert!(bytes("box_binary.stl").starts_with(b"solid"));
    assert_eq!(binary.report.notes, vec!["binary STL".to_string()]);
    assert_eq!(binary.vertices, ascii.vertices);
    assert_eq!(binary.triangles, ascii.triangles);
    assert_eq!(binary.face_groups, ascii.face_groups);
    assert_eq!(binary.group_names, vec!["model".to_string()]);
    assert_eq!(binary.report.welded_vertices, 28);
}

#[test]
fn welding_hash_agrees_with_equality() {
    // Two facets whose shared corner is written as 0 and -0 and as 1.0 and 1: equal coordinates,
    // so they must weld; a corner 1e-12 away is a different point and must not.
    let stl = "solid s\n\
        facet normal 0 0 1\n outer loop\n vertex 0 0 0\n vertex 1 0 0\n vertex 0 1 0\n endloop\n endfacet\n\
        facet normal 0 0 1\n outer loop\n vertex -0 1.0 0\n vertex 1 0 -0.0\n vertex 1 1 0\n endloop\n endfacet\n\
        facet normal 0 0 1\n outer loop\n vertex 1.000000000001 1 0\n vertex 2 1 0\n vertex 2 2 0\n endloop\n endfacet\n\
        endsolid s\n";
    let m = read_stl(stl.as_bytes(), &metres_z_up()).unwrap();
    assert_eq!(m.report.source_vertices, 9);
    assert_eq!(m.vertices.len(), 7);
    assert_eq!(m.report.welded_vertices, 2);
    assert_eq!(m.triangles, vec![[0, 1, 2], [2, 1, 3], [4, 5, 6]]);
    // -0.0 is stored as 0.0.
    assert!(
        m.vertices
            .iter()
            .flat_map(|v| v.to_array())
            .all(|c| c.to_bits() != (-0.0f64).to_bits())
    );
}

#[test]
fn welding_is_by_format_unless_asked() {
    // STL welds by default; with welding off it keeps 3 vertices per facet.
    let mut off = metres_z_up();
    off.weld = Weld::Off;
    let soup = read_stl(&bytes("box.stl"), &off).unwrap();
    assert_eq!(soup.vertices.len(), 36);
    assert!(!soup.report.welded);
    // PLY keeps the file's vertices, coincident or not; Weld::Exact merges them.
    let ply = b"ply\nformat ascii 1.0\nelement vertex 6\nproperty float x\nproperty float y\n\
        property float z\nelement face 2\nproperty list uchar int vertex_indices\nend_header\n\
        0 0 0\n1 0 0\n0 1 0\n1 0 0\n0 1 0\n1 1 0\n3 0 1 2\n3 3 5 4\n";
    let kept = read_ply(ply, &metres_z_up()).unwrap();
    assert_eq!(kept.vertices.len(), 6);
    assert_eq!(kept.triangles, vec![[0, 1, 2], [3, 5, 4]]);
    assert!(!kept.report.welded);
    let mut exact = metres_z_up();
    exact.weld = Weld::Exact;
    let welded = read_ply(ply, &exact).unwrap();
    assert_eq!(welded.vertices.len(), 4);
    assert_eq!(welded.report.welded_vertices, 2);
    assert_eq!(welded.triangles, vec![[0, 1, 2], [1, 3, 2]]);
}

#[test]
fn stl_that_is_neither_ascii_nor_binary_is_refused() {
    let mut b = bytes("box_binary.stl");
    b.push(0);
    let e = read_stl(&b, &metres_z_up()).unwrap_err();
    assert_eq!(e.code(), "syntax", "{e}");
    b[0] = b'x';
    let e = read_stl(&b, &metres_z_up()).unwrap_err();
    assert_eq!(e.code(), "invalid", "{e}");
}

// ---------------------------------------------------------------------------------------------
// OBJ.

#[test]
fn gate_f_obj_up_axis_gives_the_specified_bounding_boxes() {
    // The file is modelled Y-up: x 0..6, y 0..3 (height), z 0..10.
    let src = bytes("box_yup.obj");
    let z = read_obj(&src, &ImportOptions::new(Unit::Metre, Up::Z)).unwrap();
    let y = read_obj(&src, &ImportOptions::new(Unit::Metre, Up::Y)).unwrap();
    // --up z keeps the file's axes.
    assert_eq!(bbox(&z), ([0.0, 0.0, 0.0], [6.0, 3.0, 10.0]));
    // --up y maps (x, y, z) to (x, -z, y): 6 wide, 10 deep along -y, 3 high.
    assert_eq!(bbox(&y), ([0.0, -10.0, 0.0], [6.0, 0.0, 3.0]));
    println!("--up z: {:?}\n--up y: {:?}", bbox(&z), bbox(&y));
    // A proper rotation: the winding stays outward, the volume positive.
    assert_eq!(signed_volume(&z), 180.0);
    assert_eq!(signed_volume(&y), 180.0);
    assert_closed_and_consistently_wound(&y);
    // With --up y the file's floor is on z = 0 and its ceiling on z = 3.
    for (&t, &g) in y.triangles.iter().zip(&y.face_groups) {
        let zs = corners(&y, t).map(|p| p[2]);
        match y.group_names[g as usize].as_str() {
            "floor" => assert_eq!(zs, [0.0; 3]),
            "ceiling" => assert_eq!(zs, [3.0; 3]),
            _ => {}
        }
    }
    // Same connectivity and groups either way; only the coordinates turn.
    assert_eq!(y.triangles, z.triangles);
    assert_eq!(y.face_groups, z.face_groups);
    for (a, b) in y.vertices.iter().zip(&z.vertices) {
        let [x, yy, zz] = b.to_array();
        assert_eq!(a.to_array(), [x, -zz + 0.0, yy]);
    }
}

#[test]
fn obj_groups_by_usemtl_by_default_and_by_g_on_request() {
    let src = bytes("box_yup.obj");
    let auto = read_obj(&src, &metres_z_up()).unwrap();
    assert_eq!(auto.group_names, vec!["walls", "floor", "ceiling"]);
    assert_eq!(
        group_of_each(&auto),
        vec![
            "walls", "walls", "walls", "walls", "floor", "floor", "walls", "walls", "ceiling",
            "ceiling", "walls", "walls"
        ]
    );
    assert!(auto.report.notes.iter().any(|n| n.contains("usemtl")));
    let mut by_g = metres_z_up();
    by_g.obj_groups = ObjGroups::Group;
    let g = read_obj(&src, &by_g).unwrap();
    // The whole rest of the line names the group, not only its first word.
    assert_eq!(
        g.group_names,
        vec![
            "side z0", "side z1", "side y0", "side x1", "side y1", "side x0"
        ]
    );
    // Quads (plain, v/vt/vn and relative v//vn references) are fan-split: 6 quads, 12 triangles.
    assert_eq!(g.report.source_faces, 6);
    assert_eq!(g.report.polygons_split, 6);
    assert_eq!(g.triangles.len(), 12);
    assert_eq!(g.vertices.len(), 8);
}

#[test]
fn units_scale_to_metres() {
    let src = bytes("box_yup.obj");
    for (unit, (x, y, z)) in [
        (Unit::Metre, (6.0, 3.0, 10.0)),
        (Unit::Centimetre, (0.06, 0.03, 0.1)),
        (Unit::Millimetre, (0.006, 0.003, 0.01)),
        (Unit::Foot, (6.0 * 0.3048, 3.0 * 0.3048, 10.0 * 0.3048)),
        (Unit::Inch, (6.0 * 0.0254, 3.0 * 0.0254, 10.0 * 0.0254)),
    ] {
        let m = read_obj(&src, &ImportOptions::new(unit, Up::Z)).unwrap();
        assert_eq!(bbox(&m), ([0.0; 3], [x, y, z]), "{}", unit.symbol());
    }
    assert_eq!(Unit::Foot.to_metres(10.0), 3.048);
    assert_eq!(Unit::Inch.to_metres(100.0), 2.54);
}

#[test]
fn obj_references_out_of_range_are_errors_not_dropped_faces() {
    let opts = metres_z_up();
    for (text, code) in [
        ("v 0 0 0\nv 1 0 0\nv 0 1 0\nf 1 2 4\n", "invalid"),
        ("v 0 0 0\nv 1 0 0\nv 0 1 0\nf 0 1 2\n", "invalid"),
        ("v 0 0 0\nv 1 0 0\nv 0 1 0\nf -4 -2 -1\n", "invalid"),
        ("v 0 0 0\nv 1 0 0\nf 1 2\n", "invalid"),
        ("v 0 0 nan\nv 1 0 0\nv 0 1 0\nf 1 2 3\n", "invalid"),
        ("v 0 0\n", "syntax"),
        ("v 0 0 0\n", "empty"),
    ] {
        let e = read_obj(text.as_bytes(), &opts).unwrap_err();
        assert_eq!(e.code(), code, "{text:?}: {e}");
    }
}

// ---------------------------------------------------------------------------------------------
// PLY.

#[test]
fn gate_f_big_endian_ply_equals_its_ascii_twin() {
    let opts = metres_z_up();
    let ascii = read_ply(&bytes("box.ply"), &opts).unwrap();
    let be = read_ply(&bytes("box_be.ply"), &opts).unwrap();
    let le = read_ply(&bytes("box_le.ply"), &opts).unwrap();
    println!("{:?}", ascii.report);
    assert_eq!(be, ascii, "big-endian twin");
    assert_eq!(le, ascii, "little-endian twin");
    // What the twins are: upstream's layers, quads fan-split, float32 read as its decimal.
    assert_eq!(ascii.vertices.len(), 8);
    assert_eq!(ascii.triangles.len(), 12);
    assert_eq!(ascii.report.polygons_split, 6);
    assert_eq!(ascii.group_names, vec!["floor", "ceiling", "walls"]);
    assert_eq!(
        ascii.report.notes,
        vec!["layer 3 `unused layer` holds no face".to_string()]
    );
    assert_eq!(bbox(&ascii), ([-0.35, 0.1, 0.0], [5.65, 10.1, 3.3]));
    assert_closed_and_consistently_wound(&ascii);
    assert!(signed_volume(&ascii) > 0.0);
}

#[test]
fn ply_integer_coordinates_list_count_sizes_and_skipped_elements() {
    // Integer (short, int) and double coordinates, a uchar property between them, ushort list
    // counts, uint indices, a uchar layer_id, a uint-counted layer name and an element to skip:
    // each one a Night Mode reader defect. Big-endian equals ASCII here too.
    let opts = metres_z_up();
    let ascii = read_ply(&bytes("box_mixed.ply"), &opts).unwrap();
    let be = read_ply(&bytes("box_mixed_be.ply"), &opts).unwrap();
    assert_eq!(be, ascii);
    assert_eq!(bbox(&ascii), ([0.0; 3], [6.0, 10.0, 3.0]));
    assert_eq!(ascii.triangles.len(), 12);
    assert_eq!(ascii.report.polygons_split, 0);
    assert_eq!(signed_volume(&ascii), 180.0);
    // The same box as box.stl, triangle for triangle (the vertex order differs: STL has none of
    // its own, so welding numbers the points as the facets first use them).
    let stl = read_stl(&bytes("box.stl"), &opts).unwrap();
    assert_eq!(ascii.triangles.len(), stl.triangles.len());
    for (a, b) in ascii.triangles.iter().zip(&stl.triangles) {
        assert_eq!(corners(&ascii, *a), corners(&stl, *b));
    }
    assert_eq!(
        group_of_each(&ascii),
        vec![
            "floor", "floor", "ceiling", "ceiling", "walls", "walls", "walls", "walls", "walls",
            "walls", "walls", "walls"
        ]
    );
}

#[test]
fn ply_counts_beyond_the_data_are_refused_before_allocating() {
    let opts = metres_z_up();
    let huge = b"ply\nformat binary_little_endian 1.0\nelement vertex 4000000000\n\
        property float x\nproperty float y\nproperty float z\nelement face 1\n\
        property list uchar int vertex_indices\nend_header\n\0\0\0\0";
    assert_eq!(read_ply(huge, &opts).unwrap_err().code(), "truncated");
    let list = b"ply\nformat binary_big_endian 1.0\nelement vertex 3\nproperty uchar x\n\
        property uchar y\nproperty uchar z\nelement face 1\n\
        property list uint int vertex_indices\nend_header\n\0\0\0\x01\0\0\0\x01\0\xff\xff\xff\xff";
    assert_eq!(read_ply(list, &opts).unwrap_err().code(), "truncated");
    let index = b"ply\nformat ascii 1.0\nelement vertex 3\nproperty float x\nproperty float y\n\
        property float z\nelement face 1\nproperty list uchar int vertex_indices\nend_header\n\
        0 0 0\n1 0 0\n0 1 0\n3 0 1 3\n";
    let e = read_ply(index, &opts).unwrap_err();
    assert_eq!(e.code(), "invalid", "{e}");
}

#[test]
fn ply_layers_without_names_are_named_by_id_and_noted() {
    let ply = b"ply\nformat ascii 1.0\nelement vertex 4\nproperty double x\nproperty double y\n\
        property double z\nelement face 2\nproperty list uchar int vertex_indices\n\
        property short layer_id\nelement layer 1\nproperty list uchar uchar layer_name\n\
        end_header\n0 0 0\n1 0 0\n0 1 0\n1 1 0\n3 0 1 2 0\n3 1 3 2 7\n2 65 66\n";
    let m = read_ply(ply, &metres_z_up()).unwrap();
    assert_eq!(m.group_names, vec!["AB", "Layer 7"]);
    assert_eq!(m.face_groups, vec![0, 1]);
    assert_eq!(
        m.report.notes,
        vec!["layer_id 7 has no layer_name; its group is named `Layer 7`".to_string()]
    );
    // Without layer_id, the whole model is one group, `model`, as upstream reads it.
    let plain = b"ply\nformat ascii 1.0\nelement vertex 3\nproperty float x\nproperty float y\n\
        property float z\nelement face 1\nproperty list uchar int vertex_indices\nend_header\n\
        0 0 0\n1 0 0\n0 1 0\n3 0 1 2\n";
    assert_eq!(
        read_ply(plain, &metres_z_up()).unwrap().group_names,
        vec!["model"]
    );
}

#[test]
fn import_file_picks_the_reader_by_extension() {
    let opts = metres_z_up();
    for (name, format) in [
        ("box.stl", MeshFormat::Stl),
        ("box_yup.obj", MeshFormat::Obj),
        ("box_be.ply", MeshFormat::Ply),
    ] {
        let m = import_file(&fixture(name), &opts).unwrap();
        assert_eq!(m.format, format);
        assert_eq!(m.triangles.len(), 12);
    }
    let e = import_file(Path::new("room.3ds"), &opts).unwrap_err();
    assert!(matches!(e, ImportError::UnknownFormat { .. }));
}

#[test]
fn a_model_becomes_a_project_with_one_group_per_file_group() {
    let m = read_ply(&bytes("box.ply"), &metres_z_up()).unwrap();
    let p = m.to_project("box");
    p.check_integrity().unwrap();
    assert_eq!(p.geometry.vertices.len(), 8);
    assert_eq!(p.geometry.faces.len(), 12);
    let names: Vec<&str> = p.surface_groups.iter().map(|g| g.name.as_str()).collect();
    assert_eq!(names, vec!["floor", "ceiling", "walls"]);
    assert_eq!(p.materials.len(), 1);
    assert_eq!(p.materials[0].name, "Default");
    // Deterministic: the same model gives the same project, ids included.
    assert_eq!(
        simpa_core::schema::to_json(&p),
        simpa_core::schema::to_json(&m.to_project("box"))
    );
    assert_eq!(p.geometry.vertices[0], Vec3::new(-0.35, 0.1, 0.0));
}

// ---------------------------------------------------------------------------------------------
// Upstream's raw hall: the layered ASCII PLY upstream ships with tutorial 2.

#[test]
fn upstream_raw_elmia_ply_reads_with_its_layers() {
    let path = paths::upstream_file("src/isimpa/resources/doc/tutorial/tutorial 2/elmia.ply");
    let m = import_file(&path, &metres_z_up()).unwrap();
    println!(
        "elmia.ply: {} vertices ({} unused), {} polygons ({} split), {} triangles, groups {:?}, \
         notes {:?}",
        m.vertices.len(),
        m.report.unused_vertices,
        m.report.source_faces,
        m.report.polygons_split,
        m.triangles.len(),
        m.group_names,
        m.report.notes
    );
    // 955 vertices, 564 polygons: 522 quads and 42 triangles (harvest census).
    assert_eq!(m.report.source_vertices, 955);
    assert_eq!(
        m.vertices.len(),
        955,
        "PLY vertices are kept as the file has them"
    );
    assert_eq!(m.report.source_faces, 564);
    assert_eq!(m.report.polygons_split, 522);
    assert_eq!(m.triangles.len(), 2 * 522 + 42);
    assert_eq!(
        m.group_names,
        vec![
            "ceiling",
            "floor",
            "stairs",
            "interiorwalls",
            "audience",
            "panelwalls",
            "sidereflector",
            "exteriorwalls",
            "reflectors",
            "stage"
        ]
    );
    // The file's connectivity is kept: the harvest's edge census, 955 edges used once, 1,137
    // twice, 7 three times and 2 four times (M4 gate (a)'s open 955 and non-manifold 9).
    assert_eq!(edge_census(&m), vec![(1, 955), (2, 1137), (3, 7), (4, 2)]);
    // Welded, 7 coincident pairs merge and 16 open edges close: which is why import leaves
    // welding to repair.
    let mut exact = metres_z_up();
    exact.weld = Weld::Exact;
    let w = import_file(&path, &exact).unwrap();
    assert_eq!(w.vertices.len(), 948);
    assert_eq!(w.report.welded_vertices, 7);
    assert_eq!(edge_census(&w), vec![(1, 939), (2, 1145), (3, 7), (4, 2)]);
    println!(
        "edge census as the file stands {:?}; welded {:?}",
        edge_census(&m),
        edge_census(&w)
    );
}

/// How many undirected edges are used once, twice, ...: `(uses, edges)`, by uses.
fn edge_census(m: &ImportedModel) -> Vec<(usize, usize)> {
    let mut uses: std::collections::HashMap<(u32, u32), usize> = std::collections::HashMap::new();
    for t in &m.triangles {
        for k in 0..3 {
            let (a, b) = (t[k], t[(k + 1) % 3]);
            *uses.entry((a.min(b), a.max(b))).or_insert(0) += 1;
        }
    }
    let mut census: std::collections::BTreeMap<usize, usize> = std::collections::BTreeMap::new();
    for n in uses.into_values() {
        *census.entry(n).or_insert(0) += 1;
    }
    census.into_iter().collect()
}
