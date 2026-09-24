//! Sabine and Eyring reverberation times as TCR computes them (`src/ctr/TC_CalculationCore.cpp`;
//! `docs/params.md`, "Sabine and Eyring").
//!
//! - Sabine: `A = Σ Sᵢ·αᵢ` (lines 87-102).
//! - Eyring: `A = −S·ln(1 − ᾱ)`, `S = Σ Sᵢ`, `ᾱ = Σ Sᵢ·αᵢ / S` (lines 104-133).
//! - `T = K·V / (4·m·V + A)` with air absorption on, `K·V / A` with it off (lines 135-141).
//!
//! The caller chooses the surfaces: TCR leaves out fitting faces whose material's first band has
//! `τ ≠ 1` (`isTransparent`, lines 11-17).

use schemars::JsonSchema;
use serde::Serialize;

use super::ParamError;

/// TCR's constant, `0.163` s/m (`TC_CalculationCore.cpp:138, 140`).
pub const TCR_CONSTANT: f64 = 0.163;

/// One surface: its area and its absorption coefficient in one band.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, JsonSchema)]
pub struct Surface {
    pub area_m2: f64,
    /// α, in [0, 1].
    pub absorption: f64,
}

/// The constant `K` of `T = K·V / A`.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case", tag = "constant")]
pub enum RtConstant {
    /// TCR's `0.163`. The gates that compare with TCR use this.
    Tcr,
    /// `24·ln(10)/c`: 0.16102 at TCR's `c = 343.2 m/s`; TCR's is 1.23 % above it.
    Physical { speed_of_sound: f64 },
}

impl RtConstant {
    /// `K`, s/m.
    pub fn value(self) -> f64 {
        match self {
            RtConstant::Tcr => TCR_CONSTANT,
            RtConstant::Physical { speed_of_sound } => {
                24.0 * std::f64::consts::LN_10 / speed_of_sound
            }
        }
    }
}

fn bad(field: &str, value: f64) -> ParamError {
    ParamError::BadRoom {
        field: field.into(),
        value,
    }
}

/// `(Σ Sᵢ, Σ Sᵢ·αᵢ)`, refused (`params_bad_room`) for an area that is negative or not finite, an
/// α outside [0, 1], or no area at all.
fn sums(surfaces: &[Surface]) -> Result<(f64, f64), ParamError> {
    let (mut s, mut sa) = (0.0, 0.0);
    for x in surfaces {
        if !x.area_m2.is_finite() || x.area_m2 < 0.0 {
            return Err(bad("area_m2", x.area_m2));
        }
        if !x.absorption.is_finite() || !(0.0..=1.0).contains(&x.absorption) {
            return Err(bad("absorption", x.absorption));
        }
        s += x.area_m2;
        sa += x.area_m2 * x.absorption;
    }
    if s <= 0.0 {
        return Err(bad("total_area_m2", s));
    }
    Ok((s, sa))
}

/// Sabine's equivalent absorption area `Σ Sᵢ·αᵢ`, m².
pub fn sabine_absorption_area(surfaces: &[Surface]) -> Result<f64, ParamError> {
    Ok(sums(surfaces)?.1)
}

/// Eyring's equivalent absorption area `−S·ln(1 − ᾱ)`, m². Infinite when ᾱ = 1.
pub fn eyring_absorption_area(surfaces: &[Surface]) -> Result<f64, ParamError> {
    let (s, sa) = sums(surfaces)?;
    // ln(1 − ᾱ) through ln_1p, exact for small ᾱ where 1 − ᾱ would round.
    Ok(-s * (-sa / s).ln_1p())
}

/// `T = K·V / (4·m·V + A)`, s. `air_m_per_metre` is the energy attenuation `m`
/// ([`super::air::energy_attenuation_per_m`]); `None` is `abs_atmo_calc = 0`. Refused
/// (`params_bad_room`) for a volume that is not a positive number, an `A` that is negative or
/// NaN, or an `m` that is negative or not finite; `params_no_absorption` when `4·m·V + A = 0`.
pub fn reverberation_time(
    volume_m3: f64,
    absorption_area_m2: f64,
    air_m_per_metre: Option<f64>,
    constant: RtConstant,
) -> Result<f64, ParamError> {
    if !volume_m3.is_finite() || volume_m3 <= 0.0 {
        return Err(bad("volume_m3", volume_m3));
    }
    if absorption_area_m2.is_nan() || absorption_area_m2 < 0.0 {
        return Err(bad("absorption_area_m2", absorption_area_m2));
    }
    let air = match air_m_per_metre {
        None => 0.0,
        Some(m) if m.is_finite() && m >= 0.0 => 4.0 * m * volume_m3,
        Some(m) => return Err(bad("air_m_per_metre", m)),
    };
    let k = constant.value();
    if !k.is_finite() || k <= 0.0 {
        return Err(bad("constant", k));
    }
    let a = air + absorption_area_m2;
    if a == 0.0 {
        return Err(ParamError::NoAbsorption);
    }
    Ok(k * volume_m3 / a)
}

/// Sabine's reverberation time, s.
pub fn sabine_rt(
    volume_m3: f64,
    surfaces: &[Surface],
    air_m_per_metre: Option<f64>,
    constant: RtConstant,
) -> Result<f64, ParamError> {
    reverberation_time(
        volume_m3,
        sabine_absorption_area(surfaces)?,
        air_m_per_metre,
        constant,
    )
}

/// Eyring's reverberation time, s. 0 when ᾱ = 1.
pub fn eyring_rt(
    volume_m3: f64,
    surfaces: &[Surface],
    air_m_per_metre: Option<f64>,
    constant: RtConstant,
) -> Result<f64, ParamError> {
    reverberation_time(
        volume_m3,
        eyring_absorption_area(surfaces)?,
        air_m_per_metre,
        constant,
    )
}
