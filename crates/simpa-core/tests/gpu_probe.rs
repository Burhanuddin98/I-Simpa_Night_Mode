//! A5 (decision 70): `spps-gpu --probe` through the core's probe, on the real executable. Ignored
//! by default: it needs `spps-gpu.exe` (the manifest's build) in `$SIMPA_SOLVERS_DIR` and a CUDA
//! device, which the solver folders of the other suites do not hold. Run it with
//! `cargo test -p simpa-core --test gpu_probe -- --ignored`.
//!
//! One test, in its own binary, because it sets `CUDA_VISIBLE_DEVICES` for the child processes
//! this process starts.

#[allow(dead_code)]
#[path = "common/paths.rs"]
mod paths;

use simpa_core::bed::SOLVER_MANIFEST;
use simpa_core::bed::pe::{ManifestSource, SolverManifest};
use simpa_core::run::ExeSearch;
use simpa_core::run::gpu::{self, PROBE_TIMEOUT, SPPS_GPU_EXE_NAME};

#[test]
#[ignore = "needs spps-gpu.exe in SIMPA_SOLVERS_DIR and a CUDA device (the A5 bed)"]
fn the_probe_names_the_device_and_says_why_when_there_is_none() {
    let exe = paths::solver_exe(SPPS_GPU_EXE_NAME);
    let manifest = SolverManifest::parse(SOLVER_MANIFEST, ManifestSource::Embedded).unwrap();
    let search = ExeSearch {
        explicit: Some(exe.clone()),
        solvers_dir: None,
        exe_dir: None,
    };
    let t0 = std::time::Instant::now();
    let (found, line) = gpu::probe_verified(&search, &manifest, PROBE_TIMEOUT).unwrap();
    println!(
        "device: {line} ({:.3} s, {})",
        t0.elapsed().as_secs_f64(),
        found.display()
    );
    assert!(
        line.contains(", sm_") && line.contains("driver CUDA"),
        "{line}"
    );

    // No device visible: exit 1 with its own words, never a device.
    // SAFETY: this binary runs one test; nothing else reads the environment concurrently.
    unsafe { std::env::set_var("CUDA_VISIBLE_DEVICES", "-1") };
    let why = gpu::probe(&exe, PROBE_TIMEOUT).unwrap_err();
    println!("no device: {why}");
    assert!(why.contains("no CUDA device"), "{why}");
    assert!(why.contains("--probe exited 1"), "{why}");
}
