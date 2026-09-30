//! Running the bed's matrix through the run manager (`run::manager::run_project`), `jobs` solver
//! processes at once, and reading each run as it ends; or reading an earlier bed's runs again
//! (`--from`) with this build and no solver run.
//!
//! Each run gets its own folder, `runs/<cell>/s<seed>/`, holding `project.simpa` and the run
//! folder the run manager makes there (`run.json`, `mesh/`, `solve/`). Every project is meshed by
//! the run, with the default mesher and the verified TetGen.

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use super::bind::{self, Seal};
use super::check::{AtmosphericReads, Reads};
use super::file::{self, BedFile, CellSpec};
use super::limits;
use super::read::{self, SppsRead, TcrRead};
use crate::process::CancelToken;
use crate::run::manager::{self, MeshChoice};
use crate::run::{DEFAULT_LOSS_LIMIT, ExeSearch, RunOptions};
use crate::schema::{self, Project, SolverKind};

/// The four executables, found as `simpa run` finds them.
#[derive(Clone, Debug)]
pub struct Exes {
    pub spps: PathBuf,
    pub tcr: PathBuf,
    pub tetgen: PathBuf,
    pub preprocess: PathBuf,
}

impl Exes {
    /// `$SIMPA_SOLVERS_DIR`, else beside `simpa.exe`, else the dev tree.
    pub fn find() -> Result<Exes, String> {
        let s = ExeSearch::from_env(None);
        let f = |n: &str| s.find(n).map_err(|e| e.to_string());
        Ok(Exes {
            spps: f("spps.exe")?,
            tcr: f("classicalTheory.exe")?,
            tetgen: f(manager::TETGEN_EXE_NAME)?,
            preprocess: f(manager::PREPROCESS_EXE_NAME)?,
        })
    }

    /// `(name, path)` of each, in `SOLVER_EXES`' order.
    pub fn list(&self) -> Vec<(&'static str, &Path)> {
        vec![
            ("spps.exe", self.spps.as_path()),
            ("classicalTheory.exe", self.tcr.as_path()),
            ("tetgen.exe", self.tetgen.as_path()),
            ("preprocess.exe", self.preprocess.as_path()),
        ]
    }
}

/// What a run is for.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum RunKey {
    Cell { id: String, seed: u32 },
    Tcr { id: String },
    AtmosphericSpps { seed: u32 },
    AtmosphericTcr,
    N5 { seed: u32 },
    N6 { seed: u32 },
}

impl RunKey {
    /// The run's folder under the bed's.
    pub fn dir(&self) -> PathBuf {
        let (a, s) = match self {
            RunKey::Cell { id, seed } => (id.clone(), *seed),
            RunKey::Tcr { id } => (id.clone(), 1),
            RunKey::AtmosphericSpps { seed } => ("atmospheric-validation".into(), *seed),
            RunKey::AtmosphericTcr => ("atmospheric-validation-tcr".into(), 1),
            RunKey::N5 { seed } => ("say-no-n5".into(), *seed),
            RunKey::N6 { seed } => ("say-no-n6".into(), *seed),
        };
        Path::new("runs").join(a).join(format!("s{s}"))
    }

    pub fn label(&self) -> String {
        match self {
            RunKey::Cell { id, seed } => format!("{id} seed {seed}"),
            RunKey::Tcr { id } => id.clone(),
            RunKey::AtmosphericSpps { seed } => format!("atmospheric validation seed {seed}"),
            RunKey::AtmosphericTcr => "atmospheric validation TCR".into(),
            RunKey::N5 { seed } => format!("say-NO N5 seed {seed}"),
            RunKey::N6 { seed } => format!("say-NO N6 seed {seed}"),
        }
    }
}

/// One run to make: its key, solver, project, the receivers it reads, and its predicted cost.
#[derive(Clone, Debug)]
pub struct Planned {
    pub key: RunKey,
    pub solver: SolverKind,
    pub project: Project,
    /// `Some(n)`: the bed's receivers `R000`...; `None`: every point receiver.
    pub receivers: Option<usize>,
    /// Particle-reflections summed over the bands, SPPS's time in arbitrary units (the spec's
    /// cost model, section 8), to start the longest runs first.
    pub cost: f64,
}

/// The files a run is expected to leave, for the exFAT projection: an SPPS run of the matrix
/// left 39 (section 8), upstream's validation 52; TCR's were not measured, and take the same.
pub const FILES_PER_RUN: u64 = 45;
pub const FILES_PER_ATMOSPHERIC_RUN: u64 = 60;
/// The bed's own outputs besides one decay file per cell.
pub const FILES_OF_OUTPUTS: u64 = 16;

/// Particle-reflections summed over the bands (section 8's model).
fn cost(bed: &BedFile, spec: &CellSpec) -> f64 {
    let ell = {
        let [x, y, z] = spec.room.size_m;
        4.0 * x * y * z / (2.0 * (x * y + y * z + x * z))
    };
    let per_band: f64 = bed
        .bands_hz(spec.air)
        .iter()
        .map(|&f| {
            let m = super::check::bed_air(bed, spec.air, f64::from(f)).unwrap_or(0.0);
            match spec.method {
                file::Method::Energetic => {
                    let r = 10.0 * spec.trans_epsilon
                        / (-10.0 * (1.0 - spec.alpha).log10() + 4.343 * m * ell);
                    r.min(spec.duration_s * 343.2 / ell)
                }
                file::Method::Random => 1.0 / (1.0 - (1.0 - spec.alpha) * (-m * ell).exp()),
            }
        })
        .sum();
    f64::from(spec.particles_per_source) * per_band
}

/// Every run of the bed: its cells over its seeds, TCR, upstream's atmospheric validation (when
/// `upstream`, its tree, is given and holds the `.proj` of the right sha256; otherwise why not),
/// and the say-NO runs N5 and N6. Longest first.
pub fn plan(
    bed: &BedFile,
    upstream: Option<&Path>,
) -> Result<(Vec<Planned>, Option<String>), String> {
    let mut out = Vec::new();
    for cell in &bed.cells {
        let spec = CellSpec::of(bed, cell)?;
        let bands = bed.bands_hz(cell.air);
        for &seed in &bed.seeds {
            out.push(Planned {
                key: RunKey::Cell {
                    id: cell.id.clone(),
                    seed,
                },
                solver: SolverKind::Spps,
                project: file::cell_project(&spec, seed, bed.time_step_s, bands)?,
                receivers: Some(spec.room.receivers_m.len()),
                cost: cost(bed, &spec),
            });
        }
    }
    for t in &bed.tcr {
        out.push(Planned {
            key: RunKey::Tcr { id: t.id.clone() },
            solver: SolverKind::Tcr,
            project: file::tcr_project(bed, t)?,
            receivers: None,
            cost: 0.0,
        });
    }
    let n5 = &bed.say_no.n5;
    let cell = bed.cell(&n5.cell).ok_or(format!("no cell '{}'", n5.cell))?;
    let mut spec = CellSpec::of(bed, cell)?;
    spec.particles_per_source = n5.particles_per_source;
    out.push(Planned {
        key: RunKey::N5 { seed: n5.seed },
        solver: SolverKind::Spps,
        project: file::cell_project(&spec, n5.seed, bed.time_step_s, bed.bands_hz(cell.air))?,
        receivers: Some(spec.room.receivers_m.len()),
        cost: cost(bed, &spec),
    });
    let n6 = &bed.say_no.n6;
    let spec = CellSpec {
        room: bed.room(&n6.room).ok_or(format!("no room '{}'", n6.room))?,
        alpha: n6.alpha,
        method: n6.method,
        particles_per_source: n6.particles_per_source,
        duration_s: n6.duration_s,
        trans_epsilon: n6.trans_epsilon,
        air: false,
        scattering: n6.scattering,
    };
    out.push(Planned {
        key: RunKey::N6 { seed: n6.seed },
        solver: SolverKind::Spps,
        project: file::cell_project(&spec, n6.seed, bed.time_step_s, bed.bands_hz(false))?,
        receivers: Some(spec.room.receivers_m.len()),
        cost: cost(bed, &spec),
    });
    let v = &bed.atmospheric_validation;
    let not_run = match upstream {
        None => Some(
            "upstream's tree was not given (--upstream or $SIMPA_UPSTREAM): upstream's \
             atmospheric-absorption validation was not run"
                .to_string(),
        ),
        Some(root) => {
            let proj = root.join(&v.proj);
            let projects: Result<Vec<(u32, Project)>, String> = v
                .seeds
                .iter()
                .map(|&seed| file::atmospheric_project(&proj, v, seed).map(|p| (seed, p)))
                .collect();
            match projects {
                Err(e) => Some(format!("not run: {e}")),
                Ok(projects) => {
                    if let Some((_, first)) = projects.first() {
                        let mut p = first.clone();
                        p.solvers.spps.random_seed = 1;
                        out.push(Planned {
                            key: RunKey::AtmosphericTcr,
                            solver: SolverKind::Tcr,
                            project: p,
                            receivers: None,
                            cost: 0.0,
                        });
                    }
                    for (seed, p) in projects {
                        let particles = f64::from(p.solvers.spps.particles_per_source);
                        let bands = p.bands.len() as f64;
                        out.push(Planned {
                            key: RunKey::AtmosphericSpps { seed },
                            solver: SolverKind::Spps,
                            project: p,
                            receivers: None,
                            // Energetic at ε 7 and α 0.1: about 150 reflections a band.
                            cost: particles * bands * 150.0,
                        });
                    }
                    None
                }
            }
        }
    };
    Ok(finish(out, not_run))
}

fn finish(mut out: Vec<Planned>, not_run: Option<String>) -> (Vec<Planned>, Option<String>) {
    out.sort_by(|a, b| b.cost.total_cmp(&a.cost).then_with(|| a.key.cmp(&b.key)));
    (out, not_run)
}

/// The runs of gate C's one extension of `cells`: their seeds `bed.extension_seeds`.
pub fn plan_extension(bed: &BedFile, cells: &[String]) -> Result<Vec<Planned>, String> {
    let mut out = Vec::new();
    for id in cells {
        let cell = bed.cell(id).ok_or(format!("no cell '{id}'"))?;
        let spec = CellSpec::of(bed, cell)?;
        for &seed in &bed.extension_seeds {
            out.push(Planned {
                key: RunKey::Cell {
                    id: id.clone(),
                    seed,
                },
                solver: SolverKind::Spps,
                project: file::cell_project(&spec, seed, bed.time_step_s, bed.bands_hz(cell.air))?,
                receivers: Some(spec.room.receivers_m.len()),
                cost: cost(bed, &spec),
            });
        }
    }
    Ok(finish(out, None).0)
}

/// The files `runs` are expected to leave, and the bed's outputs.
pub fn projected_files(bed: &BedFile, runs: &[Planned]) -> u64 {
    runs.iter()
        .map(|p| match p.key {
            RunKey::AtmosphericSpps { .. } | RunKey::AtmosphericTcr => FILES_PER_ATMOSPHERIC_RUN,
            _ => FILES_PER_RUN,
        })
        .sum::<u64>()
        + bed.cells.len() as u64
        + FILES_OF_OUTPUTS
}

/// A run read.
#[derive(Clone, Debug)]
pub enum Read {
    Spps(Box<SppsRead>),
    Tcr(Box<TcrRead>),
}

/// Maps `f` over `items` on `jobs` threads, in order; the fault set on the calling thread, if
/// any, is set on each worker too (`faults`), so a say-NO reaches the work it wraps.
pub fn par_map<T: Sync, R: Send>(items: &[T], jobs: usize, f: impl Fn(&T) -> R + Sync) -> Vec<R> {
    let fault = crate::faults::active();
    let next = AtomicUsize::new(0);
    let out: Mutex<Vec<(usize, R)>> = Mutex::new(Vec::with_capacity(items.len()));
    std::thread::scope(|scope| {
        for _ in 0..jobs.clamp(1, items.len().max(1)) {
            scope.spawn(|| {
                with_fault(fault, || {
                    loop {
                        let i = next.fetch_add(1, Ordering::SeqCst);
                        let Some(item) = items.get(i) else { break };
                        let r = f(item);
                        out.lock().expect("no worker panicked").push((i, r));
                    }
                })
            });
        }
    });
    let mut v = out.into_inner().expect("no worker panicked");
    v.sort_by_key(|(i, _)| *i);
    v.into_iter().map(|(_, r)| r).collect()
}

#[cfg(feature = "fault-injection")]
fn with_fault<R>(fault: Option<crate::faults::Fault>, body: impl FnOnce() -> R) -> R {
    match fault {
        Some(f) => crate::faults::with(f, body),
        None => body(),
    }
}

#[cfg(not(feature = "fault-injection"))]
fn with_fault<R>(_: Option<crate::faults::Fault>, body: impl FnOnce() -> R) -> R {
    body()
}

/// Reads the run in `dir` (the run folder the run manager made there), its files first bound to
/// `run.json`'s output hashes or to `seal`'s entry for it ([`bind::bind`]).
pub fn read_in(dir: &Path, p: &Planned, seal: Option<&Seal>) -> Result<Read, String> {
    let folder =
        read::find_run_folder(dir).ok_or_else(|| format!("{}: no run folder", dir.display()))?;
    read_folder(dir, &folder, p, seal)
}

/// Binds the run folder `folder` under the run's folder `dir` to its records, then hashes what
/// says what it ran ([`read::on_disk`]) and reads it. Nothing is read from a file that is not
/// its record's.
fn read_folder(
    dir: &Path,
    folder: &Path,
    p: &Planned,
    seal: Option<&Seal>,
) -> Result<Read, String> {
    let manifest = read::manifest_of(folder)?;
    // A seal holds a whole bed: a run it does not name (gate C's extension seeds, say) is bound
    // to nothing the bed did not itself leave, and is not read on its own run.json's word.
    let sealed = match seal {
        Some(s) => Some(s.run(&p.key).ok_or_else(|| unsealed(folder, s, &p.key))?),
        None => None,
    };
    let bound = bind::bind(
        dir,
        folder,
        manifest.outputs.as_deref(),
        &manifest.inputs,
        sealed,
    )?;
    let on_disk = read::on_disk(dir, folder, &manifest);
    let mut r = match p.solver {
        SolverKind::Spps => {
            read::read_spps(folder, p.receivers).map(|r| Read::Spps(Box::new(r)))?
        }
        SolverKind::Tcr => read::read_tcr(folder).map(|r| Read::Tcr(Box::new(r)))?,
    };
    let info = match &mut r {
        Read::Spps(s) => &mut s.info,
        Read::Tcr(t) => &mut t.info,
    };
    info.bound_by = Some(bound.by.join(" and "));
    info.outputs_sha256 = Some(bound.outputs_sha256);
    info.output_files = bound.outputs;
    info.on_disk = Some(on_disk);
    Ok(r)
}

/// Why a run of a sealed bed that its seal does not name is refused.
fn unsealed(folder: &Path, seal: &Seal, key: &RunKey) -> String {
    format!(
        "{}: {}: the seal of the bed {} names no run {}, and a run of a sealed bed is bound only \
         by its seal: its own run.json is in the folder being judged",
        bind::RUN_UNBOUND,
        folder.display(),
        seal.bed,
        bind::seal_key(key)
    )
}

/// Runs `p` under `root` and reads it.
pub fn run_one(p: &Planned, root: &Path, exes: &Exes) -> Result<Read, String> {
    let dir = root.join(p.key.dir());
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let project = dir.join("project.simpa");
    schema::save(&p.project, &project).map_err(|e| format!("{}: {e}", project.display()))?;
    let opts = RunOptions {
        solver: p.solver,
        solver_exe: match p.solver {
            SolverKind::Spps => exes.spps.clone(),
            SolverKind::Tcr => exes.tcr.clone(),
        },
        runs_root: dir.clone(),
        loss_limit: DEFAULT_LOSS_LIMIT,
        cancel_after_ms: Some(limits::RUN_TIME_LIMIT_MS),
        cancel_after_progress: None,
        verify: None,
    };
    let mesh = MeshChoice::Build {
        tetgen: exes.tetgen.clone(),
        preprocess: Some(exes.preprocess.clone()),
    };
    let report = manager::run_project(
        &project,
        None,
        &mesh,
        &opts,
        &CancelToken::new(),
        &mut |_| {},
    )
    .map_err(|e| format!("simpa run refused or failed: {e}"))?;
    read_folder(&dir, &report.dir, p, None)
}

/// What [`execute`] calls as each run ends: how many have ended, the run, what was read, and
/// its wall time, s.
pub type RunDone<'a> = dyn Fn(usize, &Planned, &Result<Read, String>, f64) + Sync + 'a;

/// Runs every planned run under `root`, `jobs` at once, calling `done` as each ends.
pub fn execute(
    runs: &[Planned],
    root: &Path,
    exes: &Exes,
    jobs: usize,
    done: &RunDone<'_>,
) -> Vec<Result<Read, String>> {
    let count = AtomicUsize::new(0);
    par_map(runs, jobs, |p| {
        let t0 = std::time::Instant::now();
        let r = run_one(p, root, exes);
        let n = count.fetch_add(1, Ordering::SeqCst) + 1;
        done(n, p, &r, t0.elapsed().as_secs_f64());
        r
    })
}

/// Reads every planned run from an earlier bed's folder `from`, each bound to its `run.json`'s
/// output hashes or to `seal`, the committed seal of that bed, when its runs predate them.
pub fn read_existing(
    runs: &[Planned],
    from: &Path,
    jobs: usize,
    seal: Option<&Seal>,
) -> Vec<Result<Read, String>> {
    par_map(runs, jobs, |p| read_in(&from.join(p.key.dir()), p, seal))
}

/// The code a run is refused with when it is not the run its folder is filed under
/// ([`check_planned`]).
pub const RUN_NOT_PLANNED: &str = "bed_run_not_planned";

/// The sha256 of the project file the bed writes for `p` (`schema::save` writes
/// `schema::to_json`), which `run.json`'s `source.sha256` records when the run starts.
pub fn planned_project_sha256(p: &Planned) -> String {
    crate::run::manifest::sha256_bytes(schema::to_json(&p.project).as_bytes())
}

/// What the bed writes for a planned run, hashed as [`read::on_disk`] hashes a run's files: the
/// project file, and from the project as the run manager runs it
/// ([`crate::run::manager::project_as_run`]), the solver's `config.xml` (its `workingdirectory`
/// left out), its scene mesh and its mesh stamp.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlannedInputs {
    pub project_sha256: String,
    pub config_sha256: String,
    pub scene_sha256: String,
    pub mesh_input_hash: String,
}

/// [`PlannedInputs`] of `p`.
pub fn planned_inputs(p: &Planned) -> Result<PlannedInputs, String> {
    use crate::run::manifest::sha256_bytes;
    let json = schema::to_json(&p.project);
    let project = crate::run::manager::project_as_run(json.as_bytes(), None)?;
    // Any absolute folder: its value is left out of the comparison.
    let workdir = Path::new(if cfg!(windows) { r"C:\" } else { "/" });
    let config = crate::config_xml::write(&project, p.solver, None, workdir)
        .map_err(|e| format!("config.xml: {} ({e})", e.code()))?;
    let scene = crate::config_xml::scene_mesh(&project)
        .map_err(|e| format!("the scene mesh: {} ({e})", e.code()))?;
    Ok(PlannedInputs {
        project_sha256: sha256_bytes(json.as_bytes()),
        config_sha256: sha256_bytes(bind::without_workdir(&config).as_bytes()),
        scene_sha256: sha256_bytes(&crate::formats::cbin::write(&scene)),
        mesh_input_hash: crate::validate::mesh_input_hash(&project),
    })
}

/// A file hashed from disk against what the plan writes, or why not.
fn hash_problem(what: &str, got: Option<&str>, want: &str) -> Option<String> {
    match got {
        Some(g) if g == want => None,
        Some(g) => Some(format!("{what} is {g}, the plan's {want}")),
        None => Some(format!(
            "{what} is missing or unreadable, the plan's {want}"
        )),
    }
}

/// How the files a run was read from differ from what the bed writes for `p`, each named.
fn disk_problems(p: &Planned, info: &read::RunInfo) -> Vec<String> {
    let mut problems = Vec::new();
    if info.bound_by.is_none() || info.outputs_sha256.is_none() {
        problems.push(
            "its files were bound to no record (run.json's output hashes or a seal) when it was \
             read"
                .to_string(),
        );
    }
    let Some(d) = &info.on_disk else {
        problems.push("nothing of it was hashed from disk when it was read".into());
        return problems;
    };
    let want = match planned_inputs(p) {
        Ok(w) => w,
        Err(e) => {
            problems.push(format!("the bed cannot write the plan's inputs: {e}"));
            return problems;
        }
    };
    let s = |x: &Option<String>| x.clone();
    for (what, got, want) in [
        (
            "the sha256 of project.simpa on disk",
            s(&d.project_sha256),
            &want.project_sha256,
        ),
        (
            "the sha256 of its config.xml, workingdirectory aside,",
            s(&d.config_sha256),
            &want.config_sha256,
        ),
        (
            "the sha256 of its scene mesh (solve/mesh.cbin)",
            s(&d.scene_sha256),
            &want.scene_sha256,
        ),
        (
            "the mesh stamp run.json records",
            s(&d.run_mesh_input_hash),
            &want.mesh_input_hash,
        ),
        (
            "the mesh stamp mesh.json records",
            s(&d.mesh_json_input_hash),
            &want.mesh_input_hash,
        ),
    ] {
        problems.extend(hash_problem(what, got.as_deref(), want));
    }
    match (&d.mbin_sha256, &d.run_mbin_sha256, &d.mesh_json_mbin_sha256) {
        (Some(a), Some(b), Some(c)) if a == b && b == c => {}
        (a, b, c) => problems.push(format!(
            "its solve/tetramesh.mbin has sha256 {a:?}, run.json records {b:?} and mesh.json \
             {c:?}: not the mesh whose stamp is checked"
        )),
    }
    match &d.mesh_check {
        Some(reasons) if reasons.is_empty() => {}
        Some(reasons) => problems.push(format!(
            "its solve/tetramesh.mbin does not hold to its scene mesh: {}",
            reasons.join("; ")
        )),
        None => problems.push("its mesh was not checked against its scene mesh".into()),
    }
    problems
}

/// The bands `computed` marks, of `frequencies_hz`.
fn computed_bands(frequencies_hz: &[u32], computed: &[bool]) -> Vec<i32> {
    frequencies_hz
        .iter()
        .zip(computed)
        .filter(|(_, on)| **on)
        .map(|(&f, _)| f as i32)
        .collect()
}

/// A setting SPPS reads as a C `float`, as it reads it from `config.xml` (the writer prints the
/// shortest text that reads back as `v`; `atof`, then the `float`).
fn as_spps_reads(v: f64) -> f64 {
    f64::from(v as f32)
}

/// Refuses `r` unless it is the run the bed files under `p`'s key, from `p.project`: the
/// project file's sha256 in `run.json` is the sha256 of the project the bed writes for that
/// cell (or TCR run, say-NO run or atmospheric seed) and seed; and what the solver read is that
/// project's: for SPPS the particles per source, the method, the step and `trans_epsilon` (as
/// SPPS reads them, `f32`), the seed and the bands computed; for TCR the bands. And (M8b) the
/// files it was read from were bound to their record when it was read, and as hashed from disk
/// are what the bed writes for the plan ([`planned_inputs`]): `project.simpa`, the solver's
/// `config.xml` (its `workingdirectory` aside) and scene mesh, the mesh stamp in `run.json` and
/// `mesh.json`, one `.mbin` in all three, and that `.mbin` held to the scene by the run
/// manager's reused-mesh check (`read::OnDisk::mesh_check`). Fresh runs and runs read with
/// `--from` both pass through here ([`into_reads`]), so a run of another matrix filed under the
/// same folder names (an exploratory bed at 10 ms, say), or a run whose `run.json` was edited to
/// name another plan, is refused with [`RUN_NOT_PLANNED`], every difference named, and its cell
/// is not judged (E2).
pub fn check_planned(p: &Planned, r: &Read) -> Result<(), String> {
    let mut problems = Vec::new();
    let (info, solver) = match r {
        Read::Spps(s) => (&s.info, SolverKind::Spps),
        Read::Tcr(t) => (&t.info, SolverKind::Tcr),
    };
    if solver != p.solver {
        problems.push(format!(
            "a {solver:?} run where the bed runs {:?}",
            p.solver
        ));
    }
    let want = planned_project_sha256(p);
    match &info.project_sha256 {
        Some(got) if *got == want => {}
        Some(got) => problems.push(format!(
            "its project has sha256 {got}, the project the bed writes for it {want}"
        )),
        None => problems.push("its run.json records no project (a fixture run)".into()),
    }
    let spps = &p.project.solvers.spps;
    match r {
        Read::Spps(s) => {
            let mut differ = |what: &str, got: String, want: String| {
                if got != want {
                    problems.push(format!("{what} {got}, the bed's {want}"));
                }
            };
            differ(
                "particles per source",
                s.particles_per_source.to_string(),
                spps.particles_per_source.to_string(),
            );
            differ(
                "computation_method",
                s.computation_method.to_string(),
                spps.method.solver_code().to_string(),
            );
            differ(
                "time_step_s",
                format!("{:?}", s.time_step_s),
                format!("{:?}", as_spps_reads(spps.time_step_s.get())),
            );
            differ(
                "trans_epsilon",
                format!("{:?}", s.trans_epsilon),
                format!("{:?}", as_spps_reads(spps.extinction_exponent.get())),
            );
            differ(
                "random_seed",
                s.random_seed
                    .map_or_else(|| "none".into(), |v| v.to_string()),
                spps.random_seed.to_string(),
            );
            let bands = computed_bands(&p.project.bands.frequencies_hz, &spps.bands_computed);
            differ("bands", format!("{:?}", s.bands_hz), format!("{bands:?}"));
        }
        Read::Tcr(t) => {
            let bands = computed_bands(
                &p.project.bands.frequencies_hz,
                &p.project.solvers.tcr.bands_computed,
            );
            let got: Vec<i32> = t.bands.iter().map(|b| b.freq_hz).collect();
            if got != bands {
                problems.push(format!("bands {got:?}, the bed's {bands:?}"));
            }
        }
    }
    // M8b: run.json's sha256 is the run manager's word. The files themselves, as they are on
    // disk now and bound to their record, must be the ones the bed writes for the plan.
    problems.extend(disk_problems(p, info));
    if problems.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "{RUN_NOT_PLANNED}: {} is not the run the bed files under {}: {}",
            info.folder,
            p.key.label(),
            problems.join("; ")
        ))
    }
}

/// Puts what was run or read where [`super::report::evaluate`] looks for it, each run first
/// matched with the run it is filed under ([`check_planned`]): one that is not is an `Err`.
pub fn into_reads(
    reads: &mut Reads,
    runs: &[Planned],
    results: Vec<Result<Read, String>>,
    atmospheric_not_run: Option<String>,
) {
    if atmospheric_not_run.is_some() {
        reads.atmospheric = AtmosphericReads {
            not_run: atmospheric_not_run,
            ..Default::default()
        };
    }
    for (p, r) in runs.iter().zip(results) {
        let r = r.and_then(|read| check_planned(p, &read).map(|()| read));
        let spps = |r: Result<Read, String>| -> Result<SppsRead, String> {
            match r? {
                Read::Spps(s) => Ok(*s),
                Read::Tcr(_) => Err("a TCR run where SPPS was expected".into()),
            }
        };
        let tcr = |r: Result<Read, String>| -> Result<TcrRead, String> {
            match r? {
                Read::Tcr(t) => Ok(*t),
                Read::Spps(_) => Err("an SPPS run where TCR was expected".into()),
            }
        };
        match &p.key {
            RunKey::Cell { id, seed } => {
                reads
                    .spps
                    .entry(id.clone())
                    .or_default()
                    .insert(*seed, spps(r));
            }
            RunKey::Tcr { id } => {
                reads.tcr.insert(id.clone(), tcr(r));
            }
            RunKey::AtmosphericSpps { seed } => {
                reads.atmospheric.spps.insert(*seed, spps(r));
            }
            RunKey::AtmosphericTcr => reads.atmospheric.tcr = Some(tcr(r)),
            RunKey::N5 { .. } => reads.n5 = Some(spps(r)),
            RunKey::N6 { .. } => reads.n6 = Some(spps(r)),
        }
    }
    // M8b: every seed's run is its own. Over all of a cell's seeds read so far (an extension's
    // with the first ten), and over the atmospheric validation's.
    for seeds in reads.spps.values_mut() {
        refuse_shared_outputs(seeds);
    }
    refuse_shared_outputs(&mut reads.atmospheric.spps);
}

/// Refuses, [`bind::SEEDS_IDENTICAL`], every seed of `seeds` that shares a solver output file
/// byte for byte (the same path and sha256) with another, or whose outputs are all another's.
/// A seed's own random walk makes every output file its own: in M8a's bed no output file of 8,490
/// is any other seed's of the same cell. So a shared file is a seed's run filed under another,
/// or a solver that ignores its seed.
fn refuse_shared_outputs(seeds: &mut std::collections::BTreeMap<u32, Result<SppsRead, String>>) {
    use std::collections::{BTreeMap, BTreeSet};
    let mut owners: BTreeMap<(&str, &str), Vec<u32>> = BTreeMap::new();
    let mut whole: BTreeMap<&str, Vec<u32>> = BTreeMap::new();
    for (s, r) in seeds.iter() {
        let Ok(r) = r else { continue };
        for (path, sha) in &r.info.output_files {
            owners.entry((path, sha)).or_default().push(*s);
        }
        if let Some(d) = &r.info.outputs_sha256 {
            whole.entry(d).or_default().push(*s);
        }
    }
    // Per seed: the seeds it shares with, the files shared, and whether it is all of them.
    let mut shared: BTreeMap<u32, (BTreeSet<u32>, Vec<String>, bool)> = BTreeMap::new();
    for ((path, _), ss) in owners.iter().filter(|(_, ss)| ss.len() > 1) {
        for s in ss {
            let e = shared.entry(*s).or_default();
            e.0.extend(ss.iter().filter(|o| *o != s));
            e.1.push((*path).to_string());
        }
    }
    for ss in whole.values().filter(|ss| ss.len() > 1) {
        for s in ss {
            let e = shared.entry(*s).or_default();
            e.0.extend(ss.iter().filter(|o| *o != s));
            e.2 = true;
        }
    }
    for (s, (others, files, all)) in shared {
        let n = files.len();
        let mut named: Vec<String> = files.into_iter().take(3).collect();
        if n > named.len() {
            named.push(format!("and {} more", n - named.len()));
        }
        let e = format!(
            "{}: seed {s}'s solver outputs are seed(s) {others:?}'s, byte for byte: {} ({}): a \
             seed's run must be its own",
            bind::SEEDS_IDENTICAL,
            if all {
                "all of them".to_string()
            } else {
                format!("{n} file(s)")
            },
            named.join(", ")
        );
        seeds.insert(s, Err(e));
    }
}

/// Traces every transport the bed's cells need, one at a time (each takes every core), calling
/// `done` after each.
pub fn trace_transports(
    bed: &BedFile,
    transports: &mut super::transport::Transports,
    done: &dyn Fn(usize, usize, &str, f64),
) {
    let needed = super::check::needed_transports(bed);
    let n = needed.len();
    for (i, (room, alpha, air)) in needed.iter().enumerate() {
        let t0 = std::time::Instant::now();
        transports.get_or_trace(room, *alpha, *air, &bed.transport);
        done(
            i + 1,
            n,
            &format!("{} α {alpha} air {air:?}", room.name),
            t0.elapsed().as_secs_f64(),
        );
    }
}

/// The file system of the volume `path` is on (`exFAT`, `NTFS`, ...), when Windows says.
#[cfg(windows)]
pub fn filesystem_of(path: &Path) -> Option<String> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::{GetVolumeInformationW, GetVolumePathNameW};
    let wide: Vec<u16> = std::path::absolute(path)
        .ok()?
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let mut volume = [0u16; 1024];
    // SAFETY: `wide` is NUL-terminated and `volume` is a buffer of the length given.
    let ok = unsafe { GetVolumePathNameW(wide.as_ptr(), volume.as_mut_ptr(), volume.len() as u32) };
    if ok == 0 {
        return None;
    }
    let mut name = [0u16; 64];
    // SAFETY: `volume` is NUL-terminated by the call above; the other pointers are null (not
    // wanted) or a buffer of the length given.
    let ok = unsafe {
        GetVolumeInformationW(
            volume.as_ptr(),
            std::ptr::null_mut(),
            0,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            name.as_mut_ptr(),
            name.len() as u32,
        )
    };
    if ok == 0 {
        return None;
    }
    let end = name.iter().position(|c| *c == 0).unwrap_or(name.len());
    Some(String::from_utf16_lossy(&name[..end]))
}

#[cfg(not(windows))]
pub fn filesystem_of(_: &Path) -> Option<String> {
    None
}

/// Refuses `root` when it is on an exFAT volume and `files` would pass the 20,000 a bed may
/// leave there (`B:`: 128 KB clusters).
pub fn check_room_for(root: &Path, files: u64) -> Result<Option<String>, String> {
    let fs = filesystem_of(root);
    if fs
        .as_deref()
        .is_some_and(|f| f.eq_ignore_ascii_case("exFAT"))
        && files > limits::EXFAT_FILES
    {
        return Err(format!(
            "{} is on an exFAT volume, and the bed would leave about {files} files there, more \
             than the {} it may: put --out on another volume",
            root.display(),
            limits::EXFAT_FILES
        ));
    }
    Ok(fs)
}

/// Test helpers: reads made as [`read_folder`] makes them, without a run folder.
#[cfg(test)]
pub(crate) mod testing {
    use super::*;
    use crate::run::manifest::sha256_bytes;

    /// `info` as read from the files the bed writes for `p`, bound to `run.json`, with solver
    /// outputs of its own named by `tag` ([`outputs_of`]); `run.json` records `p`'s project.
    pub(crate) fn as_if_read_from(info: &mut read::RunInfo, p: &Planned, tag: &str) {
        let w = planned_inputs(p).unwrap();
        let mbin = Some(sha256_bytes(b"the mbin"));
        info.project_sha256 = Some(w.project_sha256.clone());
        info.on_disk = Some(read::OnDisk {
            project_sha256: Some(w.project_sha256),
            config_sha256: Some(w.config_sha256),
            scene_sha256: Some(w.scene_sha256),
            mbin_sha256: mbin.clone(),
            run_mesh_input_hash: Some(w.mesh_input_hash.clone()),
            run_mbin_sha256: mbin.clone(),
            mesh_json_input_hash: Some(w.mesh_input_hash),
            mesh_json_mbin_sha256: mbin,
            mesh_check: Some(vec![]),
        });
        outputs_of(info, tag);
    }

    /// `info`'s solver outputs, bound to `run.json`: two files of contents named by `tag`.
    pub(crate) fn outputs_of(info: &mut read::RunInfo, tag: &str) {
        let files: Vec<(String, String)> = ["Punctual receivers/R000/Sound level.recp", "x.gabe"]
            .iter()
            .map(|p| (p.to_string(), sha256_bytes(format!("{tag} {p}").as_bytes())))
            .collect();
        let digest: String = files.iter().map(|(p, h)| format!("{p}\t{h}\n")).collect();
        info.bound_by = Some("run.json".into());
        info.outputs_sha256 = Some(sha256_bytes(digest.as_bytes()));
        info.output_files = files;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use testing::as_if_read_from;

    #[test]
    fn the_plan_is_the_specs_412_spps_runs_and_21_tcr() {
        let bed = BedFile::m8a();
        let (runs, not_run) = plan(&bed, None).unwrap();
        assert!(not_run.is_some());
        let spps = runs.iter().filter(|p| p.solver == SolverKind::Spps).count();
        let tcr = runs.iter().filter(|p| p.solver == SolverKind::Tcr).count();
        // 40 cells × 10 seeds, N5, N6; the atmospheric validation's 10 and 1 need upstream.
        assert_eq!((spps, tcr), (402, 20));
        // Longest first: the largest random cell or an α 0.05 energetic one.
        assert!(runs[0].cost >= runs[1].cost && runs[1].cost >= runs[runs.len() - 1].cost);
        // Distinct folders.
        let dirs: std::collections::BTreeSet<PathBuf> = runs.iter().map(|p| p.key.dir()).collect();
        assert_eq!(dirs.len(), runs.len());
        // The full plan's projection is under the 20,000 files a bed may leave on exFAT.
        let files = projected_files(&bed, &runs) + 11 * FILES_PER_ATMOSPHERIC_RUN;
        assert!(files < limits::EXFAT_FILES, "{files}");
        // N5 is its cell at a thousandth of the particles, seed 10; N6 has walls of scattering 0.
        let n5 = runs
            .iter()
            .find(|p| p.key == RunKey::N5 { seed: 10 })
            .unwrap();
        assert_eq!(n5.project.solvers.spps.particles_per_source, 31_000);
        assert_eq!(n5.project.solvers.spps.random_seed, 10);
        let n6 = runs
            .iter()
            .find(|p| p.key == RunKey::N6 { seed: 1 })
            .unwrap();
        assert!(
            n6.project.materials[0]
                .scattering
                .iter()
                .all(|s| s.get() == 0.0)
        );
        let ext = plan_extension(&bed, &["6x10x3-a0.1-random-air-off".to_string()]).unwrap();
        assert_eq!(ext.len(), 10);
        assert!(
            ext.iter()
                .all(|p| matches!(p.key, RunKey::Cell { seed, .. } if seed > 10))
        );
    }

    fn info(project_sha256: Option<String>) -> read::RunInfo {
        read::RunInfo {
            folder: "runs/x/s1/run".into(),
            status: "OK".into(),
            reasons: vec![],
            exit_class: 0,
            solver_wall_s: None,
            solver_cpu_s: None,
            exe: "spps.exe".into(),
            exe_sha256: "e".into(),
            project_sha256,
            tetgen_sha256: None,
            files: 1,
            bytes: 1,
            bound_by: None,
            outputs_sha256: None,
            output_files: vec![],
            on_disk: None,
        }
    }

    /// What SPPS reports of a run of `bed`'s cell `id` at `seed`, `particles` and step `dt`, and
    /// the sha256 of the project it was run from, written literally (not from the plan); its
    /// files, as read, are the ones the bed writes for that project.
    fn spps_run(bed: &BedFile, id: &str, seed: u32, particles: u32, dt: f64) -> Read {
        let cell = bed.cell(id).unwrap();
        let mut spec = CellSpec::of(bed, cell).unwrap();
        spec.particles_per_source = particles;
        let bands = bed.bands_hz(cell.air);
        let project = file::cell_project(&spec, seed, dt, bands).unwrap();
        let mut info = info(None);
        let run_of = Planned {
            key: RunKey::Cell {
                id: id.into(),
                seed,
            },
            solver: SolverKind::Spps,
            project,
            receivers: None,
            cost: 0.0,
        };
        as_if_read_from(&mut info, &run_of, &format!("{id} {seed} {particles} {dt}"));
        Read::Spps(Box::new(SppsRead {
            info,
            bands_hz: bands.iter().map(|&f| f as i32).collect(),
            particles_per_source: particles,
            computation_method: match cell.method {
                file::Method::Random => 0,
                file::Method::Energetic => 1,
            },
            trans_epsilon: f64::from(cell.trans_epsilon as f32),
            time_step_s: f64::from(dt as f32),
            random_seed: Some(seed as i32),
            t30: vec![],
            reference: read::Reference {
                not_computed: Some("not read".into()),
                volume_m3: f64::NAN,
                area_m2: f64::NAN,
                speed_of_sound_m_s: f64::NAN,
                constant_s_per_m: f64::NAN,
                gamma2: None,
                gamma2_se: None,
                mean_free_path_m: None,
                bands: vec![],
            },
            curves: vec![],
        }))
    }

    fn spps_mut(r: &mut Read) -> &mut SppsRead {
        match r {
            Read::Spps(s) => s,
            Read::Tcr(_) => unreachable!(),
        }
    }

    /// The judge's finding 2: nothing compared a run read (with `--from`, or fresh) with the cell
    /// and seed it is filed under. Each difference refuses it, named, with `bed_run_not_planned`.
    #[test]
    fn a_run_at_10_ms_filed_under_a_1_ms_cell_is_refused() {
        let bed = BedFile::m8a();
        let (runs, _) = plan(&bed, None).unwrap();
        let id = "5x4x3-a0.2-energetic-air-on";
        let planned = |key: RunKey| runs.iter().find(|p| p.key == key).unwrap();
        let p = planned(RunKey::Cell {
            id: id.into(),
            seed: 3,
        });
        assert_eq!(p.project.solvers.spps.time_step_s.get(), 0.001);
        let particles = bed.cell(id).unwrap().particles_per_source;
        // The run the bed makes for the cell at seed 3: accepted.
        let good = spps_run(&bed, id, 3, particles, 0.001);
        assert_eq!(check_planned(p, &good), Ok(()));

        // The same cell and seed from a bed at 10 ms: its project and its step differ.
        let e = check_planned(p, &spps_run(&bed, id, 3, particles, 0.01)).unwrap_err();
        assert!(e.starts_with(RUN_NOT_PLANNED), "{e}");
        assert!(e.contains(&p.key.label()), "{e}");
        assert!(
            e.contains("time_step_s 0.009999999776482582, the bed's 0.0010000000474974513"),
            "{e}"
        );
        assert!(e.contains("its project has sha256"), "{e}");
        // The step alone (a project of the right bytes whose config said 10 ms): refused.
        let mut r = good.clone();
        spps_mut(&mut r).time_step_s = f64::from(0.01f32);
        let e = check_planned(p, &r).unwrap_err();
        assert!(e.contains("time_step_s") && !e.contains("sha256"), "{e}");
        // The project alone: refused.
        let mut r = good.clone();
        spps_mut(&mut r).info.project_sha256 = Some("0".repeat(64));
        assert!(
            check_planned(p, &r)
                .unwrap_err()
                .contains("its project has sha256")
        );
        // A fixture run, with no project: refused.
        let mut r = good.clone();
        spps_mut(&mut r).info.project_sha256 = None;
        assert!(
            check_planned(p, &r)
                .unwrap_err()
                .contains("records no project")
        );
        // Seed 4's run filed under seed 3: refused on its project and its seed.
        let e = check_planned(p, &spps_run(&bed, id, 4, particles, 0.001)).unwrap_err();
        assert!(e.contains("random_seed 4, the bed's 3"), "{e}");
        // The particles, the method, trans_epsilon, the bands: each refused, named.
        for (what, change) in [
            (
                "particles per source 1500001",
                (|s: &mut SppsRead| s.particles_per_source += 1) as fn(&mut SppsRead),
            ),
            ("computation_method 0", |s| s.computation_method = 0),
            ("trans_epsilon 7.0, the bed's 9.0", |s| {
                s.trans_epsilon = 7.0
            }),
            ("bands [125, 250, 500, 1000, 2000, 4000]", |s| {
                s.bands_hz.pop();
            }),
        ] {
            let mut r = good.clone();
            change(spps_mut(&mut r));
            let e = check_planned(p, &r).unwrap_err();
            assert!(e.contains(what), "{what}: {e}");
        }

        // The say-NO runs are held to their own projects. N5 is its cell's seed 10 at 31,000
        // particles: the cell's own seed-10 run filed as N5 is refused.
        let n5 = planned(RunKey::N5 { seed: 10 });
        let n5_cell = &bed.say_no.n5.cell;
        let full = bed.cell(n5_cell).unwrap().particles_per_source;
        assert_eq!(
            check_planned(n5, &spps_run(&bed, n5_cell, 10, 31_000, 0.001)),
            Ok(())
        );
        let e = check_planned(n5, &spps_run(&bed, n5_cell, 10, full, 0.001)).unwrap_err();
        assert!(
            e.contains("particles per source 31000000, the bed's 31000"),
            "{e}"
        );
        // N6's walls have scattering 0: a run of the matrix's walls under N6 is refused.
        let n6 = planned(RunKey::N6 { seed: 1 });
        let matrix = "5x4x3-a0.2-energetic-air-off";
        let r = spps_run(&bed, matrix, 1, 150_000, 0.001);
        let e = check_planned(n6, &r).unwrap_err();
        assert!(e.contains("its project has sha256"), "{e}");

        // A TCR run: its project and its bands.
        let t = planned(RunKey::Tcr {
            id: "5x4x3-a0.2-tcr-air-on".into(),
        });
        let tcr = |sha: Option<String>, bands: &[u32]| {
            let mut i = info(None);
            as_if_read_from(&mut i, t, "tcr");
            i.project_sha256 = sha;
            Read::Tcr(Box::new(TcrRead {
                info: i,
                bands: bands
                    .iter()
                    .map(|&f| read::TcrBand {
                        freq_hz: f as i32,
                        sabine_s: 1.0,
                        eyring_s: 1.0,
                        analytic_sabine_s: None,
                        analytic_eyring_s: None,
                        analytic_refused: None,
                        air_m_per_metre: None,
                    })
                    .collect(),
            }))
        };
        let sha = Some(planned_project_sha256(t));
        assert_eq!(
            check_planned(t, &tcr(sha.clone(), bed.bands_hz(true))),
            Ok(())
        );
        let e = check_planned(t, &tcr(sha, &bed.bands_hz(true)[..6])).unwrap_err();
        assert!(
            e.contains("bands [125, 250, 500, 1000, 2000, 4000], the bed's"),
            "{e}"
        );
        let e = check_planned(t, &tcr(Some("0".repeat(64)), bed.bands_hz(true))).unwrap_err();
        assert!(e.contains("its project has sha256"), "{e}");
        // An SPPS run where the bed runs TCR.
        assert!(
            check_planned(t, &good)
                .unwrap_err()
                .contains("a Spps run where")
        );
    }

    /// A TCR run of `p`'s project, as read from its own files.
    fn tcr_run_of(p: &Planned, bands: &[u32]) -> Read {
        let mut i = info(None);
        as_if_read_from(&mut i, p, "tcr");
        i.exe = "classicalTheory.exe".into();
        Read::Tcr(Box::new(TcrRead {
            info: i,
            bands: bands
                .iter()
                .map(|&f| read::TcrBand {
                    freq_hz: f as i32,
                    sabine_s: 1.0,
                    eyring_s: 1.0,
                    analytic_sabine_s: None,
                    analytic_eyring_s: None,
                    analytic_refused: None,
                    air_m_per_metre: None,
                })
                .collect(),
        }))
    }

    /// M8b, backlog row 42 (`VERIFY-adversarial.md` finding 2): TCR `5x4x3-a0.2-tcr-air-off`'s
    /// run with `run.json`'s sha256 edited to `5x4x3-a0.4-tcr-air-off`'s plan was accepted, and
    /// the bed passed. The files it was read from are still α 0.2's: `project.simpa` on disk and
    /// the `config.xml` the solver read are not the α 0.4 plan's, and it is refused.
    #[test]
    fn a_tcr_run_whose_run_json_names_another_plan_is_refused_on_its_files() {
        let bed = BedFile::m8a();
        let (runs, _) = plan(&bed, None).unwrap();
        let planned = |id: &str| {
            runs.iter()
                .find(|p| p.key == RunKey::Tcr { id: id.into() })
                .unwrap()
        };
        let (a02, a04) = (
            planned("5x4x3-a0.2-tcr-air-off"),
            planned("5x4x3-a0.4-tcr-air-off"),
        );
        let bands = bed.bands_hz(false);
        // Each run as the bed makes it is accepted under its own plan.
        assert_eq!(check_planned(a02, &tcr_run_of(a02, bands)), Ok(()));
        assert_eq!(check_planned(a04, &tcr_run_of(a04, bands)), Ok(()));
        // α 0.2's run, its run.json's sha256 edited to α 0.4's plan, filed as α 0.4's: refused,
        // on project.simpa as it is on disk and on the config.xml the solver read.
        let mut tampered = tcr_run_of(a02, bands);
        let Read::Tcr(t) = &mut tampered else {
            unreachable!()
        };
        t.info.project_sha256 = Some(planned_project_sha256(a04));
        let e = check_planned(a04, &tampered).unwrap_err();
        assert!(e.starts_with(RUN_NOT_PLANNED), "{e}");
        assert!(!e.contains("its project has sha256"), "{e}");
        assert!(e.contains("the sha256 of project.simpa on disk is"), "{e}");
        assert!(e.contains("the sha256 of its config.xml"), "{e}");
        // The mesh stamp and the scene are the same room's: α is in the config alone.
        assert!(
            !e.contains("mesh stamp") && !e.contains("scene mesh"),
            "{e}"
        );
    }

    /// M8b, backlog row 42: the files a run was read from, as hashed from disk, each held to what
    /// the bed writes for the plan. `run.json`'s sha256 left the plan's, each file changed alone
    /// is refused and named.
    #[test]
    fn a_run_whose_files_on_disk_are_not_the_plans_is_refused() {
        let bed = BedFile::m8a();
        let (runs, _) = plan(&bed, None).unwrap();
        let p = runs
            .iter()
            .find(|p| {
                p.key
                    == RunKey::Cell {
                        id: "5x4x3-a0.4-energetic-air-off".into(),
                        seed: 3,
                    }
            })
            .unwrap();
        let cell = bed.cell("5x4x3-a0.4-energetic-air-off").unwrap();
        let good = spps_run(&bed, &cell.id, 3, cell.particles_per_source, 0.001);
        assert_eq!(check_planned(p, &good), Ok(()));
        type Change = fn(&mut read::OnDisk);
        for (what, change) in [
            (
                "the sha256 of project.simpa on disk is 0000",
                (|d: &mut read::OnDisk| d.project_sha256 = Some("0".repeat(64))) as Change,
            ),
            ("the sha256 of project.simpa on disk is missing", |d| {
                d.project_sha256 = None
            }),
            ("the sha256 of its config.xml", |d| {
                d.config_sha256 = Some("0".repeat(64))
            }),
            ("the sha256 of its scene mesh", |d| {
                d.scene_sha256 = Some("0".repeat(64))
            }),
            ("the mesh stamp run.json records", |d| {
                d.run_mesh_input_hash = Some("0".repeat(32))
            }),
            ("the mesh stamp mesh.json records", |d| {
                d.mesh_json_input_hash = None
            }),
            ("its solve/tetramesh.mbin has sha256", |d| {
                d.mbin_sha256 = Some("0".repeat(64))
            }),
            ("its solve/tetramesh.mbin has sha256", |d| {
                d.mesh_json_mbin_sha256 = None
            }),
            ("does not hold to its scene mesh: mesh_mismatch", |d| {
                d.mesh_check = Some(vec!["mesh_mismatch: a hull face".into()])
            }),
            ("its mesh was not checked against its scene mesh", |d| {
                d.mesh_check = None
            }),
        ] {
            let mut r = good.clone();
            change(spps_mut(&mut r).info.on_disk.as_mut().unwrap());
            let e = check_planned(p, &r).unwrap_err();
            assert!(e.starts_with(RUN_NOT_PLANNED), "{what}: {e}");
            assert!(e.contains(what), "{what}: {e}");
            assert!(!e.contains("its project has sha256"), "{what}: {e}");
        }
    }

    /// M8b: a read that did not go through [`read_folder`]'s binding, or whose files were not
    /// hashed from disk, is never accepted.
    #[test]
    fn a_read_not_bound_or_not_hashed_from_disk_is_refused() {
        let bed = BedFile::m8a();
        let (runs, _) = plan(&bed, None).unwrap();
        let cell = bed.cell("5x4x3-a0.4-energetic-air-off").unwrap();
        let p = runs
            .iter()
            .find(|p| {
                p.key
                    == RunKey::Cell {
                        id: cell.id.clone(),
                        seed: 3,
                    }
            })
            .unwrap();
        let good = spps_run(&bed, &cell.id, 3, cell.particles_per_source, 0.001);
        let mut unbound = good.clone();
        spps_mut(&mut unbound).info.bound_by = None;
        let e = check_planned(p, &unbound).unwrap_err();
        assert!(e.contains("its files were bound to no record"), "{e}");
        let mut unhashed = good.clone();
        spps_mut(&mut unhashed).info.on_disk = None;
        let e = check_planned(p, &unhashed).unwrap_err();
        assert!(e.contains("nothing of it was hashed from disk"), "{e}");
    }

    #[test]
    fn par_map_keeps_order() {
        let v: Vec<u32> = (0..100).collect();
        assert_eq!(
            par_map(&v, 7, |x| x * 2),
            (0..100).map(|x| x * 2).collect::<Vec<_>>()
        );
        assert!(par_map(&Vec::<u32>::new(), 4, |x| *x).is_empty());
        // A fault set on the caller reaches the workers.
        #[cfg(feature = "fault-injection")]
        crate::faults::with(crate::faults::Fault::BedTransportAirOff, || {
            let seen = par_map(&v, 4, |_| crate::faults::active());
            assert!(
                seen.iter()
                    .all(|f| *f == Some(crate::faults::Fault::BedTransportAirOff))
            );
        });
    }
}
