# Provenance: upstream tutorial 1

Source: `src/isimpa/resources/doc/tutorial/tutorial 1/tutorial_1.proj` at upstream commit 929a5c8 (sha256 `d778983d3f862b30877be39f4119857e10122e53249d88e5a68eb6be5d0a6a7e`). Regenerate with `python tools/fixture-gen/extract_tutorial1.py`.

Edits, all in config.xml:
- `workingdirectory` becomes the placeholder `__RUNDIR__`; the gate writes the run folder in.
- SPPS `random_seed` 0 (unseeded) becomes 1, so two builds can be compared exactly. A seed makes SPPS run single-threaded.
- SPPS `nbparticules` 150000 becomes 10000, so the comparison runs in seconds.

Meshes, the TetGen input and TetGen's output are unmodified. These are format and equivalence fixtures:
the acoustic values they produce are not evidence of anything.

| fixture | zip member | sha256 |
|---|---|---|
| `spps/mesh.cbin` | `instance2/report/SPPS/2019-06-07_11h58m41s/mesh.cbin` | `b99a341413487a10` |
| `tcr/mesh.cbin` | `instance2/report/Classical theory of reverberation/2019-06-07_11h57m58s/mesh.cbin` | `b99a341413487a10` |
| `spps/tetramesh.mbin` | `instance2/report/SPPS/2019-06-07_11h58m41s/tetramesh.mbin` | `8a6b3943dd47126b` |
| `tcr/tetramesh.mbin` | `instance2/report/Classical theory of reverberation/2019-06-07_11h57m58s/tetramesh.mbin` | `8a6b3943dd47126b` |
| `spps/config.xml` | `instance2/report/SPPS/2019-06-07_11h58m41s/config.xml` | `418510288bbfc7ae` |
| `tcr/config.xml` | `instance2/report/Classical theory of reverberation/2019-06-07_11h57m58s/config.xml` | `1d809d40d869947c` |
| `tetgen/scene_mesh.poly` | `instance2/temp/scene_mesh.poly` | `ab598bcab1548cfe` |
| `tetgen/scene_mesh.var` | `instance2/temp/scene_mesh.var` | `89355d3d2aa95bb6` |
| `tetgen/scene_mesh.1.node` | `instance2/temp/scene_mesh.1.node` | `6d45dd221c49f72b` |
| `tetgen/scene_mesh.1.ele` | `instance2/temp/scene_mesh.1.ele` | `a008e8b021a515a3` |
| `tetgen/scene_mesh.1.face` | `instance2/temp/scene_mesh.1.face` | `cff82acabfe834cc` |
| `tetgen/scene_mesh.1.neigh` | `instance2/temp/scene_mesh.1.neigh` | `66acb1484c2804ad` |
| `tetgen/scene_mesh.1.edge` | `instance2/temp/scene_mesh.1.edge` | `db4aaed3a2b7924c` |
