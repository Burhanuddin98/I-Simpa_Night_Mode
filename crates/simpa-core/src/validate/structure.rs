//! Structural faults: everything [`Project::check_integrity`] checks, collected in full instead of
//! stopping at the first, each under one code. A fault a contract rule describes gets that rule's
//! code; the rest get the structural codes of `schema::IntegrityError` (see the module docs of
//! `validate`).

use std::collections::{HashMap, HashSet};
use std::hash::Hash;

use super::codes::{
    BAND_DUPLICATE, BAND_FREQUENCY_NOT_INTEGER, BAND_SET_EMPTY, BAND_SET_MISMATCH, BANDS,
    DANGLING_REFERENCE, DUPLICATE_ID, FACE_VERTEX, MATERIAL_UNASSIGNED, OVERRIDE_ORDER,
    REPEATED_GROUP, SOLVER_ID_MAPPING_INVALID, SOLVER_INT_RANGE, VARIANT_REFERENCE_INVALID,
    VERSION,
};
use super::{Issue, issue};
use crate::schema::{
    BandKind, FORMAT_VERSION, FittingShape, GroupId, MaterialId, Project, SOLVER_INT_MAX, Spectrum,
    SpectrumShape, SurfaceReceiverShape,
};

pub(super) fn check(p: &Project, out: &mut Vec<Issue>) {
    if p.format_version != FORMAT_VERSION {
        out.push(issue(
            VERSION,
            "/format_version",
            format!(
                "format_version is {}, but this build holds version {FORMAT_VERSION}",
                p.format_version
            ),
        ));
    }
    bands(p, out);
    duplicate_ids(p, out);
    band_counts(p, out);
    references(p, out);
    solver_ints(p, out);
}

pub(super) fn kind_name(kind: BandKind) -> &'static str {
    match kind {
        BandKind::Octave => "octave",
        BandKind::ThirdOctave => "third-octave",
    }
}

fn bands(p: &Project, out: &mut Vec<Issue>) {
    let f = &p.bands.frequencies_hz;
    if f.is_empty() {
        out.push(issue(
            BAND_SET_EMPTY,
            "/bands/frequencies_hz",
            "the project has no bands: both solvers would compute nothing and exit normally",
        ));
        return;
    }
    let mut seen = HashSet::new();
    for (i, &hz) in f.iter().enumerate() {
        let path = format!("/bands/frequencies_hz/{i}");
        if hz == 0 {
            out.push(issue(
                BAND_FREQUENCY_NOT_INTEGER,
                path,
                format!(
                    "band {i} is 0 Hz: a band centre frequency must be a positive whole number of \
                     hertz"
                ),
            ));
        } else if !seen.insert(hz) {
            out.push(issue(
                BAND_DUPLICATE,
                path,
                format!(
                    "{hz} Hz appears more than once: both copies would be computed and their \
                     '{hz} Hz' output folders collide, silently"
                ),
            ));
        } else if p.bands.kind.band_number(hz).is_none() {
            out.push(issue(
                BANDS,
                path,
                format!(
                    "{hz} Hz is not a nominal {} frequency",
                    kind_name(p.bands.kind)
                ),
            ));
        }
    }
    if let Some(i) = f.windows(2).position(|w| w[0] > w[1]) {
        out.push(issue(
            BANDS,
            format!("/bands/frequencies_hz/{}", i + 1),
            format!(
                "the band frequencies are not in ascending order: {} Hz then {} Hz",
                f[i],
                f[i + 1]
            ),
        ));
    }
}

fn first_duplicates<I: Copy + Eq + Hash>(ids: impl Iterator<Item = I>) -> Vec<usize> {
    let mut seen = HashSet::new();
    ids.enumerate()
        .filter(|&(_, id)| !seen.insert(id))
        .map(|(i, _)| i)
        .collect()
}

fn duplicate_ids(p: &Project, out: &mut Vec<Issue>) {
    let mut report = |collection: &str, kind: &str, dups: Vec<usize>| {
        for i in dups {
            out.push(issue(
                DUPLICATE_ID,
                format!("/{collection}/{i}/id"),
                format!("{kind} {i} has the same id as an earlier {kind}"),
            ));
        }
    };
    report(
        "surface_groups",
        "surface group",
        first_duplicates(p.surface_groups.iter().map(|g| g.id)),
    );
    report(
        "materials",
        "material",
        first_duplicates(p.materials.iter().map(|m| m.id)),
    );
    report(
        "sources",
        "source",
        first_duplicates(p.sources.iter().map(|s| s.id)),
    );
    report(
        "point_receivers",
        "point receiver",
        first_duplicates(p.point_receivers.iter().map(|r| r.id)),
    );
    report(
        "surface_receivers",
        "surface receiver",
        first_duplicates(p.surface_receivers.iter().map(|r| r.id)),
    );
    report(
        "fitting_zones",
        "fitting zone",
        first_duplicates(p.fitting_zones.iter().map(|z| z.id)),
    );
    report(
        "variants",
        "variant",
        first_duplicates(p.variants.iter().map(|v| v.id)),
    );
}

fn band_counts(p: &Project, out: &mut Vec<Issue>) {
    let n = p.bands.len();
    let mut count = |path: String, what: String, found: usize| {
        if found != n {
            out.push(issue(
                BAND_SET_MISMATCH,
                path,
                format!(
                    "{what} has {found} per-band values, but the project has {n} bands: the \
                     solvers map spectra to bands by position, so the values would land on the \
                     wrong bands or be read past their end"
                ),
            ));
        }
    };
    let spectrum = |s: &Spectrum| match &s.shape {
        SpectrumShape::Custom { relative_db } => Some(relative_db.len()),
        SpectrumShape::Pink | SpectrumShape::White => None,
    };
    for (i, m) in p.materials.iter().enumerate() {
        let what = |field: &str| format!("material '{}' {field}", m.name);
        count(
            format!("/materials/{i}/absorption"),
            what("absorption"),
            m.absorption.len(),
        );
        count(
            format!("/materials/{i}/scattering"),
            what("scattering"),
            m.scattering.len(),
        );
        if let Some(k) = m.reflection_law.band_count() {
            count(
                format!("/materials/{i}/reflection_law"),
                what("reflection law"),
                k,
            );
        }
        if let Some(t) = &m.transmission_loss_db {
            count(
                format!("/materials/{i}/transmission_loss_db"),
                what("transmission loss"),
                t.len(),
            );
        }
    }
    for (i, s) in p.sources.iter().enumerate() {
        if let Some(len) = spectrum(&s.power) {
            count(
                format!("/sources/{i}/power/shape/relative_db"),
                format!("source '{}' power", s.name),
                len,
            );
        }
    }
    for (i, r) in p.point_receivers.iter().enumerate() {
        if let Some(len) = r.background_noise.as_ref().and_then(spectrum) {
            count(
                format!("/point_receivers/{i}/background_noise/shape/relative_db"),
                format!("receiver '{}' background noise", r.name),
                len,
            );
        }
    }
    for (i, z) in p.fitting_zones.iter().enumerate() {
        let what = |field: &str| format!("fitting zone '{}' {field}", z.name);
        count(
            format!("/fitting_zones/{i}/absorption"),
            what("absorption"),
            z.absorption.len(),
        );
        count(
            format!("/fitting_zones/{i}/mean_free_path_m"),
            what("mean free path"),
            z.mean_free_path_m.len(),
        );
        count(
            format!("/fitting_zones/{i}/diffusion_law"),
            what("diffusion law"),
            z.diffusion_law.len(),
        );
    }
    count(
        "/solvers/spps/bands_computed".to_string(),
        "SPPS bands_computed".to_string(),
        p.solvers.spps.bands_computed.len(),
    );
    count(
        "/solvers/tcr/bands_computed".to_string(),
        "TCR bands_computed".to_string(),
        p.solvers.tcr.bands_computed.len(),
    );
}

/// Faults in one list of group references: a group that does not exist, or one named twice.
fn group_list(
    groups: &[GroupId],
    exists: &HashSet<GroupId>,
    path: &str,
    owner: &str,
    out: &mut Vec<Issue>,
) {
    let mut seen = HashSet::new();
    for (j, g) in groups.iter().enumerate() {
        if !exists.contains(g) {
            out.push(issue(
                DANGLING_REFERENCE,
                format!("{path}/{j}"),
                format!("{owner} names surface group {g}, which does not exist"),
            ));
        } else if !seen.insert(g) {
            out.push(issue(
                REPEATED_GROUP,
                format!("{path}/{j}"),
                format!("{owner} names surface group {g} more than once"),
            ));
        }
    }
}

fn references(p: &Project, out: &mut Vec<Issue>) {
    let groups: HashSet<GroupId> = p.surface_groups.iter().map(|g| g.id).collect();
    let materials: HashSet<MaterialId> = p.materials.iter().map(|m| m.id).collect();

    for (i, g) in p.surface_groups.iter().enumerate() {
        if !materials.contains(&g.material) {
            out.push(issue(
                MATERIAL_UNASSIGNED,
                format!("/surface_groups/{i}/material"),
                format!(
                    "surface group '{}' uses material {}, which does not exist, so its faces \
                     have no material: the solvers exit -1, or crash, on such a face",
                    g.name, g.material
                ),
            ));
        }
    }

    // Faces: one issue per fault kind, naming the first face and the count.
    let n_vertices = p.geometry.vertices.len();
    let bad_vertex: Vec<usize> = p
        .geometry
        .faces
        .iter()
        .enumerate()
        .filter(|(_, f)| f.vertices.iter().any(|&v| v as usize >= n_vertices))
        .map(|(i, _)| i)
        .collect();
    if let Some(&first) = bad_vertex.first() {
        out.push(issue(
            FACE_VERTEX,
            format!("/geometry/faces/{first}"),
            format!(
                "{} face(s), the first face {first}, use a vertex index past the {n_vertices} \
                 vertices",
                bad_vertex.len()
            ),
        ));
    }
    let mut missing_group: Vec<(GroupId, usize, usize)> = Vec::new();
    for (i, f) in p.geometry.faces.iter().enumerate() {
        if !groups.contains(&f.group) {
            match missing_group.iter_mut().find(|(g, _, _)| *g == f.group) {
                Some((_, _, n)) => *n += 1,
                None => missing_group.push((f.group, i, 1)),
            }
        }
    }
    for (g, first, n) in missing_group {
        out.push(issue(
            MATERIAL_UNASSIGNED,
            format!("/geometry/faces/{first}"),
            format!(
                "{n} face(s), the first face {first}, belong to surface group {g}, which does not \
                 exist, so they have no material: the solvers exit -1, or crash, on such a face"
            ),
        ));
    }

    for (i, r) in p.surface_receivers.iter().enumerate() {
        if let SurfaceReceiverShape::Scene { groups: list } = &r.shape {
            group_list(
                list,
                &groups,
                &format!("/surface_receivers/{i}/shape/groups"),
                &format!("surface receiver '{}'", r.name),
                out,
            );
        }
    }
    for (i, z) in p.fitting_zones.iter().enumerate() {
        if let FittingShape::Surfaces { groups: list, .. } = &z.shape {
            group_list(
                list,
                &groups,
                &format!("/fitting_zones/{i}/shape/groups"),
                &format!("fitting zone '{}'", z.name),
                out,
            );
        }
    }

    for (i, v) in p.variants.iter().enumerate() {
        if v.overrides.windows(2).any(|w| w[0].group >= w[1].group) {
            out.push(issue(
                OVERRIDE_ORDER,
                format!("/variants/{i}/overrides"),
                format!(
                    "variant '{}': the overrides are not strictly ascending by group id",
                    v.name
                ),
            ));
        }
        for (j, o) in v.overrides.iter().enumerate() {
            if !groups.contains(&o.group) {
                out.push(issue(
                    VARIANT_REFERENCE_INVALID,
                    format!("/variants/{i}/overrides/{j}/group"),
                    format!(
                        "variant '{}' overrides surface group {}, which does not exist",
                        v.name, o.group
                    ),
                ));
            }
            if !materials.contains(&o.material) {
                out.push(issue(
                    VARIANT_REFERENCE_INVALID,
                    format!("/variants/{i}/overrides/{j}/material"),
                    format!(
                        "variant '{}' puts material {} on a surface group, and that material does \
                         not exist: the group's faces would have no material",
                        v.name, o.material
                    ),
                ));
            }
        }
    }
    if let Some(active) = p.active_variant
        && p.variant(active).is_none()
    {
        out.push(issue(
            VARIANT_REFERENCE_INVALID,
            "/active_variant",
            format!("the active variant {active} does not exist"),
        ));
    }
}

fn solver_ints(p: &Project, out: &mut Vec<Issue>) {
    let seed = p.solvers.spps.random_seed;
    if seed > SOLVER_INT_MAX {
        out.push(issue(
            SOLVER_INT_RANGE,
            "/solvers/spps/random_seed",
            format!(
                "the random seed {seed} does not fit the C int the solver reads it into (at most \
                 {SOLVER_INT_MAX})"
            ),
        ));
    }
    for (i, m) in p.materials.iter().enumerate() {
        if let Some(id) = m.solver_id
            && id > SOLVER_INT_MAX
        {
            out.push(issue(
                SOLVER_INT_RANGE,
                format!("/materials/{i}/solver_id"),
                format!(
                    "material '{}' pins solver id {id}, which does not fit the C int the solver \
                     reads it into (at most {SOLVER_INT_MAX})",
                    m.name
                ),
            ));
        }
    }
    let mut first: HashMap<u32, usize> = HashMap::new();
    for (i, m) in p.materials.iter().enumerate() {
        if let Some(id) = m.solver_id {
            if let Some(&j) = first.get(&id) {
                out.push(issue(
                    SOLVER_ID_MAPPING_INVALID,
                    format!("/materials/{i}/solver_id"),
                    format!(
                        "materials '{}' and '{}' both pin solver id {id}: the solver takes the \
                         first material with an id, so the second would be hidden",
                        p.materials[j].name, m.name
                    ),
                ));
            } else {
                first.insert(id, i);
            }
        }
    }
    // The other kinds' pinned ids (a `.proj` import pins upstream's element ids, decision 13).
    let pins = |list: &str, kind: &str, pins: Vec<(Option<u32>, &str)>, out: &mut Vec<Issue>| {
        let mut first: HashMap<u32, &str> = HashMap::new();
        for (i, (pin, name)) in pins.into_iter().enumerate() {
            let Some(id) = pin else { continue };
            let path = format!("/{list}/{i}/solver_id");
            if id > SOLVER_INT_MAX {
                out.push(issue(
                    SOLVER_INT_RANGE,
                    path.clone(),
                    format!(
                        "{kind} '{name}' pins solver id {id}, which does not fit the C int the \
                         solver reads it into (at most {SOLVER_INT_MAX})"
                    ),
                ));
            }
            if list == "fitting_zones" && id == 0 {
                out.push(issue(
                    SOLVER_ID_MAPPING_INVALID,
                    path.clone(),
                    format!(
                        "fitting zone '{name}' pins solver id 0, which the solvers read on a \
                         tetrahedron as no fitting: its tetrahedra would carry none"
                    ),
                ));
            }
            match first.get(&id) {
                Some(earlier) => out.push(issue(
                    SOLVER_ID_MAPPING_INVALID,
                    path,
                    if list == "sources" {
                        format!(
                            "sources '{earlier}' and '{name}' both pin element id {id}: the id \
                             reaches no solver, but it names one element, and two cannot share it"
                        )
                    } else {
                        format!(
                            "{kind}s '{earlier}' and '{name}' both pin solver id {id}: the \
                             solver takes the first with an id, so the second would be hidden"
                        )
                    },
                )),
                None => {
                    first.insert(id, name);
                }
            }
        }
    };
    pins(
        "point_receivers",
        "point receiver",
        p.point_receivers
            .iter()
            .map(|r| (r.solver_id, r.name.as_str()))
            .collect(),
        out,
    );
    pins(
        "surface_receivers",
        "surface receiver",
        p.surface_receivers
            .iter()
            .map(|r| (r.solver_id, r.name.as_str()))
            .collect(),
        out,
    );
    pins(
        "fitting_zones",
        "fitting zone",
        p.fitting_zones
            .iter()
            .map(|z| (z.solver_id, z.name.as_str()))
            .collect(),
        out,
    );
    pins(
        "sources",
        "source",
        p.sources
            .iter()
            .map(|s| (s.solver_id, s.name.as_str()))
            .collect(),
        out,
    );
}
