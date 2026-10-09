# FDTD probe code and the chamber calibration curve

Copied from `C:\tmp\nm-fdtd-probe\` on Zeph (2026-10-09). The receipts for everything here are `../RESULT.md` to
`../RESULT-4.md`; this folder is the code and the small outputs behind them, so the work can continue on Grace.

## The calibration curve

`chamber_curve.json`: per octave (125, 250 Hz), the absorption PFFDTD's default (Paris) boundary must be given
(`input`) to *deliver* a target random-incidence absorption (`delivered`) in a virtual ISO 354 chamber. Made by
`vchamber.py` with no BRAS data (`../PREREG-3.md`, `../RESULT-3.md`): the default boundary under-delivers (74-93 % of
target), the curve brings delivered/target to 0.98-1.01. `probe.chamber_input(alpha, fc)` applies it (nearest
calibrated band; above the calibrated range the last ratio is carried; capped at 0.95).
Chamber summaries: `vchamber_curve/chamber.{json,md}` (with the curve) and `vchamber_paris/` (default boundary); the
`model_*.json` files are the chamber models. The 430 MB of raw chamber output per arm stayed on Zeph.

## Scripts

| file | what it does |
|---|---|
| `probe.py` | Builds a PFFDTD model from a Night Mode `.simpa` (voxel inside test, materials through the curve), runs the CUDA exe, reads IRs, computes EDT/T30/C80 per octave against the BRAS measurement and Eyring. `PROBE_ROOM`, `PROBE_BC` (`paris`/`curve`), `PROBE_FMAX`, `PROBE_DUR` select the arm |
| `vchamber.py` | The virtual ISO 354 chamber: sample patch at five absorptions plus empty, delivered alpha = 55.3 V/(cS)(1/T - 1/T0) |
| `check_fit.py` | Checks the chamber fit |
| `spps_compare.py` | SPPS (Night Mode's own `simpa results --json`) vs FDTD vs Eyring vs measured, per band; writes `compare_CR*.{md,json}` |
| `spps_decay_check.py` | Night Mode's T30 vs T30 recomputed from the raw echograms vs room-wide decay vs Kuttruff |
| `vtracer.py` | Independent diffuse energy ray tracer (Lambert walls, SPPS's air absorption): the exact transport reference that showed Kuttruff 10-13 % short on CR3/CR4 |
| `spatial_spectral.py` | Measured vs FDTD 40-250 Hz spectrum per position (r 0.71 against a 0.42 null) |
| `*.sh`, `*.log` | The chained runs as they were launched, with their logs |
| `compare_CR{2,3,4}.*` | The phase-4 tables (CR4 from the base room, `simpa run --base`, written 2026-10-08 00:08) |
| `ism_sitemap.md` | Which image-source codes existed on Zeph before phase 5 (all shoebox-only) |

## To run on Grace

Paths inside are Zeph paths: PFFDTD at `C:\RoomGUI\pffdtd` (MIT, with its CUDA exe `c_cuda\fdtd_main_gpu_single.exe`
built on Zeph; build it on Grace), HDF5 1.14.6 and CUDA 12.4 DLLs (`DLL_PATH`), BRAS measured RIRs at
`C:\tmp\bras_dl\targets\CR*` (on Grace `$env:AQ_BRAS_ROOT`), outputs under `C:\tmp\nm-fdtd-probe\`. Two PFFDTD setups
must not run at once (they share `pffdtd\python\mmap_dat`). Every measured-room comparison of CR4 needs the base room
(`simpa run --base`, or check `active_variant`).
