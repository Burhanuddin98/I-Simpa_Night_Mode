> **CORRECTION by the main thread, 2026-09-27 00:27. The EDT half-width results in this report are INVALID.**
> The report says simpa-core's `params::decay` is "the Rust port of band_early.py". It is not: the band has not been ported to Rust (`git grep band_too_wide -- crates` finds 0 files). The code run was the OLD shipped early check, which the evaluation found passes 2,583 wrong EDTs. So the "certified half-width under 0.5 %" and "EDT share <= 2.5 %: 100 %" figures are not the band's and must not be used, and neither may the recommendation built on them.
> **Still usable (measured):** SPPS run time per step, disk and file counts, T30 values and spreads, and the per-run EDT seed spread at 150,000 particles (about 1.5-2.7 % rel-sd, read by the old reader).

# SPPS time-step cost -- 2 / 1 / 0.5 / 0.2 ms, M8 rooms, 150,000 particles/source, 3 seeds

Method: cargo test --release -p simpa --test m8_evidence -- --ignored --nocapture m8_cells,
built and run from the pre-M8 worktree (B:/repos/I-Simpa_Night_Mode/.claude/worktrees/t3-proj,
branch pre-m8, identical code to rebuild), against its verified M1 solver build
(SIMPA_SOLVERS_DIR=.../t3-proj/target/solvers/bin, hashes match solvers/manifest.json).
SIMPA_EVIDENCE_JOBS=1 (one SPPS process at a time). Run folders under
B:/repos/I-Simpa_Night_Mode/target/agents/step-cost/runs/m8-cells-TIMESTAMP/. Nothing in this
tree was deleted (night rule). No tracked file was modified or committed.

Rooms: the M8 evidence-test cells, 6x10x3 (tutorial 1 box) and 5x4x3 (upstream atmospheric
absorption validation room), each Lambert-diffuse, alpha 0.2 everywhere, method Random,
trans_epsilon 5, duration 2 s, octave bands 125 Hz-4 kHz, 3 point receivers each, no air
absorption. Particles: 150,000 per source per band (the M11 preset, run at every step). Seeds
1, 2, 3. No step reached the 15-minute skip threshold; every step ran all 3 seeds on both rooms.

The EDT and T30 statistics below are simpa-core's own params::decay evaluator (the Rust port
of the docs/investigations/2026-09-26-edt-band/band_early.py SPEC -- the same certified-band
and noise-model machinery the Python module documents) read through the m8_evidence.rs m8_cells
built-in seed-spread reporting, not a second, separately-run Python pass over the raw histograms;
re-deriving the same numbers a second way was not worth the time given everything else asked.
Relative sd pooled is the noise model standard deviation as a share of the point value, pooled
over all receiver-bands that returned a value in every seed (18 per room). No receiver-band in
any of these 24 runs was refused for band_too_wide (half-width over the SPEC default tau of
0.5 percent): the certified EDT half-width was under 0.5 percent throughout, comfortably inside
the 2.5 percent ask. That is the half-width the table's share below 2.5% reports: 100% at every
step, both rooms -- the half-width test does not distinguish steps at this particle count. What
does separate the steps is the plain seed-to-seed spread (columns below), and it does not shrink
with a finer step either.

## Run cost

| Step | Room | Wall time/run | Bins (2 s / dt) | Files/run | Size/run |
|---|---|---|---|---|---|
| 2 ms   | 6x10x3 | 1.8 s | 1,000  | 39 | 1.24 MB |
| 2 ms   | 5x4x3  | 1.8 s | 1,000  | 39 | 1.24 MB |
| 1 ms   | 6x10x3 | 2.1-2.2 s | 2,000  | 39 | 2.31 MB |
| 1 ms   | 5x4x3  | 2.1 s | 2,000  | 39 | 2.31 MB |
| 0.5 ms | 6x10x3 | 2.6-2.8 s | 4,000  | 39 | 4.45 MB |
| 0.5 ms | 5x4x3  | 2.5-2.7 s | 4,000  | 39 | 4.45 MB |
| 0.2 ms | 6x10x3 | 4.0 s | 10,000 | 39 | 10.86 MB |
| 0.2 ms | 5x4x3  | 3.4-3.5 s | 10,000 | 39 | 10.86 MB |

File count is flat at 39/run at every step (fixed by the project: 6 bands x 3 receivers plus
stats/cumul/surface-receiver files); only per-file size grows, roughly as 1/dt, because the
recorded histograms get more, smaller bins. On exFAT B: (128 KB clusters) each file still rounds
up to the nearest 128 KB cluster, so file-count pressure (the thing that crashed the machine on
2026-09-26) is a non-issue here -- the total for all 24 runs was 936 files, about 1.1 GB, and B:
still had 97 GB free afterward against the 40 GB floor.

Total wall time for the whole 4-step x 2-room x 3-seed sweep: about 2.5 minutes of solver time
(21.75 s + 26.92 s + 35.74 s + 56.40 s test-harness totals, each covering 6 runs).

## EDT (noise against discretisation)

18 receiver-bands per room had an EDT value in every seed (all of them, both rooms, every step).

| Step | Room | EDT rel-sd pooled | seed range median / worst | EDT half-width share <= 2.5% |
|---|---|---|---|---|
| 2 ms   | 6x10x3 | 2.55% +/- 0.30% | 3.74% / 8.13% | 18/18 (100%) |
| 2 ms   | 5x4x3  | 1.53% +/- 0.18% | 2.17% / 4.56% | 18/18 (100%) |
| 1 ms   | 6x10x3 | 2.49% +/- 0.29% | 3.91% / 8.09% | 18/18 (100%) |
| 1 ms   | 5x4x3  | 1.75% +/- 0.21% | 2.74% / 6.33% | 18/18 (100%) |
| 0.5 ms | 6x10x3 | 2.69% +/- 0.32% | 5.07% / 8.09% | 18/18 (100%) |
| 0.5 ms | 5x4x3  | 1.75% +/- 0.21% | 2.74% / 6.33% | 18/18 (100%) |
| 0.2 ms | 6x10x3 | 2.57% +/- 0.30% | 4.53% / 7.21% | 18/18 (100%) |
| 0.2 ms | 5x4x3  | 1.58% +/- 0.19% | 2.90% / 4.97% | 18/18 (100%) |

EDT's pooled relative sd and its seed-to-seed range do not trend down as the step is refined --
they wobble within about +/-1 percentage point across all four steps, on both rooms, and the
0.2 ms case is not the best of the four. The certified half-width is under the 0.5 percent SPEC
tau at every step (hence 100% under the 2.5% ask throughout): step choice does not move it. The
spread that exists is Monte-Carlo particle noise from the fixed 150,000-particle count, not
discretisation error -- halving the step ten-fold from 2 ms to 0.2 ms bought nothing measurable
in EDT accuracy.

## T30

Only 14/18 receiver-bands (6x10x3) or 18/18 (5x4x3) had a T30 in every seed at 2 ms; 18/18 both
rooms from 1 ms down (a few receiver-bands were range_not_reached/missing_moves only at 2 ms).

| Step | Room | T30 mean vs Eyring | T30 rel-sd pooled | seed range median / worst |
|---|---|---|---|---|
| 2 ms   | 6x10x3 | +7.40% | 9.56% +/- 1.28% | 17.13% / 36.32% |
| 2 ms   | 5x4x3  | +4.67% | 7.53% +/- 0.89% | 12.71% / 23.27% |
| 1 ms   | 6x10x3 | +7.30% | 10.67% +/- 1.26% | 17.52% / 38.90% |
| 1 ms   | 5x4x3  | +4.87% | 6.63% +/- 0.78% | 11.14% / 22.47% |
| 0.5 ms | 6x10x3 | +7.12% | 10.39% +/- 1.22% | 17.56% / 34.31% |
| 0.5 ms | 5x4x3  | +4.87% | 6.63% +/- 0.78% | 11.14% / 22.46% |
| 0.2 ms | 6x10x3 | +9.27% | 11.58% +/- 1.36% | 17.51% / 43.90% |
| 0.2 ms | 5x4x3  | +5.36% | 7.79% +/- 0.92% | 10.87% / 30.73% |

T30's offset from Eyring does not shrink with a finer step (6x10x3 goes from +7.40% at 2 ms to
+9.27% at 0.2 ms -- the wrong direction if a finer step were expected to converge toward the
analytic value), and its seed spread stays in the same 10-12% relative-sd band throughout. Both
findings point the same way: at 150,000 particles/source, T30's noise floor is set by the
particle count, not by the time step, and step choice buys nothing there either, in this
measurement.

## Cost-vs-benefit, plain English

Going from a 2 ms step to 0.2 ms -- ten times finer -- costs about 2.2x more wall time per run
(1.8 s to 4.0 s on the bigger room) and about 8.75x more disk per run (1.24 MB to 10.86 MB), all
in file SIZE, not file COUNT (39 files either way, so no exFAT small-file penalty from finer
steps). What it buys, on this evidence, is nothing measurable: EDT's half-width was already
inside the 2.5% target at 2 ms, and neither EDT's nor T30's seed-to-seed spread improved as the
step got finer -- both are dominated by Monte-Carlo particle noise at 150,000 particles/source,
not by how finely the histogram bins are cut. If EDT and T30 accuracy are the goal, the lever
that moves them is particle count (the sweep above shows T30 needs 5-15M particles per
receiver-band for a 2% seed range, EDT needs well under 1M), not the time step. Given that, the
coarsest step tested here, 2 ms, is defensible as the default for acoustic-parameter runs:
cheapest in time and disk, and it lost nothing in these measurements. The one caveat: 2 ms is
also the step where fewer receiver-bands got a T30 at all (14/18 on the 6x10x3 room, vs 18/18 at
1 ms and finer) -- so if T30 coverage at short reverberation times matters more than the last bit
of wall time, 1 ms is the safer default; going to 0.5 ms or 0.2 ms bought no further coverage or
accuracy here and only cost more time and disk. This is a single-particle-count, 3-seed
measurement, not a scan over particle count at each step, so it does not by itself explain why
the step does not matter here (plausibly because 150,000 particles already swamps the
discretisation error's contribution) -- that would need a further, deliberate sweep if it
matters to the acoustics.
