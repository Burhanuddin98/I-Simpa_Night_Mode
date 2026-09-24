//! The 32-bit round trip every coordinate of upstream's scene takes on its way to the solvers.
//!
//! Upstream's GUI holds its scene in OpenGL coordinates, `f32`, centred and scaled to fit in
//! [-1, 1] (`CObjet3D::Unitize`, `3dengine/Core/Objet3D.cpp:527-571`). Every file it hands the
//! solvers converts back to world coordinates on the way out (`GlCoordsToCommonCoords`,
//! `3dengine/Core/Mathlib.h:50-54`): the scene mesh `.cbin` (`CObjet3D::ToCBINFormat`,
//! `Objet3D_maillage.cpp:777`), the `.poly` TetGen reads (`:941`), and the `.mbin`, whose nodes
//! come back from TetGen through `CommonCoordsToGlCoords` (`Mathlib.h:62-67`, in `LoadNodeFile`,
//! `:99`) and out again (`CObjet3D::GetTetraMesh`, `:857`). Each step is an `f32` operation on
//! values of the scene's own size, so a world coordinate can come back moved by up to about one
//! unit in the last place of the scene's largest coordinate (up to 4.77e-7 m on tutorial 1's 10 m
//! box, 198 of its 732 mesh nodes), and `0` on the world Y axis always comes back as `-0`.
//!
//! [`GlFrame`] holds upstream's `UnitizeVar` for a scene and applies that round trip. With it our
//! `.cbin` holds the same `f32` bits as upstream's for the same scene (`docs/formats/cbin.md`,
//! "Parity with upstream's GUI").

use crate::schema::Project;

/// Upstream's `UnitizeVar` of a scene: the centre of its bounding box and the scale that fits it
/// in [-1, 1], both in OpenGL axes (world `x`, `z`, `-y`) and both `f32`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GlFrame {
    /// `UnitizeVar.x`, `.y`, `.z`: the bounding box's centre, OpenGL axes.
    pub centre: [f32; 3],
    /// `UnitizeVar.w`: 2 over the bounding box's largest extent.
    pub scale: f32,
}

impl GlFrame {
    /// The frame `Unitize` computes for these world vertices (metres, Z up), or `None` when there
    /// are none, or when the frame would not be finite and positive (every vertex at one point,
    /// or an extent beyond the `f32` range), where upstream's scene would be all NaN.
    ///
    /// The arithmetic is upstream's: the box in `f32`, each centre as `(lo + hi) / 2.0` with the
    /// sum in `f32` and the halving in `f64` (`Objet3D.cpp:556-558`), and the scale as
    /// `2.0 / max(w, h, d)` in `f64`, stored as `f32` (`:560`).
    ///
    /// One difference, deliberate: upstream's box loop stops one vertex short
    /// (`v < size() - 1`, `Objet3D.cpp:542`), so its last vertex never widens the box. This
    /// takes every vertex. The two agree unless that last vertex of upstream's own list is the
    /// only one at an extreme; our welded list has no such order to follow.
    pub fn of_vertices<I: IntoIterator<Item = [f32; 3]>>(vertices: I) -> Option<GlFrame> {
        let mut lo = [f32::INFINITY; 3];
        let mut hi = [f32::NEG_INFINITY; 3];
        let mut any = false;
        for v in vertices {
            let g = to_gl_axes(v);
            for k in 0..3 {
                // Upstream's comparisons: `if (v < Lo) Lo = v; if (v > Hi) Hi = v;`.
                if g[k] < lo[k] {
                    lo[k] = g[k];
                }
                if g[k] > hi[k] {
                    hi[k] = g[k];
                }
            }
            any = true;
        }
        if !any {
            return None;
        }
        let extent = [0, 1, 2].map(|k| (hi[k] - lo[k]).abs());
        let centre = [0, 1, 2].map(|k| (f64::from(lo[k] + hi[k]) / 2.0) as f32);
        let largest = extent[0].max(extent[1]).max(extent[2]);
        let scale = (2.0 / f64::from(largest)) as f32;
        (scale.is_finite() && scale > 0.0 && centre.iter().all(|c| c.is_finite()))
            .then_some(GlFrame { centre, scale })
    }

    /// The frame of a project's scene: [`GlFrame::of_vertices`] over its vertices narrowed to
    /// `f32`, the precision upstream holds a scene in.
    pub fn of_project(project: &Project) -> Option<GlFrame> {
        GlFrame::of_vertices(
            project
                .geometry
                .vertices
                .iter()
                .map(|v| v.to_array().map(|c| c as f32)),
        )
    }

    /// World to OpenGL coordinates, `CommonCoordsToGlCoords` (`Mathlib.h:62-67`):
    /// `((x - cx) s, (z - cy) s, (-y - cz) s)`, in `f32`.
    pub fn to_gl(&self, [x, y, z]: [f32; 3]) -> [f32; 3] {
        let [cx, cy, cz] = self.centre;
        let s = self.scale;
        [(x - cx) * s, (z - cy) * s, (-y - cz) * s]
    }

    /// OpenGL to world coordinates, `GlCoordsToCommonCoords` (`Mathlib.h:50-54`):
    /// `(gx / s + cx, -(gz / s + cz), gy / s + cy)`, in `f32`.
    pub fn to_world(&self, [gx, gy, gz]: [f32; 3]) -> [f32; 3] {
        let [cx, cy, cz] = self.centre;
        let s = self.scale;
        [gx / s + cx, -(gz / s + cz), gy / s + cy]
    }

    /// A world point in and out again: what upstream writes for a point it holds.
    pub fn round_trip(&self, v: [f32; 3]) -> [f32; 3] {
        self.to_world(self.to_gl(v))
    }
}

/// World axes to OpenGL axes without the frame: `(x, z, -y)`, exact. This is how upstream loads a
/// scene before `Unitize` (`3dengine/Core/bin.cpp:78`, `CommonCoordsToGlCoords` with the identity
/// frame for the other formats).
fn to_gl_axes([x, y, z]: [f32; 3]) -> [f32; 3] {
    [x, z, -y]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bits(v: [f32; 3]) -> [u32; 3] {
        v.map(f32::to_bits)
    }

    #[test]
    fn tutorial1_frame_is_upstreams() {
        // The 6 x 10 x 3 m box: UnitizeVar = (3, 1.5, -5, 0.2f), as the pipeline investigation
        // found it reproducing the 2019 tetramesh.mbin bit for bit.
        let corners = [[0.0, 0.0, 0.0], [6.0, 10.0, 3.0], [6.0, 0.0, 3.0]];
        let f = GlFrame::of_vertices(corners).unwrap();
        assert_eq!(f.centre, [3.0, 1.5, -5.0]);
        assert_eq!(f.scale.to_bits(), (2.0f32 / 10.0).to_bits());
        // y = 0 comes back as -0, as in upstream's mesh.cbin.
        assert_eq!(bits(f.round_trip([6.0, 0.0, 0.0])), bits([6.0, -0.0, 0.0]));
        assert_eq!(bits(f.round_trip([0.0, 10.0, 3.0])), bits([0.0, 10.0, 3.0]));
    }

    #[test]
    fn round_trip_moves_some_coordinates_a_little() {
        // A frame whose scale is not a power of two: some values do not come back.
        let f = GlFrame::of_vertices([[0.0, 0.0, 0.0], [9.0, 15.0, 4.5]]).unwrap();
        let moved = (0..2000)
            .map(|i| i as f32 * 0.0075)
            .filter(|&x| f.round_trip([x, 1.0, 1.0])[0].to_bits() != x.to_bits())
            .count();
        assert!(moved > 0, "the round trip is not the identity");
        // Bounded by the rounding of values of the scene's size (15 m here), not of x's own.
        for i in 0..2000 {
            let x = i as f32 * 0.0075;
            let back = f.round_trip([x, 1.0, 1.0])[0];
            assert!(
                (back - x).abs() <= 2.0 * f32::EPSILON * 15.0,
                "{x} -> {back}"
            );
        }
    }

    #[test]
    fn degenerate_scenes_have_no_frame() {
        assert_eq!(GlFrame::of_vertices(std::iter::empty()), None);
        assert_eq!(GlFrame::of_vertices([[1.0, 2.0, 3.0]; 4]), None);
        assert_eq!(
            GlFrame::of_vertices([[-3.0e38, 0.0, 0.0], [3.0e38, 1.0, 1.0]]),
            None,
            "an extent beyond f32"
        );
    }
}
