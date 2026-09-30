//! M8b fix check, read-only on the M8a bed: every run of the bed (cells, TCR, N5, N6 and the
//! atmospheric validation) read with this build and matched with the run it is filed under
//! (`run::check_planned`, through `run::into_reads`); the bed re-judged with the report's own
//! transports (nothing re-traced); the judge's two TCR band cases on the real runs; and the
//! bed's own 1 ms runs filed under a 10 ms plan, which must each be refused.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Instant;

use simpa_core::bed::check::{Reads, Verdict};
use simpa_core::bed::file;
use simpa_core::bed::report::{self, Report};
use simpa_core::bed::run::{self, RunKey};
use simpa_core::bed::transport::{self, Transports};

const ROOT: &str = r"C:\tmp\nm-m8a-bed\20260929T093134Z";
const BED: &str = r"B:\repos\I-Simpa_Night_Mode\.claude\worktrees\m8b\beds\m8a.json";
const UPSTREAM: &str = r"B:\repos\I-Simpa-upstream";

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

fn main() {
    let jobs: usize = std::env::var("JOBS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(8);
    let l = file::load(Path::new(BED)).unwrap();
    println!("bed file: exploratory {:?}", l.exploratory);
    let root = PathBuf::from(ROOT);
    let rep: Report =
        serde_json::from_str(&std::fs::read_to_string(root.join("report.json")).unwrap()).unwrap();
    println!(
        "report.json: pass {} exploratory {} E1 runs checked {}",
        rep.pass, rep.exploratory, rep.preconditions.e1.runs_checked
    );
    let (runs, not_run) = run::plan(&l.bed, Some(Path::new(UPSTREAM))).unwrap();
    println!("planned {} runs; atmospheric not run: {not_run:?}", runs.len());

    // 1. Every run read, and matched with its plan.
    let t0 = Instant::now();
    let results = run::read_existing(&runs, &root, jobs);
    println!(
        "read {} runs in {:.0} s ({jobs} at once)",
        runs.len(),
        t0.elapsed().as_secs_f64()
    );
    let mut by: BTreeMap<&str, [usize; 3]> = BTreeMap::new();
    let mut refused = Vec::new();
    for (p, r) in runs.iter().zip(&results) {
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
    }
    println!("by kind [matched, refused by check_planned, not read]: {by:?}");
    for x in refused.iter().take(20) {
        println!("  {x}");
    }
    let mut reads = Reads::default();
    run::into_reads(&mut reads, &runs, results, not_run);
    for m in reads.spps.values_mut() {
        for r in m.values_mut().flatten() {
            r.curves.clear();
        }
    }
    for r in reads.atmospheric.spps.values_mut().flatten() {
        r.curves.clear();
    }
    for r in [&mut reads.n5, &mut reads.n6].into_iter().flatten().flatten() {
        r.curves.clear();
    }

    // 2. The bed re-judged with the report's transports.
    let mut ts = Transports::default();
    for x in &rep.transports {
        ts.by_key.insert(
            transport::key(&x.room, x.alpha, x.air_m_per_metre),
            Ok(x.clone()),
        );
    }
    let judge = |reads: &Reads| {
        report::evaluate(&report::Inputs {
            bed: &l.bed,
            reads,
            transports: &ts,
            exploratory: &l.exploratory,
            solvers: &rep.preconditions.e1.solvers,
        })
    };
    let j = judge(&reads);
    println!(
        "re-judged: pass {} | failures {} | E1 runs checked {} (report {}) | E2 {} | first: {}",
        j.pass,
        j.failures.len(),
        j.preconditions.e1.runs_checked,
        rep.preconditions.e1.runs_checked,
        j.preconditions.e2.holds,
        j.failures.first().map(String::as_str).unwrap_or("-")
    );
    let cells_same = j
        .cells
        .iter()
        .zip(&rep.cells)
        .filter(|(a, b)| a.id == b.id && a.verdict == b.verdict)
        .count();
    let tcr_same = j
        .tcr
        .iter()
        .zip(&rep.tcr)
        .filter(|(a, b)| a.id == b.id && a.verdict == b.verdict)
        .count();
    let band_problems: usize = j
        .tcr
        .iter()
        .filter_map(|t| t.d.as_ref())
        .map(|d| d.band_problems.len())
        .sum();
    println!(
        "cell verdicts as in report.json: {cells_same} of {}; TCR verdicts: {tcr_same} of {}; D band problems: {band_problems}",
        rep.cells.len(),
        rep.tcr.len()
    );
    println!(
        "say-NO: N5 as_required {} (report {}), error {:?}; N6 as_required {} (report {}), error {:?}",
        j.say_no.n5.as_required,
        rep.say_no.n5.as_required,
        j.say_no.n5.error,
        j.say_no.n6.as_required,
        rep.say_no.n6.as_required,
        j.say_no.n6.error
    );
    let atmo = &j.atmospheric_validation;
    println!(
        "atmospheric: not_run {:?}; runs {} with error {}; TCR {} error {:?}",
        atmo.not_run,
        atmo.runs.len(),
        atmo.runs.iter().filter(|r| r.error.is_some()).count(),
        atmo.tcr.is_some(),
        atmo.tcr_error
    );

    // 3. The judge's two TCR band cases, on the real run.
    let tid = "6x10x3-a0.2-tcr-air-on";
    for (name, keep) in [("8 kHz removed", 6usize), ("only 125 Hz", 1)] {
        let mut r = reads.clone();
        r.tcr
            .get_mut(tid)
            .unwrap()
            .as_mut()
            .unwrap()
            .bands
            .truncate(keep);
        let x = judge(&r);
        let t = x.tcr.iter().find(|t| t.id == tid).unwrap();
        println!(
            "  {tid}, {name}: pass {} | TCR {:?} | failures {} | {}",
            x.pass,
            t.verdict,
            x.failures.len(),
            x.failures
                .iter()
                .find(|f| f.starts_with(tid))
                .map(String::as_str)
                .unwrap_or("-")
        );
    }
    let untouched = judge(&reads);
    println!(
        "  untouched: pass {} | TCR {:?}",
        untouched.pass,
        untouched.tcr.iter().find(|t| t.id == tid).unwrap().verdict
    );

    // 4. The bed's own 1 ms runs filed under a plan at 10 ms: each must be refused.
    let mut ten = l.bed.clone();
    ten.time_step_s = 0.01;
    let (plan10, _) = run::plan(&ten, None).unwrap();
    let pick: Vec<run::Planned> = plan10
        .into_iter()
        .filter(|p| {
            matches!(&p.key, RunKey::Cell { id, seed } if id == "5x4x3-a0.4-energetic-air-off" && *seed <= 3)
                || matches!(&p.key, RunKey::Tcr { id } if id == "5x4x3-a0.4-tcr-air-off")
        })
        .collect();
    let res = run::read_existing(&pick, &root, jobs);
    let mut r10 = Reads::default();
    run::into_reads(&mut r10, &pick, res, Some("not read".into()));
    for (id, m) in &r10.spps {
        for (s, r) in m {
            println!(
                "  1 ms run under a 10 ms plan, {id} seed {s}: {}",
                match r {
                    Ok(_) => "ACCEPTED".to_string(),
                    Err(e) => e.chars().take(420).collect(),
                }
            );
        }
    }
    for (id, r) in &r10.tcr {
        println!(
            "  1 ms bed's TCR run under a 10 ms plan, {id}: {}",
            match r {
                Ok(_) => "ACCEPTED".to_string(),
                Err(e) => e.chars().take(300).collect(),
            }
        );
    }
    let v = j.cells.iter().filter(|c| c.gated).fold(BTreeMap::new(), |mut m, c| {
        *m.entry(format!("{:?}", c.verdict)).or_insert(0usize) += 1;
        m
    });
    println!("gated cell verdicts re-judged: {v:?}");
    let _ = Verdict::Pass;
}
