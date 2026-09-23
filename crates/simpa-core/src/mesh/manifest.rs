//! `mesh.json`, the mesh manifest: what was meshed, how, with which TetGen, what came out, and
//! why it failed when it did. Specified in `docs/formats/mesh-manifest.md`.

use std::io;
use std::path::Path;

use serde::{Deserialize, Serialize};

use super::build::BuildStats;
use super::diag::Intersection;
use super::input::ZoneFacets;
use super::verify::{VerifyReport, VolumeIds};
use crate::formats::FormatError;

/// The manifest's file name in a mesh folder.
pub const MANIFEST_FILE: &str = "mesh.json";

/// The layout version of [`MeshManifest`].
pub const MANIFEST_VERSION: u32 = 1;

/// What was meshed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MeshSource {
    /// A project, through [`super::mesh_project`].
    Project,
    /// A raw `.poly`, through [`super::mesh_poly`].
    Poly,
    /// TetGen output made elsewhere, through [`super::mesh_from_tetgen`].
    External,
}

/// How meshing ended. Only `OK` comes with a `tetramesh.mbin`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum MeshStatus {
    #[serde(rename = "OK")]
    Ok,
    #[serde(rename = "FAIL")]
    Fail,
    #[serde(rename = "CANCELLED")]
    Cancelled,
}

/// One TetGen call.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct TetgenCall {
    /// The program run, or the one an external set's trailer names.
    pub program: Option<String>,
    /// The program file's sha256, when it was run here.
    pub program_sha256: Option<String>,
    /// The arguments after the program.
    pub argv: Vec<String>,
    /// The folder it ran in, relative to the mesh folder (`.` or `diag`).
    pub cwd: String,
    /// The raw exit code; `None` when cancelled, or not run here.
    pub exit_code: Option<u32>,
    pub cancelled: bool,
    pub elapsed_ms: f64,
}

/// A facet TetGen skipped, mapped back to the scene.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkippedFacet {
    /// The `.poly` facet marker TetGen wrote in `_skipped.face` (its `shellmark`).
    pub marker: i64,
    /// The scene face (`.cbin` index), when the marker is one.
    pub scene_face: Option<u32>,
    /// The scene face's surface group.
    pub group: Option<String>,
    /// The box fitting zone whose triangle it is, when the marker is past the scene's faces.
    pub fitting_zone: Option<String>,
}

/// The `tetgen -d` follow-up run after skipped facets, in `diag/`.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Diagnosis {
    pub call: TetgenCall,
    /// The markers of `diag/scene_mesh_skipped.face`.
    pub skipped_markers: Vec<i64>,
    /// The intersections TetGen reported, as facet markers.
    pub intersections: Vec<Intersection>,
}

/// Sizes of what went in and came out.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Counts {
    pub scene_vertices: usize,
    pub scene_faces: usize,
    pub poly_vertices: usize,
    pub poly_facets: usize,
    pub regions: usize,
    pub var_constraints: usize,
    /// The builder's figures, when TetGen's output was read.
    pub build: Option<BuildStats>,
}

/// sha256 of the files in the folder, lowercase hex; `None` for a file not written.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Hashes {
    pub poly: Option<String>,
    pub var: Option<String>,
    pub cbin: Option<String>,
    pub mbin: Option<String>,
}

/// The mesh manifest. Written for every meshing attempt, success or not.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MeshManifest {
    pub manifest_version: u32,
    /// The `simpa-core` version that wrote it.
    pub simpa_core: String,
    pub source: MeshSource,
    /// The `.poly` or TetGen folder meshed, for `poly` and `external`.
    pub input_path: Option<String>,
    pub status: MeshStatus,
    /// Reason codes, empty exactly when `status` is `OK`.
    pub codes: Vec<String>,
    /// What went wrong, in words, and remarks on the input.
    pub messages: Vec<String>,
    /// `validate::mesh_input_hash` of the project meshed; `None` for a raw `.poly`.
    pub mesh_input_hash: Option<String>,
    pub tetgen: Option<TetgenCall>,
    pub files: Hashes,
    pub counts: Counts,
    /// The ids the `.mbin` may carry.
    pub volume_ids: VolumeIds,
    /// The box fitting zones' triangles in the `.poly`.
    pub zone_facets: Vec<ZoneFacets>,
    /// Rows of `<base>_skipped.face`: the triangles TetGen skipped.
    pub skipped_rows: usize,
    /// The distinct markers of those rows, ascending, each mapped back to the scene.
    pub skipped_facets: Vec<SkippedFacet>,
    pub diagnosis: Option<Diagnosis>,
    /// `mesh::verify::verify_mesh`'s report on the `.mbin` built, whether it passed or not.
    pub verify: Option<VerifyReport>,
    /// Wall time of the whole meshing call.
    pub elapsed_ms: f64,
}

impl MeshManifest {
    pub fn is_ok(&self) -> bool {
        self.status == MeshStatus::Ok
    }
}

/// sha256 of `bytes`, lowercase hex: the run manifest's hash, so a mesh and a run hash files
/// the same way.
pub use crate::run::manifest::sha256_bytes as sha256_hex;
/// sha256 of the file at `path`, lowercase hex, read in blocks.
pub use crate::run::manifest::sha256_file;

/// Writes `manifest` to `<dir>/mesh.json`: pretty JSON, `\n` line ends, a final newline.
pub fn write_manifest(dir: &Path, manifest: &MeshManifest) -> io::Result<()> {
    let text = serde_json::to_string_pretty(manifest).map_err(io::Error::other)? + "\n";
    std::fs::write(dir.join(MANIFEST_FILE), text)
}

/// Reads `<dir>/mesh.json` with the crate's exact JSON reader ([`crate::schema::parse_json`]:
/// serde_json's own reader is not correctly rounded, so a time or volume would not read back as
/// written). A missing file is [`FormatError::NotFound`]; a file of another layout version is
/// [`FormatError::Version`].
pub fn read_manifest(dir: &Path) -> Result<MeshManifest, FormatError> {
    let path = dir.join(MANIFEST_FILE);
    let bytes = crate::formats::read_file(&path)?;
    let invalid = |e: String| FormatError::Invalid(format!("{}: {e}", path.display()));
    let text = std::str::from_utf8(&bytes).map_err(|e| invalid(e.to_string()))?;
    let value = crate::schema::parse_json(text).map_err(|e| invalid(e.to_string()))?;
    let version = value.get("manifest_version").and_then(|v| v.as_u64());
    if version != Some(u64::from(MANIFEST_VERSION)) {
        return Err(FormatError::Version {
            what: "mesh manifest",
            found: version.map_or_else(|| "none".to_string(), |v| v.to_string()),
            expected: "1",
        });
    }
    serde_json::from_value(value).map_err(|e| invalid(e.to_string()))
}
