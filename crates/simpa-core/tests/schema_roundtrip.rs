//! Persistence, undo and fixtures for `simpa_core::schema`.
//!
//! Expected values come from upstream wherever there is one: `cube.simpa` is checked against
//! upstream's own `lib_interface/tests/cube.cbin` (read with the M2 reader), and the white-noise
//! spectrum against tutorial 1's GUI-written config.xml.

use std::collections::HashSet;
use std::path::PathBuf;

use serde_json::{Value, json};
use simpa_core::formats::cbin;
use simpa_core::schema::*;

const CASES: u64 = 10_000;

fn fixture(rel: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures")
        .join(rel)
}

/// The test's own randomness (xorshift64*, splitmix-seeded), independent of `generate`'s.
struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self {
        let mut z = seed ^ 0x005e_ed0f_7e57;
        z = z.wrapping_add(0x9e37_79b9_7f4a_7c15);
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^= z >> 31;
        Rng(if z == 0 { 1 } else { z })
    }

    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_f491_4f6c_dd1d)
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next() % n.max(1) as u64) as usize
    }

    fn chance(&mut self, one_in: usize) -> bool {
        self.below(one_in) == 0
    }

    fn unit(&mut self) -> f64 {
        (self.next() >> 11) as f64 / (1u64 << 53) as f64
    }

    fn f64(&mut self, lo: f64, hi: f64) -> F64 {
        F64::new(lo + self.unit() * (hi - lo))
    }

    /// A float whose bit pattern is an edge case of the encoding.
    fn special(&mut self) -> F64 {
        let sign = if self.next() & 1 == 1 { 1u64 << 63 } else { 0 };
        let bits = match self.below(14) {
            0 => 0x7ff8_0000_0000_0000, // canonical NaN
            1 => 0x7ff8_0000_0000_0000 | sign | (self.next() & 0x7_ffff_ffff_ffff), // quiet NaN
            2 => 0x7ff0_0000_0000_0000 | sign | (self.next() & 0x7_ffff_ffff_ffff).max(1), // signalling
            3 => f64::INFINITY.to_bits() | sign,
            4 => sign,                                         // +-0
            5 => sign | (self.next() & 0x000f_ffff_ffff_ffff), // subnormal
            6 => f64::MIN_POSITIVE.to_bits() | sign,
            7 => f64::MAX.to_bits() | sign,
            8 => f64::EPSILON.to_bits() | sign,
            9 => 1e23f64.to_bits() | sign,
            10 => (0.1f64 + 0.2).to_bits() | sign,
            11 => (2f64.powi(53).to_bits() + 1) | sign, // 2^53 + 2, 17 significant digits
            12 => 2.225_073_858_507_201e-308f64.to_bits() | sign,
            _ => self.next(), // any bit pattern
        };
        F64::from_bits(bits)
    }

    /// A string mixing escapes, controls, non-ASCII and astral characters.
    fn string(&mut self) -> String {
        const POOL: &[char] = &[
            'a', 'Z', ' ', '"', '\\', '/', '\n', '\t', '\u{0}', '\u{1f}', '\u{7f}', 'é', 'Ł', '中',
            '\u{2028}', '\u{feff}', '\u{fffd}', '😀', '%', '<', '&',
        ];
        (0..self.below(12))
            .map(|_| POOL[self.below(POOL.len())])
            .collect()
    }
}

/// Every float in a JSON tree that `F64` wrote: numbers with a fraction or exponent, and the
/// non-finite strings.
fn count_floats(v: &Value) -> usize {
    match v {
        Value::Number(n) => usize::from(n.is_f64()),
        Value::String(s) => usize::from(
            s == "NaN" || s == "Infinity" || s == "-Infinity" || s.starts_with("NaN:0x"),
        ),
        Value::Array(a) => a.iter().map(count_floats).sum(),
        Value::Object(o) => o.values().map(count_floats).sum(),
        _ => 0,
    }
}

#[test]
fn generated_projects_round_trip_byte_identical_and_bit_exact() {
    let mut texts = HashSet::new();
    let mut floats = 0;
    let mut serde_json_misreads = 0;
    for seed in 0..CASES {
        let p = generate(seed);
        p.check_integrity()
            .unwrap_or_else(|e| panic!("seed {seed}: {e}"));
        let a = to_json(&p);
        let q = from_json(&a).unwrap_or_else(|e| panic!("seed {seed}: {e}"));
        // `F64` compares by bits, so this is every float bit-equal, and everything else equal.
        assert!(q == p, "seed {seed}: loaded project differs");
        let b = to_json(&q);
        assert_eq!(a, b, "seed {seed}: re-save differs");
        floats += count_floats(&parse_json(&a).unwrap());
        // What loading with serde_json's own reader would have done.
        let lossy: Project = serde_json::from_str(&a).unwrap();
        serde_json_misreads += usize::from(lossy != p);
        texts.insert(a);
    }
    assert_eq!(
        texts.len(),
        CASES as usize,
        "some seeds gave the same project"
    );
    assert!(generate(7) == generate(7), "generate is not deterministic");
    println!(
        "{CASES} projects, {floats} floats: every one bit-equal after load, every re-save \
         byte-identical; serde_json::from_str misreads {serde_json_misreads} of the projects"
    );
}

#[test]
fn save_and_load_files_byte_identical() {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("schema_roundtrip");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(format!("generated.{FILE_EXTENSION}"));
    for seed in 0..CASES {
        let p = generate(seed);
        save(&p, &path).unwrap();
        let bytes = std::fs::read(&path).unwrap();
        assert_eq!(bytes, to_json(&p).as_bytes());
        let q = load(&path).unwrap();
        assert!(q == p, "seed {seed}");
        save(&q, &path).unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), bytes, "seed {seed}");
    }
}

/// Another process holding the project file open without delete sharing (an antivirus scan,
/// the indexer, OneDrive) blocks the rename over it. `save` waits that out, and when the hold
/// outlasts its retries it fails with the old file untouched and the new text in `<path>.tmp`.
#[cfg(windows)]
#[test]
fn save_waits_out_a_file_held_open_by_another_process() {
    use std::fs::OpenOptions;
    use std::os::windows::fs::OpenOptionsExt;
    use std::time::{Duration, Instant};
    // FILE_SHARE_READ without FILE_SHARE_DELETE, as a scanner opens a file.
    const SHARE_READ_ONLY: u32 = 0x1;
    let hold = |path: &PathBuf| {
        OpenOptions::new()
            .read(true)
            .share_mode(SHARE_READ_ONLY)
            .open(path)
            .unwrap()
    };

    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("schema_roundtrip");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("held.simpa");
    let (old, new) = (generate(1), generate(2));
    save(&old, &path).unwrap();

    // The hold does block a plain rename, or this test would prove nothing.
    let holder = hold(&path);
    let probe = dir.join("held.probe");
    std::fs::write(&probe, b"probe").unwrap();
    let blocked = std::fs::rename(&probe, &path).unwrap_err();
    println!("a plain rename over the held file fails: {blocked}");
    assert_eq!(std::fs::read(&path).unwrap(), to_json(&old).as_bytes());

    // Released after 150 ms: save succeeds once it is.
    let release = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(150));
        drop(holder);
    });
    let started = Instant::now();
    save(&new, &path).unwrap();
    let brief = started.elapsed();
    release.join().unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), to_json(&new).as_bytes());
    assert!(
        brief >= Duration::from_millis(100),
        "save returned after {brief:?}, before the hold was released"
    );

    // Held throughout: save fails after its retries, the file keeps the last saved project.
    let holder = hold(&path);
    let started = Instant::now();
    let err = save(&old, &path).unwrap_err();
    let waited = started.elapsed();
    drop(holder);
    let retries = Duration::from_millis(RETRY_DELAYS_MS.iter().sum());
    assert!(waited >= retries, "gave up after {waited:?}");
    assert_eq!(std::fs::read(&path).unwrap(), to_json(&new).as_bytes());
    assert_eq!(
        std::fs::read(dir.join("held.simpa.tmp")).unwrap(),
        to_json(&old).as_bytes()
    );
    println!(
        "save waited {brief:?} for a 150 ms hold; on a permanent hold it gave up after \
         {waited:?}: {err}"
    );
}

/// Replaces floats and names in a project's JSON tree with edge cases.
fn mutate(v: &mut Value, rng: &mut Rng, key: Option<&str>, replaced: &mut usize) {
    match v {
        Value::Number(n) if n.is_f64() => {
            if rng.chance(2) {
                *v = serde_json::to_value(rng.special()).unwrap();
                *replaced += 1;
            }
        }
        Value::String(_) if matches!(key, Some("name" | "description")) => {
            *v = Value::String(rng.string());
        }
        Value::Array(a) => a.iter_mut().for_each(|x| mutate(x, rng, None, replaced)),
        Value::Object(o) => o
            .iter_mut()
            .for_each(|(k, x)| mutate(x, rng, Some(k.as_str()), replaced)),
        _ => {}
    }
}

#[test]
fn edge_case_floats_and_strings_round_trip_bit_exact() {
    let mut rng = Rng::new(1);
    let mut replaced = 0;
    let mut non_finite = 0;
    for seed in 0..CASES {
        let mut tree = parse_json(&to_json(&generate(seed))).unwrap();
        mutate(&mut tree, &mut rng, None, &mut replaced);
        let p: Project = serde_json::from_value(tree).unwrap();
        let a = to_json(&p);
        non_finite += a.matches("\"NaN").count() + a.matches("Infinity\"").count();
        let q = from_json(&a).unwrap_or_else(|e| panic!("seed {seed}: {e}"));
        assert!(q == p, "seed {seed}: loaded project differs");
        assert_eq!(to_json(&q), a, "seed {seed}: re-save differs");
    }
    assert!(non_finite > 0);
    println!(
        "{CASES} projects, {replaced} floats replaced by edge cases ({non_finite} non-finite \
         in the files): all bit-equal after load"
    );
}

#[test]
fn every_f64_bit_pattern_class_round_trips() {
    let mut rng = Rng::new(2);
    let mut cases: Vec<u64> = vec![
        0,
        1u64 << 63,
        1,
        0x000f_ffff_ffff_ffff,
        0x0010_0000_0000_0000,
        f64::MAX.to_bits(),
        f64::INFINITY.to_bits(),
        f64::NEG_INFINITY.to_bits(),
        0x7ff8_0000_0000_0000,
        0xfff8_0000_0000_0000,
        0x7ff0_0000_0000_0001,
        0x7fff_ffff_ffff_ffff,
        1e23f64.to_bits(),
        0.3f64.to_bits(),
        (0.1f64 + 0.2).to_bits(),
        2.225_073_858_507_201e-308f64.to_bits(),
        2f64.powi(53).to_bits(),
        (2f64.powi(53) + 2.0).to_bits(),
    ];
    cases.extend((0..400_000).map(|_| rng.next()));
    let mut serde_json_misreads = 0;
    for &bits in &cases {
        let text = serde_json::to_string(&F64::from_bits(bits)).unwrap();
        let back: F64 = serde_json::from_value(parse_json(&text).unwrap()).unwrap();
        assert_eq!(back.to_bits(), bits, "{text}");
        let v = f64::from_bits(bits);
        if v.is_finite() {
            let lossy: f64 = serde_json::from_str(&text).unwrap();
            serde_json_misreads += usize::from(lossy.to_bits() != bits);
        }
    }
    println!(
        "{} bit patterns round-trip exactly; serde_json::from_str misreads {serde_json_misreads}",
        cases.len()
    );
    assert!(
        serde_json_misreads > 0,
        "serde_json's reader is exact here: its float_roundtrip feature may be on, and the own \
         reader could be reconsidered"
    );
}

// ---------------------------------------------------------------------------------------------
// Ops

fn pick<T: Clone>(rng: &mut Rng, list: &[T]) -> Option<T> {
    (!list.is_empty()).then(|| list[rng.below(list.len())].clone())
}

fn values(rng: &mut Rng, n: usize, lo: f64, hi: f64) -> Vec<F64> {
    (0..n)
        .map(|_| {
            if rng.chance(20) {
                rng.special()
            } else {
                rng.f64(lo, hi)
            }
        })
        .collect()
}

fn vec3(rng: &mut Rng) -> Vec3 {
    Vec3 {
        x: rng.f64(-10.0, 10.0),
        y: rng.f64(-10.0, 10.0),
        z: rng.f64(-10.0, 10.0),
    }
}

/// Usually `n`; one time in 15 a wrong band count, so the op is refused.
fn count(rng: &mut Rng, n: usize) -> usize {
    if rng.chance(15) {
        n + 1 + rng.below(2)
    } else {
        n
    }
}

fn uuid(rng: &mut Rng) -> uuid::Uuid {
    uuid::Uuid::from_u64_pair(rng.next(), rng.next())
}

fn spectrum(rng: &mut Rng, n: usize) -> Spectrum {
    let shape = match rng.below(3) {
        0 => SpectrumShape::Pink,
        1 => SpectrumShape::White,
        _ => {
            let k = count(rng, n);
            SpectrumShape::Custom {
                relative_db: values(rng, k, -10.0, 10.0),
            }
        }
    };
    Spectrum {
        global_db: rng.f64(0.0, 120.0),
        shape,
    }
}

fn material(rng: &mut Rng, p: &Project) -> Material {
    let n = p.bands.len();
    let (ka, ks, kt) = (count(rng, n), count(rng, n), count(rng, n));
    Material {
        id: MaterialId(uuid(rng)),
        name: rng.string(),
        color: Rgb(rng.next() as u8, rng.next() as u8, rng.next() as u8),
        absorption: values(rng, ka, 0.0, 1.0),
        scattering: values(rng, ks, 0.0, 1.0),
        // One law, or one per band (possibly the wrong count, which the ops must refuse).
        reflection_law: if rng.chance(3) {
            let kl = count(rng, n);
            ReflectionLaws::PerBand((0..kl).map(|_| ReflectionLaw::ALL[rng.below(7)]).collect())
        } else {
            ReflectionLaw::ALL[rng.below(7)].into()
        },
        // Per band on or off.
        transmission_loss_db: rng.chance(2).then(|| {
            values(rng, kt, 0.0, 60.0)
                .into_iter()
                .map(|v| (!rng.chance(4)).then_some(v))
                .collect()
        }),
        double_sided: rng.chance(2),
        solver_id: rng.chance(3).then(|| rng.below(40) as u32),
    }
}

fn source(rng: &mut Rng, p: &Project) -> Source {
    let n = p.bands.len();
    Source {
        id: SourceId(uuid(rng)),
        name: rng.string(),
        enabled: rng.chance(2),
        position: vec3(rng),
        power: spectrum(rng, n),
        directivity: match rng.below(4) {
            0 => Directivity::Omni,
            1 => Directivity::PlaneXz,
            2 => Directivity::Unidirectional {
                direction: vec3(rng),
            },
            _ => Directivity::Balloon {
                file: rng.string(),
                direction: vec3(rng),
            },
        },
        delay_s: rng.f64(0.0, 1.0),
    }
}

fn point_receiver(rng: &mut Rng, p: &Project) -> PointReceiver {
    let n = p.bands.len();
    PointReceiver {
        id: PointReceiverId(uuid(rng)),
        name: rng.string(),
        position: vec3(rng),
        orientation: vec3(rng),
        background_noise: rng.chance(2).then(|| spectrum(rng, n)),
    }
}

/// Group ids for a reference list: usually existing ones, sometimes a missing id or a repeat.
fn group_list(rng: &mut Rng, p: &Project) -> Vec<GroupId> {
    let mut groups: Vec<GroupId> = p.surface_groups.iter().map(|g| g.id).collect();
    groups.truncate(1 + rng.below(3));
    if rng.chance(15) {
        groups.push(GroupId(uuid(rng)));
    }
    if rng.chance(15)
        && let Some(&g) = groups.first()
    {
        groups.push(g);
    }
    groups
}

fn surface_receiver(rng: &mut Rng, p: &Project) -> SurfaceReceiver {
    SurfaceReceiver {
        id: SurfaceReceiverId(uuid(rng)),
        name: rng.string(),
        enabled: rng.chance(2),
        shape: if rng.chance(2) {
            SurfaceReceiverShape::Scene {
                groups: group_list(rng, p),
            }
        } else {
            SurfaceReceiverShape::CuttingPlane {
                a: vec3(rng),
                b: vec3(rng),
                c: vec3(rng),
                resolution_m: rng.f64(0.05, 2.0),
            }
        },
    }
}

fn fitting_zone(rng: &mut Rng, p: &Project) -> FittingZone {
    let n = p.bands.len();
    let (ka, km, kl) = (count(rng, n), count(rng, n), count(rng, n));
    FittingZone {
        id: FittingZoneId(uuid(rng)),
        name: rng.string(),
        enabled: rng.chance(2),
        shape: if rng.chance(2) {
            FittingShape::Box {
                min: vec3(rng),
                max: vec3(rng),
                destination: rng.chance(2).then(|| {
                    [0, 1, 2].map(|_| {
                        if rng.chance(2) {
                            BoxBound::Min
                        } else {
                            BoxBound::Max
                        }
                    })
                }),
            }
        } else {
            FittingShape::Surfaces {
                groups: group_list(rng, p),
                inside_point: vec3(rng),
            }
        },
        absorption: values(rng, ka, 0.0, 1.0),
        mean_free_path_m: values(rng, km, 0.1, 10.0),
        diffusion_law: (0..kl).map(|_| DiffusionLaw::ALL[rng.below(3)]).collect(),
    }
}

fn variant(rng: &mut Rng, p: &Project) -> Variant {
    let mut v = Variant {
        id: VariantId(uuid(rng)),
        name: rng.string(),
        overrides: Vec::new(),
    };
    for g in &p.surface_groups {
        if rng.chance(2)
            && let Some(m) = pick(rng, &p.materials)
        {
            v.set_override(g.id, Some(m.id));
        }
    }
    if rng.chance(15) {
        // Out of order: refused.
        v.overrides.reverse();
        v.overrides.push(MaterialOverride {
            group: GroupId(uuid(rng)),
            material: MaterialId(uuid(rng)),
        });
    }
    v
}

/// An id that exists, or (one time in 10) one that does not.
fn some_id<T: Clone, I>(
    rng: &mut Rng,
    list: &[T],
    id: impl Fn(&T) -> I,
    make: impl Fn(uuid::Uuid) -> I,
) -> I {
    match pick(rng, list) {
        Some(x) if !rng.chance(10) => id(&x),
        _ => make(uuid(rng)),
    }
}

/// An insertion index: usually valid, sometimes past the end.
fn index(rng: &mut Rng, len: usize) -> usize {
    if rng.chance(15) {
        len + 1
    } else {
        rng.below(len + 1)
    }
}

fn band(rng: &mut Rng, n: usize) -> usize {
    if rng.chance(15) { n } else { rng.below(n) }
}

fn random_op(rng: &mut Rng, p: &Project, depth: usize) -> Op {
    let n = p.bands.len();
    let group_id = |rng: &mut Rng| some_id(rng, &p.surface_groups, |g| g.id, GroupId);
    let material_id = |rng: &mut Rng| some_id(rng, &p.materials, |m| m.id, MaterialId);
    match rng.below(if depth > 0 { 36 } else { 37 }) {
        0 => Op::SetProjectName { name: rng.string() },
        1 => Op::SetDescription {
            description: rng.string(),
        },
        2 => {
            let target = match rng.below(7) {
                0 => EntityRef::SurfaceGroup(group_id(rng)),
                1 => EntityRef::Material(material_id(rng)),
                2 => EntityRef::Source(some_id(rng, &p.sources, |s| s.id, SourceId)),
                3 => EntityRef::PointReceiver(some_id(
                    rng,
                    &p.point_receivers,
                    |r| r.id,
                    PointReceiverId,
                )),
                4 => EntityRef::SurfaceReceiver(some_id(
                    rng,
                    &p.surface_receivers,
                    |r| r.id,
                    SurfaceReceiverId,
                )),
                5 => {
                    EntityRef::FittingZone(some_id(rng, &p.fitting_zones, |z| z.id, FittingZoneId))
                }
                _ => EntityRef::Variant(some_id(rng, &p.variants, |v| v.id, VariantId)),
            };
            Op::Rename {
                target,
                name: rng.string(),
            }
        }
        3 => {
            let kind = if rng.chance(2) {
                BandKind::Octave
            } else {
                BandKind::ThirdOctave
            };
            let all = kind.nominal_frequencies();
            let lo = rng.below(all.len());
            let hi = lo + rng.below((all.len() - lo).min(8));
            let bands = BandSet::range(kind, all[lo], all[hi]).unwrap();
            let mut op = p.rebanded(bands).unwrap();
            if let Op::SetBands { data, .. } = &mut op {
                match rng.below(20) {
                    0 => data.spps_bands_computed.push(true),
                    1 => data.materials.reverse(),
                    2 => {
                        data.source_shapes.pop();
                    }
                    3 => data
                        .background_noise_shapes
                        .iter_mut()
                        .for_each(|(_, s)| *s = if s.is_some() { None } else { Some(Vec::new()) }),
                    _ => {}
                }
            }
            op
        }
        4 => {
            let mut geometry = p.geometry.clone();
            match rng.below(4) {
                0 if !geometry.vertices.is_empty() => {
                    let i = rng.below(geometry.vertices.len());
                    geometry.vertices[i] = vec3(rng);
                }
                1 => {
                    geometry.faces.pop();
                }
                2 => geometry.faces.push(Face {
                    vertices: [0, 1, rng.below(geometry.vertices.len() + 2) as u32],
                    group: group_id(rng),
                }),
                _ => geometry.vertices.push(vec3(rng)),
            }
            Op::SetGeometry { geometry }
        }
        5 => Op::AddSurfaceGroup {
            index: index(rng, p.surface_groups.len()),
            group: SurfaceGroup {
                id: GroupId(uuid(rng)),
                name: rng.string(),
                material: material_id(rng),
            },
        },
        6 => Op::RemoveSurfaceGroup { id: group_id(rng) },
        7 => Op::SetGroupMaterial {
            group: group_id(rng),
            material: material_id(rng),
        },
        8 => Op::AddMaterial {
            index: index(rng, p.materials.len()),
            material: material(rng, p),
        },
        9 => Op::RemoveMaterial {
            id: material_id(rng),
        },
        10 => {
            let mut m = material(rng, p);
            m.id = material_id(rng);
            Op::ReplaceMaterial { material: m }
        }
        11 => Op::SetMaterialBand {
            material: material_id(rng),
            quantity: [
                MaterialQuantity::Absorption,
                MaterialQuantity::Scattering,
                MaterialQuantity::TransmissionLoss,
            ][rng.below(3)],
            band: band(rng, n),
            value: if rng.chance(4) {
                rng.special()
            } else {
                rng.f64(0.0, 1.0)
            },
        },
        12 => Op::AddSource {
            index: index(rng, p.sources.len()),
            source: source(rng, p),
        },
        13 => Op::RemoveSource {
            id: some_id(rng, &p.sources, |s| s.id, SourceId),
        },
        14 => {
            let mut s = source(rng, p);
            s.id = some_id(rng, &p.sources, |s| s.id, SourceId);
            Op::ReplaceSource { source: s }
        }
        15 => Op::MoveSource {
            id: some_id(rng, &p.sources, |s| s.id, SourceId),
            position: vec3(rng),
        },
        16 => Op::SetSourceEnabled {
            id: some_id(rng, &p.sources, |s| s.id, SourceId),
            enabled: rng.chance(2),
        },
        17 => Op::AddPointReceiver {
            index: index(rng, p.point_receivers.len()),
            receiver: point_receiver(rng, p),
        },
        18 => Op::RemovePointReceiver {
            id: some_id(rng, &p.point_receivers, |r| r.id, PointReceiverId),
        },
        19 => {
            let mut r = point_receiver(rng, p);
            r.id = some_id(rng, &p.point_receivers, |r| r.id, PointReceiverId);
            Op::ReplacePointReceiver { receiver: r }
        }
        20 => Op::MovePointReceiver {
            id: some_id(rng, &p.point_receivers, |r| r.id, PointReceiverId),
            position: vec3(rng),
        },
        21 => Op::AddSurfaceReceiver {
            index: index(rng, p.surface_receivers.len()),
            receiver: surface_receiver(rng, p),
        },
        22 => Op::RemoveSurfaceReceiver {
            id: some_id(rng, &p.surface_receivers, |r| r.id, SurfaceReceiverId),
        },
        23 => {
            let mut r = surface_receiver(rng, p);
            r.id = some_id(rng, &p.surface_receivers, |r| r.id, SurfaceReceiverId);
            Op::ReplaceSurfaceReceiver { receiver: r }
        }
        24 => Op::AddFittingZone {
            index: index(rng, p.fitting_zones.len()),
            zone: fitting_zone(rng, p),
        },
        25 => Op::RemoveFittingZone {
            id: some_id(rng, &p.fitting_zones, |z| z.id, FittingZoneId),
        },
        26 => {
            let mut z = fitting_zone(rng, p);
            z.id = some_id(rng, &p.fitting_zones, |z| z.id, FittingZoneId);
            Op::ReplaceFittingZone { zone: z }
        }
        27 => Op::SetFittingBand {
            zone: some_id(rng, &p.fitting_zones, |z| z.id, FittingZoneId),
            quantity: if rng.chance(2) {
                FittingQuantity::Absorption
            } else {
                FittingQuantity::MeanFreePath
            },
            band: band(rng, n),
            value: rng.f64(0.0, 5.0),
        },
        28 => {
            let mut environment = p.environment.clone();
            environment.temperature_c = rng.f64(-20.0, 40.0);
            environment.air_absorption = if rng.chance(2) {
                AirAbsorption::Iso9613
            } else {
                AirAbsorption::UserDefined {
                    value: rng.f64(0.0, 0.1),
                    unit: if rng.chance(2) {
                        AttenuationUnit::PerMetre
                    } else {
                        AttenuationUnit::DecibelPerMetre
                    },
                }
            };
            Op::SetEnvironment { environment }
        }
        29 => {
            let mut settings = p.solvers.clone();
            settings.spps.particles_per_source = rng.next() as u32;
            settings.spps.time_step_s = rng.special();
            settings.meshing.max_volume_m3 = rng.chance(2).then(|| rng.f64(0.1, 10.0));
            settings.tcr.bands_computed = (0..count(rng, n)).map(|_| rng.chance(2)).collect();
            Op::SetSolverSettings { settings }
        }
        30 => Op::SetBandComputed {
            solver: if rng.chance(2) {
                SolverKind::Spps
            } else {
                SolverKind::Tcr
            },
            band: band(rng, n),
            computed: rng.chance(2),
        },
        31 => Op::AddVariant {
            index: index(rng, p.variants.len()),
            variant: variant(rng, p),
        },
        32 => Op::RemoveVariant {
            id: some_id(rng, &p.variants, |v| v.id, VariantId),
        },
        33 => Op::SetVariantOverride {
            variant: some_id(rng, &p.variants, |v| v.id, VariantId),
            group: group_id(rng),
            material: (!rng.chance(3)).then(|| material_id(rng)),
        },
        34 => Op::SetActiveVariant {
            variant: (!rng.chance(3)).then(|| some_id(rng, &p.variants, |v| v.id, VariantId)),
        },
        35 => Op::SetCamera {
            camera: rng.chance(2).then(|| Camera {
                eye: vec3(rng),
                target: vec3(rng),
                vertical_fov_deg: rng.f64(10.0, 100.0),
            }),
        },
        _ => Op::Batch {
            ops: (0..rng.below(5))
                .map(|_| random_op(rng, p, depth + 1))
                .collect(),
        },
    }
}

/// One time in 15, gives an entity about to be added the id of an existing one (refused).
fn maybe_reuse_id(rng: &mut Rng, p: &Project, op: &mut Op) {
    if !rng.chance(15) {
        return;
    }
    match op {
        Op::AddSurfaceGroup { group, .. } => {
            group.id = pick(rng, &p.surface_groups).map_or(group.id, |g| g.id)
        }
        Op::AddMaterial { material, .. } => {
            material.id = pick(rng, &p.materials).map_or(material.id, |m| m.id)
        }
        Op::AddSource { source, .. } => {
            source.id = pick(rng, &p.sources).map_or(source.id, |s| s.id)
        }
        Op::AddPointReceiver { receiver, .. } => {
            receiver.id = pick(rng, &p.point_receivers).map_or(receiver.id, |r| r.id)
        }
        Op::AddSurfaceReceiver { receiver, .. } => {
            receiver.id = pick(rng, &p.surface_receivers).map_or(receiver.id, |r| r.id)
        }
        Op::AddFittingZone { zone, .. } => {
            zone.id = pick(rng, &p.fitting_zones).map_or(zone.id, |z| z.id)
        }
        Op::AddVariant { variant, .. } => {
            variant.id = pick(rng, &p.variants).map_or(variant.id, |v| v.id)
        }
        _ => {}
    }
}

#[test]
fn random_ops_undo_to_the_original_bytes_and_redo_to_the_final_ones() {
    const OPS: usize = 24;
    let mut rng = Rng::new(3);
    let (mut applied, mut refused, mut undone) = (0usize, 0usize, 0usize);
    let (mut ops_sent, mut ipc_misreads) = (0usize, 0usize);
    let mut refusal_codes = std::collections::BTreeMap::<&str, usize>::new();
    for seed in 0..CASES {
        let original = generate(seed);
        let original_json = to_json(&original);
        let mut p = original.clone();
        let mut history = History::new();
        // States after each applied op, to check every undo and redo step on the way.
        let mut states = vec![original.clone()];
        for _ in 0..OPS {
            let mut op = random_op(&mut rng, &p, 0);
            maybe_reuse_id(&mut rng, &p, &mut op);
            // Ops cross IPC as JSON text: through Op::to_json and Op::from_json they come back
            // bit for bit. Every op is then applied as it came back from its text.
            let text = op.to_json();
            let back = Op::from_json(&text)
                .unwrap_or_else(|e| panic!("seed {seed}: op text refused: {e}\n{text}"));
            assert!(back == op, "seed {seed}: op changed through its JSON text");
            // What an IPC layer parsing with serde_json::from_str would have received.
            ipc_misreads += usize::from(serde_json::from_str::<Op>(&text).ok() != Some(op));
            ops_sent += 1;
            let op = back;
            match history.apply(&mut p, op) {
                Ok(()) => {
                    p.check_integrity()
                        .unwrap_or_else(|e| panic!("seed {seed}: op broke integrity: {e}"));
                    states.push(p.clone());
                    applied += 1;
                }
                Err(e) => {
                    assert!(
                        &p == states.last().unwrap(),
                        "seed {seed}: refused op ({e}) changed the project"
                    );
                    *refusal_codes.entry(e.code()).or_default() += 1;
                    refused += 1;
                }
            }
        }
        let final_json = to_json(&p);
        for i in (0..states.len() - 1).rev() {
            assert!(history.undo(&mut p).unwrap(), "seed {seed}");
            assert!(p == states[i], "seed {seed}: undo step {i} differs");
            undone += 1;
        }
        assert!(!history.undo(&mut p).unwrap());
        assert!(p == original, "seed {seed}");
        assert_eq!(
            to_json(&p),
            original_json,
            "seed {seed}: undo left other bytes"
        );
        for (i, state) in states.iter().enumerate().skip(1) {
            assert!(history.redo(&mut p).unwrap(), "seed {seed}");
            assert!(&p == state, "seed {seed}: redo step {i} differs");
        }
        assert!(!history.redo(&mut p).unwrap());
        assert_eq!(
            to_json(&p),
            final_json,
            "seed {seed}: redo left other bytes"
        );
    }
    println!(
        "{CASES} projects x {OPS} random ops: {applied} applied, {refused} refused unchanged \
         {refusal_codes:?}, {undone} undone to the original bytes and redone to the final bytes"
    );
    println!(
        "{ops_sent} ops through their JSON text: all exact with Op::from_json; \
         serde_json::from_str would have misread {ipc_misreads}"
    );
    assert!(applied > refused);
    for code in [
        "not_found",
        "duplicate_id",
        "index",
        "band",
        "in_use",
        "no_transmission",
        "active_variant",
        "band_data",
        "integrity",
        "batch",
    ] {
        assert!(
            refusal_codes.contains_key(code),
            "no op was refused as {code}"
        );
    }
}

#[test]
fn solver_integers_beyond_a_c_int_are_refused_by_ops() {
    let mut p = generate(4);
    let before = p.clone();
    let integrity_code = |e: OpError| match e {
        OpError::Integrity(i) => i.code(),
        other => panic!("{other:?}"),
    };
    for field in 0..3 {
        let mut settings = p.solvers.clone();
        let slot = match field {
            0 => &mut settings.spps.random_seed,
            1 => &mut settings.spps.particles_per_source,
            _ => &mut settings.spps.particles_saved,
        };
        *slot = SOLVER_INT_MAX + 1;
        let err = Op::SetSolverSettings {
            settings: settings.clone(),
        }
        .apply(&mut p)
        .unwrap_err();
        assert_eq!(integrity_code(err), "solver_int_range");
        assert!(p == before);
        let slot = match field {
            0 => &mut settings.spps.random_seed,
            1 => &mut settings.spps.particles_per_source,
            _ => &mut settings.spps.particles_saved,
        };
        *slot = SOLVER_INT_MAX;
        let inverse = Op::SetSolverSettings { settings }.apply(&mut p).unwrap();
        inverse.apply(&mut p).unwrap();
        assert!(p == before);
    }
    let mut m = p.materials[0].clone();
    m.solver_id = Some(u32::MAX);
    let err = Op::ReplaceMaterial {
        material: m.clone(),
    }
    .apply(&mut p)
    .unwrap_err();
    assert_eq!(integrity_code(err), "solver_int_range");
    m.id = MaterialId::from_u128(7);
    let err = Op::AddMaterial {
        index: 0,
        material: m,
    }
    .apply(&mut p)
    .unwrap_err();
    assert_eq!(integrity_code(err), "solver_int_range");
    assert!(p == before);
}

#[test]
fn a_batch_is_all_or_nothing() {
    let mut p = generate(11);
    let before = p.clone();
    let group = p.surface_groups[0].id;
    let material = p.materials[0].id;
    let err = Op::Batch {
        ops: vec![
            Op::SetProjectName {
                name: "changed".into(),
            },
            Op::SetGroupMaterial { group, material },
            Op::RemoveMaterial { id: material },
        ],
    }
    .apply(&mut p)
    .unwrap_err();
    assert_eq!(err.code(), "batch");
    assert!(p == before);
}

#[test]
fn rebanding_octave_to_thirds_and_back_is_exact() {
    for seed in 0..500 {
        let p = generate(seed);
        if p.bands.kind != BandKind::Octave {
            continue;
        }
        let lo = p.bands.frequencies_hz[0];
        let hi = *p.bands.frequencies_hz.last().unwrap();
        // Thirds covering the same octaves (63 Hz's lower third is 50 Hz, 16 kHz's upper 20 kHz).
        let all = BandKind::ThirdOctave.nominal_frequencies();
        let i = all.iter().position(|&f| f == lo).unwrap().saturating_sub(1);
        let j = (all.iter().position(|&f| f == hi).unwrap() + 1).min(all.len() - 1);
        let thirds = BandSet::range(BandKind::ThirdOctave, all[i], all[j]).unwrap();
        let mut q = p.clone();
        q.rebanded(thirds.clone()).unwrap().apply(&mut q).unwrap();
        q.check_integrity().unwrap();
        assert_eq!(q.bands, thirds);
        // The lowest octave's centre third, and the third below it, carry the octave's value.
        let centre = all.iter().position(|&f| f == lo).unwrap() - i;
        for k in 0..=centre {
            assert_eq!(
                q.materials[0].absorption[k], p.materials[0].absorption[0],
                "seed {seed}"
            );
        }
        let back = q.rebanded(p.bands.clone()).unwrap();
        back.apply(&mut q).unwrap();
        assert!(
            q == p,
            "seed {seed}: octave -> third -> octave changed the project"
        );
    }
}

// ---------------------------------------------------------------------------------------------
// Load errors

fn edited(
    seed: u64,
    edit: impl FnOnce(&mut serde_json::Map<String, Value>),
) -> Result<Project, LoadError> {
    let mut tree = parse_json(&to_json(&generate(seed))).unwrap();
    edit(tree.as_object_mut().unwrap());
    from_json(&serde_json::to_string(&tree).unwrap())
}

#[test]
fn load_errors_are_typed() {
    let code = |r: Result<Project, LoadError>| r.map(|_| "ok").unwrap_or_else(|e| e.code());

    assert_eq!(code(edited(1, |_| {})), "ok");
    match edited(1, |o| {
        o.insert("format_version".into(), 2.into());
    }) {
        Err(LoadError::Version { found, supported }) => {
            assert_eq!((found.as_str(), supported), ("2", FORMAT_VERSION))
        }
        other => panic!("{other:?}"),
    }
    // A newer file with keys this build does not know gets the version error, not a schema one.
    assert_eq!(
        code(edited(1, |o| {
            o.insert("format_version".into(), 2.into());
            o.insert("new_thing".into(), Value::Null);
        })),
        "version"
    );
    assert_eq!(
        code(edited(1, |o| {
            o.insert("format_version".into(), "1".into());
        })),
        "version"
    );
    assert_eq!(
        code(edited(1, |o| {
            o.remove("format_version");
        })),
        "missing_version"
    );
    assert_eq!(code(from_json("[1]")), "missing_version");
    assert_eq!(
        code(edited(1, |o| {
            o.insert("colour".into(), Value::Null);
        })),
        "schema"
    );
    assert_eq!(
        code(edited(1, |o| {
            o.remove("view");
        })),
        "schema"
    );
    // An optional value must still be spelled out.
    assert_eq!(
        code(edited(1, |o| {
            o.remove("active_variant");
        })),
        "schema"
    );
    assert_eq!(
        code(edited(1, |o| {
            o["solvers"]["spps"]["particles_per_source"] = 1.5.into();
        })),
        "schema"
    );
    assert_eq!(
        code(edited(1, |o| {
            o["solvers"]["spps"]["duration_s"] = "nan".into();
        })),
        "schema"
    );
    assert_eq!(
        code(edited(1, |o| {
            o["frame"]["length_unit"] = "millimetre".into();
        })),
        "schema"
    );
    // Structure: a face pointing at a group that does not exist; a short per-band array.
    match edited(1, |o| {
        o["geometry"]["faces"][0][3] = "00000000-0000-4000-8000-000000000000".into();
    }) {
        Err(LoadError::Integrity(e)) => assert_eq!(e.code(), "dangling_reference"),
        other => panic!("{other:?}"),
    }
    match edited(1, |o| {
        o["materials"][0]["absorption"]
            .as_array_mut()
            .unwrap()
            .push(0.5.into());
    }) {
        Err(LoadError::Integrity(e)) => assert_eq!(e.code(), "band_count"),
        other => panic!("{other:?}"),
    }
    assert_eq!(
        code(from_json("{\"format_version\": 1, \"format_version\": 1}")),
        "syntax"
    );
    let text = to_json(&generate(1));
    assert_eq!(code(from_json(&text.replacen("\n}", ",\n}", 1))), "syntax");

    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("schema_roundtrip");
    std::fs::create_dir_all(&dir).unwrap();
    assert_eq!(code(load(&dir.join("does-not-exist.simpa"))), "not_found");
    let bad = dir.join("latin1.simpa");
    std::fs::write(&bad, b"{\"name\": \"caf\xe9\"}").unwrap();
    assert_eq!(code(load(&bad)), "utf8");

    // Tagged objects: a key that belongs to another kind, or to none, is refused for every kind,
    // unit kinds included (serde's derive alone accepts and drops it there). Each edit is first
    // made without the extra key, to show that the edit itself is well-formed.
    let direction = json!([1.0, 0.0, 0.0]);
    let directivities = [
        json!({"kind": "omni"}),
        json!({"kind": "plane_xy"}),
        json!({"kind": "plane_yz"}),
        json!({"kind": "plane_xz"}),
        json!({"kind": "unidirectional", "direction": direction}),
        json!({"kind": "balloon", "file": "speaker.txt", "direction": direction}),
    ];
    for d in directivities {
        for (extra, value) in [("file", json!("balloon.txt")), ("zzz", json!(1))] {
            let mut with = d.clone();
            if with.get(extra).is_some() {
                continue;
            }
            with[extra] = value;
            let set = |v: Value| edited(1, move |o| o["sources"][0]["directivity"] = v);
            assert_eq!(code(set(d.clone())), "ok", "{d}");
            assert_eq!(code(set(with.clone())), "schema", "{with}");
        }
    }
    let relative = json!([1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12]);
    for shape in [json!({"kind": "pink"}), json!({"kind": "white"})] {
        let mut with = shape.clone();
        with["relative_db"] = relative.clone();
        let set = |v: Value| edited(1, move |o| o["sources"][0]["power"]["shape"] = v);
        assert_eq!(code(set(shape.clone())), "ok", "{shape}");
        assert_eq!(code(set(with.clone())), "schema", "{with}");
    }
    let bands = generate(1).bands.len();
    let custom = json!({"kind": "custom", "relative_db": vec![0.0; bands]});
    let mut with = custom.clone();
    with["zzz"] = json!(1);
    let set = |v: Value| edited(1, move |o| o["sources"][0]["power"]["shape"] = v);
    assert_eq!(code(set(custom)), "ok");
    assert_eq!(code(set(with)), "schema");
    let user = json!({"kind": "user_defined", "value": 0.5, "unit": "decibel_per_metre"});
    let mut iso_with = user.clone();
    iso_with["kind"] = json!("iso9613");
    let mut user_with = user.clone();
    user_with["zzz"] = json!(1);
    let set = |v: Value| edited(1, move |o| o["environment"]["air_absorption"] = v);
    assert_eq!(code(set(json!({"kind": "iso9613"}))), "ok");
    assert_eq!(code(set(user.clone())), "ok");
    assert_eq!(code(set(iso_with.clone())), "schema", "{iso_with}");
    assert_eq!(code(set(user_with)), "schema");

    // The same through ops, as the UI would send them.
    let p = generate(1);
    let op_code = |tree: &Value| {
        Op::from_json(&tree.to_string())
            .map(|_| "ok")
            .unwrap_or_else(|e| e.code())
    };
    let mut set_env = serde_json::to_value(Op::SetEnvironment {
        environment: p.environment.clone(),
    })
    .unwrap();
    assert_eq!(op_code(&set_env), "ok");
    set_env["environment"]["air_absorption"] = iso_with;
    assert_eq!(op_code(&set_env), "schema");
    let mut rename = serde_json::to_value(Op::Rename {
        target: EntityRef::Material(p.materials[0].id),
        name: "x".into(),
    })
    .unwrap();
    assert_eq!(op_code(&rename), "ok");
    rename["target"]["zzz"] = json!(1);
    assert_eq!(op_code(&rename), "schema");
    rename["target"].as_object_mut().unwrap().remove("zzz");
    rename["target"].as_object_mut().unwrap().remove("id");
    assert_eq!(op_code(&rename), "schema");

    // Integers the solver reads as a C int: u32 in the model, at most i32::MAX.
    for pointer in [
        "/solvers/spps/random_seed",
        "/solvers/spps/particles_per_source",
        "/solvers/spps/particles_saved",
        "/materials/0/solver_id",
    ] {
        let set = |v: u64| {
            edited(1, move |o| {
                let mut tree = Value::Object(std::mem::take(o));
                *tree.pointer_mut(pointer).unwrap() = v.into();
                if let Value::Object(map) = tree {
                    *o = map;
                }
            })
        };
        assert_eq!(code(set(2_147_483_647)), "ok", "{pointer}");
        for v in [2_147_483_648, 4_294_967_295] {
            match set(v) {
                Err(LoadError::Integrity(e)) => assert_eq!(e.code(), "solver_int_range"),
                other => panic!("{pointer} = {v}: {other:?}"),
            }
        }
        assert_eq!(code(set(4_294_967_296)), "schema", "{pointer}");
    }

    // Fixed-length arrays: a point is 3 numbers, a face 3 indices and a group.
    assert_eq!(
        code(edited(1, |o| {
            o["sources"][0]["position"]
                .as_array_mut()
                .unwrap()
                .push(0.0.into());
        })),
        "schema"
    );
    assert_eq!(
        code(edited(1, |o| {
            o["geometry"]["vertices"][0].as_array_mut().unwrap().pop();
        })),
        "schema"
    );
    assert_eq!(
        code(edited(1, |o| {
            o["geometry"]["faces"][0]
                .as_array_mut()
                .unwrap()
                .push(0.into());
        })),
        "schema"
    );
}

// ---------------------------------------------------------------------------------------------
// Unknown keys, everywhere

/// JSON pointers of every object in a tree.
fn object_pointers(v: &Value, at: &str, out: &mut Vec<String>) {
    match v {
        Value::Object(o) => {
            out.push(at.to_string());
            for (k, x) in o {
                assert!(!k.contains(['/', '~']), "{k}");
                object_pointers(x, &format!("{at}/{k}"), out);
            }
        }
        Value::Array(a) => {
            for (i, x) in a.iter().enumerate() {
                object_pointers(x, &format!("{at}/{i}"), out);
            }
        }
        _ => {}
    }
}

/// `tree` with an unknown key added to the object at `pointer`.
fn with_unknown_key(tree: &Value, pointer: &str) -> String {
    let mut tree = tree.clone();
    tree.pointer_mut(pointer)
        .unwrap()
        .as_object_mut()
        .unwrap()
        .insert("zzz_unknown".into(), json!(1));
    serde_json::to_string(&tree).unwrap()
}

/// Every `kind` tag in a tree, with the key of the object that holds it.
fn tagged_kinds(v: &Value, key: &str, out: &mut HashSet<(String, String)>) {
    match v {
        Value::Object(o) => {
            if let Some(Value::String(kind)) = o.get("kind") {
                out.insert((key.to_string(), kind.clone()));
            }
            for (k, x) in o {
                tagged_kinds(x, k, out);
            }
        }
        Value::Array(a) => a.iter().for_each(|x| tagged_kinds(x, key, out)),
        _ => {}
    }
}

/// A generated project plus one source per directivity kind and per spectrum shape, a balloon
/// source and a fitting zone closed by surfaces: the kinds `generate` leaves out.
fn every_kind_project() -> Project {
    let mut p = generate(5);
    let n = p.bands.len();
    let template = p.sources[0].clone();
    let direction = Vec3::new(0.0, 1.0, 0.0);
    let directivities = [
        Directivity::Omni,
        Directivity::Unidirectional { direction },
        Directivity::PlaneXy,
        Directivity::PlaneYz,
        Directivity::PlaneXz,
        Directivity::Balloon {
            file: "speaker.txt".into(),
            direction,
        },
    ];
    let shapes = [
        SpectrumShape::Pink,
        SpectrumShape::White,
        SpectrumShape::Custom {
            relative_db: vec![F64::new(-1.5); n],
        },
    ];
    for (i, directivity) in directivities.into_iter().enumerate() {
        let mut s = template.clone();
        s.id = SourceId::from_u128(0x5000 + i as u128);
        s.name = format!("Kind {i}");
        s.directivity = directivity;
        s.power.shape = shapes[i % 3].clone();
        p.sources.push(s);
    }
    p.fitting_zones.push(FittingZone {
        id: FittingZoneId::from_u128(0x6000),
        name: "Closed by surfaces".into(),
        enabled: true,
        shape: FittingShape::Surfaces {
            groups: vec![p.surface_groups[0].id],
            inside_point: template.position,
        },
        absorption: vec![F64::new(0.3); n],
        mean_free_path_m: vec![F64::new(2.0); n],
        diffusion_law: vec![DiffusionLaw::LambertReflection; n],
    });
    p.check_integrity().unwrap();
    p
}

#[test]
fn an_unknown_key_is_refused_in_every_object_of_a_project() {
    let projects: Vec<Project> = (0..60)
        .map(generate)
        .chain([every_kind_project(), load(&fixture(CUBE)).unwrap()])
        .collect();
    let mut kinds = HashSet::new();
    let mut checked = 0;
    for p in &projects {
        let tree = parse_json(&to_json(p)).unwrap();
        tagged_kinds(&tree, "", &mut kinds);
        let mut pointers = Vec::new();
        object_pointers(&tree, "", &mut pointers);
        for pointer in pointers {
            match from_json(&with_unknown_key(&tree, &pointer)) {
                Err(LoadError::Schema(msg)) => assert!(msg.contains("zzz_unknown"), "{msg}"),
                Ok(_) => panic!("{}: an unknown key at {pointer:?} was accepted", p.name),
                Err(e) => panic!("{}: an unknown key at {pointer:?}: {e}", p.name),
            }
            checked += 1;
        }
    }
    // Every tagged kind a project can hold was among the objects checked.
    let expected: HashSet<(String, String)> = [
        ("directivity", "omni"),
        ("directivity", "unidirectional"),
        ("directivity", "plane_xy"),
        ("directivity", "plane_yz"),
        ("directivity", "plane_xz"),
        ("directivity", "balloon"),
        ("shape", "pink"),
        ("shape", "white"),
        ("shape", "custom"),
        ("shape", "scene"),
        ("shape", "cutting_plane"),
        ("shape", "box"),
        ("shape", "surfaces"),
        ("air_absorption", "iso9613"),
        ("air_absorption", "user_defined"),
        // Not a tagged enum: the band set's own `kind` field.
        ("bands", "octave"),
        ("bands", "third_octave"),
    ]
    .iter()
    .map(|&(k, v)| (k.to_string(), v.to_string()))
    .collect();
    assert_eq!(kinds, expected);
    println!(
        "{checked} objects in {} projects: an unknown key is refused in every one",
        projects.len()
    );
}

#[test]
fn an_unknown_key_is_refused_in_every_object_of_an_op() {
    let mut rng = Rng::new(4);
    let mut tags = HashSet::new();
    let mut kinds = HashSet::new();
    let mut checked = 0;
    for i in 0..3_000u64 {
        let p = if i % 10 == 0 {
            every_kind_project()
        } else {
            generate(i)
        };
        let op = random_op(&mut rng, &p, 0);
        let tree = parse_json(&op.to_json()).unwrap();
        tagged_kinds(&tree, "", &mut kinds);
        collect_op_tags(&tree, &mut tags);
        let mut pointers = Vec::new();
        object_pointers(&tree, "", &mut pointers);
        for pointer in pointers {
            match Op::from_json(&with_unknown_key(&tree, &pointer)) {
                Err(LoadError::Schema(msg)) => assert!(msg.contains("zzz_unknown"), "{msg}"),
                Ok(_) => panic!("op {i}: an unknown key at {pointer:?} was accepted"),
                Err(e) => panic!("op {i}: an unknown key at {pointer:?}: {e}"),
            }
            checked += 1;
        }
    }
    // All 37 ops, and every EntityRef kind (the adjacently tagged `target` of a rename).
    assert_eq!(tags.len(), 37, "{tags:?}");
    for kind in [
        "surface_group",
        "material",
        "source",
        "point_receiver",
        "surface_receiver",
        "fitting_zone",
        "variant",
    ] {
        assert!(
            kinds.contains(&("target".to_string(), kind.to_string())),
            "{kind}"
        );
    }
    println!(
        "{checked} objects in 3000 ops of all {} kinds: an unknown key is refused in every one",
        tags.len()
    );
}

/// Every `op` tag in a tree (batches nest ops).
fn collect_op_tags(v: &Value, out: &mut HashSet<String>) {
    match v {
        Value::Object(o) => {
            if let Some(Value::String(tag)) = o.get("op") {
                out.insert(tag.clone());
            }
            o.values().for_each(|x| collect_op_tags(x, out));
        }
        Value::Array(a) => a.iter().for_each(|x| collect_op_tags(x, out)),
        _ => {}
    }
}

// ---------------------------------------------------------------------------------------------
// Upstream-anchored values

#[test]
fn white_noise_shape_reproduces_tutorial1_levels() {
    let xml = std::fs::read_to_string(fixture("upstream/tutorial1/spps/config.xml")).unwrap();
    let doc = roxmltree::Document::parse(&xml).unwrap();
    let levels = |parent: &str| -> Vec<(u32, f64)> {
        let node = doc.descendants().find(|n| n.has_tag_name(parent)).unwrap();
        let mut v: Vec<(u32, f64)> = node
            .children()
            .filter(|n| n.has_tag_name("bfreq"))
            .map(|n| {
                (
                    n.attribute("freq").unwrap().parse().unwrap(),
                    n.attribute("db").unwrap().parse().unwrap(),
                )
            })
            .collect();
        v.sort_by_key(|&(f, _)| f);
        v
    };
    let bands = BandSet::range(BandKind::ThirdOctave, 50, 20000).unwrap();
    // Source 1: 80 dB of white noise. Receiver background noise: 0 dB of white noise.
    for (parent, global) in [("source", 80.0), ("recepteur_ponctuel", 0.0)] {
        let upstream = levels(parent);
        let ours = Spectrum::new(global, SpectrumShape::White)
            .band_levels_db(&bands)
            .unwrap();
        assert_eq!(upstream.len(), ours.len());
        for ((f, theirs), (mine, nominal)) in
            upstream.iter().zip(ours.iter().zip(&bands.frequencies_hz))
        {
            assert_eq!(f, nominal);
            // The GUI computes and writes in float32.
            assert!(
                (theirs - mine).abs() < 2e-5,
                "{parent} {f} Hz: {theirs} vs {mine}"
            );
        }
    }
}

// ---------------------------------------------------------------------------------------------
// The cube fixture

const CUBE: &str = "projects/cube.simpa";

/// cube.simpa, rebuilt from upstream's cube.cbin: one surface group and one material per `idMat`
/// in the file, the material's `solver_id` pinned to that `idMat`, so config.xml written from
/// this project matches `cube.cbin` and `cube_mesh.mbin` as they are.
fn cube_project() -> Project {
    let model = cbin::read_file(&fixture("upstream/lib_interface/cube.cbin")).unwrap();
    let id = |k: u128| 0x0c0b_e000_0000_4000_8000_0000_0000_0000u128 | k;
    let mut id_mats: Vec<u32> = model.faces.iter().map(|f| f.id_mat).collect();
    id_mats.sort_unstable();
    id_mats.dedup();
    let bands = BandSet::default();
    let n = bands.len();
    let materials: Vec<Material> = id_mats
        .iter()
        .enumerate()
        .map(|(i, &k)| Material {
            id: MaterialId::from_u128(id(0x100 + i as u128)),
            name: format!("Cube walls (idMat {k})"),
            color: Rgb(0xb0, 0xb0, 0xb0),
            absorption: vec![F64::new(0.2); n],
            scattering: vec![F64::new(0.1); n],
            reflection_law: ReflectionLaw::Lambert.into(),
            transmission_loss_db: None,
            double_sided: true,
            solver_id: Some(k),
        })
        .collect();
    let groups: Vec<SurfaceGroup> = id_mats
        .iter()
        .enumerate()
        .map(|(i, &k)| SurfaceGroup {
            id: GroupId::from_u128(id(0x200 + i as u128)),
            name: format!("Cube faces (idMat {k})"),
            material: materials[i].id,
        })
        .collect();
    let group_of = |k: u32| groups[id_mats.binary_search(&k).unwrap()].id;
    let mut solvers = SolverSettings::for_bands(n);
    solvers.spps.particles_per_source = 2000;
    solvers.spps.duration_s = F64::new(1.0);
    solvers.spps.time_step_s = F64::new(0.002);
    solvers.spps.random_seed = 1;
    Project {
        format_version: FORMAT_VERSION,
        id: ProjectId::from_u128(id(1)),
        name: "Cube".into(),
        description: "Upstream's 5 m test cube (src/lib_interface/tests/cube.cbin, \
                      cube_mesh.mbin at 929a5c8). Material solver ids equal the cube.cbin idMat \
                      values. Source and receiver are at least 0.35 m from every plane through \
                      three cube corners, so off every face of cube_mesh.mbin's 6 tetrahedra."
            .into(),
        frame: Frame::default(),
        bands,
        geometry: Geometry {
            vertices: model
                .vertices
                .iter()
                .map(|v| Vec3::new(f64::from(v.x), f64::from(v.y), f64::from(v.z)))
                .collect(),
            faces: model
                .faces
                .iter()
                .map(|f| Face {
                    vertices: [f.a, f.b, f.c],
                    group: group_of(f.id_mat),
                })
                .collect(),
        },
        surface_groups: groups,
        materials,
        sources: vec![Source {
            id: SourceId::from_u128(id(0x300)),
            name: "Source 1".into(),
            enabled: true,
            position: Vec3::new(3.0, 1.25, 2.5),
            power: Spectrum::new(90.0, SpectrumShape::Pink),
            directivity: Directivity::Omni,
            delay_s: F64::ZERO,
        }],
        point_receivers: vec![PointReceiver {
            id: PointReceiverId::from_u128(id(0x400)),
            name: "Receiver 1".into(),
            position: Vec3::new(2.0, 3.75, 2.5),
            orientation: Vec3::new(1.0, 0.0, 0.0),
            background_noise: None,
        }],
        surface_receivers: Vec::new(),
        fitting_zones: Vec::new(),
        environment: Environment::default(),
        solvers,
        variants: Vec::new(),
        active_variant: None,
        view: ViewState::default(),
    }
}

#[test]
#[ignore = "writes tests/fixtures/projects/cube.simpa; run on purpose to regenerate it"]
fn write_cube_fixture() {
    let path = fixture(CUBE);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    save(&cube_project(), &path).unwrap();
}

#[test]
fn cube_fixture_loads_resaves_identically_and_matches_cube_cbin() {
    let bytes = std::fs::read(fixture(CUBE)).unwrap();
    let text = std::str::from_utf8(&bytes).unwrap();
    let p = load(&fixture(CUBE)).unwrap();
    assert_eq!(
        to_json(&p),
        text,
        "cube.simpa does not re-save byte-identically"
    );
    assert!(
        p == cube_project(),
        "cube.simpa is not what cube_project() builds from cube.cbin"
    );

    // Geometry and material ids against upstream's file, independently of cube_project().
    let model = cbin::read_file(&fixture("upstream/lib_interface/cube.cbin")).unwrap();
    assert_eq!(p.geometry.vertices.len(), model.vertices.len());
    for (ours, theirs) in p.geometry.vertices.iter().zip(&model.vertices) {
        for (a, b) in [(ours.x, theirs.x), (ours.y, theirs.y), (ours.z, theirs.z)] {
            assert_eq!(a.to_bits(), f64::from(b).to_bits());
        }
    }
    assert_eq!(
        p.geometry.vertices[0].y.to_bits(),
        (-0.0f64).to_bits(),
        "cube.cbin's -0.0"
    );
    assert_eq!(p.geometry.faces.len(), model.faces.len());
    for (face, theirs) in p.geometry.faces.iter().zip(&model.faces) {
        assert_eq!(face.vertices, [theirs.a, theirs.b, theirs.c]);
        let material = p.material(p.active_material(face.group).unwrap()).unwrap();
        assert_eq!(material.solver_id, Some(theirs.id_mat));
    }

    // One omni source and one point receiver strictly inside the 5 m cube.
    let inside = |v: Vec3| v.to_array().iter().all(|&c| c > 0.5 && c < 4.5);
    assert_eq!(p.sources.len(), 1);
    assert_eq!(p.sources[0].directivity, Directivity::Omni);
    assert!(inside(p.sources[0].position));
    assert_eq!(p.point_receivers.len(), 1);
    assert!(inside(p.point_receivers[0].position));
    assert_eq!(p.bands, BandSet::default());
    assert!(p.solvers.spps.particles_per_source <= 2000);
}

// ---------------------------------------------------------------------------------------------
// What `generate` promises

#[test]
fn generated_projects_meet_the_value_rules_generate_documents() {
    for seed in 0..CASES {
        let p = generate(seed);
        let lo: Vec<f64> = (0..3)
            .map(|a| {
                p.geometry
                    .vertices
                    .iter()
                    .map(|v| v.to_array()[a])
                    .fold(f64::INFINITY, f64::min)
            })
            .collect();
        let hi: Vec<f64> = (0..3)
            .map(|a| {
                p.geometry
                    .vertices
                    .iter()
                    .map(|v| v.to_array()[a])
                    .fold(f64::NEG_INFINITY, f64::max)
            })
            .collect();
        let clear_of_walls = |v: Vec3, m: f64| {
            (0..3).all(|a| v.to_array()[a] >= lo[a] + m && v.to_array()[a] <= hi[a] - m)
        };
        let clear_of_fittings = |v: Vec3| {
            p.fitting_zones.iter().all(|z| match &z.shape {
                FittingShape::Box { min, max, .. } => (0..3).any(|a| {
                    v.to_array()[a] < min.to_array()[a] - 0.3
                        || v.to_array()[a] > max.to_array()[a] + 0.3
                }),
                FittingShape::Surfaces { .. } => true,
            })
        };
        for s in &p.sources {
            assert!(
                clear_of_walls(s.position, 0.6) && clear_of_fittings(s.position),
                "seed {seed}"
            );
            assert!(!matches!(s.directivity, Directivity::Balloon { .. }));
            if let Some(d) = s.directivity.direction() {
                assert!(d.length() > 0.0);
            }
        }
        assert!(p.sources.iter().any(|s| s.enabled));
        let radius = p.solvers.spps.receiver_radius_m.get();
        assert!(radius > 0.0 && radius < 0.6);
        for r in &p.point_receivers {
            assert!(
                clear_of_walls(r.position, 0.6) && clear_of_fittings(r.position),
                "seed {seed}"
            );
            assert!(r.orientation.length() > 0.0);
        }
        for m in &p.materials {
            for (i, a) in m.absorption.iter().enumerate() {
                let (a, s) = (a.get(), m.scattering[i].get());
                assert!(
                    (0.0..=1.0).contains(&a) && (0.0..=1.0).contains(&s),
                    "seed {seed}"
                );
                if a == 1.0 {
                    assert_eq!(s, 0.0);
                }
                if let Some(Some(r)) = m.transmission_loss_db.as_ref().map(|t| t[i]) {
                    assert!(a > 0.0 && 10f64.powf(-r.get() / 10.0) <= a, "seed {seed}");
                }
            }
        }
        let spps = &p.solvers.spps;
        assert!(spps.time_step_s.get() > 0.0 && spps.extinction_exponent.get() > 0.0);
        assert!(spps.duration_s.get() / spps.time_step_s.get() < 65_536.0);
        assert!(spps.random_seed <= i32::MAX as u32);
        assert!(
            spps.bands_computed.contains(&true) && p.solvers.tcr.bands_computed.contains(&true)
        );
        let mut names = HashSet::new();
        let all_names = p
            .surface_groups
            .iter()
            .map(|x| &x.name)
            .chain(p.materials.iter().map(|x| &x.name))
            .chain(p.sources.iter().map(|x| &x.name))
            .chain(p.point_receivers.iter().map(|x| &x.name))
            .chain(p.surface_receivers.iter().map(|x| &x.name))
            .chain(p.fitting_zones.iter().map(|x| &x.name))
            .chain(p.variants.iter().map(|x| &x.name));
        for name in all_names {
            assert!(
                name.is_ascii() && name.len() < 50 && names.insert(name.clone()),
                "seed {seed}: {name}"
            );
        }
        let mut scene_groups = HashSet::new();
        for r in &p.surface_receivers {
            if let SurfaceReceiverShape::Scene { groups } = &r.shape {
                assert!(
                    groups.iter().all(|g| scene_groups.insert(*g)),
                    "seed {seed}"
                );
            }
        }
    }
}

/// Checks `v` against `schema` for the keywords schemars emits here, and panics on any other
/// keyword so nothing is skipped silently. `pattern` and `format` are not checked. Returns the
/// first violation, with its JSON pointer.
fn schema_violation(root: &Value, schema: &Value, v: &Value, at: &str) -> Option<String> {
    const KNOWN: &[&str] = &[
        "$ref",
        "$schema",
        "$defs",
        "title",
        "description",
        "format",
        "pattern",
        "type",
        "const",
        "enum",
        "minimum",
        "maximum",
        "required",
        "properties",
        "additionalProperties",
        "prefixItems",
        "items",
        "minItems",
        "maxItems",
        "oneOf",
        "anyOf",
    ];
    let fail = |why: String| Some(format!("{at}: {why}"));
    let s = match schema {
        Value::Bool(true) => return None,
        Value::Bool(false) => return fail("no value is allowed here".into()),
        Value::Object(s) => s,
        other => panic!("not a schema: {other}"),
    };
    if let Some(k) = s.keys().find(|k| !KNOWN.contains(&k.as_str())) {
        panic!("the checker does not know the keyword {k}");
    }
    if let Some(Value::String(r)) = s.get("$ref") {
        let target = root.pointer(r.strip_prefix('#').unwrap()).unwrap();
        if let Some(e) = schema_violation(root, target, v, at) {
            return Some(e);
        }
    }
    if let Some(t) = s.get("type") {
        let types: Vec<&str> = match t {
            Value::String(t) => vec![t.as_str()],
            Value::Array(a) => a.iter().map(|t| t.as_str().unwrap()).collect(),
            other => panic!("type {other}"),
        };
        let ok = types.iter().any(|&t| match t {
            "null" => v.is_null(),
            "boolean" => v.is_boolean(),
            "object" => v.is_object(),
            "array" => v.is_array(),
            "string" => v.is_string(),
            "number" => v.is_number(),
            "integer" => v.is_u64() || v.is_i64(),
            other => panic!("type {other}"),
        });
        if !ok {
            return fail(format!("{v} is not of type {t}"));
        }
    }
    if let Some(c) = s.get("const")
        && v != c
    {
        return fail(format!("{v} is not {c}"));
    }
    if let Some(Value::Array(e)) = s.get("enum")
        && !e.contains(v)
    {
        return fail(format!("{v} is not one of {e:?}"));
    }
    if let Some(n) = v.as_f64() {
        if let Some(min) = s.get("minimum").and_then(Value::as_f64)
            && n < min
        {
            return fail(format!("{v} is below {min}"));
        }
        if let Some(max) = s.get("maximum").and_then(Value::as_f64)
            && n > max
        {
            return fail(format!("{v} is above {max}"));
        }
    }
    if let Value::Object(o) = v {
        if let Some(Value::Array(required)) = s.get("required") {
            for k in required {
                if !o.contains_key(k.as_str().unwrap()) {
                    return fail(format!("missing key {k}"));
                }
            }
        }
        let properties = s.get("properties").and_then(Value::as_object);
        for (k, x) in o {
            match properties.and_then(|p| p.get(k)) {
                Some(ps) => {
                    if let Some(e) = schema_violation(root, ps, x, &format!("{at}/{k}")) {
                        return Some(e);
                    }
                }
                None if s.get("additionalProperties") == Some(&Value::Bool(false)) => {
                    return fail(format!("unknown key {k}"));
                }
                None => {}
            }
        }
    }
    if let Value::Array(a) = v {
        let n = a.len() as u64;
        if s.get("minItems")
            .and_then(Value::as_u64)
            .is_some_and(|m| n < m)
            || s.get("maxItems")
                .and_then(Value::as_u64)
                .is_some_and(|m| n > m)
        {
            return fail(format!("{n} items"));
        }
        let prefix = s.get("prefixItems").and_then(Value::as_array);
        for (i, x) in a.iter().enumerate() {
            let item = prefix.and_then(|p| p.get(i)).or(s.get("items"));
            if let Some(e) = item.and_then(|is| schema_violation(root, is, x, &format!("{at}/{i}")))
            {
                return Some(e);
            }
        }
    }
    if let Some(Value::Array(one)) = s.get("oneOf") {
        let matching = one
            .iter()
            .filter(|x| schema_violation(root, x, v, at).is_none())
            .count();
        if matching != 1 {
            return fail(format!("{v} matches {matching} branches of a oneOf"));
        }
    }
    if let Some(Value::Array(any)) = s.get("anyOf")
        && any
            .iter()
            .all(|x| schema_violation(root, x, v, at).is_some())
    {
        return fail(format!("{v} matches no branch of an anyOf"));
    }
    None
}

/// The published JSON Schema says what the reader does: every file `to_json` writes passes it,
/// and the edits the reader refuses as schema errors fail it too, the ones on tagged objects,
/// optional values and C ints included.
#[test]
fn the_json_schema_accepts_every_file_and_refuses_what_the_reader_refuses() {
    let schema = serde_json::to_value(json_schema()).unwrap();
    let projects: Vec<Project> = (0..200)
        .map(generate)
        .chain([every_kind_project(), load(&fixture(CUBE)).unwrap()])
        .collect();
    for p in &projects {
        let tree = parse_json(&to_json(p)).unwrap();
        assert_eq!(
            schema_violation(&schema, &schema, &tree, ""),
            None,
            "{}",
            p.name
        );
    }

    let bands = generate(1).bands.len();
    // (object, key, new value or None to remove the key)
    let hostile: Vec<(&str, &str, Option<Value>)> = vec![
        (
            "/sources/0",
            "directivity",
            Some(json!({"kind": "omni", "file": "balloon.txt"})),
        ),
        (
            "/sources/0/power",
            "shape",
            Some(json!({"kind": "pink", "relative_db": vec![1.0; bands]})),
        ),
        (
            "/environment",
            "air_absorption",
            Some(json!({"kind": "iso9613", "value": 0.5, "unit": "decibel_per_metre"})),
        ),
        (
            "/solvers/spps",
            "random_seed",
            Some(json!(2_147_483_648u64)),
        ),
        (
            "/solvers/spps",
            "particles_per_source",
            Some(json!(4_294_967_295u64)),
        ),
        ("/materials/0", "solver_id", Some(json!(2_147_483_648u64))),
        ("", "active_variant", None),
        ("/view", "camera", None),
        ("/materials/0", "transmission_loss_db", None),
        ("/frame", "length_unit", Some(json!("millimetre"))),
        ("", "zzz_unknown", Some(json!(1))),
        ("/sources/0", "position", Some(json!([1.0, 2.0, 3.0, 4.0]))),
    ];
    let original = parse_json(&to_json(&generate(1))).unwrap();
    for (object, key, value) in hostile {
        let mut tree = original.clone();
        let o = tree.pointer_mut(object).unwrap().as_object_mut().unwrap();
        match &value {
            Some(v) => o.insert(key.into(), v.clone()),
            None => o.remove(key),
        };
        assert!(
            from_json(&serde_json::to_string(&tree).unwrap()).is_err(),
            "{object}/{key}"
        );
        assert!(
            schema_violation(&schema, &schema, &tree, "").is_some(),
            "the schema accepts {object}/{key} = {value:?}"
        );
    }

    // Ops against the schema schemars derives for them. The random ops include values the model
    // refuses (a particle count beyond a C int, say); the schema may refuse those too, but it
    // must never refuse an op the model applies.
    let op_schema = serde_json::to_value(schemars::schema_for!(Op)).unwrap();
    let mut rng = Rng::new(5);
    let (mut passed, mut refused_by_both) = (0, 0);
    for i in 0..2_000u64 {
        let mut p = generate(i);
        let op = random_op(&mut rng, &p, 0);
        let tree = parse_json(&op.to_json()).unwrap();
        match schema_violation(&op_schema, &op_schema, &tree, "") {
            None => passed += 1,
            Some(why) => {
                assert!(
                    op.apply(&mut p).is_err(),
                    "op {i}: the schema refuses an op the model applies: {why}"
                );
                refused_by_both += 1;
            }
        }
    }
    assert!(passed > 1_800, "{passed}");
    println!(
        "{} projects pass the JSON Schema and 12 refused edits fail it; of 2000 random ops \
         {passed} pass and {refused_by_both} fail it, each of those refused by Op::apply too",
        projects.len()
    );
}

#[test]
fn json_schema_describes_every_top_level_key() {
    let schema = serde_json::to_value(json_schema()).unwrap();
    let tree = parse_json(&to_json(&generate(3))).unwrap();
    let keys: HashSet<&String> = tree.as_object().unwrap().keys().collect();
    let properties: HashSet<&String> = schema["properties"].as_object().unwrap().keys().collect();
    assert_eq!(keys, properties);
    let required: HashSet<&str> = schema["required"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert!(
        keys.iter().all(|k| required.contains(k.as_str())),
        "{required:?}"
    );
}
