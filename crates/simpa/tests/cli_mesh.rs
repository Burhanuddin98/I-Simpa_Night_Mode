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
    // The region check against the folder's .poly adds that no tetrahedron carries id 2.
    assert_eq!(
        strings(&json(&wrong)["codes"]),
        [
            "unknown_volume_ids".to_string(),
            "fitting_region_misplaced".to_string()
        ]
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
    // Its model.poly does not read with this crate's reader (a facet header of one field), and a
    // .poly that does not read is not replaced by the .cbin: its regions are held to no cells
    // (decision 15).
    let v = verify(&fixture("meshes/broken_hall"), &["--room-id", "0"]);
    assert_eq!(v.code, 4, "{v:#?}");
    let r = json(&v);
    assert_eq!(
        strings(&r["codes"]),
        [
            "tetgen_skipped_facets",
            "neigh_missing",
            "regions_unchecked",
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

/// `preprocess.exe` still running at the mesher's limit (`--preprocess-timeout-ms`) is stopped,
/// its process killed, and the mesh fails by name: `preprocess_timeout`, exit 4, TetGen never
/// run. The hall's preprocess.exe takes about 2 s to give up; the limit is 50 ms.
#[test]
fn a_preprocess_timeout_exits_4_and_leaves_nothing_running() {
    let dir = scratch("mesh-timeout-preprocess");
    let text = std::fs::read_to_string(fixture("rooms/elmia_corrected.simpa")).unwrap();
    let project = dir.join("elmia_corrected_preprocess.simpa");
    std::fs::write(
        &project,
        text.replace("\"preprocess\": false", "\"preprocess\": true"),
    )
    .unwrap();
    let image = format!("preprocess-timeout-{}.exe", std::process::id());
    let pre = private_copy(&solver_exe("preprocess.exe"), &dir, &image);
    let out = dir.join("mesh");
    let o = mesh(
        &project,
        &out,
        &[
            "--preprocess",
            &pre.display().to_string(),
            "--preprocess-timeout-ms",
            "50",
        ],
    );
    assert_eq!(o.code, 4, "{o:#?}");
    let m = json(&o);
    assert_eq!(m["status"], "FAIL");
    assert_eq!(strings(&m["codes"]), ["preprocess_timeout".to_string()]);
    let call = &m["preprocess"]["call"];
    assert_eq!(call["timed_out"], true, "{}", m["preprocess"]);
    assert_eq!(call["timeout_ms"], 50.0);
    assert_eq!(call["exit_code"], Value::Null);
    assert_eq!(m["tetgen"], Value::Null);
    assert!(!out.join("tetramesh.mbin").exists());
    assert!(!image_running(&image), "{image} still runs");
    std::fs::remove_file(&pre).expect("the preprocess.exe copy is no longer running");
    // Says no: a limit that is not a whole number of milliseconds is a usage error.
    let bad = mesh(
        &project,
        &dir.join("bad"),
        &["--preprocess-timeout-ms", "1.5"],
    );
    assert_eq!(bad.code, 2, "{bad:#?}");
}

/// A fitting zone pinned so high that TetGen's room ids above it would pass a C `int` is refused
/// before meshing, by name, where it used to pass `simpa validate` and make `simpa mesh` panic
/// (`attempt to add with overflow` in `VolumeIds::tetgen`, exit 101). The control: pinned at the
/// limit, the scene's faces and the box's 12 triangles below `i32::MAX`, it meshes, the room one
/// id above it. `mesh-verify --fittings 2147483647` reads such ids without panicking.
#[test]
fn a_fitting_pin_without_room_for_the_rooms_ids_is_refused() {
    let dir = scratch("mesh-pin-headroom");
    let mut p = simpa_core::schema::load(&fixture("rooms/tutorial1_box_fitting.simpa")).unwrap();
    assert_eq!(p.fitting_zones.len(), 1);
    let facets = p.geometry.faces.len() as u32 + 12;
    let limit = i32::MAX as u32 - facets;
    let pinned = |p: &mut simpa_core::schema::Project, id: u32, name: &str| {
        p.fitting_zones[0].solver_id = Some(id);
        let path = dir.join(name);
        simpa_core::schema::save(p, &path).unwrap();
        path
    };
    for (id, name) in [(i32::MAX as u32, "max.simpa"), (limit + 1, "over.simpa")] {
        let project = pinned(&mut p, id, name);
        let o = mesh(&project, &dir.join(format!("mesh-{id}")), &[]);
        assert_eq!(o.code, 2, "pin {id}: {o:#?}");
        assert!(
            o.stderr.contains("solver_id_mapping_invalid")
                && o.stderr.contains(&format!("pin it at most {limit}")),
            "pin {id}: {}",
            o.stderr
        );
        assert!(!o.stderr.contains("panicked"), "{}", o.stderr);
        let v = simpa_run(&["validate".to_string(), project.display().to_string()]);
        assert_eq!(v.code, 2, "validate, pin {id}: {v:#?}");
    }
    let project = pinned(&mut p, limit, "limit.simpa");
    let out = dir.join("mesh-limit");
    let o = mesh(&project, &out, &[]);
    assert_eq!(o.code, 0, "pin {limit}: {o:#?}");
    let ids: std::collections::BTreeSet<i32> =
        simpa_core::formats::mbin::read_file(&out.join("tetramesh.mbin"))
            .unwrap()
            .tetrahedra
            .iter()
            .map(|t| t.id_volume)
            .collect();
    assert_eq!(
        ids,
        std::collections::BTreeSet::from([limit as i32, limit as i32 + 1])
    );
    let v = verify(&out, &["--fittings", "2147483647"]);
    assert!(!v.stderr.contains("panicked"), "{v:#?}");
    assert_ne!(v.code, 101, "{v:#?}");
}

/// `--parity` on a project whose mesh settings ask for no scene correction has nothing to keep:
/// a note on stderr says so, and the mesh is the default one. Says no: with the correction asked
/// for, no note, and the manifest is in parity mode; without `--parity`, no note.
#[test]
fn parity_without_the_scene_correction_says_it_has_no_effect() {
    let note = "simpa: note: --parity has no effect";
    let box_ = fixture("rooms/tutorial1_box.simpa");
    let o = mesh(&box_, &scratch("mesh-parity-off"), &["--parity"]);
    assert_eq!(o.code, 0, "{o:#?}");
    assert!(o.stderr.contains(note), "{}", o.stderr);
    assert_eq!(json(&o)["parity"], false);

    let plain = mesh(&box_, &scratch("mesh-parity-none"), &[]);
    assert_eq!(plain.code, 0, "{plain:#?}");
    assert!(!plain.stderr.contains(note), "{}", plain.stderr);

    let dir = scratch("mesh-parity-on");
    let text = std::fs::read_to_string(&box_).unwrap();
    let project = dir.join("box_preprocess.simpa");
    std::fs::write(
        &project,
        text.replace("\"preprocess\": false", "\"preprocess\": true"),
    )
    .unwrap();
    let on = mesh(&project, &dir.join("mesh"), &["--parity"]);
    assert_eq!(on.code, 0, "{on:#?}");
    assert!(!on.stderr.contains(note), "{}", on.stderr);
    assert_eq!(json(&on)["parity"], true);
}
