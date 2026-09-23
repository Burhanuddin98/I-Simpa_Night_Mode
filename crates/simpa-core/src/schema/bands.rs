//! The project's one band set, and level spectra over it.

use schemars::JsonSchema;
use serde::{Deserialize, Deserializer, Serialize};

use super::real::F64;

/// Nominal third-octave centre frequencies the project can use, in Hz: I-Simpa's own range,
/// 50 Hz to 20 kHz (tutorial 1's `freq_enum`). Index `i` has the exact base-10 centre
/// `1000 * 10^((i - 13) / 10)` Hz (IEC 61260-1).
const THIRD_OCTAVE_HZ: [u32; 27] = [
    50, 63, 80, 100, 125, 160, 200, 250, 315, 400, 500, 630, 800, 1000, 1250, 1600, 2000, 2500,
    3150, 4000, 5000, 6300, 8000, 10000, 12500, 16000, 20000,
];

/// Nominal octave centre frequencies, in Hz. Each is also a third-octave nominal.
const OCTAVE_HZ: [u32; 9] = [63, 125, 250, 500, 1000, 2000, 4000, 8000, 16000];

/// Octave or third-octave bands.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum BandKind {
    Octave,
    ThirdOctave,
}

impl BandKind {
    /// Every nominal frequency this kind allows, ascending.
    pub fn nominal_frequencies(self) -> &'static [u32] {
        match self {
            BandKind::Octave => &OCTAVE_HZ,
            BandKind::ThirdOctave => &THIRD_OCTAVE_HZ,
        }
    }

    /// The band number `k` of a nominal frequency: the exact centre is `1000 * 10^(k/10)` Hz.
    /// `k` counts third-octave steps, so it moves by 3 per octave band. `None` if `nominal_hz`
    /// is not one of [`BandKind::nominal_frequencies`].
    pub fn band_number(self, nominal_hz: u32) -> Option<i32> {
        let i = THIRD_OCTAVE_HZ.iter().position(|&f| f == nominal_hz)? as i32;
        let k = i - 13;
        match self {
            BandKind::ThirdOctave => Some(k),
            BandKind::Octave => OCTAVE_HZ.contains(&nominal_hz).then_some(k),
        }
    }

    /// The exact base-10 centre frequency of a nominal frequency, in Hz.
    pub fn exact_centre_hz(self, nominal_hz: u32) -> Option<f64> {
        self.band_number(nominal_hz)
            .map(|k| 1000.0 * 10f64.powf(f64::from(k) / 10.0))
    }
}

/// The frequency bands every per-band value in the project is given on.
///
/// `frequencies_hz` holds nominal centre frequencies of `kind`, strictly ascending. The solvers
/// sort `freq_enum` and map every per-band array by sorted position, so ascending order here is
/// the order of every per-band array in the project.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BandSet {
    pub kind: BandKind,
    pub frequencies_hz: Vec<u32>,
}

impl BandSet {
    /// Every band of `kind` from `lowest_hz` to `highest_hz` inclusive. `None` if either end is
    /// not a nominal frequency of `kind` or the range is empty.
    pub fn range(kind: BandKind, lowest_hz: u32, highest_hz: u32) -> Option<Self> {
        let all = kind.nominal_frequencies();
        let lo = all.iter().position(|&f| f == lowest_hz)?;
        let hi = all.iter().position(|&f| f == highest_hz)?;
        (lo <= hi).then(|| BandSet {
            kind,
            frequencies_hz: all[lo..=hi].to_vec(),
        })
    }

    pub fn len(&self) -> usize {
        self.frequencies_hz.len()
    }

    pub fn is_empty(&self) -> bool {
        self.frequencies_hz.is_empty()
    }

    /// Position of a nominal frequency in this set.
    pub fn index_of(&self, nominal_hz: u32) -> Option<usize> {
        self.frequencies_hz.iter().position(|&f| f == nominal_hz)
    }

    /// Band numbers `k` (see [`BandKind::band_number`]); `None` if a frequency is not nominal.
    pub fn band_numbers(&self) -> Option<Vec<i32>> {
        self.frequencies_hz
            .iter()
            .map(|&f| self.kind.band_number(f))
            .collect()
    }

    /// Why this set is not usable, if it is not: empty, a frequency that is not a nominal
    /// frequency of `kind`, or not strictly ascending.
    pub fn problem(&self) -> Option<String> {
        if self.frequencies_hz.is_empty() {
            return Some("the band set is empty".to_string());
        }
        if let Some(f) = self
            .frequencies_hz
            .iter()
            .find(|&&f| self.kind.band_number(f).is_none())
        {
            return Some(format!("{f} Hz is not a nominal {:?} frequency", self.kind));
        }
        if let Some(w) = self.frequencies_hz.windows(2).find(|w| w[0] >= w[1]) {
            return Some(format!(
                "frequencies are not strictly ascending: {} Hz then {} Hz",
                w[0], w[1]
            ));
        }
        None
    }
}

impl Default for BandSet {
    /// Octave bands 125 Hz to 4 kHz.
    fn default() -> Self {
        BandSet {
            kind: BandKind::Octave,
            frequencies_hz: vec![125, 250, 500, 1000, 2000, 4000],
        }
    }
}

/// The shape of a level spectrum across the project's bands.
///
/// In JSON: `{"kind": "pink"}`, `{"kind": "white"}` or `{"kind": "custom", "relative_db": [...]}`.
/// Extra keys are refused for every kind: serde's derive alone would read
/// `{"kind": "pink", "relative_db": [...]}` as pink and drop the levels, so the reader is written
/// by hand.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum SpectrumShape {
    /// Equal power in every band.
    Pink,
    /// Power proportional to bandwidth: +1 dB per third-octave band and +3 dB per octave band,
    /// exactly (base-10 bands). This is I-Simpa's "White noise": tutorial 1's source, 80 dB
    /// over 50 Hz to 20 kHz in thirds, reads 60.1404190063477 dB at 1 kHz, and this shape gives
    /// 60.14042 dB.
    White,
    /// Relative levels in dB, one per band. Only the differences matter.
    Custom { relative_db: Vec<F64> },
}

/// [`SpectrumShape`] as read from a file. serde ignores extra keys in a *unit* variant of an
/// internally tagged enum, `deny_unknown_fields` or not, so the reader spells the unit variants
/// as empty struct variants, whose keys it does check. The JSON is the same.
#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum SpectrumShapeIn {
    Pink {},
    White {},
    Custom { relative_db: Vec<F64> },
}

impl<'de> Deserialize<'de> for SpectrumShape {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Ok(match SpectrumShapeIn::deserialize(deserializer)? {
            SpectrumShapeIn::Pink {} => SpectrumShape::Pink,
            SpectrumShapeIn::White {} => SpectrumShape::White,
            SpectrumShapeIn::Custom { relative_db } => SpectrumShape::Custom { relative_db },
        })
    }
}

/// A level spectrum: a global level spread over the project's bands by a shape.
///
/// Used for source sound power (dB re 1 pW) and receiver background noise.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Spectrum {
    /// The energetic sum over the project's bands, in dB.
    pub global_db: F64,
    pub shape: SpectrumShape,
}

impl Spectrum {
    pub fn new(global_db: f64, shape: SpectrumShape) -> Self {
        Spectrum {
            global_db: F64::new(global_db),
            shape,
        }
    }

    /// Relative levels of the shape on `bands`, in dB. `None` if `bands` has a non-nominal
    /// frequency or a custom shape has the wrong length.
    pub fn relative_db(&self, bands: &BandSet) -> Option<Vec<f64>> {
        let numbers = bands.band_numbers()?;
        match &self.shape {
            SpectrumShape::Pink => Some(vec![0.0; numbers.len()]),
            SpectrumShape::White => Some(numbers.iter().map(|&k| f64::from(k)).collect()),
            SpectrumShape::Custom { relative_db } => (relative_db.len() == numbers.len())
                .then(|| relative_db.iter().map(|v| v.get()).collect()),
        }
    }

    /// The level in each band, in dB, such that the bands sum energetically to `global_db`:
    /// `L_i = global + r_i - 10 log10(sum_j 10^(r_j / 10))`. These are the values config.xml
    /// writes as `bfreq@db`. `None` as for [`Spectrum::relative_db`], or for an empty set.
    pub fn band_levels_db(&self, bands: &BandSet) -> Option<Vec<f64>> {
        let relative = self.relative_db(bands)?;
        if relative.is_empty() {
            return None;
        }
        let total: f64 = relative.iter().map(|r| 10f64.powf(r / 10.0)).sum();
        let offset = self.global_db.get() - 10.0 * total.log10();
        Some(relative.iter().map(|r| r + offset).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn octave_nominals_are_third_octave_nominals_three_steps_apart() {
        let ks: Vec<i32> = OCTAVE_HZ
            .iter()
            .map(|&f| BandKind::Octave.band_number(f).unwrap())
            .collect();
        assert_eq!(ks, vec![-12, -9, -6, -3, 0, 3, 6, 9, 12]);
        assert_eq!(BandKind::Octave.band_number(800), None);
        assert_eq!(BandKind::ThirdOctave.band_number(800), Some(-1));
    }

    #[test]
    fn exact_centres_round_to_their_nominals() {
        for kind in [BandKind::Octave, BandKind::ThirdOctave] {
            for &f in kind.nominal_frequencies() {
                let exact = kind.exact_centre_hz(f).unwrap();
                assert!(
                    (exact / f64::from(f) - 1.0).abs() < 0.03,
                    "{f} Hz vs {exact}"
                );
            }
        }
    }

    #[test]
    fn band_levels_sum_to_the_global_level() {
        let bands = BandSet::range(BandKind::ThirdOctave, 50, 20000).unwrap();
        for shape in [SpectrumShape::Pink, SpectrumShape::White] {
            let levels = Spectrum::new(93.5, shape).band_levels_db(&bands).unwrap();
            let sum: f64 = levels.iter().map(|l| 10f64.powf(l / 10.0)).sum();
            assert!((10.0 * sum.log10() - 93.5).abs() < 1e-9);
        }
    }
}
