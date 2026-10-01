//! A stand-in `tetgen.exe` for the M11 gate's forced mesh-failure run (gate (e),
//! `tools/gates/m11.ps1`): plays TetGen 1.6.0 skipping two self-intersecting facets, exactly as
//! `tests/fixtures/ui/tetgen_skips.bat` did (`crates/simpa-core/tests/ui_fixtures.rs`'s
//! `tetgen_skips`, still the recipe that fixture is checked against). Backlog 54's CLI half made
//! `simpa run` verify every executable it launches by default, and a `.bat` is refused outright
//! (not a PE file) before the mesh stage this gate means to reach. This binary is real PE, so the
//! gate can register it by its own code sha256 in a `SIMPA_SOLVER_MANIFEST` override
//! (`mesh_run.rs`) under the name `tetgen.exe` and have it verify.
//!
//! Writes `scene_mesh_skipped.face` (two facets, 8 and 9 of the scene) to its working folder and
//! exits 3, as TetGen 1.6.0 does when it skips self-intersecting facets
//! (`docs/investigations/2026-09-29-m11/PLAN.md`, 5).

fn main() {
    std::fs::write(
        "scene_mesh_skipped.face",
        "2 1\r\n1 1 2 3 8\r\n2 1 3 4 9\r\n",
    )
    .unwrap_or_else(|e| {
        eprintln!("simpa-stub-tetgen-skips: scene_mesh_skipped.face: {e}");
        std::process::exit(1);
    });
    std::process::exit(3);
}
