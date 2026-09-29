//! Judge (gate lens) harness for M8a: coverage read from the bed's own report.json, the verdict
//! re-derived from the bed's run folders with this build (as `simpa bed --from` would, without
//! re-tracing the transports: they are taken from the report, as bed_m8a.rs's N2/N3/N8 take them),
//! and single perturbations of what the code reads, each just outside and just inside a limit.
//! Read-only on the bed. No solver runs, no transport traced.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use simpa_core::bed::check::{Reads, Verdict};
use simpa_core::bed::file::{self, BedFile};
use simpa_core::bed::read::T30;
use simpa_core::bed::report::{self, Report};
use simpa_core::bed::run::{self, Planned, RunKey};
use simpa_core::bed::transport::{self, Transports};

const ROOT: &str = r"C:\tmp\nm-m8a-bed\20260929T093134Z";
const BED: &str = r"B:\repos\I-Simpa_Night_Mode\.claude\worktrees\m8a\beds\m8a.json";

fn traced(rep: &Report) -> Transports {
    let mut t = Transports::default();
    for x in &rep.transports {
        t.by_key.insert(
            transport::key(&x.room, x.alpha, x.air_m_per_metre),
            Ok(x.clone()),
        );
    }
    t
}

fn judge(l: &file::Loaded, reads: &Reads, ts: &Transports, rep: &Report) -> Report {
    report::evaluate(&report::Inputs {
        bed: &l.bed,
        reads,
        transports: ts,
        exploratory: &l.exploratory,
        solvers: &rep.preconditions.e1.solvers,
    })
}

fn cell<'a>(r: &'a Report, id: &str) -> &'a simpa_core::bed::check::CellReport {
    r.cells.iter().find(|c| c.id == id).unwrap()
}

fn line(name: &str, r: &Report) {
    println!(
        "  {name}: pass {} | failures {} | first: {}",
        r.pass,
        r.failures.len(),
        r.failures.first().map(String::as_str).unwrap_or("-")
    );
}

/// Bisection on a monotone scalar `f(x)` for the `x` where it reaches `target`, in `[lo, hi]`.
fn solve(mut lo: f64, mut hi: f64, target: f64, f: impl Fn(f64) -> f64) -> f64 {
    let up = f(hi) > f(lo);
    for _ in 0..80 {
        let mid = 0.5 * (lo + hi);
        let v = f(mid);
        if (v < target) == up {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    0.5 * (lo + hi)
}

fn diff(path: &str, a: &serde_json::Value, b: &serde_json::Value, out: &mut Vec<String>) {
    use serde_json::Value;
    match (a, b) {
        (Value::Object(x), Value::Object(y)) => {
            for (k, v) in x {
                match y.get(k) {
                    Some(w) => diff(&format!("{path}.{k}"), v, w, out),
                    None => out.push(format!("{path}.{k} : only in the report")),
                }
            }
            for k in y.keys().filter(|k| !x.contains_key(*k)) {
                out.push(format!("{path}.{k} : only in the re-judgement"));
            }
        }
        (Value::Array(x), Value::Array(y)) => {
            if x.len() != y.len() {
                out.push(format!("{path} : lengths {} and {}", x.len(), y.len()));
            }
            for (i, (v, w)) in x.iter().zip(y).enumerate() {
                diff(&format!("{path}.[{i}]"), v, w, out);
            }
        }
        (Value::Number(x), Value::Number(y)) => {
            let (x, y) = (x.as_f64().unwrap(), y.as_f64().unwrap());
            let rel = if x == y { 0.0 } else { ((x - y) / x.abs().max(y.abs())).abs() };
            if rel > 1e-12 {
                out.push(format!("{path} : {x} against {y} (relative {rel:e}) BIG"));
            } else if rel > 0.0 {
                out.push(format!("{path} : {x} against {y} (relative {rel:e})"));
            }
        }
        _ if a != b => out.push(format!("{path} : {a} against {b}")),
        _ => {}
    }
}

fn scale_cell(reads: &mut Reads, id: &str, seeds: &[u32], k: f64) {
    for (s, r) in reads.spps.get_mut(id).unwrap().iter_mut() {
        if seeds.contains(s) {
            let r = r.as_mut().unwrap();
            for rec in &mut r.t30 {
                for t in rec {
                    t.t = t.t.map(|v| v * k);
                }
            }
        }
    }
}

fn main() {
    let l = file::load(Path::new(BED)).unwrap();
    println!("bed file: exploratory {:?}", l.exploratory);
    let root = PathBuf::from(ROOT);
    let text = std::fs::read_to_string(root.join("report.json")).unwrap();
    let committed = std::fs::read_to_string(
        r"B:\repos\I-Simpa_Night_Mode\.claude\worktrees\m8a\beds\m8a-20260929T093134Z\report.json",
    )
    .unwrap();
    println!(
        "report.json: the committed copy is byte-identical: {}",
        text == committed
    );
    let rep: Report = serde_json::from_str(&text).unwrap();
    if std::env::var_os("JUDGE_ROUNDTRIP").is_some() {
        let again = serde_json::to_string_pretty(&rep).unwrap() + "
";
        println!("report.json parsed and written again is byte-identical: {}", again == text);
        let (a, b): (Vec<&str>, Vec<&str>) = (text.lines().collect(), again.lines().collect());
        let n = a.iter().zip(&b).filter(|(x, y)| x != y).count();
        println!("lines differing: {n} of {}", a.len());
        for (x, y) in a.iter().zip(&b).filter(|(x, y)| x != y).take(5) {
            println!("  {} | {}", x.trim(), y.trim());
        }
        return;
    }

    // ---------------------------------------------------------------- 1. coverage, from the report
    println!("\n== coverage in report.json");
    let p = &rep.preconditions;
    println!(
        "pass {} exploratory {} failures {} needs_extension {:?}",
        rep.pass,
        rep.exploratory,
        rep.failures.len(),
        rep.needs_extension
    );
    println!(
        "E1 {} (runs checked {}, solvers {}), E2 {} ({}), E3 {} ({}), E4 {} ({} cells, max diff {:e} s), E5 {} ({} cells), E6 {} ({}), E7 {}",
        p.e1.holds,
        p.e1.runs_checked,
        p.e1.solvers.len(),
        p.e2.holds,
        p.e2.problems.len(),
        p.e3.holds,
        p.e3.problems.len(),
        p.e4.holds,
        p.e4.cells.len(),
        p.e4.cells
            .iter()
            .filter_map(|c| c.max_difference_s)
            .fold(0.0, f64::max),
        p.e5.holds,
        p.e5.cells.len(),
        p.e6.holds,
        p.e6.problems.len(),
        p.e7.holds
    );
    for c in &p.e5.cells {
        println!(
            "  E5 {} a{}: K {:?} vs HIGH receivers {:+.4} %, room energy {:+.4} %",
            c.room,
            c.alpha,
            c.kuttruff_s,
            100.0 * c.against_receivers.unwrap_or(f64::NAN),
            100.0 * c.against_room_energy.unwrap_or(f64::NAN)
        );
    }
    let gated: Vec<_> = rep.cells.iter().filter(|c| c.gated).collect();
    let mut values = 0usize;
    let mut sources: BTreeMap<String, usize> = BTreeMap::new();
    let mut problems = Vec::new();
    let (mut worst_a, mut worst_b, mut worst_c, mut worst_d) =
        ((0.0f64, String::new()), (0.0f64, String::new()), (0.0f64, String::new()), (0.0f64, String::new()));
    let mut bands_judged = 0usize;
    let mut air_diff = 0.0f64;
    for c in &gated {
        let nb = if c.air { 7 } else { 6 };
        let seeds: Vec<u32> = c.seeds.iter().map(|s| s.seed).collect();
        if seeds != (1..=10).collect::<Vec<_>>() {
            problems.push(format!("{}: seeds {seeds:?}", c.id));
        }
        if c.bands_hz.len() != nb {
            problems.push(format!("{}: {} bands", c.id, c.bands_hz.len()));
        }
        for s in &c.seeds {
            if s.run.is_none() || s.error.is_some() {
                problems.push(format!("{} s{}: no run {:?}", c.id, s.seed, s.error));
            }
            if s.run.as_ref().is_some_and(|r| r.status != "OK") {
                problems.push(format!("{} s{}: status", c.id, s.seed));
            }
            if s.t30.len() != 3 {
                problems.push(format!("{} s{}: {} receivers", c.id, s.seed, s.t30.len()));
            }
            for rec in &s.t30 {
                if rec.len() != nb {
                    problems.push(format!("{} s{}: receiver with {} bands", c.id, s.seed, rec.len()));
                }
                for t in rec {
                    values += 1;
                    *sources.entry(t.source.clone()).or_default() += 1;
                    if !t.judged() {
                        problems.push(format!("{} s{}: {}", c.id, s.seed, t.source));
                    }
                }
            }
        }
        if !(c.e2.holds && c.e3.holds && c.e6.holds) {
            problems.push(format!("{}: E2/E3/E6", c.id));
        }
        match &c.a {
            Some(a) => {
                if a.bands.len() != nb || a.reference != "kuttruff_s" {
                    problems.push(format!("{}: A has {} bands, ref {}", c.id, a.bands.len(), a.reference));
                }
                for b in &a.bands {
                    bands_judged += 1;
                    if b.verdict != Verdict::Pass {
                        problems.push(format!("{} {} Hz: A {:?}", c.id, b.freq_hz, b.verdict));
                    }
                    if let Some(i) = b.interval {
                        if i.n != 10 {
                            problems.push(format!("{} {} Hz: A over {} seeds", c.id, b.freq_hz, i.n));
                        }
                        if i.reach() > worst_a.0 {
                            worst_a = (i.reach(), format!("{} {} Hz, mean {:+.3} %, [{:+.3}, {:+.3}] %", c.id, b.freq_hz, 100.0 * i.mean, 100.0 * i.lo, 100.0 * i.hi));
                        }
                    } else {
                        problems.push(format!("{} {} Hz: A no interval", c.id, b.freq_hz));
                    }
                }
            }
            None => problems.push(format!("{}: no A", c.id)),
        }
        match &c.b {
            Some(b) => {
                if b.seeds.len() != 10 || b.verdict != Verdict::Pass {
                    problems.push(format!("{}: B {:?} on {} seeds", c.id, b.verdict, b.seeds.len()));
                }
                if b.spread > worst_b.0 {
                    worst_b = (b.spread, c.id.clone());
                }
            }
            None => problems.push(format!("{}: no B", c.id)),
        }
        match &c.c {
            Some(x) => {
                if x.seeds.len() != 10 || x.verdict != Verdict::Pass {
                    problems.push(format!("{}: C {:?} on {} seeds", c.id, x.verdict, x.seeds.len()));
                }
                if x.interval.reach() > worst_c.0 {
                    worst_c = (x.interval.reach(), format!("{}: d {:+.3} %, [{:+.3}, {:+.3}] %, se_tr {:.4} %", c.id, 100.0 * x.interval.mean, 100.0 * x.interval.lo, 100.0 * x.interval.hi, 100.0 * x.se_transport));
                }
            }
            None => problems.push(format!("{}: no C", c.id)),
        }
        if c.extension.is_some() {
            problems.push(format!("{}: extended", c.id));
        }
        if c.verdict != Verdict::Pass {
            problems.push(format!("{}: verdict {:?}", c.id, c.verdict));
        }
        let rf = c.reference.as_ref().unwrap();
        if rf.bands.len() != nb {
            problems.push(format!("{}: reference has {} bands", c.id, rf.bands.len()));
        }
        for b in &rf.bands {
            if b.kuttruff_s.is_none() || b.transport_t.is_none() || !b.lambert_walls {
                problems.push(format!("{} {} Hz: reference/transport missing", c.id, b.freq_hz));
            }
            if c.air {
                match (b.air_m_per_metre, b.bed_air_m_per_metre) {
                    (Some(a), Some(x)) => air_diff = air_diff.max((a / x - 1.0).abs()),
                    _ => problems.push(format!("{} {} Hz: air m missing", c.id, b.freq_hz)),
                }
            } else if b.air_m_per_metre.is_some() || b.bed_air_m_per_metre.is_some() {
                problems.push(format!("{} {} Hz: air m with the air off", c.id, b.freq_hz));
            }
        }
    }
    let gated_tcr: Vec<_> = rep.tcr.iter().filter(|t| t.gated).collect();
    let mut d_bands = 0usize;
    for t in &gated_tcr {
        let nb = if t.id.ends_with("air-on") { 7 } else { 6 };
        match &t.d {
            Some(d) => {
                if d.bands.len() != nb || d.verdict != Verdict::Pass {
                    problems.push(format!("{}: D {:?} on {} bands", t.id, d.verdict, d.bands.len()));
                }
                for b in &d.bands {
                    d_bands += 1;
                    let x = b.deviation.map_or(f64::INFINITY, f64::abs);
                    if x > worst_d.0 {
                        worst_d = (x, format!("{} {} Hz: {:+.5} %", t.id, b.freq_hz, 100.0 * b.deviation.unwrap_or(f64::NAN)));
                    }
                }
            }
            None => problems.push(format!("{}: no D", t.id)),
        }
        if t.verdict != Verdict::Pass || t.run.is_none() || t.error.is_some() {
            problems.push(format!("{}: TCR verdict {:?}", t.id, t.verdict));
        }
    }
    println!(
        "gated cells {} of {} (verdicts: {:?}); seed-runs {}; receiver-band-seed values {}; by source {:?}",
        gated.len(),
        rep.cells.len(),
        gated.iter().fold(BTreeMap::new(), |mut m, c| {
            *m.entry(format!("{:?}", c.verdict)).or_insert(0usize) += 1;
            m
        }),
        gated.iter().map(|c| c.seeds.len()).sum::<usize>(),
        values,
        sources
    );
    println!("A bands judged {bands_judged}; gated TCR {} of {}, D bands {d_bands}", gated_tcr.len(), rep.tcr.len());
    println!("worst A reach {:.3} % ({}), limit 5 %", 100.0 * worst_a.0, worst_a.1);
    println!("worst B spread {:.3} % ({}), limit 2 %", 100.0 * worst_b.0, worst_b.1);
    println!("worst C |d|+t*SE {:.3} % ({}), limit 0.5 %", 100.0 * worst_c.0, worst_c.1);
    println!("worst D |dev| {:.5} % ({}), limit 0.5 %", 100.0 * worst_d.0, worst_d.1);
    println!("air on: run m against the bed's m, largest relative difference {air_diff:e}");
    println!("coverage problems: {}", problems.len());
    for x in problems.iter().take(30) {
        println!("  {x}");
    }
    println!(
        "N5 in the report: cell_verdict {:?}, failed_by {:?}, as_required {}, error {:?}",
        rep.say_no.n5.cell_verdict, rep.say_no.n5.failed_by, rep.say_no.n5.as_required, rep.say_no.n5.error
    );
    for f in rep.say_no.n5.failures.iter().take(8) {
        println!("  N5: {}", f.chars().take(400).collect::<String>());
    }
    println!(
        "N6 in the report: codes {:?}, as_required {}",
        rep.say_no.n6.codes, rep.say_no.n6.as_required
    );
    println!("reported notes: {}", rep.reported_notes.len());
    for f in rep.reported_notes.iter().take(12) {
        println!("  {}", f.chars().take(220).collect::<String>());
    }

    // ------------------------------------------------------- 2. the verdict again, from the runs
    println!("\n== re-judged from the run folders with this build");
    let (runs, _) = run::plan(&l.bed, None).unwrap();
    let runs: Vec<Planned> = runs
        .into_iter()
        .filter(|p| {
            matches!(
                p.key,
                RunKey::Cell { .. } | RunKey::Tcr { .. } | RunKey::N5 { .. } | RunKey::N6 { .. }
            )
        })
        .collect();
    let t0 = std::time::Instant::now();
    let results = run::read_existing(&runs, &root, 6);
    let mut reads = Reads::default();
    run::into_reads(&mut reads, &runs, results, Some("not read by the judge".into()));
    println!("read {} runs in {:.0} s", runs.len(), t0.elapsed().as_secs_f64());
    // The decay curves are for the output files only; evaluate never reads them.
    for m in reads.spps.values_mut() {
        for r in m.values_mut().flatten() {
            r.curves.clear();
        }
    }
    for r in [&mut reads.n5, &mut reads.n6].into_iter().flatten().flatten() {
        r.curves.clear();
    }
    let ts = traced(&rep);
    let base = judge(&l, &reads, &ts, &rep);
    line("re-judged", &base);
    let same_cells = serde_json::to_value(&base.cells).unwrap() == serde_json::to_value(&rep.cells).unwrap();
    let same_tcr = serde_json::to_value(&base.tcr).unwrap() == serde_json::to_value(&rep.tcr).unwrap();
    let same_n5 = serde_json::to_value(&base.say_no).unwrap() == serde_json::to_value(&rep.say_no).unwrap();
    println!(
        "  cells identical to report.json: {same_cells}; TCR identical: {same_tcr}; say_no identical: {same_n5}; E1 runs checked {} (report {}, the 11 atmospheric runs not read here)",
        base.preconditions.e1.runs_checked, rep.preconditions.e1.runs_checked
    );

    {
        let mut diffs = Vec::new();
        diff(
            "cells",
            &serde_json::to_value(&rep.cells).unwrap(),
            &serde_json::to_value(&base.cells).unwrap(),
            &mut diffs,
        );
        diff(
            "tcr",
            &serde_json::to_value(&rep.tcr).unwrap(),
            &serde_json::to_value(&base.tcr).unwrap(),
            &mut diffs,
        );
        let mut kinds: BTreeMap<String, usize> = BTreeMap::new();
        for d in &diffs {
            let k: String = d
                .split(" : ")
                .next()
                .unwrap()
                .split('.')
                .map(|p| if p.starts_with('[') { "[]" } else { p })
                .collect::<Vec<_>>()
                .join(".");
            *kinds.entry(k).or_default() += 1;
        }
        println!("  differing leaves {}; by path: {kinds:#?}", diffs.len());
        let ulps = diffs.iter().filter(|d| d.contains("(relative")).count(); let big: Vec<_> = diffs.iter().filter(|d| d.ends_with("BIG") || !d.contains("(relative")).collect(); println!("  numeric diffs within 1e-12 relative: {ulps}; others: {}", big.len()); let maxrel = diffs.iter().filter_map(|d| d.split("(relative ").nth(1)).filter_map(|s| s.trim_end_matches(|c| c == ')' || c == ' ' || c == 'B' || c == 'I' || c == 'G').parse::<f64>().ok()).fold(0.0, f64::max); println!("  largest relative difference {maxrel:e}");
        for d in big.iter().take(12) {
            println!("    {d}");
        }
    }
    if std::env::var_os("JUDGE_SKIP_PERTURB").is_some() {
        return;
    }

    // ------------------------------------------------------------ 3. perturbations, one at a time
    println!("\n== perturbations (each alone on a copy of what was read)");
    let worst_a_cell = worst_a.1.split(' ').next().unwrap().to_string();
    let wa = cell(&rep, &worst_a_cell);
    let wa_band = wa
        .a
        .as_ref()
        .unwrap()
        .bands
        .iter()
        .enumerate()
        .max_by(|x, y| x.1.interval.unwrap().reach().total_cmp(&y.1.interval.unwrap().reach()))
        .unwrap()
        .0;
    // A: that band's Kuttruff time, in every seed (bitwise equal, E3), scaled by g.
    let a_reach = |g: f64| -> (Report, f64) {
        let mut r = reads.clone();
        for (_, s) in r.spps.get_mut(&worst_a_cell).unwrap().iter_mut() {
            let s = s.as_mut().unwrap();
            let k = s.reference.bands[wa_band].kuttruff_s.unwrap();
            s.reference.bands[wa_band].kuttruff_s = Some(k * g);
        }
        let j = judge(&l, &r, &ts, &rep);
        let x = cell(&j, &worst_a_cell).a.as_ref().unwrap().bands[wa_band].interval.unwrap().reach();
        (j, x)
    };
    let g_out = solve(1.0, 1.2, 0.0501, |g| a_reach(g).1);
    let g_in = solve(1.0, 1.2, 0.0499, |g| a_reach(g).1);
    for (name, g) in [("A just outside", g_out), ("A just inside", g_in)] {
        let (j, x) = a_reach(g);
        let c = cell(&j, &worst_a_cell);
        println!(
            "  {name}: {} band {} Kuttruff x{g:.5}: reach {:.3} %; A {:?}, B {:?}, C {:?}, cell {:?}",
            worst_a_cell,
            c.bands_hz[wa_band],
            100.0 * x,
            c.a.as_ref().map(|a| a.verdict),
            c.b.as_ref().map(|b| b.verdict),
            c.c.as_ref().map(|b| b.verdict),
            c.verdict
        );
        line(name, &j);
    }

    // B: the seed with the largest cell mean of the worst-B cell, scaled by h.
    let wb = cell(&rep, &worst_b.1);
    let bb = wb.b.as_ref().unwrap();
    let top = bb.seeds[bb
        .cell_means
        .iter()
        .enumerate()
        .max_by(|x, y| x.1.total_cmp(y.1))
        .unwrap()
        .0];
    let b_spread = |h: f64| -> (Report, f64) {
        let mut r = reads.clone();
        scale_cell(&mut r, &worst_b.1, &[top], h);
        let j = judge(&l, &r, &ts, &rep);
        let x = cell(&j, &worst_b.1).b.as_ref().unwrap().spread;
        (j, x)
    };
    let h_out = solve(1.0, 1.05, 0.0201, |h| b_spread(h).1);
    let h_in = solve(1.0, 1.05, 0.0199, |h| b_spread(h).1);
    for (name, h) in [("B just outside", h_out), ("B just inside", h_in)] {
        let (j, x) = b_spread(h);
        let c = cell(&j, &worst_b.1);
        println!(
            "  {name}: {} seed {top} x{h:.5}: spread {:.3} %; A {:?}, B {:?}, C {:?}, cell {:?}",
            worst_b.1,
            100.0 * x,
            c.a.as_ref().map(|a| a.verdict),
            c.b.as_ref().map(|b| b.verdict),
            c.c.as_ref().map(|b| b.verdict),
            c.verdict
        );
        line(name, &j);
    }

    // C: every T30 of the worst-C cell scaled by f (A barely moves, B is scale-free).
    let wc = worst_c.1.split(':').next().unwrap().to_string();
    let c_reach = |f: f64| -> (Report, f64) {
        let mut r = reads.clone();
        scale_cell(&mut r, &wc, &(1..=10).collect::<Vec<_>>(), f);
        let j = judge(&l, &r, &ts, &rep);
        let x = cell(&j, &wc).c.as_ref().unwrap().interval.reach();
        (j, x)
    };
    let d0 = cell(&rep, &wc).c.as_ref().unwrap().interval.mean;
    let (lo, hi) = if d0 >= 0.0 { (1.0, 1.02) } else { (0.98, 1.0) };
    let f_out = solve(lo, hi, 0.0051, |f| c_reach(f).1);
    let f_in = solve(lo, hi, 0.0049, |f| c_reach(f).1);
    for (name, f) in [("C just outside", f_out), ("C just inside", f_in)] {
        let (j, x) = c_reach(f);
        let c = cell(&j, &wc);
        println!(
            "  {name}: {wc} all T30 x{f:.6}: reach {:.3} %; A {:?}, B {:?}, C {:?}, cell {:?}; needs_extension {:?}",
            100.0 * x,
            c.a.as_ref().map(|a| a.verdict),
            c.b.as_ref().map(|b| b.verdict),
            c.c.as_ref().map(|b| b.verdict),
            c.verdict,
            j.needs_extension
        );
        line(name, &j);
    }

    // D: one band of the worst-D gated TCR run, TCR's Eyring time at analytic x (1 +- 0.0051/0.0049).
    let wd = worst_d.1.split(' ').next().unwrap().to_string();
    for (name, k) in [("D just outside", 1.0051), ("D just inside", 1.0049)] {
        let mut r = reads.clone();
        let t = r.tcr.get_mut(&wd).unwrap().as_mut().unwrap();
        let b = &mut t.bands[0];
        b.eyring_s = b.analytic_eyring_s.unwrap() * k;
        let j = judge(&l, &r, &ts, &rep);
        let t = j.tcr.iter().find(|t| t.id == wd).unwrap();
        println!("  {name}: {wd} {} Hz x{k}: D {:?}", t.d.as_ref().unwrap().bands[0].freq_hz, t.verdict);
        line(name, &j);
    }

    // Missing or refused data in a gated cell or run.
    let id = "6x10x3-a0.2-random-air-on".to_string();
    let tid = "6x10x3-a0.2-tcr-air-on".to_string();
    let mut cases: Vec<(&str, Reads, Transports)> = Vec::new();
    {
        let mut r = reads.clone();
        r.spps.get_mut(&id).unwrap().remove(&7);
        cases.push(("seed 7's run missing", r, ts.clone()));
    }
    {
        let mut r = reads.clone();
        r.spps.get_mut(&id).unwrap().get_mut(&3).unwrap().as_mut().unwrap().t30[1][4] = T30 {
            t: None,
            mc_sd: None,
            source: "range_not_reached".into(),
        };
        cases.push(("one T30 refused range_not_reached", r, ts.clone()));
    }
    {
        let mut r = reads.clone();
        r.spps.get_mut(&id).unwrap().get_mut(&3).unwrap().as_mut().unwrap().t30[1][4].source =
            "truncated".into();
        cases.push(("one T30 relabelled truncated, value kept", r, ts.clone()));
    }
    {
        let mut r = reads.clone();
        r.spps.get_mut(&id).unwrap().get_mut(&3).unwrap().as_mut().unwrap().t30[1][4].t = Some(f64::NAN);
        cases.push(("one T30 NaN", r, ts.clone()));
    }
    {
        let mut r = reads.clone();
        r.spps.get_mut(&id).unwrap().get_mut(&5).unwrap().as_mut().unwrap().info.status = "FAIL".into();
        cases.push(("seed 5's run FAIL", r, ts.clone()));
    }
    {
        let mut r = reads.clone();
        r.spps.get_mut(&id).unwrap().get_mut(&5).unwrap().as_mut().unwrap().t30.pop();
        cases.push(("seed 5 with 2 receivers", r, ts.clone()));
    }
    {
        let mut r = reads.clone();
        for (_, s) in r.spps.get_mut(&id).unwrap().iter_mut() {
            s.as_mut().unwrap().reference.bands[2].kuttruff_s = None;
        }
        cases.push(("Kuttruff refused in one band, every seed", r, ts.clone()));
    }
    {
        let mut t2 = ts.clone();
        let rf = cell(&rep, &id).reference.as_ref().unwrap();
        let m = rf.bands[3].bed_air_m_per_metre;
        let k = transport::key("6x10x3", 0.2, m);
        println!("  (removing transport {k}: present {})", t2.by_key.contains_key(&k));
        t2.by_key.remove(&k);
        cases.push(("one band's transport missing", reads.clone(), t2));
    }
    {
        let mut r = reads.clone();
        r.tcr.remove(&tid);
        cases.push(("a gated TCR run missing", r, ts.clone()));
    }
    {
        let mut r = reads.clone();
        r.tcr.get_mut(&tid).unwrap().as_mut().unwrap().bands[2].analytic_eyring_s = None;
        cases.push(("a gated TCR band's analytic time missing", r, ts.clone()));
    }
    {
        let mut r = reads.clone();
        let t = r.tcr.get_mut(&tid).unwrap().as_mut().unwrap();
        t.bands.pop();
        cases.push(("a gated TCR run with its 8 kHz band missing", r, ts.clone()));
    }
    {
        let mut r = reads.clone();
        let t = r.tcr.get_mut(&tid).unwrap().as_mut().unwrap();
        t.bands.truncate(1);
        cases.push(("a gated TCR run with only its 125 Hz band", r, ts.clone()));
    }
    {
        let mut r = reads.clone();
        scale_cell(&mut r, "20x8x4-a0.4-random-air-off", &(1..=10).collect::<Vec<_>>(), 1.3);
        cases.push(("a reported cell 30 % long (must NOT fail the bed)", r, ts.clone()));
    }
    for (name, r, t) in &cases {
        let j = judge(&l, r, t, &rep);
        let c = cell(&j, &id);
        let tc = j.tcr.iter().find(|t| t.id == tid).unwrap();
        println!(
            "  {name}: cell {:?} (E2 {} E3 {} E6 {}), TCR {:?} D bands {}",
            c.verdict,
            c.e2.holds,
            c.e3.holds,
            c.e6.holds,
            tc.verdict,
            tc.d.as_ref().map_or(0, |d| d.bands.len())
        );
        line(name, &j);
    }
    // A receiver with a band short: not reachable from read_spps (it refuses such a run), shown
    // only to know whether the judgement would crash or pass.
    let mut r = reads.clone();
    r.spps.get_mut(&id).unwrap().get_mut(&5).unwrap().as_mut().unwrap().t30[0].pop();
    let out = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| judge(&l, &r, &ts, &rep).pass));
    println!("  a receiver one band short (unreachable from read_spps): {:?}", out.map_err(|_| "panicked"));
}
