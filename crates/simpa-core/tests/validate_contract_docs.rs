//! The validator's tables against the documents they implement: `docs/solver-contract.md` Part A
//! (every rule's code, stage and severity) and `docs/formats/config_xml.md`'s reference tables
//! (every attribute a solver reads, its type and its Writer column).

use std::path::PathBuf;

use simpa_core::validate::{
    AttrKind, CONFIG_ATTRIBUTES, RULES, STRUCTURAL_CODES, Severity, Stage, Writer,
};

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

#[test]
fn the_rule_table_is_the_contract_pages_part_a() {
    let text = read("docs/solver-contract.md");
    let mut documented = Vec::new();
    for line in section(&text, "## Part A") {
        if !line.starts_with("| `") {
            continue;
        }
        let c = cells(line);
        let stage = match c[1].as_str() {
            "project" => Stage::Project,
            "export" => Stage::Export,
            _ => continue,
        };
        let severity = if c[2].starts_with("error") {
            Severity::Error
        } else if c[2].starts_with("warning") {
            Severity::Warning
        } else {
            panic!("severity of {line}")
        };
        let code = backticked(&c[0])[0].to_string();
        documented.push((code, stage, severity));
    }
    let ours: Vec<(String, Stage, Severity)> = RULES
        .iter()
        .map(|r| (r.code.to_string(), r.stage, r.severity))
        .collect();
    assert_eq!(
        documented, ours,
        "same rules, same order, same stage and severity"
    );
    println!(
        "{} rules match docs/solver-contract.md Part A",
        documented.len()
    );

    // The structural codes are Part A's "Structural faults" table, in order, and appear nowhere
    // else on the page.
    let structural: Vec<String> = section(&text, "### Structural faults")
        .into_iter()
        .filter(|l| l.starts_with("| `"))
        .map(|l| {
            let c = cells(l);
            assert_eq!(
                (c[1].as_str(), c[2].as_str()),
                ("structure", "error"),
                "{l}"
            );
            backticked(&c[0])[0].to_string()
        })
        .collect();
    assert_eq!(structural, STRUCTURAL_CODES, "the structural faults table");
    for code in STRUCTURAL_CODES {
        assert_eq!(
            text.matches(&format!("`{code}`")).count(),
            1,
            "{code} must appear once on the contract page, in its structural row"
        );
    }
}

#[test]
fn the_attribute_table_is_config_xml_mds_reference() {
    let text = read("docs/formats/config_xml.md");
    let mut documented = Vec::new();
    for line in section(&text, "## Element and attribute reference") {
        if !line.starts_with("| `") {
            continue;
        }
        let c = cells(line);
        assert_eq!(c.len(), 6, "{line}");
        let (kind_cell, writer_cell) = (&c[1], &c[4]);
        let kind = if kind_cell.starts_with("string") {
            "text"
        } else if kind_cell.starts_with("int") {
            "int"
        } else if kind_cell.starts_with("real") {
            "real"
        } else {
            panic!("type of {line}")
        };
        let writer = if writer_cell.starts_with("always") {
            Writer::Always
        } else {
            match writer_cell.as_str() {
                "SPPS" => Writer::Spps,
                "TCR" => Writer::Tcr,
                "never" => Writer::Never,
                "if type 1 or 5" => Writer::IfDirected,
                "if type 5" => Writer::IfBalloon,
                "if the material transmits" => Writer::IfTransmits,
                "if surface receivers" | "if cutting planes" | "if fittings" => Writer::IfElement,
                other => panic!("writer {other:?} in {line}"),
            }
        };
        for key in backticked(&c[0]) {
            assert!(key.contains('@'), "{key}");
            documented.push((key.to_string(), kind, writer));
        }
    }
    let ours: Vec<(String, &str, Writer)> = CONFIG_ATTRIBUTES
        .iter()
        .map(|a| {
            let kind = match a.kind {
                AttrKind::Text => "text",
                AttrKind::Int | AttrKind::IntRange(..) => "int",
                AttrKind::Real => "real",
            };
            (a.key(), kind, a.writer)
        })
        .collect();
    assert_eq!(documented.len(), 94);
    assert_eq!(
        documented, ours,
        "same keys, order, types and Writer column"
    );
    println!(
        "{} attributes match docs/formats/config_xml.md",
        documented.len()
    );
}

#[test]
fn every_attribute_upstreams_gui_writes_is_documented_or_ignored() {
    // Tutorial 1's GUI-written configs: every attribute is either one a solver reads (in the
    // table) or on config_xml.md's "Ignored by the solvers" list.
    let ignored = [
        "type_surface@resistivite",
        "condition_atmospherique@lst_soltype",
        "simulation@intensity_filename",
        "simulation@intensity_folder",
        "simulation@intensity_rp_filename",
        "source@id",
        "recepteur_ponctuel@name",
    ];
    let known: Vec<String> = CONFIG_ATTRIBUTES.iter().map(|a| a.key()).collect();
    for solver in ["spps", "tcr"] {
        let text = read(&format!(
            "tests/fixtures/upstream/tutorial1/{solver}/config.xml"
        ));
        let doc = roxmltree::Document::parse(&text).unwrap();
        let mut unknown = Vec::new();
        for node in doc.descendants().filter(|n| n.is_element()) {
            let name = node.tag_name().name();
            let parent = node.parent_element().map(|p| p.tag_name().name());
            let element = match (name, parent) {
                ("configuration", _) => "configuration".to_string(),
                ("bfreq", Some(p)) => format!("{p}/bfreq"),
                (n, _) => n.to_string(),
            };
            for a in node.attributes() {
                let key = format!("{element}@{}", a.name());
                if !known.contains(&key) && !ignored.contains(&key.as_str()) {
                    unknown.push(key);
                }
            }
        }
        unknown.sort();
        unknown.dedup();
        assert!(unknown.is_empty(), "{solver}: {unknown:?}");
    }
}
