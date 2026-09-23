//! Solver integer ids, assigned from the project's order (see the module docs' table), and the
//! scene mesh that carries them.

use std::collections::{BTreeSet, HashMap};

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
    /// Assigns every solver id. The result depends only on the project's lists and their order,
    /// and not on the variant, the enabled flags or the names. The project should pass
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
        let index = |what: &'static str, i: usize, base: i32| -> Result<i32, WriteError> {
            i32::try_from(i)
                .ok()
                .and_then(|i| i.checked_add(base))
                .ok_or(WriteError::TooManyIds { what })
        };
        Ok(SolverIds {
            group_materials,
            surface_receivers: project
                .surface_receivers
                .iter()
                .enumerate()
                .map(|(i, r)| Ok((r.id, index("surface receivers", i, 0)?)))
                .collect::<Result<_, WriteError>>()?,
            fitting_zones: project
                .fitting_zones
                .iter()
                .enumerate()
                .map(|(i, z)| Ok((z.id, index("fitting zones", i, FIRST_FITTING_ID)?)))
                .collect::<Result<_, WriteError>>()?,
            point_receivers: project
                .point_receivers
                .iter()
                .enumerate()
                .map(|(i, r)| Ok((r.id, index("point receivers", i, 0)?)))
                .collect::<Result<_, WriteError>>()?,
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

/// The scene mesh (`mesh.cbin`) of a project, carrying the same solver ids as [`super::write()`]'s
/// `config.xml`: the project's vertices narrowed to `f32` and its faces in order, each with
/// `idMat` = its group's material id, `idRs` = the id of the enabled scene receiver covering its
/// group (-1 for none) and `idEn` = the id of the enabled fitting zone its group bounds (-1 for
/// none). It does not depend on the variant. Write it with [`cbin::write_file`].
pub fn scene_mesh(project: &Project) -> Result<cbin::Model, WriteError> {
    project.check_integrity().map_err(WriteError::Integrity)?;
    let ids = SolverIds::assign(project)?;
    let zones = group_zone_ids(project, &ids)?;
    let vertices = project
        .geometry
        .vertices
        .iter()
        .enumerate()
        .map(|(i, v)| {
            let [x, y, z] = v.to_array().map(|c| c as f32);
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
