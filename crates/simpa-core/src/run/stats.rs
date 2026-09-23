//! SPPS particle statistics: the GABE table at `simulation@stats_filename`, typed.
//!
//! `ReportManager::SaveThreadsStats` writes it (`spps/input_output/reportmanager.cpp:482-517`):
//! a short-string column holding the seven row labels in the order of [`ROW_LABELS`], then one
//! 7-row integer column per computed band, labelled `<f> Hz`, in band order
//! (`reportmanager.cpp:344-358`). Each row counts the particles of that band that ended in that
//! state; `Total` counts every particle run, energetic-mode transmission copies included.

use std::fmt;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::formats::FormatError;
use crate::formats::gabe::{self, ColumnData, Gabe};

/// The row labels, in upstream's order (`spps/input_output/reportmanager.cpp:489-496`).
pub const ROW_LABELS: [&str; 7] = [
    "Particles absorbed by the atmosphere",
    "Particles absorbed by the materials",
    "Particles absorbed by the fittings",
    "Particles lost by infinite loops",
    "Particles lost by meshing problems",
    "Particles remaining at the end of the calculation",
    "Total",
];

/// One band's column. The field order is the row order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BandStats {
    /// The band, from the column label `<f> Hz`.
    pub freq_hz: i32,
    /// `partAbsAtmo`.
    pub absorbed_by_atmosphere: u32,
    /// `partAbsSurf`.
    pub absorbed_by_materials: u32,
    /// `partAbsEncombrement`.
    pub absorbed_by_fittings: u32,
    /// `partLoop`.
    pub lost_by_infinite_loops: u32,
    /// `partLost`.
    pub lost_by_meshing_problems: u32,
    /// `partAlive`.
    pub remaining: u32,
    /// `partTotal`.
    pub total: u32,
}

impl BandStats {
    /// The particles in error: lost by infinite loops plus lost by meshing problems, the two
    /// counts SPPS's own loss warning adds (`sppsNantes.cpp:425-439`).
    pub fn lost(&self) -> u64 {
        u64::from(self.lost_by_infinite_loops) + u64::from(self.lost_by_meshing_problems)
    }

    /// [`BandStats::lost`] over the total; `None` when the total is 0.
    pub fn loss_ratio(&self) -> Option<f64> {
        (self.total > 0).then(|| self.lost() as f64 / f64::from(self.total))
    }
}

/// The whole table: one entry per band column, in file order.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ParticleStats {
    pub bands: Vec<BandStats>,
}

impl ParticleStats {
    /// The band of each column, in file order (duplicates kept).
    pub fn freqs(&self) -> Vec<i32> {
        self.bands.iter().map(|b| b.freq_hz).collect()
    }
}

/// Why a statistics table could not be read: the reason `stats_unreadable`.
#[derive(Debug)]
pub enum StatsError {
    /// The file is missing or is not a GABE table.
    Format(FormatError),
    /// A GABE table, but not laid out as `SaveThreadsStats` writes it.
    Layout(String),
}

impl fmt::Display for StatsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StatsError::Format(e) => write!(f, "{e}"),
            StatsError::Layout(m) => write!(f, "not SPPS's statistics layout: {m}"),
        }
    }
}

impl std::error::Error for StatsError {}

/// The band a column label names: `125 Hz` is 125 (`CoreString::FromInt` then ` Hz`).
pub fn band_label(label: &[u8]) -> Option<i32> {
    let digits = label.strip_suffix(b" Hz")?;
    let s = std::str::from_utf8(digits).ok()?;
    let body = s.strip_prefix('-').unwrap_or(s);
    if body.is_empty() || !body.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    s.parse().ok()
}

/// Types a decoded table.
pub fn from_gabe(g: &Gabe) -> Result<ParticleStats, StatsError> {
    fn layout(m: String) -> Result<ParticleStats, StatsError> {
        Err(StatsError::Layout(m))
    }
    let Some(labels) = g.row_labels() else {
        return layout("the first column is not the short-string row labels".into());
    };
    let want: Vec<&[u8]> = ROW_LABELS.iter().map(|l| l.as_bytes()).collect();
    let got: Vec<&[u8]> = labels.iter().map(Vec::as_slice).collect();
    if got != want {
        return layout(format!(
            "the row labels are {:?}, not upstream's seven",
            got.iter()
                .map(|l| String::from_utf8_lossy(l))
                .collect::<Vec<_>>()
        ));
    }
    let mut bands = Vec::with_capacity(g.columns.len().saturating_sub(1));
    for (i, col) in g.columns.iter().enumerate().skip(1) {
        let name = String::from_utf8_lossy(&col.label);
        let Some(freq_hz) = band_label(col.name()) else {
            return layout(format!("column {i} is labelled {name:?}, not '<f> Hz'"));
        };
        let ColumnData::Int(v) = &col.data else {
            return layout(format!("column {i} ({name}) is not an integer column"));
        };
        if v.len() != ROW_LABELS.len() {
            return layout(format!("column {i} ({name}) has {} rows, not 7", v.len()));
        }
        let mut n = [0u32; 7];
        for (k, (&x, slot)) in v.iter().zip(&mut n).enumerate() {
            match u32::try_from(x) {
                Ok(x) => *slot = x,
                Err(_) => {
                    return layout(format!(
                        "column {i} ({name}) row '{}' is {x}, a negative count",
                        ROW_LABELS[k]
                    ));
                }
            }
        }
        bands.push(BandStats {
            freq_hz,
            absorbed_by_atmosphere: n[0],
            absorbed_by_materials: n[1],
            absorbed_by_fittings: n[2],
            lost_by_infinite_loops: n[3],
            lost_by_meshing_problems: n[4],
            remaining: n[5],
            total: n[6],
        });
    }
    Ok(ParticleStats { bands })
}

/// Reads and types a statistics file.
pub fn read_file(path: &Path) -> Result<ParticleStats, StatsError> {
    let g = gabe::read_file(path).map_err(StatsError::Format)?;
    from_gabe(&g)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn band_labels() {
        assert_eq!(band_label(b"125 Hz"), Some(125));
        assert_eq!(band_label(b"20000 Hz"), Some(20000));
        assert_eq!(band_label(b"-5 Hz"), Some(-5));
        for bad in [
            &b"125Hz"[..],
            b"125 hz",
            b" Hz",
            b"12.5 Hz",
            b"+5 Hz",
            b"125 Hz ",
        ] {
            assert_eq!(band_label(bad), None, "{}", String::from_utf8_lossy(bad));
        }
    }

    #[test]
    fn loss_ratio_needs_a_total() {
        let mut b = BandStats {
            freq_hz: 125,
            absorbed_by_atmosphere: 0,
            absorbed_by_materials: 0,
            absorbed_by_fittings: 0,
            lost_by_infinite_loops: 3,
            lost_by_meshing_problems: 38,
            remaining: 0,
            total: 100_000,
        };
        assert_eq!(b.lost(), 41);
        assert_eq!(b.loss_ratio(), Some(0.00041));
        b.total = 0;
        assert_eq!(b.loss_ratio(), None);
    }
}
