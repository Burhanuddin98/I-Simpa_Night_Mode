# Tutorial 3: is our geometry refusal right?

**Short answer: yes, keep it.** Every defect our check reports is one TetGen really fails on. The one part that is stricter than needed is the unwelded-seam part of `open_boundary`, and that should become an automatic weld. The repair that works is upstream's own: run `preprocess.exe` with the box laid out in the .poly the way upstream lays it out. That reproduces upstream's .poly byte for byte.

All files are in `B:/repos/I-Simpa_Night_Mode/target/investigate/tutorial3/refusal/`, written `L/` below. Tools used:
- TetGen 1.5.0: the wf_14eb660c-ec9-1 build, sha 321491e5.
- TetGen 1.6.0: `target/solvers/bin`, sha c1c754f1.
- spps 22b55a0f and preprocess 22d33f34.
- TetGen flags are the ones upstream's own output files record: `-pq2 -A -n`.

## What gets refused
- **All three codes come from fitting zone 2083, a box, not from the hall.**
  - `simpa check Industrial_hall.ply` (88 faces): exit 0, no reasons (`L/check_ply_z.json`).
  - `simpa check` on run 2's project (100 faces): exit 3, with `self_intersections:20`, `open_boundary:6` and `unresolved_topology:2` (`L/check_run2.json`). `simpa mesh` gives the same three (`L/mesh_refused.err`).
- **Faces 88-99 of the run's mesh.cbin are the box.** It is 12 triangles, each with its own 3 vertices (40-75), material 0, idEn 2083. It spans x 13-18, y 1-4, z 0-1.2 (`L/run2_cbin.dump`). It stands on the floor (faces 53-56, z=0, material 21).
- **The 20 pairs are contacts, not crossings:**
  - 6 are the box bottom (88, 89) overlapping floor faces 53-56 in the same plane.
  - 14 are box sides (90-97) whose z=0 edges run through the inside of floor faces 53-56.

## Five versions of the scene, through our check and both TetGens
I rebuilt the .poly upstream writes before preprocessing from the run's .cbin and the stored region list (`L/make_polys.py`). All 48 of its distinct vertices appear, as identical text, in upstream's stored `temp/scene_mesh.poly`. Upstream's 9 extra vertices all lie at z=0 on the box's footprint edges.

The five versions:
- **A:** the box in Part 2 of the .poly, unwelded. This is what upstream feeds TetGen with preprocess off, and it is our refused scene.
- **C:** A with the box welded.
- **D:** C without the box bottom.
- **UP:** upstream's post-preprocess .poly.
- **PA:** our Part-2 .poly after preprocess (the 56-vertex, 140-facet one).

| version | our check | TetGen 1.5.0 | TetGen 1.6.0 |
|---|---|---|---|
| A | refused: 20 self-intersection pairs, open_boundary 6, unresolved 2 | exit 3, "A self-intersection was detected. Program stopped." | exit 3, 12 input triangles skipped, no .neigh file |
| C | refused: 20 pairs, unresolved 1 | exit 3, same stop | exit 3, 12 skipped |
| D | refused: 14 pairs | exit 3, same stop | exit 3, 7 skipped |
| UP | **accepted**: 0 pairs, 5 cells | **exit 0** | **exit 0** |
| PA | refused: 43 pairs, 1 duplicate face, 18 inverted | exit 3, "Found two facets intersect each other" (#107, #111) | exit 2, "Please report this bug" |

- **Our check agrees with TetGen 1.5.0 on all five.** TetGen 1.5.0 with `-d` on A lists exactly our 20 pairs, as a set (`L/tgd15_A/stdout.txt`). C gives the same 20; D gives 14 of ours.
- **TetGen 1.6.0 skips the floor.** On A it skips all four floor faces 53-56 (37.1 m²) plus box faces 88, 89, 91 and 94 (`L/tg16_A/scene_mesh_skipped.face`).
- **Its partial output is the wrong room:**
  - 163 tetrahedra, all attribute 0, so both fittings are gone.
  - 1,220.9 m³ against the room's 978.3 m³.
  - 22 unmarked hull faces covering 182.2 m² (`L/partial16.py`).
- **SPPS does not notice.** I derived the missing neighbours and ran SPPS on it. It runs to the end: 1 particle lost by meshing out of 60,000, and 0 absorbed by fittings, against about 1,280 on the correct mesh. The lost-particle count cannot detect a wrong domain.

## Upstream's repair, reproduced
- **The repair matches upstream byte for byte.** Take version A but with the box in Part 5, as `_SavePOLY` writes it when "preprocess" is on (`Objet3D_maillage.cpp:994-995`; `L/raw_P.poly`). Run it through our preprocess.exe, the shipped 1.4.0 one or the shipped 1.3.4 one. All three give upstream's stored `temp/scene_mesh.poly` byte for byte: 4,889 bytes, sha256 74b8f831. preprocess reports 28 vertices merged, 2 coplanar faces destroyed, 31 splits.
- **Our 56/140 comes from our .poly writer, not from preprocess.**
  - `crates/simpa-core/src/mesh/input.rs:93-94` writes the box into "the facet list (Part 2, not upstream's Part 5 user list)".
  - preprocess only compares Part-5 faces against model faces when it removes overlapping coplanar faces (`computations.cpp:209-266`).
  - On our layout it reports "Coplanar Faces destroyed : 0" and 36 splits, which gives the PA row above.
- **The rest of the chain also matches.** TetGen 1.5.0 on UP gives all 5 `.1.*` files equal to the stored ones except the last line, which is the command-line trailer. My Python port of `build_mbin` (`L/py_mbin.py`, with the room's parts keeping TetGen's ids) reproduces the stored tetramesh.mbin byte for byte, 338,528 bytes.

## SPPS on the meshes that mesh
10,000 particles × 6 sources = 60,000, 125 Hz, seeds 1, 2 and 3:

| mesh | lost by meshing | absorbed by fittings |
|---|---|---|
| 1.5.0 on UP (upstream's mesh) | 1 / 2 / 0 | 1,286 / 1,278 / 1,263 |
| 1.6.0 on UP, upstream's seed points | 1 / 0 / 0 | **46,171 / 46,081 / 45,965** |
| 1.6.0 on UP, seed for 1930 moved inside | 2 / 3 / 1 (plus 1 lost to an infinite loop on seed 3) | 1,244 / 1,250 / 1,226 |

For reference, upstream's stored 2019 run 2 lost 6 of 300,000.

- **The middle row is wrong, and the cause is upstream's seed point.** TetGen 1.6.0 labels the 755.6 m³ hall as fitting 1930, and the real 4.35 m³ fitting becomes 2085 (`L/ver_UP16.json`). Upstream's seed for 1930, (3.6188, 2.6629, 1.0), lies exactly on the fitting's top face: z=1, distance 0.0 to the plane of faces 39 and 40. TetGen 1.5.0 happens to pick the inside.
- With the inside point our importer computes, (3.913, 2.578, 0.6), TetGen 1.6.0 labels correctly and TetGen 1.5.0's .mbin does not change by a byte.

## Side finding: upstream's reader gives every box facet marker 88
- **Cause:** `poly.cpp:418-419` compares `parsedFaces`, never reset after Part 2, against the Part-2 count. Every Part-5 facet after the first keeps marker 88. The stored post-preprocess .poly has 19 facets with marker 88 and none with 89-99.
- **Our checks reject upstream's own mesh because of it:**
  - `mesh-verify` on upstream's tutorial-3 .mbin exits 4: 280 marker/geometry mismatches, 11 scene faces uncovered (89-99), 1,810 tetrahedra with an unknown volume id (`L/verify_UP15.json`).
  - `simpa run-folder` refuses it before launch: exit 5, `mesh_invalid` (`L/run_UP15.json`).
- **Consequence:** if we adopt preprocess, `mesh::verify` has to accept this, or we fix the markers.

## Verdict per refusal code
- **`self_intersections`: keep the refusal for the raw scene.** It is a real defect: TetGen 1.5.0 stops on exactly these 20 pairs, and 1.6.0 drops the floor and hands SPPS a wrong domain it runs without complaint. **Repair automatically:** write box zones into Part 5 as upstream does, run preprocess.exe, then run our check again as the gate. On tutorial 3 that yields upstream's .poly byte for byte, which our check accepts and TetGen meshes.
- **`open_boundary`: relax the unwelded-seam part into an automatic weld.** Here the check is stricter than needed:
  - The 36 open edges are the box's unwelded triangles (28 coincident vertices).
  - TetGen ignores coincident points itself ("Point #61 is coincident with #75. Ignored!", `L/tg15_A/stdout.txt`), and preprocess merges them.
  - Welding alone (version C, and `simpa repair`, which welds and still exits 3 on the remaining pairs) clears this code and changes nothing in TetGen's result.
  - The "2 faces with the exterior on both sides" are the coplanar box bottom, which `self_intersections` already covers.
- **`unresolved_topology`: keep it.** It is a consequence, not a separate defect: the box is its own component, touching the room. It drops from 2 to 1 after welding, is gone in D and absent on UP, and it never blocked a scene TetGen could mesh.

## UNKNOWN
- Whether marker 88 on the box's sides and top, instead of 89-99, changes SPPS results. All 12 box faces in the .cbin share material 0 and idEn 2083, and no run isolates it.
- Why SPPS loses only 1 particle on the partial 1.6.0 mesh despite 182 m² of unmarked hull faces. Not traced.
- Whether preprocess repairs other scenes like this one. Only tutorial 3 was tested. When its repair loop gives up, it prints "Mesh reparation has been aborted", skips saving and still exits 0 (`Preprocess.cpp:100-108`). A wrapper must check that the file actually changed.
- What upstream's GUI does with TetGen 1.6.0's exit-3 partial files. Not checked.
- Another agent wrote into my folder at 05:16: `bin/`, `import_*.err` and `industrial_proj/`. I left them untouched.