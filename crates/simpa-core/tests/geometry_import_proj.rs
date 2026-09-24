//! `core::geometry::import::import_proj` against upstream's own tutorial projects: M4 gate items
//! (b) and (c), the committed room fixtures, the embedded reference database, and the zip reader.
//!
//! These read upstream's tutorial projects from the upstream source tree at the pinned commit,
//! found by `common/paths.rs` (`$SIMPA_UPSTREAM`, else the tree `solvers/build.ps1` extracts into
//! `target/solvers/src-929a5c8`). A missing tree panics, naming where it looked, so none of these
//! tests passes without running. The same gate numbers are also asserted on the committed
//! fixtures by `geometry_import_rooms.rs`, which needs nothing outside the repo.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use simpa_core::config_xml::{import_upstream, widen_f32};
use simpa_core::geometry::import::proj::codes;
use simpa_core::geometry::import::proj::{FaceRef, read_finfo, read_scene_mesh};
use simpa_core::geometry::import::zip::{Archive, crc32};
use simpa_core::geometry::import::{
    ProjImport, REFERENCE_MATERIALS, REFERENCE_SPECTRA, UpstreamKind, import_proj,
    import_proj_file, import_proj_with_config,
};
use simpa_core::schema::{
    self, BoxBound, DiffusionLaw, Directivity, FittingShape, Project, ReflectionLaw,
    SurfaceReceiverShape, Vec3,
};

#[allow(dead_code)]
#[path = "common/paths.rs"]
mod paths;

/// A file of the upstream source tree at the pinned commit (`common/paths.rs`: `$SIMPA_UPSTREAM`,
/// else the tree `solvers/build.ps1` extracts). Panics, naming where it looked, when it is absent.
fn upstream(rel: &str) -> PathBuf {
    paths::upstream_file(rel)
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
    let path = upstream(TUTORIAL2);
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
    let path = upstream(TUTORIAL1);
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
    let path = upstream(TUTORIAL1);
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
                &m.reflection_law,
                &m.transmission_loss_db,
                m.double_sided
            ),
            (
                &w.absorption,
                &w.scattering,
                &w.reflection_law,
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
#[ignore = "rewrites tests/fixtures/rooms/tutorial1_box.simpa and elmia_corrected.simpa; run it on purpose to regenerate them, room_fixtures_are_the_import_of_upstreams_tutorials checks them"]
fn write_room_fixtures() {
    let (t1, t2) = (upstream(TUTORIAL1), upstream(TUTORIAL2));
    std::fs::create_dir_all(repo_file("tests/fixtures/rooms")).unwrap();
    schema::save(&tutorial1_box(&t1), &repo_file(BOX_FIXTURE)).unwrap();
    schema::save(&elmia_corrected(&t2), &repo_file(ELMIA_FIXTURE)).unwrap();
}

#[test]
fn room_fixtures_are_the_import_of_upstreams_tutorials() {
    let (t1, t2) = (upstream(TUTORIAL1), upstream(TUTORIAL2));
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

/// Tutorial 3's archive and its own `projet_config.xml`, to edit.
fn tutorial3_parts() -> (Vec<u8>, String) {
    let bytes = std::fs::read(upstream(TUTORIAL3)).unwrap();
    let xml = String::from_utf8(
        Archive::parse(&bytes)
            .unwrap()
            .read("instance1/projet_config.xml")
            .unwrap(),
    )
    .unwrap();
    (bytes, xml)
}

/// `xml` with the one occurrence of `from` replaced by `to`.
fn edited_once(xml: &str, from: &str, to: &str) -> String {
    assert_eq!(xml.matches(from).count(), 1, "{from}");
    xml.replacen(from, to, 1)
}

/// Tutorial 3 read with `xml` as its project: the import, or its error's code and text.
fn import_tutorial3_as(xml: &str) -> Result<ProjImport, (String, String)> {
    let (bytes, _) = tutorial3_parts();
    import_proj_with_config(&bytes, xml.as_bytes())
        .map_err(|e| (e.code().to_string(), e.to_string()))
}

/// M4's refusals of tutorial 3, lifted: its `.proj` imports with its two fitting zones (upstream's
/// element types 54 and 56), material 100's reflection law and materials 100 and 101's
/// transmission per band, and its six sources read through their two source groups, in the order
/// upstream's GUI writes them.
#[test]
fn tutorial3_imports_its_fitting_zones_laws_transmission_and_source_groups() {
    let ProjImport { project: p, report } = import_proj_file(&upstream(TUTORIAL3)).unwrap();
    for n in &report.notes {
        println!("note: {n}");
    }
    assert_eq!(
        (p.geometry.vertices.len(), p.geometry.faces.len()),
        (40, 88)
    );
    assert_eq!(
        report.welded_vertices, 0,
        "no vertex of tutorial 3 is welded"
    );

    // Zone 1, scene-fitted: the 10 faces of its own face list, which are the `fitting` group's
    // (faces 39 to 48), and its inside position as stored.
    let z1 = &p.fitting_zones[0];
    assert_eq!((z1.name.as_str(), z1.enabled), ("Fitting zone 1", true));
    let FittingShape::Surfaces {
        groups,
        inside_point,
    } = &z1.shape
    else {
        panic!("{:?}", z1.shape)
    };
    let faces: Vec<usize> = p
        .geometry
        .faces
        .iter()
        .enumerate()
        .filter(|(_, f)| groups.contains(&f.group))
        .map(|(i, _)| i)
        .collect();
    assert_eq!(faces, (39..=48).collect::<Vec<_>>());
    assert_eq!(p.group(groups[0]).unwrap().name, "fitting");
    assert_eq!(inside_point.to_array(), [3.61878, 2.66291, 1.0]);
    assert!(z1.absorption.iter().all(|a| a.get() == widen_f32(0.2)));
    assert!(z1.mean_free_path_m.iter().all(|l| l.get() == 1.0));

    // Zone 2, the box: bounds for geometry, and the corners as upstream holds them.
    let z2 = &p.fitting_zones[1];
    assert_eq!(
        z2.shape,
        FittingShape::Box {
            min: Vec3::new(13.0, 1.0, 0.0),
            max: Vec3::new(18.0, 4.0, widen_f32(1.2)),
            destination: Some([BoxBound::Max, BoxBound::Min, BoxBound::Max]),
        }
    );
    let (ba, hc) = z2.shape.box_corners().unwrap();
    assert_eq!(ba.to_array(), [13.0, 4.0, 0.0]);
    assert_eq!(hc.to_array(), [18.0, 1.0, widen_f32(1.2)]);
    // That is what upstream's region seed needs: hc - (hc - ba) * 1e-4, in f32
    // (`e_scene_encombrements_encombrement_cuboide.h:333-336`), is the seed of region 2083 in its
    // own `temp/scene_mesh.poly`, bit for bit; min and max alone give another point.
    let bytes = std::fs::read(upstream(TUTORIAL3)).unwrap();
    let stored = simpa_core::formats::poly::read(
        &Archive::parse(&bytes)
            .unwrap()
            .read("instance1/temp/scene_mesh.poly")
            .unwrap(),
    )
    .unwrap();
    let seed = |ba: Vec3, hc: Vec3| -> [u32; 3] {
        let (a, c) = (ba.to_array(), hc.to_array());
        [0, 1, 2].map(|k| {
            let (a, c) = (a[k] as f32, c[k] as f32);
            (c - (c - a) * 1e-4f32).to_bits()
        })
    };
    let region = stored
        .model_regions
        .iter()
        .find(|r| r.region_index == 2083)
        .unwrap();
    assert_eq!(seed(ba, hc), region.dot_in_region.map(f32::to_bits));
    let FittingShape::Box { min, max, .. } = &z2.shape else {
        unreachable!()
    };
    assert_ne!(seed(*min, *max), region.dot_in_region.map(f32::to_bits));

    // Material 100: Lambert in its six octave bands, specular elsewhere; it transmits in those
    // bands only. Material 101 ("Open_door") transmits from 250 Hz to 4 kHz, not at 125 Hz.
    let octaves = [125, 250, 500, 1000, 2000, 4000].map(|f| p.bands.index_of(f).unwrap());
    let m100 = p
        .materials
        .iter()
        .find(|m| m.solver_id == Some(100))
        .unwrap();
    for b in 0..p.bands.len() {
        let want = if octaves.contains(&b) {
            ReflectionLaw::Lambert
        } else {
            ReflectionLaw::Specular
        };
        assert_eq!(m100.reflection_law.at(b), Some(want), "band {b}");
    }
    let on = |m: &simpa_core::schema::Material| -> Vec<u32> {
        m.transmission_loss_db
            .as_ref()
            .unwrap()
            .iter()
            .enumerate()
            .filter(|(_, t)| t.is_some())
            .map(|(b, _)| p.bands.frequencies_hz[b])
            .collect()
    };
    assert_eq!(on(m100), [125, 250, 500, 1000, 2000, 4000]);
    let m101 = p
        .materials
        .iter()
        .find(|m| m.solver_id == Some(101))
        .unwrap();
    assert_eq!(on(m101), [250, 500, 1000, 2000, 4000]);

    // Sources: group `Milling Machine`'s three, then `Milling Machine 2`'s, as the file lists
    // them; each with upstream's element id recorded.
    let sources: Vec<(String, [f64; 3])> = p
        .sources
        .iter()
        .map(|s| (s.name.clone(), s.position.to_array()))
        .collect();
    assert_eq!(
        sources,
        [
            ("Source 1", [2.0, 7.0, widen_f32(1.2)]),
            ("Source 2", [2.0, 6.0, widen_f32(0.6)]),
            ("Source 3", [1.0, 7.0, 0.75]),
            ("Source 1", [7.0, 5.0, widen_f32(1.2)]),
            ("Source 2", [7.0, 4.0, widen_f32(0.6)]),
            ("Source 3", [6.0, 5.0, 0.75]),
        ]
        .map(|(n, x)| (n.to_string(), x))
    );
    let recorded = |kind: UpstreamKind| -> Vec<i64> {
        report
            .upstream_ids
            .iter()
            .filter(|u| u.kind == kind)
            .map(|u| u.upstream)
            .collect()
    };
    assert_eq!(
        recorded(UpstreamKind::Source),
        [974, 1133, 1292, 1452, 1611, 1770]
    );
    assert_eq!(recorded(UpstreamKind::FittingZone), [1930, 2083]);
    assert_eq!(
        recorded(UpstreamKind::PointReceiver),
        [155, 314, 473, 632, 791]
    );
    assert_eq!(recorded(UpstreamKind::CuttingPlane), [951]);
    // One-to-one: every entity once, every id once.
    let entities: BTreeSet<_> = report.upstream_ids.iter().map(|u| u.entity).collect();
    let ids: BTreeSet<_> = report.upstream_ids.iter().map(|u| u.upstream).collect();
    assert_eq!((entities.len(), ids.len()), (14, 14));
}

/// Industrial.proj (appversion 1.1.5, tutorial 3's older twin) nests its sources in two groups
/// the same way and holds three volumes, which are refused by name. Without its volumes it
/// imports, its six sources read through the groups; its fitting zones' diffusion-law lists are
/// the older one-entry kind, which upstream's loader replaces with law 0.
#[test]
fn industrial_is_refused_for_its_volumes_and_reads_its_source_groups() {
    let path = upstream(INDUSTRIAL);
    let e = import_proj_file(&path).unwrap_err();
    println!("{INDUSTRIAL}: {} ({e})", e.code());
    assert_eq!(e.code(), "proj_volumes_unsupported", "{e}");
    let bytes = std::fs::read(&path).unwrap();
    let xml = String::from_utf8(
        Archive::parse(&bytes)
            .unwrap()
            .read("instance1/projet_config.xml")
            .unwrap(),
    )
    .unwrap();
    let a = xml.find("<volumes ").unwrap();
    let b = xml.find("</volumes>").unwrap() + "</volumes>".len();
    let without = format!("{}{}", &xml[..a], &xml[b..]);
    let ProjImport { project: p, report } =
        import_proj_with_config(&bytes, without.as_bytes()).unwrap();
    for n in &report.notes {
        println!("note: {n}");
    }
    let names: Vec<&str> = p.sources.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(names, ["S1", "S2", "S3", "S1", "S2", "S3"]);
    assert_eq!(p.fitting_zones.len(), 2);
    assert!(matches!(
        p.fitting_zones[0].shape,
        FittingShape::Surfaces { .. }
    ));
    assert!(matches!(p.fitting_zones[1].shape, FittingShape::Box { .. }));
    assert!(
        p.fitting_zones
            .iter()
            .all(|z| z.diffusion_law.iter().all(|l| *l == DiffusionLaw::Uniform))
    );
}

/// Every upstream project in the source tree through the importer: the outcome of each, printed
/// as a table, and pinned. Upstream's tree holds 10: tutorials 1 to 3, Industrial.proj, and six
/// validation projects.
#[test]
fn every_upstream_project_imports_or_is_refused_by_name() {
    // src/isimpa/resources/doc, above tutorial_1.proj's folder and the tutorial folder.
    let root = upstream(TUTORIAL1)
        .ancestors()
        .nth(3)
        .unwrap()
        .to_path_buf();
    assert!(root.ends_with("resources/doc"), "{}", root.display());
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        let mut entries: Vec<PathBuf> = std::fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().path())
            .collect();
        entries.sort();
        for p in entries {
            if p.is_dir() {
                walk(&p, out);
            } else if p
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("proj"))
            {
                out.push(p);
            }
        }
    }
    let mut found = Vec::new();
    walk(&root, &mut found);
    // Projects inside a zip of the tree (the atmospheric-absorption validation ships one).
    let mut zipped = Vec::new();
    fn zips(dir: &Path, out: &mut Vec<PathBuf>) {
        for e in std::fs::read_dir(dir).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                zips(&p, out);
            } else if p.extension().is_some_and(|e| e.eq_ignore_ascii_case("zip")) {
                out.push(p);
            }
        }
    }
    zips(&root, &mut zipped);
    zipped.sort();
    let mut table = Vec::new();
    let mut outcome_of =
        |name: String, imported: Result<ProjImport, simpa_core::geometry::import::ImportError>| {
            let outcome = match imported {
                Ok(i) => format!(
                    "ok: {} faces, {} zones, {} sources",
                    i.project.geometry.faces.len(),
                    i.project.fitting_zones.len(),
                    i.project.sources.len()
                ),
                Err(e) => format!("refused: {} ({e})", e.code()),
            };
            println!("| {name} | {outcome} |");
            table.push((name, outcome.split(':').next().unwrap().to_string()));
        };
    for path in &found {
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        outcome_of(name, import_proj_file(path));
    }
    for path in &zipped {
        let bytes = std::fs::read(path).unwrap();
        let archive = Archive::parse(&bytes).unwrap();
        for e in archive.entries() {
            if e.name.to_ascii_lowercase().ends_with(".proj") {
                let inner = archive.read(&e.name).unwrap();
                let name = format!(
                    "{} in {}",
                    e.name,
                    path.file_name().unwrap().to_string_lossy()
                );
                outcome_of(name, import_proj(&inner));
            }
        }
    }
    let refused: Vec<&str> = table
        .iter()
        .filter(|(_, o)| o != "ok")
        .map(|(n, _)| n.as_str())
        .collect();
    assert_eq!(table.len(), 11, "{table:?}");
    assert_eq!(refused, ["Industrial.proj"], "{table:?}");
}

/// Each refusal of the import, fed the tutorial 3 project edited to trip it, and nothing else:
/// every one names its reason by its code (`geometry::import::proj::codes`).
#[test]
fn each_refusal_names_its_reason() {
    let (_, xml) = tutorial3_parts();
    assert!(
        import_tutorial3_as(&xml).is_ok(),
        "the unedited project imports"
    );
    let box_zone = "<encombrement name=\"Fitting zone 2\" eid=\"56\" wxid=\"2083\"";
    let group = "<sources name=\"Milling Machine\" eid=\"15\" wxid=\"973\">";
    let cases: Vec<(&str, String, &str)> = vec![
        (
            "a fitting zone of an unknown element type",
            edited_once(
                &xml,
                box_zone,
                &box_zone.replace("eid=\"56\"", "eid=\"57\""),
            ),
            codes::FITTING_TYPE_UNKNOWN,
        ),
        (
            "a fitting zone with no element type",
            edited_once(&xml, box_zone, &box_zone.replace(" eid=\"56\"", "")),
            codes::FITTING_TYPE_UNKNOWN,
        ),
        (
            "a source group with a child of an unknown element type",
            edited_once(
                &xml,
                group,
                &format!("{group}<bogus name=\"Stray\" eid=\"99\"/>"),
            ),
            codes::SOURCE_GROUP_MALFORMED,
        ),
        (
            "a source group with a child of no element type",
            edited_once(&xml, group, &format!("{group}<note name=\"Stray\"/>")),
            codes::SOURCE_GROUP_MALFORMED,
        ),
        (
            "material 100's reflection law at 125 Hz out of range",
            edited_once(
                &xml,
                "nb=\"7\" choice=\"2\" wxid=\"2959\"",
                "nb=\"7\" choice=\"7\" wxid=\"2959\"",
            ),
            codes::REFLECTION_LAW_OUT_OF_RANGE,
        ),
        (
            "material 100's reflection law at 125 Hz negative",
            edited_once(
                &xml,
                "nb=\"7\" choice=\"2\" wxid=\"2959\"",
                "nb=\"7\" choice=\"-1\" wxid=\"2959\"",
            ),
            codes::REFLECTION_LAW_OUT_OF_RANGE,
        ),
        (
            "a fitting zone's diffusion law out of range, in a list upstream keeps",
            edited_once(
                &xml,
                "wxid=\"2056\" label=\"Diffusion law\" ex=\"1\" nb=\"2\" choice=\"0\">\n                <enumeration id=\"1\" value=\"Lambert reflection\"/>\n                <enumeration id=\"0\" value=\"Uniform reflection\"/>",
                "wxid=\"2056\" label=\"Diffusion law\" ex=\"1\" nb=\"2\" choice=\"5\">\n                <enumeration id=\"0\" value=\"Uniform reflection\"/>\n                <enumeration id=\"1\" value=\"Lambert reflection\"/>",
            ),
            codes::DIFFUSION_LAW_OUT_OF_RANGE,
        ),
        (
            "the box with no height",
            edited_once(
                &xml,
                "<p name=\"z\" eid=\"25\" label=\"z\" value=\"1.2\" pr=\"6\" wxid=\"2091\"/>",
                "<p name=\"z\" eid=\"25\" label=\"z\" value=\"0\" pr=\"6\" wxid=\"2091\"/>",
            ),
            codes::FITTING_BOX_EMPTY,
        ),
        (
            "the scene-fitted zone's inside position (0, 0, 0)",
            ["1932", "1933", "1934"].iter().fold(xml.clone(), |x, w| {
                let at = x.find(&format!("wxid=\"{w}\"")).unwrap();
                let start = x[..at].rfind("value=\"").unwrap();
                let end = start + x[start + 7..].find('"').unwrap() + 8;
                format!("{}value=\"0\"{}", &x[..start], &x[end..])
            }),
            codes::FITTING_INSIDE_POINT_UNSET,
        ),
        (
            "a face listed by two fitting zones",
            {
                let a = xml.find("<encombrement name=\"Fitting zone 1\"").unwrap();
                let b = a + xml[a..].find("</encombrement>").unwrap() + "</encombrement>".len();
                let copy = xml[a..b].replacen(
                    "Fitting zone 1\" eid=\"54\" wxid=\"1930\"",
                    "Fitting zone 3\" eid=\"54\" wxid=\"9930\"",
                    1,
                );
                format!("{}{copy}{}", &xml[..b], &xml[b..])
            },
            codes::FACE_IN_TWO_FITTING_ZONES,
        ),
        (
            "the enabled scene-fitted zone without its face group",
            {
                let a = xml
                    .find("<gr label=\"Surfaces\" name=\"Surfaces\" eid=\"5\" wxid=\"1938\"")
                    .unwrap();
                let b = a + xml[a..].find("</gr>").unwrap() + "</gr>".len();
                format!("{}{}", &xml[..a], &xml[b..])
            },
            codes::FITTING_FACE_GROUP_MISSING,
        ),
        (
            "material 100 at 125 Hz transmitting more than it absorbs (5 dB against 0.11)",
            edited_once(
                &xml,
                "value=\"10\" pr=\"1\" minValue=\"0\" wxid=\"2958\"",
                "value=\"5\" pr=\"1\" minValue=\"0\" wxid=\"2958\"",
            ),
            codes::TRANSMISSION_EXCEEDS_ABSORPTION,
        ),
        (
            "material 100 at 125 Hz transmitting with no absorption",
            edited_once(
                &xml,
                "value=\"0.11\" pr=\"2\" minValue=\"0\" maxValue=\"1\" wxid=\"2955\"",
                "value=\"0\" pr=\"2\" minValue=\"0\" maxValue=\"1\" wxid=\"2955\"",
            ),
            codes::TRANSMISSION_EXCEEDS_ABSORPTION,
        ),
        (
            "a volume",
            edited_once(
                &xml,
                "<volumes name=\"Volumes\" eid=\"85\" wxid=\"2240\"/>",
                "<volumes name=\"Volumes\" eid=\"85\" wxid=\"2240\"><volume name=\"Volume 1\" eid=\"86\" wxid=\"9999\"/></volumes>",
            ),
            codes::VOLUMES_UNSUPPORTED,
        ),
    ];
    for (what, edited, code) in &cases {
        match import_tutorial3_as(edited) {
            Ok(_) => panic!("{what}: imported"),
            Err((got, text)) => {
                println!("{what}: {got} ({text})");
                assert_eq!(&got, code, "{what}: {text}");
            }
        }
    }

    // A source of a group that does not read is refused as the file's defect, naming its group.
    let broken = edited_once(
        &xml,
        group,
        &format!("{group}<source name=\"Broken\" eid=\"16\" wxid=\"99999\"/>"),
    );
    let (code, text) = import_tutorial3_as(&broken).map(|_| ()).unwrap_err();
    println!("a source of a group with no properties: {code} ({text})");
    assert_eq!(code, "invalid");
    assert!(text.contains("source group `Milling Machine`"), "{text}");
}

/// The diffusion law as upstream's loader leaves it (`e_gammeabsorption.cpp:43-59`): a stored law
/// in a list upstream saved (newest entry first) is reset to 0, and noted; in a list whose first
/// entry is "Uniform reflection", that row and every later one keep their stored law.
#[test]
fn a_fitting_zones_diffusion_law_is_upstreams_after_loading() {
    let (_, xml) = tutorial3_parts();
    let first_row = "wxid=\"2056\" label=\"Diffusion law\" ex=\"1\" nb=\"2\" choice=\"0\">\n                <enumeration id=\"1\" value=\"Lambert reflection\"/>\n                <enumeration id=\"0\" value=\"Uniform reflection\"/>";
    // Reset: law 1 stored in a list upstream saved.
    let reset = edited_once(
        &xml,
        first_row,
        &first_row.replace("choice=\"0\"", "choice=\"1\""),
    );
    let i = import_tutorial3_as(&reset).unwrap();
    let z = &i.project.fitting_zones[0];
    assert!(z.diffusion_law.iter().all(|l| *l == DiffusionLaw::Uniform));
    assert!(
        i.report
            .notes
            .iter()
            .any(|n| n.contains("Fitting zone 1") && n.contains("[(50, 1)]")),
        "{:?}",
        i.report.notes
    );
    // Kept: the same law in a list in the order upstream's loader keeps.
    let kept = edited_once(
        &xml,
        first_row,
        "wxid=\"2056\" label=\"Diffusion law\" ex=\"1\" nb=\"2\" choice=\"1\">\n                <enumeration id=\"0\" value=\"Uniform reflection\"/>\n                <enumeration id=\"1\" value=\"Lambert reflection\"/>",
    );
    let i = import_tutorial3_as(&kept).unwrap();
    let z = &i.project.fitting_zones[0];
    assert_eq!(z.diffusion_law[0], DiffusionLaw::UniformReflection);
    assert!(
        z.diffusion_law[1..]
            .iter()
            .all(|l| *l == DiffusionLaw::Uniform)
    );
}

#[test]
fn reference_database_matches_upstreams_appconst() {
    let path = upstream(APPCONST);
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
        let path = upstream(rel);
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
    let path = upstream(TUTORIAL1);
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

/// Tutorial 3's project with each edit applied in turn (`from`, `to`), imported.
fn tutorial3_edited(xml: &str, edits: &[(&str, &str)]) -> Result<ProjImport, (String, String)> {
    let x = edits
        .iter()
        .fold(xml.to_string(), |x, (from, to)| edited_once(&x, from, to));
    import_tutorial3_as(&x)
}

/// What `config_xml` hands the SPPS solver for a project: its `config.xml` and `mesh.cbin`.
fn solver_inputs(p: &Project) -> (String, Vec<u8>) {
    let config =
        simpa_core::config_xml::write(p, schema::SolverKind::Spps, None, Path::new("C:\\run\\"))
            .unwrap();
    let cbin = simpa_core::formats::cbin::write(&simpa_core::config_xml::scene_mesh(p).unwrap());
    (config, cbin)
}

/// A disabled fitting zone is kept as stored: upstream seeds no region for it, tags no face with
/// it and draws none of its triangles (`..._model.h:193`, `..._cuboide.h:311, 331`,
/// `appconfig.cpp:185`), so what the enabled-only refusals refuse is imported, with a note, and
/// the solver inputs are those of the project without it. The say-nos: the same edits with the
/// zone enabled are refused by their codes.
#[test]
fn a_disabled_zone_is_kept_as_stored_and_changes_no_solver_input() {
    let (_, xml) = tutorial3_parts();
    let zone1_on = "value=\"1\" wxid=\"1937\"";
    let zone1_off = "value=\"0\" wxid=\"1937\"";
    let box_on = "value=\"1\" wxid=\"2094\"";
    let box_off = "value=\"0\" wxid=\"2094\"";
    let origin: Vec<(String, String)> = ["1932", "1933", "1934"]
        .iter()
        .map(|w| {
            let at = xml.find(&format!("wxid=\"{w}\"")).unwrap();
            let start = xml[..at].rfind("value=\"").unwrap();
            (
                xml[start..at + 11].to_string(),
                format!("value=\"0\" pr=\"6\" wxid=\"{w}\""),
            )
        })
        .collect();
    let flat_box = (
        "<p name=\"z\" eid=\"25\" label=\"z\" value=\"1.2\" pr=\"6\" wxid=\"2091\"/>",
        "<p name=\"z\" eid=\"25\" label=\"z\" value=\"0\" pr=\"6\" wxid=\"2091\"/>",
    );
    let no_face_group = {
        let a = xml
            .find("<gr label=\"Surfaces\" name=\"Surfaces\" eid=\"5\" wxid=\"1938\"")
            .unwrap();
        let b = a + xml[a..].find("</gr>").unwrap() + "</gr>".len();
        (xml[a..b].to_string(), String::new())
    };
    // The unedited project's zones disabled: the reference the edits are compared with.
    let reference = tutorial3_edited(&xml, &[(zone1_on, zone1_off), (box_on, box_off)]).unwrap();
    assert!(reference.project.fitting_zones.iter().all(|z| !z.enabled));
    let reference_inputs = solver_inputs(&reference.project);

    // Zone 1 disabled with its inside position at the origin.
    let mut edits: Vec<(&str, &str)> = vec![(zone1_on, zone1_off), (box_on, box_off)];
    edits.extend(origin.iter().map(|(a, b)| (a.as_str(), b.as_str())));
    let i = tutorial3_edited(&xml, &edits).unwrap();
    assert!(!i.project.fitting_zones[0].enabled);
    let FittingShape::Surfaces {
        inside_point,
        groups,
    } = &i.project.fitting_zones[0].shape
    else {
        panic!()
    };
    assert_eq!(inside_point.to_array(), [0.0; 3]);
    assert_eq!(groups.len(), 1);
    assert!(
        i.report.notes.iter().any(|n| n.contains("Fitting zone 1")
            && n.contains("set its inside position before enabling it")),
        "{:?}",
        i.report.notes
    );
    assert_eq!(solver_inputs(&i.project), reference_inputs);
    // Enabled: refused.
    let mut on: Vec<(&str, &str)> = vec![(box_on, box_off)];
    on.extend(origin.iter().map(|(a, b)| (a.as_str(), b.as_str())));
    assert_eq!(
        tutorial3_edited(&xml, &on).map(|_| ()).unwrap_err().0,
        codes::FITTING_INSIDE_POINT_UNSET
    );

    // The box disabled and flat.
    let i = tutorial3_edited(&xml, &[(zone1_on, zone1_off), (box_on, box_off), flat_box]).unwrap();
    let z = &i.project.fitting_zones[1];
    assert!(!z.enabled);
    let (ba, hc) = z.shape.box_corners().unwrap();
    assert_eq!((ba.to_array()[2], hc.to_array()[2]), (0.0, 0.0));
    assert!(
        i.report
            .notes
            .iter()
            .any(|n| n.contains("Fitting zone 2") && n.contains("no volume")),
        "{:?}",
        i.report.notes
    );
    assert_eq!(solver_inputs(&i.project), reference_inputs);
    assert_eq!(
        tutorial3_edited(&xml, &[(zone1_on, zone1_off), flat_box])
            .map(|_| ())
            .unwrap_err()
            .0,
        codes::FITTING_BOX_EMPTY
    );

    // Zone 1 disabled without its face group: no face.
    let i = tutorial3_edited(
        &xml,
        &[
            (zone1_on, zone1_off),
            (box_on, box_off),
            (&no_face_group.0, ""),
        ],
    )
    .unwrap();
    let FittingShape::Surfaces { groups, .. } = &i.project.fitting_zones[0].shape else {
        panic!()
    };
    assert!(groups.is_empty());
    assert!(
        i.report
            .notes
            .iter()
            .any(|n| n.contains("has no face group")),
        "{:?}",
        i.report.notes
    );
    assert_eq!(
        tutorial3_edited(&xml, &[(box_on, box_off), (&no_face_group.0, "")])
            .map(|_| ())
            .unwrap_err()
            .0,
        codes::FITTING_FACE_GROUP_MISSING
    );

    // A second scene-fitted zone listing zone 1's faces: kept while one of the two is disabled,
    // both then covering the `fitting` group; refused with both enabled.
    // The copy goes last, so the box keeps its solver id.
    let with_copy = |copy_on: bool, first_on: bool| {
        let a = xml.find("<encombrement name=\"Fitting zone 1\"").unwrap();
        let b = a + xml[a..].find("</encombrement>").unwrap() + "</encombrement>".len();
        let end = xml.find("</encombrements>").unwrap();
        let mut copy = xml[a..b].replacen(
            "Fitting zone 1\" eid=\"54\" wxid=\"1930\"",
            "Fitting zone 3\" eid=\"54\" wxid=\"9930\"",
            1,
        );
        if !copy_on {
            copy = copy.replacen(zone1_on, "value=\"0\" wxid=\"1937\"", 1);
        }
        let x = format!("{}{copy}{}", &xml[..end], &xml[end..]);
        let x = if first_on {
            x
        } else {
            x.replacen(zone1_on, zone1_off, 1)
        };
        import_tutorial3_as(&x)
    };
    let unedited = import_tutorial3_as(&xml).unwrap();
    for (copy_on, first_on) in [(false, true), (true, false)] {
        let i =
            with_copy(copy_on, first_on).unwrap_or_else(|e| panic!("{copy_on} {first_on}: {e:?}"));
        assert_eq!(i.project.fitting_zones.len(), 3);
        let groups_of = |k: usize| match &i.project.fitting_zones[k].shape {
            FittingShape::Surfaces { groups, .. } => groups.clone(),
            other => panic!("{other:?}"),
        };
        assert_eq!(groups_of(0), groups_of(2));
        assert_eq!(i.project.group(groups_of(0)[0]).unwrap().name, "fitting");
        if first_on {
            // The copy disabled: upstream's inputs, the unedited project's.
            assert_eq!(solver_inputs(&i.project), solver_inputs(&unedited.project));
        }
    }
    assert_eq!(
        with_copy(true, true).map(|_| ()).unwrap_err().0,
        codes::FACE_IN_TWO_FITTING_ZONES
    );
}

/// Industrial's material rows are written as before 1.3.4 (`<bfreq absorb=..>`), and read as
/// upstream's loader reads them (`e_data_row_materiau.h:58-83`): a `loi` of `2.5` is 2
/// (`Convertor::ToInt` keeps what `strtol` read), a band without `affaiblissement` does not
/// transmit. Refused by their code: what upstream reports as an error and reads as 0 (a missing
/// or unreadable number), and a `loi` with no integer, which upstream leaves uninitialised.
#[test]
fn pre_1_3_4_material_rows_read_as_upstreams_loader_reads_them() {
    let path = upstream(INDUSTRIAL);
    let bytes = std::fs::read(&path).unwrap();
    let xml = String::from_utf8(
        Archive::parse(&bytes)
            .unwrap()
            .read("instance1/projet_config.xml")
            .unwrap(),
    )
    .unwrap();
    let a = xml.find("<volumes ").unwrap();
    let b = xml.find("</volumes>").unwrap() + "</volumes>".len();
    let xml = format!("{}{}", &xml[..a], &xml[b..]);
    let row = "absorb=\"0,050000\" diffusion=\"0,700000\" affaiblissement=\"15,000000\" loi=\"2\" wxid=\"1772\"";
    let import = |x: &str| {
        import_proj_with_config(&bytes, x.as_bytes())
            .map_err(|e| (e.code().to_string(), e.to_string()))
    };
    let base = import(&xml).unwrap();
    // Row 1772 is material 100's (`trans_material`) at 20 kHz.
    let material = |p: &Project| {
        p.materials
            .iter()
            .find(|m| m.solver_id == Some(100))
            .cloned()
            .expect("material 100")
    };
    let m = material(&base.project);
    let last = base.project.bands.len() - 1;
    assert_eq!(
        m.reflection_law.at(last),
        Some(ReflectionLaw::from_solver_code(2).unwrap())
    );

    // `loi` = 2.5: 2, as upstream reads it.
    let i = import(&edited_once(
        &xml,
        row,
        &row.replace("loi=\"2\"", "loi=\"2.5\""),
    ))
    .unwrap();
    assert_eq!(material(&i.project).reflection_law, m.reflection_law);
    // No `affaiblissement`: that band does not transmit.
    let i = import(&edited_once(
        &xml,
        row,
        &row.replace(" affaiblissement=\"15,000000\"", ""),
    ))
    .unwrap();
    let before = m.transmission_loss_db.as_ref().unwrap();
    assert_eq!(before[last].map(|l| l.get()), Some(15.0));
    let after = material(&i.project).transmission_loss_db.unwrap();
    assert_eq!(after[last], None);
    assert_eq!(after[..last], before[..last]);

    for (what, to) in [
        ("no loi", row.replace(" loi=\"2\"", "")),
        ("an empty loi", row.replace("loi=\"2\"", "loi=\"\"")),
        (
            "a loi with no digit",
            row.replace("loi=\"2\"", "loi=\"abc\""),
        ),
        ("no diffusion", row.replace(" diffusion=\"0,700000\"", "")),
        (
            "an absorption that is no number",
            row.replace("absorb=\"0,050000\"", "absorb=\"x\""),
        ),
        (
            "a loss that is no number",
            row.replace("affaiblissement=\"15,000000\"", "affaiblissement=\"x\""),
        ),
    ] {
        let (code, text) = import(&edited_once(&xml, row, &to))
            .map(|_| ())
            .unwrap_err();
        println!("{what}: {code} ({text})");
        assert_eq!(code, codes::MATERIAL_ROW_UNREADABLE, "{what}: {text}");
    }
}
