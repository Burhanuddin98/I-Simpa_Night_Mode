//! The validator's negative fixtures, `tests/fixtures/negative/schema/`:
//!
//! - one project file per `project` rule of `docs/solver-contract.md`, `<code>.simpa`: upstream's
//!   5 m cube (`tests/fixtures/projects/cube.simpa`) with one thing broken, listed in
//!   `expected.json` (file name to code). `mesh_out_of_date.context.json` carries the mesh stamp
//!   that fixture is checked against; `directivity/` holds the balloon files the directivity
//!   fixtures name.
//! - one run folder per `export` rule, `export/<code>/`, overlaid on `export/baseline/` (a
//!   hand-written SPPS config for the cube, a TCR one, and upstream's `cube.cbin` and
//!   `cube_mesh.mbin`), listed in `export/expected.json`. `__RUNDIR__` in a config is the run
//!   folder with a trailing separator; `__RUNDIR_NO_SEPARATOR__` is the same without it.
//!
//! Every fixture must yield its code, at the rule's severity, and no other issue at all. The
//! files are written by `write_negative_fixtures` (ignored; run it on purpose) and checked
//! against it byte for byte by `committed_fixtures_match_their_generator`.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

use simpa_core::schema::{
    self, AirAbsorption, AttenuationUnit, DiffusionLaw, Directivity, F64, FittingShape,
    FittingZone, FittingZoneId, MaterialId, MaterialOverride, PointReceiver, PointReceiverId,
    Project, SolverKind, SurfaceReceiver, SurfaceReceiverId, SurfaceReceiverShape, Variant,
    VariantId, Vec3,
};
use simpa_core::validate::{
    self, Context, Issue, RULES, Severity, Stage, mesh_input_hash, severity_of, validate_export,
};

fn repo(rel: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(rel)
}

fn negative_dir() -> PathBuf {
    repo("tests/fixtures/negative/schema")
}

fn export_dir() -> PathBuf {
    negative_dir().join("export")
}

fn cube() -> Project {
    schema::load(&repo("tests/fixtures/projects/cube.simpa")).expect("cube.simpa loads")
}

/// A fixed UUID in the cube fixture's range, for entities the fixtures add.
fn fixed(n: u128) -> u128 {
    0x0c0b_e000_0000_4000_8000_0000_0000_0000 | n
}

// ---------------------------------------------------------------------------------------------
// Generators

/// A directivity balloon in upstream's layout (`spps/tests/speaker-test3.txt`): a
/// `"Frequency"` line per frequency, then 72 rows, φ = 0 to 355, of 37 values, θ = 0 to 180.
/// `trailing` is appended to every data row.
fn directivity_file(frequencies: &[u32], trailing: &str) -> String {
    let mut s = String::from(
        "; Synthetic directivity balloon for the validator's fixtures, written by\r\n\
         ; crates/simpa-core/tests/validate_fixtures.rs in the layout of upstream's\r\n\
         ; spps/tests/speaker-test3.txt.\r\n\
         \"FileType\",\"Speaker Types\"\r\n",
    );
    for f in frequencies {
        s.push_str(&format!("\"Frequency\",{f},\"Hz\"\r\n"));
        for k in 0..72 {
            s.push_str(&format!("\"{:>3}°\"", 5 * k));
            for t in 0..37 {
                s.push_str(&format!(",{}", -(t / 4)));
            }
            s.push_str(trailing);
            s.push_str("\r\n");
        }
    }
    s
}

const OCTAVES: [u32; 6] = [125, 250, 500, 1000, 2000, 4000];

/// The directivity files the fixtures and tests name, relative to the fixture folder.
fn directivity_files() -> Vec<(&'static str, String)> {
    vec![
        (
            "directivity/125_to_4000_hz.txt",
            directivity_file(&OCTAVES, ""),
        ),
        (
            "directivity/125_to_2000_hz.txt",
            directivity_file(&OCTAVES[..5], ""),
        ),
        (
            "directivity/trailing_comma.txt",
            directivity_file(&OCTAVES, ","),
        ),
    ]
}

fn balloon(file: &str) -> Directivity {
    Directivity::Balloon {
        file: file.to_string(),
        direction: Vec3::new(1.0, 0.0, 0.0),
    }
}

/// The cube with its top corner (5, 5, 5) raised by 0.5 m: a mesh built from the plain cube is
/// out of date for it.
fn raised_corner() -> Project {
    let mut p = cube();
    p.geometry.vertices[5] = Vec3::new(5.0, 5.0, 5.5);
    p
}

/// Every project fixture: its rule's code and the cube with that one thing broken.
fn negative_projects() -> Vec<(&'static str, Project)> {
    let mut all: Vec<(&'static str, Project)> = Vec::new();
    let mut add = |code: &'static str, edit: &dyn Fn(&mut Project)| {
        let mut p = cube();
        edit(&mut p);
        all.push((code, p));
    };
    let f64s = |v: &[f64]| v.iter().copied().map(F64::new).collect::<Vec<_>>();

    add("band_set_empty", &|p| {
        p.bands.frequencies_hz.clear();
        p.materials[0].absorption.clear();
        p.materials[0].scattering.clear();
        p.solvers.spps.bands_computed.clear();
        p.solvers.tcr.bands_computed.clear();
    });
    add("band_duplicate", &|p| {
        p.bands.frequencies_hz = vec![125, 250, 500, 1000, 2000, 2000];
    });
    add("band_frequency_not_integer", &|p| {
        p.bands.frequencies_hz[0] = 0;
    });
    add("band_set_mismatch", &|p| {
        p.materials[0].absorption.pop();
    });
    add("material_unassigned", &|p| {
        p.surface_groups[0].material = MaterialId::from_u128(fixed(0x901));
    });
    add("material_value_out_of_range", &|p| {
        p.materials[0].absorption[2] = F64::new(1.2);
    });
    add("material_diffusion_ignored", &|p| {
        p.materials[0].absorption[5] = F64::new(1.0);
    });
    add("material_transmission_exceeds_absorption", &|p| {
        p.materials[0].transmission_loss_db = Some(f64s(&[3.0; 6]));
    });
    add("source_none", &|p| {
        p.sources[0].enabled = false;
    });
    add("source_outside_volume", &|p| {
        p.sources[0].position = Vec3::new(6.0, 1.25, 2.5);
    });
    add("source_near_surface", &|p| {
        p.sources[0].position = Vec3::new(3.0, 1.25, 0.0005);
    });
    add("receiver_outside_volume", &|p| {
        p.point_receivers[0].position = Vec3::new(2.0, 3.75, 5.5);
    });
    add("receiver_sphere_crosses_surface", &|p| {
        p.point_receivers[0].position = Vec3::new(2.0, 3.75, 0.2);
    });
    add("receiver_radius_invalid", &|p| {
        p.solvers.spps.receiver_radius_m = F64::new(0.0);
    });
    add("direction_vector_zero", &|p| {
        p.point_receivers[0].orientation = Vec3::new(0.0, 0.0, 0.0);
    });
    add("directivity_file_missing", &|p| {
        p.sources[0].directivity = balloon("directivity/no_such_file.txt");
    });
    add("directivity_file_invalid", &|p| {
        p.sources[0].directivity = balloon("directivity/trailing_comma.txt");
    });
    add("directivity_band_missing", &|p| {
        p.sources[0].directivity = balloon("directivity/125_to_2000_hz.txt");
    });
    add("time_step_invalid", &|p| {
        p.solvers.spps.time_step_s = F64::new(0.0);
    });
    add("step_count_overflow", &|p| {
        p.solvers.spps.duration_s = F64::new(200.0);
    });
    add("source_delay_invalid", &|p| {
        p.sources[0].delay_s = F64::new(1.5);
    });
    add("trans_epsilon_invalid", &|p| {
        p.solvers.spps.extinction_exponent = F64::new(0.0);
    });
    add("particle_count_invalid", &|p| {
        p.solvers.spps.particles_per_source = 0;
    });
    add("atmosphere_invalid", &|p| {
        p.environment.pressure_pa = F64::new(0.0);
    });
    add("absatmo_invalid", &|p| {
        p.environment.air_absorption = AirAbsorption::UserDefined {
            value: F64::new(-0.01),
            unit: AttenuationUnit::PerMetre,
        };
    });
    add("name_too_long", &|p| {
        p.sources[0].name = "Source with a name longer than the 49 bytes a cell holds".to_string();
    });
    add("name_not_filename_safe", &|p| {
        p.point_receivers[0].name = "Receiver: 1".to_string();
    });
    add("name_duplicate", &|p| {
        p.point_receivers.push(PointReceiver {
            id: PointReceiverId::from_u128(fixed(0x902)),
            name: "receiver 1".to_string(),
            position: Vec3::new(3.0, 3.75, 2.5),
            orientation: Vec3::new(1.0, 0.0, 0.0),
            background_noise: None,
        });
    });
    add("surface_receiver_empty", &|p| {
        p.surface_receivers.push(SurfaceReceiver {
            id: SurfaceReceiverId::from_u128(fixed(0x903)),
            name: "Map 1".to_string(),
            enabled: true,
            shape: SurfaceReceiverShape::Scene { groups: Vec::new() },
        });
    });
    add("cutting_plane_invalid", &|p| {
        p.surface_receivers.push(SurfaceReceiver {
            id: SurfaceReceiverId::from_u128(fixed(0x904)),
            name: "Cut 1".to_string(),
            enabled: true,
            shape: SurfaceReceiverShape::CuttingPlane {
                a: Vec3::new(0.1, 4.9, 2.5),
                b: Vec3::new(0.1, 0.1, 2.5),
                c: Vec3::new(4.9, 0.1, 2.5),
                resolution_m: F64::new(0.0),
            },
        });
    });
    add("fitting_parameters_invalid", &|p| {
        p.fitting_zones.push(FittingZone {
            id: FittingZoneId::from_u128(fixed(0x905)),
            name: "Fitting 1".to_string(),
            enabled: true,
            shape: FittingShape::Box {
                min: Vec3::new(0.5, 0.5, 0.5),
                max: Vec3::new(1.5, 1.5, 1.5),
            },
            absorption: f64s(&[0.1; 6]),
            mean_free_path_m: f64s(&[2.0, 2.0, 0.0, 2.0, 2.0, 2.0]),
            diffusion_law: vec![DiffusionLaw::Uniform; 6],
        });
    });
    add("variant_reference_invalid", &|p| {
        let id = VariantId::from_u128(fixed(0x906));
        p.variants.push(Variant {
            id,
            name: "Variant 1".to_string(),
            overrides: vec![MaterialOverride {
                group: p.surface_groups[0].id,
                material: MaterialId::from_u128(fixed(0x907)),
            }],
        });
        p.active_variant = Some(id);
    });
    add("mesh_out_of_date", &|p| {
        *p = raised_corner();
    });
    all
}

/// The context a project fixture is checked in: its folder, and the mesh stamp from
/// `<stem>.context.json` if there is one.
fn context_for(path: &Path) -> Context {
    let mut ctx = Context::for_project_file(path);
    let sidecar = path.with_extension("context.json");
    if let Ok(text) = std::fs::read_to_string(&sidecar) {
        let value: serde_json::Value = serde_json::from_str(&text).expect("context JSON");
        ctx.mesh_input_hash = value["mesh_input_hash"].as_str().map(str::to_string);
    }
    ctx
}

const BANDS_XML: &str = r#"      <bfreq freq="125" docalc="1"/>
      <bfreq freq="250" docalc="1"/>
      <bfreq freq="500" docalc="1"/>
      <bfreq freq="1000" docalc="1"/>
      <bfreq freq="2000" docalc="1"/>
      <bfreq freq="4000" docalc="1"/>
"#;

/// The lists every baseline config shares: the cube's material, source and receiver.
fn lists_xml() -> String {
    let each = |line: &str| -> String {
        OCTAVES
            .iter()
            .map(|f| line.replace("{f}", &f.to_string()))
            .collect()
    };
    format!(
        r#"  <surface_absorption_enum>
    <type_surface id="0" side_material="1">
{}    </type_surface>
  </surface_absorption_enum>
  <sources>
    <source name="Source 1" x="3" y="1.25" z="2.5" directivite="0" delay="0">
{}    </source>
  </sources>
  <recepteursp>
    <recepteur_ponctuel id="0" lbl="Receiver 1" x="2" y="3.75" z="2.5" u="1" v="0" w="0">
{}    </recepteur_ponctuel>
  </recepteursp>
  <recepteurss/>
  <encombrement_enum/>
</configuration>
"#,
        each("      <bfreq freq=\"{f}\" absorb=\"0.2\" diffusion=\"0.1\" loi=\"2\"/>\n"),
        each("      <bfreq freq=\"{f}\" db=\"82.2184874961636\"/>\n"),
        each("      <bfreq freq=\"{f}\" db=\"0\"/>\n"),
    )
}

const ATMO_XML: &str = r#"  <condition_atmospherique temperature="20" pression="101325" humidite="50" z0="0.02" alog="0" blin="0" disable_absatmo_computation="0" absatmo="0"/>
"#;

/// A hand-written SPPS config for the cube project, every attribute config_xml.md's Writer
/// column asks of SPPS, with the cube's values (90 dB pink is 82.218... dB in each of 6 bands).
fn baseline_spps() -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<configuration workingdirectory="__RUNDIR__">
{ATMO_XML}  <simulation modelName="mesh.cbin" tetrameshFileName="tetramesh.mbin" pasdetemps="0.002" duree_simulation="1" directivities_directory="" recepteurss_directory="Surface receiver\" recepteurss_filename="Sound level.csbin" recepteurss_cut_filename="rs_cut.csbin" receiversp_directory="Punctual receivers" receiversp_filename="Sound level.recp" receiversp_filename_adv="Advanced sound level.gap" cumul_filename="Total energy.recp" particules_directory="Particles\" particules_filename="particles.pbin" stats_filename="SPPS particle statistics.gabe" nbparticules="2000" nbparticules_rendu="0" abs_atmo_calc="1" output_recp_bysource="0" random_seed="1" save_surface_intersection="1" save_receivers_intersection="1" direct_calc="0" enc_calc="1" computation_method="0" rayon_recepteurp="0.31" trans_epsilon="5" trans_calc="1" output_recs_byfreq="1" surf_receiv_method="0">
    <freq_enum>
{BANDS_XML}    </freq_enum>
  </simulation>
{}"#,
        lists_xml()
    )
}

/// The TCR config for the same project: GUI mode, no SPPS settings.
fn baseline_tcr() -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<configuration workingdirectory="__RUNDIR__">
{ATMO_XML}  <simulation modelName="mesh.cbin" tetrameshFileName="tetramesh.mbin" directivities_directory="" recepteurss_directory="Surface receiver\" recepteurss_filename="Sound level.csbin" recepteurss_cut_filename="rs_cut.csbin" receiversp_directory="Punctual receivers" receiversp_filename="Sound level.recp" receiversp_filename_adv="Advanced sound level.gap" cumul_filename="Total energy.recp" abs_atmo_calc="1" output_recs_byfreq="1" direct_recepteurSOutputName="Direct field\" sabine_recepteurSOutputName="Total field (Sabine)\" eyring_recepteurSOutputName="Total field (Eyring)\">
    <freq_enum>
{BANDS_XML}    </freq_enum>
  </simulation>
{}"#,
        lists_xml()
    )
}

/// `baseline` with `from` replaced by `to`, exactly once.
fn edit(baseline: &str, from: &str, to: &str) -> String {
    assert_eq!(baseline.matches(from).count(), 1, "{from:?}");
    baseline.replacen(from, to, 1)
}

/// Every export fixture: its rule's code, its config, and the project it was exported from if
/// that is not the cube.
fn negative_exports() -> Vec<(&'static str, String, Option<Project>)> {
    let spps = baseline_spps();
    let fitting = r#"  <encombrement_enum>
    <encombrement id="1">
      <bfreq freq="125" alpha="0.1" lambda="2" loi_diff="0"/>
      <bfreq freq="250" alpha="0.1" lambda="2" loi_diff="0"/>
      <bfreq freq="500" alpha="0.1" lambda="2" loi_diff="0"/>
      <bfreq freq="1000" alpha="0.1" lambda="2" loi_diff="0"/>
      <bfreq freq="2000" alpha="0.1" lambda="2" loi_diff="0"/>
      <bfreq freq="4000" alpha="0.1" lambda="2" loi_diff="0"/>
    </encombrement>
  </encombrement_enum>
"#;
    let mut with_fitting = cube();
    with_fitting.fitting_zones.push(FittingZone {
        id: FittingZoneId::from_u128(fixed(0x908)),
        name: "Fitting 1".to_string(),
        enabled: true,
        shape: FittingShape::Box {
            min: Vec3::new(0.5, 0.5, 0.5),
            max: Vec3::new(1.5, 1.5, 1.5),
        },
        absorption: [0.1; 6].map(F64::new).to_vec(),
        mean_free_path_m: [2.0; 6].map(F64::new).to_vec(),
        diffusion_law: vec![DiffusionLaw::Uniform; 6],
    });
    vec![
        (
            "working_directory_invalid",
            edit(&spps, "\"__RUNDIR__\"", "\"__RUNDIR_NO_SEPARATOR__\""),
            None,
        ),
        (
            "output_path_too_long",
            edit(
                &spps,
                "receiversp_directory=\"Punctual receivers\"",
                &format!("receiversp_directory=\"{}\"", "R".repeat(240)),
            ),
            None,
        ),
        (
            "config_attribute_missing",
            edit(&spps, " trans_epsilon=\"5\"", ""),
            None,
        ),
        (
            "docalc_not_literal_one",
            edit(
                &spps,
                "<bfreq freq=\"1000\" docalc=\"1\"/>",
                "<bfreq freq=\"1000\" docalc=\"true\"/>",
            ),
            None,
        ),
        (
            "config_value_format",
            edit(
                &spps,
                "rayon_recepteurp=\"0.31\"",
                "rayon_recepteurp=\"0,31\"",
            ),
            None,
        ),
        (
            "solver_id_mapping_invalid",
            edit(&spps, "<type_surface id=\"0\"", "<type_surface id=\"5\""),
            None,
        ),
        (
            "fitting_id_collides_with_room_region",
            edit(&spps, "  <encombrement_enum/>\n", fitting),
            Some(with_fitting),
        ),
    ]
}

/// Every file the generator writes, relative to the fixture folder, with its bytes.
fn generated_files() -> Vec<(String, Vec<u8>)> {
    let mut files: Vec<(String, Vec<u8>)> = Vec::new();
    let mut expected = BTreeMap::new();
    for (code, project) in negative_projects() {
        let name = format!("{code}.simpa");
        files.push((name.clone(), schema::to_json(&project).into_bytes()));
        expected.insert(name, code);
    }
    files.push(("expected.json".to_string(), pretty(&expected).into_bytes()));
    let context = serde_json::json!({ "mesh_input_hash": mesh_input_hash(&cube()) });
    files.push((
        "mesh_out_of_date.context.json".to_string(),
        pretty(&context).into_bytes(),
    ));
    for (name, text) in directivity_files() {
        files.push((name.to_string(), text.into_bytes()));
    }

    files.push((
        "export/baseline/config_spps.xml".to_string(),
        baseline_spps().into_bytes(),
    ));
    files.push((
        "export/baseline/config_tcr.xml".to_string(),
        baseline_tcr().into_bytes(),
    ));
    for (name, upstream) in [
        ("mesh.cbin", "cube.cbin"),
        ("tetramesh.mbin", "cube_mesh.mbin"),
    ] {
        let bytes = std::fs::read(repo("tests/fixtures/upstream/lib_interface").join(upstream))
            .expect("upstream cube fixture");
        files.push((format!("export/baseline/{name}"), bytes));
    }
    let mut export_expected = BTreeMap::new();
    for (code, config, project) in negative_exports() {
        files.push((format!("export/{code}/config.xml"), config.into_bytes()));
        if let Some(p) = project {
            files.push((
                format!("export/{code}/project.simpa"),
                schema::to_json(&p).into_bytes(),
            ));
        }
        export_expected.insert(code.to_string(), code);
    }
    files.push((
        "export/expected.json".to_string(),
        pretty(&export_expected).into_bytes(),
    ));
    files
}

/// Pretty JSON with a final newline, for the generator's maps.
fn pretty<T: serde::Serialize>(value: &T) -> String {
    serde_json::to_string_pretty(value).expect("JSON") + "\n"
}

#[test]
#[ignore = "rewrites the committed fixtures; run it on purpose after changing the generator"]
fn write_negative_fixtures() {
    for (rel, bytes) in generated_files() {
        let path = negative_dir().join(&rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, bytes).unwrap();
    }
}

#[test]
fn committed_fixtures_match_their_generator() {
    let files = generated_files();
    for (rel, bytes) in &files {
        let on_disk = std::fs::read(negative_dir().join(rel))
            .unwrap_or_else(|e| panic!("{rel}: {e}; run write_negative_fixtures"));
        assert!(on_disk == *bytes, "{rel} differs from its generator");
    }
    println!("{} fixture files match their generator", files.len());
}

// ---------------------------------------------------------------------------------------------
// Project fixtures

/// Checks one fixture's issues: its code at the rule's severity, and nothing else.
fn judge(label: &str, code: &str, issues: &[Issue]) -> Result<(), String> {
    let want = severity_of(code);
    let hit = issues.iter().any(|i| i.code == code && i.severity == want);
    let others: Vec<&Issue> = issues.iter().filter(|i| i.code != code).collect();
    if hit && others.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "{label}: expected only {code} ({want:?}), got {:#?}",
            issues
        ))
    }
}

#[test]
fn every_project_fixture_yields_exactly_its_code() {
    let dir = negative_dir();
    let expected: BTreeMap<String, String> =
        serde_json::from_str(&std::fs::read_to_string(dir.join("expected.json")).unwrap()).unwrap();
    let mut failures = Vec::new();
    let mut matched = 0;
    for (file, code) in &expected {
        let path = dir.join(file);
        let project =
            validate::read_project(&path).unwrap_or_else(|e| panic!("{file}: {e} ({})", e.code()));
        let issues = validate::validate_with(&project, &context_for(&path));
        match judge(file, code, &issues) {
            Ok(()) => {
                matched += 1;
                let first = issues.iter().find(|i| i.code == code).unwrap();
                println!("{file}: {code} at {}: {}", first.path, first.message);
            }
            Err(e) => failures.push(e),
        }
    }
    println!("project-stage {matched}/{} matched", expected.len());
    assert!(failures.is_empty(), "{}", failures.join("\n"));

    // One fixture per project rule, and nothing else in the table.
    let mut codes: Vec<&str> = expected.values().map(String::as_str).collect();
    codes.sort_unstable();
    let mut rules: Vec<&str> = RULES
        .iter()
        .filter(|r| r.stage == Stage::Project)
        .map(|r| r.code)
        .collect();
    rules.sort_unstable();
    assert_eq!(codes, rules);
    assert!(expected.len() >= 20);
    assert!(
        expected.iter().all(|(f, c)| *f == format!("{c}.simpa")),
        "each file is named after its code"
    );
}

#[test]
fn the_structural_fixtures_are_refused_by_the_ordinary_loader() {
    // Why the validator reads fixtures with read_project: from_json refuses these six outright,
    // with the schema's integrity code instead of the contract's.
    for (code, integrity) in [
        ("band_set_empty", "bands"),
        ("band_duplicate", "bands"),
        ("band_frequency_not_integer", "bands"),
        ("band_set_mismatch", "band_count"),
        ("material_unassigned", "dangling_reference"),
        ("variant_reference_invalid", "dangling_reference"),
    ] {
        let path = negative_dir().join(format!("{code}.simpa"));
        match schema::load(&path) {
            Err(schema::LoadError::Integrity(e)) => assert_eq!(e.code(), integrity, "{code}"),
            other => panic!("{code}: {other:?}"),
        }
    }
}

#[test]
fn a_complete_directivity_file_and_the_current_mesh_pass() {
    let dir = negative_dir();
    let mut p = cube();
    p.sources[0].directivity = balloon("directivity/125_to_4000_hz.txt");
    let ctx = Context {
        project_dir: Some(dir.clone()),
        mesh_input_hash: Some(mesh_input_hash(&p)),
    };
    assert_eq!(validate::validate_with(&p, &ctx), Vec::new());

    // Without a project folder the relative file cannot be found.
    let issues = validate::validate(&p);
    assert_eq!(issues.len(), 1, "{issues:#?}");
    assert_eq!(issues[0].code, "directivity_file_missing");

    // An absolute path needs no folder.
    p.sources[0].directivity =
        balloon(dir.join("directivity/125_to_4000_hz.txt").to_str().unwrap());
    assert_eq!(validate::validate(&p), Vec::new());

    // The raised corner changes the mesh stamp; materials and names do not.
    let mut q = cube();
    assert_eq!(mesh_input_hash(&q), mesh_input_hash(&cube()));
    q.materials[0].absorption[0] = F64::new(0.5);
    q.point_receivers[0].name = "Other".to_string();
    assert_eq!(mesh_input_hash(&q), mesh_input_hash(&cube()));
    assert_ne!(mesh_input_hash(&raised_corner()), mesh_input_hash(&cube()));
}

// ---------------------------------------------------------------------------------------------
// Export fixtures

static RUN: AtomicUsize = AtomicUsize::new(0);

/// A fresh run folder under cargo's test scratch space, holding the baseline meshes and
/// `config` as `config.xml` with its placeholders filled in. Nothing is removed afterwards:
/// `target/` is build output.
fn run_folder(label: &str, config: &str) -> PathBuf {
    let n = RUN.fetch_add(1, Ordering::SeqCst);
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join("validate-export")
        .join(format!("{label}-{}-{n}-{stamp}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    for name in ["mesh.cbin", "tetramesh.mbin"] {
        std::fs::copy(export_dir().join("baseline").join(name), dir.join(name)).unwrap();
    }
    let path = dir.to_str().unwrap().to_string();
    let sep = std::path::MAIN_SEPARATOR;
    let text = config
        .replace("__RUNDIR_NO_SEPARATOR__", &path)
        .replace("__RUNDIR__", &format!("{path}{sep}"));
    std::fs::write(dir.join("config.xml"), text).unwrap();
    dir
}

fn read(rel: &str) -> String {
    std::fs::read_to_string(export_dir().join(rel)).unwrap()
}

#[test]
fn the_baseline_run_folders_are_clean() {
    let spps = run_folder("baseline_spps", &read("baseline/config_spps.xml"));
    assert_eq!(
        validate_export(&cube(), &spps, SolverKind::Spps),
        Vec::new()
    );
    let tcr = run_folder("baseline_tcr", &read("baseline/config_tcr.xml"));
    assert_eq!(validate_export(&cube(), &tcr, SolverKind::Tcr), Vec::new());

    // Each baseline is incomplete for the other solver.
    let crossed = validate_export(&cube(), &tcr, SolverKind::Spps);
    assert!(
        !crossed.is_empty() && crossed.iter().all(|i| i.code == "config_attribute_missing"),
        "{crossed:#?}"
    );
    let crossed = validate_export(&cube(), &spps, SolverKind::Tcr);
    assert_eq!(crossed.len(), 3, "{crossed:#?}");
}

#[test]
fn every_export_fixture_yields_exactly_its_code() {
    let expected: BTreeMap<String, String> = serde_json::from_str(&read("expected.json")).unwrap();
    let mut failures = Vec::new();
    let mut matched = 0;
    for (folder, code) in &expected {
        let dir = run_folder(folder, &read(&format!("{folder}/config.xml")));
        let project_file = export_dir().join(folder).join("project.simpa");
        let project = if project_file.exists() {
            validate::read_project(&project_file).unwrap()
        } else {
            cube()
        };
        let issues = validate_export(&project, &dir, SolverKind::Spps);
        match judge(folder, code, &issues) {
            Ok(()) => {
                matched += 1;
                let first = issues.iter().find(|i| i.code == code).unwrap();
                println!(
                    "export/{folder}: {code} at {}: {}",
                    first.path, first.message
                );
            }
            Err(e) => failures.push(e),
        }
    }
    println!("export-stage {matched}/{} matched", expected.len());
    assert!(failures.is_empty(), "{}", failures.join("\n"));
    let mut codes: Vec<&str> = expected.values().map(String::as_str).collect();
    codes.sort_unstable();
    let mut rules: Vec<&str> = RULES
        .iter()
        .filter(|r| r.stage == Stage::Export)
        .map(|r| r.code)
        .collect();
    rules.sort_unstable();
    assert_eq!(codes, rules);
}

#[test]
fn a_reused_run_folder_is_not_fresh() {
    let dir = run_folder("reused", &read("baseline/config_spps.xml"));
    std::fs::create_dir(dir.join("Punctual receivers")).unwrap();
    let issues = validate_export(&cube(), &dir, SolverKind::Spps);
    assert_eq!(issues.len(), 1, "{issues:#?}");
    assert_eq!(issues[0].code, "working_directory_invalid");
    assert!(
        issues[0].message.contains("not fresh"),
        "{}",
        issues[0].message
    );
}

/// Runs a solver in `dir` as the run contract launches it: cwd = the run folder, one argument.
fn run_solver(exe: &Path, dir: &Path) -> (bool, String, String) {
    let out = Command::new(exe)
        .arg("config.xml")
        .current_dir(dir)
        .output()
        .unwrap();
    (
        out.status.success(),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

#[test]
fn the_real_solvers_read_every_attribute_of_the_baselines() {
    // Grace-local: the M1 solver build. Skipped where it is absent (CI).
    let bin = repo("target/solvers/bin");
    let (spps_exe, tcr_exe) = (bin.join("spps.exe"), bin.join("classicalTheory.exe"));
    if !spps_exe.exists() || !tcr_exe.exists() {
        println!("skipped: no solver build at {}", bin.display());
        return;
    }
    let spps = run_folder("solver_spps", &read("baseline/config_spps.xml"));
    assert_eq!(
        validate_export(&cube(), &spps, SolverKind::Spps),
        Vec::new()
    );
    let (ok, stdout, stderr) = run_solver(&spps_exe, &spps);
    let xml_lines = stdout
        .lines()
        .filter(|l| l.starts_with("Xml Property"))
        .count();
    println!("SPPS: exit ok {ok}, {xml_lines} 'Xml Property' lines, stderr {stderr:?}");
    assert!(ok, "{stdout}\n{stderr}");
    assert_eq!(xml_lines, 0, "{stdout}");
    assert!(
        stdout.lines().any(|l| l == "End of calculation."),
        "{stdout}"
    );

    let tcr = run_folder("solver_tcr", &read("baseline/config_tcr.xml"));
    assert_eq!(validate_export(&cube(), &tcr, SolverKind::Tcr), Vec::new());
    let (ok, stdout, stderr) = run_solver(&tcr_exe, &tcr);
    let xml_lines = stdout
        .lines()
        .filter(|l| l.starts_with("Xml Property"))
        .count();
    println!("TCR: exit ok {ok}, {xml_lines} 'Xml Property' lines, stderr {stderr:?}");
    assert!(ok, "{stdout}\n{stderr}");
    assert_eq!(xml_lines, 0, "{stdout}");
    assert!(tcr.join("Main results.gabe").exists());

    // And the one the validator refuses for a missing attribute prints exactly that line.
    let missing = run_folder(
        "solver_missing",
        &read("config_attribute_missing/config.xml"),
    );
    let issues = validate_export(&cube(), &missing, SolverKind::Spps);
    assert_eq!(issues.len(), 1);
    let (_, stdout, _) = run_solver(&spps_exe, &missing);
    let xml: Vec<&str> = stdout
        .lines()
        .filter(|l| l.starts_with("Xml Property"))
        .collect();
    println!("SPPS without trans_epsilon: {xml:?}");
    assert_eq!(xml, vec!["Xml Property trans_epsilon doesn't exist !"]);
}

#[test]
fn severities_follow_the_rule_table() {
    for r in RULES {
        assert_eq!(severity_of(r.code), r.severity);
    }
    assert_eq!(severity_of("face_vertex"), Severity::Error);
    let issue = Issue {
        code: "source_none",
        severity: Severity::Error,
        message: "no source is enabled".to_string(),
        path: "/sources".to_string(),
    };
    assert_eq!(
        issue.to_string(),
        "error source_none at /sources: no source is enabled"
    );
    assert_eq!(
        serde_json::to_string(&issue).unwrap(),
        r#"{"code":"source_none","severity":"error","message":"no source is enabled","path":"/sources"}"#
    );
}
