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
use simpa_core::schema::{
    self, BandKind, BandSet, EntityRef, F64, GroupId, History, LoadError, MaterialId, Op, OpError,
    Project, SolverKind,
};
use simpa_core::validate::{self, Context};

use crate::events::LineClass;
use crate::guard::{CmdError, CmdResult};
use crate::scene::{
    self, CheckSummary, EditOutcome, IssueKey, IssueSeverity, LogLine, RepairReport, SceneState,
    SolverIssues, UiIssue,
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
    /// Imported in this session from a mesh file that declares no groups (C1): every face is in
    /// one surface group named after the file, for the user to carve.
    pub imported_ungrouped: bool,
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
    /// The open project was imported, just now, from a mesh file that declares no groups
    /// (`ImportReport::ungrouped`, C1): the Geometry step says so. Cleared by any other load.
    imported_ungrouped: bool,
    /// The mesh file the open project was imported from this session (parity G8: Repair writes
    /// its repaired copy beside it). Cleared by any other load.
    import_source: Option<PathBuf>,
    check: Option<CheckCache>,
    /// The validator's issues on the current state, with their UI codes.
    issues: Vec<UiIssue>,
    /// The issues of the rules about one solver's run, per solver.
    solver_issues: SolverIssues,
    /// The run-quality advisor on the current state ([`SceneState::advice`]).
    advice: Vec<simpa_core::advise::Advice>,
    /// [`SceneState::advice_conflicts`] on the current state.
    advice_conflicts: Vec<simpa_core::advise::ApplyConflict>,
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

/// Whether an op changes the mesh buffer (`scene::mesh_bytes`): a `set_geometry`, or an op that
/// moves faces between groups (each face's group index): `regroup_faces`, `move_faces`,
/// `merge_surface_groups`; directly or inside a batch.
fn touches_geometry(op: &Op) -> bool {
    match op {
        Op::SetGeometry { .. }
        | Op::RegroupFaces { .. }
        | Op::MoveFaces { .. }
        | Op::MergeSurfaceGroups { .. } => true,
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
            self.solver_issues = SolverIssues::default();
            self.advice.clear();
            self.advice_conflicts.clear();
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
        self.solver_issues = SolverIssues::of(p);
        self.readvise();
    }

    /// The advisor on the current project, with the cached check's air volume (backlog 80).
    fn readvise(&mut self) {
        self.advice = match self.project.as_ref() {
            Some(p) => simpa_core::advise::before_with(
                p,
                self.check.as_ref().and_then(|c| c.summary.air_volume_m3),
            ),
            None => Vec::new(),
        };
        self.advice_conflicts = self
            .project
            .as_ref()
            .map(simpa_core::advise::apply_conflicts)
            .unwrap_or_default();
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
        self.imported_ungrouped = false;
        self.import_source = None;
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
            imported_ungrouped: self.imported_ungrouped,
        })
    }

    /// The open project, as edited.
    pub fn project(&self) -> Option<&Project> {
        self.project.as_ref()
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

    /// What blocks a run of `solver` beside [`Session::project_blockers`]: its own errors
    /// (`no_band_computed`), as UI codes. Empty with no project open.
    pub fn solver_blockers(&self, solver: SolverKind) -> Vec<String> {
        self.solver_issues.blockers(solver)
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
            solver_issues: self.solver_issues.clone(),
            advice: self.advice.clone(),
            advice_conflicts: self.advice_conflicts.clone(),
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
        self.imported_ungrouped = model.report.ungrouped;
        self.import_source = Some(path.to_path_buf());
        self.lines.extend(report);
        self.state()
    }

    /// Parity G8, upstream's "Repair model" at import (`loadingSceneDialog.cpp:128-131`), here the
    /// core's safe fixes (`repair::repair`: welding within 1 um, degenerate and duplicate faces
    /// removed, inverted faces flipped; holes and intersections are never "fixed"). When it changes
    /// anything, the repaired mesh is written as a **new** OBJ beside the file the model came from
    /// (`<stem>_repaired.obj`, then `_repaired-2.obj`, ...; never over an existing file, the
    /// original least of all), then put in the project through the checked apply as one undo step,
    /// which checks the model again. With nothing to change, nothing is written or edited.
    pub fn model_repair(&mut self) -> CmdResult<RepairReport> {
        use simpa_core::geometry::export;
        use simpa_core::geometry::repair::{self, RepairOptions};
        let project = self.project.as_ref().ok_or_else(no_project)?;
        if project.geometry.faces.is_empty() {
            return Err(CmdError::new(
                "REPAIR_NO_MODEL",
                "the project has no model to repair",
            ));
        }
        let outcome =
            repair::repair(&project.geometry, &RepairOptions::default()).map_err(|e| {
                CmdError::new(
                    format!("REPAIR_{}", e.code().to_ascii_uppercase()),
                    e.to_string(),
                )
            })?;
        let count = |kind: &str| outcome.changes.iter().filter(|c| c.kind() == kind).count() as u32;
        let (welded, degenerate, duplicate, flipped) = (
            count("weld_vertex"),
            count("remove_degenerate_face"),
            count("remove_duplicate_face"),
            count("flip_face"),
        );
        let base = self.import_source.clone().or_else(|| self.path.clone());
        let original = base.as_deref().map(|p| p.display().to_string());
        let what = format!(
            "{}, {}, {}, {}",
            plural_n(welded, "vertex welded", "vertices welded"),
            plural_n(
                degenerate,
                "degenerate face removed",
                "degenerate faces removed"
            ),
            plural_n(
                duplicate,
                "duplicate face removed",
                "duplicate faces removed"
            ),
            plural_n(flipped, "face flipped", "faces flipped")
        );
        let still = |o: &repair::RepairOutcome| {
            o.refusals
                .iter()
                .map(|r| r.message.clone())
                .collect::<Vec<_>>()
                .join("; ")
        };
        if outcome.changes.is_empty() {
            let mut text = "Repair: nothing to change (no vertex to weld, no degenerate or \
                            duplicate face, no face to flip); no file written"
                .to_string();
            if !outcome.is_ok() {
                text.push_str(&format!(
                    ". The model check still refuses it, and repair does not fix this: {}",
                    still(&outcome)
                ));
            }
            self.lines.push(LogLine::new(LineClass::Info, text));
            let state = self.state()?;
            return Ok(RepairReport {
                changed: false,
                file: None,
                original,
                welded_vertices: 0,
                degenerate_faces: 0,
                duplicate_faces: 0,
                flipped_faces: 0,
                oriented: outcome.oriented,
                passes: outcome.is_ok(),
                outcome: EditOutcome {
                    applied: false,
                    refusals: Vec::new(),
                    state,
                },
            });
        }
        let base = base.ok_or_else(|| {
            CmdError::new(
                "REPAIR_NO_FOLDER",
                "this project has no file yet: save it first, and Repair writes the repaired \
                 model beside it",
            )
        })?;
        let mut repaired = project.clone();
        repaired.geometry = outcome.geometry.clone();
        let header = [format!(
            "{} repaired by I-Simpa Night Mode: {what}",
            file_name(&base)
        )];
        let text = export::obj_text(&repaired, &header);
        let target = write_new_beside(&base, "_repaired", "obj", text.as_bytes())?;
        self.lines.push(LogLine::new(
            LineClass::Ok,
            format!(
                "Repair: {what}; wrote {} (metres, Z up); {} is unchanged",
                target.display(),
                file_name(&base)
            ),
        ));
        if !outcome.oriented {
            self.lines.push(LogLine::new(
                LineClass::Warn,
                "Repair: faces intersect, so which way each face should face was not decided and \
                 none was flipped"
                    .to_string(),
            ));
        }
        if !outcome.is_ok() {
            self.lines.push(LogLine::new(
                LineClass::Warn,
                format!(
                    "Repair: the model check still refuses the repaired model, and repair does not \
                     fix this: {}",
                    still(&outcome)
                ),
            ));
        }
        let op = Op::SetGeometry {
            geometry: outcome.geometry.clone(),
        };
        let edit = self.edit_apply(&op.to_json())?;
        Ok(RepairReport {
            changed: true,
            file: Some(target.display().to_string()),
            original,
            welded_vertices: welded,
            degenerate_faces: degenerate,
            duplicate_faces: duplicate,
            flipped_faces: flipped,
            oriented: outcome.oriented,
            passes: outcome.is_ok(),
            outcome: edit,
        })
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
        // A surface group the edit creates may start with upstream's placeholder (G19's new group
        // from faces of different materials): that is "no material chosen yet", a run blocker
        // the Scene panel shows, not a value the edit broke. Every other new error refuses.
        let created_group = |i: &UiIssue| {
            i.rule == validate::codes::MATERIAL_PLACEHOLDER
                && matches!(i.entity, Some(EntityRef::SurfaceGroup(g)) if project.group(g).is_none())
        };
        let (refusals, warnings): (Vec<&UiIssue>, Vec<&UiIssue>) = after
            .iter()
            .filter(|i| !before.contains(&scene::issue_key(i)) && !created_group(i))
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
            if let Some(p) = self.project.as_ref() {
                self.solver_issues = SolverIssues::of(p);
            }
            self.readvise();
        }
        self.lines.extend(warnings);
        Ok(EditOutcome {
            applied: true,
            refusals: Vec::new(),
            state: self.state()?,
        })
    }

    /// "Apply" on an advice item (backlog 80): `setting` from `from` to `to`, one
    /// `Op::SetSolverSettings` through the checked apply ([`Session::edit_apply`]): one undo step,
    /// refused by the validator like any edit. Refused, `ADVICE_PROJECT_CHANGED`, when the
    /// project's value is no longer `from`, the value the advice was given for (the project
    /// changed since the run).
    pub fn advice_apply(
        &mut self,
        setting: simpa_core::advise::Setting,
        from: simpa_core::advise::SettingValue,
        to: simpa_core::advise::SettingValue,
    ) -> CmdResult<EditOutcome> {
        let project = self.project.as_ref().ok_or_else(no_project)?;
        let op = simpa_core::advise::apply_op(project, setting, from, to)
            .map_err(|e| CmdError::new(e.code(), e.to_string()))?;
        self.edit_apply(&op.to_json())
    }

    /// [`Session::advice_apply`] from its arguments as JSON text
    /// (`simpa_core::advise::ApplyArgs`), read with the core's exact reader.
    pub fn advice_apply_text(&mut self, args: &str) -> CmdResult<EditOutcome> {
        let a: simpa_core::advise::ApplyArgs =
            schema::from_json_exact(args).map_err(|e| load_error(&e))?;
        self.advice_apply(a.setting, a.from, a.to)
    }

    /// A band preset (PQ3, C26): the project moved onto every band of `kind` (`octave` or
    /// `third_octave`) from `lowest_hz` to `highest_hz`, by [`Project::rebanded`] (each new band
    /// takes every per-band value of the nearest current band), through the checked apply as one
    /// undoable edit. The UI cannot build the per-band data itself.
    pub fn edit_reband(
        &mut self,
        kind: &str,
        lowest_hz: u32,
        highest_hz: u32,
    ) -> CmdResult<EditOutcome> {
        let project = self.project.as_ref().ok_or_else(no_project)?;
        let kind = match kind {
            "octave" => BandKind::Octave,
            "third_octave" => BandKind::ThirdOctave,
            other => {
                return Err(CmdError::new(
                    "REBAND_KIND",
                    format!("unknown band kind '{other}': octave or third_octave"),
                ));
            }
        };
        let range_error = || {
            CmdError::new(
                "REBAND_RANGE",
                format!(
                    "{lowest_hz} Hz to {highest_hz} Hz is not a range of nominal {kind:?}                      frequencies, lowest first"
                ),
            )
        };
        let bands = BandSet::range(kind, lowest_hz, highest_hz).ok_or_else(range_error)?;
        let op = project.rebanded(bands).ok_or_else(range_error)?;
        self.edit_apply(&op.to_json())
    }

    /// "New group from selection" (scope row 15 (1), G19): `faces` sent to a new surface group
    /// named `Group <n>` (the first free n; upstream names it `Group`), with the material
    /// [`Project::regrouped`] picks, through the checked apply as one undoable edit. A refusal
    /// of the op itself (a selection straddling a receiver, a bad face list) is an error with
    /// that op's own code, not the batch's.
    pub fn edit_regroup(&mut self, faces: &[u32]) -> CmdResult<EditOutcome> {
        let project = self.project.as_ref().ok_or_else(no_project)?;
        let name = free_group_name(project);
        let op = project.regrouped(faces, GroupId::random(), &name, MaterialId::random());
        if let Err(mut e) = op.clone().apply(&mut project.clone()) {
            while let OpError::Batch { error, .. } = e {
                e = *error;
            }
            return Err(op_error(&e));
        }
        self.edit_apply(&op.to_json())
    }

    /// "Add surface group" (parity G18): an empty group named `Group <n>` (the first free n, as
    /// [`Session::edit_regroup`] names one), with upstream's placeholder material
    /// ([`Project::new_group`]), through the checked apply as one undoable edit. Faces join it
    /// with Move to group; the run is blocked until it has a material.
    pub fn edit_add_group(&mut self) -> CmdResult<EditOutcome> {
        let project = self.project.as_ref().ok_or_else(no_project)?;
        let name = free_group_name(project);
        let op = project.new_group(GroupId::random(), &name, MaterialId::random());
        self.edit_apply(&op.to_json())
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

/// `1 vertex welded`, `3 vertices welded`.
fn plural_n(n: u32, one: &str, many: &str) -> String {
    if n == 1 {
        format!("1 {one}")
    } else {
        format!("{n} {many}")
    }
}

/// Writes `bytes` to a new file beside `base`: `<stem><suffix>.<ext>`, or `<stem><suffix>-2.<ext>`
/// and on when that exists. Opened with `create_new`, so no existing file, `base` included, is
/// ever written over.
fn write_new_beside(base: &Path, suffix: &str, ext: &str, bytes: &[u8]) -> CmdResult<PathBuf> {
    use std::io::Write;
    let dir = base.parent().unwrap_or_else(|| Path::new("."));
    let stem = base
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "model".to_string());
    for n in 1..1000 {
        let name = if n == 1 {
            format!("{stem}{suffix}.{ext}")
        } else {
            format!("{stem}{suffix}-{n}.{ext}")
        };
        let target = dir.join(name);
        if target == base {
            continue;
        }
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&target)
        {
            Ok(mut f) => {
                f.write_all(bytes).and_then(|_| f.sync_all()).map_err(|e| {
                    CmdError::new(
                        "REPAIR_WRITE",
                        format!("could not write {}: {e}", target.display()),
                    )
                })?;
                return Ok(target);
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => {
                return Err(CmdError::new(
                    "REPAIR_WRITE",
                    format!("could not create {}: {e}", target.display()),
                ));
            }
        }
    }
    Err(CmdError::new(
        "REPAIR_WRITE",
        format!(
            "no free name beside {} for the repaired model",
            base.display()
        ),
    ))
}

/// `Group <n>` with the first n no surface group's name takes, compared as the core compares
/// group names (`validate::group_name_key`).
fn free_group_name(project: &Project) -> String {
    (1u32..)
        .map(|n| format!("Group {n}"))
        .find(|n| {
            let key = validate::group_name_key(n);
            !project
                .surface_groups
                .iter()
                .any(|g| validate::group_name_key(&g.name) == key)
        })
        .expect("some n is free")
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

    /// Wow list W1 (parity M41): the ear-height plane as the UI sends it (`ops.ts`
    /// `addSurfaceReceiver`, the text `planes.test.ts` pins), through the checked apply; the
    /// core's `cutting_plane_invalid` refuses a cell larger than a side and collinear corners,
    /// and nothing changes.
    #[test]
    fn an_ear_height_plane_goes_through_the_checked_apply() {
        let mut s = opened("tests/fixtures/rooms/outputs_box.simpa");
        let _ = s.scene_state();
        let id = "0c0be000-0000-4000-8000-0000000000a1";
        let plane = |a: &str, b: &str, c: &str, res: &str| {
            format!(
                r#"{{"id":"{id}","name":"Plane 1","enabled":true,"shape":{{"kind":"cutting_plane","a":{a},"b":{b},"c":{c},"resolution_m":{res}}},"solver_id":null}}"#
            )
        };
        let n = project(&s).surface_receivers.len();
        let add = format!(
            r#"{{"op":"add_surface_receiver","index":{n},"receiver":{}}}"#,
            plane("[0,10,1.6]", "[0,0,1.6]", "[6,0,1.6]", "1")
        );
        let ok = s.edit_apply(&add).unwrap();
        assert!(ok.applied, "{:?}", ok.refusals);
        assert_eq!(ok.state.info.undo_depth, 1);
        let added = project(&s).surface_receivers.last().unwrap().clone();
        assert_eq!(added.name, "Plane 1");
        let sid = added.id;
        let before = s.json().unwrap();

        for (shape, why) in [
            (
                plane("[0,10,1.6]", "[0,0,1.6]", "[6,0,1.6]", "50"),
                "larger than a side",
            ),
            (
                plane("[0,10,1.6]", "[0,0,1.6]", "[0,5,1.6]", "1"),
                "collinear",
            ),
        ] {
            let op = format!(r#"{{"op":"replace_surface_receiver","receiver":{shape}}}"#);
            let out = s.edit_apply(&op).unwrap();
            assert!(!out.applied, "{why}: accepted");
            assert_eq!(out.refusals.len(), 1, "{:?}", out.refusals);
            let r = &out.refusals[0];
            assert_eq!(r.rule, "cutting_plane_invalid");
            assert_eq!(r.field, "shape");
            assert_eq!(r.entity, Some(EntityRef::SurfaceReceiver(sid)));
            assert!(r.message.contains(why), "{}", r.message);
            assert_eq!(s.json().unwrap(), before, "{why}: the project changed");
            assert_eq!(out.state.info.undo_depth, 1);
        }

        // The positive control: the same plane at 1.2 m, accepted, a second undo step.
        let op = format!(
            r#"{{"op":"replace_surface_receiver","receiver":{}}}"#,
            plane("[0,10,1.2]", "[0,0,1.2]", "[6,0,1.2]", "0.5")
        );
        let moved = s.edit_apply(&op).unwrap();
        assert!(moved.applied, "{:?}", moved.refusals);
        assert_eq!(moved.state.info.undo_depth, 2);
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

    /// The teaching room's box as an OBJ, with face 4 turned inward and face 0 repeated: the
    /// check refuses it (an inverted face, a duplicate), and the core's repair can fix both.
    fn damaged_box_obj() -> String {
        let v = [
            [0, 0, 0],
            [10, 0, 0],
            [10, 6, 0],
            [0, 6, 0],
            [0, 0, 3],
            [10, 0, 3],
            [10, 6, 3],
            [0, 6, 3],
        ];
        let mut f = vec![
            [0, 2, 1],
            [0, 3, 2],
            [4, 5, 6],
            [4, 6, 7],
            [3, 7, 6],
            [3, 6, 2],
            [0, 1, 5],
            [0, 5, 4],
            [0, 4, 7],
            [0, 7, 3],
            [1, 2, 6],
            [1, 6, 5],
        ];
        f[4].swap(1, 2);
        f.push(f[0]);
        let mut out = String::new();
        for p in v {
            out.push_str(&format!("v {} {} {}\n", p[0], p[1], p[2]));
        }
        out.push_str("g Room\n");
        for t in f {
            out.push_str(&format!("f {} {} {}\n", t[0] + 1, t[1] + 1, t[2] + 1));
        }
        out
    }

    /// Parity G8: Repair writes a new file beside the imported one (never over it, nor over an
    /// earlier repair), puts the repaired geometry in the project as one undo step and checks it
    /// again; with nothing left to change it writes nothing.
    #[test]
    fn repair_writes_a_new_file_beside_the_original_and_checks_again() {
        let dir = scratch("repair");
        let original = dir.join("box.obj");
        let text = damaged_box_obj();
        std::fs::write(&original, &text).unwrap();
        // A file of the first name already there is not written over.
        std::fs::write(dir.join("box_repaired.obj"), "keep me").unwrap();

        let mut s = Session::default();
        let st = s.model_import(&original, "m", "z").unwrap();
        assert_eq!(
            st.check.as_ref().unwrap().verdict,
            scene::CheckVerdict::Refused
        );
        assert!(st.run_blockers.contains(&GEOMETRY_REFUSED.to_string()));
        let rev = st.info.geometry_rev;

        let r = s.model_repair().unwrap();
        assert!(r.changed && r.passes && r.oriented, "{r:?}");
        assert_eq!((r.duplicate_faces, r.flipped_faces), (1, 1), "{r:?}");
        let written = PathBuf::from(r.file.as_deref().unwrap());
        assert_eq!(written, dir.join("box_repaired-2.obj"));
        assert_eq!(
            std::fs::read_to_string(&original).unwrap(),
            text,
            "the original is untouched"
        );
        assert_eq!(
            std::fs::read_to_string(dir.join("box_repaired.obj")).unwrap(),
            "keep me"
        );
        assert!(r.outcome.applied, "{:?}", r.outcome.refusals);
        let st = &r.outcome.state;
        assert_eq!(st.info.geometry_rev, rev + 1);
        assert_eq!(st.check.as_ref().unwrap().verdict, scene::CheckVerdict::Ok);
        assert!(!st.run_blockers.contains(&GEOMETRY_REFUSED.to_string()));
        assert!(
            st.lines.iter().any(|l| l.text.starts_with(CHECK_OK_PREFIX)),
            "checked again: {:?}",
            st.lines
        );
        assert_eq!(project(&s).geometry.faces.len(), 12);

        // The written file imports as the model now in the project.
        let mut again = Session::default();
        let st2 = again.model_import(&written, "m", "z").unwrap();
        assert_eq!(st2.check.unwrap().verdict, scene::CheckVerdict::Ok);
        assert_eq!(
            again
                .project
                .as_ref()
                .unwrap()
                .geometry
                .faces
                .iter()
                .map(|f| f.vertices)
                .collect::<Vec<_>>(),
            project(&s)
                .geometry
                .faces
                .iter()
                .map(|f| f.vertices)
                .collect::<Vec<_>>()
        );

        // Nothing left to change: no file, no edit.
        let before = std::fs::read_dir(&dir).unwrap().count();
        let r2 = s.model_repair().unwrap();
        assert!(!r2.changed && r2.file.is_none() && !r2.outcome.applied);
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), before);

        // Undo puts the imported (refused) geometry back.
        let st = s.edit_undo().unwrap();
        assert_eq!(st.check.unwrap().verdict, scene::CheckVerdict::Refused);
        std::fs::remove_dir_all(&dir).unwrap();
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

/// PQ3, the Simulate settings editor (docs/investigations/2026-10-03-pq3/PLAN.md): the refusals
/// it shows inline, the per-solver blocker, and the band presets as one undoable edit.
#[cfg(test)]
mod pq3_tests {
    use super::*;
    use simpa_core::schema::{BandKind, SolverKind};

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

    const BOX: &str = "tests/fixtures/rooms/tutorial1_box.simpa";

    /// Turning every band off is an edit like any other (the project may hold it); what it
    /// blocks is a run of that solver, and only that solver.
    #[test]
    fn every_band_off_is_applied_and_blocks_only_that_solver() {
        let mut s = opened(BOX);
        let n = project(&s).bands.len();
        let mut last = None;
        for band in 0..n {
            let op = Op::SetBandComputed {
                solver: SolverKind::Spps,
                band,
                computed: false,
            };
            let out = s.edit_apply(&op.to_json()).unwrap();
            assert!(out.applied, "band {band}: {:?}", out.refusals);
            last = Some(out.state);
        }
        let st = last.unwrap();
        let spps: Vec<&str> = st
            .solver_issues
            .spps
            .iter()
            .map(|i| i.code.as_str())
            .collect();
        assert_eq!(spps, ["NO_BAND_COMPUTED"]);
        assert_eq!(st.solver_issues.spps[0].rule, "no_band_computed");
        assert_eq!(
            st.solver_issues.spps[0].path,
            "/solvers/spps/bands_computed"
        );
        assert!(st.solver_issues.tcr.is_empty());
        assert!(!st.run_blockers.iter().any(|b| b == "NO_BAND_COMPUTED"));
        assert_eq!(s.solver_blockers(SolverKind::Spps), ["NO_BAND_COMPUTED"]);
        assert!(s.solver_blockers(SolverKind::Tcr).is_empty());
        // One band back on clears it; so does undo.
        let out = s
            .edit_apply(
                &Op::SetBandComputed {
                    solver: SolverKind::Spps,
                    band: 2,
                    computed: true,
                }
                .to_json(),
            )
            .unwrap();
        assert!(out.state.solver_issues.spps.is_empty());
        s.edit_undo().unwrap();
        assert_eq!(s.solver_blockers(SolverKind::Spps), ["NO_BAND_COMPUTED"]);
        s.edit_undo().unwrap();
        assert!(s.solver_blockers(SolverKind::Spps).is_empty());
    }

    fn set_air(s: &mut Session, t: f64, h: f64, pa: f64) -> EditOutcome {
        let mut env = project(s).environment.clone();
        env.temperature_c = F64::new(t);
        env.relative_humidity_percent = F64::new(h);
        env.pressure_pa = F64::new(pa);
        s.edit_apply(&Op::SetEnvironment { environment: env }.to_json())
            .unwrap()
    }

    #[test]
    fn non_physical_air_is_refused_inline_and_outside_the_formula_is_a_warning() {
        let mut s = opened(BOX);
        let before = s.json().unwrap();
        for (t, h, pa, field) in [
            (20.0, 120.0, 101_325.0, "relative_humidity_percent"),
            (20.0, 50.0, 0.0, "pressure_pa"),
            (-274.0, 50.0, 101_325.0, "temperature_c"),
        ] {
            let out = set_air(&mut s, t, h, pa);
            assert!(!out.applied);
            assert_eq!(out.refusals.len(), 1, "{:?}", out.refusals);
            assert_eq!(out.refusals[0].code, "ATMOSPHERE_INVALID");
            assert_eq!(out.refusals[0].path, format!("/environment/{field}"));
            assert_eq!(s.json().unwrap(), before, "refused: unchanged");
        }
        let out = set_air(&mut s, 60.0, 50.0, 101_325.0);
        assert!(
            out.applied,
            "a warning is not a refusal: {:?}",
            out.refusals
        );
        let warned: Vec<&UiIssue> = out
            .state
            .issues
            .iter()
            .filter(|i| i.code == "ATMOSPHERE_OUTSIDE_FORMULA_RANGE")
            .collect();
        assert_eq!(warned.len(), 1);
        assert_eq!(warned[0].severity, IssueSeverity::Warning);
        assert!(
            out.state.run_blockers.is_empty(),
            "{:?}",
            out.state.run_blockers
        );
    }

    /// A preset is one edit: `Project::rebanded` in the core, applied through the checked apply,
    /// and one undo gives back the project bit for bit.
    #[test]
    fn a_band_preset_is_one_edit_and_one_undo_restores_it_bit_for_bit() {
        let mut s = opened(BOX);
        let start = s.json().unwrap();
        let original = project(&s).clone();
        let out = s.edit_reband("third_octave", 50, 20_000).unwrap();
        assert!(out.applied, "{:?}", out.refusals);
        let p = project(&s);
        assert_eq!(p.bands.kind, BandKind::ThirdOctave);
        assert_eq!(p.bands.len(), 27);
        assert_eq!(p.solvers.spps.bands_computed.len(), 27);
        assert_eq!(p.materials[0].absorption.len(), 27);
        assert_eq!(out.state.info.undo_depth, 1);
        assert!(out.state.info.dirty);
        let st = s.edit_undo().unwrap();
        assert_eq!(st.info.undo_depth, 0);
        assert_eq!(project(&s), &original);
        assert_eq!(s.json().unwrap(), start);
        // Redo is the same preset again.
        s.edit_redo().unwrap();
        assert_eq!(project(&s).bands.len(), 27);
        // Every preset the editor offers applies on this project.
        for (kind, lo, hi, n) in [
            ("octave", 63, 16_000, 9),
            ("octave", 125, 4_000, 6),
            ("third_octave", 50, 20_000, 27),
            ("third_octave", 100, 5_000, 18),
            ("octave", 125, 8_000, 7),
        ] {
            let out = s.edit_reband(kind, lo, hi).unwrap();
            assert!(out.applied, "{kind} {lo}-{hi}: {:?}", out.refusals);
            assert_eq!(project(&s).bands.len(), n, "{kind} {lo}-{hi}");
        }
        while s.info().unwrap().can_undo {
            s.edit_undo().unwrap();
        }
        assert_eq!(s.json().unwrap(), start);
    }

    #[test]
    fn a_band_range_that_is_not_one_is_refused_and_nothing_changes() {
        let mut s = opened(BOX);
        let before = s.json().unwrap();
        assert_eq!(
            s.edit_reband("fifth", 125, 4000).unwrap_err().code,
            "REBAND_KIND"
        );
        assert_eq!(
            s.edit_reband("octave", 100, 4000).unwrap_err().code,
            "REBAND_RANGE"
        );
        assert_eq!(
            s.edit_reband("octave", 4000, 125).unwrap_err().code,
            "REBAND_RANGE"
        );
        assert_eq!(s.json().unwrap(), before);
        assert_eq!(s.info().unwrap().undo_depth, 0);
        assert_eq!(
            Session::default()
                .edit_reband("octave", 125, 4000)
                .unwrap_err()
                .code,
            "NO_PROJECT"
        );
    }
}

/// Scope row 15 (1), G19: "New group from selection" (`edit_regroup`) through the checked apply.
#[cfg(test)]
mod row15_tests {
    use super::*;
    use crate::scene::MATERIALS_UNASSIGNED;

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

    /// The teaching room: Ceiling faces 10, 11; Floor 0, 1 (the scene receiver's group); Walls 2
    /// to 9.
    const BOX: &str = "tests/fixtures/rooms/tutorial1_box.simpa";

    /// Two wall faces: one edit, the new group `Group 1` with Walls' material, the mesh buffer's
    /// revision moved (its per-face group changed), and one undo gives the project back byte
    /// for byte; redo gives the edited one.
    #[test]
    fn regroup_is_one_checked_edit_and_one_undo_restores_the_project() {
        let mut s = opened(BOX);
        let before = s.json().unwrap();
        let rev = s.info().unwrap().geometry_rev;
        let walls_m = project(&s).surface_groups[2].material;
        let out = s.edit_regroup(&[2, 3]).unwrap();
        assert!(out.applied, "{:?}", out.refusals);
        let g = out.state.view.surface_groups.last().unwrap().clone();
        assert_eq!(g.name, "Group 1");
        assert_eq!(g.material, walls_m);
        assert_eq!(out.state.view.surface_groups.len(), 4);
        assert!(out.state.info.geometry_rev > rev);
        assert!(out.state.info.dirty);
        assert_eq!(out.state.info.undo_depth, 1);
        let after = s.json().unwrap();
        // The mesh buffer's group index of faces 2 and 3 is the new group's, 3.
        let mesh = s.mesh().unwrap();
        let nv = project(&s).geometry.vertices.len();
        let at = 24 + 24 * nv + 12 * 12;
        let gi = |face: usize| {
            u32::from_le_bytes(mesh[at + 4 * face..at + 4 * face + 4].try_into().unwrap())
        };
        assert_eq!((gi(2), gi(3), gi(4)), (3, 3, 2));

        s.edit_undo().unwrap();
        assert_eq!(s.json().unwrap(), before);
        s.edit_redo().unwrap();
        assert_eq!(s.json().unwrap(), after);

        // The next one is `Group 2`.
        let out = s.edit_regroup(&[4, 5]).unwrap();
        assert!(out.applied);
        assert_eq!(
            out.state.view.surface_groups.last().unwrap().name,
            "Group 2"
        );
    }

    /// Faces of two materials: the new group is the placeholder. The checked apply accepts it
    /// (a group the edit creates may start with no material chosen); the run is blocked until a
    /// material is set, and two undos give back the original.
    #[test]
    fn a_new_group_with_the_placeholder_is_accepted_and_blocks_the_run() {
        let mut s = opened(BOX);
        let before = s.json().unwrap();
        assert!(
            !s.state()
                .unwrap()
                .run_blockers
                .iter()
                .any(|b| b == MATERIALS_UNASSIGNED)
        );
        let out = s.edit_regroup(&[2, 10]).unwrap();
        assert!(out.applied, "{:?}", out.refusals);
        let st = out.state;
        let g = st.view.surface_groups.last().unwrap().clone();
        assert!(
            st.issues
                .iter()
                .any(|i| i.rule == "material_placeholder" && i.path == "/surface_groups/3/material")
        );
        assert!(st.run_blockers.iter().any(|b| b == MATERIALS_UNASSIGNED));
        assert!(!st.groups.iter().find(|x| x.id == g.id).unwrap().assigned);

        let ceiling_m = project(&s).surface_groups[0].material;
        let out = s
            .edit_apply(
                &Op::SetGroupMaterial {
                    group: g.id,
                    material: ceiling_m,
                }
                .to_json(),
            )
            .unwrap();
        assert!(out.applied, "{:?}", out.refusals);
        assert!(
            !out.state
                .run_blockers
                .iter()
                .any(|b| b == MATERIALS_UNASSIGNED)
        );

        s.edit_undo().unwrap();
        s.edit_undo().unwrap();
        assert_eq!(s.json().unwrap(), before);

        // Setting an existing group to the placeholder is still refused: only a group the
        // edit itself creates may start unassigned.
        s.edit_regroup(&[2, 10]).unwrap();
        let placeholder = project(&s).surface_groups[3].material;
        let floor = project(&s).surface_groups[1].id;
        let out = s
            .edit_apply(
                &Op::SetGroupMaterial {
                    group: floor,
                    material: placeholder,
                }
                .to_json(),
            )
            .unwrap();
        assert!(!out.applied);
        assert_eq!(out.refusals[0].rule, "material_placeholder");
    }

    /// G18: Add surface group is one edit, an empty `Group 1` with the placeholder (added with
    /// it, the box having none), which blocks the run; one undo gives the project back. The
    /// empty group deletes; a group that holds faces does not, and nothing changes.
    #[test]
    fn an_empty_group_is_added_with_the_placeholder_and_deletes() {
        let mut s = opened(BOX);
        let before = s.json().unwrap();
        let materials = project(&s).materials.len();
        let out = s.edit_add_group().unwrap();
        assert!(out.applied, "{:?}", out.refusals);
        let st = out.state;
        let g = st.view.surface_groups.last().unwrap().clone();
        assert_eq!(
            (g.name.as_str(), st.view.surface_groups.len()),
            ("Group 1", 4)
        );
        assert_eq!(st.groups.iter().find(|x| x.id == g.id).unwrap().faces, 0);
        assert!(!st.groups.iter().find(|x| x.id == g.id).unwrap().assigned);
        assert_eq!(st.view.materials.len(), materials + 1);
        assert!(st.run_blockers.iter().any(|b| b == MATERIALS_UNASSIGNED));
        assert_eq!(st.info.undo_depth, 1);
        s.edit_undo().unwrap();
        assert_eq!(s.json().unwrap(), before);

        // Two in a row: the second is `Group 2` and takes the placeholder the first added.
        s.edit_add_group().unwrap();
        let out = s.edit_add_group().unwrap();
        let groups = &out.state.view.surface_groups;
        assert_eq!(groups.last().unwrap().name, "Group 2");
        assert_eq!(groups[3].material, groups[4].material);
        assert_eq!(out.state.view.materials.len(), materials + 1);

        // The empty one deletes, one undo step.
        let id = groups[4].id;
        let out = s
            .edit_apply(&Op::RemoveSurfaceGroup { id }.to_json())
            .unwrap();
        assert!(out.applied, "{:?}", out.refusals);
        assert_eq!(out.state.view.surface_groups.len(), 4);

        // Walls holds faces: refused by the core, the project unchanged.
        let walls = project(&s).surface_groups[2].id;
        let now = s.json().unwrap();
        let err = s
            .edit_apply(&Op::RemoveSurfaceGroup { id: walls }.to_json())
            .unwrap_err();
        assert_eq!(err.code, "OP_IN_USE", "{}", err.message);
        assert_eq!(s.json().unwrap(), now);
    }

    /// A selection with faces in and out of the scene receiver is refused by its own reason,
    /// not the batch's, and nothing changes, the history included.
    #[test]
    fn a_selection_straddling_a_receiver_is_refused_by_its_reason() {
        let mut s = opened(BOX);
        let before = s.json().unwrap();
        let err = s.edit_regroup(&[0, 2]).unwrap_err();
        assert_eq!(err.code, "OP_SPLIT", "{}", err.message);
        assert!(
            err.message.contains("surface receiver 'Receiver'"),
            "{}",
            err.message
        );
        assert_eq!(s.json().unwrap(), before);
        assert_eq!(s.info().unwrap().undo_depth, 0);
        assert_eq!(s.edit_regroup(&[]).unwrap_err().code, "OP_FACES");
        assert_eq!(s.edit_regroup(&[2, 99]).unwrap_err().code, "OP_FACES");
    }
}

/// Backlog 80: the run-quality advisor in the session, and "Apply" on its items.
#[cfg(test)]
mod advisor_tests {
    use super::*;
    use simpa_core::advise::{Setting, SettingValue, code};
    use simpa_core::schema::F64;

    /// Tutorial 2 as upstream ships it: the Elmia fixture without `-Y` and with 0.31 m receivers.
    fn tutorial2() -> Session {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/rooms/elmia_corrected.simpa");
        let mut p = schema::load(&path).unwrap();
        p.solvers.meshing.preserve_boundary = false;
        p.solvers.spps.receiver_radius_m = F64::new(0.31);
        let mut s = Session::default();
        s.load_text(&schema::to_json(&p)).unwrap();
        s
    }

    fn codes_of(s: &mut Session) -> Vec<String> {
        s.scene_state()
            .unwrap()
            .advice
            .iter()
            .map(|a| a.code.clone())
            .collect()
    }

    fn radius(s: &Session) -> f64 {
        s.project().unwrap().solvers.spps.receiver_radius_m.get()
    }

    #[test]
    fn the_scene_carries_the_advice_and_apply_is_one_undo_step() {
        let mut s = tutorial2();
        let codes = codes_of(&mut s);
        assert!(
            codes.contains(&code::MESH_SPLITS_WALLS.to_string()),
            "{codes:?}"
        );
        assert!(
            codes.contains(&code::RECEIVERS_SMALL.to_string()),
            "{codes:?}"
        );
        let depth = s.info().unwrap().undo_depth;
        let out = s
            .advice_apply(
                Setting::ReceiverRadius,
                SettingValue::Number(0.31),
                SettingValue::Number(0.6),
            )
            .unwrap();
        assert!(out.applied, "{:?}", out.refusals);
        assert_eq!(radius(&s), 0.6);
        assert_eq!(s.info().unwrap().undo_depth, depth + 1);
        assert!(
            !out.state
                .advice
                .iter()
                .any(|a| a.code == code::RECEIVERS_SMALL),
            "the advice follows the edit"
        );
        // Undo restores the setting exactly, and the advice with it.
        s.edit_undo().unwrap();
        assert_eq!(radius(&s).to_bits(), 0.31f64.to_bits());
        assert!(codes_of(&mut s).contains(&code::RECEIVERS_SMALL.to_string()));
        // -Y, as the UI sends it: JSON text, read exactly.
        let out = s
            .advice_apply_text(r#"{"setting": "preserve_boundary", "from": false, "to": true}"#)
            .unwrap();
        assert!(out.applied);
        assert!(s.project().unwrap().solvers.meshing.preserve_boundary);
    }

    #[test]
    fn apply_is_refused_when_the_project_changed_since_the_run() {
        let mut s = tutorial2();
        // A run reports the radius as the solver read it, f32: the same setting.
        let run_from = SettingValue::Number(f64::from(0.31f32));
        assert!(
            s.advice_apply(Setting::ReceiverRadius, run_from, SettingValue::Number(0.6))
                .unwrap()
                .applied
        );
        // The project now holds 0.6, not the run's 0.31: refused, nothing changes.
        let depth = s.info().unwrap().undo_depth;
        let e = s
            .advice_apply(Setting::ReceiverRadius, run_from, SettingValue::Number(0.6))
            .unwrap_err();
        assert_eq!(e.code, "ADVICE_PROJECT_CHANGED");
        assert!(
            e.message.contains("the project changed since this run"),
            "{}",
            e.message
        );
        assert_eq!(s.info().unwrap().undo_depth, depth);
        // The checked apply still refuses what the validator refuses: a step count past 16 bits.
        let e = s
            .advice_apply(
                Setting::Duration,
                SettingValue::Number(10.0),
                SettingValue::Number(100.0),
            )
            .unwrap();
        assert!(!e.applied);
        assert_eq!(s.info().unwrap().undo_depth, depth);
    }
}

/// Audit fix (b80): the -Y Apply against the project as it is now.
#[cfg(test)]
mod advisor_conflict_tests {
    use super::*;

    #[test]
    fn y_apply_is_refused_and_not_offered_once_the_project_refines_a_surface_receiver() {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/rooms/tutorial1_box.simpa");
        let mut s = Session::default();
        s.scene_open(&path).unwrap();
        let p = s.project().unwrap();
        assert!(
            !p.solvers.meshing.preserve_boundary
                && p.solvers.meshing.surface_receiver_max_area_m2.is_some()
        );
        let state = s.scene_state().unwrap();
        assert_eq!(
            state.advice_conflicts.len(),
            1,
            "{:?}",
            state.advice_conflicts
        );
        let depth = s.info().unwrap().undo_depth;
        let e = s
            .advice_apply_text(r#"{"setting": "preserve_boundary", "from": false, "to": true}"#)
            .unwrap_err();
        assert_eq!(e.code, "ADVICE_CONFLICT");
        assert!(
            !e.message.contains("mesh_settings_conflict"),
            "{}",
            e.message
        );
        assert_eq!(s.info().unwrap().undo_depth, depth);
    }
}
