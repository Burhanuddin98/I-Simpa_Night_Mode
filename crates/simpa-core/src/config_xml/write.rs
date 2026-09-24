//! The writer: a [`Project`] to the `config.xml` of one solver.

use std::fmt;
use std::io;
use std::path::{Path, PathBuf};

use super::ids::{DRAWN_ZONE_MATERIAL_ID, SolverIds, group_zone_ids, has_drawn_zones};
use super::names;
use super::num::{real_text, widen_f32};
use crate::geometry::import::{REFERENCE_SPECTRA, default_material};
use crate::schema::{
    BandKind, BandSet, Directivity, FittingZone, IntegrityError, Material, MaterialId, Project,
    ReflectionLaw, SolverKind, Source, Spectrum, SpectrumShape, SurfaceReceiverShape, VariantId,
};

/// Why a `config.xml` could not be written. [`WriteError::code`] is a stable reason code.
#[derive(Debug)]
pub enum WriteError {
    /// The project fails [`Project::check_integrity`].
    Integrity(IntegrityError),
    /// No variant has this name or id.
    VariantNotFound(String),
    /// Several variants have this name; select one by its id.
    VariantAmbiguous { name: String, count: usize },
    /// The run folder cannot be written as `workingdirectory`.
    WorkingDirectory { path: String, reason: &'static str },
    /// A real is NaN or infinite (or, for a mesh vertex, beyond the `f32` range).
    NonFinite { what: String },
    /// A string holds a character XML 1.0 cannot carry, even escaped.
    Unencodable { what: String, ch: char },
    /// A project value the solvers cannot act on as the project means it.
    Unsupported { what: String, reason: String },
    /// Surface groups share a pinned material id, but the variant gives them different materials.
    SharedSolverId {
        solver_id: u32,
        first: String,
        second: String,
    },
    /// A surface group is in two enabled scene receivers, or bounds two enabled fitting zones.
    GroupInTwo { group: String, what: &'static str },
    /// More entities than solver ids fit in a C `int`.
    TooManyIds { what: &'static str },
    /// A pinned solver id that cannot be written: two entities of one kind pinned to it, a
    /// fitting zone pinned to 0 (the solvers' "no fitting"), or one above the C `int` range
    /// (`SolverIds::assign`).
    SolverIdClash {
        what: &'static str,
        id: u32,
        reason: String,
    },
    /// Writing the file failed.
    Io(io::Error),
}

impl WriteError {
    pub fn code(&self) -> &'static str {
        match self {
            WriteError::Integrity(_) => "integrity",
            WriteError::VariantNotFound(_) => "variant_not_found",
            WriteError::VariantAmbiguous { .. } => "variant_ambiguous",
            WriteError::WorkingDirectory { .. } => "working_directory_invalid",
            WriteError::NonFinite { .. } => "non_finite_value",
            WriteError::Unencodable { .. } => "unencodable_text",
            WriteError::Unsupported { .. } => "unsupported_value",
            WriteError::SharedSolverId { .. } => "shared_solver_id",
            WriteError::GroupInTwo { .. } => "group_in_two_zones",
            WriteError::TooManyIds { .. } => "solver_id_overflow",
            WriteError::SolverIdClash { .. } => "solver_id_clash",
            WriteError::Io(_) => "io",
        }
    }
}

impl fmt::Display for WriteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WriteError::Integrity(e) => write!(f, "project is inconsistent: {e}"),
            WriteError::VariantNotFound(v) => write!(f, "no variant is named or has the id '{v}'"),
            WriteError::VariantAmbiguous { name, count } => write!(
                f,
                "{count} variants are named '{name}'; select one by its id"
            ),
            WriteError::WorkingDirectory { path, reason } => {
                write!(
                    f,
                    "run folder '{path}' cannot be the working directory: {reason}"
                )
            }
            WriteError::NonFinite { what } => write!(f, "{what} is not a finite number"),
            WriteError::Unencodable { what, ch } => write!(
                f,
                "{what} holds the character U+{:04X}, which XML 1.0 cannot carry",
                u32::from(*ch)
            ),
            WriteError::Unsupported { what, reason } => write!(f, "{what}: {reason}"),
            WriteError::SharedSolverId {
                solver_id,
                first,
                second,
            } => write!(
                f,
                "surface groups '{first}' and '{second}' share the pinned material id {solver_id}, \
                 but the variant gives them different materials"
            ),
            WriteError::GroupInTwo { group, what } => {
                write!(f, "surface group '{group}' is in two {what}")
            }
            WriteError::TooManyIds { what } => {
                write!(f, "too many {what}: their solver ids do not fit in a C int")
            }
            WriteError::SolverIdClash { what, id, reason } => {
                write!(f, "{what}: solver id {id} cannot be written: {reason}")
            }
            WriteError::Io(e) => write!(f, "i/o error: {e}"),
        }
    }
}

impl std::error::Error for WriteError {}

/// Resolves [`write()`]'s variant selector: `None` is the base project; otherwise a variant's id
/// (hyphenated UUID) or its name.
pub fn resolve_variant(
    project: &Project,
    variant: Option<&str>,
) -> Result<Option<VariantId>, WriteError> {
    let Some(sel) = variant else {
        return Ok(None);
    };
    if let Some(v) = project.variants.iter().find(|v| v.id.to_string() == sel) {
        return Ok(Some(v.id));
    }
    let named: Vec<&crate::schema::Variant> =
        project.variants.iter().filter(|v| v.name == sel).collect();
    match named.as_slice() {
        [v] => Ok(Some(v.id)),
        [] => Err(WriteError::VariantNotFound(sel.to_string())),
        many => Err(WriteError::VariantAmbiguous {
            name: sel.to_string(),
            count: many.len(),
        }),
    }
}

/// `workingdirectory` for a run folder: the path as UTF-8, absolute, ending in the platform
/// separator. On Windows, `/` becomes `\` and a `\\?\C:\` or `\\?\UNC\` prefix is dropped (the
/// solvers join names to this path with plain string concatenation, `sppsNantes.cpp:300-302`);
/// other verbatim and device paths are refused. The folder is not checked for existence here.
pub fn working_directory(workdir: &Path) -> Result<String, WriteError> {
    let bad = |reason| WriteError::WorkingDirectory {
        path: workdir.display().to_string(),
        reason,
    };
    let text = workdir
        .to_str()
        .ok_or_else(|| bad("the path is not valid UTF-8"))?;
    #[cfg(windows)]
    let (text, sep) = {
        let text = if let Some(rest) = text.strip_prefix(r"\\?\UNC\") {
            format!(r"\\{rest}")
        } else if let Some(rest) = text.strip_prefix(r"\\?\") {
            let b = rest.as_bytes();
            if b.len() >= 3 && b[0].is_ascii_alphabetic() && b[1] == b':' && b[2] == b'\\' {
                rest.to_string()
            } else {
                return Err(bad("a verbatim path other than a drive or UNC path"));
            }
        } else if text.starts_with(r"\\.\") {
            return Err(bad("a device path"));
        } else {
            text.to_string()
        };
        (text.replace('/', "\\"), '\\')
    };
    #[cfg(not(windows))]
    let (text, sep) = (text.to_string(), '/');
    if !Path::new(&text).is_absolute() {
        return Err(bad("the path is not absolute"));
    }
    let mut text = text;
    if !text.ends_with(sep) {
        text.push(sep);
    }
    Ok(text)
}

/// A directivity file a run folder must hold: `project_path` is the source's
/// [`Directivity::Balloon`] `file` (relative to the project file), and `run_name` the name it has
/// in the run folder's [`names::DIRECTIVITY_DIR`] folder, as `source@directivity_file` gives it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StagedFile {
    pub project_path: String,
    pub run_name: String,
}

/// The directivity files of the enabled sources, each once, in source order. A file keeps its
/// base name unless an earlier file took that name (compared case-insensitively, as on NTFS);
/// then it becomes `2_<name>`, `3_<name>`, ... until the name is free.
pub fn directivity_files(project: &Project) -> Vec<StagedFile> {
    let mut out: Vec<StagedFile> = Vec::new();
    for s in project.sources.iter().filter(|s| s.enabled) {
        let Directivity::Balloon { file, .. } = &s.directivity else {
            continue;
        };
        if out.iter().any(|f| &f.project_path == file) {
            continue;
        }
        let base = file.rsplit(['/', '\\']).next().unwrap_or(file);
        let taken = |name: &str| {
            out.iter()
                .any(|f| f.run_name.to_lowercase() == name.to_lowercase())
        };
        // An empty name stays empty, and write() refuses it.
        let mut run_name = base.to_string();
        let mut k = 2;
        while !base.is_empty() && taken(&run_name) {
            run_name = format!("{k}_{base}");
            k += 1;
        }
        out.push(StagedFile {
            project_path: file.clone(),
            run_name,
        });
    }
    out
}

/// The `config.xml` of `solver` for `project`, with `variant`'s material overrides (`None` = the
/// base project; see [`resolve_variant`]), for a run in `workdir` (see [`working_directory`]).
/// The run folder must also hold [`names::SCENE_MESH`] (from [`super::scene_mesh`]),
/// [`names::TETRA_MESH`], and every [`directivity_files`] entry under
/// [`names::DIRECTIVITY_DIR`].
pub fn write(
    project: &Project,
    solver: SolverKind,
    variant: Option<&str>,
    workdir: &Path,
) -> Result<String, WriteError> {
    project.check_integrity().map_err(WriteError::Integrity)?;
    let variant = resolve_variant(project, variant)?;
    let ids = SolverIds::assign(project)?;
    // Refuse a group in two scene receivers or two fitting zones here too, so that a config is
    // only ever written for a project whose scene mesh can be.
    group_zone_ids(project, &ids)?;
    let wd = working_directory(workdir)?;
    let staged = directivity_files(project);
    let sep = if cfg!(windows) { '\\' } else { '/' };
    let dir = |name: &str| format!("{name}{sep}");

    let mut x = Xml::new();
    x.open(
        "configuration",
        vec![("workingdirectory", text("workingdirectory", &wd)?)],
    );

    // simulation, and freq_enum inside it.
    let mut sim = vec![
        ("modelName", names::SCENE_MESH.to_string()),
        ("tetrameshFileName", names::TETRA_MESH.to_string()),
        ("recepteurss_directory", dir(names::SURFACE_RECEIVER_DIR)),
        (
            "recepteurss_filename",
            names::SURFACE_RECEIVER_FILE.to_string(),
        ),
        (
            "recepteurss_cut_filename",
            names::CUTTING_PLANE_FILE.to_string(),
        ),
        (
            "receiversp_directory",
            names::POINT_RECEIVER_DIR.to_string(),
        ),
        (
            "receiversp_filename",
            names::POINT_RECEIVER_FILE.to_string(),
        ),
        (
            "receiversp_filename_adv",
            names::POINT_RECEIVER_ADVANCED_FILE.to_string(),
        ),
        ("cumul_filename", names::TOTAL_ENERGY_FILE.to_string()),
        (
            "directivities_directory",
            // SPPS: upstream's GUI writes its folder for every run, a directivity file or not
            // (`e_core_sppscore.h:217`), and the solver stores it (`base_core_configuration.cpp:
            // 96`). TCR: upstream's GUI writes none, so the solver reads "" after printing a line
            // the contract fails (`xml_property_missing`); "" written gives it the same value.
            // With a staged file TCR gets the folder too, so the file it loads is there.
            match solver {
                SolverKind::Tcr if staged.is_empty() => String::new(),
                _ => dir(names::DIRECTIVITY_DIR),
            },
        ),
    ];
    let computed = match solver {
        SolverKind::Spps => {
            let s = &project.solvers.spps;
            sim.extend([
                ("particules_directory", dir(names::SPPS_PARTICLE_DIR)),
                ("particules_filename", names::SPPS_PARTICLE_FILE.to_string()),
                ("stats_filename", names::SPPS_STATS_FILE.to_string()),
                (
                    "duree_simulation",
                    real("SPPS duration", s.duration_s.get())?,
                ),
                ("pasdetemps", real("SPPS time step", s.time_step_s.get())?),
                ("nbparticules", s.particles_per_source.to_string()),
                ("nbparticules_rendu", s.particles_saved.to_string()),
                ("random_seed", s.random_seed.to_string()),
                ("computation_method", s.method.solver_code().to_string()),
                ("abs_atmo_calc", flag(s.air_absorption)),
                ("enc_calc", flag(s.fittings)),
                ("direct_calc", flag(s.direct_field_only)),
                ("trans_calc", flag(s.transmission)),
                (
                    "trans_epsilon",
                    real("SPPS extinction exponent", s.extinction_exponent.get())?,
                ),
                (
                    "rayon_recepteurp",
                    real("SPPS receiver radius", s.receiver_radius_m.get())?,
                ),
                ("surf_receiv_method", s.sound_map.solver_code().to_string()),
                ("output_recs_byfreq", flag(s.sound_maps_per_band)),
                ("output_recp_bysource", flag(s.echogram_per_source)),
                (
                    "save_surface_intersection",
                    flag(s.save_surface_intersections),
                ),
                (
                    "save_receivers_intersection",
                    flag(s.save_receiver_intersections),
                ),
            ]);
            &s.bands_computed
        }
        SolverKind::Tcr => {
            let t = &project.solvers.tcr;
            sim.extend([
                ("direct_recepteurSOutputName", dir(names::TCR_DIRECT_DIR)),
                ("sabine_recepteurSOutputName", dir(names::TCR_SABINE_DIR)),
                ("eyring_recepteurSOutputName", dir(names::TCR_EYRING_DIR)),
                ("abs_atmo_calc", flag(t.air_absorption)),
                // TCR tests the attribute's presence, not its value (TC_CalculationCore.cpp:354).
                ("output_recs_byfreq", flag(true)),
            ]);
            &t.bands_computed
        }
    };
    let sim = sim
        .into_iter()
        .map(|(k, v)| Ok((k, text(k, &v)?)))
        .collect::<Result<Vec<_>, WriteError>>()?;
    x.open("simulation", sim);
    x.open("freq_enum", vec![]);
    for (f, &on) in project.bands.frequencies_hz.iter().zip(computed) {
        x.empty("bfreq", vec![("freq", f.to_string()), ("docalc", flag(on))]);
    }
    x.close("freq_enum");
    x.close("simulation");

    // condition_atmospherique.
    let env = &project.environment;
    let absatmo = env.air_absorption.solver_absatmo();
    x.empty(
        "condition_atmospherique",
        vec![
            ("temperature", real("temperature", env.temperature_c.get())?),
            (
                "humidite",
                real("relative humidity", env.relative_humidity_percent.get())?,
            ),
            ("pression", real("pressure", env.pressure_pa.get())?),
            (
                "z0",
                real("ground roughness", env.ground_roughness_m.get())?,
            ),
            (
                "alog",
                real("log celerity gradient", env.celerity_gradient_log.get())?,
            ),
            (
                "blin",
                real("linear celerity gradient", env.celerity_gradient_lin.get())?,
            ),
            ("disable_absatmo_computation", flag(absatmo.is_some())),
            ("absatmo", real("air absorption", absatmo.unwrap_or(0.0))?),
        ],
    );

    // surface_absorption_enum: one type_surface per surface-group material id.
    x.open("surface_absorption_enum", vec![]);
    let mut declared: Vec<(u32, &Material, &str)> = Vec::new();
    for g in &project.surface_groups {
        let sid = ids.material_id(g.id).expect("every group has an id");
        let m = project
            .effective_material(g.id, variant)
            .and_then(|m| project.material(m))
            .expect("integrity: groups, variants and materials resolve");
        if let Some(&(_, earlier, first)) = declared.iter().find(|(id, _, _)| *id == sid) {
            if earlier.id != m.id {
                return Err(WriteError::SharedSolverId {
                    solver_id: sid,
                    first: first.to_string(),
                    second: g.name.clone(),
                });
            }
            continue;
        }
        declared.push((sid, m, &g.name));
        write_material(&mut x, project, sid, m)?;
    }
    // The drawn zones' triangles of the scene mesh carry upstream's default material, id 0
    // (`scene_mesh`), which upstream's GUI declares in every config.xml: declared here whenever
    // they exist, as reference material 0, unless a surface group already declares id 0 (a
    // project pinned to a solver mesh's ids can), whose declaration they then share.
    if has_drawn_zones(project)
        && !declared
            .iter()
            .any(|(id, _, _)| *id == DRAWN_ZONE_MATERIAL_ID)
    {
        let default = default_material(MaterialId(uuid::Uuid::nil()), project.bands.len());
        write_material(&mut x, project, DRAWN_ZONE_MATERIAL_ID, &default)?;
    }
    x.close("surface_absorption_enum");

    // The lists below are written last item first, as upstream's GUI writes them: it creates each
    // child with `new wxXmlNode(parent, ...)`, which puts the new node first among its parent's
    // children (`e_scene_sources_source.h:113`, `e_scene_recepteursp_recepteur.h:111`,
    // `e_scene_recepteurss_recepteur.h:119`, `e_scene_encombrements_encombrement_*.h`). The solvers
    // number sources and receivers by their position in the file, so this order is part of their
    // input: a seeded SPPS run draws its particles source by source in it.

    // sources.
    x.open("sources", vec![]);
    for s in project.sources.iter().filter(|s| s.enabled).rev() {
        write_source(&mut x, project, s, &staged)?;
    }
    x.close("sources");

    // recepteursp.
    x.open("recepteursp", vec![]);
    for r in project.point_receivers.iter().rev() {
        let what = |f: &str| format!("point receiver '{}' {f}", r.name);
        let id = ids
            .point_receiver_id(r.id)
            .expect("every receiver has an id");
        let [px, py, pz] = r.position.to_array();
        let [u, v, w] = r.orientation.to_array();
        x.open(
            "recepteur_ponctuel",
            vec![
                ("id", id.to_string()),
                ("lbl", text(&what("name"), &r.name)?),
                ("x", real(&what("x"), px)?),
                ("y", real(&what("y"), py)?),
                ("z", real(&what("z"), pz)?),
                ("u", real(&what("u"), u)?),
                ("v", real(&what("v"), v)?),
                ("w", real(&what("w"), w)?),
            ],
        );
        // No background noise is written as 0 dB in every band: the value the solver's memset
        // gives a receiver with no entries (coreTypes.cpp:36-44), written so every band is
        // explicit.
        let levels = match &r.background_noise {
            Some(noise) => band_levels_written(noise, &project.bands).ok_or_else(|| {
                WriteError::Unsupported {
                    what: what("background noise"),
                    reason: "its spectrum does not fit the band set".to_string(),
                }
            })?,
            None => vec![0.0; project.bands.len()],
        };
        for (f, db) in project.bands.frequencies_hz.iter().zip(levels) {
            x.empty(
                "bfreq",
                vec![
                    ("freq", f.to_string()),
                    ("db", real(&what(&format!("{f} Hz background noise")), db)?),
                ],
            );
        }
        x.close("recepteur_ponctuel");
    }
    x.close("recepteursp");

    // recepteurss.
    x.open("recepteurss", vec![]);
    for r in project.surface_receivers.iter().filter(|r| r.enabled).rev() {
        let what = |f: &str| format!("surface receiver '{}' {f}", r.name);
        let id = ids
            .surface_receiver_id(r.id)
            .expect("every receiver has an id");
        let mut attrs = vec![
            ("id", id.to_string()),
            ("name", text(&what("name"), &r.name)?),
        ];
        match &r.shape {
            SurfaceReceiverShape::Scene { .. } => x.empty("recepteur_surfacique", attrs),
            SurfaceReceiverShape::CuttingPlane {
                a,
                b,
                c,
                resolution_m,
            } => {
                const KEYS: [[&str; 3]; 3] =
                    [["ax", "ay", "az"], ["bx", "by", "bz"], ["cx", "cy", "cz"]];
                for (keys, p) in KEYS.iter().zip([a, b, c]) {
                    for (&key, v) in keys.iter().zip(p.to_array()) {
                        attrs.push((key, real(&what(key), v)?));
                    }
                }
                attrs.push(("resolution", real(&what("resolution"), resolution_m.get())?));
                x.empty("recepteur_surfacique_coupe", attrs);
            }
        }
    }
    x.close("recepteurss");

    // encombrement_enum.
    x.open("encombrement_enum", vec![]);
    for z in project.fitting_zones.iter().filter(|z| z.enabled).rev() {
        let id = ids.fitting_zone_id(z.id).expect("every zone has an id");
        write_fitting(&mut x, project, id, z)?;
    }
    x.close("encombrement_enum");

    x.close("configuration");
    Ok(x.finish())
}

/// [`write()`], saved as [`names::CONFIG`] in `workdir`, which must exist. Returns the file's path.
pub fn write_file(
    project: &Project,
    solver: SolverKind,
    variant: Option<&str>,
    workdir: &Path,
) -> Result<PathBuf, WriteError> {
    let text = write(project, solver, variant, workdir)?;
    let path = workdir.join(names::CONFIG);
    std::fs::write(&path, text.as_bytes()).map_err(WriteError::Io)?;
    Ok(path)
}

fn write_material(
    x: &mut Xml,
    project: &Project,
    sid: u32,
    m: &Material,
) -> Result<(), WriteError> {
    let what = |f: &str| format!("material '{}' {f}", m.name);
    if let Some(band) = m.reflection_law.first_band_with(ReflectionLaw::SemiDiffuse) {
        return Err(WriteError::Unsupported {
            what: what("reflection law"),
            reason: format!(
                "semi-diffuse (loi 6, at {} Hz) has no case in SPPS, which reflects it \
                 specularly (dotreflection.h:23-45); config_value_format allows loi 0 to 5",
                project.bands.frequencies_hz.get(band).copied().unwrap_or(0)
            ),
        });
    }
    x.open(
        "type_surface",
        vec![
            ("id", sid.to_string()),
            ("side_material", flag(m.double_sided)),
        ],
    );
    for (i, f) in project.bands.frequencies_hz.iter().enumerate() {
        let loi = m
            .reflection_law
            .at(i)
            .expect("integrity: one law per band")
            .solver_code()
            .to_string();
        let mut attrs = vec![
            ("freq", f.to_string()),
            (
                "absorb",
                real(&what(&format!("{f} Hz absorption")), m.absorption[i].get())?,
            ),
            (
                "diffusion",
                real(&what(&format!("{f} Hz scattering")), m.scattering[i].get())?,
            ),
            ("loi", loi),
        ];
        // Only in a band that transmits: upstream's GUI leaves the attribute out of a band whose
        // switch is off (`e_data_row_materiau.h:98-106`).
        if let Some(tl) = m.transmission_loss_db.as_ref().and_then(|t| t[i])
            && let Some(loss) = transmission_loss_written(tl.get(), m.absorption[i].get())
        {
            attrs.push((
                "affaiblissement",
                real(&what(&format!("{f} Hz transmission loss")), loss)?,
            ));
        }
        x.empty("bfreq", attrs);
    }
    x.close("type_surface");
    Ok(())
}

/// The `affaiblissement` written for one band of a transmitting material, or `None` for no
/// attribute: the loss in dB with which no more energy is transmitted than absorbed, as upstream's
/// GUI enforces (`isimpa/data_manager/e_data_row_materiau.h:126-145`): when
/// tau = 10^(-R/10) exceeds alpha, R becomes -10 log10(alpha), so tau = alpha exactly. With alpha
/// 0 (as the solver's `f32`) upstream turns the band's `transmission` off, and its GUI then leaves
/// `affaiblissement` out of that band (`e_data_row_materiau.h:98-106`); the solvers read the
/// attribute per band (`base_core_configuration.cpp:210-219`), so leaving it out is exactly
/// upstream's input. The validator warns about both (`material_transmission_exceeds_absorption`).
pub(crate) fn transmission_loss_written(loss_db: f64, absorption: f64) -> Option<f64> {
    if absorption as f32 <= 0.0 {
        return None;
    }
    Some(if 10f64.powf(-loss_db / 10.0) > absorption {
        -10.0 * absorption.log10()
    } else {
        loss_db
    })
}

/// The band levels, in dB, that [`write()`] writes for a spectrum (a source's power or a point
/// receiver's background noise): each is written as its shortest decimal, which the solver reads
/// back to the same `f32`.
///
/// On upstream's own band set, the 27 third-octave bands from 50 Hz to 20 kHz, a
/// [`SpectrumShape::White`] or [`SpectrumShape::Pink`] spectrum is computed the way upstream's GUI
/// computes its "White noise" and "Pink noise" reference spectra at every write
/// (`E_Property_Freq::LoadLwFromBdd` and `SetGlobalLevel`, `generic_element/e_property_freq.cpp`):
/// in `f32`, from the reference levels rounded to 2 decimals in `appconst.xml`, so the solver
/// gets upstream's exact `f32`. [`Spectrum::band_levels_db`] (in `f64`) can differ from it by one
/// unit in the last place: it does in 2 of tutorial 1's 27 background-noise bands. Every other
/// spectrum, and every spectrum on another band set, is [`Spectrum::band_levels_db`]. `None` when
/// the spectrum does not fit the band set.
pub fn band_levels_written(spectrum: &Spectrum, bands: &BandSet) -> Option<Vec<f64>> {
    let reference_id = match spectrum.shape {
        SpectrumShape::White => Some(0),
        SpectrumShape::Pink => Some(1),
        SpectrumShape::Custom { .. } => None,
    };
    let reference = reference_id
        .filter(|_| is_upstream_band_set(bands))
        .and_then(|id| REFERENCE_SPECTRA.iter().find(|r| r.id == id));
    match reference {
        Some(r) => Some(
            upstream_band_levels(&r.band_db, spectrum.global_db.get() as f32)
                .iter()
                .map(|&l| widen_f32(l))
                .collect(),
        ),
        None => spectrum.band_levels_db(bands),
    }
}

/// Upstream's frequency list (`ApplicationConfiguration::GetAllFrequencies`): every third-octave
/// band from 50 Hz to 20 kHz, the bands its reference spectra are given on.
fn is_upstream_band_set(bands: &BandSet) -> bool {
    bands.kind == BandKind::ThirdOctave
        && bands.frequencies_hz == BandKind::ThirdOctave.nominal_frequencies()
        && bands.len() == 27
}

/// What upstream's GUI writes for a reference spectrum at global level `lw`, with no attenuation
/// (`e_property_freq.cpp:139-176`, `:223-227`, `:256-272` and `:331-358`,
/// `e_data_row_ext_bandefreq.h:108-111`):
/// the band levels start as the reference's, their energetic sum is formed in `f32` from `f64`
/// powers (`totdb += pow(10, lw / 10)`, then `10 * log10f(totdb)`), and every band is moved by
/// `lw` minus that sum, in `f32`. Measured: this reproduces all 39 source and receiver spectra of
/// the five runs stored in upstream's tutorials, bit for bit (`tests/parity_inputs.rs`).
fn upstream_band_levels(reference: &[f32; 27], lw: f32) -> [f32; 27] {
    let mut total = 0.0f32;
    for &r in reference {
        total = (f64::from(total) + 10f64.powf(f64::from(r / 10.0))) as f32;
    }
    let global = 10.0f32 * total.log10();
    let delta = lw - global;
    reference.map(|r| r + delta)
}

fn write_source(
    x: &mut Xml,
    project: &Project,
    s: &Source,
    staged: &[StagedFile],
) -> Result<(), WriteError> {
    let what = |f: &str| format!("source '{}' {f}", s.name);
    let [px, py, pz] = s.position.to_array();
    // Upstream's element id, when the source pins one (a `.proj` import, `docs/m5-m6-design.md`,
    // decision 13), first as upstream's GUI writes it (`e_scene_sources_source.h:114`). The
    // solvers never read it: they number sources by position (`base_core_configuration.cpp:153`).
    // It is written so that an imported project's config.xml carries every id upstream's does;
    // a source made here pins none, and none is written.
    let mut attrs = Vec::new();
    if let Some(id) = s.solver_id {
        attrs.push(("id", id.to_string()));
    }
    attrs.extend([
        ("name", text(&what("name"), &s.name)?),
        ("x", real(&what("x"), px)?),
        ("y", real(&what("y"), py)?),
        ("z", real(&what("z"), pz)?),
        ("directivite", s.directivity.solver_code().to_string()),
        ("delay", real(&what("delay"), s.delay_s.get())?),
    ]);
    // u, v, w are read only for types 1 and 5 (base_core_configuration.cpp:135-139).
    if let Some(d) = s.directivity.direction() {
        let [u, v, w] = d.to_array();
        attrs.push(("u", real(&what("u"), u)?));
        attrs.push(("v", real(&what("v"), v)?));
        attrs.push(("w", real(&what("w"), w)?));
    }
    if let Directivity::Balloon { file, .. } = &s.directivity {
        let run_name = &staged
            .iter()
            .find(|f| &f.project_path == file)
            .expect("every enabled balloon source's file is staged")
            .run_name;
        if run_name.is_empty() {
            return Err(WriteError::Unsupported {
                what: what("directivity file"),
                reason: "the file name is empty; without it SPPS crashes (P2 dir_noattr)"
                    .to_string(),
            });
        }
        attrs.push((
            "directivity_file",
            text(&what("directivity file"), run_name)?,
        ));
    }
    x.open("source", attrs);
    let levels =
        band_levels_written(&s.power, &project.bands).ok_or_else(|| WriteError::Unsupported {
            what: what("power"),
            reason: "its spectrum does not fit the band set".to_string(),
        })?;
    for (f, db) in project.bands.frequencies_hz.iter().zip(levels) {
        x.empty(
            "bfreq",
            vec![
                ("freq", f.to_string()),
                ("db", real(&what(&format!("{f} Hz power")), db)?),
            ],
        );
    }
    x.close("source");
    Ok(())
}

fn write_fitting(
    x: &mut Xml,
    project: &Project,
    id: i32,
    z: &FittingZone,
) -> Result<(), WriteError> {
    let what = |f: &str| format!("fitting zone '{}' {f}", z.name);
    x.open("encombrement", vec![("id", id.to_string())]);
    for (i, f) in project.bands.frequencies_hz.iter().enumerate() {
        x.empty(
            "bfreq",
            vec![
                ("freq", f.to_string()),
                (
                    "alpha",
                    real(&what(&format!("{f} Hz absorption")), z.absorption[i].get())?,
                ),
                (
                    "lambda",
                    real(
                        &what(&format!("{f} Hz mean free path")),
                        z.mean_free_path_m[i].get(),
                    )?,
                ),
                ("loi_diff", z.diffusion_law[i].solver_code().to_string()),
            ],
        );
    }
    x.close("encombrement");
    Ok(())
}

fn flag(on: bool) -> String {
    if on { "1" } else { "0" }.to_string()
}

fn real(what: &str, v: f64) -> Result<String, WriteError> {
    real_text(v).ok_or_else(|| WriteError::NonFinite {
        what: what.to_string(),
    })
}

/// `s` escaped for a double-quoted XML attribute value.
fn text(what: &str, s: &str) -> Result<String, WriteError> {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\t' => out.push_str("&#9;"),
            '\n' => out.push_str("&#10;"),
            '\r' => out.push_str("&#13;"),
            c if u32::from(c) < 0x20 || c == '\u{FFFE}' || c == '\u{FFFF}' => {
                return Err(WriteError::Unencodable {
                    what: what.to_string(),
                    ch: c,
                });
            }
            c => out.push(c),
        }
    }
    Ok(out)
}

/// A minimal pretty-printing XML writer. Attribute values are already escaped. An element
/// closed with no children is written self-closing (`<sources/>`).
struct Xml {
    out: String,
    /// Per open element: the length of `out` right after its start tag.
    open: Vec<usize>,
}

impl Xml {
    fn new() -> Self {
        Xml {
            out: String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n"),
            open: Vec::new(),
        }
    }

    fn indent(&mut self) {
        for _ in 0..self.open.len() {
            self.out.push_str("  ");
        }
    }

    fn tag(&mut self, name: &str, attrs: Vec<(&'static str, String)>, close: &str) {
        self.indent();
        self.out.push('<');
        self.out.push_str(name);
        for (k, v) in attrs {
            self.out.push(' ');
            self.out.push_str(k);
            self.out.push_str("=\"");
            self.out.push_str(&v);
            self.out.push('"');
        }
        self.out.push_str(close);
        self.out.push('\n');
    }

    fn open(&mut self, name: &str, attrs: Vec<(&'static str, String)>) {
        self.tag(name, attrs, ">");
        self.open.push(self.out.len());
    }

    fn empty(&mut self, name: &str, attrs: Vec<(&'static str, String)>) {
        self.tag(name, attrs, "/>");
    }

    fn close(&mut self, name: &str) {
        let after_start = self.open.pop().expect("close matches an open");
        if self.out.len() == after_start {
            // No children: turn `<name ...>\n` into `<name .../>\n`.
            self.out.truncate(after_start - 2);
            self.out.push_str("/>\n");
            return;
        }
        self.indent();
        self.out.push_str("</");
        self.out.push_str(name);
        self.out.push_str(">\n");
    }

    fn finish(self) -> String {
        debug_assert!(self.open.is_empty());
        self.out
    }
}

#[cfg(test)]
mod spectrum_tests {
    use super::band_levels_written;
    use crate::schema::{BandKind, BandSet, Spectrum, SpectrumShape};

    /// The `f32` a value of upstream's config.xml reads as.
    fn f(text: &str) -> u32 {
        (text.parse::<f64>().unwrap() as f32).to_bits()
    }

    #[test]
    fn white_noise_is_upstreams_f32_in_every_band() {
        // Tutorial 1's GUI-written config (tests/fixtures/upstream/tutorial1/spps/config.xml):
        // the source is white noise at 80 dB, the receivers' background noise white at 0 dB.
        let bands = BandSet::range(BandKind::ThirdOctave, 50, 20000).unwrap();
        let at = |s: &Spectrum, hz: u32| -> (u32, u32) {
            let i = bands.index_of(hz).unwrap();
            let written = band_levels_written(s, &bands).unwrap()[i] as f32;
            let exact = s.band_levels_db(&bands).unwrap()[i] as f32;
            (written.to_bits(), exact.to_bits())
        };
        let source = Spectrum::new(80.0, SpectrumShape::White);
        let noise = Spectrum::new(0.0, SpectrumShape::White);
        for (s, hz, upstream) in [
            (&source, 50, "47.1404190063477"),
            (&source, 1000, "60.1404190063477"),
            (&source, 20000, "73.1404190063477"),
            (&noise, 50, "-32.8595809936523"),
            (&noise, 1000, "-19.8595790863037"),
            (&noise, 16000, "-7.85957956314087"),
            (&noise, 20000, "-6.85957956314087"),
        ] {
            assert_eq!(at(s, hz).0, f(upstream), "{hz} Hz, {s:?}");
        }
        // The f64 formula misses upstream's float in two of these bands, by one unit in the last
        // place: that is why white and pink are computed upstream's way.
        assert_ne!(at(&noise, 16000).1, f("-7.85957956314087"));
        assert_ne!(at(&noise, 20000).1, f("-6.85957956314087"));
    }

    #[test]
    fn other_band_sets_and_custom_shapes_keep_the_f64_formula() {
        let octaves = BandSet::range(BandKind::Octave, 63, 16000).unwrap();
        let fewer = BandSet::range(BandKind::ThirdOctave, 100, 5000).unwrap();
        for bands in [&octaves, &fewer] {
            for shape in [SpectrumShape::White, SpectrumShape::Pink] {
                let s = Spectrum::new(80.0, shape);
                assert_eq!(band_levels_written(&s, bands), s.band_levels_db(bands));
            }
        }
        let all = BandSet::range(BandKind::ThirdOctave, 50, 20000).unwrap();
        let custom = Spectrum::new(
            70.0,
            SpectrumShape::Custom {
                relative_db: (0..27).map(|i| crate::schema::F64::new(i as f64)).collect(),
            },
        );
        assert_eq!(
            band_levels_written(&custom, &all),
            custom.band_levels_db(&all)
        );
    }
}

#[cfg(test)]
mod transmission_tests {
    use super::transmission_loss_written as w;

    #[test]
    fn clamps_transmission_to_absorption_like_upstream() {
        // tau = 10^(-5/10) = 0.316 > alpha 0.1, so the loss becomes 10 dB (tau = 0.1).
        assert!((w(5.0, 0.1).unwrap() - 10.0).abs() < 1e-12);
        // tau = 10^(-20/10) = 0.01 <= alpha 0.1: written as entered.
        assert_eq!(w(20.0, 0.1), Some(20.0));
        // Exactly at the limit: unchanged.
        assert_eq!(w(10.0, 0.1), Some(10.0));
        // No absorption, no transmission: no attribute, as upstream's GUI writes the band.
        assert_eq!(w(5.0, 0.0), None);
        assert_eq!(w(400.0, 0.0), None);
        assert_eq!(w(5.0, 1e-50), None, "0 as the solver's f32");
    }
}
