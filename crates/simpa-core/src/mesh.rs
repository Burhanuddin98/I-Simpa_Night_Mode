//! Meshing: the `.poly` and `.var` TetGen reads, the `Mesher` trait with its `tetgen.exe`
//! backend, the `.mbin` built from TetGen's output, the mesh manifest and mesh verification.
//! Milestone M5; see `docs/m5-m6-design.md`.
//!
//! Three entry points, each writing into one mesh folder and returning its [`MeshManifest`]:
//! - [`mesh_project`] meshes a project with its own [`MeshSettings`];
//! - [`mesh_poly`] meshes a raw `.poly`, whose facets then act as the scene;
//! - [`mesh_from_tetgen`] builds the `.mbin` from TetGen output made elsewhere.
//!
//! The mesh folder holds `scene_mesh.poly`, `scene_mesh.var` (when the settings ask for one),
//! TetGen's `scene_mesh.1.*` (or `scene_mesh_skipped.*`), `tetgen.stdout.txt` and
//! `tetgen.stderr.txt`, `mesh.cbin`, `tetramesh.mbin` **only when meshing succeeded**, `mesh.json`
//! always, and `diag/` after skipped facets. Every file a mesh writes, and every file TetGen
//! would read beside the `.poly` (`.var`, `.edge`, `.mtr`: `tetgen.cxx:2446-2449`), is deleted
//! before meshing starts ([`delete_stale`]), so nothing from an earlier mesh is silently reused.
//!
//! The outcome carries every reason code that applies ([`codes`]), and `status` is `OK` exactly
//! when there is none. A `.mbin` is written only after [`verify::verify_mesh`] passes it.
//! Specified in `docs/formats/mesh-manifest.md`.

use std::io;
use std::path::{Path, PathBuf};
use std::time::Instant;

use crate::config_xml::names;
use crate::formats::{cbin, mbin, poly, tetgen as tetgen_files};
use crate::process::{CancelToken, Line, Outcome, Stream};
use crate::schema::{MeshSettings, Project};

mod build;
mod diag;
mod flags;
mod input;
mod manifest;
mod tetgen;
pub mod verify;

pub use build::{AttributeMap, BuildStats, FACE_CORNERS, OutputPaths, TetgenOutput, build_mbin};
pub use diag::{Element, Intersection, intersections};
pub use flags::{
    TetgenCommand, format_g, settings_conflict, tetgen_flags, to_string_g15, trailer_command,
};
pub use input::{
    InputError, MeshInput, ZoneFacets, poly_input, project_input, refined_faces, var_bytes,
};
pub use manifest::{
    Counts, Diagnosis, Hashes, MANIFEST_FILE, MANIFEST_VERSION, MeshManifest, MeshSource,
    MeshStatus, SkippedFacet, TetgenCall, read_manifest, sha256_file, sha256_hex, write_manifest,
};
pub use tetgen::{Mesher, STDERR_LOG, STDOUT_LOG, TetgenMesher};

/// TetGen's input basename in a mesh folder, upstream's own (`projet_maillage.cpp:156-157`).
pub const BASENAME: &str = "scene_mesh";
/// The `.poly` in a mesh folder.
pub const POLY_FILE: &str = "scene_mesh.poly";
/// The `.var` in a mesh folder.
pub const VAR_FILE: &str = "scene_mesh.var";
/// The folder of the `tetgen -d` follow-up, inside a mesh folder.
pub const DIAG_DIR: &str = "diag";

/// The mesher's reason codes, snake_case like the validator's (`docs/m5-m6-design.md`,
/// decision 8). Documented in `docs/formats/mesh-manifest.md`.
pub mod codes {
    /// The settings ask for a `.var` and `-Y` together (the validator's rule of the same name).
    pub const MESH_SETTINGS_CONFLICT: &str = "mesh_settings_conflict";
    /// The project or `.poly` cannot be turned into TetGen input.
    pub const INPUT_INVALID: &str = "input_invalid";
    /// TetGen could not be started, or its output could not be logged.
    pub const TETGEN_LAUNCH_FAILED: &str = "tetgen_launch_failed";
    /// Meshing was cancelled.
    pub const CANCELLED: &str = "cancelled";
    /// TetGen's exit code is an NTSTATUS error (0xC0000000 and up).
    pub const TETGEN_CRASH: &str = "tetgen_crash";
    /// TetGen exited with a code other than 0.
    pub const TETGEN_EXIT_NONZERO: &str = "tetgen_exit_nonzero";
    /// TetGen skipped input facets as self-intersecting (`<base>_skipped.face` has rows).
    pub const TETGEN_SKIPPED_FACETS: &str = "tetgen_skipped_facets";
    /// `.1.node`, `.1.ele` or `.1.face` is missing.
    pub const TETGEN_OUTPUT_MISSING: &str = "tetgen_output_missing";
    /// `.1.neigh` is missing. There is no fallback that computes neighbours.
    pub const NEIGH_MISSING: &str = "neigh_missing";
    /// TetGen's files cannot be read, or disagree with each other or with the scene.
    pub const TETGEN_OUTPUT_INVALID: &str = "tetgen_output_invalid";
    /// The `.mbin` built fails [`super::verify::verify_mesh`]; its own codes follow.
    pub const MESH_INVALID: &str = "mesh_invalid";
    /// The `.mbin` could not be written.
    pub const MBIN_WRITE_FAILED: &str = "mbin_write_failed";
}

/// A failure to use the mesh folder at all: no manifest could be written.
#[derive(Debug)]
pub enum MeshError {
    Io { path: PathBuf, source: io::Error },
}

impl std::fmt::Display for MeshError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MeshError::Io { path, source } => write!(f, "{}: {source}", path.display()),
        }
    }
}

impl std::error::Error for MeshError {}

fn io_at(path: &Path) -> impl FnOnce(io::Error) -> MeshError + '_ {
    move |source| MeshError::Io {
        path: path.to_path_buf(),
        source,
    }
}

/// Whether `name` is a file a mesh writes, or one TetGen reads beside `scene_mesh.poly`.
fn is_stale(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    let base = BASENAME;
    [
        POLY_FILE,
        VAR_FILE,
        "scene_mesh.edge",
        "scene_mesh.mtr",
        names::TETRA_MESH,
        names::SCENE_MESH,
        MANIFEST_FILE,
        STDOUT_LOG,
        STDERR_LOG,
    ]
    .contains(&n.as_str())
        || n.starts_with(&format!("{base}.1."))
        || n.starts_with(&format!("{base}_skipped."))
}

/// Deletes from `dir` every file an earlier mesh may have left, and the `diag/` folder:
/// `scene_mesh.{poly,var,edge,mtr}`, `scene_mesh.1.*`, `scene_mesh_skipped.*`, `tetramesh.mbin`,
/// `mesh.cbin`, `mesh.json` and the TetGen logs. Other files are left alone. Returns the names
/// deleted.
pub fn delete_stale(dir: &Path) -> Result<Vec<String>, MeshError> {
    let mut deleted = Vec::new();
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(deleted),
        Err(e) => return Err(io_at(dir)(e)),
    };
    for entry in entries {
        let entry = entry.map_err(io_at(dir))?;
        let name = entry.file_name().to_string_lossy().into_owned();
        let path = entry.path();
        let kind = entry.file_type().map_err(io_at(&path))?;
        if kind.is_dir() && name.eq_ignore_ascii_case(DIAG_DIR) {
            std::fs::remove_dir_all(&path).map_err(io_at(&path))?;
            deleted.push(name);
        } else if !kind.is_dir() && is_stale(&name) {
            std::fs::remove_file(&path).map_err(io_at(&path))?;
            deleted.push(name);
        }
    }
    deleted.sort();
    Ok(deleted)
}

fn new_manifest(source: MeshSource) -> MeshManifest {
    MeshManifest {
        manifest_version: MANIFEST_VERSION,
        simpa_core: crate::VERSION.to_string(),
        source,
        input_path: None,
        status: MeshStatus::Fail,
        codes: Vec::new(),
        messages: Vec::new(),
        mesh_input_hash: None,
        tetgen: None,
        files: Hashes::default(),
        counts: Counts::default(),
        volume_ids: verify::VolumeIds::default(),
        zone_facets: Vec::new(),
        skipped_rows: 0,
        skipped_facets: Vec::new(),
        diagnosis: None,
        verify: None,
        elapsed_ms: 0.0,
    }
}

fn fail(m: &mut MeshManifest, code: &str, message: impl Into<String>) {
    if !m.codes.iter().any(|c| c == code) {
        m.codes.push(code.to_string());
    }
    m.messages.push(message.into());
}

/// Sets the status from the codes, stamps the time and writes `mesh.json`.
fn finish(dir: &Path, mut m: MeshManifest, start: Instant) -> Result<MeshManifest, MeshError> {
    m.status = if m.codes.iter().any(|c| c == codes::CANCELLED) {
        MeshStatus::Cancelled
    } else if m.codes.is_empty() {
        MeshStatus::Ok
    } else {
        MeshStatus::Fail
    };
    m.elapsed_ms = start.elapsed().as_secs_f64() * 1e3;
    write_manifest(dir, &m).map_err(io_at(&dir.join(MANIFEST_FILE)))?;
    Ok(m)
}

fn prepare(dir: &Path) -> Result<(), MeshError> {
    std::fs::create_dir_all(dir).map_err(io_at(dir))?;
    delete_stale(dir).map(|_| ())
}

/// Meshes `project` into `out_dir` with its own [`MeshSettings`] (flags from [`tetgen_flags`]).
/// Stale files go first ([`delete_stale`]). A settings conflict or a project that cannot be
/// expressed as TetGen input fails before TetGen runs. On skipped facets a `tetgen -d` follow-up
/// runs in `<out_dir>/diag/` and its findings go in the manifest. `on_line` sees TetGen's output
/// as it arrives; `cancel` stops TetGen and leaves no `.mbin`.
///
/// `Err` only when the folder itself cannot be used; every other failure is in the manifest.
pub fn mesh_project(
    project: &Project,
    out_dir: &Path,
    mesher: &dyn Mesher,
    cancel: &CancelToken,
    on_line: &mut dyn FnMut(&Line),
) -> Result<MeshManifest, MeshError> {
    let start = Instant::now();
    prepare(out_dir)?;
    let mut m = new_manifest(MeshSource::Project);
    m.mesh_input_hash = Some(crate::validate::mesh_input_hash(project));
    let settings = &project.solvers.meshing;
    if settings_conflict(settings) {
        fail(
            &mut m,
            codes::MESH_SETTINGS_CONFLICT,
            "the settings ask for a surface-receiver area constraint (.var) and -Y \
             (preserve_boundary) together; -Y forbids the facet splits the constraint asks for",
        );
        return finish(out_dir, m, start);
    }
    match project_input(project) {
        Ok(input) => run_input(&input, settings, out_dir, mesher, cancel, on_line, m, start),
        Err(e) => {
            fail(&mut m, codes::INPUT_INVALID, e.0);
            finish(out_dir, m, start)
        }
    }
}

/// Meshes the raw `.poly` at `poly_path` into `out_dir` with `settings`' flags; its facets act as
/// the scene ([`poly_input`]). Otherwise as [`mesh_project`], with no mesh input hash.
pub fn mesh_poly(
    poly_path: &Path,
    settings: &MeshSettings,
    out_dir: &Path,
    mesher: &dyn Mesher,
    cancel: &CancelToken,
    on_line: &mut dyn FnMut(&Line),
) -> Result<MeshManifest, MeshError> {
    let start = Instant::now();
    prepare(out_dir)?;
    let mut m = new_manifest(MeshSource::Poly);
    m.input_path = Some(poly_path.display().to_string());
    if settings.surface_receiver_max_area_m2.is_some() {
        m.messages
            .push("a raw .poly has no surface receivers: no .var is written".to_string());
    }
    let input = poly::read_file(poly_path)
        .map_err(|e| InputError(format!("{}: {e}", poly_path.display())))
        .and_then(|model| poly_input(&model));
    match input {
        Ok(input) => run_input(&input, settings, out_dir, mesher, cancel, on_line, m, start),
        Err(e) => {
            fail(&mut m, codes::INPUT_INVALID, e.0);
            finish(out_dir, m, start)
        }
    }
}

fn scene_counts(m: &mut MeshManifest, input: &MeshInput) {
    m.counts.scene_vertices = input.scene.vertices.len();
    m.counts.scene_faces = input.scene.faces.len();
    m.counts.poly_vertices = input.poly.model_vertices.len();
    m.counts.poly_facets = input.poly.model_faces.len();
    m.counts.regions = input.poly.model_regions.len();
    m.counts.var_constraints = input.var_markers.len();
    m.volume_ids = input.volume_ids.clone();
    m.zone_facets = input.zone_facets.clone();
    m.messages.extend(input.notes.iter().cloned());
}

fn write_file(dir: &Path, name: &str, bytes: &[u8]) -> Result<String, MeshError> {
    let path = dir.join(name);
    std::fs::write(&path, bytes).map_err(io_at(&path))?;
    Ok(sha256_hex(bytes))
}

/// Writes `mesh.cbin` with [`cbin::write_file`], which refuses a scene the format cannot hold;
/// that refusal is `input_invalid`, and `Ok(false)`.
fn write_scene(dir: &Path, scene: &cbin::Model, m: &mut MeshManifest) -> Result<bool, MeshError> {
    let path = dir.join(names::SCENE_MESH);
    match cbin::write_file(scene, &path) {
        Ok(()) => {
            m.files.cbin = Some(sha256_file(&path).map_err(io_at(&path))?);
            Ok(true)
        }
        Err(crate::formats::FormatError::Io(e)) => Err(io_at(&path)(e)),
        Err(e) => {
            fail(
                m,
                codes::INPUT_INVALID,
                format!("{}: {e}", names::SCENE_MESH),
            );
            Ok(false)
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn run_input(
    input: &MeshInput,
    settings: &MeshSettings,
    dir: &Path,
    mesher: &dyn Mesher,
    cancel: &CancelToken,
    on_line: &mut dyn FnMut(&Line),
    mut m: MeshManifest,
    start: Instant,
) -> Result<MeshManifest, MeshError> {
    scene_counts(&mut m, input);
    if !write_scene(dir, &input.scene, &mut m)? {
        return finish(dir, m, start);
    }
    let poly_bytes = poly::write(&input.poly);
    m.files.poly = Some(write_file(dir, POLY_FILE, &poly_bytes)?);
    if let Some(var) = &input.var {
        m.files.var = Some(write_file(dir, VAR_FILE, var)?);
    }

    let mut argv = tetgen_flags(settings);
    argv.push(POLY_FILE.to_string());
    let paths = OutputPaths::new(dir, BASENAME);
    if cancel.is_cancelled() {
        fail(
            &mut m,
            codes::CANCELLED,
            "meshing was cancelled before TetGen started",
        );
        classify(&paths, None, input, &mut m);
        return finish(dir, m, start);
    }
    let outcome = match call(
        mesher,
        dir,
        ".",
        &argv,
        cancel,
        on_line,
        &mut m.tetgen,
        None,
    ) {
        Ok(o) => o,
        Err(e) => {
            fail(&mut m, codes::TETGEN_LAUNCH_FAILED, e);
            return finish(dir, m, start);
        }
    };
    let skipped = classify(&paths, Some(&outcome), input, &mut m);
    if skipped && !cancel.is_cancelled() {
        m.diagnosis = diagnose(input, &poly_bytes, dir, mesher, cancel, on_line, &mut m)?;
    }
    if m.codes.is_empty() {
        build_and_write(&paths, input, dir, cancel, &mut m);
    }
    finish(dir, m, start)
}

/// Runs one mesher call and records it in `slot`; `Err` holds why it could not start. `lines`,
/// when given, collects the stdout lines.
#[allow(clippy::too_many_arguments)]
fn call(
    mesher: &dyn Mesher,
    dir: &Path,
    cwd_label: &str,
    argv: &[String],
    cancel: &CancelToken,
    on_line: &mut dyn FnMut(&Line),
    slot: &mut Option<TetgenCall>,
    mut lines: Option<&mut Vec<String>>,
) -> Result<Outcome, String> {
    let program = mesher.program();
    let mut record = TetgenCall {
        program: program.map(|p| p.display().to_string()),
        program_sha256: program.and_then(|p| sha256_file(p).ok()),
        argv: argv.to_vec(),
        cwd: cwd_label.to_string(),
        ..TetgenCall::default()
    };
    let result = mesher.run(dir, argv, cancel, &mut |l: &Line| {
        if let Some(lines) = lines.as_deref_mut()
            && l.stream == Stream::Stdout
        {
            lines.push(l.text.clone());
        }
        on_line(l);
    });
    if let Ok(o) = &result {
        record.exit_code = o.exit_code;
        record.cancelled = o.cancelled;
        record.elapsed_ms = o.elapsed_ms;
    }
    *slot = Some(record);
    result.map_err(|e| e.to_string())
}

/// The codes for one TetGen run's outcome and files, in this order: `cancelled`, `tetgen_crash`,
/// `tetgen_exit_nonzero`, `tetgen_skipped_facets`, `tetgen_output_missing`, `neigh_missing`
/// (and `tetgen_output_invalid` for an unreadable `_skipped.face`). Returns whether facets were
/// skipped.
fn classify(
    paths: &OutputPaths,
    outcome: Option<&Outcome>,
    input: &MeshInput,
    m: &mut MeshManifest,
) -> bool {
    if let Some(o) = outcome {
        if o.cancelled {
            fail(
                m,
                codes::CANCELLED,
                "meshing was cancelled; TetGen was stopped",
            );
        }
        if let Some(code) = o.exit_code {
            if code >= 0xC000_0000 {
                fail(
                    m,
                    codes::TETGEN_CRASH,
                    format!("TetGen crashed with exit code 0x{code:08X}"),
                );
            }
            if code != 0 {
                fail(
                    m,
                    codes::TETGEN_EXIT_NONZERO,
                    format!("TetGen exited with code {code} (0x{code:08X})"),
                );
            }
        }
    }
    let mut skipped = false;
    if paths.skipped_face.is_file() {
        let read = crate::formats::read_file(&paths.skipped_face)
            .and_then(|b| tetgen_files::read_face(&b));
        match read {
            Ok(f) if !f.faces.is_empty() => {
                skipped = true;
                let markers: Vec<i64> = match &f.markers {
                    Some(ms) => ms.iter().map(|&x| i64::from(x)).collect(),
                    None => vec![-1; f.faces.len()],
                };
                m.skipped_rows = markers.len();
                m.skipped_facets = map_skipped(&markers, input);
                let distinct: Vec<String> = m
                    .skipped_facets
                    .iter()
                    .map(|s| s.marker.to_string())
                    .collect();
                fail(
                    m,
                    codes::TETGEN_SKIPPED_FACETS,
                    format!(
                        "TetGen skipped {} input triangles as self-intersecting, facet markers \
                         [{}]; no mesh is built from a partial boundary",
                        markers.len(),
                        distinct.join(", ")
                    ),
                );
            }
            Ok(_) => {}
            Err(e) => fail(
                m,
                codes::TETGEN_OUTPUT_INVALID,
                format!("{}: {e}", paths.skipped_face.display()),
            ),
        }
    }
    let missing = paths.missing_mesh_files();
    if !missing.is_empty() {
        let names: Vec<String> = missing
            .iter()
            .map(|p| {
                p.file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned()
            })
            .collect();
        fail(
            m,
            codes::TETGEN_OUTPUT_MISSING,
            format!("TetGen wrote no {}", names.join(", ")),
        );
    }
    if !paths.neigh.is_file() {
        fail(
            m,
            codes::NEIGH_MISSING,
            format!(
                "TetGen wrote no {}; neighbours are never computed any other way",
                paths
                    .neigh
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
            ),
        );
    }
    skipped
}

/// One entry per distinct skipped marker, mapped to its scene face and group, or its box zone.
fn map_skipped(markers: &[i64], input: &MeshInput) -> Vec<SkippedFacet> {
    let mut distinct = markers.to_vec();
    distinct.sort_unstable();
    distinct.dedup();
    let scene_faces = input.scene.faces.len() as i64;
    distinct
        .into_iter()
        .map(|marker| {
            let scene_face = (0..scene_faces).contains(&marker).then_some(marker as u32);
            let group = scene_face
                .and_then(|f| input.face_groups.get(f as usize))
                .filter(|g| !g.is_empty())
                .cloned();
            let fitting_zone = input
                .zone_facets
                .iter()
                .find(|z| {
                    (i64::from(z.first_marker)..i64::from(z.first_marker) + 12).contains(&marker)
                })
                .map(|z| z.zone.clone());
            SkippedFacet {
                marker,
                scene_face,
                group,
                fitting_zone,
            }
        })
        .collect()
}

/// `tetgen -d` on the same `.poly` (no `.var`) in `<dir>/diag/`: the intersecting pairs it
/// reports, and its own `_skipped.face` markers.
fn diagnose(
    input: &MeshInput,
    poly_bytes: &[u8],
    dir: &Path,
    mesher: &dyn Mesher,
    cancel: &CancelToken,
    on_line: &mut dyn FnMut(&Line),
    m: &mut MeshManifest,
) -> Result<Option<Diagnosis>, MeshError> {
    let diag_dir = dir.join(DIAG_DIR);
    std::fs::create_dir_all(&diag_dir).map_err(io_at(&diag_dir))?;
    write_file(&diag_dir, POLY_FILE, poly_bytes)?;
    let argv = vec!["-d".to_string(), POLY_FILE.to_string()];
    let mut lines = Vec::new();
    let mut slot = None;
    let result = call(
        mesher,
        &diag_dir,
        DIAG_DIR,
        &argv,
        cancel,
        on_line,
        &mut slot,
        Some(&mut lines),
    );
    let call_record = slot.unwrap_or_default();
    match result {
        Ok(o) if o.cancelled => fail(m, codes::CANCELLED, "the tetgen -d follow-up was cancelled"),
        Ok(_) => {}
        Err(e) => m
            .messages
            .push(format!("the tetgen -d follow-up could not run: {e}")),
    }
    let skipped = OutputPaths::new(&diag_dir, BASENAME).skipped_face;
    let skipped_markers = crate::formats::read_file(&skipped)
        .and_then(|b| tetgen_files::read_face(&b))
        .ok()
        .and_then(|f| f.markers)
        .map(|ms| ms.into_iter().map(i64::from).collect())
        .unwrap_or_default();
    Ok(Some(Diagnosis {
        call: call_record,
        skipped_markers,
        intersections: intersections(&lines, &input.poly),
    }))
}

/// Reads TetGen's output, builds the `.mbin`, verifies it and writes it when it passes.
fn build_and_write(
    paths: &OutputPaths,
    input: &MeshInput,
    dir: &Path,
    cancel: &CancelToken,
    m: &mut MeshManifest,
) {
    let built = TetgenOutput::read(paths)
        .and_then(|out| build_mbin(&out, input.scene.faces.len(), &input.volume_ids.fittings));
    let (mesh, stats) = match built {
        Ok(b) => b,
        Err(e) => return fail(m, codes::TETGEN_OUTPUT_INVALID, e),
    };
    m.counts.build = Some(stats);
    let report = verify::verify_mesh(&mesh, &input.scene, &input.volume_ids);
    let passed = report.passed();
    let verify_codes = report.codes.clone();
    m.verify = Some(report);
    if !passed {
        fail(
            m,
            codes::MESH_INVALID,
            format!(
                "the mesh fails verification ({}); no .mbin is written",
                verify_codes.join(", ")
            ),
        );
        for c in verify_codes {
            if !m.codes.contains(&c) {
                m.codes.push(c);
            }
        }
        return;
    }
    if cancel.is_cancelled() {
        return fail(
            m,
            codes::CANCELLED,
            "meshing was cancelled before the .mbin was written",
        );
    }
    let path = dir.join(names::TETRA_MESH);
    match mbin::write_file(&mesh, &path).and_then(|()| Ok(sha256_file(&path)?)) {
        Ok(sha) => m.files.mbin = Some(sha),
        Err(e) => {
            let _ = std::fs::remove_file(&path);
            fail(
                m,
                codes::MBIN_WRITE_FAILED,
                format!("{}: {e}", path.display()),
            );
        }
    }
}

/// The TetGen basename in `dir`: the one `<base>` with a `<base>.1.ele`, or failing that a
/// `<base>.1.node`, `<base>.1.face` or `<base>_skipped.face`.
pub fn find_basename(dir: &Path) -> Result<String, String> {
    let names: Vec<String> = std::fs::read_dir(dir)
        .map_err(|e| format!("{}: {e}", dir.display()))?
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_ok_and(|t| t.is_file()))
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    for suffixes in [
        &[".1.ele"][..],
        &[".1.node", ".1.face", "_skipped.face"][..],
    ] {
        let mut bases: Vec<String> = names
            .iter()
            .filter_map(|n| {
                suffixes.iter().find_map(|s| {
                    let cut = n.len().checked_sub(s.len())?;
                    (n.is_char_boundary(cut) && n[cut..].eq_ignore_ascii_case(s))
                        .then(|| n[..cut].to_string())
                })
            })
            .collect();
        bases.sort();
        bases.dedup();
        match bases.len() {
            0 => continue,
            1 => return Ok(bases.remove(0)),
            _ => {
                return Err(format!(
                    "{} holds more than one TetGen output set: {}",
                    dir.display(),
                    bases.join(", ")
                ));
            }
        }
    }
    Err(format!("{} holds no TetGen output", dir.display()))
}

/// Builds and verifies the `.mbin` of an existing TetGen output set in `tetgen_dir` (basename
/// `basename`, or the one [`find_basename`] finds) against `project`, into `out_dir`: `mesh.cbin`,
/// `tetramesh.mbin` when it passes, and `mesh.json` with source `external`, the flags read from
/// the `.1.face` trailer and the project's mesh input hash. Only those three files are deleted
/// from `out_dir` first, so `out_dir` may be `tetgen_dir`. The project's box fitting zones and
/// ids are applied as [`mesh_project`] applies them.
pub fn mesh_from_tetgen(
    project: &Project,
    tetgen_dir: &Path,
    basename: Option<&str>,
    out_dir: &Path,
) -> Result<MeshManifest, MeshError> {
    let start = Instant::now();
    std::fs::create_dir_all(out_dir).map_err(io_at(out_dir))?;
    for name in [names::SCENE_MESH, names::TETRA_MESH, MANIFEST_FILE] {
        let path = out_dir.join(name);
        match std::fs::remove_file(&path) {
            Ok(()) => {}
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            Err(e) => return Err(io_at(&path)(e)),
        }
    }
    let mut m = new_manifest(MeshSource::External);
    m.input_path = Some(tetgen_dir.display().to_string());
    m.mesh_input_hash = Some(crate::validate::mesh_input_hash(project));
    let input = match project_input(project) {
        Ok(i) => i,
        Err(e) => {
            fail(&mut m, codes::INPUT_INVALID, e.0);
            return finish(out_dir, m, start);
        }
    };
    scene_counts(&mut m, &input);
    m.counts.poly_vertices = 0;
    m.counts.poly_facets = 0;
    m.counts.var_constraints = 0;
    if !write_scene(out_dir, &input.scene, &mut m)? {
        return finish(out_dir, m, start);
    }
    let base = match basename {
        Some(b) => b.to_string(),
        None => match find_basename(tetgen_dir) {
            Ok(b) => b,
            Err(e) => {
                fail(&mut m, codes::TETGEN_OUTPUT_MISSING, e);
                return finish(out_dir, m, start);
            }
        },
    };
    let paths = OutputPaths::new(tetgen_dir, &base);
    let trailer = std::fs::read(&paths.face)
        .ok()
        .and_then(|b| trailer_command(&String::from_utf8_lossy(&b)));
    m.tetgen = Some(TetgenCall {
        program: trailer.as_ref().map(|t| t.program.clone()),
        argv: trailer
            .as_ref()
            .map(|t| {
                let mut a = t.flags.clone();
                a.push(t.input.clone());
                a
            })
            .unwrap_or_default(),
        cwd: tetgen_dir.display().to_string(),
        ..TetgenCall::default()
    });
    if trailer.is_none() {
        m.messages.push(format!(
            "{} has no `# Generated by` trailer: the flags are unknown",
            paths.face.display()
        ));
    }
    classify(&paths, None, &input, &mut m);
    if m.codes.is_empty() {
        build_and_write(&paths, &input, out_dir, &CancelToken::new(), &mut m);
    }
    finish(out_dir, m, start)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stale_names_are_the_meshers_and_tetgens() {
        for n in [
            "scene_mesh.poly",
            "scene_mesh.var",
            "SCENE_MESH.VAR",
            "scene_mesh.edge",
            "scene_mesh.mtr",
            "scene_mesh.1.ele",
            "scene_mesh.1.neigh",
            "scene_mesh.1.edge",
            "scene_mesh_skipped.face",
            "tetramesh.mbin",
            "mesh.cbin",
            "mesh.json",
            "tetgen.stdout.txt",
            "tetgen.stderr.txt",
        ] {
            assert!(is_stale(n), "{n}");
        }
        for n in [
            "notes.txt",
            "model.1.ele",
            "scene_mesh.poly.bak",
            "project.simpa",
        ] {
            assert!(!is_stale(n), "{n}");
        }
    }
}
