//! `core::geometry::repair` on the M4 gate models and on constructed cases.
//!
//! Gate item (d) (docs/rebuild-plan-raw-2026-09-23.json, milestone M4): inject a duplicate
//! vertex, a flipped face and a zero-area face into the box; repair logs exactly those 3 changes
//! and its output passes the check. Plus the deliverable's rule: holes and self-intersections
//! are refused with a reason, never "fixed".

#[path = "geometry_check_support.rs"]
mod support;

use simpa_core::geometry::check::{self, ReasonCode};
use simpa_core::geometry::repair::{
    self, Change, DegenerateCause, RepairError, RepairOptions, RepairOutcome, RepairStatus,
};
use simpa_core::schema::{self, Geometry, GroupId, Vec3};
use support::*;

fn run(g: &Geometry) -> RepairOutcome {
    repair::repair(g, &RepairOptions::default()).expect("valid options")
}

fn exact() -> RepairOptions {
    RepairOptions {
        weld_tolerance_m: 0.0,
    }
}

fn codes(outcome: &RepairOutcome) -> Vec<ReasonCode> {
    outcome.refusals.iter().map(|r| r.code).collect()
}

fn group(n: u128) -> GroupId {
    GroupId::from_u128(n)
}

/// The receipt printed by the tests: status, log and refusals.
fn summary(outcome: &RepairOutcome) -> String {
    let changes: Vec<String> = outcome
        .changes
        .iter()
        .map(|c| serde_json::to_string(c).unwrap())
        .collect();
    let refusals: Vec<String> = outcome
        .refusals
        .iter()
        .map(|r| format!("{} ({}): {}", r.code, r.count, r.message))
        .collect();
    format!(
        "status {:?}, oriented {}, {} vertices, {} faces\n{} changes:\n  {}\nrefusals:\n  {}",
        outcome.status,
        outcome.oriented,
        outcome.geometry.vertices.len(),
        outcome.geometry.faces.len(),
        changes.len(),
        changes.join("\n  "),
        refusals.join("\n  ")
    )
}

/// The tutorial 1 box (6 x 10 x 3 m, 180 m3): the importer's fixture when it exists, else the
/// same box built here.
fn the_box() -> (Geometry, &'static str) {
    let fixture = repo("tests/fixtures/rooms/tutorial1_box.simpa");
    if fixture.is_file() {
        let project = schema::load(&fixture).expect("tutorial1_box.simpa loads");
        return (project.geometry, "tests/fixtures/rooms/tutorial1_box.simpa");
    }
    (
        box_geometry([0.0; 3], [6.0, 10.0, 3.0], group(1)),
        "box_geometry (the fixture is not there yet)",
    )
}

#[test]
fn m4d_repair_logs_exactly_the_three_injected_changes() {
    let (clean, source) = the_box();
    eprintln!("box from {source}");
    assert_eq!((clean.vertices.len(), clean.faces.len()), (8, 12));
    let before = check::check(&clean);
    assert!(before.is_ok(), "{:?}", before.reasons);
    assert!((before.measures.signed_volume_m3 - 180.0).abs() < 1e-9);

    let mut g = clean.clone();
    // A duplicate vertex: vertex 8 is a copy of face 2's third vertex, and face 2 uses it.
    let copied = g.faces[2].vertices[2];
    g.vertices.push(g.vertices[copied as usize]);
    g.faces[2].vertices[2] = 8;
    // A flipped face.
    g.faces[7].vertices.swap(1, 2);
    // A zero-area face: along face 0's first edge, through that edge's midpoint (vertex 9).
    let [a, b, _] = clean.faces[0].vertices;
    let [pa, pb] = [a, b].map(|v| clean.vertices[v as usize].to_array());
    g.vertices
        .push(Vec3::from([0, 1, 2].map(|k| 0.5 * (pa[k] + pb[k]))));
    g.faces.push(face([a, 9, b], clean.faces[0].group));
    let injected = check::check(&g);
    eprintln!(
        "injected box: verdict {:?}, reasons {:?}",
        injected.verdict,
        injected.reasons.iter().map(|r| r.code).collect::<Vec<_>>()
    );
    assert!(!injected.is_ok());

    let outcome = run(&g);
    eprintln!("m4d: {}", summary(&outcome));
    assert_eq!(
        outcome.changes,
        vec![
            Change::WeldVertex {
                vertex: 8,
                into: copied,
                distance_m: 0.0
            },
            Change::RemoveDegenerateFace {
                face: 12,
                cause: DegenerateCause::ZeroArea
            },
            Change::FlipFace { face: 7 },
        ]
    );
    assert_eq!(outcome.status, RepairStatus::Repaired);
    assert!(outcome.refusals.is_empty());
    assert!(outcome.oriented);
    // The output passes the check, by the report repair returns and by a fresh check.
    assert!(outcome.report.is_ok(), "{:?}", outcome.report.reasons);
    let after = check::check(&outcome.geometry);
    assert!(after.is_ok(), "{:?}", after.reasons);
    assert_eq!(after, outcome.report);
    assert_eq!(after.counts.manifold_edges, 18);
    assert_eq!(after.counts.edges, 18);
    assert!((after.measures.signed_volume_m3 - 180.0).abs() < 1e-9);
    // It is the clean box again: the same 12 faces, vertex for vertex, and the 8 corners. The
    // midpoint vertex is left in place, unreferenced (repair removes a vertex only by welding).
    assert_eq!(outcome.geometry.faces, clean.faces);
    assert_eq!(outcome.geometry.vertices[..8], clean.vertices[..]);
    assert_eq!(outcome.geometry.vertices.len(), 9);
    assert_eq!(after.counts.unreferenced_vertices, 1);
    // The maps.
    let mut expected_vertices: Vec<u32> = (0..8).collect();
    expected_vertices.extend([copied, 8]);
    assert_eq!(outcome.vertex_map, expected_vertices);
    let mut expected_faces: Vec<Option<u32>> = (0..12).map(Some).collect();
    expected_faces.push(None);
    assert_eq!(outcome.face_map, expected_faces);
    // Repairing the output changes nothing.
    let again = run(&outcome.geometry);
    assert_eq!(again.status, RepairStatus::Unchanged);
    assert!(again.changes.is_empty());
    assert_eq!(again.geometry, outcome.geometry);
}

#[test]
fn a_hole_is_refused_and_never_filled() {
    let mut g = box_geometry([0.0; 3], [6.0, 10.0, 3.0], group(1));
    g.faces.remove(11);
    let outcome = run(&g);
    eprintln!("open box: {}", summary(&outcome));
    assert_eq!(outcome.status, RepairStatus::Refused);
    assert!(!outcome.is_ok());
    assert_eq!(
        codes(&outcome),
        vec![ReasonCode::OpenBoundary, ReasonCode::NoEnclosedVolume]
    );
    assert!(outcome.changes.is_empty());
    assert_eq!(outcome.geometry, g, "nothing added, nothing moved");
    let open = &outcome.refusals[0];
    assert_eq!(open.count, 3);
    assert_eq!(open.faces, (0..11).collect::<Vec<u32>>());
}

#[test]
fn self_intersections_are_refused_and_never_fixed() {
    let mut g = box_geometry([0.0; 3], [1.0; 3], group(1));
    append(
        &mut g,
        &box_geometry([0.5, 0.5, 0.5], [1.5, 1.5, 1.5], group(2)),
    );
    let outcome = run(&g);
    eprintln!("interpenetrating boxes: {}", summary(&outcome));
    assert_eq!(outcome.status, RepairStatus::Refused);
    assert_eq!(codes(&outcome), vec![ReasonCode::SelfIntersections]);
    assert!(outcome.changes.is_empty());
    assert!(!outcome.oriented);
    assert_eq!(outcome.geometry, g);
    // The refusal lists the faces of the intersecting pairs, as the check does.
    let report = check::check(&g);
    let mut faces: Vec<u32> = report
        .self_intersections
        .iter()
        .flatten()
        .copied()
        .collect();
    faces.sort_unstable();
    faces.dedup();
    assert_eq!(outcome.refusals[0].faces, faces);
    assert_eq!(outcome.refusals[0].count, 18);
    assert_eq!(outcome.report.self_intersections, report.self_intersections);
}

#[test]
fn an_intersection_blocks_orientation_but_not_the_safe_fixes() {
    // An inverted box plus a crate overlapping its floor: welding and removal still run, but
    // with intersecting faces the cells are unreliable, so nothing is flipped.
    let mut g = box_geometry([0.0; 3], [4.0, 4.0, 3.0], group(1));
    for f in &mut g.faces {
        f.vertices.swap(1, 2);
    }
    append(
        &mut g,
        &box_geometry([1.0, 1.0, 0.0], [2.0, 2.0, 1.0], group(2)),
    );
    let dup = g.faces[3];
    g.faces.push(dup); // 24: a duplicate
    let outcome = run(&g);
    eprintln!("intersecting and inverted: {}", summary(&outcome));
    assert_eq!(outcome.status, RepairStatus::Refused);
    assert!(!outcome.oriented);
    assert_eq!(
        outcome.changes,
        vec![Change::RemoveDuplicateFace {
            face: 24,
            duplicate_of: 3,
            same_orientation: true,
            same_group: true
        }]
    );
    assert!(codes(&outcome).contains(&ReasonCode::SelfIntersections));
}

#[test]
fn the_raw_hall_is_welded_and_cleaned_but_refused() {
    let path = raw_elmia();
    let g = read_ply(&path);
    // Reference, computed independently in Python (exact rational zero-area test, first-kept
    // welding): 7 exact duplicate vertices, face 392 of zero area, no duplicate faces; the
    // result's census is 937 edges used once, 1,146 twice, 6 three times, 2 four times.
    let expected_welds = [
        (258, 231),
        (457, 133),
        (458, 367),
        (460, 276),
        (464, 105),
        (725, 560),
        (915, 563),
    ];
    for options in [exact(), RepairOptions::default()] {
        let outcome = repair::repair(&g, &options).unwrap();
        eprintln!(
            "raw elmia, tolerance {} m: {}",
            options.weld_tolerance_m,
            summary(&outcome)
        );
        let mut expected: Vec<Change> = expected_welds
            .iter()
            .map(|&(vertex, into)| Change::WeldVertex {
                vertex,
                into,
                distance_m: 0.0,
            })
            .collect();
        expected.push(Change::RemoveDegenerateFace {
            face: 392,
            cause: DegenerateCause::ZeroArea,
        });
        assert_eq!(outcome.changes, expected);
        assert_eq!(outcome.status, RepairStatus::Refused);
        assert!(
            !outcome.oriented,
            "intersecting faces: orientation is not decided"
        );
        let refused = codes(&outcome);
        assert!(refused.contains(&ReasonCode::OpenBoundary), "{refused:?}");
        assert!(
            refused.contains(&ReasonCode::SelfIntersections),
            "{refused:?}"
        );
        let c = &outcome.report.counts;
        assert_eq!(
            (
                outcome.geometry.vertices.len(),
                outcome.geometry.faces.len()
            ),
            (948, 1085)
        );
        assert_eq!(
            c.edge_uses
                .iter()
                .map(|(k, v)| (*k, *v))
                .collect::<Vec<_>>(),
            vec![(1, 937), (2, 1146), (3, 6), (4, 2)]
        );
        assert_eq!(c.edges, 2091);
        assert_eq!(c.coincident_vertices, 0);
    }
}

#[test]
fn the_corrected_hall_is_unchanged() {
    let (g, source) = corrected_elmia();
    let outcome = run(&g);
    eprintln!("corrected elmia from {source}: {}", summary(&outcome));
    assert_eq!(outcome.status, RepairStatus::Unchanged);
    assert!(outcome.changes.is_empty());
    assert_eq!(outcome.geometry, g);
    assert!(outcome.report.is_ok());
}

#[test]
fn an_inverted_box_is_flipped_face_by_face() {
    let mut g = box_geometry([0.0; 3], [6.0, 10.0, 3.0], group(1));
    let clean = g.clone();
    for f in &mut g.faces {
        f.vertices.swap(1, 2);
    }
    let outcome = run(&g);
    assert_eq!(outcome.status, RepairStatus::Repaired);
    assert_eq!(
        outcome.changes,
        (0..12)
            .map(|face| Change::FlipFace { face })
            .collect::<Vec<_>>()
    );
    assert_eq!(outcome.geometry, clean);
    assert!((outcome.report.measures.signed_volume_m3 - 180.0).abs() < 1e-9);
}

#[test]
fn an_inward_zone_is_flipped_and_the_room_left_alone() {
    let mut g = box_geometry([0.0; 3], [5.0; 3], group(1));
    append(&mut g, &box_geometry([1.0; 3], [2.0; 3], group(2)));
    let clean = g.clone();
    for f in &mut g.faces[12..] {
        f.vertices.swap(1, 2);
    }
    let outcome = run(&g);
    assert_eq!(outcome.status, RepairStatus::Repaired);
    assert_eq!(
        outcome.changes,
        (12..24)
            .map(|face| Change::FlipFace { face })
            .collect::<Vec<_>>()
    );
    assert_eq!(outcome.geometry, clean);
    assert_eq!(outcome.report.counts.nested_shell_faces, 12);
}

#[test]
fn a_partition_facing_either_way_is_left_alone() {
    let mut g = partitioned_box();
    g.faces[20].vertices.swap(1, 2);
    let outcome = run(&g);
    assert_eq!(outcome.status, RepairStatus::Unchanged);
    assert!(outcome.changes.is_empty());
    assert_eq!(outcome.report.counts.partition_faces, 2);
    // A flipped outer face of the partitioned box is still found and flipped.
    let mut h = partitioned_box();
    h.faces[3].vertices.swap(1, 2);
    let outcome = run(&h);
    assert_eq!(outcome.changes, vec![Change::FlipFace { face: 3 }]);
    assert_eq!(outcome.geometry, partitioned_box());
}

#[test]
fn duplicate_and_degenerate_faces_are_removed_with_their_details() {
    let mut g = box_geometry([0.0; 3], [1.0; 3], group(1));
    let clean = g.clone();
    let dup = g.faces[3];
    g.faces.push(dup); // 12: same orientation, same group
    let mut reversed = g.faces[4];
    reversed.vertices.swap(0, 1);
    reversed.group = group(2);
    g.faces.push(reversed); // 13: reversed, other group
    g.faces.push(face([2, 2, 3], group(1))); // 14: repeated vertex
    let outcome = run(&g);
    eprintln!("duplicates: {}", summary(&outcome));
    assert_eq!(
        outcome.changes,
        vec![
            Change::RemoveDuplicateFace {
                face: 12,
                duplicate_of: 3,
                same_orientation: true,
                same_group: true
            },
            Change::RemoveDuplicateFace {
                face: 13,
                duplicate_of: 4,
                same_orientation: false,
                same_group: false
            },
            Change::RemoveDegenerateFace {
                face: 14,
                cause: DegenerateCause::RepeatedVertex
            },
        ]
    );
    assert_eq!(outcome.status, RepairStatus::Repaired);
    assert_eq!(outcome.geometry, clean);
}

#[test]
fn welding_within_the_tolerance_collapses_slivers_and_logs_them() {
    // The box's top face split into a fan around vertex 8, 1e-7 m from corner 6 (x1 y1 z1): two
    // of the fan's faces are slivers 1e-7 m wide. Welding 8 into 6 collapses them.
    let mut g = box_geometry([0.0; 3], [6.0, 10.0, 3.0], group(1));
    let clean = g.clone();
    let eps = 1e-7;
    g.vertices.push(Vec3::new(6.0 - eps, 10.0 - eps, 3.0));
    let top = group(1);
    g.faces[2] = face([4, 5, 8], top);
    g.faces[3] = face([5, 6, 8], top);
    g.faces.push(face([6, 7, 8], top)); // 12
    g.faces.push(face([7, 4, 8], top)); // 13
    // As given, the fan is a valid closed mesh: exact welding leaves it alone.
    let unwelded = repair::repair(&g, &exact()).unwrap();
    assert_eq!(
        unwelded.status,
        RepairStatus::Unchanged,
        "{}",
        summary(&unwelded)
    );
    // At the default 1 micrometre, 8 welds into 6.
    let outcome = run(&g);
    eprintln!("sliver fan: {}", summary(&outcome));
    assert_eq!(outcome.changes.len(), 3);
    match outcome.changes[0] {
        Change::WeldVertex {
            vertex: 8,
            into: 6,
            distance_m,
        } => assert!(
            (distance_m - eps * 2f64.sqrt()).abs() < 1e-12,
            "{distance_m}"
        ),
        ref other => panic!("{other:?}"),
    }
    assert_eq!(
        outcome.changes[1..],
        [
            Change::RemoveDegenerateFace {
                face: 3,
                cause: DegenerateCause::CollapsedByWeld
            },
            Change::RemoveDegenerateFace {
                face: 12,
                cause: DegenerateCause::CollapsedByWeld
            },
        ]
    );
    assert_eq!(outcome.status, RepairStatus::Repaired);
    // Faces 2 and 13 become the box's top pair, as [4, 5, 6] and [7, 4, 6].
    assert_eq!(outcome.geometry.vertices, clean.vertices);
    assert_eq!(outcome.geometry.faces.len(), 12);
    assert!((outcome.report.measures.signed_volume_m3 - 180.0).abs() < 1e-9);
}

#[test]
fn a_near_duplicate_outside_the_tolerance_is_not_welded_and_the_seam_is_refused() {
    let (clean, _) = the_box();
    let mut g = clean.clone();
    let v = g.faces[2].vertices[2];
    let p = g.vertices[v as usize].to_array();
    g.vertices.push(Vec3::from(p.map(|c| c + 1e-5)));
    g.faces[2].vertices[2] = 8;
    let outcome = run(&g);
    eprintln!("near duplicate at 1.7e-5 m: {}", summary(&outcome));
    assert!(outcome.changes.is_empty());
    assert_eq!(outcome.status, RepairStatus::Refused);
    assert!(codes(&outcome).contains(&ReasonCode::OpenBoundary));
    // With a tolerance above the distance, it is welded and the box is whole again.
    let wide = repair::repair(
        &g,
        &RepairOptions {
            weld_tolerance_m: 1e-4,
        },
    )
    .unwrap();
    assert_eq!(wide.status, RepairStatus::Repaired);
    assert_eq!(wide.changes.len(), 1);
    assert_eq!(wide.geometry.faces, clean.faces);
}

#[test]
fn invalid_faces_are_refused_by_input_index() {
    let mut g = box_geometry([0.0; 3], [1.0; 3], group(1));
    g.vertices.push(Vec3::new(9.0, 9.0, 9.0)); // 8: unreferenced
    g.vertices.push(Vec3::new(0.0, 0.0, 0.0)); // 9: a duplicate of 0, welded away
    g.faces.insert(0, face([0, 1, 99], group(1))); // 0: out of range
    let outcome = run(&g);
    eprintln!("invalid face: {}", summary(&outcome));
    assert_eq!(outcome.status, RepairStatus::Refused);
    assert_eq!(codes(&outcome), vec![ReasonCode::InvalidFaces]);
    assert_eq!(outcome.refusals[0].faces, vec![0]);
    assert_eq!(outcome.face_map[0], Some(0));
    // Its in-range indices are renumbered; its out-of-range one stays out of range.
    assert_eq!(outcome.geometry.faces[0].vertices, [0, 1, 99]);
    assert_eq!(outcome.geometry.vertices.len(), 9);
}

#[test]
fn invalid_tolerances_are_errors() {
    let g = box_geometry([0.0; 3], [1.0; 3], group(1));
    for t in [-1e-6, f64::NAN, f64::INFINITY] {
        let err = repair::repair(
            &g,
            &RepairOptions {
                weld_tolerance_m: t,
            },
        )
        .unwrap_err();
        assert!(matches!(err, RepairError::InvalidTolerance(_)));
        assert_eq!(err.code(), "invalid_tolerance");
    }
}

#[test]
fn the_outcome_serialises_with_stable_kinds() {
    let mut g = box_geometry([0.0; 3], [1.0; 3], group(1));
    g.faces[5].vertices.swap(1, 2);
    let dup = g.faces[0];
    g.faces.push(dup);
    let json = serde_json::to_value(run(&g)).unwrap();
    assert_eq!(json["status"], "repaired");
    assert_eq!(json["changes"][0]["kind"], "remove_duplicate_face");
    assert_eq!(json["changes"][0]["duplicate_of"], 0);
    assert_eq!(
        json["changes"][1],
        serde_json::json!({"kind": "flip_face", "face": 5})
    );
    assert_eq!(json["face_map"][12], serde_json::Value::Null);
    assert_eq!(json["report"]["verdict"], "ok");
    assert_eq!(json["refusals"], serde_json::json!([]));
    let mut open = box_geometry([0.0; 3], [1.0; 3], group(1));
    open.faces.pop();
    let json = serde_json::to_value(run(&open)).unwrap();
    assert_eq!(json["status"], "refused");
    assert_eq!(json["refusals"][0]["code"], "open_boundary");
}

/// Gate (d) through the committed fixture the CLI gate (`tools/gates/m4.ps1`) repairs. A missing
/// fixture panics.
#[test]
fn m4d_the_cli_gate_fixture_repairs_with_exactly_three_changes() {
    let path = committed("tests/fixtures/geometry/box_three_faults.simpa");
    let project = schema::load(&path).expect("box_three_faults.simpa loads");
    assert!(!check::check(&project.geometry).is_ok());
    let outcome = run(&project.geometry);
    eprintln!("box_three_faults.simpa: {}", summary(&outcome));
    let kinds: Vec<&str> = outcome.changes.iter().map(Change::kind).collect();
    assert_eq!(
        kinds,
        ["weld_vertex", "remove_degenerate_face", "flip_face"]
    );
    assert_eq!(outcome.status, RepairStatus::Repaired);
    assert!(check::check(&outcome.geometry).is_ok());
}
