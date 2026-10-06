# Band batching: the heavy runs work by running the solver once per band group (design, 2026-10-06 06:56)

Burhan, 06:55, verbatim: "FIX IT" (after the run-size refusal of 06:53). The refusal stops the crash; this makes the
run possible. Read with `MEMORY.md` (the cause and the sizing model) and `RESEARCH.md` is unrelated.

## The fact this design rests on (checked 06:55 on run `cr4-third/runs/20261006-051350-113-spps/solve`)

- SPPS writes its large outputs **per band, in their own folders**: `Surface receiver\<f> Hz\rs_cut.csbin` (19
  files for 18 bands plus one), `Intensity animation\<f> Hz\Intensity.rpi`, `Particles\<f>\particles.pbin` and the
  two collision CSVs. One band's files never mention another band.
- `config.xml` has `<freq_enum><bfreq freq="100" docalc="1"/>...` : one flag per band, which our export already
  writes from `solvers.spps.bands_computed`. Setting `docalc="0"` on a band is upstream's own way of not computing
  it. So a run on a subset of bands is a normal run with a different `bands_computed`; **no solver change**.
- The files that hold every band in one: `Punctual receivers\<MP>\Sound level.recp`, `Advanced sound level.gap`,
  `Punctual receiver intensity.gabe`, `Sound level per source.recps`, and the run-wide `Total energy.recp` and
  `SPPS particle statistics.gabe`. These are small (1-6 MB) and the core already parses them (`simpa dump
  recp|gabe`, `results.rs`). They are read per part, never rewritten.

## The design: a run made of parts, read across, never merged on disk

1. **Sizing and grouping (core, `run` manager).** With the result cube per band `c1 = cells x steps x sources x 4 B`
   (MEMORY.md) and a budget `B` (the machine's free memory minus headroom, from a `GlobalMemoryStatusEx` call; until
   then 8 GB), the bands computed are cut into consecutive groups of at most `floor(B / (c1 x factor))` bands,
   `factor` the solver's overhead above the float32 cube (unmeasured; start at 2, calibrate from the first batched
   run's peak working set, which the run manager can read from its Job Object accounting). A single band that does
   not fit is still refused (`RESULTS_TOO_BIG` keeps that case; the UI's refusal line then says "even one band").
2. **Execution.** One run folder as today, `run.json` gaining `parts: [{ bands: [..], folder: "part-1", status,
   elapsed_s, peak_bytes }]`. Mesh once (`mesh/` is shared). For each part: export `config.xml` with `docalc="1"`
   only on the part's bands into `part-<n>/solve/`, launch, read its lines as today (progress is `(done parts +
   this part's %) / parts`), verdict per part; a FAIL or CRASH in any part fails the run with that part's reasons;
   Cancel kills the current part and marks the rest cancelled. The same Job Object and loss checks per part.
3. **Reading results (core `results.rs`, `simpa results`).** The reader takes a run with `parts`: a band's surface
   map, intensity animation and particle file are looked up in the part that holds that band; the per-receiver
   files (`.recp`, `.gap`, `.gabe`, `.recps`) are read from every part and their band columns taken from the part
   that computed each band (the columns for bands with `docalc="0"` are empty or zero in that part and are
   ignored); `SPPS particle statistics.gabe` likewise. A run without `parts` reads exactly as today, so every
   existing run, gate fixture and bed is untouched.
4. **The UI.** Nothing new to learn: one run row, one progress bar (the LED meter), the Runs tab's detail showing
   "3 parts: 100-400 Hz, 500-1600 Hz, 2000-5000 Hz". The Simulate step's memory line changes from a refusal to
   "about 149 GB of results: run in 19 parts of about 8 GB" for a run that fits per band. The advisor (M12b) gets
   the same sentence.
5. **Receipts.** A bed: CR4-third at 18 bands, 0.5 m plane, run whole and run in 3 parts, every reported value
   (T20/T30/EDT per band and receiver, the maps' texels) identical bit for bit, since each band's computation is
   the same program on the same inputs with the same seed. Then the 0.1 m plane in parts, which has never run.
   Time: the sum of the parts should be close to the whole (the bands are serial inside SPPS); measure.

## What it does not do

A single band that is too big for the machine still cannot run: a 0.1 m plane over CR4-third is 8.3 GB per band in
float32 alone, so it runs only on a machine with room for that plus the solver's overhead. The deeper fix for that
case is solver-side streaming (a patch in `patches/`, writing each step's map as it is made), which is a change to
upstream's code and a pull request; not this design.

## Order of work

core: sizing and grouping (pure function, tested) -> parts in `run.json` and the executor -> the reader across
parts (tests on a hand-made two-part run folder built from the 05:13 run by moving per-band folders) -> the UI's
words -> the bed. The memory read (`GlobalMemoryStatusEx`) lands in the same change and replaces the UI's 24 GB
constant.
