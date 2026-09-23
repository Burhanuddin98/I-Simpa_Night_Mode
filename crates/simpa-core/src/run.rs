//! Solver runs: the output-line classifier (`docs/solver-contract.md` Part B), SPPS particle
//! statistics, the expected output files, the run verdict and the run manifest. Milestone M6; see
//! `docs/m5-m6-design.md`.
//!
//! This is the half of a run that starts no process: the run manager feeds each
//! [`process::Line`](crate::process::Line) to a [`Classifier`] as it arrives, and after the child
//! exits derives the [`Expectation`] from the `config.xml` in the working directory, reads the
//! [`Outputs`], and asks [`judge`] for the [`Verdict`] it writes into the [`RunManifest`].
//!
//! - [`classify`] holds [`LINE_RULES`], the contract's table, and the streaming [`Classifier`].
//! - [`stats`] types the SPPS statistics table.
//! - [`expect`] reads what a `config.xml` asks for, and lists the files that follow from it.
//! - [`verdict`] makes the four-signal judgement and owns its reason codes.
//! - [`manifest`] is `run.json`.

pub mod classify;
pub mod expect;
pub mod manifest;
pub mod stats;
pub mod verdict;

pub use classify::{
    ClassCounts, Classified, Classifier, Continuation, LINE_RULES, LineClass, LineRule,
    classify_all,
};
pub use expect::{Band, ExpectError, Expectation, OutputNames, SppsSettings};
pub use manifest::{FileCounts, FileRef, MeshRef, RunManifest, RunSource};
pub use stats::{BandStats, ParticleStats, StatsError};
pub use verdict::{DEFAULT_LOSS_LIMIT, Evidence, Outputs, Reason, Status, Verdict, judge};
