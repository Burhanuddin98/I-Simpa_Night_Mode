# The GPU version: a GPU particle solver and a GPU graphics layer (arc opened 2026-10-06 19:24)

Burhan, 19:24, verbatim: "LETS BUILD THAT VERSION, AND ALSO SOMETHING THAT ALLOWS FOR COMPETENT GRaphics and
animations so we can have awesome ray tracing on the screen and other plots and other things".

This overrides decision 51's order (the GPU tracer after M12 and M13): decision 68. It keeps decision 51's road
for the solver (one walk, a counter-based RNG keyed by seed, band, source and particle, the same code on CPU
and GPU, so both arms trace identical particles) and v1's rule that a physics claim carries a bed.

What is measured going in (`experiments/gpu-probe/RESULT.md`, REALISED on Grace 19:17-19:20): a CUDA shoebox
tracer, 10 M particles x 2 s at 1 ms, kernel 0.162 s; the same code on all 28 CPU threads 2.193 s (13.5x);
one thread 27.8 s (171x); SPPS one thread 185.3 s. T20 against SPPS 0.999 and 1.000. WebGPU in the app's
WebView2 (Chromium 154) on Grace: adapter `nvidia / blackwell`, device created, 2 GiB storage buffers,
workgroups of 1,024, `subgroups`, `timestamp-query`, `shader-f16` (19:26). three.js is 0.186.1, which
ships `WebGPURenderer` and compute through TSL.

## Track A: the GPU solver, `spps-gpu.exe`

**A drop-in executable, not a library inside the app.** It reads what SPPS reads (`config.xml`,
`tetramesh.mbin`, `mesh.cbin`) and writes what SPPS writes (`.recp` per receiver, the `.gabe` tables, the
sparse `.csbin` maps of patch 0002, the stats), so the run manager, the verdict, `results::load`, the beds
and the app's Results step take it unchanged. The run's manifest names the executable and its hash, as it
does SPPS's; `solvers/manifest.json` gains its row. CUDA (proven tonight, and Zeph's RTX 2060 is NVIDIA);
the walk is plain C++ compiled for host and device, so a CPU build of the same walk exists from the first
day (the fallback and the bed's second arm), and a later port to a portable backend (V2-11) has one
function to move.

| step | what | done when |
|---|---|---|
| A1 | **The mesh walk, timed.** The probe's kernel walking CR4's tetrahedral mesh (11,150 tetrahedra, `tetramesh.mbin`: four faces each, neighbour across, `marker` the scene face), specular, one absorption; GPU vs the same walk on all CPU threads vs SPPS. | The 30x-190x forecast replaced by a measured number on CR4. |
| A2 | **SPPS's loop, reproduced.** From the recon map (`sonar-spps-loop.md`, receipts into upstream): source sampling and directivity, the step length, reflection laws (specular, diffuse, the rest), energetic and random methods, scattering, transmission, air absorption, extinction, fittings. | Each rule has its receipt into upstream and its unit check on CPU. |
| A3 | **SPPS's outputs.** Point receivers (`.recp`, per source), the statistics, surface receivers and cutting planes into the sparse `.csbin` (patch 0002's format), the time bin of patch 0001. | `simpa results` reads a `spps-gpu` run as it reads an SPPS run: same verdict, same report fields. |
| A4 | **The bed.** The parity tutorials and the M8 bed (the Kuttruff reference, the error bands), CPU walk vs GPU walk bit for bit at equal seed, GPU vs SPPS within the Monte-Carlo noise. | `BED-GPU.md`: every parameter in band; the speed-up realised on Grace and on Zeph. |
| A5 | **In the app.** The Simulate step's solver choice gains "SPPS on the GPU" when a CUDA device is found (the app asks `spps-gpu --probe`), with the device named; refused, with the reason, when not. | The app runs CR4 on the GPU and shows the results; the run says which solver made it. |

## Track B: the GPU graphics layer

**WebGPU in the existing app, through three.js's `WebGPURenderer`.** One renderer and one GPU context for
the 3D view (decision 53's rule, "one canvas means one WebGL context", becomes one GPU context: decision
68 (b)), WebGL2 as its fallback where WebGPU is absent (three.js falls back by itself). Compute shaders
animate what the CPU cannot: millions of particles, their trails, the rays.

| step | what | done when |
|---|---|---|
| B1 | **The renderer swap.** The viewport on `WebGPURenderer` with the WebGL2 fallback; the materials, the map shader (`resultsLayer.ts`) and the picking ported to TSL. | Every m10/m11/m12 pixel pin passes on both backends, or is re-pinned with its reason. |
| B2 | **Ray tracing on screen.** The saved particles (`.pbin`) animated by a compute shader: instanced points with additive glow, trails as fading line strips, colour by energy in the map ramp, speeds from real time down to 0.001x; a ray mode drawing each particle's reflection path in full. | A million particles at 60 fps on Grace and at least 30 on Zeph, measured (`timestamp-query`); the demo-video framing of memory (`demo-video-simple-and-framed`). |
| B3 | **Live from the solver.** `spps-gpu` streams a sample of particle positions per step to the app while it runs, so the room fills with sound on screen as it is computed. | The Simulate step shows the running solve; the cost to the solve is measured. |
| B4 | **Plots on the GPU where it pays.** The response map, the surface maps' time scrub and the large echograms drawn as GPU textures (the response window is a PNG today); uPlot stays for small charts. | The CR4-27 plane scrubs at 60 fps; the response window zooms with no re-encode. |

## Order

A1 and B1 first, side by side (A1 decides whether the solver is weeks or months; B1 is what every visual
needs). Then A2 to A4 (the solver, bedded), B2 against the saved particles meanwhile, A5 and B3 last,
where the two tracks meet.

## What does not change

Upstream's SPPS stays the reference and the default until A4 passes; a GPU run never stands in for an SPPS
run in a bed. Nothing is claimed about speed or agreement without its receipt and its label.
