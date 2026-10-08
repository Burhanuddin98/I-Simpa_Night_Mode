# FDTD probe, phase 3 result (in progress, 2026-10-07 22:20)

Pre-registration: `PREREG-3.md`. Driver `C:\tmp\nm-fdtd-probe\vchamber.py`; outputs `vchamber_paris\`
(`chamber.{md,json}`, IRs per run), the curve `chamber_curve.json`, the chain log `chain_curve.log`.

## Test A: virtual ISO 354 chamber, PFFDTD default boundary (realised)

V 190.9 m3, shell 203.2 m2, no parallel walls, six hanging rigid diffusers, walls alpha 0.02, 10.8 m2 floor
sample, fmax 400 Hz, 10 PPW, 8 s, six receivers (receiver spread of T +-2-3 %: the field is diffuse).
22-23 s of GPU per run.

| target alpha | delivered 125 Hz | ratio | delivered 250 Hz | ratio |
|---|---|---|---|---|
| 0.05 | 0.042 | 0.84 | 0.044 | 0.89 |
| 0.10 | 0.078 | 0.78 | 0.085 | 0.85 |
| 0.20 | 0.152 | 0.76 | 0.161 | 0.80 |
| 0.40 | 0.296 | 0.74 | 0.324 | 0.81 |
| 0.70 | 0.572 | 0.82 | 0.650 | 0.93 |

The empty chamber agrees: walls set to 0.02 ring 8.70 s at 125 Hz where Sabine with 0.02 gives ~6.8 s.

**Outcome (pre-registered rule): the default mapping under-delivers in a diffuse field** (ratios 0.74-0.93,
outside 0.9-1.1 at most levels). Delivering ~20 % less absorption than asked is the size of the T30 excess in
CR2 and CR3 (x 1.25), so the boundary mapping, not the rooms' wave physics, is the main fault. The chamber gives
the correction curve (`chamber_curve.json`): the input alpha that delivers a target, per octave (63 Hz uses
125, 500 Hz and up use 250); e.g. target 0.10 -> input 0.130 at 125 Hz and 0.119 at 250 Hz.

Caveat carried into the curve: the delivered value adds back 0.02 for the floor the sample covers; the empty
chamber shows that floor really delivered ~0.016, a 0.004 bias, negligible above alpha 0.05.

## Closure check (realised, 22:21)

The chamber again, every alpha passed through the curve: delivered / target 0.98-1.01 at all five levels in both
bands (`vchamber_curve\chamber.md`). The curve does what it was built to do.

## Test B: chamber-calibrated boundary on the four rooms (realised, 22:37)

Forecast with transfer history "calibrated in a virtual chamber, no BRAS measurement used". FDTD / measured T30
on the 10-pair (CR1: 4) means, default boundary -> chamber-calibrated:

| room | 125 Hz | 250 Hz | T30 per-pair err after (125 / 250) | Eyring err (125 / 250) |
|---|---|---|---|---|
| CR1 (coupled) | 1.68 -> 1.42 | 1.46 -> 1.30 | 41.8 / 29.8 % | 51.0 / 61.4 % |
| CR2 | 1.25 -> **1.01** | 1.02 -> 0.88 | 9.4 / 10.9 % | 6.8 / 15.2 % |
| CR3 | 1.25 -> **1.01** | 1.25 -> **1.02** | **4.2 / 3.2 % PASS** | 3.8 / 5.1 % |
| CR4 | 1.08 -> 0.86 | 1.13 -> **0.95** | 13.3 / 5.5 % | 4.4 / 4.3 % |

- **Pre-registered forecast (>= 6 of 8 room-bands within 0.9-1.1): FAIL, 4 of 8** (CR2 125, CR3 125 and 250,
  CR4 250).
- Mean |ratio - 1| over the 8 room-bands: 0.265 -> 0.134; without the coupled CR1, 0.163 -> 0.058.
- The calibration fixes the systematic part (the boundary under-delivering ~20 %) and leaves a room-to-room
  scatter of about +-12 % (CR2 250 Hz 0.88, CR4 125 Hz 0.86). CR1, a lab coupled to a reverberation chamber
  through a door, stays 30-40 % long: its late decay is the chamber's, whose low absorption the GA-fitted inputs
  and FDTD both handle worst.
- EDT and C80 pass nowhere, for FDTD or Eyring.
- Eyring keeps a home advantage on T30 in CR2-CR4: its inputs are BRAS's GA-fitted estimates, tuned to these
  measurements.

GPU throughput this chain: 3.9 Gvox/s (CR2), 1.6 (CR1), 4.7 (CR3), 5.8 (CR4), against 0.43-1.1 in phase 2 for
the same grids. Same numerics; phase-2 run times are not representative (likely a laptop power state; not checked).

## Burhan's ruling on scope, 2026-10-07 22:33 (verbatim)

"fdtd should not handle everything, just the lower end"

Reading: FDTD is a low-frequency engine only. It supplies the low octave bands of a run; SPPS supplies the rest,
band by band (Night Mode's parameters are per band, so the handover needs no crossover filter for the numbers;
a broadband IR for listening needs one). Where the handover sits is set by evidence, not by grid budget alone:
the highest band in which FDTD (chamber-calibrated) is at least as good as SPPS against measurement.
