//! Evidence for M8's design, gathered by the M7 follow-ups (`docs/results.md`, "What M8 needs").
//! Each test runs SPPS many times and prints what it measured; none is a gate, so each is
//! ignored and run on purpose:
//!
//! `cargo test --release -p simpa --test m8_evidence -- --ignored --nocapture <name>`
//!
//! The run folders go under `$SIMPA_EVIDENCE_ROOT` when it is set, and stay there; else under
//! cargo's test scratch space, removed when the test passes (`support::scratch`, as every test
//! folder is since the 2026-09-25 disk emergency). `$SIMPA_EVIDENCE_JOBS` (default 8) is how many solver runs go at once: a seeded SPPS run
//! is single-threaded.
//!
//! - `arrival_outside_the_onset_bin_over_receiver_positions`: tutorial 1 at upstream's defaults,
//!   200 receivers at random positions: how often `r/c` falls outside the onset bin, and what the
//!   decay times do there.
//! - `energetic_noise_against_ten_seeds`: tutorial 1 in energetic mode, ten seeds at 150,000 and
//!   1,500,000 particles: the Monte-Carlo estimate against the spread over the seeds, and which of
//!   the floor and the lost particles refuses T20 and T30. `$SIMPA_ENERGETIC_FROM_<count>` reads an
//!   earlier run's folder again.
//! - `energetic_lost_particles_from_saved_trajectories`: tutorial 1, or one of M8's cells
//!   (`$SIMPA_LOST_CELL`), in energetic mode with every trajectory saved: the energy lost particles
//!   carried, and what they would have moved T30 by. `$SIMPA_LOST_FROM` reads an earlier run's
//!   folder again; `$SIMPA_LOST_DROPPED_FACTOR` sets how far above the floor a particle must end to
//!   count as lost.
//! - `energetic_floor_against_a_lower_floor`: T30 at `trans_epsilon` 5 against 9, ten seeds.
//!   `$SIMPA_FLOOR_FROM` reads an earlier run's folder again.
//! - `m8_cells`: M8's rooms and computation methods at several particle counts, durations and time
//!   steps, air off or on (`$SIMPA_M8_CELLS`), over seeds `$SIMPA_M8_SEEDS` (1 to 3 by default):
//!   which quantities the refusal limits let through, the seed statistics of T30 and EDT, T30
//!   against Eyring, EDT and T30 against the finest step, and the wall time. `$SIMPA_M8_FROM`
//!   reads an earlier run's folder again.
//! - `m8_tcr_cells`: TCR on the same cells, its Eyring time against the analytic value.
//!
//! The independent transport M8's reference is checked with is `simpa-core`'s
//! `tests/lambert_box.rs`.

mod support;

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use serde_json::Value;
use simpa_core::params::EnergySeries;
use simpa_core::params::air;
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

/// Runs every project through `solver` (`spps` or `tcr`), `jobs()` at a time, each in its own runs
/// folder, and reads its results. A run that is not OK fails the test; results that `simpa
/// results` refuses are kept as the refusal (exit 6), so that one refused run is counted rather
/// than ending the evidence.
fn run_all(root: &Path, projects: Vec<(String, Project)>, solver: &str) -> Vec<Done> {
    solver_exe(match solver {
        "spps" => "spps.exe",
        _ => "classicalTheory.exe",
    });
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
                        solver.into(),
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

/// The projects' runs read again from `from`, the folder an earlier call of [`run_all`] with the
/// same projects wrote (`<from>/<index>/runs/<run>`), with this build's `simpa results`: no solver
/// runs. Wall times are NaN.
fn reread(from: &Path, projects: &[(String, Project)]) -> Vec<Done> {
    projects
        .iter()
        .enumerate()
        .map(|(i, (name, _))| {
            let runs = from.join(format!("{i:03}")).join("runs");
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
        .collect()
}

/// [`run_all`], or [`reread`] from the folder `$<var>` names when it is set.
fn run_or_reread(var: &str, label: &str, projects: Vec<(String, Project)>) -> Vec<Done> {
    match std::env::var_os(var) {
        Some(from) if !from.is_empty() => reread(Path::new(&from), &projects),
        _ => run_all(&evidence_root(label), projects, "spps"),
    }
}

/// Tutorial 1's box (`rooms/tutorial1_box.simpa`, upstream's defaults) on the octave bands 125 Hz
/// to 4 kHz, without its surface receiver and the intersection logs, seeded `seed`.
fn tutorial_octaves(seed: u32) -> Project {
    tutorial_octaves_to(seed, 4000)
}

/// [`tutorial_octaves`] with the octave bands from 125 Hz up to `top_hz`.
fn tutorial_octaves_to(seed: u32, top_hz: u32) -> Project {
    let mut p = schema::load(&fixture("rooms/tutorial1_box.simpa")).unwrap();
    let all = p.bands.frequencies_hz.clone();
    let keep: Vec<u32> = [125, 250, 500, 1000, 2000, 4000, 8000]
        .into_iter()
        .filter(|f| *f <= top_hz)
        .collect();
    let pick = |v: &[schema::F64]| -> Vec<schema::F64> {
        keep.iter()
            .map(|f| v[all.iter().position(|a| a == f).unwrap()])
            .collect()
    };
    for m in &mut p.materials {
        m.absorption = pick(&m.absorption);
        m.scattering = pick(&m.scattering);
    }
    p.bands = BandSet::range(BandKind::Octave, 125, top_hz).unwrap();
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
            // Tutorial 1's receivers pin upstream's solver ids since the M7 follow-ups; copies of
            // one would share its pin, which the run refuses (`solver_id_mapping_invalid`).
            r.solver_id = None;
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
    for done in accepted(run_all(&root, projects, "spps")) {
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
        &NoiseModel::crossings(d, noise::Method::Random, None).unwrap(),
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
    // `$SIMPA_ENERGETIC_FROM_<count>`: an earlier run's folder for that count, read again.
    for particles in counts {
        let projects = (1..=10u32)
            .map(|seed| {
                let mut p = tutorial_octaves(seed);
                with_receivers(&mut p, &SIX);
                p.solvers.spps.method = ComputationMethod::Energetic;
                p.solvers.spps.particles_per_source = particles;
                (format!("energetic {particles} seed {seed}"), p)
            })
            .collect();
        let done = accepted(run_or_reread(
            &format!("SIMPA_ENERGETIC_FROM_{particles}"),
            &format!("energetic-seeds-{particles}"),
            projects,
        ));
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
    // `$SIMPA_LOST_CELL`: one of M8's cells (`m8_cells`' syntax) in place of tutorial 1, its
    // particle count the particles run and saved.
    let cell = std::env::var("SIMPA_LOST_CELL")
        .ok()
        .map(|c| parse_cells(&c).remove(0));
    let particles = cell.map_or(150_000u32, |c| c.particles);
    // `$SIMPA_LOST_FROM`: the folder an earlier run of this test wrote, read again instead of
    // running the solver (each run leaves 0.7 GB of trajectories on tutorial 1).
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
                    let mut p = match &cell {
                        Some(c) => c.project(seed),
                        None => {
                            let mut p = tutorial_octaves(seed);
                            with_receivers(&mut p, &SIX);
                            p.solvers.spps.method = ComputationMethod::Energetic;
                            p.solvers.spps.particles_per_source = particles;
                            p
                        }
                    };
                    p.solvers.spps.particles_saved = particles;
                    (format!("energetic {particles} saved, seed {seed}"), p)
                })
                .collect();
            accepted(run_all(&root, projects, "spps"))
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
            // `$SIMPA_LOST_DROPPED_FACTOR` (default 10) over the floor: at α 0.4 a step can hold
            // five reflections, and a particle the floor drops can end 13 times above it.
            let factor: f64 = std::env::var("SIMPA_LOST_DROPPED_FACTOR")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(10.0);
            let dropped_at_most = factor * 10f64.powf(-eps);
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
            // measured energy and loss time, the future modelled as following the decay (the
            // trajectories stop where the particle was lost, so what it would have brought is not
            // measured); against random mode's lump bound.
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
        "T30 moved, worst receiver-band: by the lost particles with their measured energies and \
         loss times, what each would still have brought modelled as following the decay (not \
         measured) {:.2e}; by random mode's lump bound {:.2e} (limit 5e-3)",
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
    // `$SIMPA_FLOOR_FROM`: an earlier run's folder, read again.
    let done = run_or_reread("SIMPA_FLOOR_FROM", "energetic-floor", projects);
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
    // The standard error of the mean difference, from the receiver-bands' own.
    let mean_se = (diffs.iter().map(|d| d.1 * d.1).sum::<f64>()).sqrt() / diffs.len() as f64;
    // The largest difference and its own standard errors (one receiver-band), and apart the
    // largest in standard errors.
    let worst = diffs
        .iter()
        .copied()
        .max_by(|a, b| a.0.abs().total_cmp(&b.0.abs()))
        .unwrap();
    let worst_z = diffs.iter().map(|d| (d.0 / d.1).abs()).fold(0.0, f64::max);
    let beyond = diffs.iter().filter(|d| d.0.abs() > 0.005).count();
    println!(
        "{particles} particles: T30 at trans_epsilon 5 over T30 at 9, {} receiver-bands: mean \
         {:+.3} % ± {:.3} %; largest {:+.3} % in one receiver-band, {:.1} of its standard errors; \
         the largest in standard errors, in any receiver-band, {worst_z:.1}; beyond 0.5 % in \
         {beyond}",
        diffs.len(),
        100.0 * mean_diff,
        100.0 * mean_se,
        100.0 * worst.0,
        (worst.0 / worst.1).abs()
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

    fn parse(s: &str) -> Room {
        match s {
            "6x10x3" => Room::Tutorial,
            "5x4x3" => Room::Validation,
            other => panic!("room {other:?}"),
        }
    }

    /// `(V, S)`: the volume, m³, and the surface, m².
    fn volume_area(self) -> (f64, f64) {
        let [a, b, c] = self.size();
        (a * b * c, 2.0 * (a * b + b * c + a * c))
    }
}

/// SPPS's speed of sound at the fixture's 20 °C, `343.2·√(293.15/293.15)`
/// (`Celerite_du_son.cpp:46`).
const C_20C: f64 = 343.2;

/// One cell of M8's matrix: `room` with every surface of absorption `alpha` and a Lambert law
/// with scattering 1, SPPS in `method` with `particles`, `duration` s in steps of `dt`,
/// `trans_epsilon` `eps`; octave bands 125 Hz to 4 kHz with air absorption off, or with `air` on
/// (20 °C, 50 %, 101.325 kPa, the fixture's) and the octave bands up to 8 kHz, M8's second table.
#[derive(Clone, Copy, Debug)]
struct Cell {
    room: Room,
    alpha: f64,
    method: ComputationMethod,
    particles: u32,
    duration: f64,
    dt: f64,
    eps: f64,
    air: bool,
}

impl Cell {
    fn label(&self) -> String {
        format!(
            "{} alpha {} {:?} N {} {} s dt {} eps {}{}",
            self.room.name(),
            self.alpha,
            self.method,
            self.particles,
            self.duration,
            self.dt,
            self.eps,
            if self.air { " air" } else { "" }
        )
    }

    /// The same cell but for its time step.
    fn same_but_dt(&self, o: &Cell) -> bool {
        self.room.name() == o.room.name()
            && self.alpha == o.alpha
            && self.method as u8 == o.method as u8
            && self.particles == o.particles
            && self.duration == o.duration
            && self.eps == o.eps
            && self.air == o.air
            && self.dt != o.dt
    }

    /// The air term SPPS and TCR apply at `freq_hz` (`air::solver_air_absorption_per_m`), 1/m;
    /// 0 with air absorption off.
    fn air_m(&self, freq_hz: f64) -> f64 {
        if self.air {
            air::solver_air_absorption_per_m(
                freq_hz,
                &air::Atmosphere::at_reference_pressure(20.0, 50.0),
            )
            .unwrap()
        } else {
            0.0
        }
    }

    /// T_Eyring at `freq_hz` with SPPS's own constant `24·ln(10)/c`, and `4mV` when air is on.
    fn eyring(&self, freq_hz: f64) -> f64 {
        let (v, s) = self.room.volume_area();
        24.0 * std::f64::consts::LN_10 / C_20C * v
            / (-s * (1.0 - self.alpha).ln() + 4.0 * self.air_m(freq_hz) * v)
    }

    fn project(&self, seed: u32) -> Project {
        m8_cell(self, seed)
    }
}

/// [`Cell`]'s project, seeded `seed`.
fn m8_cell(cell: &Cell, seed: u32) -> Project {
    let Cell {
        room,
        alpha,
        method,
        particles,
        duration,
        dt,
        eps,
        air,
    } = *cell;
    let mut p = tutorial_octaves_to(seed, if air { 8000 } else { 4000 });
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
    spps.air_absorption = air;
    p.solvers.tcr.air_absorption = air;
    p
}

/// The range of three draws of a normal, in its standard deviations: the sorted ranges of 400,000
/// triples, for `P(range ≤ w)` and its quantiles (mean 1.693, 95 % quantile 3.31).
struct Ranges(Vec<f64>);

impl Ranges {
    fn new() -> Ranges {
        let mut rng = Rng(0x4d38_0000_0000_0003);
        let mut normal = || {
            let (u, v) = (rng.uniform().max(1e-300), rng.uniform());
            (-2.0 * u.ln()).sqrt() * (2.0 * std::f64::consts::PI * v).cos()
        };
        let mut r: Vec<f64> = (0..400_000)
            .map(|_| {
                let (a, b, c) = (normal(), normal(), normal());
                a.max(b).max(c) - a.min(b).min(c)
            })
            .collect();
        r.sort_by(f64::total_cmp);
        Ranges(r)
    }

    /// `P(range ≤ w·σ)`.
    fn below(&self, w: f64) -> f64 {
        self.0.partition_point(|x| *x <= w) as f64 / self.0.len() as f64
    }

    /// The `p`-quantile, in σ.
    fn quantile(&self, p: f64) -> f64 {
        self.0[((self.0.len() - 1) as f64 * p) as usize]
    }
}

/// `(max − min)/mean`.
fn range_of(v: &[f64]) -> f64 {
    let (m, _) = mean_sd(v);
    (v.iter().copied().fold(f64::NEG_INFINITY, f64::max)
        - v.iter().copied().fold(f64::INFINITY, f64::min))
        / m
}

/// One receiver-band of one run: its band, and T30 and EDT from the series alone, taken complete
/// and measured from the arrival and its spread; `None` where refused.
#[derive(Clone, Copy, Debug)]
struct Plain {
    receiver: usize,
    freq_hz: f64,
    t30: Option<f64>,
    edt: Option<f64>,
}

/// Every receiver-band of a report, receivers then bands.
fn plain_of(rep: &Value) -> Vec<Plain> {
    let s = &rep["spps"];
    let dt = s["time_step_s"].as_f64().unwrap();
    let half = s["receiver_crossing_s"].as_f64().unwrap() / 2.0;
    let mut out = Vec::new();
    for (ri, r) in s["point_receivers"].as_array().unwrap().iter().enumerate() {
        let t_a = r["arrival_s"].as_f64().unwrap();
        for b in r["bands"].as_array().unwrap() {
            let v: Vec<f64> = b["energy_pa2"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_f64().unwrap())
                .collect();
            let p = EnergySeries::complete(dt, v)
                .ok()
                .map(|s| simpa_core::params::decay::evaluate(&s, Arrival::spread(t_a, half)));
            out.push(Plain {
                receiver: ri,
                freq_hz: b["freq_hz"].as_f64().unwrap(),
                t30: p.as_ref().and_then(|p| p.t30.as_ref().ok()).map(|f| f.t_s),
                edt: p.as_ref().and_then(|p| p.edt.as_ref().ok()).map(|f| f.t_s),
            });
        }
    }
    out
}

/// What [`summarise`] hands the comparisons across cells: per receiver, the mean EDT and T30 from
/// the series alone over the bands and seeds, each with its standard error.
struct CellSummary {
    cell: Cell,
    per_receiver: Vec<[(f64, f64); 2]>,
}

/// The seed statistics of one quantity, `v[seed][receiver-band]` from the series alone, at
/// `particles` per band: the range of the first three seeds, per receiver-band and of the cell's
/// mean; the standard deviation pooled over the receiver-bands, with its own uncertainty; the
/// probability of a 3-seed range within 2 % it implies; the particle counts at which that
/// probability is reached; and, with more than three seeds, the same read off every triple.
fn seed_statistics(name: &str, v: &[Vec<Option<f64>>], particles: u32, ranges: &Ranges) {
    let seeds = v.len();
    let n = v[0].len();
    // The receiver-bands where every seed gives a value: the same subset in every seed.
    let full: Vec<usize> = (0..n)
        .filter(|&c| v.iter().all(|s| s[c].is_some()))
        .collect();
    if full.is_empty() || seeds < 2 {
        println!("  {name}: no receiver-band with a value in every seed");
        return;
    }
    let at = |s: usize, c: usize| v[s][c].unwrap();
    let three = seeds.min(3);
    let mut r3: Vec<f64> = full
        .iter()
        .map(|&c| range_of(&(0..three).map(|s| at(s, c)).collect::<Vec<_>>()))
        .collect();
    r3.sort_by(f64::total_cmp);
    let cell_mean = |s: usize| full.iter().map(|&c| at(s, c)).sum::<f64>() / full.len() as f64;
    let means: Vec<f64> = (0..seeds).map(cell_mean).collect();
    // Relative standard deviation per receiver-band over every seed, pooled.
    let rel_sd: Vec<f64> = full
        .iter()
        .map(|&c| {
            let (m, sd) = mean_sd(&(0..seeds).map(|s| at(s, c)).collect::<Vec<_>>());
            sd / m
        })
        .collect();
    let sigma = (rel_sd.iter().map(|x| x * x).sum::<f64>() / rel_sd.len() as f64).sqrt();
    let dof = (full.len() * (seeds - 1)) as f64;
    let sigma_se = sigma / (2.0 * dof).sqrt();
    let p_one = ranges.below(0.02 / sigma);
    let all_p = p_one.powi(full.len() as i32);
    // σ targets: a mean 3-seed range of 2 %; 95 % of receiver-bands within 2 %; every one of the
    // cell's receiver-bands within 2 % with probability 95 %.
    let targets = [
        0.02 / 1.6926,
        0.02 / ranges.quantile(0.95),
        0.02 / ranges.quantile(0.95f64.powf(1.0 / full.len() as f64)),
    ];
    let counts: Vec<String> = targets
        .iter()
        .map(|t| format!("{:.1} M", f64::from(particles) * (sigma / t).powi(2) / 1e6))
        .collect();
    println!(
        "  {name}: {} of {n} receiver-bands in all {seeds} seeds; range of seeds 1-{three} per \
         receiver-band median {:.2} %, worst {:.2} %; of the cell's mean {:.2} %; relative sd per \
         receiver-band pooled {:.2} % ± {:.2} % ({dof} dof); from it P(3-seed range <= 2 %) {:.3} \
         per receiver-band, {:.3} for all {}; particles for a mean range of 2 % / 95 % per \
         receiver-band / 95 % for all: {}",
        full.len(),
        100.0 * r3[r3.len() / 2],
        100.0 * r3[r3.len() - 1],
        100.0 * range_of(&means[..three]),
        100.0 * sigma,
        100.0 * sigma_se,
        p_one,
        all_p,
        full.len(),
        counts.join(" / ")
    );
    if seeds > 3 {
        // Every triple of seeds, as M8 would draw three.
        let (mut within, mut cells, mut all_within, mut mean_within, mut triples) =
            (0usize, 0usize, 0usize, 0usize, 0usize);
        let mut worst = Vec::new();
        for a in 0..seeds {
            for b in a + 1..seeds {
                for c in b + 1..seeds {
                    triples += 1;
                    let mut w = 0.0f64;
                    let mut all = true;
                    for &k in &full {
                        let r = range_of(&[at(a, k), at(b, k), at(c, k)]);
                        cells += 1;
                        if r <= 0.02 {
                            within += 1;
                        } else {
                            all = false;
                        }
                        w = w.max(r);
                    }
                    worst.push(w);
                    all_within += usize::from(all);
                    mean_within += usize::from(range_of(&[means[a], means[b], means[c]]) <= 0.02);
                }
            }
        }
        worst.sort_by(f64::total_cmp);
        let (m, sd) = mean_sd(&means);
        println!(
            "  {name}, over all {triples} triples of the {seeds} seeds: 3-seed range <= 2 % in {:.3} \
             of receiver-bands, in every receiver-band of the cell in {:.3} of triples; worst \
             receiver-band's range median {:.2} %, 5-95 % {:.2}-{:.2} %; the cell's mean within 2 % \
             in {:.3}; relative sd of the cell's mean {:.3} %",
            within as f64 / cells as f64,
            all_within as f64 / triples as f64,
            100.0 * worst[worst.len() / 2],
            100.0 * worst[worst.len() / 20],
            100.0 * worst[worst.len() * 19 / 20],
            mean_within as f64 / triples as f64,
            100.0 * sd / m
        );
    }
}

/// A cell of the table: which of T30, EDT, C80 and D50 every seed lets through, why the others are
/// refused, the seed statistics of T30, the estimated noise, T30 against Eyring and the wall time.
fn summarise(cell: &Cell, done: &[Done], ranges: &Ranges) -> CellSummary {
    let label = cell.label();
    let reports: Vec<&Value> = done.iter().map(|d| &d.report).collect();
    let secs: Vec<f64> = done
        .iter()
        .map(|d| d.secs)
        .filter(|s| s.is_finite())
        .collect();
    let mut through = [0usize; 4];
    let mut refused: std::collections::BTreeMap<String, usize> = Default::default();
    let first = reports[0]["spps"]["point_receivers"].as_array().unwrap();
    let mut rbs = 0;
    for (ri, r) in first.iter().enumerate() {
        for bi in 0..r["bands"].as_array().unwrap().len() {
            rbs += 1;
            let at = |rep: &Value, q: &str| {
                rep["spps"]["point_receivers"][ri]["bands"][bi]["parameters"][q].clone()
            };
            for (k, q) in ["t30_s", "edt_s", "c80_db", "d50"].iter().enumerate() {
                let vals: Vec<Value> = reports.iter().map(|rep| at(rep, q)).collect();
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
        }
    }
    let wall = if secs.is_empty() {
        "re-read".to_string()
    } else {
        format!(
            "{:.0}-{:.0} s per run",
            secs.iter().copied().fold(f64::INFINITY, f64::min),
            secs.iter().copied().fold(0.0, f64::max)
        )
    };
    println!(
        "CELL {label}: {} seeds, {rbs} receiver-bands; through in every seed: T30 {}, EDT {}, C80 \
         {}, D50 {}; wall {wall}; refused {refused:?}",
        reports.len(),
        through[0],
        through[1],
        through[2],
        through[3],
    );
    // T30 and EDT from each series alone, refused or not.
    let plain: Vec<Vec<Plain>> = reports.iter().map(|r| plain_of(r)).collect();
    let t30: Vec<Vec<Option<f64>>> = plain
        .iter()
        .map(|s| s.iter().map(|p| p.t30).collect())
        .collect();
    let edt: Vec<Vec<Option<f64>>> = plain
        .iter()
        .map(|s| s.iter().map(|p| p.edt).collect())
        .collect();
    seed_statistics("T30", &t30, cell.particles, ranges);
    seed_statistics("EDT", &edt, cell.particles, ranges);
    // The estimated standard deviation of T30 in the reports.
    let mut sds = Vec::new();
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
    sds.sort_by(f64::total_cmp);
    let q = |v: &[f64], p: f64| {
        v.get(((v.len().max(1) - 1) as f64 * p) as usize)
            .copied()
            .unwrap_or(f64::NAN)
    };
    // Mean T30 against Eyring per band (one band suffices without air: every band is the same
    // room), over the receivers and seeds.
    let bands: Vec<f64> = {
        let mut b: Vec<f64> = plain[0].iter().map(|p| p.freq_hz).collect();
        b.sort_by(f64::total_cmp);
        b.dedup();
        b
    };
    let mut against = Vec::new();
    let mut all = Vec::new();
    for f in &bands {
        let v: Vec<f64> = plain
            .iter()
            .flatten()
            .filter(|p| p.freq_hz == *f)
            .filter_map(|p| p.t30)
            .collect();
        if v.is_empty() {
            continue;
        }
        let (m, sd) = mean_sd(&v);
        against.push(format!(
            "{f} Hz {m:.4} s ({:+.2} % ± {:.2} %)",
            100.0 * (m / cell.eyring(*f) - 1.0),
            100.0 * sd / (v.len() as f64).sqrt() / m
        ));
        all.push(m / cell.eyring(*f) - 1.0);
    }
    println!(
        "  T30 estimated sd median {:.2} %, largest {:.2} %; mean T30 against Eyring (K = 24 \
         ln10/343.2{}) over the bands {:+.2} %; per band: {}",
        100.0 * q(&sds, 0.5),
        100.0 * q(&sds, 1.0),
        if cell.air { ", 4mV the solver's" } else { "" },
        100.0 * all.iter().sum::<f64>() / all.len().max(1) as f64,
        against.join("; ")
    );
    // Per receiver, over the bands and seeds.
    let receivers = plain[0].iter().map(|p| p.receiver).max().unwrap() + 1;
    let per_receiver: Vec<[(f64, f64); 2]> = (0..receivers)
        .map(|r| {
            let of = |pick: fn(&Plain) -> Option<f64>| {
                let v: Vec<f64> = plain
                    .iter()
                    .flatten()
                    .filter(|p| p.receiver == r)
                    .filter_map(pick)
                    .collect();
                if v.len() < 2 {
                    return (f64::NAN, f64::NAN);
                }
                let (m, sd) = mean_sd(&v);
                (m, sd / (v.len() as f64).sqrt())
            };
            [of(|p| p.edt), of(|p| p.t30)]
        })
        .collect();
    println!(
        "  per receiver, mean over bands and seeds: {}",
        per_receiver
            .iter()
            .enumerate()
            .map(|(i, [e, t])| format!(
                "R{i:03} EDT {:.4} ± {:.4} s, T30 {:.4} ± {:.4} s",
                e.0, e.1, t.0, t.1
            ))
            .collect::<Vec<_>>()
            .join("; ")
    );
    CellSummary {
        cell: *cell,
        per_receiver,
    }
}

/// The cells to run: `$SIMPA_M8_CELLS`, `;`-separated, each
/// `room,alpha,method,particles,duration,dt,eps[,air]` with room `6x10x3` or `5x4x3`, method
/// `random` or `energetic`, and `air` for air absorption on and the octave bands to 8 kHz.
fn cells_from_env() -> Vec<Cell> {
    let spec = std::env::var("SIMPA_M8_CELLS").unwrap_or_else(|_| {
        "6x10x3,0.2,random,150000,2,0.01,5;6x10x3,0.2,energetic,150000,2,0.01,5".into()
    });
    parse_cells(&spec)
}

/// Cells in `$SIMPA_M8_CELLS`' syntax.
fn parse_cells(spec: &str) -> Vec<Cell> {
    spec.split(';')
        .filter(|s| !s.trim().is_empty())
        .map(|c| {
            let f: Vec<&str> = c.split(',').map(str::trim).collect();
            assert!(f.len() == 7 || f.len() == 8, "cell {c:?}");
            let method = match f[2] {
                "random" => ComputationMethod::Random,
                "energetic" => ComputationMethod::Energetic,
                other => panic!("method {other:?}"),
            };
            let air = match f.get(7) {
                None => false,
                Some(&"air") => true,
                Some(other) => panic!("{other:?}: `air` or nothing"),
            };
            Cell {
                room: Room::parse(f[0]),
                alpha: f[1].parse().unwrap(),
                method,
                particles: f[3].parse().unwrap(),
                duration: f[4].parse().unwrap(),
                dt: f[5].parse().unwrap(),
                eps: f[6].parse().unwrap(),
                air,
            }
        })
        .collect()
}

/// The seeds of every cell: `$SIMPA_M8_SEEDS`, `a-b` or a comma-separated list; 1 to 3 by default.
fn seeds_from_env() -> Vec<u32> {
    let s = std::env::var("SIMPA_M8_SEEDS").unwrap_or_else(|_| "1-3".into());
    match s.split_once('-') {
        Some((a, b)) => (a.trim().parse().unwrap()..=b.trim().parse().unwrap()).collect(),
        None => s.split(',').map(|x| x.trim().parse().unwrap()).collect(),
    }
}

#[test]
#[ignore = "evidence for M8, not a gate: runs M8's cells over three SPPS seeds; run on purpose \
            with SIMPA_M8_CELLS"]
fn m8_cells() {
    let cells = cells_from_env();
    let seeds = seeds_from_env();
    let mut projects = Vec::new();
    for (ci, cell) in cells.iter().enumerate() {
        for &seed in &seeds {
            projects.push((
                format!("{ci} {} seed {seed}", cell.label()),
                cell.project(seed),
            ));
        }
    }
    // `$SIMPA_M8_FROM`: the folder an earlier run of this test wrote with the same cells and
    // seeds, read again (with this build's `simpa results`) instead of running the solver.
    let done = run_or_reread("SIMPA_M8_FROM", "m8-cells", projects);
    let ranges = Ranges::new();
    let mut summaries = Vec::new();
    for (ci, cell) in cells.iter().enumerate() {
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
        summaries.push(summarise(cell, &mine, &ranges));
    }
    // Cells that differ only in their time step: EDT and T30 per receiver against the finest step.
    for s in &summaries {
        let finest = summaries
            .iter()
            .filter(|o| o.cell.same_but_dt(&s.cell) && o.cell.dt < s.cell.dt)
            .min_by(|a, b| a.cell.dt.total_cmp(&b.cell.dt));
        let Some(f) = finest else { continue };
        let rows: Vec<String> = s
            .per_receiver
            .iter()
            .zip(&f.per_receiver)
            .enumerate()
            .map(|(i, (a, b))| {
                let d = |k: usize| {
                    let (x, y) = (a[k], b[k]);
                    let rel = x.0 / y.0 - 1.0;
                    let se = ((x.1 / y.0).powi(2) + (y.1 * x.0 / (y.0 * y.0)).powi(2)).sqrt();
                    format!("{:+.2} % ({:+.1} se)", 100.0 * rel, rel / se)
                };
                format!("R{i:03} EDT {}, T30 {}", d(0), d(1))
            })
            .collect();
        println!(
            "DT {} against dt {}: {}",
            s.cell.label(),
            f.cell.dt,
            rows.join("; ")
        );
    }
}

#[test]
#[ignore = "evidence for M8, not a gate: runs TCR on M8's cells; run on purpose with SIMPA_M8_CELLS"]
fn m8_tcr_cells() {
    // TCR is deterministic: one run for each room, absorption and air of $SIMPA_M8_CELLS.
    let mut cells: Vec<Cell> = Vec::new();
    for c in cells_from_env() {
        if !cells
            .iter()
            .any(|o| o.room.name() == c.room.name() && o.alpha == c.alpha && o.air == c.air)
        {
            cells.push(c);
        }
    }
    let root = evidence_root("m8-tcr");
    let projects = cells
        .iter()
        .enumerate()
        .map(|(i, c)| (format!("{i} {} TCR", c.label()), c.project(1)))
        .collect();
    let done = run_all(&root, projects, "tcr");
    for (c, d) in cells.iter().zip(&done) {
        let t = &d.report["tcr"];
        let (mut worst_analytic, mut against) = (0.0f64, Vec::new());
        for (i, b) in t["bands"].as_array().unwrap().iter().enumerate() {
            let f = b["freq_hz"].as_f64().unwrap();
            let tcr = b["eyring"]["reverberation_time_s"].as_f64().unwrap();
            let analytic = t["analytic"]["bands"][i]["eyring_s"]["value"]
                .as_f64()
                .unwrap();
            worst_analytic = worst_analytic.max((tcr / analytic - 1.0).abs());
            against.push(format!(
                "{f} Hz {tcr:.4} s ({:+.2} %)",
                100.0 * (tcr / c.eyring(f) - 1.0)
            ));
        }
        println!(
            "TCR {}: Eyring against the analytic value (K 0.163, the solver's m) at most {:.2e} \
             relative; against Eyring with K = 24 ln10/343.2: {}",
            c.label(),
            worst_analytic,
            against.join("; ")
        );
    }
}
