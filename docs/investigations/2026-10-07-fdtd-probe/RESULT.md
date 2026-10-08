# FDTD probe, result (2026-10-07 16:27)

Pre-registration: `PREREG.md` (targets set before any run). Driver and outputs: `C:\tmp\nm-fdtd-probe\`
(`probe.py`, `spatial_spectral.py`, `check_fit.py`; `out\` = Paris arm, `out_normal\` = normal-incidence arm;
IRs as `irs.npz` and WAV per pair, `results.{json,md}`, `spatial_spectral.md`).

Configuration: Night Mode `app/src-tauri/examples/bras_cr2.simpa` (V 146.7 m3, S 202.5 m2, BRAS fitted octave
absorption 125 Hz-4 kHz, BRAS LS1/LS2 x MP1-MP5 = 10 pairs). PFFDTD CUDA single, Cartesian, fmax 1000 Hz, 10 PPW,
2.5 s; 4.7 M cells, 54 s per source on the RTX 2060 (3.9 Gvox/s). Measured: BRAS CR2 dodecahedron RIRs.

## 1. Pre-registered verdict: FAIL

PFFDTD's default boundary (octave alpha read as random-incidence, Paris inversion, `fit_to_Sabs_oct_11`):

| band | T30 meas / FDTD / Eyring | T30 err FDTD / Eyring | EDT err FDTD / Eyring | C80 err FDTD / Eyring |
|---|---|---|---|---|
| 125 | 1.40 / 1.75 / 1.35 s | 28.5 % / 6.8 % | 31.1 % / 11.8 % | 2.18 / 1.77 dB |
| 250 | 1.79 / 1.83 / 1.50 s | 8.5 % / 15.2 % | 23.0 % / 5.7 % | 1.66 / 1.02 dB |
| 500 | 2.02 / 2.20 / 2.07 s | 9.0 % / 2.9 % | 10.7 % / 6.4 % | 0.91 / 0.62 dB |

Targets: T30 and EDT <= 5 %, C80 <= 1 dB. Only 500 Hz C80 passes. FDTD is worse than Eyring on every judged
metric except T30 at 250 Hz. FDTD decays too slowly in every band (effective alpha at 125 Hz 0.065 against the
0.083 Eyring implies).

## 2. Why: the boundary model, not the solver or the fit

- The fit delivers its target: achieved random-incidence alpha of the fitted admittances is within 7 % of target
  for every material and band, most within 2 % (`check_fit.py`; worst windows 125 Hz, 0.175 vs 0.188).
- Voxelisation loses 1-6.5 % of the lossy areas (windows -6.5 %, concrete -6.3 %): small against 22 %.
- A locally-reacting admittance absorbs about half as much at normal incidence as its random-incidence alpha says
  (4g vs 8g for small g). Below CR2's Schroeder frequency (~195 Hz) decay is carried by modes near normal and
  grazing incidence, so a wave model fed GA-tuned random-incidence alpha rings too long.
- Arm added after the fact (not pre-registered): alpha read as normal-incidence. It overshoots: T30 0.97 / 1.01 /
  1.14 s at 125 / 250 / 500 Hz against 1.40 / 1.79 / 2.02 measured. The two readings bracket the room; the right
  boundary lies between, and finding it on this room's measurement is calibration (a ceiling), not validation.

## 3. What FDTD does show: position-specific low-frequency response

`spatial_spectral.py`, Paris arm. 1/24-octave magnitude, 40-250 Hz, Pearson r of dB:
FDTD vs its own measured pair, mean **r = +0.71** (0.50 to 0.83); null, FDTD vs the other 9 measured pairs,
mean r = +0.42, 95th percentile +0.65. 7 of 10 pairs clear the null's 95th percentile. Eyring and SPPS carry no
phase or modal information, so they have no skill here at all. Per-position decay metrics are weak and mixed
(125 Hz EDT r +0.55, C80 +0.48, T30 -0.55; n = 10).

## Caveats

One room. Dodecahedron at its mid-frequency driver position. Absorption below 125 Hz carried down. SPPS is not an
arm (no solver build on Zeph yet). `BufferError: cannot close exported pointers exist` prints at each setup's
exit (h5py memory maps at interpreter shutdown); both GPU runs exited 0 and wrote complete outputs.
