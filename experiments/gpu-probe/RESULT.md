# GPU probe: result on Grace (2026-10-06 19:17)

Burhan 19:10: "PROCEED WITH 3". The number decision 51 said we did not have. All numbers REALISED on
Grace (RTX 5070, 48 SMs, CUDA 13.2; SPPS the verified build of `solvers/manifest.json`), the seats box
(6 x 10 x 3 m, walls 20 %, floor 10 %, ceiling 30 % absorbing, specular), energetic, one band, no air
absorption, 1 ms steps, 2,000 steps (2 s), 10,000,000 particles, extinction at 10^-5. Raw: `.out/gpu-probe/`
(`gpu-10M.json`, `gpu-10M.csv`, `spps-10M.json`, the SPPS run folder, `RESULT-grace.md` from
`compare.py`).

| arm | wall time for 10 M particles x 2 s | |
|---|---|---|
| SPPS, one thread (a fixed seed turns its threads off; decision 51) | **185.3 s** | run.json `outcome.elapsed_ms` |
| this probe's code, one CPU thread | 27.8 s | 100 k particles in 0.278 s, scaled linearly |
| this probe's code, CUDA kernel | **0.162 s** | cudaEvent; 0.163 s with the copy back |
| the CUDA process end to end, CUDA start-up included | 0.327 s | |

**Speed-up, kernel over SPPS: 1,141x. It splits into two factors, and only one is the GPU:**

- **6.7x is the simpler problem**, not the GPU: the probe's walls are six analytic planes, SPPS walks a
  tetrahedral mesh (any geometry) and keeps its full accounting. A real tracer must walk a mesh too.
- **171x is the GPU** over the same code on one CPU thread. Against the same code on all of Grace's 20
  cores the GPU's lead would be about 171 / 14 = 12x (FORECAST: 14x assumed for 28 threads on 20 cores,
  not measured).

**Physics agrees** (a sanity check, not a bed): T20 from the Schroeder-integrated echograms, CUDA vs SPPS,
Seat 0.868 vs 0.869 s, Seat2 0.850 vs 0.850 s, ratio 0.999 and 1.000. The GPU's and the same code's CPU
histograms agree to 6e-7 of the peak (float atomics' order). 26 % of the nominal particle-steps are
traced: the rest are particles already below the floor.

## What it means for the decision (FORECAST, transfer history: none yet)

A mesh-walking GPU tracer, against SPPS as it runs CR4 today (threaded per band, 6 threads), lands between
about **30x** (the GPU's own 171x, over 6 threads, with the mesh walk costing the GPU as much as it costs
SPPS) and **190x** (the probe's 1,141x over 6 threads: the ceiling, mesh walk free). The CR4-27 run that
took 8 min 18 s would take somewhere between 3 and 17 s of tracing. That is decision 51's road, measured
at its first step; the mesh walk is the next probe, and the number that decides the order.

## Zeph

`C:\Users\Burhan\OneDrive\I-Simpa-Night-Mode-builds\gpu-probe-2026-10-06\`: `run-probe.cmd` (double-click)
runs both arms at 10 M particles and writes `RESULT-<machine>.md` beside it. `box_tracer.exe` holds code
for the RTX 2060 (sm_75) and the RTX 5070 (sm_120); it needs an NVIDIA driver that runs CUDA 13.2
programs (R580 or newer). With an older driver, rebuild there with `build.cmd . sm75` in a VS 2022 +
CUDA 11.8 prompt. Checked on Grace at 200 k particles x 500 steps (19:18): both arms ran, T20 within 2 %.
