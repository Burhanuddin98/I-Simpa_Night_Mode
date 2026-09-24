# Provenance: room fixtures

Upstream's two tutorial projects at upstream commit 929a5c8, imported by
`simpa_core::geometry::import::import_proj`, then renamed and described. Nothing else is edited.
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
| `tutorial1_box_seeded.simpa` | `tutorial1_box.simpa` | SPPS `random_seed` 0 → 1, `particles_per_source` 150,000 → 10,000: M1's reference configuration | gate M6(a) | `960dd67769665c29` |
| `tutorial1_box_fitting.simpa` | `tutorial1_box_seeded.simpa` | one fitting zone, below | gate M5(e) | `5dda008fe04be568` |
| `elmia_loss_gate.simpa` | `elmia_corrected.simpa` | SPPS `random_seed` 0 → 1, `particles_per_source` 1,000,000 → 100,000; `bands_computed` true for 125, 250, 500, 1000, 2000 and 4000 Hz only, in both solvers (SPPS already had exactly these; TCR had all 27) | gate M6(c) | `d1c4245a41fb30ed` |

The fitting zone: id `0c0be000-0000-4000-8000-00000000f177`, name `Fitting zone`, enabled, a
box from (1, 1, 0.5) to (2, 2, 1.5) m (1 m³, dyadic corners, so the gate's 1e-9 volume check
is exact) with no upstream corner order (`destination` null, a box drawn here; the key was added
on 2026-09-24 with the `.proj` import of rectangular zones, which changed the file's hash from
`fab25e56605b7008`), and in all 27 bands absorption 0.1, mean free path 1.0 m and diffusion law
`uniform`. Those pass `fitting_parameters_invalid` (0 ≤ α ≤ 1, λ > 0). The zone clears the
source (3, 5, 1.8) and both receivers, (1, 1, 1.8) and (3, 7, 1.8).

## M7 rooms

Written by `cargo test -p simpa-core --test results_rooms -- --ignored write_m7_rooms` from
recipes in `crates/simpa-core/tests/results_rooms.rs`, which start from
`tutorial1_box_seeded.simpa` and save through the canonical writer.
`the_m7_rooms_are_their_recipes_and_validate_clean` fails when a file drifts from its recipe or
the validator reports anything on it.

| fixture | recipe | for | sha256 |
|---|---|---|---|
| `level_box_20m.simpa` | tutorial 1's box stretched to 20 × 20 × 20 m, one group `Walls` of a material with α = 1 in every band; octave bands 125 Hz–4 kHz; one omni source of 100 dB per band at (10.05, 9.97, 10.03); receivers `R2m` and `R4m` 2 m and 4 m from it; SPPS direct field only, air absorption off, random mode, seed 1, 1,000,000 particles, 20 ms in 0.2 ms steps, receiver radius 0.5 m | gate M7(c) | `88a6a29bc49b797d` |
| `seats_box.simpa` | tutorial 1's box on the octave bands 500 Hz and 1 kHz, its receivers renamed `Seat` and `Seat2`, 2,000 particles over 1 s, the floor receiver's faces refined to 4 m² | gate M7(e); its runs are `tests/fixtures/results/` | `e030381d37e5c7ea` |
| `energetic_box.simpa` | `seats_box.simpa` in energetic mode, `trans_epsilon` 3, 50,000 particles | the M7 review: the solver's floor, energetic completeness; its run is `results/energetic_spps/` | `65f81942d1e7487b` |
| `sources2_box.simpa` | `seats_box.simpa` with a second source `Source 2` at (5, 8.5, 1.2), 3 dB below `Source 1` and 20 ms late, and `echogram_per_source` on | the M7 review: echograms per source, several sources; its run is `results/sources2_spps/` | `dd31727243d1075c` |
| `tutorial1_box_asymmetric.simpa` | `tutorial1_box_seeded.simpa` with the floor's material renamed `Rising absorption`, its α `0.15 + 0.025·i` in band `i` (0.15 to 0.80 over the 27 bands), and the walls' α 0.1 in every band; the ceiling stays 0.3 | gate M7(d), the M7 review: tutorial 1's own absorption averages to the same α by area, by face and by material; here each way, a swap of the floor's and walls' materials, and a band's neighbour's α miss TCR by more than 0.5 % | `36241b6793a52118` |
