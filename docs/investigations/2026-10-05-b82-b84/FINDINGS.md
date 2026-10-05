# Backlog 82 and 84: findings (2026-10-05 20:26)

Plan: `PLAN.md` here. Scripts and tables: `B:\repos\I-Simpa_Night_Mode\.out\b82-b84\` (`probe_cap.py`,
`bed.ps1`, `extract_ends.py`, `ratios.py`; the trajectories were deleted after `extract_ends.py` kept every saved
particle's start and end, `bed\**\ends-<band>.npz`).

## Step 0: the cap is what refuses, in every room probed

Existing runs re-read with `ENERGETIC_LOST_ENERGY_RATIO` overridden (probe build, branch `b82-probe`, never merged):

| Run | at 10 (today) | 7 | 6 | 5 | 4.4 |
|---|---|---|---|---|---|
| CR2, T20 / T30 shown of 72 | 2 / 22 | 22 / 72 | 32 / 72 | 72 / 72 | 72 / 72 |
| CR4, T20 / T30 shown of 72 | 2 / 22 | 22 / 72 | 59 / 72 | 72 / 72 | - |
| Elmia fixture (0.6 m, 3 sources), T20 shown of 128 | 74 | 128 | 128 | 128 | 128 |

## Step 1: the hall bed

`energetic_lost_particles_from_saved_trajectories` with `SIMPA_LOST_PROJECT` (commit 623ca63), 150,000 particles per
source, 10,000 saved, energetic, 10 s at 1 ms, extinction 7. Elmia fixture 5 runs, CR2 3 runs.

**The detector had to change for a hall.** Its dropped-particle bound (10x the floor) assumed alpha at most 0.3;
Elmia's audience is 0.80-0.88, so 110 of the smoke run's 151 "lost" were particles the floor dropped (last-step drop
up to 25x). At 100x the floor the split is clean (largest floor drop 95x, smallest true loss 121x) and the count
matches SPPS's own: 41 found against about 44 expected from 668 counted.

| Room | lost found | ratio median | 90 % | max | **mean** (bootstrap 95 %) | per-run means | T30 moved by them |
|---|---|---|---|---|---|---|---|
| Elmia | 201 | 0.48 | 3.51 | 335 | **3.64** (1.26-7.51) | 1.91, 2.45, 0.72, 9.77 | 5.3e-4 |
| CR2 | 92 | 0.79 | 1.43 | 3.79 | **0.84** (0.71-0.98) | 0.76, 1.03 | 1.2e-4 |

(Ratio: a lost particle's energy over the mean energy of the particles alive when it was lost. "T30 moved": the
worst receiver-band, the lost particles' future modelled as following the decay; limit 5e-3.)

## What it means

1. The bound applies `10 x lost/emitted`, a bound on the lost particles' **total**, so the statistic that matters is
   the mean ratio, not the largest. `spps.rs`'s doc calls 10 "the most a lost particle's energy is taken to be";
   that is wrong for a hall (single particles reach 335), though 10 still covers every measured mean.
2. **The cap cannot come down.** One Elmia run's mean was 9.77: the heavy tail is particles lost in the reflective
   upper volume, away from the absorbing audience. CR2's is 0.84. The solver does not report which kind of room it is.
3. **The refusals are still far too pessimistic.** The lost particles actually moved T30 by 1e-4 to 5e-4, 10-40x
   below the limit. The pessimism is in the bound's shape: `following::decay_relative` = 9 x 10 lg(1+s) / range
   assumes the missing share can sit anywhere in [0, s] at every level.
4. Backlog 84 (branch `b84`, 19f21e0) applies the same bound to EDT against EDT's own range: CR2 then refuses all 70
   EDTs (bound 1.1-1.8 % against half-widths 0.50-0.77 %); Elmia's 126 stay shown.
