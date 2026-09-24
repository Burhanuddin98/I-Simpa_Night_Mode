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
//!   band.
//! - **Gate M7(d):** tutorial 1's box through TCR: per band, TCR's Sabine and Eyring times equal
//!   `core::params`' within 0.5 %, computed both from the project and from the run's own inputs,
//!   and no value TCR wrote for display is NaN or infinite. Says no: the walls' α 5 % higher
//!   moves the analytic value outside 0.5 % of the unchanged run in every band; a NaN planted in
//!   a copy of the run is refused.
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

#[test]
fn every_band_of_the_committed_runs_has_all_eight_parameters_or_their_reasons() {
    let rep = json(&results(&fixture(SEATS_SPPS), true));
    let names = [
        "spl_db", "edt_s", "t20_s", "t30_s", "c50_db", "c80_db", "d50", "ts_s",
    ];
    for r in rep["spps"]["point_receivers"].as_array().unwrap() {
        let bands = r["bands"].as_array().unwrap();
        assert_eq!(bands.len(), 2);
        for b in bands.iter().chain([&r["aggregate"]]) {
            for n in names {
                let p = &b["parameters"][n];
                let ok = p["value"].is_f64() || p["not_evaluable"]["code"].is_string();
                assert!(ok, "{} {n}: {p}", r["label"]);
            }
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

/// Per receiver and band: `(label, band, r, spl, total energy, Lw)`.
fn level_rows(run: &Path) -> Vec<(String, i32, f64, f64, f64, f64)> {
    let o = results(run, true);
    assert_eq!(o.code, 0, "{o:#?}");
    let rep = json(&o);
    let s = &rep["spps"];
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
            let spl = b["parameters"]["spl_db"]["value"]
                .as_f64()
                .unwrap_or_else(|| {
                    panic!("{} {f} Hz: SPL {}", r["label"], b["parameters"]["spl_db"])
                });
            let l = lw.iter().find(|x| x.0 == f).unwrap().1;
            out.push((
                r["label"].as_str().unwrap().to_string(),
                f,
                dist,
                spl,
                b["total_pa2"].as_f64().unwrap(),
                l,
            ));
        }
    }
    out
}

/// Gate M7(c)'s predicate.
fn gate_c(spl: f64, lw: f64, r: f64) -> bool {
    (spl - (lw - 20.0 * r.log10() - 11.0)).abs() <= 0.5
}

#[test]
fn gate_c_level_calibration_and_the_offsets_it_catches() {
    let root = scratch("gate-c");
    let run = run_ok(&fixture(LEVEL), "spps", &root, &[]);
    let rows = level_rows(&run);
    assert_eq!(rows.len(), 12, "2 receivers x 6 bands");
    // What keeps the reverberant field out is the duration: no particle reaches a surface, so
    // none is absorbed and every one remains (results_rooms.rs, level_box).
    let rep = json(&results(&run, true));
    for b in rep["spps"]["particles"]["bands"].as_array().unwrap() {
        assert_eq!(b["absorbed_by_materials"], 0, "{b}");
        assert_eq!(b["remaining"], b["total"], "{b}");
    }
    // ρ·c as SPPS computes it at 20 °C and 101 325 Pa (Masse_volumique_air.cpp:45-52;
    // Celerite_du_son.cpp:46), and the average of 1/d² over a receiver sphere of radius R.
    let rho_c = 101_325.0 * 28.9644 / (8314.32 * 293.15) * 343.2;
    let exact = |lw: f64, r: f64| {
        lw + 10.0 * (rho_c * 1e-12 / (4.0 * std::f64::consts::PI * 4e-10)).log10()
            - 20.0 * r.log10()
            + 10.0 * (1.0 + 0.25 / (5.0 * r * r)).log10()
    };
    let (mut worst_gate, mut worst_exact) = (0.0f64, 0.0f64);
    for (label, f, r, spl, total, lw) in &rows {
        assert!((lw - 100.0).abs() < 1e-9, "{f} Hz: Lw {lw}");
        let want = lw - 20.0 * r.log10() - 11.0;
        println!(
            "{label} {f:>5} Hz r {r:.6} m: SPL {spl:.3} dB, gate {want:.3} ({:+.3}), exact {:.3} \
             ({:+.3})",
            spl - want,
            exact(*lw, *r),
            spl - exact(*lw, *r)
        );
        worst_gate = worst_gate.max((spl - want).abs());
        worst_exact = worst_exact.max((spl - exact(*lw, *r)).abs());
        assert!(gate_c(*spl, *lw, *r), "{label} {f} Hz");
        // Says no: Night Mode's .gap level, energy over the intensity reference 1e-12.
        let night_mode = 10.0 * (total / 1e-12).log10();
        assert!(!gate_c(night_mode, *lw, *r), "{label} {f} Hz: {night_mode}");
        assert!((night_mode - spl - 26.0206).abs() < 1e-3);
        // Says no: 1 dB either way.
        assert!(!gate_c(spl + 1.0, *lw, *r) && !gate_c(spl - 1.0, *lw, *r));
    }
    println!("worst |SPL - gate| {worst_gate:.3} dB, worst |SPL - exact| {worst_exact:.3} dB");
}

// --- gate (d): TCR against the analytic values ---------------------------------------------------

/// Sabine and Eyring per band from the project itself: faces and their groups' materials,
/// the geometry's volume, the solver's air term; with the walls' α scaled by `walls`.
fn project_analytic(p: &schema::Project, walls: f64) -> Vec<(u32, f64, f64)> {
    let v = &p.geometry.vertices;
    let pt = |i: u32| v[i as usize].to_array();
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
        let m = p.material(g.material).unwrap();
        let scale = if g.name == "Walls" { walls } else { 1.0 };
        faces.push((area, m, scale));
    }
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
            let s: Vec<Surface> = faces
                .iter()
                .map(|(area, m, scale)| Surface {
                    area_m2: *area,
                    absorption: m.absorption[i].get() * scale,
                })
                .collect();
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

fn within(a: f64, b: f64, rel: f64) -> bool {
    (a / b - 1.0).abs() <= rel
}

#[test]
fn gate_d_tcr_equals_the_analytic_sabine_and_eyring_and_says_no() {
    let root = scratch("gate-d");
    let run = run_ok(&fixture(TUTORIAL), "tcr", &root, &[]);
    let o = results(&run, true);
    assert_eq!(o.code, 0, "{o:#?}");
    let rep = json(&o);
    let t = &rep["tcr"];
    let p = schema::load(&fixture(TUTORIAL)).unwrap();
    let ours = project_analytic(&p, 1.0);
    let moved = project_analytic(&p, 1.05);
    let from_run = &t["analytic"];
    assert_eq!(from_run["status"], "computed", "{from_run}");
    let (mut worst, mut moved_out) = (0.0f64, 0usize);
    for (i, b) in t["bands"].as_array().unwrap().iter().enumerate() {
        let f = b["freq_hz"].as_i64().unwrap() as u32;
        let (of, sab, eyr) = ours[i];
        assert_eq!(of, f);
        let tcr_sab = b["sabine"]["reverberation_time_s"].as_f64().unwrap();
        let tcr_eyr = b["eyring"]["reverberation_time_s"].as_f64().unwrap();
        let run_sab = from_run["bands"][i]["sabine_s"]["value"].as_f64().unwrap();
        let run_eyr = from_run["bands"][i]["eyring_s"]["value"].as_f64().unwrap();
        for (what, x, y) in [
            ("Sabine vs project", tcr_sab, sab),
            ("Eyring vs project", tcr_eyr, eyr),
            ("Sabine vs run inputs", tcr_sab, run_sab),
            ("Eyring vs run inputs", tcr_eyr, run_eyr),
        ] {
            assert!(within(x, y, 0.005), "{f} Hz {what}: TCR {x}, ours {y}");
            worst = worst.max((x / y - 1.0).abs());
        }
        // Says no: the walls 5 % more absorbing, against the unchanged run.
        let (_, msab, meyr) = moved[i];
        if !within(tcr_sab, msab, 0.005) && !within(tcr_eyr, meyr, 0.005) {
            moved_out += 1;
        } else {
            println!(
                "{f} Hz: walls +5 % stays within 0.5 %: {tcr_sab} vs {msab}, {tcr_eyr} vs {meyr}"
            );
        }
    }
    let n = t["bands"].as_array().unwrap().len();
    println!(
        "worst |TCR/ours - 1| {worst:.2e}; walls +5 % outside 0.5 % in {moved_out} of {n} bands"
    );
    assert_eq!(moved_out, n);

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
    let arrival = Arrival::Known { time_s: arrival_s };
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
