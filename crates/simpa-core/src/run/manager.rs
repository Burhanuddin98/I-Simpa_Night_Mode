//! The run manager: one solver run, end to end, for the CLI now and the desktop shell later
//! (`docs/m5-m6-design.md`, "Layout" and "CLI surface"). It never prints: it reports each stage,
//! each TetGen line and each classified solver line to the caller's event callback, and it stops
//! when the caller's [`CancelToken`] is set.
//!
//! Two entry points, each creating a fresh run folder `<root>/<yyyyMMdd-HHmmss-fff>-<solver>[-n]/`
//! with `create_dir` (never reused; `-2`, `-3`, ... on a collision; the time is local, see
//! [`super::clock`]) and writing
//! `run.json` ([`RunManifest`]) into it however the run ends:
//! - [`run_project`]: a project file. Its geometry is checked (refused: exit class 3), the project
//!   is validated (2), a mesh is built into `<run>/mesh/` or an existing one is checked (4), the
//!   run folder's inputs are exported and checked by `validate_export` (2), SPPS's sources and
//!   point receivers are located as SPPS locates them ([`locate`], 5), and the solver runs.
//! - [`run_folder`]: a folder as it is, `tests/fixtures/runs/<case>` for instance, with no project
//!   validator: [`pre_launch`] checks its mesh and its bands instead, then, for SPPS on a mesh
//!   that passed, [`locate`] its sources and point receivers; a failure is exit class 5 without a
//!   launch.
//!
//! The solver runs in `<run>/solve/` with the one argument `config.xml` (contract Part B,
//! "Launch"), through [`crate::process`]; its streams are logged to `solver.stdout.txt` and
//! `solver.stderr.txt` beside `solve/`. After it exits the verdict is [`judge`]'s, on the
//! `config.xml` actually in `solve/`.
//!
//! The executables are found by [`ExeSearch`] (design, "Finding the executables").

use std::fmt;
use std::fs::{self, File};
use std::io::{self, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::thread;
use std::time::{Duration, SystemTime};

use roxmltree::Document;
use serde::{Deserialize, Serialize};

use super::classify::{ClassCounts, Classified, Classifier};
pub use super::clock::{folder_stamp, rfc3339};
use super::expect::{self, Expectation};
use super::locate;
use super::manifest::{
    FILE_NAME, FileCounts, FileRef, MANIFEST_VERSION, MeshRef, RunManifest, RunSource, hash_folder,
    sha256_bytes, sha256_file,
};
use super::stats::ParticleStats;
use super::verdict::{Evidence, Outputs, Reason, Status, Verdict, codes, judge};
use crate::config_xml::{self, names};
use crate::formats::{cbin, mbin};
use crate::geometry::check;
use crate::mesh::{self, MeshStatus, Mesher, TetgenMesher, verify};
use crate::process::{self, CancelToken, Line, Outcome, Spec};
use crate::schema::{Project, SolverKind};
use crate::validate::{self, Issue, Severity};

/// The solver's stdout, logged beside `solve/`.
pub const STDOUT_LOG: &str = "solver.stdout.txt";
/// The solver's stderr, logged beside `solve/`.
pub const STDERR_LOG: &str = "solver.stderr.txt";
/// The solver's working folder inside a run folder.
pub const SOLVE_DIR: &str = "solve";
/// The mesh folder inside a run folder, when the run meshes.
pub const MESH_DIR: &str = "mesh";
/// The file in a fixture folder that `run_folder` never copies: the verdict the fixture expects.
pub const FIXTURE_EXPECTATION: &str = "expected.json";
/// The placeholder a fixture's `config.xml` has for its working directory.
pub const RUNDIR_PLACEHOLDER: &str = "__RUNDIR__";
/// The one argument every solver gets (contract Part B, "Launch").
pub const SOLVER_ARGUMENT: &str = "config.xml";

/// The CLI's exit codes (`docs/m5-m6-design.md`, "Exit codes"). M7's 6 is not produced here.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(into = "u8", try_from = "u8")]
pub enum ExitClass {
    /// 0: the run is OK.
    Ok,
    /// 2: a usage or validation error.
    Usage,
    /// 3: the geometry is refused.
    Geometry,
    /// 4: meshing failed, or the mesh is unusable.
    Mesh,
    /// 5: the run is FAIL or CRASH, or `run-folder` refused it before launch.
    Solver,
    /// 130: cancelled.
    Cancelled,
}

impl ExitClass {
    /// The process exit code.
    pub const fn code(self) -> u8 {
        match self {
            ExitClass::Ok => 0,
            ExitClass::Usage => 2,
            ExitClass::Geometry => 3,
            ExitClass::Mesh => 4,
            ExitClass::Solver => 5,
            ExitClass::Cancelled => 130,
        }
    }

    /// The class a run ending in `stage` with `status` exits with.
    pub fn of(stage: Stage, status: Status) -> ExitClass {
        match (status, stage) {
            (Status::Ok, _) => ExitClass::Ok,
            (Status::Cancelled, _) => ExitClass::Cancelled,
            (_, Stage::Geometry) => ExitClass::Geometry,
            (_, Stage::Validate | Stage::Export) => ExitClass::Usage,
            (_, Stage::Mesh) => ExitClass::Mesh,
            (_, Stage::PreLaunch | Stage::Solve) => ExitClass::Solver,
        }
    }
}

impl From<ExitClass> for u8 {
    fn from(c: ExitClass) -> u8 {
        c.code()
    }
}

impl TryFrom<u8> for ExitClass {
    type Error = String;
    fn try_from(v: u8) -> Result<Self, String> {
        [
            ExitClass::Ok,
            ExitClass::Usage,
            ExitClass::Geometry,
            ExitClass::Mesh,
            ExitClass::Solver,
            ExitClass::Cancelled,
        ]
        .into_iter()
        .find(|c| c.code() == v)
        .ok_or_else(|| format!("{v} is not an exit class"))
    }
}

/// The stages of a run, in order. `run_project` reaches `pre_launch` with SPPS only; `run_folder`
/// has only `pre_launch` and `solve`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Stage {
    Geometry,
    Validate,
    Mesh,
    Export,
    PreLaunch,
    Solve,
}

/// What the run manager tells its caller while it works.
#[derive(Clone, Copy, Debug)]
pub enum RunEvent<'a> {
    /// A stage starts.
    Stage(Stage),
    /// A TetGen output line, while meshing into the run folder.
    MeshLine(&'a Line),
    /// A solver output line, classified: its class, row id, arrival index (`seq`) and time
    /// (`line.t_ms`). Lines arrive as the classifier settles them (an unclassified line one line
    /// late; see [`Classified::seq`]).
    SolverLine(&'a Classified),
}

/// Why a run could not produce a run folder with its `run.json`. Every one is exit class 2.
#[derive(Debug)]
pub enum RunError {
    /// The project file cannot be read as a project.
    Project { path: PathBuf, message: String },
    /// The fixture folder is not a run folder: missing, or without a `config.xml`.
    Fixture { path: PathBuf, message: String },
    /// A file or folder of the run could not be created, read or written.
    Io { path: PathBuf, source: io::Error },
}

impl RunError {
    pub fn exit_class(&self) -> ExitClass {
        ExitClass::Usage
    }
}

impl fmt::Display for RunError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RunError::Project { path, message } | RunError::Fixture { path, message } => {
                write!(f, "{}: {message}", path.display())
            }
            RunError::Io { path, source } => write!(f, "{}: {source}", path.display()),
        }
    }
}

impl std::error::Error for RunError {}

fn io_at(path: &Path) -> impl FnOnce(io::Error) -> RunError + '_ {
    move |source| RunError::Io {
        path: path.to_path_buf(),
        source,
    }
}

// ---------------------------------------------------------------------------------------------
// Finding the executables

/// Where an executable is looked for (`docs/m5-m6-design.md`, "Finding the executables"). The
/// first candidate that is a file is used:
/// 1. the explicit path (`--solver-exe`, `--tetgen`); when given, it is the only candidate;
/// 2. `$SIMPA_SOLVERS_DIR/<name>`;
/// 3. `<simpa.exe dir>/solvers/<name>`;
/// 4. `<simpa.exe dir>/<name>`;
/// 5. `<ancestor>/target/solvers/bin/<name>` for the nearest ancestor of `simpa.exe`'s folder that
///    has one: the dev tree.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ExeSearch {
    pub explicit: Option<PathBuf>,
    pub solvers_dir: Option<PathBuf>,
    pub exe_dir: Option<PathBuf>,
}

/// No candidate was a file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExeNotFound {
    pub name: String,
    pub tried: Vec<PathBuf>,
}

impl fmt::Display for ExeNotFound {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} not found; tried", self.name)?;
        for p in &self.tried {
            write!(f, " {}", p.display())?;
        }
        write!(
            f,
            " (give its path, or set SIMPA_SOLVERS_DIR to the folder of the solver build)"
        )
    }
}

impl std::error::Error for ExeNotFound {}

impl ExeSearch {
    /// The search for this process: `explicit`, `$SIMPA_SOLVERS_DIR` (when set and not empty),
    /// and the folder of the running executable.
    pub fn from_env(explicit: Option<PathBuf>) -> Self {
        ExeSearch {
            explicit,
            solvers_dir: std::env::var_os("SIMPA_SOLVERS_DIR")
                .filter(|v| !v.is_empty())
                .map(PathBuf::from),
            exe_dir: std::env::current_exe()
                .ok()
                .and_then(|p| p.parent().map(Path::to_path_buf)),
        }
    }

    /// Every candidate for `name`, in order.
    pub fn candidates(&self, name: &str) -> Vec<PathBuf> {
        if let Some(p) = &self.explicit {
            return vec![p.clone()];
        }
        let mut out = Vec::new();
        if let Some(d) = &self.solvers_dir {
            out.push(d.join(name));
        }
        if let Some(d) = &self.exe_dir {
            out.push(d.join("solvers").join(name));
            out.push(d.join(name));
            let dev = d
                .ancestors()
                .map(|a| a.join("target").join("solvers").join("bin").join(name))
                .find(|p| p.is_file());
            out.extend(dev);
        }
        out
    }

    /// The first candidate that is a file, made absolute.
    pub fn find(&self, name: &str) -> Result<PathBuf, ExeNotFound> {
        let tried = self.candidates(name);
        match tried.iter().find(|p| p.is_file()) {
            Some(p) => Ok(std::path::absolute(p).unwrap_or_else(|_| p.clone())),
            None => Err(ExeNotFound {
                name: name.to_string(),
                tried,
            }),
        }
    }
}

/// The executable file name of a solver.
pub fn solver_exe_name(solver: SolverKind) -> &'static str {
    match solver {
        SolverKind::Spps => "spps.exe",
        SolverKind::Tcr => "classicalTheory.exe",
    }
}

/// TetGen's executable file name.
pub const TETGEN_EXE_NAME: &str = "tetgen.exe";

fn solver_name(solver: SolverKind) -> &'static str {
    match solver {
        SolverKind::Spps => "spps",
        SolverKind::Tcr => "tcr",
    }
}

// ---------------------------------------------------------------------------------------------
// Cancelling after a delay

/// Cancels a token after a delay, unless it is dropped first. Dropping it waits for its thread,
/// so a timer never outlives the call it timed.
#[derive(Debug)]
pub struct CancelTimer {
    stop: Option<mpsc::Sender<()>>,
    thread: Option<thread::JoinHandle<()>>,
}

impl CancelTimer {
    pub fn start(after: Duration, cancel: CancelToken) -> Self {
        let (stop, wait) = mpsc::channel::<()>();
        let thread = thread::spawn(move || {
            if let Err(RecvTimeoutError::Timeout) = wait.recv_timeout(after) {
                cancel.cancel();
            }
        });
        CancelTimer {
            stop: Some(stop),
            thread: Some(thread),
        }
    }
}

impl Drop for CancelTimer {
    fn drop(&mut self) {
        drop(self.stop.take());
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

/// A [`Mesher`] that cancels its token `after` each call starts: `simpa mesh --cancel-after-ms`
/// times the cancel from TetGen's launch, not from the start of the command, so it lands on a
/// running TetGen (gate M5(g)).
pub struct CancelAfterLaunch<'a> {
    pub inner: &'a dyn Mesher,
    pub after: Duration,
}

impl Mesher for CancelAfterLaunch<'_> {
    fn program(&self) -> Option<&Path> {
        self.inner.program()
    }

    fn run(
        &self,
        dir: &Path,
        args: &[String],
        cancel: &CancelToken,
        on_line: &mut dyn FnMut(&Line),
    ) -> io::Result<Outcome> {
        let _timer = CancelTimer::start(self.after, cancel.clone());
        self.inner.run(dir, args, cancel, on_line)
    }
}

// ---------------------------------------------------------------------------------------------
// Run folders and times

/// Creates `<root>/<stamp>-<solver>` with `create_dir`, or `-2`, `-3`, ... after it when the name
/// is taken: a run folder is never reused. `root` is created when missing.
pub fn create_run_folder(root: &Path, solver: SolverKind, at: SystemTime) -> io::Result<PathBuf> {
    fs::create_dir_all(root)?;
    let base = format!("{}-{}", folder_stamp(at), solver_name(solver));
    for n in 1u32.. {
        let name = if n == 1 {
            base.clone()
        } else {
            format!("{base}-{n}")
        };
        let dir = root.join(name);
        match fs::create_dir(&dir) {
            Ok(()) => return std::path::absolute(&dir),
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e),
        }
    }
    unreachable!("u32 run-folder suffixes exhausted")
}

// ---------------------------------------------------------------------------------------------
// Options and results

/// How a run is launched and judged.
#[derive(Clone, Debug)]
pub struct RunOptions {
    pub solver: SolverKind,
    /// The solver executable, found with [`ExeSearch`].
    pub solver_exe: PathBuf,
    /// The folder the run folder is created in.
    pub runs_root: PathBuf,
    /// The verdict's `particle_loss_excess` limit (`DEFAULT_LOSS_LIMIT`; the CLI refuses NaN and
    /// negative values).
    pub loss_limit: f64,
    /// Cancel the solver this long after its launch.
    pub cancel_after_ms: Option<u64>,
    /// Cancel the solver at its first progress line at or above this percentage. Real SPPS
    /// output ends at `#99.99`, never 100.
    pub cancel_after_progress: Option<f64>,
}

/// Where `run_project`'s mesh comes from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MeshChoice {
    /// Mesh the project into `<run>/mesh/` with this `tetgen.exe`.
    Build { tetgen: PathBuf },
    /// Use this mesh folder, after checking its `mesh.json` ([`check_mesh_dir`]).
    Reuse(PathBuf),
}

/// A finished run: its folder, and the manifest written there as `run.json`.
#[derive(Clone, Debug)]
pub struct RunReport {
    pub dir: PathBuf,
    pub manifest: RunManifest,
}

impl RunReport {
    pub fn exit_class(&self) -> ExitClass {
        self.manifest.exit_class
    }
}

/// What a run records until it ends, refused or launched.
struct Record<'a> {
    opts: &'a RunOptions,
    dir: PathBuf,
    solve: PathBuf,
    source: RunSource,
    exe: FileRef,
    started: String,
    inputs: Vec<FileRef>,
    mesh: Option<MeshRef>,
    /// Warnings found before launch (the validator's), recorded first in the verdict.
    warnings: Vec<Reason>,
}

/// What a launched run produced.
struct Launched {
    outcome: Option<Outcome>,
    verdict: Verdict,
    lines: ClassCounts,
    files: FileCounts,
    particles: Option<ParticleStats>,
}

impl Record<'_> {
    fn manifest(self, stage: Stage, launched: Launched) -> Result<RunReport, RunError> {
        let mut verdict = launched.verdict;
        let mut warnings = self.warnings;
        warnings.append(&mut verdict.warnings);
        verdict.warnings = warnings;
        let manifest = RunManifest {
            manifest_version: MANIFEST_VERSION,
            core_version: crate::VERSION.to_string(),
            solver_commit: crate::SOLVER_COMMIT.to_string(),
            source: self.source,
            solver: self.opts.solver,
            exe: self.exe,
            argv: vec![SOLVER_ARGUMENT.to_string()],
            cwd: self.solve.display().to_string(),
            started: self.started,
            inputs: self.inputs,
            mesh: self.mesh,
            stage,
            exit_class: ExitClass::of(stage, verdict.status),
            outcome: launched.outcome,
            lines: launched.lines,
            files: launched.files,
            particles: launched.particles,
            loss_limit: self.opts.loss_limit,
            verdict,
        };
        let path = self.dir.join(FILE_NAME);
        fs::write(&path, manifest.to_json()).map_err(io_at(&path))?;
        Ok(RunReport {
            dir: self.dir,
            manifest,
        })
    }

    /// Ends the run before launch, in `stage`, with `reasons` (status FAIL), or CANCELLED when
    /// `cancelled`.
    fn refuse(
        self,
        stage: Stage,
        cancelled: bool,
        reasons: Vec<Reason>,
    ) -> Result<RunReport, RunError> {
        let status = if cancelled {
            Status::Cancelled
        } else {
            Status::Fail
        };
        let verdict = Verdict {
            status,
            reasons,
            warnings: Vec::new(),
        };
        self.manifest(
            stage,
            Launched {
                outcome: None,
                verdict,
                lines: ClassCounts::default(),
                files: FileCounts::default(),
                particles: None,
            },
        )
    }

    /// Ends the run as cancelled before launch, in `stage`.
    fn cancelled(self, stage: Stage) -> Result<RunReport, RunError> {
        self.refuse(
            stage,
            true,
            vec![Reason::new(
                codes::CANCELLED,
                "the run was cancelled before the solver was launched",
            )],
        )
    }

    fn hash_inputs(&mut self) -> Result<(), RunError> {
        self.inputs = hash_folder(&self.solve).map_err(io_at(&self.solve))?;
        Ok(())
    }
}

/// A validator issue as a reason: its code, and where and what in words.
fn issue_reason(i: &Issue) -> Reason {
    Reason::new(i.code, format!("{}: {}", i.path, i.message))
}

// ---------------------------------------------------------------------------------------------
// run_project

/// Runs the project in `project_file` with the solver `opts` names, `variant`'s materials
/// (`None`: the project's own) and the mesh `mesh` says, into a fresh run folder under
/// `opts.runs_root`. `Err` only when no run folder with its `run.json` could be made; every
/// other ending, refusals included, is in the report.
pub fn run_project(
    project_file: &Path,
    variant: Option<&str>,
    mesh: &MeshChoice,
    opts: &RunOptions,
    cancel: &CancelToken,
    on_event: &mut dyn FnMut(&RunEvent),
) -> Result<RunReport, RunError> {
    let bytes = fs::read(project_file).map_err(|e| RunError::Project {
        path: project_file.to_path_buf(),
        message: e.to_string(),
    })?;
    let project = std::str::from_utf8(&bytes)
        .map_err(|e| e.to_string())
        .and_then(|t| validate::read_project_text(t).map_err(|e| format!("{} ({e})", e.code())))
        .map_err(|message| RunError::Project {
            path: project_file.to_path_buf(),
            message,
        })?;
    let exe = exe_ref(&opts.solver_exe)?;
    let started = SystemTime::now();
    let dir =
        create_run_folder(&opts.runs_root, opts.solver, started).map_err(io_at(&opts.runs_root))?;
    let mut rec = Record {
        opts,
        solve: dir.join(SOLVE_DIR),
        dir,
        source: RunSource::Project {
            path: project_file.display().to_string(),
            sha256: sha256_bytes(&bytes),
            variant: variant.map(str::to_string),
        },
        exe,
        started: rfc3339(started),
        inputs: Vec::new(),
        mesh: None,
        warnings: Vec::new(),
    };

    // Geometry: refused is exit class 3.
    on_event(&RunEvent::Stage(Stage::Geometry));
    let report = check::check(&project.geometry);
    if report.verdict != check::Verdict::Ok {
        let detail = report
            .reasons
            .iter()
            .map(|r| format!("{} ({}): {}", r.code.as_str(), r.count, r.message))
            .collect::<Vec<_>>()
            .join("; ");
        let reason = Reason::new(codes::GEOMETRY_REFUSED, detail);
        return rec.refuse(Stage::Geometry, false, vec![reason]);
    }
    if cancel.is_cancelled() {
        return rec.cancelled(Stage::Geometry);
    }

    // The project rules: any error is exit class 2. The mesh stamp is not given here: a mesh
    // folder's staleness is the mesh stage's `mesh_out_of_date`, exit class 4.
    on_event(&RunEvent::Stage(Stage::Validate));
    let issues =
        validate::validate_with(&project, &validate::Context::for_project_file(project_file));
    rec.warnings.extend(
        issues
            .iter()
            .filter(|i| i.severity == Severity::Warning)
            .map(issue_reason),
    );
    if validate::has_errors(&issues) {
        let errors = issues
            .iter()
            .filter(|i| i.severity == Severity::Error)
            .map(issue_reason);
        return rec.refuse(Stage::Validate, false, dedup_codes(errors));
    }
    if cancel.is_cancelled() {
        return rec.cancelled(Stage::Validate);
    }

    // The mesh: built into <run>/mesh/, or an existing folder checked. Failure is exit class 4.
    on_event(&RunEvent::Stage(Stage::Mesh));
    let mesh_dir = match mesh {
        MeshChoice::Reuse(d) => {
            match check_mesh_dir(d, &validate::mesh_input_hash(&project)) {
                Ok(r) => rec.mesh = Some(r),
                Err(reasons) => return rec.refuse(Stage::Mesh, false, reasons),
            }
            d.clone()
        }
        MeshChoice::Build { tetgen } => {
            let d = rec.dir.join(MESH_DIR);
            let mesher = TetgenMesher::new(tetgen);
            let built = mesh::mesh_project(&project, &d, &mesher, cancel, &mut |l: &Line| {
                on_event(&RunEvent::MeshLine(l))
            });
            let m = match built {
                Ok(m) => m,
                Err(e) => {
                    let reason = Reason::new(
                        codes::MESH_MISSING,
                        format!("the mesh folder could not be used: {e}"),
                    );
                    return rec.refuse(Stage::Mesh, false, vec![reason]);
                }
            };
            if !m.is_ok() {
                let reasons = mesh_reasons(&m, &d);
                return rec.refuse(Stage::Mesh, m.status == MeshStatus::Cancelled, reasons);
            }
            rec.mesh = Some(MeshRef {
                manifest: Some(d.join(mesh::MANIFEST_FILE).display().to_string()),
                mesh_input_hash: m.mesh_input_hash.clone(),
                mbin_sha256: m.files.mbin.clone().unwrap_or_default(),
            });
            d
        }
    };
    if cancel.is_cancelled() {
        return rec.cancelled(Stage::Mesh);
    }

    // The run folder's inputs, then the export rules: a failure is exit class 2.
    on_event(&RunEvent::Stage(Stage::Export));
    if let Err(reason) = export(
        &project,
        project_file,
        variant,
        &mesh_dir,
        &rec.solve,
        opts.solver,
    ) {
        return rec.refuse(Stage::Export, false, vec![reason]);
    }
    if let Err(e) = rec.hash_inputs() {
        let reason = Reason::new(
            codes::EXPORT_FAILED,
            format!("reading back the inputs: {e}"),
        );
        return rec.refuse(Stage::Export, false, vec![reason]);
    }
    let issues = validate::validate_export(&project, &rec.solve, opts.solver);
    rec.warnings.extend(
        issues
            .iter()
            .filter(|i| i.severity == Severity::Warning)
            .map(issue_reason),
    );
    if validate::has_errors(&issues) {
        let errors = issues
            .iter()
            .filter(|i| i.severity == Severity::Error)
            .map(issue_reason);
        return rec.refuse(Stage::Export, false, dedup_codes(errors));
    }
    if cancel.is_cancelled() {
        return rec.cancelled(Stage::Export);
    }

    // SPPS: every source and point receiver must be in a tetrahedron by SPPS's own test, or the
    // run would crash or read a receiver as silence. Exit class 5, as for `run_folder`.
    if opts.solver == SolverKind::Spps {
        on_event(&RunEvent::Stage(Stage::PreLaunch));
        let reasons = locate::check_folder(&rec.solve);
        if !reasons.is_empty() {
            return rec.refuse(Stage::PreLaunch, false, reasons);
        }
        if cancel.is_cancelled() {
            return rec.cancelled(Stage::PreLaunch);
        }
    }

    on_event(&RunEvent::Stage(Stage::Solve));
    let launched = launch(&rec, cancel, on_event);
    rec.manifest(Stage::Solve, launched)
}

/// At most one reason per code, the first of each, in order; how many more there were is added
/// to its detail.
fn dedup_codes(reasons: impl Iterator<Item = Reason>) -> Vec<Reason> {
    let mut out: Vec<(Reason, usize)> = Vec::new();
    for r in reasons {
        match out.iter_mut().find(|(o, _)| o.code == r.code) {
            Some((_, n)) => *n += 1,
            None => out.push((r, 0)),
        }
    }
    out.into_iter()
        .map(|(mut r, more)| {
            if more > 0 {
                r.detail.push_str(&format!(" (and {more} more)"));
            }
            r
        })
        .collect()
}

/// The reasons of a mesh manifest that is not OK: each code, the first with every message.
fn mesh_reasons(m: &mesh::MeshManifest, dir: &Path) -> Vec<Reason> {
    let manifest = dir.join(mesh::MANIFEST_FILE);
    m.codes
        .iter()
        .enumerate()
        .map(|(i, c)| {
            let detail = if i == 0 {
                format!("{} ({})", m.messages.join("; "), manifest.display())
            } else {
                format!("see {}", manifest.display())
            };
            Reason::new(c, detail)
        })
        .collect()
}

/// The solver executable's path and sha256.
fn exe_ref(exe: &Path) -> Result<FileRef, RunError> {
    let path = std::path::absolute(exe).map_err(io_at(exe))?;
    let sha256 = sha256_file(&path).map_err(io_at(&path))?;
    Ok(FileRef {
        path: path.display().to_string(),
        sha256,
    })
}

/// Checks a mesh folder for `run --mesh` against the project's mesh stamp `expected_hash`
/// (`validate::mesh_input_hash`), and returns what the run records of it. Refused:
/// - `mesh_missing`: no readable `mesh.json`, a manifest that is not `OK`, or no
///   `tetramesh.mbin`;
/// - `manifest_mismatch`: the manifest's `files.mbin` is not the `.mbin`'s sha256 (a partial or
///   foreign file);
/// - `mesh_out_of_date`: the manifest's `mesh_input_hash` is not the project's.
pub fn check_mesh_dir(dir: &Path, expected_hash: &str) -> Result<MeshRef, Vec<Reason>> {
    let manifest_path = dir.join(mesh::MANIFEST_FILE);
    let missing = |why: String| vec![Reason::new(codes::MESH_MISSING, why)];
    let m = mesh::read_manifest(dir).map_err(|e| {
        missing(format!(
            "{}: {e}; mesh the project into this folder first",
            manifest_path.display()
        ))
    })?;
    if !m.is_ok() {
        return Err(missing(format!(
            "{} has status {:?} ({}): only an OK mesh comes with a usable .mbin",
            manifest_path.display(),
            m.status,
            m.codes.join(", ")
        )));
    }
    let mbin_path = dir.join(names::TETRA_MESH);
    let sha = match sha256_file(&mbin_path) {
        Ok(s) => s,
        Err(e) => return Err(missing(format!("{}: {e}", mbin_path.display()))),
    };
    let mut reasons = Vec::new();
    if !m
        .files
        .mbin
        .as_deref()
        .is_some_and(|h| h.eq_ignore_ascii_case(&sha))
    {
        reasons.push(Reason::new(
            "manifest_mismatch",
            format!(
                "{} records the .mbin as {:?}, but {} hashes to {sha}: a partial or foreign file",
                manifest_path.display(),
                m.files.mbin,
                mbin_path.display()
            ),
        ));
    }
    if m.mesh_input_hash.as_deref() != Some(expected_hash) {
        reasons.push(Reason::new(
            validate::codes::MESH_OUT_OF_DATE,
            format!(
                "the mesh was built from inputs stamped {:?}, the project's are stamped \
                 {expected_hash}: re-mesh it",
                m.mesh_input_hash
            ),
        ));
    }
    if !reasons.is_empty() {
        return Err(reasons);
    }
    Ok(MeshRef {
        manifest: Some(manifest_path.display().to_string()),
        mesh_input_hash: m.mesh_input_hash,
        mbin_sha256: sha,
    })
}

/// Writes the run folder's inputs into the new folder `solve`: `config.xml` for `solver` and
/// `variant`, the scene mesh exported with the current materials, `mesh_dir`'s `tetramesh.mbin`,
/// and the directivity files.
fn export(
    project: &Project,
    project_file: &Path,
    variant: Option<&str>,
    mesh_dir: &Path,
    solve: &Path,
    solver: SolverKind,
) -> Result<(), Reason> {
    let failed = |what: String| Reason::new(codes::EXPORT_FAILED, what);
    fs::create_dir(solve).map_err(|e| failed(format!("{}: {e}", solve.display())))?;
    config_xml::write_file(project, solver, variant, solve)
        .map_err(|e| failed(format!("{}: {e}", e.code())))?;
    let scene =
        config_xml::scene_mesh(project).map_err(|e| failed(format!("{}: {e}", e.code())))?;
    let cbin_path = solve.join(names::SCENE_MESH);
    cbin::write_file(&scene, &cbin_path)
        .map_err(|e| failed(format!("{}: {e}", cbin_path.display())))?;
    let from = mesh_dir.join(names::TETRA_MESH);
    fs::copy(&from, solve.join(names::TETRA_MESH))
        .map_err(|e| failed(format!("{}: {e}", from.display())))?;
    let staged = config_xml::directivity_files(project);
    if !staged.is_empty() {
        let d = solve.join(names::DIRECTIVITY_DIR);
        fs::create_dir(&d).map_err(|e| failed(format!("{}: {e}", d.display())))?;
        let base = project_file.parent().unwrap_or(Path::new(""));
        for f in staged {
            let from = base.join(&f.project_path);
            fs::copy(&from, d.join(&f.run_name))
                .map_err(|e| failed(format!("{}: {e}", from.display())))?;
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------------------------
// run_folder

/// Runs the folder `fixture` as it is, with the solver `opts` names: it is copied into a fresh
/// `<run>/solve/` without its `expected.json`, `__RUNDIR__` in its `config.xml` becomes the
/// absolute `solve\` path, and [`pre_launch`] must pass before the solver starts, and for SPPS,
/// once its mesh has passed, [`locate::check_folder`] too. No project validator runs. `Err` when
/// `fixture` is not a folder with a `config.xml`, or no run folder with its `run.json` could be
/// made.
pub fn run_folder(
    fixture: &Path,
    opts: &RunOptions,
    cancel: &CancelToken,
    on_event: &mut dyn FnMut(&RunEvent),
) -> Result<RunReport, RunError> {
    if !fixture.join(names::CONFIG).is_file() {
        return Err(RunError::Fixture {
            path: fixture.to_path_buf(),
            message: format!("not a run folder: it has no {}", names::CONFIG),
        });
    }
    let exe = exe_ref(&opts.solver_exe)?;
    let started = SystemTime::now();
    let dir =
        create_run_folder(&opts.runs_root, opts.solver, started).map_err(io_at(&opts.runs_root))?;
    let solve = dir.join(SOLVE_DIR);
    let mut rec = Record {
        opts,
        solve: solve.clone(),
        dir,
        source: RunSource::Fixture {
            path: fixture.display().to_string(),
        },
        exe,
        started: rfc3339(started),
        inputs: Vec::new(),
        mesh: None,
        warnings: Vec::new(),
    };
    copy_fixture(fixture, &solve)?;
    let config = solve.join(names::CONFIG);
    let text = fs::read(&config).map_err(io_at(&config))?;
    let wd = config_xml::working_directory(&solve).map_err(|e| RunError::Io {
        path: solve.clone(),
        source: io::Error::other(e.to_string()),
    })?;
    let text = String::from_utf8_lossy(&text).replace(RUNDIR_PLACEHOLDER, &wd);
    fs::write(&config, text).map_err(io_at(&config))?;
    rec.hash_inputs()?;

    on_event(&RunEvent::Stage(Stage::PreLaunch));
    let mut checked = pre_launch(&solve);
    // Located only in a mesh mesh::verify passed: in a broken one every point is a consequence.
    if opts.solver == SolverKind::Spps && checked.verify.as_ref().is_some_and(|v| v.passed()) {
        checked.reasons.extend(locate::check_folder(&solve));
    }
    rec.mesh = checked.mesh;
    if !checked.reasons.is_empty() {
        return rec.refuse(Stage::PreLaunch, false, checked.reasons);
    }
    if cancel.is_cancelled() {
        return rec.cancelled(Stage::PreLaunch);
    }
    on_event(&RunEvent::Stage(Stage::Solve));
    let launched = launch(&rec, cancel, on_event);
    rec.manifest(Stage::Solve, launched)
}

/// Copies `from` into the new folder `to`, recursively, leaving out the top-level
/// `expected.json`.
fn copy_fixture(from: &Path, to: &Path) -> Result<(), RunError> {
    fn copy_dir(from: &Path, to: &Path, top: bool) -> Result<(), RunError> {
        fs::create_dir(to).map_err(io_at(to))?;
        for entry in fs::read_dir(from).map_err(io_at(from))? {
            let entry = entry.map_err(io_at(from))?;
            let path = entry.path();
            let name = entry.file_name();
            if top && name.eq_ignore_ascii_case(FIXTURE_EXPECTATION) {
                continue;
            }
            let kind = entry.file_type().map_err(io_at(&path))?;
            if kind.is_dir() {
                copy_dir(&path, &to.join(&name), false)?;
            } else {
                fs::copy(&path, to.join(&name)).map_err(io_at(&path))?;
            }
        }
        Ok(())
    }
    copy_dir(from, to, true)
}

/// What [`pre_launch`] found.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PreLaunch {
    /// Why the folder must not be launched; empty when it may be.
    pub reasons: Vec<Reason>,
    /// The mesh as the run records it, when the `.mbin` could be read.
    pub mesh: Option<MeshRef>,
    /// The mesh check's report, when it ran.
    pub verify: Option<verify::VerifyReport>,
}

/// The checks `run_folder` makes on a working folder before launch, in place of the project
/// validator (`docs/m5-m6-design.md`, "Run folder", and decision 11):
/// - **the mesh:** the `.mbin` `tetrameshFileName` names must exist and read, the `.cbin`
///   `modelName` names must read, and [`verify::verify_mesh`] must pass them, with the fittings
///   the config's `encombrement` ids and the room's first id as `volume_ids` chooses it. Anything
///   else is `mesh_invalid`, followed by the verifier's codes;
/// - **the bands:** every source's spectrum must reach every computed band. Spectra are mapped
///   to bands by position (Part A, `band_set_mismatch`), so a source with fewer entries than a
///   computed band's position is read past its end, and nothing after the run shows it.
///
/// A `config.xml` that does not parse is `config_attribute_missing`, and nothing else is checked.
pub fn pre_launch(solve: &Path) -> PreLaunch {
    let mut out = PreLaunch::default();
    let config = solve.join(names::CONFIG);
    let text = match fs::read(&config) {
        Ok(b) => {
            String::from_utf8_lossy(b.strip_prefix(b"\xef\xbb\xbf").unwrap_or(&b)).into_owned()
        }
        Err(e) => {
            out.reasons.push(Reason::new(
                codes::CONFIG_ATTRIBUTE_MISSING,
                format!("{}: {e}", config.display()),
            ));
            return out;
        }
    };
    let doc = match Document::parse(&text) {
        Ok(d) => d,
        Err(e) => {
            out.reasons.push(Reason::new(
                codes::CONFIG_ATTRIBUTE_MISSING,
                format!("{} is not well-formed XML: {e}", config.display()),
            ));
            return out;
        }
    };
    mesh_check(solve, &doc, &mut out);
    out.reasons.extend(band_check(&doc));
    out
}

/// `pre_launch`'s mesh check.
fn mesh_check(solve: &Path, doc: &Document, out: &mut PreLaunch) {
    let invalid = |why: String| Reason::new(mesh::codes::MESH_INVALID, why);
    let root = doc.root_element();
    let sim = expect::child(root, "simulation");
    let attr = |name: &str| sim.and_then(|s| s.attribute(name)).unwrap_or("");
    let (mbin_name, cbin_name) = (attr("tetrameshFileName"), attr("modelName"));
    let mbin_path = solve.join(mbin_name);
    if mbin_name.is_empty() || !mbin_path.is_file() {
        out.reasons.push(invalid(format!(
            "the tetrahedral mesh '{mbin_name}' (tetrameshFileName) is not in the folder: the \
             solver would print tetra_mesh_unreadable, and SPPS exit 0"
        )));
        return;
    }
    let bytes = match crate::formats::read_file(&mbin_path) {
        Ok(b) => b,
        Err(e) => {
            return out
                .reasons
                .push(invalid(format!("{}: {e}", mbin_path.display())));
        }
    };
    out.mesh = Some(MeshRef {
        manifest: None,
        mesh_input_hash: None,
        mbin_sha256: sha256_bytes(&bytes),
    });
    let mesh = match mbin::read(&bytes) {
        Ok(m) => m,
        Err(e) => {
            return out
                .reasons
                .push(invalid(format!("{}: {e}", mbin_path.display())));
        }
    };
    let cbin_path = solve.join(cbin_name);
    let scene = match cbin::read_file(&cbin_path) {
        Ok(s) if !cbin_name.is_empty() => s,
        Ok(_) => {
            return out.reasons.push(invalid(
                "modelName is empty: no scene mesh to check against".into(),
            ));
        }
        Err(e) => {
            return out.reasons.push(invalid(format!(
                "the scene mesh {} (modelName) cannot be read: {e}",
                cbin_path.display()
            )));
        }
    };
    let fittings: Vec<i32> = expect::child(root, "encombrement_enum")
        .map(|n| {
            expect::items(n)
                .map(|f| expect::atoi(f.attribute("id").unwrap_or("")))
                .collect()
        })
        .unwrap_or_default();
    let ids = volume_ids(&mesh, fittings);
    let report = verify::verify_mesh(&mesh, &scene, &ids);
    if !report.passed() {
        let counts = serde_json::to_value(&report).unwrap_or_default();
        out.reasons.push(invalid(format!(
            "{} fails mesh::verify with the room from id {}: {}",
            mbin_path.display(),
            ids.room,
            report.codes.join(", ")
        )));
        for c in &report.codes {
            let n = counts.get(c.as_str()).cloned().unwrap_or_default();
            out.reasons
                .push(Reason::new(c, format!("{n}, counted by mesh::verify")));
        }
    }
    out.verify = Some(report);
}

/// The volume ids a run folder's `.mbin` is checked with, with no project to say how it was
/// meshed:
/// - with fitting zones declared (`encombrement` ids), TetGen's numbering for them
///   ([`verify::VolumeIds::tetgen`]): the room's parts above the largest fitting id, as upstream's
///   GUI and this crate's mesher both write them (tutorial 3: fittings 1930 and 2083, room 2084
///   to 2086), so an id below that is no fitting's is `unknown_volume_ids`;
/// - with none, the room starts at the smallest `idVolume` in the mesh: TetGen's 1, and also the
///   0 some upstream meshes carry (its Python-binding test mesh, and Night Mode's broken hall),
///   which the solver reads as the room too (`coreTypes.h:445`); 1 for an empty mesh.
fn volume_ids(mesh: &mbin::Mesh, fittings: Vec<i32>) -> verify::VolumeIds {
    if !fittings.is_empty() {
        return verify::VolumeIds::tetgen(fittings);
    }
    let room = mesh
        .tetrahedra
        .iter()
        .map(|t| t.id_volume)
        .min()
        .unwrap_or(1);
    verify::VolumeIds { room, fittings }
}

/// `pre_launch`'s band check: each source's spectrum reaches every computed band's position.
fn band_check(doc: &Document) -> Option<Reason> {
    let root = doc.root_element();
    let mut bands: Vec<(i32, bool)> = expect::child(root, "simulation")
        .and_then(|s| expect::child(s, "freq_enum"))
        .map(|f| {
            expect::items(f)
                .map(|b| {
                    (
                        expect::atoi(b.attribute("freq").unwrap_or("")),
                        b.attribute("docalc") == Some("1"),
                    )
                })
                .collect()
        })
        .unwrap_or_default();
    // The solver orders the bands by frequency; a spectrum's entry k belongs to band k.
    bands.sort_by_key(|b| b.0);
    let last = bands.iter().rposition(|b| b.1)?;
    let short: Vec<String> = expect::child(root, "sources")
        .map(|s| {
            expect::items(s)
                .enumerate()
                .filter_map(|(i, src)| {
                    let n = expect::items(src).count();
                    (n <= last).then(|| {
                        format!(
                            "source {} '{}' has {n} band entries, but the computed {} Hz band \
                             is entry {}",
                            i + 1,
                            src.attribute("name").unwrap_or(""),
                            bands[last].0,
                            last + 1
                        )
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    (!short.is_empty()).then(|| {
        Reason::new(
            validate::codes::BAND_SET_MISMATCH,
            format!(
                "{}: spectra are mapped to bands by position, so the solver reads past the end \
                 of the spectrum, silently",
                short.join("; ")
            ),
        )
    })
}

// ---------------------------------------------------------------------------------------------
// The launch

/// Where each settled line goes: the caller, the progress cancel, the verdict.
struct Sink<'a> {
    on_event: &'a mut dyn FnMut(&RunEvent),
    lines: Vec<Classified>,
    cancel: &'a CancelToken,
    cancel_at: Option<f64>,
}

impl Sink<'_> {
    fn take(&mut self, c: Classified) {
        (self.on_event)(&RunEvent::SolverLine(&c));
        if let (Some(at), Some(p)) = (self.cancel_at, c.progress)
            && p >= at
        {
            self.cancel.cancel();
        }
        self.lines.push(c);
    }
}

/// The solver's two logs beside `solve/`, written line by line as the lines arrive. A failed
/// write does not stop the run: the lines are classified in memory all the same, so the verdict
/// stands. The first failure is kept and becomes the verdict's `log_write_failed` warning, and
/// `run.json` is written as for any launched run.
struct Logs<W: Write> {
    out: (W, PathBuf),
    err: (W, PathBuf),
    failed: Option<(PathBuf, io::Error)>,
}

impl Logs<BufWriter<File>> {
    /// Creates `solver.stdout.txt` and `solver.stderr.txt` in `dir`; `launch_failed` when either
    /// cannot be created, since the run would then have no record of what the solver printed.
    fn create(dir: &Path) -> Result<Self, Reason> {
        let open = |name: &str| {
            let path = dir.join(name);
            match File::create(&path) {
                Ok(f) => Ok((BufWriter::new(f), path)),
                Err(e) => Err(Reason::new(
                    codes::LAUNCH_FAILED,
                    format!(
                        "the solver's log {} cannot be created ({e}); the solver was not launched",
                        path.display()
                    ),
                )),
            }
        };
        Ok(Logs {
            out: open(STDOUT_LOG)?,
            err: open(STDERR_LOG)?,
            failed: None,
        })
    }
}

impl<W: Write> Logs<W> {
    /// Appends `l` to its stream's log, with its newline when it had one.
    fn line(&mut self, l: &Line) {
        let (w, path) = match l.stream {
            process::Stream::Stdout => (&mut self.out.0, &self.out.1),
            process::Stream::Stderr => (&mut self.err.0, &self.err.1),
        };
        let eol = if l.terminated { "\n" } else { "" };
        if let Err(e) = write!(w, "{}{eol}", l.text)
            && self.failed.is_none()
        {
            self.failed = Some((path.clone(), e));
        }
    }

    /// Flushes both logs. The first write or flush failure, as the `log_write_failed` warning.
    fn finish(self) -> Option<Reason> {
        let Logs {
            mut out,
            mut err,
            mut failed,
        } = self;
        for (w, path) in [&mut out, &mut err] {
            if let Err(e) = w.flush()
                && failed.is_none()
            {
                failed = Some((path.clone(), e));
            }
        }
        failed.map(|(path, e)| {
            Reason::new(
                codes::LOG_WRITE_FAILED,
                format!(
                    "{}: {e}; the log is incomplete, the lines were classified as they arrived",
                    path.display()
                ),
            )
        })
    }
}

/// A run whose solver never started, for `reason` (`launch_failed`).
fn not_launched(reason: Reason, lines: ClassCounts) -> Launched {
    Launched {
        outcome: None,
        verdict: Verdict {
            status: Status::Fail,
            reasons: vec![reason],
            warnings: Vec::new(),
        },
        lines,
        files: FileCounts::default(),
        particles: None,
    }
}

/// Launches the solver in `rec.solve` with the contract's argument, logging both streams beside
/// `solve/` and classifying every line as it arrives, then judges the run. Every ending is a
/// [`Launched`], so a launched run always gets its `run.json`.
fn launch(rec: &Record, cancel: &CancelToken, on_event: &mut dyn FnMut(&RunEvent)) -> Launched {
    let opts = rec.opts;
    let mut logs = match Logs::create(&rec.dir) {
        Ok(l) => l,
        Err(reason) => return not_launched(reason, ClassCounts::default()),
    };
    let spec = Spec {
        program: opts.solver_exe.clone(),
        args: vec![SOLVER_ARGUMENT.into()],
        cwd: rec.solve.clone(),
    };
    let mut classifier = Classifier::new();
    let mut sink = Sink {
        on_event,
        lines: Vec::new(),
        cancel,
        cancel_at: opts.cancel_after_progress,
    };
    let timer = opts
        .cancel_after_ms
        .map(|ms| CancelTimer::start(Duration::from_millis(ms), cancel.clone()));
    let result = process::run(&spec, cancel, &mut |l: &Line| {
        logs.line(l);
        for c in classifier.push(l) {
            sink.take(c);
        }
    });
    drop(timer);
    for c in classifier.finish() {
        sink.take(c);
    }
    let lines = sink.lines;
    let log_warning = logs.finish();
    let mut launched = match result {
        Ok(outcome) => judged(rec, outcome, &lines),
        Err(e) => not_launched(
            Reason::new(
                codes::LAUNCH_FAILED,
                format!("{}: {e}", opts.solver_exe.display()),
            ),
            ClassCounts::of(&lines),
        ),
    };
    launched.verdict.warnings.extend(log_warning);
    launched
}

/// The verdict on a solver that ran and ended with `outcome`, from its `lines` and the files in
/// `rec.solve`.
fn judged(rec: &Record, outcome: Outcome, lines: &[Classified]) -> Launched {
    let opts = rec.opts;
    let exp = Expectation::read(&rec.solve, opts.solver);
    let outputs = match &exp {
        Ok(e) => Outputs::read(&rec.solve, e),
        Err(_) => Outputs::default(),
    };
    let verdict = judge(&Evidence {
        solver: opts.solver,
        outcome: &outcome,
        lines,
        expectation: exp.as_ref(),
        outputs: &outputs,
        loss_limit: opts.loss_limit,
    });
    let files = match &exp {
        Ok(e) => FileCounts::of(&e.expected_files(), &outputs),
        Err(_) => FileCounts {
            total: outputs.files.len(),
            ..FileCounts::default()
        },
    };
    Launched {
        outcome: Some(outcome),
        verdict,
        lines: ClassCounts::of(lines),
        files,
        particles: outputs.stats.and_then(Result::ok),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A log writer that takes `room` bytes and then fails, and whose flush fails when
    /// `flush_fails`.
    struct Full {
        room: usize,
        got: Vec<u8>,
        flush_fails: bool,
    }

    impl Write for Full {
        fn write(&mut self, b: &[u8]) -> io::Result<usize> {
            if b.len() > self.room {
                return Err(io::Error::other("disk full"));
            }
            self.room -= b.len();
            self.got.extend_from_slice(b);
            Ok(b.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            if self.flush_fails {
                return Err(io::Error::other("flush failed"));
            }
            Ok(())
        }
    }

    fn full(room: usize, flush_fails: bool) -> Full {
        Full {
            room,
            got: Vec::new(),
            flush_fails,
        }
    }

    fn line(stream: process::Stream, text: &str, terminated: bool) -> Line {
        Line {
            stream,
            t_ms: 0.0,
            text: text.into(),
            terminated,
        }
    }

    #[test]
    fn a_failed_log_write_is_a_warning_and_the_logs_go_on() {
        use process::Stream::{Stderr, Stdout};
        let mut logs = Logs {
            out: (full(4, false), PathBuf::from("o.txt")),
            err: (full(100, false), PathBuf::from("e.txt")),
            failed: None,
        };
        logs.line(&line(Stdout, "abc", true));
        logs.line(&line(Stdout, "def", true));
        logs.line(&line(Stderr, "tail", false));
        // The failed line is lost, the other log is still written, unterminated line as it came.
        assert_eq!(logs.out.0.got, b"abc\n");
        assert_eq!(logs.err.0.got, b"tail");
        let w = logs.finish().expect("the write failure is kept");
        assert_eq!(w.code, "log_write_failed");
        assert!(w.detail.starts_with("o.txt: disk full"), "{}", w.detail);

        // A failed flush counts too; the first failure is the one reported.
        let logs = Logs {
            out: (full(100, false), PathBuf::from("o.txt")),
            err: (full(100, true), PathBuf::from("e.txt")),
            failed: None,
        };
        let w = logs.finish().expect("the flush failure is kept");
        assert!(w.detail.starts_with("e.txt: flush failed"), "{}", w.detail);

        // Nothing failed: no warning.
        let logs = Logs {
            out: (full(100, false), PathBuf::from("o.txt")),
            err: (full(100, false), PathBuf::from("e.txt")),
            failed: None,
        };
        assert_eq!(logs.finish(), None);
    }

    #[test]
    fn logs_that_cannot_be_created_are_launch_failed() {
        let base = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/tmp")
            .join(format!("manager-logs-{}", std::process::id()));
        for blocked in [STDOUT_LOG, STDERR_LOG] {
            let dir = base.join(blocked);
            // A folder where the log file should go: File::create fails on it.
            fs::create_dir_all(dir.join(blocked)).unwrap();
            let Err(r) = Logs::create(&dir) else {
                panic!("{blocked} was created over a folder");
            };
            assert_eq!(r.code, "launch_failed");
            assert!(r.detail.contains(blocked), "{}", r.detail);
        }
        let free = base.join("free");
        fs::create_dir_all(&free).unwrap();
        assert!(Logs::create(&free).is_ok());
        fs::remove_dir_all(&base).unwrap();
    }

    #[test]
    fn exit_classes_follow_the_stage_and_status() {
        use Stage::*;
        assert_eq!(ExitClass::of(Solve, Status::Ok), ExitClass::Ok);
        assert_eq!(ExitClass::of(Geometry, Status::Fail), ExitClass::Geometry);
        assert_eq!(ExitClass::of(Validate, Status::Fail), ExitClass::Usage);
        assert_eq!(ExitClass::of(Export, Status::Fail), ExitClass::Usage);
        assert_eq!(ExitClass::of(Mesh, Status::Fail), ExitClass::Mesh);
        assert_eq!(ExitClass::of(Mesh, Status::Cancelled), ExitClass::Cancelled);
        assert_eq!(ExitClass::of(PreLaunch, Status::Fail), ExitClass::Solver);
        assert_eq!(ExitClass::of(Solve, Status::Crash), ExitClass::Solver);
        assert_eq!(
            ExitClass::of(Solve, Status::Cancelled),
            ExitClass::Cancelled
        );
        let codes: Vec<u8> = [
            ExitClass::Ok,
            ExitClass::Usage,
            ExitClass::Geometry,
            ExitClass::Mesh,
            ExitClass::Solver,
            ExitClass::Cancelled,
        ]
        .map(ExitClass::code)
        .to_vec();
        assert_eq!(codes, [0, 2, 3, 4, 5, 130]);
        assert_eq!(serde_json::to_string(&ExitClass::Cancelled).unwrap(), "130");
        assert_eq!(
            serde_json::from_str::<ExitClass>("4").unwrap(),
            ExitClass::Mesh
        );
        assert!(serde_json::from_str::<ExitClass>("1").is_err());
    }

    #[test]
    fn a_run_folders_ids_follow_tetgens_numbering() {
        let tet = |id_volume| mbin::Tetrahedron {
            vertices: [0; 4],
            id_volume,
            faces: [mbin::TetraFace::default(); 4],
        };
        let mesh = |ids: &[i32]| mbin::Mesh {
            nodes: Vec::new(),
            tetrahedra: ids.iter().map(|&i| tet(i)).collect(),
        };
        let unknown = |ids: &[i32], fittings: &[i32]| -> Vec<i32> {
            let v = volume_ids(&mesh(ids), fittings.to_vec());
            ids.iter().copied().filter(|&i| !v.knows(i)).collect()
        };
        // Tutorial 3's run folders: two fittings, the room in three parts above them.
        assert_eq!(
            volume_ids(&mesh(&[2084, 1930, 2085, 2083, 2086]), vec![1930, 2083]).room,
            2084
        );
        assert!(unknown(&[2084, 1930, 2085, 2083, 2086], &[1930, 2083]).is_empty());
        // With fittings, a room written 0 (this crate's builder before 2026-09-24) is not
        // TetGen's numbering: refused.
        assert_eq!(unknown(&[0, 2, 2, 2], &[2]), [0]);
        // Without fittings, the room starts at the smallest id: TetGen's 1, or the 0 of
        // upstream's Python-binding mesh and Night Mode's broken hall.
        assert_eq!(volume_ids(&mesh(&[1, 1, 2]), vec![]).room, 1);
        assert_eq!(volume_ids(&mesh(&[0, 0]), vec![]).room, 0);
        assert!(unknown(&[3, 1], &[]).is_empty());
        assert_eq!(volume_ids(&mesh(&[]), vec![]).room, 1);
    }

    #[test]
    fn the_band_check_says_no_to_a_short_spectrum_only() {
        let config = |src: &str, docalc1000: &str| {
            format!(
                r#"<configuration><simulation><freq_enum><bfreq freq="1000" docalc="{docalc1000}"/><bfreq freq="500" docalc="1"/></freq_enum></simulation><sources>{src}</sources></configuration>"#
            )
        };
        let one = r#"<source name="S"><bfreq freq="1000" db="90"/></source>"#;
        let two =
            r#"<source name="S"><bfreq freq="500" db="90"/><bfreq freq="1000" db="90"/></source>"#;
        let check = |xml: String| band_check(&Document::parse(&xml).unwrap());
        // spps_oneband's shape: 500 and 1000 Hz computed, one entry.
        let r = check(config(one, "1")).expect("a short spectrum is refused");
        assert_eq!(r.code, "band_set_mismatch");
        assert!(r.detail.contains("has 1 band entries"), "{}", r.detail);
        assert_eq!(check(config(two, "1")), None);
        // The entry past the end belongs to a band that is not computed: nothing is read there.
        assert_eq!(check(config(one, "0")), None);
    }
}
