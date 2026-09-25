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

use super::reference::{Reference, reference};
use super::{Refusal, SurfaceFile, band_of, file_invalid, key, read_surfaces, value_invalid};
use crate::formats::gabe::{self, Gabe};
use crate::formats::pbin;
use crate::params::noise::{Method, NoiseModel};
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
    /// Its power in each computed band, W, as SPPS computes it: `10⁻¹²·10^(db/10)` in `f32`
    /// from its `bfreq@db` children sorted by `freq` and taken by position
    /// (`base_core_configuration.cpp:140-150`). Each of its particles starts with this over
    /// `nbparticules` (`sppsNantes.cpp:73`).
    pub band_power_w: Vec<f64>,
    /// `directivite` is 5, a directivity balloon: SPPS scales each particle's energy by the
    /// balloon's value in its direction (`sppsNantes.cpp:115-127`), so its particles do not all
    /// carry `W/N`.
    pub balloon: bool,
}

/// One source's own echogram at a point receiver: `<receiver folder>/<source name>/
/// <receiversp_filename>`, written when `output_recp_bysource` is on
/// (`spps/input_output/reportmanager.cpp:620-656`).
#[derive(Clone, Debug, PartialEq)]
pub struct SourceEchogram {
    pub source: String,
    /// Relative to `solve/`.
    pub file: String,
    /// One per computed band, ascending: Pa² per time step, as the `.recp`.
    pub energy: Vec<Vec<f64>>,
}

/// One band of one point receiver.
#[derive(Clone, Debug, PartialEq)]
pub struct ReceiverBand {
    pub freq_hz: i32,
    /// The `.recp` column: Pa² per time step.
    pub energy: Vec<f64>,
    /// The `.gap`'s `E·cos²φ` column (+1), or where it holds a NaN ([`LateralNaN`]).
    pub lateral_cos2: Result<Vec<f64>, LateralNaN>,
    /// The `.gap`'s `E·|cos φ|` column (+2), or where it holds a NaN.
    pub lateral_abs_cos: Result<Vec<f64>, LateralNaN>,
    /// The intensity vector per step, x, y and z (the `Sum` row left out).
    pub intensity: [Vec<f64>; 3],
    /// The `.gap`'s column 2: the sources' power in this band times `ρ·c`
    /// (`reportmanager.cpp:807-816`).
    pub source_power_rho_c: f64,
    /// The `.gap`'s column 4: the receiver's background noise in this band, dB.
    pub background_noise_db: f64,
}

/// A `.gap` lateral column that holds a NaN, at `step` (the first). SPPS computes the angle `φ`
/// between a particle's direction and the receiver's orientation as `acos` of their normalised dot
/// product in `f32`, unclamped (`lib_interface/Core/mathlib.h:176-180`, reached through
/// `direction.angle(...)` at `spps/input_output/reportmanager.cpp:222`); a direction along the
/// orientation can take the argument past ±1, and one such crossing makes that step's
/// `E·cos²φ` and `E·|cos φ|` sums NaN (`reportmanager.cpp:226-227`).
/// Seen once in ten tutorial 1 runs at 1,500,000 particles in energetic mode (`docs/results.md`).
/// Only the lateral column is unusable: the energy the parameters read is written apart, and is
/// checked equal to the `.recp`'s. So the column carries this instead of values, and the run is
/// not refused for it; any other value that is not a finite energy of at least 0 still is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LateralNaN {
    pub step: usize,
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
    /// Each source's own echogram, in `config.xml`'s order, when `output_recp_bysource` is on;
    /// empty otherwise.
    pub echograms: Vec<SourceEchogram>,
}

impl PointReceiver {
    /// The sources whose energy reaches this receiver in band `index` (their `.recps` total is
    /// above 0), by name.
    pub fn contributing(&self, index: usize) -> Vec<&str> {
        self.by_source
            .iter()
            .filter(|s| s.energy.get(index).is_some_and(|&e| e > 0.0))
            .map(|s| s.source.as_str())
            .collect()
    }
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
    /// Relative to `solve/`.
    pub path: String,
    pub freq_hz: i32,
    /// The particles written: those of the `nbparticules_rendu` per source that recorded at least
    /// one step (`docs/formats/pbin.md`).
    pub particles: usize,
    /// Their step records, all particles together.
    pub recorded_steps: usize,
}

/// A decoded `.pbin` against its siblings (`docs/formats/pbin.md`): its time step is `pasdetemps`'s
/// `f32` bit for bit and its step count the run's; it holds at most the particles requested, each
/// with at least one step and none past the last step; and every energy is finite and not
/// negative (`results_value_invalid` otherwise, `results_file_invalid` for the rest).
fn check_particles(
    rel: &str,
    p: &pbin::ParticleFile,
    dt: f32,
    steps: usize,
    requested: usize,
) -> Result<(), Refusal> {
    let h = &p.header;
    if h.time_step.to_bits() != dt.to_bits() {
        return Err(file_invalid(
            rel,
            format!("its time step is {} s, pasdetemps is {dt} s", h.time_step),
        ));
    }
    if h.nb_time_step_max as usize != steps {
        return Err(file_invalid(
            rel,
            format!(
                "it counts {} time steps, the run has {steps}",
                h.nb_time_step_max
            ),
        ));
    }
    if p.particles.len() > requested {
        return Err(file_invalid(
            rel,
            format!(
                "it holds {} particles, {requested} were asked for",
                p.particles.len()
            ),
        ));
    }
    for (i, (ph, _)) in p.iter().enumerate() {
        let end = usize::from(ph.first_time_step) + ph.nb_time_step as usize;
        if ph.nb_time_step == 0 || end > steps {
            return Err(file_invalid(
                rel,
                format!(
                    "particle {i} records {} steps from step {}, the run has {steps}",
                    ph.nb_time_step, ph.first_time_step
                ),
            ));
        }
    }
    if let Some((i, s)) = p.steps.iter().enumerate().find(|(_, s)| {
        !s.energy.is_finite() || s.energy < 0.0 || s.position.iter().any(|x| !x.is_finite())
    }) {
        return Err(value_invalid(
            rel,
            format!(
                "step record {i} has position {:?} and energy {}",
                s.position, s.energy
            ),
        ));
    }
    Ok(())
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
    /// `nbparticules`: particles per source and band.
    pub particles_per_source: u32,
    /// `trans_epsilon` as SPPS reads it (`f32`): a particle is dropped once its energy is at or
    /// below `10^-trans_epsilon` of its start (`sppsNantes.cpp:75`).
    pub trans_epsilon: f64,
    /// `output_recp_bysource`: each point receiver has an echogram per source.
    pub echogram_per_source: bool,
    pub sources: Vec<SourcePoint>,
    /// In the order of their folder names.
    pub point_receivers: Vec<PointReceiver>,
    /// `<cumul_filename>`, one per band.
    pub total_energy: Vec<BandEnergy>,
    pub particles: ParticleStats,
    pub surfaces: Vec<SurfaceFile>,
    pub particle_files: Vec<ParticleFileSummary>,
    /// The analytic reference on the run's inputs ([`super::reference`]): Kuttruff's corrected
    /// Eyring with `γ²` from the room's geometry, and plain Eyring. Not validated. Boxed: it is
    /// the largest field, and `SolverResults` holds this or a TCR run's.
    pub reference: Box<Reference>,
}

impl SppsResults {
    /// When the direct sound reaches `r`'s centre: the earliest over the sources of emission plus
    /// straight-line distance over [`SppsResults::speed_of_sound_m_s`]. `None` with a celerity
    /// gradient, or when a position is not known.
    pub fn arrival_s(&self, r: &PointReceiver) -> Option<f64> {
        let names: Vec<&str> = self.sources.iter().map(|s| s.name.as_str()).collect();
        self.arrival_from(r, &names)
    }

    /// [`SppsResults::arrival_s`] over the sources named `names` only.
    pub fn arrival_from(&self, r: &PointReceiver, names: &[&str]) -> Option<f64> {
        if self.celerity_gradient {
            return None;
        }
        let p = r.position_m?;
        let mut best: Option<f64> = None;
        for s in self
            .sources
            .iter()
            .filter(|s| names.contains(&s.name.as_str()))
        {
            let q = s.position_m?;
            let d = ((p[0] - q[0]).powi(2) + (p[1] - q[1]).powi(2) + (p[2] - q[2]).powi(2)).sqrt();
            let t = s.emission_s + d / self.speed_of_sound_m_s;
            best = Some(best.map_or(t, |b: f64| b.min(t)));
        }
        best
    }

    /// The level below a particle's start at which SPPS drops it, dB, when that can cost a
    /// histogram energy: in energetic mode, where energy falls by degrees, `-10·trans_epsilon`
    /// (`sppsNantes.cpp:75`; `CalculationCore.cpp:57-60, 141-146, 305-310`); in random mode,
    /// where a particle keeps its start energy, only when `trans_epsilon` is not above 0, since
    /// then every particle is dropped at its first surface (`CalculationCore.cpp:305`). `None`
    /// otherwise.
    pub fn floor_db(&self) -> Option<f64> {
        let energetic = self.computation_method != 0;
        let positive = self.trans_epsilon > 0.0;
        (energetic || !positive).then_some(-10.0 * self.trans_epsilon)
    }

    /// The mean energy one crossing of source `source`'s particles adds to a bin of band `index`,
    /// Pa²: `W·ρc/(N·πR²)` (`params::noise`). `ρc` is the `.gap`'s sources' power times `ρc`
    /// (`reportmanager.cpp:807-816`) over the sources' summed power. `None` when a receiver's
    /// `.gap` is not there to give it, or the powers sum to 0.
    pub fn mean_deposit(&self, source: usize, index: usize) -> Option<f64> {
        let rho_c_w = self
            .point_receivers
            .first()?
            .bands
            .get(index)?
            .source_power_rho_c;
        let total: f64 = self
            .sources
            .iter()
            .filter_map(|s| s.band_power_w.get(index))
            .sum();
        let w = *self.sources.get(source)?.band_power_w.get(index)?;
        let n = f64::from(self.particles_per_source);
        let r = self.receiver_radius_m;
        (total > 0.0).then(|| rho_c_w * (w / total) / (n * std::f64::consts::PI * r * r))
    }

    /// The noise model of a series of band `index` made by the sources named `names`: crossings
    /// of the largest of their mean deposits, under the run's computation method (which picks the
    /// calibration) and particle count (from which a refusal names the count it needs); or
    /// [`NoiseModel::Unknown`] when one of the sources is a directivity balloon, whose particles
    /// carry unequal energies, or a deposit is not known.
    pub fn noise_model(&self, index: usize, names: &[&str]) -> NoiseModel {
        let mut largest: Option<f64> = None;
        for (i, s) in self.sources.iter().enumerate() {
            if !names.contains(&s.name.as_str()) {
                continue;
            }
            if s.balloon {
                return NoiseModel::Unknown {
                    detail: format!(
                        "source {:?} has a directivity balloon, which scales each particle's \
                         energy by its direction (sppsNantes.cpp:115-127); the noise of unequal \
                         particles is not modelled",
                        s.name
                    ),
                };
            }
            match self.mean_deposit(i, index) {
                Some(d) => largest = Some(largest.map_or(d, |l: f64| l.max(d))),
                None => {
                    return NoiseModel::Unknown {
                        detail: format!("source {:?}'s mean deposit is not known", s.name),
                    };
                }
            }
        }
        let (method, n) = (self.noise_method(), Some(self.particles_per_source));
        match largest.map(|d| NoiseModel::crossings(d, method, n)) {
            Some(Ok(m)) => m,
            Some(Err(e)) => NoiseModel::Unknown {
                detail: e.to_string(),
            },
            None => NoiseModel::Unknown {
                detail: "no source emits in the band".into(),
            },
        }
    }

    /// The computation method as the noise model knows it (`computation_method` 0 random, any
    /// other energetic, as SPPS reads it), which picks its calibration.
    pub fn noise_method(&self) -> Method {
        if self.computation_method == 0 {
            Method::Random
        } else {
            Method::Energetic
        }
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
    /// `freq_hz`, but what unfinished paths would have brought: random mode with `trans_epsilon`
    /// above 0, and at most [`REMAINING_UNFINISHED_SHARE`] of the band's particles remaining at the
    /// end of the calculation. A particle is counted as remaining only when the time steps run out
    /// while it is alive (`spps/CalculationCore.cpp:49, 88-92`), and in random mode a particle's
    /// energy never dwindles, it is absorbed whole (`CalculationCore.cpp:62-67, 288-300`), so the
    /// histogram holds every particle's whole path, except the unfinished paths of the lost and
    /// the remaining ones, which [`SppsResults::lost_share`] bounds together. Energetic mode drops
    /// a particle once its energy falls below `10^-trans_epsilon` of its start
    /// (`sppsNantes.cpp:75`; `CalculationCore.cpp:305`), so it is never complete here: `params`
    /// bounds its tail and its floor instead.
    pub fn band_complete(&self, freq_hz: i32) -> bool {
        self.computation_method == 0
            && self.trans_epsilon > 0.0
            && self
                .particles
                .bands
                .iter()
                .find(|b| b.freq_hz == freq_hz)
                .is_some_and(|b| {
                    f64::from(b.remaining) <= REMAINING_UNFINISHED_SHARE * f64::from(b.total)
                })
    }

    /// The particles of band `freq_hz` whose paths stopped unfinished: lost (`partLoop`,
    /// `partLost`), and, when the band is complete ([`SppsResults::band_complete`]), remaining
    /// at the end.
    fn unfinished(&self, b: &crate::run::stats::BandStats) -> u64 {
        let remaining = if self.band_complete(b.freq_hz) {
            u64::from(b.remaining)
        } else {
            0
        };
        b.lost() + remaining
    }

    /// The share of a receiver's energy after step `bin` that band `index`'s unfinished particles
    /// can have taken with them, or `None` when there are none: those lost (`partLoop` and
    /// `partLost`, `CalculationCore.cpp:102-107`) and, in a complete band, those remaining at the
    /// end ([`SppsResults::band_complete`]). Such a particle stops mid-path; what it would still
    /// have brought is, on average, what any particle alive at that time brings. The room table
    /// (`<cumul_filename>`) holds the energy of the particles alive at the end of each step times
    /// `ρc` (`reportmanager.cpp:155-166, 426-437`), and the `.gap` the sources' power times `ρc`,
    /// so their ratio at `bin` is the share of the emitted energy still alive then, `f`. With
    /// `n` particles unfinished of `N` emitted, the share is `n / (N·f)` of the energy the
    /// receiver gets from `bin` on (`docs/results.md`, "Lost particles").
    pub fn lost_share(&self, index: usize, freq_hz: i32, bin: usize) -> Option<f64> {
        let b = self.particles.bands.iter().find(|b| b.freq_hz == freq_hz)?;
        let n = self.unfinished(b);
        if n == 0 {
            return None;
        }
        let emitted = f64::from(self.particles_per_source) * self.sources.len() as f64;
        // Nothing known alive: nothing bounds what the unfinished particles took.
        Some(match self.alive_share(index, bin) {
            Some(alive) if alive > 0.0 && emitted > 0.0 => n as f64 / (emitted * alive),
            _ => f64::MAX,
        })
    }

    /// In energetic mode, the share of the energy a receiver gets from any time on that band
    /// `freq_hz`'s lost particles can have taken with them, or `None` when none was lost or the
    /// mode is random. Energetic mode keeps every particle until the floor, its energy falling
    /// with the room's, so a particle lost at `t` carries about the mean energy of the particles
    /// then, and what it would still have brought is its share of what they all bring after `t`:
    /// at most [`ENERGETIC_LOST_ENERGY_RATIO`]`·n/N` of the energy from every time on
    /// (`params::EnergySeries::with_lost_share_following_decay`; `docs/results.md`, "Lost
    /// particles").
    pub fn lost_share_following_decay(&self, freq_hz: i32) -> Option<f64> {
        if self.computation_method == 0 {
            return None;
        }
        let b = self.particles.bands.iter().find(|b| b.freq_hz == freq_hz)?;
        if b.lost() == 0 {
            return None;
        }
        let emitted = f64::from(self.particles_per_source) * self.sources.len() as f64;
        Some(if emitted > 0.0 {
            ENERGETIC_LOST_ENERGY_RATIO * b.lost() as f64 / emitted
        } else {
            f64::MAX
        })
    }

    /// The share of the emitted energy of band `index` the room held at the end of step `bin`: the
    /// room table (`<cumul_filename>`, the energy of the particles alive times `ρc`,
    /// `reportmanager.cpp:155-166, 426-437`) over the `.gap`'s sources' power times `ρc`
    /// (`reportmanager.cpp:807-816`). In random mode the share of the particles alive; in
    /// energetic mode their mean energy over their start energy. `None` when either is missing or
    /// the power is not above 0.
    pub fn alive_share(&self, index: usize, bin: usize) -> Option<f64> {
        let power = self
            .point_receivers
            .first()?
            .bands
            .get(index)?
            .source_power_rho_c;
        let room = *self.total_energy.get(index)?.energy.get(bin)?;
        (power > 0.0).then(|| room / power)
    }
}

/// In random mode, the most particles that may remain alive at the end of a band's calculation,
/// as a share of its particles, for the band still to be complete, the remaining ones bounded as
/// unfinished paths ([`SppsResults::band_complete`], [`SppsResults::lost_share`]): one in a
/// million. Measured (`docs/results.md`, "Complete series"): in the 6×10×3 room SPPS leaves about
/// one particle in 10⁸ alive at the end of runs whose decay has fallen hundreds of decibels, which
/// no absorption draw can explain (at α 0.4, surviving the 103 reflections of 1 s has a chance of
/// 10⁻²³); refusing the band's every quantity for it was not honest. A run cut short, with many
/// particles alive, stays incomplete, and its tail is bounded from the series.
pub const REMAINING_UNFINISHED_SHARE: f64 = 1e-6;

/// In energetic mode, the most a lost particle's energy is taken to be over the mean energy of the
/// particles when it was lost: **an empirical cap, not a bound.** Measured on tutorial 1 in
/// energetic mode (3 seeds, 6 octave bands, 150,000 particles with every trajectory saved): of the
/// 24 particles SPPS counted as lost, the 17 found in the trajectories carried 0.16 to 2.02 times
/// the mean (`crates/simpa/tests/m8_evidence.rs`, `energetic_lost_particles_from_saved_
/// trajectories`); the other 7 ended with less than 10⁻⁴ of their start energy, so their ratio is
/// not known. Five times the largest measured. A late loss in a uniform, strongly absorbing room
/// can exceed it (`docs/results.md`, "Lost particles", for what that can move).
pub const ENERGETIC_LOST_ENERGY_RATIO: f64 = 10.0;

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
    cos2: Vec<Result<Vec<f64>, LateralNaN>>,
    abs_cos: Vec<Result<Vec<f64>, LateralNaN>>,
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
            // A NaN makes the column unusable, not the run (LateralNaN); anything else that is not
            // a finite energy of at least 0 refuses the run.
            out.push(match v.iter().position(|x| x.is_nan()) {
                Some(step) => Err(LateralNaN { step }),
                None => Ok(energies(rel, &format!("{f} Hz lateral +{k}"), v)?),
            });
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

/// The folders directly inside `rel` (relative to `solve`), sorted.
fn subfolders(solve: &Path, rel: &str) -> Result<Vec<String>, Refusal> {
    let mut names = Vec::new();
    for e in std::fs::read_dir(solve.join(rel)).map_err(|e| file_invalid(rel, e))? {
        let e = e.map_err(|e| file_invalid(rel, e))?;
        if !e.file_type().map_err(|e| file_invalid(rel, e))?.is_dir() {
            continue;
        }
        names.push(
            e.file_name()
                .into_string()
                .map_err(|n| file_invalid(rel, format!("a folder name is not Unicode: {n:?}")))?,
        );
    }
    names.sort();
    Ok(names)
}

/// Each source's own echogram in the receiver folder `folder`, when `on`
/// (`spps/input_output/reportmanager.cpp:620-656`), checked:
/// - the folder's subfolders are exactly the sources' names when on, and there are none when off;
/// - each file is laid out as the `.recp` (the same writer, `baseReportManager.cpp:160-190`);
/// - bin by bin the sources' echograms sum to the `.recp`, since both add the same crossings'
///   energy (`reportmanager.cpp:223-234`), to within the `f32` rounding of their sum;
/// - each sums over time to its `.recps` total.
#[allow(clippy::too_many_arguments)]
fn read_echograms(
    solve: &Path,
    folder: &str,
    file: &str,
    bands: &[i32],
    steps: usize,
    sources: &[SourcePoint],
    on: bool,
    recp: &[Vec<f64>],
    by_source: &[SourceTotals],
) -> Result<Vec<SourceEchogram>, Refusal> {
    let found = subfolders(solve, folder)?;
    let mut want: Vec<String> = if on {
        sources.iter().map(|s| s.name.clone()).collect()
    } else {
        Vec::new()
    };
    want.sort();
    if found != want {
        return Err(file_invalid(
            folder,
            format!(
                "its folders are {found:?}; with output_recp_bysource {}, SPPS writes {want:?}",
                u8::from(on)
            ),
        ));
    }
    if !on {
        return Ok(Vec::new());
    }
    let mut out = Vec::with_capacity(sources.len());
    for s in sources {
        let rel = key(&format!("{folder}/{}/{file}", s.name));
        let energy = band_table(&table(solve, &rel)?, &rel, bands, Some(steps))?;
        out.push(SourceEchogram {
            source: s.name.clone(),
            file: rel,
            energy,
        });
    }
    let n = sources.len() as f64;
    for (i, f) in bands.iter().enumerate() {
        for (k, &total) in recp[i].iter().enumerate().take(steps) {
            let sum: f64 = out.iter().map(|e| e.energy[i][k]).sum();
            let tol = 1e-6 * n * sum.max(total);
            if (sum - total).abs() > tol {
                return Err(file_invalid(
                    folder,
                    format!(
                        "at {f} Hz, step {k}, the sources' echograms sum to {sum}, its .recp holds \
                         {total}"
                    ),
                ));
            }
        }
        for (e, t) in out.iter().zip(by_source) {
            let sum: f64 = e.energy[i].iter().sum();
            let total = t.energy[i];
            if (sum - total).abs() > 1e-5 * sum.max(total) {
                return Err(file_invalid(
                    &e.file,
                    format!(
                        "at {f} Hz it sums to {sum}, the .recps gives source {:?} {total}",
                        e.source
                    ),
                ));
            }
        }
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
    // SPPS reads a missing trans_epsilon as 0 (`core_configuration.cpp:65`, `ToFloat`).
    let trans_epsilon = sim
        .and_then(|n| n.attribute("trans_epsilon"))
        .map_or(Some(0.0), locate::to_float)
        .map_or(f64::NAN, f64::from);

    // The positions, among all of config.xml's bands in ascending order, of the computed ones.
    let computed: Vec<usize> = exp
        .bands
        .iter()
        .enumerate()
        .filter(|(_, b)| b.requested)
        .map(|(i, _)| i)
        .collect();
    let mut sources: Vec<SourcePoint> = Vec::new();
    for e in expect::child(root, "sources")
        .map(|n| expect::items(n).collect::<Vec<_>>())
        .unwrap_or_default()
    {
        let name = e.attribute("name").unwrap_or("").to_string();
        let delay = locate::to_float(e.attribute("delay").unwrap_or("")).unwrap_or(0.0);
        // `bfreq` children sorted by `freq` read as an integer (`OrderChildsByProperty`,
        // `cxml.cpp:130-146`), each band taken by position (`base_core_configuration.cpp:
        // 140-150`).
        let mut levels: Vec<(i32, f32)> = Vec::new();
        for b in expect::items(e) {
            let f = expect::atoi(b.attribute("freq").unwrap_or(""));
            let db = locate::to_float(b.attribute("db").unwrap_or("")).unwrap_or(f32::NAN);
            levels.push((f, db));
        }
        levels.sort_by_key(|l| l.0);
        let mut band_power_w = Vec::with_capacity(computed.len());
        for &i in &computed {
            match levels.get(i) {
                Some(&(_, db)) if db.is_finite() => {
                    band_power_w.push(f64::from((1e-12 * 10f64.powf(f64::from(db) / 10.0)) as f32))
                }
                _ => {
                    return Err(file_invalid(
                        CONFIG,
                        format!(
                            "source {name:?} has no level for band {} Hz",
                            exp.bands[i].freq_hz
                        ),
                    ));
                }
            }
        }
        sources.push(SourcePoint {
            balloon: expect::atoi(e.attribute("directivite").unwrap_or("")) == 5,
            name,
            position_m: position(e),
            emission_s: emission_s(delay, dt),
            band_power_w,
        });
    }
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
        let per_source = exp.spps.as_ref().is_some_and(|s| s.output_recp_bysource);
        let echograms = read_echograms(
            solve,
            &folder,
            &names.receiversp_filename,
            &bands,
            steps,
            &sources,
            per_source,
            &recp,
            &by_source,
        )?;
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
            echograms,
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
    let saved = exp.spps.as_ref().map_or(0, |s| s.nbparticules_rendu);
    if saved > 0 {
        let requested = saved as usize * sources.len();
        for &f in &bands {
            let rel = key(&format!(
                "{}{f}\\{}",
                names.particules_directory, names.particules_filename
            ));
            let p = pbin::read_file(&solve.join(&rel)).map_err(|e| file_invalid(&rel, e))?;
            check_particles(&rel, &p, dt, steps, requested)?;
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
        particles_per_source: exp.spps.as_ref().map_or(1, |s| s.nbparticules),
        trans_epsilon,
        echogram_per_source: exp.spps.as_ref().is_some_and(|s| s.output_recp_bysource),
        sources,
        point_receivers,
        total_energy,
        particles,
        surfaces,
        particle_files,
        reference: Box::new(reference(solve, exp, f64::from(c), gradient)),
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

    fn run(method: i32, eps: f64, remaining: u32, lost: u32) -> SppsResults {
        use crate::run::stats::BandStats;
        SppsResults {
            time_step_s: 0.01,
            duration_s: 1.0,
            steps: 100,
            speed_of_sound_m_s: 343.2,
            receiver_radius_m: 0.31,
            celerity_gradient: false,
            computation_method: method,
            particles_per_source: 1000,
            trans_epsilon: eps,
            echogram_per_source: false,
            sources: Vec::new(),
            point_receivers: Vec::new(),
            total_energy: Vec::new(),
            particles: ParticleStats {
                bands: vec![BandStats {
                    freq_hz: 500,
                    absorbed_by_atmosphere: 0,
                    absorbed_by_materials: 1000 - remaining - lost,
                    absorbed_by_fittings: 0,
                    lost_by_infinite_loops: 0,
                    lost_by_meshing_problems: lost,
                    remaining,
                    total: 1000,
                }],
            },
            surfaces: Vec::new(),
            particle_files: Vec::new(),
            reference: Box::new(Reference::NotComputed {
                why: "a unit test's run".into(),
            }),
        }
    }

    #[test]
    fn only_random_mode_with_nothing_remaining_is_complete() {
        assert!(run(0, 5.0, 0, 0).band_complete(500));
        // Lost particles leave it complete; their share is bounded separately.
        assert!(run(0, 5.0, 0, 1).band_complete(500));
        // Say no: energetic mode, even with every particle accounted for.
        assert!(!run(1, 5.0, 0, 0).band_complete(500));
        // A particle still alive at the end.
        assert!(!run(0, 5.0, 1, 0).band_complete(500));
        // trans_epsilon 0 drops every particle at its first surface, in random mode too.
        assert!(!run(0, 0.0, 0, 0).band_complete(500));
        assert!(!run(0, f64::NAN, 0, 0).band_complete(500));
        // A band the statistics do not list.
        assert!(!run(0, 5.0, 0, 0).band_complete(1000));
    }

    #[test]
    fn the_lost_share_is_the_lost_over_the_alive() {
        // None lost: no share.
        assert_eq!(run(0, 5.0, 0, 0).lost_share(0, 500, 0), None);
        // Lost, and nothing known alive: nothing bounds it.
        assert_eq!(run(0, 5.0, 0, 3).lost_share(0, 500, 0), Some(f64::MAX));
        // 3 lost of 1000 emitted, a quarter of the emitted energy alive at the end of step 1.
        let mut r = run(0, 5.0, 0, 3);
        r.sources = vec![SourcePoint {
            name: "S".into(),
            position_m: None,
            emission_s: 0.0,
            band_power_w: vec![1.0],
            balloon: false,
        }];
        r.total_energy = vec![BandEnergy {
            freq_hz: 500,
            energy: vec![2.0, 0.5],
        }];
        r.point_receivers = vec![PointReceiver {
            label: "R".into(),
            folder: "Punctual receivers/R".into(),
            position_m: None,
            bands: vec![ReceiverBand {
                freq_hz: 500,
                energy: vec![1.0, 1.0],
                lateral_cos2: Ok(vec![0.0; 2]),
                lateral_abs_cos: Ok(vec![0.0; 2]),
                intensity: Default::default(),
                source_power_rho_c: 2.0,
                background_noise_db: 0.0,
            }],
            by_source: Vec::new(),
            echograms: Vec::new(),
        }];
        let share = r.lost_share(0, 500, 1).unwrap();
        assert!((share - 3.0 / (1000.0 * 0.25)).abs() < 1e-15, "{share}");
    }

    #[test]
    fn a_particle_in_a_million_left_alive_is_bounded_as_unfinished_not_refused() {
        // 10 million particles, a quarter of the emitted energy alive at the end of step 1.
        let big = |remaining: u32, lost: u32| {
            let mut r = run(0, 5.0, 0, 0);
            r.particles_per_source = 10_000_000;
            let b = &mut r.particles.bands[0];
            b.total = 10_000_000;
            b.remaining = remaining;
            b.lost_by_meshing_problems = lost;
            b.absorbed_by_materials = 10_000_000 - remaining - lost;
            r.sources = vec![SourcePoint {
                name: "S".into(),
                position_m: None,
                emission_s: 0.0,
                band_power_w: vec![1.0],
                balloon: false,
            }];
            r.total_energy = vec![BandEnergy {
                freq_hz: 500,
                energy: vec![2.0, 0.5],
            }];
            r.point_receivers = vec![PointReceiver {
                label: "R".into(),
                folder: "Punctual receivers/R".into(),
                position_m: None,
                bands: vec![ReceiverBand {
                    freq_hz: 500,
                    energy: vec![1.0, 1.0],
                    lateral_cos2: Ok(vec![0.0; 2]),
                    lateral_abs_cos: Ok(vec![0.0; 2]),
                    intensity: Default::default(),
                    source_power_rho_c: 2.0,
                    background_noise_db: 0.0,
                }],
                by_source: Vec::new(),
                echograms: Vec::new(),
            }];
            r
        };
        // 3 remaining and 2 lost of 10 million: complete, the 5 unfinished bounded together.
        let r = big(3, 2);
        assert!(r.band_complete(500));
        let share = r.lost_share(0, 500, 1).unwrap();
        assert!(
            (share - 5.0 / (10_000_000.0 * 0.25)).abs() < 1e-18,
            "{share}"
        );
        // 10 remaining is one in a million: still complete; 11 is not, and the remaining ones are
        // then left to the tail, the share counting the lost alone.
        assert!(big(10, 0).band_complete(500));
        let r = big(11, 2);
        assert!(!r.band_complete(500));
        assert_eq!(r.lost_share(0, 500, 1), Some(2.0 / (10_000_000.0 * 0.25)));
        // Says no: energetic mode is never complete, whatever remains.
        let mut e = big(0, 0);
        e.computation_method = 1;
        assert!(!e.band_complete(500));
    }

    #[test]
    fn energetic_lost_particles_take_rho_n_over_n_of_the_energy_from_every_time_on() {
        let with_source = |method: i32, lost: u32| {
            let mut r = run(method, 5.0, 0, lost);
            r.sources = vec![SourcePoint {
                name: "S".into(),
                position_m: None,
                emission_s: 0.0,
                band_power_w: vec![1.0],
                balloon: false,
            }];
            r
        };
        // 3 lost of 1000 emitted, energetic: ρ·3/1000.
        let share = with_source(1, 3).lost_share_following_decay(500).unwrap();
        assert_eq!(share, ENERGETIC_LOST_ENERGY_RATIO * 3.0 / 1000.0);
        // Two sources emit twice the particles.
        let mut two = with_source(1, 3);
        two.sources.push(two.sources[0].clone());
        assert_eq!(
            two.lost_share_following_decay(500),
            Some(ENERGETIC_LOST_ENERGY_RATIO * 3.0 / 2000.0)
        );
        // Says no: random mode keeps its lump from the arrival; none lost gives none; a band the
        // statistics do not list gives none; nothing emitted leaves nothing bounded.
        assert_eq!(with_source(0, 3).lost_share_following_decay(500), None);
        assert_eq!(with_source(1, 0).lost_share_following_decay(500), None);
        assert_eq!(with_source(1, 3).lost_share_following_decay(1000), None);
        assert_eq!(
            run(1, 5.0, 0, 3).lost_share_following_decay(500),
            Some(f64::MAX)
        );
    }

    #[test]
    fn the_floor_is_energetic_modes_or_a_non_positive_epsilons() {
        assert_eq!(run(1, 5.0, 0, 0).floor_db(), Some(-50.0));
        assert_eq!(run(1, 3.0, 0, 0).floor_db(), Some(-30.0));
        assert_eq!(run(0, 5.0, 0, 0).floor_db(), None);
        assert_eq!(run(0, 0.0, 0, 0).floor_db(), Some(-0.0));
        assert!(run(0, f64::NAN, 0, 0).floor_db().unwrap().is_nan());
    }

    #[test]
    fn the_speed_of_sound_is_upstreams_at_20_c() {
        assert_eq!(solver_speed_of_sound(20.0), 343.2f32);
        assert!((f64::from(solver_speed_of_sound(0.0)) - 331.29).abs() < 0.01);
    }
}
