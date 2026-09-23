mod common;

// The tracking allocator lets the huge-count cases prove they reserve nothing. Its counters are
// per thread now (common/mod.rs); the SERIAL lock predates that and is kept as belt and braces.
#[global_allocator]
static A: common::Tracking = common::Tracking;

use std::path::Path;
use std::sync::{Mutex, MutexGuard};

use simpa_core::formats::FormatError;
use simpa_core::formats::pbin::{self, FILE_HEADER_SIZE, PARTICLE_HEADER_SIZE, TIME_STEP_SIZE};

static SERIAL: Mutex<()> = Mutex::new(());

fn serial() -> MutexGuard<'static, ()> {
    SERIAL.lock().unwrap_or_else(|e| e.into_inner())
}

const FIXTURE: &str = "solver-outputs/tutorial1/spps_50hz.pbin";
const FIXTURE_LEN: usize = 1780;
/// `nbTimeStep` of each particle in the fixture, in file order.
const FIXTURE_STEPS: [u32; 17] = [2, 7, 5, 2, 11, 6, 2, 13, 1, 5, 17, 1, 5, 14, 3, 4, 3];

fn fixture_bytes() -> Vec<u8> {
    std::fs::read(common::fixture(FIXTURE)).expect("fixture present")
}

fn put_u32(bytes: &mut [u8], at: usize, v: u32) {
    bytes[at..at + 4].copy_from_slice(&v.to_le_bytes());
}

/// A version-1 file header with the Windows lengths.
fn header(nb_particles: u32, nb_time_step_max: u32, time_step: f32) -> Vec<u8> {
    let mut b = Vec::new();
    for v in [nb_particles, 1, 28, 16, 8, nb_time_step_max] {
        b.extend_from_slice(&v.to_le_bytes());
    }
    b.extend_from_slice(&time_step.to_le_bytes());
    b
}

fn particle(b: &mut Vec<u8>, first: u16, steps: &[[f32; 4]]) {
    b.extend_from_slice(&(steps.len() as u32).to_le_bytes());
    b.extend_from_slice(&first.to_le_bytes());
    b.extend_from_slice(&[0, 0]);
    for s in steps {
        for v in s {
            b.extend_from_slice(&v.to_le_bytes());
        }
    }
}

fn is_truncated<T>(r: &Result<T, FormatError>) -> bool {
    matches!(r, Err(FormatError::Truncated { .. }))
}

#[test]
fn fixture_header_values() {
    let _g = serial();
    let bytes = fixture_bytes();
    assert_eq!(bytes.len(), FIXTURE_LEN);
    let f = pbin::read_file(&common::fixture(FIXTURE)).expect("fixture reads");
    let h = f.header;
    assert_eq!(h.nb_particles, 17);
    assert_eq!(h.format_version, 1);
    assert_eq!(h.file_info_length, 28);
    assert_eq!(h.particle_info_length, 16);
    assert_eq!(h.particle_header_info_length, 8);
    // SPPS writes the simulation's time-step count (IPROP_QUANT_TIMESTEP) here.
    assert_eq!(h.nb_time_step_max, 200);
    assert_eq!(h.time_step.to_bits(), 0x3c23_d70a); // 0.01 s as f32
    assert_eq!(h.time_step, 0.01);
}

#[test]
fn fixture_steps_account_for_every_byte() {
    let _g = serial();
    let f = pbin::read_file(&common::fixture(FIXTURE)).unwrap();
    let counts: Vec<u32> = f.particles.iter().map(|p| p.nb_time_step).collect();
    assert_eq!(counts, FIXTURE_STEPS);
    assert!(f.particles.iter().all(|p| p.first_time_step == 0));
    assert_eq!(f.steps.len(), 101);
    assert_eq!(FIXTURE_STEPS.iter().sum::<u32>(), 101);
    assert_eq!(
        FILE_HEADER_SIZE + 17 * PARTICLE_HEADER_SIZE + 101 * TIME_STEP_SIZE,
        FIXTURE_LEN
    );
    assert_eq!(f.byte_len(), FIXTURE_LEN);
    // No particle outlives the simulation.
    assert!(
        f.particles
            .iter()
            .all(|p| p.nb_time_step <= f.header.nb_time_step_max)
    );
    // iter() hands each particle exactly its own steps.
    let lens: Vec<usize> = f.iter().map(|(_, s)| s.len()).collect();
    assert_eq!(lens, FIXTURE_STEPS.map(|n| n as usize));
}

#[test]
fn fixture_first_and_last_step() {
    let _g = serial();
    let f = pbin::read_file(&common::fixture(FIXTURE)).unwrap();
    let bits = |s: &pbin::TimeStep| {
        [
            s.position[0].to_bits(),
            s.position[1].to_bits(),
            s.position[2].to_bits(),
            s.energy.to_bits(),
        ]
    };
    assert_eq!(
        bits(&f.steps[0]),
        [0x4038_84a2, 0x4101_7216, 0x402d_8e7e, 0x2cb6_225b]
    );
    assert_eq!(
        bits(&f.steps[100]),
        [0x405c_fa8e, 0x4108_90eb, 0x3fca_a495, 0x2cb6_225b]
    );
    let (last, steps) = f.iter().last().unwrap();
    assert_eq!(last.nb_time_step, 3);
    assert_eq!(bits(&steps[2]), bits(&f.steps[100]));
}

#[test]
fn fixture_dump_shape() {
    let _g = serial();
    let path = common::fixture(FIXTURE);
    let d = pbin::dump_file(&path);
    assert_eq!(d, pbin::dump(&pbin::read_file(&path).unwrap()));
    let lines: Vec<&str> = d.lines().collect();
    assert_eq!(lines.len(), 3 + 17 + 101);
    assert_eq!(lines[0], "pbin 1");
    assert_eq!(lines[1], "header 17 1 28 16 8 200 3c23d70a");
    assert_eq!(lines[2], "particles 17");
    assert_eq!(lines[3], "particle 2 0");
    assert_eq!(lines[4], "step 403884a2 41017216 402d8e7e 2cb6225b");
    assert_eq!(lines[120], "step 405cfa8e 410890eb 3fcaa495 2cb6225b");
    assert!(d.ends_with('\n') && !d.contains(" \n") && !d.contains('\r'));
}

/// Upstream's reader through the oracle, built on demand (`common/paths.rs`): an oracle that
/// cannot be built fails the test, never skips it.
fn oracle_dump(file: &Path) -> String {
    let out = std::process::Command::new(common::paths::oracle("pbin"))
        .arg("dump")
        .arg("pbin")
        .arg(file)
        .output()
        .expect("run the oracle");
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).replace("\r\n", "\n")
}

#[test]
fn oracle_agrees_on_fixture() {
    let _g = serial();
    let path = common::fixture(FIXTURE);
    assert_eq!(pbin::dump_file(&path), oracle_dump(&path));
}

#[test]
fn missing_file_is_not_found() {
    let _g = serial();
    let path = common::fixture("solver-outputs/tutorial1/no_such_file.pbin");
    assert!(matches!(
        pbin::read_file(&path),
        Err(FormatError::NotFound(_))
    ));
    assert_eq!(pbin::dump_file(&path), "error notfound\n");
}

#[test]
fn truncated_mid_particle() {
    let _g = serial();
    let bytes = fixture_bytes();
    // Inside the last particle's steps, inside a particle header, and at a particle boundary.
    for cut in [FIXTURE_LEN - 8, FIXTURE_LEN - 16, 28 + 4, 28 + 8 + 2 * 16] {
        let r = pbin::read(&bytes[..cut]);
        assert!(is_truncated(&r), "cut at {cut}: {r:?}");
    }
    // Every proper prefix of the file is truncated, the empty file and a partial header included.
    for cut in 0..FIXTURE_LEN {
        assert!(is_truncated(&pbin::read(&bytes[..cut])), "cut at {cut}");
    }
}

#[test]
fn particle_count_far_larger_than_file_reserves_nothing() {
    let _g = serial();
    for nb in [u32::MAX, 1 << 28, 18] {
        let mut bytes = fixture_bytes();
        put_u32(&mut bytes, 0, nb);
        let base = common::reset_peak();
        let r = pbin::read(&bytes);
        let peak = common::peak_since_reset(base);
        assert!(is_truncated(&r), "nb_particles {nb}: {r:?}");
        assert!(
            peak <= common::budget(bytes.len()),
            "nb_particles {nb}: peak {peak}"
        );
    }
    // Header only, claiming four billion particles.
    let bytes = header(u32::MAX, 200, 0.01);
    let base = common::reset_peak();
    let r = pbin::read(&bytes);
    let peak = common::peak_since_reset(base);
    assert!(is_truncated(&r), "{r:?}");
    assert!(peak <= common::budget(bytes.len()), "peak {peak}");
}

#[test]
fn step_count_far_larger_than_file_reserves_nothing() {
    let _g = serial();
    let mut bytes = fixture_bytes();
    put_u32(&mut bytes, 28, u32::MAX); // first particle's nbTimeStep
    let base = common::reset_peak();
    let r = pbin::read(&bytes);
    let peak = common::peak_since_reset(base);
    assert!(is_truncated(&r), "{r:?}");
    assert!(peak <= common::budget(bytes.len()), "peak {peak}");
}

#[test]
fn bytes_after_the_declared_particles_are_invalid() {
    let _g = serial();
    let mut bytes = fixture_bytes();
    bytes.push(0);
    assert!(matches!(pbin::read(&bytes), Err(FormatError::Invalid(_))));
    let mut bytes = fixture_bytes();
    put_u32(&mut bytes, 0, 16); // one particle fewer than the file holds
    assert!(matches!(pbin::read(&bytes), Err(FormatError::Invalid(_))));
    put_u32(&mut bytes, 0, 0);
    assert!(matches!(pbin::read(&bytes), Err(FormatError::Invalid(_))));
}

#[test]
fn other_versions_are_refused() {
    let _g = serial();
    for v in [0, 2, u32::MAX] {
        let mut bytes = fixture_bytes();
        put_u32(&mut bytes, 4, v);
        match pbin::read(&bytes) {
            Err(FormatError::Version { found, .. }) => assert_eq!(found, v.to_string()),
            other => panic!("version {v}: {other:?}"),
        }
    }
    // An LP64 (Linux, macOS) file: 8-byte fields, so bytes 4..8 are the high half of nbParticles.
    let mut lp64 = Vec::new();
    for v in [3u64, 1, 56, 16, 16, 200] {
        lp64.extend_from_slice(&v.to_le_bytes());
    }
    lp64.extend_from_slice(&0.01f32.to_le_bytes());
    lp64.extend_from_slice(&[0; 4]);
    assert!(matches!(
        pbin::read(&lp64),
        Err(FormatError::Version { .. })
    ));
}

#[test]
fn empty_file_and_empty_particles_are_valid() {
    let _g = serial();
    let f = pbin::read(&header(0, 0, 0.02)).unwrap();
    assert!(f.particles.is_empty() && f.steps.is_empty());
    assert_eq!(
        pbin::dump(&f),
        "pbin 1\nheader 0 1 28 16 8 0 3ca3d70a\nparticles 0\n"
    );

    // ParticuleIO::NewParticle without a position writes a zero-step particle.
    let mut b = header(3, 1, 0.02);
    particle(&mut b, 7, &[]);
    particle(&mut b, 65535, &[[1.0, -0.0, f32::NAN, 0.5]]);
    particle(&mut b, 0, &[]);
    let f = pbin::read(&b).unwrap();
    assert_eq!(f.byte_len(), b.len());
    assert_eq!(
        pbin::dump(&f),
        "pbin 1\nheader 3 1 28 16 8 1 3ca3d70a\nparticles 3\nparticle 0 7\n\
         particle 1 65535\nstep 3f800000 80000000 7fc00000 3f000000\nparticle 0 0\n"
    );
}

#[test]
fn padding_is_never_read() {
    let _g = serial();
    let original = pbin::read(&fixture_bytes()).unwrap();
    let mut bytes = fixture_bytes();
    bytes[28 + 6] = 0xab; // first particle header's padding
    bytes[28 + 7] = 0xcd;
    let f = pbin::read(&bytes).unwrap();
    assert_eq!(pbin::dump(&f), pbin::dump(&original));
}

#[test]
fn struct_sizes_other_than_the_windows_layout_are_invalid() {
    let _g = serial();
    // fileInfoLength, particleInfoLength, particleHeaderInfoLength, each off by one way or another.
    for (at, v) in [
        (8, 99),
        (8, 56),
        (12, 20),
        (12, 32),
        (16, 4),
        (16, 16),
        (16, 0),
    ] {
        let mut bytes = fixture_bytes();
        put_u32(&mut bytes, at, v);
        let r = pbin::read(&bytes);
        assert!(
            matches!(r, Err(FormatError::Invalid(_))),
            "{v} at {at}: {r:?}"
        );
    }
    // Upstream would read this one off the layout: after the empty particle it seeks to that
    // particle's header + particleHeaderInfoLength (part_io.cpp:229-233).
    let mut b = header(2, 1, 0.01);
    b[16..20].copy_from_slice(&12u32.to_le_bytes());
    particle(&mut b, 0, &[]);
    particle(&mut b, 0, &[[1.0, 2.0, 3.0, 4.0]]);
    assert!(matches!(pbin::read(&b), Err(FormatError::Invalid(_))));
}
