# B3: the running solve on screen (spec, 2026-10-07 10:10)

PLAN.md's B3, verbatim (line 47): "spps-gpu streams a sample of particle positions per step to the app while
it runs, so the room fills with sound on screen as it is computed". Done when: "The Simulate step shows the
running solve; the cost to the solve is measured."

Burhan's words behind it, 2026-10-06 19:24: "LETS BUILD THAT VERSION, AND ALSO SOMETHING THAT ALLOWS FOR
COMPETENT GRaphics and animations so we can have awesome ray tracing on the screen and other plots and other
things". Order for today, 08:57: "okay proceed with da work".

## The shape, decided 10:08 (log as decision 71 in `docs/decision-log.md`)

The walk is slot-major (`solvers/spps-gpu/src/main.cu:565-568`: one thread per resident slot, each walking
its particle family for up to `budget` steps, then the next family). Between launches the slots hold
particles at different simulation times, so a snapshot of the slots is not a frame at a time *t*. A true
per-step stream needs a time-major walk (every particle resident, all advanced one chunk per launch): a
kernel restructure, and 600 k resident slots on Zeph's 6 GB is a memory question. **Not in B3.** Recorded
below as the v2 road, with what it would cost.

**B3 streams the marked particles' trajectories as each completes.** The solver already records them on
the host (`HostRec::step`, `output.cpp:509`; the retrace per marked particle at `main.cu:695-707`) and saves
the `.pbin` at the end. B3 writes them out as they finish, and the app plays them in simulation time as they
arrive, so the room fills with trails while the bands compute. Honest on screen: the live view is the
*saved sample* of particles (the project's "particles saved per source"), the same ones the Results step
replays after the run; it is not every particle.

## What to build

1. **Solver side** (`solvers/spps-gpu/src/`): a stream file in the run's `solve/` folder, one per run,
   `live.pstream` (name it to the code's idiom): a header (magic, version, the time step, the band list),
   then frames appended as marked particles finish: `{band_hz, particle index, first_time_step, n, positions
   f32×3n, energies f32×n}`. Append in whole frames with a length prefix so a reader never sees a torn
   frame; flush after each append (or every k frames, with the cost measured). The `.pbin` files are
   written exactly as today (the e2e contract and the A2 bed must not change: run the A2 CPU-vs-GPU check
   and the `vs_spps` comparison once at the end to show the walk and its outputs are bit-identical with
   streaming on). A flag or config attribute turns the stream off; measure both.
   **Cost, measured** (REALISED, configuration stated): the CR4 1 kHz 2×300 k 10 s case
   (`B:\repos\I-Simpa_Night_Mode\.out\a5\cr4-1k`, copy it) and the 27-band CR4-27 project
   (`B:\repos\I-Simpa_Night_Mode\.out\ui\cr4-27\CR4-27.simpa`, copy it, GPU) with the stream on and off,
   three runs each, wall time and solver-reported seconds. The done-when wants this number.
2. **App side** (`app/src-tauri/src/runs.rs` is where a run's child is held and its stdout classified,
   `crates/simpa-core/src/run/classify.rs:337` the PROGRESS lines): while a run is live, tail the stream
   file (poll every ~100 ms; a file watcher if the code has one), decode whole frames, and emit them to
   the UI as a Tauri event carrying the PART v1 layout the viewport already decodes
   (`app/src-tauri/src/results_data.rs` header comment, `encode_particles`): one event per batch of frames,
   batched to at most ~20 events a second. On run end, the stream reader stops and the Results step takes
   over as today.
3. **Viewport** (`app/ui/src/features/viewport/`: `particles.ts`, `rays.ts`, `glow.ts`, `resultsLayer.ts`,
   the Track B modes): a live layer that accepts arriving trajectories and plays them in simulation time
   (the clock starts when the first frame arrives; a trajectory that arrives late starts from its first
   step at the current clock, so the room keeps filling rather than jumping). Use the Glow/Rays/Dots modes
   as they are; the Simulate step shows the viewport with this layer while the run is live, with a caption
   "live: the saved sample of particles, N of M so far, band X Hz". When the run ends, the layer clears and
   the Results step's playback is unchanged. The replicated "1M" mode stays off and is not used here.
4. **Tests:** Rust unit tests for the frame decoder (a torn tail is ignored, not an error); UI unit tests
   for the live layer's clock and the caption; the m12 e2e contract untouched (do not run the gates; they
   are the main thread's).

## Not in B3 (write it in the report's last section, as the v2 road)

The time-major walk: what it changes in `main.cu`/`walk.h`, the resident-slot memory at 2×300 k and at
CR4-27's particle count on 6 GB and 12 GB, and a forecast of its cost from the per-launch overhead you
measure here. No code for it.

## Constraints

- Worktree `B:\repos\I-Simpa_Night_Mode\.claude\worktrees\b3` (branch `b3`, from `gpu`). Outputs only to
  `B:\repos\I-Simpa_Night_Mode\.out\b3\`, `C:\tmp\nm-b3\` (your spps-gpu build: `solvers/spps-gpu/build.cmd
  C:\tmp\nm-b3\bin` through cmd.exe) and `C:\tmp\nm-target-b3` (cargo target). Your solvers folder for app
  runs: copy `C:\tmp\nm-solvers-a5\*` to `C:\tmp\nm-solvers-b3\` and put your spps-gpu.exe there; the app's
  manifest check will refuse it as unverified, so for the app bed add its hashes to `solvers/manifest.json`
  on `b3` (the re-pin at merge is the main thread's; say in the report what you pinned). Launch with
  `SIMPA_SOLVERS_DIR=C:\tmp\nm-solvers-b3`.
- Build the app with `npx --no-install tauri build --no-bundle` in `app/` and `CARGO_TARGET_DIR=C:/tmp/nm-target-b3`.
  Before any `cargo test`, check C: free space; stop and report if under 20 GB. Run the test targets you
  touch, not the whole workspace; the full app crate suite once at the end.
- Do not touch `C:\tmp\nm-target`, `C:\tmp\nm-solvers-a5`, other worktrees, or any app.exe you did not start.
  Do NOT run the m10/m11/m12 e2e gates. Drive your own app headless with `tools/devtools/` (README).
- A script that copies a solve folder rewrites the ROOT element's `workingdirectory` in config.xml and
  refuses to start unless it names the copy.
- Read the solver's stderr on every run. Every number carries its configuration and the label REALISED.
- Commit on `b3` as you go, messages on the why, no Claude attribution, no Co-Authored-By. Do not push.
- `npm run typecheck` and `npm test` in `app/` pass at the end.

## Report

`docs/investigations/2026-10-06-gpu/B3-REPORT.md` on `b3`, committed. Lead with whether the done-when
holds; the cost table (stream on/off, both cases, three runs); the bit-identity receipt; what was built
(files); screenshots of the live view at three moments (`.out\b3\`); what is untested; the v2 road.
