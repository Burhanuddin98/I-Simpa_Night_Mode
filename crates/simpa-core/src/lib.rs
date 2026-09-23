//! Headless core for running I-Simpa's unchanged solvers.
//!
//! Every stage returns a typed result with a reason code; no stage downgrades a
//! failure to a warning. See `docs/rebuild-plan.md`.

pub mod config_xml;
pub mod formats;
pub mod schema;
pub mod validate;

/// Upstream commit the solvers are built from (tag `v1.4.0_snapshot_14_01_2026`).
pub const SOLVER_COMMIT: &str = "929a5c8e5f590b189f29155faeff8670f3699479";

/// Version of this crate.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn solver_commit_is_a_full_sha() {
        assert_eq!(SOLVER_COMMIT.len(), 40);
        assert!(SOLVER_COMMIT.bytes().all(|b| b.is_ascii_hexdigit()));
    }
}
