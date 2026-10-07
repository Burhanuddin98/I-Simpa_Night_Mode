//! C1 (docs/investigations/2026-10-07-blank-geometry/SPEC.md): a stranger's blank geometry carved
//! into named groups. `Op::MoveFaces` ("Move selection to group") and `Op::MergeSurfaceGroups`,
//! each one exact undo step; a mesh file that declares no groups imports as one group named for
//! the file; upstream's reference spectra as a source's shape on the project's bands.
//!
//! The load-bearing claims: undo gives the project back bit for bit; no surface receiver or
//! fitting zone gains or loses a face; a merge keeps the first group's name and material; a
//! refused op changes nothing.

use simpa_core::geometry::import::{
    self, ImportOptions, REFERENCE_SPECTRA, Unit, Up, reference_spectrum,
};
use simpa_core::schema::{
    self, BandKind, BandSet, DiffusionLaw, F64, FittingShape, FittingZone, FittingZoneId, GroupId,
    History, MaterialId, Op, OpError, Project, Spectrum, SpectrumShape,
    SurfaceReceiverShape, Variant, VariantId, Vec3,
};
use simpa_core::config_xml::widen_f32;
use simpa_core::validate;

#[allow(dead_code)]
#[path = "common/paths.rs"]
mod paths;

/// The teaching room: tutorial 1's 6 x 10 x 3 m box, Ceiling (faces 10, 11; 30 %), Floor (faces
/// 0, 1; 10 %; the scene receiver `Receiver`'s only group) and Walls (faces 2 to 9; 20 %).
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

fn faces_of(p: &Project, g: GroupId) -> Vec<u32> {
    (0..p.geometry.faces.len() as u32)
        .filter(|&i| p.geometry.faces[i as usize].group == g)
        .collect()
}

/// Applies `op` sent as JSON text (as the UI sends it), checks undo gives the original back
/// `==` and byte for byte, and redo the edited one; returns the edited project.
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

fn refused(original: &Project, op: Op) -> OpError {
    let mut p = original.clone();
    let err = op.apply(&mut p).expect_err("the op is refused");
    assert!(&p == original, "a refused op changed the project");
    err
}

const NEW: GroupId = GroupId::from_u128(0x6e65_772d_6772_6f75_7000_0000_0000_0001);

/// Two wall faces regrouped, then a third wall face moved into that group: it leaves Walls and
/// takes the new group's material; one undo puts it back.
#[test]
fn move_faces_into_an_existing_group_and_undo_is_exact() {
    let mut original = room();
    let walls = group_named(&original, "Walls");
    let regroup = original.regrouped(&[2, 3], NEW, "Group 1", MaterialId::random());
    regroup.apply(&mut original).unwrap();
    let ceiling_mat = original.group(group_named(&original, "Ceiling")).unwrap().material;
    Op::SetGroupMaterial {
        group: NEW,
        material: ceiling_mat,
    }
    .apply(&mut original)
    .unwrap();

    let p = apply_exact(
        &original,
        Op::MoveFaces {
            group: NEW,
            faces: vec![4],
        },
    );
    assert_eq!(faces_of(&p, NEW), [2, 3, 4]);
    assert_eq!(faces_of(&p, walls), [5, 6, 7, 8, 9]);
    assert_eq!(p.effective_material(NEW, None), Some(ceiling_mat));
    assert_eq!(p.surface_groups, original.surface_groups, "no group added or removed");
    assert!(!validate::has_errors(&validate::validate(&p)));

    // A face already in the group stays; moving a whole group empties it, and it stays.
    let p = apply_exact(
        &original,
        Op::MoveFaces {
            group: NEW,
            faces: vec![2, 5, 6, 7, 8, 9, 4],
        },
    );
    assert_eq!(faces_of(&p, NEW), [2, 3, 4, 5, 6, 7, 8, 9]);
    assert!(faces_of(&p, walls).is_empty());
    assert!(p.group(walls).is_some(), "the emptied group stays");
}

#[test]
fn move_faces_refusals_change_nothing() {
    let original = room();
    let walls = group_named(&original, "Walls");
    let floor = group_named(&original, "Floor");
    let ceiling = group_named(&original, "Ceiling");
    let mv = |group, faces: &[u32]| Op::MoveFaces {
        group,
        faces: faces.to_vec(),
    };
    assert_eq!(refused(&original, mv(walls, &[])).code(), "faces");
    assert_eq!(refused(&original, mv(walls, &[12])).code(), "faces");
    assert_eq!(refused(&original, mv(walls, &[10, 10])).code(), "faces");
    assert_eq!(refused(&original, mv(NEW, &[10])).code(), "not_found");
    // The scene receiver holds Floor only: a wall face into Floor would add a face to it, a
    // floor face into the Ceiling would take one out of it.
    let err = refused(&original, mv(floor, &[2]));
    assert_eq!(err.code(), "split", "{err}");
    assert!(err.to_string().contains("surface receiver 'Receiver'"), "{err}");
    assert!(err.to_string().contains("add faces to"), "{err}");
    let err = refused(&original, mv(ceiling, &[0]));
    assert_eq!(err.code(), "split", "{err}");
    assert!(err.to_string().contains("take faces out of"), "{err}");
    // Ceiling to Walls: neither is the receiver's, so it applies.
    apply_exact(&original, mv(walls, &[10]));
}

/// Walls split into two groups, then merged back: the faces return to the first group, which
/// keeps its name and material; the second is gone; one undo restores both.
#[test]
fn merge_two_groups_keeps_the_first_and_undo_is_exact() {
    let mut original = room();
    let walls = group_named(&original, "Walls");
    original
        .regrouped(&[2, 3, 4], NEW, "Group 1", MaterialId::random())
        .apply(&mut original)
        .unwrap();
    // Give the new group a different material: the merge keeps the first's.
    let ceiling_mat = original.group(group_named(&original, "Ceiling")).unwrap().material;
    Op::SetGroupMaterial {
        group: NEW,
        material: ceiling_mat,
    }
    .apply(&mut original)
    .unwrap();
    let walls_mat = original.group(walls).unwrap().material;

    let p = apply_exact(
        &original,
        Op::MergeSurfaceGroups {
            into: walls,
            from: vec![NEW],
        },
    );
    assert_eq!(p.surface_groups.len(), original.surface_groups.len() - 1);
    assert!(p.group(NEW).is_none());
    let w = p.group(walls).unwrap();
    assert_eq!((w.name.as_str(), w.material), ("Walls", walls_mat));
    assert_eq!(faces_of(&p, walls), [2, 3, 4, 5, 6, 7, 8, 9]);
    assert!(!validate::has_errors(&validate::validate(&p)));

    // The other way round: Group 1 absorbs Walls, keeping Group 1's name and material.
    let p = apply_exact(
        &original,
        Op::MergeSurfaceGroups {
            into: NEW,
            from: vec![walls],
        },
    );
    let g = p.group(NEW).unwrap();
    assert_eq!((g.name.as_str(), g.material), ("Group 1", ceiling_mat));
    assert_eq!(faces_of(&p, NEW), [2, 3, 4, 5, 6, 7, 8, 9]);
}

/// Three groups into one, from the middle of the list, with variant overrides on the merged
/// groups: undo restores the order, the overrides and the mesh exactly.
#[test]
fn a_merge_of_several_with_overrides_undoes_exactly() {
    let mut original = room();
    let ceiling = group_named(&original, "Ceiling");
    let floor = group_named(&original, "Floor");
    let walls = group_named(&original, "Walls");
    let other = original.materials[0].id;
    let mut v = Variant {
        id: VariantId::from_u128(0x7661),
        name: "Treated".into(),
        overrides: Vec::new(),
    };
    v.set_override(walls, Some(other));
    v.set_override(ceiling, Some(other));
    Op::AddVariant {
        index: 0,
        variant: v,
    }
    .apply(&mut original)
    .unwrap();
    // The scene receiver holds Floor alone, so Floor cannot be merged with the others; put it on
    // all three groups first.
    let mut r = original.surface_receivers[0].clone();
    r.shape = SurfaceReceiverShape::Scene {
        groups: vec![floor, walls, ceiling],
    };
    Op::ReplaceSurfaceReceiver { receiver: r }
        .apply(&mut original)
        .unwrap();

    let into = original.surface_groups[0].id;
    let from: Vec<GroupId> = original.surface_groups[1..].iter().map(|g| g.id).collect();
    let p = apply_exact(&original, Op::MergeSurfaceGroups { into, from });
    assert_eq!(p.surface_groups.len(), 1);
    assert_eq!(faces_of(&p, into).len(), 12);
    let SurfaceReceiverShape::Scene { groups } = &p.surface_receivers[0].shape else {
        unreachable!()
    };
    assert_eq!(groups, &[into], "the receiver keeps the merged group only");
    for g in original.surface_groups[1..].iter() {
        assert_eq!(p.variants[0].override_for(g.id), None, "{}", g.name);
    }
}

#[test]
fn merge_refusals_change_nothing() {
    let mut original = room();
    let ceiling = group_named(&original, "Ceiling");
    let floor = group_named(&original, "Floor");
    let walls = group_named(&original, "Walls");
    let merge = |into, from: &[GroupId]| Op::MergeSurfaceGroups {
        into,
        from: from.to_vec(),
    };
    assert_eq!(refused(&original, merge(walls, &[])).code(), "groups");
    assert_eq!(refused(&original, merge(walls, &[walls])).code(), "groups");
    assert_eq!(refused(&original, merge(walls, &[ceiling, ceiling])).code(), "groups");
    assert_eq!(refused(&original, merge(walls, &[NEW])).code(), "not_found");
    assert_eq!(refused(&original, merge(NEW, &[walls])).code(), "not_found");
    // The scene receiver holds Floor alone.
    let err = refused(&original, merge(walls, &[floor]));
    assert_eq!(err.code(), "split", "{err}");
    assert!(err.to_string().contains("surface receiver 'Receiver'"), "{err}");
    // A surfaces fitting zone on Walls alone refuses a merge with the Ceiling.
    let n = original.bands.len();
    Op::AddFittingZone {
        index: 0,
        zone: FittingZone {
            id: FittingZoneId::from_u128(0x7a),
            name: "Zone".into(),
            enabled: true,
            shape: FittingShape::Surfaces {
                groups: vec![walls],
                inside_point: Vec3::from([3.0, 5.0, 1.5]),
            },
            absorption: vec![F64::new(0.2); n],
            mean_free_path_m: vec![F64::new(1.0); n],
            diffusion_law: vec![DiffusionLaw::Uniform; n],
            solver_id: None,
        },
    }
    .apply(&mut original)
    .unwrap();
    let err = refused(&original, merge(walls, &[ceiling]));
    assert_eq!(err.code(), "split", "{err}");
    assert!(err.to_string().contains("fitting zone 'Zone'"), "{err}");
}

/// The JSON names the UI sends (`app/ui/src/ops.ts`).
#[test]
fn the_ops_read_from_the_ui_text() {
    let g = "6e65772d-6772-6f75-7000-000000000001";
    let h = "6e65772d-6772-6f75-7000-000000000002";
    let op = Op::from_json(&format!(r#"{{"op":"move_faces","group":"{g}","faces":[3,1]}}"#)).unwrap();
    assert_eq!(
        op,
        Op::MoveFaces {
            group: NEW,
            faces: vec![3, 1]
        }
    );
    let op = Op::from_json(&format!(
        r#"{{"op":"merge_surface_groups","into":"{g}","from":["{h}"]}}"#
    ))
    .unwrap();
    assert!(matches!(op, Op::MergeSurfaceGroups { into, ref from } if into == NEW && from.len() == 1));
    assert!(Op::from_json(&format!(r#"{{"op":"move_faces","group":"{g}","faces":[1],"x":1}}"#)).is_err());
}

// ---- a mesh file with no groups ---------------------------------------------------------------

const BOX: &str = "v 0 0 0\nv 6 0 0\nv 6 10 0\nv 0 10 0\nv 0 0 3\nv 6 0 3\nv 6 10 3\nv 0 10 3\n\
f 1 4 3 2\nf 5 6 7 8\nf 1 2 6 5\nf 3 4 8 7\nf 1 5 8 4\nf 2 3 7 6\n";

#[test]
fn a_groupless_obj_imports_as_one_group_named_for_the_file() {
    let opts = ImportOptions::new(Unit::Metre, Up::Z);
    let m = import::read_obj(BOX.as_bytes(), &opts).unwrap();
    assert!(m.report.ungrouped);
    assert_eq!(m.group_names, ["default"], "the reader's own name is unchanged");
    let p = m.to_project("box_blank");
    assert_eq!(p.surface_groups.len(), 1);
    assert_eq!(p.surface_groups[0].name, "box_blank");
    assert_eq!(p.geometry.faces.len(), 12);
    assert_eq!(m.to_project("  ").surface_groups[0].name, "Surfaces");
    // The ids are the model's: the name given changes no id.
    assert_eq!(m.to_project("a").surface_groups[0].id, m.to_project("b").surface_groups[0].id);
    p.check_integrity().unwrap();

    // `g` statements, `o`, or `usemtl` are groups: not ungrouped, and their names are kept.
    for (head, name) in [("g walls\n", "walls"), ("o room\n", "room"), ("usemtl plaster\n", "plaster")] {
        let m = import::read_obj(format!("{head}{BOX}").as_bytes(), &opts).unwrap();
        assert!(!m.report.ungrouped, "{head}");
        assert_eq!(m.to_project("box").surface_groups[0].name, name);
    }
    // Faces before the first `g` and after it: grouped (two groups, `default` and `walls`).
    let text = BOX.replacen("f 1 2 6 5", "g walls\nf 1 2 6 5", 1);
    let m = import::read_obj(text.as_bytes(), &opts).unwrap();
    assert!(!m.report.ungrouped);
    assert_eq!(m.group_names, ["default", "walls"]);
}

#[test]
fn a_ply_without_layers_and_a_binary_stl_are_ungrouped() {
    let opts = ImportOptions::new(Unit::Metre, Up::Z);
    let ply = "ply\nformat ascii 1.0\nelement vertex 4\nproperty float x\nproperty float y\nproperty float z\n\
element face 4\nproperty list uchar int vertex_indices\nend_header\n0 0 0\n1 0 0\n0 1 0\n0 0 1\n\
3 0 2 1\n3 0 1 3\n3 0 3 2\n3 1 2 3\n";
    let m = import::read_ply(ply.as_bytes(), &opts).unwrap();
    assert!(m.report.ungrouped);
    assert_eq!(m.to_project("tet").surface_groups[0].name, "tet");

    let stl = "solid\nfacet normal 0 0 0\nouter loop\nvertex 0 0 0\nvertex 1 0 0\nvertex 0 1 0\nendloop\nendfacet\nendsolid\n";
    let m = import::read_stl(stl.as_bytes(), &opts).unwrap();
    assert!(m.report.ungrouped, "one unnamed ASCII solid");
    let named = stl.replacen("solid\n", "solid hall\n", 1);
    let m = import::read_stl(named.as_bytes(), &opts).unwrap();
    assert!(!m.report.ungrouped, "a named solid is a group");
    assert_eq!(m.to_project("x").surface_groups[0].name, "hall");
}

// ---- upstream's reference spectra on the project's bands ----------------------------------------

#[test]
fn reference_spectra_map_onto_third_octave_and_octave_bands() {
    let thirds = BandSet::range(BandKind::ThirdOctave, 50, 20000).unwrap();
    let octaves = BandSet::range(BandKind::Octave, 125, 4000).unwrap();
    assert_eq!(reference_spectrum(1).unwrap().shape_on(&octaves), Some(SpectrumShape::Pink));
    assert_eq!(reference_spectrum(0).unwrap().shape_on(&octaves), Some(SpectrumShape::White));

    let es_vl = reference_spectrum(2).unwrap();
    let Some(SpectrumShape::Custom { relative_db }) = es_vl.shape_on(&thirds) else {
        panic!("ES_VL is a custom shape")
    };
    assert_eq!(relative_db.len(), 27);
    for (i, v) in relative_db.iter().enumerate() {
        assert_eq!(v.get(), widen_f32(es_vl.band_db[i]), "band {i}");
    }
    assert_eq!(relative_db[13].get(), 77.7, "1 kHz is the f32's shortest decimal");

    // An octave band is the energy sum of its three thirds: 1 kHz spans 800, 1000, 1250 Hz.
    let Some(SpectrumShape::Custom { relative_db }) = es_vl.shape_on(&octaves) else {
        panic!()
    };
    assert_eq!(relative_db.len(), 6);
    let sum = |a: f64, b: f64, c: f64| {
        10.0 * (10f64.powf(a / 10.0) + 10f64.powf(b / 10.0) + 10f64.powf(c / 10.0)).log10()
    };
    assert!((relative_db[3].get() - sum(75.6, 77.7, 76.3)).abs() < 1e-9);

    // Upstream's two noises as stored agree with the shapes they map to, within its rounding
    // (two decimals in appconst.xml).
    for (id, shape) in [(1, SpectrumShape::Pink), (0, SpectrumShape::White)] {
        let r = reference_spectrum(id).unwrap();
        let ours = Spectrum::new(0.0, shape).band_levels_db(&thirds).unwrap();
        for (a, &b) in ours.iter().zip(&r.band_db) {
            assert!((a - f64::from(b)).abs() < 0.01, "{}: {a} vs {b}", r.name);
        }
    }
    // Every reference spectrum maps onto both band sets.
    for r in &REFERENCE_SPECTRA {
        assert!(r.shape_on(&thirds).is_some() && r.shape_on(&octaves).is_some(), "{}", r.name);
    }
}
