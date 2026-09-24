//! What the parity tests share: upstream's tutorial projects and their stored run folders, and
//! the comparisons that define "our input equals original I-Simpa's" for `config.xml` and
//! `mesh.cbin`. Included with `#[path]` by `parity_inputs.rs` (this crate) and by the parity bed,
//! `crates/simpa/tests/parity_tutorials.rs`; built on its own it is an empty test target.
//!
//! The projects are read from the upstream source tree at the pinned commit (`common/paths.rs`:
//! `$SIMPA_UPSTREAM`, else the tree `solvers/build.ps1` extracts). A missing tree panics, naming
//! where it looked.
#![allow(dead_code)]

use std::collections::{BTreeMap, BTreeSet};

use simpa_core::config_xml::SolverKind;
use simpa_core::formats::cbin;
use simpa_core::geometry::import::zip::{Archive, crc32};

#[path = "config_xml_support.rs"]
pub mod support;

use support::{Value, doc_attrs, doc_ignored, parse_value, solver_view, upstream_file};

/// The one file of a tutorial project whose name ends with `suffix` (`temp/scene_mesh.poly`),
/// read from its zip; panics when there is not exactly one.
pub fn entry(t: &Tutorial, suffix: &str) -> Vec<u8> {
    let archive = Archive::parse(&t.bytes).unwrap();
    let names: Vec<String> = archive
        .entries()
        .iter()
        .filter(|e| e.name.ends_with(suffix))
        .map(|e| e.name.clone())
        .collect();
    assert_eq!(names.len(), 1, "entries ending with {suffix}: {names:?}");
    archive.read(&names[0]).unwrap()
}

/// A stored (uncompressed) zip archive of `entries`, in order, which `Archive` reads: a test
/// input made from a tutorial with some entries replaced.
pub fn stored_zip(entries: &[(String, Vec<u8>)]) -> Vec<u8> {
    let mut out: Vec<u8> = Vec::new();
    let mut cd: Vec<u8> = Vec::new();
    let u16le = |v: usize| u16::try_from(v).unwrap().to_le_bytes();
    let u32le = |v: usize| u32::try_from(v).unwrap().to_le_bytes();
    for (name, data) in entries {
        let offset = out.len();
        let crc = crc32(data).to_le_bytes();
        let n = name.as_bytes();
        // Local header: signature, version 2.0, no flags, method 0 (stored), no time or date.
        out.extend(0x0403_4b50u32.to_le_bytes());
        out.extend(u16le(20));
        out.extend([0u8; 8]);
        out.extend(crc);
        out.extend(u32le(data.len()));
        out.extend(u32le(data.len()));
        out.extend(u16le(n.len()));
        out.extend(u16le(0));
        out.extend(n);
        out.extend(data);
        // Central directory record.
        cd.extend(0x0201_4b50u32.to_le_bytes());
        cd.extend(u16le(20));
        cd.extend(u16le(20));
        cd.extend([0u8; 8]);
        cd.extend(crc);
        cd.extend(u32le(data.len()));
        cd.extend(u32le(data.len()));
        cd.extend(u16le(n.len()));
        cd.extend([0u8; 12]);
        cd.extend(u32le(offset));
        cd.extend(n);
    }
    let cd_offset = out.len();
    out.extend(&cd);
    out.extend(0x0605_4b50u32.to_le_bytes());
    out.extend([0u8; 4]);
    out.extend(u16le(entries.len()));
    out.extend(u16le(entries.len()));
    out.extend(u32le(cd.len()));
    out.extend(u32le(cd_offset));
    out.extend(u16le(0));
    out
}

/// A tutorial's archive with its entry `name` replaced by `edit` of it, every other entry kept,
/// written stored ([`stored_zip`]).
pub fn with_entry(t: &Tutorial, name: &str, edit: impl Fn(&[u8]) -> Vec<u8>) -> Vec<u8> {
    let archive = Archive::parse(&t.bytes).unwrap();
    let mut found = 0;
    let entries: Vec<(String, Vec<u8>)> = archive
        .entries()
        .iter()
        .map(|e| {
            let data = archive.read_entry(e).unwrap();
            if e.name == name {
                found += 1;
                (e.name.clone(), edit(&data))
            } else {
                (e.name.clone(), data)
            }
        })
        .collect();
    assert_eq!(found, 1, "{name}");
    stored_zip(&entries)
}

/// Whether a tutorial project holds a file whose name ends with `suffix`.
pub fn has_entry(t: &Tutorial, suffix: &str) -> bool {
    let archive = Archive::parse(&t.bytes).unwrap();
    archive.entries().iter().any(|e| e.name.ends_with(suffix))
}

pub const TUTORIAL1: &str = r"src/isimpa/resources/doc/tutorial/tutorial 1/tutorial_1.proj";

pub const TUTORIAL2: &str = r"src/isimpa/resources/doc/tutorial/tutorial 2/tutorial_2.proj";

pub const TUTORIAL3: &str = r"src/isimpa/resources/doc/tutorial/tutorial 3/tutorial_3.proj";

pub const INDUSTRIAL: &str = r"src/isimpa/resources/doc/tutorial/tutorial 3/Industrial.proj";

/// One run folder of a tutorial project: what upstream's GUI handed the solver.
pub struct Run {
    pub folder: String,
    pub solver: SolverKind,
    pub config: String,
    pub mesh_bytes: Vec<u8>,
    pub mesh: cbin::Model,
    /// The run's `tetramesh.mbin`.
    pub tetra_bytes: Vec<u8>,
    /// The project file the GUI saved beside the run (`projet_config.xml`).
    pub project_file: String,
}

pub struct Tutorial {
    pub bytes: Vec<u8>,
    pub runs: Vec<Run>,
}

pub fn tutorial(rel: &str) -> Tutorial {
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
pub fn working_folder(config: &str) -> String {
    let doc = roxmltree::Document::parse(config).unwrap();
    let wd = doc.root_element().attribute("workingdirectory").unwrap();
    assert!(wd.ends_with('\\'), "{wd}");
    wd.to_string()
}

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
pub fn solver_differences(theirs: &str, ours: &str) -> Vec<String> {
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
pub fn by_design(solver: SolverKind) -> Vec<String> {
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

/// Tutorial 1's ids through its `.proj`'s own `projet_config.xml`: an import pins each entity to
/// its `wxid` there (`docs/m5-m6-design.md`, decision 13), the ids of the session that last saved
/// the project, while each run carries those of the 2019 session that wrote it (upstream numbers
/// elements by its GUI's session counters and renumbers them on every load). A run's own
/// snapshot, imported, gives the run's ids exactly.
pub fn tutorial1_proj_ids() -> Vec<String> {
    vec![
        "recepteursp/recepteur_ponctuel[0]@id: upstream 3669, ours 1632".to_string(),
        "recepteursp/recepteur_ponctuel[1]@id: upstream 3510, ours 1473".to_string(),
        "recepteurss/recepteur_surfacique[0]@id: upstream 3503, ours 1792".to_string(),
    ]
}

/// Tutorial 1's ids for a project that pins none (its `config.xml` imported): upstream numbers
/// elements by its GUI's session counters, we by project order (config_xml's module docs).
/// Receivers are written last first by both, so the first one written is our receiver 1.
pub fn tutorial1_ids() -> Vec<String> {
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
pub fn tutorial1_directions() -> Vec<String> {
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

pub fn sorted(mut v: Vec<String>) -> Vec<String> {
    v.sort();
    v
}

/// The text of the next `f32` above the value a real attribute holds, as the solver reads it.
pub fn next_float_text(text: &str) -> String {
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
pub fn edit_attr(xml: &str, element_start: &str, attr: &str, value: &str) -> String {
    let at = xml.find(element_start).expect("element");
    let key = format!(" {attr}=\"");
    let a = at + xml[at..].find(&key).expect("attribute") + key.len();
    let b = a + xml[a..].find('"').unwrap();
    format!("{}{value}{}", &xml[..a], &xml[b..])
}

pub fn attr_in<'a>(xml: &'a str, element_start: &str, attr: &str) -> &'a str {
    let at = xml.find(element_start).expect("element");
    let key = format!(" {attr}=\"");
    let a = at + xml[at..].find(&key).expect("attribute") + key.len();
    let b = a + xml[a..].find('"').unwrap();
    &xml[a..b]
}

/// `xml` without the attribute `attr` of the first element matching `element_start`.
pub fn drop_attr(xml: &str, element_start: &str, attr: &str) -> String {
    let at = xml.find(element_start).expect("element");
    let key = format!(" {attr}=\"");
    let a = at + xml[at..].find(&key).expect("attribute");
    let b = a + key.len() + xml[a + key.len()..].find('"').unwrap() + 1;
    format!("{}{}", &xml[..a], &xml[b..])
}

/// What differs by design between a tutorial 3 run's config and ours from the `.proj` read with
/// that run's saved project, whose ids are upstream's, pinned (decision 13), so no id differs
/// (`docs/formats/config_xml.md`, "Parity with upstream's GUI").
pub fn tutorial3_by_design() -> Vec<String> {
    let mut expected = vec![
        // The GUI's section of the project tree for volumes, which no solver looks up.
        "other:subdomains: only upstream's".to_string(),
        // Optional attributes upstream's GUI never writes, written at the solver's default (1).
        "simulation@save_receivers_intersection: only ours (1)".to_string(),
        "simulation@save_surface_intersection: only ours (1)".to_string(),
    ];
    // Read only for source types 1 and 5; every source here is omni.
    for i in 0..6 {
        for a in ["u", "v", "w"] {
            expected.push(format!("sources/source[{i}]@{a}: only upstream's (1)"));
        }
    }
    expected
}

/// What differs between a tutorial 3 run's config and ours written back from it (imported with
/// `config_xml::import_upstream_with_mesh`): the ids and the differences by design.
pub fn tutorial3_ids_and_by_design() -> Vec<String> {
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

/// The byte range of `<type_surface id="{id}" ...>` up to its closing tag.
pub fn material_span(xml: &str, id: &str) -> (usize, usize) {
    let start = xml
        .find(&format!("<type_surface id=\"{id}\""))
        .expect("material");
    (start, start + xml[start..].find("</type_surface>").unwrap())
}

/// Differences the solvers would see between upstream's scene mesh and ours, face by face: the
/// face count, each face's three corners as `f32` bits in its own order (a, b, c), and its
/// `idMat`, and `idRs` and `idEn` through the id maps (upstream's id to ours). The solvers use a
/// scene vertex only as a corner of the faces that name it (`coreinitialisation.cpp:417-422`,
/// `CalculationCore.cpp:524-526`, `sppsInitialisation.cpp:99-101`, `TC_CalculationCore.cpp:33,
/// 97, 117, 127, 524-526`), so a vertex list welded differently is the same input.
pub fn face_differences(
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

pub fn vertex_set(m: &cbin::Model) -> BTreeSet<[u32; 3]> {
    m.vertices
        .iter()
        .map(|v| [v.x.to_bits(), v.y.to_bits(), v.z.to_bits()])
        .collect()
}

/// Upstream's id to ours, for every element of `tag` in the two configs, by position: both write
/// their lists in the same order (the config tests check it).
pub fn id_map(theirs: &str, ours: &str, tag: &str) -> BTreeMap<i32, i32> {
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

/// A stored run's configuration made repeatable, as `tests/fixtures/upstream/tutorial1/`
/// (`PROVENANCE.md`) makes it: SPPS's `random_seed` 0 (seeded from the clock, one thread per band,
/// so no two runs agree) becomes 1, and `nbparticules` becomes 10,000, so a run takes seconds.
/// Every run of a comparison gets the same edits. TCR draws no random numbers: unchanged.
pub fn repeatable(config: &str, kind: SolverKind) -> String {
    match kind {
        SolverKind::Spps => {
            let seeded = edit_attr(config, "<simulation ", "random_seed", "1");
            edit_attr(&seeded, "<simulation ", "nbparticules", "10000")
        }
        SolverKind::Tcr => config.to_string(),
    }
}
