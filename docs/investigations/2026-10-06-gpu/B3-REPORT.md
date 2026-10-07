# B3: the running GPU solve on screen (2026-10-07 10:09-11:00, Grace)

**Done-when holds: yes, with one limit on the cost.** "The Simulate step shows the running solve; the cost to
the solve is measured." The release `app.exe` ran CR4-27 (27 bands, 2 x 300,000 particles, 10 s) on the RTX 5070
from the Simulate step's Run button, and the 3D view filled with the run's saved particles band by band while it
computed, under the caption "live: the saved sample of particles, N of M so far, band X Hz" (screenshots below).
When the run ended the layer cleared and the Results step played the run's particles as before (400 particles,
915,076 records at 1 kHz, `m12Particles.rendered` 400). The cost: writing the stream takes **0.12-0.32 s of a
77-123 s CR4-27 solve** (10,800 frames, 345 MB) and **3 ms of a 1.2 s CR4 1 kHz solve**, measured inside the
solver; once, under disk contention, the flushes took 9.1 s. **The limit:** the wall-clock difference between
stream on and stream off is not resolvable on this machine today: CR4-27 runs of the same configuration spread
from 77 to 133 s with the stream on *or* off (the output writing, 7.3 GB a run on `B:`, is what varies; the
kernel is 25.7-30.1 s in every run), and other agents were building on Grace during both sets.

**What it means for Burhan.** Press Run with "SPPS on the GPU" chosen and the room fills with light as the bands
compute: the particles the project saves for playback ("particles saved for playback", 200 per source here),
each drawn as soon as the solver has traced it, in Dots, Glow or Rays (chosen on the caption). It is the saved
sample, not every particle: 400 of 600,000 per band, the same ones the Results step replays. On a 1 kHz-only CR4
run the solve is over in about a second, so the live view is a flash; on a 27-band run it plays for the whole
run (80-185 s here). The solver's results are unchanged, bit for bit (below).

Branch `b3` (from `gpu` at `6da3723`), commits `1e9c29b`, `44d229c`, `6a878e8` and this report. Not pushed.

## The cost (REALISED)

Configuration of every row: Grace (i7-14700KF, RTX 5070 12 GB, driver/runtime CUDA 13.2), `spps-gpu.exe`
0.1.2 (B3) fat build `C:\tmp\nm-b3\bin\spps-gpu.exe` (sha256 `dec86e7f...`), run directly on staged solve
folders (`solvers/spps-gpu/bed/stream_cost.py`: ROOT `workingdirectory` rewritten to the copy, refused
otherwise), outputs on `B:` (NVMe, exFAT) under `.out\b3\`, arms alternated off, on, off, on, off, on, then
one run with the stream kept. Stream on = the default (written, flushed per frame, removed at the end). Wall =
measured by the bed around the process; solver = `spps-gpu.json` `wall_seconds`; stream = `stream_seconds`
(time inside the frame writes and flushes). stderr empty and exit 0 in every run.

**CR4, 1 kHz only, 2 x 300,000, 10 s at 1 ms, seed 101, 200 saved per source** (`.out\a5\cr4-1k` run
`20261007-091702-499-spps`'s solve folder; receipt `.out\b3\cost-cr4-1k\cost.jsonl`):

| arm | run 1 wall / solver s | run 2 | run 3 | mean wall | stream s | frames, bytes |
|---|---|---|---|---|---|---|
| off | 1.235 / 1.201 | 1.175 / 1.146 | 1.165 / 1.135 | 1.192 | 0 | 0 |
| on | 1.195 / 1.167 | 1.193 / 1.165 | 1.220 / 1.188 | 1.203 | 0.0033 / 0.0032 / 0.0030 | 400, 7,300,156 |

On minus off: +0.011 s mean wall, inside the run-to-run spread (0.07 s). The stream's own time is 3 ms.

**CR4-27, 27 bands, 2 x 300,000, 10 s at 1 ms, seed 101 (set in the copy; the project has 0), 200 saved per
source** (`.out\ui\cr4-27\runs\20261006-193146-930-spps\solve`, the project's 19:31 run; large outputs pruned
after hashing):

| set | arm | run 1 wall / solver s | run 2 | run 3 | median wall | stream s (1/2/3) |
|---|---|---|---|---|---|---|
| A (10:17-10:29) | off | 77.37 / 77.22 | 78.75 / 78.66 | 89.74 / 89.61 | 78.75 | 0 |
| A | on | 86.63 / 86.52 | 103.98 / 103.89 | 104.74 / 104.63 | 103.98 | 0.122 / 0.124 / 0.126 |
| A | keep | 77.79 / 77.70 | | | | 0.130 |
| B (10:29-10:44) | off | 96.12 / 96.01 | 121.73 / 121.60 | 113.25 / 113.13 | 113.25 | 0 |
| B | on | 98.47 / 98.35 | 122.65 / 122.56 | 111.47 / 111.37 | 111.47 | 0.123 / 0.320 / **9.095** |
| B | keep | 132.76 / 132.60 | | | | 1.672 |

Every run: 10,800 frames, 345,360,596 bytes in the stream. Kernel 25.7-30.1 s and re-trace 8.6-26.1 s in all
arms; what moves is the band output (18.7-44.6 s) and the finish (5-27 s), the disk. Set A's "on" runs 2 and 3
overlapped my own `tsc` and test runs on `B:` (the worktree is on `B:`), which is why set B was run with this
session idle; during set B another agent's `rustc` was running (seen at 10:44), and the spread grew. **Reading:**
the stream's attributable cost is its write-and-flush time, 0.12-0.32 s per CR4-27 run when the disk keeps up,
9.1 s once when it did not (the per-frame flush waits on a write-back queue holding the run's 7 GB of maps); the
on/off wall difference (set A +25 s median, set B -1.8 s) is noise of the shared disk, not a measurement of the
stream. A clean A/B needs a machine with no other writers. Receipts: `.out\b3\cost-cr4-27\cost.jsonl`,
`.out\b3\cost-cr4-27-quiet\cost.jsonl`, each run's `spps-gpu.json` and `hashes.json`.

**In the app** (`tools/devtools/bed-b3.py`, receipt `.out\b3\app-bed-cr4-27\app-bed.json`): the CR4-27 run
from the Run button took 184.5 s (an earlier pass, `app-bed-cr4-27-try1`, 82.3 s: same run, the disk again).
The page drew 60.5, 60.3 and 60.6 frames a second at the three moments (rAF over one second). Each table
rebuild of the live layer (at most every 250 ms) took 77-113 ms of the page's main thread, for about 800,000 to
1,000,000 records. CR4 1 kHz in the app (`.out\b3\app-bed-cr4-1k`): the run took 3.5 s from the click, of
which the stream's 400 particles arrived inside about 0.3 s at its end; one 200 ms sample caught it live (62 of
400), and the first screenshot, 0.6 s later, was already after the run.

## Bit-identity with the stream on

1. **Stream on against stream off, same build, same seed** (`solvers/spps-gpu/bed/stream_identity.py`, CR4 1 kHz,
   `.out\b3\identity-cr4-1k\identity.json`): the walk dumps (every primary particle's step count, fate,
   final energy bits, children) are **identical, 0 mismatches in 600,000 particles, 681,746,674 steps**, on
   against off and off against off. Of 40 output files, **38 are byte-identical**; the 2 that differ are
   `Surface receiver/1000 Hz/rs_cut.csbin` and `Global/rs_cut.csbin`, which differ between two stream-off runs
   too (the cut cells are float atomics, their add order changes every GPU run; max 5.5e-9 of the peak in both
   pairs). The `.pbin`, both particle CSVs, every `.recp`/`.gabe`/`.gap`, the stats: identical.
2. **The cost runs:** in all 14 CR4-27 runs (12 compared with their set's first stream-off run) and the 6 CR4
   runs compared, every output file but the `rs_cut.csbin` files is byte-identical to that first stream-off run (170 files a CR4-27 run, 160 identical, the 10 `rs_cut` differing
   in every run, off or on).
3. **The stream against the `.pbin` files**, frame by frame (`stream_cost.py` `check_stream`, the kept runs):
   CR4-27 10,800 frames = 27 bands x 400 particles, 21,571,529 records, every position and energy bit for bit,
   every first step equal, in computation order, no extra frame; CR4 1 kHz 400 frames, 455,758 records.
4. **A2's CPU-against-GPU check with this build, stream on by default** (`bed/cpu_gpu_all.py`,
   `.out\b3\a2-identity\summary.jsonl`): **17 of 17 cases walk-identical, double sums within 1e-12 in all**.
   The stream ran in the 2 cases that save particles (`outputs`: 16 frames; CR4 1 kHz: 400 frames, CPU and GPU
   builds); the other 15 save none, so there is nothing to stream.
5. **The `vs_spps` comparison** (`bed/vs_spps.py cr4-1k gpu 101`, through `simpa run-folder`, verdict OK,
   `.out\b3\vs_spps\`): the run's 40 files against A2's GPU seed-101 run (`.out\spps-gpu\bed\runs\cr4-1k\
   20261006-214149-792-spps`, the build of `8f9f891`): **38 byte-identical**, the 2 `rs_cut.csbin` differing as
   above. Against SPPS (`bed/analyse.py`, A2's SPPS runs): **54 of 55 within the noise, worst |d| / range 1.14**,
   the same figures as A2's; SPPS against itself 54 of 55.

## What was built

| file | what |
|---|---|
| `solvers/spps-gpu/src/output.h`, `output.cpp` | `LiveStream`: `spps-gpu.pstream` in the working directory. Header: magic `PSTM`, version 1, time step, step count, particles saved per band, the computed bands. Frame: u32 length, band Hz, the particle's index in its `.pbin`, its first step (the `.pbin`'s u16), n, f32 x 3n positions, f32 x n energies; written in one call and flushed. `ParticleFiles::save` writes each saved particle to it as it writes the `.pbin` |
| `solvers/spps-gpu/src/main.cu` | On by default when particles are saved; `--stream off\|on\|keep` or `SPPS_GPU_STREAM=0\|1\|keep` (`simpa run-folder` passes only the path); removed at the end unless kept; `spps-gpu.json` gains `stream`, `stream_frames`, `stream_bytes`, `stream_seconds`; version 0.1.2 |
| `solvers/spps-gpu/bed/stream_cost.py`, `stream_identity.py` | The cost bed (alternated arms, hashes, the stream checked against the `.pbin` files, `--prune`) and the on/off/off identity bed |
| `solvers/spps-gpu/bed/stage.py`, `cpu_gpu.py`, `vs_spps.py` | Their output roots, executable and paths overridable (`SPPS_GPU_BED_*`), so another step's bed stays in its own folders |
| `app/src-tauri/src/live.rs` (new) | The decoder (whole frames out, a torn tail held, a wrong magic/version/length refused), `encode_batch` (a 32-byte LIVE v1 envelope: so far, total, band position, bands; then PART v1 through `results_data::encode_particles`), `spawn_tail` (polls every 100 ms, one batch per band present, stops within 10 ms of being told) |
| `app/src-tauri/src/runs.rs`, `commands.rs` | `run_start` takes a second channel, `on_live` (raw bytes, an ArrayBuffer in JS); a run on the GPU tails `solve/spps-gpu.pstream` from `started`, and the tail is stopped and joined before the last event, so nothing live follows `ended`. CPU and TCR runs never touch it |
| `app/ui/src/features/viewport/live.ts` (new) | `decodeLiveBatch`, `LiveSet` (the live clock: starts at the first batch at its earliest first step, 0.05 of real time; a late trajectory starts from its first record at the clock; at most 1,000,000 records held, oldest arrivals let go), `liveCaption` |
| `liveView.ts` (new) | The controller: batches in, table rebuilds at most every 250 ms, the clock from animation frames, cleared at the run's end; hook `liveView` |
| `engine.ts`, `rays.ts` | A second `ResultsLayer`, `live`, drawn on the Simulate step only while it holds particles, in the Results step's looks and light pass (the room dims and ghosts as in B2); the Results step's own layer is never touched; the warm ramp's span is claimed by whichever layer is shown |
| `Viewport.tsx`, `viewport.css` | The caption under the view bar, a pulsing red dot, and Dots / Glow / Rays for the live layer |
| `actions.ts`, `backend.ts` | The live channel made for every run; `liveSink` begin/batch/end (registered by `liveView.ts`, so `actions.ts` imports no viewport code) |
| `tools/devtools/bed-b3.py` | The app bed: a project run from the Run button on the GPU, the live layer sampled every 200 ms, three screenshots, frame rate while live, the layer cleared, the Results step after |
| `docs/decision-log.md` | Decision 71 |

**Pinned in `solvers/manifest.json` (for the app bed; the re-pin at merge is the main thread's):** `spps-gpu.exe`
sha256 `dec86e7fc0039db5d520ecf864f7db645449bf34b6477e47a0c6070012189caf`, code sha256
`0d3fa207d7733f55ea201b9f13be17d4b6119070dd5c3be4c4ae44fe3248178d` (`tools/fixture-gen/pe_fingerprint.py`); the
`spps_gpu` record: source commit `1e9c29b`, source tree `c575992`, built from branch `b3`, `build.cmd
C:\tmp\nm-b3\bin fat`, built 10:15; no build log on disk (the console said nvcc 13.2 V13.2.78, warnings #177 only).
The app's solvers folder `C:\tmp\nm-solvers-b3` holds A5's four upstream executables and this `spps-gpu.exe`;
every app run read "solver build verified".

**Tests:** Rust `app` crate 94 of 94 (6 new: the decoder at every cut and byte by byte, the refusals, the
batch layout, the tail on a growing file, the run thread's tail stopped before the last event); UI `npm test`
335 of 335 (5 new: the batch decode, the clock, the late start, the record budget, the caption); `npm run
typecheck` clean. The m10/m11/m12 e2e gates were not run (the spec's; the main thread's).

## Screenshots (`.out\b3\app-bed-cr4-27\`, Glow, the default camera, 1440 x 900)

- `b3-live-1-early.png`, 3.4 s after the click: 301 of 10,800, band 50 Hz, the first band's particles leaving
  LS1 and LS2 across the audience.
- `b3-live-2-middle.png`, 81.9 s: 5,600 of 10,800, band 1000 Hz: the low bands' particles spread through the
  hall, the new band's bursting from the sources.
- `b3-live-3-late.png`, 129.6 s: 10,000 of 10,800, band 12500 Hz: the hall full of light.
- `b3-after-run.png` (the layer cleared, caption gone), `b3-results-after.png` (the Results step, as before).
- The first pass, with the caption under the view bar at this width: `.out\b3\app-bed-cr4-27-try1\`; CR4 1 kHz
  in Rays: `.out\b3\app-bed-cr4-1k\` (the run had ended by the first screenshot).

## What is untested

- **Zeph** (RTX 2060, 6 GB): not run. The stream is host-side and the live layer is the B2 draw, so nothing in
  B3 is new for the device, but no frame rate or rebuild time was measured there.
- **The WebGL2 fallback**: the live layer uses the same looks, so Rays is refused there and Glow draws per record;
  not run.
- **Cancel during a live run**, and **a step change during one** (the layer is hidden off the Simulate step and
  comes back): the code paths exist (`ended`/`failed`/`last` all clear it); not driven in the app.
- **A cancelled or crashed solver leaves `spps-gpu.pstream` in `solve/`** (it is removed only at a normal end):
  345 MB on CR4-27. Not tested, not cleaned up by the app.
- **The record budget drops trajectories:** at CR4-27's low bands a band's 400 particles hold about 800,000
  records, so the layer keeps about 300-440 of them and lets the older arrivals go (5,160 let go by the middle
  screenshot); the caption counts arrivals, not what is drawn. `liveView` reports both.
- The live clock runs at 0.05 of real time and is not on the timeline card; 50 ms of sound a second was chosen,
  not asked for.
- A stream on the CPU backend (`--cpu`, `SPPS_GPU_BACKEND=cpu`) is written too, but the app tails only GPU runs.

## The v2 road: a time-major walk (not built)

**What it changes.** Today `kSlots` (`main.cu`) is slot-major: 49,152 resident slots, each walks one family for
up to `budget` steps per launch (sized for ~250 ms launches: 5 launches per band on CR4 1 kHz, 185-202 for
CR4-27's 27 bands), then takes the next family. A time-major walk keeps **every** particle of the band resident
and advances all of them by one chunk of k steps per launch, so after each launch the whole field is at one
time t and a frame (any sample of positions) can be copied out. In `walk.h` the `Walker::advance(p, budget)`
already stops a particle at a step budget, so the step logic stays; what changes is the driver: a particle
array of N per band instead of slots, a launch per chunk, compaction of dead particles (or the GPU idles on
them late in the decay), and the transmission children, which today live in a 16-deep queue per slot, need a
shared child pool (one queue per particle would be 2 KB each). The accumulators do not change.

**Memory.** `Particle` is 128 bytes (V3 x 3, 2 doubles, 9 ints, the 40-byte Philox state). Resident state for
2 x 300,000 particles: **77 MB**; CR4-27 computes its bands one after the other, so it needs the same 77 MB per
band (all 27 resident at once would be 2.07 GB of state, and their dense maps 27 x 1.76 GB, which is the
reason not to). A child pool of, say, 1 M particles is 128 MB. The band's dense maps dominate: CR4's plane is
1.76 GB a band. So a time-major band fits on **6 GB (Zeph) and 12 GB (Grace)** alike: about 2.0 GB per band
with the maps, against today's 1.76 GB maps plus 49,152 x (176 + 2,048) bytes = 109 MB of slots and queues.

**Cost, forecast from the launch overhead measured here.** One launch round trip of the host loop (memset,
launch, event sync, two small copies back, an empty kernel of the same 49,152-thread grid,
`C:\tmp\nm-b3\launch\launch.cu`, 3 x 10,000 launches): **63-76 us** on Grace (shared machine). At one step per
launch, 10,000 steps cost 0.63-0.76 s of overhead per band: on CR4 1 kHz that is about the whole kernel
(0.61 s) again, and on CR4-27 17-21 s on top of a 25.7 s kernel. At 10 steps per launch (a frame every 10 ms
of sound, 100 frames a second of sound) the overhead falls to 0.07 s a band (2 s for CR4-27), and a frame of
4,096 sampled positions is 64 kB to copy. The larger unknown is occupancy: a time-major launch runs every live
particle for k steps, so late in the decay most threads are idle unless the array is compacted every few
launches, which the slot-major scheme avoids by construction. Forecast: **a frame every 10 ms of sound for
roughly +10 % on a 27-band CR4 solve, if compaction is cheap; measured, not assumed, before it is built.**

## Fixes after audit (2026-10-07 11:02-11:07)

The audit's verdict was SHIP-WITH-FIXES. Three fixes, each its own commit; `main.cu` unchanged, so `spps-gpu.exe`
was neither rebuilt nor re-pinned.

1. **A stale stream (`cd81f36`).** The tail could open an old `spps-gpu.pstream` (a reused folder, or any
   order where one exists before the solver truncates it) and keep its header and its offset into the new
   file. Now `spawn_tail` opens the path afresh at every poll and checks it is still the stream it has read:
   not shorter than what was read and the same first 64 bytes (header and the start of the first frame).
   When it is not, the decoder, the offset and the count start again from the file's first byte
   (`TailStats.resets`). When the path is gone (the solver's removal) the last handle keeps reading. Test
   `live::tests::a_replaced_or_truncated_stream_is_read_again_from_its_start`: an old stream present at
   the start, then replaced (deleted and written with another header and a shorter frame), then truncated
   in place and rewritten: batches of bands 500, 1000, 2000 in that order, counts 3, 1, 1, 2 resets;
   passed 6 runs of 6.
2. **The stream left by a cancelled or crashed solve (`9222b31`).** `run_thread` removes
   `solve/spps-gpu.pstream` once the tail has stopped (and a stale one at `started`, before the solver
   runs), unless `SPPS_GPU_STREAM=keep` is set in the environment the app and its solver share; nothing
   else in the folder is touched. Unit test: `runs::tests::a_live_run_tails_its_stream_and_stops_before_the_last_event`
   now starts with a stale stream and a `config.xml` beside it and ends with no stream and `config.xml`
   byte for byte. In the app (`tools/devtools/bed-b3-cancel.py`, receipt
   `.out\b3\audit-cancel-2\cancel-bed.json`): CR4 1 kHz on the GPU, cancelled 0.35 s into the solve stage
   while the stream existed (its 28-byte header written); the run read CANCELLED, `spps-gpu.exe` with its
   device line; afterwards no `spps-gpu.pstream`, `solve\` holds `config.xml`, `mesh.cbin`,
   `tetramesh.mbin`, and the run folder `mesh`, `run.json`, `solve`, `solver.stderr.txt`,
   `solver.stdout.txt`; the live layer cleared. (`.out\b3\audit-cancel\` is a first try in which the GPU
   entry had not been chosen yet when Run was pressed: a CPU SPPS run, no stream; the bed now refuses that.)
3. **A step change during a live run (`d0708d2`).** Defined and tested: the run's batches are **kept** on any
   step (no frame is dropped; the caption's count follows them); the live layer's tables are rebuilt only
   while the Simulate step is shown, and at once when it is shown again, with everything that arrived
   meanwhile and the clock at wall-clock now; the run's end (`ended`, `failed` or the stream's last batch)
   clears the layer whatever step is shown, once, and nothing of that run is kept after it. The logic is
   `LiveRun` in `live.ts` (pure); `liveView.ts` drives the view from it and from `stepStore`. UI test "a run
   is kept while the Simulate step is away, drawn when it is back, and its end clears it on any step". Not
   driven in the app.

Checks after the fixes: `npm run typecheck` clean; `npm test` 336 of 336; the app crate's live tests 7 of 7
(C: had 23 GB free before each `cargo test`). Smoke, headless (`bed-b3.py`, `.out\b3\audit-smoke-cr4-1k\`):
CR4 1 kHz on the GPU from the Run button, OK in 3.66 s from the click; one 200 ms sample caught the layer
live (400 of 400 arrived, 226 drawn at that moment, Glow); afterwards the layer cleared, no stream file left,
`spps-gpu.json` 400 frames, the Results step "verified" with its 400 particles. The untested list above
changes: the stale stream and the leftover stream are now handled; a step change is unit-tested, not driven.
