//! Parity of our solver inputs with the inputs original I-Simpa wrote for its own tutorial runs:
//! the scene mesh `mesh.cbin` and the `config.xml` of every run folder (`report/<core>/<date>/`)
//! stored in upstream's tutorial projects. `docs/formats/cbin.md` and `docs/formats/config_xml.md`,
//! "Parity with upstream's GUI", state what is equal and why each remaining difference is there.
//!
//! The projects are read from the upstream source tree at the pinned commit (`common/paths.rs`:
//! `$SIMPA_UPSTREAM`, else the tree `solvers/build.ps1` extracts). A missing tree panics, naming
//! where it looked, so none of these tests passes without running. What the tutorials hold:
//!
//! - `tutorial 1/tutorial_1.proj`: an SPPS run and a TCR run (2019-06-07), one scene mesh;
//! - `tutorial 2/tutorial_2.proj`: no run folder, only TetGen's files under `temp/`;
//! - `tutorial 3/tutorial_3.proj`: three SPPS runs (2019-06-18), one scene mesh;
//!   `tutorial 3/Industrial.proj`: no run folder.
//!
//! "The same value" means the same as the solver reads it: `atoi` for an integer, `atof` into an
//! `f32` for a real (compared by bits), a string verbatim; for the scene mesh, each face's corners
//! as `f32` bits and its ids. Every check has a refusal beside it: the same comparison on an input
//! changed in one value, which it must report.
//!
//! The last section runs the differences that remain through the solvers: our M1 builds of SPPS
//! and TCR (`$SIMPA_SOLVERS_DIR`, else `target/solvers/bin`; a missing build panics, naming where
//! it looked), same seed, upstream's inputs against ours, every output file compared.

#[path = "config_xml_support.rs"]
mod support;

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use simpa_core::config_xml::{
    GlFrame, SolverKind, band_levels_written, import_upstream_with_mesh, scene_mesh, write,
};
use simpa_core::formats::{cbin, csbin, mbin, poly, tetgen};
use simpa_core::geometry::import::import_proj;
use simpa_core::geometry::import::proj::read_scene_mesh;
use simpa_core::geometry::import::zip::Archive;
use simpa_core::schema::{
    DiffusionLaw, F64, FittingShape, FittingZone, FittingZoneId, Project, Spectrum, SpectrumShape,
    Vec3,
};
use support::{
    Value, doc_attrs, doc_ignored, load_project, parse_value, solver_view, upstream_file,
};

const TUTORIAL1: &str = r"src/isimpa/resources/doc/tutorial/tutorial 1/tutorial_1.proj";
const TUTORIAL2: &str = r"src/isimpa/resources/doc/tutorial/tutorial 2/tutorial_2.proj";
const TUTORIAL3: &str = r"src/isimpa/resources/doc/tutorial/tutorial 3/tutorial_3.proj";
const INDUSTRIAL: &str = r"src/isimpa/resources/doc/tutorial/tutorial 3/Industrial.proj";
const TUTORIAL1_SIMPA: &str = "tests/fixtures/projects/tutorial1.simpa";
const TUTORIAL1_BOX: &str = "tests/fixtures/rooms/tutorial1_box.simpa";

// ---------------------------------------------------------------------------------------------
// Upstream's run folders

/// One run folder of a tutorial project: what upstream's GUI handed the solver.
struct Run {
    folder: String,
    solver: SolverKind,
    config: String,
    mesh_bytes: Vec<u8>,
    mesh: cbin::Model,
    /// The run's `tetramesh.mbin`.
    tetra_bytes: Vec<u8>,
    /// The project file the GUI saved beside the run (`projet_config.xml`).
    project_file: String,
}

struct Tutorial {
    bytes: Vec<u8>,
    runs: Vec<Run>,
}

fn tutorial(rel: &str) -> Tutorial {
    let bytes = std::fs::read(upstream_file(rel)).unwrap();
    let archive = Archive::parse(&bytes).unwrap();
    let mut runs = Vec::new();
    for e in archive.entries() {
        let Some(folder) = e.name.strip_suffix("config.xml") else {
            continue;
        };
        if !folder.contains("/report/") || folder.ends_with("projet_") {
            continue;
        }
        let read = |name: &str| archive.read(&format!("{folder}{name}")).unwrap();
        let config = String::from_utf8(read("config.xml")).unwrap();
        let mesh_bytes = read("mesh.cbin");
        let mesh = cbin::read(&mesh_bytes).unwrap();
        let solver = if folder.contains("/report/SPPS/") {
            SolverKind::Spps
        } else if folder.contains("/report/Classical theory of reverberation/") {
            SolverKind::Tcr
        } else {
            panic!("{rel}: a run folder of an unknown core: {folder}");
        };
        runs.push(Run {
            folder: folder.to_string(),
            solver,
            config,
            mesh_bytes,
            mesh,
            tetra_bytes: read("tetramesh.mbin"),
            project_file: String::from_utf8(read("projet_config.xml")).unwrap(),
        });
    }
    runs.sort_by(|a, b| a.folder.cmp(&b.folder));
    drop(archive);
    Tutorial { bytes, runs }
}

/// `configuration@workingdirectory`: writing ours with the same folder makes that attribute equal.
fn working_folder(config: &str) -> String {
    let doc = roxmltree::Document::parse(config).unwrap();
    let wd = doc.root_element().attribute("workingdirectory").unwrap();
    assert!(wd.ends_with('\\'), "{wd}");
    wd.to_string()
}

#[test]
fn what_the_tutorials_hold() {
    let found: Vec<(&str, Vec<String>)> = [TUTORIAL1, TUTORIAL2, TUTORIAL3, INDUSTRIAL]
        .into_iter()
        .map(|rel| {
            let t = tutorial(rel);
            let runs = t.runs.iter().map(|r| r.folder.clone()).collect();
            (rel.rsplit('/').next().unwrap(), runs)
        })
        .collect();
    for (name, runs) in &found {
        println!("{name}: {} run folders {runs:?}", runs.len());
    }
    assert_eq!(
        found,
        vec![
            (
                "tutorial_1.proj",
                vec![
                    "instance2/report/Classical theory of reverberation/2019-06-07_11h57m58s/"
                        .to_string(),
                    "instance2/report/SPPS/2019-06-07_11h58m41s/".to_string(),
                ]
            ),
            ("tutorial_2.proj", vec![]),
            (
                "tutorial_3.proj",
                vec![
                    "instance1/report/SPPS/2019-06-18_14h27m51s/".to_string(),
                    "instance1/report/SPPS/2019-06-18_14h28m18s/".to_string(),
                    "instance1/report/SPPS/2019-06-18_14h31m31s/".to_string(),
                ]
            ),
            ("Industrial.proj", vec![]),
        ]
    );
}

/// Upstream's GUI writes each list newest element first. It keeps a list's elements in the order
/// it created or loaded them, which is the order of their session ids (a project is loaded sorted
/// by id, `element.cpp:159`; a new element is appended), and it writes each one with
/// `new wxXmlNode(parent, ...)`, which puts the new node first among its parent's children. So in
/// every stored run each list of two or more is in strictly descending id: sources, point
/// receivers, surface receivers with cutting planes, fitting zones.
///
/// This characterises upstream's data; our writer's order is held by the config tests below,
/// which fail when a list is written first to last. The refusal here is of the check itself:
/// each list put in the order a first-to-last writer would give (ascending id) is refused.
#[test]
fn upstream_writes_every_list_newest_element_first() {
    let newest_first = |ids: &[i64]| ids.windows(2).all(|w| w[0] > w[1]);
    let mut lists = 0;
    for rel in [TUTORIAL1, TUTORIAL3] {
        for run in &tutorial(rel).runs {
            let doc = roxmltree::Document::parse(&run.config).unwrap();
            for parent in ["sources", "recepteursp", "recepteurss", "encombrement_enum"] {
                let ids: Vec<i64> = doc
                    .descendants()
                    .filter(|n| n.has_tag_name(parent))
                    .flat_map(|p| p.children().filter(|c| c.is_element()))
                    .map(|n| n.attribute("id").unwrap().parse().unwrap())
                    .collect();
                if ids.len() < 2 {
                    continue;
                }
                lists += 1;
                assert!(newest_first(&ids), "{} {parent}: {ids:?}", run.folder);
                let mut first_to_last = ids.clone();
                first_to_last.sort_unstable();
                assert!(
                    !newest_first(&first_to_last),
                    "{} {parent}: {first_to_last:?} passes",
                    run.folder
                );
            }
        }
    }
    // Tutorial 1: its two point receivers, in each of 2 runs. Tutorial 3: 6 sources, 5 point
    // receivers and 2 fitting zones, in each of 3 runs.
    assert_eq!(lists, 2 + 3 * 3);
}

// ---------------------------------------------------------------------------------------------
// config.xml: what the solvers read

/// What the solvers read from `theirs` and from `ours` (`support::solver_view`: each element
/// where the solvers find it, lists by position, materials by id, band entries by their rank
/// after the solvers' sort), value by value, as sorted lines:
///
/// - `path@attr: upstream X, ours Y` for a value that differs as the solver reads it;
/// - `path@attr: only upstream's (X)`, `only ours (Y)` for an attribute one side lacks;
/// - `path: only upstream's`, `only ours` for an element one side lacks (its band entries are
///   not listed again).
///
/// An attribute `docs/formats/config_xml.md` lists as ignored by the solvers is skipped. An
/// attribute neither listed as read nor as ignored panics: nothing is skipped unseen.
fn solver_differences(theirs: &str, ours: &str) -> Vec<String> {
    let docs: BTreeMap<String, support::DocAttr> = doc_attrs()
        .into_iter()
        .map(|a| (a.key.clone(), a))
        .collect();
    let (ignored, _) = doc_ignored();
    let is_ignored = |key: &str| {
        ignored
            .iter()
            .any(|g| g == key || (g.ends_with('*') && key.starts_with(g.trim_end_matches('*'))))
    };
    let kind = |key: &str| -> Option<support::Kind> {
        match docs.get(key) {
            Some(d) => Some(d.kind),
            None => {
                assert!(
                    is_ignored(key),
                    "{key} is neither read nor ignored in docs/formats/config_xml.md"
                );
                None
            }
        }
    };
    let (t, o) = (solver_view(theirs), solver_view(ours));
    let one_sided =
        |path: &str, have: &BTreeMap<String, support::Instance>| !have.contains_key(path);
    let mut out = Vec::new();
    for (path, ti) in &t {
        let Some(oi) = o.get(path) else {
            let parent = path.rsplit_once("/bfreq[").map(|(p, _)| p);
            if parent.is_none_or(|p| !one_sided(p, &o)) {
                out.push(format!("{path}: only upstream's"));
            }
            continue;
        };
        for (a, tv) in &ti.attrs {
            let Some(k) = kind(&format!("{}@{a}", ti.key)) else {
                continue;
            };
            match oi.attrs.get(a) {
                None => out.push(format!("{path}@{a}: only upstream's ({tv})")),
                Some(ov) => {
                    let (x, y) = (parse_value(k, tv), parse_value(k, ov));
                    assert!(!matches!(y, Value::Unparsed(_)), "{path}@{a} = '{ov}'");
                    if x != y {
                        out.push(format!("{path}@{a}: upstream {x}, ours {y}"));
                    }
                }
            }
        }
        for (a, ov) in oi.attrs.iter().filter(|(a, _)| !ti.attrs.contains_key(*a)) {
            if kind(&format!("{}@{a}", oi.key)).is_some() {
                out.push(format!("{path}@{a}: only ours ({ov})"));
            }
        }
    }
    for path in o.keys().filter(|k| !t.contains_key(*k)) {
        let parent = path.rsplit_once("/bfreq[").map(|(p, _)| p);
        if parent.is_none_or(|p| !one_sided(p, &t)) {
            out.push(format!("{path}: only ours"));
        }
    }
    out.sort();
    out
}

/// Differences that are there by design, with the reason each is there
/// (`docs/formats/config_xml.md`, "Parity with upstream's GUI").
fn by_design(solver: SolverKind) -> Vec<String> {
    let mut e = vec![
        // The GUI's section of the project tree for volumes, which no solver looks up.
        "other:subdomains: only upstream's".to_string(),
        // Read only for source types 1 and 5; tutorial 1's source is omni.
        "sources/source[0]@u: only upstream's (1)".to_string(),
        "sources/source[0]@v: only upstream's (1)".to_string(),
        "sources/source[0]@w: only upstream's (1)".to_string(),
        // The GUI's default material, which no face of the scene uses.
        "type_surface[id=0]: only upstream's".to_string(),
    ];
    match solver {
        // Optional attributes upstream's GUI never writes, written at the solver's default (1).
        SolverKind::Spps => e.extend([
            "simulation@save_receivers_intersection: only ours (1)".to_string(),
            "simulation@save_surface_intersection: only ours (1)".to_string(),
        ]),
        // Upstream's TCR config lacks it: TCR prints `Xml Property ... doesn't exist !` and reads
        // "", the value ours writes.
        SolverKind::Tcr => e.push("simulation@directivities_directory: only ours ()".to_string()),
    }
    e
}

/// Tutorial 1's ids: upstream numbers elements by its GUI's session counters, we by project
/// order (config_xml's module docs). Receivers are written last first by both, so the first one
/// written is our receiver 1.
fn tutorial1_ids() -> Vec<String> {
    vec![
        "recepteursp/recepteur_ponctuel[0]@id: upstream 3669, ours 1".to_string(),
        "recepteursp/recepteur_ponctuel[1]@id: upstream 3510, ours 0".to_string(),
        "recepteurss/recepteur_surfacique[0]@id: upstream 3503, ours 0".to_string(),
    ]
}

/// The point receivers' directions: upstream's GUI computes a direction when a receiver moves
/// and keeps it at full precision for the rest of that session, which is when these runs were
/// written; its project file keeps 6 significant digits (`-0.436852`), which is what upstream
/// itself holds after reopening the project, and what the import reads. Up to 5e-7 apart.
fn tutorial1_directions() -> Vec<String> {
    [
        (0, "u", "-0.3833573", "-0.383357"),
        (0, "v", "-0.8945003", "-0.8945"),
        (0, "w", "-0.23001435", "-0.230014"),
        (1, "u", "-0.43685207", "-0.436852"),
        (1, "v", "-0.43685207", "-0.436852"),
        (1, "w", "-0.7863337", "-0.786334"),
    ]
    .iter()
    .map(|(i, a, t, o)| format!("recepteursp/recepteur_ponctuel[{i}]@{a}: upstream {t}, ours {o}"))
    .collect()
}

fn sorted(mut v: Vec<String>) -> Vec<String> {
    v.sort();
    v
}

/// The text of the next `f32` above the value a real attribute holds, as the solver reads it.
fn next_float_text(text: &str) -> String {
    let v = text.parse::<f64>().unwrap() as f32;
    let up = if v == 0.0 {
        f32::from_bits(1)
    } else if v > 0.0 {
        f32::from_bits(v.to_bits() + 1)
    } else {
        f32::from_bits(v.to_bits() - 1)
    };
    format!("{up}")
}

/// `ours` with one attribute of the first element matching `element_start` replaced.
fn edit_attr(xml: &str, element_start: &str, attr: &str, value: &str) -> String {
    let at = xml.find(element_start).expect("element");
    let key = format!(" {attr}=\"");
    let a = at + xml[at..].find(&key).expect("attribute") + key.len();
    let b = a + xml[a..].find('"').unwrap();
    format!("{}{value}{}", &xml[..a], &xml[b..])
}

fn attr_in<'a>(xml: &'a str, element_start: &str, attr: &str) -> &'a str {
    let at = xml.find(element_start).expect("element");
    let key = format!(" {attr}=\"");
    let a = at + xml[at..].find(&key).expect("attribute") + key.len();
    let b = a + xml[a..].find('"').unwrap();
    &xml[a..b]
}

/// `xml` without the attribute `attr` of the first element matching `element_start`.
fn drop_attr(xml: &str, element_start: &str, attr: &str) -> String {
    let at = xml.find(element_start).expect("element");
    let key = format!(" {attr}=\"");
    let a = at + xml[at..].find(&key).expect("attribute");
    let b = a + key.len() + xml[a + key.len()..].find('"').unwrap() + 1;
    format!("{}{}", &xml[..a], &xml[b..])
}

/// The comparison itself, on our tutorial-1 SPPS config against itself: one value of each kind the
/// solver reads, changed in one place, gives exactly one line; so do an attribute and an element
/// left out. Two changes the solver cannot see give none: a real printed the way upstream prints
/// it (its `f32` at 15 significant digits, upstream's own text for that value), and a source's
/// band entries in the other order (the solvers sort them, `cxml.cpp:130-156`).
#[test]
fn the_comparison_gives_one_line_per_value_the_solver_reads_differently() {
    let t = tutorial(TUTORIAL1);
    let project = import_proj(&t.bytes).unwrap().project;
    let run = t
        .runs
        .iter()
        .find(|r| r.solver == SolverKind::Spps)
        .unwrap();
    let wd = working_folder(&run.config);
    let ours = write(&project, SolverKind::Spps, None, Path::new(&wd)).unwrap();
    assert_eq!(solver_differences(&ours, &ours), Vec::<String>::new());
    let against = |edited: &str| solver_differences(&ours, edited);

    // An int.
    let n = attr_in(&ours, "<simulation ", "nbparticules");
    let m = (n.parse::<i64>().unwrap() + 1).to_string();
    assert_eq!(
        against(&edit_attr(&ours, "<simulation ", "nbparticules", &m)),
        vec![format!("simulation@nbparticules: upstream {n}, ours {m}")]
    );
    // A real, one f32 step.
    let h = attr_in(&ours, "<condition_atmospherique ", "humidite");
    let up = next_float_text(h);
    assert_eq!(
        against(&edit_attr(
            &ours,
            "<condition_atmospherique ",
            "humidite",
            &up
        )),
        vec![format!(
            "condition_atmospherique@humidite: upstream {}, ours {up}",
            h.parse::<f64>().unwrap() as f32
        )]
    );
    // A string.
    assert_eq!(
        against(&edit_attr(&ours, "<simulation ", "modelName", "Mesh.cbin")),
        vec!["simulation@modelName: upstream 'mesh.cbin', ours 'Mesh.cbin'".to_string()]
    );
    // An attribute left out.
    let seed = attr_in(&ours, "<simulation ", "random_seed");
    assert_eq!(
        against(&drop_attr(&ours, "<simulation ", "random_seed")),
        vec![format!("simulation@random_seed: only upstream's ({seed})")]
    );
    // An element left out: the last point receiver, with its band entries (not listed again).
    let a = ours.rfind("<recepteur_ponctuel ").unwrap();
    let close = "</recepteur_ponctuel>";
    let b = a + ours[a..].find(close).unwrap() + close.len();
    let fewer = format!("{}{}", &ours[..a], &ours[b..]);
    assert_eq!(
        against(&fewer),
        vec!["recepteursp/recepteur_ponctuel[1]: only upstream's".to_string()]
    );

    // No line: upstream's own text for the receiver radius, 15 significant digits of its f32.
    let theirs = attr_in(&run.config, "<simulation ", "rayon_recepteurp");
    let mine = attr_in(&ours, "<simulation ", "rayon_recepteurp");
    assert_ne!(theirs, mine, "the two texts differ");
    assert_eq!(
        against(&edit_attr(
            &ours,
            "<simulation ",
            "rayon_recepteurp",
            theirs
        )),
        Vec::<String>::new()
    );
    // No line: the source's band entries last first.
    let a = ours.find("<source ").unwrap();
    let b = a + ours[a..].find("</source>").unwrap();
    let mut lines: Vec<&str> = ours[a..b].lines().collect();
    let bands: Vec<usize> = (0..lines.len())
        .filter(|&i| lines[i].trim_start().starts_with("<bfreq "))
        .collect();
    assert_eq!(bands.len(), project.bands.len());
    let reversed: Vec<&str> = bands.iter().rev().map(|&i| lines[i]).collect();
    for (&i, l) in bands.iter().zip(reversed) {
        lines[i] = l;
    }
    let swapped = format!("{}{}{}", &ours[..a], lines.join("\n"), &ours[b..]);
    assert_ne!(swapped, ours);
    assert_eq!(against(&swapped), Vec::<String>::new());
}

/// Tutorial 1, the `.proj` imported the way a user opens it, written for each of upstream's two
/// runs into upstream's own run folder: every value the solver reads is upstream's, apart from
/// the ids and the stored directions (see [`tutorial1_ids`], [`tutorial1_directions`]).
#[test]
fn tutorial1_config_from_the_proj_is_upstreams_value_for_value() {
    let t = tutorial(TUTORIAL1);
    let project = import_proj(&t.bytes).unwrap().project;
    assert_eq!(t.runs.len(), 2);
    for run in &t.runs {
        let wd = working_folder(&run.config);
        let ours = write(&project, run.solver, None, Path::new(&wd)).unwrap();
        let got = solver_differences(&run.config, &ours);
        println!(
            "{} ({:?}): {} differences",
            run.folder,
            run.solver,
            got.len()
        );
        for d in &got {
            println!("  {d}");
        }
        let mut expected = by_design(run.solver);
        expected.extend(tutorial1_ids());
        expected.extend(tutorial1_directions());
        assert_eq!(got, sorted(expected.clone()), "{}", run.folder);

        // Refusals. One f32 step on one band of the source's power: reported, and nothing else.
        let db = attr_in(&ours, "<bfreq freq=\"1000\" db=", "db");
        let bumped = next_float_text(db);
        let edited = edit_attr(&ours, "<bfreq freq=\"1000\" db=", "db", &bumped);
        let mut with_edit = expected.clone();
        with_edit.push(format!(
            "sources/source[0]/bfreq[13]@db: upstream {}, ours {}",
            db.parse::<f64>().unwrap() as f32,
            bumped
        ));
        assert_eq!(solver_differences(&run.config, &edited), sorted(with_edit));
        // One value of the project: the air temperature, 20 -> 20.5.
        let mut warmer = project.clone();
        warmer.environment.temperature_c = simpa_core::schema::F64::new(20.5);
        let x = write(&warmer, run.solver, None, Path::new(&wd)).unwrap();
        let mut with_edit = expected.clone();
        with_edit.push("condition_atmospherique@temperature: upstream 20, ours 20.5".to_string());
        assert_eq!(solver_differences(&run.config, &x), sorted(with_edit));
        // Order: the receivers written first to last instead of last first.
        let mut forward = project.clone();
        forward.point_receivers.reverse();
        let x = write(&forward, run.solver, None, Path::new(&wd)).unwrap();
        let swapped = solver_differences(&run.config, &x);
        assert!(
            swapped
                .iter()
                .any(|d| d.starts_with("recepteursp/recepteur_ponctuel[0]@x: upstream 3, ours 1")),
            "{swapped:?}"
        );
    }
}

/// Tutorial 3 cannot become a project unchanged, by either path. Its `.proj` is refused for its
/// fitting zones (`geometry::import`). Its config holds two materials a project cannot:
///
/// - material 100 has reflection law 2 (Lambert) in 6 of 27 bands and 0 (specular) in the
///   others, and a project holds one law per material;
/// - material 101 ("Open_door") transmits in 5 of the 6 bands where it absorbs, with a 0 dB loss,
///   but not at 125 Hz, and a project's material transmits in every band that absorbs or in none.
///
/// With [`importable`]'s two edits (law 0 for material 100 in every band, a 0 dB loss for 101 at
/// 125 Hz) the config and scene mesh import, and writing the project back into each run's folder
/// gives the solver every value upstream gave it, apart from those 7 band values and the ids.
#[test]
fn tutorial3_config_written_back_is_upstreams_value_for_value() {
    let t = tutorial(TUTORIAL3);
    assert_eq!(t.runs.len(), 3);
    for run in &t.runs {
        // Refused unedited, and with the first edit only: each edit is needed.
        let refused = import_upstream_with_mesh(&run.config, &run.mesh).unwrap_err();
        assert!(
            refused.to_string().contains("type_surface 100 loi"),
            "{refused}"
        );
        let refused = import_upstream_with_mesh(&one_law(&run.config), &run.mesh).unwrap_err();
        assert!(
            refused
                .to_string()
                .contains("type_surface 101 affaiblissement"),
            "{refused}"
        );
        let project = import_upstream_with_mesh(&importable(&run.config), &run.mesh).unwrap();
        let wd = working_folder(&run.config);
        let ours = write(&project, run.solver, None, Path::new(&wd)).unwrap();
        let got = solver_differences(&run.config, &ours);
        println!(
            "{} ({:?}): {} differences",
            run.folder,
            run.solver,
            got.len()
        );
        for d in &got {
            println!("  {d}");
        }
        let mut expected = tutorial3_ids_and_by_design();
        // Material 100's law in the 6 bands the project cannot hold (125 Hz to 4 kHz, octaves).
        for rank in [4, 7, 10, 13, 16, 19] {
            expected.push(format!(
                "type_surface[id=100]/bfreq[{rank}]@loi: upstream 2, ours 0"
            ));
        }
        // Material 101's loss at 125 Hz, where upstream's does not transmit.
        expected.push("type_surface[id=101]/bfreq[4]@affaiblissement: only ours (0)".to_string());
        assert_eq!(got, sorted(expected.clone()), "{}", run.folder);

        // Refusal: material 101's transmission loss at 1 kHz one f32 step up.
        let at = ours.find("<type_surface id=\"101\"").unwrap();
        let band = at + ours[at..].find("<bfreq freq=\"1000\"").unwrap();
        let loss = attr_in(&ours[band..], "<bfreq", "affaiblissement");
        let bumped = next_float_text(loss);
        let edited = format!(
            "{}{}",
            &ours[..band],
            edit_attr(&ours[band..], "<bfreq", "affaiblissement", &bumped)
        );
        let mut with_edit = expected.clone();
        with_edit.push(format!(
            "type_surface[id=101]/bfreq[13]@affaiblissement: upstream {}, ours {bumped}",
            loss.parse::<f64>().unwrap() as f32
        ));
        assert_eq!(solver_differences(&run.config, &edited), sorted(with_edit));
    }
}

/// What differs between tutorial 3's config and ours written back, apart from the two edits: the
/// ids and the differences by design.
fn tutorial3_ids_and_by_design() -> Vec<String> {
    let mut expected = vec![
        "other:subdomains: only upstream's".to_string(),
        "simulation@save_receivers_intersection: only ours (1)".to_string(),
        "simulation@save_surface_intersection: only ours (1)".to_string(),
        // Ids by project order: receivers 155..791 are our 0..4, written last first; the
        // cutting plane 951 is our 0; the fitting zones 1930 and 2083 are our 2 and 3.
        "encombrement_enum/encombrement[0]@id: upstream 2083, ours 3".to_string(),
        "encombrement_enum/encombrement[1]@id: upstream 1930, ours 2".to_string(),
        "recepteurss/recepteur_surfacique_coupe[0]@id: upstream 951, ours 0".to_string(),
    ];
    for (i, id) in [791, 632, 473, 314, 155].iter().enumerate() {
        expected.push(format!(
            "recepteursp/recepteur_ponctuel[{i}]@id: upstream {id}, ours {}",
            4 - i
        ));
    }
    // Read only for source types 1 and 5; every source here is omni.
    for i in 0..6 {
        for a in ["u", "v", "w"] {
            expected.push(format!("sources/source[{i}]@{a}: only upstream's (1)"));
        }
    }
    expected
}

/// Tutorial 3's config with the two edits a project needs (see
/// [`tutorial3_config_written_back_is_upstreams_value_for_value`]): material 100 gets reflection
/// law 0 in every band, and material 101 a 0 dB loss at 125 Hz, like its other absorbing bands.
fn importable(xml: &str) -> String {
    let xml = one_law(xml);
    let (a, b) = material_span(&xml, "101");
    let band = "<bfreq freq=\"125\" absorb=\"1\" diffusion=\"0\" loi=\"0\"/>";
    assert_eq!(xml[a..b].matches(band).count(), 1, "material 101 at 125 Hz");
    format!(
        "{}{}{}",
        &xml[..a],
        xml[a..b].replace(
            band,
            "<bfreq freq=\"125\" absorb=\"1\" diffusion=\"0\" loi=\"0\" affaiblissement=\"0\"/>"
        ),
        &xml[b..]
    )
}

/// Tutorial 3's config with the first edit only: material 100 gets reflection law 0 in every band.
fn one_law(xml: &str) -> String {
    let (a, b) = material_span(xml, "100");
    let changed = xml[a..b].replace("loi=\"2\"", "loi=\"0\"");
    assert_ne!(changed, xml[a..b], "material 100 has law 2 somewhere");
    format!("{}{changed}{}", &xml[..a], &xml[b..])
}

/// The byte range of `<type_surface id="{id}" ...>` up to its closing tag.
fn material_span(xml: &str, id: &str) -> (usize, usize) {
    let start = xml
        .find(&format!("<type_surface id=\"{id}\""))
        .expect("material");
    (start, start + xml[start..].find("</type_surface>").unwrap())
}

// ---------------------------------------------------------------------------------------------
// Spectra

/// Every white- and pink-noise spectrum of the tutorials' runs, from the project file the GUI
/// saved beside the run (spectrum, global level `lw`, attenuations), is written by ours as the
/// exact `f32` upstream wrote (`band_levels_written`). The refusal: the same spectra computed in
/// `f64` (`Spectrum::band_levels_db`) miss some bands by one unit in the last place.
#[test]
fn white_and_pink_spectra_are_upstreams_floats() {
    let bands =
        simpa_core::schema::BandSet::range(simpa_core::schema::BandKind::ThirdOctave, 50, 20000)
            .unwrap();
    let mut spectra = 0;
    let mut f64_misses = 0;
    for rel in [TUTORIAL1, TUTORIAL3] {
        for run in &tutorial(rel).runs {
            let written = roxmltree::Document::parse(&run.config).unwrap();
            let saved = roxmltree::Document::parse(&run.project_file).unwrap();
            for el in written
                .descendants()
                .filter(|n| n.has_tag_name("source") || n.has_tag_name("recepteur_ponctuel"))
            {
                let id = el.attribute("id").unwrap();
                let tag = if el.has_tag_name("source") {
                    "source"
                } else {
                    "recepteurp"
                };
                let node = saved
                    .descendants()
                    .find(|n| n.has_tag_name(tag) && n.attribute("wxid") == Some(id))
                    .unwrap_or_else(|| panic!("{}: no {tag} {id} in the project", run.folder));
                let (shape, lw) = saved_spectrum(node);
                let mut theirs: Vec<(u32, f32)> = el
                    .children()
                    .filter(|b| b.has_tag_name("bfreq"))
                    .map(|b| {
                        (
                            b.attribute("freq").unwrap().parse().unwrap(),
                            b.attribute("db").unwrap().parse::<f64>().unwrap() as f32,
                        )
                    })
                    .collect();
                theirs.sort_by_key(|b| b.0);
                let theirs: Vec<u32> = theirs.iter().map(|b| b.1.to_bits()).collect();
                let spectrum = Spectrum::new(lw, shape);
                let ours: Vec<u32> = band_levels_written(&spectrum, &bands)
                    .unwrap()
                    .iter()
                    .map(|&v| (v as f32).to_bits())
                    .collect();
                assert_eq!(ours, theirs, "{} {tag} {id}: {spectrum:?}", run.folder);
                let exact: Vec<u32> = spectrum
                    .band_levels_db(&bands)
                    .unwrap()
                    .iter()
                    .map(|&v| (v as f32).to_bits())
                    .collect();
                f64_misses += exact.iter().zip(&theirs).filter(|(a, b)| a != b).count();
                spectra += 1;
            }
        }
    }
    println!(
        "{spectra} spectra equal to upstream's floats in every band; computed in f64, {f64_misses} \
         band values would differ"
    );
    assert_eq!(
        spectra, 39,
        "tutorial 1: 2 runs x 3; tutorial 3: 3 runs x 11"
    );
    assert!(
        f64_misses > 0,
        "the f64 computation must show the check can fail"
    );
}

/// A saved source's or receiver's spectrum: the reference spectrum (white or pink, with no
/// attenuation in any band; anything else panics, since the tutorials have nothing else) and
/// the user's global level `lw`.
fn saved_spectrum(node: roxmltree::Node) -> (SpectrumShape, f64) {
    let sp = node
        .descendants()
        .find(|n| n.has_tag_name("spectre"))
        .unwrap();
    assert_eq!(
        sp.attribute("typespectre"),
        Some("45"),
        "a reference spectrum"
    );
    let shape = match sp.attribute("idspectre") {
        Some("0") => SpectrumShape::White,
        Some("1") => SpectrumShape::Pink,
        other => panic!("reference spectrum {other:?}"),
    };
    let value = |row: roxmltree::Node, name: &str| -> Option<f64> {
        row.children()
            .find(|p| p.attribute("name") == Some(name))
            .map(|p| p.attribute("value").unwrap().parse().unwrap())
    };
    let mut lw = None;
    for row in sp.children().filter(|c| c.has_tag_name("p")) {
        if row.attribute("name") == Some("cumul") {
            lw = value(row, "lw");
        } else {
            assert_eq!(value(row, "att").unwrap_or(0.0), 0.0, "no attenuation");
        }
    }
    (shape, lw.expect("a global level"))
}

// ---------------------------------------------------------------------------------------------
// The scene mesh

/// Differences the solvers would see between upstream's scene mesh and ours, face by face: the
/// face count, each face's three corners as `f32` bits in its own order (a, b, c), and its
/// `idMat`, and `idRs` and `idEn` through the id maps (upstream's id to ours). The solvers use a
/// scene vertex only as a corner of the faces that name it (`coreinitialisation.cpp:417-422`,
/// `CalculationCore.cpp:524-526`, `sppsInitialisation.cpp:99-101`, `TC_CalculationCore.cpp:33,
/// 97, 117, 127, 524-526`), so a vertex list welded differently is the same input.
fn face_differences(
    theirs: &cbin::Model,
    ours: &cbin::Model,
    id_rs: &BTreeMap<i32, i32>,
    id_en: &BTreeMap<i32, i32>,
) -> Vec<String> {
    let mut out = Vec::new();
    if theirs.faces.len() != ours.faces.len() {
        out.push(format!(
            "faces: upstream {}, ours {}",
            theirs.faces.len(),
            ours.faces.len()
        ));
    }
    let corner = |m: &cbin::Model, v: u32| {
        let p = m.vertices[v as usize];
        [p.x.to_bits(), p.y.to_bits(), p.z.to_bits()]
    };
    let map = |m: &BTreeMap<i32, i32>, id: i32| {
        if id == -1 {
            -1
        } else {
            *m.get(&id).unwrap_or(&i32::MIN)
        }
    };
    for (i, (t, o)) in theirs.faces.iter().zip(&ours.faces).enumerate() {
        for (k, (a, b)) in [t.a, t.b, t.c].into_iter().zip([o.a, o.b, o.c]).enumerate() {
            let (ca, cb) = (corner(theirs, a), corner(ours, b));
            if ca != cb {
                out.push(format!(
                    "face {i} corner {k}: upstream {:?}, ours {:?}",
                    ca.map(f32::from_bits),
                    cb.map(f32::from_bits)
                ));
            }
        }
        if t.id_mat != o.id_mat {
            out.push(format!(
                "face {i} idMat: upstream {}, ours {}",
                t.id_mat, o.id_mat
            ));
        }
        if map(id_rs, t.id_rs) != o.id_rs {
            out.push(format!(
                "face {i} idRs: upstream {}, ours {}",
                t.id_rs, o.id_rs
            ));
        }
        if map(id_en, t.id_en) != o.id_en {
            out.push(format!(
                "face {i} idEn: upstream {}, ours {}",
                t.id_en, o.id_en
            ));
        }
    }
    out
}

fn vertex_set(m: &cbin::Model) -> BTreeSet<[u32; 3]> {
    m.vertices
        .iter()
        .map(|v| [v.x.to_bits(), v.y.to_bits(), v.z.to_bits()])
        .collect()
}

/// Upstream's id to ours, for every element of `tag` in the two configs, by position: both write
/// their lists in the same order (the config tests check it).
fn id_map(theirs: &str, ours: &str, tag: &str) -> BTreeMap<i32, i32> {
    let ids = |xml: &str| -> Vec<i32> {
        roxmltree::Document::parse(xml)
            .unwrap()
            .descendants()
            .filter(|n| n.has_tag_name(tag))
            .map(|n| n.attribute("id").unwrap().parse().unwrap())
            .collect()
    };
    let (t, o) = (ids(theirs), ids(ours));
    assert_eq!(t.len(), o.len(), "{tag}");
    t.into_iter().zip(o).collect()
}

/// `model` with the project's vertices as they are before upstream's OpenGL round trip: only
/// narrowed to `f32`.
fn without_round_trip(model: &cbin::Model, project: &Project) -> cbin::Model {
    let mut m = model.clone();
    for (v, p) in m.vertices.iter_mut().zip(&project.geometry.vertices) {
        let [x, y, z] = p.to_array().map(|c| c as f32);
        *v = cbin::Vertex { x, y, z };
    }
    m
}

/// Tutorial 1, the `.proj` imported: our scene mesh gives the solver upstream's faces, corner for
/// corner bit for bit, with upstream's materials. Our vertex list is the welded one (8 vertices
/// against upstream's 36, one copy per face), holding exactly upstream's 8 distinct vertices.
#[test]
fn tutorial1_scene_mesh_from_the_proj_is_upstreams_corner_for_corner() {
    let t = tutorial(TUTORIAL1);
    let project = import_proj(&t.bytes).unwrap().project;
    let ours = scene_mesh(&project).unwrap();
    for run in &t.runs {
        let written = write(
            &project,
            run.solver,
            None,
            Path::new(&working_folder(&run.config)),
        )
        .unwrap();
        let id_rs = id_map(&run.config, &written, "recepteur_surfacique");
        assert_eq!(id_rs, BTreeMap::from([(3503, 0)]));
        let none = BTreeMap::new();
        assert_eq!(
            face_differences(&run.mesh, &ours, &id_rs, &none),
            Vec::<String>::new()
        );
        assert_eq!((run.mesh.vertices.len(), ours.vertices.len()), (36, 8));
        assert_eq!(vertex_set(&run.mesh), vertex_set(&ours));

        // Refusals. Without upstream's round trip, every corner at y = 0 is +0 where upstream
        // has -0: 18 corners of 8 faces.
        let plain = without_round_trip(&ours, &project);
        let got = face_differences(&run.mesh, &plain, &id_rs, &none);
        println!("without the round trip: {} corner differences", got.len());
        assert_eq!(got.len(), 18, "{got:?}");
        assert!(
            got.iter()
                .all(|d| d.contains("-0.0") && d.contains("corner"))
        );
        // One vertex one f32 step off: every corner that names it.
        let mut nudged = ours.clone();
        nudged.vertices[5].z = f32::from_bits(nudged.vertices[5].z.to_bits() + 1);
        let named = ours
            .faces
            .iter()
            .flat_map(|f| [f.a, f.b, f.c])
            .filter(|&v| v == 5)
            .count();
        assert_eq!(
            face_differences(&run.mesh, &nudged, &id_rs, &none).len(),
            named
        );
        // One face's material.
        let mut other = ours.clone();
        other.faces[3].id_mat = 21;
        assert_eq!(
            face_differences(&run.mesh, &other, &id_rs, &none),
            vec!["face 3 idMat: upstream 22, ours 21".to_string()]
        );
        // The receiver's id not mapped.
        assert_eq!(face_differences(&run.mesh, &ours, &none, &none).len(), 2);
    }
}

/// Tutorial 1 as `tutorial1.simpa` holds it (its config and scene mesh imported): our `mesh.cbin`
/// is upstream's, byte for byte, once our receiver id 0 is written as upstream's 3503.
///
/// This shows the writer's layout, face order and ids, not the round trip: the fixture's
/// vertices were read from upstream's `.cbin`, which already took it, and taking them through it
/// again changes none of them. The round trip is held by the `.proj` import above and the `.poly`
/// and `.mbin` tests below, whose vertices have not taken it.
#[test]
fn tutorial1_scene_mesh_from_its_config_is_upstreams_byte_for_byte() {
    let t = tutorial(TUTORIAL1);
    let project = load_project(TUTORIAL1_SIMPA);
    let ours = scene_mesh(&project).unwrap();
    for run in &t.runs {
        let mut relabelled = ours.clone();
        for f in &mut relabelled.faces {
            if f.id_rs == 0 {
                f.id_rs = 3503;
            }
        }
        let bytes = cbin::write(&relabelled);
        assert!(
            bytes == run.mesh_bytes,
            "{}: not byte-identical",
            run.folder
        );
        println!("{}: {} bytes, identical", run.folder, bytes.len());
        // Refusal: one bit of one vertex.
        let mut flipped = relabelled.clone();
        flipped.vertices[35].x = f32::from_bits(flipped.vertices[35].x.to_bits() ^ 1);
        assert!(cbin::write(&flipped) != run.mesh_bytes);
        let id_rs = BTreeMap::from([(3503, 3503)]);
        assert_eq!(
            face_differences(&run.mesh, &flipped, &id_rs, &BTreeMap::new()).len(),
            1
        );
    }
}

/// The mesher's `.poly` takes its vertices from [`scene_mesh`], as upstream's `_SavePOLY` takes
/// them through the same round trip (`Objet3D_maillage.cpp:942`). So the tutorial box's
/// `scene_mesh.poly` and `.var`, from the room fixture and from `tutorial_1.proj` imported, are
/// the files upstream's GUI wrote in 2019 (`tests/fixtures/upstream/tutorial1/tetgen/`), byte for
/// byte. The refusal: the vertices only narrowed to `f32`, as before the round trip, give a
/// different file (`0` where upstream's has `-0`).
#[test]
fn the_box_poly_from_the_scene_mesh_is_upstreams_byte_for_byte() {
    let theirs = |name: &str| {
        std::fs::read(support::repo_file(&format!(
            "tests/fixtures/upstream/tutorial1/tetgen/{name}"
        )))
        .unwrap()
    };
    let (poly_2019, var_2019) = (theirs("scene_mesh.poly"), theirs("scene_mesh.var"));
    let from_proj = import_proj(&tutorial(TUTORIAL1).bytes).unwrap().project;
    for (what, project) in [
        ("rooms/tutorial1_box.simpa", load_project(TUTORIAL1_BOX)),
        ("tutorial_1.proj", from_proj),
    ] {
        let input = simpa_core::mesh::project_input(&project).unwrap();
        assert!(poly::write(&input.poly) == poly_2019, "{what}: .poly");
        assert!(input.var.as_deref() == Some(&var_2019[..]), "{what}: .var");
        let mut plain = input.poly.clone();
        for (v, p) in plain
            .model_vertices
            .iter_mut()
            .zip(&project.geometry.vertices)
        {
            *v = p.to_array().map(|c| f64::from(c as f32));
        }
        assert!(
            poly::write(&plain) != poly_2019,
            "{what}: without the round trip"
        );
    }
}

/// Tutorial 3 (its config with [`importable`]'s two edits, and its scene mesh, imported): our
/// scene mesh is upstream's vertex for vertex and face for face, the drawn fitting box's 12
/// faces included, with the fitting ids mapped. And upstream's own scene file, `sceneMesh.bin`,
/// taken through [`GlFrame`]'s round trip, gives the run's `.cbin` vertices bit for bit.
///
/// Like the test above, this holds the layout and the ids, not the round trip: the vertices come
/// from upstream's `.cbin`, and plain narrowing of `sceneMesh.bin` also gives its 40 scene
/// vertices. What it does show of the frame is that one `f32` step off in scale moves some.
#[test]
fn tutorial3_scene_mesh_is_upstreams_vertex_for_vertex() {
    let t = tutorial(TUTORIAL3);
    let archive = Archive::parse(&t.bytes).unwrap();
    let scene = read_scene_mesh(&archive.read("instance1/sceneMesh.bin").unwrap()).unwrap();
    let frame = GlFrame::of_vertices(scene.vertices.iter().copied()).unwrap();
    for run in &t.runs {
        let project = import_upstream_with_mesh(&importable(&run.config), &run.mesh).unwrap();
        let ours = scene_mesh(&project).unwrap();
        let written = write(
            &project,
            run.solver,
            None,
            Path::new(&working_folder(&run.config)),
        )
        .unwrap();
        let id_en = id_map(&run.config, &written, "encombrement");
        assert_eq!(id_en, BTreeMap::from([(2083, 3), (1930, 2)]));
        let none = BTreeMap::new();
        assert_eq!(
            face_differences(&run.mesh, &ours, &none, &id_en),
            Vec::<String>::new()
        );
        assert_eq!(run.mesh.vertices.len(), 76);
        let bits = |m: &cbin::Model| -> Vec<[u32; 3]> {
            m.vertices
                .iter()
                .map(|v| [v.x.to_bits(), v.y.to_bits(), v.z.to_bits()])
                .collect()
        };
        assert_eq!(bits(&run.mesh), bits(&ours), "vertex for vertex, by bits");
        assert_eq!(run.mesh.faces.len(), 100);

        // Upstream's scene file through the round trip: the run's first 40 vertices (the 36
        // after them are the drawn box's, appended by the GUI).
        let tripped: Vec<[u32; 3]> = scene
            .vertices
            .iter()
            .map(|&v| frame.round_trip(v).map(f32::to_bits))
            .collect();
        let theirs: Vec<[u32; 3]> = run.mesh.vertices[..40]
            .iter()
            .map(|v| [v.x.to_bits(), v.y.to_bits(), v.z.to_bits()])
            .collect();
        assert_eq!(tripped, theirs);

        // Refusals: the fitting ids unmapped, and a frame one f32 step off in scale.
        assert_eq!(face_differences(&run.mesh, &ours, &none, &none).len(), 22);
        let off = GlFrame {
            scale: f32::from_bits(frame.scale.to_bits() + 1),
            ..frame
        };
        let moved = scene
            .vertices
            .iter()
            .zip(&theirs)
            .filter(|(v, t)| off.round_trip(**v).map(f32::to_bits) != **t)
            .count();
        println!("a frame one step off moves {moved} of 40 vertices");
        assert!(moved > 0);
    }
}

/// The round trip where it moves values. Upstream's tetrahedral mesh takes every node TetGen
/// wrote through it: `LoadNodeFile` reads each `.node` coordinate as a `double` narrowed to
/// `float` (`Convertor::ToFloat`) and converts it to OpenGL coordinates
/// (`Objet3D_maillage.cpp:93-98`), and `GetTetraMesh` converts it back (`:857`). Tutorial 1's
/// project keeps TetGen's output under `temp/`, so [`GlFrame`] over the project's own scene
/// (`sceneMesh.bin`) must turn `scene_mesh.1.node` into the nodes of both runs'
/// `tetramesh.mbin`, bit for bit. The scene mesh's own round trip, above, only turns `+0` into
/// `-0` on these tutorials; here it moves values. The refusal: the nodes only narrowed to `f32`.
#[test]
fn tutorial1_mbin_nodes_are_tetgens_nodes_through_the_round_trip() {
    let t = tutorial(TUTORIAL1);
    let archive = Archive::parse(&t.bytes).unwrap();
    let scene = read_scene_mesh(&archive.read("instance2/sceneMesh.bin").unwrap()).unwrap();
    let frame = GlFrame::of_vertices(scene.vertices.iter().copied()).unwrap();
    let node =
        tetgen::read_node(&archive.read("instance2/temp/scene_mesh.1.node").unwrap()).unwrap();
    let narrowed: Vec<[f32; 3]> = node.points.iter().map(|p| p.map(|c| c as f32)).collect();
    for run in &t.runs {
        let mesh = mbin::read(&run.tetra_bytes).unwrap();
        assert_eq!(mesh.nodes.len(), narrowed.len(), "{}", run.folder);
        let differing = |take: &dyn Fn([f32; 3]) -> [f32; 3]| -> (usize, usize) {
            let mut bits = 0;
            let mut values = 0;
            for (&n, m) in narrowed.iter().zip(&mesh.nodes) {
                let ours = take(n);
                bits += (0..3)
                    .filter(|&k| ours[k].to_bits() != m[k].to_bits())
                    .count();
                values += (0..3).filter(|&k| ours[k] != m[k]).count();
            }
            (bits, values)
        };
        let tripped = differing(&|n| frame.round_trip(n));
        let plain = differing(&|n| n);
        println!(
            "{}: {} nodes; through the round trip {} coordinates differ by bits; only narrowed \
             {} by bits, {} by value",
            run.folder,
            narrowed.len(),
            tripped.0,
            plain.0,
            plain.1
        );
        assert_eq!(tripped, (0, 0), "{}", run.folder);
        assert_eq!(plain.1, 229, "{}", run.folder);
    }
}

/// A box fitting zone flush with a wall lies exactly in that wall's plane. The mesher takes the
/// scene's vertices through the round trip (from [`scene_mesh`]) and a box's corners through the
/// same one, which works coordinate by coordinate, so equal coordinates come back equal. The case:
/// the tutorial box with its west wall moved from x = 0 to x = 0.37, a coordinate the round trip
/// moves in that scene's frame, and a box from x = 0.37. The refusal: the corner only narrowed to
/// `f32`, as the mesher wrote it before, is off the wall's plane.
#[test]
fn a_box_zone_flush_with_a_wall_lies_in_the_walls_plane() {
    let wall = 0.37;
    let mut project = load_project(TUTORIAL1_BOX);
    let mut moved = 0;
    for v in &mut project.geometry.vertices {
        if v.x.get() == 0.0 {
            v.x = F64::new(wall);
            moved += 1;
        }
    }
    assert_eq!(moved, 4, "the west wall's corners");
    let n = project.bands.frequencies_hz.len();
    project.fitting_zones.push(FittingZone {
        id: FittingZoneId::from_u128(0x0f17_0000_0000_4000_8000_0000_0000_0001),
        name: "Against the west wall".to_string(),
        enabled: true,
        shape: FittingShape::Box {
            min: Vec3::new(wall, 1.0, 0.5),
            max: Vec3::new(2.0, 2.0, 1.5),
        },
        absorption: vec![F64::new(0.1); n],
        mean_free_path_m: vec![F64::new(2.0); n],
        diffusion_law: vec![DiffusionLaw::Uniform; n],
    });
    let input = simpa_core::mesh::project_input(&project).unwrap();
    let scene = project.geometry.vertices.len();
    let near = |x: f64| (x - wall).abs() < 1e-5;
    let walls: BTreeSet<u64> = input.poly.model_vertices[..scene]
        .iter()
        .filter(|v| near(v[0]))
        .map(|v| v[0].to_bits())
        .collect();
    assert_eq!(walls.len(), 1, "the wall's corners share one x");
    let wall_x = f64::from_bits(*walls.first().unwrap());
    let corners: Vec<f64> = input.poly.model_vertices[scene..]
        .iter()
        .map(|v| v[0])
        .filter(|&x| near(x))
        .collect();
    println!(
        "wall at x = {wall}: the .poly holds {wall_x:?}; the box's min-x corners {corners:?}; \
         narrowed only, {:?}",
        f64::from(wall as f32)
    );
    assert_eq!(corners.len(), 4, "the box's corners at its min x");
    assert!(corners.iter().all(|x| x.to_bits() == wall_x.to_bits()));
    assert_ne!(
        f64::from(wall as f32).to_bits(),
        wall_x.to_bits(),
        "the round trip must move the wall here, or this case shows nothing"
    );
}

// ---------------------------------------------------------------------------------------------
// What the solvers make of the differences left
//
// Upstream's SPPS and TCR, our M1 builds (`$SIMPA_SOLVERS_DIR`, `common/paths.rs`), each run in
// a fresh folder under `target/test-runs/config_xml/` (kept), once on upstream's inputs and once
// with ours in their place, with the same seed. The outputs must be the same files, byte for
// byte, apart from the ids the differences carry.

fn solver(kind: SolverKind) -> std::path::PathBuf {
    support::solver_exe(match kind {
        SolverKind::Spps => "spps.exe",
        SolverKind::Tcr => "classicalTheory.exe",
    })
}

/// A stored run's configuration made repeatable, as `tests/fixtures/upstream/tutorial1/`
/// (`PROVENANCE.md`) makes it: SPPS's `random_seed` 0 (seeded from the clock, one thread per band,
/// so no two runs agree) becomes 1, and `nbparticules` becomes 10,000, so a run takes seconds.
/// Every run of a comparison gets the same edits. TCR draws no random numbers: unchanged.
fn repeatable(config: &str, kind: SolverKind) -> String {
    match kind {
        SolverKind::Spps => {
            let seeded = edit_attr(config, "<simulation ", "random_seed", "1");
            edit_attr(&seeded, "<simulation ", "nbparticules", "10000")
        }
        SolverKind::Tcr => config.to_string(),
    }
}

/// Runs `kind` in a fresh folder on `config` (its working directory set to that folder), the
/// scene mesh `cbin` and the tetrahedral mesh `mbin`, and returns its output files. It must exit
/// 0 and print its completion line.
fn run_on(
    label: &str,
    kind: SolverKind,
    config: &str,
    cbin: &[u8],
    mbin: &[u8],
) -> BTreeMap<String, Vec<u8>> {
    let dir = support::fresh_run_dir(label);
    // The solvers open plain paths: a file whose full path reaches 260 characters is skipped
    // with exit 0 and no message (config_xml_solver.rs). Leave room for the deepest output.
    let longest = dir.as_os_str().len()
        + "/Punctual receivers/Receiver 1/Punctual receiver intensity.gabe".len();
    assert!(longest < 240, "{} is too deep for MAX_PATH", dir.display());
    let wd = simpa_core::config_xml::working_directory(&dir).unwrap();
    let text = edit_attr(config, "<configuration ", "workingdirectory", &wd);
    std::fs::write(dir.join("config.xml"), text).unwrap();
    std::fs::write(dir.join("mesh.cbin"), cbin).unwrap();
    std::fs::write(dir.join("tetramesh.mbin"), mbin).unwrap();
    let run = support::run_solver(&solver(kind), &dir);
    assert_eq!(run.exit, Some(0), "{}: {}", dir.display(), run.stderr);
    let done = match kind {
        SolverKind::Spps => "End of calculation.",
        SolverKind::Tcr => "Step 3/3",
    };
    assert!(run.has_line(done), "{}:\n{}", dir.display(), run.stdout);
    support::outputs(&dir, &["config.xml", "mesh.cbin", "tetramesh.mbin"])
}

/// The output files that differ between two runs, as sorted lines: a file only one run wrote, or
/// one whose bytes differ. A `.csbin` is compared decoded (its padding differs from run to run,
/// M1), with upstream's surface-receiver ids first mapped to ours through `rs_ids`.
fn output_differences(
    theirs: &BTreeMap<String, Vec<u8>>,
    ours: &BTreeMap<String, Vec<u8>>,
    rs_ids: &BTreeMap<i32, i32>,
) -> Vec<String> {
    let mut out = Vec::new();
    for k in theirs.keys().filter(|k| !ours.contains_key(*k)) {
        out.push(format!("{k}: only upstream's run"));
    }
    for k in ours.keys().filter(|k| !theirs.contains_key(*k)) {
        out.push(format!("{k}: only ours"));
    }
    for (k, t) in theirs {
        let Some(o) = ours.get(k) else { continue };
        let same = if k.ends_with(".csbin") {
            let mut t = csbin::read(t).unwrap();
            for r in &mut t.receivers {
                r.xml_index = *rs_ids.get(&r.xml_index).unwrap_or(&r.xml_index);
            }
            csbin::dump(&t) == csbin::dump(&csbin::read(o).unwrap())
        } else {
            t == o
        };
        if !same {
            out.push(format!("{k}: differs"));
        }
    }
    out.sort();
    out
}

/// Tutorial 1, the `.proj` imported: our scene mesh welds upstream's 36 vertices into 8
/// (`docs/formats/cbin.md`, "What still differs", 2). Upstream's own run configuration
/// ([`repeatable`]) and tetrahedral mesh, run once with upstream's `mesh.cbin` and once with
/// ours (its receiver id 0 written as upstream's 3503, which the configuration names), give the
/// same output files byte for byte, in SPPS and in TCR: the solvers see the same triangles. The
/// refusal: ours with one face's material changed (22 to 21, both declared) gives different
/// output.
#[test]
fn the_welded_scene_mesh_gives_upstreams_output() {
    let t = tutorial(TUTORIAL1);
    let project = import_proj(&t.bytes).unwrap().project;
    let mut ours = scene_mesh(&project).unwrap();
    assert_eq!(ours.vertices.len(), 8);
    for f in &mut ours.faces {
        if f.id_rs == 0 {
            f.id_rs = 3503;
        }
    }
    let mut other_material = ours.clone();
    assert_eq!(other_material.faces[3].id_mat, 22);
    other_material.faces[3].id_mat = 21;
    let none = BTreeMap::new();
    for run in &t.runs {
        let kind = if run.solver == SolverKind::Spps {
            "spps"
        } else {
            "tcr"
        };
        let config = &repeatable(&run.config, run.solver);
        let mbin = &run.tetra_bytes;
        let theirs = run_on(
            &format!("weld-{kind}-t"),
            run.solver,
            config,
            &run.mesh_bytes,
            mbin,
        );
        let welded = run_on(
            &format!("weld-{kind}-o"),
            run.solver,
            config,
            &cbin::write(&ours),
            mbin,
        );
        let got = output_differences(&theirs, &welded, &none);
        println!(
            "{} ({:?}): {} output files; with our welded mesh.cbin {} differ",
            run.folder,
            run.solver,
            theirs.len(),
            got.len()
        );
        assert!(!theirs.is_empty());
        assert_eq!(got, Vec::<String>::new(), "{}", run.folder);

        let changed = run_on(
            &format!("weld-{kind}-x"),
            run.solver,
            config,
            &cbin::write(&other_material),
            mbin,
        );
        let refused = output_differences(&theirs, &changed, &none);
        println!(
            "  face 3 with material 21: {} differ {refused:?}",
            refused.len()
        );
        assert!(!refused.is_empty(), "{}", run.folder);
    }
}

/// Tutorial 3, each stored run: its config with [`importable`]'s two edits, made [`repeatable`],
/// is run as upstream wrote it, with the run's `mesh.cbin` and `tetramesh.mbin`; then imported
/// with its scene mesh and written back, and ours is run with our `config.xml` and `mesh.cbin`.
/// The inputs differ only in the ids and the differences by design
/// ([`tutorial3_ids_and_by_design`]). Upstream's `.mbin` carries the fitting zones as 2083 and
/// 1930; ours is given the same tetrahedra with those written as our 3 and 2, as our mesher
/// writes a fitting's tetrahedra. (The room's keep upstream's 2084 to 2086, which name no
/// fitting.) Every output file is upstream's, byte for byte, the cutting plane's `.csbin` once
/// its id 951 is read as our 0: the ids reach no output but that one. The refusal: ours with the
/// first fitting zone absorbing 0.9 in every band gives different output.
#[test]
fn tutorial3_written_back_gives_upstreams_output() {
    let t = tutorial(TUTORIAL3);
    for (i, run) in t.runs.iter().enumerate() {
        let config = repeatable(&importable(&run.config), run.solver);
        let project = import_upstream_with_mesh(&config, &run.mesh).unwrap();
        let wd = working_folder(&config);
        let ours = write(&project, run.solver, None, Path::new(&wd)).unwrap();
        assert_eq!(
            solver_differences(&config, &ours),
            sorted(tutorial3_ids_and_by_design()),
            "{}",
            run.folder
        );
        let id_en = id_map(&config, &ours, "encombrement");
        let id_rs = id_map(&config, &ours, "recepteur_surfacique_coupe");
        assert_eq!(id_rs, BTreeMap::from([(951, 0)]));
        let mut tetra = mbin::read(&run.tetra_bytes).unwrap();
        let volumes: BTreeSet<i32> = tetra.tetrahedra.iter().map(|t| t.id_volume).collect();
        println!("{}: idVolume {volumes:?}", run.folder);
        assert!(volumes.contains(&2083) && volumes.contains(&1930));
        assert!(
            !volumes.contains(&2) && !volumes.contains(&3),
            "our ids must not name another volume"
        );
        for t in &mut tetra.tetrahedra {
            if let Some(&o) = id_en.get(&t.id_volume) {
                t.id_volume = o;
            }
        }
        let tetra = mbin::write(&tetra);
        let our_mesh = cbin::write(&scene_mesh(&project).unwrap());

        let theirs = run_on(
            &format!("t3-{i}-t"),
            run.solver,
            &config,
            &run.mesh_bytes,
            &run.tetra_bytes,
        );
        let mine = run_on(&format!("t3-{i}-o"), run.solver, &ours, &our_mesh, &tetra);
        let got = output_differences(&theirs, &mine, &id_rs);
        println!(
            "{} ({:?}): {} output files; ours {} differ {got:?}",
            run.folder,
            run.solver,
            theirs.len(),
            got.len()
        );
        assert!(!theirs.is_empty());
        assert_eq!(got, Vec::<String>::new(), "{}", run.folder);
        // The one id that reaches an output, as each side wrote it.
        let cut = "Surface receiver/Global/rs_cut.csbin";
        let index = |files: &BTreeMap<String, Vec<u8>>| -> Vec<i32> {
            let c = csbin::read(&files[cut]).unwrap();
            c.receivers.iter().map(|r| r.xml_index).collect()
        };
        assert_eq!((index(&theirs), index(&mine)), (vec![951], vec![0]));

        if i == 0 {
            let mut denser = project.clone();
            for a in &mut denser.fitting_zones[0].absorption {
                *a = F64::new(0.9);
            }
            let x = write(&denser, run.solver, None, Path::new(&wd)).unwrap();
            let changed = run_on(&format!("t3-{i}-x"), run.solver, &x, &our_mesh, &tetra);
            let refused = output_differences(&theirs, &changed, &id_rs);
            println!(
                "  first fitting zone absorbing 0.9: {} differ",
                refused.len()
            );
            assert!(!refused.is_empty(), "{}", run.folder);
        }
    }
}
