//! G19, faces sent to a new surface group (docs/investigations/2026-10-03-row15/PLAN.md, order of
//! work 1): `Op::RegroupFaces` and `Project::regrouped`, the op the UI's "New group from
//! selection" applies. The load-bearing claims: one undo gives the project back bit for bit; no
//! face takes a material it did not have (the new group keeps the faces' common material, or is
//! upstream's placeholder, which blocks a run); every surface receiver and fitting zone keeps
//! exactly the faces it had; and the emptied group stays, accepted by the validator and the
//! `config.xml` writer.

use std::path::Path;

use simpa_core::config_xml::{self, write};
use simpa_core::schema::{
    self, DiffusionLaw, F64, FittingShape, FittingZone, FittingZoneId, GroupId, History,
    MaterialId, Op, OpError, Project, SolverKind, SurfaceGroup, SurfaceReceiverShape, Variant,
    VariantId, Vec3,
};
use simpa_core::validate::{self, codes, is_placeholder_material};

#[allow(dead_code)]
#[path = "common/paths.rs"]
mod paths;

/// The teaching room: tutorial 1's 6 x 10 x 3 m box, Ceiling (faces 10, 11; 30 %), Floor (faces 0,
/// 1; 10 %; the scene receiver `Receiver`'s only group) and Walls (faces 2 to 9; 20 %).
fn room() -> Project {
    schema::load(&paths::fixture("rooms/tutorial1_box.simpa")).expect("tutorial1_box.simpa loads")
}

fn group_named(p: &Project, name: &str) -> GroupId {
    p.surface_groups
        .iter()
        .find(|g| g.name == name)
        .unwrap_or_else(|| panic!("no group {name}"))
        .id
}

fn material_named(p: &Project, name: &str) -> MaterialId {
    p.materials
        .iter()
        .find(|m| m.name == name)
        .unwrap_or_else(|| panic!("no material {name}"))
        .id
}

fn faces_of(p: &Project, g: GroupId) -> Vec<u32> {
    (0..p.geometry.faces.len() as u32)
        .filter(|&i| p.geometry.faces[i as usize].group == g)
        .collect()
}

const NEW: GroupId = GroupId::from_u128(0x6e65_772d_6772_6f75_7000_0000_0000_0001);
const PLACEHOLDER: MaterialId = MaterialId::from_u128(0x706c_6163_6568_6f6c_6465_7200_0000_0001);

/// Applies `op` sent as JSON text (as the UI sends it), checks the undo gives the original
/// project back `==` and byte for byte, and redo the edited one; returns the edited project.
fn apply_exact(original: &Project, op: Op) -> Project {
    let op = Op::from_json(&op.to_json()).expect("the op reads back from its text");
    let mut p = original.clone();
    let mut history = History::new();
    history.apply(&mut p, op).expect("the op applies");
    p.check_integrity().expect("the result keeps integrity");
    let edited = p.clone();
    assert!(history.undo(&mut p).unwrap());
    assert!(&p == original, "undo is not exact");
    assert_eq!(schema::to_json(&p), schema::to_json(original));
    assert!(history.redo(&mut p).unwrap());
    assert!(p == edited, "redo is not exact");
    edited
}

/// Applies `op`, which must be refused with the project unchanged; returns the refusal, the
/// failing op's own when `op` is a batch (as `Project::regrouped` builds when it adds the
/// placeholder).
fn refused(original: &Project, op: Op) -> OpError {
    let mut p = original.clone();
    let mut err = op.apply(&mut p).expect_err("the op is refused");
    assert!(&p == original, "a refused op changed the project");
    while let OpError::Batch { error, .. } = err {
        err = *error;
    }
    err
}

fn regroup(p: &Project, faces: &[u32], name: &str, material: MaterialId) -> Op {
    Op::RegroupFaces {
        index: p.surface_groups.len(),
        group: SurfaceGroup {
            id: NEW,
            name: name.to_string(),
            material,
        },
        faces: faces.to_vec(),
    }
}

// ---- apply and undo -----------------------------------------------------------------------------

/// Two wall faces sent to a new group: they leave Walls, the new group is appended with Walls'
/// material, and the scene receiver (on Floor only) is untouched. Undo gives back the original.
#[test]
fn faces_move_to_a_new_group_with_their_common_material_and_undo_is_exact() {
    let original = room();
    let walls = group_named(&original, "Walls");
    let op = original.regrouped(&[3, 2], NEW, "Group 1", PLACEHOLDER);
    let p = apply_exact(&original, op);

    assert_eq!(p.surface_groups.len(), 4);
    let g = p.surface_groups.last().unwrap();
    assert_eq!((g.id, g.name.as_str()), (NEW, "Group 1"));
    assert_eq!(g.material, material_named(&original, "20% absorbing"));
    assert_eq!(faces_of(&p, NEW), [2, 3]);
    assert_eq!(faces_of(&p, walls), [4, 5, 6, 7, 8, 9]);
    assert_eq!(p.materials, original.materials, "no material added");
    assert_eq!(p.surface_receivers, original.surface_receivers);
    // Every face keeps its material.
    for (i, (a, b)) in original
        .geometry
        .faces
        .iter()
        .zip(&p.geometry.faces)
        .enumerate()
    {
        assert_eq!(
            original.active_material(a.group),
            p.active_material(b.group),
            "face {i}"
        );
        assert_eq!(a.vertices, b.vertices);
    }
    assert!(!validate::has_errors(&validate::validate(&p)));
}

/// Faces of two materials: the new group is upstream's placeholder (reference material 0), added
/// to the project's materials since the room has none, and `material_placeholder` blocks the run
/// until a material is chosen. One undo removes the group and the placeholder.
#[test]
fn faces_of_two_materials_get_the_placeholder_which_blocks_the_run() {
    let original = room();
    assert!(!original.materials.iter().any(is_placeholder_material));
    let op = original.regrouped(&[2, 10], NEW, "Group 1", PLACEHOLDER);
    let p = apply_exact(&original, op);

    let g = p.group(NEW).unwrap();
    assert_eq!(g.material, PLACEHOLDER);
    assert!(is_placeholder_material(p.material(PLACEHOLDER).unwrap()));
    assert_eq!(p.materials.len(), original.materials.len() + 1);
    let issues = validate::validate(&p);
    let hits: Vec<_> = issues
        .iter()
        .filter(|i| i.code == codes::MATERIAL_PLACEHOLDER)
        .collect();
    assert_eq!(hits.len(), 1, "{issues:#?}");
    assert_eq!(hits[0].path, "/surface_groups/3/material");
    assert!(validate::has_errors(&issues));

    // A project that already holds the placeholder reuses it.
    let mut with = original.clone();
    let first = original.regrouped(&[2, 10], NEW, "Group 1", PLACEHOLDER);
    first.apply(&mut with).unwrap();
    let again = with.regrouped(
        &[4, 11],
        GroupId::from_u128(7),
        "Group 2",
        MaterialId::from_u128(8),
    );
    let q = apply_exact(&with, again);
    assert_eq!(q.materials.len(), with.materials.len());
    assert_eq!(
        q.group(GroupId::from_u128(7)).unwrap().material,
        PLACEHOLDER
    );
}

/// The op takes the material it is given only when no face changes material: the faces' own, or
/// the placeholder. Any other is refused.
#[test]
fn a_material_none_of_the_faces_had_is_refused() {
    let original = room();
    let ceiling = material_named(&original, "30% absorbing");
    let err = refused(&original, regroup(&original, &[2, 3], "Group 1", ceiling));
    assert_eq!(err.code(), "material_change", "{err}");
    let err = refused(&original, regroup(&original, &[2, 10], "Group 1", ceiling));
    assert_eq!(err.code(), "material_change", "{err}");
}

// ---- refusals -----------------------------------------------------------------------------------

#[test]
fn bad_face_lists_ids_and_names_are_refused_and_nothing_changes() {
    let p = room();
    let walls = material_named(&p, "20% absorbing");
    let n = p.geometry.faces.len() as u32;
    let cases: Vec<(&str, Op, &str)> = vec![
        ("empty", regroup(&p, &[], "Group 1", walls), "faces"),
        (
            "out of range",
            regroup(&p, &[2, n], "Group 1", walls),
            "faces",
        ),
        (
            "duplicate",
            regroup(&p, &[2, 3, 2], "Group 1", walls),
            "faces",
        ),
        (
            "name taken",
            regroup(&p, &[2, 3], "Walls", walls),
            "name_taken",
        ),
        (
            "id taken",
            Op::RegroupFaces {
                index: 3,
                group: SurfaceGroup {
                    id: p.surface_groups[0].id,
                    name: "Group 1".into(),
                    material: walls,
                },
                faces: vec![2, 3],
            },
            "duplicate_id",
        ),
        (
            "index past the end",
            Op::RegroupFaces {
                index: 4,
                group: SurfaceGroup {
                    id: NEW,
                    name: "Group 1".into(),
                    material: walls,
                },
                faces: vec![2, 3],
            },
            "index",
        ),
        (
            "no such material",
            regroup(&p, &[2, 3], "Group 1", MaterialId::from_u128(99)),
            "integrity",
        ),
    ];
    for (what, op, code) in cases {
        let err = refused(&p, op);
        assert_eq!(err.code(), code, "{what}: {err}");
    }
}

// ---- surface receivers and fitting zones keep their faces ---------------------------------------

/// The scene receiver `Receiver` holds Floor. Floor's faces moved to a new group take the
/// receiver with them (the new group joins its list); a selection with faces in and out of it is
/// refused, since the new group would either drop faces from the receiver or add faces to it.
#[test]
fn a_scene_receiver_keeps_exactly_its_faces() {
    let original = room();
    let floor = group_named(&original, "Floor");
    let receives = |p: &Project| -> Vec<u32> {
        let SurfaceReceiverShape::Scene { groups } = &p.surface_receivers[0].shape else {
            panic!("the room's receiver is a scene receiver")
        };
        (0..p.geometry.faces.len() as u32)
            .filter(|&i| groups.contains(&p.geometry.faces[i as usize].group))
            .collect()
    };
    assert_eq!(faces_of(&original, floor), [0, 1]);
    assert_eq!(receives(&original), [0, 1]);

    for faces in [&[0u32][..], &[1, 0][..]] {
        let p = apply_exact(
            &original,
            original.regrouped(faces, NEW, "Group 1", PLACEHOLDER),
        );
        assert_eq!(receives(&p), [0, 1], "{faces:?}");
        let SurfaceReceiverShape::Scene { groups } = &p.surface_receivers[0].shape else {
            unreachable!()
        };
        assert_eq!(groups, &[floor, NEW]);
        assert!(!validate::has_errors(&validate::validate(&p)), "{faces:?}");
    }

    let err = refused(
        &original,
        original.regrouped(&[0, 2], NEW, "Group 1", PLACEHOLDER),
    );
    assert_eq!(err.code(), "split", "{err}");
    assert!(
        err.to_string().contains("surface receiver 'Receiver'"),
        "{err}"
    );
}

fn surfaces_zone(p: &Project, groups: Vec<GroupId>) -> FittingZone {
    let n = p.bands.len();
    FittingZone {
        id: FittingZoneId::from_u128(0x7a),
        name: "Zone".into(),
        enabled: true,
        shape: FittingShape::Surfaces {
            groups,
            inside_point: Vec3::from([3.0, 5.0, 1.5]),
        },
        absorption: vec![F64::new(0.2); n],
        mean_free_path_m: vec![F64::new(1.0); n],
        diffusion_law: vec![DiffusionLaw::Uniform; n],
        solver_id: None,
    }
}

#[test]
fn a_fitting_zone_keeps_exactly_its_faces() {
    let mut original = room();
    let walls = group_named(&original, "Walls");
    Op::AddFittingZone {
        index: 0,
        zone: surfaces_zone(&original, vec![walls]),
    }
    .apply(&mut original)
    .unwrap();

    let p = apply_exact(
        &original,
        original.regrouped(&[4, 5, 6], NEW, "Group 1", PLACEHOLDER),
    );
    let FittingShape::Surfaces { groups, .. } = &p.fitting_zones[0].shape else {
        unreachable!()
    };
    assert_eq!(groups, &[walls, NEW]);

    let err = refused(
        &original,
        original.regrouped(&[4, 10], NEW, "Group 1", PLACEHOLDER),
    );
    assert_eq!(err.code(), "split", "{err}");
    assert!(err.to_string().contains("fitting zone 'Zone'"), "{err}");
}

// ---- variants -----------------------------------------------------------------------------------

/// A variant overriding Walls: wall faces moved to a new group keep the variant's material under
/// it (the override is carried) and their base material without it.
#[test]
fn a_variant_override_is_carried_so_no_face_changes_material_under_it() {
    let mut original = room();
    let walls = group_named(&original, "Walls");
    let ceiling_m = material_named(&original, "30% absorbing");
    let v = VariantId::from_u128(0x76);
    Op::AddVariant {
        index: 0,
        variant: Variant {
            id: v,
            name: "Curtains".into(),
            overrides: Vec::new(),
        },
    }
    .apply(&mut original)
    .unwrap();
    Op::SetVariantOverride {
        variant: v,
        group: walls,
        material: Some(ceiling_m),
    }
    .apply(&mut original)
    .unwrap();
    let mut active = original.clone();
    Op::SetActiveVariant { variant: Some(v) }
        .apply(&mut active)
        .unwrap();

    for start in [&original, &active] {
        let p = apply_exact(start, start.regrouped(&[2, 3], NEW, "Group 1", PLACEHOLDER));
        for variant in [None, Some(v)] {
            for (i, (a, b)) in start
                .geometry
                .faces
                .iter()
                .zip(&p.geometry.faces)
                .enumerate()
            {
                assert_eq!(
                    start.effective_material(a.group, variant),
                    p.effective_material(b.group, variant),
                    "face {i} under {variant:?}"
                );
            }
        }
        assert_eq!(
            p.group(NEW).unwrap().material,
            material_named(start, "20% absorbing")
        );
        assert_eq!(p.variant(v).unwrap().override_for(NEW), Some(ceiling_m));
    }

    // Wall and ceiling faces: under the variant both are 30 %, in the base project 20 % and 30 %.
    // The base project's materials differ, so the new group is the placeholder, and the variant's
    // common 30 % is carried; the active variant's run sees 30 % on every moved face.
    let p = apply_exact(
        &active,
        active.regrouped(&[2, 10], NEW, "Group 1", PLACEHOLDER),
    );
    assert_eq!(p.group(NEW).unwrap().material, PLACEHOLDER);
    assert_eq!(p.active_material(NEW), Some(ceiling_m));
    assert!(
        !validate::validate(&p)
            .iter()
            .any(|i| i.code == codes::MATERIAL_PLACEHOLDER)
    );
}

/// The base project's materials agree but a variant's do not: keeping the common material would
/// change some faces under that variant, so the group is the placeholder; the bare op given the
/// common material is refused.
#[test]
fn a_variant_that_splits_the_faces_makes_the_group_a_placeholder() {
    let mut original = room();
    let walls = group_named(&original, "Walls");
    let ceiling = group_named(&original, "Ceiling");
    let walls_m = material_named(&original, "20% absorbing");
    let floor_m = material_named(&original, "10% absorbing");
    Op::SetGroupMaterial {
        group: ceiling,
        material: walls_m,
    }
    .apply(&mut original)
    .unwrap();
    let v = VariantId::from_u128(0x76);
    Op::AddVariant {
        index: 0,
        variant: Variant {
            id: v,
            name: "Carpet".into(),
            overrides: Vec::new(),
        },
    }
    .apply(&mut original)
    .unwrap();
    Op::SetVariantOverride {
        variant: v,
        group: walls,
        material: Some(floor_m),
    }
    .apply(&mut original)
    .unwrap();

    let err = refused(&original, regroup(&original, &[2, 10], "Group 1", walls_m));
    assert_eq!(err.code(), "split", "{err}");
    assert!(err.to_string().contains("variant 'Carpet'"), "{err}");

    let p = apply_exact(
        &original,
        original.regrouped(&[2, 10], NEW, "Group 1", PLACEHOLDER),
    );
    assert_eq!(p.group(NEW).unwrap().material, PLACEHOLDER);
    assert_eq!(p.variant(v).unwrap().override_for(NEW), None);
}

// ---- the emptied group --------------------------------------------------------------------------

/// Every face of Floor moved out: Floor stays, empty, so the undo is exact. The validator raises
/// nothing for it, and the scene mesh and `config.xml` are written as before for every face.
#[test]
fn the_emptied_group_stays_and_export_accepts_it() {
    let original = room();
    let floor = group_named(&original, "Floor");
    let p = apply_exact(
        &original,
        original.regrouped(&[0, 1], NEW, "Floor 2", PLACEHOLDER),
    );
    assert!(faces_of(&p, floor).is_empty());
    assert!(p.group(floor).is_some());
    let before: Vec<_> = validate::validate(&original)
        .into_iter()
        .map(|i| (i.code, i.message))
        .collect();
    let after: Vec<_> = validate::validate(&p)
        .into_iter()
        .map(|i| (i.code, i.message))
        .collect();
    assert_eq!(after, before);

    // The solvers see the same model: per face, the same material and the same receiver.
    let a = config_xml::scene_mesh(&original).unwrap();
    let b = config_xml::scene_mesh(&p).unwrap();
    assert_eq!(format!("{a:?}"), format!("{b:?}"));
    let workdir = Path::new(r"C:\run");
    for solver in [SolverKind::Spps, SolverKind::Tcr] {
        assert_eq!(
            write(&p, solver, None, workdir).unwrap(),
            write(&original, solver, None, workdir).unwrap(),
            "{solver:?}"
        );
    }
}
