//! The analytic reference of an SPPS run, for M8 and M12 (`docs/params.md`, "Kuttruff's
//! reference"; Burhan, 2026-09-24 23:14): per computed band, **Kuttruff's corrected Eyring** with
//! `γ²` computed from the run's own room by the diffuse transport (`params::lambert`), M8's
//! reference, and **plain Eyring**, reported only. Both use SPPS's own speed of sound in
//! `K = 24·ln 10/c` and the air term as the solver applies it (M8 decisions 17:45 and 00:20).
//!
//! The room is read as TCR's analytic references read it ([`super::tcr::RoomInputs`]): the `.cbin`
//! faces with their materials, and the `.mbin`'s tetrahedra, which give the volume and where the
//! transport's rays start. Nothing of the run's output is read, so the reference is the same
//! whatever SPPS computed.
//!
//! **Nothing here is validated**: M8's bed has not run (`Report::validated_by_bed`). Both formulas
//! describe a diffuse field: each band says whether every face of the room reflects by Lambert's
//! law with scattering 1 in it (`lambert_walls`), the only case in which the transport's `γ²`
//! describes the run's walls.

use roxmltree::Document;

use super::tcr::{RoomInputs, band_surfaces};
use crate::params::ParamError;
use crate::params::lambert::{Enclosure, FreePaths, free_paths};
use crate::params::room::{self, RtConstant, Surface};
use crate::run::expect::{self, Expectation};
use crate::run::locate;

use std::path::Path;

/// What the reference is, said beside its numbers wherever they are shown.
pub const REFERENCE_LABEL: &str = "analytic reference for a diffuse field, not validated (M8's bed \
     has not run): kuttruff_s is Kuttruff's corrected Eyring with gamma^2 computed from this \
     room's geometry by a diffuse transport, M8's reference; eyring_s is plain Eyring, reported \
     only";

/// One computed band of the reference.
#[derive(Clone, Debug, PartialEq)]
pub struct ReferenceBand {
    pub freq_hz: i32,
    /// The energy attenuation `m` the solver applies, 1/m, added as `4·m·V`; `None` with air
    /// absorption off.
    pub air_m_per_metre: Option<f64>,
    /// `ᾱ = Σ Sᵢ·αᵢ / S` over the room's faces.
    pub mean_absorption: f64,
    /// Every face's material reflects by Lambert's law (`loi` 2) with scattering (`diffusion`) 1
    /// in this band.
    pub lambert_walls: bool,
    /// Every face has the same absorption in this band (the noise calibration tells uniform
    /// Lambert rooms apart: `params::noise::Walls`).
    pub uniform_absorption: bool,
    /// Plain Eyring, `params::room::eyring_rt`, with SPPS's `K`.
    pub eyring_s: Result<f64, ParamError>,
    /// Kuttruff's time and the standard deviation it inherits from `γ²`'s standard error
    /// (`params::room::kuttruff_rt`, `kuttruff_rt_sd`), with SPPS's `K`; refused with the
    /// transport's own refusal when the transport refused.
    pub kuttruff_s: Result<(f64, f64), ParamError>,
}

/// The reference on a run's inputs, or why there is none.
#[derive(Clone, Debug, PartialEq)]
pub enum Reference {
    Computed {
        /// The `.mbin`'s volume, m³.
        volume_m3: f64,
        /// The `.cbin` faces' total area, m².
        area_m2: f64,
        /// SPPS's speed of sound, m/s.
        speed_of_sound_m_s: f64,
        /// `K = 24·ln 10/c`, s/m.
        constant_s_per_m: f64,
        /// The transport's free paths in the room, or its refusal.
        free_paths: Result<FreePaths, ParamError>,
        bands: Vec<ReferenceBand>,
    },
    NotComputed {
        why: String,
    },
}

/// The reference of the SPPS run whose solver folder is `solve` ([module docs](self)), with
/// SPPS's speed of sound `speed_of_sound_m_s`. Not computed when the speed of sound varies with
/// height (`celerity_gradient`: rays curve, and neither formula describes that room), or the room
/// cannot be read as [`RoomInputs::read`] reads it.
pub fn reference(
    solve: &Path,
    exp: &Expectation,
    speed_of_sound_m_s: f64,
    celerity_gradient: bool,
) -> Reference {
    if celerity_gradient {
        return Reference::NotComputed {
            why: "the speed of sound varies with height (alog or blin is not 0): rays curve, and \
                  neither Eyring's formula nor Kuttruff's describes that room"
                .into(),
        };
    }
    inner(solve, exp, speed_of_sound_m_s).unwrap_or_else(|why| Reference::NotComputed { why })
}

fn inner(solve: &Path, exp: &Expectation, c: f64) -> Result<Reference, String> {
    let room = RoomInputs::read(solve, exp)?;
    let doc = Document::parse(&room.config).map_err(|e| format!("config.xml: {e}"))?;
    let laws = reflection_laws(&doc)?;
    let constant = RtConstant::Physical { speed_of_sound: c };
    let area_m2: f64 = room.faces.iter().map(|f| f.0).sum();
    if !(area_m2.is_finite() && area_m2 > 0.0) {
        return Err(format!("the scene's faces have an area of {area_m2} m²"));
    }
    let paths =
        Enclosure::from_mesh(&room.triangles, &room.tetrahedra).and_then(|e| free_paths(&e));
    let mut bands = Vec::with_capacity(room.bands.len());
    for band in &room.bands {
        let surfaces = band_surfaces(&room.faces, &room.materials, band.index)?;
        let air = band.air_m_per_metre.clone()?;
        let lambert_walls = lambert_walls(&room.faces, &laws, band.index);
        let sa: f64 = surfaces.iter().map(|x| x.area_m2 * x.absorption).sum();
        let uniform_absorption = surfaces
            .first()
            .is_some_and(|f| surfaces.iter().all(|x| x.absorption == f.absorption))
            || crate::faults::active() == Some(crate::faults::Fault::UniformAbsorptionForced);
        bands.push(ReferenceBand {
            freq_hz: band.freq_hz,
            air_m_per_metre: air,
            mean_absorption: sa / area_m2,
            lambert_walls,
            uniform_absorption,
            eyring_s: room::eyring_rt(room.volume_m3, &surfaces, air, constant),
            kuttruff_s: kuttruff(&paths, &surfaces, air, constant),
        });
    }
    Ok(Reference::Computed {
        volume_m3: room.volume_m3,
        area_m2,
        speed_of_sound_m_s: c,
        constant_s_per_m: constant.value(),
        free_paths: paths,
        bands,
    })
}

fn kuttruff(
    paths: &Result<FreePaths, ParamError>,
    surfaces: &[Surface],
    air: Option<f64>,
    constant: RtConstant,
) -> Result<(f64, f64), ParamError> {
    let p = paths.as_ref().map_err(Clone::clone)?;
    Ok((
        room::kuttruff_rt(p, surfaces, air, constant)?,
        room::kuttruff_rt_sd(p, surfaces, air, constant)?,
    ))
}

/// Each material's id and its `(diffusion, loi)` per band.
type Laws = Vec<(u32, Vec<(f64, i32)>)>;

/// Whether every face's material reflects by Lambert's law (`loi` 2) with scattering 1 in band
/// `index`: SPPS then reflects every particle diffusely, by Lambert's law
/// (`CalculationCore.cpp:318`; `docs/formats/config_xml.md`). A material or band not declared
/// counts as not.
fn lambert_walls(faces: &[(f64, u32)], laws: &Laws, index: usize) -> bool {
    faces.iter().all(|&(_, id)| {
        laws.iter()
            .find(|(m, _)| *m == id)
            .and_then(|(_, b)| b.get(index))
            .is_some_and(|&(diffusion, law)| diffusion == 1.0 && law == 2)
    })
}

/// Each material's `(diffusion, loi)` per band, in frequency order as [`super::tcr::materials`]
/// orders the absorption. Refused, with why, for a value that does not read.
fn reflection_laws(doc: &Document) -> Result<Laws, String> {
    let Some(list) = expect::child(doc.root_element(), "surface_absorption_enum") else {
        return Err("config.xml has no surface_absorption_enum".into());
    };
    let mut out = Vec::new();
    for m in expect::items(list) {
        let id = expect::atoi(m.attribute("id").unwrap_or(""));
        let mut bands = Vec::new();
        for b in expect::items(m) {
            let freq = expect::atoi(b.attribute("freq").unwrap_or(""));
            let text = b.attribute("diffusion").unwrap_or("");
            let diffusion = locate::to_float(text)
                .filter(|d| d.is_finite())
                .ok_or_else(|| format!("material {id}: diffusion {text:?} is not a number"))?;
            let law = b.attribute("loi").unwrap_or("");
            if law.is_empty() {
                return Err(format!("material {id}: a band has no loi"));
            }
            bands.push((freq, f64::from(diffusion), expect::atoi(law)));
        }
        bands.sort_by_key(|b| b.0);
        out.push((
            u32::try_from(id).map_err(|_| format!("material id {id}"))?,
            bands.into_iter().map(|b| (b.1, b.2)).collect(),
        ));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Material 1 is Lambert with scattering 1 in both bands; material 2 only in its second band
    /// (written first); material 3 is Lambert with scattering 0.5.
    const CONFIG: &str = r#"<configuration>
  <surface_absorption_enum>
    <type_surface id="1">
      <bfreq freq="500" absorb="0.2" diffusion="1" loi="2"/>
      <bfreq freq="1000" absorb="0.2" diffusion="1" loi="2"/>
    </type_surface>
    <type_surface id="2">
      <bfreq freq="1000" absorb="0.2" diffusion="1" loi="2"/>
      <bfreq freq="500" absorb="0.2" diffusion="1" loi="0"/>
    </type_surface>
    <type_surface id="3">
      <bfreq freq="500" absorb="0.2" diffusion="0.5" loi="2"/>
      <bfreq freq="1000" absorb="0.2" diffusion="0.5" loi="2"/>
    </type_surface>
  </surface_absorption_enum>
</configuration>"#;

    #[test]
    fn lambert_walls_need_every_face_lambert_with_scattering_1_in_the_band() {
        let laws = reflection_laws(&Document::parse(CONFIG).unwrap()).unwrap();
        let faces = |ids: &[u32]| ids.iter().map(|&id| (1.0, id)).collect::<Vec<_>>();
        assert!(lambert_walls(&faces(&[1, 1]), &laws, 0));
        assert!(lambert_walls(&faces(&[1, 2]), &laws, 1));
        // Says no: material 2 is specular at 500 Hz (its bands sorted by frequency), material 3
        // scatters half, and material 4 is not declared.
        assert!(!lambert_walls(&faces(&[1, 2]), &laws, 0));
        assert!(!lambert_walls(&faces(&[1, 3]), &laws, 1));
        assert!(!lambert_walls(&faces(&[1, 4]), &laws, 0));
        assert!(!lambert_walls(&faces(&[1]), &laws, 2));
        // A scattering is read as SPPS reads it (`atof`): "x" is 0, specular, so not Lambert. One
        // that reads as NaN, or a band with no law, refuses the reference.
        let laws_of = |attrs: &str| {
            reflection_laws(
                &Document::parse(&format!(
                    r#"<configuration><surface_absorption_enum><type_surface id="1"><bfreq freq="500" absorb="0.2" {attrs}/></type_surface></surface_absorption_enum></configuration>"#
                ))
                .unwrap(),
            )
        };
        let one = [(1.0, 1)];
        assert!(lambert_walls(
            &one,
            &laws_of(r#"diffusion="1" loi="2""#).unwrap(),
            0
        ));
        assert!(!lambert_walls(
            &one,
            &laws_of(r#"diffusion="x" loi="2""#).unwrap(),
            0
        ));
        assert!(laws_of(r#"diffusion="nan" loi="2""#).is_err());
        assert!(laws_of(r#"diffusion="1""#).is_err());
    }

    /// The committed SPPS fixture of tutorial 1's 6×10×3 m box (`tests/fixtures/results/`).
    fn seats() -> (std::path::PathBuf, Expectation) {
        let solve = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/results/seats_spps/solve");
        let exp = Expectation::read(&solve, crate::schema::SolverKind::Spps)
            .unwrap_or_else(|e| panic!("{}: {e}", solve.display()));
        (solve, exp)
    }

    #[test]
    fn a_run_has_its_rooms_reference_and_a_celerity_gradient_none() {
        let (solve, exp) = seats();
        let Reference::Computed {
            volume_m3,
            area_m2,
            free_paths,
            bands,
            ..
        } = reference(&solve, &exp, 343.2, false)
        else {
            panic!("the seats box has a reference");
        };
        assert!((volume_m3 - 180.0).abs() < 1e-6 && (area_m2 - 216.0).abs() < 1e-6);
        // The box's exact γ², 0.388874 (tests/params_lambert.rs), from the run's own mesh.
        let p = free_paths.unwrap();
        assert!(
            (p.gamma2() - 0.388_874).abs() < 3.0 * p.gamma2_se(),
            "{p:?}"
        );
        assert_eq!(bands.len(), 2);
        for b in &bands {
            // Its walls are specular (diffusion 0, loi 0): neither time describes its field.
            assert!(!b.lambert_walls);
            assert!((b.mean_absorption - 0.2).abs() < 1e-6);
            assert!(b.kuttruff_s.as_ref().unwrap().0 > *b.eyring_s.as_ref().unwrap());
        }
        // Rays curve when the speed of sound varies with height: no reference.
        match reference(&solve, &exp, 343.2, true) {
            Reference::NotComputed { why } => assert!(why.contains("height"), "{why}"),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn a_refused_transport_is_kuttruffs_refusal_never_a_number() {
        let refused: Result<FreePaths, ParamError> = Err(ParamError::TransportRefused {
            detail: "a ray left the enclosure".into(),
        });
        let s = [Surface {
            area_m2: 1.0,
            absorption: 0.2,
        }];
        let k = kuttruff(
            &refused,
            &s,
            None,
            RtConstant::Physical {
                speed_of_sound: 343.2,
            },
        );
        assert_eq!(
            k.unwrap_err().code(),
            crate::params::codes::TRANSPORT_REFUSED
        );
    }
}
