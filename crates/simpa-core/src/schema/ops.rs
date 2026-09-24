//! Edits as invertible operations, and the undo history built on them.

use std::fmt;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::bands::{BandSet, SpectrumShape};
use super::ids::{
    FittingZoneId, GroupId, MaterialId, PointReceiverId, SourceId, SurfaceReceiverId, VariantId,
};
use super::integrity::{self, IntegrityError};
use super::json::{LoadError, from_json_exact};
use super::model::{
    Camera, DiffusionLaw, Environment, FittingShape, FittingZone, Geometry, Material, Nullable,
    PointReceiver, Project, ReflectionLaws, SolverSettings, Source, SurfaceGroup, SurfaceReceiver,
    SurfaceReceiverShape, Variant, required,
};
use super::real::{F64, Vec3};

/// A named entity of the project. In JSON: `{"kind": "material", "id": "<uuid>"}`; any other
/// key is refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(
    tag = "kind",
    content = "id",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum EntityRef {
    SurfaceGroup(GroupId),
    Material(MaterialId),
    Source(SourceId),
    PointReceiver(PointReceiverId),
    SurfaceReceiver(SurfaceReceiverId),
    FittingZone(FittingZoneId),
    Variant(VariantId),
}

/// A per-band quantity of a material.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum MaterialQuantity {
    Absorption,
    Scattering,
    /// Only in a band where the material transmits (its loss is not `None` there).
    TransmissionLoss,
}

/// A per-band quantity of a fitting zone.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum FittingQuantity {
    Absorption,
    MeanFreePath,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SolverKind {
    Spps,
    Tcr,
}

/// A material's per-band values.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MaterialBands {
    pub absorption: Vec<F64>,
    pub scattering: Vec<F64>,
    /// One law, which fits any band set, or one per band.
    pub reflection_law: ReflectionLaws,
    #[serde(deserialize_with = "required")]
    #[schemars(with = "Nullable<Vec<Option<F64>>>")]
    pub transmission_loss_db: Option<Vec<Option<F64>>>,
}

/// A fitting zone's per-band arrays.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FittingBands {
    pub absorption: Vec<F64>,
    pub mean_free_path_m: Vec<F64>,
    pub diffusion_law: Vec<DiffusionLaw>,
}

/// Every per-band array of a project, keyed by entity in project order: what changes with the
/// band set. Custom spectrum shapes are `Some`, other shapes `None`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BandData {
    pub materials: Vec<(MaterialId, MaterialBands)>,
    pub source_shapes: Vec<(SourceId, Option<Vec<F64>>)>,
    pub background_noise_shapes: Vec<(PointReceiverId, Option<Vec<F64>>)>,
    pub fitting_zones: Vec<(FittingZoneId, FittingBands)>,
    pub spps_bands_computed: Vec<bool>,
    pub tcr_bands_computed: Vec<bool>,
}

/// One edit. [`Op::apply`] returns its exact inverse.
///
/// `Add*` ops insert at `index` (at most the list length); their inverse is `Remove*` by id,
/// whose inverse re-inserts at the same index. `Replace*` ops swap a whole entity, found by its
/// id. A removal is refused while anything refers to the entity.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum Op {
    SetProjectName {
        name: String,
    },
    SetDescription {
        description: String,
    },
    Rename {
        target: EntityRef,
        name: String,
    },
    /// Changes the band set and every per-band array at once; see [`Project::rebanded`].
    SetBands {
        bands: BandSet,
        data: BandData,
    },
    /// Replaces the whole mesh (a re-import). Every face's group must exist.
    SetGeometry {
        geometry: Geometry,
    },
    AddSurfaceGroup {
        index: usize,
        group: SurfaceGroup,
    },
    RemoveSurfaceGroup {
        id: GroupId,
    },
    SetGroupMaterial {
        group: GroupId,
        material: MaterialId,
    },
    AddMaterial {
        index: usize,
        material: Material,
    },
    RemoveMaterial {
        id: MaterialId,
    },
    ReplaceMaterial {
        material: Material,
    },
    SetMaterialBand {
        material: MaterialId,
        quantity: MaterialQuantity,
        band: usize,
        value: F64,
    },
    AddSource {
        index: usize,
        source: Source,
    },
    RemoveSource {
        id: SourceId,
    },
    ReplaceSource {
        source: Source,
    },
    MoveSource {
        id: SourceId,
        position: Vec3,
    },
    SetSourceEnabled {
        id: SourceId,
        enabled: bool,
    },
    AddPointReceiver {
        index: usize,
        receiver: PointReceiver,
    },
    RemovePointReceiver {
        id: PointReceiverId,
    },
    ReplacePointReceiver {
        receiver: PointReceiver,
    },
    MovePointReceiver {
        id: PointReceiverId,
        position: Vec3,
    },
    AddSurfaceReceiver {
        index: usize,
        receiver: SurfaceReceiver,
    },
    RemoveSurfaceReceiver {
        id: SurfaceReceiverId,
    },
    ReplaceSurfaceReceiver {
        receiver: SurfaceReceiver,
    },
    AddFittingZone {
        index: usize,
        zone: FittingZone,
    },
    RemoveFittingZone {
        id: FittingZoneId,
    },
    ReplaceFittingZone {
        zone: FittingZone,
    },
    SetFittingBand {
        zone: FittingZoneId,
        quantity: FittingQuantity,
        band: usize,
        value: F64,
    },
    SetEnvironment {
        environment: Environment,
    },
    SetSolverSettings {
        settings: SolverSettings,
    },
    SetBandComputed {
        solver: SolverKind,
        band: usize,
        computed: bool,
    },
    AddVariant {
        index: usize,
        variant: Variant,
    },
    /// Refused while the variant is active.
    RemoveVariant {
        id: VariantId,
    },
    /// Sets (`Some`) or removes (`None`) a variant's override on one group.
    SetVariantOverride {
        variant: VariantId,
        group: GroupId,
        #[serde(deserialize_with = "required")]
        #[schemars(with = "Nullable<MaterialId>")]
        material: Option<MaterialId>,
    },
    SetActiveVariant {
        #[serde(deserialize_with = "required")]
        #[schemars(with = "Nullable<VariantId>")]
        variant: Option<VariantId>,
    },
    SetCamera {
        #[serde(deserialize_with = "required")]
        #[schemars(with = "Nullable<Camera>")]
        camera: Option<Camera>,
    },
    /// Applies `ops` in order, all or nothing. The inverse is the inverses in reverse order.
    Batch {
        ops: Vec<Op>,
    },
}

/// Why an op was refused. The project is unchanged. [`OpError::code`] is a stable reason code.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OpError {
    NotFound {
        kind: &'static str,
        id: Uuid,
    },
    DuplicateId {
        kind: &'static str,
        id: Uuid,
    },
    /// An insertion index past the end of the list.
    Index {
        kind: &'static str,
        index: usize,
        len: usize,
    },
    /// A band index past the band set.
    Band {
        band: usize,
        bands: usize,
    },
    /// A removal of an entity something still refers to.
    InUse {
        kind: &'static str,
        id: Uuid,
        by: String,
    },
    /// A transmission-loss edit on a material, or a band of it, that does not transmit.
    NoTransmission {
        material: Uuid,
    },
    /// A removal of the active variant.
    ActiveVariant {
        variant: Uuid,
    },
    /// [`BandData`] that does not match the project's entities.
    BandData(String),
    /// The result would break a structural invariant.
    Integrity(IntegrityError),
    /// An op inside a [`Op::Batch`] failed; the ops before it were rolled back.
    Batch {
        index: usize,
        error: Box<OpError>,
    },
}

impl OpError {
    pub fn code(&self) -> &'static str {
        match self {
            OpError::NotFound { .. } => "not_found",
            OpError::DuplicateId { .. } => "duplicate_id",
            OpError::Index { .. } => "index",
            OpError::Band { .. } => "band",
            OpError::InUse { .. } => "in_use",
            OpError::NoTransmission { .. } => "no_transmission",
            OpError::ActiveVariant { .. } => "active_variant",
            OpError::BandData(_) => "band_data",
            OpError::Integrity(_) => "integrity",
            OpError::Batch { .. } => "batch",
        }
    }
}

impl fmt::Display for OpError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OpError::NotFound { kind, id } => write!(f, "no {kind} with id {id}"),
            OpError::DuplicateId { kind, id } => write!(f, "a {kind} with id {id} already exists"),
            OpError::Index { kind, index, len } => {
                write!(f, "cannot insert a {kind} at {index}: the list holds {len}")
            }
            OpError::Band { band, bands } => {
                write!(
                    f,
                    "band {band} does not exist: the project has {bands} bands"
                )
            }
            OpError::InUse { kind, id, by } => write!(f, "{kind} {id} is still used by {by}"),
            OpError::NoTransmission { material } => {
                write!(f, "material {material} has no transmission loss to edit")
            }
            OpError::ActiveVariant { variant } => {
                write!(
                    f,
                    "variant {variant} is active; switch variants before removing it"
                )
            }
            OpError::BandData(msg) => write!(f, "band data does not match the project: {msg}"),
            OpError::Integrity(e) => write!(f, "{e}"),
            OpError::Batch { index, error } => write!(f, "op {index} of the batch: {error}"),
        }
    }
}

impl std::error::Error for OpError {}

impl From<IntegrityError> for OpError {
    fn from(e: IntegrityError) -> Self {
        OpError::Integrity(e)
    }
}

type Result<T> = std::result::Result<T, OpError>;

fn position<T, I>(list: &[T], kind: &'static str, id: I, key: impl Fn(&T) -> I) -> Result<usize>
where
    I: PartialEq + Copy + Into<Uuid>,
{
    list.iter()
        .position(|x| key(x) == id)
        .ok_or(OpError::NotFound {
            kind,
            id: id.into(),
        })
}

/// Checks an insertion: `index` in range and `id` not yet used.
fn insertable<T, I>(
    list: &[T],
    kind: &'static str,
    index: usize,
    id: I,
    key: impl Fn(&T) -> I,
) -> Result<()>
where
    I: PartialEq + Copy + Into<Uuid>,
{
    if index > list.len() {
        return Err(OpError::Index {
            kind,
            index,
            len: list.len(),
        });
    }
    if list.iter().any(|x| key(x) == id) {
        return Err(OpError::DuplicateId {
            kind,
            id: id.into(),
        });
    }
    Ok(())
}

/// The pinned solver ids `list` would hold with its item `i` replaced by one pinned to `new`.
fn others<'a, T>(
    list: &'a [T],
    i: usize,
    pin: impl Fn(&T) -> Option<u32> + 'a,
    new: Option<u32>,
) -> impl Iterator<Item = Option<u32>> + 'a {
    list.iter()
        .enumerate()
        .filter(move |&(j, _)| j != i)
        .map(move |(_, x)| pin(x))
        .chain([new])
}

fn band_slot(values: &mut [F64], band: usize) -> Result<&mut F64> {
    let bands = values.len();
    values.get_mut(band).ok_or(OpError::Band { band, bands })
}

fn custom_shape(shape: &SpectrumShape) -> Option<&Vec<F64>> {
    match shape {
        SpectrumShape::Custom { relative_db } => Some(relative_db),
        SpectrumShape::Pink | SpectrumShape::White => None,
    }
}

fn custom_shape_mut(shape: &mut SpectrumShape) -> Option<&mut Vec<F64>> {
    match shape {
        SpectrumShape::Custom { relative_db } => Some(relative_db),
        SpectrumShape::Pink | SpectrumShape::White => None,
    }
}

fn noise_shape(r: &PointReceiver) -> Option<&Vec<F64>> {
    r.background_noise
        .as_ref()
        .and_then(|s| custom_shape(&s.shape))
}

fn noise_shape_mut(r: &mut PointReceiver) -> Option<&mut Vec<F64>> {
    r.background_noise
        .as_mut()
        .and_then(|s| custom_shape_mut(&mut s.shape))
}

/// Checks that `data` covers exactly the project's entities, in order, with `n` values each.
fn check_band_data(p: &Project, data: &BandData, n: usize) -> Result<()> {
    fn same_ids<A: PartialEq + fmt::Display, B>(
        what: &str,
        project: impl ExactSizeIterator<Item = A>,
        data: &[(A, B)],
    ) -> Result<()> {
        if project.len() != data.len() {
            return Err(OpError::BandData(format!(
                "{} {what} in the project, {} in the data",
                project.len(),
                data.len()
            )));
        }
        for (i, (a, (b, _))) in project.zip(data).enumerate() {
            if a != *b {
                return Err(OpError::BandData(format!(
                    "{what} {i} is {a} in the project but {b} in the data"
                )));
            }
        }
        Ok(())
    }
    let count = |what: String, found: usize| -> Result<()> {
        integrity::band_count(|| what, found, n).map_err(OpError::Integrity)
    };
    let presence = |what: String, project_has: bool, data_has: bool| -> Result<()> {
        if project_has == data_has {
            Ok(())
        } else {
            Err(OpError::BandData(format!(
                "{what}: custom shape in the project {project_has}, in the data {data_has}"
            )))
        }
    };

    same_ids(
        "materials",
        p.materials.iter().map(|m| m.id),
        &data.materials,
    )?;
    for (id, b) in &data.materials {
        count(format!("material {id} absorption"), b.absorption.len())?;
        count(format!("material {id} scattering"), b.scattering.len())?;
        if let Some(k) = b.reflection_law.band_count() {
            count(format!("material {id} reflection law"), k)?;
        }
        if let Some(t) = &b.transmission_loss_db {
            count(format!("material {id} transmission loss"), t.len())?;
        }
    }
    same_ids(
        "sources",
        p.sources.iter().map(|s| s.id),
        &data.source_shapes,
    )?;
    for (s, (id, shape)) in p.sources.iter().zip(&data.source_shapes) {
        presence(
            format!("source {id}"),
            custom_shape(&s.power.shape).is_some(),
            shape.is_some(),
        )?;
        if let Some(v) = shape {
            count(format!("source {id} power shape"), v.len())?;
        }
    }
    same_ids(
        "point receivers",
        p.point_receivers.iter().map(|r| r.id),
        &data.background_noise_shapes,
    )?;
    for (r, (id, shape)) in p.point_receivers.iter().zip(&data.background_noise_shapes) {
        presence(
            format!("receiver {id}"),
            noise_shape(r).is_some(),
            shape.is_some(),
        )?;
        if let Some(v) = shape {
            count(format!("receiver {id} noise shape"), v.len())?;
        }
    }
    same_ids(
        "fitting zones",
        p.fitting_zones.iter().map(|z| z.id),
        &data.fitting_zones,
    )?;
    for (id, b) in &data.fitting_zones {
        count(format!("fitting zone {id} absorption"), b.absorption.len())?;
        count(
            format!("fitting zone {id} mean free path"),
            b.mean_free_path_m.len(),
        )?;
        count(
            format!("fitting zone {id} diffusion law"),
            b.diffusion_law.len(),
        )?;
    }
    count("SPPS bands_computed".into(), data.spps_bands_computed.len())?;
    count("TCR bands_computed".into(), data.tcr_bands_computed.len())
}

/// Swaps the project's per-band arrays with `data` (already checked) and returns the old ones.
fn swap_band_data(p: &mut Project, mut data: BandData) -> BandData {
    use std::mem::swap;
    for (m, (_, b)) in p.materials.iter_mut().zip(&mut data.materials) {
        swap(&mut m.absorption, &mut b.absorption);
        swap(&mut m.scattering, &mut b.scattering);
        swap(&mut m.reflection_law, &mut b.reflection_law);
        swap(&mut m.transmission_loss_db, &mut b.transmission_loss_db);
    }
    for (s, (_, shape)) in p.sources.iter_mut().zip(&mut data.source_shapes) {
        if let (Some(mine), Some(theirs)) = (custom_shape_mut(&mut s.power.shape), shape.as_mut()) {
            swap(mine, theirs);
        }
    }
    for (r, (_, shape)) in p
        .point_receivers
        .iter_mut()
        .zip(&mut data.background_noise_shapes)
    {
        if let (Some(mine), Some(theirs)) = (noise_shape_mut(r), shape.as_mut()) {
            swap(mine, theirs);
        }
    }
    for (z, (_, b)) in p.fitting_zones.iter_mut().zip(&mut data.fitting_zones) {
        swap(&mut z.absorption, &mut b.absorption);
        swap(&mut z.mean_free_path_m, &mut b.mean_free_path_m);
        swap(&mut z.diffusion_law, &mut b.diffusion_law);
    }
    swap(
        &mut p.solvers.spps.bands_computed,
        &mut data.spps_bands_computed,
    );
    swap(
        &mut p.solvers.tcr.bands_computed,
        &mut data.tcr_bands_computed,
    );
    data
}

impl Project {
    /// A copy of every per-band array, as [`Op::SetBands`] takes them.
    pub fn band_data(&self) -> BandData {
        BandData {
            materials: self
                .materials
                .iter()
                .map(|m| {
                    (
                        m.id,
                        MaterialBands {
                            absorption: m.absorption.clone(),
                            scattering: m.scattering.clone(),
                            reflection_law: m.reflection_law.clone(),
                            transmission_loss_db: m.transmission_loss_db.clone(),
                        },
                    )
                })
                .collect(),
            source_shapes: self
                .sources
                .iter()
                .map(|s| (s.id, custom_shape(&s.power.shape).cloned()))
                .collect(),
            background_noise_shapes: self
                .point_receivers
                .iter()
                .map(|r| (r.id, noise_shape(r).cloned()))
                .collect(),
            fitting_zones: self
                .fitting_zones
                .iter()
                .map(|z| {
                    (
                        z.id,
                        FittingBands {
                            absorption: z.absorption.clone(),
                            mean_free_path_m: z.mean_free_path_m.clone(),
                            diffusion_law: z.diffusion_law.clone(),
                        },
                    )
                })
                .collect(),
            spps_bands_computed: self.solvers.spps.bands_computed.clone(),
            tcr_bands_computed: self.solvers.tcr.bands_computed.clone(),
        }
    }

    /// The [`Op::SetBands`] that moves the project onto `bands`. Each new band takes every
    /// per-band value of the nearest current band on a log-frequency scale (ties go to the lower
    /// band): an octave band becomes three equal third-octave bands, and a third-octave set
    /// becomes octaves by keeping each octave's centre third. `None` if either band set is not
    /// usable.
    pub fn rebanded(&self, bands: BandSet) -> Option<Op> {
        let old = self.bands.band_numbers()?;
        let new = bands.band_numbers()?;
        if old.is_empty() || new.is_empty() {
            return None;
        }
        let pick: Vec<usize> = new
            .iter()
            .map(|&k| {
                (0..old.len())
                    .min_by_key(|&i| (old[i] - k).unsigned_abs())
                    .unwrap_or(0)
            })
            .collect();
        fn map<T: Clone>(pick: &[usize], v: &[T]) -> Vec<T> {
            pick.iter().map(|&i| v[i].clone()).collect()
        }
        let mut data = self.band_data();
        for (_, b) in &mut data.materials {
            b.absorption = map(&pick, &b.absorption);
            b.scattering = map(&pick, &b.scattering);
            if let ReflectionLaws::PerBand(laws) = &b.reflection_law {
                b.reflection_law = ReflectionLaws::from_bands(map(&pick, laws));
            }
            if let Some(t) = &mut b.transmission_loss_db {
                *t = map(&pick, t);
            }
        }
        let sources = data.source_shapes.iter_mut().map(|(_, s)| s);
        let noises = data.background_noise_shapes.iter_mut().map(|(_, s)| s);
        for v in sources.chain(noises).flatten() {
            *v = map(&pick, v);
        }
        for (_, b) in &mut data.fitting_zones {
            b.absorption = map(&pick, &b.absorption);
            b.mean_free_path_m = map(&pick, &b.mean_free_path_m);
            b.diffusion_law = map(&pick, &b.diffusion_law);
        }
        data.spps_bands_computed = map(&pick, &data.spps_bands_computed);
        data.tcr_bands_computed = map(&pick, &data.tcr_bands_computed);
        Some(Op::SetBands { bands, data })
    }
}

impl Op {
    /// The op as compact JSON text, for IPC. Floats are written as in a project file (see the
    /// `schema` module docs), so [`Op::from_json`] reads back the same bits.
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).expect("an Op serialises without error: no map keys")
    }

    /// Reads an op from JSON text with the exact reader ([`from_json_exact`]). This is the only
    /// safe way to take an op from the UI: `serde_json::from_str` (and so a Tauri command
    /// argument typed `Op`) misreads some floats in this build.
    pub fn from_json(text: &str) -> std::result::Result<Op, LoadError> {
        from_json_exact(text)
    }

    /// Applies the op. On success the project has changed and the returned op undoes it
    /// exactly: applying the inverse gives back a project `==` to the one before (bit for bit,
    /// order included). On error nothing has changed.
    ///
    /// Every op keeps [`Project::check_integrity`]: applied to a project that passes it, the
    /// result passes it too.
    pub fn apply(self, p: &mut Project) -> Result<Op> {
        use std::mem::replace;
        let n = p.bands.len();
        match self {
            Op::SetProjectName { name } => Ok(Op::SetProjectName {
                name: replace(&mut p.name, name),
            }),
            Op::SetDescription { description } => Ok(Op::SetDescription {
                description: replace(&mut p.description, description),
            }),
            Op::Rename { target, name } => {
                let slot = match target {
                    EntityRef::SurfaceGroup(id) => {
                        let i = position(&p.surface_groups, "surface group", id, |g| g.id)?;
                        &mut p.surface_groups[i].name
                    }
                    EntityRef::Material(id) => {
                        let i = position(&p.materials, "material", id, |m| m.id)?;
                        &mut p.materials[i].name
                    }
                    EntityRef::Source(id) => {
                        let i = position(&p.sources, "source", id, |s| s.id)?;
                        &mut p.sources[i].name
                    }
                    EntityRef::PointReceiver(id) => {
                        let i = position(&p.point_receivers, "point receiver", id, |r| r.id)?;
                        &mut p.point_receivers[i].name
                    }
                    EntityRef::SurfaceReceiver(id) => {
                        let i = position(&p.surface_receivers, "surface receiver", id, |r| r.id)?;
                        &mut p.surface_receivers[i].name
                    }
                    EntityRef::FittingZone(id) => {
                        let i = position(&p.fitting_zones, "fitting zone", id, |z| z.id)?;
                        &mut p.fitting_zones[i].name
                    }
                    EntityRef::Variant(id) => {
                        let i = position(&p.variants, "variant", id, |v| v.id)?;
                        &mut p.variants[i].name
                    }
                };
                Ok(Op::Rename {
                    target,
                    name: replace(slot, name),
                })
            }
            Op::SetBands { bands, data } => {
                integrity::check_bands(&bands)?;
                check_band_data(p, &data, bands.len())?;
                let data = swap_band_data(p, data);
                Ok(Op::SetBands {
                    bands: replace(&mut p.bands, bands),
                    data,
                })
            }
            Op::SetGeometry { geometry } => {
                let exists = |g: GroupId| p.group(g).is_some();
                for (i, f) in geometry.faces.iter().enumerate() {
                    integrity::check_face(i, f, geometry.vertices.len(), &exists)?;
                }
                Ok(Op::SetGeometry {
                    geometry: replace(&mut p.geometry, geometry),
                })
            }
            Op::AddSurfaceGroup { index, group } => {
                insertable(&p.surface_groups, "surface group", index, group.id, |g| {
                    g.id
                })?;
                integrity::check_group(&group, &|m| p.material(m).is_some())?;
                let id = group.id;
                p.surface_groups.insert(index, group);
                Ok(Op::RemoveSurfaceGroup { id })
            }
            Op::RemoveSurfaceGroup { id } => {
                let index = position(&p.surface_groups, "surface group", id, |g| g.id)?;
                let in_use = |by: String| OpError::InUse {
                    kind: "surface group",
                    id: id.0,
                    by,
                };
                if let Some(i) = p.geometry.faces.iter().position(|f| f.group == id) {
                    return Err(in_use(format!("face {i}")));
                }
                if let Some(v) = p.variants.iter().find(|v| v.override_for(id).is_some()) {
                    return Err(in_use(format!("variant '{}'", v.name)));
                }
                if let Some(r) = p.surface_receivers.iter().find(|r| {
                    matches!(&r.shape, SurfaceReceiverShape::Scene { groups } if groups.contains(&id))
                }) {
                    return Err(in_use(format!("surface receiver '{}'", r.name)));
                }
                if let Some(z) = p.fitting_zones.iter().find(|z| {
                    matches!(&z.shape, FittingShape::Surfaces { groups, .. } if groups.contains(&id))
                }) {
                    return Err(in_use(format!("fitting zone '{}'", z.name)));
                }
                Ok(Op::AddSurfaceGroup {
                    index,
                    group: p.surface_groups.remove(index),
                })
            }
            Op::SetGroupMaterial { group, material } => {
                let i = position(&p.surface_groups, "surface group", group, |g| g.id)?;
                position(&p.materials, "material", material, |m| m.id)?;
                Ok(Op::SetGroupMaterial {
                    group,
                    material: replace(&mut p.surface_groups[i].material, material),
                })
            }
            Op::AddMaterial { index, material } => {
                insertable(&p.materials, "material", index, material.id, |m| m.id)?;
                integrity::check_material(&material, n)?;
                integrity::check_solver_ids(p.materials.iter().chain([&material]))?;
                let id = material.id;
                p.materials.insert(index, material);
                Ok(Op::RemoveMaterial { id })
            }
            Op::RemoveMaterial { id } => {
                let index = position(&p.materials, "material", id, |m| m.id)?;
                let in_use = |by: String| OpError::InUse {
                    kind: "material",
                    id: id.0,
                    by,
                };
                if let Some(g) = p.surface_groups.iter().find(|g| g.material == id) {
                    return Err(in_use(format!("surface group '{}'", g.name)));
                }
                if let Some(v) = p
                    .variants
                    .iter()
                    .find(|v| v.overrides.iter().any(|o| o.material == id))
                {
                    return Err(in_use(format!("variant '{}'", v.name)));
                }
                Ok(Op::AddMaterial {
                    index,
                    material: p.materials.remove(index),
                })
            }
            Op::ReplaceMaterial { material } => {
                let i = position(&p.materials, "material", material.id, |m| m.id)?;
                integrity::check_material(&material, n)?;
                integrity::check_solver_ids(
                    p.materials
                        .iter()
                        .enumerate()
                        .filter(|&(j, _)| j != i)
                        .map(|(_, m)| m)
                        .chain([&material]),
                )?;
                Ok(Op::ReplaceMaterial {
                    material: replace(&mut p.materials[i], material),
                })
            }
            Op::SetMaterialBand {
                material,
                quantity,
                band,
                value,
            } => {
                let i = position(&p.materials, "material", material, |m| m.id)?;
                let m = &mut p.materials[i];
                let no_transmission = OpError::NoTransmission {
                    material: material.0,
                };
                let slot = match quantity {
                    MaterialQuantity::Absorption => band_slot(&mut m.absorption, band)?,
                    MaterialQuantity::Scattering => band_slot(&mut m.scattering, band)?,
                    MaterialQuantity::TransmissionLoss => {
                        let losses = m.transmission_loss_db.as_mut().ok_or(no_transmission)?;
                        let bands = losses.len();
                        losses
                            .get_mut(band)
                            .ok_or(OpError::Band { band, bands })?
                            .as_mut()
                            .ok_or(OpError::NoTransmission {
                                material: material.0,
                            })?
                    }
                };
                Ok(Op::SetMaterialBand {
                    material,
                    quantity,
                    band,
                    value: replace(slot, value),
                })
            }
            Op::AddSource { index, source } => {
                insertable(&p.sources, "source", index, source.id, |s| s.id)?;
                integrity::check_source(&source, n)?;
                integrity::unique_pins(
                    "source",
                    p.sources
                        .iter()
                        .map(|s| s.solver_id)
                        .chain([source.solver_id]),
                )?;
                let id = source.id;
                p.sources.insert(index, source);
                Ok(Op::RemoveSource { id })
            }
            Op::RemoveSource { id } => {
                let index = position(&p.sources, "source", id, |s| s.id)?;
                Ok(Op::AddSource {
                    index,
                    source: p.sources.remove(index),
                })
            }
            Op::ReplaceSource { source } => {
                let i = position(&p.sources, "source", source.id, |s| s.id)?;
                integrity::check_source(&source, n)?;
                integrity::unique_pins(
                    "source",
                    others(&p.sources, i, |s| s.solver_id, source.solver_id),
                )?;
                Ok(Op::ReplaceSource {
                    source: replace(&mut p.sources[i], source),
                })
            }
            Op::MoveSource { id, position: to } => {
                let i = position(&p.sources, "source", id, |s| s.id)?;
                Ok(Op::MoveSource {
                    id,
                    position: replace(&mut p.sources[i].position, to),
                })
            }
            Op::SetSourceEnabled { id, enabled } => {
                let i = position(&p.sources, "source", id, |s| s.id)?;
                Ok(Op::SetSourceEnabled {
                    id,
                    enabled: replace(&mut p.sources[i].enabled, enabled),
                })
            }
            Op::AddPointReceiver { index, receiver } => {
                insertable(
                    &p.point_receivers,
                    "point receiver",
                    index,
                    receiver.id,
                    |r| r.id,
                )?;
                integrity::check_point_receiver(&receiver, n)?;
                integrity::unique_pins(
                    "point receiver",
                    p.point_receivers
                        .iter()
                        .map(|r| r.solver_id)
                        .chain([receiver.solver_id]),
                )?;
                let id = receiver.id;
                p.point_receivers.insert(index, receiver);
                Ok(Op::RemovePointReceiver { id })
            }
            Op::RemovePointReceiver { id } => {
                let index = position(&p.point_receivers, "point receiver", id, |r| r.id)?;
                Ok(Op::AddPointReceiver {
                    index,
                    receiver: p.point_receivers.remove(index),
                })
            }
            Op::ReplacePointReceiver { receiver } => {
                let i = position(&p.point_receivers, "point receiver", receiver.id, |r| r.id)?;
                integrity::check_point_receiver(&receiver, n)?;
                integrity::unique_pins(
                    "point receiver",
                    others(&p.point_receivers, i, |r| r.solver_id, receiver.solver_id),
                )?;
                Ok(Op::ReplacePointReceiver {
                    receiver: replace(&mut p.point_receivers[i], receiver),
                })
            }
            Op::MovePointReceiver { id, position: to } => {
                let i = position(&p.point_receivers, "point receiver", id, |r| r.id)?;
                Ok(Op::MovePointReceiver {
                    id,
                    position: replace(&mut p.point_receivers[i].position, to),
                })
            }
            Op::AddSurfaceReceiver { index, receiver } => {
                insertable(
                    &p.surface_receivers,
                    "surface receiver",
                    index,
                    receiver.id,
                    |r| r.id,
                )?;
                integrity::check_surface_receiver(&receiver, &|g| p.group(g).is_some())?;
                integrity::unique_pins(
                    "surface receiver",
                    p.surface_receivers
                        .iter()
                        .map(|r| r.solver_id)
                        .chain([receiver.solver_id]),
                )?;
                let id = receiver.id;
                p.surface_receivers.insert(index, receiver);
                Ok(Op::RemoveSurfaceReceiver { id })
            }
            Op::RemoveSurfaceReceiver { id } => {
                let index = position(&p.surface_receivers, "surface receiver", id, |r| r.id)?;
                Ok(Op::AddSurfaceReceiver {
                    index,
                    receiver: p.surface_receivers.remove(index),
                })
            }
            Op::ReplaceSurfaceReceiver { receiver } => {
                let i = position(&p.surface_receivers, "surface receiver", receiver.id, |r| {
                    r.id
                })?;
                integrity::check_surface_receiver(&receiver, &|g| p.group(g).is_some())?;
                integrity::unique_pins(
                    "surface receiver",
                    others(&p.surface_receivers, i, |r| r.solver_id, receiver.solver_id),
                )?;
                Ok(Op::ReplaceSurfaceReceiver {
                    receiver: replace(&mut p.surface_receivers[i], receiver),
                })
            }
            Op::AddFittingZone { index, zone } => {
                insertable(&p.fitting_zones, "fitting zone", index, zone.id, |z| z.id)?;
                integrity::check_fitting_zone(&zone, n, &|g| p.group(g).is_some())?;
                integrity::unique_pins(
                    "fitting zone",
                    p.fitting_zones
                        .iter()
                        .map(|z| z.solver_id)
                        .chain([zone.solver_id]),
                )?;
                let id = zone.id;
                p.fitting_zones.insert(index, zone);
                Ok(Op::RemoveFittingZone { id })
            }
            Op::RemoveFittingZone { id } => {
                let index = position(&p.fitting_zones, "fitting zone", id, |z| z.id)?;
                Ok(Op::AddFittingZone {
                    index,
                    zone: p.fitting_zones.remove(index),
                })
            }
            Op::ReplaceFittingZone { zone } => {
                let i = position(&p.fitting_zones, "fitting zone", zone.id, |z| z.id)?;
                integrity::check_fitting_zone(&zone, n, &|g| p.group(g).is_some())?;
                integrity::unique_pins(
                    "fitting zone",
                    others(&p.fitting_zones, i, |z| z.solver_id, zone.solver_id),
                )?;
                Ok(Op::ReplaceFittingZone {
                    zone: replace(&mut p.fitting_zones[i], zone),
                })
            }
            Op::SetFittingBand {
                zone,
                quantity,
                band,
                value,
            } => {
                let i = position(&p.fitting_zones, "fitting zone", zone, |z| z.id)?;
                let z = &mut p.fitting_zones[i];
                let values = match quantity {
                    FittingQuantity::Absorption => &mut z.absorption,
                    FittingQuantity::MeanFreePath => &mut z.mean_free_path_m,
                };
                let slot = band_slot(values, band)?;
                Ok(Op::SetFittingBand {
                    zone,
                    quantity,
                    band,
                    value: replace(slot, value),
                })
            }
            Op::SetEnvironment { environment } => Ok(Op::SetEnvironment {
                environment: replace(&mut p.environment, environment),
            }),
            Op::SetSolverSettings { settings } => {
                integrity::check_solver_settings(&settings, n)?;
                Ok(Op::SetSolverSettings {
                    settings: replace(&mut p.solvers, settings),
                })
            }
            Op::SetBandComputed {
                solver,
                band,
                computed,
            } => {
                let flags = match solver {
                    SolverKind::Spps => &mut p.solvers.spps.bands_computed,
                    SolverKind::Tcr => &mut p.solvers.tcr.bands_computed,
                };
                let bands = flags.len();
                let slot = flags.get_mut(band).ok_or(OpError::Band { band, bands })?;
                Ok(Op::SetBandComputed {
                    solver,
                    band,
                    computed: replace(slot, computed),
                })
            }
            Op::AddVariant { index, variant } => {
                insertable(&p.variants, "variant", index, variant.id, |v| v.id)?;
                integrity::check_variant(&variant, &|g| p.group(g).is_some(), &|m| {
                    p.material(m).is_some()
                })?;
                let id = variant.id;
                p.variants.insert(index, variant);
                Ok(Op::RemoveVariant { id })
            }
            Op::RemoveVariant { id } => {
                let index = position(&p.variants, "variant", id, |v| v.id)?;
                if p.active_variant == Some(id) {
                    return Err(OpError::ActiveVariant { variant: id.0 });
                }
                Ok(Op::AddVariant {
                    index,
                    variant: p.variants.remove(index),
                })
            }
            Op::SetVariantOverride {
                variant,
                group,
                material,
            } => {
                let i = position(&p.variants, "variant", variant, |v| v.id)?;
                position(&p.surface_groups, "surface group", group, |g| g.id)?;
                if let Some(m) = material {
                    position(&p.materials, "material", m, |x| x.id)?;
                }
                Ok(Op::SetVariantOverride {
                    variant,
                    group,
                    material: p.variants[i].set_override(group, material),
                })
            }
            Op::SetActiveVariant { variant } => {
                if let Some(v) = variant {
                    position(&p.variants, "variant", v, |x| x.id)?;
                }
                Ok(Op::SetActiveVariant {
                    variant: replace(&mut p.active_variant, variant),
                })
            }
            Op::SetCamera { camera } => Ok(Op::SetCamera {
                camera: replace(&mut p.view.camera, camera),
            }),
            Op::Batch { ops } => {
                let mut inverses = Vec::with_capacity(ops.len());
                for (index, op) in ops.into_iter().enumerate() {
                    match op.apply(p) {
                        Ok(inverse) => inverses.push(inverse),
                        Err(error) => {
                            for inverse in inverses.into_iter().rev() {
                                inverse
                                    .apply(p)
                                    .expect("the inverse of an op just applied applies");
                            }
                            return Err(OpError::Batch {
                                index,
                                error: Box::new(error),
                            });
                        }
                    }
                }
                inverses.reverse();
                Ok(Op::Batch { ops: inverses })
            }
        }
    }
}

/// Undo and redo stacks of inverse ops.
#[derive(Clone, Debug, Default)]
pub struct History {
    undo: Vec<Op>,
    redo: Vec<Op>,
}

impl History {
    pub fn new() -> Self {
        History::default()
    }

    /// Applies `op` and records its inverse for undo. Clears the redo stack. On error nothing
    /// changes, the history included.
    pub fn apply(&mut self, project: &mut Project, op: Op) -> Result<()> {
        let inverse = op.apply(project)?;
        self.undo.push(inverse);
        self.redo.clear();
        Ok(())
    }

    /// Undoes the last op. `Ok(false)` when there is nothing to undo.
    pub fn undo(&mut self, project: &mut Project) -> Result<bool> {
        let Some(op) = self.undo.pop() else {
            return Ok(false);
        };
        match op.clone().apply(project) {
            Ok(redo) => {
                self.redo.push(redo);
                Ok(true)
            }
            Err(e) => {
                self.undo.push(op);
                Err(e)
            }
        }
    }

    /// Re-applies the last undone op. `Ok(false)` when there is nothing to redo.
    pub fn redo(&mut self, project: &mut Project) -> Result<bool> {
        let Some(op) = self.redo.pop() else {
            return Ok(false);
        };
        match op.clone().apply(project) {
            Ok(undo) => {
                self.undo.push(undo);
                Ok(true)
            }
            Err(e) => {
                self.redo.push(op);
                Err(e)
            }
        }
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    pub fn undo_len(&self) -> usize {
        self.undo.len()
    }

    pub fn redo_len(&self) -> usize {
        self.redo.len()
    }

    pub fn clear(&mut self) {
        self.undo.clear();
        self.redo.clear();
    }
}
