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
| `scene_mesh.poly` | always, before TetGen | the scene as a TetGen PLC (`docs/formats/poly.md`); see "What TetGen is given". With upstream's scene correction, what `preprocess.exe` saved, markers restored outside parity mode |
| `scene_mesh.input.poly` | with upstream's scene correction | the `.poly` as the mesher wrote it, before `preprocess.exe` rewrote it |
| `preprocess.stdout.txt`, `preprocess.stderr.txt` | with upstream's scene correction, line by line | |
| `scene_mesh.var` | when `surface_receiver_max_area_m2` is set | `docs/formats/var.md` |
| `mesh.cbin` | always, before TetGen | `config_xml::scene_mesh`: the scene the `.mbin` markers index |
| `scene_mesh.1.{node,ele,face,edge,neigh}` | by TetGen | `docs/formats/tetgen.md` |
| `scene_mesh_skipped.{node,face}` | by TetGen 1.6.0, on self-intersecting facets | then no `.1.neigh`. TetGen 1.5.0, the mesher since decision 3, never writes them: it stops instead |
| `tetgen.stdout.txt`, `tetgen.stderr.txt` | always, line by line as TetGen writes | |
| `diag/` | after skipped facets or a self-intersection stop | the `tetgen -d` follow-up: its own `scene_mesh.poly`, logs and TetGen files (1.5.0 writes `scene_mesh.1.node` and a `scene_mesh.1.face` of the intersecting triangles) |
| `tetramesh.mbin` | **only when meshing succeeded**, and in parity mode whether it verifies or not | `docs/formats/mbin.md`. A parity mesh's `mesh.json` is `FAIL` when it does not verify: its `.mbin` is for byte comparison, and `run --mesh` refuses the folder whatever its `mesh.json` says (`mesh_parity`; with the manifest edited, `mesh.json` is not signed, the `.mbin` is held to the geometry again before launch, `docs/solver-contract.md`, "The run manager") |
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
- **Vertices** from the scene mesh (`config_xml::scene_mesh`): narrowed to `f32`, then taken
  through upstream's OpenGL round trip in the scene's frame (`config_xml::GlFrame`), written back
  as `f64`. The `.poly` holds exactly the `.cbin`'s values, as upstream's does
  (`Objet3D_maillage.cpp:777, 941-942`); tutorial 1's box gives upstream's 2019 `.poly` byte for
  byte (`tests/parity_inputs.rs`).
- **Facets** in project order, facet marker = face index = `.cbin` face index.
- **Box fitting zones:** 8 corners and 12 triangles each, in the facet list (not the Part 5 user
  list), with markers `scene faces + k`. The `.mbin` writes those as -1. Each corner coordinate
  takes the same round trip as the scene's vertices, as upstream's drawn boxes do; the round trip
  works coordinate by coordinate, so a box face flush with a wall stays in that wall's plane.
- **Regions:** one per enabled fitting zone, at the box centre (from its corners as written) or
  the zone's `inside_point`, narrowed to `f32`, with attribute = the zone's solver id and no volume bound. The room has none.

With upstream's scene correction (`MeshSettings::preprocess`; `docs/solver-contract.md` Part B,
"Preprocessing and the meshed volume"), the `.poly` is upstream's GUI's for `preprocess.exe`
(`CObjet3D::_SavePOLY(path, true, true, true, ...)`, `Objet3D_maillage.cpp:931-1044`;
`mesh::preprocess_layout`):
- **Nodes and facets:** the scene is `config_xml::scene_mesh`, the room's faces and then each
  enabled box zone's 12 triangles on three nodes of their own, and every one of its vertices is a
  node. The room's faces are the facet list, marker = face index; the box triangles are the user
  facet list (Part 5), marker = their `.cbin` face index. So every marker, a box triangle's
  included, is a `.cbin` face index, and `mesh.cbin` is that scene.
- **Regions**, in ascending solver id (upstream's order is its drawable table's, a wx hash map its
  source does not fix; ascending is tutorial 3's): a box seeded at `hc - (hc - ba) * 1e-4f`, per
  component in `f32`, from its corners as upstream holds them (`FittingShape::box_corners`;
  `e_scene_encombrements_encombrement_cuboide.h:333-335`); a scene-fitted zone at its
  `inside_point` (upstream's `volpos`), as it is. The attribute is the zone's solver id, which
  for a project imported from a `.proj` is upstream's element id, pinned on import
  (`docs/m5-m6-design.md`, decision 13): tutorial 3's region lines are upstream's, 1930 and 2083,
  byte for byte.
- **Order, a known difference.** Upstream writes each drawable's triangles and then its region,
  drawable by drawable in its table's order (`Objet3D_maillage.cpp:970-1040`); this mesher writes
  the user facets in project order and the regions in ascending id. With one box zone the two
  agree (tutorial 3, byte for byte). With two or more, `preprocess.exe` may meet the user facets
  in another order than upstream's, which can change its splits and so the mesh; no upstream
  project has two.
- **Then `preprocess.exe`** (`mesh::preprocess`): the file it saved is read and accounted for
  facet by facet against the one it was given (`preprocess::account`), its user-facet markers
  are restored (each facet takes the marker of the facet it lies in) unless in parity mode, and
  `geometry::check` must pass it before TetGen runs.
- **A seed on a facet** (within `16 · 2⁻²⁴ · R`) is moved, outside parity mode, along the
  normal of the facet it lies on into the zone's cell, halfway to the next facet
  (`mesh::verify::seed_inside`), and recorded in `seeds_moved`. This is this crate's rule, for
  Burhan to confirm (`docs/m5-m6-design.md`, decision 12); upstream writes `volpos` as it is.
  Without upstream's scene correction no seed is moved.

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
| `tetgen` | object or null | the TetGen call: `program`, `program_sha256` (hashed after the run, so hashing does not delay the launch), `argv` (after the program), `cwd` (`.`), `exit_code` (raw `u32`, null when cancelled or timed out), `cancelled`, `elapsed_ms`, `timeout_ms` (the limit it ran under, `mesh::Timeouts`; null when not run here) and `timed_out` (stopped at that limit; decision 14). For `external`, `program` and `argv` come from the `.1.face` trailer, `cwd` is the TetGen folder, and there is no hash, exit code, time or limit. `timeout_ms` and `timed_out` are read as null and false when absent |
| `files` | object | sha256 (64 lowercase hex digits) of `poly`, `var`, `cbin` and `mbin`; null for a file not written |
| `counts` | object | `scene_vertices`, `scene_faces`, `poly_vertices`, `poly_facets`, `regions`, `var_constraints`, and `build` (below) once TetGen's output was read |
| `volume_ids` | object | `fittings` (the seeded solver ids) and `room`, the room's first id as TetGen numbers it, one above the largest fitting or 1 (`VolumeIds::tetgen`; its further parts carry the ids above): the `idVolume` values the `.mbin` may carry (`docs/m5-m6-design.md`, decision 1) |
| `zone_facets` | array | per box fitting zone: `zone` (its name), `solver_id`, `first_marker` of its 12 triangles |
| `skipped_rows` | integer | rows of `scene_mesh_skipped.face` |
| `skipped_facets` | array | per distinct skipped marker, ascending: `marker`, `scene_face` (when the marker is one), `group` (its surface group), `fitting_zone` (when it is a box zone's triangle) |
| `self_intersection` | object or null | TetGen 1.5.0's stop on a self-intersection, below; null when TetGen did not stop on one. Read as null when absent |
| `diagnosis` | object or null | the `tetgen -d` follow-up, below. Null when no facet was skipped and TetGen did not stop on a self-intersection, when the run was cancelled, or when `diag/` could not be set up (a message then says why) |
| `verify` | object or null | `mesh::verify::verify_mesh_with`'s report on the `.mbin` built, whether it passed or not |
| `preprocess` | object or null | `preprocess.exe`'s run, when the settings asked for it: `call` (as `tetgen`), `markers` (`restored` or `parity`), `printed` (`status`, `aborted`, `not_found`, `vertices_merged`, `faces_destroyed`, `faces_split`, `split_lines`, as it printed them), `input_sha256` and `output_sha256` (also recorded when it said it saved nothing and the file changed all the same, `preprocess_output_invalid`), `input` and `output` (`vertices`, `facets`, `user_facets`, `regions`), `accounting` (below), `deleted_facets` (mapped to the scene as `skipped_facets`), `tolerance_m`, `markers_rewritten`, `summary`, the line also printed among the messages, `outcome` (`corrected`: TetGen read what it saved; `aborted`: it saved nothing, and the `.poly` as written went on to the geometry check and TetGen, as upstream's GUI meshes it; null when the run failed) and `aborted_reason` (why, with its last line). Read as null when absent |
| `geometry` | object or null | `geometry::check` on the `.poly` TetGen read: `checked` (`preprocessed`, `written`, `written, preprocess.exe having given up` (the `.poly` as the mesher wrote it, meshed because `preprocess.exe` saved nothing; `preprocess.outcome` is `aborted`), `external`, or `project`: for `external` with no `<base>.poly` beside TetGen's output, the project's own `.poly` as the mesher writes it without upstream's scene correction, which stands in for it; the region check is never skipped), `vertices`, `facets`, `verdict` (`ok` or `refused`), `reasons` (`code`, `count`, `facets`: positions in the facet list, the first 20, `markers`, `message`), `pairs` (each self-intersecting pair as markers), `cells` (`id`, `depth`, `volume_m3`) and `enclosed_volume_m3`. With upstream's scene correction a refusal is the gate before TetGen; without it, TetGen judges first, and a mesh it makes of a refused `.poly` is `geometry_refused`. Read as null when absent |
| `parity` | boolean | parity mode: `preprocess.exe`'s markers kept, the `.mbin` written whether it verifies or not. Read as false when absent. `run --mesh` refuses a folder whose manifest says `true`, `mesh_parity`, even when its status is `OK` (the box without a fitting zone meshes `OK` in parity mode) |
| `seeds_moved` | array | per fitting zone whose seed lay on a facet and was moved into its cell: `zone`, `solver_id`, `from`, `to`, `on_facets`, `cell` |
| `elapsed_ms` | number | wall time of the whole call |

`counts.build` holds `nodes`, `tetrahedra`, `face_rows` (of the `.1.face`), `marked_tet_faces`
(tetrahedron faces written with a scene marker, both sides of an internal facet counted),
`zone_tet_faces` (faces on box-zone triangles, written -1), `hull_tet_faces` (neighbour -2), and
`attributes`: each TetGen region attribute seen, the `idVolume` written for it (the attribute
itself, decision 1) and its number of tetrahedra.

`self_intersection` holds `stop`, the pair TetGen named before it stopped (an intersection as
below, with `message` its `Found ...` line), or null when it named none; `pairs`, every pair of
facet markers found intersecting, `[a, b]` with `a < b`, ascending, from the stop and the `-d`
follow-up; and `facets`, every facet named by the stop, the `-d` pairs or the rows of
`diag/scene_mesh.1.face`, once each, ascending, mapped as `skipped_facets` are (`marker`,
`scene_face`, `group`, `fitting_zone`). How the stop's lines are read, and why a facet number maps
through its position in the `.poly` while a segment maps by its points, is in
`crates/simpa-core/src/mesh/diag.rs`.

`diagnosis` holds `call` (as `tetgen`, with `argv` `["-d", "scene_mesh.poly"]` and `cwd` `diag`),
`skipped_markers` (from `diag/scene_mesh_skipped.face`, TetGen 1.6.0), `face_markers` (one per row
of `diag/scene_mesh.1.face`, the intersecting triangles TetGen 1.5.0's `-d` writes; read as empty
when absent) and `intersections`, each once however often TetGen printed it. Each intersection has
TetGen's `message` and a `first` and `second` element, each with `kind` (`facet`, `segment`,
`vertex`, or `unknown` when TetGen printed no detail), the `points` TetGen printed (`.poly` node
numbers), the printed `tag`, and the facet `markers` it maps to. A segment's tag is -1, so a
segment maps to every facet with that edge. Upstream's debug parser looks for TetGen 1.4's
messages (`logger_tetgen_debug.hpp:47-71`), which TetGen 1.6.0 no longer prints; both forms are
read (`crates/simpa-core/src/mesh/diag.rs`). Measured on the survey's self-intersecting cube: with
TetGen 1.6.0, `-d` writes its own `_skipped.face` and exits 3, and the pairs are ({12}, {8}),
({12}, {9}) and ({8, 9}, {12}); with TetGen 1.5.0, `-d` exits 0, prints the pairs (#9, #13) and
(#10, #13) four times each, and writes a `.1.face` with markers 8, 9 and 12.

## Reason codes

Every code that applies is listed in `codes`, not only the first. Two of them are documented
where they were defined first, and mean the same here:
- **`mesh_settings_conflict`** (`docs/solver-contract.md` Part A): the settings ask for a `.var`
  and `-Y` together. The mesher refuses it itself, and nothing is written but the manifest.
- **`cancelled`** (`docs/solver-contract.md` Part B): the cancel token was set, before TetGen
  started, while it ran (it is killed), during the `-d` follow-up, or before the `.mbin` was
  written. The status is then `CANCELLED`.

Upstream's scene correction and the time limits bring eight more that `mesh.json` can carry,
documented in `docs/solver-contract.md` Part B: `geometry_refused` (the run contract's code, in
the reason codes table) and `preprocess_launch_failed`, `preprocess_crash`,
`preprocess_exit_nonzero`, `preprocess_timeout`, `preprocess_aborted` (a recorded outcome, not a
refusal), `preprocess_output_invalid` and `tetgen_timeout` ("Preprocessing and the meshed
volume"). `regions_unchecked`, which `mesh-verify` and `run-folder` give, is in that section's
region table.

The mesher's own codes:

| Code | When |
|---|---|
| `input_invalid` | the project or `.poly` cannot be expressed as TetGen input: a structural fault, a vertex or region point not finite as `f32`, an empty box zone, a `-q` or `-a` value not above 0 and finite as `f32`, a `.poly` that does not parse, a scene `mesh.cbin` cannot hold. For `mesh_from_tetgen`: a folder holding more than one TetGen output set, with no basename given |
| `stale_delete_failed` | a file an earlier mesh left could not be deleted; nothing is meshed |
| `input_write_failed` | `mesh.cbin`, `scene_mesh.poly` or `scene_mesh.var` could not be written; TetGen does not run |
| `tetgen_launch_failed` | TetGen could not be started (no such file), or its log could not be written |
| `tetgen_crash` | the exit code is 0xC0000000 or above (an NTSTATUS error such as 0xC0000005) |
| `tetgen_exit_nonzero` | the exit code is not 0; a crash has both codes. TetGen exits 3 after skipping facets (1.6.0) and when it stops on a self-intersection (1.5.0) |
| `tetgen_self_intersection` | TetGen stopped on a self-intersection of its input: exit code 3 with TetGen 1.5.0's line `A self-intersection was detected. Program stopped.` (`tetgen.h:2265-2267`), which it prints on every exit 3. The pair it names, when it names one, and the `-d` follow-up's pairs and `.1.face` rows are mapped to scene faces and groups in `self_intersection`; `diag/` holds the follow-up. TetGen 1.6.0's own stop line does not give this code: its `_skipped.face` does (`tetgen_skipped_facets`) |
| `tetgen_skipped_facets` | `scene_mesh_skipped.face` has rows (TetGen 1.6.0; a committed 1.6.0 set such as `tests/fixtures/meshes/broken_hall` is still read). Its markers are facet markers (`docs/formats/tetgen.md`) and are mapped to scene faces and groups; `diag/` then holds the `-d` follow-up |
| `tetgen_output_missing` | `.1.node`, `.1.ele` or `.1.face` is missing |
| `neigh_missing` | `.1.neigh` is missing. Neighbours are never computed any other way. `mesh_from_tetgen` on a folder with no TetGen output reads it as `scene_mesh`, so it reports this and `tetgen_output_missing` |
| `tetgen_output_invalid` | a TetGen file does not read (`scene_mesh_skipped.face` included), the files disagree (`formats::tetgen::check_mesh`), `.ele` has no `-A` attribute or a non-integral one, `.face` has no marker column, one triangle has two markers, or a `.face` triangle belongs to no tetrahedron |
| `mesh_invalid` | the `.mbin` built fails `verify_mesh`; the verifier's own codes follow it |
| `mbin_write_failed` | the `.mbin` could not be written |

Only an `OK` manifest comes with a `tetramesh.mbin`, and its `files.mbin` is that file's sha256.
`run` checks the `.mbin` against it. The `.cbin`'s sha256 is recorded only, because `run`
re-exports the `.cbin` with the current materials (decision 7).

## Verification codes

`mesh::verify` checks a `.mbin` against the `.cbin` its markers index (`verify_mesh`), and a whole
folder (`verify_dir`). The mesher runs `verify_mesh` on every `.mbin` it builds and lists its
codes after `mesh_invalid`, and `run-folder` does the same before it launches a folder
(`docs/solver-contract.md` Part B, "The run manager"). Each count of the report is a number of offending
items, and its code is spelled as its field. A mesh passes exactly when every count is 0.

| Code | Counts |
|---|---|
| `index_errors` | corner, face-vertex or neighbour indices out of range, one per index; a tetrahedron with a corner out of range gets no geometric check |
| `degenerate_tets` | tetrahedra with a repeated corner, or with `\|(A−D)·((B−D)×(C−D))\|` at or below the `f32` noise floor of their corners; they get no orientation or face-order check |
| `inverted_tets` | tetrahedra with `(A−D)·((B−D)×(C−D)) > 0`, against the `.mbin` convention (`docs/formats/mbin.md`) |
| `misordered_faces` | faces that are not the face opposite their slot's corner, wound as the face table winds it (a rotation is accepted): the solver takes a face's normal from this winding (`coreTypes.cpp:233`) |
| `unmarked_boundary_faces` | faces with no neighbour and a marker below 0 |
| `marker_out_of_range` | markers at or above the `.cbin`'s face count |
| `nonmutual_neighbors` | faces whose neighbour does not name them back across the same three nodes, and faces with no neighbour whose three nodes another face holds |
| `asymmetric_internal_faces` | marked faces whose neighbour's shared face carries a different marker: internal facets are marked on both sides (decision 6) |
| `marker_geometry_mismatches` | marked faces with a node farther than 16·2⁻²⁴·R from the scene face their marker names, R being the scene's largest \|coordinate\| |
| `uncovered_scene_faces` | scene faces no tetrahedron face carries (the report lists the first 20), a drawn zone's triangles excepted (below) |
| `unknown_volume_ids` | tetrahedra whose `idVolume` is no declared fitting's and not one of the room's parts: below the room's first id (TetGen's numbering; a room written 0 beside it, for one), or past a gap, since TetGen numbers the parts up by one from the first (`tetgen.cxx:22403-22436`: a room of parts 4, 5 and 7 has 7 unknown) |

**Region volumes.** Four more counts are documented in `docs/solver-contract.md` Part B
("Preprocessing and the meshed volume"): `region_volume_mismatch`, `unmeshed_cells`,
`fitting_region_misplaced` and `fitting_seed_ambiguous`. They are checked only against the
geometry TetGen was given and its cells (`verify_mesh_with` with a `Reference`), which the mesher
has and `mesh-verify` and `run-folder` do not. Each region's representative point, the centroid
of its largest tetrahedron, is located exactly among the geometry's facets, as the geometry check
places its components, and the regions and cells are matched one to one; the report lists them
in `regions` (per `idVolume`: `tetrahedra`, `volume_m3`, `cell`, `cell_volume_m3`,
`tolerance_m3`, `fitting`, `zone_cell`, `seed_on_facets`, `problems`) and `cells` (`cell`,
`depth`, `volume_m3`, `boundary_area_m2`, `regions`), with `regions_checked` true. The tolerance,
`2 · A · 16 · 2⁻²⁴ · R` for a cell whose boundary has area `A`, is twice the volume of a shell
that thick over its boundary: every node of a marked face lies within `16 · 2⁻²⁴ · R` of its scene
face. Measured on tutorial 3 (default mode): the hall 755.648132 m³ against its cell's
755.648136 m³, 4.2e-6 m³ apart for a tolerance of 1.8e-2 m³; the other four regions within
7.1e-8 m³. A zone's cell is the one its seed lies in; for a seed on facets, of the cells on
either side, the one closed by the zone's own faces (by marker) and the outer shell alone.

**The mesher knows which faces go unmeshed.** With upstream's scene correction it gives the
verifier the facets `preprocess.exe` deleted (tutorial 3: the box's bottom, faces 88 and 89, which
lie on the floor) in place of the recognition below, so every other box triangle must be
covered; the report lists them in `expected_unmeshed_faces`.

**A drawn zone's triangles need no marker.** Upstream's GUI appends each enabled rectangular
fitting zone's 12 triangles to the `.cbin` after the room's faces (three vertices of their own
each, `idMat` 0, `idRs` -1, `idEn` the zone's id, `Objet3D_maillage.cpp:783-816`), and so does
the `.cbin` of every run folder this crate writes (`config_xml::scene_mesh`); this crate's mesher
marks none of them (`docs/m5-m6-design.md`, decision 5). `verify_mesh` leaves them out of
`uncovered_scene_faces` and counts them in the report's `drawn_zone_faces`. A face is one only in
a run of 12 at the end of the `.cbin` (counted back from its last face) whose faces all carry
`idMat` 0, `idRs` -1 and one `idEn` that is a declared fitting's (`--fittings`, or the config's
`encombrement` ids in `run-folder`), use 36 vertices no other face uses, and make one axis-aligned
box with a volume, each side two triangles sharing its diagonal (`mesh::verify::drawn_zone_faces`).
Anything else is a scene face like any other: `mesh-verify` on such a run folder without the zone
declared fails `uncovered_scene_faces`.

`verify_dir` adds what only the folder shows. The mesher's `tetgen_skipped_facets`,
`neigh_missing` and `tetgen_output_missing` (above) mean the same there, for TetGen output under
any basename. It also holds the `.mbin`'s regions to the folder's own geometry (its `.poly`,
else its `.cbin`), and a folder whose geometry gives no cells is `regions_unchecked`, a code of
`docs/solver-contract.md` Part B ("Preprocessing and the meshed volume"), beside the region
check's own. Its
own codes:

| Code | When |
|---|---|
| `cbin_missing` | a `.mbin` with no `.cbin` beside it to check its markers against |
| `manifest_mismatch` | `mesh.json` records a `.mbin` sha256 that is not the folder's `.mbin`'s, or one for a `.mbin` the folder lacks, or `files.mbin` null beside a `.mbin`. `run --mesh <dir>` refuses a folder whose `files.mbin` is not its `.mbin`'s sha256 with the same code |
| `nothing_to_verify` | the folder holds neither TetGen output nor a `.mbin` |

## Measured with TetGen 1.5.0 (2026-09-24, Grace, debug build of the tests)

| Mesh | Flags | Result |
|---|---|---|
| tutorial 1's box | `-pq2 -A -n` + `.var` | 732 nodes, 2,257 tetrahedra, 934 floor faces of at most 0.09976 m², upstream's 2019 mesh (`tests/mesh_mbin_parity.rs`) |
| tutorial 1's box without its `.var` | `-pq2 -A -n` | 60 tetrahedra, 10 floor faces, the largest 13.43 m² |
| the survey's self-intersecting cube | `-pq5 -A -n -Y` | exit 3, `tetgen_self_intersection`: the stop names the edge [9, 10] (facet 12) against facet #9 (marker 8); `-d` names 8, 9 and 12 in the pairs [8, 12] and [9, 12]; no `_skipped.face`, no `.mbin` |
| the box plus a baffle piercing wall face 9 | `-pq2 -A -n` + `.var` | exit 3, the stop names no pair (it stops in `Constrained Delaunay...`); `-d` names faces 9 (`Walls`) and 12 (`Baffle`), the pair [9, 12] |
| the box plus two overlapping box zones | `-pq2 -A -n` + `.var` | exit 3, the stop names no pair; `-d` names 18 pairs over zone 1's markers 14, 15, 18-21 and zone 2's 24, 25, 28, 29, 34, 35, each named by its zone |
| upstream's raw Elmia hall as a `.poly` (`elmia.ply`, 1,086 faces, self-intersecting; release `simpa mesh`) | `-pq5 -A -n -Y` | exit 3 after 0.25 s, `tetgen_self_intersection`: the stop names facets 553 and 581 (`Found two facets intersect each other.`); `-d` exits 0 and names 1,397 distinct pairs over 897 facets, its `.1.face` 897 rows |
| upstream's tutorial 3 through `preprocess.exe`, parity mode (`simpa mesh --parity`, debug build) | `-pq2 -A -n` | `preprocess.exe` 76 -> 57 vertices, 88 + 12 user facets -> 133; 3,285 tetrahedra, 835 nodes, upstream's `.1.*` and `.mbin` byte for byte with no id map (decision 13); FAIL by name: `mesh_invalid`, `marker_geometry_mismatches` (280), `uncovered_scene_faces` (90-99) |
| upstream's tutorial 3 through `preprocess.exe`, default mode | `-pq2 -A -n` | OK: 19 markers restored, zone 1's seed moved off its top face; 3,394 tetrahedra, 843 nodes; regions 4.352, 18.000, 93.445, 106.895 and 755.648 m³, each its cell's |
| upstream's tutorial 2 (the Elmia hall) through `preprocess.exe` (`simpa import-proj`, then `simpa mesh`, debug build, remeasured after the fallback) | `-pq2 -A -n` | `preprocess.exe` prints 104 splits and `Mesh reparation has been aborted`, saves nothing and exits 0 after 2.3 s; the `.poly` as written passes the geometry check and is meshed, as upstream's GUI meshes it: OK, `preprocess.outcome` `aborted` and a note on stderr; TetGen 0.91 s, 41,607 nodes, 161,543 tetrahedra, all `idVolume` 1; 6.7 s in all. The `.poly` and TetGen's `.1.*` files equal upstream's tutorial-2 set in value (`parity_tutorials.rs`, `tutorial_2`; 307 `.poly` and 1,036 `.node` lines differ in text at decimal ties). Until 2026-09-24 this was refused as `preprocess_aborted`, TetGen not run |
| tutorial 3 without `preprocess.exe` (the box in the facet list) | `-pq2 -A -n` | exit 3, `tetgen_self_intersection`; `geometry` refused with `self_intersections`, 22 pairs, the same set as the `-d` follow-up's |

## Measured with TetGen 1.6.0 (2026-09-23, Grace, debug build of the tests)

The mesher before decision 3 moved to TetGen 1.5.0; kept as the record of what 1.6.0 does.

| Mesh | Flags | Result |
|---|---|---|
| tutorial 1's box | `-pq2 -A -n` + `.var` | 8 nodes, 6 tetrahedra; TetGen 9-19 ms (the `.var` is inert, `docs/formats/var.md`) |
| the box with a box fitting zone (1, 1, 0.5)-(2, 2, 1.5) | `-pq2 -A -n` + `.var` | zone volume 1.000 m³ (idVolume 2), room 179.000 m³ (TetGen's attribute 3, written unchanged; 0 before decision 1 was reversed on 2026-09-24) |
| the box with a `Surfaces` zone: an inner box of 12 scene faces | `-pq2 -A -n` + `.var` | zone 1.000 m³ (idVolume 2), room 179.000 m³ (idVolume 3); each inner-box triangle marked on both of its tetrahedron faces |
| the corrected hall | `-pq2 -A -n` | 29,472 nodes, 123,718 tetrahedra, 36,716 `.face` rows covering all 7,860 scene faces; TetGen 1,074-1,310 ms, `mesh_project` 1,992-2,230 ms |
| the hall, cancelled 50 ms after TetGen's launch | `-pq2 -A -n` | TetGen launched 36-45 ms into the call (debug build: the inputs are built and hashed first), killed after 58-69 ms of running, `CANCELLED` 108-119 ms after the call started; no `.1.ele`, no `.mbin` |
| the survey's self-intersecting cube | `-pq5 -A -n -Y` | exit 3, skipped markers 8, 9, 12, no `.1.neigh`, no `.mbin` |
| the box plus a baffle piercing wall face 9 | `-pq2 -A -n` | exit 3; TetGen skips the pierced face 9 (group `Walls`), not the baffle; `-d` names the baffle's edge against face 9 |
| the box plus two overlapping box zones | `-pq2 -A -n` | exit 3; skipped markers 14, 15, 18-21 (zone 1) and 28, 29 (zone 2), each named by its zone |
