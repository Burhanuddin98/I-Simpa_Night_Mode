//! The output-line classifier: `docs/solver-contract.md` Part B, "Output line classification".
//!
//! [`LINE_RULES`] is that page's table, row for row; `tests/run_contract_docs.rs` parses the page
//! and fails when the two differ in any row. A line is classified by its content alone, never by
//! the stream it arrived on: the first rule whose pattern matches wins (no two patterns match the
//! same line), and a line no pattern matches is `unclassified_line`, which is WARN. Every pattern
//! is anchored at the start of the line, and one that ends in `$` must match the whole line.
//!
//! # Continuation lines
//!
//! Two events print a file path on a line of its own: the line after `scene_mesh_unreadable`
//! (`coreinitialisation.cpp:396`) and the line before `tetra_mesh_empty` (`coreTypes.cpp:241`).
//! Those lines take the event's id and class with [`Classified::continuation`] set, never
//! `unclassified_line`. Adjacency is judged within one stream, because the order of lines read
//! from two pipes is not reliable; each event prints its path with the same `cout` statement.
//!
//! # Streaming
//!
//! The [`Classifier`] sees lines one at a time, in arrival order. To recognise the line *before*
//! `tetra_mesh_empty` it holds back a line that matches no pattern until the next line on the same
//! stream arrives, or until [`Classifier::finish`]: an unclassified line is therefore settled one
//! line late, and a live consumer sees it only then. A line that matches a pattern is never held.
//! Settled lines can thus leave in another order than they arrived; each carries its arrival index
//! ([`Classified::seq`]), by which the verdict orders what it records.

use std::sync::LazyLock;

use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::process::{Line, Stream};

/// A line's class (`docs/solver-contract.md` Part B).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum LineClass {
    /// Progress, with a percentage ([`Classified::progress`]).
    Progress,
    Info,
    /// Evidence of success: `spps_end_of_calculation`, required for an SPPS OK.
    Ok,
    /// Recorded in the verdict; the run can still be OK.
    Warn,
    /// The run is not OK; the row's id is the verdict's reason code.
    Fail,
}

impl LineClass {
    /// The class as the contract page spells it.
    pub fn as_str(self) -> &'static str {
        match self {
            LineClass::Progress => "PROGRESS",
            LineClass::Info => "INFO",
            LineClass::Ok => "OK",
            LineClass::Warn => "WARN",
            LineClass::Fail => "FAIL",
        }
    }
}

/// Which neighbouring line belongs to a rule's event.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Continuation {
    /// The next line on the same stream (`scene_mesh_unreadable`: the path follows).
    Next,
    /// The previous line on the same stream (`tetra_mesh_empty`: the file name comes first).
    Previous,
}

/// One row of the contract's classification table.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LineRule {
    /// The row's "Pattern id / reason": the line's id, and the verdict's reason code for a FAIL.
    pub id: &'static str,
    /// The stream the page documents, `None` for "either". Documentation only: classification
    /// never looks at the stream.
    pub stream: Option<Stream>,
    /// The regular expression, anchored with `^`. `None` for `unclassified_line`, which is what
    /// no pattern matches.
    pub pattern: Option<&'static str>,
    pub class: LineClass,
    /// The event's continuation line, if it has one.
    pub continuation: Option<Continuation>,
    /// The message ends without a newline, so it arrives as the final partial line of its stream.
    pub unterminated: bool,
    /// The page's receipt, its backticked spans joined with `; ` (empty for none).
    pub receipt: &'static str,
}

const fn rule(
    id: &'static str,
    stream: Option<Stream>,
    pattern: Option<&'static str>,
    class: LineClass,
    receipt: &'static str,
) -> LineRule {
    LineRule {
        id,
        stream,
        pattern,
        class,
        continuation: None,
        unterminated: false,
        receipt,
    }
}

const fn with_continuation(r: LineRule, c: Continuation) -> LineRule {
    LineRule {
        continuation: Some(c),
        ..r
    }
}

const fn unterminated(r: LineRule) -> LineRule {
    LineRule {
        unterminated: true,
        ..r
    }
}

const OUT: Option<Stream> = Some(Stream::Stdout);
const ERR: Option<Stream> = Some(Stream::Stderr);
use LineClass::{Fail, Info, Ok as OkClass, Progress, Warn};

/// The id of the row that matches what no pattern matches.
pub const UNCLASSIFIED: &str = "unclassified_line";
/// The id of the line an SPPS OK requires.
pub const END_OF_CALCULATION: &str = "spps_end_of_calculation";

/// `docs/solver-contract.md` Part B's classification table, in the page's order: 22 rows.
pub const LINE_RULES: [LineRule; 22] = [
    rule(
        "progress",
        OUT,
        Some(r"^#[0-9.eE+-]+$"),
        Progress,
        "progressionInfo.h:154-155",
    ),
    rule(
        "spps_banner",
        OUT,
        Some(r"^SPPS version \d+\.\d+\.\d+$"),
        Info,
        "sppsNantes.cpp:247",
    ),
    rule(
        "tcr_banner",
        OUT,
        Some(r"^Classical Theory of Reverberation version \d+\.\d+\.\d+$"),
        Info,
        "TC_CalculationCore.cpp:48",
    ),
    rule(
        "tcr_loading",
        OUT,
        Some(r"^XML configuration file (is currently loading\.\.\.|has been loaded\.)$"),
        Info,
        "main_tc.cpp:57, 61",
    ),
    rule(
        "tcr_config_echo",
        OUT,
        Some(r"^config\.xml$"),
        Info,
        "main_tc.cpp:58",
    ),
    rule(
        "tcr_step",
        OUT,
        Some(r"^Step [123]/3 : .+$"),
        Info,
        "TC_CalculationCore.cpp:54-80",
    ),
    rule(
        "ground_height",
        OUT,
        Some(
            r"^(Compute of z ground inside each tetra|Calculation of z ground inside each tetra has been done)\.$",
        ),
        Info,
        "coreinitialisation.cpp:139, 148",
    ),
    rule(
        "spps_output_start",
        OUT,
        Some(r"^Output results files\.$"),
        Info,
        "coreinitialisation.cpp:477",
    ),
    rule(
        END_OF_CALCULATION,
        OUT,
        Some(r"^End of calculation\.$"),
        OkClass,
        "sppsNantes.cpp:389",
    ),
    rule(
        "xml_property_missing",
        OUT,
        Some(r"^Xml Property (.+) doesn't exist !$"),
        Fail,
        "cxml.cpp:115",
    ),
    with_continuation(
        rule(
            "scene_mesh_unreadable",
            OUT,
            Some(r"^Unable to read the scene mesh file :$"),
            Fail,
            "coreinitialisation.cpp:396",
        ),
        Continuation::Next,
    ),
    rule(
        "tetra_mesh_unreadable",
        OUT,
        Some(
            r"^Unable to read the tetrahedalization of the scene mesh file, calculation canceled\.$",
        ),
        Fail,
        "coreinitialisation.cpp:456",
    ),
    with_continuation(
        rule(
            "tetra_mesh_empty",
            OUT,
            Some(r"^Tetrahedron file is empty, the calculation can't be done !$"),
            Fail,
            "coreTypes.cpp:241",
        ),
        Continuation::Previous,
    ),
    rule(
        "config_path_missing",
        OUT,
        Some(r"^The path of the XML configuration file must be specified!$"),
        Fail,
        "sppsNantes.cpp:287; main_tc.cpp:50",
    ),
    rule(
        "directivity_not_open",
        OUT,
        Some(r"^DirectivityBalloon : File not open$"),
        Fail,
        "directivityParser.cpp:46",
    ),
    rule(
        "source_moved_off_vertex",
        OUT,
        Some(r"^Source at tetrahedron vertex, move source position from \[.*\] to \[.*\]$"),
        Warn,
        "sppsInitialisation.cpp:27-29",
    ),
    rule(
        "material_missing",
        ERR,
        Some(
            r"^Wrong project configuration, a face is defined but no materials are attached to it !",
        ),
        Fail,
        "coreinitialisation.cpp:430",
    ),
    unterminated(rule(
        "degenerate_tetrahedron",
        ERR,
        Some(r"^Error in input mesh, a tetrahedra have at least the same two vertices"),
        Fail,
        "coreTypes.cpp:213",
    )),
    rule(
        "source_on_surface",
        ERR,
        Some(r"^A sound source position is intersecting with the 3D model"),
        Fail,
        "sppsNantes.cpp:323",
    ),
    unterminated(rule(
        "source_not_located",
        ERR,
        Some(r"^Unable to find the source position!"),
        Fail,
        "sppsNantes.cpp:63",
    )),
    unterminated(rule(
        "particle_loss_reported",
        ERR,
        Some(r"^Warning (\d+) particles has been in error on (\d+) particles\."),
        Fail,
        "sppsNantes.cpp:425-439",
    )),
    rule(UNCLASSIFIED, None, None, Warn, ""),
];

/// The compiled patterns, index for index with [`LINE_RULES`].
static PATTERNS: LazyLock<Vec<Option<Regex>>> = LazyLock::new(|| {
    LINE_RULES
        .iter()
        .map(|r| {
            r.pattern
                .map(|p| Regex::new(p).expect("LINE_RULES patterns compile"))
        })
        .collect()
});

/// The row with this id.
pub fn rule_by_id(id: &str) -> Option<&'static LineRule> {
    LINE_RULES.iter().find(|r| r.id == id)
}

fn unclassified_rule() -> &'static LineRule {
    &LINE_RULES[LINE_RULES.len() - 1]
}

/// The rule a line's text matches, by content alone; `unclassified_line` when none does.
pub fn match_text(text: &str) -> &'static LineRule {
    PATTERNS
        .iter()
        .zip(&LINE_RULES)
        .find(|(p, _)| p.as_ref().is_some_and(|p| p.is_match(text)))
        .map(|(_, r)| r)
        .unwrap_or_else(unclassified_rule)
}

/// Every rule whose pattern matches `text` (for checking that no two rows overlap).
pub fn matching_rules(text: &str) -> Vec<&'static str> {
    PATTERNS
        .iter()
        .zip(&LINE_RULES)
        .filter(|(p, _)| p.as_ref().is_some_and(|p| p.is_match(text)))
        .map(|(_, r)| r.id)
        .collect()
}

/// The percentage a `progress` line carries (`#12.5` is 12.5), when it parses to a finite number.
pub fn progress_value(text: &str) -> Option<f64> {
    text.strip_prefix('#')?
        .parse::<f64>()
        .ok()
        .filter(|v| v.is_finite())
}

/// A classified line.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Classified {
    pub line: Line,
    /// The row's id: the reason code of a FAIL line, `unclassified_line` for a line no pattern
    /// matches.
    pub id: &'static str,
    pub class: LineClass,
    /// The line is the file path printed with its event, which [`Classified::id`] names.
    pub continuation: bool,
    /// The percentage of a PROGRESS line.
    pub progress: Option<f64>,
    /// The line's arrival index: 0 for the first line pushed, counting both streams.
    pub seq: u64,
}

impl Classified {
    fn new(line: Line, seq: u64, rule: &'static LineRule, continuation: bool) -> Self {
        let progress = if rule.class == LineClass::Progress && !continuation {
            progress_value(&line.text)
        } else {
            None
        };
        Classified {
            line,
            id: rule.id,
            class: rule.class,
            continuation,
            progress,
            seq,
        }
    }
}

#[derive(Debug, Default)]
struct StreamState {
    /// The next line on this stream is the path after `scene_mesh_unreadable`.
    path_follows: bool,
    /// A line that matched nothing, with its arrival index, held until the next line shows
    /// whether it is the file name before `tetra_mesh_empty`.
    held: Option<(Line, u64)>,
}

/// Classifies lines as they arrive, holding the continuation-line state per stream.
#[derive(Debug, Default)]
pub struct Classifier {
    streams: [StreamState; 2],
    /// Lines pushed so far: the next line's [`Classified::seq`].
    pushed: u64,
}

fn slot(stream: Stream) -> usize {
    match stream {
        Stream::Stdout => 0,
        Stream::Stderr => 1,
    }
}

impl Classifier {
    pub fn new() -> Self {
        Self::default()
    }

    /// Takes the next line and returns the lines it settles, in order: none (the line is held),
    /// one, or two (a held line, then this one). A held line is an unclassified one, settled by
    /// the next line on its stream or by [`Classifier::finish`].
    pub fn push(&mut self, line: &Line) -> Vec<Classified> {
        let seq = self.pushed;
        self.pushed += 1;
        let state = &mut self.streams[slot(line.stream)];
        if state.path_follows {
            state.path_follows = false;
            // A held line was settled when the event line arrived, so there is none here.
            let event = rule_by_id("scene_mesh_unreadable").expect("row exists");
            return vec![Classified::new(line.clone(), seq, event, true)];
        }
        let rule = match_text(&line.text);
        let mut out = Vec::with_capacity(2);
        if rule.pattern.is_none() {
            if let Some((held, at)) = state.held.replace((line.clone(), seq)) {
                out.push(Classified::new(held, at, rule, false));
            }
            return out;
        }
        if let Some((held, at)) = state.held.take() {
            out.push(if rule.continuation == Some(Continuation::Previous) {
                Classified::new(held, at, rule, true)
            } else {
                Classified::new(held, at, unclassified_rule(), false)
            });
        }
        state.path_follows = rule.continuation == Some(Continuation::Next);
        out.push(Classified::new(line.clone(), seq, rule, false));
        out
    }

    /// Settles the held lines at the end of both streams, in arrival order.
    pub fn finish(&mut self) -> Vec<Classified> {
        let mut out: Vec<Classified> = self
            .streams
            .iter_mut()
            .filter_map(|s| {
                s.path_follows = false;
                s.held.take()
            })
            .map(|(held, at)| Classified::new(held, at, unclassified_rule(), false))
            .collect();
        out.sort_by_key(|c| c.seq);
        out
    }
}

/// Classifies a whole recorded run.
pub fn classify_all(lines: &[Line]) -> Vec<Classified> {
    let mut c = Classifier::new();
    let mut out: Vec<Classified> = lines.iter().flat_map(|l| c.push(l)).collect();
    out.extend(c.finish());
    out
}

/// Lines per class, as the run manifest records them. A continuation line counts in its event's
/// class, and `unclassified_line` in WARN.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClassCounts {
    pub progress: u64,
    pub info: u64,
    pub ok: u64,
    pub warn: u64,
    pub fail: u64,
    /// Of the WARN lines, those that matched no pattern.
    pub unclassified: u64,
}

impl ClassCounts {
    pub fn of(lines: &[Classified]) -> Self {
        let mut c = ClassCounts::default();
        for l in lines {
            match l.class {
                LineClass::Progress => c.progress += 1,
                LineClass::Info => c.info += 1,
                LineClass::Ok => c.ok += 1,
                LineClass::Warn => c.warn += 1,
                LineClass::Fail => c.fail += 1,
            }
            if l.id == UNCLASSIFIED {
                c.unclassified += 1;
            }
        }
        c
    }

    pub fn total(&self) -> u64 {
        self.progress + self.info + self.ok + self.warn + self.fail
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(stream: Stream, text: &str) -> Line {
        Line {
            stream,
            t_ms: 0.0,
            text: text.to_string(),
            terminated: true,
        }
    }

    #[test]
    fn every_pattern_is_anchored_and_compiles() {
        for (r, p) in LINE_RULES.iter().zip(PATTERNS.iter()) {
            match r.pattern {
                Some(src) => {
                    assert!(src.starts_with('^'), "{}", r.id);
                    assert!(p.is_some());
                }
                None => assert_eq!(r.id, UNCLASSIFIED),
            }
        }
        assert_eq!(LINE_RULES.last().unwrap().id, UNCLASSIFIED);
    }

    #[test]
    fn anchoring_refuses_a_prefix_or_a_suffix() {
        assert_eq!(match_text("End of calculation.").id, END_OF_CALCULATION);
        assert_eq!(match_text(" End of calculation.").id, UNCLASSIFIED);
        assert_eq!(match_text("End of calculation. ").id, UNCLASSIFIED);
        assert_eq!(match_text("End of calculation").id, UNCLASSIFIED);
        // Patterns without `$` accept a tail: the loss warning continues after its first sentence.
        assert_eq!(
            match_text("Warning 12 particles has been in error on 99 particles. The computation")
                .id,
            "particle_loss_reported"
        );
        assert_eq!(
            match_text("xWarning 12 particles has been in error on 99 particles.").id,
            UNCLASSIFIED
        );
    }

    #[test]
    fn progress_carries_its_value() {
        let c = classify_all(&[
            line(Stream::Stdout, "#12.5"),
            line(Stream::Stdout, "#1e+02"),
        ]);
        assert_eq!(c[0].progress, Some(12.5));
        assert_eq!(c[1].progress, Some(100.0));
        // Matches the pattern but is no number: PROGRESS without a value.
        assert_eq!(match_text("#-").id, "progress");
        assert_eq!(progress_value("#-"), None);
        assert_eq!(match_text("#12%").id, UNCLASSIFIED);
    }

    #[test]
    fn the_scene_mesh_path_follows_its_event_on_the_same_stream() {
        let lines = [
            line(Stream::Stdout, "Unable to read the scene mesh file :"),
            line(Stream::Stderr, "something on stderr"),
            line(Stream::Stdout, r"C:\run\mesh.cbin"),
            line(Stream::Stdout, r"C:\run\another"),
        ];
        let c = classify_all(&lines);
        let got: Vec<(&str, bool, &str)> = c
            .iter()
            .map(|c| (c.id, c.continuation, c.line.text.as_str()))
            .collect();
        assert_eq!(
            got,
            [
                (
                    "scene_mesh_unreadable",
                    false,
                    "Unable to read the scene mesh file :"
                ),
                ("scene_mesh_unreadable", true, r"C:\run\mesh.cbin"),
                // Both held to the end; finish() settles them in arrival order.
                (UNCLASSIFIED, false, "something on stderr"),
                (UNCLASSIFIED, false, r"C:\run\another"),
            ]
        );
        let seq: Vec<u64> = c.iter().map(|c| c.seq).collect();
        assert_eq!(seq, [0, 2, 1, 3]);
    }

    #[test]
    fn the_tetra_mesh_file_name_precedes_its_event() {
        let mut c = Classifier::new();
        assert!(
            c.push(&line(Stream::Stdout, r"C:\run\tetramesh.mbin"))
                .is_empty()
        );
        let out = c.push(&line(
            Stream::Stdout,
            "Tetrahedron file is empty, the calculation can't be done !",
        ));
        assert_eq!(out.len(), 2);
        assert_eq!(
            (out[0].id, out[0].continuation, out[0].class),
            ("tetra_mesh_empty", true, LineClass::Fail)
        );
        assert_eq!(
            (out[1].id, out[1].continuation),
            ("tetra_mesh_empty", false)
        );
        assert!(c.finish().is_empty());
    }

    #[test]
    fn a_held_line_before_anything_else_is_unclassified() {
        let mut c = Classifier::new();
        assert!(c.push(&line(Stream::Stdout, "garbage")).is_empty());
        // A line on the other stream does not settle it.
        let out = c.push(&line(
            Stream::Stderr,
            "Tetrahedron file is empty, the calculation can't be done !",
        ));
        assert_eq!(out.len(), 1);
        assert!(!out[0].continuation);
        let out = c.push(&line(Stream::Stdout, "#50"));
        assert_eq!(
            out.iter().map(|c| c.id).collect::<Vec<_>>(),
            [UNCLASSIFIED, "progress"]
        );
        assert!(c.push(&line(Stream::Stdout, "tail")).is_empty());
        let out = c.finish();
        assert_eq!(out.len(), 1);
        assert_eq!(
            (out[0].id, out[0].line.text.as_str()),
            (UNCLASSIFIED, "tail")
        );
    }

    #[test]
    fn counts_by_class() {
        let c = classify_all(&[
            line(Stream::Stdout, "#1"),
            line(Stream::Stdout, "SPPS version 2.2.1"),
            line(Stream::Stdout, "End of calculation."),
            line(Stream::Stdout, "who knows"),
            line(
                Stream::Stderr,
                "A sound source position is intersecting with the 3D model",
            ),
        ]);
        let n = ClassCounts::of(&c);
        assert_eq!(
            n,
            ClassCounts {
                progress: 1,
                info: 1,
                ok: 1,
                warn: 1,
                fail: 1,
                unclassified: 1
            }
        );
        assert_eq!(n.total(), 5);
    }
}
