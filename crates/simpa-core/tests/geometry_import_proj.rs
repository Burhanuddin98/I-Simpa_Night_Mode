//! `core::geometry::import::import_proj` against upstream's own tutorial projects: M4 gate items
//! (b) and (c), the committed room fixtures, the embedded reference database, and the zip reader.
//!
//! These read upstream's checkout (`B:\repos\I-Simpa-upstream` on Grace, or
//! `$SIMPA_UPSTREAM`), which CI does not have: without it each test says so and passes, unless
//! `SIMPA_REQUIRE_UPSTREAM=1` is set (the M4 gate sets it), when a missing checkout fails. The
//! same gate numbers are also asserted on the committed fixtures by `geometry_import_rooms.rs`,
//! which needs nothing outside the repo.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use simpa_core::config_xml::{import_upstream, widen_f32};
use simpa_core::geometry::import::proj::{FaceRef, read_finfo, read_scene_mesh};
use simpa_core::geometry::import::zip::{Archive, crc32};
use simpa_core::geometry::import::{
    ImportError, REFERENCE_MATERIALS, REFERENCE_SPECTRA, import_proj, import_proj_file,
};
use simpa_core::schema::{self, Directivity, Project, SurfaceReceiverShape, Vec3};

fn upstream_root() -> PathBuf {
    std::env::var_os("SIMPA_UPSTREAM")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(r"B:\repos\I-Simpa-upstream"))
}

/// A file of upstream's checkout, or `None` (and a note) when the checkout is absent and not
/// required.
fn upstream(rel: &str) -> Option<PathBuf> {
    let p = upstream_root().join(rel);
    if p.exists() {
        return Some(p);
    }
    if std::env::var_os("SIMPA_REQUIRE_UPSTREAM").is_some() {
        panic!(
            "SIMPA_REQUIRE_UPSTREAM is set but {} is missing",
            p.display()
        );
    }
    eprintln!("upstream checkout not found ({}); skipping", p.display());
    None
}

const TUTORIAL1: &str = r"src/isimpa/resources/doc/tutorial/tutorial 1/tutorial_1.proj";
const TUTORIAL2: &str = r"src/isimpa/resources/doc/tutorial/tutorial 2/tutorial_2.proj";
const TUTORIAL3: &str = r"src/isimpa/resources/doc/tutorial/tutorial 3/tutorial_3.proj";
const INDUSTRIAL: &str = r"src/isimpa/resources/doc/tutorial/tutorial 3/Industrial.proj";
const APPCONST: &str = r"src/isimpa/resources/appconst.xml";

fn repo_file(rel: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(rel)
}

const BOX_FIXTURE: &str = "tests/fixtures/rooms/tutorial1_box.simpa";
const ELMIA_FIXTURE: &str = "tests/fixtures/rooms/elmia_corrected.simpa";

/// Total area of every face, m2.
fn area(p: &Project) -> f64 {
    p.geometry
        .faces
        .iter()
        .map(|f| {
            let [a, b, c] = f
                .vertices
                .map(|v| p.geometry.vertices[v as usize].to_array());
            let u = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
            let v = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
            let n = [
                u[1] * v[2] - u[2] * v[1],
                u[2] * v[0] - u[0] * v[2],
                u[0] * v[1] - u[1] * v[0],
            ];
            0.5 * (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt()
        })
        .sum()
}

/// Signed volume enclosed by the faces (divergence theorem), m3.
fn signed_volume(p: &Project) -> f64 {
    p.geometry
        .faces
        .iter()
        .map(|f| {
            let [a, b, c] = f
                .vertices
                .map(|v| p.geometry.vertices[v as usize].to_array());
            (a[0] * (b[1] * c[2] - b[2] * c[1]) - a[1] * (b[0] * c[2] - b[2] * c[0])
                + a[2] * (b[0] * c[1] - b[1] * c[0]))
                / 6.0
        })
        .sum()
}

/// Face indices per surface-group name.
fn faces_by_group(p: &Project) -> BTreeMap<String, BTreeSet<usize>> {
    let mut out: BTreeMap<String, BTreeSet<usize>> = BTreeMap::new();
    for (i, f) in p.geometry.faces.iter().enumerate() {
        let name = p.group(f.group).unwrap().name.clone();
        out.entry(name).or_default().insert(i);
    }
    out
}

/// The project the tutorial 1 fixture holds: `tutorial_1.proj` imported, then named.
fn tutorial1_box(proj: &Path) -> Project {
    let mut p = import_proj_file(proj).unwrap().project;
    p.name = "Tutorial 1 box".into();
    p.description = "Upstream I-Simpa tutorial 1 (tutorial_1.proj at 929a5c8), imported by \
                     geometry::import::import_proj: a 6 x 10 x 3 m box, groups and materials \
                     from its .finfo lists and idmat, sources and receivers from \
                     projet_config.xml."
        .into();
    p
}

/// The project the corrected Elmia fixture holds: `tutorial_2.proj` imported, then named.
fn elmia_corrected(proj: &Path) -> Project {
    let mut p = import_proj_file(proj).unwrap().project;
    p.name = "Elmia (corrected)".into();
    p.description = "Upstream I-Simpa tutorial 2 (tutorial_2.proj at 929a5c8), imported by \
                     geometry::import::import_proj: the Elmia hall as upstream's scene correction \
                     left it, each face in the group its .finfo list puts it in, with that \
                     group's idmat. Replaces Night Mode's testdata/elmia_corrected.ply, whose \
                     centroid-guessed grouping is wrong for 2,438 of 7,860 faces."
        .into();
    p
}

fn assert_near(a: f64, b: f64, tol: f64, what: &str) {
    assert!((a - b).abs() <= tol, "{what}: {a} vs {b} (tolerance {tol})");
}

fn vec3_near(v: Vec3, want: [f64; 3], what: &str) {
    for (k, (a, b)) in v.to_array().iter().zip(want).enumerate() {
        assert_near(*a, b, 1e-12, &format!("{what}[{k}]"));
    }
}

// ---------------------------------------------------------------------------------------------
// Gate (b): tutorial 2, the corrected Elmia hall.

#[test]
fn gate_b_tutorial2_is_the_corrected_hall_grouped_by_its_finfo_lists() {
    let Some(path) = upstream(TUTORIAL2) else {
        return;
    };
    let imported = import_proj_file(&path).unwrap();
    let p = &imported.project;
    let r = &imported.report;
    println!("report: {r:#?}");
    assert_eq!(p.geometry.vertices.len(), 3926, "vertices");
    assert_eq!(p.geometry.faces.len(), 7860, "faces");
    assert_eq!(p.surface_groups.len(), 10, "groups");
    assert_eq!(r.welded_vertices, 0);
    assert_eq!(r.unused_vertices, 0);
    assert_eq!(r.face_record_bytes, Some(32));
    assert_eq!(r.scene_mesh_version, (1, 2));

    // Independently of the importer's grouping: read each group's .finfo list from the
    // archive and check every face's group against it.
    let bytes = std::fs::read(&path).unwrap();
    let archive = Archive::parse(&bytes).unwrap();
    let xml = String::from_utf8(archive.read("instance1/projet_config.xml").unwrap()).unwrap();
    let doc = roxmltree::Document::parse(&xml).unwrap();
    let mut listed: BTreeMap<String, BTreeSet<usize>> = BTreeMap::new();
    let mut idmat: BTreeMap<String, u32> = BTreeMap::new();
    for gr in doc
        .descendants()
        .filter(|n| n.has_tag_name("sgroupes"))
        .flat_map(|s| s.children().filter(|c| c.has_tag_name("gr")))
    {
        let name = gr.attribute("name").unwrap().to_string();
        let file = gr.attribute("facesFile").unwrap();
        let refs = read_finfo(&archive.read(&format!("instance1/{file}")).unwrap()).unwrap();
        // Tutorial 2's scene mesh has one mesh group, so the face index is the project's.
        assert!(refs.iter().all(|r| r.group == 0));
        listed.insert(name.clone(), refs.iter().map(|r| r.face as usize).collect());
        let id = gr
            .children()
            .find(|c| c.attribute("name") == Some("idmat"))
            .unwrap()
            .attribute("value")
            .unwrap();
        idmat.insert(name, id.parse().unwrap());
    }
    assert_eq!(listed.values().map(BTreeSet::len).sum::<usize>(), 7860);
    let mut agree = 0;
    for (i, f) in p.geometry.faces.iter().enumerate() {
        let g = p.group(f.group).unwrap();
        if listed[&g.name].contains(&i) {
            agree += 1;
        }
    }
    println!("face group equal to its .finfo group: {agree}/7860");
    assert_eq!(agree, 7860);
    // Each group carries its idmat's material.
    for g in &p.surface_groups {
        let m = p.material(g.material).unwrap();
        assert_eq!(m.solver_id, Some(idmat[&g.name]), "group {}", g.name);
    }

    let a = area(p);
    println!("total area {a:.4} m2");
    assert_near(a, 4001.8, 0.1, "total area");

    // What else the file holds.
    assert_eq!(p.sources.len(), 3);
    assert_eq!(p.point_receivers.len(), 6);
    assert_eq!(p.surface_receivers.len(), 2);
    for s in &p.surface_receivers {
        assert!(matches!(s.shape, SurfaceReceiverShape::CuttingPlane { .. }));
    }
    let r01 = p.point_receivers.iter().find(|r| r.name == "R01").unwrap();
    vec3_near(r01.position, [13.8, 0.0, 1.45], "R01");
    // The SPPS bands: 125 Hz to 4 kHz octaves only.
    let computed: Vec<u32> = p
        .bands
        .frequencies_hz
        .iter()
        .zip(&p.solvers.spps.bands_computed)
        .filter(|(_, on)| **on)
        .map(|(f, _)| *f)
        .collect();
    assert_eq!(computed, vec![125, 250, 500, 1000, 2000, 4000]);
    // 15 user materials in the project's database, 10 of them used.
    assert_eq!(p.materials.len(), 15);
}

// ---------------------------------------------------------------------------------------------
// Gate (c): tutorial 1, the box.

#[test]
fn gate_c_tutorial1_is_the_six_by_ten_by_three_box() {
    let Some(path) = upstream(TUTORIAL1) else {
        return;
    };
    let imported = import_proj_file(&path).unwrap();
    let p = &imported.project;
    let r = &imported.report;
    println!("report: {r:#?}");
    assert_eq!(
        r.source_vertices, 36,
        "upstream stores a copy of each vertex per face"
    );
    assert_eq!(r.welded_vertices, 28);
    assert_eq!(p.geometry.vertices.len(), 8, "vertices");
    assert_eq!(p.geometry.faces.len(), 12, "faces");

    let groups = faces_by_group(p);
    let want: BTreeMap<String, BTreeSet<usize>> = [
        ("Ceiling", vec![10, 11]),
        ("Floor", vec![0, 1]),
        ("Walls", (2..=9).collect()),
    ]
    .into_iter()
    .map(|(n, f)| (n.to_string(), f.into_iter().collect()))
    .collect();
    assert_eq!(groups, want);
    // Materials: the reference materials the groups' idmat names.
    for (name, id, absorption) in [("Ceiling", 21, 0.3), ("Floor", 25, 0.1), ("Walls", 22, 0.2)] {
        let g = p.surface_groups.iter().find(|g| g.name == name).unwrap();
        let m = p.material(g.material).unwrap();
        assert_eq!(m.solver_id, Some(id), "{name}");
        assert!(m.absorption.iter().all(|a| a.get() == absorption), "{name}");
    }

    // The scene surface receiver covers faces {0, 1}.
    assert_eq!(p.surface_receivers.len(), 1);
    let rs = &p.surface_receivers[0];
    assert_eq!(rs.name, "Receiver");
    let SurfaceReceiverShape::Scene { groups: rs_groups } = &rs.shape else {
        panic!("not a scene receiver");
    };
    let covered: BTreeSet<usize> = p
        .geometry
        .faces
        .iter()
        .enumerate()
        .filter(|(_, f)| rs_groups.contains(&f.group))
        .map(|(i, _)| i)
        .collect();
    assert_eq!(covered, BTreeSet::from([0, 1]));

    let v = signed_volume(p);
    println!("signed volume {v:.6} m3, area {:.6} m2", area(p));
    assert_near(v.abs(), 180.0, 5e-4, "volume");
    assert_near(area(p), 216.0, 1e-9, "area");

    assert_eq!(p.sources.len(), 1);
    vec3_near(p.sources[0].position, [3.0, 5.0, 1.8], "source");
    assert_eq!(p.sources[0].directivity, Directivity::Omni);
    let mut receivers: Vec<(String, [f64; 3])> = p
        .point_receivers
        .iter()
        .map(|r| (r.name.clone(), r.position.to_array()))
        .collect();
    receivers.sort_by(|a, b| a.0.cmp(&b.0));
    assert_eq!(
        receivers,
        vec![
            ("Receiver 1".to_string(), [1.0, 1.0, 1.8]),
            ("Receiver 2".to_string(), [3.0, 7.0, 1.8]),
        ]
    );
}

/// Tutorial 1 carries the configuration upstream's GUI wrote for its SPPS run
/// (`report/SPPS/2019-06-07_11h58m41s/config.xml`). Importing that file with `config_xml`'s
/// importer must give the same materials, sources, receivers, environment and SPPS settings as
/// reading the project's element tree: that is the check that this importer reads the tree the
/// way upstream's GUI writes it.
#[test]
fn tutorial1_reads_as_upstreams_gui_wrote_its_spps_run() {
    let Some(path) = upstream(TUTORIAL1) else {
        return;
    };
    let bytes = std::fs::read(&path).unwrap();
    let archive = Archive::parse(&bytes).unwrap();
    let config = String::from_utf8(
        archive
            .read("instance2/report/SPPS/2019-06-07_11h58m41s/config.xml")
            .unwrap(),
    )
    .unwrap();
    let written = import_upstream(&config).unwrap();
    let ours = import_proj(&bytes).unwrap().project;

    assert_eq!(ours.bands, written.bands);
    assert_eq!(ours.environment, written.environment);
    assert_eq!(ours.solvers.spps, written.solvers.spps);
    // Materials, by solver id. The GUI writes the default material 0 as well, which no group
    // uses; ours holds only the materials in use.
    for m in &ours.materials {
        let w = written
            .materials
            .iter()
            .find(|w| w.solver_id == m.solver_id)
            .unwrap_or_else(|| panic!("material {:?} not written", m.solver_id));
        assert_eq!(
            (
                &m.absorption,
                &m.scattering,
                m.reflection_law,
                &m.transmission_loss_db,
                m.double_sided
            ),
            (
                &w.absorption,
                &w.scattering,
                w.reflection_law,
                &w.transmission_loss_db,
                w.double_sided
            ),
            "material {:?}",
            m.solver_id
        );
    }
    // Sources: same position, power per band (as the solver's f32), directivity and delay.
    assert_eq!(ours.sources.len(), written.sources.len());
    for (a, b) in ours.sources.iter().zip(&written.sources) {
        assert_eq!(a.name, b.name);
        assert_eq!(a.position, b.position);
        assert_eq!(a.directivity, b.directivity);
        assert_eq!(a.delay_s, b.delay_s);
        let la: Vec<f32> = a
            .power
            .band_levels_db(&ours.bands)
            .unwrap()
            .iter()
            .map(|&v| v as f32)
            .collect();
        let lb: Vec<f32> = b
            .power
            .band_levels_db(&written.bands)
            .unwrap()
            .iter()
            .map(|&v| v as f32)
            .collect();
        assert_eq!(la, lb, "source {} power", a.name);
    }
    // Receivers, by name (the GUI writes them in another order).
    assert_eq!(ours.point_receivers.len(), written.point_receivers.len());
    for a in &ours.point_receivers {
        let b = written
            .point_receivers
            .iter()
            .find(|b| b.name == a.name)
            .unwrap();
        assert_eq!(a.position, b.position, "{}", a.name);
        // The GUI computes a receiver's direction from its orientation point when a position
        // changes (`E_Scene_Recepteursp_Recepteur::Modified`) and saves it in the project file to
        // 6 significant digits (`-0.436852`); the run's config.xml was written in the session
        // that computed it, at full precision (`-0.436852067708969`). The file's value is what
        // upstream holds after reopening the project, so that is what is imported.
        for (x, y) in a
            .orientation
            .to_array()
            .iter()
            .zip(b.orientation.to_array())
        {
            assert!((x - y).abs() < 1e-6, "{} orientation {x} vs {y}", a.name);
        }
        // Upstream adds its float offset to reference levels rounded to 2 decimals; ours is the
        // exact white shape at the same global level. They agree to one f32 unit in the last
        // place (5e-7 dB here), not always exactly.
        let la: Vec<f32> = a
            .background_noise
            .as_ref()
            .unwrap()
            .band_levels_db(&ours.bands)
            .unwrap()
            .iter()
            .map(|&v| v as f32)
            .collect();
        let lb: Vec<f32> = b
            .background_noise
            .as_ref()
            .unwrap()
            .band_levels_db(&written.bands)
            .unwrap()
            .iter()
            .map(|&v| v as f32)
            .collect();
        let ulps: Vec<u32> = la
            .iter()
            .zip(&lb)
            .map(|(x, y)| x.to_bits().abs_diff(y.to_bits()))
            .collect();
        println!(
            "receiver {} background noise: {} of {} bands differ, by at most {} ulp",
            a.name,
            ulps.iter().filter(|&&u| u > 0).count(),
            ulps.len(),
            ulps.iter().max().unwrap()
        );
        assert!(
            ulps.iter().all(|&u| u <= 1),
            "receiver {} background noise: {la:?} vs {lb:?}",
            a.name
        );
    }
    assert_eq!(
        ours.surface_receivers
            .iter()
            .map(|r| r.name.clone())
            .collect::<Vec<_>>(),
        written
            .surface_receivers
            .iter()
            .map(|r| r.name.clone())
            .collect::<Vec<_>>()
    );
}

// ---------------------------------------------------------------------------------------------
// The committed fixtures.

#[test]
#[ignore]
fn write_room_fixtures() {
    let (Some(t1), Some(t2)) = (upstream(TUTORIAL1), upstream(TUTORIAL2)) else {
        panic!("the fixtures are regenerated from upstream's checkout");
    };
    std::fs::create_dir_all(repo_file("tests/fixtures/rooms")).unwrap();
    schema::save(&tutorial1_box(&t1), &repo_file(BOX_FIXTURE)).unwrap();
    schema::save(&elmia_corrected(&t2), &repo_file(ELMIA_FIXTURE)).unwrap();
}

#[test]
fn room_fixtures_are_the_import_of_upstreams_tutorials() {
    let (Some(t1), Some(t2)) = (upstream(TUTORIAL1), upstream(TUTORIAL2)) else {
        return;
    };
    for (rel, project) in [
        (BOX_FIXTURE, tutorial1_box(&t1)),
        (ELMIA_FIXTURE, elmia_corrected(&t2)),
    ] {
        let on_disk = std::fs::read_to_string(repo_file(rel)).unwrap();
        assert!(
            on_disk == schema::to_json(&project),
            "{rel} is not the import of upstream's tutorial; regenerate with --ignored \
             write_room_fixtures"
        );
    }
}

// ---------------------------------------------------------------------------------------------
// Refusals, the reference database and the zip reader.

#[test]
fn tutorial3_projects_are_refused_by_name_for_their_fitting_zones() {
    for rel in [TUTORIAL3, INDUSTRIAL] {
        let Some(path) = upstream(rel) else {
            return;
        };
        let e = import_proj_file(&path).unwrap_err();
        println!("{rel}: {} ({e})", e.code());
        assert!(matches!(e, ImportError::Unsupported { .. }), "{e}");
        assert!(e.to_string().contains("fitting zones and volumes"), "{e}");
    }
}

#[test]
fn reference_database_matches_upstreams_appconst() {
    let Some(path) = upstream(APPCONST) else {
        return;
    };
    let xml = std::fs::read_to_string(path).unwrap();
    let doc = roxmltree::Document::parse(&xml).unwrap();
    let f32_of = |t: &str| t.trim().parse::<f64>().unwrap() as f32;
    let mats: Vec<_> = doc
        .descendants()
        .filter(|n| n.has_tag_name("appmateriau"))
        .collect();
    assert_eq!(mats.len(), REFERENCE_MATERIALS.len());
    for (m, r) in mats.iter().zip(&REFERENCE_MATERIALS) {
        let spectrum = m.children().find(|c| c.has_tag_name("materiau")).unwrap();
        assert_eq!(spectrum.attribute("idmateriau").unwrap(), r.id.to_string());
        assert_eq!(m.attribute("name").unwrap(), r.name);
        let rows: Vec<_> = spectrum.children().filter(|c| c.is_element()).collect();
        assert_eq!(rows.len(), 27);
        for row in rows {
            let v = |n: &str| {
                let p = row
                    .children()
                    .find(|c| c.attribute("name") == Some(n))
                    .unwrap();
                p.attribute("value")
                    .or(p.attribute("choice"))
                    .unwrap()
                    .to_string()
            };
            assert_eq!(f32_of(&v("absorb")), r.absorption, "{}", r.name);
            assert_eq!(f32_of(&v("diffusion")), 0.0);
            assert_eq!(v("loi"), "0");
            assert_eq!(v("transmission"), "0");
        }
        let color = m
            .descendants()
            .find(|c| c.attribute("name") == Some("mat_color"))
            .unwrap();
        let rgb = ["r", "g", "b"].map(|a| color.attribute(a).unwrap().parse::<u8>().unwrap());
        assert_eq!(rgb, r.color);
        let side = m
            .descendants()
            .find(|c| c.attribute("name") == Some("side_material"))
            .unwrap();
        assert_eq!(side.attribute("choice"), Some("1"));
    }
    let spectra: Vec<_> = doc
        .descendants()
        .filter(|n| n.has_tag_name("appspectrums"))
        .flat_map(|s| s.children().filter(|c| c.is_element()))
        .collect();
    assert_eq!(spectra.len(), REFERENCE_SPECTRA.len());
    for (s, r) in spectra.iter().zip(&REFERENCE_SPECTRA) {
        assert_eq!(s.attribute("idspectre").unwrap(), r.id.to_string());
        assert_eq!(s.attribute("name").unwrap(), r.name);
        let mut rows: Vec<(u32, f32)> = s
            .children()
            .filter(|c| c.is_element() && c.attribute("name") != Some("cumul"))
            .map(|row| {
                let db = row
                    .children()
                    .find(|c| c.attribute("name") == Some("db"))
                    .unwrap();
                (
                    row.attribute("name").unwrap().parse().unwrap(),
                    f32_of(db.attribute("value").unwrap()),
                )
            })
            .collect();
        rows.sort_by_key(|r| r.0);
        let levels: Vec<f32> = rows.iter().map(|r| r.1).collect();
        assert_eq!(levels, r.band_db.to_vec(), "{}", r.name);
    }
    // The widened values are the shortest decimals of those f32.
    assert_eq!(widen_f32(REFERENCE_SPECTRA[3].band_db[26]), -6.86);
}

#[test]
fn every_entry_of_every_tutorial_archive_inflates_to_its_crc() {
    let mut total = (0usize, 0u64);
    for rel in [TUTORIAL1, TUTORIAL2, TUTORIAL3, INDUSTRIAL] {
        let Some(path) = upstream(rel) else {
            return;
        };
        let bytes = std::fs::read(&path).unwrap();
        let archive = Archive::parse(&bytes).unwrap();
        for e in archive.entries() {
            let data = archive.read_entry(e).unwrap();
            assert_eq!(data.len() as u64, e.size, "{}", e.name);
            assert_eq!(crc32(&data), e.crc32, "{}", e.name);
            total.0 += 1;
            total.1 += e.size;
        }
    }
    println!(
        "{} entries, {} bytes inflated, every CRC-32 equal",
        total.0, total.1
    );
}

#[test]
fn finfo_reads_both_count_widths_and_checks_the_count() {
    // Written on Windows: a 4-byte count. On Linux or macOS, `unsigned long` is 8 bytes.
    let pairs: [(i32, i32); 2] = [(0, 5), (1, 2)];
    let body: Vec<u8> = pairs
        .iter()
        .flat_map(|(g, f)| g.to_le_bytes().into_iter().chain(f.to_le_bytes()))
        .collect();
    let want = vec![FaceRef { group: 0, face: 5 }, FaceRef { group: 1, face: 2 }];
    let windows = [&2u32.to_le_bytes()[..], &body].concat();
    let unix = [&2u64.to_le_bytes()[..], &body].concat();
    assert_eq!(read_finfo(&windows).unwrap(), want);
    assert_eq!(read_finfo(&unix).unwrap(), want);
    let wrong = [&3u32.to_le_bytes()[..], &body].concat();
    assert_eq!(read_finfo(&wrong).unwrap_err().code(), "invalid");
    assert_eq!(read_finfo(&body[..6]).unwrap_err().code(), "invalid");
    assert_eq!(read_finfo(&[0, 0, 0, 0]).unwrap(), vec![]);
}

#[test]
fn scene_mesh_readers_agree_with_the_archive() {
    let Some(path) = upstream(TUTORIAL1) else {
        return;
    };
    let bytes = std::fs::read(&path).unwrap();
    let archive = Archive::parse(&bytes).unwrap();
    let mesh = read_scene_mesh(&archive.read("instance2/sceneMesh.bin").unwrap()).unwrap();
    assert_eq!(mesh.version, (1, 2));
    assert_eq!(mesh.vertices.len(), 36);
    assert_eq!(mesh.groups.len(), 1);
    assert_eq!(mesh.groups[0].faces.len(), 12);
    assert_eq!(mesh.face_record_bytes, Some(32));
    let floor = read_finfo(&archive.read("instance2/3144.finfo").unwrap()).unwrap();
    assert_eq!(
        floor,
        vec![FaceRef { group: 0, face: 0 }, FaceRef { group: 0, face: 1 }]
    );
}
