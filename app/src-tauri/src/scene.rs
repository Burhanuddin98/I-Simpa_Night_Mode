//! What M10's UI reads about the open project (docs/investigations/2026-09-29-m10/PLAN.md,
//! section 1): the [`SceneState`] every M10 command returns, the check summary and its Console
//! wording, the UI codes, the placeholder rule for "assigned", and the binary mesh buffer.
//!
//! Everything here is a pure function of a project and its check report; the session in
//! `bridge.rs` caches and sequences them. No value here is computed by a solver: areas, volumes
//! and counts are geometry facts.

use std::collections::HashMap;

use schemars::JsonSchema;
use serde::Serialize;
use simpa_core::geometry::check::{CheckReport, ReasonCode, Verdict};
use simpa_core::geometry::import::{ImportReport, REFERENCE_MATERIALS};
use simpa_core::schema::{
    BandSet, EntityRef, GroupId, Material, PointReceiver, Project, Source, SurfaceGroup,
    SurfaceReceiver, Variant, VariantId,
};
use simpa_core::validate::{Issue, Severity, codes};

use crate::bridge::ProjectInfo;
use crate::events::LineClass;

/// The state of the open project as the M10 UI shows it. Returned by every M10 command.
#[derive(Clone, Debug, Serialize, JsonSchema)]
pub struct SceneState {
    pub info: ProjectInfo,
    /// The project without its mesh; the mesh travels as bytes (`scene_mesh`).
    pub view: ProjectView,
    /// One per surface group, in project order.
    pub groups: Vec<GroupStats>,
    /// The model check of the current geometry; `None` when the project has no faces.
    pub check: Option<CheckSummary>,
    /// The validator's issues on the current project, with their UI codes.
    pub issues: Vec<UiIssue>,
    /// Why Run is disabled, as UI codes. Never empty in M10: `M11_PENDING` is always present.
    pub run_blockers: Vec<String>,
    /// Console lines produced since the last state was returned.
    pub lines: Vec<LogLine>,
}

/// The project without its geometry.
#[derive(Clone, Debug, Serialize, JsonSchema)]
pub struct ProjectView {
    pub name: String,
    pub description: String,
    pub bands: BandSet,
    pub surface_groups: Vec<SurfaceGroup>,
    pub materials: Vec<Material>,
    pub sources: Vec<Source>,
    pub point_receivers: Vec<PointReceiver>,
    pub surface_receivers: Vec<SurfaceReceiver>,
    pub variants: Vec<Variant>,
    pub active_variant: Option<VariantId>,
}

/// A surface group's geometry facts, and whether it has a real material.
#[derive(Clone, Debug, PartialEq, Serialize, JsonSchema)]
pub struct GroupStats {
    pub id: GroupId,
    pub faces: u32,
    pub area_m2: f64,
    /// False when its effective material under the active variant is the placeholder
    /// ([`is_placeholder`]).
    pub assigned: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CheckVerdict {
    Ok,
    Refused,
}

/// One reason the check refused the geometry.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, JsonSchema)]
pub struct CheckReasonOut {
    /// The core's stable reason code (`open_boundary`, ...).
    pub code: String,
    /// How many findings, as the message counts them.
    pub count: usize,
    pub repairable: bool,
    /// How many faces are involved.
    pub faces: usize,
    pub message: String,
}

/// The check's counts that the UI shows.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, JsonSchema)]
pub struct CheckCounts {
    pub vertices: usize,
    pub faces: usize,
    pub edges: usize,
    /// Census edges used once (by index, before analysis).
    pub open_edges: usize,
    pub nonmanifold_edges: usize,
    pub self_intersecting_pairs: usize,
    pub self_intersecting_faces: usize,
    pub degenerate_faces: usize,
    pub duplicate_faces: usize,
    pub invalid_faces: usize,
    pub inverted_faces: usize,
    pub exterior_faces: usize,
    /// Open edges on the outer shell, after analysis.
    pub boundary_open_edges: usize,
    pub cells: usize,
    pub components: usize,
    pub coincident_vertices: usize,
}

/// The model check, for the Geometry panel and the view's highlight.
#[derive(Clone, Debug, PartialEq, Serialize, JsonSchema)]
pub struct CheckSummary {
    pub verdict: CheckVerdict,
    /// In the core's reason order; empty exactly when the verdict is ok.
    pub reasons: Vec<CheckReasonOut>,
    pub counts: CheckCounts,
    pub area_m2: f64,
    pub enclosed_volume_m3: f64,
    /// The bounding box of the vertices the faces use.
    pub bbox_min: [f64; 3],
    pub extents_m: [f64; 3],
    /// Every face any reason names, ascending, unique.
    pub highlight_faces: Vec<u32>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum IssueSeverity {
    Error,
    Warning,
}

/// A validator issue as the UI shows it: the UI code and the core rule, both.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, JsonSchema)]
pub struct UiIssue {
    /// The UI code ([`ui_code`]), e.g. `RECEIVER_OUTSIDE`.
    pub code: String,
    /// The core's rule code, e.g. `receiver_outside_volume`.
    pub rule: String,
    pub severity: IssueSeverity,
    /// The validator's JSON pointer into the project.
    pub path: String,
    /// The entity the pointer names, resolved in the issue's own project.
    pub entity: Option<EntityRef>,
    /// The pointer after the entity, e.g. `position` or `absorption/3`; empty when there is no
    /// entity or the pointer names the entity itself.
    pub field: String,
    pub message: String,
}

/// One Console line.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, JsonSchema)]
pub struct LogLine {
    pub class: LineClass,
    pub text: String,
}

impl LogLine {
    pub fn new(class: LineClass, text: impl Into<String>) -> Self {
        LogLine {
            class,
            text: text.into(),
        }
    }
}

/// The result of `edit_apply`, the checked apply (PLAN.md 1.8).
#[derive(Clone, Debug, Serialize, JsonSchema)]
pub struct EditOutcome {
    /// False when the validator refused the edit: the project and its history are unchanged.
    pub applied: bool,
    /// The new errors the edit would have introduced.
    pub refusals: Vec<UiIssue>,
    pub state: SceneState,
}

// ---- UI codes ---------------------------------------------------------------------------------

/// Run blockers only the app produces.
pub const GEOMETRY_REFUSED: &str = "GEOMETRY_REFUSED";
pub const MATERIALS_UNASSIGNED: &str = "MATERIALS_UNASSIGNED";
/// Always present in M10: Run is wired in M11.
pub const M11_PENDING: &str = "M11_PENDING";

/// The UI code of a core rule or structural code (PLAN.md 1.7). The core codes are stable API and
/// stay as they are; the UI shows both.
pub fn ui_code(rule: &str) -> String {
    let named = match rule {
        codes::RECEIVER_OUTSIDE_VOLUME => "RECEIVER_OUTSIDE",
        codes::RECEIVER_ON_SURFACE => "RECEIVER_ON_SURFACE",
        codes::RECEIVER_SPHERE_CROSSES_SURFACE => "RECEIVER_SPHERE_CROSSES",
        codes::SOURCE_OUTSIDE_VOLUME => "SOURCE_OUTSIDE",
        codes::SOURCE_NEAR_SURFACE => "SOURCE_NEAR_SURFACE",
        codes::NAME_NOT_FILENAME_SAFE => "LABEL_UNSAFE",
        codes::NAME_TOO_LONG => "LABEL_TOO_LONG",
        codes::NAME_DUPLICATE => "LABEL_DUPLICATE",
        other => return other.to_ascii_uppercase(),
    };
    named.to_string()
}

/// The entity a validator pointer names, and the rest of the pointer after it.
pub fn resolve(project: &Project, path: &str) -> (Option<EntityRef>, String) {
    let mut parts = path.strip_prefix('/').unwrap_or(path).splitn(3, '/');
    let (Some(list), Some(index)) = (parts.next(), parts.next()) else {
        return (None, String::new());
    };
    let rest = parts.next().unwrap_or("").to_string();
    let Ok(i) = index.parse::<usize>() else {
        return (None, String::new());
    };
    let entity = match list {
        "surface_groups" => project
            .surface_groups
            .get(i)
            .map(|x| EntityRef::SurfaceGroup(x.id)),
        "materials" => project.materials.get(i).map(|x| EntityRef::Material(x.id)),
        "sources" => project.sources.get(i).map(|x| EntityRef::Source(x.id)),
        "point_receivers" => project
            .point_receivers
            .get(i)
            .map(|x| EntityRef::PointReceiver(x.id)),
        "surface_receivers" => project
            .surface_receivers
            .get(i)
            .map(|x| EntityRef::SurfaceReceiver(x.id)),
        "fitting_zones" => project
            .fitting_zones
            .get(i)
            .map(|x| EntityRef::FittingZone(x.id)),
        "variants" => project.variants.get(i).map(|x| EntityRef::Variant(x.id)),
        _ => None,
    };
    match entity {
        Some(e) => (Some(e), rest),
        None => (None, String::new()),
    }
}

/// A core issue with its UI code and its entity, resolved in `project`.
pub fn ui_issue(project: &Project, issue: &Issue) -> UiIssue {
    let (entity, field) = resolve(project, &issue.path);
    UiIssue {
        code: ui_code(issue.code),
        rule: issue.code.to_string(),
        severity: match issue.severity {
            Severity::Error => IssueSeverity::Error,
            Severity::Warning => IssueSeverity::Warning,
        },
        path: issue.path.clone(),
        entity,
        field,
        message: issue.message.clone(),
    }
}

/// An issue's identity across an edit (PLAN.md 1.8, point 4): the rule with the entity and field
/// when the pointer names an entity, else the rule with the pointer. An insertion shifts list
/// indices, so the pointer alone would make every issue below it look new.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum IssueKey {
    Entity(String, EntityRef, String),
    Path(String, String),
}

pub fn issue_key(issue: &UiIssue) -> IssueKey {
    match issue.entity {
        Some(e) => IssueKey::Entity(issue.rule.clone(), e, issue.field.clone()),
        None => IssueKey::Path(issue.rule.clone(), issue.path.clone()),
    }
}

// ---- the placeholder rule -----------------------------------------------------------------------

/// Upstream's reference material 0 as an import leaves it: named `Default`, absorption and
/// scattering 0 in every band. Upstream gives it to a face "that no surface group holds"; the UI
/// counts it as no material chosen (PLAN.md F1).
pub fn is_placeholder(material: &Material) -> bool {
    material.name == REFERENCE_MATERIALS[0].name
        && material.absorption.iter().all(|v| v.get() == 0.0)
        && material.scattering.iter().all(|v| v.get() == 0.0)
}

/// Whether the group's effective material under the active variant is a real one.
pub fn group_assigned(project: &Project, group: GroupId) -> bool {
    project
        .active_material(group)
        .and_then(|m| project.material(m))
        .is_some_and(|m| !is_placeholder(m))
}

pub fn groups_assigned(project: &Project) -> usize {
    project
        .surface_groups
        .iter()
        .filter(|g| group_assigned(project, g.id))
        .count()
}

// ---- geometry facts ---------------------------------------------------------------------------

fn triangle_area(p: [[f64; 3]; 3]) -> f64 {
    let u = [0, 1, 2].map(|k| p[1][k] - p[0][k]);
    let v = [0, 1, 2].map(|k| p[2][k] - p[0][k]);
    let n = [
        u[1] * v[2] - u[2] * v[1],
        u[2] * v[0] - u[0] * v[2],
        u[0] * v[1] - u[1] * v[0],
    ];
    0.5 * (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt()
}

/// Face count and area per surface group, in project order.
pub fn group_stats(project: &Project) -> Vec<GroupStats> {
    let index: HashMap<GroupId, usize> = project
        .surface_groups
        .iter()
        .enumerate()
        .map(|(i, g)| (g.id, i))
        .collect();
    let mut faces = vec![0u32; index.len()];
    let mut area = vec![0.0f64; index.len()];
    let v = &project.geometry.vertices;
    for f in &project.geometry.faces {
        let Some(&g) = index.get(&f.group) else {
            continue;
        };
        faces[g] += 1;
        let corners = f.vertices.map(|i| v.get(i as usize).map(|p| p.to_array()));
        if let [Some(a), Some(b), Some(c)] = corners {
            area[g] += triangle_area([a, b, c]);
        }
    }
    project
        .surface_groups
        .iter()
        .enumerate()
        .map(|(i, g)| GroupStats {
            id: g.id,
            faces: faces[i],
            area_m2: area[i],
            assigned: group_assigned(project, g.id),
        })
        .collect()
}

/// The bounding box of the vertices the faces use: (min, extents). Zeros when no face has its
/// vertices in range.
fn bounding_box(project: &Project) -> ([f64; 3], [f64; 3]) {
    let v = &project.geometry.vertices;
    let mut lo = [f64::INFINITY; 3];
    let mut hi = [f64::NEG_INFINITY; 3];
    for f in &project.geometry.faces {
        for &i in &f.vertices {
            if let Some(p) = v.get(i as usize) {
                let p = p.to_array();
                for k in 0..3 {
                    lo[k] = lo[k].min(p[k]);
                    hi[k] = hi[k].max(p[k]);
                }
            }
        }
    }
    if lo[0] > hi[0] {
        return ([0.0; 3], [0.0; 3]);
    }
    (lo, [0, 1, 2].map(|k| hi[k] - lo[k]))
}

/// The check report as the UI reads it.
pub fn check_summary(project: &Project, report: &CheckReport) -> CheckSummary {
    let c = &report.counts;
    let mut highlight: Vec<u32> = report
        .reasons
        .iter()
        .flat_map(|r| r.faces.iter().copied())
        .collect();
    highlight.sort_unstable();
    highlight.dedup();
    let (bbox_min, extents_m) = bounding_box(project);
    CheckSummary {
        verdict: match report.verdict {
            Verdict::Ok => CheckVerdict::Ok,
            Verdict::Refused => CheckVerdict::Refused,
        },
        reasons: report
            .reasons
            .iter()
            .map(|r| CheckReasonOut {
                code: r.code.as_str().to_string(),
                count: r.count,
                repairable: r.repairable,
                faces: r.faces.len(),
                message: r.message.clone(),
            })
            .collect(),
        counts: CheckCounts {
            vertices: c.vertices,
            faces: c.faces,
            edges: c.edges,
            open_edges: c.open_edges,
            nonmanifold_edges: c.nonmanifold_edges,
            self_intersecting_pairs: c.self_intersecting_pairs,
            self_intersecting_faces: c.self_intersecting_faces,
            degenerate_faces: c.degenerate_faces,
            duplicate_faces: c.duplicate_faces,
            invalid_faces: c.invalid_faces,
            inverted_faces: c.inverted_faces,
            exterior_faces: c.exterior_faces,
            boundary_open_edges: c.boundary_open_edges,
            cells: c.cells,
            components: c.components,
            coincident_vertices: c.coincident_vertices,
        },
        area_m2: report.measures.area_m2,
        enclosed_volume_m3: report.measures.enclosed_volume_m3,
        bbox_min,
        extents_m,
        highlight_faces: highlight,
    }
}

// ---- Console wording (PLAN.md 1.11) -------------------------------------------------------------

/// The literal the gate reads from the Console when the check passes.
pub const CHECK_OK_PREFIX: &str = "Closed volume, 0 self-intersections";

fn plural(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

fn sentence(text: &str) -> String {
    if text.ends_with('.') {
        text.to_string()
    } else {
        format!("{text}.")
    }
}

/// The check's Console lines: one INFO line when it passes, one FAIL line per reason, in the
/// core's reason order, when it refuses. `file` names the model.
pub fn check_lines(file: &str, report: &CheckReport, surface_groups: usize) -> Vec<LogLine> {
    let c = &report.counts;
    if report.is_ok() {
        return vec![LogLine::new(
            LineClass::Info,
            format!(
                "{CHECK_OK_PREFIX} · {} faces, {surface_groups} surface groups",
                c.faces
            ),
        )];
    }
    report
        .reasons
        .iter()
        .map(|r| {
            let summary = match r.code {
                ReasonCode::OpenBoundary => format!(
                    "{} in the census ({} on the outer shell after analysis), {} with the \
                     exterior on both sides.",
                    plural(c.open_edges, "open edge", "open edges"),
                    c.boundary_open_edges,
                    plural(c.exterior_faces, "face", "faces")
                ),
                ReasonCode::SelfIntersections => format!(
                    "{}, {}.",
                    plural(
                        c.self_intersecting_pairs,
                        "intersecting face pair",
                        "intersecting face pairs"
                    ),
                    plural(c.self_intersecting_faces, "face", "faces")
                ),
                ReasonCode::EmptyGeometry => "no faces.".to_string(),
                ReasonCode::NoEnclosedVolume => {
                    format!(
                        "{}.",
                        plural(c.cells, "enclosed volume", "enclosed volumes")
                    )
                }
                ReasonCode::UnresolvedTopology => format!(
                    "{} could not be placed.",
                    plural(r.count, "component", "components")
                ),
                ReasonCode::InvalidFaces
                | ReasonCode::DegenerateFaces
                | ReasonCode::DuplicateFaces
                | ReasonCode::InvertedFaces => {
                    format!("{}.", plural(r.faces.len(), "face", "faces"))
                }
            };
            let tail = if r.faces.is_empty() {
                "Run refused."
            } else {
                "Run refused; faces highlighted in the view."
            };
            LogLine::new(
                LineClass::Fail,
                format!(
                    "Model check refused {file}: {}: {summary} {} {tail}",
                    r.code.as_str(),
                    sentence(&r.message)
                ),
            )
        })
        .collect()
}

/// The import report's Console lines, all INFO.
pub fn import_lines(
    file: &str,
    unit: &str,
    up: &str,
    report: &ImportReport,
    groups: usize,
) -> Vec<LogLine> {
    let info = |t: String| LogLine::new(LineClass::Info, t);
    let mut out = vec![info(format!(
        "Imported {file}: {} from {} and {}, {}; unit {unit}, up axis {up}",
        plural(report.triangles, "triangle", "triangles"),
        plural(report.source_faces, "face", "faces"),
        plural(report.source_vertices, "vertex", "vertices"),
        plural(groups, "surface group", "surface groups"),
    ))];
    if report.polygons_split > 0 {
        out.push(info(format!(
            "Import: {} split into triangles from the first vertex",
            plural(report.polygons_split, "polygon", "polygons")
        )));
    }
    if report.welded {
        out.push(info(format!(
            "Import: {} welded to an earlier vertex at the same position",
            plural(report.welded_vertices, "vertex", "vertices")
        )));
    }
    if report.unused_vertices > 0 {
        out.push(info(format!(
            "Import: {} that no face uses dropped",
            plural(report.unused_vertices, "vertex", "vertices")
        )));
    }
    if !report.empty_groups.is_empty() {
        out.push(info(format!(
            "Import: empty groups dropped: {}",
            report.empty_groups.join(", ")
        )));
    }
    out.extend(report.notes.iter().map(|n| info(format!("Import: {n}"))));
    out
}

// ---- run blockers -------------------------------------------------------------------------------

/// Why Run is disabled: the refused check, unassigned groups, each error's UI code once, and
/// `M11_PENDING`, in that order.
pub fn run_blockers(
    project: &Project,
    check: Option<&CheckSummary>,
    issues: &[UiIssue],
) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    if check.is_some_and(|c| c.verdict == CheckVerdict::Refused) {
        out.push(GEOMETRY_REFUSED.to_string());
    }
    if groups_assigned(project) < project.surface_groups.len() {
        out.push(MATERIALS_UNASSIGNED.to_string());
    }
    for i in issues {
        if i.severity == IssueSeverity::Error && !out.contains(&i.code) {
            out.push(i.code.clone());
        }
    }
    out.push(M11_PENDING.to_string());
    out
}

// ---- the mesh buffer (PLAN.md 1.9) --------------------------------------------------------------

pub const MESH_MAGIC: u32 = 0x4853_454D;
pub const MESH_VERSION: u32 = 1;

/// The geometry as little-endian bytes: a 24-byte header (magic, version, vertex count, face
/// count, geometry revision), the positions as f64 (8-aligned at offset 24), the vertex indices
/// and each face's group index (its position in `surface_groups`, `u32::MAX` if unknown).
pub fn mesh_bytes(project: &Project, geometry_rev: u64) -> Vec<u8> {
    let g = &project.geometry;
    let nv = g.vertices.len();
    let nf = g.faces.len();
    let mut out = Vec::with_capacity(24 + 24 * nv + 16 * nf);
    out.extend_from_slice(&MESH_MAGIC.to_le_bytes());
    out.extend_from_slice(&MESH_VERSION.to_le_bytes());
    out.extend_from_slice(&(nv as u32).to_le_bytes());
    out.extend_from_slice(&(nf as u32).to_le_bytes());
    out.extend_from_slice(&geometry_rev.to_le_bytes());
    for v in &g.vertices {
        for c in v.to_array() {
            out.extend_from_slice(&c.to_bits().to_le_bytes());
        }
    }
    for f in &g.faces {
        for i in f.vertices {
            out.extend_from_slice(&i.to_le_bytes());
        }
    }
    let index: HashMap<GroupId, u32> = project
        .surface_groups
        .iter()
        .enumerate()
        .map(|(i, s)| (s.id, i as u32))
        .collect();
    for f in &g.faces {
        let gi = index.get(&f.group).copied().unwrap_or(u32::MAX);
        out.extend_from_slice(&gi.to_le_bytes());
    }
    out
}

pub fn view(project: &Project) -> ProjectView {
    ProjectView {
        name: project.name.clone(),
        description: project.description.clone(),
        bands: project.bands.clone(),
        surface_groups: project.surface_groups.clone(),
        materials: project.materials.clone(),
        sources: project.sources.clone(),
        point_receivers: project.point_receivers.clone(),
        surface_receivers: project.surface_receivers.clone(),
        variants: project.variants.clone(),
        active_variant: project.active_variant,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use simpa_core::geometry::check::check;
    use simpa_core::schema;
    use simpa_core::validate::{RULES, STRUCTURAL_CODES};
    use std::path::PathBuf;

    pub fn fixture(rel: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join(rel)
    }

    fn load(rel: &str) -> Project {
        schema::load(&fixture(rel)).unwrap_or_else(|e| panic!("{rel}: {e}"))
    }

    #[test]
    fn ui_codes_cover_every_core_code() {
        let mut seen = std::collections::HashSet::new();
        for code in RULES.iter().map(|r| r.code).chain(STRUCTURAL_CODES) {
            let ui = ui_code(code);
            assert!(!ui.is_empty(), "{code}");
            assert!(
                ui.bytes().all(|b| b.is_ascii_uppercase() || b == b'_'),
                "{code} -> {ui}"
            );
            assert!(seen.insert(ui.clone()), "two core codes map to {ui}");
        }
        assert_eq!(ui_code("receiver_outside_volume"), "RECEIVER_OUTSIDE");
        assert_eq!(ui_code("name_not_filename_safe"), "LABEL_UNSAFE");
        assert_eq!(
            ui_code("receiver_sphere_crosses_surface"),
            "RECEIVER_SPHERE_CROSSES"
        );
        assert_eq!(
            ui_code("material_value_out_of_range"),
            "MATERIAL_VALUE_OUT_OF_RANGE"
        );
        assert_eq!(ui_code("source_none"), "SOURCE_NONE");
        for app in [GEOMETRY_REFUSED, MATERIALS_UNASSIGNED, M11_PENDING] {
            assert!(!seen.contains(app), "{app} collides with a core code");
        }
    }

    #[test]
    fn pointers_resolve_to_entities_and_fields() {
        let p = load("tests/fixtures/rooms/tutorial1_box.simpa");
        let (e, f) = resolve(&p, "/point_receivers/0/position");
        assert_eq!(e, Some(EntityRef::PointReceiver(p.point_receivers[0].id)));
        assert_eq!(f, "position");
        let (e, f) = resolve(&p, "/materials/0/absorption/3");
        assert_eq!(e, Some(EntityRef::Material(p.materials[0].id)));
        assert_eq!(f, "absorption/3");
        assert_eq!(resolve(&p, "/sources/0").1, "");
        assert_eq!(resolve(&p, "/sources"), (None, String::new()));
        assert_eq!(
            resolve(&p, "/bands/frequencies_hz/2"),
            (None, String::new())
        );
        assert_eq!(resolve(&p, "/sources/99/position"), (None, String::new()));
    }

    #[test]
    fn the_placeholder_rule_on_three_projects() {
        let hall = simpa_core::geometry::import::import_file(
            &fixture("testdata/elmia_corrected.ply"),
            &simpa_core::geometry::import::ImportOptions::new(
                simpa_core::geometry::import::Unit::Metre,
                simpa_core::geometry::import::Up::Z,
            ),
        )
        .unwrap()
        .to_project("elmia_corrected");
        assert_eq!((groups_assigned(&hall), hall.surface_groups.len()), (0, 10));
        let box_ = load("tests/fixtures/rooms/tutorial1_box.simpa");
        assert_eq!((groups_assigned(&box_), box_.surface_groups.len()), (3, 3));
        let room = load("tests/fixtures/ui/teaching_room.simpa");
        assert_eq!((groups_assigned(&room), room.surface_groups.len()), (6, 6));
        // The rule needs all three: a `Default` with a real value counts as chosen.
        let mut m = hall.materials[0].clone();
        assert!(is_placeholder(&m));
        m.scattering[2] = schema::F64::new(0.1);
        assert!(!is_placeholder(&m));
    }

    #[test]
    fn the_check_lines_use_the_gate_s_wording() {
        let box_ = load("tests/fixtures/rooms/tutorial1_box.simpa");
        let lines = check_lines("box.simpa", &check(&box_.geometry), 3);
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].class, LineClass::Info);
        assert_eq!(
            lines[0].text,
            "Closed volume, 0 self-intersections · 12 faces, 3 surface groups"
        );

        let mut open = box_.clone();
        open.geometry.faces.remove(5);
        let report = check(&open.geometry);
        let census = report.counts.open_edges;
        assert_eq!(census, 3);
        let lines = check_lines("box.simpa", &report, 3);
        assert!(lines.iter().all(|l| l.class == LineClass::Fail));
        let ob: Vec<&LogLine> = lines
            .iter()
            .filter(|l| l.text.contains("open_boundary"))
            .collect();
        assert_eq!(ob.len(), 1, "{lines:?}");
        assert!(
            ob[0]
                .text
                .starts_with("Model check refused box.simpa: open_boundary: ")
        );
        assert!(ob[0].text.contains("open"));
        assert!(
            ob[0]
                .text
                .contains(&format!("{census} open edges in the census"))
        );
        assert!(
            ob[0]
                .text
                .ends_with("Run refused; faces highlighted in the view.")
        );
        assert!(!lines.iter().any(|l| l.text.starts_with(CHECK_OK_PREFIX)));
    }

    #[test]
    fn the_check_summary_highlights_every_reason_s_faces() {
        let mut open = load("tests/fixtures/rooms/tutorial1_box.simpa");
        open.geometry.faces.remove(5);
        let report = check(&open.geometry);
        let s = check_summary(&open, &report);
        assert_eq!(s.verdict, CheckVerdict::Refused);
        let mut want: Vec<u32> = report
            .reasons
            .iter()
            .flat_map(|r| r.faces.clone())
            .collect();
        want.sort_unstable();
        want.dedup();
        assert!(!want.is_empty());
        assert_eq!(s.highlight_faces, want);
        let ok = load("tests/fixtures/rooms/tutorial1_box.simpa");
        let s = check_summary(&ok, &check(&ok.geometry));
        assert_eq!(s.verdict, CheckVerdict::Ok);
        assert!(s.highlight_faces.is_empty() && s.reasons.is_empty());
        assert!(
            (s.enclosed_volume_m3 - 180.0).abs() < 1e-9,
            "{}",
            s.enclosed_volume_m3
        );
        assert!((s.area_m2 - 216.0).abs() < 1e-9);
    }

    #[test]
    fn the_mesh_buffer_round_trips() {
        let p = load("tests/fixtures/rooms/tutorial1_box.simpa");
        let bytes = mesh_bytes(&p, 7);
        let u32_at = |o: usize| u32::from_le_bytes(bytes[o..o + 4].try_into().unwrap());
        let nv = p.geometry.vertices.len();
        let nf = p.geometry.faces.len();
        assert_eq!(u32_at(0), MESH_MAGIC);
        assert_eq!(&bytes[0..4], b"MESH");
        assert_eq!(u32_at(4), 1);
        assert_eq!((u32_at(8) as usize, u32_at(12) as usize), (nv, nf));
        assert_eq!(u64::from_le_bytes(bytes[16..24].try_into().unwrap()), 7);
        assert_eq!(bytes.len(), 24 + 24 * nv + 16 * nf);
        for (i, v) in p.geometry.vertices.iter().enumerate() {
            for (k, c) in v.to_array().into_iter().enumerate() {
                let o = 24 + 24 * i + 8 * k;
                let got = u64::from_le_bytes(bytes[o..o + 8].try_into().unwrap());
                assert_eq!(got, c.to_bits());
            }
        }
        let fo = 24 + 24 * nv;
        let go = fo + 12 * nf;
        for (i, f) in p.geometry.faces.iter().enumerate() {
            for k in 0..3 {
                assert_eq!(u32_at(fo + 12 * i + 4 * k), f.vertices[k]);
            }
            let g = u32_at(go + 4 * i) as usize;
            assert_eq!(p.surface_groups[g].id, f.group);
        }
    }

    #[test]
    fn group_stats_count_faces_and_area() {
        let p = load("tests/fixtures/rooms/tutorial1_box.simpa");
        let stats = group_stats(&p);
        let by_name: HashMap<&str, &GroupStats> = p
            .surface_groups
            .iter()
            .zip(&stats)
            .map(|(g, s)| (g.name.as_str(), s))
            .collect();
        assert_eq!(by_name["Ceiling"].faces, 2);
        assert_eq!(by_name["Walls"].faces, 8);
        let total: f64 = stats.iter().map(|s| s.area_m2).sum();
        assert!((total - 216.0).abs() < 1e-9, "{total}");
    }
}
