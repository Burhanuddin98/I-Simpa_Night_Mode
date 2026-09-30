//! M8a's say-NO partners that re-read a bed's own runs through test-only faults
//! (`docs/investigations/2026-09-29-m8a/SPEC.md`, section 7, step 6), and the check that its
//! `report.json` validates against its schema. Each needs a bed that has run, so each is ignored
//! and run on purpose by `tools/gates/m8a.ps1`, in a release build:
//!
//! `SIMPA_BED_FROM=<root>/<stamp> cargo test --release -p simpa --test bed_m8a -- --ignored
//! --exact --nocapture <name>`
//!
//! `$SIMPA_BED_FROM` is the folder `simpa bed` wrote (`runs/`, `report.json`), and
//! `$SIMPA_BED_REPORT` a `report.json` elsewhere (a bed run with `--from`); `$SIMPA_BED_FILE`
//! the bed file (default `beds/m8a.json`); `$SIMPA_BED_SEAL` the seal the runs are read against,
//! which is required and must lie outside the bed folder: the bed's committed seal (M8a's:
//! `beds/m8a-<stamp>/outputs-seal.json`), or for a bed the gate has just run, the copy it took of
//! the seal `simpa bed` wrote of it; `$SIMPA_BED_JOBS` how many runs are read at once (default
//! 4). No solver runs: the faults go through the code that reads and judges the runs.
//!
//! - N2: Kuttruff with its ½ dropped (`Fault::KuttruffFullVariance`, through `results::reference`):
//!   A fails in every α 0.4 gated cell.
//! - N3: plain Eyring in place of Kuttruff as A's reference (`Fault::BedEyringReference`): A
//!   fails in every α 0.4 gated cell.
//! - N4: the transport reflecting evenly over the hemisphere (`Fault::LambertUniformReflection`),
//!   traced again: C fails in every gated cell.
//! - N7: the transport traced with the air off (`Fault::BedTransportAirOff`): C fails in every
//!   air-on gated cell.
//! - N8: TCR's analytic times with the physical K (`Fault::TcrAnalyticPhysicalConstant`, through
//!   `results::tcr`, over the planned project's analytic time that D holds TCR to): D fails in
//!   every band of every gated TCR run.

use std::path::{Path, PathBuf};

use simpa_core::bed::bind;
use simpa_core::bed::check::{Reads, Verdict};
use simpa_core::bed::file::{self, BedFile};
use simpa_core::bed::report::{self, Report};
use simpa_core::bed::run::{self, Planned, RunKey};
use simpa_core::bed::transport::Transports;
use simpa_core::faults::{self, Fault};

fn from() -> PathBuf {
    let p = PathBuf::from(
        std::env::var_os("SIMPA_BED_FROM")
            .expect("set SIMPA_BED_FROM to the folder simpa bed wrote (runs/, report.json)"),
    );
    assert!(p.join("runs").is_dir(), "{}: no runs/", p.display());
    p
}

/// `$SIMPA_BED_SEAL`, loaded and checked against the bed folder `from`, which must not hold it.
/// Required: an earlier bed's runs are read only against a seal (`bed::run::read_existing`).
fn seal(from: &Path) -> bind::Seal {
    let path = std::env::var_os("SIMPA_BED_SEAL")
        .filter(|v| !v.is_empty())
        .expect(
            "set SIMPA_BED_SEAL to the seal of the bed in SIMPA_BED_FROM: its runs are read only              against a seal, never on their own run.json's word",
        );
    let path = Path::new(&path);
    let (s, _) = bind::Seal::load(path).unwrap();
    s.for_bed(from).unwrap();
    bind::seal_outside(from, path).unwrap();
    s
}

fn jobs() -> usize {
    std::env::var("SIMPA_BED_JOBS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(4)
}

fn bed() -> file::Loaded {
    let path = std::env::var_os("SIMPA_BED_FILE")
        .map(PathBuf::from)
        .unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("../../beds/m8a.json"));
    file::load(&path).unwrap()
}

/// The bed's own report: `$SIMPA_BED_REPORT`, or the one beside the runs.
fn earlier(from: &Path) -> Report {
    let path = std::env::var_os("SIMPA_BED_REPORT")
        .map(PathBuf::from)
        .unwrap_or_else(|| from.join("report.json"));
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    serde_json::from_str(&text).unwrap()
}

/// The transports the bed traced, as its report holds them.
fn traced(rep: &Report) -> Transports {
    let mut t = Transports::default();
    for x in &rep.transports {
        t.by_key.insert(
            simpa_core::bed::transport::key(&x.room, x.alpha, x.air_m_per_metre),
            Ok(x.clone()),
        );
    }
    t
}

/// The runs of `from` whose key `keep` takes, the extension's that are there too, read again with
/// this build, on the calling thread's fault.
fn read(bed: &BedFile, from: &Path, keep: impl Fn(&RunKey) -> bool) -> Reads {
    let (runs, _) = run::plan(bed, None).unwrap();
    let gated: Vec<String> = bed
        .cells
        .iter()
        .filter(|c| c.gated)
        .map(|c| c.id.clone())
        .collect();
    let mut runs: Vec<Planned> = runs.into_iter().filter(|p| keep(&p.key)).collect();
    runs.extend(
        run::plan_extension(bed, &gated)
            .unwrap()
            .into_iter()
            .filter(|p| keep(&p.key) && from.join(p.key.dir()).is_dir()),
    );
    let results = run::read_existing(&runs, from, jobs(), &seal(from));
    let mut reads = Reads::default();
    run::into_reads(&mut reads, &runs, results, Some("not read".into()));
    reads
}

fn judge(l: &file::Loaded, reads: &Reads, transports: &Transports, rep: &Report) -> Report {
    report::evaluate(&report::Inputs {
        bed: &l.bed,
        reads,
        transports,
        exploratory: &l.exploratory,
        solvers: &rep.preconditions.e1.solvers,
    })
}

/// The gated SPPS cells at `alpha`.
fn cells_at(bed: &BedFile, alpha: f64) -> Vec<String> {
    bed.cells
        .iter()
        .filter(|c| c.gated && c.alpha == alpha)
        .map(|c| c.id.clone())
        .collect()
}

/// Prints each named cell's check, and requires it to fail (never pass, never go unjudged: a
/// cell the bed could not judge shows nothing).
fn require_fail(rep: &Report, cells: &[String], check: &str) {
    assert!(!cells.is_empty());
    let mut wrong = Vec::new();
    for id in cells {
        let c = rep.cells.iter().find(|c| &c.id == id).unwrap();
        let v = match check {
            "A" => c.a.as_ref().map(|a| a.verdict),
            "C" => c.c.as_ref().map(|x| x.verdict),
            _ => unreachable!(),
        };
        let numbers = match check {
            "A" => {
                c.a.as_ref()
                    .map(|a| {
                        a.bands
                            .iter()
                            .map(|b| {
                                format!(
                                    "{} Hz {:+.2} %",
                                    b.freq_hz,
                                    100.0 * b.interval.map_or(f64::NAN, |i| i.mean)
                                )
                            })
                            .collect::<Vec<_>>()
                            .join(", ")
                    })
                    .unwrap_or_default()
            }
            _ => {
                c.c.as_ref()
                    .map(|x| {
                        format!(
                            "d {:+.3} %, [{:+.3}, {:+.3}] %",
                            100.0 * x.interval.mean,
                            100.0 * x.interval.lo,
                            100.0 * x.interval.hi
                        )
                    })
                    .unwrap_or_default()
            }
        };
        println!("{id}: {check} {v:?}: {numbers}");
        if v != Some(Verdict::Fail) {
            wrong.push(format!("{id}: {check} {v:?}"));
        }
    }
    assert!(
        wrong.is_empty(),
        "the say-NO did not make {check} fail: {wrong:?}"
    );
}

#[test]
#[ignore = "needs a bed that has run: SIMPA_BED_FROM; run by tools/gates/m8a.ps1"]
fn report_validates_against_its_schema() {
    // `$SIMPA_BED_REPORT`, or the report beside the runs (a bed run with --from writes its report
    // in a folder of its own).
    let path = std::env::var_os("SIMPA_BED_REPORT")
        .map(PathBuf::from)
        .unwrap_or_else(|| from().join("report.json"));
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let value: serde_json::Value = serde_json::from_str(&text).unwrap();
    let schema = serde_json::to_value(report::schema()).unwrap();
    let mut schemas = boon::Schemas::new();
    let mut compiler = boon::Compiler::new();
    compiler.add_resource("bed-report.json", schema).unwrap();
    let idx = compiler.compile("bed-report.json", &mut schemas).unwrap();
    if let Err(e) = schemas.validate(&value, idx) {
        panic!("report.json does not validate: {e:#}");
    }
    // And it reads back into the report it was written from.
    let rep: Report = serde_json::from_value(value).unwrap();
    println!(
        "report.json validates: pass {}, exploratory {}, {} cells, {} failures",
        rep.pass,
        rep.exploratory,
        rep.cells.len(),
        rep.failures.len()
    );
}

#[test]
#[ignore = "needs a bed that has run: SIMPA_BED_FROM; run by tools/gates/m8a.ps1"]
fn n2_kuttruff_without_its_half_fails_a() {
    let (from, l) = (from(), bed());
    let rep = earlier(&from);
    let cells = cells_at(&l.bed, 0.4);
    let reads = faults::with(Fault::KuttruffFullVariance, || {
        read(
            &l.bed,
            &from,
            |k| matches!(k, RunKey::Cell { id, .. } if cells.contains(id)),
        )
    });
    let judged = judge(&l, &reads, &traced(&rep), &rep);
    require_fail(&judged, &cells, "A");
}

#[test]
#[ignore = "needs a bed that has run: SIMPA_BED_FROM; run by tools/gates/m8a.ps1"]
fn n3_plain_eyring_as_the_reference_fails_a() {
    let (from, l) = (from(), bed());
    let rep = earlier(&from);
    let cells = cells_at(&l.bed, 0.4);
    let reads = read(
        &l.bed,
        &from,
        |k| matches!(k, RunKey::Cell { id, .. } if cells.contains(id)),
    );
    let judged = faults::with(Fault::BedEyringReference, || {
        judge(&l, &reads, &traced(&rep), &rep)
    });
    require_fail(&judged, &cells, "A");
}

#[test]
#[ignore = "needs a bed that has run: SIMPA_BED_FROM; traces every transport again (about half an \
            hour); run by tools/gates/m8a.ps1"]
fn n4_a_transport_that_does_not_reflect_by_lamberts_law_fails_c() {
    let (from, l) = (from(), bed());
    let rep = earlier(&from);
    let cells: Vec<String> = l
        .bed
        .cells
        .iter()
        .filter(|c| c.gated)
        .map(|c| c.id.clone())
        .collect();
    let reads = read(
        &l.bed,
        &from,
        |k| matches!(k, RunKey::Cell { id, .. } if cells.contains(id)),
    );
    let judged = faults::with(Fault::LambertUniformReflection, || {
        let mut t = Transports::default();
        run::trace_transports(&l.bed, &mut t, &|i, n, what, s| {
            println!("uniform-reflection transport {i}/{n} {what}: {s:.0} s")
        });
        judge(&l, &reads, &t, &rep)
    });
    require_fail(&judged, &cells, "C");
}

#[test]
#[ignore = "needs a bed that has run: SIMPA_BED_FROM; traces the air-off transports again; run by \
            tools/gates/m8a.ps1"]
fn n7_the_transport_without_air_fails_c_in_every_air_on_cell() {
    let (from, l) = (from(), bed());
    let rep = earlier(&from);
    let cells: Vec<String> = l
        .bed
        .cells
        .iter()
        .filter(|c| c.gated && c.air)
        .map(|c| c.id.clone())
        .collect();
    let reads = read(
        &l.bed,
        &from,
        |k| matches!(k, RunKey::Cell { id, .. } if cells.contains(id)),
    );
    let judged = faults::with(Fault::BedTransportAirOff, || {
        let mut t = Transports::default();
        run::trace_transports(&l.bed, &mut t, &|i, n, what, s| {
            println!("air-off transport {i}/{n} {what}: {s:.0} s")
        });
        judge(&l, &reads, &t, &rep)
    });
    require_fail(&judged, &cells, "C");
}

#[test]
#[ignore = "needs a bed that has run: SIMPA_BED_FROM; run by tools/gates/m8a.ps1"]
fn n8_tcrs_analytic_time_with_the_physical_constant_fails_d() {
    let (from, l) = (from(), bed());
    let rep = earlier(&from);
    // Since M8b, D's reference is the analytic time of the planned project, computed when the
    // runs are judged: the fault is held over the judging as well as the reading.
    let judged = faults::with(Fault::TcrAnalyticPhysicalConstant, || {
        let reads = read(&l.bed, &from, |k| matches!(k, RunKey::Tcr { .. }));
        judge(&l, &reads, &traced(&rep), &rep)
    });
    let gated: Vec<_> = judged.tcr.iter().filter(|t| t.gated).collect();
    assert!(!gated.is_empty());
    let mut wrong = Vec::new();
    for t in gated {
        let d = t.d.as_ref().expect("D was judged");
        for b in &d.bands {
            println!(
                "{} {} Hz: D {:?}, {:+.3} %",
                t.id,
                b.freq_hz,
                b.verdict,
                100.0 * b.deviation.unwrap_or(f64::NAN)
            );
            if b.verdict != Verdict::Fail {
                wrong.push(format!("{} {} Hz: {:?}", t.id, b.freq_hz, b.verdict));
            }
        }
    }
    assert!(wrong.is_empty(), "D did not fail: {wrong:?}");
}
