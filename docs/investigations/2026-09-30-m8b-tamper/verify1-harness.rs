//! M8b tamper, adversarial check round 1 (`docs/investigations/2026-09-30-m8b-tamper/
//! VERIFY-adversarial-1.md`), on m8b-tamper at dd5c683 (code 239c6f5), with this build.
//!
//! V1. The M8a bed, read only, through `run::read_existing` with the committed seal: every run bound
//!     and matched to its plan; re-judged (transports from its report.json) to report.json's verdicts.
//! R.  Every earlier tampered copy (`C:\tmp\nm-judge-m8b\tamper`), copied fresh: with the seal,
//!     without it, and with a forged record (run.json's `outputs` written for the tampered files).
//! M.  The earlier in-memory and real-folder cases of VERIFY-adversarial.md (M8b step 1).
//! A.  New attempts on the sealed bed, editing only the bed folder.
//! U.  A bed read without a seal, where run.json is the only record (a new bed, or a renamed copy):
//!     forged records.
//! X.  Gate C's extension seeds when a seal is given.
//! V2. Fresh runs of this build, bound by their own run.json.
//!
//! Every write goes through `guard`, which refuses any path outside C:\tmp\nm-judge-tamper\adv1.
//! Deletes nothing.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

use simpa_core::bed::bind::{self, Seal};
use simpa_core::bed::check::{Reads, Verdict};
use simpa_core::bed::file;
use simpa_core::bed::read as bread;
use simpa_core::bed::report::{self, Report};
use simpa_core::bed::run::{self, Planned, Read, RunKey};
use simpa_core::bed::transport::{self, Transports};
use simpa_core::run::manifest::{RunSource, hash_tree, sha256_bytes, sha256_file};

const ROOT: &str = r"C:\tmp\nm-m8a-bed\20260929T093134Z";
const WT: &str = r"B:\repos\I-Simpa_Night_Mode\.claude\worktrees\m8b-tamper";
const UPSTREAM: &str = r"B:\repos\I-Simpa-upstream";
const OLD_TAMPER: &str = r"C:\tmp\nm-judge-m8b\tamper";
const BED10: &str = r"C:\tmp\nm-judge-m8b\bed10";
const SMOKE: &str = r"C:\tmp\nm-m8a-smoke\20260929T072041Z";
const FRESH: &str = r"C:\tmp\nm-judge-tamper\fresh";
const ADV: &str = r"C:\tmp\nm-judge-tamper\adv1";
const ID: &str = "5x4x3-a0.4-energetic-air-off";

// ------------------------------------------------------------------------------------------
// Writing, only under ADV.

fn guard(p: &Path) {
    let s = p.to_string_lossy().to_lowercase().replace('/', "\\");
    assert!(
        s.starts_with(&ADV.to_lowercase()),
        "refusing to write outside {ADV}: {}",
        p.display()
    );
}

fn cases_root() -> PathBuf {
    std::env::var_os("CASES")
        .map(PathBuf::from)
        .unwrap_or_else(|| Path::new(ADV).join("cases"))
}

/// A case folder made by this run: refused if it is there already.
fn fresh(name: &str) -> PathBuf {
    let d = cases_root().join(name);
    guard(&d);
    assert!(!d.exists(), "{} exists: give a new CASES root", d.display());
    d
}

fn copy_dir_except(src: &Path, dst: &Path, skip: &dyn Fn(&Path) -> bool) {
    guard(dst);
    std::fs::create_dir_all(dst).unwrap();
    for e in std::fs::read_dir(src).unwrap().flatten() {
        let (s, d) = (e.path(), dst.join(e.file_name()));
        if skip(&s) {
            continue;
        }
        let t = e.file_type().unwrap();
        if t.is_dir() {
            copy_dir_except(&s, &d, skip);
        } else if t.is_file() {
            std::fs::copy(&s, &d).unwrap();
        } else {
            panic!("neither a file nor a folder: {}", s.display());
        }
    }
}

fn copy_dir(src: &Path, dst: &Path) {
    copy_dir_except(src, dst, &|_| false);
}

fn write(p: &Path, bytes: &[u8]) {
    guard(p);
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(p, bytes).unwrap();
}

fn mklink(args: &[&str], link: &Path, target: &Path) -> Result<(), String> {
    guard(link);
    let mut a: Vec<String> = vec!["/c".into(), "mklink".into()];
    a.extend(args.iter().map(|s| s.to_string()));
    a.push(link.display().to_string());
    a.push(target.display().to_string());
    let o = Command::new("cmd").args(&a).output().map_err(|e| e.to_string())?;
    if o.status.success() {
        Ok(())
    } else {
        Err(format!(
            "mklink {args:?} failed: {} {}",
            String::from_utf8_lossy(&o.stdout).trim(),
            String::from_utf8_lossy(&o.stderr).trim()
        ))
    }
}

// ------------------------------------------------------------------------------------------
// Run folders.

fn cell(id: &str, seed: u32) -> RunKey {
    RunKey::Cell {
        id: id.into(),
        seed,
    }
}

fn tcr(id: &str) -> RunKey {
    RunKey::Tcr { id: id.into() }
}

fn run_folder(root: &Path, key: &RunKey) -> PathBuf {
    bread::find_run_folder(&root.join(key.dir()))
        .unwrap_or_else(|| panic!("no run folder under {}", root.join(key.dir()).display()))
}

/// Copies the slot of `key` under `root` (project.simpa and the run folder) to `case` as `as_key`.
fn slot(root: &Path, key: &RunKey, case: &Path, as_key: &RunKey) -> PathBuf {
    let dst = case.join(as_key.dir());
    copy_dir(&root.join(key.dir()), &dst);
    bread::find_run_folder(&dst).unwrap()
}

fn inputs_of(folder: &Path) -> Vec<String> {
    bread::manifest_of(folder)
        .unwrap()
        .inputs
        .iter()
        .map(|i| i.path.clone())
        .collect()
}

/// The solver's outputs of a run folder: files under solve/ that are not run.json's inputs,
/// path under solve/ to sha256.
fn outputs(folder: &Path) -> BTreeMap<String, String> {
    let ins = inputs_of(folder);
    hash_tree(&folder.join("solve"), &[])
        .unwrap()
        .into_iter()
        .filter(|f| !ins.contains(&f.path))
        .map(|f| (f.path, f.sha256))
        .collect()
}

/// Overwrites `run`'s outputs with `other`'s, at the same paths.
fn swap_outputs(run: &Path, other: &Path) -> usize {
    let mut n = 0;
    for rel in outputs(other).keys() {
        let d = run.join("solve").join(rel);
        guard(&d);
        std::fs::create_dir_all(d.parent().unwrap()).unwrap();
        std::fs::copy(other.join("solve").join(rel), &d).unwrap();
        n += 1;
    }
    n
}

/// Sets byte 17 of a gabe file: the first of the three padding bytes after the read-only flag,
/// which `formats::gabe::read_prefix` skips unchecked. The file reads as the same table and is
/// not the same bytes.
fn pad(path: &Path, v: u8) {
    guard(path);
    let mut b = std::fs::read(path).unwrap();
    assert!(
        b.len() > 20 && b[0..4] == [2, 0, 0, 0],
        "{} is not a gabe",
        path.display()
    );
    assert!(v != 0);
    b[17] ^= v;
    std::fs::write(path, b).unwrap();
}

fn pad_outputs(folder: &Path, v: u8) -> usize {
    let o = outputs(folder);
    for rel in o.keys() {
        pad(&folder.join("solve").join(rel), v);
    }
    o.len()
}

/// run.json's `outputs` written by this build's own code for the folder as it now stands: a
/// forged record.
fn forge_record(folder: &Path) {
    guard(folder);
    let mut m = bread::manifest_of(folder).unwrap();
    m.outputs = Some(hash_tree(folder, &["run.json"]).unwrap());
    write(&folder.join("run.json"), m.to_json().as_bytes());
}

fn set_source_sha(folder: &Path, sha: &str) {
    let mut m = bread::manifest_of(folder).unwrap();
    match &mut m.source {
        RunSource::Project { sha256, .. } => *sha256 = sha.into(),
        RunSource::Fixture { .. } => panic!("fixture"),
    }
    write(&folder.join("run.json"), m.to_json().as_bytes());
}

fn file_sha(p: &Path) -> String {
    sha256_file(p).unwrap()
}

// ------------------------------------------------------------------------------------------
// Reading and judging.

fn short(s: &str, n: usize) -> String {
    let v: String = s.chars().take(n).collect();
    if s.chars().count() > n {
        format!("{v}…")
    } else {
        v
    }
}

fn info(r: &Read) -> &bread::RunInfo {
    match r {
        Read::Spps(s) => &s.info,
        Read::Tcr(t) => &t.info,
    }
}

fn clear(r: &mut Result<Read, String>) {
    if let Ok(Read::Spps(s)) = r {
        s.curves.clear();
    }
}

/// The run as `simpa bed --from` reads it, then `check_planned`, in words.
fn verdict(p: &Planned, r: &Result<Read, String>) -> String {
    match r {
        Err(e) => format!("REFUSED at read: {}", short(e, 520)),
        Ok(read) => match run::check_planned(p, read) {
            Err(e) => format!("read, then REFUSED by check_planned: {}", short(&e, 520)),
            Ok(()) => format!(
                "ACCEPTED (bound by {}, outputs {})",
                info(read).bound_by.as_deref().unwrap_or("-"),
                short(info(read).outputs_sha256.as_deref().unwrap_or("-"), 12)
            ),
        },
    }
}

/// The refusal's code, or ACCEPTED.
fn code(p: &Planned, r: &Result<Read, String>) -> String {
    match r {
        Err(e) => e.split(':').next().unwrap_or("").to_string(),
        Ok(read) => match run::check_planned(p, read) {
            Err(e) => format!("check_planned {}", e.split(':').next().unwrap_or("")),
            Ok(()) => "ACCEPTED".into(),
        },
    }
}

struct Ctx<'a> {
    l: &'a file::Loaded,
    runs: &'a [Planned],
    base: Vec<Result<Read, String>>,
    ts: Transports,
    rep: &'a Report,
    not_run: Option<String>,
}

impl Ctx<'_> {
    fn idx(&self, key: &RunKey) -> usize {
        self.runs
            .iter()
            .position(|p| &p.key == key)
            .unwrap_or_else(|| panic!("{key:?}"))
    }
    fn planned(&self, key: &RunKey) -> &Planned {
        &self.runs[self.idx(key)]
    }
    fn base_read(&self, key: &RunKey) -> Read {
        self.base[self.idx(key)].clone().unwrap()
    }
    fn reads(&self, results: Vec<Result<Read, String>>) -> Reads {
        let mut reads = Reads::default();
        run::into_reads(&mut reads, self.runs, results, self.not_run.clone());
        reads
    }
    fn evaluate(&self, reads: &Reads) -> Report {
        report::evaluate(&report::Inputs {
            bed: &self.l.bed,
            reads,
            transports: &self.ts,
            exploratory: &self.l.exploratory,
            solvers: &self.rep.preconditions.e1.solvers,
        })
    }
    fn judge_with(&self, subst: Vec<(RunKey, Result<Read, String>)>) -> Report {
        let mut results = self.base.clone();
        for (k, r) in subst {
            let i = self.idx(&k);
            results[i] = r;
        }
        self.evaluate(&self.reads(results))
    }
    /// The slot `root/<as_key.dir()>` read as `as_key`, as `simpa bed --from` reads it.
    fn read(&self, root: &Path, key: &RunKey, seal: Option<&Seal>) -> Result<Read, String> {
        let mut r = run::read_in(&root.join(key.dir()), self.planned(key), seal);
        clear(&mut r);
        r
    }
    /// The folder `dir` (a slot) read as `key`.
    fn read_dir_as(&self, dir: &Path, key: &RunKey, seal: Option<&Seal>) -> Result<Read, String> {
        let mut r = run::read_in(dir, self.planned(key), seal);
        clear(&mut r);
        r
    }
    /// Reads `keys` under `root` 8 at once, as `read_existing` does.
    fn read_many(&self, root: &Path, keys: &[RunKey], seal: Option<&Seal>) -> Vec<(RunKey, Result<Read, String>)> {
        let ps: Vec<Planned> = keys.iter().map(|k| self.planned(k).clone()).collect();
        let mut rs = run::read_existing(&ps, root, 8, seal);
        rs.iter_mut().for_each(clear);
        keys.iter().cloned().zip(rs).collect()
    }
}

fn verdict_of(r: &Report, id: &str) -> String {
    r.cells
        .iter()
        .find(|c| c.id == id)
        .map(|c| {
            format!(
                "cell {:?} (A {:?}, B {:?}, C {:?}{})",
                c.verdict,
                c.a.as_ref().map(|x| x.verdict),
                c.b.as_ref().map(|x| x.verdict),
                c.c.as_ref().map(|x| x.verdict),
                c.c.as_ref()
                    .map(|x| format!(
                        " d {:+.4} % ± {:.4} %",
                        100.0 * x.interval.mean,
                        100.0 * x.interval.t * x.interval.se
                    ))
                    .unwrap_or_default()
            )
        })
        .or_else(|| {
            r.tcr.iter().find(|t| t.id == id).map(|t| {
                format!(
                    "TCR {:?} (D {:?})",
                    t.verdict,
                    t.d.as_ref().map(|d| d.verdict)
                )
            })
        })
        .unwrap_or_else(|| "-".into())
}

fn line(name: &str, r: &Report, id: &str) {
    let f: Vec<String> = r
        .failures
        .iter()
        .filter(|f| f.contains(id))
        .take(2)
        .map(|f| short(f, 300))
        .collect();
    println!(
        "    {name}: pass {} | E2 {} | failures {} | {id}: {} | {}",
        r.pass,
        r.preconditions.e2.holds,
        r.failures.len(),
        verdict_of(r, id),
        if f.is_empty() { "-".to_string() } else { f.join(" || ") }
    );
}

fn seeds_line(r: &Report, id: &str) {
    if let Some(c) = r.cells.iter().find(|c| c.id == id) {
        let errs: Vec<String> = c
            .seeds
            .iter()
            .filter_map(|s| s.error.as_ref().map(|e| format!("s{} {}", s.seed, e.split(':').next().unwrap_or(""))))
            .collect();
        println!("      seed errors: {}", if errs.is_empty() { "none".into() } else { errs.join(", ") });
    }
}

fn counts(v: impl Iterator<Item = String>) -> BTreeMap<String, usize> {
    let mut m = BTreeMap::new();
    for s in v {
        *m.entry(s).or_default() += 1;
    }
    m
}

fn t30_eq(a: &Read, b: &Read) -> bool {
    match (a, b) {
        (Read::Spps(a), Read::Spps(b)) => a.t30 == b.t30,
        (Read::Tcr(a), Read::Tcr(b)) => a.bands == b.bands,
        _ => false,
    }
}

// ------------------------------------------------------------------------------------------

fn main() {
    let jobs: usize = std::env::var("JOBS").ok().and_then(|s| s.parse().ok()).unwrap_or(8);
    let t_all = Instant::now();
    let root = PathBuf::from(ROOT);
    let wt = PathBuf::from(WT);
    println!("cases under {}", cases_root().display());
    let l = file::load(&wt.join("beds").join("m8a.json")).unwrap();
    let rep_value =
        simpa_core::schema::parse_json(&std::fs::read_to_string(root.join("report.json")).unwrap())
            .unwrap();
    let rep: Report = serde_json::from_value(rep_value).unwrap();
    let seal_path = wt.join(r"beds\m8a-20260929T093134Z\outputs-seal.json");
    let (seal, seal_sha) = Seal::load(&seal_path).unwrap();
    seal.for_bed(&root).unwrap();
    println!(
        "seal: sha256 {seal_sha}, bed {}, {} runs, {} files, {} bytes",
        seal.bed,
        seal.runs.len(),
        seal.files,
        seal.bytes
    );
    let (runs, not_run) = run::plan(&l.bed, Some(Path::new(UPSTREAM))).unwrap();
    println!("planned {} runs; atmospheric not run: {not_run:?}", runs.len());
    let mut ts = Transports::default();
    for x in &rep.transports {
        ts.by_key
            .insert(transport::key(&x.room, x.alpha, x.air_m_per_metre), Ok(x.clone()));
    }

    // ======================================================================================
    println!("\n== V1. The M8a bed, read only, with the committed seal ({jobs} at once)");
    let t0 = Instant::now();
    let mut base = run::read_existing(&runs, &root, jobs, Some(&seal));
    base.iter_mut().for_each(clear);
    println!("  read {} runs in {:.0} s", runs.len(), t0.elapsed().as_secs_f64());
    let by = counts(base.iter().map(|r| match r {
        Ok(read) => format!("bound by {}", info(read).bound_by.as_deref().unwrap_or("-")),
        Err(_) => "not read".into(),
    }));
    let planned = counts(runs.iter().zip(&base).map(|(p, r)| code(p, r)));
    println!("  {by:?}; check_planned: {planned:?}");
    for (p, r) in runs.iter().zip(&base) {
        if code(p, r) != "ACCEPTED" {
            println!("    {}: {}", p.key.label(), verdict(p, r));
        }
    }
    let kinds = counts(runs.iter().map(|p| match &p.key {
        RunKey::Cell { .. } => "cell".to_string(),
        RunKey::Tcr { .. } => "tcr".into(),
        RunKey::AtmosphericSpps { .. } => "atmospheric spps".into(),
        RunKey::AtmosphericTcr => "atmospheric tcr".into(),
        RunKey::N5 { .. } => "n5".into(),
        RunKey::N6 { .. } => "n6".into(),
    }));
    println!("  planned runs by kind: {kinds:?}");
    let ctx = Ctx {
        l: &l,
        runs: &runs,
        base,
        ts,
        rep: &rep,
        not_run: not_run.clone(),
    };
    let base_reads = ctx.reads(ctx.base.clone());
    let seed_refusals: usize = base_reads
        .spps
        .values()
        .flat_map(|m| m.values())
        .chain(base_reads.atmospheric.spps.values())
        .filter(|r| r.as_ref().err().is_some_and(|e| e.starts_with(bind::SEEDS_IDENTICAL)))
        .count();
    let judged = ctx.evaluate(&base_reads);
    println!(
        "  re-judged: pass {}, {} failures, E1 {} ({} runs), E2 {}, needs_extension {:?}, seeds refused {}: {seed_refusals}",
        judged.pass,
        judged.failures.len(),
        judged.preconditions.e1.holds,
        judged.preconditions.e1.runs_checked,
        judged.preconditions.e2.holds,
        judged.needs_extension,
        bind::SEEDS_IDENTICAL
    );
    let mut same = (0, 0, 0, 0, 0);
    for c in &judged.cells {
        let o = rep.cells.iter().find(|x| x.id == c.id).unwrap();
        same.0 += (c.verdict == o.verdict) as usize;
        same.1 += (c.seeds.len() == o.seeds.len()
            && c.seeds.iter().zip(&o.seeds).all(|(a, b)| a.t30 == b.t30 && a.error == b.error))
            as usize;
    }
    for t in &judged.tcr {
        let o = rep.tcr.iter().find(|x| x.id == t.id).unwrap();
        same.2 += (t.verdict == o.verdict) as usize;
        same.3 += t
            .d
            .as_ref()
            .zip(o.d.as_ref())
            .is_some_and(|(a, b)| a.bands.iter().zip(&b.bands).all(|(x, y)| x.verdict == y.verdict))
            as usize;
    }
    same.4 = judged
        .atmospheric_validation
        .runs
        .iter()
        .zip(&rep.atmospheric_validation.runs)
        .filter(|(a, b)| a.t30 == b.t30 && a.error.is_none() && b.error.is_none())
        .count();
    println!(
        "  against report.json: cell verdicts equal {}/{}, every seed's T30 and error equal in {}/{} cells, TCR verdicts equal {}/{}, D band verdicts equal in {}/{}, atmospheric seeds' T30 equal {}/{}; N5 as_required {} (was {}), N6 {} (was {})",
        same.0,
        judged.cells.len(),
        same.1,
        judged.cells.len(),
        same.2,
        judged.tcr.len(),
        same.3,
        judged.tcr.len(),
        same.4,
        rep.atmospheric_validation.runs.len(),
        judged.say_no.n5.as_required,
        rep.say_no.n5.as_required,
        judged.say_no.n6.as_required,
        rep.say_no.n6.as_required
    );

    // ======================================================================================
    println!("\n== R. Every earlier tampered copy ({OLD_TAMPER}), copied fresh");
    let a005 = "5x4x3-a0.05-energetic-air-off";
    let a01 = "5x4x3-a0.1-energetic-air-off";
    let t04 = "5x4x3-a0.4-tcr-air-off";
    let t02 = "5x4x3-a0.2-tcr-air-off";
    let bed10 = Path::new(BED10);
    let cases: Vec<(&str, Vec<RunKey>, &str, &str)> = vec![
        ("swap-seed", vec![cell(ID, 3)], ID, "pass true (finding 1)"),
        ("swap-all-to-s1", (2..=10).map(|s| cell(ID, s)).collect(), ID, "pass true, cell Pass on ten identical seeds (finding 1)"),
        ("edited-sha-other-cell", (1..=10).map(|s| cell(a005, s)).collect(), a005, "accepted by check_planned; the bed failed at E3 and C (finding 2)"),
        ("edited-sha-other-seed", vec![cell(ID, 3)], ID, "refused: random_seed 4, the bed's 3"),
        ("outputs-10ms", vec![cell(ID, 1)], ID, "not read: results_file_invalid, time step"),
        ("tcr-outputs-10ms", vec![tcr(t04)], t04, "accepted; its outputs byte-identical to its own, not a bypass"),
        ("tcr-edited-sha", vec![tcr(t04)], t04, "pass true, TCR Pass (finding 2)"),
    ];
    for (name, keys, id, before) in &cases {
        let old = Path::new(OLD_TAMPER).join(name);
        let case = fresh(&format!("R-{name}"));
        copy_dir(&old, &case);
        let same_bytes = hash_tree(&old, &[]).unwrap() == hash_tree(&case, &[]).unwrap();
        println!("  R {name} ({} run(s)); before: {before}; the fresh copy is the old one byte for byte: {same_bytes}", keys.len());
        // What the tamper is, checked on the copy against the bed.
        let k0 = &keys[0];
        let f0 = run_folder(&case, k0);
        let intact = match *name {
            "swap-seed" => {
                outputs(&f0) == outputs(&run_folder(&root, &cell(ID, 4)))
                    && std::fs::read(f0.join("run.json")).unwrap()
                        == std::fs::read(run_folder(&root, &cell(ID, 3)).join("run.json")).unwrap()
            }
            "swap-all-to-s1" => keys.iter().all(|k| outputs(&run_folder(&case, k)) == outputs(&run_folder(&root, &cell(ID, 1)))),
            "edited-sha-other-cell" => keys.iter().all(|k| {
                let RunKey::Cell { seed, .. } = k else { unreachable!() };
                let f = run_folder(&case, k);
                file_sha(&f.join("solve/config.xml")) == file_sha(&run_folder(&root, &cell(a01, *seed)).join("solve/config.xml"))
                    && bread::run_info(&f).unwrap().project_sha256 == Some(run::planned_project_sha256(ctx.planned(k)))
            }),
            "edited-sha-other-seed" => {
                file_sha(&f0.join("solve/config.xml")) == file_sha(&run_folder(&root, &cell(ID, 4)).join("solve/config.xml"))
                    && bread::run_info(&f0).unwrap().project_sha256 == Some(run::planned_project_sha256(ctx.planned(k0)))
            }
            "outputs-10ms" => outputs(&f0) == outputs(&run_folder(bed10, &cell(ID, 1))),
            "tcr-outputs-10ms" => {
                let own = outputs(&f0) == outputs(&run_folder(&root, &tcr(t04)));
                println!("    its outputs are the M8a run's own, byte for byte: {own}");
                outputs(&f0) == outputs(&run_folder(bed10, &tcr(t04)))
            }
            "tcr-edited-sha" => {
                file_sha(&f0.join("solve/config.xml")) == file_sha(&run_folder(&root, &tcr(t02)).join("solve/config.xml"))
                    && bread::run_info(&f0).unwrap().project_sha256 == Some(run::planned_project_sha256(ctx.planned(k0)))
            }
            _ => unreachable!(),
        };
        println!("    the tamper is intact in the copy: {intact}");
        let sealed = ctx.read_many(&case, keys, Some(&seal));
        println!("    with the seal: {}", verdict(ctx.planned(k0), &sealed[0].1));
        println!("      all: {:?}", counts(sealed.iter().map(|(k, r)| code(ctx.planned(k), r))));
        line("judged with the seal", &ctx.judge_with(sealed), id);
        let unsealed = ctx.read_many(&case, keys, None);
        println!("    without the seal: {:?}", counts(unsealed.iter().map(|(k, r)| code(ctx.planned(k), r))));
        // The record forged too: run.json's outputs written for the tampered files, no seal.
        let forged = fresh(&format!("R-{name}-forged"));
        copy_dir(&old, &forged);
        for k in keys {
            forge_record(&run_folder(&forged, k));
        }
        let f = ctx.read_many(&forged, keys, None);
        println!("    forged record, no seal: {}", verdict(ctx.planned(k0), &f[0].1));
        println!("      all: {:?}", counts(f.iter().map(|(k, r)| code(ctx.planned(k), r))));
        let j = ctx.judge_with(f);
        line("judged, forged record", &j, id);
        seeds_line(&j, id);
    }

    // ======================================================================================
    println!("\n== M. The other cases of VERIFY-adversarial.md (M8b step 1)");
    // M1: TCR band lists, on a read (before into_reads) and on the reads (after, D alone).
    let tid = "6x10x3-a0.2-tcr-air-on";
    let bt = ctx.base_read(&tcr(tid));
    let tb = match &bt {
        Read::Tcr(t) => t.bands.clone(),
        _ => unreachable!(),
    };
    let with = |f: &[(usize, i32)]| {
        f.iter()
            .map(|&(i, hz)| {
                let mut b = tb[i].clone();
                b.freq_hz = hz;
                b
            })
            .collect::<Vec<_>>()
    };
    let band_cases: Vec<(&str, Vec<bread::TcrBand>)> = vec![
        ("8 kHz removed", with(&[(0, 125), (1, 250), (2, 500), (3, 1000), (4, 2000), (5, 4000)])),
        ("only 125 Hz", with(&[(0, 125)])),
        ("8 kHz relabelled 16 kHz", with(&[(0, 125), (1, 250), (2, 500), (3, 1000), (4, 2000), (5, 4000), (6, 16000)])),
        ("4 kHz twice, no 8 kHz", with(&[(0, 125), (1, 250), (2, 500), (3, 1000), (4, 2000), (5, 4000), (5, 4000)])),
        ("125 Hz twice, no 8 kHz", with(&[(0, 125), (0, 125), (1, 250), (2, 500), (3, 1000), (4, 2000), (5, 4000)])),
        ("4 kHz row relabelled 8 kHz", with(&[(0, 125), (1, 250), (2, 500), (3, 1000), (4, 2000), (6, 8000), (5, 8000)])),
        ("8 kHz twice (8 rows)", with(&[(0, 125), (1, 250), (2, 500), (3, 1000), (4, 2000), (5, 4000), (6, 8000), (6, 8000)])),
    ];
    println!("  M1 TCR band lists of {tid} (read: check_planned; reads: D alone after into_reads):");
    for (name, b) in &band_cases {
        let mut read = bt.clone();
        if let Read::Tcr(t) = &mut read {
            t.bands = b.clone();
        }
        let c1 = code(ctx.planned(&tcr(tid)), &Ok(read.clone()));
        let j1 = ctx.judge_with(vec![(tcr(tid), Ok(read))]);
        let mut rd = base_reads.clone();
        rd.tcr.get_mut(tid).unwrap().as_mut().unwrap().bands = b.clone();
        let j2 = ctx.evaluate(&rd);
        println!(
            "    {name}: read {c1}, pass {}; after into_reads: pass {}, {}",
            j1.pass,
            j2.pass,
            verdict_of(&j2, tid)
        );
    }
    let off = "6x10x3-a0.2-tcr-air-off";
    let mut ro = ctx.base_read(&tcr(off));
    if let Read::Tcr(t) = &mut ro {
        t.bands[5].freq_hz = 8000;
    }
    let mut rd = base_reads.clone();
    if let (Read::Tcr(t), Some(Ok(x))) = (&ro, rd.tcr.get_mut(off)) {
        x.bands = t.bands.clone();
    }
    println!(
        "    air-off, 4 kHz relabelled 8 kHz: read {}; after into_reads pass {}",
        code(ctx.planned(&tcr(off)), &Ok(ro)),
        ctx.evaluate(&rd).pass
    );
    // M2: SPPS settings, one at a time.
    let k3 = cell(ID, 3);
    let r3 = ctx.base_read(&k3);
    let p4sha = run::planned_project_sha256(ctx.planned(&cell(ID, 4)));
    type Mut = Box<dyn Fn(&mut bread::SppsRead)>;
    let muts: Vec<(&str, Mut)> = vec![
        ("trans_epsilon 7", Box::new(|s| s.trans_epsilon = 7.0)),
        ("trans_epsilon 9.000000000001", Box::new(|s| s.trans_epsilon = 9.000000000001)),
        ("particles 150,000", Box::new(|s| s.particles_per_source = 150_000)),
        ("particles +1", Box::new(|s| s.particles_per_source += 1)),
        ("method 0", Box::new(|s| s.computation_method = 0)),
        ("seed none", Box::new(|s| s.random_seed = None)),
        ("seed 0", Box::new(|s| s.random_seed = Some(0))),
        ("4 kHz relabelled 8 kHz", Box::new(|s| s.bands_hz[5] = 8000)),
        ("2 kHz twice", Box::new(|s| s.bands_hz[5] = 2000)),
        ("two bands swapped", Box::new(|s| s.bands_hz.swap(0, 1))),
        ("seed 4's project sha256", Box::new(move |s| s.info.project_sha256 = Some(p4sha.clone()))),
        ("step NaN", Box::new(|s| s.time_step_s = f64::NAN)),
    ];
    println!("  M2 {} settings, one at a time: {}", k3.label(), code(ctx.planned(&k3), &Ok(r3.clone())));
    for (name, m) in &muts {
        let mut r = r3.clone();
        if let Read::Spps(s) = &mut r {
            m(s);
        }
        println!("    {name}: {}", code(ctx.planned(&k3), &Ok(r)));
    }
    // M3, M4: real runs filed under another key.
    println!("  M3/M4 real runs filed under another key (the folder read as that key with the seal; the base read checked against that plan):");
    let others: Vec<(RunKey, RunKey)> = vec![
        (RunKey::N5 { seed: 10 }, cell("5x4x3-a0.4-random-air-off", 10)),
        (cell("5x4x3-a0.4-random-air-off", 1), cell(ID, 1)),
        (cell(a01, 3), cell(a005, 3)),
        (cell(ID, 1), tcr(t04)),
        (cell("5x4x3-a0.2-energetic-air-off", 1), RunKey::N6 { seed: 1 }),
        (cell(ID, 4), cell(ID, 3)),
        (cell(ID, 3), cell(ID, 4)),
        (tcr(t02), tcr(t04)),
    ];
    let mut subst = Vec::new();
    for (from, as_key) in &others {
        if !runs.iter().any(|p| &p.key == as_key) {
            println!("    {} as {}: that key is not planned", from.label(), as_key.label());
            continue;
        }
        let r = ctx.read_dir_as(&root.join(from.dir()), as_key, Some(&seal));
        let m = code(ctx.planned(as_key), &Ok(ctx.base_read(from)));
        println!(
            "    {} as {}: folder {}; in memory {m}",
            from.label(),
            as_key.label(),
            code(ctx.planned(as_key), &r)
        );
        if *from == cell(ID, 4) || *from == cell(ID, 3) {
            subst.push((as_key.clone(), r));
        }
    }
    line("s3 and s4 swapped (M4)", &ctx.judge_with(subst), ID);
    // M5: the smoke bed read in place of the M8a runs.
    let smoke = Path::new(SMOKE);
    let keys: Vec<RunKey> = runs
        .iter()
        .filter(|p| bread::find_run_folder(&smoke.join(p.key.dir())).is_some())
        .map(|p| p.key.clone())
        .collect();
    let s_sealed = ctx.read_many(smoke, &keys, Some(&seal));
    let s_none = ctx.read_many(smoke, &keys, None);
    println!(
        "  M5 the smoke bed's {} matching keys: with the seal {:?}; without {:?}",
        keys.len(),
        counts(s_sealed.iter().map(|(k, r)| code(ctx.planned(k), r))),
        counts(s_none.iter().map(|(k, r)| code(ctx.planned(k), r)))
    );
    line("smoke runs in place, with the seal", &ctx.judge_with(s_sealed), ID);
    // The smoke runs with forged records.
    let sf = fresh("M5-smoke-forged");
    for k in &keys {
        slot(smoke, k, &sf, k);
        forge_record(&run_folder(&sf, k));
    }
    let s_forged = ctx.read_many(&sf, &keys, None);
    println!(
        "    forged records: {:?}; e.g. {}",
        counts(s_forged.iter().map(|(k, r)| code(ctx.planned(k), r))),
        verdict(ctx.planned(&s_forged[0].0), &s_forged[0].1)
    );
    // M6: the 10 ms runs.
    let k10 = [cell(ID, 1), tcr(t04)];
    let b_sealed = ctx.read_many(bed10, &k10, Some(&seal));
    let b_none = ctx.read_many(bed10, &k10, None);
    let bf = fresh("M6-bed10-forged");
    for k in &k10 {
        slot(bed10, k, &bf, k);
        forge_record(&run_folder(&bf, k));
    }
    let b_forged = ctx.read_many(&bf, &k10, None);
    for i in 0..2 {
        println!(
            "  M6 the 10 ms {} under the 1 ms plan: with the seal {}; without {}; forged record {}",
            k10[i].label(),
            code(ctx.planned(&k10[i]), &b_sealed[i].1),
            code(ctx.planned(&k10[i]), &b_none[i].1),
            verdict(ctx.planned(&k10[i]), &b_forged[i].1)
        );
    }

    // ======================================================================================
    println!("\n== A. New attempts on the sealed bed, editing only the bed folder (each read with the committed seal)");
    let s3 = cell(ID, 3);
    let s4 = cell(ID, 4);
    let f3 = run_folder(&root, &s3);
    let f4 = run_folder(&root, &s4);
    let recp = "solve/Punctual receivers/R000/Sound level.recp";
    let a_case = |name: &str, key: &RunKey, build: &dyn Fn(&Path)| {
        let case = fresh(name);
        build(&case);
        let r = ctx.read(&case, key, Some(&seal));
        println!("  {name}: {}", verdict(ctx.planned(key), &r));
        let j = ctx.judge_with(vec![(key.clone(), r.clone())]);
        let id = match key {
            RunKey::Cell { id, .. } | RunKey::Tcr { id } => id.clone(),
            _ => ID.to_string(),
        };
        println!("    judged: pass {} | {}", j.pass, verdict_of(&j, &id));
        (r, j)
    };
    // A1
    a_case("A1-forged-run-json", &s3, &|c| {
        let f = slot(&root, &s3, c, &s3);
        swap_outputs(&f, &f4);
        forge_record(&f);
    });
    // A2
    let renamed = Path::new(ADV).join("20260929T093135Z");
    let lower = Path::new(r"C:\tmp\nm-m8a-bed\20260929t093134z");
    println!(
        "  A2a the committed seal for a renamed bed folder: {:?}; for the bed's own path in lower case: {:?}",
        seal.for_bed(&renamed).err(),
        seal.for_bed(lower).err()
    );
    a_case("A2b-log-not-read-changed", &s3, &|c| {
        let f = slot(&root, &s3, c, &s3);
        let p = f.join("solver.stdout.txt");
        let mut b = std::fs::read(&p).unwrap();
        b.push(b'\n');
        write(&p, &b);
    });
    let mut extra = seal.clone();
    extra
        .runs
        .get_mut(&bind::seal_key(&s3))
        .unwrap()
        .files
        .push(bind::SealedFile(format!("{}/solve/not-there.recp", f3.file_name().unwrap().to_str().unwrap()), 1, "0".repeat(64)));
    let r = ctx.read(&root, &s3, Some(&extra));
    println!("  A2c (the seal edited, out of scope) an entry for a file the folder does not have: {}", verdict(ctx.planned(&s3), &r));
    // A3
    let case = fresh("A3a-slots-s3-s4-swapped");
    slot(&root, &s4, &case, &s3);
    slot(&root, &s3, &case, &s4);
    let (x, y) = (ctx.read(&case, &s3, Some(&seal)), ctx.read(&case, &s4, Some(&seal)));
    println!("  A3a whole slots s3 and s4 swapped: s3 {}; s4 {}", code(ctx.planned(&s3), &x), code(ctx.planned(&s4), &y));
    line("judged", &ctx.judge_with(vec![(s3.clone(), x), (s4.clone(), y)]), ID);
    a_case("A3b-run-folder-of-s4-beside-s3-project", &s3, &|c| {
        let d = c.join(s3.dir());
        copy_dir(&f4, &d.join(f4.file_name().unwrap()));
        write(&d.join("project.simpa"), &std::fs::read(root.join(s3.dir()).join("project.simpa")).unwrap());
    });
    a_case("A3c-cell-a0.1-s3-as-a0.05-s3", &cell(a005, 3), &|c| {
        slot(&root, &cell(a01, 3), c, &cell(a005, 3));
    });
    a_case("A3d-tcr-a0.2-slot-as-a0.4", &tcr(t04), &|c| {
        slot(&root, &tcr(t02), c, &tcr(t04));
    });
    // A4
    a_case("A4a-padding-byte-of-one-output", &s3, &|c| {
        let f = slot(&root, &s3, c, &s3);
        pad(&f.join(recp), 1);
    });
    a_case("A4b-padding-byte-and-its-record", &s3, &|c| {
        let f = slot(&root, &s3, c, &s3);
        pad(&f.join(recp), 1);
        forge_record(&f);
    });
    // A5
    a_case("A5a-band-files-125-250-swapped", &s3, &|c| {
        let f = slot(&root, &s3, c, &s3);
        let (a, b) = (
            f.join("solve/Intensity animation/125 Hz/Intensity.rpi"),
            f.join("solve/Intensity animation/250 Hz/Intensity.rpi"),
        );
        let (x, y) = (std::fs::read(&a).unwrap(), std::fs::read(&b).unwrap());
        write(&a, &y);
        write(&b, &x);
    });
    a_case("A5b-receivers-R000-R001-swapped", &s3, &|c| {
        let f = slot(&root, &s3, c, &s3);
        for name in ["Sound level.recp", "Advanced sound level.gap", "Punctual receiver intensity.gabe", "Sound level per source.recps"] {
            let a = f.join("solve/Punctual receivers/R000").join(name);
            let b = f.join("solve/Punctual receivers/R001").join(name);
            let (x, y) = (std::fs::read(&a).unwrap(), std::fs::read(&b).unwrap());
            write(&a, &y);
            write(&b, &x);
        }
    });
    // A6
    a_case("A6a-truncated-by-one-byte", &s3, &|c| {
        let f = slot(&root, &s3, c, &s3);
        let b = std::fs::read(f.join(recp)).unwrap();
        write(&f.join(recp), &b[..b.len() - 1]);
    });
    a_case("A6b-truncated-to-nothing", &s3, &|c| {
        let f = slot(&root, &s3, c, &s3);
        write(&f.join(recp), b"");
    });
    // A7: links, every target a scratch copy (never the bed).
    let src = fresh("A7-sources");
    let s4_copy = src.join("s4");
    copy_dir(&root.join(s4.dir()), &s4_copy);
    let s3_copy = src.join("s3");
    copy_dir(&root.join(s3.dir()), &s3_copy);
    let rel_r000 = Path::new("solve/Punctual receivers/R000");
    let link_case = |name: &str, build: &dyn Fn(&Path) -> Result<(), String>| {
        let case = fresh(name);
        match build(&case) {
            Err(e) => println!("  {name}: the link could not be made: {e}"),
            Ok(()) => {
                let r = ctx.read(&case, &s3, Some(&seal));
                let same = r.as_ref().ok().map(|x| t30_eq(x, &ctx.base_read(&s3)));
                println!("  {name}: {} (T30 the real s3's: {same:?})", verdict(ctx.planned(&s3), &r));
            }
        }
    };
    link_case("A7a-slot-junction-to-s4", &|c| {
        let d = c.join(s3.dir());
        std::fs::create_dir_all(d.parent().unwrap()).unwrap();
        mklink(&["/J"], &d, &s4_copy)
    });
    link_case("A7b-slot-junction-to-an-identical-s3", &|c| {
        let d = c.join(s3.dir());
        std::fs::create_dir_all(d.parent().unwrap()).unwrap();
        mklink(&["/J"], &d, &s3_copy)
    });
    let r000 = |c: &Path, target: &Path| -> Result<(), String> {
        let d = c.join(s3.dir());
        let skip = f3.join(rel_r000);
        copy_dir_except(&root.join(s3.dir()), &d, &|p| p == skip);
        let f = bread::find_run_folder(&d).unwrap();
        mklink(&["/J"], &f.join(rel_r000), target)
    };
    link_case("A7c-inner-junction-R000-to-s4s", &|c| r000(c, &bread::find_run_folder(&s4_copy).unwrap().join(rel_r000)));
    link_case("A7d-inner-junction-R000-to-an-identical-copy", &|c| r000(c, &bread::find_run_folder(&s3_copy).unwrap().join(rel_r000)));
    let one = |c: &Path, how: &[&str], target: &Path| -> Result<(), String> {
        let d = c.join(s3.dir());
        let skip = f3.join(recp);
        copy_dir_except(&root.join(s3.dir()), &d, &|p| p == skip);
        let f = bread::find_run_folder(&d).unwrap();
        mklink(how, &f.join(recp), target)
    };
    let s4_recp = bread::find_run_folder(&s4_copy).unwrap().join(recp);
    let s3_recp = bread::find_run_folder(&s3_copy).unwrap().join(recp);
    link_case("A7e-file-symlink-to-s4s", &|c| one(c, &[], &s4_recp));
    link_case("A7f-file-symlink-to-an-identical-copy", &|c| one(c, &[], &s3_recp));
    link_case("A7g-hard-link-to-s4s", &|c| one(c, &["/H"], &s4_recp));
    link_case("A7h-hard-link-to-an-identical-copy", &|c| one(c, &["/H"], &s3_recp));
    // A8: extra files and folders.
    let extra_case = |name: &str, build: &dyn Fn(&Path, &Path)| {
        let case = fresh(name);
        let f = slot(&root, &s3, &case, &s3);
        build(&case.join(s3.dir()), &f);
        let r = ctx.read(&case, &s3, Some(&seal));
        let same = r.as_ref().ok().map(|x| {
            t30_eq(x, &ctx.base_read(&s3)) && info(x).outputs_sha256 == info(&ctx.base_read(&s3)).outputs_sha256
        });
        println!("  {name}: {} (T30 and outputs the real s3's: {same:?})", verdict(ctx.planned(&s3), &r));
    };
    extra_case("A8a-extra-file-in-solve", &|_, f| write(&f.join("solve/extra.recp"), b"x"));
    extra_case("A8b-extra-file-beside-project", &|d, _| write(&d.join("notes.txt"), b"x"));
    extra_case("A8c-extra-file-in-mesh", &|_, f| write(&f.join("mesh/extra.txt"), b"x"));
    extra_case("A8d-empty-folder-in-solve", &|_, f| std::fs::create_dir_all(f.join("solve/zz-empty")).unwrap());
    extra_case("A8e-empty-receiver-folder-R099", &|_, f| std::fs::create_dir_all(f.join("solve/Punctual receivers/R099")).unwrap());
    extra_case("A8f-empty-folder-in-the-slot", &|d, _| std::fs::create_dir_all(d.join("zz-empty")).unwrap());
    extra_case("A8g-empty-folder-in-R000", &|_, f| std::fs::create_dir_all(f.join("solve/Punctual receivers/R000/S000")).unwrap());
    extra_case("A8h-empty-band-folder-16000-Hz", &|_, f| std::fs::create_dir_all(f.join("solve/Intensity animation/16000 Hz")).unwrap());
    extra_case("A8i-second-run-folder-sorting-after", &|d, _| copy_dir(&f4, &d.join("zz-99999999-spps")));
    // A9: TCR.
    let ft04 = run_folder(&root, &tcr(t04));
    let tcr_out = outputs(&ft04).keys().next().unwrap().clone();
    println!("  (TCR {t04}'s outputs: {:?})", outputs(&ft04).keys().collect::<Vec<_>>());
    a_case("A9a-tcr-padding-byte", &tcr(t04), &|c| {
        let f = slot(&root, &tcr(t04), c, &tcr(t04));
        pad(&f.join("solve").join(&tcr_out), 1);
    });
    a_case("A9b-tcr-carrying-a0.2s-outputs", &tcr(t04), &|c| {
        let f = slot(&root, &tcr(t04), c, &tcr(t04));
        swap_outputs(&f, &run_folder(&root, &tcr(t02)));
    });
    // A10: the atmospheric validation (reported, not gated).
    let at = |s| RunKey::AtmosphericSpps { seed: s };
    let case = fresh("A10a-atmospheric-s2-carrying-s1s-outputs");
    let f = slot(&root, &at(2), &case, &at(2));
    swap_outputs(&f, &run_folder(&root, &at(1)));
    let r = ctx.read(&case, &at(2), Some(&seal));
    println!("  A10a atmospheric seed 2 carrying seed 1's outputs: {}", verdict(ctx.planned(&at(2)), &r));
    let j = ctx.judge_with(vec![(at(2), r)]);
    println!(
        "    judged: pass {}; the atmospheric report's seed 2 error: {:?}",
        j.pass,
        j.atmospheric_validation.runs.iter().find(|x| x.seed == 2).and_then(|x| x.error.as_deref().map(|e| short(e, 120)))
    );
    let case = fresh("A10b-atmospheric-tcr-padding-byte");
    let f = slot(&root, &RunKey::AtmosphericTcr, &case, &RunKey::AtmosphericTcr);
    let o = outputs(&f).keys().next().unwrap().clone();
    pad(&f.join("solve").join(&o), 1);
    let r = ctx.read(&case, &RunKey::AtmosphericTcr, Some(&seal));
    println!("  A10b atmospheric TCR, one padding byte: {}", verdict(ctx.planned(&RunKey::AtmosphericTcr), &r));
    // A11: N5 and N6.
    let n5 = RunKey::N5 { seed: 10 };
    let case = fresh("A11a-n5-carrying-its-cells-seed-10");
    let f = slot(&root, &n5, &case, &n5);
    swap_outputs(&f, &run_folder(&root, &cell("5x4x3-a0.4-random-air-off", 10)));
    let r = ctx.read(&case, &n5, Some(&seal));
    println!("  A11a N5 carrying its cell's seed-10 outputs (31 M particles): {}", verdict(ctx.planned(&n5), &r));
    let j = ctx.judge_with(vec![(n5.clone(), r)]);
    println!("    judged: pass {}; N5 as_required {} (error {:?}): the gate's N5 check fails", j.pass, j.say_no.n5.as_required, j.say_no.n5.error.as_deref().map(|e| short(e, 80)));
    let n6 = RunKey::N6 { seed: 1 };
    let case = fresh("A11b-n6-carrying-a-scattering-cells-outputs");
    let f = slot(&root, &n6, &case, &n6);
    swap_outputs(&f, &run_folder(&root, &cell("5x4x3-a0.2-energetic-air-off", 1)));
    let r = ctx.read(&case, &n6, Some(&seal));
    println!("  A11b N6 carrying 5x4x3-a0.2-energetic-air-off seed 1's outputs: {}", verdict(ctx.planned(&n6), &r));
    let j = ctx.judge_with(vec![(n6.clone(), r)]);
    println!("    judged: pass {}; N6 as_required {} (error {:?})", j.pass, j.say_no.n6.as_required, j.say_no.n6.error.as_deref().map(|e| short(e, 80)));

    // ======================================================================================
    println!("\n== U. No seal: run.json is the only record (a new bed, or the M8a bed under another folder name)");
    let seeds: Vec<RunKey> = (1..=10).map(|s| cell(ID, s)).collect();
    let u_case = |name: &str, change: &dyn Fn(&Path)| -> Vec<(RunKey, Result<Read, String>)> {
        let case = fresh(name);
        for k in &seeds {
            slot(&root, k, &case, k);
        }
        change(&case);
        for k in &seeds {
            forge_record(&run_folder(&case, k));
        }
        ctx.read_many(&case, &seeds, None)
    };
    let u0 = u_case("U0-ten-seeds-with-their-true-record", &|_| {});
    println!(
        "  U0 the ten seeds of {ID}, each run.json given its true outputs: {:?}; reads equal to the sealed reads (T30 and outputs sha256): {}",
        counts(u0.iter().map(|(k, r)| code(ctx.planned(k), r))),
        u0.iter().all(|(k, r)| r.as_ref().is_ok_and(|x| t30_eq(x, &ctx.base_read(k)) && info(x).outputs_sha256 == info(&ctx.base_read(k)).outputs_sha256))
    );
    line("U0 judged", &ctx.judge_with(u0), ID);
    let u1 = u_case("U1-s3-carrying-s4s-outputs-one-padding-byte-each", &|c| {
        let f = run_folder(c, &s3);
        swap_outputs(&f, &f4);
        pad_outputs(&f, 1);
    });
    let r3u = &u1.iter().find(|(k, _)| *k == s3).unwrap().1;
    println!(
        "  U1 s3 carrying s4's 20 outputs, one padding byte set in each, record forged: s3 {}; its T30 is s4's: {:?}",
        verdict(ctx.planned(&s3), r3u),
        r3u.as_ref().ok().map(|x| t30_eq(x, &ctx.base_read(&s4)))
    );
    let j = ctx.judge_with(u1);
    line("U1 judged", &j, ID);
    seeds_line(&j, ID);
    let u2 = u_case("U2-s2-to-s10-carrying-s1s-outputs", &|c| {
        let f1 = run_folder(&root, &cell(ID, 1));
        for s in 2..=10u32 {
            let f = run_folder(c, &cell(ID, s));
            swap_outputs(&f, &f1);
            pad_outputs(&f, s as u8);
        }
    });
    let same1 = u2
        .iter()
        .filter(|(_, r)| r.as_ref().is_ok_and(|x| t30_eq(x, &ctx.base_read(&cell(ID, 1)))))
        .count();
    println!(
        "  U2 seeds 2 to 10 carrying seed 1's outputs, padding byte = seed, records forged: {:?}; seeds whose T30 is seed 1's: {same1} of 10",
        counts(u2.iter().map(|(k, r)| code(ctx.planned(k), r)))
    );
    let j = ctx.judge_with(u2);
    line("U2 judged", &j, ID);
    seeds_line(&j, ID);
    if let Some(c) = j.cells.iter().find(|c| c.id == ID) {
        println!("      B spread {:?}; C per-seed {:?}", c.b.as_ref().map(|b| b.spread), c.c.as_ref().map(|x| x.per_seed.iter().map(|v| format!("{v:+.5}")).collect::<Vec<_>>()));
    }
    // U3: TCR, hole 42 with a forged record.
    let case = fresh("U3-tcr-a0.4-carrying-a0.2s-outputs");
    let f = slot(&root, &tcr(t04), &case, &tcr(t04));
    swap_outputs(&f, &run_folder(&root, &tcr(t02)));
    forge_record(&f);
    let r = ctx.read(&case, &tcr(t04), None);
    println!("  U3 TCR {t04}'s run carrying {t02}'s outputs, record forged: {}", verdict(ctx.planned(&tcr(t04)), &r));
    let j = ctx.judge_with(vec![(tcr(t04), r)]);
    line("U3 judged", &j, t04);
    if let Some(d) = j.tcr.iter().find(|t| t.id == t04).and_then(|t| t.d.as_ref()) {
        println!("      D deviations: {:?}", d.bands.iter().map(|b| format!("{} Hz {:+.2} %", b.freq_hz, 100.0 * b.deviation.unwrap_or(f64::NAN))).collect::<Vec<_>>());
    }
    // U4: the smoke bed's s3 outputs (fewer particles) in the M8a s3 slot.
    let sm3 = run_folder(smoke, &s3);
    let smoke_particles = {
        let t = std::fs::read_to_string(sm3.join("solve/config.xml")).unwrap();
        let i = t.find("nbparticules=\"").map(|i| i + 14).unwrap();
        t[i..i + t[i..].find('"').unwrap()].to_string()
    };
    let u4 = u_case("U4-s3-carrying-the-smoke-beds-s3-outputs", &|c| {
        swap_outputs(&run_folder(c, &s3), &sm3);
    });
    let r = &u4.iter().find(|(k, _)| *k == s3).unwrap().1;
    println!(
        "  U4 s3 carrying the smoke bed's s3 outputs (its config.xml: nbparticules {smoke_particles}), record forged: {}; particles as read {:?}",
        verdict(ctx.planned(&s3), r),
        r.as_ref().ok().and_then(|x| match x { Read::Spps(s) => Some(s.particles_per_source), _ => None })
    );
    let j = ctx.judge_with(u4);
    line("U4 judged", &j, ID);
    seeds_line(&j, ID);
    // U5: the atmospheric validation.
    let atm: Vec<RunKey> = (1..=10).map(at).collect();
    let case = fresh("U5-atmospheric-s2-carrying-s1s-outputs");
    for k in &atm {
        slot(&root, k, &case, k);
    }
    let f = run_folder(&case, &at(2));
    swap_outputs(&f, &run_folder(&root, &at(1)));
    pad_outputs(&f, 2);
    for k in &atm {
        forge_record(&run_folder(&case, k));
    }
    let u5 = ctx.read_many(&case, &atm, None);
    println!("  U5 atmospheric seed 2 carrying seed 1's outputs, padding byte set, records forged: {:?}", counts(u5.iter().map(|(k, r)| code(ctx.planned(k), r))));
    let j = ctx.judge_with(u5);
    let (b0, b1) = (&judged.atmospheric_validation.bands[0], &j.atmospheric_validation.bands[0]);
    println!(
        "    judged: pass {}; {} Hz mean over {} seeds {:?} (real {:?} over {})",
        j.pass,
        b1.freq_hz,
        b1.seeds_judged,
        b1.spps.as_ref().map(|i| i.mean),
        b0.spps.as_ref().map(|i| i.mean),
        b0.seeds_judged
    );
    // U6: s4's genuine run filed as s3, its own record.
    let case = fresh("U6-s4s-genuine-run-filed-as-s3");
    let f = slot(&root, &s4, &case, &s3);
    forge_record(&f);
    let r = ctx.read(&case, &s3, None);
    println!("  U6 s4's genuine run filed as s3, its true record: {}", verdict(ctx.planned(&s3), &r));

    // ======================================================================================
    println!("\n== X. Gate C's extension seeds (11 to 20) when a seal is given");
    println!("  the real bed re-judged: needs_extension {:?} (so no extension is planned or read for it)", judged.needs_extension);
    let ext = run::plan_extension(&l.bed, &[ID.to_string()]).unwrap();
    let p11 = ext.iter().find(|p| p.key == cell(ID, 11)).unwrap().clone();
    let case = fresh("X-extension-seed-11");
    let f = slot(&root, &cell(ID, 10), &case, &p11.key);
    write(&case.join(p11.key.dir()).join("project.simpa"), simpa_core::schema::to_json(&p11.project).as_bytes());
    let cfg = f.join("solve/config.xml");
    let text = std::fs::read_to_string(&cfg).unwrap();
    assert_eq!(text.matches(" random_seed=\"10\"").count(), 1, "one random_seed");
    let text = text.replace(" random_seed=\"10\"", " random_seed=\"11\"");
    write(&cfg, text.as_bytes());
    let want = run::planned_inputs(&p11).unwrap();
    println!(
        "  config.xml of seed 10 with random_seed 11 is the plan's for seed 11 (workingdirectory aside): {}",
        sha256_bytes(bind::without_workdir(&text).as_bytes()) == want.config_sha256
    );
    pad_outputs(&f, 11);
    let mut m = bread::manifest_of(&f).unwrap();
    if let RunSource::Project { sha256, .. } = &mut m.source {
        *sha256 = run::planned_project_sha256(&p11);
    }
    let new_cfg = file_sha(&cfg);
    m.inputs.iter_mut().filter(|i| i.path == "config.xml").for_each(|i| i.sha256 = new_cfg.clone());
    write(&f.join("run.json"), m.to_json().as_bytes());
    forge_record(&f);
    let mut r = run::read_in(&case.join(p11.key.dir()), &p11, Some(&seal));
    clear(&mut r);
    println!("  seed 11 made from seed 10's run (seed 11's config and project, one padding byte in each output, record forged), read WITH the seal: {}", verdict(&p11, &r));
    let mut rd = base_reads.clone();
    let ext_results: Vec<Result<Read, String>> = ext
        .iter()
        .map(|p| if p.key == p11.key { r.clone() } else { Err("not made here".into()) })
        .collect();
    run::into_reads(&mut rd, &ext, ext_results, None);
    println!(
        "  through into_reads with seeds 1 to 10: seed 11 {}",
        match rd.spps.get(ID).and_then(|m| m.get(&11)) {
            Some(Ok(_)) => "kept (no seed rule refusal)".to_string(),
            Some(Err(e)) => format!("refused: {}", short(e, 200)),
            None => "absent".into(),
        }
    );

    // ======================================================================================
    println!("\n== V2. Fresh runs by this build ({FRESH}), bound by their own run.json, no seal");
    for k in [tcr(t02), RunKey::N5 { seed: 10 }] {
        let r = ctx.read(Path::new(FRESH), &k, None);
        println!("  {}: {}", k.label(), verdict(ctx.planned(&k), &r));
    }

    println!("\ndone in {:.0} s", t_all.elapsed().as_secs_f64());
    let _ = Verdict::Pass;
}
