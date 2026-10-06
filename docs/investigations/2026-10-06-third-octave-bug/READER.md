# The app's results reader on a large run (2026-10-06, 17:16 to 18:xx)

Burhan, 17:16: "results are not showing, the acoustic results are not showing, it crashed wehn it has to
show all the results". 18:03: "WE NEED TO MAKE SURE THAT IT HAS APPROPRIATE MEMORY HANDLING, THE RESULTS
CAN EASILY SHOW EVEN THE DENSEST OF RESULTS".

The run: `.out/ui/cr4-27/CR4-27.simpa`, CR4 at 27 third-octave bands with the 0.1 m plane (43,930 cells),
written by the sparse solver (patch 0002, `BED-0002.md`) at 6.47 GB peak: 28 map files, 6.7 GB on disk,
about 700 million records (`IR-27.md`).

## Cause, at source

Reproduced 17:17:04 on the 16:05 app: Results step, "Checking this run's results..." for 90 s, working set
5.9 GB and commit 9.8 GB at 17:16:30, then gone with no Application Error event.

1. `crates/simpa-core/src/results.rs::load` read EVERY `.csbin` of the run in full and kept the decoded
   records (`SurfaceFile { data: Csbin }`) inside `RunResults`: 700 million records at 8 bytes plus a
   `Box` a face.
2. `app/src-tauri/src/results_data.rs::loaded` called `results::load` afresh for EVERY command, and the
   Results step issues `run_data`, `run_surface_map` and `run_report` together, so the run was decoded
   three times at once.
3. A map served to the viewport (`run_surface_map`, SMAP v1) carried every record: the Global file of
   this run is 149 million records, 1.2 GB as one ArrayBuffer in the WebView.

The solver is not the problem; its memory is bedded in BED-0001/0002.

## Part 1 (commit 9b98a45, 17:23): summaries, not records

`SurfaceFile` keeps `record_type`, `time_step_count`, `time_step`, `nodes` and per receiver `{id, name,
faces, records, value_sum}`; `read_surfaces` drops each file's data after summarising it;
`surface_map_bytes` reads the ONE file asked for and encodes it. REALISED: `simpa results` on CR4-27 went
from about 8 GB (and the app's death) to 114 s at 3.29 GB peak. The app built with part 1 alone still
reached 6.46 GB on the Results step (17:24): cause 2.

## Part 2 (this commit): one load per run, and a bound on a served map

- **Cache** (`results_data.rs`): the loaded runs by run folder, fingerprinted on `run.json`'s size and
  modification time (the solver writes it last). One slot lock per run: commands that arrive together
  wait for ONE load. A refusal is not kept. Bounded at 4 runs; the entries are summaries, so small.
- **Bound** (`MAP_RECORDS_MAX` = 32 Mi records = 256 MB of SMAP): a map past it is served at a coarser
  time bin, `csbin::read_binned(per_bin)`, which sums each face's energies per bin in `f64` in file
  order (the same sum the solver makes for its own bin, patch 0001) and scales the header's
  `time_step` by `per_bin`, `time_step_count` to the bin count. `bin_factor` picks the smallest factor
  with `faces * ceil(steps / factor) <= bound`, a guarantee however the records fall. Only
  `SplStandard` (energy) maps bin; TCR's fields are levels and are small. The viewport already draws a
  map at its header's bin (`resultsView.ts:247` stepRatio, `resultsLayer.ts:681`), so no UI change.
  Never refused.

Tests written: `csbin::binned_tests` (binned = dense summed per bin, bit for bit; header scaled; per_bin
1 is the dense read), `results_data::tests::{a_large_map_is_served_at_a_coarser_bin_within_the_bound,
a_run_is_loaded_once_and_again_when_its_manifest_changes}`. NOT RUN: cargo test is held until Burhan
names a target directory (C: filled twice today from `nm-target\debug`). The release build compiles them
out; the code they test compiled.

## Bed (the app on CR4-27, no clicks)

`bed-cr4-27.ps1`: `app.exe --project CR4-27.simpa` with `SIMPA_SOLVERS_DIR=C:\tmp\nm-solvers-timebin\bin`,
memory sampled every 5 s for 240 s. Log `C:\tmp\nm-bed-part2.log`.

| build | outcome | app peak working set | peak private |
|---|---|---|---|
| 16:05 (before) | died at 90 s on the Results step | 5.9 GB (commit 9.8 GB) | |
| 17:23 (part 1) | alive at 17:24 | 6.46 GB | |
| 18:11 (part 2, cache + record bound) | alive 213 s; Results "verified" in under 40 s; the Acoustics report shown; the 1000 Hz plane map REFUSED by the viewport ("87860 faces x 10000 steps is more than one 16384 x 16384 texture holds") | 1.41 GB (42 samples) | 1.63 GB; WebView tree 0.96 GB |
| 18:16 (part 2 + texel budget) | (fill from `nm-bed-part2.log`) | | |

The 18:11 refusal: the record bound did not bind (the 1000 Hz file fits 32 Mi records; it is sparse), but
the viewport's texture is dense, faces x steps, and 878 M texels is more than 16384² holds. So the UI now
passes a texel budget (`run_surface_map`'s `max_texels`: the GPU's texture side squared, at most 64 Mi,
`resultsView.ts texelBudget`) and `bin_factor` honours it: CR4-27's plane is served at 14 steps a bin,
87,860 x 715 = 63 M texels, 251 MB of texture.

## Traps paid in the bed

- **`cargo build --release -p app` makes a DEV-MODE binary**: without the `custom-protocol` feature the
  window loads `devUrl` (`http://localhost:5173`, the Vite dev server) and shows Edge's
  "localhost refused to connect" when none runs. The app is built the gates' way:
  `npx --no-install tauri build --no-bundle` in `app/` with `CARGO_TARGET_DIR` set (m10.ps1:340,
  m11.ps1:438); it runs `npm run build:ui` first and embeds `ui/dist`. The 18:05 bed measured an
  error page at 27 MB and proved nothing.
- Opening the project does not open the Results step; the bed drives it with `--e2e` and
  `window.__m10.setStep('results')` over WebView2's DevTools port
  (`WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=9223`; the websocket client must
  send no Origin header, or the port answers 403). No mouse: his window is never clicked.

## Still owed

- A windowed read: the viewport asking for a step range so the file's own bin is kept for any size (the
  bound now trades time resolution for size; the space resolution is kept).
- A streaming `.csbin` decode: `read_binned` still reads the whole file's bytes (1.1 GB for the Global
  file) before summing.
- Cause 1's cousin: `load` still decodes every map once to summarise and check it (114 s on CR4-27);
  a header-only pass with the finite check streamed would make the Results step open in seconds.
