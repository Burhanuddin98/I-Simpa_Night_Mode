# Wow list W2, W3 and W9: sound filling the hall, particle trails, export (2026-10-04)

Burhan 11:45: "IT WOULD BE COOL TO HAVE SOME OF THESE OTHER COOL FEATURES". Definitions:
`docs/investigations/2026-10-03-wow/WOW-LIST.md` W2 (parity R40), W3 (R54), W9 (R56, R63). Branch `wow-w2w3w9`
off `wow-w1w5`, worktree `.claude/worktrees/wow-w2w3w9`, cargo target `C:\tmp\nm-target-wow2`. Precedent and
gates: `docs/investigations/2026-10-04-wow-w1w5/PLAN.md`. Receipts and screenshots: `B:\data\m12\wow\`.

## What is already there

- The map texture is faces x steps float32, face-major (`mapData.ts`), one per map; a step is one uniform.
- Particles: every `.pbin` record on the GPU once (position, step, energy); `kept()` in `particles.ts` is
  the one place a record is dropped, and gate (d)'s count draws the same material in a count mode.
- Upstream's cumulative map (`Recepteurs_surfacique.cpp:337-387`, menu "Cumulative instantaneous sound level",
  `e_report_recepteurssvisualisation.h:173`): per face, each record's energy added into its step in float32
  (a non-finite record read as 0), then `energy[t] += energy[t-1]` over the steps, then dB.
- Upstream's rays (`Particules.cpp:366-386`, "Rays"): per live particle, one line from its position at the
  step before to its position at the step.
- No file-writing command: the app has the dialog plugin (`dialog:allow-save`) and no fs plugin.

## W2: sound filling the hall (R40), the smaller slice

1. `features/viewport/cumulative.ts` (pure): the cumulative texture by upstream's rule in float32 (a
   `Float32Array` store rounds each add to float32; double rounding of a float sum through a double is
   innocuous, so this is C's `float +=`), one face's cumulative value for the probe by the same arithmetic,
   and its level range.
2. Map panel: a switch "Cumulative (sound building up)". On, the map's texture is the cumulative one, the
   legend says "cumulative from 0 ms", the probe says the number is the face's energy summed from the first
   step to this one. It builds and holds; the decay is the instantaneous map (the default), and the switch's
   hint says so: the wow list's "then fades" is the instantaneous map, not this one.
3. Off for a difference map (the switch says why). Smooth colour and the fixed range work on it.

## W3: particle trails (R54)

1. `particles.ts` (pure): the trail segments, one per pair of consecutive records of a particle, each tagged
   with its head's step, the particle's last step and the head's energy; the count a step keeps by the rule.
2. The trail shader keeps a segment only when its particle is alive at the step (as the dot is) and its head
   is one of the last N steps (N = 1, upstream's ray, 5, 20 or 60); older is fainter. A segment is a straight
   line between two saved positions: a bounce inside a step is cut as a chord; the hint says so.
3. A GPU budget: trails take 48 bytes a segment; above 256 MB the switch is refused with the size.
4. Count hook like gate (d): the trail material in its count mode, two points a kept segment.

## W9: export (R56, R63, the smallest complete slice)

1. Core: one command `export_write` (raw bytes, the path and kind in headers). It writes only an absolute
   path with the kind's extension (`.csv`, `.json`, `.png`), whole or not at all (a temporary file renamed).
   The path comes from the save dialog; the e2e passes one the way the save-as hook does.
2. `features/export/exportModel.ts` (pure): the Acoustics tab's parameters as rows, through the tab's own
   `cell()` (PASS parameters only, a value only with its range and status, or its refusal), every receiver,
   band, the bands summed and each source; each row carries the report path it was read from. CSV and JSON
   write the report's double as JavaScript prints it (shortest round-trip), so the number in the file is the
   report's number, not the tab's rounding. Marks and the MQ2 wording travel in JSON and in a CSV column.
3. PNG of the view: the frame as drawn (read back after a render), over the window's background, with a
   strip below it holding the shown map's legend (title, gradient, ends, step time) when a map is shown.
   The strip draws the legend's own text; the hook reports what it drew.
4. File menu: Export view as PNG..., Export parameters as CSV..., Export parameters as JSON..., disabled with
   the reason where there is nothing to export (no run, results refused, off the Results step).

Left on purpose: animation video (R56's second half), chart images (R63), a report page (W12).

## The overlap (W5's screenshot)

The legend, the "No particles saved" card and the probe overlapped at the bottom of the view. They move into
one bottom dock (`.vp-dock`) between the plan inset and the axis gizmo: the cards are in flow, so none can
cover another; the probe docks there too instead of following the pointer (its "Face n" line names the
face). e2e `w9-layout` reads the rectangles of every card and of the map
panel and the plan inset and asserts no two overlap, with the probe shown.

## Tests first

- UI (`node --test`): `cumulative.test.ts`, trail additions to `particles.test.ts`, `exportModel.test.ts`;
  each with a say-NO case.
- Core: `export_write` unit tests in `commands.rs`'s module (a wrong extension, a relative path: nothing
  written; a good path written whole).
- e2e `m12.fill` (W2): `w2-fill` (cumulative texels from the GPU equal the spec's own float32 running sum
  from the `.csbin` bit for bit; the frame changes; the legend says cumulative), `w2-refuse` (no cumulative on
  a difference), `w2-probe`. Screenshot `w2-fill.png`.
- e2e `m12.trails` (W3): `w3-count` (segments drawn at 5 steps equal the rule's count from the `.pbin`),
  `w3-refuse` (no trails without particles; the switch says why). Screenshot `w3-trails.png`.
- e2e `m12.export` (W9): `w9-csv`, `w9-json` (every number equals `simpa results --json` at its path; a
  planted wrong digit is caught), `w9-png` (decodes; the frame's sampled pixels; the strip's text is the
  legend's), `w9-refuse` (a `.exe` path refused by the core, nothing written), `w9-layout`. Screenshot
  `w9-export.png`.
- Registered in `tools/gates/m11.ps1`'s spec table; run with `m12.plane`, `m12.mapview`, `m12.viewport`,
  `m12.response`.

## Result (13:24)

Built as planned, on `wow-w2w3w9` (not pushed, not merged). Receipts: `B:\data\m12\wow\` (`red-w3-ui.txt`,
`red-w3-window.txt`, `red-w9-ui.txt`, `red-w9-snapshot.txt`, `cargo-app-{1,2}.log`, `e2e-w2w3w9-try{1,2,3,4}.log`);
screenshots `shots\w2-fill.png`, `w3-trails.png`, `w9-layout.png`, `w9-export.png` (the exported PNG itself).

- UI suite 235 of 235 (new: `cumulative.test.ts` 4, trails 5 in `particles.test.ts`, `exportModel.test.ts` 6,
  `snapshot.test.ts` 3, each with a say-NO case); typecheck clean; e2e reader test 4 of 4; `cargo test -p app --bins`
  74 of 74 (new: `export.rs` 4: a good path written whole, a wrong extension / relative path / missing folder /
  folder path writes nothing, bytes not of the kind refused, the path header decoded).
- e2e try 4 (`-Spec m12.fill,m12.trails,m12.export,m12.plane,m12.mapview,m12.viewport,m12.response`): 25 of 25 ids.
  Try 1 failed on two real faults and one wrong assumption: the trail window kept N + 1 heads (12 drawn where the
  `.pbin` rule says 10 at step 11; fixed, and the shader's `keptTrail()` is now run as JavaScript in a unit test);
  the box leaves 50 of 56 parameter rows refused, so a floor of 20 numbers was wrong (both kinds are checked now).
  Try 2's screenshots showed the cumulative note pushing the map panel over the plan inset and 10 px into the dock;
  try 3 bounds the panel (left of the dock, above the inset, scrolling) and w9-layout checks it at its tallest.
- Measured: cumulative texels equal the file's float32 running sums bit for bit (3 cells, e.g. face 0 step 2
  0x35ba5858, 61.43 dB); the probe's bits equal the sum (face 1 step 30, 62.2 dB); trails 5 and 1 steps at 5 steps
  equal the rule (27, 50, 10, 5, 0 and 27, 10, 2, 1, 0; 135 segments, 6,480 B); CSV and JSON: 56 rows, 18 numbers,
  50 refusals, 0 mismatches against `simpa results --json` by path; PNG 1440 x 964 (the frame and the 64 px strip),
  210 sampled pixels equal the frame over the background, 0 differ; layout: 8 cards, 0 overlaps.

## Left

- W2: no "decay after the fill" view of its own (the instantaneous map is that); a difference is never cumulative.
- W3: trails are straight chords between saved steps (said on screen); no per-trail colour by age beyond fading;
  the budget (256 MB) is not measured on Elmia.
- W9: animation video (R56's second half) and chart images (R63) are not built; the PNG is the canvas only (side
  panels are not in it, by design), with the legend redrawn in a strip; no PNG of the plan inset alone.
- Layout: the dock is right of the map panel's widest; on a narrow window it wraps upward and covers more of the
  view's centre (no overlap, measured at 1440 x 900 only).
- Not run here: the full m10/m11 e2e regressions, `m12.ps1`, and m11's static half (its command inventory now
  expects 45, with `export_write`).
