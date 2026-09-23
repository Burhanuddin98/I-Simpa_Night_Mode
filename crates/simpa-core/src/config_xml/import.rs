//! The importer: a `config.xml` written by upstream's GUI, to a [`Project`].

use std::fmt;

use roxmltree::Node;
use uuid::{Builder, Uuid};

use super::num::{solver_int, solver_real, widen_f32};
use crate::formats::cbin;
use crate::schema::{
    AirAbsorption, AttenuationUnit, BandKind, BandSet, ComputationMethod, DiffusionLaw,
    Directivity, Environment, F64, Face, FittingShape, FittingZone, FittingZoneId, Geometry,
    GroupId, IntegrityError, Material, MaterialId, PointReceiver, PointReceiverId, Project,
    ProjectId, ReflectionLaw, Rgb, SolverSettings, SoundMapQuantity, Source, SourceId, Spectrum,
    SpectrumShape, SppsSettings, SurfaceGroup, SurfaceReceiver, SurfaceReceiverId,
    SurfaceReceiverShape, TcrSettings, Vec3,
};

/// Why a `config.xml` could not be imported. [`ImportError::code`] is a stable reason code.
#[derive(Debug)]
pub enum ImportError {
    /// Not well-formed XML. Line and column are 1-based.
    Xml {
        line: u32,
        column: u32,
        message: String,
    },
    /// An element or attribute the solvers need is absent.
    Missing { what: String },
    /// A value the solvers would misread: not a number, out of range, a duplicate.
    Invalid { what: String, value: String },
    /// Something the project model cannot hold as the solvers would act on it.
    Unsupported { what: String, reason: String },
    /// The scene mesh does not match the configuration.
    Mesh { reason: String },
    /// The imported project fails [`Project::check_integrity`].
    Integrity(IntegrityError),
}

impl ImportError {
    pub fn code(&self) -> &'static str {
        match self {
            ImportError::Xml { .. } => "xml_syntax",
            ImportError::Missing { .. } => "missing",
            ImportError::Invalid { .. } => "invalid_value",
            ImportError::Unsupported { .. } => "unsupported",
            ImportError::Mesh { .. } => "mesh_mismatch",
            ImportError::Integrity(_) => "integrity",
        }
    }
}

impl fmt::Display for ImportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ImportError::Xml {
                line,
                column,
                message,
            } => write!(
                f,
                "config.xml is not well-formed at {line}:{column}: {message}"
            ),
            ImportError::Missing { what } => write!(f, "config.xml has no {what}"),
            ImportError::Invalid { what, value } => write!(f, "{what} is invalid: '{value}'"),
            ImportError::Unsupported { what, reason } => write!(f, "{what}: {reason}"),
            ImportError::Mesh { reason } => write!(f, "scene mesh does not match: {reason}"),
            ImportError::Integrity(e) => write!(f, "imported project is inconsistent: {e}"),
        }
    }
}

impl std::error::Error for ImportError {}

type Result<T> = std::result::Result<T, ImportError>;

/// Imports a `config.xml` written by upstream's GUI, for SPPS or for TCR, with no geometry.
///
/// The project gets one surface group per declared material (`type_surface`), each carrying
/// that material, whose [`Material::solver_id`] pins the declared `@id`. Scene surface receivers
/// cover no group, since which faces they cover is only in the mesh; fitting zones cannot be
/// imported at all ([`ImportError::Unsupported`]), since their volume is only in the mesh. Use
/// [`import_upstream_with_mesh`] to get both.
///
/// What is read, and how:
/// - every value as the solver reads it (see `docs/formats/config_xml.md`); every real then
///   widened with [`widen_f32`], so writing the project back gives the solver the same `f32`;
/// - bands from `freq_enum`, sorted; octave if every frequency is a nominal octave centre,
///   third-octave if every one is a nominal third-octave centre, refused otherwise; `docalc`
///   becomes the importing solver's `bands_computed`, and the other solver keeps its defaults;
/// - spectra by position, as the solvers map them; a spectrum becomes pink or white at a global
///   level with at most 6 decimals when that reproduces every band's `f32` exactly, and custom
///   otherwise;
/// - a material's reflection law, and whether it has a transmission loss, must be the same in
///   every band, since the project holds one of each per material;
/// - a point receiver's name is its `@lbl`, the name the solvers use;
/// - file and folder names, `workingdirectory` and everything the solvers ignore are dropped.
///
/// Ids are deterministic: the same input always gives the same project, byte for byte.
pub fn import_upstream(xml: &str) -> std::result::Result<Project, ImportError> {
    import(xml, None)
}

/// [`import_upstream`], with the geometry from the scene mesh the configuration was run with.
///
/// Vertices are widened with [`widen_f32`]. Faces keep their order, and each distinct
/// (`idMat`, `idRs`, `idEn`) triple becomes a surface group, in order of first appearance,
/// carrying the material declared with that `idMat`. A scene surface receiver covers the groups
/// whose `idRs` is its `@id`; a fitting zone is bounded by the groups whose `idEn` is its `@id`,
/// with its inside point at the centroid of their vertices (a seed that meshing must check,
/// since a non-convex zone's centroid can lie outside it). A face whose `idMat`, `idRs` or `idEn`
/// is not declared is [`ImportError::Mesh`].
///
/// Materials no face uses stay in the project's library, but no group carries them.
pub fn import_upstream_with_mesh(
    xml: &str,
    mesh: &cbin::Model,
) -> std::result::Result<Project, ImportError> {
    import(xml, Some(mesh))
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Solver {
    Spps,
    Tcr,
}

fn import(xml: &str, mesh: Option<&cbin::Model>) -> Result<Project> {
    let doc = roxmltree::Document::parse(xml).map_err(|e| {
        let pos = e.pos();
        ImportError::Xml {
            line: pos.row,
            column: pos.col,
            message: e.to_string(),
        }
    })?;
    let root = doc.root_element();
    let ids = IdSource::new(xml, mesh);

    // Which solver wrote it.
    let sim = child(root, "simulation")?;
    let solver = if sim.has_attribute("nbparticules") {
        Solver::Spps
    } else if sim.has_attribute("direct_recepteurSOutputName") {
        Solver::Tcr
    } else {
        return Err(ImportError::Unsupported {
            what: "simulation".to_string(),
            reason: "neither SPPS's nbparticules nor TCR's direct_recepteurSOutputName is \
                     present, so the solver it was written for is unknown"
                .to_string(),
        });
    };

    // Bands.
    let fe = child(sim, "freq_enum")?;
    let mut entries: Vec<(i32, bool)> = Vec::new();
    for b in items(fe, "freq_enum")? {
        let freq = int_attr(b, "freq", "freq_enum/bfreq")?;
        entries.push((freq, attr(b, "docalc", "freq_enum/bfreq")? == "1"));
    }
    entries.sort_by_key(|e| e.0);
    let mut frequencies_hz = Vec::with_capacity(entries.len());
    for (i, &(f, _)) in entries.iter().enumerate() {
        let f = u32::try_from(f)
            .ok()
            .filter(|&f| f > 0)
            .ok_or_else(|| ImportError::Invalid {
                what: "freq_enum/bfreq@freq".to_string(),
                value: f.to_string(),
            })?;
        if i > 0 && entries[i - 1].0 == entries[i].0 {
            return Err(ImportError::Invalid {
                what: "freq_enum (a duplicate band)".to_string(),
                value: f.to_string(),
            });
        }
        frequencies_hz.push(f);
    }
    let kind = [BandKind::Octave, BandKind::ThirdOctave]
        .into_iter()
        .find(|k| {
            frequencies_hz
                .iter()
                .all(|f| k.nominal_frequencies().contains(f))
        })
        .ok_or_else(|| ImportError::Unsupported {
            what: "freq_enum".to_string(),
            reason: format!(
                "{frequencies_hz:?} Hz are not all nominal octave or third-octave centres"
            ),
        })?;
    let bands = BandSet {
        kind,
        frequencies_hz,
    };
    if let Some(problem) = bands.problem() {
        return Err(ImportError::Unsupported {
            what: "freq_enum".to_string(),
            reason: problem,
        });
    }
    let n = bands.len();
    let docalc: Vec<bool> = entries.iter().map(|e| e.1).collect();

    // Materials, keyed by their declared id.
    let mut materials: Vec<Material> = Vec::new();
    if let Some(enum_node) = opt_child(root, "surface_absorption_enum") {
        for (i, m) in items(enum_node, "surface_absorption_enum")?
            .into_iter()
            .enumerate()
        {
            materials.push(read_material(m, i, n, &ids, &materials)?);
        }
    }

    // Sources.
    let dir_prefix = sim.attribute("directivities_directory").unwrap_or("");
    let mut sources = Vec::new();
    if let Some(list) = opt_child(root, "sources") {
        for (i, s) in items(list, "sources")?.into_iter().enumerate() {
            sources.push(read_source(s, i, &bands, &ids, dir_prefix)?);
        }
    }

    // Point receivers.
    let mut point_receivers = Vec::new();
    if let Some(list) = opt_child(root, "recepteursp") {
        for (i, r) in items(list, "recepteursp")?.into_iter().enumerate() {
            point_receivers.push(read_point_receiver(r, i, &bands, &ids)?);
        }
    }

    // Surface receivers (only these two element names count) and their declared ids. The
    // solvers keep scene receivers and cutting planes in two lists, and match a face's idRs
    // against scene receivers only (coreinitialisation.cpp:48-53), so ids are per list, and
    // `Some(id)` marks a scene receiver.
    let mut surface_receivers = Vec::new();
    let mut surface_receiver_xml_ids: Vec<Option<i32>> = Vec::new();
    let mut cutting_plane_xml_ids: Vec<i32> = Vec::new();
    if let Some(list) = opt_child(root, "recepteurss") {
        for (i, r) in items(list, "recepteurss")?.into_iter().enumerate() {
            let tag = r.tag_name().name();
            let what = format!("recepteurss/{tag}");
            let shape = match tag {
                "recepteur_surfacique" => SurfaceReceiverShape::Scene { groups: Vec::new() },
                "recepteur_surfacique_coupe" => {
                    let p = |a: &str| -> Result<Vec3> {
                        Ok(Vec3::new(
                            real_attr(r, &format!("{a}x"), &what)?,
                            real_attr(r, &format!("{a}y"), &what)?,
                            real_attr(r, &format!("{a}z"), &what)?,
                        ))
                    };
                    SurfaceReceiverShape::CuttingPlane {
                        a: p("a")?,
                        b: p("b")?,
                        c: p("c")?,
                        resolution_m: F64::new(real_attr(r, "resolution", &what)?),
                    }
                }
                _ => continue,
            };
            let xml_id = int_attr(r, "id", &what)?;
            let scene = matches!(shape, SurfaceReceiverShape::Scene { .. });
            let duplicate = if scene {
                surface_receiver_xml_ids.contains(&Some(xml_id))
            } else {
                cutting_plane_xml_ids.contains(&xml_id)
            };
            if duplicate {
                return Err(ImportError::Invalid {
                    what: format!("{what}@id (a duplicate)"),
                    value: xml_id.to_string(),
                });
            }
            if scene {
                surface_receiver_xml_ids.push(Some(xml_id));
            } else {
                surface_receiver_xml_ids.push(None);
                cutting_plane_xml_ids.push(xml_id);
            }
            surface_receivers.push(SurfaceReceiver {
                id: SurfaceReceiverId(ids.uuid("surface receiver", i)),
                name: attr(r, "name", &what)?.to_string(),
                enabled: true,
                shape,
            });
        }
    }

    // Fitting zones: their declared data now, their shape from the mesh below.
    let mut fittings: Vec<(i32, FittingZone)> = Vec::new();
    if let Some(list) = opt_child(root, "encombrement_enum") {
        for (i, z) in items(list, "encombrement_enum")?.into_iter().enumerate() {
            let what = "encombrement_enum/encombrement";
            let xml_id = int_attr(z, "id", what)?;
            if fittings.iter().any(|(id, _)| *id == xml_id) {
                return Err(ImportError::Invalid {
                    what: format!("{what}@id (a duplicate)"),
                    value: xml_id.to_string(),
                });
            }
            let mut absorption = Vec::with_capacity(n);
            let mut mean_free_path_m = Vec::with_capacity(n);
            let mut diffusion_law = Vec::with_capacity(n);
            for b in spectrum(z, what, n)? {
                let bw = format!("{what}/bfreq");
                absorption.push(F64::new(real_attr(b, "alpha", &bw)?));
                mean_free_path_m.push(F64::new(real_attr(b, "lambda", &bw)?));
                let code = int_attr(b, "loi_diff", &bw)?;
                diffusion_law.push(
                    u8::try_from(code)
                        .ok()
                        .and_then(DiffusionLaw::from_solver_code)
                        .ok_or_else(|| ImportError::Unsupported {
                            what: format!("{bw}@loi_diff"),
                            reason: format!(
                                "{code} is not a diffusion law (0 to 2); SPPS leaves the \
                                 direction unchanged for it (CalculationCore.cpp:166-182)"
                            ),
                        })?,
                );
            }
            fittings.push((
                xml_id,
                FittingZone {
                    id: FittingZoneId(ids.uuid("fitting zone", i)),
                    name: format!("Fitting {xml_id}"),
                    enabled: true,
                    // Replaced from the mesh below.
                    shape: FittingShape::Box {
                        min: Vec3::ZERO,
                        max: Vec3::ZERO,
                    },
                    absorption,
                    mean_free_path_m,
                    diffusion_law,
                },
            ));
        }
    }

    // Environment.
    let atmo = child(root, "condition_atmospherique")?;
    let aw = "condition_atmospherique";
    let air_absorption = if int_attr(atmo, "disable_absatmo_computation", aw)? == 1 {
        AirAbsorption::UserDefined {
            value: F64::new(real_attr(atmo, "absatmo", aw)?),
            unit: AttenuationUnit::PerMetre,
        }
    } else {
        AirAbsorption::Iso9613
    };
    let environment = Environment {
        temperature_c: F64::new(real_attr(atmo, "temperature", aw)?),
        relative_humidity_percent: F64::new(real_attr(atmo, "humidite", aw)?),
        pressure_pa: F64::new(real_attr(atmo, "pression", aw)?),
        air_absorption,
        ground_roughness_m: F64::new(real_attr(atmo, "z0", aw)?),
        celerity_gradient_log: F64::new(real_attr(atmo, "alog", aw)?),
        celerity_gradient_lin: F64::new(real_attr(atmo, "blin", aw)?),
    };

    // Solver settings.
    let mut solvers = SolverSettings::for_bands(n);
    let sw = "simulation";
    match solver {
        Solver::Spps => {
            let count = |name: &str| -> Result<u32> {
                let v = int_attr(sim, name, sw)?;
                u32::try_from(v).map_err(|_| ImportError::Invalid {
                    what: format!("{sw}@{name}"),
                    value: v.to_string(),
                })
            };
            let on = |name: &str| -> Result<bool> { Ok(int_attr(sim, name, sw)? != 0) };
            let opt_on = |name: &str, default: bool| -> Result<bool> {
                Ok(match sim.attribute(name) {
                    Some(_) => int_attr(sim, name, sw)? != 0,
                    None => default,
                })
            };
            let sound_map = match int_attr(sim, "surf_receiv_method", sw)? {
                0 => SoundMapQuantity::Intensity,
                1 => SoundMapQuantity::Spl,
                other => {
                    return Err(ImportError::Unsupported {
                        what: format!("{sw}@surf_receiv_method"),
                        reason: format!(
                            "{other} divides by cos(incidence) without the SPL scaling, which \
                             the project cannot express"
                        ),
                    });
                }
            };
            solvers.spps = SppsSettings {
                particles_per_source: count("nbparticules")?,
                particles_saved: count("nbparticules_rendu")?,
                duration_s: F64::new(real_attr(sim, "duree_simulation", sw)?),
                time_step_s: F64::new(real_attr(sim, "pasdetemps", sw)?),
                random_seed: match sim.attribute("random_seed") {
                    Some(_) => count("random_seed")?,
                    None => 0,
                },
                method: if int_attr(sim, "computation_method", sw)? == 0 {
                    ComputationMethod::Random
                } else {
                    ComputationMethod::Energetic
                },
                air_absorption: on("abs_atmo_calc")?,
                fittings: on("enc_calc")?,
                direct_field_only: on("direct_calc")?,
                transmission: on("trans_calc")?,
                extinction_exponent: F64::new(real_attr(sim, "trans_epsilon", sw)?),
                receiver_radius_m: F64::new(real_attr(sim, "rayon_recepteurp", sw)?),
                sound_map,
                sound_maps_per_band: on("output_recs_byfreq")?,
                echogram_per_source: opt_on("output_recp_bysource", false)?,
                save_surface_intersections: opt_on("save_surface_intersection", true)?,
                save_receiver_intersections: opt_on("save_receivers_intersection", true)?,
                bands_computed: docalc,
            };
        }
        Solver::Tcr => {
            solvers.tcr = TcrSettings {
                air_absorption: int_attr(sim, "abs_atmo_calc", sw)? != 0,
                bands_computed: docalc,
            };
        }
    }

    // Geometry and surface groups.
    let mut surface_groups = Vec::new();
    let mut geometry = Geometry::default();
    let material_for = |id_mat: u32| materials.iter().find(|m| m.solver_id == Some(id_mat));
    match mesh {
        None => {
            if let Some((xml_id, _)) = fittings.first() {
                return Err(ImportError::Unsupported {
                    what: format!("encombrement {xml_id}"),
                    reason: "a fitting zone's volume is only in the scene mesh; import with \
                             the .cbin"
                        .to_string(),
                });
            }
            for (i, m) in materials.iter().enumerate() {
                surface_groups.push(SurfaceGroup {
                    id: GroupId(ids.uuid("surface group", i)),
                    name: format!(
                        "idMat {}",
                        m.solver_id.expect("imported materials are pinned")
                    ),
                    material: m.id,
                });
            }
        }
        Some(mesh) => {
            geometry.vertices = mesh
                .vertices
                .iter()
                .map(|v| Vec3::new(widen_f32(v.x), widen_f32(v.y), widen_f32(v.z)))
                .collect();
            let mut triples: Vec<(u32, i32, i32)> = Vec::new();
            for (fi, f) in mesh.faces.iter().enumerate() {
                let triple = (f.id_mat, f.id_rs, f.id_en);
                let gi = match triples.iter().position(|t| *t == triple) {
                    Some(gi) => gi,
                    None => {
                        let m = material_for(f.id_mat).ok_or_else(|| ImportError::Mesh {
                            reason: format!(
                                "face {fi} uses idMat {}, which config.xml does not declare",
                                f.id_mat
                            ),
                        })?;
                        if f.id_rs != -1 && !surface_receiver_xml_ids.contains(&Some(f.id_rs)) {
                            return Err(ImportError::Mesh {
                                reason: format!(
                                    "face {fi} uses idRs {}, which no recepteur_surfacique of \
                                     config.xml declares",
                                    f.id_rs
                                ),
                            });
                        }
                        if f.id_en != -1 && !fittings.iter().any(|(id, _)| *id == f.id_en) {
                            return Err(ImportError::Mesh {
                                reason: format!(
                                    "face {fi} uses idEn {}, which config.xml does not declare",
                                    f.id_en
                                ),
                            });
                        }
                        let mut name = format!("idMat {}", f.id_mat);
                        if f.id_rs != -1 {
                            name.push_str(&format!(" / idRs {}", f.id_rs));
                        }
                        if f.id_en != -1 {
                            name.push_str(&format!(" / idEn {}", f.id_en));
                        }
                        surface_groups.push(SurfaceGroup {
                            id: GroupId(ids.uuid("surface group", triples.len())),
                            name,
                            material: m.id,
                        });
                        triples.push(triple);
                        triples.len() - 1
                    }
                };
                geometry.faces.push(Face {
                    vertices: [f.a, f.b, f.c],
                    group: surface_groups[gi].id,
                });
            }
            for (r, xml_id) in surface_receivers.iter_mut().zip(&surface_receiver_xml_ids) {
                if let (SurfaceReceiverShape::Scene { groups }, Some(xml_id)) =
                    (&mut r.shape, xml_id)
                {
                    *groups = triples
                        .iter()
                        .zip(&surface_groups)
                        .filter(|(t, _)| t.1 == *xml_id)
                        .map(|(_, g)| g.id)
                        .collect();
                }
            }
            for (xml_id, z) in &mut fittings {
                let groups: Vec<GroupId> = triples
                    .iter()
                    .zip(&surface_groups)
                    .filter(|(t, _)| t.2 == *xml_id)
                    .map(|(_, g)| g.id)
                    .collect();
                if groups.is_empty() {
                    return Err(ImportError::Unsupported {
                        what: format!("encombrement {xml_id}"),
                        reason: "no face of the scene mesh carries its idEn, so its volume \
                                 cannot be recovered"
                            .to_string(),
                    });
                }
                let mut sum = [0.0f64; 3];
                let mut count = 0.0;
                for f in mesh.faces.iter().filter(|f| f.id_en == *xml_id) {
                    for v in [f.a, f.b, f.c] {
                        let p = geometry.vertices[v as usize].to_array();
                        for a in 0..3 {
                            sum[a] += p[a];
                        }
                        count += 1.0;
                    }
                }
                z.shape = FittingShape::Surfaces {
                    groups,
                    inside_point: Vec3::from(sum.map(|s| s / count)),
                };
            }
        }
    }

    let project = Project {
        format_version: crate::schema::FORMAT_VERSION,
        id: ProjectId(ids.uuid("project", 0)),
        name: "Imported project".to_string(),
        description: format!(
            "Imported from an upstream I-Simpa {} config.xml{}.",
            match solver {
                Solver::Spps => "SPPS",
                Solver::Tcr => "TCR",
            },
            if mesh.is_some() {
                ", with the geometry of its scene mesh"
            } else {
                ", without geometry"
            }
        ),
        frame: Default::default(),
        bands,
        geometry,
        surface_groups,
        materials,
        sources,
        point_receivers,
        surface_receivers,
        fitting_zones: fittings.into_iter().map(|(_, z)| z).collect(),
        environment,
        solvers,
        variants: Vec::new(),
        active_variant: None,
        view: Default::default(),
    };
    project.check_integrity().map_err(ImportError::Integrity)?;
    Ok(project)
}

/// Display colours for imported materials, by position.
const PALETTE: [Rgb; 8] = [
    Rgb(0xb0, 0xb0, 0xb0),
    Rgb(0xc8, 0x9f, 0x6e),
    Rgb(0x7a, 0x9e, 0xc4),
    Rgb(0x9c, 0xc4, 0x7a),
    Rgb(0xd4, 0x8a, 0x8a),
    Rgb(0xb4, 0x96, 0xd0),
    Rgb(0xd8, 0xc8, 0x6a),
    Rgb(0x6a, 0xc0, 0xb8),
];

fn read_material(
    m: Node<'_, '_>,
    index: usize,
    n: usize,
    ids: &IdSource,
    earlier: &[Material],
) -> Result<Material> {
    let what = "surface_absorption_enum/type_surface";
    let id = int_attr(m, "id", what)?;
    let solver_id = u32::try_from(id).map_err(|_| ImportError::Unsupported {
        what: format!("{what}@id"),
        reason: format!("{id} is negative, and the solvers store it unsigned"),
    })?;
    if earlier.iter().any(|e| e.solver_id == Some(solver_id)) {
        return Err(ImportError::Invalid {
            what: format!("{what}@id (a duplicate: the solvers use only the first)"),
            value: id.to_string(),
        });
    }
    let what = format!("type_surface {id}");
    // side_material is optional: absent means double-sided (coreTypes.h:137).
    let double_sided = match m.attribute("side_material") {
        Some(_) => int_attr(m, "side_material", &what)? == 1,
        None => true,
    };
    let mut absorption = Vec::with_capacity(n);
    let mut scattering = Vec::with_capacity(n);
    let mut laws = Vec::with_capacity(n);
    let mut transmission = Vec::with_capacity(n);
    for b in spectrum(m, &what, n)? {
        let bw = format!("{what}/bfreq");
        absorption.push(F64::new(real_attr(b, "absorb", &bw)?));
        scattering.push(F64::new(real_attr(b, "diffusion", &bw)?));
        laws.push(int_attr(b, "loi", &bw)?);
        transmission.push(match b.attribute("affaiblissement") {
            Some(_) => Some(F64::new(real_attr(b, "affaiblissement", &bw)?)),
            None => None,
        });
    }
    let law = laws.first().copied().unwrap_or(0);
    if laws.iter().any(|&l| l != law) {
        return Err(ImportError::Unsupported {
            what: format!("{what} loi"),
            reason: format!("the reflection law differs between bands ({laws:?})"),
        });
    }
    let reflection_law = u8::try_from(law)
        .ok()
        .and_then(ReflectionLaw::from_solver_code)
        .ok_or_else(|| ImportError::Unsupported {
            what: format!("{what} loi"),
            reason: format!("{law} is not a reflection law (0 to 6)"),
        })?;
    let transmission_loss_db = if transmission.iter().all(Option::is_some) && n > 0 {
        Some(transmission.into_iter().map(Option::unwrap).collect())
    } else if transmission.iter().all(Option::is_none) {
        None
    } else {
        return Err(ImportError::Unsupported {
            what: format!("{what} affaiblissement"),
            reason: "only some bands have a transmission loss".to_string(),
        });
    };
    Ok(Material {
        id: MaterialId(ids.uuid("material", index)),
        name: format!("Material {id}"),
        color: PALETTE[index % PALETTE.len()],
        absorption,
        scattering,
        reflection_law,
        transmission_loss_db,
        double_sided,
        solver_id: Some(solver_id),
    })
}

fn read_source(
    s: Node<'_, '_>,
    index: usize,
    bands: &BandSet,
    ids: &IdSource,
    dir_prefix: &str,
) -> Result<Source> {
    let what = format!("source {}", index + 1);
    let code = int_attr(s, "directivite", &what)?;
    let direction = || -> Result<Vec3> {
        Ok(Vec3::new(
            real_attr(s, "u", &what)?,
            real_attr(s, "v", &what)?,
            real_attr(s, "w", &what)?,
        ))
    };
    let directivity = match code {
        0 => Directivity::Omni,
        1 => Directivity::Unidirectional {
            direction: direction()?,
        },
        2 => Directivity::PlaneXy,
        3 => Directivity::PlaneYz,
        4 => Directivity::PlaneXz,
        5 => Directivity::Balloon {
            file: format!("{dir_prefix}{}", attr(s, "directivity_file", &what)?).replace('\\', "/"),
            direction: direction()?,
        },
        other => {
            return Err(ImportError::Unsupported {
                what: format!("{what}@directivite"),
                reason: format!("{other} is not a source type (0 to 5)"),
            });
        }
    };
    let levels = spectrum(s, &what, bands.len())?
        .into_iter()
        .map(|b| real_attr_f32(b, "db", &format!("{what}/bfreq")))
        .collect::<Result<Vec<f32>>>()?;
    Ok(Source {
        id: SourceId(ids.uuid("source", index)),
        name: attr(s, "name", &what)?.to_string(),
        enabled: true,
        position: Vec3::new(
            real_attr(s, "x", &what)?,
            real_attr(s, "y", &what)?,
            real_attr(s, "z", &what)?,
        ),
        power: spectrum_from_levels(bands, &levels),
        directivity,
        delay_s: F64::new(real_attr(s, "delay", &what)?),
    })
}

fn read_point_receiver(
    r: Node<'_, '_>,
    index: usize,
    bands: &BandSet,
    ids: &IdSource,
) -> Result<PointReceiver> {
    let what = format!("recepteur_ponctuel {}", index + 1);
    let entries = items(r, &what)?;
    let background_noise = if entries.is_empty() {
        None
    } else {
        let levels = spectrum(r, &what, bands.len())?
            .into_iter()
            .map(|b| real_attr_f32(b, "db", &format!("{what}/bfreq")))
            .collect::<Result<Vec<f32>>>()?;
        Some(spectrum_from_levels(bands, &levels))
    };
    Ok(PointReceiver {
        id: PointReceiverId(ids.uuid("point receiver", index)),
        name: attr(r, "lbl", &what)?.to_string(),
        position: Vec3::new(
            real_attr(r, "x", &what)?,
            real_attr(r, "y", &what)?,
            real_attr(r, "z", &what)?,
        ),
        orientation: Vec3::new(
            real_attr(r, "u", &what)?,
            real_attr(r, "v", &what)?,
            real_attr(r, "w", &what)?,
        ),
        background_noise,
    })
}

/// The spectrum whose band levels, as `f32`, are exactly `levels`: pink, then white, at a global
/// level rounded to 0 to 6 decimals, then at the unrounded estimate; custom if none matches. A
/// custom spectrum holds the widened levels as its relative levels and their energetic sum as its
/// global level, which [`Spectrum::band_levels_db`] turns back into exactly those levels.
pub(crate) fn spectrum_from_levels(bands: &BandSet, levels: &[f32]) -> Spectrum {
    let widened: Vec<f64> = levels.iter().map(|&l| widen_f32(l)).collect();
    // The same expression band_levels_db uses, so a custom spectrum's offset is exactly 0.
    let total: f64 = widened.iter().map(|r| 10f64.powf(r / 10.0)).sum();
    let estimate = 10.0 * total.log10();
    let matches = |s: &Spectrum| {
        s.band_levels_db(bands).is_some_and(|l| {
            l.len() == levels.len()
                && l.iter()
                    .zip(levels)
                    .all(|(a, b)| (*a as f32).to_bits() == b.to_bits())
        })
    };
    if estimate.is_finite() {
        let candidates = (0..=6usize)
            .filter_map(|d| format!("{estimate:.d$}").parse::<f64>().ok())
            .chain([estimate]);
        for global in candidates {
            for shape in [SpectrumShape::Pink, SpectrumShape::White] {
                let s = Spectrum::new(global, shape);
                if matches(&s) {
                    return s;
                }
            }
        }
    }
    Spectrum::new(
        estimate,
        SpectrumShape::Custom {
            relative_db: widened.into_iter().map(F64::new).collect(),
        },
    )
}

/// Deterministic UUIDs: a 128-bit FNV-1a digest of the inputs, then of each entity's kind and
/// position, shaped as version-4 UUIDs.
struct IdSource {
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
    fn new(xml: &str, mesh: Option<&cbin::Model>) -> Self {
        let mut d = FNV_OFFSETS;
        fnv(&mut d, xml.as_bytes());
        if let Some(mesh) = mesh {
            fnv(&mut d, b"\0mesh");
            for v in &mesh.vertices {
                for c in [v.x, v.y, v.z] {
                    fnv(&mut d, &c.to_bits().to_le_bytes());
                }
            }
            for f in &mesh.faces {
                for x in [f.a, f.b, f.c, f.id_mat] {
                    fnv(&mut d, &x.to_le_bytes());
                }
                for x in [f.id_rs, f.id_en] {
                    fnv(&mut d, &x.to_le_bytes());
                }
            }
        }
        IdSource { digest: d }
    }

    fn uuid(&self, kind: &str, index: usize) -> Uuid {
        let mut s = self.digest;
        fnv(&mut s, kind.as_bytes());
        fnv(&mut s, &(index as u64).to_le_bytes());
        let mut bytes = [0u8; 16];
        bytes[..8].copy_from_slice(&s[0].to_le_bytes());
        bytes[8..].copy_from_slice(&s[1].to_le_bytes());
        Builder::from_random_bytes(bytes).into_uuid()
    }
}

// ---------------------------------------------------------------------------------------------
// Reading nodes the way the solvers do.

/// The first child element with this name (`CXmlNode::GetChild`, `cxml.cpp:171-179`).
fn opt_child<'a, 'i>(node: Node<'a, 'i>, name: &str) -> Option<Node<'a, 'i>> {
    node.children()
        .find(|c| c.is_element() && c.tag_name().name() == name)
}

fn child<'a, 'i>(node: Node<'a, 'i>, name: &str) -> Result<Node<'a, 'i>> {
    opt_child(node, name).ok_or_else(|| ImportError::Missing {
        what: format!("<{name}> in <{}>", node.tag_name().name()),
    })
}

/// A list's items: every child element, whatever its name. Text other than whitespace would be
/// an extra, nameless item for the solvers, so it is refused.
fn items<'a, 'i>(node: Node<'a, 'i>, what: &str) -> Result<Vec<Node<'a, 'i>>> {
    let mut out = Vec::new();
    for c in node.children() {
        if c.is_element() {
            out.push(c);
        } else if c.is_text() && !c.text().unwrap_or("").trim().is_empty() {
            return Err(ImportError::Unsupported {
                what: what.to_string(),
                reason: "text content, which the solvers would read as an extra item".to_string(),
            });
        }
    }
    Ok(out)
}

/// A spectrum's entries, sorted by `@freq` as the solvers sort them, which must number `n`: the
/// solvers map them to bands by position, and read past a short spectrum.
fn spectrum<'a, 'i>(node: Node<'a, 'i>, what: &str, n: usize) -> Result<Vec<Node<'a, 'i>>> {
    let mut keyed = items(node, what)?
        .into_iter()
        .map(|b| Ok((int_attr(b, "freq", &format!("{what}/bfreq"))?, b)))
        .collect::<Result<Vec<_>>>()?;
    if keyed.len() != n {
        return Err(ImportError::Unsupported {
            what: what.to_string(),
            reason: format!(
                "{} band entries for {n} bands; the solvers map entries to bands by position",
                keyed.len()
            ),
        });
    }
    keyed.sort_by_key(|(f, _)| *f);
    Ok(keyed.into_iter().map(|(_, b)| b).collect())
}

fn attr<'a>(node: Node<'a, '_>, name: &str, what: &str) -> Result<&'a str> {
    node.attribute(name).ok_or_else(|| ImportError::Missing {
        what: format!("{what}@{name}"),
    })
}

fn int_attr(node: Node<'_, '_>, name: &str, what: &str) -> Result<i32> {
    let text = attr(node, name, what)?;
    solver_int(text).ok_or_else(|| ImportError::Invalid {
        what: format!("{what}@{name}"),
        value: text.to_string(),
    })
}

fn real_attr_f32(node: Node<'_, '_>, name: &str, what: &str) -> Result<f32> {
    let text = attr(node, name, what)?;
    solver_real(text).ok_or_else(|| ImportError::Invalid {
        what: format!("{what}@{name}"),
        value: text.to_string(),
    })
}

/// A real attribute as the solver reads it, widened with [`widen_f32`].
fn real_attr(node: Node<'_, '_>, name: &str, what: &str) -> Result<f64> {
    real_attr_f32(node, name, what).map(widen_f32)
}
