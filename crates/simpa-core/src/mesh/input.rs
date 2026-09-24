//! What TetGen is given: the `.poly` and `.var` built from a [`Project`] or taken from a raw
//! `.poly`, and the `.cbin` scene the `.mbin` markers will index.

use serde::{Deserialize, Serialize};

use super::flags::to_string_g15;
use super::verify::VolumeIds;
use crate::config_xml::{self, GlFrame, SolverIds};
use crate::formats::{cbin, poly};
use crate::mesh::verify::DRAWN_ZONE_TRIANGLES;
use crate::schema::{FittingShape, Project, SurfaceReceiverShape, Vec3};

/// What a `.poly` marker at or above the scene's face count stands for: one triangle of a `Box`
/// fitting zone (`docs/m5-m6-design.md`, decision 5).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ZoneFacets {
    pub zone: String,
    pub solver_id: i32,
    /// The first marker of the zone's 12 triangles; they run `first_marker..first_marker + 12`.
    pub first_marker: u32,
}

/// One enabled fitting zone, as the region volume check needs it (`mesh::verify`, "Regions").
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FittingRegion {
    pub zone: String,
    pub solver_id: i32,
    /// The region seed the `.poly` carries, as written.
    pub seed: [f32; 3],
    /// A `Box` zone's centre, `(min + max) / 2`: a point strictly inside it, whatever its seed.
    pub box_centre: Option<[f64; 3]>,
    /// A `Surfaces` zone's scene faces (`.cbin` indices, ascending): the faces of its groups.
    pub faces: Vec<u32>,
}

/// Everything the mesher writes before it runs TetGen.
#[derive(Clone, Debug, PartialEq)]
pub struct MeshInput {
    /// `scene_mesh.poly`: facet `i` of the scene carries marker `i`; box-zone triangles follow,
    /// in the facet list ([`project_input`] without preprocessing) or in the Part 5 user facet
    /// list, as upstream's GUI writes them for `preprocess.exe` (with it).
    pub poly: poly::Model,
    /// `scene_mesh.var`, when the settings ask for a facet-area constraint.
    pub var: Option<Vec<u8>>,
    /// The markers the `.var` constrains, in file order.
    pub var_markers: Vec<u32>,
    /// `mesh.cbin`: the scene the `.mbin` markers index. For a project, the room's faces
    /// ([`config_xml::room_mesh`]); a run's `mesh.cbin` ([`config_xml::scene_mesh`]) adds the
    /// drawn box zones' triangles after them, which no marker names (decision 5).
    pub scene: cbin::Model,
    /// The ids the `.mbin` may carry: the seeded fitting ids, and the room's parts numbered by
    /// TetGen above them ([`VolumeIds::tetgen`]).
    pub volume_ids: VolumeIds,
    /// Per scene face, the name of its surface group (empty for a raw `.poly`).
    pub face_groups: Vec<String>,
    /// The box-zone triangles, in marker order.
    pub zone_facets: Vec<ZoneFacets>,
    /// Remarks the manifest records (for example, markers a raw `.poly` had that were replaced).
    pub notes: Vec<String>,
    /// Whether the `.poly` goes through upstream's `preprocess.exe` before TetGen (the project's
    /// `MeshSettings::preprocess`; never for a raw `.poly`).
    pub preprocess: bool,
    /// How many of `scene`'s vertices, from the first, upstream's `UnitizeVar` is fitted to: the
    /// room's ([`config_xml::room_mesh`]), never the drawn zones' after them.
    pub frame_vertices: usize,
    /// The enabled fitting zones, in solver-id order: the regions the `.poly` seeds.
    pub fittings: Vec<FittingRegion>,
}

/// Why no input could be built.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InputError(pub String);

impl std::fmt::Display for InputError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

fn narrow(v: Vec3, what: &str) -> Result<[f32; 3], InputError> {
    let p = v.to_array().map(|c| c as f32);
    if p.iter().all(|c| c.is_finite()) {
        Ok(p)
    } else {
        Err(InputError(format!(
            "{what} is not finite as a 32-bit float"
        )))
    }
}

/// Which of a box's bounds corner `k` takes on each axis (true: max). Corners 0-3 go round the
/// bottom, (x0,y0) (x1,y0) (x1,y1) (x0,y1); corners 4-7 are the same round the top.
fn box_corner(k: usize) -> [bool; 3] {
    [matches!(k & 3, 1 | 2), matches!(k & 3, 2 | 3), k >= 4]
}

/// The 12 triangles of a box over [`box_corner`]'s numbering, wound to face outward.
const BOX_TRIANGLES: [[usize; 3]; 12] = [
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
];

/// Builds the mesher input for `project`. Common to both layouts:
/// - the `.var` when `surface_receiver_max_area_m2` is set ([`var_bytes`]);
/// - per enabled fitting zone, one region, attribute = its solver id, no volume bound (-1),
///   written as upstream's `ExportPOLY` writes a region (`poly.cpp:220-231`);
/// - the flags' `-q` and `-a` values must be above 0 and finite as `f32`.
///
/// Without preprocessing (`MeshSettings::preprocess` off; `docs/m5-m6-design.md`, decision 5):
/// - vertices taken from [`config_xml::room_mesh`] (narrowed to `f32`, then through upstream's
///   OpenGL round trip in the scene's [`GlFrame`]) and written back as `f64`, so the `.poly`
///   holds exactly the `.cbin`'s values, as upstream's `_SavePOLY` and `ToCBINFormat` write the
///   same `f32` vertices (`Objet3D_maillage.cpp:777, 941-942`);
/// - faces in project order, facet marker = face index = `.cbin` face index, `saveFaceIndex` on;
/// - per enabled `Box` fitting zone, its 8 corners and 12 triangles in the facet list (Part 2,
///   not upstream's Part 5 user list), markers `scene_faces + k`. Each corner coordinate is
///   narrowed and taken through the same round trip as the scene's vertices, as upstream takes a
///   drawn box's corners (in, `e_scene_encombrements_encombrement_cuboide.h:180`; out,
///   `Objet3D_maillage.cpp:984-990`). The round trip works coordinate by coordinate, so a box
///   face flush with a wall stays exactly in that wall's plane;
/// - regions in project order, a box seeded at its centre, a `Surfaces` zone at its
///   `inside_point`, narrowed to `f32`.
///
/// With preprocessing, upstream's GUI layout for `preprocess.exe`: [`preprocess_layout`].
pub fn project_input(project: &Project) -> Result<MeshInput, InputError> {
    let settings = &project.solvers.meshing;
    // The flags carry these as the f32 upstream holds (`flags::setting_g15`).
    let q = settings.min_radius_edge_ratio.get();
    if !((q as f32).is_finite() && q as f32 > 0.0) {
        return Err(InputError(format!(
            "the radius-edge ratio (-q) is {q}; it must be above 0 and finite as a 32-bit float"
        )));
    }
    if let Some(a) = settings.max_volume_m3.map(|a| a.get())
        && !((a as f32).is_finite() && a as f32 > 0.0)
    {
        return Err(InputError(format!(
            "the maximum tetrahedron volume (-a) is {a} m³; it must be above 0 and finite as a \
             32-bit float"
        )));
    }
    let face_groups = project
        .geometry
        .faces
        .iter()
        .map(|f| {
            project
                .group(f.group)
                .map_or_else(String::new, |g| g.name.clone())
        })
        .collect();
    let (var, var_markers) = match settings.surface_receiver_max_area_m2 {
        None => (None, Vec::new()),
        Some(area) => {
            let area32 = area.get() as f32;
            if !(area32.is_finite() && area32 > 0.0) {
                return Err(InputError(format!(
                    "the surface-receiver area constraint is {area} m²; it must be above 0 and \
                     finite as a 32-bit float"
                )));
            }
            let markers = refined_faces(project);
            (Some(var_bytes(&markers, area32)), markers)
        }
    };
    let layout = if settings.preprocess {
        preprocess_layout(project)?
    } else {
        tetgen_layout(project)?
    };
    let fittings: Vec<i32> = layout.fittings.iter().map(|f| f.solver_id).collect();
    Ok(MeshInput {
        poly: layout.poly,
        var,
        var_markers,
        scene: layout.scene,
        volume_ids: VolumeIds::tetgen(fittings),
        face_groups,
        zone_facets: layout.zone_facets,
        notes: Vec::new(),
        preprocess: settings.preprocess,
        frame_vertices: layout.frame_vertices,
        fittings: layout.fittings,
    })
}

/// What [`tetgen_layout`] and [`preprocess_layout`] build: the `.poly`, the `.cbin` its markers
/// index, and what the manifest and the region volume check need.
#[derive(Clone, Debug, PartialEq)]
pub struct Layout {
    pub poly: poly::Model,
    pub scene: cbin::Model,
    pub zone_facets: Vec<ZoneFacets>,
    pub frame_vertices: usize,
    pub fittings: Vec<FittingRegion>,
}

/// The scene faces of a `Surfaces` zone's groups, ascending.
fn zone_faces(project: &Project, groups: &[crate::schema::GroupId]) -> Vec<u32> {
    project
        .geometry
        .faces
        .iter()
        .enumerate()
        .filter(|(_, f)| groups.contains(&f.group))
        .map(|(i, _)| i as u32)
        .collect()
}

/// A box zone's bounds as the mesher checks them: `f32`, `min` below `max` on every axis.
fn box_bounds(
    z: &crate::schema::FittingZone,
    i: usize,
    min: Vec3,
    max: Vec3,
) -> Result<([f32; 3], [f32; 3]), InputError> {
    let what = |s: &str| format!("fitting zone '{}' {s}", z.name);
    let (lo, hi) = (narrow(min, &what("min"))?, narrow(max, &what("max"))?);
    if (0..3).any(|a| lo[a] >= hi[a]) {
        return Err(InputError(format!(
            "fitting zone '{}' (/fitting_zones/{i}) is an empty box: min {lo:?} is not below max \
             {hi:?} on every axis as 32-bit floats",
            z.name
        )));
    }
    Ok((lo, hi))
}

/// The layout without preprocessing (decision 5): see [`project_input`].
fn tetgen_layout(project: &Project) -> Result<Layout, InputError> {
    let scene =
        config_xml::room_mesh(project).map_err(|e| InputError(format!("scene mesh: {e}")))?;
    // The frame room_mesh took the scene's vertices through.
    let frame = GlFrame::of_project(project);
    let ids = SolverIds::assign(project).map_err(|e| InputError(format!("solver ids: {e}")))?;
    let mut model_vertices: Vec<[f64; 3]> = scene
        .vertices
        .iter()
        .map(|v| [v.x, v.y, v.z].map(f64::from))
        .collect();
    let mut model_faces: Vec<poly::Face> = project
        .geometry
        .faces
        .iter()
        .enumerate()
        .map(|(i, f)| poly::Face {
            vertices: f.vertices,
            face_index: i as u32,
        })
        .collect();
    let scene_faces = model_faces.len();
    let mut model_regions = Vec::new();
    let mut zone_facets = Vec::new();
    let mut fittings = Vec::new();
    for (i, z) in project.fitting_zones.iter().enumerate() {
        if !z.enabled {
            continue;
        }
        let id = ids.fitting_zone_id(z.id).expect("every zone has an id");
        let what = |s: &str| format!("fitting zone '{}' {s}", z.name);
        let (seed, box_centre, faces) = match &z.shape {
            FittingShape::Box { min, max, .. } => {
                let (lo, hi) = (narrow(*min, &what("min"))?, narrow(*max, &what("max"))?);
                let (lo, hi) = match &frame {
                    Some(f) => (f.round_trip(lo), f.round_trip(hi)),
                    None => (lo, hi),
                };
                if (0..3).any(|a| lo[a] >= hi[a]) {
                    return Err(InputError(format!(
                        "fitting zone '{}' (/fitting_zones/{i}) is an empty box: min {lo:?} is \
                         not below max {hi:?} on every axis as 32-bit floats, as the mesher \
                         writes them",
                        z.name
                    )));
                }
                let base = u32::try_from(model_vertices.len())
                    .map_err(|_| InputError("too many vertices".to_string()))?;
                for k in 0..8 {
                    let at_max = box_corner(k);
                    model_vertices
                        .push([0, 1, 2].map(|a| f64::from(if at_max[a] { hi[a] } else { lo[a] })));
                }
                let first_marker = u32::try_from(model_faces.len())
                    .map_err(|_| InputError("too many facets".to_string()))?;
                for (t, tri) in BOX_TRIANGLES.iter().enumerate() {
                    model_faces.push(poly::Face {
                        vertices: tri.map(|c| base + c as u32),
                        face_index: first_marker + t as u32,
                    });
                }
                zone_facets.push(ZoneFacets {
                    zone: z.name.clone(),
                    solver_id: id,
                    first_marker,
                });
                // The centre, from the corners as written, narrowed again.
                let centre = [0, 1, 2].map(|a| (f64::from(lo[a]) + f64::from(hi[a])) / 2.0);
                (centre.map(|c| c as f32), Some(centre), Vec::new())
            }
            FittingShape::Surfaces {
                inside_point,
                groups,
            } => (
                narrow(*inside_point, &what("inside_point"))?,
                None,
                zone_faces(project, groups),
            ),
        };
        model_regions.push(poly::Region {
            region_index: id,
            dot_in_region: seed,
            region_refinement: -1.0,
        });
        fittings.push(FittingRegion {
            zone: z.name.clone(),
            solver_id: id,
            seed,
            box_centre,
            faces,
        });
    }
    debug_assert!(
        zone_facets
            .iter()
            .all(|z| z.first_marker as usize >= scene_faces)
    );
    let frame_vertices = scene.vertices.len();
    Ok(Layout {
        poly: poly::Model {
            save_face_index: true,
            user_defined_faces: Vec::new(),
            model_faces,
            model_vertices,
            model_regions,
        },
        scene,
        zone_facets,
        frame_vertices,
        fittings,
    })
}

/// Upstream's `BARELY_EPSILON`, `(decimal)0.0001` with `decimal` = `float`
/// (`lib_interface/Core/mathlib.h:54, 58`).
const BARELY_EPSILON: f32 = 0.0001;

/// A rectangular zone's region seed as upstream's GUI computes it, `dotInsideVol = posFin -
/// (posFin - posDeb) * BARELY_EPSILON` from the corners `ba` (`posDeb`) and `hc` (`posFin`) as
/// stored, unordered (`e_scene_encombrements_encombrement_cuboide.h:333-335`), every operation
/// per component in `f32` (`vec3` is `base_vec3<float>`, `mathlib.h:109, 120, 239`): a point just
/// inside the `hc` corner.
pub fn upstream_box_seed(ba: [f32; 3], hc: [f32; 3]) -> [f32; 3] {
    [0, 1, 2].map(|k| hc[k] - (hc[k] - ba[k]) * BARELY_EPSILON)
}

/// The layout upstream's GUI writes for `preprocess.exe`, `CObjet3D::_SavePOLY(path, true,
/// doMeshRepair = true, true, ...)` (`Objet3D_maillage.cpp:931-1044`, called at
/// `projet_maillage.cpp:206`):
/// - the scene is [`config_xml::scene_mesh`]: the room's faces, then each enabled box zone's 12
///   triangles with three vertices of their own (`ToCBINFormat`), so every `.poly` marker is a
///   `.cbin` face index, box triangles included;
/// - nodes: that scene's vertices, widened to `f64` (`vec3_to_dvec3`, `:939-943, 984-990`);
/// - Part 2, the facet list: the room's faces, marker = face index (`:950-965`);
/// - Part 5, the user facet list: the box triangles, marker = their `.cbin` face index, each on
///   its own three nodes (`:968-1001`);
/// - Part 4, the regions (`:1003-1037`): one per enabled zone, attribute = its solver id (where
///   upstream writes its element id, `xmlIdElement`, `:1009`; storing upstream's ids is Burhan's
///   open decision, so ours are compared through the recorded id map), refinement -1
///   (`drawable_element.h:101`). A box is seeded at [`upstream_box_seed`] of its corners as
///   upstream holds them ([`FittingShape::box_corners`]); a `Surfaces` zone at its
///   `inside_point` (upstream's `volpos`, `e_scene_encombrements_encombrement_model.h:177`),
///   narrowed to `f32` and written as it is, even on one of the zone's faces (tutorial 3's zone 1).
///   The user facets are in project order and the lines in ascending solver id. Upstream writes
///   each drawable's triangles and then its region, drawable by drawable in its table's order, a
///   wx hash map keyed by element id (`appconfig.h:52`) that this layout does not reproduce. With
///   one box zone the files agree (tutorial 3, byte for byte); with two or more, `preprocess.exe`
///   may meet the user facets in another order, which can change its splits
///   (`docs/formats/mesh-manifest.md`, "Order, a known difference").
pub fn preprocess_layout(project: &Project) -> Result<Layout, InputError> {
    let scene =
        config_xml::scene_mesh(project).map_err(|e| InputError(format!("scene mesh: {e}")))?;
    let room_faces = project.geometry.faces.len();
    let frame_vertices = project.geometry.vertices.len();
    let ids = SolverIds::assign(project).map_err(|e| InputError(format!("solver ids: {e}")))?;
    let model_vertices: Vec<[f64; 3]> = scene
        .vertices
        .iter()
        .map(|v| [v.x, v.y, v.z].map(f64::from))
        .collect();
    let face = |(i, f): (usize, &cbin::Face)| poly::Face {
        vertices: [f.a, f.b, f.c],
        face_index: i as u32,
    };
    let model_faces: Vec<poly::Face> = scene.faces[..room_faces]
        .iter()
        .enumerate()
        .map(face)
        .collect();
    let user_defined_faces: Vec<poly::Face> = scene
        .faces
        .iter()
        .enumerate()
        .skip(room_faces)
        .map(face)
        .collect();
    let mut zone_facets = Vec::new();
    let mut fittings = Vec::new();
    let mut next_marker = room_faces;
    for (i, z) in project.fitting_zones.iter().enumerate() {
        if !z.enabled {
            continue;
        }
        let id = ids.fitting_zone_id(z.id).expect("every zone has an id");
        let what = |s: &str| format!("fitting zone '{}' {s}", z.name);
        match &z.shape {
            FittingShape::Box { min, max, .. } => {
                let (lo, hi) = box_bounds(z, i, *min, *max)?;
                let (ba, hc) = z.shape.box_corners().expect("a box");
                let (ba, hc) = (narrow(ba, &what("ba"))?, narrow(hc, &what("hc"))?);
                let first_marker = u32::try_from(next_marker)
                    .map_err(|_| InputError("too many facets".to_string()))?;
                next_marker += DRAWN_ZONE_TRIANGLES;
                zone_facets.push(ZoneFacets {
                    zone: z.name.clone(),
                    solver_id: id,
                    first_marker,
                });
                fittings.push(FittingRegion {
                    zone: z.name.clone(),
                    solver_id: id,
                    seed: upstream_box_seed(ba, hc),
                    box_centre: Some(
                        [0, 1, 2].map(|a| (f64::from(lo[a]) + f64::from(hi[a])) / 2.0),
                    ),
                    faces: Vec::new(),
                });
            }
            FittingShape::Surfaces {
                inside_point,
                groups,
            } => fittings.push(FittingRegion {
                zone: z.name.clone(),
                solver_id: id,
                seed: narrow(*inside_point, &what("inside_point"))?,
                box_centre: None,
                faces: zone_faces(project, groups),
            }),
        }
    }
    if next_marker != scene.faces.len() {
        return Err(InputError(format!(
            "the scene mesh holds {} drawn-zone triangles, the enabled box zones {}",
            scene.faces.len() - room_faces,
            next_marker - room_faces
        )));
    }
    fittings.sort_by_key(|f| f.solver_id);
    let model_regions = fittings
        .iter()
        .map(|f| poly::Region {
            region_index: f.solver_id,
            dot_in_region: f.seed,
            region_refinement: -1.0,
        })
        .collect();
    Ok(Layout {
        poly: poly::Model {
            save_face_index: true,
            user_defined_faces,
            model_faces,
            model_vertices,
            model_regions,
        },
        scene,
        zone_facets,
        frame_vertices,
        fittings,
    })
}

/// The faces a facet-area constraint applies to: every face whose group belongs to an enabled
/// scene surface receiver, in project order (upstream `var.cpp`'s caller,
/// `Objet3D_maillage.cpp:900-929`, takes every face with a surface receiver). The same set as
/// `validate::mesh_input_hash` stamps.
pub fn refined_faces(project: &Project) -> Vec<u32> {
    let groups: Vec<_> = project
        .surface_receivers
        .iter()
        .filter(|r| r.enabled)
        .flat_map(|r| match &r.shape {
            SurfaceReceiverShape::Scene { groups } => groups.as_slice(),
            SurfaceReceiverShape::CuttingPlane { .. } => &[],
        })
        .collect();
    project
        .geometry
        .faces
        .iter()
        .enumerate()
        .filter(|(_, f)| groups.contains(&&f.group))
        .map(|(i, _)| i as u32)
        .collect()
}

/// The `.var` upstream's `CVar::BuildVar` writes (`isimpa/3dengine/Core/var.cpp:54-64`): text
/// mode on Windows, so CRLF; one row `k  marker area` per constrained facet, `k` from 1, the area
/// the `float` upstream holds printed by `Convertor::ToString` at 15 significant digits; no
/// segment constraints, and no newline after the final `0`.
pub fn var_bytes(markers: &[u32], area_m2: f32) -> Vec<u8> {
    let area = to_string_g15(f64::from(area_m2));
    let mut s = format!("# Facet constraints\r\n{}\r\n", markers.len());
    for (k, m) in markers.iter().enumerate() {
        s.push_str(&format!("{}  {m} {area}\r\n", k + 1));
    }
    s.push_str("# Segment constraints\r\n0");
    s.into_bytes()
}

/// The mesher input for a raw `.poly`, whose facets then act as the scene:
/// - vertices narrowed to `f32` (the solvers read nothing wider), faces in file order;
/// - each facet's marker becomes its position, so markers index the `.cbin` built from the same
///   facets (material 0, no receiver, no fitting); a changed marker is noted;
/// - regions kept as they are, their attributes the fitting ids;
/// - the Part 5 user facet list dropped, since TetGen never reads it (noted).
pub fn poly_input(model: &poly::Model) -> Result<MeshInput, InputError> {
    let mut notes = Vec::new();
    let mut vertices = Vec::with_capacity(model.model_vertices.len());
    for (i, v) in model.model_vertices.iter().enumerate() {
        let p = v.map(|c| c as f32);
        if !p.iter().all(|c| c.is_finite()) {
            return Err(InputError(format!(
                "node {} is not finite as a 32-bit float",
                i + 1
            )));
        }
        vertices.push(cbin::Vertex {
            x: p[0],
            y: p[1],
            z: p[2],
        });
    }
    let renumbered = model
        .model_faces
        .iter()
        .enumerate()
        .filter(|(i, f)| f.face_index as usize != *i)
        .count();
    if renumbered > 0 {
        notes.push(format!(
            "{renumbered} facet markers were replaced by the facet's position, so that markers \
             index the scene"
        ));
    }
    if !model.save_face_index {
        notes.push("the input facet list had no markers; each facet's position is used".into());
    }
    if !model.user_defined_faces.is_empty() {
        notes.push(format!(
            "{} facets of the Part 5 user facet list were ignored: TetGen never reads it",
            model.user_defined_faces.len()
        ));
    }
    let faces = model
        .model_faces
        .iter()
        .map(|f| cbin::Face {
            a: f.vertices[0],
            b: f.vertices[1],
            c: f.vertices[2],
            id_mat: 0,
            id_rs: -1,
            id_en: -1,
        })
        .collect();
    let mut fittings: Vec<i32> = model.model_regions.iter().map(|r| r.region_index).collect();
    fittings.sort_unstable();
    fittings.dedup();
    let poly = poly::Model {
        save_face_index: true,
        user_defined_faces: Vec::new(),
        model_faces: model
            .model_faces
            .iter()
            .enumerate()
            .map(|(i, f)| poly::Face {
                vertices: f.vertices,
                face_index: i as u32,
            })
            .collect(),
        model_vertices: vertices
            .iter()
            .map(|v| [v.x, v.y, v.z].map(f64::from))
            .collect(),
        model_regions: model.model_regions.clone(),
    };
    let n = model.model_faces.len();
    let frame_vertices = vertices.len();
    Ok(MeshInput {
        poly,
        var: None,
        var_markers: Vec::new(),
        scene: cbin::Model { faces, vertices },
        volume_ids: VolumeIds::tetgen(fittings),
        face_groups: vec![String::new(); n],
        zone_facets: Vec::new(),
        notes,
        preprocess: false,
        frame_vertices,
        fittings: Vec::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn var_layout_is_var_cpp() {
        let bytes = var_bytes(&[0, 1], 0.1);
        assert_eq!(
            String::from_utf8(bytes).unwrap(),
            "# Facet constraints\r\n2\r\n1  0 0.100000001490116\r\n2  1 0.100000001490116\r\n\
             # Segment constraints\r\n0"
        );
        assert_eq!(
            var_bytes(&[], 5.0),
            b"# Facet constraints\r\n0\r\n# Segment constraints\r\n0"
        );
    }

    #[test]
    fn box_triangles_face_outward() {
        let corner = |k: usize| box_corner(k).map(|at_max| f64::from(u8::from(at_max)));
        let centre = [0.5; 3];
        for t in BOX_TRIANGLES {
            let [a, b, c] = t.map(corner);
            let u = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
            let v = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
            let n = [
                u[1] * v[2] - u[2] * v[1],
                u[2] * v[0] - u[0] * v[2],
                u[0] * v[1] - u[1] * v[0],
            ];
            let out = [a[0] - centre[0], a[1] - centre[1], a[2] - centre[2]];
            let dot = n[0] * out[0] + n[1] * out[1] + n[2] * out[2];
            assert!(dot > 0.0, "{t:?}");
        }
    }
}
