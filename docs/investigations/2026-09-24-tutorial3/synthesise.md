# Tutorial 3: why our pipeline refuses it, and how to get it through faithfully

**Bottom line.** Tutorial 3 is refused at three separate gates. The 88-face hall is not the problem. The problem is fitting zone 2, a box that upstream writes as 12 loose triangles standing on the floor.
- **Our geometry refusal is right.** TetGen 1.5.0 stops on exactly the same 20 face pairs.
- **Our preprocess.exe is not faulty.** It gives upstream's stored .poly byte for byte once the box sits in the user-facet part of the .poly, where upstream puts it.
- **A faithful run also copies a defect in upstream's own result.** With transmission on, 20.1% of particle records end as infinite loops.

## 1. What tutorial 3 is, and each refusal
- **The model** (CONFIRMED, from the scene lens's unzip). `tutorial_3.proj` is 2,517,698 B, appversion 1.3.4, an industrial hall. The scene has 88 faces and 40 vertices:
  - rooms: hall 755.648 m³, corridor 106.895 m³, room 2 111.445 m³;
  - fitting zone 1: a 4.352 m³ prism modelled by hand (faces 39-48);
  - fitting zone 2: an 18 m³ box, 5 × 3 × 1.2 m, on room 2's floor.
  Two source groups of 3 sources each (Lw 80), 5 receivers and a cutting plane at z = 1.6. SPPS runs 50k particles for 2 s at 125 Hz only. `preprocess = 1`; TetGen is run with `-pq2 -A -n`.
- **Upstream's mesh** (CONFIRMED, from the `.1.ele` ids and summed region volumes). 3,285 tets in 5 TetGen regions: 1930 = 4.352 (zone 1), 2083 = 18.000 (box), 2084 = 93.445 (room 2 minus the box), 2085 = 755.648 (hall), 2086 = 106.895 (corridor).

**Gate A: `import-proj` exits 2.** It refuses for four reasons stacked in sequence, and a user sees only the first. CONFIRMED by removing each in copies (`scene/lens/mkvariant*.py`):
1. Fitting zones. `proj.rs:365-379` refuses any child of `<encombrements>` or `<volumes>`.
2. Material 100's reflection law changes between bands (law 2 at 125-4k Hz, law 0 elsewhere): `proj.rs:1121-1127`.
3. Materials 100 and 101 have transmission on for only some bands: `proj.rs:1134-1143`.
4. Source groups fail with `<sources> has no <prop>`. `proj.rs:557` lets a group through, then `read_source` looks for `<prop>` in it at `:1305`. [inferred] This is a bug; Industrial.proj nests its sources the same way.

With all four removed, the import exits 0 and `simpa check` returns ok with 4 cells. **The scene on its own passes.** CONFIRMED.

**Gate B: the geometry check exits 3** on the bed's project (config.xml plus mesh.cbin: 100 faces, 76 vertices). CONFIRMED (`refusal/check_run2.json`). Every reason comes from the box's 12 triangles (faces 88-99, each with its own 3 vertices):
- **`self_intersections`, 20 pairs.** 6 are the box bottom (faces 88 and 89, at z = 0) overlapping floor faces 53-56 in the same plane. 10 are side-face bottom edges running through floor-face interiors. 4 are box corners landing inside floor faces.
- **`open_boundary`, 36 open edges.** The box is not welded: its 36 vertices sit at 8 positions.
- **`unresolved_topology`.** 2 components cannot be placed.

**Gate C: the pre-launch `mesh::verify` exits 5 on upstream's own .mbin.** CONFIRMED (`refusal/run_UP15.json`, `scene/lens/rf/`). It reports:
- 1,810 unknown volume ids. These are the 733 + 1,077 tets of rooms 2084 and 2085, because verify accepts only one room id.
- 11 scene faces uncovered: faces 89-99.
- 280 marker mismatches. These are the 140 + 140 tet faces on the two sides of the box interface, all carrying marker 88 (fittings lens, `interfaces.py`). This settles the scene lens's inferred 2 × 140.

## 2. preprocess.exe
**What it does,** from upstream source. The logic is unchanged since `fa978fb`, one minute after the last stored run. CONFIRMED (preprocess lens, source cites).
0. **Reads the file.** Part 2 becomes scene facets and Part 5 user facets. A reader bug (`poly.cpp:418-423`) gives every user facet the first one's marker, 88.
1. **Deletes tiny scene facets:** any under 1e-4 m² in float, so up to 1 cm², not only zero-area ones. None on T3; the smallest face is 0.515 m².
2. **Merges vertices** that are bit-equal. On T3: 28 merges weld the box to 8 corners.
3. **Deletes user facets that lie in the plane of a scene facet and inside it.** It checks only user facets. On T3 this removes the box's 2 bottom triangles.
4. **Appends the surviving user facets** to the scene facets.
5. **Splits facets where they cross** until they meet at shared vertices. On T3: 31 splits, 9 new vertices on the box footprint at z = 0.
6. **Saves.** If it runs out of passes it prints "Mesh reparation has been aborted" and does not save, but still exits 0.

**Why ours gives 56/140.** Our .poly writer puts the box in Part 2:
- `mesh/input.rs:93-94` does this for native Box zones.
- The bed's box arrives as a Surfaces fitting from mesh.cbin and also lands in Part 2 (`input.rs:230`, `user_defined_faces: Vec::new()`).

Step 3 only looks at Part 5, so the box bottom survives ("Coplanar destroyed 0", 36 splits). The 15 m² footprint is then covered twice: the area at z = 0 is 167.78 m² against 152.78 m². CONFIRMED. Moving facets 88-99 to Part 5 in our input (`ours_pre_part5.poly`) gives the stored file except its 2 region lines. CONFIRMED.
- **A contradiction, resolved.** The fittings lens inferred "8 welded corners, Part 2, centre seed". That describes the native Box path. The bed's 56/140 comes from 76 unwelded vertices in Part 2: `ours_pre.poly` produces `cfc851287f73e329`, the same as all 9 bed folders. In both paths the cause is Part 2 against Part 5.

**Which binary reproduces 57/133.** All four post-2017 builds do, from the Part-5 input: ours (`target/solvers`, `bdffba89`), ours (`ec9-1`, `22d33f34`), shipped 1.3.4 and shipped 1.4.0. Each gives 4,889 B, sha256 `74b8f8311d5d0f64`, equal to the stored `temp/scene_mesh.poly`. CONFIRMED: I re-hashed `preprocess/runs/up_pre/*`, `fittings/prep/gui_like.poly`, `refusal/pre_ours_P` and `refusal/pre_s140_P`. Shipped 1.3.3 (2016) gives 4,729 B: the same facets, but 8-digit coordinates and box markers 90-99, because it predates the reader bug.

**Downstream matches too.** The stored .poly through our TetGen 1.5.0 gives `.1.node`, `.ele`, `.face`, `.neigh` and `.edge` equal to the stored files except the trailer line. A build_mbin that keeps TetGen's region ids reproduces the stored `tetramesh.mbin` (338,528 B) byte for byte. CONFIRMED (`refusal/py_mbin.py`).

## 3. Verdict per refusal
| Refusal | Verdict | Evidence |
|---|---|---|
| import: fitting zones | too strict (missing feature) | the schema already has `FittingShape::{Box,Surfaces}` (`model.rs:565-590`); the config_xml import already reads fittings |
| import: law or transmission varies by band | [inferred] too strict | upstream stores these per band; only 125 Hz is computed; the effect is unmeasured |
| import: source groups | [inferred] a bug | the group's contents are never read |
| `self_intersections` | **right** | TetGen 1.5.0 `-d` lists exactly our 20 pairs and exits 3. TetGen 1.6.0 skips the floor and meshes 1,220.9 m³ against 978.3 m³ with both fittings gone; SPPS runs that mesh and loses only 1 particle of 60,000 |
| `open_boundary` | half right | the 36 open edges are unwelded seams: welding clears them (version C), and TetGen ignores coincident points. The 2 two-sided bottom faces are already counted as self-intersections |
| `unresolved_topology` | right, a consequence | 2 before welding, 1 after, 0 on upstream's repaired .poly |
| verify: 1,810 unknown ids | too strict for a faithful mesh | the room is three TetGen regions |
| verify: 280 + 11 | right about the file | caused by upstream's marker-88 reader bug |

Our check accepts upstream's repaired .poly: 0 pairs, 5 cells equal to TetGen's region volumes. CONFIRMED (`refusal/chk_UP.json`).

## 4. Fitting zones, decision 5, decision 1
**How upstream represents them** (CONFIRMED, fittings lens):
- **Zone 1** (eid 54, Model, id 1930). It adds no triangles of its own; its scene faces 39-48 get idEn 1930 in the .cbin. Its seed is `volpos` (3.61878, 2.66291, 1), which lies exactly on its top face.
- **Zone 2** (eid 56, Box, id 2083). Its corners `ba` and `hc` are not ordered min/max (ba.y = 4, hc.y = 1). Its 12 triangles go into the .cbin as faces 88-99 (idMat 0, idEn 2083), and into Part 5 of the .poly with 36 private vertices. Its seed is hc − (hc − ba)·1e-4 = (17.9995, 1.0003, 1.19988). Read literally, those corners trip our "empty box" check (`input.rs:150-160`).
- **In SPPS,** each face's zone is set from its .cbin idEn. Then every tet with a nonzero idVolume that touches the face overwrites it, and the last tet in .mbin order wins. An id that names no zone, such as 2084, writes NULL (`coreinitialisation.cpp:151-176`).

**What this does to upstream's own result** (CONFIRMED). Face 88's last writer is room tet 3261 (region 2084), so its zone becomes NULL. The box's 10 surviving facets all carry marker 88, so they reflect as material 0 off face 88's horizontal plane. 6 of zone 1's 10 faces also end NULL.

Loop losses, 125 Hz. I re-read the stored and A/B/D statistics files with `simpa dump gabe`:

| Run | Loops / total |
|---|---|
| stored 14h28m18s, trans_calc 1 | 536,546 / 2,670,907 (20.1%) |
| stored 14h31m31s, trans_calc 0 | 1 / 300,000 (6 lost to meshing: the refusal lens's "6 of 300k") |
| A: stored .mbin, seed 1 | 106,180 / 529,399 (20.1%) |
| B: room idVolume written as 0 | 0 / 597,411 |
| D: room ids kept, box markers −1 (decision 5) | 0 / 608,841 |
| E: room ids kept, box faces pointed at their true faces | 3 / 598,194 |

Summed receiver levels across A-E differ by at most 0.9 dB, against seed noise of up to 0.87 dB. This bed cannot resolve an effect at the receivers.

**Decision 5 is not physically equivalent.**
- Against upstream's actual run, it removes the 20% loop loss.
- Against the evident intent, zone boundaries as scene faces, it changes behaviour at the boundary. Crossing a −1 face does not redraw the fitting free path (`CalculationCore.cpp:384`), so [inferred] a particle entering for the first time collides right at the entry point.
- Measured on zone 1 over seeds 1-3: particles absorbed by fittings were 4,422-4,584 (B) against 5,040-5,202 (F); the ranges do not overlap. Total energy moves by at most 0.03 dB against about 0.01 dB of seed noise. The effect is small but resolved.

**What decision 1 (the idVolume reversal) must do.**
- Write each tet's idVolume exactly as TetGen's region attribute: 1930 and 2083 for the fittings, and TetGen's own numbers for the room's parts (2084/2085/2086, maxattr+1 onward, `tetgen.cxx:22404`). Never 0, and never merged into one room id. CONFIRMED: this rule reproduces the stored 338,528 B .mbin.
- `mesh::verify` must then accept a room made of several ids, or Gate C stays shut.
- [inferred from A against B] It will reproduce upstream's 20% loop loss on T3 whenever transmission is on.

## 5. Options
1. **Import fittings faithfully.** Needed whatever else we choose, because Gate A stops everything.
   - Work: tell the types apart by eid 54 or 56; read `useforcalculation` and per-band α, λ and law; take min/max of the box corners; use `volpos`. Also fix the other three import refusals.
   - Cost moderate, risk low. Not enough alone: the box still overlaps the floor.
2. **Opt-in upstream preprocessing.**
   - Work: write box zones into Part 5 as `_SavePOLY` does (36 private vertices, the near-hc seed), run preprocess.exe, then run our check again as the gate. This is proven byte for byte on T3.
   - Cost: a change to the .poly writer; a wrapper that detects an aborted repair, since the exit code is always 0 (compare the file); and verify accepting marker 88 and multi-id rooms.
   - Risk:
     - Only tested on T3.
     - It silently deletes facets under 1 cm².
     - It carries the marker bug, and with it the loop defect.
     - With seed 1930 on a face, TetGen 1.6.0 labels the hall as the fitting: 46,171 particles absorbed by fittings against 1,286. Pin TetGen 1.5.0, or move that seed inside; moving it leaves 1.5.0's .mbin unchanged.
3. **Teach our own repair to do it, visibly.**
   - Work: weld (already exists), delete zone faces lying inside scene faces, split facets to conform.
   - Cost high: robust triangle splitting in Rust. Risk: not byte-equal to upstream, so we lose parity on T3.
   - Gain: every change is reported, and there is no marker bug.
4. **Relax checks.** Only the weld part of `open_boundary` can be defended, and it does not get T3 through (the welded version still has 20 pairs). Relaxing `self_intersections` hands TetGen 1.6.0 a wrong room, which SPPS then runs without complaint. Reject.

**Recommendation: options 1 and 2.** Preprocessing becomes an explicit opt-in step, with our check re-run after it. The checks stay strict, apart from adding an automatic weld. Record the loop defect as an upstream finding, and do not patch it in parity mode.
**The one measurement:** take `tutorial_3.proj` through our import, .poly, preprocess and TetGen 1.5.0, then byte-compare the resulting .mbin with the stored `tetramesh.mbin` (338,528 B, sha `bc2f0904…`). Equality confirms the import, the Part-5 layout, the seeds, preprocess and the idVolume rule together.

## Decisions for Burhan
- **M6(a).** On T3, upstream's own mesh loses 20.1% to loops with trans_calc = 1, so "no more lost than upstream's own mesh" would pass that. A lost-particle count also cannot see a wrong room: the 1.6.0 mesh of the wrong room lost 1 of 60,000. Suggestion: compare meshing losses only, and add a check on the meshed volume.
- **The loop defect.** Parity mode reproduces it (decision 1 plus marker 88). Keep it for parity only, or also offer a corrected mode? Room id 0 gave 0 loops; true markers gave 3.
- **Opt-in preprocessing.** Not needed tonight. Yes or no for the release arc.

## UNKNOWN
- Whether preprocess repairs any other scene correctly: only T3 was tested.
- The GUI's actual pre-preprocess .poly, which preprocess overwrote. Our rebuilt input gives the stored output byte for byte through 4 builds, but its region lines were copied from the stored output.
- The effect on receiver levels of the loop losses and of decision 5: not resolved above seed noise at 10k particles per source.
- Whether `receiver_unlocatable` refuses Receiver 1, which sits exactly on the x = 0 wall. The run stopped at `mesh_invalid` first.
- Which 2 components are unplaced. [inferred] Box bottom faces 88 and 89.
- Which binary made the 2019 run. It does not matter here: all four post-2017 builds give the same bytes.
- Why run 14h27m51s has no results, and why the seed was left at 0.
- Whether the .proj's departures from the tutorial docs are intended: door_room2 uses material 102, and the zones' α is 0.2 and 0 where the docs say 0.25 and 0.15.
- Whether the shipped 1.3.4 and 1.4.0 spps.exe loop the same way. Not run.

Files: `B:/repos/I-Simpa_Night_Mode/target/investigate/tutorial3/{scene,preprocess,refusal,fittings}/`