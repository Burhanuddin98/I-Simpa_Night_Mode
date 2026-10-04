# Wow list W1 and W5: the ear-height plane, and smooth colour, contours, fixed range and probe (2026-10-04)

Burhan 11:45: "IT WOULD BE COOL TO HAVE SOME OF THESE OTHER COOL FEATURES". Definitions:
`docs/investigations/2026-10-03-wow/WOW-LIST.md` W1 (parity M41) and W5 (R46, R47, R48, R51). Branch `wow-w1w5`
off `ir-window` (which holds `glass`), worktree `.claude/worktrees/wow-w1w5`, cargo target `C:\tmp\nm-target-wow`.
Receipts and screenshots: `B:\data\m12\wow\`.

## What is already there (read before writing)

- The core has the plane: `SurfaceReceiverShape::CuttingPlane {a, b, c, resolution_m}` (schema/model.rs:632), the ops
  `add_surface_receiver`, `replace_surface_receiver`, `remove_surface_receiver` (schema/ops.rs:210-219), and the check
  `cutting_plane_invalid` (validate/project.rs:920: collinear corners, resolution <= 0 or above a side, too many cells).
  The checked apply (`bridge.rs:542`) refuses any edit that adds an error. **No new core code is needed**; W1 is UI.
- The Results step already draws any cutting-plane `.csbin` per band and step (M12 P3, group "Cutting planes").
- Upstream's new plane (`e_scene_recepteurss_recepteurcoupe.h:89-104`): A = (xmin, ymax, zmin + 1.6),
  B = (xmin, ymin, zmin + 1.6), C = (xmax, ymin, zmin + 1.6) over the model's box; resolution 0.5 m (`_proprietes.h:51`).
- Upstream's smooth colour (`Recepteurs_surfacique.cpp:384-430, 639-650`): a node's energy is the sum of its linked
  faces' energies over their count, then dB; the colour is set per vertex.

## W1: place an ear-height plane (M41, the smaller slice)

1. `ops.ts`: builders for the three surface-receiver ops and `newCuttingPlane`.
2. `chrome/planes.ts` (pure): `earPlane(bbox)` by upstream's rule at floor + 1.6 m; the cell count
   (`ceil(|BC|/r) x ceil(|BA|/r)`, two map faces a cell); the height of a level plane (null when tilted); the
   height and resolution fields' parse, saying NO to a non-number, a height outside the room's floor-to-ceiling range,
   a resolution <= 0; the planes of the project that a run does not hold (by name).
3. Sources & receivers panel: a "Sound-level planes" section, `+ Ear-height plane`, each plane with its height above
   the floor and resolution (committed through the checked apply: the core's `cutting_plane_invalid` refuses inline),
   its cell count, Remove; the hint "A new or moved plane has no map until SPPS runs again."
4. The 3D view draws each plane's outline and grid (upstream's `DrawPlan`) off the Results step.
5. Results: the map panel says "<plane> is not in this run: run SPPS again to map it." for a project plane the
   selected run's `.csbin` does not hold.
6. **Default resolution 1 m, not upstream's 0.5 m**: P3 measured Elmia's two planes (2,858 triangles) at 114 MB of
   texture at 10,000 steps; a 0.5 m plane over the whole hall is about four times one at 1 m. The field takes 0.5.

Left (said in the report): a 3D drag gizmo for the three corners; tilted planes are shown read-only.

## W5: how the maps look (R46, R47, R48, R51)

1. `features/viewport/mapView.ts` (pure): each node's linked faces (CSR), upstream's node mean, the fixed-range
   parse (NO to lo >= hi, a non-number, a span beyond 200 dB), the probe's text from the file's own float32.
2. GPU (`resultsLayer.ts`): a node-to-faces texture; the vertex shader averages the faces' texels at the step
   (upstream's mean) and passes the node's level, interpolated across the face; the fragment colours from it (the
   value is interpolated, not the RGB). A face with no energy is still not drawn; a face whose corner has none falls
   back to its own flat colour. Contours: lines every 1, 3 or 6 dB on the smoothed level (`fwidth`), only with smooth
   on. Fixed range: the legend and the colour scale take the typed lo/hi, kept across bands and runs, level maps only.
3. Probe: on the Results step, the pointer over the map (and not behind a wall) names the face, and a card shows the
   **face's own value from the `.csbin`**, its level in dB re 1e-12 to 0.1 dB, the band, the step's time, and "no
   energy at this step" where the file has none; under smooth colour it says the colours between faces are smoothed.
   A difference map's probe shows this run minus the baseline and both levels.

## Tests first

- UI (`node --test`): `planes.test.ts`, `mapView.test.ts`, additions to `ops.test.ts`; each with a say-NO case.
- e2e `m12.plane` (W1): `w1-add` (the button adds the plane through the core at the upstream corners, floor + 1.6 m),
  `w1-refuse` (resolution 0 refused by the core, `cutting_plane_invalid`, project unchanged; a height above the
  ceiling refused by the field), `w1-rerun` (the run made before the plane says it is not in the run), `w1-map`
  (after SPPS, the plane's map holds 2 x cells faces and its texels equal the file). Screenshot.
- e2e `m12.mapview` (W5): `w5-smooth` (node levels read back from the GPU equal the spec's own mean from the
  `.csbin`; the picture changes), `w5-contours`, `w5-range` (legend and scale equal the typed range; lo >= hi
  refused), `w5-probe` (hover a face: the card's value bits equal the file's, level to 0.1 dB; off the map, no card).
  Screenshot.
- Both registered in `tools/gates/m11.ps1`'s spec table; run with `m12.viewport` as the regression.

## Result (12:51)

Built as planned, on `wow-w1w5` (not pushed, not merged). Receipts and screenshots: `B:\data\m12\wow\` (`red-*.txt`,
`e2e-try{1,2,3}.log`, `shots\w1-plane-placed.png`, `w1-plane-map.png`, `w5-probe.png`).

- UI suite 217 of 217 (`planes.test.ts` 9, `mapView.test.ts` 9 new, each with a say-NO case); typecheck clean; e2e
  harness reader test 4 of 4; `cargo test -p app --bins` 70 of 70 (new: `an_ear_height_plane_goes_through_the_checked_apply`,
  the UI's op text through `edit_apply`, `cutting_plane_invalid` refusing a cell larger than a side and collinear
  corners); `simpa-core` `schema_roundtrip` 17 and `validate_fixtures` 10 pass (the latter needs `SIMPA_SOLVERS_DIR`).
- e2e `m11.ps1 -Only e2e -Spec m12.plane,m12.mapview,m12.viewport` (try 3): 12 of 12 ids, 0 files left. Try 1 and 2
  failed `w5-range` on a real fault: the range fields remounted on every commit, so typing the second end was lost
  (fixed), then a WebDriver clear that never reached React's state (the spec now types as a person does).
- Measured: the box's new plane holds 120 faces (2 x 6 x 10 cells), three texels equal the file; GPU node means equal
  upstream's rule from the file to 1e-6 relative; the probe's bits equal the file's (face 4, 52.2 dB).
- Elmia (not run): its largest node links 17 faces, under the 64 the smooth average takes.

## Left

- W1: no drag gizmo for the three corners in the 3D view (height and cell size are fields); a tilted plane is shown,
  not edited; no on/off per plane. On Elmia an ear plane at 1 m cells adds on the order of 3,000 faces to a
  cutting-plane map of 10,000 steps (texture grows with faces x steps, P3's table); not measured there.
- W5: the fixed range applies to level maps only (a difference keeps its symmetric range); the probe follows the
  pointer (no pinned probes); contours only on smooth colour; palette choice (R49) not built.
- Product choice for Burhan: the new plane's cell size defaults to 1 m, not upstream's 0.5 m (reason above).
- Not run here: the full m10/m11 e2e regressions and `m12.ps1`.
