//! The intersecting facets TetGen reports on its stdout, mapped to `.poly` facet markers.
//!
//! Upstream's debug mode (`tetgen -d`) parses two messages
//! (`isimpa/data_manager/logger_tetgen_debug.hpp:47-71`):
//! `  Facet (a, b, c) (n) is not a valid polygon.` and
//! ` Facet #i intersects facet #j at triangles:`, both with 1-based facet numbers
//! (`projet_maillage.cpp:260`). Those are TetGen 1.4's. The TetGen 1.6.0 that upstream builds
//! prints neither: it prints a `Warning:` line naming the kind of intersection, then one or two
//! lines naming each element by its points (the `.poly` node numbers) and, for a facet
//! triangle, its marker, `tag(n)` (`tetgen.cxx:18905-19060, 20576-20670`). Both forms are read.
//! A segment's tag is TetGen's segment mark, -1 for the edges of facets, so a segment is mapped
//! to the markers of every facet that has it as an edge.

use serde::{Deserialize, Serialize};

use crate::formats::poly;

/// One element of an intersection.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Element {
    /// `facet`, `segment` or `vertex`; `unknown` when TetGen printed no detail (`  ...`).
    pub kind: String,
    /// The `.poly` node numbers TetGen printed (1-based as in the file).
    pub points: Vec<i64>,
    /// The printed `tag(n)`, when there is one.
    pub tag: Option<i64>,
    /// The facet markers the element belongs to.
    pub markers: Vec<u32>,
}

/// Two elements TetGen found intersecting.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Intersection {
    /// The warning, e.g. `A segment and a facet intersect.`
    pub message: String,
    pub first: Element,
    pub second: Option<Element>,
}

/// The markers of the facets of `model` that hold every one of `points` (1-based node numbers).
fn facets_with(model: &poly::Model, points: &[i64]) -> Vec<u32> {
    let mut out: Vec<u32> = model
        .model_faces
        .iter()
        .filter(|f| {
            points
                .iter()
                .all(|&p| f.vertices.iter().any(|&v| i64::from(v) + 1 == p))
        })
        .map(|f| f.face_index)
        .collect();
    out.sort_unstable();
    out.dedup();
    out
}

/// `[a,b,c]` or `[a, b]` after the first `[`.
fn bracket_ints(s: &str) -> Option<Vec<i64>> {
    let open = s.find('[')?;
    let close = open + s[open..].find(']')?;
    s[open + 1..close]
        .split(',')
        .map(|t| t.trim().parse().ok())
        .collect()
}

/// The `n` of `tag(n)`.
fn tag(s: &str) -> Option<i64> {
    let at = s.find("tag(")? + 4;
    let end = at + s[at..].find(')')?;
    s[at..end].trim().parse().ok()
}

/// One detail line of a 1.6 warning: `  segment: [9,10] tag(-1).`,
/// `  facet triangle: [1,4,6] tag(8)`, `  vertex : [5]`, `  1st: [3,4].`.
fn element(line: &str, model: &poly::Model) -> Option<Element> {
    let points = bracket_ints(line)?;
    let head = line.trim_start();
    let kind = if head.contains("facet triangle") {
        "facet"
    } else if head.starts_with("vertex") {
        "vertex"
    } else {
        "segment"
    };
    let tag = tag(line);
    let markers = match (kind, tag) {
        ("facet", Some(t)) if t >= 0 => u32::try_from(t).map(|m| vec![m]).unwrap_or_default(),
        _ => facets_with(model, &points),
    };
    Some(Element {
        kind: kind.to_string(),
        points,
        tag,
        markers,
    })
}

fn facet_number(n: &str) -> Option<Element> {
    // Upstream subtracts 1: "First face in tetgen is 1 not 0" (projet_maillage.cpp:260).
    let n: i64 = n.trim().parse().ok()?;
    Some(Element {
        kind: "facet".to_string(),
        points: Vec::new(),
        tag: Some(n),
        markers: u32::try_from(n - 1).map(|m| vec![m]).unwrap_or_default(),
    })
}

/// The legacy forms of `logger_tetgen_debug.hpp:47-48`.
fn legacy(line: &str) -> Option<Intersection> {
    if let Some(rest) = line.strip_prefix(" Facet #")
        && let Some((i, rest)) = rest.split_once(" intersects facet #")
        && let Some((j, _)) = rest.split_once(" at triangles:")
    {
        return Some(Intersection {
            message: line.trim().to_string(),
            first: facet_number(i)?,
            second: Some(facet_number(j)?),
        });
    }
    if let Some(rest) = line.strip_prefix("  Facet (")
        && let Some((_, rest)) = rest.split_once(") (")
        && let Some((n, _)) = rest.split_once(") is not a valid polygon.")
    {
        return Some(Intersection {
            message: line.trim().to_string(),
            first: facet_number(n)?,
            second: None,
        });
    }
    None
}

/// Every intersection reported in `lines` (TetGen's stdout), with the markers of `model`, the
/// `.poly` TetGen read.
pub fn intersections(lines: &[String], model: &poly::Model) -> Vec<Intersection> {
    let mut out: Vec<Intersection> = Vec::new();
    let mut open: Option<Intersection> = None;
    for line in lines {
        if let Some(i) = legacy(line) {
            out.extend(open.take());
            out.push(i);
            continue;
        }
        let trimmed = line.trim();
        if let Some(message) = trimmed.strip_prefix("Warning:") {
            out.extend(open.take());
            let message = message.trim();
            if message.contains("intersect")
                || message.contains("crossing")
                || message.contains("lies on a facet")
            {
                open = Some(Intersection {
                    message: message.to_string(),
                    first: Element {
                        kind: String::new(),
                        points: Vec::new(),
                        tag: None,
                        markers: Vec::new(),
                    },
                    second: None,
                });
            }
            continue;
        }
        if let Some(i) = open.as_mut()
            && line.starts_with("  ")
        {
            if let Some(e) = element(line, model) {
                if i.first.kind.is_empty() {
                    i.first = e;
                } else if i.second.is_none() {
                    i.second = Some(e);
                    out.extend(open.take());
                }
            }
            continue;
        }
        out.extend(open.take());
    }
    out.extend(open);
    // A warning whose detail lines were `  ...` names nothing; keep it, marked as such.
    for i in &mut out {
        if i.first.kind.is_empty() {
            i.first.kind = "unknown".to_string();
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The survey's self-intersecting cube (`mkpolybad.py`): facets 8 and 9 form the x = 5 wall,
    /// facet 12 is the triangle (9, 10, 11) piercing it.
    fn bad_cube() -> poly::Model {
        let tri = |a: u32, b: u32, c: u32, m: u32| poly::Face {
            vertices: [a - 1, b - 1, c - 1],
            face_index: m,
        };
        poly::Model {
            save_face_index: true,
            model_faces: vec![
                tri(1, 4, 6, 8),
                tri(8, 1, 6, 9),
                tri(9, 10, 11, 12),
                tri(1, 2, 3, 0),
            ],
            model_vertices: vec![[0.0; 3]; 11],
            ..poly::Model::default()
        }
    }

    #[test]
    fn tetgen_1_6_warnings_become_marker_pairs() {
        // tetgen -d scene_mesh.poly on the survey's tg_bad, 2026-09-23 (TetGen 1.6.0, M1 build).
        let stdout = "Recovering boundaries...
Warning:  A segment and a facet intersect.
  segment: [9,10] tag(-1).
  facet triangle: [1,4,6] tag(8)
Warning:  A segment and a facet intersect.
  segment: [11,9] tag(-1).
  facet triangle: [8,1,6] tag(9)
Warning:  A segment and a facet intersect.
  segment: [6,1] tag(-1).
  facet triangle: [9,10,11] tag(12)
Boundary recovery seconds:  0.001";
        let lines: Vec<String> = stdout.lines().map(str::to_string).collect();
        let found = intersections(&lines, &bad_cube());
        let pairs: Vec<(Vec<u32>, Vec<u32>)> = found
            .iter()
            .map(|i| {
                (
                    i.first.markers.clone(),
                    i.second.as_ref().unwrap().markers.clone(),
                )
            })
            .collect();
        assert_eq!(
            pairs,
            [
                (vec![12], vec![8]),
                (vec![12], vec![9]),
                (vec![8, 9], vec![12])
            ]
        );
        assert_eq!(found[0].first.kind, "segment");
        assert_eq!(found[0].second.as_ref().unwrap().tag, Some(8));
    }

    #[test]
    fn upstreams_legacy_messages_are_read_one_based() {
        let lines = [
            " Facet #3 intersects facet #10 at triangles:".to_string(),
            "  Facet (1, 2, 3) (7) is not a valid polygon.".to_string(),
        ];
        let found = intersections(&lines, &bad_cube());
        assert_eq!(found.len(), 2);
        assert_eq!(found[0].first.markers, [2]);
        assert_eq!(found[0].second.as_ref().unwrap().markers, [9]);
        assert_eq!(found[1].first.markers, [6]);
        assert!(found[1].second.is_none());
    }

    #[test]
    fn lines_that_report_nothing_give_nothing() {
        let lines: Vec<String> = [
            "Opening scene_mesh.poly.",
            "Warning:  Found 3 duplicated vertices.",
            "  [1,2] something",
            "The input surface mesh is correct.",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();
        assert!(intersections(&lines, &bad_cube()).is_empty());
        // A warning without details is kept, naming nothing (tetgen.cxx:18959-18963).
        let lines = [
            "Warning:  Two facets intersect.".to_string(),
            "  ...".to_string(),
        ];
        let found = intersections(&lines, &bad_cube());
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].first.kind, "unknown");
        assert!(found[0].first.markers.is_empty());
    }
}
