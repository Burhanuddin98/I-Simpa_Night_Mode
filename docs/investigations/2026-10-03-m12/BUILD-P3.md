# M12 P3 build: the viewport, surface maps and particle playback (2026-10-03)

Branch `m12-viewport` off `m12` @ `b9a60cd`, worktree `.claude/worktrees/m12-viewport`. Spec: `PLAN.md` beside
this file, P3, gate (c) and (d), MQ4 at its default. Receipts (red and green logs, every e2e try, the progress
log): `B:\data\m12\p3\`.

## What it means

**On the Results step the 3D view now draws the run's surface map and plays its particles, and both move on one
timeline.** The map is the solver's own `.csbin` values, coloured black through red to yellow in dB (upstream's
`10 log10(E / 1e-12)`), per band, with a legend, a step slider and Play, and a switch that shows this run minus a
baseline run in dB (the design's cool ramp where it is quieter, the hot ramp where it is louder). A run that
saved no particles says "No particles saved for this run", where to turn it on, and what it costs on disk for
that run (MQ4).

Gate (c) and (d) pass on a real run made from the app: the GPU's texture holds the file's float32 values bit
for bit, and the particles the playback draws at a step are exactly the particles alive in the `.pbin` there.

**What Burhan may want to choose (nothing blocks P2 or P4):**
- The map controls are a panel over the 3D view (top left), not the design's right-hand panel (design
  :436-450), because that panel is P2's `ResultsPanel`. P4 or a later pass can move them; the controls are one
  component (`ResultsOverlay.tsx`).
- The design's map selector names parameter maps (T30, EDT, C80, D50, STI: parity R42). P3's list is the
  level map per step (R38, R39); parameter maps are not built. Only the solver's own maps are offered: surface
  receivers and cutting planes, per band and "All" (the solver's `Global` file).
- The baseline of the difference is the newest earlier run of the project (`resultsView.setBaseline` exists,
  no control for it yet). A baseline whose map is not the same surface (nodes, faces, steps) is refused by name.

## Commits

| Commit | What |
|---|---|
| `f1a4034` | P3.1: `mapData.ts` (faces x steps layout, dense float32 by bits, levels, legend range, difference range, baseline check, the design's ramps), `particles.ts` (record steps, alive counts, MQ4 text), `animator.ts` (the one timeline) |
| `31056f0` | P3.2: `app/e2e/specs/m12.viewport.e2e.ts`, `app/e2e/lib/m12files.ts` (the spec's own `.csbin` and `.pbin` readers) and its test; `m11.conf.ts` and `m11.ps1` take a dotted spec name as its own file; `m12.ps1` runs it |
| `87d4f9b` | P3.3: `resultsLayer.ts` (the GPU side), `resultsView.ts` (the loads), `ResultsOverlay.tsx` (controls, legend, transport, MQ4, the m12 hooks), the engine's layer, `viewport.css` |

## Tests, red then green

| Test | Red (receipt in `B:\data\m12\p3\`) | Green |
|---|---|---|
| UI `mapData.test.ts` (8), `particles.test.ts` (3): layout and wrap, every record's bits (a signalling NaN payload too), levels, the 60 dB legend range, the difference range, the baseline check, the ramps, the labels; alive at first step + k; MQ4's size by upstream's formula; the timeline's carry and stop | `red-p3.1-ui.txt`: module not found | `green-p3.1-ui.txt`: 11 passed; UI suite 170 of 170 |
| harness `m12files.test.ts` (4): the fixtures read to their last byte, a byte short or long refused | (helper of the spec; passed when written) | 4 passed; harness 49 of 49 |
| e2e `m12.viewport.e2e.ts`: `m12-c`, `m12-d`, `m12-mq4`, `m12-p3-maps` | `red-p3.2-e2e.log`: before-all fails, the m12 hooks not registered, 4 ids not passed | `try-p3.3-e2e-4.log`: 4 of 4 |

Between red and green, three tries failed on real faults, each fixed: `try-p3.3-e2e-1` (three's `vec3[]`
uniforms take a flat array: the map and particle shaders could not draw), `-2` (the spec read React's
`data-results-region="true"` as wrong; it is presence that counts), `-3` (the difference map was shown without
its baseline texture). After `-3` a pixel check was added: hiding the map changes 38,486 of 461,070 pixels.

## The gate run and its receipts (`try-p3.3-e2e-4.log`)

`tests/fixtures/rooms/outputs_box.simpa` (10 x 6 x 3 m box, floor surface receiver, cutting plane, 2,000
particles, 1 s in 10 ms steps, 500 Hz and 1 kHz) copied to C: and run twice from the app: run A particles saved
0, seed 2 (baseline, MQ4); run B particles saved 40, seed 1.

- **(c)** `Surface receiver/1000 Hz/Sound level.csbin`, read back through the map shader's own `mapTexel`:
  face 0 step 0 `0x3588659e`, face 13 step 5 `0x3389df02`, face 27 step 17 `0x32713c50`, each equal to the file;
  a cell with no record reads 0; texture 2800 x 1 R32F, nearest.
- **(d)** steps 0, 11, 22, 23, 1: drawn 33, 2, 1, 0, 27 = alive in the `.pbin` 33, 2, 1, 0, 27; the map at
  the same step each time.
- **MQ4** run A: "No particles saved for this run" / "To play particles, set Simulate › Particles saved for
  playback above 0 and run again. 1,000 saved particles write about 1.6 MB a band for this run's steps and
  sources."
- **maps** legend 39 / 50 / 61 dB against the file's 39.778..60.755 dB; 500 Hz loads the 500 Hz file (a
  texel equal); difference at 3 cells equals the two files' levels within 1e-3 dB (-0.83240, -0.79182, 0);
  Play paused at step 6 with map and particles together.

How the counts are taken: the particle count is the particle material itself drawn in a count mode (each
record that passes the same step test adds 1 to one float32 pixel, additive blending, `EXT_float_blend`), so it
counts what the playback draw lets through, not what the CPU expects. The `.csbin` and `.pbin` are read by the
spec's own readers (`m12files.ts`), never by the app's IPC or decoders.

## GPU memory (the test machine has 12 GB)

Per map texel 4 bytes (float32), one band loaded at a time, twice with a baseline. Per particle record 20 bytes
(position 12, step 4, energy 4); the whole band's `.pbin` is resident.

| | Map texture | Particle buffer |
|---|---|---|
| Gate run (box, 28 faces x 100 steps; 168 records) | 11,200 B (22,400 B with the baseline) | 3,360 B |
| Elmia (`elmia_corrected.simpa`, its two cutting planes: 2,858 triangles by the cell formula) at new-project defaults, 10 s / 1 ms = 10,000 steps | 114.4 MB (16,384 x 1,745 texels), 228.7 MB with the baseline | 0: particles saved is 0 by default (decision 44 (2)) |
| Elmia, if 1,000 particles saved per source (3 sources) | as above | at most 30 M records = 600 MB (the `.pbin` is up to 480 MB a band) |

The texture wraps faces x steps into rows of the GPU's limit (`maxTextureSize`; 16,384 assumed above) and refuses
a map past one square texture by name rather than cut it. The JS side also holds the dense array (the texture's
bytes, kept by three for a context restore) and the SMAP bytes while a map is shown.

## Regression

`m11.ps1 -Only e2e -Spec gate,simulate,dock,smoke` on this build (`regress-m11-e2e.log`): 21 of 21 ids passed
(m11-h, m11-sim-numbers, m11-sim-link, m11-e-results among them), focus clean. Its one failed check was "files
left in the repository": this file, written while it ran. The full static half (lints, clippy) and the prior gates
were not run here; P4 runs them.

The final run of `m12.ps1 -SkipPrior` comes after this file is committed; its P3 checks are expected to pass and
its P2 ids to fail as NOT BUILT (it prints "M12 PARTIAL RUN ... FAILED" by design).

## Wiring

- `tools/gates/m12.ps1`: a P3 block runs `m11.ps1 -Only e2e -Spec m12.viewport` (its harness: build, drivers,
  private solvers, the C: projects, the e2e lock, the focus watcher) and checks `m12-c`, `m12-d`, `m12-mq4`,
  `m12-p3-maps` from its verdict lines; it removes `m12-c` and `m12-d` from the NOT BUILT list. Edits are
  insertions in their own block, so P2's entries merge apart.
- `app/e2e/m11.conf.ts` and `tools/gates/m11.ps1`: a dotted spec name is its own file (`m12.viewport` is
  `specs/m12.viewport.e2e.ts`); `m12.*` is not part of M11's own full run.

## Left open

- **Gate (a) and the legend.** The legend's numbers come from the `.csbin`, not from `simpa results --json`,
  so a gate (a) that reads every number in the Results regions would count them. They are checked by
  `m12-p3-maps` against the file instead. P4 must scope (a) to the report's values (`[data-result]` or P2's
  marker) or accept this check. The same holds for the step time (`120 ms`) and band labels.
- Map controls placement and the parameter maps (above); no baseline chooser.
- Every band switch re-reads and re-verifies the run through `run_surface_map` (P1 has no cache): fine on the
  gate run, slow on a large hall.
- A refused or unverified run draws no map (the overlay says which); P2's panel says why.
- `app/src-tauri/Cargo.toml` shows modified after a build (line endings only); not committed.
