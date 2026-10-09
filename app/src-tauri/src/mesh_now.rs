//! Parity G32, mesh on demand: upstream's "Meshing" on a calculation, which runs TetGen on the
//! scene without solving, so a user learns whether the model meshes and how large the mesh is
//! before a run. The core already meshed inside every run (`run::run_project`, `MeshChoice::Build`);
//! this runs the same `mesh::mesh_project_with`, with the same verified `tetgen.exe` and
//! `preprocess.exe` and the project's own mesh settings, into a scratch folder on the temp drive,
//! and reports the outcome. A run still meshes again itself: nothing here is reused by a run, so a
//! run's input is what it was.
//!
//! The project is cloned under the session lock and meshed without it, so the app stays usable
//! while TetGen works; one mesh on demand at a time, and none while a run is active.

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

use schemars::JsonSchema;
use serde::Serialize;
use simpa_core::mesh::{self, MeshManifest, MeshStatus};
use simpa_core::process::CancelToken;
use simpa_core::schema::Project;

use crate::bridge::Session;
use crate::events::LineClass;
use crate::guard::{CmdError, CmdResult, lock};
use crate::runs::{self, RunSlot, SolversCache};
use crate::scene::SceneState;

/// Set while a mesh on demand runs.
static MESHING: AtomicBool = AtomicBool::new(false);

/// What a mesh on demand gave (`mesh_now`).
#[derive(Clone, Debug, Serialize, JsonSchema)]
pub struct MeshNowReport {
    /// The mesh manifest's status: `OK`, `FAIL` or `CANCELLED`.
    pub status: String,
    /// The mesher's reason codes (`simpa_core::mesh::codes`), empty exactly when `OK`.
    pub codes: Vec<String>,
    /// What went wrong, in words, and the mesher's remarks.
    pub messages: Vec<String>,
    /// Tetrahedra and nodes of the mesh built; `None` when TetGen's output was not read.
    pub tetrahedra: Option<usize>,
    pub nodes: Option<usize>,
    /// `preprocess.exe` ran first (the project's scene correction is on).
    pub preprocessed: bool,
    /// Faces of surface receivers TetGen was asked to split (the `.var`); 0 without a face size.
    pub refined_faces: usize,
    /// The mesh input hash of the project as meshed, as a run's `mesh.json` records it.
    pub mesh_input_hash: Option<String>,
    /// The scratch folder, on the temp drive; the next mesh on demand of this project replaces it.
    pub folder: String,
    /// The scene, with the Console lines this added.
    pub state: SceneState,
}

/// The scratch folder for `project`: one per project, under the temp directory.
fn scratch(project: &Project) -> PathBuf {
    std::env::temp_dir()
        .join("simpa-mesh-now")
        .join(project.id.to_string())
}

struct Busy;

impl Busy {
    fn take() -> CmdResult<Busy> {
        if MESHING.swap(true, Ordering::SeqCst) {
            return Err(CmdError::new(
                "MESH_NOW_ACTIVE",
                "a mesh on demand is already running: wait for it to finish",
            ));
        }
        Ok(Busy)
    }
}

impl Drop for Busy {
    fn drop(&mut self) {
        MESHING.store(false, Ordering::SeqCst);
    }
}

/// Meshes the open project now with its own mesh settings (G32) and reports it.
pub fn mesh_now(
    session: &Mutex<Session>,
    slot: &Mutex<RunSlot>,
    cache: &Mutex<SolversCache>,
) -> CmdResult<MeshNowReport> {
    let _busy = Busy::take()?;
    runs::refuse_while_running(slot, "Mesh now")?;
    let project = {
        let s = lock(session, "project")?;
        let p = s
            .project()
            .ok_or_else(|| CmdError::new("NO_PROJECT", "no project is open"))?;
        if p.geometry.faces.is_empty() {
            return Err(CmdError::new(
                "MESH_NOW_NO_MODEL",
                "the project has no model to mesh",
            ));
        }
        p.clone()
    };
    let (tetgen, preprocess) = runs::mesh_exes(cache)?;
    let dir = scratch(&project);
    let manifest = build(&project, &dir, &tetgen, &preprocess)?;
    let report_lines = lines(&manifest, &dir);
    let mut s = lock(session, "project")?;
    for (class, text) in report_lines {
        s.log(class, text);
    }
    let state = s
        .scene_state()
        .ok_or_else(|| CmdError::new("NO_PROJECT", "no project is open"))?;
    let build = manifest.counts.build.as_ref();
    Ok(MeshNowReport {
        status: status_word(manifest.status).to_string(),
        codes: manifest.codes.clone(),
        messages: manifest.messages.clone(),
        tetrahedra: build.map(|b| b.tetrahedra),
        nodes: build.map(|b| b.nodes),
        preprocessed: manifest.preprocess.is_some(),
        refined_faces: manifest.counts.var_constraints,
        mesh_input_hash: manifest.mesh_input_hash.clone(),
        folder: dir.display().to_string(),
        state,
    })
}

fn build(
    project: &Project,
    dir: &Path,
    tetgen: &Path,
    preprocess: &Path,
) -> CmdResult<MeshManifest> {
    let mesher = mesh::TetgenMesher::new(tetgen);
    let program = mesh::PreprocessProgram::new(preprocess);
    let tools = mesh::MeshTools {
        tetgen: &mesher,
        preprocess: Some(&program),
        markers: mesh::Markers::Restored,
        timeouts: mesh::Timeouts::default(),
    };
    mesh::mesh_project_with(project, dir, &tools, &CancelToken::new(), &mut |_| {}).map_err(|e| {
        CmdError::new(
            "MESH_NOW_IO",
            format!("the mesh folder {} could not be used: {e}", dir.display()),
        )
    })
}

fn status_word(s: MeshStatus) -> &'static str {
    match s {
        MeshStatus::Ok => "OK",
        MeshStatus::Fail => "FAIL",
        MeshStatus::Cancelled => "CANCELLED",
    }
}

/// The Console's lines for a mesh on demand: the outcome, then each reason.
fn lines(m: &MeshManifest, dir: &Path) -> Vec<(LineClass, String)> {
    let mut out = Vec::new();
    let pre = if m.preprocess.is_some() {
        ", after preprocess.exe"
    } else {
        ""
    };
    match (m.status, m.counts.build.as_ref()) {
        (MeshStatus::Ok, Some(b)) => out.push((
            LineClass::Ok,
            format!(
                "Mesh now: TetGen meshed the model{pre}: {} tetrahedra, {} nodes ({})",
                b.tetrahedra,
                b.nodes,
                dir.display()
            ),
        )),
        (status, _) => out.push((
            LineClass::Fail,
            format!(
                "Mesh now: {}{pre}: {}",
                status_word(status),
                if m.codes.is_empty() {
                    "no reason recorded".to_string()
                } else {
                    m.codes.join(", ")
                }
            ),
        )),
    }
    for msg in &m.messages {
        out.push((LineClass::Info, format!("Mesh now: {msg}")));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Needs the verified `tetgen.exe` and `preprocess.exe` (`$SIMPA_SOLVERS_DIR`): ignored in the
    /// gates' solver-free `cargo test -p app` (m9, m10, m11), run by hand with
    /// `cargo test -p app -- --ignored mesh_now`.
    #[test]
    #[ignore = "needs the verified tetgen.exe: set SIMPA_SOLVERS_DIR, run with --ignored"]
    fn mesh_now_meshes_the_box_and_changes_nothing_in_the_project() {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/rooms/tutorial1_box.simpa");
        let session = Mutex::new(Session::default());
        session.lock().unwrap().scene_open(&path).unwrap();
        let (json, undo) = {
            let s = session.lock().unwrap();
            (s.json().unwrap(), s.info().unwrap().undo_depth)
        };
        let slot = Mutex::new(RunSlot::default());
        let cache = Mutex::new(SolversCache::default());

        let r = mesh_now(&session, &slot, &cache).unwrap();
        println!(
            "mesh now: {} {:?} tetrahedra, {:?} nodes, {} refined faces, in {}",
            r.status, r.tetrahedra, r.nodes, r.refined_faces, r.folder
        );
        assert_eq!(r.status, "OK", "{:?} {:?}", r.codes, r.messages);
        assert!(r.codes.is_empty());
        assert!(r.tetrahedra.unwrap() > 0 && r.nodes.unwrap() > 0);
        // The box refines its scene receiver to 0.1 m²: the .var was written and obeyed.
        assert!(r.refined_faces > 0);
        assert!(!r.preprocessed, "the box's scene correction is off");
        assert!(
            r.state
                .lines
                .iter()
                .any(|l| l.text.starts_with("Mesh now: TetGen meshed the model")),
            "{:?}",
            r.state.lines
        );
        // Into the temp drive, never beside the project.
        let folder = PathBuf::from(&r.folder);
        assert!(folder.starts_with(std::env::temp_dir()), "{}", r.folder);
        assert!(folder.join(mesh::MANIFEST_FILE).is_file());
        // Nothing in the project moved: no edit, no undo step.
        let s = session.lock().unwrap();
        assert_eq!(s.json().unwrap(), json);
        assert_eq!(s.info().unwrap().undo_depth, undo);
    }

    /// The refusals, before any program is looked for: no project, no model, one at a time.
    #[test]
    fn mesh_now_refuses_with_no_model_and_while_one_runs() {
        let session = Mutex::new(Session::default());
        let slot = Mutex::new(RunSlot::default());
        let cache = Mutex::new(SolversCache::default());
        assert_eq!(
            mesh_now(&session, &slot, &cache).unwrap_err().code,
            "NO_PROJECT"
        );
        session.lock().unwrap().new_project("empty");
        assert_eq!(
            mesh_now(&session, &slot, &cache).unwrap_err().code,
            "MESH_NOW_NO_MODEL"
        );
        let held = Busy::take().unwrap();
        assert_eq!(
            mesh_now(&session, &slot, &cache).unwrap_err().code,
            "MESH_NOW_ACTIVE"
        );
        drop(held);
        assert_eq!(
            mesh_now(&session, &slot, &cache).unwrap_err().code,
            "MESH_NOW_NO_MODEL",
            "freed when the first ends"
        );
    }
}
