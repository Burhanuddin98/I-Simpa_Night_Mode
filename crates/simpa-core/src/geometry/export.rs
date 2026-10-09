//! `core::geometry::export`: a project's geometry written as a Wavefront OBJ that
//! [`super::import::read_obj`] reads back to the same model: every vertex in order (unused ones
//! too), every face in order with its vertex order kept (so a repair's flips survive), and one
//! `g <group name>` line before each run of faces of one surface group. World metres, Z up: import
//! it with unit m and up z.
//!
//! Coordinates are written with Rust's shortest round-trip spelling of the `f64`, so they read
//! back bit for bit. The reader takes a `g` line's words joined by one space and stops at a `#`,
//! so a name is written that way: runs of whitespace and control characters (line breaks and
//! tabs included) become one space, a `#` becomes a space, the ends are trimmed, and an empty name is `default`. Two groups
//! whose names differ only in what that drops would read back as one group; the later one is
//! written as `<name> (2)`, `(3)`, ... so every group stays a group of its own. A group no face
//! uses is not written (OBJ has no empty group). Written by the app's Repair (parity G8) beside
//! the original, never over it.

use std::collections::{HashMap, HashSet};
use std::fmt::Write as _;

use crate::schema::{GroupId, Project};

/// The OBJ text of `project`'s geometry; `header` is written as comment lines first.
pub fn obj_text(project: &Project, header: &[String]) -> String {
    let g = &project.geometry;
    let names = written_names(project);
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
            let _ = writeln!(out, "g {}", names[&f.group]);
        }
        let [a, b, c] = f.vertices;
        let _ = writeln!(out, "f {} {} {}", a + 1, b + 1, c + 1);
    }
    out
}

/// A group name as the OBJ reader would read it back from a `g` line.
fn as_read(name: &str) -> String {
    let words: Vec<&str> = name
        .split(|c: char| c.is_ascii_whitespace() || c.is_control() || c == '#')
        .filter(|w| !w.is_empty())
        .collect();
    if words.is_empty() {
        "default".to_string()
    } else {
        words.join(" ")
    }
}

/// The name written for every group a face uses (a face whose group is not in the project is
/// `default`), distinct from one another as read back, in face order.
fn written_names(project: &Project) -> HashMap<GroupId, String> {
    let mut names = HashMap::new();
    let mut taken = HashSet::new();
    for f in &project.geometry.faces {
        if names.contains_key(&f.group) {
            continue;
        }
        let base = as_read(
            project
                .group(f.group)
                .map(|s| s.name.as_str())
                .unwrap_or("default"),
        );
        let mut name = base.clone();
        let mut n = 2;
        while !taken.insert(name.clone()) {
            name = format!("{base} ({n})");
            n += 1;
        }
        names.insert(f.group, name);
    }
    names
}
