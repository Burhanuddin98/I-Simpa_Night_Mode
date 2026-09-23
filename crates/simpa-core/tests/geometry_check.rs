//! `core::geometry::check` on the M4 gate models and on constructed cases.
//!
//! Gate items (docs/rebuild-plan-raw-2026-09-23.json, milestone M4):
//! - (a) the raw Elmia hall is refused, with the harvest census 955 / 1,137 / 7 / 2;
//! - (b) the corrected hall passes: 11,790 edges each used twice, no self-intersection, positive
//!   volume, in well under a second in release;
//! - (e) two interpenetrating boxes are refused and every intersecting pair is listed;
//! - the critic's fix: a box split by a partition is accepted, its internal faces classified.

#[path = "geometry_check_support.rs"]
mod support;

use std::time::Instant;

use simpa_core::geometry::check::{self, CheckReport, EdgeClass, FaceClass, ReasonCode, Verdict};
use simpa_core::schema::{self, Face, Geometry, GroupId, Vec3};
use support::*;

fn codes(report: &CheckReport) -> Vec<ReasonCode> {
    report.reasons.iter().map(|r| r.code).collect()
}

/// The counts and reasons, without the long per-face lists: the receipt printed by the gates.
fn summary(report: &CheckReport) -> String {
    let reasons: Vec<String> = report
        .reasons
        .iter()
        .map(|r| format!("{} ({}): {}", r.code, r.count, r.message))
        .collect();
    format!(
        "verdict {:?}\ncounts {}\nmeasures {}\ntopology_reliable {}\nreasons:\n  {}",
        report.verdict,
        serde_json::to_string(&report.counts).unwrap(),
        serde_json::to_string(&report.measures).unwrap(),
        report.topology_reliable,
        reasons.join("\n  ")
    )
}

#[test]
fn m4a_raw_elmia_is_refused_with_the_harvest_census() {
    let path = raw_elmia();
    let g = read_ply(&path);
    assert_eq!(
        (g.vertices.len(), g.faces.len()),
        (955, 1086),
        "fan-triangulated 564 polygons"
    );
    let report = check::check(&g);
    eprintln!("raw elmia: {}", summary(&report));
    assert_eq!(report.verdict, Verdict::Refused);
    let c = &report.counts;
    assert_eq!(c.open_edges, 955);
    assert_eq!(c.nonmanifold_edges, 9);
    assert_eq!(c.manifold_edges, 1137);
    assert_eq!(
        c.edge_uses
            .iter()
            .map(|(k, v)| (*k, *v))
            .collect::<Vec<_>>(),
        vec![(1, 955), (2, 1137), (3, 7), (4, 2)]
    );
    assert_eq!(c.edges, 2101);
    // The open shell is the refusal reason the gate is about.
    let open = report
        .reason(ReasonCode::OpenBoundary)
        .expect("open_boundary");
    assert!(open.count > 0 && !open.faces.is_empty());
    // The census lists every open and non-manifold edge with its faces.
    let listed_open = report.edges.iter().filter(|e| e.uses == 1).count();
    let listed_nonmanifold = report.edges.iter().filter(|e| e.uses >= 3).count();
    assert_eq!((listed_open, listed_nonmanifold), (955, 9));
    // 7 exact duplicate vertices (948 unique positions of 955), as the critic measured.
    assert_eq!(c.coincident_vertices, 7);
    // Face 392 is a zero-area (collinear) triangle of a fan-triangulated polygon.
    assert_eq!(report.degenerate_faces.len(), 1);
    assert_eq!(report.degenerate_faces[0].face, 392);
    // The hall also self-intersects. Reference, computed independently in exact rationals
    // (Python `fractions`: separating axes for pairs sharing no position, the corner-cone test
    // for pairs sharing one, the coplanar-fold test for pairs sharing an edge): 1,395 pairs over
    // 896 faces, and the same list (first pairs, last pair and a checksum of the whole list).
    assert_eq!(
        (c.self_intersecting_pairs, c.self_intersecting_faces),
        (1395, 896)
    );
    let pairs = &report.self_intersections;
    assert_eq!(pairs[..3], [[0, 573], [0, 574], [1, 574]]);
    assert_eq!(pairs.last(), Some(&[1047, 1049]));
    let checksum = pairs.iter().fold(0u128, |h, &[i, j]| {
        (h * 1_000_003 + u128::from(i) * 4099 + u128::from(j)) % (1u128 << 61)
    });
    assert_eq!(checksum, 806_807_918_354_733_732);
    let reason = report
        .reason(ReasonCode::SelfIntersections)
        .expect("self_intersections");
    assert_eq!((reason.count, reason.faces.len()), (1395, 896));
}

#[test]
fn m4b_corrected_elmia_passes() {
    let (g, source) = corrected_elmia();
    eprintln!("corrected hall from {source}");
    let start = Instant::now();
    let report = check::check(&g);
    let elapsed = start.elapsed();
    eprintln!("corrected elmia ({elapsed:?}): {}", summary(&report));
    assert_eq!((g.vertices.len(), g.faces.len()), (3926, 7860));
    assert_eq!(report.verdict, Verdict::Ok, "{:?}", report.reasons);
    let c = &report.counts;
    assert_eq!(c.edges, 11_790);
    assert_eq!(c.manifold_edges, 11_790);
    assert_eq!(
        c.edge_uses
            .iter()
            .map(|(k, v)| (*k, *v))
            .collect::<Vec<_>>(),
        vec![(2, 11_790)]
    );
    assert_eq!(c.self_intersecting_pairs, 0);
    assert!(report.self_intersections.is_empty());
    assert!(report.measures.signed_volume_m3 > 0.0);
    assert_eq!(c.boundary_faces, 7860);
    assert_eq!(c.cells, 1);
    assert!(report.topology_reliable);
    assert!(
        (report.measures.area_m2 - 4001.8).abs() < 0.1,
        "area {}",
        report.measures.area_m2
    );
    // One closed shell: its enclosed volume equals the signed volume.
    assert!((report.measures.enclosed_volume_m3 - report.measures.signed_volume_m3).abs() < 1e-6);
    if cfg!(not(debug_assertions)) {
        assert!(elapsed.as_secs_f64() < 1.0, "check took {elapsed:?}");
    }
}

#[test]
fn m4e_interpenetrating_boxes_are_refused_with_their_pairs() {
    let mut g = box_geometry([0.0, 0.0, 0.0], [1.0, 1.0, 1.0], group(1));
    append(
        &mut g,
        &box_geometry([0.5, 0.5, 0.5], [1.5, 1.5, 1.5], group(2)),
    );
    let report = check::check(&g);
    eprintln!("interpenetrating boxes: {}", summary(&report));
    assert_eq!(report.verdict, Verdict::Refused);
    assert!(codes(&report).contains(&ReasonCode::SelfIntersections));
    // Reference: every cross pair, tested with the separating-axis theorem (exact here, since
    // every coordinate is a multiple of 1/2). The boxes share no vertex, so nothing is exempt.
    let mut expected = Vec::new();
    for i in 0..12u32 {
        for j in 12..24u32 {
            if sat_meet(&tri(&g, i), &tri(&g, j)) {
                expected.push([i, j]);
            }
        }
    }
    assert!(!expected.is_empty());
    assert_eq!(report.self_intersections, expected);
    eprintln!("intersecting pairs: {:?}", report.self_intersections);
    let reason = report.reason(ReasonCode::SelfIntersections).unwrap();
    assert_eq!(reason.count, expected.len());
}

#[test]
fn a_box_split_by_a_partition_is_accepted_with_the_partition_classified() {
    let g = partitioned_box();
    let report = check::check(&g);
    eprintln!("partitioned box: {}", summary(&report));
    assert_eq!(report.verdict, Verdict::Ok, "{:?}", report.reasons);
    let c = &report.counts;
    // The four edges where the partition meets the walls are used three times.
    assert_eq!(c.nonmanifold_edges, 4);
    assert_eq!(c.edge_uses.get(&3), Some(&4));
    assert_eq!(c.partition_faces, 2);
    assert_eq!(c.boundary_faces, 20);
    assert_eq!(c.cells, 2);
    let partition: Vec<u32> = report
        .internal_faces
        .iter()
        .filter(|f| f.class == FaceClass::Partition)
        .map(|f| f.face)
        .collect();
    assert_eq!(partition, vec![20, 21]);
    assert!(
        report
            .edges
            .iter()
            .filter(|e| e.uses == 3)
            .all(|e| e.class == EdgeClass::BoundaryJunction)
    );
    let volumes: Vec<f64> = report.cells.iter().map(|c| c.volume_m3).collect();
    assert_eq!(volumes.len(), 2);
    assert!(
        volumes.iter().all(|v| (v - 1.0).abs() < 1e-12),
        "{volumes:?}"
    );
    assert!((report.measures.enclosed_volume_m3 - 2.0).abs() < 1e-12);
    // The partition may face either way.
    let mut flipped = g.clone();
    flipped.faces[20].vertices.swap(1, 2);
    flipped.faces[21].vertices.swap(1, 2);
    assert_eq!(check::check(&flipped).verdict, Verdict::Ok);
}

#[test]
fn a_hanging_reflector_is_accepted_as_a_sheet() {
    let mut g = box_geometry([0.0, 0.0, 0.0], [4.0, 4.0, 4.0], group(1));
    // A single-sided square panel, floating.
    append(
        &mut g,
        &quad(
            [1.0, 1.0, 2.0],
            [3.0, 1.0, 2.0],
            [3.0, 3.0, 2.5],
            [1.0, 3.0, 2.5],
            group(2),
        ),
    );
    let report = check::check(&g);
    eprintln!("hanging reflector: {}", summary(&report));
    assert_eq!(report.verdict, Verdict::Ok, "{:?}", report.reasons);
    assert_eq!(report.counts.sheet_faces, 2);
    assert_eq!(report.counts.open_edges, 4);
    assert_eq!(report.counts.internal_free_edges, 4);
    assert_eq!(report.counts.boundary_open_edges, 0);
    assert!(
        report
            .edges
            .iter()
            .all(|e| e.class == EdgeClass::InternalFree)
    );
    assert_eq!(report.counts.components, 2);
}

#[test]
fn a_shelf_resting_on_a_wall_is_accepted() {
    // A room whose wall x = 0 is split at z = 1 so that a shelf can be attached along that line.
    let v = [
        [0.0, 0.0, 0.0],
        [2.0, 0.0, 0.0],
        [2.0, 2.0, 0.0],
        [0.0, 2.0, 0.0],
        [0.0, 0.0, 2.0],
        [2.0, 0.0, 2.0],
        [2.0, 2.0, 2.0],
        [0.0, 2.0, 2.0],
        [0.0, 0.0, 1.0], // 8: on the wall's edge x = 0, y = 0
        [0.0, 2.0, 1.0], // 9: on the wall's edge x = 0, y = 2
        [1.0, 0.5, 1.0], // 10, 11: the shelf's free corners
        [1.0, 1.5, 1.0],
    ];
    let g1 = group(1);
    let faces = [
        [0, 2, 1],
        [0, 3, 2], // floor, normal -z
        [4, 5, 6],
        [4, 6, 7], // ceiling, +z
        [0, 1, 5],
        [0, 5, 4], // y = 0, -y ... split at vertex 8 on the x = 0 edge:
        [1, 2, 6],
        [1, 6, 5], // x = 2, +x
        [2, 3, 7],
        [2, 7, 6], // y = 2, +y
        [0, 8, 9],
        [0, 9, 3], // x = 0 lower half, -x
        [8, 4, 7],
        [8, 7, 9], // x = 0 upper half
        [8, 10, 11],
        [8, 11, 9], // the shelf, from the wall line 8-9 to its free edge
    ];
    let mut g = Geometry {
        vertices: v.iter().map(|&p| Vec3::from(p)).collect(),
        faces: faces.iter().map(|&f| face(f, g1)).collect(),
    };
    // The y = 0 wall uses the edge 0-4, but the x = 0 wall now uses 0-8 and 8-4: make the y = 0
    // wall use vertex 8 too, so the mesh stays conforming.
    g.faces[4] = face([0, 1, 5], g1);
    g.faces[5] = face([0, 5, 8], g1);
    g.faces.push(face([8, 5, 4], g1));
    // Same on y = 2: 3-7 is split by 9.
    g.faces[8] = face([2, 3, 9], g1);
    g.faces[9] = face([2, 9, 6], g1);
    g.faces.push(face([9, 7, 6], g1));
    let report = check::check(&g);
    eprintln!("shelf: {}", summary(&report));
    assert_eq!(report.verdict, Verdict::Ok, "{:?}", report.reasons);
    assert_eq!(report.counts.sheet_faces, 2);
    assert_eq!(report.counts.cells, 1);
    assert_eq!(
        report.counts.nonmanifold_edges, 1,
        "the shelf's line on the wall"
    );
    assert!(
        report
            .edges
            .iter()
            .any(|e| e.vertices == [8, 9] && e.class == EdgeClass::BoundaryJunction)
    );
    assert_eq!(report.counts.internal_free_edges, 3);
}

#[test]
fn a_fitting_zone_floating_in_the_room_is_a_nested_shell() {
    let mut g = box_geometry([0.0, 0.0, 0.0], [5.0, 5.0, 5.0], group(1));
    append(
        &mut g,
        &box_geometry([1.0, 1.0, 1.0], [2.0, 2.0, 2.0], group(2)),
    );
    let report = check::check(&g);
    eprintln!("nested zone: {}", summary(&report));
    assert_eq!(report.verdict, Verdict::Ok, "{:?}", report.reasons);
    assert_eq!(report.counts.nested_shell_faces, 12);
    assert_eq!(report.counts.cells, 2);
    let mut depths: Vec<(u32, f64)> = report
        .cells
        .iter()
        .map(|c| (c.depth, c.volume_m3))
        .collect();
    depths.sort_by_key(|d| d.0);
    assert_eq!(depths.len(), 2);
    assert_eq!(depths[0].0, 1);
    assert!((depths[0].1 - 124.0).abs() < 1e-9, "room {depths:?}");
    assert_eq!(depths[1].0, 2);
    assert!((depths[1].1 - 1.0).abs() < 1e-9, "zone {depths:?}");
    // A zone whose faces point into it is inverted, face by face.
    let mut inward = g.clone();
    for f in &mut inward.faces[12..] {
        f.vertices.swap(1, 2);
    }
    let report = check::check(&inward);
    assert_eq!(codes(&report), vec![ReasonCode::InvertedFaces]);
    assert_eq!(report.inverted_faces, (12..24).collect::<Vec<u32>>());
}

#[test]
fn a_box_with_a_missing_face_is_refused_as_open() {
    let mut g = box_geometry([0.0, 0.0, 0.0], [6.0, 10.0, 3.0], group(1));
    g.faces.remove(11);
    let report = check::check(&g);
    eprintln!("open box: {}", summary(&report));
    assert_eq!(report.verdict, Verdict::Refused);
    assert_eq!(
        codes(&report),
        vec![ReasonCode::OpenBoundary, ReasonCode::NoEnclosedVolume]
    );
    assert_eq!(report.counts.open_edges, 3);
    assert_eq!(report.counts.boundary_open_edges, 3);
    assert_eq!(report.counts.exterior_faces, 11);
    assert!(
        report
            .edges
            .iter()
            .all(|e| e.class == EdgeClass::BoundaryOpen)
    );
}

#[test]
fn a_fin_outside_the_room_is_refused() {
    let mut g = box_geometry([0.0, 0.0, 0.0], [2.0, 2.0, 2.0], group(1));
    // A triangle hanging off the box's corner edge 0-4 (x = 0, y = 0), outside.
    g.vertices.push(Vec3::new(-1.0, -1.0, 1.0));
    g.faces.push(face([0, 4, 8], group(2)));
    let report = check::check(&g);
    eprintln!("fin: {}", summary(&report));
    assert_eq!(codes(&report), vec![ReasonCode::OpenBoundary]);
    assert_eq!(report.counts.exterior_faces, 1);
    assert_eq!(
        report.reason(ReasonCode::OpenBoundary).unwrap().faces,
        vec![12]
    );
    assert!(
        report
            .edges
            .iter()
            .any(|e| e.class == EdgeClass::PinchedBoundary)
    );
}

#[test]
fn an_inverted_box_is_refused_face_by_face() {
    let mut g = box_geometry([0.0, 0.0, 0.0], [6.0, 10.0, 3.0], group(1));
    for f in &mut g.faces {
        f.vertices.swap(1, 2);
    }
    let report = check::check(&g);
    assert_eq!(codes(&report), vec![ReasonCode::InvertedFaces]);
    assert_eq!(report.inverted_faces, (0..12).collect::<Vec<u32>>());
    assert!((report.measures.signed_volume_m3 + 180.0).abs() < 1e-9);
    assert!((report.measures.enclosed_volume_m3 - 180.0).abs() < 1e-9);
    // One flipped face is found as that face, and as the three manifold edges it clashes on.
    let mut one = box_geometry([0.0, 0.0, 0.0], [6.0, 10.0, 3.0], group(1));
    one.faces[5].vertices.swap(1, 2);
    let report = check::check(&one);
    assert_eq!(report.inverted_faces, vec![5]);
    assert_eq!(report.counts.orientation_conflict_edges, 3);
}

#[test]
fn a_box_resting_on_the_floor_without_sharing_vertices_is_an_intersection() {
    // A crate standing on the floor whose footprint is not cut into the floor: its bottom face
    // lies in the floor's plane, overlapping it. TetGen refuses this, so the check does.
    let mut g = box_geometry([0.0, 0.0, 0.0], [4.0, 4.0, 3.0], group(1));
    append(
        &mut g,
        &box_geometry([1.0, 1.0, 0.0], [2.0, 2.0, 1.0], group(2)),
    );
    let report = check::check(&g);
    assert!(codes(&report).contains(&ReasonCode::SelfIntersections));
    // Every listed pair is the room's floor (faces 0, 1) against the crate: its bottom overlaps
    // the floor, and its walls' bottom edges lie on it.
    assert!(
        report
            .self_intersections
            .iter()
            .all(|&[i, j]| i < 2 && j >= 12),
        "{:?}",
        report.self_intersections
    );
    assert!(
        report
            .self_intersections
            .iter()
            .any(|&[_, j]| j == 12 || j == 13)
    );
    assert!(!report.topology_reliable);
}

#[test]
fn the_committed_fixtures() {
    let cube = schema::load(&repo("tests/fixtures/projects/cube.simpa")).unwrap();
    let report = check::check(&cube.geometry);
    assert_eq!(report.verdict, Verdict::Ok, "{:?}", report.reasons);
    assert!((report.measures.signed_volume_m3 - 125.0).abs() < 1e-9);
    // tutorial1.simpa stores each triangle with its own three vertices, as upstream's .cbin
    // does: 36 vertices at 8 positions. By index every edge is open; welding (repair) closes it.
    let tutorial = schema::load(&repo("tests/fixtures/projects/tutorial1.simpa")).unwrap();
    let report = check::check(&tutorial.geometry);
    eprintln!("tutorial1.simpa: {}", summary(&report));
    assert_eq!(
        codes(&report),
        vec![ReasonCode::OpenBoundary, ReasonCode::NoEnclosedVolume]
    );
    assert_eq!(report.counts.open_edges, 36);
    assert_eq!(report.counts.coincident_vertices, 28);
    assert!(
        report
            .reason(ReasonCode::OpenBoundary)
            .unwrap()
            .message
            .contains("welding")
    );
    // Unwelded but coincident: no face pair counts as intersecting.
    assert_eq!(report.counts.self_intersecting_pairs, 0);
}

#[test]
fn degenerate_duplicate_and_invalid_faces_are_reported_by_index() {
    let g1 = group(1);
    let mut g = box_geometry([0.0, 0.0, 0.0], [1.0, 1.0, 1.0], g1);
    g.vertices.push(Vec3::new(0.5, 0.0, 0.0)); // 8: midpoint of edge 0-1
    g.vertices.push(Vec3::new(f64::NAN, 0.0, 0.0)); // 9
    g.faces.push(face([0, 8, 1], g1)); // 12: zero area
    g.faces.push(face([2, 2, 3], g1)); // 13: repeated vertex
    let dup = g.faces[3];
    g.faces.push(dup); // 14: duplicate of 3
    let mut reversed = g.faces[4];
    reversed.vertices.swap(0, 1);
    reversed.group = group(2);
    g.faces.push(reversed); // 15: reversed duplicate of 4, other group
    g.faces.push(face([0, 1, 99], g1)); // 16: index out of range
    g.faces.push(face([0, 1, 9], g1)); // 17: non-finite vertex
    let report = check::check(&g);
    assert_eq!(
        codes(&report),
        vec![
            ReasonCode::InvalidFaces,
            ReasonCode::DegenerateFaces,
            ReasonCode::DuplicateFaces
        ]
    );
    let degenerate: Vec<(u32, check::DegenerateKind)> = report
        .degenerate_faces
        .iter()
        .map(|d| (d.face, d.kind))
        .collect();
    assert_eq!(
        degenerate,
        vec![
            (12, check::DegenerateKind::ZeroArea),
            (13, check::DegenerateKind::RepeatedVertex)
        ]
    );
    let duplicates: Vec<(u32, u32, bool, bool)> = report
        .duplicate_faces
        .iter()
        .map(|d| (d.face, d.duplicate_of, d.same_orientation, d.same_group))
        .collect();
    assert_eq!(duplicates, vec![(14, 3, true, true), (15, 4, false, false)]);
    let invalid: Vec<(u32, check::InvalidCause)> = report
        .invalid_faces
        .iter()
        .map(|d| (d.face, d.cause))
        .collect();
    assert_eq!(
        invalid,
        vec![
            (16, check::InvalidCause::IndexOutOfRange),
            (17, check::InvalidCause::NonFiniteVertex)
        ]
    );
    // The analysed faces are the box itself.
    assert_eq!(report.counts.analysed_faces, 12);
    assert_eq!(report.counts.cells, 1);
    assert_eq!(report.counts.non_finite_vertices, 1);
    // Faces 12, 15 and 17 use edge 0-1 besides the box's 0 and 4 (16 is out of range, so not in
    // the census): 5 uses, 2 of them analysed.
    let e01 = report.edges.iter().find(|e| e.vertices == [0, 1]).unwrap();
    assert_eq!(
        (e01.uses, e01.analysed_uses, e01.class),
        (5, 2, EdgeClass::Manifold)
    );
    assert_eq!(e01.faces, vec![0, 4, 12, 15, 17]);
    // Face 13 `[2, 2, 3]` uses its one real edge once, not once per half-edge: the box's faces 1
    // and 8 plus face 13 make 3 uses.
    let e23 = report.edges.iter().find(|e| e.vertices == [2, 3]).unwrap();
    assert_eq!(
        (e23.uses, e23.analysed_uses, e23.class),
        (3, 2, EdgeClass::Manifold)
    );
    assert_eq!(e23.faces, vec![1, 8, 13]);
}

#[test]
fn empty_geometry_is_refused() {
    let report = check::check(&Geometry::default());
    assert_eq!(codes(&report), vec![ReasonCode::EmptyGeometry]);
}

#[test]
fn the_report_serialises_with_stable_codes() {
    let mut g = box_geometry([0.0, 0.0, 0.0], [1.0, 1.0, 1.0], group(1));
    g.faces.remove(0);
    let json = serde_json::to_value(check::check(&g)).unwrap();
    assert_eq!(json["verdict"], "refused");
    assert_eq!(json["reasons"][0]["code"], "open_boundary");
    assert_eq!(json["counts"]["open_edges"], 3);
    assert_eq!(json["counts"]["edge_uses"]["1"], 3);
    assert_eq!(json["edges"][0]["class"], "boundary_open");
    let ok =
        serde_json::to_value(check::check(&box_geometry([0.0; 3], [1.0; 3], group(1)))).unwrap();
    assert_eq!(ok["verdict"], "ok");
    assert_eq!(ok["reasons"], serde_json::json!([]));
}

#[test]
fn check_does_not_depend_on_face_order_or_vertex_numbering() {
    // The corrected hall with its faces reversed and its vertices renumbered backwards gives the
    // same verdict and counts.
    let (g, _) = corrected_elmia();
    let n = g.vertices.len() as u32;
    let shuffled = Geometry {
        vertices: g.vertices.iter().rev().copied().collect(),
        faces: g
            .faces
            .iter()
            .rev()
            .map(|f| Face {
                vertices: f.vertices.map(|v| n - 1 - v),
                group: f.group,
            })
            .collect(),
    };
    let (a, b) = (check::check(&g), check::check(&shuffled));
    assert_eq!(a.verdict, b.verdict);
    assert_eq!(a.counts, b.counts);
}

fn group(n: u128) -> GroupId {
    GroupId::from_u128(n)
}

/// Gate (e) through the committed fixture the CLI gate (`tools/gates/m4.ps1`) checks. A missing
/// fixture panics.
#[test]
fn m4e_the_cli_gate_fixture_is_refused_with_its_pairs() {
    let path = committed("tests/fixtures/geometry/two_boxes_interpenetrating.simpa");
    let project = schema::load(&path).expect("two_boxes_interpenetrating.simpa loads");
    let report = check::check(&project.geometry);
    eprintln!("two_boxes_interpenetrating.simpa: {}", summary(&report));
    assert_eq!(report.verdict, Verdict::Refused);
    assert!(!report.self_intersections.is_empty());
    let reason = report.reason(ReasonCode::SelfIntersections).unwrap();
    assert_eq!(reason.count, report.self_intersections.len());
}
