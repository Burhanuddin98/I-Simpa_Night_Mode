//! A stand-in `preprocess.exe` for the CLI test that needs `preprocess_aborted` deterministically
//! (`crates/simpa/tests/cli_run.rs`, `a_run_whose_preprocess_gives_up_records_the_warning`),
//! without depending on the real repair loop's 100 passes on a geometry that triggers it
//! (`crates/simpa-core/src/mesh/preprocess.rs`'s module docs, upstream's tutorial 2). Backlog 54's
//! CLI half made `simpa run` verify its executables by default, same as the app
//! (`run_options`/`RunOptions::verify`), and that check refuses anything that is not a PE32+
//! image before it looks at what the file does — so the test's old `.bat` stand-in can no longer
//! reach the scenario it tests. This binary is real PE, cargo-built like `simpa-stub-solver`, so
//! the test can register its own code sha256 in a `SIMPA_SOLVER_MANIFEST` override (`mesh_run.rs`)
//! and have it verify.
//!
//! It ignores every argument (`preprocess.exe` is launched with one, `scene_mesh.poly`, which it
//! would otherwise rewrite in place; this binary never touches it, exactly as a real aborted run
//! leaves the file untouched), prints upstream's exact gives-up line
//! (`preprocess::ABORTED_LINE`, `Preprocess.cpp:100-105`) to stdout, and exits 0, as upstream's
//! `main` always does (`Preprocess.cpp:112-121`).

fn main() {
    println!(
        "Mesh reparation has been aborted. The algorithm enter into an infinite loop. Try to \
         stick coplanar faces or destroy manually."
    );
}
