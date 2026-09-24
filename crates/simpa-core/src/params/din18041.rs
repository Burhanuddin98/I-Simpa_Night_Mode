//! DIN 18041:2016-03 target reverberation times for group-A rooms (`docs/params.md`, "DIN 18041
//! targets").
//!
//! **The standard was not read.** The formulas are those BNB V 2020, criterion 3.1.4 (BMI),
//! quotes "nach DIN 18041:2016-03": A1 on its page 6, A2 and A3 on page 7, A4 on page 4, A5 with
//! its volume range on page 8. The 5000 m³ upper limit of A1–A4 is the DEGA Akustik Journal 03/19,
//! page 17. The lower limits of A1–A4 are not sourced.

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
}

fn out_of_range(group: Group, volume_m3: f64, detail: &str) -> ParamError {
    ParamError::DinOutOfRange {
        group: format!("{group:?}"),
        volume_m3,
        detail: detail.into(),
    }
}

/// `T_soll`, s, for `group` at `volume_m3`. Refused (`params_din_out_of_range`): a volume that is
/// not a positive number; above 5000 m³ for A1–A4; below 200 m³ for A5; or a formula value of 0 s
/// or less. A5 above 10 000 m³ is 2.0 s.
pub fn target_s(group: Group, volume_m3: f64) -> Result<f64, ParamError> {
    if !volume_m3.is_finite() || volume_m3 <= 0.0 {
        return Err(out_of_range(group, volume_m3, "not a positive volume"));
    }
    match group {
        Group::A5 if volume_m3 < 200.0 => {
            return Err(out_of_range(group, volume_m3, "A5 starts at 200 m³"));
        }
        Group::A5 if volume_m3 > 10_000.0 => return Ok(2.0),
        Group::A5 => {}
        _ if volume_m3 > 5000.0 => {
            return Err(out_of_range(
                group,
                volume_m3,
                "above 5000 m³ DIN 18041 sets targets for sport only",
            ));
        }
        _ => {}
    }
    let (a, b) = group.coefficients();
    let t = a * volume_m3.log10() + b;
    if t <= 0.0 {
        return Err(out_of_range(
            group,
            volume_m3,
            "the formula gives no positive target at this volume",
        ));
    }
    Ok(t)
}
