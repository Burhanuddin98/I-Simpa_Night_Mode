# Backlog 82 and 84 plan: lost particles and the decay times (2026-10-05)

Ordered by Burhan 19:27 ("yes do as you would recommend"), as the remaining M12 work: T30 must show in clean rooms
(82), and EDT must not show where the same lost energy refuses T20/T30 beside it (84). Branch off `map-window` once its
gate is green. Outputs under `.out\b82-b84\` (decision 55).

## What is already known (receipts)

- The refusal: `params/decay.rs` `settle` refuses `missing_moves` when the value moves more than 0.5 %
  (`limits::DECAY_RELATIVE`) with the solver's floor and its lost particles' energy added back.
- The lost part is `10 x lost / emitted` (`results/spps.rs:562-576`, `ENERGETIC_LOST_ENERGY_RATIO` = 10, line 618):
  10 is a cap on a lost particle's energy over the mean energy alive when it was lost.
- Measured ratios so far: tutorial 1, 0.16-2.02 (17 of 24 lost found); an M8 cell, 0.04-4.4 (44 of 76). No hall.
  Trajectory evidence: `crates/simpa/tests/m8_evidence.rs:642`, logs in `docs/investigations/2026-09-24-m8-evidence/`.
- CR2 125 Hz T20, receiver 0, source 0: 1.4948 s from the series, 1.5032 s with the missing energy added (0.56 %
  against 0.5 %), lost share 2.865e-3, floor -70 dB (`B:\data\m12\pearl-geom\cr2-solve\results.json`).
- The cap is global, so it cannot go below the largest ratio measured anywhere (4.4): it can fall by about 2x at most.

## Steps

0. **Probe first, no solver run.** Re-read the existing CR2 run and the Elmia runs of backlog 78
   (`B:\data\m12\b78-mesh\`) with the ratio set to 4.4, 2 and 1 in a throwaway probe build (never merged), and count the
   T20/T30 refusals. This answers whether the cap is what refuses them. If T30 still refuses at a ratio of 1, the
   floor term or the 0.5 % limit is the cause, not the cap, and step 1 changes target.
1. **The hall bed (82).** Only if step 0 says the cap matters: run Elmia (decision 49's fixture) and CR2 in energetic
   mode with a saved subset of particles (`particles_saved` below `particles`, so the files fit on B:), 3 seeds, and
   read every lost particle's ratio with `energetic_lost_particles_from_saved_trajectories` (`SIMPA_LOST_CELL`
   extended to a project file). Smoke one seed first and measure the file size before the 3 seeds.
   Set the cap from the largest ratio over tutorial 1, the M8 cell and the halls, with the margin rule written
   in `spps.rs`'s doc, or keep 10 with this bed as its receipt.
2. **EDT against lost energy (84).** In `results/report.rs:1697` `edt_report`, compute EDT also on the series with the
   lost energy and the floor added back, as `settle` does for T20/T30, and refuse only where that value falls outside
   EDT's own noise range (backlog 84: the 0.5 % rule over 10 dB would refuse every EDT). A test with a series that
   loses particles late must refuse; one that loses none must not.
3. Results version bump, the gates (m10/m11/m12), assay on the diff, backlog rows 82/84 closed with receipts.
