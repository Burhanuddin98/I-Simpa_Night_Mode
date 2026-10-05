//! `simpa advise <project> [--json]`: the run-quality advisor before a run (backlog 80, T5).

mod support;

use support::*;

/// Tutorial 2 as upstream ships it (`-q2` without `-Y`, 0.31 m): the Elmia fixture with
/// `elmia_corrected()`'s edits undone, written to a scratch file.
fn tutorial2_as_shipped(dir: &std::path::Path) -> std::path::PathBuf {
    let mut p = simpa_core::schema::load(&fixture("rooms/elmia_corrected.simpa")).unwrap();
    p.solvers.meshing.preserve_boundary = false;
    p.solvers.spps.receiver_radius_m = simpa_core::schema::F64::new(0.31);
    let out = dir.join("tutorial2.simpa");
    simpa_core::schema::save(&p, &out).unwrap();
    out
}

#[test]
fn advise_names_both_warnings_for_tutorial_2_and_none_for_the_fixture() {
    let dir = scratch("advise");
    let t2 = tutorial2_as_shipped(&dir);
    let o = simpa_run(&["advise".as_ref(), t2.as_os_str(), "--json".as_ref()]);
    assert_eq!(o.code, 0, "{o:#?}");
    let v = json(&o);
    let codes: Vec<&str> = v
        .as_array()
        .unwrap()
        .iter()
        .map(|a| a["code"].as_str().unwrap())
        .collect();
    assert!(codes.contains(&"mesh_splits_walls"), "{codes:?}");
    assert!(codes.contains(&"receivers_small"), "{codes:?}");
    let small = v
        .as_array()
        .unwrap()
        .iter()
        .find(|a| a["code"] == "receivers_small")
        .unwrap();
    assert_eq!(small["fix"]["to"], 0.6);
    assert_eq!(small["fix"]["pointer"], "/solvers/spps/receiver_radius_m");
    // The text form names the setting and the Apply.
    let o = simpa_run(&["advise".as_ref(), t2.as_os_str()]);
    assert_eq!(o.code, 0, "{o:#?}");
    assert!(
        o.stdout
            .contains("receivers_small: The receivers are small"),
        "{}",
        o.stdout
    );
    assert!(
        o.stdout.contains("apply: Receiver radius 0.31 -> 0.6"),
        "{}",
        o.stdout
    );
    // Says no: the fixture has -Y and 0.6 m at a million particles.
    let f = fixture("rooms/elmia_corrected.simpa");
    let o = simpa_run(&["advise".as_ref(), f.as_os_str(), "--json".as_ref()]);
    assert_eq!(o.code, 0, "{o:#?}");
    let v = json(&o);
    assert!(
        v.as_array()
            .unwrap()
            .iter()
            .all(|a| a["code"] != "mesh_splits_walls" && a["code"] != "receivers_small"),
        "{v}"
    );
    // Usage.
    assert_eq!(simpa_run(&["advise"]).code, 2);
    assert_eq!(simpa_run(&["advise", "no-such.simpa"]).code, 2);
}
