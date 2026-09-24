//! Geometry import: PLY, OBJ and STL room models, and upstream I-Simpa `.proj` projects.
//!
//! # Mesh files
//!
//! [`read_ply`], [`read_obj`] and [`read_stl`] (or [`import_file`], which picks one by extension)
//! turn a mesh file into an [`ImportedModel`]: vertices in world metres with Z up, triangles, and
//! one surface group per group the file declares. [`ImportedModel::to_project`] makes a project of
//! it. What every reader does, whatever the format:
//!
//! - **Units and up axis are explicit** ([`ImportOptions`]); nothing is guessed from the extents.
//!   The up-axis map is a proper rotation: [`Up::Z`] keeps `(x, y, z)`, [`Up::Y`] maps a file's
//!   `(x, y, z)` to `(x, -z, y)`, so the file's +Y becomes world +Z. Units scale after that:
//!   centimetres and millimetres divide by 100 and 1000 (one correctly rounded operation), feet
//!   and inches multiply by 0.3048 and 0.0254.
//! - **Welding is exact and consistent, and only where the format needs it** ([`Weld`]). An STL
//!   facet carries its own three corners, so an STL is welded: two vertices are merged exactly
//!   when their coordinates, as read from the file, are equal (`-0.0` equals `0.0`), and the hash
//!   is taken over the same canonical bits the equality compares, so equal vertices always meet.
//!   PLY and OBJ faces index shared vertices already, so their vertices are kept as the file has
//!   them: merging two coincident vertices changes the model's connectivity, which is
//!   `geometry::repair`'s to do and log (upstream's raw `elmia.ply` has 7 such pairs; its edge
//!   census is 955 open edges as the file stands, 939 welded). [`Weld::Exact`] welds them anyway.
//!   Vertices no face uses are dropped. Every count is in the [`ImportReport`]. Faces are never
//!   reordered, merged or dropped: degenerate or duplicate faces are `geometry::check`'s to
//!   report and `geometry::repair`'s to fix.
//! - **Float32 values are widened** with [`widen_f32`]: a PLY
//!   `float` property (ASCII or binary) and a binary STL coordinate become the `f64` of their
//!   shortest decimal, so an ASCII file and its binary twin import identically. Text read as
//!   `double` (OBJ, ASCII STL, PLY `double`) is parsed as `f64` directly.
//! - **Polygons are fan-triangulated** from their first vertex, as upstream's PLY reader does; the
//!   count is reported, since a fan is only right for a convex polygon.
//! - **Nothing is dropped silently.** An index out of range, a non-finite coordinate, a face with
//!   fewer than 3 vertices, a truncated file: each is an [`ImportError`] with a reason code.
//! - **No panic, no allocation beyond what the input justifies,** on any input
//!   (`tests/geometry_import_fuzz.rs`).
//!
//! # Upstream projects
//!
//! [`import_proj`] reads an upstream `.proj` archive into a complete [`Project`]; see [`proj`].
//!
//! # Re-import
//!
//! [`reassign`] puts a new model into an existing project, keeping each face's surface group
//! (and so its material) where the new face's centre lies within a tolerance of an old face.

use std::collections::HashMap;
use std::fmt;
use std::path::{Path, PathBuf};

use uuid::{Builder, Uuid};

use crate::config_xml::widen_f32;
use crate::schema::{
    Face, GroupId, IntegrityError, Material, MaterialId, Project, ProjectId, ReflectionLaw, Rgb,
    SurfaceGroup, Vec3,
};

mod appconst;
mod obj;
mod ply;
pub mod proj;
mod reassign;
mod stl;
pub mod zip;

pub use appconst::{REFERENCE_MATERIALS, REFERENCE_SPECTRA, ReferenceMaterial, ReferenceSpectrum};
pub use proj::{
    ProjImport, ProjReport, UpstreamId, UpstreamKind, import_proj, import_proj_file,
    import_proj_with_config,
};
pub use reassign::{DEFAULT_REASSIGN_TOLERANCE_M, Reassigned, reassign};

/// A length unit a mesh file may be drawn in.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Unit {
    Metre,
    Centimetre,
    Millimetre,
    Foot,
    Inch,
}

impl Unit {
    pub const ALL: [Unit; 5] = [
        Unit::Metre,
        Unit::Centimetre,
        Unit::Millimetre,
        Unit::Foot,
        Unit::Inch,
    ];

    /// The unit's symbol: `m`, `cm`, `mm`, `ft`, `in`.
    pub fn symbol(self) -> &'static str {
        match self {
            Unit::Metre => "m",
            Unit::Centimetre => "cm",
            Unit::Millimetre => "mm",
            Unit::Foot => "ft",
            Unit::Inch => "in",
        }
    }

    /// The unit named by its symbol ([`Unit::symbol`]).
    pub fn from_symbol(symbol: &str) -> Option<Unit> {
        Unit::ALL.into_iter().find(|u| u.symbol() == symbol)
    }

    /// A length in this unit, in metres.
    pub fn to_metres(self, value: f64) -> f64 {
        match self {
            Unit::Metre => value,
            Unit::Centimetre => value / 100.0,
            Unit::Millimetre => value / 1000.0,
            Unit::Foot => value * 0.3048,
            Unit::Inch => value * 0.0254,
        }
    }
}

/// The file's vertical axis. The world's is +Z.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Up {
    /// The file is Y-up: `(x, y, z)` becomes `(x, -z, y)`.
    Y,
    /// The file is Z-up, like the world: coordinates are kept.
    Z,
}

impl Up {
    /// `y` or `z`.
    pub fn from_name(name: &str) -> Option<Up> {
        match name {
            "y" | "Y" => Some(Up::Y),
            "z" | "Z" => Some(Up::Z),
            _ => None,
        }
    }

    /// A file point in world axes.
    pub fn to_world(self, [x, y, z]: [f64; 3]) -> [f64; 3] {
        match self {
            Up::Z => [x, y, z],
            Up::Y => [x, -z, y],
        }
    }
}

/// Which OBJ statement names a face's surface group.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ObjGroups {
    /// `usemtl` if the file has any `usemtl` statement, otherwise `g`/`o`. The choice made is
    /// written in the [`ImportReport`].
    #[default]
    Auto,
    /// The current `usemtl` material name (`default` before the first one).
    Material,
    /// The current `g` or `o` name, whichever came last (`default` before the first one).
    Group,
}

/// Whether an import merges vertices with equal coordinates (see the module docs).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Weld {
    /// STL is welded; PLY and OBJ keep the file's vertices.
    #[default]
    ByFormat,
    /// Every two vertices with equal coordinates are merged, whatever the format.
    Exact,
    /// Every vertex a face uses is kept, whatever the format (an STL keeps 3 per facet).
    Off,
}

/// How to read a mesh file. There is no `Default`: the unit and the up axis must be given.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ImportOptions {
    pub unit: Unit,
    pub up: Up,
    /// OBJ only.
    pub obj_groups: ObjGroups,
    pub weld: Weld,
}

impl ImportOptions {
    /// `unit` and `up`, with OBJ groups [`ObjGroups::Auto`] and welding [`Weld::ByFormat`].
    pub fn new(unit: Unit, up: Up) -> Self {
        ImportOptions {
            unit,
            up,
            obj_groups: ObjGroups::Auto,
            weld: Weld::ByFormat,
        }
    }

    /// Whether a file of this format is welded under these options.
    pub fn welds(&self, format: MeshFormat) -> bool {
        match self.weld {
            Weld::ByFormat => format == MeshFormat::Stl,
            Weld::Exact => true,
            Weld::Off => false,
        }
    }

    /// A file point, in world metres with Z up.
    pub fn to_world(&self, p: [f64; 3]) -> [f64; 3] {
        self.up.to_world(p).map(|c| self.unit.to_metres(c) + 0.0)
    }
}

/// The file formats [`import_file`] reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MeshFormat {
    Ply,
    Obj,
    Stl,
}

impl MeshFormat {
    /// The format a file extension names (any case), if one does.
    pub fn from_path(path: &Path) -> Option<MeshFormat> {
        let ext = path.extension()?.to_str()?.to_ascii_lowercase();
        match ext.as_str() {
            "ply" => Some(MeshFormat::Ply),
            "obj" => Some(MeshFormat::Obj),
            "stl" => Some(MeshFormat::Stl),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            MeshFormat::Ply => "ply",
            MeshFormat::Obj => "obj",
            MeshFormat::Stl => "stl",
        }
    }
}

/// Why a file could not be imported. [`ImportError::code`] is a stable reason code.
#[derive(Debug)]
pub enum ImportError {
    /// The file could not be read.
    Io { path: PathBuf, message: String },
    /// The file's extension names no format this module reads.
    UnknownFormat { path: PathBuf },
    /// The file is not well-formed: a bad header line, a token that is not a number.
    Syntax {
        format: &'static str,
        location: String,
        message: String,
    },
    /// The data ends before something the header or a count promised.
    Truncated { format: &'static str, what: String },
    /// Well-formed but wrong: an index out of range, a non-finite coordinate, a bad count.
    Invalid {
        format: &'static str,
        message: String,
    },
    /// Something this importer does not read, and why.
    Unsupported {
        format: &'static str,
        message: String,
    },
    /// A project refused for a reason with its own stable code (`reason`, one of
    /// [`proj::codes`]), which [`ImportError::code`] returns and `docs/solver-contract.md` lists.
    Refused {
        format: &'static str,
        reason: &'static str,
        message: String,
    },
    /// The file holds no face.
    Empty { format: &'static str },
    /// The imported project fails its integrity check.
    Integrity(IntegrityError),
}

impl ImportError {
    pub fn code(&self) -> &'static str {
        match self {
            ImportError::Io { .. } => "io",
            ImportError::UnknownFormat { .. } => "unknown_format",
            ImportError::Syntax { .. } => "syntax",
            ImportError::Truncated { .. } => "truncated",
            ImportError::Invalid { .. } => "invalid",
            ImportError::Unsupported { .. } => "unsupported",
            ImportError::Refused { reason, .. } => reason,
            ImportError::Empty { .. } => "empty",
            ImportError::Integrity(_) => "integrity",
        }
    }

    pub(crate) fn invalid(format: &'static str, message: impl Into<String>) -> Self {
        ImportError::Invalid {
            format,
            message: message.into(),
        }
    }

    pub(crate) fn unsupported(format: &'static str, message: impl Into<String>) -> Self {
        ImportError::Unsupported {
            format,
            message: message.into(),
        }
    }

    pub(crate) fn refused(
        format: &'static str,
        reason: &'static str,
        message: impl Into<String>,
    ) -> Self {
        ImportError::Refused {
            format,
            reason,
            message: message.into(),
        }
    }

    pub(crate) fn truncated(format: &'static str, what: impl Into<String>) -> Self {
        ImportError::Truncated {
            format,
            what: what.into(),
        }
    }

    pub(crate) fn syntax(
        format: &'static str,
        location: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        ImportError::Syntax {
            format,
            location: location.into(),
            message: message.into(),
        }
    }
}

impl fmt::Display for ImportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ImportError::Io { path, message } => {
                write!(f, "cannot read {}: {message}", path.display())
            }
            ImportError::UnknownFormat { path } => write!(
                f,
                "{}: not a .ply, .obj, .stl or .proj file",
                path.display()
            ),
            ImportError::Syntax {
                format,
                location,
                message,
            } => write!(f, "{format}: syntax error at {location}: {message}"),
            ImportError::Truncated { format, what } => {
                write!(f, "{format}: the data ends before {what}")
            }
            ImportError::Invalid { format, message } => write!(f, "{format}: {message}"),
            ImportError::Unsupported { format, message } => {
                write!(f, "{format}: not supported: {message}")
            }
            ImportError::Refused {
                format, message, ..
            } => write!(f, "{format}: {message}"),
            ImportError::Empty { format } => write!(f, "{format}: the file holds no face"),
            ImportError::Integrity(e) => write!(f, "imported project is inconsistent: {e}"),
        }
    }
}

impl std::error::Error for ImportError {}

pub type Result<T> = std::result::Result<T, ImportError>;

/// What an import did to the file's data, for the user to see.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ImportReport {
    /// Vertices in the file.
    pub source_vertices: usize,
    /// Faces (polygons, facets) in the file.
    pub source_faces: usize,
    /// Triangles imported.
    pub triangles: usize,
    /// Faces with more than 3 vertices, fan-triangulated.
    pub polygons_split: usize,
    /// Whether vertices with equal coordinates were merged ([`Weld`]).
    pub welded: bool,
    /// Vertices merged into an earlier vertex with equal coordinates.
    pub welded_vertices: usize,
    /// Vertices no face uses, dropped.
    pub unused_vertices: usize,
    /// Groups the file declares that hold no face, dropped.
    pub empty_groups: Vec<String>,
    /// Anything else worth knowing, one sentence each.
    pub notes: Vec<String>,
}

/// A room model read from a mesh file.
#[derive(Clone, Debug, PartialEq)]
pub struct ImportedModel {
    pub format: MeshFormat,
    /// World metres, Z up.
    pub vertices: Vec<Vec3>,
    pub triangles: Vec<[u32; 3]>,
    /// Per triangle, its index into `group_names`.
    pub face_groups: Vec<u32>,
    /// One per surface group, in the file's order; every group holds at least one triangle.
    pub group_names: Vec<String>,
    pub report: ImportReport,
}

/// Reads a PLY file: ASCII, binary little- or big-endian, with upstream's `layer_id` and
/// `layer_name` groups; see the module docs.
pub fn read_ply(bytes: &[u8], options: &ImportOptions) -> Result<ImportedModel> {
    ply::read(bytes, options)
}

/// Reads a Wavefront OBJ file, grouped by `usemtl` or `g` ([`ObjGroups`]); see the module docs.
pub fn read_obj(bytes: &[u8], options: &ImportOptions) -> Result<ImportedModel> {
    obj::read(bytes, options)
}

/// Reads an STL file, ASCII or binary (told apart by the binary size rule, not by a leading
/// `solid`); see the module docs.
pub fn read_stl(bytes: &[u8], options: &ImportOptions) -> Result<ImportedModel> {
    stl::read(bytes, options)
}

/// Reads a mesh file, choosing the reader by extension (`.ply`, `.obj`, `.stl`, any case).
/// A `.proj` is refused here: use [`import_proj_file`], which returns a whole project.
pub fn import_file(path: &Path, options: &ImportOptions) -> Result<ImportedModel> {
    let format = MeshFormat::from_path(path).ok_or_else(|| ImportError::UnknownFormat {
        path: path.to_path_buf(),
    })?;
    let bytes = read_bytes(path)?;
    match format {
        MeshFormat::Ply => read_ply(&bytes, options),
        MeshFormat::Obj => read_obj(&bytes, options),
        MeshFormat::Stl => read_stl(&bytes, options),
    }
}

pub(crate) fn read_bytes(path: &Path) -> Result<Vec<u8>> {
    std::fs::read(path).map_err(|e| ImportError::Io {
        path: path.to_path_buf(),
        message: e.to_string(),
    })
}

impl ImportedModel {
    /// A project holding this model: default bands and settings, one surface group per group of
    /// the model, each carrying one material named `Default` with upstream's default material's
    /// values (reference material 0: absorption 0, scattering 0, specular, double-sided, no
    /// transmission), for the user to replace. Ids are derived from the model, so the same model
    /// always gives the same project.
    pub fn to_project(&self, name: &str) -> Project {
        let ids = IdSource::from_parts(&[b"mesh import", &self.digest_bytes()]);
        let mut project = Project::new(name);
        project.id = ProjectId(ids.uuid("project", 0));
        project.description = format!(
            "Imported from a {} file: {} triangles, {} surface groups.",
            self.format.name().to_ascii_uppercase(),
            self.triangles.len(),
            self.group_names.len()
        );
        let material = default_material(MaterialId(ids.uuid("material", 0)), project.bands.len());
        let groups: Vec<SurfaceGroup> = self
            .group_names
            .iter()
            .enumerate()
            .map(|(i, name)| SurfaceGroup {
                id: GroupId(ids.uuid("surface group", i)),
                name: name.clone(),
                material: material.id,
            })
            .collect();
        project.geometry.vertices = self.vertices.clone();
        project.geometry.faces = self
            .triangles
            .iter()
            .zip(&self.face_groups)
            .map(|(t, &g)| Face {
                vertices: *t,
                group: groups[g as usize].id,
            })
            .collect();
        project.surface_groups = groups;
        project.materials.push(material);
        project
    }

    fn digest_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.vertices.len() * 24 + self.triangles.len() * 16);
        for v in &self.vertices {
            for c in v.to_array() {
                out.extend_from_slice(&c.to_bits().to_le_bytes());
            }
        }
        for (t, g) in self.triangles.iter().zip(&self.face_groups) {
            for i in t {
                out.extend_from_slice(&i.to_le_bytes());
            }
            out.extend_from_slice(&g.to_le_bytes());
        }
        for name in &self.group_names {
            out.extend_from_slice(name.as_bytes());
            out.push(0);
        }
        out
    }
}

/// Upstream's default material (reference material 0) on `n_bands` bands.
pub(crate) fn default_material(id: MaterialId, n_bands: usize) -> Material {
    let r = &REFERENCE_MATERIALS[0];
    debug_assert_eq!(r.id, 0);
    Material {
        id,
        name: r.name.to_string(),
        color: Rgb(r.color[0], r.color[1], r.color[2]),
        absorption: vec![crate::schema::F64::new(widen_f32(r.absorption)); n_bands],
        scattering: vec![crate::schema::F64::ZERO; n_bands],
        reflection_law: ReflectionLaw::Specular.into(),
        transmission_loss_db: None,
        double_sided: true,
        solver_id: None,
    }
}

// ---------------------------------------------------------------------------------------------
// Assembly shared by the mesh readers.

/// A mesh as a reader collects it: raw vertices (file axes and units), polygons with their group
/// index, and group names.
pub(crate) struct RawMesh {
    pub vertices: Vec<[f64; 3]>,
    /// Vertex indices of each polygon (at least 3), already range-checked.
    pub polygons: Vec<(Vec<u32>, u32)>,
    pub groups: Vec<String>,
    pub notes: Vec<String>,
}

impl RawMesh {
    /// Welds, drops unused vertices, drops empty groups, triangulates and transforms to world.
    pub fn finish(
        self,
        format: MeshFormat,
        options: &ImportOptions,
        source_faces: usize,
    ) -> Result<ImportedModel> {
        let fmt = format.name();
        if self.polygons.is_empty() {
            return Err(ImportError::Empty { format: fmt });
        }
        let n = self.vertices.len();
        for v in &self.vertices {
            if !v.iter().all(|c| c.is_finite()) {
                return Err(ImportError::invalid(
                    fmt,
                    format!("a vertex has a non-finite coordinate: {v:?}"),
                ));
            }
        }
        // Which vertices are used.
        let mut used = vec![false; n];
        for (poly, _) in &self.polygons {
            for &i in poly {
                used[i as usize] = true;
            }
        }
        // Keep the used vertices, in file order, welding equal ones if asked.
        let weld = options.welds(format);
        let mut remap = vec![u32::MAX; n];
        let mut seen: HashMap<[u64; 3], u32> = HashMap::new();
        let mut vertices: Vec<[f64; 3]> = Vec::new();
        let mut welded = 0usize;
        let mut unused = 0usize;
        for (i, v) in self.vertices.iter().enumerate() {
            if !used[i] {
                unused += 1;
                continue;
            }
            let next = u32::try_from(vertices.len())
                .map_err(|_| ImportError::unsupported(fmt, "more than 4,294,967,295 vertices"))?;
            let idx = if weld {
                *seen.entry(weld_key(*v)).or_insert(next)
            } else {
                next
            };
            if idx == next {
                vertices.push(*v);
            } else {
                welded += 1;
            }
            remap[i] = idx;
        }
        // Keep non-empty groups, in order.
        let mut group_used = vec![false; self.groups.len()];
        for (_, g) in &self.polygons {
            group_used[*g as usize] = true;
        }
        let mut group_remap = vec![u32::MAX; self.groups.len()];
        let mut group_names = Vec::new();
        let mut empty_groups = Vec::new();
        for (i, name) in self.groups.into_iter().enumerate() {
            if group_used[i] {
                group_remap[i] = group_names.len() as u32;
                group_names.push(name);
            } else {
                empty_groups.push(name);
            }
        }
        // Triangulate.
        let mut triangles = Vec::new();
        let mut face_groups = Vec::new();
        let mut polygons_split = 0usize;
        for (poly, g) in &self.polygons {
            if poly.len() > 3 {
                polygons_split += 1;
            }
            let g = group_remap[*g as usize];
            for k in 1..poly.len() - 1 {
                triangles.push([
                    remap[poly[0] as usize],
                    remap[poly[k] as usize],
                    remap[poly[k + 1] as usize],
                ]);
                face_groups.push(g);
            }
        }
        if u32::try_from(triangles.len()).is_err() {
            return Err(ImportError::unsupported(
                fmt,
                "more than 4,294,967,295 triangles",
            ));
        }
        let report = ImportReport {
            source_vertices: n,
            source_faces,
            triangles: triangles.len(),
            polygons_split,
            welded: weld,
            welded_vertices: welded,
            unused_vertices: unused,
            empty_groups,
            notes: self.notes,
        };
        Ok(ImportedModel {
            format,
            vertices: vertices
                .into_iter()
                .map(|v| Vec3::from(options.to_world(v)))
                .collect(),
            triangles,
            face_groups,
            group_names,
            report,
        })
    }
}

/// The welding key: the coordinates' bits with `-0.0` made `0.0`, so that the hash and the
/// equality agree with `f64` equality for every finite value.
pub(crate) fn weld_key(v: [f64; 3]) -> [u64; 3] {
    v.map(|c| (c + 0.0).to_bits())
}

// ---------------------------------------------------------------------------------------------
// Deterministic ids.

/// Deterministic UUIDs: a 128-bit FNV-1a digest of the input, then of each entity's kind and
/// position, shaped as version-4 UUIDs (the scheme `config_xml`'s importer uses).
pub(crate) struct IdSource {
    digest: [u64; 2],
}

const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;
const FNV_OFFSETS: [u64; 2] = [0xcbf2_9ce4_8422_2325, 0x6c62_272e_07bb_0142];

fn fnv(state: &mut [u64; 2], bytes: &[u8]) {
    for s in state.iter_mut() {
        for &b in bytes {
            *s ^= u64::from(b);
            *s = s.wrapping_mul(FNV_PRIME);
        }
    }
}

impl IdSource {
    pub fn from_parts(parts: &[&[u8]]) -> Self {
        let mut d = FNV_OFFSETS;
        for p in parts {
            fnv(&mut d, &(p.len() as u64).to_le_bytes());
            fnv(&mut d, p);
        }
        IdSource { digest: d }
    }

    pub fn uuid(&self, kind: &str, index: usize) -> Uuid {
        let mut s = self.digest;
        fnv(&mut s, kind.as_bytes());
        fnv(&mut s, &(index as u64).to_le_bytes());
        let mut bytes = [0u8; 16];
        bytes[..8].copy_from_slice(&s[0].to_le_bytes());
        bytes[8..].copy_from_slice(&s[1].to_le_bytes());
        Builder::from_random_bytes(bytes).into_uuid()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn units_convert_to_metres() {
        assert_eq!(Unit::Millimetre.to_metres(1234.0), 1.234);
        assert_eq!(Unit::Centimetre.to_metres(250.0), 2.5);
        assert_eq!(Unit::Foot.to_metres(10.0), 3.048);
        assert_eq!(Unit::Inch.to_metres(100.0), 2.54);
        for u in Unit::ALL {
            assert_eq!(Unit::from_symbol(u.symbol()), Some(u));
        }
    }

    #[test]
    fn y_up_is_a_proper_rotation_taking_y_to_z() {
        let o = ImportOptions::new(Unit::Metre, Up::Y);
        assert_eq!(o.to_world([0.0, 1.0, 0.0]), [0.0, 0.0, 1.0]);
        assert_eq!(o.to_world([1.0, 0.0, 0.0]), [1.0, 0.0, 0.0]);
        assert_eq!(o.to_world([0.0, 0.0, 1.0]), [0.0, -1.0, 0.0]);
        // det = +1: x cross y = z holds after the map.
        let (x, y) = (o.to_world([1.0, 0.0, 0.0]), o.to_world([0.0, 1.0, 0.0]));
        let z = [
            x[1] * y[2] - x[2] * y[1],
            x[2] * y[0] - x[0] * y[2],
            x[0] * y[1] - x[1] * y[0],
        ];
        assert_eq!(z, o.to_world([0.0, 0.0, 1.0]));
    }

    #[test]
    fn weld_key_equates_signed_zeros_only() {
        assert_eq!(weld_key([0.0, -0.0, 1.0]), weld_key([-0.0, 0.0, 1.0]));
        assert_ne!(
            weld_key([0.0, 0.0, 1.0]),
            weld_key([0.0, 0.0, 1.0 + f64::EPSILON])
        );
    }
}
