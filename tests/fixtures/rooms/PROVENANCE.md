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
