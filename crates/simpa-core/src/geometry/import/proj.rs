//! Upstream I-Simpa projects (`.proj`), read-only, into a complete [`Project`].
//!
//! A `.proj` is a zip archive (read by [`super::zip`]) holding one project folder, `instanceN/`,
//! with the GUI's element tree `projet_config.xml`, the scene mesh `sceneMesh.bin` and one `.finfo`
//! face list per surface group and per scene surface receiver. Everything else in it (past runs
//! under `report/`, TetGen's files under `temp/`) is ignored.
//!
//! # Geometry
//!
//! - **`sceneMesh.bin`** ([`read_scene_mesh`], upstream `3dengine/Core/bin.cpp`): a `u32` major
//!   and minor version, then a chain of nodes (`u16` type, 2 bytes of padding, `u32` first son,
//!   `u32` next brother). Type 0 holds the vertices (`u32` count, then `f32` x, y, z in world
//!   metres, Z up); type 1 a mesh group (a 255-byte name, 1 byte of padding, `u32` face count,
//!   then the face records); types 2 to 4 (materials, textures, texture coordinates) are skipped.
//!   The face record size is **taken from the node's span** (its next brother, or the end of the
//!   file for the last node) divided by the face count, and must be the size the version's
//!   `binaryFace` struct has: 32 bytes in versions 1.1 and 1.2, 20 in 1.0. Night Mode read 24 and
//!   misread every face after the first. Only the first three `u32` of a record (the vertex
//!   indices) are read.
//! - **Vertices** are widened from `f32` ([`widen_f32`]) and welded exactly, as an STL import is
//!   (`geometry::import`): upstream's GUI may keep a separate copy of each vertex per face
//!   (tutorial 1's box has 36 vertices for 12 faces), so without welding no edge would be shared.
//!   Faces keep their order: project face `i` is scene-mesh face `i`, counting the mesh groups in
//!   file order.
//! - **`.finfo`** ([`read_finfo`], `data_manager/grpInfo/data_group_info.cpp`): a face count, then
//!   `(i32 mesh group, i32 face in that group)` pairs. The count is a C `unsigned long`: 4 bytes in
//!   a file written on Windows, 8 on Linux and macOS. The two layouts are told apart by the file's
//!   length (`4 + 8n` against `8 + 8n`), so both are read.
//!
//! # The element tree
//!
//! `projet_config.xml` is read in upstream's load order, not the file's: every element's children
//! are first re-ordered by `wxid` as upstream's element constructor re-orders them, quirks
//! included ([`kids`], [`upstream_sort`]; `data_manager/element.cpp:64-106, 158-159`). The two
//! differ in a file saved in the session that created its elements, such as a run's snapshot
//! (tutorial 1's SPPS run lists `Receiver 2` before `Receiver 1`); every list read below (surface
//! groups, receivers, fitting zones, sources and source groups, materials and their rows) is
//! read in load order.
//!
//! # Surface groups
//!
//! Each face gets the material group (`Scene/donnees/sgroupes/gr`) whose `.finfo` lists it, the
//! scene surface receiver (`recepteurss/recepteurs/gr`) whose `.finfo` lists it, if any, and the
//! scene-fitted zones (`encombrements/encombrement`, element type 54) whose `.finfo` lists it, if
//! any. Each distinct (material group, receiver, zones) key becomes one [`SurfaceGroup`], ordered
//! by material group, then receiver, then zones, carrying the material group's name, with
//! ` / <receiver>` appended only when receivers split the material group and ` / <zone>` (zones
//! joined by ` + `) only when zones do. A scene receiver covers the groups whose faces it lists,
//! and a scene-fitted zone is the [`FittingShape::Surfaces`] of the groups whose faces it lists.
//! A face that no material group lists gets upstream's default material (reference material 0,
//! `data_manager/appconfig.h:122`) in a group named `(no surface group)`, noted in the report. A
//! face listed by two material groups, two receivers or two *enabled* zones is refused: upstream
//! would let the last one win, silently. Upstream tags faces for enabled zones only
//! (`appconfig.cpp:185`), so a face may be listed by any number of disabled zones besides.
//!
//! # Fitting zones
//!
//! Read as upstream's GUI loads them (`tree_scene/e_scene_encombrements.h:56-73`), in load order,
//! told apart by their element type `eid`: 54 is a scene-fitted zone
//! (`e_scene_encombrements_encombrement_model.h`), 56 a rectangular one
//! (`e_scene_encombrements_encombrement_cuboide.h`). Upstream skips any other child silently; this
//! import refuses it ([`codes::FITTING_TYPE_UNKNOWN`]). Each keeps its name and
//! `useforcalculation` (as `enabled`: a disabled zone is left out of `config.xml`, the `.cbin` and
//! the `.poly`, `..._cuboide.h:311, 331, 352`, `..._model.h:148, 175`), and per band `alpha`,
//! `lambda` and `loi_diff` from its `absorption` rows (see [`read_zone_bands`] for how upstream's
//! loader resets the diffusion law).
//! - **A scene-fitted zone** is the faces its own `gr/@facesFile` lists, and its region seed is
//!   its `volpos`, as upstream seeds it (`Objet3D_maillage.cpp:1002-1036`). Upstream derives a
//!   seed from the zone's first face when `volpos` is (0, 0, 0); that is not reproduced
//!   ([`codes::FITTING_INSIDE_POINT_UNSET`]). An enabled zone without its `gr` is written to
//!   `config.xml` by upstream with no region at all ([`codes::FITTING_FACE_GROUP_MISSING`]).
//! - **A rectangular zone** is a [`FittingShape::Box`] over its corners `ba` and `hc`, which
//!   upstream keeps as typed, not ordered (tutorial 3: `ba` (13, 4, 0), `hc` (18, 1, 1.2)):
//!   `min` and `max` are their bounds, and `destination` records which bound `hc` takes, which
//!   upstream's region seed `hc - (hc - ba) * 1e-4` needs (`..._cuboide.h:333-336`). An enabled
//!   box with no volume is refused ([`codes::FITTING_BOX_EMPTY`]).
//!
//! These three refusals, and two zones listing one face, concern enabled zones only: upstream
//! seeds, tags and draws nothing for a disabled zone, so it is kept as stored, with a note saying
//! what enabling it would need.
//!
//! Volumes (`volumes/volume`, element type 86) are refused ([`codes::VOLUMES_UNSUPPORTED`]).
//!
//! # Element ids
//!
//! Upstream writes its GUI's element ids into `config.xml` (`encombrement@id` 1930,
//! `recepteur_ponctuel@id` 155, ...). The file holds them as each element's `wxid`: the ids of the
//! session that last saved it. Upstream does not reuse them: it gives every element a new id from
//! a global counter each time it loads a project (`Element::Element` → `SetXmlId`,
//! `element.cpp:134, 231-244`, overwriting the node's `wxid`, `:142-144`; the counter restarts at
//! project close, `projet.cpp:625`; `docs/formats/cbin.md`, "What still differs"). So a run
//! carries the ids of the session that wrote it, which are the `.proj`'s only when that session
//! also saved the file: tutorial 3's runs carry its `.proj`'s ids, tutorial 1's carry 3503, 3510
//! and 3669 where its `.proj` holds 1792, 1473 and 1632. Imported projects keep the file's ids
//! (Burhan, 2026-09-24 14:11; `docs/m5-m6-design.md`, decision 13): every source, point receiver,
//! surface receiver or cutting plane and fitting zone is pinned to its `wxid` (`solver_id`), so
//! export writes the ids the `.proj` holds (`config_xml`, "Solver ids"), not the ones upstream
//! would give after reopening it, and TetGen numbers the room's parts above those fitting ids,
//! as upstream's own meshes carry them (tutorial 3: fittings 1930 and 2083, the room 2084 to
//! 2086). The ids reach labels and matching only: the solvers match them between `config.xml`,
//! the `.cbin` and the `.mbin`, and write them into `.csbin` and TCR output names and labels. A
//! `wxid` outside 0 to `SOLVER_INT_MAX` is invalid. [`ProjReport::upstream_ids`] still records
//! each `wxid` beside the entity it became. Each source also keeps the source group it sat in
//! (`Source::group`).
//!
//! # Everything else
//!
//! Read the way upstream's GUI writes it into `config.xml`, values being the `f32` the GUI holds,
//! widened with [`widen_f32`]:
//!
//! - **Materials:** every material of the project's own database (`bdd/materiaux`) with its name,
//!   colour, side and per-band absorption, scattering, reflection law ([`ReflectionLaws`]) and
//!   transmission loss (per band, only where that band's `transmission` is on: upstream writes
//!   `affaiblissement` only there, `e_data_row_materiau.h:98-106`), pinned to its `idmateriau`;
//!   then each reference material a group uses, from upstream's reference database
//!   (`REFERENCE_MATERIALS`). A group whose `idmat` is in neither is refused. A law outside
//!   upstream's seven (`appconfig.cpp:110-117`) is refused
//!   ([`codes::REFLECTION_LAW_OUT_OF_RANGE`]), as is a pre-1.3.4 band row with a value upstream
//!   reports or leaves undefined ([`codes::MATERIAL_ROW_UNREADABLE`]) and a loss the writer would
//!   not write as stored ([`codes::TRANSMISSION_EXCEEDS_ABSORPTION`]).
//! - **Spectra** (source power, receiver background noise): upstream writes each band as the
//!   chosen spectrum's band level plus the user's global level `Lw` (normalised so the reference
//!   sums to `Lw`), minus the band's attenuation (`E_Property_Freq::LoadLwFromBdd`); the band
//!   levels stored in the file are not used, since the GUI recomputes them at every write
//!   (tutorial 1's receivers store 0 dB per band and write -6.86 dB at 20 kHz). Reference
//!   spectrum `White noise` with no attenuation becomes a white [`Spectrum`] at `Lw`, `Pink
//!   noise` a pink one; anything else a custom one with those relative levels.
//! - **Sources**, in load order through any nesting of source groups (a `sources` element of type
//!   15 inside the list), as upstream's GUI loads them (`e_scene_sources.h:73-87`) and writes them,
//!   a group's sources in place of the group (`e_scene_sources.h:192-202`); a child of any other
//!   type is refused ([`codes::SOURCE_GROUP_MALFORMED`]). **Point receivers.** Both with their
//!   `<position>` children, directions, delays and enable flags; **cutting-plane receivers** from
//!   `verta`, `vertb`, `vertc` and `resolution`.
//! - **Environment** from `atmoconfig`; **SPPS** and **TCR** settings and computed bands from
//!   `Core/spps` and `Core/tc`; **meshing** from `Core/spps/mesh_conf`, with `Core/tc/mesh_conf`
//!   read the same way and any difference noted (upstream meshes each core's run with its own).
//!   A settings property the file lacks keeps upstream's default where upstream's loader
//!   supplies one; a meshing property it lacks reads as upstream's getters read a missing one
//!   (off, 0, empty), since upstream's loader supplies none there but `debugmode`
//!   (`read_mesh_conf`). Each one is noted in the report.
//!
//! Refused with [`ImportError::Unsupported`], by name: directivity balloons, user-defined TetGen
//! parameters, and additional TetGen parameters other than `-Y`. Refused with its own code
//! ([`ImportError::Refused`], [`codes`]): what upstream would skip silently or take in a way this
//! import does not reproduce, each listed in `docs/solver-contract.md`, "Importing an upstream
//! project".

use std::path::Path;

use roxmltree::{Document, Node};
use uuid::Uuid;

use super::appconst::{reference_material, reference_spectrum};
use super::zip::Archive;
use super::{IdSource, ImportError, Result, default_material, read_bytes, weld_key};
use crate::config_xml::widen_f32;
use crate::schema::{
    AirAbsorption, AttenuationUnit, BandKind, BandSet, BoxBound, ComputationMethod, DiffusionLaw,
    Directivity, Environment, F64, Face, FittingShape, FittingZone, FittingZoneId, Geometry,
    GroupId, Material, MaterialId, MeshSettings, PointReceiver, PointReceiverId, Project,
    ProjectId, ReflectionLaw, ReflectionLaws, Rgb, SolverSettings, SoundMapQuantity, Source,
    SourceId, Spectrum, SpectrumShape, SurfaceGroup, SurfaceReceiver, SurfaceReceiverId,
    SurfaceReceiverShape, TcrSettings, Vec3,
};

const FMT_PROJ: &str = "proj";
const FMT_MESH: &str = "sceneMesh.bin";
const FMT_FINFO: &str = "finfo";
const FMT_XML: &str = "projet_config.xml";

/// The element type upstream gives its reference spectra (`typespectre`).
const APP_SPECTRUM_TYPE: i64 = 45;

/// Upstream's element types (`data_manager/element.h:97-206`, numbered from 0; `PROPERTY_FREQ`
/// is pinned at 49, `:145`) that this import tells apart by `eid`.
mod eid {
    /// `ELEMENT_TYPE_SCENE_SOURCES`: the source list, and a source group inside it.
    pub const SOURCES: i64 = 15;
    /// `ELEMENT_TYPE_SCENE_SOURCES_SOURCE`.
    pub const SOURCE: i64 = 16;
    /// `ELEMENT_TYPE_ROW`: one band of a fitting zone's `absorption`.
    pub const ROW: i64 = 50;
    /// `ELEMENT_TYPE_ROW_MATERIAU`: one band of a material.
    pub const ROW_MATERIAU: i64 = 52;
    /// `ELEMENT_TYPE_SCENE_ENCOMBREMENTS_ENCOMBREMENT`: a scene-fitted zone.
    pub const FITTING_MODEL: i64 = 54;
    /// `ELEMENT_TYPE_SCENE_ENCOMBREMENTS_ENCOMBREMENT_CUBOIDE`: a rectangular zone.
    pub const FITTING_BOX: i64 = 56;
}

/// The reasons this import refuses a project with, each its own stable code
/// ([`ImportError::Refused`], [`ImportError::code`]). `docs/solver-contract.md` lists each once,
/// under "Importing an upstream project".
pub mod codes {
    /// A child of `encombrements` whose element type is neither a scene-fitted zone (54) nor a
    /// rectangular one (56), or has none. Upstream's GUI skips it silently
    /// (`e_scene_encombrements.h:56-73`).
    pub const FITTING_TYPE_UNKNOWN: &str = "proj_fitting_type_unknown";
    /// An enabled scene-fitted zone whose inside point `volpos` is missing or (0, 0, 0) while it
    /// lists faces. Upstream then derives a seed from the zone's first face
    /// (`Objet3D_maillage.cpp:1012-1031`), which this import does not reproduce.
    pub const FITTING_INSIDE_POINT_UNSET: &str = "proj_fitting_inside_point_unset";
    /// An enabled scene-fitted zone without its face group (`gr`). Upstream writes the zone into
    /// `config.xml` but seeds no region for it (`GetMaillageVolumeInfos` returns false,
    /// `e_scene_encombrements_encombrement_model.h:146-157, 178-191`), which a project cannot
    /// hold: this import would seed one at its inside point. Upstream's GUI always creates the
    /// group (`..._model.h:122`), so only an edited file has none.
    pub const FITTING_FACE_GROUP_MISSING: &str = "proj_fitting_face_group_missing";
    /// An enabled rectangular zone whose corners `ba` and `hc` share a coordinate: it has no
    /// volume. Upstream builds no triangles for equal corners and flat ones for a shared
    /// coordinate (`e_scene_encombrements_encombrement_cuboide.h:113-165`), and still seeds a
    /// region at `hc`.
    pub const FITTING_BOX_EMPTY: &str = "proj_fitting_box_empty";
    /// A face listed by two enabled fitting zones. Upstream would give it the last one's id,
    /// silently (`appconfig.cpp:174-200`, which tags faces for enabled zones only, `:185`).
    pub const FACE_IN_TWO_FITTING_ZONES: &str = "proj_face_in_two_fitting_zones";
    /// A fitting zone's diffusion law (`loi_diff`) that SPPS has no case for (0 to 2,
    /// `coreTypes.h:108-113`), in a band upstream's loader keeps as stored.
    pub const DIFFUSION_LAW_OUT_OF_RANGE: &str = "proj_diffusion_law_out_of_range";
    /// A material's reflection law (`loi`) in some band that is none of upstream's seven (0 to 6,
    /// `appconfig.cpp:110-117`).
    pub const REFLECTION_LAW_OUT_OF_RANGE: &str = "proj_reflection_law_out_of_range";
    /// A material band row of a project older than 1.3.4 (`<bfreq absorb=..>`) with a value
    /// upstream's loader does not read (`e_data_row_materiau.h:55-84`): no `loi`, or one that is
    /// not an integer, which `Convertor::ToInt` returns uninitialised
    /// (`manager/sppsString.cpp:107-112`; `wxString::ToLong` leaves its output untouched when it
    /// reads no digit), or an `absorb`, `diffusion` or `affaiblissement` that is missing or not a
    /// number, which upstream's GUI reports as an error and reads as 0 (`StringToFloat`,
    /// `data_manager/e_data.h:212-252`). A missing `affaiblissement` is not one: it means the
    /// band does not transmit.
    pub const MATERIAL_ROW_UNREADABLE: &str = "proj_material_row_unreadable";
    /// A material band that transmits with a loss that lets more energy through than the band
    /// absorbs (`10^(-R/10)` above `alpha`), or with no absorption. Upstream's GUI prevents both
    /// only when the user edits the band (`e_data_row_materiau.h:109-205`; loading calls
    /// `Modified` with the row itself, which matches none of its cases) and writes a loaded
    /// value unchanged (`:98-107`); this crate's writer applies the rule at every write
    /// (`config_xml::transmission_loss_written`), so it would not write the stored value.
    pub const TRANSMISSION_EXCEEDS_ABSORPTION: &str = "proj_transmission_exceeds_absorption";
    /// A child of the source list or of a source group that is neither a source (16) nor a
    /// source group (15), or has no element type. Upstream's GUI skips it silently
    /// (`e_scene_sources.h:73-87`).
    pub const SOURCE_GROUP_MALFORMED: &str = "proj_source_group_malformed";
    /// Volumes (`volumes/volume`): TetGen regions with their own seed and volume bound
    /// (`e_scene_volumes_volume.h:168-188`), which a project does not hold.
    pub const VOLUMES_UNSUPPORTED: &str = "proj_volumes_unsupported";
    /// A `mesh_conf` of SPPS or TCR with "Test mesh topology" (`debugmode`) on. Upstream's GUI then
    /// runs `tetgen -d` alone, skips the scene correction and loads no mesh
    /// (`projet_maillage.cpp:165-174, 212, 242-275`), and runs the solver on whatever mesh it
    /// already held (`projet.cpp:751-769`), which this import does not reproduce.
    pub const MESH_DEBUG_MODE: &str = "proj_mesh_debug_mode";

    /// Every code above, in the order `docs/solver-contract.md` lists them.
    pub const ALL: &[&str] = &[
        FITTING_TYPE_UNKNOWN,
        FITTING_INSIDE_POINT_UNSET,
        FITTING_FACE_GROUP_MISSING,
        FITTING_BOX_EMPTY,
        FACE_IN_TWO_FITTING_ZONES,
        DIFFUSION_LAW_OUT_OF_RANGE,
        REFLECTION_LAW_OUT_OF_RANGE,
        MATERIAL_ROW_UNREADABLE,
        TRANSMISSION_EXCEEDS_ABSORPTION,
        SOURCE_GROUP_MALFORMED,
        VOLUMES_UNSUPPORTED,
        MESH_DEBUG_MODE,
    ];
}

/// A `.proj` imported: the project, and what the import did.
#[derive(Clone, Debug)]
pub struct ProjImport {
    pub project: Project,
    pub report: ProjReport,
}

/// What a `.proj` import read and did.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ProjReport {
    /// The project folder inside the archive, such as `instance1`.
    pub folder: String,
    /// `sceneMesh.bin`'s version.
    pub scene_mesh_version: (u32, u32),
    /// The face record size found from the node spans, if any group has a face.
    pub face_record_bytes: Option<usize>,
    /// Mesh groups (type 1 nodes) in `sceneMesh.bin`.
    pub mesh_groups: usize,
    /// Vertices in `sceneMesh.bin`.
    pub source_vertices: usize,
    /// Vertices merged into an earlier one with equal coordinates.
    pub welded_vertices: usize,
    /// Vertices no face uses, dropped.
    pub unused_vertices: usize,
    pub faces: usize,
    /// Faces per material group of the file, in its order: (name, `idmat`, faces).
    pub material_groups: Vec<(String, u32, usize)>,
    /// Upstream's element id of every source, point receiver, surface receiver and fitting zone
    /// this import made, in project order within each kind (see the module docs, "Element ids").
    pub upstream_ids: Vec<UpstreamId>,
    /// Anything else worth knowing, one sentence each.
    pub notes: Vec<String>,
}

/// One entity of an imported project and the element id upstream's GUI gave it (`wxid`), which
/// upstream writes into `config.xml` as the entity's `@id`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UpstreamId {
    pub kind: UpstreamKind,
    /// The entity's id in the project.
    pub entity: Uuid,
    pub name: String,
    /// Upstream's element id.
    pub upstream: i64,
}

/// The kinds of entity [`UpstreamId`] records.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum UpstreamKind {
    Source,
    PointReceiver,
    /// A scene surface receiver.
    SurfaceReceiver,
    /// A cutting-plane surface receiver.
    CuttingPlane,
    FittingZone,
}

impl UpstreamKind {
    /// The `config.xml` element that carries this kind's id.
    pub fn config_element(self) -> &'static str {
        match self {
            UpstreamKind::Source => "source",
            UpstreamKind::PointReceiver => "recepteur_ponctuel",
            UpstreamKind::SurfaceReceiver => "recepteur_surfacique",
            UpstreamKind::CuttingPlane => "recepteur_surfacique_coupe",
            UpstreamKind::FittingZone => "encombrement",
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            UpstreamKind::Source => "source",
            UpstreamKind::PointReceiver => "point receiver",
            UpstreamKind::SurfaceReceiver => "surface receiver",
            UpstreamKind::CuttingPlane => "cutting plane",
            UpstreamKind::FittingZone => "fitting zone",
        }
    }
}

/// Reads a `.proj` file.
pub fn import_proj_file(path: &Path) -> Result<ProjImport> {
    import_proj(&read_bytes(path)?)
}

/// Reads a `.proj` archive held in memory. See the module docs.
pub fn import_proj(bytes: &[u8]) -> Result<ProjImport> {
    import(bytes, None)
}

/// Reads a `.proj` archive with another element tree in place of its own `projet_config.xml`:
/// the scene, face lists and databases the archive holds, with `projet_config` as the project.
/// Upstream's GUI saves the project it ran beside each run (`report/<core>/<run>/projet_config.xml`),
/// so this reads a run's project as it was when upstream wrote that run's inputs (tutorial 3's
/// three runs differ in `nbparticules` and `trans_calc`).
pub fn import_proj_with_config(bytes: &[u8], projet_config: &[u8]) -> Result<ProjImport> {
    import(bytes, Some(projet_config))
}

fn import(bytes: &[u8], projet_config: Option<&[u8]>) -> Result<ProjImport> {
    let archive = Archive::parse(bytes)?;
    // The project folder: the one `projet_config.xml` directly inside a top-level folder (or at
    // the root). Copies under report/ are snapshots of past runs.
    let configs: Vec<&str> = archive
        .entries()
        .iter()
        .map(|e| e.name.as_str())
        .filter(|n| {
            let parts: Vec<&str> = n.split('/').collect();
            parts.last() == Some(&"projet_config.xml") && parts.len() <= 2
        })
        .collect();
    let config_name = match configs.as_slice() {
        [one] => one.to_string(),
        [] => {
            return Err(ImportError::invalid(
                FMT_PROJ,
                "the archive holds no projet_config.xml: not an I-Simpa project",
            ));
        }
        _ => {
            return Err(ImportError::unsupported(
                FMT_PROJ,
                format!("the archive holds {} project folders", configs.len()),
            ));
        }
    };
    let folder = config_name
        .strip_suffix("projet_config.xml")
        .unwrap_or("")
        .to_string();
    let mut report = ProjReport {
        folder: folder.trim_end_matches('/').to_string(),
        ..ProjReport::default()
    };

    let xml_bytes = match projet_config {
        Some(b) => b.to_vec(),
        None => archive.read(&config_name)?,
    };
    let xml_text = match std::str::from_utf8(&xml_bytes) {
        Ok(t) => t.to_string(),
        Err(_) => {
            report.notes.push(
                "projet_config.xml is not valid UTF-8; invalid bytes were replaced (names may \
                 show U+FFFD)"
                    .to_string(),
            );
            String::from_utf8_lossy(&xml_bytes).into_owned()
        }
    };
    let doc = Document::parse(&xml_text).map_err(|e| {
        let p = e.pos();
        ImportError::syntax(FMT_XML, format!("{}:{}", p.row, p.col), e.to_string())
    })?;
    let root = doc.root_element();
    if root.tag_name().name() != "projet" {
        return Err(ImportError::invalid(
            FMT_XML,
            format!(
                "the root element is <{}>, not <projet>",
                root.tag_name().name()
            ),
        ));
    }
    let scene = child(root, "Scene")?;
    let data = child(scene, "donnees")?;
    let project_el = child(scene, "projet")?;

    // The scene mesh.
    let mesh_name = element_kids(project_el)
        .filter(|c| c.has_tag_name("Configuration"))
        .find_map(|c| prop(c, "urlmodel"))
        .and_then(|p| p.attribute("textValue"))
        .unwrap_or("sceneMesh.bin")
        .to_string();
    let mesh_bytes = archive.read(&format!("{folder}{}", plain_file_name(&mesh_name)?))?;
    let mesh = read_scene_mesh(&mesh_bytes)?;
    report.scene_mesh_version = mesh.version;
    report.face_record_bytes = mesh.face_record_bytes;
    report.mesh_groups = mesh.groups.len();
    report.source_vertices = mesh.vertices.len();

    // Faces, in file order, and where each mesh group starts.
    let mut group_start = Vec::with_capacity(mesh.groups.len());
    let mut faces: Vec<[u32; 3]> = Vec::new();
    for g in &mesh.groups {
        group_start.push(faces.len());
        faces.extend_from_slice(&g.faces);
    }
    let n_faces = faces.len();
    report.faces = n_faces;
    if n_faces == 0 {
        return Err(ImportError::Empty { format: FMT_MESH });
    }
    let mut digest_parts: Vec<Vec<u8>> = vec![xml_bytes.clone(), mesh_bytes.clone()];

    // Material groups and scene receivers, from their .finfo lists.
    let mut face_group: Vec<Option<usize>> = vec![None; n_faces];
    let mut groups_in: Vec<(String, u32)> = Vec::new();
    let sgroupes = child(data, "sgroupes")?;
    for gr in element_kids(sgroupes) {
        if !gr.has_tag_name("gr") {
            return Err(ImportError::unsupported(
                FMT_XML,
                format!("<{}> in the surface groups", gr.tag_name().name()),
            ));
        }
        let name = gr.attribute("name").unwrap_or("").to_string();
        let what = format!("surface group `{name}`");
        let idmat = prop_int(gr, "idmat", &what)?;
        let idmat = u32::try_from(idmat).map_err(|_| {
            ImportError::invalid(FMT_XML, format!("{what} has material id {idmat}"))
        })?;
        let k = groups_in.len();
        let list = finfo_faces(&archive, &folder, gr, &mesh, &group_start, &what)?;
        let list = match list {
            Some((faces, bytes)) => {
                digest_parts.push(bytes);
                faces
            }
            None => {
                report.notes.push(format!(
                    "{what}: its face list is not in the archive, so it holds no face (as \
                     upstream reads it)"
                ));
                Vec::new()
            }
        };
        for f in list {
            if let Some(prev) = face_group[f].replace(k)
                && prev != k
            {
                return Err(ImportError::invalid(
                    FMT_PROJ,
                    format!(
                        "face {f} is in two surface groups, `{}` and `{name}`; upstream would \
                         let the last one win",
                        groups_in[prev].0
                    ),
                ));
            }
        }
        groups_in.push((name, idmat));
    }
    let mut counts = vec![0usize; groups_in.len()];
    for g in face_group.iter().flatten() {
        counts[*g] += 1;
    }
    for ((name, idmat), count) in groups_in.iter().zip(counts) {
        report.material_groups.push((name.clone(), *idmat, count));
    }

    // Surface receivers, with upstream's element id.
    enum Rs {
        Scene { name: String, enabled: bool },
        Plane(SurfaceReceiverShape, String, bool),
    }
    let mut receivers: Vec<(Rs, Option<i64>)> = Vec::new();
    let mut face_receiver: Vec<Option<usize>> = vec![None; n_faces];
    if let Some(list) = opt_child(data, "recepteurss") {
        for r in element_kids(list) {
            let name = r.attribute("name").unwrap_or("").to_string();
            let what = format!("surface receiver `{name}`");
            let props = opt_child(r, "prop");
            let enabled = match props {
                Some(p) => opt_prop_bool(p, "enabled", &what)?.unwrap_or(true),
                None => true,
            };
            match r.tag_name().name() {
                "recepteurs" => {
                    let k = receivers.len();
                    if let Some(gr) = element_kids(r).find(|c| c.has_tag_name("gr")) {
                        let list = finfo_faces(&archive, &folder, gr, &mesh, &group_start, &what)?;
                        let list = match list {
                            Some((faces, bytes)) => {
                                digest_parts.push(bytes);
                                faces
                            }
                            None => {
                                report.notes.push(format!(
                                    "{what}: its face list is not in the archive, so it covers \
                                     no face (as upstream reads it)"
                                ));
                                Vec::new()
                            }
                        };
                        for f in list {
                            if let Some(prev) = face_receiver[f].replace(k)
                                && prev != k
                            {
                                return Err(ImportError::unsupported(
                                    FMT_PROJ,
                                    format!(
                                        "face {f} is on two surface receivers; a face carries one"
                                    ),
                                ));
                            }
                        }
                    }
                    receivers.push((Rs::Scene { name, enabled }, element_id(r)));
                }
                "recepteurscoupe" => {
                    let a = position(r, "verta", &what)?;
                    let b = position(r, "vertb", &what)?;
                    let c = position(r, "vertc", &what)?;
                    let resolution = match props {
                        Some(p) => prop_real(p, "resolution", &what)?,
                        None => {
                            return Err(ImportError::invalid(
                                FMT_XML,
                                format!("{what} has no properties"),
                            ));
                        }
                    };
                    receivers.push((
                        Rs::Plane(
                            SurfaceReceiverShape::CuttingPlane {
                                a,
                                b,
                                c,
                                resolution_m: F64::new(resolution),
                            },
                            name,
                            enabled,
                        ),
                        element_id(r),
                    ));
                }
                other => {
                    return Err(ImportError::unsupported(
                        FMT_XML,
                        format!("surface receiver element <{other}>"),
                    ));
                }
            }
        }
    }

    // Bands: upstream's GUI always holds all 27 third-octave bands.
    let bands = BandSet::range(BandKind::ThirdOctave, 50, 20_000).expect("nominal range");
    let n = bands.len();

    // Fitting zones, and the zones (in load order) that list each face. A face may be listed by
    // several scene-fitted zones as long as at most one of them is enabled: upstream tags a face
    // for enabled zones only (`GetFaceLink`, `appconfig.cpp:174-200`, the test at `:185`).
    let mut zones: Vec<ZoneIn> = Vec::new();
    let mut face_zones: Vec<Vec<usize>> = vec![Vec::new(); n_faces];
    if let Some(list) = opt_child(data, "encombrements") {
        let files = ZoneFiles {
            archive: &archive,
            folder: &folder,
            mesh: &mesh,
            group_start: &group_start,
        };
        for z in element_kids(list) {
            let k = zones.len();
            let zone = read_zone(z, &files, &bands, &mut digest_parts, &mut report.notes)?;
            if let ZoneKind::Model { faces, .. } = &zone.kind {
                for &f in faces {
                    let listed = &mut face_zones[f];
                    if listed.contains(&k) {
                        continue;
                    }
                    if let Some(&prev) = listed.iter().find(|&&p| zones[p].enabled)
                        && zone.enabled
                    {
                        return Err(ImportError::refused(
                            FMT_PROJ,
                            codes::FACE_IN_TWO_FITTING_ZONES,
                            format!(
                                "face {f} is in two enabled fitting zones, `{}` and `{}`; \
                                 upstream would give it the last one's id, silently \
                                 (appconfig.cpp:174-200)",
                                zones[prev].name, zone.name
                            ),
                        ));
                    }
                    listed.push(k);
                }
            }
            zones.push(zone);
        }
    }
    // Volumes are not read.
    if let Some(list) = opt_child(data, "volumes")
        && let Some(first) = element_kids(list).next()
    {
        return Err(ImportError::refused(
            FMT_XML,
            codes::VOLUMES_UNSUPPORTED,
            format!(
                "volume `{}`: volumes are TetGen regions with their own seed and volume bound \
                 (e_scene_volumes_volume.h:168-188), which a project does not hold",
                first.attribute("name").unwrap_or("")
            ),
        ));
    }

    let digest_refs: Vec<&[u8]> = digest_parts.iter().map(Vec::as_slice).collect();
    let mut all_parts: Vec<&[u8]> = vec![b"upstream proj"];
    all_parts.extend(digest_refs);
    let ids = IdSource::from_parts(&all_parts);

    // Materials: the project's own database, then the reference materials groups use.
    let mut materials: Vec<Material> = Vec::new();
    if let Some(bdd) = opt_child(project_el, "bdd")
        && let Some(mats) = opt_child(bdd, "materiaux")
    {
        for m in preorder(mats)
            .into_iter()
            .filter(|d| d.has_tag_name("materiau"))
        {
            let index = materials.len();
            let material = read_material(m, &bands, MaterialId(ids.uuid("material", index)))?;
            if materials.iter().any(|x| x.solver_id == material.solver_id) {
                return Err(ImportError::invalid(
                    FMT_XML,
                    format!(
                        "two materials of the project database have id {}",
                        material.solver_id.unwrap_or(0)
                    ),
                ));
            }
            materials.push(material);
        }
    }
    let material_for = |idmat: u32, materials: &mut Vec<Material>| -> Result<MaterialId> {
        if let Some(m) = materials.iter().find(|m| m.solver_id == Some(idmat)) {
            return Ok(m.id);
        }
        let r = reference_material(idmat).ok_or_else(|| {
            ImportError::unsupported(
                FMT_PROJ,
                format!(
                    "material id {idmat} is neither in the project's database nor one of \
                     upstream's reference materials"
                ),
            )
        })?;
        let index = materials.len();
        let mut m = default_material(MaterialId(ids.uuid("material", index)), n);
        m.name = r.name.to_string();
        m.color = Rgb(r.color[0], r.color[1], r.color[2]);
        m.absorption = vec![F64::new(widen_f32(r.absorption)); n];
        m.solver_id = Some(idmat);
        materials.push(m);
        Ok(materials[index].id)
    };

    // Surface groups: one per (material group, scene receiver, scene-fitted zones) key, the
    // receiver stored plus one so that "none" (NONE + 1 wraps to 0) sorts first, as no zone (the
    // empty list) does.
    const NONE: usize = usize::MAX;
    type Key = (usize, usize, Vec<usize>);
    let key_of = |f: usize| -> Key {
        (
            face_group[f].unwrap_or(NONE),
            face_receiver[f].unwrap_or(NONE).wrapping_add(1),
            face_zones[f].clone(),
        )
    };
    let mut keys: Vec<Key> = (0..n_faces).map(key_of).collect();
    keys.sort_unstable();
    keys.dedup();
    let mut surface_groups: Vec<SurfaceGroup> = Vec::with_capacity(keys.len());
    let unassigned = face_group.iter().filter(|g| g.is_none()).count();
    for (i, (g, r1, z)) in keys.iter().enumerate() {
        let (g, r) = (*g, r1.wrapping_sub(1));
        let split_by_receiver = keys.iter().any(|k| k.0 == g && k.1 != *r1);
        let split_by_zone = keys.iter().any(|k| k.0 == g && k.2 != *z);
        let (base, material) = if g == NONE {
            (
                "(no surface group)".to_string(),
                material_for(0, &mut materials)?,
            )
        } else {
            (
                groups_in[g].0.clone(),
                material_for(groups_in[g].1, &mut materials)?,
            )
        };
        let mut name = base;
        if split_by_receiver && let Some((Rs::Scene { name: receiver, .. }, _)) = receivers.get(r) {
            name = format!("{name} / {receiver}");
        }
        if split_by_zone && !z.is_empty() {
            let names: Vec<&str> = z.iter().map(|&k| zones[k].name.as_str()).collect();
            name = format!("{name} / {}", names.join(" + "));
        }
        surface_groups.push(SurfaceGroup {
            id: GroupId(ids.uuid("surface group", i)),
            name,
            material,
        });
    }
    if unassigned > 0 {
        report.notes.push(format!(
            "{unassigned} faces are in no surface group; they get upstream's default material \
             (reference material 0), as upstream gives them"
        ));
    }
    let key_group = |f: usize| -> GroupId {
        let i = keys
            .binary_search(&key_of(f))
            .expect("every face's key is listed");
        surface_groups[i].id
    };

    // Geometry: welded vertices, faces in file order.
    let mut used = vec![false; mesh.vertices.len()];
    for f in &faces {
        for &v in f {
            used[v as usize] = true;
        }
    }
    let mut remap = vec![u32::MAX; mesh.vertices.len()];
    let mut seen = std::collections::HashMap::new();
    let mut vertices: Vec<Vec3> = Vec::new();
    for (i, v) in mesh.vertices.iter().enumerate() {
        if !used[i] {
            report.unused_vertices += 1;
            continue;
        }
        let p = v.map(widen_f32);
        let next = vertices.len() as u32;
        let idx = *seen.entry(weld_key(p)).or_insert(next);
        if idx == next {
            vertices.push(Vec3::from(p.map(|c| c + 0.0)));
        } else {
            report.welded_vertices += 1;
        }
        remap[i] = idx;
    }
    let geometry = Geometry {
        vertices,
        faces: faces
            .iter()
            .enumerate()
            .map(|(f, t)| Face {
                vertices: t.map(|v| remap[v as usize]),
                group: key_group(f),
            })
            .collect(),
    };
    let groups_where = |keep: &dyn Fn(&Key) -> bool| -> Vec<GroupId> {
        keys.iter()
            .zip(&surface_groups)
            .filter(|(key, _)| keep(key))
            .map(|(_, g)| g.id)
            .collect()
    };

    // Surface receivers.
    let mut surface_receivers = Vec::with_capacity(receivers.len());
    for (k, (r, wxid)) in receivers.into_iter().enumerate() {
        let id = SurfaceReceiverId(ids.uuid("surface receiver", k));
        let receiver = match r {
            Rs::Scene { name, enabled } => SurfaceReceiver {
                id,
                solver_id: pin(wxid, &format!("surface receiver `{name}`"))?,
                name,
                enabled,
                shape: SurfaceReceiverShape::Scene {
                    groups: groups_where(&|key| key.1 == k.wrapping_add(1)),
                },
            },
            Rs::Plane(shape, name, enabled) => SurfaceReceiver {
                id,
                solver_id: pin(wxid, &format!("surface receiver `{name}`"))?,
                name,
                enabled,
                shape,
            },
        };
        if let Some(upstream) = wxid {
            report.upstream_ids.push(UpstreamId {
                kind: match receiver.shape {
                    SurfaceReceiverShape::Scene { .. } => UpstreamKind::SurfaceReceiver,
                    SurfaceReceiverShape::CuttingPlane { .. } => UpstreamKind::CuttingPlane,
                },
                entity: id.0,
                name: receiver.name.clone(),
                upstream,
            });
        }
        surface_receivers.push(receiver);
    }

    // Fitting zones.
    let mut fitting_zones = Vec::with_capacity(zones.len());
    for (k, z) in zones.into_iter().enumerate() {
        let id = FittingZoneId(ids.uuid("fitting zone", k));
        let shape = match z.kind {
            ZoneKind::Model { inside_point, .. } => FittingShape::Surfaces {
                groups: groups_where(&|key| key.2.contains(&k)),
                inside_point,
            },
            ZoneKind::Box { ba, hc } => box_shape(ba, hc),
        };
        if let Some(upstream) = z.wxid {
            report.upstream_ids.push(UpstreamId {
                kind: UpstreamKind::FittingZone,
                entity: id.0,
                name: z.name.clone(),
                upstream,
            });
        }
        fitting_zones.push(FittingZone {
            id,
            solver_id: pin(z.wxid, &format!("fitting zone `{}`", z.name))?,
            name: z.name,
            enabled: z.enabled,
            shape,
            absorption: z.bands.absorption,
            mean_free_path_m: z.bands.mean_free_path_m,
            diffusion_law: z.bands.diffusion_law,
        });
    }

    // Sources, through any nesting of source groups, and point receivers.
    let mut sources = Vec::new();
    if let Some(list) = opt_child(data, "sources") {
        for (s, group) in source_elements(list)? {
            let index = sources.len();
            let id = SourceId(ids.uuid("source", index));
            let mut source = read_source(s, &bands, id).map_err(|e| in_group(e, &group))?;
            source.group = (!group.is_empty()).then(|| group.clone());
            if let Some(upstream) = element_id(s) {
                report.upstream_ids.push(UpstreamId {
                    kind: UpstreamKind::Source,
                    entity: id.0,
                    name: source.name.clone(),
                    upstream,
                });
            }
            sources.push(source);
        }
    }
    let mut point_receivers = Vec::new();
    if let Some(list) = opt_child(data, "recepteursp") {
        for r in element_kids(list) {
            if !r.has_tag_name("recepteurp") {
                return Err(ImportError::unsupported(
                    FMT_XML,
                    format!("<{}> in the point receivers", r.tag_name().name()),
                ));
            }
            let index = point_receivers.len();
            let receiver = read_point_receiver(
                r,
                &bands,
                PointReceiverId(ids.uuid("point receiver", index)),
            )?;
            if let Some(upstream) = element_id(r) {
                report.upstream_ids.push(UpstreamId {
                    kind: UpstreamKind::PointReceiver,
                    entity: receiver.id.0,
                    name: receiver.name.clone(),
                    upstream,
                });
            }
            point_receivers.push(receiver);
        }
    }

    // Environment and solver settings.
    let environment = read_environment(project_el, &mut report.notes)?;
    let core = child(root, "Core")?;
    let solvers = read_solvers(core, &bands, &mut report.notes)?;

    // Name and description.
    let info = element_kids(project_el)
        .filter(|c| c.has_tag_name("Configuration"))
        .find(|c| prop(*c, "projectname").is_some());
    let text = |name: &str| {
        info.and_then(|c| prop(c, name))
            .and_then(|p| p.attribute("textValue"))
            .unwrap_or("")
            .trim()
            .to_string()
    };
    let name = match text("projectname") {
        n if n.is_empty() => "Imported project".to_string(),
        n => n,
    };
    let mut description = text("projectdesc");
    if description.is_empty() {
        description = format!(
            "Imported from an upstream I-Simpa project ({} faces, {} surface groups).",
            n_faces,
            surface_groups.len()
        );
    }

    let project = Project {
        format_version: crate::schema::FORMAT_VERSION,
        id: ProjectId(ids.uuid("project", 0)),
        name,
        description,
        frame: Default::default(),
        bands,
        geometry,
        surface_groups,
        materials,
        sources,
        point_receivers,
        surface_receivers,
        fitting_zones,
        environment,
        solvers,
        variants: Vec::new(),
        active_variant: None,
        view: Default::default(),
    };
    project.check_integrity().map_err(ImportError::Integrity)?;
    Ok(ProjImport { project, report })
}

// ---------------------------------------------------------------------------------------------
// Element types and ids.

/// An element's type, `@eid`, as upstream's loaders read it.
fn element_type(node: Node<'_, '_>) -> Option<i64> {
    node.attribute("eid")
        .and_then(|v| v.trim().parse::<i64>().ok())
}

/// An element's id in upstream's GUI, `@wxid`: what it writes as the entity's `@id` in
/// `config.xml`.
fn element_id(node: Node<'_, '_>) -> Option<i64> {
    node.attribute("wxid")
        .and_then(|v| v.trim().parse::<i64>().ok())
}

/// The solver id an imported entity is pinned to: its element id (`wxid`), which upstream writes
/// as its `@id` (see the module docs, "Element ids"); `None` without one. An id the solvers
/// cannot read as a C `int`, or below 0, is invalid.
fn pin(wxid: Option<i64>, what: &str) -> Result<Option<u32>> {
    match wxid {
        None => Ok(None),
        Some(id) => u32::try_from(id)
            .ok()
            .filter(|&v| v <= crate::schema::SOLVER_INT_MAX)
            .map(Some)
            .ok_or_else(|| {
                ImportError::invalid(
                    FMT_XML,
                    format!(
                        "{what} has element id {id}, which no solver reads as an id (0 to {})",
                        crate::schema::SOLVER_INT_MAX
                    ),
                )
            }),
    }
}

// ---------------------------------------------------------------------------------------------
// Sources and source groups.

/// Every source element of the source list `list`, in upstream's load order ([`kids`]) through
/// any nesting of source groups, with the path of the groups it sits in (`Milling Machine`, or
/// empty at the top level). Upstream's GUI loads a child of type 16 as a source and one of type 15
/// as a group, whose children it reads the same way (`e_scene_sources.h:73-87`, after its element
/// constructor has sorted them, `element.cpp:159`); on writing, a group writes each of its sources
/// in its place (`Element::SaveXMLCoreDoc`, `element.cpp:635-644`; the group's own
/// `SaveXMLCoreDoc`, `e_scene_sources.h:192-202`), so this order, reversed as every list is
/// (`config_xml`), is the order upstream writes. Any other child is refused, where upstream skips
/// it silently: [`codes::SOURCE_GROUP_MALFORMED`]. Walked with an explicit stack, so no nesting
/// depth can overflow the call stack.
fn source_elements<'a, 'i>(list: Node<'a, 'i>) -> Result<Vec<(Node<'a, 'i>, String)>> {
    let mut out = Vec::new();
    let children = |n: Node<'a, 'i>| element_kids(n).collect::<Vec<_>>();
    let mut stack: Vec<(Node<'a, 'i>, String)> = children(list)
        .into_iter()
        .rev()
        .map(|c| (c, String::new()))
        .collect();
    while let Some((node, group)) = stack.pop() {
        let name = node.attribute("name").unwrap_or("");
        match element_type(node) {
            Some(eid::SOURCE) => out.push((node, group)),
            Some(eid::SOURCES) => {
                let path = if group.is_empty() {
                    name.to_string()
                } else {
                    format!("{group} / {name}")
                };
                for c in children(node).into_iter().rev() {
                    stack.push((c, path.clone()));
                }
            }
            other => {
                let place = if group.is_empty() {
                    "the sound sources".to_string()
                } else {
                    format!("source group `{group}`")
                };
                let what = match other {
                    Some(t) => format!("element type {t}"),
                    None => "no element type (eid)".to_string(),
                };
                return Err(ImportError::refused(
                    FMT_XML,
                    codes::SOURCE_GROUP_MALFORMED,
                    format!(
                        "<{} name=\"{name}\"> in {place} has {what}: it is neither a source (16) \
                         nor a source group (15), and upstream's GUI would skip it silently \
                         (e_scene_sources.h:73-87)",
                        node.tag_name().name()
                    ),
                ));
            }
        }
    }
    Ok(out)
}

/// A source's error, naming the group the source sits in.
fn in_group(e: ImportError, group: &str) -> ImportError {
    if group.is_empty() {
        return e;
    }
    match e {
        ImportError::Invalid { format, message } => ImportError::Invalid {
            format,
            message: format!("source group `{group}`: {message}"),
        },
        ImportError::Unsupported { format, message } => ImportError::Unsupported {
            format,
            message: format!("source group `{group}`: {message}"),
        },
        other => other,
    }
}

// ---------------------------------------------------------------------------------------------
// Fitting zones.

/// A fitting zone as the project file holds it (see the module docs).
struct ZoneIn {
    name: String,
    wxid: Option<i64>,
    enabled: bool,
    kind: ZoneKind,
    bands: ZoneBands,
}

enum ZoneKind {
    /// Element type 54: the faces its own face list names, and its `volpos`.
    Model {
        faces: Vec<usize>,
        inside_point: Vec3,
    },
    /// Element type 56: its corners `ba` and `hc`, as stored.
    Box { ba: Vec3, hc: Vec3 },
}

/// A fitting zone's per-band values.
struct ZoneBands {
    absorption: Vec<F64>,
    mean_free_path_m: Vec<F64>,
    diffusion_law: Vec<DiffusionLaw>,
}

/// What a scene-fitted zone's face list is read from.
struct ZoneFiles<'r, 'a> {
    archive: &'r Archive<'a>,
    folder: &'r str,
    mesh: &'r SceneMesh,
    group_start: &'r [usize],
}

/// One child of `encombrements` (see the module docs).
fn read_zone(
    z: Node<'_, '_>,
    files: &ZoneFiles<'_, '_>,
    bands: &BandSet,
    digest_parts: &mut Vec<Vec<u8>>,
    notes: &mut Vec<String>,
) -> Result<ZoneIn> {
    let name = z.attribute("name").unwrap_or("").to_string();
    let what = format!("fitting zone `{name}`");
    let is_model = match element_type(z) {
        Some(eid::FITTING_MODEL) => true,
        Some(eid::FITTING_BOX) => false,
        other => {
            let found = match other {
                Some(t) => format!("element type {t}"),
                None => "no element type (eid)".to_string(),
            };
            return Err(ImportError::refused(
                FMT_XML,
                codes::FITTING_TYPE_UNKNOWN,
                format!(
                    "<{}> `{name}` in the fitting zones has {found}: it is neither a scene-fitted \
                     zone (54) nor a rectangular one (56), and upstream's GUI would skip it \
                     silently (e_scene_encombrements.h:56-73)",
                    z.tag_name().name()
                ),
            ));
        }
    };
    // `useforcalculation`, read through `GetBoolConfig`, which gives false for a missing
    // property (`element.cpp:1300-1314`); a missing `prop` element would crash upstream's GUI,
    // which looks the property up on it unchecked (`..._cuboide.h:311`, `..._model.h:148`).
    let props = opt_child(z, "prop")
        .ok_or_else(|| ImportError::invalid(FMT_XML, format!("{what} has no properties (prop)")))?;
    let enabled = match opt_prop_bool(props, "useforcalculation", &what)? {
        Some(on) => on,
        None => {
            notes.push(format!(
                "{what} has no `useforcalculation`: upstream reads that as off, so the zone is \
                 disabled"
            ));
            false
        }
    };
    let zone_bands = read_zone_bands(z, bands, &what, notes)?;
    // What follows refuses only what upstream would act on: a disabled zone seeds no region
    // (`GetMaillageVolumeInfos` gives false, `..._model.h:193`, `..._cuboide.h:331`,
    // `drawable_element.cpp:250-253`), tags no face (`appconfig.cpp:185`) and draws no triangle
    // (`..._cuboide.h:311`), so it is kept as stored, and a note says what enabling it would need.
    let off = if enabled { "" } else { " (disabled)" };
    let kind = if is_model {
        let faces = match element_kids(z).find(|c| c.has_tag_name("gr")) {
            Some(gr) => match finfo_faces(
                files.archive,
                files.folder,
                gr,
                files.mesh,
                files.group_start,
                &what,
            )? {
                Some((faces, bytes)) => {
                    digest_parts.push(bytes);
                    faces
                }
                None => {
                    notes.push(format!(
                        "{what}{off}: its face list is not in the archive, so it lists no face \
                         (as upstream reads it)"
                    ));
                    Vec::new()
                }
            },
            // Upstream writes an enabled zone without its face group into config.xml but seeds
            // no region for it (`..._model.h:146-157, 178-191`); a project cannot hold that.
            None if enabled => {
                return Err(ImportError::refused(
                    FMT_XML,
                    codes::FITTING_FACE_GROUP_MISSING,
                    format!(
                        "{what} has no face group (gr): upstream writes it into config.xml but \
                         seeds no TetGen region for it (e_scene_encombrements_encombrement_model.h:\
                         178-191), which a project cannot hold; this import would seed one at its \
                         inside position"
                    ),
                ));
            }
            None => {
                notes.push(format!(
                    "{what}{off} has no face group (gr): it lists no face, and enabled, upstream \
                     would seed no region for it"
                ));
                Vec::new()
            }
        };
        // Upstream's GUI gives a zone without a position one named `volpos` at (0, 0, 0)
        // (`..._model.h:108-112`), and seeds the region of a zone that lists faces at a point
        // derived from its first face instead when `volpos` is (0, 0, 0)
        // (`Objet3D_maillage.cpp:1012-1031`).
        let has_volpos = element_kids(z)
            .any(|c| c.has_tag_name("position") && c.attribute("name") == Some("volpos"));
        let inside_point = if has_volpos {
            position(z, "volpos", &what)?
        } else {
            Vec3::ZERO
        };
        let unset = if has_volpos {
            "the inside position (0, 0, 0)"
        } else {
            "no inside position (volpos)"
        };
        if inside_point.to_array() == [0.0; 3] && !faces.is_empty() {
            if enabled {
                return Err(ImportError::refused(
                    FMT_XML,
                    codes::FITTING_INSIDE_POINT_UNSET,
                    format!(
                        "{what} has {unset}: upstream seeds its region at a point derived from \
                         its first face in its OpenGL frame (Objet3D_maillage.cpp:1012-1031), \
                         which this import does not reproduce; set the zone's inside position"
                    ),
                ));
            }
            notes.push(format!(
                "{what}{off} has {unset}: enabled, upstream would seed its region at a point \
                 derived from its first face (Objet3D_maillage.cpp:1012-1031) and this project at \
                 (0, 0, 0); set its inside position before enabling it"
            ));
        }
        if faces.is_empty() && enabled {
            notes.push(format!(
                "{what} lists no face: upstream still seeds a region at its inside position"
            ));
        }
        ZoneKind::Model {
            faces,
            inside_point,
        }
    } else {
        let ba = position(z, "ba", &what)?;
        let hc = position(z, "hc", &what)?;
        let (a, c) = (ba.to_array(), hc.to_array());
        if let Some(axis) = (0..3).find(|&k| a[k] == c[k]) {
            let flat = format!(
                "{what}{off}: its corners ba {a:?} and hc {c:?} share the {} coordinate, so it \
                 has no volume",
                ["x", "y", "z"][axis]
            );
            if enabled {
                return Err(ImportError::refused(
                    FMT_XML,
                    codes::FITTING_BOX_EMPTY,
                    format!(
                        "{flat}; upstream would still seed a region at hc \
                         (e_scene_encombrements_encombrement_cuboide.h:113-165, 331-340)"
                    ),
                ));
            }
            notes.push(format!(
                "{flat}; kept as stored, since upstream ignores a disabled zone, and refused \
                 by the mesher if enabled"
            ));
        }
        ZoneKind::Box { ba, hc }
    };
    Ok(ZoneIn {
        name,
        wxid: element_id(z),
        enabled,
        kind,
        bands: zone_bands,
    })
}

/// A rectangular zone's shape: its bounds, and which bound `hc` takes on each axis (`None` when
/// `hc` is the upper corner, as for a box drawn here).
fn box_shape(ba: Vec3, hc: Vec3) -> FittingShape {
    let (a, c) = (ba.to_array(), hc.to_array());
    let bound = |k: usize| {
        if c[k] > a[k] {
            BoxBound::Max
        } else {
            BoxBound::Min
        }
    };
    let destination = [bound(0), bound(1), bound(2)];
    FittingShape::Box {
        min: Vec3::from([0, 1, 2].map(|k| a[k].min(c[k]))),
        max: Vec3::from([0, 1, 2].map(|k| a[k].max(c[k]))),
        destination: (destination != [BoxBound::Max; 3]).then_some(destination),
    }
}

/// A fitting zone's `absorption` rows: per band `alpha`, `lambda` and `loi_diff`, as upstream
/// writes them (`E_GammeAbsorption::SaveXMLCoreDoc`, `generic_element/e_gammeabsorption.cpp:
/// 76-87`; the `Average` row, `moyenne`, is not written).
///
/// **The diffusion law as upstream's loader leaves it.** Loading a zone, upstream's GUI walks its
/// rows in order and replaces the `loi_diff` list of each by a new one set to 0 ("Uniform
/// reflection"), until it meets a row whose list already has at least two entries, the first
/// labelled "Uniform reflection"; that row and every later one keep their value
/// (`e_gammeabsorption.cpp:43-59`). A list is read from the file in document order
/// (`e_data_list.h:52-82`), and upstream saves every list newest entry first
/// (`e_data_list.h:160-167`, `wxXmlNode` puts a new child first), so a list upstream saved starts
/// with "Lambert reflection" and is replaced on every load: every zone of every project upstream
/// saved is written with law 0 in every band. This import does the same, and notes each stored
/// law it resets. A band with no `loi_diff` gets 0, the value SPPS reads for the missing
/// attribute. A law kept as stored must be one SPPS has a case for, 0 to 2
/// ([`codes::DIFFUSION_LAW_OUT_OF_RANGE`]).
fn read_zone_bands(
    z: Node<'_, '_>,
    bands: &BandSet,
    what: &str,
    notes: &mut Vec<String>,
) -> Result<ZoneBands> {
    let rows = opt_child(z, "absorption").ok_or_else(|| {
        ImportError::invalid(
            FMT_XML,
            format!("{what} has no acoustic parameters (absorption)"),
        )
    })?;
    let n = bands.len();
    let mut alpha = vec![None; n];
    let mut lambda = vec![None; n];
    let mut law = vec![None; n];
    let mut replacing = true;
    let mut reset: Vec<(u32, i64)> = Vec::new();
    let mut missing: Vec<u32> = Vec::new();
    // Upstream writes, and resets the law of, the rows of element type ROW only
    // (`e_gammeabsorption.cpp:47, 79`).
    let band_rows =
        element_kids(rows).filter(|c| c.has_tag_name("p") && element_type(*c) == Some(eid::ROW));
    for row in band_rows {
        let Some(freq) = row
            .attribute("name")
            .and_then(|f| f.trim().parse::<u32>().ok())
        else {
            continue;
        };
        let b = bands
            .index_of(freq)
            .ok_or_else(|| ImportError::unsupported(FMT_XML, format!("{what}: band {freq} Hz")))?;
        let w = format!("{what} at {freq} Hz");
        alpha[b] = Some(F64::new(prop_real(row, "alpha", &w)?));
        lambda[b] = Some(F64::new(prop_real(row, "lambda", &w)?));
        let code = match prop(row, "loi_diff") {
            None => {
                missing.push(freq);
                0
            }
            Some(list) if replacing && !upstream_list_format(list) => {
                let stored = opt_prop_choice(row, "loi_diff", &w).unwrap_or(None);
                if let Some(s) = stored.filter(|&s| s != 0) {
                    reset.push((freq, s));
                }
                0
            }
            Some(_) => {
                replacing = false;
                opt_prop_choice(row, "loi_diff", &w)?.ok_or_else(|| {
                    ImportError::invalid(FMT_XML, format!("{w}: `loi_diff` has no choice"))
                })?
            }
        };
        law[b] = Some(
            u8::try_from(code)
                .ok()
                .and_then(DiffusionLaw::from_solver_code)
                .ok_or_else(|| {
                    ImportError::refused(
                        FMT_XML,
                        codes::DIFFUSION_LAW_OUT_OF_RANGE,
                        format!(
                            "{w}: diffusion law {code} has no case in SPPS (0 to 2, \
                             coreTypes.h:108-113; CalculationCore.cpp:166-182 leaves the \
                             direction unchanged for it)"
                        ),
                    )
                })?,
        );
    }
    if let Some(b) = (0..n).find(|&b| alpha[b].is_none()) {
        return Err(ImportError::invalid(
            FMT_XML,
            format!("{what} has no value for {} Hz", bands.frequencies_hz[b]),
        ));
    }
    if !reset.is_empty() {
        notes.push(format!(
            "{what}: the stored diffusion law {reset:?} (Hz, law) is reset to 0, as upstream's \
             loader resets it (e_gammeabsorption.cpp:43-59)"
        ));
    }
    if !missing.is_empty() {
        notes.push(format!(
            "{what}: no diffusion law at {missing:?} Hz; 0 is written, the value SPPS reads for \
             the attribute upstream leaves out"
        ));
    }
    Ok(ZoneBands {
        absorption: alpha.into_iter().flatten().collect(),
        mean_free_path_m: lambda.into_iter().flatten().collect(),
        diffusion_law: law.into_iter().flatten().collect(),
    })
}

/// Whether a `loi_diff` list is in the format upstream's loader keeps: read as `E_Data_List`
/// reads it (`e_data_list.h:52-82`: only with an `nb` attribute, entries in document order, a
/// repeated id skipped), at least two entries, the first labelled "Uniform reflection"
/// (`e_gammeabsorption.cpp:50-51`).
fn upstream_list_format(list: Node<'_, '_>) -> bool {
    if !list.has_attribute("nb") {
        return false;
    }
    let mut ids: Vec<i64> = Vec::new();
    let mut labels: Vec<&str> = Vec::new();
    for e in element_kids(list).filter(|c| c.has_tag_name("enumeration")) {
        let (Some(id), Some(value)) = (
            e.attribute("id").and_then(|v| v.trim().parse::<i64>().ok()),
            e.attribute("value"),
        ) else {
            continue;
        };
        if !ids.contains(&id) {
            ids.push(id);
            labels.push(value);
        }
    }
    labels.len() >= 2 && labels[0] == "Uniform reflection"
}

// ---------------------------------------------------------------------------------------------
// sceneMesh.bin and .finfo.

/// A scene mesh as `sceneMesh.bin` holds it.
#[derive(Clone, Debug, PartialEq)]
pub struct SceneMesh {
    pub version: (u32, u32),
    /// World metres, Z up, as stored.
    pub vertices: Vec<[f32; 3]>,
    pub groups: Vec<SceneGroup>,
    /// The face record size found from the node spans; `None` if no group has a face.
    pub face_record_bytes: Option<usize>,
}

/// A mesh group of `sceneMesh.bin`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SceneGroup {
    pub name: String,
    pub faces: Vec<[u32; 3]>,
}

fn u16_at(b: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes(
        b.get(at..at.checked_add(2)?)?.try_into().ok()?,
    ))
}

fn u32_at(b: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(
        b.get(at..at.checked_add(4)?)?.try_into().ok()?,
    ))
}

/// Reads `sceneMesh.bin`. See the module docs for the layout and the checks.
pub fn read_scene_mesh(bytes: &[u8]) -> Result<SceneMesh> {
    let major = u32_at(bytes, 0).ok_or_else(|| ImportError::truncated(FMT_MESH, "the header"))?;
    let minor = u32_at(bytes, 4).ok_or_else(|| ImportError::truncated(FMT_MESH, "the header"))?;
    let record = match (major, minor) {
        (1, 0) => 20usize,
        (1, 1) | (1, 2) => 32,
        _ => {
            return Err(ImportError::unsupported(
                FMT_MESH,
                format!("version {major}.{minor}; versions 1.0 to 1.2 are read"),
            ));
        }
    };
    let mut vertices: Option<Vec<[f32; 3]>> = None;
    let mut groups = Vec::new();
    let mut face_record_bytes = None;
    let mut off = 8usize;
    loop {
        let node_type = u16_at(bytes, off)
            .ok_or_else(|| ImportError::truncated(FMT_MESH, format!("the node at {off}")))?;
        let next = u32_at(bytes, off + 8)
            .ok_or_else(|| ImportError::truncated(FMT_MESH, format!("the node at {off}")))?
            as usize;
        let end = if next == 0 { bytes.len() } else { next };
        if (next != 0 && end <= off + 12) || end > bytes.len() {
            return Err(ImportError::invalid(
                FMT_MESH,
                format!("the node at {off} points its next node to {next}"),
            ));
        }
        let payload = off + 12;
        match node_type {
            0 => {
                if vertices.is_some() {
                    return Err(ImportError::invalid(FMT_MESH, "two vertex nodes"));
                }
                let count = u32_at(bytes, payload)
                    .ok_or_else(|| ImportError::truncated(FMT_MESH, "the vertex count"))?
                    as usize;
                if count
                    .checked_mul(12)
                    .and_then(|b| b.checked_add(4 + payload))
                    .is_none_or(|b| b > end)
                {
                    return Err(ImportError::truncated(
                        FMT_MESH,
                        format!("the {count} vertices"),
                    ));
                }
                let mut vs = Vec::with_capacity(count);
                for i in 0..count {
                    let at = payload + 4 + 12 * i;
                    let f = |k: usize| {
                        f32::from_le_bytes(bytes[at + 4 * k..at + 4 * k + 4].try_into().unwrap())
                    };
                    vs.push([f(0), f(1), f(2)]);
                }
                vertices = Some(vs);
            }
            1 => {
                let name_bytes = bytes
                    .get(payload..payload + 255)
                    .ok_or_else(|| ImportError::truncated(FMT_MESH, "a group name"))?;
                let name_end = name_bytes.iter().position(|&b| b == 0).unwrap_or(255);
                let name = String::from_utf8_lossy(&name_bytes[..name_end]).into_owned();
                let count = u32_at(bytes, payload + 256)
                    .ok_or_else(|| ImportError::truncated(FMT_MESH, "a group's face count"))?
                    as usize;
                let first = payload + 260;
                let span = end
                    .checked_sub(first)
                    .ok_or_else(|| ImportError::truncated(FMT_MESH, format!("group `{name}`")))?;
                let mut faces = Vec::new();
                if count > 0 {
                    if span % count != 0 || span / count != record {
                        return Err(ImportError::invalid(
                            FMT_MESH,
                            format!(
                                "group `{name}`: {span} bytes for {count} faces is not the \
                                 {record}-byte face record of version {major}.{minor}"
                            ),
                        ));
                    }
                    face_record_bytes = Some(record);
                    faces.reserve(count);
                    for i in 0..count {
                        let at = first + record * i;
                        let v = |k: usize| u32_at(bytes, at + 4 * k).unwrap_or(u32::MAX);
                        faces.push([v(0), v(1), v(2)]);
                    }
                }
                groups.push(SceneGroup { name, faces });
            }
            2..=4 => {}
            other => {
                return Err(ImportError::invalid(
                    FMT_MESH,
                    format!("unknown node type {other} at {off}"),
                ));
            }
        }
        if next == 0 {
            break;
        }
        off = next;
    }
    let vertices = vertices.ok_or_else(|| ImportError::invalid(FMT_MESH, "no vertex node"))?;
    let n = vertices.len();
    for g in &groups {
        for (i, f) in g.faces.iter().enumerate() {
            if f.iter().any(|&v| v as usize >= n) {
                return Err(ImportError::invalid(
                    FMT_MESH,
                    format!(
                        "group `{}` face {i} uses vertices {f:?}, but there are {n}",
                        g.name
                    ),
                ));
            }
        }
    }
    for v in &vertices {
        if !v.iter().all(|c| c.is_finite()) {
            return Err(ImportError::invalid(
                FMT_MESH,
                format!("a vertex has a non-finite coordinate: {v:?}"),
            ));
        }
    }
    Ok(SceneMesh {
        version: (major, minor),
        vertices,
        groups,
        face_record_bytes,
    })
}

/// One `.finfo` entry: a face of a mesh group.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct FaceRef {
    /// The mesh group's index in `sceneMesh.bin`.
    pub group: i32,
    /// The face's index in that group.
    pub face: i32,
}

/// Reads a `.finfo` face list, written on Windows (4-byte count) or on Linux or macOS (8-byte).
pub fn read_finfo(bytes: &[u8]) -> Result<Vec<FaceRef>> {
    let (header, count) = match bytes.len() % 8 {
        4 => (
            4usize,
            u64::from(
                u32_at(bytes, 0).ok_or_else(|| ImportError::truncated(FMT_FINFO, "the count"))?,
            ),
        ),
        0 if bytes.len() >= 8 => (
            8,
            u64::from_le_bytes(bytes[..8].try_into().expect("8 bytes")),
        ),
        _ => {
            return Err(ImportError::invalid(
                FMT_FINFO,
                format!(
                    "{} bytes is neither 4 + 8n (written on Windows) nor 8 + 8n (Linux, macOS)",
                    bytes.len()
                ),
            ));
        }
    };
    let n = (bytes.len() - header) / 8;
    if count != n as u64 {
        return Err(ImportError::invalid(
            FMT_FINFO,
            format!("the count says {count} faces, the file holds {n}"),
        ));
    }
    Ok((0..n)
        .map(|i| {
            let at = header + 8 * i;
            let g = i32::from_le_bytes(bytes[at..at + 4].try_into().expect("4 bytes"));
            let f = i32::from_le_bytes(bytes[at + 4..at + 8].try_into().expect("4 bytes"));
            FaceRef { group: g, face: f }
        })
        .collect())
}

/// The faces a `<gr facesFile=...>` lists, as project face indices, with the file's bytes;
/// `None` if the archive has no such file. Upstream reads a missing face list as an empty group
/// (`E_Scene_Groupesurfaces_Groupe::sendStringToVector`).
fn finfo_faces(
    archive: &Archive<'_>,
    folder: &str,
    gr: Node<'_, '_>,
    mesh: &SceneMesh,
    group_start: &[usize],
    what: &str,
) -> Result<Option<(Vec<usize>, Vec<u8>)>> {
    let file = gr
        .attribute("facesFile")
        .ok_or_else(|| ImportError::invalid(FMT_XML, format!("{what} has no facesFile")))?;
    let file = plain_file_name(file)?;
    let bytes = match archive.find(&format!("{folder}{file}"))? {
        Some(e) => archive.read_entry(e)?,
        None => return Ok(None),
    };
    let refs = read_finfo(&bytes)?;
    let mut out = Vec::with_capacity(refs.len());
    for r in refs {
        let g = usize::try_from(r.group)
            .ok()
            .filter(|&g| g < mesh.groups.len())
            .ok_or_else(|| {
                ImportError::invalid(
                    FMT_FINFO,
                    format!(
                        "{what}: {file} names mesh group {}, but there are {}",
                        r.group,
                        mesh.groups.len()
                    ),
                )
            })?;
        let f = usize::try_from(r.face)
            .ok()
            .filter(|&f| f < mesh.groups[g].faces.len())
            .ok_or_else(|| {
                ImportError::invalid(
                    FMT_FINFO,
                    format!(
                        "{what}: {file} names face {} of mesh group {g}, which has {}",
                        r.face,
                        mesh.groups[g].faces.len()
                    ),
                )
            })?;
        out.push(group_start[g] + f);
    }
    Ok(Some((out, bytes)))
}

/// A file name the project refers to, which must name a file inside the project folder.
fn plain_file_name(name: &str) -> Result<&str> {
    if name.is_empty() || name.contains(['/', '\\', ':']) || name == ".." || name == "." {
        return Err(ImportError::invalid(
            FMT_XML,
            format!("`{name}` is not a plain file name"),
        ));
    }
    Ok(name)
}

// ---------------------------------------------------------------------------------------------
// The element tree, in upstream's load order.

/// `node`'s children in the order upstream's GUI loads them, which is not always the file's.
///
/// Upstream builds every element of a project from its node, and the constructor first re-orders
/// the node's children by their `wxid` (`SortChildrensByProperty(nodeElement, "wxid")`,
/// `data_manager/element.cpp:158-159`), before any loader walks them (the source list's,
/// `e_scene_sources.h:68-89`; the fitting zones', `e_scene_encombrements.h:56-73`; every other).
/// The file's order differs in a file saved in the session that created its elements: upstream
/// saves an element that has no node yet as a new node, which `wxXmlNode`'s parent constructor
/// makes its parent's first child (`GenericSaveXmlDoc`, `element.cpp:610-613`). Tutorial 1's SPPS
/// run keeps such a `projet_config.xml`: it lists `Receiver 2` (wxid 3669) before `Receiver 1`
/// (3510), upstream loads `Receiver 1` first, and the run's `config.xml`, written from what it
/// loaded, lists 3669 first, as this import and the writer do (`import_proj_with_config`).
///
/// The children are the nodes `wxXmlDocument` keeps with the flags upstream loads with (none,
/// `data_manager/projet.cpp:1860`): whitespace-only text is dropped (spaces, tabs, line breaks);
/// elements, other text and comments are kept, and only elements carry a `wxid`. Their order is
/// [`upstream_sort`]'s. Children read by name (a property, a position) are the first of that name
/// in this order.
fn kids<'a, 'i>(node: Node<'a, 'i>) -> Vec<Node<'a, 'i>> {
    let white = |t: &str| t.chars().all(|c| matches!(c, ' ' | '\t' | '\n' | '\r'));
    let mut out: Vec<Node<'a, 'i>> = node
        .children()
        .filter(|c| {
            c.is_element() || c.is_comment() || (c.is_text() && !white(c.text().unwrap_or("")))
        })
        .collect();
    upstream_sort(&mut out, |n| n.attribute("wxid"));
    out
}

/// The element children of `node`, in upstream's load order ([`kids`]).
fn element_kids<'a, 'i>(node: Node<'a, 'i>) -> impl Iterator<Item = Node<'a, 'i>> {
    kids(node).into_iter().filter(|c| c.is_element())
}

/// Every element below `node` (not `node` itself), depth first, each level in upstream's load
/// order ([`kids`]). Walked with an explicit stack, so no depth can overflow the call stack.
fn preorder<'a, 'i>(node: Node<'a, 'i>) -> Vec<Node<'a, 'i>> {
    let mut out = Vec::new();
    let mut stack: Vec<Node<'a, 'i>> = element_kids(node).collect();
    stack.reverse();
    while let Some(n) = stack.pop() {
        out.push(n);
        let mut below: Vec<Node<'a, 'i>> = element_kids(n).collect();
        below.reverse();
        stack.extend(below);
    }
    out
}

/// Upstream's `SortChildrensByProperty(node, "wxid")` (`data_manager/element.cpp:64-106`) on
/// `nodes`, each node's `wxid` given by `wxid`, with its quirks:
/// - A pass compares every node, in turn, with the id the **first** node had when the pass began,
///   never with its neighbour's (after a swap, `nextid = currentid; currentid = nextid;` leaves
///   `currentid` as it was, `:97-98`), and moves every node whose id is smaller one place towards
///   the front. Passes repeat until one moves nothing. The first node then holds the smallest id,
///   but the others need not be in order: `[2, 5, 4]` is left as it is.
/// - Ids are read with `wxString::ToLong` ([`to_long`]) from one buffer. A node without a `wxid`
///   leaves that buffer as it was (`wxXmlNode::GetAttribute` does not touch its output then), so
///   it counts with the last id read, the previous pass's included.
/// - Nothing is sorted when the first node's id does not read.
///
/// The loop ends. The first node always has an id that reads: a node without one, or with one
/// that does not read, never compares below it. Each pass moves every node whose own id is below
/// the first's one place forward, so one of them becomes first within `nodes.len()` passes, and
/// the first id then falls, which it can do only finitely often.
fn upstream_sort<T>(nodes: &mut [T], wxid: impl Fn(&T) -> Option<&str>) {
    if nodes.is_empty() {
        return;
    }
    let mut buffer = String::new();
    loop {
        let mut moved = false;
        if let Some(v) = wxid(&nodes[0]) {
            buffer = v.to_string();
        }
        if let Some(current) = to_long(&buffer) {
            for i in 0..nodes.len() - 1 {
                if let Some(v) = wxid(&nodes[i + 1]) {
                    buffer = v.to_string();
                }
                if to_long(&buffer).is_some_and(|next| next < current) {
                    nodes.swap(i, i + 1);
                    moved = true;
                }
            }
        }
        if !moved {
            return;
        }
    }
}

/// The integer `strtol` reads at the start of `s` in base 10, as upstream's `Convertor::ToInt`
/// returns it (`manager/sppsString.cpp:107-112`: `wxString::ToLong` stores what it read even when
/// more follows, `2.5` giving 2). `None` where it reads no digit or overflows a 32-bit `long`:
/// `ToLong` then leaves `ToInt`'s variable uninitialised.
fn strtol_prefix(s: &str) -> Option<i64> {
    let t = s.trim_start_matches([' ', '\t', '\n', '\r', '\u{b}', '\u{c}']);
    let sign = usize::from(matches!(t.as_bytes().first(), Some(b'-' | b'+')));
    let digits = t[sign..].bytes().take_while(u8::is_ascii_digit).count();
    if digits == 0 {
        return None;
    }
    to_long(&t[..sign + digits])
}

/// `wxString::ToLong` in base 10, as a test: `Some` when the whole string reads as a C `long`.
/// It reads with `strtol`, which skips leading white space and takes a sign, and it fails when no
/// digit is read, on overflow (`long` is 32 bits in upstream's Windows build) and when anything
/// follows the digits.
fn to_long(s: &str) -> Option<i64> {
    let t = s.trim_start_matches([' ', '\t', '\n', '\r', '\u{b}', '\u{c}']);
    let (negative, digits) = match t.as_bytes().first() {
        Some(b'-') => (true, &t[1..]),
        Some(b'+') => (false, &t[1..]),
        _ => (false, t),
    };
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let magnitude: i64 = digits.parse().ok()?;
    let v = if negative { -magnitude } else { magnitude };
    (i64::from(i32::MIN)..=i64::from(i32::MAX))
        .contains(&v)
        .then_some(v)
}

fn opt_child<'a, 'i>(node: Node<'a, 'i>, name: &str) -> Option<Node<'a, 'i>> {
    element_kids(node).find(|c| c.has_tag_name(name))
}

fn child<'a, 'i>(node: Node<'a, 'i>, name: &str) -> Result<Node<'a, 'i>> {
    opt_child(node, name).ok_or_else(|| {
        ImportError::invalid(
            FMT_XML,
            format!("<{}> has no <{name}>", node.tag_name().name()),
        )
    })
}

/// The property `<p name="...">` directly under `node`.
fn prop<'a, 'i>(node: Node<'a, 'i>, name: &str) -> Option<Node<'a, 'i>> {
    element_kids(node).find(|c| c.has_tag_name("p") && c.attribute("name") == Some(name))
}

/// A number as upstream's GUI writes it: C locale, or with a decimal comma in files written
/// under a French locale (tutorial 3).
fn number(text: &str) -> Option<f64> {
    let t = text.trim();
    let v = if t.contains(',') && !t.contains('.') {
        t.replace(',', ".").parse::<f64>().ok()?
    } else {
        t.parse::<f64>().ok()?
    };
    v.is_finite().then_some(v)
}

fn attr_value<'a>(p: Node<'a, '_>, attr: &str, what: &str, name: &str) -> Result<&'a str> {
    p.attribute(attr).ok_or_else(|| {
        ImportError::invalid(FMT_XML, format!("{what}: property `{name}` has no {attr}"))
    })
}

fn opt_prop_real(node: Node<'_, '_>, name: &str, what: &str) -> Result<Option<f64>> {
    let Some(p) = prop(node, name) else {
        return Ok(None);
    };
    let text = attr_value(p, "value", what, name)?;
    let v = number(text).ok_or_else(|| {
        ImportError::invalid(
            FMT_XML,
            format!("{what}: `{name}` = `{text}` is not a number"),
        )
    })?;
    // The GUI holds a float.
    Ok(Some(widen_f32(v as f32)))
}

fn prop_real(node: Node<'_, '_>, name: &str, what: &str) -> Result<f64> {
    opt_prop_real(node, name, what)?
        .ok_or_else(|| ImportError::invalid(FMT_XML, format!("{what} has no property `{name}`")))
}

fn opt_prop_int(node: Node<'_, '_>, name: &str, what: &str) -> Result<Option<i64>> {
    let Some(p) = prop(node, name) else {
        return Ok(None);
    };
    let text = p
        .attribute("value")
        .or_else(|| p.attribute("choice"))
        .ok_or_else(|| {
            ImportError::invalid(FMT_XML, format!("{what}: property `{name}` has no value"))
        })?;
    let t = text.trim();
    let v = t.parse::<i64>().ok().or_else(|| {
        number(t)
            .filter(|v| v.fract() == 0.0 && v.abs() < 9.0e15)
            .map(|v| v as i64)
    });
    v.map(Some).ok_or_else(|| {
        ImportError::invalid(
            FMT_XML,
            format!("{what}: `{name}` = `{text}` is not an integer"),
        )
    })
}

fn prop_int(node: Node<'_, '_>, name: &str, what: &str) -> Result<i64> {
    opt_prop_int(node, name, what)?
        .ok_or_else(|| ImportError::invalid(FMT_XML, format!("{what} has no property `{name}`")))
}

fn opt_prop_bool(node: Node<'_, '_>, name: &str, what: &str) -> Result<Option<bool>> {
    Ok(opt_prop_int(node, name, what)?.map(|v| v != 0))
}

/// A list property's chosen id (`@choice`).
fn opt_prop_choice(node: Node<'_, '_>, name: &str, what: &str) -> Result<Option<i64>> {
    let Some(p) = prop(node, name) else {
        return Ok(None);
    };
    let text = attr_value(p, "choice", what, name)?;
    text.trim().parse::<i64>().map(Some).map_err(|_| {
        ImportError::invalid(
            FMT_XML,
            format!("{what}: `{name}` choice `{text}` is not an integer"),
        )
    })
}

fn opt_prop_text<'a>(node: Node<'a, '_>, name: &str) -> Option<&'a str> {
    prop(node, name).and_then(|p| p.attribute("textValue"))
}

/// `<position name="...">` with `x`, `y`, `z` properties.
fn position(node: Node<'_, '_>, name: &str, what: &str) -> Result<Vec3> {
    let pos = element_kids(node)
        .find(|c| c.has_tag_name("position") && c.attribute("name") == Some(name))
        .ok_or_else(|| ImportError::invalid(FMT_XML, format!("{what} has no position `{name}`")))?;
    let w = format!("{what} position `{name}`");
    Ok(Vec3::new(
        prop_real(pos, "x", &w)?,
        prop_real(pos, "y", &w)?,
        prop_real(pos, "z", &w)?,
    ))
}

// ---------------------------------------------------------------------------------------------
// Materials, spectra, sources, receivers.

fn read_material(m: Node<'_, '_>, bands: &BandSet, id: MaterialId) -> Result<Material> {
    let idmat = m
        .attribute("idmateriau")
        .and_then(|v| v.trim().parse::<u32>().ok());
    let idmat = idmat.ok_or_else(|| {
        ImportError::invalid(
            FMT_XML,
            "a material of the project database has no idmateriau",
        )
    })?;
    let owner = m.parent_element();
    let name = owner
        .and_then(|o| o.attribute("name"))
        .unwrap_or("")
        .to_string();
    let what = format!("material {idmat} `{name}`");
    let n = bands.len();
    let mut absorption = vec![None; n];
    let mut scattering = vec![None; n];
    let mut law = vec![None; n];
    let mut transmission = vec![None; n];
    // A band's row is `<p>` since 1.3.4 and `<bfreq>`, its values as attributes, before (Industrial,
    // 1.1.5); upstream's loader takes either by its element type (`e_data_row_materiau.h:55-84`).
    let rows = element_kids(m).filter(|c| {
        c.has_tag_name("p")
            || (c.has_tag_name("bfreq") && element_type(*c) == Some(eid::ROW_MATERIAU))
    });
    for row in rows {
        let Some(freq) = row
            .attribute("name")
            .and_then(|f| f.trim().parse::<u32>().ok())
        else {
            continue;
        };
        let b = bands
            .index_of(freq)
            .ok_or_else(|| ImportError::unsupported(FMT_XML, format!("{what}: band {freq} Hz")))?;
        let w = format!("{what} at {freq} Hz");
        if row.has_attribute("absorb") {
            // Projects before 1.3.4: attributes on the row, read as upstream's loader reads them
            // (`e_data_row_materiau.h:58-83`): the band transmits iff it has `affaiblissement`;
            // the numbers go through `StringToFloat`, which reads a missing or unreadable one as
            // 0 and reports an error (`e_data.h:212-252`); `loi` through `Convertor::ToInt`,
            // which returns the digits `strtol` reads first, and an uninitialised value when it
            // reads none (`sppsString.cpp:107-112`). What upstream reports or leaves undefined is
            // refused ([`codes::MATERIAL_ROW_UNREADABLE`]).
            let unreadable = |a: &str, why: String| {
                ImportError::refused(
                    FMT_XML,
                    codes::MATERIAL_ROW_UNREADABLE,
                    format!("{w}: `{a}` {why} (e_data_row_materiau.h:58-83)"),
                )
            };
            let real = |a: &str| -> Result<Option<f64>> {
                match row.attribute(a) {
                    None => Ok(None),
                    Some(t) => number(t).map(|v| Some(widen_f32(v as f32))).ok_or_else(|| {
                        unreadable(
                            a,
                            format!(
                                "= `{t}` is not a number: upstream's GUI reports \
                                 \"Cannot convert string\" and reads 0 (e_data.h:247-250)"
                            ),
                        )
                    }),
                }
            };
            let needed = |a: &str| -> Result<f64> {
                real(a)?.ok_or_else(|| {
                    unreadable(
                        a,
                        "is missing: upstream's GUI reports \"Cannot convert string\" for the \
                         empty text and reads 0 (e_data.h:247-250)"
                            .to_string(),
                    )
                })
            };
            absorption[b] = Some(needed("absorb")?);
            scattering[b] = Some(needed("diffusion")?);
            let loi = row.attribute("loi").unwrap_or("");
            law[b] = Some(strtol_prefix(loi).ok_or_else(|| {
                unreadable(
                    "loi",
                    format!(
                        "= `{loi}` holds no integer: upstream's Convertor::ToInt returns an \
                         uninitialised value for it (sppsString.cpp:107-112)"
                    ),
                )
            })?);
            transmission[b] = Some(real("affaiblissement")?);
        } else {
            absorption[b] = Some(prop_real(row, "absorb", &w)?);
            scattering[b] = Some(prop_real(row, "diffusion", &w)?);
            law[b] = Some(opt_prop_choice(row, "loi", &w)?.unwrap_or(0));
            let on = opt_prop_bool(row, "transmission", &w)?.unwrap_or(false);
            transmission[b] = Some(if on {
                Some(prop_real(row, "affaiblissement", &w)?)
            } else {
                None
            });
        }
    }
    fn missing<T>(v: &[Option<T>]) -> Option<usize> {
        v.iter().position(Option::is_none)
    }
    if let Some(b) = missing(&absorption)
        .or_else(|| missing(&scattering))
        .or_else(|| missing(&law))
    {
        return Err(ImportError::invalid(
            FMT_XML,
            format!("{what} has no value for {} Hz", bands.frequencies_hz[b]),
        ));
    }
    // One law per band, as each row holds its own and upstream writes it per band
    // (`e_data_row_materiau.h:96-107, 218`): tutorial 3's material 100 is Lambert in its six
    // octave bands and specular in the others.
    let mut laws = Vec::with_capacity(n);
    for (b, code) in law.into_iter().flatten().enumerate() {
        let known = u8::try_from(code)
            .ok()
            .and_then(ReflectionLaw::from_solver_code)
            .ok_or_else(|| {
                ImportError::refused(
                    FMT_XML,
                    codes::REFLECTION_LAW_OUT_OF_RANGE,
                    format!(
                        "{what} at {} Hz: reflection law {code} is none of upstream's seven, 0 to \
                         6 (appconfig.cpp:110-117)",
                        bands.frequencies_hz[b]
                    ),
                )
            })?;
        laws.push(known);
    }
    let reflection_law = ReflectionLaws::from_bands(laws);
    // Per band: a loss where the band's `transmission` switch is on, none where it is off.
    let tl: Vec<Option<F64>> = transmission
        .into_iter()
        .map(|t| t.flatten().map(F64::new))
        .collect();
    // A loss this crate's writer would not write as stored: upstream writes a loaded value
    // unchanged (`e_data_row_materiau.h:98-107`) and enforces tau <= alpha only when the user
    // edits the band (`:109-205`); the writer enforces it at every write.
    for (b, loss) in tl.iter().enumerate() {
        let (Some(loss), Some(alpha)) = (loss, absorption[b]) else {
            continue;
        };
        if crate::config_xml::transmission_loss_written(loss.get(), alpha) != Some(loss.get()) {
            return Err(ImportError::refused(
                FMT_XML,
                codes::TRANSMISSION_EXCEEDS_ABSORPTION,
                format!(
                    "{what} at {} Hz transmits with a loss of {} dB against an absorption of \
                     {alpha}: 10^(-R/10) is above the absorption, or the absorption is 0. \
                     Upstream writes the stored loss as it is (e_data_row_materiau.h:98-107) \
                     and prevents this only when the band is edited (:109-205); this crate's \
                     writer would change it",
                    bands.frequencies_hz[b],
                    loss.get()
                ),
            ));
        }
    }
    let transmission_loss_db = tl.iter().any(Option::is_some).then_some(tl);
    let props = owner.and_then(|o| opt_child(o, "property"));
    let double_sided = match props {
        Some(p) => opt_prop_choice(p, "side_material", &what)?.is_none_or(|c| c == 1),
        None => true,
    };
    let color = owner
        .and_then(|o| opt_child(o, "matcolor"))
        .and_then(|c| prop(c, "mat_color"))
        .map(|p| {
            let ch = |a: &str| {
                p.attribute(a)
                    .and_then(|v| v.trim().parse::<u8>().ok())
                    .unwrap_or(128)
            };
            Rgb(ch("r"), ch("g"), ch("b"))
        })
        .unwrap_or(Rgb(128, 128, 128));
    Ok(Material {
        id,
        name: if name.is_empty() {
            format!("Material {idmat}")
        } else {
            name
        },
        color,
        absorption: absorption
            .into_iter()
            .map(|v| F64::new(v.unwrap_or(0.0)))
            .collect(),
        scattering: scattering
            .into_iter()
            .map(|v| F64::new(v.unwrap_or(0.0)))
            .collect(),
        reflection_law,
        transmission_loss_db,
        double_sided,
        solver_id: Some(idmat),
    })
}

/// A `<spectre>`, the way upstream writes it (see the module docs).
fn read_spectrum(sp: Node<'_, '_>, bands: &BandSet, what: &str) -> Result<Spectrum> {
    let id = sp
        .attribute("idspectre")
        .and_then(|v| v.trim().parse::<i64>().ok());
    let ty = sp
        .attribute("typespectre")
        .and_then(|v| v.trim().parse::<i64>().ok());
    let (Some(id), Some(ty)) = (id, ty) else {
        return Err(ImportError::invalid(
            FMT_XML,
            format!("{what}: the spectrum has no idspectre or typespectre"),
        ));
    };
    let n = bands.len();
    // The chosen spectrum's band levels, in dB.
    let reference: Vec<f64> = if ty == APP_SPECTRUM_TYPE {
        let r = u32::try_from(id)
            .ok()
            .and_then(reference_spectrum)
            .ok_or_else(|| {
                ImportError::unsupported(
                    FMT_XML,
                    format!("{what}: reference spectrum {id} does not exist upstream"),
                )
            })?;
        r.band_db.iter().map(|&v| widen_f32(v)).collect()
    } else {
        user_spectrum(sp, id, ty, bands).ok_or_else(|| {
            ImportError::unsupported(
                FMT_XML,
                format!("{what}: user spectrum {id} (type {ty}) is not in the project database"),
            )
        })??
    };
    let rows: Vec<Node> = element_kids(sp).filter(|c| c.has_tag_name("p")).collect();
    let cumul = rows
        .iter()
        .find(|r| r.attribute("name") == Some("cumul"))
        .ok_or_else(|| {
            ImportError::invalid(FMT_XML, format!("{what}: the spectrum has no global row"))
        })?;
    let lw = prop_real(*cumul, "lw", &format!("{what} global level"))?;
    let mut att = vec![0.0f64; n];
    for r in &rows {
        let Some(freq) = r
            .attribute("name")
            .and_then(|f| f.trim().parse::<u32>().ok())
        else {
            continue;
        };
        if let Some(b) = bands.index_of(freq) {
            att[b] = opt_prop_real(*r, "att", &format!("{what} at {freq} Hz"))?.unwrap_or(0.0);
        }
    }
    let plain = att.iter().all(|&a| a == 0.0);
    if plain && ty == APP_SPECTRUM_TYPE && id == 0 {
        return Ok(Spectrum::new(lw, SpectrumShape::White));
    }
    if plain && ty == APP_SPECTRUM_TYPE && id == 1 {
        return Ok(Spectrum::new(lw, SpectrumShape::Pink));
    }
    let relative: Vec<f64> = reference.iter().zip(&att).map(|(r, a)| r - a).collect();
    let sum = |v: &[f64]| 10.0 * v.iter().map(|x| 10f64.powf(x / 10.0)).sum::<f64>().log10();
    let global = lw + sum(&relative) - sum(&reference);
    if !global.is_finite() {
        return Err(ImportError::invalid(
            FMT_XML,
            format!("{what}: the spectrum's global level is not finite"),
        ));
    }
    Ok(Spectrum::new(
        global,
        SpectrumShape::Custom {
            relative_db: relative.into_iter().map(F64::new).collect(),
        },
    ))
}

/// A user spectrum of the project database: the `frequences` element with this `idspectre` and
/// element type `typespectre`. `None` if there is none; its band levels (`db`) otherwise.
fn user_spectrum(sp: Node<'_, '_>, id: i64, ty: i64, bands: &BandSet) -> Option<Result<Vec<f64>>> {
    let root = sp.document().root_element();
    let bdd = preorder(root).into_iter().find(|d| d.has_tag_name("bdd"))?;
    let el = preorder(bdd).into_iter().find(|d| {
        d.attribute("idspectre")
            .and_then(|v| v.trim().parse::<i64>().ok())
            == Some(id)
            && d.attribute("eid")
                .and_then(|v| v.trim().parse::<i64>().ok())
                == Some(ty)
    })?;
    let what = format!("user spectrum {id}");
    let mut out = vec![None; bands.len()];
    for r in element_kids(el).filter(|c| c.has_tag_name("p")) {
        let Some(freq) = r
            .attribute("name")
            .and_then(|f| f.trim().parse::<u32>().ok())
        else {
            continue;
        };
        if let Some(b) = bands.index_of(freq) {
            match prop_real(r, "db", &format!("{what} at {freq} Hz")) {
                Ok(v) => out[b] = Some(v),
                Err(e) => return Some(Err(e)),
            }
        }
    }
    if let Some(b) = out.iter().position(Option::is_none) {
        return Some(Err(ImportError::invalid(
            FMT_XML,
            format!("{what} has no level at {} Hz", bands.frequencies_hz[b]),
        )));
    }
    Some(Ok(out.into_iter().map(|v| v.unwrap_or(0.0)).collect()))
}

fn read_source(s: Node<'_, '_>, bands: &BandSet, id: SourceId) -> Result<Source> {
    let name = s.attribute("name").unwrap_or("").to_string();
    let what = format!("source `{name}`");
    let props = child(s, "prop")?;
    let pos = element_kids(s)
        .find(|c| c.has_tag_name("position"))
        .and_then(|c| c.attribute("name"))
        .unwrap_or("pos_source")
        .to_string();
    let position = position(s, &pos, &what)?;
    let direction = || -> Result<Vec3> {
        Ok(Vec3::new(
            prop_real(props, "u", &what)?,
            prop_real(props, "v", &what)?,
            prop_real(props, "w", &what)?,
        ))
    };
    let directivity = match opt_prop_choice(props, "directivite", &what)?.unwrap_or(0) {
        0 => Directivity::Omni,
        1 => Directivity::Unidirectional {
            direction: direction()?,
        },
        2 => Directivity::PlaneXy,
        3 => Directivity::PlaneYz,
        4 => Directivity::PlaneXz,
        5 => {
            return Err(ImportError::unsupported(
                FMT_XML,
                format!(
                    "{what} uses a directivity balloon; upstream's directivity database is not \
                     imported"
                ),
            ));
        }
        other => {
            return Err(ImportError::unsupported(
                FMT_XML,
                format!("{what}: directivity {other}"),
            ));
        }
    };
    let spectre = child(s, "spectre")?;
    Ok(Source {
        id,
        enabled: opt_prop_bool(props, "enable", &what)?.unwrap_or(true),
        position,
        power: read_spectrum(spectre, bands, &what)?,
        directivity,
        delay_s: F64::new(opt_prop_real(props, "delay", &what)?.unwrap_or(0.0)),
        group: None,
        solver_id: pin(element_id(s), &what)?,
        name,
    })
}

fn read_point_receiver(
    r: Node<'_, '_>,
    bands: &BandSet,
    id: PointReceiverId,
) -> Result<PointReceiver> {
    let name = r.attribute("name").unwrap_or("").to_string();
    let what = format!("point receiver `{name}`");
    let props = child(r, "prop")?;
    let background_noise = match opt_child(r, "spectre") {
        Some(sp) => Some(read_spectrum(sp, bands, &what)?),
        None => None,
    };
    Ok(PointReceiver {
        id,
        position: position(r, "pos_recepteur", &what)?,
        orientation: Vec3::new(
            prop_real(props, "u", &what)?,
            prop_real(props, "v", &what)?,
            prop_real(props, "w", &what)?,
        ),
        background_noise,
        solver_id: pin(element_id(r), &what)?,
        name,
    })
}

fn read_environment(project_el: Node<'_, '_>, notes: &mut Vec<String>) -> Result<Environment> {
    let mut env = Environment::default();
    let Some(a) = opt_child(project_el, "atmoconfig") else {
        notes.push("no environment (atmoconfig): upstream's defaults are used".to_string());
        return Ok(env);
    };
    let what = "environment";
    let mut set = |name: &str, field: &mut F64| -> Result<()> {
        match opt_prop_real(a, name, what)? {
            Some(v) => *field = F64::new(v),
            None => notes.push(format!(
                "environment has no `{name}`: upstream's default is used"
            )),
        }
        Ok(())
    };
    set("temperature", &mut env.temperature_c)?;
    set("humidite", &mut env.relative_humidity_percent)?;
    set("pression", &mut env.pressure_pa)?;
    set("z0", &mut env.ground_roughness_m)?;
    set("alog", &mut env.celerity_gradient_log)?;
    set("blin", &mut env.celerity_gradient_lin)?;
    if opt_prop_bool(a, "disable_absatmo_computation", what)?.unwrap_or(false) {
        env.air_absorption = AirAbsorption::UserDefined {
            value: F64::new(prop_real(a, "absatmo", what)?),
            unit: AttenuationUnit::PerMetre,
        };
    }
    Ok(env)
}

/// One core's meshing settings (`mesh_conf`), as upstream's `RunCoreMaillage` reads them
/// (`projet_maillage.cpp:50-58`). A loaded `mesh_conf` gets none of its GUI defaults: its XML
/// constructor never calls `InitProperties` and adds only `debugmode`, off
/// (`e_core_core_tetconf.h:45-54`, reached from `e_core_core.h:89-90`). So a property the file
/// lacks reads as upstream's getters read a missing one, and each is noted: a switch off
/// (`GetBoolConfig`, `element.cpp:1300-1314`), a number 0 (`GetDecimalConfig`, `:1252-1266`), a
/// text empty (`GetStringConfig`, `:1173-1187`). A missing ratio is 0, which upstream gives
/// TetGen as `-pq0`; so is a missing volume bound or area constraint that is switched on. The
/// mesher refuses those (`input_invalid`) rather than mesh otherwise.
///
/// Refused: "Test mesh topology" on ([`codes::MESH_DEBUG_MODE`]); user-defined TetGen parameters,
/// and additional ones other than `-Y` (unsupported).
fn read_mesh_conf(mesh: Node<'_, '_>, mw: &str, notes: &mut Vec<String>) -> Result<MeshSettings> {
    let mut missing = |name: &str, reads: &str| {
        notes.push(format!(
            "{mw}: no `{name}`, read as upstream reads a missing property: {reads} (a loaded \
             mesh_conf gets none of its GUI defaults, e_core_core_tetconf.h:45-54)"
        ));
    };
    match opt_prop_bool(mesh, "debugmode", mw)? {
        Some(true) => {
            return Err(ImportError::refused(
                FMT_XML,
                codes::MESH_DEBUG_MODE,
                format!(
                    "{mw}: \"Test mesh topology\" (`debugmode`) is on. Upstream's GUI then runs \
                     `tetgen -d` alone, skips the scene correction, loads no mesh \
                     (projet_maillage.cpp:165-174, 212, 242-275), and runs the solver on \
                     whatever mesh it already held (projet.cpp:751-769)"
                ),
            ));
        }
        Some(false) => {}
        None => missing(
            "debugmode",
            "off, which upstream's loader also adds (e_core_core_tetconf.h:50-53)",
        ),
    }
    let q = match opt_prop_real(mesh, "minratio", mw)? {
        Some(v) => v,
        None => {
            missing(
                "minratio",
                "0, so upstream gives TetGen -pq0, which the mesher refuses (input_invalid)",
            );
            0.0
        }
    };
    let max_volume_m3 = match opt_prop_bool(mesh, "ismaxvol", mw)? {
        Some(true) => Some(match opt_prop_real(mesh, "maxvol", mw)? {
            Some(v) => v,
            None => {
                missing(
                    "maxvol",
                    "0, so upstream gives TetGen -a0, which the mesher refuses (input_invalid)",
                );
                0.0
            }
        }),
        Some(false) => None,
        None => {
            missing("ismaxvol", "off, no volume bound");
            None
        }
    };
    let surface_receiver_max_area_m2 = match opt_prop_bool(mesh, "isareaconstraint", mw)? {
        Some(true) => Some(match opt_prop_real(mesh, "constraintrecepteurss", mw)? {
            Some(v) => v,
            None => {
                missing(
                    "constraintrecepteurss",
                    "0 m², which the mesher refuses (input_invalid)",
                );
                0.0
            }
        }),
        Some(false) => None,
        None => {
            missing(
                "isareaconstraint",
                "off, no surface-receiver area constraint",
            );
            None
        }
    };
    let user = match opt_prop_text(mesh, "userdefineparams") {
        Some(t) => t.trim(),
        None => {
            missing("userdefineparams", "empty");
            ""
        }
    };
    if !user.is_empty() {
        return Err(ImportError::unsupported(
            FMT_XML,
            format!("{mw}: user-defined TetGen parameters `{user}`"),
        ));
    }
    let append = match opt_prop_text(mesh, "appendparams") {
        Some(t) => t,
        None => {
            missing("appendparams", "empty, so no -Y");
            ""
        }
    };
    let mut preserve_boundary = false;
    for t in append.split_ascii_whitespace() {
        match t {
            "-Y" => preserve_boundary = true,
            other => {
                return Err(ImportError::unsupported(
                    FMT_XML,
                    format!("{mw}: additional TetGen parameter `{other}`"),
                ));
            }
        }
    }
    // "Scene correction before meshing": `projet_maillage.cpp:57, 212` runs `preprocess.exe` on
    // the `.poly` when it is on (and "Test mesh topology" off, which is refused above).
    let preprocess = match opt_prop_bool(mesh, "preprocess", mw)? {
        Some(v) => v,
        None => {
            missing(
                "preprocess",
                "off, so the scene is meshed without preprocess.exe's correction",
            );
            false
        }
    };
    Ok(MeshSettings {
        min_radius_edge_ratio: F64::new(q),
        max_volume_m3: max_volume_m3.map(F64::new),
        surface_receiver_max_area_m2: surface_receiver_max_area_m2.map(F64::new),
        preserve_boundary,
        preprocess,
    })
}

/// How `b` differs from `a`, one entry per setting that differs, `name a / b`.
fn mesh_differences(a: &MeshSettings, b: &MeshSettings) -> Vec<String> {
    let bound = |v: Option<F64>| v.map_or_else(|| "none".to_string(), |x| x.get().to_string());
    let on = |v: bool| if v { "on" } else { "off" };
    let mut out = Vec::new();
    if a.min_radius_edge_ratio != b.min_radius_edge_ratio {
        out.push(format!(
            "radius-edge ratio {} / {}",
            a.min_radius_edge_ratio.get(),
            b.min_radius_edge_ratio.get()
        ));
    }
    if a.max_volume_m3 != b.max_volume_m3 {
        out.push(format!(
            "volume bound {} / {}",
            bound(a.max_volume_m3),
            bound(b.max_volume_m3)
        ));
    }
    if a.surface_receiver_max_area_m2 != b.surface_receiver_max_area_m2 {
        out.push(format!(
            "surface-receiver area constraint {} / {}",
            bound(a.surface_receiver_max_area_m2),
            bound(b.surface_receiver_max_area_m2)
        ));
    }
    if a.preserve_boundary != b.preserve_boundary {
        out.push(format!(
            "-Y {} / {}",
            on(a.preserve_boundary),
            on(b.preserve_boundary)
        ));
    }
    if a.preprocess != b.preprocess {
        out.push(format!(
            "scene correction (preprocess) {} / {}",
            on(a.preprocess),
            on(b.preprocess)
        ));
    }
    out
}

fn band_switches(core: Node<'_, '_>, bands: &BandSet, what: &str) -> Result<Vec<bool>> {
    let conf = child(core, "core_conf_bfreq")?;
    let mut out = vec![None; bands.len()];
    for p in element_kids(conf).filter(|c| c.has_tag_name("p")) {
        let Some(freq) = p
            .attribute("name")
            .and_then(|f| f.trim().parse::<u32>().ok())
        else {
            continue;
        };
        let b = bands
            .index_of(freq)
            .ok_or_else(|| ImportError::unsupported(FMT_XML, format!("{what}: band {freq} Hz")))?;
        out[b] = Some(prop_int(conf, &freq.to_string(), what)? != 0);
    }
    if let Some(b) = out.iter().position(Option::is_none) {
        return Err(ImportError::invalid(
            FMT_XML,
            format!("{what}: no switch for {} Hz", bands.frequencies_hz[b]),
        ));
    }
    Ok(out.into_iter().map(|v| v.unwrap_or(false)).collect())
}

fn read_solvers(
    core: Node<'_, '_>,
    bands: &BandSet,
    notes: &mut Vec<String>,
) -> Result<SolverSettings> {
    let mut s = SolverSettings::for_bands(bands.len());
    let spps = child(core, "spps")?;
    let conf = child(spps, "configuration")?;
    let what = "SPPS settings";
    let sp = &mut s.spps;
    macro_rules! real {
        ($name:literal, $field:expr) => {
            match opt_prop_real(conf, $name, what)? {
                Some(v) => $field = F64::new(v),
                None => notes.push(format!("{what}: no `{}`, upstream's default kept", $name)),
            }
        };
    }
    macro_rules! flag {
        ($name:literal, $field:expr) => {
            match opt_prop_bool(conf, $name, what)? {
                Some(v) => $field = v,
                None => notes.push(format!("{what}: no `{}`, upstream's default kept", $name)),
            }
        };
    }
    macro_rules! count {
        ($name:literal, $field:expr) => {
            match opt_prop_int(conf, $name, what)? {
                Some(v) => {
                    $field = u32::try_from(v).map_err(|_| {
                        ImportError::invalid(FMT_XML, format!("{what}: `{}` = {v}", $name))
                    })?
                }
                None => notes.push(format!("{what}: no `{}`, upstream's default kept", $name)),
            }
        };
    }
    count!("nbparticules", sp.particles_per_source);
    count!("nbparticules_rendu", sp.particles_saved);
    real!("duree_simulation", sp.duration_s);
    real!("pasdetemps", sp.time_step_s);
    count!("random_seed", sp.random_seed);
    flag!("abs_atmo_calc", sp.air_absorption);
    flag!("enc_calc", sp.fittings);
    flag!("direct_calc", sp.direct_field_only);
    flag!("trans_calc", sp.transmission);
    real!("trans_epsilon", sp.extinction_exponent);
    real!("rayon_recepteurp", sp.receiver_radius_m);
    flag!("output_recs_byfreq", sp.sound_maps_per_band);
    flag!("output_recp_bysource", sp.echogram_per_source);
    flag!("save_surface_intersection", sp.save_surface_intersections);
    flag!(
        "save_receivers_intersection",
        sp.save_receiver_intersections
    );
    match opt_prop_choice(conf, "computation_method", what)? {
        Some(0) => sp.method = ComputationMethod::Random,
        Some(1) => sp.method = ComputationMethod::Energetic,
        Some(other) => {
            return Err(ImportError::unsupported(
                FMT_XML,
                format!("{what}: computation method {other}"),
            ));
        }
        None => notes.push(format!(
            "{what}: no `computation_method`, upstream's default kept"
        )),
    }
    match opt_prop_choice(conf, "surf_receiv_method", what)? {
        Some(0) => sp.sound_map = SoundMapQuantity::Intensity,
        Some(1) => sp.sound_map = SoundMapQuantity::Spl,
        Some(other) => {
            return Err(ImportError::unsupported(
                FMT_XML,
                format!("{what}: surface receiver method {other}"),
            ));
        }
        None => notes.push(format!(
            "{what}: no `surf_receiv_method`, upstream's default kept"
        )),
    }
    sp.bands_computed = band_switches(spps, bands, "SPPS bands")?;

    // Meshing. Upstream meshes a run with the settings of the core it runs
    // (`RunCoreMaillage(selectedCore)`, `projet_maillage.cpp:40-59`; each core has its own
    // `mesh_conf`, `e_core_sppscore.h:142`, `e_core_tccore.h:87`). A project holds one set:
    // SPPS's. TCR's is read the same way below, and any difference is noted.
    s.meshing = read_mesh_conf(child(spps, "mesh_conf")?, "SPPS meshing", notes)?;

    // TCR.
    if let Some(tc) = opt_child(core, "tc") {
        let conf = child(tc, "configuration")?;
        s.tcr = TcrSettings {
            air_absorption: opt_prop_bool(conf, "abs_atmo_calc", "TCR settings")?.unwrap_or(true),
            bands_computed: band_switches(tc, bands, "TCR bands")?,
        };
        match opt_child(tc, "mesh_conf") {
            Some(tm) => {
                let tcr_mesh = read_mesh_conf(tm, "TCR meshing", notes)?;
                let differences = mesh_differences(&s.meshing, &tcr_mesh);
                if !differences.is_empty() {
                    notes.push(format!(
                        "TCR's meshing settings differ from SPPS's ({}): upstream meshes a TCR run \
                         with TCR's (projet_maillage.cpp:40-59), and a project holds one set, so \
                         SPPS's are imported and a TCR run here meshes with them",
                        differences.join("; ")
                    ));
                }
            }
            None => notes.push(
                "TCR has no meshing settings (mesh_conf): upstream's GUI then runs TCR on the mesh \
                 it already holds, without meshing (projet_maillage.cpp:45-62); a TCR run here \
                 meshes with SPPS's settings"
                    .to_string(),
            ),
        }
    } else {
        notes.push("no TCR settings: upstream's defaults are used".to_string());
    }
    Ok(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// [`upstream_sort`] on ids given as text (`None`: no `wxid`), as the list comes out.
    fn sorted(ids: &[Option<&str>]) -> Vec<Option<String>> {
        let mut v: Vec<Option<&str>> = ids.to_vec();
        upstream_sort(&mut v, |x| *x);
        v.into_iter().map(|x| x.map(str::to_string)).collect()
    }

    fn some(ids: &[&str]) -> Vec<Option<String>> {
        ids.iter().map(|s| Some(s.to_string())).collect()
    }

    #[test]
    fn upstream_sort_orders_what_upstream_orders() {
        // Tutorial 1's SPPS snapshot: receivers and surface groups saved newest first.
        assert_eq!(
            sorted(&[Some("3669"), Some("3510")]),
            some(&["3510", "3669"])
        );
        assert_eq!(
            sorted(&[Some("3147"), Some("3144"), Some("3141")]),
            some(&["3141", "3144", "3147"])
        );
        // A list already ascending is left as it is.
        assert_eq!(
            sorted(&[Some("1463"), Some("1466"), Some("1469")]),
            some(&["1463", "1466", "1469"])
        );
    }

    #[test]
    fn upstream_sort_keeps_its_quirks() {
        // Once the first holds the smallest id, nothing else moves: not a full sort.
        assert_eq!(
            sorted(&[Some("2"), Some("5"), Some("4")]),
            some(&["2", "5", "4"])
        );
        // Every pass compares with the first id, not the neighbour's: 5 and 4 never meet.
        assert_eq!(
            sorted(&[Some("3"), Some("1"), Some("5"), Some("4")]),
            some(&["1", "3", "5", "4"])
        );
        // No sort when the first id does not read.
        assert_eq!(
            sorted(&[None, Some("5"), Some("1")]),
            vec![None, Some("5".into()), Some("1".into())]
        );
        assert_eq!(
            sorted(&[Some("x"), Some("5"), Some("1")]),
            some(&["x", "5", "1"])
        );
        // A node without a wxid counts with the last id read: here 1, so it moves with it.
        assert_eq!(
            sorted(&[Some("5"), Some("1"), None]),
            vec![Some("1".into()), None, Some("5".into())]
        );
        // One after a larger id counts with that one and stays.
        assert_eq!(
            sorted(&[Some("1"), Some("5"), None]),
            vec![Some("1".into()), Some("5".into()), None]
        );
    }

    #[test]
    fn upstream_sort_ends_on_every_short_list() {
        // Every list of up to 6 nodes with ids 0 to 5, missing or unreadable, in every order:
        // the loop ends, and the first node holds the smallest readable id or the list is as it
        // came (first id unreadable).
        let values = ["0", "1", "2", "3", "4", "5"];
        for n in 1..=6usize {
            let choices = n + 2;
            let total = choices.pow(n as u32);
            for code in 0..total {
                let mut c = code;
                let ids: Vec<Option<&str>> = (0..n)
                    .map(|_| {
                        let k = c % choices;
                        c /= choices;
                        match k {
                            k if k < n => Some(values[k]),
                            k if k == n => None,
                            _ => Some("x"),
                        }
                    })
                    .collect();
                let out = sorted(&ids);
                let first = ids[0].and_then(to_long);
                if first.is_none() {
                    assert_eq!(
                        out,
                        ids.iter()
                            .map(|x| x.map(str::to_string))
                            .collect::<Vec<_>>()
                    );
                    continue;
                }
                let least = ids.iter().filter_map(|x| x.and_then(to_long)).min();
                assert_eq!(
                    out[0].as_deref().and_then(to_long),
                    least,
                    "{ids:?} -> {out:?}"
                );
            }
        }
    }

    #[test]
    fn to_long_reads_as_wxstring_tolong() {
        for (text, v) in [
            ("3669", Some(3669)),
            (" 12", Some(12)),
            ("+7", Some(7)),
            ("-2147483648", Some(-2_147_483_648)),
            ("2147483647", Some(2_147_483_647)),
            ("2147483648", None),
            ("12 ", None),
            ("1.5", None),
            ("", None),
            ("-", None),
            ("0x10", None),
        ] {
            assert_eq!(to_long(text), v, "{text:?}");
        }
    }

    #[test]
    fn strtol_prefix_reads_as_convertor_toint() {
        for (text, v) in [
            ("2", Some(2)),
            ("2.5", Some(2)),
            (" 6", Some(6)),
            ("-1", Some(-1)),
            ("2abc", Some(2)),
            ("", None),
            ("abc", None),
            ("99999999999", None),
        ] {
            assert_eq!(strtol_prefix(text), v, "{text:?}");
        }
    }
}
