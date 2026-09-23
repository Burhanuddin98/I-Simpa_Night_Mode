//! An axis-aligned bounding-box tree over triangles.
//!
//! Boxes are the exact `min`/`max` of the triangle's `f64` coordinates, and box overlap is tested
//! closed (touching boxes overlap), so the tree never drops a pair of triangles that touch: it is
//! only a filter in front of the exact predicates.

use super::predicates::P3;

/// A closed axis-aligned box.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Aabb {
    pub min: P3,
    pub max: P3,
}

impl Aabb {
    pub const EMPTY: Aabb = Aabb {
        min: [f64::INFINITY; 3],
        max: [f64::NEG_INFINITY; 3],
    };

    pub fn of_points(points: &[P3]) -> Aabb {
        points.iter().fold(Aabb::EMPTY, |b, p| b.with_point(*p))
    }

    fn with_point(self, p: P3) -> Aabb {
        Aabb {
            min: [0, 1, 2].map(|k| self.min[k].min(p[k])),
            max: [0, 1, 2].map(|k| self.max[k].max(p[k])),
        }
    }

    pub fn union(self, o: Aabb) -> Aabb {
        Aabb {
            min: [0, 1, 2].map(|k| self.min[k].min(o.min[k])),
            max: [0, 1, 2].map(|k| self.max[k].max(o.max[k])),
        }
    }

    /// Closed overlap: boxes that only touch overlap.
    pub fn overlaps(&self, o: &Aabb) -> bool {
        (0..3).all(|k| self.min[k] <= o.max[k] && o.min[k] <= self.max[k])
    }

    fn centre(&self, axis: usize) -> f64 {
        0.5 * (self.min[axis] + self.max[axis])
    }

    /// Length of the diagonal.
    pub fn diagonal(&self) -> f64 {
        (0..3)
            .map(|k| (self.max[k] - self.min[k]).powi(2))
            .sum::<f64>()
            .sqrt()
    }

    /// Whether the segment `p q` passes within `pad` of the box (a conservative slab test).
    fn meets_segment(&self, p: P3, q: P3, pad: f64) -> bool {
        let (mut t0, mut t1) = (0.0f64, 1.0f64);
        for k in 0..3 {
            let (lo, hi) = (self.min[k] - pad, self.max[k] + pad);
            let d = q[k] - p[k];
            if d == 0.0 {
                if p[k] < lo || p[k] > hi {
                    return false;
                }
                continue;
            }
            let (a, b) = ((lo - p[k]) / d, (hi - p[k]) / d);
            let (a, b) = if a <= b { (a, b) } else { (b, a) };
            t0 = t0.max(a);
            t1 = t1.min(b);
            if t0 > t1 {
                return false;
            }
        }
        true
    }
}

#[derive(Clone, Copy, Debug)]
struct Node {
    bbox: Aabb,
    /// Leaf: first index into `items`. Inner: index of the left child (the right one follows the
    /// left subtree, at `right`).
    start: u32,
    /// Leaf: number of items (> 0). Inner: 0.
    count: u32,
    right: u32,
}

const LEAF_SIZE: usize = 4;

/// Bounding-box tree over items identified by `u32` (face indices).
pub(crate) struct Bvh {
    nodes: Vec<Node>,
    items: Vec<(u32, Aabb)>,
}

impl Bvh {
    pub fn build(mut items: Vec<(u32, Aabb)>) -> Bvh {
        let mut nodes = Vec::with_capacity(2 * items.len() / LEAF_SIZE + 1);
        if !items.is_empty() {
            let n = items.len();
            build_node(&mut nodes, &mut items, 0, n);
        }
        Bvh { nodes, items }
    }

    /// Calls `visit` for every item whose box overlaps `query` (closed).
    pub fn query_box(&self, query: &Aabb, mut visit: impl FnMut(u32)) {
        self.walk(
            |b| b.overlaps(query),
            |id, b| {
                if b.overlaps(query) {
                    visit(id);
                }
            },
        );
    }

    /// Calls `visit` for every item whose box passes within `pad` of the segment `p q`.
    pub fn query_segment(&self, p: P3, q: P3, pad: f64, mut visit: impl FnMut(u32)) {
        self.walk(
            |b| b.meets_segment(p, q, pad),
            |id, b| {
                if b.meets_segment(p, q, pad) {
                    visit(id);
                }
            },
        );
    }

    fn walk(&self, enter: impl Fn(&Aabb) -> bool, mut leaf: impl FnMut(u32, &Aabb)) {
        if self.nodes.is_empty() {
            return;
        }
        let mut stack = vec![0u32];
        while let Some(i) = stack.pop() {
            let node = &self.nodes[i as usize];
            if !enter(&node.bbox) {
                continue;
            }
            if node.count > 0 {
                let range = node.start as usize..(node.start + node.count) as usize;
                for (id, b) in &self.items[range] {
                    leaf(*id, b);
                }
            } else {
                stack.push(node.right);
                stack.push(node.start);
            }
        }
    }
}

/// Builds the subtree over `items[lo..hi]` and returns its node index.
fn build_node(nodes: &mut Vec<Node>, items: &mut [(u32, Aabb)], lo: usize, hi: usize) -> u32 {
    let bbox = items[lo..hi]
        .iter()
        .fold(Aabb::EMPTY, |acc, (_, b)| acc.union(*b));
    let index = nodes.len() as u32;
    nodes.push(Node {
        bbox,
        start: lo as u32,
        count: (hi - lo) as u32,
        right: 0,
    });
    if hi - lo <= LEAF_SIZE {
        return index;
    }
    // Split at the median centre along the axis where the centres spread most.
    let spread = |axis: usize| {
        let (mn, mx) = items[lo..hi]
            .iter()
            .fold((f64::INFINITY, f64::NEG_INFINITY), |(mn, mx), (_, b)| {
                (mn.min(b.centre(axis)), mx.max(b.centre(axis)))
            });
        mx - mn
    };
    let axis = (0..3)
        .max_by(|&a, &b| spread(a).total_cmp(&spread(b)))
        .unwrap_or(0);
    let mid = lo + (hi - lo) / 2;
    items[lo..hi].select_nth_unstable_by(mid - lo, |(ia, a), (ib, b)| {
        a.centre(axis).total_cmp(&b.centre(axis)).then(ia.cmp(ib))
    });
    let left = build_node(nodes, items, lo, mid);
    let right = build_node(nodes, items, mid, hi);
    let node = &mut nodes[index as usize];
    node.count = 0;
    node.start = left;
    node.right = right;
    index
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn box_query_finds_exactly_the_overlapping_items() {
        // A 20 x 20 grid of unit boxes; a query box touching four of them at a corner.
        let mut items = Vec::new();
        for i in 0..20u32 {
            for j in 0..20u32 {
                let (x, y) = (f64::from(i), f64::from(j));
                items.push((
                    i * 20 + j,
                    Aabb {
                        min: [x, y, 0.0],
                        max: [x + 1.0, y + 1.0, 1.0],
                    },
                ));
            }
        }
        let all = items.clone();
        let bvh = Bvh::build(items);
        let q = Aabb {
            min: [4.5, 4.5, 0.5],
            max: [5.0, 5.0, 0.5],
        };
        let mut found = Vec::new();
        bvh.query_box(&q, |id| found.push(id));
        found.sort_unstable();
        let mut expected: Vec<u32> = all
            .iter()
            .filter(|(_, b)| b.overlaps(&q))
            .map(|(id, _)| *id)
            .collect();
        expected.sort_unstable();
        assert_eq!(found, expected);
        assert_eq!(found, vec![4 * 20 + 4, 4 * 20 + 5, 5 * 20 + 4, 5 * 20 + 5]);
    }

    #[test]
    fn segment_query_is_conservative() {
        let items: Vec<(u32, Aabb)> = (0..50u32)
            .map(|i| {
                let x = f64::from(i);
                (
                    i,
                    Aabb {
                        min: [x, 0.0, 0.0],
                        max: [x + 0.5, 1.0, 1.0],
                    },
                )
            })
            .collect();
        let bvh = Bvh::build(items);
        let mut found = Vec::new();
        bvh.query_segment([-1.0, 0.5, 0.5], [100.0, 0.5, 0.5], 0.0, |id| {
            found.push(id)
        });
        found.sort_unstable();
        assert_eq!(found, (0..50).collect::<Vec<_>>());
        let mut none = Vec::new();
        bvh.query_segment([-1.0, 2.0, 0.5], [100.0, 2.0, 0.5], 0.0, |id| none.push(id));
        assert!(none.is_empty());
    }
}
