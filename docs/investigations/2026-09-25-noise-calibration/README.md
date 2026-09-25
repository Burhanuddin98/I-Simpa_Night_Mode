# Calibrating the Monte-Carlo noise model against SPPS's own seeds (pre-M8, piece B)

**Asked for** (Burhan, 2026-09-24 17:45, M8 decision 3): make the noise model's standard
deviation match the seed-to-seed spread SPPS actually shows, per quantity and computation method,
never below it; have a refusal for noise name the particle count that would bring the value within
its limit; measure what that changes at upstream's default count on tutorial 1. The refusal limits
are Burhan's and are unchanged (EDT, T20, T30 2.5 % relative; C50, C80 0.5 dB; D50 0.025; Ts 5 ms;
SPL 0.5 dB).

**Result, in short.**
- **Random mode:** M7's model was *not* the sound bound it was taken for. In 11 of the 13 cells
  SPPS's spread was significantly above it (the one-sided 95 % lower bound of the pooled ratio above
  1) for at least one quantity, by up to 1.43 times (T30, a specular room whose absorption is on its
  floor). Calibrated factors 1.2 to 1.6 now hold it at or above the spread in every validation cell
  (pooled ratios 0.61 to 0.92).
- **Energetic mode:** M7's model overstates the spread 1.2 to 38 times. SPL, EDT, C50, C80, D50
  and Ts are calibrated (factors 0.59 to 0.91) and pass every validation cell. **T20 and T30 are
  not calibrated:** two rounds each failed a held-out room (a 20 m corridor with its absorption on
  the floor, specular, then the same corridor scattering 0.3, where T30's spread was 1.9 and 2.0
  times the calibrated prediction). They keep M7's bound, factor 1, which holds in all nineteen
  energetic cells (at most 0.62 of it).
- **A refusal for noise names a particle count**, `N·(margin·sd/limit)²`, except where the spread's
  fall as `1/√N` was not confirmed at two standard errors: random-mode T30 and energetic EDT. On
  the pairs of cells that differ only in their count, every refusal whose named count the higher
  count reached gave its value there (100 % of 1,394 such receiver-band seeds).
- **Tutorial 1 at upstream's default** (150,000 particles): almost nothing changes. T30 and EDT
  come through in 0 of 540 receiver-bands before and after (EDT is refused `early_unresolved` at
  the 10 ms step, T30 for its range, the energetic floor or, in random mode, noise it really
  has); C80 472 against 479 (random) and 490 against 480 (energetic); D50 540 of 540 throughout.

Everything below is measured; the rules were fixed before the numbers they judge were read
(`PREREGISTER.txt` and its two addenda): the design at `ecae322`, before any run was read; round
1's factors and the margin rule at `641a7ba`, before any validation cell was read; round 2's design
at `fbc45a8` and its factors at `732951c`, before any of its cells was read.

## The design

- **Cells.** A room (tutorial 1's box scaled), its walls, SPPS's computation method, particles per
  source, duration, time step and `trans_epsilon`; octave bands 125 Hz to 4 kHz; six receivers each
  at least 1 m from the walls and the source; seeds 1 to 10. 36 receiver-bands per cell. 32 cells:
  7 calibration and 6 validation cells per method in round 1, and 6 energetic validation cells in
  round 2. The configuration of every cell is in `calibration.json` (`cells[].cell`) and in
  `crates/simpa/tests/noise_calibration.rs` (`CELLS`).
- **Observed:** per receiver-band and quantity, the standard deviation over the ten seeds of the
  value each seed's series gives as the bootstrap evaluates its resamples (complete, the early
  reverberation unresolved as the report reads it, from the report's arrival); relative to the
  mean for the decay times. Only receiver-bands where all ten seeds give a value.
- **Predicted:** the root-mean-square over the seeds of the model's standard deviation
  (`params::noise::bootstrap`, before calibration).
- **Pooled ratio** per cell and quantity: `√(Σ observed² / Σ predicted²)` over the receiver-bands,
  with one-sided 95 % bounds from the chi-square distribution of `receiver-bands × 9` degrees of
  freedom.
- **Rules** (pre-registered): 1, the structure (energetic mode: M7's constant deposit, or a deposit
  scaled step by step by the particles' mean energy from the room table); 2, the factor per method
  and quantity is the largest upper bound over the calibration cells, rounded up to two digits;
  3, on every validation cell the lower bound of observed over calibrated prediction is at most 1;
  4, `1/√N` must hold within two joint standard errors on every pair of cells that differ only in
  their count, or no count is named; 5b, the named count's margin, `1 + 1.28·s` (at least 1.1),
  `s` one seed's estimate scatter; 6, the named counts tested on the pairs; R2-1 to R2-5, round 2.

**Compute.** 320 SPPS runs on Grace, 24 at a time: round 1's 260 in 1,059 s, round 2's 60 in
251 s; tutorial 1's 20 in 56 s. Each run's wall time is in `calibration.json` (`cell.wall_s`).

## Round 1: the seeds' spread over M7's model, cell by cell

Factor 1, the constant structure: what M7 shipped. A ratio above 1 is noise M7 did not claim.
"–": no receiver-band where every seed gives the value (C50, C80, D50 and Ts are refused
`params_bad_arrival` at most receivers at a 1 ms step, and EDT and Ts `early_unresolved` in
tutorial 1's room at 10 ms).

#### Random mode

| Cell | Room | Walls | N | Duration | dt | ε | SPL | EDT | T20 | T30 | C50 | C80 | D50 | Ts |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| C-R1 | 6x10x3 | Lambert α 0.1 | 150 k | 3 s | 10 ms | 5 | 1.119 | 1.084 | 1.050 | 1.177 | 1.052 | 1.038 | 1.052 | 1.111 |
| C-R2 | 6x10x3 | Lambert α 0.1 | 1.5 M | 3 s | 10 ms | 5 | 1.018 | 1.040 | 1.091 | 0.987 | 1.048 | 0.994 | 1.047 | 1.037 |
| C-R3 | 5x4x3 | Lambert α 0.4 | 150 k | 1 s | 1 ms | 5 | 0.975 | 1.030 | 0.932 | 1.112 | – | – | – | – |
| C-R4 | 5x4x3 | Lambert α 0.4 | 15 M | 1 s | 1 ms | 5 | 1.044 | 1.049 | 1.021 | 1.089 | – | – | – | – |
| C-R5 | 6x10x3 | tutorial 1's materials, air on | 150 k | 2 s | 10 ms | 5 | 1.054 | – | 0.997 | 0.948 | 0.997 | 1.012 | 0.999 | – |
| C-R6 | 6x10x3 | tutorial 1's materials, air on | 1.5 M | 2 s | 1 ms | 5 | 1.079 | 0.971 | 1.038 | 1.282 | 1.042 | 1.009 | 1.034 | 0.967 |
| C-R7 | 6x10x3 | dead floor (0.6; 0.05 elsewhere), specular | 500 k | 3 s | 10 ms | 5 | 1.165 | 1.222 | 1.283 | 1.432 | 1.020 | 1.073 | 1.019 | 1.287 |
| V-R1 | 20x4x3 | Lambert α 0.2 | 500 k | 2 s | 10 ms | 5 | 1.056 | 0.856 | 0.949 | 1.131 | 1.012 | 0.971 | 1.006 | 0.973 |
| V-R2 | 6x10x3 | Lambert α 0.4 | 600 k | 1 s | 1 ms | 5 | 1.040 | 1.047 | 0.970 | 1.014 | 0.941 | 0.980 | 0.946 | 1.029 |
| V-R3 | 5x4x3 | Lambert α 0.05 | 300 k | 3 s | 10 ms | 5 | 1.120 | 1.197 | 1.206 | 1.101 | 0.999 | 1.013 | 0.998 | 1.223 |
| V-R4 | 6x10x3 | tutorial 1's materials, air on | 600 k | 2 s | 10 ms | 5 | 1.069 | – | 1.078 | 1.079 | 1.062 | 1.076 | 1.068 | – |
| V-R5 | 5x4x3 | specular α 0.2 | 5 M | 1.5 s | 10 ms | 5 | 1.019 | – | 1.161 | 1.250 | 1.029 | 1.105 | 1.027 | – |
| V-R6 | 20x4x3 | dead floor, specular | 1.5 M | 3 s | 1 ms | 5 | 0.992 | 1.237 | 1.180 | 1.229 | – | – | – | – |

The largest ratios come with specular walls and absorption concentrated on one surface: a
particle's crossings of a receiver at several times on a near-periodic path, which the model,
counting crossings as independent, leaves out.

#### Energetic mode

| Cell | Room | Walls | N | Duration | dt | ε | SPL | EDT | T20 | T30 | C50 | C80 | D50 | Ts |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| C-E1 | 6x10x3 | Lambert α 0.1 | 150 k | 2.5 s | 10 ms | 7 | 0.757 | 0.356 | 0.077 | 0.027 | 0.735 | 0.662 | 0.735 | 0.505 |
| C-E2 | 6x10x3 | Lambert α 0.1 | 600 k | 2.5 s | 10 ms | 7 | 0.771 | 0.371 | 0.073 | 0.027 | 0.779 | 0.716 | 0.779 | 0.570 |
| C-E3 | 5x4x3 | Lambert α 0.4 | 150 k | 0.5 s | 1 ms | 9 | 0.783 | 0.408 | 0.091 | 0.034 | – | – | – | – |
| C-E4 | 5x4x3 | Lambert α 0.4 | 2.4 M | 0.5 s | 1 ms | 9 | 0.742 | 0.362 | 0.087 | 0.033 | – | – | – | – |
| C-E5 | 6x10x3 | tutorial 1's materials, air on | 150 k | 2 s | 10 ms | 5 | 0.710 | – | 0.110 | 0.069 | 0.540 | 0.404 | 0.542 | – |
| C-E6 | 6x10x3 | tutorial 1's materials, air on | 600 k | 2 s | 1 ms | 7 | 0.702 | 0.349 | 0.124 | 0.065 | 0.544 | 0.418 | 0.542 | 0.446 |
| C-E7 | 6x10x3 | dead floor, specular | 300 k | 3 s | 10 ms | 7 | 0.844 | 0.549 | 0.242 | 0.136 | 0.728 | 0.697 | 0.727 | 0.540 |
| V-E1 | 20x4x3 | Lambert α 0.2 | 300 k | 1.5 s | 10 ms | 7 | 0.605 | 0.239 | 0.059 | 0.031 | 0.553 | 0.413 | 0.556 | 0.485 |
| V-E2 | 6x10x3 | Lambert α 0.4 | 150 k | 0.6 s | 1 ms | 9 | 0.696 | 0.326 | 0.084 | 0.048 | 0.291 | 0.142 | 0.292 | 0.494 |
| V-E3 | 5x4x3 | Lambert α 0.05 | 150 k | 3 s | 10 ms | 7 | 0.718 | 0.380 | 0.072 | 0.026 | 0.838 | 0.778 | 0.839 | 0.525 |
| V-E4 | 6x10x3 | tutorial 1's materials, air on | 1.5 M | 2 s | 10 ms | 5 | 0.726 | – | 0.121 | 0.067 | 0.528 | 0.405 | 0.529 | – |
| V-E5 | 5x4x3 | specular α 0.2 | 600 k | 1.5 s | 1 ms | 7 | 0.666 | 0.328 | 0.094 | 0.048 | – | – | – | – |
| V-E6 | 20x4x3 | dead floor, specular | 300 k | 3 s | 10 ms | 7 | 0.742 | 0.559 | 0.394 | 0.285 | 0.731 | 0.715 | 0.731 | 0.588 |
| W-E1 | 30x4x3 | dead floor, specular | 300 k | 3 s | 10 ms | 7 | 0.716 | 0.484 | 0.423 | 0.329 | 0.758 | 0.666 | 0.764 | 0.571 |
| W-E2 | 20x4x3 | dead ceiling (0.6; 0.05 elsewhere), specular | 300 k | 3 s | 10 ms | 7 | 0.758 | 0.495 | 0.362 | 0.284 | 0.688 | 0.662 | 0.688 | 0.513 |
| W-E3 | 20x8x6 | dead floor, specular | 300 k | 3 s | 10 ms | 7 | 0.731 | 0.489 | 0.224 | 0.206 | 0.759 | 0.741 | 0.762 | 0.489 |
| W-E4 | 20x4x3 | dead floor, every surface scattering 0.3 | 300 k | 3 s | 10 ms | 7 | 0.750 | 0.520 | 0.467 | 0.622 | 0.626 | 0.537 | 0.614 | 0.558 |
| W-E5 | 10x10x10 | dead floor, specular | 300 k | 4 s | 10 ms | 7 | 0.783 | 0.447 | 0.158 | 0.082 | 0.822 | 0.752 | 0.823 | 0.464 |
| W-E6 | 20x4x3 | dead floor, specular | 600 k | 3 s | 1 ms | 7 | 0.697 | 0.558 | 0.359 | 0.271 | – | – | – | – |

T20 and T30 carry the most spread between rooms: 0.026 to 0.622 of M7's bound for T30. In
energetic mode a particle's energy falls at each reflection, so late in the decay the energy is
carried by the few particles that met the absorbing surface least. How far their energies spread
apart depends on the room's geometry, its scattering and where its absorption is, which the model
does not see. No one factor both follows a Lambert box (0.03) and covers a partly diffuse dead-floor
corridor (0.62).

## Rule 1: the structure for energetic mode

Calibrated on round 1's calibration cells, the constant structure overstated the seeds' spread by
1.653 (geometric mean over cells and quantities) and the mean-energy structure by 2.231 (T30's
factor 16, set by the dead-floor room, where the particles' energies spread apart the most). So
energetic mode keeps M7's constant deposit, and the mean-energy structure is not in the code
(`calibration-cells.txt` has both; `calibration.json` keeps its numbers as
`predicted_sd.mean_energy`).

## Rule 2: the factors, and what validates them

| Method | SPL | EDT | T20 | T30 | C50 | C80 | D50 | Ts |
|---|---|---|---|---|---|---|---|---|
| random (upper bound; set by) | 1.3 (1.2457, C-R7) | 1.4 (1.3066, C-R7) | 1.4 (1.3724, C-R7) | 1.6 (1.5622, C-R5) | 1.2 (1.1475, C-R6) | 1.2 (1.1661, C-R7) | 1.2 (1.1426, C-R1) | 1.4 (1.3987, C-R7) |
| energetic, round 1 | 0.91 (0.9026, C-E7) | 0.59 (0.5867, C-E7) | 0.26 (0.2585, C-E7) | 0.15 (0.1457, C-E7) | 0.85 (0.8460, C-E2) | 0.78 (0.7784, C-E2) | 0.85 (0.8463, C-E2) | 0.62 (0.6189, C-E2) |
| energetic, round 2 (13 cells) | | | 0.43 (0.4209, V-E6) | 0.31 (0.3051, V-E6) | | | | |
| **energetic, shipped** | **0.91** | **0.59** | **1** | **1** | **0.85** | **0.78** | **0.85** | **0.62** |

C-R5's T30 has one receiver-band with a value in every seed; its wide upper bound (1.562) sets
random T30 at 1.6, and C-R7's 1.531 would have rounded to 1.6 as well.

**Validation, round 1** (`validation-round1.txt`): every random-mode cell passes every quantity.
Energetic mode passes five cells of six; **V-E6 fails T20 (1.514, lower bound 1.423) and T30
(1.902, lower bound 1.787)**. **Round 2** (`validation-round2.txt`): T20 and T30 re-calibrated on
all thirteen energetic cells (0.43, 0.31), validated on six new rooms chosen for being at least as
adverse: W-E1 passes T30 by a hair (1.063, lower bound 0.998), and **W-E4, the corridor with its
surfaces scattering 0.3, fails T20 (1.086, 1.020) and T30 (2.007, 1.886)**. By R2-4 energetic T20
and T30 ship at factor 1. Every other quantity passes every round-2 cell.

**The shipped model on every validation cell** (observed over calibrated prediction, one-sided 95 %
lower bound in brackets; energetic T20 and T30, at M7's bound with no cell behind the factor, on
every energetic cell):

| Cell | SPL | EDT | T20 | T30 | C50 | C80 | D50 | Ts |
|---|---|---|---|---|---|---|---|---|
| V-R1 | 0.81 [0.76] | 0.61 [0.53] | 0.68 [0.64] | 0.71 [0.65] | 0.84 [0.79] | 0.81 [0.76] | 0.84 [0.78] | 0.70 [0.62] |
| V-R2 | 0.80 [0.75] | 0.75 [0.70] | 0.69 [0.65] | 0.63 [0.60] | 0.78 [0.72] | 0.82 [0.75] | 0.79 [0.72] | 0.73 [0.67] |
| V-R3 | 0.86 [0.81] | 0.85 [0.80] | 0.86 [0.81] | 0.69 [0.65] | 0.83 [0.78] | 0.84 [0.79] | 0.83 [0.78] | 0.87 [0.82] |
| V-R4 | 0.82 [0.77] | – | 0.77 [0.72] | 0.67 [0.63] | 0.89 [0.82] | 0.90 [0.83] | 0.89 [0.83] | – |
| V-R5 | 0.78 [0.74] | – | 0.83 [0.78] | 0.78 [0.73] | 0.86 [0.81] | 0.92 [0.86] | 0.86 [0.80] | – |
| V-R6 | 0.76 [0.72] | 0.88 [0.83] | 0.84 [0.79] | 0.77 [0.72] | – | – | – | – |
| V-E1 | 0.66 [0.62] | 0.41 [0.35] | 0.06 [0.06] | 0.03 [0.03] | 0.65 [0.61] | 0.53 [0.50] | 0.65 [0.61] | 0.78 [0.69] |
| V-E2 | 0.76 [0.72] | 0.55 [0.52] | 0.08 [0.08] | 0.05 [0.04] | 0.34 [0.31] | 0.18 [0.17] | 0.34 [0.31] | 0.80 [0.72] |
| V-E3 | 0.79 [0.74] | 0.64 [0.60] | 0.07 [0.07] | 0.03 [0.02] | 0.99 [0.93] | 1.00 [0.94] | 0.99 [0.93] | 0.85 [0.80] |
| V-E4 | 0.80 [0.75] | – | 0.12 [0.11] | 0.07 [0.06] | 0.62 [0.58] | 0.52 [0.48] | 0.62 [0.58] | – |
| V-E5 | 0.73 [0.69] | 0.56 [0.52] | 0.09 [0.09] | 0.05 [0.04] | – | – | – | – |
| V-E6 | 0.82 [0.77] | 0.95 [0.87] | 0.39 [0.37] | 0.29 [0.27] | 0.86 [0.80] | 0.92 [0.86] | 0.86 [0.80] | 0.95 [0.87] |
| W-E1 | 0.79 [0.74] | 0.82 [0.75] | 0.42 [0.40] | 0.33 [0.31] | 0.89 [0.84] | 0.85 [0.80] | 0.90 [0.85] | 0.92 [0.85] |
| W-E2 | 0.83 [0.78] | 0.84 [0.77] | 0.36 [0.34] | 0.28 [0.27] | 0.81 [0.76] | 0.85 [0.79] | 0.81 [0.76] | 0.83 [0.76] |
| W-E3 | 0.80 [0.75] | 0.83 [0.78] | 0.22 [0.21] | 0.21 [0.19] | 0.89 [0.83] | 0.95 [0.89] | 0.90 [0.84] | 0.79 [0.74] |
| W-E4 | 0.82 [0.77] | 0.88 [0.70] | 0.47 [0.44] | 0.62 [0.58] | 0.74 [0.69] | 0.69 [0.64] | 0.72 [0.68] | 0.90 [0.77] |
| W-E5 | 0.86 [0.81] | 0.76 [0.71] | 0.16 [0.15] | 0.08 [0.08] | 0.97 [0.90] | 0.96 [0.90] | 0.97 [0.90] | 0.75 [0.70] |
| W-E6 | 0.77 [0.72] | 0.95 [0.89] | 0.36 [0.34] | 0.27 [0.25] | – | – | – | – |

Energetic T20 and T30 at M7's bound on the calibration cells: 0.03 to 0.24, lower bounds all below
1 (`analysis.txt`). **Nothing in any validation cell claims less noise than SPPS showed.** How much
more it claims ("overstates", the inverse of the ratio): random 1.1 to 1.6; energetic SPL, EDT,
C, D and Ts 1.0 to 5.5 (C80 in V-E2); energetic T20 and T30 1.6 to 38.

## Rules 4, 5b and 6: the particle count a refusal names

**`1/√N`** (the seeds' spread times `√N`, lower count against higher, over the receiver-bands both
have, in joint standard errors; `**` beyond two):

| Pair | SPL | EDT | T20 | T30 | C50 | C80 | D50 | Ts |
|---|---|---|---|---|---|---|---|---|
| C-R1/C-R2 (150 k/1.5 M) | −1.7 | −1.0 | −0.0 | **−2.1** | −0.0 | −0.6 | −0.0 | −1.3 |
| C-R3/C-R4 (150 k/15 M) | +0.9 | +0.1 | −0.0 | +0.0 | – | – | – | – |
| C-R5/V-R4 (150 k/600 k) | +0.2 | – | −0.4 | +1.2 | +0.9 | +0.7 | +1.0 | – |
| C-E1/C-E2 (150 k/600 k) | +0.3 | +0.3 | −0.9 | −0.2 | +0.5 | +1.0 | +0.5 | +1.4 |
| C-E3/C-E4 (150 k/2.4 M) | −1.2 | **−2.5** | −1.4 | −0.6 | – | – | – | – |
| C-E5/V-E4 (150 k/1.5 M) | +0.5 | – | +1.9 | +1.4 | −0.5 | −0.3 | −0.5 | – |

By rule 4 as written, **random-mode T30 and energetic EDT name no count**
(`particle_count: scaling_not_confirmed`). Two of 36 comparisons beyond two standard errors is what
chance gives about half the time; the pre-registered rule has no correction for the number of
comparisons, and it is kept as written (Burhan's call below).

**The margin** (rule 5b): one run's model estimate scatters from seed to seed. Median relative
scatter over a cell's receiver-bands, largest over the calibration cells: random SPL 0.050, EDT
0.064, T20 0.271, T30 0.360, C50 0.053, C80 0.057, D50 0.053, Ts 0.054; energetic 0.047 to 0.058,
T30 0.103 (round 1) and 0.220 over round 2's thirteen cells (V-E2). Margins: random T20 1.4,
T30 1.5; energetic T20 1.2, T30 1.3; every other 1.1. The synthetic model shows the same scatter
(`crates/simpa-core/tests/params_noise.rs`, `the_bootstrap_against_the_true_spread_at_low_counts`:
T30's estimate scatters 35 % at 4,000 crossings, 9 % at 400,000; every other quantity about 5 %).

**The named counts on the pairs** (rule 6, with the shipped factors): of the receiver-band seeds
refused for noise at the lower count whose named count the higher count reached, the share of the
higher count's seeds that gave the value:

| Pair | Quantity | Refused | Name a count | Within the higher count | Given there |
|---|---|---|---|---|---|
| C-R1/C-R2 | EDT | 28 | 28 | 28 | 100 % |
| C-R1/C-R2 | T20 | 360 | 360 | 137 | 100 % |
| C-R3/C-R4 | EDT | 360 | 360 | 360 | 100 % |
| C-R3/C-R4 | T20 | 360 | 360 | 359 | 100 % |
| C-E1/C-E2 | T20 | 360 | 360 | 149 | 100 % |
| C-E3/C-E4 | T20 | 360 | 360 | 360 | 100 % |
| C-E5/V-E4 | T20 | 360 | 360 | 1 | 100 % |

Target at least 80 %; met. Most higher counts are well above the named ones, so this says the
counts are not too low, not that they are tight. The synthetic check
(`the_particle_count_a_refusal_names_brings_the_value_within_its_limit`): T20 refused at 10,000
crossings, ten new runs at the named count all pass, and at a quarter of it one of ten does.

## Tutorial 1 at upstream's default

`rooms/tutorial1_box.simpa` as shipped (27 third-octave bands, both receivers, 150,000 particles,
10 ms, 2 s, `trans_epsilon` 5), seeds 1 to 10: 540 receiver-bands per method. Before: the build of
`d0af0f6` (M7's model; the binary kept as `simpa-before-d0af0f6.exe`, sha256 `f62284f4…`); after:
this piece, the same runs read again (`tutorial1_at_upstreams_default`, `$SIMPA_T1_FROM`).

| Method | Quantity | Before | After | What refuses the rest (after) |
|---|---|---|---|---|
| random | T30 | 0 | 0 | noise 285, range not reached 253, early unresolved 1, missing moves 1 |
| random | EDT | 0 | 0 | early unresolved 540 |
| random | C80 | 479 | 472 | noise 66 (59 before), missing moves 2 |
| random | D50 | 540 | 540 | |
| energetic | T30 | 0 | 0 | missing moves 472 (the floor at ε 5), noise 68 |
| energetic | EDT | 0 | 0 | early unresolved 540 |
| energetic | C80 | 480 | 490 | noise 30 (40 before), missing moves 20 |
| energetic | D50 | 540 | 540 | |

Also C50 504 → 499 (random), 509 → 510 (energetic); T20 0 → 0 in both (random: noise 529;
energetic: noise 540, at M7's bound); SPL 540 throughout. At upstream's default the noise model is
not what holds the decay times back: EDT needs a finer step than 10 ms, energetic T30 a lower
floor than ε 5, and random-mode T30 at 150,000 particles really is that noisy (8.7 % relative in
the Lambert box of the same size, C-R1). The random-mode factors refuse 7 more C80 values: M7
claimed less noise for them than SPPS shows.

## The files

- `PREREGISTER.txt`: the design and rules, with the addendum before round 1's validation and the
  round-2 addendum, each committed before the numbers it governs were read.
- `calibration.json`: every cell's configuration (with each seed's wall time, `wall_s`) and, per
  quantity, a row per receiver-band with a value in every seed: `receiver`, `freq_hz`, each seed's
  value and model standard deviation before calibration (`seed_values`, `seed_model_sd`, seed 1
  first: every run's result), and from them `mean`, `observed_sd`, `predicted_sd` (`constant`, and
  `mean_energy` for energetic cells) and `predicted_scatter`. Six significant digits. The suite
  re-derives every factor, margin and flag from it (`crates/simpa-core/tests/
  params_noise_calibration.rs`).
- `calibration-cells.txt`: round 1's calibration cells read alone, the output that set the factors
  (`641a7ba`).
- `validation-round1.txt`, `validation-round2.txt`: each round's validation as read, with the
  factors of that round.
- `analysis.txt`: everything read with the shipped factors: every cell, the rules, the validation,
  `1/√N` and the named counts.
- Scratch, not committed (the main checkout's `target/agents/pm8-noise-scratch/`): the 320 runs
  (`runs/noise-cal-1790307822`, `runs/noise-cal-1790310131`) with each run's `report.json`,
  tutorial 1's 20 runs (`runs/tutorial1-default-1790308917`, `report.json` before and
  `report-after.json` after), the logs, and `simpa-before-d0af0f6.exe`.

**To reproduce:** `cargo test --release -p simpa --test noise_calibration -- --ignored --nocapture
--exact noise_calibration_runs` with `$SIMPA_EVIDENCE_ROOT` (and `$SIMPA_NOISE_CELLS` for round 2's
`W-` cells), then `noise_calibration` with `$SIMPA_NOISE_FROM` (both folders, `;`-separated),
`$SIMPA_NOISE_ROLES=calibration,validation,validation2` and `$SIMPA_NOISE_OUT`. The runs are
seeded; the receipt re-derives to the same numbers (the second read wrote a receipt identical to
the first).

## Open, for Burhan

- **Energetic T20 and T30 stay at M7's bound**, 1.6 to 38 times the spread. A factor cannot
  follow how the particles' energies spread apart; a model that sees it would need the second
  moment of the particles' energies per step, which SPPS does not write, or an estimate of each
  run's own bin-to-bin scatter. Both are new work with their own validation.
- **Rule 4** names no count for random-mode T30 and energetic EDT on one comparison each at
  −2.1 and −2.5 standard errors, with the other pairs of the same quantity at +0.0 and +1.2, and
  +0.3. A rule that corrects for the number of comparisons (36 here) would name them. Changing the
  rule after seeing the data is not done here.
- **Gate M7(c)'s exact free-field check is wider**: it weighs the level run by `mc_sd`, now 1.3
  times M7's for SPL, so it catches offsets above +0.044 or below −0.099 dB (was +0.028 and
  −0.083 dB), and its `p₀²` 0.1 dB-high partner is caught by 0.0004 dB (the test's own bound,
  0.1 dB, holds by the same margin). Deterministic on its seed, so it does not flicker, but thin; a
  partner at 0.15 dB, or more particles in the level run, would restore the margin.
- The factors rest on these 32 cells. A room unlike all of them (a coupled volume, a long
  specular tunnel) is outside what was measured; the random-mode factors' largest source, specular
  walls with absorption on one surface, is among them.
