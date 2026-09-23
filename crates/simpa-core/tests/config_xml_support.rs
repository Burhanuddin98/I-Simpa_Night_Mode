//! Shared helpers for the `config_xml_*` tests, included by each with `#[path]`. Built on its own
//! it is an empty test target.
//!
//! - [`doc_attrs`] parses the reference tables of `docs/formats/config_xml.md`: every attribute a
//!   solver reads, its type and its Writer obligation.
//! - [`solver_view`] reads a `config.xml` into what the solvers would hold: each element instance
//!   keyed the way the solvers find it, each documented attribute parsed as the solver parses it.
//! - [`run_solver`] runs one of our M1 solver builds in a run folder.
#![allow(dead_code)]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use simpa_core::schema::{self, Project};

/// The repository root: an absolute path with no `..` in it.
pub fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crates/simpa-core sits two levels below the root")
        .to_path_buf()
}

pub fn repo_file(rel: &str) -> PathBuf {
    repo_root().join(rel)
}

pub fn read_text(rel: &str) -> String {
    let p = repo_file(rel);
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))
}

pub fn load_project(rel: &str) -> Project {
    schema::load(&repo_file(rel)).unwrap_or_else(|e| panic!("{rel}: {e}"))
}

/// A new, empty folder under `target/test-runs/config_xml/`. Never reused and never deleted.
pub fn fresh_run_dir(label: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = repo_root()
        .join("target/test-runs/config_xml")
        .join(format!("{label}-{nanos}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

// ---------------------------------------------------------------------------------------------
// Projects

/// `tests/fixtures/projects/cube.simpa`, extended so that every conditional attribute of
/// `config.xml` is written: three surface groups (one pinned id 0, two assigned), a material with
/// a transmission loss and an unused semi-diffuse one, omni, unidirectional, directivity-balloon
/// and a disabled plane source, a receiver with background noise and one without, a scene
/// receiver, a cutting plane and a disabled scene receiver, a box fitting zone, user-defined air
/// absorption in dB/m, and a variant. The balloon file is upstream's sample
/// `spps/tests/speaker-test3.txt` (40 Hz to 8 kHz). Every point is at least 0.43 m from every
/// plane through three cube corners, so off every face of `cube_mesh.mbin`'s tetrahedra.
pub fn rich_cube() -> Project {
    use schema::*;
    let mut p = load_project("tests/fixtures/projects/cube.simpa");
    let n = p.bands.len();
    let id = |k: u128| 0x0c0b_e000_0000_4000_8000_0000_0000_1000u128 + k;
    let walls = p.surface_groups[0].id;
    p.surface_groups[0].name = "Walls".into();
    let floor = GroupId::from_u128(id(1));
    let ceiling = GroupId::from_u128(id(2));
    let absorber = MaterialId::from_u128(id(11));
    let partition = MaterialId::from_u128(id(12));
    let unused = MaterialId::from_u128(id(13));
    let mat = |mid, name: &str, a: f64, s: f64, law, tl: Option<f64>, double_sided| Material {
        id: mid,
        name: name.into(),
        color: Rgb(10, 20, 30),
        absorption: vec![F64::new(a); n],
        scattering: vec![F64::new(s); n],
        reflection_law: law,
        transmission_loss_db: tl.map(|t| vec![F64::new(t); n]),
        double_sided,
        solver_id: None,
    };
    p.materials.push(mat(
        absorber,
        "Absorber",
        0.6,
        0.3,
        ReflectionLaw::Uniform,
        None,
        false,
    ));
    p.materials.push(mat(
        partition,
        "Partition",
        0.5,
        0.0,
        ReflectionLaw::Specular,
        Some(10.0),
        true,
    ));
    p.materials.push(mat(
        unused,
        "Unused",
        0.1,
        0.5,
        ReflectionLaw::SemiDiffuse,
        None,
        true,
    ));
    p.surface_groups.push(SurfaceGroup {
        id: floor,
        name: "Floor".into(),
        material: absorber,
    });
    p.surface_groups.push(SurfaceGroup {
        id: ceiling,
        name: "Ceiling".into(),
        material: partition,
    });
    for (i, f) in p.geometry.faces.iter_mut().enumerate() {
        f.group = match i {
            0 | 1 => floor,
            10 | 11 => ceiling,
            _ => walls,
        };
    }
    let src = |k, name: &str, pos: [f64; 3], power, directivity, enabled| Source {
        id: SourceId::from_u128(id(k)),
        name: name.into(),
        enabled,
        position: Vec3::from(pos),
        power,
        directivity,
        delay_s: F64::new(0.01),
    };
    p.sources.push(src(
        21,
        "Directional",
        [4.5, 2.5, 3.75],
        Spectrum::new(85.0, SpectrumShape::White),
        Directivity::Unidirectional {
            direction: Vec3::new(-1.0, 0.0, 0.0),
        },
        true,
    ));
    p.sources.push(src(
        22,
        "Loudspeaker",
        [2.5, 4.5, 1.25],
        Spectrum::new(80.0, SpectrumShape::Pink),
        Directivity::Balloon {
            file: "directivities/speaker-test3.txt".into(),
            direction: Vec3::new(0.0, -1.0, 0.0),
        },
        true,
    ));
    p.sources.push(src(
        23,
        "Disabled",
        [3.75, 0.5, 2.5],
        Spectrum::new(80.0, SpectrumShape::Pink),
        Directivity::PlaneXy,
        false,
    ));
    p.point_receivers.push(PointReceiver {
        id: PointReceiverId::from_u128(id(31)),
        name: "Receiver 2".into(),
        position: Vec3::new(0.5, 2.5, 3.75),
        orientation: Vec3::new(0.0, 0.0, 1.0),
        background_noise: Some(Spectrum::new(20.0, SpectrumShape::White)),
    });
    p.surface_receivers = vec![
        SurfaceReceiver {
            id: SurfaceReceiverId::from_u128(id(41)),
            name: "Floor map".into(),
            enabled: true,
            shape: SurfaceReceiverShape::Scene {
                groups: vec![floor],
            },
        },
        SurfaceReceiver {
            id: SurfaceReceiverId::from_u128(id(42)),
            name: "Mid-height plane".into(),
            enabled: true,
            shape: SurfaceReceiverShape::CuttingPlane {
                a: Vec3::new(0.5, 4.5, 2.5),
                b: Vec3::new(0.5, 0.5, 2.5),
                c: Vec3::new(4.5, 0.5, 2.5),
                resolution_m: F64::new(0.5),
            },
        },
        SurfaceReceiver {
            id: SurfaceReceiverId::from_u128(id(43)),
            name: "Ceiling map (off)".into(),
            enabled: false,
            shape: SurfaceReceiverShape::Scene {
                groups: vec![ceiling],
            },
        },
    ];
    p.fitting_zones.push(FittingZone {
        id: FittingZoneId::from_u128(id(51)),
        name: "Chairs".into(),
        enabled: true,
        shape: FittingShape::Box {
            min: Vec3::new(3.6, 3.6, 0.4),
            max: Vec3::new(4.4, 4.4, 1.2),
        },
        absorption: vec![F64::new(0.2); n],
        mean_free_path_m: vec![F64::new(1.5); n],
        diffusion_law: (0..n).map(|i| DiffusionLaw::ALL[i % 3]).collect(),
    });
    let mut v = Variant {
        id: VariantId::from_u128(id(61)),
        name: "Absorbent walls".into(),
        overrides: Vec::new(),
    };
    v.set_override(walls, Some(absorber));
    p.variants.push(v);
    p.environment.air_absorption = AirAbsorption::UserDefined {
        value: F64::new(0.005),
        unit: AttenuationUnit::DecibelPerMetre,
    };
    p.solvers.spps.particles_saved = 10;
    p.solvers.spps.sound_map = SoundMapQuantity::Spl;
    p.solvers.spps.echogram_per_source = true;
    p.solvers.spps.save_surface_intersections = false;
    p.solvers.tcr.air_absorption = false;
    p.solvers.tcr.bands_computed[1] = false;
    p.check_integrity().expect("rich_cube is consistent");
    p
}

// ---------------------------------------------------------------------------------------------
// docs/formats/config_xml.md

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Int,
    Real,
    Str,
}

#[derive(Clone, Debug)]
pub struct DocAttr {
    /// `element@attribute`, e.g. `source/bfreq@db`.
    pub key: String,
    pub kind: Kind,
    /// The "Read by" column.
    pub read_by: String,
    /// The "Writer" column.
    pub writer: String,
}

/// Splits a Markdown table row on `|` that are not escaped as `\|`.
fn cells(row: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut chars = row.trim().trim_start_matches('|').chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\\' if chars.peek() == Some(&'|') => {
                cur.push('|');
                chars.next();
            }
            '|' => out.push(std::mem::take(&mut cur).trim().to_string()),
            c => cur.push(c),
        }
    }
    if !cur.trim().is_empty() {
        out.push(cur.trim().to_string());
    }
    out
}

/// Backticked `element@attribute` keys in a table cell.
fn keys_in(cell: &str) -> Vec<String> {
    cell.split('`')
        .skip(1)
        .step_by(2)
        .filter(|t| {
            let Some((e, a)) = t.split_once('@') else {
                return false;
            };
            !e.is_empty()
                && !a.is_empty()
                && e.bytes()
                    .all(|b| b.is_ascii_lowercase() || b == b'_' || b == b'/')
                && a.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
        })
        .map(str::to_string)
        .collect()
}

/// Every attribute the reference tables of `docs/formats/config_xml.md` say a solver reads.
pub fn doc_attrs() -> Vec<DocAttr> {
    let md = read_text("docs/formats/config_xml.md");
    let start = md
        .find("## Element and attribute reference")
        .expect("reference section");
    let end = start
        + md[start..]
            .find("## Directivity files")
            .expect("next section");
    let mut out = Vec::new();
    for line in md[start..end].lines() {
        if !line.starts_with("| `") {
            continue;
        }
        let c = cells(line);
        assert_eq!(c.len(), 6, "reference rows have 6 columns: {line}");
        let kind = if c[1].starts_with("int") {
            Kind::Int
        } else if c[1].starts_with("real") {
            Kind::Real
        } else if c[1].starts_with("string") {
            Kind::Str
        } else {
            panic!("unknown type column '{}' in {line}", c[1]);
        };
        for key in keys_in(&c[0]) {
            out.push(DocAttr {
                key,
                kind,
                read_by: c[2].clone(),
                writer: c[4].clone(),
            });
        }
    }
    out
}

/// The attributes the page's "Ignored by the solvers" table lists, as `element@attribute`
/// (with `simulation@intensity_*` expanded), and the ignored elements.
pub fn doc_ignored() -> (Vec<String>, Vec<String>) {
    let md = read_text("docs/formats/config_xml.md");
    let start = md
        .find("## Ignored by the solvers")
        .expect("ignored section");
    let end = start + md[start..].find("**Receipts.**").expect("receipts");
    let mut attrs = Vec::new();
    let mut elements = Vec::new();
    for line in md[start..end].lines() {
        if !line.starts_with("| `") {
            continue;
        }
        let first = &cells(line)[0];
        attrs.extend(keys_in(first));
        for t in first.split('`').skip(1).step_by(2) {
            if let Some(e) = t.strip_prefix('<').and_then(|t| t.strip_suffix('>')) {
                elements.push(e.to_string());
            }
        }
    }
    (attrs, elements)
}

// ---------------------------------------------------------------------------------------------
// What the solvers read

/// A value as the solver holds it.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Value {
    /// `atoi`.
    Int(i64),
    /// `atof` into a `float`, by bits.
    Real(u32),
    Str(String),
    /// Text the solver would read as something else than it says (for example `1e3` as an int).
    Unparsed(String),
}

impl std::fmt::Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Value::Int(i) => write!(f, "{i}"),
            Value::Real(bits) => write!(f, "{}", f32::from_bits(*bits)),
            Value::Str(s) => write!(f, "'{s}'"),
            Value::Unparsed(s) => write!(f, "unparsed '{s}'"),
        }
    }
}

pub fn parse_value(kind: Kind, text: &str) -> Value {
    match kind {
        Kind::Int => {
            let t = text.trim();
            let digits = t.strip_prefix(['+', '-']).unwrap_or(t);
            if !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit()) {
                Value::Int(t.parse().unwrap())
            } else {
                Value::Unparsed(text.to_string())
            }
        }
        Kind::Real => match text.replacen(',', ".", 1).trim().parse::<f64>() {
            Ok(v) if v.is_finite() => Value::Real((v as f32).to_bits()),
            _ => Value::Unparsed(text.to_string()),
        },
        Kind::Str => Value::Str(text.to_string()),
    }
}

/// One element as the solvers find it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Instance {
    /// `element@...` key prefix: the element name, `parent/bfreq` for a band entry.
    pub key: String,
    /// Every attribute, raw.
    pub attrs: BTreeMap<String, String>,
}

/// A config as the solvers see it: each element instance under a path that says how the solvers
/// find it (`sources/source[0]` by position, `type_surface[id=21]` by id, band entries by their
/// rank after sorting by `@freq`, `freq_enum/bfreq[freq=125]` by frequency).
pub fn solver_view(xml: &str) -> BTreeMap<String, Instance> {
    let doc = roxmltree::Document::parse(xml).expect("well-formed XML");
    let root = doc.root_element();
    let mut out = BTreeMap::new();
    let attrs = |n: roxmltree::Node| -> BTreeMap<String, String> {
        n.attributes()
            .map(|a| (a.name().to_string(), a.value().to_string()))
            .collect()
    };
    fn elements<'a, 'i>(n: roxmltree::Node<'a, 'i>) -> Vec<roxmltree::Node<'a, 'i>> {
        n.children().filter(|c| c.is_element()).collect()
    }
    let add_bands = |out: &mut BTreeMap<String, Instance>, path: &str, n: roxmltree::Node| {
        let mut bands: Vec<_> = elements(n);
        bands.sort_by_key(|b| {
            b.attribute("freq")
                .and_then(|f| f.trim().parse::<i64>().ok())
                .unwrap_or(0)
        });
        for (rank, b) in bands.into_iter().enumerate() {
            let parent = n.tag_name().name();
            out.insert(
                format!("{path}/bfreq[{rank}]"),
                Instance {
                    key: format!("{parent}/bfreq"),
                    attrs: attrs(b),
                },
            );
        }
    };
    out.insert(
        "configuration".to_string(),
        Instance {
            key: root.tag_name().name().to_string(),
            attrs: attrs(root),
        },
    );
    for top in elements(root) {
        let name = top.tag_name().name();
        match name {
            "simulation" => {
                out.insert(
                    "simulation".into(),
                    Instance {
                        key: "simulation".into(),
                        attrs: attrs(top),
                    },
                );
                for fe in elements(top)
                    .into_iter()
                    .filter(|c| c.tag_name().name() == "freq_enum")
                {
                    for b in elements(fe) {
                        let f = b.attribute("freq").unwrap_or("?").trim().to_string();
                        out.insert(
                            format!("freq_enum/bfreq[freq={f}]"),
                            Instance {
                                key: "freq_enum/bfreq".into(),
                                attrs: attrs(b),
                            },
                        );
                    }
                }
            }
            "condition_atmospherique" => {
                out.insert(
                    name.into(),
                    Instance {
                        key: name.into(),
                        attrs: attrs(top),
                    },
                );
            }
            "surface_absorption_enum" => {
                for m in elements(top) {
                    let id = m.attribute("id").unwrap_or("?").trim().to_string();
                    let path = format!("type_surface[id={id}]");
                    out.insert(
                        path.clone(),
                        Instance {
                            key: m.tag_name().name().into(),
                            attrs: attrs(m),
                        },
                    );
                    add_bands(&mut out, &path, m);
                }
            }
            "sources" | "recepteursp" | "encombrement_enum" => {
                for (i, s) in elements(top).into_iter().enumerate() {
                    let path = format!("{name}/{}[{i}]", s.tag_name().name());
                    out.insert(
                        path.clone(),
                        Instance {
                            key: s.tag_name().name().into(),
                            attrs: attrs(s),
                        },
                    );
                    add_bands(&mut out, &path, s);
                }
            }
            "recepteurss" => {
                let mut counts: BTreeMap<String, usize> = BTreeMap::new();
                for r in elements(top) {
                    let tag = r.tag_name().name().to_string();
                    let i = counts.entry(tag.clone()).or_default();
                    out.insert(
                        format!("recepteurss/{tag}[{i}]"),
                        Instance {
                            key: tag.clone(),
                            attrs: attrs(r),
                        },
                    );
                    *i += 1;
                }
            }
            _ => {
                out.insert(
                    format!("other:{name}"),
                    Instance {
                        key: name.into(),
                        attrs: attrs(top),
                    },
                );
            }
        }
    }
    out
}

/// Every element name in a document, with its parent's name.
pub fn element_names(xml: &str) -> Vec<(String, String)> {
    let doc = roxmltree::Document::parse(xml).expect("well-formed XML");
    doc.descendants()
        .filter(|n| n.is_element())
        .map(|n| {
            let parent = n
                .parent_element()
                .map(|p| p.tag_name().name().to_string())
                .unwrap_or_default();
            (parent, n.tag_name().name().to_string())
        })
        .collect()
}

// ---------------------------------------------------------------------------------------------
// Solver runs

pub struct Run {
    pub dir: PathBuf,
    pub exit: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

impl Run {
    /// Lines matching `Xml Property .* doesn't exist`, on stdout and stderr.
    pub fn xml_property_lines(&self) -> Vec<String> {
        self.stdout
            .lines()
            .chain(self.stderr.lines())
            .filter(|l| {
                l.find("Xml Property ")
                    .is_some_and(|i| l[i..].contains(" doesn't exist"))
            })
            .map(str::to_string)
            .collect()
    }

    pub fn has_line(&self, prefix: &str) -> bool {
        self.stdout.lines().any(|l| l.starts_with(prefix))
    }
}

pub fn solver_exe(name: &str) -> PathBuf {
    let p = repo_file(&format!("target/solvers/bin/{name}"));
    assert!(
        p.is_file(),
        "{} is missing: these tests run the M1 solver build (tools/gates/m1.ps1)",
        p.display()
    );
    p
}

/// Runs `exe config.xml` with the run folder as the working directory, the way upstream's GUI and
/// the M1 gate launch it, and keeps its output beside the results.
pub fn run_solver(exe: &Path, dir: &Path) -> Run {
    let out = Command::new(exe)
        .arg("config.xml")
        .current_dir(dir)
        .output()
        .unwrap_or_else(|e| panic!("{}: {e}", exe.display()));
    std::fs::write(dir.join("_stdout.txt"), &out.stdout).unwrap();
    std::fs::write(dir.join("_stderr.txt"), &out.stderr).unwrap();
    Run {
        dir: dir.to_path_buf(),
        exit: out.status.code(),
        stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
    }
}

/// Every output file of a run folder (inputs and captured streams left out), by relative path.
pub fn outputs(dir: &Path, inputs: &[&str]) -> BTreeMap<String, Vec<u8>> {
    let mut out = BTreeMap::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                stack.push(p);
                continue;
            }
            let rel = p
                .strip_prefix(dir)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/");
            let name = p.file_name().unwrap().to_string_lossy().to_string();
            if inputs.contains(&rel.as_str()) || name.starts_with("_std") {
                continue;
            }
            out.insert(rel, std::fs::read(&p).unwrap());
        }
    }
    out
}
