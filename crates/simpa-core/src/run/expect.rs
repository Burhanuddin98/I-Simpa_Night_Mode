//! What a run folder's `config.xml` asks of the solver: the settings the verdict needs, as the
//! solver reads them, and the output files it must write (`docs/solver-contract.md` Part B,
//! "Expected outputs").
//!
//! The expectation comes from the `config.xml` actually in the folder, for `run` and `run-folder`
//! alike (`docs/m5-m6-design.md`: "That file is the one source of truth"), plus the `.cbin` its
//! `modelName` names, which says whether the first surface receiver owns any face.
//!
//! Values are read as the solvers read them: integers with `atoi`, a missing attribute as empty
//! (`cxml.cpp:115`), list items as every child element and any non-blank text (pugixml keeps both
//! as children, `cxml.cpp:61-64`).
//!
//! **Requested bands.** The solvers compute a band only when `docalc` is exactly `"1"`
//! (`base_core_configuration.cpp:106`). The expectation asks for every band whose `docalc` is not
//! `"0"`, so a band the solver drops over a malformed switch shows up as missing outputs and as
//! `stats_band_mismatch`, as P2 `docalc_true` did.
//!
//! **Surface receivers need faces.** Both solvers write a surface-receiver file, per band or
//! `Global`, only when the first `recepteur_surfacique` owns at least one face
//! (`baseReportManager.cpp:331-334, 361-364, 430-433`). This holds for the per-band files as well,
//! which the contract's table does not say. Faces are counted on the `.cbin` (`idRs`); the solver
//! counts tetrahedron faces linked to those scene faces, the same set on a mesh that covers every
//! scene face (inferred).

use std::fmt;
use std::path::Path;

use roxmltree::{Document, Node};
use serde::{Deserialize, Serialize};

use crate::config_xml::names;
use crate::formats::cbin;
use crate::schema::SolverKind;

/// Fixed names the solvers write whatever the config says.
pub mod fixed {
    /// SPPS: per point receiver (`sppsNantes.cpp:402`).
    pub const POINT_RECEIVER_INTENSITY: &str = "Punctual receiver intensity.gabe";
    /// SPPS: per point receiver (`sppsNantes.cpp:406`).
    pub const POINT_RECEIVER_BY_SOURCE: &str = "Sound level per source.recps";
    /// SPPS: the folder of the per-band intensity files (`spps/reportmanager.cpp:719`).
    pub const INTENSITY_DIR: &str = "Intensity animation";
    /// SPPS: per computed band (`spps/reportmanager.cpp:751`).
    pub const INTENSITY_FILE: &str = "Intensity.rpi";
    /// SPPS: per band with particles saved (`spps/reportmanager.cpp:113-114`).
    pub const SURFACE_COLLISIONS: &str = "particle_surface_collision_statistics.csv";
    pub const RECEIVER_COLLISIONS: &str = "particle_receivers_collision_statistics.csv";
    /// TCR: `Main results` plus `.gabe` (`main_tc.cpp:136-137`; `ctr/reportmanager.cpp:214`).
    pub const TCR_MAIN_RESULTS: &str = "Main results.gabe";
    /// TCR: the point-receiver folder; `receiversp_directory` is ignored (`main_tc.cpp:141`).
    pub const TCR_POINT_RECEIVER_DIR: &str = "Punctual receivers";
    /// The surface-receiver folder of all bands together (`sppsNantes.cpp:409`;
    /// `TC_CalculationCore.cpp:410-418`).
    pub const GLOBAL: &str = "Global";
}

/// Why no expectation could be derived.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExpectError {
    /// `config.xml` cannot be read.
    Unreadable(String),
    /// It is not UTF-8, or not well-formed XML.
    NotXml(String),
    /// Its root element is not `<configuration>`.
    NotAConfiguration(String),
}

impl fmt::Display for ExpectError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ExpectError::Unreadable(m) => write!(f, "config.xml cannot be read: {m}"),
            ExpectError::NotXml(m) => write!(f, "config.xml is not a readable XML file: {m}"),
            ExpectError::NotAConfiguration(root) => {
                write!(
                    f,
                    "config.xml's root element is <{root}>, not <configuration>"
                )
            }
        }
    }
}

impl std::error::Error for ExpectError {}

/// One `freq_enum/bfreq`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Band {
    /// `@freq`, read with `atoi`.
    pub freq_hz: i32,
    /// `@docalc` as written.
    pub docalc: String,
    /// The solver computes it: `docalc` is exactly `"1"`.
    pub computed: bool,
    /// The config asks for it: `docalc` is not `"0"`.
    pub requested: bool,
}

/// The output names from `<simulation>`, as written (empty when absent).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OutputNames {
    pub model_name: String,
    pub tetramesh_file_name: String,
    pub recepteurss_directory: String,
    pub recepteurss_filename: String,
    pub recepteurss_cut_filename: String,
    pub receiversp_directory: String,
    pub receiversp_filename: String,
    pub receiversp_filename_adv: String,
    pub cumul_filename: String,
    /// SPPS only.
    pub stats_filename: String,
    pub particules_directory: String,
    pub particules_filename: String,
    /// TCR only: the three result-folder prefixes.
    pub direct_prefix: String,
    pub sabine_prefix: String,
    pub eyring_prefix: String,
}

/// SPPS's settings, as `spps/data_manager/core_configuration.cpp:18-68` reads them.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SppsSettings {
    /// `nbparticules`, at least 1 (`core_configuration.cpp:21-23`): particles per source per band.
    pub nbparticules: u32,
    /// `nbparticules_rendu`, at least 0 (`core_configuration.cpp:24-26`).
    pub nbparticules_rendu: u32,
    /// `computation_method`: 0 random, 1 energetic.
    pub computation_method: i32,
    /// `random_seed`, 0 when absent; non-zero makes SPPS single-threaded.
    pub random_seed: i32,
    /// `output_recp_bysource` ≠ 0; off when absent.
    pub output_recp_bysource: bool,
    /// `save_surface_intersection` ≠ 0; on when absent.
    pub save_surface_intersection: bool,
    /// `save_receivers_intersection` ≠ 0; on when absent.
    pub save_receivers_intersection: bool,
}

/// What a run folder's `config.xml` asks for.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Expectation {
    pub solver: SolverKind,
    /// `configuration@workingdirectory` as written.
    pub working_directory: String,
    /// Every `freq_enum` item, in ascending `@freq`, as the solver orders them.
    pub bands: Vec<Band>,
    /// `source@name` of each source, in order.
    pub sources: Vec<String>,
    /// `recepteur_ponctuel@lbl` of each point receiver, in order.
    pub point_receivers: Vec<String>,
    /// `recepteur_surfacique@id` of each surface receiver, in order.
    pub surface_receivers: Vec<i32>,
    /// Whether the first surface receiver owns a `.cbin` face. `Some(false)` with no surface
    /// receiver; `None` when the `.cbin` was not read, in which case its files are expected
    /// (a solver that cannot read the `.cbin` fails anyway).
    pub first_surface_receiver_has_faces: Option<bool>,
    /// `recepteur_surfacique_coupe` elements.
    pub cutting_planes: usize,
    /// `simulation@output_recs_byfreq` ≠ 0. TCR ignores it: it tests the setting's pointer, not
    /// its value (`TC_CalculationCore.cpp:354, 382, 391`).
    pub output_recs_byfreq: bool,
    pub names: OutputNames,
    /// `None` for TCR.
    pub spps: Option<SppsSettings>,
}

/// `atoi`: leading C spaces, an optional sign, then digits; 0 when there are none. Saturates at
/// the `int` range, where C's behaviour is undefined.
pub(crate) fn atoi(s: &str) -> i32 {
    let s = s.trim_start_matches([' ', '\t', '\n', '\r', '\x0b', '\x0c']);
    let (neg, digits) = match s.as_bytes().first() {
        Some(b'-') => (true, &s[1..]),
        Some(b'+') => (false, &s[1..]),
        _ => (false, s),
    };
    let mut v: i64 = 0;
    for b in digits.bytes().take_while(u8::is_ascii_digit) {
        v = (v * 10 + i64::from(b - b'0')).min(i64::from(i32::MAX) + 1);
    }
    let v = if neg { -v } else { v };
    v.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32
}

/// The children the solver iterates: every element, and any non-blank text.
pub(crate) fn items<'a, 'i>(node: Node<'a, 'i>) -> impl Iterator<Item = Node<'a, 'i>> {
    node.children()
        .filter(|c| c.is_element() || (c.is_text() && !c.text().unwrap_or("").trim().is_empty()))
}

/// The first child element named `name` (`CXmlNode::GetChild`).
pub(crate) fn child<'a, 'i>(node: Node<'a, 'i>, name: &str) -> Option<Node<'a, 'i>> {
    node.children()
        .find(|c| c.is_element() && c.tag_name().name() == name)
}

/// Joins name parts exactly as the solver concatenates them, then writes every separator as
/// `/` and drops empty components: the key under which a file is expected.
pub fn normalize(path: &str) -> String {
    path.split(['\\', '/'])
        .filter(|c| !c.is_empty())
        .collect::<Vec<_>>()
        .join("/")
}

impl Expectation {
    /// Reads `dir/config.xml` and, when there are surface receivers, the `.cbin` it names in
    /// `dir`. `dir` is the solver's working directory.
    pub fn read(dir: &Path, solver: SolverKind) -> Result<Expectation, ExpectError> {
        let path = dir.join(names::CONFIG);
        let bytes = std::fs::read(&path)
            .map_err(|e| ExpectError::Unreadable(format!("{}: {e}", path.display())))?;
        let bytes = bytes.strip_prefix(b"\xef\xbb\xbf").unwrap_or(&bytes);
        let text = std::str::from_utf8(bytes).map_err(|e| ExpectError::NotXml(e.to_string()))?;
        let mut exp = Expectation::from_config(text, solver)?;
        if !exp.surface_receivers.is_empty() && !exp.names.model_name.is_empty() {
            exp.first_surface_receiver_has_faces =
                cbin::read_file(&dir.join(&exp.names.model_name))
                    .ok()
                    .map(|m| exp.first_receiver_owns_a_face(&m));
        }
        Ok(exp)
    }

    /// Parses a `config.xml`. Surface-receiver faces are unknown until [`Expectation::with_scene`].
    pub fn from_config(xml: &str, solver: SolverKind) -> Result<Expectation, ExpectError> {
        let doc = Document::parse(xml).map_err(|e| ExpectError::NotXml(e.to_string()))?;
        let root = doc.root_element();
        if root.tag_name().name() != "configuration" {
            return Err(ExpectError::NotAConfiguration(
                root.tag_name().name().to_string(),
            ));
        }
        let sim = child(root, "simulation");
        let s = |name: &str| -> String {
            sim.and_then(|n| n.attribute(name))
                .unwrap_or("")
                .to_string()
        };
        let present = |name: &str| sim.and_then(|n| n.attribute(name)).is_some();

        let mut bands: Vec<Band> = sim
            .and_then(|n| child(n, "freq_enum"))
            .map(|f| {
                items(f)
                    .map(|b| {
                        let docalc = b.attribute("docalc").unwrap_or("").to_string();
                        Band {
                            freq_hz: atoi(b.attribute("freq").unwrap_or("")),
                            computed: docalc == "1",
                            requested: docalc != "0",
                            docalc,
                        }
                    })
                    .collect()
            })
            .unwrap_or_default();
        bands.sort_by_key(|b| b.freq_hz);

        let list = |name: &str, attr: &str| -> Vec<String> {
            child(root, name)
                .map(|n| {
                    items(n)
                        .map(|i| i.attribute(attr).unwrap_or("").to_string())
                        .collect()
                })
                .unwrap_or_default()
        };
        let sources = list("sources", "name");
        let point_receivers = list("recepteursp", "lbl");
        let (surface_receivers, cutting_planes) = match child(root, "recepteurss") {
            Some(rs) => {
                let named = |n: &'static str| {
                    rs.children()
                        .filter(move |c| c.is_element() && c.tag_name().name() == n)
                };
                (
                    named("recepteur_surfacique")
                        .map(|r| atoi(r.attribute("id").unwrap_or("")))
                        .collect::<Vec<_>>(),
                    named("recepteur_surfacique_coupe").count(),
                )
            }
            None => (Vec::new(), 0),
        };

        let spps = (solver == SolverKind::Spps).then(|| SppsSettings {
            nbparticules: atoi(&s("nbparticules")).max(1) as u32,
            nbparticules_rendu: atoi(&s("nbparticules_rendu")).max(0) as u32,
            computation_method: atoi(&s("computation_method")),
            random_seed: atoi(&s("random_seed")),
            output_recp_bysource: atoi(&s("output_recp_bysource")) != 0,
            save_surface_intersection: !present("save_surface_intersection")
                || atoi(&s("save_surface_intersection")) != 0,
            save_receivers_intersection: !present("save_receivers_intersection")
                || atoi(&s("save_receivers_intersection")) != 0,
        });

        Ok(Expectation {
            solver,
            working_directory: root.attribute("workingdirectory").unwrap_or("").to_string(),
            bands,
            sources,
            point_receivers,
            first_surface_receiver_has_faces: surface_receivers.is_empty().then_some(false),
            surface_receivers,
            cutting_planes,
            output_recs_byfreq: atoi(&s("output_recs_byfreq")) != 0,
            names: OutputNames {
                model_name: s("modelName"),
                tetramesh_file_name: s("tetrameshFileName"),
                recepteurss_directory: s("recepteurss_directory"),
                recepteurss_filename: s("recepteurss_filename"),
                recepteurss_cut_filename: s("recepteurss_cut_filename"),
                receiversp_directory: s("receiversp_directory"),
                receiversp_filename: s("receiversp_filename"),
                receiversp_filename_adv: s("receiversp_filename_adv"),
                cumul_filename: s("cumul_filename"),
                stats_filename: s("stats_filename"),
                particules_directory: s("particules_directory"),
                particules_filename: s("particules_filename"),
                direct_prefix: s("direct_recepteurSOutputName"),
                sabine_prefix: s("sabine_recepteurSOutputName"),
                eyring_prefix: s("eyring_recepteurSOutputName"),
            },
            spps,
        })
    }

    fn first_receiver_owns_a_face(&self, scene: &cbin::Model) -> bool {
        self.surface_receivers
            .first()
            .is_some_and(|&id| scene.faces.iter().any(|f| f.id_rs == id))
    }

    /// Settles [`Expectation::first_surface_receiver_has_faces`] from the scene mesh.
    pub fn with_scene(mut self, scene: &cbin::Model) -> Self {
        self.first_surface_receiver_has_faces = Some(self.first_receiver_owns_a_face(scene));
        self
    }

    /// The bands the config asks for, ascending, each once.
    pub fn requested_bands(&self) -> Vec<i32> {
        let mut v: Vec<i32> = self
            .bands
            .iter()
            .filter(|b| b.requested)
            .map(|b| b.freq_hz)
            .collect();
        v.dedup();
        v
    }

    /// Whether the surface-receiver files (per band and `Global`) are expected.
    fn surface_files(&self) -> bool {
        !self.surface_receivers.is_empty() && self.first_surface_receiver_has_faces != Some(false)
    }

    /// The tables whose displayed values must be finite (TCR's `nonfinite_result`): the main
    /// results and each point receiver's table.
    pub fn result_tables(&self) -> Vec<String> {
        if self.solver != SolverKind::Tcr {
            return Vec::new();
        }
        let mut v = vec![normalize(fixed::TCR_MAIN_RESULTS)];
        v.extend(
            self.point_receivers
                .iter()
                .map(|lbl| normalize(&format!("{}/{lbl}.gabe", fixed::TCR_POINT_RECEIVER_DIR))),
        );
        dedup(v)
    }

    /// Every file the solver must write, relative to its working directory, normalised with
    /// [`normalize`], each once, in the order of the contract's tables.
    pub fn expected_files(&self) -> Vec<String> {
        let n = &self.names;
        let bands = self.requested_bands();
        let rss = &n.recepteurss_directory;
        let mut v: Vec<String> = Vec::new();
        match &self.spps {
            Some(spps) => {
                v.push(n.stats_filename.clone());
                v.push(n.cumul_filename.clone());
                for lbl in &self.point_receivers {
                    let dir = format!("{}\\{lbl}\\", n.receiversp_directory);
                    for file in [
                        n.receiversp_filename.as_str(),
                        n.receiversp_filename_adv.as_str(),
                        fixed::POINT_RECEIVER_INTENSITY,
                        fixed::POINT_RECEIVER_BY_SOURCE,
                    ] {
                        v.push(format!("{dir}{file}"));
                    }
                    if spps.output_recp_bysource {
                        // `spps/reportmanager.cpp:651-653`.
                        for src in &self.sources {
                            v.push(format!(
                                "{}\\{lbl}/{src}\\{}",
                                n.receiversp_directory, n.receiversp_filename
                            ));
                        }
                    }
                }
                for f in &bands {
                    v.push(format!(
                        "{}/{f} Hz/{}",
                        fixed::INTENSITY_DIR,
                        fixed::INTENSITY_FILE
                    ));
                }
                if self.output_recs_byfreq {
                    for f in &bands {
                        if self.surface_files() {
                            v.push(format!("{rss}{f} Hz/{}", n.recepteurss_filename));
                        }
                        if self.cutting_planes > 0 {
                            v.push(format!("{rss}{f} Hz/{}", n.recepteurss_cut_filename));
                        }
                    }
                }
                if self.surface_files() {
                    v.push(format!(
                        "{rss}{}\\{}",
                        fixed::GLOBAL,
                        n.recepteurss_filename
                    ));
                }
                if self.cutting_planes > 0 {
                    v.push(format!(
                        "{rss}{}\\{}",
                        fixed::GLOBAL,
                        n.recepteurss_cut_filename
                    ));
                }
                if spps.nbparticules_rendu > 0 {
                    for f in &bands {
                        let dir = format!("{}{f}\\", n.particules_directory);
                        v.push(format!("{dir}{}", n.particules_filename));
                        if spps.save_surface_intersection {
                            v.push(format!("{dir}{}", fixed::SURFACE_COLLISIONS));
                        }
                        if spps.save_receivers_intersection {
                            v.push(format!("{dir}{}", fixed::RECEIVER_COLLISIONS));
                        }
                    }
                }
            }
            None => {
                v.push(fixed::TCR_MAIN_RESULTS.to_string());
                for lbl in &self.point_receivers {
                    v.push(format!("{}/{lbl}.gabe", fixed::TCR_POINT_RECEIVER_DIR));
                }
                v.extend(self.tcr_surface_files());
            }
        }
        dedup(v.iter().map(|p| normalize(p)).collect())
    }

    /// TCR's surface-receiver and cutting-plane files, as the solver joins their names: per
    /// computed band and `Global`, under each of the three result prefixes.
    fn tcr_surface_files(&self) -> Vec<String> {
        let n = &self.names;
        let rss = &n.recepteurss_directory;
        let mut v = Vec::new();
        for prefix in [&n.direct_prefix, &n.sabine_prefix, &n.eyring_prefix] {
            let root = format!("{prefix}{rss}");
            for f in self
                .requested_bands()
                .iter()
                .map(|f| format!("{f} Hz"))
                .chain([fixed::GLOBAL.to_string()])
            {
                if self.surface_files() {
                    v.push(format!("{root}{f}/{}", n.recepteurss_filename));
                }
                if self.cutting_planes > 0 {
                    v.push(format!("{root}{f}/{}", n.recepteurss_cut_filename));
                }
            }
        }
        v
    }

    /// The `.csbin` files whose values must be finite (TCR's `nonfinite_result`): every
    /// surface-receiver and cutting-plane file TCR must write, [`normalize`]d. Empty for SPPS.
    pub fn surface_tables(&self) -> Vec<String> {
        if self.solver != SolverKind::Tcr {
            return Vec::new();
        }
        dedup(
            self.tcr_surface_files()
                .iter()
                .map(|p| normalize(p))
                .collect(),
        )
    }
}

/// Keeps the first occurrence of each path.
fn dedup(v: Vec<String>) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    v.into_iter().filter(|p| seen.insert(p.clone())).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn atoi_reads_like_c() {
        assert_eq!(atoi("  -12abc"), -12);
        assert_eq!(atoi("true"), 0);
        assert_eq!(atoi(""), 0);
        assert_eq!(atoi("1.9"), 1);
        assert_eq!(atoi("99999999999"), i32::MAX);
        assert_eq!(atoi("-99999999999"), i32::MIN);
    }

    #[test]
    fn normalize_joins_either_separator() {
        assert_eq!(
            normalize(r"Surface receiver\125 Hz/Sound level.csbin"),
            "Surface receiver/125 Hz/Sound level.csbin"
        );
        assert_eq!(normalize(r"a\\b//c\"), "a/b/c");
    }

    const MINIMAL: &str = r#"<configuration workingdirectory="W\">
      <simulation recepteurss_directory="RS\" recepteurss_filename="s.csbin"
        recepteurss_cut_filename="c.csbin" receiversp_directory="RP" receiversp_filename="r.recp"
        receiversp_filename_adv="a.gap" cumul_filename="t.recp" stats_filename="st.gabe"
        particules_directory="P\" particules_filename="p.pbin" nbparticules="0"
        nbparticules_rendu="-4" output_recs_byfreq="1">
        <freq_enum><bfreq freq="500" docalc="true"/><bfreq freq="125" docalc="1"/><bfreq freq="250" docalc="0"/></freq_enum>
      </simulation>
      <sources><source name="S"/>stray text</sources>
      <recepteursp><recepteur_ponctuel lbl="R"/></recepteursp>
      <recepteurss><recepteur_surfacique id="7"/><recepteur_surfacique_coupe id="8"/></recepteurss>
    </configuration>"#;

    #[test]
    fn values_are_read_as_the_solver_reads_them() {
        let e = Expectation::from_config(MINIMAL, SolverKind::Spps).unwrap();
        let freqs: Vec<(i32, bool, bool)> = e
            .bands
            .iter()
            .map(|b| (b.freq_hz, b.computed, b.requested))
            .collect();
        assert_eq!(
            freqs,
            [(125, true, true), (250, false, false), (500, false, true)]
        );
        assert_eq!(e.requested_bands(), [125, 500]);
        // Non-blank text is a list item: two sources.
        assert_eq!(e.sources, ["S", ""]);
        let spps = e.spps.as_ref().unwrap();
        assert_eq!((spps.nbparticules, spps.nbparticules_rendu), (1, 0));
        assert!(spps.save_surface_intersection && spps.save_receivers_intersection);
        assert!(!spps.output_recp_bysource);
        assert_eq!(e.surface_receivers, [7]);
        assert_eq!(e.cutting_planes, 1);
        assert_eq!(e.first_surface_receiver_has_faces, None);
    }

    #[test]
    fn a_first_receiver_without_faces_writes_no_receiver_file() {
        let e = Expectation::from_config(MINIMAL, SolverKind::Spps).unwrap();
        let face = |id_rs| cbin::Face {
            id_rs,
            ..cbin::Face::default()
        };
        let with = e.clone().with_scene(&cbin::Model {
            faces: vec![face(-1), face(7)],
            vertices: vec![],
        });
        let without = e.with_scene(&cbin::Model {
            faces: vec![face(-1), face(8)],
            vertices: vec![],
        });
        let count = |e: &Expectation| {
            e.expected_files()
                .iter()
                .filter(|p| p.ends_with("s.csbin"))
                .count()
        };
        // 125 and 500 Hz, then Global.
        assert_eq!(count(&with), 3);
        assert_eq!(count(&without), 0);
        // The cutting plane's files do not depend on faces.
        assert_eq!(
            without
                .expected_files()
                .iter()
                .filter(|p| p.ends_with("c.csbin"))
                .count(),
            3
        );
    }

    #[test]
    fn not_a_configuration() {
        assert!(matches!(
            Expectation::from_config("<simulation/>", SolverKind::Tcr),
            Err(ExpectError::NotAConfiguration(_))
        ));
        assert!(matches!(
            Expectation::from_config("<configuration>", SolverKind::Tcr),
            Err(ExpectError::NotXml(_))
        ));
    }
}
