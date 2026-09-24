# Run-folder fixtures

Inputs for `simpa run-folder` (docs/m5-m6-design.md, "Run folder"), each with the verdict
it must give. Generated; do not edit by hand. Regenerate on Windows, from the repo root:

```
python tools/fixture-gen/mkcfg.py tests/fixtures/upstream/lib_interface tests/fixtures/runs
python tools/fixture-gen/mklossy.py tests/fixtures/runs
python tools/fixture-gen/mktcr.py tests/fixtures/runs [--broken-hall build-clean/sim_output/tcr]
python tools/fixture-gen/mkstubs.py tests/fixtures/runs
powershell -File tools/fixture-gen/runsolvers.ps1 -Runs tests/fixtures/runs -Out <fresh scratch folder>
python tools/fixture-gen/mkexpected.py tests/fixtures/runs <that folder> --simpa <simpa.exe>
```

`mktcr.py` without `--broken-hall` keeps the committed `tcr_broken_hall`; its source folder
exists on one machine only. `mkexpected.py --check` compares instead of writing. Both
refuse a transcript or stub with a row's line on the wrong stream, or a "(no newline)"
row's line ending in one. `python tools/fixture-gen/test_fixture_gen.py` runs the
negative tests of these checks.

## A fixture

- `config.xml` has `workingdirectory="__RUNDIR__"`. `run-folder` replaces it with the
  absolute run folder plus `\`; every file it names is in the folder.
- `stub_*` folders add `stub.json` for the stub solver and reuse `spps_ok`'s inputs.
- `expected.json`:
  - `status` and `codes`: the verdict `run-folder` must give. `codes` are the decisive
    reasons and must all appear; a verdict may add consequences (for example
    `expected_file_missing` after a crash).
  - `codes_any_of`: groups of codes of which the verdict must carry at least one each.
    Only `tcr_broken_hall` has one: its near-flat tetrahedra are `degenerate_tets` or
    `inverted_tets` depending on mesh::verify's noise floor (`pre_launch.floor_sweep`).
  - `warnings`: WARN-class rows seen.
  - `pre_launch`: the checks `run-folder` makes before launch, computed by
    `mkexpected.py`'s references: the mesh check (decision 6 and the VerifyReport
    counts, with the noise floor it used), the band check (decision 11) and, for SPPS
    on a mesh that passed, the location check (SPPS's own f32 point test, emulated). A
    failed mesh check makes the verdict FAIL with `mesh_invalid` and the failing counts'
    codes; a failed band check adds `band_set_mismatch`, a failed location check
    `source_unlocatable` or `receiver_unlocatable`.
  - `observed`: what the real solver did when run anyway, launched as Part B says
    (fresh copy, cwd = the folder, argument `config.xml`): exit code, the solver's own
    status and codes, the decisive lines, the full transcript classified by row (paths
    under the run folder written as `__RUNDIR__`), and the post-run checks. For stubs
    it is the playback of `stub.json`.

## What the runs showed

Observed with the M1 solvers of `solvers/manifest.json` (`spps.exe` `5b2fac424a025b94`,
`classicalTheory.exe` `9ea03b323df1be9e`). Every survey behaviour cited in the receipts
reproduced. Beyond it:

- **`spps_oneband` is not caught by any Part B signal.** Exit 0, no FAIL line, 2,000
  particles per band, no loss: the 1000 Hz band's particles are all absorbed by the
  atmosphere at the first step. `run-folder`'s band check refuses it before launch with
  `band_set_mismatch` (decision 11).
- **`tcr_srcout` is caught after all**, by `nonfinite_result`: the direct field at R1 is
  -inf in every band, although TCR exits 0.
- **`spps_srcout` never reaches SPPS.** `run-folder`'s location check finds its source in
  no tetrahedron by SPPS's own f32 test and refuses it with `source_unlocatable`; run
  anyway, SPPS crashes with `0xC0000005`.
- **Exit `0xFFFFFFFF` is FAIL**, as the exit tables say (SC:264, SC:275). The rule at
  SC:278, "any exit at or above `0xC0000000` is CRASH", would make it `crash_other`.
- **Night Mode's broken-hall config fails on its own**: TCR prints `xml_property_missing`
  for `disable_absatmo_computation` and `absatmo`, and R2's direct field is -inf.
- **`tcr_broken_hall`'s tetrahedron counts depend on the noise floor.** With the floor on
  |6V| / (longest edge)^3 at 0, 2^-26, 2^-23, 2^-20, 2^-17, it has 0, 38, 65, 82, 138
  degenerate and 22, 8, 0, 0, 0 inverted tetrahedra. Every floor gives one of the two codes,
  so the verdict must carry one (`codes_any_of`).
- **Rows the contract had not seen run:** `degenerate_tetrahedron` (exit 1, no
  newline), `tetra_mesh_empty`, `ground_height`, `source_moved_off_vertex` and
  `source_on_surface` all print exactly as the table says. A source inside a floor
  triangle of the cube does trip the on-face check, unlike P2's `src_face`.

## Cases

| case | built as | status | codes | solver alone |
|---|---|---|---|---|
| `spps_ok` | the survey's run_ok: cube.cbin, cube_mesh.mbin, 2 bands, 2,000 particles, seed 1 | OK | - | same |
| `spps_noeps` | spps_ok without the trans_epsilon attribute | FAIL | xml_property_missing | same |
| `spps_oneband` | source spectrum with the 1000 Hz band only | FAIL | band_set_mismatch | OK  (exit 0x00000000) |
| `spps_dirmiss` | balloon source (directivite 5), directivity_file="nope.txt", no such file | FAIL | directivity_not_open | same |
| `spps_dirempty` | balloon source with no directivity_file attribute | CRASH | xml_property_missing, crash_access_violation | same |
| `spps_mat0miss` | faces use material 0; the config declares only 5 | CRASH | crash_access_violation | same |
| `spps_mat7miss` | every face's idMat set to 7; the config declares only 5 | FAIL | material_missing, exit_nonzero | same |
| `spps_srcout` | source at x = 12 m, outside the 5 m cube | FAIL | source_unlocatable | CRASH crash_access_violation (exit 0xC0000005) |
| `spps_unreadable_mesh` | mesh.cbin cut to its first 100 bytes (the survey's fx/trunc.cbin) | FAIL | mesh_invalid | FAIL scene_mesh_unreadable (exit 0x00000000) |
| `spps_nomesh` | no tetramesh.mbin | FAIL | mesh_invalid | FAIL tetra_mesh_unreadable (exit 0x00000000) |
| `spps_emptymesh` | tetramesh.mbin with T = 0, N = 0 (8 bytes) | FAIL | mesh_invalid, uncovered_scene_faces | FAIL tetra_mesh_empty, tetra_mesh_unreadable (exit 0x00000000) |
| `spps_degenerate` | tetramesh.mbin with tetrahedron 0's corner D set to corner A | FAIL | mesh_invalid, degenerate_tets | FAIL degenerate_tetrahedron, exit_nonzero (exit 0x00000001) |
| `spps_gradient` | blin = 0.001 (a sound-speed gradient) | OK | - | same |
| `spps_srcvertex` | source at the cube corner (0, 0, 0), a mesh vertex | OK | - | same |
| `spps_srcface` | source at (1.2, 1.7, 0), on floor triangle 0 | FAIL | source_on_surface | same |
| `spps_lossy` | spps_ok with every neighbour link in tetramesh.mbin set to -2 (mklossy.py) | FAIL | mesh_invalid, unmarked_boundary_faces | FAIL particle_loss_reported (exit 0x00000000) |
| `tcr_ok` | spps_ok's folder, run by TCR | OK | - | same |
| `tcr_mat7miss` | spps_mat7miss's folder, run by TCR | FAIL | material_missing, exit_nonzero | same |
| `tcr_srcout` | spps_srcout's folder, run by TCR | FAIL | nonfinite_result | same |
| `tcr_nomesh` | spps_ok's folder without tetramesh.mbin, run by TCR | FAIL | mesh_invalid | FAIL tetra_mesh_unreadable, exit_nonzero (exit 0x00000001) |
| `tcr_broken_hall` | Night Mode's build-clean/sim_output/tcr/ (2026-09-08): config.xml, model.cbin, tetramesh.mbin | FAIL | mesh_invalid, unmarked_boundary_faces, uncovered_scene_faces | FAIL xml_property_missing (exit 0x00000000) |

| stub | status | codes | exit |
|---|---|---|---|
| `stub_config_path_missing` | FAIL | config_path_missing | 0x00000000 |
| `stub_degenerate_tetrahedron` | FAIL | degenerate_tetrahedron, exit_nonzero | 0x00000001 |
| `stub_source_not_located` | FAIL | source_not_located | 0x00000000 |
| `stub_particle_loss_unterminated` | FAIL | particle_loss_reported | 0x00000000 |
| `stub_scene_mesh_unreadable` | FAIL | scene_mesh_unreadable | 0x00000000 |
| `stub_tetra_mesh_unreadable` | FAIL | tetra_mesh_unreadable | 0x00000000 |
| `stub_tetra_mesh_empty` | FAIL | tetra_mesh_empty, tetra_mesh_unreadable | 0x00000000 |
| `stub_unclassified_line` | FAIL | stats_unreadable, expected_file_missing | 0x00000000 |

## Classifier coverage

Which fixture's output hits each row of docs/solver-contract.md Part B. "Through
run-folder" lists real cases that pass the pre-launch checks, so the solver
actually starts; "refused before launch" lists real cases that print the row only
when run directly.

| # | row | class | through run-folder | refused before launch | stubs |
|---|---|---|---|---|---|
| 1 | `progress` | PROGRESS | `spps_gradient`, `spps_noeps`, `spps_ok`, `spps_srcvertex`, `tcr_ok`, `tcr_srcout` | `spps_lossy`, `spps_oneband`, `tcr_broken_hall` | `stub_particle_loss_unterminated` |
| 2 | `spps_banner` | INFO | `spps_dirempty`, `spps_dirmiss`, `spps_gradient`, `spps_mat0miss`, `spps_mat7miss`, `spps_noeps`, `spps_ok`, `spps_srcface`, `spps_srcvertex` | `spps_degenerate`, `spps_emptymesh`, `spps_lossy`, `spps_nomesh`, `spps_oneband`, `spps_srcout`, `spps_unreadable_mesh` | `stub_config_path_missing`, `stub_degenerate_tetrahedron`, `stub_particle_loss_unterminated`, `stub_scene_mesh_unreadable`, `stub_source_not_located`, `stub_tetra_mesh_empty`, `stub_tetra_mesh_unreadable`, `stub_unclassified_line` |
| 3 | `tcr_banner` | INFO | `tcr_ok`, `tcr_srcout` | `tcr_broken_hall` | - |
| 4 | `tcr_loading` | INFO | `tcr_mat7miss`, `tcr_ok`, `tcr_srcout` | `tcr_broken_hall`, `tcr_nomesh` | - |
| 5 | `tcr_config_echo` | INFO | `tcr_mat7miss`, `tcr_ok`, `tcr_srcout` | `tcr_broken_hall`, `tcr_nomesh` | - |
| 6 | `tcr_step` | INFO | `tcr_ok`, `tcr_srcout` | `tcr_broken_hall` | - |
| 7 | `ground_height` | INFO | `spps_gradient` | - | - |
| 8 | `spps_output_start` | INFO | `spps_dirmiss`, `spps_gradient`, `spps_noeps`, `spps_ok`, `spps_srcvertex` | `spps_lossy`, `spps_oneband` | `stub_particle_loss_unterminated`, `stub_source_not_located`, `stub_unclassified_line` |
| 9 | `spps_end_of_calculation` | OK | `spps_dirmiss`, `spps_gradient`, `spps_noeps`, `spps_ok`, `spps_srcvertex` | `spps_lossy`, `spps_oneband` | `stub_particle_loss_unterminated`, `stub_source_not_located`, `stub_unclassified_line` |
| 10 | `xml_property_missing` | FAIL | `spps_dirempty`, `spps_noeps` | `tcr_broken_hall` | - |
| 11 | `scene_mesh_unreadable` | FAIL | - | `spps_unreadable_mesh` | `stub_scene_mesh_unreadable` |
| 12 | `tetra_mesh_unreadable` | FAIL | - | `spps_emptymesh`, `spps_nomesh`, `tcr_nomesh` | `stub_tetra_mesh_empty`, `stub_tetra_mesh_unreadable` |
| 13 | `tetra_mesh_empty` | FAIL | - | `spps_emptymesh` | `stub_tetra_mesh_empty` |
| 14 | `config_path_missing` | FAIL | - | - | `stub_config_path_missing` |
| 15 | `directivity_not_open` | FAIL | `spps_dirmiss` | - | - |
| 16 | `source_moved_off_vertex` | WARN | `spps_srcvertex` | - | - |
| 17 | `material_missing` | FAIL | `spps_mat7miss`, `tcr_mat7miss` | - | - |
| 18 | `degenerate_tetrahedron` | FAIL | - | `spps_degenerate` | `stub_degenerate_tetrahedron` |
| 19 | `source_on_surface` | FAIL | `spps_srcface` | - | - |
| 20 | `source_not_located` | FAIL | - | - | `stub_source_not_located` |
| 21 | `particle_loss_reported` | FAIL | - | `spps_lossy` | `stub_particle_loss_unterminated` |
| 22 | `unclassified_line` | WARN | - | - | `stub_unclassified_line` |
