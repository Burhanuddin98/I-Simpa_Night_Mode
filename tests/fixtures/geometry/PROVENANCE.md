# Provenance: mesh import fixtures

Every file in this folder was written by `make_fixtures.py` here, which writes the same bytes
on every run. Nothing comes from upstream. Each model is a closed, outward-wound box. The files
are the inputs to M4 gate item (f) and the import fuzz, asserted by
`crates/simpa-core/tests/geometry_import_meshes.rs` and `geometry_import_fuzz.rs`.

| file | what it exercises |
|---|---|
| `box.stl` | ASCII STL, 6 x 10 x 3 m: 36 vertex records that weld to 8 |
| `box_binary.stl` | Its binary twin. The header starts with `solid`, as many exporters write it |
| `box_yup.obj` | A Y-up model (x 0..6, y 0..3 high, z 0..10), as quads. Groups by `usemtl` (floor, ceiling, walls) and, differently, by `g` (one per side). Plain, `v/vt/vn` and relative `v//vn` references |
| `box.ply`, `box_be.ply`, `box_le.ply` | Upstream's layered PLY (`layer_id` plus a `layer` element of `layer_name` lists), as quads with float32 decimal coordinates. ASCII, binary big-endian and binary little-endian twins, plus one layer that holds no face |
| `box_mixed.ply`, `box_mixed_be.ply` | Night Mode's PLY reader defects: short and int coordinates, a double z, an extra vertex property between the coordinates, ushort list counts, uint indices, a uchar `layer_id`, a uint-counted layer name, and an element to skip. ASCII and big-endian twins |

`.gitattributes` turns off line-ending conversion under `tests/fixtures/`, so the binary files
and the byte-exact text files survive checkout.
