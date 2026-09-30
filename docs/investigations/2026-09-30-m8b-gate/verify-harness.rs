//! M8b adversarial verification of the two gate fixes (8c2e603), on the M8a bed read-only.
//!
//! A. False positives: every one of the bed's 433 runs read with this build and matched with its
//!    plan; the project file on disk, run.json's recorded sha256 and the plan's compared
//!    independently; config.xml's seed, particles and step compared as text; band counts.
//! B. The judge's break cases replayed: TCR 8 kHz removed / only 125 Hz (after and before
//!    into_reads); a real exploratory bed with the same cell ids (the M8a smoke bed) read as
//!    --from; real 10 ms runs made by this build (run_one) filed under the 1 ms plan.
//! C. Attempts to get around each fix: band lists with the right count, duplicates, single
//!    settings changed on real reads, real runs of other settings, a seed under the wrong
//!    folder, and tampered copies (outputs of another run under a run.json whose project sha256
//!    matches; run.json's sha256 edited).
//!
//! Reads C:\tmp\nm-m8a-bed\20260929T093134Z and C:\tmp\nm-m8a-smoke\20260929T072041Z, never
//! writes there. Everything written is under C:\tmp\nm-judge-m8b.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Instant;

use simpa_core::bed::check::{self, Reads, Verdict};
use simpa_core::bed::file;
use simpa_core::bed::read as bread;
use simpa_core::bed::report::{self, Report};
use simpa_core::bed::run::{self, Planned, Read, RunKey};
use simpa_core::bed::transport::{self, Transports};

const ROOT: &str = r"C:\tmp\nm-m8a-bed\20260929T093134Z";
const SMOKE: &str = r"C:\tmp\nm-m8a-smoke\20260929T072041Z";
const BED: &str = r"B:\repos\I-Simpa_Night_Mode\.claude\worktrees\m8b\beds\m8a.json";
const UPSTREAM: &str = r"B:\repos\I-Simpa-upstream";
const SCRATCH: &str = r"C:\tmp\nm-judge-m8b";

fn kind(k: &RunKey) -> &'static str {
    match k {
        RunKey::Cell { .. } => "cell",
        RunKey::Tcr { .. } => "tcr",
        RunKey::AtmosphericSpps { .. } => "atmospheric-spps",
        RunKey::AtmosphericTcr => "atmospheric-tcr",
        RunKey::N5 { .. } => "n5",
        RunKey::N6 { .. } => "n6",
    }
}

fn short(s: &str, n: usize) -> String {
    let v: String = s.chars().take(n).collect();
    if s.chars().count() > n { format!("{v}…") } else { v }
}

fn cell(id: &str, seed: u32) -> RunKey {
    RunKey::Cell {
        id: id.into(),
        seed,
    }
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
}

impl Ctx<'_> {
    fn idx(&self, key: &RunKey) -> usize {
        self.runs
            .iter()
            .position(|p| &p.key == key)
            .unwrap_or_else(|| panic!("no planned run {key:?}"))
    }
    fn planned(&self, key: &RunKey) -> &Planned {
        &self.runs[self.idx(key)]
    }
    fn base_read(&self, key: &RunKey) -> Read {
        self.base[self.idx(key)].clone().unwrap()
    }
    fn reads_with(&self, subst: Vec<(RunKey, Result<Read, String>)>) -> Reads {
        let mut results = self.base.clone();
        for (k, r) in subst {
            let i = self.idx(&k);
            results[i] = r;
        }
        let mut reads = Reads::default();
        run::into_reads(&mut reads, self.runs, results, self.not_run.clone());
        reads
    }
    fn judge(&self, reads: &Reads) -> Report {
        report::evaluate(&report::Inputs {
            bed: &self.l.bed,
            reads,
            transports: &self.ts,
            exploratory: &self.l.exploratory,
            solvers: &self.rep.preconditions.e1.solvers,
        })
    }
    fn judge_with(&self, subst: Vec<(RunKey, Result<Read, String>)>) -> Report {
        self.judge(&self.reads_with(subst))
    }
}

fn line(name: &str, r: &Report, id: &str) {
    let v = r
        .cells
        .iter()
        .find(|c| c.id == id)
        .map(|c| format!("cell {:?}", c.verdict))
        .or_else(|| {
            r.tcr
                .iter()
                .find(|t| t.id == id)
                .map(|t| format!("TCR {:?}", t.verdict))
        })
        .unwrap_or_else(|| "-".into());
    let f: Vec<String> = r
        .failures
        .iter()
        .filter(|f| f.contains(id))
        .take(2)
        .map(|f| short(f, 330))
        .collect();
    println!(
        "  {name}: pass {} | E2 {} | failures {} | {id}: {v} | {}",
        r.pass,
        r.preconditions.e2.holds,
        r.failures.len(),
        if f.is_empty() { "-".to_string() } else { f.join(" || ") }
    );
}

fn planned_verdict(p: &Planned, r: &Result<Read, String>) -> String {
    match r {
        Ok(read) => match run::check_planned(p, read) {
            Ok(()) => "ACCEPTED by check_planned".into(),
            Err(e) => format!("REFUSED: {}", short(&e, 520)),
        },
        Err(e) => format!("NOT READ: {}", short(e, 300)),
    }
}

fn clear_curves(r: &mut Result<Read, String>) {
    if let Ok(Read::Spps(s)) = r {
        s.curves.clear();
    }
}

fn spps_mut(r: &mut Read) -> &mut bread::SppsRead {
    match r {
        Read::Spps(s) => s,
        Read::Tcr(_) => panic!("not SPPS"),
    }
}

fn tcr_mut(r: &mut Read) -> &mut bread::TcrRead {
    match r {
        Read::Tcr(t) => t,
        Read::Spps(_) => panic!("not TCR"),
    }
}

fn attr(text: &str, name: &str) -> Option<String> {
    let pat = format!(" {name}=\"");
    let i = text.find(&pat)? + pat.len();
    let j = text[i..].find('"')? + i;
    Some(text[i..j].to_string())
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

fn files_under(dir: &Path, rel: &Path, out: &mut Vec<PathBuf>) {
    for e in std::fs::read_dir(dir).unwrap().flatten() {
        let r = rel.join(e.file_name());
        if e.file_type().unwrap().is_dir() {
            files_under(&e.path(), &r, out);
        } else {
            out.push(r);
        }
    }
}

/// Copies the planned run folder `src_slot` (runs/<id>/s<seed>, project.simpa and the run folder)
/// to `dst_slot`, unless it is there already; returns the copy's run folder.
fn copy_slot(src_slot: &Path, dst_slot: &Path) -> PathBuf {
    if bread::find_run_folder(dst_slot).is_none() {
        copy_dir(src_slot, dst_slot);
    }
    bread::find_run_folder(dst_slot).unwrap()
}

/// Overwrites every file of `run`'s solve/ that run.json does not list as an input with
/// `other`'s: the outputs are `other`'s, config.xml, the mesh and run.json stay `run`'s.
fn swap_outputs(run: &Path, other: &Path) -> usize {
    let m: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(run.join("run.json")).unwrap()).unwrap();
    let inputs: Vec<String> = m["inputs"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["path"].as_str().unwrap().to_string())
        .collect();
    let mut files = Vec::new();
    files_under(&other.join("solve"), Path::new(""), &mut files);
    let mut n = 0;
    for rel in files {
        let key = rel.to_string_lossy().replace('\\', "/");
        if inputs.contains(&key) {
            continue;
        }
        let d = run.join("solve").join(&rel);
        std::fs::create_dir_all(d.parent().unwrap()).unwrap();
        std::fs::copy(other.join("solve").join(&rel), &d).unwrap();
        n += 1;
    }
    n
}

/// Sets run.json's source.sha256 to `sha`.
fn edit_project_sha(run: &Path, sha: &str) {
    let p = run.join("run.json");
    let mut m: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&p).unwrap()).unwrap();
    m["source"]["sha256"] = serde_json::Value::String(sha.into());
    std::fs::write(&p, serde_json::to_string_pretty(&m).unwrap()).unwrap();
}

fn t30_equal(a: &Read, b: &Read) -> bool {
    match (a, b) {
        (Read::Spps(a), Read::Spps(b)) => a.t30 == b.t30,
        _ => false,
    }
}

fn main() {
    let jobs: usize = std::env::var("JOBS").ok().and_then(|s| s.parse().ok()).unwrap_or(8);
    let t_all = Instant::now();
    let l = file::load(Path::new(BED)).unwrap();
    println!("bed file {BED}: exploratory {:?}", l.exploratory);
    let root = PathBuf::from(ROOT);
    let rep: Report =
        serde_json::from_str(&std::fs::read_to_string(root.join("report.json")).unwrap()).unwrap();
    println!(
        "M8a report.json: pass {} exploratory {} E1 runs checked {}",
        rep.pass, rep.exploratory, rep.preconditions.e1.runs_checked
    );
    let (runs, not_run) = run::plan(&l.bed, Some(Path::new(UPSTREAM))).unwrap();
    println!("planned {} runs; atmospheric not run: {not_run:?}", runs.len());

    // ------------------------------------------------------------------------------------
    println!("\n== A. False positives: every run of the M8a bed against its plan");
    let t0 = Instant::now();
    let mut base = run::read_existing(&runs, &root, jobs);
    println!("read {} runs in {:.0} s ({jobs} at once)", runs.len(), t0.elapsed().as_secs_f64());
    let mut by: BTreeMap<&str, [usize; 3]> = BTreeMap::new();
    let mut refused = Vec::new();
    let mut bands: BTreeMap<String, BTreeMap<usize, usize>> = BTreeMap::new();
    let (mut sha_disk_eq_json, mut sha_json_eq_plan, mut sha_n) = (0, 0, 0);
    let mut text_problems = Vec::new();
    for (p, r) in runs.iter().zip(&base) {
        let e = by.entry(kind(&p.key)).or_default();
        match r {
            Ok(read) => match run::check_planned(p, read) {
                Ok(()) => e[0] += 1,
                Err(x) => {
                    e[1] += 1;
                    refused.push(x);
                }
            },
            Err(x) => {
                e[2] += 1;
                refused.push(format!("{}: not read: {x}", p.key.label()));
            }
        }
        let slot = root.join(p.key.dir());
        // The project file on disk, run.json's record of it, and the plan's: three sha256s.
        if let Ok(read) = r {
            let info = match read {
                Read::Spps(s) => &s.info,
                Read::Tcr(t) => &t.info,
            };
            let disk = simpa_core::run::manifest::sha256_file(&slot.join("project.simpa")).ok();
            sha_n += 1;
            if disk.is_some() && disk == info.project_sha256 {
                sha_disk_eq_json += 1;
            }
            if info.project_sha256.as_deref() == Some(run::planned_project_sha256(p).as_str()) {
                sha_json_eq_plan += 1;
            }
            let air = match &p.key {
                RunKey::Cell { id, .. } | RunKey::Tcr { id } => {
                    if id.ends_with("air-on") { "air-on" } else { "air-off" }
                }
                _ => "",
            };
            let n = match read {
                Read::Spps(s) => s.bands_hz.len(),
                Read::Tcr(t) => t.bands.len(),
            };
            *bands
                .entry(format!("{} {air}", kind(&p.key)))
                .or_default()
                .entry(n)
                .or_default() += 1;
        }
        // config.xml as text: seed, particles, step (SPPS).
        if p.solver == simpa_core::schema::SolverKind::Spps {
            let folder = bread::find_run_folder(&slot).unwrap();
            let text = std::fs::read_to_string(folder.join("solve").join("config.xml")).unwrap();
            let s = &p.project.solvers.spps;
            let want_seed = match &p.key {
                RunKey::Cell { seed, .. }
                | RunKey::AtmosphericSpps { seed }
                | RunKey::N5 { seed }
                | RunKey::N6 { seed } => *seed,
                _ => 0,
            };
            let got_seed = attr(&text, "random_seed");
            let got_n = attr(&text, "nbparticules");
            let got_dt = attr(&text, "pasdetemps");
            if got_seed.as_deref() != Some(want_seed.to_string().as_str())
                || got_n.as_deref() != Some(s.particles_per_source.to_string().as_str())
                || got_dt.as_deref() != Some("0.001")
            {
                text_problems.push(format!(
                    "{}: seed {got_seed:?} particles {got_n:?} step {got_dt:?}",
                    p.key.label()
                ));
            }
        }
    }
    println!("by kind [accepted, refused by check_planned, not read]: {by:?}");
    for x in refused.iter().take(10) {
        println!("  {}", short(x, 400));
    }
    println!(
        "project sha256: on disk = run.json's in {sha_disk_eq_json} of {sha_n}; run.json's = the plan's in {sha_json_eq_plan} of {sha_n}"
    );
    println!("band counts by kind (count: runs): {bands:?}");
    println!(
        "config.xml text (seed = folder seed, particles = plan's, step 0.001) problems: {} {:?}",
        text_problems.len(),
        text_problems.iter().take(5).collect::<Vec<_>>()
    );
    let n5p = runs.iter().find(|p| matches!(p.key, RunKey::N5 { .. })).unwrap();
    let n6p = runs.iter().find(|p| matches!(p.key, RunKey::N6 { .. })).unwrap();
    println!(
        "N5 plan: {} particles, seed {}; N6 plan: scattering {:?}",
        n5p.project.solvers.spps.particles_per_source,
        n5p.project.solvers.spps.random_seed,
        n6p.project.materials.iter().map(|m| m.scattering.iter().map(|s| s.get()).collect::<Vec<_>>()).collect::<Vec<_>>()
    );
    for r in base.iter_mut() {
        clear_curves(r);
    }
    let mut ts = Transports::default();
    for x in &rep.transports {
        ts.by_key.insert(
            transport::key(&x.room, x.alpha, x.air_m_per_metre),
            Ok(x.clone()),
        );
    }
    let ctx = Ctx {
        l: &l,
        runs: &runs,
        base,
        ts,
        rep: &rep,
        not_run,
    };
    let reads0 = ctx.reads_with(vec![]);
    let j0 = ctx.judge(&reads0);
    let same_cells = j0.cells.iter().zip(&rep.cells).filter(|(a, b)| a.id == b.id && a.verdict == b.verdict).count();
    let same_tcr = j0.tcr.iter().zip(&rep.tcr).filter(|(a, b)| a.id == b.id && a.verdict == b.verdict).count();
    let bp: usize = j0.tcr.iter().filter_map(|t| t.d.as_ref()).map(|d| d.band_problems.len()).sum();
    println!(
        "baseline re-judged: pass {} | failures {} | E2 {} | cells as report {same_cells}/{} | TCR as report {same_tcr}/{} | D band problems {bp} | N5 as_required {} error {:?} | N6 as_required {} error {:?} | atmospheric runs with error {}",
        j0.pass,
        j0.failures.len(),
        j0.preconditions.e2.holds,
        rep.cells.len(),
        rep.tcr.len(),
        j0.say_no.n5.as_required,
        j0.say_no.n5.error,
        j0.say_no.n6.as_required,
        j0.say_no.n6.error,
        j0.atmospheric_validation.runs.iter().filter(|r| r.error.is_some()).count()
    );

    // ------------------------------------------------------------------------------------
    println!("\n== B. The judge's break cases, on the fixed code");
    let tid = "6x10x3-a0.2-tcr-air-on";
    println!("B1. after into_reads, as the judge's harness did (only D sees it):");
    for (name, keep) in [("8 kHz removed", 6usize), ("only 125 Hz", 1)] {
        let mut r = reads0.clone();
        r.tcr.get_mut(tid).unwrap().as_mut().unwrap().bands.truncate(keep);
        line(name, &ctx.judge(&r), tid);
    }
    println!("B2. before into_reads (what a run folder with those bands would give):");
    for (name, keep) in [("8 kHz removed", 6usize), ("only 125 Hz", 1)] {
        let mut read = ctx.base_read(&tcr_key(tid));
        tcr_mut(&mut read).bands.truncate(keep);
        line(name, &ctx.judge_with(vec![(tcr_key(tid), Ok(read))]), tid);
    }

    println!("B3. --from over a real exploratory bed with the same cell ids ({SMOKE}):");
    let smoke = PathBuf::from(SMOKE);
    let mut subst = Vec::new();
    for p in &runs {
        let dir = smoke.join(p.key.dir());
        if !dir.is_dir() {
            continue;
        }
        let r = run::read_in(&dir, p);
        println!("  {}: {}", p.key.label(), planned_verdict(p, &r));
        if let (RunKey::Tcr { id }, Ok(Read::Tcr(t))) = (&p.key, &r) {
            let def = l.bed.tcr.iter().find(|x| &x.id == id).unwrap();
            let d = check::evaluate_tcr(&l.bed, def, Some(&Ok((**t).clone())), None);
            println!(
                "    the same TCR read judged by D alone (no check_planned): {:?}, {} bands",
                d.verdict,
                t.bands.len()
            );
        }
        subst.push((p.key.clone(), r));
    }
    let n_sub = subst.len();
    let j = ctx.judge_with(subst);
    for id in ["5x4x3-a0.4-energetic-air-off", "5x4x3-a0.4-tcr-air-on"] {
        line(&format!("M8a bed with its {n_sub} keys the smoke bed has read from it"), &j, id);
    }
    println!(
        "    failures naming bed_run_not_planned: {}",
        j.failures.iter().filter(|f| f.contains("bed_run_not_planned")).count()
    );

    println!("B4. real 10 ms runs made by this build (run::run_one) filed under the 1 ms plan:");
    let mut ten = l.bed.clone();
    ten.time_step_s = 0.01;
    let (plan10, _) = run::plan(&ten, None).unwrap();
    let root10 = Path::new(SCRATCH).join("bed10");
    let exes = run::Exes::find().unwrap();
    let mut made10: BTreeMap<RunKey, Result<Read, String>> = BTreeMap::new();
    for key in [tcr_key("5x4x3-a0.4-tcr-air-off"), cell("5x4x3-a0.4-energetic-air-off", 1)] {
        let p10 = plan10.iter().find(|p| p.key == key).unwrap();
        let dir = root10.join(key.dir());
        let t = Instant::now();
        let mut r = if bread::find_run_folder(&dir).is_some() {
            run::read_in(&dir, p10)
        } else {
            run::run_one(p10, &root10, &exes)
        };
        clear_curves(&mut r);
        println!(
            "  {} at 10 ms ({:.0} s): under its own 10 ms plan (fresh path): {}",
            key.label(),
            t.elapsed().as_secs_f64(),
            planned_verdict(p10, &r)
        );
        println!("    under the M8a plan: {}", planned_verdict(ctx.planned(&key), &r));
        if let Ok(Read::Tcr(tr)) = &r {
            let def = l.bed.tcr.iter().find(|x| x.id == "5x4x3-a0.4-tcr-air-off").unwrap();
            let d = check::evaluate_tcr(&l.bed, def, Some(&Ok((**tr).clone())), None);
            println!("    the 10 ms TCR read judged by D alone (no check_planned): {:?}", d.verdict);
        }
        if let Ok(Read::Spps(s)) = &r {
            println!("    it read time_step_s {:?}, status {}", s.time_step_s, s.info.status);
        }
        made10.insert(key, r);
    }
    let j = ctx.judge_with(made10.clone().into_iter().collect());
    line("both 10 ms runs in place", &j, "5x4x3-a0.4-energetic-air-off");
    line("both 10 ms runs in place", &j, "5x4x3-a0.4-tcr-air-off");

    // ------------------------------------------------------------------------------------
    println!("\n== C. Getting around the fixes");
    println!("C1. TCR band lists (6x10x3-a0.2-tcr-air-on, 7 bands; and an air-off run, 6):");
    let base_t = ctx.base_read(&tcr_key(tid));
    let tb = match &base_t {
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
    let cases: Vec<(&str, Vec<bread::TcrBand>)> = vec![
        ("right count, 8 kHz relabelled 16 kHz", with(&[(0, 125), (1, 250), (2, 500), (3, 1000), (4, 2000), (5, 4000), (6, 16000)])),
        ("right count, 4 kHz twice, no 8 kHz", with(&[(0, 125), (1, 250), (2, 500), (3, 1000), (4, 2000), (5, 4000), (5, 4000)])),
        ("right count, 125 Hz twice, no 8 kHz", with(&[(0, 125), (0, 125), (1, 250), (2, 500), (3, 1000), (4, 2000), (5, 4000)])),
        ("right count, 8 kHz's numbers under 8 kHz but 4 kHz's row relabelled 8 kHz too", with(&[(0, 125), (1, 250), (2, 500), (3, 1000), (4, 2000), (6, 8000), (5, 8000)])),
        ("all 7, 8 kHz twice (8 rows)", with(&[(0, 125), (1, 250), (2, 500), (3, 1000), (4, 2000), (5, 4000), (6, 8000), (6, 8000)])),
    ];
    for (name, b) in &cases {
        let mut r = reads0.clone();
        r.tcr.get_mut(tid).unwrap().as_mut().unwrap().bands = b.clone();
        line(&format!("{name} [after into_reads]"), &ctx.judge(&r), tid);
        let mut read = base_t.clone();
        tcr_mut(&mut read).bands = b.clone();
        println!("    check_planned: {}", planned_verdict(ctx.planned(&tcr_key(tid)), &Ok(read.clone())));
        line(&format!("{name} [before into_reads]"), &ctx.judge_with(vec![(tcr_key(tid), Ok(read))]), tid);
    }
    let off = "6x10x3-a0.2-tcr-air-off";
    let base_off = ctx.base_read(&tcr_key(off));
    let ob = match &base_off {
        Read::Tcr(t) => t.bands.clone(),
        _ => unreachable!(),
    };
    let mut b = ob.clone();
    b[5].freq_hz = 8000;
    let mut r = reads0.clone();
    r.tcr.get_mut(off).unwrap().as_mut().unwrap().bands = b.clone();
    line("air-off run, right count (6), 4 kHz relabelled 8 kHz [after into_reads]", &ctx.judge(&r), off);
    let mut read = base_off.clone();
    tcr_mut(&mut read).bands = b;
    println!("    check_planned: {}", planned_verdict(ctx.planned(&tcr_key(off)), &Ok(read)));

    println!("C2. SPPS settings changed one at a time on a real read (5x4x3-a0.4-energetic-air-off seed 3):");
    let k3 = cell("5x4x3-a0.4-energetic-air-off", 3);
    let p3 = ctx.planned(&k3);
    let r3 = ctx.base_read(&k3);
    println!("  untouched: {}", planned_verdict(p3, &Ok(r3.clone())));
    let muts: Vec<(&str, Box<dyn Fn(&mut bread::SppsRead)>)> = vec![
        ("trans_epsilon 7 (step right)", Box::new(|s: &mut bread::SppsRead| s.trans_epsilon = 7.0)),
        ("trans_epsilon 9.000000000001 (step right)", Box::new(|s: &mut bread::SppsRead| s.trans_epsilon = 9.000000000001)),
        ("particles 150,000 (step right)", Box::new(|s: &mut bread::SppsRead| s.particles_per_source = 150_000)),
        ("particles +1", Box::new(|s: &mut bread::SppsRead| s.particles_per_source += 1)),
        ("computation_method 0", Box::new(|s: &mut bread::SppsRead| s.computation_method = 0)),
        ("random_seed none", Box::new(|s: &mut bread::SppsRead| s.random_seed = None)),
        ("random_seed 0", Box::new(|s: &mut bread::SppsRead| s.random_seed = Some(0))),
        ("bands right count, 4 kHz relabelled 8 kHz", Box::new(|s: &mut bread::SppsRead| s.bands_hz[5] = 8000)),
        ("bands right count, 2 kHz twice", Box::new(|s: &mut bread::SppsRead| s.bands_hz[5] = 2000)),
        ("bands in another order", Box::new(|s: &mut bread::SppsRead| s.bands_hz.swap(0, 1))),
        ("project sha256 of seed 4's project", Box::new(|s: &mut bread::SppsRead| s.info.project_sha256 = Some("x".into()))),
        ("time_step_s NaN", Box::new(|s: &mut bread::SppsRead| s.time_step_s = f64::NAN)),
    ];
    let p4sha = run::planned_project_sha256(ctx.planned(&cell("5x4x3-a0.4-energetic-air-off", 4)));
    for (name, m) in &muts {
        let mut r = r3.clone();
        m(spps_mut(&mut r));
        if name.starts_with("project sha256") {
            spps_mut(&mut r).info.project_sha256 = Some(p4sha.clone());
        }
        println!("  {name}: {}", planned_verdict(p3, &Ok(r)));
    }

    println!("C3. real runs of other settings filed under a cell:");
    let n5k = RunKey::N5 { seed: 10 };
    let n5cell = cell("5x4x3-a0.4-random-air-off", 10);
    let n5read = ctx.base_read(&n5k);
    println!(
        "  N5's run (31,000 particles, seed 10) as {}: {}",
        n5cell.label(),
        planned_verdict(ctx.planned(&n5cell), &Ok(n5read.clone()))
    );
    line("N5's run in the cell's seed-10 slot", &ctx.judge_with(vec![(n5cell.clone(), Ok(n5read))]), "5x4x3-a0.4-random-air-off");
    let rnd1 = ctx.base_read(&cell("5x4x3-a0.4-random-air-off", 1));
    let en1 = cell("5x4x3-a0.4-energetic-air-off", 1);
    println!(
        "  the random cell's seed-1 run as {}: {}",
        en1.label(),
        planned_verdict(ctx.planned(&en1), &Ok(rnd1))
    );
    let a01 = ctx.base_read(&cell("5x4x3-a0.1-energetic-air-off", 3));
    let a005 = cell("5x4x3-a0.05-energetic-air-off", 3);
    println!(
        "  5x4x3-a0.1-energetic-air-off seed 3 (same particles, method, step, epsilon, seed, bands; alpha and duration differ) as {}: {}",
        a005.label(),
        planned_verdict(ctx.planned(&a005), &Ok(a01))
    );
    let cell_s1 = ctx.base_read(&cell("5x4x3-a0.4-energetic-air-off", 1));
    println!(
        "  the energetic cell's seed-1 SPPS run as TCR 5x4x3-a0.4-tcr-air-off (the same project): {}",
        planned_verdict(ctx.planned(&tcr_key("5x4x3-a0.4-tcr-air-off")), &Ok(cell_s1))
    );
    let m6 = ctx.base_read(&cell("5x4x3-a0.2-energetic-air-off", 1));
    println!(
        "  the matrix's 5x4x3-a0.2-energetic-air-off seed 1 (scattering 1, 1.5 M) as N6: {}",
        planned_verdict(ctx.planned(&RunKey::N6 { seed: 1 }), &Ok(m6))
    );

    println!("C4. a seed filed under the wrong seed folder (real folders):");
    let id = "5x4x3-a0.4-energetic-air-off";
    let r4in3 = run::read_in(&root.join(cell(id, 4).dir()), ctx.planned(&cell(id, 3)));
    println!("  s4's folder read as seed 3: {}", planned_verdict(ctx.planned(&cell(id, 3)), &r4in3));
    let r3in4 = run::read_in(&root.join(cell(id, 3).dir()), ctx.planned(&cell(id, 4)));
    line(
        "s3 and s4 swapped",
        &ctx.judge_with(vec![(cell(id, 3), r4in3), (cell(id, 4), r3in4)]),
        id,
    );

    println!("C5. tampered copies under {SCRATCH}\\tamper (the M8a folders only read):");
    let tamper = Path::new(SCRATCH).join("tamper");
    // (i) seed 3's run folder, its outputs replaced with seed 4's: run.json (project sha256) and
    // config.xml are seed 3's.
    let dst = tamper.join("swap-seed").join(cell(id, 3).dir());
    let run_copy = copy_slot(&root.join(cell(id, 3).dir()), &dst);
    let s4_run = bread::find_run_folder(&root.join(cell(id, 4).dir())).unwrap();
    let n = swap_outputs(&run_copy, &s4_run);
    let r = run::read_in(&dst, ctx.planned(&cell(id, 3)));
    println!(
        "  (i) {id} s3 with s4's {n} output files: {}",
        planned_verdict(ctx.planned(&cell(id, 3)), &r)
    );
    if let Ok(read) = &r {
        println!(
            "      its T30 equal to seed 4's: {}; to seed 3's: {}",
            t30_equal(read, &ctx.base_read(&cell(id, 4))),
            t30_equal(read, &ctx.base_read(&cell(id, 3)))
        );
    }
    let mut r = r;
    clear_curves(&mut r);
    line("(i) in place of seed 3", &ctx.judge_with(vec![(cell(id, 3), r)]), id);
    // (i-b) every seed's outputs replaced with seed 1's.
    let mut subst = Vec::new();
    let s1_run = bread::find_run_folder(&root.join(cell(id, 1).dir())).unwrap();
    for s in 2..=10u32 {
        let dst = tamper.join("swap-all-to-s1").join(cell(id, s).dir());
        let run_copy = copy_slot(&root.join(cell(id, s).dir()), &dst);
        swap_outputs(&run_copy, &s1_run);
        let mut r = run::read_in(&dst, ctx.planned(&cell(id, s)));
        if s == 2 {
            println!("  (i-b) {id} s2 with s1's outputs: {}", planned_verdict(ctx.planned(&cell(id, s)), &r));
        }
        clear_curves(&mut r);
        subst.push((cell(id, s), r));
    }
    line("(i-b) seeds 2 to 10 all carrying seed 1's outputs", &ctx.judge_with(subst), id);
    // (ii) run.json's project sha256 edited: a whole run of another cell with the same SPPS
    // settings, and one of another seed.
    let a = "5x4x3-a0.05-energetic-air-off";
    let b = "5x4x3-a0.1-energetic-air-off";
    let mut subst = Vec::new();
    for s in 1..=10u32 {
        let dst = tamper.join("edited-sha-other-cell").join(cell(a, s).dir());
        let run_copy = copy_slot(&root.join(cell(b, s).dir()), &dst);
        edit_project_sha(&run_copy, &run::planned_project_sha256(ctx.planned(&cell(a, s))));
        let mut r = run::read_in(&dst, ctx.planned(&cell(a, s)));
        if s == 3 {
            println!(
                "  (ii) {b} s3, run.json's sha256 set to {a} s3's plan: {}",
                planned_verdict(ctx.planned(&cell(a, s)), &r)
            );
        }
        clear_curves(&mut r);
        subst.push((cell(a, s), r));
    }
    let j = ctx.judge_with(subst[2..3].to_vec());
    line("(ii) that one seed in place", &j, a);
    let j = ctx.judge_with(subst);
    line("(ii) all ten seeds of the other cell, sha256 edited", &j, a);
    if let Some(c) = j.cells.iter().find(|c| c.id == a) {
        println!(
            "      A {:?} B {:?} C {:?}",
            c.a.as_ref().map(|x| x.verdict),
            c.b.as_ref().map(|x| x.verdict),
            c.c.as_ref().map(|x| x.verdict)
        );
    }
    let dst = tamper.join("edited-sha-other-seed").join(cell(id, 3).dir());
    let run_copy = copy_slot(&root.join(cell(id, 4).dir()), &dst);
    edit_project_sha(&run_copy, &run::planned_project_sha256(ctx.planned(&cell(id, 3))));
    let r = run::read_in(&dst, ctx.planned(&cell(id, 3)));
    println!("  (ii-b) {id} s4's run, run.json's sha256 set to s3's plan: {}", planned_verdict(ctx.planned(&cell(id, 3)), &r));
    // (iii) a 1 ms run folder carrying the 10 ms run's outputs.
    let k1 = cell(id, 1);
    let dst = tamper.join("outputs-10ms").join(k1.dir());
    let run_copy = copy_slot(&root.join(k1.dir()), &dst);
    let r10 = bread::find_run_folder(&root10.join(k1.dir())).unwrap();
    let n = swap_outputs(&run_copy, &r10);
    let r = run::read_in(&dst, ctx.planned(&k1));
    println!("  (iii) {id} s1 carrying the 10 ms run's {n} output files: {}", planned_verdict(ctx.planned(&k1), &r));
    // (iv) a TCR run folder: the M8a TCR run's run.json, the 10 ms bed's TCR outputs.
    let kt = tcr_key("5x4x3-a0.4-tcr-air-off");
    let dst = tamper.join("tcr-outputs-10ms").join(kt.dir());
    let run_copy = copy_slot(&root.join(kt.dir()), &dst);
    let r10 = bread::find_run_folder(&root10.join(kt.dir())).unwrap();
    let n = swap_outputs(&run_copy, &r10);
    let r = run::read_in(&dst, ctx.planned(&kt));
    println!("  (iv) TCR 5x4x3-a0.4-tcr-air-off carrying the 10 ms TCR run's {n} output files: {}", planned_verdict(ctx.planned(&kt), &r));
    // (v) a TCR run of another alpha, run.json's sha256 edited.
    let kt2 = tcr_key("5x4x3-a0.2-tcr-air-off");
    let dst = tamper.join("tcr-edited-sha").join(kt.dir());
    let run_copy = copy_slot(&root.join(kt2.dir()), &dst);
    edit_project_sha(&run_copy, &run::planned_project_sha256(ctx.planned(&kt)));
    let mut r = run::read_in(&dst, ctx.planned(&kt));
    println!("  (v) TCR 5x4x3-a0.2-tcr-air-off's run, run.json's sha256 set to 5x4x3-a0.4-tcr-air-off's plan: {}", planned_verdict(ctx.planned(&kt), &r));
    clear_curves(&mut r);
    line("(v) in place", &ctx.judge_with(vec![(kt.clone(), r)]), "5x4x3-a0.4-tcr-air-off");

    println!("\ndone in {:.0} s", t_all.elapsed().as_secs_f64());
    let _ = Verdict::Pass;
}
