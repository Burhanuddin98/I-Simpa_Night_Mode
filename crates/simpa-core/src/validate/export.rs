//! The `export` rules of `docs/solver-contract.md` Part A: checks on the exact files the exporter
//! wrote to a run folder, immediately before launch.

use std::collections::{HashMap, HashSet};
use std::path::{Component, Path};

use roxmltree::{Document, Node};

use super::codes::*;
use super::geometry::{self, PointLocation};
use super::{Issue, MAX_PATH_UTF16, issue};
use crate::formats::{cbin, mbin};
use crate::schema::{FittingShape, Project, SolverKind};

/// The configuration file a run folder holds; the solver is launched with this one argument.
pub const CONFIG_FILE_NAME: &str = "config.xml";

/// How a solver reads an attribute (`docs/formats/config_xml.md`, "How the solvers parse the
/// file").
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AttrKind {
    /// A string, used verbatim.
    Text,
    /// `atoi` into a C `int`.
    Int,
    /// `atof` into a C `float`.
    Real,
    /// An `int` whose valid values are this inclusive range (an enumeration).
    IntRange(i32, i32),
}

/// When our writer must write an attribute: `docs/formats/config_xml.md`'s Writer column.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Writer {
    /// In both SPPS and TCR configs.
    Always,
    /// In SPPS's config only.
    Spps,
    /// In TCR's config only.
    Tcr,
    /// On every element of its kind, which exists only when the project has one: "if surface
    /// receivers", "if cutting planes", "if fittings".
    IfElement,
    /// On a source whose `directivite` is 1 or 5 ("if type 1 or 5").
    IfDirected,
    /// On a source whose `directivite` is 5 ("if type 5").
    IfBalloon,
    /// On a material band that transmits ("if the material transmits"); its absence means no
    /// transmission, so it is never missing.
    IfTransmits,
    /// Never written.
    Never,
}

/// One attribute that at least one solver reads, as `docs/formats/config_xml.md` lists it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ConfigAttribute {
    /// The element part of the page's key: `simulation`, `source/bfreq`, `freq_enum/bfreq`, ...
    pub element: &'static str,
    pub name: &'static str,
    pub kind: AttrKind,
    pub writer: Writer,
}

impl ConfigAttribute {
    /// The page's key, `<element>@<attribute>`.
    pub fn key(&self) -> String {
        format!("{}@{}", self.element, self.name)
    }
}

const fn attr(
    element: &'static str,
    name: &'static str,
    kind: AttrKind,
    writer: Writer,
) -> ConfigAttribute {
    ConfigAttribute {
        element,
        name,
        kind,
        writer,
    }
}

use AttrKind::{Int, IntRange, Real, Text};
use Writer::{
    Always, IfBalloon, IfDirected, IfElement, IfTransmits, Never, Spps as SppsOnly, Tcr as TcrOnly,
};

const ROOT: &str = "configuration";
const ATMO: &str = "condition_atmospherique";
const SIM: &str = "simulation";
const FREQ: &str = "freq_enum/bfreq";
const SRC: &str = "source";
const SRC_BAND: &str = "source/bfreq";
const MAT: &str = "type_surface";
const MAT_BAND: &str = "type_surface/bfreq";
const RCV: &str = "recepteur_ponctuel";
const RCV_BAND: &str = "recepteur_ponctuel/bfreq";
const RS: &str = "recepteur_surfacique";
const CUT: &str = "recepteur_surfacique_coupe";
const FIT: &str = "encombrement";
const FIT_BAND: &str = "encombrement/bfreq";

/// Every attribute of `docs/formats/config_xml.md`'s reference tables, in the page's order: 94.
pub const CONFIG_ATTRIBUTES: [ConfigAttribute; 94] = [
    attr(ROOT, "workingdirectory", Text, Always),
    attr(ATMO, "temperature", Real, Always),
    attr(ATMO, "pression", Real, Always),
    attr(ATMO, "humidite", Real, Always),
    attr(ATMO, "z0", Real, Always),
    attr(ATMO, "alog", Real, Always),
    attr(ATMO, "blin", Real, Always),
    attr(ATMO, "disable_absatmo_computation", Int, Always),
    attr(ATMO, "absatmo", Real, Always),
    attr(SIM, "modelName", Text, Always),
    attr(SIM, "tetrameshFileName", Text, Always),
    attr(SIM, "pasdetemps", Real, SppsOnly),
    attr(SIM, "duree_simulation", Real, SppsOnly),
    attr(SIM, "directivities_directory", Text, Always),
    attr(SIM, "recepteurss_directory", Text, Always),
    attr(SIM, "recepteurss_filename", Text, Always),
    attr(SIM, "recepteurss_cut_filename", Text, Always),
    attr(SIM, "receiversp_directory", Text, Always),
    attr(SIM, "receiversp_filename", Text, Always),
    attr(SIM, "receiversp_filename_adv", Text, Always),
    attr(SIM, "cumul_filename", Text, Always),
    attr(FREQ, "freq", Int, Always),
    attr(FREQ, "docalc", Text, Always),
    attr(SIM, "particules_directory", Text, SppsOnly),
    attr(SIM, "particules_filename", Text, SppsOnly),
    attr(SIM, "stats_filename", Text, SppsOnly),
    attr(SIM, "nbparticules", Int, SppsOnly),
    attr(SIM, "nbparticules_rendu", Int, SppsOnly),
    attr(SIM, "abs_atmo_calc", Int, Always),
    attr(SIM, "output_recp_bysource", Int, SppsOnly),
    attr(SIM, "random_seed", Int, SppsOnly),
    attr(SIM, "save_surface_intersection", Int, SppsOnly),
    attr(SIM, "save_receivers_intersection", Int, SppsOnly),
    attr(SIM, "direct_calc", Int, SppsOnly),
    attr(SIM, "enc_calc", Int, SppsOnly),
    attr(SIM, "computation_method", IntRange(0, 1), SppsOnly),
    attr(SIM, "rayon_recepteurp", Real, SppsOnly),
    attr(SIM, "trans_epsilon", Real, SppsOnly),
    attr(SIM, "trans_calc", Int, SppsOnly),
    attr(SIM, "output_recs_byfreq", Int, Always),
    attr(SIM, "surf_receiv_method", IntRange(0, 1), SppsOnly),
    attr(SIM, "direct_recepteurSOutputName", Text, TcrOnly),
    attr(SIM, "sabine_recepteurSOutputName", Text, TcrOnly),
    attr(SIM, "eyring_recepteurSOutputName", Text, TcrOnly),
    attr(SIM, "output_folder", Text, Never),
    attr(SIM, "do_angular_weighting", Int, Never),
    attr(SRC, "x", Real, Always),
    attr(SRC, "y", Real, Always),
    attr(SRC, "z", Real, Always),
    attr(SRC, "directivite", IntRange(0, 5), Always),
    attr(SRC, "u", Real, IfDirected),
    attr(SRC, "v", Real, IfDirected),
    attr(SRC, "w", Real, IfDirected),
    attr(SRC, "delay", Real, Always),
    attr(SRC, "name", Text, Always),
    attr(SRC, "directivity_file", Text, IfBalloon),
    attr(SRC_BAND, "db", Real, Always),
    attr(SRC_BAND, "freq", Int, Always),
    attr(MAT, "id", Int, Always),
    attr(MAT, "side_material", Int, Always),
    attr(MAT_BAND, "absorb", Real, Always),
    attr(MAT_BAND, "diffusion", Real, Always),
    attr(MAT_BAND, "loi", IntRange(0, 5), Always),
    attr(MAT_BAND, "affaiblissement", Real, IfTransmits),
    attr(MAT_BAND, "freq", Int, Always),
    attr(RCV, "x", Real, Always),
    attr(RCV, "y", Real, Always),
    attr(RCV, "z", Real, Always),
    attr(RCV, "id", Int, Always),
    attr(RCV, "lbl", Text, Always),
    attr(RCV, "u", Real, Always),
    attr(RCV, "v", Real, Always),
    attr(RCV, "w", Real, Always),
    attr(RCV_BAND, "db", Real, Always),
    attr(RCV_BAND, "freq", Int, Always),
    attr(RS, "id", Int, IfElement),
    attr(RS, "name", Text, IfElement),
    attr(CUT, "id", Int, IfElement),
    attr(CUT, "name", Text, IfElement),
    attr(CUT, "ax", Real, IfElement),
    attr(CUT, "ay", Real, IfElement),
    attr(CUT, "az", Real, IfElement),
    attr(CUT, "bx", Real, IfElement),
    attr(CUT, "by", Real, IfElement),
    attr(CUT, "bz", Real, IfElement),
    attr(CUT, "cx", Real, IfElement),
    attr(CUT, "cy", Real, IfElement),
    attr(CUT, "cz", Real, IfElement),
    attr(CUT, "resolution", Real, IfElement),
    attr(FIT, "id", Int, IfElement),
    attr(FIT_BAND, "alpha", Real, IfElement),
    attr(FIT_BAND, "lambda", Real, IfElement),
    attr(FIT_BAND, "loi_diff", IntRange(0, 2), IfElement),
    attr(FIT_BAND, "freq", Int, IfElement),
];

/// `atoi`: leading C spaces, an optional sign, then digits; 0 when there are none. Saturates
/// instead of overflowing (the C behaviour is undefined there).
fn atoi(s: &str) -> i64 {
    let s = s.trim_start_matches([' ', '\t', '\n', '\r', '\x0b', '\x0c']);
    let (neg, digits) = match s.as_bytes().first() {
        Some(b'-') => (true, &s[1..]),
        Some(b'+') => (false, &s[1..]),
        _ => (false, s),
    };
    let mut v: i64 = 0;
    for b in digits.bytes().take_while(u8::is_ascii_digit) {
        v = v.saturating_mul(10).saturating_add(i64::from(b - b'0'));
    }
    if neg { -v } else { v }
}

/// A plain C-locale integer literal that fits a C `int`.
fn plain_int(s: &str) -> Option<i32> {
    let digits = s.strip_prefix(['+', '-']).unwrap_or(s);
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    s.parse::<i64>().ok().and_then(|v| i32::try_from(v).ok())
}

/// A plain C-locale real literal (`.` as the decimal point, no grouping, optional exponent) that
/// is finite once read into a C `float`.
fn plain_real(s: &str) -> Option<f32> {
    let body = s.strip_prefix(['+', '-']).unwrap_or(s);
    let (mantissa, exponent) = match body.find(['e', 'E']) {
        Some(i) => (&body[..i], Some(&body[i + 1..])),
        None => (body, None),
    };
    let (int_part, frac_part) = match mantissa.split_once('.') {
        Some((a, b)) => (a, b),
        None => (mantissa, ""),
    };
    let digits = |t: &str| t.bytes().all(|b| b.is_ascii_digit());
    let mantissa_ok =
        digits(int_part) && digits(frac_part) && !(int_part.is_empty() && frac_part.is_empty());
    let exponent_ok = exponent.is_none_or(|e| {
        let e = e.strip_prefix(['+', '-']).unwrap_or(e);
        !e.is_empty() && digits(e)
    });
    if !(mantissa_ok && exponent_ok) {
        return None;
    }
    let v = s.parse::<f64>().ok()? as f32;
    v.is_finite().then_some(v)
}

/// The child nodes the solver iterates as list items: every element, and any text that is not
/// whitespace (pugixml keeps it as a nameless node, which becomes an item whose every value is 0).
fn items<'a, 'i>(node: Node<'a, 'i>) -> Vec<Node<'a, 'i>> {
    node.children()
        .filter(|c| c.is_element() || (c.is_text() && !c.text().unwrap_or("").trim().is_empty()))
        .collect()
}

/// The first child element with this name (`CXmlNode::GetChild`).
fn child<'a, 'i>(node: Node<'a, 'i>, name: &str) -> Option<Node<'a, 'i>> {
    node.children()
        .find(|c| c.is_element() && c.tag_name().name() == name)
}

/// An XPath-like location: element names from the root, with a 1-based index where a name
/// repeats among siblings.
fn xpath(node: Node) -> String {
    let mut parts = Vec::new();
    let mut current = Some(node);
    while let Some(n) = current {
        if n.is_root() {
            break;
        }
        let name = if n.is_element() {
            n.tag_name().name().to_string()
        } else {
            "text()".to_string()
        };
        let same = |s: &Node| {
            (n.is_element() && s.is_element() && s.tag_name().name() == name)
                || (!n.is_element() && s.is_text())
        };
        // roxmltree's sibling iterators start with the node itself.
        let index = n.prev_siblings().skip(1).filter(|s| same(s)).count();
        let total = index + 1 + n.next_siblings().skip(1).filter(|s| same(s)).count();
        if total > 1 {
            parts.push(format!("{name}[{}]", index + 1))
        } else {
            parts.push(name);
        }
        current = n.parent();
    }
    parts.reverse();
    format!("/{}", parts.join("/"))
}

fn at(node: Node, attribute: &str) -> String {
    format!("{CONFIG_FILE_NAME}:{}@{attribute}", xpath(node))
}

/// The parsed config: the nodes each rule looks at.
struct Cfg<'a, 'i> {
    root: Node<'a, 'i>,
    atmo: Option<Node<'a, 'i>>,
    sim: Option<Node<'a, 'i>>,
    bands: Vec<Node<'a, 'i>>,
    sources: Vec<Node<'a, 'i>>,
    materials: Vec<Node<'a, 'i>>,
    receivers: Vec<Node<'a, 'i>>,
    surface_receivers: Vec<Node<'a, 'i>>,
    cutting_planes: Vec<Node<'a, 'i>>,
    fittings: Vec<Node<'a, 'i>>,
}

impl<'a, 'i> Cfg<'a, 'i> {
    fn new(doc: &'a Document<'i>) -> Self {
        let root = doc.root_element();
        let sim = child(root, "simulation");
        let list = |name: &str| child(root, name).map(items).unwrap_or_default();
        let (surface_receivers, cutting_planes) = match child(root, "recepteurss") {
            Some(rs) => {
                let named = |n: &str| {
                    rs.children()
                        .filter(|c| c.is_element() && c.tag_name().name() == n)
                        .collect::<Vec<_>>()
                };
                (named(RS), named(CUT))
            }
            None => (Vec::new(), Vec::new()),
        };
        Cfg {
            root,
            atmo: child(root, "condition_atmospherique"),
            sim,
            bands: sim
                .and_then(|s| child(s, "freq_enum"))
                .map(items)
                .unwrap_or_default(),
            sources: list("sources"),
            materials: list("surface_absorption_enum"),
            receivers: list("recepteursp"),
            surface_receivers,
            cutting_planes,
            fittings: list("encombrement_enum"),
        }
    }

    fn sim_attr(&self, name: &str) -> &'a str {
        self.sim.and_then(|s| s.attribute(name)).unwrap_or("")
    }

    /// Every element the attribute table applies to, with its table key prefix.
    fn elements(&self) -> Vec<(Node<'a, 'i>, &'static str)> {
        let mut all = vec![(self.root, ROOT)];
        all.extend(self.atmo.map(|n| (n, ATMO)));
        all.extend(self.sim.map(|n| (n, SIM)));
        all.extend(self.bands.iter().map(|&n| (n, FREQ)));
        for (list, element, band) in [
            (&self.sources, SRC, SRC_BAND),
            (&self.materials, MAT, MAT_BAND),
            (&self.receivers, RCV, RCV_BAND),
            (&self.fittings, FIT, FIT_BAND),
        ] {
            for &n in list {
                all.push((n, element));
                all.extend(items(n).into_iter().map(|b| (b, band)));
            }
        }
        all.extend(self.surface_receivers.iter().map(|&n| (n, RS)));
        all.extend(self.cutting_planes.iter().map(|&n| (n, CUT)));
        all
    }
}

/// Checks the run folder `run_dir` as the exporter left it for `solver`, against the project it
/// was exported from. It reads `run_dir/config.xml` and the `.cbin` and `.mbin` that config
/// names, and returns the issues in a fixed order: working directory, output paths, attributes,
/// `docalc`, value formats, solver ids, fitting regions.
///
/// Besides the 7 `export` rules, three project codes can appear here, for the same faults found
/// in the files: `band_set_mismatch` (a spectrum whose entry count differs from `freq_enum`'s, or
/// a band count that differs from the project's), and `mesh_out_of_date` (an `.mbin` face marker
/// past the `.cbin`'s faces).
///
/// The project is used for three things only: the band switches `docalc` must match, the band
/// count, and the fitting zones' shapes, which tell room tetrahedra from fitting tetrahedra (a
/// tetrahedron whose centroid lies in an enabled fitting zone belongs to that zone).
///
/// **Deviation from the contract's wording.** `solver_id_mapping_invalid` says every `idVolume`
/// other than 0 must be declared. TetGen's `-A` gives the room's own region a non-zero attribute
/// (every one of tutorial 1's 2,257 tetrahedra, and all 6 of upstream's `cube_mesh.mbin`, carry
/// 1, with no fitting declared), and `config_xml`'s writer relies on that. So the check applies
/// to tetrahedra inside a fitting zone: theirs must be declared, or the fitting is dropped
/// silently. A room tetrahedron may carry any region except a declared fitting id, which is
/// `fitting_id_collides_with_room_region`.
pub fn validate_export(project: &Project, run_dir: &Path, solver: SolverKind) -> Vec<Issue> {
    let mut out = Vec::new();
    let config_path = run_dir.join(CONFIG_FILE_NAME);
    let bytes = match std::fs::read(&config_path) {
        Ok(b) => b,
        Err(e) => {
            out.push(issue(
                CONFIG_ATTRIBUTE_MISSING,
                CONFIG_FILE_NAME,
                format!(
                    "{} cannot be read ({e}): the solvers would read no attribute at all",
                    config_path.display()
                ),
            ));
            return out;
        }
    };
    let bytes = bytes.strip_prefix(b"\xef\xbb\xbf").unwrap_or(&bytes);
    let text = match std::str::from_utf8(bytes) {
        Ok(t) => t,
        Err(e) => {
            out.push(issue(
                CONFIG_ATTRIBUTE_MISSING,
                CONFIG_FILE_NAME,
                format!("config.xml is not UTF-8 ({e}); the writer writes UTF-8"),
            ));
            return out;
        }
    };
    let doc = match Document::parse(text) {
        Ok(d) => d,
        Err(e) => {
            out.push(issue(
                CONFIG_ATTRIBUTE_MISSING,
                CONFIG_FILE_NAME,
                format!(
                    "config.xml is not well-formed XML ({e}): the solvers would read nothing and \
                     every value would stay 0 or empty"
                ),
            ));
            return out;
        }
    };
    let cfg = Cfg::new(&doc);

    working_directory(&cfg, run_dir, &mut out);
    output_paths(&cfg, solver, &mut out);
    attributes(&cfg, solver, project, &mut out);
    docalc(&cfg, solver, project, &mut out);
    value_formats(&cfg, &mut out);
    meshes(&cfg, run_dir, project, &mut out);
    out
}

/// The first component of a relative file name, for the list of what a fresh run folder holds.
fn first_component(name: &str) -> Option<String> {
    Path::new(name).components().find_map(|c| match c {
        Component::Normal(s) => Some(s.to_string_lossy().to_lowercase()),
        _ => None,
    })
}

fn working_directory(cfg: &Cfg, run_dir: &Path, out: &mut Vec<Issue>) {
    let Some(wd) = cfg.root.attribute("workingdirectory") else {
        return; // config_attribute_missing
    };
    let path = at(cfg.root, "workingdirectory");
    let problem = if wd.is_empty() {
        Some(
            "workingdirectory is empty: SPPS runs the whole solve, then aborts (0xC0000409)"
                .to_string(),
        )
    } else if !Path::new(wd).is_absolute() {
        Some(format!("workingdirectory {wd:?} is not an absolute path"))
    } else if !wd.ends_with(['\\', '/']) {
        Some(format!(
            "workingdirectory {wd:?} does not end with a separator: every file name is appended \
             to it with none, so the solver finds no mesh and exits 0"
        ))
    } else {
        match (std::fs::canonicalize(wd), std::fs::canonicalize(run_dir)) {
            (Err(e), _) => Some(format!("workingdirectory {wd:?} does not exist ({e})")),
            (Ok(a), Ok(b)) if a == b => None,
            _ => Some(format!(
                "workingdirectory {wd:?} is not the run folder {}",
                run_dir.display()
            )),
        }
    };
    if let Some(why) = problem {
        out.push(issue(WORKING_DIRECTORY_INVALID, path, why));
        return;
    }

    // Fresh: the folder holds the exporter's inputs and nothing else.
    let mut allowed: HashSet<String> = HashSet::new();
    allowed.insert(CONFIG_FILE_NAME.to_string());
    for name in ["modelName", "tetrameshFileName", "directivities_directory"] {
        if let Some(c) = first_component(cfg.sim_attr(name)) {
            allowed.insert(c);
        }
    }
    let mut extra: Vec<String> = match std::fs::read_dir(run_dir) {
        Ok(entries) => entries
            .filter_map(Result::ok)
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|n| !allowed.contains(&n.to_lowercase()))
            .collect(),
        Err(e) => vec![format!("<unreadable: {e}>")],
    };
    extra.sort();
    if let Some(first) = extra.first() {
        out.push(issue(
            WORKING_DIRECTORY_INVALID,
            path,
            format!(
                "the run folder is not fresh: besides the exporter's inputs it holds '{first}'{}; \
                 a reused folder is overwritten silently and receiver folders gain suffixes",
                match extra.len() {
                    1 => String::new(),
                    n => format!(" and {} more", n - 1),
                }
            ),
        ));
    }
}

fn output_paths(cfg: &Cfg, solver: SolverKind, out: &mut Vec<Issue>) {
    let wd = cfg.root.attribute("workingdirectory").unwrap_or("");
    let sim = |n: &str| cfg.sim_attr(n);
    let freqs: Vec<i64> = cfg
        .bands
        .iter()
        .map(|b| atoi(b.attribute("freq").unwrap_or("")))
        .collect();
    let labels: Vec<&str> = cfg
        .receivers
        .iter()
        .map(|r| r.attribute("lbl").unwrap_or(""))
        .collect();
    let names: Vec<&str> = cfg
        .sources
        .iter()
        .map(|s| s.attribute("name").unwrap_or(""))
        .collect();
    let (rss_dir, rss_file, rss_cut) = (
        sim("recepteurss_directory"),
        sim("recepteurss_filename"),
        sim("recepteurss_cut_filename"),
    );
    let mut paths: Vec<String> = Vec::new();
    match solver {
        SolverKind::Spps => {
            paths.push(format!("{wd}{}", sim("cumul_filename")));
            paths.push(format!("{wd}{}", sim("stats_filename")));
            let saved = atoi(sim("nbparticules_rendu")) > 0;
            for f in &freqs {
                for file in [rss_file, rss_cut] {
                    paths.push(format!("{wd}{rss_dir}{f} Hz/{file}"));
                }
                paths.push(format!("{wd}Intensity animation/{f} Hz/Intensity.rpi"));
                if saved {
                    let dir = format!("{wd}{}{f}\\", sim("particules_directory"));
                    for file in [
                        sim("particules_filename"),
                        "particle_surface_collision_statistics.csv",
                        "particle_receivers_collision_statistics.csv",
                    ] {
                        paths.push(format!("{dir}{file}"));
                    }
                }
            }
            for file in [rss_file, rss_cut] {
                paths.push(format!("{wd}{rss_dir}Global\\{file}"));
            }
            let rp_dir = sim("receiversp_directory");
            let by_source = atoi(sim("output_recp_bysource")) != 0;
            for lbl in &labels {
                for file in [
                    sim("receiversp_filename"),
                    sim("receiversp_filename_adv"),
                    "Sound level per source.recps",
                    "Punctual receiver intensity.gabe",
                ] {
                    paths.push(format!("{wd}{rp_dir}\\{lbl}\\{file}"));
                }
                if by_source {
                    for name in &names {
                        paths.push(format!(
                            "{wd}{rp_dir}\\{lbl}/{name}\\{}",
                            sim("receiversp_filename")
                        ));
                    }
                }
            }
        }
        SolverKind::Tcr => {
            paths.push(format!("{wd}Main results.gabe"));
            for lbl in &labels {
                paths.push(format!("{wd}Punctual receivers/{lbl}.gabe"));
            }
            for prefix in [
                sim("direct_recepteurSOutputName"),
                sim("sabine_recepteurSOutputName"),
                sim("eyring_recepteurSOutputName"),
            ] {
                for f in &freqs {
                    for file in [rss_file, rss_cut] {
                        paths.push(format!("{wd}{prefix}{rss_dir}{f} Hz/{file}"));
                    }
                }
                for file in [rss_file, rss_cut] {
                    paths.push(format!("{wd}{prefix}{rss_dir}Global/{file}"));
                }
            }
        }
    }
    let long: Vec<(usize, &String)> = paths
        .iter()
        .map(|p| (p.encode_utf16().count(), p))
        .filter(|&(n, _)| n >= MAX_PATH_UTF16)
        .collect();
    if let Some(&(n, longest)) = long.iter().max_by_key(|(n, _)| *n) {
        out.push(issue(
            OUTPUT_PATH_TOO_LONG,
            at(cfg.root, "workingdirectory"),
            format!(
                "{} output path(s) reach {MAX_PATH_UTF16} UTF-16 units; the longest is {n}: \
                 {longest:?}. The solvers open plain paths, so Windows' MAX_PATH applies",
                long.len()
            ),
        ));
    }
}

fn required(a: &ConfigAttribute, solver: SolverKind, node: Node) -> bool {
    match a.writer {
        Always | IfElement => true,
        SppsOnly => solver == SolverKind::Spps,
        TcrOnly => solver == SolverKind::Tcr,
        IfDirected => matches!(atoi(node.attribute("directivite").unwrap_or("")), 1 | 5),
        IfBalloon => atoi(node.attribute("directivite").unwrap_or("")) == 5,
        IfTransmits | Never => false,
    }
}

fn attributes(cfg: &Cfg, solver: SolverKind, project: &Project, out: &mut Vec<Issue>) {
    let root_path = format!("{CONFIG_FILE_NAME}:{}", xpath(cfg.root));
    let mut sections = vec![(
        "condition_atmospherique",
        cfg.atmo.is_some(),
        root_path.clone(),
    )];
    sections.push(("simulation", cfg.sim.is_some(), root_path));
    if let Some(sim) = cfg.sim {
        sections.push((
            "freq_enum",
            child(sim, "freq_enum").is_some(),
            format!("{CONFIG_FILE_NAME}:{}", xpath(sim)),
        ));
    }
    for (name, present, path) in sections {
        if !present {
            out.push(issue(
                CONFIG_ATTRIBUTE_MISSING,
                path,
                format!(
                    "the <{name}> element is missing: every attribute in it reads as 0 or empty"
                ),
            ));
        }
    }
    for (node, element) in cfg.elements() {
        for a in CONFIG_ATTRIBUTES.iter().filter(|a| a.element == element) {
            if node.attribute(a.name).is_none() && required(a, solver, node) {
                out.push(issue(
                    CONFIG_ATTRIBUTE_MISSING,
                    at(node, a.name),
                    format!(
                        "{} is missing: the solver prints 'Xml Property {} doesn't exist !' and \
                         reads it as 0 or empty",
                        a.key(),
                        a.name
                    ),
                ));
            }
        }
    }

    // Spectra: one entry per band, mapped by position.
    let n = cfg.bands.len();
    if let Some(freq_enum) = cfg.sim.and_then(|s| child(s, "freq_enum"))
        && n != project.bands.len()
    {
        out.push(issue(
            BAND_SET_MISMATCH,
            format!("{CONFIG_FILE_NAME}:{}", xpath(freq_enum)),
            format!(
                "freq_enum has {n} bands, but the project has {}",
                project.bands.len()
            ),
        ));
    }
    for (list, what, empty_ok) in [
        (&cfg.sources, "source", false),
        (&cfg.materials, "material", false),
        (&cfg.receivers, "point receiver", true),
        (&cfg.fittings, "fitting", false),
    ] {
        for &node in list {
            let found = items(node).len();
            if found != n && !(empty_ok && found == 0) {
                out.push(issue(
                    BAND_SET_MISMATCH,
                    format!("{CONFIG_FILE_NAME}:{}", xpath(node)),
                    format!(
                        "this {what} has {found} band entries, but freq_enum has {n}: spectra are \
                         mapped to bands by position"
                    ),
                ));
            }
        }
    }
}

fn docalc(cfg: &Cfg, solver: SolverKind, project: &Project, out: &mut Vec<Issue>) {
    for &b in &cfg.bands {
        if let Some(v) = b.attribute("docalc")
            && v != "1"
            && v != "0"
        {
            out.push(issue(
                DOCALC_NOT_LITERAL_ONE,
                at(b, "docalc"),
                format!(
                    "docalc is {v:?}: only the exact string \"1\" computes a band, so this band \
                     would be dropped silently"
                ),
            ));
        }
    }
    let computed = match solver {
        SolverKind::Spps => &project.solvers.spps.bands_computed,
        SolverKind::Tcr => &project.solvers.tcr.bands_computed,
    };
    if cfg.bands.len() != computed.len() {
        return; // band_set_mismatch
    }
    // The solver sorts freq_enum by atoi(@freq); a band's index is its sorted position.
    let mut sorted = cfg.bands.clone();
    sorted.sort_by_key(|b| atoi(b.attribute("freq").unwrap_or("")));
    for (b, &on) in sorted.iter().zip(computed) {
        let expected = if on { "1" } else { "0" };
        if let Some(v) = b.attribute("docalc")
            && (v == "1" || v == "0")
            && v != expected
        {
            out.push(issue(
                DOCALC_NOT_LITERAL_ONE,
                at(*b, "docalc"),
                format!(
                    "the band at {} Hz is {} in the project but written docalc=\"{v}\"",
                    b.attribute("freq").unwrap_or("?"),
                    if on { "computed" } else { "not computed" }
                ),
            ));
        }
    }
}

fn value_formats(cfg: &Cfg, out: &mut Vec<Issue>) {
    for (node, element) in cfg.elements() {
        for a in CONFIG_ATTRIBUTES.iter().filter(|a| a.element == element) {
            let Some(v) = node.attribute(a.name) else {
                continue;
            };
            let why = match a.kind {
                Text => None,
                Int => plain_int(v).is_none().then(|| {
                    "is not a plain decimal integer that fits a C int (atoi reads only a leading \
                     integer)"
                        .to_string()
                }),
                Real => plain_real(v).is_none().then(|| {
                    "is not a plain C-locale real ('.' decimal point, no grouping) that is finite \
                     as a float (atof stops at the first character it cannot read)"
                        .to_string()
                }),
                IntRange(lo, hi) => match plain_int(v) {
                    None => Some("is not a plain decimal integer".to_string()),
                    Some(x) if !(lo..=hi).contains(&x) => Some(format!(
                        "is outside {lo} to {hi}: the solver has no branch for it"
                    )),
                    Some(_) => None,
                },
            };
            if let Some(why) = why {
                out.push(issue(
                    CONFIG_VALUE_FORMAT,
                    at(node, a.name),
                    format!("{} = {v:?} {why}", a.key()),
                ));
            }
        }
    }
}

/// Ids declared by a list of elements, as the solver reads them, with the first duplicate of
/// each reported.
fn declared_ids(nodes: &[Node], what: &str, out: &mut Vec<Issue>) -> HashSet<i32> {
    let mut seen: HashMap<i32, Node> = HashMap::new();
    for &n in nodes {
        let Some(id) = n.attribute("id").and_then(plain_int) else {
            continue; // config_attribute_missing or config_value_format
        };
        if let Some(first) = seen.get(&id) {
            out.push(issue(
                SOLVER_ID_MAPPING_INVALID,
                at(n, "id"),
                format!(
                    "{what} id {id} is also declared at {}: the solver takes the first match, so \
                     this one is hidden",
                    xpath(*first)
                ),
            ));
        } else {
            seen.insert(id, n);
        }
    }
    seen.into_keys().collect()
}

/// Groups `(index, value)` by value, keeping the first index and the count, in first-seen order.
fn tally(values: impl Iterator<Item = (usize, i64)>) -> Vec<(i64, usize, usize)> {
    let mut groups: Vec<(i64, usize, usize)> = Vec::new();
    for (i, v) in values {
        match groups.iter_mut().find(|(g, _, _)| *g == v) {
            Some((_, _, n)) => *n += 1,
            None => groups.push((v, i, 1)),
        }
    }
    groups
}

fn meshes(cfg: &Cfg, run_dir: &Path, project: &Project, out: &mut Vec<Issue>) {
    let materials = declared_ids(&cfg.materials, "material", out);
    let receivers = declared_ids(&cfg.surface_receivers, "surface receiver", out);
    let fittings = declared_ids(&cfg.fittings, "fitting", out);

    let wd_path = at(cfg.root, "workingdirectory");
    let model_name = cfg.sim_attr("modelName");
    let mesh_name = cfg.sim_attr("tetrameshFileName");
    let model = if model_name.is_empty() {
        None
    } else {
        match cbin::read_file(&run_dir.join(model_name)) {
            Ok(m) => Some(m),
            Err(e) => {
                out.push(issue(
                    WORKING_DIRECTORY_INVALID,
                    wd_path.clone(),
                    format!("the run folder has no readable scene mesh {model_name:?}: {e}"),
                ));
                None
            }
        }
    };
    let mesh = if mesh_name.is_empty() {
        None
    } else {
        match mbin::read_file(&run_dir.join(mesh_name)) {
            Ok(m) => Some(m),
            Err(e) => {
                out.push(issue(
                    WORKING_DIRECTORY_INVALID,
                    wd_path,
                    format!("the run folder has no readable tetrahedral mesh {mesh_name:?}: {e}"),
                ));
                None
            }
        }
    };

    if let Some(model) = &model {
        let faces = || model.faces.iter().enumerate();
        for (id, first, n) in tally(faces().map(|(i, f)| (i, i64::from(f.id_mat)))) {
            // The solver stores type_surface@id unsigned and compares it with idMat.
            if !materials.iter().any(|&m| i64::from(m as u32) == id) {
                out.push(issue(
                    SOLVER_ID_MAPPING_INVALID,
                    format!("{model_name}:/faces/{first}"),
                    format!(
                        "{n} face(s), the first face {first}, use material id {id}, which no \
                         type_surface declares: the solvers exit -1, or crash"
                    ),
                ));
            }
        }
        for (id, first, n) in tally(
            faces()
                .filter(|(_, f)| f.id_rs != -1)
                .map(|(i, f)| (i, i64::from(f.id_rs))),
        ) {
            if !receivers.contains(&(id as i32)) {
                out.push(issue(
                    SOLVER_ID_MAPPING_INVALID,
                    format!("{model_name}:/faces/{first}"),
                    format!(
                        "{n} face(s), the first face {first}, belong to surface receiver {id}, \
                         which no recepteur_surfacique declares: the solver indexes its receiver \
                         list at -1"
                    ),
                ));
            }
        }
        for (id, first, n) in tally(
            faces()
                .filter(|(_, f)| f.id_en != -1)
                .map(|(i, f)| (i, i64::from(f.id_en))),
        ) {
            if !fittings.contains(&(id as i32)) {
                out.push(issue(
                    SOLVER_ID_MAPPING_INVALID,
                    format!("{model_name}:/faces/{first}"),
                    format!(
                        "{n} face(s), the first face {first}, belong to fitting {id}, which no \
                         encombrement declares: the fitting is dropped silently"
                    ),
                ));
            }
        }
    }
    let Some(mesh) = &mesh else {
        return;
    };
    let in_zone = tetrahedra_in_fitting_zones(mesh, project);
    let fitting_tets = mesh
        .tetrahedra
        .iter()
        .zip(&in_zone)
        .enumerate()
        .filter(|(_, (t, zone))| **zone == Some(true) && t.id_volume != 0)
        .map(|(i, (t, _))| (i, i64::from(t.id_volume)));
    for (id, first, n) in tally(fitting_tets) {
        if !fittings.contains(&(id as i32)) {
            out.push(issue(
                SOLVER_ID_MAPPING_INVALID,
                format!("{mesh_name}:/tetrahedra/{first}"),
                format!(
                    "{n} tetrahedra inside a fitting zone, the first {first}, carry region {id}, \
                     which no encombrement declares: the fitting is dropped silently"
                ),
            ));
        }
    }
    if let Some(model) = &model {
        let past: Vec<usize> = mesh
            .tetrahedra
            .iter()
            .enumerate()
            .filter(|(_, t)| {
                t.faces
                    .iter()
                    .any(|f| f.marker >= 0 && f.marker as usize >= model.faces.len())
            })
            .map(|(i, _)| i)
            .collect();
        if let Some(&first) = past.first() {
            out.push(issue(
                MESH_OUT_OF_DATE,
                format!("{mesh_name}:/tetrahedra/{first}"),
                format!(
                    "{} tetrahedra, the first {first}, have a face marker past the {} faces of \
                     {model_name}: the mesh was not built from this scene",
                    past.len(),
                    model.faces.len()
                ),
            ));
        }
    }
    // fitting_id_collides_with_room_region: a room tetrahedron's region is not a fitting id.
    let room_tets = mesh
        .tetrahedra
        .iter()
        .zip(&in_zone)
        .enumerate()
        .filter(|(_, (t, zone))| **zone == Some(false) && t.id_volume != 0)
        .map(|(i, (t, _))| (i, i64::from(t.id_volume)));
    for (id, first, n) in tally(room_tets) {
        if fittings.contains(&(id as i32)) {
            let declared = cfg
                .fittings
                .iter()
                .find(|f| f.attribute("id").and_then(plain_int) == Some(id as i32))
                .map(|f| xpath(*f))
                .unwrap_or_default();
            out.push(issue(
                FITTING_ID_COLLIDES_WITH_ROOM_REGION,
                format!("{mesh_name}:/tetrahedra/{first}"),
                format!(
                    "{n} tetrahedra of the room itself (outside every fitting zone), the first \
                     {first}, carry region {id}, which is also the id of the fitting at \
                     {declared}: the solver would fill them with that fitting"
                ),
            ));
        }
    }
}

/// For each tetrahedron: `Some(true)` if its centroid lies in an enabled fitting zone of the
/// project, `Some(false)` if it belongs to the room, `None` if a corner index is out of range.
fn tetrahedra_in_fitting_zones(mesh: &mbin::Mesh, project: &Project) -> Vec<Option<bool>> {
    let zones: Vec<Box<dyn Fn([f64; 3]) -> bool>> = project
        .fitting_zones
        .iter()
        .filter(|z| z.enabled)
        .map(|z| -> Box<dyn Fn([f64; 3]) -> bool> {
            match &z.shape {
                FittingShape::Box { min, max, .. } => {
                    let (a, b) = (min.to_array(), max.to_array());
                    Box::new(move |c: [f64; 3]| {
                        (0..3).all(|k| a[k].min(b[k]) <= c[k] && c[k] <= a[k].max(b[k]))
                    })
                }
                FittingShape::Surfaces { groups, .. } => {
                    let v = &project.geometry.vertices;
                    let triangles: Vec<[[f64; 3]; 3]> = project
                        .geometry
                        .faces
                        .iter()
                        .filter(|f| groups.contains(&f.group))
                        .filter_map(|f| {
                            let [a, b, c] =
                                f.vertices.map(|i| v.get(i as usize).map(|p| p.to_array()));
                            Some([a?, b?, c?])
                        })
                        .collect();
                    Box::new(move |c: [f64; 3]| {
                        matches!(
                            geometry::locate(&triangles, c),
                            PointLocation::Inside { .. } | PointLocation::OnSurface { .. }
                        )
                    })
                }
            }
        })
        .collect();
    mesh.tetrahedra
        .iter()
        .map(|t| {
            let corners: Option<Vec<[f32; 3]>> = t
                .vertices
                .iter()
                .map(|&v| {
                    usize::try_from(v)
                        .ok()
                        .and_then(|v| mesh.nodes.get(v))
                        .copied()
                })
                .collect();
            let corners = corners?;
            let centroid: [f64; 3] =
                [0, 1, 2].map(|k| corners.iter().map(|c| f64::from(c[k])).sum::<f64>() / 4.0);
            Some(zones.iter().any(|inside| inside(centroid)))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn number_literals() {
        for ok in ["0", "-3", "+12", "2147483647", "-2147483648"] {
            assert!(plain_int(ok).is_some(), "{ok}");
        }
        for bad in [
            "",
            "1e3",
            "1.0",
            " 1",
            "1 ",
            "2147483648",
            "0x10",
            "true",
            "+",
        ] {
            assert!(plain_int(bad).is_none(), "{bad}");
        }
        for ok in ["0", "0.31", "-5", ".5", "5.", "1e-05", "3.4E+38", "+2.0e3"] {
            assert!(plain_real(ok).is_some(), "{ok}");
        }
        for bad in [
            "", "0,5", "1,000.5", "1e39", "inf", "nan", ".", "e5", "1e", " 1", "1.2.3", "--1",
        ] {
            assert!(plain_real(bad).is_none(), "{bad}");
        }
        assert_eq!(atoi("  -12abc"), -12);
        assert_eq!(atoi("true"), 0);
        assert_eq!(atoi("1.9"), 1);
    }

    #[test]
    fn xpath_indexes_repeated_names_only() {
        let doc = Document::parse(
            "<configuration><simulation><freq_enum><bfreq/><bfreq/></freq_enum></simulation>\
             <sources><source/></sources></configuration>",
        )
        .unwrap();
        let root = doc.root_element();
        let sim = child(root, "simulation").unwrap();
        let bands = items(child(sim, "freq_enum").unwrap());
        assert_eq!(
            xpath(bands[0]),
            "/configuration/simulation/freq_enum/bfreq[1]"
        );
        assert_eq!(
            xpath(bands[1]),
            "/configuration/simulation/freq_enum/bfreq[2]"
        );
        let sources = items(child(root, "sources").unwrap());
        assert_eq!(xpath(sources[0]), "/configuration/sources/source");
        assert_eq!(
            at(root, "workingdirectory"),
            "config.xml:/configuration@workingdirectory"
        );
    }
}
