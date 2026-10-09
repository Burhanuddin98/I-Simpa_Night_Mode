//! `core::geometry::export`: a project's geometry written as a Wavefront OBJ that
//! [`super::import::read_obj`] reads back to the same model: every vertex in order (unused ones
//! too), every face in order with its vertex order kept (so a repair's flips survive), and one
//! `g <group name>` line before each run of faces of one surface group. World metres, Z up: import
//! it with unit m and up z.
//!
//! Coordinates are written with Rust's shortest round-trip spelling of the `f64`, so they read
//! back bit for bit. A group name is the rest of its `g` line, so a line break or a tab in a name
//! is written as a space; a group no face uses is not written (OBJ has no empty group).
//! Written by the app's Repair (parity G8) beside the original, never over it.

use std::fmt::Write as _;

use crate::schema::Project;

/// The OBJ text of `project`'s geometry; `header` is written as comment lines first.
pub fn obj_text(project: &Project, header: &[String]) -> String {
    let g = &project.geometry;
    let mut out = String::with_capacity(48 * (g.vertices.len() + g.faces.len()) + 256);
    for line in header {
        for part in line.lines() {
            let _ = writeln!(out, "# {part}");
        }
    }
    let _ = writeln!(out, "# metres, Z up");
    for v in &g.vertices {
        let [x, y, z] = v.to_array();
        let _ = writeln!(out, "v {x:?} {y:?} {z:?}");
    }
    let mut current = None;
    for f in &g.faces {
        if current != Some(f.group) {
            current = Some(f.group);
            let name = project
                .group(f.group)
                .map(|s| s.name.as_str())
                .unwrap_or("default");
            let name: String = name
                .chars()
                .map(|c| if c.is_control() { ' ' } else { c })
                .collect();
            let _ = writeln!(out, "g {}", name.trim());
        }
        let [a, b, c] = f.vertices;
        let _ = writeln!(out, "f {} {} {}", a + 1, b + 1, c + 1);
    }
    out
}
