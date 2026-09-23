# Test fixtures

Byte-exact: `.gitattributes` turns off line-ending conversion here. Acoustic values in these
files are format evidence only, never results to quote.

| Folder | Source |
|---|---|
| `upstream/tutorial1/` | Tutorial 1's SPPS and TCR inputs and its TetGen input. See `PROVENANCE.md` there. |
| `upstream/lib_interface/` | `src/lib_interface/tests/` at upstream `929a5c8`, unmodified: `cube.cbin`, `cube_mesh.mbin`, `rs_cut.csbin`, `test_import1.poly` |
| `upstream/python_bindings/` | `src/python_bindings/tests/` at upstream `929a5c8`, unmodified: `mesh.cbin`, `tetramesh.mbin`, `retrocompat_test.gabe`, `rs_cut.csbin` |
| `solver-outputs/tutorial1/` | Outputs of the **reference** solvers from the M1 gate run `target/gates/m1/20260923-043220` on the tutorial 1 fixture. `spps_50hz.pbin` comes from our M1 build, rerun on the same fixture with `nbparticules_rendu="20"` so it exports particles. |
| `solver-outputs/nightmode-2026-09-08/` | Night Mode's valid corrected-hall run (`build-clean/sim_output/`, 2026-09-08): the SPPS stats GABE, TCR `Main results.gabe`, and the TCR mesh's `model_skipped.face/.node` (a failed TetGen run, kept as a negative case) |
