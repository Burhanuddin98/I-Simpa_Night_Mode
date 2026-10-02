//! Sound strength G and the A-weighted level, both over SPL (`docs/params.md`, "Sound strength G"
//! and "A-weighted level").
//!
//! - [`free_field_level_db`] is the level of the free field at [`G_DISTANCE_M`] of the source
//!   whose power times `ρc` is given, against the same reference SPL divides by. G is SPL minus
//!   it (ISO 3382-1:2009 A.2.1, Eqs. A.1-A.3): `G = 10·lg(Σ B_k / (W·ρc/(4π·100 m²)))`, so `ρc`
//!   and `p₀²` cancel. The `ρc` is SPPS's own: the `.gap`'s sources' power times `ρc`
//!   (`reportmanager.cpp:807-816`) uses the same `ρ` and `c` as the deposits of the series SPL
//!   sums (`spps/sppsInitialisation.cpp:86`). Not the standard's `G = Lp − Lw + 31 dB` (Eq. A.9),
//!   which assumes `ρc ≈ 400`: at SPPS's 413.25 the exact constant is 30.85 dB
//!   ([`iso_constant_db`]), 0.15 dB from 31.
//! - [`a_weighted`] is the energy sum over the computed octave bands of SPL plus the
//!   IEC 61672-1 A-weighting at the octave centres ([`A_WEIGHTS_DB`]), with its standard deviation
//!   propagated from the bands', which are independent (SPPS runs each band's particles on its
//!   own).

use super::{NotEvaluable, ParamError, Quantity, not_evaluable};

/// The distance G's free field is taken at, m (ISO 3382-1 A.2.1).
pub const G_DISTANCE_M: f64 = 10.0;

/// The reference sound power, W, of a sound power level `Lw` (ISO 3382-1 Eq. A.9's `Lw`).
pub const W_REF: f64 = 1e-12;

/// The IEC 61672-1 A-weighting at the octave centres from 125 Hz to 8 kHz, dB: the values the
/// plan pins (`docs/investigations/2026-10-02-m8b-metrics/PLAN.md`, "SPL (and dB(A), G on
/// top)"). A band not listed has no weight here, and a sum over it is refused, `no_a_weight`.
pub const A_WEIGHTS_DB: [(i32, f64); 7] = [
    (125, -16.1),
    (250, -8.6),
    (500, -3.2),
    (1000, 0.0),
    (2000, 1.2),
    (4000, 1.0),
    (8000, -1.1),
];

/// The A-weighting of the octave centred on `freq_hz`, dB; `None` for any other band.
pub fn a_weight_db(freq_hz: i32) -> Option<f64> {
    A_WEIGHTS_DB
        .iter()
        .find(|(f, _)| *f == freq_hz)
        .map(|(_, w)| *w)
}

/// The free field's mean-square pressure at `r_m` from an omni source whose power times `ρc` is
/// `power_rho_c` (Pa²·m²): `W·ρc/(4π·r²)`, Pa².
pub fn free_field_pa2(power_rho_c: f64, r_m: f64) -> f64 {
    power_rho_c / (4.0 * std::f64::consts::PI * r_m * r_m)
}

/// The free field's level at [`G_DISTANCE_M`] of the source whose power times `ρc` is
/// `power_rho_c`, dB, against the reference SPL divides by (`params::decay`): G is SPL minus this.
/// With several sources, `power_rho_c` is their sum, and G the level of their energies summed
/// against their free fields at 10 m summed. Refused `params_no_energy` when the power is 0 (no
/// source emits in the band), and `params_bad_energy` when it is negative or not finite.
pub fn free_field_level_db(power_rho_c: f64) -> Result<f64, ParamError> {
    if !power_rho_c.is_finite() || power_rho_c < 0.0 {
        return Err(ParamError::BadEnergy {
            index: 0,
            value: power_rho_c,
        });
    }
    if power_rho_c == 0.0 {
        return Err(ParamError::NoEnergy);
    }
    Ok(10.0
        * (free_field_pa2(power_rho_c, G_DISTANCE_M) / super::decay::level_reference_pa2()).log10())
}

/// ISO 3382-1 Eq. A.9's constant, `G = Lp − Lw + K`, exactly for `rho_c`: `K = 10·lg(4π·100 m²
/// ·p₀²/(ρc·W_ref))`, 30.85 dB at SPPS's 413.25 and 31.0 only at `ρc` ≈ 400. Not used to compute
/// G; it says what the shortcut would be off by.
pub fn iso_constant_db(rho_c: f64) -> f64 {
    10.0 * (4.0 * std::f64::consts::PI * G_DISTANCE_M * G_DISTANCE_M * super::decay::P_REF_SQUARED
        / (rho_c * W_REF))
        .log10()
}

/// The A-weighted level of `bands`, `(freq_hz, SPL dB, its standard deviation)`:
/// `L_A = 10·lg Σ 10^((L_i + A_i)/10)`, with the standard deviation propagated to first order,
/// the bands independent: `sd_A = √Σ (w_i·sd_i)²`, `w_i` band `i`'s share of the weighted energy.
/// The standard deviation is `None` when any band's is. Refused `params_no_energy` with no band,
/// and `params_not_evaluable` (`no_a_weight`) when a band is not an octave centre of
/// [`A_WEIGHTS_DB`].
pub fn a_weighted(bands: &[(i32, f64, Option<f64>)]) -> Result<(f64, Option<f64>), ParamError> {
    if bands.is_empty() {
        return Err(ParamError::NoEnergy);
    }
    let mut weighted = Vec::with_capacity(bands.len());
    for &(f, level, sd) in bands {
        let a = a_weight_db(f).ok_or_else(|| {
            not_evaluable(Quantity::AWeighted, NotEvaluable::NoAWeight { freq_hz: f })
        })?;
        weighted.push((10f64.powf((level + a) / 10.0), sd));
    }
    let total: f64 = weighted.iter().map(|(e, _)| e).sum();
    let sd = weighted
        .iter()
        .map(|(e, sd)| sd.map(|s| (e / total * s).powi(2)))
        .sum::<Option<f64>>()
        .map(f64::sqrt);
    Ok((10.0 * total.log10(), sd))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::params::decay::P_REF_SQUARED;

    /// `ρc` as SPPS computes it at 20 °C and 101 325 Pa (`docs/results.md`, "Level calibration").
    const RHO_C: f64 = 413.25;

    /// The level SPL gives a series whose bins sum to `pa2`.
    fn spl(pa2: f64) -> f64 {
        10.0 * (pa2 / P_REF_SQUARED).log10()
    }

    /// The free field at 10 m of `w` watts, as the M7 gate's direct field gives it at a point:
    /// `W·ρc/(4π·r²·p₀²)`.
    fn free_field_spl(w: f64, r: f64) -> f64 {
        spl(w * RHO_C / (4.0 * std::f64::consts::PI * r * r))
    }

    #[test]
    fn the_free_field_at_ten_metres_reads_g_zero() {
        for w in [1e-6, 1e-3, 1.0, 10.0] {
            let g = free_field_spl(w, 10.0) - free_field_level_db(w * RHO_C).unwrap();
            assert!(g.abs() < 1e-9, "W {w}: G {g}");
        }
        // And the inverse square law away from 10 m: 20 lg(10/r).
        let g5 = free_field_spl(1.0, 5.0) - free_field_level_db(RHO_C).unwrap();
        assert!((g5 - 20.0 * 2f64.log10()).abs() < 1e-9, "{g5}");
    }

    #[test]
    fn one_db_more_source_power_leaves_g_unchanged() {
        let w = 0.01;
        let w1 = w * 10f64.powf(0.1);
        // The room's level rises by 1 dB with the power, and so does the free field.
        let room = |w: f64| spl(w * RHO_C * 0.02);
        let g = room(w) - free_field_level_db(w * RHO_C).unwrap();
        let g1 = room(w1) - free_field_level_db(w1 * RHO_C).unwrap();
        assert!((room(w1) - room(w) - 1.0).abs() < 1e-9);
        assert!((g1 - g).abs() < 1e-9, "{g} then {g1}");
    }

    #[test]
    fn g_uses_the_exact_rho_c_constant_not_the_standards_31() {
        // The exact constant at SPPS's rho c, and what it is at 400.
        let k = iso_constant_db(RHO_C);
        assert!((k - 30.85).abs() < 0.005, "{k}");
        assert!((iso_constant_db(400.0) - 31.0).abs() < 0.01);
        // A free field at 10 m of Lw = 100 dB: G is 0, and Lp - Lw + 31 is 0.15 dB high.
        let lw = 100.0;
        let w = W_REF * 10f64.powf(lw / 10.0);
        let lp = free_field_spl(w, 10.0);
        let g = lp - free_field_level_db(w * RHO_C).unwrap();
        let shortcut = lp - lw + 31.0;
        assert!(g.abs() < 1e-9, "{g}");
        assert!((shortcut - g - (31.0 - k)).abs() < 1e-9);
        assert!(
            (shortcut - g).abs() > 0.1,
            "the 31 dB shortcut is in G: {shortcut} vs {g}"
        );
        // rho c cancels: the same room at another rho c reads the same G.
        let at = |rho_c: f64| spl(w * rho_c * 0.02) - free_field_level_db(w * rho_c).unwrap();
        assert!((at(RHO_C) - at(400.0)).abs() < 1e-9);
    }

    #[test]
    fn a_source_with_no_power_or_a_bad_one_refuses_g() {
        assert_eq!(free_field_level_db(0.0), Err(ParamError::NoEnergy));
        for bad in [-1.0, f64::NAN, f64::INFINITY] {
            assert_eq!(
                free_field_level_db(bad).unwrap_err().code(),
                crate::params::codes::BAD_ENERGY
            );
        }
    }

    #[test]
    fn a_single_band_reads_its_spl_plus_its_weight() {
        for (f, a) in A_WEIGHTS_DB {
            let (l, sd) = a_weighted(&[(f, 70.0, Some(0.3))]).unwrap();
            assert!((l - (70.0 + a)).abs() < 1e-9, "{f} Hz: {l}");
            assert!((sd.unwrap() - 0.3).abs() < 1e-12);
        }
        // The pinned table itself (IEC 61672-1 at the octave centres, as the plan carries it).
        assert_eq!(
            A_WEIGHTS_DB.map(|(_, a)| a),
            [-16.1, -8.6, -3.2, 0.0, 1.2, 1.0, -1.1]
        );
    }

    #[test]
    fn a_flat_spectrum_sums_its_weighted_bands() {
        let bands: Vec<(i32, f64, Option<f64>)> =
            A_WEIGHTS_DB.iter().map(|(f, _)| (*f, 60.0, None)).collect();
        let (l, sd) = a_weighted(&bands).unwrap();
        // 10 lg(10^-1.61 + 10^-0.86 + 10^-0.32 + 1 + 10^0.12 + 10^0.1 + 10^-0.11) = 6.985 dB above.
        let sum: f64 = A_WEIGHTS_DB.iter().map(|(_, a)| 10f64.powf(a / 10.0)).sum();
        assert!((l - (60.0 + 10.0 * sum.log10())).abs() < 1e-9);
        assert!((l - 66.985).abs() < 1e-3, "{l}");
        assert_eq!(sd, None, "a band without a standard deviation gives none");
        // Two equal bands at 1 kHz weight: +3.01 dB.
        let (two, _) = a_weighted(&[(1000, 60.0, None), (1000, 60.0, None)]).unwrap();
        assert!((two - 63.0103).abs() < 1e-3);
    }

    #[test]
    fn the_standard_deviation_is_propagated_by_energy_share() {
        // Two bands, the 1 kHz one carrying 9/10 of the weighted energy.
        let l2k = 60.0 + 10.0 * (1.0f64 / 9.0).log10() - 1.2;
        let (_, sd) = a_weighted(&[(1000, 60.0, Some(0.5)), (2000, l2k, Some(1.0))]).unwrap();
        let expect = ((0.9 * 0.5f64).powi(2) + (0.1 * 1.0f64).powi(2)).sqrt();
        assert!((sd.unwrap() - expect).abs() < 1e-9, "{sd:?} vs {expect}");
    }

    #[test]
    fn a_band_with_no_pinned_weight_or_no_band_refuses_the_sum() {
        let e = a_weighted(&[(1000, 60.0, None), (630, 60.0, None)]).unwrap_err();
        assert_eq!(
            e,
            not_evaluable(
                Quantity::AWeighted,
                NotEvaluable::NoAWeight { freq_hz: 630 }
            )
        );
        assert!(e.to_string().contains("630"), "{e}");
        assert_eq!(a_weighted(&[]), Err(ParamError::NoEnergy));
    }
}
