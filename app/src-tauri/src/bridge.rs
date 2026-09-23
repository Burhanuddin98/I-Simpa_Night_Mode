//! The typed bridge between the UI and `simpa_core::schema`.
//!
//! Schema values cross the IPC boundary as JSON **text** and are read here with the core's exact
//! reader (`Op::from_json`, `schema::from_json`, `schema::from_json_exact`). No command argument
//! is ever typed as a schema struct: Tauri would deserialize it with `serde_json`, which in this
//! build misreads some floats (the schema module docs; [`exact_float_probe`] shows it live). The
//! TypeScript side of the same types is generated from the core's JSON Schema
//! (`ui/src/bindings/`).

use std::path::{Path, PathBuf};

use schemars::JsonSchema;
use serde::Serialize;
use simpa_core::schema::{self, F64, History, LoadError, Op, OpError, Project};

use crate::guard::{CmdError, CmdResult};

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
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, JsonSchema)]
pub struct VariantInfo {
    pub id: String,
    pub name: String,
}

/// The open project and its undo history.
#[derive(Default)]
pub struct Session {
    project: Option<Project>,
    path: Option<PathBuf>,
    history: History,
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

fn no_project() -> CmdError {
    CmdError::new("NO_PROJECT", "no project is open")
}

impl Session {
    fn replace(&mut self, project: Project, path: Option<PathBuf>) -> ProjectInfo {
        self.project = Some(project);
        self.path = path;
        self.history.clear();
        self.info().expect("a project was just set")
    }

    pub fn new_project(&mut self, name: &str) -> ProjectInfo {
        self.replace(Project::new(name), None)
    }

    /// Reads a whole project from `.simpa` JSON text (version, schema and integrity checked).
    pub fn load_text(&mut self, text: &str) -> CmdResult<ProjectInfo> {
        let project = schema::from_json(text).map_err(|e| load_error(&e))?;
        Ok(self.replace(project, None))
    }

    pub fn open(&mut self, path: &Path) -> CmdResult<ProjectInfo> {
        let project = schema::load(path).map_err(|e| load_error(&e))?;
        Ok(self.replace(project, Some(path.to_path_buf())))
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
        })
    }

    /// The project in its canonical file form (`schema::to_json`).
    pub fn json(&self) -> CmdResult<String> {
        self.project
            .as_ref()
            .map(schema::to_json)
            .ok_or_else(no_project)
    }

    /// Applies an op sent as JSON text. On any error the project is unchanged.
    pub fn apply(&mut self, op_text: &str) -> CmdResult<ProjectInfo> {
        let op = Op::from_json(op_text).map_err(|e| load_error(&e))?;
        let project = self.project.as_mut().ok_or_else(no_project)?;
        self.history.apply(project, op).map_err(|e| op_error(&e))?;
        Ok(self.info().expect("a project is open"))
    }

    pub fn undo(&mut self) -> CmdResult<ProjectInfo> {
        let project = self.project.as_mut().ok_or_else(no_project)?;
        self.history.undo(project).map_err(|e| op_error(&e))?;
        Ok(self.info().expect("a project is open"))
    }

    pub fn redo(&mut self) -> CmdResult<ProjectInfo> {
        let project = self.project.as_mut().ok_or_else(no_project)?;
        self.history.redo(project).map_err(|e| op_error(&e))?;
        Ok(self.info().expect("a project is open"))
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
