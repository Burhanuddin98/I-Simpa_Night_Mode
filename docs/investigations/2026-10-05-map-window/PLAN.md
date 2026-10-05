# The map's time window (2026-10-05)

Burhan 01:59 ("cool"): the sound maps look patchy because a step's value per triangle comes from the few
particles that crossed it in that 1 ms (CR4 1 kHz plane, 300k particles, 3,588 triangles: 88 % blank at
20 ms, 12 % at 100 ms, 3-4 % at 200-800 ms), and a square is two triangles, so you see half-squares.
Branch `map-window` off `playback`, worktree `.claude/worktrees/map-window`, not pushed.

## The rule (`app/ui/src/features/viewport/window.ts`)

At step t with a window of w steps, a face's value is the mean of the file's own per-step values over
steps max(0, t - w + 1) .. t (a step with no record counts as 0, and so does a non-finite record), added in
float32 in step order and divided by the number of steps the window holds. The window trails the step, so
nothing shows before the file has it. w = 1 is the raw step, bit for bit. A window that holds no energy draws
nothing. Choices: Off, 5, 10, 20, 50 ms, with 10 ms as the default. Each choice becomes whole steps of the
map's own time step. A choice that comes to one step (5 or 10 ms on a 10 ms run) or to more than 64 steps
is disabled, and its title says why.

- GPU: `faceValue` in `resultsLayer.ts`. The draw, the difference, the smooth node mean and the read-back
  all go through it. Changing the window changes one uniform (`uWin`) and uploads nothing.
- On screen: the legend's title says "averaged over 10 ms" (so the PNG export says it too). The probe shows
  the window mean and says so: "mean of the file's values over 10 ms (steps a–b, k of 10 with energy)".
- With cumulative (W2): refused. The cumulative map already sums every step from 0 ms; the chips are
  disabled and the panel says why. Cumulative off, the window comes back.
- With the difference: both runs are averaged over the same window, then the levels are subtracted.
- With smooth colour: the node mean of the faces' window means.
- Legend range: the map's own range from the records. A mean is never above the largest record it holds; a
  mean below the floor is drawn at the floor's colour.

## Receipts

- UI `window.test.ts`, 6 tests: a known sparse series, window 1 = the raw bits, NaN / empty / past-the-end
  say-NO cases, choices and refusals, label, probe (level, difference, smooth). Suite 263 of 263, typecheck clean.
- e2e `m12.mapwindow` (box at 1 ms, 500 steps, two runs): `mw-default`, `mw-off`, `mw-probe`,
  `mw-cumulative`, `mw-diff`. GPU drawn values match this spec's own window mean from the `.csbin` to within
  1e-6 relative (3 of 4 bit for bit). At step 9, 14 of 28 faces are blank raw and 10 under 10 ms, the rule's
  count. Off gives the raw bits. The 50 ms mean differs from the 10 ms one past step 60. The probe's bits equal
  the file's mean. Cumulative texels are the running sums. Difference dB match the files' window means to 1e-3.
  With `m12.viewport, mapview, fill, plane, export`: 26 of 26.
- Pictures: `B:\data\m12\map-window\cr4-before.png` and `cr4-after.png`, CR4 1 kHz plane at 300 ms, Off vs
  10 ms (tour harness, `TOUR_WINDOW` mode).
