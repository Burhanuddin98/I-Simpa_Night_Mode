//! The bed file, `beds/m8a.json`: the matrix of `docs/investigations/2026-09-29-m8a/SPEC.md`,
//! section 2, the seeds, the transport's settings, and upstream's atmospheric-absorption values
//! (section 2.5) with their sha256; and the projects each run is built from.
//!
//! **The file is not a way to tune the matrix** (E7). [`BedFile::m8a`] is the matrix, in code;
//! [`load`] reads a file, puts it in its canonical form ([`canonical_text`]: the parsed file
//! written again, so line ends and spacing do not count, every value does) and compares it with
//! the matrix's, byte for byte. Any other file runs as `exploratory`, and an exploratory report
//! never passes. The limits of section 5 are constants in the code (`bed::limits`), not fields of
//! the file.

use serde::{Deserialize, Serialize};

use crate::schema::{
    self, BandKind, BandSet, ComputationMethod, Face, GroupId, MaterialId, PointReceiverId,
    Project, ReflectionLaw, Vec3,
};

/// SPPS's computation method.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Method {
    Random,
    Energetic,
}

impl Method {
    pub fn name(self) -> &'static str {
        match self {
            Method::Random => "random",
            Method::Energetic => "energetic",
        }
    }

    fn schema(self) -> ComputationMethod {
        match self {
            Method::Random => ComputationMethod::Random,
            Method::Energetic => ComputationMethod::Energetic,
        }
    }
}

/// A box room: `[0, x] × [0, y] × [0, z]` m, its source and receivers.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Room {
    pub name: String,
    pub size_m: [f64; 3],
    pub source_m: [f64; 3],
    pub receivers_m: Vec<[f64; 3]>,
    /// Its cells are gated; otherwise reported only.
    pub gated: bool,
}

/// One SPPS cell: a room with every face absorbing `alpha` in every band, Lambert with scattering
/// 1, run over the bed's seeds.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Cell {
    pub id: String,
    pub room: String,
    pub alpha: f64,
    pub method: Method,
    pub particles_per_source: u32,
    pub duration_s: f64,
    pub trans_epsilon: f64,
    /// Air absorption on, and the octave bands to 8 kHz; off, and to 4 kHz.
    pub air: bool,
    pub gated: bool,
}

/// One TCR run: TCR is deterministic, one run, seed 1, on the project of the SPPS cell
/// `project_of` (TCR reads none of SPPS's settings).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TcrCell {
    pub id: String,
    pub project_of: String,
    pub gated: bool,
}

/// The independent transport's settings (`params_kuttruff.rs`,
/// `kuttruff_against_the_transport_at_high_counts`): `replicas` of
/// `rays_factor·16384·α/0.05` rays, followed to `duration_eyring_factor` times the cell's plain
/// Eyring time, seed `seed_base + seed_per_alpha·α`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TransportSettings {
    pub replicas: u32,
    pub rays_factor: f64,
    pub duration_eyring_factor: f64,
    /// `0x` and 16 hex digits.
    pub seed_base: String,
    pub seed_per_alpha: f64,
    pub speed_of_sound_m_s: f64,
    pub receiver_radius_m: f64,
    pub time_step_s: f64,
}

impl TransportSettings {
    /// The rays per replica at `alpha`, as the high-count test computes them.
    pub fn rays_per_replica(&self, alpha: f64) -> u32 {
        (self.rays_factor * 16384.0 * alpha / 0.05) as u32
    }

    /// The seed at `alpha`, as the high-count test computes it.
    pub fn seed(&self, alpha: f64) -> Result<u64, String> {
        let base = self
            .seed_base
            .strip_prefix("0x")
            .and_then(|h| u64::from_str_radix(h, 16).ok())
            .ok_or(format!("seed_base '{}' is not 0x and hex", self.seed_base))?;
        Ok(base + (alpha * self.seed_per_alpha) as u64)
    }
}

/// The air the rooms are run in.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Atmosphere {
    pub temperature_c: f64,
    pub relative_humidity_percent: f64,
    pub pressure_pa: f64,
}

/// Upstream's atmospheric-absorption validation (section 2.5): reported, not gated.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AtmosphericValidation {
    /// The `.proj`, relative to upstream's tree (read-only).
    pub proj: String,
    pub proj_sha256: String,
    /// The `.xlsx` the values below are transcribed from.
    pub xlsx: String,
    pub xlsx_sha256: String,
    /// The three changes to the imported project, each forced by a rule of the spec: seeds 1 to
    /// 10 (SB-2), `trans_epsilon` 7 (decision 6), `dt` 1 ms (row 11).
    pub seeds: Vec<u32>,
    pub trans_epsilon: f64,
    pub time_step_s: f64,
    /// Where each column is in the `.xlsx`.
    pub columns: String,
    pub bands_hz: Vec<u32>,
    pub spps_2018_rt30_s: Vec<f64>,
    pub spps_1_3_4_rt30_s: Vec<f64>,
    pub tcr_eyring_s: Vec<f64>,
    pub md_octave_rt30_s: Vec<f64>,
}

/// N5: the cell `cell` with seed `seed` run again at `particles_per_source`, which must make the
/// cell fail when it takes that seed's place.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SayNoN5 {
    pub cell: String,
    pub seed: u32,
    pub particles_per_source: u32,
}

/// N6: a cell with walls of scattering `scattering`, which the bed must refuse
/// (`params_reference_not_applicable`, E3).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SayNoN6 {
    pub room: String,
    pub alpha: f64,
    pub method: Method,
    pub particles_per_source: u32,
    pub duration_s: f64,
    pub trans_epsilon: f64,
    pub scattering: f64,
    pub seed: u32,
}

/// The bed's two say-NO runs (section 7, N5 and N6).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SayNo {
    pub n5: SayNoN5,
    pub n6: SayNoN6,
}

/// The bed file.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BedFile {
    pub bed: String,
    /// The pre-registered spec it is the matrix of.
    pub spec: String,
    /// The project every cell is built from, and its sha256 (the copy compiled into `simpa`).
    pub template: String,
    pub template_sha256: String,
    pub seeds: Vec<u32>,
    /// Gate C's one extension of an INCONCLUSIVE cell.
    pub extension_seeds: Vec<u32>,
    pub time_step_s: f64,
    pub bands_air_off_hz: Vec<u32>,
    pub bands_air_on_hz: Vec<u32>,
    pub atmosphere: Atmosphere,
    pub rooms: Vec<Room>,
    pub cells: Vec<Cell>,
    pub tcr: Vec<TcrCell>,
    pub transport: TransportSettings,
    pub atmospheric_validation: AtmosphericValidation,
    pub say_no: SayNo,
}

/// A cell's id: `<room>-a<α>-<method>-air-<on|off>`.
pub fn cell_id(room: &str, alpha: f64, method: Method, air: bool) -> String {
    format!(
        "{room}-a{alpha}-{}-air-{}",
        method.name(),
        if air { "on" } else { "off" }
    )
}

/// A TCR run's id: `<room>-a<α>-tcr-air-<on|off>`.
pub fn tcr_id(room: &str, alpha: f64, air: bool) -> String {
    format!("{room}-a{alpha}-tcr-air-{}", if air { "on" } else { "off" })
}

/// The xlsx's bands: third octaves 50 Hz to 20 kHz.
const XLSX_BANDS_HZ: [u32; 27] = [
    50, 63, 80, 100, 125, 160, 200, 250, 315, 400, 500, 630, 800, 1000, 1250, 1600, 2000, 2500,
    3150, 4000, 5000, 6300, 8000, 10000, 12500, 16000, 20000,
];
/// Sheet "SPPS", rows 33-59, column B.
const XLSX_SPPS_2018: [f64; 27] = [
    0.9970279, 0.9965895, 0.9964896, 0.9950515, 0.9941306, 0.9937508, 0.9922833, 0.9894863,
    0.9872737, 0.9836834, 0.9816568, 0.9782395, 0.97488, 0.9708613, 0.9650897, 0.955174, 0.9440701,
    0.9242678, 0.8964995, 0.8531513, 0.7953542, 0.7190524, 0.6227126, 0.5232825, 0.4232365,
    0.3227535, 0.2492215,
];
/// Sheet "SPPS", rows 33-59, column C.
const XLSX_SPPS_1_3_4: [f64; 27] = [
    0.9970102, 0.9961597, 0.9953468, 0.995307, 0.9934611, 0.9936479, 0.9919118, 0.9894354,
    0.9873552, 0.9837132, 0.9828663, 0.9794886, 0.9744252, 0.970069, 0.9658157, 0.9552461,
    0.9437342, 0.9241039, 0.8955307, 0.8519886, 0.7952037, 0.7191042, 0.62239, 0.5229257,
    0.4232141, 0.3233969, 0.249577,
];
/// Sheet "TCR", rows 33-59, column B, under "RT Eyring (s)".
const XLSX_TCR_EYRING: [f64; 27] = [
    0.9870604, 0.9868162, 0.9864252, 0.9858739, 0.985073, 0.9838083, 0.9822621, 0.9803256,
    0.9779752, 0.9753052, 0.9726825, 0.9698054, 0.9664195, 0.9624361, 0.9569871, 0.948068,
    0.9358492, 0.9176299, 0.8896199, 0.8472099, 0.7919419, 0.7168926, 0.6219621, 0.5234252,
    0.4244825, 0.3253897, 0.2515414,
];
/// Sheet "MD", rows 33-59, column B.
const XLSX_MD_OCTAVE: [f64; 27] = [
    1.001793, 1.001531, 1.001132, 1.00057, 0.9997421, 0.9984554, 0.9968535, 0.9948663, 0.992456,
    0.9897066, 0.987009, 0.9840598, 0.9805676, 0.9764861, 0.9708909, 0.9617118, 0.9491774,
    0.9304868, 0.9017729, 0.8583668, 0.8019147, 0.7254755, 0.6291149, 0.529474, 0.4298387,
    0.3304634, 0.2567113,
];

/// The random counts (section 2.3's rule) and durations, and the energetic durations and
/// `trans_epsilon`, per room and α.
struct Row {
    room: &'static str,
    alpha: f64,
    random: (u32, f64),
    energetic: (f64, f64),
}

const ROWS: [Row; 12] = [
    Row {
        room: "6x10x3",
        alpha: 0.05,
        random: (8_400_000, 4.0),
        energetic: (4.0, 7.0),
    },
    Row {
        room: "6x10x3",
        alpha: 0.1,
        random: (18_000_000, 3.0),
        energetic: (2.0, 7.0),
    },
    Row {
        room: "6x10x3",
        alpha: 0.2,
        random: (33_000_000, 2.0),
        energetic: (1.0, 9.0),
    },
    Row {
        room: "6x10x3",
        alpha: 0.4,
        random: (110_000_000, 1.5),
        energetic: (0.5, 9.0),
    },
    Row {
        room: "5x4x3",
        alpha: 0.05,
        random: (3_500_000, 3.0),
        energetic: (3.0, 7.0),
    },
    Row {
        room: "5x4x3",
        alpha: 0.1,
        random: (7_900_000, 2.0),
        energetic: (2.0, 7.0),
    },
    Row {
        room: "5x4x3",
        alpha: 0.2,
        random: (14_000_000, 1.5),
        energetic: (0.8, 9.0),
    },
    Row {
        room: "5x4x3",
        alpha: 0.4,
        random: (31_000_000, 1.0),
        energetic: (0.4, 9.0),
    },
    Row {
        room: "20x8x4",
        alpha: 0.05,
        random: (1_500_000, 5.7),
        energetic: (5.7, 7.0),
    },
    Row {
        room: "20x8x4",
        alpha: 0.1,
        random: (1_500_000, 4.3),
        energetic: (2.9, 7.0),
    },
    Row {
        room: "20x8x4",
        alpha: 0.2,
        random: (1_500_000, 2.9),
        energetic: (1.5, 9.0),
    },
    Row {
        room: "20x8x4",
        alpha: 0.4,
        random: (1_500_000, 2.2),
        energetic: (0.8, 9.0),
    },
];

/// Energetic particles per source in every cell (section 2.3).
const ENERGETIC_PARTICLES: u32 = 1_500_000;
/// `trans_epsilon` of every random cell: inert there, set ≥ 7 as every bed file (section 2.3).
const RANDOM_EPSILON: f64 = 7.0;

impl BedFile {
    /// M8a's matrix: `docs/investigations/2026-09-29-m8a/SPEC.md`, sections 2 and 7.
    pub fn m8a() -> BedFile {
        let rooms = vec![
            Room {
                name: "6x10x3".into(),
                size_m: [6.0, 10.0, 3.0],
                source_m: [3.0, 5.0, 1.8],
                receivers_m: vec![[1.0, 1.0, 1.8], [3.0, 7.0, 1.8], [5.0, 8.5, 1.2]],
                gated: true,
            },
            Room {
                name: "5x4x3".into(),
                size_m: [5.0, 4.0, 3.0],
                source_m: [2.52, 1.97, 1.53],
                receivers_m: vec![[1.0, 1.0, 1.0], [4.0, 3.0, 2.0], [1.0, 3.0, 1.9]],
                gated: true,
            },
            Room {
                name: "20x8x4".into(),
                size_m: [20.0, 8.0, 4.0],
                source_m: [4.02, 3.97, 1.53],
                receivers_m: vec![[2.0, 2.0, 1.2], [10.0, 6.0, 1.8], [18.0, 3.0, 2.5]],
                gated: false,
            },
        ];
        let mut cells = Vec::new();
        let mut tcr = Vec::new();
        for room in &rooms {
            // The reported room runs air off only (the raw gate lists it in the first table).
            let airs: &[bool] = if room.gated { &[false, true] } else { &[false] };
            for &air in airs {
                for row in ROWS.iter().filter(|r| r.room == room.name) {
                    for method in [Method::Random, Method::Energetic] {
                        let (particles, duration, eps) = match method {
                            Method::Random => (row.random.0, row.random.1, RANDOM_EPSILON),
                            Method::Energetic => {
                                (ENERGETIC_PARTICLES, row.energetic.0, row.energetic.1)
                            }
                        };
                        cells.push(Cell {
                            id: cell_id(row.room, row.alpha, method, air),
                            room: row.room.into(),
                            alpha: row.alpha,
                            method,
                            particles_per_source: particles,
                            duration_s: duration,
                            trans_epsilon: eps,
                            air,
                            gated: room.gated,
                        });
                    }
                    tcr.push(TcrCell {
                        id: tcr_id(row.room, row.alpha, air),
                        project_of: cell_id(row.room, row.alpha, Method::Energetic, air),
                        gated: room.gated,
                    });
                }
            }
        }
        BedFile {
            bed: "m8a".into(),
            spec: "docs/investigations/2026-09-29-m8a/SPEC.md".into(),
            template: "tests/fixtures/rooms/tutorial1_box.simpa".into(),
            template_sha256: "e32acbf09c588abf7392f062093488389bd699c2fd22b95ee6b90703a72f3857"
                .into(),
            seeds: (1..=10).collect(),
            extension_seeds: (11..=20).collect(),
            time_step_s: 0.001,
            bands_air_off_hz: vec![125, 250, 500, 1000, 2000, 4000],
            bands_air_on_hz: vec![125, 250, 500, 1000, 2000, 4000, 8000],
            atmosphere: Atmosphere {
                temperature_c: 20.0,
                relative_humidity_percent: 50.0,
                pressure_pa: 101_325.0,
            },
            rooms,
            cells,
            tcr,
            transport: TransportSettings {
                replicas: 16,
                rays_factor: 16.0,
                duration_eyring_factor: 1.4,
                seed_base: "0x6d3863656c6c0000".into(),
                seed_per_alpha: 1000.0,
                speed_of_sound_m_s: 343.2,
                receiver_radius_m: 0.31,
                time_step_s: 0.001,
            },
            atmospheric_validation: AtmosphericValidation {
                proj: "src/isimpa/resources/doc/validation/atmospheric_absorption/\
                       Validation_atmospheric_absorption.proj"
                    .into(),
                proj_sha256: "487e4d0ff19e69e14d60b065d110de3da4f30ee812f2bd5006163379ff1e2e5b"
                    .into(),
                xlsx: "src/isimpa/resources/doc/validation/atmospheric_absorption/\
                       Validation_atmospheric_absorption.xlsx"
                    .into(),
                xlsx_sha256: "bb11404ea70b147bb33f01e78311f4c11977f9252c585143d20aa7dc944d1062"
                    .into(),
                seeds: (1..=10).collect(),
                trans_epsilon: 7.0,
                time_step_s: 0.001,
                columns: "spps_2018_rt30_s and spps_1_3_4_rt30_s: sheet 'SPPS' (2), rows 33-59, \
                          columns B ('SPPS 2018') and C ('SPPS 1.3.4') under 'RT30 (s)'; \
                          tcr_eyring_s: sheet 'TCR' (3), rows 33-59, column B under 'RT Eyring \
                          (s)'; md_octave_rt30_s: sheet 'MD' (4), rows 33-59, column B ('MD \
                          Octave') under 'RT30 (s)'. Made by upstream at seed 0, dt 10 ms and \
                          trans_epsilon 6: not like for like"
                    .into(),
                bands_hz: XLSX_BANDS_HZ.to_vec(),
                spps_2018_rt30_s: XLSX_SPPS_2018.to_vec(),
                spps_1_3_4_rt30_s: XLSX_SPPS_1_3_4.to_vec(),
                tcr_eyring_s: XLSX_TCR_EYRING.to_vec(),
                md_octave_rt30_s: XLSX_MD_OCTAVE.to_vec(),
            },
            say_no: SayNo {
                n5: SayNoN5 {
                    cell: cell_id("5x4x3", 0.4, Method::Random, false),
                    seed: 10,
                    particles_per_source: 31_000,
                },
                n6: SayNoN6 {
                    room: "5x4x3".into(),
                    alpha: 0.2,
                    method: Method::Energetic,
                    particles_per_source: 150_000,
                    duration_s: 0.8,
                    trans_epsilon: 9.0,
                    scattering: 0.0,
                    seed: 1,
                },
            },
        }
    }

    pub fn room(&self, name: &str) -> Option<&Room> {
        self.rooms.iter().find(|r| r.name == name)
    }

    pub fn cell(&self, id: &str) -> Option<&Cell> {
        self.cells.iter().find(|c| c.id == id)
    }

    /// The bands of a cell with air `air`.
    pub fn bands_hz(&self, air: bool) -> &[u32] {
        if air {
            &self.bands_air_on_hz
        } else {
            &self.bands_air_off_hz
        }
    }

    /// What makes the file unusable, whatever its matrix: ids that repeat or refer to nothing,
    /// seeds that are 0 or repeat (SB-2), bands that are not octaves.
    pub fn problems(&self) -> Vec<String> {
        let mut out = Vec::new();
        let mut ids = std::collections::BTreeSet::new();
        for c in &self.cells {
            if !ids.insert(c.id.clone()) {
                out.push(format!("cell id '{}' repeats", c.id));
            }
            if self.room(&c.room).is_none() {
                out.push(format!("cell '{}': no room '{}'", c.id, c.room));
            }
            if !(0.0..1.0).contains(&c.alpha) || c.alpha <= 0.0 {
                out.push(format!("cell '{}': alpha {} not in (0, 1)", c.id, c.alpha));
            }
            if c.particles_per_source == 0 || c.duration_s.is_nan() || c.duration_s <= 0.0 {
                out.push(format!("cell '{}': no particles or no duration", c.id));
            }
        }
        for t in &self.tcr {
            if !ids.insert(t.id.clone()) {
                out.push(format!("TCR id '{}' repeats", t.id));
            }
            if self.cell(&t.project_of).is_none() {
                out.push(format!("TCR '{}': no cell '{}'", t.id, t.project_of));
            }
        }
        for (what, seeds) in [
            ("seeds", &self.seeds),
            ("extension_seeds", &self.extension_seeds),
            (
                "atmospheric_validation.seeds",
                &self.atmospheric_validation.seeds,
            ),
        ] {
            let set: std::collections::BTreeSet<u32> = seeds.iter().copied().collect();
            if seeds.contains(&0) || set.len() != seeds.len() {
                out.push(format!(
                    "{what}: 0 or a repeat (SB-2 asks distinct non-zero seeds)"
                ));
            }
        }
        if self.seeds.len() < 2 {
            out.push("at least two seeds are needed for an interval".into());
        }
        if self.extension_seeds.iter().any(|s| self.seeds.contains(s)) {
            out.push("extension_seeds repeat seeds".into());
        }
        for bands in [&self.bands_air_off_hz, &self.bands_air_on_hz] {
            let ok = bands.first().zip(bands.last()).is_some_and(|(&lo, &hi)| {
                BandSet::range(BandKind::Octave, lo, hi)
                    .is_some_and(|b| b.frequencies_hz.as_slice() == bands.as_slice())
            });
            if !ok {
                out.push(format!(
                    "bands {bands:?} are not the octaves from first to last"
                ));
            }
        }
        for r in &self.rooms {
            if r.receivers_m.is_empty() {
                out.push(format!("room '{}' has no receivers", r.name));
            }
        }
        if self.cell(&self.say_no.n5.cell).is_none() {
            out.push(format!("say_no.n5: no cell '{}'", self.say_no.n5.cell));
        }
        if self.room(&self.say_no.n6.room).is_none() {
            out.push(format!("say_no.n6: no room '{}'", self.say_no.n6.room));
        }
        if let Err(e) = self.transport.seed(0.1) {
            out.push(e);
        }
        out
    }
}

/// The bed file's canonical text: pretty JSON, `\n` line ends, a final newline.
pub fn canonical_text(bed: &BedFile) -> String {
    serde_json::to_string_pretty(bed).expect("a bed file serialises") + "\n"
}

/// A bed file as read: the file, and whether it is M8a's matrix.
#[derive(Clone, Debug)]
pub struct Loaded {
    pub bed: BedFile,
    /// The file as given.
    pub raw_sha256: String,
    /// Its canonical text's.
    pub canonical_sha256: String,
    /// Why it is exploratory; empty exactly when it is M8a's matrix (E7).
    pub exploratory: Vec<String>,
}

/// Reads and checks a bed file. Refused for a file that does not parse, or has
/// [`BedFile::problems`]; any other file that is not the matrix of [`BedFile::m8a`] is loaded as
/// exploratory, with the reasons.
pub fn load(path: &std::path::Path) -> Result<Loaded, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    parse(&bytes).map_err(|e| format!("{}: {e}", path.display()))
}

/// [`load`] on the file's bytes.
pub fn parse(bytes: &[u8]) -> Result<Loaded, String> {
    use super::pe::sha256_hex;
    let text = std::str::from_utf8(bytes).map_err(|e| e.to_string())?;
    let bed: BedFile = schema::from_json_exact(text).map_err(|e| e.to_string())?;
    let problems = bed.problems();
    if !problems.is_empty() {
        return Err(problems.join("; "));
    }
    let canonical = canonical_text(&bed);
    let mut exploratory = Vec::new();
    if canonical != canonical_text(&BedFile::m8a()) {
        exploratory.push(
            "the file is not M8a's matrix in its canonical form (simpa bed --canonical prints it)"
                .to_string(),
        );
    }
    let template = sha256_hex(super::TEMPLATE.as_bytes());
    if template != bed.template_sha256 {
        exploratory.push(format!(
            "the template compiled into this build has sha256 {template}, not the file's {}",
            bed.template_sha256
        ));
    }
    Ok(Loaded {
        raw_sha256: sha256_hex(bytes),
        canonical_sha256: sha256_hex(canonical.as_bytes()),
        bed,
        exploratory,
    })
}

// ---------------------------------------------------------------------------------------------
// Projects

/// What a run's project is made of: one of the bed's cells, or a say-NO run's variant of one.
#[derive(Clone, Debug, PartialEq)]
pub struct CellSpec<'a> {
    pub room: &'a Room,
    pub alpha: f64,
    pub method: Method,
    pub particles_per_source: u32,
    pub duration_s: f64,
    pub trans_epsilon: f64,
    pub air: bool,
    /// Every face's scattering: 1 in the matrix, 0 in N6.
    pub scattering: f64,
}

impl<'a> CellSpec<'a> {
    pub fn of(bed: &'a BedFile, cell: &Cell) -> Result<CellSpec<'a>, String> {
        Ok(CellSpec {
            room: bed
                .room(&cell.room)
                .ok_or(format!("no room '{}'", cell.room))?,
            alpha: cell.alpha,
            method: cell.method,
            particles_per_source: cell.particles_per_source,
            duration_s: cell.duration_s,
            trans_epsilon: cell.trans_epsilon,
            air: cell.air,
            scattering: 1.0,
        })
    }
}

/// The template ([`super::TEMPLATE`], tutorial 1's box at upstream's defaults) on the octave
/// bands `bands_hz`, without its surface receiver and the intersection logs, seeded `seed`: as
/// `m8_evidence.rs`' `tutorial_octaves_to` makes it.
fn template_octaves(seed: u32, bands_hz: &[u32]) -> Result<Project, String> {
    let mut p = schema::from_json(super::TEMPLATE).map_err(|e| format!("the template: {e}"))?;
    let all = p.bands.frequencies_hz.clone();
    let pick = |v: &[schema::F64]| -> Result<Vec<schema::F64>, String> {
        bands_hz
            .iter()
            .map(|f| {
                all.iter()
                    .position(|a| a == f)
                    .map(|i| v[i])
                    .ok_or(format!("the template has no band {f} Hz"))
            })
            .collect()
    };
    for m in &mut p.materials {
        m.absorption = pick(&m.absorption)?;
        m.scattering = pick(&m.scattering)?;
    }
    let (lo, hi) = (bands_hz[0], bands_hz[bands_hz.len() - 1]);
    p.bands = BandSet::range(BandKind::Octave, lo, hi)
        .ok_or(format!("no octave bands from {lo} to {hi} Hz"))?;
    p.surface_receivers.clear();
    let s = &mut p.solvers.spps;
    s.random_seed = seed;
    s.save_surface_intersections = false;
    s.save_receiver_intersections = false;
    s.bands_computed = vec![true; bands_hz.len()];
    p.solvers.tcr.bands_computed = vec![true; bands_hz.len()];
    Ok(p)
}

/// `p`'s point receivers replaced by one per position, named `R000`, `R001`, ...
fn with_receivers(p: &mut Project, at: &[[f64; 3]]) {
    let proto = p.point_receivers[0].clone();
    p.point_receivers = at
        .iter()
        .enumerate()
        .map(|(i, x)| {
            let mut r = proto.clone();
            r.id =
                PointReceiverId::from_u128(0x0c0b_e000_0000_4000_8000_0000_0001_0000 + i as u128);
            r.name = format!("R{i:03}");
            r.position = Vec3::new(x[0], x[1], x[2]);
            r.solver_id = None;
            r
        })
        .collect();
}

/// The receivers' names, in order.
pub fn receiver_name(i: usize) -> String {
    format!("R{i:03}")
}

/// A cell's project, seeded `seed`, as `m8_evidence.rs`' `m8_cell` builds M8's cells: the
/// template scaled to the room, one surface group, one material of absorption `α` in every band,
/// Lambert with scattering `spec.scattering`, no transmission; the room's source and receivers;
/// SPPS in the cell's method, particles, duration, step `time_step_s` and `trans_epsilon`; air
/// for SPPS and TCR as the cell's.
pub fn cell_project(
    spec: &CellSpec,
    seed: u32,
    time_step_s: f64,
    bands_hz: &[u32],
) -> Result<Project, String> {
    let mut p = template_octaves(seed, bands_hz)?;
    let [lx, ly, lz] = spec.room.size_m;
    p.geometry.vertices = p
        .geometry
        .vertices
        .iter()
        .map(|v| {
            let [x, y, z] = v.to_array();
            Vec3::new(x / 6.0 * lx, y / 10.0 * ly, z / 3.0 * lz)
        })
        .collect();
    let group = GroupId::from_u128(0x0c0b_e000_0000_4000_8000_0000_0008_0001);
    let mat = MaterialId::from_u128(0x0c0b_e000_0000_4000_8000_0000_0008_0002);
    p.geometry.faces = p
        .geometry
        .faces
        .iter()
        .map(|f| Face {
            vertices: f.vertices,
            group,
        })
        .collect();
    let n = p.bands.len();
    let mut m = p.materials[0].clone();
    m.id = mat;
    m.name = format!("alpha {}", spec.alpha);
    m.absorption = vec![schema::F64::new(spec.alpha); n];
    m.scattering = vec![schema::F64::new(spec.scattering); n];
    m.reflection_law = ReflectionLaw::Lambert.into();
    m.transmission_loss_db = None;
    m.solver_id = None;
    p.materials = vec![m];
    p.surface_groups = vec![schema::SurfaceGroup {
        id: group,
        name: "Surfaces".into(),
        material: mat,
    }];
    let s = spec.room.source_m;
    p.sources[0].position = Vec3::new(s[0], s[1], s[2]);
    with_receivers(&mut p, &spec.room.receivers_m);
    let spps = &mut p.solvers.spps;
    spps.method = spec.method.schema();
    spps.particles_per_source = spec.particles_per_source;
    spps.duration_s = schema::F64::new(spec.duration_s);
    spps.time_step_s = schema::F64::new(time_step_s);
    spps.extinction_exponent = schema::F64::new(spec.trans_epsilon);
    spps.air_absorption = spec.air;
    p.solvers.tcr.air_absorption = spec.air;
    Ok(p)
}

/// Upstream's atmospheric-absorption validation project, imported from `proj` and changed in
/// the three ways section 2.5 names: the seed, `trans_epsilon` and the step.
pub fn atmospheric_project(
    proj: &std::path::Path,
    v: &AtmosphericValidation,
    seed: u32,
) -> Result<Project, String> {
    let bytes = std::fs::read(proj).map_err(|e| format!("{}: {e}", proj.display()))?;
    let sha = super::pe::sha256_hex(&bytes);
    if sha != v.proj_sha256 {
        return Err(format!(
            "{}: sha256 {sha}, not the bed file's {}",
            proj.display(),
            v.proj_sha256
        ));
    }
    let mut imported = crate::geometry::import::import_proj(&bytes)
        .map_err(|e| format!("{}: {} ({e})", proj.display(), e.code()))?;
    let p = &mut imported.project;
    p.name = "Validation_atmospheric_absorption".into();
    p.solvers.spps.random_seed = seed;
    p.solvers.spps.extinction_exponent = schema::F64::new(v.trans_epsilon);
    p.solvers.spps.time_step_s = schema::F64::new(v.time_step_s);
    Ok(imported.project)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_matrix_is_the_specs() {
        let b = BedFile::m8a();
        assert!(b.problems().is_empty(), "{:?}", b.problems());
        let gated: Vec<&Cell> = b.cells.iter().filter(|c| c.gated).collect();
        // 2 rooms × 4 α × 2 methods × 2 air: 32 gated cells; 8 reported (20×8×4 m, air off).
        assert_eq!(gated.len(), 32);
        assert_eq!(b.cells.len(), 40);
        assert!(b.cells.iter().filter(|c| !c.gated).all(|c| !c.air));
        assert_eq!(b.tcr.iter().filter(|t| t.gated).count(), 16);
        assert_eq!(b.tcr.len(), 20);
        assert_eq!(b.seeds, (1..=10).collect::<Vec<_>>());
        // Section 2.3's table, spot checks: the largest random count, the energetic counts.
        let c = b.cell("6x10x3-a0.4-random-air-on").unwrap();
        assert_eq!(
            (c.particles_per_source, c.duration_s, c.trans_epsilon),
            (110_000_000, 1.5, 7.0)
        );
        let e = b.cell("5x4x3-a0.2-energetic-air-off").unwrap();
        assert_eq!(
            (e.particles_per_source, e.duration_s, e.trans_epsilon),
            (1_500_000, 0.8, 9.0)
        );
        assert!(
            b.cells
                .iter()
                .filter(|c| c.method == Method::Energetic)
                .all(|c| c.particles_per_source == 1_500_000)
        );
        // E4 and E5 need all eight of M8's air-off cells gated.
        for h in crate::bed::transport::HIGH {
            let room = crate::bed::transport::HIGH_ROOMS[h.room];
            assert!(
                b.cells
                    .iter()
                    .any(|c| c.gated && !c.air && c.room == room && c.alpha == h.alpha)
            );
        }
        // The transport's rays and seeds as the high-count test computes them.
        assert_eq!(b.transport.rays_per_replica(0.05), 262_144);
        assert_eq!(b.transport.rays_per_replica(0.4), 2_097_152);
        assert_eq!(b.transport.seed(0.05).unwrap(), 0x6d38_6365_6c6c_0000 + 50);
        assert_eq!(b.transport.seed(0.4).unwrap(), 0x6d38_6365_6c6c_0000 + 400);
    }

    #[test]
    fn the_committed_file_is_the_matrix() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../beds/m8a.json");
        let l = load(&path).unwrap();
        assert!(l.exploratory.is_empty(), "{:?}", l.exploratory);
    }

    #[test]
    fn any_change_to_the_matrix_makes_the_bed_exploratory() {
        let canonical = canonical_text(&BedFile::m8a());
        let l = parse(canonical.as_bytes()).unwrap();
        assert!(l.exploratory.is_empty());
        // Line ends and spacing do not count.
        let crlf = canonical.replace('\n', "\r\n");
        assert!(parse(crlf.as_bytes()).unwrap().exploratory.is_empty());
        // Says no: one particle count, one seed, one receiver moved.
        let mut b = BedFile::m8a();
        b.cells[0].particles_per_source += 1;
        assert!(
            !parse(canonical_text(&b).as_bytes())
                .unwrap()
                .exploratory
                .is_empty()
        );
        let mut b = BedFile::m8a();
        b.seeds.pop();
        assert!(
            !parse(canonical_text(&b).as_bytes())
                .unwrap()
                .exploratory
                .is_empty()
        );
        let mut b = BedFile::m8a();
        b.rooms[1].receivers_m[0][0] = 1.01;
        assert!(
            !parse(canonical_text(&b).as_bytes())
                .unwrap()
                .exploratory
                .is_empty()
        );
        // A file with a seed 0 or an unknown field is refused, not run.
        let mut b = BedFile::m8a();
        b.seeds[0] = 0;
        assert!(parse(canonical_text(&b).as_bytes()).is_err());
        let extra = canonical.replacen("{", "{\n  \"tolerance\": 0.1,", 1);
        assert!(parse(extra.as_bytes()).is_err());
    }

    /// With `$SIMPA_M8A_MEASURED_PROJECT` naming the `project.simpa` of the cell section 8 of the
    /// spec measured (`m8_evidence.rs`' `m8_cell`: 6×10×3 m, α 0.05, energetic, 1.5 M, 4 s, 1 ms,
    /// ε 7, air off, seed 1), the bed builds that cell's project byte for byte. Skipped without it.
    #[test]
    fn a_cells_project_is_the_measured_cells_byte_for_byte() {
        let Some(path) = std::env::var_os("SIMPA_M8A_MEASURED_PROJECT") else {
            println!("SIMPA_M8A_MEASURED_PROJECT not set: skipped");
            return;
        };
        let b = BedFile::m8a();
        let cell = b.cell("6x10x3-a0.05-energetic-air-off").unwrap();
        let p = cell_project(
            &CellSpec::of(&b, cell).unwrap(),
            1,
            0.001,
            b.bands_hz(false),
        )
        .unwrap();
        let measured = schema::load(std::path::Path::new(&path)).unwrap();
        assert_eq!(schema::to_json(&p), schema::to_json(&measured));
    }

    /// With `$SIMPA_UPSTREAM` naming upstream's tree, its validation project imports as section
    /// 2.5 found it, with the three changes. Skipped without it.
    #[test]
    fn the_atmospheric_validation_imports_as_the_spec_saw_it() {
        let Some(root) = std::env::var_os("SIMPA_UPSTREAM") else {
            println!("SIMPA_UPSTREAM not set: skipped");
            return;
        };
        let b = BedFile::m8a();
        let v = &b.atmospheric_validation;
        let p = atmospheric_project(&std::path::Path::new(&root).join(&v.proj), v, 4).unwrap();
        assert_eq!(p.bands.frequencies_hz, v.bands_hz);
        assert_eq!(p.geometry.faces.len(), 12);
        assert_eq!(p.point_receivers.len(), 1);
        assert_eq!(p.point_receivers[0].position.to_array(), [1.0, 1.0, 1.0]);
        assert_eq!(p.sources[0].position.to_array(), [2.5, 2.0, 1.5]);
        let s = &p.solvers.spps;
        assert_eq!(s.method, ComputationMethod::Energetic);
        assert_eq!(s.particles_per_source, 500_000);
        assert_eq!(s.duration_s.get(), 2.0);
        assert_eq!(
            (
                s.random_seed,
                s.extinction_exponent.get(),
                s.time_step_s.get()
            ),
            (4, 7.0, 0.001)
        );
        // Says no: a .proj of another sha256 is refused.
        let mut other = v.clone();
        other.proj_sha256 = "0".repeat(64);
        assert!(
            atmospheric_project(&std::path::Path::new(&root).join(&v.proj), &other, 1).is_err()
        );
    }

    #[test]
    fn a_cells_project_is_the_evidence_runs() {
        let b = BedFile::m8a();
        let cell = b.cell("5x4x3-a0.4-energetic-air-on").unwrap();
        let p = cell_project(&CellSpec::of(&b, cell).unwrap(), 3, 0.001, b.bands_hz(true)).unwrap();
        assert_eq!(
            p.bands.frequencies_hz,
            vec![125, 250, 500, 1000, 2000, 4000, 8000]
        );
        assert_eq!(p.materials.len(), 1);
        assert!(p.materials[0].absorption.iter().all(|a| a.get() == 0.4));
        assert!(p.materials[0].scattering.iter().all(|a| a.get() == 1.0));
        assert_eq!(p.point_receivers.len(), 3);
        assert_eq!(p.point_receivers[2].position.to_array(), [1.0, 3.0, 1.9]);
        assert_eq!(p.sources[0].position.to_array(), [2.52, 1.97, 1.53]);
        let s = &p.solvers.spps;
        assert_eq!(s.random_seed, 3);
        assert_eq!(s.particles_per_source, 1_500_000);
        assert_eq!(s.time_step_s.get(), 0.001);
        assert_eq!(s.extinction_exponent.get(), 9.0);
        assert!(s.air_absorption && p.solvers.tcr.air_absorption);
        let max = p.geometry.vertices.iter().fold([0.0f64; 3], |m, v| {
            let a = v.to_array();
            [m[0].max(a[0]), m[1].max(a[1]), m[2].max(a[2])]
        });
        assert_eq!(max, [5.0, 4.0, 3.0]);
        let issues = crate::validate::validate(&p);
        assert!(!crate::validate::has_errors(&issues), "{issues:?}");
    }
}
