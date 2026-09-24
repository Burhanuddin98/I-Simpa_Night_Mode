//! What TetGen is given: the `.poly` and `.var` built from a [`Project`] or taken from a raw
//! `.poly`, and the `.cbin` scene the `.mbin` markers will index.

use serde::{Deserialize, Serialize};

use super::flags::to_string_g15;
use super::verify::VolumeIds;
use crate::config_xml::{self, GlFrame, SolverIds};
use crate::formats::{cbin, poly};
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

/// Everything the mesher writes before it runs TetGen.
#[derive(Clone, Debug, PartialEq)]
pub struct MeshInput {
    /// `scene_mesh.poly`: facet `i` of the scene carries marker `i`; box-zone triangles follow.
    pub poly: poly::Model,
    /// `scene_mesh.var`, when the settings ask for a facet-area constraint.
    pub var: Option<Vec<u8>>,
    /// The markers the `.var` constrains, in file order.
    pub var_markers: Vec<u32>,
    /// `mesh.cbin`: the scene the `.mbin` markers index.
    pub scene: cbin::Model,
    /// The ids the `.mbin` may carry: room 0, and the seeded fitting ids.
    pub volume_ids: VolumeIds,
    /// Per scene face, the name of its surface group (empty for a raw `.poly`).
    pub face_groups: Vec<String>,
    /// The box-zone triangles, in marker order.
    pub zone_facets: Vec<ZoneFacets>,
    /// Remarks the manifest records (for example, markers a raw `.poly` had that were replaced).
    pub notes: Vec<String>,
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

/// Builds the mesher input for `project`:
/// - vertices taken from [`config_xml::scene_mesh`] (narrowed to `f32`, then through upstream's
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
/// - per enabled fitting zone, one region: the box centre, or the zone's `inside_point`, narrowed
///   to `f32`, attribute = its solver id, no volume bound (-1);
/// - the `.var` when `surface_receiver_max_area_m2` is set ([`var_bytes`]).
pub fn project_input(project: &Project) -> Result<MeshInput, InputError> {
    let scene =
        config_xml::scene_mesh(project).map_err(|e| InputError(format!("scene mesh: {e}")))?;
    // The frame scene_mesh took the scene's vertices through.
    let frame = GlFrame::of_project(project);
    let ids = SolverIds::assign(project).map_err(|e| InputError(format!("solver ids: {e}")))?;
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
            "the maximum tetrahedron volume (-a) is {a} m³; it must be above 0 and finite as a              32-bit float"
        )));
    }

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
        let seed = match &z.shape {
            FittingShape::Box { min, max } => {
                let (lo, hi) = (narrow(*min, &what("min"))?, narrow(*max, &what("max"))?);
                let (lo, hi) = match &frame {
                    Some(f) => (f.round_trip(lo), f.round_trip(hi)),
                    None => (lo, hi),
                };
                if (0..3).any(|a| lo[a] >= hi[a]) {
                    return Err(InputError(format!(
                        "fitting zone '{}' (/fitting_zones/{i}) is an empty box: min {lo:?} is not \
                         below max {hi:?} on every axis as 32-bit floats, as the mesher writes them",
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
                [0, 1, 2].map(|a| ((f64::from(lo[a]) + f64::from(hi[a])) / 2.0) as f32)
            }
            FittingShape::Surfaces { inside_point, .. } => {
                narrow(*inside_point, &what("inside_point"))?
            }
        };
        model_regions.push(poly::Region {
            region_index: id,
            dot_in_region: seed,
            region_refinement: -1.0,
        });
        fittings.push(id);
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
    debug_assert!(
        zone_facets
            .iter()
            .all(|z| z.first_marker as usize >= scene_faces)
    );
    Ok(MeshInput {
        poly: poly::Model {
            save_face_index: true,
            user_defined_faces: Vec::new(),
            model_faces,
            model_vertices,
            model_regions,
        },
        var,
        var_markers,
        scene,
        volume_ids: VolumeIds { room: 0, fittings },
        face_groups,
        zone_facets,
        notes: Vec::new(),
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
    Ok(MeshInput {
        poly,
        var: None,
        var_markers: Vec::new(),
        scene: cbin::Model { faces, vertices },
        volume_ids: VolumeIds { room: 0, fittings },
        face_groups: vec![String::new(); n],
        zone_facets: Vec::new(),
        notes,
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
