//! Runs from the app (docs/investigations/2026-09-29-m11/PLAN.md, section 2): the run slot and
//! its thread, the stream of run events to the UI, the Runs tab's rows read from `run.json`, the
//! results state, the solvers' status and the material library.
//!
//! The core does the work: `run::run_project` meshes, launches and judges; the process layer's
//! Job Object is what Cancel ends. This module only starts it, streams what it reports, and
//! reads back what it wrote. Every number the UI shows about a run is formatted here, from the
//! manifest, with integer rules the e2e recomputes (2.3): the UI prints strings.
//!
//! **No acoustic value leaves this module.** A row carries diagnostics only: status, reasons,
//! particles lost, hashes, times. `ResultsState` says whether a run's results verify, never what
//! they are (M12 is the first milestone that may show one).

use std::collections::HashMap;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime};

use schemars::JsonSchema;
use serde::Serialize;
use simpa_core::bed::SOLVER_MANIFEST;
use simpa_core::bed::pe::{ManifestSource, SolverCheck, SolverManifest, check_solvers};
use simpa_core::geometry::import::{REFERENCE_MATERIALS, library_material};
use simpa_core::mesh;
use simpa_core::process::{self, CancelToken};
use simpa_core::run::gpu::{self, SPPS_GPU_EXE_NAME, SppsDevice};
use simpa_core::run::manager::{PREPROCESS_EXE_NAME, TETGEN_EXE_NAME, solver_exe_name};
use simpa_core::run::{
    DEFAULT_LOSS_LIMIT, ExeSearch, LineClass as CoreClass, MeshChoice, RunError, RunEvent,
    RunManifest, RunOptions, RunReport, RunSource, Stage, Status, run_project,
};
use simpa_core::schema::{F64, MaterialId, Rgb, SolverKind};
use simpa_core::{results, validate};
use tauri::ipc::Channel;

use crate::bridge::Session;
use crate::events::{BATCH_PERIOD, Batcher, Stream};
use crate::guard::{CmdError, CmdResult, lock, payload_message};
use crate::live;
use crate::scene;

/// The runs root of a project: `runs` beside the project file, the CLI's default (T1).
pub const RUNS_DIR: &str = "runs";
/// `run.json`, beside `solve/` in a run folder.
pub const MANIFEST_FILE: &str = "run.json";
/// How long quitting waits for a cancelled run to write its `run.json`.
pub const QUIT_WAIT: Duration = Duration::from_secs(3);

// ---- the types the UI reads (bindings.rs dumps them) --------------------------------------------

/// What `run_start` answers at once; the run itself reports through the stream.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, JsonSchema)]
pub struct RunStarted {
    pub solver: String,
    /// SPPS on the GPU (decision 70): the device line the probe printed; `None` on the CPU.
    pub gpu_device: Option<String>,
    /// The active variant's id, `None` for the project's own materials.
    pub variant: Option<String>,
    pub project_path: String,
    pub runs_root: String,
}

/// Where a streamed line came from: the solver (classified by the core) or TetGen while the run
/// meshes (INFO).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum LineSource {
    Solver,
    Mesh,
}

/// The core's line classes (`docs/solver-contract.md` Part B), as the Console labels them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "UPPERCASE")]
pub enum RunLineClass {
    Progress,
    Info,
    Ok,
    Warn,
    Fail,
}

impl From<CoreClass> for RunLineClass {
    fn from(c: CoreClass) -> Self {
        match c {
            CoreClass::Progress => RunLineClass::Progress,
            CoreClass::Info => RunLineClass::Info,
            CoreClass::Ok => RunLineClass::Ok,
            CoreClass::Warn => RunLineClass::Warn,
            CoreClass::Fail => RunLineClass::Fail,
        }
    }
}

/// One event of a run, in order. `seq` is 0, 1, 2, ... per run, so the UI can prove it lost
/// nothing.
#[derive(Clone, Debug, PartialEq, Serialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RunStreamEvent {
    /// The run folder exists (the core's `RunEvent::Started`). Always the first event.
    Started {
        seq: u64,
        t_ms: f64,
        /// The folder's name: the run's key everywhere in the UI.
        run: String,
        folder: String,
    },
    /// A stage begins: solvers, geometry, validate, mesh, export, pre_launch or solve.
    Stage { seq: u64, t_ms: f64, stage: String },
    /// A classified solver line, or a TetGen line (`source: mesh`, INFO).
    Line {
        seq: u64,
        t_ms: f64,
        source: LineSource,
        stream: Stream,
        class: RunLineClass,
        /// The contract's row id (`spps_end_of_calculation`, ...); empty for a mesh line.
        rule: String,
        /// The core's arrival index of a solver line (`Classified::seq`).
        solver_seq: Option<u64>,
        /// A PROGRESS line's percentage.
        progress: Option<f64>,
        continuation: bool,
        /// Exactly as the program printed it.
        text: String,
    },
    /// `run.json` is written. `row` is what `runs_list` lists for it.
    Ended {
        seq: u64,
        t_ms: f64,
        row: Box<RunRow>,
    },
    /// No run folder with its `run.json` could be made (the core's `RunError`), or the run
    /// thread panicked: `RUN_PROJECT`, `RUN_IO` or `RUN_PANIC`.
    Failed {
        seq: u64,
        t_ms: f64,
        error: CmdError,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, JsonSchema)]
pub struct RunStreamBatch {
    /// 0, 1, 2, ... per run.
    pub batch: u64,
    pub events: Vec<RunStreamEvent>,
    /// The run thread has finished; no batch follows.
    pub last: bool,
}

/// A run as the Runs tab lists it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "UPPERCASE")]
pub enum RunStatusUi {
    Ok,
    Fail,
    Crash,
    Cancelled,
    /// The active run, whose `run.json` is not written yet.
    Running,
    /// A run folder with no `run.json` that is not the active run: the app died mid-run
    /// (Stop-Process, a crash). Listed, never dropped and never guessed.
    Interrupted,
}

/// A reason with its core code and its UI code (row 22 (3): both are shown).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, JsonSchema)]
pub struct ReasonUi {
    pub code: String,
    pub ui_code: String,
    pub detail: String,
}

/// A band's particle loss: lost by meshing problems plus lost by infinite loops, over its total.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, JsonSchema)]
pub struct BandLossUi {
    pub freq_hz: i32,
    pub lost: u64,
    pub total: u32,
    /// The loss in percent, two decimals, rounded half up in integers ([`loss_pct`]).
    pub pct: String,
}

/// SPPS's particle loss, as the verdict holds it against the limit: per band.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, JsonSchema)]
pub struct LossUi {
    /// The largest band's `pct` (the first such band on a tie).
    pub worst_pct: String,
    pub worst_band_hz: i32,
    /// The verdict's limit, in percent ([`limit_pct`]).
    pub limit_pct: String,
    pub bands: Vec<BandLossUi>,
}

/// `run.json`'s `lines`: the solver's lines per class.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, JsonSchema)]
pub struct LineCounts {
    pub progress: u64,
    pub info: u64,
    pub ok: u64,
    pub warn: u64,
    pub fail: u64,
    /// Of the WARN lines, those no pattern matched.
    pub unclassified: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, JsonSchema)]
pub struct FileRefUi {
    pub path: String,
    pub sha256: String,
}

/// One row of the Runs tab: every field is read from the run's `run.json`, formatted here.
#[derive(Clone, Debug, PartialEq, Serialize, JsonSchema)]
pub struct RunRow {
    /// The folder's name: the row's key.
    pub run: String,
    /// 1, 2, ... by start time among this project's runs.
    pub number: u32,
    /// `run.json`'s `started` (RFC 3339).
    pub started: Option<String>,
    pub status: RunStatusUi,
    /// The stage the run ended in.
    pub stage: Option<String>,
    /// `spps` or `tcr`.
    pub solver: Option<String>,
    /// The variant written (its id); `None` for the project's own materials.
    pub variant: Option<String>,
    /// The verdict's reasons, in order.
    pub reasons: Vec<ReasonUi>,
    pub warnings: Vec<ReasonUi>,
    /// SPPS with its statistics table read.
    pub loss: Option<LossUi>,
    pub lines: Option<LineCounts>,
    /// The solver executable and its sha256.
    pub exe: Option<FileRefUi>,
    /// SPPS on the GPU (decision 70): `run.json`'s `gpu_device`, the device line `spps-gpu
    /// --probe` printed before the run; `None` for a run on the CPU.
    pub gpu_device: Option<String>,
    pub mesh_sha256: Option<String>,
    /// The executables checked against the verified build before the run; `None` for a CLI run
    /// or one written before M11.
    pub solvers: Option<Vec<SolverCheck>>,
    /// The solver build's verdict, the one the row shows (backlog 38): the core's
    /// (`results::solver_build`, the predicate the Results step and `simpa results` use too),
    /// never worked out from `solvers` in the UI. `None` for a row with no `run.json` that reads.
    pub solver_build: Option<SolverBuildUi>,
    /// The solver's wall time in seconds, one decimal ([`elapsed_s`]).
    pub elapsed_s: Option<String>,
    pub exit_code: Option<u32>,
    /// `run.json` is there but does not read.
    pub manifest_error: Option<String>,
}

/// A run's solver build, verified or not, with the reason's core and UI codes (backlog 38).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum SolverBuildUi {
    Verified,
    Unverified { reason: ReasonUi },
}

impl SolverBuildUi {
    /// The core's verdict, its reason with its UI code.
    pub fn of(b: &results::SolverBuild) -> Self {
        match b.reason() {
            None => SolverBuildUi::Verified,
            Some(r) => SolverBuildUi::Unverified {
                reason: reason_ui(r, None),
            },
        }
    }
}

/// The Runs tab: every run of the open project under its runs root.
#[derive(Clone, Debug, PartialEq, Serialize, JsonSchema)]
pub struct RunsView {
    pub root: String,
    pub rows: Vec<RunRow>,
    /// Run folders of other projects under the same root: counted, not listed.
    pub other_projects: usize,
    /// The active run's folder name.
    pub active: Option<String>,
}

/// Whether a run's results verify (`results::load`) and its solver build was verified
/// (`results::solver_build`), never a value from them.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, JsonSchema)]
pub struct ResultsState {
    pub run: String,
    /// The results load and the solver build was verified.
    pub verified: bool,
    pub refusal: Option<ReasonUi>,
    /// Why results that load are still not verified: the solver build's reason
    /// (`results::solver_build`, backlog 38). `None` when verified, and when refused.
    pub unverified: Option<ReasonUi>,
}

/// One of upstream's reference materials, as the library adds it (`library_material`).
#[derive(Clone, Debug, PartialEq, Serialize, JsonSchema)]
pub struct LibraryMaterial {
    /// Upstream's `idmateriau`.
    pub reference_id: u32,
    pub name: String,
    /// The `f32`-widened absorption, the same in every band, as a `.proj` import makes it.
    pub absorption: F64,
    pub color: Rgb,
}

/// The four executables a run needs, each checked against the verified build.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, JsonSchema)]
pub struct SolversStatus {
    pub checks: Vec<SolverCheck>,
    /// `SOLVER_NOT_FOUND` and/or `SOLVER_UNVERIFIED`; empty when all four are the verified build.
    pub blockers: Vec<String>,
}

/// Whether SPPS can run on the GPU here (decision 70): `spps-gpu.exe`, found as the other solvers
/// are and the verified build, answered `--probe` with a device. Asked once per app session
/// ([`spps_gpu_status`]). Never a blocker of the run: SPPS on the CPU and TCR are unaffected.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, JsonSchema)]
pub struct GpuStatus {
    pub available: bool,
    /// The device line `spps-gpu --probe` printed, when available: `NVIDIA GeForce RTX 5070,
    /// sm_120, 48 SMs, 11.9 GiB, driver CUDA 13.2, runtime 13.2`.
    pub device: Option<String>,
    /// Why SPPS cannot run on the GPU here, when not: the executable not found (where it was
    /// looked for), not the verified build, no CUDA device (its own words and exit), or no
    /// answer in time.
    pub reason: Option<String>,
}

// ---- numbers, formatted with integer rules (PLAN.md 2.3, T14) ----------------------------------

/// A band's loss in hundredths of a percent, rounded half up: `(lost·10000·2 + total) /
/// (2·total)` in integers. A band with no particles has lost none (0).
pub fn loss_hundredths(lost: u64, total: u64) -> u64 {
    if total == 0 {
        return 0;
    }
    (lost * 20_000 + total) / (2 * total)
}

/// [`loss_hundredths`] printed as `x.yz`.
pub fn loss_pct(lost: u64, total: u64) -> String {
    let h = loss_hundredths(lost, total);
    format!("{}.{:02}", h / 100, h % 100)
}

/// The loss limit (a fraction) in percent: the limit's shortest decimal (Rust's `{}` of an f64,
/// which never uses an exponent) with its point moved two places, so with no trailing zeros
/// (0.01 is `1`). Moving the point in the text, not multiplying the double by 100, is what keeps
/// 0.07 at `7` (M11 review 2, app 5: `0.07 * 100.0` is 7.000000000000001, and the Runs tab read
/// "7.000000000000001 % limit" for `simpa run --loss-limit 0.07`). NaN and the infinities print
/// as Rust prints them.
pub fn limit_pct(limit: f64) -> String {
    if !limit.is_finite() {
        return format!("{limit}");
    }
    let text = format!("{}", limit.abs());
    let (int, frac) = text.split_once('.').unwrap_or((&text, ""));
    let digits = format!("{int}{frac:0<2}");
    let (int, frac) = digits.split_at(int.len() + 2);
    let int = int.trim_start_matches('0');
    let frac = frac.trim_end_matches('0');
    let int = if int.is_empty() { "0" } else { int };
    let sign = if limit < 0.0 { "-" } else { "" };
    if frac.is_empty() {
        format!("{sign}{int}")
    } else {
        format!("{sign}{int}.{frac}")
    }
}

/// A wall time in ms as seconds with one decimal: `floor(ms / 100 + 0.5) / 10`, printed from the
/// integer number of tenths.
pub fn elapsed_s(ms: f64) -> String {
    let tenths = (ms / 100.0 + 0.5).floor();
    let t = if tenths.is_finite() && tenths > 0.0 {
        tenths as u64
    } else {
        0
    };
    format!("{}.{}", t / 10, t % 10)
}

// ---- UI codes for a run's reasons (PLAN.md 2.7, T7) ---------------------------------------------

/// The gate's name for TetGen 1.6.0's skipped facets.
pub const MESH_TETGEN_SKIPPED: &str = "MESH_TETGEN_SKIPPED";

/// The UI code of a reason a run ended with, in a run that ended in `stage`. Exhaustive by
/// construction (every code gets one, upper-cased by default); the named cases are:
/// - `tetgen_skipped_facets`: `MESH_TETGEN_SKIPPED`, the gate's name;
/// - a validator code: M10's table (`scene::ui_code`), where `material_placeholder` is
///   `MATERIALS_UNASSIGNED`;
/// - a verdict, results or solver-build code: upper-cased (`particle_loss_excess` is
///   `PARTICLE_LOSS_EXCESS`), at every stage;
/// - a mesher code in a run that ended at stage `mesh`: `MESH_` and the code upper-cased, or the
///   code upper-cased when it already starts with `mesh_`;
/// - anything else (a FAIL line's row id, a pre-launch code): upper-cased.
pub fn run_ui_code(code: &str, stage: Option<Stage>) -> String {
    if code == mesh::codes::TETGEN_SKIPPED_FACETS {
        return MESH_TETGEN_SKIPPED.to_string();
    }
    if is_validator_code(code) {
        return scene::ui_code(code);
    }
    let upper = code.to_ascii_uppercase();
    if simpa_core::run::verdict::codes::ALL.contains(&code)
        || results::codes::ALL.contains(&code)
        || results::build_codes::ALL.contains(&code)
    {
        return upper;
    }
    if stage == Some(Stage::Mesh) && !upper.starts_with("MESH_") {
        return format!("MESH_{upper}");
    }
    upper
}

fn is_validator_code(code: &str) -> bool {
    validate::RULES.iter().any(|r| r.code == code) || validate::STRUCTURAL_CODES.contains(&code)
}

pub(crate) fn reason_ui(r: &simpa_core::run::Reason, stage: Option<Stage>) -> ReasonUi {
    ReasonUi {
        code: r.code.clone(),
        ui_code: run_ui_code(&r.code, stage),
        detail: r.detail.clone(),
    }
}

// ---- rows from run.json ---------------------------------------------------------------------------

fn stage_name(s: Stage) -> &'static str {
    match s {
        Stage::Solvers => "solvers",
        Stage::Geometry => "geometry",
        Stage::Validate => "validate",
        Stage::Mesh => "mesh",
        Stage::Export => "export",
        Stage::PreLaunch => "pre_launch",
        Stage::Solve => "solve",
    }
}

fn solver_name(s: SolverKind) -> &'static str {
    match s {
        SolverKind::Spps => "spps",
        SolverKind::Tcr => "tcr",
    }
}

/// Whether `name` is a run folder's name: `yyyyMMdd-HHmmss-fff-(spps|tcr)` and an optional
/// `-<n>`. Nothing else is a run, so no path can climb out of a runs root through it.
pub fn is_run_name(name: &str) -> bool {
    let b = name.as_bytes();
    let digits =
        |r: std::ops::Range<usize>| b.get(r).is_some_and(|s| s.iter().all(u8::is_ascii_digit));
    if b.len() < 23 || !digits(0..8) || b[8] != b'-' || !digits(9..15) || b[15] != b'-' {
        return false;
    }
    if !digits(16..19) || b[19] != b'-' {
        return false;
    }
    let rest = &name[20..];
    let (solver, suffix) = match rest.split_once('-') {
        Some((s, n)) => (s, Some(n)),
        None => (rest, None),
    };
    (solver == "spps" || solver == "tcr")
        && suffix.is_none_or(|n| !n.is_empty() && n.bytes().all(|c| c.is_ascii_digit()))
}

/// Run folders in start order: by their stamp, then by their `-n` suffix.
fn run_order(name: &str) -> (String, u64) {
    let stamp = name.get(..19).unwrap_or(name).to_string();
    let n = name
        .rsplit_once('-')
        .and_then(|(_, n)| n.parse::<u64>().ok())
        .unwrap_or(1);
    (stamp, n)
}

/// The loss rows of SPPS's statistics.
fn loss_of(m: &RunManifest) -> Option<LossUi> {
    let stats = m.particles.as_ref()?;
    let bands: Vec<BandLossUi> = stats
        .bands
        .iter()
        .map(|b| {
            let lost = b.lost();
            BandLossUi {
                freq_hz: b.freq_hz,
                lost,
                total: b.total,
                pct: loss_pct(lost, u64::from(b.total)),
            }
        })
        .collect();
    let mut worst: Option<(u64, &BandLossUi)> = None;
    for b in &bands {
        let h = loss_hundredths(b.lost, u64::from(b.total));
        if worst.is_none_or(|(w, _)| h > w) {
            worst = Some((h, b));
        }
    }
    let (_, w) = worst?;
    Some(LossUi {
        worst_pct: w.pct.clone(),
        worst_band_hz: w.freq_hz,
        limit_pct: limit_pct(m.loss_limit),
        bands: bands.clone(),
    })
}

/// A row from a manifest that read.
pub fn row_from_manifest(run: &str, number: u32, m: &RunManifest) -> RunRow {
    let stage = Some(m.stage);
    RunRow {
        run: run.to_string(),
        number,
        started: Some(m.started.clone()),
        status: match m.verdict.status {
            Status::Ok => RunStatusUi::Ok,
            Status::Fail => RunStatusUi::Fail,
            Status::Crash => RunStatusUi::Crash,
            Status::Cancelled => RunStatusUi::Cancelled,
        },
        stage: Some(stage_name(m.stage).to_string()),
        solver: Some(solver_name(m.solver).to_string()),
        variant: match &m.source {
            RunSource::Project { variant, .. } => variant.clone(),
            RunSource::Fixture { .. } => None,
        },
        reasons: m
            .verdict
            .reasons
            .iter()
            .map(|r| reason_ui(r, stage))
            .collect(),
        warnings: m
            .verdict
            .warnings
            .iter()
            .map(|r| reason_ui(r, stage))
            .collect(),
        loss: loss_of(m),
        lines: Some(LineCounts {
            progress: m.lines.progress,
            info: m.lines.info,
            ok: m.lines.ok,
            warn: m.lines.warn,
            fail: m.lines.fail,
            unclassified: m.lines.unclassified,
        }),
        exe: Some(FileRefUi {
            path: m.exe.path.clone(),
            sha256: m.exe.sha256.clone(),
        }),
        gpu_device: m.gpu_device.clone(),
        mesh_sha256: m.mesh.as_ref().map(|x| x.mbin_sha256.clone()),
        solvers: m.solvers.clone(),
        solver_build: Some(SolverBuildUi::of(&results::solver_build(m))),
        elapsed_s: m.outcome.as_ref().map(|o| elapsed_s(o.elapsed_ms)),
        exit_code: m.outcome.as_ref().and_then(|o| o.exit_code),
        manifest_error: None,
    }
}

/// A row for a folder whose `run.json` is missing (Running or Interrupted) or does not read.
fn bare_row(
    run: &str,
    status: RunStatusUi,
    reason: Option<ReasonUi>,
    error: Option<String>,
) -> RunRow {
    let solver = run
        .get(20..)
        .map(|s| s.split('-').next().unwrap_or(s).to_string());
    RunRow {
        run: run.to_string(),
        number: 0,
        started: None,
        status,
        stage: None,
        solver,
        variant: None,
        reasons: reason.into_iter().collect(),
        warnings: Vec::new(),
        loss: None,
        lines: None,
        exe: None,
        gpu_device: None,
        mesh_sha256: None,
        solvers: None,
        solver_build: None,
        elapsed_s: None,
        exit_code: None,
        manifest_error: error,
    }
}

/// A path for comparing: absolute, `\` separators, lower case (Windows paths are compared
/// case-insensitively).
fn path_key(p: &Path) -> String {
    std::path::absolute(p)
        .unwrap_or_else(|_| p.to_path_buf())
        .to_string_lossy()
        .replace('/', "\\")
        .trim_end_matches('\\')
        .to_lowercase()
}

/// Whether a manifest's run is of the project at `project`: its `source.path`, made absolute
/// against the project's folder, is that path (T1).
fn belongs(m: &RunManifest, project: &Path) -> bool {
    let RunSource::Project { path, .. } = &m.source else {
        return false;
    };
    let p = PathBuf::from(path);
    let p = if p.is_absolute() {
        p
    } else {
        project.parent().unwrap_or(Path::new("")).join(p)
    };
    path_key(&p) == path_key(project)
}

/// The runs root of the project at `project`.
pub fn runs_root(project: &Path) -> PathBuf {
    project.parent().unwrap_or(Path::new("")).join(RUNS_DIR)
}

/// Every run of the project at `project` under `root`, numbered by start time. `active` is the
/// active run's folder name: listed as Running while it has no `run.json`.
pub fn list(root: &Path, project: &Path, active: Option<&str>) -> CmdResult<RunsView> {
    let mut view = RunsView {
        root: root.display().to_string(),
        rows: Vec::new(),
        other_projects: 0,
        active: active.map(str::to_string),
    };
    let entries = match std::fs::read_dir(root) {
        Ok(e) => e,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(view),
        Err(e) => {
            return Err(CmdError::new(
                "RUNS_IO",
                format!("could not read {}: {e}", root.display()),
            ));
        }
    };
    let mut names: Vec<String> = entries
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| is_run_name(n))
        .collect();
    names.sort_by_key(|n| run_order(n));
    for name in names {
        let manifest = root.join(&name).join(MANIFEST_FILE);
        let row = match std::fs::read_to_string(&manifest) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                if active == Some(name.as_str()) {
                    bare_row(&name, RunStatusUi::Running, None, None)
                } else {
                    let reason = ReasonUi {
                        code: results::codes::MANIFEST_MISSING.to_string(),
                        ui_code: results::codes::MANIFEST_MISSING.to_ascii_uppercase(),
                        detail: "no run.json".to_string(),
                    };
                    bare_row(&name, RunStatusUi::Interrupted, Some(reason), None)
                }
            }
            Err(e) => unreadable(&name, format!("{}: {e}", manifest.display())),
            Ok(text) => match RunManifest::from_json(&text) {
                Ok(m) if belongs(&m, project) => row_from_manifest(&name, 0, &m),
                Ok(_) => {
                    view.other_projects += 1;
                    continue;
                }
                Err(e) => unreadable(&name, format!("{}: {e}", manifest.display())),
            },
        };
        view.rows.push(row);
    }
    for (i, r) in view.rows.iter_mut().enumerate() {
        r.number = u32::try_from(i + 1).unwrap_or(u32::MAX);
    }
    Ok(view)
}

fn unreadable(name: &str, error: String) -> RunRow {
    let reason = ReasonUi {
        code: results::codes::MANIFEST_INVALID.to_string(),
        ui_code: results::codes::MANIFEST_INVALID.to_ascii_uppercase(),
        detail: error.clone(),
    };
    bare_row(name, RunStatusUi::Fail, Some(reason), Some(error))
}

/// Whether the run `run` of the project under `root` has results that verify, from a solver build
/// that was verified. `run` must be a bare run-folder name that exists under `root`; anything else
/// is `RUN_NOT_FOUND`. The state is `results_data::open`'s, which the Results step's reads share,
/// so the two cannot disagree; the results themselves are dropped here.
pub fn results_state(root: &Path, run: &str) -> CmdResult<ResultsState> {
    crate::results_data::open(root, run).map(|(state, _)| state)
}

// ---- the library and the solvers ------------------------------------------------------------------

/// Upstream's reference materials but `Default` (the placeholder), each with the absorption
/// `library_material` gives it: the one constructor a `.proj` import uses too.
pub fn material_library() -> Vec<LibraryMaterial> {
    REFERENCE_MATERIALS[1..]
        .iter()
        .map(|r| {
            let m = library_material(r, MaterialId::from_u128(0), 1);
            LibraryMaterial {
                reference_id: r.id,
                name: m.name,
                absorption: m.absorption[0],
                color: m.color,
            }
        })
        .collect()
}

/// The verified build, as this app was compiled with it. Never `$SIMPA_SOLVER_MANIFEST`: the app
/// has no override lever, unlike the CLI (`mesh_run.rs::solver_manifest`) — every app run's
/// `solver_manifest.source` is `embedded`.
fn manifest() -> CmdResult<SolverManifest> {
    SolverManifest::parse(SOLVER_MANIFEST, ManifestSource::Embedded).map_err(|e| {
        CmdError::new(
            "SOLVER_UNVERIFIED",
            format!("the embedded solvers/manifest.json does not read: {e}"),
        )
    })
}

/// Checks by (path, size, modification time): the same file is hashed once.
#[derive(Default)]
pub struct SolversCache {
    checks: HashMap<PathBuf, (u64, Option<SystemTime>, SolverCheck)>,
}

impl SolversCache {
    fn check(&mut self, name: &str, path: &Path, manifest: &SolverManifest) -> SolverCheck {
        let meta = std::fs::metadata(path).ok();
        let key = (
            meta.as_ref().map_or(0, std::fs::Metadata::len),
            meta.as_ref().and_then(|m| m.modified().ok()),
        );
        if let Some((len, time, c)) = self.checks.get(path)
            && (*len, *time) == key
            && c.name == name
        {
            return c.clone();
        }
        let c = check_solvers(&[(name, path)], manifest)
            .pop()
            .unwrap_or_else(|| not_found_check(name, "not checked".into()));
        self.checks
            .insert(path.to_path_buf(), (key.0, key.1, c.clone()));
        c
    }
}

fn not_found_check(name: &str, detail: String) -> SolverCheck {
    SolverCheck {
        name: name.to_string(),
        path: String::new(),
        code_sha256: None,
        raw_sha256: None,
        manifest_code_sha256: None,
        matches: false,
        detail: Some(detail),
    }
}

pub const SOLVER_NOT_FOUND: &str = "SOLVER_NOT_FOUND";
/// `run_start` with the GPU, where [`spps_gpu_status`] found none, or with TCR, which has none.
pub const SPPS_GPU_UNAVAILABLE: &str = "SPPS_GPU_UNAVAILABLE";
pub const SOLVER_UNVERIFIED: &str = "SOLVER_UNVERIFIED";
pub const RUN_ACTIVE: &str = "RUN_ACTIVE";

/// The four executables, found as the CLI finds them (`$SIMPA_SOLVERS_DIR`, beside the app, the
/// dev tree) and checked against the verified build.
pub fn solvers_status(cache: &Mutex<SolversCache>) -> CmdResult<SolversStatus> {
    let manifest = manifest()?;
    let search = ExeSearch::from_env(None);
    let mut cache = lock(cache, "solvers")?;
    let mut checks = Vec::new();
    let (mut missing, mut wrong) = (false, false);
    for name in simpa_core::bed::SOLVER_EXES {
        match search.find(name) {
            Ok(path) => {
                let c = cache.check(name, &path, &manifest);
                wrong |= !c.matches;
                checks.push(c);
            }
            Err(e) => {
                missing = true;
                checks.push(not_found_check(name, e.to_string()));
            }
        }
    }
    let mut blockers = Vec::new();
    if missing {
        blockers.push(SOLVER_NOT_FOUND.to_string());
    }
    if wrong {
        blockers.push(SOLVER_UNVERIFIED.to_string());
    }
    Ok(SolversStatus { checks, blockers })
}

/// The probe's answer for this app session, and the executable it found, once it found a device:
/// kept, since `spps-gpu --probe` starts the CUDA runtime, which need not be paid on every Simulate
/// step. A failure is never kept: a transient one, or a driver installed since, is probed again on
/// the next ask.
#[derive(Default)]
pub struct GpuCache {
    probed: Option<(GpuStatus, Option<PathBuf>)>,
}

/// The status of `search`'s `spps-gpu.exe` against `manifest`: the device, or why there is none.
pub fn probe_status(search: &ExeSearch, manifest: &SolverManifest) -> (GpuStatus, Option<PathBuf>) {
    match gpu::probe_verified(search, manifest, gpu::PROBE_TIMEOUT) {
        Ok((exe, line)) => (
            GpuStatus {
                available: true,
                device: Some(line),
                reason: None,
            },
            Some(exe),
        ),
        Err(why) => (
            GpuStatus {
                available: false,
                device: None,
                reason: Some(why),
            },
            None,
        ),
    }
}

/// SPPS on the GPU here: the kept answer once a device was found, else a fresh probe (with its
/// timeout and its kill on timeout, holding the cache's lock).
pub fn spps_gpu_status(cache: &Mutex<GpuCache>) -> CmdResult<GpuStatus> {
    Ok(probed(cache)?.0)
}

fn probed(cache: &Mutex<GpuCache>) -> CmdResult<(GpuStatus, Option<PathBuf>)> {
    probed_with(cache, &mut || {
        Ok(probe_status(&ExeSearch::from_env(None), &manifest()?))
    })
}

/// [`probed`] with the probe given: a success is kept, a failure is returned and asked again next
/// time.
fn probed_with(
    cache: &Mutex<GpuCache>,
    probe: &mut dyn FnMut() -> CmdResult<(GpuStatus, Option<PathBuf>)>,
) -> CmdResult<(GpuStatus, Option<PathBuf>)> {
    let mut c = lock(cache, "gpu")?;
    if let Some(kept) = &c.probed {
        return Ok(kept.clone());
    }
    let got = probe()?;
    if got.0.available {
        c.probed = Some(got.clone());
    }
    Ok(got)
}

// ---- the run slot and the run thread ----------------------------------------------------------------

/// The active run, if any. The session lock is held only for `start`'s checks: the run reads the
/// project file from disk, so an edit during a run changes the session, not the running file.
#[derive(Default)]
pub struct RunSlot {
    active: Option<ActiveRun>,
}

struct ActiveRun {
    id: u64,
    /// The run folder's name, once the core has made it.
    run: Option<String>,
    token: CancelToken,
    /// Set when `run_project` has returned: `run.json` is written, or never will be.
    finished: Arc<AtomicBool>,
}

impl RunSlot {
    /// A run is active: started and its `run.json` not yet written. The updater (M13) is to
    /// check this before it installs anything (A24).
    pub fn run_active(&self) -> bool {
        self.active.is_some()
    }

    /// The active run's folder name, once known.
    pub fn active_run(&self) -> Option<&str> {
        self.active.as_ref().and_then(|a| a.run.as_deref())
    }

    /// Sets the active run's cancel token: the core stops at its next stage, TetGen or the
    /// solver, and the process layer terminates the Job Object and returns only once no
    /// process of the tree is alive. No process id is looked up and nothing is swept.
    pub fn cancel(&self) -> bool {
        match &self.active {
            Some(a) => {
                a.token.cancel();
                true
            }
            None => false,
        }
    }

    fn finished_flag(&self) -> Option<Arc<AtomicBool>> {
        self.active.as_ref().map(|a| a.finished.clone())
    }
}

/// PQ4 in the backend (M11 review 2, app 2): New, Open and Import replace the project whose runs
/// the Runs tab lists, so they are refused while a run is active. The page refuses them too, but
/// its record of the run lives in its memory, which a reload of the page loses. The caller holds
/// the session lock, as `plan` does when it checks the slot.
pub fn refuse_while_running(slot: &Mutex<RunSlot>, what: &str) -> CmdResult<()> {
    if lock(slot, "run")?.run_active() {
        return Err(CmdError::new(
            RUN_ACTIVE,
            format!("{what}: a run is active: cancel it first"),
        ));
    }
    Ok(())
}

/// Cancels the active run, if any, and waits up to `limit` for its `run.json`. `true` when no
/// run is left running.
pub fn cancel_and_wait(slot: &Mutex<RunSlot>, limit: Duration) -> bool {
    let flag = match slot.lock() {
        Ok(s) => {
            s.cancel();
            s.finished_flag()
        }
        Err(_) => None,
    };
    let Some(flag) = flag else { return true };
    let t0 = Instant::now();
    while !flag.load(Ordering::SeqCst) {
        if t0.elapsed() >= limit {
            return false;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    true
}

static RUN_IDS: AtomicU64 = AtomicU64::new(1);

fn solver_kind(solver: &str) -> CmdResult<SolverKind> {
    match solver {
        "spps" => Ok(SolverKind::Spps),
        "tcr" => Ok(SolverKind::Tcr),
        other => Err(CmdError::new(
            "SOLVER_UNKNOWN",
            format!("unknown solver '{other}': spps or tcr"),
        )),
    }
}

/// `cpu` or `gpu`; `None` (a caller from before A5) is the CPU.
fn spps_device(device: Option<&str>) -> CmdResult<SppsDevice> {
    device.map_or(Ok(SppsDevice::Cpu), |d| {
        SppsDevice::parse(d).map_err(|e| CmdError::new("DEVICE_UNKNOWN", e))
    })
}

/// What `start` found to run.
struct Planned {
    project: PathBuf,
    variant: Option<String>,
    kind: SolverKind,
    /// SPPS on the GPU: the probe's device line.
    gpu_device: Option<String>,
    solver_exe: PathBuf,
    tetgen: PathBuf,
    preprocess: PathBuf,
    manifest: SolverManifest,
}

/// The checks of `run_start` (PLAN.md 2.2), in order: a project is open, has a path, has no
/// unsaved changes, no run is active, the project has no blocker, and the executables are found
/// and are the verified build.
fn plan(
    session: &Mutex<Session>,
    slot: &Mutex<RunSlot>,
    cache: &Mutex<SolversCache>,
    gpu_cache: &Mutex<GpuCache>,
    solver: &str,
    device: Option<&str>,
) -> CmdResult<Planned> {
    let kind = solver_kind(solver)?;
    let device = spps_device(device)?;
    if device == SppsDevice::Gpu && kind == SolverKind::Tcr {
        return Err(CmdError::new(
            SPPS_GPU_UNAVAILABLE,
            "TCR has no GPU build: the GPU runs SPPS only",
        ));
    }
    let (project, variant) = {
        let s = lock(session, "project")?;
        let info = s
            .info()
            .ok_or_else(|| CmdError::new("NO_PROJECT", "no project is open"))?;
        let path = s.path().map(Path::to_path_buf).ok_or_else(|| {
            CmdError::new(
                "SAVE_NO_PATH",
                "this project has never been saved: save it, then run",
            )
        })?;
        if info.dirty {
            return Err(CmdError::new(
                "RUN_DIRTY",
                "the project has unsaved changes: a run runs the file, so save first",
            ));
        }
        if lock(slot, "run")?.run_active() {
            return Err(CmdError::new(
                RUN_ACTIVE,
                "a run is active: cancel it first",
            ));
        }
        let mut blockers = s.project_blockers().unwrap_or_default();
        for b in s.solver_blockers(kind) {
            if !blockers.contains(&b) {
                blockers.push(b);
            }
        }
        if !blockers.is_empty() {
            return Err(CmdError::new(
                "RUN_BLOCKED",
                format!("the project cannot run: {}", blockers.join(", ")),
            ));
        }
        (path, info.active_variant)
    };
    let manifest = manifest()?;
    let search = ExeSearch::from_env(None);
    let find = |name: &str| {
        search
            .find(name)
            .map_err(|e| CmdError::new(SOLVER_NOT_FOUND, e.to_string()))
    };
    // SPPS on the GPU: the executable the session's probe found, with its device; refused with
    // the probe's reason when it found none. The probe is never run twice in a session.
    let (solver_name_checked, solver_exe, gpu_device) = match device {
        SppsDevice::Cpu => (solver_exe_name(kind), find(solver_exe_name(kind))?, None),
        SppsDevice::Gpu => match probed(gpu_cache)? {
            (
                GpuStatus {
                    available: true,
                    device: Some(line),
                    ..
                },
                Some(exe),
            ) => (SPPS_GPU_EXE_NAME, exe, Some(line)),
            (status, _) => {
                return Err(CmdError::new(
                    SPPS_GPU_UNAVAILABLE,
                    format!(
                        "SPPS cannot run on the GPU here: {}",
                        status.reason.as_deref().unwrap_or("no device")
                    ),
                ));
            }
        },
    };
    let tetgen = find(TETGEN_EXE_NAME)?;
    let preprocess = find(PREPROCESS_EXE_NAME)?;
    {
        let mut c = lock(cache, "solvers")?;
        for (name, path) in [
            (solver_name_checked, &solver_exe),
            (TETGEN_EXE_NAME, &tetgen),
            (PREPROCESS_EXE_NAME, &preprocess),
        ] {
            let check = c.check(name, path, &manifest);
            if !check.matches {
                return Err(CmdError::new(
                    SOLVER_UNVERIFIED,
                    format!(
                        "{} is not the verified build: code sha256 {}, the manifest's {} ({})",
                        path.display(),
                        check.code_sha256.as_deref().unwrap_or("unreadable"),
                        check.manifest_code_sha256.as_deref().unwrap_or("none"),
                        check.detail.as_deref().unwrap_or("")
                    ),
                ));
            }
        }
    }
    Ok(Planned {
        project,
        variant,
        kind,
        gpu_device,
        solver_exe,
        tetgen,
        preprocess,
        manifest,
    })
}

/// Where the live particles of a run go (B3): one LIVE v1 batch at a time (`live.rs`); `false`
/// when it was not delivered.
pub type LiveSink = Box<dyn FnMut(Vec<u8>) -> bool + Send>;

/// Starts a run of the open project with `solver` and returns at once; the run reports through
/// `channel` (PLAN.md 2.5). A run of SPPS on the GPU also sends its saved particles to `live` as
/// the solver writes them (B3); other runs never touch it.
#[allow(clippy::too_many_arguments)]
pub fn start(
    session: &Mutex<Session>,
    slot: &Arc<Mutex<RunSlot>>,
    cache: &Mutex<SolversCache>,
    gpu_cache: &Mutex<GpuCache>,
    solver: &str,
    device: Option<&str>,
    channel: Channel<RunStreamBatch>,
    live: Option<LiveSink>,
) -> CmdResult<RunStarted> {
    let p = plan(session, slot, cache, gpu_cache, solver, device)?;
    let root = runs_root(&p.project);
    let started = RunStarted {
        solver: solver_name(p.kind).to_string(),
        gpu_device: p.gpu_device.clone(),
        variant: p.variant.clone(),
        project_path: p.project.display().to_string(),
        runs_root: root.display().to_string(),
    };
    let token = CancelToken::new();
    let finished = Arc::new(AtomicBool::new(false));
    let id = RUN_IDS.fetch_add(1, Ordering::SeqCst);
    {
        let mut s = lock(slot, "run")?;
        if s.run_active() {
            return Err(CmdError::new(
                RUN_ACTIVE,
                "a run is active: cancel it first",
            ));
        }
        s.active = Some(ActiveRun {
            id,
            run: None,
            token: token.clone(),
            finished: finished.clone(),
        });
    }
    let opts = RunOptions {
        solver: p.kind,
        solver_exe: p.solver_exe.clone(),
        runs_root: root.clone(),
        loss_limit: DEFAULT_LOSS_LIMIT,
        cancel_after_ms: None,
        cancel_after_progress: None,
        verify: Some(p.manifest.clone()),
        gpu_device: p.gpu_device.clone(),
    };
    let mesh = MeshChoice::Build {
        tetgen: p.tetgen.clone(),
        preprocess: Some(p.preprocess.clone()),
    };
    let thread_slot = slot.clone();
    let project = p.project.clone();
    let variant = p.variant.clone();
    let live = if p.gpu_device.is_some() { live } else { None };
    let spawned = std::thread::Builder::new()
        .name("run".to_string())
        .spawn(move || {
            let job = RunJob {
                id,
                project: p.project,
                token,
                finished,
                slot: thread_slot,
                channel,
                live,
            };
            run_thread(job, |token, on_event| {
                run_project(&project, variant.as_deref(), &mesh, &opts, token, on_event)
            });
        });
    if let Err(e) = spawned {
        if let Ok(mut s) = slot.lock() {
            s.active = None;
        }
        return Err(CmdError::new(
            "RUN_IO",
            format!("could not start the run thread: {e}"),
        ));
    }
    Ok(started)
}

struct RunJob {
    id: u64,
    /// The project file: its runs are listed to make the `ended` row.
    project: PathBuf,
    token: CancelToken,
    finished: Arc<AtomicBool>,
    slot: Arc<Mutex<RunSlot>>,
    channel: Channel<RunStreamBatch>,
    /// B3: SPPS on the GPU's saved particles, tailed from the run's stream file while it runs.
    live: Option<LiveSink>,
}

fn stream_of(s: process::Stream) -> Stream {
    match s {
        process::Stream::Stdout => Stream::Stdout,
        process::Stream::Stderr => Stream::Stderr,
    }
}

/// Maps one core event to its stream event (without `seq` and `t_ms`, which the caller sets).
fn stream_event(e: &RunEvent, seq: u64, t_ms: f64) -> RunStreamEvent {
    match e {
        RunEvent::Started(dir) => RunStreamEvent::Started {
            seq,
            t_ms,
            run: dir
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default(),
            folder: dir.display().to_string(),
        },
        RunEvent::Stage(s) => RunStreamEvent::Stage {
            seq,
            t_ms,
            stage: stage_name(*s).to_string(),
        },
        RunEvent::MeshLine(l) => RunStreamEvent::Line {
            seq,
            t_ms,
            source: LineSource::Mesh,
            stream: stream_of(l.stream),
            class: RunLineClass::Info,
            rule: String::new(),
            solver_seq: None,
            progress: None,
            continuation: false,
            text: l.text.clone(),
        },
        RunEvent::SolverLine(c) => RunStreamEvent::Line {
            seq,
            t_ms,
            source: LineSource::Solver,
            stream: stream_of(c.line.stream),
            class: c.class.into(),
            rule: c.id.to_string(),
            solver_seq: Some(c.seq),
            progress: c.progress,
            continuation: c.continuation,
            text: c.line.text.clone(),
        },
    }
}

/// The run thread: `run` (the core's `run_project`) with every event streamed, a panic caught,
/// and the slot freed before the last event, so the UI can start the next run when it reads
/// `ended`. With a live sink, the run folder's stream file is tailed from `started` on, and the
/// tail is stopped and joined before the last event, so no live batch follows `ended`.
fn run_thread<F>(job: RunJob, run: F)
where
    F: FnOnce(&CancelToken, &mut dyn FnMut(&RunEvent)) -> Result<RunReport, RunError>,
{
    let RunJob {
        id,
        project,
        token,
        finished,
        slot,
        channel,
        live,
    } = job;
    let mut live = live;
    let live_stop = Arc::new(AtomicBool::new(false));
    let mut tail: Option<std::thread::JoinHandle<live::TailStats>> = None;
    let batcher: Batcher<RunStreamEvent> =
        Batcher::spawn(BATCH_PERIOD, move |batch, events, last| {
            channel
                .send(RunStreamBatch {
                    batch,
                    events,
                    last,
                })
                .is_ok()
        });
    let t0 = Instant::now();
    let mut seq = 0u64;
    let mut run_dir: Option<PathBuf> = None;
    let outcome = catch_unwind(AssertUnwindSafe(|| {
        run(&token, &mut |e: &RunEvent| {
            if let RunEvent::Started(dir) = e {
                run_dir = Some(dir.to_path_buf());
                if let Some(sink) = live.take() {
                    tail = live::spawn_tail(live::stream_path(dir), live::POLL, live_stop.clone(), sink).ok();
                }
                let name = dir.file_name().map(|n| n.to_string_lossy().into_owned());
                if let Ok(mut s) = slot.lock()
                    && let Some(a) = s.active.as_mut()
                    && a.id == id
                {
                    a.run = name;
                }
            }
            batcher.push(stream_event(e, seq, t0.elapsed().as_secs_f64() * 1e3));
            seq += 1;
        })
    }));
    live_stop.store(true, Ordering::SeqCst);
    if let Some(t) = tail {
        let _ = t.join();
    }
    finished.store(true, Ordering::SeqCst);
    if let Ok(mut s) = slot.lock()
        && s.active.as_ref().is_some_and(|a| a.id == id)
    {
        s.active = None;
    }
    let t_ms = t0.elapsed().as_secs_f64() * 1e3;
    let last = match outcome {
        Ok(Ok(report)) => {
            let name = report
                .dir
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            let root = runs_root(&project);
            let row = list(&root, &project, None)
                .ok()
                .and_then(|v| v.rows.into_iter().find(|r| r.run == name))
                .unwrap_or_else(|| row_from_manifest(&name, 0, &report.manifest));
            RunStreamEvent::Ended {
                seq,
                t_ms,
                row: Box::new(row),
            }
        }
        Ok(Err(e)) => RunStreamEvent::Failed {
            seq,
            t_ms,
            error: run_error(&e),
        },
        Err(payload) => RunStreamEvent::Failed {
            seq,
            t_ms,
            error: CmdError::new(
                "RUN_PANIC",
                format!(
                    "the run thread panicked: {}{}",
                    payload_message(&*payload),
                    run_dir
                        .map(|d| format!("; its folder {} has no run.json", d.display()))
                        .unwrap_or_default()
                ),
            ),
        },
    };
    batcher.push(last);
    let _ = batcher.finish();
}

fn run_error(e: &RunError) -> CmdError {
    let code = match e {
        RunError::Io { .. } => "RUN_IO",
        RunError::Project { .. } | RunError::Fixture { .. } => "RUN_PROJECT",
    };
    CmdError::new(code, e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::MATERIALS_UNASSIGNED;
    use simpa_core::run::{LINE_RULES, run_folder};

    /// Every `LINE_RULES` id: a FAIL line's reason is its row id.
    fn line_rule_ids() -> Vec<&'static str> {
        LINE_RULES.iter().map(|r| r.id).collect()
    }

    /// A5 bed item 4: the probe pointed at a folder with no `spps-gpu.exe` answers unavailable,
    /// with the reason naming where it looked; one that is not the verified build is never
    /// started and says so. Neither has a device, and `plan` refuses the GPU with that reason.
    #[test]
    fn a5_the_gpu_is_unavailable_with_its_reason_when_the_probe_finds_nothing() {
        let dir = std::env::temp_dir().join(format!("nm-a5-gpu-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let search = ExeSearch {
            explicit: None,
            solvers_dir: Some(dir.clone()),
            exe_dir: None,
        };
        let m = manifest().unwrap();
        let (missing, exe) = probe_status(&search, &m);
        assert!(
            !missing.available && missing.device.is_none() && exe.is_none(),
            "{missing:?}"
        );
        let why = missing.reason.unwrap();
        assert!(why.contains("spps-gpu.exe not found"), "{why}");
        assert!(why.contains(&dir.display().to_string()), "{why}");
        // A file there that is not the verified build: refused before it is ever started.
        std::fs::write(dir.join(SPPS_GPU_EXE_NAME), b"MZ not a solver").unwrap();
        let (bad, exe) = probe_status(&search, &m);
        assert!(!bad.available && exe.is_none(), "{bad:?}");
        assert!(
            bad.reason
                .as_deref()
                .unwrap()
                .contains("is not the verified build"),
            "{bad:?}"
        );
        std::fs::remove_dir_all(&dir).unwrap();
        // The device names: cpu, gpu, none (the CPU); anything else is refused.
        assert_eq!(spps_device(None).unwrap(), SppsDevice::Cpu);
        assert_eq!(spps_device(Some("gpu")).unwrap(), SppsDevice::Gpu);
        assert_eq!(
            spps_device(Some("cuda")).unwrap_err().code,
            "DEVICE_UNKNOWN"
        );
    }

    /// A5 audit fix 2: a failed probe is asked again on the next call (a transient failure, a
    /// driver installed since); the first success is kept and the probe is not run again.
    #[test]
    fn a5_a_failed_probe_is_asked_again_and_a_success_is_kept() {
        let cache = Mutex::new(GpuCache::default());
        let mut calls = 0;
        let mut answers = vec![
            (true, "RTX"),
            (false, "no CUDA device (--probe exited 1)"),
            (false, "did not answer within 5 s"),
        ];
        let mut probe = || {
            calls += 1;
            let (ok, text) = answers.pop().expect("asked more often than answered");
            Ok((
                GpuStatus {
                    available: ok,
                    device: ok.then(|| text.to_string()),
                    reason: (!ok).then(|| text.to_string()),
                },
                ok.then(|| PathBuf::from("spps-gpu.exe")),
            ))
        };
        let a = probed_with(&cache, &mut probe).unwrap();
        assert!(!a.0.available && a.0.reason.as_deref() == Some("did not answer within 5 s"));
        let b = probed_with(&cache, &mut probe).unwrap();
        assert!(!b.0.available, "{b:?}");
        let c = probed_with(&cache, &mut probe).unwrap();
        assert_eq!(c.0.device.as_deref(), Some("RTX"));
        let d = probed_with(&cache, &mut probe).unwrap();
        assert_eq!(d, c);
        drop(probe);
        assert_eq!(calls, 3, "two failures asked again, the success kept");
    }

    #[test]
    fn the_ui_code_table_covers_every_code_a_run_can_carry() {
        let mut all: Vec<&str> = Vec::new();
        all.extend(simpa_core::run::verdict::codes::ALL);
        all.extend(results::codes::ALL);
        all.extend(results::build_codes::ALL);
        all.extend(mesh::codes::ALL);
        all.extend(validate::RULES.iter().map(|r| r.code));
        all.extend(validate::STRUCTURAL_CODES);
        all.extend(line_rule_ids());
        for code in &all {
            for stage in [
                None,
                Some(Stage::Solvers),
                Some(Stage::Geometry),
                Some(Stage::Validate),
                Some(Stage::Mesh),
                Some(Stage::Export),
                Some(Stage::PreLaunch),
                Some(Stage::Solve),
            ] {
                let ui = run_ui_code(code, stage);
                assert!(!ui.is_empty(), "{code}");
                assert!(
                    ui.bytes()
                        .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'_'),
                    "{code} -> {ui}"
                );
            }
        }
        // The named keys are real codes, so none is stale.
        assert!(mesh::codes::ALL.contains(&"tetgen_skipped_facets"));
        assert!(
            validate::RULES
                .iter()
                .any(|r| r.code == "material_placeholder")
        );
        // The gate's names and the plan's examples.
        assert_eq!(
            run_ui_code("tetgen_skipped_facets", Some(Stage::Mesh)),
            MESH_TETGEN_SKIPPED
        );
        assert_eq!(
            run_ui_code("tetgen_self_intersection", Some(Stage::Mesh)),
            "MESH_TETGEN_SELF_INTERSECTION"
        );
        assert_eq!(
            run_ui_code("mesh_invalid", Some(Stage::Mesh)),
            "MESH_INVALID"
        );
        assert_eq!(
            run_ui_code("receiver_outside_volume", Some(Stage::Validate)),
            "RECEIVER_OUTSIDE"
        );
        assert_eq!(
            run_ui_code("material_placeholder", Some(Stage::Validate)),
            MATERIALS_UNASSIGNED
        );
        assert_eq!(
            run_ui_code("particle_loss_excess", Some(Stage::Solve)),
            "PARTICLE_LOSS_EXCESS"
        );
        assert_eq!(
            run_ui_code("results_run_failed", None),
            "RESULTS_RUN_FAILED"
        );
        // A code the verdict and the mesher share keeps the verdict's name at stage mesh.
        assert_eq!(run_ui_code("cancelled", Some(Stage::Mesh)), "CANCELLED");
        assert_eq!(
            run_ui_code("solver_unverified", Some(Stage::Solvers)),
            "SOLVER_UNVERIFIED"
        );
        // A solver-build code (backlog 38) keeps its name on a run that ended at stage mesh.
        assert_eq!(
            run_ui_code("solver_build_unrecorded", Some(Stage::Mesh)),
            "SOLVER_BUILD_UNRECORDED"
        );
    }

    #[test]
    fn the_loss_and_time_rules_round_half_up_in_integers() {
        assert_eq!(loss_pct(0, 150_000), "0.00");
        assert_eq!(loss_pct(1, 10_000), "0.01");
        // Exact halves round up: 1 of 800 is 0.125 %, 1 of 400 is 0.25 %.
        assert_eq!(loss_pct(1, 800), "0.13");
        assert_eq!(loss_pct(1, 400), "0.25");
        assert_eq!(loss_pct(1, 3), "33.33");
        assert_eq!(loss_pct(2, 3), "66.67");
        assert_eq!(loss_pct(1, 1), "100.00");
        assert_eq!(loss_pct(0, 0), "0.00");
        // Just below and at a half of a hundredth.
        assert_eq!(loss_pct(49, 1_000_000), "0.00");
        assert_eq!(loss_pct(50, 1_000_000), "0.01");
        assert_eq!(limit_pct(0.01), "1");
        assert_eq!(limit_pct(0.015), "1.5");
        assert_eq!(limit_pct(DEFAULT_LOSS_LIMIT), "1");
        // M11 review 2, app 5: limits whose double times 100 is not the decimal times 100.
        assert_eq!(limit_pct(0.07), "7");
        assert_eq!(limit_pct(0.035), "3.5");
        assert_eq!(limit_pct(0.29), "29");
        assert_eq!(limit_pct(0.14), "14");
        assert_eq!(limit_pct(0.001), "0.1");
        assert_eq!(limit_pct(0.00001), "0.001");
        assert_eq!(limit_pct(0.5), "50");
        assert_eq!(limit_pct(1.0), "100");
        assert_eq!(limit_pct(12.5), "1250");
        assert_eq!(limit_pct(0.0), "0");
        assert_eq!(limit_pct(-0.07), "-7");
        assert_eq!(limit_pct(f64::NAN), "NaN");
        assert_eq!(limit_pct(f64::INFINITY), "inf");
        assert_eq!(elapsed_s(1497.2341), "1.5");
        assert_eq!(elapsed_s(1449.999), "1.4");
        assert_eq!(elapsed_s(1450.0), "1.5");
        assert_eq!(elapsed_s(64_269.289_9), "64.3");
        assert_eq!(elapsed_s(0.0), "0.0");
        assert_eq!(elapsed_s(f64::NAN), "0.0");
    }

    #[test]
    fn run_names_are_checked_before_any_path_is_built() {
        for good in [
            "20260929-195300-123-spps",
            "20260929-195300-123-tcr",
            "20260929-195300-123-spps-2",
            "20260929-195300-123-tcr-17",
        ] {
            assert!(is_run_name(good), "{good}");
        }
        for bad in [
            "",
            "..",
            "../20260929-195300-123-spps",
            "20260929-195300-123-spps/..",
            "20260929-195300-123-spps\\x",
            "20260929-195300-123-ray",
            "20260929-195300-123-spps-",
            "20260929-195300-123-spps-2a",
            "2026092-195300-123-spps",
            "20260929_195300-123-spps",
            "C:\\runs\\20260929-195300-123-spps",
        ] {
            assert!(!is_run_name(bad), "{bad}");
        }
        let root = std::env::temp_dir().join(format!("simpa-app-runs-{}", std::process::id()));
        for bad in ["..", "../x", "C:\\Windows"] {
            assert_eq!(results_state(&root, bad).unwrap_err().code, "RUN_NOT_FOUND");
        }
        assert_eq!(
            results_state(&root, "20260929-195300-123-spps")
                .unwrap_err()
                .code,
            "RUN_NOT_FOUND",
            "a good name with no folder"
        );
    }

    #[test]
    fn the_library_is_upstreams_reference_materials_widened() {
        let lib = material_library();
        assert_eq!(lib.len(), 11);
        assert!(lib.iter().all(|m| m.name != "Default"));
        for (m, r) in lib.iter().zip(&REFERENCE_MATERIALS[1..]) {
            assert_eq!(m.reference_id, r.id);
            assert_eq!(m.name, r.name);
            assert_eq!(
                m.absorption.get(),
                simpa_core::config_xml::widen_f32(r.absorption)
            );
            assert_eq!(m.color, Rgb(r.color[0], r.color[1], r.color[2]));
        }
        let thirty = lib.iter().find(|m| m.reference_id == 21).unwrap();
        assert_eq!(thirty.name, "30% absorbing");
        assert_eq!(thirty.absorption.get(), 0.3);
    }

    fn repo(rel: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join(rel)
    }

    /// A fresh folder on the system temp drive (never the exFAT repo drive), removed by the test.
    fn scratch(tag: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "simpa-app-m11-{tag}-{}-{nanos}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// A solver that runs nothing: a batch file printing SPPS-shaped lines, one banner, `n`
    /// progress lines and the end line.
    fn fake_solver(dir: &Path, n: usize) -> PathBuf {
        let bat = dir.join("fake_spps.bat");
        let mut text = "@echo off\r\necho SPPS version 2.2.1\r\n".to_string();
        for i in 1..=n {
            text.push_str(&format!("echo #{i}\r\n"));
        }
        text.push_str("echo End of calculation.\r\n");
        std::fs::write(&bat, text).unwrap();
        bat
    }

    fn fake_options(exe: PathBuf, root: PathBuf) -> RunOptions {
        RunOptions {
            solver: SolverKind::Spps,
            solver_exe: exe,
            runs_root: root,
            loss_limit: DEFAULT_LOSS_LIMIT,
            cancel_after_ms: None,
            cancel_after_progress: None,
            verify: None,
            gpu_device: None,
        }
    }

    /// The stream of a run, driven by the core's `run_folder` on a fixture folder with a fake
    /// solver: `started` first, every event once in `seq` order, every solver line once in its
    /// own order, `ended` last in the batch marked `last`, and the slot freed by then.
    #[test]
    fn a_run_streams_started_first_every_line_once_and_ended_last() {
        let dir = scratch("stream");
        let exe = fake_solver(&dir, 40);
        let batches = Arc::new(Mutex::new(Vec::<serde_json::Value>::new()));
        let got = batches.clone();
        let channel: Channel<RunStreamBatch> = Channel::new(move |body| {
            if let tauri::ipc::InvokeResponseBody::Json(text) = body {
                got.lock()
                    .unwrap()
                    .push(serde_json::from_str(&text).unwrap());
            }
            Ok(())
        });
        let slot = Arc::new(Mutex::new(RunSlot::default()));
        let finished = Arc::new(AtomicBool::new(false));
        let token = CancelToken::new();
        slot.lock().unwrap().active = Some(ActiveRun {
            id: 7,
            run: None,
            token: token.clone(),
            finished: finished.clone(),
        });
        let opts = fake_options(exe, dir.join("runs"));
        let fixture = repo("tests/fixtures/runs/spps_ok");
        let job = RunJob {
            id: 7,
            project: dir.join("none.simpa"),
            token,
            finished: finished.clone(),
            slot: slot.clone(),
            channel,
            live: None,
        };
        run_thread(job, |t, on| run_folder(&fixture, &opts, t, on));

        assert!(finished.load(Ordering::SeqCst));
        assert!(!slot.lock().unwrap().run_active(), "the slot is freed");
        let batches = batches.lock().unwrap();
        let numbers: Vec<u64> = batches
            .iter()
            .map(|b| b["batch"].as_u64().unwrap())
            .collect();
        assert_eq!(numbers, (0..batches.len() as u64).collect::<Vec<_>>());
        let lasts: Vec<bool> = batches
            .iter()
            .map(|b| b["last"].as_bool().unwrap())
            .collect();
        assert_eq!(lasts.iter().filter(|l| **l).count(), 1);
        assert!(lasts.last().copied().unwrap());
        let events: Vec<&serde_json::Value> = batches
            .iter()
            .flat_map(|b| b["events"].as_array().unwrap())
            .collect();
        let seqs: Vec<u64> = events.iter().map(|e| e["seq"].as_u64().unwrap()).collect();
        assert_eq!(seqs, (0..events.len() as u64).collect::<Vec<_>>());
        assert_eq!(events[0]["kind"], "started");
        let last = events.last().unwrap();
        assert_eq!(last["kind"], "ended", "{last:#}");
        let run = events[0]["run"].as_str().unwrap();
        assert!(is_run_name(run), "{run}");
        assert_eq!(last["row"]["run"], run);
        // Every solver line once, in the core's own order, and as the manifest counts them.
        let solver: Vec<&&serde_json::Value> = events
            .iter()
            .filter(|e| e["kind"] == "line" && e["source"] == "solver")
            .collect();
        let solver_seqs: Vec<u64> = solver
            .iter()
            .map(|e| e["solver_seq"].as_u64().unwrap())
            .collect();
        assert_eq!(solver_seqs, (0..solver.len() as u64).collect::<Vec<_>>());
        let count = |c: &str| solver.iter().filter(|e| e["class"] == c).count() as u64;
        assert_eq!(count("PROGRESS"), 40);
        assert_eq!(count("OK"), 1, "End of calculation.");
        let lines = &last["row"]["lines"];
        for (key, class) in [
            ("progress", "PROGRESS"),
            ("info", "INFO"),
            ("ok", "OK"),
            ("warn", "WARN"),
            ("fail", "FAIL"),
        ] {
            assert_eq!(lines[key].as_u64().unwrap(), count(class), "{key}");
        }
        let stages: Vec<&str> = events
            .iter()
            .filter(|e| e["kind"] == "stage")
            .map(|e| e["stage"].as_str().unwrap())
            .collect();
        assert_eq!(stages, ["pre_launch", "solve"]);
        let progress = solver.iter().find(|e| e["class"] == "PROGRESS").unwrap();
        assert_eq!(progress["progress"].as_f64(), Some(1.0));
        assert_eq!(progress["text"], "#1");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// A panic inside the run ends as a `failed` event with `RUN_PANIC`, still last, and the
    /// slot is freed.
    #[test]
    fn a_panicking_run_ends_failed_and_frees_the_slot() {
        let batches = Arc::new(Mutex::new(Vec::<serde_json::Value>::new()));
        let got = batches.clone();
        let channel: Channel<RunStreamBatch> = Channel::new(move |body| {
            if let tauri::ipc::InvokeResponseBody::Json(text) = body {
                got.lock()
                    .unwrap()
                    .push(serde_json::from_str(&text).unwrap());
            }
            Ok(())
        });
        let slot = Arc::new(Mutex::new(RunSlot::default()));
        let finished = Arc::new(AtomicBool::new(false));
        let token = CancelToken::new();
        slot.lock().unwrap().active = Some(ActiveRun {
            id: 9,
            run: None,
            token: token.clone(),
            finished: finished.clone(),
        });
        let job = RunJob {
            id: 9,
            project: PathBuf::from("none.simpa"),
            token,
            finished,
            slot: slot.clone(),
            channel,
            live: None,
        };
        run_thread(job, |_, _| panic!("a deliberate panic in the run"));
        assert!(!slot.lock().unwrap().run_active());
        let batches = batches.lock().unwrap();
        let events: Vec<&serde_json::Value> = batches
            .iter()
            .flat_map(|b| b["events"].as_array().unwrap())
            .collect();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0]["kind"], "failed");
        assert_eq!(events[0]["error"]["code"], "RUN_PANIC");
        assert!(batches.last().unwrap()["last"].as_bool().unwrap());
    }

    /// B3: with a live sink, the run folder's stream file is tailed from `started` on, its
    /// particles reach the sink as LIVE v1 batches, and the tail has stopped before the last event,
    /// so nothing reaches the sink after `run_thread` returns.
    #[test]
    fn a_live_run_tails_its_stream_and_stops_before_the_last_event() {
        let dir = scratch("live");
        let run_dir = dir.join("20261007-101010-000-spps");
        let solve = run_dir.join("solve");
        std::fs::create_dir_all(&solve).unwrap();
        let channel: Channel<RunStreamBatch> = Channel::new(|_| Ok(()));
        let got = Arc::new(Mutex::new(Vec::<Vec<u8>>::new()));
        let sink_got = got.clone();
        let done = Arc::new(AtomicBool::new(false));
        let late = Arc::new(AtomicBool::new(false));
        let (sink_done, sink_late) = (done.clone(), late.clone());
        let slot = Arc::new(Mutex::new(RunSlot::default()));
        let job = RunJob {
            id: 13,
            project: dir.join("none.simpa"),
            token: CancelToken::new(),
            finished: Arc::new(AtomicBool::new(false)),
            slot,
            channel,
            live: Some(Box::new(move |b| {
                if sink_done.load(Ordering::SeqCst) {
                    sink_late.store(true, Ordering::SeqCst);
                }
                sink_got.lock().unwrap().push(b);
                true
            })),
        };
        let stream = solve.join(live::STREAM_FILE);
        run_thread(job, |_, on| {
            on(&RunEvent::Started(&run_dir));
            // the solver: a header, then one frame a poll apart, then a torn frame it never finishes
            let mut b: Vec<u8> = Vec::new();
            for v in [live::STREAM_MAGIC, live::STREAM_VERSION, 0.001f32.to_bits(), 100, 2, 1, 1000] {
                b.extend_from_slice(&v.to_le_bytes());
            }
            use std::io::Write;
            let mut file = std::fs::File::create(&stream).unwrap();
            file.write_all(&b).unwrap();
            for i in 0..2u32 {
                let mut f = Vec::new();
                for v in [32u32, 1000, i, 5, 1] {
                    f.extend_from_slice(&v.to_le_bytes());
                }
                for v in [1.0f32, 2.0, 3.0, 0.5] {
                    f.extend_from_slice(&v.to_le_bytes());
                }
                file.write_all(&f).unwrap();
                file.flush().unwrap();
                std::thread::sleep(live::POLL * 3);
            }
            file.write_all(&64u32.to_le_bytes()).unwrap();
            file.flush().unwrap();
            Err(RunError::Io {
                path: run_dir.clone(),
                source: std::io::Error::other("the fake run ends here"),
            })
        });
        done.store(true, Ordering::SeqCst);
        std::thread::sleep(live::POLL * 2);
        assert!(!late.load(Ordering::SeqCst), "a batch reached the sink after the run thread ended");
        let got = got.lock().unwrap();
        let particles: u32 = got
            .iter()
            .map(|b| u32::from_le_bytes(b[live::LIVE_HEADER + 8..live::LIVE_HEADER + 12].try_into().unwrap()))
            .sum();
        assert_eq!(particles, 2, "both whole frames arrived, the torn one did not");
        assert_eq!(u32::from_le_bytes(got.last().unwrap()[8..12].try_into().unwrap()), 2);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// The Runs tab's rows from run folders of every kind: OK, FAIL, CANCELLED, Interrupted,
    /// Running, unreadable, another project's (counted, not listed) and a folder that is not a
    /// run (ignored). Numbered by start time.
    #[test]
    fn runs_list_reads_every_kind_of_run_folder() {
        use simpa_core::run::{BandStats, ParticleStats, Reason, Verdict};
        let dir = scratch("list");
        let project = dir.join("room.simpa");
        let root = runs_root(&project);
        // One manifest made by the core itself (a fake solver: FAIL), then variations of it.
        let exe = fake_solver(&dir, 3);
        let r = run_folder(
            &repo("tests/fixtures/runs/spps_ok"),
            &fake_options(exe, dir.join("made")),
            &CancelToken::new(),
            &mut |_: &RunEvent| {},
        )
        .unwrap();
        let base = r.manifest.clone();
        assert_eq!(base.verdict.status, Status::Fail);
        let mine = |relative: bool| RunSource::Project {
            path: if relative {
                "room.simpa".to_string()
            } else {
                project.display().to_string().to_uppercase()
            },
            sha256: "0".repeat(64),
            variant: None,
        };
        let write = |name: &str, m: &RunManifest| {
            let d = root.join(name);
            std::fs::create_dir_all(&d).unwrap();
            std::fs::write(d.join(MANIFEST_FILE), m.to_json()).unwrap();
        };
        // 1: FAIL, the project named by a path relative to its folder.
        let mut fail = base.clone();
        fail.source = mine(true);
        write("20260101-000001-000-spps", &fail);
        // 2: OK, the path absolute in another case; one band lost 1 of 800 (0.125 %: 0.13).
        let mut ok = base.clone();
        ok.source = mine(false);
        ok.verdict = Verdict {
            status: Status::Ok,
            reasons: Vec::new(),
            warnings: Vec::new(),
        };
        let band = |freq_hz, lost| BandStats {
            freq_hz,
            absorbed_by_atmosphere: 0,
            absorbed_by_materials: 800 - lost,
            absorbed_by_fittings: 0,
            lost_by_infinite_loops: lost,
            lost_by_meshing_problems: 0,
            remaining: 0,
            total: 800,
        };
        ok.particles = Some(ParticleStats {
            bands: vec![band(500, 0), band(1000, 1), band(2000, 1)],
        });
        write("20260101-000001-000-spps-2", &ok);
        // Another project's run: counted only.
        let mut other = base.clone();
        other.source = RunSource::Project {
            path: dir.join("other.simpa").display().to_string(),
            sha256: "0".repeat(64),
            variant: None,
        };
        write("20260101-000002-000-spps", &other);
        // 3: CANCELLED.
        let mut cancelled = fail.clone();
        cancelled.verdict = Verdict {
            status: Status::Cancelled,
            reasons: vec![Reason::new("cancelled", "the run was cancelled")],
            warnings: Vec::new(),
        };
        write("20260101-000003-000-tcr", &cancelled);
        // 4: Interrupted (no run.json), 5: the active run, 6: unreadable, and not a run.
        for name in [
            "20260101-000004-000-spps",
            "20260101-000005-000-spps",
            "20260101-000006-000-tcr",
            "notes",
        ] {
            std::fs::create_dir_all(root.join(name)).unwrap();
        }
        std::fs::write(
            root.join("20260101-000006-000-tcr").join(MANIFEST_FILE),
            "{",
        )
        .unwrap();

        let view = list(&root, &project, Some("20260101-000005-000-spps")).unwrap();
        let got: Vec<(u32, &str, RunStatusUi)> = view
            .rows
            .iter()
            .map(|r| (r.number, r.run.as_str(), r.status))
            .collect();
        assert_eq!(
            got,
            [
                (1, "20260101-000001-000-spps", RunStatusUi::Fail),
                (2, "20260101-000001-000-spps-2", RunStatusUi::Ok),
                (3, "20260101-000003-000-tcr", RunStatusUi::Cancelled),
                (4, "20260101-000004-000-spps", RunStatusUi::Interrupted),
                (5, "20260101-000005-000-spps", RunStatusUi::Running),
                (6, "20260101-000006-000-tcr", RunStatusUi::Fail),
            ]
        );
        assert_eq!(view.other_projects, 1);
        let row = |n: usize| &view.rows[n];
        // FAIL: its reasons with both codes.
        assert!(!row(0).reasons.is_empty());
        assert!(row(0).reasons.iter().all(|r| !r.ui_code.is_empty()));
        // OK: the worst band's loss, with the limit, formatted in integers.
        let loss = row(1).loss.as_ref().unwrap();
        assert_eq!(
            (
                loss.worst_pct.as_str(),
                loss.worst_band_hz,
                loss.limit_pct.as_str()
            ),
            ("0.13", 1000, "1")
        );
        assert_eq!(loss.bands[0].pct, "0.00");
        assert_eq!(row(1).exe.as_ref().unwrap().sha256, base.exe.sha256);
        assert_eq!(row(1).lines.unwrap().progress, 3);
        assert!(row(1).reasons.is_empty());
        // CANCELLED, Interrupted ("no run.json"), Running, unreadable.
        assert_eq!(row(2).reasons[0].ui_code, "CANCELLED");
        assert_eq!(row(3).reasons[0].detail, "no run.json");
        assert_eq!(row(3).reasons[0].code, "results_manifest_missing");
        assert!(row(4).reasons.is_empty());
        assert!(row(5).manifest_error.is_some());
        // No active run: the fifth folder is Interrupted too.
        let idle = list(&root, &project, None).unwrap();
        assert_eq!(idle.rows[4].status, RunStatusUi::Interrupted);
        // A root that does not exist is an empty list, not an error.
        let none = list(&dir.join("nowhere"), &project, None).unwrap();
        assert!(none.rows.is_empty());
        // The results of a failed run are refused, with the code the Results step shows.
        let st = results_state(&root, "20260101-000001-000-spps").unwrap();
        assert!(!st.verified);
        assert_eq!(st.refusal.unwrap().ui_code, "RESULTS_RUN_FAILED");
        let st = results_state(&root, "20260101-000004-000-spps").unwrap();
        assert_eq!(st.refusal.unwrap().code, "results_manifest_missing");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// Cancel reaches the solver through the slot: `RunSlot::cancel` sets the run's token, the
    /// core's process layer ends the Job Object (the fake solver's `ping` grandchild with it),
    /// and the run ends CANCELLED long before the 30 s the solver would have taken, with the slot
    /// freed. No process id is looked up anywhere.
    #[test]
    fn new_open_and_import_are_refused_while_a_run_is_active() {
        let slot = Mutex::new(RunSlot::default());
        assert!(refuse_while_running(&slot, "New project").is_ok());
        slot.lock().unwrap().active = Some(ActiveRun {
            id: 1,
            run: None,
            token: CancelToken::new(),
            finished: Arc::new(AtomicBool::new(false)),
        });
        let e = refuse_while_running(&slot, "New project").unwrap_err();
        assert_eq!(e.code, RUN_ACTIVE);
        assert_eq!(e.message, "New project: a run is active: cancel it first");
    }

    #[test]
    fn cancel_ends_the_run_through_its_token() {
        let dir = scratch("cancel");
        let bat = dir.join("slow_spps.bat");
        std::fs::write(
            &bat,
            "@echo off\r\necho SPPS version 2.2.1\r\necho #1\r\nping -n 30 127.0.0.1 > nul\r\necho End of calculation.\r\n",
        )
        .unwrap();
        let batches = Arc::new(Mutex::new(Vec::<serde_json::Value>::new()));
        let got = batches.clone();
        let channel: Channel<RunStreamBatch> = Channel::new(move |body| {
            if let tauri::ipc::InvokeResponseBody::Json(text) = body {
                got.lock()
                    .unwrap()
                    .push(serde_json::from_str(&text).unwrap());
            }
            Ok(())
        });
        let slot = Arc::new(Mutex::new(RunSlot::default()));
        let finished = Arc::new(AtomicBool::new(false));
        let token = CancelToken::new();
        slot.lock().unwrap().active = Some(ActiveRun {
            id: 11,
            run: None,
            token: token.clone(),
            finished: finished.clone(),
        });
        // Cancel as run_cancel does, once the run has a folder and the solver has printed.
        let canceller = {
            let slot = slot.clone();
            std::thread::spawn(move || {
                let t0 = Instant::now();
                while t0.elapsed() < Duration::from_secs(20) {
                    if slot.lock().unwrap().active_run().is_some() {
                        std::thread::sleep(Duration::from_millis(500));
                        return slot.lock().unwrap().cancel();
                    }
                    std::thread::sleep(Duration::from_millis(20));
                }
                false
            })
        };
        let opts = fake_options(bat, dir.join("runs"));
        let fixture = repo("tests/fixtures/runs/spps_ok");
        let job = RunJob {
            id: 11,
            project: dir.join("none.simpa"),
            token,
            finished: finished.clone(),
            slot: slot.clone(),
            channel,
            live: None,
        };
        let t0 = Instant::now();
        run_thread(job, |t, on| run_folder(&fixture, &opts, t, on));
        let took = t0.elapsed();
        assert!(canceller.join().unwrap(), "a run was active to cancel");
        assert!(
            took < Duration::from_secs(15),
            "the run took {took:?}: not cancelled"
        );
        assert!(!slot.lock().unwrap().run_active());
        let batches = batches.lock().unwrap();
        let last = batches
            .iter()
            .flat_map(|b| b["events"].as_array().unwrap())
            .last()
            .unwrap()
            .clone();
        assert_eq!(last["kind"], "ended", "{last:#}");
        assert_eq!(last["row"]["status"], "CANCELLED", "{last:#}");
        assert_eq!(last["row"]["reasons"][0]["ui_code"], "CANCELLED");
        std::fs::remove_dir_all(&dir).unwrap();
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

    /// T38-5 (backlog 38): the app boundary. T38-1's run, the committed TCR run whose `run.json`
    /// has no `solvers` key, loads (C1), so it is not refused; but its solver build was never
    /// verified, so `results_state` answers `verified: false` with the reason code, never
    /// "Results verified" (C6).
    #[test]
    fn t38_5_results_state_marks_a_run_without_a_solver_record_unverified() {
        let dir = scratch("t38-5");
        let root = dir.join("runs");
        let run = "20260924-115031-155-tcr";
        copy_dir(&repo("tests/fixtures/results/seats_tcr"), &root.join(run));
        let st = results_state(&root, run).unwrap();
        assert!(
            !st.verified,
            "a run with no solver record is not verified: {st:?}"
        );
        assert_eq!(st.refusal, None, "unverified is not refused (C6): {st:?}");
        let reason = st
            .unverified
            .as_ref()
            .unwrap_or_else(|| panic!("the reason code is given: {st:?}"));
        assert_eq!(reason.code, results::build_codes::UNRECORDED, "{st:?}");
        assert_eq!(reason.ui_code, "SOLVER_BUILD_UNRECORDED", "{st:?}");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// T38-8 (backlog 38): the Runs row's data, where `runs_list` makes it. A run whose checks
    /// cover only TetGen, or that records none, is not verified on its row. The row carries the
    /// core predicate's verdict (`results::solver_build`, C5), code and detail, and agrees with
    /// what `results_state` answers for the same run, so the Runs row and the Results step cannot
    /// disagree. The run is T38-5's (the committed TCR run, whose results load), its `run.json`
    /// rewritten for each case with the checks it records and a source naming this project.
    #[test]
    fn t38_8_the_runs_row_shows_the_core_verdict_on_the_solver_build() {
        use results::build_codes::{MISMATCH, UNCHECKED, UNRECORDED};
        let dir = scratch("t38-8");
        let project = dir.join("room.simpa");
        let root = runs_root(&project);
        let run = "20260924-115031-155-tcr";
        copy_dir(&repo("tests/fixtures/results/seats_tcr"), &root.join(run));
        let path = root.join(run).join(MANIFEST_FILE);
        let mut base = RunManifest::from_json(&std::fs::read_to_string(&path).unwrap()).unwrap();
        base.source = RunSource::Project {
            path: project.display().to_string(),
            sha256: "0".repeat(64),
            variant: None,
        };
        let solver = solver_exe_name(base.solver);
        let check = |name: &str, matches: bool| SolverCheck {
            name: name.to_string(),
            path: format!(r"C:\solvers\{name}"),
            code_sha256: Some("a".repeat(64)),
            raw_sha256: Some("b".repeat(64)),
            manifest_code_sha256: Some(if matches { "a" } else { "c" }.repeat(64)),
            matches,
            detail: (!matches).then(|| "not the verified build".to_string()),
        };
        for (case, solvers, want) in [
            (
                "checks that cover only TetGen",
                Some(vec![check(TETGEN_EXE_NAME, true)]),
                Some(UNCHECKED),
            ),
            ("no solver record", None, Some(UNRECORDED)),
            ("an empty solver record", Some(Vec::new()), Some(UNRECORDED)),
            (
                "the solver's own check failing",
                Some(vec![check(solver, false), check(TETGEN_EXE_NAME, true)]),
                Some(MISMATCH),
            ),
            (
                "every check matching, the solver's among them",
                Some(vec![check(solver, true), check(TETGEN_EXE_NAME, true)]),
                None,
            ),
        ] {
            let m = RunManifest {
                solvers,
                ..base.clone()
            };
            std::fs::write(&path, m.to_json()).unwrap();
            let view = list(&root, &project, None).unwrap();
            assert_eq!(view.rows.len(), 1, "{case}: {view:?}");
            let shown = view.rows[0].solver_build.as_ref();
            // Not shown as verified unless every check matches and one is the solver's own.
            assert_eq!(
                matches!(shown, Some(SolverBuildUi::Verified)),
                want.is_none(),
                "{case}: the Runs row reads {shown:?}"
            );
            // The row's verdict is the core predicate's, code and detail.
            let core = results::solver_build(&m);
            match (shown, core.reason()) {
                (Some(SolverBuildUi::Verified), None) => {}
                (Some(SolverBuildUi::Unverified { reason }), Some(r)) => {
                    assert_eq!(
                        (reason.code.as_str(), reason.detail.as_str()),
                        (r.code.as_str(), r.detail.as_str()),
                        "{case}"
                    );
                    assert_eq!(reason.ui_code, r.code.to_ascii_uppercase(), "{case}");
                }
                (s, c) => panic!("{case}: the Runs row reads {s:?}, the core predicate {c:?}"),
            }
            assert_eq!(core.reason().map(|r| r.code.as_str()), want, "{case}");
            // The Results step's answer for the same run agrees with the row.
            let st = results_state(&root, run).unwrap();
            assert_eq!(st.refusal, None, "{case}: the results load (C1): {st:?}");
            let row_reason = match shown {
                Some(SolverBuildUi::Unverified { reason }) => Some(reason),
                _ => None,
            };
            assert_eq!(
                (st.verified, st.unverified.as_ref()),
                (want.is_none(), row_reason),
                "{case}: the Results step disagrees with the Runs row"
            );
        }
        // A row with no run.json that reads carries no verdict.
        std::fs::remove_file(&path).unwrap();
        let view = list(&root, &project, None).unwrap();
        assert_eq!(view.rows[0].status, RunStatusUi::Interrupted);
        assert_eq!(view.rows[0].solver_build, None);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
