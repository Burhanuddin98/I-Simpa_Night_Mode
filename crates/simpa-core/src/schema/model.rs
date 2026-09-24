//! The project types. Each field names the config.xml attribute it becomes, where it becomes one.

use std::borrow::Cow;
use std::fmt;

use schemars::{JsonSchema, Schema, SchemaGenerator, json_schema};
use serde::de::{self, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::bands::{BandSet, Spectrum};
use super::ids::{
    FittingZoneId, GroupId, MaterialId, PointReceiverId, ProjectId, SourceId, SurfaceReceiverId,
    VariantId,
};
use super::real::{F64, Vec3};

/// The file format version this build writes and reads.
pub const FORMAT_VERSION: u32 = 1;

/// The project file extension, without the dot.
pub const FILE_EXTENSION: &str = "simpa";

/// The largest value of an integer the solvers read from config.xml: `CoreString::ToInt` is
/// `atoi` into a C `int` (`lib_interface/coreString.cpp:84-87`), so anything above `INT_MAX`
/// cannot be expressed. The model keeps these integers as `u32` (none may be negative) and
/// [`Project::check_integrity`] refuses a value above this bound.
pub const SOLVER_INT_MAX: u32 = i32::MAX as u32;

/// Deserialises an `Option` field that must be present in the file (as `null` when empty).
/// Serde would otherwise read a missing `Option` key as `None`, silently.
pub(crate) fn required<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(deserializer)
}

/// The JSON Schema of an `Option<T>` field read with [`required`]: `T` or `null`, and the key
/// required. Used as `#[schemars(with = "Nullable<T>")]`. A plain `Option<T>` would make the key
/// optional, and `#[schemars(required)]` would drop the `null`.
pub(crate) struct Nullable<T>(std::marker::PhantomData<T>);

impl<T: JsonSchema> JsonSchema for Nullable<T> {
    fn inline_schema() -> bool {
        true
    }

    fn schema_name() -> Cow<'static, str> {
        format!("Nullable_{}", T::schema_name()).into()
    }

    fn json_schema(generator: &mut SchemaGenerator) -> Schema {
        Option::<T>::json_schema(generator)
    }
}

/// A room-acoustics project: the model, its materials, sources and receivers, the solver settings
/// and the GUI state.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Project {
    /// Always [`FORMAT_VERSION`]; checked first on load.
    pub format_version: u32,
    pub id: ProjectId,
    pub name: String,
    pub description: String,
    pub frame: Frame,
    /// The one band set every per-band array follows (`freq_enum`).
    pub bands: BandSet,
    pub geometry: Geometry,
    pub surface_groups: Vec<SurfaceGroup>,
    /// The project's material library. Entries need not be in use.
    pub materials: Vec<Material>,
    pub sources: Vec<Source>,
    pub point_receivers: Vec<PointReceiver>,
    pub surface_receivers: Vec<SurfaceReceiver>,
    pub fitting_zones: Vec<FittingZone>,
    pub environment: Environment,
    pub solvers: SolverSettings,
    pub variants: Vec<Variant>,
    /// The variant the GUI shows and a run uses; `None` is the base project.
    #[serde(deserialize_with = "required")]
    #[schemars(with = "Nullable<VariantId>")]
    pub active_variant: Option<VariantId>,
    pub view: ViewState,
}

impl Project {
    /// An empty project with default bands, environment and solver settings and a random id.
    pub fn new(name: impl Into<String>) -> Self {
        let bands = BandSet::default();
        let n = bands.len();
        Project {
            format_version: FORMAT_VERSION,
            id: ProjectId::random(),
            name: name.into(),
            description: String::new(),
            frame: Frame::default(),
            bands,
            geometry: Geometry::default(),
            surface_groups: Vec::new(),
            materials: Vec::new(),
            sources: Vec::new(),
            point_receivers: Vec::new(),
            surface_receivers: Vec::new(),
            fitting_zones: Vec::new(),
            environment: Environment::default(),
            solvers: SolverSettings::for_bands(n),
            variants: Vec::new(),
            active_variant: None,
            view: ViewState::default(),
        }
    }

    pub fn group(&self, id: GroupId) -> Option<&SurfaceGroup> {
        self.surface_groups.iter().find(|g| g.id == id)
    }

    pub fn material(&self, id: MaterialId) -> Option<&Material> {
        self.materials.iter().find(|m| m.id == id)
    }

    pub fn source(&self, id: SourceId) -> Option<&Source> {
        self.sources.iter().find(|s| s.id == id)
    }

    pub fn point_receiver(&self, id: PointReceiverId) -> Option<&PointReceiver> {
        self.point_receivers.iter().find(|r| r.id == id)
    }

    pub fn surface_receiver(&self, id: SurfaceReceiverId) -> Option<&SurfaceReceiver> {
        self.surface_receivers.iter().find(|r| r.id == id)
    }

    pub fn fitting_zone(&self, id: FittingZoneId) -> Option<&FittingZone> {
        self.fitting_zones.iter().find(|z| z.id == id)
    }

    pub fn variant(&self, id: VariantId) -> Option<&Variant> {
        self.variants.iter().find(|v| v.id == id)
    }

    /// The material a surface group carries under `variant` (`None` = the base project): the
    /// variant's override if it has one, the group's own material otherwise. `None` if the
    /// group or the variant does not exist.
    pub fn effective_material(
        &self,
        group: GroupId,
        variant: Option<VariantId>,
    ) -> Option<MaterialId> {
        let base = self.group(group)?.material;
        match variant {
            None => Some(base),
            Some(v) => Some(self.variant(v)?.override_for(group).unwrap_or(base)),
        }
    }

    /// [`Project::effective_material`] under the active variant.
    pub fn active_material(&self, group: GroupId) -> Option<MaterialId> {
        self.effective_material(group, self.active_variant)
    }
}

/// Length unit of every coordinate in the project. Only metres: the file states it so that a
/// file in any other unit is refused rather than misread.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum LengthUnit {
    #[default]
    Metre,
}

/// The vertical axis. Only +Z, as the solvers use it (`@alog`/`@blin` gradients act along z).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum UpAxis {
    #[default]
    Z,
}

/// The world frame.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Frame {
    pub length_unit: LengthUnit,
    pub up_axis: UpAxis,
}

/// The room's surface mesh, in world metres. Each face belongs to one surface group.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Geometry {
    pub vertices: Vec<Vec3>,
    pub faces: Vec<Face>,
}

/// A triangle: three vertex indices and the surface group it belongs to.
/// In JSON: `[a, b, c, "group-id"]`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Face {
    pub vertices: [u32; 3],
    pub group: GroupId,
}

impl Serialize for Face {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let [a, b, c] = self.vertices;
        (a, b, c, self.group).serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Face {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let (a, b, c, group) = <(u32, u32, u32, GroupId)>::deserialize(deserializer)?;
        Ok(Face {
            vertices: [a, b, c],
            group,
        })
    }
}

impl JsonSchema for Face {
    fn inline_schema() -> bool {
        true
    }

    fn schema_name() -> Cow<'static, str> {
        "Face".into()
    }

    fn json_schema(generator: &mut SchemaGenerator) -> Schema {
        json_schema!({
            "description": "[a, b, c, group]: three vertex indices and a surface group id",
            "type": "array",
            "prefixItems": [
                { "type": "integer", "minimum": 0 },
                { "type": "integer", "minimum": 0 },
                { "type": "integer", "minimum": 0 },
                generator.subschema_for::<GroupId>()
            ],
            "items": false,
            "minItems": 4
        })
    }
}

/// A named set of faces sharing one material: the unit materials are assigned to.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SurfaceGroup {
    pub id: GroupId,
    pub name: String,
    /// The base project's material; a [`Variant`] may override it.
    pub material: MaterialId,
}

/// A colour, for display only. In JSON: `"#rrggbb"`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Rgb(pub u8, pub u8, pub u8);

impl fmt::Display for Rgb {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "#{:02x}{:02x}{:02x}", self.0, self.1, self.2)
    }
}

impl Serialize for Rgb {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

struct RgbVisitor;

impl Visitor<'_> for RgbVisitor {
    type Value = Rgb;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("a colour \"#rrggbb\"")
    }

    fn visit_str<E: de::Error>(self, v: &str) -> Result<Rgb, E> {
        let bad = || E::invalid_value(de::Unexpected::Str(v), &self);
        let hex = v.strip_prefix('#').ok_or_else(bad)?;
        if hex.len() != 6 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(bad());
        }
        let byte = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16).map_err(|_| bad());
        Ok(Rgb(byte(0)?, byte(2)?, byte(4)?))
    }
}

impl<'de> Deserialize<'de> for Rgb {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_str(RgbVisitor)
    }
}

impl JsonSchema for Rgb {
    fn inline_schema() -> bool {
        true
    }

    fn schema_name() -> Cow<'static, str> {
        "Rgb".into()
    }

    fn json_schema(_: &mut SchemaGenerator) -> Schema {
        json_schema!({ "type": "string", "pattern": "^#[0-9a-fA-F]{6}$" })
    }
}

/// How a surface reflects the diffuse part of the energy: `type_surface/bfreq@loi`, the solver's
/// `REFLECTION_LAW` (`lib_interface/coreTypes.h:83-92`). The discriminant is the solver code.
///
/// SPPS draws a diffuse reflection with probability equal to the scattering coefficient
/// (`spps/CalculationCore.cpp:318`), so the law has no effect at scattering 0.
/// [`ReflectionLaw::SemiDiffuse`] has no case in SPPS's `ReflectionLaws::SolveReflection`
/// (`spps/tools/dotreflection.h:23-45`) and falls to its default, a specular reflection.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[repr(u8)]
pub enum ReflectionLaw {
    #[default]
    Specular = 0,
    /// Uniform, `w1` in upstream's comments.
    Uniform = 1,
    Lambert = 2,
    W2 = 3,
    W3 = 4,
    W4 = 5,
    SemiDiffuse = 6,
}

impl ReflectionLaw {
    pub const ALL: [ReflectionLaw; 7] = [
        ReflectionLaw::Specular,
        ReflectionLaw::Uniform,
        ReflectionLaw::Lambert,
        ReflectionLaw::W2,
        ReflectionLaw::W3,
        ReflectionLaw::W4,
        ReflectionLaw::SemiDiffuse,
    ];

    /// The value config.xml writes as `@loi`.
    pub const fn solver_code(self) -> u8 {
        self as u8
    }

    pub fn from_solver_code(code: u8) -> Option<Self> {
        Self::ALL.get(usize::from(code)).copied()
    }
}

/// A material's reflection law: one law for every band, or one per band. The solvers read it per
/// band (`type_surface/bfreq@loi`, `base_core_configuration.cpp:210-219`), and upstream's GUI
/// keeps one per band (the `loi` list of each row, `e_data_row_materiau.h:218`): tutorial 3's
/// material 100 is Lambert in the six octave bands and specular in the others.
///
/// In JSON: one law, `"lambert"`, or one per band in the band set's order,
/// `["specular", "lambert", ...]`. [`ReflectionLaws::from_bands`] gives the single spelling
/// whenever every band has the same law.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum ReflectionLaws {
    /// The same law in every band.
    All(ReflectionLaw),
    /// One law per band; its length is the band count (`Project::check_integrity`).
    PerBand(Vec<ReflectionLaw>),
}

impl Default for ReflectionLaws {
    fn default() -> Self {
        ReflectionLaws::All(ReflectionLaw::default())
    }
}

impl From<ReflectionLaw> for ReflectionLaws {
    fn from(law: ReflectionLaw) -> Self {
        ReflectionLaws::All(law)
    }
}

impl ReflectionLaws {
    /// One law per band: [`ReflectionLaws::All`] when every band has the same law (and there is
    /// at least one band), [`ReflectionLaws::PerBand`] otherwise.
    pub fn from_bands(laws: Vec<ReflectionLaw>) -> Self {
        match laws.first() {
            Some(&first) if laws.iter().all(|&l| l == first) => ReflectionLaws::All(first),
            _ => ReflectionLaws::PerBand(laws),
        }
    }

    /// The law of band `band`: `None` only past the end of a per-band list.
    pub fn at(&self, band: usize) -> Option<ReflectionLaw> {
        match self {
            ReflectionLaws::All(law) => Some(*law),
            ReflectionLaws::PerBand(laws) => laws.get(band).copied(),
        }
    }

    /// The per-band list's length; `None` for one law, which fits every band set.
    pub fn band_count(&self) -> Option<usize> {
        match self {
            ReflectionLaws::All(_) => None,
            ReflectionLaws::PerBand(laws) => Some(laws.len()),
        }
    }

    /// The first band (0 for one law) whose law is `law`, if any.
    pub fn first_band_with(&self, law: ReflectionLaw) -> Option<usize> {
        match self {
            ReflectionLaws::All(l) => (*l == law).then_some(0),
            ReflectionLaws::PerBand(laws) => laws.iter().position(|&l| l == law),
        }
    }
}

/// A surface material: `surface_absorption_enum/type_surface`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Material {
    pub id: MaterialId,
    pub name: String,
    /// Display colour.
    pub color: Rgb,
    /// Absorption coefficient per band, 0 to 1 (`bfreq@absorb`).
    pub absorption: Vec<F64>,
    /// Scattering coefficient per band, 0 to 1: the share of reflections that are diffuse
    /// (`bfreq@diffusion`).
    pub scattering: Vec<F64>,
    /// `bfreq@loi`, one law for every band or one per band.
    pub reflection_law: ReflectionLaws,
    /// Transmission loss per band in dB (`bfreq@affaiblissement`, tau = 10^(-R/10)). `None`
    /// means no transmission in any band; a `None` band does not transmit. Either way the
    /// attribute is left out of that band, which is how the solver knows
    /// (`base_core_configuration.cpp:210-219`), as upstream's GUI leaves it out of a band whose
    /// `transmission` switch is off (`e_data_row_materiau.h:98-106`): tutorial 3's
    /// `Open_door` transmits from 250 Hz to 4 kHz but not at 125 Hz.
    #[serde(deserialize_with = "required")]
    #[schemars(with = "Nullable<Vec<Option<F64>>>")]
    pub transmission_loss_db: Option<Vec<Option<F64>>>,
    /// `@side_material`: the material acts on both sides of a face.
    pub double_sided: bool,
    /// A pinned solver id (`type_surface@id`, the `.cbin` face `idMat`). `None` lets export
    /// assign one. Importers set it so that a project imported from a solver mesh exports the
    /// same ids and still matches that mesh. At most [`SOLVER_INT_MAX`]: the solver reads
    /// `@id` with `atoi`.
    #[serde(deserialize_with = "required")]
    #[schemars(with = "Nullable<u32>", range(max = 2_147_483_647))]
    pub solver_id: Option<u32>,
}

/// Source directivity: `source@directivite`, the solver's `SOURCE_TYPE`
/// (`lib_interface/coreTypes.h:96-104`). See [`Directivity::solver_code`].
///
/// In JSON: `{"kind": "omni"}`, `{"kind": "unidirectional", "direction": [x, y, z]}`, ...
/// Extra keys are refused for every kind. (serde's derive alone would read
/// `{"kind": "omni", "file": "balloon.txt"}` as omni and drop the file, so the reader is written
/// by hand.)
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Directivity {
    /// Code 0.
    #[default]
    Omni,
    /// Code 1: emits along `direction` (`@u`, `@v`, `@w`, normalised by the solver).
    Unidirectional { direction: Vec3 },
    /// Code 2: emits in the XY plane.
    PlaneXy,
    /// Code 3: emits in the YZ plane.
    PlaneYz,
    /// Code 4: emits in the XZ plane.
    PlaneXz,
    /// Code 5: a directivity balloon file (`@directivity_file`), aimed along `direction`.
    /// `file` is relative to the project file. A missing file makes the source emit nothing,
    /// with exit code 0 (contract survey, `run_dirmiss`), so it is checked before launch.
    Balloon { file: String, direction: Vec3 },
}

impl Directivity {
    /// The value config.xml writes as `@directivite`.
    pub fn solver_code(&self) -> u8 {
        match self {
            Directivity::Omni => 0,
            Directivity::Unidirectional { .. } => 1,
            Directivity::PlaneXy => 2,
            Directivity::PlaneYz => 3,
            Directivity::PlaneXz => 4,
            Directivity::Balloon { .. } => 5,
        }
    }

    /// The aim direction, for the two kinds that have one.
    pub fn direction(&self) -> Option<Vec3> {
        match self {
            Directivity::Unidirectional { direction } | Directivity::Balloon { direction, .. } => {
                Some(*direction)
            }
            _ => None,
        }
    }
}

/// [`Directivity`] as read from a file. serde ignores extra keys in a *unit* variant of an
/// internally tagged enum, `deny_unknown_fields` or not, so the reader spells the unit variants
/// as empty struct variants, whose keys it does check. The JSON is the same.
#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum DirectivityIn {
    Omni {},
    Unidirectional { direction: Vec3 },
    PlaneXy {},
    PlaneYz {},
    PlaneXz {},
    Balloon { file: String, direction: Vec3 },
}

impl<'de> Deserialize<'de> for Directivity {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Ok(match DirectivityIn::deserialize(deserializer)? {
            DirectivityIn::Omni {} => Directivity::Omni,
            DirectivityIn::Unidirectional { direction } => {
                Directivity::Unidirectional { direction }
            }
            DirectivityIn::PlaneXy {} => Directivity::PlaneXy,
            DirectivityIn::PlaneYz {} => Directivity::PlaneYz,
            DirectivityIn::PlaneXz {} => Directivity::PlaneXz,
            DirectivityIn::Balloon { file, direction } => Directivity::Balloon { file, direction },
        })
    }
}

/// A point sound source: `sources/source`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Source {
    pub id: SourceId,
    /// `@name`.
    pub name: String,
    /// A disabled source is left out of config.xml.
    pub enabled: bool,
    /// `@x`, `@y`, `@z`. Must lie strictly inside the meshed volume: SPPS crashes on a source
    /// outside it (contract survey, `run_srcout`).
    pub position: Vec3,
    /// Sound power, dB re 1 pW (`bfreq@db` per band, from [`Spectrum::band_levels_db`]).
    pub power: Spectrum,
    pub directivity: Directivity,
    /// `@delay`, seconds.
    pub delay_s: F64,
    /// The source group it sits in, as upstream's GUI groups sources (a source list's element of
    /// type 15, `e_scene_sources.h:73-87`): the groups' names from the outermost, joined by
    /// ` / `. `None` at the top level, as for every source made here. It reaches no solver:
    /// upstream writes a group's sources in its place. Source names need be unique only within
    /// one group (`name_duplicate`), unless per-source output makes them folder names.
    #[serde(deserialize_with = "required")]
    #[schemars(with = "Nullable<String>")]
    pub group: Option<String>,
    /// A pinned element id: upstream's `source@id` for a source imported from a `.proj` (its
    /// `wxid`, `docs/m5-m6-design.md`, decision 13). `None` for a source made here. The solvers
    /// number sources by position and never read `source@id`
    /// (`base_core_configuration.cpp:153`); `config.xml` carries it when pinned, as upstream's
    /// GUI writes it, so that an imported project keeps upstream's ids. At most
    /// [`SOLVER_INT_MAX`], and unique among sources.
    #[serde(deserialize_with = "required")]
    #[schemars(with = "Nullable<u32>", range(max = 2_147_483_647))]
    pub solver_id: Option<u32>,
}

/// A point receiver: `recepteursp/recepteur_ponctuel`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PointReceiver {
    pub id: PointReceiverId,
    /// `@lbl` and `@name`. SPPS makes it a folder name and TCR a file name.
    pub name: String,
    /// `@x`, `@y`, `@z`; must lie inside the meshed volume.
    pub position: Vec3,
    /// `@u`, `@v`, `@w`, normalised by the solver; must not be zero.
    pub orientation: Vec3,
    /// Background noise, `bfreq@db` per band. `None` writes none.
    #[serde(deserialize_with = "required")]
    #[schemars(with = "Nullable<Spectrum>")]
    pub background_noise: Option<Spectrum>,
    /// A pinned solver id (`recepteur_ponctuel@id`). `None` lets export assign one
    /// (`config_xml`, "Solver ids"). A `.proj` import pins upstream's element id (its `wxid`,
    /// `docs/m5-m6-design.md`, decision 13), so an imported project writes upstream's ids. At
    /// most [`SOLVER_INT_MAX`], and unique among point receivers.
    #[serde(deserialize_with = "required")]
    #[schemars(with = "Nullable<u32>", range(max = 2_147_483_647))]
    pub solver_id: Option<u32>,
}

/// A surface receiver (sound map): `recepteurss/recepteur_surfacique` or
/// `recepteur_surfacique_coupe`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SurfaceReceiver {
    pub id: SurfaceReceiverId,
    /// `@name`.
    pub name: String,
    pub enabled: bool,
    pub shape: SurfaceReceiverShape,
    /// A pinned solver id (`recepteur_surfacique@id` or `recepteur_surfacique_coupe@id`, and a
    /// scene receiver's `.cbin` `idRs`). `None` lets export assign one (`config_xml`, "Solver
    /// ids"). A `.proj` import pins upstream's element id (decision 13). At most
    /// [`SOLVER_INT_MAX`], and unique among surface receivers and cutting planes.
    #[serde(deserialize_with = "required")]
    #[schemars(with = "Nullable<u32>", range(max = 2_147_483_647))]
    pub solver_id: Option<u32>,
}

/// Where a surface receiver lies.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum SurfaceReceiverShape {
    /// On the model: every face of these surface groups carries this receiver's id as its `.cbin`
    /// `idRs`. A face has one `idRs`, so a group belongs to at most one scene receiver.
    Scene { groups: Vec<GroupId> },
    /// A rectangle through corners `a`, `b`, `c` (`@ax`..`@cz`), cut into
    /// `ceil(|bc| / resolution) x ceil(|ba| / resolution)` cells (`@resolution`, metres).
    CuttingPlane {
        a: Vec3,
        b: Vec3,
        c: Vec3,
        resolution_m: F64,
    },
}

/// Diffusion law inside a fitting zone: `encombrement/bfreq@loi_diff`, the solver's
/// `DIFFUSION_LAW` (`lib_interface/coreTypes.h:108-113`). The discriminant is the solver code.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[repr(u8)]
pub enum DiffusionLaw {
    #[default]
    Uniform = 0,
    UniformReflection = 1,
    LambertReflection = 2,
}

impl DiffusionLaw {
    pub const ALL: [DiffusionLaw; 3] = [
        DiffusionLaw::Uniform,
        DiffusionLaw::UniformReflection,
        DiffusionLaw::LambertReflection,
    ];

    /// The value config.xml writes as `@loi_diff`.
    pub const fn solver_code(self) -> u8 {
        self as u8
    }

    pub fn from_solver_code(code: u8) -> Option<Self> {
        Self::ALL.get(usize::from(code)).copied()
    }
}

/// A fitting zone (encombrement): a volume filled with scattering objects, `encombrement_enum`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FittingZone {
    pub id: FittingZoneId,
    pub name: String,
    pub enabled: bool,
    pub shape: FittingShape,
    /// `bfreq@alpha` per band: absorption of the fittings, 0 to 1.
    pub absorption: Vec<F64>,
    /// `bfreq@lambda` per band: mean free path between fittings, metres.
    pub mean_free_path_m: Vec<F64>,
    /// `bfreq@loi_diff` per band.
    pub diffusion_law: Vec<DiffusionLaw>,
    /// A pinned solver id (`encombrement@id`, the `.cbin` `idEn`, and the TetGen region
    /// attribute its tetrahedra carry as `idVolume`; the room's parts are numbered by TetGen
    /// above the largest). `None` lets export assign one (`config_xml`, "Solver ids"). A `.proj`
    /// import pins upstream's element id (tutorial 3: 1930 and 2083, its room then 2084 to 2086;
    /// decision 13). At least 1 (the solvers read `idVolume` 0 as no fitting), at most
    /// [`SOLVER_INT_MAX`], unique among fitting zones, and, when enabled, low enough that
    /// TetGen's room ids fit above it (`solver_id_mapping_invalid`).
    #[serde(deserialize_with = "required")]
    #[schemars(with = "Nullable<u32>", range(max = 2_147_483_647))]
    pub solver_id: Option<u32>,
}

/// The volume of a fitting zone. Export marks it as a TetGen region whose attribute is the zone's
/// solver id (`mbin idVolume`, `.cbin idEn`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum FittingShape {
    /// An axis-aligned box, upstream's rectangular fitting zone (element type 56,
    /// `e_scene_encombrements_encombrement_cuboide.h`). `min` and `max` are its bounds, for every
    /// geometric use.
    Box {
        min: Vec3,
        max: Vec3,
        /// Which bound upstream's destination corner `hc` ("Destination volume") takes on each
        /// axis; its origin corner `ba` takes the other. Upstream stores the two corners as the
        /// user typed them, not ordered (tutorial 3's box: `ba` (13, 4, 0), `hc` (18, 1, 1.2)),
        /// and seeds the box's TetGen region at `hc - (hc - ba) * 1e-4`
        /// (`e_scene_encombrements_encombrement_cuboide.h:333-336`), so this is what reproduces
        /// that seed ([`FittingShape::box_corners`]). `None`: `ba` is `min` and `hc` is `max`,
        /// as for a box drawn here.
        #[serde(deserialize_with = "required")]
        #[schemars(with = "Nullable<[BoxBound; 3]>")]
        destination: Option<[BoxBound; 3]>,
    },
    /// The volume closed by the faces of these surface groups; `inside_point` seeds the region.
    Surfaces {
        groups: Vec<GroupId>,
        inside_point: Vec3,
    },
}

/// One bound of a box on one axis: see `FittingShape::Box::destination`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum BoxBound {
    Min,
    Max,
}

impl FittingShape {
    /// A box's corners as upstream holds them, `(ba, hc)`: the origin and the destination, each
    /// taking on every axis the bound `destination` gives it (`hc`) or the other one (`ba`).
    /// `None` for a zone that is not a box.
    pub fn box_corners(&self) -> Option<(Vec3, Vec3)> {
        let FittingShape::Box {
            min,
            max,
            destination,
        } = self
        else {
            return None;
        };
        let (lo, hi) = (min.to_array(), max.to_array());
        let bounds = destination.unwrap_or([BoxBound::Max; 3]);
        let mut ba = [0.0; 3];
        let mut hc = [0.0; 3];
        for a in 0..3 {
            (ba[a], hc[a]) = match bounds[a] {
                BoxBound::Max => (lo[a], hi[a]),
                BoxBound::Min => (hi[a], lo[a]),
            };
        }
        Some((Vec3::from(ba), Vec3::from(hc)))
    }
}

/// Unit of a user-defined air absorption.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum AttenuationUnit {
    /// Energy attenuation per metre (1/m): energy falls as `exp(-m x)`. This is what the solver
    /// reads from `@absatmo`, verbatim.
    PerMetre,
    /// Level attenuation in dB per metre. The solver's own computed path converts dB/m by
    /// ln(10)/10 (`base_core_configuration.cpp:111-115`); so does
    /// [`AttenuationUnit::to_per_metre`].
    DecibelPerMetre,
}

impl AttenuationUnit {
    /// `value` in this unit, as the per-metre energy attenuation the solver reads.
    pub fn to_per_metre(self, value: f64) -> f64 {
        match self {
            AttenuationUnit::PerMetre => value,
            AttenuationUnit::DecibelPerMetre => value * std::f64::consts::LN_10 / 10.0,
        }
    }
}

/// Air absorption: `condition_atmospherique@disable_absatmo_computation` and `@absatmo`.
///
/// In JSON: `{"kind": "iso9613"}` or `{"kind": "user_defined", "value": v, "unit": u}`. Extra
/// keys are refused for both kinds, so a hand-edited `{"kind": "iso9613", "value": 0.5}` is an
/// error, not a silent switch to the computed value.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum AirAbsorption {
    /// Computed by the solver per band from temperature, humidity and pressure (ISO 9613-1).
    #[default]
    Iso9613,
    /// One value for every band, with its unit.
    UserDefined { value: F64, unit: AttenuationUnit },
}

/// [`AirAbsorption`] as read from a file; see [`DirectivityIn`] for why.
#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum AirAbsorptionIn {
    Iso9613 {},
    UserDefined { value: F64, unit: AttenuationUnit },
}

impl<'de> Deserialize<'de> for AirAbsorption {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Ok(match AirAbsorptionIn::deserialize(deserializer)? {
            AirAbsorptionIn::Iso9613 {} => AirAbsorption::Iso9613,
            AirAbsorptionIn::UserDefined { value, unit } => {
                AirAbsorption::UserDefined { value, unit }
            }
        })
    }
}

impl AirAbsorption {
    /// The `@absatmo` value, in 1/m, when user-defined.
    pub fn solver_absatmo(&self) -> Option<f64> {
        match self {
            AirAbsorption::Iso9613 => None,
            AirAbsorption::UserDefined { value, unit } => Some(unit.to_per_metre(value.get())),
        }
    }
}

/// The medium: `condition_atmospherique`. A missing element would leave the sound speed and
/// density at 0, so config.xml always writes every attribute.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Environment {
    /// `@temperature`, degrees Celsius.
    pub temperature_c: F64,
    /// `@humidite`, percent relative humidity.
    pub relative_humidity_percent: F64,
    /// `@pression`, pascals.
    pub pressure_pa: F64,
    pub air_absorption: AirAbsorption,
    /// `@z0`, metres: ground roughness for the celerity profile.
    pub ground_roughness_m: F64,
    /// `@alog`: logarithmic celerity-gradient coefficient (0 = homogeneous).
    pub celerity_gradient_log: F64,
    /// `@blin`: linear celerity-gradient coefficient (0 = homogeneous).
    pub celerity_gradient_lin: F64,
}

impl Default for Environment {
    /// Upstream's GUI defaults: 20 degC, 50 %, 101325 Pa, ISO 9613-1, z0 0.02 m, homogeneous.
    fn default() -> Self {
        Environment {
            temperature_c: F64::new(20.0),
            relative_humidity_percent: F64::new(50.0),
            pressure_pa: F64::new(101_325.0),
            air_absorption: AirAbsorption::Iso9613,
            ground_roughness_m: F64::new(0.02),
            celerity_gradient_log: F64::ZERO,
            celerity_gradient_lin: F64::ZERO,
        }
    }
}

/// SPPS's absorption method: `simulation@computation_method`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[repr(u8)]
pub enum ComputationMethod {
    /// Code 0: Monte-Carlo absorption.
    #[default]
    Random = 0,
    /// Code 1: energy weighting.
    Energetic = 1,
}

impl ComputationMethod {
    pub const fn solver_code(self) -> u8 {
        self as u8
    }
}

/// What SPPS writes on surface receivers: `simulation@surf_receiv_method`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[repr(u8)]
pub enum SoundMapQuantity {
    /// Code 0: angle-weighted intensity.
    #[default]
    Intensity = 0,
    /// Code 1: sound pressure level.
    Spl = 1,
}

impl SoundMapQuantity {
    pub const fn solver_code(self) -> u8 {
        self as u8
    }
}

/// Settings of every solver the project can run.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SolverSettings {
    pub spps: SppsSettings,
    pub tcr: TcrSettings,
    pub meshing: MeshSettings,
}

impl SolverSettings {
    /// Upstream's defaults, computing every one of `n_bands` bands.
    pub fn for_bands(n_bands: usize) -> Self {
        SolverSettings {
            spps: SppsSettings::for_bands(n_bands),
            tcr: TcrSettings::for_bands(n_bands),
            meshing: MeshSettings::default(),
        }
    }
}

/// SPPS, the particle solver: the `simulation` element's SPPS attributes.
///
/// The three integers are C `int`s in the solver: each is at most [`SOLVER_INT_MAX`], which
/// [`Project::check_integrity`] enforces.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SppsSettings {
    /// `@nbparticules`: particles per source per band. The solver raises values below 1 to 1.
    #[schemars(range(max = 2_147_483_647))]
    pub particles_per_source: u32,
    /// `@nbparticules_rendu`: particles per source saved for display; 0 saves none.
    #[schemars(range(max = 2_147_483_647))]
    pub particles_saved: u32,
    /// `@duree_simulation`, seconds.
    pub duration_s: F64,
    /// `@pasdetemps`, seconds. `duration / time step` must stay below 65,536 steps.
    pub time_step_s: F64,
    /// `@random_seed`: 0 leaves the generator unseeded and runs one thread per band; any other
    /// value seeds it and forces a single thread. The solver reads it as a C `int`.
    #[schemars(range(max = 2_147_483_647))]
    pub random_seed: u32,
    pub method: ComputationMethod,
    /// `@abs_atmo_calc`.
    pub air_absorption: bool,
    /// `@enc_calc`: fitting zones on. Off, the solver ignores them silently.
    pub fittings: bool,
    /// `@direct_calc`: direct field only (every surface fully absorbing).
    pub direct_field_only: bool,
    /// `@trans_calc`: transmission through materials that have a transmission loss.
    pub transmission: bool,
    /// `@trans_epsilon`: a particle dies below `E0 * 10^-n`. A missing value (0) kills every
    /// particle at the first step in energetic mode (contract survey, `run_noeps`).
    pub extinction_exponent: F64,
    /// `@rayon_recepteurp`: point-receiver sphere radius, metres. Must be above 0.
    pub receiver_radius_m: F64,
    /// `@surf_receiv_method`.
    pub sound_map: SoundMapQuantity,
    /// `@output_recs_byfreq`: surface-receiver files for every band, not only the global one.
    pub sound_maps_per_band: bool,
    /// `@output_recp_bysource`: an echogram per source as well as the total.
    pub echogram_per_source: bool,
    /// `@save_surface_intersection`: per-band surface collision CSVs.
    pub save_surface_intersections: bool,
    /// `@save_receivers_intersection`: per-band receiver collision CSVs.
    pub save_receiver_intersections: bool,
    /// `freq_enum/bfreq@docalc` per band: which bands SPPS computes.
    pub bands_computed: Vec<bool>,
}

impl SppsSettings {
    /// Upstream's GUI defaults (`e_core_sppscore.h`, `e_core_core_config.h`).
    pub fn for_bands(n_bands: usize) -> Self {
        SppsSettings {
            particles_per_source: 150_000,
            particles_saved: 0,
            duration_s: F64::new(2.0),
            time_step_s: F64::new(0.01),
            random_seed: 0,
            method: ComputationMethod::Random,
            air_absorption: true,
            fittings: true,
            direct_field_only: false,
            transmission: true,
            extinction_exponent: F64::new(5.0),
            receiver_radius_m: F64::new(0.31),
            sound_map: SoundMapQuantity::Intensity,
            sound_maps_per_band: true,
            echogram_per_source: false,
            save_surface_intersections: true,
            save_receiver_intersections: true,
            bands_computed: vec![true; n_bands],
        }
    }
}

/// TCR, the classical theory of reverberation. TCR always writes surface receivers per band
/// (it tests the attribute's presence, not its value), so that switch is not a setting.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TcrSettings {
    /// `@abs_atmo_calc`.
    pub air_absorption: bool,
    /// `freq_enum/bfreq@docalc` per band.
    pub bands_computed: Vec<bool>,
}

impl TcrSettings {
    pub fn for_bands(n_bands: usize) -> Self {
        TcrSettings {
            air_absorption: true,
            bands_computed: vec![true; n_bands],
        }
    }
}

/// TetGen settings (`e_core_core_tetconf.h`).
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MeshSettings {
    /// `-q`: radius-to-edge quality bound.
    pub min_radius_edge_ratio: F64,
    /// `-a`: maximum tetrahedron volume, cubic metres; `None` for no bound.
    #[serde(deserialize_with = "required")]
    #[schemars(with = "Nullable<F64>")]
    pub max_volume_m3: Option<F64>,
    /// Facet-area constraint on surface-receiver faces, square metres (the `.var` file); `None`
    /// for no refinement.
    #[serde(deserialize_with = "required")]
    #[schemars(with = "Nullable<F64>")]
    pub surface_receiver_max_area_m2: Option<F64>,
    /// `-Y`: add no Steiner points on the boundary.
    pub preserve_boundary: bool,
    /// Upstream's "Scene correction before meshing" (`mesh_conf@preprocess`,
    /// `e_core_core_tetconf.h:108`): the `.poly` goes through upstream's `preprocess.exe` before
    /// TetGen, with box fitting zones in its user facet list, as upstream's GUI does it
    /// (`projet_maillage.cpp:206-213`). The geometry check then runs on what `preprocess.exe`
    /// wrote; when it gives up and saves nothing, the `.poly` as written is meshed, as upstream's
    /// GUI meshes it, and the check runs on that, the abort recorded in the mesh manifest.
    /// Upstream's GUI default is on; this crate's default is off (`docs/m5-m6-design.md`,
    /// decision 12).
    pub preprocess: bool,
}

impl Default for MeshSettings {
    fn default() -> Self {
        MeshSettings {
            min_radius_edge_ratio: F64::new(5.0),
            max_volume_m3: None,
            surface_receiver_max_area_m2: None,
            preserve_boundary: true,
            preprocess: false,
        }
    }
}

/// One material override in a [`Variant`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MaterialOverride {
    pub group: GroupId,
    pub material: MaterialId,
}

/// A named set of material overrides on top of the base project.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Variant {
    pub id: VariantId,
    pub name: String,
    /// At most one per group, strictly ascending by group id (so each variant has one spelling).
    pub overrides: Vec<MaterialOverride>,
}

impl Variant {
    /// The material this variant puts on `group`, if it overrides it.
    pub fn override_for(&self, group: GroupId) -> Option<MaterialId> {
        self.overrides
            .binary_search_by(|o| o.group.cmp(&group))
            .ok()
            .map(|i| self.overrides[i].material)
    }

    /// Sets (`Some`) or removes (`None`) the override on `group`, keeping the order. Returns
    /// the previous override.
    pub fn set_override(
        &mut self,
        group: GroupId,
        material: Option<MaterialId>,
    ) -> Option<MaterialId> {
        match (
            self.overrides.binary_search_by(|o| o.group.cmp(&group)),
            material,
        ) {
            (Ok(i), Some(m)) => Some(std::mem::replace(&mut self.overrides[i].material, m)),
            (Ok(i), None) => Some(self.overrides.remove(i).material),
            (Err(i), Some(m)) => {
                self.overrides
                    .insert(i, MaterialOverride { group, material: m });
                None
            }
            (Err(_), None) => None,
        }
    }
}

/// GUI state that belongs to the project.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ViewState {
    /// The saved 3D view; `None` lets the GUI frame the model.
    #[serde(deserialize_with = "required")]
    #[schemars(with = "Nullable<Camera>")]
    pub camera: Option<Camera>,
}

/// A perspective camera looking from `eye` at `target`, Z up.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Camera {
    pub eye: Vec3,
    pub target: Vec3,
    pub vertical_fov_deg: F64,
}
