# `mesh.json`: the mesh manifest, and the mesh folder

What one meshing attempt did and produced. `core::mesh` writes it into the mesh folder after every
attempt, successful or not, and `run` reads it before it uses a mesh (`docs/m5-m6-design.md`,
decision 7). It is our own format: no upstream program reads or writes it.

- Rust: `crates/simpa-core/src/mesh/manifest.rs` (`MeshManifest`, `write_manifest`,
  `read_manifest`); written by `mesh_project`, `mesh_poly` and `mesh_from_tetgen` in
  `crates/simpa-core/src/mesh.rs`.
- Tests: `crates/simpa-core/tests/mesh_project.rs`, `mesh_poly.rs` and `mesh_hall.rs`.

## The mesh folder

| File | Written | Notes |
|---|---|---|
| `scene_mesh.poly` | always, before TetGen | the scene as a TetGen PLC (`docs/formats/poly.md`); see "What TetGen is given" |
| `scene_mesh.var` | when `surface_receiver_max_area_m2` is set | `docs/formats/var.md` |
| `mesh.cbin` | always, before TetGen | `config_xml::scene_mesh`: the scene the `.mbin` markers index |
| `scene_mesh.1.{node,ele,face,edge,neigh}` | by TetGen | `docs/formats/tetgen.md` |
| `scene_mesh_skipped.{node,face}` | by TetGen, on self-intersecting facets | then no `.1.neigh` |
| `tetgen.stdout.txt`, `tetgen.stderr.txt` | always, line by line as TetGen writes | |
| `diag/` | after skipped facets | the `tetgen -d` follow-up: its own `scene_mesh.poly`, logs and TetGen files |
| `tetramesh.mbin` | **only when meshing succeeded** | `docs/formats/mbin.md` |
| `mesh.json` | always, last | this page. The one exception: the folder cannot be created, or `mesh.json` cannot be written into it, and the call returns an error instead |

TetGen runs with the mesh folder as its working folder and the relative argument
`scene_mesh.poly`, because it opens files with the narrow `fopen`.

**Stale files.** Before `mesh_project` or `mesh_poly` writes anything, it deletes from the folder
every file in the table, the `diag/` folder, and `scene_mesh.edge` and `scene_mesh.mtr`: TetGen
loads `<name>.edge`, `<name>.var` and `<name>.mtr` from beside the `.poly` whenever they exist
(`tetgen.cxx:2446-2449`), so a leftover one would be applied silently. File names are compared
without case. Other files are left alone. A file that will not be deleted (read-only, or held
open) is `stale_delete_failed`, and nothing is meshed over it. `mesh_from_tetgen` deletes only
`mesh.cbin`, `tetramesh.mbin` and `mesh.json` from its output folder, which may be the folder
holding the TetGen files it reads.

## What TetGen is given

For a project (`mesh_project`), per `docs/m5-m6-design.md` decisions 1-5:
- **Flags** from the project's `MeshSettings`, in upstream's order: `[-a<v>] -pq<r> -A -n [-Y]`,
  then `scene_mesh.poly` (`projet_maillage.cpp:167-170, 181`). Numbers are spelled as upstream
  spells them: narrowed to the `float` upstream holds (`projet.h:98-99`), then
  `Convertor::ToString` (`%.15g`, classic locale). The default settings give `-pq5 -A -n -Y`,
  both room fixtures give `-pq2 -A -n`, and q = 1.1 gives `-pq1.10000002384186`, as upstream's
  command line has it. A `-q` or `-a` value must be above 0 and finite as `f32`.
- **Vertices** narrowed to `f32`, written back as `f64`: the `.poly` holds exactly the `.cbin`'s
  values.
- **Facets** in project order, facet marker = face index = `.cbin` face index.
- **Box fitting zones:** 8 corners and 12 triangles each, in the facet list (not the Part 5 user
  list), with markers `scene faces + k`. The `.mbin` writes those as -1.
- **Regions:** one per enabled fitting zone, at the box centre or the zone's `inside_point`
  narrowed to `f32`, with attribute = the zone's solver id and no volume bound. The room has none.

For a raw `.poly` (`mesh_poly`): its facets are the scene. Vertices are narrowed to `f32`, each
facet's marker becomes its position (a note records any that changed), its regions are kept,
their attributes become the fitting ids, and its Part 5 user facets are dropped, since TetGen never
reads them (a note records how many). The flags come from the `MeshSettings` the caller passes.
There is no `.var`.

## Layout

UTF-8 JSON, pretty-printed with two-space indents, `\n` line ends and a final newline. Read it
with `read_manifest`, which uses the crate's correctly rounded JSON reader
(`schema::parse_json`): serde_json's own reader does not give back the `f64` values written.

| Key | Type | Meaning |
|---|---|---|
| `manifest_version` | integer | 1. `read_manifest` refuses any other value |
| `simpa_core` | string | the `simpa-core` version that wrote it |
| `source` | `"project"`, `"poly"`, `"external"` | which entry point |
| `input_path` | string or null | the `.poly`, or the TetGen folder, for `poly` and `external` |
| `status` | `"OK"`, `"FAIL"`, `"CANCELLED"` | `CANCELLED` when `codes` holds `cancelled`; otherwise `OK` exactly when `codes` is empty |
| `codes` | array of strings | reason codes, below, in the order they were found |
| `messages` | array of strings | the same failures in words, then remarks on the input |
| `mesh_input_hash` | string or null | `validate::mesh_input_hash` of the project meshed (32 hex digits); null for a raw `.poly` |
| `tetgen` | object or null | the TetGen call: `program`, `program_sha256` (hashed after the run, so hashing does not delay the launch), `argv` (after the program), `cwd` (`.`), `exit_code` (raw `u32`, null when cancelled), `cancelled`, `elapsed_ms`. For `external`, `program` and `argv` come from the `.1.face` trailer, `cwd` is the TetGen folder, and there is no hash, exit code or time |
| `files` | object | sha256 (64 lowercase hex digits) of `poly`, `var`, `cbin` and `mbin`; null for a file not written |
| `counts` | object | `scene_vertices`, `scene_faces`, `poly_vertices`, `poly_facets`, `regions`, `var_constraints`, and `build` (below) once TetGen's output was read |
| `volume_ids` | object | `room` (0) and `fittings` (the seeded solver ids): the `idVolume` values the `.mbin` may carry |
| `zone_facets` | array | per box fitting zone: `zone` (its name), `solver_id`, `first_marker` of its 12 triangles |
| `skipped_rows` | integer | rows of `scene_mesh_skipped.face` |
| `skipped_facets` | array | per distinct skipped marker, ascending: `marker`, `scene_face` (when the marker is one), `group` (its surface group), `fitting_zone` (when it is a box zone's triangle) |
| `diagnosis` | object or null | the `tetgen -d` follow-up, below. Null when no facet was skipped, when the run was cancelled, or when `diag/` could not be set up (a message then says why) |
| `verify` | object or null | `mesh::verify::verify_mesh`'s report on the `.mbin` built, whether it passed or not |
| `elapsed_ms` | number | wall time of the whole call |

`counts.build` holds `nodes`, `tetrahedra`, `face_rows` (of the `.1.face`), `marked_tet_faces`
(tetrahedron faces written with a scene marker, both sides of an internal facet counted),
`zone_tet_faces` (faces on box-zone triangles, written -1), `hull_tet_faces` (neighbour -2), and
`attributes`: each TetGen region attribute seen, the `idVolume` written for it and its number of
tetrahedra.

`diagnosis` holds `call` (as `tetgen`, with `argv` `["-d", "scene_mesh.poly"]` and `cwd` `diag`),
`skipped_markers` (from `diag/scene_mesh_skipped.face`) and `intersections`. Each intersection has
TetGen's `message` and a `first` and `second` element, each with `kind` (`facet`, `segment`,
`vertex`, or `unknown` when TetGen printed no detail), the `points` TetGen printed (`.poly` node
numbers), the printed `tag`, and the facet `markers` it maps to. A segment's tag is -1, so a
segment maps to every facet with that edge. Upstream's debug parser looks for TetGen 1.4's
messages (`logger_tetgen_debug.hpp:47-71`), which TetGen 1.6.0 no longer prints; both forms are
read (`crates/simpa-core/src/mesh/diag.rs`). Measured on the survey's self-intersecting cube: `-d`
writes its own `_skipped.face` and exits 3, and the pairs are ({12}, {8}), ({12}, {9}) and
({8, 9}, {12}).

## Reason codes

Every code that applies is listed, not only the first.

| Code | When |
|---|---|
| `mesh_settings_conflict` | the settings ask for a `.var` and `-Y` together; nothing is written but the manifest. The validator's rule of the same name |
| `input_invalid` | the project or `.poly` cannot be expressed as TetGen input: a structural fault, a vertex or region point not finite as `f32`, an empty box zone, a `-q` or `-a` value not above 0 and finite as `f32`, a `.poly` that does not parse, a scene `mesh.cbin` cannot hold. For `mesh_from_tetgen`: a folder holding more than one TetGen output set, with no basename given |
| `stale_delete_failed` | a file an earlier mesh left could not be deleted; nothing is meshed |
| `input_write_failed` | `mesh.cbin`, `scene_mesh.poly` or `scene_mesh.var` could not be written; TetGen does not run |
| `tetgen_launch_failed` | TetGen could not be started (no such file), or its log could not be written |
| `cancelled` | the cancel token was set: before TetGen started, while it ran (it is killed), during the `-d` follow-up, or before the `.mbin` was written |
| `tetgen_crash` | the exit code is 0xC0000000 or above (an NTSTATUS error such as 0xC0000005) |
| `tetgen_exit_nonzero` | the exit code is not 0; a crash has both codes. TetGen exits 3 after skipping facets |
| `tetgen_skipped_facets` | `scene_mesh_skipped.face` has rows. Its markers are facet markers (`docs/formats/tetgen.md`) and are mapped to scene faces and groups; `diag/` then holds the `-d` follow-up |
| `tetgen_output_missing` | `.1.node`, `.1.ele` or `.1.face` is missing |
| `neigh_missing` | `.1.neigh` is missing. Neighbours are never computed any other way. `mesh_from_tetgen` on a folder with no TetGen output reads it as `scene_mesh`, so it reports this and `tetgen_output_missing` |
| `tetgen_output_invalid` | a TetGen file does not read (`scene_mesh_skipped.face` included), the files disagree (`formats::tetgen::check_mesh`), `.ele` has no `-A` attribute or a non-integral one, `.face` has no marker column, one triangle has two markers, or a `.face` triangle belongs to no tetrahedron |
| `mesh_invalid` | the `.mbin` built fails `verify_mesh`; the verifier's own codes follow it |
| `mbin_write_failed` | the `.mbin` could not be written |

Only an `OK` manifest comes with a `tetramesh.mbin`, and its `files.mbin` is that file's sha256.
`run` checks the `.mbin` against it. The `.cbin`'s sha256 is recorded only, because `run`
re-exports the `.cbin` with the current materials (decision 7).

## Measured (2026-09-23, Grace, debug build of the tests)

| Mesh | Flags | Result |
|---|---|---|
| tutorial 1's box | `-pq2 -A -n` + `.var` | 8 nodes, 6 tetrahedra; TetGen 9-19 ms (the `.var` is inert, `docs/formats/var.md`) |
| the box with a box fitting zone (1, 1, 0.5)-(2, 2, 1.5) | `-pq2 -A -n` + `.var` | zone volume 1.000 m³ (idVolume 2), room 179.000 m³ (TetGen's attribute 3, written 0) |
| the box with a `Surfaces` zone: an inner box of 12 scene faces | `-pq2 -A -n` + `.var` | zone 1.000 m³ (idVolume 2), room 179.000 m³; each inner-box triangle marked on both of its tetrahedron faces |
| the corrected hall | `-pq2 -A -n` | 29,472 nodes, 123,718 tetrahedra, 36,716 `.face` rows covering all 7,860 scene faces; TetGen 1,074-1,310 ms, `mesh_project` 1,992-2,230 ms |
| the hall, cancelled 50 ms after TetGen's launch | `-pq2 -A -n` | TetGen launched 36-45 ms into the call (debug build: the inputs are built and hashed first), killed after 58-69 ms of running, `CANCELLED` 108-119 ms after the call started; no `.1.ele`, no `.mbin` |
| the survey's self-intersecting cube | `-pq5 -A -n -Y` | exit 3, skipped markers 8, 9, 12, no `.1.neigh`, no `.mbin` |
| the box plus a baffle piercing wall face 9 | `-pq2 -A -n` | exit 3; TetGen skips the pierced face 9 (group `Walls`), not the baffle; `-d` names the baffle's edge against face 9 |
| the box plus two overlapping box zones | `-pq2 -A -n` | exit 3; skipped markers 14, 15, 18-21 (zone 1) and 28, 29 (zone 2), each named by its zone |
