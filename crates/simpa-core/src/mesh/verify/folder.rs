//! The region volume check for a mesh the verifier did not build: [`super::verify_dir`]
//! (`simpa mesh-verify`) and `run-folder`'s pre-launch check hold a folder's `.mbin` to the cells
//! of the geometry the folder itself holds (`docs/m5-m6-design.md`, decision 15).
//!
//! Without it, a folder's regions were held to nothing but TetGen's numbering: any run of room
//! ids without a gap above the fittings passed, a wrong room included. The geometry, in order:
//! - **the folder's `.poly`**, the one TetGen read (`<base>.poly` beside TetGen's output,
//!   `scene_mesh.poly` in a mesh folder, or the folder's only `.poly`), its region lines giving
//!   each fitting's seed;
//! - **else its `.cbin`**, the scene the `.mbin`'s markers index, its vertices welded where their
//!   `f32` coordinates are equal (upstream's `.cbin` holds a copy of each vertex per face), each
//!   fitting found by its faces alone (the faces whose `idEn` is its id: a seed-less zone's cell
//!   is the one its own faces and the outer shell close, [`super::zone_cell`]).
//!
//! A drawn box zone (its 12 triangles in the `.cbin`, `super::drawn_zone_faces`) is held to its
//! box's volume, as the mesher holds it. A geometry the check refuses gives no cells: the regions
//! are then not checked, and [`FolderRegions::unchecked`] says why.

use serde::{Deserialize, Serialize};

use super::{Expectations, Reference, VerifyReport, VolumeIds, drawn_zone_faces, verify_mesh_with};
use crate::formats::{cbin, mbin, poly};
use crate::geometry::check::{self as geometry_check, CheckReport};
use crate::mesh::input::FittingRegion;
use crate::schema::{Face, Geometry, GroupId, Vec3};

/// How a folder's regions were checked.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct FolderRegions {
    /// The file whose cells the regions were held to (`scene_mesh.poly`, `mesh.cbin`); `None`
    /// when they were not checked.
    pub reference: Option<String>,
    /// Why the regions were not checked: no geometry in the folder, or one the check refuses.
    pub unchecked: Option<String>,
    /// The folder's `mesh.json` is the mesher's record of this very `.mbin` (its `files.mbin` is
    /// the `.mbin`'s sha256), with status `OK` and `verify.regions_checked` true: the mesher held
    /// the regions to the geometry TetGen read when it built the file.
    pub proven_by_manifest: bool,
}

/// The geometry a folder's regions are held to, owned, and the fittings found in it.
struct Owned {
    vertices: Vec<[f64; 3]>,
    facets: Vec<[u32; 3]>,
    markers: Vec<u32>,
    check: CheckReport,
    fittings: Vec<FittingRegion>,
}

impl Owned {
    fn reference(&self) -> Reference<'_> {
        Reference {
            vertices: &self.vertices,
            facets: &self.facets,
            markers: &self.markers,
            check: &self.check,
            fittings: &self.fittings,
        }
    }
}

fn geometry_of(vertices: &[[f64; 3]], facets: &[[u32; 3]]) -> Geometry {
    let group = GroupId::from_u128(0);
    Geometry {
        vertices: vertices.iter().map(|&v| Vec3::from(v)).collect(),
        faces: facets
            .iter()
            .map(|&vertices| Face { vertices, group })
            .collect(),
    }
}

/// The fittings `ids` declares, as the region check needs them: each one's scene faces (the
/// `.cbin` faces whose `idEn` is its id), its seed from `seeds` (the `.poly`'s region lines;
/// none from a `.cbin`, and the zone is then found by its faces), and for a drawn box zone its
/// centre and volume from its 12 triangles.
fn fittings_of(scene: &cbin::Model, ids: &VolumeIds, seeds: &[poly::Region]) -> Vec<FittingRegion> {
    let drawn = drawn_zone_faces(scene, ids);
    ids.fittings
        .iter()
        .map(|&id| {
            let faces: Vec<u32> = scene
                .faces
                .iter()
                .enumerate()
                .filter(|(_, f)| f.id_en == id)
                .map(|(i, _)| i as u32)
                .collect();
            let seed = seeds
                .iter()
                .find(|r| r.region_index == id)
                .map_or([f32::NAN; 3], |r| r.dot_in_region);
            let is_box = !faces.is_empty() && faces.iter().all(|&i| drawn[i as usize]);
            let (box_centre, box_volume_m3) = if is_box {
                let (mut lo, mut hi) = ([f64::INFINITY; 3], [f64::NEG_INFINITY; 3]);
                for &i in &faces {
                    let f = &scene.faces[i as usize];
                    for v in [f.a, f.b, f.c] {
                        let p = &scene.vertices[v as usize];
                        for (k, c) in [p.x, p.y, p.z].into_iter().enumerate() {
                            lo[k] = lo[k].min(f64::from(c));
                            hi[k] = hi[k].max(f64::from(c));
                        }
                    }
                }
                (
                    Some([0, 1, 2].map(|k| (lo[k] + hi[k]) / 2.0)),
                    Some((0..3).map(|k| hi[k] - lo[k]).product()),
                )
            } else {
                (None, None)
            };
            FittingRegion {
                zone: format!("encombrement {id}"),
                solver_id: id,
                seed,
                box_centre,
                box_volume_m3,
                faces,
            }
        })
        .collect()
}

/// The geometry of a `.poly`: its nodes and facet list (TetGen never reads the user facet list).
fn from_poly(model: &poly::Model, scene: &cbin::Model, ids: &VolumeIds) -> Owned {
    let facets: Vec<[u32; 3]> = model.model_faces.iter().map(|f| f.vertices).collect();
    let check = geometry_check::check(&geometry_of(&model.model_vertices, &facets));
    Owned {
        markers: model.model_faces.iter().map(|f| f.face_index).collect(),
        fittings: fittings_of(scene, ids, &model.model_regions),
        vertices: model.model_vertices.clone(),
        facets,
        check,
    }
}

/// The geometry of a `.cbin`, its vertices welded where their `f32` coordinates are equal; each
/// facet's marker its face index.
fn from_cbin(scene: &cbin::Model, ids: &VolumeIds) -> Owned {
    let mut index: std::collections::HashMap<[u32; 3], u32> = std::collections::HashMap::new();
    let mut vertices: Vec<[f64; 3]> = Vec::new();
    let mut remap = Vec::with_capacity(scene.vertices.len());
    for v in &scene.vertices {
        // -0 and +0 are one coordinate.
        let key = [v.x, v.y, v.z].map(|c| (c + 0.0).to_bits());
        let next = vertices.len() as u32;
        let k = *index.entry(key).or_insert(next);
        if k == next {
            vertices.push([v.x, v.y, v.z].map(f64::from));
        }
        remap.push(k);
    }
    let facets: Vec<[u32; 3]> = scene
        .faces
        .iter()
        .map(|f| [f.a, f.b, f.c].map(|v| remap.get(v as usize).copied().unwrap_or(u32::MAX)))
        .collect();
    let check = geometry_check::check(&geometry_of(&vertices, &facets));
    Owned {
        markers: (0..scene.faces.len() as u32).collect(),
        fittings: fittings_of(scene, ids, &[]),
        vertices,
        facets,
        check,
    }
}

/// [`super::verify_mesh`] on `mesh` and `scene`, and the region volume check against the
/// geometry the folder holds: `poly` (its name, and its model or why it does not read) when the
/// folder has one, else `scene` itself. The report's `regions_checked` says whether it ran;
/// [`FolderRegions`] names the file, or why not. `cbin_name` names `scene`'s file for the record.
/// A `.poly` that does not read is not replaced by the `.cbin`: the regions are then unchecked.
pub fn verify_with_folder_geometry(
    mesh: &mbin::Mesh,
    scene: &cbin::Model,
    ids: &VolumeIds,
    poly: Option<(&str, Result<&poly::Model, &str>)>,
    cbin_name: &str,
) -> (VerifyReport, FolderRegions) {
    let (owned, from) = match poly {
        Some((name, Ok(model))) => (from_poly(model, scene, ids), name.to_string()),
        Some((name, Err(why))) => {
            let regions = FolderRegions {
                unchecked: Some(format!("{name} does not read ({why})")),
                ..FolderRegions::default()
            };
            return (
                verify_mesh_with(mesh, scene, ids, &Expectations::default()),
                regions,
            );
        }
        None => (from_cbin(scene, ids), cbin_name.to_string()),
    };
    let mut regions = FolderRegions::default();
    let expect = if owned.check.is_ok() && owned.vertices.iter().flatten().all(|c| c.is_finite()) {
        regions.reference = Some(from);
        Expectations {
            unmeshed_faces: None,
            reference: Some(owned.reference()),
        }
    } else {
        let codes: Vec<&str> = owned
            .check
            .reasons
            .iter()
            .map(|r| r.code.as_str())
            .collect();
        regions.unchecked = Some(format!(
            "the geometry check refuses {from} ({}): the folder holds no cells to hold its \
             regions to",
            codes.join(", ")
        ));
        Expectations::default()
    };
    (verify_mesh_with(mesh, scene, ids, &expect), regions)
}
