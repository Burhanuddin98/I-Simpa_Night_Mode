# Tutorial 3: what it is, and why our pipeline refuses it

Files: the unzip is in `B:/repos/I-Simpa_Night_Mode/target/investigate/tutorial3/scene/instance1/`. My scripts, the variant .proj files, the OBJs and the check JSONs are in `.../tutorial3/scene/lens/`. I ran copies of the worktree's `simpa.exe` (release sha f7f13f69…, debug 0c3b3bf7…; both print `simpa 0.1.0`). Two copies of my files sit in another lens's folder: before I saw that `tutorial3/refusal/` belongs to another lens, I wrote `refusal/bin/`, `refusal/industrial_proj/` and `refusal/import_{release,debug}.*` there. Nothing was deleted.

## 1. The model

- **The file.** `tutorial_3.proj` is 2,517,698 B (sha e3e9fefd…), a zip of 106 entries dated 2019-06-17/18, `appversion="1.3.4"`. The docs say it has no results ("(without Results)", `Docs/tutorial_industrial_hall.rst:21`), but it holds 3 SPPS run folders.
- **Scene.** `sceneMesh.bin` is version 1.2 with 32-byte face records: 40 vertices and 88 faces in 10 mesh groups, plus an empty `Model` group. It has the same faces as `Industrial_hall.ply` (88 faces, 11 layers). Only 16 of 40 vertices are exactly equal; the others differ by at most 2.4e-7 m.
- **Rooms** (from our check's cells):
  - hall, 755.648 m³ (x 0–10, y 0–8, z up to 10)
  - corridor, 106.895 m³
  - room 2, 111.445 m³ (x 12.49–19.10, y 0–5.62, z 0–3)
  - a 4.352 m³ box, which is fitting zone 1
- **Surface groups** (faces / idmat): ceiling 7/21, ceiling_ro 4/21, diff_wall 3/21, door_room1 2/101, door_room2 2/102, ext_walls 21/21, fitting 10/21, floor 25/21, trans_room 8/100, trans_ro01 6/100. Every face is listed exactly once.
- **Materials** in the project database:
  - 100 `trans_material`: law 2 and transmission on only at the six octave bands 125–4k Hz. The other 21 bands have law 0 and transmission off.
  - 101 `Open_door`: α = 1, transmission on at 250–4k Hz but off at 125 Hz, the only band computed.
  - 102 `Absorbing_material`: it sits on door_room2, where doc:197 says `Open_door`. diff_wall stays at 21, where doc:433 says `Absorbing_material`.
- **Sources.** Two source groups, `Milling Machine` and `Milling Machine 2`, three sources each, Lw 80, omnidirectional. They sit at (2,7,1.2), (2,6,0.6), (1,7,0.75), and the same three shifted by (+5,−2,0). Source 3 of each group uses spectrum id 1 (pink); the others use id 0 (white).
- **Receivers.**
  - Five point receivers from (0,1,1.6) to (4,5,1.6). Receiver 1 is at x = 0, which is exactly the hall's west-wall plane (the config.xml has `x="0" y="1"`).
  - One cutting plane at z = 1.6 across the whole building, resolution 0.5. There is no scene surface receiver.
- **Fitting zone 1** (a scene fitted zone):
  - Its surfaces are `4008.finfo`, faces 39–48: the `fitting` group, 4 sides and a top, 10 triangles.
  - Its base is floor faces 69 and 70, which are not listed; doc:111 says to add them.
  - `volpos` is (3.61878, 2.66291, 1), and z = 1 is exactly the box's top plane.
  - α 0.2, λ 1, law 0 (Lambert) on all 27 bands; doc:113 says 0.25 / 0.5 / Uniform.
- **Fitting zone 2** (a cuboid):
  - Corners `ba` (13,4,0) and `hc` (18,1,1.2): 18 m³, standing on room 2's floor.
  - α 0, λ 1, Lambert; doc:136 says 0.15 / 0.3 / Uniform.
- `<volumes>` is empty.
- **Meshing** (SPPS and TC `mesh_conf`): minratio 2, ismaxvol 0, isareaconstraint 0, **preprocess 1**, appendparams empty. The stored `.1.face` trailer records `tetgen.exe -pq2 -A -n`.
- **SPPS settings:** 50,000 particles, 2 s, 2 ms step, energetic, enc_calc 1, trans_calc 0, seed 0, receiver radius 0.31, and only 125 Hz computed.
- **Stored runs:**
  - `14h27m51s`: 150k particles, trans 1. It has inputs and an empty `Surface receiver/` but no results.
  - `14h28m18s`: 50k, trans 1, full results.
  - `14h31m31s`: 50k, trans 0, full results.
  - The three config.xml files differ only on lines 2–3 (working folder, nbparticules, trans_calc). All three have the same `mesh.cbin` (sha 0c03dcff…: 76 vertices, 100 faces) and the same 338,528 B `tetramesh.mbin`.
- **Upstream's mesh** (`temp/`, 3,285 tets), volume per TetGen region, from my own sum:
  - 1930 = 4.3519 m³ (fitting zone 1)
  - 2083 = 18.000 (fitting zone 2)
  - 2084 = 93.4446 (room 2 minus fitting zone 2)
  - 2085 = 755.648 (hall)
  - 2086 = 106.895 (corridor)

## 2a. `import-proj`: four stacked refusals, and a user sees only the first

`simpa import-proj tutorial_3.proj … --json` exits 2 with `unsupported (proj: not supported: encombrement 'Fitting zone 1': fitting zones and volumes are not imported from a .proj)`.

I removed each cause in a copy of the file (`lens/mkvariant*.py`) to reveal the next one:

1. **Fitting zones.** `proj.rs:365-379` refuses any child of `<encombrements>` or `<volumes>`.
2. **Reflection law** (`t3_nofit.proj`): `material 100 'trans_material': the reflection law differs between bands ([0,0,0,0,2,0,0,2,…])`, from `proj.rs:1121-1127`.
3. **Transmission** (`t3_nofit_law.proj`): `material 100: only some bands have transmission on`, from `proj.rs:1134-1143`. Material 101 would fail the same way.
4. **Source groups** (`t3_nofit_mat.proj`): `invalid (projet_config.xml: <sources> has no <prop>)`. `proj.rs:557` lets a `<sources>` group through its tag filter, then `read_source` asks it for `<prop>` at `proj.rs:1305`. The group is never looked inside. [inferred: a bug, not a deliberate refusal.] Industrial.proj (appversion 1.1.5) has the same nesting.

With all four removed and the source groups flattened (`t3_nofit_mat_flat.proj`), the import exits 0: 88 faces, 40 vertices, 6 sources, 5 receivers and the cutting plane. `simpa check` on it gives **verdict ok**:
- 0 intersecting pairs, 0 open edges, 18 edges used three times
- 4 cells: 106.895, 111.445, 755.648 and 4.352 m³
- fitting zone 1's 10 faces are classed as partitions between cell 4 and the hall

`Industrial_hall.ply` also checks ok with the same 4 cells. **The scene itself is not refused.** A preprocess=1 setting is only noted by the importer (`proj.rs:1559-1565`), not refused.

## 2b. The geometry check refuses only the scene plus fitting zone 2's 12 triangles

The bed's refusal is on projects built from each run's config.xml and mesh.cbin (`parity_tutorials.rs:744-760`, asserted at 890-912). `simpa check` on copies of the bed's `run{0,1,2}.simpa` exits 3 for all three, with the same result:
- 100 faces, 76 vertices, 36 open edges, 18 edges used three times, 20 intersecting pairs
- reasons: `self_intersections`, `open_boundary`, `unresolved_topology`

**Where the 12 faces come from.** Upstream's `_SavePOLY` appends each drawable element's triangles with three new vertices per triangle (`Objet3D_maillage.cpp:968-990`). When preprocess is on, they go into a separate user-facet list (`:994-995`); the flag is read at `projet_maillage.cpp:57` and preprocess runs at 212-213. The .cbin carries the same triangles: faces 88–99 on vertices 40–75, material 0, encombrement 2083.

What each refusal code points at:

- **`self_intersections`** (`check.rs:766`; "touching counts", `check.rs:22`). All 20 pairs are room 2's floor faces 53–56 against fitting zone 2's faces 88–97 (`lens/` pair classifier):
  - 6 pairs where fitting zone 2's bottom (faces 88 and 89, z = 0) overlaps the floor in its plane.
  - 10 pairs where a bottom edge of a side face runs across the inside of a floor face, for 0.14 to 4.86 m.
  - 4 pairs where a box corner sits on the inside of a floor face (a T-junction).
  - The top faces 98 and 99 are not involved.
- **`open_boundary`** (`check.rs:778`). Faces 88 and 89 have the exterior on both sides, with 6 open edges. There are 28 coincident vertices: the box's 36 vertices sit at 8 positions. The other 10 box faces are classed as sheets in room 2, with 30 free edges; those alone are accepted.
- **`unresolved_topology`** (`check.rs:818`; `cells.rs:377-380`, no ray decides). 2 of the 13 components cannot be placed; the components are the scene plus the 12 unconnected triangles.

**Counterfactuals** (`simpa check --weld off`, OBJs written from the .cbin):

| Geometry | Verdict |
|---|---|
| Fitting zone 2 welded to 48 vertices | refused: the same 20 pairs, plus 1 component unplaced; a new 18.0 m³ cell appears |
| Welded, bottom faces removed | refused: 14 pairs (side edges and corners on the floor) |
| The 88 scene faces only | ok |
| **Upstream's post-preprocess `temp/scene_mesh.poly`** (57 vertices, 133 facets) | **ok**: 0 pairs, 0 open edges, 5 cells of 106.895, 93.4446, 755.648, 4.3519 and 18.000 m³, which equal TetGen's region volumes |

**What preprocess did**, read from its output:
- welded fitting zone 2 to 8 corners
- added 9 vertices where its bottom edges cross floor edges (40 + 8 + 9 = 57)
- split floor faces 53, 54, 55 and 56 into 5, 7, 9 and 9 triangles; 11 of them tile the box's footprint and become its base
- removed the box's bottom
- split its sides into 19 facets, all with marker 88. Markers 89–99 appear nowhere in the `.poly` or in `.1.face` (1,544 faces, markers 0–88).

So the check is right about the raw geometry: the box as upstream writes it overlaps the floor. It also accepts what upstream's preprocess makes of that geometry.

## 2c. A third refusal, after meshing: our pre-launch check refuses upstream's own run

I ran `simpa run-folder` on copies of the `14h31m31s` config.xml, mesh.cbin and tetramesh.mbin. It exits 5 at stage `pre_launch` (`lens/rf/runs/*/run.json`): `tetramesh.mbin fails mesh::verify with room id 2086: marker_geometry_mismatches 280, uncovered_scene_faces 11, unknown_volume_ids 1810`.
- 1810 = 733 + 1077: the tets of regions 2084 and 2085. The room is three regions, and verify accepts one room id.
- 11 = scene faces 89–99, which no tet face carries as a marker.

## UNKNOWNs

- Which 2 components are unplaced. [inferred: faces 88 and 89, which lie in the floor plane]
- How the 280 marker mismatches break down. [inferred: related to the 140 tet faces with marker 88, since 280 = 2 × 140]
- Whether our `receiver_unlocatable` check would refuse Receiver 1, which sits on the x = 0 wall plane. The run stopped at `mesh_invalid` before reaching it.
- Whether TetGen's placement of region 1930 is reliable, given that the seed lies on the box's top facet. In the stored mesh it went into the box (4.3519 m³).
- Why run `14h27m51s` has no results.