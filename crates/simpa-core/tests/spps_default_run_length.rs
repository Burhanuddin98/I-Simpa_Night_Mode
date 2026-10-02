//! The product's default SPPS run length: a fixed 10 s for new projects, decided over upstream's
//! own 2 s GUI default (decision row 36, `docs/decision-log.md`). Its own test, its own gate, as
//! the row asks: "The product default changes from 2.0 s in its own step, with its own tests and
//! gates".

use simpa_core::schema::Project;

#[test]
fn a_new_project_s_spps_run_length_defaults_to_ten_seconds() {
    let p = Project::new("New project");
    assert_eq!(
        p.solvers.spps.duration_s.get(),
        10.0,
        "decision row 36: the product default is a fixed 10 s, not upstream's 2 s"
    );
}

#[test]
fn the_default_holds_for_every_band_count_for_bands_builds() {
    for n in [1, 3, 6, 12] {
        let s = simpa_core::schema::SolverSettings::for_bands(n);
        assert_eq!(s.spps.duration_s.get(), 10.0, "{n} bands");
    }
}
