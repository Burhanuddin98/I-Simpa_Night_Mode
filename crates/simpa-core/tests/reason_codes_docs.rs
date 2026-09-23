//! Every reason code the core produces is documented in exactly one code table, and every code a
//! code table documents is produced.
//!
//! **Documented.** A code table is a Markdown table whose header's first cell is `Code` or
//! `Pattern id / reason`, in `docs/solver-contract.md` Part A (the validator) or Part B (runs), or
//! in `docs/formats/mesh-manifest.md` (the mesher and `mesh::verify`). Each body row's first cell
//! must be one backticked snake_case code; any other row is an error, never skipped.
//!
//! **Produced.** Read from the source of the three families whose reasons carry codes, every
//! `.rs` file of `src/validate*`, `src/run*` and `src/mesh*`:
//! - a `const NAME: &str = "code";` inside a `mod codes { .. }` block;
//! - a row id of `LINE_RULES`: `rule("id", ..` or `rule(CONST, ..`, the constant in the same file;
//! - a count of `VerifyReport::counts`: `("code", self.code)`, the field spelled as its code;
//! - a folder code of `verify_dir`: `codes.push("code")`;
//! - a code literal passed straight to `issue(`, `Reason::new(` or the mesher's `fail(m, `.
//!
//! Other modules' error codes (`geometry::check`, `schema`, `config_xml`'s writer and importer)
//! are their own APIs. A run reports them inside one of these families' reasons, never as one.

use std::collections::BTreeMap;
use std::path::Path;

use regex::Regex;

#[allow(dead_code)]
#[path = "common/paths.rs"]
mod paths;

/// Code to the places it was found.
type Sites = BTreeMap<String, Vec<String>>;

/// `(relative path, text)` of every source file of the three families.
fn source_files() -> Vec<(String, String)> {
    fn walk(root: &Path, dir: &Path, out: &mut Vec<(String, String)>) {
        let mut entries: Vec<_> = std::fs::read_dir(dir)
            .unwrap_or_else(|e| panic!("{}: {e}", dir.display()))
            .map(|e| e.unwrap().path())
            .collect();
        entries.sort();
        for p in entries {
            if p.is_dir() {
                walk(root, &p, out);
            } else if p.extension().is_some_and(|e| e == "rs") {
                let rel = p
                    .strip_prefix(root)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/");
                let family = rel.split(['/', '.']).next().unwrap_or("");
                if ["validate", "run", "mesh"].contains(&family) {
                    let text = std::fs::read_to_string(&p).unwrap();
                    out.push((rel, text));
                }
            }
        }
    }
    let root = paths::repo_file("crates/simpa-core/src");
    let mut out = Vec::new();
    walk(&root, &root, &mut out);
    assert!(
        out.iter().any(|(p, _)| p == "run/verdict.rs") && out.len() > 10,
        "the source walk found {} files",
        out.len()
    );
    out
}

/// The body of each `mod codes { .. }` block of `text`.
fn codes_blocks(text: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(at) = rest.find("mod codes {") {
        let body = &rest[at + "mod codes {".len()..];
        let mut depth = 1usize;
        let mut end = body.len();
        for (i, c) in body.char_indices() {
            match c {
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        end = i;
                        break;
                    }
                }
                _ => {}
            }
        }
        out.push(&body[..end]);
        rest = &body[end..];
    }
    out
}

/// Every code the sources produce, with where.
fn produced(files: &[(String, String)]) -> Sites {
    let re = |p: &str| Regex::new(p).unwrap();
    let str_const = re(r#"const\s+([A-Z][A-Z0-9_]*)\s*:\s*&str\s*=\s*"([^"]*)"\s*;"#);
    let rule_lit = re(r#"\brule\(\s*"([^"]*)""#);
    let rule_const = re(r#"\brule\(\s*([A-Z][A-Z0-9_]*)\s*,"#);
    let count = re(r#"\(\s*"([a-z0-9_]+)"\s*,\s*self\.([a-z0-9_]+)\s*,?\s*\)"#);
    let push = re(r#"\bcodes\.push\(\s*"([^"]*)"\s*\)"#);
    let call = re(r#"\b(?:issue|Reason::new)\(\s*"([^"]*)""#);
    let fail = re(r#"\bfail\(\s*(?:&mut\s+)?[a-z_][a-z0-9_]*\s*,\s*"([^"]*)""#);
    let mut out = Sites::new();
    for (path, text) in files {
        let mut add = |code: &str, how: &str| {
            out.entry(code.to_string())
                .or_default()
                .push(format!("{path} ({how})"));
        };
        for block in codes_blocks(text) {
            for c in str_const.captures_iter(block) {
                add(&c[2], "mod codes");
            }
        }
        let consts: BTreeMap<&str, &str> = str_const
            .captures_iter(text)
            .map(|c| (c.get(1).unwrap().as_str(), c.get(2).unwrap().as_str()))
            .collect();
        for c in rule_lit.captures_iter(text) {
            add(&c[1], "LINE_RULES");
        }
        for c in rule_const.captures_iter(text) {
            let value = consts
                .get(&c[1])
                .unwrap_or_else(|| panic!("{path}: rule({}, ..) names no &str const", &c[1]));
            add(value, "LINE_RULES");
        }
        // A count's code is spelled as its field: `("index_errors", self.index_errors)`.
        for c in count.captures_iter(text).filter(|c| c[1] == c[2]) {
            add(&c[1], "VerifyReport::counts");
        }
        for (re, how) in [
            (&push, "codes.push"),
            (&call, "issue / Reason::new"),
            (&fail, "fail"),
        ] {
            for c in re.captures_iter(text) {
                add(&c[1], how);
            }
        }
    }
    out
}

/// Every code the code tables of `docs` (name, text) document, with where; `Err` for a malformed
/// row.
fn documented(docs: &[(&str, &str)]) -> Result<Sites, String> {
    let code = Regex::new(r"^`([a-z][a-z0-9]*(?:_[a-z0-9]+)*)`$").unwrap();
    let mut out = Sites::new();
    for (name, text) in docs {
        let lines: Vec<&str> = text.lines().collect();
        let mut i = 0;
        while i < lines.len() {
            if !lines[i].trim_start().starts_with('|') {
                i += 1;
                continue;
            }
            let start = i;
            while i < lines.len() && lines[i].trim_start().starts_with('|') {
                i += 1;
            }
            let first = |l: &str| {
                l.trim()
                    .trim_start_matches('|')
                    .split('|')
                    .next()
                    .unwrap_or("")
                    .trim()
                    .to_string()
            };
            let header = first(lines[start]);
            if header != "Code" && header != "Pattern id / reason" {
                continue;
            }
            for (k, row) in lines[start + 2..i].iter().enumerate() {
                let cell = first(row);
                let Some(c) = code.captures(&cell) else {
                    return Err(format!(
                        "{name}, line {}: the first cell {cell:?} is not one backticked code",
                        start + 3 + k
                    ));
                };
                out.entry(c[1].to_string())
                    .or_default()
                    .push(format!("{name}, table at line {}", start + 1));
            }
        }
    }
    Ok(out)
}

/// The contract page's Part A and Part B, and the mesh manifest page.
fn docs_of(contract: &str, manifest: &str) -> Vec<(&'static str, String)> {
    let a = contract.find("\n## Part A").expect("## Part A") + 1;
    let b = contract.find("\n## Part B").expect("## Part B") + 1;
    // Part B runs to the next level-2 heading, if any.
    let end = contract[b + 3..]
        .find("\n## ")
        .map_or(contract.len(), |e| b + 3 + e + 1);
    vec![
        ("docs/solver-contract.md Part A", contract[a..b].to_string()),
        (
            "docs/solver-contract.md Part B",
            contract[b..end].to_string(),
        ),
        ("docs/formats/mesh-manifest.md", manifest.to_string()),
    ]
}

/// What is wrong: codes produced but documented in no table, documented in more than one row,
/// or documented but produced nowhere.
fn problems(produced: &Sites, documented: &Sites) -> Vec<String> {
    let snake = Regex::new(r"^[a-z][a-z0-9]*(?:_[a-z0-9]+)*$").unwrap();
    let mut out = Vec::new();
    for (code, sites) in produced {
        if !snake.is_match(code) {
            out.push(format!("not a snake_case code: {code:?} at {sites:?}"));
        }
        match documented.get(code).map(Vec::len) {
            None => out.push(format!("undocumented: {code} (produced at {sites:?})")),
            Some(1) => {}
            Some(n) => out.push(format!(
                "documented {n} times: {code} ({:?})",
                documented[code]
            )),
        }
    }
    for (code, sites) in documented {
        if !produced.contains_key(code) {
            out.push(format!("unproduced: {code} (documented at {sites:?})"));
        }
    }
    out
}

fn check(
    files: &[(String, String)],
    contract: &str,
    manifest: &str,
) -> Result<Vec<String>, String> {
    let docs = docs_of(contract, manifest);
    let docs: Vec<(&str, &str)> = docs.iter().map(|(n, t)| (*n, t.as_str())).collect();
    Ok(problems(&produced(files), &documented(&docs)?))
}

fn read(rel: &str) -> String {
    std::fs::read_to_string(paths::repo_file(rel)).unwrap_or_else(|e| panic!("{rel}: {e}"))
}

#[test]
fn every_code_is_produced_and_documented_in_exactly_one_table() {
    let files = source_files();
    let (contract, manifest) = (
        read("docs/solver-contract.md"),
        read("docs/formats/mesh-manifest.md"),
    );
    let found = produced(&files);
    let docs = docs_of(&contract, &manifest);
    let docs: Vec<(&str, &str)> = docs.iter().map(|(n, t)| (*n, t.as_str())).collect();
    let tables = documented(&docs).unwrap();
    let bad = problems(&found, &tables);
    assert!(bad.is_empty(), "{}", bad.join("\n"));
    let per_doc = |d: &str| tables.values().filter(|s| s[0].starts_with(d)).count();
    println!(
        "{} distinct codes, each produced and documented once: Part A {}, Part B {}, \
         mesh-manifest {}",
        found.len(),
        per_doc("docs/solver-contract.md Part A"),
        per_doc("docs/solver-contract.md Part B"),
        per_doc("docs/formats/mesh-manifest.md")
    );
    // Part A holds exactly the validator's rules and structural faults.
    assert_eq!(
        per_doc("docs/solver-contract.md Part A"),
        simpa_core::validate::RULES.len() + simpa_core::validate::STRUCTURAL_CODES.len()
    );
}

#[test]
fn the_check_says_no_to_each_kind_of_drift() {
    let files = source_files();
    let contract = read("docs/solver-contract.md");
    let manifest = read("docs/formats/mesh-manifest.md");
    assert_eq!(check(&files, &contract, &manifest), Ok(vec![]));

    let has = |r: Result<Vec<String>, String>, want: &str| {
        let text = match r {
            Ok(v) => v.join("\n"),
            Err(e) => e,
        };
        assert!(text.contains(want), "wanted {want:?}, got:\n{text}");
    };
    let row = "| `cbin_missing` |";
    assert!(manifest.contains(row));

    // A documented code's row removed: undocumented.
    let removed: String = manifest
        .lines()
        .filter(|l| !l.starts_with(row))
        .collect::<Vec<_>>()
        .join("\n");
    has(
        check(&files, &contract, &removed),
        "undocumented: cbin_missing",
    );

    // A row no source produces: unproduced.
    let added = contract.replacen(
        "| `result_unreadable` |",
        "| `made_up_code` | FAIL | files | nothing |\n| `result_unreadable` |",
        1,
    );
    has(check(&files, &added, &manifest), "unproduced: made_up_code");

    // One code in two tables: documented twice.
    let twice = contract.replacen(
        "| `result_unreadable` |",
        "| `mesh_invalid` | FAIL | files | twice |\n| `result_unreadable` |",
        1,
    );
    has(
        check(&files, &twice, &manifest),
        "documented 2 times: mesh_invalid",
    );

    // A row whose first cell is not one backticked code: an error, not a skipped row.
    let malformed = manifest.replacen(row, "| cbin_missing |", 1);
    has(
        check(&files, &contract, &malformed),
        "is not one backticked code",
    );

    // A new code in a source's codes module, and a new folder code: both undocumented.
    let mut edited = files.clone();
    for (path, text) in edited.iter_mut() {
        if path == "run/verdict.rs" {
            *text = text.replacen(
                "pub mod codes {",
                "pub mod codes {\n    pub const BRAND_NEW: &str = \"brand_new_code\";",
                1,
            );
        }
        if path == "mesh/verify/dir.rs" {
            *text = text.replacen(
                "codes.push(\"cbin_missing\")",
                "codes.push(\"cbin_missing\"); codes.push(\"another_new_code\")",
                1,
            );
        }
    }
    let r = check(&edited, &contract, &manifest);
    has(r.clone(), "undocumented: brand_new_code");
    has(r, "undocumented: another_new_code");

    // A table that is not a code table (the manifest's key table) is not read as one.
    let keys = manifest.replacen("| `manifest_version` |", "| `not_a_code_row` |", 1);
    assert_eq!(check(&files, &contract, &keys), Ok(vec![]));
}
