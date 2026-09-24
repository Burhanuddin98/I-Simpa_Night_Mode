//! ISO 9613-1:1993 atmospheric attenuation, and the value SPPS and TCR actually use
//! (`docs/params.md`, "ISO 9613-1").
//!
//! - [`attenuation_db_per_m`] is the standard: equations (3) to (5), read on its page 3, with the
//!   humidity conversion of Annex B as upstream writes it
//!   (`lib_interface/data_manager/data_calculation/Coef_Att_Atmos.cpp:54-56`). Table 1 is computed
//!   at the exact midband frequencies ([`exact_midband_hz`], equation (6)).
//! - [`upstream_attenuation_db_per_m`] is upstream's transcription, `Coef_Att_Atmos.cpp:48-75`,
//!   term for term: the same equations written another way, with `h` missing the pressure factor.
//! - [`solver_air_absorption_per_m`] is what the solvers use: upstream's value at the nominal band
//!   frequency, converted to the per-metre energy attenuation
//!   (`lib_interface/data_manager/base_core_configuration.cpp:114`).

use schemars::JsonSchema;
use serde::Serialize;

use super::ParamError;

/// `p_r`, the reference ambient pressure, Pa (ISO 9613-1, 4.2).
pub const REFERENCE_PRESSURE_PA: f64 = 101_325.0;
/// `T₀`, the reference air temperature, K (ISO 9613-1, 4.2).
pub const REFERENCE_TEMPERATURE_K: f64 = 293.15;
/// `T₀₁`, the triple-point isotherm temperature, K (Annex B, as upstream writes it:
/// `calculsPropagation.h:54`).
pub const TRIPLE_POINT_K: f64 = 273.16;
/// Celsius to kelvin.
pub const KELVIN_OFFSET_K: f64 = 273.15;

/// The air: temperature, relative humidity and ambient pressure.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, JsonSchema)]
pub struct Atmosphere {
    pub temperature_c: f64,
    /// Relative to saturation over liquid water, %.
    pub relative_humidity_percent: f64,
    pub pressure_pa: f64,
}

impl Atmosphere {
    /// The standard's reference air at `temperature_c` and `relative_humidity_percent`, at
    /// 101.325 kPa.
    pub fn at_reference_pressure(temperature_c: f64, relative_humidity_percent: f64) -> Self {
        Atmosphere {
            temperature_c,
            relative_humidity_percent,
            pressure_pa: REFERENCE_PRESSURE_PA,
        }
    }

    fn kelvin(&self) -> f64 {
        self.temperature_c + KELVIN_OFFSET_K
    }

    /// Refused (`params_bad_air`): a temperature at or below absolute zero, a humidity outside
    /// 0–100 %, a pressure that is not positive, or any of them not finite.
    fn check(&self) -> Result<(), ParamError> {
        let bad = |field: &str, value: f64| {
            Err(ParamError::BadAir {
                field: field.into(),
                value,
            })
        };
        let (t, rh, p) = (
            self.temperature_c,
            self.relative_humidity_percent,
            self.pressure_pa,
        );
        if !t.is_finite() || self.kelvin() <= 0.0 {
            return bad("temperature_c", t);
        }
        if !rh.is_finite() || !(0.0..=100.0).contains(&rh) {
            return bad("relative_humidity_percent", rh);
        }
        if !p.is_finite() || p <= 0.0 {
            return bad("pressure_pa", p);
        }
        Ok(())
    }
}

fn check_frequency(f_hz: f64) -> Result<(), ParamError> {
    if f_hz.is_finite() && f_hz > 0.0 {
        Ok(())
    } else {
        Err(ParamError::BadAir {
            field: "frequency_hz".into(),
            value: f_hz,
        })
    }
}

/// `p_sat / p_r = 10^C`, `C = −6.8346·(T₀₁/T)^1.261 + 4.6151` (Annex B, as upstream writes it:
/// `Coef_Att_Atmos.cpp:54-55`).
fn saturation_ratio(kelvin: f64) -> f64 {
    let c = -6.8346 * (TRIPLE_POINT_K / kelvin).powf(1.261) + 4.6151;
    10f64.powf(c)
}

/// `h`, the molar concentration of water vapour, %: `h_r·(p_sat/p_r)/(p_a/p_r)` (Annex B).
pub fn molar_concentration_percent(air: &Atmosphere) -> Result<f64, ParamError> {
    air.check()?;
    Ok(
        air.relative_humidity_percent * saturation_ratio(air.kelvin())
            / (air.pressure_pa / REFERENCE_PRESSURE_PA),
    )
}

/// α in dB/m for a pure tone of `f_hz` by ISO 9613-1 equations (3) to (5). The value alone:
/// [`attenuation`] carries the accuracy the standard states for it.
pub fn attenuation_db_per_m(f_hz: f64, air: &Atmosphere) -> Result<f64, ParamError> {
    check_frequency(f_hz)?;
    let h = molar_concentration_percent(air)?;
    let pa_pr = air.pressure_pa / REFERENCE_PRESSURE_PA;
    let tt0 = air.kelvin() / REFERENCE_TEMPERATURE_K;
    // (3), (4)
    let fr_o = pa_pr * (24.0 + 4.04e4 * h * (0.02 + h) / (0.391 + h));
    let fr_n =
        pa_pr * tt0.powf(-0.5) * (9.0 + 280.0 * h * (-4.170 * (tt0.powf(-1.0 / 3.0) - 1.0)).exp());
    // (5)
    let t = air.kelvin();
    let f2 = f_hz * f_hz;
    let classical = 1.84e-11 / pa_pr * tt0.sqrt();
    let oxygen = 0.01275 * (-2239.1 / t).exp() / (fr_o + f2 / fr_o);
    let nitrogen = 0.1068 * (-3352.0 / t).exp() / (fr_n + f2 / fr_n);
    Ok(8.686 * f2 * (classical + tt0.powf(-2.5) * (oxygen + nitrogen)))
}

/// The accuracy ISO 9613-1 states for equations (3) to (5) (clause 7, printed page 4; PDF page 8
/// of the iTeh preview).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum StatedAccuracy {
    /// ±10 % (7.1): `h` from 0.05 % to 5 %, −20 °C to +50 °C.
    TenPercent,
    /// ±20 % (7.2): `h` from 0.005 % to 0.05 %, or above 5 %, −20 °C to +50 °C.
    TwentyPercent,
    /// ±50 % (7.3): `h` below 0.005 %, above 200 K.
    FiftyPercent,
}

/// α with the accuracy the standard states for it.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, JsonSchema)]
pub struct Attenuation {
    pub db_per_m: f64,
    /// `None` outside every range of clause 7: the standard states no accuracy there.
    pub stated_accuracy: Option<StatedAccuracy>,
}

/// The class of clause 7 that `f_hz` and `air` fall in, or `None`. Every class also needs a
/// pressure below 200 kPa and a frequency-to-pressure ratio from 4·10⁻⁴ to 10 Hz/Pa, which at
/// 101.325 kPa excludes the bands below 40.5 Hz.
pub fn stated_accuracy(f_hz: f64, air: &Atmosphere) -> Result<Option<StatedAccuracy>, ParamError> {
    check_frequency(f_hz)?;
    let h = molar_concentration_percent(air)?;
    let (k, p) = (air.kelvin(), air.pressure_pa);
    if p >= 200_000.0 || !(4e-4..=10.0).contains(&(f_hz / p)) {
        return Ok(None);
    }
    let temperate = (253.15..=323.15).contains(&k);
    Ok(if temperate && (0.05..=5.0).contains(&h) {
        Some(StatedAccuracy::TenPercent)
    } else if temperate && ((0.005..0.05).contains(&h) || h > 5.0) {
        Some(StatedAccuracy::TwentyPercent)
    } else if h < 0.005 && k > 200.0 {
        Some(StatedAccuracy::FiftyPercent)
    } else {
        None
    })
}

/// α in dB/m by ISO 9613-1, with the accuracy clause 7 states for it.
pub fn attenuation(f_hz: f64, air: &Atmosphere) -> Result<Attenuation, ParamError> {
    Ok(Attenuation {
        db_per_m: attenuation_db_per_m(f_hz, air)?,
        stated_accuracy: stated_accuracy(f_hz, air)?,
    })
}

/// The speed of sound upstream derives from the temperature, `343.2·√(T/293.15)` m/s
/// (`Celerite_du_son.cpp:46`).
pub fn speed_of_sound(temperature_c: f64) -> f64 {
    343.2 * ((temperature_c + KELVIN_OFFSET_K) / REFERENCE_TEMPERATURE_K).sqrt()
}

/// α in dB/m as upstream computes it, `Coef_Att_Atmos.cpp:48-75`, term for term. At 101.325 kPa
/// it agrees with [`attenuation_db_per_m`] to 0.035 %; elsewhere its `h` lacks the pressure
/// factor (line 56).
pub fn upstream_attenuation_db_per_m(f_hz: f64, air: &Atmosphere) -> Result<f64, ParamError> {
    check_frequency(f_hz)?;
    air.check()?;
    let (f, k, p) = (f_hz, air.kelvin(), air.pressure_pa);
    let (pref, kref) = (REFERENCE_PRESSURE_PA, REFERENCE_TEMPERATURE_K);
    let cson = 343.2 * (k / kref).sqrt();
    // Lines 54-56: h = H·Ps/Pref, no division by P/Pref.
    let hmol = air.relative_humidity_percent * saturation_ratio(k);
    // Line 59.
    let acr = (pref / p) * 1.60e-10 * (k / kref).sqrt() * f * f;
    // Lines 62-64.
    let fr = (p / pref) * (24.0 + 4.04e4 * hmol * (0.02 + hmol) / (0.391 + hmol));
    let am = 1.559 * 0.209 * (-2239.1 / k).exp() * (2239.1 / k).powi(2);
    let avib_o = am * (f / cson) * 2.0 * (f / fr) / (1.0 + (f / fr).powi(2));
    // Lines 67-69.
    let fr = (p / pref)
        * (kref / k).sqrt()
        * (9.0 + 280.0 * hmol * (-4.170 * ((k / kref).powf(-1.0 / 3.0) - 1.0)).exp());
    let am = 1.559 * 0.781 * (-3352.0 / k).exp() * (3352.0 / k).powi(2);
    let avib_n = am * (f / cson) * 2.0 * (f / fr) / (1.0 + (f / fr).powi(2));
    Ok(acr + avib_o + avib_n)
}

/// The per-metre energy attenuation `m` of `I(x) = I₀·e^(−m·x)` from α in dB/m:
/// `m = α·ln(10)/10`, the conversion upstream applies (`base_core_configuration.cpp:114`). TCR adds
/// `4·m·V` to the absorption area (`TC_CalculationCore.cpp:138`). SPPS stores `e^(−m·c·dt)` per
/// step (`base_core_configuration.cpp:115`): in energetic mode it multiplies a particle's energy
/// by it (`spps/CalculationCore.cpp:57`), in random mode it keeps the particle with that
/// probability (`spps/CalculationCore.cpp:64`).
pub fn energy_attenuation_per_m(alpha_db_per_m: f64) -> f64 {
    alpha_db_per_m * std::f64::consts::LN_10 / 10.0
}

/// The `m` SPPS and TCR use for a band whose `config.xml` frequency is `nominal_hz`, when
/// `disable_absatmo_computation` is not 1: upstream's α at the nominal frequency, converted
/// (`base_core_configuration.cpp:114`). Upstream stores it as `f32`; this does not round.
pub fn solver_air_absorption_per_m(nominal_hz: f64, air: &Atmosphere) -> Result<f64, ParamError> {
    Ok(energy_attenuation_per_m(upstream_attenuation_db_per_m(
        nominal_hz, air,
    )?))
}

/// The exact midband frequency `1000·10^(k/10)` Hz of the one-third-octave band whose nominal
/// frequency is `nominal_hz` (ISO 9613-1 equation (6), `b = 1/3`), or `None` when `nominal_hz`
/// is not within 2 % of one.
pub fn exact_midband_hz(nominal_hz: f64) -> Option<f64> {
    if !nominal_hz.is_finite() || nominal_hz <= 0.0 {
        return None;
    }
    let k = (10.0 * (nominal_hz / 1000.0).log10()).round();
    let fm = 1000.0 * 10f64.powf(k / 10.0);
    ((nominal_hz / fm - 1.0).abs() <= 0.02).then_some(fm)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::params::codes;

    #[test]
    fn exact_midbands_follow_equation_6() {
        assert_eq!(exact_midband_hz(1000.0), Some(1000.0));
        let near = |n: f64, want: f64| {
            let got = exact_midband_hz(n).unwrap();
            assert!((got - want).abs() < 1e-3, "{n}: {got} vs {want}");
        };
        near(50.0, 50.1187);
        near(125.0, 125.8925);
        near(160.0, 158.4893);
        near(1600.0, 1584.8932);
        near(8000.0, 7943.2823);
        near(16000.0, 15848.9319);
        near(20000.0, 19952.6231);
        // Not a one-third-octave nominal frequency.
        assert_eq!(exact_midband_hz(1100.0), None);
        assert_eq!(exact_midband_hz(0.0), None);
        assert_eq!(exact_midband_hz(f64::NAN), None);
    }

    #[test]
    fn bad_air_is_refused() {
        let ok = Atmosphere::at_reference_pressure(20.0, 50.0);
        assert!(attenuation_db_per_m(1000.0, &ok).is_ok());
        let cases = [
            Atmosphere {
                temperature_c: -273.15,
                ..ok
            },
            Atmosphere {
                temperature_c: f64::NAN,
                ..ok
            },
            Atmosphere {
                relative_humidity_percent: -1.0,
                ..ok
            },
            Atmosphere {
                relative_humidity_percent: 100.5,
                ..ok
            },
            Atmosphere {
                pressure_pa: 0.0,
                ..ok
            },
            Atmosphere {
                pressure_pa: f64::INFINITY,
                ..ok
            },
        ];
        for air in cases {
            assert_eq!(
                attenuation_db_per_m(1000.0, &air).unwrap_err().code(),
                codes::BAD_AIR,
                "{air:?}"
            );
            assert_eq!(
                upstream_attenuation_db_per_m(1000.0, &air)
                    .unwrap_err()
                    .code(),
                codes::BAD_AIR
            );
        }
        for f in [0.0, -1.0, f64::NAN] {
            assert_eq!(
                attenuation_db_per_m(f, &ok).unwrap_err().code(),
                codes::BAD_AIR
            );
        }
        // Dry air and saturated air are both inside the domain.
        assert!(
            attenuation_db_per_m(1000.0, &Atmosphere::at_reference_pressure(20.0, 0.0)).is_ok()
        );
        assert!(
            attenuation_db_per_m(1000.0, &Atmosphere::at_reference_pressure(20.0, 100.0)).is_ok()
        );
    }

    #[test]
    fn the_speed_of_sound_is_upstreams() {
        assert!((speed_of_sound(20.0) - 343.2).abs() < 1e-12);
        assert!(speed_of_sound(0.0) < 343.2);
    }
}
