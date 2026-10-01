//! `config.xml` carries the product's default SPPS run length: a fixed 10 s for a new project,
//! not upstream's own 2 s (decision row 36, `docs/decision-log.md`). Its own test, alongside
//! `spps_default_run_length.rs`'s check on the model itself.

use simpa_core::config_xml::{SolverKind, write};
use simpa_core::schema::Project;

#[test]
fn a_new_project_writes_duree_simulation_as_ten() {
    let p = Project::new("New project");
    let workdir = if cfg!(windows) {
        std::path::PathBuf::from(r"C:\runs\default_duration")
    } else {
        std::path::PathBuf::from("/runs/default_duration")
    };
    let xml =
        write(&p, SolverKind::Spps, None, &workdir).unwrap_or_else(|e| panic!("{}: {e}", e.code()));
    let doc = roxmltree::Document::parse(&xml).expect("well-formed config.xml");
    let sim = doc
        .descendants()
        .find(|n| n.has_tag_name("simulation"))
        .expect("a <simulation> element");
    assert_eq!(
        sim.attribute("duree_simulation"),
        Some("10"),
        "the default project's SPPS run length, as written: 10 s (decision row 36), not \
         upstream's 2 s"
    );
}
