//! Structural integrity: the invariants every loaded project and every [`Op`](super::Op) keeps.

use std::collections::HashSet;
use std::fmt;
use std::hash::Hash;

use uuid::Uuid;

use super::bands::{BandSet, Spectrum, SpectrumShape};
use super::ids::GroupId;
use super::model::{
    FORMAT_VERSION, Face, FittingShape, FittingZone, Material, PointReceiver, Project,
    SOLVER_INT_MAX, SolverSettings, Source, SurfaceGroup, SurfaceReceiver, SurfaceReceiverShape,
    Variant,
};

/// A broken structural invariant. [`IntegrityError::code`] is a stable reason code.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum IntegrityError {
    /// `format_version` is not [`FORMAT_VERSION`].
    Version { found: u32 },
    /// The band set is empty, not ascending or holds a non-nominal frequency.
    Bands(String),
    /// A per-band array does not have one value per band.
    BandCount {
        what: String,
        expected: usize,
        found: usize,
    },
    /// Two entities of one kind share an id.
    DuplicateId { kind: &'static str, id: Uuid },
    /// A reference to an entity that does not exist.
    Dangling {
        what: String,
        kind: &'static str,
        id: Uuid,
    },
    /// A face indexes past the vertex list.
    FaceVertex { face: usize, vertex: u32 },
    /// A list of group references names one group twice.
    RepeatedGroup { what: String, group: GroupId },
    /// A variant's overrides are not strictly ascending by group id.
    OverrideOrder { variant: Uuid },
    /// Two materials pin the same solver id.
    DuplicateSolverId { solver_id: u32 },
    /// An integer the solver reads as a C `int` is above [`SOLVER_INT_MAX`].
    SolverInt { what: String, value: u32 },
}

impl IntegrityError {
    pub fn code(&self) -> &'static str {
        match self {
            IntegrityError::Version { .. } => "version",
            IntegrityError::Bands(_) => "bands",
            IntegrityError::BandCount { .. } => "band_count",
            IntegrityError::DuplicateId { .. } => "duplicate_id",
            IntegrityError::Dangling { .. } => "dangling_reference",
            IntegrityError::FaceVertex { .. } => "face_vertex",
            IntegrityError::RepeatedGroup { .. } => "repeated_group",
            IntegrityError::OverrideOrder { .. } => "override_order",
            IntegrityError::DuplicateSolverId { .. } => "duplicate_solver_id",
            IntegrityError::SolverInt { .. } => "solver_int_range",
        }
    }
}

impl fmt::Display for IntegrityError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            IntegrityError::Version { found } => write!(
                f,
                "format_version is {found}, but this build holds version {FORMAT_VERSION}"
            ),
            IntegrityError::Bands(msg) => write!(f, "band set: {msg}"),
            IntegrityError::BandCount {
                what,
                expected,
                found,
            } => write!(
                f,
                "{what} has {found} per-band values, but the project has {expected} bands"
            ),
            IntegrityError::DuplicateId { kind, id } => write!(f, "two {kind}s share the id {id}"),
            IntegrityError::Dangling { what, kind, id } => {
                write!(f, "{what} refers to {kind} {id}, which does not exist")
            }
            IntegrityError::FaceVertex { face, vertex } => {
                write!(f, "face {face} uses vertex {vertex}, past the vertex list")
            }
            IntegrityError::RepeatedGroup { what, group } => {
                write!(f, "{what} lists surface group {group} more than once")
            }
            IntegrityError::OverrideOrder { variant } => write!(
                f,
                "variant {variant}: overrides are not strictly ascending by group id"
            ),
            IntegrityError::DuplicateSolverId { solver_id } => {
                write!(f, "two materials pin the solver id {solver_id}")
            }
            IntegrityError::SolverInt { what, value } => write!(
                f,
                "{what} is {value}, but the solver reads it as a C int (at most {SOLVER_INT_MAX})"
            ),
        }
    }
}

impl std::error::Error for IntegrityError {}

type Result<T = ()> = std::result::Result<T, IntegrityError>;

pub(crate) fn band_count(what: impl FnOnce() -> String, found: usize, n: usize) -> Result {
    if found == n {
        Ok(())
    } else {
        Err(IntegrityError::BandCount {
            what: what(),
            expected: n,
            found,
        })
    }
}

fn unique<I: Copy + Eq + Hash + Into<Uuid>>(
    kind: &'static str,
    ids: impl Iterator<Item = I>,
) -> Result {
    let mut seen = HashSet::new();
    for id in ids {
        if !seen.insert(id) {
            return Err(IntegrityError::DuplicateId {
                kind,
                id: id.into(),
            });
        }
    }
    Ok(())
}

/// `value` fits the C `int` the solver reads it into.
fn solver_int(what: impl FnOnce() -> String, value: u32) -> Result {
    if value <= SOLVER_INT_MAX {
        Ok(())
    } else {
        Err(IntegrityError::SolverInt {
            what: what(),
            value,
        })
    }
}

pub(crate) fn check_bands(bands: &BandSet) -> Result {
    match bands.problem() {
        None => Ok(()),
        Some(msg) => Err(IntegrityError::Bands(msg)),
    }
}

pub(crate) fn check_spectrum(what: impl FnOnce() -> String, s: &Spectrum, n: usize) -> Result {
    match &s.shape {
        SpectrumShape::Custom { relative_db } => band_count(what, relative_db.len(), n),
        SpectrumShape::Pink | SpectrumShape::White => Ok(()),
    }
}

pub(crate) fn check_material(m: &Material, n: usize) -> Result {
    let what = |field: &str| format!("material '{}' {field}", m.name);
    band_count(|| what("absorption"), m.absorption.len(), n)?;
    band_count(|| what("scattering"), m.scattering.len(), n)?;
    if let Some(k) = m.reflection_law.band_count() {
        band_count(|| what("reflection law"), k, n)?;
    }
    if let Some(t) = &m.transmission_loss_db {
        band_count(|| what("transmission loss"), t.len(), n)?;
    }
    if let Some(id) = m.solver_id {
        solver_int(|| what("solver_id"), id)?;
    }
    Ok(())
}

pub(crate) fn check_source(s: &Source, n: usize) -> Result {
    check_spectrum(|| format!("source '{}' power", s.name), &s.power, n)
}

pub(crate) fn check_point_receiver(r: &PointReceiver, n: usize) -> Result {
    match &r.background_noise {
        Some(noise) => check_spectrum(
            || format!("receiver '{}' background noise", r.name),
            noise,
            n,
        ),
        None => Ok(()),
    }
}

/// Every group in `groups` exists and appears once.
fn check_group_list(
    what: impl Fn() -> String,
    groups: &[GroupId],
    exists: &dyn Fn(GroupId) -> bool,
) -> Result {
    let mut seen = HashSet::new();
    for &g in groups {
        if !exists(g) {
            return Err(IntegrityError::Dangling {
                what: what(),
                kind: "surface group",
                id: g.0,
            });
        }
        if !seen.insert(g) {
            return Err(IntegrityError::RepeatedGroup {
                what: what(),
                group: g,
            });
        }
    }
    Ok(())
}

pub(crate) fn check_surface_receiver(
    r: &SurfaceReceiver,
    exists: &dyn Fn(GroupId) -> bool,
) -> Result {
    match &r.shape {
        SurfaceReceiverShape::Scene { groups } => {
            check_group_list(|| format!("surface receiver '{}'", r.name), groups, exists)
        }
        SurfaceReceiverShape::CuttingPlane { .. } => Ok(()),
    }
}

pub(crate) fn check_fitting_zone(
    z: &FittingZone,
    n: usize,
    exists: &dyn Fn(GroupId) -> bool,
) -> Result {
    let what = |field: &str| format!("fitting zone '{}' {field}", z.name);
    band_count(|| what("absorption"), z.absorption.len(), n)?;
    band_count(|| what("mean free path"), z.mean_free_path_m.len(), n)?;
    band_count(|| what("diffusion law"), z.diffusion_law.len(), n)?;
    match &z.shape {
        FittingShape::Surfaces { groups, .. } => {
            check_group_list(|| format!("fitting zone '{}'", z.name), groups, exists)
        }
        FittingShape::Box { .. } => Ok(()),
    }
}

pub(crate) fn check_group(
    g: &SurfaceGroup,
    material_exists: &dyn Fn(super::ids::MaterialId) -> bool,
) -> Result {
    if material_exists(g.material) {
        Ok(())
    } else {
        Err(IntegrityError::Dangling {
            what: format!("surface group '{}'", g.name),
            kind: "material",
            id: g.material.0,
        })
    }
}

pub(crate) fn check_variant(
    v: &Variant,
    group_exists: &dyn Fn(GroupId) -> bool,
    material_exists: &dyn Fn(super::ids::MaterialId) -> bool,
) -> Result {
    if v.overrides.windows(2).any(|w| w[0].group >= w[1].group) {
        return Err(IntegrityError::OverrideOrder { variant: v.id.0 });
    }
    for o in &v.overrides {
        if !group_exists(o.group) {
            return Err(IntegrityError::Dangling {
                what: format!("variant '{}'", v.name),
                kind: "surface group",
                id: o.group.0,
            });
        }
        if !material_exists(o.material) {
            return Err(IntegrityError::Dangling {
                what: format!("variant '{}'", v.name),
                kind: "material",
                id: o.material.0,
            });
        }
    }
    Ok(())
}

pub(crate) fn check_face(
    i: usize,
    f: &Face,
    n_vertices: usize,
    group_exists: &dyn Fn(GroupId) -> bool,
) -> Result {
    if let Some(&v) = f.vertices.iter().find(|&&v| v as usize >= n_vertices) {
        return Err(IntegrityError::FaceVertex { face: i, vertex: v });
    }
    if !group_exists(f.group) {
        return Err(IntegrityError::Dangling {
            what: format!("face {i}"),
            kind: "surface group",
            id: f.group.0,
        });
    }
    Ok(())
}

pub(crate) fn check_solver_settings(s: &SolverSettings, n: usize) -> Result {
    let spps = &s.spps;
    solver_int(
        || "SPPS particles_per_source".to_string(),
        spps.particles_per_source,
    )?;
    solver_int(|| "SPPS particles_saved".to_string(), spps.particles_saved)?;
    solver_int(|| "SPPS random_seed".to_string(), spps.random_seed)?;
    band_count(
        || "SPPS bands_computed".to_string(),
        s.spps.bands_computed.len(),
        n,
    )?;
    band_count(
        || "TCR bands_computed".to_string(),
        s.tcr.bands_computed.len(),
        n,
    )
}

/// Pinned solver ids are unique.
pub(crate) fn check_solver_ids<'a>(materials: impl Iterator<Item = &'a Material>) -> Result {
    let mut seen = HashSet::new();
    for id in materials.filter_map(|m| m.solver_id) {
        if !seen.insert(id) {
            return Err(IntegrityError::DuplicateSolverId { solver_id: id });
        }
    }
    Ok(())
}

impl Project {
    /// Checks every structural invariant and returns the first one broken:
    /// the format version; the band set; one value per band in every per-band array; unique ids
    /// within each kind of entity; every reference (face to group, group to material, receivers,
    /// fitting zones and variants to groups and materials, the active variant) resolves; face
    /// vertex indices in range; variant overrides strictly ascending by group; group lists
    /// without repeats; pinned material solver ids unique; every integer the solver reads as a
    /// C `int` (pinned solver ids, SPPS particle counts and random seed) at most
    /// [`SOLVER_INT_MAX`], since a larger one cannot be written to config.xml at all.
    ///
    /// Other value ranges, geometry and names are not checked here: that is `core::validate`.
    pub fn check_integrity(&self) -> Result {
        if self.format_version != FORMAT_VERSION {
            return Err(IntegrityError::Version {
                found: self.format_version,
            });
        }
        check_bands(&self.bands)?;
        let n = self.bands.len();

        unique("surface group", self.surface_groups.iter().map(|g| g.id))?;
        unique("material", self.materials.iter().map(|m| m.id))?;
        unique("source", self.sources.iter().map(|s| s.id))?;
        unique("point receiver", self.point_receivers.iter().map(|r| r.id))?;
        unique(
            "surface receiver",
            self.surface_receivers.iter().map(|r| r.id),
        )?;
        unique("fitting zone", self.fitting_zones.iter().map(|z| z.id))?;
        unique("variant", self.variants.iter().map(|v| v.id))?;

        let groups: HashSet<GroupId> = self.surface_groups.iter().map(|g| g.id).collect();
        let materials: HashSet<_> = self.materials.iter().map(|m| m.id).collect();
        let group_exists = |g: GroupId| groups.contains(&g);
        let material_exists = |m| materials.contains(&m);

        for m in &self.materials {
            check_material(m, n)?;
        }
        check_solver_ids(self.materials.iter())?;
        for g in &self.surface_groups {
            check_group(g, &material_exists)?;
        }
        let n_vertices = self.geometry.vertices.len();
        for (i, f) in self.geometry.faces.iter().enumerate() {
            check_face(i, f, n_vertices, &group_exists)?;
        }
        for s in &self.sources {
            check_source(s, n)?;
        }
        for r in &self.point_receivers {
            check_point_receiver(r, n)?;
        }
        for r in &self.surface_receivers {
            check_surface_receiver(r, &group_exists)?;
        }
        for z in &self.fitting_zones {
            check_fitting_zone(z, n, &group_exists)?;
        }
        check_solver_settings(&self.solvers, n)?;
        for v in &self.variants {
            check_variant(v, &group_exists, &material_exists)?;
        }
        if let Some(active) = self.active_variant
            && self.variant(active).is_none()
        {
            return Err(IntegrityError::Dangling {
                what: "active_variant".to_string(),
                kind: "variant",
                id: active.0,
            });
        }
        Ok(())
    }
}
