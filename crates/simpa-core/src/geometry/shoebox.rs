//! Parity G11, upstream's File > New scene (`MainUiFrame::OnCreateModel`, `i_simpa_main.cpp:891-931`):
//! a blank project whose model is a box room of a given width (x), length (y) and height (z), with one
//! corner at the origin, as upstream builds it (`CObjet3D::BuildModel`, `Objet3D.cpp:447`).
//!
//! The room is watertight: 8 corners, 12 triangles, each edge shared by exactly two of them, every
//! triangle facing out of the room, as every model here does. Upstream puts all twelve in one group;
//! here the floor, the ceiling and the walls are three groups, so the usual first edit (a floor and
//! a ceiling of their own) needs no regrouping. Each group has upstream's placeholder material, as a
//! mesh import gives it: Run waits until a material is chosen for every surface. No source, receiver
//! or plane is added.

use crate::schema::{Face, GroupId, MaterialId, Project, SurfaceGroup, Vec3};

/// The largest side accepted, metres: past this the room is no room (and TetGen's coordinates
/// would lose the centimetre in `f32`).
pub const MAX_SIDE_M: f64 = 10_000.0;

/// The surface groups, in the order the project lists them.
pub const GROUP_NAMES: [&str; 3] = ["Floor", "Walls", "Ceiling"];

/// Why no box room was made.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShoeboxError(pub String);

impl std::fmt::Display for ShoeboxError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// The 12 triangles of the box with corners `i` at `(i & 1, i >> 1 & 1, i >> 2 & 1)` times the size,
/// each facing out, and the group (index into [`GROUP_NAMES`]) each belongs to.
const TRIANGLES: [([u32; 3], usize); 12] = [
    // Floor, z = 0, facing -z.
    ([0, 2, 1], 0),
    ([1, 2, 3], 0),
    // Walls: y = 0 (-y), y = L (+y), x = 0 (-x), x = W (+x).
    ([0, 1, 5], 1),
    ([0, 5, 4], 1),
    ([2, 6, 7], 1),
    ([2, 7, 3], 1),
    ([0, 4, 6], 1),
    ([0, 6, 2], 1),
    ([1, 3, 7], 1),
    ([1, 7, 5], 1),
    // Ceiling, z = H, facing +z.
    ([4, 5, 6], 2),
    ([5, 7, 6], 2),
];

/// A new project `name` holding a box room `width` (x) by `length` (y) by `height` (z) metres, its
/// corner at the origin; each side finite, above 0 and at most [`MAX_SIDE_M`].
pub fn shoebox(name: &str, width: f64, length: f64, height: f64) -> Result<Project, ShoeboxError> {
    for (what, v) in [("width", width), ("length", length), ("height", height)] {
        if !(v.is_finite() && v > 0.0) {
            return Err(ShoeboxError(format!(
                "the {what} is {v} m; each side must be a number of metres above 0"
            )));
        }
        if v > MAX_SIDE_M {
            return Err(ShoeboxError(format!(
                "the {what} is {v} m; a side is at most {MAX_SIDE_M} m"
            )));
        }
    }
    let mut p = Project::new(name);
    p.description = format!(
        "A box room {width} m wide (x), {length} m long (y) and {height} m high (z), made in the app."
    );
    let material = super::import::default_material(MaterialId::random(), p.bands.len());
    let groups: Vec<SurfaceGroup> = GROUP_NAMES
        .iter()
        .map(|n| SurfaceGroup {
            id: GroupId::random(),
            name: n.to_string(),
            material: material.id,
        })
        .collect();
    let size = [width, length, height];
    p.geometry.vertices = (0..8u32)
        .map(|i| Vec3::from([0, 1, 2].map(|a| if i >> a & 1 == 1 { size[a] } else { 0.0 })))
        .collect();
    p.geometry.faces = TRIANGLES
        .iter()
        .map(|&(vertices, g)| Face {
            vertices,
            group: groups[g].id,
        })
        .collect();
    p.surface_groups = groups;
    p.materials.push(material);
    Ok(p)
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;

    fn normal(p: &Project, f: &Face) -> [f64; 3] {
        let v = f
            .vertices
            .map(|i| p.geometry.vertices[i as usize].to_array());
        let (u, w) = (
            [0, 1, 2].map(|a| v[1][a] - v[0][a]),
            [0, 1, 2].map(|a| v[2][a] - v[0][a]),
        );
        [
            u[1] * w[2] - u[2] * w[1],
            u[2] * w[0] - u[0] * w[2],
            u[0] * w[1] - u[1] * w[0],
        ]
    }

    #[test]
    fn the_box_is_watertight_faces_out_and_has_its_three_groups() {
        let p = shoebox("Box", 6.0, 10.0, 3.0).unwrap();
        p.check_integrity().unwrap();
        assert_eq!(p.geometry.vertices.len(), 8);
        assert_eq!(p.geometry.faces.len(), 12);
        // Every edge in exactly two triangles, once each way: closed and consistently wound.
        let mut edges: HashMap<(u32, u32), i32> = HashMap::new();
        for f in &p.geometry.faces {
            for k in 0..3 {
                let (a, b) = (f.vertices[k], f.vertices[(k + 1) % 3]);
                *edges.entry((a.min(b), a.max(b))).or_default() += if a < b { 1 } else { -1 };
            }
        }
        assert_eq!(edges.len(), 18);
        assert!(edges.values().all(|&n| n == 0), "{edges:?}");
        // Out of the room: each normal points away from the centre.
        let c = [3.0, 5.0, 1.5];
        for f in &p.geometry.faces {
            let n = normal(&p, f);
            let v = p.geometry.vertices[f.vertices[0] as usize].to_array();
            let out: f64 = (0..3).map(|a| n[a] * (v[a] - c[a])).sum();
            assert!(out > 0.0, "{f:?} faces in");
        }
        let names: Vec<&str> = p.surface_groups.iter().map(|g| g.name.as_str()).collect();
        assert_eq!(names, GROUP_NAMES);
        let count = |g: usize| {
            p.geometry
                .faces
                .iter()
                .filter(|f| f.group == p.surface_groups[g].id)
                .count()
        };
        assert_eq!([count(0), count(1), count(2)], [2, 8, 2]);
        // Upstream's placeholder on every group: Run waits for materials.
        assert_eq!(p.materials.len(), 1);
        assert!(crate::validate::is_placeholder_material(&p.materials[0]));
        assert!(p.sources.is_empty() && p.point_receivers.is_empty());
        // The model check takes it, and measures it as asked.
        let r = crate::geometry::check::check(&p.geometry);
        assert_eq!(r.verdict, crate::geometry::check::Verdict::Ok, "{r:?}");
    }

    #[test]
    fn a_side_that_is_not_a_length_is_refused() {
        for bad in [0.0, -1.0, f64::NAN, f64::INFINITY, 2.0 * MAX_SIDE_M] {
            let e = shoebox("Box", 5.0, bad, 3.0).unwrap_err();
            assert!(e.0.contains("length"), "{e}");
        }
        assert!(shoebox("Box", 5.0, 5.0, 5.0).is_ok());
    }
}
