//! DIN 18041:2016-03 target reverberation times for group-A rooms (`docs/params.md`, "DIN 18041
//! targets").
//!
//! **The standard was not read.** What is used, and where it comes from:
//! - The formulas: BNB V 2020, criterion 3.1.4 (BMI), which quotes them "nach DIN 18041:2016-03":
//!   A1 on its page 6, A2 and A3 on page 7, A4 on page 4, A5 with its volume range on page 8.
//! - The volume ranges: the volumes each group's line is drawn solid over in the standard's
//!   figure of `T_soll` against `V`, as reproduced in C. Nocke, "Die neue DIN 18041 – Hörsamkeit
//!   in Räumen", Lärmbekämpfung 11 (2016) Nr. 2, Bild 2, page 51. The article calls the solid
//!   stretches the typical volumes. Measured from the figure: A1 30–1000 m³, A2 50–5000 m³, A3
//!   30–5000 m³, A4 30–500 m³, A5 200–10 000 m³ and then flat at 2.0 s to 30 000 m³. The text
//!   sources agree where they say anything: A4 is not for rooms above 500 m³ (the same article,
//!   Table 1, page 52); the equations apply between 30 and 5000 m³ (C. Ruhe, "Akustik in
//!   Bildungsbauten", 5.2); A5 is the formula from 200 to 10 000 m³ and 2.0 s above (BNB, page 8);
//!   sports halls are covered up to 30 000 m³ (fennext.eu, "DIN 18041 Hörsamkeit in Räumen").
//!   Whether the standard's own text gives exactly these limits, and whether they are inclusive,
//!   is not checked.
//!
//! **What a target applies to.** `T_soll` is for the room 80 % occupied, at mid frequencies
//! (Nocke, page 51; Ruhe, 5.2), and `V` is the effective room volume (Ruhe, 5.2). An empty room's
//! model does not meet it by matching it: the audience's absorption has to be in the model.

use schemars::JsonSchema;
use serde::Serialize;

use super::ParamError;

/// A group-A use.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, JsonSchema)]
pub enum Group {
    /// Music.
    A1,
    /// Speech, lecture.
    A2,
    /// Teaching, communication.
    A3,
    /// Teaching, communication, inclusive.
    A4,
    /// Sport.
    A5,
}

impl Group {
    /// `(a, b)` of `T_soll = a·lg(V/m³) + b`.
    pub fn coefficients(self) -> (f64, f64) {
        match self {
            Group::A1 => (0.45, 0.07),
            Group::A2 => (0.37, -0.14),
            Group::A3 => (0.32, -0.17),
            Group::A4 => (0.26, -0.14),
            Group::A5 => (0.75, -1.00),
        }
    }

    /// The volumes, m³, a target is given for (both ends accepted). For A5 the formula runs to
    /// 10 000 m³ and the target is 2.0 s above.
    pub fn volume_range_m3(self) -> (f64, f64) {
        match self {
            Group::A1 => (30.0, 1000.0),
            Group::A2 => (50.0, 5000.0),
            Group::A3 => (30.0, 5000.0),
            Group::A4 => (30.0, 500.0),
            Group::A5 => (200.0, 30_000.0),
        }
    }
}

/// Where A5's formula gives way to a flat 2.0 s, m³.
pub const A5_FLAT_FROM_M3: f64 = 10_000.0;

/// `T_soll`, s, for `group` at `volume_m3`: for the room 80 % occupied, at mid frequencies.
/// Refused (`params_din_out_of_range`) for a volume outside [`Group::volume_range_m3`].
pub fn target_s(group: Group, volume_m3: f64) -> Result<f64, ParamError> {
    let (lo, hi) = group.volume_range_m3();
    if !(lo..=hi).contains(&volume_m3) {
        return Err(ParamError::DinOutOfRange {
            group: format!("{group:?}"),
            volume_m3,
            detail: format!("{group:?} gives targets from {lo} to {hi} m³"),
        });
    }
    if group == Group::A5 && volume_m3 > A5_FLAT_FROM_M3 {
        return Ok(2.0);
    }
    let (a, b) = group.coefficients();
    Ok(a * volume_m3.log10() + b)
}
