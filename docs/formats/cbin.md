# `.cbin`: scene mesh

The room as triangles. Each triangle carries a material id, a surface-receiver id and a fitting
(encombrement) id. The GUI writes it as `mesh.cbin` beside `config.xml`, and SPPS and TCR read
it at start-up.

| | |
|---|---|
| Upstream class | `formatCoreBIN::CformatBIN` in `lib_interface/input_output/bin.h` and `bin.cpp` |
| Reader | `ImportBIN`, `bin.cpp:117-147`, plus `ProcessNode` (`:236-271`), `ProcessNodeVertices` (`:273-289`) and `ProcessNodeGroup` (`:291-320`) |
| Writer | `ExportBIN`, `bin.cpp:149-229`, plus `writeNode` (`:322-336`) |
| Solver consumer | `initMesh`, `lib_interface/coreinitialisation.cpp:387-452` |
| GUI producer | `CObjet3D::ToCBINFormat` and `_SaveCBIN`, `isimpa/3dengine/Core/Objet3D_maillage.cpp:742-828` |
| Rust | `crates/simpa-core/src/formats/cbin.rs`: `read`, `read_file`, `dump`, `dump_file`, `write`, `write_file`, `generate` |
| Oracle | `oracle/dump_cbin.cpp`, which calls upstream's `ImportBIN` unchanged |

Line numbers refer to `target/solvers/src-929a5c8/src/`.

## What the reader exposes

`ImportBIN` fills an `ioModel` (`bin.h:91-94`) and nothing else:

| field | type | meaning |
|---|---|---|
| `faces[i].a`, `.b`, `.c` | `unsigned int` (u32) | vertex indices (`bin.h:80-82`) |
| `faces[i].idMat` | `unsigned int` (u32) | material id. 0 means none (`bin.h:83`). Upstream's own test writes -1, which is stored as 0xFFFFFFFF (`io_test.cpp:33`) |
| `faces[i].idRs` | `int` (i32) | surface-receiver id, -1 for none (`bin.h:84`) |
| `faces[i].idEn` | `int` (i32) | fitting id, -1 for none (`bin.h:85`) |
| `vertices[i]` | `float[3]` (f32 x, y, z) | `t_pos`, `bin.h:53-65` |

Group names, node padding and `firtSon` are skipped or read into locals and then dropped. They
are not part of the model, and they are not dumped.

The Rust types are `Model { faces, vertices }`, `Face { a, b, c, id_mat, id_rs, id_en }` and
`Vertex { x, y, z }`, in upstream's field order.

## Byte layout

Everything is little-endian, with no alignment padding between fields: upstream reads and writes
field by field, never whole structs. The layout below is what `ExportBIN` writes for V vertices
and F faces.

| offset | size | field | written as | reader |
|---|---|---|---|---|
| 0 | 4 | `majorVersion` u32 | 1 | must be 1 (`bin.cpp:134`) |
| 4 | 4 | `minorVersion` u32 | 0 | must be 0 (`bin.cpp:135`) |
| 8 | 2 | vertices node: `nodeType` u16 | 0 (`NODE_TYPE_VERTICES`, `bin.h:135`) | selects the node parser |
| 10 | 2 | padding | 0 (`bin.cpp:333`) | skipped, `seekg(2)` (`bin.cpp:243`) |
| 12 | 4 | `firtSon` u32 | 0 (`nodeHeadSize` is always 0) | read, never used |
| 16 | 4 | `nextBrother` u32 | 24 + 12V, the group node's offset (rewritten at `bin.cpp:188-192`) | see the node walk below |
| 20 | 4 | `nbVertex` u32 | V | |
| 24 | 12V | V × (x, y, z f32) | | |
| 24+12V | 2 | group node: `nodeType` u16 | 1 (`NODE_TYPE_GROUP`) | |
| 26+12V | 2 | padding | 0 | skipped |
| 28+12V | 4 | `firtSon` u32 | 0 | ignored |
| 32+12V | 4 | `nextBrother` u32 | 0: the last node | |
| 36+12V | 255 | `groupName` | zeros (`memset`, `bin.cpp:81`) | read and discarded (`bin.cpp:297`) |
| 291+12V | 1 | one more name byte | 0 (`bin.cpp:205`) | skipped, `seekg(1)` (`bin.cpp:298`) |
| 292+12V | 4 | `nbFace` u32 | F | |
| 296+12V | 24F | F × (a, b, c, idMaterial u32; idRs, idEn i32) | | order `bin.cpp:304-309` |

The file size is **296 + 12V + 24F**. cube.cbin (V = 8, F = 12) is 680 bytes, tutorial 1's
mesh.cbin (36, 12) is 1016 and python_bindings' mesh.cbin (42, 76) is 2624.

**Older writers left garbage in the ignored bytes.** In cube.cbin the padding holds `e3 38` and
`a0 40`, and the group name is non-zero. python_bindings' mesh.cbin has padding `12 00` and a
non-zero name. tutorial 1's mesh.cbin (2019) is clean, and our writer reproduces it byte for
byte. A reader must not check these bytes.

## The node walk

`ProcessNode` (`bin.cpp:236-271`) reads the rest of the file as a chain of nodes:

1. Read the 12-byte header: `nodeType`, 2 skipped bytes, `firtSon`, `nextBrother`.
2. `nodeType` 0 appends a vertices node's vertices to `model.vertices`. 1 appends a group node's
   faces to `model.faces`. Any other value makes the read fail (`bin.cpp:255-256`).
3. If `nextBrother` is 0, stop: the read succeeds. Any bytes after this node are never read.
4. If `nextBrother` is past the current position, seek to it. Bytes in between are skipped.
   A `nextBrother` at or behind the current position is **ignored**, and the next node is read
   where this one ended (`bin.cpp:259-267`).
5. Go to step 1.

So a file may hold several vertices and group nodes, in any order. They are concatenated, and
face indices refer to the concatenated vertex list. `ExportBIN` always writes exactly one
vertices node, then one group node.

## Sentinels and versions

- Version: 1.0 is the only one the source at 929a5c8 reads or writes. Anything else fails
  (`bin.cpp:39-40`, `:134-137`).
- Terminal node: `nextBrother` = 0. The vertices node `ExportBIN` writes can never have 0 there
  (it is at least 24), so the walk always reaches the group.
- `idRs` = -1: the face is not part of a surface receiver. `idEn` = -1: the face belongs to no
  fitting. `idMat` = 0: no material. The GUI writes 0 for faces of fittings the user drew
  (`Objet3D_maillage.cpp:811`).

## Failure modes

Upstream's reader never checks the stream state after a read. On a short or corrupt file it
carries on with whatever the failed read left: an uninitialised local, a partly overwritten
previous value, or a count from a corrupt header. What happens then depends on stack contents.

| input | upstream `ImportBIN` | Rust `read` |
|---|---|---|
| file missing | false | `NotFound` |
| version not 1.0 | false | `Version { found: "M.m" }` |
| file ends anywhere before the terminal node is complete | **often true, with a partial or garbage model.** Measured on every truncation: it accepts 554 of cube.cbin's 680 and 553 of tutorial 1's 1016. A 5-byte cube.cbin is accepted as an empty model, while an 8-byte one fails. cube.cbin cut at byte 500, inside its group, gives all 12 faces: 4 read whole, a fifth whose ids are left over from the fourth, then 7 copies of that fifth. | `Truncated` |
| `nbVertex` or `nbFace` larger than the rest of the file | `reserve(count)`, then `count` `push_back`s of stale values (`bin.cpp:280-287`, `:300-318`). Measured: cube.cbin with `nbFace` = 1,000,000 is **accepted** with 1,000,000 faces, the 12 real ones and 999,988 copies of the last. With `nbVertex` = 1,000,000 it fails, because the vertices node's `nextBrother` can no longer be reached. Memory grows by 24 bytes for each counted face: on a corrupted count, one oracle run reached 8.4 GB before it was stopped (2026-09-23). | `Truncated`, checked before anything is reserved |
| `nextBrother` past the end of the file | the seek succeeds, then the next header read fails, and the read returns false | `Truncated` |
| unknown `nodeType` | false | `Invalid` |
| face index ≥ number of vertices | **true.** The solver then reads `pvertices[index]` out of bounds (`coreinitialisation.cpp:417-421`). | `Invalid` (deliberate divergence) |
| zero vertices | true | accepted, as upstream does. The solver refuses it (`coreinitialisation.cpp:394`). |
| `idMat` naming no material in config.xml | true | accepted: this is a project error, not a format one. The solver prints an error and calls `exit(-1)` (`coreinitialisation.cpp:425-432`). |
| bytes after the terminal node | ignored | ignored |
| NaN, infinite or subnormal coordinates | kept bit-exact, signalling NaNs included (checked with the oracle) | kept bit-exact |

Wherever upstream's result is defined, the Rust reader gives the same model, and
`oracle/dump_cbin.cpp` shows it. That covers a complete chain of nodes, forward jumps over junk,
ignored backward links, several nodes of each kind, and trailing bytes. Where upstream reads past
the end, the Rust reader returns `Truncated`, and there the two are **expected to differ**.
Upstream prints a dump of garbage; Rust prints `error truncated`.

## Writer

`write` produces exactly what `ExportBIN` writes: header 1.0, one vertices node whose
`nextBrother` points at the group node, and one group node with a zeroed name and
`nextBrother` 0. Padding and `firtSon` are 0. `write_file` first refuses a model that `read`
would reject (a face index out of range) or that the 32-bit fields cannot hold (a group-node
offset or face count ≥ 2^32). In either case it returns `Invalid` without creating the file.
`write` does not validate. It panics only on a model with 2^32 faces or more than about 357
million vertices.

`generate(seed)` returns a small valid model, different for every seed: 1 to 40 vertices, 0 to 60
faces, coordinates that include -0.0 and subnormals, and ids that include the -1 and 0xFFFFFFFF
sentinels.

## Canonical dump

```
cbin 1 0
vertices <V>
<x> <y> <z>                         V lines, f32 bits as 8 hex digits, in model order
faces <F>
<a> <b> <c> <idMat> <idRs> <idEn>   F lines, decimal; idMat unsigned, idRs and idEn signed
```

- The first line is constant, because only version 1.0 is ever read.
- The vertex list comes before the face list: that is the order `ExportBIN` writes them in, and
  each face line then follows the vertices it indexes. In `ioModel` the field order is faces,
  then vertices.
- The order within each list is the model order: nodes in the order they are walked, records in
  file order within each node.
- On failure: `error <kind>` from Rust, with kind `notfound`, `truncated`, `version`, `invalid` or
  `io`, and `error` from the oracle.

cube.cbin dumps as:

```
cbin 1 0
vertices 8
40a00000 80000000 00000000
00000000 80000000 00000000
...
faces 12
0 1 2 0 -1 -1
...
6 7 4 0 -1 -1
```

(`80000000` is -0.0. It is in the file, and upstream's test cannot see it because it compares
with a tolerance, `bin.h:60-62`.)

## Parity with upstream's GUI

**What upstream's GUI writes.** `CObjet3D::ToCBINFormat` (`isimpa/3dengine/Core/Objet3D_maillage.cpp:742-819`):

- the scene's faces, group by group in the scene's own order, each with its vertex indices into
  the scene's vertex list, and `idMat`, `idRs` and `idEn` from its surface group, surface receiver
  and fitting (`ApplicationConfiguration::GetFaceLink`, `:746-774`);
- the scene's vertices in their own order, each converted from the GUI's OpenGL coordinates back
  to world coordinates in `f32` (`GlCoordsToCommonCoords`, `:777`). The GUI holds the scene
  centred and scaled into [-1, 1] (`CObjet3D::Unitize`, `Objet3D.cpp:527-571`), so every
  coordinate takes a 32-bit round trip: a world `y` of 0 comes back as `-0`, and a coordinate
  can move by about one unit in the last place of the scene's largest coordinate;
- then each fitting box the user drew: 12 triangles, 3 new vertices each, `idMat` 0, `idRs` -1,
  `idEn` = the box's id (`:783-815`).

**What ours writes.** `config_xml::scene_mesh`: the room (`config_xml::room_mesh`), the
project's faces in project order and its vertices narrowed to `f32` and then taken through the
same round trip in the scene's frame (`config_xml::GlFrame`: upstream's `UnitizeVar`, computed
with upstream's own `f32` and `f64` steps); then, per enabled box fitting zone, its 12 triangles
as upstream builds them (`config_xml::upstream_box_triangles`: `BuildModel` in OpenGL
coordinates, `e_scene_encombrements_encombrement_cuboide.h:113-165`), 3 new vertices each,
`idMat` 0, `idRs` -1, `idEn` the zone's id. A project imported from a `.proj` keeps the scene's
face order. The mesher's `.poly` takes its vertices from the room (`mesh::project_input`), as
upstream's `_SavePOLY` takes them through the same round trip (`Objet3D_maillage.cpp:942`), and
its markers index the room's faces.

**Measured** (`crates/simpa-core/tests/parity_inputs.rs`, against every run folder stored in
upstream's tutorials at 929a5c8: tutorial 1's SPPS and TCR runs, tutorial 3's three SPPS runs;
tutorial 2 and `Industrial.proj` store none).

*The round trip, from vertices that have not taken it.* These fail without `GlFrame`:

| Input | Result |
|---|---|
| `tutorial_1.proj` imported (`simpa import-proj`) | All 12 faces equal upstream's corner for corner (`f32` bits, in face order a, b, c), every `idMat` equal, `idRs` 3503 is our 0. Without the round trip, 18 corners of 8 faces differ (`+0` where upstream has `-0`). Our vertex list is the welded one: 8 vertices against upstream's 36 (one copy per face), holding exactly upstream's 8 distinct vertices |
| `tests/fixtures/rooms/tutorial1_box.simpa`, and `tutorial_1.proj` imported: the mesher's input | `scene_mesh.poly` and `scene_mesh.var` byte-identical to the ones upstream's GUI wrote in 2019 (`tests/fixtures/upstream/tutorial1/tetgen/`). Without the round trip the `.poly` differs |
| tutorial 1's TetGen output `temp/scene_mesh.1.node`, through `GlFrame` of the project's `sceneMesh.bin`: upstream's `.mbin` path (`LoadNodeFile`, `GetTetraMesh`) | Both runs' `tetramesh.mbin` nodes, 2,196 of 2,196 coordinates bit for bit. Only narrowed to `f32`, 229 differ by value (268 by bits). This is the one stored case where the round trip moves values; on the scene meshes above it only turns `+0` into `-0` |

*Writer fidelity, not the round trip.* In these the vertices were read from upstream's own
`.cbin`, which already took the round trip, and taking them through it again changes none of
them, so they pass with or without `GlFrame` (checked 2026-09-24 by switching it off). They show
the layout, the face order and the ids:

| Input | Result |
|---|---|
| `tests/fixtures/projects/tutorial1.simpa` (tutorial 1's config and `.cbin` imported) | **Byte-identical** to upstream's `mesh.cbin`, 1,016 bytes, once our receiver id 0 is written as upstream's 3503 |
| tutorial 3 (its config and its `.cbin`, imported) | All 76 vertices and 100 faces equal bit for bit, the drawn box's 12 faces included; `idEn` 2083 and 1930 are our 3 and 2 |
| `tutorial_3.proj` imported, each run's saved project (`parity_tutorials.rs`, `tutorial_3`) | **Byte-identical** to each run's `mesh.cbin`, 3,608 bytes, once upstream's `idEn` 1930 and 2083 are read as our 2 and 3 through the map the import records: the 40 scene vertices and 88 faces, and the box built from its stored corners `ba` (13, 4, 0) and `hc` (18, 1, 1.2) as faces 88 to 99 with their 36 vertices |
| upstream's own `sceneMesh.bin` of tutorial 3, through `GlFrame` | The run's 40 scene vertices, bit for bit (plain narrowing gives the same 40; a frame one `f32` step off in scale moves 4) |

Each check has its refusal in the same test: one vertex one `f32` step off is reported at every
corner that names it; one face's `idMat` changed is reported; an id left unmapped is reported.

*The mesher's box zones.* A box fitting zone's corners take the same round trip as the scene
(`mesh::project_input`), as upstream's drawn boxes do (in through
`e_scene_encombrements_encombrement_cuboide.h:180`, out through `Objet3D_maillage.cpp:984-990`).
The round trip works coordinate by coordinate, so a box face flush with a wall stays in the
wall's plane: the tutorial box with its west wall moved to x = 0.37 and a box from x = 0.37 gives
the `.poly` 0.36999988555908203 for the wall and for the box's 4 corners there, where the corners
only narrowed, as the mesher wrote them before, would be 0.3700000047683716
(`a_box_zone_flush_with_a_wall_lies_in_the_walls_plane`).

*What the solvers make of it.* Same-seed runs of our M1 SPPS and TCR builds, upstream's inputs
against ours, every output file compared byte for byte (a `.csbin` decoded, since its padding
differs from run to run):

| Comparison | Result |
|---|---|
| Tutorial 1's run config and `.mbin`, with upstream's `mesh.cbin` and with our welded one from the `.proj` import (`the_welded_scene_mesh_gives_upstreams_output`) | SPPS: 65 of 65 output files identical; TCR: 87 of 87. Refusal: our mesh with face 3's material 22 changed to 21 gives 65 and 59 files that differ |
| Tutorial 3, each of the three runs: upstream's config (the two edits applied) with its `.cbin` and `.mbin`, against ours written back from it (`tutorial3_written_back_gives_upstreams_output`) | 24 of 24 output files identical in each run, once the cutting plane's `xmlIndex` 951 is read as our 0. Refusal: our first fitting zone absorbing 0.9 gives 24 that differ |
| `tutorial1.simpa` written, against upstream's config (`config_xml_solver.rs`, `tutorial1_runs_clean_and_matches_upstreams_own_configuration`) | Every output file identical, the `.csbin` files once `xmlIndex` 3503 is read as our 0 |

A stored run's SPPS config has `random_seed` 0, which seeds from the clock and runs a thread per
band, so no two runs agree; each comparison sets it to 1 and `nbparticules` to 10,000 on both
sides, as `tests/fixtures/upstream/tutorial1/PROVENANCE.md` does.

What the round trip is worth to the solvers on these tutorials: with `GlFrame` switched off in
`scene_mesh` (probe, 2026-09-24, reverted), the welded tutorial-1 comparison still gives 65 of
65 and 87 of 87 identical files. There the round trip only turns `+0` into `-0`, which the
solvers' arithmetic does not see. It is kept because it is what upstream writes, and because on
a scene whose frame moves values (the `.mbin` case above) the solver would read different
coordinates without it.

**What still differs, why, and what the solver sees.**

1. **Ids.** Our `idRs` and `idEn` are assigned from project order (receivers from 0, fitting zones
   from 2; `config_xml.rs`); upstream's are its GUI's element ids (3503; 1930, 2083). Those are
   session state, not part of the project: upstream gives every element a new id from a global
   counter each time it loads a project (`Element::Element` → `SetXmlId`, `element.cpp:134`), and
   overwrites the id the file holds (`:143-144`); loading first closes the current project, which
   restarts the counter at the number of live references (`LoadCurrentProject` → `CloseApp`,
   `projet.cpp:1855, 625`; `instanceManager.cpp:60-67`), and the counter counts every element of
   the tree, each property row and band included. The project has
   nowhere to hold them. The solvers only match these ids with `config.xml`, which carries the
   same ones, so every face gets the same receiver and fitting. The one place an id leaves the
   solver is a surface receiver's `.csbin` output, which records it as `xmlIndex`
   (`baseReportManager.cpp:40`): there 3503 reads 0. Measured above: the same-seed runs differ in
   nothing else. Making them equal needs upstream's ids stored in the project (a field like
   `Material::solver_id` on receivers and fitting zones, filled by both importers): a change to
   the `.simpa` format and to the 2026-09-23 convention that solver ids are assigned at export,
   so it is a decision, not a fix in this writer.
2. **The vertex list of a `.proj` import.** The import welds equal vertices (tutorial 1: 8 for
   upstream's 36), by design (`geometry::import::proj`: without welding no edge would be shared,
   and the checks would see an open mesh); upstream's own 2019 `scene_mesh.poly` holds the same 8.
   The solvers use a scene vertex only as a corner of the faces that name it
   (`coreinitialisation.cpp:417-422`, `CalculationCore.cpp:524-526`,
   `sppsInitialisation.cpp:99-101`, `TC_CalculationCore.cpp:33, 97, 117, 127, 524-526`), and their
   surface-receiver output takes its nodes from the tetrahedral mesh, not the scene
   (`UTILISER_MAILLAGE_OPTIMISATION`, `sppsTypes.h:7`; `TC_CalculationCore.cpp:390, 400`). So the
   solver sees the same triangles. Measured above: with the same seed, SPPS and TCR write every
   output file byte for byte as with upstream's 36-vertex file.
3. **One vertex of the bounding box.** Upstream's `Unitize` leaves its list's last vertex out of the
   bounding box (`v < size() - 1`, `Objet3D.cpp:542`); `GlFrame` takes every vertex, since a welded
   list has no such order to follow. They agree unless that one vertex alone sets an extreme of the
   box; then the frame differs, and some coordinates move by about one unit in the last place.
   None of the tutorials is such a case.
4. **Fitting boxes drawn by the user.** Upstream appends each box's 12 triangles to the `.cbin`,
   and so does ours since 2026-09-24 (above). Our mesher still puts a box's triangles in the
   `.poly` as 8 welded corners in the facet list (Part 2, not upstream's Part 5 user list) and
   writes them as plain tetrahedron-to-tetrahedron faces in the `.mbin`, markers -1, so no
   marker names the `.cbin`'s box faces (`docs/m5-m6-design.md`, decision 5). A particle crosses
   both (`CalculationCore.cpp:218, 226` for upstream's fitting faces), and TCR leaves the `.cbin`'s
   box faces, ours as upstream's, out of the room's surfaces: they carry a fitting
   (`TC_CalculationCore.cpp:11-17`). That the two meshes are the same physics is not yet shown by
   a run (decision 5). A zone imported from a `.cbin`, like tutorial 3's drawn box through
   `config_xml::import_upstream_with_mesh`, is a surface zone and keeps its 12 faces; imported
   from the `.proj`, it is a box.

## Verification (2026-09-23)

- `tests/cbin_golden.rs`: values from upstream's own tests, never from this reader.
  - cube.cbin: every vertex and face in `io_test.cpp:83-118`.
  - python_bindings' mesh.cbin: the 76 faces and the bounding box in
    `python_bindings/tests/check_retrocompat.py`.
  - Tutorial 1's mesh.cbin: the 6 × 10 × 3 m room, with material ids 21, 22 and 25 and receiver
    3503, all declared in its config.xml.
  - Round trips: `io_test.cpp:20-79` and 200 generated seeds. The writer reproduces tutorial 1's
    file byte for byte.
  - The node walk, and the negative cases: missing file, versions, every truncation length,
    oversized counts, a link past the end, an unknown node, an out-of-range index.
- `tests/cbin_fuzz.rs`: 10,000 cases each of arbitrary bytes, arbitrary bytes after a valid
  header, and 1 to 5 edits of a real file. Every 32-bit field of each fixture is also set to
  0xFFFFFFFF. No input panics, and the peak allocation stays within `budget(len)` on every case.
- Oracle: every `.cbin` fixture (4) matches exactly, and so do 1,000 generated models written by
  `write_file`. Six hand-built files covering the node walk and special floats also match.
  In 4,000 random edits of the fixtures, 1,333 were rejected by Rust as truncated, and the oracle
  was not run on those because upstream loops on their counts. Of the other 2,667, 1,736 agree
  (1,232 with the same model, 504 both failing). The remaining 931 are all out-of-range indices,
  which upstream accepts and Rust rejects. No other disagreement was found.
