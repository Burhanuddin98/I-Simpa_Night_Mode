//! `simpa bed <bed.json> --out <root> [--jobs <n>] [--from <earlier> [--seal <file>]]
//! [--upstream <dir>] [--json]`,
//! `simpa bed --schema` and `simpa bed --canonical`: M8a's T30 physics bed
//! (`docs/investigations/2026-09-29-m8a/SPEC.md`, section 6), over `simpa_core::bed`.
//!
//! 1. The bed file is read; any file that is not M8a's matrix runs as exploratory and never
//!    passes (E7).
//! 2. E1 before anything runs: each of the four executables (found as `simpa run` finds them) has
//!    the code sha256 `solvers/manifest.json` lists. Any other refuses the bed, exit 2, and nothing
//!    is written.
//! 3. Every run goes through the run manager, `--jobs` at once (default 4), longest first, under
//!    `<root>/<UTC stamp>/runs/`, and is read as it ends; or, with `--from`, an earlier bed's
//!    folder is read again with this build and no solver runs. Every run's files are first bound
//!    to their record (`bed::bind`): `run.json`'s output hashes, or for an earlier bed whose runs
//!    predate them, its committed seal given with `--seal`. A run bound to neither is refused.
//! 4. The transport runs in its own phase, never beside the solvers (it takes every core).
//! 5. The checks; gate C's one extension (seeds 11 to 20) of an INCONCLUSIVE cell is run and
//!    judged again.
//! 6. `report.json`, `summary.json` and `decays/<cell>.csv` are written beside `runs/`. The plots
//!    are `tools/bed/m8a_plots.py`'s.
//!
//! Exit codes: 0 only when `report.pass` is true; 8 the bed ran and did not pass; 2 usage, an
//! invalid bed file, or solvers that are not the verified build; 5 a run was not OK.

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::{Instant, SystemTime};

use simpa_core::bed::check::Reads;
use simpa_core::bed::run::{self, Exes, Planned, Read};
use simpa_core::bed::transport::Transports;
use simpa_core::bed::{self, file, output, pe, report};
use simpa_core::run::clock;

const EXIT_NOT_PASSED: u8 = 8;
const EXIT_RUN_NOT_OK: u8 = 5;

fn usage(msg: &str) -> ExitCode {
    eprintln!(
        "simpa: {msg}\nusage: simpa bed <bed.json> --out <root> [--jobs <n>] [--from <earlier> \
         [--seal <file>]] [--upstream <dir>] [--json]\n       simpa bed --schema | --canonical"
    );
    ExitCode::from(2)
}

/// The local clock time, `HH:MM:SS`, for progress lines.
fn now() -> String {
    let (c, _) = clock::local(SystemTime::now());
    format!("{:02}:{:02}:{:02}", c.hour, c.minute, c.second)
}

/// `yyyyMMddTHHmmssZ`, UTC.
fn utc_stamp(t: SystemTime) -> String {
    let c = clock::utc(t);
    format!(
        "{:04}{:02}{:02}T{:02}{:02}{:02}Z",
        c.year, c.month, c.day, c.hour, c.minute, c.second
    )
}

fn utc_rfc3339(t: SystemTime) -> String {
    let c = clock::utc(t);
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        c.year, c.month, c.day, c.hour, c.minute, c.second
    )
}

/// `git rev-parse HEAD` and whether `git status --porcelain` prints anything, in `dir`.
fn git(dir: &Path) -> (Option<String>, Option<bool>) {
    let run = |args: &[&str]| {
        std::process::Command::new("git")
            .arg("-C")
            .arg(dir)
            .args(args)
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
    };
    let head = run(&["rev-parse", "HEAD"]);
    let dirty = run(&["status", "--porcelain"]).map(|s| !s.is_empty());
    (head, dirty)
}

struct Options<'a> {
    bed: &'a str,
    out: PathBuf,
    jobs: usize,
    from: Option<PathBuf>,
    seal: Option<PathBuf>,
    upstream: Option<PathBuf>,
    json: bool,
}

fn parse<'a>(args: &[&'a str]) -> Result<Options<'a>, String> {
    let (mut bed, mut out, mut jobs, mut from, mut seal, mut upstream, mut json) =
        (None, None, 4usize, None, None, None, false);
    let mut it = args.iter();
    while let Some(&a) = it.next() {
        let mut value = |name: &str| {
            it.next()
                .copied()
                .ok_or_else(|| format!("{name} needs a value"))
        };
        match a {
            "--out" => out = Some(PathBuf::from(value("--out")?)),
            "--jobs" => {
                let v = value("--jobs")?;
                jobs = v.parse().ok().filter(|n| *n >= 1).ok_or(format!(
                    "--jobs must be a whole number of at least 1, not '{v}'"
                ))?;
            }
            "--from" => from = Some(PathBuf::from(value("--from")?)),
            "--seal" => seal = Some(PathBuf::from(value("--seal")?)),
            "--upstream" => upstream = Some(PathBuf::from(value("--upstream")?)),
            "--json" => json = true,
            _ if a.starts_with("--") => return Err(format!("unknown option '{a}'")),
            _ if bed.is_none() => bed = Some(a),
            _ => return Err(format!("unexpected argument '{a}'")),
        }
    }
    if seal.is_some() && from.is_none() {
        return Err("--seal needs --from: a seal holds an earlier bed's runs".into());
    }
    Ok(Options {
        bed: bed.ok_or("bed needs a bed file")?,
        out: out.ok_or("bed needs --out <root>: put it off B: (exFAT)")?,
        jobs,
        from,
        seal,
        upstream: upstream.or_else(|| {
            std::env::var_os("SIMPA_UPSTREAM")
                .filter(|v| !v.is_empty())
                .map(PathBuf::from)
        }),
        json,
    })
}

pub fn bed_cmd(args: &[&str]) -> ExitCode {
    match args {
        ["--schema"] => {
            println!(
                "{}",
                serde_json::to_string_pretty(&report::schema()).expect("a schema serialises")
            );
            return ExitCode::SUCCESS;
        }
        ["--canonical"] => {
            print!("{}", file::canonical_text(&file::BedFile::m8a()));
            return ExitCode::SUCCESS;
        }
        _ => {}
    }
    let o = match parse(args) {
        Ok(o) => o,
        Err(e) => return usage(&e),
    };
    let started = SystemTime::now();
    let clock0 = Instant::now();
    let loaded = match file::load(Path::new(o.bed)) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("simpa: bed refused: invalid bed file: {e}");
            return ExitCode::from(2);
        }
    };
    for why in &loaded.exploratory {
        eprintln!("simpa: bed: EXPLORATORY, the report cannot pass: {why}");
    }
    let bed = &loaded.bed;

    // E1 before anything runs.
    let exes = match Exes::find() {
        Ok(e) => e,
        Err(e) => {
            eprintln!("simpa: bed refused: {e}");
            return ExitCode::from(2);
        }
    };
    let manifest = match pe::SolverManifest::parse(bed::SOLVER_MANIFEST) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("simpa: bed refused: solvers/manifest.json: {e}");
            return ExitCode::from(2);
        }
    };
    let solvers = pe::check_solvers(&exes.list(), &manifest);
    for s in &solvers {
        eprintln!(
            "E1 {} {}: code sha256 {} ({})",
            if s.matches { "ok  " } else { "FAIL" },
            s.name,
            s.code_sha256.as_deref().unwrap_or("-"),
            s.path
        );
    }
    if let Some(bad) = solvers.iter().find(|s| !s.matches) {
        eprintln!(
            "simpa: bed refused before any run (E1): {} is not the verified build: {}",
            bad.path,
            bad.detail.as_deref().unwrap_or("")
        );
        return ExitCode::from(2);
    }

    // The folder, the plan, the room for it.
    let stamp = o.out.join(utc_stamp(started));
    if let Some(from) = &o.from
        && !from.join("runs").is_dir()
    {
        return usage(&format!("--from {}: no runs/ folder there", from.display()));
    }
    // The earlier bed's seal, when its runs predate the output hashes in run.json.
    let seal = match (&o.seal, &o.from) {
        (Some(path), Some(from)) => {
            match bed::bind::Seal::load(path)
                .and_then(|(s, sha)| s.for_bed(from).map(|()| (s, sha)))
            {
                Ok(s) => Some(s),
                Err(e) => {
                    eprintln!("simpa: bed refused: --seal {e}");
                    return ExitCode::from(2);
                }
            }
        }
        _ => None,
    };
    let (runs, atmospheric_not_run) = match run::plan(bed, o.upstream.as_deref()) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("simpa: bed refused: {e}");
            return ExitCode::from(2);
        }
    };
    let projected = if o.from.is_some() {
        0
    } else {
        run::projected_files(bed, &runs)
    };
    if let Err(e) = std::fs::create_dir_all(&stamp) {
        eprintln!("simpa: bed refused: {}: {e}", stamp.display());
        return ExitCode::from(2);
    }
    let filesystem = match run::check_room_for(&stamp, projected) {
        Ok(fs) => fs,
        Err(e) => {
            // The folder is empty: nothing else is left behind.
            let _ = std::fs::remove_dir(&stamp);
            eprintln!("simpa: bed refused: {e}");
            return ExitCode::from(2);
        }
    };
    if let Some(why) = &atmospheric_not_run {
        eprintln!("simpa: bed: {why}");
    }
    eprintln!(
        "[{}] bed {}: {} runs, {} at once, into {} ({}; about {projected} files)",
        now(),
        o.bed,
        runs.len(),
        o.jobs,
        stamp.display(),
        filesystem.as_deref().unwrap_or("file system unknown")
    );

    // The solver phase.
    let solver_clock = Instant::now();
    let sealed = seal.as_ref().map(|(s, _)| s);
    let results = run_or_read(&runs, &o, &stamp, &exes, sealed);
    let mut reads = Reads::default();
    run::into_reads(&mut reads, &runs, results, atmospheric_not_run);
    let solver_phase_s = solver_clock.elapsed().as_secs_f64();

    // The transport phase, alone.
    let transport_clock = Instant::now();
    let mut transports = Transports::default();
    run::trace_transports(bed, &mut transports, &|i, n, what, secs| {
        eprintln!("[{}] transport {i}/{n} {what}: {secs:.0} s", now());
    });
    let transport_phase_s = transport_clock.elapsed().as_secs_f64();

    let judge = |reads: &Reads, transports: &Transports| {
        report::evaluate(&report::Inputs {
            bed,
            reads,
            transports,
            exploratory: &loaded.exploratory,
            solvers: &solvers,
        })
    };
    let mut rep = judge(&reads, &transports);

    // Gate C's one extension.
    if !rep.needs_extension.is_empty() {
        eprintln!(
            "[{}] C is INCONCLUSIVE on seeds {:?} in {}: extending by seeds {:?}",
            now(),
            bed.seeds,
            rep.needs_extension.join(", "),
            bed.extension_seeds
        );
        match run::plan_extension(bed, &rep.needs_extension) {
            Ok(ext) => {
                let ext: Vec<Planned> = match &o.from {
                    // Only the extension runs the earlier bed made.
                    Some(from) => ext
                        .into_iter()
                        .filter(|p| from.join(p.key.dir()).is_dir())
                        .collect(),
                    None => ext,
                };
                let room = if o.from.is_none() {
                    let (files, _) = bed::read::count_files(&stamp);
                    run::check_room_for(&stamp, files + run::projected_files(bed, &ext)).map(|_| ())
                } else {
                    Ok(())
                };
                match room {
                    Ok(()) if !ext.is_empty() => {
                        let results = run_or_read(&ext, &o, &stamp, &exes, sealed);
                        run::into_reads(&mut reads, &ext, results, None);
                        rep = judge(&reads, &transports);
                    }
                    Ok(()) => eprintln!("simpa: bed: the extension's runs are not there"),
                    Err(e) => eprintln!("simpa: bed: extension not run: {e}"),
                }
            }
            Err(e) => eprintln!("simpa: bed: extension not planned: {e}"),
        }
    }

    // The outputs.
    let decays = stamp.join("decays");
    let written = std::fs::create_dir_all(&decays).and_then(|()| {
        for cell in &bed.cells {
            output::write_cell(&decays, bed, cell, &reads, &transports)?;
        }
        output::write_atmospheric(&decays, &reads).map(|_| ())
    });
    if let Err(e) = written {
        eprintln!("simpa: bed: decay files: {e}");
    }
    let (files, bytes) = bed::read::count_files(&stamp);
    let bed_dir = Path::new(o.bed)
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    // The bed file's work tree, else the current folder's.
    let (git_commit, git_dirty) = match git(bed_dir) {
        (None, _) => git(Path::new(".")),
        found => found,
    };
    let finished = SystemTime::now();
    rep.meta = report::Meta {
        bed_file: o.bed.to_string(),
        bed_raw_sha256: loaded.raw_sha256.clone(),
        bed_canonical_sha256: loaded.canonical_sha256.clone(),
        spec: bed.spec.clone(),
        git_commit,
        git_dirty,
        simpa_core_version: simpa_core::VERSION.into(),
        solver_commit: simpa_core::SOLVER_COMMIT.into(),
        solver_manifest_sha256: bed::solver_manifest_sha256(),
        machine: std::env::var("COMPUTERNAME").ok(),
        logical_cpus: std::thread::available_parallelism().map_or(1, |n| n.get()),
        started_utc: utc_rfc3339(started),
        finished_utc: utc_rfc3339(finished),
        wall_s: clock0.elapsed().as_secs_f64(),
        root: stamp.display().to_string(),
        from: o.from.as_ref().map(|p| p.display().to_string()),
        jobs: o.jobs,
        solver_phase_s: Some(solver_phase_s),
        transport_phase_s: Some(transport_phase_s),
        limits: report::limits_map(),
        seal: o.seal.as_ref().map(|p| p.display().to_string()),
        seal_sha256: seal.as_ref().map(|(_, sha)| sha.clone()),
    };
    rep.files = report::Files {
        projected,
        files,
        bytes,
        filesystem,
    };
    let text = serde_json::to_string_pretty(&rep).expect("a report serialises") + "\n";
    let report_path = stamp.join("report.json");
    if let Err(e) = std::fs::write(&report_path, &text) {
        eprintln!("simpa: bed: {}: {e}", report_path.display());
        return ExitCode::from(EXIT_NOT_PASSED);
    }
    let summary = report::summary(&rep, &pe::sha256_hex(text.as_bytes()));
    let summary_text = serde_json::to_string_pretty(&summary).expect("a summary serialises") + "\n";
    if let Err(e) = std::fs::write(stamp.join("summary.json"), &summary_text) {
        eprintln!("simpa: bed: summary.json: {e}");
    }

    for f in &rep.failures {
        eprintln!("FAIL  {f}");
    }
    let verdicts = |gated: bool| {
        let mut m: std::collections::BTreeMap<String, usize> = Default::default();
        for c in rep.cells.iter().filter(|c| c.gated == gated) {
            *m.entry(format!("{:?}", c.verdict)).or_default() += 1;
        }
        m
    };
    eprintln!(
        "[{}] bed {}: gated cells {:?}, reported cells {:?}, {} failures, {files} files, {:.1} MB; \
         {}",
        now(),
        if rep.pass { "PASS" } else { "NOT PASSED" },
        verdicts(true),
        verdicts(false),
        rep.failures.len(),
        bytes as f64 / 1e6,
        report_path.display()
    );
    if o.json {
        print!("{summary_text}");
    } else {
        println!(
            "{} {}",
            if rep.pass { "PASS" } else { "NOT PASSED" },
            report_path.display()
        );
    }
    if !rep.preconditions.e2.holds {
        ExitCode::from(EXIT_RUN_NOT_OK)
    } else if rep.pass {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(EXIT_NOT_PASSED)
    }
}

/// Runs `runs` (or reads them from `--from`, held to `seal` when there is one), printing a line
/// as each ends.
fn run_or_read(
    runs: &[Planned],
    o: &Options,
    stamp: &Path,
    exes: &Exes,
    seal: Option<&bed::bind::Seal>,
) -> Vec<Result<Read, String>> {
    match &o.from {
        Some(from) => {
            eprintln!(
                "[{}] reading {} runs from {}",
                now(),
                runs.len(),
                from.display()
            );
            run::read_existing(runs, from, o.jobs, seal)
        }
        None => {
            let total = runs.len();
            run::execute(runs, stamp, exes, o.jobs, &|n, p, r, secs| {
                let what = match r {
                    Ok(Read::Spps(s)) => format!(
                        "{} {:.0} s solver",
                        s.info.status,
                        s.info.solver_wall_s.unwrap_or(f64::NAN)
                    ),
                    Ok(Read::Tcr(t)) => t.info.status.clone(),
                    Err(e) => format!("NOT READ: {e}"),
                };
                eprintln!(
                    "[{}] {n}/{total} {}: {what}, {secs:.0} s",
                    now(),
                    p.key.label()
                );
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `--seal` holds an earlier bed's runs: it is taken with `--from`, and refused without it.
    #[test]
    fn a_seal_is_taken_only_with_from() {
        let o = parse(&["m8a.json", "--out", "o", "--from", "f", "--seal", "s.json"]).unwrap();
        assert_eq!(o.seal.as_deref(), Some(Path::new("s.json")));
        assert_eq!(o.from.as_deref(), Some(Path::new("f")));
        let o = parse(&["m8a.json", "--out", "o", "--from", "f"]).unwrap();
        assert!(o.seal.is_none());
        let e = parse(&["m8a.json", "--out", "o", "--seal", "s.json"])
            .err()
            .unwrap();
        assert!(e.contains("--seal needs --from"), "{e}");
        assert!(parse(&["m8a.json", "--out", "o", "--from", "f", "--seal"]).is_err());
    }
}
