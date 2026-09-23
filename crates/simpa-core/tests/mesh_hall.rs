//! The corrected Elmia hall (`tests/fixtures/rooms/elmia_corrected.simpa`) meshed with the real
//! `tetgen.exe` and its own settings, and the same mesh cancelled 50 ms in.

#[path = "mesh_support.rs"]
mod support;

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use simpa_core::formats::mbin;
use simpa_core::formats::tetgen::{self, TetgenFile};
use simpa_core::mesh::{MeshManifest, MeshStatus, TetgenMesher, codes, mesh_project};
use simpa_core::process::{CancelToken, Line};
use support::{invariants, load_room, process_running, scratch, tetgen_exe};

struct Reference {
    dir: PathBuf,
    manifest: MeshManifest,
    wall_ms: f64,
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
        Reference {
            dir,
            manifest,
            wall_ms: t0.elapsed().as_secs_f64() * 1e3,
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

    let TetgenFile::Face(face) = tetgen::read_file(&r.dir.join("scene_mesh.1.face")).unwrap()
    else {
        panic!("not a .face")
    };
    let markers = face.markers.as_ref().expect(".face has markers");
    assert!(markers.iter().all(|&x| x >= 0), "every .face marker >= 0");
    let covered: BTreeSet<i32> = markers.iter().copied().collect();
    assert_eq!(covered, (0..7860).collect::<BTreeSet<i32>>());

    let mesh = mbin::read_file(&r.dir.join("tetramesh.mbin")).unwrap();
    assert_eq!(invariants(&mesh), Vec::<String>::new());
    assert!(mesh.tetrahedra.iter().all(|t| t.id_volume == 0));
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
        .map(|t| support::orient(&mesh, t).abs() / 6.0)
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

#[test]
fn a_cancel_at_50_ms_stops_tetgen_and_leaves_no_mbin() {
    let reference = hall();
    assert!(reference.manifest.is_ok());
    let tetgen_ms = reference.manifest.tetgen.as_ref().unwrap().elapsed_ms;
    assert!(
        tetgen_ms > 200.0,
        "an uncancelled hall mesh takes {tetgen_ms:.0} ms: too short for a 50 ms cancel to prove \
         anything"
    );

    // A copy of tetgen.exe under a name no other process uses, so that "nothing is left
    // running" can be checked by image name, and by deleting the copy (Windows refuses to
    // delete a running image).
    let dir = scratch("hall-cancel");
    let image = format!(
        "tetgen_cancel_{}_{}.exe",
        std::process::id(),
        COPY.fetch_add(1, Ordering::SeqCst)
    );
    let exe = dir.join(&image);
    std::fs::copy(tetgen_exe(), &exe).unwrap();
    let out = dir.join("mesh");

    let p = load_room("elmia_corrected.simpa");
    let cancel = CancelToken::new();
    let trigger = cancel.clone();
    let t0 = Instant::now();
    let timer = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(50));
        trigger.cancel();
    });
    let m = mesh_project(
        &p,
        &out,
        &TetgenMesher::new(&exe),
        &cancel,
        &mut |_: &Line| {},
    )
    .unwrap();
    let wall_ms = t0.elapsed().as_secs_f64() * 1e3;
    timer.join().unwrap();
    println!(
        "cancelled hall: {:.0} ms wall, TetGen {:?} ms; uncancelled {:.0} ms wall; codes {:?}",
        wall_ms,
        m.tetgen.as_ref().map(|c| c.elapsed_ms),
        reference.wall_ms,
        m.codes
    );

    assert_eq!(m.status, MeshStatus::Cancelled, "{m:#?}");
    assert!(m.codes.iter().any(|c| c == codes::CANCELLED));
    assert!(!out.join("tetramesh.mbin").exists());
    assert!(
        !out.join("scene_mesh.1.ele").exists(),
        "TetGen had not finished"
    );
    assert!(wall_ms < reference.wall_ms);
    assert!(!process_running(&image), "{image} is still running");
    std::fs::remove_file(&exe).expect("the tetgen copy is no longer running");
}
