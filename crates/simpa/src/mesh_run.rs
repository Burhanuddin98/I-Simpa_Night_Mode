//! `simpa mesh`, `simpa mesh-verify`, `simpa run` and `simpa run-folder`
//! (`docs/m5-m6-design.md`, "CLI surface"). Each is a thin layer over `simpa_core::mesh` and
//! `simpa_core::run::manager`: it parses the options, finds the executables, prints, and turns
//! the outcome into the exit code of the design's table (0, 2, 3, 4, 5, 130).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Duration;

use simpa_core::mesh::{
    self, Markers, MeshManifest, MeshStatus, MeshTools, Mesher, PreprocessProgram, TetgenMesher,
    verify,
};
use simpa_core::process::{CancelToken, Line};
use simpa_core::run::manager::{self, PREPROCESS_EXE_NAME, TETGEN_EXE_NAME, solver_exe_name};
use simpa_core::run::{
    CancelAfterLaunch, DEFAULT_LOSS_LIMIT, ExeSearch, ExitClass, MeshChoice, RunEvent, RunOptions,
    RunReport, Status,
};
use simpa_core::schema::{MeshSettings, SolverKind};
use simpa_core::{geometry, validate};

/// Parsed arguments: positionals, `--name value` options and `--switch`es.
struct Args<'a> {
    positional: Vec<&'a str>,
    values: BTreeMap<&'a str, &'a str>,
    switches: Vec<&'a str>,
}

impl<'a> Args<'a> {
    /// `with_value` names the options that take a value, `switches` those that do not; anything
    /// else starting with `--` is an error.
    fn parse(args: &[&'a str], with_value: &[&str], switches: &[&str]) -> Result<Self, String> {
        let mut out = Args {
            positional: Vec::new(),
            values: BTreeMap::new(),
            switches: Vec::new(),
        };
        let mut it = args.iter();
        while let Some(&a) = it.next() {
            match a.strip_prefix("--") {
                Some(name) if with_value.contains(&name) => match it.next() {
                    Some(&v) => {
                        if out.values.insert(name, v).is_some() {
                            return Err(format!("--{name} is given twice"));
                        }
                    }
                    None => return Err(format!("--{name} needs a value")),
                },
                Some(name) if switches.contains(&name) => out.switches.push(name),
                Some(_) => return Err(format!("unknown option '{a}'")),
                None => out.positional.push(a),
            }
        }
        Ok(out)
    }

    fn switch(&self, name: &str) -> bool {
        self.switches.contains(&name)
    }

    fn value(&self, name: &str) -> Option<&'a str> {
        self.values.get(name).copied()
    }

    fn path(&self, name: &str) -> Option<PathBuf> {
        self.value(name).map(PathBuf::from)
    }

    fn millis(&self, name: &str) -> Result<Option<u64>, String> {
        self.value(name)
            .map(|v| {
                v.parse::<u64>()
                    .map_err(|_| format!("--{name} must be a whole number of milliseconds"))
            })
            .transpose()
    }
}

fn usage_error(msg: &str) -> ExitCode {
    eprintln!("simpa: {msg}\n{}", super::USAGE);
    ExitCode::from(2)
}

fn exit(class: ExitClass) -> ExitCode {
    ExitCode::from(class.code())
}

fn solver_of(a: &Args) -> Result<SolverKind, String> {
    match a.value("solver") {
        Some("spps") => Ok(SolverKind::Spps),
        Some("tcr") => Ok(SolverKind::Tcr),
        Some(other) => Err(format!("--solver must be spps or tcr, not '{other}'")),
        None => Err("--solver spps|tcr is required".to_string()),
    }
}

/// `--loss-limit`: a number that is not NaN and not negative.
fn loss_limit(a: &Args) -> Result<f64, String> {
    match a.value("loss-limit") {
        None => Ok(DEFAULT_LOSS_LIMIT),
        Some(v) => match v.parse::<f64>() {
            Ok(x) if x >= 0.0 => Ok(x),
            _ => Err(format!(
                "--loss-limit must be a number at or above 0 (a fraction: 0.01 is 1 %), not '{v}'"
            )),
        },
    }
}

/// `--cancel-after-progress`: a percentage that is not NaN.
fn progress(a: &Args) -> Result<Option<f64>, String> {
    a.value("cancel-after-progress")
        .map(|v| match v.parse::<f64>() {
            Ok(x) if !x.is_nan() => Ok(x),
            _ => Err(format!(
                "--cancel-after-progress must be a percentage, not '{v}'"
            )),
        })
        .transpose()
}

/// An executable by the design's search: the explicit option, `$SIMPA_SOLVERS_DIR`, beside
/// `simpa.exe`, then the dev tree.
fn find_exe(explicit: Option<PathBuf>, name: &str) -> Result<PathBuf, String> {
    ExeSearch::from_env(explicit)
        .find(name)
        .map_err(|e| e.to_string())
}

// ---------------------------------------------------------------------------------------------
// simpa mesh

/// Rules a project must pass before it is meshed: the structural faults, and the settings
/// conflict the mesher would refuse anyway.
fn mesh_relevant(code: &str) -> bool {
    validate::STRUCTURAL_CODES.contains(&code) || code == validate::codes::MESH_SETTINGS_CONFLICT
}

/// `preprocess.exe` for a project whose settings ask for it: `--preprocess`, else the design's
/// search. Not found is `None`, said on stderr: the mesher then fails with
/// `preprocess_launch_failed`, in its manifest.
fn preprocess_program(
    a: &Args,
    project: &simpa_core::schema::Project,
) -> Option<PreprocessProgram> {
    if !project.solvers.meshing.preprocess {
        return None;
    }
    match find_exe(a.path("preprocess"), PREPROCESS_EXE_NAME) {
        Ok(exe) => Some(PreprocessProgram::new(exe)),
        Err(e) => {
            eprintln!("simpa: {e}");
            None
        }
    }
}

/// `mesh <project.simpa | file.poly> --out <dir> [--json] [--tetgen <exe>] [--preprocess <exe>]
/// [--parity] [--from-tetgen <dir> [--basename <b>]] [--cancel-after-ms <n>]`.
pub fn mesh_cmd(args: &[&str]) -> ExitCode {
    let a = match Args::parse(
        args,
        &[
            "out",
            "tetgen",
            "preprocess",
            "from-tetgen",
            "basename",
            "cancel-after-ms",
        ],
        &["json", "parity"],
    ) {
        Ok(a) => a,
        Err(e) => return usage_error(&e),
    };
    let ([input], Some(out)) = (a.positional.as_slice(), a.path("out")) else {
        return usage_error("mesh needs <project.simpa | file.poly> and --out <dir>");
    };
    let cancel_after = match a.millis("cancel-after-ms") {
        Ok(v) => v.map(Duration::from_millis),
        Err(e) => return usage_error(&e),
    };
    let input = Path::new(input);
    let is_poly = input
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("poly"));
    let cancel = CancelToken::new();
    let mut on_line = |l: &Line| eprintln!("{}", l.text);

    let manifest = if let Some(tg) = a.path("from-tetgen") {
        let project = match mesh_ready_project(input) {
            Ok(p) => p,
            Err(code) => return code,
        };
        mesh::mesh_from_tetgen(&project, &tg, a.value("basename"), &out)
    } else {
        let exe = match find_exe(a.path("tetgen"), TETGEN_EXE_NAME) {
            Ok(e) => e,
            Err(e) => return usage_error(&e),
        };
        let tetgen = TetgenMesher::new(exe);
        let timed;
        let mesher: &dyn Mesher = match cancel_after {
            Some(after) => {
                timed = CancelAfterLaunch {
                    inner: &tetgen,
                    after,
                };
                &timed
            }
            None => &tetgen,
        };
        if is_poly {
            mesh::mesh_poly(
                input,
                &MeshSettings::default(),
                &out,
                mesher,
                &cancel,
                &mut on_line,
            )
        } else {
            let project = match mesh_ready_project(input) {
                Ok(p) => p,
                Err(code) => return code,
            };
            let program = preprocess_program(&a, &project);
            let tools = MeshTools {
                tetgen: mesher,
                preprocess: program.as_ref().map(|p| p as &dyn Mesher),
                markers: if a.switch("parity") {
                    Markers::Parity
                } else {
                    Markers::Restored
                },
            };
            mesh::mesh_project_with(&project, &out, &tools, &cancel, &mut on_line)
        }
    };
    let m = match manifest {
        Ok(m) => m,
        Err(e) => {
            eprintln!("simpa: {e}");
            return exit(ExitClass::Mesh);
        }
    };
    print_mesh(&m, &out, a.switch("json"));
    exit(match m.status {
        MeshStatus::Ok => ExitClass::Ok,
        MeshStatus::Fail if m.codes.iter().any(|c| c == mesh::codes::GEOMETRY_REFUSED) => {
            ExitClass::Geometry
        }
        MeshStatus::Fail => ExitClass::Mesh,
        MeshStatus::Cancelled => ExitClass::Cancelled,
    })
}

/// The project in `path`, once it passes the geometry check (exit 3) and the mesh-relevant
/// validator rules (exit 2); the refusal is printed. A project meshed through upstream's scene
/// correction is checked after it instead, on what `preprocess.exe` saves (the mesher's
/// `geometry_refused`, exit 3 too).
fn mesh_ready_project(path: &Path) -> Result<simpa_core::schema::Project, ExitCode> {
    let project = validate::read_project(path).map_err(|e| {
        eprintln!("simpa: {}: {} ({e})", path.display(), e.code());
        ExitCode::from(2)
    })?;
    let report = geometry::check::check(&project.geometry);
    if report.verdict != geometry::check::Verdict::Ok && !project.solvers.meshing.preprocess {
        for r in &report.reasons {
            eprintln!("refused {}: {}", r.code.as_str(), r.message);
        }
        return Err(exit(ExitClass::Geometry));
    }
    let issues = validate::validate_with(&project, &validate::Context::for_project_file(path));
    let blocking: Vec<_> = issues
        .iter()
        .filter(|i| i.severity == validate::Severity::Error && mesh_relevant(i.code))
        .collect();
    if !blocking.is_empty() {
        for i in blocking {
            eprintln!("{i}");
        }
        return Err(exit(ExitClass::Usage));
    }
    Ok(project)
}

fn print_mesh(m: &MeshManifest, out: &Path, json: bool) {
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(m).expect("a mesh manifest serialises")
        );
        return;
    }
    let status = match m.status {
        MeshStatus::Ok => "OK",
        MeshStatus::Fail => "FAIL",
        MeshStatus::Cancelled => "CANCELLED",
    };
    let (tets, nodes) = m
        .counts
        .build
        .as_ref()
        .map_or((0, 0), |b| (b.tetrahedra, b.nodes));
    let codes = if m.codes.is_empty() {
        "-".to_string()
    } else {
        m.codes.join(", ")
    };
    println!(
        "{status} {codes}: {tets} tetrahedra, {nodes} nodes, {}",
        out.display()
    );
    for msg in &m.messages {
        println!("  {msg}");
    }
}

// ---------------------------------------------------------------------------------------------
// simpa mesh-verify

/// `mesh-verify <dir> [--json] [--room-id <n>] [--fittings <a,b,..>]`: exit 0 on a pass, 4 on a
/// failure.
pub fn mesh_verify_cmd(args: &[&str]) -> ExitCode {
    let a = match Args::parse(args, &["room-id", "fittings"], &["json"]) {
        Ok(a) => a,
        Err(e) => return usage_error(&e),
    };
    let [dir] = a.positional.as_slice() else {
        return usage_error("mesh-verify needs one folder");
    };
    let room = match a.value("room-id").map(str::parse::<i32>).transpose() {
        Ok(r) => r,
        Err(_) => return usage_error("--room-id must be an integer"),
    };
    let fittings: Result<Vec<i32>, _> = a
        .value("fittings")
        .map(|f| f.split(',').map(|x| x.trim().parse::<i32>()).collect())
        .unwrap_or(Ok(Vec::new()));
    let Ok(fittings) = fittings else {
        return usage_error("--fittings must be integers separated by commas");
    };
    // TetGen's numbering for the fittings (the room from one above the largest), unless the room's
    // first id is given.
    let mut ids = verify::VolumeIds::tetgen(fittings);
    if let Some(room) = room {
        ids.room = room;
    }
    let report = match verify::verify_dir(Path::new(dir), &ids) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("simpa: {dir}: {e}");
            return exit(ExitClass::Mesh);
        }
    };
    if a.switch("json") {
        println!(
            "{}",
            serde_json::to_string_pretty(&report).expect("a report serialises")
        );
    } else {
        let codes = if report.codes.is_empty() {
            "-".to_string()
        } else {
            report.codes.join(", ")
        };
        println!(
            "{} {codes}: {}",
            if report.passed() { "OK" } else { "FAIL" },
            report.dir.display()
        );
        if report.skipped_facets > 0 {
            println!(
                "  {} skipped facets, markers {:?}",
                report.skipped_facets, report.skipped_markers
            );
        }
        if let Some(m) = &report.mesh {
            println!(
                "  {} tetrahedra, {} nodes, {} scene faces",
                m.tetrahedra, m.nodes, m.scene_faces
            );
        }
    }
    exit(if report.passed() {
        ExitClass::Ok
    } else {
        ExitClass::Mesh
    })
}

// ---------------------------------------------------------------------------------------------
// simpa run and simpa run-folder

/// Prints a classified solver line to stderr as `CLASS  text`.
fn print_event(e: &RunEvent) {
    if let RunEvent::SolverLine(c) = e {
        eprintln!("{}  {}", c.class.as_str(), c.line.text);
    }
}

/// The run's manifest on stdout (`--json`), or one line naming the verdict and the run folder.
fn print_report(r: &RunReport, json: bool) -> ExitCode {
    let m = &r.manifest;
    if json {
        print!("{}", m.to_json());
    } else {
        let codes = m.verdict.codes();
        let status = match m.verdict.status {
            Status::Ok => "OK",
            Status::Fail => "FAIL",
            Status::Crash => "CRASH",
            Status::Cancelled => "CANCELLED",
        };
        println!(
            "{status} {} exit {}: {}",
            if codes.is_empty() {
                "-".to_string()
            } else {
                codes.join(", ")
            },
            m.exit_class.code(),
            r.dir.display()
        )
    }
    exit(m.exit_class)
}

/// The options `run` and `run-folder` share.
fn run_options(a: &Args, default_root: PathBuf) -> Result<RunOptions, String> {
    let solver = solver_of(a)?;
    Ok(RunOptions {
        solver,
        solver_exe: find_exe(a.path("solver-exe"), solver_exe_name(solver))?,
        runs_root: a.path("runs").unwrap_or(default_root),
        loss_limit: loss_limit(a)?,
        cancel_after_ms: a.millis("cancel-after-ms")?,
        cancel_after_progress: progress(a)?,
    })
}

const RUN_VALUES: [&str; 6] = [
    "solver",
    "runs",
    "solver-exe",
    "loss-limit",
    "cancel-after-ms",
    "cancel-after-progress",
];

/// `run <project> --solver spps|tcr [--variant <v>] [--mesh <dir>] [--runs <root>]
/// [--loss-limit <f>] [--cancel-after-ms <n>] [--cancel-after-progress <p>]
/// [--solver-exe <exe>] [--tetgen <exe>] [--json]`. The runs root defaults to `runs` beside the
/// project file.
pub fn run_cmd(args: &[&str]) -> ExitCode {
    let mut with_value = RUN_VALUES.to_vec();
    with_value.extend(["variant", "mesh", "tetgen", "preprocess"]);
    let a = match Args::parse(args, &with_value, &["json"]) {
        Ok(a) => a,
        Err(e) => return usage_error(&e),
    };
    let [project] = a.positional.as_slice() else {
        return usage_error("run needs one project file");
    };
    let project = Path::new(project);
    let default_root = project.parent().unwrap_or(Path::new(".")).join("runs");
    let opts = match run_options(&a, default_root) {
        Ok(o) => o,
        Err(e) => return usage_error(&e),
    };
    let mesh = match a.path("mesh") {
        Some(dir) => MeshChoice::Reuse(dir),
        None => match find_exe(a.path("tetgen"), TETGEN_EXE_NAME) {
            // Found whether or not the project asks for it: the run manager reads the project.
            Ok(tetgen) => MeshChoice::Build {
                tetgen,
                preprocess: find_exe(a.path("preprocess"), PREPROCESS_EXE_NAME).ok(),
            },
            Err(e) => return usage_error(&e),
        },
    };
    let report = manager::run_project(
        project,
        a.value("variant"),
        &mesh,
        &opts,
        &CancelToken::new(),
        &mut print_event,
    );
    match report {
        Ok(r) => print_report(&r, a.switch("json")),
        Err(e) => {
            eprintln!("simpa: {e}");
            exit(e.exit_class())
        }
    }
}

/// `run-folder <dir> --solver spps|tcr [--runs <root>] [--solver-exe <exe>] [--loss-limit <f>]
/// [--cancel-after-ms <n>] [--cancel-after-progress <p>] [--json]`. The runs root defaults to
/// `runs` in the current folder, never beside the fixture.
pub fn run_folder_cmd(args: &[&str]) -> ExitCode {
    let a = match Args::parse(args, &RUN_VALUES, &["json"]) {
        Ok(a) => a,
        Err(e) => return usage_error(&e),
    };
    let [folder] = a.positional.as_slice() else {
        return usage_error("run-folder needs one folder");
    };
    let opts = match run_options(&a, PathBuf::from("runs")) {
        Ok(o) => o,
        Err(e) => return usage_error(&e),
    };
    match manager::run_folder(
        Path::new(folder),
        &opts,
        &CancelToken::new(),
        &mut print_event,
    ) {
        Ok(r) => print_report(&r, a.switch("json")),
        Err(e) => {
            eprintln!("simpa: {e}");
            exit(e.exit_class())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn options_are_parsed_and_unknown_ones_refused() {
        let a = Args::parse(&["p", "--solver", "tcr", "--json"], &["solver"], &["json"]).unwrap();
        assert_eq!(a.positional, ["p"]);
        assert_eq!(a.value("solver"), Some("tcr"));
        assert!(a.switch("json"));
        assert!(Args::parse(&["--solver"], &["solver"], &[]).is_err());
        assert!(Args::parse(&["--nope"], &["solver"], &[]).is_err());
        assert!(Args::parse(&["--solver", "a", "--solver", "b"], &["solver"], &[]).is_err());
    }

    #[test]
    fn the_loss_limit_refuses_nan_and_negative_values() {
        let limit = |v: &str| {
            let args = ["--loss-limit", v];
            loss_limit(&Args::parse(&args, &["loss-limit"], &[]).unwrap())
        };
        assert_eq!(limit("0.05"), Ok(0.05));
        assert_eq!(limit("0"), Ok(0.0));
        assert_eq!(limit("inf"), Ok(f64::INFINITY));
        for bad in ["NaN", "nan", "-0.01", "-inf", "1%", ""] {
            assert!(limit(bad).is_err(), "{bad}");
        }
        let none = Args::parse(&[], &["loss-limit"], &[]).unwrap();
        assert_eq!(loss_limit(&none), Ok(DEFAULT_LOSS_LIMIT));
    }

    #[test]
    fn the_progress_limit_refuses_nan() {
        let p = |v: &str| {
            let args = ["--cancel-after-progress", v];
            progress(&Args::parse(&args, &["cancel-after-progress"], &[]).unwrap())
        };
        assert_eq!(p("1"), Ok(Some(1.0)));
        assert!(p("NaN").is_err());
        assert!(p("x").is_err());
    }
}
