# Provenance: the broken hall's TetGen set (arc item 12)

**Source.** Night Mode's local run folder `build-clean/sim_output/tcr/`, written 2026-09-08 05:57
by the old C++ GUI on branch `main`. `build-clean/` is ignored, so this copy is the only one in
git. The files were copied unmodified on 2026-09-23. The results, `config.xml` and `model.1.edge`
were left out.

**How it was made:**
- The GUI exported upstream's raw hall `elmia.ply` as `model.poly` and `model.cbin`.
  - `model.poly` has 955 nodes and 1,086 facets.
  - Its facet header is `1086 0`, so the facets carry no markers.
  - Its nodes are written with 6 significant digits.
- TetGen ran with the command in the trailer of every `.1.*` file:
  `build-clean\solvers\tetgen.exe -pq1.500000 -A -n -Y -T1e-7 ...\tcr/model.poly`.
  These were Night Mode's flags, not upstream's; see `project/solver.cpp:933` on `main`.
- TetGen skipped 535 of the 1,086 input triangles as self-intersecting. Their markers are all -1
  in `model_skipped.face` (header `535 1`) because the `.poly` carried none.
- TetGen then wrote a partial `.1.node`, `.1.ele` and `.1.face` (1,067 nodes, 6,294 tetrahedra,
  641 faces) and no `.1.neigh`. It exited with code 3.
- Night Mode's converter wrote `tetramesh.mbin` anyway (`project/solver.cpp` on `main`,
  "Compute neighbors ourselves if .neigh is missing", about :668-719):
  - it rebuilt the neighbours from the tetrahedra
  - it matched markers to scene faces by vertex triple
- The TCR solver ran on the result and exited 0.

The survey rows are `docs/rebuild-plan-raw-2026-09-23.json`, lines 254, 744 and 1057-1058.
The raw hall itself is arc item 12 in `docs/release-arc-plan.md`.

**What it is expected to show** (`crates/simpa-core/tests/mesh_verify.rs`, `broken_hall_folder`):

`verify_dir` with room id 0 finds:
- basename `model`
- 535 skipped facets, every marker -1
- codes, in this order: `tetgen_skipped_facets`, `neigh_missing`, `degenerate_tets`,
  `unmarked_boundary_faces`, `uncovered_scene_faces`

The `.mbin` checked against `model.cbin` gives:

| check | count |
|---|---|
| `degenerate_tets` | 67 (flat slivers; the largest has `\|6V\|` at 0.49 of its f32 floor) |
| `unmarked_boundary_faces` | 267 |
| `uncovered_scene_faces` | 338 of 1,086 |
| every other count | 0 |

The index, orientation, neighbour and marker checks pass. The neighbours are consistent because
Night Mode rebuilt them itself. The markers lie on their faces because they were matched by
vertex triple.

The 267 unmarked boundary faces are the mechanism of arc item 12. SPPS destroys a particle at a
tetrahedron face that has neither a material nor a neighbour (`CalculationCore.cpp:365-372`,
cited in the arc plan). That is how 599,982 of 600,000 particles were lost while the run
exited 0.

| file | bytes | sha256 |
|---|---:|---|
| `model.poly` | 38,591 | `117d6ef5f210db4d9e08abc400d9debee937174e241a8c9911e3e18b3fc45f5d` |
| `model.1.node` | 55,822 | `6d03c2e8b80826c7cdae41a8164ac04e031e6fc044e2fb57dd2f9a3dabd67a44` |
| `model.1.ele` | 239,354 | `5ba56926373a16b94d7ec52dd6452979a630fe19364e0a77bcbcb67eaad0ac3d` |
| `model.1.face` | 25,650 | `2757593c959045d3f89f2d161e26f83e48ef5082116dcf7f82851c4a4dbd89a3` |
| `model_skipped.face` | 11,625 | `403093aa8ae767aede05c0a6cb8c7910ec4dde39986fe020135da65420b677f2` |
| `model_skipped.node` | 55,822 | `6d03c2e8b80826c7cdae41a8164ac04e031e6fc044e2fb57dd2f9a3dabd67a44` |
| `model.cbin` | 40,268 | `da5061bb2547919a898dea64fc19674b1a687b4366318fd70279bb695949e120` |
| `tetramesh.mbin` | 642,212 | `c45122e2e7c86ce81b427a02053d15e282fbdf15aafe0f8130ad856e3c1f551c` |

Total: 1,109,344 bytes. `model_skipped.face` and `model_skipped.node` are byte-identical to
`solver-outputs/nightmode-2026-09-08/tcr_model_skipped.{face,node}`.
