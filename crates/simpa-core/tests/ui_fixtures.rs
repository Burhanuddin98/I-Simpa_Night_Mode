//! The M10 UI fixtures (docs/investigations/2026-09-29-m10/PLAN.md, section 5), built from core
//! types only: nothing here imports app or UI code.
//!
//! - `teaching_room.simpa`: Concept B's room, a 10 x 6 x 3 m box with six materials (every value
//!   0.10), one source and three receivers, fixed ids.
//! - `materials_6x6.tsv`: six rows (the materials, in project order) by six octave bands of
//!   absorption, as a spreadsheet copies them: CRLF, a trailing CRLF, no header, no name column.
//!   One cell holds a 17-significant-digit decimal that `serde_json::from_str` misreads in this
//!   build, so a lossy step anywhere between a paste and a save changes the bytes.
//! - `materials_6x6.expected.json`: the committed start project with the committed TSV applied,
//!   one `SetMaterialBand` per cell through `History`, written with `schema::to_json`. It is
//!   computed from the committed files' bytes, not from this recipe's values, and never typed to
//!   match what the UI produces.
//!
//!
//! The M11 run fixtures (docs/investigations/2026-09-29-m11/PLAN.md, section 5):
//! - `box_run.simpa`: the teaching room with a fixed SPPS seed, the first whose run lost no
//!   particle in any band (measured, PROVENANCE.md): "the box" of gate (a).
//! - `box_long.simpa`: the same with more particles, so that its solve lasts at least 60 s: the
//!   run gate (d) closes the window on and kills.
//! - `hall_run.simpa`: `testdata/elmia_corrected.ply` imported by the core, every group on the
//!   library's "20% absorbing", the first source and two receivers of the hall fixture, SPPS
//!   energetic with a million particles and a fixed seed: the run gates (b) and (c) measure and
//!   cancel. Its grouping is the PLY's; the run tests responsiveness and cancel, not physics.
//! - `tetgen_skips.bat`: a stand-in `tetgen.exe` that plays TetGen 1.6.0 skipping two
//!   self-intersecting facets (it writes `scene_mesh_skipped.face` and exits 3). The verified
//!   TetGen 1.5.0 never writes that file, and the geometry check keeps self-intersecting input
//!   from reaching TetGen at all, so this is the one way a run can end `tetgen_skipped_facets`
//!   for gate (e). It is named as the fake it is; everything the app shows of that run is read
//!   from the core's real records.
//!
//! Regenerate with SIMPA_WRITE_FIXTURES=1; otherwise every committed file must equal its recipe.

use std::path::PathBuf;

use regex::Regex;
use simpa_core::geometry::check::check;
use simpa_core::schema::{
    self, BandSet, Directivity, F64, Face, GroupId, History, Material, MaterialId,
    MaterialQuantity, Op, PointReceiver, PointReceiverId, Project, ProjectId, ReflectionLaw, Rgb,
    Source, SourceId, Spectrum, SpectrumShape, SurfaceGroup, Vec3,
};
use simpa_core::validate::validate;

const ROOM: &str = "tests/fixtures/ui/teaching_room.simpa";
const BOX_RUN: &str = "tests/fixtures/ui/box_run.simpa";
const BOX_LONG: &str = "tests/fixtures/ui/box_long.simpa";
const HALL_RUN: &str = "tests/fixtures/ui/hall_run.simpa";
const TETGEN_SKIPS: &str = "tests/fixtures/ui/tetgen_skips.bat";

/// The SPPS seed of `box_run.simpa`: the first of 1, 2, 3, ... whose run of the teaching room at
/// 150,000 particles lost no particle in any of its 6 bands, with the verified solvers
/// (PROVENANCE.md lists every seed tried and its losses).
const BOX_SEED: u32 = 5;
/// Particles per source of `box_long.simpa`: measured so that its solve lasts at least 60 s
/// (PROVENANCE.md).
const BOX_LONG_PARTICLES: u32 = 7_000_000;
/// The hall's seed (any fixed value: it makes SPPS single-threaded and the run reproducible).
const HALL_SEED: u32 = 1;
const HALL_PARTICLES: u32 = 1_000_000;
/// Upstream's reference material id of the library's "20% absorbing".
const HALL_MATERIAL: u32 = 22;
const TSV: &str = "tests/fixtures/ui/materials_6x6.tsv";
const EXPECTED: &str = "tests/fixtures/ui/materials_6x6.expected.json";

/// The grammar the UI's `parseStrictDecimal` accepts (PLAN.md 2.2).
const STRICT_DECIMAL: &str = r"^[+-]?(\d+(\.\d*)?|\.\d+)([eE][+-]?\d+)?$";

fn repo(rel: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(rel)
}

fn writing() -> bool {
    std::env::var_os("SIMPA_WRITE_FIXTURES").is_some()
}

/// Writes `bytes` under SIMPA_WRITE_FIXTURES, then requires the committed file to equal them.
fn check_or_write(rel: &str, bytes: &[u8]) {
    let path = repo(rel);
    if writing() {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, bytes).unwrap();
    }
    let committed = std::fs::read(&path).unwrap_or_else(|e| panic!("{rel}: {e}"));
    assert!(
        committed == bytes,
        "{rel} differs from its recipe; regenerate with SIMPA_WRITE_FIXTURES=1"
    );
}

const GROUPS: [&str; 6] = [
    "Floor",
    "Ceiling",
    "Left wall",
    "Right wall",
    "Front wall",
    "Rear wall",
];

/// (name, display colour, the groups that use it). The swatches are the design's
/// (concept-b-approved.dc.html, MATERIALS); the glass swatch is new.
const MATERIALS: [(&str, [u8; 3], &[usize]); 6] = [
    ("Linoleum on concrete", [0x6b, 0x6b, 0x73], &[0]),
    ("Mineral-fibre tile", [0xb8, 0xb8, 0xc0], &[1]),
    ("Painted plaster", [0x4a, 0x4a, 0x52], &[2, 3, 4]),
    ("Slotted wood panel", [0x8a, 0x5a, 0x3c], &[5]),
    ("Heavy velour curtain", [0x7a, 0x15, 0x20], &[]),
    ("Glass window", [0x5e, 0x7a, 0x86], &[]),
];

fn group_id(i: usize) -> GroupId {
    GroupId::from_u128(0x100 + i as u128)
}

fn material_id(i: usize) -> MaterialId {
    MaterialId::from_u128(0x200 + i as u128)
}

/// The teaching room: x 0-10 m, y 0-6 m, z 0-3 m; Front wall at x = 0, Rear wall at x = 10,
/// Right wall at y = 0, Left wall at y = 6. Twelve triangles, normals out of the room, two per
/// group in group order.
fn teaching_room() -> Project {
    let mut p = Project::new("Teaching room");
    p.id = ProjectId::from_u128(0x1);
    p.description =
        "Concept B's teaching room (crates/simpa-core/tests/ui_fixtures.rs).".to_string();
    // Pinned to its historical 2 s, not left to track `SppsSettings::for_bands`'s new-project
    // default (now 10 s, decision row 36, `docs/decision-log.md`): this fixture, and
    // `box_run.simpa`, `box_long.simpa` and `hall_run.simpa` built from it, are each a specific
    // measured run (fixed seeds, particle counts, named gates); letting them silently follow the
    // default would change their numbers and invalidate what each gate measured.
    p.solvers.spps.duration_s = F64::new(2.0);
    // Pinned to random for the same reason: the new-project default is energetic since
    // decision row 38, and these measured runs were made in random mode.
    p.solvers.spps.method = simpa_core::schema::ComputationMethod::Random;
    assert_eq!(p.bands, BandSet::default(), "octave bands, 125 Hz to 4 kHz");
    let n = p.bands.len();
    p.geometry.vertices = [
        [0.0, 0.0, 0.0],
        [10.0, 0.0, 0.0],
        [10.0, 6.0, 0.0],
        [0.0, 6.0, 0.0],
        [0.0, 0.0, 3.0],
        [10.0, 0.0, 3.0],
        [10.0, 6.0, 3.0],
        [0.0, 6.0, 3.0],
    ]
    .map(Vec3::from)
    .to_vec();
    let triangles: [[[u32; 3]; 2]; 6] = [
        [[0, 2, 1], [0, 3, 2]], // Floor, -z
        [[4, 5, 6], [4, 6, 7]], // Ceiling, +z
        [[3, 7, 6], [3, 6, 2]], // Left wall, +y
        [[0, 1, 5], [0, 5, 4]], // Right wall, -y
        [[0, 4, 7], [0, 7, 3]], // Front wall, -x
        [[1, 2, 6], [1, 6, 5]], // Rear wall, +x
    ];
    p.geometry.faces = triangles
        .iter()
        .enumerate()
        .flat_map(|(g, pair)| {
            pair.iter().map(move |&vertices| Face {
                vertices,
                group: group_id(g),
            })
        })
        .collect();
    p.materials = MATERIALS
        .iter()
        .enumerate()
        .map(|(i, (name, [r, g, b], _))| Material {
            id: material_id(i),
            name: name.to_string(),
            color: Rgb(*r, *g, *b),
            absorption: vec![F64::new(0.10); n],
            scattering: vec![F64::new(0.10); n],
            reflection_law: ReflectionLaw::Specular.into(),
            transmission_loss_db: None,
            double_sided: true,
            solver_id: None,
        })
        .collect();
    p.surface_groups = GROUPS
        .iter()
        .enumerate()
        .map(|(g, name)| SurfaceGroup {
            id: group_id(g),
            name: name.to_string(),
            material: material_id(
                MATERIALS
                    .iter()
                    .position(|(_, _, used)| used.contains(&g))
                    .expect("every group has a material"),
            ),
        })
        .collect();
    p.sources = vec![Source {
        id: SourceId::from_u128(0x300),
        name: "S1".to_string(),
        enabled: true,
        position: Vec3::new(2.0, 3.0, 1.5),
        power: Spectrum::new(85.0, SpectrumShape::Pink),
        directivity: Directivity::Omni,
        delay_s: F64::ZERO,
        group: None,
        solver_id: None,
    }];
    p.point_receivers = [
        ("R1", [5.0, 2.0, 1.2]),
        ("R2", [8.0, 4.0, 1.2]),
        ("R3", [3.5, 4.5, 1.2]),
    ]
    .iter()
    .enumerate()
    .map(|(i, (name, pos))| PointReceiver {
        id: PointReceiverId::from_u128(0x400 + i as u128),
        name: name.to_string(),
        position: Vec3::from(*pos),
        orientation: Vec3::new(1.0, 0.0, 0.0),
        background_noise: None,
        solver_id: None,
    })
    .collect();
    p.check_integrity()
        .expect("the teaching room is consistent");
    p
}

/// Absorption per material and octave band, 125 Hz to 4 kHz. Rows 0-4 are the design's
/// (concept-b-approved.dc.html, MATERIALS: lino, tile, plaster, panel, curtain); row 5 is the
/// published "ordinary window glass" row (PROVENANCE.md).
const ABSORPTION: [[&str; 6]; 6] = [
    ["0.02", "0.03", "0.03", "0.03", "0.03", "0.02"],
    ["0.50", "0.70", "0.60", "0.70", "0.70", "0.50"],
    ["0.013", "0.015", "0.02", "0.03", "0.04", "0.05"],
    ["0.25", "0.60", "0.80", "0.70", "0.50", ""],
    ["0.14", "0.35", "0.55", "0.72", "0.70", "0.65"],
    ["0.35", "0.25", "0.18", "0.12", "0.07", "0.04"],
];

/// The cell that holds the misread decimal: the wood panel at 4 kHz (design value 0.40).
const MISREAD_CELL: (usize, usize) = (3, 5);

/// A decimal in [0.395, 0.405) that is its double's shortest round-trip spelling, has 17
/// significant digits, and that `serde_json::from_str` reads to a different double: the first a
/// seeded xorshift64 finds. Being the shortest spelling, it is also the text JavaScript's
/// `String(Number(cell))` sends over IPC.
///
/// Why this range: serde_json computes `round(significand) / 10^17` here, two roundings. Their
/// error can pass half an ulp of the result only where the 17-digit significand sits low in its
/// binary exponent range and the value high in its own; at 0.40 the significand is 4e16, just
/// above 2^55, and about a third of such decimals are misread. At 0.35 (3.5e16, just below 2^55)
/// none is, and above 0.5 no double needs 17 digits at all.
fn misread_decimal() -> String {
    let (lo, hi) = (0.395f64, 0.405f64);
    let mut x: u64 = 0x005e_ed0f_0010;
    for _ in 0..1_000_000 {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        let v = lo + (hi - lo) * ((x >> 11) as f64 / (1u64 << 53) as f64);
        let text = format!("{v}");
        let digits = text.trim_start_matches("0.").trim_start_matches('0').len();
        if digits != 17 || text.parse::<f64>().unwrap().to_bits() != v.to_bits() {
            continue;
        }
        let lossy: f64 = serde_json::from_str(&text).unwrap();
        if lossy.to_bits() != v.to_bits() {
            return text;
        }
    }
    panic!("no misread decimal in a million draws");
}

fn tsv_text() -> String {
    let odd = misread_decimal();
    let mut out = String::new();
    for (r, row) in ABSORPTION.iter().enumerate() {
        let cells: Vec<&str> = row
            .iter()
            .enumerate()
            .map(|(b, &c)| {
                if (r, b) == MISREAD_CELL {
                    odd.as_str()
                } else {
                    c
                }
            })
            .collect();
        out.push_str(&cells.join("\t"));
        out.push_str("\r\n");
    }
    out
}

/// The committed TSV read the plain way: CRLF rows, tab cells, `str::parse::<f64>` (correctly
/// rounded, as JavaScript's `Number()` is).
fn read_tsv(bytes: &[u8]) -> Vec<Vec<f64>> {
    let text = std::str::from_utf8(bytes).expect("the TSV is UTF-8");
    let body = text.strip_suffix("\r\n").expect("a trailing CRLF");
    body.split("\r\n")
        .map(|row| {
            row.split('\t')
                .map(|c| c.parse::<f64>().unwrap_or_else(|e| panic!("{c:?}: {e}")))
                .collect()
        })
        .collect()
}

/// One test, the steps in order: each later step reads the files the earlier ones committed,
/// and separate tests would run in parallel and race on them under SIMPA_WRITE_FIXTURES.
#[test]
fn ui_fixtures_match_their_recipes() {
    teaching_room_matches_its_recipe_and_passes_every_check();
    materials_tsv_matches_its_recipe();
    the_expected_project_is_the_core_s_output_for_the_committed_files();
    the_run_fixtures_match_their_recipes();
}

/// The checks every run fixture passes: the M4 check is ok, and the validator finds no error
/// (so no run blocker is the project's; the app adds only the solver and run-slot blockers).
fn runnable(label: &str, p: &Project) {
    let report = check(&p.geometry);
    assert!(report.is_ok(), "{label}: {:?}", report.reasons);
    let errors: Vec<_> = validate(p)
        .into_iter()
        .filter(|i| i.severity == simpa_core::validate::Severity::Error)
        .collect();
    assert!(errors.is_empty(), "{label}: {errors:#?}");
    assert!(
        !p.materials
            .iter()
            .any(simpa_core::validate::is_placeholder_material),
        "{label}: no placeholder material"
    );
}

/// The teaching room at `BOX_SEED`.
fn box_run() -> Project {
    let mut p = teaching_room();
    p.solvers.spps.random_seed = BOX_SEED;
    p
}

fn box_long() -> Project {
    let mut p = box_run();
    p.solvers.spps.particles_per_source = BOX_LONG_PARTICLES;
    p
}

/// The corrected hall as the app would build it: the core's PLY import (m, z up), the library's
/// "20% absorbing" on every group in place of the placeholder, the hall fixture's first source
/// and first two receivers with nothing pinned, SPPS energetic at `HALL_PARTICLES` and
/// `HALL_SEED`, the import's own 6 octave bands.
fn hall_run() -> Project {
    use simpa_core::geometry::import::{
        ImportOptions, Unit, Up, import_file, library_material, reference_material,
    };
    let mut p = import_file(
        &repo("testdata/elmia_corrected.ply"),
        &ImportOptions::new(Unit::Metre, Up::Z),
    )
    .unwrap()
    .to_project("elmia_corrected");
    assert_eq!(p.bands, BandSet::default(), "octave bands, 125 Hz to 4 kHz");
    let n = p.bands.len();
    let reference = reference_material(HALL_MATERIAL).unwrap();
    assert_eq!(reference.name, "20% absorbing");
    let material = library_material(reference, MaterialId::from_u128(0x500), n);
    p.materials = vec![material.clone()];
    for g in &mut p.surface_groups {
        g.material = material.id;
    }
    let hall = schema::load(&repo("tests/fixtures/rooms/elmia_corrected.simpa")).unwrap();
    let mut source = hall.sources[0].clone();
    source.solver_id = None;
    source.id = SourceId::from_u128(0x600);
    p.sources = vec![source];
    p.point_receivers = hall.point_receivers[..2]
        .iter()
        .enumerate()
        .map(|(i, r)| {
            let mut r = r.clone();
            r.solver_id = None;
            r.id = PointReceiverId::from_u128(0x700 + i as u128);
            r
        })
        .collect();
    p.solvers.spps.method = simpa_core::schema::ComputationMethod::Energetic;
    p.solvers.spps.particles_per_source = HALL_PARTICLES;
    p.solvers.spps.random_seed = HALL_SEED;
    // Pinned to its historical 2 s for the same reason as `teaching_room`'s: `import_file`'s
    // project (`ImportedModel::to_project`) also starts from `Project::new`'s new-project
    // default, now 10 s (decision row 36).
    p.solvers.spps.duration_s = F64::new(2.0);
    p.check_integrity().expect("the hall run is consistent");
    p
}

/// TetGen 1.6.0 skipping two input facets as self-intersecting: `scene_mesh_skipped.face` with
/// the rows `mesh_project.rs`'s fake writes (facets 8 and 9 of the scene), exit 3. CRLF, as
/// `cmd.exe` reads a batch file; `(...)> file` so that no digit before `>` is read as a handle.
fn tetgen_skips() -> String {
    [
        "@echo off",
        "rem A stand-in for tetgen.exe, for the M11 gate's mesh-failure run (gate (e)) only:",
        "rem it plays TetGen 1.6.0 skipping two self-intersecting facets. The verified",
        "rem TetGen 1.5.0 never writes this file (docs/investigations/2026-09-29-m11/PLAN.md, 5).",
        "(echo 2 1& echo 1 1 2 3 8& echo 2 1 3 4 9)> scene_mesh_skipped.face",
        "exit /b 3",
        "",
    ]
    .join("\r\n")
}

fn the_run_fixtures_match_their_recipes() {
    let room = teaching_room();
    for (rel, p) in [
        (BOX_RUN, box_run()),
        (BOX_LONG, box_long()),
        (HALL_RUN, hall_run()),
    ] {
        check_or_write(rel, schema::to_json(&p).as_bytes());
        let loaded = schema::load(&repo(rel)).unwrap();
        assert_eq!(loaded, p, "{rel}");
        runnable(rel, &p);
    }
    // The box differs from the teaching room in its seed only, and the long box from the box in
    // its particle count only.
    let (b, l) = (box_run(), box_long());
    assert_ne!(room.solvers.spps.random_seed, b.solvers.spps.random_seed);
    let mut back = b.clone();
    back.solvers.spps.random_seed = room.solvers.spps.random_seed;
    assert_eq!(back, room);
    let mut back = l.clone();
    back.solvers.spps.particles_per_source = b.solvers.spps.particles_per_source;
    assert_eq!(back, b);
    assert!(l.solvers.spps.particles_per_source > b.solvers.spps.particles_per_source);
    // The hall: the PLY's 7,860 faces in 10 groups, closed, every group on "20% absorbing" with
    // the f32-widened 0.2 a `.proj` import gives it.
    let h = hall_run();
    let report = check(&h.geometry);
    assert_eq!(
        (
            report.counts.faces,
            report.counts.open_edges,
            h.surface_groups.len()
        ),
        (7860, 0, 10)
    );
    assert_eq!(h.materials.len(), 1);
    assert_eq!(h.materials[0].name, "20% absorbing");
    assert!(
        h.materials[0].absorption.iter().all(|a| a.get() == 0.2),
        "{:?}",
        h.materials[0].absorption
    );
    assert_eq!((h.sources.len(), h.point_receivers.len()), (1, 2));

    let bat = tetgen_skips();
    check_or_write(TETGEN_SKIPS, bat.as_bytes());
    assert!(bat.ends_with("exit /b 3\r\n"));
    assert_eq!(
        bat.matches("\r\n").count(),
        bat.matches('\n').count(),
        "CRLF only"
    );
}

fn teaching_room_matches_its_recipe_and_passes_every_check() {
    let room = teaching_room();
    check_or_write(ROOM, schema::to_json(&room).as_bytes());
    let loaded = schema::load(&repo(ROOM)).unwrap();
    assert_eq!(loaded, room);

    let report = check(&room.geometry);
    assert!(report.is_ok(), "{:?}", report.reasons);
    assert!((report.measures.enclosed_volume_m3 - 180.0).abs() < 1e-9);
    assert!((report.measures.area_m2 - 216.0).abs() < 1e-9);
    let issues = validate(&room);
    assert!(issues.is_empty(), "0 errors and 0 warnings: {issues:#?}");
    // The placeholder is upstream's `Default`; no material here is it (the app's rule counts
    // 6 of 6 groups assigned, tested in app/src-tauri/src/scene.rs).
    assert!(room.materials.iter().all(|m| m.name != "Default"));
    let used: Vec<usize> = room
        .surface_groups
        .iter()
        .map(|g| {
            room.materials
                .iter()
                .position(|m| m.id == g.material)
                .unwrap()
        })
        .collect();
    assert_eq!(used, [0, 1, 2, 2, 2, 3]);
}

fn materials_tsv_matches_its_recipe() {
    let text = tsv_text();
    check_or_write(TSV, text.as_bytes());
    let strict = Regex::new(STRICT_DECIMAL).unwrap();
    let rows: Vec<&str> = text.strip_suffix("\r\n").unwrap().split("\r\n").collect();
    assert_eq!(rows.len(), 6);
    for row in &rows {
        let cells: Vec<&str> = row.split('\t').collect();
        assert_eq!(cells.len(), 6);
        for c in cells {
            assert!(strict.is_match(c), "{c:?} fails the strict decimal grammar");
            let v: f64 = c.parse().unwrap();
            assert!((0.0..=1.0).contains(&v), "{c}");
        }
    }
    let odd = rows[MISREAD_CELL.0]
        .split('\t')
        .nth(MISREAD_CELL.1)
        .unwrap();
    let exact: f64 = odd.parse().unwrap();
    let lossy: f64 = serde_json::from_str(odd).unwrap();
    assert_ne!(exact.to_bits(), lossy.to_bits(), "{odd} must be misread");
    assert!(exact > 0.05 && exact < 0.95);
    assert_eq!(odd.trim_start_matches("0.").len(), 17);
    assert!(!text.contains('\n') || text.matches('\n').count() == text.matches("\r\n").count());
}

fn the_expected_project_is_the_core_s_output_for_the_committed_files() {
    let tsv = std::fs::read(repo(TSV)).expect("the committed TSV");
    let start = schema::load(&repo(ROOM)).expect("the committed teaching room");
    let values = read_tsv(&tsv);
    assert_eq!(values.len(), start.materials.len());
    let mut project = start.clone();
    let mut history = History::new();
    for (r, row) in values.iter().enumerate() {
        assert_eq!(row.len(), project.bands.len());
        for (b, &v) in row.iter().enumerate() {
            let op = Op::SetMaterialBand {
                material: start.materials[r].id,
                quantity: MaterialQuantity::Absorption,
                band: b,
                value: F64::new(v),
            };
            history.apply(&mut project, op).unwrap();
        }
    }
    let expected = schema::to_json(&project);
    check_or_write(EXPECTED, expected.as_bytes());
    // The start file differs from the expected one, so the gate's comparison can fail.
    assert_ne!(schema::to_json(&start), expected);
    // The misread cell reached the file with its exact bits.
    let back = schema::from_json(&expected).unwrap();
    let (r, b) = MISREAD_CELL;
    assert_eq!(
        back.materials[r].absorption[b].to_bits(),
        values[r][b].to_bits()
    );
}
