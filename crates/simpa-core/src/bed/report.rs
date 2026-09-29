//! `report.json` and `summary.json` (`docs/investigations/2026-09-29-m8a/SPEC.md`, section 6),
//! and [`evaluate`], which judges everything the bed read.
//!
//! **`pass` is true exactly when** A, B and C pass in every gated cell, D in every gated TCR run,
//! and E1 to E7 hold (section 5.2). E5's cargo test is run by the gate
//! (`tools/gates/m8a.ps1`); the bed holds its own part of it, Kuttruff as shipped within 0.6 % of
//! the committed transport T30s in the eight air-off gated cells.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::check::{
    self, CellContext, CellReport, Precondition, RandomAgainstEnergetic, Reads, SeedRun, TcrReport,
    Verdict,
};
use super::file::BedFile;
use super::limits;
use super::pe::SolverCheck;
use super::read::RunInfo;
use super::stats::{self, Interval};
use super::transport::{self, TransportT30, Transports};

/// The layout of [`Report`]; bumped when a field changes meaning.
pub const BED_REPORT_VERSION: u32 = 1;

/// Where and on what the bed ran. Filled by the caller; [`evaluate`] leaves it default.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Meta {
    pub bed_file: String,
    pub bed_raw_sha256: String,
    pub bed_canonical_sha256: String,
    pub spec: String,
    pub git_commit: Option<String>,
    /// `git status --porcelain` printed something.
    pub git_dirty: Option<bool>,
    pub simpa_core_version: String,
    pub solver_commit: String,
    /// `solvers/manifest.json` as compiled in, line ends as git stores them.
    pub solver_manifest_sha256: String,
    pub machine: Option<String>,
    pub logical_cpus: usize,
    pub started_utc: String,
    pub finished_utc: String,
    pub wall_s: f64,
    /// The folder this bed wrote to, and the earlier one it read runs from (`--from`).
    pub root: String,
    pub from: Option<String>,
    pub jobs: usize,
    pub solver_phase_s: Option<f64>,
    pub transport_phase_s: Option<f64>,
    /// The limits the checks used (constants of the code, not of the bed file).
    pub limits: BTreeMap<String, f64>,
}

/// E1: the executables against `solvers/manifest.json` before any run, and every run's
/// executable after.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct E1 {
    pub solvers: Vec<SolverCheck>,
    pub runs_checked: usize,
    pub holds: bool,
    pub problems: Vec<String>,
}

/// One cell of E4.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct E4Cell {
    pub room: String,
    pub alpha: f64,
    pub t: Option<f64>,
    pub se: Option<f64>,
    pub room_t: Option<f64>,
    pub room_se: Option<f64>,
    pub high: [f64; 4],
    /// The largest of the four differences, s.
    pub max_difference_s: Option<f64>,
}

/// E4: the transport reproduces the committed high-count values.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct E4 {
    pub cells: Vec<E4Cell>,
    pub limit_s: f64,
    pub holds: bool,
    pub problems: Vec<String>,
}

/// One cell of E5's in-bed part.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct E5Cell {
    pub room: String,
    pub alpha: f64,
    pub kuttruff_s: Option<f64>,
    /// `T_K/HIGH.t − 1` and `T_K/HIGH.room_t − 1`.
    pub against_receivers: Option<f64>,
    pub against_room_energy: Option<f64>,
}

/// E5: Kuttruff within 0.6 % of the committed transport T30s. The cargo test is the gate's.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct E5 {
    pub cells: Vec<E5Cell>,
    pub limit: f64,
    pub holds: bool,
    pub problems: Vec<String>,
    pub cargo_test: String,
}

/// E1 to E7.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Preconditions {
    pub e1: E1,
    /// Every run of the matrix (cells and TCR, gated and reported) OK and read.
    pub e2: Precondition,
    /// Every gated cell's reference.
    pub e3: Precondition,
    pub e4: E4,
    pub e5: E5,
    /// Every gated cell's values judged.
    pub e6: Precondition,
    /// The bed file is M8a's matrix.
    pub e7: Precondition,
}

/// One band of upstream's atmospheric-absorption validation (R11).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct AtmosphericBand {
    pub freq_hz: u32,
    /// The mean over the seeds whose T30 is a value or a noise refusal, and its interval; the
    /// number of such seeds.
    pub spps: Option<Interval>,
    pub seeds_judged: usize,
    pub sources: BTreeMap<String, usize>,
    pub tcr_eyring_s: Option<f64>,
    pub kuttruff_s: Option<f64>,
    pub xlsx_spps_2018_s: f64,
    pub xlsx_spps_1_3_4_s: f64,
    pub xlsx_tcr_eyring_s: f64,
    pub xlsx_md_octave_s: f64,
    /// Our SPPS mean against each, `ours/theirs − 1`; our TCR against theirs.
    pub against_spps_2018: Option<f64>,
    pub against_spps_1_3_4: Option<f64>,
    pub against_md_octave: Option<f64>,
    pub tcr_against_xlsx: Option<f64>,
}

/// R11: reported, not gated, and not like for like (upstream ran seed 0, 10 ms, ε 6).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct AtmosphericReport {
    pub not_run: Option<String>,
    pub note: String,
    pub runs: Vec<SeedRun>,
    pub tcr: Option<RunInfo>,
    pub tcr_error: Option<String>,
    pub bands: Vec<AtmosphericBand>,
}

/// N5: the cell with one seed replaced by a run at a thousandth of its particles.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct N5Report {
    pub cell: String,
    pub seed: u32,
    pub run: Option<RunInfo>,
    pub error: Option<String>,
    pub cell_verdict: Option<Verdict>,
    /// What stopped the cell: `B`, `E6`, ...
    pub failed_by: Vec<String>,
    pub failures: Vec<String>,
    /// The cell did not pass.
    pub as_required: bool,
}

/// N6: walls of scattering 0, which the bed must refuse (E3).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct N6Report {
    pub run: Option<RunInfo>,
    pub error: Option<String>,
    /// Every band's `kuttruff_s` refusal code.
    pub codes: Vec<Option<String>>,
    pub e3_problems: Vec<String>,
    /// E3 refused the cell, `params_reference_not_applicable` in every band.
    pub as_required: bool,
}

/// The bed's own say-NO runs.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SayNoReport {
    pub n5: N5Report,
    pub n6: N6Report,
}

/// What the bed left on disk.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Files {
    /// Projected before the runs (`bed::run::projected_files`).
    pub projected: u64,
    /// Counted under the bed's folder (and `--from`'s runs, which it reads) before `report.json`
    /// and `summary.json` were written.
    pub files: u64,
    pub bytes: u64,
    pub filesystem: Option<String>,
}

/// `report.json`.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Report {
    /// [`BED_REPORT_VERSION`].
    pub bed_report_version: u32,
    pub pass: bool,
    /// The bed file is not M8a's matrix: the report never passes.
    pub exploratory: bool,
    /// Every failed or unjudged gated check, with its numbers.
    pub failures: Vec<String>,
    /// Gated cells whose C is INCONCLUSIVE on seeds 1 to 10 and has not been extended.
    pub needs_extension: Vec<String>,
    pub meta: Meta,
    pub preconditions: Preconditions,
    pub cells: Vec<CellReport>,
    pub tcr: Vec<TcrReport>,
    /// R7.
    pub random_against_energetic: Vec<RandomAgainstEnergetic>,
    /// R11.
    pub atmospheric_validation: AtmosphericReport,
    pub say_no: SayNoReport,
    /// Every transport traced, successful ones; `transport_errors` the others.
    pub transports: Vec<TransportT30>,
    pub transport_errors: BTreeMap<String, String>,
    /// Failures of the reported cells and runs: information, never a failure of the bed.
    pub reported_notes: Vec<String>,
    pub files: Files,
}

/// The JSON Schema of `report.json`.
pub fn schema() -> schemars::Schema {
    schemars::schema_for!(Report)
}

/// The limits, by name, for [`Meta::limits`].
pub fn limits_map() -> BTreeMap<String, f64> {
    [
        ("a_kuttruff", limits::KUTTRUFF),
        ("b_seed_spread", limits::SEED_SPREAD),
        ("c_transport", limits::TRANSPORT),
        ("d_tcr_eyring", limits::TCR_EYRING),
        ("e4_high_s", limits::HIGH_TOLERANCE_S),
        ("e5_kuttruff_against_high", limits::KUTTRUFF_AGAINST_HIGH),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_string(), v))
    .collect()
}

/// What [`evaluate`] judges.
pub struct Inputs<'a> {
    pub bed: &'a BedFile,
    pub reads: &'a Reads,
    pub transports: &'a Transports,
    /// E7: why the bed file is exploratory; empty for M8a's matrix.
    pub exploratory: &'a [String],
    /// E1 before any run.
    pub solvers: &'a [SolverCheck],
}

/// The raw sha256 E1 found for `name`.
fn raw_of<'a>(solvers: &'a [SolverCheck], name: &str) -> Option<&'a str> {
    solvers
        .iter()
        .find(|s| s.name == name)
        .and_then(|s| s.raw_sha256.as_deref())
}

/// Judges everything read ([module docs](self)); [`Meta`] and [`Files`] are left to the caller.
pub fn evaluate(i: &Inputs) -> Report {
    let bed = i.bed;
    let ctx = CellContext {
        bed,
        transports: i.transports,
    };
    let cells: Vec<CellReport> = bed
        .cells
        .iter()
        .map(|c| check::evaluate_cell(&ctx, c, i.reads.spps.get(&c.id), None))
        .collect();
    let tcr: Vec<TcrReport> = bed
        .tcr
        .iter()
        .map(|t| {
            let k = cells
                .iter()
                .find(|c| c.id == t.project_of)
                .and_then(|c| c.reference.as_ref());
            check::evaluate_tcr(t, i.reads.tcr.get(&t.id), k)
        })
        .collect();
    let mut failures = Vec::new();
    let mut reported_notes = Vec::new();

    // E1: before, and after from every run's record.
    let mut e1 = E1 {
        solvers: i.solvers.to_vec(),
        ..Default::default()
    };
    for s in i.solvers.iter().filter(|s| !s.matches) {
        e1.problems.push(format!(
            "{} ({}): {}",
            s.name,
            s.path,
            s.detail.as_deref().unwrap_or("does not match")
        ));
    }
    for name in super::SOLVER_EXES {
        if !i.solvers.iter().any(|s| s.name == name) {
            e1.problems.push(format!("{name} was not checked"));
        }
    }
    let mut runs: Vec<(&str, &RunInfo)> = Vec::new();
    for c in &cells {
        for s in c
            .seeds
            .iter()
            .chain(c.extension.iter().flat_map(|e| e.seeds.iter()))
        {
            if let Some(r) = &s.run {
                runs.push(("spps.exe", r));
            }
        }
    }
    for t in &tcr {
        if let Some(r) = &t.run {
            runs.push(("classicalTheory.exe", r));
        }
    }
    for r in i.reads.atmospheric.spps.values().flatten() {
        runs.push(("spps.exe", &r.info));
    }
    if let Some(Ok(r)) = &i.reads.atmospheric.tcr {
        runs.push(("classicalTheory.exe", &r.info));
    }
    for r in [&i.reads.n5, &i.reads.n6].into_iter().flatten().flatten() {
        runs.push(("spps.exe", &r.info));
    }
    let tetgen = raw_of(i.solvers, "tetgen.exe");
    for (exe, r) in &runs {
        e1.runs_checked += 1;
        if raw_of(i.solvers, exe) != Some(r.exe_sha256.as_str()) {
            e1.problems.push(format!(
                "{}: ran {} of sha256 {}, not the checked {exe}",
                r.folder, r.exe, r.exe_sha256
            ));
        }
        if let Some(t) = &r.tetgen_sha256
            && tetgen != Some(t.as_str())
        {
            e1.problems.push(format!(
                "{}: meshed with a tetgen.exe of sha256 {t}, not the checked one",
                r.folder
            ));
        }
    }
    e1.holds = e1.problems.is_empty();

    // E2 over the matrix; E3 and E6 over the gated cells.
    let mut e2 = Vec::new();
    let (mut e3, mut e6) = (Vec::new(), Vec::new());
    for c in &cells {
        e2.extend(c.e2.problems.iter().map(|p| format!("{}: {p}", c.id)));
        if c.gated {
            e3.extend(c.e3.problems.iter().map(|p| format!("{}: {p}", c.id)));
            e6.extend(c.e6.problems.iter().map(|p| format!("{}: {p}", c.id)));
        }
    }
    for t in &tcr {
        if let Some(e) = &t.error {
            e2.push(format!("{}: {e}", t.id));
        } else if let Some(r) = t.run.as_ref().filter(|r| r.status != "OK") {
            e2.push(format!("{}: run {} is {}", t.id, r.folder, r.status));
        }
    }

    // E4 and E5 on the air-off gated cells that are M8's eight.
    let mut e4 = E4 {
        limit_s: limits::HIGH_TOLERANCE_S,
        ..Default::default()
    };
    let mut e5 = E5 {
        limit: limits::KUTTRUFF_AGAINST_HIGH,
        cargo_test: "cargo test --release -p simpa-core --test params_kuttruff: run by \
                     tools/gates/m8a.ps1"
            .into(),
        ..Default::default()
    };
    let mut seen = std::collections::BTreeSet::new();
    for c in cells.iter().filter(|c| c.gated && !c.air) {
        let Some(h) = transport::high(&c.room, c.alpha) else {
            continue;
        };
        if !seen.insert((c.room.clone(), c.alpha.to_bits())) {
            continue;
        }
        let tr = i.transports.get(&c.room, c.alpha, None);
        let t = tr.and_then(|t| t.as_ref().ok());
        let got = t.map(|t| [t.t, t.se, t.room_t, t.room_se]);
        let want = [h.t, h.se, h.room_t, h.room_se];
        let max = got.map(|g| {
            g.iter()
                .zip(&want)
                .map(|(a, b)| (a - b).abs())
                .fold(0.0, f64::max)
        });
        match max {
            Some(m) if m <= limits::HIGH_TOLERANCE_S => {}
            Some(m) => e4.problems.push(format!(
                "{} α {}: the transport is {m:.3e} s from HIGH (limit {:.0e} s): {got:?} \
                 against {want:?}",
                c.room,
                c.alpha,
                limits::HIGH_TOLERANCE_S
            )),
            None => e4.problems.push(format!(
                "{} α {}: no transport ({})",
                c.room,
                c.alpha,
                tr.and_then(|t| t.as_ref().err().cloned())
                    .unwrap_or_else(|| "not traced".into())
            )),
        }
        e4.cells.push(E4Cell {
            room: c.room.clone(),
            alpha: c.alpha,
            t: got.map(|g| g[0]),
            se: got.map(|g| g[1]),
            room_t: got.map(|g| g[2]),
            room_se: got.map(|g| g[3]),
            high: want,
            max_difference_s: max,
        });
        let k = c
            .reference
            .as_ref()
            .and_then(|r| r.bands.first())
            .and_then(|b| b.kuttruff_s);
        let (ar, ae) = (k.map(|k| k / h.t - 1.0), k.map(|k| k / h.room_t - 1.0));
        let within = |x: Option<f64>| x.is_some_and(|x| x.abs() <= limits::KUTTRUFF_AGAINST_HIGH);
        if !(within(ar) && within(ae)) {
            e5.problems.push(format!(
                "{} α {}: Kuttruff as shipped {k:?} s is {:?} of the receivers' and {:?} of the \
                 room energy's committed T30 (limit ±{:.1} %)",
                c.room,
                c.alpha,
                ar,
                ae,
                100.0 * limits::KUTTRUFF_AGAINST_HIGH
            ));
        }
        e5.cells.push(E5Cell {
            room: c.room.clone(),
            alpha: c.alpha,
            kuttruff_s: k,
            against_receivers: ar,
            against_room_energy: ae,
        });
    }
    e4.holds = e4.problems.is_empty() && !e4.cells.is_empty();
    e5.holds = e5.problems.is_empty() && !e5.cells.is_empty();
    let e7 = Precondition {
        holds: i.exploratory.is_empty(),
        problems: i.exploratory.to_vec(),
    };
    let preconditions = Preconditions {
        e1,
        e2: Precondition {
            holds: e2.is_empty(),
            problems: e2,
        },
        e3: Precondition {
            holds: e3.is_empty(),
            problems: e3,
        },
        e4,
        e5,
        e6: Precondition {
            holds: e6.is_empty(),
            problems: e6,
        },
        e7,
    };
    for (name, holds, problems) in [
        ("E1", preconditions.e1.holds, &preconditions.e1.problems),
        ("E2", preconditions.e2.holds, &preconditions.e2.problems),
        ("E3", preconditions.e3.holds, &preconditions.e3.problems),
        ("E4", preconditions.e4.holds, &preconditions.e4.problems),
        ("E5", preconditions.e5.holds, &preconditions.e5.problems),
        ("E6", preconditions.e6.holds, &preconditions.e6.problems),
        ("E7", preconditions.e7.holds, &preconditions.e7.problems),
    ] {
        if !holds {
            if problems.is_empty() {
                failures.push(format!("{name} does not hold: nothing to hold it on"));
            }
            failures.extend(problems.iter().map(|p| format!("{name}: {p}")));
        }
    }
    for c in &cells {
        if c.gated {
            failures.extend(c.failures.iter().cloned());
        } else {
            reported_notes.extend(c.failures.iter().cloned());
        }
    }
    for t in &tcr {
        if t.gated {
            failures.extend(t.failures.iter().cloned());
        } else {
            reported_notes.extend(t.failures.iter().cloned());
        }
    }
    let gated_cells: Vec<&CellReport> = cells.iter().filter(|c| c.gated).collect();
    let gated_tcr: Vec<&TcrReport> = tcr.iter().filter(|t| t.gated).collect();
    let needs_extension: Vec<String> = gated_cells
        .iter()
        .filter(|c| {
            c.extension.is_none()
                && c.c
                    .as_ref()
                    .is_some_and(|c| c.verdict == Verdict::Inconclusive)
        })
        .map(|c| c.id.clone())
        .collect();
    let pass = i.exploratory.is_empty()
        && preconditions.e1.holds
        && preconditions.e2.holds
        && preconditions.e3.holds
        && preconditions.e4.holds
        && preconditions.e5.holds
        && preconditions.e6.holds
        && preconditions.e7.holds
        && !gated_cells.is_empty()
        && gated_cells.iter().all(|c| c.verdict == Verdict::Pass)
        && gated_tcr.iter().all(|t| t.verdict == Verdict::Pass);
    if !pass && failures.is_empty() {
        failures.push("the bed has no gated cell".into());
    }
    let mut transports = Vec::new();
    let mut transport_errors = BTreeMap::new();
    for (k, t) in &i.transports.by_key {
        match t {
            Ok(t) => transports.push(t.clone()),
            Err(e) => {
                transport_errors.insert(k.clone(), e.clone());
            }
        }
    }
    Report {
        bed_report_version: BED_REPORT_VERSION,
        pass,
        exploratory: !i.exploratory.is_empty(),
        failures,
        needs_extension,
        meta: Meta {
            limits: limits_map(),
            ..Default::default()
        },
        random_against_energetic: check::random_against_energetic(&cells),
        atmospheric_validation: atmospheric(bed, i.reads),
        say_no: say_no(&ctx, i.reads),
        preconditions,
        cells,
        tcr,
        transports,
        transport_errors,
        reported_notes,
        files: Files::default(),
    }
}

/// R11.
fn atmospheric(bed: &BedFile, reads: &Reads) -> AtmosphericReport {
    let v = &bed.atmospheric_validation;
    let a = &reads.atmospheric;
    let mut out = AtmosphericReport {
        not_run: a.not_run.clone(),
        note: "reported, not gated; not like for like: upstream's values were made with seed 0, \
               dt 10 ms and trans_epsilon 6, ours with seeds 1 to 10, dt 1 ms and \
               trans_epsilon 7. Its source sits at the room's centre, where a symmetric mesh may \
               put an internal facet; a run that is not OK is itself the result"
            .into(),
        ..Default::default()
    };
    let missing: Result<super::read::SppsRead, String> = Err("no run".into());
    let runs: Vec<(u32, &Result<super::read::SppsRead, String>)> = v
        .seeds
        .iter()
        .map(|s| (*s, a.spps.get(s).unwrap_or(&missing)))
        .collect();
    if a.not_run.is_none() {
        out.runs = runs
            .iter()
            .map(|(s, r)| match r {
                Ok(read) => SeedRun {
                    seed: *s,
                    run: Some(read.info.clone()),
                    error: None,
                    t30: read.t30.clone(),
                },
                Err(e) => SeedRun {
                    seed: *s,
                    run: None,
                    error: Some(e.clone()),
                    t30: Vec::new(),
                },
            })
            .collect();
    }
    match &a.tcr {
        Some(Ok(t)) => out.tcr = Some(t.info.clone()),
        Some(Err(e)) => out.tcr_error = Some(e.clone()),
        None => {}
    }
    for (bi, &f) in v.bands_hz.iter().enumerate() {
        let mut sources: BTreeMap<String, usize> = BTreeMap::new();
        let mut values = Vec::new();
        let mut kuttruff = None;
        for (_, r) in &runs {
            let Ok(r) = r else { continue };
            let Some(b) = r.bands_hz.iter().position(|x| *x == f as i32) else {
                continue;
            };
            if kuttruff.is_none() {
                kuttruff = r.reference.bands.get(b).and_then(|x| x.kuttruff_s);
            }
            if let Some(t) = r.t30.first().and_then(|rec| rec.get(b)) {
                *sources.entry(t.source.clone()).or_default() += 1;
                if t.judged() {
                    values.push(t.t.expect("judged"));
                }
            }
        }
        let spps = (values.len() >= 2).then(|| Interval::of(&values));
        let mean = (!values.is_empty()).then(|| stats::mean(&values));
        let tcr = match &a.tcr {
            Some(Ok(t)) => t
                .bands
                .iter()
                .find(|b| b.freq_hz == f as i32)
                .map(|b| b.eyring_s),
            _ => None,
        };
        let rel = |ours: Option<f64>, theirs: f64| ours.map(|o| o / theirs - 1.0);
        out.bands.push(AtmosphericBand {
            freq_hz: f,
            seeds_judged: values.len(),
            sources,
            tcr_eyring_s: tcr,
            kuttruff_s: kuttruff,
            xlsx_spps_2018_s: v.spps_2018_rt30_s[bi],
            xlsx_spps_1_3_4_s: v.spps_1_3_4_rt30_s[bi],
            xlsx_tcr_eyring_s: v.tcr_eyring_s[bi],
            xlsx_md_octave_s: v.md_octave_rt30_s[bi],
            against_spps_2018: rel(mean, v.spps_2018_rt30_s[bi]),
            against_spps_1_3_4: rel(mean, v.spps_1_3_4_rt30_s[bi]),
            against_md_octave: rel(mean, v.md_octave_rt30_s[bi]),
            tcr_against_xlsx: rel(tcr, v.tcr_eyring_s[bi]),
            spps,
        });
    }
    out
}

/// N5 and N6.
fn say_no(ctx: &CellContext, reads: &Reads) -> SayNoReport {
    let bed = ctx.bed;
    let mut out = SayNoReport::default();
    let n5 = &bed.say_no.n5;
    out.n5.cell = n5.cell.clone();
    out.n5.seed = n5.seed;
    match (&reads.n5, bed.cell(&n5.cell)) {
        (Some(r), Some(cell)) => {
            match r {
                Ok(read) => out.n5.run = Some(read.info.clone()),
                Err(e) => out.n5.error = Some(e.clone()),
            }
            let c = check::evaluate_cell(ctx, cell, reads.spps.get(&cell.id), Some((n5.seed, r)));
            let mut by = Vec::new();
            for (name, holds) in [("E2", c.e2.holds), ("E3", c.e3.holds), ("E6", c.e6.holds)] {
                if !holds {
                    by.push(name.to_string());
                }
            }
            for (name, v) in [
                ("A", c.a.as_ref().map(|x| x.verdict)),
                ("B", c.b.as_ref().map(|x| x.verdict)),
                ("C", c.c.as_ref().map(|x| x.verdict)),
            ] {
                if v.is_some_and(|v| v != Verdict::Pass) {
                    by.push(name.to_string());
                }
            }
            // A cell that is not gated reads `reported`; judge by its checks.
            let passes = c.e2.holds
                && c.e3.holds
                && c.e6.holds
                && [
                    c.a.as_ref().map(|x| x.verdict),
                    c.b.as_ref().map(|x| x.verdict),
                    c.c.as_ref().map(|x| x.verdict),
                ]
                .iter()
                .all(|v| *v == Some(Verdict::Pass));
            out.n5.cell_verdict = Some(c.verdict);
            out.n5.failed_by = by;
            out.n5.failures = c.failures;
            out.n5.as_required = !passes && out.n5.error.is_none();
        }
        (None, _) => out.n5.error = Some("no run".into()),
        (_, None) => out.n5.error = Some(format!("no cell '{}'", n5.cell)),
    }
    match &reads.n6 {
        Some(Ok(r)) => {
            out.n6.run = Some(r.info.clone());
            out.n6.codes = r
                .reference
                .bands
                .iter()
                .map(|b| b.kuttruff_refused.clone())
                .collect();
            let mut problems = Vec::new();
            for band in &r.reference.bands {
                if !band.lambert_walls {
                    problems.push(format!("{} Hz: lambert_walls false", band.freq_hz));
                }
                if band.kuttruff_s.is_none() {
                    problems.push(format!(
                        "{} Hz: kuttruff_s refused {}",
                        band.freq_hz,
                        band.kuttruff_refused.as_deref().unwrap_or("?")
                    ));
                }
            }
            out.n6.e3_problems = problems;
            out.n6.as_required =
                !out.n6.codes.is_empty()
                    && out.n6.codes.iter().all(|c| {
                        c.as_deref() == Some(crate::params::codes::REFERENCE_NOT_APPLICABLE)
                    });
        }
        Some(Err(e)) => out.n6.error = Some(e.clone()),
        None => out.n6.error = Some("no run".into()),
    }
    out
}

/// One row of `summary.json`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SummaryRow {
    pub cell: String,
    pub check: String,
    pub gated: bool,
    pub value: Option<f64>,
    pub lo: Option<f64>,
    pub hi: Option<f64>,
    pub limit: Option<f64>,
    pub verdict: String,
}

/// `summary.json`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Summary {
    pub pass: bool,
    pub exploratory: bool,
    pub git_commit: Option<String>,
    pub solvers_code_sha256: BTreeMap<String, Option<String>>,
    pub report_sha256: String,
    pub wall_s: f64,
    pub files: u64,
    pub bytes: u64,
    pub failures: usize,
    pub rows: Vec<SummaryRow>,
}

/// `summary.json` of `report`, whose text has sha256 `report_sha256`.
pub fn summary(report: &Report, report_sha256: &str) -> Summary {
    let mut rows = Vec::new();
    let v = |x: Verdict| {
        serde_json::to_value(x)
            .ok()
            .and_then(|v| v.as_str().map(str::to_string))
            .unwrap_or_default()
    };
    for c in &report.cells {
        let pre = [("E2", &c.e2), ("E3", &c.e3), ("E6", &c.e6)];
        for (name, p) in pre {
            rows.push(SummaryRow {
                cell: c.id.clone(),
                check: name.into(),
                gated: c.gated,
                value: Some(p.problems.len() as f64),
                lo: None,
                hi: None,
                limit: Some(0.0),
                verdict: if p.holds { "pass" } else { "fail" }.into(),
            });
        }
        if let Some(a) = &c.a {
            for b in &a.bands {
                rows.push(SummaryRow {
                    cell: c.id.clone(),
                    check: format!("A {} Hz", b.freq_hz),
                    gated: c.gated,
                    value: b.interval.map(|i| i.mean),
                    lo: b.interval.map(|i| i.lo),
                    hi: b.interval.map(|i| i.hi),
                    limit: Some(b.limit),
                    verdict: v(b.verdict),
                });
            }
        }
        if let Some(b) = &c.b {
            rows.push(SummaryRow {
                cell: c.id.clone(),
                check: "B".into(),
                gated: c.gated,
                value: Some(b.spread),
                lo: None,
                hi: None,
                limit: Some(b.limit),
                verdict: v(b.verdict),
            });
        }
        if let Some(x) = &c.c {
            rows.push(SummaryRow {
                cell: c.id.clone(),
                check: format!("C ({} seeds)", x.seeds.len()),
                gated: c.gated,
                value: Some(x.interval.mean),
                lo: Some(x.interval.lo),
                hi: Some(x.interval.hi),
                limit: Some(x.limit),
                verdict: v(x.verdict),
            });
        }
        rows.push(SummaryRow {
            cell: c.id.clone(),
            check: "cell".into(),
            gated: c.gated,
            value: None,
            lo: None,
            hi: None,
            limit: None,
            verdict: v(c.verdict),
        });
    }
    for t in &report.tcr {
        if let Some(d) = &t.d {
            for b in &d.bands {
                rows.push(SummaryRow {
                    cell: t.id.clone(),
                    check: format!("D {} Hz", b.freq_hz),
                    gated: t.gated,
                    value: b.deviation,
                    lo: None,
                    hi: None,
                    limit: Some(d.limit),
                    verdict: v(b.verdict),
                });
            }
        }
    }
    let p = &report.preconditions;
    for (name, holds) in [
        ("E1", p.e1.holds),
        ("E2", p.e2.holds),
        ("E3", p.e3.holds),
        ("E4", p.e4.holds),
        ("E5", p.e5.holds),
        ("E6", p.e6.holds),
        ("E7", p.e7.holds),
    ] {
        rows.push(SummaryRow {
            cell: "bed".into(),
            check: name.into(),
            gated: true,
            value: None,
            lo: None,
            hi: None,
            limit: None,
            verdict: if holds { "pass" } else { "fail" }.into(),
        });
    }
    Summary {
        pass: report.pass,
        exploratory: report.exploratory,
        git_commit: report.meta.git_commit.clone(),
        solvers_code_sha256: p
            .e1
            .solvers
            .iter()
            .map(|s| (s.name.clone(), s.code_sha256.clone()))
            .collect(),
        report_sha256: report_sha256.to_string(),
        wall_s: report.meta.wall_s,
        files: report.files.files,
        bytes: report.files.bytes,
        failures: report.failures.len(),
        rows,
    }
}
