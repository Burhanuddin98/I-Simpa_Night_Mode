//! The Rust port of EDT v2.1 against `frozen2/method.py` (docs/investigations/2026-10-02-edt-port/PORT.md).
//!
//! The fixture (`testdata/edt_parity`, made by `tools/edt_port/make_fixture.py`) holds histograms
//! drawn from round 2's inputs plus crafted ones, each with frozen2's own output, recorded after
//! frozen2/method.py passed its sha256 check.

use std::collections::BTreeSet;
use std::path::PathBuf;

use serde_json::Value;
use simpa_core::params::edt::{self, REFUSAL_REASONS, Status};

struct Row {
    id: String,
    set: String,
    mode: Option<String>,
    step_ms: f64,
    dt: f64,
    t_arrival: Option<f64>,
    half_width: Option<f64>,
    bins: Vec<f64>,
    status: String,
    reason: String,
    edt: [Option<f64>; 3],
}

fn fixture() -> Vec<Row> {
    // EDT_FIXTURE_DIR points the run at a larger scratch fixture (every round-2 row).
    let dir = std::env::var_os("EDT_FIXTURE_DIR").map_or_else(
        || PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../testdata/edt_parity"),
        PathBuf::from,
    );
    let blob = std::fs::read(dir.join("bins.bin")).expect("bins.bin");
    let manifest = std::fs::read_to_string(dir.join("manifest.jsonl")).expect("manifest.jsonl");
    manifest
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| {
            let v: Value = serde_json::from_str(l).unwrap();
            let f = |k: &str| v[k].as_f64();
            let n = v["n"].as_u64().unwrap() as usize;
            let off = v["offset"].as_u64().unwrap() as usize;
            let len = v["bytes"].as_u64().unwrap() as usize;
            let raw = &blob[off..off + len];
            let mut bins = vec![0.0; n];
            match v["enc"].as_str().unwrap() {
                "dense" => {
                    assert_eq!(len, 8 * n);
                    for (i, c) in raw.chunks_exact(8).enumerate() {
                        bins[i] = f64::from_le_bytes(c.try_into().unwrap());
                    }
                }
                _ => {
                    for c in raw.chunks_exact(12) {
                        let i = u32::from_le_bytes(c[..4].try_into().unwrap()) as usize;
                        bins[i] = f64::from_le_bytes(c[4..].try_into().unwrap());
                    }
                }
            }
            let e = &v["expect"];
            Row {
                id: v["id"].as_str().unwrap().into(),
                set: v["set"].as_str().unwrap().into(),
                mode: v["mode"].as_str().map(String::from),
                step_ms: f("step_ms").unwrap(),
                dt: f("dt").unwrap(),
                t_arrival: f("t_arrival"),
                half_width: f("half_width"),
                bins,
                status: e["status"].as_str().unwrap().into(),
                reason: e["reason"].as_str().unwrap().into(),
                edt: [
                    e["edt"].as_f64(),
                    e["edt_lo"].as_f64(),
                    e["edt_hi"].as_f64(),
                ],
            }
        })
        .collect()
}

fn status_name(s: Status) -> &'static str {
    match s {
        Status::Ok => "ok",
        Status::Wide => "wide",
        Status::Refused => "refused",
    }
}

/// The tolerance, 1e-9 relative, is for float order of operations: numpy sums in its own
/// unrolled/pairwise order and takes `log10` from the C library, Rust sums left to right; every
/// operation is correctly rounded either way, so values should agree to a few ulps (1e-15), far
/// inside it. It is a ceiling, not an expectation: the run prints the worst difference found.
const TOLERANCE: f64 = 1e-9;

#[test]
fn every_fixture_row_equals_frozen2_in_status_reason_and_values() {
    let rows = fixture();
    assert!(rows.len() >= 500, "{} rows", rows.len());
    let mut worst: (f64, String) = (0.0, String::new());
    let mut bad = Vec::new();
    for r in &rows {
        let o = edt::analyse(&r.bins, r.dt, r.t_arrival, r.half_width);
        if status_name(o.status) != r.status || o.reason != r.reason {
            bad.push(format!(
                "{}: {} / {} vs frozen2 {} / {}",
                r.id,
                status_name(o.status),
                o.reason,
                r.status,
                r.reason
            ));
            continue;
        }
        for (got, want) in [o.edt, o.edt_lo, o.edt_hi].into_iter().zip(r.edt) {
            match (got, want) {
                (None, None) => {}
                (Some(g), Some(w)) => {
                    let rel = (g - w).abs() / w.abs();
                    if rel > worst.0 {
                        worst = (rel, r.id.clone());
                    }
                    assert!(rel <= TOLERANCE, "{}: {g} vs {w}", r.id);
                }
                _ => bad.push(format!("{}: {got:?} vs {want:?}", r.id)),
            }
        }
    }
    assert!(
        bad.is_empty(),
        "{} rows differ: {:#?}",
        bad.len(),
        &bad[..bad.len().min(10)]
    );
    println!(
        "PARITY: {} rows, max rel diff {:e} ({})",
        rows.len(),
        worst.0,
        worst.1
    );
}

#[test]
fn the_fixture_covers_every_status_set_mode_and_step_of_round_2() {
    let rows = fixture();
    let sets: BTreeSet<&str> = rows.iter().map(|r| r.set.as_str()).collect();
    assert_eq!(
        sets,
        BTreeSet::from(["crafted", "ism", "spps", "synth"]),
        "every set"
    );
    for st in ["ok", "wide", "refused"] {
        assert!(rows.iter().any(|r| r.status == st), "{st}");
    }
    for mode in ["random", "energetic"] {
        for step in [1.0, 2.0, 5.0] {
            assert!(
                rows.iter().any(|r| r.set == "spps"
                    && r.mode.as_deref() == Some(mode)
                    && (r.step_ms - step).abs() < 1e-3),
                "spps {mode} {step} ms"
            );
        }
    }
    for set in ["ism", "synth"] {
        for step in [1.0, 2.0, 5.0] {
            assert!(
                rows.iter()
                    .any(|r| r.set == set && (r.step_ms - step).abs() < 1e-3),
                "{set} {step} ms"
            );
        }
    }
    assert!(
        rows.iter()
            .any(|r| r.set == "synth" && (r.step_ms - 10.0).abs() < 1e-3)
    );
}

#[test]
fn every_refusal_reason_of_frozen2_is_reachable_from_rust() {
    // `not_decaying` cannot be reached by any input: the Schroeder level is non-increasing, the
    // fit starts above -10 dB and ends at or below it, so the least-squares slope is negative
    // (frozen2 keeps the branch as a guard; a fuzz of 6000 inputs never reached it). The other
    // eight are reached here from the fixture, by Rust, and are the ones frozen2 gave.
    let rows = fixture();
    let reached: BTreeSet<String> = rows
        .iter()
        .map(|r| edt::analyse(&r.bins, r.dt, r.t_arrival, r.half_width))
        .filter(|o| o.status == Status::Refused)
        .map(|o| o.reason)
        .collect();
    let expected: BTreeSet<String> = REFUSAL_REASONS
        .iter()
        .filter(|r| **r != "not_decaying")
        .map(|s| s.to_string())
        .collect();
    assert_eq!(reached, expected);
    let frozen: BTreeSet<String> = rows
        .iter()
        .filter(|r| r.status == "refused")
        .map(|r| r.reason.clone())
        .collect();
    assert_eq!(frozen, expected, "frozen2's own reasons on the fixture");
}

#[test]
fn a_missing_arrival_is_in_the_fixture_and_agrees() {
    // `noise=unknown` (the noise window holds no block with energy) is not reachable: the fit
    // window is at least four blocks long once `receiver_too_large` has passed, and a decay has
    // energy in its inner blocks. frozen2 and the fuzz in make_fixture.py never reach it; the
    // port keeps the guard, as frozen2 does.
    let rows = fixture();
    assert!(
        rows.iter()
            .any(|r| r.t_arrival.is_none() && r.status != "refused")
    );
}
