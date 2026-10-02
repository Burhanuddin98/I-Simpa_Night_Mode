//! The product's default band set: octave bands 125 Hz to 8 kHz for new projects (decision-log
//! row 43), so that STI (IEC 60268-16, seven octaves 125 Hz to 8 kHz) can be computed by default.
//! An import keeps its own bands: a `config.xml` its `freq_enum`, a `.proj` upstream's 27
//! third-octave bands.

use simpa_core::schema::{BandKind, BandSet, Project, SolverSettings};

#[test]
fn a_new_project_computes_the_octaves_125_hz_to_8_khz() {
    let p = Project::new("New project");
    assert_eq!(p.bands.kind, BandKind::Octave);
    assert_eq!(
        p.bands.frequencies_hz,
        vec![125, 250, 500, 1000, 2000, 4000, 8000],
        "decision row 43: seven octaves, 8 kHz included"
    );
    assert_eq!(p.bands, BandSet::default());
    assert_eq!(p.bands, BandSet::range(BandKind::Octave, 125, 8000).unwrap());
    // Every per-band setting follows the band count.
    assert_eq!(p.solvers, SolverSettings::for_bands(7));
    assert_eq!(p.solvers.spps.bands_computed.len(), 7);
    assert_eq!(p.solvers.tcr.bands_computed.len(), 7);
    assert!(p.check_integrity().is_ok());
}

#[test]
fn a_mesh_import_is_a_new_project_and_takes_the_default_bands() {
    use simpa_core::geometry::import::{ImportOptions, Unit, Up, import_file};
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../testdata");
    let p = import_file(
        &root.join("elmia_corrected.ply"),
        &ImportOptions::new(Unit::Metre, Up::Z),
    )
    .unwrap()
    .to_project("elmia_corrected");
    assert_eq!(p.bands.frequencies_hz.last(), Some(&8000));
    assert_eq!(p.materials[0].absorption.len(), 7);
}
