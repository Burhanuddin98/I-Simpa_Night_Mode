//! Wavefront OBJ.
//!
//! Read: `v x y z` (extra values, such as `w` or a colour, are ignored), `f` with `v`, `v/vt`,
//! `v/vt/vn` or `v//vn` references (1-based, or negative for relative), `g`, `o` and `usemtl`.
//! Every other statement (`vt`, `vn`, `s`, `mtllib`, `l`, `p`, ...) is ignored; line (`l`) and
//! point (`p`) elements are counted in the report, since they are geometry a room cannot use.
//!
//! Surface groups follow [`ObjGroups`]: by default the `usemtl` material when the file has any
//! `usemtl` (in OBJ the material is usually what a surface is made of), otherwise the `g`/`o`
//! name. A group name is the whole rest of its line, not only its first word (Night Mode kept
//! only the first, `mesh/ply_loader.cpp:765`).

use std::collections::HashMap;

use super::{ImportError, ImportOptions, ImportedModel, MeshFormat, ObjGroups, RawMesh, Result};

const FMT: &str = "obj";

pub(super) fn read(bytes: &[u8], options: &ImportOptions) -> Result<ImportedModel> {
    let text = String::from_utf8_lossy(bytes);
    let by_material = match options.obj_groups {
        ObjGroups::Material => true,
        ObjGroups::Group => false,
        ObjGroups::Auto => text
            .lines()
            .any(|l| l.split_ascii_whitespace().next() == Some("usemtl")),
    };

    let mut vertices: Vec<[f64; 3]> = Vec::new();
    // Polygons with raw 1-based indices (positive), resolved at the end since a positive index
    // may name a vertex defined further down.
    let mut raw_polygons: Vec<(Vec<i64>, u32, usize)> = Vec::new();
    let mut groups: Vec<String> = Vec::new();
    let mut group_index: HashMap<String, u32> = HashMap::new();
    let mut current_group = String::from("default");
    let mut current_material = String::from("default");
    let mut ignored_lines = 0usize;
    let mut ignored_points = 0usize;

    for (i, line) in text.lines().enumerate() {
        let line_no = i + 1;
        let line = line.split('#').next().unwrap_or("");
        let mut tok = line.split_ascii_whitespace();
        let Some(keyword) = tok.next() else { continue };
        let at = || format!("line {line_no}");
        match keyword {
            "v" => {
                let mut p = [0.0f64; 3];
                for c in &mut p {
                    let t = tok.next().ok_or_else(|| {
                        ImportError::syntax(FMT, at(), "a vertex needs three coordinates")
                    })?;
                    *c = t.parse::<f64>().map_err(|_| {
                        ImportError::syntax(FMT, at(), format!("`{t}` is not a number"))
                    })?;
                }
                vertices.push(p);
            }
            "f" => {
                let mut idx = Vec::new();
                for t in tok {
                    let v = t.split('/').next().unwrap_or("");
                    let v: i64 = v.parse().map_err(|_| {
                        ImportError::syntax(FMT, at(), format!("`{t}` is not a vertex reference"))
                    })?;
                    let resolved = if v > 0 {
                        v
                    } else if v < 0 {
                        // Relative to the vertices defined so far: -1 is the last one.
                        let r = vertices.len() as i64 + v + 1;
                        if r < 1 {
                            return Err(ImportError::invalid(
                                FMT,
                                format!(
                                    "line {line_no}: relative vertex {v} reaches before the first \
                                     vertex"
                                ),
                            ));
                        }
                        r
                    } else {
                        return Err(ImportError::invalid(
                            FMT,
                            format!("line {line_no}: vertex index 0 (OBJ indices start at 1)"),
                        ));
                    };
                    idx.push(resolved);
                }
                if idx.len() < 3 {
                    return Err(ImportError::invalid(
                        FMT,
                        format!(
                            "line {line_no}: a face with {} vertices; a face needs 3 or more",
                            idx.len()
                        ),
                    ));
                }
                let name = if by_material {
                    &current_material
                } else {
                    &current_group
                };
                let g = *group_index.entry(name.clone()).or_insert_with(|| {
                    groups.push(name.clone());
                    (groups.len() - 1) as u32
                });
                raw_polygons.push((idx, g, line_no));
            }
            "g" | "o" => {
                let name: Vec<&str> = tok.collect();
                current_group = if name.is_empty() {
                    "default".to_string()
                } else {
                    name.join(" ")
                };
            }
            "usemtl" => {
                let name: Vec<&str> = tok.collect();
                current_material = if name.is_empty() {
                    "default".to_string()
                } else {
                    name.join(" ")
                };
            }
            "l" => ignored_lines += 1,
            "p" => ignored_points += 1,
            _ => {}
        }
    }

    if u32::try_from(vertices.len()).is_err() {
        return Err(ImportError::unsupported(
            FMT,
            "more than 4,294,967,295 vertices",
        ));
    }
    let n = vertices.len() as i64;
    let mut polygons = Vec::with_capacity(raw_polygons.len());
    for (idx, g, line_no) in raw_polygons {
        let mut out = Vec::with_capacity(idx.len());
        for v in idx {
            if v > n {
                return Err(ImportError::invalid(
                    FMT,
                    format!("line {line_no}: vertex {v}, but the file has {n} vertices"),
                ));
            }
            out.push((v - 1) as u32);
        }
        polygons.push((out, g));
    }
    let mut notes = vec![if by_material {
        "faces grouped by `usemtl` material".to_string()
    } else {
        "faces grouped by `g`/`o` name".to_string()
    }];
    if ignored_lines > 0 {
        notes.push(format!("{ignored_lines} line elements (`l`) ignored"));
    }
    if ignored_points > 0 {
        notes.push(format!("{ignored_points} point elements (`p`) ignored"));
    }
    let source_faces = polygons.len();
    RawMesh {
        vertices,
        polygons,
        groups,
        notes,
    }
    .finish(MeshFormat::Obj, options, source_faces)
}
