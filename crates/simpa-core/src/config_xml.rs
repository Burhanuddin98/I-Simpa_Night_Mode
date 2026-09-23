//! `config.xml`: the solver configuration SPPS and TCR read, written from a [`Project`], and read
//! back from a file upstream's GUI wrote.
//!
//! What the solvers read, and what each attribute does when it is missing, is specified in
//! `docs/formats/config_xml.md`. The column **Writer** of that page's reference tables is this
//! module's obligation, attribute by attribute; `tests/config_xml_*.rs` parse the page and hold
//! the writer to it.
//!
//! [`Project`]: crate::schema::Project
//!
//! # Writing
//!
//! [`write()`] returns the file for one solver ([`SolverKind`]), one variant and one run folder.
//! It needs a project that passes [`Project::check_integrity`](crate::schema::Project::check_integrity),
//! and refuses anything it cannot express exactly. The text is:
//!
//! - **UTF-8** with no byte-order mark, an XML declaration saying so, `\n` line ends and a
//!   two-space indent. Attribute values escape `& < > "` and tab, line feed and carriage return;
//!   a character XML 1.0 cannot carry at all is [`WriteError::Unencodable`].
//! - **Deterministic.** The same project, solver, variant and folder always give the same bytes:
//!   elements and attributes are written in a fixed order, lists in the project's order, and every
//!   spectrum in the band set's ascending order.
//! - **Numbers as the C locale reads them.** A real is the shortest decimal that reads back to the
//!   same `f64` bits (never more than C++'s `max_digits10`, 17 significant digits), with `.` as
//!   the decimal point, no exponent and no grouping; `atof` then reads exactly that `f64`, and the
//!   solver stores it as `value as f32`. A non-finite real is [`WriteError::NonFinite`]. Integers
//!   are plain decimal, and every switch is `"1"` or `"0"`, `docalc` included.
//! - **`workingdirectory`** is the run folder as an absolute path ending in the platform
//!   separator ([`working_directory`]).
//! - **No dead payload:** no `<surface_mesh>`, `<vertices>` or `<subdomains>`, and none of the
//!   attributes `docs/formats/config_xml.md` lists as ignored by the solvers. Air absorption is
//!   always explicit (`disable_absatmo_computation` and `absatmo`, the latter in 1/m whatever the
//!   project's unit), and so is every per-receiver background-noise band and `trans_epsilon`.
//!
//! # Solver ids
//!
//! The project's ids are UUIDs and never reach a solver. The integers the solvers match between
//! `config.xml`, the scene mesh (`.cbin`) and the tetrahedral mesh (`.mbin`) are assigned here, by
//! [`SolverIds::assign`], from the project's own order, so the same project always gets the same
//! ids, whatever its variant:
//!
//! | Solver id | Written as | Assigned |
//! |---|---|---|
//! | material, per **surface group** | `type_surface@id`, `.cbin` face `idMat` | The group's base material's pinned [`Material::solver_id`](crate::schema::Material::solver_id) if it has one. Otherwise, in surface-group order, the smallest id from [`FIRST_ASSIGNED_MATERIAL_ID`] (1) up that no pinned id and no earlier group uses |
//! | surface receiver or cutting plane | `recepteur_surfacique@id`, `recepteur_surfacique_coupe@id`, `.cbin` face `idRs` | Its position in `surface_receivers`, from 0, disabled ones counted |
//! | fitting zone | `encombrement@id`, `.cbin` face `idEn`, `.mbin` `idVolume` | [`FIRST_FITTING_ID`] (2) plus its position in `fitting_zones`, disabled ones counted |
//! | point receiver | `recepteur_ponctuel@id` | Its position in `point_receivers`, from 0 |
//!
//! Sources carry no id: the solvers number them by position and do not read `source@id`.
//!
//! **Materials are declared per surface group, not per library entry.** Each surface group gets
//! its own solver material id, and `config.xml` declares one `type_surface` per id, holding the
//! material that group carries under the chosen variant. The scene mesh therefore never depends on
//! the variant ([`scene_mesh`]): switching variants rewrites only `config.xml`, and a variant
//! changes exactly the `type_surface` elements of the groups it overrides. Materials no group uses
//! are not written. Groups whose base material pins the same id share one `type_surface`; a variant
//! that overrides some of them but not all cannot be expressed and is
//! [`WriteError::SharedSolverId`].
//!
//! Fitting ids start at 2 because TetGen's `-A` numbers a region that no region seed marks with
//! the next integer above the largest seeded attribute, starting from 1 (`tetgen.cxx:24218-24300`):
//! a mesh made with no fitting regions gives every room tetrahedron the attribute 1, and
//! upstream's meshes keep it, so tutorial 1's `tetramesh.mbin` carries the room as `idVolume` 1.
//! The `.mbin` builder of [`crate::mesh`] does not: it writes the room as `idVolume` 0, the
//! solver's "main volume" (`coreTypes.h:445`), and a seeded fitting as its own id
//! (`docs/m5-m6-design.md`, decision 1). To the solver `idVolume` 0 means no fitting. So no
//! fitting id can be 0, nor 1, the room's attribute in TetGen's output and upstream's meshes.
//!
//! # Variants
//!
//! [`write()`]'s `variant` is `None` for the base project, or a variant's name or id (its
//! hyphenated UUID). A name two variants share is [`WriteError::VariantAmbiguous`]; pass the id.
//! To write what the GUI shows, pass `project.active_variant.map(|v| v.to_string())`.
//!
//! # Importing
//!
//! [`import_upstream`] and [`import_upstream_with_mesh`] read a `config.xml` upstream's GUI wrote
//! into a [`Project`](crate::schema::Project). Every value is read the way the solver reads it, and
//! every real is widened from the `f32` the solver would hold to the `f64` whose shortest decimal
//! is that `f32`'s shortest decimal ([`widen_f32`]): `0.310000002384186` becomes `0.31`, and
//! writing the project back gives the solver the identical `f32`. Spectra become a pink or white
//! [`Spectrum`](crate::schema::Spectrum) when one reproduces every band's `f32` exactly, and a
//! custom one otherwise.

mod ids;
mod import;
mod num;
mod write;

pub use ids::{FIRST_ASSIGNED_MATERIAL_ID, FIRST_FITTING_ID, SolverIds, scene_mesh};
pub use import::{ImportError, import_upstream, import_upstream_with_mesh};
pub use num::widen_f32;
pub use write::{
    StagedFile, WriteError, directivity_files, resolve_variant, working_directory, write,
    write_file,
};

pub use crate::schema::SolverKind;

/// File and folder names the written `config.xml` gives the solvers, relative to the run folder.
/// They are upstream's GUI's own names. Folder names are given without their trailing separator;
/// the writer adds the platform's.
pub mod names {
    /// The file [`write_file`](super::write_file) writes, and the solvers' one argument.
    pub const CONFIG: &str = "config.xml";
    /// `simulation@modelName`: the scene mesh the run folder must hold.
    pub const SCENE_MESH: &str = "mesh.cbin";
    /// `simulation@tetrameshFileName`: the tetrahedral mesh the run folder must hold.
    pub const TETRA_MESH: &str = "tetramesh.mbin";
    /// `simulation@recepteurss_directory`.
    pub const SURFACE_RECEIVER_DIR: &str = "Surface receiver";
    /// `simulation@recepteurss_filename`.
    pub const SURFACE_RECEIVER_FILE: &str = "Sound level.csbin";
    /// `simulation@recepteurss_cut_filename`.
    pub const CUTTING_PLANE_FILE: &str = "rs_cut.csbin";
    /// `simulation@receiversp_directory` (the solver appends the separator itself).
    pub const POINT_RECEIVER_DIR: &str = "Punctual receivers";
    /// `simulation@receiversp_filename`.
    pub const POINT_RECEIVER_FILE: &str = "Sound level.recp";
    /// `simulation@receiversp_filename_adv`.
    pub const POINT_RECEIVER_ADVANCED_FILE: &str = "Advanced sound level.gap";
    /// `simulation@cumul_filename`.
    pub const TOTAL_ENERGY_FILE: &str = "Total energy.recp";
    /// `simulation@directivities_directory`, when a source uses a directivity file.
    pub const DIRECTIVITY_DIR: &str = "directivities";
    /// SPPS `simulation@stats_filename`.
    pub const SPPS_STATS_FILE: &str = "SPPS particle statistics.gabe";
    /// SPPS `simulation@particules_directory`.
    pub const SPPS_PARTICLE_DIR: &str = "Particles";
    /// SPPS `simulation@particules_filename`.
    pub const SPPS_PARTICLE_FILE: &str = "particles.pbin";
    /// TCR `simulation@direct_recepteurSOutputName`.
    pub const TCR_DIRECT_DIR: &str = "Direct field";
    /// TCR `simulation@sabine_recepteurSOutputName`.
    pub const TCR_SABINE_DIR: &str = "Total field (Sabine)";
    /// TCR `simulation@eyring_recepteurSOutputName`.
    pub const TCR_EYRING_DIR: &str = "Total field (Eyring)";
}
