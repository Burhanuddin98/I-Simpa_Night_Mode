# Upstream's tutorials 1-3 on this build: expected vs ours (parity A43)

2026-10-09, M12c P2. The three tutorial projects (`app/src-tauri/examples/tutorial_{1,2,3}.simpa`, built by
`build_tutorials.py` beside them) were opened in the release app (`C:\tmp\nm-target\release\app.exe`, built from
`330e215` with the tutorial files added, run as a copy in `C:\tmp\p2h-app`) and run there, through the Run button's
path, by the bed `.out\v1q-p2h\bed_p2h.py` (phases `runs` and `runs3`; logs `bed-runs.log`, `bed-runs3.log`).
Solvers: `C:\tmp\nm-solvers-gpu`, the verified build of `solvers/manifest.json`. Every run ended OK, every solver
stderr was read and was **0 bytes**, and every run's results passed the app's verification gate (Results panel
`verified`; `simpa results --json` on the app's own run folder, `.out\v1q-p2h\results-*.json`).

## What upstream's documents expect

Upstream's tutorial texts (`Docs/tutorial_*.rst`, tag `v1.4.0_snapshot_14_01_2026`) print **no acoustic value**.
The only quantitative expectations are two figures and one volume; the projects ship without results
(`src/isimpa/resources/doc/tutorial/tutorial {1,2,3}/`, checked in the checkout and in the 1.3.4 and 1.4.0 installers
under `target/investigate/release-binary/`).

| tutorial | upstream's expectation | receipt |
|---|---|---|
| 1 | none: the result kinds only (receiver levels direct/Sabine/Eyring, main results, parameters, maps) | `tutorial_teaching_room.rst:119-170` |
| 2 | R01's Global echogram and Schroeder curve, 1,000,000 particles per source; read off the figure: slope -32.83 dB/s over 0.2-1.0 s, i.e. 1.828 s for 60 dB | `tutorial_Elmia_hall.rst:175-179`, `images/Tutorial/tutorial_2_Results_Echogram.PNG`, digitised by `t2_compare.py` |
| 2 | the remeshed hall's volume, 9624 m3 | `tutorial_Elmia_hall.rst:80` |
| 3 | the cumulated Global difference map on the plane, absorbing diff_wall minus reference: legend -0.12 to -1.83 dB, contours -0.5 and -1.0 dB | `tutorial_industrial_hall.rst:433-450`, `images/Tutorial/Screenshot_6_tutorial_3.PNG` (legend rows 84-423, ticks at rows 160 and 259) |

## Ours, and the difference

| tutorial | configuration | ours | vs upstream |
|---|---|---|---|
| 1 | `tutorial_1.proj` as imported: 27 third-octave bands, SPPS 150,000 particles/source, 2 s, 10 ms, random; TCR; surface-receiver face 0.1 m2 | TCR run `C:\tmp\p2h\t1-20261009-171246\runs\20261009-171252-469-tcr`, SPPS run `...\20261009-171300-037-spps`: both OK, stderr 0 bytes, verified. TCR at 1 kHz: Sabine 0.667 s, Eyring 0.599 s (equal to the app's own classical formulas, `results-t1-tcr.json` `tcr.analytic`); SPPS Receiver 1 at 1 kHz: SPL 48.76 dB, EDT 0.661 s, T30 0.946 s, C80 6.87 dB | nothing to compare: upstream gives no value |
| 2 | `tutorial_2.proj` + rst:150-159: octaves 125 Hz-4 kHz, no scene correction, 0.005 s, Energetic, 100,000 particles/source, no per-band surface export; duration 10 s (the project's) | run `C:\tmp\p2h\t2-20261009-171246\runs\20261009-171316-472-spps`, OK, stderr 0 bytes, verified. R01, all bands and sources summed, Schroeder integrated to 1.5 s (upstream's plotted span): -33.10 dB/s, 1.813 s; integrated over the whole run: 1.821 s | **reproduced**: -0.015 s (-0.8 %) like for like, at a tenth of upstream's particles; the figure's own reading is good to a few per cent |
| 2 | the hall upstream's project carries (corrected geometry; no average-model remesh in this version) | air volume 10,389.1 m3 (`results-t2-spps.json` `room.volume_m3`) | **not reproduced, not comparable**: a different model; the remesh (G9) is v1.1 |
| 3 | `tutorial_3.proj` + the document: pink noise; zones 0.25/0.5 m and 0.15/0.3 m; both doors Open_door, open in all bands; SPPS per rst:334-373 (150,000 particles/source, 2 s, 0.002 s, extinction 5, Energetic, transmission, fittings, air absorption, SPL map, octaves 125 Hz-4 kHz); Receiver 1 moved 0.5 m off the wall | reference run `C:\tmp\p2h\t3-20261009-171723\runs\20261009-171727-526-spps`, then diff_wall set to Absorbing_material and saved, run `...\20261009-171915-042-spps`: both OK, stderr 0 bytes, verified. Cumulated difference over the plane's 1248 faces (`t3_compare.py`): min -1.887, p5 -1.500, median -0.916, p95 -0.464, max -0.175 dB; 6.1 % above -0.5 dB, 55.9 % between, 38.0 % below -1.0 dB; mean by 2 m of x: -1.21 at x 0-2, -0.66 at x 6-8 (the machines) | **reproduced**: range -0.18 to -1.89 against the legend's -0.12 to -1.83, within 0.06 dB at both ends; the same pattern (least around the machines, most at the x = 0 end) |

## What is not reproduced in the app, and why

- **The cumulated difference map is not drawn.** The Map card's difference is step by step over a time window
  (the bed's map: "Difference from Run 1 · all bands · averaged over 10 ms", legend ±44 dB); Cumulative is refused for
  a difference. The tutorial-3 comparison above is computed from the two runs' `rs_cut.csbin` files, which are what the
  app draws. `docs/v1.1-backlog.md` row 110.
- **Tutorial 3's comparison cannot be a variant.** The first `runs` phase ran it as a variant and the run was refused
  at export (`C:\tmp\p2h\t3-20261009-171246\runs\20261009-171553-616-spps\run.json`: `shared_solver_id`, ceiling and
  diff_wall share the pinned material id 21). The project ships without the variant; the tutorial text says to set
  the group's material, as upstream's text does. Row 109.
- **Tutorial 2's geometry steps** (import of the raw `elmia.ply` and Average model remesh, rst:48-96) are not in this
  version; the project carries upstream's corrected hall.

## Where the project and the document disagree (the document is followed)

Tutorial 2: R02 at y 10.2 in the project, 10.5 in the text (rst:108): the project is kept. SPPS: the project has
1,000,000 particles and 0.001 s; the text 100,000 and 0.005 s (rst:155-157): the text is followed.
Tutorial 3: white noise (text: pink, rst:63); zones 0.2/1.0 m and 0.0/1.0 m (text: rst:113, 136); door_room2 on
Absorbing_material and Open_door closed at 125 Hz (text: both doors Open_door, all bands, rst:197-245); SPPS 50,000
particles and one band (text: 150,000 and octaves, rst:359, 373). The receivers are the project's line (k-1, k, 1.6);
the text's grid table (rst:278-306) describes another.

## Reproduce

`python app/src-tauri/examples/build_tutorials.py <simpa.exe>`; the bed `.out\v1q-p2h\bed_p2h.py <app.exe> <solvers>
<out> <port> runs|runs3`; then `python t2_compare.py <results-t2-spps.json> <upstream png> t2-compare.json` and
`python t3_compare.py <simpa.exe> <reference run> <absorbing run> t3-compare.json` (both here, with their outputs).
