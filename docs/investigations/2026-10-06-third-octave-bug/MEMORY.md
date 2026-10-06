# The solver crashes (06:17, 06:22, 06:37, 06:38): a sound-level plane at 0.1 m, not the band count

Burhan, 06:39, verbatim: "HOW CAN ALL THE HEAVIEST FEATURES WORK WITHOUT KILLING THE FUCKING APP AGAIN AND AGAIN
BECAUSE OF FUCKING MEMORY ISSUES". Supersedes the 06:24 verdict ("27 bands") in `FINDINGS.md` and the handoff.

## What crashed, with receipts

`spps.exe` (upstream, `C:\tmp\nm-m8a-solvers\spps.exe` 1d9900db) aborted (Application Error 1000, 0xc0000409 = UCRT
abort) 15-18 s into each of these app runs; our `app.exe` never crashed. Windows Error Reporting failed with "the
paging file is too small" at 06:22:46 and `MicrosoftSecurityApp.exe` crashed in the same second each time:
casualties of the solver's allocation, not causes.

| Run | Project | Bands | Plane | Cells | Result cube, float32 | Outcome |
|---|---|---|---|---|---|---|
| 05:13 CLI | CR4-third (script) | 18 | Audience plane 23 x 19 m at 0.5 m | 1,794 | 2.4 GB | OK, 2 min 48 s |
| 06:01 app | CR4 | 6 | Audience plane at 0.1 m | about 26k | about 12 GB | OK |
| 06:17, 06:22 app | CR4 | 27 | Audience plane at 0.1 m | about 26k | about 56 GB | abort |
| 06:37:09 app | CR4-third | 18 | **Plane 1**, 33.1 x 33.3 m at **0.1 m** | **110,888** | **149 GB** | abort |
| 06:37:46 app | CR4-third | 9 (octaves) | Plane 1 at 0.1 m | 110,888 | 74 GB | abort |

Cube = cells x time steps (10 s / 1 ms = 10,000) x bands x sources (2) x 4 bytes AS FIRST WRITTEN; 07:30: SPPS
accumulates in `l_decimal` = `double` (upstream `coreString.h:43`), so every figure in the table is x2 (298 GB, 25 GB,
112 GB, 149 GB) and the 'fitted' row sat at about 25 GB under the 36 GB line. SPPS keeps the whole cube of the
surface receivers in memory for the run; the machine has 31.8 GB RAM and a page file of about 4.8 GB (commit limit
36.6 GB). The row that fitted and the rows that did not sit either side of that line; the exact peak was not
measured (no sampler ran), so the line is a forecast from the cube alone, not a realised number.

The 06:37 runs also carry a changed material (the 05:13 seating's absorption 0.22-0.4 is 0.05-0.1 in the 06:37
config), irrelevant to the crash.

## Why the app let it happen

- A new plane ("Plane 1") spans the whole room box at 0.1 m by default. For a 33 m hall that is 110,888 cells,
  60 times the plane that ran. Nothing says what it costs.
- The pre-run checks (`flow.ts` blockers, the Simulate checklist) know nothing about memory. The run-quality
  advisor (M12b) warns about particle counts and receiver sizes, not the result cube.
- The band presets multiply the cube silently: 6 -> 27 bands is 4.5x.

## What makes the heavy features work (the plan)

1. **Size every run before it launches.** Cube = (plane cells + surface-receiver faces) x steps x bands computed x
   sources x 4 bytes, times a solver factor to be calibrated by one sampled run. Show it on the Simulate step as
   words ("This run would hold about 149 GB of results in memory") and **refuse** above the machine's memory,
   **warn** above half of it. The machine's memory needs one small backend command (no system-info crate is in the
   app today); until then a stated ceiling.
2. **A plane's default cell size scales with the room**: about 40 cells along the longest side (0.8 m on CR4, 0.15 m
   on CR2), never 0.1 m on a hall; each plane's row shows its cell count and its share of the cube.
3. **Band batching** (the structural fix, later): the app runs the solver once per band group sized to fit, with
   the same mesh, and merges the per-band outputs into one run; SPPS already writes per-band files. Time is
   unchanged (the bands are serial inside SPPS too); memory is divided by the number of groups.
4. **Solver-side streaming** (a patch in `patches/`, later): write each step's map to disk instead of holding the
   cube. The deepest fix and the only one that lets a 0.1 m whole-room plane run at all.

1 and 2 are owed now; 3 and 4 are backlog rows with this file as their receipt.
