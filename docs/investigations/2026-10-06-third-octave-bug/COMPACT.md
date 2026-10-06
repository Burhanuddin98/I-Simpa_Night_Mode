# A compact surface-receiver format: bin the time axis in the solver (design, 2026-10-06 07:30)

Burhan, 07:28, verbatim: "THIS IS annoying becauyse its so low res it shouldnt cause issuers, find a better way of
storing the data in a super compact format". He is right about the cells: 110,888 is small. The cube is the time
axis: SPPS keeps a value for every cell at every particle time step (10 s at 1 ms = 10,000), per band, per source,
for the whole run and in the file. The map needs one sum per cell; the map's playback and the per-cell decay need a
few dozen to a few hundred bins, not ten thousand. Read with `MEMORY.md` (sizing) and `BATCHING.md` (the fallback
that needs no solver change).

## Where it lives in upstream (`B:\repos\I-Simpa-upstream\src\spps\input_output\reportmanager.cpp`, read-only)

- Line 557: `ReportManager::SetPostProcessCutSurfaceReceiver(coreConfig, freqIndex, surface_receiver_list, timeStep)`
  writes a cut plane's `rs_cut.csbin`; line 540 the same for surface-group receivers.
- Line 634: `t_Recepteur_P receiverData(nbfreqMax, params.nbTimeStep)`: every receiver holds `nbTimeStep` values per
  frequency; line 642 loops `idStep < params.nbTimeStep`. The energy is accumulated per particle step
  (`particleInfos.pasCourant`, line 163) into the cell's series.
- Lines 135-141: the output header (`enteteSortie`) carries `nbTimeStepMax` and `timeStep` itself, so a reader
  already takes the step count and length from the file, not from the project.
- The value type is `l_decimal`, which is `#define l_decimal double` (`src/lib_interface/coreString.h:43`): **8 bytes
  a value**, so every size in MEMORY.md written at 4 bytes doubles. CR4-third's 0.1 m plane at 18 bands is 298 GB
  in the solver; CR4 at 6 bands with its 0.1 m plane was about 25 GB, which is why it fitted under the 36 GB line
  and 27 bands (112 GB) did not. `runSize.ts` now counts 8 bytes.

## The patch (in `patches/` here, and a pull request upstream, per CLAUDE.md)

1. **A surface-receiver time bin, `recepteurs_surfaciques_pas_temps` (s)**, a new config value, default equal to the
   particle time step (so every existing project and every bed produce byte-identical output). The accumulation
   writes `energy[idStep / ratio]` with `ratio = round(bin / timeStep)`; `nbTimeStep` for the receivers becomes
   `ceil(nbTimeStep / ratio)`; the header's `timeStep` becomes the bin. Sums are unchanged (energy is additive);
   the per-cell decay is the same curve at coarser spacing. **Memory and file size divide by `ratio`**: 10 ms
   gives 10x, 50 ms 50x. CR4-third's 0.1 m whole-room plane: 149 GB (float32, 18 bands) -> 15 GB at 10 ms, 3 GB
   at 50 ms. Its map's playback goes from 10,000 frames to 1,000 or 200, which is still more than anyone watches.
2. **Sixteen-bit storage** (optional, a second step): write the cell series as `uint16` log-encoded (0.1 dB steps
   over a 96 dB span under the cell's own maximum, the maximum as one float32), halving or quartering the file
   again. This changes the csbin layout, so it is a new header version and the core's reader (`simpa dump csbin`,
   `results.rs`) follows; the m12 texel gate stays bit-exact because the map is built from the same decoded values
   on both sides. Only after 1 has landed and been measured.
3. **Our side.** `solvers.spps.map_time_step_s` in the project (default: the particle step), exported into
   `config.xml`; the Simulate step shows it beside the time step with the cube recomputed from it; the examples
   set 10 ms; the advisor suggests it when the cube is heavy; `runSize.ts` divides by the ratio.

## Receipts to produce

- A bed on CR4-third, 0.5 m plane, 18 bands: run at bin = step and at bin = 10 ms; per cell, the sum of the binned
  series equals the sum of the fine series bit for bit (float addition order is the same within a bin, so check
  to 1 ulp); T20/T30/EDT from point receivers unchanged (they are not surface receivers).
- The 0.1 m whole-room plane at 10 ms, which has never run: peak working set from the Job Object, and the time.
- Then BATCHING.md's grouping handles whatever is still too big; the two compose (bin first, then group).

## Order of work

Read `l_decimal`'s width and the csbin header writer (lines 700-800) -> the patch, built with the solver build
(`C:\tmp\nm-m8a-solvers` is a pinned build; the patch means a new verified build and manifest) -> the project
field and export -> the UI -> the bed. The pinned-solver rule means the shipped solver changes only with its
manifest re-verified; until then the examples ship with 0.5 m planes and the refusal stands.
