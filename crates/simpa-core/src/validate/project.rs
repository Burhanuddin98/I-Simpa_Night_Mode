//! The `project` rules of `docs/solver-contract.md` Part A that are about values, geometry,
//! names and files, in the page's order. Structural faults are in `structure.rs`.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use super::codes::*;
use super::directivity::{self, Balloon};
use super::geometry::{self, PointLocation, cross, norm};
use super::names::{collision_key, filename_problem};
use super::{
    Context, Issue, MAX_NAME_BYTES, MAX_TIME_STEPS, SOURCE_CLEARANCE_M, issue, mesh_input_hash,
};
use crate::schema::{
    AirAbsorption, Directivity, F64, MaterialId, Project, SOLVER_INT_MAX, SurfaceReceiverShape,
    Vec3,
};

pub(super) fn check(p: &Project, ctx: &Context, out: &mut Vec<Issue>) {
    materials(p, out);
    let triangles = geometry::triangles(&p.geometry);
    sources_and_receivers(p, &triangles, out);
    directivity_files(p, ctx, out);
    time(p, out);
    spps_settings(p, out);
    environment(p, out);
    names(p, out);
    surface_receivers(p, out);
    fittings(p, out);
    mesh(p, ctx, out);
    mesh_settings(p, out);
}

/// Finite in `f64` and still finite once rounded to the solver's `f32`.
fn finite(x: f64) -> bool {
    x.is_finite() && (x as f32).is_finite()
}

/// Above 0 and finite, in `f64` and in `f32`.
fn positive(x: f64) -> bool {
    finite(x) && x > 0.0 && (x as f32) > 0.0
}

/// In [0, 1] and finite.
fn unit(x: f64) -> bool {
    finite(x) && (0.0..=1.0).contains(&x)
}

fn band_label(p: &Project, j: usize) -> String {
    match p.bands.frequencies_hz.get(j) {
        Some(f) => format!("{f} Hz"),
        None => format!("band {j}"),
    }
}

/// The first index where `bad` holds and how many more there are.
fn first_bad(values: &[F64], bad: impl Fn(f64) -> bool) -> Option<(usize, usize)> {
    let hits: Vec<usize> = values
        .iter()
        .enumerate()
        .filter(|(_, v)| bad(v.get()))
        .map(|(j, _)| j)
        .collect();
    hits.first().map(|&j| (j, hits.len() - 1))
}

fn and_more(n: usize) -> String {
    match n {
        0 => String::new(),
        1 => " and in 1 other band".to_string(),
        _ => format!(" and in {n} other bands"),
    }
}

/// Indices into `p.materials` of the materials the solver will see: the effective material of
/// every surface group under the active variant (the base project if it does not exist).
fn materials_in_use(p: &Project) -> Vec<usize> {
    let variant = p.active_variant.and_then(|v| p.variant(v));
    let used: HashSet<MaterialId> = p
        .surface_groups
        .iter()
        .map(|g| {
            variant
                .and_then(|v| v.overrides.iter().find(|o| o.group == g.id))
                .map_or(g.material, |o| o.material)
        })
        .collect();
    (0..p.materials.len())
        .filter(|&i| used.contains(&p.materials[i].id))
        .collect()
}

fn materials(p: &Project, out: &mut Vec<Issue>) {
    let in_use = materials_in_use(p);
    let mut out_of_range = Vec::new();
    let mut ignored = Vec::new();
    let mut transmission = Vec::new();
    for &i in &in_use {
        let m = &p.materials[i];
        for (field, values) in [("absorption", &m.absorption), ("scattering", &m.scattering)] {
            if let Some((j, more)) = first_bad(values, |v| !unit(v)) {
                out_of_range.push(issue(
                    MATERIAL_VALUE_OUT_OF_RANGE,
                    format!("/materials/{i}/{field}/{j}"),
                    format!(
                        "material '{}' {field} is {} at {}{}; it must be a finite value from 0 \
                         to 1. The solvers check nothing, so the energy balance breaks",
                        m.name,
                        values[j],
                        band_label(p, j),
                        and_more(more)
                    ),
                ));
            }
        }
        // A band that does not transmit (None) has no loss to check.
        let losses: Vec<(usize, f64)> = m
            .transmission_loss_db
            .iter()
            .flatten()
            .enumerate()
            .filter_map(|(j, r)| r.map(|r| (j, r.get())))
            .collect();
        let bad: Vec<(usize, f64)> = losses
            .iter()
            .copied()
            .filter(|&(_, r)| !(finite(r) && r >= 0.0))
            .collect();
        if let Some(&(j, r)) = bad.first() {
            out_of_range.push(issue(
                MATERIAL_VALUE_OUT_OF_RANGE,
                format!("/materials/{i}/transmission_loss_db/{j}"),
                format!(
                    "material '{}' transmission loss is {} dB at {}{}; it must be finite and at \
                     least 0 dB, or tau = 10^(-R/10) exceeds 1",
                    m.name,
                    F64::new(r),
                    band_label(p, j),
                    and_more(bad.len() - 1)
                ),
            ));
        }

        // Warnings, on bands whose values are in range.
        let bands = m.absorption.len().min(m.scattering.len());
        let full: Vec<usize> = (0..bands)
            .filter(|&j| {
                let (a, s) = (m.absorption[j].get(), m.scattering[j].get());
                unit(a) && unit(s) && a == 1.0 && s > 0.0
            })
            .collect();
        if let Some(&j) = full.first() {
            ignored.push(issue(
                MATERIAL_DIFFUSION_IGNORED,
                format!("/materials/{i}/scattering/{j}"),
                format!(
                    "material '{}' absorbs fully (alpha = 1) at {}{}, so its scattering {} there \
                     has no effect: a fully absorbed particle never reflects. Upstream's GUI sets \
                     it to 0",
                    m.name,
                    band_label(p, j),
                    and_more(full.len() - 1),
                    m.scattering[j]
                ),
            ));
        }
        {
            let exceeds: Vec<(usize, f64)> = losses
                .iter()
                .copied()
                .filter(|&(j, r)| {
                    let Some(a) = m.absorption.get(j).map(|a| a.get()) else {
                        return false;
                    };
                    unit(a) && finite(r) && r >= 0.0 && (a == 0.0 || 10f64.powf(-r / 10.0) > a)
                })
                .collect();
            if let Some(&(j, r)) = exceeds.first() {
                let a = m.absorption[j].get();
                let tau = 10f64.powf(-r / 10.0);
                let why = if a == 0.0 {
                    "transmission needs alpha > 0: with alpha = 0 neither mode transmits"
                        .to_string()
                } else {
                    format!("tau = 10^(-R/10) = {tau} exceeds alpha = {a}")
                };
                transmission.push(issue(
                    MATERIAL_TRANSMISSION_EXCEEDS_ABSORPTION,
                    format!("/materials/{i}/transmission_loss_db/{j}"),
                    format!(
                        "material '{}' at {}{}: {why}. The exporter writes tau = alpha \
                         (R = -10 log10(alpha)) instead, as upstream's GUI does",
                        m.name,
                        band_label(p, j),
                        and_more(exceeds.len() - 1)
                    ),
                ));
            }
        }
    }
    out.append(&mut out_of_range);
    out.append(&mut ignored);
    out.append(&mut transmission);
}

/// Non-zero and finite in `f64`, and still non-zero once each component is an `f32`: the
/// solver divides the vector by its `f32` length.
fn usable_direction(v: Vec3) -> bool {
    let [x, y, z] = v.to_array();
    let l64 = (x * x + y * y + z * z).sqrt();
    let (xf, yf, zf) = (x as f32, y as f32, z as f32);
    let l32 = (xf * xf + yf * yf + zf * zf).sqrt();
    l64.is_finite() && l64 > 0.0 && l32.is_finite() && l32 > 0.0
}

fn fmt_point(v: Vec3) -> String {
    let [x, y, z] = v.to_array();
    format!("({x}, {y}, {z})")
}

fn sources_and_receivers(p: &Project, triangles: &[[[f64; 3]; 3]], out: &mut Vec<Issue>) {
    let enabled: Vec<usize> = (0..p.sources.len())
        .filter(|&i| p.sources[i].enabled)
        .collect();
    if enabled.is_empty() {
        out.push(issue(
            SOURCE_NONE,
            "/sources",
            "no source is enabled: the run would emit nothing and exit 0",
        ));
    }

    let mut outside = Vec::new();
    let mut near = Vec::new();
    for &i in &enabled {
        let s = &p.sources[i];
        let path = format!("/sources/{i}/position");
        match geometry::locate(triangles, s.position.to_array()) {
            PointLocation::Outside { distance_m } => outside.push(issue(
                SOURCE_OUTSIDE_VOLUME,
                path,
                format!(
                    "source '{}' at {} is outside the room ({} m from the nearest face): SPPS \
                     crashes (0xC0000005) and TCR computes on regardless",
                    s.name,
                    fmt_point(s.position),
                    distance_m
                ),
            )),
            PointLocation::OnSurface { distance_m: d }
            | PointLocation::Inside { clearance_m: d }
                if d < SOURCE_CLEARANCE_M =>
            {
                near.push(issue(
                    SOURCE_NEAR_SURFACE,
                    path,
                    format!(
                        "source '{}' at {} is {d} m from the nearest face, closer than \
                         {SOURCE_CLEARANCE_M} m: SPPS may place it in either neighbouring \
                         tetrahedron, or stop without results",
                        s.name,
                        fmt_point(s.position)
                    ),
                ));
            }
            _ => {}
        }
    }
    out.append(&mut outside);
    out.append(&mut near);

    let radius = p.solvers.spps.receiver_radius_m.get();
    let radius_ok = positive(radius);
    let mut r_outside = Vec::new();
    let mut crosses = Vec::new();
    for (i, r) in p.point_receivers.iter().enumerate() {
        let path = format!("/point_receivers/{i}/position");
        match geometry::locate(triangles, r.position.to_array()) {
            PointLocation::Outside { distance_m } => r_outside.push(issue(
                RECEIVER_OUTSIDE_VOLUME,
                path,
                format!(
                    "receiver '{}' at {} is outside the room ({} m from the nearest face): it is \
                     never linked to a tetrahedron and records zero, silently",
                    r.name,
                    fmt_point(r.position),
                    distance_m
                ),
            )),
            PointLocation::OnSurface { .. } => r_outside.push(issue(
                RECEIVER_OUTSIDE_VOLUME,
                path,
                format!(
                    "receiver '{}' at {} lies on a face, not strictly inside the room",
                    r.name,
                    fmt_point(r.position)
                ),
            )),
            PointLocation::Inside { clearance_m } if radius_ok && clearance_m < radius => {
                crosses.push(issue(
                    RECEIVER_SPHERE_CROSSES_SURFACE,
                    path,
                    format!(
                        "receiver '{}' is {clearance_m} m from the nearest face, inside its \
                         sphere of radius {radius} m: SPPS normalises by the whole sphere's \
                         volume, so the level reads low",
                        r.name
                    ),
                ));
            }
            PointLocation::Inside { .. } => {}
        }
    }
    out.append(&mut r_outside);
    out.append(&mut crosses);

    if !radius_ok {
        out.push(issue(
            RECEIVER_RADIUS_INVALID,
            "/solvers/spps/receiver_radius_m",
            format!(
                "the receiver radius is {radius} m; it must be above 0 and finite, or every \
                 receiver value is NaN"
            ),
        ));
    }

    for &i in &enabled {
        let s = &p.sources[i];
        if let Some(d) = s.directivity.direction()
            && !usable_direction(d)
        {
            out.push(issue(
                DIRECTION_VECTOR_ZERO,
                format!("/sources/{i}/directivity/direction"),
                format!(
                    "source '{}' direction {} is zero or not finite: the solver divides it by its \
                     length and gets NaN",
                    s.name,
                    fmt_point(d)
                ),
            ));
        }
    }
    for (i, r) in p.point_receivers.iter().enumerate() {
        if !usable_direction(r.orientation) {
            out.push(issue(
                DIRECTION_VECTOR_ZERO,
                format!("/point_receivers/{i}/orientation"),
                format!(
                    "receiver '{}' orientation {} is zero or not finite: the solver divides it by \
                     its length and gets NaN",
                    r.name,
                    fmt_point(r.orientation)
                ),
            ));
        }
    }
}

/// The file a balloon source names, resolved against the project folder.
fn resolve(file: &str, ctx: &Context) -> Result<PathBuf, String> {
    let path = Path::new(file);
    if path.is_absolute() {
        Ok(path.to_path_buf())
    } else {
        match &ctx.project_dir {
            Some(dir) => Ok(dir.join(path)),
            None => Err(format!(
                "names '{file}', a relative path, and the project has no folder to resolve it \
                 against"
            )),
        }
    }
}

/// A directivity file, read once per path.
enum Loaded {
    Unreadable(String),
    Invalid(String),
    Parsed(Balloon),
}

fn directivity_files(p: &Project, ctx: &Context, out: &mut Vec<Issue>) {
    let mut parsed: HashMap<PathBuf, Loaded> = HashMap::new();
    let mut missing = Vec::new();
    let mut invalid = Vec::new();
    let mut bands = Vec::new();
    for (i, s) in p.sources.iter().enumerate() {
        let Directivity::Balloon { file, .. } = &s.directivity else {
            continue;
        };
        if !s.enabled {
            continue;
        }
        let path = format!("/sources/{i}/directivity/file");
        if file.trim().is_empty() {
            missing.push(issue(
                DIRECTIVITY_FILE_MISSING,
                path,
                format!(
                    "source '{}' has a directivity balloon but names no file: SPPS crashes \
                     (0xC0000005)",
                    s.name
                ),
            ));
            continue;
        }
        let resolved = match resolve(file, ctx) {
            Ok(r) => r,
            Err(why) => {
                missing.push(issue(
                    DIRECTIVITY_FILE_MISSING,
                    path,
                    format!("source '{}' {why}", s.name),
                ));
                continue;
            }
        };
        let loaded =
            parsed
                .entry(resolved.clone())
                .or_insert_with(|| match std::fs::read(&resolved) {
                    Ok(bytes) => match directivity::parse(&bytes) {
                        Ok(balloon) => Loaded::Parsed(balloon),
                        Err(why) => Loaded::Invalid(why),
                    },
                    Err(e) => Loaded::Unreadable(e.to_string()),
                });
        match loaded {
            Loaded::Unreadable(e) => missing.push(issue(
                DIRECTIVITY_FILE_MISSING,
                path,
                format!(
                    "source '{}': the directivity file {} cannot be read ({e}): the source would \
                     emit nothing in any band, exit 0",
                    s.name,
                    resolved.display(),
                ),
            )),
            Loaded::Invalid(why) => invalid.push(issue(
                DIRECTIVITY_FILE_INVALID,
                path,
                format!(
                    "source '{}': the directivity file {} is invalid: {why}",
                    s.name,
                    resolved.display(),
                ),
            )),
            Loaded::Parsed(balloon) => {
                let absent: Vec<u32> = p
                    .bands
                    .frequencies_hz
                    .iter()
                    .copied()
                    .filter(|&f| f != 0 && !balloon.covers(f))
                    .collect();
                if let Some(&first) = absent.first() {
                    bands.push(issue(
                        DIRECTIVITY_BAND_MISSING,
                        path,
                        format!(
                            "source '{}': the directivity file {} has no complete block at \
                             {first} Hz{}: SPPS emits nothing from this source in those bands",
                            s.name,
                            resolved.display(),
                            and_more(absent.len() - 1)
                        ),
                    ));
                }
            }
        }
    }
    out.append(&mut missing);
    out.append(&mut invalid);
    out.append(&mut bands);
}

fn time(p: &Project, out: &mut Vec<Issue>) {
    let spps = &p.solvers.spps;
    let (dt, dur) = (spps.time_step_s.get(), spps.duration_s.get());
    let mut ok = true;
    for (field, value, what) in [
        ("time_step_s", dt, "time step"),
        ("duration_s", dur, "duration"),
    ] {
        if !positive(value) {
            ok = false;
            out.push(issue(
                TIME_STEP_INVALID,
                format!("/solvers/spps/{field}"),
                format!(
                    "the {what} is {value} s; it must be above 0 and finite, or the step count \
                     does not fit an int (SPPS aborts)"
                ),
            ));
        }
    }
    if !ok {
        return;
    }
    let (dt32, dur32) = (dt as f32, dur as f32);
    // The solver: (int)ceil(duree / pasdetemps), in float (base_core_configuration.cpp:94).
    let steps32 = f64::from((dur32 / dt32).ceil());
    let steps = (dur / dt).ceil().max(steps32);
    if steps >= f64::from(MAX_TIME_STEPS) {
        out.push(issue(
            STEP_COUNT_OVERFLOW,
            "/solvers/spps/duration_s",
            format!(
                "{dur} s in steps of {dt} s is {steps} steps; it must stay below \
                 {MAX_TIME_STEPS}, because a particle's step counter is 16 bits"
            ),
        ));
    }
    for (i, s) in p.sources.iter().enumerate() {
        if !s.enabled {
            continue;
        }
        let d = s.delay_s.get();
        let path = format!("/sources/{i}/delay_s");
        let problem = if !finite(d) || d < 0.0 {
            Some(format!("{d} s is negative or not finite"))
        } else {
            // The solver: start = (u16)ceil(delay / pasdetemps), in float, and the source emits
            // only while start < the step count (sppsNantes.cpp:91-93).
            let start32 = f64::from(((d as f32) / dt32).ceil());
            let start = (d / dt).ceil().max(start32);
            if start >= f64::from(MAX_TIME_STEPS) {
                Some(format!(
                    "{d} s is step {start}, past the 16-bit step counter, which wraps"
                ))
            } else if d >= dur {
                Some(format!(
                    "{d} s is at or after the end of the {dur} s simulation, so the source emits \
                     nothing"
                ))
            } else if start32 >= steps32 {
                Some(format!(
                    "{d} s starts emission at step {start32} of a {steps32}-step simulation \
                     (ceil(delay / time step), in the solver's float arithmetic), so the source \
                     emits nothing"
                ))
            } else {
                None
            }
        };
        if let Some(why) = problem {
            out.push(issue(
                SOURCE_DELAY_INVALID,
                path,
                format!("source '{}' delay: {why}", s.name),
            ));
        }
    }
}

fn spps_settings(p: &Project, out: &mut Vec<Issue>) {
    let spps = &p.solvers.spps;
    let eps = spps.extinction_exponent.get();
    if !positive(eps) {
        out.push(issue(
            TRANS_EPSILON_INVALID,
            "/solvers/spps/extinction_exponent",
            format!(
                "the particle extinction exponent is {eps}; it must be above 0 and finite, or \
                 every particle dies at once, silently"
            ),
        ));
    }
    let (n, saved) = (spps.particles_per_source, spps.particles_saved);
    if n == 0 || n > SOLVER_INT_MAX {
        out.push(issue(
            PARTICLE_COUNT_INVALID,
            "/solvers/spps/particles_per_source",
            format!("{n} particles per source per band; it must be from 1 to {SOLVER_INT_MAX}"),
        ));
    }
    if saved > n || saved > SOLVER_INT_MAX {
        out.push(issue(
            PARTICLE_COUNT_INVALID,
            "/solvers/spps/particles_saved",
            format!(
                "{saved} particles saved per source, more than the {n} computed: SPPS would save \
                 none while sizing the .pbin for them"
            ),
        ));
    }
}

fn environment(p: &Project, out: &mut Vec<Issue>) {
    let e = &p.environment;
    let mut bad = |field: &str, message: String| {
        out.push(issue(
            ATMOSPHERE_INVALID,
            format!("/environment/{field}"),
            message,
        ));
    };
    let t = e.temperature_c.get();
    if !(finite(t) && t > -273.15 && (t as f32) + 273.15f32 > 0.0) {
        bad(
            "temperature_c",
            format!("the temperature is {t} °C; it must be above -273.15 °C and finite"),
        );
    }
    let pa = e.pressure_pa.get();
    if !positive(pa) {
        bad(
            "pressure_pa",
            format!("the pressure is {pa} Pa; it must be above 0 and finite"),
        );
    }
    let h = e.relative_humidity_percent.get();
    if !(finite(h) && (0.0..=100.0).contains(&h)) {
        bad(
            "relative_humidity_percent",
            format!("the relative humidity is {h} %; it must be from 0 to 100"),
        );
    }
    let (alog, blin) = (e.celerity_gradient_log.get(), e.celerity_gradient_lin.get());
    for (field, value) in [
        ("celerity_gradient_log", alog),
        ("celerity_gradient_lin", blin),
    ] {
        if !finite(value) {
            bad(
                field,
                format!("the sound-speed gradient coefficient is {value}; it must be finite"),
            );
        }
    }
    let z0 = e.ground_roughness_m.get();
    if (alog != 0.0 || blin != 0.0) && !positive(z0) {
        bad(
            "ground_roughness_m",
            format!(
                "the ground roughness z0 is {z0} m while a sound-speed gradient is set; it must \
                 be above 0, since the profile uses ln(1 + z/z0)"
            ),
        );
    }
    if let AirAbsorption::UserDefined { value, unit } = &e.air_absorption {
        let v = value.get();
        if !(finite(v) && v >= 0.0 && finite(unit.to_per_metre(v))) {
            out.push(issue(
                ABSATMO_INVALID,
                "/environment/air_absorption/value",
                format!("the user-defined air absorption is {v}; it must be finite and at least 0"),
            ));
        }
    }
}

fn names(p: &Project, out: &mut Vec<Issue>) {
    // (path, kind, name) of every name that reaches a solver.
    let sources: Vec<(String, &str)> = p
        .sources
        .iter()
        .enumerate()
        .filter(|(_, s)| s.enabled)
        .map(|(i, s)| (format!("/sources/{i}/name"), s.name.as_str()))
        .collect();
    let receivers: Vec<(String, &str)> = p
        .point_receivers
        .iter()
        .enumerate()
        .map(|(i, r)| (format!("/point_receivers/{i}/name"), r.name.as_str()))
        .collect();
    let groups = [("source name", &sources), ("receiver label", &receivers)];

    for (kind, list) in groups {
        for (path, name) in list.iter() {
            if name.len() > MAX_NAME_BYTES {
                out.push(issue(
                    NAME_TOO_LONG,
                    path.clone(),
                    format!(
                        "the {kind} '{name}' is {} bytes in UTF-8; at most {MAX_NAME_BYTES} fit \
                         the solver's 50-byte label cell",
                        name.len()
                    ),
                ));
            }
        }
    }
    for (kind, list) in groups {
        for (path, name) in list.iter() {
            if let Some(why) = filename_problem(name) {
                out.push(issue(
                    NAME_NOT_FILENAME_SAFE,
                    path.clone(),
                    format!(
                        "the {kind} {name:?} {why}; the solvers use it as a file or folder name"
                    ),
                ));
            }
        }
    }
    for (kind, list) in groups {
        let mut first: HashMap<String, &str> = HashMap::new();
        for (path, name) in list.iter() {
            let key = collision_key(name);
            if let Some(earlier) = first.get(&key) {
                out.push(issue(
                    NAME_DUPLICATE,
                    path.clone(),
                    format!(
                        "the {kind} '{name}' is the same file name as '{earlier}' (compared \
                         without case): the solvers would overwrite or rename its results"
                    ),
                ));
            } else {
                first.insert(key, name);
            }
        }
    }
}

fn surface_receivers(p: &Project, out: &mut Vec<Issue>) {
    let mut empty = Vec::new();
    let mut planes = Vec::new();
    for (i, r) in p.surface_receivers.iter().enumerate() {
        if !r.enabled {
            continue;
        }
        match &r.shape {
            SurfaceReceiverShape::Scene { groups } => {
                let faces = p
                    .geometry
                    .faces
                    .iter()
                    .filter(|f| groups.contains(&f.group))
                    .count();
                if faces == 0 {
                    empty.push(issue(
                        SURFACE_RECEIVER_EMPTY,
                        format!("/surface_receivers/{i}/shape/groups"),
                        format!(
                            "surface receiver '{}' covers no model face: when it is the first \
                             receiver, no Global surface-receiver file is written at all",
                            r.name
                        ),
                    ));
                }
            }
            SurfaceReceiverShape::CuttingPlane {
                a,
                b,
                c,
                resolution_m,
            } => {
                if let Some(why) = cutting_plane_problem(*a, *b, *c, resolution_m.get()) {
                    planes.push(issue(
                        CUTTING_PLANE_INVALID,
                        format!("/surface_receivers/{i}/shape"),
                        format!("cutting plane '{}': {why}", r.name),
                    ));
                }
            }
        }
    }
    out.append(&mut empty);
    out.append(&mut planes);
}

fn cutting_plane_problem(a: Vec3, b: Vec3, c: Vec3, res: f64) -> Option<String> {
    let [a, b, c] = [a, b, c].map(Vec3::to_array);
    if ![a, b, c].iter().flatten().all(|&x| finite(x)) {
        return Some("a corner is not finite".to_string());
    }
    let ba = [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
    let bc = [c[0] - b[0], c[1] - b[1], c[2] - b[2]];
    let (lba, lbc) = (norm(ba), norm(bc));
    if norm(cross(ba, bc)) <= 1e-12 * lba * lbc || lba == 0.0 || lbc == 0.0 {
        return Some("corners A, B and C are collinear, so the grid has no area".to_string());
    }
    if !positive(res) {
        return Some(format!(
            "the resolution is {res} m; it must be above 0 (the cell count divides by it)"
        ));
    }
    if res > lba || res > lbc {
        return Some(format!(
            "the resolution {res} m is larger than a side (|BA| = {lba} m, |BC| = {lbc} m)"
        ));
    }
    let cells = (lba / res).ceil().max((lbc / res).ceil());
    if cells > f64::from(u32::MAX) {
        return Some(format!(
            "{cells} cells along one side do not fit the solver's unsigned cell count"
        ));
    }
    None
}

fn fittings(p: &Project, out: &mut Vec<Issue>) {
    for (i, z) in p.fitting_zones.iter().enumerate() {
        if !z.enabled {
            continue;
        }
        if let Some((j, more)) = first_bad(&z.absorption, |a| !unit(a)) {
            out.push(issue(
                FITTING_PARAMETERS_INVALID,
                format!("/fitting_zones/{i}/absorption/{j}"),
                format!(
                    "fitting zone '{}' absorption is {} at {}{}; it must be a finite value from \
                     0 to 1",
                    z.name,
                    z.absorption[j],
                    band_label(p, j),
                    and_more(more)
                ),
            ));
        }
        if let Some((j, more)) = first_bad(&z.mean_free_path_m, |l| !positive(l)) {
            out.push(issue(
                FITTING_PARAMETERS_INVALID,
                format!("/fitting_zones/{i}/mean_free_path_m/{j}"),
                format!(
                    "fitting zone '{}' mean free path is {} m at {}{}; it must be above 0 and \
                     finite",
                    z.name,
                    z.mean_free_path_m[j],
                    band_label(p, j),
                    and_more(more)
                ),
            ));
        }
    }
}

fn mesh(p: &Project, ctx: &Context, out: &mut Vec<Issue>) {
    if let Some(stamp) = &ctx.mesh_input_hash {
        let now = mesh_input_hash(p);
        if *stamp != now {
            out.push(issue(
                MESH_OUT_OF_DATE,
                "/geometry",
                format!(
                    "the tetrahedral mesh was built from mesher inputs {stamp}, but the project's \
                     are now {now}: its face markers would index the wrong faces"
                ),
            ));
        }
    }
}

/// `mesh_settings_conflict`: a facet-area constraint (the `.var`) and `-Y` together. The `.var`
/// asks TetGen to split the receiver faces and `-Y` forbids splitting any boundary facet
/// (`docs/m5-m6-design.md`, decision 3; `docs/formats/var.md`).
fn mesh_settings(p: &Project, out: &mut Vec<Issue>) {
    let m = &p.solvers.meshing;
    if crate::mesh::settings_conflict(m)
        && let Some(area) = m.surface_receiver_max_area_m2
    {
        out.push(issue(
            MESH_SETTINGS_CONFLICT,
            "/solvers/meshing/preserve_boundary",
            format!(
                "the mesh settings ask TetGen to refine surface-receiver faces to at most {area} \
                 m² (the .var) and keep -Y (preserve_boundary) on, which forbids splitting any \
                 boundary face. Upstream's GUI turns -Y off whenever the constraint is on"
            ),
        ));
    }
}
