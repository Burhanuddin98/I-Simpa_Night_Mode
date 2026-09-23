//! Mesh verification: the `.mbin` invariants and the checks on a folder of TetGen output that
//! `simpa mesh-verify` reports. See `docs/m5-m6-design.md`, "Interfaces between pieces".
//!
//! Scaffold: the types are the interface the mesher and the CLI code against; the bodies are
//! filled in by the verification work.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::formats::{FormatError, cbin, mbin};

/// What [`verify_mesh`] accepts as volume ids. The default is this crate's convention: room 0,
/// no fittings.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct VolumeIds {
    /// The id room tetrahedra carry: 0 for meshes this crate builds, 1 for upstream's own.
    pub room: i32,
    /// The solver ids of the enabled fitting zones.
    pub fittings: Vec<i32>,
}

/// The result of [`verify_mesh`]. Every count is a number of offending items; `codes` lists the
/// reason code of each non-zero count, and is empty exactly when the mesh passes.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct VerifyReport {
    pub tetrahedra: usize,
    pub nodes: usize,
    /// Scene faces in the `.cbin` the markers index.
    pub scene_faces: usize,
    /// Corner or neighbour indices out of range (`index_errors`).
    pub index_errors: usize,
    /// Tetrahedra with a repeated corner or a volume at or below the f32 noise floor
    /// (`degenerate_tets`).
    pub degenerate_tets: usize,
    /// Tetrahedra with `(A−D)·((B−D)×(C−D)) >= 0`, against the `.mbin` convention
    /// (`inverted_tets`).
    pub inverted_tets: usize,
    /// Faces with neighbour -2 and a marker below 0 (`unmarked_boundary_faces`).
    pub unmarked_boundary_faces: usize,
    /// Markers at or above `scene_faces` (`marker_out_of_range`).
    pub marker_out_of_range: usize,
    /// Neighbour links that are not returned by the neighbour (`nonmutual_neighbors`).
    pub nonmutual_neighbors: usize,
    /// Marked faces with a neighbour whose shared face carries a different marker
    /// (`asymmetric_internal_faces`).
    pub asymmetric_internal_faces: usize,
    /// Marked faces that do not lie on the scene face their marker names
    /// (`marker_geometry_mismatches`).
    pub marker_geometry_mismatches: usize,
    /// Scene faces no tetrahedron face carries (`uncovered_scene_faces`): the count, and the
    /// first 20 face indices.
    pub uncovered_scene_faces: usize,
    pub uncovered_scene_faces_first: Vec<u32>,
    /// Tetrahedra whose `idVolume` is neither the room's nor a fitting's (`unknown_volume_ids`).
    pub unknown_volume_ids: usize,
    /// Total volume in cubic metres per `idVolume`.
    pub volume_by_id: BTreeMap<i32, f64>,
    pub codes: Vec<String>,
}

impl VerifyReport {
    pub fn passed(&self) -> bool {
        self.codes.is_empty()
    }
}

/// Checks `mesh` against the `.mbin` invariants (`docs/m5-m6-design.md`, decision 6) and against
/// the scene its markers index.
pub fn verify_mesh(mesh: &mbin::Mesh, scene: &cbin::Model, ids: &VolumeIds) -> VerifyReport {
    let _ = (mesh, scene, ids);
    VerifyReport::default()
}

/// The result of [`verify_dir`].
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct DirReport {
    pub dir: PathBuf,
    /// The TetGen basename found (`scene_mesh`, `model`, ...), if any.
    pub basename: Option<String>,
    /// Rows in `<base>_skipped.face`, and their markers.
    pub skipped_facets: usize,
    pub skipped_markers: Vec<i32>,
    /// The `.mbin` check, when the folder holds a `.mbin` and the `.cbin` it indexes.
    pub mesh: Option<VerifyReport>,
    /// Reason codes: the folder-level ones (`tetgen_skipped_facets`, `neigh_missing`,
    /// `tetgen_output_missing`, `manifest_mismatch`) followed by the mesh report's.
    pub codes: Vec<String>,
}

impl DirReport {
    pub fn passed(&self) -> bool {
        self.codes.is_empty()
    }
}

/// Checks a folder holding TetGen output, a `.mbin` and its `.cbin`, a mesh manifest, or any
/// subset of them, whatever the TetGen basename.
pub fn verify_dir(dir: &Path, ids: &VolumeIds) -> Result<DirReport, FormatError> {
    let _ = ids;
    Ok(DirReport {
        dir: dir.to_path_buf(),
        ..DirReport::default()
    })
}
