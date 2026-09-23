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
//! # Surface groups
//!
//! Each face gets the material group (`Scene/donnees/sgroupes/gr`) whose `.finfo` lists it, and
//! the scene surface receiver (`recepteurss/recepteurs/gr`) whose `.finfo` lists it, if any. Each
//! distinct (material group, receiver) pair becomes one [`SurfaceGroup`], ordered by material group
//! then receiver, carrying the material group's name, with ` / <receiver>` appended only when a
//! receiver splits the material group. A scene receiver covers the groups whose faces it lists.
//! A face that no material group lists gets upstream's default material (reference material 0,
//! `data_manager/appconfig.h:122`) in a group named `(no surface group)`, noted in the report. A
//! face listed by two material groups, or by two receivers, is refused: upstream would let the
//! last one win, silently.
//!
//! # Everything else
//!
//! Read the way upstream's GUI writes it into `config.xml`, values being the `f32` the GUI holds,
//! widened with [`widen_f32`]:
//!
//! - **Materials:** every material of the project's own database (`bdd/materiaux`) with its name,
//!   colour, side and per-band absorption, scattering, law and transmission loss (written only
//!   where `transmission` is on, `e_data_row_materiau.h`), pinned to its `idmateriau`; then each
//!   reference material a group uses, from upstream's reference database (`REFERENCE_MATERIALS`).
//!   A group whose `idmat` is in neither is refused.
//! - **Spectra** (source power, receiver background noise): upstream writes each band as the
//!   chosen spectrum's band level plus the user's global level `Lw` (normalised so the reference
//!   sums to `Lw`), minus the band's attenuation (`E_Property_Freq::LoadLwFromBdd`); the band
//!   levels stored in the file are not used, since the GUI recomputes them at every write
//!   (tutorial 1's receivers store 0 dB per band and write -6.86 dB at 20 kHz). Reference
//!   spectrum `White noise` with no attenuation becomes a white [`Spectrum`] at `Lw`, `Pink
//!   noise` a pink one; anything else a custom one with those relative levels.
//! - **Sources, point receivers** with their `<position>` children, directions, delays and enable
//!   flags; **cutting-plane receivers** from `verta`, `vertb`, `vertc` and `resolution`.
//! - **Environment** from `atmoconfig`; **SPPS** and **TCR** settings and computed bands from
//!   `Core/spps` and `Core/tc`; **meshing** from `Core/spps/mesh_conf`. A property the file lacks
//!   keeps upstream's GUI default, as upstream's GUI does when it loads such a file; each one is
//!   noted in the report.
//!
//! Refused with [`ImportError::Unsupported`], by name: fitting zones and volumes, directivity
//! balloons, user-defined TetGen parameters, and additional TetGen parameters other than `-Y`.

use std::path::Path;

use roxmltree::{Document, Node};

use super::appconst::{reference_material, reference_spectrum};
use super::zip::Archive;
use super::{IdSource, ImportError, Result, default_material, read_bytes, weld_key};
use crate::config_xml::widen_f32;
use crate::schema::{
    AirAbsorption, AttenuationUnit, BandKind, BandSet, ComputationMethod, Directivity, Environment,
    F64, Face, Geometry, GroupId, Material, MaterialId, MeshSettings, PointReceiver,
    PointReceiverId, Project, ProjectId, ReflectionLaw, Rgb, SolverSettings, SoundMapQuantity,
    Source, SourceId, Spectrum, SpectrumShape, SurfaceGroup, SurfaceReceiver, SurfaceReceiverId,
    SurfaceReceiverShape, TcrSettings, Vec3,
};

const FMT_PROJ: &str = "proj";
const FMT_MESH: &str = "sceneMesh.bin";
const FMT_FINFO: &str = "finfo";
const FMT_XML: &str = "projet_config.xml";

/// The element type upstream gives its reference spectra (`typespectre`).
const APP_SPECTRUM_TYPE: i64 = 45;

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
    /// Anything else worth knowing, one sentence each.
    pub notes: Vec<String>,
}

/// Reads a `.proj` file.
pub fn import_proj_file(path: &Path) -> Result<ProjImport> {
    import_proj(&read_bytes(path)?)
}

/// Reads a `.proj` archive held in memory. See the module docs.
pub fn import_proj(bytes: &[u8]) -> Result<ProjImport> {
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

    let xml_bytes = archive.read(&config_name)?;
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
    let mesh_name = project_el
        .children()
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
    for gr in sgroupes.children().filter(|c| c.is_element()) {
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

    // Surface receivers.
    enum Rs {
        Scene { name: String, enabled: bool },
        Plane(SurfaceReceiverShape, String, bool),
    }
    let mut receivers: Vec<Rs> = Vec::new();
    let mut face_receiver: Vec<Option<usize>> = vec![None; n_faces];
    if let Some(list) = opt_child(data, "recepteurss") {
        for r in list.children().filter(|c| c.is_element()) {
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
                    if let Some(gr) = r.children().find(|c| c.has_tag_name("gr")) {
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
                    receivers.push(Rs::Scene { name, enabled });
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
                    receivers.push(Rs::Plane(
                        SurfaceReceiverShape::CuttingPlane {
                            a,
                            b,
                            c,
                            resolution_m: F64::new(resolution),
                        },
                        name,
                        enabled,
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

    // Fitting zones and volumes are not read.
    for tag in ["encombrements", "volumes"] {
        if let Some(list) = opt_child(data, tag)
            && let Some(first) = list.children().find(|c| c.is_element())
        {
            return Err(ImportError::unsupported(
                FMT_PROJ,
                format!(
                    "{} `{}`: fitting zones and volumes are not imported from a .proj",
                    first.tag_name().name(),
                    first.attribute("name").unwrap_or("")
                ),
            ));
        }
    }

    let digest_refs: Vec<&[u8]> = digest_parts.iter().map(Vec::as_slice).collect();
    let mut all_parts: Vec<&[u8]> = vec![b"upstream proj"];
    all_parts.extend(digest_refs);
    let ids = IdSource::from_parts(&all_parts);

    // Bands: upstream's GUI always holds all 27 third-octave bands.
    let bands = BandSet::range(BandKind::ThirdOctave, 50, 20_000).expect("nominal range");
    let n = bands.len();

    // Materials: the project's own database, then the reference materials groups use.
    let mut materials: Vec<Material> = Vec::new();
    if let Some(bdd) = opt_child(project_el, "bdd")
        && let Some(mats) = opt_child(bdd, "materiaux")
    {
        for m in mats.descendants().filter(|d| d.has_tag_name("materiau")) {
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

    // Surface groups: one per (material group, scene receiver) pair.
    const NONE: usize = usize::MAX;
    let mut keys: Vec<(usize, usize)> = (0..n_faces)
        .map(|f| {
            (
                face_group[f].unwrap_or(NONE),
                face_receiver[f].unwrap_or(NONE),
            )
        })
        .collect();
    keys.sort_unstable_by_key(|&(g, r)| (g, r.wrapping_add(1)));
    keys.dedup();
    let mut surface_groups: Vec<SurfaceGroup> = Vec::with_capacity(keys.len());
    let unassigned = face_group.iter().filter(|g| g.is_none()).count();
    for (i, &(g, r)) in keys.iter().enumerate() {
        let split = keys.iter().filter(|k| k.0 == g).count() > 1;
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
        let name = match (&receivers.get(r), split) {
            (Some(Rs::Scene { name, .. }), true) => format!("{base} / {name}"),
            _ => base,
        };
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
        let key = (
            face_group[f].unwrap_or(NONE),
            face_receiver[f].unwrap_or(NONE),
        );
        let i = keys
            .binary_search_by_key(&(key.0, key.1.wrapping_add(1)), |&(g, r)| {
                (g, r.wrapping_add(1))
            })
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

    // Surface receivers.
    let mut surface_receivers = Vec::with_capacity(receivers.len());
    for (k, r) in receivers.into_iter().enumerate() {
        let id = SurfaceReceiverId(ids.uuid("surface receiver", k));
        surface_receivers.push(match r {
            Rs::Scene { name, enabled } => SurfaceReceiver {
                id,
                name,
                enabled,
                shape: SurfaceReceiverShape::Scene {
                    groups: keys
                        .iter()
                        .zip(&surface_groups)
                        .filter(|(key, _)| key.1 == k)
                        .map(|(_, g)| g.id)
                        .collect(),
                },
            },
            Rs::Plane(shape, name, enabled) => SurfaceReceiver {
                id,
                name,
                enabled,
                shape,
            },
        });
    }

    // Sources and point receivers.
    let mut sources = Vec::new();
    if let Some(list) = opt_child(data, "sources") {
        for s in list.children().filter(|c| c.is_element()) {
            if !(s.has_tag_name("source") || s.has_tag_name("sources")) {
                return Err(ImportError::unsupported(
                    FMT_XML,
                    format!("<{}> in the sound sources", s.tag_name().name()),
                ));
            }
            let index = sources.len();
            sources.push(read_source(s, &bands, SourceId(ids.uuid("source", index)))?);
        }
    }
    let mut point_receivers = Vec::new();
    if let Some(list) = opt_child(data, "recepteursp") {
        for r in list.children().filter(|c| c.is_element()) {
            if !r.has_tag_name("recepteurp") {
                return Err(ImportError::unsupported(
                    FMT_XML,
                    format!("<{}> in the point receivers", r.tag_name().name()),
                ));
            }
            let index = point_receivers.len();
            point_receivers.push(read_point_receiver(
                r,
                &bands,
                PointReceiverId(ids.uuid("point receiver", index)),
            )?);
        }
    }

    // Environment and solver settings.
    let environment = read_environment(project_el, &mut report.notes)?;
    let core = child(root, "Core")?;
    let solvers = read_solvers(core, &bands, &mut report.notes)?;

    // Name and description.
    let info = project_el
        .children()
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
        fitting_zones: Vec::new(),
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
// The element tree.

fn opt_child<'a, 'i>(node: Node<'a, 'i>, name: &str) -> Option<Node<'a, 'i>> {
    node.children().find(|c| c.has_tag_name(name))
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
    node.children()
        .find(|c| c.has_tag_name("p") && c.attribute("name") == Some(name))
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
    let pos = node
        .children()
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
    for row in m.children().filter(|c| c.has_tag_name("p")) {
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
            // Projects before 1.3.4: attributes on the row, transmission iff affaiblissement.
            let attr = |a: &str| -> Result<Option<f64>> {
                match row.attribute(a) {
                    None => Ok(None),
                    Some(t) => number(t).map(|v| Some(widen_f32(v as f32))).ok_or_else(|| {
                        ImportError::invalid(FMT_XML, format!("{w}: `{a}` = `{t}`"))
                    }),
                }
            };
            absorption[b] = attr("absorb")?;
            scattering[b] = attr("diffusion")?;
            law[b] = attr("loi")?.map(|v| v as i64);
            transmission[b] = Some(attr("affaiblissement")?);
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
    let laws: Vec<i64> = law.into_iter().flatten().collect();
    if laws.iter().any(|&l| l != laws[0]) {
        return Err(ImportError::unsupported(
            FMT_XML,
            format!("{what}: the reflection law differs between bands ({laws:?})"),
        ));
    }
    let reflection_law = u8::try_from(laws[0])
        .ok()
        .and_then(ReflectionLaw::from_solver_code)
        .ok_or_else(|| {
            ImportError::unsupported(FMT_XML, format!("{what}: reflection law {}", laws[0]))
        })?;
    let tl: Vec<Option<f64>> = transmission.into_iter().map(Option::flatten).collect();
    let transmission_loss_db = if tl.iter().all(Option::is_some) {
        Some(tl.into_iter().map(|v| F64::new(v.unwrap_or(0.0))).collect())
    } else if tl.iter().all(Option::is_none) {
        None
    } else {
        return Err(ImportError::unsupported(
            FMT_XML,
            format!("{what}: only some bands have transmission on"),
        ));
    };
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
    let rows: Vec<Node> = sp.children().filter(|c| c.has_tag_name("p")).collect();
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
    let bdd = root.descendants().find(|d| d.has_tag_name("bdd"))?;
    let el = bdd.descendants().find(|d| {
        d.attribute("idspectre")
            .and_then(|v| v.trim().parse::<i64>().ok())
            == Some(id)
            && d.attribute("eid")
                .and_then(|v| v.trim().parse::<i64>().ok())
                == Some(ty)
    })?;
    let what = format!("user spectrum {id}");
    let mut out = vec![None; bands.len()];
    for r in el.children().filter(|c| c.has_tag_name("p")) {
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
    let pos = s
        .children()
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

fn band_switches(core: Node<'_, '_>, bands: &BandSet, what: &str) -> Result<Vec<bool>> {
    let conf = child(core, "core_conf_bfreq")?;
    let mut out = vec![None; bands.len()];
    for p in conf.children().filter(|c| c.has_tag_name("p")) {
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

    // Meshing, from SPPS's settings (TCR's are the same in upstream's GUI by default).
    let mesh = child(spps, "mesh_conf")?;
    let mw = "SPPS meshing";
    let m: &mut MeshSettings = &mut s.meshing;
    if let Some(v) = opt_prop_real(mesh, "minratio", mw)? {
        m.min_radius_edge_ratio = F64::new(v);
    }
    m.max_volume_m3 = if opt_prop_bool(mesh, "ismaxvol", mw)?.unwrap_or(false) {
        Some(F64::new(prop_real(mesh, "maxvol", mw)?))
    } else {
        None
    };
    m.surface_receiver_max_area_m2 =
        if opt_prop_bool(mesh, "isareaconstraint", mw)?.unwrap_or(false) {
            Some(F64::new(prop_real(mesh, "constraintrecepteurss", mw)?))
        } else {
            None
        };
    let user = opt_prop_text(mesh, "userdefineparams").unwrap_or("").trim();
    if !user.is_empty() {
        return Err(ImportError::unsupported(
            FMT_XML,
            format!("{mw}: user-defined TetGen parameters `{user}`"),
        ));
    }
    let append = opt_prop_text(mesh, "appendparams").unwrap_or("-Y");
    let mut preserve = false;
    for t in append.split_ascii_whitespace() {
        match t {
            "-Y" => preserve = true,
            other => {
                return Err(ImportError::unsupported(
                    FMT_XML,
                    format!("{mw}: additional TetGen parameter `{other}`"),
                ));
            }
        }
    }
    m.preserve_boundary = preserve;
    if opt_prop_bool(mesh, "preprocess", mw)?.unwrap_or(false) {
        notes.push(
            "upstream ran its scene correction (preprocess) before meshing this project; this \
             import does not"
                .to_string(),
        );
    }

    // TCR.
    if let Some(tc) = opt_child(core, "tc") {
        let conf = child(tc, "configuration")?;
        s.tcr = TcrSettings {
            air_absorption: opt_prop_bool(conf, "abs_atmo_calc", "TCR settings")?.unwrap_or(true),
            bands_computed: band_switches(tc, bands, "TCR bands")?,
        };
    } else {
        notes.push("no TCR settings: upstream's defaults are used".to_string());
    }
    Ok(s)
}
