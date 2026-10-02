//! EDT v2.1, ported from `docs/investigations/2026-09-27-edt-heldout/frozen2/method.py`.
//! STUB: the tests are written first and must fail against this.

use schemars::JsonSchema;
use serde::Serialize;

/// Every reason the method refuses with.
pub const REFUSAL_REASONS: [&str; 9] = [
    "no_energy",
    "no_energy_after_arrival",
    "run_too_short",
    "direct_only",
    "step_too_coarse",
    "not_decaying",
    "too_few_particles",
    "not_decaying_at_run_end",
    "receiver_too_large",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Ok,
    Wide,
    Refused,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Outcome {
    pub edt: Option<f64>,
    pub edt_lo: Option<f64>,
    pub edt_hi: Option<f64>,
    pub status: Status,
    pub reason: String,
}

pub fn analyse(_bins: &[f64], _dt: f64, _t_arrival: Option<f64>, _half_width: Option<f64>) -> Outcome {
    Outcome {
        edt: None,
        edt_lo: None,
        edt_hi: None,
        status: Status::Refused,
        reason: "not_implemented".into(),
    }
}
