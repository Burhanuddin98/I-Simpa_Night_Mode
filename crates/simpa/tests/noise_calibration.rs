//! Evidence for the calibrated Monte-Carlo noise model (pre-M8, piece B;
//! `docs/investigations/2026-09-25-noise-calibration/`). Each test runs SPPS many times or reads
//! such runs again; none is a gate, so each is ignored and run on purpose:
//!
//! `cargo test --release -p simpa --test noise_calibration -- --ignored --nocapture <name>`
//!
//! - `noise_calibration_runs`: every cell of [`CELLS`] (or those `$SIMPA_NOISE_CELLS` names,
//!   comma-separated ids) over SPPS seeds 1 to 10, each run's `simpa results --json` kept as
//!   `report.json` beside its project. The runs go under `$SIMPA_EVIDENCE_ROOT` (else cargo's test
//!   scratch space); `$SIMPA_EVIDENCE_JOBS` (default 8) runs go at once.
//! - `noise_calibration`: reads those reports (`$SIMPA_NOISE_FROM`, the folder the first test
//!   wrote) and measures, per cell, quantity and receiver-band, the spread of the values over the
//!   seeds against the model's standard deviation; `$SIMPA_NOISE_ROLES` (`calibration`,
//!   `validation` or both, comma-separated; `calibration` by default) says which cells are looked
//!   at, so that the validation cells stay unseen until the calibration is fixed. With
//!   `$SIMPA_NOISE_OUT` it writes the numbers there as JSON (the receipt committed under
//!   `docs/investigations/2026-09-25-noise-calibration/`).
//! - `tutorial1_at_upstreams_default`: tutorial 1 as shipped (27 third-octave bands, both
//!   receivers, 150,000 particles, random mode, and again in energetic mode), seeds 1 to 10: how
//!   many T30, EDT, C80 and D50 values come through. `$SIMPA_T1_FROM` reads an earlier run's
//!   folder again with this build's `simpa results`.

mod support;

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use serde_json::{Value, json};
use simpa_core::params::EnergySeries;
use simpa_core::params::decay::{self, Arrival};
use simpa_core::params::noise;
use simpa_core::schema::{
    self, BandKind, BandSet, ComputationMethod, MaterialId, PointReceiverId, Project,
    ReflectionLaw, Vec3,
};
use support::*;

// --- the cells ---------------------------------------------------------------------------------

/// A box room: tutorial 1's mesh scaled.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Room {
    /// 6 × 10 × 3 m, tutorial 1's own.
    Tutorial,
    /// 5 × 4 × 3 m, upstream's atmospheric-absorption validation room.
    Small,
    /// 20 × 4 × 3 m, elongated.
    Long,
}

impl Room {
    fn size(self) -> [f64; 3] {
        match self {
            Room::Tutorial => [6.0, 10.0, 3.0],
            Room::Small => [5.0, 4.0, 3.0],
            Room::Long => [20.0, 4.0, 3.0],
        }
    }

    /// The source: tutorial 1's, and in the other rooms a point a little off every symmetry
    /// plane, so that it lies on no internal facet of a symmetric mesh.
    fn source(self) -> [f64; 3] {
        match self {
            Room::Tutorial => [3.0, 5.0, 1.8],
            Room::Small => [2.52, 1.97, 1.53],
            Room::Long => [5.03, 1.97, 1.53],
        }
    }

    /// Six receivers, each at least 1 m from every wall and from the source.
    fn receivers(self) -> [[f64; 3]; 6] {
        match self {
            Room::Tutorial => [
                [1.0, 1.0, 1.8],
                [3.0, 7.0, 1.8],
                [5.0, 8.5, 1.2],
                [1.2, 8.0, 1.5],
                [4.8, 2.0, 2.0],
                [2.0, 3.0, 1.0],
            ],
            Room::Small => [
                [1.0, 1.0, 1.0],
                [4.0, 3.0, 2.0],
                [1.0, 3.0, 1.9],
                [4.0, 1.0, 1.2],
                [1.2, 2.0, 1.0],
                [3.9, 2.9, 1.1],
            ],
            Room::Long => [
                [2.0, 1.0, 1.0],
                [8.0, 3.0, 2.0],
                [12.0, 2.0, 1.5],
                [16.0, 1.2, 1.8],
                [18.5, 2.8, 1.1],
                [3.2, 3.0, 1.9],
            ],
        }
    }

    fn name(self) -> &'static str {
        match self {
            Room::Tutorial => "6x10x3",
            Room::Small => "5x4x3",
            Room::Long => "20x4x3",
        }
    }
}

/// What the room's surfaces are.
#[derive(Clone, Copy, Debug)]
enum Walls {
    /// Every surface of absorption `α`, Lambert with scattering 1 (M8's walls).
    Lambert(f64),
    /// Every surface of absorption `α`, specular (scattering 0).
    Specular(f64),
    /// Tutorial 1's own: ceiling 0.3, floor 0.1, walls 0.2, specular, and air absorption on.
    Tutorial,
    /// Absorption on one surface: the floor 0.6, the ceiling and walls 0.05, specular.
    DeadFloor,
}

impl Walls {
    fn label(self) -> String {
        match self {
            Walls::Lambert(a) => format!("Lambert a{a}"),
            Walls::Specular(a) => format!("specular a{a}"),
            Walls::Tutorial => "tutorial 1's materials, air on".into(),
            Walls::DeadFloor => "dead floor (0.6; 0.05 elsewhere), specular".into(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Role {
    Calibration,
    Validation,
}

/// One cell: a room and its walls, SPPS in `method` with `particles` per source, `duration` s in
/// steps of `dt`, `trans_epsilon` `eps`; octave bands 125 Hz to 4 kHz; six receivers.
#[derive(Clone, Copy, Debug)]
struct Cell {
    id: &'static str,
    role: Role,
    room: Room,
    walls: Walls,
    method: ComputationMethod,
    particles: u32,
    duration: f64,
    dt: f64,
    eps: f64,
}

impl Cell {
    fn label(&self) -> String {
        format!(
            "{} {} {} {:?} N {} {} s dt {} eps {}",
            self.id,
            self.room.name(),
            self.walls.label(),
            self.method,
            self.particles,
            self.duration,
            self.dt,
            self.eps
        )
    }

    fn config(&self) -> Value {
        json!({
            "id": self.id,
            "role": match self.role { Role::Calibration => "calibration", Role::Validation => "validation" },
            "room": self.room.name(),
            "walls": self.walls.label(),
            "method": match self.method { ComputationMethod::Random => "random", ComputationMethod::Energetic => "energetic" },
            "particles_per_source": self.particles,
            "duration_s": self.duration,
            "time_step_s": self.dt,
            "trans_epsilon": self.eps,
            "bands_hz": [125, 250, 500, 1000, 2000, 4000],
            "receivers": self.room.receivers().to_vec(),
            "source": self.room.source(),
            "seeds": SEEDS.collect::<Vec<u32>>(),
        })
    }
}

const SEEDS: std::ops::RangeInclusive<u32> = 1..=10;

use ComputationMethod::{Energetic, Random};
use Role::{Calibration as C, Validation as V};

const fn cell(
    id: &'static str,
    role: Role,
    room: Room,
    walls: Walls,
    method: ComputationMethod,
    particles: u32,
    duration: f64,
    dt: f64,
    eps: f64,
) -> Cell {
    Cell {
        id,
        role,
        room,
        walls,
        method,
        particles,
        duration,
        dt,
        eps,
    }
}

/// The cells, pre-registered before any was run (`PREREGISTER-noise.txt` in the investigation's
/// folder). Random mode's `trans_epsilon` is SPPS's default 5; it drops nothing there.
const CELLS: [Cell; 26] = [
    cell(
        "C-R1",
        C,
        Room::Tutorial,
        Walls::Lambert(0.1),
        Random,
        150_000,
        3.0,
        0.01,
        5.0,
    ),
    cell(
        "C-R2",
        C,
        Room::Tutorial,
        Walls::Lambert(0.1),
        Random,
        1_500_000,
        3.0,
        0.01,
        5.0,
    ),
    cell(
        "C-R3",
        C,
        Room::Small,
        Walls::Lambert(0.4),
        Random,
        150_000,
        1.0,
        0.001,
        5.0,
    ),
    cell(
        "C-R4",
        C,
        Room::Small,
        Walls::Lambert(0.4),
        Random,
        15_000_000,
        1.0,
        0.001,
        5.0,
    ),
    cell(
        "C-R5",
        C,
        Room::Tutorial,
        Walls::Tutorial,
        Random,
        150_000,
        2.0,
        0.01,
        5.0,
    ),
    cell(
        "C-R6",
        C,
        Room::Tutorial,
        Walls::Tutorial,
        Random,
        1_500_000,
        2.0,
        0.001,
        5.0,
    ),
    cell(
        "C-R7",
        C,
        Room::Tutorial,
        Walls::DeadFloor,
        Random,
        500_000,
        3.0,
        0.01,
        5.0,
    ),
    cell(
        "V-R1",
        V,
        Room::Long,
        Walls::Lambert(0.2),
        Random,
        500_000,
        2.0,
        0.01,
        5.0,
    ),
    cell(
        "V-R2",
        V,
        Room::Tutorial,
        Walls::Lambert(0.4),
        Random,
        600_000,
        1.0,
        0.001,
        5.0,
    ),
    cell(
        "V-R3",
        V,
        Room::Small,
        Walls::Lambert(0.05),
        Random,
        300_000,
        3.0,
        0.01,
        5.0,
    ),
    cell(
        "V-R4",
        V,
        Room::Tutorial,
        Walls::Tutorial,
        Random,
        600_000,
        2.0,
        0.01,
        5.0,
    ),
    cell(
        "V-R5",
        V,
        Room::Small,
        Walls::Specular(0.2),
        Random,
        5_000_000,
        1.5,
        0.01,
        5.0,
    ),
    cell(
        "V-R6",
        V,
        Room::Long,
        Walls::DeadFloor,
        Random,
        1_500_000,
        3.0,
        0.001,
        5.0,
    ),
    cell(
        "C-E1",
        C,
        Room::Tutorial,
        Walls::Lambert(0.1),
        Energetic,
        150_000,
        2.5,
        0.01,
        7.0,
    ),
    cell(
        "C-E2",
        C,
        Room::Tutorial,
        Walls::Lambert(0.1),
        Energetic,
        600_000,
        2.5,
        0.01,
        7.0,
    ),
    cell(
        "C-E3",
        C,
        Room::Small,
        Walls::Lambert(0.4),
        Energetic,
        150_000,
        0.5,
        0.001,
        9.0,
    ),
    cell(
        "C-E4",
        C,
        Room::Small,
        Walls::Lambert(0.4),
        Energetic,
        2_400_000,
        0.5,
        0.001,
        9.0,
    ),
    cell(
        "C-E5",
        C,
        Room::Tutorial,
        Walls::Tutorial,
        Energetic,
        150_000,
        2.0,
        0.01,
        5.0,
    ),
    cell(
        "C-E6",
        C,
        Room::Tutorial,
        Walls::Tutorial,
        Energetic,
        600_000,
        2.0,
        0.001,
        7.0,
    ),
    cell(
        "C-E7",
        C,
        Room::Tutorial,
        Walls::DeadFloor,
        Energetic,
        300_000,
        3.0,
        0.01,
        7.0,
    ),
    cell(
        "V-E1",
        V,
        Room::Long,
        Walls::Lambert(0.2),
        Energetic,
        300_000,
        1.5,
        0.01,
        7.0,
    ),
    cell(
        "V-E2",
        V,
        Room::Tutorial,
        Walls::Lambert(0.4),
        Energetic,
        150_000,
        0.6,
        0.001,
        9.0,
    ),
    cell(
        "V-E3",
        V,
        Room::Small,
        Walls::Lambert(0.05),
        Energetic,
        150_000,
        3.0,
        0.01,
        7.0,
    ),
    cell(
        "V-E4",
        V,
        Room::Tutorial,
        Walls::Tutorial,
        Energetic,
        1_500_000,
        2.0,
        0.01,
        5.0,
    ),
    cell(
        "V-E5",
        V,
        Room::Small,
        Walls::Specular(0.2),
        Energetic,
        600_000,
        1.5,
        0.001,
        7.0,
    ),
    cell(
        "V-E6",
        V,
        Room::Long,
        Walls::DeadFloor,
        Energetic,
        300_000,
        3.0,
        0.01,
        7.0,
    ),
];

/// Tutorial 1's box (`rooms/tutorial1_box.simpa`) on the octave bands 125 Hz to 4 kHz, without its
/// surface receiver and the intersection logs, seeded `seed` (as `m8_evidence.rs` builds it).
fn tutorial_octaves(seed: u32) -> Project {
    let mut p = schema::load(&fixture("rooms/tutorial1_box.simpa")).unwrap();
    let all = p.bands.frequencies_hz.clone();
    let keep = [125u32, 250, 500, 1000, 2000, 4000];
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
            // Tutorial 1's receivers pin upstream's solver ids; new receivers take their own.
            r.solver_id = None;
            r
        })
        .collect();
}

/// The cell's project, seeded `seed`.
fn project(c: &Cell, seed: u32) -> Project {
    let mut p = tutorial_octaves(seed);
    let [lx, ly, lz] = c.room.size();
    p.geometry.vertices = p
        .geometry
        .vertices
        .iter()
        .map(|v| {
            let [x, y, z] = v.to_array();
            Vec3::new(x / 6.0 * lx, y / 10.0 * ly, z / 3.0 * lz)
        })
        .collect();
    let n = p.bands.len();
    let material = |id: u128, name: &str, alpha: f64, lambert: bool| {
        let mut m = p.materials[0].clone();
        m.id = MaterialId::from_u128(0x0c0b_e000_0000_4000_8000_0000_0009_0000 + id);
        m.name = name.to_string();
        m.absorption = vec![schema::F64::new(alpha); n];
        m.scattering = vec![schema::F64::new(if lambert { 1.0 } else { 0.0 }); n];
        m.reflection_law = if lambert {
            ReflectionLaw::Lambert
        } else {
            ReflectionLaw::Specular
        }
        .into();
        m.transmission_loss_db = None;
        m.solver_id = None;
        m
    };
    // Each surface group's (floor, the rest) material.
    let set = match c.walls {
        Walls::Tutorial => None,
        Walls::Lambert(a) => Some((material(1, "uniform", a, true), None)),
        Walls::Specular(a) => Some((material(1, "uniform", a, false), None)),
        Walls::DeadFloor => Some((
            material(1, "rest", 0.05, false),
            Some(material(2, "floor", 0.6, false)),
        )),
    };
    if let Some((rest, floor)) = set {
        for g in &mut p.surface_groups {
            g.material = match (&floor, g.name.as_str()) {
                (Some(f), "Floor") => f.id,
                _ => rest.id,
            };
        }
        p.materials = std::iter::once(rest).chain(floor).collect();
        p.solvers.spps.air_absorption = false;
        p.solvers.tcr.air_absorption = false;
    }
    let s = c.room.source();
    p.sources[0].position = Vec3::new(s[0], s[1], s[2]);
    with_receivers(&mut p, &c.room.receivers());
    let spps = &mut p.solvers.spps;
    spps.method = c.method;
    spps.particles_per_source = c.particles;
    spps.duration_s = schema::F64::new(c.duration);
    spps.time_step_s = schema::F64::new(c.dt);
    spps.extinction_exponent = schema::F64::new(c.eps);
    p
}

/// The cells `$SIMPA_NOISE_CELLS` names, or all of them.
fn chosen_cells() -> Vec<Cell> {
    match std::env::var("SIMPA_NOISE_CELLS") {
        Ok(s) if !s.trim().is_empty() => s
            .split(',')
            .map(|id| {
                *CELLS
                    .iter()
                    .find(|c| c.id == id.trim())
                    .unwrap_or_else(|| panic!("no cell {id:?}"))
            })
            .collect(),
        _ => CELLS.to_vec(),
    }
}

fn jobs() -> usize {
    std::env::var("SIMPA_EVIDENCE_JOBS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(8)
}

/// Where the runs go: `$SIMPA_EVIDENCE_ROOT/<label>-<stamp>`, else cargo's test scratch space.
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

/// Runs every `(folder, project)` through SPPS, `jobs()` at a time, each in its own runs folder
/// under `root/<folder>`, and keeps `simpa results --json` as `root/<folder>/report.json`. A run
/// that is not OK fails the test; results `simpa results` refuses (exit 6) are kept as the refusal.
/// Returns the wall time of each run, in the given order.
fn run_all(root: &Path, projects: &[(String, Project)]) -> Vec<f64> {
    solver_exe("spps.exe");
    let next = AtomicUsize::new(0);
    let secs: Mutex<Vec<(usize, f64)>> = Mutex::new(Vec::new());
    let clock = std::time::Instant::now();
    std::thread::scope(|scope| {
        for _ in 0..jobs().min(projects.len()) {
            scope.spawn(|| {
                loop {
                    let i = next.fetch_add(1, Ordering::SeqCst);
                    let Some((folder, p)) = projects.get(i) else {
                        break;
                    };
                    let dir = root.join(folder);
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
                    let s = t0.elapsed().as_secs_f64();
                    let m = json(&o);
                    assert_eq!(o.code, 0, "{folder}: {o:#?}");
                    assert_eq!(m["verdict"]["status"], "OK", "{folder}");
                    let run = run_dir(&m);
                    let r = simpa_run(&[
                        "results".to_string(),
                        run.display().to_string(),
                        "--json".into(),
                    ]);
                    assert!(r.code == 0 || r.code == 6, "{folder}: {r:#?}");
                    std::fs::write(dir.join("report.json"), &r.stdout).unwrap();
                    println!(
                        "[{:>6.0} s] {folder}: {s:.1} s",
                        clock.elapsed().as_secs_f64()
                    );
                    secs.lock().unwrap().push((i, s));
                }
            });
        }
    });
    let mut v = secs.into_inner().unwrap();
    v.sort_by_key(|(i, _)| *i);
    v.into_iter().map(|(_, s)| s).collect()
}

/// A rough cost of one run, for running the longest first.
fn cost(c: &Cell) -> f64 {
    let life = match c.method {
        Random => 0.1,
        Energetic => c.duration,
    };
    f64::from(c.particles) * life / c.dt.powf(0.15)
}

#[test]
#[ignore = "evidence for the noise model, not a gate: runs SPPS on 26 cells over ten seeds; run on \
            purpose"]
fn noise_calibration_runs() {
    let root = evidence_root("noise-cal");
    let mut cells = chosen_cells();
    let manifest: Vec<Value> = cells.iter().map(Cell::config).collect();
    std::fs::write(
        root.join("cells.json"),
        serde_json::to_string_pretty(&manifest).unwrap(),
    )
    .unwrap();
    cells.sort_by(|a, b| cost(b).total_cmp(&cost(a)));
    let mut projects = Vec::new();
    for c in &cells {
        for seed in SEEDS {
            projects.push((format!("{}/seed{seed:02}", c.id), project(c, seed)));
        }
    }
    let secs = run_all(&root, &projects);
    let mut times: Vec<Value> = Vec::new();
    for c in &cells {
        let s: Vec<f64> = projects
            .iter()
            .zip(&secs)
            .filter(|((f, _), _)| f.starts_with(&format!("{}/", c.id)))
            .map(|(_, s)| *s)
            .collect();
        println!(
            "{}: wall {:.0} to {:.0} s per run",
            c.label(),
            s.iter().copied().fold(f64::INFINITY, f64::min),
            s.iter().copied().fold(0.0, f64::max)
        );
        times.push(json!({"id": c.id, "wall_s": s}));
    }
    std::fs::write(
        root.join("wall.json"),
        serde_json::to_string_pretty(&times).unwrap(),
    )
    .unwrap();
    println!("runs in {}", root.display());
}

// --- the analysis ------------------------------------------------------------------------------

/// The eight quantities in `params::noise`'s order, their JSON names and whether their standard
/// deviation is relative.
const EIGHT: [(&str, bool); 8] = [
    ("spl_db", false),
    ("edt_s", true),
    ("t20_s", true),
    ("t30_s", true),
    ("c50_db", false),
    ("c80_db", false),
    ("d50", false),
    ("ts_s", false),
];

/// One receiver-band of one run, as the analysis needs it.
struct Band {
    dt: f64,
    energy: Vec<f64>,
    arrival: Arrival,
    mean_deposit: f64,
    /// The share of the emitted energy alive at the end of each step (the room table over the
    /// sources' power, both times `ρc`).
    alive: Vec<f64>,
}

/// Every receiver-band of a report, receivers then bands.
fn bands_of(rep: &Value) -> Vec<Band> {
    let s = &rep["spps"];
    let dt = s["time_step_s"].as_f64().unwrap();
    let half = s["receiver_crossing_s"].as_f64().unwrap() / 2.0;
    let floats = |v: &Value| -> Vec<f64> {
        v.as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_f64().unwrap())
            .collect()
    };
    let mut out = Vec::new();
    for r in s["point_receivers"].as_array().unwrap() {
        let t_a = r["arrival_s"].as_f64().unwrap();
        for (bi, b) in r["bands"].as_array().unwrap().iter().enumerate() {
            let power = b["source_power_rho_c"].as_f64().unwrap();
            let room = floats(&s["total_energy"][bi]["energy"]);
            out.push(Band {
                dt,
                energy: floats(&b["energy_pa2"]),
                arrival: Arrival::spread(t_a, half),
                mean_deposit: b["noise_model"]["mean_deposit"].as_f64().unwrap(),
                alive: room.iter().map(|e| e / power).collect(),
            });
        }
    }
    out
}

/// The series as the bootstrap takes its resamples: complete, the early reverberation unresolved
/// as `core::results` reads every SPPS series.
fn plain_series(dt: f64, v: Vec<f64>) -> Option<EnergySeries> {
    EnergySeries::complete(dt, v)
        .ok()
        .map(EnergySeries::with_early_reverberation_unresolved)
}

/// The eight values of a series, `None` where refused.
fn values(s: &EnergySeries, arrival: Arrival) -> [Option<f64>; 8] {
    let p = decay::evaluate(s, arrival);
    [
        p.spl_db.ok(),
        p.edt.ok().map(|f| f.t_s),
        p.t20.ok().map(|f| f.t_s),
        p.t30.ok().map(|f| f.t_s),
        p.c50_db.ok(),
        p.c80_db.ok(),
        p.d50.ok(),
        p.ts_s.ok(),
    ]
}

/// The candidate structures of the model.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Structure {
    /// M7's: every crossing deposits `d̄` on average (`params::noise` as it stood).
    Constant,
    /// A crossing in step `k` deposits `d̄` times the particles' mean energy then over their start
    /// energy, read from the room table at the start of the step.
    MeanEnergy,
}

const STRUCTURES: [Structure; 2] = [Structure::Constant, Structure::MeanEnergy];

/// The per-bin mean deposit under `st`.
fn deposits(b: &Band, st: Structure) -> Vec<f64> {
    match st {
        Structure::Constant => vec![b.mean_deposit; b.energy.len()],
        Structure::MeanEnergy => (0..b.energy.len())
            .map(|k| {
                let f = if k == 0 {
                    1.0
                } else {
                    b.alive.get(k - 1).copied().unwrap_or(1.0)
                };
                b.mean_deposit * f.clamp(f64::MIN_POSITIVE, 1.0)
            })
            .collect(),
    }
}

/// `params::noise`'s bootstrap with a per-bin mean deposit: each quantity's standard deviation
/// over the resamples that give it (relative to the series' value for the decay times), `None`
/// where fewer than two do or the series gives no value.
fn bootstrap(s: &EnergySeries, arrival: Arrival, dep: &[f64]) -> [Option<f64>; 8] {
    let base = values(s, arrival);
    let mut rng = noise::Rng::new(noise::SEED);
    let mut got: Vec<Vec<f64>> = vec![Vec::new(); 8];
    for _ in 0..noise::RESAMPLES {
        let drawn: Vec<f64> = s
            .values()
            .iter()
            .zip(dep)
            .map(|(&e, &d)| {
                if e <= 0.0 {
                    return 0.0;
                }
                let lambda = e / d;
                if lambda > 30.0 {
                    (e + (noise::CHORD_FACTOR * d * e).sqrt() * rng.normal()).max(0.0)
                } else {
                    let n = rng.poisson(lambda);
                    (0..n).map(|_| d * 1.5 * rng.uniform().sqrt()).sum()
                }
            })
            .collect();
        if let Some(r) = plain_series(s.dt(), drawn) {
            for (i, v) in values(&r, arrival).into_iter().enumerate() {
                if let Some(v) = v {
                    got[i].push(v);
                }
            }
        }
    }
    let mut out = [None; 8];
    for i in 0..8 {
        if let (Some(v), Some(sd)) = (base[i], noise::standard_deviation(&got[i])) {
            out[i] = Some(if EIGHT[i].1 { sd / v.abs() } else { sd });
        }
    }
    out
}

/// One run's receiver-bands: each quantity's value, and its standard deviation under each
/// structure.
struct RunNumbers {
    values: Vec<[Option<f64>; 8]>,
    sd: Vec<Vec<[Option<f64>; 8]>>,
}

fn numbers(rep: &Value, structures: &[Structure]) -> RunNumbers {
    let bands = bands_of(rep);
    let mut values_out = Vec::new();
    let mut sd = vec![Vec::new(); structures.len()];
    for b in &bands {
        let s = plain_series(b.dt, b.energy.clone());
        values_out.push(s.as_ref().map_or([None; 8], |s| values(s, b.arrival)));
        for (si, st) in structures.iter().enumerate() {
            sd[si].push(
                s.as_ref()
                    .map_or([None; 8], |s| bootstrap(s, b.arrival, &deposits(b, *st))),
            );
        }
    }
    RunNumbers {
        values: values_out,
        sd,
    }
}

/// The inverse of the chi-square distribution's CDF at `p` with `dof` degrees of freedom
/// (Wilson–Hilferty, good to about 0.1 % above 30 degrees of freedom).
fn chi2_quantile(dof: f64, p: f64) -> f64 {
    let z = normal_quantile(p);
    let a = 2.0 / (9.0 * dof);
    dof * (1.0 - a + z * a.sqrt()).powi(3)
}

/// The standard normal quantile (Acklam's rational approximation, 1e-9 relative).
fn normal_quantile(p: f64) -> f64 {
    const A: [f64; 6] = [
        -3.969_683_028_665_376e1,
        2.209_460_984_245_205e2,
        -2.759_285_104_469_687e2,
        1.383_577_518_672_69e2,
        -3.066_479_806_614_716e1,
        2.506_628_277_459_239,
    ];
    const B: [f64; 5] = [
        -5.447_609_879_822_406e1,
        1.615_858_368_580_409e2,
        -1.556_989_798_598_866e2,
        6.680_131_188_771_972e1,
        -1.328_068_155_288_572e1,
    ];
    const C: [f64; 6] = [
        -7.784_894_002_430_293e-3,
        -3.223_964_580_411_365e-1,
        -2.400_758_277_161_838,
        -2.549_732_539_343_734,
        4.374_664_141_464_968,
        2.938_163_982_698_783,
    ];
    const D: [f64; 4] = [
        7.784_695_709_041_462e-3,
        3.224_671_290_700_398e-1,
        2.445_134_137_142_996,
        3.754_408_661_907_416,
    ];
    let pl = 0.024_25;
    if p < pl {
        let q = (-2.0 * p.ln()).sqrt();
        (((((C[0] * q + C[1]) * q + C[2]) * q + C[3]) * q + C[4]) * q + C[5])
            / ((((D[0] * q + D[1]) * q + D[2]) * q + D[3]) * q + 1.0)
    } else if p <= 1.0 - pl {
        let q = p - 0.5;
        let r = q * q;
        (((((A[0] * r + A[1]) * r + A[2]) * r + A[3]) * r + A[4]) * r + A[5]) * q
            / (((((B[0] * r + B[1]) * r + B[2]) * r + B[3]) * r + B[4]) * r + 1.0)
    } else {
        -normal_quantile(1.0 - p)
    }
}

/// The mean and the sample standard deviation.
fn mean_sd(x: &[f64]) -> (f64, f64) {
    let n = x.len() as f64;
    let m = x.iter().sum::<f64>() / n;
    let v = x.iter().map(|v| (v - m).powi(2)).sum::<f64>() / (n - 1.0);
    (m, v.sqrt())
}

/// The cells `$SIMPA_NOISE_ROLES` lets the analysis look at.
fn roles() -> Vec<Role> {
    let s = std::env::var("SIMPA_NOISE_ROLES").unwrap_or_else(|_| "calibration".into());
    s.split(',')
        .map(|r| match r.trim() {
            "calibration" => Role::Calibration,
            "validation" => Role::Validation,
            other => panic!("role {other:?}"),
        })
        .collect()
}

#[test]
#[ignore = "evidence for the noise model, not a gate: reads the calibration runs; run on purpose \
            with SIMPA_NOISE_FROM"]
fn noise_calibration() {
    let from = PathBuf::from(std::env::var("SIMPA_NOISE_FROM").expect("SIMPA_NOISE_FROM"));
    let roles = roles();
    let cells: Vec<Cell> = chosen_cells()
        .into_iter()
        .filter(|c| roles.contains(&c.role))
        .collect();
    let mut out_cells = Vec::new();
    for c in &cells {
        let reports: Vec<Value> = SEEDS
            .map(|seed| {
                let p = from
                    .join(c.id)
                    .join(format!("seed{seed:02}"))
                    .join("report.json");
                serde_json::from_str(
                    &std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display())),
                )
                .unwrap()
            })
            .collect();
        for (i, r) in reports.iter().enumerate() {
            assert!(
                r.get("refused").is_none(),
                "{} seed {}: refused",
                c.id,
                i + 1
            );
        }
        let t0 = std::time::Instant::now();
        // Every run's numbers, in parallel.
        let runs: Vec<RunNumbers> = std::thread::scope(|scope| {
            let hs: Vec<_> = reports
                .iter()
                .map(|r| scope.spawn(|| numbers(r, &STRUCTURES)))
                .collect();
            hs.into_iter().map(|h| h.join().unwrap()).collect()
        });
        let n = runs.len();
        let rbs = runs[0].values.len();
        println!(
            "\nCELL {} ({} seeds, {rbs} receiver-bands; analysed in {:.0} s)",
            c.label(),
            n,
            t0.elapsed().as_secs_f64()
        );
        let first = &reports[0]["spps"]["point_receivers"];
        let mut quantities = serde_json::Map::new();
        for (qi, (q, relative)) in EIGHT.iter().enumerate() {
            let mut rows = Vec::new();
            let mut sums = vec![(0.0f64, 0.0f64); STRUCTURES.len()];
            let mut short = 0;
            for rb in 0..rbs {
                let got: Vec<f64> = runs.iter().filter_map(|r| r.values[rb][qi]).collect();
                let sds: Vec<Vec<f64>> = (0..STRUCTURES.len())
                    .map(|si| runs.iter().filter_map(|r| r.sd[si][rb][qi]).collect())
                    .collect();
                if got.len() < n || sds.iter().any(|s| s.len() < n) {
                    short += 1;
                    continue;
                }
                let (m, sd) = mean_sd(&got);
                let observed = if *relative { sd / m.abs() } else { sd };
                let predicted: Vec<f64> = sds
                    .iter()
                    .map(|s| (s.iter().map(|x| x * x).sum::<f64>() / n as f64).sqrt())
                    .collect();
                for (si, p) in predicted.iter().enumerate() {
                    sums[si].0 += observed * observed;
                    sums[si].1 += p * p;
                }
                let receivers = first.as_array().unwrap().len();
                let per = rbs / receivers;
                rows.push(json!({
                    "receiver": rb / per,
                    "freq_hz": first[rb / per]["bands"][rb % per]["freq_hz"],
                    "mean": m,
                    "observed_sd": observed,
                    "predicted_sd": STRUCTURES.iter().zip(&predicted)
                        .map(|(s, p)| (format!("{s:?}"), json!(p)))
                        .collect::<serde_json::Map<_, _>>(),
                }));
            }
            let dof = (rows.len() * (n - 1)) as f64;
            let mut line = format!("  {q:<7} {:>2} rb ({short} short):", rows.len());
            for (si, st) in STRUCTURES.iter().enumerate() {
                if rows.is_empty() {
                    break;
                }
                let ratio = (sums[si].0 / sums[si].1).sqrt();
                let ucb = ratio * (dof / chi2_quantile(dof, 0.05)).sqrt();
                let lcb = ratio * (dof / chi2_quantile(dof, 0.95)).sqrt();
                line += &format!(
                    " {st:?} ratio {ratio:.3} [{lcb:.3}, {ucb:.3}] obs rms {:.4}{}",
                    (sums[si].0 / rows.len() as f64).sqrt(),
                    if *relative { " rel" } else { "" }
                );
            }
            println!("{line}");
            quantities.insert(q.to_string(), json!(rows));
        }
        out_cells.push(json!({"cell": c.config(), "quantities": quantities}));
    }
    if let Ok(out) = std::env::var("SIMPA_NOISE_OUT") {
        std::fs::write(
            &out,
            serde_json::to_string(&json!({"cells": out_cells})).unwrap(),
        )
        .unwrap();
        println!("written {out}");
    }
}

// --- tutorial 1 at upstream's default ------------------------------------------------------------

/// How many receiver-bands give each of T30, EDT, C80 and D50 a value, and why the others are
/// refused, over the reports.
fn through(label: &str, reports: &[Value]) -> Value {
    let mut out = serde_json::Map::new();
    for q in [
        "t30_s", "edt_s", "c80_db", "d50", "t20_s", "c50_db", "ts_s", "spl_db",
    ] {
        let (mut values, mut total) = (0usize, 0usize);
        let mut why: std::collections::BTreeMap<String, usize> = Default::default();
        for rep in reports {
            for r in rep["spps"]["point_receivers"].as_array().unwrap() {
                for b in r["bands"].as_array().unwrap() {
                    total += 1;
                    let p = &b["parameters"][q];
                    if p["value"].is_f64() {
                        values += 1;
                    } else {
                        let w = &p["not_evaluable"]["error"]["why"]["why"];
                        *why.entry(
                            w.as_str()
                                .unwrap_or(p["not_evaluable"]["code"].as_str().unwrap_or("?"))
                                .to_string(),
                        )
                        .or_default() += 1;
                    }
                }
            }
        }
        println!(
            "{label} {q:<7}: {values} of {total} receiver-bands give a value; refused {why:?}"
        );
        out.insert(
            q.to_string(),
            json!({"values": values, "receiver_bands": total, "refused": why}),
        );
    }
    Value::Object(out)
}

#[test]
#[ignore = "evidence for the noise model, not a gate: runs tutorial 1 as shipped over ten seeds in \
            both methods; run on purpose"]
fn tutorial1_at_upstreams_default() {
    let mut projects = Vec::new();
    for method in [Random, Energetic] {
        for seed in SEEDS {
            let mut p = schema::load(&fixture("rooms/tutorial1_box.simpa")).unwrap();
            p.solvers.spps.random_seed = seed;
            p.solvers.spps.method = method;
            p.surface_receivers.clear();
            p.solvers.spps.save_surface_intersections = false;
            p.solvers.spps.save_receiver_intersections = false;
            projects.push((format!("{method:?}/seed{seed:02}"), p));
        }
    }
    let root = match std::env::var_os("SIMPA_T1_FROM") {
        Some(from) if !from.is_empty() => {
            // Read again with this build's `simpa results`.
            let from = PathBuf::from(from);
            for (folder, _) in &projects {
                let runs = from.join(folder).join("runs");
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
                let name =
                    std::env::var("SIMPA_T1_REPORT").unwrap_or_else(|_| "report.json".into());
                std::fs::write(from.join(folder).join(name), &r.stdout).unwrap();
            }
            from
        }
        _ => {
            let root = evidence_root("tutorial1-default");
            run_all(&root, &projects);
            root
        }
    };
    let name = std::env::var("SIMPA_T1_REPORT").unwrap_or_else(|_| "report.json".into());
    let mut summary = serde_json::Map::new();
    for method in [Random, Energetic] {
        let reports: Vec<Value> = projects
            .iter()
            .filter(|(f, _)| f.starts_with(&format!("{method:?}/")))
            .map(|(f, _)| {
                serde_json::from_str(&std::fs::read_to_string(root.join(f).join(&name)).unwrap())
                    .unwrap()
            })
            .collect();
        summary.insert(
            format!("{method:?}"),
            through(
                &format!("tutorial 1, {method:?}, 150,000 particles, 10 seeds:"),
                &reports,
            ),
        );
    }
    println!(
        "{}",
        serde_json::to_string_pretty(&Value::Object(summary)).unwrap()
    );
    println!("runs in {}", root.display());
}
