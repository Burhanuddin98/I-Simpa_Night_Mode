# `.poly`: TetGen PLC input

The closed boundary of the scene, written as a TetGen piecewise linear complex (PLC) for
meshing. Upstream's driver is `formatPOLY::CPoly` in `lib_interface/input_output/poly/`:
`ExportPOLY` writes the file and `ImportPOLY` reads it back.

- Rust: `crates/simpa-core/src/formats/poly.rs`, with `read`, `read_file`, `dump`, `dump_file`,
  `write`, `write_file` and `generate`, plus `upstream_import_view` (see the upstream bug below).
- Oracle: `oracle/dump_poly.cpp`, built on `ImportPOLY`.

Receipts are into `target/solvers/src-929a5c8/src/`. Paths starting `poly.*` mean
`lib_interface/input_output/poly/poly.*`, and `tetgen.*` means `tetgen/tetgen.*`.

| Who | What | Receipt |
|---|---|---|
| GUI, meshing | builds a `t_model` from the scene and calls `ExportPOLY`; TetGen then reads the file | `isimpa/3dengine/Core/Objet3D_maillage.cpp:1043` |
| GUI, import | `ImportPOLY` of a `.poly` model | `isimpa/3dengine/Core/Objet3D.cpp:343`, `:1051` |
| `preprocess` (mesh repair) | `ImportPOLY`, repair, then `ExportPOLY` | `preprocess/computations.cpp:135`, `:648` |
| TetGen | `tetgenio::load_poly` reads Parts 1 to 4 and stops after the region list, so it never sees Part 5 | `tetgen.cxx:855-1247` |

There is no version field and no magic number. The file is text, in the classic ("C") locale.

## What the file holds (`formatPOLY::t_model`, `poly.h:74-81`)

| Field | C++ type | Rust (`poly::Model`) |
|---|---|---|
| `saveFaceIndex` | `bool` | `save_face_index: bool`: the facet list's boundary-marker flag |
| `userDefinedFaces` | `vector<t_face>` | `user_defined_faces: Vec<Face>`: Part 5, an I-Simpa extension |
| `modelFaces` | `vector<t_face>` | `model_faces: Vec<Face>`: Part 2 |
| `modelVertices` | `vector<dvec3>` (double) | `model_vertices: Vec<[f64; 3]>`: Part 1 |
| `modelRegions` | `vector<t_region>` | `model_regions: Vec<Region>`: Part 4 |

- `t_face` (`poly.h:55-68`) holds `indicesSommets`, an `ivec3` of `long` (32-bit on MSVC,
  `mathlib.h:615`) with **0-based** node indices, and `faceIndex`, an `unsigned int` boundary
  marker. In Rust it is `Face { vertices: [u32; 3], face_index: u32 }`.
- `t_region` (`poly.h:45-51`) holds `regionIndex` (`int`, TetGen's regional attribute),
  `dotInRegion` (`vec3`, which is **`float`**: `mathlib.h:54`, `:239`) and `regionRefinement`
  (`float`, TetGen's maximum tetrahedron volume). The constructor sets `regionRefinement = -1`,
  meaning no constraint. In Rust it is `Region { region_index: i32, dot_in_region: [f32; 3],
  region_refinement: f32 }`, and `Default` gives -1 as upstream does.
- **Holes are not kept.** `ImportPOLY` parses hole lines and discards them (`poly.cpp:365-372`).
  `ExportPOLY` always writes an empty hole list (`poly.cpp:216`).

## Layout as `ExportPOLY` writes it (`poly.cpp:177-250`)

The C++ writes `\n` through a text-mode `std::ofstream`, so every line ends in **CRLF** on
Windows. The byte-exact fixture `tests/fixtures/upstream/tutorial1/tetgen/scene_mesh.poly`,
written by upstream's GUI, has CRLF. `⎵` stands for one space.

```
# Part 1 - node list
<nV>⎵⎵3⎵⎵0⎵⎵0                                   poly.cpp:188
<i+1>⎵<x>⎵<y>⎵<z>                                poly.cpp:192   one line per node, i = 0..nV-1
                                                 poly.cpp:194   (empty line)
# Part 2 - facet list
<nF>⎵<1 if saveFaceIndex else 0>                 poly.cpp:200-203
1⎵⎵0⎵⎵<faceIndex>                                poly.cpp:208   per facet: one polygon, no hole, marker
3⎵⎵<a+1>⎵<b+1>⎵<c+1>                             poly.cpp:210   per facet: the triangle, 1-based
                                                 poly.cpp:214
# Part 3 - hole list
0                                                poly.cpp:216   always zero holes
                                                 poly.cpp:216
# Part 4 - region list
<nR>                                             poly.cpp:218
<r+1>⎵⎵<x>⎵⎵<y>⎵⎵<z>⎵⎵<regionIndex>⎵⎵<refinement> poly.cpp:223-228
                                                 poly.cpp:232
# Part 5 - user facet list
<nU>⎵⎵1                                          poly.cpp:236
1⎵⎵0⎵⎵<faceIndex>                                poly.cpp:243
3⎵⎵<a+1>⎵⎵<b+1>⎵⎵<c+1>                           poly.cpp:245-247   note: two spaces here, one in Part 2
```

The file ends right after the last line's CRLF. `nV` and `nU` are cast to `unsigned int`, while
`nF` and `nR` are `size_t`.

**Numbers.** The stream is imbued with the classic locale and has precision
`numeric_limits<double>::max_digits10` = 17 (`poly.cpp:180-181`), with the default float field.
The rules:

- A `double` prints as `printf("%.17g")`.
- A `float` (region point, refinement) is promoted to `double` first, so `0.1f` prints as
  `0.10000000149011612`.
- Integers print in decimal.

The Rust writer reproduces MSVC's UCRT spelling exactly, as a probe built with the oracle's
compiler showed on 2026-09-23:

- It uses 17 significant digits of the exact binary value, rounded **half to even**:
  2^-25 → `2.9802322387695312e-08`, and 1e15+0.25 → `1000000000000000.2`.
- Fixed notation applies when the decimal exponent X satisfies -4 ≤ X < 17, otherwise
  scientific. Trailing zeros are dropped, and the point too when nothing follows it.
- The exponent has at least two digits: `1e+20`, `1.234e-05`, `1e+100`.
- Zero prints as `0` and negative zero as `-0`.
- Non-finite values: `inf`, `-inf`, `nan`, `-nan(ind)` (the default negative quiet NaN,
  payload 0), `-nan` (negative quiet with payload), and `nan(snan)` / `-nan(snan)` (signalling).
  Neither reader accepts these back.

## How upstream reads it (`ImportPOLY`, `poly.cpp:257-431`)

`ImportPOLY` is a line state machine over `std::getline`. In text mode CRLF arrives as LF, and a
Ctrl-Z byte (0x1A) ends the file: the oracle read nothing of `scene_mesh.poly` with a Ctrl-Z in
its first comment (`poly_golden.rs::oracle_reads_a_ctrl_z_comment_as_the_end_of_the_file`). Each
line goes through an `istringstream` in the classic locale.

- **Comments:** a line containing `#` **anywhere** is skipped whole (`:297`).
- **Failure is silent.** `ImportPOLY` returns `false` only when the file cannot be opened
  (`:263-265`). A line that does not parse is skipped, and the function returns `true` whatever
  it kept (`:430`). A file of garbage text gives an empty model and success (oracle, 2026-09-23).
- **Header counts are trusted.** `reserve(sizeVertices)` (`:303`) and `reserve(sizeFaces)`
  (`:324`) run before any record is read. A header of `4294967295  3  0  0` aborts the process
  with 0xC0000409 (oracle, 2026-09-23).
- **What it ignores:**
  - the node number (`:312`): nodes are taken in file order and indices have 1 subtracted;
  - the dimension and attribute count (`:302`);
  - a node header's fourth field;
  - a facet's polygon and hole counts (`:333`);
  - the region number (`:383`);
  - the second field of the user facet header (`:392`).
- **Polygons:** a corner count of 3 gives one triangle. A count of 4 gives the triangles
  (a,b,c) and (c,d,a) with the same marker (`:342-346`). Any other count is counted as a facet
  and dropped (`:338-354`).
- **Numbers** are read by MSVC's `num_get`, always in decimal (`010` is 10):
  - `>> double` accepts hexadecimal (`0x10` is 16);
  - `>> unsigned` wraps `-1` to 4294967295;
  - out-of-range integers fail;
  - a real fails on overflow to infinity or when a non-zero literal underflows to zero, but
    subnormal results are accepted;
  - `inf` and `nan` fail.
- **Upstream bug, user facets (`:406-424`):** the user-facet loop compares its counter with the
  main facet count `sizeFaces`, not with `nbUseFace`. So after the first user facet the state
  stays in `USER_FACE_CONTENT_VALUE`:
  - the next facet header `1 0 <m>` parses as a polygon of 1 corner and is dropped;
  - the next `3 a b c` line becomes a face carrying the **first** user facet's marker.

  The result is that every user facet after the first takes the first one's marker. Receipt: the
  oracle reads user facets with markers 7 then 9 as `0 1 2 7` / `2 1 0 7`
  (`poly_golden.rs::user_facets_keep_their_own_markers` holds the input).

  `poly::upstream_import_view(&model)` is the model upstream reads back from `write(model)`, for
  finite values and node indices below 2^31: the same model with that marker collapse and
  nothing else. It holds for any file this reader accepts, since every accepted facet header is
  `1 0 <m>`. `poly_golden.rs::oracle_reads_generated_files_as_modelled` checks it against the
  oracle for 200 seeds, and `oracle_agrees_on_every_accepted_variant` for hand-made files.

  **For an oracle differential over generated files,** compare the oracle's dump with
  `dump(&upstream_import_view(&model))`. Against `dump(&model)`, the bug alone makes about half
  the seeds differ (497 of seeds 1 to 1000), because `generate` gives up to 3 user facets with
  independent markers. That is deliberate: a generator that avoided the bug would hide it.

## How TetGen reads it (`tetgenio::load_poly`, `tetgen.cxx:855-1247`)

TetGen is the consumer the file is written for. It opens the file with `fopen(.., "r")`
(`:875`), so on Windows it too reads CRLF as LF and stops at Ctrl-Z.

- **Lines:** `readnumberline` (`:2894-2914`) reads with `fgets` into `INPUTLINESIZE` = 2048
  bytes (`tetgen.h:70`), so at most 2047 characters at a time: a longer line is split and its
  tail read as the next line. It skips every byte until one that can start a number (a digit,
  `+`, `-`, `.`) or a `#`. A line where `#` comes first is a comment, and a line with neither is
  blank.
- **Fields:** `findnextnumber` (`:2926-2948`) ends a field only at space, tab, comma or `#`, then
  skips to the next byte that can start a number. A `#` ends the line's data.
- **Numbers:** integers are read by `strtol(.., 0)`, which reads `0x10` as 16, `010` as octal 8
  and `08` as 0 (`:902`, `:957`, `:986-993`, `:1014`, `:1034`, `:1147`, `:1189`). `long` is
  32 bits on Windows, so a value above 2147483647 clamps to 2147483647. Reals are read by
  `strtod` as `double` (`:121-134`, `:1204-1231`), the region's attribute included.
- **What it keeps:** the nodes, taking `firstnumber` from the first node's number
  (`load_node_call`, `:106-114`); the facets, each with its polygons and holes, and a marker
  when the facet flag is 1; the holes; and the regions, 5 doubles each.
- **Values it treats specially:** a node count of 0 means "read the nodes from a `.node` file"
  (`:925-931`); fewer than 4 nodes is an error (`:939-943`); a facet count of 0 ends the reading
  with success, before the holes and regions (`:957-961`).

## This reader (`poly::read`)

**What "accepted" means.** A file is accepted only when `ImportPOLY` reads it exactly as this
reader does: the same model, apart from the user-facet marker bug above. The oracle checks this
(see Verification). Where `ImportPOLY` would read part of a file and skip the rest silently, this
reader refuses the file.

The reader also refuses every spelling on which TetGen's tokenizer would see different fields
from `ImportPOLY`'s. So TetGen reads the same numbers from Parts 1 to 4 of an accepted file.
What TetGen then makes of some of those numbers differs, as listed under "What TetGen does
differently". Agreement with TetGen on those values is not claimed.

**Lines.** Lines split on `\n`. One `\r` just before it, or at the very end of the file, is the
line end, as text mode reads it. So CRLF and LF both work, and the last line needs no newline.
Then:

- **Ctrl-Z** (0x1A) anywhere is `Invalid`. Both upstream readers stop at it on Windows, and
  read on elsewhere.
- **A line longer than 2047 bytes**, not counting its line end, is `Invalid`, comment or not,
  because TetGen would read its tail as another line. Receipt (2026-09-23): with the comment
  `# Part 2 - facet list` padded with `9`s to 2048 bytes, TetGen's `load_poly` fails, because it
  reads the last `9` as the facet count. `ImportPOLY` reads the whole model. At 2047 bytes both
  readers read the whole model.
- **Comments.** A line holding `#` is a comment when no byte before its first `#` can start a
  number (a digit, `+`, `-`, `.`). Both readers skip exactly such a line: `ImportPOLY` skips
  any line holding `#`, and TetGen skips the line when `#` comes first. This covers a UTF-8
  byte order mark before the first comment. With a number-like byte before the `#`, the line
  is `Invalid`: TetGen would read the number, and `ImportPOLY` would drop the line.
- **Fields.** Any other line splits into fields at spaces and tabs only. A `\r` inside the line,
  `\v` or `\f` is `Invalid`, because `ImportPOLY`'s `>>` separates fields there and TetGen does
  not. A comma stays inside its field, which then is not a number, so the line is `Invalid`.
- A line of no fields is blank and skipped.
- A line with more than 6 fields is `Invalid`.

**Numbers.**

| Kind | Where | Accepted |
|---|---|---|
| int (C++ `int`/`long`) | node, hole and region numbers; polygon and hole counts; corners; region attribute; list counts in `int` | `[+-]?[0-9]+` with no leading zero, within i32 |
| unsigned (C++ `unsigned int`) | node count, facet count, facet flag, facet marker | `+?[0-9]+` with no leading zero, within u32. A minus sign is `Invalid`, since MSVC would wrap it |
| real (`double`/`float`) | node and hole coordinates (f64); region point and constraint (f32) | `[+-]?(D+(.D*)?\|.D+)([eE][+-]?D+)?`, rounded correctly into the target type |

- An integer with a leading zero (`03`, `010`, `00`) is `Invalid`: `ImportPOLY` reads it in
  decimal, TetGen's `strtol` in octal. A lone `0` is fine, with a sign where the field allows
  one (`+0` anywhere, `-0` in an int field).
- A real's leading zeros are decimal to both readers and accepted.
- A real that overflows to infinity, or a non-zero real that underflows to zero, is `Invalid`,
  as MSVC fails there. Subnormals are kept.
- Hexadecimal, `inf` and `nan` are `Invalid`.

**Sections, in order.**
1. **Node header:** `nV dim nattr [marker]`, 3 or 4 fields. `dim` must be 3, `nattr` 0, and
   `marker` 0 if present.
2. **nV node lines:** `i x y z`, exactly 4 fields. `i` must be the line's 1-based position:
   `ImportPOLY` ignores it, and TetGen would take a first number of 0 as 0-based numbering.
3. **Facet header:** `nF flag`, exactly 2 fields. `flag` must be 0 or 1 and becomes
   `save_face_index`.
4. **nF facets.** Each is two lines:
   - a facet header of exactly 3 fields: 1 polygon, 0 holes, and the marker (unsigned);
   - a polygon of `3 a b c` or `4 a b c d`, with exactly corners+1 fields. Each corner must be
     a node, 1 to nV. A quad splits as upstream splits it.
5. **Hole header:** `nH` of 1 field (int, 0 or more), then nH lines of `i x y z`. They are
   validated, then dropped.
6. **Region header (optional):** end of file here is fine. Otherwise `nR` of 1 field, then nR
   lines of `i x y z attr vol`, exactly 6 fields, with `attr` an int.
7. **User facet header (optional):** end of file here is fine. Otherwise `nU [flag]`, 1 or 2
   fields, flag 0 or 1. Then nU facets as in step 4, into `user_defined_faces`, **each keeping its
   own marker** (see the upstream bug above).
8. Anything after that other than blank or comment lines is `Invalid`.

**What TetGen does differently with an accepted file.** Each of these was seen in the TetGen
differential under Verification, and nothing else was:

- **Facet markers above 2147483647** clamp to 2147483647 in TetGen's `strtol`. `ImportPOLY`
  keeps the unsigned value. `generate` writes such markers on purpose, and upstream's type
  allows them.
- **Region point and constraint** are `float` to `ImportPOLY` and `double` to TetGen. For what
  `ExportPOLY` writes (a float printed exactly) the two are the same number. For any other
  decimal TetGen keeps the nearer double. `test_import1.poly`'s region is one such case.
- **A node count of 0** sends TetGen to a `.node` file. **Fewer than 4 nodes** makes TetGen
  refuse the file: 22 of the first 200 generated models have 3 nodes. **A facet count of 0**
  ends TetGen's reading before the holes and regions.
- By design of the two readers: TetGen keeps a quad as one 4-corner polygon, keeps the holes,
  and never reads Part 5.
- A count above 2147483647 would clamp in TetGen. This cannot happen in an accepted file, which
  must hold that many records.

**Failure kinds.**

| Kind | When |
|---|---|
| `NotFound` | missing file (`read_file`) |
| `Truncated` | the data ends inside a required part: before or inside Parts 1 to 3, or inside a region or user facet list whose count promised more |
| `Invalid` | everything else above, with the line number |

`Version` is never produced, because the format has none.

**Sizes and the allocation budget.** Reading takes two passes over the bytes. The first
validates everything and counts records, allocating nothing. The second fills vectors reserved
to exactly those counts. No header count is ever trusted, even for reservation.

The model then costs:
- 24 bytes per node;
- 16 bytes per triangle;
- 20 bytes per region.

The smallest encodings those come from:
- A node line is `k 0 0 0\n`, digits(k) + 7 bytes, and numbering must count up.
- A quad facet is `1 0 0\n4 1 1 1 1\n`, 16 bytes for two triangles: exactly 2×.
- A triangle facet is 14 bytes for one triangle.
- A region line is at least 12 bytes.

Only node lines exceed 2×, by 10 - 2·digits(k) bytes each, summed:

    9·8 + 90·6 + 900·4 + 9000·2 = 22,212 bytes < 64 KiB

That holds for any node count, so every read stays within `budget(len) = 2·len + 64 KiB`.
Error messages quote at most 32 bytes of a bad field. `tests/poly_fuzz.rs` checks all of these
worst cases directly.

## Canonical dump

This is what `ImportPOLY` exposes, in `t_model`'s field order. Faces are `t_face`'s order: the
three 0-based indices, then the marker. Regions are `t_region`'s order: attribute, point,
constraint. There are no version tokens, since the file has none.

```
poly
save_face_index <0|1>
user_faces <n>
<a> <b> <c> <faceIndex>          n lines, decimal
faces <n>
<a> <b> <c> <faceIndex>
vertices <n>
<x> <y> <z>                      f64_hex each
regions <n>
<regionIndex> <x> <y> <z> <refinement>     decimal int, then f32_hex ×4
```

On failure the output is `error <kind>` (Rust) or `error` (oracle). The oracle fails only when
the file cannot be opened.

## Verification (2026-09-23)

| Check | Result |
|---|---|
| `poly_golden`: fixtures, upstream `io_test.cpp` expectations, writer bytes vs the GUI-written fixture, MSVC float spellings, the refused spellings (Ctrl-Z, separators, leading zeros, long lines, `#` after data), comments, and negative cases | 26 tests pass |
| `poly_golden`, oracle tests: fixtures, 200 generated files, 11 hand-made variants at the edges of what is accepted, and the Ctrl-Z receipt. They build the oracle with `oracle/build.ps1 -Only poly` when it is missing or older than its sources, and fail when it cannot be built | 4 of the 26; they never pass without running |
| `poly_fuzz`: 10,000 arbitrary-byte cases, 10,000 fixture and generated mutations, 5,000 prefix-plus-token-soup cases, and the worst cases above | 8 tests pass, 0 panics, peak ≤ `budget(len)` |
| Oracle vs `dump_file`, every `.poly` fixture | 2 of 2 agree |
| Oracle reading `write_file(generate(seed))` vs `dump(&upstream_import_view(&model))` | 0 of 200 differ (seeds 0 to 199), and 0 of 1000 (seeds 1 to 1000, the gate's range). Against `dump(&model)`, 93 of 200 and 497 of 1000 differ, every one from the upstream bug alone |
| `write` vs upstream `ExportPOLY` on the same model (throwaway driver) | byte-identical for the 200 generated models and for 3,000 stress models (144,000 adversarial doubles and floats: ties, subnormals, NaN payloads, notation boundaries) |
| Mutation differential (throwaway harness): 30,000 mutants of both fixtures and 10 generated files, through `read`, the oracle, and TetGen's own `load_poly` | 7,127 accepted. `ImportPOLY` reads 0 of them differently. TetGen: 4,582 identical, 1,859 differ only by marker clamping, 686 only by keeping a region value as a double, 0 otherwise. Fixtures and 200 generated files through TetGen: 0 otherwise |

The fixtures:
- `upstream/lib_interface/test_import1.poly`: 19 nodes, 39 facets, 0 holes, 1 region, 0 user
  facets, flag 1. It comes from an older writer that used double spaces, so it round-trips as a
  model, not as bytes.
- `upstream/tutorial1/tetgen/scene_mesh.poly`: 8 nodes, 12 facets, 0 holes, 0 regions, 0 user
  facets, flag 1. `write(read(f))` reproduces it byte for byte.
