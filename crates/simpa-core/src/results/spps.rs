//! What SPPS wrote, typed (`docs/results.md`, "SPPS").
//!
//! Point receivers are what upstream's GUI reads for them (`isimpa/data_manager/tree_rapport/
//! e_report_file.cpp:296-321` maps each extension to its report class):
//! - `<receiversp_filename>` (`.recp`, `e_report_gabe_recp.cpp`): one float column per computed
//!   band, one row per time step, each value `energy_sum × c·ρ / V_receiver`
//!   (`baseReportManager.cpp:183`; `sppsInitialisation.cpp:86`), which upstream turns into a
//!   level with `10·lg(value / p₀²)` (`e_report_gabe_recp.cpp:56, 145`): a mean-square pressure
//!   contribution in Pa². This is the series `core::params` reads.
//! - `<receiversp_filename_adv>` (`.gap`, `e_report_gabe_gap.cpp`): a layout index, the time
//!   step, the sources' power times `ρ·c` per band, the bands, the receiver's background noise,
//!   then per band the energy again and two lateral series (`spps/reportmanager.cpp:760-876`).
//!   **Column +1 holds `E·cos²φ` and column +2 `E·|cos φ|`** (`reportmanager.cpp:226-227,
//!   388-389, 412-413, 862, 867`), although the variables are named the other way round and
//!   upstream's viewer labels +1 `E*cos theta` (`e_report_gabe_gap.cpp:80-81`). Its energy
//!   columns are written from the same product as the `.recp`'s, so they must be equal bit for
//!   bit, and its time step must be `pasdetemps`.
//! - `Sound level per source.recps` (`e_report_gabe_recps.cpp`): each source's total per band
//!   (`reportmanager.cpp:398-410`).
//! - `Punctual receiver intensity.gabe`: the intensity vector per band and step, then a `Sum`
//!   row (`reportmanager.cpp:373-397`).
//!
//! Besides them: `<cumul_filename>` (the energy in the room per band and step), the statistics
//! (already typed by the verdict), every surface-receiver and cutting-plane `.csbin`, the
//! particle files when saved, and the per-band `Intensity.rpi`, which is decoded to check it
//! reads and not kept.

use std::path::Path;

use roxmltree::Document;
use schemars::JsonSchema;
use serde::Serialize;

use super::{Refusal, SurfaceFile, band_of, file_invalid, key, read_surfaces, value_invalid};
use crate::formats::gabe::{self, Gabe};
use crate::formats::pbin;
use crate::run::expect::{self, Expectation, fixed};
use crate::run::locate;
use crate::run::stats::ParticleStats;

/// The per-step speed of sound SPPS derives from the temperature: `343.2·√(T/293.15)` in `f64`,
/// stored as `f32` (`base_core_configuration.cpp:64`; `Celerite_du_son.cpp:46`).
pub fn solver_speed_of_sound(temperature_c: f32) -> f32 {
    (343.2 * ((f64::from(temperature_c) + 273.15) / 293.15).sqrt()) as f32
}

/// When a source's particles start: step `(uentier_court)ceil(delay / dt)`, the division in `f32`
/// (`sppsNantes.cpp:91`), times `dt`.
pub fn emission_s(delay_s: f32, dt: f32) -> f64 {
    let step = f64::from(delay_s / dt)
        .ceil()
        .clamp(0.0, f64::from(u16::MAX)) as u16;
    f64::from(step) * f64::from(dt)
}

/// A source as SPPS reads it.
#[derive(Clone, Debug, PartialEq, Serialize, JsonSchema)]
pub struct SourcePoint {
    /// `source@name`.
    pub name: String,
    /// The position SPPS stores (`f32`); `None` when a coordinate is not one SPPS's reading is
    /// emulated for (`run::locate::to_float`).
    pub position_m: Option<[f64; 3]>,
    /// When its particles start: `ceil(delay / dt)` steps, cast to `u16`
    /// (`sppsNantes.cpp:91`), times `dt`.
    pub emission_s: f64,
}

/// One band of one point receiver.
#[derive(Clone, Debug, PartialEq)]
pub struct ReceiverBand {
    pub freq_hz: i32,
    /// The `.recp` column: Pa² per time step.
    pub energy: Vec<f64>,
    /// The `.gap`'s `E·cos²φ` column (+1).
    pub lateral_cos2: Vec<f64>,
    /// The `.gap`'s `E·|cos φ|` column (+2).
    pub lateral_abs_cos: Vec<f64>,
    /// The intensity vector per step, x, y and z (the `Sum` row left out).
    pub intensity: [Vec<f64>; 3],
    /// The `.gap`'s column 2: the sources' power in this band times `ρ·c`
    /// (`reportmanager.cpp:807-816`).
    pub source_power_rho_c: f64,
    /// The `.gap`'s column 4: the receiver's background noise in this band, dB.
    pub background_noise_db: f64,
}

/// One source's total at a receiver, per band (`.recps`).
#[derive(Clone, Debug, PartialEq, Serialize, JsonSchema)]
pub struct SourceTotals {
    pub source: String,
    /// Pa², one per band.
    pub energy: Vec<f64>,
}

/// A point receiver: its folder and what is in it.
#[derive(Clone, Debug, PartialEq)]
pub struct PointReceiver {
    /// The folder's name, which is exactly one `recepteur_ponctuel@lbl`.
    pub label: String,
    /// The folder, relative to `solve/`.
    pub folder: String,
    /// The position SPPS stores (`f32`); `None` as for [`SourcePoint::position_m`].
    pub position_m: Option<[f64; 3]>,
    /// One per computed band, ascending.
    pub bands: Vec<ReceiverBand>,
    /// One per source, in `config.xml`'s order.
    pub by_source: Vec<SourceTotals>,
}

/// One band's series of a whole-room table (`<cumul_filename>`).
#[derive(Clone, Debug, PartialEq, Serialize, JsonSchema)]
pub struct BandEnergy {
    pub freq_hz: i32,
    pub energy: Vec<f64>,
}

/// A saved particle file (`nbparticules_rendu` > 0), decoded and summarised.
#[derive(Clone, Debug, PartialEq, Serialize, JsonSchema)]
pub struct ParticleFileSummary {
    pub path: String,
    pub freq_hz: i32,
    pub particles: usize,
    pub recorded_steps: usize,
}

/// Everything read from an SPPS run.
#[derive(Clone, Debug)]
pub struct SppsResults {
    /// `pasdetemps` as SPPS stores it (`f32`), s.
    pub time_step_s: f64,
    /// `duree_simulation` as SPPS stores it, s.
    pub duration_s: f64,
    /// Rows of every per-step table.
    pub steps: usize,
    /// [`solver_speed_of_sound`] at the run's temperature, m/s.
    pub speed_of_sound_m_s: f64,
    /// `rayon_recepteurp`, m.
    pub receiver_radius_m: f64,
    /// `alog` or `blin` is not 0: the speed of sound varies with height, so a straight-line
    /// arrival time is not computed.
    pub celerity_gradient: bool,
    /// `computation_method`: 0 random, 1 energetic.
    pub computation_method: i32,
    pub sources: Vec<SourcePoint>,
    /// In the order of their folder names.
    pub point_receivers: Vec<PointReceiver>,
    /// `<cumul_filename>`, one per band.
    pub total_energy: Vec<BandEnergy>,
    pub particles: ParticleStats,
    pub surfaces: Vec<SurfaceFile>,
    pub particle_files: Vec<ParticleFileSummary>,
}

impl SppsResults {
    /// When the direct sound reaches `r`'s centre: the earliest over the sources of emission plus
    /// straight-line distance over [`SppsResults::speed_of_sound_m_s`]. `None` with a celerity
    /// gradient, or when a position is not known.
    pub fn arrival_s(&self, r: &PointReceiver) -> Option<f64> {
        if self.celerity_gradient {
            return None;
        }
        let p = r.position_m?;
        let mut best: Option<f64> = None;
        for s in &self.sources {
            let q = s.position_m?;
            let d = ((p[0] - q[0]).powi(2) + (p[1] - q[1]).powi(2) + (p[2] - q[2]).powi(2)).sqrt();
            let t = s.emission_s + d / self.speed_of_sound_m_s;
            best = Some(best.map_or(t, |b: f64| b.min(t)));
        }
        best
    }

    /// How long a particle takes to cross a receiver sphere: `2R/c`. The direct sound is spread
    /// over that time.
    pub fn receiver_crossing_s(&self) -> f64 {
        2.0 * self.receiver_radius_m / self.speed_of_sound_m_s
    }

    /// The receiver whose folder is named `label` exactly.
    pub fn point_receiver(&self, label: &str) -> Option<&PointReceiver> {
        self.point_receivers.iter().find(|r| r.label == label)
    }

    /// Whether SPPS's own statistics show that no energy arrives after the series' end in band
    /// `freq_hz`: random mode, and no particle remaining at the end of the calculation. A particle
    /// is counted as remaining only when the time steps run out while it is alive
    /// (`spps/CalculationCore.cpp:49, 88-92`), and in random mode a particle's energy never
    /// dwindles, it is absorbed whole (`CalculationCore.cpp:62-67, 288-300`), so with none
    /// remaining the histogram holds every particle's whole path. Energetic mode drops a particle
    /// once its energy falls below `10^-trans_epsilon` of its start (`sppsNantes.cpp:75`;
    /// `CalculationCore.cpp:305`), energy the histogram never holds, so it is never complete here
    /// and `params` bounds its tail instead.
    pub fn band_complete(&self, freq_hz: i32) -> bool {
        self.computation_method == 0
            && self
                .particles
                .bands
                .iter()
                .find(|b| b.freq_hz == freq_hz)
                .is_some_and(|b| b.remaining == 0)
    }
}

const CONFIG: &str = crate::config_xml::names::CONFIG;
const BY_SOURCE: &str = fixed::POINT_RECEIVER_BY_SOURCE;
const INTENSITY: &str = fixed::POINT_RECEIVER_INTENSITY;

fn position(e: roxmltree::Node) -> Option<[f64; 3]> {
    let [x, y, z] = ["x", "y", "z"].map(|a| locate::to_float(e.attribute(a).unwrap_or("")));
    Some([f64::from(x?), f64::from(y?), f64::from(z?)])
}

/// A real attribute as SPPS reads it, which must be finite.
fn real(node: Option<roxmltree::Node>, element: &str, name: &str) -> Result<f32, Refusal> {
    let text = node.and_then(|n| n.attribute(name)).unwrap_or("");
    match locate::to_float(text) {
        Some(v) if v.is_finite() => Ok(v),
        _ => Err(file_invalid(
            CONFIG,
            format!("{element}@{name} is {text:?}, not a number SPPS reads"),
        )),
    }
}

/// Reads a GABE table under `solve`, refused when it does not decode.
fn table(solve: &Path, rel: &str) -> Result<Gabe, Refusal> {
    gabe::read_file(&solve.join(rel)).map_err(|e| file_invalid(rel, e))
}

fn floats<'a>(g: &'a Gabe, rel: &str, i: usize) -> Result<&'a [f32], Refusal> {
    g.columns
        .get(i)
        .and_then(|c| c.floats())
        .ok_or_else(|| file_invalid(rel, format!("column {i} is missing or not a float column")))
}

fn ints<'a>(g: &'a Gabe, rel: &str, i: usize) -> Result<&'a [i32], Refusal> {
    g.columns.get(i).and_then(|c| c.ints()).ok_or_else(|| {
        file_invalid(
            rel,
            format!("column {i} is missing or not an integer column"),
        )
    })
}

/// Energies: finite and not negative.
fn energies(rel: &str, what: &str, v: &[f32]) -> Result<Vec<f64>, Refusal> {
    if let Some((i, x)) = v
        .iter()
        .enumerate()
        .find(|(_, x)| !x.is_finite() || **x < 0.0)
    {
        return Err(value_invalid(
            rel,
            format!("{what} value {i} is {x}, not a finite energy of at least 0"),
        ));
    }
    Ok(v.iter().map(|&x| f64::from(x)).collect())
}

/// Finite values of any sign.
fn finite(rel: &str, what: &str, v: &[f32]) -> Result<Vec<f64>, Refusal> {
    if let Some((i, x)) = v.iter().enumerate().find(|(_, x)| !x.is_finite()) {
        return Err(value_invalid(
            rel,
            format!("{what} value {i} is {x}, not finite"),
        ));
    }
    Ok(v.iter().map(|&x| f64::from(x)).collect())
}

/// A table as SPPS's `.recp` writer lays it out (`baseReportManager.cpp:160-190`): a short-string
/// label column, then one float column per computed band labelled `<f> Hz`, in band order, all
/// of `rows` rows (the label column's when `rows` is `None`). The band columns' energies.
fn band_table(
    g: &Gabe,
    rel: &str,
    bands: &[i32],
    rows: Option<usize>,
) -> Result<Vec<Vec<f64>>, Refusal> {
    let labels = g
        .row_labels()
        .ok_or_else(|| file_invalid(rel, "column 0 is not the short-string row labels"))?;
    let n = rows.unwrap_or(labels.len());
    if labels.len() != n {
        return Err(file_invalid(
            rel,
            format!("{} rows, the run's tables have {n}", labels.len()),
        ));
    }
    let got: Vec<Option<i32>> = g.columns[1..].iter().map(|c| band_of(c.name())).collect();
    let want: Vec<Option<i32>> = bands.iter().map(|&f| Some(f)).collect();
    if got != want {
        return Err(file_invalid(
            rel,
            format!(
                "band columns {:?}, the run computed {bands:?}",
                g.columns[1..]
                    .iter()
                    .map(|c| String::from_utf8_lossy(&c.label).into_owned())
                    .collect::<Vec<_>>()
            ),
        ));
    }
    let mut out = Vec::with_capacity(bands.len());
    for (i, f) in bands.iter().enumerate() {
        let v = floats(g, rel, i + 1)?;
        if v.len() != n {
            return Err(file_invalid(
                rel,
                format!("the {f} Hz column has {} rows, not {n}", v.len()),
            ));
        }
        out.push(energies(rel, &format!("{f} Hz"), v)?);
    }
    Ok(out)
}

/// The `.gap` of one receiver, checked against its `.recp` columns `recp`.
struct Gap {
    source_power_rho_c: Vec<f64>,
    noise_db: Vec<f64>,
    cos2: Vec<Vec<f64>>,
    abs_cos: Vec<Vec<f64>>,
}

fn read_gap(
    g: &Gabe,
    rel: &str,
    bands: &[i32],
    steps: usize,
    dt: f32,
    recp: &[Vec<f64>],
) -> Result<Gap, Refusal> {
    let nb = bands.len();
    let index = ints(g, rel, 0)?;
    let want = [1, 2, 3, 4, 5, nb as i32, 3, steps as i32];
    if index != want {
        return Err(file_invalid(
            rel,
            format!("its index column is {index:?}, SPPS writes {want:?} for this run"),
        ));
    }
    if g.columns.len() != 5 + 3 * nb {
        return Err(file_invalid(
            rel,
            format!(
                "{} columns, not 5 + 3 per band = {}",
                g.columns.len(),
                5 + 3 * nb
            ),
        ));
    }
    let times = floats(g, rel, 1)?;
    if times.first().map(|t| t.to_bits()) != Some(dt.to_bits()) {
        return Err(file_invalid(
            rel,
            format!(
                "its time step is {:?}, config.xml's pasdetemps is {dt}",
                times.first()
            ),
        ));
    }
    let freqs = ints(g, rel, 3)?;
    if freqs != bands {
        return Err(file_invalid(
            rel,
            format!("its bands are {freqs:?}, the run computed {bands:?}"),
        ));
    }
    let power = floats(g, rel, 2)?;
    let noise = floats(g, rel, 4)?;
    if power.len() != nb || noise.len() != nb {
        return Err(file_invalid(
            rel,
            "its source-power or noise column does not have one row per band",
        ));
    }
    let mut gap = Gap {
        source_power_rho_c: energies(rel, "source power", power)?,
        noise_db: finite(rel, "background noise", noise)?,
        cos2: Vec::with_capacity(nb),
        abs_cos: Vec::with_capacity(nb),
    };
    for (i, f) in bands.iter().enumerate() {
        let col = 5 + 3 * i;
        let e = floats(g, rel, col)?;
        let same = e.len() == steps
            && e.iter()
                .zip(&recp[i])
                .all(|(a, b)| f64::from(*a).to_bits() == b.to_bits());
        if !same {
            return Err(file_invalid(
                rel,
                format!(
                    "its {f} Hz energy column differs from the .recp's, which SPPS writes from the same values"
                ),
            ));
        }
        for (k, out) in [(1, &mut gap.cos2), (2, &mut gap.abs_cos)] {
            let v = floats(g, rel, col + k)?;
            if v.len() != steps {
                return Err(file_invalid(
                    rel,
                    format!(
                        "its {f} Hz lateral column +{k} has {} rows, not {steps}",
                        v.len()
                    ),
                ));
            }
            out.push(energies(rel, &format!("{f} Hz lateral +{k}"), v)?);
        }
    }
    Ok(gap)
}

/// The intensity table: per band x, y and z, `steps` rows and a `Sum` row.
fn read_intensity(
    g: &Gabe,
    rel: &str,
    bands: &[i32],
    steps: usize,
) -> Result<Vec<[Vec<f64>; 3]>, Refusal> {
    let labels = g
        .row_labels()
        .ok_or_else(|| file_invalid(rel, "column 0 is not the short-string row labels"))?;
    if labels.len() != steps + 1 {
        return Err(file_invalid(
            rel,
            format!("{} rows, not {steps} steps and a Sum row", labels.len()),
        ));
    }
    if g.columns.len() != 1 + 3 * bands.len() {
        return Err(file_invalid(
            rel,
            format!("{} columns, not 1 + 3 per band", g.columns.len()),
        ));
    }
    let mut out = Vec::with_capacity(bands.len());
    for (i, f) in bands.iter().enumerate() {
        let mut xyz: [Vec<f64>; 3] = Default::default();
        for (k, axis) in ["x", "y", "z"].iter().enumerate() {
            let c = &g.columns[1 + 3 * i + k];
            let want = format!("{f} Hz\n{axis}");
            if c.label != want.as_bytes() {
                return Err(file_invalid(
                    rel,
                    format!(
                        "column {} is labelled {:?}, not {want:?}",
                        1 + 3 * i + k,
                        String::from_utf8_lossy(&c.label)
                    ),
                ));
            }
            let v = floats(g, rel, 1 + 3 * i + k)?;
            if v.len() != steps + 1 {
                return Err(file_invalid(rel, format!("{want:?} has {} rows", v.len())));
            }
            xyz[k] = finite(rel, &want, &v[..steps])?;
        }
        out.push(xyz);
    }
    Ok(out)
}

/// The `.recps`: one row per source, labelled with its name in `config.xml`'s order, one float
/// column per band.
fn read_by_source(
    g: &Gabe,
    rel: &str,
    bands: &[i32],
    sources: &[SourcePoint],
) -> Result<Vec<SourceTotals>, Refusal> {
    let per_band = band_table(g, rel, bands, Some(sources.len()))?;
    let labels = g.row_labels().expect("band_table checked the labels");
    let mut out = Vec::with_capacity(sources.len());
    for (k, (s, l)) in sources.iter().zip(labels).enumerate() {
        if l.as_slice() != s.name.as_bytes() {
            return Err(file_invalid(
                rel,
                format!(
                    "row {k} is {:?}, config.xml's source {k} is {:?}",
                    String::from_utf8_lossy(l),
                    s.name
                ),
            ));
        }
        out.push(SourceTotals {
            source: s.name.clone(),
            energy: per_band.iter().map(|b| b[k]).collect(),
        });
    }
    Ok(out)
}

/// The receiver folders under `dir`, sorted by name: exactly the config's labels, each once.
fn receiver_folders(solve: &Path, dir: &str, labels: &[String]) -> Result<Vec<String>, Refusal> {
    let mut sorted = labels.to_vec();
    sorted.sort();
    if let Some(w) = sorted.windows(2).find(|w| w[0] == w[1]) {
        return Err(file_invalid(
            CONFIG,
            format!("two point receivers are labelled {:?}", w[0]),
        ));
    }
    if labels.is_empty() {
        return Ok(Vec::new());
    }
    let entries = std::fs::read_dir(solve.join(dir)).map_err(|e| file_invalid(dir, e))?;
    let mut names = Vec::new();
    for e in entries {
        let e = e.map_err(|e| file_invalid(dir, e))?;
        if !e.file_type().map_err(|e| file_invalid(dir, e))?.is_dir() {
            continue;
        }
        let name = e
            .file_name()
            .into_string()
            .map_err(|n| file_invalid(dir, format!("a folder name is not Unicode: {n:?}")))?;
        names.push(name);
    }
    names.sort();
    let extra: Vec<&String> = names.iter().filter(|n| !labels.contains(n)).collect();
    let missing: Vec<&String> = labels.iter().filter(|l| !names.contains(l)).collect();
    if !extra.is_empty() || !missing.is_empty() {
        return Err(file_invalid(
            dir,
            format!(
                "its receiver folders are {names:?}; config.xml's labels are {labels:?} \
                 (no label for {extra:?}, no folder for {missing:?})"
            ),
        ));
    }
    Ok(names)
}

pub(crate) fn read(
    solve: &Path,
    exp: &Expectation,
    particles: ParticleStats,
) -> Result<SppsResults, Refusal> {
    let bands = exp.requested_bands();
    let text = std::fs::read(solve.join(CONFIG)).map_err(|e| file_invalid(CONFIG, e))?;
    let text = text.strip_prefix(b"\xef\xbb\xbf").unwrap_or(&text);
    let text = std::str::from_utf8(text).map_err(|e| file_invalid(CONFIG, e))?;
    let doc = Document::parse(text).map_err(|e| file_invalid(CONFIG, e))?;
    let root = doc.root_element();
    let sim = expect::child(root, "simulation");
    let atmo = expect::child(root, "condition_atmospherique");
    let dt = real(sim, "simulation", "pasdetemps")?;
    if dt <= 0.0 {
        return Err(file_invalid(CONFIG, format!("pasdetemps is {dt}")));
    }
    let duration = real(sim, "simulation", "duree_simulation")?;
    let radius = real(sim, "simulation", "rayon_recepteurp")?;
    let temperature = real(atmo, "condition_atmospherique", "temperature")?;
    let gradient = real(atmo, "condition_atmospherique", "alog")? != 0.0
        || real(atmo, "condition_atmospherique", "blin")? != 0.0;
    let c = solver_speed_of_sound(temperature);

    let sources: Vec<SourcePoint> = expect::child(root, "sources")
        .map(|n| {
            expect::items(n)
                .map(|e| {
                    let delay = locate::to_float(e.attribute("delay").unwrap_or("")).unwrap_or(0.0);
                    SourcePoint {
                        name: e.attribute("name").unwrap_or("").to_string(),
                        position_m: position(e),
                        emission_s: emission_s(delay, dt),
                    }
                })
                .collect()
        })
        .unwrap_or_default();
    let config_receivers: Vec<(String, Option<[f64; 3]>)> = expect::child(root, "recepteursp")
        .map(|n| {
            expect::items(n)
                .map(|e| (e.attribute("lbl").unwrap_or("").to_string(), position(e)))
                .collect()
        })
        .unwrap_or_default();

    let names = &exp.names;
    let cumul = key(&names.cumul_filename);
    let total = band_table(&table(solve, &cumul)?, &cumul, &bands, None)?;
    let steps = total.first().map_or(0, Vec::len);
    let total_energy = bands
        .iter()
        .zip(total)
        .map(|(&freq_hz, energy)| BandEnergy { freq_hz, energy })
        .collect();

    let dir = key(&names.receiversp_directory);
    let labels: Vec<String> = config_receivers.iter().map(|r| r.0.clone()).collect();
    let mut point_receivers = Vec::new();
    for label in receiver_folders(solve, &dir, &labels)? {
        let folder = format!("{dir}/{label}");
        let path = |file: &str| key(&format!("{folder}/{file}"));
        let recp_rel = path(&names.receiversp_filename);
        let recp = band_table(&table(solve, &recp_rel)?, &recp_rel, &bands, Some(steps))?;
        let gap_rel = path(&names.receiversp_filename_adv);
        let gap = read_gap(&table(solve, &gap_rel)?, &gap_rel, &bands, steps, dt, &recp)?;
        let int_rel = path(INTENSITY);
        let intensity = read_intensity(&table(solve, &int_rel)?, &int_rel, &bands, steps)?;
        let src_rel = path(BY_SOURCE);
        let by_source = read_by_source(&table(solve, &src_rel)?, &src_rel, &bands, &sources)?;
        let position_m = config_receivers
            .iter()
            .find(|r| r.0 == label)
            .and_then(|r| r.1);
        let mut rows = Vec::with_capacity(bands.len());
        for (i, ((&freq_hz, energy), intensity)) in
            bands.iter().zip(recp).zip(intensity).enumerate()
        {
            rows.push(ReceiverBand {
                freq_hz,
                energy,
                lateral_cos2: gap.cos2[i].clone(),
                lateral_abs_cos: gap.abs_cos[i].clone(),
                intensity,
                source_power_rho_c: gap.source_power_rho_c[i],
                background_noise_db: gap.noise_db[i],
            });
        }
        point_receivers.push(PointReceiver {
            label,
            folder,
            position_m,
            bands: rows,
            by_source,
        });
    }

    let csbin: Vec<String> = exp
        .expected_files()
        .into_iter()
        .filter(|p| p.ends_with(".csbin"))
        .collect();
    let surfaces = read_surfaces(solve, &csbin, exp, &[])?;

    for f in &bands {
        let rel = key(&format!(
            "{}/{f} Hz/{}",
            fixed::INTENSITY_DIR,
            fixed::INTENSITY_FILE
        ));
        table(solve, &rel)?;
    }
    let mut particle_files = Vec::new();
    if exp.spps.as_ref().is_some_and(|s| s.nbparticules_rendu > 0) {
        for &f in &bands {
            let rel = key(&format!(
                "{}{f}\\{}",
                names.particules_directory, names.particules_filename
            ));
            let p = pbin::read_file(&solve.join(&rel)).map_err(|e| file_invalid(&rel, e))?;
            particle_files.push(ParticleFileSummary {
                path: rel,
                freq_hz: f,
                particles: p.particles.len(),
                recorded_steps: p.steps.len(),
            });
        }
    }

    Ok(SppsResults {
        time_step_s: f64::from(dt),
        duration_s: f64::from(duration),
        steps,
        speed_of_sound_m_s: f64::from(c),
        receiver_radius_m: f64::from(radius),
        celerity_gradient: gradient,
        computation_method: exp.spps.as_ref().map_or(0, |s| s.computation_method),
        sources,
        point_receivers,
        total_energy,
        particles,
        surfaces,
        particle_files,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_delay_starts_the_particles_at_the_next_whole_step() {
        let dt = 0.01f32;
        assert_eq!(emission_s(0.0, dt), 0.0);
        // 0.015 s is step 1.5, so step 2.
        assert_eq!(emission_s(0.015, dt), 2.0 * f64::from(dt));
        // 0.01 / 0.01 is exactly 1 in f32: step 1, not 2.
        assert_eq!(emission_s(0.01, dt), f64::from(dt));
        // A delay past the u16 step counter is held at its largest step.
        assert_eq!(emission_s(1e6, dt), f64::from(u16::MAX) * f64::from(dt));
    }

    #[test]
    fn the_speed_of_sound_is_upstreams_at_20_c() {
        assert_eq!(solver_speed_of_sound(20.0), 343.2f32);
        assert!((f64::from(solver_speed_of_sound(0.0)) - 331.29).abs() < 0.01);
    }
}
