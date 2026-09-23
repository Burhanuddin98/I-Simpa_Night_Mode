//! The run module's tables against `docs/solver-contract.md` Part B: the classification table,
//! row for row, and the verdict's reason codes.

use std::path::PathBuf;

use simpa_core::process::Stream;
use simpa_core::run::classify::{self, Continuation, LINE_RULES, LineClass};
use simpa_core::run::verdict::codes;
use simpa_core::validate::RULES;

fn read(rel: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(rel);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

/// The lines between the heading that starts with `start` and the next heading of that level.
fn section<'a>(text: &'a str, start: &str) -> Vec<&'a str> {
    let level = start.split(' ').next().unwrap();
    let mut lines = text.lines().skip_while(|l| !l.starts_with(start));
    let first = lines.next().expect("section heading");
    assert!(first.starts_with(start));
    lines
        .take_while(|l| !(l.starts_with(level) && l[level.len()..].starts_with(' ')))
        .collect()
}

/// The cells of a Markdown table row, split on `|` but not on `\|`.
fn cells(row: &str) -> Vec<String> {
    let inner = row.trim().trim_start_matches('|').trim_end_matches('|');
    let mut out = vec![String::new()];
    let mut escaped = false;
    for c in inner.chars() {
        match c {
            '\\' if !escaped => escaped = true,
            '|' if !escaped => out.push(String::new()),
            _ => {
                if escaped && c != '|' {
                    out.last_mut().unwrap().push('\\');
                }
                out.last_mut().unwrap().push(c);
                escaped = false;
            }
        }
    }
    out.iter().map(|c| c.trim().to_string()).collect()
}

/// The spans inside backticks.
fn backticked(cell: &str) -> Vec<&str> {
    cell.split('`').skip(1).step_by(2).collect()
}

/// One row, as the test compares it: id, stream, pattern, class, continuation, no newline,
/// receipt.
type Row = (
    String,
    Option<Stream>,
    Option<String>,
    LineClass,
    Option<Continuation>,
    bool,
    String,
);

const HEADER: &str = "| Pattern id / reason | Stream | Pattern | Class | Receipt |";

/// One body row of the table, or why it does not read as one.
fn row(line: &str) -> Result<Row, String> {
    let c = cells(line);
    if c.len() != 5 {
        return Err(format!("{} cells, not 5: {line}", c.len()));
    }
    // The id cell is exactly one backticked identifier.
    let id = match backticked(&c[0])[..] {
        [id] if c[0] == format!("`{id}`") && !id.is_empty() => id.to_string(),
        _ => {
            return Err(format!(
                "id cell {:?} is not one backticked id: {line}",
                c[0]
            ));
        }
    };
    let stream = match c[1].as_str() {
        "stdout" => Some(Stream::Stdout),
        "stderr" => Some(Stream::Stderr),
        "either" => None,
        other => return Err(format!("stream {other:?}: {line}")),
    };
    let class = match c[3].split([':', ' ']).next().unwrap() {
        "PROGRESS" => LineClass::Progress,
        "INFO" => LineClass::Info,
        "OK" => LineClass::Ok,
        "WARN" => LineClass::Warn,
        "FAIL" => LineClass::Fail,
        other => return Err(format!("class {other:?}: {line}")),
    };
    let continuation = if c[2].contains("next line") {
        Some(Continuation::Next)
    } else if c[2].contains("previous line") {
        Some(Continuation::Previous)
    } else {
        None
    };
    Ok((
        id,
        stream,
        backticked(&c[2]).first().map(|p| p.to_string()),
        class,
        continuation,
        c[2].contains("no newline"),
        backticked(&c[4]).join("; "),
    ))
}

/// The classification table of a contract page's text. Every line of the section that starts
/// with `|` is the header, the separator, or a body row that must parse, all in one block: a row
/// the parser cannot read, or a table line away from the table, is an error, never skipped.
fn documented(text: &str) -> Result<Vec<Row>, String> {
    let part_b = section(text, "## Part B").join("\n");
    let lines = section(&part_b, "### Output line classification");
    let table: Vec<(usize, &str)> = lines
        .iter()
        .enumerate()
        .filter(|(_, l)| l.trim_start().starts_with('|'))
        .map(|(i, l)| (i, *l))
        .collect();
    let (Some(&(_, header)), Some(&(_, separator))) = (table.first(), table.get(1)) else {
        return Err("no table in the section".into());
    };
    if header != HEADER {
        return Err(format!("header {header:?}"));
    }
    if cells(separator).iter().any(|c| c != "---") {
        return Err(format!("separator {separator:?}"));
    }
    if table.windows(2).any(|w| w[1].0 != w[0].0 + 1) {
        return Err("the table's lines are not one block".into());
    }
    table[2..].iter().map(|(_, l)| row(l)).collect()
}

fn ours() -> Vec<Row> {
    LINE_RULES
        .iter()
        .map(|r| {
            (
                r.id.to_string(),
                r.stream,
                r.pattern.map(str::to_string),
                r.class,
                r.continuation,
                r.unterminated,
                r.receipt.to_string(),
            )
        })
        .collect()
}

#[test]
fn the_classifier_table_is_the_contract_pages_part_b() {
    let rows = documented(&read("docs/solver-contract.md")).unwrap();
    assert_eq!(rows.len(), 22, "Part B's table has 22 rows");
    assert_eq!(
        rows,
        ours(),
        "same rows, same order: id, stream, pattern, class, continuation, receipt"
    );
    let n = |class| rows.iter().filter(|r| r.3 == class).count();
    assert_eq!(
        [
            n(LineClass::Progress),
            n(LineClass::Info),
            n(LineClass::Ok),
            n(LineClass::Fail),
            n(LineClass::Warn)
        ],
        [1, 7, 1, 11, 2]
    );
    println!("{} rows match docs/solver-contract.md Part B", rows.len());
}

#[test]
fn the_comparison_notices_a_row_added_removed_or_changed() {
    let text = read("docs/solver-contract.md");
    let row = "| `directivity_not_open` | stdout | `^DirectivityBalloon : File not open$` | FAIL |";
    let full = text.lines().find(|l| l.starts_with(row)).expect("the row");
    let mutations = [
        // A row changed: its class, its pattern, its stream.
        text.replacen(row, &row.replace("| FAIL |", "| WARN |"), 1),
        text.replacen(row, &row.replace("File not open$", "File not opened$"), 1),
        text.replacen(row, &row.replace("| stdout |", "| stderr |"), 1),
        // A row removed.
        text.lines()
            .filter(|l| !l.starts_with(row))
            .collect::<Vec<_>>()
            .join("\n"),
        // A row added: well formed, without backticks on its id, with a cell missing, and
        // after the table.
        text.replacen(
            row,
            &format!("| `new_row` | stdout | `^New$` | INFO | `x.cpp:1` |\n{row}"),
            1,
        ),
        text.replacen(
            full,
            &format!("{full}\n| new_row | stdout | `^New line$` | FAIL | `x.cpp:1` |"),
            1,
        ),
        text.replacen(full, &format!("{full}\n| `new_row` | stdout | FAIL |"), 1),
        text.replacen(
            "Notes on the classifier:",
            "| `new_row` | stdout | `^New$` | INFO | `x.cpp:1` |\n\nNotes on the classifier:",
            1,
        ),
        // The header changed.
        text.replacen("| Pattern id / reason |", "| Pattern id |", 1),
    ];
    let good = Ok(ours());
    assert_eq!(documented(&text), good);
    for (i, m) in mutations.iter().enumerate() {
        assert_ne!(m, &text, "mutation {i} changed nothing");
        assert_ne!(documented(m), good, "mutation {i} went unnoticed");
    }
}

#[test]
fn no_two_patterns_match_one_documented_sample() {
    // One sample per pattern row, from the upstream sources the receipts name.
    let samples = [
        "#99.99",
        "SPPS version 2.2.1",
        "Classical Theory of Reverberation version 1.0.1",
        "XML configuration file has been loaded.",
        "config.xml",
        "Step 2/3 : Punctual receivers calculation.",
        "Calculation of z ground inside each tetra has been done.",
        "Output results files.",
        "End of calculation.",
        "Xml Property trans_epsilon doesn't exist !",
        "Unable to read the scene mesh file :",
        "Unable to read the tetrahedalization of the scene mesh file, calculation canceled.",
        "Tetrahedron file is empty, the calculation can't be done !",
        "The path of the XML configuration file must be specified!",
        "DirectivityBalloon : File not open",
        "Source at tetrahedron vertex, move source position from [1;2;3] to [1;2;3.1]",
        "Wrong project configuration, a face is defined but no materials are attached to it ! Check",
        "Error in input mesh, a tetrahedra have at least the same two vertices idTetra:1",
        "A sound source position is intersecting with the 3D model. Move the sound source",
        "Unable to find the source position!",
        "Warning 5 particles has been in error on 10 particles. The computation result may be",
    ];
    for (s, r) in samples.iter().zip(&LINE_RULES) {
        assert_eq!(classify::matching_rules(s), [r.id], "{s}");
    }
    assert_eq!(samples.len(), LINE_RULES.len() - 1);
}

/// The codes of Part B's "Reason codes" table, in order.
fn reason_table(text: &str) -> Vec<String> {
    let part_b = section(text, "## Part B").join("\n");
    section(&part_b, "### Reason codes")
        .into_iter()
        .filter(|l| l.starts_with("| `"))
        .map(|l| backticked(&cells(l)[0])[0].to_string())
        .collect()
}

#[test]
fn the_verdicts_codes_are_the_contracts() {
    let text = read("docs/solver-contract.md");
    // Every code the verdict owns is a row of Part B's reason table, in the module's order; the
    // one it borrows is Part A's code for an unreadable config.
    let owned: Vec<&str> = codes::ALL
        .into_iter()
        .filter(|&c| c != codes::CONFIG_ATTRIBUTE_MISSING)
        .collect();
    assert_eq!(reason_table(&text), owned, "Part B's reason codes table");
    assert!(
        RULES
            .iter()
            .any(|r| r.code == codes::CONFIG_ATTRIBUTE_MISSING)
    );
    // Negative: a row dropped from the table, or a code the module does not own added to it.
    let dropped = text.replacen("| `result_unreadable` |", "| result_unreadable |", 1);
    assert_ne!(reason_table(&dropped), owned);
    let added = text.replacen(
        "| `result_unreadable` |",
        "| `made_up` | FAIL | files | nothing |\n| `result_unreadable` |",
        1,
    );
    assert_ne!(reason_table(&added), owned);
    // Part B's codes and the line ids do not collide with Part A's rule codes.
    for r in RULES.iter() {
        assert!(
            !LINE_RULES.iter().any(|l| l.id == r.code),
            "{} is both a rule and a line id",
            r.code
        );
        if r.code != codes::CONFIG_ATTRIBUTE_MISSING {
            assert!(!codes::ALL.contains(&r.code), "{}", r.code);
        }
    }
    for l in LINE_RULES.iter() {
        assert!(!codes::ALL.contains(&l.id), "{}", l.id);
    }
}
