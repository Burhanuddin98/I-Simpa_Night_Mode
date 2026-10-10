# Bed for patches 0001 + 0002 on TCR (2026-10-10 03:08)

BED-0001 and BED-0002 ran SPPS only; `classicalTheory.exe` links the same changed `lib_interface` and the patches
touch `src/ctr/TC_CalculationCore.cpp` and `src/ctr/input_output/reportmanager.cpp`. This closes that gap.

| Arm | Solver | Code sha |
|---|---|---|
| A | `C:\tmp\nm-m8a-solvers\classicalTheory.exe`, upstream 929a5c8 as shipped, unpatched | `fad4ab5d3f02…` |
| B | `C:\tmp\nm-solvers-gpu\classicalTheory.exe`, 929a5c8 + 0001 + 0002, as `solvers/manifest.json` pins it | `6a0ac763b852…` |

Input: the app's own TCR run on BRAS CR4 (`.out\v1q-p1b\cr4e\runs\20261009-095657-440-tcr\solve`: 5 point
receivers, 1 surface map, 1000 and 8000 Hz), copied into each arm's folder with `workingdirectory` pointed at it;
each arm run as the app runs it (`<exe> config.xml`, cwd the arm's folder). Both exit 0, stderr empty.

15 output files compared. The 6 `.gabe` files (Main results included): byte-identical. The 9 `.csbin` maps: decoded
through `simpa dump csbin` (nodes, faces, records, values), identical in all 9. Their raw sha256 differs in 8 of 9
(239-524 four-byte words each), outside the decoded fields; arm A run twice also differs from itself in 1-3 words of
the same files, so unpatched TCR writes unstable bytes there (most likely uninitialised struct padding; not proven).

**Verdict: every value TCR writes is identical with and without the patches.** The raw-byte difference is
upstream's own instability, not a patch effect. Receipt: `.out\v1q-gate\tcr-bed.txt` (gitignored).
