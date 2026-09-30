//! M8b tamper: the closure of backlog rows 41 and 42 (m8b-tamper) on real runs, with this build.
//!
//! A. The M8a bed `C:\tmp\nm-m8a-bed\20260929T093134Z`, read only, through `run::read_existing` with
//!    the committed seal `beds/m8a-20260929T093134Z/outputs-seal.json`: every run bound, every run
//!    matched to its plan, and the bed re-judged (transports from its report.json, nothing traced)
//!    against its report.json, value by value. The same bed without the seal: every run refused.
//! B. The judge's tampered copies (`C:\tmp\nm-judge-m8b\tamper`, copied to `cases\`), which gave
//!    `pass true` before this fix, read with the real seal and with a forged one (the tampered
//!    folder's own hashes in its entry, as if the record had been forged too): each refused.
//!    More copies made here from the bed: project.simpa of another seed, one output byte flipped.
//! C. Fresh runs by this build (`run::run_one`: TCR 5x4x3-a0.2-tcr-air-off and N5), bound by
//!    their run.json; copies with an output byte flipped, another project.simpa, and run.json's
//!    output hashes removed.
//!
//! Writes only under C:\tmp\nm-judge-tamper. Deletes nothing.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Instant;

use serde_json::Value;
use simpa_core::bed::bind::{self, Seal, SealedFile, SealedRun};
use simpa_core::bed::check::{self, Reads, Verdict};
use simpa_core::bed::file;
use simpa_core::bed::read as bread;
use simpa_core::bed::report::{self, Report};
use simpa_core::bed::run::{self, Planned, Read, RunKey};
use simpa_core::bed::transport::{self, Transports};
use simpa_core::run::manifest::hash_tree;

const ROOT: &str = r"C:\tmp\nm-m8a-bed\20260929T093134Z";
const WT: &str = r"B:\repos\I-Simpa_Night_Mode\.claude\worktrees\m8b-tamper";
const UPSTREAM: &str = r"B:\repos\I-Simpa-upstream";
const SCRATCH: &str = r"C:\tmp\nm-judge-tamper";

fn short(s: &str, n: usize) -> String {
    let v: String = s.chars().take(n).collect();
    if s.chars().count() > n { format!("{v}…") } else { v }
}

fn cell(id: &str, seed: u32) -> RunKey {
    RunKey::Cell { id: id.into(), seed }
}

fn tcr_key(id: &str) -> RunKey {
    RunKey::Tcr { id: id.into() }
}

struct Ctx<'a> {
    l: &'a file::Loaded,
    runs: &'a [Planned],
    base: Vec<Result<Read, String>>,
    ts: Transports,
    rep: &'a Report,
    not_run: Option<String>,
    seal: &'a Seal,
}

impl Ctx<'_> {
    fn idx(&self, key: &RunKey) -> usize {
        self.runs.iter().position(|p| &p.key == key).unwrap_or_else(|| panic!("{key:?}"))
    }
    fn planned(&self, key: &RunKey) -> &Planned {
        &self.runs[self.idx(key)]
    }
    fn judge(&self, results: Vec<Result<Read, String>>) -> Report {
        let mut reads = Reads::default();
        run::into_reads(&mut reads, self.runs, results, self.not_run.clone());
        report::evaluate(&report::Inputs {
            bed: &self.l.bed,
            reads: &reads,
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
        self.judge(results)
    }
    /// The run under `root` filed as `key`, read as `simpa bed --from` reads it, with `seal`.
    fn read(&self, root: &Path, key: &RunKey, seal: Option<&Seal>) -> Result<Read, String> {
        let mut r = run::read_in(&root.join(key.dir()), self.planned(key), seal);
        if let Ok(Read::Spps(s)) = &mut r {
            s.curves.clear();
        }
        r
    }
    /// The seal with `key`'s entry made from the folder under `root` as it is now: a forged
    /// record, so that what is left to refuse the tamper is everything after the binding.
    fn forged(&self, root: &Path, keys: &[RunKey]) -> Seal {
        let mut s = self.seal.clone();
        for key in keys {
            let dir = root.join(key.dir());
            let folder = bread::find_run_folder(&dir).unwrap();
            s.runs.insert(
                bind::seal_key(key),
                SealedRun {
                    run_folder: folder.file_name().unwrap().to_str().unwrap().into(),
                    files: hash_tree(&dir, &[])
                        .unwrap()
                        .into_iter()
                        .map(|f| SealedFile(f.path, f.size, f.sha256))
                        .collect(),
                },
            );
        }
        s
    }
}

fn verdict_of(r: &Report, id: &str) -> String {
    r.cells
        .iter()
        .find(|c| c.id == id)
        .map(|c| format!("cell {:?}", c.verdict))
        .or_else(|| r.tcr.iter().find(|t| t.id == id).map(|t| format!("TCR {:?}", t.verdict)))
        .unwrap_or_else(|| "-".into())
}

fn line(name: &str, r: &Report, id: &str) {
    let f: Vec<String> = r
        .failures
        .iter()
        .filter(|f| f.contains(id))
        .take(2)
        .map(|f| short(f, 360))
        .collect();
    println!(
        "  {name}: pass {} | E2 {} | failures {} | {id}: {} | {}",
        r.pass,
        r.preconditions.e2.holds,
        r.failures.len(),
        verdict_of(r, id),
        if f.is_empty() { "-".to_string() } else { f.join(" || ") }
    );
}

fn read_line(name: &str, p: &Planned, r: &Result<Read, String>) {
    let v = match r {
        Ok(read) => match run::check_planned(p, read) {
            Ok(()) => {
                let info = match read {
                    Read::Spps(s) => &s.info,
                    Read::Tcr(t) => &t.info,
                };
                format!(
                    "READ, bound by {:?}, outputs {}, and ACCEPTED by check_planned",
                    info.bound_by,
                    short(info.outputs_sha256.as_deref().unwrap_or("-"), 16)
                )
            }
            Err(e) => format!("read, then REFUSED by check_planned: {}", short(&e, 700)),
        },
        Err(e) => format!("REFUSED: {}", short(e, 700)),
    };
    println!("  {name}: {v}");
}

fn copy_dir(src: &Path, dst: &Path) {
    std::fs::create_dir_all(dst).unwrap();
    for e in std::fs::read_dir(src).unwrap().flatten() {
        let (s, d) = (e.path(), dst.join(e.file_name()));
        if e.file_type().unwrap().is_dir() {
            copy_dir(&s, &d);
        } else {
            std::fs::copy(&s, &d).unwrap();
        }
    }
}

/// Copies `src` to `dst` and applies `change` once: nothing when `dst` is there (a re-run).
fn case(src: &Path, dst: &Path, change: impl FnOnce(&Path)) {
    if dst.exists() {
        return;
    }
    copy_dir(src, dst);
    change(dst);
}

fn flip_last_byte(path: &Path) {
    let mut b = std::fs::read(path).unwrap();
    let n = b.len() - 1;
    b[n] ^= 1;
    std::fs::write(path, b).unwrap();
}

/// Numeric leaves compared between two JSON values; keys in `ignore` skipped.
#[derive(Default)]
struct Diff {
    numbers: usize,
    differing: usize,
    max_abs: f64,
    max_at: String,
    other: Vec<String>,
}

fn diff(a: &Value, b: &Value, at: &str, ignore: &[&str], d: &mut Diff) {
    match (a, b) {
        (Value::Number(x), Value::Number(y)) => {
            d.numbers += 1;
            let (x, y) = (x.as_f64().unwrap(), y.as_f64().unwrap());
            let e = (x - y).abs();
            if x.to_bits() != y.to_bits() {
                d.differing += 1;
            }
            if e > d.max_abs {
                d.max_abs = e;
                d.max_at = at.to_string();
            }
        }
        (Value::Object(x), Value::Object(y)) => {
            let keys: std::collections::BTreeSet<&String> = x.keys().chain(y.keys()).collect();
            for k in keys.into_iter().filter(|k| !ignore.contains(&k.as_str())) {
                match (x.get(k), y.get(k)) {
                    (Some(p), Some(q)) => diff(p, q, &format!("{at}.{k}"), ignore, d),
                    _ => d.other.push(format!("{at}.{k}: in one only")),
                }
            }
        }
        (Value::Array(x), Value::Array(y)) => {
            if x.len() != y.len() {
                d.other.push(format!("{at}: {} against {} items", x.len(), y.len()));
            }
            for (i, (p, q)) in x.iter().zip(y).enumerate() {
                diff(p, q, &format!("{at}[{i}]"), ignore, d);
            }
        }
        _ => {
            if a != b {
                d.other.push(format!("{at}: {} against {}", short(&a.to_string(), 80), short(&b.to_string(), 80)));
            }
        }
    }
}

fn main() {
    let jobs: usize = std::env::var("JOBS").ok().and_then(|s| s.parse().ok()).unwrap_or(8);
    let t_all = Instant::now();
    let root = PathBuf::from(ROOT);
    let wt = PathBuf::from(WT);
    let l = file::load(&wt.join("beds").join("m8a.json")).unwrap();
    println!("bed file beds/m8a.json: exploratory {:?}", l.exploratory);
    // The crate's exact reader: serde_json's own does not round every number correctly.
    let rep_value =
        simpa_core::schema::parse_json(&std::fs::read_to_string(root.join("report.json")).unwrap())
            .unwrap();
    let rep: Report = serde_json::from_value(rep_value.clone()).unwrap();
    println!("M8a report.json: pass {}, {} failures", rep.pass, rep.failures.len());
    let seal_path = wt.join(r"beds\m8a-20260929T093134Z\outputs-seal.json");
    let (seal, seal_sha) = Seal::load(&seal_path).unwrap();
    seal.for_bed(&root).unwrap();
    println!(
        "seal {}: sha256 {seal_sha}, bed {}, {} runs, {} files, {} bytes",
        seal_path.display(),
        seal.bed,
        seal.runs.len(),
        seal.files,
        seal.bytes
    );
    let (runs, not_run) = run::plan(&l.bed, Some(Path::new(UPSTREAM))).unwrap();
    println!("planned {} runs; atmospheric not run: {not_run:?}", runs.len());
    let mut ts = Transports::default();
    for x in &rep.transports {
        ts.by_key.insert(transport::key(&x.room, x.alpha, x.air_m_per_metre), Ok(x.clone()));
    }

    // ---------------------------------------------------------------------------------------
    println!("\nA1. the M8a bed read with the seal ({jobs} at once):");
    let t0 = Instant::now();
    let mut base = run::read_existing(&runs, &root, jobs, Some(&seal));
    for r in &mut base {
        if let Ok(Read::Spps(s)) = r {
            s.curves.clear();
        }
    }
    let secs = t0.elapsed().as_secs_f64();
    let mut by: BTreeMap<String, usize> = BTreeMap::new();
    let mut errors = Vec::new();
    for (p, r) in runs.iter().zip(&base) {
        match r {
            Ok(read) => {
                let info = match read {
                    Read::Spps(s) => &s.info,
                    Read::Tcr(t) => &t.info,
                };
                *by.entry(info.bound_by.clone().unwrap_or_default()).or_default() += 1;
                if let Err(e) = run::check_planned(p, read) {
                    errors.push(format!("{}: {}", p.key.label(), short(&e, 400)));
                }
            }
            Err(e) => errors.push(format!("{}: NOT READ {}", p.key.label(), short(e, 400))),
        }
    }
    println!("  {} runs read in {secs:.0} s; bound by {by:?}; {} refused or not read", runs.len(), errors.len());
    for e in errors.iter().take(20) {
        println!("    {e}");
    }
    let ctx = Ctx { l: &l, runs: &runs, base, ts, rep: &rep, not_run, seal: &seal };
    let judged = ctx.judge(ctx.base.clone());
    println!(
        "  re-judged: pass {}, {} failures, E1 {} ({} runs), E2 {}, cells {:?}",
        judged.pass,
        judged.failures.len(),
        judged.preconditions.e1.holds,
        judged.preconditions.e1.runs_checked,
        judged.preconditions.e2.holds,
        {
            let mut m: BTreeMap<String, usize> = BTreeMap::new();
            for c in &judged.cells {
                *m.entry(format!("{:?}", c.verdict)).or_default() += 1;
            }
            m
        }
    );
    for f in judged.failures.iter().take(10) {
        println!("    FAIL {}", short(f, 300));
    }
    // Keys the M8a report does not have: M8b's, and step 1's project_sha256.
    let new_keys = ["bound_by", "outputs_sha256", "on_disk", "run_analytic_eyring_s", "project_sha256"];
    let a = simpa_core::schema::parse_json(&serde_json::to_string(&judged).unwrap()).unwrap();
    let b = &rep_value;
    for section in [
        "pass", "exploratory", "failures", "needs_extension", "preconditions", "cells", "tcr",
        "say_no", "atmospheric_validation", "random_against_energetic", "transports",
        "reported_notes",
    ] {
        let mut d = Diff::default();
        diff(&a[section], &b[section], section, &new_keys, &mut d);
        // The other differences, by their last key and what they are.
        let mut kinds: BTreeMap<String, usize> = BTreeMap::new();
        for o in &d.other {
            let (path, what) = o.split_once(": ").unwrap_or((o, ""));
            let last = path.rsplit('.').next().unwrap_or(path);
            *kinds.entry(format!("{last}: {what}")).or_default() += 1;
        }
        println!(
            "  against report.json, {section}: {} numbers, {} not bit-identical, largest |difference| {:e} ({}); other differences {}: {:?}",
            d.numbers,
            d.differing,
            d.max_abs,
            if d.max_at.is_empty() { "-" } else { &d.max_at },
            d.other.len(),
            kinds
        );
    }
    // D's reference: the planned project's analytic time against the run's own (report.json's).
    let mut worst = (0.0f64, String::new());
    let mut bands = 0;
    for t in &judged.tcr {
        let old = rep.tcr.iter().find(|x| x.id == t.id).unwrap();
        for (x, y) in t.d.as_ref().unwrap().bands.iter().zip(&old.d.as_ref().unwrap().bands) {
            bands += 1;
            let rel = x.analytic_eyring_s.unwrap() / y.analytic_eyring_s.unwrap() - 1.0;
            if rel.abs() >= worst.0 {
                worst = (rel.abs(), format!("{} {} Hz", t.id, x.freq_hz));
            }
            assert_eq!(x.run_analytic_eyring_s, y.analytic_eyring_s);
            assert_eq!(x.verdict, y.verdict);
        }
    }
    println!(
        "  D: {bands} bands; the planned project's analytic time against the run's own (report.json's): largest relative difference {:e} ({}); every band's run_analytic_eyring_s is report.json's analytic_eyring_s, every verdict the same",
        worst.0, worst.1
    );
    let t30_same = judged.cells.iter().zip(&rep.cells).all(|(x, y)| {
        x.seeds.iter().zip(&y.seeds).all(|(s, t)| s.t30 == t.t30) && x.seeds.len() == y.seeds.len()
    });
    println!("  every seed's T30 array equal to report.json's: {t30_same}");

    println!("\nA2. the same bed read without the seal:");
    let unsealed = run::read_existing(&runs, &root, jobs, None);
    let unbound = unsealed
        .iter()
        .filter(|r| r.as_ref().err().is_some_and(|e| e.starts_with(bind::RUN_UNBOUND)))
        .count();
    println!("  {} of {} runs refused {}", unbound, runs.len(), bind::RUN_UNBOUND);
    if let Some(Err(e)) = unsealed.first() {
        println!("  e.g. {}", short(e, 300));
    }
    let r = ctx.judge(unsealed);
    println!("  judged: pass {}, E2 {}, {} failures", r.pass, r.preconditions.e2.holds, r.failures.len());
    // One run whose entry the seal lacks.
    let k = cell("5x4x3-a0.4-energetic-air-off", 3);
    let mut less = seal.clone();
    less.runs.remove(&bind::seal_key(&k));
    let r = ctx.read(&root, &k, Some(&less));
    read_line("A3. a seal without 5x4x3-a0.4-energetic-air-off s3's entry, that run", ctx.planned(&k), &r);
    line("A3 in place", &ctx.judge_with(vec![(k.clone(), r)]), "5x4x3-a0.4-energetic-air-off");

    // ---------------------------------------------------------------------------------------
    let cases = Path::new(SCRATCH).join("cases");
    let id = "5x4x3-a0.4-energetic-air-off";
    println!("\nB1. the judge's swap-seed: {id} s3's folder with s4's 20 output files (its run.json, config.xml, mesh its own):");
    let c = cases.join("swap-seed");
    let r = ctx.read(&c, &cell(id, 3), Some(&seal));
    read_line("real seal", ctx.planned(&cell(id, 3)), &r);
    line("real seal, in place of s3", &ctx.judge_with(vec![(cell(id, 3), r)]), id);
    let forged = ctx.forged(&c, &[cell(id, 3)]);
    let r = ctx.read(&c, &cell(id, 3), Some(&forged));
    read_line("forged seal", ctx.planned(&cell(id, 3)), &r);
    let j = ctx.judge_with(vec![(cell(id, 3), r)]);
    line("forged seal, in place of s3", &j, id);
    for s in &j.cells.iter().find(|x| x.id == id).unwrap().seeds {
        if let Some(e) = &s.error {
            println!("    seed {}: {}", s.seed, short(e, 300));
        }
    }

    println!("\nB2. the judge's swap-all-to-s1: {id} seeds 2 to 10 each with s1's outputs:");
    let c = cases.join("swap-all-to-s1");
    let keys: Vec<RunKey> = (2..=10).map(|s| cell(id, s)).collect();
    let real: Vec<(RunKey, Result<Read, String>)> =
        keys.iter().map(|k| (k.clone(), ctx.read(&c, k, Some(&seal)))).collect();
    let n = real.iter().filter(|(_, r)| r.as_ref().err().is_some_and(|e| e.starts_with(bind::FILES_CHANGED))).count();
    println!("  real seal: {n} of 9 refused {}", bind::FILES_CHANGED);
    read_line("real seal, s2", ctx.planned(&keys[0]), &real[0].1);
    line("real seal, in place", &ctx.judge_with(real), id);
    let forged = ctx.forged(&c, &keys);
    let fr: Vec<(RunKey, Result<Read, String>)> =
        keys.iter().map(|k| (k.clone(), ctx.read(&c, k, Some(&forged)))).collect();
    read_line("forged seal, s2", ctx.planned(&keys[0]), &fr[0].1);
    let j = ctx.judge_with(fr);
    line("forged seal, in place", &j, id);
    let refused = j.cells.iter().find(|x| x.id == id).unwrap().seeds.iter()
        .filter(|s| s.error.as_deref().is_some_and(|e| e.starts_with(bind::SEEDS_IDENTICAL))).count();
    println!("    seeds refused {}: {refused} of 10", bind::SEEDS_IDENTICAL);

    let (a02, a04) = ("5x4x3-a0.2-tcr-air-off", "5x4x3-a0.4-tcr-air-off");
    println!("\nB3. the judge's tcr-edited-sha: {a02}'s run filed as {a04}, run.json's sha256 edited to {a04}'s plan:");
    let c = cases.join("tcr-edited-sha");
    let r = ctx.read(&c, &tcr_key(a04), Some(&seal));
    read_line("real seal", ctx.planned(&tcr_key(a04)), &r);
    line("real seal, in place", &ctx.judge_with(vec![(tcr_key(a04), r)]), a04);
    let forged = ctx.forged(&c, &[tcr_key(a04)]);
    let r = ctx.read(&c, &tcr_key(a04), Some(&forged));
    read_line("forged seal", ctx.planned(&tcr_key(a04)), &r);
    line("forged seal, in place", &ctx.judge_with(vec![(tcr_key(a04), r)]), a04);
    // D alone: α 0.2's own run, read under its own key, judged as α 0.4's TCR run.
    let own = ctx.read(&root, &tcr_key(a02), Some(&seal));
    let Ok(Read::Tcr(t)) = own else { panic!("{a02} not read") };
    let t04 = l.bed.tcr.iter().find(|t| t.id == a04).unwrap();
    let d = check::evaluate_tcr(&l.bed, t04, Some(&Ok(*t)), None);
    println!("  D alone ({a02}'s real outputs judged as {a04}): {:?}", d.verdict);
    for b in &d.d.as_ref().unwrap().bands {
        println!(
            "    {} Hz: TCR {:.6} s, the {a04} plan's analytic {:.6} s, d {:+.3} %, {:?}; against the run's own analytic {:.6} s it would be {:+.2e}",
            b.freq_hz,
            b.tcr_eyring_s,
            b.analytic_eyring_s.unwrap(),
            100.0 * b.deviation.unwrap(),
            b.verdict,
            b.run_analytic_eyring_s.unwrap(),
            b.tcr_eyring_s / b.run_analytic_eyring_s.unwrap() - 1.0
        );
    }

    println!("\nB4. {id} s3 with s4's project.simpa (a copy made here):");
    let c = cases.join("project-simpa");
    case(&root.join(cell(id, 3).dir()), &c.join(cell(id, 3).dir()), |d| {
        std::fs::copy(root.join(cell(id, 4).dir()).join("project.simpa"), d.join("project.simpa")).unwrap();
    });
    let r = ctx.read(&c, &cell(id, 3), Some(&seal));
    read_line("real seal", ctx.planned(&cell(id, 3)), &r);
    let forged = ctx.forged(&c, &[cell(id, 3)]);
    let r = ctx.read(&c, &cell(id, 3), Some(&forged));
    read_line("forged seal", ctx.planned(&cell(id, 3)), &r);
    line("forged seal, in place", &ctx.judge_with(vec![(cell(id, 3), r)]), id);

    println!("\nB5. {id} s3 with the last byte of R000's Sound level.recp flipped (a copy made here):");
    let c = cases.join("flipped-recp");
    case(&root.join(cell(id, 3).dir()), &c.join(cell(id, 3).dir()), |d| {
        let run = bread::find_run_folder(d).unwrap();
        flip_last_byte(&run.join(r"solve\Punctual receivers\R000\Sound level.recp"));
    });
    let r = ctx.read(&c, &cell(id, 3), Some(&seal));
    read_line("real seal", ctx.planned(&cell(id, 3)), &r);
    line("real seal, in place", &ctx.judge_with(vec![(cell(id, 3), r)]), id);

    // ---------------------------------------------------------------------------------------
    println!("\nC. fresh runs by this build (run::run_one), bound by their run.json:");
    let fresh = Path::new(SCRATCH).join("fresh");
    let exes = run::Exes::find().unwrap();
    let n5 = runs.iter().find(|p| matches!(p.key, RunKey::N5 { .. })).unwrap().clone();
    for key in [tcr_key(a02), n5.key.clone()] {
        let p = ctx.planned(&key);
        let dir = fresh.join(key.dir());
        let r = if bread::find_run_folder(&dir).is_some() {
            run::read_in(&dir, p, None)
        } else {
            run::run_one(p, &fresh, &exes)
        };
        let folder = bread::find_run_folder(&dir).unwrap();
        let m = bread::manifest_of(&folder).unwrap();
        println!(
            "  {}: {} ; run.json status {:?}, outputs recorded {}",
            key.label(),
            folder.display(),
            m.verdict.status,
            m.outputs.as_ref().map_or(0, |o| o.len())
        );
        read_line("  read", p, &r);
    }
    let kt = tcr_key(a02);
    let c = cases.join("fresh-flipped");
    case(&fresh.join(kt.dir()), &c.join(kt.dir()), |d| {
        let run = bread::find_run_folder(d).unwrap();
        flip_last_byte(&run.join(r"solve\Main results.gabe"));
    });
    let r = run::read_in(&c.join(kt.dir()), ctx.planned(&kt), None);
    read_line("C1. fresh TCR run, one byte of Main results.gabe flipped", ctx.planned(&kt), &r);
    let c = cases.join("fresh-project");
    let k10 = cell(&l.bed.say_no.n5.cell, 10);
    case(&fresh.join(n5.key.dir()), &c.join(n5.key.dir()), |d| {
        simpa_core::schema::save(&ctx.planned(&k10).project, &d.join("project.simpa")).unwrap();
    });
    let r = run::read_in(&c.join(n5.key.dir()), &n5, None);
    read_line("C2. fresh N5 run, project.simpa replaced by its cell's seed-10 project", &n5, &r);
    let c = cases.join("fresh-unbound");
    case(&fresh.join(kt.dir()), &c.join(kt.dir()), |d| {
        let run = bread::find_run_folder(d).unwrap();
        let p = run.join("run.json");
        let mut v: Value = serde_json::from_str(&std::fs::read_to_string(&p).unwrap()).unwrap();
        v.as_object_mut().unwrap().remove("outputs");
        std::fs::write(&p, serde_json::to_string_pretty(&v).unwrap() + "\n").unwrap();
    });
    let r = run::read_in(&c.join(kt.dir()), ctx.planned(&kt), None);
    read_line("C3. fresh TCR run, run.json's outputs removed, no seal", ctx.planned(&kt), &r);

    println!("\ndone in {:.0} s", t_all.elapsed().as_secs_f64());
    let _ = Verdict::Pass;
}
