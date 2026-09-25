//! `core::config_xml` importer, `tests/fixtures/projects/tutorial1.simpa`, and the M3 gate's
//! attribute-coverage check (d) against upstream's GUI-written tutorial 1 configuration.

#[path = "config_xml_support.rs"]
mod support;

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use simpa_core::config_xml::{
    self, SolverKind, import_upstream, import_upstream_with_mesh, scene_mesh, write,
};
use simpa_core::formats::cbin;
use simpa_core::schema::{self, Project, SpectrumShape};
use support::{Value, doc_attrs, doc_ignored, parse_value, read_text, repo_file, solver_view};

const UPSTREAM_SPPS: &str = "tests/fixtures/upstream/tutorial1/spps/config.xml";
const UPSTREAM_TCR: &str = "tests/fixtures/upstream/tutorial1/tcr/config.xml";
const UPSTREAM_MESH: &str = "tests/fixtures/upstream/tutorial1/spps/mesh.cbin";
const TUTORIAL1: &str = "tests/fixtures/projects/tutorial1.simpa";

fn workdir() -> PathBuf {
    if cfg!(windows) {
        PathBuf::from(r"C:\runs\tutorial1")
    } else {
        PathBuf::from("/runs/tutorial1")
    }
}

/// The project `tutorial1.simpa` holds: upstream's SPPS configuration and scene mesh, imported,
/// then named.
fn tutorial1_from_upstream() -> Project {
    let mesh = cbin::read_file(&repo_file(UPSTREAM_MESH)).unwrap();
    let mut p = import_upstream_with_mesh(&read_text(UPSTREAM_SPPS), &mesh).unwrap();
    p.name = "Tutorial 1".into();
    p.description = "Upstream I-Simpa tutorial 1 (tutorial_1.proj at 929a5c8), imported by \
                     config_xml::import_upstream_with_mesh from \
                     tests/fixtures/upstream/tutorial1/spps/config.xml and its mesh.cbin. SPPS \
                     settings as in that fixture: 10,000 particles, random seed 1."
        .into();
    p
}

/// Regenerates the fixture. Run it on purpose: `cargo test --test config_xml_import -- --ignored`.
#[test]
#[ignore = "rewrites tests/fixtures/projects/tutorial1.simpa; run it on purpose to regenerate it"]
fn write_tutorial1_fixture() {
    schema::save(&tutorial1_from_upstream(), &repo_file(TUTORIAL1)).unwrap();
}

#[test]
fn tutorial1_fixture_is_the_import_of_upstream_tutorial1() {
    let expected = schema::to_json(&tutorial1_from_upstream());
    let on_disk = read_text(TUTORIAL1);
    assert_eq!(
        on_disk, expected,
        "regenerate with --ignored write_tutorial1_fixture"
    );
    let p = schema::from_json(&on_disk).unwrap();
    // What it holds.
    assert_eq!(p.bands.frequencies_hz.len(), 27);
    assert_eq!(p.bands.kind, schema::BandKind::ThirdOctave);
    assert_eq!(p.geometry.vertices.len(), 36);
    assert_eq!(p.geometry.faces.len(), 12);
    let groups: Vec<&str> = p.surface_groups.iter().map(|g| g.name.as_str()).collect();
    // In order of first appearance in mesh.cbin: faces 0 and 1 (the floor) carry idRs 3503.
    assert_eq!(groups, vec!["idMat 25 / idRs 3503", "idMat 22", "idMat 21"]);
    let pins: Vec<Option<u32>> = p.materials.iter().map(|m| m.solver_id).collect();
    assert_eq!(pins, vec![Some(0), Some(21), Some(22), Some(25)]);
    // f32 widening recovers what was typed.
    assert_eq!(p.materials[1].absorption[0].get(), 0.3);
    assert_eq!(p.solvers.spps.receiver_radius_m.get(), 0.31);
    assert_eq!(p.solvers.spps.time_step_s.get(), 0.01);
    assert_eq!(p.environment.ground_roughness_m.get(), 0.02);
    assert_eq!(p.sources[0].position.to_array(), [3.0, 5.0, 1.8]);
    // The source is I-Simpa's white noise at 80 dB, exactly.
    assert_eq!(
        p.sources[0].power,
        schema::Spectrum::new(80.0, SpectrumShape::White)
    );
    // The background noise is I-Simpa's white noise at 0 dB: the writer computes white and pink
    // in f32 as upstream's GUI does (band_levels_written), which gives the GUI's own floats in
    // every band, so the import recognises it.
    let noise = p.point_receivers[0].background_noise.as_ref().unwrap();
    assert_eq!(*noise, schema::Spectrum::new(0.0, SpectrumShape::White));
    // Upstream writes its lists last first; the project holds them in the GUI's own order.
    let names: Vec<&str> = p.point_receivers.iter().map(|r| r.name.as_str()).collect();
    assert_eq!(names, vec!["Receiver 1", "Receiver 2"]);
    let scene = &p.surface_receivers[0];
    assert_eq!(scene.name, "Receiver");
    assert_eq!(
        scene.shape,
        schema::SurfaceReceiverShape::Scene {
            groups: vec![p.surface_groups[0].id]
        }
    );
    println!(
        "tutorial1.simpa: {} bytes, {} materials, {} groups, {} sources, {} receivers",
        on_disk.len(),
        p.materials.len(),
        p.surface_groups.len(),
        p.sources.len(),
        p.point_receivers.len()
    );
}

#[test]
fn import_is_deterministic() {
    let a = schema::to_json(&tutorial1_from_upstream());
    let b = schema::to_json(&tutorial1_from_upstream());
    assert_eq!(a, b);
    let x = read_text(UPSTREAM_SPPS);
    assert_eq!(
        schema::to_json(&import_upstream(&x).unwrap()),
        schema::to_json(&import_upstream(&x).unwrap())
    );
}

/// M3 gate (d). Every attribute `docs/formats/config_xml.md` says a solver reads, and that
/// upstream's GUI-written tutorial 1 SPPS configuration contains, is in the configuration we
/// write from `tutorial1.simpa`. The ignored list is the page's "Ignored by the solvers" table.
/// `source@u`, `@v` and `@w` are read only for source types 1 and 5 (`base_core_configuration.cpp:
/// 135-139`), so on tutorial 1's omni source they are not read, and they are counted apart.
#[test]
fn attribute_coverage_against_upstream_tutorial1() {
    let docs: BTreeMap<String, support::DocAttr> = doc_attrs()
        .into_iter()
        .map(|a| (a.key.clone(), a))
        .collect();
    let (ignored, ignored_elements) = doc_ignored();
    let upstream = read_text(UPSTREAM_SPPS);
    let ours = write(
        &support::load_project(TUTORIAL1),
        SolverKind::Spps,
        None,
        &workdir(),
    )
    .unwrap();
    let theirs_view = solver_view(&upstream);
    let ours_view = solver_view(&ours);
    let pairs = |v: &BTreeMap<String, support::Instance>| -> BTreeSet<String> {
        v.values()
            .flat_map(|i| i.attrs.keys().map(move |a| format!("{}@{a}", i.key)))
            .collect()
    };
    let (theirs, ours_pairs) = (pairs(&theirs_view), pairs(&ours_view));
    let mut expected = BTreeSet::new();
    let mut ignored_seen = BTreeSet::new();
    let mut not_read_here = BTreeSet::new();
    let mut undocumented = BTreeSet::new();
    for i in theirs_view.values() {
        for a in i.attrs.keys() {
            let key = format!("{}@{a}", i.key);
            let is_ignored = ignored.iter().any(|g| {
                g == &key || (g.ends_with('*') && key.starts_with(g.trim_end_matches('*')))
            });
            if is_ignored {
                ignored_seen.insert(key);
            } else if !docs.contains_key(&key) {
                undocumented.insert(key);
            } else if i.key == "source"
                && ["u", "v", "w"].contains(&a.as_str())
                && !["1", "5"].contains(&i.attrs["directivite"].as_str())
            {
                not_read_here.insert(key);
            } else {
                expected.insert(key);
            }
        }
    }
    let ignored_elements_seen: Vec<String> = support::element_names(&upstream)
        .into_iter()
        .map(|(_, e)| e)
        .filter(|e| ignored_elements.contains(e))
        .collect();
    let missing: Vec<&String> = expected
        .iter()
        .filter(|k| !ours_pairs.contains(*k))
        .collect();
    println!(
        "upstream tutorial 1 SPPS config: {} distinct attributes",
        theirs.len()
    );
    println!("documented as read and expected: {}", expected.len());
    println!("ignored by the solvers (not written): {ignored_seen:?}");
    println!("ignored elements (not written): {ignored_elements_seen:?}");
    println!("not read for tutorial 1's omni source (not written): {not_read_here:?}");
    println!("undocumented: {undocumented:?}");
    println!("ours: {} distinct attributes", ours_pairs.len());
    println!("missing: {}", missing.len());
    for m in &missing {
        println!("  missing {m}");
    }
    assert!(
        undocumented.is_empty(),
        "upstream writes attributes the page does not list"
    );
    // 75 distinct attributes upstream, less 7 ignored and the omni source's u, v, w.
    assert_eq!(theirs.len(), 75);
    assert_eq!(
        expected.len(),
        65,
        "the expected set itself must not shrink unnoticed"
    );
    assert_eq!(missing.len(), 0);
    // We also write what upstream leaves out and the page requires.
    let extra: BTreeSet<&String> = ours_pairs.difference(&theirs).collect();
    println!("written by us, not by upstream: {extra:?}");
    assert!(extra.contains(&"simulation@save_surface_intersection".to_string()));
}

/// What the solver reads from our configuration of `tutorial1.simpa`, attribute by attribute,
/// against what it reads from upstream's: equal, as the solver parses each value (`atoi`, or
/// `atof` into a `float`), apart from a stated list.
#[test]
fn the_solver_reads_the_same_values_as_from_upstreams_configs() {
    let docs: BTreeMap<String, support::DocAttr> = doc_attrs()
        .into_iter()
        .map(|a| (a.key.clone(), a))
        .collect();
    let p = support::load_project(TUTORIAL1);
    for (solver, upstream) in [
        (SolverKind::Spps, UPSTREAM_SPPS),
        (SolverKind::Tcr, UPSTREAM_TCR),
    ] {
        let theirs = solver_view(&read_text(upstream));
        let ours = solver_view(&write(&p, solver, None, &workdir()).unwrap());
        let mut differences: Vec<String> = Vec::new();
        let mut compared = 0usize;
        for (path, t) in &theirs {
            let Some(o) = ours.get(path) else {
                differences.push(format!("{path}: only upstream's"));
                continue;
            };
            for (a, tv) in &t.attrs {
                let Some(d) = docs.get(&format!("{}@{a}", t.key)) else {
                    continue; // ignored by the solvers
                };
                let theirs_value = parse_value(d.kind, tv);
                match o.attrs.get(a) {
                    None => differences.push(format!("{path}@{a}: only upstream's")),
                    Some(ov) => {
                        let ours_value = parse_value(d.kind, ov);
                        assert!(!matches!(ours_value, Value::Unparsed(_)), "{path}@{a}={ov}");
                        compared += 1;
                        if ours_value != theirs_value {
                            differences.push(format!(
                                "{path}@{a}: upstream {theirs_value}, ours {ours_value}"
                            ));
                        }
                    }
                }
            }
        }
        for path in ours.keys().filter(|k| !theirs.contains_key(*k)) {
            differences.push(format!("{path}: only ours"));
        }
        let wd = config_xml::working_directory(&workdir()).unwrap();
        let expected: Vec<String> = {
            let mut e = vec![
                // The run folder.
                format!("configuration@workingdirectory: upstream '__RUNDIR__', ours '{wd}'"),
                // Ids assigned from the project's order, not the GUI's session counters. Point
                // receiver ids are only stored in GUI mode; the surface receiver's id matches
                // the idRs of the scene mesh written with it (scene_mesh).
                // Both lists are written last first, so project receiver 1 (id 1) comes first.
                "recepteursp/recepteur_ponctuel[0]@id: upstream 3669, ours 1".to_string(),
                "recepteursp/recepteur_ponctuel[1]@id: upstream 3510, ours 0".to_string(),
                "recepteurss/recepteur_surfacique[0]@id: upstream 3503, ours 0".to_string(),
                // The GUI's default material: no face of mesh.cbin uses idMat 0.
                "type_surface[id=0]: only upstream's".to_string(),
                // Ignored by the solvers (docs/formats/config_xml.md).
                "other:subdomains: only upstream's".to_string(),
                // Read only for source types 1 and 5; tutorial 1's source is omni.
                "sources/source[0]@u: only upstream's".to_string(),
                "sources/source[0]@v: only upstream's".to_string(),
                "sources/source[0]@w: only upstream's".to_string(),
            ];
            e.extend((0..27).map(|i| format!("type_surface[id=0]/bfreq[{i}]: only upstream's")));
            // SPPS: both write upstream's `loudspeakers\`. Upstream's TCR config lacks it, which
            // prints `Xml Property ... doesn't exist` and reads "", the value ours writes.
            if solver == SolverKind::Tcr {
                e.push("simulation@directivities_directory: only ours".into());
            }
            e.sort();
            e
        };
        // Attributes only we write. Where upstream leaves out an optional one, the solver uses
        // its default (`docs/formats/config_xml.md`), so ours equal to it is the same input.
        const OPTIONAL_DEFAULTS: [(&str, &str); 5] = [
            ("simulation@save_surface_intersection", "1"),
            ("simulation@save_receivers_intersection", "1"),
            ("simulation@output_recp_bysource", "0"),
            ("simulation@random_seed", "0"),
            ("simulation@recepteurss_cut_filename", "rs_cut.csbin"),
        ];
        let mut defaults = Vec::new();
        for (path, o) in &ours {
            let Some(t) = theirs.get(path) else { continue };
            for (a, ov) in o.attrs.iter().filter(|(a, _)| !t.attrs.contains_key(*a)) {
                let key = format!("{}@{a}", o.key);
                let kind = docs[&key].kind;
                match OPTIONAL_DEFAULTS.iter().find(|(k, _)| *k == key) {
                    Some((_, d)) if parse_value(kind, ov) == parse_value(kind, d) => {
                        defaults.push(key)
                    }
                    _ => differences.push(format!("{path}@{a}: only ours")),
                }
            }
        }
        differences.sort();
        println!("{solver:?}: written by us only, equal to the solver's default: {defaults:?}");
        println!(
            "{solver:?}: {compared} attribute values compared, {} differences:",
            differences.len()
        );
        for d in &differences {
            println!("  {d}");
        }
        assert_eq!(differences, expected, "{solver:?}");
    }
}

#[test]
fn the_tcr_config_imports_to_the_same_model() {
    let mesh = cbin::read_file(&repo_file(UPSTREAM_MESH)).unwrap();
    let s = import_upstream_with_mesh(&read_text(UPSTREAM_SPPS), &mesh).unwrap();
    let t = import_upstream_with_mesh(&read_text(UPSTREAM_TCR), &mesh).unwrap();
    assert!(s.description.contains("SPPS") && t.description.contains("TCR"));
    assert_eq!(s.bands, t.bands);
    assert_eq!(s.geometry.vertices, t.geometry.vertices);
    assert_eq!(s.environment, t.environment);
    // The ids differ (they derive from the input text); everything else is equal.
    let strip = |p: &Project| {
        let mut q = p.clone();
        q.materials
            .iter_mut()
            .for_each(|m| m.id = schema::MaterialId::from_u128(0));
        q.sources
            .iter_mut()
            .for_each(|x| x.id = schema::SourceId::from_u128(0));
        q.point_receivers
            .iter_mut()
            .for_each(|x| x.id = schema::PointReceiverId::from_u128(0));
        (q.materials, q.sources, q.point_receivers)
    };
    assert_eq!(strip(&s), strip(&t));
    // SPPS settings come from the SPPS config; TCR's from the TCR config.
    assert_eq!(s.solvers.spps.particles_per_source, 10_000);
    assert_eq!(s.solvers.spps.random_seed, 1);
    assert!(t.solvers.tcr.air_absorption);
}

#[test]
fn the_scene_mesh_of_tutorial1_is_upstreams_with_our_receiver_id() {
    let upstream = cbin::read_file(&repo_file(UPSTREAM_MESH)).unwrap();
    let ours = scene_mesh(&support::load_project(TUTORIAL1)).unwrap();
    assert_eq!(ours.vertices.len(), upstream.vertices.len());
    for (a, b) in ours.vertices.iter().zip(&upstream.vertices) {
        assert_eq!(
            (a.x.to_bits(), a.y.to_bits(), a.z.to_bits()),
            (b.x.to_bits(), b.y.to_bits(), b.z.to_bits()),
            "vertices narrow back to upstream's exact floats, -0.0 included"
        );
    }
    assert_eq!(ours.faces.len(), upstream.faces.len());
    for (a, b) in ours.faces.iter().zip(&upstream.faces) {
        assert_eq!(
            (a.a, a.b, a.c, a.id_mat, a.id_en),
            (b.a, b.b, b.c, b.id_mat, b.id_en)
        );
        assert_eq!(a.id_rs, if b.id_rs == 3503 { 0 } else { -1 });
    }
}

/// Write, import the result with its scene mesh, write again: the solver reads the same values
/// from both configurations. (Surface-receiver ids are renumbered when a disabled receiver is
/// dropped, and this importer keeps no pinned receiver or fitting id, so those ids are compared by
/// rank and through the faces they label, and a pinned source's `source@id`, which no solver
/// reads, does not come back.)
#[test]
fn write_import_write_gives_the_solver_the_same_input() {
    let docs: BTreeMap<String, support::DocAttr> = doc_attrs()
        .into_iter()
        .map(|a| (a.key.clone(), a))
        .collect();
    let mut projects: Vec<Project> = vec![support::rich_cube()];
    projects.extend((0..400).map(schema::generate));
    let mut round_trips = 0;
    let mut values = 0usize;
    for (k, p) in projects.iter().enumerate() {
        for solver in [SolverKind::Spps, SolverKind::Tcr] {
            let Ok(x1) = write(p, solver, None, &workdir()) else {
                continue;
            };
            let m1 = scene_mesh(p).unwrap();
            let q = match import_upstream_with_mesh(&x1, &m1) {
                Ok(q) => q,
                // A box fitting zone bounds no face, so its volume is not in the mesh.
                Err(e) if p.fitting_zones.iter().any(|z| z.enabled) => {
                    assert_eq!(e.code(), "unsupported", "project {k}: {e}");
                    continue;
                }
                Err(e) => panic!("project {k}: {}: {e}", e.code()),
            };
            let x2 = write(&q, solver, None, &workdir()).unwrap();
            let m2 = scene_mesh(&q).unwrap();
            // Receiver and fitting ids: rank-map both and compare per face and per element. This
            // importer pins no id but the materials' (a `.proj` import pins them all, decision 13),
            // so a generated project's pinned ids come back as ours, in the same order.
            let rank = |x: &str, prefix: &str| -> BTreeMap<i64, usize> {
                solver_view(x)
                    .values()
                    .filter(|i| i.key.starts_with(prefix) && i.attrs.contains_key("id"))
                    .map(|i| i.attrs["id"].parse::<i64>().unwrap())
                    .collect::<BTreeSet<_>>()
                    .into_iter()
                    .enumerate()
                    .map(|(r, id)| (id, r))
                    .collect()
            };
            let ranked = ["recepteur_surfacique", "recepteur_ponctuel", "encombrement"];
            let ranks = |x: &str| -> Vec<BTreeMap<i64, usize>> {
                ranked.iter().map(|p| rank(x, p)).collect()
            };
            let (r1, r2) = (ranks(&x1), ranks(&x2));
            assert_eq!(m1.faces.len(), m2.faces.len());
            for (f1, f2) in m1.faces.iter().zip(&m2.faces) {
                assert_eq!((f1.a, f1.b, f1.c, f1.id_mat), (f2.a, f2.b, f2.c, f2.id_mat));
                let map = |r: &BTreeMap<i64, usize>, id: i32| (id != -1).then(|| r[&i64::from(id)]);
                assert_eq!(map(&r1[0], f1.id_rs), map(&r2[0], f2.id_rs), "project {k}");
                assert_eq!(map(&r1[2], f1.id_en), map(&r2[2], f2.id_en), "project {k}");
            }
            let (v1, v2) = (solver_view(&x1), solver_view(&x2));
            assert_eq!(
                v1.keys().collect::<Vec<_>>(),
                v2.keys().collect::<Vec<_>>(),
                "project {k}"
            );
            // `source@id` is written from a pinned source only (decision 13), and this importer
            // pins no source, so it comes back without one; the solvers never read it.
            let is_source_id = |i: &support::Instance, a: &str| i.key == "source" && a == "id";
            for (path, i1) in &v1 {
                let i2 = &v2[path];
                assert_eq!(
                    i1.attrs
                        .keys()
                        .filter(|a| !is_source_id(i1, a))
                        .collect::<Vec<_>>(),
                    i2.attrs.keys().collect::<Vec<_>>(),
                    "project {k} {path}"
                );
                for (a, t1) in &i1.attrs {
                    if is_source_id(i1, a) {
                        continue;
                    }
                    if let Some(p) = ranked.iter().position(|p| i1.key.starts_with(p))
                        && a == "id"
                    {
                        let id = |t: &str| t.parse::<i64>().unwrap();
                        assert_eq!(
                            r1[p][&id(t1)],
                            r2[p][&id(&i2.attrs[a])],
                            "project {k} {solver:?} {path}@id, by rank"
                        );
                        continue;
                    }
                    let kind = docs[&format!("{}@{a}", i1.key)].kind;
                    assert_eq!(
                        parse_value(kind, t1),
                        parse_value(kind, &i2.attrs[a]),
                        "project {k} {solver:?} {path}@{a}"
                    );
                    values += 1;
                }
            }
            round_trips += 1;
        }
    }
    println!(
        "{round_trips} write-import-write round trips, {values} values equal as the solver reads them"
    );
    assert!(round_trips > 300);
}

#[test]
fn import_without_a_mesh_declares_every_material() {
    let p = import_upstream(&read_text(UPSTREAM_SPPS)).unwrap();
    assert!(p.geometry.faces.is_empty());
    assert_eq!(p.surface_groups.len(), 4);
    let x = write(&p, SolverKind::Spps, None, &workdir()).unwrap();
    let ids: Vec<String> = solver_view(&x)
        .iter()
        .filter(|(_, i)| i.key == "type_surface")
        .map(|(_, i)| i.attrs["id"].clone())
        .collect();
    assert_eq!(ids, vec!["0", "21", "22", "25"]);
}

#[test]
fn import_errors_are_typed() {
    let spps = read_text(UPSTREAM_SPPS);
    let mesh = cbin::read_file(&repo_file(UPSTREAM_MESH)).unwrap();
    let code = |xml: &str| import_upstream(xml).unwrap_err().code();
    assert_eq!(code("<configuration><simulation"), "xml_syntax");
    assert_eq!(code("<configuration/>"), "missing");
    assert_eq!(
        code(&spps.replace("nbparticules=\"10000\"", "")),
        "unsupported",
        "no solver marker"
    );
    assert_eq!(code(&spps.replace("trans_epsilon=\"5\"", "")), "missing");
    assert_eq!(
        code(&spps.replace("nbparticules=\"10000\"", "nbparticules=\"1e4\"")),
        "invalid_value"
    );
    assert_eq!(
        code(&spps.replace("type_surface id=\"21\"", "type_surface id=\"0\"")),
        "invalid_value"
    );
    assert_eq!(
        code(&spps.replacen("<bfreq freq=\"50\" db=\"47.1404190063477\"/>", "", 1)),
        "unsupported"
    );
    assert_eq!(
        code(&spps.replace(
            "<bfreq freq=\"63\" docalc=\"1\"/>",
            "<bfreq freq=\"31\" docalc=\"1\"/>"
        )),
        "unsupported"
    );
    assert_eq!(
        code(&spps.replace(
            "<encombrement_enum/>",
            "<encombrement_enum><encombrement id=\"7\"/></encombrement_enum>"
        )),
        "unsupported"
    );
    assert_eq!(
        code(&spps.replace("surf_receiv_method=\"0\"", "surf_receiv_method=\"2\"")),
        "unsupported"
    );
    // A face whose idMat config.xml does not declare.
    let e = import_upstream_with_mesh(
        &spps.replace("type_surface id=\"21\"", "type_surface id=\"23\""),
        &mesh,
    )
    .unwrap_err();
    assert_eq!(e.code(), "mesh_mismatch");
    println!("{e}");
    let e = import_upstream_with_mesh(
        &spps.replace(
            "recepteur_surfacique id=\"3503\"",
            "recepteur_surfacique id=\"1\"",
        ),
        &mesh,
    )
    .unwrap_err();
    assert_eq!(e.code(), "mesh_mismatch");
    // Scene receivers and cutting planes are separate lists in the solvers: they may share an
    // id, but a face's idRs only ever names a scene receiver.
    let cut = "<recepteur_surfacique_coupe id=\"3503\" name=\"Cut\" ax=\"0.5\" ay=\"9.5\" \
               az=\"1.5\" bx=\"0.5\" by=\"0.5\" bz=\"1.5\" cx=\"5.5\" cy=\"0.5\" cz=\"1.5\" \
               resolution=\"0.5\"/>";
    let scene = "<recepteur_surfacique id=\"3503\" name=\"Receiver\"/>";
    let both = spps.replace(scene, &format!("{scene}{cut}"));
    let p = import_upstream_with_mesh(&both, &mesh).unwrap();
    assert_eq!(p.surface_receivers.len(), 2);
    let only_cut = spps.replace(scene, cut);
    let e = import_upstream_with_mesh(&only_cut, &mesh).unwrap_err();
    assert_eq!(e.code(), "mesh_mismatch");
    println!("{e}");
    let twice = spps.replace(scene, &format!("{cut}{cut}"));
    assert_eq!(import_upstream(&twice).unwrap_err().code(), "invalid_value");
    // The solvers' comma decimal is read as they read it.
    let p = import_upstream(&spps.replace("temperature=\"20\"", "temperature=\"20,5\"")).unwrap();
    assert_eq!(p.environment.temperature_c.get(), 20.5);
}

/// A material's scattering (`diffusion`) that is not a number is refused on import, naming the
/// attribute, where SPPS itself would read it with `atof` as 0, specular, and go on
/// (`coreString.cpp:89-105`). So a project of this program never carries one; a run folder given
/// as it is can, and there `results::reference` reads it as SPPS does, 0, so the band is not
/// Lambert and the Kuttruff reference does not describe it (`docs/params.md`, "How a band's
/// scattering is read"; `results::reference`'s tests). Says no: the solvers' comma decimal, `0,5`,
/// imports as 0.5, as they read it.
#[test]
fn a_scattering_that_is_not_a_number_is_refused_on_import() {
    let spps = read_text(UPSTREAM_SPPS);
    let band = r#"<bfreq freq="20000" absorb="0.300000011920929" diffusion="0" loi="0"/>"#;
    assert!(spps.contains(band));
    let with = |diffusion: &str| {
        spps.replacen(
            band,
            &band.replace("diffusion=\"0\"", &format!("diffusion=\"{diffusion}\"")),
            1,
        )
    };
    for bad in ["x", "", "nan", "1e99"] {
        let e = import_upstream(&with(bad)).unwrap_err();
        assert_eq!(e.code(), "invalid_value", "{bad:?}: {e}");
        assert!(e.to_string().contains("diffusion"), "{bad:?}: {e}");
    }
    let p = import_upstream(&with("0,5")).unwrap();
    let m = p
        .materials
        .iter()
        .find(|m| m.solver_id == Some(21))
        .unwrap();
    // The 20 kHz band is the last of the project's bands.
    assert_eq!(m.scattering.last().unwrap().get(), 0.5);
}
