//! `run.json`: a manifest survives a round trip through its JSON, its field names are fixed,
//! and a manifest of another layout is refused rather than read with fields lost.

#[path = "run_support.rs"]
mod support;

use simpa_core::process::Outcome;
use simpa_core::run::manifest::{self, MANIFEST_VERSION, sha256_bytes, sha256_file};
use simpa_core::run::{
    ClassCounts, DEFAULT_LOSS_LIMIT, Evidence, FileCounts, FileRef, MeshRef, RunManifest,
    RunSource, classify_all, judge,
};
use simpa_core::schema::SolverKind;
use support::*;

fn sample() -> RunManifest {
    let exp = tutorial1_expectation(SolverKind::Spps);
    let outputs = good_outputs(&exp);
    let lines = classify_all(&clean_spps_lines());
    let outcome = Outcome {
        exit_code: Some(0xC000_0005),
        cancelled: false,
        elapsed_ms: 1234.5,
    };
    let verdict = judge(&Evidence {
        solver: SolverKind::Spps,
        outcome: &outcome,
        lines: &lines,
        expectation: Ok(&exp),
        outputs: &outputs,
        loss_limit: DEFAULT_LOSS_LIMIT,
    });
    RunManifest {
        manifest_version: MANIFEST_VERSION,
        core_version: simpa_core::VERSION.to_string(),
        solver_commit: simpa_core::SOLVER_COMMIT.to_string(),
        source: RunSource::Project {
            path: r"C:\p\tutorial1_box.simpa".into(),
            sha256: sha256_bytes(b"project"),
            variant: None,
        },
        solver: SolverKind::Spps,
        exe: FileRef {
            path: r"C:\solvers\spps.exe".into(),
            sha256: sha256_bytes(b"exe"),
        },
        argv: vec!["config.xml".into()],
        cwd: r"C:\runs\20260923-191500-123-spps\solve".into(),
        started: "2026-09-23T19:15:00.123+02:00".into(),
        inputs: vec![FileRef {
            path: "config.xml".into(),
            sha256: sha256_bytes(b"config"),
        }],
        mesh: Some(MeshRef {
            manifest: Some(r"C:\runs\20260923-191500-123-spps\mesh\mesh.json".into()),
            mesh_input_hash: Some("0123456789abcdef0123456789abcdef".into()),
            mbin_sha256: sha256_bytes(b"mbin"),
        }),
        outcome,
        lines: ClassCounts::of(&lines),
        files: FileCounts {
            total: 68,
            expected: 65,
            present: 65,
        },
        loss_limit: DEFAULT_LOSS_LIMIT,
        verdict,
    }
}

#[test]
fn a_manifest_round_trips() {
    let m = sample();
    let json = m.to_json();
    assert!(json.ends_with("}\n"));
    assert_eq!(RunManifest::from_json(&json).unwrap(), m);
    let fixture_run = RunManifest {
        source: RunSource::Fixture {
            path: "tests/fixtures/runs/mat7miss".into(),
        },
        mesh: None,
        ..sample()
    };
    let json = fixture_run.to_json();
    assert!(json.contains(r#""kind": "fixture""#), "{json}");
    assert_eq!(RunManifest::from_json(&json).unwrap(), fixture_run);
}

#[test]
fn the_field_names_are_fixed() {
    let v: serde_json::Value = serde_json::from_str(&sample().to_json()).unwrap();
    let keys: Vec<&str> = v.as_object().unwrap().keys().map(String::as_str).collect();
    let mut want = [
        "manifest_version",
        "core_version",
        "solver_commit",
        "source",
        "solver",
        "exe",
        "argv",
        "cwd",
        "started",
        "inputs",
        "mesh",
        "outcome",
        "lines",
        "files",
        "loss_limit",
        "verdict",
    ];
    want.sort_unstable();
    let mut keys = keys;
    keys.sort_unstable();
    assert_eq!(keys, want);
    assert_eq!(v["solver"], "spps");
    assert_eq!(v["source"]["kind"], "project");
    assert_eq!(v["outcome"]["exit_code"], 0xC000_0005u32);
    assert_eq!(v["verdict"]["status"], "CRASH");
    assert_eq!(v["verdict"]["reasons"][0]["code"], "crash_access_violation");
    assert_eq!(v["lines"]["ok"], 1);
}

#[test]
fn a_manifest_of_another_layout_is_refused() {
    let json = sample().to_json();
    // An unknown field, at the top and inside.
    let extra = json.replacen('{', r#"{"surprise": 1,"#, 1);
    assert!(RunManifest::from_json(&extra).is_err());
    let inner = json.replacen(r#""total": 68"#, r#""total": 68, "hidden": 0"#, 1);
    assert_ne!(inner, json);
    assert!(RunManifest::from_json(&inner).is_err());
    // A missing field.
    let v: serde_json::Value = serde_json::from_str(&json).unwrap();
    let mut obj = v.as_object().unwrap().clone();
    obj.remove("loss_limit");
    assert!(RunManifest::from_json(&serde_json::Value::Object(obj).to_string()).is_err());
}

#[test]
fn file_counts_count_only_present_non_empty_expected_files() {
    let exp = tutorial1_expectation(SolverKind::Tcr);
    let expected = exp.expected_files();
    let mut out = good_outputs(&exp);
    out.files.insert("config.xml".into(), 900);
    assert_eq!(
        FileCounts::of(&expected, &out),
        FileCounts {
            total: 88,
            expected: 87,
            present: 87
        }
    );
    out.files.remove(&expected[0]);
    out.files.insert(expected[1].clone(), 0);
    assert_eq!(
        FileCounts::of(&expected, &out),
        FileCounts {
            total: 87,
            expected: 87,
            present: 85
        }
    );
}

#[test]
fn the_inputs_are_hashed_by_relative_path() {
    let dir = stage_tutorial1("manifest-inputs", SolverKind::Spps, |t| t);
    std::fs::create_dir_all(dir.join("directivities")).unwrap();
    std::fs::write(dir.join("directivities").join("b.txt"), b"abc").unwrap();
    let inputs = manifest::hash_folder(&dir).unwrap();
    let paths: Vec<&str> = inputs.iter().map(|f| f.path.as_str()).collect();
    assert_eq!(
        paths,
        [
            "config.xml",
            "directivities/b.txt",
            "mesh.cbin",
            "tetramesh.mbin"
        ]
    );
    assert!(inputs[2].sha256.starts_with("b99a341413487a10"));
    assert_eq!(inputs[1].sha256, sha256_bytes(b"abc"));
    assert!(manifest::hash_folder(&dir.join("absent")).is_err());
}

#[test]
fn sha256_of_a_file_matches_the_fixture_provenance() {
    // tests/fixtures/upstream/tutorial1/PROVENANCE.md lists 16-hex-digit prefixes.
    for (rel, prefix) in [
        ("upstream/tutorial1/spps/mesh.cbin", "b99a341413487a10"),
        ("upstream/tutorial1/spps/tetramesh.mbin", "8a6b3943dd47126b"),
    ] {
        let path = fixture(rel);
        let h = sha256_file(&path).unwrap();
        assert!(h.starts_with(prefix), "{rel}: {h}");
        assert_eq!(h, sha256_bytes(&std::fs::read(&path).unwrap()));
        assert_eq!(h.len(), 64);
    }
    assert!(sha256_file(&fixture("upstream/tutorial1/spps/nope.bin")).is_err());
    assert_eq!(manifest::FILE_NAME, "run.json");
}
