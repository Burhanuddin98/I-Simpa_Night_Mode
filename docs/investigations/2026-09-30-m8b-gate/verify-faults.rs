//! The say-NO reads N2 and N8 (bed_m8a.rs), with their faults set, through run::into_reads: are
//! any of the M8a bed's real runs refused by check_planned under a fault? And does the fault still
//! reach the verdict (A fails at alpha 0.4 under N2; D fails under N8)? Read-only on the bed.

use std::path::{Path, PathBuf};

use simpa_core::bed::check::{Reads, Verdict};
use simpa_core::bed::file;
use simpa_core::bed::report::{self, Report};
use simpa_core::bed::run::{self, RunKey};
use simpa_core::bed::transport::{self, Transports};
use simpa_core::faults::{self, Fault};

const ROOT: &str = r"C:\tmp\nm-m8a-bed\20260929T093134Z";
const BED: &str = r"B:\repos\I-Simpa_Night_Mode\.claude\worktrees\m8b\beds\m8a.json";

fn main() {
    let l = file::load(Path::new(BED)).unwrap();
    let root = PathBuf::from(ROOT);
    let rep: Report =
        serde_json::from_str(&std::fs::read_to_string(root.join("report.json")).unwrap()).unwrap();
    let mut ts = Transports::default();
    for x in &rep.transports {
        ts.by_key.insert(transport::key(&x.room, x.alpha, x.air_m_per_metre), Ok(x.clone()));
    }
    let (all, _) = run::plan(&l.bed, None).unwrap();
    let a04: Vec<String> = l.bed.cells.iter().filter(|c| c.gated && c.alpha == 0.4).map(|c| c.id.clone()).collect();
    let judge = |reads: &Reads| {
        report::evaluate(&report::Inputs {
            bed: &l.bed,
            reads,
            transports: &ts,
            exploratory: &l.exploratory,
            solvers: &rep.preconditions.e1.solvers,
        })
    };
    for (name, fault, keep) in [
        ("N2 (KuttruffFullVariance), alpha 0.4 gated cells", Fault::KuttruffFullVariance, 0),
        ("N8 (TcrAnalyticPhysicalConstant), TCR runs", Fault::TcrAnalyticPhysicalConstant, 1),
    ] {
        let runs: Vec<_> = all
            .iter()
            .filter(|p| match &p.key {
                RunKey::Cell { id, .. } => keep == 0 && a04.contains(id),
                RunKey::Tcr { .. } => keep == 1,
                _ => false,
            })
            .cloned()
            .collect();
        let reads = faults::with(fault, || {
            let results = run::read_existing(&runs, &root, 8);
            let mut refused = 0;
            for (p, r) in runs.iter().zip(&results) {
                match r {
                    Ok(read) => {
                        if let Err(e) = run::check_planned(p, read) {
                            refused += 1;
                            println!("  REFUSED {}: {e}", p.key.label());
                        }
                    }
                    Err(e) => {
                        refused += 1;
                        println!("  NOT READ {}: {e}", p.key.label());
                    }
                }
            }
            println!("{name}: {} runs read, {refused} refused or not read", runs.len());
            let mut reads = Reads::default();
            run::into_reads(&mut reads, &runs, results, Some("not read".into()));
            reads
        });
        let j = judge(&reads);
        if keep == 0 {
            let v: Vec<String> = j
                .cells
                .iter()
                .filter(|c| a04.contains(&c.id))
                .map(|c| format!("{} A {:?}", c.id, c.a.as_ref().map(|a| a.verdict)))
                .collect();
            let fails = j.cells.iter().filter(|c| a04.contains(&c.id) && c.a.as_ref().map(|a| a.verdict) == Some(Verdict::Fail)).count();
            println!("  A Fail in {fails} of {} alpha 0.4 gated cells: {v:?}", a04.len());
        } else {
            let gated: Vec<_> = j.tcr.iter().filter(|t| t.gated).collect();
            let bands: usize = gated.iter().filter_map(|t| t.d.as_ref()).map(|d| d.bands.len()).sum();
            let failed: usize = gated
                .iter()
                .filter_map(|t| t.d.as_ref())
                .map(|d| d.bands.iter().filter(|b| b.verdict == Verdict::Fail).count())
                .sum();
            let probs: usize = gated.iter().filter_map(|t| t.d.as_ref()).map(|d| d.band_problems.len()).sum();
            println!("  gated TCR runs {}: D bands {bands}, of them Fail {failed}; band problems {probs}", gated.len());
        }
    }
}
