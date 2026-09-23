//! Re-import: a new model into an existing project, keeping material assignments.
//!
//! Upstream's re-import (`ProjectManager::LoadFacesFromModel`, `data_manager/projet.cpp`) takes
//! each new face's centre, finds the old faces within a user epsilon of it (an octree), and puts
//! the new face in the surface group of the nearest one; a face with none goes to a new group
//! named after the new model's group. [`reassign`] does the same, with exact point-to-triangle
//! distances and a uniform grid instead of the octree:
//!
//! - a new face whose centre lies within `tolerance_m` of an old face joins that face's group
//!   (the nearest one; on a tie, the lowest old face index), so it keeps its material, its
//!   variant overrides and its surface receiver;
//! - a new face with no old face that near joins a new group named after its group in the new
//!   model, carrying upstream's default material (reference material 0), added to the library
//!   once as `Default`;
//! - every old group stays, even one that receives no face (so nothing that refers to it breaks);
//!   such groups are listed in [`Reassigned::empty_groups`].
//!
//! Everything else in the project (materials, sources, receivers, settings, variants) is kept.

use std::collections::HashMap;

use super::{IdSource, ImportError, ImportedModel, Result, default_material};
use crate::schema::{Face, GroupId, MaterialId, Project, SurfaceGroup};

const FMT: &str = "reassign";

/// Upstream's default re-import tolerance, 1 cm (`IHM/loadingSceneDialog.cpp:93`).
pub const DEFAULT_REASSIGN_TOLERANCE_M: f64 = 0.01;

/// The result of [`reassign`].
#[derive(Clone, Debug)]
pub struct Reassigned {
    pub project: Project,
    /// New faces that kept an old group.
    pub matched_faces: usize,
    /// New faces that went to a new group.
    pub unmatched_faces: usize,
    /// Groups created for unmatched faces.
    pub new_groups: Vec<GroupId>,
    /// Old groups that received no face.
    pub empty_groups: Vec<GroupId>,
}

/// Puts `model` into `previous`, keeping each face's surface group where its centre lies within
/// `tolerance_m` metres of an old face. See the module docs.
pub fn reassign(previous: &Project, model: &ImportedModel, tolerance_m: f64) -> Result<Reassigned> {
    if !(tolerance_m.is_finite() && tolerance_m >= 0.0) {
        return Err(ImportError::invalid(
            FMT,
            format!("the tolerance must be a finite distance of 0 or more, not {tolerance_m}"),
        ));
    }
    if model.face_groups.len() != model.triangles.len() {
        return Err(ImportError::invalid(
            FMT,
            "the model has a different number of face groups and triangles",
        ));
    }
    let nv = model.vertices.len();
    if let Some(t) = model
        .triangles
        .iter()
        .find(|t| t.iter().any(|&v| v as usize >= nv))
    {
        return Err(ImportError::invalid(
            FMT,
            format!("a triangle {t:?} indexes past the model's {nv} vertices"),
        ));
    }
    if let Some(g) = model
        .face_groups
        .iter()
        .find(|&&g| g as usize >= model.group_names.len())
    {
        return Err(ImportError::invalid(
            FMT,
            format!(
                "a face names group {g}, but the model has {}",
                model.group_names.len()
            ),
        ));
    }
    let old: Vec<[[f64; 3]; 3]> = previous
        .geometry
        .faces
        .iter()
        .map(|f| {
            f.vertices.map(|v| {
                previous
                    .geometry
                    .vertices
                    .get(v as usize)
                    .map(|p| p.to_array())
                    .unwrap_or([f64::NAN; 3])
            })
        })
        .collect();
    let grid = Grid::new(&old, tolerance_m);

    let ids = IdSource::from_parts(&[
        b"reassign",
        previous.id.uuid().as_bytes(),
        &model.digest_bytes(),
        &tolerance_m.to_bits().to_le_bytes(),
    ]);
    let mut project = previous.clone();
    let mut new_group_of: HashMap<u32, GroupId> = HashMap::new();
    let mut new_groups = Vec::new();
    let mut default: Option<MaterialId> = None;
    let mut faces = Vec::with_capacity(model.triangles.len());
    let mut matched = 0usize;
    for (t, &g) in model.triangles.iter().zip(&model.face_groups) {
        let p = t.map(|v| model.vertices[v as usize].to_array());
        let centre = [0, 1, 2].map(|k| (p[0][k] + p[1][k] + p[2][k]) / 3.0);
        let group = match grid.nearest(&old, centre, tolerance_m) {
            Some(j) => {
                matched += 1;
                previous.geometry.faces[j].group
            }
            None => *new_group_of.entry(g).or_insert_with(|| {
                let material = *default.get_or_insert_with(|| {
                    let m =
                        default_material(MaterialId(ids.uuid("material", 0)), project.bands.len());
                    let id = m.id;
                    project.materials.push(m);
                    id
                });
                let id = GroupId(ids.uuid("surface group", new_groups.len()));
                project.surface_groups.push(SurfaceGroup {
                    id,
                    name: model.group_names[g as usize].clone(),
                    material,
                });
                new_groups.push(id);
                id
            }),
        };
        faces.push(Face {
            vertices: *t,
            group,
        });
    }
    let used: std::collections::HashSet<GroupId> = faces.iter().map(|f| f.group).collect();
    let empty_groups = previous
        .surface_groups
        .iter()
        .map(|g| g.id)
        .filter(|g| !used.contains(g))
        .collect();
    project.geometry.vertices = model.vertices.clone();
    project.geometry.faces = faces;
    project.check_integrity().map_err(ImportError::Integrity)?;
    Ok(Reassigned {
        project,
        matched_faces: matched,
        unmatched_faces: model.triangles.len() - matched,
        new_groups,
        empty_groups,
    })
}

/// A sparse uniform grid over the old triangles' boxes, each grown by the tolerance: a point
/// within the tolerance of a triangle lies in that triangle's grown box, so in one of its cells.
struct Grid {
    origin: [f64; 3],
    cell: f64,
    cells: HashMap<[i64; 3], Vec<u32>>,
    /// Triangles whose grown box spans more than [`MAX_CELLS_PER_TRIANGLE`] cells: checked for
    /// every point instead, so that one huge triangle cannot fill the memory.
    oversized: Vec<u32>,
}

/// The grid has at most this many cells along its longest axis.
const MAX_CELLS_PER_AXIS: f64 = 1024.0;
const MAX_CELLS_PER_TRIANGLE: i64 = 4096;

impl Grid {
    fn new(tris: &[[[f64; 3]; 3]], tol: f64) -> Grid {
        let finite: Vec<usize> = (0..tris.len())
            .filter(|&i| tris[i].iter().flatten().all(|c| c.is_finite()))
            .collect();
        let mut lo = [f64::INFINITY; 3];
        let mut hi = [f64::NEG_INFINITY; 3];
        let mut extent_sum = 0.0;
        for &i in &finite {
            let (a, b) = aabb(&tris[i]);
            let mut ext: f64 = 0.0;
            for k in 0..3 {
                lo[k] = lo[k].min(a[k] - tol);
                hi[k] = hi[k].max(b[k] + tol);
                ext = ext.max(b[k] - a[k]);
            }
            extent_sum += ext;
        }
        if finite.is_empty() {
            return Grid {
                origin: [0.0; 3],
                cell: 1.0,
                cells: HashMap::new(),
                oversized: Vec::new(),
            };
        }
        let span = (0..3).map(|k| hi[k] - lo[k]).fold(0.0, f64::max);
        let mean = extent_sum / finite.len() as f64;
        let cell = mean
            .max(2.0 * tol)
            .max(span / MAX_CELLS_PER_AXIS)
            .max(f64::MIN_POSITIVE);
        let mut grid = Grid {
            origin: lo,
            cell,
            cells: HashMap::new(),
            oversized: Vec::new(),
        };
        for &i in &finite {
            let (a, b) = aabb(&tris[i]);
            let c0 = grid.cell_of([a[0] - tol, a[1] - tol, a[2] - tol]);
            let c1 = grid.cell_of([b[0] + tol, b[1] + tol, b[2] + tol]);
            let count = (0..3).fold(1i64, |n, k| {
                n.saturating_mul(c1[k].saturating_sub(c0[k]).saturating_add(1))
            });
            if count > MAX_CELLS_PER_TRIANGLE {
                grid.oversized.push(i as u32);
                continue;
            }
            for x in c0[0]..=c1[0] {
                for y in c0[1]..=c1[1] {
                    for z in c0[2]..=c1[2] {
                        grid.cells.entry([x, y, z]).or_default().push(i as u32);
                    }
                }
            }
        }
        grid
    }

    fn cell_of(&self, p: [f64; 3]) -> [i64; 3] {
        [0, 1, 2].map(|k| ((p[k] - self.origin[k]) / self.cell).floor() as i64)
    }

    /// The nearest triangle within `tol` of `p`; on a tie, the lowest index.
    fn nearest(&self, tris: &[[[f64; 3]; 3]], p: [f64; 3], tol: f64) -> Option<usize> {
        if !p.iter().all(|c| c.is_finite()) {
            return None;
        }
        let in_cell = self
            .cells
            .get(&self.cell_of(p))
            .map_or(&[][..], Vec::as_slice);
        let mut best: Option<(f64, usize)> = None;
        for &i in in_cell.iter().chain(&self.oversized) {
            let i = i as usize;
            let d = distance_to_triangle(p, &tris[i]);
            if d <= tol && best.is_none_or(|(bd, bi)| d < bd || (d == bd && i < bi)) {
                best = Some((d, i));
            }
        }
        best.map(|(_, i)| i)
    }
}

fn aabb(t: &[[f64; 3]; 3]) -> ([f64; 3], [f64; 3]) {
    let mut lo = t[0];
    let mut hi = t[0];
    for v in &t[1..] {
        for k in 0..3 {
            lo[k] = lo[k].min(v[k]);
            hi[k] = hi[k].max(v[k]);
        }
    }
    (lo, hi)
}

fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn dist(a: [f64; 3], b: [f64; 3]) -> f64 {
    let d = sub(a, b);
    dot(d, d).sqrt()
}

fn distance_to_segment(p: [f64; 3], a: [f64; 3], b: [f64; 3]) -> f64 {
    let ab = sub(b, a);
    let len2 = dot(ab, ab);
    if len2 == 0.0 {
        return dist(p, a);
    }
    let t = (dot(sub(p, a), ab) / len2).clamp(0.0, 1.0);
    dist(p, [a[0] + t * ab[0], a[1] + t * ab[1], a[2] + t * ab[2]])
}

/// Exact distance from `p` to a triangle (Ericson, *Real-Time Collision Detection*, 5.1.5); a
/// degenerate triangle is measured as its three edges.
fn distance_to_triangle(p: [f64; 3], t: &[[f64; 3]; 3]) -> f64 {
    let [a, b, c] = *t;
    let ab = sub(b, a);
    let ac = sub(c, a);
    let n = [
        ab[1] * ac[2] - ab[2] * ac[1],
        ab[2] * ac[0] - ab[0] * ac[2],
        ab[0] * ac[1] - ab[1] * ac[0],
    ];
    if dot(n, n) == 0.0 {
        return distance_to_segment(p, a, b)
            .min(distance_to_segment(p, b, c))
            .min(distance_to_segment(p, c, a));
    }
    let ap = sub(p, a);
    let d1 = dot(ab, ap);
    let d2 = dot(ac, ap);
    if d1 <= 0.0 && d2 <= 0.0 {
        return dist(p, a);
    }
    let bp = sub(p, b);
    let d3 = dot(ab, bp);
    let d4 = dot(ac, bp);
    if d3 >= 0.0 && d4 <= d3 {
        return dist(p, b);
    }
    let vc = d1 * d4 - d3 * d2;
    if vc <= 0.0 && d1 >= 0.0 && d3 <= 0.0 {
        let v = d1 / (d1 - d3);
        return dist(p, [a[0] + v * ab[0], a[1] + v * ab[1], a[2] + v * ab[2]]);
    }
    let cp = sub(p, c);
    let d5 = dot(ab, cp);
    let d6 = dot(ac, cp);
    if d6 >= 0.0 && d5 <= d6 {
        return dist(p, c);
    }
    let vb = d5 * d2 - d1 * d6;
    if vb <= 0.0 && d2 >= 0.0 && d6 <= 0.0 {
        let w = d2 / (d2 - d6);
        return dist(p, [a[0] + w * ac[0], a[1] + w * ac[1], a[2] + w * ac[2]]);
    }
    let va = d3 * d6 - d5 * d4;
    if va <= 0.0 && (d4 - d3) >= 0.0 && (d5 - d6) >= 0.0 {
        let w = (d4 - d3) / ((d4 - d3) + (d5 - d6));
        let bc = sub(c, b);
        return dist(p, [b[0] + w * bc[0], b[1] + w * bc[1], b[2] + w * bc[2]]);
    }
    let denom = 1.0 / (va + vb + vc);
    let v = vb * denom;
    let w = vc * denom;
    dist(
        p,
        [
            a[0] + ab[0] * v + ac[0] * w,
            a[1] + ab[1] * v + ac[1] * w,
            a[2] + ab[2] * v + ac[2] * w,
        ],
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn distances_to_a_triangle() {
        let t = [[0.0, 0.0, 0.0], [2.0, 0.0, 0.0], [0.0, 2.0, 0.0]];
        assert_eq!(distance_to_triangle([0.5, 0.5, 3.0], &t), 3.0);
        assert_eq!(distance_to_triangle([-1.0, 0.0, 0.0], &t), 1.0);
        assert_eq!(distance_to_triangle([3.0, 0.0, 0.0], &t), 1.0);
        assert!((distance_to_triangle([2.0, 2.0, 0.0], &t) - 2f64.sqrt()).abs() < 1e-15);
        let degenerate = [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [2.0, 0.0, 0.0]];
        assert_eq!(distance_to_triangle([1.0, 1.0, 0.0], &degenerate), 1.0);
    }
}
