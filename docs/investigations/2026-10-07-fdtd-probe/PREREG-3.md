# FDTD probe, phase 3 pre-registration (2026-10-07 22:13, written before any phase-3 run)

Burhan, 2026-10-07 21:06: "nice okay, i believe that we should test more and find the RIGHT way to integrate it"

## Why this phase

Phase 2 (`RESULT-2.md`): with BRAS fitted estimates, FDTD's default boundary decays too slowly in all four rooms
by a room-dependent factor (T30 ratio 1.02 to 1.68), and a forecast of one shared factor missed on CR4. A factor
fitted on BRAS rooms is fitted on the answer. Night Mode's users enter random-incidence absorption, which by
definition is what a reverberation chamber measures (ISO 354). The principled rule is therefore to make the FDTD
boundary deliver, in a virtual ISO 354 chamber, the absorption the user typed, with no BRAS measurement involved.

BRAS's own fitting moved 125 Hz absorption above its literature values (initial estimates, now on Zeph at
`C:\tmp\bras_surface\`) by about 1.2x (CR3, CR4) to 1.8x (CR2) and up to 3x (CR1): a geometrical model already
needed more low-frequency absorption than the literature gives, and FDTD needs more again.

## Test A: virtual ISO 354 chamber (`C:\tmp\nm-fdtd-probe\vchamber.py`)

- Chamber about 200 m3, no two walls parallel, rigid-ish walls (alpha 0.02), a few hanging rigid diffuser
  panels absorbing on both sides; a 10.8 m2 sample patch on the floor.
- Sample alpha (flat in frequency) 0.05, 0.10, 0.20, 0.40, 0.70, each mapped by PFFDTD's default (Paris); one
  empty-chamber run. fmax 400 Hz, 10 PPW. One source, six receivers, T20 per receiver, averaged.
- Delivered alpha = 55.3 V / (c S) (1/T_sample - 1/T_empty), octaves 125 and 250 Hz.
- **Outcome rule.** If delivered / target is within 0.9-1.1 at every level, the default mapping is right in a
  diffuse field and the room gap is not a boundary-mapping fault. Otherwise the chamber gives a correction curve
  alpha_in(alpha_target), used unchanged in Test B.

## Test B: the chamber-calibrated boundary on the BRAS rooms (only if Test A gives a curve)

All four rooms, fitted estimates passed through Test A's curve, the same grids as phase 2. This is a forecast
with transfer history "calibrated in a virtual chamber, no BRAS measurement used". Judged against the
`PREREG.md` targets and against Eyring. The forecast to beat: the curve brings each room's 125 / 250 Hz FDTD /
measured T30 ratio within 0.9-1.1 in at least 6 of 8 room-bands.
