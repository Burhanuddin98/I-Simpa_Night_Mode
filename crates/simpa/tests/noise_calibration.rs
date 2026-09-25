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
//! - `noise_calibration`: reads those reports (`$SIMPA_NOISE_FROM`, the folders the first test
//!   wrote, `;`-separated) and measures, per cell, quantity and receiver-band, the spread of the
//!   values over the seeds against the model's standard deviation; applies the pre-registered
//!   rules (factors, margins, validation, `1/√N`, the named counts on the pairs);
//!   `$SIMPA_NOISE_ROLES` (`calibration`, `validation`, `validation2`, comma-separated;
//!   `calibration` by default) says which cells are looked at, so that validation cells stay
//!   unseen until the calibration is fixed. With `$SIMPA_NOISE_OUT` it writes the numbers there as
//!   JSON (the receipt committed under `docs/investigations/2026-09-25-noise-calibration/`).
//! - `tutorial1_at_upstreams_default`: tutorial 1 as shipped (27 third-octave bands, both
//!   receivers, 150,000 particles, random mode, and again in energetic mode), seeds 1 to 10: how
//!   many T30, EDT, C80 and D50 values come through. `$SIMPA_T1_FROM` reads an earlier run's
//!   folder again with this build's `simpa results`, keeping each report as `$SIMPA_T1_REPORT`
//!   (default `report.json`) beside the run, or under `$SIMPA_T1_OUT` when it is set.
//!
//! Round 3 (`PREREGISTER.txt`, "ROUND 3" and its addendum) reads its cells with
//! `$SIMPA_NOISE_ROLES` taking `calibration3` and then `validation3`: the first prints what the
//! rules derive, the second judges the V3 cells with the numbers committed in
//! `params::noise::calibration`, the named counts on every pair and, per cell, how many values
//! come through the shipped model and why the rest are refused.

mod support;

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use serde_json::{Value, json};
use simpa_core::params::EnergySeries;
use simpa_core::params::decay::{self, Arrival};
use simpa_core::params::noise::{self, NoiseModel};
use simpa_core::schema::{
    self, BandKind, BandSet, ComputationMethod, MaterialId, PointReceiverId, Project,
    ReflectionLaw, Vec3,
};
use support::*;

/// The rules, shared with the suite's check of the committed receipt.
#[path = "../../simpa-core/tests/common/noise_calibration.rs"]
mod calibration;

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
    /// 30 × 4 × 3 m, a corridor (round 2).
    Corridor,
    /// 20 × 8 × 6 m, a hall (round 2).
    Hall,
    /// 10 × 10 × 10 m (round 2).
    Cube,
}

impl Room {
    fn size(self) -> [f64; 3] {
        match self {
            Room::Tutorial => [6.0, 10.0, 3.0],
            Room::Small => [5.0, 4.0, 3.0],
            Room::Long => [20.0, 4.0, 3.0],
            Room::Corridor => [30.0, 4.0, 3.0],
            Room::Hall => [20.0, 8.0, 6.0],
            Room::Cube => [10.0, 10.0, 10.0],
        }
    }

    /// The source: tutorial 1's, and in the other rooms a point a little off every symmetry
    /// plane, so that it lies on no internal facet of a symmetric mesh.
    fn source(self) -> [f64; 3] {
        match self {
            Room::Tutorial => [3.0, 5.0, 1.8],
            Room::Small => [2.52, 1.97, 1.53],
            Room::Long => [5.03, 1.97, 1.53],
            Room::Corridor => [7.03, 1.97, 1.53],
            Room::Hall => [5.03, 3.97, 2.53],
            Room::Cube => [4.03, 4.97, 3.53],
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
            Room::Corridor => [
                [2.0, 1.0, 1.0],
                [10.0, 3.0, 2.0],
                [16.0, 2.0, 1.5],
                [22.0, 1.2, 1.8],
                [28.0, 2.8, 1.1],
                [4.2, 3.0, 1.9],
            ],
            Room::Hall => [
                [2.0, 1.5, 1.2],
                [10.0, 6.5, 4.5],
                [15.0, 4.0, 2.0],
                [18.5, 1.5, 3.5],
                [8.0, 2.0, 5.0],
                [3.0, 6.5, 1.5],
            ],
            Room::Cube => [
                [2.0, 2.0, 2.0],
                [8.0, 8.0, 8.0],
                [8.0, 2.0, 5.0],
                [2.0, 8.0, 3.0],
                [5.0, 5.0, 8.5],
                [7.0, 3.0, 1.5],
            ],
        }
    }

    fn name(self) -> &'static str {
        match self {
            Room::Tutorial => "6x10x3",
            Room::Small => "5x4x3",
            Room::Long => "20x4x3",
            Room::Corridor => "30x4x3",
            Room::Hall => "20x8x6",
            Room::Cube => "10x10x10",
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
    /// The same on the ceiling: the ceiling 0.6, the floor and walls 0.05, specular (round 2).
    DeadCeiling,
    /// The dead floor with every surface scattering `s` by Lambert's law (round 2).
    DeadFloorScattering(f64),
    /// The floor, the ceiling and the four walls each of their own absorption, every surface
    /// scattering `scattering`: 1 is Lambert's law throughout (M8's walls), 0 specular (round 3).
    Mixed {
        floor: f64,
        ceiling: f64,
        walls: f64,
        scattering: f64,
    },
}

impl Walls {
    /// Every face has the same absorption in every band (A2): tutorial 1's materials do not.
    fn uniform(self) -> bool {
        match self {
            Walls::Lambert(_) | Walls::Specular(_) => true,
            Walls::Mixed {
                floor,
                ceiling,
                walls,
                ..
            } => floor == ceiling && ceiling == walls,
            Walls::Tutorial
            | Walls::DeadFloor
            | Walls::DeadCeiling
            | Walls::DeadFloorScattering(_) => false,
        }
    }

    /// Every face reflects by Lambert's law with scattering 1 (the report's `lambert_walls`).
    fn lambert(self) -> bool {
        match self {
            Walls::Lambert(_) => true,
            Walls::Mixed { scattering, .. } => scattering == 1.0,
            Walls::Specular(_)
            | Walls::Tutorial
            | Walls::DeadFloor
            | Walls::DeadCeiling
            | Walls::DeadFloorScattering(_) => false,
        }
    }

    fn label(self) -> String {
        match self {
            Walls::Lambert(a) => format!("Lambert a{a}"),
            Walls::Specular(a) => format!("specular a{a}"),
            Walls::Tutorial => "tutorial 1's materials, air on".into(),
            Walls::DeadFloor => "dead floor (0.6; 0.05 elsewhere), specular".into(),
            Walls::DeadCeiling => "dead ceiling (0.6; 0.05 elsewhere), specular".into(),
            Walls::DeadFloorScattering(sc) => {
                format!("dead floor (0.6; 0.05 elsewhere), scattering {sc}")
            }
            Walls::Mixed {
                floor,
                ceiling,
                walls,
                scattering,
            } => {
                let law = if scattering == 1.0 {
                    "Lambert".to_string()
                } else if scattering == 0.0 {
                    "specular".to_string()
                } else {
                    format!("scattering {scattering}")
                };
                format!("{law}: floor a{floor}, ceiling a{ceiling}, walls a{walls}")
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Role {
    Calibration,
    Validation,
    /// Round 2's validation cells, held out of both rounds' factors.
    Validation2,
    /// Round 3's new calibration cells (`PREREGISTER.txt`, "ROUND 3"); round 3 calibrates on
    /// these and on every cell of rounds 1 and 2.
    Calibration3,
    /// Round 3's validation cells, held out of every round's factors.
    Validation3,
    /// Round 4's cells (`PREREGISTER.txt`, "ROUND 4"): held out of every factor; each validates
    /// and some are the higher count of a pair.
    Validation4,
}

/// One cell: a room and its walls, SPPS in `method` with `particles` per source, `duration` s in
/// steps of `dt`, `trans_epsilon` `eps`, receiver spheres of radius `radius` m; octave bands
/// 125 Hz to 4 kHz; six receivers.
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
    radius: f64,
}

impl Cell {
    fn label(&self) -> String {
        format!(
            "{} {} {} {:?} N {} {} s dt {} eps {} R {}",
            self.id,
            self.room.name(),
            self.walls.label(),
            self.method,
            self.particles,
            self.duration,
            self.dt,
            self.eps,
            self.radius
        )
    }

    fn config(&self) -> Value {
        json!({
            "id": self.id,
            "role": match self.role {
                Role::Calibration => "calibration",
                Role::Validation => "validation",
                Role::Validation2 => "validation2",
                Role::Calibration3 => "calibration3",
                Role::Validation3 => "validation3",
                Role::Validation4 => "validation4",
            },
            "room": self.room.name(),
            "walls": self.walls.label(),
            "method": match self.method { ComputationMethod::Random => "random", ComputationMethod::Energetic => "energetic" },
            "particles_per_source": self.particles,
            "duration_s": self.duration,
            "time_step_s": self.dt,
            "trans_epsilon": self.eps,
            "receiver_radius_m": self.radius,
            "bands_hz": [125, 250, 500, 1000, 2000, 4000],
            "receivers": self.room.receivers().to_vec(),
            "source": self.room.source(),
            "seeds": SEEDS.collect::<Vec<u32>>(),
        })
    }

    /// Every receiver sphere clear of the walls by 5 cm and of the source by 40 cm, so that the
    /// project validates (`receiver_sphere_crosses_surface`) and no source sits in a sphere.
    fn check_geometry(&self) {
        let size = self.room.size();
        let s = self.room.source();
        for r in self.room.receivers() {
            let clearance = (0..3)
                .map(|i| r[i].min(size[i] - r[i]))
                .fold(f64::INFINITY, f64::min);
            assert!(
                clearance >= self.radius + 0.05,
                "{}: receiver {r:?} is {clearance} m from a wall, radius {}",
                self.id,
                self.radius
            );
            let d = ((r[0] - s[0]).powi(2) + (r[1] - s[1]).powi(2) + (r[2] - s[2]).powi(2)).sqrt();
            assert!(
                d >= self.radius + 0.4,
                "{}: receiver {r:?} is {d} m from the source, radius {}",
                self.id,
                self.radius
            );
        }
    }
}

const SEEDS: std::ops::RangeInclusive<u32> = 1..=10;

use ComputationMethod::{Energetic, Random};
use Role::{
    Calibration as C, Calibration3 as C3, Validation as V, Validation2 as W, Validation3 as V3,
    Validation4 as V4,
};

/// SPPS's default receiver radius, every cell's in rounds 1 and 2.
const R0: f64 = 0.31;

#[allow(clippy::too_many_arguments)]
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
    cell_r(
        id, role, room, walls, method, particles, duration, dt, eps, R0,
    )
}

/// [`cell`] with receiver spheres of radius `radius` m (round 3).
#[allow(clippy::too_many_arguments)]
const fn cell_r(
    id: &'static str,
    role: Role,
    room: Room,
    walls: Walls,
    method: ComputationMethod,
    particles: u32,
    duration: f64,
    dt: f64,
    eps: f64,
    radius: f64,
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
        radius,
    }
}

/// Round 3's wall layouts (`Walls::Mixed`).
const fn mixed(floor: f64, ceiling: f64, walls: f64, scattering: f64) -> Walls {
    Walls::Mixed {
        floor,
        ceiling,
        walls,
        scattering,
    }
}

/// The dead floor with Lambert walls: floor 0.6, the rest 0.05, scattering 1.
const LAMBERT_DEAD_FLOOR: Walls = mixed(0.6, 0.05, 0.05, 1.0);
/// The dead ceiling with Lambert walls.
const LAMBERT_DEAD_CEILING: Walls = mixed(0.05, 0.6, 0.05, 1.0);
/// An extreme contrast with Lambert walls: floor 0.9, the rest 0.02.
const LAMBERT_EXTREME_FLOOR: Walls = mixed(0.9, 0.02, 0.02, 1.0);
/// Dead walls: the four walls 0.8, floor and ceiling 0.1, Lambert. The direct sound dominates.
const DEAD_WALLS: Walls = mixed(0.1, 0.1, 0.8, 1.0);
/// Round 4: the four walls 0.3, floor and ceiling 0.05, Lambert.
const LAMBERT_LIVE_FLOORS: Walls = mixed(0.05, 0.05, 0.3, 1.0);
/// Round 4: the ceiling 0.9, floor and walls 0.02, Lambert.
const LAMBERT_EXTREME_CEILING: Walls = mixed(0.02, 0.9, 0.02, 1.0);

#[rustfmt::skip]
/// The cells, pre-registered before any was run (`PREREGISTER.txt` in the investigation's
/// folder; round 3's under "ROUND 3"). Random mode's `trans_epsilon` is SPPS's default 5; it
/// drops nothing there.
const CELLS: [Cell; 94] = [
    cell("C-R1", C, Room::Tutorial, Walls::Lambert(0.1), Random, 150_000, 3.0, 0.01, 5.0),
    cell("C-R2", C, Room::Tutorial, Walls::Lambert(0.1), Random, 1_500_000, 3.0, 0.01, 5.0),
    cell("C-R3", C, Room::Small, Walls::Lambert(0.4), Random, 150_000, 1.0, 0.001, 5.0),
    cell("C-R4", C, Room::Small, Walls::Lambert(0.4), Random, 15_000_000, 1.0, 0.001, 5.0),
    cell("C-R5", C, Room::Tutorial, Walls::Tutorial, Random, 150_000, 2.0, 0.01, 5.0),
    cell("C-R6", C, Room::Tutorial, Walls::Tutorial, Random, 1_500_000, 2.0, 0.001, 5.0),
    cell("C-R7", C, Room::Tutorial, Walls::DeadFloor, Random, 500_000, 3.0, 0.01, 5.0),
    cell("V-R1", V, Room::Long, Walls::Lambert(0.2), Random, 500_000, 2.0, 0.01, 5.0),
    cell("V-R2", V, Room::Tutorial, Walls::Lambert(0.4), Random, 600_000, 1.0, 0.001, 5.0),
    cell("V-R3", V, Room::Small, Walls::Lambert(0.05), Random, 300_000, 3.0, 0.01, 5.0),
    cell("V-R4", V, Room::Tutorial, Walls::Tutorial, Random, 600_000, 2.0, 0.01, 5.0),
    cell("V-R5", V, Room::Small, Walls::Specular(0.2), Random, 5_000_000, 1.5, 0.01, 5.0),
    cell("V-R6", V, Room::Long, Walls::DeadFloor, Random, 1_500_000, 3.0, 0.001, 5.0),
    cell("C-E1", C, Room::Tutorial, Walls::Lambert(0.1), Energetic, 150_000, 2.5, 0.01, 7.0),
    cell("C-E2", C, Room::Tutorial, Walls::Lambert(0.1), Energetic, 600_000, 2.5, 0.01, 7.0),
    cell("C-E3", C, Room::Small, Walls::Lambert(0.4), Energetic, 150_000, 0.5, 0.001, 9.0),
    cell("C-E4", C, Room::Small, Walls::Lambert(0.4), Energetic, 2_400_000, 0.5, 0.001, 9.0),
    cell("C-E5", C, Room::Tutorial, Walls::Tutorial, Energetic, 150_000, 2.0, 0.01, 5.0),
    cell("C-E6", C, Room::Tutorial, Walls::Tutorial, Energetic, 600_000, 2.0, 0.001, 7.0),
    cell("C-E7", C, Room::Tutorial, Walls::DeadFloor, Energetic, 300_000, 3.0, 0.01, 7.0),
    cell("V-E1", V, Room::Long, Walls::Lambert(0.2), Energetic, 300_000, 1.5, 0.01, 7.0),
    cell("V-E2", V, Room::Tutorial, Walls::Lambert(0.4), Energetic, 150_000, 0.6, 0.001, 9.0),
    cell("V-E3", V, Room::Small, Walls::Lambert(0.05), Energetic, 150_000, 3.0, 0.01, 7.0),
    cell("V-E4", V, Room::Tutorial, Walls::Tutorial, Energetic, 1_500_000, 2.0, 0.01, 5.0),
    cell("V-E5", V, Room::Small, Walls::Specular(0.2), Energetic, 600_000, 1.5, 0.001, 7.0),
    cell("V-E6", V, Room::Long, Walls::DeadFloor, Energetic, 300_000, 3.0, 0.01, 7.0),
    // Round 2 (PREREGISTER.txt, "ROUND 2"): energetic T20 and T30 re-calibrated on every energetic
    // cell above; these validate it.
    cell("W-E1", W, Room::Corridor, Walls::DeadFloor, Energetic, 300_000, 3.0, 0.01, 7.0),
    cell("W-E2", W, Room::Long, Walls::DeadCeiling, Energetic, 300_000, 3.0, 0.01, 7.0),
    cell("W-E3", W, Room::Hall, Walls::DeadFloor, Energetic, 300_000, 3.0, 0.01, 7.0),
    cell("W-E4", W, Room::Long, Walls::DeadFloorScattering(0.3), Energetic, 300_000, 3.0, 0.01, 7.0),
    cell("W-E5", W, Room::Cube, Walls::DeadFloor, Energetic, 300_000, 4.0, 0.01, 7.0),
    cell("W-E6", W, Room::Long, Walls::DeadFloor, Energetic, 600_000, 3.0, 0.001, 7.0),
    // Round 3 (PREREGISTER.txt, "ROUND 3"): larger receivers (more crossings per particle), fewer
    // particles, the direct field alone and dead walls, Lambert walls with the absorption on one
    // surface; calibrated on these and on every cell above, validated on the V3- cells.
    cell_r("C3-R1", C3, Room::Small, Walls::Lambert(0.05), Random, 150_000, 3.0, 0.01, 5.0, 0.9),
    cell_r("C3-R2", C3, Room::Tutorial, Walls::Lambert(0.1), Random, 150_000, 3.0, 0.01, 5.0, 0.9),
    cell_r("C3-R3", C3, Room::Tutorial, Walls::DeadFloor, Random, 500_000, 3.0, 0.01, 5.0, 0.9),
    cell_r("C3-R4", C3, Room::Cube, Walls::Lambert(0.05), Random, 150_000, 6.0, 0.01, 5.0, 1.4),
    cell_r("C3-R5", C3, Room::Small, Walls::Lambert(0.2), Random, 5_000, 1.5, 0.01, 5.0, R0),
    cell_r("C3-R6", C3, Room::Tutorial, Walls::Tutorial, Random, 15_000, 2.0, 0.01, 5.0, R0),
    cell_r("C3-R7", C3, Room::Tutorial, Walls::Lambert(1.0), Random, 150_000, 0.05, 0.001, 5.0, R0),
    cell_r("C3-R8", C3, Room::Small, DEAD_WALLS, Random, 150_000, 1.0, 0.01, 5.0, R0),
    cell_r("C3-R9", C3, Room::Small, Walls::Lambert(0.05), Random, 50_000, 3.0, 0.01, 5.0, 0.6),
    cell_r("V3-R1", V3, Room::Long, Walls::Lambert(0.05), Random, 300_000, 4.0, 0.01, 5.0, 0.9),
    cell_r("V3-R2", V3, Room::Small, Walls::Specular(0.05), Random, 300_000, 3.0, 0.01, 5.0, 0.7),
    cell_r("V3-R3", V3, Room::Cube, Walls::Lambert(0.1), Random, 30_000, 3.0, 0.01, 5.0, 1.2),
    cell_r("V3-R4", V3, Room::Tutorial, Walls::Tutorial, Random, 8_000, 2.0, 0.01, 5.0, R0),
    cell_r("V3-R5", V3, Room::Long, Walls::Lambert(1.0), Random, 150_000, 0.08, 0.001, 5.0, 0.5),
    cell_r("V3-R6", V3, Room::Tutorial, DEAD_WALLS, Random, 150_000, 1.0, 0.001, 5.0, R0),
    cell_r("V3-R7", V3, Room::Hall, Walls::DeadFloor, Random, 300_000, 3.0, 0.01, 5.0, 0.9),
    cell_r("V3-R8", V3, Room::Small, Walls::Lambert(0.2), Random, 50_000, 1.5, 0.01, 5.0, R0),
    cell_r("V3-R9", V3, Room::Small, Walls::Lambert(0.05), Random, 500_000, 3.0, 0.01, 5.0, 0.6),
    cell_r("C3-E1", C3, Room::Small, Walls::Lambert(0.05), Energetic, 150_000, 3.0, 0.01, 7.0, 0.9),
    cell_r("C3-E2", C3, Room::Tutorial, LAMBERT_DEAD_FLOOR, Energetic, 300_000, 3.0, 0.01, 7.0, R0),
    cell_r("C3-E3", C3, Room::Long, LAMBERT_DEAD_FLOOR, Energetic, 300_000, 3.0, 0.01, 7.0, R0),
    cell_r("C3-E4", C3, Room::Tutorial, Walls::DeadFloor, Energetic, 300_000, 3.0, 0.01, 7.0, 0.9),
    cell_r("C3-E5", C3, Room::Small, Walls::Lambert(0.2), Energetic, 5_000, 1.5, 0.01, 7.0, R0),
    cell_r("C3-E6", C3, Room::Tutorial, Walls::Tutorial, Energetic, 15_000, 2.0, 0.01, 5.0, R0),
    cell_r("C3-E7", C3, Room::Tutorial, Walls::Lambert(1.0), Energetic, 150_000, 0.05, 0.001, 7.0, R0),
    cell_r("C3-E8", C3, Room::Hall, LAMBERT_EXTREME_FLOOR, Energetic, 300_000, 3.0, 0.01, 7.0, R0),
    cell_r("C3-E9", C3, Room::Small, DEAD_WALLS, Energetic, 150_000, 1.0, 0.01, 7.0, R0),
    cell_r("C3-E10", C3, Room::Small, Walls::Lambert(0.05), Energetic, 50_000, 3.0, 0.01, 7.0, 0.6),
    cell_r("V3-E1", V3, Room::Long, Walls::Lambert(0.05), Energetic, 300_000, 3.0, 0.01, 7.0, 0.9),
    cell_r("V3-E2", V3, Room::Corridor, LAMBERT_DEAD_CEILING, Energetic, 300_000, 3.0, 0.01, 7.0, R0),
    cell_r("V3-E3", V3, Room::Cube, LAMBERT_DEAD_FLOOR, Energetic, 150_000, 3.0, 0.01, 7.0, 1.2),
    cell_r("V3-E4", V3, Room::Small, Walls::Specular(0.05), Energetic, 150_000, 3.0, 0.01, 7.0, 0.7),
    cell_r("V3-E5", V3, Room::Small, Walls::Lambert(0.2), Energetic, 50_000, 1.5, 0.01, 7.0, R0),
    cell_r("V3-E6", V3, Room::Long, Walls::Lambert(1.0), Energetic, 150_000, 0.08, 0.001, 7.0, 0.5),
    cell_r("V3-E7", V3, Room::Tutorial, DEAD_WALLS, Energetic, 150_000, 1.0, 0.01, 7.0, R0),
    cell_r("V3-E8", V3, Room::Tutorial, LAMBERT_EXTREME_FLOOR, Energetic, 300_000, 2.0, 0.001, 7.0, R0),
    cell_r("V3-E9", V3, Room::Small, Walls::Lambert(0.05), Energetic, 300_000, 3.0, 0.01, 7.0, 0.6),
    cell_r("V3-E10", V3, Room::Long, Walls::DeadFloorScattering(0.3), Energetic, 300_000, 3.0, 0.01, 7.0, 0.6),
    // Round 4 (PREREGISTER.txt, "ROUND 4"): every cell held out. Uniform Lambert rooms at a mean
    // absorption of 0.3 to 0.4 in rooms other than 5 x 4 x 3 and 6 x 10 x 3 m; rooms whose walls
    // are not uniform Lambert, for the roughness structure; and the higher counts of new pairs.
    cell_r("V4-E1", V4, Room::Long, Walls::Lambert(0.4), Energetic, 300_000, 0.6, 0.001, 9.0, R0),
    cell_r("V4-E2", V4, Room::Cube, Walls::Lambert(0.3), Energetic, 150_000, 1.5, 0.01, 9.0, R0),
    cell_r("V4-E3", V4, Room::Hall, Walls::Lambert(0.35), Energetic, 300_000, 1.0, 0.01, 9.0, R0),
    cell_r("V4-E4", V4, Room::Corridor, Walls::Lambert(0.4), Energetic, 150_000, 0.8, 0.01, 9.0, R0),
    cell_r("V4-E5", V4, Room::Small, Walls::Specular(0.1), Energetic, 300_000, 2.0, 0.01, 7.0, R0),
    cell_r("V4-E6", V4, Room::Hall, Walls::DeadCeiling, Energetic, 300_000, 3.0, 0.01, 7.0, R0),
    cell_r("V4-E7", V4, Room::Cube, LAMBERT_LIVE_FLOORS, Energetic, 150_000, 3.0, 0.01, 7.0, R0),
    cell_r("V4-E8", V4, Room::Corridor, Walls::DeadFloorScattering(0.5), Energetic, 300_000, 3.0, 0.01, 7.0, R0),
    cell_r("V4-E9", V4, Room::Tutorial, Walls::DeadFloor, Energetic, 600_000, 3.0, 0.001, 7.0, R0),
    cell_r("V4-E10", V4, Room::Hall, Walls::Tutorial, Energetic, 300_000, 3.0, 0.01, 7.0, R0),
    cell_r("V4-E11", V4, Room::Small, LAMBERT_EXTREME_CEILING, Energetic, 300_000, 2.0, 0.001, 7.0, R0),
    cell_r("V4-E12", V4, Room::Tutorial, Walls::Specular(0.1), Energetic, 300_000, 3.0, 0.01, 7.0, 0.6),
    cell_r("V4-E13", V4, Room::Tutorial, Walls::Tutorial, Energetic, 150_000, 2.0, 0.01, 7.0, R0),
    cell_r("V4-E14", V4, Room::Tutorial, Walls::Tutorial, Energetic, 1_200_000, 2.0, 0.01, 7.0, R0),
    cell_r("V4-E15", V4, Room::Hall, Walls::DeadFloor, Energetic, 2_400_000, 3.0, 0.01, 7.0, R0),
    cell_r("V4-E16", V4, Room::Tutorial, Walls::Lambert(0.4), Energetic, 2_400_000, 0.6, 0.001, 9.0, R0),
    cell_r("V4-E17", V4, Room::Small, Walls::Lambert(0.2), Energetic, 800_000, 1.5, 0.01, 7.0, R0),
    cell_r("V4-E18", V4, Room::Long, Walls::Lambert(1.0), Energetic, 1_200_000, 0.08, 0.001, 7.0, 0.5),
    cell_r("V4-R1", V4, Room::Tutorial, Walls::Lambert(0.1), Random, 6_000_000, 3.0, 0.01, 5.0, R0),
    cell_r("V4-R2", V4, Room::Tutorial, Walls::Tutorial, Random, 6_000_000, 2.0, 0.01, 5.0, R0),
    cell_r("V4-R3", V4, Room::Small, Walls::Lambert(0.2), Random, 800_000, 1.5, 0.01, 5.0, R0),
    cell_r("V4-R4", V4, Room::Small, Walls::Lambert(0.05), Random, 2_000_000, 3.0, 0.01, 5.0, 0.6),
    cell_r("V4-R5", V4, Room::Tutorial, Walls::DeadFloor, Random, 4_000_000, 3.0, 0.01, 5.0, R0),
    cell_r("V4-R6", V4, Room::Long, Walls::Lambert(1.0), Random, 1_200_000, 0.08, 0.001, 5.0, 0.5),
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
    // The rest's material, and the one surface group with its own.
    let set = match c.walls {
        Walls::Tutorial => None,
        Walls::Lambert(a) => Some((material(1, "uniform", a, true), None)),
        Walls::Specular(a) => Some((material(1, "uniform", a, false), None)),
        Walls::DeadFloor => Some((
            material(1, "rest", 0.05, false),
            Some(("Floor", material(2, "floor", 0.6, false))),
        )),
        Walls::DeadCeiling => Some((
            material(1, "rest", 0.05, false),
            Some(("Ceiling", material(2, "ceiling", 0.6, false))),
        )),
        Walls::DeadFloorScattering(sc) => {
            let scatter = |mut m: schema::Material| {
                m.scattering = vec![schema::F64::new(sc); n];
                m
            };
            Some((
                scatter(material(1, "rest", 0.05, true)),
                Some(("Floor", scatter(material(2, "floor", 0.6, true)))),
            ))
        }
        Walls::Mixed { .. } => None,
    };
    // One material per surface group: tutorial 1's box has exactly Floor, Ceiling and Walls.
    let mixed = match c.walls {
        Walls::Mixed {
            floor,
            ceiling,
            walls,
            scattering,
        } => {
            let with = |id: u128, name: &str, alpha: f64| {
                let mut m = material(id, name, alpha, scattering > 0.0);
                m.scattering = vec![schema::F64::new(scattering); n];
                m
            };
            Some([
                ("Floor", with(1, "floor", floor)),
                ("Ceiling", with(2, "ceiling", ceiling)),
                ("Walls", with(3, "walls", walls)),
            ])
        }
        _ => None,
    };
    if let Some((rest, dead)) = set {
        for g in &mut p.surface_groups {
            g.material = match &dead {
                Some((group, m)) if g.name == *group => m.id,
                _ => rest.id,
            };
        }
        p.materials = std::iter::once(rest).chain(dead.map(|(_, m)| m)).collect();
        p.solvers.spps.air_absorption = false;
        p.solvers.tcr.air_absorption = false;
    }
    if let Some(mats) = mixed {
        assert_eq!(
            p.surface_groups.len(),
            3,
            "tutorial 1's box has three groups"
        );
        for g in &mut p.surface_groups {
            g.material = mats
                .iter()
                .find(|(name, _)| g.name == *name)
                .unwrap_or_else(|| panic!("surface group {:?}", g.name))
                .1
                .id;
        }
        p.materials = mats.into_iter().map(|(_, m)| m).collect();
        p.solvers.spps.air_absorption = false;
        p.solvers.tcr.air_absorption = false;
    }
    let s = c.room.source();
    p.sources[0].position = Vec3::new(s[0], s[1], s[2]);
    with_receivers(&mut p, &c.room.receivers());
    c.check_geometry();
    let spps = &mut p.solvers.spps;
    spps.method = c.method;
    spps.particles_per_source = c.particles;
    spps.duration_s = schema::F64::new(c.duration);
    spps.time_step_s = schema::F64::new(c.dt);
    spps.extinction_exponent = schema::F64::new(c.eps);
    spps.receiver_radius_m = schema::F64::new(c.radius);
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
#[ignore = "evidence for the noise model, not a gate: runs SPPS on the cells of CELLS (70) over ten \
            seeds; run on purpose"]
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

/// One receiver-band of one run, as the analysis needs it. Everything the noise model's
/// calibration and domain read is taken from the report, as `simpa results` computed it (round 4,
/// review of `50695f6`: the harness had taken the uniform flag from the cell and recomputed `n`
/// with the code it was checking), and checked against the harness's own computation and the
/// cell's configuration.
struct Band {
    dt: f64,
    energy: Vec<f64>,
    arrival: Arrival,
    mean_deposit: f64,
    /// The share of the emitted energy alive at the end of each step (the room table over the
    /// sources' power, both times `ρc`).
    alive: Vec<f64>,
    /// Crossings per particle: the report's `crossings_per_particle` over its lifetime spread.
    n1: f64,
    /// The spread of the particles' lifetimes: the report's `noise_model.run.lifetime_cv2`.
    cv2: f64,
    /// Every face reflects by Lambert's law with scattering 1 in the band (the report's
    /// `reference`).
    lambert: bool,
    /// Every face has the same absorption in the band (the report's `reference`).
    uniform: bool,
    /// The band's mean absorption over the faces (the report's `reference`).
    mean_absorption: f64,
    /// The band's noise model, rebuilt from the report's `noise_model`.
    model: NoiseModel,
    /// How the report judged each of the eight quantities.
    judged: [Option<Judged>; 8],
}

/// `Var L / (E L)²` of the lifetimes, from the share alive at the end of each step, written again
/// here (not `params::noise::lifetime_cv2`): `E L = ∫S`, `E L² = ∫2t·S`, by trapezoids from
/// `(0, 1)`.
fn own_lifetime_cv2(alive: &[f64], dt: f64) -> f64 {
    let mut t_prev = 0.0;
    let mut s_prev = 1.0;
    let (mut m1, mut m2) = (0.0, 0.0);
    for (k, &s) in alive.iter().enumerate() {
        let t = dt * (k + 1) as f64;
        m1 += 0.5 * dt * (s_prev + s);
        m2 += 0.5 * dt * (2.0 * t_prev * s_prev + 2.0 * t * s);
        t_prev = t;
        s_prev = s;
    }
    (m2 / (m1 * m1) - 1.0).max(0.0)
}

/// How the report judged a quantity: `None` when it gives no value from the series for a reason
/// that is not its noise (the harness then counts it as the series' refusal).
fn judged_of(p: &Value) -> Option<Judged> {
    if p["value"].is_f64() {
        return Some(Judged::Given);
    }
    let why = &p["not_evaluable"]["error"]["why"];
    match why["why"].as_str()? {
        "monte_carlo_noise" => {
            let c = &why["particle_count"];
            Some(match (c["count"].as_str(), c["particles"].as_u64()) {
                (Some("named"), Some(n)) => Judged::Named(n),
                (Some("resampled"), Some(n)) => Judged::Resampled(n),
                (Some("beyond_resampled"), _) => Judged::BeyondResampled,
                _ => Judged::NoCount,
            })
        }
        "noise_uncalibrated" => Some(match why["particles_at_least"].as_u64() {
            Some(n) => Judged::FewParticles(n),
            None => Judged::ManyCrossings,
        }),
        _ => None,
    }
}

/// Every receiver-band of a report, receivers then bands. `cell`: the cell whose run it is, whose
/// configuration the report's reading of the walls must match.
fn bands_of(rep: &Value, cell: &Cell) -> Vec<Band> {
    let s = &rep["spps"];
    let dt = s["time_step_s"].as_f64().unwrap();
    let crossing = s["receiver_crossing_s"].as_f64().unwrap();
    let half = crossing / 2.0;
    let particles = s["particles_per_source"].as_f64().unwrap();
    assert_eq!(
        s["sources"].as_array().unwrap().len(),
        1,
        "one source a cell"
    );
    let method = if s["computation_method"] == 0 {
        noise::Method::Random
    } else {
        noise::Method::Energetic
    };
    let floats = |v: &Value| -> Vec<f64> {
        v.as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_f64().unwrap())
            .collect()
    };
    let reference = &s["reference"];
    let mut out = Vec::new();
    for r in s["point_receivers"].as_array().unwrap() {
        let t_a = r["arrival_s"].as_f64().unwrap();
        for (bi, b) in r["bands"].as_array().unwrap().iter().enumerate() {
            let power = b["source_power_rho_c"].as_f64().unwrap();
            let room = floats(&s["total_energy"][bi]["energy"]);
            let energy = floats(&b["energy_pa2"]);
            let nm = &b["noise_model"];
            let mean_deposit = nm["mean_deposit"].as_f64().unwrap();
            let alive: Vec<f64> = room.iter().map(|e| e / power).collect();
            assert_eq!(
                reference["status"], "computed",
                "the cells' references are computed"
            );
            let rb = &reference["bands"][bi];
            assert_eq!(rb["freq_hz"], b["freq_hz"]);
            let lambert = rb["lambert_walls"] == true;
            let uniform = rb["uniform_absorption"] == true;
            let mean_absorption = rb["mean_absorption"].as_f64().unwrap();
            // The report's reading of the walls against the cell's configuration.
            assert_eq!(lambert, cell.walls.lambert(), "{}: lambert_walls", cell.id);
            assert_eq!(
                uniform,
                cell.walls.uniform(),
                "{}: uniform_absorption",
                cell.id
            );
            // What the calibration's correction and domain read, as the report computed them,
            // against the harness's own computation.
            let run = &nm["run"];
            let cv2 = run["lifetime_cv2"].as_f64().unwrap();
            let own_cv2 = own_lifetime_cv2(&alive, dt);
            assert!(
                (cv2 - own_cv2).abs() <= 1e-9 * own_cv2.max(1.0),
                "{}: lifetime_cv2 {cv2} against {own_cv2}",
                cell.id
            );
            let least = run["least_deposit"].as_f64().unwrap();
            assert_eq!(least, mean_deposit, "{}: one source", cell.id);
            assert_eq!(run["particles"].as_f64(), Some(particles), "{}", cell.id);
            let n = b["crossings_per_particle"].as_f64().unwrap();
            let own_n1 = energy.iter().sum::<f64>() / (least * particles);
            assert!(
                (n - own_n1 * own_cv2).abs() <= 1e-9 * (own_n1 * own_cv2).max(1e-6),
                "{}: crossings_per_particle {n} against {}",
                cell.id,
                own_n1 * own_cv2
            );
            assert_eq!(run["lambert_walls"].as_bool(), Some(lambert), "{}", cell.id);
            assert_eq!(
                run["uniform_absorption"].as_bool(),
                Some(uniform),
                "{}",
                cell.id
            );
            let model = NoiseModel::of_run(
                mean_deposit,
                method,
                noise::RunNoise {
                    particles: particles as u32,
                    least_deposit: least,
                    lifetime_cv2: cv2,
                    lambert_walls: lambert,
                    uniform_absorption: uniform,
                    mean_absorption: run["mean_absorption"].as_f64().unwrap(),
                    bands: 1,
                    receiver_crossing_s: crossing,
                },
            )
            .unwrap();
            let judged =
                std::array::from_fn(|i| judged_of(&b["parameters"][noise::QUANTITY_NAMES[i]]));
            out.push(Band {
                dt,
                n1: own_n1,
                cv2,
                lambert,
                uniform,
                mean_absorption,
                energy,
                arrival: Arrival::spread(t_a, half),
                mean_deposit,
                alive,
                model,
                judged,
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

/// The candidate structures of the model (rule 1 and round 4).
#[derive(Clone, Copy, Debug, PartialEq)]
enum Structure {
    /// M7's: every crossing deposits `d̄` on average; `params::noise` as the code has it.
    Constant,
    /// A crossing in step `k` deposits `d̄` times the particles' mean energy then over their start
    /// energy, read from the room table at the start of the step (1 in step 0): the candidate the
    /// calibration measured and rejected for energetic mode. Not in the code; computed here.
    MeanEnergy,
    /// Round 4's: each bin's deposit from the series' own roughness, at most `d̄`
    /// (`params::noise::Structure::Roughness`), energetic mode only.
    Roughness,
}

const STRUCTURES: [Structure; 3] = [
    Structure::Constant,
    Structure::MeanEnergy,
    Structure::Roughness,
];

/// Each quantity's model standard deviation before calibration under structure `st`, relative to
/// the series' value for the decay times; `None` where fewer than two resamples or the series
/// give a value. The constant and roughness structures are the code's (`params::noise::
/// bootstrap_with`, on the band's model); the mean-energy candidate is computed here.
fn bootstrap(
    s: &EnergySeries,
    b: &Band,
    st: Structure,
    refused: &mut [usize; 8],
) -> [Option<f64>; 8] {
    let base = values(s, b.arrival);
    let sd: [Option<f64>; 8] = match st {
        Structure::Constant | Structure::Roughness => {
            let code = if st == Structure::Constant {
                noise::Structure::Constant
            } else {
                noise::Structure::Roughness
            };
            let raw = noise::bootstrap_with(s, b.arrival, &b.model, code, 1);
            if st == Structure::Constant {
                *refused = raw.map(|(_, r)| r);
            }
            raw.map(|(sd, _)| sd)
        }
        Structure::MeanEnergy => {
            let dep: Vec<f64> = (0..b.energy.len())
                .map(|k| {
                    let f = if k == 0 {
                        1.0
                    } else {
                        b.alive.get(k - 1).copied().unwrap_or(1.0)
                    };
                    b.mean_deposit * f.clamp(f64::MIN_POSITIVE, 1.0)
                })
                .collect();
            let mut rng = noise::Rng::new(noise::SEED);
            let mut got: Vec<Vec<f64>> = vec![Vec::new(); 8];
            for _ in 0..noise::RESAMPLES {
                let drawn = noise::resample_with(s.values(), &dep, &mut rng);
                if let Some(r) = plain_series(s.dt(), drawn) {
                    for (i, v) in values(&r, b.arrival).into_iter().enumerate() {
                        if let Some(v) = v {
                            got[i].push(v);
                        }
                    }
                }
            }
            std::array::from_fn(|i| noise::standard_deviation(&got[i]))
        }
    };
    std::array::from_fn(|i| {
        let (v, sd) = (base[i]?, sd[i]?);
        Some(if noise::relative(i) { sd / v.abs() } else { sd })
    })
}

/// One run's receiver-bands: each quantity's value, its standard deviation under each
/// structure, how many of the constant structure's resamples refused it, the band's crossings per
/// particle, lifetime spread, Lambert and uniform flags and mean absorption (as the report read
/// them), and how the report judged each quantity.
struct RunNumbers {
    values: Vec<[Option<f64>; 8]>,
    sd: Vec<Vec<[Option<f64>; 8]>>,
    n1: Vec<f64>,
    cv2: Vec<f64>,
    lambert: Vec<bool>,
    uniform: Vec<bool>,
    mean_absorption: Vec<f64>,
    judged: Vec<[Option<Judged>; 8]>,
}

fn numbers(rep: &Value, cell: &Cell, structures: &[Structure]) -> RunNumbers {
    let bands = bands_of(rep, cell);
    let method = if rep["spps"]["computation_method"] == 0 {
        noise::Method::Random
    } else {
        noise::Method::Energetic
    };
    let mut values_out = Vec::new();
    let mut sd = vec![Vec::new(); structures.len()];
    for b in &bands {
        let s = plain_series(b.dt, b.energy.clone());
        values_out.push(s.as_ref().map_or([None; 8], |s| values(s, b.arrival)));
        let mut refused = [noise::RESAMPLES; 8];
        for (si, st) in structures.iter().enumerate() {
            // The mean-energy and roughness structures are energetic mode's only: in random mode a
            // particle keeps its start energy.
            let skip = *st != Structure::Constant && method == noise::Method::Random;
            sd[si].push(match (&s, skip) {
                (Some(s), false) => bootstrap(s, b, *st, &mut refused),
                _ => [None; 8],
            });
        }
    }
    RunNumbers {
        values: values_out,
        sd,
        n1: bands.iter().map(|b| b.n1).collect(),
        cv2: bands.iter().map(|b| b.cv2).collect(),
        lambert: bands.iter().map(|b| b.lambert).collect(),
        uniform: bands.iter().map(|b| b.uniform).collect(),
        mean_absorption: bands.iter().map(|b| b.mean_absorption).collect(),
        judged: bands.iter().map(|b| b.judged).collect(),
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
            "validation2" => Role::Validation2,
            "calibration3" => Role::Calibration3,
            "validation3" => Role::Validation3,
            "validation4" => Role::Validation4,
            other => panic!("role {other:?}"),
        })
        .collect()
}

/// The structure's name in the receipt.
fn structure_name(st: Structure) -> &'static str {
    match st {
        Structure::Constant => "constant",
        Structure::MeanEnergy => "mean_energy",
        Structure::Roughness => "roughness",
    }
}

/// With `$SIMPA_NOISE_REREAD` set to a folder, every run of `cells` found under `from` is read
/// again with this build's `simpa results`, `jobs()` at a time, into
/// `<folder>/<cell>/seed<NN>/report.json` (an existing report there is kept), and the analysis reads
/// those: the reports then carry what this build computes (round 4). Returns the folders to read
/// the reports from, in the order of `from`.
fn reread(cells: &[Cell], from: &[PathBuf]) -> Vec<PathBuf> {
    let Some(out) = std::env::var_os("SIMPA_NOISE_REREAD").filter(|s| !s.is_empty()) else {
        return from.to_vec();
    };
    let out = PathBuf::from(out);
    let mut todo: Vec<(PathBuf, PathBuf)> = Vec::new();
    for c in cells {
        let Some(dir) = from.iter().find(|f| f.join(c.id).is_dir()) else {
            panic!("{}: in none of {from:?}", c.id);
        };
        for seed in SEEDS {
            let folder = format!("seed{seed:02}");
            let to = out.join(c.id).join(&folder).join("report.json");
            if to.exists() {
                continue;
            }
            let runs = dir.join(c.id).join(&folder).join("runs");
            let run = std::fs::read_dir(&runs)
                .unwrap_or_else(|e| panic!("{}: {e}", runs.display()))
                .next()
                .unwrap()
                .unwrap()
                .path();
            todo.push((run, to));
        }
    }
    let next = AtomicUsize::new(0);
    let clock = std::time::Instant::now();
    std::thread::scope(|scope| {
        for _ in 0..jobs().min(todo.len()) {
            scope.spawn(|| {
                loop {
                    let i = next.fetch_add(1, Ordering::SeqCst);
                    let Some((run, to)) = todo.get(i) else {
                        break;
                    };
                    let r = simpa_run(&[
                        "results".to_string(),
                        run.display().to_string(),
                        "--json".into(),
                    ]);
                    assert!(r.code == 0 || r.code == 6, "{}: {r:#?}", run.display());
                    std::fs::create_dir_all(to.parent().unwrap()).unwrap();
                    std::fs::write(to, &r.stdout).unwrap();
                    if i % 50 == 0 {
                        println!(
                            "[{:>5.0} s] read again {} of {}",
                            clock.elapsed().as_secs_f64(),
                            i + 1,
                            todo.len()
                        );
                    }
                }
            });
        }
    });
    vec![out]
}

/// One cell's receipt: its configuration and, per quantity, a row per receiver-band where every
/// seed gives a value (`calibration::rows` reads it).
fn cell_receipt(c: &Cell, from: &Path, runs_dir: &Path) -> (Value, Vec<RunNumbers>) {
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
            .map(|r| scope.spawn(|| numbers(r, c, &STRUCTURES)))
            .collect();
        hs.into_iter().map(|h| h.join().unwrap()).collect()
    });
    let n = runs.len();
    let rbs = runs[0].values.len();
    println!(
        "\nCELL {} ({n} seeds, {rbs} receiver-bands; read in {:.0} s)",
        c.label(),
        t0.elapsed().as_secs_f64()
    );
    let first = &reports[0]["spps"]["point_receivers"];
    let per = rbs / first.as_array().unwrap().len();
    let mut quantities = serde_json::Map::new();
    let mut incomplete = serde_json::Map::new();
    for (qi, q) in calibration::QUANTITIES.iter().enumerate() {
        let mut rows = Vec::new();
        let mut partial = Vec::new();
        for rb in 0..rbs {
            let got: Vec<f64> = runs.iter().filter_map(|r| r.values[rb][qi]).collect();
            let sds: Vec<Vec<f64>> = (0..STRUCTURES.len())
                .map(|si| runs.iter().filter_map(|r| r.sd[si][rb][qi]).collect())
                .collect();
            // Every seed gives the value and the code's structure its standard deviation; the
            // other candidate is kept where it gives one in every seed too. The others are kept
            // apart, each seed's value or null, so that every run's result is in the receipt.
            if got.len() < n || sds[0].len() < n {
                partial.push(json!({
                    "receiver": rb / per,
                    "freq_hz": first[rb / per]["bands"][rb % per]["freq_hz"],
                    "seed_values": runs.iter().map(|r| r.values[rb][qi]).collect::<Vec<_>>(),
                    "seed_model_sd": runs.iter().map(|r| r.sd[0][rb][qi]).collect::<Vec<_>>(),
                }));
                continue;
            }
            let (m, sd) = mean_sd(&got);
            let observed = if noise::relative(qi) {
                sd / m.abs()
            } else {
                sd
            };
            // How far one seed's model standard deviation strays from the others' (relative
            // standard deviation over the seeds): what a single run's named count rests on.
            let scatter = {
                let (pm, psd) = mean_sd(&sds[0]);
                psd / pm
            };
            // Round 4's roughness structure (energetic mode): each seed's standard deviation, or
            // null where its resamples gave none, and its scatter over the seeds.
            let rough_si = STRUCTURES
                .iter()
                .position(|s| *s == Structure::Roughness)
                .unwrap();
            let rough: Vec<Option<f64>> = runs.iter().map(|r| r.sd[rough_si][rb][qi]).collect();
            let rough_scatter = (sds[rough_si].len() == n).then(|| {
                let (pm, psd) = mean_sd(&sds[rough_si]);
                psd / pm
            });
            rows.push(json!({
                "receiver": rb / per,
                "freq_hz": first[rb / per]["bands"][rb % per]["freq_hz"],
                "mean": m,
                "observed_sd": observed,
                "predicted_scatter": scatter,
                // Each seed's value, and the model's standard deviation for its series before
                // calibration (relative for the decay times): every run's result, seed 1 first.
                "seed_values": got,
                "seed_model_sd": sds[0],
                "seed_model_sd_roughness": rough,
                "predicted_scatter_roughness": rough_scatter,
                "predicted_sd": STRUCTURES.iter().zip(&sds)
                    .filter(|(_, s)| s.len() == n)
                    .map(|(st, s)| (
                        structure_name(*st).to_string(),
                        json!((s.iter().map(|x| x * x).sum::<f64>() / n as f64).sqrt()),
                    ))
                    .collect::<serde_json::Map<_, _>>(),
            }));
        }
        quantities.insert(q.to_string(), Value::Array(rows));
        incomplete.insert(q.to_string(), Value::Array(partial));
    }
    let mut config = c.config();
    // Each seed's wall time, s, from the runs' `wall.json` when it is there.
    let wall = std::fs::read_to_string(runs_dir.join("wall.json"))
        .ok()
        .and_then(|t| serde_json::from_str::<Value>(&t).ok())
        .and_then(|w| {
            w.as_array()?
                .iter()
                .find(|x| x["id"] == c.id)
                .map(|x| x["wall_s"].clone())
        });
    config["wall_s"] = wall.unwrap_or(Value::Null);
    // Round 3, once per receiver-band: each seed's crossings per particle and lifetime spread,
    // whether every face is Lambert with scattering 1 in the band, whether every face has the
    // same absorption (A2), and the mean absorption; since round 4 all as the report read them,
    // checked against the cell's configuration (`bands_of`).
    let bands: Vec<Value> = (0..rbs)
        .map(|rb| {
            json!({
                "receiver": rb / per,
                "freq_hz": first[rb / per]["bands"][rb % per]["freq_hz"],
                "seed_n1": runs.iter().map(|r| r.n1[rb]).collect::<Vec<_>>(),
                "seed_cv2": runs.iter().map(|r| r.cv2[rb]).collect::<Vec<_>>(),
                "lambert": runs[0].lambert[rb],
                "uniform": runs[0].uniform[rb],
                "mean_absorption": runs[0].mean_absorption[rb],
            })
        })
        .collect();
    let receipt = json!({
        "cell": config,
        "bands": bands,
        "quantities": quantities,
        "incomplete": incomplete,
    });
    for q in calibration::QUANTITIES {
        let mut sc: Vec<f64> = receipt["quantities"][q]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| r["predicted_scatter"].as_f64().unwrap())
            .collect();
        if sc.is_empty() {
            continue;
        }
        sc.sort_by(f64::total_cmp);
        println!(
            "  {q:<7} one seed's model sd over the seeds: median scatter {:.3}, largest {:.3}",
            sc[sc.len() / 2],
            sc[sc.len() - 1]
        );
    }
    for q in calibration::QUANTITIES {
        let mut line = format!(
            "  {q:<7} {:>2} of {rbs}:",
            receipt["quantities"][q].as_array().unwrap().len()
        );
        for st in STRUCTURES {
            if let Some(p) =
                calibration::pooled(&calibration::rows(&receipt, q, structure_name(st)), 1.0, n)
            {
                line += &format!(
                    "  {} {:.3} [{:.3}, {:.3}]",
                    structure_name(st),
                    p.ratio,
                    p.lower,
                    p.upper
                );
            }
        }
        println!("{line}");
    }
    (receipt, runs)
}

/// How the report judged a value (R3-9, R4-3 and the per-cell counts): `simpa results` of this
/// build on the run (`$SIMPA_NOISE_REREAD`).
#[derive(Clone, Copy, Debug, PartialEq)]
enum Judged {
    /// Given.
    Given,
    /// Refused for its noise, naming this many particles per source from its standard deviation.
    Named(u64),
    /// Refused because too many of its resamples refuse it, naming this many particles per source,
    /// where they would not (R4-3).
    Resampled(u64),
    /// Refused because too many of its resamples refuse it, and they still would at the largest
    /// multiple tried.
    BeyondResampled,
    /// Refused for its noise with no count named for another reason.
    NoCount,
    /// Outside the calibration's domain: too few particles (run at least this many) ...
    FewParticles(u64),
    /// ... or too many crossings per particle.
    ManyCrossings,
}

/// How the report judged quantity `qi` of receiver-band `rb` of one run; `None` when it refused
/// the value for a reason other than its noise.
fn judged(run: &RunNumbers, rb: usize, qi: usize) -> Option<Judged> {
    run.judged[rb][qi]
}

#[test]
#[ignore = "evidence for the noise model, not a gate: reads the calibration runs; run on purpose \
            with SIMPA_NOISE_FROM"]
fn noise_calibration() {
    // `$SIMPA_NOISE_FROM`: the folders `noise_calibration_runs` wrote, `;`-separated; each cell is
    // read from the first that holds it.
    let from: Vec<PathBuf> = std::env::var("SIMPA_NOISE_FROM")
        .expect("SIMPA_NOISE_FROM")
        .split(';')
        .filter(|s| !s.trim().is_empty())
        .map(|s| PathBuf::from(s.trim()))
        .collect();
    let roles = roles();
    let cells: Vec<Cell> = chosen_cells()
        .into_iter()
        .filter(|c| roles.contains(&c.role))
        .collect();
    let read_from = reread(&cells, &from);
    let (receipts, runs): (Vec<Value>, Vec<Vec<RunNumbers>>) = cells
        .iter()
        .map(|c| {
            let dir = from
                .iter()
                .find(|f| f.join(c.id).is_dir())
                .unwrap_or_else(|| panic!("{}: in none of {from:?}", c.id));
            let reports = read_from
                .iter()
                .find(|f| f.join(c.id).is_dir())
                .unwrap_or_else(|| panic!("{}: in none of {read_from:?}", c.id));
            cell_receipt(c, reports, dir)
        })
        .unzip();
    let all = json!({ "cells": receipts });
    let method_of = |name: &str| {
        if name == "random" {
            noise::Method::Random
        } else {
            noise::Method::Energetic
        }
    };
    // Rules 1, 2 and 5b on each quantity's calibration cells (round 1's, or for energetic T20 and
    // T30 round 2's): the factor each structure would get, how much it would overstate the seeds'
    // spread cell by cell, and the margin.
    for method in ["random", "energetic"] {
        println!("\n{method} mode:");
        for st in STRUCTURES {
            let name = structure_name(st);
            let mut logs = Vec::new();
            let mut line = format!("  {name}");
            for q in calibration::QUANTITIES {
                let cal =
                    calibration::cells_in(&all, method, calibration::calibration_roles(method, q));
                let mut best: Option<(f64, &str)> = None;
                let mut ratios = Vec::new();
                for c in &cal {
                    let Some(p) = calibration::pooled(
                        &calibration::rows(c, q, name),
                        1.0,
                        calibration::seeds(c),
                    ) else {
                        continue;
                    };
                    ratios.push(p.ratio);
                    if best.is_none_or(|(b, _)| p.upper > b) {
                        best = Some((p.upper, c["cell"]["id"].as_str().unwrap()));
                    }
                }
                let Some((upper, id)) = best else {
                    continue;
                };
                let k = calibration::round_up_two_digits(upper);
                logs.extend(ratios.iter().map(|r| (k / r).ln()));
                let margin = calibration::margin(&cal, q)
                    .map_or("-".to_string(), |(m, by)| format!("{m} (by {by})"));
                line += &format!(
                    "\n    {q:<7} on {} cells: k {k} (upper bound {upper:.6}, set by {id}); \
                     overstates {:.2} to {:.2}; margin {margin}",
                    cal.len(),
                    k / ratios.iter().copied().fold(0.0, f64::max),
                    k / ratios.iter().copied().fold(f64::INFINITY, f64::min)
                );
            }
            if logs.is_empty() {
                continue;
            }
            let gm = (logs.iter().sum::<f64>() / logs.len() as f64).exp();
            println!("{line}\n    geometric-mean overstatement {gm:.3}");
        }
    }
    // Rule 3 on each quantity's validation cells, with rounds 1 and 2's factors as their rules
    // give them (history: round 3's numbers replace them in the code).
    for method in ["random", "energetic"] {
        println!("\n{method} mode, validation with rounds 1 and 2's factors:");
        for c in calibration::cells_in(&all, method, &["validation", "validation2"]) {
            let mut line = format!("  {}", c["cell"]["id"].as_str().unwrap());
            for q in calibration::QUANTITIES {
                let role = c["cell"]["role"].as_str().unwrap();
                if !calibration::validation_roles(method, q).contains(&role) {
                    line += &format!("\n    {q:<7} (calibrates it)");
                    continue;
                }
                let k = calibration::shipped_factor(&all, method, q).0;
                match calibration::validates(c, q, k) {
                    Some((ok, p)) => {
                        line += &format!(
                            "\n    {q:<7} {:.3} [{:.3}, {:.3}] {}",
                            p.ratio,
                            p.lower,
                            p.upper,
                            if ok { "ok" } else { "FAIL" }
                        )
                    }
                    None => line += &format!("\n    {q:<7} -"),
                }
            }
            println!("{line}");
        }
    }
    // Rule 4: cells that differ only in N.
    for (a, b) in calibration::PAIRS {
        let find = |id: &str| receipts.iter().find(|r| r["cell"]["id"] == id);
        let (Some(ra), Some(rb)) = (find(a), find(b)) else {
            continue;
        };
        println!("\n1/sqrt(N), {a} against {b}:");
        for q in calibration::QUANTITIES {
            if let Some((x, y, ok)) = calibration::root_n(ra, rb, q) {
                println!(
                    "    {q:<7} {:.4} vs {:.4} ({:+.1} se) {}",
                    x.0,
                    y.0,
                    (y.0 - x.0) / x.1.hypot(y.1),
                    if ok { "ok" } else { "DIFFER" }
                );
            }
        }
    }
    for method in ["random", "energetic"] {
        let line: Vec<String> = calibration::QUANTITIES
            .iter()
            .map(|q| {
                let (ok, differ) = calibration::root_n_confirmed(&all, method, q);
                format!(
                    "{q} {}",
                    if ok {
                        "confirmed".into()
                    } else {
                        format!("NOT {differ:?}")
                    }
                )
            })
            .collect();
        println!(
            "{method} mode, 1/sqrt(N) over the pairs read: {}",
            line.join("; ")
        );
    }
    // Rule 6's named counts are read with round 3's model on every pair (R3-9), below.
    // Round 3 (PREREGISTER.txt, "ROUND 3"), when its calibration cells are read: R3-2's variable,
    // R3-1's factor and correction, R3-5's domain, rule 5b's margin and R3-6's 1/√N over the
    // pairs read, per method, quantity and split.
    if roles.contains(&Role::Calibration3) {
        let ids: Vec<&str> = cells.iter().map(|c| c.id).collect();
        for method in ["random", "energetic"] {
            let cal = calibration::calibration3_cells(&all, method);
            let (var, gms) = calibration::choose_var(&all, method);
            println!(
                "\nROUND 3, {method} mode, on {} calibration cells: variable {} (geometric-mean \
                 overstatement {})",
                cal.len(),
                calibration::var_name(var),
                gms.iter()
                    .map(|(v, g)| format!("{} {g:.4}", calibration::var_name(*v)))
                    .collect::<Vec<_>>()
                    .join(", ")
            );
            for q in calibration::QUANTITIES {
                let (confirmed, read, unsafe_) =
                    calibration::root_n3_confirmed(&all, method, q, &ids);
                for &s in calibration::splits(method, q) {
                    let fit = calibration::fit(&cal, q, var, s);
                    let dom = calibration::domain(&cal, q, var, s);
                    let margin = calibration::margin3(&cal, q, s);
                    println!(
                        "  {q:<7} {:<7} {}; domain {}; margin {}; 1/sqrt(N) {} (read {read:?}, \
                         unsafe {unsafe_:?})",
                        calibration::split_name(s),
                        match &fit {
                            Some(f) => format!(
                                "k {} kappa {} (set by {}, {} cells, overstates {:.3})",
                                f.k, f.kappa, f.by, f.cells, f.overstates
                            ),
                            None => "NO FIT".into(),
                        },
                        dom.map_or("-".into(), |d| format!(
                            "N >= {}, {} <= {:.6}, mean absorption <= {}",
                            d.min_particles,
                            calibration::var_name(var),
                            d.max_n,
                            d.max_mean_absorption
                        )),
                        margin.map_or("-".into(), |(m, by)| format!("{m} (by {by})")),
                        if confirmed { "confirmed" } else { "NOT" },
                    );
                    // Every calibration cell's pooled ratio against the fit (those below the
                    // domain's particle bound, A1, marked).
                    if let Some(f) = &fit {
                        let least = dom.map_or(0.0, |d| d.min_particles);
                        for c in &cal {
                            let below = c["cell"]["particles_per_source"].as_f64().unwrap() < least;
                            let rows = calibration::rows3(c, q, var, s, f64::INFINITY);
                            if let Some(p) =
                                calibration::pooled3(&rows, f.k, f.kappa, calibration::seeds(c))
                            {
                                println!(
                                    "      {:<6} {:>2} rows, n up to {:.4}: {:.3} [{:.3}, {:.3}] \
                                     dof {:.1}{}",
                                    c["cell"]["id"].as_str().unwrap(),
                                    rows.len(),
                                    rows.iter().map(|r| r.n_max()).fold(0.0, f64::max),
                                    p.ratio,
                                    p.lower,
                                    p.upper,
                                    p.dof,
                                    if below { " (below the domain)" } else { "" }
                                );
                            }
                        }
                    }
                }
            }
        }
    }
    // Round 3's validation (R3-8), with the numbers committed in `params::noise::calibration`,
    // and the named counts on every pair read (R3-9), judged by the code a report uses.
    if roles.contains(&Role::Validation3) {
        for method in ["random", "energetic"] {
            let m = method_of(method);
            let var = match noise::calibration::variable(m) {
                noise::calibration::Variable::CrossingsPerParticle => calibration::Var::N1,
                noise::calibration::Variable::CrossingsTimesLifetimeSpread => {
                    calibration::Var::N1Cv2
                }
            };
            println!("\nROUND 3 VALIDATION, {method} mode, with the code's numbers:");
            for c in calibration::validation3_cells(&all, method) {
                let id = c["cell"]["id"].as_str().unwrap();
                let particles = c["cell"]["particles_per_source"].as_f64().unwrap();
                let mut line = format!("  {id} (N {particles})");
                for (qi, q) in calibration::QUANTITIES.iter().enumerate() {
                    for &s in calibration::splits(method, q) {
                        let walls = match s {
                            calibration::Split::UniformLambert => noise::Walls::UniformLambert,
                            calibration::Split::Lambert => noise::Walls::Lambert,
                            _ => noise::Walls::Other,
                        };
                        let e = noise::calibration::entry(m, qi, walls);
                        let max_a = noise::calibration::UNIFORM_LAMBERT_MAX_MEAN_ABSORPTION;
                        let all_rows = calibration::rows_in_use(c, q, var, s, max_a, f64::INFINITY);
                        if all_rows.is_empty() {
                            continue;
                        }
                        let inside = if particles < f64::from(e.min_particles) {
                            Vec::new()
                        } else {
                            calibration::rows_in_use(
                                c,
                                q,
                                var,
                                s,
                                max_a,
                                e.max_crossings_per_particle,
                            )
                        };
                        let name = format!("{q} {}", calibration::split_name(s));
                        match calibration::pooled3(
                            &inside,
                            e.factor,
                            e.kappa,
                            calibration::seeds(c),
                        ) {
                            Some(p) => {
                                line += &format!(
                                    "\n    {name:<17} {:>2} of {:>2} rows inside: {:.3} [{:.3}, \
                                     {:.3}] dof {:.1} {}",
                                    inside.len(),
                                    all_rows.len(),
                                    p.ratio,
                                    p.lower,
                                    p.upper,
                                    p.dof,
                                    if p.lower <= 1.0 { "ok" } else { "FAIL" }
                                )
                            }
                            None => {
                                line += &format!(
                                    "\n    {name:<17}  0 of {:>2} rows inside the domain",
                                    all_rows.len()
                                )
                            }
                        }
                    }
                }
                println!("{line}");
            }
            let ids: Vec<&str> = cells.iter().map(|c| c.id).collect();
            for q in calibration::QUANTITIES {
                let (ok, read, unsafe_) = calibration::root_n3_confirmed(&all, method, q, &ids);
                println!("  1/sqrt(N) {q:<7}: {ok} over {read:?}; unsafe {unsafe_:?}");
            }
        }
        for (a, b) in calibration::PAIRS3 {
            let (Some(ia), Some(ib)) = (
                cells.iter().position(|c| c.id == a),
                cells.iter().position(|c| c.id == b),
            ) else {
                continue;
            };
            let (ca, cb) = (&cells[ia], &cells[ib]);
            println!(
                "\nnamed counts (round 3), {a} ({}) against {b} ({}):",
                ca.particles, cb.particles
            );
            for qi in 0..8 {
                let (mut refusals, mut named, mut within, mut pass, mut outside) =
                    (0, 0, 0, 0.0, 0);
                let rbs = runs[ia][0].values.len();
                for rb in 0..rbs {
                    let hi: Vec<Option<Judged>> =
                        runs[ib].iter().map(|r| judged(r, rb, qi)).collect();
                    for run in &runs[ia] {
                        let n = match judged(run, rb, qi) {
                            None | Some(Judged::Given) => continue,
                            Some(Judged::FewParticles(_) | Judged::ManyCrossings) => {
                                outside += 1;
                                continue;
                            }
                            Some(Judged::Named(n)) => n,
                            Some(_) => {
                                refusals += 1;
                                continue;
                            }
                        };
                        refusals += 1;
                        named += 1;
                        if n > u64::from(cb.particles) {
                            continue;
                        }
                        within += 1;
                        let given = hi.iter().filter(|h| **h == Some(Judged::Given)).count();
                        pass += given as f64 / hi.len() as f64;
                    }
                }
                if refusals > 0 || outside > 0 {
                    println!(
                        "    {:<7} {refusals} refused for noise at {a}, {named} name a count, \
                         {within} at most {b}'s; of those, {:.1} % of {b}'s seeds give the \
                         value; {outside} outside the domain at {a}",
                        calibration::QUANTITIES[qi],
                        if within > 0 {
                            100.0 * pass / within as f64
                        } else {
                            f64::NAN
                        }
                    );
                }
            }
        }
        // Every cell of the pairs and the validation: how many values come through the shipped
        // model, and why the rest are refused.
        println!(
            "\nvalues through the shipped model (receiver-band seeds whose series gives one):"
        );
        for (ci, c) in cells.iter().enumerate() {
            let mut line = format!("  {:<6}", c.id);
            for qi in 0..8 {
                let (mut given, mut total) = (0, 0);
                let mut why: std::collections::BTreeMap<&str, usize> = Default::default();
                for run in &runs[ci] {
                    for rb in 0..run.values.len() {
                        let Some(j) = judged(run, rb, qi) else {
                            continue;
                        };
                        total += 1;
                        let w = match j {
                            Judged::Given => {
                                given += 1;
                                continue;
                            }
                            Judged::Named(_) | Judged::NoCount => "noise",
                            Judged::Resampled(_) | Judged::BeyondResampled => "resamples",
                            Judged::FewParticles(_) => "few",
                            Judged::ManyCrossings => "crossings",
                        };
                        *why.entry(w).or_default() += 1;
                    }
                }
                if total > 0 {
                    line += &format!(
                        " {} {given}/{total}{}",
                        calibration::QUANTITIES[qi],
                        if why.is_empty() {
                            String::new()
                        } else {
                            format!(" {why:?}")
                        }
                    );
                }
            }
            println!("{line}");
        }
    }
    if let Ok(out) = std::env::var("SIMPA_NOISE_OUT") {
        // Six significant digits are more than ten seeds resolve, and keep the receipt small.
        fn round(v: &mut Value) {
            match v {
                Value::Number(n) if n.is_f64() => {
                    let x = n.as_f64().unwrap();
                    if x != 0.0 && x.is_finite() {
                        let s: f64 = format!("{x:.5e}").parse().unwrap();
                        *v = json!(s);
                    }
                }
                Value::Array(a) => a.iter_mut().for_each(round),
                Value::Object(o) => o.values_mut().for_each(round),
                _ => {}
            }
        }
        let mut out_json = json!({
            "preregistration": "PREREGISTER.txt",
            "cells": receipts,
        });
        round(&mut out_json);
        std::fs::write(&out, serde_json::to_string(&out_json).unwrap()).unwrap();
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
    // Where the reports go: beside each run, or under `$SIMPA_T1_OUT` when it is set (reading an
    // earlier piece's runs again without writing into its folders).
    let out = std::env::var_os("SIMPA_T1_OUT").map(PathBuf::from);
    let (root, reports_root) = match std::env::var_os("SIMPA_T1_FROM") {
        Some(from) if !from.is_empty() => {
            // Read again with this build's `simpa results`.
            let from = PathBuf::from(from);
            let to = out.clone().unwrap_or_else(|| from.clone());
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
                std::fs::create_dir_all(to.join(folder)).unwrap();
                std::fs::write(to.join(folder).join(name), &r.stdout).unwrap();
            }
            (from, to)
        }
        _ => {
            let root = evidence_root("tutorial1-default");
            run_all(&root, &projects);
            (root.clone(), root)
        }
    };
    let name = std::env::var("SIMPA_T1_REPORT").unwrap_or_else(|_| "report.json".into());
    let mut summary = serde_json::Map::new();
    for method in [Random, Energetic] {
        let reports: Vec<Value> = projects
            .iter()
            .filter(|(f, _)| f.starts_with(&format!("{method:?}/")))
            .map(|(f, _)| {
                serde_json::from_str(
                    &std::fs::read_to_string(reports_root.join(f).join(&name)).unwrap(),
                )
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
