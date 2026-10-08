# FDTD probe, pre-registration (2026-10-07, written before any run)

Burhan, 2026-10-07 16:09: "work on integrating fdtd" · 16:09: "or whatever you think is necessary"

## Question

Does a wave solver, fed only what Night Mode's own `bras_cr2.simpa` example holds (geometry, BRAS fitted
octave absorption, the BRAS source and microphone positions), reproduce BRAS CR2's measured room response
in the low octaves? Integration is worth building only if it does, and only where the geometric baseline
does not.

## Arms

- **FDTD**: PFFDTD (Hamilton, MIT), CUDA single precision, Cartesian grid, frequency-dependent boundary
  admittances fitted to the octave absorption (`fit_to_Sabs_oct_11`). fmax 1000 Hz, 10 points per wavelength,
  2.5 s. One run per source (LS1, LS2), five microphones each: 10 pairs. Driver: `C:\tmp\nm-fdtd-probe\probe.py`.
- **Eyring / Sabine**: closed form from the same mesh volume, group areas and absorption (what TCR gives).
- **Measured**: `C:\tmp\bras_dl\targets\CR2\CR2_RIR_LS{1,2}_MP{1..5}_Dodecahedron.wav`.

SPPS is not an arm today: no solver build on Zeph yet.

## Metrics and targets (ISO 3382-1 JNDs)

Octave bands 125, 250, 500 Hz (63 Hz reported, not judged: the example has no absorption below 125 Hz).
Per band, the mean over the 10 pairs of the per-pair error:

| metric | target |
|---|---|
| T30 | mean abs relative error <= 5 % |
| EDT | mean abs relative error <= 5 % |
| C80 | mean abs error <= 1 dB |

FDTD passes a band when all three hold. The integration case is made if FDTD passes 125 and 250 Hz and beats
Eyring on EDT and C80 there.

## Known biases, declared now

- Absorption below 125 Hz is the 125 Hz value carried down; BRAS fitted estimates are themselves fits.
- The dodecahedron position is its mid-frequency driver; BRAS's LF driver sits at another height.
- No air absorption in FDTD (below 1 % on T at 500 Hz over this decay).
- One-sided octave filters (4th-order Butterworth, forward), identical for every arm; onset at -20 dB of peak.
- Measured noise: truncation where the late-decay line meets the noise floor, exponential tail compensation.
