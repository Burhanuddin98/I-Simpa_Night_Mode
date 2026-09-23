# `.var`: TetGen facet-area constraints

The constraint file TetGen loads from beside the `.poly`, `<basename>.var`. I-Simpa uses it to ask
for small triangles on the faces of surface receivers, so that a receiver map has a fine grid.

| | |
|---|---|
| Upstream writer | `formatVAR::CVar::BuildVar`, `isimpa/3dengine/Core/var.cpp:42-67`. It is called by `CObjet3D::BuildVarConstraintFile` (`isimpa/3dengine/Core/Objet3D_maillage.cpp:900-929`) when `isareaconstraint` is on and debug mode is off (`isimpa/data_manager/projet_maillage.cpp:217-224`) |
| Reader | TetGen's `tetgenio::load_var` (`tetgen/tetgen.cxx:614-673`), called for every input after `load_poly` (`:2446-2449`) |
| Rust | `crates/simpa-core/src/mesh/input.rs`: `var_bytes` writes it, `refined_faces` picks the faces |
| Oracle | none. We write the file and TetGen reads it, so there is no reader and no canonical dump |

Receipts are into upstream `929a5c8` (`B:\repos\I-Simpa-upstream`).

## Layout

The file is written with `fopen(.., "w")`, which is text mode, so every `\n` is CRLF on Windows.
`⎵` stands for one space and `⏎` for CRLF.

```
# Facet constraints⏎                       var.cpp:54
<N>⏎                                       var.cpp:55   %i
<k>⎵⎵<marker>⎵<area>⏎                      var.cpp:58   one row per constraint, k = 1..N
# Segment constraints⏎                     var.cpp:63
0                                          var.cpp:64   no line end after it
```

- `k` and `marker` are printed with `%i`.
- `area` is `Convertor::ToString(double)` of the `float` upstream holds. That is an
  `ostringstream` in the classic locale at precision 15 (`manager/sppsString.cpp:91-98`; the
  default precision is at `stringTools.h:123`), which prints like `%.15g`. So 0.1 m² is
  `0.100000001490116`: the `f32` nearest 0.1, to 15 significant digits.
- There are never any segment constraints.

**Byte target.** Upstream's own tutorial-1 file, `tests/fixtures/upstream/tutorial1/tetgen/scene_mesh.var`
(96 bytes, rows `1  0 0.100000001490116` and `2  1 0.100000001490116`). The box fixture,
`tests/fixtures/rooms/tutorial1_box.simpa`, gives it byte for byte
(`crates/simpa-core/tests/mesh_project.rs`, `the_box_meshes_with_its_own_settings`).

## What goes in

- **When.** A `.var` is written exactly when `MeshSettings::surface_receiver_max_area_m2` is set,
  even if no face qualifies (`N` is then 0). Upstream writes one whenever its constraint is on.
- **Which faces.** Every scene face whose surface group belongs to an *enabled* surface receiver
  of the `scene` kind, in project face order. Upstream takes every face whose
  `idRecepteurSurfacique` is not -1 (`Objet3D_maillage.cpp:900-929`), which is the same set: a
  face carries a receiver id only through an enabled scene receiver (`config_xml/ids.rs`,
  `group_zone_ids`). Cutting planes have no faces. The triangles of `Box` fitting zones are never
  constrained.
- **Marker.** The face's `.poly` facet marker, which is its `.cbin` face index
  (`docs/m5-m6-design.md`, decision 4).
- **Area.** One value for every row: the setting narrowed to `f32`, as upstream holds it
  (`param_TetGenMaillage::maxAreaOnRecepteurss` is a `float`, `data_manager/projet.h:102`;
  `Element::GetDecimalConfig` returns `float`, `data_manager/element.h:752`). It must be above 0
  and finite as an `f32`, or the mesher refuses the project with `input_invalid`.
- The face set is the one `validate::mesh_input_hash` stamps as `refined_faces`, so changing a
  receiver's groups makes an existing mesh out of date.

## How TetGen reads it

- `load_var` fills `facetconstraintlist` with (marker, area) pairs. A loaded list sets
  `checkconstraints` (`tetgen.cxx:4859-4861`).
- Each facet's bound is set by marker, and only under `-q`: on the subfaces made from the facet
  (`:13898-13902`), and again on every subface before refinement (`:25066-25077`, inside
  `if (!b->nobisect || checkconstraints)`, `:25017`, which a loaded `.var` keeps open under `-Y`).
- TetGen loads a `.var` it finds, whether or not anyone meant it to. The mesher therefore
  deletes any `scene_mesh.var` (and `scene_mesh.edge` and `.mtr`, which TetGen loads the same
  way) before every mesh.

## The pinned TetGen ignores the bound

**Measured 2026-09-23: with the TetGen we ship, a `.var` changes nothing.**

- TetGen 1.6.0 as built from upstream `929a5c8` never reads a subface's area bound when it
  decides whether to split it. `check_subface` tests only the radius-edge ratio
  (`tetgen.cxx:27347-27388`), and `areabound()` is read only to copy a bound onto the pieces of a
  split subface or segment (`:12362, 12607, 12727, 12899-12900, 13030, 16539`).
- **The box with upstream's own `.var`**, meshed `-pq2 -A -n`: TetGen prints
  `Opening scene_mesh.var.`, then makes 6 tetrahedra on the 8 corners. The floor stays 2 faces of
  30 m² each, and the `.mbin` is identical to the one made with no `.var` at all
  (`mesh_project.rs`, `the_pinned_tetgen_reads_the_var_and_refines_nothing`).
- **Upstream's own tutorial-1 mesh** (`tests/fixtures/upstream/tutorial1/spps/tetramesh.mbin`,
  written by the GUI in 2019) has 2,257 tetrahedra and 732 nodes. 934 of its tetrahedron faces
  lie on faces 0 and 1, the largest 0.0998 m², 60.000 m² in all. It was made by a TetGen that
  honoured the bound, which the pinned build cannot reproduce.
- Consequences:
  - Gate M5(a)'s refinement check ("more than 2 tetrahedron faces carry markers 0/1, each at
    most 0.1 m² × (1 + 1e-4)") cannot pass. It is kept, ignored, as `the_var_refines_the_receiver_faces`.
  - The "6 tetrahedra instead of 2,257" that decision 3 attributes to `-Y` has this cause:
    without `-Y` the count is 6 as well. `mesh_settings_conflict` stands for the reason given in
    `docs/solver-contract.md` Part A, but it does not restore refinement.
  - Two ways out, neither decided: a fix to TetGen as a file in `patches/`, or splitting each
    receiver face in the `.poly` before meshing (every piece keeping its face's marker, so the
    markers still index the `.cbin`).

## `-Y`

`-Y` (`nobisect`) forbids splitting any boundary facet: the whole facet-refinement block of
`delaunayrefinement` is under `if (!b->nobisect)` (`tetgen.cxx:29377-29551`). A `.var` and `-Y`
together ask for opposite things, and upstream's GUI clears `-Y` whenever the constraint is turned
on (`data_manager/tree_core/e_core_core_tetconf.h:82-90`). The validator refuses the combination
as `mesh_settings_conflict` (`docs/solver-contract.md` Part A), and so does the mesher.
