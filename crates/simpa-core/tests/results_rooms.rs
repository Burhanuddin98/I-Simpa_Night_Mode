//! The M7 rooms (`tests/fixtures/rooms/PROVENANCE.md`, "M7 rooms"), written from their recipes
//! below and held to them:
//! - `level_box_20m.simpa`, gate M7(c): a 20 x 20 x 20 m box whose every surface absorbs all
//!   energy, SPPS with `direct_calc` on and air absorption off, one omni source of 100 dB per
//!   band, and receivers 2 m and 4 m from it;
//! - `seats_box.simpa`, gate M7(e): tutorial 1's box on two octave bands with its two point
//!   receivers renamed `Seat` and `Seat2`, whose runs are the committed fixtures under
//!   `tests/fixtures/results/`.
//!
//! Regenerate with `cargo test -p simpa-core --test results_rooms -- --ignored write_m7_rooms`.

use std::path::Path;

use simpa_core::schema::{
    self, BandSet, ComputationMethod, Directivity, Face, GroupId, Material, MaterialId, Project,
    ProjectId, SourceId, Spectrum, SpectrumShape, SurfaceGroup, Vec3,
};
use simpa_core::validate;

mod common;

/// The level box's source, a little off the centre so that it lies on no internal facet of a
/// symmetric mesh.
pub const LEVEL_SOURCE: [f64; 3] = [10.05, 9.97, 10.03];
/// Its receivers: 2 m along (0.6, 0.64, 0.48) and 4 m along (-0.48, 0.6, -0.64), both unit
/// vectors with exact decimal components, so the distances are 2 and 4 to rounding.
pub const LEVEL_RECEIVERS: [(&str, [f64; 3]); 2] =
    [("R2m", [11.25, 11.25, 10.99]), ("R4m", [8.13, 12.37, 7.47])];

fn tutorial_box() -> Project {
    schema::load(&common::fixture("rooms/tutorial1_box_seeded.simpa"))
        .expect("the seeded tutorial box loads")
}

/// The values of `values` at the positions of `keep` in the tutorial's 27 third-octave bands.
fn pick<T: Clone>(values: &[T], all: &[u32], keep: &[u32]) -> Vec<T> {
    keep.iter()
        .map(|f| values[all.iter().position(|a| a == f).expect("a tutorial band")].clone())
        .collect()
}

/// Gate M7(c)'s room.
///
/// - **Why every surface absorbs everything.** `direct_calc = 1` already makes SPPS absorb a
///   particle at its first surface hit (`spps/CalculationCore.cpp:236-242`). α = 1 in every
///   band makes the same true without that switch, in both computation methods
///   (`CalculationCore.cpp:249-261, 288-300`), with no transmission (no transmission loss is
///   set). Only the direct field can reach a receiver, whichever of the two a future change
///   broke, so the check isolates the chain from source power to level.
/// - **Why 0.2 ms steps and 0.5 m receivers.** A receiver of radius `R` sees the direct sound
///   for `2R/c` = 2.9 ms. Spread over about 15 steps, its last steps fall steadily, which is
///   what `params`' tail bound needs before it states a level (`docs/params.md`,
///   "Truncation"). The larger sphere puts 4 times the default's particles through each
///   receiver; averaging `1/d²` over it reads `10·lg(1 + R²/(5r²))` = +0.054 dB high at 2 m.
/// - **Why 20 ms.** The last direct sound, at 4 m, has passed by 13.1 ms.
pub fn level_box() -> Project {
    let t = tutorial_box();
    let bands = BandSet::default();
    let n = bands.len();
    let walls = GroupId::from_u128(0x0c0b_e000_0000_4000_8000_0000_0000_0701);
    let absorber = MaterialId::from_u128(0x0c0b_e000_0000_4000_8000_0000_0000_0702);
    let mut p = t.clone();
    p.id = ProjectId::from_u128(0x0c0b_e000_0000_4000_8000_0000_0000_0700);
    p.name = "Level calibration box".into();
    p.description = "Gate M7(c): a 20 x 20 x 20 m box, every surface fully absorbing, SPPS direct \
                     field only without air absorption, one omni source of 100 dB per band, \
                     receivers at 2 m and 4 m. Written by crates/simpa-core/tests/results_rooms.rs."
        .into();
    p.bands = bands;
    // Tutorial 1's box, 6 x 10 x 3 m, stretched to 20 m on every axis: same faces, same winding.
    p.geometry.vertices = t
        .geometry
        .vertices
        .iter()
        .map(|v| {
            let [x, y, z] = v.to_array();
            Vec3::new(x / 6.0 * 20.0, y / 10.0 * 20.0, z / 3.0 * 20.0)
        })
        .collect();
    p.geometry.faces = t
        .geometry
        .faces
        .iter()
        .map(|f| Face {
            vertices: f.vertices,
            group: walls,
        })
        .collect();
    p.surface_groups = vec![SurfaceGroup {
        id: walls,
        name: "Walls".into(),
        material: absorber,
    }];
    let mut m: Material = t.materials[0].clone();
    m.id = absorber;
    m.name = "Fully absorbing".into();
    m.absorption = vec![schema::F64::new(1.0); n];
    m.scattering = vec![schema::F64::ZERO; n];
    m.transmission_loss_db = None;
    m.solver_id = None;
    p.materials = vec![m];
    let mut s = t.sources[0].clone();
    s.id = SourceId::from_u128(0x0c0b_e000_0000_4000_8000_0000_0000_0703);
    s.name = "Source".into();
    s.position = Vec3::new(LEVEL_SOURCE[0], LEVEL_SOURCE[1], LEVEL_SOURCE[2]);
    // Equal power in every band, summing to 100 dB + 10·lg(6): 100 dB in each.
    s.power = Spectrum::new(100.0 + 10.0 * (n as f64).log10(), SpectrumShape::Pink);
    s.directivity = Directivity::Omni;
    p.sources = vec![s];
    p.point_receivers = LEVEL_RECEIVERS
        .iter()
        .enumerate()
        .map(|(i, (name, at))| {
            let mut r = t.point_receivers[0].clone();
            r.id = schema::PointReceiverId::from_u128(
                0x0c0b_e000_0000_4000_8000_0000_0000_0710 + i as u128,
            );
            r.name = (*name).into();
            r.position = Vec3::new(at[0], at[1], at[2]);
            r.orientation = Vec3::new(1.0, 0.0, 0.0);
            r
        })
        .collect();
    p.surface_receivers = Vec::new();
    p.fitting_zones = Vec::new();
    let spps = &mut p.solvers.spps;
    spps.particles_per_source = 1_000_000;
    spps.particles_saved = 0;
    spps.duration_s = schema::F64::new(0.02);
    spps.time_step_s = schema::F64::new(0.0002);
    spps.random_seed = 1;
    spps.method = ComputationMethod::Random;
    spps.air_absorption = false;
    spps.direct_field_only = true;
    spps.transmission = false;
    spps.receiver_radius_m = schema::F64::new(0.5);
    spps.bands_computed = vec![true; n];
    p.solvers.tcr.air_absorption = false;
    p.solvers.tcr.bands_computed = vec![true; n];
    p
}

/// Gate M7(e)'s room: tutorial 1's box on the octave bands 500 Hz and 1 kHz, its receivers
/// renamed `Seat` (at (1, 1, 1.8)) and `Seat2` (at (3, 7, 1.8)), so that one label is a prefix of
/// the other; 2,000 particles over 1 s, seed 1; the floor's surface receiver meshed to 4 m² faces
/// so the committed run folders stay small.
pub fn seats_box() -> Project {
    let t = tutorial_box();
    let all = t.bands.frequencies_hz.clone();
    let keep = [500u32, 1000];
    let mut p = t.clone();
    p.id = ProjectId::from_u128(0x0c0b_e000_0000_4000_8000_0000_0000_0e00);
    p.name = "Seat and Seat2 box".into();
    p.description = "Gate M7(e): tutorial 1's box on the 500 Hz and 1 kHz octave bands, its point \
                     receivers named Seat and Seat2. Written by \
                     crates/simpa-core/tests/results_rooms.rs."
        .into();
    p.bands = BandSet::range(schema::BandKind::Octave, 500, 1000).expect("octave bands");
    for m in &mut p.materials {
        m.absorption = pick(&m.absorption, &all, &keep);
        m.scattering = pick(&m.scattering, &all, &keep);
    }
    for (r, name) in p.point_receivers.iter_mut().zip(["Seat", "Seat2"]) {
        r.name = name.into();
    }
    let spps = &mut p.solvers.spps;
    spps.particles_per_source = 2_000;
    spps.duration_s = schema::F64::new(1.0);
    spps.bands_computed = vec![true; keep.len()];
    p.solvers.tcr.bands_computed = vec![true; keep.len()];
    p.solvers.meshing.surface_receiver_max_area_m2 = Some(schema::F64::new(4.0));
    p
}

/// Every M7 room, by file name.
fn rooms() -> Vec<(&'static str, Project)> {
    vec![
        ("level_box_20m.simpa", level_box()),
        ("seats_box.simpa", seats_box()),
    ]
}

fn room_path(name: &str) -> std::path::PathBuf {
    common::fixture(&format!("rooms/{name}"))
}

#[test]
#[ignore = "writes tests/fixtures/rooms/; run on purpose after changing a recipe"]
fn write_m7_rooms() {
    for (name, p) in rooms() {
        schema::save(&p, &room_path(name)).unwrap();
        println!("wrote {}", room_path(name).display());
    }
}

#[test]
fn the_m7_rooms_are_their_recipes_and_validate_clean() {
    for (name, p) in rooms() {
        let path = room_path(name);
        let committed = std::fs::read_to_string(&path).unwrap_or_else(|e| {
            panic!(
                "{}: {e}; write it with `cargo test -p simpa-core --test results_rooms -- \
                 --ignored write_m7_rooms`",
                path.display()
            )
        });
        assert_eq!(
            committed,
            schema::to_json(&p),
            "{name} drifted from its recipe"
        );
        let loaded = schema::load(&path).unwrap();
        assert_eq!(loaded, p, "{name} does not load back as its recipe");
        let ctx = validate::Context::for_project_file(Path::new(&path));
        let issues = validate::validate_with(&loaded, &ctx);
        assert!(issues.is_empty(), "{name}: {issues:#?}");
    }
}

#[test]
fn the_level_box_emits_100_db_in_every_band_and_its_receivers_are_2_and_4_m_away() {
    let p = level_box();
    let levels =
        simpa_core::config_xml::band_levels_written(&p.sources[0].power, &p.bands).unwrap();
    assert_eq!(levels.len(), 6);
    for l in &levels {
        assert!((l - 100.0).abs() < 1e-12, "{levels:?}");
        // The solver reads the level as an f32: exactly 100.
        assert_eq!(*l as f32, 100.0f32);
    }
    let s = LEVEL_SOURCE;
    for ((_, at), want) in LEVEL_RECEIVERS.iter().zip([2.0, 4.0]) {
        let r = ((at[0] - s[0]).powi(2) + (at[1] - s[1]).powi(2) + (at[2] - s[2]).powi(2)).sqrt();
        assert!((r - want).abs() < 1e-12, "{r}");
    }
    // Every surface absorbs everything, and SPPS keeps the direct field only.
    assert!(
        p.materials
            .iter()
            .all(|m| m.absorption.iter().all(|a| a.get() == 1.0))
    );
    assert!(p.solvers.spps.direct_field_only && !p.solvers.spps.air_absorption);
}
