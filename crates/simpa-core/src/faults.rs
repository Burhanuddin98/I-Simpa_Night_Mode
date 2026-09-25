//! Faults a test can put into the code paths the M7 gates hold, so that each gate's say-NO
//! partner runs the fault through the code, not beside it: a number the correct run already
//! produced, moved by the test, says nothing about whether the code would catch the mistake.
//!
//! **Compiled only with the `fault-injection` feature**, which no normal build turns on: the CLI,
//! the app and `cargo build` never have it. The test builds of `simpa-core` and `simpa` enable it
//! through their dev-dependencies. Without it [`active`] is a constant `None`, and every seam
//! below reads as the correct code.
//!
//! A fault is set for the current thread only ([`with`]): `core::params` and `core::results` do
//! their work on the calling thread, so a fault reaches exactly the call it wraps and never a test
//! running beside it.
//!
//! The seams (`docs/params.md`, `docs/results.md`, "Level calibration" and gate M7(d)):
//! - [`Fault::LevelReference`]: the reference `params::decay` divides a series' energy by to give
//!   SPL, `p₀²` in the correct code. Night Mode's `.gap` reader used `10⁻¹²`, 26.02 dB high.
//! - [`Fault::DinNaturalLog`]: `params::din18041` with `ln` for `lg`.
//! - [`Fault::AirTermDropped`] and [`Fault::AirIsoExactMidband`]: the air term `results::tcr`'s
//!   analytic references add as `4·m·V`, dropped, or taken from ISO 9613-1 at the exact midband
//!   frequency instead of the solver's value at the nominal one.
//! - [`Fault::LambertUniformReflection`]: `params::lambert`'s transport reflecting uniformly over
//!   the hemisphere instead of by Lambert's cosine law, the say-NO of its mean-free-path check.
//!   The transport reads it on the calling thread and hands it to its replicas' threads.
//! - [`Fault::KuttruffFullVariance`] and [`Fault::KuttruffAirInsideMean`]: `params::room`'s
//!   Kuttruff time with the correction's `½` dropped, or with the air term folded into `ᾱ` (the
//!   form a secondary source quotes) instead of added as `4·m·V`: the say-NOs of its known-answer
//!   checks against the transport (`docs/params.md`, "Kuttruff's reference").

/// One fault, set by [`with`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Fault {
    /// SPL divides the energy by `pa2` instead of `p₀² = 4·10⁻¹⁰ Pa²`.
    LevelReference { pa2: f64 },
    /// DIN 18041's `T_soll = a·lg V + b` with the natural logarithm.
    DinNaturalLog,
    /// The analytic Sabine and Eyring times on a TCR run's inputs without the air term `4·m·V`.
    AirTermDropped,
    /// The same with `m` from ISO 9613-1's own equations at the exact midband frequency
    /// (`params::air::attenuation_db_per_m`), not the solver's value at the nominal one.
    AirIsoExactMidband,
    /// `params::lambert`'s transport draws each reflected direction uniformly over the hemisphere
    /// (`cos θ` uniform) instead of by Lambert's law (`cos θ = √u`).
    LambertUniformReflection,
    /// `params::room`'s Kuttruff factor `1 + γ²·ln(1 − ᾱ)` instead of `1 + (γ²/2)·ln(1 − ᾱ)`.
    KuttruffFullVariance,
    /// `params::room`'s Kuttruff time `K·V/A'` with `ᾱ' = ᾱ + 4·m·V/S` inside both logarithms,
    /// instead of `K·V/(4·m·V + A)`.
    KuttruffAirInsideMean,
}

#[cfg(feature = "fault-injection")]
mod imp {
    use std::cell::Cell;

    use super::Fault;

    thread_local! {
        static ACTIVE: Cell<Option<Fault>> = const { Cell::new(None) };
    }

    /// The fault set on this thread, if any.
    pub fn active() -> Option<Fault> {
        ACTIVE.with(Cell::get)
    }

    /// Restores the fault that was set before, even when `body` panics.
    struct Restore(Option<Fault>);

    impl Drop for Restore {
        fn drop(&mut self) {
            ACTIVE.with(|a| a.set(self.0));
        }
    }

    /// Runs `body` with `fault` set on this thread.
    pub fn with<R>(fault: Fault, body: impl FnOnce() -> R) -> R {
        let _restore = Restore(ACTIVE.with(|a| a.replace(Some(fault))));
        body()
    }
}

#[cfg(feature = "fault-injection")]
pub use imp::{active, with};

/// Without the `fault-injection` feature no fault can be set.
#[cfg(not(feature = "fault-injection"))]
#[inline(always)]
pub fn active() -> Option<Fault> {
    None
}

#[cfg(all(test, feature = "fault-injection"))]
mod tests {
    use super::*;

    #[test]
    fn a_fault_holds_for_its_body_only_and_is_restored_after_a_panic() {
        assert_eq!(active(), None);
        with(Fault::DinNaturalLog, || {
            assert_eq!(active(), Some(Fault::DinNaturalLog));
            with(Fault::AirTermDropped, || {
                assert_eq!(active(), Some(Fault::AirTermDropped));
            });
            assert_eq!(active(), Some(Fault::DinNaturalLog));
        });
        assert_eq!(active(), None);
        let r = std::panic::catch_unwind(|| with(Fault::AirTermDropped, || panic!("inside")));
        assert!(r.is_err());
        assert_eq!(active(), None);
        // Another thread never sees it.
        with(Fault::DinNaturalLog, || {
            assert_eq!(std::thread::spawn(active).join().unwrap(), None);
        });
    }
}
