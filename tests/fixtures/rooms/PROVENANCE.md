# Provenance: room fixtures

Upstream's two tutorial projects at upstream commit 929a5c8, imported by
`simpa_core::geometry::import::import_proj`, then renamed and described, and upstream's scene
correction switched off (`solvers.meshing.preprocess` false where both `.proj` files ask for it):
these rooms are the M5 and M6 gates' rooms, meshed without `preprocess.exe`
(`docs/m5-m6-design.md`, decision 12). Nothing else is edited; `tools/gates/m4.ps1` (b)(c)
applies the same edit to a fresh import and requires the rest to be equal.
Regenerate on Grace with
`cargo test -p simpa-core --test geometry_import_proj -- --ignored write_room_fixtures`;
`room_fixtures_are_the_import_of_upstreams_tutorials` fails when these files drift from the
import.

| fixture | source | source sha256 |
|---|---|---|
| `tutorial1_box.simpa` | `src/isimpa/resources/doc/tutorial/tutorial 1/tutorial_1.proj` | `d778983d3f862b30877be39f4119857e10122e53249d88e5a68eb6be5d0a6a7e` |
| `elmia_corrected.simpa` | `src/isimpa/resources/doc/tutorial/tutorial 2/tutorial_2.proj` | `937a6845b23689cf205105c8715498c39f1aa218be1a37b05cb51e35e0d8ea12` |

`elmia_corrected.simpa` replaces Night Mode's `testdata/elmia_corrected.ply`. The geometry is
the same hall, but the groups here come from the `.proj`'s own `.finfo` face lists and `idmat`
material ids. The PLY's centroid-guessed groups are wrong for 2,438 of its 7,860 faces.

Since 2026-09-24 (`docs/m5-m6-design.md`, decision 13) the import pins every source, receiver
and surface receiver to upstream's element id (`solver_id`, the `.proj`'s `wxid`: the box's
surface receiver 1792, receivers 1473 and 1632, source 1799), and records each source's group;
the three derived rooms and their hashes below were regenerated with them. Their hashes have
changed with every key the `.simpa` format gained that day (`destination`, `preprocess`,
`solver_id`, `group`). `tools/fixture-gen/test_fixture_gen.py` holds the files to the derivation
(`test_the_committed_rooms_are_the_derivation`) and every table's sha256, the M7 rooms' below
included, to the files they name (`test_the_provenance_tables_give_each_files_sha256`).

What each file holds is asserted by `crates/simpa-core/tests/geometry_import_rooms.rs`, which
needs nothing outside the repo:

- **Box:** 8 vertices, 12 faces. Groups Floor {0, 1}, Walls {2..9}, Ceiling {10, 11}, and the
  scene surface receiver `Receiver` on faces {0, 1}. Volume 180.000 m3. Source at (3, 5, 1.8),
  receivers at (1, 1, 1.8) and (3, 7, 1.8).
- **Hall:** 3,926 vertices, 7,860 faces, 10 groups. Total area 4,001.809 m2. 11,790 edges, each
  used exactly twice. Signed volume 10,389.096 m3.

The acoustic values in these files are format evidence only, never results to quote.

## Derived rooms

Written by `python tools/fixture-gen/mkrooms.py tests/fixtures/rooms --simpa <simpa.exe>` from
the two imported rooms above, by editing their canonical text. Nothing else changes: the
script parses each result and requires it to equal its source with exactly these edits
applied. With `--simpa` it also requires `simpa validate` to exit 0 (all three do, with no
warnings) and `simpa repair <file> <copy>`, a load and save through the canonical writer, to
give back the same bytes.

| fixture | from | edits | for | sha256 |
|---|---|---|---|---|
| `tutorial1_box_seeded.simpa` | `tutorial1_box.simpa` | SPPS `random_seed` 0 → 1, `particles_per_source` 150,000 → 10,000: M1's reference configuration | gate M6(a) | `9786c83432c2be57` |
| `tutorial1_box_fitting.simpa` | `tutorial1_box_seeded.simpa` | one fitting zone, below | gate M5(e) | `0fb1de7a635cb854` |
| `elmia_loss_gate.simpa` | `elmia_corrected.simpa` | SPPS `random_seed` 0 → 1, `particles_per_source` 1,000,000 → 100,000; `bands_computed` true for 125, 250, 500, 1000, 2000 and 4000 Hz only, in both solvers (SPPS already had exactly these; TCR had all 27) | gate M6(c) | `ae8e2ecbe83309f0` |

The fitting zone: id `0c0be000-0000-4000-8000-00000000f177`, name `Fitting zone`, enabled, a
box from (1, 1, 0.5) to (2, 2, 1.5) m (1 m³, dyadic corners, so the gate's 1e-9 volume check
is exact) with no upstream corner order (`destination` null, a box drawn here; the key was added
on 2026-09-24 with the `.proj` import of rectangular zones), and in all 27 bands absorption 0.1,
mean free path 1.0 m and diffusion law
`uniform`, and no pinned solver id (`solver_id` null: a zone drawn here, numbered 2 by export).
Those pass `fitting_parameters_invalid` (0 ≤ α ≤ 1, λ > 0). The zone clears the
source (3, 5, 1.8) and both receivers, (1, 1, 1.8) and (3, 7, 1.8).

## M7 rooms

Written by `cargo test -p simpa-core --test results_rooms -- --ignored write_m7_rooms` from
recipes in `crates/simpa-core/tests/results_rooms.rs`, which start from
`tutorial1_box_seeded.simpa` and save through the canonical writer.
`the_m7_rooms_are_their_recipes_and_validate_clean` fails when a file drifts from its recipe or
the validator reports anything on it.

Rewritten on 2026-09-24 when the tutorial 3 follow-ups were merged into the M7 line: the files
had been written before the `.simpa` format gained `preprocess`, `Source.group` and `solver_id`
on sources, receivers and surface receivers, and no longer loaded. What the recipes keep of the
seeded box keeps its pins (the source 1799, receivers 1473 and 1632, the surface receiver 1792);
what they make is unpinned, as the level box's material already was: the level box's source and
both its receivers (cloned from the tutorial's first receiver, they would both pin 1473, which
`check_integrity` refuses on load) and the two-source box's `Source 2` (a second 1799). The
committed runs under `tests/fixtures/results/` were made before this rewrite, from
`seats_box.simpa`, `energetic_box.simpa` and `sources2_box.simpa` at the hashes their `run.json`
records; their `config.xml` carries the ids export assigned then.

| fixture | recipe | for | sha256 |
|---|---|---|---|
| `level_box_20m.simpa` | tutorial 1's box stretched to 20 × 20 × 20 m, one group `Walls` of a material with α = 1 in every band; octave bands 125 Hz–4 kHz; one omni source of 100 dB per band at (10.05, 9.97, 10.03); receivers `R2m` and `R4m` 2 m and 4 m from it; SPPS direct field only, air absorption off, random mode, seed 1, 1,000,000 particles, 20 ms in 0.2 ms steps, receiver radius 0.5 m | gate M7(c) | `831e3ce379f9a44e` |
| `seats_box.simpa` | tutorial 1's box on the octave bands 500 Hz and 1 kHz, its receivers renamed `Seat` and `Seat2`, 2,000 particles over 1 s, the floor receiver's faces refined to 4 m² | gate M7(e); its runs are `tests/fixtures/results/` | `ce61b850e78f5bf8` |
| `energetic_box.simpa` | `seats_box.simpa` in energetic mode, `trans_epsilon` 3, 50,000 particles | the M7 review: the solver's floor, energetic completeness; its run is `results/energetic_spps/` | `da78f07b4ed2f042` |
| `sources2_box.simpa` | `seats_box.simpa` with a second source `Source 2` at (5, 8.5, 1.2), 3 dB below `Source 1` and 20 ms late, and `echogram_per_source` on | the M7 review: echograms per source, several sources; its run is `results/sources2_spps/` | `043ca6baf31d6d57` |
| `tutorial1_box_asymmetric.simpa` | `tutorial1_box_seeded.simpa` with the floor's material renamed `Rising absorption`, its α `0.15 + 0.025·i` in band `i` (0.15 to 0.80 over the 27 bands), and the walls' α 0.1 in every band; the ceiling stays 0.3 | gate M7(d), the M7 review: tutorial 1's own absorption averages to the same α by area, by face and by material; here each way, a swap of the floor's and walls' materials, and a band's neighbour's α miss TCR by more than 0.5 % | `7da969c0642a2337` |
