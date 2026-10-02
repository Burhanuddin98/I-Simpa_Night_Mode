//! T20 part 2's shim (docs/investigations/2026-10-02-t20/PREREG-S1S2.md, "What is tested"): the
//! product's own T20, `params::decay::evaluate` → `BandParameters.t20`, which `parameters.t20_s`
//! reports through `params::noise::values` (noise.rs:728-746), on noise-free histograms the harness
//! writes (`docs/investigations/2026-10-02-t20/harness/`). Test-only and ignored: it runs when the
//! harness asks for it, never in the default run. The fixture pattern is `edt_port.rs`'s.
//!
//! Input, the folder `T20_SHIM_DIR`: `bins.bin`, every row's bins as dense f64 little-endian, and
//! `manifest.jsonl`, one row per line: `id`, `dt` (s), `offset` and `n` (bytes into `bins.bin`,
//! bins), `arrival` (the direct sound's centre, s, the source's emission delay already rounded up
//! to a whole step by the harness as `results/spps.rs:51-56` `emission_s` does and added as
//! `SppsResults::arrival_from` does, `spps.rs:295-312`; `null` for a detected arrival),
//! `half_width` (s, `R/c`), and optionally `complete` (bool, default false).
//!
//! The series and the arrival are built as `results/report.rs` builds them for an SPPS receiver:
//! - `EnergySeries::new`, or `EnergySeries::complete` when the row says so (report.rs:987-991,
//!   `band_complete`), then `.with_early_reverberation_unresolved()` (report.rs:992, every SPPS
//!   series). The run's floor and lost share (report.rs:997-1015) are not applied: they come from
//!   SPPS's particle statistics, which a noise-free histogram has none of.
//! - `Arrival::spread(arrival, half_width)` (report.rs:1134-1137 `known_arrival`, whose half-width
//!   is `receiver_crossing_s() / 2 = R/c`, spps.rs:475-477), or `Arrival::Detected` for `null`.
//!
//! Output, `T20_SHIM_OUT` (default `<T20_SHIM_DIR>/t20.jsonl`): per row `id`, `t20` (s, or null),
//! `code` (the refusal's `ParamError::code`, or null), `why` (a `NotEvaluable`'s tag, or null),
//! `decay_arrival` ("known" or "detected", what the decay times were measured from), `span_s`.
//!
//! ```text
//! T20_SHIM_DIR=<dir> cargo test -p simpa-core --test t20_shim --release -- --ignored --nocapture
//! ```

use std::io::Write;
use std::path::PathBuf;

use serde_json::{Value, json};
use simpa_core::params::EnergySeries;
use simpa_core::params::decay::{self, Arrival};

struct Row {
    id: String,
    dt: f64,
    arrival: Option<f64>,
    half_width: f64,
    complete: bool,
    bins: Vec<f64>,
}

fn load(dir: &std::path::Path) -> Vec<Row> {
    let blob = std::fs::read(dir.join("bins.bin")).expect("bins.bin");
    let manifest = std::fs::read_to_string(dir.join("manifest.jsonl")).expect("manifest.jsonl");
    manifest
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| {
            let v: Value = serde_json::from_str(l).unwrap();
            let n = v["n"].as_u64().unwrap() as usize;
            let off = v["offset"].as_u64().unwrap() as usize;
            let raw = &blob[off..off + 8 * n];
            let bins = raw
                .as_chunks::<8>()
                .0
                .iter()
                .map(|c| f64::from_le_bytes(*c))
                .collect();
            Row {
                id: v["id"].as_str().unwrap().into(),
                dt: v["dt"].as_f64().unwrap(),
                arrival: v["arrival"].as_f64(),
                half_width: v["half_width"].as_f64().unwrap(),
                complete: v["complete"].as_bool().unwrap_or(false),
                bins,
            }
        })
        .collect()
}

fn one(r: &Row) -> Value {
    let series = if r.complete {
        EnergySeries::complete(r.dt, r.bins.clone())
    } else {
        EnergySeries::new(r.dt, r.bins.clone())
    }
    .map(EnergySeries::with_early_reverberation_unresolved);
    let series = match series {
        Ok(s) => s,
        Err(e) => {
            return json!({"id": r.id, "t20": null, "code": e.code(), "why": null,
                          "decay_arrival": null, "span_s": null});
        }
    };
    let arrival = r
        .arrival
        .map_or(Arrival::Detected, |t| Arrival::spread(t, r.half_width));
    let p = decay::evaluate(&series, arrival);
    let decay_arrival = match p.decay_arrival {
        Arrival::Known { .. } => "known",
        Arrival::Detected => "detected",
    };
    match p.t20 {
        Ok(f) => json!({"id": r.id, "t20": f.t_s, "code": null, "why": null,
                        "decay_arrival": decay_arrival, "span_s": f.span_s}),
        Err(e) => {
            let why = e
                .not_evaluable()
                .and_then(|w| serde_json::to_value(w).ok())
                .and_then(|v| v["why"].as_str().map(String::from));
            json!({"id": r.id, "t20": null, "code": e.code(), "why": why,
                   "decay_arrival": decay_arrival, "span_s": null})
        }
    }
}

#[test]
#[ignore = "T20 part 2's shim: run by docs/investigations/2026-10-02-t20/harness with T20_SHIM_DIR set"]
fn t20_shim() {
    let Some(dir) = std::env::var_os("T20_SHIM_DIR").map(PathBuf::from) else {
        println!("SKIPPED: T20_SHIM_DIR is not set");
        return;
    };
    let out = std::env::var_os("T20_SHIM_OUT").map_or_else(|| dir.join("t20.jsonl"), PathBuf::from);
    let rows = load(&dir);
    let mut f = std::io::BufWriter::new(std::fs::File::create(&out).expect("output"));
    for r in &rows {
        writeln!(f, "{}", one(r)).unwrap();
    }
    f.flush().unwrap();
    println!("T20 SHIM: {} rows -> {}", rows.len(), out.display());
}

/// The shim's own wiring on a pure exponential with a direct impulse: T20 is the decay's T to
/// the product's 0.5 % (a check of the shim, not of the product).
#[test]
fn the_shim_reads_a_pure_exponential() {
    let dt = 1e-3;
    let t60 = 1.0;
    let k = 6.0 * std::f64::consts::LN_10 / t60;
    let mut bins: Vec<f64> = (0..3000)
        .map(|n| {
            let (a, b) = (n as f64 * dt, (n + 1) as f64 * dt);
            if a < 0.0105 {
                0.0
            } else {
                ((-k * (a.max(0.0105) - 0.0105)).exp() - (-k * (b - 0.0105)).exp()) / k
            }
        })
        .collect();
    bins[10] += 0.05;
    let r = Row {
        id: "exp".into(),
        dt,
        arrival: Some(0.0105),
        half_width: 0.0,
        complete: false,
        bins,
    };
    let v = one(&r);
    let t20 = v["t20"].as_f64().unwrap_or_else(|| panic!("{v}"));
    assert!((t20 / t60 - 1.0).abs() < 0.005, "{v}");
    assert_eq!(v["decay_arrival"], "known");
}
