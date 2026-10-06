# A2: `spps-gpu`, built and bedded (2026-10-06 20:46-21:45, Grace)

**What it means.** There is now a GPU particle solver that `simpa run-folder` and `simpa results` accept
exactly as they accept SPPS. On CR4's 1 kHz band (2 sources x 300,000 particles, 10 s) it takes
**4.0 s against SPPS's 108.9 s (27x)**. Every parameter it gives is within the Monte-Carlo noise of SPPS's
value: 54 of 55 on CR4 and every value on the fixtures and twelve rule variants. SPPS against itself also
gives 54 of 55. Its CPU build and its GPU build trace every particle identically, in 17 of 17 cases, and
their double sums agree to 5e-14. Fittings, stratified media and directivity balloons are refused with a
message and exit code 2, as the spec says.

**What is not proven.** Transmission (the transmitted children) is ported but no bed case exercises it.
In a closed box every transmitted child leaves through an outer wall and is never run. The GPU's
advantage over the same walk on all 28 CPU threads is 4.1x on the trace, not A1's 15.6x (section c says
why). A2 does not change `solvers/manifest.json`, the app or the run manager: by PLAN's rule a GPU run
never stands in for an SPPS run until A4 passes.

Branch `spps-gpu`, commits `3a14008..` (listed at the end). Sources are in `solvers/spps-gpu/`. The bed
scripts are in `solvers/spps-gpu/bed/`. Raw bed runs are in `.out/spps-gpu/bed/` (gitignored), and the
CPU/GPU pairs are in `C:\tmp\nm-spps-gpu\runs\`.

## What is built

| file | what |
|---|---|
| `src/walk.h` | **The walk, once, `__host__ __device__`.** It holds every A2 rule with its receipt into upstream (`v1.4.0_snapshot_14_01_2026`): Run, Movement, the first-hit face test (Moller-Trumbore, non-culling), TraverserTetra, the reflection laws, the receivers, the surface and cut-plane cells, and the source distributions. It also holds Philox-4x32-10, written out, and its own double-precision sin, cos, acos, log, exp and pow, built from IEEE `+ - * /`, `sqrt`, `floor`, `frexp` and `ldexp` only, so the CPU and GPU builds compute the same bits. |
| `src/model.cpp` | Reads `config.xml` (TinyXML-2, vendored, zlib licence, `THIRD-PARTY.md`), `mesh.cbin` (the node walk) and `tetramesh.mbin` as SPPS does. Patch 0001's time bin is included. Prepares the scene as `initTetraMesh` does: source and receiver localisation, cut-plane linking, `InitRecepteurS`, fitting volumes, `ExpandPunctualReceiverTetrahedronLocalisation`, `TranslateSourceAtTetrahedronVertex` and `CheckSourcePosition`. Also holds the refusals. |
| `src/output.cpp` | Writes SPPS's files with the same names, folders and formats: `.recp`, `.gap`, `Punctual receiver intensity.gabe`, `Sound level per source.recps` and the per-source `.recp`, `Intensity animation/<f> Hz/Intensity.rpi`, the statistics and `Total energy.recp` GABE files, the surface and cut `.csbin` per band and `Global`, and `.pbin` with both CSVs. |
| `src/main.cu` | The command line, the GPU and CPU drivers, and the host re-trace of the particles SPPS marks for its particle file. |
| `build.cmd` | nvcc 13.2 with VS 2022, run through `cmd.exe`. Writes `C:\tmp\nm-spps-gpu\bin\spps-gpu.exe`. `fat` adds sm_75 (Zeph). Uses `-fmad=false`. |
| `bed/*.py`, `bed/test_math.cpp` | The bed: staging, CPU/GPU identity, runs through `simpa`, analysis, refusals. |

**Command line.** `spps-gpu <config.xml>` runs on the GPU, or on the CPU when `SPPS_GPU_BACKEND=cpu` is
set, because `simpa run-folder` passes only the path. `spps-gpu --cpu <config.xml>` runs the same walk
with OpenMP. `spps-gpu --probe` prints the device and exits 0, or prints why there is none and exits 1:
`NVIDIA GeForce RTX 5070, sm_120, 48 SMs, 11.9 GiB, driver CUDA 13.2, runtime 13.2`, and with
`CUDA_VISIBLE_DEVICES=-1`, `no CUDA device: no CUDA-capable device is detected`. Without a device and
without `--cpu`, it refuses with exit 2. The bed adds `--dump-walk` and `--dump-sums`.

**stdout** is SPPS's: `#<pct>` progress lines, `Source at tetrahedron vertex...` when it applies,
`End of calculation.` and `Output results files.`. Every line matches a pattern in the run manager's
classifier. SPPS's own `SPPS version 2.2.1` banner is not printed. **Extra file:** `spps-gpu.json`
beside the outputs records the backend, device, seed and timings.

**GPU scheme.** One persistent slot per resident thread (49,152 on the 5070) holds a family in flight:
the particle and its child queue. Each launch advances every slot by a step budget sized for about
250 ms, the next family comes from a shared counter, and no launch waits on one long-lived particle.
Point-receiver and per-step sums are `double` atomics into replicas, which are folded on the host in a
fixed order (256 replicas for the per-step total, up to 64 for the receivers). Surface and cut cells
are `float` atomics into dense per-band arrays. CR4's plane is 43,930 cells x 10,000 bins, 1.76 GB, and
the solver refuses with `surface_maps_too_large` when the device cannot hold them. The CPU build uses
per-thread Kahan-compensated sums and CAS float adds.

### The two gaps the spec named, read before porting

- **`TranslateSourceAtTetrahedronVertex`** (`spps/sppsInitialisation.cpp:13-34`). When a source lies
  within 1e-4 m (`BARELY_EPSILON`) of a vertex of its tetrahedron, it is moved 0.5 % of the way
  (`distTranslation = 0.005`) towards the tetrahedron's centroid, `(A+B+C+D)/4`, and a line is printed.
  Only the first vertex found counts. Ported as is in `model.cpp`.
- **`vec3::Rotation(n, angle)`** (`lib_interface/Core/mathlib.h:220-230`). This is the Rodrigues
  rotation of `*this` about the axis `n` by `angle`, with `m1 = cos`, `m2 = 1 - cos`, `m3 = sin`, in
  `float`. `BaseUniformReflection` rotates the face normal about the tangent `(n.y, -n.x, 0)` by phi,
  then the result about the normal by theta. Ported term for term (`walk.h` `rotation`).

### The rules (A2-SPEC.md's table)

| # | rule | status | bed coverage |
|---|---|---|---|
| 1-2 | energy w/N in `double` from the `float` w (as SPPS divides), extinction at E0 x 10^-eps | ported | every case |
| 3 | source moved off a vertex, start step `(u16)ceil(delay/dt)` | ported | `sources2` (20 ms delay) |
| 4 | omni: phi then z | ported | every omni case |
| 5 | directivity: unidirection and the XY/YZ/XZ planes ported; **balloon refused** (A3) | ported / refused | `v-srcuni`, `v-srcxy`, `v-srcyz`, and XZ in the identity set; refusal tested |
| 6 | step c dt, c = 343.2 sqrt(T/293.15), `float` | ported | every case |
| 7 | exit face = first of faces 0-3 with n.d < 1e-6 hit by Moller-Trumbore, t unbounded | ported | every case (`v-srcuni` bit-identical to SPPS, below) |
| 8 | the tetrahedron changes only at a collision | ported | every case |
| 9 | LOST (no face after the neighbours, or no neighbour); LOOP after 1000 legs | ported, counted | fates tables |
| 10 | pass-through: fitting faces, back-facing single-sided materials with a neighbour | ported | `v-onesided`, a single-sided box: no back face is ever met, so it is not truly exercised |
| 11 | energetic children through 0 < alpha < 1 when transmission is on, queue of 16 per family, overflow counted on stderr and in `spps-gpu.json` | ported | **not exercised**: no fixture has an interior transmitting wall. Overflow was 0 in every run |
| 12 | random: U <= alpha absorbs, unless U2 alpha <= tau | ported | `seats`, `outputs`, `sources2`, `v-random-diffuse` |
| 13 | diffusion draw (none at 1); specular, Lambert, uniform, W2-W4; law 6 is specular | ported | `v-lambert`, `v-uniform`, `v-w2`, `v-w4`, CR4 (Lambert 5 %) |
| 14 | direct field only | ported | `v-direct` |
| 15 | air absorption once a step, ISO 9613-1 per band | ported | every case, plus `v-noatmo` |
| 16-17 | receivers by the leg's START tetrahedron; chord x E; Lf, Lfc, intensity, per-source | ported | every case |
| 18 | surface receivers before absorption, mode 0 and 1, bin = step/ratio | ported. The in-place normal flip is reproduced per hit. Its persistence across particles cannot be reproduced in parallel; it matters only on a receiver face's unlinked side | `seats`, `energetic`, `outputs`, `v-surfspl` (mode 1) |
| 19 | cut planes per leg, cell `floor(u NU)`, `floor(v NV)`, bin = step/ratio | ported | `outputs`, CR4 |
| 20 | outputs | ported, every file above | `simpa run-folder` verdict OK on all 49 bed runs (24 SPPS, 25 spps-gpu) |

**Refusals (exit 2, one stderr line each), run by `bed/refusals.py`:**
`stratified_unsupported` (alog or blin non-zero), `fittings_unsupported` (`enc_calc` on and a declared
fitting volume in the mesh; with `enc_calc` off the same mesh runs), and `directivity_balloon_unsupported`.
Refusals the spec did not list, each where SPPS crashes or misbehaves: `source_outside_mesh`,
`too_many_steps` (above 65,535, SPPS's 16-bit step), `material_missing`,
`surface_receiver_undeclared`, `surface_maps_too_large` and `no_cuda_device`.

**Where it departs from SPPS on purpose** (none is silent):
- The RNG is Philox, keyed by seed, band and source and counted by particle, child and draw. A seed of
  0 takes the clock and is recorded in `spps-gpu.json`. It is not in the statistics file, because that
  GABE's 7-row layout is read by `simpa` (a deviation from the spec's wording).
- A surface or cut cell adds `(float)E` in `float`. SPPS adds `E` to the cell in `double`, then rounds.
- After a particle dies mid-step, SPPS still translates it with E = 0, which adds only zeros. That
  translation is skipped.
- A GABE string longer than 50 bytes is cut at 50; SPPS overflows the cell. `.csbin` padding is
  written as zeros; SPPS writes heap garbage there.

**Checks of the building blocks** (`bed/test_math.cpp`, MSVC): against the C library on the walk's
ranges, max error is sin and cos 1.1e-16, acos 8.9e-16, pow(x, 1/k) 1.1e-16, log 2.6e-16 relative and
exp 2.2e-16 relative. `acos(1 + 2^-23)` is NaN, as `acosf` is. Philox-4x32-10 matches Random123's three
published known-answer vectors.

## (a) CPU build against GPU build, same config, same seed

`bed/cpu_gpu_all.py` runs both builds on 17 cases, with seed 11 (CR4 seed 7). The receipts are
`C:\tmp\nm-spps-gpu\runs\a-final\summary.jsonl` and `<case>-compare.json`. In the table:
- **"walk"** counts primary particles whose step count, fate, final energy bits, child count or child
  steps differ between the builds.
- **"double sums"** is the largest difference across the per-step total, the receivers' energy, Lf,
  Lfc, intensity and per-source sums, relative to each array's peak.
- **"surface" and "cut"** give the largest relative cell difference among cells above 1e-6 of the
  peak, with the number of non-zero cells in brackets.

| case | bands | particles | steps traced | walk | fates equal | double sums | surface cells | cut cells | trace s, CPU / GPU |
|---|---|---|---|---|---|---|---|---|---|
| seats (random) | 2 | 400,000 | 1,657,460 | **0** | yes | 5.4e-16 | 0 (2,066) | - | 0.045 / 0.010 |
| energetic | 2 | 400,000 | 12,222,947 | **0** | yes | 1.2e-14 | 1.2e-6 (3,679) | - | 0.259 / 0.026 |
| outputs (cut plane, particle file) | 2 | 400,000 | 1,657,460 | **0** | yes | 5.9e-16 | 0 (2,066) | 0 (3,262) | 0.057 / 0.010 |
| sources2 (2 sources, delay, per source) | 2 | 800,000 | 3,287,739 | **0** | yes | 5.0e-16 | 1.5e-7 (2,346) | - | 0.083 / 0.014 |
| v-lambert | 2 | 400,000 | 11,679,112 | **0** | yes | 1.1e-14 | 1.9e-6 (2,394) | - | 0.317 / 0.182 |
| v-uniform | 2 | 400,000 | 11,746,946 | **0** | yes | 1.1e-14 | 1.5e-6 (2,542) | - | 0.292 / 0.146 |
| v-w2 | 2 | 400,000 | 11,715,560 | **0** | yes | 1.1e-14 | 2.3e-6 (2,420) | - | 0.290 / 0.158 |
| v-w4 | 2 | 400,000 | 12,219,033 | **0** | yes | 1.0e-14 | 1.8e-6 (2,429) | - | 0.321 / 0.169 |
| v-direct | 2 | 400,000 | 129,093 | **0** | yes | 6.6e-15 | 0 (89) | - | 0.010 / 0.004 |
| v-srcxy | 2 | 400,000 | 21,266,973 | **0** | yes | 1.4e-14 | (0) | - | 0.390 / 0.086 |
| v-srcyz | 2 | 400,000 | 13,878,356 | **0** | yes | 1.2e-14 | 2.3e-6 (1,390) | - | 0.269 / 0.064 |
| v-srcuni | 2 | 400,000 | 21,200,000 | **0** | yes | 4.0e-14 | 0 (12) | - | 0.310 / 0.010 |
| v-surfspl (map mode 1) | 2 | 400,000 | 12,222,947 | **0** | yes | 1.2e-14 | 1.7e-6 (3,679) | - | 0.262 / 0.065 |
| v-onesided | 2 | 400,000 | 12,222,947 | **0** | yes | 1.1e-14 | 1.2e-6 (3,679) | - | 0.257 / 0.026 |
| v-noatmo | 2 | 400,000 | 12,400,838 | **0** | yes | 5.1e-15 | 9.5e-7 (3,768) | - | 0.254 / 0.026 |
| v-random-diffuse | 2 | 400,000 | 1,658,603 | **0** | yes | 4.0e-16 | 0 (2,023) | - | 0.054 / 0.030 |
| **CR4, 1 kHz** | 1 | 600,000 | **1,360,935,330** | **0** | yes | **4.6e-14** | - | 1.2e-7 (14,365,677) | 6.164 / 1.496 |

**Verdict: the walk is identical** (0 mismatches in 9.0 M primary particles, 1.53 billion steps), and
the double sums agree within 1e-12 in every case (worst 4.6e-14).

The float cells agree to float rounding: a few to about 20 float ulps after thousands of adds in a
different order. The 1e-12 target is met only because of two changes made during the bed:
- On CR4, the per-step total first differed by 1.7e-12. 600,000 nearly equal terms land in one double,
  and the order of the adds shows. Replicas on the GPU and compensated sums on the CPU fixed it
  (commit `3a461c5`).
- The unidirectional source differed by 7e-12: 200,000 identical terms land in one receiver cell.
  Point-receiver replicas fixed it (commit `8f9f891`).

## (b) Against upstream SPPS

**Method.** Each case is staged as a `simpa run-folder` template whose working directory is
`__RUNDIR__`. `simpa` writes it to the fresh run's own folder under `.out\spps-gpu\bed\runs\`. Two runs
are compared with `simpa results --json`.
- **Arms.** The verified SPPS build (`C:\tmp\nm-solvers-timebin\bin\spps.exe`, "solver build verified")
  and `spps-gpu` (checked against a `$SIMPA_SOLVER_MANIFEST` override holding its own code sha256, so its
  runs read "unverified: solver_manifest_override", as they must). Both used seeds 101 and 202 and equal
  particle counts.
- **Particle counts.** The fixtures were raised to 200,000 per source (from 2,000 and 50,000) so their
  parameters leave the noise model's uncalibrated region. CR4 used its own 300,000 per source.
- **"Within noise".** |a - b| <= sqrt(ha² + hb²), where each h is half of the range `simpa results`
  reports with the value: 2.5 Monte-Carlo standard deviations, or EDT's own method range.
- **Control.** SPPS seed 101 against SPPS seed 202, run through the same test.

Bed log: `.out\spps-gpu\bed\bed-*.jsonl`. Analysis: `.out\spps-gpu\bed\analysis.md` and `.json`
(`bed/analyse.py`). Every run's verdict is OK: 24 SPPS runs (10 fixture runs, 2 on CR4, 12 variants)
and 25 `spps-gpu` runs (the same 24, plus the CPU arm on CR4).

| case | pair | values compared | within noise | worst \|d\| / range | not evaluable on both / on one |
|---|---|---|---|---|---|
| **CR4 1 kHz** (5 receivers, each source apart) | gpu s101 vs spps s101 | 55 | **54** | 1.14 (MP3/LS2 EDT) | 20 / 0 |
| | gpu s202 vs spps s202 | 55 | **55** | 0.79 | 20 / 0 |
| | *spps s101 vs spps s202 (control)* | 55 | *54* | *1.11* | 20 / 0 |
| | cpu s101 vs spps s101 | 55 | 54 | 1.14 | 20 / 0 |
| energetic | gpu vs spps, s101 / s202 | 12 / 12 | **12 / 12** | 0.28 / 0.80 | 8 / 0 |
| seats3s (seats box, 3 s) | gpu vs spps, s101 / s202 | 19 / 18 | **19 / 18** | 0.64 / 0.60 | 0 / 1-2 |
| seats (1 s) | gpu vs spps, s101 / s202 | 10 / 13 | **10 / 13** | 0.55 / 0.50 | 0 / 10, 7 |
| outputs | gpu vs spps, s101 / s202 | 10 / 13 | **10 / 13** | 0.55 / 0.50 | 0 / 10, 7 |
| sources2 | gpu vs spps, s101 / s202 | 16 / 29 | **16 / 29** | 0.64 / 0.47 | 16 / 28, 15 |
| v-lambert, v-uniform, v-w2, v-w4 | gpu vs spps s101 | 12 each | **12 each** | 0.50, 0.40, 0.29, 0.97 | 8 / 0 |
| v-srcxy, v-srcyz | gpu vs spps s101 | 12, 6 | **12, 6** | 0.55, 0.43 | 8, 14 / 0 |
| v-surfspl, v-onesided, v-noatmo | gpu vs spps s101 | 12 each | **12 each** | 0.28, 0.28, 0.38 | 8 / 0 |
| v-random-diffuse | gpu vs spps s101 | 18 | **18** | 0.63 | 1 / 1 |
| v-srcuni | gpu vs spps s101 | 2 | **2, bit-identical** | 0.00 | 18 / 0 |
| v-direct | gpu vs spps s101 | report refused by `simpa results` for both | same refusal | - | - |

**No bias.** On CR4 the mean signed |d|/range over both seeds is:

| parameter | spps-gpu vs SPPS: mean, rms | SPPS vs SPPS control: mean, rms |
|---|---|---|
| SPL | -0.08, 0.35 | +0.05, 0.39 |
| EDT | +0.11, 0.45 | -0.06, 0.47 |
| T20 | +0.01, 0.34 | +0.08, 0.48 |
| T30 | +0.05, 0.37 | +0.21, 0.40 |
| C80 | +0.09, 0.28 | -0.03, 0.30 |

Pure noise gives an rms near 0.4.

**CR4 1 kHz, seed 101** (from `analysis.md`; the summed receivers' decay values are not evaluable,
"several_sources", in both arms, so each source's echogram from `output_recp_bysource` is shown):

| receiver/source | SPL dB spps-gpu / SPPS | EDT s | T20 s | T30 s | C80 dB |
|---|---|---|---|---|---|
| MP1/LS1 | 54.90±0.16 / 55.01±0.15 | 2.489±0.059 / 2.451±0.050 | 2.429±0.031 / 2.412±0.044 | 2.410±0.030 / 2.421±0.026 | -1.60±0.21 / -1.52±0.23 |
| MP1/LS2 | 54.08±0.16 / 54.12±0.17 | 2.628 / 2.615 | 2.429 / 2.420 | 2.401 / 2.430 | -2.82 / -2.91 |
| MP2/LS1 | 54.68±0.17 / 54.75±0.16 | 2.429 / 2.417 | 2.370 / 2.348 | 2.372 / 2.374 | -1.13 / -1.16 |
| MP2/LS2 | 55.60±0.15 / 55.51±0.13 | 2.537 / 2.530 | 2.361 / 2.367 | 2.368 / 2.379 | 0.11 / 0.05 |
| MP3/LS1 | 53.85±0.17 / 53.94±0.17 | 2.356 / 2.389 | 2.361 / 2.380 | 2.393 / 2.381 | -1.72 / -1.62 |
| MP3/LS2 | 54.27±0.15 / 54.40±0.16 | **2.416±0.057 / 2.330±0.049 (1.14)** | 2.388 / 2.387 | 2.387 / 2.387 | -1.05 / -0.99 |
| MP4/LS1 | 56.00±0.13 / 55.99±0.13 | 2.312 / 2.326 | 2.362 / 2.386 | 2.380 / 2.368 | 0.87 / 0.68 |
| MP4/LS2 | 53.45±0.18 / 53.52±0.17 | 2.525 / 2.518 | 2.346 / 2.360 | 2.375 / 2.362 | -3.42 / -3.48 |
| MP5/LS1 | 53.67±0.16 / 53.51±0.18 | 2.318 / 2.333 | 2.369 / 2.406 | 2.389 / 2.381 | -1.18 / -1.29 |
| MP5/LS2 | 52.73±0.20 / 52.82±0.18 | 2.389 / 2.365 | 2.381 / 2.333 | 2.417 / 2.376 | -1.54 / -1.57 |

Summed SPL per receiver: MP1 57.52 / 57.60, MP2 58.17 / 58.16, MP3 57.08 / 57.19, MP4 57.92 / 57.94,
MP5 56.24 / 56.19 dB, all within range.

Fates, CR4 s101 (atmosphere / materials / loops / lost / remaining of 600,000): spps-gpu 29,919 /
569,848 / 18 / 215 / 0, SPPS 29,862 / 569,923 / 23 / 192 / 0.

**Beyond the parameters:**
- **Unidirectional source** (`v-srcuni`, every particle on one specular path). The receivers'
  `Sound level.recp` from spps-gpu and from SPPS are **bit-identical, 100 of 100 steps in each band**
  (`bed/recp_totals.py`). This is the walk's geometry, reflection, chord and accumulation reproduced
  exactly along a path where SPPS's RNG plays no part.
- **Maps** (`bed/maps.py`, `.out\spps-gpu\bed\maps.jsonl`; record count and sum of every record):

  | case | map | spps-gpu, s101 / s202 | SPPS, s101 / s202 |
  |---|---|---|---|
  | outputs | Global surface | 5.5840e-5 / 5.5841e-5 | 5.5916e-5 / 5.5766e-5 |
  | outputs | Global cut | 3.6835e-4 / 3.6785e-4 | 3.6849e-4 / 3.6760e-4 |
  | energetic | Global surface | 5.5757e-5 / 5.5612e-5 | 5.5725e-5 / 5.5757e-5 |
  | CR4 | Global cut, 28.7 M records | 2.13860e-2 / 2.13738e-2 | 2.13538e-2 / 2.14192e-2 |

  Each arm's two seeds bracket the other's.
- **Direct field only** (`v-direct`). `simpa results` refuses the report for both solvers with the
  same reason, `results_value_invalid: 6 numbers are not finite` at
  `...spl_db.not_evaluable.error.why.with_missing`. This is a finding for the results reader, not the
  solver. The raw `.recp` totals differ by 1.3 % at Seat2 (about 1,200 crossings, counting noise 3 %)
  and by 4 and 14 % at Seat (about 240 crossings, noise 6.5 %): within 2.2 counting sigma.
- **Rare fates** (`bed/fates.py`; seats box, 1 M particles a seed; spps-gpu over 8 seeds, SPPS over 4):

  | | spps-gpu per 16 M | SPPS per 8 M |
  |---|---|---|
  | lost | 25 (1.6e-6) | 7 (0.9e-6) |
  | loops | 2 | 2 |
  | alive at the end | 17 (1.1e-6) | 1 (1.2e-7) |

  At 200,000 particles, though, SPPS had 4 alive in 800,000 (seeds 101 and 202), so the rates are too
  rare to separate. These "alive" particles are flying out of the mesh: a face hit with t < 0 puts the
  next collision behind them, so they never collide again. That is SPPS's own walk. A band with one of
  them reads "incomplete" in `simpa results`, and its decay values become NE(truncated). That is why
  `seats`, `outputs` and `sources2` show "one side NE" in both directions, and in the SPPS control.

## (c) Speed: CR4, 1 kHz, 2 sources x 300,000 particles, 10 s at 1 ms

Times are the solver process's elapsed time as `simpa run-folder` records it (`run.json`). spps-gpu's
own breakdown comes from `spps-gpu.json`. Source: `.out\spps-gpu\bed\timings-cr4.jsonl`. Grace:
i7-14700KF, RTX 5070.

| arm | seed | wall s | of which: trace s (GPU kernel s) | band output + Global maps s | particle-file re-trace s |
|---|---|---|---|---|---|
| SPPS, verified build (one band: one thread) | 202 | **108.9** | - | - | - |
| SPPS (while a GPU build ran beside it) | 101 | 109.4 | - | - | - |
| **spps-gpu, GPU** | 101 / 202 | **4.02 / 4.03** | 1.43 / 1.45 (1.08 / 1.11) | 0.78 + 0.65 | 0.38 |
| spps-gpu, CPU build, 28 threads | 101 | 8.93 | 6.43 | 0.85 + 0.60 | 0.38 |

- **GPU over SPPS: 27x end to end (108.9 / 4.03 s).** The GPU trace is now 1.4 s of the 4.0 s. The rest
  is writing the same files SPPS writes: two 230 MB cut-plane `.csbin`, from a 1.76 GB plane.
- **GPU over the CPU build: 2.2x end to end, 4.3x on the trace (6.43 / 1.45 s).** This is far from A1's
  15.6x, for three reasons:
  - The same full-rules walk runs 6x faster per particle on the CPU than SPPS does (CPU build 8.9 s
    against SPPS 108.9 s, both writing the same files).
  - SPPS's rules cost the GPU more than they cost the CPU: double energies and receiver chords, the
    first-hit triangle tests, and 5 % Lambert reflections.
  - Double division was the largest of these: the series' divisions held the kernel at 2.6 s until
    they were replaced by literal coefficients (commit `fc83259`, 2.55 s to 0.92 s on the same case).
- Tuning measured on the way (kernel seconds, same case): one thread per family 4.74. Bounded to 64
  registers 3.39. Packed faces 3.05. Persistent slots 2.55. Horner series 0.92 to 1.11.

## Open

- **Transmission children (rule 11):** ported, but not bedded. A case with an interior transmitting
  wall is needed, run against SPPS.
- **Fittings and directivity balloons:** refused, until A3.
- **Stratified media:** refused (no step planned).
- **The receiver-face normal flip's persistence across particles (hazard 4)** cannot be reproduced in
  a parallel walk. It matters only on an unlinked side of a surface-receiver face.
- **`simpa results` refuses direct-field-only runs**, for SPPS as for spps-gpu (`results_value_invalid`).
- **Not done:**
  - the `solvers/manifest.json` row, the app's solver choice and the run manager (A4/A5);
  - Zeph: `build.cmd <out> fat` holds sm_75 but has not been built or run there;
  - the 27-band CR4 run, whose host memory for its maps is 3 x 1.76 GB, fine on Grace and not yet sized
    for Zeph;
  - the stats file does not carry the run's seed.
- **Not compared with SPPS:** the `.pbin` and its CSVs. Their format and selection follow SPPS, but
  the particles differ by RNG.

## Commits on `spps-gpu`

`3a14008` the walk, readers and writers · `3a461c5` replicated and compensated per-step sums ·
`9558a14` persistent GPU slots, packed faces · `fc83259` Horner series, rule variants, the bed ·
`8f9f891` point-receiver replicas · the report commit (this file, with the last bed scripts).
