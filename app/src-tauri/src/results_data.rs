//! What the Results step reads of a run (M12 PLAN.md, P1 item 3): the report `simpa results
//! --json` prints, and the result files' data for the viewport, each from a run whose results
//! load (`results::load`, which verifies every file before anything is read).
//!
//! **This is the only module through which an acoustic value leaves the app** (`runs.rs` sends
//! diagnostics only). The UI shows a parameter only when its status in the report's `bed` is PASS
//! (M12 gate (b)); this module does not filter, so the CLI and the screen read the same report.
//!
//! - [`report_view`] (`run_report`): the run's results state and its report, the CLI's JSON.
//! - [`data_index`] (`run_data`): which surface maps, particle files and echograms the run has.
//! - [`surface_map_bytes`] (`run_surface_map`): one `.csbin`, its float32 values bit for bit, as
//!   bytes (layout below; decoded by `app/ui/src/resultsData.ts`).
//! - [`particles_bytes`] (`run_particles`): one band's `.pbin`, bit for bit, as bytes.
//! - [`echogram`] (`run_echogram`): one SPPS point receiver's `.recp` series per band, and each
//!   source's when the run wrote them.
//!
//! Byte layouts, little-endian throughout, every section 4-aligned:
//!
//! ```text
//! SMAP v1 (a surface map)
//!   0   u32 magic 0x50414D53 ("SMAP")      4   u32 version 1
//!   8   u32 nn, nodes                     12   u32 nf, faces (every receiver's, in file order)
//!   16  u32 nr, records                   20   u32 the file's time-step count
//!   24  f32 the file's time step (s)      28   i32 the file's record type
//!   32  f32 x 3nn   node positions, as the file stores them
//!       u32 x 3nf   each face's vertices (indices into the nodes)
//!       u32 x nf    each face's receiver (its position in the index's `receivers`)
//!       u32 x nf+1  each face's first record; face i holds records [off[i], off[i+1])
//!       u32 x nr    each record's time step (the file's u16)
//!       f32 x nr    each record's value, bit for bit
//!
//! PART v1 (a particle file)
//!   0   u32 magic 0x54524150 ("PART")      4   u32 version 1
//!   8   u32 np, particles                 12   u32 nr, step records
//!   16  u32 the file's nb_time_step_max   20   f32 the file's time step (s)
//!   24  i32 the band, Hz                  28   u32 0
//!   32  u32 x np    each particle's first time step (the file's u16)
//!       u32 x np+1  each particle's first record; particle i holds [off[i], off[i+1])
//!       f32 x 3nr   each record's position, bit for bit
//!       f32 x nr    each record's energy, bit for bit
//! ```

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock, Mutex};
use std::time::SystemTime;

use schemars::JsonSchema;
use serde::Serialize;
use simpa_core::config_xml::SolverIds;
use simpa_core::formats::{csbin, pbin};
use simpa_core::results::spps::{BandEnergy, ParticleFileSummary};
use simpa_core::results::{self, Report, RunResults, SolverResults, SurfaceFile};
use simpa_core::schema::Project;

use crate::guard::{CmdError, CmdResult};
use crate::runs::{ReasonUi, ResultsState, is_run_name, reason_ui};

pub const SMAP_MAGIC: u32 = 0x5041_4D53;
pub const PART_MAGIC: u32 = 0x5452_4150;
pub const LAYOUT_VERSION: u32 = 1;
const HEADER: usize = 32;

// ---- the types the UI reads (bindings.rs dumps them) --------------------------------------------

/// `run_report`: the run's results state (as `run_results` answers it) and, when its results load,
/// the report `simpa results <run> --json` prints for it (`results::checked_report`).
#[derive(Clone, Debug, PartialEq, Serialize, JsonSchema)]
pub struct ReportView {
    pub state: ResultsState,
    /// `None` when the results are refused (`state.refusal` says why).
    pub report: Option<Report>,
    /// M12 P2: the surface groups the open project gives each solver material id
    /// ([`group_names`]), so the Acoustics tab can name `report.room.surfaces` by group. Names,
    /// not numbers: every number shown stays the report's. Empty with no project open.
    pub surface_groups: Vec<SurfaceGroupNames>,
}

/// One solver material id (`type_surface@id`, the `.cbin` faces' `idMat`) and the surface groups
/// that carry it, in project order.
#[derive(Clone, Debug, PartialEq, Serialize, JsonSchema)]
pub struct SurfaceGroupNames {
    pub material_id: u32,
    pub names: Vec<String>,
}

/// Each material id the project's config.xml is written with ([`SolverIds::assign`]) and its
/// groups' names, ascending by id; none when the ids cannot be assigned (a clash the run itself
/// would have refused).
pub fn group_names(project: &Project) -> Vec<SurfaceGroupNames> {
    let Ok(ids) = SolverIds::assign(project) else {
        return Vec::new();
    };
    let mut out: Vec<SurfaceGroupNames> = Vec::new();
    for (group, id) in ids.group_materials {
        let Some(name) = project.group(group).map(|g| g.name.clone()) else {
            continue;
        };
        match out.iter_mut().find(|g| g.material_id == id) {
            Some(g) => g.names.push(name),
            None => out.push(SurfaceGroupNames {
                material_id: id,
                names: vec![name],
            }),
        }
    }
    out.sort_by_key(|g| g.material_id);
    out
}

/// `run_data`: what the run holds for the viewport and the charts.
#[derive(Clone, Debug, PartialEq, Serialize, JsonSchema)]
pub struct RunDataIndex {
    pub state: ResultsState,
    /// `None` when the results are refused.
    pub data: Option<RunData>,
}

#[derive(Clone, Debug, PartialEq, Serialize, JsonSchema)]
pub struct RunData {
    /// `spps` or `tcr`.
    pub solver: String,
    pub bands_hz: Vec<i32>,
    /// SPPS's time step (`pasdetemps` as `f32`, widened), s; `None` for TCR.
    pub time_step_s: Option<f64>,
    /// SPPS's step count; `None` for TCR.
    pub steps: Option<usize>,
    /// Every `.csbin`: surface receivers and cutting planes, per band and `Global`.
    pub surfaces: Vec<SurfaceMapInfo>,
    /// The saved particle files, one per band; empty when the run saved none
    /// (`nbparticules_rendu` 0, the new-project default: decision 44 (2)).
    pub particle_files: Vec<ParticleFileSummary>,
    /// SPPS's point receivers whose echograms `run_echogram` reads; empty for TCR.
    pub echograms: Vec<EchogramInfo>,
}

/// One `.csbin` as `run_surface_map` serves it.
#[derive(Clone, Debug, PartialEq, Serialize, JsonSchema)]
pub struct SurfaceMapInfo {
    /// Relative to `solve/`, `/`-separated: the key `run_surface_map` takes.
    pub path: String,
    /// TCR's field (`Direct field`, `Total field (Sabine)`, `Total field (Eyring)`); `None` for
    /// SPPS.
    pub field: Option<String>,
    /// The band; `None` for the `Global` file of all bands together.
    pub band_hz: Option<i32>,
    pub cutting_plane: bool,
    pub time_step_count: u32,
    /// The file's time step (`f32`, widened), s.
    pub time_step_s: f64,
    /// The file's receivers in order; a face's receiver index in the bytes is a position here.
    pub receivers: Vec<SurfaceReceiverInfo>,
    pub nodes: usize,
    pub faces: usize,
    pub records: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, JsonSchema)]
pub struct SurfaceReceiverInfo {
    /// The receiver's id in `config.xml`.
    pub xml_index: i32,
    pub name: String,
    pub faces: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, JsonSchema)]
pub struct EchogramInfo {
    /// The receiver's label: the key `run_echogram` takes.
    pub label: String,
    /// The sources with their own echogram here (`output_recp_bysource`), in `config.xml`'s
    /// order; empty when the run wrote none.
    pub sources: Vec<String>,
}

/// `run_echogram`: one SPPS point receiver's `.recp` series, Pa² per time step (`f32` widened),
/// per computed band, and each source's own when the run wrote them.
#[derive(Clone, Debug, PartialEq, Serialize, JsonSchema)]
pub struct EchogramView {
    pub receiver: String,
    /// The receiver's folder, relative to `solve/`.
    pub folder: String,
    pub time_step_s: f64,
    pub bands: Vec<BandEnergy>,
    pub sources: Vec<SourceEchogramView>,
}

#[derive(Clone, Debug, PartialEq, Serialize, JsonSchema)]
pub struct SourceEchogramView {
    pub source: String,
    /// Relative to `solve/`.
    pub file: String,
    pub bands: Vec<BandEnergy>,
}

// ---- reading a run ----------------------------------------------------------------------------

/// What a cached load was made from: `run.json`'s size and modification time. The solver writes
/// `run.json` last (its status), so a run that finishes, or is re-run into the same folder, reads
/// afresh; a results file edited underneath an unchanged `run.json` does not, until the app
/// restarts.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Fingerprint {
    len: u64,
    modified: Option<SystemTime>,
}

impl Fingerprint {
    fn of(run_json: &Path) -> Option<Self> {
        let m = std::fs::metadata(run_json).ok()?;
        Some(Fingerprint {
            len: m.len(),
            modified: m.modified().ok(),
        })
    }
}

/// One run's loaded results, with what they were loaded from.
struct Loaded {
    fingerprint: Fingerprint,
    results: Arc<RunResults>,
}

/// The loaded runs, by run folder. Each run has its own slot lock, so commands that arrive together
/// for a run that is not loaded yet wait for ONE load instead of each making their own: the CR4-27
/// run (28 maps, 6.7 GB) took 114 s and 3.3 GB to load, and the Results step issues `run_report`,
/// `run_data` and `run_echogram` at once (2026-10-06 17:24: the app reached 6.5 GB on their
/// concurrent loads). The results kept are summaries (`SurfaceFile`), so the cache is small; it is
/// bounded anyway ([`CACHE_RUNS`]).
static CACHE: LazyLock<Mutex<HashMap<PathBuf, Arc<Mutex<Option<Loaded>>>>>> =
    LazyLock::new(Mutex::default);

/// Runs kept at once. One is viewed at a time; a few more cover switching back and forth. The
/// unit tests share this process-wide cache across every test running in parallel, so there the
/// bound is wider: at 4, other tests' loads evicted the run that
/// `a_run_is_loaded_once_and_again_when_its_manifest_changes` opens twice in a row.
const CACHE_RUNS: usize = if cfg!(test) { 64 } else { 4 };

/// Locks, trusting a lock poisoned by a panic elsewhere: the cache holds only immutable `Arc`s,
/// so a half-finished write cannot be seen.
fn unpoisoned<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

/// The slot for `dir`, made if absent; the oldest-inserted slots are dropped past the bound (a
/// dropped slot's results live on in any command still holding their `Arc`).
fn slot_for(dir: &Path) -> Arc<Mutex<Option<Loaded>>> {
    let key = std::fs::canonicalize(dir).unwrap_or_else(|_| dir.to_path_buf());
    let mut cache = unpoisoned(&CACHE);
    if let Some(s) = cache.get(&key) {
        return s.clone();
    }
    if cache.len() >= CACHE_RUNS {
        // HashMap has no order; drop every slot that is not loaded, then any one if still full.
        cache.retain(|_, s| unpoisoned(s).is_some());
        if cache.len() >= CACHE_RUNS {
            if let Some(k) = cache.keys().next().cloned() {
                cache.remove(&k);
            }
        }
    }
    cache.entry(key).or_default().clone()
}

/// `results::load` of the run folder `dir`, through the cache: a hit whose `run.json` is unchanged
/// is returned; otherwise the run is loaded now (the slot held, so a second caller waits and then
/// hits) and kept if it loaded. A refusal is not kept: it is given again on the next call, so a
/// run repaired on disk reads again.
fn load_cached(dir: &Path) -> Result<Arc<RunResults>, results::Refusal> {
    let fingerprint = Fingerprint::of(&dir.join(simpa_core::run::manifest::FILE_NAME));
    let slot = slot_for(dir);
    let mut held = unpoisoned(&slot);
    if let (Some(fp), Some(l)) = (&fingerprint, held.as_ref()) {
        if l.fingerprint == *fp {
            return Ok(l.results.clone());
        }
    }
    let results = Arc::new(results::load(dir)?);
    if let Some(fp) = fingerprint {
        *held = Some(Loaded {
            fingerprint: fp,
            results: results.clone(),
        });
    }
    Ok(results)
}

/// Forgets every loaded run (tests; a project closed).
#[allow(dead_code)]
pub fn forget_all() {
    unpoisoned(&CACHE).clear();
}

/// The run `run` under `root`, loaded: its state (as `run_results` answers it) and, unless
/// refused, its results. `run` must be a bare run-folder name that exists under `root`; anything
/// else is `RUN_NOT_FOUND`. The results are read once per run and shared ([`CACHE`]).
pub fn open(root: &Path, run: &str) -> CmdResult<(ResultsState, Option<Arc<RunResults>>)> {
    let not_found = || {
        CmdError::new(
            "RUN_NOT_FOUND",
            format!("no run '{run}' in {}", root.display()),
        )
    };
    if !is_run_name(run) {
        return Err(not_found());
    }
    let dir = root.join(run);
    if !dir.is_dir() {
        return Err(not_found());
    }
    Ok(match load_cached(&dir) {
        // Results that load are verified only when the solver build was (backlog 38); otherwise
        // they are marked unverified with its reason, not refused (C6).
        Ok(r) => {
            let unverified = results::solver_build(&r.manifest)
                .reason()
                .map(|x| reason_ui(x, None));
            let state = ResultsState {
                run: run.to_string(),
                verified: unverified.is_none(),
                refusal: None,
                unverified,
            };
            (state, Some(r))
        }
        Err(r) => (refused(run, r), None),
    })
}

fn refused(run: &str, r: results::Refusal) -> ResultsState {
    ResultsState {
        run: run.to_string(),
        verified: false,
        refusal: Some(ReasonUi {
            ui_code: r.code.to_ascii_uppercase(),
            code: r.code,
            detail: r.detail,
        }),
        unverified: None,
    }
}

/// The run's results, or `RESULTS_REFUSED` naming why: for the reads that serve data.
fn loaded(root: &Path, run: &str) -> CmdResult<Arc<RunResults>> {
    match open(root, run)? {
        (_, Some(r)) => Ok(r),
        (state, None) => {
            let why = state
                .refusal
                .map_or_else(String::new, |r| format!("{}: {}", r.ui_code, r.detail));
            Err(CmdError::new(
                "RESULTS_REFUSED",
                format!("run '{run}': its results are refused ({why})"),
            ))
        }
    }
}

pub fn report_view(root: &Path, run: &str) -> CmdResult<ReportView> {
    let (state, r) = open(root, run)?;
    Ok(match r {
        None => ReportView {
            state,
            report: None,
            surface_groups: Vec::new(),
        },
        // The CLI prints this report (`results_cmd`); one that holds a number that is not
        // finite is refused there and here alike.
        Some(r) => match results::checked_report(&r) {
            Ok(report) => ReportView {
                state,
                report: Some(report),
                surface_groups: Vec::new(),
            },
            Err(refusal) => ReportView {
                state: refused(run, refusal),
                report: None,
                surface_groups: Vec::new(),
            },
        },
    })
}

fn surfaces(r: &RunResults) -> &[SurfaceFile] {
    match &r.data {
        SolverResults::Spps(s) => &s.surfaces,
        SolverResults::Tcr(t) => &t.surfaces,
    }
}

fn surface_info(f: &SurfaceFile) -> SurfaceMapInfo {
    // From the summary `load` kept, not the records (a run's maps can be gigabytes).
    SurfaceMapInfo {
        path: f.path.clone(),
        field: f.field.clone(),
        band_hz: f.band_hz,
        cutting_plane: f.cutting_plane,
        time_step_count: f.time_step_count,
        time_step_s: f64::from(f.time_step),
        receivers: f
            .receivers
            .iter()
            .map(|r| SurfaceReceiverInfo {
                xml_index: r.id,
                name: r.name.clone(),
                faces: r.faces,
            })
            .collect(),
        nodes: f.nodes,
        faces: f.receivers.iter().map(|r| r.faces).sum(),
        records: f.receivers.iter().map(|r| r.records).sum(),
    }
}

pub fn data_index(root: &Path, run: &str) -> CmdResult<RunDataIndex> {
    let (state, r) = open(root, run)?;
    let Some(r) = r else {
        return Ok(RunDataIndex { state, data: None });
    };
    let (solver, time_step_s, steps, particle_files, echograms) = match &r.data {
        SolverResults::Spps(s) => (
            "spps",
            Some(s.time_step_s),
            Some(s.steps),
            s.particle_files.clone(),
            s.point_receivers
                .iter()
                .map(|p| EchogramInfo {
                    label: p.label.clone(),
                    sources: p.echograms.iter().map(|e| e.source.clone()).collect(),
                })
                .collect(),
        ),
        SolverResults::Tcr(_) => ("tcr", None, None, Vec::new(), Vec::new()),
    };
    let data = RunData {
        solver: solver.to_string(),
        bands_hz: r.bands_hz.clone(),
        time_step_s,
        steps,
        surfaces: surfaces(&r).iter().map(surface_info).collect(),
        particle_files,
        echograms,
    };
    Ok(RunDataIndex {
        state,
        data: Some(data),
    })
}

fn count(n: usize, what: &str) -> CmdResult<u32> {
    u32::try_from(n).map_err(|_| {
        CmdError::new(
            "RESULTS_TOO_LARGE",
            format!("{n} {what}: more than a u32 counts"),
        )
    })
}

fn put_u32(b: &mut Vec<u8>, v: u32) {
    b.extend_from_slice(&v.to_le_bytes());
}

fn put_f32(b: &mut Vec<u8>, v: f32) {
    b.extend_from_slice(&v.to_bits().to_le_bytes());
}

/// A `.csbin` as SMAP v1 (the module's header).
fn encode_surface(c: &csbin::Csbin) -> CmdResult<Vec<u8>> {
    let faces: Vec<(usize, &csbin::Face)> = c
        .receivers
        .iter()
        .enumerate()
        .flat_map(|(i, r)| r.faces.iter().map(move |face| (i, face)))
        .collect();
    let nr: usize = faces.iter().map(|(_, face)| face.records.len()).sum();
    let (nn, nf) = (c.nodes.len(), faces.len());
    let mut b = Vec::with_capacity(HEADER + 12 * nn + 20 * nf + 4 + 8 * nr);
    put_u32(&mut b, SMAP_MAGIC);
    put_u32(&mut b, LAYOUT_VERSION);
    put_u32(&mut b, count(nn, "nodes")?);
    put_u32(&mut b, count(nf, "faces")?);
    put_u32(&mut b, count(nr, "records")?);
    put_u32(&mut b, c.time_step_count);
    put_f32(&mut b, c.time_step);
    b.extend_from_slice(&c.record_type.as_i32().to_le_bytes());
    for n in &c.nodes {
        n.iter().for_each(|&x| put_f32(&mut b, x));
    }
    for (_, face) in &faces {
        face.vertices.iter().for_each(|&v| put_u32(&mut b, v));
    }
    for (i, _) in &faces {
        put_u32(&mut b, count(*i, "receivers")?);
    }
    let mut at = 0usize;
    put_u32(&mut b, 0);
    for (_, face) in &faces {
        at += face.records.len();
        put_u32(&mut b, count(at, "records")?);
    }
    for (_, face) in &faces {
        face.records
            .iter()
            .for_each(|r| put_u32(&mut b, u32::from(r.time_step)));
    }
    for (_, face) in &faces {
        face.records.iter().for_each(|r| put_f32(&mut b, r.energy));
    }
    Ok(b)
}

/// Records a served map may hold: 8 bytes a record in SMAP, so 256 MB of bytes, which the WebView
/// then holds as one ArrayBuffer. CR4 at 27 third-octave bands with its 0.1 m plane writes maps of
/// about 150 million records each (1.2 GB as SMAP): a map past this bound is served at a coarser
/// time bin ([`bin_factor`]), never refused.
pub const MAP_RECORDS_MAX: usize = 32 * 1024 * 1024;

/// Texels (faces x served steps) a map may ask of the viewport when the UI does not say: its
/// texture is dense, one float32 per face per step (`mapData.ts mapLayout`), and the JS array it
/// is filled from is as large. 64 Mi texels is 256 MB of each. The UI passes its own budget
/// (`run_surface_map`'s `max_texels`); this is the fallback.
pub const MAP_TEXELS_MAX: u64 = 64 * 1024 * 1024;

/// Time steps merged to a bin so that a map of `faces` faces over `file.time_step_count` steps is
/// served within `max_records` records AND `faces * steps <= max_texels` (the viewport's dense
/// texture): 1 when the file's own records and texels fit, or when it cannot be binned (a
/// single-step map; TCR's fields, whose values are levels, not energies that add). Otherwise the
/// smallest factor under which `faces * ceil(steps / factor)` meets both bounds, which holds
/// however the records fall (CR4-27's 1000 Hz plane: 87,860 faces x 10,000 steps, 878 M texels,
/// refused by the viewport at 16384 x 16384 until binned). When even one bin a face is too many,
/// one bin a face it is.
fn bin_factor(file: &SurfaceFile, records: usize, max_records: usize, max_texels: u64) -> u32 {
    let faces: usize = file.receivers.iter().map(|r| r.faces).sum();
    let steps = file.time_step_count as usize;
    let texels = (faces as u64).saturating_mul(steps as u64);
    if (records <= max_records && texels <= max_texels)
        || file.time_step_count <= 1
        || file.record_type != csbin::RecordType::SplStandard
    {
        return 1;
    }
    // Bins a face allowed by each bound; the smaller wins.
    let by_records = max_records / faces.max(1);
    let by_texels = usize::try_from(max_texels / (faces as u64).max(1)).unwrap_or(usize::MAX);
    let bins_per_face = by_records.min(by_texels);
    if bins_per_face == 0 {
        return file.time_step_count;
    }
    u32::try_from(steps.div_ceil(bins_per_face))
        .unwrap_or(u32::MAX)
        .clamp(1, file.time_step_count)
}

/// The map `path` of `run`, within [`MAP_RECORDS_MAX`] records and `max_texels` texels (the UI's
/// budget, else [`MAP_TEXELS_MAX`]).
pub fn surface_map_bytes(
    root: &Path,
    run: &str,
    path: &str,
    max_texels: Option<u64>,
) -> CmdResult<Vec<u8>> {
    surface_map_bytes_within(
        root,
        run,
        path,
        MAP_RECORDS_MAX,
        max_texels.unwrap_or(MAP_TEXELS_MAX),
    )
}

/// [`surface_map_bytes`] with the bounds given: the one map `path`, read from its file now (the
/// results keep summaries, not records), at the file's own time bin when it fits `max_records`
/// and `max_texels`, else merged to the coarser bin [`bin_factor`] gives, which the SMAP header
/// carries (`time_step`, `time_step_count`) so a reader scales by it.
pub fn surface_map_bytes_within(
    root: &Path,
    run: &str,
    path: &str,
    max_records: usize,
    max_texels: u64,
) -> CmdResult<Vec<u8>> {
    let r = loaded(root, run)?;
    let file = surfaces(&r)
        .iter()
        .find(|s| s.path == path)
        .ok_or_else(|| {
            CmdError::new(
                "MAP_NOT_FOUND",
                format!("run '{run}' has no surface map '{path}'"),
            )
        })?;
    let records: usize = file.receivers.iter().map(|r| r.records).sum();
    let per_bin = bin_factor(file, records, max_records, max_texels);
    // `load` already read and checked this file; it is read again here, encoded, and dropped.
    let data = csbin::read_file_binned(&r.folder.join("solve").join(&file.path), per_bin)
        .map_err(|e| {
            CmdError::new(
                "MAP_UNREADABLE",
                format!("run '{run}': surface map '{path}' does not read now: {e}"),
            )
        })?;
    encode_surface(&data)
}

/// A `.pbin` as PART v1 (the module's header).
fn encode_particles(p: &pbin::ParticleFile, band_hz: i32) -> CmdResult<Vec<u8>> {
    let (np, nr) = (p.particles.len(), p.steps.len());
    let mut b = Vec::with_capacity(HEADER + 8 * np + 4 + 16 * nr);
    put_u32(&mut b, PART_MAGIC);
    put_u32(&mut b, LAYOUT_VERSION);
    put_u32(&mut b, count(np, "particles")?);
    put_u32(&mut b, count(nr, "step records")?);
    put_u32(&mut b, p.header.nb_time_step_max);
    put_f32(&mut b, p.header.time_step);
    b.extend_from_slice(&band_hz.to_le_bytes());
    put_u32(&mut b, 0);
    for h in &p.particles {
        put_u32(&mut b, u32::from(h.first_time_step));
    }
    let mut at = 0usize;
    put_u32(&mut b, 0);
    for (_, steps) in p.iter() {
        at += steps.len();
        put_u32(&mut b, count(at, "step records")?);
    }
    for s in &p.steps {
        s.position.iter().for_each(|&x| put_f32(&mut b, x));
    }
    for s in &p.steps {
        put_f32(&mut b, s.energy);
    }
    Ok(b)
}

pub fn particles_bytes(root: &Path, run: &str, band_hz: i32) -> CmdResult<Vec<u8>> {
    let r = loaded(root, run)?;
    let files = match &r.data {
        SolverResults::Spps(s) => &s.particle_files,
        SolverResults::Tcr(_) => {
            return Err(CmdError::new(
                "PARTICLES_NOT_SAVED",
                format!("run '{run}' is TCR's: it traces no particles"),
            ));
        }
    };
    if files.is_empty() {
        return Err(CmdError::new(
            "PARTICLES_NOT_SAVED",
            format!("run '{run}' saved no particles (particles saved per source is 0)"),
        ));
    }
    let file = files.iter().find(|p| p.freq_hz == band_hz).ok_or_else(|| {
        CmdError::new(
            "PARTICLES_NOT_FOUND",
            format!("run '{run}' has no particle file for {band_hz} Hz"),
        )
    })?;
    // `load` decoded and checked this file against its siblings; it is decoded again here
    // because the results keep only its summary.
    let p = pbin::read_file(&r.solve_dir().join(&file.path)).map_err(|e| {
        CmdError::new(
            "RESULTS_REFUSED",
            format!("run '{run}': {} does not read: {e}", file.path),
        )
    })?;
    encode_particles(&p, band_hz)
}

fn bands(bands_hz: &[i32], energy: &[Vec<f64>]) -> Vec<BandEnergy> {
    bands_hz
        .iter()
        .zip(energy)
        .map(|(&freq_hz, e)| BandEnergy {
            freq_hz,
            energy: e.clone(),
        })
        .collect()
}

pub fn echogram(root: &Path, run: &str, receiver: &str) -> CmdResult<EchogramView> {
    let r = loaded(root, run)?;
    let SolverResults::Spps(s) = &r.data else {
        return Err(CmdError::new(
            "NO_TIME_SERIES",
            format!("run '{run}' is TCR's: it writes no echogram"),
        ));
    };
    let p = s
        .point_receivers
        .iter()
        .find(|p| p.label == receiver)
        .ok_or_else(|| {
            CmdError::new(
                "RECEIVER_NOT_FOUND",
                format!("run '{run}' has no point receiver '{receiver}'"),
            )
        })?;
    let freqs: Vec<i32> = p.bands.iter().map(|b| b.freq_hz).collect();
    let total: Vec<Vec<f64>> = p.bands.iter().map(|b| b.energy.clone()).collect();
    Ok(EchogramView {
        receiver: p.label.clone(),
        folder: p.folder.clone(),
        time_step_s: s.time_step_s,
        bands: bands(&freqs, &total),
        sources: p
            .echograms
            .iter()
            .map(|e| SourceEchogramView {
                source: e.source.clone(),
                file: e.file.clone(),
                bands: bands(&freqs, &e.energy),
            })
            .collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::time::SystemTime;

    use simpa_core::formats::gabe;

    fn repo(rel: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join(rel)
    }

    /// A fresh folder on the system temp drive, removed by the test.
    fn scratch(tag: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "simpa-app-m12-{tag}-{}-{nanos}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn copy_dir(from: &Path, to: &Path) {
        std::fs::create_dir_all(to).unwrap();
        for e in std::fs::read_dir(from).unwrap() {
            let e = e.unwrap();
            let target = to.join(e.file_name());
            if e.file_type().unwrap().is_dir() {
                copy_dir(&e.path(), &target);
            } else {
                std::fs::copy(e.path(), &target).unwrap();
            }
        }
    }

    const RUN: &str = "20260924-115031-155-spps";

    /// A runs root holding the committed fixture `name` as the run [`RUN`].
    fn runs_with(tag: &str, name: &str) -> (PathBuf, PathBuf) {
        let dir = scratch(tag);
        let root = dir.join("runs");
        copy_dir(
            &repo(&format!("tests/fixtures/results/{name}")),
            &root.join(RUN),
        );
        (dir, root)
    }

    fn u32_at(b: &[u8], o: usize) -> u32 {
        u32::from_le_bytes(b[o..o + 4].try_into().unwrap())
    }
    fn f32_bits_at(b: &[u8], o: usize) -> u32 {
        u32_at(b, o)
    }

    #[test]
    fn the_report_is_the_one_the_cli_prints_with_its_bed_status() {
        let (dir, root) = runs_with("report", "seats_spps");
        let v = report_view(&root, RUN).unwrap();
        // The committed run records no solver check: loaded, not verified, not refused.
        assert!(!v.state.verified, "{:?}", v.state);
        assert_eq!(v.state.refusal, None);
        let rep = v.report.expect("a report");
        let want = results::checked_report(&results::load(&root.join(RUN)).unwrap()).unwrap();
        assert_eq!(
            serde_json::to_value(&rep).unwrap(),
            serde_json::to_value(&want).unwrap(),
            "the CLI's report (results_cmd prints checked_report)"
        );
        assert_eq!(rep.results_version, 17);
        let summary: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(repo("beds/summary.json")).unwrap())
                .unwrap();
        let bed = serde_json::to_value(&rep.bed.parameters).unwrap();
        for (name, p) in summary["parameters"].as_object().unwrap() {
            assert_eq!(bed[name]["status"], p["status"], "{name}");
        }
        for bad in ["../x", "nope", ""] {
            assert_eq!(report_view(&root, bad).unwrap_err().code, "RUN_NOT_FOUND");
        }
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// M12 P2: the absorption table names each of the report's material ids by the surface groups
    /// the open project gives it (`SolverIds::assign`, the ids config.xml was written with): the
    /// seats box pins 21, 22 and 25; the teaching room numbers its six groups 1 to 6 in order.
    #[test]
    fn the_surface_groups_name_the_reports_material_ids() {
        let seats =
            simpa_core::schema::load(&repo("tests/fixtures/rooms/seats_box.simpa")).unwrap();
        let names = group_names(&seats);
        let got: Vec<(u32, Vec<String>)> = names
            .iter()
            .map(|g| (g.material_id, g.names.clone()))
            .collect();
        assert_eq!(
            got,
            vec![
                (21, vec!["Ceiling".to_string()]),
                (22, vec!["Walls".to_string()]),
                (25, vec!["Floor".to_string()]),
            ]
        );
        // Every material id of the seats run's report has its names.
        let (dir, root) = runs_with("names", "seats_spps");
        let rep = report_view(&root, RUN).unwrap().report.unwrap();
        let room = serde_json::to_value(&rep.room).unwrap();
        let ids: Vec<u64> = room["surfaces"]
            .as_array()
            .unwrap()
            .iter()
            .map(|s| s["material_id"].as_u64().unwrap())
            .collect();
        assert_eq!(ids, vec![21, 22, 25]);
        std::fs::remove_dir_all(&dir).unwrap();
        let room =
            simpa_core::schema::load(&repo("tests/fixtures/ui/teaching_room.simpa")).unwrap();
        let got: Vec<(u32, Vec<String>)> = group_names(&room)
            .iter()
            .map(|g| (g.material_id, g.names.clone()))
            .collect();
        let want: Vec<(u32, Vec<String>)> = [
            "Floor",
            "Ceiling",
            "Left wall",
            "Right wall",
            "Front wall",
            "Rear wall",
        ]
        .iter()
        .enumerate()
        .map(|(i, n)| (i as u32 + 1, vec![n.to_string()]))
        .collect();
        assert_eq!(got, want);
    }

    #[test]
    fn a_refused_run_has_its_refusal_and_no_report_or_data() {
        let (dir, root) = runs_with("refused", "seats_spps");
        std::fs::remove_file(
            root.join(RUN)
                .join("solve/Punctual receivers/Seat/Sound level.recp"),
        )
        .unwrap();
        let v = report_view(&root, RUN).unwrap();
        assert!(v.report.is_none());
        assert!(v.state.refusal.is_some(), "{:?}", v.state);
        let d = data_index(&root, RUN).unwrap();
        assert!(d.data.is_none());
        assert_eq!(d.state, v.state);
        let e =
            surface_map_bytes(&root, RUN, "Surface receiver/Global/Sound level.csbin", None)
                .unwrap_err();
        assert_eq!(e.code, "RESULTS_REFUSED", "{e:?}");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn the_index_lists_every_map_particle_file_and_echogram() {
        let (dir, root) = runs_with("index", "outputs_spps");
        let d = data_index(&root, RUN).unwrap().data.expect("data");
        assert_eq!(d.solver, "spps");
        assert_eq!(d.bands_hz, vec![500, 1000]);
        let mut paths: Vec<&str> = d.surfaces.iter().map(|s| s.path.as_str()).collect();
        paths.sort_unstable();
        assert_eq!(
            paths,
            [
                "Surface receiver/1000 Hz/Sound level.csbin",
                "Surface receiver/1000 Hz/rs_cut.csbin",
                "Surface receiver/500 Hz/Sound level.csbin",
                "Surface receiver/500 Hz/rs_cut.csbin",
                "Surface receiver/Global/Sound level.csbin",
                "Surface receiver/Global/rs_cut.csbin",
            ]
        );
        assert_eq!(
            d.surfaces.iter().filter(|s| s.cutting_plane).count(),
            3,
            "{:?}",
            d.surfaces
        );
        let bands: Vec<i32> = d.particle_files.iter().map(|p| p.freq_hz).collect();
        assert_eq!(bands, [500, 1000]);
        let labels: Vec<&str> = d.echograms.iter().map(|e| e.label.as_str()).collect();
        assert_eq!(labels, ["Seat", "Seat2"]);
        assert!(d.time_step_s.is_some() && d.steps.is_some());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_surface_map_carries_the_csbin_float32s_bit_for_bit() {
        let (dir, root) = runs_with("smap", "outputs_spps");
        let d = data_index(&root, RUN).unwrap().data.unwrap();
        let mut compared = 0usize;
        for info in &d.surfaces {
            let b = surface_map_bytes(&root, RUN, &info.path, None).unwrap();
            let file = csbin::read_file(&root.join(RUN).join("solve").join(&info.path)).unwrap();
            assert_eq!(u32_at(&b, 0), SMAP_MAGIC);
            assert_eq!(u32_at(&b, 4), LAYOUT_VERSION);
            let (nn, nf, nr) = (
                u32_at(&b, 8) as usize,
                u32_at(&b, 12) as usize,
                u32_at(&b, 16) as usize,
            );
            assert_eq!(nn, file.nodes.len());
            assert_eq!((nn, nf, nr), (info.nodes, info.faces, info.records));
            assert_eq!(u32_at(&b, 20), file.time_step_count);
            assert_eq!(f32_bits_at(&b, 24), file.time_step.to_bits());
            assert_eq!(u32_at(&b, 28) as i32, file.record_type.as_i32());
            let mut o = HEADER;
            for n in &file.nodes {
                for c in n {
                    assert_eq!(f32_bits_at(&b, o), c.to_bits());
                    o += 4;
                }
            }
            let faces: Vec<(usize, &csbin::Face)> = file
                .receivers
                .iter()
                .enumerate()
                .flat_map(|(i, r)| r.faces.iter().map(move |f| (i, f)))
                .collect();
            assert_eq!(faces.len(), nf);
            let (vert, recv, off) = (o, o + 12 * nf, o + 16 * nf);
            let steps = off + 4 * (nf + 1);
            let vals = steps + 4 * nr;
            assert_eq!(b.len(), vals + 4 * nr, "{}", info.path);
            for (k, (ri, f)) in faces.iter().enumerate() {
                for j in 0..3 {
                    assert_eq!(u32_at(&b, vert + 12 * k + 4 * j), f.vertices[j]);
                }
                assert_eq!(u32_at(&b, recv + 4 * k) as usize, *ri);
                let (lo, hi) = (
                    u32_at(&b, off + 4 * k) as usize,
                    u32_at(&b, off + 4 * k + 4) as usize,
                );
                assert_eq!(hi - lo, f.records.len());
                for (m, r) in f.records.iter().enumerate() {
                    assert_eq!(u32_at(&b, steps + 4 * (lo + m)), u32::from(r.time_step));
                    assert_eq!(f32_bits_at(&b, vals + 4 * (lo + m)), r.energy.to_bits());
                    compared += 1;
                }
            }
            let names: Vec<&str> = info.receivers.iter().map(|r| r.name.as_str()).collect();
            let want: Vec<String> = file
                .receivers
                .iter()
                .map(|r| r.name_lossy().into_owned())
                .collect();
            assert_eq!(names, want);
        }
        assert!(compared > 0, "the fixture's maps hold records");
        let e = surface_map_bytes(&root, RUN, "Surface receiver/../run.json", None).unwrap_err();
        assert_eq!(e.code, "MAP_NOT_FOUND", "{e:?}");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// A map past the bound is served at a coarser bin: within the bound, its header scaled, each
    /// face's energy conserved to f32 rounding; one within the bound is the file's own.
    #[test]
    fn a_large_map_is_served_at_a_coarser_bin_within_the_bound() {
        let (dir, root) = runs_with("binned", "outputs_spps");
        let d = data_index(&root, RUN).unwrap().data.unwrap();
        let info = d
            .surfaces
            .iter()
            .filter(|s| s.band_hz.is_some() && !s.cutting_plane)
            .max_by_key(|s| s.records)
            .unwrap();
        assert!(info.time_step_count > 4 && info.records > info.faces, "{info:?}");
        let file = csbin::read_file(&root.join(RUN).join("solve").join(&info.path)).unwrap();
        // Two bins a face at most.
        let max = info.faces * 2;
        assert!(max < info.records, "the bound binds");
        let b = surface_map_bytes_within(&root, RUN, &info.path, max, u64::MAX).unwrap();
        let (nf, nr) = (u32_at(&b, 12) as usize, u32_at(&b, 16) as usize);
        assert_eq!(nf, info.faces);
        assert!(nr <= max, "{nr} records served, bound {max}");
        let per_bin = info.time_step_count.div_ceil(2);
        assert_eq!(u32_at(&b, 20), info.time_step_count.div_ceil(per_bin));
        assert_eq!(
            f32_bits_at(&b, 24),
            (file.time_step * per_bin as f32).to_bits()
        );
        // Each face's energy, summed over its served records, is its file records' sum. The face
        // offsets come after the node positions (12 bytes a node) and the faces' 16 bytes each.
        let nn = u32_at(&b, 8) as usize;
        let off = HEADER + 12 * nn + 16 * nf;
        let steps = off + 4 * (nf + 1);
        let vals = steps + 4 * nr;
        let faces: Vec<&csbin::Face> = file
            .receivers
            .iter()
            .flat_map(|r| r.faces.iter())
            .collect();
        for (k, f) in faces.iter().enumerate() {
            let (lo, hi) = (
                u32_at(&b, off + 4 * k) as usize,
                u32_at(&b, off + 4 * k + 4) as usize,
            );
            assert!(hi - lo <= 2);
            let got: f64 = (lo..hi)
                .map(|m| f64::from(f32::from_bits(f32_bits_at(&b, vals + 4 * m))))
                .sum();
            let want: f64 = f.records.iter().map(|r| f64::from(r.energy)).sum();
            assert!(
                (got - want).abs() <= want.abs() * 1e-5 + f64::from(f32::EPSILON),
                "face {k}: {got} vs {want}"
            );
            for m in lo..hi {
                assert!((u32_at(&b, steps + 4 * m) as usize) < 2);
            }
        }
        // Within the bound: the file's own records, as `surface_map_bytes` serves them.
        let own = surface_map_bytes_within(&root, RUN, &info.path, info.records, u64::MAX).unwrap();
        assert_eq!(own, surface_map_bytes(&root, RUN, &info.path, None).unwrap());
        assert_eq!(u32_at(&own, 16) as usize, info.records);
        assert_eq!(u32_at(&own, 20), info.time_step_count);
        // The texel bound alone: three steps a face at most, whatever the records.
        let t = surface_map_bytes_within(&root, RUN, &info.path, usize::MAX, (info.faces * 3) as u64)
            .unwrap();
        assert!(u32_at(&t, 20) <= 3, "{} steps served", u32_at(&t, 20));
        assert!((u32_at(&t, 12) as u64) * u64::from(u32_at(&t, 20)) <= (info.faces * 3) as u64);
        // The cutting plane and Global files take the same road.
        for s in d.surfaces.iter().filter(|s| s.path != info.path) {
            let cap = (s.faces * 2).max(1);
            let b = surface_map_bytes_within(&root, RUN, &s.path, cap, u64::MAX).unwrap();
            assert!(u32_at(&b, 16) as usize <= cap.max(s.records.min(cap)), "{}", s.path);
        }
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// The results are read once per run and shared until `run.json` changes or the cache is
    /// cleared: three commands of the Results step cost one load, not three.
    #[test]
    fn a_run_is_loaded_once_and_again_when_its_manifest_changes() {
        let (dir, root) = runs_with("cache", "seats_spps");
        let (_, a) = open(&root, RUN).unwrap();
        let (_, b) = open(&root, RUN).unwrap();
        let (a, b) = (a.unwrap(), b.unwrap());
        assert!(Arc::ptr_eq(&a, &b), "the second open is the first's results");
        // The data reads share it too.
        let c = loaded(&root, RUN).unwrap();
        assert!(Arc::ptr_eq(&a, &c));
        // run.json rewritten (the same text, a later modification time): read afresh.
        let manifest = root.join(RUN).join(simpa_core::run::manifest::FILE_NAME);
        let text = std::fs::read(&manifest).unwrap();
        std::fs::write(&manifest, &text).unwrap();
        let later = SystemTime::now() + std::time::Duration::from_secs(5);
        std::fs::File::options()
            .write(true)
            .open(&manifest)
            .unwrap()
            .set_modified(later)
            .unwrap();
        let (_, d) = open(&root, RUN).unwrap();
        let d = d.unwrap();
        assert!(!Arc::ptr_eq(&a, &d), "a changed run.json reloads");
        assert_eq!(
            serde_json::to_value(results::checked_report(&a).unwrap()).unwrap(),
            serde_json::to_value(results::checked_report(&d).unwrap()).unwrap()
        );
        // Cleared: read afresh.
        forget_all();
        let (_, e) = open(&root, RUN).unwrap();
        assert!(!Arc::ptr_eq(&d, &e.unwrap()));
        // A refusal is not kept: the run repaired on disk reads again.
        let recp = root
            .join(RUN)
            .join("solve/Punctual receivers/Seat/Sound level.recp");
        let saved = std::fs::read(&recp).unwrap();
        std::fs::remove_file(&recp).unwrap();
        forget_all();
        assert!(open(&root, RUN).unwrap().1.is_none(), "refused without the file");
        std::fs::write(&recp, &saved).unwrap();
        assert!(open(&root, RUN).unwrap().1.is_some(), "loads once repaired");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn particles_carry_the_pbin_bit_for_bit_and_none_saved_is_said() {
        let (dir, root) = runs_with("part", "outputs_spps");
        let d = data_index(&root, RUN).unwrap().data.unwrap();
        for info in &d.particle_files {
            let b = particles_bytes(&root, RUN, info.freq_hz).unwrap();
            let file = pbin::read_file(&root.join(RUN).join("solve").join(&info.path)).unwrap();
            assert_eq!(u32_at(&b, 0), PART_MAGIC);
            assert_eq!(u32_at(&b, 4), LAYOUT_VERSION);
            let (np, nr) = (u32_at(&b, 8) as usize, u32_at(&b, 12) as usize);
            assert_eq!((np, nr), (file.particles.len(), file.steps.len()));
            assert_eq!((np, nr), (info.particles, info.recorded_steps));
            assert_eq!(u32_at(&b, 16), file.header.nb_time_step_max);
            assert_eq!(f32_bits_at(&b, 20), file.header.time_step.to_bits());
            assert_eq!(u32_at(&b, 24) as i32, info.freq_hz);
            let first = HEADER;
            let off = first + 4 * np;
            let pos = off + 4 * (np + 1);
            let en = pos + 12 * nr;
            assert_eq!(b.len(), en + 4 * nr);
            let mut k = 0usize;
            for (i, (h, steps)) in file.iter().enumerate() {
                assert_eq!(u32_at(&b, first + 4 * i), u32::from(h.first_time_step));
                assert_eq!(u32_at(&b, off + 4 * i) as usize, k);
                for s in steps {
                    for j in 0..3 {
                        assert_eq!(
                            f32_bits_at(&b, pos + 12 * k + 4 * j),
                            s.position[j].to_bits()
                        );
                    }
                    assert_eq!(f32_bits_at(&b, en + 4 * k), s.energy.to_bits());
                    k += 1;
                }
            }
            assert_eq!(u32_at(&b, off + 4 * np) as usize, k);
        }
        assert_eq!(
            particles_bytes(&root, RUN, 250).unwrap_err().code,
            "PARTICLES_NOT_FOUND"
        );
        std::fs::remove_dir_all(&dir).unwrap();
        // A run that saved none says so, by its own code.
        let (dir, root) = runs_with("part0", "seats_spps");
        assert!(
            data_index(&root, RUN)
                .unwrap()
                .data
                .unwrap()
                .particle_files
                .is_empty()
        );
        assert_eq!(
            particles_bytes(&root, RUN, 500).unwrap_err().code,
            "PARTICLES_NOT_SAVED"
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn an_echogram_is_the_recp_series_widened_and_each_sources_own() {
        let (dir, root) = runs_with("echo", "sources2_spps");
        let d = data_index(&root, RUN).unwrap().data.unwrap();
        let first = &d.echograms[0];
        assert!(
            !first.sources.is_empty(),
            "the fixture writes per-source echograms"
        );
        let e = echogram(&root, RUN, &first.label).unwrap();
        let solve = root.join(RUN).join("solve");
        let recp = gabe::read_file(&solve.join(&e.folder).join("Sound level.recp")).unwrap();
        assert_eq!(e.bands.len(), d.bands_hz.len());
        for (i, band) in e.bands.iter().enumerate() {
            assert_eq!(band.freq_hz, d.bands_hz[i]);
            let col = recp.columns[i + 1].floats().unwrap();
            let want: Vec<f64> = col.iter().map(|&x| f64::from(x)).collect();
            assert_eq!(band.energy, want, "{} Hz", band.freq_hz);
        }
        assert_eq!(
            e.sources
                .iter()
                .map(|s| s.source.as_str())
                .collect::<Vec<_>>(),
            first.sources.iter().map(String::as_str).collect::<Vec<_>>()
        );
        for s in &e.sources {
            let g = gabe::read_file(&solve.join(&s.file)).unwrap();
            for (i, band) in s.bands.iter().enumerate() {
                let want: Vec<f64> = g.columns[i + 1]
                    .floats()
                    .unwrap()
                    .iter()
                    .map(|&x| f64::from(x))
                    .collect();
                assert_eq!(band.energy, want, "{} {} Hz", s.source, band.freq_hz);
            }
        }
        assert_eq!(
            echogram(&root, RUN, "No such receiver").unwrap_err().code,
            "RECEIVER_NOT_FOUND"
        );
        std::fs::remove_dir_all(&dir).unwrap();
        // TCR writes no series.
        let dir = scratch("echo-tcr");
        let root = dir.join("runs");
        let tcr = "20260924-115031-155-tcr";
        copy_dir(&repo("tests/fixtures/results/seats_tcr"), &root.join(tcr));
        assert_eq!(
            echogram(&root, tcr, "Seat").unwrap_err().code,
            "NO_TIME_SERIES"
        );
        assert!(
            data_index(&root, tcr)
                .unwrap()
                .data
                .unwrap()
                .echograms
                .is_empty()
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
