//! The project model and its file format, `.simpa`.
//!
//! A [`Project`] holds everything the solvers' `config.xml` needs (every row of the contract
//! survey in `docs/rebuild-plan-raw-2026-09-23.json`, `surveys[contract].config_xml`), plus the
//! geometry and GUI state the solvers never see. Conventions, decided 2026-09-23:
//!
//! - **World units:** `f64` metres, Z up ([`Frame`]).
//! - **Ids** are UUIDs ([`GroupId`], [`MaterialId`], ...). They never reach a solver as counters:
//!   solver integer ids are assigned at export. The one exception is [`Material::solver_id`], an
//!   optional pinned id that lets a project imported from a solver mesh keep its `idMat` values.
//! - **One band set per project** ([`BandSet`], default octave 125-4000 Hz, third-octave
//!   selectable). Every per-band array (materials, sources, fitting zones, solver band switches)
//!   has exactly one value per band, in the band set's ascending order. The solvers map arrays by
//!   sorted position (`base_core_configuration.cpp:98-152`), so this is what keeps them aligned.
//! - **Variants** are named sets of material overrides per surface group on top of the base
//!   project ([`Variant`]); [`Project::active_variant`] picks one, `None` meaning the base.
//! - **Air absorption carries its unit** ([`AirAbsorption`]), because the solver reads
//!   `@absatmo` verbatim as a per-metre energy attenuation.
//!
//! # File format
//!
//! Versioned JSON ([`FORMAT_VERSION`], extension [`FILE_EXTENSION`]), UTF-8, written by one
//! canonical writer ([`to_json`]): the same project always gives the same bytes, and
//! `to_json(&from_json(&to_json(p))?) == to_json(p)` for every project that passes
//! [`Project::check_integrity`]. That is every project [`from_json`] returns, every result of an
//! [`Op`] and every [`generate()`] output; a project built by hand with, say, a dangling
//! reference still serialises, but [`from_json`] refuses it. Objects are indented by
//! two spaces, one key per line, in declaration order. An array holding only scalars sits on one
//! line; an array holding objects or arrays puts each element on its own line. The file ends with
//! a newline. Every key is required on load (an absent optional value is written `null`), an
//! unknown key is refused in every object, tagged ones included (`{"kind": "omni", "file": ..}`
//! is an error, not an omni source), and the version is checked before anything else
//! ([`LoadError::Version`]).
//!
//! **Floats round-trip bit-exactly**, NaN payloads and -0.0 included:
//! - a finite value is a JSON number, written as the shortest decimal that reads back to the
//!   same bits (serde_json's `ryu` output, for example `0.3`, `-0.0`, `1e-7`);
//! - a non-finite value is a JSON string: `"Infinity"`, `"-Infinity"`, `"NaN"` for the quiet NaN
//!   `0x7ff8000000000000`, and `"NaN:0x<16 hex digits>"` for any other NaN bit pattern.
//!
//! serde_json's own reader cannot be used for loading: built without its `float_roundtrip`
//! feature (as here), it computes `significand as f64 * 10^exp` (`de.rs`, `f64_from_parts`),
//! which is not correctly rounded; `tests/schema_roundtrip.rs` counts how often it misreads the
//! writer's output. [`from_json`] therefore parses with its own strict
//! RFC 8259 reader ([`parse_json`]), which converts every number with Rust's correctly rounded
//! `str::parse`, and only then hands the tree to serde. Never load a project with
//! `serde_json::from_str`. The same holds for anything else that arrives as text: an [`Op`] from
//! the UI goes through [`Op::from_json`], any other schema type through [`from_json_exact`].
//! Writing is safe with serde_json (`to_string`, `to_value`): it prints the shortest round-trip
//! decimal.
//!
//! # Integrity and validity
//!
//! [`Project::check_integrity`] checks the structure: ids unique, every reference resolves,
//! every per-band array has one value per band, face indices in range, and every integer the
//! solver reads as a C `int` at most [`SOLVER_INT_MAX`]. [`from_json`] refuses a
//! project that fails it, and every [`Op`] keeps it. Whether a project can be *run* (values in
//! range, points inside the room, names safe as file names) is `core::validate`'s job.
//!
//! # Undo
//!
//! Edits are [`Op`]s. [`Op::apply`] either changes the project and returns the exact inverse,
//! or returns an [`OpError`] and changes nothing. [`History`] keeps the undo and redo stacks.

mod bands;
mod generate;
mod ids;
mod integrity;
mod json;
mod model;
mod ops;
mod real;

pub use bands::{BandKind, BandSet, Spectrum, SpectrumShape};
pub use generate::generate;
pub use ids::{
    FittingZoneId, GroupId, MaterialId, PointReceiverId, ProjectId, SourceId, SurfaceReceiverId,
    VariantId,
};
pub use integrity::IntegrityError;
pub use json::{
    LoadError, RETRY_DELAYS_MS, from_json, from_json_exact, load, parse_json, save, to_json,
};
pub use model::*;
pub use ops::{
    BandData, EntityRef, FittingBands, FittingQuantity, History, MaterialBands, MaterialQuantity,
    Op, OpError, SolverKind,
};
pub use real::{F64, Vec3};

/// The JSON Schema of a project file, generated from the types (for CLI validation and UI forms).
pub fn json_schema() -> schemars::Schema {
    schemars::schema_for!(Project)
}
