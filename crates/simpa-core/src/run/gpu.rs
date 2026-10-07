//! SPPS on the GPU (decision 70, `docs/investigations/2026-10-06-gpu/A5-SPEC.md`): `spps-gpu.exe`,
//! our CUDA build of SPPS's walk (`solvers/spps-gpu`), reads SPPS's `config.xml` and writes SPPS's
//! files, so a GPU run is an SPPS run: [`SolverKind`](crate::schema::SolverKind) stays
//! `{Spps, Tcr}`, and every SPPS-keyed check and reader takes a GPU run unchanged. The device is a
//! run option ([`SppsDevice`]), never project data: a project made on a machine with CUDA opens on
//! one without.
//!
//! [`probe`] asks `spps-gpu.exe --probe` for the device: exit 0 with one line naming it
//! (`NVIDIA GeForce RTX 5070, sm_120, 48 SMs, 11.9 GiB, driver CUDA 13.2, runtime 13.2`), exit 1
//! with why there is none. [`probe_verified`] finds the executable with an
//! [`ExeSearch`](super::ExeSearch) and checks it against the verified build first: an executable
//! that is not `solvers/manifest.json`'s is never run, not even with `--probe`. A run on the GPU
//! records the device line in `run.json` (`RunManifest::gpu_device`).

use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};

use super::manager::{CancelTimer, ExeSearch};
use crate::bed::pe::{SolverManifest, check_solvers};
use crate::process::{self, CancelToken, Spec};

/// The executable's file name, as `solvers/manifest.json` lists it.
pub const SPPS_GPU_EXE_NAME: &str = "spps-gpu.exe";

/// The argument that asks for the device and nothing else.
pub const PROBE_ARGUMENT: &str = "--probe";

/// How long a probe may take before it is called unavailable. It measured 0.15 s on Grace
/// (2026-10-07); the CUDA runtime's first initialisation can take seconds on a cold driver.
pub const PROBE_TIMEOUT: Duration = Duration::from_secs(5);

/// Where SPPS runs: upstream's `spps.exe` on the CPU, or `spps-gpu.exe` on the GPU. A run option
/// passed with the run request, never saved in a project.
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum SppsDevice {
    #[default]
    Cpu,
    Gpu,
}

impl SppsDevice {
    /// `cpu` or `gpu`, as the CLI's `--device` and the app's run request spell it.
    pub fn parse(s: &str) -> Result<SppsDevice, String> {
        match s {
            "cpu" => Ok(SppsDevice::Cpu),
            "gpu" => Ok(SppsDevice::Gpu),
            other => Err(format!("unknown device '{other}': cpu or gpu")),
        }
    }
}

/// Runs `exe --probe` with `timeout`: the device line on exit 0, otherwise why there is none (the
/// exit and the executable's own words, a timeout, or a launch failure).
pub fn probe(exe: &Path, timeout: Duration) -> Result<String, String> {
    let cwd = exe
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    let spec = Spec {
        program: exe.to_path_buf(),
        args: vec![PROBE_ARGUMENT.into()],
        cwd,
    };
    let cancel = CancelToken::new();
    let mut out: Vec<String> = Vec::new();
    let outcome = {
        let _timer = CancelTimer::start(timeout, cancel.clone());
        process::run(&spec, &cancel, &mut |l| {
            let t = l.text.trim();
            if !t.is_empty() {
                out.push(t.to_string());
            }
        })
    }
    .map_err(|e| format!("{} could not be started: {e}", exe.display()))?;
    let said = out.join("; ");
    if outcome.cancelled {
        return Err(format!(
            "{} {PROBE_ARGUMENT} did not answer within {} s",
            exe.display(),
            timeout.as_secs_f64()
        ));
    }
    match outcome.exit_code {
        Some(0) if out.len() == 1 => Ok(said),
        Some(0) => Err(format!(
            "{} {PROBE_ARGUMENT} exited 0 but printed {} lines, not one device line: {said}",
            exe.display(),
            out.len()
        )),
        Some(c) => Err(if said.is_empty() {
            format!("{} {PROBE_ARGUMENT} exited {c}", exe.display())
        } else {
            format!("{said} ({PROBE_ARGUMENT} exited {c})")
        }),
        None => Err(format!(
            "{} {PROBE_ARGUMENT} ended without an exit code",
            exe.display()
        )),
    }
}

/// The verified `spps-gpu.exe` and its device line, or why SPPS cannot run on the GPU here: the
/// executable is not found by `search`, it is not `manifest`'s build (it is then never started),
/// or [`probe`] finds no device.
pub fn probe_verified(
    search: &ExeSearch,
    manifest: &SolverManifest,
    timeout: Duration,
) -> Result<(PathBuf, String), String> {
    let exe = search.find(SPPS_GPU_EXE_NAME).map_err(|e| e.to_string())?;
    let check = check_solvers(&[(SPPS_GPU_EXE_NAME, exe.as_path())], manifest)
        .pop()
        .expect("one check per executable");
    if !check.matches {
        return Err(format!(
            "{} is not the verified build: {}",
            exe.display(),
            check
                .detail
                .as_deref()
                .unwrap_or("its code sha256 is not solvers/manifest.json's")
        ));
    }
    let line = probe(&exe, timeout)?;
    Ok((exe, line))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn devices_parse_as_the_cli_spells_them() {
        assert_eq!(SppsDevice::parse("cpu"), Ok(SppsDevice::Cpu));
        assert_eq!(SppsDevice::parse("gpu"), Ok(SppsDevice::Gpu));
        assert!(
            SppsDevice::parse("cuda")
                .unwrap_err()
                .contains("cpu or gpu")
        );
        assert_eq!(SppsDevice::default(), SppsDevice::Cpu);
        assert_eq!(serde_json::to_string(&SppsDevice::Gpu).unwrap(), "\"gpu\"");
    }

    #[test]
    fn a_missing_executable_is_unavailable_with_where_it_was_looked_for() {
        let dir = std::env::temp_dir().join(format!("simpa-gpu-probe-{}", std::process::id()));
        let search = ExeSearch {
            explicit: None,
            solvers_dir: Some(dir.clone()),
            exe_dir: None,
        };
        let manifest = SolverManifest::parse(
            crate::bed::SOLVER_MANIFEST,
            crate::bed::pe::ManifestSource::Embedded,
        )
        .unwrap();
        let why = probe_verified(&search, &manifest, PROBE_TIMEOUT).unwrap_err();
        assert!(why.contains("spps-gpu.exe not found"), "{why}");
        assert!(why.contains(&dir.display().to_string()), "{why}");
    }

    #[test]
    fn an_executable_that_is_not_the_verified_build_is_never_started() {
        let dir = std::env::temp_dir().join(format!("simpa-gpu-unverified-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        // A PE file of the measured shape whose code sha256 is not the manifest's.
        std::fs::write(
            dir.join(SPPS_GPU_EXE_NAME),
            crate::bed::pe::tests::tiny_pe(7, &[13], 0),
        )
        .unwrap();
        let search = ExeSearch {
            explicit: None,
            solvers_dir: Some(dir.clone()),
            exe_dir: None,
        };
        let manifest = SolverManifest::parse(
            crate::bed::SOLVER_MANIFEST,
            crate::bed::pe::ManifestSource::Embedded,
        )
        .unwrap();
        let why = probe_verified(&search, &manifest, PROBE_TIMEOUT).unwrap_err();
        assert!(why.contains("is not the verified build"), "{why}");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn the_embedded_manifest_pins_spps_gpu() {
        let m = SolverManifest::parse(
            crate::bed::SOLVER_MANIFEST,
            crate::bed::pe::ManifestSource::Embedded,
        )
        .unwrap();
        assert_eq!(m.code_sha256[SPPS_GPU_EXE_NAME].len(), 64);
        assert_eq!(m.sha256[SPPS_GPU_EXE_NAME].len(), 64);
    }
}
