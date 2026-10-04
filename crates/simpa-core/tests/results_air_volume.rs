//! Backlog 85: the report's room volume is the air's. Tutorial 1's 6 x 10 x 3 m box (180 m³)
//! with a closed 2 x 1 x 0.5 m box inside it (1 m³), as BRAS CR2's radiators sit in its room,
//! meshed by the real `tetgen.exe` (`mesh_support.rs`) and exported as `simpa run` exports a
//! run's inputs (`run::manager::export`). The air is 179 m³, known without the mesh; the room,
//! SPPS's reference and TCR's analytic times all take it, and the room reports the 1 m³ left
//! out. Say-NO: the box without the obstacle reads 180 m³, the sum of every tetrahedron, as
//! every version before 14 read it.

#[path = "mesh_support.rs"]
mod support;

use std::path::{Path, PathBuf};

use simpa_core::config_xml::{self, names};
use simpa_core::formats::{cbin, mbin};
use simpa_core::geometry::check;
use simpa_core::mesh::{MeshStatus, TetgenMesher, mesh_project};
use simpa_core::params::room::{RtConstant, Surface, sabine_rt};
use simpa_core::process::{CancelToken, Line};
use simpa_core::results::reference::{self, Reference};
use simpa_core::results::room::{self, Room};
use simpa_core::results::tcr::{self, Analytic};
use simpa_core::run::expect::Expectation;
use simpa_core::schema::{Face, Project, SolverKind, Vec3};
use support::{load_room, scratch, tetgen_exe, volume_by_id};

/// The obstacle's corners: 2 x 1 x 0.5 m, every coordinate exact in `f32`, clear of the source
/// (3, 5, 1.8) and both receivers.
const OBSTACLE: ([f64; 3], [f64; 3]) = ([2.0, 4.0, 0.5], [4.0, 5.0, 1.0]);

/// Adds the closed box `min`-`max` as 12 faces of the walls' group, wound outward (out of the
/// box, into the room), as `geometry::check` requires of a nested shell.
fn with_obstacle(mut p: Project, (min, max): ([f64; 3], [f64; 3])) -> Project {
    let base = p.geometry.vertices.len() as u32;
    for k in 0..8usize {
        let at_max = [matches!(k & 3, 1 | 2), matches!(k & 3, 2 | 3), k >= 4];
        p.geometry.vertices.push(Vec3::from(
            [0, 1, 2].map(|a| if at_max[a] { max[a] } else { min[a] }),
        ));
    }
    let group = p.surface_groups[0].id;
    for t in [
        [0, 2, 1],
        [0, 3, 2],
        [4, 5, 6],
        [4, 6, 7],
        [0, 1, 5],
        [0, 5, 4],
        [1, 2, 6],
        [1, 6, 5],
        [2, 3, 7],
        [2, 7, 6],
        [3, 0, 4],
        [3, 4, 7],
    ] {
        p.geometry.faces.push(Face {
            vertices: t.map(|c: u32| base + c),
            group,
        });
    }
    p
}

/// `p` meshed and exported for `solver` into a fresh `solve/`, with the mesh read back.
fn exported(label: &str, p: &Project, solver: SolverKind) -> (PathBuf, mbin::Mesh) {
    let dir = scratch(label);
    let mesh_dir = dir.join("mesh");
    let m = mesh_project(
        p,
        &mesh_dir,
        &TetgenMesher::new(tetgen_exe()),
        &CancelToken::new(),
        &mut |_: &Line| {},
    )
    .unwrap();
    assert_eq!(m.status, MeshStatus::Ok, "{m:#?}");
    let solve = dir.join("solve");
    std::fs::create_dir(&solve).unwrap();
    config_xml::write_file(p, solver, None, &solve).unwrap();
    cbin::write_file(
        &config_xml::scene_mesh(p).unwrap(),
        &solve.join(names::SCENE_MESH),
    )
    .unwrap();
    std::fs::copy(
        mesh_dir.join(names::TETRA_MESH),
        solve.join(names::TETRA_MESH),
    )
    .unwrap();
    let mesh = mbin::read_file(&solve.join(names::TETRA_MESH)).unwrap();
    (solve, mesh)
}

/// The room's volume and the volume it leaves out.
fn room_volumes(solve: &Path, solver: SolverKind) -> (f64, f64) {
    let exp = Expectation::read(solve, solver).unwrap();
    match room::room(solve, &exp) {
        Room::Computed {
            volume_m3,
            obstacle_volume_m3,
            ..
        } => (volume_m3, obstacle_volume_m3),
        Room::NotComputed { why } => panic!("{why}"),
    }
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-6 * b.max(1.0)
}

#[test]
fn the_room_s_volume_is_its_air_without_a_closed_obstacle_s_inside() {
    let p = with_obstacle(load_room("tutorial1_box.simpa"), OBSTACLE);
    // The geometry check: the air is the room's cell, the obstacle a nested cell of 1 m³, and
    // the signed volume counts the obstacle with its own sign (180 + 1), not the air.
    let c = check::check(&p.geometry);
    assert!(c.is_ok(), "{:?}", c.reasons);
    assert!(close(c.measures.air_volume_m3, 179.0), "{:?}", c.measures);
    assert!(
        close(c.measures.enclosed_volume_m3, 180.0),
        "{:?}",
        c.measures
    );
    assert!(
        close(c.measures.signed_volume_m3, 181.0),
        "{:?}",
        c.measures
    );

    let (solve, mesh) = exported("air-obstacle-tcr", &p, SolverKind::Tcr);
    // TetGen meshed the obstacle's inside as a region of its own: 1 m³ of the 180 the mesh fills,
    // which is what the report summed before version 14.
    let by_id = volume_by_id(&mesh);
    assert_eq!(by_id.len(), 2, "{by_id:?}");
    assert!(close(by_id.values().sum::<f64>(), 180.0), "{by_id:?}");
    assert!(by_id.values().any(|&v| close(v, 1.0)), "{by_id:?}");
    let air = room::air_tetrahedra(&mesh);
    assert!(air.iter().any(|&a| a) && air.iter().any(|&a| !a));

    let (volume, obstacle) = room_volumes(&solve, SolverKind::Tcr);
    assert!(close(volume, 179.0), "room volume {volume}");
    assert!(close(obstacle, 1.0), "obstacle volume {obstacle}");

    // TCR's analytic Sabine takes the air: its time is Sabine's at 179 m³, not at 180.
    let exp = Expectation::read(&solve, SolverKind::Tcr).unwrap();
    let Analytic::Computed {
        volume_m3,
        area_m2,
        bands,
    } = tcr::analytic(&solve, &exp)
    else {
        panic!("analytic not computed")
    };
    assert!(close(volume_m3, 179.0), "{volume_m3}");
    assert!(close(area_m2, 216.0 + 7.0), "{area_m2}");
    // Sabine's time at the band's own absorption area: at 179 m³, and not at 180.
    let Room::Computed { groups, .. } = room::room(&solve, &exp) else {
        panic!("room not computed")
    };
    let b = &bands[0];
    let a: f64 = groups.iter().map(|g| g.area_m2 * g.absorption[0].1).sum();
    let at = |v: f64| {
        sabine_rt(
            v,
            &[Surface {
                area_m2: a,
                absorption: 1.0,
            }],
            b.air_m_per_metre,
            RtConstant::Tcr,
        )
        .unwrap()
    };
    let t = *b.sabine_s.as_ref().unwrap();
    assert!(close(t, at(179.0)), "{t} vs {}", at(179.0));
    assert!(
        !close(t, at(180.0)),
        "{t} is Sabine's at the enclosed volume"
    );

    // SPPS's reference takes the air too.
    let (solve, _) = exported("air-obstacle-spps", &p, SolverKind::Spps);
    let exp = Expectation::read(&solve, SolverKind::Spps).unwrap();
    match reference::reference(&solve, &exp, 343.0, false) {
        Reference::Computed { volume_m3, .. } => assert!(close(volume_m3, 179.0), "{volume_m3}"),
        Reference::NotComputed { why } => panic!("{why}"),
    }
}

#[test]
fn a_room_without_an_obstacle_reads_the_volume_every_tetrahedron_gives() {
    let p = load_room("tutorial1_box.simpa");
    let c = check::check(&p.geometry);
    assert!(close(c.measures.air_volume_m3, 180.0), "{:?}", c.measures);
    assert!(
        close(c.measures.enclosed_volume_m3, 180.0),
        "{:?}",
        c.measures
    );
    let (solve, mesh) = exported("air-plain", &p, SolverKind::Tcr);
    let every: f64 = volume_by_id(&mesh).values().sum();
    assert!(room::air_tetrahedra(&mesh).iter().all(|&a| a));
    let (volume, obstacle) = room_volumes(&solve, SolverKind::Tcr);
    assert_eq!(volume, every, "the same sum as before version 14");
    assert!(close(volume, 180.0), "{volume}");
    assert_eq!(obstacle, 0.0);
}

/// The rule on its own, with no mesher: a mesh of one region is all air, whatever it holds, as
/// upstream's meshes without region attributes are; relabel the tetrahedra that touch no outer
/// face and they are left out.
#[test]
fn a_region_that_reaches_no_outer_face_is_left_out() {
    let faces = |t: &mbin::Tetrahedron| {
        let v = t.vertices;
        [
            [v[1], v[2], v[3]],
            [v[0], v[2], v[3]],
            [v[0], v[1], v[3]],
            [v[0], v[1], v[2]],
        ]
        .map(|mut f| {
            f.sort_unstable();
            f
        })
    };
    let mut tried = 0;
    for seed in 0..400u64 {
        let mesh = mbin::generate(seed);
        assert!(
            room::air_tetrahedra(&mesh).iter().all(|&a| a),
            "seed {seed}"
        );
        let mut uses = std::collections::HashMap::new();
        for t in &mesh.tetrahedra {
            for f in faces(t) {
                *uses.entry(f).or_insert(0) += 1;
            }
        }
        // The tetrahedra none of whose faces is on the outer surface: a 3 x 3 x 3 box's middle
        // cell, the stand-in for an obstacle's inside.
        let inner: Vec<bool> = mesh
            .tetrahedra
            .iter()
            .map(|t| faces(t).iter().all(|f| uses[f] == 2))
            .collect();
        if !inner.iter().any(|&i| i) {
            continue;
        }
        tried += 1;
        let mut relabelled = mesh.clone();
        for (t, &i) in relabelled.tetrahedra.iter_mut().zip(&inner) {
            if i {
                t.id_volume += 1000;
            }
        }
        let air = room::air_tetrahedra(&relabelled);
        for (k, (&a, &i)) in air.iter().zip(&inner).enumerate() {
            assert_eq!(a, !i, "seed {seed}, tetrahedron {k}");
        }
    }
    assert!(
        tried > 0,
        "no seed gave a tetrahedron off the outer surface"
    );
}
