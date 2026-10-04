# 2026-10-04 07:56-09:15: Elmia's reverberation times (backlog 78), ranges below zero, per-source results (77)

Branch `b78-elmia` off `rebuild` @ `1cef197`. Measurements in `B:\data\m12\b78-mesh\FINDINGS.md`.

## Backlog 78: why the Elmia hall showed no T20/T30

- Not the decay's depth (each band fell 62-76 dB before the series ended) and not the -70 dB floor.
- The refusal was `missing_moves`: SPPS lost 119-171 particles per band to "meshing problems"
  (0.08-0.11 %), and the lost-share bound (cap `ENERGETIC_LOST_ENERGY_RATIO` 10) exceeded the 0.5 % limit.
- The surface mesh is clean (`B:\data\m12\mesh-audit-2026-10-04\FINDINGS.md`: no self-intersection,
  open or non-manifold edge, duplicate or degenerate face). The cause is tutorial 2's meshing, `-q2`
  without `-Y`: TetGen split the boundary (25,554 segment points). With `-Y` the loss is 28-44 per band;
  `-q` does not matter.
- Then T30 showed, but noise-dominated at 0.31 m receivers and 150,000 particles: median sd 34 % of the
  value, 9 of 36 ranges below zero (R03 1 kHz 1.96 s, -0.99 to 4.91 s). 10x particles: 3.7 %; 1.0 m: 2.3 %.
  The noise fell 9x and 15x where the square-root rule forecast 3.2x.
- 1.0 m receivers crossed the walls at R01-R04 (`receiver_sphere_crosses_surface`, level reads low): the
  fixture has 0.6 m, below R03's 0.647 m. Tutorial 2's 1.5 s, 5 ms, trans_epsilon 5 refused every
  per-source T30 (truncated); the fixture takes the new-project 10 s, 1 ms, 7.
- Result (run H, fixture as committed, 3 sources, 1,000,000 particles each, no warnings): per source over
  108 values, T30 108 (70 ok, 38 wide, sd median 1.2 %), EDT, C80, D50, Ts 108, T20 72. R01 1 kHz T30
  1.96-1.99 s; Sabine 2.07, Eyring 1.88. No exact solution for the hall, so no bed.

## Decisions (decision log 48, 49), on Burhan's "PROCEED" (08:30)

- 48: EDT, T20, T30, D50 and Ts whose range reaches below zero are refused `range_below_zero`; results
  version 13. SPL, C50, C80 keep any range.
- 49: the Elmia fixture's settings (above), each asserted against tutorial 2 in `elmia_corrected()`;
  then a run-quality advisor (backlog 80).

## Backlog 77: per-source results

A Source picker on the Acoustics tab (several sources only, opening on the first in the project's order;
`config.xml` lists them last first); the RT chart and table, receivers table and decay follow it, through
the same gate (a)/(b) filters. Several sources with no echogram per source say how to get one. e2e
`m12.sources` (b77-picker, b77-numbers) passed on the M11 harness, 09:10 run.

## Tests

Full workspace suite 08:51-08:57: 1,112 passed, 0 failed, 40 ignored. UI 189/189. e2e `m12.sources` PASS.

## New backlog

80 run-quality advisor (V1) · 81 re-score the beds under version 13 · 82 Elmia's remaining T20
`missing_moves` (the cap of 10) · 83 the default receiver radius, within the walls · 84 EDT is not checked
against lost-particle energy.

## Traps paid for

- A `git stash` while the suite ran swapped the tree under it: the suite was stopped and rerun. Never
  stash or switch while a build or test reads the tree.
- "verdict OK" is not "no warnings": runs E and G carried four sphere-crossing warnings, unread when G was
  reported. Read `verdict.warnings` of every run quoted.
- `app/node_modules` lacked `three` and `uplot` (typecheck errors not ours); `npm ci` fixed it.
