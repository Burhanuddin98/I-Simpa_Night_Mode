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
//!   elements and attributes are written in a fixed order, and every spectrum in the band set's
//!   ascending order.
//! - **Lists last item first,** as upstream's GUI writes them: sources, point receivers, surface
//!   receivers and fitting zones. It holds a list in the order its elements were loaded or
//!   created and writes each one in front of the last, so its lists come out newest first. The
//!   solvers number sources and receivers by their position in the file, so this is part of
//!   their input (`docs/formats/config_xml.md`, "Parity with upstream's GUI").
//!   [`import_upstream`] reverses them back into project order.
//! - **`directivities_directory`** is upstream's [`names::DIRECTIVITY_DIR`] folder for every
//!   SPPS run, as its GUI writes it; for TCR, whose config upstream writes without it, only when
//!   a source has a directivity file, and `""` (the value TCR reads when it is absent) otherwise.
//! - **Numbers as the C locale reads them.** A real is the shortest decimal that reads back to the
//!   same `f64` bits (never more than C++'s `max_digits10`, 17 significant digits), with `.` as
//!   the decimal point, no exponent and no grouping; `atof` then reads exactly that `f64`, and the
//!   solver stores it as `value as f32`. A non-finite real is [`WriteError::NonFinite`]. Integers
//!   are plain decimal, and every switch is `"1"` or `"0"`, `docalc` included.
//! - **Spectra as upstream computes them:** white and pink noise on upstream's 27 bands are the
//!   `f32` levels upstream's GUI computes ([`band_levels_written`]); a transmitting material has
//!   no `affaiblissement` in a band that absorbs nothing, as upstream writes it.
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
//! [`SolverIds::assign`], from the project's own order and its pinned ids, so the same project
//! always gets the same ids, whatever its variant. Upstream's GUI writes its own element ids
//! (3503, 1930, ...); a project imported from a `.proj` keeps them as pins (each entity's
//! `solver_id`, set from its `wxid`; `docs/m5-m6-design.md`, decision 13), so it writes the ids
//! upstream wrote, and a project made here pins none and keeps the numbering below. The solvers
//! match the ids between the three files and write a scene receiver's id into its `.csbin`
//! output, and nothing else reads them:
//!
//! | Solver id | Written as | Assigned |
//! |---|---|---|
//! | material, per **surface group** | `type_surface@id`, `.cbin` face `idMat` | The group's base material's pinned [`Material::solver_id`](crate::schema::Material::solver_id) if it has one. Otherwise, in surface-group order, the smallest id from [`FIRST_ASSIGNED_MATERIAL_ID`] (1) up that no pinned id and no earlier group uses |
//! | surface receiver or cutting plane | `recepteur_surfacique@id`, `recepteur_surfacique_coupe@id`, `.cbin` face `idRs` | Its pinned `solver_id`; otherwise, in list order, the smallest id from 0 up that no pin and no earlier receiver uses: its position in `surface_receivers`, disabled ones counted, when none is pinned |
//! | fitting zone | `encombrement@id`, `.cbin` face `idEn`, `.mbin` `idVolume` | Its pinned `solver_id` (never 0); otherwise the same from [`FIRST_FITTING_ID`] (2) up: 2 plus its position in `fitting_zones`, disabled ones counted, when none is pinned |
//! | point receiver | `recepteur_ponctuel@id` | Its pinned `solver_id`; otherwise the same from 0 up: its position in `point_receivers` when none is pinned |
//!
//! Sources carry no id in `config.xml`: the solvers number them by position and do not read
//! `source@id`. An imported source keeps upstream's as its pinned `solver_id`, unwritten. Two
//! entities of one kind pinned to one id cannot be written ([`WriteError::SolverIdClash`]).
//!
//! **Materials are declared per surface group, not per library entry.** Each surface group gets
//! its own solver material id, and `config.xml` declares one `type_surface` per id, holding the
//! material that group carries under the chosen variant. The scene mesh therefore never depends on
//! the variant ([`scene_mesh`]): switching variants rewrites only `config.xml`, and a variant
//! changes exactly the `type_surface` elements of the groups it overrides. Materials no group uses
//! are not written. Groups whose base material pins the same id share one `type_surface`; a variant
//! that overrides some of them but not all cannot be expressed and is
//! [`WriteError::SharedSolverId`]. One more material is declared when the scene mesh carries a
//! drawn box zone's triangles: upstream's default material, id 0 ([`DRAWN_ZONE_MATERIAL_ID`]),
//! which those triangles carry as upstream's do, unless a surface group already declares id 0 (a
//! project pinned to a solver mesh's ids can), whose declaration they then share.
//!
//! Fitting ids start at 2 because TetGen's `-A` numbers a region that no region seed marks with
//! the next integer above the largest seeded attribute, starting from 1 (TetGen 1.5.0,
//! `tetgen.cxx:22403-22436`): a mesh made with no fitting regions gives every room tetrahedron the
//! attribute 1, and upstream's meshes keep it, so tutorial 1's `tetramesh.mbin` carries the room
//! as `idVolume` 1. The `.mbin` builder of [`crate::mesh`] writes the attribute unchanged too
//! (`docs/m5-m6-design.md`, decision 1): a seeded fitting carries its own id and the room's parts
//! the ids above the largest one. To the solver `idVolume` 0 means no fitting, and any other id
//! that no `encombrement` declares a NULL fitting, handed to the tetrahedron's scene faces
//! (`coreinitialisation.cpp:151-176`). So no fitting id can be 0, nor 1, the room's attribute
//! in TetGen's output and upstream's meshes.
//!
//! # Variants
//!
//! [`write()`]'s `variant` is `None` for the base project, or a variant's name or id (its
//! hyphenated UUID). A name two variants share is [`WriteError::VariantAmbiguous`]; pass the id.
//! To write what the GUI shows, pass `project.active_variant.map(|v| v.to_string())`.
//!
//! # The scene mesh
//!
//! [`scene_mesh`] takes every vertex through upstream's 32-bit OpenGL round trip ([`GlFrame`]), as
//! upstream's GUI writes its `.cbin`, so the solvers read the same `f32` bits for the same scene
//! (`docs/formats/cbin.md`, "Parity with upstream's GUI"). After the room's faces it appends each
//! enabled box zone's 12 triangles as upstream's GUI builds and appends them
//! ([`upstream_box_triangles`]; tutorial 3's box is its faces 88 to 99, byte for byte). The
//! mesher's `.poly` takes its vertices from the room alone ([`room_mesh`]), as upstream's does,
//! and its markers index the room's faces.
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

mod gl;
mod ids;
mod import;
mod num;
mod write;

pub use gl::GlFrame;
pub use ids::{
    DRAWN_ZONE_MATERIAL_ID, FIRST_ASSIGNED_MATERIAL_ID, FIRST_FITTING_ID, SolverIds, room_mesh,
    scene_mesh, upstream_box_triangles,
};
pub use import::{ImportError, import_upstream, import_upstream_with_mesh};
pub use num::widen_f32;
pub(crate) use write::transmission_loss_written;
pub use write::{
    StagedFile, WriteError, band_levels_written, directivity_files, resolve_variant,
    working_directory, write, write_file,
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
    /// `simulation@directivities_directory`: upstream's `CONST_REPORT_DIRECTIVITIES_FOLDER_PATH`
    /// (`appconfig.cpp:61`), written for every SPPS run and for a TCR run with a directivity file.
    pub const DIRECTIVITY_DIR: &str = "loudspeakers";
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
