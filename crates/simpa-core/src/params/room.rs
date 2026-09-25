//! Sabine and Eyring reverberation times as TCR computes them (`src/ctr/TC_CalculationCore.cpp`;
//! `docs/params.md`, "Sabine and Eyring").
//!
//! - Sabine: `A = Σ Sᵢ·αᵢ` (lines 87-102).
//! - Eyring: `A = −S·ln(1 − ᾱ)`, `S = Σ Sᵢ`, `ᾱ = Σ Sᵢ·αᵢ / S` (lines 104-133).
//! - `T = K·V / (4·m·V + A)` with air absorption on, `K·V / A` with it off (lines 135-141).
//!
//! The caller chooses the surfaces: TCR leaves out fitting faces whose material's first band has
//! `τ ≠ 1` (`isTransparent`, lines 11-17).
//!
//! **Kuttruff's correction** (M8's reference, Burhan, 2026-09-24 23:14; `docs/params.md`,
//! "Kuttruff's reference"): Eyring's formula takes every free path to be the mean, `4V/S`. Free
//! paths that spread about it, with relative variance `γ² = (⟨ℓ²⟩ − ⟨ℓ⟩²)/⟨ℓ⟩²`, make the decay
//! slower. **The form used here**, with `ᾱ = Σ Sᵢ·αᵢ / S` as Eyring's:
//!
//! `A_K = −S·ln(1 − ᾱ)·[1 + (γ²/2)·ln(1 − ᾱ)]`, `T = K·V / (4·m·V + A_K)`, `K = 24·ln 10 / c`.
//!
//! - **The wall term** is Kuttruff's as U. M. Stephenson gives it, citing H. Kuttruff, *Room
//!   Acoustics* (Elsevier, Barking; no edition named): `α'' = α'·(1 − γ²·α'/2)` with
//!   `α' = −ln(1 − α)`, "A rigorous definition of the term 'diffuse sound field' and a discussion
//!   of different reverberation formulae", ICA 2016, paper ICA2016-556, eq. (21) (read). With
//!   `α'` substituted it is the factor above. Kuttruff's book itself was not opened here.
//! - **The air term** is not in Stephenson's eq. (21). It is added outside the wall term, as TCR
//!   adds it to Eyring's (`TC_CalculationCore.cpp:138`): air multiplies every path's energy by
//!   `e^(−m·c·t)` whatever its reflections, so its decay rate adds to the walls' exactly. The form
//!   Arup's Strutt help quotes from Bies and Hansen, *Engineering Noise Control*, 4th ed.,
//!   eq. 7.64 (the help page read, the book not), folds `4·m·V/S` into `ᾱ` inside both
//!   logarithms instead; the transport tells the two apart
//!   ([`crate::faults::Fault::KuttruffAirInsideMean`], `tests/params_kuttruff.rs`).
//! - **Where it comes from**: the energy after time `t` is `⟨(1 − ᾱ)^n⟩` over the number `n` of
//!   reflections, whose mean is `c·t·S/(4V)` and, for independent free paths, whose variance is
//!   `γ²` times it; the formula keeps the first two cumulants of `n`. Two approximations, measured
//!   against the transport in M8's rooms (`docs/params.md`, "Kuttruff's reference"): successive
//!   free paths in a room with flat walls are positively correlated (lag 1 0.067 and 0.075 in M8's
//!   boxes, 0 in a sphere), so `n`'s variance is about a quarter larger than `γ²·n` and the
//!   formula reads low where `ᾱ` is small (−0.2 to −0.4 %); and the second-order truncation reads
//!   high as `ᾱ` grows (+0.3 and +0.6 % at 0.4). In M8's cells the two partly cancel, within
//!   0.6 %; in other room shapes neither is bounded by that.
//! - **`γ²` comes only from [`super::lambert::FreePaths`]**, which only the transport makes, from
//!   the room alone at fixed settings: no function here takes `γ²` as a number. `V` comes from the
//!   same `FreePaths`; the surfaces give only `ᾱ`, and must add up to the traced room's area.

use schemars::JsonSchema;
use serde::Serialize;

use super::ParamError;
use super::lambert::FreePaths;

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

/// `(S, ln(1 − ᾱ))` of `surfaces`, refused as [`kuttruff_absorption_area`] refuses them.
fn kuttruff_inputs(free_paths: &FreePaths, surfaces: &[Surface]) -> Result<(f64, f64), ParamError> {
    let (s, sa) = sums(surfaces)?;
    let enclosure = free_paths.area_m2();
    if (s - enclosure).abs() > 1e-6 * enclosure {
        return Err(bad(
            &format!(
                "total_area_m2 (the enclosure the free paths were traced in has {enclosure} m²)"
            ),
            s,
        ));
    }
    let ln = (-sa / s).ln_1p();
    if !ln.is_finite() {
        return Err(bad(
            "mean_absorption (Kuttruff's correction has no value at 1)",
            sa / s,
        ));
    }
    Ok((s, ln))
}

/// `−S·ln(1 − ᾱ)·[1 + (γ²/2)·ln(1 − ᾱ)]` from `S` and `ln(1 − ᾱ)`, refused where it no longer
/// grows with `ᾱ`: its derivative in `ln(1 − ᾱ)` is `−S·(1 + γ²·ln(1 − ᾱ))`, so beyond
/// `ln(1 − ᾱ) = −1/γ²` (`ᾱ` ≈ 0.92 at `γ²` = 0.4) more absorption would give a longer time, and
/// the second-order correction describes nothing. [`crate::faults::Fault::KuttruffFullVariance`]
/// drops the `½`, in a test build only.
fn kuttruff_area(s: f64, ln: f64, gamma2: f64) -> Result<f64, ParamError> {
    let slope = 1.0 + gamma2 * ln;
    if slope.is_nan() || slope <= 0.0 {
        return Err(bad(
            "kuttruff_monotone (1 + γ²·ln(1 − ᾱ); the correction is not valid where it is not \
             positive)",
            slope,
        ));
    }
    let half = match crate::faults::active() {
        Some(crate::faults::Fault::KuttruffFullVariance) => 1.0,
        _ => 0.5,
    };
    Ok(-s * ln * (1.0 + half * gamma2 * ln))
}

/// Kuttruff's equivalent absorption area `−S·ln(1 − ᾱ)·[1 + (γ²/2)·ln(1 − ᾱ)]`, m² ([module
/// docs](self)), with `γ²` from `free_paths`. Refused (`params_bad_room`) as Eyring's is, and for
/// surfaces whose total area is not the enclosure's the free paths were traced in (within 10⁻⁶),
/// an `ᾱ` of 1, or an `ᾱ` so large that `1 + γ²·ln(1 − ᾱ)` is not positive, where the area would
/// fall as the absorption grows.
pub fn kuttruff_absorption_area(
    free_paths: &FreePaths,
    surfaces: &[Surface],
) -> Result<f64, ParamError> {
    let (s, ln) = kuttruff_inputs(free_paths, surfaces)?;
    kuttruff_area(s, ln, free_paths.gamma2())
}

/// Kuttruff's reverberation time `K·V / (4·m·V + A)`, s, with `A` from
/// [`kuttruff_absorption_area`] and `V` the volume of the enclosure the free paths were traced
/// in. Refused as [`reverberation_time`] and [`kuttruff_absorption_area`] refuse.
/// [`crate::faults::Fault::KuttruffAirInsideMean`] folds the air into `ᾱ` instead, in a test build
/// only ([module docs](self)).
pub fn kuttruff_rt(
    free_paths: &FreePaths,
    surfaces: &[Surface],
    air_m_per_metre: Option<f64>,
    constant: RtConstant,
) -> Result<f64, ParamError> {
    if let (Some(crate::faults::Fault::KuttruffAirInsideMean), Some(m)) =
        (crate::faults::active(), air_m_per_metre)
    {
        let (s, sa) = sums(surfaces)?;
        let v = free_paths.volume_m3();
        let ln = (-(sa + 4.0 * m * v) / s).ln_1p();
        return reverberation_time(
            v,
            kuttruff_area(s, ln, free_paths.gamma2())?,
            None,
            constant,
        );
    }
    reverberation_time(
        free_paths.volume_m3(),
        kuttruff_absorption_area(free_paths, surfaces)?,
        air_m_per_metre,
        constant,
    )
}

/// The standard deviation [`kuttruff_rt`] inherits from `γ²`'s standard error, s:
/// `|∂T/∂γ²|·σ(γ²)`, with `∂T/∂γ² = T·S·ln²(1 − ᾱ)/(2·(4·m·V + A))`. Refused as [`kuttruff_rt`].
///
/// **Only the statistical part.** It leaves out the formula's own error against a diffuse room
/// ([module docs](self)), which no number of rays reduces: measured −0.41 % to +0.59 % in M8's
/// cells, where this standard deviation is at most 0.02 %, and not measured in other rooms. It is
/// not the reference's total uncertainty.
pub fn kuttruff_rt_sd(
    free_paths: &FreePaths,
    surfaces: &[Surface],
    air_m_per_metre: Option<f64>,
    constant: RtConstant,
) -> Result<f64, ParamError> {
    let t = kuttruff_rt(free_paths, surfaces, air_m_per_metre, constant)?;
    let (s, ln) = kuttruff_inputs(free_paths, surfaces)?;
    let a = kuttruff_absorption_area(free_paths, surfaces)?;
    let air = 4.0 * air_m_per_metre.unwrap_or(0.0) * free_paths.volume_m3();
    Ok(t * s * ln * ln / (2.0 * (air + a)) * free_paths.gamma2_se())
}
