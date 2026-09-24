//! `core::results::load` on the committed run folders `tests/fixtures/results/seats_spps` and
//! `seats_tcr` (tutorial 1's box, point receivers `Seat` and `Seat2`, written by
//! `cargo test -p simpa --test cli_results -- --ignored write_results_fixtures`), with no solver.
//!
//! - What is read is typed and consistent with itself: bands, steps, positions and arrivals, the
//!   `.gap` against the `.recp`, the per-source totals, the surface files, TCR's tables and the
//!   analytic references on the run's inputs.
//! - Every refusal code is produced by at least one spoiled copy of a fixture, and each spoiled
//!   copy gives the code it should: the refusals say no.

mod common;

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use serde_json::Value;
use simpa_core::formats::gabe::{self, ColumnData, Gabe};
use simpa_core::results::{self, RunResults, codes, spps::solver_speed_of_sound, tcr::Analytic};

const SPPS: &str = "results/seats_spps";
const TCR: &str = "results/seats_tcr";
const ENERGETIC: &str = "results/energetic_spps";
const SOURCES2: &str = "results/sources2_spps";

fn load(name: &str) -> RunResults {
    results::load(&common::fixture(name)).unwrap_or_else(|r| panic!("{name}: {r}"))
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

/// A fresh copy of a fixture run folder.
fn copy_of(name: &str, label: &str) -> PathBuf {
    static N: AtomicUsize = AtomicUsize::new(0);
    let to = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!(
        "results-load/{label}-{}-{}",
        std::process::id(),
        N.fetch_add(1, Ordering::SeqCst)
    ));
    if to.exists() {
        panic!("{} exists", to.display());
    }
    copy_dir(&common::fixture(name), &to);
    to
}

fn edit_manifest(run: &Path, f: impl FnOnce(&mut Value)) {
    let p = run.join("run.json");
    let mut v: Value = serde_json::from_str(&std::fs::read_to_string(&p).unwrap()).unwrap();
    f(&mut v);
    std::fs::write(&p, serde_json::to_string_pretty(&v).unwrap()).unwrap();
}

/// The byte offset of row `row` of float column `col` (`docs/formats/gabe.md`, "Layout").
fn value_offset(g: &Gabe, col: usize, row: usize) -> usize {
    let size = |c: &gabe::Column| match &c.data {
        ColumnData::Float { values, .. } => 280 + 4 + 4 * values.len(),
        ColumnData::Int(v) => 280 + 1 + 4 * v.len(),
        ColumnData::ShortString(v) => 280 + 1 + 50 * v.len(),
    };
    20 + g.columns[..col].iter().map(size).sum::<usize>() + 280 + 4 + 4 * row
}

fn plant(path: &Path, col: usize, row: usize, value: f32) {
    let g = gabe::read_file(path).unwrap();
    let at = value_offset(&g, col, row);
    let mut bytes = std::fs::read(path).unwrap();
    assert_eq!(
        f32::from_le_bytes(bytes[at..at + 4].try_into().unwrap()).to_bits(),
        g.columns[col].floats().unwrap()[row].to_bits()
    );
    bytes[at..at + 4].copy_from_slice(&value.to_le_bytes());
    std::fs::write(path, bytes).unwrap();
}

#[test]
fn the_spps_run_reads_typed_and_consistent() {
    let r = load(SPPS);
    assert_eq!(r.bands_hz, [500, 1000]);
    let s = r.spps().expect("an SPPS run");
    assert!(r.tcr().is_none());
    assert_eq!(s.time_step_s, f64::from(0.01f32));
    assert_eq!(s.steps, 100);
    assert_eq!(s.speed_of_sound_m_s, f64::from(solver_speed_of_sound(20.0)));
    assert_eq!(s.speed_of_sound_m_s, f64::from(343.2f32));
    assert_eq!(s.receiver_radius_m, f64::from(0.31f32));
    assert!(!s.celerity_gradient);
    assert_eq!(s.sources.len(), 1);
    assert_eq!(s.sources[0].name, "Source 1");
    assert_eq!(s.sources[0].emission_s, 0.0);
    let labels: Vec<&str> = s.point_receivers.iter().map(|p| p.label.as_str()).collect();
    assert_eq!(labels, ["Seat", "Seat2"]);
    let seat = s.point_receiver("Seat").unwrap();
    let seat2 = s.point_receiver("Seat2").unwrap();
    assert_eq!(seat.folder, "Punctual receivers/Seat");
    assert_eq!(seat.position_m, Some([1.0, 1.0, f64::from(1.8f32)]));
    // Arrival: the distance from the source over c, from f32 positions.
    let c = s.speed_of_sound_m_s;
    let d = |p: [f64; 3], q: [f64; 3]| {
        ((p[0] - q[0]).powi(2) + (p[1] - q[1]).powi(2) + (p[2] - q[2]).powi(2)).sqrt()
    };
    let src = s.sources[0].position_m.unwrap();
    assert_eq!(
        s.arrival_s(seat),
        Some(d(seat.position_m.unwrap(), src) / c)
    );
    assert!((s.arrival_s(seat2).unwrap() - 2.0 / 343.2).abs() < 1e-7);
    assert!((s.receiver_crossing_s() - 0.62 / 343.2).abs() < 1e-7);
    for p in [seat, seat2] {
        assert_eq!(p.bands.len(), 2);
        for (b, f) in p.bands.iter().zip([500, 1000]) {
            assert_eq!(b.freq_hz, f);
            assert_eq!(b.energy.len(), 100);
            assert!(b.energy.iter().sum::<f64>() > 0.0);
            // E·cos²φ ≤ E·|cos φ| ≤ E, step by step.
            for k in 0..100 {
                let tol = 1e-6 * b.energy[k] + 1e-30;
                assert!(
                    b.lateral_cos2[k] <= b.lateral_abs_cos[k] + tol,
                    "{f} Hz step {k}"
                );
                assert!(b.lateral_abs_cos[k] <= b.energy[k] + tol, "{f} Hz step {k}");
            }
            assert!(b.source_power_rho_c > 0.0);
            assert_eq!(
                b.intensity.iter().map(Vec::len).collect::<Vec<_>>(),
                [100; 3]
            );
        }
        // One source: its totals are the series' sums.
        assert_eq!(p.by_source.len(), 1);
        for (i, b) in p.bands.iter().enumerate() {
            let sum: f64 = b.energy.iter().sum();
            let total = p.by_source[0].energy[i];
            assert!((total / sum - 1.0).abs() < 1e-5, "{total} vs {sum}");
        }
    }
    assert_ne!(seat.bands[0].energy, seat2.bands[0].energy);
    assert_eq!(s.total_energy.len(), 2);
    assert_eq!(
        s.particles
            .bands
            .iter()
            .map(|b| b.total)
            .collect::<Vec<_>>(),
        [2000, 2000]
    );
    // Random mode, and the statistics count the particles still alive at the end.
    for b in &s.particles.bands {
        assert_eq!(s.band_complete(b.freq_hz), b.remaining == 0);
    }
    // The floor's receiver: per band and Global.
    let bands: Vec<Option<i32>> = s.surfaces.iter().map(|f| f.band_hz).collect();
    assert_eq!(bands, [Some(500), Some(1000), None]);
    assert!(
        s.surfaces
            .iter()
            .all(|f| f.field.is_none() && !f.cutting_plane)
    );
}

#[test]
fn the_tcr_run_reads_typed_and_matches_its_analytic_values() {
    let r = load(TCR);
    let t = r.tcr().expect("a TCR run");
    assert_eq!(
        t.bands.iter().map(|b| b.freq_hz).collect::<Vec<_>>(),
        [500, 1000]
    );
    let labels: Vec<&str> = t.point_receivers.iter().map(|p| p.label.as_str()).collect();
    assert_eq!(labels, ["Seat", "Seat2"]);
    assert_eq!(
        t.point_receiver("Seat").unwrap().file,
        "Punctual receivers/Seat.gabe"
    );
    // 3 fields, each 2 bands and Global.
    assert_eq!(t.surfaces.len(), 9);
    let fields: std::collections::BTreeSet<&str> = t
        .surfaces
        .iter()
        .filter_map(|f| f.field.as_deref())
        .collect();
    assert_eq!(
        fields.into_iter().collect::<Vec<_>>(),
        [
            "Direct field",
            "Total field (Eyring)",
            "Total field (Sabine)"
        ]
    );
    let Analytic::Computed {
        volume_m3,
        area_m2,
        bands,
    } = &t.analytic
    else {
        panic!("{:?}", t.analytic);
    };
    assert!((volume_m3 - 180.0).abs() < 1e-3, "{volume_m3}");
    assert!((area_m2 - 216.0).abs() < 1e-3, "{area_m2}");
    for (b, a) in t.bands.iter().zip(bands) {
        let sab = a.sabine_s.clone().unwrap();
        let eyr = a.eyring_s.clone().unwrap();
        assert!((b.sabine.reverberation_time_s / sab - 1.0).abs() < 1e-5);
        assert!((b.eyring.reverberation_time_s / eyr - 1.0).abs() < 1e-5);
        assert!(a.air_m_per_metre.is_some_and(|m| m > 0.0));
    }
}

/// `ρ·c` as SPPS computes it at 20 °C and 101 325 Pa (`Masse_volumique_air.cpp:45-52`;
/// `Celerite_du_son.cpp:46`).
fn solver_rho_c() -> f64 {
    101_325.0 * 28.9644 / (8314.32 * 293.15) * 343.2
}

#[test]
fn the_energetic_run_is_never_complete_and_its_floor_refuses_the_decay_times() {
    let r = load(ENERGETIC);
    let s = r.spps().unwrap();
    assert_eq!(s.computation_method, 1);
    assert_eq!(s.trans_epsilon, 3.0);
    assert_eq!(s.floor_db(), Some(-30.0));
    // Every particle was dropped, absorbed or lost before the end, and still no band is complete:
    // the drop is energy the histogram never holds. In at least one band none was lost either,
    // so energetic mode alone is what refuses the claim there.
    let mut isolated = 0;
    for b in &s.particles.bands {
        println!("{} Hz: {b:?}", b.freq_hz);
        assert_eq!(b.remaining, 0, "{} Hz", b.freq_hz);
        assert!(!s.band_complete(b.freq_hz), "{} Hz", b.freq_hz);
        if b.lost() == 0 {
            isolated += 1;
        }
    }
    assert!(isolated >= 1);
    let rep = serde_json::to_value(results::report(&r)).unwrap();
    let mut floor = 0;
    for p in rep["spps"]["point_receivers"].as_array().unwrap() {
        for b in p["bands"].as_array().unwrap() {
            assert_eq!(b["complete"], false);
            assert_eq!(b["floor_db"], -30.0);
            let t30 = &b["parameters"]["t30_s"]["not_evaluable"];
            assert!(
                t30.is_object(),
                "{} {}: T30 {t30}",
                p["label"],
                b["freq_hz"]
            );
            let why = &t30["error"]["why"];
            if why["why"].as_str().unwrap().starts_with("missing_") && why["floor_db"] == -30.0 {
                floor += 1;
            }
            println!("{} {} Hz: T30 {}", p["label"], b["freq_hz"], t30["message"]);
        }
    }
    // Three of the four receiver-bands give the floor as the reason; Seat2 at 500 Hz is refused
    // earlier, its tail not decaying.
    assert!(floor >= 3, "{floor}");
}

#[test]
fn the_two_source_run_reads_each_sources_echogram_and_refuses_their_sum() {
    let r = load(SOURCES2);
    let s = r.spps().unwrap();
    assert!(s.echogram_per_source);
    let names: Vec<&str> = s.sources.iter().map(|x| x.name.as_str()).collect();
    assert_eq!(names, ["Source 2", "Source 1"]);
    // Source 2 is 3 dB weaker: half the power, as SPPS computes it in f32.
    let (w2, w1) = (s.sources[0].band_power_w[0], s.sources[1].band_power_w[0]);
    assert!((w1 / w2 - 10f64.powf(0.3)).abs() < 1e-5, "{w1} / {w2}");
    // Its particles start 2 steps late.
    assert_eq!(s.sources[0].emission_s, 2.0 * f64::from(0.01f32));
    // The mean deposit of Source 1's particles against its closed form W·ρc/(N·πR²).
    let d1 = s.mean_deposit(1, 0).unwrap();
    let want = w1 * solver_rho_c() / (2000.0 * std::f64::consts::PI * f64::from(0.31f32).powi(2));
    assert!((d1 / want - 1.0).abs() < 1e-5, "{d1} vs {want}");
    for p in &s.point_receivers {
        assert_eq!(p.echograms.len(), 2);
        assert_eq!(
            p.echograms[0].file,
            format!("Punctual receivers/{}/Source 2/Sound level.recp", p.label)
        );
        for (i, b) in p.bands.iter().enumerate() {
            let (a, c) = (&p.echograms[0].energy[i], &p.echograms[1].energy[i]);
            assert_ne!(a, c);
            for k in 0..b.energy.len() {
                let sum = a[k] + c[k];
                assert!((sum - b.energy[k]).abs() <= 2e-6 * sum.max(b.energy[k]));
            }
            assert_eq!(p.contributing(i).len(), 2);
        }
        // Each source's arrival is its own.
        let one = s.arrival_from(p, &["Source 1"]).unwrap();
        let two = s.arrival_from(p, &["Source 2"]).unwrap();
        assert_ne!(one, two);
        assert_eq!(s.arrival_s(p), Some(one.min(two)));
    }
    let rep = serde_json::to_value(results::report(&r)).unwrap();
    for p in rep["spps"]["point_receivers"].as_array().unwrap() {
        for b in p["bands"].as_array().unwrap() {
            // The sum of two sources: every onset-relative parameter refused, SPL not for that.
            for q in ["edt_s", "t20_s", "t30_s", "c50_db", "c80_db", "d50", "ts_s"] {
                let why = &b["parameters"][q]["not_evaluable"]["error"]["why"];
                assert_eq!(why["why"], "several_sources", "{q}");
                assert_eq!(why["sources"], serde_json::json!(["Source 2", "Source 1"]));
            }
            let spl = &b["parameters"]["spl_db"]["not_evaluable"]["error"]["why"]["why"];
            assert_ne!(spl, "several_sources");
            // The noise model takes the stronger source's deposit.
            assert_eq!(b["noise_model"]["model"], "crossings");
        }
        let per: Vec<&str> = p["per_source"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x["source"].as_str().unwrap())
            .collect();
        assert_eq!(per, ["Source 2", "Source 1"]);
        for x in p["per_source"].as_array().unwrap() {
            for b in x["bands"].as_array().unwrap() {
                for q in ["edt_s", "t20_s", "t30_s", "c50_db", "c80_db", "d50", "ts_s"] {
                    let why = &b["parameters"][q]["not_evaluable"]["error"]["why"]["why"];
                    assert_ne!(why, "several_sources", "{} {q}", x["source"]);
                }
            }
        }
    }
    // The band's model is Source 1's deposit, the larger.
    let m = &rep["spps"]["point_receivers"][0]["bands"][0]["noise_model"]["mean_deposit"];
    assert!((m.as_f64().unwrap() / d1 - 1.0).abs() < 1e-12, "{m}");
}

type Spoil = fn(&Path);

/// `(fixture, what is spoiled, how, the code it must give)`.
fn cases() -> Vec<(&'static str, &'static str, Spoil, &'static str)> {
    vec![
        (
            SPPS,
            "no run.json",
            |r| std::fs::remove_file(r.join("run.json")).unwrap(),
            codes::MANIFEST_MISSING,
        ),
        (
            SPPS,
            "run.json not JSON",
            |r| std::fs::write(r.join("run.json"), "{").unwrap(),
            codes::MANIFEST_INVALID,
        ),
        (
            SPPS,
            "manifest version 2",
            |r| edit_manifest(r, |v| v["manifest_version"] = 2.into()),
            codes::MANIFEST_INVALID,
        ),
        (
            SPPS,
            "another solver commit",
            |r| edit_manifest(r, |v| v["solver_commit"] = "0".repeat(40).into()),
            codes::MANIFEST_INVALID,
        ),
        (
            SPPS,
            "OK at stage export",
            |r| edit_manifest(r, |v| v["stage"] = "export".into()),
            codes::MANIFEST_INVALID,
        ),
        (
            SPPS,
            "OK with a reason",
            |r| {
                edit_manifest(r, |v| {
                    v["verdict"]["reasons"] =
                        serde_json::json!([{"code": "exit_nonzero", "detail": "x"}])
                })
            },
            codes::MANIFEST_INVALID,
        ),
        (
            SPPS,
            "OK with exit 1",
            |r| edit_manifest(r, |v| v["outcome"]["exit_code"] = 1.into()),
            codes::MANIFEST_INVALID,
        ),
        (
            SPPS,
            "OK with no outcome",
            |r| edit_manifest(r, |v| v["outcome"] = Value::Null),
            codes::MANIFEST_INVALID,
        ),
        (
            SPPS,
            "FAIL",
            |r| {
                edit_manifest(r, |v| {
                    v["verdict"]["status"] = "FAIL".into();
                    v["verdict"]["reasons"] =
                        serde_json::json!([{"code": "particle_loss_excess", "detail": "x"}]);
                })
            },
            codes::RUN_FAILED,
        ),
        (
            SPPS,
            "CRASH",
            |r| {
                edit_manifest(r, |v| {
                    v["verdict"]["status"] = "CRASH".into();
                    v["verdict"]["reasons"] =
                        serde_json::json!([{"code": "crash_access_violation", "detail": "x"}]);
                })
            },
            codes::RUN_FAILED,
        ),
        (
            SPPS,
            "CANCELLED",
            |r| {
                edit_manifest(r, |v| {
                    v["verdict"]["status"] = "CANCELLED".into();
                    v["verdict"]["reasons"] =
                        serde_json::json!([{"code": "cancelled", "detail": "x"}]);
                })
            },
            codes::RUN_CANCELLED,
        ),
        (
            SPPS,
            "config.xml edited",
            |r| {
                let p = r.join("solve/config.xml");
                let mut t = std::fs::read(&p).unwrap();
                t.push(b'\n');
                std::fs::write(&p, t).unwrap();
            },
            codes::INPUTS_CHANGED,
        ),
        (
            SPPS,
            "mesh.cbin removed",
            |r| std::fs::remove_file(r.join("solve/mesh.cbin")).unwrap(),
            codes::INPUTS_CHANGED,
        ),
        (
            SPPS,
            "statistics removed",
            |r| std::fs::remove_file(r.join("solve/SPPS particle statistics.gabe")).unwrap(),
            codes::OUTPUTS_INVALID,
        ),
        (
            SPPS,
            "Global surface file emptied",
            |r| {
                std::fs::write(
                    r.join("solve/Surface receiver/Global/Sound level.csbin"),
                    b"",
                )
                .unwrap()
            },
            codes::OUTPUTS_INVALID,
        ),
        (
            TCR,
            "NaN in Seat's direct level",
            |r| {
                plant(
                    &r.join("solve/Punctual receivers/Seat.gabe"),
                    1,
                    0,
                    f32::NAN,
                )
            },
            codes::OUTPUTS_INVALID,
        ),
        (
            SPPS,
            "a folder no label names",
            |r| std::fs::create_dir(r.join("solve/Punctual receivers/Seat3")).unwrap(),
            codes::FILE_INVALID,
        ),
        (
            TCR,
            "a table no label names",
            |r| {
                std::fs::copy(
                    r.join("solve/Punctual receivers/Seat2.gabe"),
                    r.join("solve/Punctual receivers/Seat3.gabe"),
                )
                .map(|_| ())
                .unwrap()
            },
            codes::FILE_INVALID,
        ),
        (
            SPPS,
            ".gap energy differs from the .recp",
            |r| {
                let p = r.join("solve/Punctual receivers/Seat2/Advanced sound level.gap");
                let v = gabe::read_file(&p).unwrap().columns[5].floats().unwrap()[10];
                plant(&p, 5, 10, v * 2.0 + 1e-9);
            },
            codes::FILE_INVALID,
        ),
        (
            SPPS,
            "intensity table cut short",
            |r| {
                let p = r.join("solve/Punctual receivers/Seat/Punctual receiver intensity.gabe");
                let b = std::fs::read(&p).unwrap();
                std::fs::write(&p, &b[..b.len() - 100]).unwrap();
            },
            codes::FILE_INVALID,
        ),
        (
            SPPS,
            "NaN in a .recp",
            |r| {
                plant(
                    &r.join("solve/Punctual receivers/Seat/Sound level.recp"),
                    1,
                    12,
                    f32::NAN,
                )
            },
            codes::VALUE_INVALID,
        ),
        (
            SPPS,
            "a negative energy in the room total",
            |r| plant(&r.join("solve/Total energy.recp"), 2, 5, -1.0),
            codes::VALUE_INVALID,
        ),
        (
            TCR,
            "NaN in Seat's Global row, an energetic sum the verdict does not scan",
            |r| {
                // Column 1 is Direct; row 2 is Global, after the two bands.
                plant(
                    &r.join("solve/Punctual receivers/Seat.gabe"),
                    1,
                    2,
                    f32::NAN,
                )
            },
            codes::VALUE_INVALID,
        ),
        (
            SOURCES2,
            "a source's echogram removed",
            |r| {
                std::fs::remove_file(
                    r.join("solve/Punctual receivers/Seat2/Source 1/Sound level.recp"),
                )
                .unwrap()
            },
            codes::OUTPUTS_INVALID,
        ),
        (
            SOURCES2,
            "a source's echogram that no longer sums to the .recp",
            |r| {
                let p = r.join("solve/Punctual receivers/Seat/Source 2/Sound level.recp");
                let g = gabe::read_file(&p).unwrap();
                let (row, v) = g.columns[1]
                    .floats()
                    .unwrap()
                    .iter()
                    .enumerate()
                    .find(|(_, v)| **v > 0.0)
                    .map(|(i, v)| (i, *v))
                    .unwrap();
                plant(&p, 1, row, v * 1.01);
            },
            codes::FILE_INVALID,
        ),
        (
            SOURCES2,
            "a folder in a receiver folder that no source names",
            |r| std::fs::create_dir(r.join("solve/Punctual receivers/Seat/Source 3")).unwrap(),
            codes::FILE_INVALID,
        ),
        (
            SPPS,
            "a per-source folder when output_recp_bysource is off",
            |r| std::fs::create_dir(r.join("solve/Punctual receivers/Seat/Source 1")).unwrap(),
            codes::FILE_INVALID,
        ),
    ]
}

#[test]
fn every_refusal_says_no_with_its_code_and_every_code_is_produced() {
    let mut seen = std::collections::BTreeSet::new();
    for (fixture, what, spoil, want) in cases() {
        let run = copy_of(fixture, "refusal");
        results::load(&run).unwrap_or_else(|r| panic!("{what}: the clean copy is refused: {r}"));
        spoil(&run);
        let refused = match results::load(&run) {
            Ok(_) => panic!("{what}: read, not refused"),
            Err(r) => r,
        };
        assert_eq!(refused.code, want, "{what}: {refused}");
        let exit = if want == codes::RUN_FAILED || want == codes::RUN_CANCELLED {
            5
        } else {
            6
        };
        assert_eq!(refused.exit_code(), exit, "{what}");
        println!("{what}: {refused}");
        seen.insert(want);
    }
    let all: std::collections::BTreeSet<&str> = codes::ALL.into_iter().collect();
    assert_eq!(seen, all, "a code no case produces");
}

#[test]
fn a_failed_runs_reasons_are_carried() {
    let run = copy_of(SPPS, "failed-reasons");
    edit_manifest(&run, |v| {
        v["verdict"]["status"] = "FAIL".into();
        v["verdict"]["reasons"] =
            serde_json::json!([{"code": "particle_loss_excess", "detail": "3 bands"}]);
    });
    let r = results::load(&run).unwrap_err();
    assert_eq!(r.reasons.len(), 1);
    assert_eq!(r.reasons[0].code, "particle_loss_excess");
    assert!(
        r.to_string().contains("particle_loss_excess: 3 bands"),
        "{r}"
    );
}

#[test]
fn the_contract_table_lists_every_code_in_order_with_its_exit() {
    let text =
        std::fs::read_to_string(common::paths::repo_file("docs/solver-contract.md")).unwrap();
    let at = text.find("### Result refusals").expect("the section");
    let rows: Vec<(String, String)> = text[at..]
        .lines()
        .skip_while(|l| !l.starts_with("| Code |"))
        .skip(2)
        .take_while(|l| l.starts_with('|'))
        .map(|l| {
            let cells: Vec<&str> = l.trim_matches('|').split('|').map(str::trim).collect();
            (
                cells[0].trim_matches('`').to_string(),
                cells.last().unwrap().to_string(),
            )
        })
        .collect();
    let codes_in_doc: Vec<&str> = rows.iter().map(|r| r.0.as_str()).collect();
    assert_eq!(codes_in_doc, codes::ALL);
    for (code, exit) in &rows {
        let r = results::Refusal {
            code: code.clone(),
            detail: String::new(),
            reasons: Vec::new(),
        };
        assert_eq!(exit, &r.exit_code().to_string(), "{code}");
    }
}
