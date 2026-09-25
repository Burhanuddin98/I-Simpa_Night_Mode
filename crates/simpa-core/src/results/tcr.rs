//! What TCR wrote, typed (`docs/results.md`, "TCR").
//!
//! TCR writes no time series, so `core::params` has nothing to read from it; its own values are
//! kept as it computed them:
//! - `Main results.gabe` (`ctr/input_output/reportmanager.cpp:154-216`): per band row, the
//!   equivalent absorption area, reverberation time and reverberant level of Sabine and of
//!   Eyring, then a `Global` row. The areas and times of `Global` are NaN by design (line 208), so
//!   only its levels are read.
//! - `Punctual receivers/<lbl>.gabe` (`reportmanager.cpp:104-150`): per band, the direct level and
//!   the total level with Sabine's and with Eyring's reverberant field, dB; and a `Global` row,
//!   their energetic sum. The folder is fixed; `receiversp_directory` is ignored
//!   (`main_tc.cpp:139-144`).
//! - every surface-receiver and cutting-plane `.csbin`, under each of the three fields.
//!
//! Point receivers are enumerated by the `.gabe` files in the folder, each file's name without
//! `.gabe` matched exactly to one `config.xml` label.
//!
//! [`analytic`] computes `core::params`' Sabine and Eyring times from the run's own inputs, the
//! way TCR takes them (`TC_CalculationCore.cpp:87-141, 199-209`): every `.cbin` face with its
//! material's absorption in the band, the `.mbin`'s volume, and the air term from
//! `config.xml`. It is what gate M7(d) and M8 compare TCR with.

use std::path::Path;

use roxmltree::Document;
use schemars::JsonSchema;
use serde::Serialize;

use super::{Refusal, SurfaceFile, file_invalid, key, read_surfaces, value_invalid};
use crate::formats::gabe::{self, Gabe};
use crate::formats::{cbin, mbin};
use crate::params::ParamError;
use crate::params::air::{self, Atmosphere};
use crate::params::room::{self, RtConstant, Surface};
use crate::run::expect::{self, Expectation, fixed};
use crate::run::locate;

/// One theory's values in one band of `Main results.gabe`.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, JsonSchema)]
pub struct Theory {
    /// `A_*`: the equivalent absorption area, m².
    pub absorption_area_m2: f64,
    /// `TR_*`: the reverberation time, s.
    pub reverberation_time_s: f64,
    /// `L_*`: the reverberant field's level, dB.
    pub level_db: f64,
}

/// One band row of `Main results.gabe`.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, JsonSchema)]
pub struct MainBand {
    pub freq_hz: i32,
    pub sabine: Theory,
    pub eyring: Theory,
}

/// One band of a TCR point receiver, dB.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, JsonSchema)]
pub struct ReceiverBand {
    pub freq_hz: i32,
    pub direct_db: f64,
    pub total_sabine_db: f64,
    pub total_eyring_db: f64,
}

/// A TCR point receiver: its file and its values.
#[derive(Clone, Debug, PartialEq, Serialize, JsonSchema)]
pub struct PointReceiver {
    /// The file's name without `.gabe`: exactly one `recepteur_ponctuel@lbl`.
    pub label: String,
    /// Relative to `solve/`.
    pub file: String,
    pub bands: Vec<ReceiverBand>,
    /// The `Global` row: each column's energetic sum over the bands, an aggregate. A value that
    /// is not finite is refused, `results_value_invalid`.
    pub global_direct_db: f64,
    pub global_total_sabine_db: f64,
    pub global_total_eyring_db: f64,
}

/// `core::params`' reverberation times for one band, on the run's inputs.
#[derive(Clone, Debug, PartialEq)]
pub struct AnalyticBand {
    pub freq_hz: i32,
    /// The energy attenuation `m` TCR adds as `4·m·V`, 1/m; `None` when `abs_atmo_calc` is off.
    pub air_m_per_metre: Option<f64>,
    /// `params::room::sabine_rt` with TCR's constant.
    pub sabine_s: Result<f64, ParamError>,
    /// `params::room::eyring_rt` with TCR's constant.
    pub eyring_s: Result<f64, ParamError>,
}

/// The analytic references on the run's inputs, or why there are none.
#[derive(Clone, Debug, PartialEq)]
pub enum Analytic {
    Computed {
        /// The `.mbin`'s volume, the sum of its tetrahedra's (`TC_CalculationCore.cpp:199-209`).
        volume_m3: f64,
        /// The `.cbin` faces' total area.
        area_m2: f64,
        bands: Vec<AnalyticBand>,
    },
    /// The inputs could not be read, or hold what TCR's rules here do not cover.
    NotComputed { why: String },
}

/// The per-band absorption of each material id, in band order: the solver sorts a material's
/// `bfreq` children by `freq` read as an integer (`OrderChildsByProperty`, `cxml.cpp:130-146`)
/// and maps them to the sorted `freq_enum` by position (`base_core_configuration.cpp:201-221`),
/// so the order they are written in does not matter.
pub(crate) fn materials(doc: &Document) -> Result<Vec<(u32, Vec<f64>)>, String> {
    let root = doc.root_element();
    let Some(list) = expect::child(root, "surface_absorption_enum") else {
        return Err("config.xml has no surface_absorption_enum".into());
    };
    let mut out = Vec::new();
    for m in expect::items(list) {
        let id = expect::atoi(m.attribute("id").unwrap_or(""));
        let mut alphas = Vec::new();
        for b in expect::items(m) {
            let text = b.attribute("absorb").unwrap_or("");
            let freq = expect::atoi(b.attribute("freq").unwrap_or(""));
            match locate::to_float(text) {
                Some(a) if a.is_finite() => alphas.push((freq, f64::from(a))),
                _ => return Err(format!("material {id}: absorb {text:?} is not a number")),
            }
        }
        alphas.sort_by_key(|a| a.0);
        out.push((
            u32::try_from(id).map_err(|_| format!("material id {id}"))?,
            alphas.into_iter().map(|a| a.1).collect(),
        ));
    }
    Ok(out)
}

/// The surfaces of band `index` (among all of config.xml's bands, ascending): each face's area
/// with its material's absorption at that position.
pub(crate) fn band_surfaces(
    faces: &[(f64, u32)],
    mats: &[(u32, Vec<f64>)],
    index: usize,
) -> Result<Vec<Surface>, String> {
    faces
        .iter()
        .map(|&(area_m2, id_mat)| {
            let alphas = mats
                .iter()
                .find(|(id, _)| *id == id_mat)
                .map(|(_, a)| a)
                .ok_or_else(|| format!("a face's material {id_mat} is not declared"))?;
            let absorption = *alphas
                .get(index)
                .ok_or_else(|| format!("material {id_mat} has no band {index}"))?;
            Ok(Surface {
                area_m2,
                absorption,
            })
        })
        .collect()
}

/// `core::params`' Sabine and Eyring times on the run's own inputs ([module docs](self)).
pub fn analytic(solve: &Path, exp: &Expectation) -> Analytic {
    analytic_inner(solve, exp).unwrap_or_else(|why| Analytic::NotComputed { why })
}

fn analytic_inner(solve: &Path, exp: &Expectation) -> Result<Analytic, String> {
    let room = RoomInputs::read(solve, exp)?;
    let mut bands = Vec::new();
    for band in &room.bands {
        let surfaces = band_surfaces(&room.faces, &room.materials, band.index)?;
        let air_m_per_metre = band.air_m_per_metre.clone()?;
        let volume_m3 = room.volume_m3;
        bands.push(AnalyticBand {
            freq_hz: band.freq_hz,
            air_m_per_metre,
            sabine_s: room::sabine_rt(volume_m3, &surfaces, air_m_per_metre, RtConstant::Tcr),
            eyring_s: room::eyring_rt(volume_m3, &surfaces, air_m_per_metre, RtConstant::Tcr),
        });
    }
    Ok(Analytic::Computed {
        volume_m3: room.volume_m3,
        area_m2: room.faces.iter().map(|f| f.0).sum(),
        bands,
    })
}

/// One computed band of [`RoomInputs`].
#[derive(Clone, Debug)]
pub(crate) struct RoomBand {
    /// Its position among all of config.xml's bands, ascending: the index of its absorption.
    pub index: usize,
    pub freq_hz: i32,
    /// The energy attenuation `m` the solver applies, 1/m: `None` with `abs_atmo_calc` off, the
    /// user's `absatmo` when the computation is disabled, else the solver's value at the nominal
    /// frequency ([`solver_air`]); why not, when it cannot be read.
    pub air_m_per_metre: Result<Option<f64>, String>,
}

/// The room as a run's own inputs give it, the way TCR takes it (`TC_CalculationCore.cpp:87-141,
/// 199-209`): every `.cbin` face with its material, the `.mbin`'s volume, the materials' absorption
/// per band and the air term from `config.xml`. [`analytic`] and `results::reference` read it.
#[derive(Clone, Debug)]
pub(crate) struct RoomInputs {
    /// The `.cbin`'s faces, in its order, as triangles in metres.
    pub triangles: Vec<[[f64; 3]; 3]>,
    /// Each face's area, m², and material id, in the same order.
    pub faces: Vec<(f64, u32)>,
    /// The `.mbin`'s volume, the sum of its tetrahedra's, m³.
    pub volume_m3: f64,
    /// The `.mbin`'s tetrahedra, in metres: they fill the meshed room.
    pub tetrahedra: Vec<[[f64; 3]; 4]>,
    /// [`materials`]: each material's absorption per band, in frequency order.
    pub materials: Vec<(u32, Vec<f64>)>,
    /// The computed bands, ascending.
    pub bands: Vec<RoomBand>,
    /// `config.xml`'s text, for what else a caller reads from it.
    pub config: String,
}

impl RoomInputs {
    /// Refused, with why, for a scene with fitting faces (whose rule TCR applies per face and
    /// this does not emulate), a file that does not read, or an index out of range.
    pub(crate) fn read(solve: &Path, exp: &Expectation) -> Result<RoomInputs, String> {
        let scene_rel = key(&exp.names.model_name);
        let scene =
            cbin::read_file(&solve.join(&scene_rel)).map_err(|e| format!("{scene_rel}: {e}"))?;
        if scene.faces.iter().any(|f| f.id_en >= 0) {
            return Err(
                "the scene has fitting faces (idEn), which TCR leaves out or keeps per face by \
                 its material's transmission (TC_CalculationCore.cpp:11-17); that rule is not \
                 emulated"
                    .into(),
            );
        }
        let mesh_rel = key(&exp.names.tetramesh_file_name);
        let mesh =
            mbin::read_file(&solve.join(&mesh_rel)).map_err(|e| format!("{mesh_rel}: {e}"))?;
        let node = |i: i32| -> Result<[f64; 3], String> {
            let n = usize::try_from(i)
                .ok()
                .and_then(|i| mesh.nodes.get(i))
                .ok_or_else(|| format!("{mesh_rel}: node {i} out of range"))?;
            Ok(n.map(f64::from))
        };
        let mut volume_m3 = 0.0;
        let mut tetrahedra = Vec::with_capacity(mesh.tetrahedra.len());
        for t in &mesh.tetrahedra {
            let [a, b, c, d] = [
                node(t.vertices[0])?,
                node(t.vertices[1])?,
                node(t.vertices[2])?,
                node(t.vertices[3])?,
            ];
            let (u, v, w) = (sub(a, d), sub(b, d), sub(c, d));
            volume_m3 += dot(u, cross(v, w)).abs() / 6.0;
            tetrahedra.push([a, b, c, d]);
        }
        let text = std::fs::read(solve.join(crate::config_xml::names::CONFIG))
            .map_err(|e| format!("config.xml: {e}"))?;
        let text = text.strip_prefix(b"\xef\xbb\xbf").unwrap_or(&text);
        let text = std::str::from_utf8(text).map_err(|e| format!("config.xml: {e}"))?;
        let doc = Document::parse(text).map_err(|e| format!("config.xml: {e}"))?;
        let materials = materials(&doc)?;
        let root = doc.root_element();
        let sim = expect::child(root, "simulation");
        let atmo = expect::child(root, "condition_atmospherique");
        let attr = |n: Option<roxmltree::Node>, name: &str| -> String {
            n.and_then(|n| n.attribute(name)).unwrap_or("").to_string()
        };
        let real = |n: Option<roxmltree::Node>, name: &str| -> Result<f64, String> {
            let t = attr(n, name);
            locate::to_float(&t)
                .filter(|v| v.is_finite())
                .map(f64::from)
                .ok_or_else(|| format!("config.xml: {name} {t:?} is not a number"))
        };
        let air_on = expect::atoi(&attr(sim, "abs_atmo_calc")) != 0;
        let user_air = expect::atoi(&attr(atmo, "disable_absatmo_computation")) == 1;
        let atmosphere = Atmosphere {
            temperature_c: real(atmo, "temperature")?,
            relative_humidity_percent: real(atmo, "humidite")?,
            pressure_pa: real(atmo, "pression")?,
        };

        let mut triangles = Vec::with_capacity(scene.faces.len());
        let mut faces = Vec::with_capacity(scene.faces.len());
        for f in &scene.faces {
            let p = |i: u32| {
                scene
                    .vertices
                    .get(i as usize)
                    .map(|v| [f64::from(v.x), f64::from(v.y), f64::from(v.z)])
                    .ok_or_else(|| format!("{scene_rel}: vertex {i} out of range"))
            };
            let (a, b, c) = (p(f.a)?, p(f.b)?, p(f.c)?);
            let n = cross(sub(b, a), sub(c, a));
            triangles.push([a, b, c]);
            faces.push((0.5 * dot(n, n).sqrt(), f.id_mat));
        }
        let bands = exp
            .bands
            .iter()
            .enumerate()
            .filter(|(_, b)| b.requested)
            .map(|(index, band)| {
                let air = if !air_on {
                    Ok(None)
                } else if user_air {
                    real(atmo, "absatmo").map(Some)
                } else {
                    solver_air(f64::from(band.freq_hz), &atmosphere)
                        .map(Some)
                        .map_err(|e| e.to_string())
                };
                let air = match crate::faults::active() {
                    Some(crate::faults::Fault::AirTermDropped) => air.map(|_| None),
                    _ => air,
                };
                RoomBand {
                    index,
                    freq_hz: band.freq_hz,
                    air_m_per_metre: air,
                }
            })
            .collect();
        Ok(RoomInputs {
            triangles,
            faces,
            volume_m3,
            tetrahedra,
            materials,
            bands,
            config: text.to_string(),
        })
    }
}

/// The air term TCR adds for a band at nominal frequency `nominal_hz`: the solver's `m`
/// ([`air::solver_air_absorption_per_m`]). In a test build,
/// [`crate::faults::Fault::AirIsoExactMidband`] takes ISO 9613-1's own at the exact midband
/// frequency instead, gate M7(d)'s say-NO for the air term.
fn solver_air(nominal_hz: f64, atmosphere: &Atmosphere) -> Result<f64, ParamError> {
    if let Some(crate::faults::Fault::AirIsoExactMidband) = crate::faults::active() {
        let f = air::exact_midband_hz(nominal_hz).unwrap_or(nominal_hz);
        return Ok(air::energy_attenuation_per_m(air::attenuation_db_per_m(
            f, atmosphere,
        )?));
    }
    air::solver_air_absorption_per_m(nominal_hz, atmosphere)
}

fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

/// Everything read from a TCR run.
#[derive(Clone, Debug)]
pub struct TcrResults {
    /// One per computed band, ascending.
    pub bands: Vec<MainBand>,
    /// `Global` row levels: the energetic sum of the band levels, an aggregate.
    pub global_sabine_level_db: f64,
    pub global_eyring_level_db: f64,
    /// In the order of their file names.
    pub point_receivers: Vec<PointReceiver>,
    pub surfaces: Vec<SurfaceFile>,
    /// [`analytic`] on the run's inputs.
    pub analytic: Analytic,
}

impl TcrResults {
    /// The receiver whose file is named `<label>.gabe` exactly.
    pub fn point_receiver(&self, label: &str) -> Option<&PointReceiver> {
        self.point_receivers.iter().find(|r| r.label == label)
    }
}

/// `(column name, row label) -> value`, refusing a missing column or row, or a value that is not
/// finite.
fn cell(g: &Gabe, rel: &str, column: &str, row: &str) -> Result<f64, Refusal> {
    let v = optional_cell(g, rel, column, row)?;
    if !v.is_finite() {
        return Err(value_invalid(rel, format!("{column} at {row} is {v}")));
    }
    Ok(v)
}

fn optional_cell(g: &Gabe, rel: &str, column: &str, row: &str) -> Result<f64, Refusal> {
    let i = g
        .row_index(row.as_bytes())
        .ok_or_else(|| file_invalid(rel, format!("no row labelled {row:?}")))?;
    let col = g
        .column(column.as_bytes())
        .and_then(|c| c.floats())
        .ok_or_else(|| file_invalid(rel, format!("no float column {column:?}")))?;
    col.get(i)
        .map(|&v| f64::from(v))
        .ok_or_else(|| file_invalid(rel, format!("{column:?} has no row {i} ({row})")))
}

fn read_table(solve: &Path, rel: &str) -> Result<Gabe, Refusal> {
    gabe::read_file(&solve.join(rel)).map_err(|e| file_invalid(rel, e))
}

/// The `.gabe` files of the point-receiver folder, sorted by name: exactly the config's labels.
fn receiver_files(solve: &Path, labels: &[String]) -> Result<Vec<String>, Refusal> {
    let dir = fixed::TCR_POINT_RECEIVER_DIR;
    let mut sorted = labels.to_vec();
    sorted.sort();
    if let Some(w) = sorted.windows(2).find(|w| w[0] == w[1]) {
        return Err(file_invalid(
            crate::config_xml::names::CONFIG,
            format!("two point receivers are labelled {:?}", w[0]),
        ));
    }
    if labels.is_empty() {
        return Ok(Vec::new());
    }
    let mut names = Vec::new();
    for e in std::fs::read_dir(solve.join(dir)).map_err(|e| file_invalid(dir, e))? {
        let e = e.map_err(|e| file_invalid(dir, e))?;
        if !e.file_type().map_err(|e| file_invalid(dir, e))?.is_file() {
            continue;
        }
        let name = e
            .file_name()
            .into_string()
            .map_err(|n| file_invalid(dir, format!("a file name is not Unicode: {n:?}")))?;
        match name.strip_suffix(".gabe") {
            Some(stem) => names.push(stem.to_string()),
            None => {
                return Err(file_invalid(
                    dir,
                    format!("{name:?} is not a receiver table"),
                ));
            }
        }
    }
    names.sort();
    let extra: Vec<&String> = names.iter().filter(|n| !labels.contains(n)).collect();
    let missing: Vec<&String> = labels.iter().filter(|l| !names.contains(l)).collect();
    if !extra.is_empty() || !missing.is_empty() {
        return Err(file_invalid(
            dir,
            format!(
                "its receiver tables are {names:?}; config.xml's labels are {labels:?} (no label \
                 for {extra:?}, no table for {missing:?})"
            ),
        ));
    }
    Ok(names)
}

pub(crate) fn read(solve: &Path, exp: &Expectation) -> Result<TcrResults, Refusal> {
    let bands_hz = exp.requested_bands();
    let main_rel = key(fixed::TCR_MAIN_RESULTS);
    let main = read_table(solve, &main_rel)?;
    let mut bands = Vec::with_capacity(bands_hz.len());
    for &f in &bands_hz {
        let row = format!("{f} Hz");
        let theory = |t: &str| -> Result<Theory, Refusal> {
            Ok(Theory {
                absorption_area_m2: cell(&main, &main_rel, &format!("A_{t}"), &row)?,
                reverberation_time_s: cell(&main, &main_rel, &format!("TR_{t}"), &row)?,
                level_db: cell(&main, &main_rel, &format!("L_{t}"), &row)?,
            })
        };
        bands.push(MainBand {
            freq_hz: f,
            sabine: theory("Sabine")?,
            eyring: theory("Eyring")?,
        });
    }
    let global_sabine_level_db = cell(&main, &main_rel, "L_Sabine", fixed::GLOBAL)?;
    let global_eyring_level_db = cell(&main, &main_rel, "L_Eyring", fixed::GLOBAL)?;

    let labels = exp.point_receivers.clone();
    let mut point_receivers = Vec::new();
    for label in receiver_files(solve, &labels)? {
        let rel = key(&format!("{}/{label}.gabe", fixed::TCR_POINT_RECEIVER_DIR));
        let g = read_table(solve, &rel)?;
        let mut rows = Vec::with_capacity(bands_hz.len());
        for &f in &bands_hz {
            let row = format!("{f} Hz");
            rows.push(ReceiverBand {
                freq_hz: f,
                direct_db: cell(&g, &rel, "Direct", &row)?,
                total_sabine_db: cell(&g, &rel, "Total (Sabine)", &row)?,
                total_eyring_db: cell(&g, &rel, "Total (Eyring)", &row)?,
            });
        }
        // The Global row is each column's energetic sum over the bands
        // (`ctr/input_output/reportmanager.cpp:131-143`): finite whenever the band rows are, so a
        // value that is not is refused like a band row's.
        point_receivers.push(PointReceiver {
            file: rel.clone(),
            label,
            bands: rows,
            global_direct_db: cell(&g, &rel, "Direct", fixed::GLOBAL)?,
            global_total_sabine_db: cell(&g, &rel, "Total (Sabine)", fixed::GLOBAL)?,
            global_total_eyring_db: cell(&g, &rel, "Total (Eyring)", fixed::GLOBAL)?,
        });
    }
    let n = &exp.names;
    let fields = [
        n.direct_prefix.clone(),
        n.sabine_prefix.clone(),
        n.eyring_prefix.clone(),
    ];
    let surfaces = read_surfaces(solve, &exp.surface_tables(), exp, &fields)?;
    Ok(TcrResults {
        bands,
        global_sabine_level_db,
        global_eyring_level_db,
        point_receivers,
        surfaces,
        analytic: analytic(solve, exp),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Material 1 absorbs differently in every band and is written out of order; material 2 is
    /// flat. Faces of unequal area carry them, so taking the wrong band, the file's order, or the
    /// other face's material each changes the absorption area.
    const CONFIG: &str = r#"<configuration>
  <surface_absorption_enum>
    <type_surface id="1">
      <bfreq freq="500" absorb="0.3"/>
      <bfreq freq="1000" absorb="0.5"/>
      <bfreq freq="125" absorb="0.1"/>
      <bfreq freq="250" absorb="0.2"/>
    </type_surface>
    <type_surface id="2">
      <bfreq freq="125" absorb="0.05"/>
      <bfreq freq="250" absorb="0.05"/>
      <bfreq freq="500" absorb="0.05"/>
      <bfreq freq="1000" absorb="0.05"/>
    </type_surface>
  </surface_absorption_enum>
</configuration>"#;

    #[test]
    fn each_band_takes_its_own_absorption_in_frequency_order() {
        let doc = Document::parse(CONFIG).unwrap();
        let mats = materials(&doc).unwrap();
        let f32s = |v: [f32; 4]| v.map(f64::from).to_vec();
        assert_eq!(mats[0], (1, f32s([0.1, 0.2, 0.3, 0.5])));
        let faces = [(10.0, 1), (30.0, 2)];
        // Band i of 125, 250, 500, 1000 Hz: A = 10·α₁(i) + 30·0.05.
        for (i, want) in [2.5, 3.5, 4.5, 6.5].into_iter().enumerate() {
            let s = band_surfaces(&faces, &mats, i).unwrap();
            let a: f64 = s.iter().map(|s| s.area_m2 * s.absorption).sum();
            assert!((a - want).abs() < 1e-6, "band {i}: A = {a}, want {want}");
        }
        // Says no: a material missing from the declarations, or a band it does not have.
        assert!(band_surfaces(&[(1.0, 3)], &mats, 0).is_err());
        assert!(band_surfaces(&faces, &mats, 4).is_err());
    }
}
