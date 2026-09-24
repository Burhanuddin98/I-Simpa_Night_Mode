//! The region volume check of [`super::verify_mesh_with`]: every region TetGen made fills one
//! cell of the geometry it was given, with that cell's volume, and each fitting zone's id is on
//! its zone's cell (`docs/formats/mesh-manifest.md`, "Region volumes").
//!
//! Why it exists: a lost-particle count cannot see a wrong room. TetGen 1.6.0 on tutorial 3's raw
//! scene skips the floor and meshes 1,220.9 m³ where the room is 978.3 m³, both fittings gone, and
//! SPPS loses 1 particle in 60,000 on it (`docs/investigations/2026-09-24-tutorial3/`).
//!
//! How a region finds its cell: the centroid of its largest tetrahedron lies strictly inside it,
//! so it lies strictly inside the one cell the region fills. That point is located exactly among
//! the geometry's facets by a segment to far outside, as `geometry::check` places its components
//! ([`predicates::segment_crossing`]: the first facet crossed, and the side of it the point is
//! on, give the cell; a segment through an edge, a vertex or along a facet is replaced by the next
//! direction). Regions and cells are then matched one to one, and their volumes compared.

use serde::{Deserialize, Serialize};

use super::geometry::{self, P};
use crate::formats::mbin;
use crate::geometry::check::CheckReport;
use crate::geometry::check::predicates::{self, Crossing};
use crate::mesh::input::FittingRegion;

/// A face the check did not analyse, in [`CheckReport::face_cells`].
const NOT_ANALYSED: u32 = u32::MAX;

/// The geometry TetGen was given, as the region volume check reads it.
#[derive(Clone, Copy, Debug)]
pub struct Reference<'a> {
    /// Its nodes, as TetGen read them (the `.poly`'s node list).
    pub vertices: &'a [[f64; 3]],
    /// Its facets (the `.poly`'s facet list), 0-based node indices.
    pub facets: &'a [[u32; 3]],
    /// Per facet, its marker: the scene face (`.cbin` index) it lies in.
    pub markers: &'a [u32],
    /// `geometry::check` on those nodes and facets, which must have passed: its cells are the
    /// volumes the regions must fill.
    pub check: &'a CheckReport,
    /// The enabled fitting zones, whose ids TetGen gave the regions their seeds lie in.
    pub fittings: &'a [FittingRegion],
}

/// One region of the mesh (one `idVolume`) against the cell it fills.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct RegionCheck {
    /// Its `idVolume`.
    pub id: i32,
    pub tetrahedra: usize,
    /// The sum of its tetrahedra's volumes, m³.
    pub volume_m3: f64,
    /// The cell the centroid of its largest tetrahedron lies in (0: the exterior); `None` when no
    /// segment decided it.
    pub cell: Option<u32>,
    /// That cell's volume from the geometry check, m³.
    pub cell_volume_m3: Option<f64>,
    /// How far the two volumes may differ: twice the cell's boundary area times the verifier's
    /// distance tolerance (see [`check`]).
    pub tolerance_m3: f64,
    /// The fitting zone whose solver id it is, if any.
    pub fitting: Option<String>,
    /// For a fitting zone's id: the cell its zone is (from its seed, and a box's centre); `None`
    /// when that could not be decided.
    pub zone_cell: Option<u32>,
    /// For a fitting zone's id: the facets its seed lies on (within the tolerance), when it lies
    /// on any. TetGen's choice of side then decides the zone, and this check decides it too, from
    /// the zone's faces.
    pub seed_on_facets: Vec<u32>,
    /// What is wrong with it, in words; empty when it passes.
    pub problems: Vec<String>,
}

/// One cell of the geometry, and the regions that fill it.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct CellCheck {
    /// Its id in the geometry check (1, 2, ...; 0, the exterior, is not listed).
    pub cell: u32,
    pub depth: u32,
    pub volume_m3: f64,
    /// The area of the facets with this cell on exactly one side, m².
    pub boundary_area_m2: f64,
    /// The `idVolume` of each region found in it.
    pub regions: Vec<i32>,
}

/// What [`check`] found.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Outcome {
    pub regions: Vec<RegionCheck>,
    pub cells: Vec<CellCheck>,
    /// Regions whose volume is not their cell's, or that lie in no cell, in the exterior, or in a
    /// cell another region fills too.
    pub region_volume_mismatch: usize,
    /// Cells no region fills.
    pub unmeshed_cells: usize,
    /// Fitting zones whose id is not on their zone's cell, or on no tetrahedron.
    pub fitting_region_misplaced: usize,
    /// Fitting zones whose seed lies on facets between cells that the zone's own faces do not
    /// tell apart.
    pub fitting_seed_ambiguous: usize,
}

fn sub(a: P, b: P) -> P {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn cross(a: P, b: P) -> P {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn norm(a: P) -> f64 {
    (a[0] * a[0] + a[1] * a[1] + a[2] * a[2]).sqrt()
}

/// Segment directions: a golden-angle spiral turned off the axes, as the geometry check's own
/// placement rays (`geometry/check/cells.rs`), so that none runs along an axis-aligned wall.
fn directions() -> impl Iterator<Item = P> {
    const N: usize = 48;
    (0..N).map(|k| {
        let z = 1.0 - (2 * k + 1) as f64 / N as f64;
        let r = (1.0 - z * z).sqrt();
        let phi = k as f64 * 2.399_963_229_728_653 + 0.3;
        [r * phi.cos(), r * phi.sin(), z]
    })
}

impl Reference<'_> {
    fn tri(&self, f: usize) -> [P; 3] {
        self.facets[f].map(|v| self.vertices[v as usize])
    }

    fn cells_of(&self, f: usize) -> Option<[u32; 2]> {
        self.check
            .face_cells
            .get(f)
            .copied()
            .filter(|c| c[0] != NOT_ANALYSED && c[1] != NOT_ANALYSED)
    }

    /// The cell `p` lies in, decided exactly; `None` when every segment tried was degenerate
    /// (`p` on a facet, for one).
    fn locate(&self, p: P) -> Option<u32> {
        let (mut lo, mut hi) = ([f64::INFINITY; 3], [f64::NEG_INFINITY; 3]);
        for v in self.vertices.iter().chain(std::iter::once(&p)) {
            for k in 0..3 {
                lo[k] = lo[k].min(v[k]);
                hi[k] = hi[k].max(v[k]);
            }
        }
        let reach = 2.0 * norm(sub(hi, lo)) + 1.0;
        'direction: for d in directions() {
            let q = [0, 1, 2].map(|k| p[k] + reach * d[k]);
            let mut first: Option<(f64, u32)> = None;
            for f in 0..self.facets.len() {
                let Some([front, back]) = self.cells_of(f) else {
                    continue;
                };
                match predicates::segment_crossing(p, q, &self.tri(f)) {
                    Crossing::Miss => {}
                    Crossing::Degenerate => continue 'direction,
                    Crossing::Hit { t, from_front, .. } => {
                        if first.is_none_or(|(t0, _)| t < t0) {
                            first = Some((t, if from_front { front } else { back }));
                        }
                    }
                }
            }
            return Some(first.map_or(0, |(_, cell)| cell));
        }
        None
    }

    /// The facets `p` lies within `tolerance` of.
    fn facets_near(&self, p: P, tolerance: f64) -> Vec<u32> {
        (0..self.facets.len())
            .filter(|&f| {
                self.cells_of(f).is_some()
                    && geometry::point_triangle_distance(p, self.tri(f)) <= tolerance
            })
            .map(|f| f as u32)
            .collect()
    }

    /// Whether every facet bounding `cell` (with it on exactly one side) is one of `zone_faces`'
    /// (by marker) or has the exterior on its other side: the cell is closed by the zone's own
    /// faces and the room's outer shell.
    fn enclosed_by(&self, cell: u32, zone_faces: &[u32]) -> bool {
        (0..self.facets.len()).all(|f| match self.cells_of(f) {
            Some([a, b]) if a != b && (a == cell || b == cell) => {
                let other = if a == cell { b } else { a };
                other == 0 || zone_faces.binary_search(&self.markers[f]).is_ok()
            }
            _ => true,
        })
    }
}

/// Where a fitting zone's seed lies among `reference`'s cells, and which cell the zone is.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ZoneCell {
    /// The facets the seed lies within the tolerance of.
    pub on_facets: Vec<u32>,
    /// The cells the seed lies in (one), or on (those on either side of `on_facets`), the
    /// exterior left out.
    pub candidates: Vec<u32>,
    /// The zone's cell: a box's, the one its centre lies in, when the seed is in or on it; any
    /// other zone's, its one candidate, or of several, the one closed by the zone's own faces and
    /// the outer shell alone (every facet bounding it is one of the zone's faces, by marker, or
    /// has the exterior behind it). `None` when that decides nothing.
    pub zone_cell: Option<u32>,
}

/// [`ZoneCell`] for `fitting` in `reference`, within `tolerance`.
pub fn zone_cell(reference: &Reference, fitting: &FittingRegion, tolerance: f64) -> ZoneCell {
    let seed = fitting.seed.map(f64::from);
    let on = reference.facets_near(seed, tolerance);
    let mut candidates: Vec<u32> = if on.is_empty() {
        reference.locate(seed).into_iter().collect()
    } else {
        let mut c: Vec<u32> = on
            .iter()
            .flat_map(|&k| reference.cells_of(k as usize).unwrap_or([0, 0]))
            .collect();
        c.sort_unstable();
        c.dedup();
        c
    };
    candidates.retain(|&c| c != 0);
    let mut faces = fitting.faces.clone();
    faces.sort_unstable();
    let zone_cell = match fitting.box_centre {
        Some(centre) => reference
            .locate(centre)
            .filter(|c| candidates.contains(c) || candidates.is_empty()),
        None if candidates.len() == 1 => Some(candidates[0]),
        None => {
            let enclosed: Vec<u32> = candidates
                .iter()
                .copied()
                .filter(|&c| reference.enclosed_by(c, &faces))
                .collect();
            (enclosed.len() == 1).then(|| enclosed[0])
        }
    };
    ZoneCell {
        on_facets: on,
        candidates,
        zone_cell,
    }
}

/// A seed for `fitting` strictly inside its zone's cell, for a seed that lies on a facet: from the
/// seed along the normal of a facet it lies on that bounds the zone's cell, pointing into that
/// cell, to halfway to the next facet, narrowed to `f32`. `None` when the zone's cell is not
/// decided, the seed lies on no facet bounding it, or the point found is not strictly inside it
/// (in it, and farther than `tolerance` from every facet). The seed is then left as it is, and
/// the region volume check judges what TetGen makes of it.
pub fn seed_inside(
    reference: &Reference,
    fitting: &FittingRegion,
    tolerance: f64,
) -> Option<[f32; 3]> {
    let zone = zone_cell(reference, fitting, tolerance);
    let cell = zone.zone_cell?;
    let p = fitting.seed.map(f64::from);
    let (f, into) = zone.on_facets.iter().find_map(|&f| {
        let [front, back] = reference.cells_of(f as usize)?;
        (front != back && (front == cell || back == cell)).then_some((f, front == cell))
    })?;
    let t = reference.tri(f as usize);
    let n = cross(sub(t[1], t[0]), sub(t[2], t[0]));
    let len = norm(n);
    if !len.is_normal() {
        return None;
    }
    let s = if into { 1.0 } else { -1.0 } / len;
    let d = n.map(|c| c * s);
    // The nearest facet the ray from `p` along `d` meets past the tolerance, the facets `p` lies
    // on left out (Möller-Trumbore; the result is checked exactly below).
    let mut nearest = f64::INFINITY;
    for g in 0..reference.facets.len() {
        if zone.on_facets.contains(&(g as u32)) || reference.cells_of(g).is_none() {
            continue;
        }
        let [a, b, c] = reference.tri(g);
        let (e1, e2) = (sub(b, a), sub(c, a));
        let h = cross(d, e2);
        let det = e1[0] * h[0] + e1[1] * h[1] + e1[2] * h[2];
        if det.abs() < 1e-300 {
            continue;
        }
        let inv = 1.0 / det;
        let sv = sub(p, a);
        let u = inv * (sv[0] * h[0] + sv[1] * h[1] + sv[2] * h[2]);
        if !(0.0..=1.0).contains(&u) {
            continue;
        }
        let q = cross(sv, e1);
        let v = inv * (d[0] * q[0] + d[1] * q[1] + d[2] * q[2]);
        if v < 0.0 || u + v > 1.0 {
            continue;
        }
        let dist = inv * (e2[0] * q[0] + e2[1] * q[1] + e2[2] * q[2]);
        if dist > tolerance && dist < nearest {
            nearest = dist;
        }
    }
    if !nearest.is_finite() {
        return None;
    }
    let moved = [0, 1, 2].map(|k| (p[k] + d[k] * nearest / 2.0) as f32);
    let m = moved.map(f64::from);
    (reference.facets_near(m, tolerance).is_empty() && reference.locate(m) == Some(cell))
        .then_some(moved)
}

/// Checks `mesh`'s regions against `reference`'s cells. `tolerance` is the verifier's distance
/// tolerance (`16 · 2⁻²⁴ · R`): every node of a marked face lies within it of its scene face, so
/// a region's boundary lies within it of its cell's, and the two volumes differ by at most the
/// volume of a shell that thick over the cell's boundary. The bound used is twice that,
/// `2 · A · tolerance` for a cell whose boundary has area `A`.
pub fn check(mesh: &mbin::Mesh, reference: &Reference, tolerance: f64) -> Outcome {
    let node = |v: i32| {
        usize::try_from(v)
            .ok()
            .and_then(|i| mesh.nodes.get(i))
            .map(|p| p.map(f64::from))
    };
    // Per idVolume: tetrahedra, volume, and the largest tetrahedron's centroid.
    struct Acc {
        tets: usize,
        volume: f64,
        largest: f64,
        centroid: Option<P>,
    }
    let mut by_id: std::collections::BTreeMap<i32, Acc> = std::collections::BTreeMap::new();
    for tet in &mesh.tetrahedra {
        let acc = by_id.entry(tet.id_volume).or_insert(Acc {
            tets: 0,
            volume: 0.0,
            largest: 0.0,
            centroid: None,
        });
        acc.tets += 1;
        let [Some(a), Some(b), Some(c), Some(d)] = tet.vertices.map(node) else {
            continue;
        };
        let v = geometry::orient([a, b, c, d]).abs() / 6.0;
        if !v.is_finite() {
            continue;
        }
        acc.volume += v;
        if v > acc.largest {
            acc.largest = v;
            acc.centroid = Some([0, 1, 2].map(|k| (a[k] + b[k] + c[k] + d[k]) / 4.0));
        }
    }

    // The cells, with their boundary areas.
    let mut cells: Vec<CellCheck> = reference
        .check
        .cells
        .iter()
        .map(|c| CellCheck {
            cell: c.id,
            depth: c.depth,
            volume_m3: c.volume_m3,
            boundary_area_m2: 0.0,
            regions: Vec::new(),
        })
        .collect();
    for f in 0..reference.facets.len() {
        if let Some([a, b]) = reference.cells_of(f)
            && a != b
        {
            let t = reference.tri(f);
            let area = 0.5 * norm(cross(sub(t[1], t[0]), sub(t[2], t[0])));
            for c in [a, b] {
                if let Some(cell) = cells.iter_mut().find(|x| x.cell == c) {
                    cell.boundary_area_m2 += area;
                }
            }
        }
    }

    let mut out = Outcome::default();
    for (&id, acc) in &by_id {
        let cell = acc.centroid.and_then(|p| reference.locate(p));
        let found = cell.and_then(|c| cells.iter_mut().find(|x| x.cell == c));
        let mut r = RegionCheck {
            id,
            tetrahedra: acc.tets,
            volume_m3: acc.volume,
            cell,
            ..RegionCheck::default()
        };
        match found {
            Some(c) => {
                c.regions.push(id);
                r.cell_volume_m3 = Some(c.volume_m3);
                r.tolerance_m3 = 2.0 * c.boundary_area_m2 * tolerance;
                if (acc.volume - c.volume_m3).abs() > r.tolerance_m3 {
                    r.problems.push(format!(
                        "its volume {:.6} m³ is not cell {}'s {:.6} m³ (tolerance {:.3e} m³)",
                        acc.volume, c.cell, c.volume_m3, r.tolerance_m3
                    ));
                }
            }
            None => r.problems.push(match cell {
                Some(0) => "it lies outside the geometry, in the exterior".to_string(),
                Some(c) => format!("it lies in cell {c}, which the geometry check does not list"),
                None => "no segment decided which cell it lies in".to_string(),
            }),
        }
        out.regions.push(r);
    }
    for c in &cells {
        if c.regions.len() > 1 {
            for r in out.regions.iter_mut().filter(|r| c.regions.contains(&r.id)) {
                r.problems.push(format!(
                    "cell {} holds {} regions, {:?}",
                    c.cell,
                    c.regions.len(),
                    c.regions
                ));
            }
        }
    }
    out.unmeshed_cells = cells.iter().filter(|c| c.regions.is_empty()).count();

    // The fitting zones.
    for f in reference.fittings {
        let ZoneCell {
            on_facets: on,
            candidates,
            zone_cell,
        } = zone_cell(reference, f, tolerance);
        let region = out.regions.iter_mut().find(|r| r.id == f.solver_id);
        let ambiguous = zone_cell.is_none() && f.box_centre.is_none() && candidates.len() > 1;
        if ambiguous {
            out.fitting_seed_ambiguous += 1;
        }
        match region {
            Some(r) => {
                r.fitting = Some(f.zone.clone());
                r.zone_cell = zone_cell;
                r.seed_on_facets = on.clone();
                if ambiguous {
                    r.problems.push(format!(
                        "zone '{}': its seed lies on facets {on:?} between cells {candidates:?}, \
                         and its faces close none of them alone",
                        f.zone
                    ));
                } else if zone_cell.is_none() || zone_cell != r.cell {
                    out.fitting_region_misplaced += 1;
                    r.problems.push(format!(
                        "zone '{}' is cell {}, and its id is on cell {}",
                        f.zone,
                        zone_cell.map_or_else(|| "?".to_string(), |c| c.to_string()),
                        r.cell.map_or_else(|| "?".to_string(), |c| c.to_string())
                    ));
                }
            }
            None => {
                out.fitting_region_misplaced += 1;
                out.regions.push(RegionCheck {
                    id: f.solver_id,
                    fitting: Some(f.zone.clone()),
                    zone_cell,
                    seed_on_facets: on,
                    problems: vec![format!(
                        "zone '{}': no tetrahedron carries its id {}",
                        f.zone, f.solver_id
                    )],
                    ..RegionCheck::default()
                });
            }
        }
    }
    out.regions.sort_by_key(|r| r.id);
    out.region_volume_mismatch = out
        .regions
        .iter()
        .filter(|r| {
            r.tetrahedra > 0
                && (r.cell_volume_m3.is_none()
                    || (r.volume_m3 - r.cell_volume_m3.unwrap_or(0.0)).abs() > r.tolerance_m3
                    || cells
                        .iter()
                        .any(|c| c.regions.len() > 1 && c.regions.contains(&r.id)))
        })
        .count();
    out.cells = cells;
    out
}
