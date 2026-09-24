//! The M7 rooms (`tests/fixtures/rooms/PROVENANCE.md`, "M7 rooms"), written from their recipes
//! below and held to them:
//! - `level_box_20m.simpa`, gate M7(c): a 20 x 20 x 20 m box whose every surface absorbs all
//!   energy, SPPS with `direct_calc` on and air absorption off, one omni source of 100 dB per
//!   band, and receivers 2 m and 4 m from it;
//! - `seats_box.simpa`, gate M7(e): tutorial 1's box on two octave bands with its two point
//!   receivers renamed `Seat` and `Seat2`, whose runs are the committed fixtures under
//!   `tests/fixtures/results/`;
//! - `energetic_box.simpa`: the same in energetic mode with `trans_epsilon` 3, for the solver's
//!   floor and energetic mode's completeness;
//! - `sources2_box.simpa`: the same with a second source and an echogram per source, for the
//!   per-source echograms and the refusal of onset-relative parameters on a sum of sources;
//! - `tutorial1_box_asymmetric.simpa`, gate M7(d)'s second room: tutorial 1's box with absorption
//!   under which a mean by area, by face and by material, and a swap of the floor's and the
//!   walls' materials, all give different reverberation times.
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
/// - **What keeps the reverberant field out: the duration.** The source is 9.97 m from the
///   nearest wall, and a particle covers `c·20 ms` = 6.86 m in the whole run, so no particle
///   reaches a surface: SPPS's statistics count 0 absorbed by the materials and every particle
///   remaining (`cli_results.rs`, `gate_c_...`, which asserts it). The receivers see the direct
///   field and nothing else.
/// - **Two more guards, which this run does not exercise.** `direct_calc = 1` makes SPPS absorb a
///   particle at its first surface hit (`spps/CalculationCore.cpp:236-242`), and α = 1 in every
///   band does the same without that switch in both computation methods
///   (`CalculationCore.cpp:249-261, 288-300`), with no transmission (no transmission loss is
///   set). Neither is reached while no particle reaches a surface; they matter only if the
///   duration is made longer.
/// - **Why 0.2 ms steps and 0.5 m receivers.** A receiver of radius `R` sees the direct sound
///   for `2R/c` = 2.9 ms. Spread over about 15 steps, its last steps fall steadily, which is
///   what `params`' tail bound needs before it states a level (`docs/params.md`,
///   "Truncation"). The larger sphere puts 4 times the default's particles through each
///   receiver; averaging `1/d²` over it reads `10·lg(1 + R²/(5r²))` = +0.054 dB high at 2 m.
/// - **Why 20 ms.** The last direct sound, at 4 m, has passed by 13.1 ms, and the first
///   reflection could arrive no earlier than 29 ms.
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

/// The M7 review's energetic-mode room: [`seats_box`] in energetic mode with `trans_epsilon` 3,
/// so SPPS drops each particle at 30 dB below its start, inside the ranges of T20 and T30; 50,000
/// particles, so that the cliff the drop leaves is a smooth fall and not a few stray particles.
pub fn energetic_box() -> Project {
    let mut p = seats_box();
    p.id = ProjectId::from_u128(0x0c0b_e000_0000_4000_8000_0000_0000_0e01);
    p.name = "Energetic seats box".into();
    p.description = "M7 review: the Seat and Seat2 box in energetic mode, trans_epsilon 3, 50,000 \
                     particles. Written by crates/simpa-core/tests/results_rooms.rs."
        .into();
    p.solvers.spps.method = ComputationMethod::Energetic;
    p.solvers.spps.extinction_exponent = schema::F64::new(3.0);
    p.solvers.spps.particles_per_source = 50_000;
    p
}

/// The M7 review's two-source room: [`seats_box`] with a second source, 3 dB weaker and 20 ms
/// late, and an echogram per source at every receiver.
pub fn sources2_box() -> Project {
    let mut p = seats_box();
    p.id = ProjectId::from_u128(0x0c0b_e000_0000_4000_8000_0000_0000_0e02);
    p.name = "Two-source seats box".into();
    p.description = "M7 review: the Seat and Seat2 box with a second source, 3 dB weaker and \
                     20 ms late, and an echogram per source. Written by \
                     crates/simpa-core/tests/results_rooms.rs."
        .into();
    let mut s = p.sources[0].clone();
    s.id = SourceId::from_u128(0x0c0b_e000_0000_4000_8000_0000_0000_0e03);
    s.name = "Source 2".into();
    s.position = Vec3::new(5.0, 8.5, 1.2);
    s.power = Spectrum::new(s.power.global_db.get() - 3.0, s.power.shape.clone());
    s.delay_s = schema::F64::new(0.02);
    p.sources.push(s);
    p.solvers.spps.echogram_per_source = true;
    p
}

/// The floor's absorption in band `i` (0 to 26) of [`asymmetric_box`]: `0.15 + 0.025·i`, to three
/// decimals.
pub fn asymmetric_floor_alpha(i: usize) -> f64 {
    ((0.15 + 0.025 * i as f64) * 1000.0).round() / 1000.0
}

/// Gate M7(d)'s second room (M7 review): tutorial 1's box with absorption that tells apart the
/// ways of combining it. On tutorial 1 itself the floor's 0.1 and the ceiling's 0.3 sit on equal
/// areas and average to the walls' 0.2, so a mean weighted by area, a mean over faces and a mean
/// over materials all give 0.2, and the check cannot see which one the code takes. Here:
/// - the floor (60 m², 2 faces) rises by band, [`asymmetric_floor_alpha`], 0.15 to 0.80;
/// - the walls (96 m², 8 faces) are 0.1 in every band;
/// - the ceiling (60 m², 2 faces) stays 0.3.
///
/// Then `A` weighted by area is `60·α_f + 27.6` m², and in every band a mean over the 12 faces
/// (`36·α_f + 25.2`) is at least 16 % lower, a mean over the three materials (`72·α_f + 28.8`) at
/// least 8 % higher, the floor's and the walls' materials swapped (`96·α_f + 24`) at least 4.9 %
/// higher, and the neighbouring band's floor 1.5 m² off. The floor and the walls never share an
/// α, so a swap between them always shows. Swapping the floor and the ceiling cannot show in any
/// room like this: they have equal areas, and Sabine and Eyring see only `A` and `S`.
pub fn asymmetric_box() -> Project {
    let mut p = tutorial_box();
    p.id = ProjectId::from_u128(0x0c0b_e000_0000_4000_8000_0000_0000_0d00);
    p.name = "Asymmetric absorption box".into();
    p.description = "Gate M7(d), M7 review: tutorial 1's box with the floor's absorption rising \
                     from 0.15 to 0.80 over the 27 bands, the walls at 0.1 and the ceiling at \
                     0.3, so that a mean by area, by face and by material differ. Written by \
                     crates/simpa-core/tests/results_rooms.rs."
        .into();
    let n = p.bands.len();
    let material_of = |p: &Project, group: &str| {
        let g = p
            .surface_groups
            .iter()
            .find(|g| g.name == group)
            .expect("a tutorial group");
        p.materials
            .iter()
            .position(|m| m.id == g.material)
            .expect("its material")
    };
    let floor = material_of(&p, "Floor");
    let walls = material_of(&p, "Walls");
    p.materials[floor].name = "Rising absorption".into();
    p.materials[floor].absorption = (0..n)
        .map(|i| schema::F64::new(asymmetric_floor_alpha(i)))
        .collect();
    p.materials[walls].name = "10% absorbing".into();
    p.materials[walls].absorption = vec![schema::F64::new(0.1); n];
    p
}

/// Every M7 room, by file name.
fn rooms() -> Vec<(&'static str, Project)> {
    vec![
        ("level_box_20m.simpa", level_box()),
        ("seats_box.simpa", seats_box()),
        ("energetic_box.simpa", energetic_box()),
        ("sources2_box.simpa", sources2_box()),
        ("tutorial1_box_asymmetric.simpa", asymmetric_box()),
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

/// Per surface group: its name, its faces' areas and its material's absorption per band.
fn groups(p: &Project) -> Vec<(String, Vec<f64>, Vec<f64>)> {
    let v = &p.geometry.vertices;
    p.surface_groups
        .iter()
        .map(|g| {
            let areas = p
                .geometry
                .faces
                .iter()
                .filter(|f| f.group == g.id)
                .map(|f| {
                    let [a, b, c] = f.vertices.map(|i| v[i as usize].to_array());
                    let (u, w) = (
                        [b[0] - a[0], b[1] - a[1], b[2] - a[2]],
                        [c[0] - a[0], c[1] - a[1], c[2] - a[2]],
                    );
                    let n = [
                        u[1] * w[2] - u[2] * w[1],
                        u[2] * w[0] - u[0] * w[2],
                        u[0] * w[1] - u[1] * w[0],
                    ];
                    0.5 * (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt()
                })
                .collect();
            let m = p.materials.iter().find(|m| m.id == g.material).unwrap();
            let alphas = m.absorption.iter().map(|a| a.get()).collect();
            (g.name.clone(), areas, alphas)
        })
        .collect()
}

#[test]
fn the_asymmetric_box_tells_apart_the_ways_of_combining_absorption() {
    let asym = asymmetric_box();
    let g = groups(&asym);
    let find = |name: &str| g.iter().find(|x| x.0 == name).unwrap();
    let (floor, walls, ceiling) = (find("Floor"), find("Walls"), find("Ceiling"));
    assert_eq!(
        [floor.1.len(), walls.1.len(), ceiling.1.len()],
        [2, 8, 2],
        "faces"
    );
    let area = |x: &(String, Vec<f64>, Vec<f64>)| x.1.iter().sum::<f64>();
    assert!((area(floor) - 60.0).abs() < 1e-9 && (area(ceiling) - 60.0).abs() < 1e-9);
    assert!((area(walls) - 96.0).abs() < 1e-9);
    let (mut face, mut material, mut swapped, mut neighbour) =
        (f64::MAX, f64::MAX, f64::MAX, f64::MAX);
    for i in 0..27 {
        let (af, aw, ac) = (floor.2[i], walls.2[i], ceiling.2[i]);
        assert_eq!(af, asymmetric_floor_alpha(i));
        assert!(af != aw, "band {i}: the floor and the walls share α");
        let a = 60.0 * af + 96.0 * aw + 60.0 * ac;
        let rel = |x: f64| (x / a - 1.0).abs();
        face = face.min(rel(216.0 * (2.0 * af + 8.0 * aw + 2.0 * ac) / 12.0));
        material = material.min(rel(216.0 * (af + aw + ac) / 3.0));
        swapped = swapped.min(rel(96.0 * af + 60.0 * aw + 60.0 * ac));
        let j = if i < 26 { i + 1 } else { i - 1 };
        neighbour = f64::min(neighbour, (60.0 * (floor.2[j] - af)).abs());
    }
    println!(
        "smallest differences from A by area: by face {face:.3}, by material {material:.3}, \
         floor and walls swapped {swapped:.3} (relative); neighbouring band {neighbour:.3} m2"
    );
    assert!(
        face >= 0.16 && material >= 0.08 && swapped >= 0.049,
        "the doc's figures"
    );
    assert!((neighbour - 1.5).abs() < 1e-9);

    // Says no: tutorial 1's own box cannot tell them apart; every rule gives the same A.
    let t = groups(&tutorial_box());
    let alpha = |name: &str| t.iter().find(|x| x.0 == name).unwrap().2[0];
    let (af, aw, ac) = (alpha("Floor"), alpha("Walls"), alpha("Ceiling"));
    let a = 60.0 * af + 96.0 * aw + 60.0 * ac;
    for other in [
        216.0 * (2.0 * af + 8.0 * aw + 2.0 * ac) / 12.0,
        216.0 * (af + aw + ac) / 3.0,
    ] {
        assert!((other / a - 1.0).abs() < 1e-12, "{other} vs {a}");
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
