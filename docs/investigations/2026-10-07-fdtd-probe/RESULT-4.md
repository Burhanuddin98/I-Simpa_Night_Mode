# FDTD probe, phase 4: FDTD against Night Mode's own SPPS (2026-10-07 23:32, all realised)

> **CORRECTION 23:50 — every CR4 SPPS number below is void.** `bras_cr4.simpa` ships with the active variant
> "Absorbing panels", which swaps `mat_CR4_whitePanels` (1655.4 m2, 28 % of the room, alpha 0.082 at 125 Hz) for
> the seating absorber (0.150): +112.6 m2 of absorption area (sonar, `reference.rs:113,137,145`,
> `config_xml/write.rs:404-421`). `simpa run` exports the active variant, so SPPS simulated a room BRAS never
> measured; FDTD and my Eyring read the base materials, i.e. the measured room. My "Eyring disagrees on CR4" note
> is the same cause: the core's Eyring (variant) is right for its run, mine (base) for the measured room. The
> CR4 rows and the three-room means are being redone with `simpa run --base`; CR2 and CR3 have no variants and
> stand.

SPPS = the verified `spps-gpu.exe` from branch `zeph-build` (all five solvers match `solvers/manifest.json`'s
code sha256), run headless by `simpa run --solver spps --device gpu` on the shipped examples, read with
`simpa results --json`. CR3 and CR4 rerun as the run-quality advisor asked (receiver radius 0.31 -> 1.0 m,
1 M particles per source; copies in `C:\tmp\nm-spps-projects\`, the examples untouched); the advised runs moved
no T30 by more than 0.06 s, so particle noise was not the issue. FDTD = PFFDTD with the chamber-calibrated
boundary (`RESULT-3.md`). Driver `C:\tmp\nm-fdtd-probe\spps_compare.py`; tables `compare_CR{2,3,4}.md`.

## Mean per-pair error over CR2, CR3, CR4 (10 pairs each)

| band | T30 FDTD / SPPS / Eyring | EDT FDTD / SPPS / Eyring | C80 FDTD / SPPS / Eyring |
|---|---|---|---|
| 125 Hz | **9.0** / 15.1 / 5.0 % | **11.9** / 18.4 / 12.7 % | 1.61 / **1.40** / 1.52 dB |
| 250 Hz | **6.5** / 14.8 / 8.2 % | **9.8** / 14.8 / 8.2 % | 1.62 / **1.46** / 1.63 dB |

Per room and metric, FDTD beats SPPS in 6 of 9 cells at 125 Hz and 5 of 9 at 250 Hz. At 500 Hz only CR2 has an
FDTD grid (fmax 1000 Hz): FDTD beats SPPS on T30 (6.8 vs 9.2 %) and EDT (8.3 vs 9.4 %), not on C80.

Eyring has a home advantage in every column: its inputs are BRAS's estimates fitted to these measurements, and
it is one number per room. FDTD's boundary was calibrated in a virtual chamber with no BRAS data.

## Findings

1. **Hybrid split: FDTD owns the low bands.** It beats SPPS on decay (T30, EDT) at 125 and 250 Hz by 6-8 points
   of mean error. Handover after the 250 Hz octave (FDTD fmax 400 Hz, the grid all of phase 2-3 ran on); 500 Hz
   is supported by one room only and costs ~16x the GPU.
2. **C80 is no engine's strength.** SPPS is 0.15-0.2 dB better than FDTD on C80, and every engine misses the
   1 dB target in most room-bands. C80 is the early/late energy balance: the part an image-source early field
   addresses (Burhan 22:45: "maybe even Image sources method might be useful").
3. **SPPS overshoots CR3 by 26-32 % on T30 in every band** (125, 250, 500 Hz), at default and at advised
   settings, while Eyring on the same materials is within 2.5-5 %. A Night Mode correctness question in its own
   right, independent of FDTD: SPPS's bed has never met a measured room (its own results JSON: "No comparison
   with a measured room (V2-9)"). Not investigated.
4. SPPS on CR4 is the reverse of CR3: T30 right at 125 Hz (2.8 %), short at 250 and 500 Hz (-11 %, -19 %).

## Finding 3 followed up, 23:43 (Burhan 23:39: "i agree" to fixing SPPS on CR3 first)

- SPPS's room is the right room: volume 3330.6 m3 (3334.5 less 1.96 m3 of obstacles), area 2763.0 m2, both as
  the mesh gives; particles lost 0.02-0.04 % per band. SPPS's own Eyring reference is 1.65 s at 125 Hz
  (measured 1.64 s): SPPS's 2.13 s is 29 % above its own reference.
- CR3's scattering is 0.05-0.10 on 94 % of the area (seating 0.21-0.30 on 6.5 %). **Every surface's scattering
  set to 1** (`C:\tmp\nm-spps-projects\bras_cr3_s1.simpa`, default 300 k particles): T30 2.13 -> 1.93 s (125 Hz),
  1.83 -> 1.60 s (250), 1.71 -> 1.46 s (500); still +18 / +11 / +13 % against measured, and against SPPS's own
  Kuttruff reference (exact for scattering 1) +11.5 / +1.4 / +7 %.
- Reading: about half of the excess is the low scattering keeping specular paths off the absorbing seats; the
  rest, at 125 and 500 Hz, is unexplained. Not yet run: the same two arms on CR2 and CR4 (does SPPS sit above
  Kuttruff there too?), and SPPS's CPU build against spps-gpu on CR3.
- Side note: my Eyring for CR4 (2.35 s at 125 Hz) disagrees with SPPS's (1.92 s) while CR2 and CR3 agree to
  0.01 s. One of the two computations has a fault on CR4 (double-sided or grouped faces are the suspects).
  The CR4 "Eyring" columns above are mine and are unconfirmed.

## Corrected with the base CR4 room, 2026-10-08 00:08 (Burhan 23:48: "proceed")

CR4 SPPS rerun with `simpa run --base` (300 k particles, as shipped): `C:\tmp\nm-spps-runs\cr4_base.json`.
SPPS's own Eyring on the base room is 2.32 s at 125 Hz (mean alpha 0.0971), matching mine (2.35 s): **my CR4
Eyring columns were right for the measured room**; the 23:43 side note's disagreement was the variant alone.

Mean per-pair error over CR2, CR3 (advised), CR4 (base), realised:

| band | T30 FDTD / SPPS / Eyring | EDT FDTD / SPPS / Eyring | C80 FDTD / SPPS / Eyring |
|---|---|---|---|
| 125 Hz | **9.0** / 19.4 / 5.0 % | **11.9** / 23.1 / 12.7 % | 1.61 / **1.52** / 1.52 dB |
| 250 Hz | **6.5** / 17.5 / 8.2 % | **9.8** / 17.0 / 8.2 % | 1.62 / **1.34** / 1.63 dB |

FDTD beats SPPS in 7 of 9 room-metric cells at 125 Hz and 5 of 9 at 250 Hz; SPPS keeps C80 in 5 of 6 room-bands.

**SPPS runs long on every BRAS room, and above its own exact reference by an amount that grows with the room.**
With every surface's scattering at 1 (the case Kuttruff's corrected Eyring describes exactly), SPPS T30 against
that reference: CR2 (147 m3) +3 / +2 / +1 %, CR3 (3331 m3) +11.5 / +1.4 / +7 %, CR4 (8657 m3) +16 / +13 / +15 %
(125 / 250 / 500 Hz). The shipped low scattering (0.05-0.10) adds 7-14 % in CR2 and 15-17 % in CR3 on top; in CR4
scattering 1 lengthens SPPS slightly (2.81 -> 2.87 s at 125 Hz). Not explained tonight. Candidates to test next:
the reference itself on large non-convex rooms (CR4: seating rake, 39 m3 of obstacles), SPPS-CPU against
spps-gpu on CR4, and the advisor's open `run_too_short` on CR4.

## SPPS's long decay explained, 2026-10-08 00:22 (Burhan 00:13: "you do whatever you need")

Three cuts, all realised, all on scattering 1 (the fully diffuse case):

1. **Not Night Mode's parameter code.** Its t30_s, T30 recomputed from the same raw per-band echograms
   (`spps_decay_check.py`) and the room-wide energy decay agree within 0.04 s in CR2, CR3, CR4.
2. **Not the GPU port.** The verified CPU `spps.exe` on CR4 (base): 2.866 / 2.919 / 2.581 s against spps-gpu's
   2.868 / 2.903 / 2.581 s.
3. **Not SPPS: the reference.** An independent energy ray tracer written from scratch (`vtracer.py`: same mesh,
   base materials, SPPS's air absorption, Lambert walls, room-wide decay, 0 rays lost) gives, at 125/250/500 Hz,
   CR2 1.41/1.54/2.06 s (SPPS 1.41/1.54/2.05, Kuttruff 1.37/1.51/2.03), CR3 1.93/1.69/1.46 s (SPPS
   1.93/1.60/1.46, Kuttruff 1.73/1.58/1.37), CR4 2.79/2.82/2.45 s (SPPS 2.87/2.90/2.58, Kuttruff 2.48/2.56/2.25).
   The tracer agrees with SPPS within 0-6 %; both sit 10-13 % above Kuttruff's corrected Eyring in the two large
   rooms.

**Reading.** Upstream SPPS solves the diffuse transport these rooms define; Kuttruff's formula, Night Mode's
M8 T30 reference, undershoots in large non-uniform rooms (its own bed note: "The T30 stand-in range is not
calibrated (backlog 65)"). SPPS's 8-30 % excess against the measurements is then the model, not the code: BRAS's
fitted absorption was tuned until RAVEN (image sources + ray tracing with its own scattering) matched these
rooms, and with that absorption the diffuse-transport physics decays more slowly than the real rooms. Eyring
lands close to the measurements partly because the fitted absorption was pushed to where it does.

Consequences:
- **No SPPS code fix is indicated.** Above the 250 Hz handover, the hybrid's accuracy on real rooms depends on
  absorption data meant for this engine, not on SPPS.
- **For Night Mode's M8 bed (backlog 1 and 65):** the Kuttruff reference is 10-13 % short on CR3/CR4-class rooms
  with diffuse walls. An exact transport reference (the tracer above is one) is a better bar for T30 there.
- FDTD's low-band win stands on the same inputs: with GA-tuned absorption it still beats SPPS on T30/EDT at
  125/250 Hz, through a boundary calibrated without any BRAS data.
