//! `core::validate` on valid projects (the cube fixture and 1,000 generated ones), on every kind
//! of structural fault `check_integrity` knows, and on the edges of the value rules.

use std::collections::BTreeMap;
use std::path::PathBuf;

use simpa_core::schema::{
    self, Directivity, F64, GroupId, MaterialId, MaterialOverride, Project, SOLVER_INT_MAX,
    SurfaceGroup, SurfaceReceiver, SurfaceReceiverId, SurfaceReceiverShape, Variant, VariantId,
    Vec3, generate,
};
use simpa_core::validate::{
    self, Context, Issue, PointLocation, STRUCTURAL_CODES, Severity, codes, has_errors,
    locate_point,
};

fn repo(rel: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(rel)
}

fn cube() -> Project {
    schema::load(&repo("tests/fixtures/projects/cube.simpa")).expect("cube.simpa loads")
}

fn codes_of(issues: &[Issue]) -> Vec<&'static str> {
    issues.iter().map(|i| i.code).collect()
}

#[test]
fn the_cube_fixture_has_no_issue() {
    let path = repo("tests/fixtures/projects/cube.simpa");
    let p = cube();
    assert_eq!(validate::validate(&p), Vec::new());
    assert_eq!(
        validate::validate_with(&p, &Context::for_project_file(&path)),
        Vec::new()
    );
    // read_project gives the same project as load for a file that passes integrity.
    assert_eq!(validate::read_project(&path).unwrap(), p);
}

#[test]
fn a_thousand_generated_projects_have_no_error() {
    let mut by_code: BTreeMap<&str, usize> = BTreeMap::new();
    let mut failing = Vec::new();
    let mut clean = 0;
    for seed in 0..1000u64 {
        let p = generate(seed);
        let issues = validate::validate(&p);
        for i in &issues {
            *by_code.entry(i.code).or_default() += 1;
        }
        if has_errors(&issues) {
            failing.push((seed, issues));
        } else if issues.is_empty() {
            clean += 1;
        }
    }
    println!(
        "{}/1000 generated projects with 0 errors; {clean} with no issue at all; issues by code: \
         {by_code:?}",
        1000 - failing.len()
    );
    for (seed, issues) in failing.iter().take(5) {
        println!("seed {seed}: {issues:#?}");
    }
    assert!(failing.is_empty(), "{} projects failed", failing.len());
    assert_eq!(clean, 1000, "generated projects also carry no warning");
}

#[test]
#[ignore = "a wider scan of generate(): 100,000 seeds, about a minute in debug"]
fn a_hundred_thousand_generated_projects() {
    let mut failing: BTreeMap<&str, Vec<u64>> = BTreeMap::new();
    for seed in 0..100_000u64 {
        for i in validate::validate(&generate(seed)) {
            let seeds = failing.entry(i.code).or_default();
            if seeds.len() < 2 {
                println!("seed {seed}: {} at {}: {}", i.code, i.path, i.message);
            }
            seeds.push(seed);
        }
    }
    for (code, seeds) in &failing {
        println!(
            "{code}: {} seeds, the first {:?}",
            seeds.len(),
            &seeds[..seeds.len().min(5)]
        );
    }
    println!("{} codes raised over 100000 seeds", failing.len());
    assert!(failing.is_empty());
}

#[test]
fn validation_is_deterministic() {
    for seed in [0, 7, 123, 999] {
        let p = generate(seed);
        assert_eq!(validate::validate(&p), validate::validate(&p.clone()));
    }
    let mut p = cube();
    p.environment.pressure_pa = F64::new(-1.0);
    p.materials[0].absorption[0] = F64::new(2.0);
    p.point_receivers[0].name = "CON".to_string();
    let first = validate::validate(&p);
    for _ in 0..10 {
        assert_eq!(validate::validate(&p), first);
    }
    // The contract page's order: materials before the environment before names.
    assert_eq!(
        codes_of(&first),
        vec![
            codes::MATERIAL_VALUE_OUT_OF_RANGE,
            codes::ATMOSPHERE_INVALID,
            codes::NAME_NOT_FILENAME_SAFE
        ]
    );
}

type Mutation = (&'static str, fn(&mut Project) -> bool);

/// One structural fault per kind `check_integrity` knows, and the code `validate` gives it.
fn structural_mutations() -> Vec<Mutation> {
    vec![
        (codes::VERSION, |p| {
            p.format_version = 2;
            true
        }),
        (codes::BANDS, |p| {
            p.bands.frequencies_hz[0] += 1;
            true
        }),
        (codes::BANDS, |p| {
            p.bands.frequencies_hz.reverse();
            p.bands.len() >= 2
        }),
        (codes::BAND_DUPLICATE, |p| {
            let f = &mut p.bands.frequencies_hz;
            if f.len() < 2 {
                return false;
            }
            f[1] = f[0];
            true
        }),
        (codes::BAND_SET_EMPTY, |p| {
            p.bands.frequencies_hz.clear();
            true
        }),
        (codes::DUPLICATE_ID, |p| {
            let g = p.surface_groups[0].clone();
            p.surface_groups.push(g);
            true
        }),
        (codes::BAND_SET_MISMATCH, |p| {
            p.materials[0].scattering.push(F64::ZERO);
            true
        }),
        (codes::BAND_SET_MISMATCH, |p| {
            p.solvers.tcr.bands_computed.pop();
            true
        }),
        (codes::MATERIAL_UNASSIGNED, |p| {
            p.surface_groups[0].material = MaterialId::from_u128(1);
            true
        }),
        (codes::MATERIAL_UNASSIGNED, |p| {
            p.geometry.faces[3].group = GroupId::from_u128(2);
            true
        }),
        (codes::FACE_VERTEX, |p| {
            p.geometry.faces[0].vertices[1] = p.geometry.vertices.len() as u32;
            true
        }),
        (codes::DANGLING_REFERENCE, |p| {
            p.surface_receivers.push(SurfaceReceiver {
                id: SurfaceReceiverId::from_u128(3),
                name: "Dangling".to_string(),
                enabled: false,
                shape: SurfaceReceiverShape::Scene {
                    groups: vec![GroupId::from_u128(4)],
                },
            });
            true
        }),
        (codes::REPEATED_GROUP, |p| {
            let g = p.surface_groups[0].id;
            p.surface_receivers.push(SurfaceReceiver {
                id: SurfaceReceiverId::from_u128(5),
                name: "Twice".to_string(),
                enabled: true,
                shape: SurfaceReceiverShape::Scene { groups: vec![g, g] },
            });
            true
        }),
        (codes::OVERRIDE_ORDER, |p| {
            let m = p.materials[0].id;
            let mut groups: Vec<GroupId> = p.surface_groups.iter().map(|g| g.id).collect();
            if groups.len() < 2 {
                p.surface_groups.push(SurfaceGroup {
                    id: GroupId::from_u128(6),
                    name: "Extra".to_string(),
                    material: m,
                });
                groups.push(GroupId::from_u128(6));
            }
            groups.sort();
            groups.reverse();
            p.variants.push(Variant {
                id: VariantId::from_u128(7),
                name: "Unsorted".to_string(),
                overrides: groups
                    .iter()
                    .map(|&group| MaterialOverride { group, material: m })
                    .collect(),
            });
            true
        }),
        (codes::VARIANT_REFERENCE_INVALID, |p| {
            p.active_variant = Some(VariantId::from_u128(8));
            true
        }),
        (codes::VARIANT_REFERENCE_INVALID, |p| {
            p.variants.push(Variant {
                id: VariantId::from_u128(9),
                name: "Dangling".to_string(),
                overrides: vec![MaterialOverride {
                    group: p.surface_groups[0].id,
                    material: MaterialId::from_u128(10),
                }],
            });
            true
        }),
        (codes::SOLVER_INT_RANGE, |p| {
            p.solvers.spps.random_seed = SOLVER_INT_MAX + 1;
            true
        }),
        (codes::SOLVER_INT_RANGE, |p| {
            p.materials[0].solver_id = Some(u32::MAX);
            true
        }),
        (codes::SOLVER_ID_MAPPING_INVALID, |p| {
            let mut twin = p.materials[0].clone();
            twin.id = MaterialId::from_u128(11);
            twin.solver_id = Some(12_345);
            p.materials[0].solver_id = Some(12_345);
            p.materials.push(twin);
            true
        }),
        (codes::PARTICLE_COUNT_INVALID, |p| {
            p.solvers.spps.particles_per_source = SOLVER_INT_MAX + 1;
            true
        }),
    ]
}

#[test]
fn every_structural_fault_is_an_error_under_its_code() {
    let mut table: BTreeMap<(&str, &str), usize> = BTreeMap::new();
    let mut applied = 0;
    for seed in 0..200u64 {
        for (code, mutate) in structural_mutations() {
            let mut p = generate(seed);
            if !mutate(&mut p) {
                continue;
            }
            applied += 1;
            let integrity = p
                .check_integrity()
                .expect_err("the mutation breaks integrity");
            let issues = validate::validate(&p);
            assert!(
                issues
                    .iter()
                    .any(|i| i.code == code && i.severity == Severity::Error),
                "seed {seed}, {code}: {issues:#?}"
            );
            *table.entry((integrity.code(), code)).or_default() += 1;
        }
    }
    println!(
        "{applied} mutated projects; (check_integrity's first code, validate's code): {table:#?}"
    );
    // Every structural code is exercised.
    for code in STRUCTURAL_CODES {
        assert!(table.keys().any(|(_, c)| *c == code), "{code}");
    }
}

#[test]
fn projects_that_pass_integrity_get_no_structural_code() {
    for seed in 0..300u64 {
        let p = generate(seed);
        p.check_integrity().unwrap();
        let issues = validate::validate(&p);
        assert!(
            issues.iter().all(|i| !STRUCTURAL_CODES.contains(&i.code)),
            "{issues:#?}"
        );
    }
}

/// xorshift64*, for the perturbation test.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_f491_4f6c_dd1d)
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
}

/// JSON pointers to every number and every array in `v`.
fn leaves(v: &serde_json::Value, at: String, numbers: &mut Vec<String>, arrays: &mut Vec<String>) {
    match v {
        serde_json::Value::Number(_) => numbers.push(at),
        serde_json::Value::Array(items) => {
            for (i, item) in items.iter().enumerate() {
                leaves(item, format!("{at}/{i}"), numbers, arrays);
            }
            arrays.push(at);
        }
        serde_json::Value::Object(map) => {
            for (k, item) in map {
                leaves(item, format!("{at}/{k}"), numbers, arrays);
            }
        }
        _ => {}
    }
}

#[test]
fn perturbed_projects_never_panic_and_broken_ones_always_fail() {
    let replacements = [
        serde_json::json!(0),
        serde_json::json!(-1),
        serde_json::json!(0.5),
        serde_json::json!(1.5),
        serde_json::json!(1e-50),
        serde_json::json!(1e39),
        serde_json::json!(1e300),
        serde_json::json!(2_147_483_648u64),
        serde_json::json!(4_294_967_295u64),
        serde_json::json!("NaN"),
        serde_json::json!("Infinity"),
        serde_json::json!("-Infinity"),
    ];
    let mut rng = Rng(0x9e37_79b9_7f4a_7c15);
    let (mut read, mut refused, mut broken, mut with_errors) = (0, 0, 0, 0);
    let mut by_code: BTreeMap<&str, usize> = BTreeMap::new();
    for seed in 0..300u64 {
        let text = schema::to_json(&generate(seed));
        let original: serde_json::Value = serde_json::from_str(&text).unwrap();
        let (mut numbers, mut arrays) = (Vec::new(), Vec::new());
        leaves(&original, String::new(), &mut numbers, &mut arrays);
        for _ in 0..10 {
            let mut v = original.clone();
            for _ in 0..1 + rng.below(3) {
                if rng.below(3) == 0 {
                    let at = &arrays[rng.below(arrays.len())];
                    if let Some(serde_json::Value::Array(items)) = v.pointer_mut(at) {
                        match rng.below(3) {
                            0 => {
                                items.pop();
                            }
                            1 if !items.is_empty() => {
                                let first = items[0].clone();
                                items.push(first);
                            }
                            _ => items.clear(),
                        }
                    }
                } else {
                    let at = &numbers[rng.below(numbers.len())];
                    if let Some(slot) = v.pointer_mut(at) {
                        *slot = replacements[rng.below(replacements.len())].clone();
                    }
                }
            }
            // Written with serde_json and read with the exact reader, as a file would be.
            let Ok(p) = validate::read_project_text(&serde_json::to_string(&v).unwrap()) else {
                refused += 1;
                continue;
            };
            read += 1;
            let issues = validate::validate(&p);
            for i in &issues {
                *by_code.entry(i.code).or_default() += 1;
            }
            if has_errors(&issues) {
                with_errors += 1;
            }
            if p.check_integrity().is_err() {
                broken += 1;
                assert!(
                    has_errors(&issues),
                    "seed {seed}: integrity fails, no error"
                );
            }
        }
    }
    println!(
        "3000 perturbed projects: {read} read ({refused} refused by the schema), {broken} break \
         integrity, {with_errors} with errors; codes raised: {by_code:?}"
    );
    assert!(read > 1000 && broken > 100);
}

fn only(issues: &[Issue], code: &str) {
    assert_eq!(codes_of(issues), vec![code], "{issues:#?}");
}

#[test]
fn values_are_judged_as_the_solver_reads_them() {
    // 1e-50 m is above 0 as an f64 and 0 as the solver's f32.
    let mut p = cube();
    p.solvers.spps.receiver_radius_m = F64::new(1e-50);
    only(&validate::validate(&p), codes::RECEIVER_RADIUS_INVALID);

    // 1e39 is finite as an f64 and infinite as an f32.
    let mut p = cube();
    p.solvers.spps.extinction_exponent = F64::new(1e39);
    only(&validate::validate(&p), codes::TRANS_EPSILON_INVALID);

    // A delay inside the last step: ceil(0.9999 / 0.002) = 500 = the step count, so the source
    // would never emit, although the delay is below the duration.
    let mut p = cube();
    p.sources[0].delay_s = F64::new(0.9999);
    only(&validate::validate(&p), codes::SOURCE_DELAY_INVALID);
    p.sources[0].delay_s = F64::new(0.997);
    assert_eq!(validate::validate(&p), Vec::new());

    // 65,535 steps pass, 65,536 do not.
    let mut p = cube();
    p.solvers.spps.time_step_s = F64::new(1.0 / 64.0);
    p.solvers.spps.duration_s = F64::new(65_535.0 / 64.0);
    assert_eq!(validate::validate(&p), Vec::new());
    p.solvers.spps.duration_s = F64::new(65_536.0 / 64.0);
    only(&validate::validate(&p), codes::STEP_COUNT_OVERFLOW);

    // A direction whose components vanish in f32.
    let mut p = cube();
    p.point_receivers[0].orientation = Vec3::new(1e-46, 0.0, 0.0);
    only(&validate::validate(&p), codes::DIRECTION_VECTOR_ZERO);

    // NaN is out of range everywhere.
    let mut p = cube();
    p.materials[0].scattering[0] = F64::new(f64::NAN);
    only(&validate::validate(&p), codes::MATERIAL_VALUE_OUT_OF_RANGE);
}

#[test]
fn only_what_reaches_a_solver_is_judged() {
    // A disabled source may hold anything.
    let mut p = cube();
    let mut ghost = p.sources[0].clone();
    ghost.id = schema::SourceId::from_u128(20);
    ghost.enabled = false;
    ghost.position = Vec3::new(100.0, 0.0, 0.0);
    ghost.name = "CON".to_string();
    ghost.delay_s = F64::new(-1.0);
    ghost.directivity = Directivity::Balloon {
        file: String::new(),
        direction: Vec3::ZERO,
    };
    p.sources.push(ghost);
    assert_eq!(validate::validate(&p), Vec::new());

    // An unused library material too, until a variant puts it on a group.
    let mut bad = p.materials[0].clone();
    bad.id = MaterialId::from_u128(21);
    bad.solver_id = None;
    bad.absorption[0] = F64::new(-0.5);
    p.materials.push(bad);
    assert_eq!(validate::validate(&p), Vec::new());
    p.variants.push(Variant {
        id: VariantId::from_u128(22),
        name: "Bad walls".to_string(),
        overrides: vec![MaterialOverride {
            group: p.surface_groups[0].id,
            material: MaterialId::from_u128(21),
        }],
    });
    assert_eq!(validate::validate(&p), Vec::new());
    p.active_variant = Some(VariantId::from_u128(22));
    let issues = validate::validate(&p);
    only(&issues, codes::MATERIAL_VALUE_OUT_OF_RANGE);
    assert_eq!(issues[0].path, "/materials/1/absorption/0");
}

#[test]
fn geometry_rules_follow_the_room() {
    let mut p = cube();
    // Exactly on a face: near the surface, not outside.
    p.sources[0].position = Vec3::new(3.0, 0.0, 2.5);
    only(&validate::validate(&p), codes::SOURCE_NEAR_SURFACE);
    // 1 mm in is the clearance itself, which passes; just outside a wall is outside.
    p.sources[0].position = Vec3::new(3.0, 0.001, 2.5);
    assert_eq!(validate::validate(&p), Vec::new());
    p.sources[0].position = Vec3::new(3.0, -0.0005, 2.5);
    only(&validate::validate(&p), codes::SOURCE_OUTSIDE_VOLUME);
    p.sources[0].position = Vec3::new(f64::NAN, 1.0, 1.0);
    only(&validate::validate(&p), codes::SOURCE_OUTSIDE_VOLUME);

    // A receiver on a face is not strictly inside.
    let mut p = cube();
    p.point_receivers[0].position = Vec3::new(5.0, 2.0, 2.0);
    only(&validate::validate(&p), codes::RECEIVER_OUTSIDE_VOLUME);

    // Flipping every triangle leaves inside inside.
    let mut p = cube();
    for f in &mut p.geometry.faces {
        f.vertices.swap(1, 2);
    }
    assert_eq!(validate::validate(&p), Vec::new());

    // No geometry at all: everything is outside.
    let mut p = cube();
    p.geometry.faces.clear();
    assert_eq!(
        codes_of(&validate::validate(&p)),
        vec![codes::SOURCE_OUTSIDE_VOLUME, codes::RECEIVER_OUTSIDE_VOLUME]
    );

    let p = cube();
    assert_eq!(
        locate_point(&p.geometry, Vec3::new(2.0, 3.75, 2.5)),
        PointLocation::Inside { clearance_m: 1.25 }
    );
}

#[test]
fn names_are_compared_as_windows_compares_them() {
    let mut p = cube();
    let mut second = p.point_receivers[0].clone();
    second.id = schema::PointReceiverId::from_u128(30);
    second.position = Vec3::new(3.0, 3.0, 3.0);
    second.name = "RECEIVER 1".to_string();
    p.point_receivers.push(second);
    only(&validate::validate(&p), codes::NAME_DUPLICATE);
    // A source may share a receiver's name: they are different files.
    p.point_receivers[1].name = "Source 1".to_string();
    assert_eq!(validate::validate(&p), Vec::new());
    // 49 bytes pass, 50 do not; "é" is two bytes.
    p.sources[0].name = "é".repeat(24) + "x";
    assert_eq!(validate::validate(&p), Vec::new());
    p.sources[0].name = "é".repeat(25);
    only(&validate::validate(&p), codes::NAME_TOO_LONG);
    for bad in ["LPT1", "aux.txt", "a|b", "trailing.", ""] {
        p.sources[0].name = bad.to_string();
        only(&validate::validate(&p), codes::NAME_NOT_FILENAME_SAFE);
    }
}

#[test]
fn transmission_and_diffusion_warnings_do_not_block() {
    let mut p = cube();
    p.materials[0].absorption = [0.0, 1.0, 0.2, 0.2, 0.2, 0.2].map(F64::new).to_vec();
    p.materials[0].transmission_loss_db =
        Some([10.0, 10.0, 10.0, 10.0, 7.0, 6.0].map(F64::new).to_vec());
    let issues = validate::validate(&p);
    // 125 Hz: alpha = 0 cannot transmit. 250 Hz: alpha = 1 with scattering 0.1. 2 kHz: tau =
    // 0.1995 < 0.2 passes; 4 kHz: tau = 0.2512 > 0.2 does not.
    assert_eq!(
        codes_of(&issues),
        vec![
            codes::MATERIAL_DIFFUSION_IGNORED,
            codes::MATERIAL_TRANSMISSION_EXCEEDS_ABSORPTION
        ]
    );
    assert!(!has_errors(&issues));
    assert!(
        issues[1].message.contains("and in 1 other band"),
        "{}",
        issues[1].message
    );
}
