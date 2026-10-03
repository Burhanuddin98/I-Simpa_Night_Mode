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
        // Every status says what it rests on; one that rests on a ruling keeps the rule's beside it.
        let by = p["by"]
            .as_str()
            .unwrap_or_else(|| panic!("{name}: no `by`"));
        if by != "the rule" {
            assert!(by.starts_with("decision "), "{name}: {by}");
            assert_eq!(
                p["rule"]["status"], "FAIL",
                "{name}: a ruling only where the rule fails"
            );
            assert!(
                !p["rule"]["reasons"].as_array().unwrap().is_empty(),
                "{name}: the rule's reasons are kept"
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

/// Rewrites one JSON artifact of a scratch copy of the evidence.
fn plant(root: &Path, rel: &str, f: impl FnOnce(&mut Value)) {
    let p = root.join(rel);
    let mut v: Value = serde_json::from_slice(&std::fs::read(&p).unwrap()).unwrap();
    f(&mut v);
    std::fs::write(&p, serde_json::to_vec_pretty(&v).unwrap()).unwrap();
}

fn derive_planted(name: &str, f: impl FnOnce(&Path)) -> Value {
    let data = need_evidence();
    let (_, s) = committed();
    let root = scratch(name);
    copy_evidence(&data, &root, &s);
    f(&root);
    let out = root.join("out.json");
    derive(&root, &out);
    let got: Value = serde_json::from_slice(&std::fs::read(&out).unwrap()).unwrap();
    let _ = std::fs::remove_dir_all(&root);
    got
}

fn paths_of(p: &Value) -> Vec<String> {
    p["artifacts"]
        .as_array()
        .unwrap()
        .iter()
        .map(|a| a["path"].as_str().unwrap().to_string())
        .collect()
}

const EDT_SCORE: &str = "m8b-edt/round2/results/score/summary.json";
const EDT_H6: &str = "m8b-edt/round2/results/attack/h6.json";
const GDBA: &str = "m8b-bed/C-score-GdBA/summary.json";
const T30_RULED: &str = "m8b-bed/B-score-F-fresh/summary.json";

/// EDT (SENTINEL-EDT.md): the scored round-2 file's own verdict was computed with an empty attack
/// set; VERDICT-2.md joins its H1-H5 on the real sets with H6 from the attacker run. The summary
/// makes that join, from both files, hashed, and it fails when either half does.
#[test]
fn edt_is_the_join_of_h1_to_h5_on_the_real_sets_with_h6_from_the_attacker_run() {
    let (_, s) = committed();
    let edt = &s["parameters"]["edt_s"];
    assert_eq!(edt["status"], "PASS", "{edt:#}");
    assert_eq!(edt["by"], "the rule", "{edt:#}");
    let paths = paths_of(edt);
    for want in [EDT_SCORE, EDT_H6] {
        assert!(
            paths.contains(&format!("B:/data/{want}")),
            "{want} is read and hashed: {paths:?}"
        );
    }
    let notes = edt["notes"].to_string();
    assert!(
        notes.contains("receiver radius above 1 m")
            && notes.contains("Energetic-mode EDT not validated"),
        "row 37's two marks stay in the notes: {notes}"
    );

    // H6 failing in the attacker run fails EDT.
    let got = derive_planted("edt-h6", |r| plant(r, EDT_H6, |v| v["pass"] = false.into()));
    let p = &got["parameters"]["edt_s"];
    assert_eq!(p["status"], "FAIL", "{p:#}");
    assert!(p["reasons"].to_string().contains("h6.json"), "{p:#}");

    // H5 failing on a real set fails EDT.
    let got = derive_planted("edt-h5", |r| {
        plant(r, EDT_SCORE, |v| {
            v["criteria"]["frozen"]["random"]["H5"]["per_set"]["ism"]["pass"] = false.into()
        })
    });
    let p = &got["parameters"]["edt_s"];
    assert_eq!(p["status"], "FAIL", "{p:#}");
    assert!(
        p["reasons"].to_string().contains("H5.per_set.ism.pass"),
        "{p:#}"
    );

    // The join holds only while the scored file left the attack set empty for H6 to judge.
    let got = derive_planted("edt-attack", |r| {
        plant(r, EDT_SCORE, |v| {
            v["inputs"]["per_set"]["attack"] = 5.into()
        })
    });
    assert_eq!(got["parameters"]["edt_s"]["status"], "FAIL");

    // The attacker run must have scored the same frozen method.
    let got = derive_planted("edt-method", |r| {
        plant(r, EDT_H6, |v| v["method_sha256"] = "0".repeat(64).into())
    });
    let p = &got["parameters"]["edt_s"];
    assert_eq!(p["status"], "FAIL", "{p:#}");
    assert!(p["reasons"].to_string().contains("method_sha256"), "{p:#}");
}

/// G and dB(A) (ADDENDUM-6, RESULT-GDBA.md): set C's G and dB(A) summary is read as the other set
/// summaries are: no wrong-silent value, the scorer's own pass flag.
#[test]
fn g_and_dba_are_read_from_their_set_c_summary_like_the_other_sets() {
    let (_, s) = committed();
    for name in ["g_db", "dba"] {
        let p = &s["parameters"][name];
        assert_eq!(p["status"], "PASS", "{name}: {p:#}");
        assert_eq!(
            paths_of(p),
            vec![format!("B:/data/{GDBA}")],
            "{name}: read from ADDENDUM-6's summary"
        );
        let reads: Vec<&str> = p["artifacts"][0]["checks"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| c["read"].as_str().unwrap())
            .collect();
        assert_eq!(
            reads,
            vec![
                format!("score.{name}.wrong_silent").as_str(),
                format!("score.{name}.pass_").as_str()
            ],
            "{name}"
        );
    }
    let got = derive_planted("gdba", |r| {
        plant(r, GDBA, |v| v["score"]["g_db"]["wrong_silent"] = 1.into())
    });
    let g = &got["parameters"]["g_db"];
    assert_eq!(g["status"], "FAIL", "{g:#}");
    assert!(
        g["reasons"].to_string().contains("score.g_db.wrong_silent"),
        "{g:#}"
    );
    assert_eq!(
        got["parameters"]["dba"]["status"], "PASS",
        "dB(A) did not move"
    );
}

/// T30 (decision 46): FAIL by the rule, PASS by the ruling, and the file says which. The ruling
/// covers the one wrong-silent row it names and nothing else.
#[test]
fn t30_is_pass_by_decision_46_only_while_its_named_row_is_the_only_wrong_silent_row() {
    let (_, s) = committed();
    let t30 = &s["parameters"]["t30_s"];
    assert_eq!(t30["status"], "PASS", "{t30:#}");
    assert_eq!(t30["by"], "decision 46", "{t30:#}");
    assert!(t30["reasons"].as_array().unwrap().is_empty(), "{t30:#}");
    let ruling = &t30["ruling"];
    assert_eq!(ruling["decision"], 46, "{t30:#}");
    assert_eq!(ruling["date"], "2026-10-03 12:52", "{t30:#}");
    assert!(
        ruling["text"]
            .as_str()
            .unwrap()
            .contains("docs/decision-log.md"),
        "{t30:#}"
    );
    assert_eq!(ruling["row"]["room"], "G6");
    assert_eq!(ruling["row"]["receiver"], "R007");
    assert_eq!(ruling["row"]["band_hz"], 1000);
    assert_eq!(ruling["row"]["seed"], 4201);
    // The rule's own status and reasons, kept beside the ruling.
    assert_eq!(t30["rule"]["status"], "FAIL", "{t30:#}");
    let why = t30["rule"]["reasons"].to_string();
    assert!(
        why.contains("B-score-F-fresh") && why.contains("score.t30.wrong_silent"),
        "{t30:#}"
    );
    assert!(
        t30["notes"].to_string().contains("decision 46"),
        "the report's notes say PASS by ruling: {t30:#}"
    );
    // No other parameter is PASS by a ruling.
    for (name, p) in s["parameters"].as_object().unwrap() {
        if name != "t30_s" {
            assert_eq!(p["by"], "the rule", "{name}");
        }
    }

    // A second wrong-silent row beside the named one: the ruling does not cover it.
    let got = derive_planted("t30-second", |r| {
        plant(r, T30_RULED, |v| {
            let t = &mut v["score"]["t30"];
            t["wrong_silent"] = 2.into();
            t["wrong_silent_rows"]
                .as_array_mut()
                .unwrap()
                .push(serde_json::json!([
                    "G1", "R000", 500, 4202, 1.2, 1.0, 1.1, 1.15
                ]));
        })
    });
    let p = &got["parameters"]["t30_s"];
    assert_eq!(p["status"], "FAIL", "{p:#}");
    assert_eq!(p["by"], "the rule", "{p:#}");
    assert!(p["reasons"].to_string().contains("wrong_silent"), "{p:#}");
    assert!(
        p["ruling"]["applies"] == false && p["ruling"]["why_not"].to_string().contains("G1"),
        "the ruling says why it does not apply: {p:#}"
    );

    // A wrong-silent row on another set: likewise.
    let got = derive_planted("t30-other-set", |r| {
        plant(r, "m8b-bed/C-score-F/summary.json", |v| {
            v["score"]["t30"]["wrong_silent"] = 1.into()
        })
    });
    let p = &got["parameters"]["t30_s"];
    assert_eq!(p["status"], "FAIL", "{p:#}");
    assert_eq!(p["by"], "the rule", "{p:#}");
}
