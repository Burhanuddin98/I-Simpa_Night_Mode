//! `core::geometry::repair`: the safe, mechanical fixes, every one of them logged.
//!
//! [`repair`] takes a [`Geometry`] and returns a [`RepairOutcome`]: the repaired geometry, a log
//! with one typed [`Change`] per change made, maps from input to output indices, and the
//! [`check`] report of the output. It applies four fixes, in this order:
//!
//! 1. **Welding.** A vertex within the tolerance of an earlier vertex is merged into the first
//!    such vertex ([`Change::WeldVertex`]) and removed from the vertex list; the faces are
//!    renumbered. Only vertices that were not merged themselves are targets, so no vertex moves
//!    further than the tolerance and chains do not form. Vertices with a non-finite coordinate
//!    are never welded.
//! 2. **Degenerate faces** are removed ([`Change::RemoveDegenerateFace`]): a repeated vertex
//!    index, three exactly collinear or coincident vertices (exact predicates), or a face that
//!    welding collapsed.
//! 3. **Duplicate faces**, the same three (welded) vertices as an earlier face in either
//!    orientation, are removed and the earlier face kept ([`Change::RemoveDuplicateFace`]).
//! 4. **Orientation.** The check runs on the result, and every face it finds **inverted** (normal
//!    pointing into the volume it bounds; see [`super::check`]) is flipped by swapping its
//!    second and third vertices ([`Change::FlipFace`]). Partitions and sheets have no required
//!    orientation and are left alone. When the check says its topology is unreliable (faces
//!    intersect, or a component could not be placed), orientation is not decided and nothing is
//!    flipped ([`RepairOutcome::oriented`] is false).
//!
//! Nothing else changes: no vertex moves except by welding, no face is added, no group is
//! changed, and a vertex that no face uses stays unless it is welded (a face removed by repair
//! can leave its vertices unused; they stay too). **Holes and self-intersections are
//! never "fixed"**: they, and anything else the output's check still refuses, come back as
//! [`RepairOutcome::refusals`] with [`RepairStatus::Refused`], face indices given in the input's
//! numbering.
//!
//! Every index in a [`Change`] or a [`Refusal`] is an index into the **input** geometry; use
//! [`RepairOutcome::vertex_map`] and [`RepairOutcome::face_map`] to find the output's.

use std::collections::HashMap;

use serde::Serialize;

use super::check::predicates::{self, P3};
use super::check::{CheckReport, ReasonCode, check};
use crate::schema::{Face, Geometry};

/// The default welding tolerance: one micrometre, far below any acoustically meaningful feature
/// and far above the rounding of coordinates written with 9 significant digits in a hall of
/// tens of metres.
pub const DEFAULT_WELD_TOLERANCE_M: f64 = 1e-6;

/// How [`repair`] works.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct RepairOptions {
    /// Vertices at most this far apart (Euclidean distance, metres) are welded. `0.0` welds exact
    /// duplicates only (`-0.0` and `0.0` are one coordinate). Must be finite and not negative.
    pub weld_tolerance_m: f64,
}

impl Default for RepairOptions {
    fn default() -> Self {
        RepairOptions {
            weld_tolerance_m: DEFAULT_WELD_TOLERANCE_M,
        }
    }
}

/// Why [`repair`] did not run.
#[derive(Clone, Debug, PartialEq)]
pub enum RepairError {
    /// The welding tolerance is negative, NaN or infinite.
    InvalidTolerance(f64),
}

impl RepairError {
    /// A stable machine-readable code.
    pub fn code(&self) -> &'static str {
        match self {
            RepairError::InvalidTolerance(_) => "invalid_tolerance",
        }
    }
}

impl std::fmt::Display for RepairError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RepairError::InvalidTolerance(t) => write!(
                f,
                "the welding tolerance must be a finite, non-negative length in metres, not {t}"
            ),
        }
    }
}

impl std::error::Error for RepairError {}

/// Why a face was removed as degenerate.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DegenerateCause {
    /// Two of its vertex indices were equal in the input.
    RepeatedVertex,
    /// Three distinct vertices, exactly collinear or coincident, in the input.
    ZeroArea,
    /// It had an area in the input; welding merged two of its vertices (or, rarely, left three
    /// exactly collinear).
    CollapsedByWeld,
}

/// One change made by [`repair`]. Indices are into the input geometry. In JSON the variant is
/// the `"kind"` field, in snake_case; these names are stable API.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Change {
    /// `vertex` was merged into `into` (an earlier vertex, which is kept), `distance_m` away.
    WeldVertex {
        vertex: u32,
        into: u32,
        distance_m: f64,
    },
    /// `face` had zero area and was removed.
    RemoveDegenerateFace { face: u32, cause: DegenerateCause },
    /// `face` had the same three vertices (after welding) as `duplicate_of`, which is kept.
    RemoveDuplicateFace {
        face: u32,
        duplicate_of: u32,
        /// Same cyclic order, hence the same normal.
        same_orientation: bool,
        /// Same surface group. When false, the removed face's group (and its material) is
        /// dropped for the kept face's.
        same_group: bool,
    },
    /// `face` pointed into the volume it bounds; its second and third vertices were swapped.
    FlipFace { face: u32 },
}

impl Change {
    /// The stable `kind` string, as serialised.
    pub fn kind(&self) -> &'static str {
        match self {
            Change::WeldVertex { .. } => "weld_vertex",
            Change::RemoveDegenerateFace { .. } => "remove_degenerate_face",
            Change::RemoveDuplicateFace { .. } => "remove_duplicate_face",
            Change::FlipFace { .. } => "flip_face",
        }
    }
}

/// The overall outcome.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RepairStatus {
    /// Nothing needed changing and the geometry passes the check.
    Unchanged,
    /// Changes were made and the result passes the check.
    Repaired,
    /// The result still fails the check, for the [`RepairOutcome::refusals`]; repair does not fix
    /// those. Changes may still have been made (see the log).
    Refused,
}

/// A reason the output still fails the check: the check's [`super::check::Reason`], with face
/// indices mapped back to the input.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Refusal {
    pub code: ReasonCode,
    pub count: usize,
    /// The faces involved, as input indices, ascending.
    pub faces: Vec<u32>,
    pub message: String,
}

/// The result of [`repair`].
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct RepairOutcome {
    pub status: RepairStatus,
    pub options: RepairOptions,
    /// Every change made, in this order: welds by vertex, face removals by face, flips by face.
    pub changes: Vec<Change>,
    /// Empty exactly when the status is not refused.
    pub refusals: Vec<Refusal>,
    /// Whether orientation was decided (step 4 ran). False when the check found the topology
    /// unreliable; nothing is flipped then.
    pub oriented: bool,
    /// The repaired geometry.
    pub geometry: Geometry,
    /// Per input vertex: its output index (a welded vertex maps to where it was welded to).
    pub vertex_map: Vec<u32>,
    /// Per input face: its output index, or `None` if it was removed.
    pub face_map: Vec<Option<u32>>,
    /// The check of the output geometry (output indices).
    pub report: CheckReport,
}

impl RepairOutcome {
    /// Whether the output passes the check.
    pub fn is_ok(&self) -> bool {
        self.status != RepairStatus::Refused
    }
}

fn is_finite(p: &P3) -> bool {
    p.iter().all(|c| c.is_finite())
}

fn distance(a: P3, b: P3) -> f64 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
}

/// Exact-position key; `-0.0` and `0.0` are one position.
fn position_key(p: &P3) -> [u64; 3] {
    p.map(|c| if c == 0.0 { 0 } else { c.to_bits() })
}

/// Per vertex, the vertex it is welded into: itself when kept, else an earlier kept vertex at
/// most `tolerance` away (the lowest-numbered such vertex).
fn weld_targets(pos: &[P3], tolerance: f64) -> Vec<u32> {
    let mut target: Vec<u32> = (0..pos.len() as u32).collect();
    if tolerance == 0.0 {
        let mut first: HashMap<[u64; 3], u32> = HashMap::with_capacity(pos.len());
        for (i, p) in pos.iter().enumerate() {
            if is_finite(p) {
                target[i] = *first.entry(position_key(p)).or_insert(i as u32);
            }
        }
        return target;
    }
    // Grid cells twice the tolerance: two points within the tolerance are in the same or
    // neighbouring cells even after the division rounds. Casts saturate, so far-out coordinates
    // share a cell, which is only slower, never wrong: every candidate's distance is tested.
    let cell = 2.0 * tolerance;
    let mut grid: HashMap<[i64; 3], Vec<u32>> = HashMap::new();
    for (i, p) in pos.iter().enumerate() {
        if !is_finite(p) {
            continue;
        }
        let c = p.map(|x| (x / cell).floor() as i64);
        let mut best: Option<u32> = None;
        for dx in -1i64..=1 {
            for dy in -1i64..=1 {
                for dz in -1i64..=1 {
                    let key = [
                        c[0].saturating_add(dx),
                        c[1].saturating_add(dy),
                        c[2].saturating_add(dz),
                    ];
                    for &r in grid.get(&key).into_iter().flatten() {
                        if distance(*p, pos[r as usize]) <= tolerance && best.is_none_or(|b| r < b)
                        {
                            best = Some(r);
                        }
                    }
                }
            }
        }
        match best {
            Some(r) => target[i] = r,
            None => grid.entry(c).or_default().push(i as u32),
        }
    }
    target
}

/// Whether two cyclic triples list the same cycle (same orientation).
fn same_cycle(a: [u32; 3], b: [u32; 3]) -> bool {
    (0..3).any(|r| (0..3).all(|k| a[k] == b[(k + r) % 3]))
}

fn has_repeat(t: [u32; 3]) -> bool {
    t[0] == t[1] || t[1] == t[2] || t[0] == t[2]
}

/// Repairs `geometry`. See the module docs. Fails only on invalid options.
pub fn repair(geometry: &Geometry, options: &RepairOptions) -> Result<RepairOutcome, RepairError> {
    let tolerance = options.weld_tolerance_m;
    if !(tolerance.is_finite() && tolerance >= 0.0) {
        return Err(RepairError::InvalidTolerance(tolerance));
    }
    let pos: Vec<P3> = geometry.vertices.iter().map(|v| v.to_array()).collect();
    let nv = pos.len();
    let mut changes = Vec::new();

    // 1. Weld.
    let target = weld_targets(&pos, tolerance);
    let mut vertex_map = vec![0u32; nv];
    let mut vertices = Vec::with_capacity(nv);
    for (v, &t) in target.iter().enumerate() {
        if t == v as u32 {
            vertex_map[v] = vertices.len() as u32;
            vertices.push(geometry.vertices[v]);
        } else {
            changes.push(Change::WeldVertex {
                vertex: v as u32,
                into: t,
                distance_m: distance(pos[v], pos[t as usize]),
            });
        }
    }
    for v in 0..nv {
        // Targets are kept vertices numbered no higher, so their entry is already final.
        vertex_map[v] = vertex_map[target[v] as usize];
    }

    // 2, 3. Degenerate and duplicate faces, in the input's welded numbering.
    let nf = geometry.faces.len();
    let mut face_map: Vec<Option<u32>> = vec![None; nf];
    let mut kept: Vec<u32> = Vec::with_capacity(nf);
    let mut faces: Vec<Face> = Vec::with_capacity(nf);
    let mut first: HashMap<[u32; 3], u32> = HashMap::with_capacity(nf);
    for (i, face) in geometry.faces.iter().enumerate() {
        let t = face.vertices;
        let keep = |faces: &mut Vec<Face>, kept: &mut Vec<u32>, vertices: [u32; 3]| {
            kept.push(i as u32);
            faces.push(Face {
                vertices,
                group: face.group,
            });
            Some(faces.len() as u32 - 1)
        };
        if t.iter().any(|&v| v as usize >= nv) {
            // Not repairable. In-range indices are renumbered, out-of-range ones kept, and they
            // stay out of range (the output has no more vertices than the input); the check
            // refuses the face as invalid.
            let renumbered = t.map(|v| vertex_map.get(v as usize).copied().unwrap_or(v));
            face_map[i] = keep(&mut faces, &mut kept, renumbered);
            continue;
        }
        let welded = t.map(|v| target[v as usize]);
        let renumbered = welded.map(|v| vertex_map[v as usize]);
        if t.iter().any(|&v| !is_finite(&pos[v as usize])) {
            // Not repairable either: the check refuses it as invalid.
            face_map[i] = keep(&mut faces, &mut kept, renumbered);
            continue;
        }
        let cause = if has_repeat(t) {
            Some(DegenerateCause::RepeatedVertex)
        } else if predicates::is_zero_area(&t.map(|v| pos[v as usize])) {
            Some(DegenerateCause::ZeroArea)
        } else if has_repeat(welded) || predicates::is_zero_area(&welded.map(|v| pos[v as usize])) {
            Some(DegenerateCause::CollapsedByWeld)
        } else {
            None
        };
        if let Some(cause) = cause {
            changes.push(Change::RemoveDegenerateFace {
                face: i as u32,
                cause,
            });
            continue;
        }
        let mut key = welded;
        key.sort_unstable();
        if let Some(&j) = first.get(&key) {
            let other = &geometry.faces[j as usize];
            changes.push(Change::RemoveDuplicateFace {
                face: i as u32,
                duplicate_of: j,
                same_orientation: same_cycle(welded, other.vertices.map(|v| target[v as usize])),
                same_group: other.group == face.group,
            });
            continue;
        }
        first.insert(key, i as u32);
        face_map[i] = keep(&mut faces, &mut kept, renumbered);
    }
    let mut output = Geometry { vertices, faces };

    // 4. Orientation.
    let first_report = check(&output);
    let oriented = first_report.topology_reliable;
    let report = if oriented && !first_report.inverted_faces.is_empty() {
        for &f in &first_report.inverted_faces {
            output.faces[f as usize].vertices.swap(1, 2);
            changes.push(Change::FlipFace {
                face: kept[f as usize],
            });
        }
        check(&output)
    } else {
        first_report
    };

    let refusals: Vec<Refusal> = report
        .reasons
        .iter()
        .map(|r| {
            let mut faces: Vec<u32> = r.faces.iter().map(|&f| kept[f as usize]).collect();
            faces.sort_unstable();
            Refusal {
                code: r.code,
                count: r.count,
                faces,
                message: r.message.clone(),
            }
        })
        .collect();
    let status = if !report.is_ok() {
        RepairStatus::Refused
    } else if changes.is_empty() {
        RepairStatus::Unchanged
    } else {
        RepairStatus::Repaired
    };
    Ok(RepairOutcome {
        status,
        options: *options,
        changes,
        refusals,
        oriented,
        geometry: output,
        vertex_map,
        face_map,
        report,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn welding_picks_the_first_kept_vertex_and_never_chains() {
        // 0 and 1 are 0.6 apart, 1 and 2 are 0.6 apart, 0 and 2 are 1.2 apart. With a tolerance
        // of 1, vertex 1 welds into 0; vertex 2 is 1.2 from 0 (the only kept vertex near it), so
        // it stays: no vertex moves further than the tolerance.
        let pos = [
            [0.0, 0.0, 0.0],
            [0.6, 0.0, 0.0],
            [1.2, 0.0, 0.0],
            [0.0, 0.0, 0.0],
        ];
        assert_eq!(weld_targets(&pos, 1.0), vec![0, 0, 2, 0]);
        // Exactly the tolerance apart welds; a hair more does not.
        let pos = [
            [0.0, 0.0, 0.0],
            [0.5, 0.0, 0.0],
            [10.0, 0.0, 0.0],
            [10.5000001, 0.0, 0.0],
        ];
        assert_eq!(weld_targets(&pos, 0.5), vec![0, 0, 2, 3]);
    }

    #[test]
    fn exact_welding_equates_signed_zeros_and_skips_non_finite() {
        let pos = [
            [0.0, -0.0, 1.0],
            [-0.0, 0.0, 1.0],
            [f64::NAN, 0.0, 0.0],
            [f64::NAN, 0.0, 0.0],
            [0.0, 0.0, 1.0 + f64::EPSILON],
        ];
        assert_eq!(weld_targets(&pos, 0.0), vec![0, 0, 2, 3, 4]);
        assert_eq!(weld_targets(&pos, 1e-9), vec![0, 0, 2, 3, 0]);
    }

    #[test]
    fn grid_welding_matches_brute_force() {
        // Points on a jittered lattice, many pairs near the tolerance.
        let mut state: u64 = 0x853c_49e6_748f_ea9b;
        let mut next = || {
            state = state
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            ((state >> 11) as f64) / ((1u64 << 53) as f64)
        };
        let pos: Vec<P3> = (0..3000)
            .map(|_| [0.0, 0.0, 0.0].map(|_: f64| (next() * 20.0).floor() * 0.01 + next() * 0.012))
            .collect();
        let tolerance = 0.01;
        let mut expected: Vec<u32> = (0..pos.len() as u32).collect();
        let mut kept: Vec<u32> = Vec::new();
        for i in 0..pos.len() {
            match kept
                .iter()
                .copied()
                .find(|&r| distance(pos[i], pos[r as usize]) <= tolerance)
            {
                Some(r) => expected[i] = r,
                None => kept.push(i as u32),
            }
        }
        let welded = expected
            .iter()
            .enumerate()
            .filter(|&(i, &t)| t != i as u32)
            .count();
        assert!(
            welded > 500,
            "only {welded} welds: the case does not exercise the grid"
        );
        assert_eq!(weld_targets(&pos, tolerance), expected);
    }
}
