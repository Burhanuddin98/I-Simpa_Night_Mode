//! `core::config_xml` writer: the Writer column of `docs/formats/config_xml.md`, determinism,
//! exact numbers, solver ids, variants and refusals. No solver runs here (see
//! `config_xml_solver.rs`).

#[path = "config_xml_support.rs"]
mod support;

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use simpa_core::config_xml::{
    self, SolverIds, SolverKind, WriteError, names, scene_mesh, working_directory, write,
};
use simpa_core::schema::{
    self, AirAbsorption, AttenuationUnit, Directivity, F64, Project, ReflectionLaw,
    SurfaceReceiverShape,
};
use support::{DocAttr, Kind, doc_attrs, doc_ignored, parse_value, rich_cube, solver_view};

const SOLVERS: [SolverKind; 2] = [SolverKind::Spps, SolverKind::Tcr];

fn workdir() -> PathBuf {
    if cfg!(windows) {
        PathBuf::from(r"C:\runs\config_xml test\é run")
    } else {
        PathBuf::from("/runs/config_xml test/é run")
    }
}

fn wr(p: &Project, solver: SolverKind, variant: Option<&str>) -> String {
    write(p, solver, variant, &workdir()).unwrap_or_else(|e| panic!("{}: {e}", e.code()))
}

fn doc_by_key() -> BTreeMap<String, DocAttr> {
    doc_attrs()
        .into_iter()
        .map(|a| (a.key.clone(), a))
        .collect()
}

/// The documented attributes, keyed by element key, as `(element key, attribute)`.
fn written_pairs(xml: &str) -> BTreeSet<(String, String)> {
    solver_view(xml)
        .values()
        .flat_map(|i| i.attrs.keys().map(|a| (i.key.clone(), a.clone())))
        .collect()
}

#[test]
fn the_reference_tables_parse_to_94_attributes() {
    let attrs = doc_attrs();
    let keys: BTreeSet<&str> = attrs.iter().map(|a| a.key.as_str()).collect();
    println!("documented attributes: {}", attrs.len());
    assert_eq!(attrs.len(), 94, "the page's Count paragraph says 94");
    assert_eq!(keys.len(), 94, "no key twice");
    let (ignored, elements) = doc_ignored();
    println!("ignored: {ignored:?} and elements {elements:?}");
    let expected = [
        "type_surface@resistivite",
        "condition_atmospherique@lst_soltype",
        "simulation@intensity_filename",
        "simulation@intensity_folder",
        "simulation@intensity_rp_filename",
        "source@id",
        "recepteur_ponctuel@name",
        "volume@id",
        "volume@name",
        // Seen in tutorial 3's configs (tests/parity_inputs.rs).
        "type_surface@masse_volumique",
        "encombrement@x",
        "encombrement@y",
        "encombrement@z",
    ];
    assert_eq!(ignored, expected);
    assert_eq!(elements, ["subdomains"]);
}

/// Every Writer obligation, attribute by attribute, for both solvers, on the base project and on
/// its variant. And nothing written that the page does not document as read.
#[test]
fn every_writer_obligation_holds_for_both_solvers() {
    let doc = doc_by_key();
    let p = rich_cube();
    let ids = SolverIds::assign(&p).unwrap();
    // Solver ids of the groups whose material transmits, per variant.
    let transmitting = |variant: Option<schema::VariantId>| -> BTreeSet<String> {
        p.surface_groups
            .iter()
            .filter(|g| {
                let m = p.effective_material(g.id, variant).unwrap();
                p.material(m).unwrap().transmission_loss_db.is_some()
            })
            .map(|g| ids.material_id(g.id).unwrap().to_string())
            .collect()
    };
    let mut checked = 0usize;
    for variant in [None, Some("Absorbent walls")] {
        let vid = config_xml::resolve_variant(&p, variant).unwrap();
        let transmits = transmitting(vid);
        assert!(!transmits.is_empty());
        let configs: BTreeMap<&str, String> = [
            ("SPPS", wr(&p, SolverKind::Spps, variant)),
            ("TCR", wr(&p, SolverKind::Tcr, variant)),
        ]
        .into();
        for (solver, xml) in &configs {
            let view = solver_view(xml);
            // Nothing undocumented.
            for i in view.values() {
                for a in i.attrs.keys() {
                    let key = format!("{}@{a}", i.key);
                    assert!(doc.contains_key(&key), "{solver} writes undocumented {key}");
                }
            }
            for (key, d) in &doc {
                let (element, attr) = key.split_once('@').unwrap();
                let instances: Vec<(&String, &support::Instance)> =
                    view.iter().filter(|(_, i)| i.key == element).collect();
                let w = d.writer.as_str();
                let expect_all = |present: bool| {
                    for (path, i) in &instances {
                        assert_eq!(
                            i.attrs.contains_key(attr),
                            present,
                            "{solver} {path}: {key} (Writer: {w})"
                        );
                    }
                };
                if w.starts_with("always") {
                    assert!(
                        !instances.is_empty(),
                        "{solver}: no {element} to check {key} on"
                    );
                    expect_all(true);
                } else if w == "SPPS" || w == "TCR" {
                    if w == *solver {
                        assert!(!instances.is_empty(), "{solver}: no {element}");
                    }
                    expect_all(w == *solver);
                } else if w == "never" {
                    expect_all(false);
                } else if w == "if type 1 or 5" || w == "if type 5" {
                    let types: &[&str] = if w == "if type 5" {
                        &["5"]
                    } else {
                        &["1", "5"]
                    };
                    assert!(!instances.is_empty());
                    let mut present = 0;
                    for (path, i) in &instances {
                        let t = i.attrs["directivite"].as_str();
                        assert_eq!(
                            i.attrs.contains_key(attr),
                            types.contains(&t),
                            "{solver} {path}: {key} with directivite {t}"
                        );
                        present += usize::from(i.attrs.contains_key(attr));
                    }
                    assert!(present > 0, "rich_cube exercises {key}");
                } else if w == "if the material transmits" {
                    assert!(!instances.is_empty());
                    for (path, i) in &instances {
                        let owner = path
                            .split("]/")
                            .next()
                            .unwrap()
                            .trim_start_matches("type_surface[id=");
                        assert_eq!(
                            i.attrs.contains_key(attr),
                            transmits.contains(owner),
                            "{solver} {path}: {key}"
                        );
                    }
                } else if ["if surface receivers", "if cutting planes", "if fittings"].contains(&w)
                {
                    assert!(
                        !instances.is_empty(),
                        "{solver}: rich_cube has no {element}"
                    );
                    expect_all(true);
                } else {
                    panic!("unknown Writer obligation '{w}' for {key}: update this test");
                }
                checked += instances.len();
            }
            // Every band on every spectrum, and on freq_enum.
            let n = p.bands.len();
            let mut per_parent: BTreeMap<String, usize> = BTreeMap::new();
            for path in view.keys().filter(|k| !k.starts_with("freq_enum/")) {
                if let Some(i) = path.find("/bfreq[") {
                    *per_parent.entry(path[..i].to_string()).or_default() += 1;
                }
            }
            let spectra = view
                .iter()
                .filter(|(_, i)| {
                    [
                        "type_surface",
                        "source",
                        "recepteur_ponctuel",
                        "encombrement",
                    ]
                    .contains(&i.key.as_str())
                })
                .count();
            assert_eq!(
                per_parent.len(),
                spectra,
                "{solver}: every spectrum has entries"
            );
            assert!(
                per_parent.values().all(|&c| c == n),
                "{solver}: {per_parent:?}"
            );
            assert_eq!(
                view.keys().filter(|k| k.starts_with("freq_enum/")).count(),
                n
            );
            // No dead payload.
            for (_, e) in support::element_names(xml) {
                assert!(
                    !["subdomains", "surface_mesh", "vertices", "volume",].contains(&e.as_str()),
                    "{solver} writes <{e}>"
                );
            }
        }
    }
    println!("obligations checked on {checked} element instances");
}

/// Every attribute that upstream's GUI writes and no solver reads stays out, but `source@id` on a
/// source that pins upstream's element id (decision 13), written as that pin, source by source,
/// and on no other source. The generator pins every entity in one project of three.
#[test]
fn nothing_the_solvers_ignore_is_written() {
    let (ignored, elements) = doc_ignored();
    let mut projects = vec![rich_cube()];
    projects.extend((0..50).map(schema::generate));
    let mut pinned_sources = 0;
    for p in &projects {
        for solver in SOLVERS {
            let Ok(xml) = write(p, solver, None, &workdir()) else {
                continue;
            };
            let pairs = written_pairs(&xml);
            // Written last first, enabled sources only.
            let written: Vec<Option<String>> = source_ids(&xml);
            let expected: Vec<Option<String>> = p
                .sources
                .iter()
                .rev()
                .filter(|s| s.enabled)
                .map(|s| s.solver_id.map(|id| id.to_string()))
                .collect();
            assert_eq!(written, expected, "{solver:?}: source@id");
            pinned_sources += expected.iter().flatten().count();
            for key in &ignored {
                if key == "source@id" {
                    continue;
                }
                let (e, a) = key.split_once('@').unwrap();
                let hit = pairs.iter().any(|(pe, pa)| {
                    pe == e
                        && (pa == a
                            || (a.ends_with('*') && pa.starts_with(a.trim_end_matches('*'))))
                });
                assert!(!hit, "{solver:?} writes ignored {key}");
            }
            for (_, e) in support::element_names(&xml) {
                assert!(!elements.contains(&e), "{solver:?} writes ignored <{e}>");
            }
        }
    }
    assert!(pinned_sources > 0, "no generated project pins a source");
}

/// Each `<source>`'s `id`, in the file's order.
fn source_ids(xml: &str) -> Vec<Option<String>> {
    roxmltree::Document::parse(xml)
        .unwrap()
        .descendants()
        .filter(|n| n.has_tag_name("source"))
        .map(|n| n.attribute("id").map(str::to_string))
        .collect()
}

/// A source pinned to upstream's element id writes it as `source@id`, first, as upstream's GUI
/// does (`e_scene_sources_source.h:114`), so an imported project's config carries every id
/// upstream's does; a source made here pins none and writes none. Says no: the pin changed gives
/// the changed id, and two sources pinned alike are refused before anything is written.
#[test]
fn a_pinned_source_writes_its_element_id() {
    let mut p = rich_cube();
    assert!(p.sources.iter().all(|s| s.solver_id.is_none()));
    let enabled = p.sources.iter().filter(|s| s.enabled).count();
    assert!(enabled >= 2, "rich_cube has {enabled} enabled sources");
    for solver in SOLVERS {
        assert_eq!(source_ids(&wr(&p, solver, None)), vec![None; enabled]);
    }
    for (i, s) in p.sources.iter_mut().enumerate() {
        s.solver_id = Some(974 + 159 * i as u32);
    }
    let expected: Vec<Option<String>> = p
        .sources
        .iter()
        .rev()
        .filter(|s| s.enabled)
        .map(|s| s.solver_id.map(|id| id.to_string()))
        .collect();
    for solver in SOLVERS {
        let xml = wr(&p, solver, None);
        assert_eq!(source_ids(&xml), expected, "{solver:?}");
        assert!(xml.contains("<source id=\""), "{solver:?}: id first");
    }
    // Says no: one pin changed.
    let mut other = p.clone();
    other.sources[0].solver_id = Some(975);
    let xml = wr(&other, SolverKind::Spps, None);
    assert!(source_ids(&xml).contains(&Some("975".to_string())));
    assert!(!source_ids(&xml).contains(&Some("974".to_string())));
    // Says no: two sources pinned alike.
    let mut twice = p.clone();
    twice.sources[1].solver_id = twice.sources[0].solver_id;
    let e = write(&twice, SolverKind::Spps, None, &workdir()).unwrap_err();
    assert_eq!(e.code(), "integrity", "{e}");
    assert!(e.to_string().contains("pin the solver id 974"), "{e}");
}

#[test]
fn text_is_utf8_with_a_declaration_and_no_bom() {
    let mut p = rich_cube();
    p.sources[0].name = "Source é Ł 音".into();
    let xml = wr(&p, SolverKind::Spps, None);
    assert!(xml.starts_with("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<configuration "));
    assert!(!xml.as_bytes().starts_with(&[0xef, 0xbb, 0xbf]));
    assert!(xml.ends_with("</configuration>\n"));
    assert!(!xml.contains('\r'));
    assert!(xml.contains("name=\"Source é Ł 音\""));
    let wd = working_directory(&workdir()).unwrap();
    let doc = roxmltree::Document::parse(&xml).unwrap();
    assert_eq!(
        doc.root_element().attribute("workingdirectory"),
        Some(wd.as_str())
    );
}

#[test]
fn writing_is_deterministic() {
    let mut n = 0;
    let mut projects = vec![
        rich_cube(),
        support::load_project("tests/fixtures/projects/cube.simpa"),
    ];
    projects.extend((0..300).map(schema::generate));
    for p in &projects {
        let reloaded = schema::from_json(&schema::to_json(p)).unwrap();
        let variants: Vec<Option<String>> = std::iter::once(None)
            .chain(p.variants.iter().map(|v| Some(v.id.to_string())))
            .collect();
        for solver in SOLVERS {
            for v in &variants {
                let a = write(p, solver, v.as_deref(), &workdir());
                let b = write(&reloaded, solver, v.as_deref(), &workdir());
                match (a, b) {
                    (Ok(a), Ok(b)) => {
                        assert_eq!(a, b);
                        n += 1;
                    }
                    (Err(a), Err(b)) => assert_eq!(a.code(), b.code()),
                    (a, b) => panic!("{:?} vs {:?}", a.map(|_| ()), b.map(|_| ())),
                }
            }
        }
    }
    println!("{n} configs written twice, byte-identical");
    assert!(n > 500);
}

/// Every real reads back, as `atof` reads it, to the project's exact `f64`; and the solver's
/// `float` is that value narrowed.
#[test]
fn reals_are_exact_shortest_c_locale_decimals() {
    let doc = doc_by_key();
    let mut checked = 0usize;
    let mut projects = vec![rich_cube()];
    projects.extend((0..300).map(schema::generate));
    for p in &projects {
        let Ok(xml) = write(p, SolverKind::Spps, None, &workdir()) else {
            continue;
        };
        let view = solver_view(&xml);
        for i in view.values() {
            for (a, text) in &i.attrs {
                let key = format!("{}@{a}", i.key);
                // A pinned source's element id, which no solver reads: an integer.
                if key == "source@id" {
                    assert!(text.parse::<u32>().is_ok(), "source@id {text:?}");
                    continue;
                }
                let d = &doc[&key];
                match d.kind {
                    Kind::Real => {
                        assert!(
                            !text.is_empty()
                                && text
                                    .bytes()
                                    .all(|b| b.is_ascii_digit() || b == b'.' || b == b'-'),
                            "{}@{a} = '{text}'",
                            i.key
                        );
                        let v: f64 = text.parse().unwrap();
                        assert_eq!(format!("{v}"), *text, "shortest round-trip form");
                        let sig = text.trim_start_matches('-').replace('.', "");
                        assert!(sig.trim_start_matches('0').trim_end_matches('0').len() <= 17);
                        checked += 1;
                    }
                    Kind::Int => assert!(
                        matches!(parse_value(Kind::Int, text), support::Value::Int(_)),
                        "{}@{a} = '{text}'",
                        i.key
                    ),
                    Kind::Str => {}
                }
            }
        }
        // Spot values against the project, bit for bit. Sources are written last first, as
        // upstream's GUI writes them; their levels are the writer's own (band_levels_written:
        // upstream's f32 computation for white and pink on its 27 bands).
        let enabled: Vec<_> = p.sources.iter().filter(|s| s.enabled).collect();
        for (k, s) in enabled.iter().rev().enumerate() {
            let at = &view[&format!("sources/source[{k}]")].attrs;
            for (key, v) in ["x", "y", "z"].iter().zip(s.position.to_array()) {
                assert_eq!(at[*key].parse::<f64>().unwrap().to_bits(), v.to_bits());
            }
            let levels = config_xml::band_levels_written(&s.power, &p.bands).unwrap();
            for (b, l) in levels.iter().enumerate() {
                let t = &view[&format!("sources/source[{k}]/bfreq[{b}]")].attrs["db"];
                assert_eq!(t.parse::<f64>().unwrap().to_bits(), l.to_bits());
            }
        }
        let spps = &view["simulation"].attrs;
        assert_eq!(
            spps["pasdetemps"].parse::<f64>().unwrap().to_bits(),
            p.solvers.spps.time_step_s.get().to_bits()
        );
        assert_eq!(
            spps["trans_epsilon"].parse::<f64>().unwrap().to_bits(),
            p.solvers.spps.extinction_exponent.get().to_bits()
        );
    }
    println!("{checked} real attributes checked");
    assert!(checked > 10_000);
}

#[test]
fn air_absorption_is_explicit_and_in_per_metre() {
    let mut p = rich_cube();
    let at = |p: &Project| {
        solver_view(&wr(p, SolverKind::Tcr, None))["condition_atmospherique"]
            .attrs
            .clone()
    };
    // 0.005 dB/m is 0.005 * ln(10) / 10 per metre.
    let a = at(&p);
    assert_eq!(a["disable_absatmo_computation"], "1");
    let expect = 0.005 * std::f64::consts::LN_10 / 10.0;
    assert_eq!(a["absatmo"].parse::<f64>().unwrap(), expect);
    println!("0.005 dB/m -> absatmo=\"{}\"", a["absatmo"]);
    p.environment.air_absorption = AirAbsorption::UserDefined {
        value: F64::new(0.01),
        unit: AttenuationUnit::PerMetre,
    };
    let a = at(&p);
    assert_eq!(
        (
            a["disable_absatmo_computation"].as_str(),
            a["absatmo"].as_str()
        ),
        ("1", "0.01")
    );
    p.environment.air_absorption = AirAbsorption::Iso9613;
    let a = at(&p);
    assert_eq!(
        (
            a["disable_absatmo_computation"].as_str(),
            a["absatmo"].as_str()
        ),
        ("0", "0")
    );
}

#[test]
fn working_directory_is_absolute_with_a_trailing_separator() {
    #[cfg(windows)]
    {
        let ok = |p: &str| working_directory(Path::new(p)).unwrap();
        assert_eq!(ok(r"C:\runs\a"), r"C:\runs\a\");
        assert_eq!(ok(r"C:\runs\a\"), r"C:\runs\a\");
        assert_eq!(ok("C:/runs/a"), r"C:\runs\a\");
        assert_eq!(ok(r"\\?\C:\runs\a"), r"C:\runs\a\");
        assert_eq!(ok(r"\\?\UNC\server\share\a"), r"\\server\share\a\");
        assert_eq!(ok(r"\\server\share\a"), r"\\server\share\a\");
        assert_eq!(ok(r"B:\repos\Récepteur Ł"), r"B:\repos\Récepteur Ł\");
        for bad in [
            "runs\\a",
            r"\runs\a",
            "C:runs",
            "",
            r"\\.\pipe\x",
            r"\\?\GLOBALROOT\x",
        ] {
            let e = working_directory(Path::new(bad)).unwrap_err();
            assert_eq!(e.code(), "working_directory_invalid", "{bad}");
        }
        use std::os::windows::ffi::OsStringExt;
        let lone = std::ffi::OsString::from_wide(&[u16::from(b'C'), u16::from(b':'), 0x5c, 0xd800]);
        assert_eq!(
            working_directory(Path::new(&lone)).unwrap_err().code(),
            "working_directory_invalid"
        );
    }
    #[cfg(not(windows))]
    {
        assert_eq!(working_directory(Path::new("/runs/a")).unwrap(), "/runs/a/");
        assert!(working_directory(Path::new("runs/a")).is_err());
    }
    // The writer refuses a relative folder rather than writing it.
    let e = write(&rich_cube(), SolverKind::Spps, None, Path::new("relative")).unwrap_err();
    assert_eq!(e.code(), "working_directory_invalid");
}

#[test]
fn strings_are_escaped_and_uncarriable_characters_refused() {
    let mut p = rich_cube();
    let name = "A&B <\"x\"> 'q'\ttab";
    p.point_receivers[0].name = name.into();
    p.surface_receivers[0].name = "map\nline".into();
    let xml = wr(&p, SolverKind::Spps, None);
    let view = solver_view(&xml);
    // Lists are written last item first, as upstream's GUI writes them.
    let last = p.point_receivers.len() - 1;
    assert_eq!(
        view[&format!("recepteursp/recepteur_ponctuel[{last}]")].attrs["lbl"],
        name
    );
    assert_eq!(
        view["recepteurss/recepteur_surfacique[0]"].attrs["name"],
        "map\nline"
    );
    p.sources[0].name = "bell\u{7}".into();
    let e = write(&p, SolverKind::Spps, None, &workdir()).unwrap_err();
    assert_eq!(e.code(), "unencodable_text");
    println!("{e}");
}

#[test]
fn solver_ids_follow_the_documented_rules() {
    let p = rich_cube();
    let ids = SolverIds::assign(&p).unwrap();
    let g: Vec<u32> = p
        .surface_groups
        .iter()
        .map(|g| ids.material_id(g.id).unwrap())
        .collect();
    // Walls pin 0 through cube.simpa's material; floor and ceiling get 1 and 2.
    assert_eq!(g, vec![0, 1, 2]);
    let rs: Vec<i32> = p
        .surface_receivers
        .iter()
        .map(|r| ids.surface_receiver_id(r.id).unwrap())
        .collect();
    assert_eq!(rs, vec![0, 1, 2]);
    assert_eq!(ids.fitting_zone_id(p.fitting_zones[0].id), Some(2));
    let pr: Vec<i32> = p
        .point_receivers
        .iter()
        .map(|r| ids.point_receiver_id(r.id).unwrap())
        .collect();
    assert_eq!(pr, vec![0, 1]);

    // Unpinned groups take the smallest free id from 1, skipping pinned ones.
    let mut q = p.clone();
    q.materials[0].solver_id = Some(1);
    let ids = SolverIds::assign(&q).unwrap();
    let g: Vec<u32> = q
        .surface_groups
        .iter()
        .map(|g| ids.material_id(g.id).unwrap())
        .collect();
    assert_eq!(g, vec![1, 2, 3]);

    // Ids depend on neither the variant nor the enabled flags.
    let mut r = p.clone();
    r.active_variant = Some(r.variants[0].id);
    for s in &mut r.surface_receivers {
        s.enabled = !s.enabled;
    }
    for z in &mut r.fitting_zones {
        z.enabled = false;
    }
    assert_eq!(
        SolverIds::assign(&r).unwrap(),
        SolverIds::assign(&p).unwrap()
    );

    // On generated projects: pins kept, assigned ids unique, fitting ids from 2.
    let mut groups = 0;
    for seed in 0..500 {
        let p = schema::generate(seed);
        let ids = SolverIds::assign(&p).unwrap();
        let mut seen: BTreeMap<u32, schema::MaterialId> = BTreeMap::new();
        for gr in &p.surface_groups {
            let id = ids.material_id(gr.id).unwrap();
            let m = p.material(gr.material).unwrap();
            match m.solver_id {
                Some(pin) => assert_eq!(id, pin),
                None => assert!(id >= config_xml::FIRST_ASSIGNED_MATERIAL_ID),
            }
            // Only groups sharing a pinned base material share an id.
            if let Some(&other) = seen.get(&id) {
                assert_eq!(other, m.id);
                assert!(m.solver_id.is_some());
            }
            seen.insert(id, m.id);
            groups += 1;
        }
        for (i, z) in p.fitting_zones.iter().enumerate() {
            let want = z.solver_id.map_or(2 + i as i32, |pin| pin as i32);
            assert_eq!(ids.fitting_zone_id(z.id), Some(want));
        }
        for (i, r) in p.point_receivers.iter().enumerate() {
            let want = r.solver_id.map_or(i as i32, |pin| pin as i32);
            assert_eq!(ids.point_receiver_id(r.id), Some(want));
        }
    }
    println!("{groups} generated surface groups checked");
}

/// Decision 13 (`docs/m5-m6-design.md`): a pinned id is kept, as a `.proj` import pins upstream's
/// element ids; the others are numbered around the pins, and a project with none keeps the
/// numbering above. Says no: two entities of one kind pinned to one id, and a fitting zone pinned
/// to 0, are refused by name.
#[test]
fn pinned_solver_ids_are_kept_and_a_clash_is_refused() {
    let p = rich_cube();
    let base = SolverIds::assign(&p).unwrap();
    let mut q = p.clone();
    q.point_receivers[1].solver_id = Some(3510);
    q.surface_receivers[0].solver_id = Some(3503);
    q.fitting_zones[0].solver_id = Some(2083);
    let ids = SolverIds::assign(&q).unwrap();
    let pr: Vec<i32> = q
        .point_receivers
        .iter()
        .map(|r| ids.point_receiver_id(r.id).unwrap())
        .collect();
    assert_eq!(
        pr,
        vec![0, 3510],
        "the pin kept, the other numbered as before"
    );
    let rs: Vec<i32> = q
        .surface_receivers
        .iter()
        .map(|r| ids.surface_receiver_id(r.id).unwrap())
        .collect();
    assert_eq!(rs, vec![3503, 0, 1], "numbered around the pin from 0");
    assert_eq!(ids.fitting_zone_id(q.fitting_zones[0].id), Some(2083));
    // A pin on an id an unpinned item would have taken moves that item on.
    let mut r = p.clone();
    r.point_receivers[1].solver_id = Some(0);
    let ids = SolverIds::assign(&r).unwrap();
    let pr: Vec<i32> = r
        .point_receivers
        .iter()
        .map(|x| ids.point_receiver_id(x.id).unwrap())
        .collect();
    assert_eq!(pr, vec![1, 0]);
    // The written config and scene mesh carry the pins.
    let view = solver_view(&wr(&q, SolverKind::Spps, None));
    let fitting_ids: Vec<&String> = view
        .values()
        .filter(|i| i.key == "encombrement")
        .map(|i| &i.attrs["id"])
        .collect();
    assert_eq!(fitting_ids, ["2083"]);
    let mesh = scene_mesh(&q).unwrap();
    assert!(mesh.faces.iter().any(|f| f.id_en == 2083));
    assert!(mesh.faces.iter().any(|f| f.id_rs == 3503));
    // A project with no pins: exactly the ids it had before pins existed.
    assert_eq!(
        base.point_receivers.iter().map(|x| x.1).collect::<Vec<_>>(),
        [0, 1]
    );

    // Says no: two point receivers pinned to one id.
    let mut clash = p.clone();
    clash.point_receivers[0].solver_id = Some(155);
    clash.point_receivers[1].solver_id = Some(155);
    let e = SolverIds::assign(&clash).unwrap_err();
    assert_eq!(e.code(), "solver_id_clash", "{e}");
    assert!(e.to_string().contains("155"), "{e}");
    // ... which the project's integrity refuses too, and the writer.
    let integrity = clash.check_integrity().unwrap_err();
    assert_eq!(integrity.code(), "duplicate_solver_id", "{integrity}");
    assert!(write(&clash, SolverKind::Spps, None, &workdir()).is_err());
    // Two fitting zones (one disabled) pinned alike: refused whatever the enabled flags.
    let mut zones = q.clone();
    let mut second = zones.fitting_zones[0].clone();
    second.id = schema::FittingZoneId::from_u128(0x0c0b_e000_0000_4000_8000_0000_0000_0999);
    second.enabled = false;
    zones.fitting_zones.push(second);
    assert_eq!(
        SolverIds::assign(&zones).unwrap_err().code(),
        "solver_id_clash"
    );
    // A fitting zone pinned to 0, the solvers' "no fitting".
    let mut zero = p.clone();
    zero.fitting_zones[0].solver_id = Some(0);
    let e = SolverIds::assign(&zero).unwrap_err();
    assert_eq!(e.code(), "solver_id_clash");
    assert!(e.to_string().contains("no fitting"), "{e}");
    // Control: the same pins on different kinds do not clash.
    let mut kinds = p.clone();
    kinds.point_receivers[0].solver_id = Some(7);
    kinds.surface_receivers[0].solver_id = Some(7);
    kinds.fitting_zones[0].solver_id = Some(7);
    assert!(SolverIds::assign(&kinds).is_ok());
}

#[test]
fn scene_mesh_carries_the_same_ids_as_the_config() {
    let p = rich_cube();
    let mesh = scene_mesh(&p).unwrap();
    // The room, then the enabled box zone's 12 triangles, three vertices each, as upstream's GUI
    // appends a drawn zone (`Objet3D_maillage.cpp:783-816`).
    let (nv, nf) = (p.geometry.vertices.len(), p.geometry.faces.len());
    assert_eq!((mesh.vertices.len(), mesh.faces.len()), (nv + 36, nf + 12));
    let ids = SolverIds::assign(&p).unwrap();
    let chairs = ids.fitting_zone_id(p.fitting_zones[0].id).unwrap();
    for (k, f) in mesh.faces[nf..].iter().enumerate() {
        let first = (nv + 3 * k) as u32;
        assert_eq!(
            (f.a, f.b, f.c, f.id_mat, f.id_rs, f.id_en),
            (first, first + 1, first + 2, 0, -1, chairs)
        );
    }
    // Refusal: the room's own mesh has none of them.
    let room = config_xml::room_mesh(&p).unwrap();
    assert_eq!(room.faces[..], mesh.faces[..nf]);
    assert_eq!(room.vertices[..], mesh.vertices[..nv]);
    let floor = p.surface_groups[1].id;
    for (f, pf) in mesh.faces.iter().zip(&p.geometry.faces) {
        assert_eq!([f.a, f.b, f.c], pf.vertices);
        assert_eq!(f.id_mat, ids.material_id(pf.group).unwrap());
        // The floor is the enabled scene receiver 0; the ceiling's receiver is disabled.
        assert_eq!(f.id_rs, if pf.group == floor { 0 } else { -1 });
        assert_eq!(f.id_en, -1, "the fitting is a box, bounded by no group");
    }
    // Every idMat of the mesh is declared by the config, for every variant.
    for v in [None, Some("Absorbent walls")] {
        let view = solver_view(&wr(&p, SolverKind::Spps, v));
        for f in &mesh.faces {
            assert!(view.contains_key(&format!("type_surface[id={}]", f.id_mat)));
        }
        let declared: BTreeSet<i64> = view
            .iter()
            .filter(|(_, i)| i.key == "recepteur_surfacique")
            .map(|(_, i)| i.attrs["id"].parse().unwrap())
            .collect();
        for f in mesh.faces.iter().filter(|f| f.id_rs != -1) {
            assert!(declared.contains(&i64::from(f.id_rs)));
        }
    }
    // The box's 36 vertices are its 8 corners, through the scene's round trip.
    let drawn: BTreeSet<[u32; 3]> = mesh.vertices[nv..]
        .iter()
        .map(|v| [v.x.to_bits(), v.y.to_bits(), v.z.to_bits()])
        .collect();
    assert_eq!(drawn.len(), 8);
    for c in &drawn {
        let [x, y, z] = c.map(f32::from_bits);
        let near = |v: f32, a: f32, b: f32| (v - a).abs() < 1e-5 || (v - b).abs() < 1e-5;
        assert!(
            near(x, 3.6, 4.4) && near(y, 3.6, 4.4) && near(z, 0.4, 1.2),
            "{x} {y} {z}"
        );
    }
    // Vertices narrow exactly: the cube's are all representable.
    for (v, pv) in mesh.vertices.iter().zip(&p.geometry.vertices) {
        let [x, y, z] = pv.to_array();
        assert_eq!(
            (v.x.to_bits(), v.y.to_bits(), v.z.to_bits()),
            (
                (x as f32).to_bits(),
                (y as f32).to_bits(),
                (z as f32).to_bits()
            )
        );
    }
}

/// Only the `type_surface` elements of the overridden groups change, each to exactly the
/// override material's values; everything else is byte-identical.
#[test]
fn a_variant_changes_exactly_the_overridden_materials() {
    let p = rich_cube();
    let walls_id = "0";
    for solver in SOLVERS {
        let base = wr(&p, solver, None);
        let var = wr(&p, solver, Some("Absorbent walls"));
        let (b, v) = (solver_view(&base), solver_view(&var));
        assert_eq!(b.keys().collect::<Vec<_>>(), v.keys().collect::<Vec<_>>());
        let changed: Vec<&String> = b.keys().filter(|k| b[*k] != v[*k]).collect();
        let prefix = format!("type_surface[id={walls_id}]");
        assert!(!changed.is_empty());
        assert!(
            changed.iter().all(|k| k.starts_with(&prefix)),
            "{solver:?}: {changed:?}"
        );
        // The walls now carry the absorber, which the floor (id 1) carries in both.
        for k in b.keys().filter(|k| k.starts_with(&prefix)) {
            let floor = k.replacen(&prefix, "type_surface[id=1]", 1);
            let mut expect = b[&floor].attrs.clone();
            if !k.contains("/bfreq") {
                expect.insert("id".into(), walls_id.into());
            }
            assert_eq!(v[k].attrs, expect, "{solver:?} {k}");
        }
        // Line by line: only lines inside the walls' type_surface differ.
        let (bl, vl): (Vec<&str>, Vec<&str>) = (base.lines().collect(), var.lines().collect());
        assert_eq!(bl.len(), vl.len());
        let start = bl
            .iter()
            .position(|l| l.contains("<type_surface id=\"0\""))
            .unwrap();
        let end = start
            + bl[start..]
                .iter()
                .position(|l| l.contains("</type_surface>"))
                .unwrap();
        let differing: Vec<usize> = (0..bl.len()).filter(|&i| bl[i] != vl[i]).collect();
        assert!(
            differing.iter().all(|&i| i >= start && i <= end),
            "{differing:?}"
        );
        println!(
            "{solver:?}: {} of {} lines differ, all in <type_surface id=\"0\">",
            differing.len(),
            bl.len()
        );
    }
    // The same on generated projects: per variant, the differing instances are exactly the
    // type_surface elements of overridden groups whose material changed, and each holds the
    // override material's values.
    let mut variants = 0;
    for seed in 0..400 {
        let p = schema::generate(seed);
        let Ok(base) = write(&p, SolverKind::Spps, None, &workdir()) else {
            continue;
        };
        let ids = SolverIds::assign(&p).unwrap();
        let b = solver_view(&base);
        for var in &p.variants {
            let Ok(x) = write(&p, SolverKind::Spps, Some(&var.id.to_string()), &workdir()) else {
                continue;
            };
            let v = solver_view(&x);
            let mut expect_changed = BTreeSet::new();
            for g in &p.surface_groups {
                let m = p.effective_material(g.id, Some(var.id)).unwrap();
                let mat = p.material(m).unwrap();
                let sid = ids.material_id(g.id).unwrap();
                let at = |k: &str| {
                    v.get(&format!("type_surface[id={sid}]{k}"))
                        .map(|i| &i.attrs)
                };
                assert_eq!(
                    at("").unwrap()["side_material"],
                    if mat.double_sided { "1" } else { "0" }
                );
                for (i, a) in mat.absorption.iter().enumerate() {
                    let t = &at(&format!("/bfreq[{i}]")).unwrap()["absorb"];
                    assert_eq!(t.parse::<f64>().unwrap().to_bits(), a.get().to_bits());
                }
                if m != g.material {
                    expect_changed.insert(sid.to_string());
                }
            }
            for k in b.keys().chain(v.keys()) {
                if b.get(k) != v.get(k) {
                    let owner = k
                        .trim_start_matches("type_surface[id=")
                        .split(']')
                        .next()
                        .unwrap();
                    assert!(
                        k.starts_with("type_surface[") && expect_changed.contains(owner),
                        "seed {seed}: {k}"
                    );
                }
            }
            variants += 1;
        }
    }
    println!("{variants} generated variants checked");
    assert!(variants > 50);
}

#[test]
fn variants_are_selected_by_name_or_id() {
    let mut p = rich_cube();
    let id = p.variants[0].id.to_string();
    assert_eq!(
        wr(&p, SolverKind::Spps, Some(&id)),
        wr(&p, SolverKind::Spps, Some("Absorbent walls"))
    );
    assert_ne!(
        wr(&p, SolverKind::Spps, None),
        wr(&p, SolverKind::Spps, Some(&id))
    );
    let e = write(&p, SolverKind::Spps, Some("nope"), &workdir()).unwrap_err();
    assert_eq!(e.code(), "variant_not_found");
    let mut twin = p.variants[0].clone();
    twin.id = schema::VariantId::from_u128(99);
    p.variants.push(twin);
    let e = write(&p, SolverKind::Spps, Some("Absorbent walls"), &workdir()).unwrap_err();
    assert_eq!(e.code(), "variant_ambiguous");
    assert!(write(&p, SolverKind::Spps, Some(&id), &workdir()).is_ok());
}

#[test]
fn the_writer_refuses_what_the_solvers_cannot_do_as_meant() {
    let p = rich_cube();
    // A semi-diffuse material on a group.
    let mut q = p.clone();
    let unused = q
        .materials
        .iter()
        .find(|m| m.reflection_law == ReflectionLaw::SemiDiffuse.into())
        .unwrap()
        .id;
    q.surface_groups[1].material = unused;
    let e = write(&q, SolverKind::Tcr, None, &workdir()).unwrap_err();
    assert_eq!(e.code(), "unsupported_value");
    println!("{e}");
    // Unused, it is not written and not refused.
    assert!(!wr(&p, SolverKind::Spps, None).contains("loi=\"6\""));

    // Two groups sharing a pinned base material, split by a variant.
    let mut q = p.clone();
    q.surface_groups[1].material = q.materials[0].id;
    let (walls, floor) = (q.surface_groups[0].id, q.surface_groups[1].id);
    assert!(write(&q, SolverKind::Spps, None, &workdir()).is_ok());
    q.variants[0].set_override(walls, None);
    q.variants[0].set_override(floor, Some(q.materials[1].id));
    let e = write(&q, SolverKind::Spps, Some("Absorbent walls"), &workdir()).unwrap_err();
    assert_eq!(e.code(), "shared_solver_id");
    println!("{e}");

    // A group in two enabled scene receivers.
    let mut q = p.clone();
    q.surface_receivers[2].enabled = true;
    q.surface_receivers[2].shape = SurfaceReceiverShape::Scene {
        groups: vec![q.surface_groups[1].id],
    };
    assert_eq!(
        write(&q, SolverKind::Spps, None, &workdir())
            .unwrap_err()
            .code(),
        "group_in_two_zones"
    );
    assert_eq!(scene_mesh(&q).unwrap_err().code(), "group_in_two_zones");

    // Non-finite values, and a broken project.
    let mut q = p.clone();
    q.solvers.spps.time_step_s = F64::new(f64::NAN);
    assert_eq!(
        write(&q, SolverKind::Spps, None, &workdir())
            .unwrap_err()
            .code(),
        "non_finite_value"
    );
    assert!(
        write(&q, SolverKind::Tcr, None, &workdir()).is_ok(),
        "TCR does not get pasdetemps"
    );
    let mut q = p.clone();
    q.geometry.vertices[0].x = F64::new(1e39);
    assert_eq!(scene_mesh(&q).unwrap_err().code(), "non_finite_value");
    let mut q = p.clone();
    q.materials[0].absorption.pop();
    assert_eq!(
        write(&q, SolverKind::Spps, None, &workdir())
            .unwrap_err()
            .code(),
        "integrity"
    );
}

/// Generated projects either write, or are refused for a reason the test can confirm.
#[test]
fn generated_projects_write_or_are_refused_for_a_stated_reason() {
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    for seed in 0..1000 {
        let p = schema::generate(seed);
        let semi_diffuse_on_a_group = |v: Option<schema::VariantId>| {
            p.surface_groups.iter().any(|g| {
                let m = p.effective_material(g.id, v).unwrap();
                p.material(m)
                    .unwrap()
                    .reflection_law
                    .first_band_with(ReflectionLaw::SemiDiffuse)
                    .is_some()
            })
        };
        let variants = std::iter::once(None).chain(p.variants.iter().map(|v| Some(v.id)));
        for v in variants {
            let sel = v.map(|v| v.to_string());
            for solver in SOLVERS {
                let r = write(&p, solver, sel.as_deref(), &workdir());
                let key = match &r {
                    Ok(x) => {
                        roxmltree::Document::parse(x).expect("well-formed");
                        assert!(!semi_diffuse_on_a_group(v));
                        "ok".to_string()
                    }
                    Err(WriteError::Unsupported { .. }) => {
                        assert!(
                            semi_diffuse_on_a_group(v),
                            "seed {seed}: {}",
                            r.unwrap_err()
                        );
                        "unsupported_value (semi-diffuse)".to_string()
                    }
                    Err(WriteError::SharedSolverId { .. }) => {
                        assert!(v.is_some());
                        "shared_solver_id".to_string()
                    }
                    Err(e) => panic!("seed {seed}: {}: {e}", e.code()),
                };
                *counts.entry(key).or_default() += 1;
            }
        }
    }
    println!("{counts:?}");
    assert!(counts["ok"] > 1000);
}

#[test]
fn write_file_writes_config_xml_into_the_folder() {
    let dir = support::fresh_run_dir("write_file");
    let p = support::load_project("tests/fixtures/projects/cube.simpa");
    let path = config_xml::write_file(&p, SolverKind::Spps, None, &dir).unwrap();
    assert_eq!(path, dir.join(names::CONFIG));
    let bytes = std::fs::read(&path).unwrap();
    assert_eq!(
        bytes,
        write(&p, SolverKind::Spps, None, &dir).unwrap().as_bytes()
    );
    // The folder is written as workingdirectory, with its trailing separator.
    let text = String::from_utf8(bytes).unwrap();
    let wd = working_directory(&dir).unwrap();
    assert!(text.contains(&format!("workingdirectory=\"{wd}\"")));
    assert!(wd.ends_with(std::path::MAIN_SEPARATOR));
}

#[test]
fn directivity_files_are_staged_once_under_unique_names() {
    let mut p = rich_cube();
    let mut extra = p.sources[2].clone();
    extra.id = schema::SourceId::from_u128(7);
    extra.name = "Second loudspeaker".into();
    extra.directivity = Directivity::Balloon {
        file: "other/SPEAKER-TEST3.txt".into(),
        direction: schema::Vec3::new(1.0, 0.0, 0.0),
    };
    p.sources.push(extra.clone());
    extra.id = schema::SourceId::from_u128(8);
    extra.name = "Third".into();
    p.sources.push(extra);
    let staged = config_xml::directivity_files(&p);
    let names: Vec<(&str, &str)> = staged
        .iter()
        .map(|f| (f.project_path.as_str(), f.run_name.as_str()))
        .collect();
    assert_eq!(
        names,
        vec![
            ("directivities/speaker-test3.txt", "speaker-test3.txt"),
            ("other/SPEAKER-TEST3.txt", "2_SPEAKER-TEST3.txt"),
        ]
    );
    let view = solver_view(&wr(&p, SolverKind::Spps, None));
    let sim = &view["simulation"].attrs;
    assert_eq!(
        sim["directivities_directory"],
        format!("{}{}", names::DIRECTIVITY_DIR, std::path::MAIN_SEPARATOR)
    );
    let files: Vec<&str> = view
        .values()
        .filter(|i| i.key == "source")
        .filter_map(|i| i.attrs.get("directivity_file").map(String::as_str))
        .collect();
    // In the file, sources come last first, as upstream's GUI writes them.
    assert_eq!(
        files,
        vec![
            "2_SPEAKER-TEST3.txt",
            "2_SPEAKER-TEST3.txt",
            "speaker-test3.txt"
        ]
    );
    // A file with no name is refused, however many there are (SPPS crashes without one).
    for extra_empty in [1, 2] {
        let mut q = p.clone();
        for k in 0..extra_empty {
            let mut s = q.sources[2].clone();
            s.id = schema::SourceId::from_u128(100 + k);
            s.directivity = Directivity::Balloon {
                file: format!("folder{k}/"),
                direction: schema::Vec3::new(1.0, 0.0, 0.0),
            };
            q.sources.push(s);
        }
        let e = write(&q, SolverKind::Spps, None, &workdir()).unwrap_err();
        assert_eq!(e.code(), "unsupported_value", "{e}");
    }
    // With no balloon source the attribute is still written, empty.
    let cube = support::load_project("tests/fixtures/projects/cube.simpa");
    assert_eq!(
        solver_view(&wr(&cube, SolverKind::Tcr, None))["simulation"].attrs["directivities_directory"],
        ""
    );
}

#[test]
fn doc_kinds_cover_every_attribute_written() {
    // Guard for the helpers: every attribute kind the page gives is one the view parses.
    let kinds: BTreeSet<String> = doc_attrs()
        .iter()
        .map(|a| format!("{:?}", a.kind))
        .collect();
    assert_eq!(
        kinds,
        ["Int", "Real", "Str"]
            .iter()
            .map(|s| s.to_string())
            .collect()
    );
}

/// A box zone's 12 triangles as upstream's `BuildModel` builds them
/// (`e_scene_encombrements_encombrement_cuboide.h:113-173`), worked out by hand for corners
/// `ba` (0, 0, 0) and `hc` (1, 2, 3) in the unit frame, where OpenGL `(x, y, z)` is world
/// `(x, z, -y)` (`Mathlib.h:50-67`): `ba` is GL (0, 0, -0) and `hc` GL (1, 3, -2), so only `z`
/// swaps, giving `BA` = GL (0, 0, -2) and `HC` = GL (1, 3, -0), and the other six corners from
/// them (`:144-150`). Back in world coordinates: BA (0, 2, 0), BB (1, 2, 0), BC (1, -0, 0),
/// BD (0, -0, 0), HA (0, 2, 3), HB (1, 2, 3), HC (1, -0, 3), HD (0, -0, 3); `-0` where GL `z` is
/// `-0`. Then the triangles in `PushTriangle` order (`:157-172`), vertex order included: the
/// winding is the solver's face normal. Compared bit for bit; the say-no is the first triangle
/// with two vertices swapped.
#[test]
fn upstream_box_triangles_are_build_models_worked_by_hand() {
    let (ba, bb, bc, bd) = (
        [0.0f32, 2.0, 0.0],
        [1.0, 2.0, 0.0],
        [1.0, -0.0, 0.0],
        [0.0, -0.0, 0.0],
    );
    let (ha, hb, hc, hd) = (
        [0.0f32, 2.0, 3.0],
        [1.0, 2.0, 3.0],
        [1.0, -0.0, 3.0],
        [0.0, -0.0, 3.0],
    );
    let expected = [
        [bc, bd, ba],
        [bc, ba, bb],
        [ba, ha, hb],
        [ba, hb, bb],
        [ba, hd, ha],
        [ba, bd, hd],
        [bd, bc, hc],
        [hd, bd, hc],
        [bc, bb, hb],
        [hc, bc, hb],
        [hc, hb, ha],
        [hd, hc, ha],
    ];
    let bits = |t: &[[[f32; 3]; 3]]| -> Vec<[[u32; 3]; 3]> {
        t.iter()
            .map(|tri| tri.map(|p| p.map(f32::to_bits)))
            .collect()
    };
    let built =
        simpa_core::config_xml::upstream_box_triangles(None, [0.0, 0.0, 0.0], [1.0, 2.0, 3.0]);
    assert_eq!(bits(&built), bits(&expected));
    // Say-no: one triangle wound the other way is a different list.
    let mut flipped = expected;
    flipped[0].swap(0, 1);
    assert_ne!(bits(&built), bits(&flipped));
    // `BuildModel` orders the corners itself (`:126-143`): from the other corner, the same list.
    let other =
        simpa_core::config_xml::upstream_box_triangles(None, [1.0, 2.0, 3.0], [0.0, 0.0, 0.0]);
    assert_eq!(bits(&built), bits(&other));
    // Equal corners build nothing (`:124-125`).
    assert!(simpa_core::config_xml::upstream_box_triangles(None, [1.0; 3], [1.0; 3]).is_empty());
}
