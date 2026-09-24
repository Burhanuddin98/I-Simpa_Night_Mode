//! Evidence for M8's design, gathered by the M7 follow-ups (`docs/results.md`, "What M8 needs").
//! Each test runs SPPS many times and prints what it measured; none is a gate, so each is
//! ignored and run on purpose:
//!
//! `cargo test --release -p simpa --test m8_evidence -- --ignored --nocapture <name>`
//!
//! The run folders go under `$SIMPA_EVIDENCE_ROOT` when it is set, else under cargo's test scratch
//! space. `$SIMPA_EVIDENCE_JOBS` (default 8) is how many solver runs go at once: a seeded SPPS run
//! is single-threaded.
//!
//! - `arrival_outside_the_onset_bin_over_receiver_positions`: tutorial 1 at upstream's defaults,
//!   200 receivers at random positions: how often `r/c` falls outside the onset bin, and what the
//!   decay times do there.
//! - `energetic_noise_against_ten_seeds`: tutorial 1 in energetic mode, ten seeds at 150,000 and
//!   1,500,000 particles: the Monte-Carlo estimate against the spread over the seeds, and which of
//!   the floor and the lost particles refuses T20 and T30.
//! - `energetic_lost_particles_from_saved_trajectories`: tutorial 1 in energetic mode with every
//!   trajectory saved: the energy lost particles carried, and what they would have moved T30 by.
//!   `$SIMPA_LOST_FROM` reads an earlier run's folder again.
//! - `energetic_floor_against_a_lower_floor`: T30 at `trans_epsilon` 5 against 9, ten seeds.
//! - `m8_cells`: M8's rooms and computation methods at several particle counts and durations
//!   (`$SIMPA_M8_CELLS`): which quantities the refusal limits let through, the spread of T30 over
//!   three seeds, T30 against Eyring, and the wall time. `$SIMPA_M8_FROM` reads an earlier run's
//!   folder again.

mod support;

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use serde_json::Value;
use simpa_core::params::EnergySeries;
use simpa_core::params::decay::Arrival;
use simpa_core::params::noise::{self, NoiseModel};
use simpa_core::schema::{
    self, BandKind, BandSet, ComputationMethod, Face, GroupId, MaterialId, PointReceiverId,
    Project, ReflectionLaw, Vec3,
};
use support::*;

/// Where the evidence runs go.
fn evidence_root(label: &str) -> PathBuf {
    match std::env::var_os("SIMPA_EVIDENCE_ROOT") {
        Some(d) if !d.is_empty() => {
            let stamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs();
            let p = PathBuf::from(d).join(format!("{label}-{stamp}"));
            std::fs::create_dir_all(&p).unwrap();
            p
        }
        _ => scratch(label),
    }
}

fn jobs() -> usize {
    std::env::var("SIMPA_EVIDENCE_JOBS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(8)
}

/// One finished run: its name, wall time and `simpa results --json`, or the refusal `simpa
/// results` printed.
struct Done {
    name: String,
    secs: f64,
    report: Value,
}

impl Done {
    fn refused(&self) -> bool {
        self.report.get("refused").is_some()
    }
}

/// The reports that were not refused; the refused ones are printed.
fn accepted(done: Vec<Done>) -> Vec<Done> {
    let (bad, good): (Vec<Done>, Vec<Done>) = done.into_iter().partition(Done::refused);
    for d in &bad {
        println!("REFUSED {}: {}", d.name, d.report["refused"]);
    }
    good
}

/// Runs every project through SPPS, `jobs()` at a time, each in its own runs folder, and reads its
/// results. A run that is not OK fails the test; results that `simpa results` refuses are kept as
/// the refusal (exit 6), so that one refused run is counted rather than ending the evidence.
fn run_all(root: &Path, projects: Vec<(String, Project)>) -> Vec<Done> {
    solver_exe("spps.exe");
    let next = AtomicUsize::new(0);
    let out: Mutex<Vec<(usize, Done)>> = Mutex::new(Vec::new());
    let clock = std::time::Instant::now();
    std::thread::scope(|scope| {
        for _ in 0..jobs().min(projects.len()) {
            scope.spawn(|| {
                loop {
                    let i = next.fetch_add(1, Ordering::SeqCst);
                    let Some((name, p)) = projects.get(i) else {
                        break;
                    };
                    let dir = root.join(format!("{i:03}"));
                    std::fs::create_dir_all(&dir).unwrap();
                    let path = dir.join("project.simpa");
                    schema::save(p, &path).unwrap();
                    let t0 = std::time::Instant::now();
                    let o = simpa_run(&[
                        "run".to_string(),
                        path.display().to_string(),
                        "--solver".into(),
                        "spps".into(),
                        "--runs".into(),
                        dir.join("runs").display().to_string(),
                        "--json".into(),
                    ]);
                    let secs = t0.elapsed().as_secs_f64();
                    let m = json(&o);
                    assert_eq!(o.code, 0, "{name}: {o:#?}");
                    assert_eq!(m["verdict"]["status"], "OK", "{name}");
                    let run = run_dir(&m);
                    let r = simpa_run(&[
                        "results".to_string(),
                        run.display().to_string(),
                        "--json".into(),
                    ]);
                    assert!(r.code == 0 || r.code == 6, "{name}: {r:#?}");
                    let report = json(&r);
                    println!(
                        "[{:>6.0} s] {name}: {secs:.1} s",
                        clock.elapsed().as_secs_f64()
                    );
                    out.lock().unwrap().push((
                        i,
                        Done {
                            name: name.clone(),
                            secs,
                            report,
                        },
                    ));
                }
            });
        }
    });
    let mut v = out.into_inner().unwrap();
    v.sort_by_key(|(i, _)| *i);
    v.into_iter().map(|(_, d)| d).collect()
}

/// Tutorial 1's box (`rooms/tutorial1_box.simpa`, upstream's defaults) on the octave bands 125 Hz
/// to 4 kHz, without its surface receiver and the intersection logs, seeded `seed`.
fn tutorial_octaves(seed: u32) -> Project {
    let mut p = schema::load(&fixture("rooms/tutorial1_box.simpa")).unwrap();
    let all = p.bands.frequencies_hz.clone();
    let keep: Vec<u32> = vec![125, 250, 500, 1000, 2000, 4000];
    let pick = |v: &[schema::F64]| -> Vec<schema::F64> {
        keep.iter()
            .map(|f| v[all.iter().position(|a| a == f).unwrap()])
            .collect()
    };
    for m in &mut p.materials {
        m.absorption = pick(&m.absorption);
        m.scattering = pick(&m.scattering);
    }
    p.bands = BandSet::range(BandKind::Octave, 125, 4000).unwrap();
    p.surface_receivers.clear();
    let s = &mut p.solvers.spps;
    s.random_seed = seed;
    s.save_surface_intersections = false;
    s.save_receiver_intersections = false;
    s.bands_computed = vec![true; keep.len()];
    p.solvers.tcr.bands_computed = vec![true; keep.len()];
    p
}

/// `p`'s point receivers replaced by one per position, named `R000`, `R001`, ...
fn with_receivers(p: &mut Project, at: &[[f64; 3]]) {
    let proto = p.point_receivers[0].clone();
    p.point_receivers = at
        .iter()
        .enumerate()
        .map(|(i, x)| {
            let mut r = proto.clone();
            r.id =
                PointReceiverId::from_u128(0x0c0b_e000_0000_4000_8000_0000_0001_0000 + i as u128);
            r.name = format!("R{i:03}");
            r.position = Vec3::new(x[0], x[1], x[2]);
            r
        })
        .collect();
}

/// SplitMix64, for reproducible positions.
struct Rng(u64);
impl Rng {
    fn uniform(&mut self) -> f64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        ((z ^ (z >> 31)) >> 11) as f64 / (1u64 << 53) as f64
    }
}

fn mean_sd(x: &[f64]) -> (f64, f64) {
    let n = x.len() as f64;
    let m = x.iter().sum::<f64>() / n;
    let v = x.iter().map(|v| (v - m).powi(2)).sum::<f64>() / (n - 1.0);
    (m, v.sqrt())
}

/// Why a parameter was refused, in a word: the code, or `params_not_evaluable`'s reason.
fn why(p: &Value) -> String {
    if p["value"].is_f64() {
        return "value".into();
    }
    let r = &p["not_evaluable"];
    match r["error"]["why"]["why"].as_str() {
        Some(w) => w.to_string(),
        None => r["code"].as_str().unwrap_or("?").to_string(),
    }
}

#[test]
#[ignore = "evidence for M8, not a gate: runs SPPS on tutorial 1 with 200 receivers; run on purpose"]
fn arrival_outside_the_onset_bin_over_receiver_positions() {
    let root = evidence_root("arrival-positions");
    // 200 positions uniform over the box, at least 0.5 m from every wall and 1 m from the source.
    let mut rng = Rng(0x4d37_a771_0000_0001);
    let source = [3.0, 5.0, 1.8];
    let mut at = Vec::new();
    while at.len() < 200 {
        let x = [
            0.5 + 5.0 * rng.uniform(),
            0.5 + 9.0 * rng.uniform(),
            0.5 + 2.0 * rng.uniform(),
        ];
        let d =
            ((x[0] - source[0]).powi(2) + (x[1] - source[1]).powi(2) + (x[2] - source[2]).powi(2))
                .sqrt();
        if d >= 1.0 {
            at.push(x);
        }
    }
    // Upstream's default step, and a step of 1 ms.
    let projects = [0.01, 0.001]
        .map(|dt| {
            let mut p = tutorial_octaves(1);
            with_receivers(&mut p, &at);
            p.solvers.spps.time_step_s = schema::F64::new(dt);
            (format!("tutorial 1, 200 receivers, dt {dt} s"), p)
        })
        .to_vec();
    for done in accepted(run_all(&root, projects)) {
        positions_report(&done.report["spps"]);
    }
}

/// How often `r/c` falls outside the onset bin in one run, and what the decay times and C80 do
/// there.
fn positions_report(rep: &Value) {
    let dt = rep["time_step_s"].as_f64().unwrap();
    let half = rep["receiver_crossing_s"].as_f64().unwrap() / 2.0;
    let (mut cells, mut after, mut before, mut positions_out) = (0, 0, 0, 0);
    let mut decay_after: std::collections::BTreeMap<String, usize> = Default::default();
    let mut c80_after: std::collections::BTreeMap<String, usize> = Default::default();
    let mut predicted = 0;
    for r in rep["point_receivers"].as_array().unwrap() {
        let t_a = r["arrival_s"].as_f64().unwrap();
        // The leading edge of the ball's crossing lies in the bin before r/c's.
        if (t_a / dt).floor() > ((t_a - half) / dt).floor() {
            predicted += 1;
        }
        let mut any = false;
        for b in r["bands"].as_array().unwrap() {
            cells += 1;
            let o = &b["onset"];
            let (s, e) = (
                o["bin_start_s"].as_f64().unwrap(),
                o["bin_end_s"].as_f64().unwrap(),
            );
            let q = &b["parameters"];
            if t_a >= e {
                after += 1;
                any = true;
                for k in ["edt_s", "t20_s", "t30_s"] {
                    *decay_after
                        .entry(format!("{k} {}", why(&q[k])))
                        .or_default() += 1;
                }
                *c80_after.entry(why(&q["c80_db"])).or_default() += 1;
            } else if t_a < s - 1e-9 * dt {
                before += 1;
                any = true;
            }
        }
        positions_out += usize::from(any);
    }
    let n = rep["point_receivers"].as_array().unwrap().len();
    println!(
        "{n} receivers x 6 bands = {cells} receiver-bands at dt {dt} s, R/c {:.4} ms: r/c after \
         the onset bin in {after} ({:.1} %), before it in {before}; receivers with any: \
         {positions_out} ({:.1} %). The leading edge R/c before r/c falls in the bin before in \
         {predicted} receivers ({:.1} %; R/(c dt) = {:.1} %)",
        1000.0 * half,
        100.0 * after as f64 / cells as f64,
        100.0 * positions_out as f64 / n as f64,
        100.0 * predicted as f64 / n as f64,
        100.0 * half / dt
    );
    println!("decay times where r/c is after the onset bin: {decay_after:?}");
    println!("C80 there: {c80_after:?}");
    assert!(cells > 0);
}

/// Each of the eight quantities' value and estimated standard deviation, or `None`.
type Eight = [Option<(f64, f64)>; 8];

/// The value of each of the eight quantities and its estimated standard deviation from the series
/// alone, as the bootstrap takes its resamples: complete, nothing missing. `None` where the
/// quantity is refused for any reason but its noise.
fn plain_estimates(band: &Value, arrival: Arrival) -> Eight {
    let dt = band["dt"].as_f64().unwrap();
    let e: Vec<f64> = band["energy_pa2"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_f64().unwrap())
        .collect();
    let d = band["noise_model"]["mean_deposit"].as_f64().unwrap();
    let p = noise::evaluate(
        &EnergySeries::complete(dt, e),
        arrival,
        &NoiseModel::crossings(d).unwrap(),
    );
    let one = |r: &Result<noise::Estimate, simpa_core::params::ParamError>| match r {
        Ok(x) => Some((x.value, x.sd)),
        Err(e) => match e.not_evaluable() {
            Some(simpa_core::params::NotEvaluable::MonteCarloNoise {
                value,
                sd: Some(sd),
                ..
            }) => Some((*value, *sd)),
            _ => None,
        },
    };
    [
        one(&p.spl_db),
        one(&p.edt_s),
        one(&p.t20_s),
        one(&p.t30_s),
        one(&p.c50_db),
        one(&p.c80_db),
        one(&p.d50),
        one(&p.ts_s),
    ]
}

const EIGHT: [&str; 8] = [
    "spl_db", "edt_s", "t20_s", "t30_s", "c50_db", "c80_db", "d50", "ts_s",
];

/// Six receivers in tutorial 1's box: its own two and four more, each at least 1 m from the walls
/// and the source.
const SIX: [[f64; 3]; 6] = [
    [1.0, 1.0, 1.8],
    [3.0, 7.0, 1.8],
    [5.0, 8.5, 1.2],
    [1.2, 8.0, 1.5],
    [4.8, 2.0, 2.0],
    [2.0, 3.0, 1.0],
];

/// For every receiver, band and quantity with a value in every report: the spread of the values
/// over the reports against the root-mean-square of the estimates, pooled per quantity.
fn spread_against_estimate(label: &str, reports: &[Value]) -> Vec<(String, usize, f64, f64)> {
    let n = reports.len();
    let rs = |rep: &Value| rep["spps"]["point_receivers"].as_array().unwrap().clone();
    let first = rs(&reports[0]);
    let dt = reports[0]["spps"]["time_step_s"].as_f64().unwrap();
    let half = reports[0]["spps"]["receiver_crossing_s"].as_f64().unwrap() / 2.0;
    // Each report's estimates, per receiver and band.
    let estimates: Vec<Vec<Vec<Eight>>> = reports
        .iter()
        .map(|rep| {
            rs(rep)
                .iter()
                .map(|r| {
                    let t_a = r["arrival_s"].as_f64().unwrap();
                    r["bands"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|b| {
                            let mut b = b.clone();
                            b["dt"] = dt.into();
                            plain_estimates(&b, Arrival::spread(t_a, half))
                        })
                        .collect()
                })
                .collect()
        })
        .collect();
    let mut out = Vec::new();
    for (qi, q) in EIGHT.iter().enumerate() {
        let (mut spread2, mut model2, mut cells, mut short) = (0.0, 0.0, 0, 0);
        let mut ratios = Vec::new();
        for (ri, r) in first.iter().enumerate() {
            for bi in 0..r["bands"].as_array().unwrap().len() {
                let got: Vec<(f64, f64)> = estimates.iter().filter_map(|e| e[ri][bi][qi]).collect();
                if got.len() < n {
                    short += 1;
                    continue;
                }
                let values: Vec<f64> = got.iter().map(|g| g.0).collect();
                let (_, spread) = mean_sd(&values);
                let model = (got.iter().map(|g| g.1 * g.1).sum::<f64>() / n as f64).sqrt();
                spread2 += spread * spread;
                model2 += model * model;
                ratios.push(spread / model);
                cells += 1;
            }
        }
        ratios.sort_by(f64::total_cmp);
        let pooled = (spread2 / model2).sqrt();
        if cells > 0 {
            println!(
                "{label} {q:<7}: {cells} receiver-bands, spread/estimate pooled {pooled:.2}, \
                 median {:.2}, from {:.2} to {:.2}; {short} left out",
                ratios[cells / 2],
                ratios[0],
                ratios[cells - 1]
            );
        } else {
            println!("{label} {q:<7}: no receiver-band with a value in every seed");
        }
        out.push((
            q.to_string(),
            cells,
            pooled,
            (spread2 / cells.max(1) as f64).sqrt(),
        ));
    }
    out
}

/// Which of the two missing-energy bounds refuses T20 and T30 in energetic mode: each series
/// evaluated with the solver's floor alone, the lost particles' share alone, and both, as
/// `core::results` builds them (`report::series_of`).
fn missing_decomposition(label: &str, reports: &[Value]) {
    let mut counts: std::collections::BTreeMap<String, usize> = Default::default();
    for rep in reports {
        let s = &rep["spps"];
        let dt = s["time_step_s"].as_f64().unwrap();
        let half = s["receiver_crossing_s"].as_f64().unwrap() / 2.0;
        let eps = s["trans_epsilon"].as_f64().unwrap();
        let n = s["particles_per_source"].as_f64().unwrap();
        let stats = s["particles"]["bands"].as_array().unwrap();
        for r in s["point_receivers"].as_array().unwrap() {
            let t_a = r["arrival_s"].as_f64().unwrap();
            let bin = (t_a / dt).floor() as usize;
            for (bi, b) in r["bands"].as_array().unwrap().iter().enumerate() {
                let e: Vec<f64> = b["energy_pa2"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|v| v.as_f64().unwrap())
                    .collect();
                let room = s["total_energy"][bi]["energy"][bin].as_f64().unwrap();
                let alive = room / b["source_power_rho_c"].as_f64().unwrap();
                let st = &stats[bi];
                let lost = st["lost_by_infinite_loops"].as_f64().unwrap()
                    + st["lost_by_meshing_problems"].as_f64().unwrap();
                let base = EnergySeries::new(dt, e).unwrap();
                let floor = base.clone().with_solver_floor(-10.0 * eps, alive).unwrap();
                let lost_only = base.clone().with_lost_share(lost / (n * alive)).unwrap();
                let both = floor.clone().with_lost_share(lost / (n * alive)).unwrap();
                let arrival = Arrival::spread(t_a, half);
                for (name, series) in [
                    ("neither", &base),
                    ("floor", &floor),
                    ("lost", &lost_only),
                    ("both", &both),
                ] {
                    let p = simpa_core::params::decay::evaluate(series, arrival);
                    for (q, r) in [("t20", &p.t20), ("t30", &p.t30)] {
                        let w = match r {
                            Ok(_) => "value".to_string(),
                            Err(e) => e
                                .not_evaluable()
                                .map(|w| format!("{w}").split(':').next().unwrap().to_string())
                                .unwrap_or_else(|| e.code().to_string()),
                        };
                        *counts.entry(format!("{q} with {name}: {w}")).or_default() += 1;
                    }
                }
            }
        }
    }
    for (k, v) in counts {
        println!("{label} missing energy: {k}: {v}");
    }
}

/// How many receiver-bands refuse each quantity, and why, in the reports.
fn refusals(label: &str, reports: &[Value]) {
    for q in EIGHT {
        let mut why_count: std::collections::BTreeMap<String, usize> = Default::default();
        for rep in reports {
            for r in rep["spps"]["point_receivers"].as_array().unwrap() {
                for b in r["bands"].as_array().unwrap() {
                    *why_count.entry(why(&b["parameters"][q])).or_default() += 1;
                }
            }
        }
        println!("{label} {q:<7}: {why_count:?}");
    }
}

#[test]
#[ignore = "evidence for M8, not a gate: runs tutorial 1 in energetic mode over ten SPPS seeds at \
            two particle counts; run on purpose"]
fn energetic_noise_against_ten_seeds() {
    let counts: Vec<u32> = std::env::var("SIMPA_ENERGETIC_COUNTS")
        .unwrap_or_else(|_| "150000,1500000".into())
        .split(',')
        .map(|s| s.trim().parse().unwrap())
        .collect();
    for particles in counts {
        let root = evidence_root(&format!("energetic-seeds-{particles}"));
        let projects = (1..=10u32)
            .map(|seed| {
                let mut p = tutorial_octaves(seed);
                with_receivers(&mut p, &SIX);
                p.solvers.spps.method = ComputationMethod::Energetic;
                p.solvers.spps.particles_per_source = particles;
                (format!("energetic {particles} seed {seed}"), p)
            })
            .collect();
        let done = accepted(run_all(&root, projects));
        let secs: Vec<f64> = done.iter().map(|d| d.secs).collect();
        let reports: Vec<Value> = done.into_iter().map(|d| d.report).collect();
        let label = format!(
            "energetic, {particles} particles ({} seeds):",
            reports.len()
        );
        println!(
            "{label} wall time per run {:.0} to {:.0} s",
            secs.iter().copied().fold(f64::INFINITY, f64::min),
            secs.iter().copied().fold(0.0, f64::max)
        );
        refusals(&label, &reports);
        missing_decomposition(&label, &reports);
        spread_against_estimate(&label, &reports);
        for rep in &reports {
            println!("{label} statistics {}", rep["spps"]["particles"]["bands"]);
        }
    }
}

/// T30 of a series from the arrival, taken as complete (nothing added after its end), or `None`.
fn t30_plain(dt: f64, v: Vec<f64>, arrival: Arrival) -> Option<f64> {
    let s = EnergySeries::complete(dt, v).ok()?;
    simpa_core::params::decay::evaluate(&s, arrival)
        .t30
        .ok()
        .map(|f| f.t_s)
}

#[test]
#[ignore = "evidence for M8, not a gate: runs tutorial 1 in energetic mode saving every particle's \
            trajectory, about 0.7 GB per run; run on purpose"]
fn energetic_lost_particles_from_saved_trajectories() {
    let seeds: u32 = std::env::var("SIMPA_LOST_SEEDS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(3);
    let particles = 150_000u32;
    // `$SIMPA_LOST_FROM`: the folder an earlier run of this test wrote, read again instead of
    // running the solver (each run leaves 0.7 GB of trajectories).
    let done = match std::env::var_os("SIMPA_LOST_FROM") {
        Some(from) => {
            let mut done = Vec::new();
            let mut jobs: Vec<PathBuf> = std::fs::read_dir(&from)
                .unwrap()
                .map(|e| e.unwrap().path().join("runs"))
                .filter(|p| p.is_dir())
                .collect();
            jobs.sort();
            for runs in jobs {
                for run in std::fs::read_dir(&runs).unwrap() {
                    let run = run.unwrap().path();
                    let r = simpa_run(&[
                        "results".to_string(),
                        run.display().to_string(),
                        "--json".into(),
                    ]);
                    assert_eq!(r.code, 0, "{r:#?}");
                    done.push(Done {
                        name: run.display().to_string(),
                        secs: 0.0,
                        report: json(&r),
                    });
                }
            }
            done
        }
        None => {
            let root = evidence_root("energetic-lost");
            let projects = (1..=seeds)
                .map(|seed| {
                    let mut p = tutorial_octaves(seed);
                    with_receivers(&mut p, &SIX);
                    p.solvers.spps.method = ComputationMethod::Energetic;
                    p.solvers.spps.particles_per_source = particles;
                    p.solvers.spps.particles_saved = particles;
                    (format!("energetic {particles} saved, seed {seed}"), p)
                })
                .collect();
            accepted(run_all(&root, projects))
        }
    };
    let (mut lost_seen, mut lost_counted) = (0usize, 0usize);
    let mut rhos = Vec::new();
    let mut times = Vec::new();
    let (mut worst_measured, mut worst_bound) = (0.0f64, 0.0f64);
    let mut bound_over_measured = Vec::new();
    for d in &done {
        let s = &d.report["spps"];
        let solve = Path::new(d.report["run_folder"].as_str().unwrap()).join("solve");
        let dt = s["time_step_s"].as_f64().unwrap();
        let half = s["receiver_crossing_s"].as_f64().unwrap() / 2.0;
        let eps = s["trans_epsilon"].as_f64().unwrap();
        let steps = s["steps"].as_u64().unwrap() as usize;
        let n = f64::from(particles);
        for (bi, pf) in s["particle_files"].as_array().unwrap().iter().enumerate() {
            let file =
                simpa_core::formats::pbin::read_file(&solve.join(pf["path"].as_str().unwrap()))
                    .unwrap();
            let st = &s["particles"]["bands"][bi];
            lost_counted += (st["lost_by_infinite_loops"].as_u64().unwrap()
                + st["lost_by_meshing_problems"].as_u64().unwrap())
                as usize;
            let room: Vec<f64> = s["total_energy"][bi]["energy"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_f64().unwrap())
                .collect();
            let power = s["point_receivers"][0]["bands"][bi]["source_power_rho_c"]
                .as_f64()
                .unwrap();
            let e_max = file
                .iter()
                .filter_map(|(_, st)| st.first().map(|x| f64::from(x.energy)))
                .fold(0.0, f64::max);
            // Lost: stopped before the last step with more energy than a particle the floor drops
            // can hold at the end of its last recorded step: 10^-eps of its start over the
            // reflections of one step, (1 − α)^k with α at most 0.3. Measured on these runs, no
            // particle the floor dropped ended above 4.6·10^-5 of its start; 10^-4 is taken.
            let dropped_at_most = 10.0 * 10f64.powf(-eps);
            let mut lost: Vec<(usize, f64)> = Vec::new();
            for (h, st) in file.iter() {
                let last = usize::from(h.first_time_step) + st.len() - 1;
                let x = f64::from(st.last().unwrap().energy) / e_max;
                if last + 1 < steps && x > dropped_at_most {
                    lost.push((last, x));
                }
            }
            lost_seen += lost.len();
            for &(k, x) in &lost {
                let f = room[k] / power;
                rhos.push(x / f);
                times.push((k + 1) as f64 * dt);
            }
            // Per receiver: what the lost particles would still have brought, each by its own
            // energy and loss time, against the bound core::results applies.
            for r in s["point_receivers"].as_array().unwrap() {
                let t_a = r["arrival_s"].as_f64().unwrap();
                let v: Vec<f64> = r["bands"][bi]["energy_pa2"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|v| v.as_f64().unwrap())
                    .collect();
                let mut sums = vec![0.0; v.len() + 1];
                for k in (0..v.len()).rev() {
                    sums[k] = sums[k + 1] + v[k];
                }
                // m(k) at each bin edge: Σ x/(N f) · S(max(k, loss)).
                let m: Vec<f64> = (0..=v.len())
                    .map(|k| {
                        lost.iter()
                            .map(|&(kl, x)| {
                                let f = room[kl] / power;
                                x / (n * f) * sums[k.max((kl + 1).min(v.len()))]
                            })
                            .sum()
                    })
                    .collect();
                let with_measured: Vec<f64> =
                    (0..v.len()).map(|k| v[k] + m[k] - m[k + 1]).collect();
                let arrival = Arrival::spread(t_a, half);
                let (Some(t0), Some(t1)) = (
                    t30_plain(dt, v.clone(), arrival),
                    t30_plain(dt, with_measured, arrival),
                ) else {
                    continue;
                };
                let measured = (t1 / t0 - 1.0).abs();
                // The bound: n/(N f_a) of S(onset), added to every backward sum, as params
                // applies it (`with_lost_share`), with n the particles SPPS counted as lost.
                let bin = (t_a / dt).floor() as usize;
                let n_lost = st["lost_by_infinite_loops"].as_f64().unwrap()
                    + st["lost_by_meshing_problems"].as_f64().unwrap();
                let share = n_lost / (n * room[bin] / power);
                let onset = sums[bin];
                let bound = match EnergySeries::complete(dt, v.clone())
                    .unwrap()
                    .with_lost_share(share)
                    .map(|s| simpa_core::params::decay::evaluate(&s, arrival).t30)
                {
                    // Accepted: it moved T30 by less than the limit.
                    Ok(Ok(_)) => 0.0,
                    Ok(Err(e)) => match e.not_evaluable() {
                        Some(simpa_core::params::NotEvaluable::MissingMoves {
                            value,
                            with_missing: Some(w),
                            ..
                        }) => (w / value - 1.0).abs(),
                        _ => f64::INFINITY,
                    },
                    Err(_) => f64::INFINITY,
                };
                worst_measured = worst_measured.max(measured);
                worst_bound = worst_bound.max(bound);
                if !lost.is_empty() {
                    bound_over_measured.push(m[bin] / (share * onset));
                }
            }
        }
    }
    rhos.sort_by(f64::total_cmp);
    times.sort_by(f64::total_cmp);
    bound_over_measured.sort_by(f64::total_cmp);
    let q = |v: &[f64], p: f64| {
        v.get(((v.len() as f64 - 1.0) * p) as usize)
            .copied()
            .unwrap_or(f64::NAN)
    };
    println!(
        "{} runs x 6 bands: lost particles found in the trajectories {lost_seen}, counted by SPPS \
         {lost_counted}",
        done.len()
    );
    println!(
        "a lost particle's energy over the mean energy of the particles when it was lost: min \
         {:.3}, median {:.3}, 90 % {:.3}, max {:.3}",
        q(&rhos, 0.0),
        q(&rhos, 0.5),
        q(&rhos, 0.9),
        q(&rhos, 1.0)
    );
    println!(
        "when lost, s: min {:.3}, median {:.3}, max {:.3}",
        q(&times, 0.0),
        q(&times, 0.5),
        q(&times, 1.0)
    );
    println!(
        "what they would have brought from the arrival on, over the bound's: median {:.2e}, max \
         {:.2e}",
        q(&bound_over_measured, 0.5),
        q(&bound_over_measured, 1.0)
    );
    println!(
        "T30 moved, worst receiver-band: by the measured missing energy {:.2e}, by the bound \
         {:.2e} (limit 5e-3)",
        worst_measured, worst_bound
    );
}

#[test]
#[ignore = "evidence for M8, not a gate: runs tutorial 1 in energetic mode over ten seeds at two \
            floors; run on purpose"]
fn energetic_floor_against_a_lower_floor() {
    // What the floor at trans_epsilon 5 (upstream's default) costs T30, measured: the same seeds
    // with the floor 40 dB lower, against the bound params applies for the floor.
    let particles: u32 = std::env::var("SIMPA_FLOOR_PARTICLES")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(1_500_000);
    let root = evidence_root("energetic-floor");
    let mut projects = Vec::new();
    for eps in [5.0, 9.0] {
        for seed in 1..=10u32 {
            let mut p = tutorial_octaves(seed);
            with_receivers(&mut p, &SIX);
            p.solvers.spps.method = ComputationMethod::Energetic;
            p.solvers.spps.particles_per_source = particles;
            p.solvers.spps.extinction_exponent = schema::F64::new(eps);
            projects.push((format!("eps {eps} seed {seed}"), p));
        }
    }
    let done = run_all(&root, projects);
    // Per receiver-band: T30 from the series alone at each floor, per seed.
    let t30s = |eps: &str| -> Vec<Vec<Option<f64>>> {
        done.iter()
            .filter(|d| d.name.starts_with(&format!("eps {eps} ")) && !d.refused())
            .map(|d| {
                let s = &d.report["spps"];
                let dt = s["time_step_s"].as_f64().unwrap();
                let half = s["receiver_crossing_s"].as_f64().unwrap() / 2.0;
                s["point_receivers"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .flat_map(|r| {
                        let t_a = r["arrival_s"].as_f64().unwrap();
                        r["bands"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .map(|b| {
                                let v: Vec<f64> = b["energy_pa2"]
                                    .as_array()
                                    .unwrap()
                                    .iter()
                                    .map(|v| v.as_f64().unwrap())
                                    .collect();
                                t30_plain(dt, v, Arrival::spread(t_a, half))
                            })
                            .collect::<Vec<_>>()
                    })
                    .collect()
            })
            .collect()
    };
    let (high, low) = (t30s("5"), t30s("9"));
    let cells = high[0].len();
    let mut diffs = Vec::new();
    for c in 0..cells {
        let h: Vec<f64> = high.iter().filter_map(|s| s[c]).collect();
        let l: Vec<f64> = low.iter().filter_map(|s| s[c]).collect();
        if h.len() < 3 || l.len() < 3 {
            continue;
        }
        let (mh, sh) = mean_sd(&h);
        let (ml, sl) = mean_sd(&l);
        let se = (sh * sh / h.len() as f64 + sl * sl / l.len() as f64).sqrt();
        diffs.push((mh / ml - 1.0, se / ml));
    }
    let mean_diff = diffs.iter().map(|d| d.0).sum::<f64>() / diffs.len() as f64;
    let worst = diffs.iter().map(|d| d.0.abs()).fold(0.0, f64::max);
    let worst_z = diffs.iter().map(|d| (d.0 / d.1).abs()).fold(0.0, f64::max);
    println!(
        "{particles} particles: T30 at trans_epsilon 5 over T30 at 9, {} receiver-bands: mean \
         {:+.3} %, largest {:.3} %, largest in standard errors {worst_z:.1}",
        diffs.len(),
        100.0 * mean_diff,
        100.0 * worst
    );
    for d in done
        .iter()
        .filter(|d| d.name.starts_with("eps 5 ") && !d.refused())
        .take(1)
    {
        refusals("trans_epsilon 5:", std::slice::from_ref(&d.report));
        missing_decomposition("trans_epsilon 5:", std::slice::from_ref(&d.report));
    }
    for d in done
        .iter()
        .filter(|d| d.name.starts_with("eps 9 ") && !d.refused())
        .take(1)
    {
        refusals("trans_epsilon 9:", std::slice::from_ref(&d.report));
        missing_decomposition("trans_epsilon 9:", std::slice::from_ref(&d.report));
    }
    println!(
        "wall time: eps 5 {:.0} s, eps 9 {:.0} s per run",
        done.iter()
            .filter(|d| d.name.starts_with("eps 5 "))
            .map(|d| d.secs)
            .sum::<f64>()
            / 10.0,
        done.iter()
            .filter(|d| d.name.starts_with("eps 9 "))
            .map(|d| d.secs)
            .sum::<f64>()
            / 10.0
    );
}

/// Room geometry for M8: tutorial 1's box (6 x 10 x 3 m) or upstream's atmospheric-absorption
/// validation room (5 x 4 x 3 m, `Docs/validations/validation_atmospheric_absorption.rst`).
#[derive(Clone, Copy, Debug)]
enum Room {
    Tutorial,
    Validation,
}

impl Room {
    fn size(self) -> [f64; 3] {
        match self {
            Room::Tutorial => [6.0, 10.0, 3.0],
            Room::Validation => [5.0, 4.0, 3.0],
        }
    }

    /// The source: tutorial 1's, and the validation room's centre, a little off it so that it
    /// lies on no internal facet of a symmetric mesh.
    fn source(self) -> [f64; 3] {
        match self {
            Room::Tutorial => [3.0, 5.0, 1.8],
            Room::Validation => [2.52, 1.97, 1.53],
        }
    }

    /// Three receivers, each at least 1 m from the walls and the source (M8's matrix): tutorial
    /// 1's two and one more; the validation room's own (1, 1, 1) and two more.
    fn receivers(self) -> [[f64; 3]; 3] {
        match self {
            Room::Tutorial => [[1.0, 1.0, 1.8], [3.0, 7.0, 1.8], [5.0, 8.5, 1.2]],
            Room::Validation => [[1.0, 1.0, 1.0], [4.0, 3.0, 2.0], [1.0, 3.0, 1.9]],
        }
    }

    fn name(self) -> &'static str {
        match self {
            Room::Tutorial => "6x10x3",
            Room::Validation => "5x4x3",
        }
    }
}

/// One cell of M8's matrix: `room` with every surface of absorption `alpha` and a Lambert law
/// with scattering 1, air absorption off, SPPS in `method` with `particles`, `duration` s in steps
/// of `dt`, `trans_epsilon` `eps`, seeded `seed`; octave bands 125 Hz to 4 kHz.
#[allow(clippy::too_many_arguments)]
fn m8_cell(
    room: Room,
    alpha: f64,
    method: ComputationMethod,
    particles: u32,
    duration: f64,
    dt: f64,
    eps: f64,
    seed: u32,
) -> Project {
    let mut p = tutorial_octaves(seed);
    let [lx, ly, lz] = room.size();
    p.geometry.vertices = p
        .geometry
        .vertices
        .iter()
        .map(|v| {
            let [x, y, z] = v.to_array();
            Vec3::new(x / 6.0 * lx, y / 10.0 * ly, z / 3.0 * lz)
        })
        .collect();
    let group = GroupId::from_u128(0x0c0b_e000_0000_4000_8000_0000_0008_0001);
    let mat = MaterialId::from_u128(0x0c0b_e000_0000_4000_8000_0000_0008_0002);
    p.geometry.faces = p
        .geometry
        .faces
        .iter()
        .map(|f| Face {
            vertices: f.vertices,
            group,
        })
        .collect();
    let n = p.bands.len();
    let mut m = p.materials[0].clone();
    m.id = mat;
    m.name = format!("alpha {alpha}");
    m.absorption = vec![schema::F64::new(alpha); n];
    m.scattering = vec![schema::F64::new(1.0); n];
    m.reflection_law = ReflectionLaw::Lambert.into();
    m.transmission_loss_db = None;
    m.solver_id = None;
    p.materials = vec![m];
    p.surface_groups = vec![schema::SurfaceGroup {
        id: group,
        name: "Surfaces".into(),
        material: mat,
    }];
    let s = room.source();
    p.sources[0].position = Vec3::new(s[0], s[1], s[2]);
    with_receivers(&mut p, &room.receivers());
    let spps = &mut p.solvers.spps;
    spps.method = method;
    spps.particles_per_source = particles;
    spps.duration_s = schema::F64::new(duration);
    spps.time_step_s = schema::F64::new(dt);
    spps.extinction_exponent = schema::F64::new(eps);
    spps.air_absorption = false;
    p.solvers.tcr.air_absorption = false;
    p
}

/// A cell of the table: which of T30, EDT, C80 and D50 every seed lets through, and T30's spread
/// over the seeds.
fn summarise(label: &str, done: &[Done]) {
    let reports: Vec<&Value> = done.iter().map(|d| &d.report).collect();
    let secs: Vec<f64> = done.iter().map(|d| d.secs).collect();
    let (mut t30_cells, mut t30_all, mut worst_spread) = (0, 0, 0.0f64);
    let mut through = [0usize; 4];
    let mut refused: std::collections::BTreeMap<String, usize> = Default::default();
    let mut spreads = Vec::new();
    let first = reports[0]["spps"]["point_receivers"].as_array().unwrap();
    for (ri, r) in first.iter().enumerate() {
        for bi in 0..r["bands"].as_array().unwrap().len() {
            t30_cells += 1;
            let cell = |rep: &Value, q: &str| {
                rep["spps"]["point_receivers"][ri]["bands"][bi]["parameters"][q].clone()
            };
            for (k, q) in ["t30_s", "edt_s", "c80_db", "d50"].iter().enumerate() {
                let vals: Vec<Value> = reports.iter().map(|rep| cell(rep, q)).collect();
                if vals.iter().all(|v| v["value"].is_f64()) {
                    through[k] += 1;
                } else {
                    for v in &vals {
                        if !v["value"].is_f64() {
                            *refused.entry(format!("{q} {}", why(v))).or_default() += 1;
                        }
                    }
                }
            }
            let t30: Vec<f64> = reports
                .iter()
                .filter_map(|rep| cell(rep, "t30_s")["value"].as_f64())
                .collect();
            if t30.len() == reports.len() {
                t30_all += 1;
                let (m, _) = mean_sd(&t30);
                let s = (t30.iter().copied().fold(f64::NEG_INFINITY, f64::max)
                    - t30.iter().copied().fold(f64::INFINITY, f64::min))
                    / m;
                worst_spread = worst_spread.max(s);
                spreads.push(s);
            }
        }
    }
    spreads.sort_by(f64::total_cmp);
    let median = spreads.get(spreads.len() / 2).copied().unwrap_or(f64::NAN);
    println!(
        "CELL {label}: receiver-bands {t30_cells}; through in every seed: T30 {}, EDT {}, C80 {}, \
         D50 {}; T30 spread (max-min)/mean over {} seeds where all give it ({t30_all}): median \
         {:.2} %, worst {:.2} %; wall {:.0}-{:.0} s per run; refused {refused:?}",
        through[0],
        through[1],
        through[2],
        through[3],
        reports.len(),
        100.0 * median,
        100.0 * worst_spread,
        secs.iter().copied().fold(f64::INFINITY, f64::min),
        secs.iter().copied().fold(0.0, f64::max),
    );
    // T30 from each series alone, refused or not: its spread over the seeds per receiver-band and
    // for the mean over the cell's receiver-bands; the estimated standard deviation; and the mean
    // against Eyring with SPPS's own constant.
    let (mut plain_spreads, mut cell_means, mut sds) = (Vec::new(), Vec::new(), Vec::new());
    let mut all_t30 = Vec::new();
    let per_seed: Vec<Vec<Option<f64>>> = reports
        .iter()
        .map(|rep| {
            let s = &rep["spps"];
            let dt = s["time_step_s"].as_f64().unwrap();
            let half = s["receiver_crossing_s"].as_f64().unwrap() / 2.0;
            s["point_receivers"]
                .as_array()
                .unwrap()
                .iter()
                .flat_map(|r| {
                    let t_a = r["arrival_s"].as_f64().unwrap();
                    r["bands"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|b| {
                            let v: Vec<f64> = b["energy_pa2"]
                                .as_array()
                                .unwrap()
                                .iter()
                                .map(|v| v.as_f64().unwrap())
                                .collect();
                            t30_plain(dt, v, Arrival::spread(t_a, half))
                        })
                        .collect::<Vec<_>>()
                })
                .collect()
        })
        .collect();
    for c in 0..t30_cells {
        let v: Vec<f64> = per_seed.iter().filter_map(|s| s[c]).collect();
        if v.len() == per_seed.len() && v.len() > 1 {
            let (m, _) = mean_sd(&v);
            plain_spreads.push(
                (v.iter().copied().fold(f64::NEG_INFINITY, f64::max)
                    - v.iter().copied().fold(f64::INFINITY, f64::min))
                    / m,
            );
        }
        all_t30.extend(v);
    }
    for s in &per_seed {
        let v: Vec<f64> = s.iter().flatten().copied().collect();
        if !v.is_empty() {
            cell_means.push(v.iter().sum::<f64>() / v.len() as f64);
        }
    }
    for rep in &reports {
        for r in rep["spps"]["point_receivers"].as_array().unwrap() {
            for b in r["bands"].as_array().unwrap() {
                let p = &b["parameters"]["t30_s"];
                let rel = match (p["value"].as_f64(), p["mc_sd"].as_f64()) {
                    (Some(v), Some(sd)) => Some(sd / v),
                    _ => {
                        let w = &p["not_evaluable"]["error"]["why"];
                        (w["why"] == "monte_carlo_noise")
                            .then(|| w["sd"].as_f64().zip(w["value"].as_f64()))
                            .flatten()
                            .map(|(sd, v)| sd / v)
                    }
                };
                if let Some(x) = rel {
                    sds.push(x);
                }
            }
        }
    }
    plain_spreads.sort_by(f64::total_cmp);
    sds.sort_by(f64::total_cmp);
    let q = |v: &[f64], p: f64| {
        v.get(((v.len().max(1) - 1) as f64 * p) as usize)
            .copied()
            .unwrap_or(f64::NAN)
    };
    let cell_spread = if cell_means.len() > 1 {
        let (m, _) = mean_sd(&cell_means);
        (cell_means.iter().copied().fold(f64::NEG_INFINITY, f64::max)
            - cell_means.iter().copied().fold(f64::INFINITY, f64::min))
            / m
    } else {
        f64::NAN
    };
    let mean_t30 = all_t30.iter().sum::<f64>() / all_t30.len().max(1) as f64;
    let eyring = eyring_of(label);
    println!(
        "  from the series alone: T30 in {} of {} receiver-band-seeds; spread over the seeds per \
         receiver-band median {:.2} %, worst {:.2} %; of the cell's mean T30 {:.2} %; estimated sd \
         median {:.2} %, largest {:.2} %; mean T30 {mean_t30:.4} s against Eyring (K = \
         24 ln10/343.2) {eyring:.4} s: {:+.2} %",
        all_t30.len(),
        t30_cells * reports.len(),
        100.0 * q(&plain_spreads, 0.5),
        100.0 * q(&plain_spreads, 1.0),
        100.0 * cell_spread,
        100.0 * q(&sds, 0.5),
        100.0 * q(&sds, 1.0),
        100.0 * (mean_t30 / eyring - 1.0),
    );
}

/// T_Eyring of the cell named in `label` (`<room> alpha <a> ...`), air absorption off, with
/// SPPS's own constant `24·ln(10)/343.2`.
fn eyring_of(label: &str) -> f64 {
    let f: Vec<&str> = label.split_whitespace().collect();
    let (v, s) = match f[0] {
        "6x10x3" => (180.0, 216.0),
        _ => (60.0, 94.0),
    };
    let alpha: f64 = f[2].parse().unwrap();
    24.0 * std::f64::consts::LN_10 / 343.2 * v / (-s * (1.0 - alpha).ln())
}

/// The cells to run: `$SIMPA_M8_CELLS`, `;`-separated, each
/// `room,alpha,method,particles,duration,dt,eps` with room `6x10x3` or `5x4x3` and method
/// `random` or `energetic`.
fn cells_from_env() -> Vec<(Room, f64, ComputationMethod, u32, f64, f64, f64)> {
    let spec = std::env::var("SIMPA_M8_CELLS").unwrap_or_else(|_| {
        "6x10x3,0.2,random,150000,2,0.01,5;6x10x3,0.2,energetic,150000,2,0.01,5".into()
    });
    spec.split(';')
        .filter(|s| !s.trim().is_empty())
        .map(|c| {
            let f: Vec<&str> = c.split(',').map(str::trim).collect();
            assert_eq!(f.len(), 7, "cell {c:?}");
            let room = match f[0] {
                "6x10x3" => Room::Tutorial,
                "5x4x3" => Room::Validation,
                other => panic!("room {other:?}"),
            };
            let method = match f[2] {
                "random" => ComputationMethod::Random,
                "energetic" => ComputationMethod::Energetic,
                other => panic!("method {other:?}"),
            };
            (
                room,
                f[1].parse().unwrap(),
                method,
                f[3].parse().unwrap(),
                f[4].parse().unwrap(),
                f[5].parse().unwrap(),
                f[6].parse().unwrap(),
            )
        })
        .collect()
}

#[test]
#[ignore = "evidence for M8, not a gate: runs M8's cells over three SPPS seeds; run on purpose \
            with SIMPA_M8_CELLS"]
fn m8_cells() {
    let cells = cells_from_env();
    let root = evidence_root("m8-cells");
    let mut projects = Vec::new();
    for (ci, &(room, alpha, method, particles, duration, dt, eps)) in cells.iter().enumerate() {
        for seed in 1..=3u32 {
            projects.push((
                format!(
                    "{ci} {} alpha {alpha} {method:?} {particles} {duration} s dt {dt} eps {eps} seed {seed}",
                    room.name()
                ),
                m8_cell(room, alpha, method, particles, duration, dt, eps, seed),
            ));
        }
    }
    // `$SIMPA_M8_FROM`: the folder an earlier run of this test wrote with the same cells, read
    // again (with this build's `simpa results`) instead of running the solver.
    let done = match std::env::var_os("SIMPA_M8_FROM") {
        Some(from) => projects
            .iter()
            .enumerate()
            .map(|(i, (name, _))| {
                let runs = Path::new(&from).join(format!("{i:03}")).join("runs");
                let run = std::fs::read_dir(&runs)
                    .unwrap_or_else(|e| panic!("{}: {e}", runs.display()))
                    .next()
                    .unwrap()
                    .unwrap()
                    .path();
                let r = simpa_run(&[
                    "results".to_string(),
                    run.display().to_string(),
                    "--json".into(),
                ]);
                assert!(r.code == 0 || r.code == 6, "{r:#?}");
                Done {
                    name: name.clone(),
                    secs: f64::NAN,
                    report: json(&r),
                }
            })
            .collect(),
        None => run_all(&root, projects),
    };
    for (ci, &(room, alpha, method, particles, duration, dt, eps)) in cells.iter().enumerate() {
        let mine: Vec<Done> = done
            .iter()
            .filter(|d| d.name.starts_with(&format!("{ci} ")))
            .map(|d| Done {
                name: d.name.clone(),
                secs: d.secs,
                report: d.report.clone(),
            })
            .collect();
        let mine = accepted(mine);
        if mine.is_empty() {
            println!("CELL {ci}: every run refused");
            continue;
        }
        summarise(
            &format!(
                "{} alpha {alpha} {method:?} N {particles} {duration} s dt {dt} eps {eps}",
                room.name()
            ),
            &mine,
        );
    }
}
