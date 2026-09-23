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
