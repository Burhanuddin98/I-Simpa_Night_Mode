# TetGen 1.5.0 (option C) on the Elmia hall: receiver faces pass, particle loss fails the strict test

Everything was done under `B:/repos/I-Simpa_Night_Mode/target/investigate/confirm150/` with a copy of simpa.exe at `bin/simpa.exe` (same sha256 as `target/release/simpa.exe`, `aa901110…`). I ran no cargo, deleted nothing and changed no tracked files. The folder now holds 719 MB, and B: had 27.67 GB free at the end.

**Verdict on option C:**

| Criterion | Result |
|---|---|
| 1. Exits 0 with no self-intersection stop | **Pass** |
| 2. Every receiver face ≤ 0.5 m² | **Pass** (max 0.499217) |
| 3. Loss to meshing problems no higher than on the 1.6 mesh | **Fail** as written: +36% |

The 1.5 mesh loses about a third more particles to meshing problems in every seed I ran. It still sits at 6.8e-4, about 15 times under the 0.01 loss limit. The control run says the extra loss comes from 1.5's finer base mesh, not from the receiver refinement.

## Setup
- The project's surface receivers **cover no groups**. Both are `cutting_plane`, and `refined_faces()` (`mesh/input.rs`) gives cutting planes no `.var` rows. Setting the area limit alone would have constrained nothing.
- I added a third receiver, `{"kind":"scene","groups":[audience]}`: 1,568 faces, 722.12 m², largest input face 25.54 m².
- Settings: `surface_receiver_max_area_m2` 0.5, q 2, preserve_boundary false, so TetGen ran with `-pq2 -A -n`. The SPPS block is copied from `elmia_loss_gate.simpa` (seed 1, 6 octave bands), with `particles_per_source` 20000.
- `simpa validate hall_rs.simpa --json`: exit 0, output `[]`.

## 1. Meshing (`simpa mesh hall_rs.simpa --out 16|15 --tetgen <exe> --json`)
| | 1.6.0 (pinned, `c1c754f1…`) | 1.5.0 (`tetgen15.exe`, `6f75c513…`) |
|---|---|---|
| exit / status / codes | 0 / OK / [] | 0 / OK / [] |
| verify codes, inverted, degenerate | [], 0, 0 | [], 0, 0 |
| nodes / tets | 29,472 / 123,718 | 41,933 / 162,697 |
| boundary face rows | 36,716 | 61,486 |
| TetGen elapsed_ms (whole mesh step) | 1165 (1330) | 938 (1127) |
| `.var` rows (sha `b8575d54…` in both) | 1568, and the log shows "Opening scene_mesh.var." | 1568 |
| receiver faces (marker in `.var`) | 7,113 | 12,417 |
| largest receiver face | **6.1439 m²**, 275 faces over 0.5 | **0.499217 m²**, 0 over 0.5 |
| receiver area sum | 722.120 m² | 722.120 m² |
| `.mbin` sha | `cccb6898…` | `c6538d50…` |

- Receiver faces were counted with `receiver_faces.py` from `.1.node` and `.1.face`.
- **Control without a `.var`** (`hall_novar.simpa`):
  - 1.6 gives the byte-identical `.mbin` `cccb6898…`, so 1.6 ignores the `.var` completely on this hall.
  - 1.5 gives 161,543 tets and 60,974 faces (`a7db7660…`). The receiver constraint adds only 1,154 tets.

## 2. SPPS loss (`simpa run <proj> --solver spps --mesh <dir>`)
The pinned `spps.exe` (`cacbbee2…`) was used throughout. Reusing a mesh with `--mesh` was not blocked. Every run: exit 0, verdict OK, warn 0, fail 0, 46 of 46 files. Particles: 60,000 per band.

**Lost to meshing problems** (bands 125 / 250 / 500 / 1k / 2k / 4k Hz):
| run | 1.6 mesh | 1.5 mesh |
|---|---|---|
| seed 1 | 44/28/21/30/27/39 = **189** | 63/42/37/39/31/44 = **256** |
| seed 2 | 26/28/22/19/39/38 = 172 | 43/44/45/37/33/39 = 241 |
| seed 3 | 36/33/24/22/24/38 = 177 | 50/47/27/38/37/37 = 236 |
| seed 1, no `.var` | 189 (same mesh) | 49/45/45/34/37/32 = 242 |

**Lost to infinite loops:**
| run | 1.6 mesh | 1.5 mesh |
|---|---|---|
| seed 1 | 0/0/0/0/2/1 | 0/0/1/0/1/3 |
| seed 2 | sum 3 | sum 6 |
| seed 3 | sum 1 | sum 3 |

**SPPS time** (elapsed_ms / wall):
| run | 1.6 mesh | 1.5 mesh |
|---|---|---|
| seed 1 | 33,677 ms / 34.4 s | 43,570 ms / 44.5 s |
| seeds 2 and 3 | 32.7 s and 32.5 s | 44.2 s and 43.2 s |

- Over the 3 seeds: 1.5 loses 733 against 538 (6.8e-4 vs 5.0e-4 of 1.08M particles).
- 1.5 is higher in 16 of 18 band-and-seed cells.
- Per tet, the rate is almost the same: 4.5e-3 vs 4.35e-3. [inferred] The loss grows with mesh size.

## 3. Raw hall (upstream `tutorial 2/elmia.ply`), self-intersecting
- `simpa import … --unit m --up z --json` exits 0: 1,086 faces, 955 vertices.
- `simpa mesh` (q2, no -Y) exits **3 with both TetGens**, before TetGen runs:
  - degenerate_faces 1;
  - self_intersections, 1,395 pairs over 896 faces;
  - open_boundary;
  - no_enclosed_volume.
- `simpa check` gives the same, exit 3.
- To get past the check I wrote the `.poly` myself with `write_poly.py`. Applied to hall_rs, it reproduces simpa's `scene_mesh.poly` byte for byte (sha `59de5773…`).

| input / flags | 1.6.0 | 1.5.0 |
|---|---|---|
| `simpa mesh elmia_raw.poly` (raw-poly defaults `-pq5 -A -n -Y`) | exit 4; codes tetgen_crash, tetgen_exit_nonzero, tetgen_output_missing, neigh_missing; TetGen **0xC0000005**, stdout cut off at 32,768 B | exit 4; codes tetgen_exit_nonzero, tetgen_output_missing, neigh_missing; TetGen **exit 3**; diagnosis null |
| TetGen direct, `-pq2 -A -n` | 0xC0000005 after 2.0 s, 33,797 B of warnings ("Two segments exactly intersect" ×150, …) | exit 3 in 1.0 s: "Found two facets intersect each other. 1st: [608, 611, 610] #554 2nd: [608, 611, 774] #582 A self-intersection was detected. Program stopped. Hint: use -d" |
| `-pq2 -A -n -Y` | 0xC0000005, identical stdout | exit 3, identical message |
| `-d` | **0xC0000005** (same stdout sha `55a0284b…`) | **exit 0**: "!! Found 2293 pairs of faces are intersecting."; writes `.1.node` and `.1.face` (897 rows, marker = 0-based face index) |
| `_skipped.face` | none written | none written |

- 1.5's "#554 / #582" are 1-based facet numbers. They are our faces 553 and 581, which our own check lists as an intersecting pair.
- 1.5's `-d` `.1.face` markers cover all 896 faces our check flags, plus one more.
- 1.5.0 therefore stops cleanly and names the facets. 1.6.0 crashes on this input with every flag set tried, including `-d`.

## 4. Corner order
- I dumped each `.mbin` (`simpa dump mbin`) and compared every tet with `.1.ele`:
  - 1.6: 123,718 of 123,718 tets are TetGen's (a,b,c,d) order minus 1; 0 are (d,c,b,a).
  - 1.5: 162,697 of 162,697 the same; 0 are (d,c,b,a).
- Example, 1.5 tet 1: `.1.ele` `1749 1834 13646 22186` becomes `.mbin` `1748 1833 13645 22185`.
- So both `.mbin` files use TetGen's order, not upstream's (d,c,b,a) with the float32 round trip. None of the 14 hall runs crashed.

## What argues against C
1. **Criterion 3 fails as written.** The 1.5 mesh is about 31% finer even without a `.var` (1.5 adds 1,128 Steiner points on facets and 25,685 on segments; 1.6 adds 585 and 13,843). That costs +36% meshing loss and +32% SPPS time. The criterion needs rewording to a rate, or to the loss limit, before C can pass it.
2. **Our diagnosis never fires with 1.5.0.** `diagnose()` in `mesh.rs:405-406` runs only when a `_skipped.face` exists, and 1.5.0 never writes one. The self-intersection reaches the manifest only as `tetgen_exit_nonzero` (exit 3) plus `tetgen_output_missing`, and the facet pair appears only in stdout, 1-based. Adopting C needs an adapter that parses exit 3's "Found two facets intersect" or reads the `-d` `.1.face`.
3. **The `.mbin` corner order is unchanged by C.** The (d,c,b,a) rewrite from §5 of the synthesis is still needed. This measurement neither proves nor disproves it.

## What argues for C
1. 1.6.0 misses the limit by up to 12×: 6.14 m² faces, 275 over 0.5 m².
2. On self-intersecting input, 1.6.0 crashes on every flag set tried, including `-d`. 1.5.0 exits 3 and names the pair.

Files are in `B:/repos/I-Simpa_Night_Mode/target/investigate/confirm150/`:
- projects: `hall_rs.simpa`, `hall_rs_s2.simpa`, `hall_rs_s3.simpa`, `hall_novar.simpa`
- meshes: `16/`, `15/`, `nv16/`, `nv15/`
- runs: `runs{16,15}{,_s2,_s3}/`, `runsnv{16,15}/`, plus the `run*.stdout.json` manifests
- raw hall: `raw/` (`elmia_raw.simpa`, `elmia_raw.poly`, `p16/`, `p15/`, `direct_{16,15}_{q2,q2Y,d}/`)
- scripts: `receiver_faces.py`, `write_poly.py`, `seeds.ps1`, `raw_direct.ps1`