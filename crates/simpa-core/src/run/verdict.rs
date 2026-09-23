//! The run verdict: `docs/solver-contract.md` Part B, "Exit codes" and "Judging a run", with
//! decisions 9 and 10 of `docs/m5-m6-design.md`.
//!
//! A run is OK only when all four signals agree:
//! 1. **Exit.** Cancelled is `cancelled`. An exit at or above `0xC0000000` is CRASH
//!    (`crash_access_violation`, `crash_abort`, `crash_other`), any other non-zero exit is
//!    `exit_nonzero`. Exit 0 is necessary, never sufficient: SPPS also needs its
//!    `spps_end_of_calculation` line, or the reason is `end_of_calculation_missing`.
//! 2. **Lines.** No FAIL line (the reason is the row's id, once per id); every WARN line is
//!    recorded in [`Verdict::warnings`], in arrival order.
//! 3. **Statistics** (SPPS): `stats_unreadable`, `stats_band_mismatch`, `particle_total_short`,
//!    `particle_loss_excess`.
//! 4. **Files.** `expected_file_missing`; for TCR also `nonfinite_result` for any displayed value
//!    that is NaN or ±inf (the `.gabe` tables' band rows, every `.csbin` value), and
//!    `result_unreadable` for a result file that does not decode.
//!
//! **`nonfinite_result` has one exclusion,** the `Global` row of a `.gabe` table (see
//! `table_reasons`). In particular `-inf` in a point receiver's `Direct` column fails the run:
//! it is the level of zero direct energy (`TC_CalculationCore.cpp:160-184`: a source hidden by
//! the scene adds nothing, then `10*log10f(0)`), and a source outside the room (P2
//! `tcr_src_out`, a contract-listed silent failure) produces exactly that, with nothing else to
//! tell it apart. A receiver hidden from every source in an otherwise valid room fails the same
//! way; the reason's detail names both causes.
//!
//! Signals 3 and 4 are judged only after exit 0 without cancel: after a crash, a non-zero exit or
//! a cancel the outputs are partial by construction, and listing them would bury the cause. The
//! FAIL lines are reported whatever the exit.
//!
//! Three points the contract states explicitly (Part B, "Exit codes" and "Judging a run"):
//! - `0xFFFFFFFF` (-1) is `exit_nonzero`, not CRASH, although it lies in the NTSTATUS range: it
//!   is the solvers' own `return -1` on an undeclared material, not an exception.
//! - Every code of [`codes`] but one is a row of Part B's "Reason codes" table, in [`codes::ALL`]'s
//!   order (`tests/run_contract_docs.rs`).
//! - An unreadable `config.xml` is `config_attribute_missing`, the Part A code `validate_export`
//!   gives the same fault (`validate/export.rs:403-441`): without it no output can be expected.

use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::Path;

use serde::{Deserialize, Serialize};

use super::classify::{Classified, Continuation, END_OF_CALCULATION, LineClass, rule_by_id};
use super::expect::{ExpectError, Expectation, normalize};
use super::stats::{self, ParticleStats, StatsError};
use crate::formats::csbin::{self, Csbin};
use crate::formats::gabe::{self, Gabe};
use crate::process::Outcome;
use crate::schema::SolverKind;

/// Decision 9's proposed run limit: a band fails when (lost by loops + lost by meshing) / total
/// exceeds 1 %. Open decision 7 is still open, so every manifest records the limit it used.
pub const DEFAULT_LOSS_LIMIT: f64 = 0.01;

/// The verdict's own reason codes. A FAIL line's reason is its row id
/// ([`LINE_RULES`](super::classify::LINE_RULES)).
pub mod codes {
    /// The run manager refused the project's geometry (`geometry::check`); its own codes are in
    /// the reason's detail.
    pub const GEOMETRY_REFUSED: &str = "geometry_refused";
    /// `run --mesh <dir>`: the folder has no `OK` mesh manifest or no `tetramesh.mbin`, or an
    /// in-run mesh folder could not be used at all.
    pub const MESH_MISSING: &str = "mesh_missing";
    /// The run folder's inputs could not be written: `config_xml`'s writer refused the project
    /// or variant (its code is in the detail), or a file could not be staged.
    pub const EXPORT_FAILED: &str = "export_failed";
    /// The solver could not be started, or its process tree could not be ended.
    pub const LAUNCH_FAILED: &str = "launch_failed";
    pub const CANCELLED: &str = "cancelled";
    pub const CRASH_ACCESS_VIOLATION: &str = "crash_access_violation";
    pub const CRASH_ABORT: &str = "crash_abort";
    pub const CRASH_OTHER: &str = "crash_other";
    pub const EXIT_NONZERO: &str = "exit_nonzero";
    pub const END_OF_CALCULATION_MISSING: &str = "end_of_calculation_missing";
    pub const CONFIG_ATTRIBUTE_MISSING: &str = crate::validate::codes::CONFIG_ATTRIBUTE_MISSING;
    pub const STATS_UNREADABLE: &str = "stats_unreadable";
    pub const STATS_BAND_MISMATCH: &str = "stats_band_mismatch";
    pub const PARTICLE_TOTAL_SHORT: &str = "particle_total_short";
    pub const PARTICLE_LOSS_EXCESS: &str = "particle_loss_excess";
    pub const EXPECTED_FILE_MISSING: &str = "expected_file_missing";
    pub const NONFINITE_RESULT: &str = "nonfinite_result";
    pub const RESULT_UNREADABLE: &str = "result_unreadable";

    /// Every code above, in the order a verdict lists them: the run manager's refusals before
    /// launch, then the four signals.
    pub const ALL: [&str; 18] = [
        GEOMETRY_REFUSED,
        MESH_MISSING,
        EXPORT_FAILED,
        LAUNCH_FAILED,
        CANCELLED,
        CRASH_ACCESS_VIOLATION,
        CRASH_ABORT,
        CRASH_OTHER,
        EXIT_NONZERO,
        END_OF_CALCULATION_MISSING,
        CONFIG_ATTRIBUTE_MISSING,
        STATS_UNREADABLE,
        STATS_BAND_MISMATCH,
        PARTICLE_TOTAL_SHORT,
        PARTICLE_LOSS_EXCESS,
        EXPECTED_FILE_MISSING,
        NONFINITE_RESULT,
        RESULT_UNREADABLE,
    ];
}
use codes::*;

/// `STATUS_ACCESS_VIOLATION`.
const ACCESS_VIOLATION: u32 = 0xC000_0005;
/// `STATUS_STACK_BUFFER_OVERRUN`: `abort()` after an uncaught C++ exception.
const FAST_FAIL: u32 = 0xC000_0409;
/// The first NTSTATUS error code.
const CRASH_FLOOR: u32 = 0xC000_0000;

/// How a run ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum Status {
    Ok,
    Fail,
    Crash,
    Cancelled,
}

/// One reason, or one recorded WARN line.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Reason {
    pub code: String,
    pub detail: String,
}

impl Reason {
    /// A reason with `code` and its detail in words.
    pub fn new(code: &str, detail: impl Into<String>) -> Self {
        Reason {
            code: code.to_string(),
            detail: detail.into(),
        }
    }
}

/// The judgement of one run.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Verdict {
    pub status: Status,
    /// Why the run is not OK: at most one entry per code, in signal order. Empty exactly when
    /// the status is OK.
    pub reasons: Vec<Reason>,
    /// Every WARN line, in arrival order ([`Classified::seq`]): its row id and its text.
    pub warnings: Vec<Reason>,
}

impl Verdict {
    pub fn is_ok(&self) -> bool {
        self.status == Status::Ok
    }

    /// The reason codes, in order.
    pub fn codes(&self) -> Vec<&str> {
        self.reasons.iter().map(|r| r.code.as_str()).collect()
    }
}

/// A `.csbin`'s stored values, reduced to what [`judge`] checks.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SurfaceValues {
    /// Values stored, over every receiver, face and recorded time step.
    pub values: usize,
    /// Of those, the NaN and ±inf ones.
    pub nonfinite: usize,
    /// Where the first non-finite value is: receiver, face, time step and value.
    pub first_nonfinite: Option<String>,
}

impl SurfaceValues {
    /// Counts every value of `c` and locates the first that is not finite.
    pub fn of(c: &Csbin) -> Self {
        let mut s = SurfaceValues::default();
        for r in &c.receivers {
            for (i, f) in r.faces.iter().enumerate() {
                for rec in f.records.iter() {
                    s.values += 1;
                    if !rec.energy.is_finite() {
                        s.nonfinite += 1;
                        s.first_nonfinite.get_or_insert_with(|| {
                            format!(
                                "receiver '{}' face {i} time step {} is {}",
                                r.name_lossy(),
                                rec.time_step,
                                rec.energy
                            )
                        });
                    }
                }
            }
        }
        s
    }
}

/// What a finished run left in its working directory, read once after the solver exits.
#[derive(Debug, Default)]
pub struct Outputs {
    /// Every file under the folder, keyed by its [`normalize`]d relative path, with its size.
    pub files: BTreeMap<String, u64>,
    /// SPPS: the statistics table, or why it could not be read. `None` when not read.
    pub stats: Option<Result<ParticleStats, StatsError>>,
    /// TCR: each existing [`Expectation::result_tables`] entry, decoded, or why not.
    pub tables: BTreeMap<String, Result<Gabe, String>>,
    /// TCR: each existing [`Expectation::surface_tables`] entry, decoded and reduced to its
    /// values, or why it could not be decoded.
    pub surfaces: BTreeMap<String, Result<SurfaceValues, String>>,
}

fn list_files(root: &Path, dir: &Path, out: &mut BTreeMap<String, u64>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for e in entries.filter_map(Result::ok) {
        let Ok(kind) = e.file_type() else { continue };
        let path = e.path();
        if kind.is_dir() {
            list_files(root, &path, out);
        } else if kind.is_file() {
            let size = e.metadata().map(|m| m.len()).unwrap_or(0);
            if let Ok(rel) = path.strip_prefix(root) {
                out.insert(normalize(&rel.to_string_lossy()), size);
            }
        }
    }
}

impl Outputs {
    /// Lists `dir` (the solver's working directory) and reads what the expectation says to
    /// check: SPPS's statistics table, TCR's result tables and surface-receiver files.
    pub fn read(dir: &Path, exp: &Expectation) -> Outputs {
        let mut files = BTreeMap::new();
        list_files(dir, dir, &mut files);
        let stats = (exp.solver == SolverKind::Spps)
            .then(|| stats::read_file(&dir.join(&exp.names.stats_filename)));
        let tables = exp
            .result_tables()
            .into_iter()
            .filter(|t| files.contains_key(t))
            .map(|t| {
                let g = gabe::read_file(&dir.join(&t)).map_err(|e| e.to_string());
                (t, g)
            })
            .collect();
        let surfaces = exp
            .surface_tables()
            .into_iter()
            .filter(|t| files.contains_key(t))
            .map(|t| {
                let c = csbin::read_file(&dir.join(&t))
                    .map(|c| SurfaceValues::of(&c))
                    .map_err(|e| e.to_string());
                (t, c)
            })
            .collect();
        Outputs {
            files,
            stats,
            tables,
            surfaces,
        }
    }
}

/// Everything [`judge`] looks at.
#[derive(Debug)]
pub struct Evidence<'a> {
    pub solver: SolverKind,
    pub outcome: &'a Outcome,
    /// Every classified line, in the order the classifier settled them (not always arrival
    /// order: see [`Classified::seq`]).
    pub lines: &'a [Classified],
    /// What the run folder's `config.xml` asks for, or why it could not be read.
    pub expectation: Result<&'a Expectation, &'a ExpectError>,
    pub outputs: &'a Outputs,
    /// The largest (lost by loops + lost by meshing) / total a band may have. Compared so that
    /// a NaN limit fails every band with particles.
    pub loss_limit: f64,
}

/// At most `n` items, then how many more.
fn first_of<T: std::fmt::Display>(items: &[T], n: usize) -> String {
    let mut s = String::new();
    for (i, x) in items.iter().take(n).enumerate() {
        if i > 0 {
            s.push_str("; ");
        }
        let _ = write!(s, "{x}");
    }
    if items.len() > n {
        let _ = write!(s, "; and {} more", items.len() - n);
    }
    s
}

/// The exit signal: the status it forces, if any, and its reason.
fn exit_reason(outcome: &Outcome) -> Option<(Status, Reason)> {
    if outcome.cancelled {
        return Some((
            Status::Cancelled,
            Reason::new(CANCELLED, "the run was cancelled; its outputs are partial"),
        ));
    }
    let Some(code) = outcome.exit_code else {
        return Some((
            Status::Crash,
            Reason::new(CRASH_OTHER, "the solver ended without an exit code"),
        ));
    };
    let hex = format!("exit 0x{code:08X}");
    match code {
        0 => None,
        ACCESS_VIOLATION => Some((
            Status::Crash,
            Reason::new(CRASH_ACCESS_VIOLATION, format!("{hex}: access violation")),
        )),
        FAST_FAIL => Some((
            Status::Crash,
            Reason::new(
                CRASH_ABORT,
                format!("{hex}: abort, typically an uncaught C++ exception"),
            ),
        )),
        u32::MAX => Some((
            Status::Fail,
            Reason::new(EXIT_NONZERO, format!("{hex} (-1): the solver returned -1")),
        )),
        c if c >= CRASH_FLOOR => Some((
            Status::Crash,
            Reason::new(CRASH_OTHER, format!("{hex}: an NTSTATUS error")),
        )),
        c => Some((
            Status::Fail,
            Reason::new(EXIT_NONZERO, format!("exit {c} (0x{c:08X})")),
        )),
    }
}

/// One reason per FAIL id, in order of first appearance: the first event's text with its
/// continuation line, and the number of events.
fn fail_line_reasons(lines: &[Classified]) -> Vec<Reason> {
    let mut order: Vec<&str> = Vec::new();
    let mut first: BTreeMap<&str, String> = BTreeMap::new();
    let mut count: BTreeMap<&str, usize> = BTreeMap::new();
    let mut path: BTreeMap<&str, String> = BTreeMap::new();
    for l in lines.iter().filter(|l| l.class == LineClass::Fail) {
        if l.continuation {
            path.entry(l.id).or_insert_with(|| l.line.text.clone());
            continue;
        }
        if !first.contains_key(l.id) {
            order.push(l.id);
            first.insert(l.id, l.line.text.clone());
        }
        *count.entry(l.id).or_default() += 1;
    }
    order
        .into_iter()
        .map(|id| {
            let text = &first[id];
            let mut detail = match (path.get(id), rule_by_id(id).and_then(|r| r.continuation)) {
                (Some(p), Some(Continuation::Previous)) => format!("{p}: {text}"),
                (Some(p), _) => format!("{text} {p}"),
                (None, _) => text.clone(),
            };
            if count[id] > 1 {
                let _ = write!(detail, " ({} lines)", count[id]);
            }
            Reason::new(id, detail)
        })
        .collect()
}

fn stats_reasons(
    exp: &Expectation,
    stats: Option<&Result<ParticleStats, StatsError>>,
    loss_limit: f64,
    out: &mut Vec<Reason>,
) {
    let st = match stats {
        None => {
            out.push(Reason::new(
                STATS_UNREADABLE,
                format!("{} was not read", exp.names.stats_filename),
            ));
            return;
        }
        Some(Err(e)) => {
            out.push(Reason::new(
                STATS_UNREADABLE,
                format!("{}: {e}", exp.names.stats_filename),
            ));
            return;
        }
        Some(Ok(st)) => st,
    };

    let want = exp.requested_bands();
    let got = st.freqs();
    let mut sorted = got.clone();
    sorted.sort_unstable();
    let duplicated = sorted.windows(2).any(|w| w[0] == w[1]);
    if duplicated || sorted != want {
        out.push(Reason::new(
            STATS_BAND_MISMATCH,
            format!("the statistics have band columns {got:?}; config.xml asks for {want:?}"),
        ));
    }

    let spps = exp.spps.as_ref();
    let per_source = u64::from(spps.map_or(1, |s| s.nbparticules));
    let sources = exp.sources.len() as u64;
    let need = per_source * sources;
    let short: Vec<String> = st
        .bands
        .iter()
        .filter(|b| u64::from(b.total) < need)
        .map(|b| format!("{} Hz: {}", b.freq_hz, b.total))
        .collect();
    if !short.is_empty() {
        out.push(Reason::new(
            PARTICLE_TOTAL_SHORT,
            format!(
                "{} band(s) ran fewer than nbparticules {per_source} x {sources} source(s) = \
                 {need} particles: {}",
                short.len(),
                first_of(&short, 6)
            ),
        ));
    }

    let excess: Vec<String> = st
        .bands
        .iter()
        .filter_map(|b| {
            let r = b.loss_ratio()?;
            // Within the limit only when comparable and not above it: a NaN limit fails the band.
            let within = matches!(
                r.partial_cmp(&loss_limit),
                Some(Ordering::Less | Ordering::Equal)
            );
            (!within).then(|| {
                format!(
                    "{} Hz: {} of {} ({:.4} %)",
                    b.freq_hz,
                    b.lost(),
                    b.total,
                    r * 100.0
                )
            })
        })
        .collect();
    if !excess.is_empty() {
        out.push(Reason::new(
            PARTICLE_LOSS_EXCESS,
            format!(
                "{} band(s) lost more than {} % of their particles to loops and meshing: {}",
                excess.len(),
                loss_limit * 100.0,
                first_of(&excess, 6)
            ),
        ));
    }
}

fn file_reasons(exp: &Expectation, outputs: &Outputs, out: &mut Vec<Reason>) {
    let expected = exp.expected_files();
    let missing: Vec<String> = expected
        .iter()
        .filter_map(|p| match outputs.files.get(p) {
            None => Some(format!("{p} (missing)")),
            Some(0) => Some(format!("{p} (empty)")),
            Some(_) => None,
        })
        .collect();
    if !missing.is_empty() {
        out.push(Reason::new(
            EXPECTED_FILE_MISSING,
            format!(
                "{} of {} expected files: {}",
                missing.len(),
                expected.len(),
                first_of(&missing, 5)
            ),
        ));
    }
}

/// The point-receiver column TCR writes the direct field in (`main_tc.cpp:131`).
const DIRECT_COLUMN: &[u8] = b"Direct";

/// What `-inf` in a `Direct` column means, added once to the reason's detail: the level of zero
/// direct energy (`TC_CalculationCore.cpp:160-184`).
const NO_DIRECT_SOUND: &str = "; -inf in Direct means no direct sound from any source: a source \
                               outside the room, or a receiver every source is hidden from";

/// A result file that exists and is not empty (otherwise `expected_file_missing` covers it).
fn present(outputs: &Outputs, path: &str) -> bool {
    outputs.files.get(path).is_some_and(|&n| n > 0)
}

/// TCR: every float value in a requested band's row of each `.gabe` result table must be finite,
/// and so must every value of each surface-receiver `.csbin`. The `Global` row is left out: it is
/// NaN by design in the non-energetic columns (`ctr/reportmanager.cpp:206-209`), and elsewhere it
/// is derived from the band rows, which include uninitialised values for a band that is not
/// computed (`ctr/tcTypes.h:38-48`). The `.csbin` files have no such exception: their values are
/// linear energies, zeroed before any band is computed (`coreTypes.cpp:91-99`; `rsbin.h:113`).
fn table_reasons(exp: &Expectation, outputs: &Outputs, out: &mut Vec<Reason>) {
    let bands = exp.requested_bands();
    let mut unreadable: Vec<String> = Vec::new();
    let mut nonfinite: Vec<String> = Vec::new();
    let mut nonfinite_values = 0usize;
    let mut no_direct_sound = false;
    for t in exp.result_tables() {
        if !present(outputs, &t) {
            continue;
        }
        let g = match outputs.tables.get(&t) {
            None => {
                unreadable.push(format!("{t}: not decoded"));
                continue;
            }
            Some(Err(e)) => {
                unreadable.push(format!("{t}: {e}"));
                continue;
            }
            Some(Ok(g)) => g,
        };
        if g.row_labels().is_none() {
            unreadable.push(format!("{t}: no row labels"));
            continue;
        }
        for f in &bands {
            let label = format!("{f} Hz");
            let Some(row) = g.row_index(label.as_bytes()) else {
                unreadable.push(format!("{t}: no row for {label}"));
                continue;
            };
            for col in &g.columns[1..] {
                if let Some(&v) = col.floats().and_then(|v| v.get(row))
                    && !v.is_finite()
                {
                    nonfinite_values += 1;
                    no_direct_sound |= v == f32::NEG_INFINITY && col.name() == DIRECT_COLUMN;
                    nonfinite.push(format!(
                        "{t}: {} at {label} is {v}",
                        String::from_utf8_lossy(col.name())
                    ));
                }
            }
        }
    }
    for t in exp.surface_tables() {
        if !present(outputs, &t) {
            continue;
        }
        match outputs.surfaces.get(&t) {
            None => unreadable.push(format!("{t}: not decoded")),
            Some(Err(e)) => unreadable.push(format!("{t}: {e}")),
            Some(Ok(s)) => {
                if let Some(first) = &s.first_nonfinite {
                    nonfinite_values += s.nonfinite;
                    nonfinite.push(format!(
                        "{t}: {} of {} values, first {first}",
                        s.nonfinite, s.values
                    ));
                }
            }
        }
    }
    if !nonfinite.is_empty() {
        let mut detail = format!(
            "{nonfinite_values} displayed value(s) are not finite: {}",
            first_of(&nonfinite, 5)
        );
        if no_direct_sound {
            detail.push_str(NO_DIRECT_SOUND);
        }
        out.push(Reason::new(NONFINITE_RESULT, detail));
    }
    if !unreadable.is_empty() {
        out.push(Reason::new(
            RESULT_UNREADABLE,
            format!(
                "{} result table(s) cannot be checked: {}",
                unreadable.len(),
                first_of(&unreadable, 5)
            ),
        ));
    }
}

/// Judges a run from its evidence.
pub fn judge(ev: &Evidence) -> Verdict {
    let mut warned: Vec<&Classified> = ev
        .lines
        .iter()
        .filter(|l| l.class == LineClass::Warn)
        .collect();
    // The classifier settles an unclassified line one line late; record in arrival order.
    warned.sort_by_key(|l| l.seq);
    let warnings: Vec<Reason> = warned
        .into_iter()
        .map(|l| Reason::new(l.id, l.line.text.clone()))
        .collect();

    let mut reasons = Vec::new();
    let exit = exit_reason(ev.outcome);
    let forced = exit.as_ref().map(|(s, _)| *s);
    reasons.extend(exit.map(|(_, r)| r));
    let clean_exit = forced.is_none();

    if clean_exit
        && ev.solver == SolverKind::Spps
        && !ev
            .lines
            .iter()
            .any(|l| l.id == END_OF_CALCULATION && !l.continuation)
    {
        reasons.push(Reason::new(
            END_OF_CALCULATION_MISSING,
            "SPPS exited 0 without printing 'End of calculation.': exit 0 is no evidence of \
             success",
        ));
    }
    reasons.extend(fail_line_reasons(ev.lines));

    if clean_exit {
        match ev.expectation {
            Err(e) => reasons.push(Reason::new(
                CONFIG_ATTRIBUTE_MISSING,
                format!("{e}: no output can be expected"),
            )),
            Ok(exp) => {
                if ev.solver == SolverKind::Spps {
                    stats_reasons(exp, ev.outputs.stats.as_ref(), ev.loss_limit, &mut reasons);
                }
                file_reasons(exp, ev.outputs, &mut reasons);
                if ev.solver == SolverKind::Tcr {
                    table_reasons(exp, ev.outputs, &mut reasons);
                }
            }
        }
    }

    let status = match forced {
        Some(s) => s,
        None if reasons.is_empty() => Status::Ok,
        None => Status::Fail,
    };
    Verdict {
        status,
        reasons,
        warnings,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn outcome(exit_code: Option<u32>, cancelled: bool) -> Outcome {
        Outcome {
            exit_code,
            cancelled,
            elapsed_ms: 1.0,
        }
    }

    #[test]
    fn exit_codes_map_to_status() {
        let cases = [
            (Some(0), false, None),
            (Some(0), true, Some((Status::Cancelled, CANCELLED))),
            (None, true, Some((Status::Cancelled, CANCELLED))),
            (None, false, Some((Status::Crash, CRASH_OTHER))),
            (Some(1), false, Some((Status::Fail, EXIT_NONZERO))),
            (Some(u32::MAX), false, Some((Status::Fail, EXIT_NONZERO))),
            (
                Some(0xC000_0005),
                false,
                Some((Status::Crash, CRASH_ACCESS_VIOLATION)),
            ),
            (Some(0xC000_0409), false, Some((Status::Crash, CRASH_ABORT))),
            (Some(0xC000_0000), false, Some((Status::Crash, CRASH_OTHER))),
            (Some(0xE06D_7363), false, Some((Status::Crash, CRASH_OTHER))),
            (Some(0xBFFF_FFFF), false, Some((Status::Fail, EXIT_NONZERO))),
            (Some(0xFFFF_FFFE), false, Some((Status::Crash, CRASH_OTHER))),
        ];
        for (code, cancelled, want) in cases {
            let got = exit_reason(&outcome(code, cancelled)).map(|(s, r)| (s, r.code));
            assert_eq!(
                got.as_ref().map(|(s, c)| (*s, c.as_str())),
                want,
                "{code:?} {cancelled}"
            );
        }
    }

    #[test]
    fn first_of_counts_the_rest() {
        assert_eq!(first_of(&[1, 2, 3], 2), "1; 2; and 1 more");
        assert_eq!(first_of(&[1], 2), "1");
    }
}
