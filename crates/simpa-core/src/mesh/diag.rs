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
//!
//! TetGen 1.5.0, the mesher since decision 3 (`docs/m5-m6-design.md`), speaks a third way:
//! - **It stops at the first self-intersection it meets** with exit code 3 and the line
//!   [`SELF_INTERSECTION_STOP`] (`terminatetetgen`, `tetgen.h:2265-2267`), and writes no
//!   `_skipped.face`. Before the stop it usually names the pair ([`stop_pair`]):
//!   `Found two facets intersect each other.` (or `two duplicated facets`, `two segments`,
//!   `a segment and a subface`) followed by `  1st: [..] n` and `  2nd: [..] n`
//!   (`tetgen.cxx:13571-13583, 19264-19273, 19603-19609`). A facet there is `[a, b, c]` with its
//!   `shellmark`, the 1-based position of its facet in the `.poly` (`tetgen.cxx:13454`), so it maps
//!   to that facet's marker; a segment is `[a, b]` with its segment mark (1 by default,
//!   `tetgen.cxx:13225`), so it maps by its points.
//! - **`-d` detects every intersecting pair and exits 0** (`tetgen.cxx:30893-30911`): it prints
//!   `  Facet #i intersects facet #j at triangles:` (or `duplicates facet #j at triangle:`), each
//!   pair as often as the recursive search meets it (`tetgen.cxx:14487-14500`), with the same
//!   1-based facet positions, and writes a `.1.face` of the intersecting triangles whose markers
//!   are the facets' own markers (`tetgen.cxx:29472-29476`).

use serde::{Deserialize, Serialize};

use crate::formats::poly;

/// The line TetGen 1.5.0 prints whenever it stops with exit code 3 (`tetgen.h:2265-2267`).
pub const SELF_INTERSECTION_STOP: &str = "A self-intersection was detected. Program stopped.";

/// One element of an intersection.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
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
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
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

/// The marker of the facet at 1-based position `n` of `model`'s facet list, TetGen's `shellmark`
/// of a facet; none when `n` is out of range.
fn marker_at(model: &poly::Model, n: i64) -> Vec<u32> {
    usize::try_from(n - 1)
        .ok()
        .and_then(|i| model.model_faces.get(i))
        .map(|f| vec![f.face_index])
        .unwrap_or_default()
}

fn facet_number(n: &str, model: &poly::Model) -> Option<Element> {
    // Upstream subtracts 1: "First face in tetgen is 1 not 0" (projet_maillage.cpp:260). The
    // number is the facet's position, so it maps to the marker of the facet there, which is the
    // same number less 1 for every .poly upstream writes.
    let n: i64 = n.trim().parse().ok()?;
    Some(Element {
        kind: "facet".to_string(),
        points: Vec::new(),
        tag: Some(n),
        markers: marker_at(model, n),
    })
}

/// The legacy forms of `logger_tetgen_debug.hpp:47-48`, which TetGen 1.5.0's `-d` also prints
/// (with two leading spaces, and `duplicates facet #j at triangle:` for a repeated triangle).
fn legacy(line: &str, model: &poly::Model) -> Option<Intersection> {
    let head = line.trim_start();
    if let Some(rest) = head.strip_prefix("Facet #") {
        for (verb, tail) in [
            (" intersects facet #", " at triangles:"),
            (" duplicates facet #", " at triangle:"),
        ] {
            if let Some((i, rest)) = rest.split_once(verb)
                && let Some((j, _)) = rest.split_once(tail)
            {
                return Some(Intersection {
                    message: line.trim().to_string(),
                    first: facet_number(i, model)?,
                    second: Some(facet_number(j, model)?),
                });
            }
        }
    }
    if let Some(rest) = head.strip_prefix("Facet (")
        && let Some((_, rest)) = rest.split_once(") (")
        && let Some((n, _)) = rest.split_once(") is not a valid polygon.")
    {
        return Some(Intersection {
            message: line.trim().to_string(),
            first: facet_number(n, model)?,
            second: None,
        });
    }
    None
}

/// One of the two lines after a 1.5.0 `Found ...` report: `  1st: [9, 10] 1.`,
/// `  2nd: [1,4,6] 9`, `  1st: [608, 611, 610] #554`. Three points are a facet, whose number is
/// its 1-based position; two are a segment, mapped by its points.
fn stop_element(line: &str, model: &poly::Model) -> Option<Element> {
    let points = bracket_ints(line)?;
    let after = &line[line.find(']')? + 1..];
    let tag: Option<i64> = after
        .trim()
        .trim_start_matches('#')
        .trim_end_matches('.')
        .trim()
        .parse()
        .ok();
    match points.len() {
        3 => Some(Element {
            kind: "facet".to_string(),
            markers: tag.map(|n| marker_at(model, n)).unwrap_or_default(),
            points,
            tag,
        }),
        2 => Some(Element {
            kind: "segment".to_string(),
            markers: facets_with(model, &points),
            points,
            tag,
        }),
        _ => None,
    }
}

/// The integers of each `(..)` group of `s`, in order.
fn paren_groups(s: &str) -> Vec<Vec<i64>> {
    let mut out = Vec::new();
    let mut rest = s;
    while let Some(open) = rest.find('(') {
        let Some(close) = rest[open..].find(')').map(|c| open + c) else {
            break;
        };
        let ints: Option<Vec<i64>> = rest[open + 1..close]
            .split(',')
            .map(|t| t.trim().parse().ok())
            .collect();
        out.extend(ints);
        rest = &rest[close + 1..];
    }
    out
}

/// An element named by its points only: three points a facet, two a segment.
fn by_points(points: Vec<i64>, model: &poly::Model) -> Element {
    Element {
        kind: if points.len() == 3 {
            "facet"
        } else {
            "segment"
        }
        .to_string(),
        markers: facets_with(model, &points),
        points,
        tag: None,
    }
}

/// Whether TetGen's stdout ends in 1.5.0's stop on a self-intersection ([`SELF_INTERSECTION_STOP`]).
pub fn stopped_on_self_intersection(lines: &[String]) -> bool {
    lines.iter().any(|l| l.trim() == SELF_INTERSECTION_STOP)
}

/// The pair TetGen 1.5.0 names before it stops on a self-intersection, mapped to the markers of
/// `model`, the `.poly` it read. The forms, from `tetgen.cxx`:
/// - `Found two facets intersect each other.`, `Found two duplicated facets.`,
///   `Found two segments intersect each other.` and `Found a segment and a subface intersect.`,
///   each followed by a `1st:` and a `2nd:` line (13571-13583, 19264-19273, 19603-19609);
/// - `  Two segments (a, b) and (c, d) intersect.` (13246);
/// - `      subface: (a, b, c) edge: (d, e)` after `A non-valid facet - edge intersection` (15690).
///
/// `None` when the stop names no pair: several of its paths print nothing before it.
pub fn stop_pair(lines: &[String], model: &poly::Model) -> Option<Intersection> {
    for (k, line) in lines.iter().enumerate() {
        let t = line.trim();
        if t.starts_with("Found ")
            && (t.ends_with(" intersect each other.")
                || t.ends_with(" intersect.")
                || t == "Found two duplicated facets.")
        {
            let first = lines
                .get(k + 1)
                .filter(|l| l.trim_start().starts_with("1st:"));
            let second = lines
                .get(k + 2)
                .filter(|l| l.trim_start().starts_with("2nd:"));
            if let (Some(a), Some(b)) = (first, second)
                && let (Some(a), Some(b)) = (stop_element(a, model), stop_element(b, model))
            {
                return Some(Intersection {
                    message: t.to_string(),
                    first: a,
                    second: Some(b),
                });
            }
        }
        if t.starts_with("Two segments (") && t.ends_with(") intersect.") {
            let mut g = paren_groups(t).into_iter();
            if let (Some(a), Some(b)) = (g.next(), g.next()) {
                return Some(Intersection {
                    message: t.to_string(),
                    first: by_points(a, model),
                    second: Some(by_points(b, model)),
                });
            }
        }
        if t.starts_with("subface: (") && t.contains(" edge: (") {
            let mut g = paren_groups(t).into_iter();
            if let (Some(a), Some(b)) = (g.next(), g.next()) {
                return Some(Intersection {
                    message: "A non-valid facet - edge intersection".to_string(),
                    first: by_points(a, model),
                    second: Some(by_points(b, model)),
                });
            }
        }
    }
    None
}

/// The distinct unordered pairs of different facet markers `intersections` name, ascending: each
/// marker of an intersection's first element with each of its second's.
pub fn marker_pairs<'a>(
    intersections: impl IntoIterator<Item = &'a Intersection>,
) -> Vec<[u32; 2]> {
    let mut out: Vec<[u32; 2]> = Vec::new();
    for i in intersections {
        let Some(second) = &i.second else { continue };
        for &a in &i.first.markers {
            for &b in &second.markers {
                if a != b {
                    out.push([a.min(b), a.max(b)]);
                }
            }
        }
    }
    out.sort_unstable();
    out.dedup();
    out
}

/// Every intersection reported in `lines` (TetGen's stdout), with the markers of `model`, the
/// `.poly` TetGen read, each once, in the order first reported: TetGen 1.5.0's `-d` prints one
/// pair as often as its search meets it.
pub fn intersections(lines: &[String], model: &poly::Model) -> Vec<Intersection> {
    let mut out: Vec<Intersection> = Vec::new();
    let mut open: Option<Intersection> = None;
    for line in lines {
        if let Some(i) = legacy(line, model) {
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
    let mut seen = std::collections::HashSet::new();
    out.retain(|i| seen.insert(i.clone()));
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

    /// The survey's self-intersecting cube in full, as `tests/fixtures/meshes/tg_bad` holds it:
    /// facet k (0-based) has marker k, and facet 12 = (9, 10, 11) pierces the x = 5 wall, facets
    /// 8 = (1, 4, 6) and 9 = (8, 1, 6).
    fn tg_bad() -> poly::Model {
        let facets: [[u32; 3]; 13] = [
            [1, 2, 3],
            [1, 3, 4],
            [3, 5, 6],
            [3, 6, 4],
            [3, 7, 5],
            [3, 2, 7],
            [2, 1, 8],
            [7, 2, 8],
            [1, 4, 6],
            [8, 1, 6],
            [8, 6, 5],
            [7, 8, 5],
            [9, 10, 11],
        ];
        poly::Model {
            save_face_index: true,
            model_faces: facets
                .iter()
                .enumerate()
                .map(|(k, f)| poly::Face {
                    vertices: f.map(|v| v - 1),
                    face_index: k as u32,
                })
                .collect(),
            model_vertices: vec![[0.0; 3]; 11],
            ..poly::Model::default()
        }
    }

    fn lines(text: &str) -> Vec<String> {
        text.lines().map(str::to_string).collect()
    }

    #[test]
    fn upstreams_legacy_messages_are_read_one_based() {
        let lines = [
            " Facet #3 intersects facet #10 at triangles:".to_string(),
            "  Facet (1, 2, 3) (7) is not a valid polygon.".to_string(),
        ];
        let found = intersections(&lines, &tg_bad());
        assert_eq!(found.len(), 2);
        assert_eq!(found[0].first.markers, [2]);
        assert_eq!(found[0].second.as_ref().unwrap().markers, [9]);
        assert_eq!(found[1].first.markers, [6]);
        assert!(found[1].second.is_none());
        // The number is a position: it maps to the marker of the facet there. In `bad_cube` the
        // third facet carries marker 12 and there is no tenth, so that side names nothing.
        let found = intersections(&lines[..1], &bad_cube());
        assert_eq!(found[0].first.markers, [12]);
        assert!(found[0].second.as_ref().unwrap().markers.is_empty());
    }

    #[test]
    fn tetgen_1_5_stops_with_its_pair_named() {
        // tetgen -pq5 -A -n -Y on tests/fixtures/meshes/tg_bad, 2026-09-24 (TetGen 1.5.0, exit 3).
        let stdout = lines(
            "Recovering boundaries...
Found a segment and a subface intersect.
  1st: [9, 10] 1.
  2nd: [1,4,6] 9
A self-intersection was detected. Program stopped.
Hint: use -d option to detect all self-intersections.",
        );
        assert!(stopped_on_self_intersection(&stdout));
        let pair = stop_pair(&stdout, &tg_bad()).expect("the stop names a pair");
        assert_eq!(pair.message, "Found a segment and a subface intersect.");
        assert_eq!(
            (pair.first.kind.as_str(), &pair.first.points, pair.first.tag),
            ("segment", &vec![9, 10], Some(1))
        );
        assert_eq!(
            pair.first.markers,
            [12],
            "the segment is an edge of facet 12"
        );
        let second = pair.second.as_ref().unwrap();
        assert_eq!(
            (second.kind.as_str(), &second.points, second.tag),
            ("facet", &vec![1, 4, 6], Some(9))
        );
        assert_eq!(second.markers, [8], "facet #9 is the ninth facet, marker 8");
        assert_eq!(marker_pairs([&pair]), [[8, 12]]);

        // Says no: TetGen 1.6.0's own stop line is not 1.5.0's, and a run that ends well has none.
        let v16 = lines("The input surface mesh contain self-intersections. Program stopped.");
        assert!(!stopped_on_self_intersection(&v16));
        assert!(stop_pair(&v16, &tg_bad()).is_none());
        let ok = lines("Writing scene_mesh.1.neigh.\n\nOutput seconds:  0");
        assert!(!stopped_on_self_intersection(&ok));
    }

    #[test]
    fn tetgen_1_5_stop_forms_map_to_markers() {
        // A model with 600 facets, marker k at position k, facet 553 = (608, 611, 610) and 581 =
        // (608, 611, 774): the raw Elmia hall's first pair (docs/investigations/.../confirm150.md).
        let mut model = poly::Model {
            save_face_index: true,
            model_vertices: vec![[0.0; 3]; 800],
            ..poly::Model::default()
        };
        for k in 0..600u32 {
            let vertices = match k {
                553 => [607, 610, 609],
                581 => [607, 610, 773],
                _ => [k, k + 1, k + 2],
            };
            model.model_faces.push(poly::Face {
                vertices,
                face_index: k,
            });
        }
        let two_facets = lines(
            "Found two facets intersect each other.
  1st: [608, 611, 610] #554
  2nd: [608, 611, 774] #582
A self-intersection was detected. Program stopped.",
        );
        let p = stop_pair(&two_facets, &model).unwrap();
        assert_eq!(p.first.markers, [553]);
        assert_eq!(p.second.as_ref().unwrap().markers, [581]);
        // Two segments: each mapped by its points to the facets having it as an edge.
        let segments = lines(
            "Found two segments intersect each other.
  1st: [608,611] 1.
  2nd: [611,774] 1.",
        );
        let p = stop_pair(&segments, &model).unwrap();
        assert_eq!(p.first.markers, [553, 581]);
        assert_eq!(p.second.as_ref().unwrap().markers, [581]);
        assert_eq!(marker_pairs([&p]), [[553, 581]]);
        // The two forms without a `Found` line.
        let invalid =
            lines("Error:  Invalid PLC.\n  Two segments (608, 610) and (611, 774) intersect.");
        let p = stop_pair(&invalid, &model).unwrap();
        assert_eq!(p.first.markers, [553]);
        assert_eq!(p.second.as_ref().unwrap().markers, [581]);
        let edge = lines(
            "Warning:  A non-valid facet - edge intersection
      subface: (608, 611, 610) edge: (611, 774)",
        );
        let p = stop_pair(&edge, &model).unwrap();
        assert_eq!(p.first.markers, [553]);
        assert_eq!(p.second.as_ref().unwrap().markers, [581]);
        // A `Found` line without its two lines names nothing.
        assert!(stop_pair(&two_facets[..1], &model).is_none());
    }

    #[test]
    fn tetgen_1_5_minus_d_pairs_are_read_once_each() {
        // tetgen -d on tests/fixtures/meshes/tg_bad, 2026-09-24 (TetGen 1.5.0, exit 0): 8 pairs
        // printed, 2 distinct.
        let stdout = lines(
            "Detecting self-intersecting facets...
  Facet #9 intersects facet #13 at triangles:
    (   1,    4,    6) and (   9,   10,   11)
  Facet #10 intersects facet #13 at triangles:
    (   8,    1,    6) and (   9,   10,   11)
  Facet #9 intersects facet #13 at triangles:
    (   1,    4,    6) and (   9,   10,   11)
  Facet #10 intersects facet #13 at triangles:
    (   8,    1,    6) and (   9,   10,   11)

!! Found 8 pairs of faces are intersecting.
",
        );
        let found = intersections(&stdout, &tg_bad());
        assert_eq!(found.len(), 2, "{found:#?}");
        assert_eq!(marker_pairs(&found), [[8, 12], [9, 12]]);
        // A repeated triangle is read too.
        let dup = lines("  Facet #2 duplicates facet #7 at triangle:");
        let found = intersections(&dup, &tg_bad());
        assert_eq!(marker_pairs(&found), [[1, 6]]);
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
