# A2: what `spps-gpu` reproduces of SPPS, rule by rule (2026-10-06 19:45)

Source of every rule: `SPPS-LOOP.md` beside this file (receipts into upstream `v1.4.0_snapshot_14_01_2026`,
paths under `src/`). PLAN.md, Track A. The target is **statistical** equality with SPPS, not bit equality:
SPPS draws every particle from one global lagged-Fibonacci stream in band, source, particle, child order
(SPPS-LOOP section 1 and hazard 1), which no parallel tracer can replay. The bed (A4) compares within the
Monte-Carlo noise; our own CPU and GPU builds of the walk compare bit for bit at equal seed.

## Kept as SPPS does it

| # | rule | SPPS (SPPS-LOOP) | `spps-gpu` |
|---|---|---|---|
| 1 | particles per source per band, energy w_j/N, w_j = 1e-12 x 10^(dB/10) | s.1 | same, `double` energy |
| 2 | extinction at E0 x 10^-trans_epsilon | s.1, s.4 | same |
| 3 | start at the source moved to a tetra vertex (`TranslateSourceAtTetrahedronVertex`); start step ceil(delay/dt) | s.1 | same; the vertex move read before porting (gap) |
| 4 | omni direction: phi = 2 pi U, z = 2U - 1, phi drawn first | s.1 | same formula, counter RNG |
| 5 | directivity (XY/XZ/YZ planes, unidirection, measured balloon) | s.1 | omni and unidirection in A2; the balloon in A3 (it needs the loudspeaker files) |
| 6 | step = c dt, c from T (`Celerite_du_son`), float | s.1 | same formula and type |
| 7 | the walk: exit face = **first** of faces 0-3 with n.d below 1e-6 hit by Moller-Trumbore, not the nearest | s.2 | the same test in the same order (the probe's nearest-plane walk is replaced) |
| 8 | tetra changes only at a collision; free legs keep the start tetra | s.2 | same |
| 9 | LOST: no exit face after trying the neighbours, or no neighbour; LOOP after 1000 legs in a step | s.2 | same states, counted in the stats |
| 10 | pass-through faces: fitting faces, and back-facing one-sided materials with a neighbour | s.2 | same |
| 11 | energetic: child at tau E through the face when 0 < alpha < 1 and transmission is on; parent x (1 - alpha) | s.3 | same; children on a per-thread stack (depth bounded, overflow counted, never silent) |
| 12 | random: U <= alpha absorbs, unless transmission (U2 alpha <= tau) | s.3 | same draws, counter RNG |
| 13 | reflection: diffuse with probability `diffusion` (1: no draw), laws specular, Lambert, uniform, W2-W4 (`BaseWn`, theta then phi) | s.3 | same laws; `vec3::Rotation` read before porting (gap) |
| 14 | direct field only: the first surface hit kills | s.3 | same |
| 15 | air absorption once a step, exp(-alpha c dt), alpha from `Coef_Att_Atmos` | s.4 | same formula, ported with its receipt |
| 16 | point receivers: per leg, chord through the sphere x E, `double`, at the step's index; tested only for receivers linked to the leg's **start** tetra (`ExpandPunctualReceiverTetrahedronLocalisation`) | s.5 | same list, built the same way on the host |
| 17 | the receivers' Lf, Lfc, intensity and per-source sums | s.5 | same |
| 18 | surface receivers: on every scene-face hit, before absorption, E or E/cos (`surf_receiv_method`), float; bin = step / ratio (patch 0001) | s.6 | same; the in-place normal flip (hazard 4) reproduced and flagged, not fixed, so the bed compares like with like |
| 19 | cutting planes: per leg crossing the parallelogram, cell floor(u NU), floor(v NV), float, bin = step / ratio | s.6 | same |
| 20 | outputs: the point-receiver gabe files x c rho, the stats and cumul files, the surface `.csbin` per band and Global (sparse, patch 0002's format), cut planes | s.8 | the same files, names and formats (`docs/formats/`), so `simpa results` and the verdict read them unchanged |

## Refused in A2, each with its message (never silently approximated)

- **Fittings** (`enc_calc` with fitting volumes, s.7): refused with `fittings_unsupported` until A3.
- **Stratified media** (tetra z != -1, s.2): refused with `stratified_unsupported`.
- **A seed of 0**: accepted; the counter RNG takes the run's own seed (the time) and writes it to the stats,
  so every GPU run is replayable, which SPPS's unseeded threaded runs are not.

## The RNG

Counter-based, keyed by (seed, band, source, particle, child, draw index): Philox-4x32-10 (Random123's
algorithm, the one cuRAND implements, written out in the source so the CPU build computes the same
numbers). Each draw site has a fixed index, so CPU and GPU builds trace identical particles.

## Accumulation

Point receivers and the per-step totals in `double` (SPPS does), by atomics on the GPU; the order of adds
changes the last bits, so the CPU/GPU check is to 1e-12 relative, and the GPU/SPPS check is the bed's.
Surface and cut cells in `float` as SPPS keeps them, accumulated per band in device memory sized from the
map (sparse on output: patch 0002's rule, dense in memory while it fits, else per-face lists; the CR4-27
plane is 43,930 cells x 10,000 steps x 4 B = 1.76 GB a band, which fits the 5070's 12 GB but not the
2060's 6 GB: the bin of patch 0001 or a per-band flush decides it, A3).

## Done when (A2)

The CPU build of `spps-gpu` runs the parity tutorials and CR4 one band with every rule above, its outputs
read by `simpa results`, and the GPU build traces the same particles bit for bit on the walk and to 1e-12
on the sums.
