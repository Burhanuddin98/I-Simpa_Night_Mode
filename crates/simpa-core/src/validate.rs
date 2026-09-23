//! `core::validate`: the pre-launch rules of `docs/solver-contract.md`, Part A.
//!
//! Every rule has a stable reason code, listed in [`RULES`] in the page's order. **The codes are
//! this module's API**: a code is never renamed or reused. A rule's severity is fixed by the
//! table: an [`Severity::Error`] blocks the run, a [`Severity::Warning`] is reported and the run
//! proceeds. No rule turns a solver failure into a warning.
//!
//! There are two stages, as on the contract page:
//! - [`validate`] and [`validate_with`] check a typed [`Project`] (34 `project` rules), with its
//!   geometry, its directivity files and, when one is given, the stamp of its tetrahedral mesh.
//! - [`validate_export`] checks the exact files the exporter wrote to a run folder
//!   (`config.xml`, the `.cbin` and the `.mbin`) immediately before launch (7 `export` rules).
//!
//! # Structure
//!
//! [`validate`] accepts any [`Project`], including one that fails
//! [`Project::check_integrity`]: such a project can be built in memory, or read from a file with
//! [`read_project`], which checks the version and the schema but not integrity. A structural
//! fault that a contract rule describes is reported under that rule's code (a per-band array of
//! the wrong length is `band_set_mismatch`, a group whose material does not exist is
//! `material_unassigned`, a dangling variant reference is `variant_reference_invalid`). The
//! structural faults no contract rule describes are reported as errors under the stable codes
//! [`schema::IntegrityError::code`] already gives them, listed in [`STRUCTURAL_CODES`]. So every
//! project that fails `check_integrity` gets at least one error, and each fault gets one code.
//!
//! # Scope of the value rules
//!
//! The rules check what reaches a solver: enabled sources, every point receiver, enabled surface
//! receivers and fitting zones, and the materials in use, which are the effective materials of
//! the surface groups under the active variant. A disabled source or an unused library material
//! can hold any value. Structural faults are checked on every entity, used or not, as
//! `check_integrity` does.
//!
//! # Numbers as the solver reads them
//!
//! Every real reaches the solver as a 32-bit `float` (`coreString.h:41`). A rule that requires a
//! value above 0 therefore also requires it above 0 once rounded to `f32` (1e-50 s is not a time
//! step), and finite there (1e39 is not). Step counts and emission steps are computed as the
//! solver computes them, in `f32`, as well as in `f64`, and both must pass.
//!
//! # Geometry
//!
//! Inside and outside are decided by the generalised winding number of the project's triangles
//! (see `geometry.rs` for the method and its limits), and clearances by the exact distance to the
//! nearest triangle.

use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::schema::{
    self, FORMAT_VERSION, FittingShape, FittingZoneId, LoadError, MeshSettings, Project,
    SurfaceReceiverShape, Vec3,
};

mod directivity;
mod export;
mod geometry;
mod names;
mod project;
mod structure;

pub use export::{
    AttrKind, CONFIG_ATTRIBUTES, CONFIG_FILE_NAME, ConfigAttribute, Writer, validate_export,
};
pub use geometry::{PointLocation, locate_point};

/// Whether an issue blocks the run.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    /// Blocks the run; the CLI exits 2.
    Error,
    /// Reported and recorded in the run manifest; the run proceeds.
    Warning,
}

/// Where a rule is checked (`docs/solver-contract.md`, "Stage").
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Stage {
    /// On the typed project, with its geometry and mesh, before export: [`validate_with`].
    Project,
    /// On the files the exporter wrote, before launch: [`validate_export`].
    Export,
}

/// One rule of the contract.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rule {
    pub code: &'static str,
    pub stage: Stage,
    pub severity: Severity,
}

/// One broken rule.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Issue {
    /// A code from [`RULES`] or [`STRUCTURAL_CODES`].
    pub code: &'static str,
    pub severity: Severity,
    /// What is wrong, in words, with the values involved.
    pub message: String,
    /// Where. For a project: a JSON pointer into the project file, such as
    /// `/sources/0/position` or `/materials/2/absorption/3`. For a run folder: the file name, a
    /// colon and a location in it, such as `config.xml:/configuration/simulation@trans_epsilon`
    /// or `mesh.cbin:/faces/7`.
    pub path: String,
}

impl std::fmt::Display for Issue {
    /// `error band_duplicate at /bands/frequencies_hz/5: 2000 Hz appears more than once: ...`
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let severity = match self.severity {
            Severity::Error => "error",
            Severity::Warning => "warning",
        };
        write!(
            f,
            "{severity} {} at {}: {}",
            self.code, self.path, self.message
        )
    }
}

/// The reason codes, one constant per rule of `docs/solver-contract.md` Part A and per
/// structural code.
pub mod codes {
    // Bands.
    pub const BAND_SET_EMPTY: &str = "band_set_empty";
    pub const BAND_DUPLICATE: &str = "band_duplicate";
    pub const BAND_FREQUENCY_NOT_INTEGER: &str = "band_frequency_not_integer";
    pub const BAND_SET_MISMATCH: &str = "band_set_mismatch";
    // Materials.
    pub const MATERIAL_UNASSIGNED: &str = "material_unassigned";
    pub const MATERIAL_VALUE_OUT_OF_RANGE: &str = "material_value_out_of_range";
    pub const MATERIAL_DIFFUSION_IGNORED: &str = "material_diffusion_ignored";
    pub const MATERIAL_TRANSMISSION_EXCEEDS_ABSORPTION: &str =
        "material_transmission_exceeds_absorption";
    // Sources and receivers.
    pub const SOURCE_NONE: &str = "source_none";
    pub const SOURCE_OUTSIDE_VOLUME: &str = "source_outside_volume";
    pub const SOURCE_NEAR_SURFACE: &str = "source_near_surface";
    pub const RECEIVER_OUTSIDE_VOLUME: &str = "receiver_outside_volume";
    pub const RECEIVER_SPHERE_CROSSES_SURFACE: &str = "receiver_sphere_crosses_surface";
    pub const RECEIVER_RADIUS_INVALID: &str = "receiver_radius_invalid";
    pub const DIRECTION_VECTOR_ZERO: &str = "direction_vector_zero";
    // Directivity.
    pub const DIRECTIVITY_FILE_MISSING: &str = "directivity_file_missing";
    pub const DIRECTIVITY_FILE_INVALID: &str = "directivity_file_invalid";
    pub const DIRECTIVITY_BAND_MISSING: &str = "directivity_band_missing";
    // Time.
    pub const TIME_STEP_INVALID: &str = "time_step_invalid";
    pub const STEP_COUNT_OVERFLOW: &str = "step_count_overflow";
    pub const SOURCE_DELAY_INVALID: &str = "source_delay_invalid";
    // SPPS settings.
    pub const TRANS_EPSILON_INVALID: &str = "trans_epsilon_invalid";
    pub const PARTICLE_COUNT_INVALID: &str = "particle_count_invalid";
    // Environment.
    pub const ATMOSPHERE_INVALID: &str = "atmosphere_invalid";
    pub const ABSATMO_INVALID: &str = "absatmo_invalid";
    // Names.
    pub const NAME_TOO_LONG: &str = "name_too_long";
    pub const NAME_NOT_FILENAME_SAFE: &str = "name_not_filename_safe";
    pub const NAME_DUPLICATE: &str = "name_duplicate";
    // Surface receivers, cutting planes and fittings.
    pub const SURFACE_RECEIVER_EMPTY: &str = "surface_receiver_empty";
    pub const CUTTING_PLANE_INVALID: &str = "cutting_plane_invalid";
    pub const FITTING_PARAMETERS_INVALID: &str = "fitting_parameters_invalid";
    // Project integrity.
    pub const VARIANT_REFERENCE_INVALID: &str = "variant_reference_invalid";
    pub const MESH_OUT_OF_DATE: &str = "mesh_out_of_date";
    // Meshing.
    pub const MESH_SETTINGS_CONFLICT: &str = "mesh_settings_conflict";
    // Export checks.
    pub const WORKING_DIRECTORY_INVALID: &str = "working_directory_invalid";
    pub const OUTPUT_PATH_TOO_LONG: &str = "output_path_too_long";
    pub const CONFIG_ATTRIBUTE_MISSING: &str = "config_attribute_missing";
    pub const DOCALC_NOT_LITERAL_ONE: &str = "docalc_not_literal_one";
    pub const CONFIG_VALUE_FORMAT: &str = "config_value_format";
    pub const SOLVER_ID_MAPPING_INVALID: &str = "solver_id_mapping_invalid";
    pub const FITTING_ID_COLLIDES_WITH_ROOM_REGION: &str = "fitting_id_collides_with_room_region";

    // Structural codes: the same strings as `schema::IntegrityError::code`.
    /// `format_version` is not the version this build holds.
    pub const VERSION: &str = "version";
    /// A band frequency that is not a nominal frequency of the band kind, or bands out of order.
    pub const BANDS: &str = "bands";
    /// Two entities of one kind share an id.
    pub const DUPLICATE_ID: &str = "duplicate_id";
    /// A surface receiver or fitting zone names a surface group that does not exist.
    pub const DANGLING_REFERENCE: &str = "dangling_reference";
    /// A face indexes past the vertex list.
    pub const FACE_VERTEX: &str = "face_vertex";
    /// A list of group references names one group twice.
    pub const REPEATED_GROUP: &str = "repeated_group";
    /// A variant's overrides are not strictly ascending by group id.
    pub const OVERRIDE_ORDER: &str = "override_order";
    /// The random seed or a pinned material solver id does not fit a C `int`.
    pub const SOLVER_INT_RANGE: &str = "solver_int_range";
}

const fn rule(code: &'static str, stage: Stage, severity: Severity) -> Rule {
    Rule {
        code,
        stage,
        severity,
    }
}

use Severity::{Error as E, Warning as W};
use Stage::{Export as X, Project as P};

/// Every rule of `docs/solver-contract.md` Part A, in the page's order: 34 project rules and 7
/// export rules, 38 errors and 3 warnings.
pub const RULES: [Rule; 41] = [
    rule(codes::BAND_SET_EMPTY, P, E),
    rule(codes::BAND_DUPLICATE, P, E),
    rule(codes::BAND_FREQUENCY_NOT_INTEGER, P, E),
    rule(codes::BAND_SET_MISMATCH, P, E),
    rule(codes::MATERIAL_UNASSIGNED, P, E),
    rule(codes::MATERIAL_VALUE_OUT_OF_RANGE, P, E),
    rule(codes::MATERIAL_DIFFUSION_IGNORED, P, W),
    rule(codes::MATERIAL_TRANSMISSION_EXCEEDS_ABSORPTION, P, W),
    rule(codes::SOURCE_NONE, P, E),
    rule(codes::SOURCE_OUTSIDE_VOLUME, P, E),
    rule(codes::SOURCE_NEAR_SURFACE, P, E),
    rule(codes::RECEIVER_OUTSIDE_VOLUME, P, E),
    rule(codes::RECEIVER_SPHERE_CROSSES_SURFACE, P, W),
    rule(codes::RECEIVER_RADIUS_INVALID, P, E),
    rule(codes::DIRECTION_VECTOR_ZERO, P, E),
    rule(codes::DIRECTIVITY_FILE_MISSING, P, E),
    rule(codes::DIRECTIVITY_FILE_INVALID, P, E),
    rule(codes::DIRECTIVITY_BAND_MISSING, P, E),
    rule(codes::TIME_STEP_INVALID, P, E),
    rule(codes::STEP_COUNT_OVERFLOW, P, E),
    rule(codes::SOURCE_DELAY_INVALID, P, E),
    rule(codes::TRANS_EPSILON_INVALID, P, E),
    rule(codes::PARTICLE_COUNT_INVALID, P, E),
    rule(codes::ATMOSPHERE_INVALID, P, E),
    rule(codes::ABSATMO_INVALID, P, E),
    rule(codes::NAME_TOO_LONG, P, E),
    rule(codes::NAME_NOT_FILENAME_SAFE, P, E),
    rule(codes::NAME_DUPLICATE, P, E),
    rule(codes::SURFACE_RECEIVER_EMPTY, P, E),
    rule(codes::CUTTING_PLANE_INVALID, P, E),
    rule(codes::FITTING_PARAMETERS_INVALID, P, E),
    rule(codes::VARIANT_REFERENCE_INVALID, P, E),
    rule(codes::MESH_OUT_OF_DATE, P, E),
    rule(codes::MESH_SETTINGS_CONFLICT, P, E),
    rule(codes::WORKING_DIRECTORY_INVALID, X, E),
    rule(codes::OUTPUT_PATH_TOO_LONG, X, E),
    rule(codes::CONFIG_ATTRIBUTE_MISSING, X, E),
    rule(codes::DOCALC_NOT_LITERAL_ONE, X, E),
    rule(codes::CONFIG_VALUE_FORMAT, X, E),
    rule(codes::SOLVER_ID_MAPPING_INVALID, X, E),
    rule(codes::FITTING_ID_COLLIDES_WITH_ROOM_REGION, X, E),
];

/// Codes for structural faults that no contract rule describes. Each is an error, and each is
/// the code [`schema::IntegrityError::code`] gives the same fault. The other integrity faults map
/// to contract codes: see the module docs.
pub const STRUCTURAL_CODES: [&str; 8] = [
    codes::VERSION,
    codes::BANDS,
    codes::DUPLICATE_ID,
    codes::DANGLING_REFERENCE,
    codes::FACE_VERTEX,
    codes::REPEATED_GROUP,
    codes::OVERRIDE_ORDER,
    codes::SOLVER_INT_RANGE,
];

/// Proposed minimum distance between a source and every model face, in metres (open decision in
/// `docs/solver-contract.md`, `source_near_surface`; the solver's own on-face test uses 1e-4 m).
pub const SOURCE_CLEARANCE_M: f64 = 1e-3;

/// A point receiver closer than this to a face, in metres, is on the face and so not strictly
/// inside the room.
pub const ON_SURFACE_M: f64 = 1e-9;

/// Step counts must stay below this: a particle's step counter is a `u16`.
pub const MAX_TIME_STEPS: u32 = 65_536;

/// Longest source name or receiver label, in UTF-8 bytes: a GABE label cell holds 50 bytes with
/// its terminating NUL.
pub const MAX_NAME_BYTES: usize = 49;

/// Every path a solver creates must be shorter than this, in UTF-16 code units (`MAX_PATH`).
pub const MAX_PATH_UTF16: usize = 260;

/// The severity of a code: from [`RULES`], or [`Severity::Error`] for a structural code.
pub fn severity_of(code: &str) -> Severity {
    RULES
        .iter()
        .find(|r| r.code == code)
        .map_or(Severity::Error, |r| r.severity)
}

pub(crate) fn issue(
    code: &'static str,
    path: impl Into<String>,
    message: impl Into<String>,
) -> Issue {
    Issue {
        code,
        severity: severity_of(code),
        message: message.into(),
        path: path.into(),
    }
}

/// Whatever [`validate_with`] needs besides the project.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Context {
    /// The folder of the project file: relative directivity file names resolve against it. With
    /// `None`, a relative name cannot be found and is reported as `directivity_file_missing`.
    pub project_dir: Option<PathBuf>,
    /// The stamp of the tetrahedral mesh that would be launched with this project: the
    /// [`mesh_input_hash`] of the project it was built from. `None` when no mesh exists yet, so
    /// export will build a fresh one and `mesh_out_of_date` does not apply.
    pub mesh_input_hash: Option<String>,
}

impl Context {
    /// The context of a project file at `path`: its folder, and no mesh.
    pub fn for_project_file(path: &Path) -> Self {
        Context {
            project_dir: path.parent().map(Path::to_path_buf),
            mesh_input_hash: None,
        }
    }
}

/// Checks every `project` rule with an empty [`Context`]: no project folder, no mesh.
pub fn validate(project: &Project) -> Vec<Issue> {
    validate_with(project, &Context::default())
}

/// Checks every `project` rule. The order is deterministic: structural faults first, then the
/// rules in the contract page's order, each over its entities in project order.
pub fn validate_with(project: &Project, ctx: &Context) -> Vec<Issue> {
    let mut out = Vec::new();
    structure::check(project, &mut out);
    project::check(project, ctx, &mut out);
    out
}

/// True if any issue is an error.
pub fn has_errors(issues: &[Issue]) -> bool {
    issues.iter().any(|i| i.severity == Severity::Error)
}

/// Reads a project file for validation: UTF-8, the exact JSON reader, the version check and the
/// schema, as [`schema::load`] does, but **not** [`Project::check_integrity`], so that
/// [`validate_with`] can report every structural fault under its contract code.
pub fn read_project(path: &Path) -> Result<Project, LoadError> {
    let bytes = std::fs::read(path).map_err(|e| match e.kind() {
        std::io::ErrorKind::NotFound => LoadError::NotFound(path.to_path_buf()),
        _ => LoadError::Io(e),
    })?;
    let text = std::str::from_utf8(&bytes).map_err(|e| LoadError::Utf8 {
        valid_up_to: e.valid_up_to(),
    })?;
    read_project_text(text)
}

/// [`read_project`] on text already in memory.
pub fn read_project_text(text: &str) -> Result<Project, LoadError> {
    let value = schema::parse_json(text)?;
    let version = match &value {
        serde_json::Value::Object(map) => map.get("format_version"),
        _ => None,
    };
    match version {
        None => return Err(LoadError::MissingVersion),
        Some(serde_json::Value::Number(n)) if n.as_u64() == Some(u64::from(FORMAT_VERSION)) => {}
        Some(v) => {
            return Err(LoadError::Version {
                found: v.to_string(),
                supported: FORMAT_VERSION,
            });
        }
    }
    serde_json::from_value(value).map_err(|e| LoadError::Schema(e.to_string()))
}

/// Everything the tetrahedral mesh is built from, in a fixed layout. Its hash is the mesh stamp.
#[derive(Serialize)]
struct MeshInputs<'a> {
    stamp_version: u32,
    vertices: &'a [Vec3],
    faces: Vec<[u32; 3]>,
    fitting_zones: Vec<(FittingZoneId, &'a FittingShape)>,
    meshing: &'a MeshSettings,
    refined_faces: Vec<usize>,
}

/// The stamp a tetrahedral mesh must carry to be launched with this project: a 128-bit FNV-1a
/// hash, as 32 lowercase hex digits, of every input the mesher uses. Those are the vertices (bit
/// for bit), each face's three vertex indices in order (the `.mbin` face markers index this
/// list), the enabled fitting zones in order with their shapes (they become TetGen regions), the
/// meshing settings, and the faces refined for surface receivers. Materials, names, sources and
/// receivers do not change the mesh and are not hashed.
///
/// FNV-1a detects accidental change, not tampering. The mesher (M5) must record this value when
/// it builds a mesh, and pass it back through [`Context::mesh_input_hash`].
pub fn mesh_input_hash(project: &Project) -> String {
    let refined_faces = if project
        .solvers
        .meshing
        .surface_receiver_max_area_m2
        .is_some()
    {
        let groups: Vec<_> = project
            .surface_receivers
            .iter()
            .filter(|r| r.enabled)
            .flat_map(|r| match &r.shape {
                SurfaceReceiverShape::Scene { groups } => groups.as_slice(),
                SurfaceReceiverShape::CuttingPlane { .. } => &[],
            })
            .collect();
        project
            .geometry
            .faces
            .iter()
            .enumerate()
            .filter(|(_, f)| groups.contains(&&f.group))
            .map(|(i, _)| i)
            .collect()
    } else {
        Vec::new()
    };
    let inputs = MeshInputs {
        stamp_version: 1,
        vertices: &project.geometry.vertices,
        faces: project.geometry.faces.iter().map(|f| f.vertices).collect(),
        fitting_zones: project
            .fitting_zones
            .iter()
            .filter(|z| z.enabled)
            .map(|z| (z.id, &z.shape))
            .collect(),
        meshing: &project.solvers.meshing,
        refined_faces,
    };
    let bytes = serde_json::to_vec(&inputs).expect("mesh inputs serialise: no map keys");
    format!("{:032x}", fnv1a_128(&bytes))
}

fn fnv1a_128(bytes: &[u8]) -> u128 {
    const OFFSET: u128 = 0x6c62_272e_07bb_0142_62b8_2175_6295_c58d;
    const PRIME: u128 = 0x0000_0000_0100_0000_0000_0000_0000_013b;
    bytes
        .iter()
        .fold(OFFSET, |h, &b| (h ^ u128::from(b)).wrapping_mul(PRIME))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rule_table_matches_the_contract_counts() {
        assert_eq!(RULES.len(), 41);
        let project = RULES.iter().filter(|r| r.stage == Stage::Project).count();
        let warnings = RULES
            .iter()
            .filter(|r| r.severity == Severity::Warning)
            .count();
        assert_eq!((project, 41 - project), (34, 7));
        assert_eq!((41 - warnings, warnings), (38, 3));
        let mut all: Vec<&str> = RULES.iter().map(|r| r.code).collect();
        all.extend(STRUCTURAL_CODES);
        let n = all.len();
        all.sort_unstable();
        all.dedup();
        assert_eq!(all.len(), n, "codes are unique");
        assert!(all.iter().all(|c| {
            c.bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
        }));
    }

    #[test]
    fn fnv1a_128_known_values() {
        // "" is the offset basis; "a" was computed independently with Python's big integers:
        // ((basis ^ 0x61) * prime) mod 2^128.
        assert_eq!(fnv1a_128(b""), 0x6c62_272e_07bb_0142_62b8_2175_6295_c58d);
        assert_eq!(fnv1a_128(b"a"), 0xd228_cb69_6f1a_8caf_7891_2b70_4e4a_8964);
    }
}
