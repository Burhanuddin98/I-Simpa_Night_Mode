# Tutorial 3: what preprocess.exe does, and why our 56/140 differs from upstream's 57/133

**The cause is our input, not our program.** Our build of preprocess.exe reproduces upstream's stored `temp/scene_mesh.poly` byte for byte (4,889 bytes, sha256 `74b8f8311d5d0f64`) when it is given the file upstream's GUI wrote. Our .poly has the same 76 vertices and 100 faces. The difference is where it puts the fitting box's 12 triangles: in Part 2 (scene facets), where upstream puts them in Part 5 (user facets). preprocess.exe only removes overlapping faces when they come from Part 5.

All files are in `B:/repos/I-Simpa_Night_Mode/target/investigate/tutorial3/preprocess/`.

## 1. What preprocess.exe does, in order (upstream source, logic unchanged since the run)
`git diff fa978fb HEAD` over `src/preprocess`, `poly.cpp` and `mathlib.h` shows only header, include and encoding changes, plus a missing `return true;` added to `ImportPOLY`. `fa978fb` is "Update tutorial_3.proj", 2019-06-18 14:32, one minute after the last stored run.

0. **Load** (`CPoly::ImportPOLY`, poly.cpp:257-431). Part 2 becomes `modelFaces` and Part 5 becomes `userDefinedFaces`. There is a bug at poly.cpp:418-423: the user-facet loop tests `parsedFaces < sizeFaces`, the scene-facet counters. So every user facet gets the first user facet's marker. Our poly.rs:757-762 already documents this. Then it builds an octree (computations.cpp:137-138).
1. **`DestroyNoAreaFaces`** (Preprocess.cpp:73, computations.cpp:270-290). It deletes scene facets whose float area is ≤ `BARELY_EPSILON` = `(float)1e-4` (mathlib.h:54,58). That removes anything under 1 cm², not only zero-area faces. User facets are not tested.
2. **`mergeVertices`** (Preprocess.cpp:74, computations.cpp:652-705). Two vertices merge only when their double lengths are bit-equal (multimap key, :666) and each coordinate is within 1e-4 (`barelyEqual`, mathlib.h:145). In practice the tolerance is exact equality: near-duplicates of different length are never merged. Vertices are renumbered in first-occurrence order, in both facet lists.
3. **`MeshDestroyCoplanarFaces`**, repeated while it changes something, up to `MESH_CORRECTION_MAX` = 100 (Preprocess.cpp:39,79-82; computations.cpp:209-268).
   - It loops over user facets only (:215) against scene facets.
   - A user facet is erased when its centroid passes `DotIsInVertex`, an angle-sum test for "in the plane and inside", with deviation < 1e-4 (mathlib.h:725-742).
   - Bug at :245-246: both flags read `ecart1`, so the reverse test (scene centroid inside the user facet) is computed but never used.
4. **`TransferUserFaceToGlobalFaces`** (Preprocess.cpp:85-88, computations.cpp:199-207). The surviving user facets are appended to the scene facets.
5. **`MeshReconstruction`**, repeated up to 100 times (Preprocess.cpp:95-98, computations.cpp:408-491).
   - Faces are paired through the octree and tested with `tri_tri_intersect_with_isectline`.
   - Non-coplanar pairs: the facet that does not already own the intersection point is split there.
   - Coplanar pairs: `coplanarIntersection` (:359-407) splits where a vertex of one facet lies on an edge of the other.
   - `SplitTriangleByThree` (:584-635) splits into 2 facets if the point is collinear with an edge within 1e-4, otherwise into 3. It reuses an existing vertex within 1e-4 (`FindIndexWithPosition`) or appends a new one.
6. **`ShowStats` and `Save`** (Preprocess.cpp:100-106). Everything is written to Part 2, Part 5 is written as `0  1`, and regions pass through unchanged. If 100 tries are used up, it prints "Mesh reparation has been aborted" and does **not** save, leaving the GUI's file. `main` returns 0 in every case (:112-121), so the exit code never signals a failure.

The GUI side:
- It writes box-shaped fitting zones ("drawables") to Part 5 only when the "preprocess" setting is on: `_SavePOLY(path, true, doMeshRepair, true, …)` at projet_maillage.cpp:206, and Objet3D_maillage.cpp:994-995.
- Each triangle gets 3 private vertices (Objet3D_maillage.cpp:984).
- The setting defaults to true (e_core_core_tetconf.h:108). preprocess runs at projet_maillage.cpp:212-213.

## 2. The file before and after preprocess
**Before:** the pre-image is not stored anywhere, because preprocess overwrites it. I rebuilt it as `up_pre.poly` (4,456 bytes) with `build_pre.py`:
- **Scene:** decoded from `instance1/sceneMesh.bin` (6,328 bytes, format 1.2) with `scenebin.py`: 40 vertices and 88 faces in 10 groups, the last one empty ("Model").
- **Box:** fitting 2083, corners ba=(13,4,0) and hc=(18,1,1.2), built as the cuboid element's `BuildModel` does (e_scene_…_cuboide.h:113-165). That gives 12 triangles with 36 private vertices, markers 88-99, in Part 5.
- **Regions:** 1930 and 2083, copied from the stored file (preprocess does not touch them).
- **Check:** my 40 scene vertex lines equal lines 1-40 of the stored post-preprocess file, byte for byte.

**After:** upstream's stored file (4,889 bytes) has 57 vertices, 133 facets and Part 5 = `0  1`.
- Lines 41-48 are the box's 8 corners, in first-occurrence order.
- The new vertices 49-57 all lie at z=0, on the box's footprint lines x=18, y=1 and y=4.

## 3. Running each preprocess.exe build (`run_all.py`, each run in its own copy under `runs/<input>/<build>/`)

| input | build (sha256 prefix) | V / F | stdout stats (merged / destroyed / split) | bytes, equals stored? |
|---|---|---|---|---|
| up_pre | ours target/solvers (bdffba89) | 57/133 | 28 / 2 / 31 | 4889, **identical** |
| up_pre | ours ec9-1 (22d33f34, manifest) | 57/133 | 28 / 2 / 31 | 4889, **identical** |
| up_pre | shipped 1.3.4 (dabf4f70) | 57/133 | 28 / 2 / 31 | 4889, **identical** |
| up_pre | shipped 1.4.0 (22e84ebd) | 57/133 | 28 / 2 / 31 | 4889, **identical** |
| up_pre | shipped 1.3.3 (e5de9789, Dec 2016) | 57/133 | 28 / 2 / 31 | 4729, no: 8-digit coordinates; facets identical except the 10 box markers are 90-99, not 88 |
| ours_pre | all four post-2017 builds | 56/140 | 28 / **0** / 36 | 5008, sha `cfc851287f73e329` |
| ours_pre | shipped 1.3.3 | 56/140 | 28 / 0 / 36 | 4893 |

- **My rebuilt "ours" input matches the bed's.** `ours_pre.poly` (4,417 bytes) through our build gives `cfc851287f73e329`, 5,008 bytes. That equals every `parity-bed/*t3*/preprocess/scene_mesh.poly` (9 folders) and `refusal/ours_after_preprocess_bed.poly`.
- **First difference.** Line 81 of the two inputs reads `100 1` (ours) against `88 1` (upstream). A `diff` of the two pre-images shows only three things:
  - the box triangles sitting in Part 2 instead of Part 5;
  - the region lines;
  - the spacing inside those same triangle lines.

  All 76 vertices are bit-equal by position.
- **Proof that this is the whole cause.** `ours_pre_part5.poly` is our input with facets 88-99 moved to Part 5. Our build then reports 28/2/31, and `diff` against the stored file shows only lines 335-336, the two region lines (our seeds and ids 2 and 3). **So the difference is in the input; the program does not cause it.**
- **Where our input comes from.** `mesh/input.rs:128-137` puts every project face in `model_faces`, and `:227-233` sets `user_defined_faces: Vec::new()`. In the bed, the box arrives from `mesh.cbin` as a "surfaces" fitting (group `4d2ec5e3`): 12 ordinary scene faces.

## 4. What each change repairs on tutorial 3
- **`DestroyNoAreaFaces`:** degenerate or sliver facets. It does nothing here: the smallest of the 88 scene faces is 0.515 m², so 0 are removed. The "destroyed 2" in the stats is therefore all from step 3; both steps share one counter.
- **`mergeVertices`:** a defect of the GUI's own export, which writes the box as 12 disconnected triangles with 36 private vertices (Objet3D_maillage.cpp:984). 28 merges weld them into a closed 8-corner box (76 → 48 vertices).
- **`MeshDestroyCoplanarFaces`:** the box's 2 bottom triangles (`PushTriangle(BC,BD,BA)` and `(BC,BA,BB)`, cuboide.h:157-158) lie on the floor at z=0 and overlap it. They are deleted, and the floor serves as the box's bottom. No marker-≥88 facet at z=0 remains in upstream's output.
  - **In ours** they survive, split into 10 facets covering 15.000 m² (the 5 × 3 m footprint): the z=0 area is 167.78 m² against 152.78 m².
  - **TetGen 1.5.0 on our output:** exit 3, "Found two facets intersect each other. [50,49,55] #107 / [50,49,43] #111".
  - **TetGen 1.6.0 on our output:** exit 2, "Two facets are overlapping at triangle (55,44,49)" and more.
- **`MeshReconstruction`:** the box's bottom edges cross the interiors of floor triangles, so the facets meet at points that are not shared vertices, which is an improper boundary for TetGen. 31 splits (31 "Split Triangle" lines in stdout) add vertices 49-57 on the footprint lines and make the floor conform to the box.
  - Compare fitting 1930, the prism "fitting" group, faces 39-48. It was modelled by hand with no bottom, and the floor faces 61-72 are already triangulated around its footprint. It needs no repair.
- **End to end:** upstream's output through our TetGen 1.5.0 (`-pq2 -A -n`) matches the stored `.1.node`, `.1.ele`, `.1.face`, `.1.neigh` and `.1.edge` once `#` lines are excluded (835 points, 3,285 tets). The only differing line is the trailer, `# Generated by meshing\tetgen\tetgen.exe … D:\picaut\…`.
- **Marker side effect:** in upstream's output all 10 box facets carry marker 88. That is the marker of a bottom triangle preprocess deleted, a result of the reader bug in step 0. The 1.3.3 build predates that reader and keeps 90-99.
- **[inferred]** Our geometry check's `open_boundary` and `self_intersections` refusals on the bed project are these same two defects: the unwelded 36-vertex box and its bottom lying on the floor. The refusal itself belongs to the other lens and I did not test it.

## UNKNOWN
- **The GUI's actual pre-image** is gone, because preprocess overwrote it. My rebuild is output-equivalent: it gives the stored bytes through 4 builds. I cannot prove every byte of it equals what the GUI wrote. In particular, the Part 4 region lines were copied from the output rather than rebuilt, and the drawable iteration order that set them is not reconstructed.
- **Which binary made the 2019 run.** `projet_config.xml:2` says `appversion="1.3.4"`, but the shipped 1.3.4 preprocess.exe is dated 2020-12-23. For this input it makes no difference: our build, 1.3.4 and 1.4.0 are byte-identical here.
- **Merging on other scenes.** Whether the exact-length key in `mergeVertices` leaves near-duplicates unwelded was not exercised: every duplicate in tutorial 3 is bit-exact.
- **Whether preprocess's repair is right.** I did not check that it is acoustically or geometrically correct beyond TetGen accepting its output, for example the 88 marker pointing at a deleted face.