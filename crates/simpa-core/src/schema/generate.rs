//! [`generate`]: small valid random projects, for round-trip, undo and validator tests.

use std::collections::BTreeMap;

use uuid::{Builder, Uuid};

use super::bands::{BandKind, BandSet, Spectrum, SpectrumShape};
use super::ids::{
    FittingZoneId, GroupId, MaterialId, PointReceiverId, ProjectId, SourceId, SurfaceReceiverId,
    VariantId,
};
use super::model::*;
use super::real::{F64, Vec3};

/// A small valid project, different per seed and identical for the same seed.
///
/// The room is an axis-aligned box, 2 to 25 m a side, its walls cut into a grid of 1 to 2 cells
/// per axis, with outward-facing triangles; when the box sits at the origin some zero
/// coordinates are -0.0, as in upstream's `cube.cbin`. Floats mix full 53-bit precision with
/// short decimals. Every structural invariant holds ([`Project::check_integrity`]), and so does
/// every value rule listed for `core::validate` in the plan:
/// - one band set (1 to 6 octave or 1 to 12 third-octave bands) and every array sized to it;
/// - sources and point receivers strictly inside the box, at least 0.6 m from every wall and
///   0.3 m outside every fitting zone; receiver radius 0.05 to 0.5 m;
/// - no directivity files (no balloon sources), no zero direction vectors;
/// - absorption and scattering in [0, 1], scattering 0 wherever absorption is 1, transmission
///   only in bands whose absorption is above 0 and with tau <= alpha;
/// - reflection laws one per material or one per band, and box fitting zones with or without
///   upstream's corner order (`FittingShape::Box::destination`);
/// - time step 0.5 to 20 ms, duration 0.1 to 3 s (at most 6,000 steps), extinction exponent 1
///   to 10, random seed within a C `int`, at least one band computed per solver;
/// - names ASCII, under 50 bytes and unique across the project;
/// - pinned material solver ids unique; one seed in three pins the other entities' ids too
///   (unique within each kind, a fitting zone never 0), and puts every source but the first in
///   a source group; a surface group in at most one scene receiver;
/// - no surface-receiver area constraint together with `-Y` (`preserve_boundary`).
pub fn generate(seed: u64) -> Project {
    let mut r = Rng::new(seed);
    // One seed in three pins every entity's solver id, as a `.proj` import does; taken from the
    // seed, not the generator, so the rest of the project is what it was before pins existed.
    let pinned = seed.is_multiple_of(3);
    let bands = random_bands(&mut r);
    let n = bands.len();

    // Room.
    let at_origin = r.below(4) == 0;
    let origin: [f64; 3] = if at_origin {
        [0.0; 3]
    } else {
        [0, 1, 2].map(|_| r.value(-50.0, 50.0))
    };
    let size: [f64; 3] = [0, 1, 2].map(|_| r.value(2.0, 25.0));
    let cells: [usize; 3] = [0, 1, 2].map(|_| 1 + r.below(2) as usize);

    // Materials.
    let n_materials = 1 + r.below(4) as usize;
    let materials: Vec<Material> = (0..n_materials)
        .map(|i| random_material(&mut r, i, n))
        .collect();

    // Surface groups: every one of the 6 walls in a group, every group with at least one wall.
    let n_groups = 1 + r.below(6) as usize;
    let surface_groups: Vec<SurfaceGroup> = (0..n_groups)
        .map(|i| SurfaceGroup {
            id: GroupId(r.uuid()),
            name: format!("Surface {}", i + 1),
            material: materials[r.below(n_materials as u64) as usize].id,
        })
        .collect();
    let mut walls: Vec<usize> = (0..6).collect();
    r.shuffle(&mut walls);
    let mut wall_group = [0usize; 6];
    for (k, &w) in walls.iter().enumerate() {
        wall_group[w] = if k < n_groups {
            k
        } else {
            r.below(n_groups as u64) as usize
        };
    }
    let geometry = box_geometry(
        &mut r,
        origin,
        size,
        cells,
        at_origin,
        wall_group.map(|g| surface_groups[g].id),
    );

    // Fitting zone: a box inside the room.
    let mut fitting_zones = Vec::new();
    let mut fitting_box: Option<([f64; 3], [f64; 3])> = None;
    if r.below(3) == 0 {
        // At most 0.4 of the room on each axis, so a point 0.6 m from the walls and 0.3 m clear
        // of the box always exists (the room is at least 2 m a side).
        let lo: [f64; 3] = [0, 1, 2].map(|a| origin[a] + size[a] * r.value(0.1, 0.2));
        let hi: [f64; 3] = [0, 1, 2].map(|a| lo[a] + size[a] * r.value(0.1, 0.2));
        fitting_box = Some((lo, hi));
        fitting_zones.push(FittingZone {
            id: FittingZoneId(r.uuid()),
            name: "Fitting 1".to_string(),
            enabled: r.below(4) != 0,
            shape: FittingShape::Box {
                min: Vec3::from(lo),
                max: Vec3::from(hi),
                destination: (r.below(3) == 0).then(|| {
                    [0, 1, 2].map(|_| {
                        if r.bool() {
                            BoxBound::Max
                        } else {
                            BoxBound::Min
                        }
                    })
                }),
            },
            absorption: (0..n).map(|_| F64::new(r.value(0.0, 1.0))).collect(),
            mean_free_path_m: (0..n).map(|_| F64::new(r.value(0.5, 5.0))).collect(),
            diffusion_law: (0..n)
                .map(|_| DiffusionLaw::ALL[r.below(3) as usize])
                .collect(),
            solver_id: pinned.then_some(2083),
        });
    }

    // A point strictly inside the room, `margin` from every wall and 0.3 m clear of the fitting.
    let inside = |r: &mut Rng, margin: f64| -> Vec3 {
        loop {
            let p: [f64; 3] =
                [0, 1, 2].map(|a| origin[a] + margin + r.unit() * (size[a] - 2.0 * margin));
            let clear = fitting_box
                .is_none_or(|(lo, hi)| (0..3).any(|a| p[a] < lo[a] - 0.3 || p[a] > hi[a] + 0.3));
            if clear {
                return Vec3::from(p);
            }
        }
    };

    let sources: Vec<Source> = (0..1 + r.below(3) as usize)
        .map(|i| {
            let directivity = match r.below(6) {
                0 => Directivity::Unidirectional {
                    direction: r.direction(),
                },
                1 => Directivity::PlaneXy,
                2 => Directivity::PlaneYz,
                3 => Directivity::PlaneXz,
                _ => Directivity::Omni,
            };
            Source {
                id: SourceId(r.uuid()),
                name: format!("Source {}", i + 1),
                enabled: i == 0 || r.below(3) != 0,
                position: inside(&mut r, 0.6),
                power: random_spectrum(&mut r, 70.0, 120.0, n),
                directivity,
                // Below 0.05 s: durations are >= 0.1 s and steps <= 0.02 s, so emission always
                // starts at least two steps before the run ends (rule source_delay_invalid).
                delay_s: if r.below(2) == 0 {
                    F64::ZERO
                } else {
                    F64::new(r.value(0.0, 0.05))
                },
                group: (pinned && i > 0).then(|| "Group 1".to_string()),
                solver_id: pinned.then_some(974 + i as u32),
            }
        })
        .collect();

    let point_receivers: Vec<PointReceiver> = (0..r.below(4) as usize)
        .map(|i| PointReceiver {
            id: PointReceiverId(r.uuid()),
            name: format!("Receiver {}", i + 1),
            position: inside(&mut r, 0.6),
            orientation: r.direction(),
            background_noise: (r.below(3) == 0).then(|| random_spectrum(&mut r, 0.0, 40.0, n)),
            solver_id: pinned.then_some(155 + i as u32),
        })
        .collect();

    let mut free_groups: Vec<GroupId> = surface_groups.iter().map(|g| g.id).collect();
    let surface_receivers: Vec<SurfaceReceiver> = (0..r.below(3) as usize)
        .map(|i| {
            let shape = if r.below(2) == 0 && !free_groups.is_empty() {
                r.shuffle(&mut free_groups);
                let take = 1 + r.below(free_groups.len() as u64) as usize;
                SurfaceReceiverShape::Scene {
                    groups: free_groups.drain(..take).collect(),
                }
            } else {
                let m = 0.1;
                let z = origin[2] + size[2] * r.value(0.2, 0.8);
                let (x0, x1) = (origin[0] + m, origin[0] + size[0] - m);
                let (y0, y1) = (origin[1] + m, origin[1] + size[1] - m);
                SurfaceReceiverShape::CuttingPlane {
                    a: Vec3::new(x0, y1, z),
                    b: Vec3::new(x0, y0, z),
                    c: Vec3::new(x1, y0, z),
                    resolution_m: F64::new(r.value(0.1, 1.0)),
                }
            };
            SurfaceReceiver {
                id: SurfaceReceiverId(r.uuid()),
                name: format!("Map {}", i + 1),
                enabled: r.below(4) != 0,
                shape,
                solver_id: pinned.then_some(951 + i as u32),
            }
        })
        .collect();

    let variants: Vec<Variant> = (0..r.below(3) as usize)
        .map(|i| {
            let mut v = Variant {
                id: VariantId(r.uuid()),
                name: format!("Variant {}", i + 1),
                overrides: Vec::new(),
            };
            for g in &surface_groups {
                if r.below(2) == 0 {
                    let m = materials[r.below(n_materials as u64) as usize].id;
                    v.set_override(g.id, Some(m));
                }
            }
            v
        })
        .collect();
    let active_variant = if variants.is_empty() || r.below(2) == 0 {
        None
    } else {
        Some(variants[r.below(variants.len() as u64) as usize].id)
    };

    let environment = Environment {
        temperature_c: F64::new(r.value(-10.0, 40.0)),
        relative_humidity_percent: F64::new(r.value(0.0, 100.0)),
        pressure_pa: F64::new(r.value(80_000.0, 110_000.0)),
        air_absorption: match r.below(3) {
            0 => AirAbsorption::UserDefined {
                value: F64::new(r.value(0.0, 0.05)),
                unit: AttenuationUnit::DecibelPerMetre,
            },
            1 => AirAbsorption::UserDefined {
                value: F64::new(r.value(0.0, 0.01)),
                unit: AttenuationUnit::PerMetre,
            },
            _ => AirAbsorption::Iso9613,
        },
        ground_roughness_m: F64::new(r.value(0.001, 1.0)),
        celerity_gradient_log: F64::new(if r.below(4) == 0 {
            r.value(-1.0, 1.0)
        } else {
            0.0
        }),
        celerity_gradient_lin: F64::new(if r.below(4) == 0 {
            r.value(-0.1, 0.1)
        } else {
            0.0
        }),
    };

    // Never more particles saved than emitted (the validator's particle_count_invalid).
    let particles_per_source = 1 + r.below(20_000) as u32;
    let particles_saved = r.below(21.min(u64::from(particles_per_source) + 1)) as u32;
    let solvers = SolverSettings {
        spps: SppsSettings {
            particles_per_source,
            particles_saved,
            duration_s: F64::new(r.value(0.1, 3.0)),
            time_step_s: F64::new(r.value(0.0005, 0.02)),
            random_seed: r.below(1 << 31) as u32,
            method: if r.below(2) == 0 {
                ComputationMethod::Random
            } else {
                ComputationMethod::Energetic
            },
            air_absorption: r.bool(),
            fittings: r.bool(),
            direct_field_only: r.below(5) == 0,
            transmission: r.bool(),
            extinction_exponent: F64::new(r.value(1.0, 10.0)),
            receiver_radius_m: F64::new(r.value(0.05, 0.5)),
            sound_map: if r.bool() {
                SoundMapQuantity::Intensity
            } else {
                SoundMapQuantity::Spl
            },
            sound_maps_per_band: r.bool(),
            echogram_per_source: r.bool(),
            save_surface_intersections: r.bool(),
            save_receiver_intersections: r.bool(),
            bands_computed: r.flags(n),
        },
        tcr: TcrSettings {
            air_absorption: r.bool(),
            bands_computed: r.flags(n),
        },
        meshing: {
            // The same draws in the same order as before; -Y is dropped where a facet-area
            // constraint is set, since -Y forbids the splits it asks for
            // (`mesh_settings_conflict`).
            let min_radius_edge_ratio = F64::new(r.value(1.1, 5.0));
            let max_volume_m3 = r.bool().then(|| F64::new(r.value(0.5, 100.0)));
            let surface_receiver_max_area_m2 = r.bool().then(|| F64::new(r.value(0.1, 5.0)));
            let preserve_boundary = r.bool() && surface_receiver_max_area_m2.is_none();
            MeshSettings {
                min_radius_edge_ratio,
                max_volume_m3,
                surface_receiver_max_area_m2,
                preserve_boundary,
                // Drawn last, below, so that every earlier draw stays as it was.
                preprocess: false,
            }
        },
    };

    let view = ViewState {
        camera: r.bool().then(|| Camera {
            eye: Vec3::from([0, 1, 2].map(|a| origin[a] + size[a] * r.value(-0.5, 1.5))),
            target: Vec3::from([0, 1, 2].map(|a| origin[a] + size[a] * 0.5)),
            vertical_fov_deg: F64::new(r.value(30.0, 90.0)),
        }),
    };

    let mut project = Project {
        format_version: FORMAT_VERSION,
        id: ProjectId(r.uuid()),
        name: format!("Generated {seed}"),
        description: if r.bool() {
            String::new()
        } else {
            format!("A {:.1} x {:.1} x {:.1} m box", size[0], size[1], size[2])
        },
        frame: Frame::default(),
        bands,
        geometry,
        surface_groups,
        materials,
        sources,
        point_receivers,
        surface_receivers,
        fitting_zones,
        environment,
        solvers,
        variants,
        active_variant,
        view,
    };
    project.solvers.meshing.preprocess = r.bool();
    project
}

fn random_bands(r: &mut Rng) -> BandSet {
    let (kind, max) = if r.bool() {
        (BandKind::Octave, 6)
    } else {
        (BandKind::ThirdOctave, 12)
    };
    let all = kind.nominal_frequencies();
    let count = 1 + r.below(max) as usize;
    let lo = r.below((all.len() - count + 1) as u64) as usize;
    BandSet {
        kind,
        frequencies_hz: all[lo..lo + count].to_vec(),
    }
}

fn random_spectrum(r: &mut Rng, lo: f64, hi: f64, n: usize) -> Spectrum {
    let shape = match r.below(3) {
        0 => SpectrumShape::Pink,
        1 => SpectrumShape::White,
        _ => SpectrumShape::Custom {
            relative_db: (0..n).map(|_| F64::new(r.value(-10.0, 10.0))).collect(),
        },
    };
    Spectrum::new(r.value(lo, hi), shape)
}

fn random_material(r: &mut Rng, index: usize, n: usize) -> Material {
    let absorption: Vec<f64> = (0..n)
        .map(|_| match r.below(8) {
            0 => 1.0,
            1 => 0.0,
            _ => r.value(0.0, 1.0),
        })
        .collect();
    let scattering: Vec<F64> = absorption
        .iter()
        .map(|&a| F64::new(if a == 1.0 { 0.0 } else { r.value(0.0, 1.0) }))
        .collect();
    // tau = 10^(-R/10) <= alpha  <=>  R >= -10 log10(alpha); keep a 0.5 dB margin. A band that
    // absorbs nothing does not transmit, and one transmitting material in four switches some
    // other bands off too, as upstream's per-band switch does (tutorial 3's `Open_door`).
    let transmission_loss_db = (r.below(4) == 0)
        .then(|| {
            let some_off = r.below(4) == 0;
            absorption
                .iter()
                .map(|&a| {
                    let on = a > 0.0 && !(some_off && r.below(2) == 0);
                    on.then(|| F64::new(-10.0 * a.log10() + r.value(0.5, 30.0)))
                })
                .collect::<Vec<_>>()
        })
        .filter(|t| t.iter().any(Option::is_some));
    // One law, or one per band (tutorial 3's material 100).
    let reflection_law = if r.below(4) == 0 {
        ReflectionLaws::from_bands(
            (0..n)
                .map(|_| ReflectionLaw::ALL[r.below(7) as usize])
                .collect(),
        )
    } else {
        ReflectionLaw::ALL[r.below(7) as usize].into()
    };
    Material {
        id: MaterialId(r.uuid()),
        name: format!("Material {}", index + 1),
        color: Rgb(r.next() as u8, r.next() as u8, r.next() as u8),
        absorption: absorption.into_iter().map(F64::new).collect(),
        scattering,
        reflection_law,
        transmission_loss_db,
        double_sided: r.bool(),
        solver_id: (r.below(3) == 0).then(|| index as u32 * 7 + r.below(7) as u32),
    }
}

/// The surface of an axis-aligned box: the boundary points of a `cells` grid, and two
/// outward-facing triangles per wall cell. `wall_groups` is indexed by `2 * axis + side`.
fn box_geometry(
    r: &mut Rng,
    origin: [f64; 3],
    size: [f64; 3],
    cells: [usize; 3],
    signed_zeros: bool,
    wall_groups: [GroupId; 6],
) -> Geometry {
    let mut index: BTreeMap<[usize; 3], u32> = BTreeMap::new();
    let mut vertices = Vec::new();
    for i in 0..=cells[0] {
        for j in 0..=cells[1] {
            for k in 0..=cells[2] {
                let g = [i, j, k];
                if !(0..3).any(|a| g[a] == 0 || g[a] == cells[a]) {
                    continue;
                }
                let p: [f64; 3] = [0, 1, 2].map(|a| {
                    let v = origin[a] + size[a] * (g[a] as f64 / cells[a] as f64);
                    if signed_zeros && v == 0.0 && r.bool() {
                        -0.0
                    } else {
                        v
                    }
                });
                index.insert(g, vertices.len() as u32);
                vertices.push(Vec3::from(p));
            }
        }
    }
    let mut faces = Vec::new();
    for axis in 0..3 {
        for side in 0..2 {
            // e_b x e_c = e_axis, so (b, c) faces +axis; swap for the min side.
            let (b, c) = ((axis + 1) % 3, (axis + 2) % 3);
            let (u, v) = if side == 1 { (b, c) } else { (c, b) };
            let group = wall_groups[2 * axis + side];
            for iu in 0..cells[u] {
                for iv in 0..cells[v] {
                    let corner = |du: usize, dv: usize| {
                        let mut g = [0usize; 3];
                        g[axis] = side * cells[axis];
                        g[u] = iu + du;
                        g[v] = iv + dv;
                        index[&g]
                    };
                    let (p00, p10, p11, p01) =
                        (corner(0, 0), corner(1, 0), corner(1, 1), corner(0, 1));
                    faces.push(Face {
                        vertices: [p00, p10, p11],
                        group,
                    });
                    faces.push(Face {
                        vertices: [p00, p11, p01],
                        group,
                    });
                }
            }
        }
    }
    Geometry { vertices, faces }
}

/// xorshift64* with a splitmix64-scrambled seed, so seeds 0, 1, 2... give unrelated streams.
struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self {
        let mut z = seed.wrapping_add(0x9e37_79b9_7f4a_7c15);
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^= z >> 31;
        Rng(if z == 0 { 0x2545_f491_4f6c_dd1d } else { z })
    }

    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_f491_4f6c_dd1d)
    }

    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }

    fn bool(&mut self) -> bool {
        self.next() & 1 == 1
    }

    /// Uniform in [0, 1), with all 53 bits random.
    fn unit(&mut self) -> f64 {
        (self.next() >> 11) as f64 * (1.0 / (1u64 << 53) as f64)
    }

    /// A value in [lo, hi]: at full precision, or rounded to two decimals (short decimals are
    /// what people type), clamped back into range.
    fn value(&mut self, lo: f64, hi: f64) -> f64 {
        let v = lo + self.unit() * (hi - lo);
        if self.below(3) == 0 {
            ((v * 100.0).round() / 100.0).clamp(lo, hi)
        } else {
            v
        }
    }

    /// A direction whose length is between 0.2 and sqrt(3).
    fn direction(&mut self) -> Vec3 {
        loop {
            let d = Vec3::from([0, 1, 2].map(|_| self.value(-1.0, 1.0)));
            if d.length() >= 0.2 {
                return d;
            }
        }
    }

    /// `n` flags, at least one set.
    fn flags(&mut self, n: usize) -> Vec<bool> {
        let mut f: Vec<bool> = (0..n).map(|_| self.below(4) != 0).collect();
        if !f.contains(&true) {
            let i = self.below(n as u64) as usize;
            f[i] = true;
        }
        f
    }

    fn shuffle<T>(&mut self, v: &mut [T]) {
        for i in (1..v.len()).rev() {
            let j = self.below(i as u64 + 1) as usize;
            v.swap(i, j);
        }
    }

    fn uuid(&mut self) -> Uuid {
        let mut bytes = [0u8; 16];
        bytes[..8].copy_from_slice(&self.next().to_le_bytes());
        bytes[8..].copy_from_slice(&self.next().to_le_bytes());
        Builder::from_random_bytes(bytes).into_uuid()
    }
}
