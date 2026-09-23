//! Self-intersections: pairs of faces that meet anywhere other than along the edge or at the
//! vertex they share.
//!
//! "Share" is by **position**: two vertices at exactly the same coordinates count as one point,
//! so an unwelded seam is reported as open edges (its real fault), not as thousands of touching
//! pairs. Anything else is an intersection, touching included: a vertex resting on another face,
//! an edge lying along another face, coplanar overlap. TetGen refuses all of these in a
//! piecewise linear complex, so the check does too.

use super::Mesh;
use super::bvh::{Aabb, Bvh};
use super::predicates;

/// Every intersecting pair `[i, j]` with `i < j`, sorted. `boxes` is indexed by face.
pub(super) fn pairs(mesh: &Mesh, analysed: &[u32], boxes: &[Aabb], bvh: &Bvh) -> Vec<[u32; 2]> {
    let mut found = Vec::new();
    for &i in analysed {
        bvh.query_box(&boxes[i as usize], |j| {
            if j > i && meet(mesh, i, j) {
                found.push([i, j]);
            }
        });
    }
    found.sort_unstable();
    found
}

/// Whether analysed faces `i` and `j` (non-degenerate, not duplicates) meet improperly.
fn meet(mesh: &Mesh, i: u32, j: u32) -> bool {
    let (ti, tj) = (mesh.tri(i), mesh.tri(j));
    let (pi, pj) = (mesh.pids(i), mesh.pids(j));
    let mut shared = [(0usize, 0usize); 3];
    let mut n = 0;
    for (a, pa) in pi.iter().enumerate() {
        if let Some(b) = pj.iter().position(|pb| pb == pa) {
            shared[n] = (a, b);
            n += 1;
        }
    }
    match n {
        0 => predicates::triangles_meet(&ti, &tj),
        1 => {
            let (a, b) = shared[0];
            predicates::vertex_neighbours_meet(
                ti[a],
                ti[(a + 1) % 3],
                ti[(a + 2) % 3],
                tj[(b + 1) % 3],
                tj[(b + 2) % 3],
            )
        }
        2 => {
            let ((a0, b0), (a1, b1)) = (shared[0], shared[1]);
            predicates::edge_neighbours_overlap(ti[a0], ti[a1], ti[3 - a0 - a1], tj[3 - b0 - b1])
        }
        // Three shared positions is a duplicate face; duplicates are not analysed.
        _ => true,
    }
}
