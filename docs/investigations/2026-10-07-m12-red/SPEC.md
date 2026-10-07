# C3: the m12 results gate, red since the 10-06 merges (spec, 2026-10-07 14:41)

The dock builder ran the m12 specs at 14:35 (`C:\tmp\nm-target-dock\gates\m11\20261007-141116\wdio.log`,
screens beside it) and 5 of 40 checks failed on `gpu`, the same with its own change reverted. No m12 gate pass is
recorded after 10-05 22:40 (decision 57). A sentinel read each one at 14:40 (cause commits from TRACK-B.md and
commit dates, not all from a code diff):

| id | message (verbatim) | spec | likely cause | class |
|---|---|---|---|---|
| resp-crop | a "Full run" toggle with nothing cut: 'true' !== null | m12.response.e2e.ts:306 | 7e44f63, the response window rebuilt on Burhan's 10-06 18:20 order, renders `data-action="response-full"` always | stale pin: re-pin with the reason |
| resp-numbers | a digit outside the marked numbers: "nd (SPPS echogram) · ReceiverR1R2R3×The energy SPPS counted " | m12.response.e2e.ts:356 | 7e44f63, the receiver picker's R1/R2/R3 chips | probably stale: mark the picker labels the way the spec's number rule expects, or narrow the rule with the reason; never hide a real stray number |
| w5-probe | The input did not match /^Cutting planes · 1 kHz · /. Input: '' | m12.mapview.e2e.ts:257 | 5b3ab0b/a945290: the overlay's cards grew (Particles/Glow rows) and the probe's title is scrolled off under the view bar (`screens\w9-layout.png`); `.vp-dock` viewport.css:410 has no max-height | REAL layout defect |
| w9-png | Expected values to be strictly equal: 170 !== 0 (170 of 210 sampled pixels differ from the frame) | m12.export.e2e.ts:278 | 61f9e24: `wowFrameSamples` is an async WebGPU read-back, premultiplied and BGRA (ResultsOverlay.tsx:121); the spec composites `c + bg*(255-a)/255` | UNDECIDED: decide with a receipt whether the exported PNG matches what is on screen. If the export is wrong, that is a wrong output reaching a user: fix the export. If only the test's read-back or compositing is wrong, fix the test with the receipt. |
| w9-layout | Expected values to be strictly deep-equal: + [ 'legend and tools' ] - [] | m12.export.e2e.ts:355 | a945290/5b3ab0b: the Present and Turntable buttons lengthen the tools column; the legend card overlaps it at 1440x900 (x≈1034 vs 1027-1052) | REAL layout defect |

Burhan's order for the day, 08:57: "okay proceed with da work"; nothing reaches a push while its gate is red.

## Done when

All 40 m12 checks pass on this branch, run with `tools/gates/m11.ps1 -Only e2e -Spec m12.viewport,acoustics,
bedplant,sources,response,plane,mapview,fill,trails,export,mapwindow` (the dock builder's list) and the dock spec;
each re-pin carries its reason in the spec's comment and the commit; the two layout defects fixed in the app (at
1440x900 and at the minimum window size the specs use) without moving the m12 hook fields or `data-part` names;
w9-png decided with a receipt (the exported PNG's pixels against the on-screen frame, both read the same way) and
fixed on the side that is wrong. Screenshots of the probe and the legend after, in `.out\m12fix\`.

## Constraints

- Worktree `B:\repos\I-Simpa_Night_Mode\.claude\worktrees\m12fix` (branch `m12fix`, from `gpu` 7944ed7, which has
  the dock merged). Build with `npx --no-install tauri build --no-bundle` in `app/` and
  `CARGO_TARGET_DIR=C:/tmp/nm-target-dock` (REUSE that directory: its release build is of almost this tree, and C:
  is short, about 20 GB). Pass `-TargetDir C:\tmp\nm-target-dock` to the gate runner. No `cargo test`. Outputs only
  to `B:\repos\I-Simpa_Night_Mode\.out\m12fix\` and that target dir. Solvers `C:\tmp\nm-solvers-gpu`.
- Run only the m12 specs and the dock spec, never the full m10/m11 gates. The windows they open are unfocused.
- Another builder (C1, branch `blank`) is editing the outliner, materials grid, sources panel, ops and core; you own
  the results overlay, the response window, the export path and the m12 specs. Do not edit their files.
- Your app's DevTools port: 9271; check that the pid you drive is the one you started.
- Commit on `m12fix` as you go, messages on the why, no Claude attribution, no Co-Authored-By. Do not push.
  `npm run typecheck` and `npm test` pass at the end.

## Report

`docs/investigations/2026-10-07-m12-red/REPORT.md` on `m12fix`, committed: the five rows with what each turned out
to be and its fix or re-pin (with the receipt for w9-png), the gate result (n/40), screenshots, anything else found.
