# Track B: the GPU graphics layer, B1 and B2 (2026-10-06, 19:27-20:45)

Branch `gfx`. Commits: `61f9e24` (B1), `5cd2944` (merge of `gpu` for decision 69), `5b3ab0b` (B2). Not pushed.
Screenshots: `C:\tmp\nm-gfx-shots\`. Measured on Grace (RTX 5070, WebView2 / Chromium 154, 1440 x 900 page,
DPR 1, 60 Hz display), in a WebView2 profile of its own (`C:\tmp\nm-gfx-shots\wv2`).

## What it means

- **B1 done.** The 3D view runs on three.js's `WebGPURenderer`: WebGPU on this machine, WebGL2 where a WebView has
  no WebGPU. The picture is the old one: every read-back of the map is bit-identical to the WebGL build on both
  backends, and what differs on screen is anti-aliasing at 1-pixel line edges.
- **B2 done, with one limit.** The saved particles are animated by a compute pass and drawn as warm light (decision 69):
  **Glow** (particles, with trails that cool behind them when Trails is on), **Rays** (every path so far, sparks at the
  bounces), bloom and a filmic tone map, the room dimmed while it plays. A million particles hold 60 fps on Grace in every
  look. The limit: the million is the 400 saved CR4-27 paths replayed 2,500 times with a fixed offset per copy, because
  SPPS saves 400. Zeph's 30 fps target was not measured (no access from here).
- **Dots stays the default** (the measuring view, unchanged). Whether Glow should be the default is Burhan's call.

## B1: what was ported

| Was (WebGL) | Now (WebGPURenderer, TSL) | File |
|---|---|---|
| `WebGLRenderer`, sync start | `WebGPURenderer`, `await init()`; canvas made at once, view `live` while it starts so `idle()` waits for the first frame; `nm.forceWebGL` in localStorage forces the fallback for testing | engine.ts |
| colour converted to sRGB per built-in material, ShaderMaterials raw | output colour space linear (no framebuffer pass); each built-in material converts its own output (`srgbOutput`), so blending runs on the same sRGB values; map, ground, particles raw as before | nodes.ts |
| `onBeforeCompile` GLSL patches: corners (ao), glow, build-up, fade, glass | TSL: `aoFactor`, `glowTerm`, `buildKeep`/`buildLine`, `fadeAmount`, `glassOpacity`, applied through `outputNode` / `maskNode` / `opacityNode` | ao.ts, glow.ts, build.ts, fade.ts, glass.ts |
| ground ShaderMaterial | `groundColour` fragment node | ground.ts |
| map ShaderMaterial (texelFetch, window loop, node mean, levelDb, HOT/COOL, smooth, contours, floor(step/ratio)) | `mapNodes.ts`, the same operations in the same order; `isinf/isnan` (no WGSL equivalent) as an exponent-bit test; `fwidth` taken outside the branch (WGSL uniformity) | mapNodes.ts, resultsLayer.ts |
| texel / node / diff read-back passes, particle and trail count passes (float blending) | same passes as TSL point materials into float targets; counts need `float32-blendable` (present) or `EXT_float_blend` | resultsLayer.ts |
| sized `Points` (markers, particles) | instanced `Sprite`s with `PointsNodeMaterial` (WebGPU points are 1 px) | engine.ts, resultsLayer.ts |
| `LineMaterial` fat lines | `Line2NodeMaterial` subclass keeping ordinary alpha blending (three's transparent path mixes in an opaque frame copy and otherwise forced alpha 1) | nodes.ts |
| scissored inset clear | a quad replacing colour and depth inside the scissor (a WebGPU clear takes the whole canvas); viewport/scissor y now from the top | engine.ts |
| `gl.readPixels` hooks, sync | canvas texture copied to a buffer (WebGPU, padded rows unpacked, BGRA swapped) or `readPixels` (fallback); **async** | engine.ts, snapshot.ts |
| `R32F` internalFormat | dropped (WebGPU took it as its format name and failed the texture) | resultsLayer.ts |

The keep rules of the particle draw and its count pass are one function over numbers or nodes (`keptRecord`,
`keptTrail`), so `particles.test.ts` runs the rule the GPU runs. Tests: 325 pass (`npm test`), typecheck clean.

## B1: what was measured (pixel parity)

Same state script on the WebGL build (`C:\tmp\nm-target-gfx\before\app.exe`) and the new one: dock folded, reduced
motion, fixed cameras, CR4 run 6 and CR4-27 run `20261006-193146-930` (the 19:31 regeneration), 1 kHz cutting plane, step 60.

| Check | WebGL build | WebGPU | WebGL2 fallback |
|---|---|---|---|
| `m12Texels`, 7 cells (bits) | reference | identical (CR4, CR4-27) | identical |
| `m12DrawnTexels` (window) | reference | identical | identical |
| `wowNodeEnergies`, 3 nodes | reference | identical | identical |
| `m12MapPixels` changed, CR4 | 74,511 | 74,502 | 71,788 |
| `m12MapPixels` changed, CR4-27 | 77,090 | 77,086 | not run |
| `m12Particles.rendered` step 60 | 400 | 400 | 400 |
| `wowTrails.drawn` 20 steps / Ray | 8,000 / 400 | 8,000 / 400 | n/a |
| CR4 screen, pixels off by > 8 (> 32), of 1,296,000 | | model 44,413 (2,001); top 16,343 (764) | model 72,733; top 54,119 |
| CR4-27 screen, outside the timeline card, > 8 (> 32) | | model 29,596 (427); top 18,111 (639); smooth 17,833 (604) | |
| 120 x 40 px patch inside the map, exact | | 93 % (max 45) | 57 % (max 132) |

The differences are 1-px line edges and map-cell edges (MSAA resolves differently on D3D12 and ANGLE/D3D11); the
fallback has more because its map edges are not multisampled the same way. Side by side of the CR4-27 1 kHz plane:
`cr427-1k-plane-before-vs-after.png`; diffs: `diff-cr427-*.png`, `diff-b1-*-cr4-*.png`.

## B2: what was built

- `raysData.ts` (pure, tested): the GPU tables, `particleAt` (the interpolation rule), `bounceRecords`, `copyJitter`.
- `rays.ts`: storage buffers, the compute pass (state per particle), Glow heads (gaussian core + halo, size by level,
  depth-attenuated within 2.5-18 px, hashed shimmer), Glow trails (ribbons, length = the card's Trails choice, cooling
  head colour to crimson to black), Rays (all consecutive-record ribbons up to the step, colour by segment energy),
  sparks at bounces (4 steps, expanding, fading). WebGL2: Glow per record, no trails; Rays refused with the reason.
- `warmRamp.ts` (tested): decision 69's stops, OKLab interpolation, lightness strictly rising, 256-texel LUT, legend.
- engine.ts: the light pass into a half-float MSAA target with the room's depth, bloom (three's `BloomNode`, drawn into its
  own target first: inside the frame it wiped the canvas), ACES (black stays black), added onto the frame; the room dimmed
  while playing (`nmDim`); Glow off / soft / full.
- UI (timeline card): Particles Dots / Glow / Rays, Glow Off / Soft / Full, the warm ramp with dB labels re the loudest.
- Check: compute vs CPU `particleAt` on all 400 CR4-27 particles: exact at step 150, max 2.6 um and 2e-6 relative energy
  at 150.37, 400/400 agree on alive.

## B2: measured frame times (CR4-27, 1 kHz, step 150, default framing, 300 frames each)

GPU times per frame are timestamp queries per stage, with Chromium's 100 us timestamp quantization turned off
(`--disable-dawn-features=timestamp_quantization`, `C:\tmp\nm-gfx-shots\launch-bench.sh`); `nm.gpuTiming` enables
three's queries. "Batched" is 300 frames back to back, then `onSubmittedWorkDone`, divided by 300 (no display limit).

| Particles | Look | Display fps (rAF) | Batched ms/frame | GPU particle pass | Compute | Bloom + composite |
|---|---|---|---|---|---|---|
| 400 (saved) | glow, soft | 60.0 | 0.53 | 0.011 ms | 0.004 ms | 0.090 ms |
| 1,000,000 | glow, bloom off | 59.8 | 2.01 | 1.61 ms | 0.036 ms | 0.019 ms (composite only) |
| 1,000,000 | glow, soft | 60.0 | 2.31 | 1.86 ms | 0.036 ms | 0.086 ms |
| 1,000,000 | glow + 20-step trails (10 segments) | 41 | 23.3 | 23.5 ms | 0.03 ms | 0.108 ms |
| 1,000,000 | glow + 20-step trails (6 segments, shipped) | 59.6 | 12.3 | 12.2 ms | 0.03 ms | 0.097 ms |
| 1,000,000 | rays, soft | 59.8 | 3.14 | 2.78 ms | 0.034 ms | 0.095 ms |

Receipts: `C:\tmp\nm-gfx-shots\bench-b2.json`, `bench-b2-trails6.json`. The trail cost is mostly overdraw: the 2,500 copies
of each saved path sit within 0.6 m of each other. The first bench run measured nothing (the shared sprite quad had been
disposed, every frame failed validation); fixed before these numbers.

## Screenshots (looked at)

- B2, CR4-27: `final-b2-cr427-early-direct.png` (6 ms, the direct sound leaving LS2), `-particles.png` (120 ms, Glow, no
  trails), `-trails.png` (120 ms, 20-step trails), `-rays.png` (300 ms), `-late-diffuse.png` (900 ms, deep reds),
  `-hero-trails.png`, `-hero-rays.png` (close camera), `-1M-glow-trails-hero.png`, `-1M-rays-hero.png`,
  `-webgl2-glow.png`, `-webgl2-glow-hero.png` (fallback). Iterations: `b2-try*.png`.
- B1: `base-cr4-*` vs `b1-webgpu-cr4-*` / `b1-webgl2-cr4-*`; `before-cr427-*` vs `after-cr427-*`; `export-webgpu.png`
  (File > Export PNG through the new read-back).

## Pins and e2e expectations that change

- Hooks now return promises (the harness awaits every hook, `e2e/lib/hooks.ts`): `m12Texels`, `m12DiffTexels`,
  `m12DrawnTexels`, `wowNodeEnergies`, `m12MapPixels`, `wowFramePixels`, `wowFrameSamples`, `highlightPixels`,
  `m12Particles`, `wowTrails`. Any caller outside the harness must await them.
- `wowFramePixels` hashes are of top-row-first RGBA now: values differ from the WebGL build (they were only ever
  compared within a run).
- `m12MapPixels` counts move by about 0.01 % on WebGPU (74,511 to 74,502; 77,090 to 77,086); the fallback by 3.7 %.
- `highlightPixels` was only run on CR4 (no check faces: 0 changed); m10-viewport's MIN_WARN_PIXELS on a broken model
  is untested on WebGPU.
- The timeline card is taller by one row (Particles) while particles are shown, and by two more with Glow or Rays:
  W9's `wowCardRects` layout check and any screenshot pin of the card change.
- The warm legend shows −60 / −30 / 0 dB (not report values) inside a `[data-results-region]`; a gate that requires
  every number in a results region to come from the report would flag it.
- New hooks: `gpuBackend`, `gpuInfo`, `gpuParticles`, `gpuParticleState`, `gpuParticleCheck`, `gpuDimHold`, `gpuGlow`,
  `gpuBench`, `gpuBenchSet`. New localStorage keys: `nm.forceWebGL`, `nm.gpuTiming` (both test-only, off by default).
- The m10/m11/m12 e2e suites were not run (WebDriver on Burhan's live desktop); every number above is from the hooks
  over the DevTools port.

## Not done, and why

- Zeph (RTX 2060) not measured: no access from this session.
- A million distinct particles: SPPS saves 400 here; the benchmark replays them. A run saving more would test it for real.
- Per-material WebGL2 MSAA parity of the map edges (57 % exact patch on the fallback) not chased.
- B3 (live from the solver) and B4 (plots on the GPU) not started.
- Housekeeping: at 19:35-20:06 my first runs used the app's own WebView2 profile; I folded the dock and set
  `nm.forceWebGL` there, then restored it (`nm.viewStyle` as found, `nm-fold` dock open, `nm.forceWebGL` removed). All
  later runs used the separate profile.
