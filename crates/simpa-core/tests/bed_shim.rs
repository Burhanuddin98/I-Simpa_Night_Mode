//! The bed's shim (docs/investigations/2026-10-02-bed/PREREG.md, set A): T20 part 2's shim
//! (`t20_shim.rs`) extended to every metric the bed scores. The product's own code,
//! `params::decay::evaluate` → `BandParameters`' `t20`, `t30`, `ts_s`, `c50_db`, `c80_db`, `d50`
//! and `spl_db`, on noise-free histograms the harness writes
//! (`docs/investigations/2026-10-02-bed/harness/`). Test-only and ignored: it runs when the
//! harness asks for it, never in the default run.
//!
//! Input, the folder `BED_SHIM_DIR`: `bins.bin` and `manifest.jsonl`, exactly `t20_shim.rs`'s
//! format (`id`, `dt`, `offset`, `n`, `arrival` or `null`, `half_width`, optional `complete`).
//! The series and the arrival are built as `t20_shim.rs` builds them, which is how
//! `results/report.rs` builds them for an SPPS receiver: `EnergySeries::new` (or `complete`),
//! `.with_early_reverberation_unresolved()`, and `Arrival::spread(arrival, R/c)`. No floor and no
//! lost share: a noise-free histogram has no particle statistics.
//!
//! Output, `BED_SHIM_OUT` (default `<BED_SHIM_DIR>/bed.jsonl`): per row `id`, `decay_arrival`,
//! `arrival` ("known" or "detected", what C, D and Ts were measured from), and for each metric
//! `m` of `t20`, `t30`, `ts`, `c50`, `c80`, `d50`, `spl`: `m` (the value: s, s, s, dB, dB, a
//! fraction, dB re (20 µPa)² on the bins' own units) or null, and `m_code` (the refusal's
//! `ParamError::code`, with a `NotEvaluable`'s tag after a colon) or null. Since build F also
//! `m_status`, the status a noise-free value is shown with: `wide` for a C50, C80 or D50 whose bin
//! straddling te can move it beyond its limit (`decay::Straddle`), `ok` for every other value, null
//! for a refusal; and for C50, C80 and D50 `m_lo`, `m_hi`, that bracket (null when refused).
//!
//! ```text
//! BED_SHIM_DIR=<dir> cargo test -p simpa-core --test bed_shim --release -- --ignored --nocapture
//! ```

use std::io::Write;
use std::path::PathBuf;

use serde_json::{Map, Value, json};
use simpa_core::params::decay::{self, Arrival};
use simpa_core::params::{EnergySeries, ParamError};

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

const METRICS: [&str; 7] = ["t20", "t30", "ts", "c50", "c80", "d50", "spl"];

fn code(e: &ParamError) -> String {
    let why = e
        .not_evaluable()
        .and_then(|w| serde_json::to_value(w).ok())
        .and_then(|v| v["why"].as_str().map(String::from));
    match why {
        Some(w) => format!("{}:{}", e.code(), w),
        None => e.code().to_string(),
    }
}

fn put(m: &mut Map<String, Value>, name: &str, r: Result<f64, &ParamError>) {
    match r {
        Ok(v) => {
            m.insert(name.into(), json!(v));
            m.insert(format!("{name}_code"), Value::Null);
            m.insert(format!("{name}_status"), json!("ok"));
        }
        Err(e) => {
            m.insert(name.into(), Value::Null);
            m.insert(format!("{name}_code"), json!(code(e)));
            m.insert(format!("{name}_status"), Value::Null);
        }
    }
}

/// `m_status`, `m_lo` and `m_hi` from C50's, C80's or D50's straddle bracket.
fn put_straddle(m: &mut Map<String, Value>, name: &str, s: Option<decay::Straddle>) {
    let (lo, hi) = s.map_or((Value::Null, Value::Null), |s| (json!(s.lo), json!(s.hi)));
    m.insert(format!("{name}_lo"), lo);
    m.insert(format!("{name}_hi"), hi);
    if let Some(s) = s
        && s.beyond_limit
    {
        m.insert(format!("{name}_status"), json!("wide"));
    }
}

fn kind(a: &Arrival) -> &'static str {
    match a {
        Arrival::Known { .. } => "known",
        Arrival::Detected => "detected",
    }
}

fn one(r: &Row) -> Value {
    let mut m = Map::new();
    m.insert("id".into(), json!(r.id));
    let series = if r.complete {
        EnergySeries::complete(r.dt, r.bins.clone())
    } else {
        EnergySeries::new(r.dt, r.bins.clone())
    }
    .map(EnergySeries::with_early_reverberation_unresolved);
    let series = match series {
        Ok(s) => s,
        Err(e) => {
            m.insert("decay_arrival".into(), Value::Null);
            m.insert("arrival".into(), Value::Null);
            for name in METRICS {
                put(&mut m, name, Err(&e));
            }
            for name in ["c50", "c80", "d50"] {
                put_straddle(&mut m, name, None);
            }
            return Value::Object(m);
        }
    };
    let arrival = r
        .arrival
        .map_or(Arrival::Detected, |t| Arrival::spread(t, r.half_width));
    let p = decay::evaluate(&series, arrival);
    m.insert("decay_arrival".into(), json!(kind(&p.decay_arrival)));
    m.insert("arrival".into(), json!(kind(&p.arrival)));
    put(&mut m, "t20", p.t20.as_ref().map(|f| f.t_s));
    put(&mut m, "t30", p.t30.as_ref().map(|f| f.t_s));
    put(&mut m, "ts", p.ts_s.as_ref().copied());
    put(&mut m, "c50", p.c50_db.as_ref().copied());
    put(&mut m, "c80", p.c80_db.as_ref().copied());
    put(&mut m, "d50", p.d50.as_ref().copied());
    put(&mut m, "spl", p.spl_db.as_ref().copied());
    put_straddle(&mut m, "c50", p.c50_straddle);
    put_straddle(&mut m, "c80", p.c80_straddle);
    put_straddle(&mut m, "d50", p.d50_straddle);
    Value::Object(m)
}

#[test]
#[ignore = "the bed's shim: run by docs/investigations/2026-10-02-bed/harness with BED_SHIM_DIR set"]
fn bed_shim() {
    let Some(dir) = std::env::var_os("BED_SHIM_DIR").map(PathBuf::from) else {
        println!("SKIPPED: BED_SHIM_DIR is not set");
        return;
    };
    let out = std::env::var_os("BED_SHIM_OUT").map_or_else(|| dir.join("bed.jsonl"), PathBuf::from);
    let rows = load(&dir);
    let mut f = std::io::BufWriter::new(std::fs::File::create(&out).expect("output"));
    for r in &rows {
        writeln!(f, "{}", one(r)).unwrap();
    }
    f.flush().unwrap();
    println!("BED SHIM: {} rows -> {}", rows.len(), out.display());
}

/// The shim's own wiring on a direct impulse plus a pure exponential from the arrival: every metric
/// is its closed form (a check of the shim, not of the product; the product's own tests hold the
/// bounds). T60 1 s, k = 6 ln 10 / T60; direct Ed; reverberant total 1/k.
#[test]
fn the_shim_reads_a_pure_exponential() {
    let dt = 1e-3;
    let t60 = 1.0;
    let ta = 0.0105;
    let k = 6.0 * std::f64::consts::LN_10 / t60;
    let ed = 0.05;
    let mut bins: Vec<f64> = (0..12000)
        .map(|n| {
            let (a, b) = (n as f64 * dt, (n + 1) as f64 * dt);
            if b <= ta {
                0.0
            } else {
                ((-k * (a.max(ta) - ta)).exp() - (-k * (b - ta)).exp()) / k
            }
        })
        .collect();
    bins[10] += ed;
    let r = Row {
        id: "exp".into(),
        dt,
        arrival: Some(ta),
        half_width: 0.0,
        complete: true,
        bins,
    };
    let v = one(&r);
    let total = ed + 1.0 / k;
    let late = |te: f64| (-k * te).exp() / k;
    let c = |te: f64| 10.0 * ((total - late(te)) / late(te)).log10();
    let ts = (1.0 / (k * k)) / total;
    let spl = 10.0 * (total / 4e-10).log10();
    let near = |name: &str, want: f64, tol: f64| {
        let got = v[name].as_f64().unwrap_or_else(|| panic!("{name}: {v}"));
        assert!((got - want).abs() <= tol, "{name}: {got} vs {want}: {v}");
    };
    near("t20", t60, 0.005 * t60);
    near("c50", c(0.05), 0.01);
    near("c80", c(0.08), 0.01);
    near("d50", (total - late(0.05)) / total, 0.001);
    near("ts", ts, 1e-4);
    near("spl", spl, 0.01);
    assert_eq!(v["arrival"], "known");
    // At 1 ms the bins straddling 50 and 80 ms hold under 2 % of the late energy (T60 1 s): ok.
    for name in ["t20", "c50", "c80", "d50", "spl"] {
        assert_eq!(v[format!("{name}_status")], "ok", "{name}: {v}");
    }
    assert!(v["c50_lo"].as_f64().unwrap() <= v["c50"].as_f64().unwrap());
}

/// The bed's STI shim (docs/investigations/2026-10-02-bed/ADDENDUM-2-STI.md, set A; ADDENDUM-3.md
/// item 6): the product's `sti::receiver_sti` on noise-free seven-band receivers, each band built
/// as `results/report.rs` builds an SPPS receiver's band: the series complete, SPL, T30 and T20
/// from `decay::evaluate` with `Arrival::spread(t, R/c)`, EDT from `edt::analyse`, the reverberation
/// time the first of T30, T20, EDT, and `from` the bin of the direct sound's leading edge.
///
/// Input, `BED_SHIM_DIR`: `bins.bin` (as above) and `sti_manifest.jsonl`, per row `id`, `dt`,
/// `arrival`, `half_width` and `bands`, each `{freq_hz, offset, n, power_rho_c, noise_db}`
/// (`noise_db` null for none; bands may share an offset). Output, `BED_SHIM_STI_OUT` (default
/// `<BED_SHIM_DIR>/sti.jsonl`): per row `id`, `male` and `female` (the value or null), their
/// `*_untruncated` and `*_code` (the refusal's code, with a `NotEvaluable`'s tag after a colon),
/// and `bands`, each `{freq_hz, from, spl_db, reverberation_s, transfer_db, speech_male_db,
/// speech_female_db, noise_db, mti_male, mti_female}`.
///
/// ```text
/// BED_SHIM_DIR=<dir> cargo test -p simpa-core --test bed_shim --release -- --ignored --exact bed_shim_sti --nocapture
/// ```
#[test]
#[ignore = "the bed's STI shim: run by docs/investigations/2026-10-02-bed/harness/stibed.py"]
fn bed_shim_sti() {
    use simpa_core::params::{edt, sti};
    let Some(dir) = std::env::var_os("BED_SHIM_DIR").map(PathBuf::from) else {
        println!("SKIPPED: BED_SHIM_DIR is not set");
        return;
    };
    let out =
        std::env::var_os("BED_SHIM_STI_OUT").map_or_else(|| dir.join("sti.jsonl"), PathBuf::from);
    let blob = std::fs::read(dir.join("bins.bin")).expect("bins.bin");
    let manifest = std::fs::read_to_string(dir.join("sti_manifest.jsonl")).expect("sti_manifest");
    let mut f = std::io::BufWriter::new(std::fs::File::create(&out).expect("output"));
    let mut rows = 0;
    for line in manifest.lines().filter(|l| !l.trim().is_empty()) {
        let v: Value = serde_json::from_str(line).unwrap();
        let dt = v["dt"].as_f64().unwrap();
        let t = v["arrival"].as_f64().unwrap();
        let hw = v["half_width"].as_f64().unwrap();
        let arrival = Arrival::spread(t, hw);
        let from = ((t - hw) / dt).floor().max(0.0) as usize;
        struct Band {
            freq_hz: i32,
            bins: Vec<f64>,
            spl: Result<f64, String>,
            power_rho_c: f64,
            noise_db: Option<f64>,
            reverberation_s: Option<f64>,
            unusable: Option<String>,
        }
        let bands: Vec<Band> = v["bands"]
            .as_array()
            .unwrap()
            .iter()
            .map(|b| {
                let n = b["n"].as_u64().unwrap() as usize;
                let off = b["offset"].as_u64().unwrap() as usize;
                let bins: Vec<f64> = blob[off..off + 8 * n]
                    .as_chunks::<8>()
                    .0
                    .iter()
                    .map(|c| f64::from_le_bytes(*c))
                    .collect();
                let series = EnergySeries::complete(dt, bins.clone())
                    .map(EnergySeries::with_early_reverberation_unresolved);
                let (spl, reverberation_s, unusable) = match &series {
                    Ok(s) => {
                        let p = decay::evaluate(s, arrival);
                        let e = edt::analyse(&bins, dt, Some(t), Some(hw));
                        let edt_s = e.edt.filter(|_| e.status != edt::Status::Refused);
                        let rt = p
                            .t30
                            .as_ref()
                            .ok()
                            .map(|x| x.t_s)
                            .or(p.t20.as_ref().ok().map(|x| x.t_s))
                            .or(edt_s);
                        (
                            p.spl_db.as_ref().copied().map_err(|e| e.to_string()),
                            rt,
                            None,
                        )
                    }
                    Err(e) => (
                        Err(e.to_string()),
                        None,
                        Some(format!("its series is refused: {e}")),
                    ),
                };
                Band {
                    freq_hz: b["freq_hz"].as_i64().unwrap() as i32,
                    bins,
                    spl,
                    power_rho_c: b["power_rho_c"].as_f64().unwrap(),
                    noise_db: b["noise_db"].as_f64(),
                    reverberation_s,
                    unusable,
                }
            })
            .collect();
        let inputs: Vec<sti::ReceiverBand<'_>> = bands
            .iter()
            .map(|b| sti::ReceiverBand {
                freq_hz: b.freq_hz,
                energy: &b.bins,
                from,
                spl_db: b.spl.clone(),
                power_rho_c: b.power_rho_c,
                noise_db: b.noise_db,
                reverberation_s: b.reverberation_s,
                unusable: b.unusable.clone(),
            })
            .collect();
        let r = sti::receiver_sti(dt, &inputs, true);
        let mut m = Map::new();
        m.insert("id".into(), v["id"].clone());
        for (name, s) in [("male", &r.male), ("female", &r.female)] {
            match s {
                Ok(x) => {
                    m.insert(name.into(), json!(x.value));
                    m.insert(format!("{name}_untruncated"), json!(x.untruncated));
                    m.insert(format!("{name}_code"), Value::Null);
                }
                Err(e) => {
                    m.insert(name.into(), Value::Null);
                    m.insert(format!("{name}_untruncated"), Value::Null);
                    m.insert(format!("{name}_code"), json!(code(e)));
                }
            }
        }
        let per: Vec<Value> = r
            .bands
            .iter()
            .map(|b| {
                let src = bands.iter().find(|x| x.freq_hz == b.freq_hz).unwrap();
                json!({
                    "freq_hz": b.freq_hz, "from": from, "spl_db": src.spl.as_ref().ok(),
                    "reverberation_s": src.reverberation_s, "transfer_db": b.transfer_db,
                    "speech_male_db": b.speech_male_db, "speech_female_db": b.speech_female_db,
                    "noise_db": b.noise_db, "mti_male": b.mti_male, "mti_female": b.mti_female,
                })
            })
            .collect();
        m.insert("bands".into(), Value::Array(per));
        writeln!(f, "{}", Value::Object(m)).unwrap();
        rows += 1;
    }
    f.flush().unwrap();
    println!("BED SHIM STI: {rows} rows -> {}", out.display());
}
