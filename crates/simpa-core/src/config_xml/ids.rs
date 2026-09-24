//! Solver integer ids, assigned from the project's order (see the module docs' table), and the
//! scene mesh that carries them.

use std::collections::{BTreeSet, HashMap};

use super::gl::GlFrame;
use super::write::WriteError;
use crate::formats::cbin;
use crate::schema::{
    FittingShape, FittingZoneId, GroupId, PointReceiverId, Project, SOLVER_INT_MAX,
    SurfaceReceiverId, SurfaceReceiverShape,
};

/// The first material id [`SolverIds::assign`] gives a surface group whose base material pins
/// none. 0 is left out: it is the `.cbin`'s "no material" value, and the solvers look it up once,
/// outside their face loop (`coreinitialisation.cpp:409-434`).
pub const FIRST_ASSIGNED_MATERIAL_ID: u32 = 1;

/// The id of the first fitting zone; the next one is 3, and so on. See the module docs for why
/// fitting ids avoid 0 and 1.
pub const FIRST_FITTING_ID: i32 = 2;

/// The solver ids of one project. See the table in the module docs for the rules.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SolverIds {
    /// Per surface group, in project order: `type_surface@id` and `.cbin` `idMat`.
    pub group_materials: Vec<(GroupId, u32)>,
    /// Per surface receiver, in project order, disabled ones included.
    pub surface_receivers: Vec<(SurfaceReceiverId, i32)>,
    /// Per fitting zone, in project order, disabled ones included.
    pub fitting_zones: Vec<(FittingZoneId, i32)>,
    /// Per point receiver, in project order.
    pub point_receivers: Vec<(PointReceiverId, i32)>,
}

impl SolverIds {
    /// Assigns every solver id. The result depends only on the project's lists, their order and
    /// their pinned ids, and not on the variant, the enabled flags or the names. A pinned id is
    /// kept (a `.proj` import pins upstream's element ids, `docs/m5-m6-design.md`, decision 13);
    /// the others are numbered around the pins ([`assign_kind`]), so a project with no pins gets
    /// the ids it always had. Two items of one kind pinned to one id, a fitting zone pinned to 0,
    /// or a pin above [`SOLVER_INT_MAX`] is [`WriteError::SolverIdClash`]. The project should pass
    /// [`Project::check_integrity`]; [`super::write()`] and [`scene_mesh`] check it first.
    pub fn assign(project: &Project) -> Result<SolverIds, WriteError> {
        let pinned_of = |g: GroupId| -> Option<u32> {
            project
                .group(g)
                .and_then(|g| project.material(g.material))
                .and_then(|m| m.solver_id)
        };
        let mut used: BTreeSet<u32> = project
            .surface_groups
            .iter()
            .filter_map(|g| pinned_of(g.id))
            .collect();
        let mut next = FIRST_ASSIGNED_MATERIAL_ID;
        let mut group_materials = Vec::with_capacity(project.surface_groups.len());
        for g in &project.surface_groups {
            let id = match pinned_of(g.id) {
                Some(pinned) => pinned,
                None => {
                    while used.contains(&next) {
                        next = next.checked_add(1).ok_or(WriteError::TooManyIds {
                            what: "surface groups",
                        })?;
                    }
                    if next > SOLVER_INT_MAX {
                        return Err(WriteError::TooManyIds {
                            what: "surface groups",
                        });
                    }
                    used.insert(next);
                    next
                }
            };
            group_materials.push((g.id, id));
        }
        Ok(SolverIds {
            group_materials,
            surface_receivers: assign_kind(
                "surface receivers",
                project
                    .surface_receivers
                    .iter()
                    .map(|r| (r.id, r.solver_id, r.name.as_str())),
                0,
            )?,
            fitting_zones: assign_kind(
                "fitting zones",
                project
                    .fitting_zones
                    .iter()
                    .map(|z| (z.id, z.solver_id, z.name.as_str())),
                FIRST_FITTING_ID as u32,
            )?,
            point_receivers: assign_kind(
                "point receivers",
                project
                    .point_receivers
                    .iter()
                    .map(|r| (r.id, r.solver_id, r.name.as_str())),
                0,
            )?,
        })
    }

    pub fn material_id(&self, group: GroupId) -> Option<u32> {
        lookup(&self.group_materials, group)
    }

    pub fn surface_receiver_id(&self, id: SurfaceReceiverId) -> Option<i32> {
        lookup(&self.surface_receivers, id)
    }

    pub fn fitting_zone_id(&self, id: FittingZoneId) -> Option<i32> {
        lookup(&self.fitting_zones, id)
    }

    pub fn point_receiver_id(&self, id: PointReceiverId) -> Option<i32> {
        lookup(&self.point_receivers, id)
    }
}

/// The solver ids of one kind's list, in its order: an item's pinned id when it has one; otherwise
/// the smallest id from `base` up that no pin of the list and no earlier item uses. Without pins
/// that is `base` plus the item's position, the numbering of a project made here. Refused
/// ([`WriteError::SolverIdClash`]): two items pinned to one id (a solver looks an id up and takes
/// the first match, `base_core_configuration.cpp:374-391`, so the second would be hidden), a
/// fitting zone pinned to 0 (the solvers' "no fitting", `coreinitialisation.cpp:151-176`), or a
/// pin above [`SOLVER_INT_MAX`].
fn assign_kind<'a, K: Copy>(
    what: &'static str,
    items: impl Iterator<Item = (K, Option<u32>, &'a str)>,
    base: u32,
) -> Result<Vec<(K, i32)>, WriteError> {
    let items: Vec<(K, Option<u32>, &str)> = items.collect();
    let mut pinned: HashMap<u32, &str> = HashMap::new();
    for &(_, pin, name) in &items {
        let Some(id) = pin else { continue };
        let clash = |reason: String| WriteError::SolverIdClash {
            what,
            id,
            reason: format!("'{name}' is pinned to it, {reason}"),
        };
        if id > SOLVER_INT_MAX {
            return Err(clash(format!(
                "above the largest id the solvers read, {SOLVER_INT_MAX}"
            )));
        }
        if what == "fitting zones" && id == 0 {
            return Err(clash(
                "the id the solvers read as no fitting on a tetrahedron".to_string(),
            ));
        }
        if let Some(first) = pinned.insert(id, name) {
            return Err(clash(format!("and so is '{first}'")));
        }
    }
    let mut next = base;
    let mut out = Vec::with_capacity(items.len());
    for (key, pin, _) in items {
        let id = match pin {
            Some(id) => id,
            None => {
                while pinned.contains_key(&next) {
                    next = next.checked_add(1).ok_or(WriteError::TooManyIds { what })?;
                }
                let id = next;
                next = next.checked_add(1).ok_or(WriteError::TooManyIds { what })?;
                id
            }
        };
        let id = i32::try_from(id).map_err(|_| WriteError::TooManyIds { what })?;
        out.push((key, id));
    }
    Ok(out)
}

fn lookup<K: PartialEq, V: Copy>(pairs: &[(K, V)], key: K) -> Option<V> {
    pairs.iter().find(|(k, _)| *k == key).map(|&(_, v)| v)
}

/// For each surface group, the `.cbin` `idRs` of the enabled scene receiver that covers it and
/// the `idEn` of the enabled fitting zone it bounds. A group in two enabled scene receivers, or
/// bounding two enabled fitting zones, cannot be expressed (a face has one of each) and is
/// [`WriteError::GroupInTwo`].
pub(crate) fn group_zone_ids(
    project: &Project,
    ids: &SolverIds,
) -> Result<HashMap<GroupId, (i32, i32)>, WriteError> {
    let mut out: HashMap<GroupId, (i32, i32)> = project
        .surface_groups
        .iter()
        .map(|g| (g.id, (-1, -1)))
        .collect();
    let group_name = |g: GroupId| {
        project
            .group(g)
            .map_or_else(|| g.to_string(), |g| g.name.clone())
    };
    for r in project.surface_receivers.iter().filter(|r| r.enabled) {
        if let SurfaceReceiverShape::Scene { groups } = &r.shape {
            let id = ids
                .surface_receiver_id(r.id)
                .expect("every receiver has an id");
            for &g in groups {
                let slot = &mut out.get_mut(&g).expect("integrity: groups exist").0;
                if *slot != -1 {
                    return Err(WriteError::GroupInTwo {
                        group: group_name(g),
                        what: "enabled scene surface receivers",
                    });
                }
                *slot = id;
            }
        }
    }
    for z in project.fitting_zones.iter().filter(|z| z.enabled) {
        if let FittingShape::Surfaces { groups, .. } = &z.shape {
            let id = ids.fitting_zone_id(z.id).expect("every zone has an id");
            for &g in groups {
                let slot = &mut out.get_mut(&g).expect("integrity: groups exist").1;
                if *slot != -1 {
                    return Err(WriteError::GroupInTwo {
                        group: group_name(g),
                        what: "enabled fitting zones",
                    });
                }
                *slot = id;
            }
        }
    }
    Ok(out)
}

/// The scene mesh (`mesh.cbin`) the solvers read, carrying the same solver ids as
/// [`super::write()`]'s `config.xml`, as upstream's GUI writes it (`CObjet3D::ToCBINFormat`,
/// `Objet3D_maillage.cpp:765-816`):
/// - first the room, [`room_mesh`]: the project's vertices and its faces in order;
/// - then, per enabled [`FittingShape::Box`] zone in project order, its 12 triangles as upstream
///   builds a rectangular zone ([`upstream_box_triangles`]), each with three vertices of its own,
///   `idMat` 0 (upstream's default material, which [`super::write()`] declares), `idRs` -1 and
///   `idEn` the zone's id (`:790-813`). Tutorial 3's box is its faces 88 to 99.
///
/// Upstream appends every drawn element's triangles in the order of its element table
/// (`ELEMENT_REF_TYPE_DRAWABLE`); with more than one box that order is not reproduced here
/// (upstream's tutorials hold one). No `.mbin` marker names a box triangle: the mesher's markers
/// index [`room_mesh`] (`docs/m5-m6-design.md`, decision 5), and `mesh::verify` leaves these
/// triangles out of its coverage check (`mesh::verify::drawn_zone_faces`). It does not depend on
/// the variant.
/// Write it with [`cbin::write_file`].
pub fn scene_mesh(project: &Project) -> Result<cbin::Model, WriteError> {
    let mut model = room_mesh(project)?;
    let ids = SolverIds::assign(project)?;
    let frame = GlFrame::of_project(project);
    for z in project.fitting_zones.iter().filter(|z| z.enabled) {
        let Some((ba, hc)) = z.shape.box_corners() else {
            continue;
        };
        let id = ids.fitting_zone_id(z.id).expect("every zone has an id");
        let narrow = |v: crate::schema::Vec3, corner: &str| -> Result<[f32; 3], WriteError> {
            let p = v.to_array().map(|c| c as f32);
            if p.iter().all(|c| c.is_finite()) {
                Ok(p)
            } else {
                Err(WriteError::NonFinite {
                    what: format!(
                        "fitting zone '{}' corner {corner} (as a 32-bit float)",
                        z.name
                    ),
                })
            }
        };
        let (ba, hc) = (narrow(ba, "ba")?, narrow(hc, "hc")?);
        for triangle in upstream_box_triangles(frame.as_ref(), ba, hc) {
            let first = u32::try_from(model.vertices.len())
                .map_err(|_| WriteError::TooManyIds { what: "vertices" })?;
            for [x, y, z] in triangle {
                if ![x, y, z].iter().all(|c| c.is_finite()) {
                    return Err(WriteError::NonFinite {
                        what: "a fitting zone's triangle vertex (as a 32-bit float)".to_string(),
                    });
                }
                model.vertices.push(cbin::Vertex { x, y, z });
            }
            model.faces.push(cbin::Face {
                a: first,
                b: first + 1,
                c: first + 2,
                id_mat: DRAWN_ZONE_MATERIAL_ID,
                id_rs: -1,
                id_en: id,
            });
        }
    }
    Ok(model)
}

/// The material id upstream gives a drawn fitting zone's triangles in the `.cbin`
/// (`Objet3D_maillage.cpp:810`): its default material, reference material 0.
pub const DRAWN_ZONE_MATERIAL_ID: u32 = 0;

/// Whether [`scene_mesh`] gives this project's scene triangles of drawn zones, which need
/// [`DRAWN_ZONE_MATERIAL_ID`] declared.
pub(crate) fn has_drawn_zones(project: &Project) -> bool {
    project
        .fitting_zones
        .iter()
        .any(|z| z.enabled && matches!(z.shape, FittingShape::Box { .. }))
}

/// A rectangular fitting zone's triangles, in world coordinates, as upstream's GUI makes them for
/// its `.cbin` and `.poly`: its corners `ba` and `hc` taken into OpenGL coordinates
/// (`CommonCoordsToGlCoords`, `e_scene_encombrements_encombrement_cuboide.h:180`), made the lower
/// and upper corner coordinate by coordinate there, the 12 triangles of `BuildModel` built from
/// them (`:113-165`), and each vertex taken back to world coordinates (`GlCoordsToCommonCoords`,
/// `Objet3D_maillage.cpp:800-807`), every step in `f32` ([`GlFrame`]). Equal corners give none,
/// as upstream's `BuildModel` returns before building. Without a frame (a scene with no extent)
/// the unit frame is used: upstream's axes only.
///
/// The OpenGL `z` axis is world `-y`, so upstream's lower corner `BA` there is the world corner
/// with the *largest* `y`: tutorial 3's box from `ba` (13, 4, 0) to `hc` (18, 1, 1.2) builds from
/// `BA` = (13, 4, 0) and `HC` = (18, 1, 1.2) with no swap.
pub fn upstream_box_triangles(
    frame: Option<&GlFrame>,
    ba: [f32; 3],
    hc: [f32; 3],
) -> Vec<[[f32; 3]; 3]> {
    let unit = GlFrame {
        centre: [0.0; 3],
        scale: 1.0,
    };
    let frame = frame.unwrap_or(&unit);
    let (mut lo, mut hi) = (frame.to_gl(ba), frame.to_gl(hc));
    if lo == hi {
        return Vec::new();
    }
    for k in 0..3 {
        // `if (origine.x - destination.x > 0)`: swap that coordinate only.
        if lo[k] - hi[k] > 0.0 {
            std::mem::swap(&mut lo[k], &mut hi[k]);
        }
    }
    let [bx, by, bz] = lo;
    let [hx, hy, hz] = hi;
    // Upstream's names: B* on the lower OpenGL y (world z), H* on the upper.
    let ba_ = [bx, by, bz];
    let hc_ = [hx, hy, hz];
    let ha = [bx, hy, bz];
    let hb = [hx, hy, bz];
    let hd = [bx, hy, hz];
    let bb = [hx, by, bz];
    let bc = [hx, by, hz];
    let bd = [bx, by, hz];
    [
        // The bottom.
        [bc, bd, ba_],
        [bc, ba_, bb],
        // Side ab.
        [ba_, ha, hb],
        [ba_, hb, bb],
        // Side ad.
        [ba_, hd, ha],
        [ba_, bd, hd],
        // Side dc.
        [bd, bc, hc_],
        [hd, bd, hc_],
        // Side cb.
        [bc, bb, hb],
        [hc_, bc, hb],
        // The top.
        [hc_, hb, ha],
        [hd, hc_, ha],
    ]
    .into_iter()
    .map(|t| t.map(|p| frame.to_world(p)))
    .collect()
}

/// The room's scene mesh: the project's vertices and its faces in order, each face with `idMat`
/// = its group's material id, `idRs` = the id of the enabled scene receiver covering its group
/// (-1 for none) and `idEn` = the id of the enabled fitting zone its group bounds (-1 for none).
/// [`scene_mesh`] is this, then the drawn zones' triangles; the mesher's `.poly` markers index
/// this. It does not depend on the variant.
///
/// Each vertex is narrowed to `f32` and then taken through upstream's OpenGL round trip in the
/// scene's [`GlFrame`], as upstream's GUI writes its `.cbin` (`Objet3D_maillage.cpp:777`), so
/// the solver reads the same `f32` bits for the same scene (a world `y` of 0 becomes `-0`, and a
/// coordinate may move by about one unit in the last place of the scene's largest coordinate). A
/// scene with no frame (no vertices, or every vertex at one point) is written narrowed only.
pub fn room_mesh(project: &Project) -> Result<cbin::Model, WriteError> {
    project.check_integrity().map_err(WriteError::Integrity)?;
    let ids = SolverIds::assign(project)?;
    let zones = group_zone_ids(project, &ids)?;
    let frame = GlFrame::of_project(project);
    let vertices = project
        .geometry
        .vertices
        .iter()
        .enumerate()
        .map(|(i, v)| {
            let narrowed = v.to_array().map(|c| c as f32);
            let [x, y, z] = match &frame {
                Some(f) => f.round_trip(narrowed),
                None => narrowed,
            };
            if [x, y, z].iter().all(|c| c.is_finite()) {
                Ok(cbin::Vertex { x, y, z })
            } else {
                Err(WriteError::NonFinite {
                    what: format!("vertex {i} (as a 32-bit float)"),
                })
            }
        })
        .collect::<Result<Vec<_>, _>>()?;
    let faces = project
        .geometry
        .faces
        .iter()
        .map(|f| {
            let [a, b, c] = f.vertices;
            let (id_rs, id_en) = zones[&f.group];
            cbin::Face {
                a,
                b,
                c,
                id_mat: ids.material_id(f.group).expect("every group has an id"),
                id_rs,
                id_en,
            }
        })
        .collect();
    Ok(cbin::Model { faces, vertices })
}
