# C3 report: the m12 gate back to green (2026-10-07 15:08)

Written by the main thread from the C3 builder's final report (the harness refused the builder's own write of
this file). Branch `m12fix`: ad30781 (layout), af605dc (response window + re-pins), a451d30 (w9-png spec).

## Result

**m12 40/40 and m11.dock 5/5 (45/45, verdict PASS, focus PASS)**, gate run
`C:\tmp\nm-target-dock\gates\m11\20261007-150805`, at a451d30, 1 min 46 s (the dock builder's run took 1 min 43 s).
The run before it on the same tree (`20261007-150046`) had all five targets green but `m12.sources` timed out once
in its before-all, waiting for the Acoustics tab to be ready; it did not recur. UI: typecheck clean, 341/341.

## The five failures

| id | what it was | what was done |
|---|---|---|
| resp-crop | stale pin. 7e44f63 (the response window rebuilt on Burhan's 10-06 18:20 order) always offers "Full run", because it also undoes a zoom | re-pinned with the reason: with nothing cut, "Full run" is pressed and "Fit energy" is disabled |
| resp-numbers | the bin chips' widths come from the report's time step but were not marked as numbers | the chips are now `[data-num]` of `spps.time_step_s` and checked; span chips are `data-label="control"`; selects are exempted the way the tab's own scan exempts them, and their options are checked against the run's JSON (with a control); the readout off the map must hold no digit. No digit is hidden |
| w5-probe | REAL layout defect: the overlay's cards grew with the gfx rows and pushed the map probe's title under the view bar | fixed in `viewport.css`: the legend sits beside the timeline, right of the plan inset, and never grows wider than its row; the map panel's foot is at 214 px. At 1440x900 the probe title moved from y 86 to y 167, no overlaps |
| w9-layout | REAL layout defect: the legend card overlapped the tools column at 1440x900 | the same `viewport.css` change |
| w9-png | the TEST was wrong, not the export. The spec composited against a hard-coded `[9,9,11]`; ff820a9 changed `--bg` to `#0c0a0a` (at most 3 levels apart). Right after a step change two frame reads differ in 9,082 points, so the spec read an unsettled frame | spec now uses the page's `--bg` and waits for the frame to settle. Receipt (`.out\m12fix\w9png-bed2\`): the PNG equals the GPU frame over `--bg` at 0 of 36,000 points (the spec's grid: 0 of 210). Against a screenshot the PNG differs by at most 6 levels: the 7 % red tint behind the canvas, which the export leaves out by design |

## Still open

- With particles + Glow + every note shown at once (no spec covers it), the overlay is 38 px too tall and the
  probe title is cut.
- At 1024x640 the shell was already broken everywhere; this change neither helps nor worsens it.

## Screens

`.out\m12fix\w5-probe.png`, `w9-layout.png`, `w9-export.png`, `ir-window.png`; before/after beds in
`.out\m12fix\layout-before\`, `layout-after2\`, `layout-particles\`.
