## Tutorial 3 fitting zones: findings with receipts

Working files are in `B:/repos/I-Simpa_Night_Mode/target/investigate/tutorial3/fittings/`: `t3/` (the unzipped .proj), the scripts `parse.py`, `simenc.py`, `variants.py`, `variantE.py`, `variantF.py`, `mkpoly.py` and `levels.py`, plus `prep/` and `runs/`. Every solver run used `target/solvers/bin/spps.exe` (the M1 build) on the stored 14h28m18s inputs, with the bed's edits: seed 1, 10,000 particles, and 125 Hz, the config's only computed band.

**1. What the .proj declares.** `projet_config.xml:2430-2920` has two enabled zones:
- **Fitting zone 1**, eid 54, wxid 1930. By computing the `element.h` enum, eid 54 is `ENCOMBREMENT`, the Model type.
  - Inside point `volpos` = (3.61878, 2.66291, 1).
  - Its surfaces come from `4008.finfo`. That file is 84 bytes: 10 faces of mesh group 6, and it is byte-identical to `3059.finfo` of material group `fitting` (idmat 21, `:153`).
  - 125 Hz: alpha 0.2, lambda 1, law 0 (`config.xml:562`).
- **Fitting zone 2**, eid 56, wxid 2083. eid 56 is `CUBOIDE`, the Box type.
  - `ba` = (13, 4, 0), `hc` = (18, 1, 1.2). **ba.y > hc.y, so the corners are not ordered min/max.**
  - 125 Hz: alpha 0, lambda 1 (`config.xml:533`).
- Both tags are `<encombrement>` (`..._model.h:199-201`, `..._cuboide.h:343-347`). Only `eid`, or which children are present, tells the two types apart.

**2. How upstream's GUI turns them into mesh inputs.**
- **.cbin**: `Objet3D_maillage.cpp:783-816` appends the box's `GetConsistentModel` triangles as faces with idMat 0, idRs -1, idEn = xmlId.
  - Stored `mesh.cbin` (3608 B, the same sha `0c03dcff…` in all 3 runs): 76 vertices, 100 faces.
  - Faces 88-99 are `(0,-1,2083)`. Faces 39-48 are `(21,-1,1930)`, the Model zone's scene faces, tagged through `GetFaceLink`.
  - The Model zone adds no triangles.
- **.poly**: `projet_maillage.cpp:206` calls `_SavePOLY(path, true, doMeshRepair, true, …)`. The box's 12 triangles, with 36 unwelded vertices, go to Part 5 with markers 88-99 (`Objet3D_maillage.cpp:966-999`). Then preprocess runs (`:212-213`).
- **Region seeds** (`:1002-1036`), attribute = xmlId:
  - Model zone: `volpos`.
  - Box: `hc-(hc-ba)*BARELY_EPSILON` (`..._cuboide.h:333-336`). The stored `scene_mesh.poly:335-336` holds `1930` at (3.61878, 2.66291, 1) and `2083` at (17.9995, 1.0003, 1.19988).
- **I reproduced upstream's post-preprocess .poly byte for byte.** A GUI-faithful input (the .cbin's 76 vertices, facets 0-87, the 2 regions, 12 Part-5 triangles marked 88-99) run through our `preprocess.exe` gave `prep/gui_like.poly`: 4889 B, sha256 `74b8f831…`, identical to the stored `temp/scene_mesh.poly`. preprocess reported "Vertices merged 28, Coplanar destroyed 2, Face splitted 31", and the result has 57 nodes and 133 facets.
  - [inferred] Our own .poly gives 56/140 because it is built differently: 8 welded corners, the box in Part 2, and a centre seed.
- **Upstream reader bug: every box facet gets the first box marker.** In `poly.cpp:401-424`, `parsedFaces` is not reset and is compared against `sizeFaces` (the model facet count), not `nbUseFace`. After the first user facet the reader stays in `USER_FACE_CONTENT_VALUE`, so every later facet keeps marker 88.
  - Receipt: the output has 19 facets marked 88. When I set the Part-5 markers to 500-511 instead, the output has 19 facets marked 500 (`prep/gui_like_500.poly`).
  - The 19 facets lie on y=4 (8), x=13 (2), y=1 (3), x=18 (4) and z=1.2 (2). **None is at z=0**: the box bottom, faces 88-89, was the part destroyed as coplanar with the floor.

**3. Stored `tetramesh.mbin`.** 338,528 B = 8 + 835×12 + 3285×100; the sha `bc2f0904…` is the same in all 3 runs.

| idVolume | tets | volume (m³) | what it is |
|---|---|---|---|
| 1930 | 56 | 4.35 | fitting zone 1 |
| 2083 | 249 | 18.00 | the box (5×3×1.2) |
| 2084 | 733 | 93.4 | room part |
| 2085 | 1077 | 755.6 | room part |
| 2086 | 1170 | 106.9 | room part |

- The ids match `temp/scene_mesh.1.ele` exactly.
- TetGen 1.5.0 `tetgen.cxx:22352-22455`: seeded regions keep their attribute; each unmarked connected component then gets `maxattr+1`, `+2`, … (`:22404`), bounded by subfaces.
- So the room keeps TetGen's numbers as **three parts, 2084/2085/2086**, divided by the internal wall materials 100/101/102. Both sides of those faces carry the same marker (`interfaces.py`).
- The fittings carry their xml ids (wxid 1930 and 2083).
- All 280 box-interface tet faces (140 on the 2083 side, 140 on the 2084 side) carry marker 88. The largest marker is 88, so faces 89-99 are never referenced.

**4. Why `import-proj` refuses the project.**
- `proj.rs:365-378` refuses any child of `<encombrements>` or `<volumes>`, and `proj.rs:628` sets `fitting_zones: Vec::new()`.
- Command `simpa import-proj …/tutorial_3.proj t3_import.simpa` exits 2 with: `unsupported (proj: … encombrement 'Fitting zone 1': fitting zones and volumes are not imported from a .proj)`.
- The schema already has `FittingShape::{Box, Surfaces}` (`schema/model.rs:565-590`), and `config_xml/import.rs:303-360, 538-570` already reads fittings from a config.xml, but imports every zone, box included, as `Surfaces`.

What a faithful import needs:
- **Tell the types apart** by eid 54 vs 56. Take `enabled` from `prop/useforcalculation`; disabled zones are left out of the config and the .poly (`..._cuboide.h:311,352`, `..._model.h:148`).
- **Read the per-band values** `loi_diff` choice, `alpha` and `lambda` from `absorption/p[@name=<freq>]`.
- **Model zones**: faces from the zone's own `gr/@facesFile`, inside point = `volpos` (upstream falls back to the first face's centroid + 0.05·normal when `volpos` is (0,0,0), `Objet3D_maillage.cpp:1017-1030`).
  - Here the zone's faces are exactly material group `fitting`. [inferred] A zone covering part of a group would need groups split by fitting as well as by (material, receiver).
- **Box zones need componentwise min/max.** Taken literally, `ba`/`hc` would hit `mesh/input.rs:150-160` ("empty box": lo.y 4 ≥ hi.y 1).
- **For .poly parity**, reproduce four things (proven in 2): upstream's triangle and vertex order (`BuildModel`), the 36 unwelded vertices in Part 5, the near-`hc` seed rather than the centre (`input.rs:183`), and the preprocess run.

**5. What SPPS does with fitting faces.**
- **Set up from the .cbin** (`coreinitialisation.cpp:437-445`): a face's `faceEncombrement` = the zone whose id is its idEn.
- **Then overwritten from the tetrahedra, last one wins** (`:151-176`): tets are walked in .mbin order, and every tet with idVolume ≠ 0 writes its zone onto every scene face it touches. `GetEncombrementByOutsideIndex` returns NULL for an unknown id such as 2084 (`base_core_configuration.cpp:384-391`).
- **Cross or bounce** (`CalculationCore.cpp:218`): a particle passes through a face that has no scene face (marker -1), or a face that has a zone and a neighbour tet. Every material here has `side_material="1"`, so any other face is a material collision: specular reflection off **that face's own normal** (`:327`).
- **More than 1000 collision iterations in one step** kills the particle as LOOP (`:337-341`).
- **The fitting free path is redrawn on crossing only when the face has a scene face** (`:384`). A particle starts with `distanceToNextEncombrementEle=0` (`sppsTypes.h:78`), and it is drawn at Run start only inside a zone (`:32-39`).

**6. In upstream's own tutorial 3 result, the box is a reflector with the wrong normal, and 20% of particles are lost.**
- Simulating `:151-176` on the stored files (`simenc.py`): face 88's last writer is tet 3261, room part 2084, so its zone ends **NULL**.
  - Fitting 1's faces 39, 40, 41, 43, 44 and 47 also end NULL (last writer: part 2085). Faces 42, 45, 46 and 48 keep 1930.
  - So the box's sides and top reflect as material 0 (absorb 0) off face 88's **horizontal** plane. Six of fitting 1's ten faces reflect as material 21.
- Particle statistics (`simpa dump gabe`; the columns are atmosphere, materials, fittings, loops, meshing, remaining, total):

| run | loops | total |
|---|---|---|
| 2019 shipped run, 14h28m18s (trans_calc=1) | **536,546** | 2,670,907 (**20.1%**) |
| 2019 shipped run, 14h31m31s (only difference: trans_calc=0) | 1 | 300,000 |
| A: stored .mbin, seed 1 | **106,180** | 529,399 (20.1%) |
| B: room written 0 | 0 | 597,411 |
| D: room kept, box markers -1 | 0 | 608,841 |
| E: room kept, box tet faces re-pointed to their true .cbin face (90/92/94/96/98) | 3 | 598,194 |

- So the losses come from box facets reflecting off face 88's normal.
- [inferred from the geometry] They need transmission because all 6 sources (x 1-7) and all 5 receivers (x 0-4) are in part 2085. The box is in part 2084, which borders the rest only through faces of materials 100 and 101.
- The summed receiver energies of A-E differ by ≤0.9 dB, and seed-to-seed noise in B alone is ≤0.87 dB. **This bed does not resolve an effect on the receivers.**

**7. Decision 5 is not physically equivalent.**
- **Compared with upstream's actual run (A):** D, which is decision 5 with the room kept, never reflects and never loops, where A loses 20%.
- **Compared with a zone boundary made of scene faces (upstream's evident intent; B):** marker -1 skips the redraw at `:384`. [inferred] A particle entering the zone for the first time then carries distance 0, so `:132` forces a collision with the fittings right at the entry point.
  - I measured the mechanism on fitting 1, which the sources reach (`variantF.py`: room 0, its 80 interface tet faces set to -1), over seeds 1-3.
  - Particles absorbed by fittings: **B 4422-4584, F 5040-5202**, ranges not overlapping. Absorbed by the atmosphere: 735-741 vs 635-645.
  - Total energy moves ≤0.03 dB, against about 0.01 dB of seed noise.
- **It never shows on tutorial 3's box itself**: alpha is 0 there, and the box sits behind two walls.

**8. idVolume and the room written 0 (`coreinitialisation.cpp:158,163`).**
- A room written 0 is skipped, so every zone face keeps its zone: face 88 → 2083 and faces 39-48 → 1930 (`simenc.py`).
- Particles then cross into both zones instead of reflecting, and the loops vanish (B).
- [inferred] Once one particle's fate differs, the random streams diverge, which is why all 24 files differ.

**What this means for the decisions:** matching upstream byte for byte means keeping the room as TetGen numbers it and building box zones the GUI's way (.cbin faces, Part 5, preprocess). That reproduces the loop defect.

## UNKNOWNs
- Why the 2019 run left `random_seed="0"`, and so whether its exact particle counts are reproducible.
- `BuildModel`'s triangle and vertex order for the box: I took it from the stored .cbin, not from the source.
- Whether upstream's 1.3.4 and 1.4.0 release binaries (`release-binary/`) loop the same way on these inputs: not run.
- The effect of the loop losses and of decision 5 on receiver levels above Monte Carlo noise: it would need many seeds or more particles.
- Whether any upstream issue or commit reports the `poly.cpp` Part-5 reader bug.