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

Since 2026-09-24 (`docs/m5-m6-design.md`, decision 13) the import pins every source, receiver
and surface receiver to upstream's element id (`solver_id`, the `.proj`'s `wxid`: the box's
surface receiver 1792, receivers 1473 and 1632, source 1799), and records each source's group;
the three derived rooms and their hashes below were regenerated with them.

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
on 2026-09-24 with the `.proj` import of rectangular zones, which changed the file's hash from
`fab25e56605b7008`), and in all 27 bands absorption 0.1, mean free path 1.0 m and diffusion law
`uniform`, and no pinned solver id (`solver_id` null: a zone drawn here, numbered 2 by export).
Those pass `fitting_parameters_invalid` (0 ≤ α ≤ 1, λ > 0). The zone clears the
source (3, 5, 1.8) and both receivers, (1, 1, 1.8) and (3, 7, 1.8).
