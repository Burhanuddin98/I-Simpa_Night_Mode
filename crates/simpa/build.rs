//! `simpa-stub-preprocess` and `simpa-stub-solver` (`src/bin/`) stand in for `preprocess.exe` and
//! a solver in CLI tests (`tests/cli_run.rs`, `a_run_whose_preprocess_gives_up_records_the_warning`;
//! `tests/run_folder_fixtures.rs`'s `stub_*` cases): real PE binaries, so a `SIMPA_SOLVER_MANIFEST`
//! override can register either by its own code sha256 and have it verify (backlog 54's CLI half
//! checks every executable a run launches by default, `mesh_run.rs`). The code sha256 is defined
//! only for a PE with no debug directory or a POGO one, the shape the measured solver builds have
//! (`solvers/pe-fingerprint.ps1`, `bed::pe::code_sha256_of`); a normal MSVC link embeds a CodeView
//! entry instead, which the fingerprint refuses by design (a link it was never measured on).
//! `/DEBUG:NONE` on these two binary targets leaves no debug directory at all, without touching
//! any other binary's debug info (`cargo:rustc-link-arg-bin`, scoped to one target each).

fn main() {
    println!("cargo:rustc-link-arg-bin=simpa-stub-preprocess=/DEBUG:NONE");
    println!("cargo:rustc-link-arg-bin=simpa-stub-solver=/DEBUG:NONE");
}
