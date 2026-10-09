//! Typed results of a verified run: milestone M7, piece B (`docs/results.md`).
//!
//! [`load`] reads a run folder that `simpa run` or `simpa run-folder` wrote and returns
//! [`RunResults`] only for a run that is OK and still verifies. It refuses, with a code of
//! [`codes`]:
//! - a folder with no `run.json`, or one that does not read as this core's manifest, or that
//!   says OK while its stage, exit or reasons say otherwise;
//! - a run whose verdict is FAIL, CRASH or CANCELLED;
//! - a run whose inputs in `solve/` no longer have the hashes the manifest recorded before launch;
//! - a run whose outputs, judged again now by the verdict's own output signals
//!   ([`crate::run::verdict::output_reasons`]), fail them: a file removed or emptied since, a
//!   statistics table spoiled, a NaN planted in a TCR table;
//! - a result file this module reads that does not decode, is not laid out as the solver writes
//!   it, or disagrees with its sibling; and a value in one that is NaN, infinite, or a negative
//!   energy.
//!
//! What is read, per solver, is in [`spps`] and [`tcr`]; [`report`] turns it into the JSON that
//! `simpa results --json` prints, with `core::params` per receiver and band. Point receivers are
//! enumerated by their folders (SPPS) or files (TCR), each matched to one `config.xml` label
//! exactly, so that a receiver named `Seat` is never confused with `Seat2`.
//!
//! **No number read here is shown to a user before M8's physics bed passes**
//! (`docs/rebuild-plan.md`, M12).

use std::fmt;
use std::path::{Path, PathBuf};

use schemars::JsonSchema;
use serde::Serialize;

use crate::run::expect::{self, Expectation};
use crate::run::manager::{ExitClass, SOLVE_DIR, Stage};
use crate::run::manifest::{self, MANIFEST_VERSION, RunManifest};
use crate::run::verdict::{self, Outputs, Reason, Status};
use crate::schema::SolverKind;

pub mod bed;
pub mod extra_bed;
pub mod reference;
pub mod report;
pub mod room;
pub mod spps;
pub mod tcr;

pub use report::{Report, checked_report, report};

/// The refusal codes. Each is a row of `docs/solver-contract.md`, Part B, "Result refusals"
/// (`tests/reason_codes_docs.rs`).
pub mod codes {
    /// The folder has no `run.json`.
    pub const MANIFEST_MISSING: &str = "results_manifest_missing";
    /// `run.json` does not read as this core's manifest, names another manifest version or solver
    /// commit, or says OK while its stage, exit or reasons contradict it.
    pub const MANIFEST_INVALID: &str = "results_manifest_invalid";
    /// The run's verdict is FAIL or CRASH.
    pub const RUN_FAILED: &str = "results_run_failed";
    /// The run was cancelled.
    pub const RUN_CANCELLED: &str = "results_run_cancelled";
    /// An input in `solve/` is missing or no longer has the sha256 the manifest recorded.
    pub const INPUTS_CHANGED: &str = "results_inputs_changed";
    /// The outputs, judged again now by the verdict's output signals, fail them.
    pub const OUTPUTS_INVALID: &str = "results_outputs_invalid";
    /// A result file read here does not decode, is not laid out as the solver writes it, or
    /// disagrees with its sibling.
    pub const FILE_INVALID: &str = "results_file_invalid";
    /// A value in a result file is NaN, infinite, or a negative energy.
    pub const VALUE_INVALID: &str = "results_value_invalid";

    /// Every code, in the order of the documentation table.
    pub const ALL: [&str; 8] = [
        MANIFEST_MISSING,
        MANIFEST_INVALID,
        RUN_FAILED,
        RUN_CANCELLED,
        INPUTS_CHANGED,
        OUTPUTS_INVALID,
        FILE_INVALID,
        VALUE_INVALID,
    ];
}

/// The CLI's exit code for a run whose verdict is FAIL, CRASH or CANCELLED: the solver run did not
/// succeed, which is the plan's 5 ("5 solver run", `docs/rebuild-plan-raw-2026-09-23.json`,
/// `plan.architecture`). A cancelled run is refused with 5 as well, not 130: 130 says that the
/// command itself was cancelled (`simpa run` exits 130 for the cancel), and `simpa results` was
/// not; it read a run that did not finish (`docs/results.md`, "Verified runs only").
pub const EXIT_RUN_NOT_OK: u8 = 5;
/// The CLI's exit code for a run whose results do not verify (the plan's 6, "result
/// verification").
pub const EXIT_UNVERIFIED: u8 = 6;

/// Why a run's results were refused.
#[derive(Clone, Debug, PartialEq, Serialize, JsonSchema)]
pub struct Refusal {
    /// One of [`codes::ALL`].
    pub code: String,
    /// What was found, in words.
    pub detail: String,
    /// The verdict's reasons, for a run that failed or was cancelled; the output signals' reasons
    /// judged now, for `results_outputs_invalid`; otherwise empty.
    pub reasons: Vec<Reason>,
}

impl Refusal {
    fn new(code: &str, detail: impl Into<String>) -> Self {
        Refusal {
            code: code.to_string(),
            detail: detail.into(),
            reasons: Vec::new(),
        }
    }

    /// The CLI's exit code: [`EXIT_RUN_NOT_OK`] for a failed or cancelled run, [`EXIT_UNVERIFIED`]
    /// for every other refusal.
    pub fn exit_code(&self) -> u8 {
        if self.code == codes::RUN_FAILED || self.code == codes::RUN_CANCELLED {
            EXIT_RUN_NOT_OK
        } else {
            EXIT_UNVERIFIED
        }
    }
}

impl fmt::Display for Refusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.code, self.detail)?;
        for r in &self.reasons {
            write!(f, "; {}: {}", r.code, r.detail)?;
        }
        Ok(())
    }
}

impl std::error::Error for Refusal {}

pub(crate) fn file_invalid(path: &str, what: impl fmt::Display) -> Refusal {
    Refusal::new(codes::FILE_INVALID, format!("{path}: {what}"))
}

pub(crate) fn value_invalid(path: &str, what: impl fmt::Display) -> Refusal {
    Refusal::new(codes::VALUE_INVALID, format!("{path}: {what}"))
}

/// The results of one verified run.
#[derive(Clone, Debug)]
pub struct RunResults {
    /// The run folder, as given.
    pub folder: PathBuf,
    /// Its `run.json`.
    pub manifest: RunManifest,
    /// What its `config.xml` asks for.
    pub expectation: Expectation,
    /// The bands the run computed, ascending: the config's requested bands, which an OK verdict
    /// has checked against the solver's own output.
    pub bands_hz: Vec<i32>,
    /// What the solver wrote, typed.
    pub data: SolverResults,
    /// The room from the run's own inputs (M12 P2): volume, area, DIN 18041 targets and the
    /// absorption by surface group, for either solver.
    pub room: room::Room,
}

/// The results of each solver.
#[derive(Clone, Debug)]
pub enum SolverResults {
    Spps(spps::SppsResults),
    Tcr(tcr::TcrResults),
}

impl RunResults {
    /// The solver's working folder, `<run>/solve`.
    pub fn solve_dir(&self) -> PathBuf {
        self.folder.join(SOLVE_DIR)
    }

    /// The SPPS results, when the run is SPPS's.
    pub fn spps(&self) -> Option<&spps::SppsResults> {
        match &self.data {
            SolverResults::Spps(s) => Some(s),
            SolverResults::Tcr(_) => None,
        }
    }

    /// The TCR results, when the run is TCR's.
    pub fn tcr(&self) -> Option<&tcr::TcrResults> {
        match &self.data {
            SolverResults::Tcr(t) => Some(t),
            SolverResults::Spps(_) => None,
        }
    }
}

/// Reads the manifest of `folder`: `results_manifest_missing` or `results_manifest_invalid`.
fn read_manifest(folder: &Path) -> Result<RunManifest, Refusal> {
    let path = folder.join(manifest::FILE_NAME);
    let bytes = match std::fs::read(&path) {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Err(Refusal::new(
                codes::MANIFEST_MISSING,
                format!(
                    "{} has no {}: not a run folder written by `simpa run` or `simpa run-folder`",
                    folder.display(),
                    manifest::FILE_NAME
                ),
            ));
        }
        Err(e) => {
            return Err(Refusal::new(
                codes::MANIFEST_INVALID,
                format!("{}: {e}", path.display()),
            ));
        }
    };
    let text = std::str::from_utf8(&bytes).map_err(|e| {
        Refusal::new(
            codes::MANIFEST_INVALID,
            format!("{}: not UTF-8 ({e})", path.display()),
        )
    })?;
    let m = RunManifest::from_json(text).map_err(|e| {
        Refusal::new(
            codes::MANIFEST_INVALID,
            format!("{}: not this core's run manifest ({e})", path.display()),
        )
    })?;
    if m.manifest_version != MANIFEST_VERSION {
        return Err(Refusal::new(
            codes::MANIFEST_INVALID,
            format!(
                "manifest version {}, this core reads {MANIFEST_VERSION}",
                m.manifest_version
            ),
        ));
    }
    if m.solver_commit != crate::SOLVER_COMMIT {
        return Err(Refusal::new(
            codes::MANIFEST_INVALID,
            format!(
                "the run's solvers are from upstream {}, this core reads the outputs of {}",
                m.solver_commit,
                crate::SOLVER_COMMIT
            ),
        ));
    }
    Ok(m)
}

/// A stage as `run.json` writes it.
fn stage_name(s: Stage) -> String {
    serde_json::to_value(s)
        .ok()
        .and_then(|v| v.as_str().map(str::to_string))
        .unwrap_or_else(|| format!("{s:?}"))
}

/// Refuses a run whose verdict is not OK, and an OK verdict its own manifest contradicts.
fn check_status(m: &RunManifest) -> Result<(), Refusal> {
    let reasons = m.verdict.reasons.clone();
    let codes_of = || {
        reasons
            .iter()
            .map(|r| r.code.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    };
    match m.verdict.status {
        Status::Fail | Status::Crash => {
            let status = if m.verdict.status == Status::Fail {
                "FAIL"
            } else {
                "CRASH"
            };
            return Err(Refusal {
                code: codes::RUN_FAILED.into(),
                detail: format!(
                    "the run's verdict is {status} at stage {}: {}",
                    stage_name(m.stage),
                    codes_of()
                ),
                reasons,
            });
        }
        Status::Cancelled => {
            return Err(Refusal {
                code: codes::RUN_CANCELLED.into(),
                detail: format!(
                    "the run was cancelled at stage {}; its outputs are partial",
                    stage_name(m.stage)
                ),
                reasons,
            });
        }
        Status::Ok => {}
    }
    let mut wrong = Vec::new();
    if !reasons.is_empty() {
        wrong.push(format!("it lists reasons ({})", codes_of()));
    }
    if m.stage != Stage::Solve {
        wrong.push(format!("its stage is {}, not solve", stage_name(m.stage)));
    }
    if m.exit_class != ExitClass::Ok {
        wrong.push(format!("its exit class is {}", m.exit_class.code()));
    }
    match &m.outcome {
        None => wrong.push("it records no solver outcome".into()),
        Some(o) if o.cancelled || o.exit_code != Some(0) => wrong.push(format!(
            "its solver outcome is exit {:?}, cancelled {}",
            o.exit_code, o.cancelled
        )),
        Some(_) => {}
    }
    if !m.argv.iter().eq([crate::run::manager::SOLVER_ARGUMENT]) {
        wrong.push(format!("its solver arguments are {:?}", m.argv));
    }
    if wrong.is_empty() {
        Ok(())
    } else {
        Err(Refusal::new(
            codes::MANIFEST_INVALID,
            format!("the manifest says OK but {}", wrong.join("; ")),
        ))
    }
}

/// Every input the manifest recorded must still be in `solve/` with the same sha256.
fn check_inputs(m: &RunManifest, solve: &Path) -> Result<(), Refusal> {
    if !m
        .inputs
        .iter()
        .any(|f| f.path == crate::config_xml::names::CONFIG)
    {
        return Err(Refusal::new(
            codes::MANIFEST_INVALID,
            "the manifest's inputs do not include config.xml",
        ));
    }
    let mut changed = Vec::new();
    for f in &m.inputs {
        match manifest::sha256_file(&solve.join(&f.path)) {
            Ok(h) if h == f.sha256 => {}
            Ok(h) => changed.push(format!(
                "{} has sha256 {}, the manifest recorded {}",
                f.path,
                &h[..12],
                &f.sha256[..f.sha256.len().min(12)]
            )),
            Err(e) => changed.push(format!("{}: {e}", f.path)),
        }
    }
    if changed.is_empty() {
        Ok(())
    } else {
        Err(Refusal::new(
            codes::INPUTS_CHANGED,
            format!(
                "{} of {} inputs changed since the run: {}",
                changed.len(),
                m.inputs.len(),
                changed.join("; ")
            ),
        ))
    }
}

/// Reads the results of the run in `folder` (`<run>/run.json` beside `<run>/solve/`), or refuses
/// them (module docs).
pub fn load(folder: &Path) -> Result<RunResults, Refusal> {
    let m = read_manifest(folder)?;
    check_status(&m)?;
    let solve = folder.join(SOLVE_DIR);
    check_inputs(&m, &solve)?;

    let exp = Expectation::read(&solve, m.solver);
    let outputs = match &exp {
        Ok(e) => Outputs::read(&solve, e),
        Err(_) => Outputs::default(),
    };
    let reasons = verdict::output_reasons(m.solver, exp.as_ref(), &outputs, m.loss_limit);
    if !reasons.is_empty() {
        return Err(Refusal {
            code: codes::OUTPUTS_INVALID.into(),
            detail: format!(
                "the outputs no longer pass the verdict's output signals: {}",
                reasons
                    .iter()
                    .map(|r| r.code.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            reasons,
        });
    }
    // output_reasons refuses an unreadable config.xml (config_attribute_missing) and SPPS
    // statistics that do not read (stats_unreadable), so neither refusal below is reached; they
    // stay refusals, not panics.
    let exp = exp.map_err(|e| Refusal::new(codes::OUTPUTS_INVALID, e.to_string()))?;
    let bands_hz = exp.requested_bands();
    let data = match m.solver {
        SolverKind::Spps => {
            let Some(Ok(stats)) = outputs.stats else {
                return Err(Refusal::new(
                    codes::OUTPUTS_INVALID,
                    "the SPPS statistics were not read",
                ));
            };
            SolverResults::Spps(spps::read(&solve, &exp, stats)?)
        }
        SolverKind::Tcr => SolverResults::Tcr(tcr::read(&solve, &exp)?),
    };
    let room = room::room(&solve, &exp);
    Ok(RunResults {
        folder: folder.to_path_buf(),
        manifest: m,
        expectation: exp,
        bands_hz,
        data,
        room,
    })
}

/// The codes of a run whose solver build is not verified (backlog 38). "Unverified" marks a run;
/// it does not refuse it, so none of these is a [`Refusal`] code and none changes an exit code.
pub mod build_codes {
    /// The manifest records no solver check: `solvers` is absent (a CLI or bed run, or one
    /// written before M11) or an empty list.
    pub const UNRECORDED: &str = "solver_build_unrecorded";
    /// A recorded check does not match the verified build (`solvers/manifest.json`).
    pub const MISMATCH: &str = "solver_build_mismatch";
    /// Checks are recorded, but none covers the solver the run executed.
    pub const UNCHECKED: &str = "solver_build_unchecked";
    /// The checks were made against `$SIMPA_SOLVER_MANIFEST`'s override
    /// (`run.json`'s `solver_manifest.source`), not the embedded verified build
    /// (`solvers/manifest.json`): a test-only lever that lets a run proceed against a stand-in
    /// (`crates/simpa/src/mesh_run.rs::solver_manifest`), never a way to read verified — a
    /// manifest holding the running executable's own hash would otherwise match trivially.
    pub const OVERRIDE: &str = "solver_manifest_override";

    /// Every code.
    pub const ALL: [&str; 4] = [UNRECORDED, MISMATCH, UNCHECKED, OVERRIDE];
}

/// Whether a run's solver build was verified, decided from its manifest alone (backlog 38,
/// `docs/investigations/2026-09-30-b38-39/PLAN.md` C5). Computed beside [`load`], never by it:
/// `load`'s Ok or Err does not depend on it (C1).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum SolverBuild {
    /// `solvers` is present and non-empty, every check matches, and one covers the solver the
    /// run executed.
    Verified,
    /// Anything else, with a reason whose code is one of [`build_codes::ALL`].
    Unverified { reason: Reason },
}

impl SolverBuild {
    /// Whether the build was verified.
    pub fn is_verified(&self) -> bool {
        matches!(self, SolverBuild::Verified)
    }

    /// The reason, when the build was not verified.
    pub fn reason(&self) -> Option<&Reason> {
        match self {
            SolverBuild::Verified => None,
            SolverBuild::Unverified { reason } => Some(reason),
        }
    }
}

/// The one predicate for "solver build verified" (PLAN C5): the app's Results verdict, the Runs
/// row's solver status and `simpa results`' printed verdict all use it. Checks are matched by
/// name, as `check_solvers` names them ([`solver_exe_name`], `tetgen.exe`, `preprocess.exe`):
/// - no `solvers` record, or an empty one: [`build_codes::UNRECORDED`];
/// - `solver_manifest.source` is `override` (M8b): [`build_codes::OVERRIDE`], whatever the checks
///   say — a manifest made of `$SIMPA_SOLVER_MANIFEST`'s file can hold the running executable's
///   own hash, so "every check matches" proves nothing about the verified build;
/// - a check that does not match: [`build_codes::MISMATCH`];
/// - no check names the solver the run executed: [`build_codes::UNCHECKED`];
/// - otherwise verified.
///
/// [`solver_exe_name`]: crate::run::manager::solver_exe_name
pub fn solver_build(m: &RunManifest) -> SolverBuild {
    let unverified = |code: &str, detail: String| SolverBuild::Unverified {
        reason: Reason::new(code, detail),
    };
    let checks = match m.solvers.as_deref() {
        None => {
            return unverified(
                build_codes::UNRECORDED,
                "run.json records no check of the executables against the verified build \
                 (solvers/manifest.json): a command-line or bed run, or one made before the app \
                 checked them"
                    .into(),
            );
        }
        Some([]) => {
            return unverified(
                build_codes::UNRECORDED,
                "run.json's check of the executables lists none".into(),
            );
        }
        Some(c) => c,
    };
    if m.solver_manifest
        .as_ref()
        .is_some_and(|sm| sm.source == crate::bed::pe::ManifestSource::Override)
    {
        return unverified(
            build_codes::OVERRIDE,
            "the checks were made against $SIMPA_SOLVER_MANIFEST's override, not the embedded \
             verified build (solvers/manifest.json): a test run, never verified"
                .into(),
        );
    }
    let names = |pick: fn(&crate::bed::pe::SolverCheck) -> bool| {
        let picked: Vec<&str> = checks
            .iter()
            .filter(|c| pick(c))
            .map(|c| c.name.as_str())
            .collect();
        picked.join(", ")
    };
    let bad = names(|c| !c.matches);
    if !bad.is_empty() {
        return unverified(
            build_codes::MISMATCH,
            format!("not the verified build (solvers/manifest.json): {bad}"),
        );
    }
    let solver = crate::run::manager::run_exe_name(m.solver, m.gpu_device.is_some());
    if !checks.iter().any(|c| c.name == solver) {
        return unverified(
            build_codes::UNCHECKED,
            format!(
                "no check covers {solver}, the solver the run executed; checked: {}",
                names(|_| true)
            ),
        );
    }
    SolverBuild::Verified
}

/// A path under `solve/` as the solver joins it, `/`-separated (the key of [`Outputs::files`]).
pub(crate) fn key(path: &str) -> String {
    expect::normalize(path)
}

/// The band a column or folder label names: `125 Hz` is 125.
pub(crate) fn band_of(label: &[u8]) -> Option<i32> {
    crate::run::stats::band_label(label)
}

/// A surface-receiver or cutting-plane file, read, checked and summarised. The records themselves
/// are NOT kept: a run's maps can be gigabytes (CR4 at 27 bands with a 0.1 m plane: 28 files,
/// 6.7 GB, 700 million records), and holding them killed the app on the Results step
/// (2026-10-06 17:07 and 17:17). A map is read again from its file when it is drawn
/// (`csbin::read_file`), one file at a time.
#[derive(Clone, Debug)]
pub struct SurfaceFile {
    /// Relative to `solve/`, `/`-separated.
    pub path: String,
    /// TCR's field (`Direct field`, `Total field (Sabine)`, `Total field (Eyring)`); `None` for
    /// SPPS.
    pub field: Option<String>,
    /// The band, or `None` for the `Global` file of all bands together.
    pub band_hz: Option<i32>,
    /// Whether it is the cutting-plane file (`recepteurss_cut_filename`), not the receiver file.
    pub cutting_plane: bool,
    /// The header's `recordType`.
    pub record_type: crate::formats::csbin::RecordType,
    /// The header's `nbTimeStep`: the bins the maps are stored on.
    pub time_step_count: u32,
    /// The header's `timeStep`, seconds (the sound-map time bin, patch 0001; the particle step
    /// when unset).
    pub time_step: f32,
    /// `quantNodes`: the node positions the faces index.
    pub nodes: usize,
    /// Every receiver in the file, summarised.
    pub receivers: Vec<SurfaceReceiverStats>,
}

/// One receiver of a surface file, as [`SurfaceFile`] keeps it: counts and the sum, not the records.
#[derive(Clone, Debug, PartialEq)]
pub struct SurfaceReceiverStats {
    /// `xmlIndex`: its id in `config.xml`.
    pub id: i32,
    pub name: String,
    pub faces: usize,
    /// Records over every face.
    pub records: usize,
    /// The sum of every stored value, in `f64`.
    pub value_sum: f64,
}

/// `recepteur_surfacique_coupe@id` of each cutting plane in `solve/config.xml`, read as the solver
/// reads it (`atoi`).
fn cutting_plane_ids(solve: &Path) -> Result<Vec<i32>, Refusal> {
    let config = crate::config_xml::names::CONFIG;
    let text = std::fs::read(solve.join(config)).map_err(|e| file_invalid(config, e))?;
    let text = text.strip_prefix(b"\xef\xbb\xbf").unwrap_or(&text);
    let text = std::str::from_utf8(text).map_err(|e| file_invalid(config, e))?;
    let doc = roxmltree::Document::parse(text).map_err(|e| file_invalid(config, e))?;
    Ok(expect::child(doc.root_element(), "recepteurss")
        .map(|rs| {
            rs.children()
                .filter(|c| c.is_element() && c.tag_name().name() == "recepteur_surfacique_coupe")
                .map(|c| expect::atoi(c.attribute("id").unwrap_or("")))
                .collect()
        })
        .unwrap_or_default())
}

/// Decodes every `.csbin` of `paths` (relative to `solve`), refusing one that does not decode or
/// holds a value that is not finite. `fields` are the TCR prefixes to recognise in a path.
///
/// **Surface receivers and cutting planes are kept apart by name** (the plan's `core::results`):
/// a file named `recepteurss_cut_filename` is the cutting planes', any other the scene surface
/// receivers'. Each file must hold only receivers of its kind, told by their `xmlIndex`, the id
/// `config.xml` gives each `recepteur_surfacique` and `recepteur_surfacique_coupe`
/// (`results_file_invalid` otherwise): a cutting plane in the receivers' file, or the reverse, is
/// refused, not read as the other.
pub(crate) fn read_surfaces(
    solve: &Path,
    paths: &[String],
    exp: &Expectation,
    fields: &[String],
) -> Result<Vec<SurfaceFile>, Refusal> {
    let cut_name = key(&exp.names.recepteurss_cut_filename);
    let cut_ids = if paths.is_empty() {
        Vec::new()
    } else {
        cutting_plane_ids(solve)?
    };
    let mut out = Vec::with_capacity(paths.len());
    for p in paths {
        let data =
            crate::formats::csbin::read_file(&solve.join(p)).map_err(|e| file_invalid(p, e))?;
        let s = verdict::SurfaceValues::of(&data);
        if let Some(first) = s.first_nonfinite {
            return Err(value_invalid(
                p,
                format!(
                    "{} of {} values are not finite, first {first}",
                    s.nonfinite, s.values
                ),
            ));
        }
        let parts: Vec<&str> = p.split('/').collect();
        let folder = parts.len().checked_sub(2).map(|i| parts[i]).unwrap_or("");
        let band_hz = band_of(folder.as_bytes());
        if band_hz.is_none() && folder != expect::fixed::GLOBAL {
            return Err(file_invalid(
                p,
                "its folder names neither a band nor Global",
            ));
        }
        let field = fields
            .iter()
            .find(|f| !f.is_empty() && p.starts_with(&key(f)))
            .map(|f| key(f));
        let cutting_plane = p.ends_with(&cut_name) && !cut_name.is_empty();
        let (own, kind) = if cutting_plane {
            (&cut_ids, "recepteur_surfacique_coupe")
        } else {
            (&exp.surface_receivers, "recepteur_surfacique")
        };
        if let Some(r) = data.receivers.iter().find(|r| !own.contains(&r.xml_index)) {
            return Err(file_invalid(
                p,
                format!(
                    "it holds receiver {} ({:?}), which is no {kind} of config.xml (their ids \
                     are {own:?})",
                    r.xml_index,
                    r.name_lossy()
                ),
            ));
        }
        // Summarise and drop the records: the next file's decode reuses the memory.
        let receivers = data
            .receivers
            .iter()
            .map(|r| SurfaceReceiverStats {
                id: r.xml_index,
                name: r.name_lossy().into_owned(),
                faces: r.faces.len(),
                records: r.faces.iter().map(|f| f.records.len()).sum(),
                value_sum: r
                    .faces
                    .iter()
                    .flat_map(|f| f.records.iter())
                    .map(|v| f64::from(v.energy))
                    .sum(),
            })
            .collect();
        out.push(SurfaceFile {
            path: p.clone(),
            field,
            band_hz,
            cutting_plane,
            record_type: data.record_type,
            time_step_count: data.time_step_count,
            time_step: data.time_step,
            nodes: data.nodes.len(),
            receivers,
        });
        drop(data);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_code_has_its_exit() {
        for c in codes::ALL {
            let r = Refusal::new(c, "x");
            let want = if c == codes::RUN_FAILED || c == codes::RUN_CANCELLED {
                5
            } else {
                6
            };
            assert_eq!(r.exit_code(), want, "{c}");
            assert!(r.to_string().starts_with(c));
        }
    }
}
