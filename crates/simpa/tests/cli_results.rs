//! `simpa results` (milestone M7, piece B) through the built binary.
//!
//! - **Every exit path:** 0 on the committed Seat/Seat2 run folders (SPPS and TCR), in JSON and
//!   in text; 2 for each usage error; 5 for a real FAIL run (`run-folder` on the negative fixture
//!   `spps_nomesh`) and a real cancelled one; 6 for each way a run's results fail to verify, on
//!   copies of the committed SPPS run with one thing spoiled each.
//! - **Gate M7(e):** both `Seat` and `Seat2` are read, each from its own folder or file, and a
//!   reader that took `Seat`'s data from the first folder whose name starts with `Seat` would be
//!   caught: the say-no is a copy with the two folders' contents swapped, which reads each label's
//!   data from its own folder, so the series swap with them.
//! - **Gate M7(c):** `rooms/level_box_20m.simpa` through SPPS: every band's SPL at 2 m and 4 m
//!   within ±0.5 dB of `Lw − 20·lg r − 11`. Says no: Night Mode's `.gap` level (energy over
//!   1e-12, `main:project/result_parser.cpp:486`) and a 1 dB offset each way miss it in every
//!   band. Tighter (M7 review): the same SPL against the exact free field within its Monte-Carlo
//!   noise, which the SPL 0.1 dB either way, or with `ρc` = 400, misses.
//! - **Gate M7(d):** tutorial 1's box through TCR: per band, TCR's Sabine and Eyring times equal
//!   `core::params`' within 0.5 %, computed both from the project and from the run's own inputs,
//!   and no value TCR wrote for display is NaN or infinite. Says no: the walls' α 5 % higher
//!   moves the analytic value outside 0.5 % of the unchanged run in every band; a NaN planted in
//!   a copy of the run is refused. Tutorial 1's absorption cannot tell a mean by area from one by
//!   face or by material, so `rooms/tutorial1_box_asymmetric.simpa` does (M7 review): each wrong
//!   way misses TCR by more than 0.5 % in every band.
//! - **Spec item 4 for TCR:** every TCR receiver band and aggregate carries the eight parameters,
//!   each refused `no_time_series`.
//! - **Evidence, run on purpose:** the level box over ten seeds (no level bias at the 0.01 dB
//!   scale), and tutorial 1 over twenty seeds (the Monte-Carlo estimate against the real spread).
//!
//! The committed fixtures under `tests/fixtures/results/` are written by
//! `cargo test -p simpa --test cli_results -- --ignored write_results_fixtures`.

mod support;

use std::path::{Path, PathBuf};

use serde_json::Value;
use simpa_core::params::air::{self, Atmosphere};
use simpa_core::params::room::{self, RtConstant, Surface};
use simpa_core::schema;
use support::*;

const SEATS: &str = "rooms/seats_box.simpa";
const LEVEL: &str = "rooms/level_box_20m.simpa";
const TUTORIAL: &str = "rooms/tutorial1_box_seeded.simpa";
const SEATS_SPPS: &str = "results/seats_spps";
const SEATS_TCR: &str = "results/seats_tcr";
const ENERGETIC: &str = "rooms/energetic_box.simpa";
const SOURCES2: &str = "rooms/sources2_box.simpa";
const ENERGETIC_SPPS: &str = "results/energetic_spps";
const SOURCES2_SPPS: &str = "results/sources2_spps";
const ASYMMETRIC: &str = "rooms/tutorial1_box_asymmetric.simpa";

fn results(folder: &Path, json: bool) -> Out {
    let mut args = vec!["results".to_string(), folder.display().to_string()];
    if json {
        args.push("--json".into());
    }
    simpa_run(&args)
}

/// `simpa run <project> --solver <s> --runs <root> --json`, which must be OK; its run folder.
fn run_ok(project: &Path, solver: &str, root: &Path, extra: &[&str]) -> PathBuf {
    solver_exe(if solver == "spps" {
        "spps.exe"
    } else {
        "classicalTheory.exe"
    });
    let mut args: Vec<String> = vec![
        "run".into(),
        project.display().to_string(),
        "--solver".into(),
        solver.into(),
        "--runs".into(),
        root.display().to_string(),
        "--json".into(),
    ];
    args.extend(extra.iter().map(|s| s.to_string()));
    let o = simpa_run(&args);
    let m = json(&o);
    assert_eq!(o.code, 0, "{solver} on {}: {o:#?}", project.display());
    assert_eq!(m["verdict"]["status"], "OK");
    run_dir(&m)
}

fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for e in std::fs::read_dir(from).unwrap() {
        let e = e.unwrap();
        let target = to.join(e.file_name());
        if e.file_type().unwrap().is_dir() {
            copy_dir(&e.path(), &target);
        } else {
            std::fs::copy(e.path(), &target).unwrap();
        }
    }
}

/// A copy of a committed results fixture in a fresh scratch folder.
fn fixture_copy(name: &str, label: &str) -> PathBuf {
    let to = scratch(label).join("run");
    copy_dir(&fixture(name), &to);
    to
}

fn refused_code(o: &Out) -> String {
    json(o)["refused"]["code"].as_str().unwrap().to_string()
}

/// The path of every null in a JSON value.
fn nulls(v: &Value, path: String, out: &mut Vec<String>) {
    match v {
        Value::Null => out.push(path),
        Value::Array(a) => a
            .iter()
            .enumerate()
            .for_each(|(i, x)| nulls(x, format!("{path}[{i}]"), out)),
        Value::Object(m) => m
            .iter()
            .for_each(|(k, x)| nulls(x, format!("{path}.{k}"), out)),
        _ => {}
    }
}

/// The keys `docs/formats/results-json.md` documents as nullable in a report, and in a refusal's
/// typed `error`.
const NULLABLE: [&str; 15] = [
    ".spps",
    ".tcr",
    ".mc_sd",
    ".band_hz",
    ".field",
    ".air_m_per_metre",
    ".onset",
    ".position_m",
    ".arrival_s",
    ".floor_db",
    ".lost_share",
    ".crossings",
    ".sd",
    ".with_tail",
    ".with_missing",
];

#[test]
fn every_null_in_a_report_is_at_a_nullable_key() {
    for name in [SEATS_SPPS, SEATS_TCR, ENERGETIC_SPPS, SOURCES2_SPPS] {
        let rep = json(&results(&fixture(name), true));
        let mut all = Vec::new();
        nulls(&rep, String::new(), &mut all);
        let unexpected: Vec<&String> = all
            .iter()
            .filter(|p| !NULLABLE.iter().any(|k| p.ends_with(k)))
            .collect();
        assert!(unexpected.is_empty(), "{name}: {unexpected:?}");
        // Top-level spps or tcr is null for the other solver.
        assert!(rep["spps"].is_null() != rep["tcr"].is_null());
    }
}

#[test]
#[ignore = "runs SPPS and TCR and writes tests/fixtures/results/; run on purpose"]
fn write_results_fixtures() {
    let root = scratch("write-results");
    for (room, solver, name) in [
        (SEATS, "spps", SEATS_SPPS),
        (SEATS, "tcr", SEATS_TCR),
        (ENERGETIC, "spps", ENERGETIC_SPPS),
        (SOURCES2, "spps", SOURCES2_SPPS),
    ] {
        let to = fixture(name);
        // This writer deletes nothing: to regenerate a fixture, remove its folder first.
        if to.exists() {
            println!("kept {}: it exists", to.display());
            continue;
        }
        let dir = run_ok(&fixture(room), solver, &root, &[]);
        copy_dir(&dir, &to);
        let o = results(&to, true);
        assert_eq!(o.code, 0, "{o:#?}");
        println!("wrote {} from {}", to.display(), dir.display());
    }
}

// --- exit 0, and gate (e) -----------------------------------------------------------------------

/// Each receiver's 500 Hz series as the JSON gives it, by label.
fn spps_series(report: &Value) -> Vec<(String, Vec<f64>)> {
    report["spps"]["point_receivers"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| {
            let e = r["bands"][0]["energy_pa2"]
                .as_array()
                .unwrap()
                .iter()
                .map(|x| x.as_f64().unwrap())
                .collect();
            (r["label"].as_str().unwrap().to_string(), e)
        })
        .collect()
}

/// The 500 Hz column of a receiver folder's `.recp`, read with the format reader alone.
fn recp_500(run: &Path, label: &str) -> Vec<f64> {
    let g = simpa_core::formats::gabe::read_file(
        &run.join(format!("solve/Punctual receivers/{label}/Sound level.recp")),
    )
    .unwrap();
    assert_eq!(g.columns[1].name(), b"500 Hz");
    g.columns[1]
        .floats()
        .unwrap()
        .iter()
        .map(|&x| f64::from(x))
        .collect()
}

#[test]
fn gate_e_seat_and_seat2_are_both_read_each_from_its_own_folder() {
    let run = fixture(SEATS_SPPS);
    let o = results(&run, true);
    assert_eq!(o.code, 0, "{o:#?}");
    let rep = json(&o);
    assert_eq!(rep["validated_by_bed"], false);
    let series = spps_series(&rep);
    let labels: Vec<&str> = series.iter().map(|(l, _)| l.as_str()).collect();
    assert_eq!(labels, ["Seat", "Seat2"]);
    // Compared as the f32 the solver wrote: serde_json's reader is not correctly rounded, so a
    // value can come back from the JSON text one f64 unit off.
    let f32s = |v: &[f64]| v.iter().map(|&x| x as f32).collect::<Vec<f32>>();
    for (label, e) in &series {
        assert_eq!(f32s(e), f32s(&recp_500(&run, label)), "{label}");
    }
    assert_ne!(
        series[0].1, series[1].1,
        "the two receivers' data are distinct"
    );

    // TCR: Seat.gabe and Seat2.gabe, each its own.
    let rep = json(&results(&fixture(SEATS_TCR), true));
    let rs = rep["tcr"]["point_receivers"].as_array().unwrap();
    let labels: Vec<&str> = rs.iter().map(|r| r["label"].as_str().unwrap()).collect();
    assert_eq!(labels, ["Seat", "Seat2"]);
    for r in rs {
        let label = r["label"].as_str().unwrap();
        let g = simpa_core::formats::gabe::read_file(
            &fixture(SEATS_TCR).join(format!("solve/Punctual receivers/{label}.gabe")),
        )
        .unwrap();
        let direct = g.column(b"Direct").unwrap().floats().unwrap()[0];
        assert_eq!(r["bands"][0]["direct_db"].as_f64().unwrap() as f32, direct);
    }
    assert_ne!(
        rs[0]["bands"][0]["direct_db"],
        rs[1]["bands"][0]["direct_db"]
    );
}

#[test]
fn gate_e_says_no_when_the_folders_swap_or_a_third_appears() {
    // Swapped: each label is read from the folder of that name, so the series swap too. A reader
    // that matched by prefix and read the first match would give Seat the same data both times.
    let run = fixture_copy(SEATS_SPPS, "seats-swapped");
    let pr = run.join("solve/Punctual receivers");
    std::fs::rename(pr.join("Seat"), pr.join("tmp")).unwrap();
    std::fs::rename(pr.join("Seat2"), pr.join("Seat")).unwrap();
    std::fs::rename(pr.join("tmp"), pr.join("Seat2")).unwrap();
    let original = spps_series(&json(&results(&fixture(SEATS_SPPS), true)));
    let swapped = spps_series(&json(&results(&run, true)));
    assert_eq!(swapped[0].1, original[1].1);
    assert_eq!(swapped[1].1, original[0].1);

    // A third folder whose name starts with Seat, which no label names: refused, not merged.
    let run = fixture_copy(SEATS_SPPS, "seats-third");
    copy_dir(
        &run.join("solve/Punctual receivers/Seat2"),
        &run.join("solve/Punctual receivers/Seat3"),
    );
    let o = results(&run, true);
    assert_eq!(o.code, 6, "{o:#?}");
    assert_eq!(refused_code(&o), "results_file_invalid");
    assert!(o.stderr.contains("Seat3"), "{}", o.stderr);
}

#[test]
fn text_mode_prints_the_unvalidated_banner_and_a_row_per_band_and_receiver() {
    let o = results(&fixture(SEATS_SPPS), false);
    assert_eq!(o.code, 0, "{o:#?}");
    assert!(o.stdout.starts_with("UNVALIDATED:"), "{}", o.stdout);
    for want in [
        "receiver \"Seat\"",
        "receiver \"Seat2\"",
        "  500 Hz",
        " 1000 Hz",
    ] {
        assert!(o.stdout.contains(want), "{want}: {}", o.stdout);
    }
    let o = results(&fixture(SEATS_TCR), false);
    assert_eq!(o.code, 0, "{o:#?}");
    assert!(o.stdout.contains("TR Sab s"), "{}", o.stdout);
}

const EIGHT: [&str; 8] = [
    "spl_db", "edt_s", "t20_s", "t30_s", "c50_db", "c80_db", "d50", "ts_s",
];

#[test]
fn every_band_of_the_committed_runs_has_all_eight_parameters_or_their_reasons() {
    let rep = json(&results(&fixture(SEATS_SPPS), true));
    let mut values = 0;
    for r in rep["spps"]["point_receivers"].as_array().unwrap() {
        let bands = r["bands"].as_array().unwrap();
        assert_eq!(bands.len(), 2);
        for b in bands.iter().chain([&r["aggregate"]]) {
            for n in EIGHT {
                let p = &b["parameters"][n];
                let ok = p["value"].is_f64() || p["not_evaluable"]["code"].is_string();
                assert!(ok, "{} {n}: {p}", r["label"]);
                values += usize::from(p["value"].is_f64());
                // No SPPS receiver is refused for having no series.
                assert_ne!(p["not_evaluable"]["error"]["why"]["why"], "no_time_series");
            }
        }
    }
    assert!(values > 0, "the SPPS run has values");
}

#[test]
fn a_tcr_receiver_has_all_eight_parameters_each_refused_for_having_no_series() {
    // Spec item 4: per receiver and band, each value or its NOT_EVALUABLE reason. TCR writes
    // steady-state levels and no series, so every one is refused, in the SPPS receivers' shape.
    let rep = json(&results(&fixture(SEATS_TCR), true));
    let rs = rep["tcr"]["point_receivers"].as_array().unwrap();
    assert_eq!(rs.len(), 2);
    for r in rs {
        let bands = r["bands"].as_array().unwrap();
        assert_eq!(bands.len(), 2);
        assert_eq!(r["aggregate"]["bands_hz"], serde_json::json!([]));
        for b in bands.iter().chain([&r["aggregate"]]) {
            for n in EIGHT {
                let p = &b["parameters"][n];
                assert!(p["value"].is_null(), "{} {n}: {p}", r["label"]);
                assert_eq!(p["not_evaluable"]["code"], "params_not_evaluable", "{p}");
                let why = &p["not_evaluable"]["error"]["why"];
                assert_eq!(why["why"], "no_time_series", "{p}");
                assert!(
                    why["detail"]
                        .as_str()
                        .unwrap()
                        .contains(r["file"].as_str().unwrap()),
                    "{p}"
                );
            }
        }
        // TCR's own levels stay beside them, as it wrote them.
        for b in bands {
            assert!(b["direct_db"].is_f64() && b["total_sabine_db"].is_f64());
            assert!(b["total_eyring_db"].is_f64());
        }
    }
}

// --- exit 2 --------------------------------------------------------------------------------------

#[test]
fn usage_errors_exit_2() {
    let dir = scratch("results-usage");
    let file = dir.join("not-a-folder.txt");
    std::fs::write(&file, "x").unwrap();
    let f = fixture(SEATS_SPPS).display().to_string();
    for args in [
        vec!["results".to_string()],
        vec!["results".into(), "--bogus".into(), f.clone()],
        vec!["results".into(), f.clone(), f.clone()],
        vec!["results".into(), file.display().to_string()],
        vec!["results".into(), dir.join("missing").display().to_string()],
        vec!["results".into(), "--schema".into(), f.clone()],
    ] {
        let o = simpa_run(&args);
        assert_eq!(o.code, 2, "{args:?}: {o:#?}");
        assert!(o.stderr.starts_with("simpa: "), "{args:?}: {}", o.stderr);
        assert!(o.stdout.is_empty(), "{args:?}: {}", o.stdout);
    }
}

// --- exit 5 --------------------------------------------------------------------------------------

#[test]
fn a_failed_run_is_refused_with_exit_5_and_its_reasons() {
    let root = scratch("results-failed");
    let o = simpa_run(&[
        "run-folder".to_string(),
        fixture("runs/spps_nomesh").display().to_string(),
        "--solver".into(),
        "spps".into(),
        "--runs".into(),
        root.display().to_string(),
        "--json".into(),
    ]);
    assert_eq!(o.code, 5, "{o:#?}");
    let m = json(&o);
    assert_eq!(m["verdict"]["status"], "FAIL");
    let r = results(&run_dir(&m), true);
    assert_eq!(r.code, 5, "{r:#?}");
    let v = json(&r);
    assert_eq!(v["refused"]["code"], "results_run_failed");
    assert_eq!(v["exit_code"], 5);
    assert_eq!(v["refused"]["reasons"][0]["code"], "mesh_invalid");
    assert!(
        r.stderr
            .starts_with("simpa: results refused: results_run_failed")
    );
    // Without --json: the reason on stderr, nothing on stdout.
    let r = results(&run_dir(&m), false);
    assert_eq!(r.code, 5);
    assert!(r.stdout.is_empty(), "{}", r.stdout);
}

#[test]
fn a_cancelled_run_is_refused_with_exit_5() {
    solver_exe("spps.exe");
    let root = scratch("results-cancelled");
    let o = simpa_run(&[
        "run".to_string(),
        fixture(SEATS).display().to_string(),
        "--solver".into(),
        "spps".into(),
        "--runs".into(),
        root.display().to_string(),
        "--cancel-after-ms".into(),
        "1".into(),
        "--json".into(),
    ]);
    assert_eq!(o.code, 130, "{o:#?}");
    let m = json(&o);
    assert_eq!(m["verdict"]["status"], "CANCELLED");
    let r = results(&run_dir(&m), true);
    assert_eq!(r.code, 5, "{r:#?}");
    assert_eq!(refused_code(&r), "results_run_cancelled");
}

// --- exit 6 --------------------------------------------------------------------------------------

/// The byte offset of row `row` of float column `col` in a GABE file whose decoded table is
/// `g`, as the current writer lays it out (`docs/formats/gabe.md`, "Layout"): a 20-byte header,
/// then per column a 280-byte header and its payload.
fn value_offset(g: &simpa_core::formats::gabe::Gabe, col: usize, row: usize) -> usize {
    use simpa_core::formats::gabe::ColumnData;
    let size = |c: &simpa_core::formats::gabe::Column| match &c.data {
        ColumnData::Float { values, .. } => 280 + 4 + 4 * values.len(),
        ColumnData::Int(v) => 280 + 1 + 4 * v.len(),
        ColumnData::ShortString(v) => 280 + 1 + 50 * v.len(),
    };
    assert!(g.columns[col].floats().is_some());
    20 + g.columns[..col].iter().map(size).sum::<usize>() + 280 + 4 + 4 * row
}

/// Overwrites one float of a GABE file in place.
fn plant(path: &Path, col: usize, row: usize, value: f32) {
    let g = simpa_core::formats::gabe::read_file(path).unwrap();
    let at = value_offset(&g, col, row);
    let mut bytes = std::fs::read(path).unwrap();
    assert_eq!(
        f32::from_le_bytes(bytes[at..at + 4].try_into().unwrap()).to_bits(),
        g.columns[col].floats().unwrap()[row].to_bits(),
        "the offset holds the value"
    );
    bytes[at..at + 4].copy_from_slice(&value.to_le_bytes());
    std::fs::write(path, bytes).unwrap();
}

#[test]
fn results_that_do_not_verify_are_refused_with_exit_6() {
    type Spoil = fn(&Path);
    let cases: [(&str, Spoil, &str); 8] = [
        (
            "no-manifest",
            |r| std::fs::remove_file(r.join("run.json")).unwrap(),
            "results_manifest_missing",
        ),
        (
            "bad-manifest",
            |r| std::fs::write(r.join("run.json"), "{\"manifest_version\": 1}").unwrap(),
            "results_manifest_invalid",
        ),
        (
            "config-edited",
            |r| {
                let p = r.join("solve/config.xml");
                let t = std::fs::read_to_string(&p).unwrap();
                std::fs::write(
                    &p,
                    t.replacen("nbparticules=\"2000\"", "nbparticules=\"2001\"", 1),
                )
                .unwrap();
            },
            "results_inputs_changed",
        ),
        (
            "recp-removed",
            |r| {
                std::fs::remove_file(r.join("solve/Punctual receivers/Seat/Sound level.recp"))
                    .unwrap()
            },
            "results_outputs_invalid",
        ),
        (
            "stats-emptied",
            |r| std::fs::write(r.join("solve/SPPS particle statistics.gabe"), b"").unwrap(),
            "results_outputs_invalid",
        ),
        (
            "gap-disagrees",
            |r| {
                // Column 5 is the first band's energy, which must equal the .recp's.
                let p = r.join("solve/Punctual receivers/Seat/Advanced sound level.gap");
                let g = simpa_core::formats::gabe::read_file(&p).unwrap();
                let v = g.columns[5].floats().unwrap()[20];
                plant(&p, 5, 20, v * 1.5 + 1e-12);
            },
            "results_file_invalid",
        ),
        (
            "nan-in-recp",
            |r| {
                let p = r.join("solve/Punctual receivers/Seat2/Sound level.recp");
                plant(&p, 2, 30, f32::NAN);
            },
            "results_value_invalid",
        ),
        (
            "extra-folder",
            |r| std::fs::create_dir(r.join("solve/Punctual receivers/Seat 3")).unwrap(),
            "results_file_invalid",
        ),
    ];
    for (label, spoil, want) in cases {
        let run = fixture_copy(SEATS_SPPS, label);
        // The copy itself reads.
        assert_eq!(results(&run, true).code, 0, "{label}: the clean copy");
        spoil(&run);
        let o = results(&run, true);
        assert_eq!(o.code, 6, "{label}: {o:#?}");
        assert_eq!(refused_code(&o), want, "{label}: {}", o.stderr);
        assert_eq!(json(&o)["exit_code"], 6);
        println!("{label}: {}", o.stderr.trim());
    }
}

// --- the schema ----------------------------------------------------------------------------------

#[test]
fn the_committed_schema_is_the_one_results_schema_prints() {
    let o = simpa_run(&["results", "--schema"]);
    assert_eq!(o.code, 0, "{o:#?}");
    let path = paths::repo_file("docs/formats/results-json.schema.json");
    let committed = std::fs::read_to_string(&path).unwrap_or_else(|e| {
        panic!(
            "{}: {e}; write it with `simpa results --schema > {}`",
            path.display(),
            path.display()
        )
    });
    assert_eq!(
        committed.replace("\r\n", "\n").trim_end(),
        o.stdout.replace("\r\n", "\n").trim_end(),
        "docs/formats/results-json.schema.json is stale"
    );
    // Every top-level key the report prints is a property of the schema's report.
    let schema = json(&o);
    let props = schema["report"]["properties"].as_object().unwrap();
    let rep = json(&results(&fixture(SEATS_SPPS), true));
    for k in rep.as_object().unwrap().keys() {
        assert!(props.contains_key(k), "{k} is not in the schema");
    }
}

// --- gate (c): level calibration -----------------------------------------------------------------

/// The source's level in each band as SPPS reads it from `config.xml` (an `f32`).
fn source_levels(run: &Path) -> Vec<(i32, f64)> {
    let text = std::fs::read_to_string(run.join("solve/config.xml")).unwrap();
    let doc = roxmltree::Document::parse(&text).unwrap();
    let src = doc
        .descendants()
        .find(|n| n.has_tag_name("source"))
        .unwrap();
    let mut v: Vec<(i32, f64)> = src
        .children()
        .filter(|n| n.has_tag_name("bfreq"))
        .map(|b| {
            let f: i32 = b.attribute("freq").unwrap().parse().unwrap();
            let db: f32 = b.attribute("db").unwrap().parse().unwrap();
            (f, f64::from(db))
        })
        .collect();
    v.sort_by_key(|x| x.0);
    v
}

/// One receiver and band of the level box.
struct LevelRow {
    label: String,
    freq_hz: i32,
    /// The receiver's distance from the source, m.
    r: f64,
    spl: f64,
    /// SPL's estimated Monte-Carlo standard deviation, dB.
    sd: f64,
    /// The `.recp`'s total, Pa².
    total: f64,
    /// The source's level in the band, as SPPS reads it.
    lw: f64,
}

/// The level box's rows, and its receiver radius.
fn level_rows(run: &Path) -> (Vec<LevelRow>, f64) {
    let o = results(run, true);
    assert_eq!(o.code, 0, "{o:#?}");
    let rep = json(&o);
    let s = &rep["spps"];
    let radius = s["receiver_radius_m"].as_f64().unwrap();
    let src: Vec<f64> = s["sources"][0]["position_m"]
        .as_array()
        .unwrap()
        .iter()
        .map(|x| x.as_f64().unwrap())
        .collect();
    let lw = source_levels(run);
    let mut out = Vec::new();
    for r in s["point_receivers"].as_array().unwrap() {
        let p: Vec<f64> = r["position_m"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_f64().unwrap())
            .collect();
        let dist =
            ((p[0] - src[0]).powi(2) + (p[1] - src[1]).powi(2) + (p[2] - src[2]).powi(2)).sqrt();
        for b in r["bands"].as_array().unwrap() {
            let f = b["freq_hz"].as_i64().unwrap() as i32;
            let p = &b["parameters"]["spl_db"];
            let spl = p["value"]
                .as_f64()
                .unwrap_or_else(|| panic!("{} {f} Hz: SPL {p}", r["label"]));
            out.push(LevelRow {
                label: r["label"].as_str().unwrap().to_string(),
                freq_hz: f,
                r: dist,
                spl,
                sd: p["mc_sd"].as_f64().unwrap(),
                total: b["total_pa2"].as_f64().unwrap(),
                lw: lw.iter().find(|x| x.0 == f).unwrap().1,
            });
        }
    }
    (out, radius)
}

/// Gate M7(c)'s predicate.
fn gate_c(spl: f64, lw: f64, r: f64) -> bool {
    (spl - (lw - 20.0 * r.log10() - 11.0)).abs() <= 0.5
}

/// `ρ·c` as SPPS computes it (`Masse_volumique_air.cpp:45-52`, `ρ = P·M/(R·T)` with M = 28.9644
/// kg/kmol and R = 8314.32 J/(K·kmol); `Celerite_du_son.cpp:46`, `c = 343.2·√(T/293.15)`).
fn solver_rho_c(temperature_c: f64, pressure_pa: f64) -> f64 {
    let k = temperature_c + 273.15;
    pressure_pa * 28.9644 / (8314.32 * k) * 343.2 * (k / 293.15).sqrt()
}

/// The mean of `1/d²` over a ball of radius `a` whose centre is `r > a` from the source: over a
/// shell of radius `ρ` it is `ln((r+ρ)/(r−ρ))/(2rρ)`, and over the ball
/// `3/(2r·a³)·[(a² − r²)/2·ln((r+a)/(r−a)) + r·a]`.
fn mean_inverse_square(r: f64, a: f64) -> f64 {
    3.0 / (2.0 * r * a.powi(3)) * ((a * a - r * r) / 2.0 * ((r + a) / (r - a)).ln() + r * a)
}

/// The free field's level at a receiver sphere of radius `a` centred `r` from an omni source of
/// `lw` dB, averaged over the sphere as SPPS's receiver does: `W·ρc·⟨1/d²⟩/(4π·p₀²)`.
fn exact_free_field(lw: f64, r: f64, a: f64, rho_c: f64) -> f64 {
    let w = 1e-12 * 10f64.powf(lw / 10.0);
    10.0 * (w * rho_c * mean_inverse_square(r, a) / (4.0 * std::f64::consts::PI * 4e-10)).log10()
}

/// The tighter level check (M7 review): the gate's reference assumes `ρc = 400` and a point
/// receiver, so it sits 0.11–0.30 dB below what SPPS should give and cannot see an error between
/// about −0.6 and +0.2 dB. Against the exact free field, with the noise each value carries: every
/// band within 4 of its `mc_sd`, and the mean of all bands, weighted by `1/mc_sd²`, within 4 of its
/// standard deviation. Returns that mean's difference, its standard deviation, and whether all
/// passes, with `offset` dB added to every SPL.
fn exact_check(rows: &[LevelRow], radius: f64, rho_c: f64, offset: f64) -> (f64, f64, bool) {
    let (mut sum, mut weights, mut bands_ok) = (0.0, 0.0, true);
    for x in rows {
        let d = x.spl + offset - exact_free_field(x.lw, x.r, radius, rho_c);
        bands_ok &= d.abs() <= 4.0 * x.sd;
        sum += d / (x.sd * x.sd);
        weights += 1.0 / (x.sd * x.sd);
    }
    let (mean, sd) = (sum / weights, weights.sqrt().recip());
    (mean, sd, bands_ok && mean.abs() <= 4.0 * sd)
}

#[test]
fn the_ball_average_of_the_inverse_square_is_its_closed_form() {
    // Against a midpoint sum over the ball in spherical shells, and the series 1 + a²/(5r²) + ….
    for (r, a) in [(2.0, 0.5), (4.0, 0.5), (1.0, 0.31)] {
        let (n, mut s) = (4000, 0.0);
        for k in 0..n {
            let rho = (k as f64 + 0.5) / n as f64 * a;
            let shell = ((r + rho) / (r - rho)).ln() / (2.0 * r * rho);
            s += 3.0 * rho * rho / a.powi(3) * shell * a / n as f64;
        }
        let closed = mean_inverse_square(r, a);
        assert!(
            (closed / s - 1.0).abs() < 1e-6,
            "r {r}, a {a}: {closed} vs {s}"
        );
        let series = (1.0 + a * a / (5.0 * r * r) + 3.0 * a.powi(4) / (35.0 * r.powi(4))) / (r * r);
        assert!((closed / series - 1.0).abs() < 1e-3, "{closed} vs {series}");
    }
    // 10·lg of it at 2 m and 4 m for R = 0.5 m, above 1/r²: +0.0554 and +0.0137 dB.
    let db = |r: f64| 10.0 * (mean_inverse_square(r, 0.5) * r * r).log10();
    assert!((db(2.0) - 0.0554).abs() < 5e-4 && (db(4.0) - 0.0137).abs() < 5e-4);
    // SPPS's ρc at 20 °C, 101 325 Pa.
    assert!((solver_rho_c(20.0, 101_325.0) - 413.25).abs() < 0.01);
}

#[test]
fn gate_c_level_calibration_and_the_offsets_it_catches() {
    let root = scratch("gate-c");
    let run = run_ok(&fixture(LEVEL), "spps", &root, &[]);
    let (rows, radius) = level_rows(&run);
    assert_eq!(rows.len(), 12, "2 receivers x 6 bands");
    assert_eq!(radius, 0.5);
    // What keeps the reverberant field out is the duration: no particle reaches a surface, so
    // none is absorbed and every one remains (results_rooms.rs, level_box).
    let rep = json(&results(&run, true));
    for b in rep["spps"]["particles"]["bands"].as_array().unwrap() {
        assert_eq!(b["absorbed_by_materials"], 0, "{b}");
        assert_eq!(b["remaining"], b["total"], "{b}");
    }
    // The run's own atmosphere.
    let text = std::fs::read_to_string(run.join("solve/config.xml")).unwrap();
    let doc = roxmltree::Document::parse(&text).unwrap();
    let atmo = doc
        .descendants()
        .find(|n| n.has_tag_name("condition_atmospherique"))
        .unwrap();
    let num = |k: &str| atmo.attribute(k).unwrap().parse::<f64>().unwrap();
    let rho_c = solver_rho_c(num("temperature"), num("pression"));
    let (mut worst_gate, mut worst_exact) = (0.0f64, 0.0f64);
    for x in &rows {
        let (spl, lw, r) = (x.spl, x.lw, x.r);
        assert!((lw - 100.0).abs() < 1e-9, "{} Hz: Lw {lw}", x.freq_hz);
        let want = lw - 20.0 * r.log10() - 11.0;
        let exact = exact_free_field(lw, r, radius, rho_c);
        println!(
            "{} {:>5} Hz r {r:.6} m: SPL {spl:.3} dB (mc_sd {:.3}), gate {want:.3} ({:+.3}), \
             exact {exact:.3} ({:+.3}, {:+.1} mc_sd)",
            x.label,
            x.freq_hz,
            x.sd,
            spl - want,
            spl - exact,
            (spl - exact) / x.sd
        );
        worst_gate = worst_gate.max((spl - want).abs());
        worst_exact = worst_exact.max((spl - exact).abs());
        assert!(gate_c(spl, lw, r), "{} {} Hz", x.label, x.freq_hz);
        // Says no: Night Mode's .gap level, energy over the intensity reference 1e-12.
        let night_mode = 10.0 * (x.total / 1e-12).log10();
        assert!(!gate_c(night_mode, lw, r), "{} Hz: {night_mode}", x.freq_hz);
        assert!((night_mode - spl - 26.0206).abs() < 1e-3);
        // Says no: 1 dB either way.
        assert!(!gate_c(spl + 1.0, lw, r) && !gate_c(spl - 1.0, lw, r));
    }
    println!("worst |SPL - gate| {worst_gate:.3} dB, worst |SPL - exact| {worst_exact:.3} dB");

    // The tighter check against the exact free field.
    let (mean, sd, ok) = exact_check(&rows, radius, rho_c, 0.0);
    println!(
        "exact free field (rho c {rho_c:.2}): weighted mean SPL - exact {mean:+.4} dB, standard \
         deviation {sd:.4} dB ({:+.1} of them): {}",
        mean / sd,
        if ok { "within" } else { "OUTSIDE" }
    );
    assert!(ok);
    // Says no: 0.1 dB either way, and the level SPPS would give with ρc = 400 (−0.141 dB), each
    // miss it. The smallest offsets it catches on this run, each way.
    let rho_400 = 10.0 * (400.0 / rho_c).log10();
    for offset in [0.1, -0.1, rho_400] {
        let (m, _, ok) = exact_check(&rows, radius, rho_c, offset);
        println!(
            "exact says no: SPL {offset:+.3} dB gives a mean of {m:+.4} dB: {}",
            if ok { "PASSES" } else { "caught" }
        );
        assert!(!ok, "{offset}");
    }
    let (up, down) = (4.0 * sd - mean, 4.0 * sd + mean);
    println!(
        "exact catches an offset above {up:+.3} dB or below {:+.3} dB",
        -down
    );
    assert!(up < 0.1 && down < 0.1);
}

// --- gate (d): TCR against the analytic values ---------------------------------------------------

/// How [`project_analytic`] combines the project's absorption: TCR's way, and the wrong ways
/// gate M7(d)'s say-no holds against it.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Rule {
    /// Every face's area with its group's material in the band: what TCR does.
    ByFace,
    /// The same with the walls' α scaled.
    WallsScaled(f64),
    /// One α for the whole surface, the mean over the faces, not weighted by area.
    FaceMean,
    /// One α for the whole surface, the mean over the materials in use.
    MaterialMean,
    /// The floor's and the walls' materials swapped.
    FloorWallsSwapped,
    /// Each band with its neighbour's absorption: the next band's, the previous for the last.
    NeighbourBand,
}

/// Sabine and Eyring per band from the project itself: faces and their groups' materials,
/// the geometry's volume, the solver's air term; the absorption combined by `rule`.
fn project_analytic(p: &schema::Project, rule: Rule) -> Vec<(u32, f64, f64)> {
    let v = &p.geometry.vertices;
    let pt = |i: u32| v[i as usize].to_array();
    let group_material = |name: &str| {
        let g = p.surface_groups.iter().find(|g| g.name == name).unwrap();
        p.material(g.material).unwrap()
    };
    let mut volume = 0.0;
    let mut faces = Vec::new();
    for f in &p.geometry.faces {
        let [a, b, c] = f.vertices.map(pt);
        let cr = [
            (b[1] - a[1]) * (c[2] - a[2]) - (b[2] - a[2]) * (c[1] - a[1]),
            (b[2] - a[2]) * (c[0] - a[0]) - (b[0] - a[0]) * (c[2] - a[2]),
            (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0]),
        ];
        let area = 0.5 * (cr[0] * cr[0] + cr[1] * cr[1] + cr[2] * cr[2]).sqrt();
        volume += (a[0] * (b[1] * c[2] - b[2] * c[1]) - a[1] * (b[0] * c[2] - b[2] * c[0])
            + a[2] * (b[0] * c[1] - b[1] * c[0]))
            / 6.0;
        let g = p.group(f.group).unwrap();
        let m = match (rule, g.name.as_str()) {
            (Rule::FloorWallsSwapped, "Floor") => group_material("Walls"),
            (Rule::FloorWallsSwapped, "Walls") => group_material("Floor"),
            _ => p.material(g.material).unwrap(),
        };
        let scale = match rule {
            Rule::WallsScaled(s) if g.name == "Walls" => s,
            _ => 1.0,
        };
        faces.push((area, m, scale));
    }
    let mut used: Vec<&schema::Material> = Vec::new();
    for (_, m, _) in &faces {
        if !used.iter().any(|u| u.id == m.id) {
            used.push(m);
        }
    }
    let total: f64 = faces.iter().map(|f| f.0).sum();
    let n = p.bands.frequencies_hz.len();
    let env = &p.environment;
    let atm = Atmosphere {
        temperature_c: env.temperature_c.get(),
        relative_humidity_percent: env.relative_humidity_percent.get(),
        pressure_pa: env.pressure_pa.get(),
    };
    p.bands
        .frequencies_hz
        .iter()
        .enumerate()
        .map(|(i, &f)| {
            let band = match rule {
                Rule::NeighbourBand if i + 1 < n => i + 1,
                Rule::NeighbourBand => i - 1,
                _ => i,
            };
            let s: Vec<Surface> = match rule {
                Rule::FaceMean => vec![Surface {
                    area_m2: total,
                    absorption: faces
                        .iter()
                        .map(|(_, m, _)| m.absorption[band].get())
                        .sum::<f64>()
                        / faces.len() as f64,
                }],
                Rule::MaterialMean => vec![Surface {
                    area_m2: total,
                    absorption: used.iter().map(|m| m.absorption[band].get()).sum::<f64>()
                        / used.len() as f64,
                }],
                _ => faces
                    .iter()
                    .map(|(area, m, scale)| Surface {
                        area_m2: *area,
                        absorption: m.absorption[band].get() * scale,
                    })
                    .collect(),
            };
            let m = p
                .solvers
                .tcr
                .air_absorption
                .then(|| air::solver_air_absorption_per_m(f64::from(f), &atm).unwrap());
            (
                f,
                room::sabine_rt(volume.abs(), &s, m, RtConstant::Tcr).unwrap(),
                room::eyring_rt(volume.abs(), &s, m, RtConstant::Tcr).unwrap(),
            )
        })
        .collect()
}

/// A TCR run's report, which must read; its analytic values computed from the run's inputs.
fn tcr_report(run: &Path) -> Value {
    let o = results(run, true);
    assert_eq!(o.code, 0, "{o:#?}");
    let rep = json(&o);
    assert_eq!(
        rep["tcr"]["analytic"]["status"], "computed",
        "{}",
        rep["tcr"]["analytic"]
    );
    rep
}

/// Per band `(freq, Sabine, Eyring)`: TCR's times from a report, or with `analytic` the times
/// core::params computed on the run's inputs.
fn report_times(rep: &Value, analytic: bool) -> Vec<(u32, f64, f64)> {
    let t = &rep["tcr"];
    t["bands"]
        .as_array()
        .unwrap()
        .iter()
        .enumerate()
        .map(|(i, b)| {
            let f = b["freq_hz"].as_i64().unwrap() as u32;
            if analytic {
                let a = &t["analytic"]["bands"][i];
                assert_eq!(a["freq_hz"], b["freq_hz"]);
                (
                    f,
                    a["sabine_s"]["value"].as_f64().unwrap(),
                    a["eyring_s"]["value"].as_f64().unwrap(),
                )
            } else {
                (
                    f,
                    b["sabine"]["reverberation_time_s"].as_f64().unwrap(),
                    b["eyring"]["reverberation_time_s"].as_f64().unwrap(),
                )
            }
        })
        .collect()
}

/// Against TCR's times per band: the largest relative difference over bands and theories; the
/// bands in which both theories are within `rel`; the bands in which both are outside it; and the
/// smallest relative difference.
fn compare(
    tcr: &[(u32, f64, f64)],
    ours: &[(u32, f64, f64)],
    rel: f64,
) -> (f64, usize, usize, f64) {
    assert_eq!(tcr.len(), ours.len());
    let (mut worst, mut inside, mut outside, mut closest) = (0.0f64, 0, 0, f64::MAX);
    for (t, o) in tcr.iter().zip(ours) {
        assert_eq!(t.0, o.0, "bands");
        let (ds, de) = ((t.1 / o.1 - 1.0).abs(), (t.2 / o.2 - 1.0).abs());
        worst = worst.max(ds).max(de);
        closest = closest.min(ds).min(de);
        if ds <= rel && de <= rel {
            inside += 1;
        }
        if ds > rel && de > rel {
            outside += 1;
        }
    }
    (worst, inside, outside, closest)
}

#[test]
fn gate_d_tcr_equals_the_analytic_sabine_and_eyring_and_says_no() {
    let root = scratch("gate-d");
    let run = run_ok(&fixture(TUTORIAL), "tcr", &root, &[]);
    let rep = tcr_report(&run);
    let p = schema::load(&fixture(TUTORIAL)).unwrap();
    let tcr = report_times(&rep, false);
    let n = tcr.len();
    assert_eq!(n, 27);
    let (w1, in1, _, _) = compare(&tcr, &project_analytic(&p, Rule::ByFace), 0.005);
    let (w2, in2, _, _) = compare(&tcr, &report_times(&rep, true), 0.005);
    println!(
        "TCR against ours from the project: worst {w1:.2e}, within 0.5 % in {in1} of {n} bands; \
         from the run's inputs: worst {w2:.2e}, within in {in2} of {n}"
    );
    assert_eq!((in1, in2), (n, n));
    // Says no: the walls 5 % more absorbing, against the unchanged run.
    let (_, _, moved_out, _) = compare(&tcr, &project_analytic(&p, Rule::WallsScaled(1.05)), 0.005);
    println!("walls +5 % outside 0.5 % in {moved_out} of {n} bands");
    assert_eq!(moved_out, n);
    // What this room cannot say no to (M7 review): its floor's 0.1 and ceiling's 0.3 sit on equal
    // areas and average to the walls' 0.2, so a mean over faces or over materials gives the same
    // times as TCR. `gate_d_the_asymmetric_room_...` is the room that tells them apart.
    for rule in [Rule::FaceMean, Rule::MaterialMean] {
        let (w, inside, _, _) = compare(&tcr, &project_analytic(&p, rule), 0.005);
        println!(
            "tutorial 1 cannot tell {rule:?} apart: within 0.5 % in {inside} of {n} bands (worst \
             {w:.2e})"
        );
        assert_eq!(inside, n);
    }

    // No NaN or infinity in any value TCR wrote for display: every row of every table and every
    // .csbin value, read with the format readers alone. The one exception is Main results'
    // Global row for the areas and times, NaN by design (ctr/input_output/reportmanager.cpp:208);
    // its levels, and every receiver table's Global row, an energetic sum, are scanned.
    let mut values = 0usize;
    for e in walk(&run.join("solve")) {
        let name = e.to_string_lossy().to_string();
        if name.ends_with(".gabe") {
            let g = simpa_core::formats::gabe::read_file(&e).unwrap();
            let labels = g.row_labels().unwrap().to_vec();
            let main = name.ends_with("Main results.gabe");
            for c in &g.columns[1..] {
                let by_design =
                    main && (c.name().starts_with(b"A_") || c.name().starts_with(b"TR_"));
                for (k, v) in c.floats().unwrap().iter().enumerate() {
                    if labels[k] == b"Global" && by_design {
                        assert!(v.is_nan(), "{name} {:?}: Global is NaN by design", c.name());
                        continue;
                    }
                    values += 1;
                    assert!(v.is_finite(), "{name} {:?} row {k}", c.name());
                }
            }
        } else if name.ends_with(".csbin") {
            let c = simpa_core::formats::csbin::read_file(&e).unwrap();
            for r in &c.receivers {
                for f in &r.faces {
                    for v in f.records.iter() {
                        values += 1;
                        assert!(v.energy.is_finite(), "{name}");
                    }
                }
            }
        }
    }
    println!("{values} displayed values, all finite");
    // serde_json prints a NaN as null, so a JSON number is always finite and says nothing; the
    // report is walked for non-finite numbers before it is printed (report::checked_report).
    // What the output can show: every null is at a key documented as nullable.
    let mut all = Vec::new();
    nulls(&rep, String::new(), &mut all);
    let unexpected: Vec<&String> = all
        .iter()
        .filter(|p| !NULLABLE.iter().any(|k| p.ends_with(k)))
        .collect();
    assert!(unexpected.is_empty(), "{unexpected:?}");

    // Says no: a NaN planted in a copy's Main results is refused (the verdict's nonfinite_result,
    // judged again).
    let copy = scratch("gate-d-nan").join("run");
    copy_dir(&run, &copy);
    // Column 2 is TR_Sabine; row 3 is the 100 Hz band.
    plant(&copy.join("solve/Main results.gabe"), 2, 3, f32::NAN);
    let o = results(&copy, true);
    assert_eq!(o.code, 6, "{o:#?}");
    assert_eq!(refused_code(&o), "results_outputs_invalid");
    assert!(o.stderr.contains("nonfinite_result"), "{}", o.stderr);
}

#[test]
fn gate_d_the_asymmetric_room_tells_apart_the_ways_of_combining_absorption() {
    // The room: tutorial 1's box, the floor's α rising from 0.15 to 0.80 over the bands, the walls
    // 0.1, the ceiling 0.3 (results_rooms.rs, asymmetric_box, which holds the recipe's margins).
    let root = scratch("gate-d-asymmetric");
    let run = run_ok(&fixture(ASYMMETRIC), "tcr", &root, &[]);
    let rep = tcr_report(&run);
    let p = schema::load(&fixture(ASYMMETRIC)).unwrap();
    let tcr = report_times(&rep, false);
    let n = tcr.len();
    assert_eq!(n, 27);
    let (w1, in1, _, _) = compare(&tcr, &project_analytic(&p, Rule::ByFace), 0.005);
    let (w2, in2, _, _) = compare(&tcr, &report_times(&rep, true), 0.005);
    println!(
        "asymmetric: TCR against ours from the project: worst {w1:.2e}, within 0.5 % in {in1} of \
         {n} bands; from the run's inputs: worst {w2:.2e}, within in {in2} of {n}"
    );
    assert_eq!((in1, in2), (n, n));
    // Says no: each wrong way of combining the absorption is outside 0.5 % of TCR in every band,
    // in both theories.
    for rule in [
        Rule::FaceMean,
        Rule::MaterialMean,
        Rule::FloorWallsSwapped,
        Rule::NeighbourBand,
    ] {
        let (_, _, out, closest) = compare(&tcr, &project_analytic(&p, rule), 0.005);
        println!(
            "asymmetric says no: {rule:?} outside 0.5 % in {out} of {n} bands, closest {:.2} %",
            100.0 * closest
        );
        assert_eq!(out, n, "{rule:?}");
    }
    // The absorption areas TCR wrote are the areas by face, band by band: 60·α_f + 27.6 m².
    for (i, b) in rep["tcr"]["bands"].as_array().unwrap().iter().enumerate() {
        let want = 60.0
            * p.materials
                .iter()
                .find(|m| m.name == "Rising absorption")
                .unwrap()
                .absorption[i]
                .get()
            + 27.6;
        let a = b["sabine"]["absorption_area_m2"].as_f64().unwrap();
        assert!(
            (a / want - 1.0).abs() < 1e-5,
            "{} Hz: A {a}, want {want}",
            b["freq_hz"]
        );
    }
}

fn walk(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for e in std::fs::read_dir(dir).unwrap() {
        let p = e.unwrap().path();
        if p.is_dir() {
            out.extend(walk(&p));
        } else {
            out.push(p);
        }
    }
    out
}

// --- tutorial 1 beside upstream's own numbers (information, not a gate) --------------------------

/// A cell: the value to `digits`, or `NE`.
fn show(v: Option<f64>, digits: usize) -> String {
    v.map_or("NE".to_string(), |x| format!("{x:.digits$}"))
}

fn param(p: &Value, name: &str) -> Option<f64> {
    p[name]["value"].as_f64()
}

/// A decay time with its Monte-Carlo standard deviation, or why it was refused (with the
/// standard deviation, relative, when it was refused for its noise).
fn why(p: &Value, name: &str) -> String {
    let q = &p[name];
    if let Some(v) = q["value"].as_f64() {
        return format!(
            "{v:.2}±{:.1}%",
            100.0 * q["mc_sd"].as_f64().unwrap_or(f64::NAN) / v
        );
    }
    let w = &q["not_evaluable"]["error"]["why"];
    match w["why"].as_str() {
        Some("monte_carlo_noise") => format!(
            "mc {:.1}%",
            100.0 * w["sd"].as_f64().unwrap_or(f64::NAN) / w["value"].as_f64().unwrap_or(f64::NAN)
        ),
        Some(other) => other.to_string(),
        None => q["not_evaluable"]["code"]
            .as_str()
            .unwrap_or("?")
            .to_string(),
    }
}

#[test]
#[ignore = "information for the M7 report, not a gate: runs SPPS and TCR on tutorial 1 and prints \
            our parameters beside upstream's stored 2019 ones"]
fn tutorial1_parameters_beside_upstreams() {
    use simpa_core::formats::gabe;
    use simpa_core::geometry::import::zip::Archive;
    use simpa_core::params::EnergySeries;
    use simpa_core::params::decay::Arrival;
    use simpa_core::params::noise::NoiseModel;
    use simpa_core::results::report;

    let proj = paths::upstream_file("src/isimpa/resources/doc/tutorial/tutorial 1/tutorial_1.proj");
    let bytes = std::fs::read(&proj).unwrap();
    let z = Archive::parse(&bytes).unwrap();
    let spps = "instance2/report/SPPS/2019-06-07_11h58m41s/";
    let read = |n: &str| gabe::read(&z.read(&format!("{spps}{n}")).unwrap()).unwrap();
    let up = read("Punctual receivers/Receiver 1/Acoustic parameters.gabe");
    let recp = read("Punctual receivers/Receiver 1/Sound level.recp");
    // The .gap's column 2, the source's power times ρc per band, for the noise model: the 2019
    // run's config.xml has 150,000 particles and a receiver radius of 0.31 m (f32).
    let gap = read("Punctual receivers/Receiver 1/Advanced sound level.gap");
    let power_rho_c = gap.columns[2].floats().unwrap().to_vec();
    let radius = f64::from(0.31f32);
    let stats = simpa_core::run::stats::from_gabe(&read("SPPS particle statistics.gabe")).unwrap();
    let tcr2019 = gabe::read(
        &z.read(
            "instance2/report/Classical theory of reverberation/2019-06-07_11h57m58s/Main results.gabe",
        )
        .unwrap(),
    )
    .unwrap();
    let up_row = |col: &str, band: &str| -> Option<f64> {
        let i = up.row_index(band.as_bytes())?;
        up.column(col.as_bytes())?
            .floats()?
            .get(i)
            .map(|&v| f64::from(v))
    };

    // Ours on upstream's own .recp: its config's dt (f32), Receiver 1 at (1, 1, 1.8) and the
    // source at (3, 5, 1.8), c = 343.2 m/s at 20 °C, and each band's series complete where the
    // 2019 statistics count no particle remaining (random mode).
    let dt = f64::from(0.01f32);
    let c = f64::from(simpa_core::results::spps::solver_speed_of_sound(20.0));
    let arrival_s = (2.0f64.powi(2) + 4.0f64.powi(2)).sqrt() / c;
    let arrival = Arrival::at(arrival_s);
    let room = read("Total energy.recp");
    let mut on_2019 = Vec::new();
    for (i, col) in recp.columns[1..].iter().enumerate() {
        let f = simpa_core::run::stats::band_label(col.name()).unwrap();
        let e: Vec<f64> = col
            .floats()
            .unwrap()
            .iter()
            .map(|&x| f64::from(x))
            .collect();
        let band = stats.bands.iter().find(|b| b.freq_hz == f).unwrap();
        let s = if band.remaining == 0 {
            EnergySeries::complete(dt, e)
        } else {
            EnergySeries::new(dt, e)
        };
        // The lost particles' share, as core::results gives it: lost / (N·f), f the share of the
        // emitted energy alive at the end of the arrival's step (the room table over the .gap's
        // source power).
        let s = match (s, band.lost()) {
            (Ok(s), 0) => Ok(s),
            (Ok(s), lost) => {
                let bin = (arrival_s / dt).floor() as usize;
                let alive = f64::from(room.columns[i + 1].floats().unwrap()[bin])
                    / f64::from(power_rho_c[i]);
                s.with_lost_share(lost as f64 / (150_000.0 * alive))
            }
            (Err(e), _) => Err(e),
        };
        let model = NoiseModel::crossings(
            f64::from(power_rho_c[i]) / (150_000.0 * std::f64::consts::PI * radius * radius),
        )
        .unwrap();
        let (p, _) = report::parameters(&s, arrival, &model);
        on_2019.push((f, serde_json::to_value(&p).unwrap()));
    }

    // Ours on our own run of the same project (150,000 particles, unseeded, as in 2019).
    let root = scratch("tutorial1-compare");
    let run = run_ok(&fixture("rooms/tutorial1_box.simpa"), "spps", &root, &[]);
    let ours = json(&results(&run, true));
    let r1 = ours["spps"]["point_receivers"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["label"] == "Receiver 1")
        .unwrap()
        .clone();
    let tcr_run = run_ok(&fixture("rooms/tutorial1_box.simpa"), "tcr", &root, &[]);
    let tcr = json(&results(&tcr_run, true));

    println!(
        "Receiver 1. U: upstream's GUI, 2019 (Acoustic parameters.gabe). A: ours on the same 2019 \
         .recp. B: ours on our run {}.",
        run.display()
    );
    println!(
        "{:>6} | {:^20} | {:^17} | {:^17} | {:^17} | {:^17} | {:^20} | {:^17}",
        "band",
        "SPL dB U/A/B",
        "EDT s U/A/B",
        "T30 s U/A/B",
        "C50 dB U/A/B",
        "C80 dB U/A/B",
        "D50 % U/A/B",
        "Ts ms U/A/B"
    );
    let trio = |u: Option<f64>, x: Option<f64>, y: Option<f64>, d: usize, s: f64| {
        format!(
            "{}/{}/{}",
            show(u, d),
            show(x.map(|v| v * s), d),
            show(y.map(|v| v * s), d)
        )
    };
    for (i, (f, a)) in on_2019.iter().enumerate() {
        let band = format!("{f} Hz");
        let b = &r1["bands"][i]["parameters"];
        assert_eq!(r1["bands"][i]["freq_hz"], *f);
        println!(
            "{:>6} | {:^20} | {:^17} | {:^17} | {:^17} | {:^17} | {:^20} | {:^17}",
            f,
            trio(
                up_row("Sound level (dB)", &band),
                param(a, "spl_db"),
                param(b, "spl_db"),
                1,
                1.0
            ),
            trio(
                up_row("EDT (s)", &band),
                param(a, "edt_s"),
                param(b, "edt_s"),
                2,
                1.0
            ),
            trio(
                up_row("RT-30 (s)", &band),
                param(a, "t30_s"),
                param(b, "t30_s"),
                2,
                1.0
            ),
            trio(
                up_row("C-50 (dB)", &band),
                param(a, "c50_db"),
                param(b, "c50_db"),
                1,
                1.0
            ),
            trio(
                up_row("C-80 (dB)", &band),
                param(a, "c80_db"),
                param(b, "c80_db"),
                1,
                1.0
            ),
            trio(
                up_row("D-50 (%)", &band),
                param(a, "d50"),
                param(b, "d50"),
                1,
                100.0
            ),
            trio(
                up_row("Ts (ms)", &band),
                param(a, "ts_s"),
                param(b, "ts_s"),
                1,
                1000.0
            ),
        );
    }
    println!(
        "\nThe decay times behind the table: each value with its Monte-Carlo standard deviation, \
         or why it was refused (mc: its noise, with the standard deviation)."
    );
    println!(
        "{:>6} | {:^16} | {:^16} | {:^16} | {:^16} | {:^16} | {:^16}",
        "band", "EDT A", "EDT B", "T20 A", "T20 B", "T30 A", "T30 B"
    );
    for (i, (f, a)) in on_2019.iter().enumerate() {
        let b = &r1["bands"][i]["parameters"];
        println!(
            "{:>6} | {:^16} | {:^16} | {:^16} | {:^16} | {:^16} | {:^16}",
            f,
            why(a, "edt_s"),
            why(b, "edt_s"),
            why(a, "t20_s"),
            why(b, "t20_s"),
            why(a, "t30_s"),
            why(b, "t30_s")
        );
    }
    println!(
        "\nTCR reverberation times, s. 2019: upstream's stored run. Ours: our run. Analytic: \
         core::params on our run's inputs."
    );
    println!(
        "{:>6} | {:^26} | {:^26}",
        "band", "Sabine 2019/ours/analytic", "Eyring 2019/ours/analytic"
    );
    let col = |name: &str, i: usize| {
        f64::from(tcr2019.column(name.as_bytes()).unwrap().floats().unwrap()[i])
    };
    for (i, b) in tcr["tcr"]["bands"].as_array().unwrap().iter().enumerate() {
        let a = &tcr["tcr"]["analytic"]["bands"][i];
        println!(
            "{:>6} | {:>8.4}/{:>8.4}/{:>8.4} | {:>8.4}/{:>8.4}/{:>8.4}",
            b["freq_hz"],
            col("TR_Sabine", i),
            b["sabine"]["reverberation_time_s"].as_f64().unwrap(),
            a["sabine_s"]["value"].as_f64().unwrap(),
            col("TR_Eyring", i),
            b["eyring"]["reverberation_time_s"].as_f64().unwrap(),
            a["eyring_s"]["value"].as_f64().unwrap(),
        );
    }
}

// --- evidence for the noise model and the level calibration (run on purpose) --------------------

/// `project` with SPPS seeded `seed` and changed by `edit`, saved into `dir`.
fn seeded(project: &str, seed: u32, dir: &Path, edit: impl Fn(&mut schema::Project)) -> PathBuf {
    let mut p = schema::load(&fixture(project)).unwrap();
    p.solvers.spps.random_seed = seed;
    edit(&mut p);
    std::fs::create_dir_all(dir).unwrap();
    let path = dir.join(format!("seed{seed}.simpa"));
    schema::save(&p, &path).unwrap();
    path
}

/// The mean and the sample standard deviation.
fn mean_sd(x: &[f64]) -> (f64, f64) {
    let n = x.len() as f64;
    let m = x.iter().sum::<f64>() / n;
    let v = x.iter().map(|v| (v - m).powi(2)).sum::<f64>() / (n - 1.0);
    (m, v.sqrt())
}

#[test]
#[ignore = "evidence, not a gate: runs the level box over ten SPPS seeds; run on purpose"]
fn level_box_over_ten_seeds() {
    let root = scratch("level-seeds");
    let mut runs: Vec<Vec<LevelRow>> = Vec::new();
    let mut radius = 0.0;
    for seed in 1..=10u32 {
        let project = seeded(LEVEL, seed, &root, |_| {});
        let run = run_ok(&project, "spps", &root.join("runs"), &[]);
        let (rows, r) = level_rows(&run);
        radius = r;
        runs.push(rows);
    }
    let rho_c = solver_rho_c(20.0, 101_325.0);
    let cells = runs[0].len();
    let mut ratios = Vec::new();
    let (mut spread2, mut model2) = (0.0, 0.0);
    for c in 0..cells {
        let x = &runs[0][c];
        let spl: Vec<f64> = runs.iter().map(|r| r[c].spl).collect();
        let model = (runs.iter().map(|r| r[c].sd.powi(2)).sum::<f64>() / runs.len() as f64).sqrt();
        let (m, sd) = mean_sd(&spl);
        let exact = exact_free_field(x.lw, x.r, radius, rho_c);
        println!(
            "{} {:>5} Hz: mean SPL {m:.4} dB, exact {exact:.4} ({:+.4}); spread over the seeds \
             {sd:.4} dB, mc_sd {model:.4} dB, ratio {:.2}",
            x.label,
            x.freq_hz,
            m - exact,
            sd / model
        );
        ratios.push(sd / model);
        spread2 += sd * sd;
        model2 += model * model;
    }
    // Each seed's mean difference from the exact free field, weighted by 1/mc_sd², and their
    // spread over the seeds: a standard error that does not lean on mc_sd.
    let per_seed: Vec<f64> = runs
        .iter()
        .map(|rows| exact_check(rows, radius, rho_c, 0.0).0)
        .collect();
    let (bias, spread) = mean_sd(&per_seed);
    let se = spread / (per_seed.len() as f64).sqrt();
    let pooled = (spread2 / model2).sqrt();
    ratios.sort_by(f64::total_cmp);
    println!(
        "over {} seeds: each seed's weighted mean SPL - exact {per_seed:.4?}; their mean \
         {bias:+.4} dB, standard error {se:.4} dB ({:+.1} of them)",
        runs.len(),
        bias / se
    );
    println!(
        "spread over the seeds against mc_sd, {cells} receiver-bands: pooled {pooled:.2}, median \
         {:.2}, from {:.2} to {:.2}",
        ratios[cells / 2],
        ratios[0],
        ratios[cells - 1]
    );
    assert!(bias.abs() <= 4.0 * se, "a level bias of {bias:+.4} dB");
    assert!((0.67..=1.5).contains(&pooled), "mc_sd is off by {pooled}");
}

/// A value and its standard deviation: a value with its `mc_sd`, or one refused for its noise with
/// the standard deviation it had. `None` for any other refusal.
fn value_and_sd(p: &Value) -> Option<(f64, f64)> {
    if let (Some(v), Some(s)) = (p["value"].as_f64(), p["mc_sd"].as_f64()) {
        return Some((v, s));
    }
    let why = &p["not_evaluable"]["error"]["why"];
    if why["why"] == "monte_carlo_noise" {
        return Some((why["value"].as_f64()?, why["sd"].as_f64()?));
    }
    None
}

#[test]
#[ignore = "evidence, not a gate: runs tutorial 1 over twenty SPPS seeds on six octave bands; run \
            on purpose"]
fn noise_estimate_against_the_spread_of_twenty_seeds() {
    // Tutorial 1 at its 150,000 particles, where T30 is mostly out of reach, and at ten times
    // that for T30.
    seed_spread(150_000);
    seed_spread(1_500_000);
}

/// Tutorial 1 at `particles`, seeded 1 to 20, on the octave bands 125 Hz to 4 kHz, without its
/// surface receiver (which the point receivers do not see): per receiver, band and quantity with a
/// value in every seed, the spread of the values over the seeds against the estimated `mc_sd`.
fn seed_spread(particles: u32) {
    let root = scratch("tutorial1-seeds");
    let keep = [125u32, 250, 500, 1000, 2000, 4000];
    let mut reports = Vec::new();
    let clock = std::time::Instant::now();
    for seed in 1..=20u32 {
        let project = seeded("rooms/tutorial1_box.simpa", seed, &root, |p| {
            let all = p.bands.frequencies_hz.clone();
            p.solvers.spps.bands_computed = all.iter().map(|f| keep.contains(f)).collect();
            p.solvers.spps.particles_per_source = particles;
            p.surface_receivers.clear();
            p.solvers.spps.save_surface_intersections = false;
        });
        let run = run_ok(&project, "spps", &root.join("runs"), &[]);
        let o = results(&run, true);
        assert_eq!(o.code, 0, "{o:#?}");
        reports.push(json(&o));
        println!("seed {seed}: {:.0} s", clock.elapsed().as_secs_f64());
    }
    let n = reports.len();
    println!("\n{particles} particles:");
    println!(
        "{:<11} {:>6} {:<7} {:>10} {:>10} {:>10} {:>6}",
        "receiver", "band", "value", "mean", "spread", "model", "ratio"
    );
    let mut summary = Vec::new();
    for q in EIGHT {
        let (mut ratios, mut spread2, mut model2, mut short) = (Vec::new(), 0.0, 0.0, 0);
        for (ri, r) in reports[0]["spps"]["point_receivers"]
            .as_array()
            .unwrap()
            .iter()
            .enumerate()
        {
            for (bi, b) in r["bands"].as_array().unwrap().iter().enumerate() {
                let got: Vec<(f64, f64)> = reports
                    .iter()
                    .filter_map(|rep| {
                        value_and_sd(
                            &rep["spps"]["point_receivers"][ri]["bands"][bi]["parameters"][q],
                        )
                    })
                    .collect();
                // Only cells where every seed gives a value, so no seed is selected away.
                if got.len() < n {
                    short += 1;
                    continue;
                }
                let values: Vec<f64> = got.iter().map(|g| g.0).collect();
                let (m, spread) = mean_sd(&values);
                let model = (got.iter().map(|g| g.1 * g.1).sum::<f64>() / n as f64).sqrt();
                println!(
                    "{:<11} {:>6} {q:<7} {m:>10.4} {spread:>10.4} {model:>10.4} {:>6.2}",
                    r["label"].as_str().unwrap(),
                    b["freq_hz"],
                    spread / model
                );
                ratios.push(spread / model);
                spread2 += spread * spread;
                model2 += model * model;
            }
        }
        ratios.sort_by(f64::total_cmp);
        summary.push((q, ratios, (spread2 / model2).sqrt(), short));
    }
    println!(
        "\n{particles} particles: spread over {n} seeds against the estimated mc_sd, per quantity:"
    );
    for (q, ratios, pooled, short) in &summary {
        if ratios.is_empty() {
            println!("{q:<7}: no receiver-band with a value in every seed ({short} short)");
            continue;
        }
        println!(
            "{q:<7}: {} receiver-bands, pooled {pooled:.2}, median {:.2}, from {:.2} to {:.2}; \
             {short} left out (a seed without a value)",
            ratios.len(),
            ratios[ratios.len() / 2],
            ratios[0],
            ratios[ratios.len() - 1]
        );
    }
    for (q, ratios, pooled, _) in &summary {
        if !ratios.is_empty() {
            assert!((0.67..=1.5).contains(pooled), "{q}: pooled ratio {pooled}");
        }
    }
}
