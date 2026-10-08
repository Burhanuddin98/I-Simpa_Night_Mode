# FDTD probe, phase 5: an image-source early part above the handover (2026-10-08 22:44 to 2026-10-09 01:4x, Zeph)

Pre-registration with every amendment, each timestamped before its run: `PREREG-5.md`. Code: `ism/` here (copied
from `C:\tmp\nm-fdtd-probe\ism\` on Zeph; paths inside are Zeph paths). Large artefacts stay on Zeph: SPPS runs
`C:\tmp\nm-spps-runs\` (`cr3_s0`, `cr3_s0_r01`, `cr4_s0`, `shoebox_s0` and their `.json`), spliced arms
`C:\tmp\nm-fdtd-probe\ism\runs\`, arrival lists `C:\tmp\nm-fdtd-probe\ism\arrivals_*.npz`.

Burhan's rulings during the phase (verbatim in `PREREG-5.md`): 23:45 "the goal is to build a hybrid engine, so if we
need fdtd and image and rays, thats what we do"; 23:48 "every aspect must use the GPU". The image sources are
therefore a component of the hybrid, not an option this phase could reject; their bar became "correct and complete".

## 1. The energy question: answered, no gain (realised)

Arms on SPPS's own echograms, each through Night Mode's own parameter code (`simpa results --json` on a spliced copy
of the run folder; gate 1: the untouched copy reproduces all 300 parameter values exactly on CR2, CR3, CR4).
S = SPPS; H = image sources up to sqrt(V) ms after the direct sound with (1-alpha)(1-s) per reflection, SPPS after;
H' = the same with (1-alpha) only. Mean error against the BRAS measurement, 500-4000 Hz:

| room | C80 S / H / H' (dB) | EDT S / H / H' (%) |
|---|---|---|
| CR2 | 0.41-0.66 / 0.43-0.74 / 0.41-0.67 | 7.1-10.0 / 7.1-10.1 / 7.1-10.0 |
| CR3 | 0.71-1.11 / 1.64-2.58 / 0.84-1.45 | 12.1-21.4 / 19.1-28.1 / 12.3-21.6 |
| CR4 | 1.08-1.82 / 2.64-3.62 / 1.58-2.42 | 12.1-25.0 / 17.7-40.8 / 16.3-34.2 |

Neither arm moves C80 or EDT toward the measurement anywhere; the pre-registered forecast held. SPPS already carries
the specular plus scattered energy in expectation; the 1-3 dB C80 and 12-25 % EDT gaps are the long decay RESULT-4
traced to the GA-tuned BRAS absorption. **These arms used the image-source code before the two fixes of section 3**,
so H and H' are low by the missing paths (in CR4 by about 1.2 dB of early energy); the conclusion "no gain over
SPPS" does not depend on it, since H' can at most reach S's early energy, but the CR4 numbers above are not final.

## 2. SPPS's specular part is right (realised)

- Shoebox (7.3 x 5.1 x 3.2 m, alpha 0.10, scattering 0, 10 M particles, r 0.31 m) against the closed-form image
  lattice: cumulative energy at 100 ms -0.025 dB mean (-0.039..-0.008), 1588 of 1590 checks in band; the 2 misses
  (MP2 4000 Hz at 7-8 ms) are the gate's window edge, per a sentinel seat (direct sound +1.28 % against a 1.18 % 3-sigma
  tolerance, and the first reflection smeared across the window edge).
- spps-gpu applies (1 - alpha) before choosing specular or diffuse (`walk.h:612`, branch at `walk.h:634`); reflection
  timing is exact (`walk.h:576/657`); TetGen's boundary equals the .simpa geometry to 1 mm at 20 000 points.
- SPPS's 0.31 m sphere receives part of a neighbouring image near panel edges; at r = 0.10 m (100 M particles, 500 Hz)
  the order 0-2 reflections in CR3 match the point image sources (SPPS/ISM 1.004, 1.023, 0.944).

## 3. The image-source code: two bugs found and fixed (realised)

A path-logging specular tracer on the GPU (`ism/tracer.py`: 20 M rays, logs every pass within 5 cm of a receiver
with its plane sequence) found real specular paths the image sources rejected, e.g. CR4 LS2-MP2 first-order
brickwall at 42.91 ms, 59 rays within 2 mm of the receiver. Causes (`ism/dbg_path.py`):

1. **A reflecting triangle occluded its own reflection.** The reflection point is computed on the merged plane; its
   triangle sat 1 um off it, and the occlusion margin was relative (1e-7 of the segment, 0.9 um on 8.7 m). Fix: no
   occluder within 2.5 mm (absolute) of either end of a segment (`EPS_ABS`).
2. **The plane merge was too loose.** Angle 1e-4 in cos (0.8 degrees) with the offset checked at one triangle let far
   triangles sit 18.6 mm off their plane (CR4 plane 192, triangle 4660), which then occluded their own reflection 3 cm
   along the path. Fix: a triangle joins a plane only if parallel (either sign) and all three vertices lie within
   1 mm; planes are now flat to 0.002 / 0.80 / 0.87 mm (CR2 / CR3 / CR4; 102 / 365 / 1397 planes). The same fix
   folds 554 exactly coplanar same-material plane pairs in CR4 that a sign-canonicalisation split had kept apart.

Gate 2 (closed-form shoebox lattice, orders <= 3) passes exactly on CPU and GPU after both fixes.

Ruled out on the way, each with a receipt in the log above: absorption handling, geometry differences, facets, ray
recall (1 M against 100 k rays added no energy before t_c in CR3 and moved CR4 by 0.06 dB), SPPS time stepping.

## 4. Gates as run (realised; before the fixes unless marked)

| gate | result |
|---|---|
| 1 metric reproduction | PASS, CR2 CR3 CR4, 300/300 values identical |
| 2 shoebox lattice | PASS, CPU and GPU, before and after the fixes |
| 3(b) CR3, v1 | FAIL 2/60 (window edge, sentinel); v2 amended for CR4 PASS |
| 3c CR3 s=0, r 0.31 | FAIL 1179/2724; E_ISM/E_SPPS at t_c -0.192 dB mean |
| 3c CR4 s=0, r 0.31 | FAIL 3464/5568; -1.175 dB mean |
| 3d shoebox | 1588/1590 (misses are the gate's edge) |
| 3e CR3 r 0.10 | FAIL by its rule: mean abs 0.342 -> 0.249 dB (needed <= 0.171) |
| 3c CR4 s=0, **after the fixes** | FAIL 1070/4884 by the letter; **-0.071 dB mean** (was -1.175) |

**CR4 after the fixes (2026-10-09 01:32, realised).** Arrivals regenerated with the fixed code on the GPU (orders <= 2
exhaustive, 3-6 from 1 M rays; 940 s against 2586 s before the fused kernels). Gate 3c on `cr4_s0.json` (scattering 0,
10 M particles, r 0.31 m): E_ISM/E_SPPS at t_c **-0.071 dB mean**, range -0.421..+0.146 dB, mean |.| 0.096-0.143 dB per
band (before: -1.175 dB, range -1.656..+0.048). The tracer (20 M rays, LS2 to MP2 and MP4, < 110 ms) now finds 5 traced
paths without an image-source arrival, down from 16; the 16 rejected by the old code passed within 2-9 mm of the
receiver, the 5 left pass 16-33 mm from it, the signature of paths valid for part of a sphere near a panel edge rather
than of a third bug (not yet shown). Gate 3c still fails by its letter (1070 of 4884 checks): at 10 M particles its
3-sigma band is a few hundredths of a dB, tighter than the sphere's edge effect. **Not yet run with the fixed code:**
CR3 arrivals and gate 3c on `cr3_s0` / `cr3_s0_r01` (the bar for "CR3 works"), and the energy arms of section 1.

## 5. GPU

The image sources now run on the GPU (Burhan 23:48): fused CUDA kernels through CuPy, one thread per ray
(`nearest_f32`, float32: the ray search only proposes candidates) and one per segment (`anyhit_f64`, float64 with
the CPU's tolerances: occlusion decides validity), plus a padded per-plane triangle table for the polygon test.
Each port reproduced the CPU reference exactly (gate 2; CR2 1149/1149 arrivals, |dL| <= 7e-15 m). CR2 25 s CPU ->
6 s GPU. The RTX 2060 Max-Q runs float64 at 1/32 of float32; Grace's RTX 5070 will be faster.
