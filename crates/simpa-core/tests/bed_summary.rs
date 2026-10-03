//! `beds/summary.json` is generated, never hand-written (M12 PLAN.md, P1 item 1): the script
//! `tools/bed/summary.py` reads the bed artifacts the result documents name as final and derives
//! each parameter's PASS or FAIL from them, with each artifact's sha256. These tests re-run the
//! script and hold the committed file to its output byte for byte, so the file cannot drift from
//! its evidence, and show the status is derived (a planted wrong-silent row turns it FAIL; a
//! missing artifact refuses its parameters by name).
//!
//! Needs the evidence under `$SIMPA_BED_DATA` (default `B:/data`: `m8b-bed`, `m8b-edt`) and Python
//! 3 (`$PYTHON`, default `python`). Without the evidence the tests fail and say so: a skipped
//! regeneration would let the file drift unseen.

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

fn repo() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn data_root() -> PathBuf {
    std::env::var_os("SIMPA_BED_DATA").map_or_else(|| PathBuf::from("B:/data"), PathBuf::from)
}

fn script() -> PathBuf {
    repo().join("tools/bed/summary.py")
}

/// Runs the script with `data` as the evidence root, writing to `out`.
fn derive(data: &Path, out: &Path) {
    let output = Command::new(std::env::var_os("PYTHON").unwrap_or("python".into()))
        .arg(script())
        .arg("--data")
        .arg(data)
        .arg("--out")
        .arg(out)
        .output()
        .expect("python runs tools/bed/summary.py");
    assert!(
        output.status.success(),
        "summary.py failed: {}\n{}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );
}

fn need_evidence() -> PathBuf {
    let data = data_root();
    assert!(
        data.join("m8b-bed").is_dir() && data.join("m8b-edt").is_dir(),
        "the bed evidence is not under {} (set SIMPA_BED_DATA): beds/summary.json cannot be checked \
         against it",
        data.display()
    );
    data
}

fn scratch(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("simpa-bed-summary-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn committed() -> (Vec<u8>, Value) {
    let bytes = std::fs::read(repo().join("beds/summary.json")).expect("beds/summary.json");
    let v = serde_json::from_slice(&bytes).expect("beds/summary.json is JSON");
    (bytes, v)
}

/// The external artifacts the committed summary names, relative to the evidence root.
fn external_artifacts(summary: &Value) -> Vec<String> {
    let mut out: Vec<String> = summary["parameters"]
        .as_object()
        .unwrap()
        .values()
        .flat_map(|p| p["artifacts"].as_array().unwrap().iter())
        .flat_map(|a| [a["path"].as_str(), a["git_commit_from"]["path"].as_str()])
        .flatten()
        .filter_map(|p| p.strip_prefix("B:/data/").map(str::to_string))
        .collect();
    out.sort();
    out.dedup();
    out
}

/// A copy of the evidence the summary reads, under a scratch root.
fn copy_evidence(data: &Path, to: &Path, summary: &Value) {
    for rel in external_artifacts(summary) {
        let dst = to.join(&rel);
        std::fs::create_dir_all(dst.parent().unwrap()).unwrap();
        std::fs::copy(data.join(&rel), &dst).unwrap_or_else(|e| panic!("{rel}: {e}"));
    }
}

#[test]
fn the_committed_summary_is_what_the_script_derives_from_the_artifacts_byte_for_byte() {
    let data = need_evidence();
    let dir = scratch("regen");
    let out = dir.join("summary.json");
    derive(&data, &out);
    let (want, _) = committed();
    let got = std::fs::read(&out).unwrap();
    assert!(
        got == want,
        "beds/summary.json differs from what tools/bed/summary.py derives from the evidence: \
         regenerate it with the script, never by hand ({} vs {} bytes)",
        want.len(),
        got.len()
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn every_report_parameter_has_a_derived_status_with_hashed_artifacts() {
    let (_, s) = committed();
    let params = s["parameters"].as_object().unwrap();
    let names: Vec<&str> = params.keys().map(String::as_str).collect();
    let mut want = vec![
        "spl_db", "edt_s", "t20_s", "t30_s", "c50_db", "c80_db", "d50", "ts_s", "sti", "g_db",
        "dba",
    ];
    want.sort();
    let mut got = names.clone();
    got.sort();
    assert_eq!(got, want, "one entry per parameter the report carries");
    for (name, p) in params {
        let status = p["status"].as_str().unwrap();
        assert!(status == "PASS" || status == "FAIL", "{name}: {status}");
        let reasons = p["reasons"].as_array().unwrap();
        assert_eq!(
            status == "FAIL",
            !reasons.is_empty(),
            "{name}: a FAIL names its reasons, a PASS has none"
        );
        assert!(
            p["result"]["document"].is_string(),
            "{name}: its result document"
        );
        for a in p["artifacts"].as_array().unwrap() {
            let sha = a["sha256"].as_str().unwrap_or("");
            assert!(
                sha.len() == 64 && sha.bytes().all(|b| b.is_ascii_hexdigit()),
                "{name}: {} has no sha256",
                a["path"]
            );
            assert!(
                a.get("git_commit").is_some(),
                "{name}: {} names its commit (or null)",
                a["path"]
            );
        }
        if status == "PASS" {
            assert!(
                !p["artifacts"].as_array().unwrap().is_empty(),
                "{name}: a PASS rests on at least one artifact"
            );
        }
    }
}

#[test]
fn a_wrong_silent_row_planted_in_an_artifact_turns_that_parameter_fail() {
    let data = need_evidence();
    let (_, s) = committed();
    assert_eq!(
        s["parameters"]["t20_s"]["status"], "PASS",
        "the premise: T20 passes as committed"
    );
    let root = scratch("plant");
    copy_evidence(&data, &root, &s);
    let f = root.join("m8b-bed/B-score-F-fresh/summary.json");
    let mut v: Value = serde_json::from_slice(&std::fs::read(&f).unwrap()).unwrap();
    v["score"]["t20"]["wrong_silent"] = 1.into();
    std::fs::write(&f, serde_json::to_vec_pretty(&v).unwrap()).unwrap();
    let out = root.join("out.json");
    derive(&root, &out);
    let got: Value = serde_json::from_slice(&std::fs::read(&out).unwrap()).unwrap();
    let t20 = &got["parameters"]["t20_s"];
    assert_eq!(t20["status"], "FAIL", "{t20:#}");
    assert!(
        t20["reasons"].to_string().contains("wrong_silent"),
        "the reason names the check: {t20:#}"
    );
    // Nothing else moved but T20 (and the artifact's hash wherever it is listed).
    assert_eq!(
        got["parameters"]["spl_db"]["status"],
        s["parameters"]["spl_db"]["status"]
    );
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn a_missing_artifact_refuses_its_parameters_by_name() {
    let data = need_evidence();
    let (_, s) = committed();
    let root = scratch("missing");
    copy_evidence(&data, &root, &s);
    std::fs::remove_file(root.join("m8b-bed/C-score-F/summary.json")).unwrap();
    let out = root.join("out.json");
    derive(&root, &out);
    let got: Value = serde_json::from_slice(&std::fs::read(&out).unwrap()).unwrap();
    for name in ["spl_db", "c50_db", "t20_s"] {
        let p = &got["parameters"][name];
        assert_eq!(p["status"], "FAIL", "{name}: {p:#}");
        assert!(
            p["reasons"].to_string().contains("C-score-F/summary.json"),
            "{name} names the missing artifact: {p:#}"
        );
    }
    let _ = std::fs::remove_dir_all(&root);
}
