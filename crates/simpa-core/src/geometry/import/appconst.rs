//! Upstream's reference database: the materials and spectra every upstream project may use without
//! storing them, from `src/isimpa/resources/appconst.xml` at `929a5c8` (`appmaterials`,
//! `appspectrums`). A `.proj` names a reference material only by its `idmateriau` and a reference
//! spectrum only by its `idspectre`, so reading one needs this table.
//!
//! Generated from that file; `tests/geometry_import_proj.rs` re-reads the upstream file, when the
//! checkout is present, and checks every value here against it. Every reference material is the
//! same in all 27 bands, specular, with no scattering and no transmission, and acts on both sides
//! of a face (`side_material` 1); only the absorption and the colour differ. Values are the `f32`
//! upstream's GUI holds, written as their shortest decimal.

/// A material of upstream's reference database.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ReferenceMaterial {
    /// `idmateriau`: the id groups and `config.xml` use.
    pub id: u32,
    pub name: &'static str,
    /// The same in every band.
    pub absorption: f32,
    pub color: [u8; 3],
}

/// A spectrum of upstream's reference database: a level per third-octave band, 50 Hz to 20 kHz.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ReferenceSpectrum {
    /// `idspectre`.
    pub id: u32,
    pub name: &'static str,
    pub band_db: [f32; 27],
}

/// Upstream's reference materials, in `appconst.xml` order. Entry 0 is id 0, `Default`: the
/// material upstream gives a face that no surface group holds.
#[rustfmt::skip]
pub const REFERENCE_MATERIALS: [ReferenceMaterial; 12] = [
    ReferenceMaterial { id: 0, name: "Default", absorption: 0.0, color: [255, 0, 0] },
    ReferenceMaterial { id: 24, name: "100% absorbing", absorption: 1.0, color: [5, 5, 5] },
    ReferenceMaterial { id: 15, name: "90% absorbing", absorption: 0.9, color: [25, 25, 25] },
    ReferenceMaterial { id: 16, name: "80% absorbing", absorption: 0.8, color: [51, 51, 51] },
    ReferenceMaterial { id: 17, name: "70% absorbing", absorption: 0.7, color: [76, 76, 76] },
    ReferenceMaterial { id: 18, name: "60% absorbing", absorption: 0.6, color: [102, 102, 102] },
    ReferenceMaterial { id: 19, name: "50% absorbing", absorption: 0.5, color: [127, 127, 127] },
    ReferenceMaterial { id: 20, name: "40% absorbing", absorption: 0.4, color: [153, 153, 153] },
    ReferenceMaterial { id: 21, name: "30% absorbing", absorption: 0.3, color: [178, 178, 178] },
    ReferenceMaterial { id: 22, name: "20% absorbing", absorption: 0.2, color: [204, 204, 204] },
    ReferenceMaterial { id: 25, name: "10% absorbing", absorption: 0.1, color: [230, 230, 230] },
    ReferenceMaterial { id: 23, name: "0% absorbing", absorption: 0.0, color: [255, 255, 255] },
];

/// Upstream's reference spectra, in `appconst.xml` order. Each is normalised to about 0 dB
/// overall; a source or receiver adds its own global level `Lw` to it.
#[rustfmt::skip]
pub const REFERENCE_SPECTRA: [ReferenceSpectrum; 8] = [
    ReferenceSpectrum { id: 2, name: "ES_VL", band_db: [49.9, 49.9, 49.9, 49.9, 52.3, 54.6, 57.7, 61.0, 63.2, 64.3, 66.6, 72.5, 75.6, 77.7, 76.3, 71.6, 68.6, 66.2, 63.6, 61.3, 58.1, 58.1, 58.1, 58.1, 58.1, 58.1, 58.1] },
    ReferenceSpectrum { id: 3, name: "ES_TR", band_db: [57.1, 57.1, 57.1, 57.1, 60.2, 63.1, 66.7, 68.3, 70.6, 72.5, 75.6, 80.3, 79.2, 77.7, 74.5, 72.8, 71.0, 69.3, 67.6, 65.8, 63.6, 63.6, 63.6, 63.6, 63.6, 63.6, 63.6] },
    ReferenceSpectrum { id: 1, name: "Pink noise", band_db: [-14.31, -14.31, -14.31, -14.31, -14.31, -14.31, -14.31, -14.31, -14.31, -14.31, -14.31, -14.31, -14.31, -14.31, -14.31, -14.31, -14.31, -14.31, -14.31, -14.31, -14.31, -14.31, -14.31, -14.31, -14.31, -14.31, -14.31] },
    ReferenceSpectrum { id: 0, name: "White noise", band_db: [-32.86, -31.86, -30.86, -29.86, -28.86, -27.86, -26.86, -25.86, -24.86, -23.86, -22.86, -21.86, -20.86, -19.86, -18.86, -17.86, -16.86, -15.86, -14.86, -13.86, -12.86, -11.86, -10.86, -9.86, -8.86, -7.86, -6.86] },
    ReferenceSpectrum { id: 4, name: "BBSG_VL", band_db: [47.9, 47.9, 47.9, 47.9, 46.2, 48.4, 51.6, 53.3, 54.7, 56.2, 58.7, 62.9, 66.0, 69.9, 69.5, 68.5, 66.7, 63.8, 61.2, 58.2, 55.6, 55.6, 55.6, 55.6, 55.6, 55.6, 55.6] },
    ReferenceSpectrum { id: 5, name: "BBSG_TR", band_db: [51.9, 51.9, 51.9, 51.9, 55.2, 56.9, 59.5, 61.8, 65.8, 70.5, 70.8, 76.6, 77.1, 76.5, 75.5, 73.9, 72.3, 69.1, 67.1, 64.5, 62.2, 62.2, 62.2, 62.2, 62.2, 62.2, 62.2] },
    ReferenceSpectrum { id: 6, name: "BBDR_VL", band_db: [46.6, 46.6, 46.6, 46.6, 44.8, 48.0, 49.5, 52.8, 55.0, 55.9, 57.1, 63.4, 64.4, 63.9, 65.7, 66.0, 64.7, 60.9, 57.6, 56.1, 53.7, 53.7, 53.7, 53.7, 53.7, 53.7, 53.7] },
    ReferenceSpectrum { id: 7, name: "BBDR_TR", band_db: [51.4, 51.4, 51.4, 51.4, 53.1, 55.300003, 57.9, 60.6, 64.9, 66.9, 67.8, 73.6, 69.9, 68.9, 69.4, 69.2, 68.4, 65.3, 63.8, 62.5, 60.1, 60.1, 60.1, 60.1, 60.1, 60.1, 60.1] },
];

/// The reference material with this id.
pub fn reference_material(id: u32) -> Option<&'static ReferenceMaterial> {
    REFERENCE_MATERIALS.iter().find(|m| m.id == id)
}

/// The reference spectrum with this id.
pub fn reference_spectrum(id: u32) -> Option<&'static ReferenceSpectrum> {
    REFERENCE_SPECTRA.iter().find(|s| s.id == id)
}
