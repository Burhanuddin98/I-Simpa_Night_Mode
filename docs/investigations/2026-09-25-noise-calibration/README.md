# Calibrating the Monte-Carlo noise model against SPPS's own seeds (pre-M8, piece B)

**Asked for** (Burhan, 2026-09-24 17:45, M8 decision 3): make the noise model's standard
deviation match the seed-to-seed spread SPPS actually shows, per quantity and computation method,
never below it; have a refusal for noise name the particle count that would bring the value within
its limit; measure what that changes at upstream's default count on tutorial 1. The refusal limits
are Burhan's and are unchanged (EDT, T20, T30 2.5 % relative; C50, C80 0.5 dB; D50 0.025; Ts 5 ms;
SPL 0.5 dB).

**What ships is round 4** (below) on top of round 3. Rounds 1 and 2 (`7132e42`) calibrated on 32
cells with receivers of 0.31 m at 150,000 particles or more; round 3 (`50695f6`) added larger
receivers, fewer particles, the direct field alone and a domain, and split energetic T20 and T30
by kind of wall. Its review found that energetic T20 and T30 outside uniform Lambert rooms still
sat at M7's structure (14 times the seeds' spread for T30 on tutorial 1), that a value refused by
its resamples named no count, that named counts were never checked at a higher count for several
quantities, that the uniform-Lambert factors had no held-out cell above a mean absorption of 0.2,
and that no test held the inputs choosing and sizing a band's calibration to the run. Round 4
answers each.

**Result, in short (round 4).**
- **Energetic T20 and T30 outside uniform Lambert rooms read their noise from the series' own
  roughness** (a new structure, `params::noise::roughness_deposits`): late in an energetic decay a
  crossing brings far less than M7's full-energy deposit, and how much less shows in the series'
  bin-to-bin roughness. Calibrated on the 39 energetic cells of rounds 1 to 3: T20 `k` 1.2 with
  `κ` 5.25 from 15,000 particles (shipped 1.4, below), T30 `k` 1.3 with `κ` 4.75 from 150,000, `n`
  up to 1.37. On tutorial 1's room at 150,000 particles T30 is now overstated 2.4 times (M7's
  structure: 14 times), T20 3.2 times (9 times). **Validated on 11 held-out rooms, every one
  passing** (pooled ratios 0.17 to 0.85), and on 5 more after F6 (below).
- **The uniform-Lambert factors (0.052, 0.098) failed two held-out long rooms at a mean absorption
  of 0.4** (V4-E1, 20 × 4 × 3 m at 1 ms: T30 1.19 of the prediction, lower bound 1.10; V4-E4,
  30 × 4 × 3 m: T30 2.88, T20 1.10). The cube at 0.3 and the hall at 0.35 passed. By the
  pre-registered F6 their bound falls from 0.4 to 0.2, and uniform Lambert bands above 0.2 take the
  roughness entries. **M8's α 0.4 cells therefore run under the roughness structure**, which reads
  their T30 at 0.90 to 1.02 of the seeds' spread in the boxes (right, with no margin to spare;
  0.052 read 0.64 to 0.92 there) and holds in the corridor, where 0.052 claimed a third of the real
  noise. Energetic T20's roughness factor ships 1.4, not the fitted 1.2, to cover the one box at 0.4
  where 1.2 claimed less noise than it showed (C-E3; F3's remedy, after the validation).
- **A value its resamples refuse names a count** (R4-3): the first of 2 to 64 times the run's
  particles at which the model's own resamples clear it. On the pairs it was tested on, 100 % of
  the higher count's seeds gave the value for energetic T30 (C-E3/C-E4, V-E2/V4-E16 and
  V3-E5/V4-E17: 303, 360 and 360 refusals within reach, as validated; since F6 the first two
  rooms' α 0.4 bands take the roughness and are no longer refused) and random T30 97.1 %
  (C-R1/V4-R1, 48).
  Energetic C50, C80, D50 and Ts fell short (64.7 % on V-E2/V4-E16, where a receiver whose
  arrival sits at a 1 ms bin's edge keeps refusing them), so by F8 they name none.
- **Counts under the roughness structure are named from M7's structure** (a deviation, decided
  after the V4 pairs were read, in the safe direction): named from the roughness they fell short
  (43.5 % and 36.0 % of the higher count's seeds on V4-E13/V4-E14, 42.8 % on C-E5/V-E4), because the
  roughness also holds the curve's fine structure, which does not fall with more particles. Named
  from M7's structure they are high (tutorial 1's T20: 5.2 to 22 million particles), and no higher
  count of a pair reached them: safe by construction, not shown tight.
- **1/√N** holds one-sided on all 23 pairs (the largest unsafe `z` +1.78), so every refusal for
  noise names a count except where F8 withdrew it or no multiple up to 64 clears the resamples.
- **The inputs that pick and size the calibration are now tested** (the review's major): the
  harness reads every run again with the shipped build and stops on any band whose
  `crossings_per_particle`, lifetime spread or reading of the walls differs from its own
  computation or the cell's configuration (R4-6; none did, 940 runs); the suite checks every
  committed run's inputs against its own room table and series, and the walls a band is
  calibrated by against its materials rewritten five ways, each with a fault seam that makes the
  check say no.
- **Tutorial 1 at upstream's default** (150,000 particles, 540 receiver-bands per method):
  energetic T20 0 → **206** and T30 0 → **66** values (M7's model and rounds 1 to 3 gave none);
  random T30 and EDT and energetic EDT stay at 0, held back by the 10 ms step, the floor at
  `trans_epsilon` 5 and random mode's real noise, not by the model.

Every number below is measured; the rules were fixed before the numbers they judge were read, with
the deviations listed where they were not. Round 4: the design at `160368f` (before any V4 cell
ran and before the roughness was read with the project's decay code); its numbers at `1cf8339`,
before any V4 cell was read.

## Round 4

**Why** (`PREREGISTER.txt`, "ROUND 4"; the review of `50695f6`): energetic T20 and T30 outside
uniform Lambert rooms kept M7's structure (14 times the seeds' spread for T30 on tutorial 1); a
refusal by the resamples named no count, and said more particles need not help, although C-E3 gave
57 of 360 T30 values at 150,000 particles and C-E4 all 360 at 2,400,000; named counts were never
checked at a higher count for energetic T30, EDT, Ts and SPL, random T30 on one pair only; the
uniform-Lambert factors had no held-out cell above a mean absorption of 0.2 although their domain
ran to 0.4; and no test held the inputs that pick and size a band's calibration (the uniform flag,
the 0.4 bound, the lifetime spread) to the run: mutations making shipped values 10 to 14 times too
optimistic passed every test.

**The exploration, disclosed** (before the design was committed, on cells of rounds 1 to 3 only,
with a Python stand-in for T20 and T30, not the project's decay code;
`target/agents/pm8-fix-noise-scratch/explore_*.py` in the main checkout): with each bin's deposit
set to the variance of that bin over the ten seeds, an oracle no run has, the seeds' spread over the
bootstrap reads 0.9 to 1.2 at 10 ms and 0.31 m receivers, 1.4 to 1.8 at 1 ms and in the dead-floor
rooms, and 2.1 to 3.1 with receivers of 0.7 and 0.9 m: what M7's constant deposit misses in
energetic mode is the particles' energies spreading apart, bin by bin. One run's own roughness
reads 0.30 to 2.3 of the seeds' spread; M7's structure 0.027 to 0.54.

**The structure** (`params::noise::roughness_deposits`, `Structure::Roughness`): each bin's
deposit read from the series itself, over blocks of at least one receiver crossing
(`b = ⌈2R/c / dt⌉` bins: SPPS splits one crossing over the steps it spans): each block's
departure from its neighbours' geometric mean, `r_j = B_j/√(B_{j−1}·B_{j+1}) − 1`, over the blocks
within six of it (five at least), gives `d_j = min(d̄, mean(r²)/1.5 · B_j/(9/8))`. It applies to
energetic T20, T30 and the curvature in bands not uniform Lambert; everything else keeps round 3's
structure and numbers. The roughness also holds the true curve's fine structure (specular echoes),
which does not fall with more particles, so it overstates where that is strong, and more at higher
counts (tutorial 1's room: 2.4 times at 150,000 particles, 5.6 times at 1,500,000).

**Its calibration** (R4-1, R4-2, on every energetic cell of rounds 1 to 3, `calibration-round4.txt`,
committed at `1cf8339` before any V4 cell was read):

| Quantity | k | κ | Set by | Particles at least | `n` at most | Margin | Overstates (geometric mean) |
|---|---|---|---|---|---|---|---|
| energetic T20, not uniform Lambert | 1.2 (fitted; ships 1.4, below) | 5.25 | V3-E3 (10 × 10 × 10 m, 1.2 m receivers) | 15,000 | 1.367 | 1.5 | 1.82 over 24 cells |
| energetic T30, not uniform Lambert | 1.3 | 4.75 | V3-E3 | 150,000 | 1.367 | 1.6 | 1.98 over 23 cells |

Per cell (T30; "M7" is round 3's shipped structure there at factor 1; below 1 the prediction is
above the seeds' spread): tutorial 1's room C-E5 0.42 (M7 0.069), at 1 ms C-E6 0.55 (0.065), at
1,500,000 V-E4 0.18 (0.067); the dead floor C-E7 0.87 (0.14), W-E5 0.89 (0.08); the partly
scattering corridor W-E4 0.44 (0.62); Lambert with the floor at 0.9 C3-E8 0.47 (0.63). Where the
absorption is concentrated on Lambert walls, or the walls scatter partly, the roughness overstates
about as much as, or more than, round 3's 0.73 and 1 did there; everywhere specular it overstates
far less.

**Validation** (R4-4, once, with the numbers of `1cf8339`, `validation-round4.txt`): 24 V4 cells,
every method, quantity and split, over the rows inside the domain; pooled ratio of the seeds' spread
to the prediction, one-sided 95 % lower bound in brackets.

| Cell | Room, walls, N | T20 | T30 | Worst other quantity |
|---|---|---|---|---|
| V4-E1 | 20x4x3 Lambert 0.4, 300k, 1 ms | U 0.72 [0.65] | **U 1.19 [1.10] FAIL** | SPL 0.58 |
| V4-E2 | 10x10x10 Lambert 0.3, 150k | U 0.82 [0.77] | U 0.92 [0.85] | SPL 0.68 |
| V4-E3 | 20x8x6 Lambert 0.35, 300k | U 0.75 [0.68] | U 0.90 [0.83] | Ts 0.63 |
| V4-E4 | 30x4x3 Lambert 0.4, 150k | **U 1.10 [1.01] FAIL** | **U 2.88 [2.48] FAIL** | SPL 0.61 |
| V4-E5 | 5x4x3 specular 0.1, 300k | R 0.65 [0.61] | R 0.76 [0.71] | C50 0.72 |
| V4-E6 | 20x8x6 dead ceiling, 300k | R 0.57 [0.52] | R 0.45 [0.40] | C80 0.84 |
| V4-E7 | 10x10x10 Lambert, walls 0.3, 150k | R 0.83 [0.78] | R 0.77 [0.72] | D50 0.73 |
| V4-E8 | 30x4x3 dead floor, scattering 0.5, 300k | R 0.46 [0.41] | R 0.52 [0.46] | SPL 0.62 |
| V4-E9 | 6x10x3 dead floor, 600k, 1 ms | R 0.84 [0.78] | R 0.81 [0.75] | C80 0.78 |
| V4-E10 | 20x8x6 tutorial 1's materials, 300k | R 0.41 [0.38] | R 0.27 [0.24] | C50 0.71 |
| V4-E11 | 5x4x3 Lambert, ceiling 0.9, 300k, 1 ms | R 0.85 [0.80] | R 0.56 [0.53] | EDT 0.92 |
| V4-E12 | 6x10x3 specular 0.1, 300k, R 0.6 | R 0.29 [0.26] | R 0.35 [0.31] | C50 0.78 |
| V4-E13 | 6x10x3 tutorial 1's materials, 150k, ε 7 | R 0.40 [0.36] | R 0.47 [0.43] | SPL 0.62 |
| V4-E14 | the same at 1,200k | R 0.17 [0.16] | R 0.18 [0.17] | SPL 0.62 |
| V4-E15 | 20x8x6 dead floor, 2,400k | R 0.53 [0.47] | R 0.46 [0.41] | C80 0.84 |
| V4-E16 | 6x10x3 Lambert 0.4, 2,400k, 1 ms | U 0.87 [0.80] | U 0.64 [0.60] | SPL 0.69 |
| V4-E17 | 5x4x3 Lambert 0.2, 800k | U 0.89 [0.83] | U 0.55 [0.51] | SPL 0.70 |
| V4-E18 | 20x4x3 every surface 1, 1,200k | – | – | SPL 0.87 |
| V4-R1 to V4-R6 | random mode | 0.59 to 0.79 | 0.57 to 0.83 | SPL 0.91 [0.84] (V4-R1) |

U: the uniform-Lambert entries (M7's structure, 0.098 and 0.052); R: the roughness entries.
- **The roughness passes every held-out room** it was registered for (11 cells, T20 0.17 to 0.85,
  T30 0.18 to 0.81 at k 1.2 and 1.3, every lower bound at most 0.80).
- **The uniform-Lambert entries fail the two long rooms at 0.4** (V4-E1 T30; V4-E4 T20 and T30),
  and pass the cube at 0.3, the hall at 0.35 and the 6 × 10 × 3 m room at 0.4 with 2,400,000
  particles. A long room at high absorption spreads the particles' energies apart (paths along its
  length meet the walls less often), which the uniform-Lambert factor, fitted on two boxes, did not
  see: the reviewer's warning held. **F6** (pre-registered): their bound falls from 0.4 to 0.2, and
  uniform Lambert bands above 0.2 take the roughness entries. Under them the four V4 cells above 0.2
  and V4-E16 pass (`f6_bands.py`, `f6_bands.txt`: V4-E1 T30 1.05 [0.97], the closest).
- **F6 exposed one in-sample room**: the cells of rounds 1 to 3 with uniform Lambert bands above 0.2
  were in no fit of the roughness; under it C-E3 (5 × 4 × 3 m, α 0.4, 1 ms, 150,000) reads T20 1.07
  [lower bound 1.004] at k 1.2. **T20's roughness factor ships 1.4**, raised to cover that cell's
  upper bound (F3's remedy, taken after the validation and disclosed; it only raises the
  prediction, so every check above still passes). T30 reads 0.99 [0.93] there and 1.02 [0.95] in
  V-E2 (6 × 10 × 3 m at 0.4): at k 1.3 the roughness is about right at α 0.4 in the boxes, with no
  margin to spare.
- **Every other quantity and random mode pass every V4 cell** (closest: random SPL in V4-R1, 0.91
  [0.84]).
- **Coverage** (R4-4): every method, quantity and split has at least two V4 cells with six rows
  inside its domain, but energetic T20 and T30 under the uniform-Lambert entries: after F6 they
  hold up to a mean absorption of 0.2, and one V4 cell has six rows there (V4-E17, 5 × 4 × 3 m at
  0.2); round 3 checked them on V3-E1, V3-E5 and V3-E9. A shortfall, reported and not made up.

**`1/√N`** (R3-6 over all 23 pairs, round 4's 11 included): no pair shows a spread falling slower
than `1/√N` by more than two joint standard errors (the largest `z` in the unsafe direction +1.78,
energetic T20 on C-E5/V-E4; round 4's largest +1.69, W-E3/V4-E15); every flag stays on.

**Named counts at the higher count** (R4-3, R4-5): of the lower count's refusals for noise whose
named count the higher count reaches, the share of the higher count's seeds that give the value
(as registered) and that are not refused for their noise.

With the shipped code (`analysis.txt`; the suite re-derives it from `calibration.json`,
`a_named_count_brings_the_value_through_at_the_higher_count`), per pair, quantity and kind of count
(sd: from the standard deviation; res.: from the resamples), refusals within the higher count's
reach, and there the share of its seeds that give the value / that are not refused for noise:

| Pair (counts) | Quantity | Kind | Within reach | Given | Not refused for noise |
|---|---|---|---|---|---|
| C-R1/V4-R1 (150k/6M) | random T20 | sd | 355 | 100 % | 100 % |
| C-R1/V4-R1 | random T30 | res. | 48 | 97.1 % | 97.1 % |
| C-R3/C-R4 (150k/15M) | random EDT, T20 | sd | 357, 358 | 100 % | 100 % |
| C-R3/C-R4 | random T30 | res. | 87 | 100 % | 100 % |
| C3-R9/V4-R4 (50k/2M) | random T20, T30 | sd | 360, 217 | 100 % | 100 % |
| C-R5/V4-R2 (150k/6M) | random T20 | sd | 102 | 99.7 % | 99.7 % |
| C-R7/V4-R5 (500k/4M) | random EDT | sd | 159 | 71.9 % | 100 % |
| V3-R5/V4-R6 (150k/1.2M) | random SPL | sd | 120 | 100 % | 100 % |
| V3-E6/V4-E18 (150k/1.2M) | energetic SPL | sd | 53 | 100 % | 100 % |
| V-E2/V4-E16 (150k/2.4M) | energetic EDT, C80 | sd | 358, 109 | 100 % | 100 % |
| W-E3/V4-E15 (300k/2.4M) | energetic EDT | sd | 203 | 65.4 % | 100 % |
| V3-E5/V4-E17 (50k/800k) | energetic T30 (uniform Lambert) | res. | 360 | 100 % | 100 % |
| C3-E5/V3-E5 (5k/50k) | energetic T20 (uniform Lambert) | res. | 354 | 100 % | 100 % |
| C3-R5/V3-R8, C3-R6/C-R5, C3-E5/V3-E5, C3-E6/C-E5 | C50, C80, D50 | sd | 78 to 360 each | 100 % | 100 % |
| V3-R8/V4-R3 (50k/800k) | random T20 | sd | 7 | 28.6 % | 28.6 % |

Counts under the roughness structure (energetic T20 and T30 outside uniform Lambert rooms) are
named from M7's structure and no higher count reached them (V4-E13/V4-E14: 129 T20 and 92 T30
refusals, none named at most 1,200,000; C-E5/V-E4: 134 T20 refusals, none at most 1,500,000):
they are safe by construction, not shown tight. Before F6, the counts the resamples named for
energetic T30 in uniform Lambert rooms at α 0.4 gave the value in 100 % of the higher count's
seeds on C-E3/C-E4 (303) and V-E2/V4-E16 (360) (`validation-round4.txt`); since F6 those bands take
the roughness, whose resamples carry the run's own noise, and no longer refuse them.

- **From the resamples (R4-3)**: energetic T30 in uniform Lambert rooms 100 % on C-E3/C-E4 (303),
  V-E2/V4-E16 (360) and V3-E5/V4-E17 (360) as validated; random T30 97.1 % on C-R1/V4-R1 (48) and
  100 % on C-R3/C-R4 (87). Energetic C50, C80, D50 and Ts fell short on V-E2/V4-E16 (64.7 % of 17: at
  1 ms a receiver whose arrival sits at a bin's edge keeps refusing them at 2,400,000), so by **F8**
  they name no count from their resamples (`resampled_not_confirmed`).
- **From the roughness, as first built, the counts fell short** (V4-E13/V4-E14 T20 43.5 % of 100,
  T30 36.0 % of 89; C-E5/V-E4 T20 42.8 % of 109; C3-E6/C-E5 T20 77.7 % of 215): the higher count's
  roughness still holds the curve's fine structure, so the prediction there does not fall as
  `1/√N` although the seeds' spread does. **Counts are now named from M7's structure** (every
  deposit at most `d̄` and falling exactly as `1/N`, calibrated with the roughness's factor): an
  upper bound, so safe but not tight; no pair's higher count reaches the counts so named, so they
  are untested at a higher count. This was decided after the V4 pairs were read; no fallback for
  counts from the standard deviation had been registered.
- **EDT** falls short as registered on W-E3/V4-E15 (65.4 %) and C-R7/V4-R5 (71.9 %) but not for its
  noise: at the higher count every seed that does not give EDT is refused `truncated` or
  `missing_moves` (the series' end, the floor), which no particle count cures.
- Random T20 on V3-R8/V4-R3 gave 28.6 % on 7 refusals (fewer than R4-5's 10; reported).

**The production inputs** (the review's major; R4-6): the harness now reads every run again with
the shipped build (`$SIMPA_NOISE_REREAD`) and takes each band's crossings per particle, lifetime
spread and walls from the report, stopping on any that differs from its own computation (the
lifetime spread by trapezoids written again, the crossings from the series' own sum) or from the
cell's configuration: none did, over 940 runs. The suite:
`crates/simpa-core/tests/noise_inputs.rs` rewrites the committed energetic run's materials five
ways (uniform Lambert 0.2 and 0.5, Lambert with three absorptions, specular, scattering 0.5) and
checks the kind of wall and structure every band's model takes, with the fault seams
`UniformAbsorptionForced` and `UniformLambertBoundIgnored` (the reviewer's probe A) each moving its
case to the wrong entry; `cli_results`, `the_noise_inputs_of_every_band_are_the_runs_own`, checks
every band of every committed SPPS run against its own room table and series, with
`AliveShareScaled` (probe B) moving every band.

**Tutorial 1 at upstream's default** (the same 20 runs read by every round;
`tutorial1_at_upstreams_default` with `$SIMPA_T1_FROM`):

| Method | Quantity | M7 | Rounds 1–2 | Round 3 | Round 4 | What refuses the rest (round 4) |
|---|---|---|---|---|---|---|
| random | T30 | 0 | 0 | 0 | 0 | noise 285, range not reached 253, early unresolved 1, missing moves 1 |
| random | EDT | 0 | 0 | 0 | 0 | early unresolved 540 |
| random | C80 | 479 | 472 | 458 | 458 | noise 80, missing moves 2 |
| random | D50 | 540 | 540 | 540 | 540 | |
| energetic | T20 | 0 | 0 | 0 | **206** | noise 334, naming 5,200,000 to 22,000,000 particles (median 8,500,000; from M7's structure, so high: V4-E14 gave 245 of 360 at 1,200,000) |
| energetic | T30 | 0 | 0 | 0 | **66** | missing moves 472 (the floor at ε 5), noise 2 |
| energetic | EDT | 0 | 0 | 0 | 0 | early unresolved 540 |
| energetic | C80 | 480 | 490 | 484 | 484 | noise 36, missing moves 20 |
| energetic | D50 | 540 | 540 | 540 | 540 | |

Up to round 3 the noise model was one of the things holding energetic T20 and T30 back on
tutorial 1 (the round-3 README said it was not; that was wrong): its specular walls kept M7's
structure, which refused every T20 and the 68 T30 values the floor let through. The rest is held
back by the 10 ms step (EDT), the floor at `trans_epsilon` 5 (energetic T30) and random mode's real
noise at 150,000 particles.

**Selection by the refusal** (`selection.py` on `calibration.json`, round 4's numbers,
`selection.txt`): where the shipped model refuses some seeds of a receiver-band for their noise and
passes the
others, the passing seeds' mean sits below all ten seeds' mean: random T30 −0.82 % ± 0.13 % (60
receiver-bands), T20 −0.47 % ± 0.05 % (192), EDT −0.27 % ± 0.14 % (87); energetic T30 −1.23 % ±
0.13 % (190), T20 −0.75 % ± 0.10 % (236), EDT −0.03 % ± 0.17 % (43). The roughness structure lets
many more energetic decay times through than M7's did, so partly refused receiver-bands are now
common in energetic mode, and the bias with them: M8 must not average only what comes through
(`docs/results.md`, "Constraints on M8 from the noise calibration").

**What round 4 does not cover** (`monte_carlo.measured_on`): as round 3 (coupled volumes, long
specular tunnels, transmission, fitting zones; boxes of 60 to 1,000 m³; steps of 1 and 10 ms). The
uniform-Lambert entries are now held to 0.2; above it, and in every room not uniform Lambert, the
roughness decides, validated on the 16 rooms above.

## Round 3 (history, `50695f6`)

What round 3 shipped and why; round 4 keeps its numbers but for energetic T20 and T30 outside
uniform Lambert rooms (and in them above a mean absorption of 0.2), and the suite still re-derives
round 3's from the same receipt (`the_codes_round_three_numbers_are_the_rules_on_the_receipt`).

**Why, and what changed** (`PREREGISTER.txt`, "ROUND 3"): 38 new cells over seeds 1 to 10, 380
SPPS runs in 905 s on Grace (24 at a time): receivers of 0.5 to 1.4 m (up to 2.2 crossings of a
receiver per particle, against 0.25 in rounds 1 and 2), 5,000 to 50,000 particles, every surface
absorbing 1 (the direct field alone) and dead walls (walls 0.8, floor and ceiling 0.1), and Lambert
walls with the absorption on one surface (floor or ceiling 0.6, or the floor 0.9 and the rest
0.02). Round 3 calibrates on its 19 C3- cells and every cell of rounds 1 and 2 (22 random and 29
energetic), and validates on its 19 V3- cells, which no rule had seen. Each cell's configuration
and wall times are in `calibration.json`.

**The rules** (R3-1 to R3-10, and the addendum): `k` the largest one-sided 95 % upper bound over
the calibration cells of the pooled ratio of the seeds' spread to `√(1 + κ·n)·sd`, rounded up to
two digits, `κ` the value of 0, 0.25, …, 3 that overstates least; the bounds on effective degrees
of freedom (the rows' dispersion against the chi-square's and the correlation between the
receivers of one band, R3-3: review R2-m4); the domain; the one-sided `1/√N` rule (review
R2-m5); rule 5b's margins over round 3's cells; F1 to F4 for a failed validation.

**The addendum** (`50c7f4f`, written after the calibration cells were read and before any V3 cell
was; a deviation from the plan as first registered, disclosed here):
- *A1.* As first registered, random T20's factor came out 3.0, set by one receiver-band of C3-R6
  (15,000 particles, 9 degrees of freedom): the only one where every seed gave a T20 at that count,
  its spread 1.8 times the bootstrap's. One thin cell at a count where the quantity hardly ever
  comes through set the factor for every count (round 1's random T30 1.6 came the same way, from
  one receiver-band of C-R5). A1: a domain starts at the fewest particles of a calibration cell
  with six rows of the quantity, and the fit runs over the cells at or above it. Random T20 is
  1.5 with it; at 15,000 particles it is refused `noise_uncalibrated`, which C3-R6 says it should
  be.
- *A2.* As first registered, energetic T20 and T30 in Lambert bands came out 0.52 and 0.74, set by
  C3-E8 (floor 0.9, the rest 0.02) and C3-E9 (dead walls), while the ten Lambert cells whose every
  face absorbs alike read 0.027 to 0.047 of M7's structure for T30, receivers of 0.9 m and 50,000
  particles included. A2 splits "uniform Lambert" off, with the largest mean absorption of its rows
  (0.4) as a further bound: every particle meets the same absorption at every reflection there,
  so their energies spread apart only as their reflection counts do, and faster the more each
  reflection absorbs.

**The shipped numbers** (`params::noise::calibration`; `calibration-round3.txt` is the read that
set them; the suite re-derives every one from `calibration.json`,
`tests/params_noise_calibration.rs`):

| Method | Quantity | k | κ | Particles at least | `n` at most | Margin |
|---|---|---|---|---|---|---|
| random | SPL | 1.2 | 1.75 | 5,000 | 2.164 | 1.1 |
| random | EDT | 1.3 | 2 | 50,000 | 2.164 | 1.2 |
| random | T20 | 1.5 | 2.75 | 50,000 | 2.164 | 1.5 |
| random | T30 | 1.6 | 3 | 50,000 | 2.164 | 1.5 |
| random | C50 | 1.2 | 1.25 | 5,000 | 2.164 | 1.2 |
| random | C80 | 1.4 | 1 | 5,000 | 2.164 | 1.2 |
| random | D50 | 1.2 | 1.25 | 5,000 | 2.164 | 1.2 |
| random | Ts | 1.4 | 3 | 50,000 | 2.164 | 1.1 |
| energetic | SPL | 1.1 | 0 | 5,000 | 2.129 | 1.1 |
| energetic | EDT | 0.86 | 0 | 50,000 | 2.129 | 1.2 |
| energetic | T20, uniform Lambert (ᾱ ≤ 0.4) | 0.098 | 0 | 5,000 | 2.129 | 1.2 |
| energetic | T20, Lambert | 1 (F1; fitted 0.52) | 0 | 150,000 | 0.032 | 1.3 |
| energetic | T20, other walls | 1 | 0 | 15,000 | 0.819 | 1.2 |
| energetic | T30, uniform Lambert (ᾱ ≤ 0.4) | 0.052 | 0 | 50,000 | 2.129 | 1.3 |
| energetic | T30, Lambert | 0.73 | 2 | 150,000 | 0.032 | 1.3 |
| energetic | T30, other walls | 1 | 0 | 150,000 | 0.819 | 1.3 |
| energetic | C50 | 0.96 | 0.25 | 5,000 | 2.129 | 1.1 |
| energetic | C80 | 0.86 | 0.5 | 5,000 | 2.129 | 1.1 |
| energetic | D50 | 0.96 | 0.25 | 5,000 | 2.129 | 1.1 |
| energetic | Ts | 0.84 | 0.5 | 50,000 | 2.129 | 1.1 |

`n` is `n1·cv2` in both methods (R3-2: overstatement 1.357 against 1.607 for `n1` alone in random
mode, 1.924 against 1.942 in energetic mode). Energetic SPL's factor is now above 1 (review
R2-M2): the direct field alone (C3-E7, every particle stopped at its first surface, where energetic
mode runs random mode's transport) and a specular room with receivers of 0.9 m (C3-E4, which sets
it) are among the cells. The Lambert split's domain stops at `n` 0.032 because its four cells all
had receivers of 0.31 m; a Lambert room with larger receivers is refused its T20 and T30
(V3-E3's are, below).

**Validation** (`validation-round3.txt`): pooled ratio of the seeds' spread to the shipped
prediction over the rows inside the domain, one-sided 95 % lower bound in brackets; U, L and O the
kinds of wall for energetic T20 and T30; "outside": no row inside the domain (the value is
refused there); "–": no receiver-band where every seed gives the value.

| Cell | SPL | EDT | T20 | T30 | C50 | C80 | D50 | Ts |
|---|---|---|---|---|---|---|---|---|
| V3-R1 | 0.90 [0.83] | 0.82 [0.77] | 0.79 [0.72] | 0.65 [0.60] | 0.79 [0.74] | 0.75 [0.70] | 0.76 [0.70] | 0.75 [0.69] |
| V3-R2 | 0.71 [0.65] | 0.71 [0.64] | 0.65 [0.58] | 0.61 [0.55] | 0.64 [0.60] | 0.60 [0.55] | 0.64 [0.60] | 0.62 [0.56] |
| V3-R3 | 0.87 [0.82] | outside | outside | outside | 0.82 [0.75] | 0.73 [0.65] | 0.82 [0.75] | outside |
| V3-R4 | 0.82 [0.76] | – | – | – | 0.80 [0.73] | 0.72 [0.67] | 0.80 [0.73] | – |
| V3-R5 | 0.87 [0.82] | – | – | – | – | – | – | – |
| V3-R6 | 0.79 [0.74] | 0.75 [0.70] | 0.57 [0.53] | 0.96 [0.81] | 0.89 [0.81] | 0.64 [0.58] | 0.90 [0.82] | 0.68 [0.62] |
| V3-R7 | 0.85 [0.79] | 0.85 [0.80] | 0.83 [0.76] | 0.72 [0.67] | 0.77 [0.71] | 0.73 [0.67] | 0.77 [0.71] | 0.90 [0.83] |
| V3-R8 | 0.88 [0.82] | – | 0.57 [0.53] | – | 0.81 [0.77] | 0.70 [0.66] | 0.82 [0.77] | – |
| V3-R9 | 0.69 [0.63] | 0.77 [0.70] | 0.60 [0.54] | 0.56 [0.50] | 0.68 [0.63] | 0.65 [0.61] | 0.68 [0.63] | 0.65 [0.57] |
| V3-E1 | 0.78 [0.72] | 0.56 [0.52] | U 0.94 [0.86] | U 0.68 [0.61] | 0.98 [0.92] | 1.06 [0.97] | 0.96 [0.90] | 0.64 [0.59] |
| V3-E2 | 0.66 [0.62] | – | L 0.12 [0.11] | L 0.13 [0.11] | 0.64 [0.60] | 0.51 [0.48] | 0.65 [0.61] | 0.61 [0.51] |
| V3-E3 | 0.79 [0.75] | 0.58 [0.55] | L outside | L outside | 1.01 [0.92] | 1.01 [0.93] | 1.01 [0.93] | 0.79 [0.72] |
| V3-E4 | 0.76 [0.71] | 0.49 [0.46] | O outside | O outside | 0.75 [0.70] | 0.72 [0.68] | 0.75 [0.70] | 0.52 [0.49] |
| V3-E5 | 0.69 [0.65] | – | U 0.82 [0.77] | U 0.67 [0.63] | 0.52 [0.48] | 0.38 [0.36] | 0.52 [0.48] | – |
| V3-E6 | 0.95 [0.89] | – | – | – | – | – | – | – |
| V3-E7 | 0.70 [0.66] | – | L 0.28 [0.26] | L 0.38 [0.35] | 0.51 [0.47] | 0.37 [0.34] | 0.51 [0.47] | – |
| V3-E8 | 0.85 [0.80] | 0.89 [0.84] | L 0.58 [0.54] | L 0.69 [0.63] | 0.82 [0.74] | 0.85 [0.77] | 0.82 [0.75] | 0.91 [0.82] |
| V3-E9 | 0.67 [0.63] | 0.42 [0.39] | U 0.85 [0.79] | U 0.57 [0.52] | 0.86 [0.81] | 0.76 [0.71] | 0.86 [0.81] | 0.54 [0.50] |
| V3-E10 | 0.68 [0.64] | 0.67 [0.55] | O 0.53 [0.50] | O 0.52 [0.48] | 0.72 [0.66] | 0.70 [0.65] | 0.71 [0.66] | 0.77 [0.66] |

- **The one failure** (F1): energetic T20 in Lambert rooms with concentrated absorption, at the
  fitted 0.52, read 1.119 [1.040] on V3-E8 (6 × 10 × 3 m, floor 0.9, the rest 0.02, 1 ms steps; its
  calibration cell C3-E8 was a 20 × 8 × 6 m hall at 10 ms). By F1 it ships the other walls' factor
  1 in its own domain: 0.58 [0.54] there (the table's V3-E8 T20).
- **Close to 1, passing:** energetic C80 in V3-E1 (a 20 m corridor at α 0.05 with 0.9 m receivers)
  1.06 [0.97], and C50, C80 and D50 in V3-E3 1.01 [0.92]: the prediction is at the spread there
  within its uncertainty, not above it by a margin. Uniform-Lambert T20 in V3-E1: 0.94 [0.86].
- **Coverage** (R3-8): every method, quantity and kind of wall is checked on at least two V3 cells
  with six rows inside the domain, except energetic T20 and T30 outside Lambert walls, on one
  (V3-E10; V3-E4's 0.7 m receivers are outside the domain). Rounds 1 and 2 checked that split, at
  the same factor 1, on nineteen cells. A shortfall, reported and not made up.
- **Says no** (`tests/params_noise_calibration.rs`): the factors halved fail every V3 cell; one
  quantity's spread raised to 1.5 times its prediction fails that check alone; every κ set to 0
  fails the large-receiver cells (V3-R1, V3-R2 and V3-R9 in 4 to 5 quantities each; C3-R1 and C3-R3
  in 7); the fitted 0.52 fails V3-E8 as it failed.

**`1/√N`** (R3-6, one-sided, over all twelve pairs; `z` = higher count's spread·√N minus the
lower's, in joint standard errors on effective degrees of freedom): the largest in the unsafe
direction is +1.78 (energetic T20, C-E5/V-E4), so every quantity names a
count (random SPL on C3-R5/V3-R8 reads +1.77). Rounds 1 and 2's two-sided rule withheld random
T30 (−2.1, now −1.89 on effective degrees
of freedom) and energetic EDT (−2.5, now −2.18): both in the safe direction, where the spread falls
faster than `1/√N` and a count named from the lower count is, if anything, too high.

**Named counts on the pairs** (R3-9; `validation-round3.txt`): of the receiver-band seeds refused
for their noise at the lower count whose named count the higher count reached, the share of the
higher count's seeds that gave the value: EDT 100 % (10, 357), T20 98.2 % (11), 100 % (358, 290, 1,
2), T30 100 % (265), C50 100 % (360, 239, 359, 147), C80 100 % (206, 239, 360, 232), D50 100 % (359,
237, 79, 105). SPL and Ts were refused for noise at no lower count whose named count a higher count
reached. Values refused `noise_uncalibrated` at the lower count (random T20, T30 and Ts at 5,000
and 15,000; energetic T30 at 5,000 and 15,000) name where the domain starts, not a count that
brings the value within its limit, and are counted apart.

**Values through the shipped model, per cell** (`validation-round3.txt`, last table): for
instance energetic T30 in the uniform Lambert cells now comes through in every seed of C-E1, C-E2,
C-E4, V-E3, C3-E1, C3-E10, V3-E1 and V3-E9. Where it does not, the reason is often the resamples,
not the calibrated noise: C-E3 57 of 360 (5 × 4 × 3 m, α 0.4, 1 ms, 150,000), V-E2 0 of 360
(6 × 10 × 3 m, α 0.4, 1 ms, 150,000) and V3-E5 0 of 360 (α 0.2, 50,000) are refused because more
than 10 of the 200 resamples refuse T30 themselves, and the resamples carry M7's structure's
noise, 21 to 38 times the real one for T30 in these rooms. At 2,400,000 particles (C-E4) all 360
come through. That rule predates this piece; it is an open
point for M8 (below).

**Selection by the refusal** (review R2-M3; `selection.py` on `calibration.json`, output in
`selection.txt`): where the shipped model refuses some seeds of a receiver-band for their noise and
passes the others, the passing seeds' mean sits below all ten seeds' mean: random T30 −0.97 % ±
0.16 % (48 receiver-bands), T20 −0.45 % ± 0.07 % (118), EDT −0.27 % ± 0.14 % (87); energetic T20
−0.16 % ± 0.06 % (37), EDT −0.06 % ± 0.19 %, T30 −1.8 % ± 1.7 % (4). A seed whose decay reads long
has a larger spread of its own and is the one refused. That is more than the 0.2 % by which SPPS
and the independent transport agree, on which M8's cross-check rests, so M8 must not average only
what comes through in a cell where some seeds or receiver-bands are refused (`docs/results.md`,
"What M8 needs").

**Tutorial 1 at upstream's default** (`rooms/tutorial1_box.simpa` as shipped: 27 third-octave
bands, both receivers, 150,000 particles, 10 ms, 2 s, `trans_epsilon` 5, seeds 1 to 10; the same 20
runs read three times: by the build of `d0af0f6`, M7's model; by `7132e42`, rounds 1 and 2; and by
this piece, round 3, `tutorial1_at_upstreams_default` with `$SIMPA_T1_FROM`):

| Method | Quantity | M7 | Rounds 1–2 | Round 3 | What refuses the rest (round 3) |
|---|---|---|---|---|---|
| random | T30 | 0 | 0 | 0 | noise 285, range not reached 253, early unresolved 1, missing moves 1 |
| random | EDT | 0 | 0 | 0 | early unresolved 540 |
| random | C80 | 479 | 472 | 458 | noise 80, missing moves 2 |
| random | D50 | 540 | 540 | 540 | |
| energetic | T30 | 0 | 0 | 0 | missing moves 472 (the floor at ε 5), noise 68 |
| energetic | EDT | 0 | 0 | 0 | early unresolved 540 |
| energetic | C80 | 480 | 490 | 484 | noise 36, missing moves 20 |
| energetic | D50 | 540 | 540 | 540 | |

Also C50 504 → 499 → 498 (random), 509 → 510 → 510 (energetic); T20 0 in both (random: noise
529; energetic: noise 540: tutorial 1's walls are specular, factor 1); SPL 540 throughout. Round 3
refuses 14 more random-mode C80 values than rounds 1 and 2: its C80 factor rose from 1.2 to 1.4,
set by the dead-walls room (C3-R8), where the direct sound dominates (upper bound 1.4 of the
bootstrap there).

**What round 3 does not cover** (`monte_carlo.measured_on` in every report): coupled volumes, long
specular tunnels, transmission (energetic mode duplicates a transmitted particle, whose copies'
deposits are correlated) and fitting zones were not measured; the rooms are boxes of 60 to
1,000 m³; steps of 1 and 10 ms. The code cannot see those, so it says so beside every calibration
instead of guarding them. It does guard particles and crossings per particle.

## Rounds 1 and 2 (history, `7132e42`)

What rounds 1 and 2 shipped and why; round 3 replaces their numbers in the code, and the suite
still re-derives theirs from the same receipt (`rounds_one_and_two_numbers_are_their_rules_on_the_receipt`).

### The design

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

### Round 1: the seeds' spread over M7's model, cell by cell

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

### Rule 1: the structure for energetic mode

Calibrated on round 1's calibration cells, the constant structure overstated the seeds' spread by
1.653 (geometric mean over cells and quantities) and the mean-energy structure by 2.231 (T30's
factor 16, set by the dead-floor room, where the particles' energies spread apart the most). So
energetic mode keeps M7's constant deposit, and the mean-energy structure is not in the code
(`calibration-cells.txt` has both; `calibration.json` keeps its numbers as
`predicted_sd.mean_energy`).

### Rule 2: the factors, and what validates them

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

### Rules 4, 5b and 6: the particle count a refusal names

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
comparisons, and it was kept as written; round 3's one-sided rule R3-6 replaced it.

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
crossings, ten new runs at the named count all pass, and at a quarter of it one of ten did (with
round 3's margin of 1.5 the test checks a sixth of it: 1 of 10).

### Tutorial 1 at upstream's default (rounds 1 and 2)

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
energetic: noise 540, at M7's bound); SPL 540 throughout. At upstream's default EDT needs a finer
step than 10 ms, energetic T30 a lower floor than ε 5, and random-mode T30 at 150,000 particles
really is that noisy (8.7 % relative in the Lambert box of the same size, C-R1). (This README said
here, and in round 3, that the noise model was not what held the decay times back; for energetic
T20, and the 68 energetic T30 values the floor let through, it was: M7's structure at factor 1,
9 and 14 times the seeds' spread. Round 4's review caught it.) The random-mode factors refuse 7 more C80 values: M7
claimed less noise for them than SPPS shows.

## The files

- `PREREGISTER.txt`: the design and rules of every round, each committed before the numbers it
  governs were read (round 3's addendum A1 and A2; round 4's design, its fallbacks F5 to F9, and
  what was done after its validation, including the two changes no rule had registered).
- `calibration.json`: all 94 cells. Per cell: its configuration (with each seed's wall time,
  `wall_s`); per receiver-band (`bands`), each seed's crossings per particle and lifetime spread,
  the Lambert and uniform flags and the mean absorption, as the shipped build's report read them;
  per quantity, a row per receiver-band with a value in every seed (`receiver`, `freq_hz`, each
  seed's value and model standard deviation before calibration under M7's structure,
  `seed_model_sd`, and under the roughness, `seed_model_sd_roughness`, energetic mode, seed 1 first,
  and from them `mean`, `observed_sd`, `predicted_sd`, `predicted_scatter` and
  `predicted_scatter_roughness`), under `incomplete` every other receiver-band with each seed's value
  and model standard deviation or `null`, and for every cell of a pair, `judged`: how the report
  judged every receiver-band seed of every quantity (`g` given, `n<count>` or `r<count>` refused for
  noise naming a count from the standard deviation or the resamples, `b` and `x` naming none, `f` and
  `c` outside the domain, `o` refused for another reason). Six significant digits. The suite
  re-derives every number of the code from it.
- Round 4: `calibration-round4.txt`, its calibration read (every cell of rounds 1 to 3 with the
  build of `160368f`, before any V4 cell); `validation-round4.txt`, the V4 cells read with the
  numbers of `1cf8339` (R4-4 and R4-5 as registered); `analysis.txt`, every cell read at the end
  with the shipped code (after F6, F8, T20's cover and the counts from M7's structure); `f6_bands.py`
  and `f6_bands.txt`, the uniform Lambert bands above 0.2 under the roughness entries.
- Round 3: `calibration-round3.txt` (`4d2c6ef`) and `validation-round3.txt`.
- `calibration-cells.txt`, `validation-round1.txt`, `validation-round2.txt`: rounds 1 and 2's reads.
- `selection.py`, `selection.txt`: the selection by the refusal, on `calibration.json`.
- Scratch, not committed (`target/agents/` in the main checkout): rounds 1 and 2's 320 runs and
  tutorial 1's 20 runs in `pm8-noise-scratch/runs/` (`noise-cal-1790307822`, `noise-cal-1790310131`,
  `tutorial1-default-1790308917`); round 3's 380 runs in `pm8-fix-noise-scratch/runs/noise-cal-1790316455`;
  round 4's 240 in `pm8-fix-noise-scratch/runs/noise-cal-1790324769`, every run's report read again
  by round 4's builds in `pm8-fix-noise-scratch/reread-r4cal` (calibration), `reread-r4final`
  (validation) and `reread-r4final3` (shipped), tutorial 1's in `tutorial1-round4/`, and the
  exploration's `explore_*.py`.

**To reproduce:** `cargo test --release -p simpa --test noise_calibration -- --ignored --nocapture
--exact noise_calibration_runs` with `$SIMPA_EVIDENCE_ROOT` (and `$SIMPA_NOISE_CELLS` for a list of
cells), then `noise_calibration` with `$SIMPA_NOISE_FROM` (the runs' folders, `;`-separated),
`$SIMPA_NOISE_ROLES` (`calibration,validation,validation2,calibration3,validation3`, then with
`validation4`), `$SIMPA_NOISE_REREAD` (a folder for the reports this build writes) and
`$SIMPA_NOISE_OUT`. The runs are seeded; a second read writes the same receipt.

## Open, for Burhan

- **Named counts are safe, not tight, under the roughness structure**: named from M7's structure,
  tutorial 1's energetic T20 refusals ask for 5.2 to 22 million particles where 1.2 million gave
  most values. A tight count would need the roughness's fine-structure part told apart from its
  noise, which one run cannot do. The change was made after the V4 pairs were read.
- **Energetic T20's roughness factor, 1.4**, was raised after the validation to cover one in-sample
  box at α 0.4 (C-E3) that F6 sent to the roughness; T30 has no margin there (0.99 of the
  prediction). M8's α 0.4 cells now run under the roughness; the α 0.2 cells at the uniform-Lambert
  bound.
- **The uniform-Lambert factors failed long rooms at α 0.4** (a corridor read 2.9 times its
  prediction): uniform Lambert walls alone do not keep the particles' energies together when the
  room is long. M8's boxes are not long; the bound of 0.2 is the pre-registered consequence.
- **The refused-resamples rule** still refuses energetic T30 in uniform Lambert rooms at α 0.2
  below about a million particles although its calibrated noise is within the limit (the
  resamples carry M7's structure: V-E1, 20 × 4 × 3 m, 163 of 360 through at 300,000; V3-E5 none at
  50,000, all at 800,000); each such refusal now names the count at which they clear (borne out in
  100 % of the higher count's seeds on three pairs). At α 0.4 the roughness judges since F6, and
  C-E3 and V-E2 give all 360 T30 values at 150,000 (57 and 0 before). M8's energetic cells run 1.5
  million and more.
- **M8's counts** in `docs/results.md`, "What M8 needs", were counted with M7's model; the bed should
  re-count.
- **The receiver radius**: the corrections are large (κ up to 3 in random mode, 5.25 for energetic
  T20 under the roughness), so large receivers buy less than `R²` suggests; measured up to 1.4 m.
