//! The typed bridge between the UI and `simpa_core::schema`.
//!
//! Schema values cross the IPC boundary as JSON **text** and are read here with the core's exact
//! reader (`Op::from_json`, `schema::from_json`, `schema::from_json_exact`). No command argument
//! is ever typed as a schema struct: Tauri would deserialize it with `serde_json`, which in this
//! build misreads some floats (the schema module docs; [`exact_float_probe`] shows it live). The
//! TypeScript side of the same types is generated from the core's JSON Schema
//! (`ui/src/bindings/`).
//!
//! The [`Session`] holds the open project, its undo history, and what M10 keeps beside them
//! (docs/investigations/2026-09-29-m10/PLAN.md 1.10): the dirty serials, the geometry revision,
//! the cached model check and validator issues, and the Console lines not yet returned.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use schemars::JsonSchema;
use serde::Serialize;
use simpa_core::geometry::check;
use simpa_core::geometry::import::{self, ImportOptions, Unit, Up};
use simpa_core::schema::{self, F64, History, LoadError, Op, OpError, Project};
use simpa_core::validate::{self, Context};

use crate::events::LineClass;
use crate::guard::{CmdError, CmdResult};
use crate::scene::{
    self, CheckSummary, EditOutcome, IssueKey, IssueSeverity, LogLine, SceneState, UiIssue,
};

/// What the chrome shows about the open project: names and counts, never an acoustic value.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, JsonSchema)]
pub struct ProjectInfo {
    pub id: String,
    pub name: String,
    pub path: Option<String>,
    pub vertices: usize,
    pub faces: usize,
    pub surface_groups: usize,
    pub materials: usize,
    pub sources: usize,
    pub point_receivers: usize,
    pub surface_receivers: usize,
    pub variants: Vec<VariantInfo>,
    pub active_variant: Option<String>,
    pub can_undo: bool,
    pub can_redo: bool,
    /// Entries on the undo stack.
    pub undo_depth: usize,
    /// Entries on the redo stack.
    pub redo_depth: usize,
    /// Changed since it was last saved or opened, or never saved.
    pub dirty: bool,
    /// Changes whenever the geometry can have changed; the UI refetches the mesh on a change.
    pub geometry_rev: u64,
    /// Surface groups whose effective material is not the import placeholder
    /// (`scene::is_placeholder`).
    pub groups_assigned: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, JsonSchema)]
pub struct VariantInfo {
    pub id: String,
    pub name: String,
}

/// One history entry, mirrored beside `History`'s own stacks: a serial for the dirty flag, and
/// whether the op holds a `set_geometry`, for the geometry revision.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Mark {
    serial: u64,
    geometry: bool,
}

/// The check of one geometry revision.
struct CheckCache {
    rev: u64,
    summary: CheckSummary,
}

/// The open project and its undo history, with the M10 additions (module docs).
#[derive(Default)]
pub struct Session {
    project: Option<Project>,
    path: Option<PathBuf>,
    history: History,
    /// Mirror `history`'s undo and redo stacks, entry for entry.
    undo_marks: Vec<Mark>,
    redo_marks: Vec<Mark>,
    /// The serial of the state the project was opened, imported or created in.
    base_serial: u64,
    /// The serial of the state last saved or opened.
    saved_serial: u64,
    next_serial: u64,
    geometry_rev: u64,
    /// What the check's Console lines call the model: the imported file or the project file.
    model_name: String,
    check: Option<CheckCache>,
    /// The validator's issues on the current state, with their UI codes.
    issues: Vec<UiIssue>,
    /// Console lines not yet returned in a [`SceneState`].
    lines: Vec<LogLine>,
}

pub fn load_error(e: &LoadError) -> CmdError {
    CmdError::new(
        format!("LOAD_{}", e.code().to_ascii_uppercase()),
        e.to_string(),
    )
}

fn op_error(e: &OpError) -> CmdError {
    CmdError::new(
        format!("OP_{}", e.code().to_ascii_uppercase()),
        e.to_string(),
    )
}

fn import_error(e: &import::ImportError) -> CmdError {
    CmdError::new(
        format!("IMPORT_{}", e.code().to_ascii_uppercase()),
        e.to_string(),
    )
}

/// A `.proj` import's refusal: its own `proj_*` codes upper-cased (`PROJ_VOLUMES_UNSUPPORTED`),
/// any other import error as `IMPORT_*`.
fn proj_error(e: &import::ImportError) -> CmdError {
    let code = e.code();
    if code.starts_with("proj_") {
        CmdError::new(code.to_ascii_uppercase(), e.to_string())
    } else {
        import_error(e)
    }
}

fn no_project() -> CmdError {
    CmdError::new("NO_PROJECT", "no project is open")
}

/// Whether an op holds a `set_geometry`, directly or inside a batch.
fn touches_geometry(op: &Op) -> bool {
    match op {
        Op::SetGeometry { .. } => true,
        Op::Batch { ops } => ops.iter().any(touches_geometry),
        _ => false,
    }
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string())
}

impl Session {
    fn serial(&mut self) -> u64 {
        self.next_serial += 1;
        self.next_serial
    }

    fn top_serial(&self) -> u64 {
        self.undo_marks
            .last()
            .map_or(self.base_serial, |m| m.serial)
    }

    fn context(&self) -> Context {
        self.path
            .as_deref()
            .map(Context::for_project_file)
            .unwrap_or_default()
    }

    /// Re-checks the geometry when its revision moved (a new check of a non-empty geometry
    /// pushes its Console lines) and re-validates the project.
    fn refresh(&mut self) {
        let Some(p) = self.project.as_ref() else {
            self.check = None;
            self.issues.clear();
            return;
        };
        if p.geometry.faces.is_empty() {
            self.check = None;
        } else if self
            .check
            .as_ref()
            .is_none_or(|c| c.rev != self.geometry_rev)
        {
            let report = check::check(&p.geometry);
            self.lines.extend(scene::check_lines(
                &self.model_name,
                &report,
                p.surface_groups.len(),
            ));
            self.check = Some(CheckCache {
                rev: self.geometry_rev,
                summary: scene::check_summary(p, &report),
            });
        }
        let ctx = self.context();
        self.issues = validate::validate_with(p, &ctx)
            .iter()
            .map(|i| scene::ui_issue(p, i))
            .collect();
    }

    fn replace(
        &mut self,
        project: Project,
        path: Option<PathBuf>,
        model_name: String,
    ) -> ProjectInfo {
        self.project = Some(project);
        self.path = path;
        self.history.clear();
        self.undo_marks.clear();
        self.redo_marks.clear();
        self.base_serial = self.serial();
        self.saved_serial = self.base_serial;
        self.geometry_rev += 1;
        self.model_name = model_name;
        self.check = None;
        self.refresh();
        self.info().expect("a project was just set")
    }

    pub fn new_project(&mut self, name: &str) -> ProjectInfo {
        self.replace(Project::new(name), None, name.to_string())
    }

    /// Reads a whole project from `.simpa` JSON text (version, schema and integrity checked).
    pub fn load_text(&mut self, text: &str) -> CmdResult<ProjectInfo> {
        let project = schema::from_json(text).map_err(|e| load_error(&e))?;
        let name = project.name.clone();
        Ok(self.replace(project, None, name))
    }

    pub fn open(&mut self, path: &Path) -> CmdResult<ProjectInfo> {
        let project = schema::load(path).map_err(|e| load_error(&e))?;
        self.lines.push(LogLine::new(
            LineClass::Ok,
            format!("Opened {}", path.display()),
        ));
        Ok(self.replace(project, Some(path.to_path_buf()), file_name(path)))
    }

    pub fn info(&self) -> Option<ProjectInfo> {
        let p = self.project.as_ref()?;
        Some(ProjectInfo {
            id: p.id.to_string(),
            name: p.name.clone(),
            path: self.path.as_ref().map(|p| p.display().to_string()),
            vertices: p.geometry.vertices.len(),
            faces: p.geometry.faces.len(),
            surface_groups: p.surface_groups.len(),
            materials: p.materials.len(),
            sources: p.sources.len(),
            point_receivers: p.point_receivers.len(),
            surface_receivers: p.surface_receivers.len(),
            variants: p
                .variants
                .iter()
                .map(|v| VariantInfo {
                    id: v.id.to_string(),
                    name: v.name.clone(),
                })
                .collect(),
            active_variant: p.active_variant.map(|v| v.to_string()),
            can_undo: self.history.can_undo(),
            can_redo: self.history.can_redo(),
            undo_depth: self.history.undo_len(),
            redo_depth: self.history.redo_len(),
            dirty: self.path.is_none() || self.top_serial() != self.saved_serial,
            geometry_rev: self.geometry_rev,
            groups_assigned: scene::groups_assigned(p),
        })
    }

    /// The project file this session was opened from or last saved to.
    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    /// Why the project may not run, as `SceneState::run_blockers` has it, without taking the
    /// pending Console lines. `None` with no project open.
    pub fn project_blockers(&self) -> Option<Vec<String>> {
        let p = self.project.as_ref()?;
        let check = self.check.as_ref().map(|c| &c.summary);
        Some(scene::run_blockers(p, check, &self.issues))
    }

    /// The project in its canonical file form (`schema::to_json`).
    pub fn json(&self) -> CmdResult<String> {
        self.project
            .as_ref()
            .map(schema::to_json)
            .ok_or_else(no_project)
    }

    /// Applies `op` through the history and keeps the marks in step. On error nothing changes.
    fn commit(&mut self, op: Op) -> CmdResult<()> {
        let geometry = touches_geometry(&op);
        let project = self.project.as_mut().ok_or_else(no_project)?;
        self.history.apply(project, op).map_err(|e| op_error(&e))?;
        let serial = self.serial();
        self.undo_marks.push(Mark { serial, geometry });
        self.redo_marks.clear();
        if geometry {
            self.geometry_rev += 1;
        }
        Ok(())
    }

    /// Undoes (`undo`) or redoes one entry and moves its mark. `Ok(false)` when there is
    /// nothing to move. An inverse that fails is returned as an error, never swallowed.
    fn step(&mut self, undo: bool) -> CmdResult<bool> {
        let project = self.project.as_mut().ok_or_else(no_project)?;
        let moved = if undo {
            self.history.undo(project)
        } else {
            self.history.redo(project)
        }
        .map_err(|e| op_error(&e))?;
        if !moved {
            return Ok(false);
        }
        let (from, to) = if undo {
            (&mut self.undo_marks, &mut self.redo_marks)
        } else {
            (&mut self.redo_marks, &mut self.undo_marks)
        };
        let Some(mark) = from.pop() else {
            return Err(CmdError::new(
                "HISTORY_MARKS",
                "the history marks are out of step with the history",
            ));
        };
        to.push(mark);
        if mark.geometry {
            self.geometry_rev += 1;
        }
        Ok(true)
    }

    fn info_or_error(&self) -> CmdResult<ProjectInfo> {
        self.info().ok_or_else(no_project)
    }

    /// Applies an op sent as JSON text, unchecked (M9's `project_apply`, used by the self-test).
    /// On any error the project is unchanged.
    pub fn apply(&mut self, op_text: &str) -> CmdResult<ProjectInfo> {
        let op = Op::from_json(op_text).map_err(|e| load_error(&e))?;
        self.commit(op)?;
        self.refresh();
        self.info_or_error()
    }

    pub fn undo(&mut self) -> CmdResult<ProjectInfo> {
        if self.step(true)? {
            self.refresh();
        }
        self.info_or_error()
    }

    pub fn redo(&mut self) -> CmdResult<ProjectInfo> {
        if self.step(false)? {
            self.refresh();
        }
        self.info_or_error()
    }

    // ---- M10 (PLAN.md 1.2) --------------------------------------------------------------------

    /// The current state, taking the pending Console lines. `None` with no project open; the
    /// lines then wait for the next state.
    pub fn scene_state(&mut self) -> Option<SceneState> {
        let info = self.info()?;
        let p = self.project.as_ref()?;
        let check = self.check.as_ref().map(|c| c.summary.clone());
        Some(SceneState {
            info,
            view: scene::view(p),
            groups: scene::group_stats(p),
            run_blockers: scene::run_blockers(p, check.as_ref(), &self.issues),
            check,
            issues: self.issues.clone(),
            lines: std::mem::take(&mut self.lines),
        })
    }

    fn state(&mut self) -> CmdResult<SceneState> {
        self.scene_state().ok_or_else(no_project)
    }

    pub fn scene_new(&mut self, name: &str) -> CmdResult<SceneState> {
        self.new_project(name);
        self.lines.push(LogLine::new(
            LineClass::Info,
            format!("New project \"{name}\""),
        ));
        self.state()
    }

    pub fn scene_open(&mut self, path: &Path) -> CmdResult<SceneState> {
        self.open(path)?;
        self.state()
    }

    /// Imports a mesh file as a new project named after the file. A geometry the check refuses
    /// is loaded, not rejected: its faces are highlighted and Run is blocked.
    pub fn model_import(&mut self, path: &Path, unit: &str, up: &str) -> CmdResult<SceneState> {
        // An upstream project carries its own units: opened as a `.proj`, whatever was chosen.
        if path
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("proj"))
        {
            return self.proj_import(path);
        }
        let u = Unit::from_symbol(unit).ok_or_else(|| {
            CmdError::new(
                "IMPORT_UNIT",
                format!("unknown unit '{unit}': one of m, cm, mm, ft, in"),
            )
        })?;
        let a = match up {
            "z" => Up::Z,
            "y" => Up::Y,
            _ => {
                return Err(CmdError::new(
                    "IMPORT_UP",
                    format!("unknown up axis '{up}': y or z"),
                ));
            }
        };
        let model =
            import::import_file(path, &ImportOptions::new(u, a)).map_err(|e| import_error(&e))?;
        let stem = path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "Imported model".to_string());
        let file = file_name(path);
        let report = scene::import_lines(&file, unit, up, &model.report, model.group_names.len());
        self.replace(model.to_project(&stem), None, file);
        self.lines.extend(report);
        self.state()
    }

    /// Opens an upstream I-Simpa `.proj` as a new, unsaved project (`import_proj_file`: the same
    /// file always gives the same project, byte for byte, so Save as gives what `simpa
    /// import-proj` writes). The project is named after the file in place of upstream's default
    /// name, by the CLI's own rule (`name_after_file`); the check's lines
    /// call the model by the file's name. The import's notes become INFO lines. A refusal is an
    /// error with its code, and the session is left as it was.
    pub fn proj_import(&mut self, path: &Path) -> CmdResult<SceneState> {
        let mut imported = import::import_proj_file(path).map_err(|e| proj_error(&e))?;
        // The CLI's rule: upstream's default name gives way to the file's (M11 m11-r22-a3 found
        // the app kept "New project" while `simpa import-proj` wrote "tutorial_1").
        import::name_after_file(&mut imported.project, path);
        let file = file_name(path);
        let r = &imported.report;
        let p = &imported.project;
        let mut lines = vec![LogLine::new(
            LineClass::Info,
            format!(
                "Imported {file}: {} faces, {} surface groups, {} materials, {} sources, {} point \
                 receivers, {} surface receivers",
                r.faces,
                p.surface_groups.len(),
                p.materials.len(),
                p.sources.len(),
                p.point_receivers.len(),
                p.surface_receivers.len()
            ),
        )];
        lines.extend(
            r.notes
                .iter()
                .map(|n| LogLine::new(LineClass::Info, format!("Import: {n}"))),
        );
        self.replace(imported.project, None, file);
        self.lines.extend(lines);
        self.state()
    }

    /// Saves to `path` (Save As; the session then points there) or to the session's own path.
    /// Never changes the project's name.
    pub fn save(&mut self, path: Option<&Path>) -> CmdResult<SceneState> {
        let project = self.project.as_ref().ok_or_else(no_project)?;
        let target = match path {
            Some(p) => p.to_path_buf(),
            None => self.path.clone().ok_or_else(|| {
                CmdError::new(
                    "SAVE_NO_PATH",
                    "this project has never been saved: use Save as",
                )
            })?,
        };
        schema::save(project, &target).map_err(|e| {
            CmdError::new(
                "SAVE_IO",
                format!("could not save {}: {e}", target.display()),
            )
        })?;
        let moved = self.path.as_deref() != Some(target.as_path());
        self.path = Some(target.clone());
        self.saved_serial = self.top_serial();
        self.lines.push(LogLine::new(
            LineClass::Ok,
            format!("Saved {}", target.display()),
        ));
        if moved {
            // Relative directivity files now resolve against the new folder.
            self.refresh();
        }
        self.state()
    }

    /// The checked apply (PLAN.md 1.8). The op is tried on a copy and validated. If that
    /// introduces an error the project does not already have, the edit is refused and the
    /// project and its history are unchanged; otherwise it goes through the history.
    pub fn edit_apply(&mut self, op_text: &str) -> CmdResult<EditOutcome> {
        let op = Op::from_json(op_text).map_err(|e| load_error(&e))?;
        let project = self.project.as_ref().ok_or_else(no_project)?;
        let mut candidate = project.clone();
        op.clone().apply(&mut candidate).map_err(|e| op_error(&e))?;
        let ctx = self.context();
        let after: Vec<UiIssue> = validate::validate_with(&candidate, &ctx)
            .iter()
            .map(|i| scene::ui_issue(&candidate, i))
            .collect();
        let before: HashSet<IssueKey> = self.issues.iter().map(scene::issue_key).collect();
        let (refusals, warnings): (Vec<&UiIssue>, Vec<&UiIssue>) = after
            .iter()
            .filter(|i| !before.contains(&scene::issue_key(i)))
            .partition(|i| i.severity == IssueSeverity::Error);
        if !refusals.is_empty() {
            let refusals: Vec<UiIssue> = refusals.into_iter().cloned().collect();
            for r in &refusals {
                self.lines.push(LogLine::new(
                    LineClass::Fail,
                    format!(
                        "Edit refused, project unchanged: {} ({}) at {}: {}",
                        r.code, r.rule, r.path, r.message
                    ),
                ));
            }
            return Ok(EditOutcome {
                applied: false,
                refusals,
                state: self.state()?,
            });
        }
        let warnings: Vec<LogLine> = warnings
            .into_iter()
            .map(|w| {
                LogLine::new(
                    LineClass::Warn,
                    format!("{} ({}) at {}: {}", w.code, w.rule, w.path, w.message),
                )
            })
            .collect();
        let rev = self.geometry_rev;
        self.commit(op)?;
        if self.geometry_rev != rev {
            // A new geometry: check it (and push its lines) as well.
            self.refresh();
        } else {
            self.issues = after;
        }
        self.lines.extend(warnings);
        Ok(EditOutcome {
            applied: true,
            refusals: Vec::new(),
            state: self.state()?,
        })
    }

    pub fn edit_undo(&mut self) -> CmdResult<SceneState> {
        if self.step(true)? {
            self.refresh();
        }
        self.state()
    }

    pub fn edit_redo(&mut self) -> CmdResult<SceneState> {
        if self.step(false)? {
            self.refresh();
        }
        self.state()
    }

    /// The geometry as the `scene_mesh` buffer (PLAN.md 1.9).
    pub fn mesh(&self) -> CmdResult<Vec<u8>> {
        let p = self.project.as_ref().ok_or_else(no_project)?;
        Ok(scene::mesh_bytes(p, self.geometry_rev))
    }
}

/// What each reader made of a JSON array of numbers: the bits as `0x` + 16 hex digits.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, JsonSchema)]
pub struct FloatProbe {
    /// Read with the core's exact reader, as every schema value from the UI is.
    pub exact_bits: Vec<String>,
    /// Read with `serde_json::from_str`, as a command argument typed `f64` would be.
    pub serde_json_bits: Vec<String>,
    /// How many values the two readers disagree on.
    pub disagreements: usize,
}

fn bits(v: f64) -> String {
    format!("0x{:016x}", v.to_bits())
}

/// Reads `text` (a JSON array of numbers) with both readers, for the self-test.
pub fn exact_float_probe(text: &str) -> CmdResult<FloatProbe> {
    let exact: Vec<F64> = schema::from_json_exact(text).map_err(|e| load_error(&e))?;
    let lossy: Vec<f64> =
        serde_json::from_str(text).map_err(|e| CmdError::new("PROBE_SERDE_JSON", e.to_string()))?;
    let exact: Vec<f64> = exact.into_iter().map(F64::get).collect();
    let disagreements = exact
        .iter()
        .zip(&lossy)
        .filter(|(a, b)| a.to_bits() != b.to_bits())
        .count();
    Ok(FloatProbe {
        exact_bits: exact.into_iter().map(bits).collect(),
        serde_json_bits: lossy.into_iter().map(bits).collect(),
        disagreements,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Shortest decimals that `serde_json::from_str` reads back wrong in this build (found by a
    /// seeded search over bit patterns); the UI's self-test sends the same three.
    pub const MISREAD: [(&str, u64); 3] = [
        ("1.5990461000457081", 0x3ff9_95b1_5d07_e1f0),
        ("484.53035250855277", 0x407e_487c_52e9_795f),
        ("980.0819440890737", 0x408e_a0a7_d24d_7560),
    ];

    #[test]
    fn the_exact_reader_keeps_the_bits_serde_json_loses() {
        let text = format!("[{}]", MISREAD.map(|(t, _)| t).join(", "));
        let probe = exact_float_probe(&text).unwrap();
        let want: Vec<String> = MISREAD
            .iter()
            .map(|&(_, b)| format!("0x{b:016x}"))
            .collect();
        assert_eq!(probe.exact_bits, want);
        assert_eq!(probe.disagreements, 3, "{probe:?}");
        for (t, b) in MISREAD {
            assert_eq!(serde_json::to_string(&f64::from_bits(b)).unwrap(), t);
        }
    }

    #[test]
    fn ops_arrive_as_text_and_keep_their_bits() {
        let mut s = Session::default();
        assert_eq!(s.apply("{}").unwrap_err().code, "LOAD_SCHEMA");
        s.new_project("Probe");
        let [(a, _), (b, _), (c, _)] = MISREAD;
        let op = format!(
            r#"{{"op": "set_camera", "camera": {{"eye": [{a}, {b}, {c}], "target": [0, 0, 0], "vertical_fov_deg": 45}}}}"#
        );
        let info = s.apply(&op).unwrap();
        assert!(info.can_undo && !info.can_redo);
        let back = schema::from_json(&s.json().unwrap()).unwrap();
        let eye = back.view.camera.expect("camera set").eye.to_array();
        let want = MISREAD.map(|(_, bits)| f64::from_bits(bits));
        assert_eq!(eye.map(f64::to_bits), want.map(f64::to_bits));
        // What a command argument typed `Op` would have received instead.
        let lossy: Op = serde_json::from_str(&op).unwrap();
        assert_ne!(Some(lossy), Op::from_json(&op).ok());
    }

    #[test]
    fn undo_redo_and_refusals_leave_the_project_consistent() {
        let mut s = Session::default();
        assert_eq!(s.undo().unwrap_err().code, "NO_PROJECT");
        assert_eq!(s.json().unwrap_err().code, "NO_PROJECT");
        s.new_project("A");
        let before = s.json().unwrap();
        s.apply(r#"{"op": "set_project_name", "name": "B"}"#)
            .unwrap();
        assert_eq!(s.info().unwrap().name, "B");
        let info = s.undo().unwrap();
        assert_eq!((info.name.as_str(), info.can_redo), ("A", true));
        assert_eq!(s.json().unwrap(), before);
        assert_eq!(s.redo().unwrap().name, "B");
        let refused = s
            .apply(r#"{"op": "remove_material", "id": "00000000-0000-0000-0000-000000000001"}"#)
            .unwrap_err();
        assert_eq!(refused.code, "OP_NOT_FOUND");
        assert_eq!(s.info().unwrap().name, "B");
    }

    #[test]
    fn whole_projects_load_from_text_with_the_core_s_checks() {
        let mut s = Session::default();
        let text = schema::to_json(&Project::new("Loaded"));
        let info = s.load_text(&text).unwrap();
        assert_eq!(
            (info.name.as_str(), info.faces, info.can_undo),
            ("Loaded", 0, false)
        );
        assert_eq!(s.load_text("{").unwrap_err().code, "LOAD_SYNTAX");
        assert_eq!(
            s.load_text(r#"{"format_version": 999}"#).unwrap_err().code,
            "LOAD_VERSION"
        );
        assert_eq!(
            s.open(Path::new("no/such/file.simpa")).unwrap_err().code,
            "LOAD_NOT_FOUND"
        );
    }
}

/// The M10 session: the checked apply, dirty serials, the geometry revision, import and save.
#[cfg(test)]
mod m10_tests {
    use super::*;
    use crate::scene::{CHECK_OK_PREFIX, GEOMETRY_REFUSED, MATERIALS_UNASSIGNED};
    use simpa_core::schema::{EntityRef, MaterialQuantity, PointReceiverId, Vec3};

    fn repo(rel: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join(rel)
    }

    fn opened(rel: &str) -> Session {
        let mut s = Session::default();
        s.scene_open(&repo(rel)).unwrap();
        s
    }

    fn project(s: &Session) -> &Project {
        s.project.as_ref().unwrap()
    }

    /// A folder on the system temp drive (never the exFAT repo drive), removed by the caller.
    fn scratch(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("simpa-app-m10-{tag}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn move_receiver(id: PointReceiverId, p: [f64; 3]) -> String {
        Op::MovePointReceiver {
            id,
            position: Vec3::new(p[0], p[1], p[2]),
        }
        .to_json()
    }

    #[test]
    fn a_receiver_outside_is_refused_and_nothing_changes() {
        let mut s = opened("tests/fixtures/rooms/tutorial1_box.simpa");
        let _ = s.scene_state();
        let r1 = project(&s).point_receivers[0].id;
        let before = s.json().unwrap();
        let out = s.edit_apply(&move_receiver(r1, [20.0, 1.0, 1.8])).unwrap();
        assert!(!out.applied);
        assert_eq!(out.refusals.len(), 1, "{:?}", out.refusals);
        let r = &out.refusals[0];
        assert_eq!(
            (r.code.as_str(), r.rule.as_str(), r.field.as_str()),
            ("RECEIVER_OUTSIDE", "receiver_outside_volume", "position")
        );
        assert_eq!(r.entity, Some(EntityRef::PointReceiver(r1)));
        assert_eq!(s.json().unwrap(), before);
        assert_eq!(
            (out.state.info.undo_depth, out.state.info.redo_depth),
            (0, 0)
        );
        assert!(!out.state.info.dirty);
        assert!(
            out.state
                .lines
                .iter()
                .any(|l| l.class == LineClass::Fail && l.text.contains("RECEIVER_OUTSIDE")),
            "{:?}",
            out.state.lines
        );

        // The positive control: inside the room, accepted, one undo step.
        let ok = s.edit_apply(&move_receiver(r1, [4.0, 2.0, 1.8])).unwrap();
        assert!(ok.applied && ok.refusals.is_empty());
        assert_eq!(ok.state.info.undo_depth, 1);
        assert!(ok.state.info.dirty);
    }

    #[test]
    fn an_unsafe_label_is_refused_and_a_safe_one_accepted() {
        let mut s = opened("tests/fixtures/rooms/tutorial1_box.simpa");
        let r1 = project(&s).point_receivers[0].id;
        let rename = |name: &str| {
            Op::Rename {
                target: EntityRef::PointReceiver(r1),
                name: name.to_string(),
            }
            .to_json()
        };
        let before = s.json().unwrap();
        let out = s.edit_apply(&rename("a/b")).unwrap();
        assert!(!out.applied);
        assert_eq!(out.refusals[0].code, "LABEL_UNSAFE");
        assert_eq!(out.refusals[0].rule, "name_not_filename_safe");
        assert_eq!(out.refusals[0].field, "name");
        assert_eq!(s.json().unwrap(), before);
        let out = s.edit_apply(&rename("Front row")).unwrap();
        assert!(out.applied);
        assert_eq!(project(&s).point_receivers[0].name, "Front row");
    }

    /// An insertion shifts the index of an existing error. Its identity is its entity, so it is
    /// not taken for a new error and the insertion is accepted.
    #[test]
    fn an_existing_error_does_not_block_an_edit_that_shifts_its_index() {
        let mut s = opened("tests/fixtures/rooms/tutorial1_box.simpa");
        let r1 = project(&s).point_receivers[0].id;
        // Put R1 outside with the unchecked M9 apply: a pre-existing error.
        s.apply(&move_receiver(r1, [20.0, 1.0, 1.8])).unwrap();
        assert!(
            s.issues
                .iter()
                .any(|i| i.path == "/point_receivers/0/position")
        );
        let mut added = project(&s).point_receivers[1].clone();
        added.id = PointReceiverId::from_u128(0xabc);
        added.name = "R new".to_string();
        added.position = Vec3::new(2.0, 2.0, 1.2);
        added.solver_id = None;
        let op = Op::AddPointReceiver {
            index: 0,
            receiver: added,
        }
        .to_json();
        let out = s.edit_apply(&op).unwrap();
        assert!(out.applied, "{:?}", out.refusals);
        let shifted: Vec<&UiIssue> = out
            .state
            .issues
            .iter()
            .filter(|i| i.rule == "receiver_outside_volume")
            .collect();
        assert_eq!(shifted.len(), 1);
        assert_eq!(shifted[0].path, "/point_receivers/1/position");
        assert_eq!(shifted[0].entity, Some(EntityRef::PointReceiver(r1)));
        // An edit that fixes the error is accepted too.
        let fixed = s.edit_apply(&move_receiver(r1, [4.0, 2.0, 1.8])).unwrap();
        assert!(fixed.applied);
        assert!(
            !fixed
                .state
                .issues
                .iter()
                .any(|i| i.rule == "receiver_outside_volume")
        );
    }

    #[test]
    fn dirty_follows_edits_undo_redo_and_save() {
        let dir = scratch("dirty");
        let mut s = opened("tests/fixtures/rooms/tutorial1_box.simpa");
        let dirty = |s: &Session| s.info().unwrap().dirty;
        assert!(!dirty(&s));
        let r1 = project(&s).point_receivers[0].id;
        assert!(
            s.edit_apply(&move_receiver(r1, [4.0, 2.0, 1.8]))
                .unwrap()
                .applied
        );
        assert!(dirty(&s));
        s.edit_undo().unwrap();
        assert!(!dirty(&s), "back at the opened state");
        s.edit_redo().unwrap();
        assert!(dirty(&s));
        let saved = dir.join("box.simpa");
        let st = s.save(Some(&saved)).unwrap();
        assert!(!st.info.dirty);
        assert_eq!(st.info.name, project(&s).name, "Save As never renames");
        assert!(
            st.lines
                .iter()
                .any(|l| l.class == LineClass::Ok && l.text.starts_with("Saved "))
        );
        assert_eq!(std::fs::read_to_string(&saved).unwrap(), s.json().unwrap());
        s.edit_undo().unwrap();
        assert!(dirty(&s), "undone past the save");
        s.edit_redo().unwrap();
        assert!(!dirty(&s), "redone back to the saved state");
        // Plain Save to the session's path.
        s.edit_undo().unwrap();
        assert!(!s.save(None).unwrap().info.dirty);

        let mut n = Session::default();
        let st = n.scene_new("Untitled").unwrap();
        assert!(st.info.dirty, "never saved");
        assert_eq!(n.save(None).unwrap_err().code, "SAVE_NO_PATH");
        assert_eq!(
            Session::default().save(None).unwrap_err().code,
            "NO_PROJECT"
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn edits_then_undos_give_back_the_same_bytes() {
        let mut s = opened("tests/fixtures/rooms/tutorial1_box.simpa");
        let start = s.json().unwrap();
        let p = project(&s).clone();
        let m0 = p.materials[0].id;
        let src = p.sources[0].id;
        let r2 = p.point_receivers[1].id;
        let ops = [
            Op::SetMaterialBand {
                material: m0,
                quantity: MaterialQuantity::Absorption,
                band: 3,
                value: F64::new(0.4),
            },
            Op::MoveSource {
                id: src,
                position: Vec3::new(2.0, 4.0, 1.5),
            },
            Op::MovePointReceiver {
                id: r2,
                position: Vec3::new(-0.0 + 3.0, 6.0, 1.2),
            },
            Op::Rename {
                target: EntityRef::Source(src),
                name: "S1".to_string(),
            },
            Op::SetMaterialBand {
                material: m0,
                quantity: MaterialQuantity::Scattering,
                band: 0,
                value: F64::new(-0.0),
            },
        ];
        for op in &ops {
            let out = s.edit_apply(&op.to_json()).unwrap();
            assert!(out.applied, "{op:?}: {:?}", out.refusals);
        }
        assert_eq!(s.info().unwrap().undo_depth, ops.len());
        for _ in &ops {
            s.edit_undo().unwrap();
        }
        assert_eq!(s.json().unwrap(), start);
        let info = s.info().unwrap();
        assert_eq!(
            (info.undo_depth, info.redo_depth, info.dirty),
            (0, ops.len(), false)
        );
        // Nothing left to undo is not an error.
        assert_eq!(s.edit_undo().unwrap().info.undo_depth, 0);
    }

    #[test]
    fn the_geometry_revision_moves_with_set_geometry_only() {
        let mut s = opened("tests/fixtures/rooms/tutorial1_box.simpa");
        let _ = s.scene_state();
        let rev0 = s.info().unwrap().geometry_rev;
        let r1 = project(&s).point_receivers[0].id;
        s.edit_apply(&move_receiver(r1, [4.0, 2.0, 1.8])).unwrap();
        assert_eq!(s.info().unwrap().geometry_rev, rev0);

        let mut geometry = project(&s).geometry.clone();
        geometry.faces.remove(5);
        let op = Op::Batch {
            ops: vec![Op::SetGeometry { geometry }],
        };
        // The unchecked apply (the checked one may refuse the receivers of an open box); the
        // check then refuses the geometry.
        s.apply(&op.to_json()).unwrap();
        let st = s.scene_state().unwrap();
        assert_eq!(st.info.geometry_rev, rev0 + 1);
        let check = st.check.as_ref().unwrap();
        assert_eq!(check.verdict, scene::CheckVerdict::Refused);
        assert!(!check.highlight_faces.is_empty());
        assert!(st.run_blockers.contains(&GEOMETRY_REFUSED.to_string()));
        assert!(
            st.lines
                .iter()
                .any(|l| l.class == LineClass::Fail && l.text.contains("open_boundary")),
            "{:?}",
            st.lines
        );
        let st = s.edit_undo().unwrap();
        assert_eq!(st.info.geometry_rev, rev0 + 2);
        assert_eq!(st.check.unwrap().verdict, scene::CheckVerdict::Ok);
        assert!(st.lines.iter().any(|l| l.text.starts_with(CHECK_OK_PREFIX)));
        let st = s.edit_redo().unwrap();
        assert_eq!(st.info.geometry_rev, rev0 + 3);
    }

    #[test]
    fn the_corrected_hall_imports_closed_with_no_material_chosen() {
        let mut s = Session::default();
        let st = s
            .model_import(&repo("testdata/elmia_corrected.ply"), "m", "z")
            .unwrap();
        let first = &st.lines[0];
        assert_eq!(first.class, LineClass::Info);
        assert_eq!(
            first.text,
            "Closed volume, 0 self-intersections · 7860 faces, 10 surface groups"
        );
        assert!(
            st.lines[1]
                .text
                .starts_with("Imported elmia_corrected.ply: 7860 triangles")
        );
        assert_eq!((st.info.groups_assigned, st.info.surface_groups), (0, 10));
        assert!(st.info.dirty && st.info.path.is_none());
        assert_eq!(st.info.name, "elmia_corrected");
        assert!(!st.run_blockers.contains(&GEOMETRY_REFUSED.to_string()));
        assert!(st.run_blockers.contains(&MATERIALS_UNASSIGNED.to_string()));
        assert!(st.run_blockers.contains(&"SOURCE_NONE".to_string()));
        // The core's placeholder rule fires on all 10 groups, and its blocker is listed once.
        assert_eq!(
            st.issues
                .iter()
                .filter(|i| i.rule == "material_placeholder")
                .count(),
            10
        );
        assert_eq!(
            st.run_blockers
                .iter()
                .filter(|b| *b == MATERIALS_UNASSIGNED)
                .count(),
            1,
            "{:?}",
            st.run_blockers
        );
        assert!(st.groups.iter().all(|g| !g.assigned && g.faces > 0));
        let check = st.check.unwrap();
        assert_eq!(check.counts.faces, 7860);
        assert!(check.highlight_faces.is_empty());
        assert!(
            s.scene_state().unwrap().lines.is_empty(),
            "lines are taken once"
        );

        // A pre-existing error (no source) does not block an unrelated edit.
        let m = project(&s).materials[0].id;
        let op = Op::SetMaterialBand {
            material: m,
            quantity: MaterialQuantity::Absorption,
            band: 0,
            value: F64::new(0.2),
        };
        let out = s.edit_apply(&op.to_json()).unwrap();
        assert!(out.applied, "{:?}", out.refusals);
        assert_eq!(
            out.state.info.groups_assigned, 10,
            "no longer the placeholder"
        );

        let mesh = s.mesh().unwrap();
        assert_eq!(mesh.len(), 24 + 24 * 3926 + 16 * 7860);
    }

    #[test]
    fn import_options_and_files_are_refused_with_codes() {
        let mut s = Session::default();
        let ply = repo("testdata/elmia_corrected.ply");
        assert_eq!(
            s.model_import(&ply, "furlong", "z").unwrap_err().code,
            "IMPORT_UNIT"
        );
        assert_eq!(
            s.model_import(&ply, "m", "x").unwrap_err().code,
            "IMPORT_UP"
        );
        assert_eq!(
            s.model_import(&repo("no/such/model.ply"), "m", "z")
                .unwrap_err()
                .code,
            "IMPORT_IO"
        );
        assert_eq!(
            s.model_import(&repo("tests/fixtures/rooms/tutorial1_box.simpa"), "m", "z")
                .unwrap_err()
                .code,
            "IMPORT_UNKNOWN_FORMAT"
        );
        assert!(s.scene_state().is_none(), "nothing was opened");
        assert_eq!(s.edit_apply("{}").unwrap_err().code, "LOAD_SCHEMA");
        assert_eq!(
            s.edit_apply(r#"{"op": "set_project_name", "name": "x"}"#)
                .unwrap_err()
                .code,
            "NO_PROJECT"
        );
        assert_eq!(s.mesh().unwrap_err().code, "NO_PROJECT");
        assert_eq!(
            s.scene_open(&repo("no/such/file.simpa")).unwrap_err().code,
            "LOAD_NOT_FOUND"
        );
    }

    #[test]
    fn an_op_error_leaves_the_project_and_history_unchanged() {
        let mut s = opened("tests/fixtures/rooms/tutorial1_box.simpa");
        let before = s.json().unwrap();
        let err = s
            .edit_apply(
                &Op::RemoveMaterial {
                    id: project(&s).materials[0].id,
                }
                .to_json(),
            )
            .unwrap_err();
        assert_eq!(err.code, "OP_IN_USE");
        assert_eq!(s.json().unwrap(), before);
        assert_eq!(s.info().unwrap().undo_depth, 0);
    }
}
