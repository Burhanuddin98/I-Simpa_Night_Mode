//! `simpa mesh` and `simpa mesh-verify` through the built binary, with the M1 build of
//! `tetgen.exe` (found as the CLI finds it: `$SIMPA_SOLVERS_DIR`, else the dev tree):
//! - the tutorial box and the corrected hall mesh, and the result verifies (gates M5(a), M5(b));
//! - a self-intersecting raw `.poly` is refused with its intersecting facets named, and the
//!   committed broken hall fails `mesh-verify` (M5(c));
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
    // Every tetrahedron carries TetGen's room, 1, as upstream's tutorial mesh does (decision 1):
    // the folder passes with upstream's ids given explicitly, and fails read as the mesh of a
    // project with one fitting zone, 2, whose room TetGen would number from 3.
    assert_eq!(r["mesh"]["volume_by_id"].as_object().unwrap().len(), 1);
    assert!(r["mesh"]["volume_by_id"]["1"].is_number(), "{}", r["mesh"]);
    let upstream = verify(&out, &["--room-id", "1"]);
    assert_eq!(upstream.code, 0, "{upstream:#?}");
    let wrong = verify(&out, &["--fittings", "2"]);
    assert_eq!(wrong.code, 4, "{wrong:#?}");
    assert_eq!(
        strings(&json(&wrong)["codes"]),
        ["unknown_volume_ids".to_string()]
    );

    // --from-tetgen builds the same .mbin from that TetGen output, with no TetGen run.
    let again = scratch("mesh-box-external");
    let out_arg = out.display().to_string();
    let e = mesh(
        &fixture("rooms/tutorial1_box.simpa"),
        &again,
        &["--from-tetgen", &out_arg],
    );
    assert_eq!(e.code, 0, "{e:#?}");
    let em = json(&e);
    assert_eq!(
        (em["status"].as_str(), em["source"].as_str()),
        (Some("OK"), Some("external"))
    );
    assert_eq!(em["files"]["mbin"], m["files"]["mbin"]);
    // And from a folder with no TetGen output it fails, naming what is missing.
    let empty = scratch("mesh-box-no-tetgen");
    let f = mesh(
        &fixture("rooms/tutorial1_box.simpa"),
        &again,
        &["--from-tetgen", &empty.display().to_string()],
    );
    assert_eq!(f.code, 4, "{f:#?}");
    assert!(strings(&json(&f)["codes"]).contains(&"tetgen_output_missing".to_string()));
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

/// Gate M5(c) with TetGen 1.5.0: the survey's self-intersecting cube stops TetGen (exit 3), which
/// writes no `_skipped.face`; `tetgen_self_intersection` names the facets its stop and its `-d`
/// follow-up find, 8, 9 and 12, mapped to the same scene faces. The committed broken-hall set,
/// TetGen 1.6.0's output with its `_skipped.face`, still verifies as before.
#[test]
fn a_self_intersecting_poly_is_refused_and_the_broken_hall_fails_verification() {
    let out = scratch("mesh-tg-bad");
    let o = mesh(&fixture("meshes/tg_bad/scene_mesh.poly"), &out, &[]);
    assert_eq!(o.code, 4, "{o:#?}");
    let m = json(&o);
    assert_eq!(m["status"], "FAIL");
    assert_eq!(
        strings(&m["codes"]),
        [
            "tetgen_exit_nonzero",
            "tetgen_self_intersection",
            "tetgen_output_missing",
            "neigh_missing"
        ]
    );
    assert_eq!(m["skipped_facets"].as_array().unwrap().len(), 0);
    let si = &m["self_intersection"];
    let field = |key: &str| -> Vec<i64> {
        si["facets"]
            .as_array()
            .unwrap()
            .iter()
            .map(|s| s[key].as_i64().unwrap())
            .collect()
    };
    assert_eq!(field("marker"), [8, 9, 12]);
    assert_eq!(
        field("scene_face"),
        [8, 9, 12],
        "mapped back to scene faces"
    );
    assert_eq!(si["pairs"], serde_json::json!([[8, 12], [9, 12]]));
    assert_eq!(si["stop"]["second"]["markers"], serde_json::json!([8]));
    assert!(!out.join("tetramesh.mbin").exists());
    // Without --json: the summary names the code.
    let plain = simpa_run(&[
        "mesh".to_string(),
        fixture("meshes/tg_bad/scene_mesh.poly")
            .display()
            .to_string(),
        "--out".into(),
        out.display().to_string(),
    ]);
    assert_eq!(plain.code, 4, "{plain:#?}");
    assert!(
        plain.stdout.contains("tetgen_self_intersection"),
        "{}",
        plain.stdout
    );

    // Night Mode's own .mbin of the broken hall writes the room as 0: read so, its codes are the
    // folder's and the mesh's own; read with TetGen's numbering, every tetrahedron is also unknown.
    let v = verify(&fixture("meshes/broken_hall"), &["--room-id", "0"]);
    assert_eq!(v.code, 4, "{v:#?}");
    let r = json(&v);
    assert_eq!(
        strings(&r["codes"]),
        [
            "tetgen_skipped_facets",
            "neigh_missing",
            "degenerate_tets",
            "unmarked_boundary_faces",
            "uncovered_scene_faces"
        ]
    );
    assert_eq!(r["skipped_facets"], 535);
    let v = verify(&fixture("meshes/broken_hall"), &[]);
    assert_eq!(v.code, 4, "{v:#?}");
    let codes = strings(&json(&v)["codes"]);
    assert_eq!(codes[..2], ["tetgen_skipped_facets", "neigh_missing"]);
    assert_eq!(codes.last().map(String::as_str), Some("unknown_volume_ids"));
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

/// `--cancel-after-ms` reaches `preprocess.exe` too: the corrected hall with upstream's scene
/// correction on, whose `preprocess.exe` runs for longer than a second, cancelled 50 ms in, exits
/// 130 with `preprocess.exe` killed, TetGen never started and nothing left running.
#[test]
fn a_cancel_50_ms_into_preprocess_exits_130_and_leaves_nothing_running() {
    let dir = scratch("mesh-cancel-preprocess");
    let text = std::fs::read_to_string(fixture("rooms/elmia_corrected.simpa")).unwrap();
    assert_eq!(text.matches("\"preprocess\": false").count(), 1);
    let project = dir.join("elmia_corrected_preprocess.simpa");
    std::fs::write(
        &project,
        text.replace("\"preprocess\": false", "\"preprocess\": true"),
    )
    .unwrap();
    let image = format!("preprocess-cancel-{}.exe", std::process::id());
    let pre = private_copy(&solver_exe("preprocess.exe"), &dir, &image);
    let out = dir.join("mesh");
    let o = mesh(
        &project,
        &out,
        &[
            "--preprocess",
            &pre.display().to_string(),
            "--cancel-after-ms",
            "50",
        ],
    );
    assert_eq!(o.code, 130, "{o:#?}");
    let m = json(&o);
    assert_eq!(m["status"], "CANCELLED");
    assert_eq!(strings(&m["codes"]), ["cancelled".to_string()]);
    // preprocess.exe itself was killed: no exit code, cancelled; TetGen never ran.
    let call = &m["preprocess"]["call"];
    assert_eq!(call["cancelled"], true, "{}", m["preprocess"]);
    assert_eq!(call["exit_code"], Value::Null);
    assert_eq!(m["tetgen"], Value::Null);
    assert!(!out.join("tetramesh.mbin").exists());
    assert!(!image_running(&image), "{image} still runs");
    std::fs::remove_file(&pre).expect("the preprocess.exe copy is no longer running");
    println!(
        "cancelled: preprocess.exe ran {:.0} ms before the kill, simpa mesh took {:.0} ms",
        call["elapsed_ms"], o.ms
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
