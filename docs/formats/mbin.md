# `.mbin`: tetrahedral mesh

The volume mesh the solvers propagate in. The GUI writes it from TetGen's output
(`CObjet3D::SaveMaillage`, `isimpa/3dengine/Core/Objet3D_maillage.cpp:885`), and names it in
`config.xml` as `tetrameshFileName="tetramesh.mbin"`. SPPS and TCR read it through
`t_TetraMesh::LoadFile` (`lib_interface/coreTypes.cpp:168`), which calls `CMBIN::ImportBIN`.

Upstream: `lib_interface/input_output/importExportMaillage/mbin.h` and `mbin.cpp`,
namespace `formatMBIN`, class `CMBIN`. Receipts are into
`target/solvers/src-929a5c8/src/` (upstream `929a5c8`).

Rust: `crates/simpa-core/src/formats/mbin.rs`. Oracle: `oracle/dump_mbin.cpp`.

## Byte layout

Little-endian. Upstream writes the in-memory integers raw (`mbin.cpp:109-153`), which is
little-endian on every platform it builds for. There is no magic number, no version field and no
padding. The file is three sections back to back:

| Offset | Size | Type | Field | Upstream |
|---|---|---|---|---|
| 0 | 4 | `u32` (`Longb`) | `quantTetra`, the number of tetrahedra T | `t_FileHeader`, `mbin.cpp:43-47` |
| 4 | 4 | `u32` (`Longb`) | `quantNodes`, the number of nodes N | same |
| 8 | 12·N | nodes | N × `{f32 x, f32 y, f32 z}` (`Floatb`) | `t_binNode`, `mbin.h:55`; read `mbin.cpp:185-189` |
| 8 + 12·N | 100·T | tetrahedra | T × the 100-byte record below | `bintetrahedre`, `mbin.h:92`; read `mbin.cpp:193-211` |

The header holds the tetrahedron count **first**, although the nodes come first in the body.

One tetrahedron record, all fields `i32` (`Intb`):

| Offset in record | Field |
|---|---|
| 0, 4, 8, 12 | `vertices[0..4]`: node indices of the four corners (A, B, C, D) |
| 16 | `idVolume` |
| 20 + 20·f | face f (f = 0..3): `vertices[0..3]` (3 × `i32`), then `marker`, then `neighbor` |

A face (`bintetraface`, `mbin.h:75`) is `{i32 a, i32 b, i32 c, i32 marker, i32 neighbor}`,
20 bytes. 4 + 1 + 4 × 5 = 25 `i32` = 100 bytes.

**File size** of anything upstream's writer produces is exactly `8 + 12·N + 100·T`.

In memory upstream holds vertex indices as `long` (`ivec3`, `ivec4`, `Core/mathlib.h:558,632`),
32-bit on Windows, and narrows them to `Intb` on write (`mbin.cpp:140,148`). The Rust types are
`i32` throughout, the width on disk.

## Sentinels and meaning

| Field | Value | Meaning | Receipt |
|---|---|---|---|
| `marker` | `-1` | the face lies on no model face (interior face). Default. | `mbin.h:81` |
| `marker` | `≥ 0` | index of the model face in the scene's `.cbin`; the solver takes `&sceneMesh.pfaces[marker]` | `coreTypes.cpp:223-225` |
| `neighbor` | `-2` | no tetrahedron across this face. Default. TetGen's `.neigh` writes `-1` for none; the GUI subtracts 1 from every index while reading it | `mbin.h:82`, `Objet3D_maillage.cpp:435-442` |
| `neighbor` | `≥ 0` | index of the tetrahedron across the face | `coreTypes.cpp:230-231` |

The solver treats **any** negative `marker` or `neighbor` as "none" (it tests `>= 0`).

### Conventions of upstream's meshes

The GUI builds the faces of every tetrahedron `(A, B, C, D)` in one fixed pattern
(`Objet3D_maillage.cpp:182-185`):

| face | vertices | opposite |
|---|---|---|
| 0 | `B, D, C` | A |
| 1 | `C, D, A` | B |
| 2 | `A, D, B` | C |
| 3 | `B, C, A` | D |

Face `i` is the face opposite vertex `i`, and its neighbour is the tetrahedron across that
face. The winding sets the face normal the solver computes (`coreTypes.cpp:233`), which it
compares with the scene face's normal when attaching surface receivers
(`coreinitialisation.cpp:292`) and when it locates a source (`coreinitialisation.cpp:71-95`).

`(A, B, C, D)` is **not** TetGen's `.ele` row as written: the GUI stores each row `(a, b, c, d)`
as `(d, c, b, a)`, and each `.neigh` row reversed with it. See "How the GUI builds it" below.

### How the GUI builds it from TetGen's output

`CObjet3D::LoadMaillage` reads TetGen's `.node`, `.ele`, `.face` and `.neigh`
(`isimpa/3dengine/Core/Objet3D_maillage.cpp:54-488`), and `SaveMaillage(.., true)` writes the
`.mbin` from what it read (`:830-898`, called at `data_manager/projet.cpp:769`). Two steps change
TetGen's numbers on the way, and both are needed to reproduce upstream's files:

1. **Corner order `(d, c, b, a)`.** `LoadEleFile` builds the corners as
   `ivec4 sommets(ToInt(GetNextToken()) - 1, ...)`, four token reads as the arguments of one call
   (`:161-164`). C++ leaves the order of argument evaluation unspecified; MSVC, which builds
   upstream's Windows releases, evaluates right to left, so the row's first corner lands in `D`.
   `LoadNeighFile` reads the neighbours the same way (`:435-438`), so neighbour `i` stays the
   tetrahedron across the face opposite corner `i`, and TetGen's `-1` becomes `-2` by the same
   `- 1`. The cause is inferred from the code; the effect is measured (below). The permutation
   `(a d)(b c)` is even, so every tetrahedron keeps the sign of its orientation determinant.
2. **A float round trip through the GUI's GL frame.** `LoadNodeFile` reads each coordinate as a
   `double` narrowed to `float` (`Convertor::ToFloat`, `manager/sppsString.cpp:43-50`) and moves it
   into the GL frame with `CommonCoordsToGlCoords`; `GetTetraMesh(.., true)` moves it back with
   `GlCoordsToCommonCoords` (`3dengine/Core/Mathlib.h:50-67`), all in `f32`:
   `g = ((x - cx)·s, (z - cy)·s, (-y - cz)·s)`, then `(g.x/s + cx, -(g.z/s + cz), g.y/s + cy)`.
   `(cx, cy, cz, s)` is `UnitizeVar`, which `CObjet3D::Unitize` fits to the scene
   (`Objet3D.cpp:527-571`): the box of the scene's vertices in GL axes `(x, z, -y)`, **the last
   vertex left out** (the loop runs `v < size() - 1`), centre `(lo + hi) / 2.0` with the sum in
   `f32`, and `s = 2.0 / max(w, h, d)` divided in `f64` and narrowed to `f32`. Tutorial 1's box
   gives `(3, 1.5, -5, 0.2f)`. The round trip moves a coordinate by a few ulps at most, and a 0 in
   y always comes back as `-0.0`.

   **Which vertices.** Upstream fits `_pVertices`, its list of scene vertices, each time it
   replaces the list (a scene or project load, `Objet3D.cpp:441`; a cuboid built, `:480`; a
   corrected `.poly` reloaded, `:393`; a boundary mesh, `Objet3D_maillage.cpp:1100`; nothing else
   writes it), and hands the same list, verbatim and in order, to the solvers as the scene part
   of the `.cbin` (`ToCBINFormat`, `:775-779`, before the fitting zones' triangles) and to TetGen
   as the first nodes of the `.poly` (`_SavePOLY`, `:938-942`). We fit the vertices of our own
   `.cbin` (`mesh::Unitize::of_scene`): the same rule on the same file's list, so when our `.cbin`
   is upstream's, so is the frame. The lists can differ: upstream keeps a copy of a vertex per face
   corner for an STL (`stl.cpp:274-287`) or a cuboid it builds (`Objet3D.cpp:462-471`), and
   tutorial 1's `.cbin` holds 36 vertices where ours holds the 8 points. In such a list of a closed
   surface every point appears at least three times, so leaving its last copy out changes
   nothing, and the two frames differ exactly when our last vertex is the scene's only vertex at
   its minimum or maximum on some axis (a box has none). Where upstream keeps one copy of each
   point (a `.ply`; tutorial 3's `.cbin`, 40 vertices), its last is left out as ours is.
   Tutorials 1 and 3 give the same frame from upstream's list as from ours, and no upstream file
   at hand has a last vertex that decides the frame, so that rule rests on the code and on unit
   tests (`mesh/build.rs`).

Markers go to every tetrahedron face holding a `.face` row's three vertices (`:369-381`), and
`idVolume` is the `.ele` attribute unchanged (`:165, 868`).

**Evidence.** `crates/simpa-core/tests/mesh_mbin_parity.rs` builds tutorial 1's 2019 TetGen output
(`tests/fixtures/upstream/tutorial1/tetgen/scene_mesh.1.*`) with `core::mesh` and gets upstream's
2019 `tetramesh.mbin` (sha256 `8a6b3943dd47126b`, 234,492 bytes) byte for byte, except the 2,257
`idVolume` fields that decision 1 writes as 0 (every one of them is 1 in upstream's file, as is
every `.ele` attribute). With one step undone the comparison fails:
- TetGen's own corner order: all 2,257 tetrahedron records differ (47,524 of their 56,425 `i32`
  fields);
- no round trip: 230 of the 732 nodes differ, 198 of them in value by at most 2^-21 m
  (4.77e-7 m), the other 32 only in the sign of a zero.

The same test rebuilds tutorial 3's 2019 `.mbin` (3,285 tetrahedra, 835 nodes, five regions,
read from the upstream checkout's `tutorial_3.proj`) byte for byte with `build_mbin` given
upstream's own region ids, the frame fitted to the 40 scene vertices of upstream's own `.cbin`:
`(9.548741, 5, -4, 0.10472585)`. That scene is no box of integers, so the frame's `f32`
arithmetic is exercised for real. A frame one `f32` step off, or centred in the scene's axes
rather than the GUI's, fails the tutorial-1 comparison.

End to end, from the project: tutorial 1's box meshed by `mesh::mesh_project` with TetGen 1.5.0,
the mesher Burhan chose ("Our own build",
`docs/investigations/2026-09-23-upstream-meshing/DECISIONS.md`), gives TetGen's 2019 `.ele`,
`.face` and `.neigh` (the command-line trailer apart) and its `.node` (the sign of the zeros in y
apart: our `.poly` writes `0` where upstream's, back from the GL frame, wrote `-0`), and then
upstream's 2019 `.mbin`, byte for byte except `idVolume`. A TetGen 1.6.0 build meshes that box
into 6 tetrahedra, and the test says so.

Neither conversion changes an invariant below, which is why only a byte comparison can tell them
apart. The corner order still matters to the solver: on TetGen 1.6.0's 6-tetrahedron tutorial box
(`tests/fixtures/solver-outputs/tutorial1/tetgen_scene_mesh.1.*`) the source (3, 5, 1.8) lies on
an internal facet, and SPPS locates it in upstream's order and crashes with an access violation
in TetGen's (`crates/simpa/tests/cli_run.rs`).

### Invariants

The invariants of a usable `.mbin` (`docs/m5-m6-design.md`, decision 6). The meshes `core::mesh`
builds (`crates/simpa-core/src/mesh/build.rs`) keep them, and `mesh::verify` checks them:
- every face follows the table above, and face `i`'s neighbour is the tetrahedron across the face
  opposite corner `i`;
- a face with neighbour `-2` has a marker `≥ 0`: the hull is made of scene faces;
- a face with a marker `≥ 0` and a neighbour `≥ 0` lies on an internal facet, and the face across
  it, in that neighbour, carries the same marker. Upstream marks **both** sides of such a facet:
  each `.face` row goes to every tetrahedron face holding its three vertices, in any order
  (`Objet3D_maillage.cpp:369-381`);
- neighbours are mutual;
- every tetrahedron has `(A−D)·((B−D)×(C−D)) < 0`.

Two conventions of the meshes `core::mesh` builds depart from upstream's own, and nothing else
does (evidence above):
- **`idVolume`** is 0 for the room and a fitting zone's solver id (2 and up) inside it
  (decision 1). Upstream's meshes carry TetGen's attribute unchanged, so their room is 1, or the
  largest fitting id plus 1 (`tetgen.cxx:24223-24306`).
- **Box fitting zones** are TetGen facets with markers past the scene's faces; the builder writes
  those faces with marker -1 (decision 5), so they are plain tetrahedron-to-tetrahedron
  transitions and are not internal facets in the sense above.

The previous statement here, "a face has a marker `≥ 0` exactly when its neighbour is `-2`", was
measured on the four fixtures (18,488 faces), none of which has an internal facet. It holds only
for such meshes: a mesh with a `Surfaces` fitting zone, or any scene face with room on both
sides, has marked faces with a neighbour. The rest was measured there too: every face follows
the table exactly (same order, not only the same set), neighbours are mutual, and every
tetrahedron has the negative orientation.

Measured on meshes `core::mesh` built on 2026-09-23 (`crates/simpa-core/tests/mesh_*.rs`): the
box (6 tetrahedra), the box with a box fitting zone, the corrected hall (123,718 tetrahedra) and
the survey's cube all keep every invariant. Those were built in TetGen's corner order; the same
tests pass in upstream's order, and so do tutorial 1's 2019 mesh and its rebuild
(`mesh_mbin_parity.rs`, which also checks every orientation sign exactly in both orders).

None of this is checked by upstream's reader or by `read`: they are properties of the files, not
of the format. `generate` produces meshes that follow all of them.

## Versions

None. The format has no version field. The tutorial meshes, written by the GUI in 2019, read
correctly with the reader at `929a5c8` (their volumes and conventions check out, above and below),
so the layout has not changed since then.

## Failure modes

Upstream's reader checks one thing: that the file opens (`mbin.cpp:172`). It never checks its
stream after that, and returns `true` for any file that opened.

| Input | Upstream `ImportBIN` | Rust `read` / `read_file` | Oracle |
|---|---|---|---|
| file missing | `false` | `NotFound` | `error` |
| path is a directory | `false` | `Io` | `error` |
| shorter than 8 bytes | reads uninitialised header counts (`t_FileHeader` has no initialiser, `mbin.cpp:176-180`): undefined behaviour | `Truncated` | `error` (guard) |
| `8 + 12·N + 100·T` > file size | `new t_binNode[N]` and `new bintetrahedre[T]` with the file's counts, unchecked (`mbin.cpp:184,192`); a huge count throws `bad_alloc` or allocates gigabytes. Reads past the end fail silently: nodes stay `0,0,0`, tetrahedron corners are uninitialised locals (`mbin.cpp:195`). Returns `true` | `Truncated`, before reserving anything; the size sum is computed without overflow | `error` (guard) |
| bytes after the last tetrahedron | ignored | ignored | ignored |
| counts smaller than the body (e.g. T = 5 in a 6-tetrahedron file) | reads the prefix, returns `true` | reads the prefix | reads the prefix |
| `N = 0` or `T = 0` | reads an empty mesh | reads an empty mesh | same |
| vertex, face-vertex or neighbour index out of range | accepted | accepted by `read`; `validate` returns `Invalid` | accepted |
| repeated vertex in a tetrahedron | accepted | accepted by `read`; `validate` returns `Invalid` | accepted |

What the solver then does with a mesh upstream's reader accepted (`coreTypes.cpp:168-243`):
- `T = 0` or `N = 0`: prints "Tetrahedron file is empty" and fails (`:188`, `:241`).
- Corner index `≥ N`: an `assert` (`:205`), compiled out in release builds; then `nodes[...]`
  reads out of bounds.
- A tetrahedron with two equal corners: prints an error and calls `exit(1)` (`:215`).
- Face vertex `≥ N`, `marker ≥` number of scene faces, or `neighbor ≥ T`: out-of-bounds pointer
  arithmetic (`:225`, `:231`, `:233`), no check.

`validate(&Mesh)` checks everything above that the `.mbin` alone can decide: corner and
face-vertex indices in `0..N`, four distinct corners, `neighbor < T`, and both counts within
`u32`. It cannot check `marker`, which indexes the scene's `.cbin`.

### Writer

Upstream `ExportBIN` (`mbin.cpp:89-158`):
- never checks that the file opened: it writes into a failed stream and returns `true`. The GUI
  compensates by testing that the file exists afterwards (`Objet3D_maillage.cpp:892`).
- refuses a tetrahedron with two equal corners (`mbin.cpp:119-133`), but only **after** writing
  the header and all nodes, so a refused mesh leaves a partial file behind.

Rust `write(&Mesh) -> Vec<u8>` encodes any mesh byte-for-byte as `ExportBIN` would; it panics only
if a count exceeds `u32::MAX`. `write_file` runs `validate` first and writes nothing when it
fails, then reports I/O errors as `Io`.

## Deviations

- **Truncation is an error, not undefined behaviour.** Rust returns `Truncated`. The oracle adds
  one guard before calling upstream: it opens the file, reads the 8-byte header itself, and
  prints `error` unless the file holds at least `8 + 12·N + 100·T` bytes (64-bit arithmetic).
  Every file the guard passes is read by upstream's `ImportBIN` unchanged, and everything the
  oracle prints comes from `ImportBIN`'s arrays.
- **Index checks are separate from reading.** `read` accepts what upstream accepts, so the two
  agree on every complete file; `validate` and `write_file` refuse what the solver cannot index.
- **`write_file` is stricter than `ExportBIN`** (it validates ranges, and never leaves a partial
  file).

## Canonical dump

```
mbin
nodes <N>
node <x> <y> <z>                                        (N lines, file order)
tetrahedra <T>
tetra <a> <b> <c> <d> <idVolume> face <fa> <fb> <fc> <marker> <neighbor> face … face … face …
                                                        (T lines, file order, 4 faces each)
```

- `mbin` alone on the first line: the format has no version tokens.
- `<x> <y> <z>` are `f32_hex` (8 lowercase hex digits of the raw bits), so `-0.0` is `80000000`.
  The fixtures do contain `-0.0`.
- Every integer is signed decimal: `-1`, `-2` appear as such.
- Each tetrahedron is one line of 1 + 5 + 4 × 6 = 30 tokens: the word `tetra`, its four corners
  and `idVolume`, then for each face the word `face` and its five fields.
- On failure: Rust prints `error <kind>` (`notfound`, `truncated`, `io`; this reader never
  returns `version` or `invalid`), the oracle prints `error`.

The dump has `N + T + 3` lines. It holds every field `ImportBIN` fills: the two counts, every
node coordinate, and all 25 integers of every tetrahedron.

Example, the first lines of `upstream/lib_interface/cube_mesh.mbin`:

```
mbin
nodes 8
node 40a00000 80000000 00000000
node 00000000 80000000 00000000
…
tetrahedra 6
tetra 4 5 7 2 1 face 5 2 7 -1 5 face 7 2 4 -1 4 face 4 2 5 2 -2 face 5 7 4 10 -2
…
```

## Rust API

| Item | What it is |
|---|---|
| `Mesh { tetrahedra, nodes }` | the file (upstream `trimeshmodel`, same field order); `nodes: Vec<[f32; 3]>` |
| `Tetrahedron { vertices: [i32; 4], id_volume, faces: [TetraFace; 4] }` | `bintetrahedre` |
| `TetraFace { vertices: [i32; 3], marker, neighbor }` | `bintetraface`; `Default` is upstream's `-1` / `-2` |
| `read`, `read_file` | decode; errors `NotFound`, `Truncated`, `Io` |
| `validate` | index checks the solver lacks (`Invalid`) |
| `dump`, `dump_file` | canonical dump; `dump_file` prints `error <kind>` on failure |
| `write`, `write_file` | encode as `ExportBIN` does; `write_file` validates first |
| `generate(seed)` | a small valid conforming mesh: a jittered box of 1 to 27 cells split into 6 tetrahedra each (Freudenthal), nodes and tetrahedra shuffled, following every convention above |

Allocation: `read` reserves exactly `12·N + 100·T` bytes of mesh, which equals the body it has
already checked is present, so it stays within `2·len + 64 KiB` for any input.

## Evidence

| Fixture | T | N | Volume (f64 sum of \|tet\|) | Node extent |
|---|---|---|---|---|
| `upstream/lib_interface/cube_mesh.mbin` | 6 | 8 | 125.000 m³ | 5 × 5 × 5 m |
| `upstream/python_bindings/tetramesh.mbin` | 102 | 46 | 5999.9995 m³ | 20 × 30 × 10 m |
| `upstream/tutorial1/spps/tetramesh.mbin` | 2257 | 732 | 180.000 m³ | 6 × 10 × 3 m |
| `upstream/tutorial1/tcr/tetramesh.mbin` | 2257 | 732 | 180.000 m³ | 6 × 10 × 3 m (same bytes as `spps/`) |

Each volume equals its bounding box's to within f32 rounding, which is what a mesh that fills a
box gives: a check on the node coordinates and the connectivity together that does not rest on
this reader's own output alone. The cube's full content (all 8 nodes and
all 6 tetrahedra with their faces) is also asserted against upstream's own test,
`lib_interface/tests/io_test.cpp:123-169`.

`tests/mbin_golden.rs` asserts these values, the negative cases, byte-exact round trips of every
fixture, and `validate` / `generate`. `tests/mbin_fuzz.rs` runs 10,000 proptest cases each of
arbitrary bytes, of mutated `cube_mesh.mbin` and of mutated tutorial mesh through `read`, with
the allocation meter on.

Oracle agreement, 2026-09-23 (Rust `dump_file` against `oracle.exe dump mbin`, CRLF normalised):
4 of 4 fixtures identical; 200 of 200 `generate` seeds identical after `write_file`; 16 of 16 edge
cases agree (both read and identical, or both fail); 1000 of 1000 randomly mutated cubes agree
(369 read by both, 631 refused by both).
