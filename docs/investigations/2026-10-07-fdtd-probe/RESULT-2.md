# FDTD probe, phase 2 result (in progress, 2026-10-07 21:40)

Pre-registration: `PREREG-2.md`. Driver `C:\tmp\nm-fdtd-probe\probe.py` with `PROBE_ROOM`, `PROBE_FMAX`,
`PROBE_DUR`; outputs `C:\tmp\nm-fdtd-probe\out_<room>_paris_400\` (IRs `irs.npz` + WAV, `results.{json,md}`).
Default boundary (octave alpha as random-incidence, Paris inversion) in every row below. All numbers realised.

## T30, 10-pair (CR1: 4-pair) means, measured / FDTD / Eyring

| room | f_S | 125 Hz | FDTD err / Eyring err | 250 Hz | FDTD err / Eyring err | grid |
|---|---|---|---|---|---|---|
| CR1 | ~170 Hz | 4.30 / 7.21 / 2.02 s | 67.8 / 51.0 % | 3.90 / 5.68 / 1.47 s | 45.5 / 61.4 % | 400 Hz, 8 s |
| CR2 | ~195 Hz | 1.40 / 1.75 / 1.35 s | 28.5 / 6.8 % | 1.79 / 1.83 / 1.50 s | 8.5 / 15.2 % | 1000 Hz, 2.5 s |
| CR3 | ~50 Hz | 1.64 / 2.05 / 1.66 s | 25.1 / 3.8 % | 1.45 / 1.81 / 1.52 s | 24.9 / 5.1 % | 400 Hz, 5 s |
| CR4 | ~26 Hz | 2.43 / 2.62 / 2.35 s | 8.1 / 4.4 % | 2.37 / 2.67 / 2.47 s | 12.6 / 4.3 % | 400 Hz, 4 s |

EDT and C80 per room: each `results.md`. No judged band passes in any room.

FDTD / measured T30 on the means: CR1 1.68 / 1.46, CR2 1.25 / 1.02, CR3 1.25 / 1.25, CR4 1.08 / 1.13
(125 / 250 Hz).

## Hypotheses

- **H-modal is refuted by CR3**: Schroeder ~50 Hz, yet FDTD T30 is +25 % at both 125 and 250 Hz while Eyring
  is within 3.8 / 5.1 %. The over-long decay is not a below-Schroeder effect.
- **H-sys holds in direction, not in size**: FDTD decays too slowly in all 8 room-bands (ratio 1.02 to 1.68).
- **The 21:39 CR4 forecast missed** (1.15-1.35 forecast; 1.08 / 1.13 realised). The over-long decay is not one
  systematic factor, so the planned CR2-fitted factor cannot transfer and is not run.

## What this does to the comparison itself

The BRAS "fitted estimates" in every Night Mode example are absorption values that BRAS's authors adjusted until
their geometrical-acoustics simulations matched these same measurements. Eyring (and SPPS) therefore start
calibrated to the answer; FDTD gets numbers tuned for a different model of the walls. Measured-vs-FDTD with
fitted estimates is not a fair test of FDTD. The fair test runs every arm on BRAS's **initial estimates**
(literature values, not tuned to the measurement), which are not on Zeph.

## Run notes

- CR1 at 3 s was truncated (FDTD T30 ~ 5.7-7 s needs more than 3 s to fall 35 dB); the 8 s rerun is the one
  reported. The truncated run read 5.69 s at 125 Hz against the full run's 7.21 s.
- The `BufferError: cannot close exported pointers exist` tracebacks are Python 3.12 SharedMemory cleanup
  during voxelisation, before any .h5 is written; all outputs read complete (sentinel, 21:17). Two setups must
  not run at once: they share PFFDTD's `python\mmap_dat` scratch folder, and one collision lost a run.
- Throughput depends on boundary share: 3.9 Gvox/s on CR2 (1000 Hz grid), 0.70 Gvox/s on CR3 (628 k boundary
  nodes, frequency-dependent updates).
