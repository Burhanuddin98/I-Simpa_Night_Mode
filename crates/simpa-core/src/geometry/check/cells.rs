//! Which volume each side of each face faces.
//!
//! Every analysed face has two sides, front (the side its normal `(b - a) x (c - a)` points to)
//! and back. Sides that face the same air are joined:
//!
//! 1. **Around each edge.** The faces using an edge are sorted by angle about it (exactly, with
//!    `orient3d`). Between two consecutive faces lies a wedge of air; the two sides facing that
//!    wedge are joined. An edge used once joins its face's front and back (air flows round a
//!    free edge). With two faces this is the usual manifold rule, and it needs no sorting.
//! 2. **Across gaps.** A face-connected component that touches no other (a floating reflector,
//!    a fitting zone, the room itself) is placed by casting one ray from a point on it to far
//!    outside: the last hit on its own faces gives its outward side; the first hit on each other
//!    component gives the side of that component it sits in. A component enclosed by several
//!    others is placed in the innermost one. Degenerate rays (through an edge or a vertex, or
//!    grazing a plane) are detected exactly and replaced by the next direction.
//!
//! The joined classes are the **cells**: cell 0 is the exterior, the others are enclosed
//! volumes. A cell's depth is its distance from the exterior across faces, so the room is 1 and
//! a fitting zone floating in it is 2.

use std::cmp::Ordering;
use std::collections::HashMap;
use std::collections::VecDeque;

use super::Mesh;
use super::bvh::{Aabb, Bvh};
use super::predicates::{self, Crossing, P3};

/// "No value" for per-face and per-cell tables.
pub(super) const NONE: u32 = u32::MAX;

pub(super) struct UnionFind {
    parent: Vec<u32>,
    size: Vec<u32>,
}

impl UnionFind {
    pub fn new(n: usize) -> Self {
        UnionFind {
            parent: (0..n as u32).collect(),
            size: vec![1; n],
        }
    }

    pub fn find(&mut self, mut x: u32) -> u32 {
        while self.parent[x as usize] != x {
            let grand = self.parent[self.parent[x as usize] as usize];
            self.parent[x as usize] = grand;
            x = grand;
        }
        x
    }

    pub fn union(&mut self, a: u32, b: u32) {
        let (mut a, mut b) = (self.find(a), self.find(b));
        if a == b {
            return;
        }
        if self.size[a as usize] < self.size[b as usize] {
            std::mem::swap(&mut a, &mut b);
        }
        self.parent[b as usize] = a;
        self.size[a as usize] += self.size[b as usize];
    }
}

/// One use of an edge `u < v` by an analysed face.
#[derive(Clone, Copy, Debug)]
pub(super) struct Use {
    pub face: u32,
    /// The face runs along the edge from `u` to `v` (its vertex cycle contains `u, v`).
    pub fwd: bool,
    /// The face's vertex that is not on the edge.
    pub third: u32,
}

pub(super) fn front(face: u32) -> u32 {
    2 * face
}

pub(super) fn back(face: u32) -> u32 {
    2 * face + 1
}

/// The side of `a` that faces the wedge following it in right-hand rotation about `u -> v`.
/// Rotating the face `u, v, w` a little about `u -> v` moves it along `(v - u) x (w - u)`, its
/// normal; so for a face running `u -> v` that wedge is on its front.
fn after(a: &Use) -> u32 {
    if a.fwd { front(a.face) } else { back(a.face) }
}

/// The side of `b` that faces the wedge preceding it.
fn before(b: &Use) -> u32 {
    if b.fwd { back(b.face) } else { front(b.face) }
}

/// What an edge contributed, kept to classify it once the cells are known.
#[derive(Clone, Debug)]
pub(super) enum EdgeTopo {
    /// Used once; the side node of its face.
    Open(u32),
    /// Used twice.
    Manifold,
    /// Used three times or more; for each wedge, a side node facing it.
    Junction(Vec<u32>),
}

/// The side graph: nodes `2f` (front of face f) and `2f + 1` (back), plus the exterior.
pub(super) struct Sides {
    pub uf: UnionFind,
    pub exterior: u32,
    /// Edges whose incident faces tie in angle (overlap); the result is then unreliable.
    pub ties: usize,
    /// Manifold edges both faces run in the same direction.
    pub conflicts: usize,
}

impl Sides {
    pub fn new(faces: usize) -> Self {
        Sides {
            uf: UnionFind::new(2 * faces + 1),
            exterior: 2 * faces as u32,
            ties: 0,
            conflicts: 0,
        }
    }

    /// Joins the sides around the edge `u < v` used by the analysed faces `uses`.
    pub fn join_edge(&mut self, pos: &[P3], u: u32, v: u32, uses: &mut [Use]) -> EdgeTopo {
        match uses.len() {
            0 => EdgeTopo::Manifold,
            1 => {
                let f = uses[0].face;
                self.uf.union(front(f), back(f));
                EdgeTopo::Open(front(f))
            }
            2 => {
                if uses[0].fwd == uses[1].fwd {
                    self.conflicts += 1;
                }
                self.uf.union(after(&uses[0]), before(&uses[1]));
                self.uf.union(after(&uses[1]), before(&uses[0]));
                EdgeTopo::Manifold
            }
            k => {
                if radial_sort(pos, u, v, uses) {
                    self.ties += 1;
                }
                for i in 0..k {
                    let (a, b) = (uses[i], uses[(i + 1) % k]);
                    self.uf.union(after(&a), before(&b));
                }
                EdgeTopo::Junction(uses.iter().map(after).collect())
            }
        }
    }
}

/// Sorts the uses of edge `u < v` by the angle of their third vertex about `u -> v`, exactly.
/// Returns whether two faces lie on one half-plane (they overlap; the order is then arbitrary).
fn radial_sort(pos: &[P3], u: u32, v: u32, uses: &mut [Use]) -> bool {
    let (pu, pv) = (pos[u as usize], pos[v as usize]);
    let r = pos[uses[0].third as usize];
    let key = |x: &Use| predicates::half_turn(pu, pv, r, pos[x.third as usize]);
    let order = |a: &Use, b: &Use| {
        let (ka, kb) = (key(a), key(b));
        ka.cmp(&kb).then_with(|| {
            if ka == 1 || ka == 3 {
                predicates::angular_order(pu, pv, pos[a.third as usize], pos[b.third as usize])
            } else {
                Ordering::Equal
            }
        })
    };
    uses.sort_by(order);
    uses.windows(2)
        .any(|w| order(&w[0], &w[1]) == Ordering::Equal)
}

/// Directions tried for the placement rays: a golden-angle spiral, turned off the axes so that no
/// direction is parallel to an axis-aligned wall.
fn directions() -> impl Iterator<Item = P3> {
    const N: usize = 48;
    (0..N).map(|k| {
        let z = 1.0 - (2 * k + 1) as f64 / N as f64;
        let r = (1.0 - z * z).sqrt();
        let phi = k as f64 * 2.399_963_229_728_653 + 0.3;
        [r * phi.cos(), r * phi.sin(), z]
    })
}

struct Ray {
    /// The side node facing out of the component (towards the far end of the ray).
    exit: u32,
    /// For each other component the ray meets: the side node of its first hit that faces the
    /// ray's start, which lies in the same cell of that component as the start.
    first: Vec<(u32, u32)>,
}

fn sub(a: P3, b: P3) -> P3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn cross(a: P3, b: P3) -> P3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn dot(a: P3, b: P3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn length(a: P3) -> f64 {
    dot(a, a).sqrt()
}

/// The inscribed-circle radius, `2 * area / perimeter`: the ray starts at the centroid of the
/// fattest face, far (relative to rounding) from that face's edges.
fn inradius(t: &[P3; 3]) -> f64 {
    let area2 = length(cross(sub(t[1], t[0]), sub(t[2], t[0])));
    let perimeter = length(sub(t[1], t[0])) + length(sub(t[2], t[1])) + length(sub(t[0], t[2]));
    if perimeter > 0.0 {
        area2 / perimeter
    } else {
        0.0
    }
}

fn cast(mesh: &Mesh, bvh: &Bvh, comp_of: &[u32], comp: u32, f0: u32, bbox: &Aabb) -> Option<Ray> {
    let t0 = mesh.tri(f0);
    let p = [0, 1, 2].map(|k| (t0[0][k] + t0[1][k] + t0[2][k]) / 3.0);
    let n = cross(sub(t0[1], t0[0]), sub(t0[2], t0[0]));
    let n_len = length(n);
    if !n_len.is_normal() {
        return None;
    }
    let reach = 2.0 * bbox.diagonal() + 1.0;
    let pad = 1e-9 * reach;
    for d in directions() {
        let dn = dot(n, d) / n_len;
        if dn.abs() < 0.2 {
            continue;
        }
        let q = [0, 1, 2].map(|k| p[k] + reach * d[k]);
        let mut hits: Vec<(f64, u32, bool, bool)> = Vec::new();
        let mut degenerate = false;
        bvh.query_segment(p, q, pad, |g| {
            if degenerate || g == f0 {
                return;
            }
            match predicates::segment_crossing(p, q, &mesh.tri(g)) {
                Crossing::Miss => {}
                Crossing::Degenerate => degenerate = true,
                Crossing::Hit {
                    t,
                    from_front,
                    to_front,
                } => hits.push((t, g, from_front, to_front)),
            }
        });
        if degenerate {
            continue;
        }
        hits.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
        let exit = match hits.iter().rev().find(|h| comp_of[h.1 as usize] == comp) {
            Some(&(_, g, _, to_front)) => {
                if to_front {
                    front(g)
                } else {
                    back(g)
                }
            }
            None => {
                if dn > 0.0 {
                    front(f0)
                } else {
                    back(f0)
                }
            }
        };
        let mut first: Vec<(u32, u32)> = Vec::new();
        for &(_, g, from_front, _) in &hits {
            let c = comp_of[g as usize];
            if c != comp && !first.iter().any(|&(seen, _)| seen == c) {
                first.push((c, if from_front { front(g) } else { back(g) }));
            }
        }
        return Some(Ray { exit, first });
    }
    None
}

/// The cells, and what else the placement found.
pub(super) struct Cells {
    /// Per face: the cell on its front and on its back ([`NONE`] for faces not analysed).
    pub front_cell: Vec<u32>,
    pub back_cell: Vec<u32>,
    /// Per cell: its depth ([`NONE`] if unreachable, which only happens when placement failed).
    pub depth: Vec<u32>,
    /// Face-connected components of the analysed faces.
    pub components: usize,
    /// Components whose placement no ray could decide.
    pub unresolved: usize,
    /// Components placed inside two others that do not nest (only with intersecting faces).
    pub ambiguous: usize,
    /// Node-to-cell lookup, for classifying edges.
    node_cell: HashMap<u32, u32>,
    uf: UnionFind,
}

impl Cells {
    /// The cell a side node belongs to.
    pub fn cell_of(&mut self, node: u32) -> u32 {
        let root = self.uf.find(node);
        self.node_cell.get(&root).copied().unwrap_or(NONE)
    }
}

/// Places the components and builds the cells. `components` holds, per face, a representative
/// face of its component (from joining faces across edges), or [`NONE`] if not analysed.
pub(super) fn build(
    mesh: &Mesh,
    analysed: &[u32],
    mut sides: Sides,
    mut components: UnionFind,
    bvh: &Bvh,
    bbox: &Aabb,
) -> (Cells, usize, usize) {
    let faces = mesh.tris.len();
    // Number the components in order of their first face.
    let mut comp_of = vec![NONE; faces];
    let mut comp_rep: HashMap<u32, u32> = HashMap::new();
    let mut seeds: Vec<u32> = Vec::new();
    let mut best: Vec<f64> = Vec::new();
    for &f in analysed {
        let root = components.find(f);
        let next = comp_rep.len() as u32;
        let c = *comp_rep.entry(root).or_insert(next);
        comp_of[f as usize] = c;
        let r = inradius(&mesh.tri(f));
        if c as usize == seeds.len() {
            seeds.push(f);
            best.push(r);
        } else if r > best[c as usize] {
            seeds[c as usize] = f;
            best[c as usize] = r;
        }
    }
    let m = seeds.len();
    let rays: Vec<Option<Ray>> = (0..m)
        .map(|c| cast(mesh, bvh, &comp_of, c as u32, seeds[c], bbox))
        .collect();

    // Decide the nesting on the edge-joined classes, before any placement join.
    let out_root: Vec<Option<u32>> = rays
        .iter()
        .map(|r| r.as_ref().map(|r| sides.uf.find(r.exit)))
        .collect();
    let mut enclosing: Vec<Vec<(u32, u32)>> = vec![Vec::new(); m];
    for (c, ray) in rays.iter().enumerate() {
        let Some(ray) = ray else { continue };
        for &(j, node) in &ray.first {
            if let Some(out_j) = out_root[j as usize]
                && sides.uf.find(node) != out_j
            {
                enclosing[c].push((j, node));
            }
        }
    }
    let nesting: Vec<usize> = enclosing.iter().map(Vec::len).collect();
    let mut unresolved = 0;
    let mut ambiguous = 0;
    for c in 0..m {
        let target = match &rays[c] {
            None => {
                unresolved += 1;
                None
            }
            Some(_) => {
                let mut candidates = enclosing[c].clone();
                candidates.sort_by_key(|&(j, _)| (std::cmp::Reverse(nesting[j as usize]), j));
                if candidates.len() >= 2
                    && nesting[candidates[0].0 as usize] == nesting[candidates[1].0 as usize]
                {
                    ambiguous += 1;
                }
                candidates.first().map(|&(_, node)| node)
            }
        };
        let own = match &rays[c] {
            Some(ray) => ray.exit,
            None => front(seeds[c]),
        };
        sides.uf.union(own, target.unwrap_or(sides.exterior));
    }

    // Cells: exterior first, then in order of first appearance.
    let mut uf = sides.uf;
    let mut node_cell: HashMap<u32, u32> = HashMap::new();
    node_cell.insert(uf.find(sides.exterior), 0);
    let mut front_cell = vec![NONE; faces];
    let mut back_cell = vec![NONE; faces];
    for &f in analysed {
        for (node, slot) in [(front(f), &mut front_cell), (back(f), &mut back_cell)] {
            let root = uf.find(node);
            let next = node_cell.len() as u32;
            slot[f as usize] = *node_cell.entry(root).or_insert(next);
        }
    }
    let cells = node_cell.len();
    let mut adjacency: Vec<Vec<u32>> = vec![Vec::new(); cells];
    for &f in analysed {
        let (a, b) = (front_cell[f as usize], back_cell[f as usize]);
        if a != b {
            adjacency[a as usize].push(b);
            adjacency[b as usize].push(a);
        }
    }
    let mut depth = vec![NONE; cells];
    depth[0] = 0;
    let mut queue = VecDeque::from([0u32]);
    while let Some(c) = queue.pop_front() {
        for &n in &adjacency[c as usize] {
            if depth[n as usize] == NONE {
                depth[n as usize] = depth[c as usize] + 1;
                queue.push_back(n);
            }
        }
    }
    let ties = sides.ties;
    let conflicts = sides.conflicts;
    (
        Cells {
            front_cell,
            back_cell,
            depth,
            components: m,
            unresolved,
            ambiguous,
            node_cell,
            uf,
        },
        ties,
        conflicts,
    )
}
