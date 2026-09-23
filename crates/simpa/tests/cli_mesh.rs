//! `simpa mesh` and `simpa mesh-verify` through the built binary, with the M1 build of
//! `tetgen.exe` (found as the CLI finds it: `$SIMPA_SOLVERS_DIR`, else the dev tree):
//! - the tutorial box and the corrected hall mesh, and the result verifies (gates M5(a), M5(b));
//! - a self-intersecting raw `.poly` is refused with its skipped facets named, and the committed
//!   broken hall fails `mesh-verify` (M5(c));
//! - a cancel 50 ms into TetGen exits 130 with TetGen killed and nothing left running (M5(g)).

mod support;

use serde_json::Value;
use support::*;

fn mesh(input: &std::path::Path, out: &std::path::Path, extra: &[&str]) -> Out {
    let mut args: Vec<String> = vec![
        "mesh".into(),
        input.display().to_string(),
        "--out".into(),
        out.display().to_string(),
        "--json".into(),
    ];
    args.extend(extra.iter().map(|s| s.to_string()));
    simpa_run(&args)
}

fn verify(dir: &std::path::Path, extra: &[&str]) -> Out {
    let mut args: Vec<String> = vec![
        "mesh-verify".into(),
        dir.display().to_string(),
        "--json".into(),
    ];
    args.extend(extra.iter().map(|s| s.to_string()));
    simpa_run(&args)
}

fn strings(v: &Value) -> Vec<String> {
    v.as_array()
        .into_iter()
        .flatten()
        .map(|c| c.as_str().unwrap().to_string())
        .collect()
}

#[test]
fn the_box_meshes_and_verifies() {
    solver_exe("tetgen.exe");
    let out = scratch("mesh-box");
    let o = mesh(&fixture("rooms/tutorial1_box.simpa"), &out, &[]);
    assert_eq!(o.code, 0, "{o:#?}");
    let m = json(&o);
    assert_eq!(m["status"], "OK");
    assert_eq!(
        m["tetgen"]["argv"],
        serde_json::json!(["-pq2", "-A", "-n", "scene_mesh.poly"])
    );
    assert!(out.join("tetramesh.mbin").is_file());
    // TetGen's lines went to stderr, the manifest alone to stdout.
    assert!(!o.stderr.is_empty());
    let v = verify(&out, &[]);
    assert_eq!(v.code, 0, "{v:#?}");
    let r = json(&v);
    assert_eq!(strings(&r["codes"]), Vec::<String>::new());
    assert_eq!(r["mesh"]["unmarked_boundary_faces"], 0);
    assert_eq!(r["mesh"]["misordered_faces"], 0);
    assert_eq!(r["mesh"]["uncovered_scene_faces"], 0);
    println!(
        "box: {} tetrahedra, TetGen {:.0} ms, simpa mesh {:.0} ms",
        r["mesh"]["tetrahedra"], m["tetgen"]["elapsed_ms"], o.ms
    );
    // The same folder read with upstream's room id: every tetrahedron is then unknown.
    let wrong = verify(&out, &["--room-id", "1"]);
    assert_eq!(wrong.code, 4, "{wrong:#?}");
    assert!(strings(&json(&wrong)["codes"]).contains(&"unknown_volume_ids".to_string()));
}

#[test]
fn the_corrected_hall_meshes_with_every_scene_face_covered() {
    let out = scratch("mesh-hall");
    let o = mesh(&fixture("rooms/elmia_corrected.simpa"), &out, &[]);
    assert_eq!(o.code, 0, "{}", o.stdout);
    let m = json(&o);
    assert_eq!(m["status"], "OK");
    assert_eq!(m["skipped_rows"], 0);
    let v = verify(&out, &[]);
    assert_eq!(v.code, 0, "{v:#?}");
    let r = json(&v);
    assert_eq!(r["mesh"]["scene_faces"], 7860);
    assert_eq!(r["mesh"]["uncovered_scene_faces"], 0);
    assert_eq!(r["mesh"]["unmarked_boundary_faces"], 0);
    println!(
        "hall: {} tetrahedra, {} .face rows, TetGen {:.0} ms, simpa mesh {:.0} ms",
        r["mesh"]["tetrahedra"], m["counts"]["build"]["face_rows"], m["tetgen"]["elapsed_ms"], o.ms
    );
}

#[test]
fn a_self_intersecting_poly_is_refused_and_the_broken_hall_fails_verification() {
    let out = scratch("mesh-tg-bad");
    let o = mesh(&fixture("meshes/tg_bad/scene_mesh.poly"), &out, &[]);
    assert_eq!(o.code, 4, "{o:#?}");
    let m = json(&o);
    assert_eq!(m["status"], "FAIL");
    assert!(strings(&m["codes"]).contains(&"tetgen_skipped_facets".to_string()));
    let markers: Vec<i64> = m["skipped_facets"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["marker"].as_i64().unwrap())
        .collect();
    assert_eq!(markers, [8, 9, 12]);
    let faces: Vec<i64> = m["skipped_facets"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["scene_face"].as_i64().unwrap())
        .collect();
    assert_eq!(faces, [8, 9, 12], "mapped back to scene faces");
    assert!(!out.join("tetramesh.mbin").exists());

    let v = verify(&fixture("meshes/broken_hall"), &[]);
    assert_eq!(v.code, 4, "{v:#?}");
    let r = json(&v);
    let codes = strings(&r["codes"]);
    assert_eq!(codes[..2], ["tetgen_skipped_facets", "neigh_missing"]);
    assert_eq!(r["skipped_facets"], 535);
}

#[test]
fn a_cancel_50_ms_into_tetgen_exits_130_and_leaves_nothing_running() {
    let dir = scratch("mesh-cancel");
    // A copy under a name of its own: no sibling test runs it, and a running image cannot be
    // deleted, which is the second proof that it ended.
    let image = format!("tetgen-cancel-{}.exe", std::process::id());
    let tetgen = private_copy(&solver_exe("tetgen.exe"), &dir, &image);
    let out = dir.join("mesh");
    let o = mesh(
        &fixture("rooms/elmia_corrected.simpa"),
        &out,
        &[
            "--tetgen",
            &tetgen.display().to_string(),
            "--cancel-after-ms",
            "50",
        ],
    );
    assert_eq!(o.code, 130, "{o:#?}");
    let m = json(&o);
    assert_eq!(m["status"], "CANCELLED");
    // TetGen itself was killed: no exit code, cancelled, and it never wrote its elements.
    assert_eq!(m["tetgen"]["cancelled"], true);
    assert_eq!(m["tetgen"]["exit_code"], Value::Null);
    assert!(!out.join("scene_mesh.1.ele").exists());
    assert!(!out.join("tetramesh.mbin").exists());
    assert!(!image_running(&image), "{image} still runs");
    std::fs::remove_file(&tetgen).expect("the TetGen copy is no longer running");
    println!(
        "cancelled: TetGen ran {:.0} ms before the kill, simpa mesh took {:.0} ms",
        m["tetgen"]["elapsed_ms"], o.ms
    );
}

#[test]
fn a_project_the_geometry_check_refuses_is_exit_3() {
    let o = mesh(
        &fixture("geometry/two_boxes_interpenetrating.simpa"),
        &scratch("mesh-refused"),
        &[],
    );
    assert_eq!(o.code, 3, "{o:#?}");
    assert!(o.stderr.contains("self_intersections"), "{}", o.stderr);
}
