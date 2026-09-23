//! Gate M6(d) and (e) through the CLI: every folder of `tests/fixtures/runs/` is run with
//! `simpa run-folder <case> --solver <expected.solver>` (the stub solver for `stub_*` cases, the
//! M1 solvers otherwise), and its verdict must meet the case's `expected.json`
//! (`tests/fixtures/runs/README.md`):
//! - `status` is the verdict's status, and the exit code follows it (0 OK, 5 FAIL or CRASH);
//! - every code of `codes` is among the verdict's reason codes;
//! - each group of `codes_any_of` has at least one code among them;
//! - `warnings` are the verdict's WARN rows, each once, in order.
//!
//! A case whose expectation cannot be met fails the test; none is skipped. Then every row of the
//! contract's classification table must have been hit by a line some case's solver printed, and
//! the stub whose last stderr line has no newline must have had that line captured.

mod support;

use std::collections::BTreeSet;
use std::path::Path;

use serde_json::Value;
use simpa_core::process::{Line, Stream};
use simpa_core::run::{LINE_RULES, classify_all};
use support::*;

/// What a case's `expected.json` asks of the verdict in `m` (a `run.json`), and the exit `code`:
/// every way they disagree.
fn unmet(expected: &Value, m: &Value, code: i32) -> Vec<String> {
    let mut out = Vec::new();
    let status = m["verdict"]["status"].as_str().unwrap_or("?");
    let want = expected["status"].as_str().unwrap_or("?");
    if status != want {
        out.push(format!("status {status}, expected {want}"));
    }
    let want_exit = if want == "OK" { 0 } else { 5 };
    if code != want_exit {
        out.push(format!("exit {code}, expected {want_exit}"));
    }
    let got: BTreeSet<String> = codes(m).into_iter().collect();
    for c in expected["codes"].as_array().into_iter().flatten() {
        let c = c.as_str().unwrap();
        if !got.contains(c) {
            out.push(format!("code {c} missing from {got:?}"));
        }
    }
    for group in expected["codes_any_of"].as_array().into_iter().flatten() {
        let group: Vec<&str> = group
            .as_array()
            .unwrap()
            .iter()
            .map(|c| c.as_str().unwrap())
            .collect();
        if !group.iter().any(|c| got.contains(*c)) {
            out.push(format!("none of {group:?} in {got:?}"));
        }
    }
    let mut warned: Vec<&str> = Vec::new();
    for w in m["verdict"]["warnings"].as_array().into_iter().flatten() {
        let c = w["code"].as_str().unwrap();
        if !warned.contains(&c) {
            warned.push(c);
        }
    }
    let want_warned: Vec<&str> = expected["warnings"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|c| c.as_str().unwrap())
        .collect();
    if warned != want_warned {
        out.push(format!("warnings {warned:?}, expected {want_warned:?}"));
    }
    out
}

/// The lines a run's logs hold, as the process layer would have delivered them.
fn logged(dir: &Path) -> Vec<Line> {
    let mut lines = Vec::new();
    for (name, stream) in [
        ("solver.stdout.txt", Stream::Stdout),
        ("solver.stderr.txt", Stream::Stderr),
    ] {
        let Ok(bytes) = std::fs::read(dir.join(name)) else {
            continue;
        };
        let text = String::from_utf8_lossy(&bytes);
        let mut parts: Vec<&str> = text.split('\n').collect();
        let last = parts.pop().unwrap_or("");
        for p in parts {
            lines.push(Line {
                stream,
                t_ms: 0.0,
                text: p.strip_suffix('\r').unwrap_or(p).to_string(),
                terminated: true,
            });
        }
        if !last.is_empty() {
            lines.push(Line {
                stream,
                t_ms: 0.0,
                text: last.to_string(),
                terminated: false,
            });
        }
    }
    lines
}

#[test]
fn every_run_folder_fixture_gives_its_expected_verdict() {
    let runs = fixture("runs");
    let root = scratch("run-folder-fixtures");
    let mut cases: Vec<_> = std::fs::read_dir(&runs)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.is_dir())
        .collect();
    cases.sort();
    assert!(
        cases.len() >= 29,
        "{} cases in {}",
        cases.len(),
        runs.display()
    );
    let mut failures = Vec::new();
    let mut hit: BTreeSet<&'static str> = BTreeSet::new();
    let mut unterminated_captured = false;
    for case in &cases {
        let name = case.file_name().unwrap().to_string_lossy().into_owned();
        let expected: Value = serde_json::from_str(
            &std::fs::read_to_string(case.join("expected.json"))
                .unwrap_or_else(|e| panic!("{name}: expected.json: {e}")),
        )
        .unwrap_or_else(|e| panic!("{name}: expected.json: {e}"));
        let solver = expected["solver"].as_str().expect("expected.solver");
        let mut args = vec![
            "run-folder".to_string(),
            case.display().to_string(),
            "--solver".into(),
            solver.into(),
            "--runs".into(),
            root.display().to_string(),
            "--json".into(),
        ];
        if name.starts_with("stub_") {
            args.extend(["--solver-exe".into(), stub().display().to_string()]);
        }
        let o = simpa_run(&args);
        let m: Value = match serde_json::from_str(&o.stdout) {
            Ok(m) => m,
            Err(e) => {
                failures.push(format!("{name}: no manifest ({e}): {o:?}"));
                continue;
            }
        };
        let problems = unmet(&expected, &m, o.code);
        println!(
            "{name:34} {:6} exit {:3} {:5.0} ms  {}  {}",
            m["verdict"]["status"].as_str().unwrap_or("?"),
            o.code,
            o.ms,
            codes(&m).join(", "),
            if problems.is_empty() { "ok" } else { "UNMET" }
        );
        for p in problems {
            failures.push(format!("{name}: {p}"));
        }
        // What the solver printed, classified as the run manager classified it.
        let dir = run_dir(&m);
        let lines = logged(&dir);
        for c in classify_all(&lines) {
            hit.insert(c.id);
        }
        if name == "stub_particle_loss_unterminated" {
            let last = lines.iter().rfind(|l| l.stream == Stream::Stderr);
            unterminated_captured = last.is_some_and(|l| {
                !l.terminated
                    && l.text
                        .starts_with("Warning 4000 particles has been in error")
            }) && codes(&m).iter().any(|c| c == "particle_loss_reported");
        }
    }
    let met = cases.len()
        - failures
            .iter()
            .map(|f| f.split(':').next().unwrap())
            .collect::<BTreeSet<_>>()
            .len();
    println!(
        "{met}/{} run-folder fixtures give their expected verdict",
        cases.len()
    );
    assert!(failures.is_empty(), "{}", failures.join("\n"));

    // M6(e): every row is hit through run-folder, and the unterminated last line is captured.
    let missed: Vec<&str> = LINE_RULES
        .iter()
        .map(|r| r.id)
        .filter(|id| !hit.contains(id))
        .collect();
    println!(
        "{}/{} classifier rows hit",
        LINE_RULES.len() - missed.len(),
        LINE_RULES.len()
    );
    assert!(missed.is_empty(), "rows no fixture hit: {missed:?}");
    assert!(
        unterminated_captured,
        "stub_particle_loss_unterminated's last stderr line was not captured without its newline"
    );
}

#[test]
fn an_expectation_the_verdict_does_not_meet_is_reported() {
    let m: Value = serde_json::json!({
        "verdict": {
            "status": "FAIL",
            "reasons": [{"code": "mesh_invalid", "detail": ""}, {"code": "degenerate_tets", "detail": ""}],
            "warnings": [{"code": "unclassified_line", "detail": "x"}, {"code": "unclassified_line", "detail": "y"}]
        }
    });
    let expected = serde_json::json!({
        "status": "FAIL", "codes": ["mesh_invalid"],
        "codes_any_of": [["degenerate_tets", "inverted_tets"]], "warnings": ["unclassified_line"]
    });
    assert_eq!(unmet(&expected, &m, 5), Vec::<String>::new());
    // Each way it can fail to meet it.
    let with = |key: &str, v: Value| {
        let mut e = expected.clone();
        e[key] = v;
        e
    };
    let cases = [
        (
            with("status", "CRASH".into()),
            5,
            "status FAIL, expected CRASH",
        ),
        (expected.clone(), 0, "exit 0, expected 5"),
        (
            with("codes", serde_json::json!(["exit_nonzero"])),
            5,
            "code exit_nonzero missing",
        ),
        (
            with("codes_any_of", serde_json::json!([["inverted_tets"]])),
            5,
            "none of",
        ),
        (with("warnings", serde_json::json!([])), 5, "warnings"),
    ];
    for (e, code, want) in cases {
        let got = unmet(&e, &m, code);
        assert!(
            got.iter().any(|p| p.contains(want)),
            "wanted {want:?}, got {got:?}"
        );
    }
}

#[test]
fn the_stub_plays_back_its_script_byte_for_byte() {
    let dir = scratch("stub-bytes");
    std::fs::write(
        dir.join("stub.json"),
        r#"{"exit_code": 3221225477, "lines": [
            {"stream": "stdout", "text": "one", "newline": true, "delay_ms": 0},
            {"stream": "stderr", "text": "two", "newline": true, "delay_ms": 20},
            {"stream": "stdout", "text": "three", "newline": false, "delay_ms": 0}],
          "files": {"a/b.txt": "hello"}}"#,
    )
    .unwrap();
    let out = std::process::Command::new(stub())
        .arg("config.xml")
        .current_dir(&dir)
        .output()
        .unwrap();
    assert_eq!(out.stdout, b"one\nthree");
    assert_eq!(out.stderr, b"two\n");
    assert_eq!(out.status.code().map(|c| c as u32), Some(0xC000_0005));
    assert_eq!(
        std::fs::read_to_string(dir.join("a/b.txt")).unwrap(),
        "hello"
    );
    // Any other argument is refused: a launcher that passes one fails.
    let wrong = std::process::Command::new(stub())
        .arg(dir.join("config.xml"))
        .current_dir(&dir)
        .output()
        .unwrap();
    assert_eq!(wrong.status.code(), Some(97));
    // exit -1 arrives as 0xFFFFFFFF.
    std::fs::write(dir.join("stub.json"), r#"{"exit_code": -1, "lines": []}"#).unwrap();
    let minus = std::process::Command::new(stub())
        .arg("config.xml")
        .current_dir(&dir)
        .output()
        .unwrap();
    assert_eq!(minus.status.code().map(|c| c as u32), Some(u32::MAX));
}
