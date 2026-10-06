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
| this probe's code, all 28 CPU threads (OpenMP, i7-14700KF) | **2.193 s** | measured at 10 M, 19:20 |
| this probe's code, CUDA kernel | **0.162 s** | cudaEvent; 0.163 s with the copy back |
| the CUDA process end to end, CUDA start-up included | 0.327 s | |

**Speed-up, kernel over SPPS: 1,141x. It splits into two factors, and only one is the GPU:**

- **6.7x is the simpler problem**, not the GPU: the probe's walls are six analytic planes, SPPS walks a
  tetrahedral mesh (any geometry) and keeps its full accounting. A real tracer must walk a mesh too.
- **171x is the GPU** over the same code on one CPU thread, and **13.5x over the same code on all 28 of
  Grace's CPU threads** (2.193 s against 0.162 s, REALISED 19:20; the 18:19 forecast said 12x). That is
  the honest GPU-versus-CPU number: same algorithm, whole chip against whole chip.

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

## A1: the mesh walk on CR4 (2026-10-06 19:30-19:43, REALISED on Grace)

CR4's own tetrahedral mesh from the CR4-27 run (11,150 tetrahedra, 5,148 scene faces), each face's 1 kHz
absorption from the run's config.xml (0.048 to 0.349), specular, energetic, source LS1, receivers MP1-MP5
(r = 1 m), 1 ms steps, 2 s, extinction 1e-5, no air absorption. `prep_mesh.py` makes the input,
`mesh_walk.cu` walks it (planes per tetrahedron, the neighbour across the exit face, reflection on scene
faces), `spps_cr4_arm.py` runs SPPS on a copy of the same folder with the same settings (diffusion set to 0
at the band). Raw: `.out/gpu-probe/a1/`.

| arm | particles | time | per particle |
|---|---|---|---|
| SPPS, the verified build (one band: one thread) | 200,000 | 19.59 s (process, outputs included) | 98.0 us |
| the walk, all 28 CPU threads | 200,000 | 0.958 s | 4.79 us |
| the walk, CUDA kernel, RTX 5070 | 10,000,000 | 3.023 s | **0.302 us** |

**GPU over SPPS at equal particles on CR4: 324x. GPU over the same walk on the whole CPU: 15.6x.** 0 of
10 M particles lost; the walk's GPU and CPU histograms agree to 4.3e-4 of the peak.

T20 (Schroeder over the 2 s), GPU at 10 M vs SPPS at 200 k: MP1 2.976 / 2.925, MP2 2.917 / 2.881, MP3 2.945 /
2.976, MP4 2.911 / 2.895, MP5 2.963 / 2.960: ratios 0.990 to 1.017, inside the 5 % JND. A sanity check, not
a bed: the 2 s window truncates both decays alike.

FORECAST (from A1's per-particle times, transfer history: none): CR4-27 as the app runs it (2 sources x
300 k particles x 27 bands x 10 s, SPPS threaded per band) took 525 s at 19:40; the GPU at about 1 us a
particle for 10 s would trace its 16.2 M particles in 15 to 25 s, about 20 to 35x faster, before A2 adds
SPPS's full rules (diffusion, children, air absorption), which cost the GPU and SPPS alike.
