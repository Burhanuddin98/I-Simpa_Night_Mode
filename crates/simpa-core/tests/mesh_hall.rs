//! The corrected Elmia hall (`tests/fixtures/rooms/elmia_corrected.simpa`) meshed with the real
//! `tetgen.exe` and its own settings, and the same mesh cancelled 50 ms after TetGen starts.

#[path = "mesh_support.rs"]
mod support;

use std::collections::BTreeSet;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use simpa_core::formats::mbin;
use simpa_core::formats::tetgen::{self, TetgenFile};
use simpa_core::mesh::{MeshManifest, MeshStatus, Mesher, TetgenMesher, codes, mesh_project};
use simpa_core::process::{CancelToken, Line, Outcome};
use support::{invariants, load_room, process_running, scratch, tetgen_exe};

/// The hall's mesh, and what the tests read of its folder, held in memory: the folder is the
/// scratch folder of the test that meshed it, removed when that test passes
/// (`common/scratch.rs`), while the other test may still be reading.
struct Reference {
    manifest: MeshManifest,
    wall_ms: f64,
    /// `scene_mesh.1.face` and `tetramesh.mbin`, as read right after the mesh, or why not.
    face: Result<TetgenFile, String>,
    mesh: Result<mbin::Mesh, String>,
}

/// The hall, meshed once, uncancelled: the reference the cancel test compares with.
fn hall() -> &'static Reference {
    static HALL: OnceLock<Reference> = OnceLock::new();
    HALL.get_or_init(|| {
        let p = load_room("elmia_corrected.simpa");
        let dir = scratch("hall");
        let t0 = Instant::now();
        let manifest = mesh_project(
            &p,
            &dir,
            &TetgenMesher::new(tetgen_exe()),
            &CancelToken::new(),
            &mut |_: &Line| {},
        )
        .unwrap();
        let wall_ms = t0.elapsed().as_secs_f64() * 1e3;
        Reference {
            manifest,
            wall_ms,
            face: tetgen::read_file(&dir.join("scene_mesh.1.face")).map_err(|e| e.to_string()),
            mesh: mbin::read_file(&dir.join("tetramesh.mbin")).map_err(|e| e.to_string()),
        }
    })
}

#[test]
fn the_corrected_hall_meshes() {
    let r = hall();
    let m = &r.manifest;
    assert_eq!(m.status, MeshStatus::Ok, "{m:#?}");
    let call = m.tetgen.as_ref().unwrap();
    assert_eq!(call.argv, ["-pq2", "-A", "-n", "scene_mesh.poly"]);

    let TetgenFile::Face(face) = r.face.as_ref().unwrap() else {
        panic!("not a .face")
    };
    let markers = face.markers.as_ref().expect(".face has markers");
    assert!(markers.iter().all(|&x| x >= 0), "every .face marker >= 0");
    let covered: BTreeSet<i32> = markers.iter().copied().collect();
    assert_eq!(covered, (0..7860).collect::<BTreeSet<i32>>());

    let mesh = r.mesh.as_ref().unwrap();
    assert_eq!(invariants(mesh), Vec::<String>::new());
    // One region, TetGen's attribute 1, written unchanged (decision 1).
    assert!(mesh.tetrahedra.iter().all(|t| t.id_volume == 1));
    let in_mbin: BTreeSet<i32> = mesh
        .tetrahedra
        .iter()
        .flat_map(|t| t.faces.iter())
        .map(|f| f.marker)
        .filter(|&x| x >= 0)
        .collect();
    assert_eq!(in_mbin.len(), 7860);
    let volume: f64 = mesh
        .tetrahedra
        .iter()
        .map(|t| support::orient(mesh, t).abs() / 6.0)
        .sum();
    println!(
        "hall -pq2 -A -n: {} nodes, {} tetrahedra, {} .face rows, volume {volume:.3} m³; TetGen \
         {:.0} ms, whole mesh_project {:.0} ms (wall {:.0} ms)",
        mesh.nodes.len(),
        mesh.tetrahedra.len(),
        face.faces.len(),
        call.elapsed_ms,
        m.elapsed_ms,
        r.wall_ms
    );
    // PROVENANCE.md: signed volume 10,389.096 m³ (f64). The nodes are f32 here: 1e-5 relative.
    assert!(
        ((volume - 10_389.096) / 10_389.096).abs() < 1e-5,
        "volume {volume}"
    );
}

static COPY: AtomicUsize = AtomicUsize::new(0);

/// A copy of tetgen.exe in `dir` under a name no other process uses, so that "nothing is left
/// running" can be checked by image name, and by deleting the copy (Windows refuses to delete a
/// running image). Returns the image name and the path.
fn tetgen_copy(dir: &Path) -> (String, PathBuf) {
    let image = format!(
        "tetgen_cancel_{}_{}.exe",
        std::process::id(),
        COPY.fetch_add(1, Ordering::SeqCst)
    );
    let exe = dir.join(&image);
    std::fs::copy(tetgen_exe(), &exe).unwrap();
    (image, exe)
}

/// Sets the cancel token `after` the moment TetGen is launched, so the cancel lands while TetGen
/// runs whatever the work before the launch costs. `honour_cancel: false` hands TetGen a token
/// nobody sets: a mesher whose kill is broken, for the negative control.
struct CancelAfterLaunch {
    inner: TetgenMesher,
    after: Duration,
    honour_cancel: bool,
    launched: Mutex<Option<Instant>>,
}

impl Mesher for CancelAfterLaunch {
    fn program(&self) -> Option<&Path> {
        self.inner.program()
    }

    fn run(
        &self,
        dir: &Path,
        args: &[String],
        cancel: &CancelToken,
        on_line: &mut dyn FnMut(&Line),
    ) -> io::Result<Outcome> {
        *self.launched.lock().unwrap() = Some(Instant::now());
        let trigger = cancel.clone();
        let after = self.after;
        let timer = std::thread::spawn(move || {
            std::thread::sleep(after);
            trigger.cancel();
        });
        let ignored = CancelToken::new();
        let token = if self.honour_cancel { cancel } else { &ignored };
        let outcome = self.inner.run(dir, args, token, on_line);
        timer.join().unwrap();
        outcome
    }
}

/// One hall mesh through [`CancelAfterLaunch`], with its own copy of tetgen.exe.
struct CancelRun {
    out: PathBuf,
    image: String,
    exe: PathBuf,
    manifest: MeshManifest,
    wall_ms: f64,
    /// When TetGen was launched, from the start of the call.
    launch_ms: f64,
}

fn cancel_run(label: &str, honour_cancel: bool) -> CancelRun {
    let dir = scratch(label);
    let (image, exe) = tetgen_copy(&dir);
    let out = dir.join("mesh");
    let p = load_room("elmia_corrected.simpa");
    let mesher = CancelAfterLaunch {
        inner: TetgenMesher::new(&exe),
        after: Duration::from_millis(50),
        honour_cancel,
        launched: Mutex::new(None),
    };
    let t0 = Instant::now();
    let manifest =
        mesh_project(&p, &out, &mesher, &CancelToken::new(), &mut |_: &Line| {}).unwrap();
    let wall_ms = t0.elapsed().as_secs_f64() * 1e3;
    let launched = mesher
        .launched
        .lock()
        .unwrap()
        .expect("TetGen was launched");
    CancelRun {
        out,
        image,
        exe,
        manifest,
        wall_ms,
        launch_ms: launched.duration_since(t0).as_secs_f64() * 1e3,
    }
}

#[test]
fn a_cancel_50_ms_into_tetgen_stops_it_and_leaves_no_mbin() {
    let reference = hall();
    assert!(reference.manifest.is_ok());
    let tetgen_ms = reference.manifest.tetgen.as_ref().unwrap().elapsed_ms;
    assert!(
        tetgen_ms > 200.0,
        "an uncancelled hall mesh takes {tetgen_ms:.0} ms of TetGen: too short for a 50 ms \
         cancel to prove anything"
    );

    let CancelRun {
        out,
        image,
        exe,
        manifest: m,
        wall_ms,
        launch_ms,
    } = cancel_run("hall-cancel", true);
    let call = m.tetgen.as_ref().expect("TetGen was called");
    println!(
        "cancelled hall: TetGen launched {launch_ms:.0} ms into the call, cancelled 50 ms later, \
         ran {:.0} ms; {wall_ms:.0} ms wall; uncancelled: TetGen {tetgen_ms:.0} ms, {:.0} ms \
         wall; codes {:?}",
        call.elapsed_ms, reference.wall_ms, m.codes
    );

    assert_eq!(m.status, MeshStatus::Cancelled, "{m:#?}");
    assert!(m.codes.iter().any(|c| c == codes::CANCELLED));
    // TetGen was running when the cancel came, and was killed: no exit code of its own.
    assert!(call.cancelled && call.exit_code.is_none(), "{call:#?}");
    assert!(call.elapsed_ms < tetgen_ms, "{call:#?}");
    assert!(!out.join("tetramesh.mbin").exists());
    assert!(
        !out.join("scene_mesh.1.ele").exists(),
        "TetGen had not finished"
    );
    assert!(wall_ms < reference.wall_ms);
    assert!(!process_running(&image), "{image} is still running");
    std::fs::remove_file(&exe).expect("the tetgen copy is no longer running");
}

/// The negative control for the test above: a mesher that never passes the cancel on. The
/// token is still set 50 ms in, so the pipeline refuses to write a `.mbin` and the status is
/// `CANCELLED` either way: only the TetGen call's own record, and the `.1.ele` it finished,
/// show that nothing was killed.
#[test]
fn a_mesher_that_ignores_cancel_is_caught() {
    let CancelRun {
        out,
        image,
        exe,
        manifest: m,
        ..
    } = cancel_run("hall-nocancel", false);
    let call = m.tetgen.as_ref().expect("TetGen was called");
    assert_eq!(m.status, MeshStatus::Cancelled, "{m:#?}");
    assert!(!out.join("tetramesh.mbin").exists());
    assert!(!call.cancelled && call.exit_code == Some(0), "{call:#?}");
    assert!(out.join("scene_mesh.1.ele").exists(), "TetGen finished");
    assert!(!process_running(&image));
    std::fs::remove_file(&exe).unwrap();
}
