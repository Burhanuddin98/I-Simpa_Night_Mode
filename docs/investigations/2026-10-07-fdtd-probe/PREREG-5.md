# Phase 5 pre-registration: an image-source early part above the handover (2026-10-08 22:47, before any run)

Burhan, 2026-10-07 22:45: "fdtd needs to maybe be combined with spps to deliver a hybrid method, who knows, maybe
even Image sources method might be useful"; 23:39 "i agree" (ISM as a test after the 250 Hz handover);
2026-10-08 22:44 "lets continue with the next piece of work". Backlog row 102 (v1.1).

## Question

Above the 250 Hz handover, does an exact specular early part (image sources with visibility) in place of SPPS's
early echogram move EDT and C80 toward the BRAS measurements on CR2, CR3 (advised run) and CR4 (base room)?

Row 102's own "done when" is about the auralization (discrete reflections at exact delays). This phase asks the
energy question only: is there a room-acoustic-parameter gain as well, or is the ISM's value the listening alone?

## Inputs (all existing, no new solver run)

- SPPS: `C:\tmp\nm-spps-runs\cr2.json`, `cr3_advised.json`, `cr4_base.json` (verified spps-gpu, per-band
  `energy_pa2` echograms at 1 ms, 125-4000 Hz, per source x receiver).
- Geometry and materials: `app/src-tauri/examples/bras_cr{2,3,4}.simpa`, base materials (CR4: never the variant).
- Measured: `C:\tmp\bras_dl\targets\CR{2,3,4}\*_RIR_LS{1,2}_MP*_Dodecahedron.wav`, metrics by `probe.metrics`
  (noisy=True), the code every earlier phase used.
- Bands judged: 500, 1000, 2000, 4000 Hz.

## Arms (energy domain, per source x receiver x band, 1 ms bins)

- **S**: SPPS's echogram as it is (control).
- **H**: for t < t_tr, the ISM's specular arrivals up to order N, each W/(4 pi d^2) x prod(1 - alpha_i)(1 - s_i)
  x air attenuation over d (SPPS's own air absorption); for t >= t_tr, SPPS. Specular-only: a lower bound on
  early energy (the early scattered energy is dropped).
- **H'**: as H with prod(1 - alpha_i) only (scattered energy assumed to arrive at the specular time): the upper
  bound. A real hybrid (ODEON's early scattered rays) lies between H and H'.
- t_tr = sqrt(V) ms per room (CR2 ~12, CR3 ~58, CR4 ~93 ms). **Amended 22:55, before any EDT/C80 of any arm
  was computed:** counted from each pair's direct arrival, not from emission. Found at gate 3 on CR2: LS1-MP1's
  direct path is 4.44 m (12.9 ms), so a window from emission was empty and H equalled S by construction. The
  mixing time is a time after the direct sound (Lindau et al. 2012). N is as high as the ISM reaches in tractable time;
  the report states what fraction of order N+1 arrivals fall before t_tr (the completeness of the early part).

## Gates, in order (each stops the phase if it fails)

1. **Metric reproduction.** My echogram metric (Schroeder from the first nonzero bin, EDT 0..-10 dB, C80 at
   80 ms after the direct arrival) reproduces Night Mode's own SPPS EDT and C80 from `simpa results --json` within
   0.02 s and 0.1 dB on every pair and band. Otherwise the comparison is between two metric codes, not two models.
2. **ISM against an analytical reference.** In a shoebox, the ISM's arrival list (delays and energies, orders
   <= 3) equals the closed-form shoebox image lattice exactly (delay within 1e-9 s, energy within 1e-9 relative).
3. **Consistency with SPPS.** Order 0 (direct sound) energy: ISM and SPPS's first arrival bins within 1 dB
   (SPPS's receiver-volume estimator). The ISM's energy before t_tr never exceeds SPPS's by more than SPPS's
   Monte-Carlo scatter (an ISM above SPPS early would mean one of the two is wrong).

### Gate 3 amended, 2026-10-08 23:08, for CR4 only, before any CR4 gate-3 number existed (Burhan 23:07: "sure okay")

Gate 3(b) as written FAILED on CR3 (LS2-MP3, 125 Hz +0.24 dB against a 3-sigma tolerance of 0.09; 250 Hz +0.12
against 0.10). A sentinel seat found no image-source defect (219 arrivals, no duplicate lengths or plane
sequences) and traced it to the gate: the image sources put each arrival in one bin, SPPS's receiver sphere
(r = 1.0 m in CR3) spreads it over +-r/c, and about 8 arrivals sit within 2 bins of the hard window end, so SPPS's
window misses part of energy the image-source window counts in full; at 125-250 Hz CR3's scattering of 0.05-0.10
leaves H only just below SPPS, so the spill flips the sign. A residual of about 0.05 dB at 125 Hz stays
unexplained. **CR2 and CR3 stand as run: CR3 is a gate-3 FAIL with that diagnosis.**

For CR4, gate 3(b) is: the image-source energy of arm H over [0, n_hi) against SPPS's energy over
[0, n_hi + ceil(r/c / dt)), so SPPS's window holds every arrival the image-source window holds, smear included.
Pass: H never above SPPS by more than 3 sigma of SPPS's Monte-Carlo scatter (sigma_rel = 1/sqrt(N), N = window
energy / mean deposit per crossing), in any band 125-4000 Hz. Part (a) is unchanged.

### Gate 3c: CR3 with the scattering taken out, 2026-10-08 23:09, before the run (Burhan 23:07: "we still need to ensure that CR3 actually works man")

A FAIL with a diagnosis is not a working room. With every surface's scattering 0, SPPS is purely specular and must
reproduce the image sources themselves, so the comparison becomes two-sided. SPPS run: `bras_cr3.simpa` with
scattering 0 on every material, everything else as shipped except 10 M particles per source and 0.5 s duration
(`C:\tmp\nm-spps-projects\bras_cr3_s0.simpa`), receiver radius 0.31 m.

Per pair and band, cumulative early energy from emission to t, at t on a 1 ms grid from the direct arrival + r/c
to t_c, where t_c is 1 ms before the pair's earliest valid arrival of the image sources' top order (6), so every
arrival of order <= 5 before t_c is in the list. Image sources (orders <= 5, prod (1 - alpha) per reflection,
SPPS's air absorption, rho c = 413.25):

  E_SPPS(t - r/c) / (1 + 3 sigma) <= E_ISM(t) <= E_SPPS(t + r/c) x (1 + 3 sigma),  sigma = 1/sqrt(N(t))

Pass: at every t, pair and band 125-4000 Hz, both inequalities hold. Any order-6-or-higher arrival before t_c
would make the image-source energy fall short, which the left inequality then reports, not hides.
Also reported: each pair's energy ratio E_ISM/E_SPPS at t_c (the number expected near 0 dB).

### Gate 3d: specular SPPS in a shoebox against the closed-form lattice, 2026-10-08 23:59, before the run

Gate 3c failed on CR3 (1179 of 2724 checks; E_ISM/E_SPPS at t_c mean -0.19 dB, range -0.68..+0.39). Ruled out
since, each with a receipt in RESULT-5: absorption (spps-gpu applies (1 - alpha) before the specular/diffuse branch,
walk.h:612/634), geometry (TetGen's boundary = the .simpa to 1 mm at 20 000 points), ray recall (1 M rays add no
energy before t_c on LS1), time stepping (exact, walk.h:576/657). Left: the polyhedral image-source code, or SPPS's
sphere receiver itself. A shoebox separates them: its exact answer is the closed-form lattice, no polyhedral code.

Box 7.3 x 5.1 x 3.2 m, alpha 0.10 flat, scattering 0, LS1 (1.9, 2.2, 1.4), MP1-MP3, receiver radius 0.31 m,
10 M particles, 0.3 s (`C:\tmp\nm-spps-projects\shoebox_s0.simpa`). Same two-sided cumulative test as gate 3c, to
100 ms, against the lattice (all images to order 12). Reading: PASS means SPPS's specular part is right and CR3's
residual is in the polyhedral code or CR3's geometry; FAIL with the same signature as CR3 means it is SPPS's
receiver, and the bar for the image sources must allow it.

### Gate 3e: does SPPS converge onto the image sources as its receiver shrinks? 2026-10-09 00:46, before the run

Gate 3d (00:44): in a shoebox, specular SPPS matches the closed-form lattice to -0.025 dB mean at 100 ms (1588 of
1590 checks; the 2 misses, MP2 4000 Hz at 7-8 ms, are with a sentinel). So SPPS's receiver is not biased in
general. In CR3, the isolated reflections that disagree with the image sources have their reflection point within
0.15-0.21 m of a polygon edge (SPPS/ISM 1.18-1.51); those 1.5-2.5 m from an edge agree (0.94-1.05) (`edges.py`,
n = 7, suggestive only). Hypothesis: near an edge, a 0.31 m sphere receives part of a neighbouring image a point
receiver does not; the image sources answer for a point, which is what the hybrid's listener is.

Run: CR3, scattering 0, receiver radius 0.10 m, 100 M particles (crossings scale with r^2), 0.15 s
(`bras_cr3_s0_r01.simpa`). Same gate 3c test and the same t_c. **Pass (the image sources are right, the residual was
the receiver):** the mean |E_ISM/E_SPPS| at t_c over the 60 pair-bands falls by at least half from r = 0.31 m to
r = 0.10 m, and the per-reflection ratios of the edge-near reflections move toward 1. **Fail:** no such fall; the
residual is then in the polyhedral image-source code, and it is debugged before CR4.

**Amended 00:59, before the run started:** the r = 0.10 m run computes 500 Hz only (the residual is band-independent,
-0.18 to -0.21 dB band means at r = 0.31 m; six bands cost six times the GPU time, Burhan 00:59 "its gpu, so whys it
taking so long"). The pass rule compares the 10 pairs at 500 Hz against r = 0.31 m at 500 Hz (mean -0.197 dB,
range -0.644..+0.340, mean |ratio| 0.342 dB): pass needs mean |ratio| <= 0.171 dB at r = 0.10 m.

### Burhan's ruling, 2026-10-08 23:45

"welll the goal is to build a hybrid engine, so if we need fdtd and image and rays, thats what we do"

Reading: the image sources are a component of the hybrid engine (FDTD below the handover, image sources for the
early specular part above it, SPPS's rays for the scattered and late field), not an option this phase may reject.
The energy question below still gets its answer, but the image-source part's own bar is now that it is correct
(gate 3c: reflection by reflection against specular SPPS) and complete up to the transition time in every room.

### Burhan's ruling, 2026-10-08 23:48

"every aspect must use the GPU, if we are equipping I-Simpa with FDTD, IMAGE SOURCES AND RAY TRACING, WE MUST USE
THE GPU FOR THE HYBRID ENGINE"

Reading: a design rule for the hybrid engine (V2-15 and row 102), not only for this probe. PFFDTD (CUDA) and
spps-gpu already run on the GPU; the image-source method was NumPy on one CPU core until 23:48 and is being moved
to the GPU (CuPy here, a CUDA kernel in the engine), each port held to the CPU version's output on gate 2 and CR2.

## Outcome rule

The ISM early part **moves the parameters toward measurement** if, in both H and H', the mean absolute C80 error
over the 10 pairs x 3 rooms falls by >= 0.3 dB, and the mean EDT error by >= 2 points, in at least 3 of the 4
bands. If S lies between H and H' and neither arm meets that, the ISM's value is the auralization alone and the
C80/EDT gap belongs to the absorption and scattering data (RESULT-4's reading), not the early-part method.

## Forecast (written before any run)

Neither arm meets the rule. SPPS is an unbiased estimator of the specular plus scattered energy (it follows
specular paths with probability 1 - s at each hit), so its early energy in expectation sits between H and H';
the gap to measurement (SPPS C80 -1.0 to -2.9 dB, EDT +15 to +32 % in CR3/CR4 at 125-500 Hz, `compare_*.json`)
is the long late decay RESULT-4 traced to the GA-tuned absorption. Forecast: |C80(H') - C80(S)| < 0.3 dB and
S inside [H, H'] on C80 in >= 80 % of pair-bands. Transfer history: none (first ISM bed in this arc); a reasoned
forecast, not a measured one.
