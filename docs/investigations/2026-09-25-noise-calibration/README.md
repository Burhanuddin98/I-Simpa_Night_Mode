# Calibrating the Monte-Carlo noise model against SPPS's own seeds (pre-M8, piece B)

**Asked for** (Burhan, 2026-09-24 17:45, M8 decision 3): make the noise model's standard
deviation match the seed-to-seed spread SPPS actually shows, per quantity and computation method,
never below it; have a refusal for noise name the particle count that would bring the value within
its limit; measure what that changes at upstream's default count on tutorial 1. The refusal limits
are Burhan's and are unchanged (EDT, T20, T30 2.5 % relative; C50, C80 0.5 dB; D50 0.025; Ts 5 ms;
SPL 0.5 dB).

**What ships is round 3** (below). Rounds 1 and 2 (`7132e42`) calibrated on 32 cells, all with
receivers of 0.31 m, at 150,000 particles or more, never the direct field alone; two reviews found
that the model then claimed less noise than SPPS shows outside that range (larger receivers,
direct-field SPL in energetic mode), that nothing stopped a run from leaving it, that energetic T20
and T30 still sat at M7's structure (30 to 40 times the spread in M8's rooms), and that the
`1/√N` rule withheld counts over deviations in the safe direction. Round 3 answers each.

**Result, in short (round 3).**
- **The model** is `k·√(1 + κ·n)·sd`: the bootstrap's standard deviation `sd`, a factor `k` and a
  correction for a particle crossing the same receiver at several times, `n` being the run's own
  crossings of the receiver per particle times the spread of the particles' lifetimes
  (`n1·cv2`, both from the run: rule R3-2 chose it over `n1` alone in both methods). Random
  mode's corrections are large (κ 1 to 3): with receivers of 0.9 m in a 60 m³ room at α 0.05
  (C3-R1, `n` up to 2.2) the seeds' spread is 1.7 (SPL) to 2.6 (T30) times the bootstrap's.
- **A domain per quantity** (R3-5 with A1): a value is given only with at least the particles per
  source and at most the `n` its quantity was calibrated at: random SPL, C50, C80 and D50 from
  5,000 particles, the decay times and Ts from 50,000; `n` up to 2.16 (random) and 2.13
  (energetic). Outside, it is refused `noise_uncalibrated`, naming the particles that reach the
  domain or the most the receiver radius may be (`n` grows as its square).
- **Energetic T20 and T30 by kind of wall** (A2): in bands whose every face is Lambert with
  scattering 1 and absorbs alike, at a mean absorption up to 0.4 (M8's rooms), **T30's factor is
  0.052 and T20's 0.098** of M7's structure, which overstated them 21 to 38 and 11 to 17 times
  there (13 cells); with the
  absorption concentrated on one surface, Lambert walls spread the particles' energies apart as
  specular ones do (T30 0.73 with κ 2; T20 1 after its fitted 0.52 failed a held-out room, F1);
  elsewhere factor 1, as before.
- **Validation** on 19 held-out cells (R3-8): every method, quantity and kind of wall passes on
  every cell but one: energetic T20 in Lambert rooms with concentrated absorption failed V3-E8
  (1.12 of the prediction, lower bound 1.04) and ships at factor 1 (F1). Pooled ratios of the
  seeds' spread to the shipped prediction: 0.12 to 1.06, every one-sided lower bound at most 0.97.
- **Every refusal for noise names a particle count** (R3-6, one-sided: no pair of cells showed a
  spread falling slower than `1/√N`). On the twelve pairs of cells that differ only in their
  count, the refusals whose named count the higher count reached gave their value there in 98.2 %
  to 100 % of that count's seeds (target 80 %).
- **Tutorial 1 at upstream's default** (150,000 particles, 540 receiver-bands per method; M7's
  model → rounds 1 and 2 → round 3): T30 0 → 0 → 0 and EDT 0 → 0 → 0 in both modes; C80 479 → 472
  → 458 (random), 480 → 490 → 484 (energetic); D50 540 throughout. The noise model is not what
  holds the decay times back there (the 10 ms step, the floor at `trans_epsilon` 5, specular
  walls, and random mode's real noise at 150,000 particles).

Every number below is measured; the rules were fixed before the numbers they judge were read
(`PREREGISTER.txt` and its addenda). Round 3: the design at `64a083d`, before any of its cells ran;
the addendum (A1, A2) at `50c7f4f`, after its calibration cells were read and before any V3 cell
was; its numbers at `4d2c6ef`, before any V3 cell was read. Rounds 1 and 2: the design at
`ecae322`; round 1's factors and the margin rule at `641a7ba`; round 2's design at `fbc45a8` and
its factors at `732951c`.

## Round 3

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
energetic: noise 540, at M7's bound); SPL 540 throughout. At upstream's default the noise model is
not what holds the decay times back: EDT needs a finer step than 10 ms, energetic T30 a lower
floor than ε 5, and random-mode T30 at 150,000 particles really is that noisy (8.7 % relative in
the Lambert box of the same size, C-R1). The random-mode factors refuse 7 more C80 values: M7
claimed less noise for them than SPPS shows.

## The files

- `PREREGISTER.txt`: the design and rules of every round, each addendum committed before the
  numbers it governs were read (round 3's design, its addendum A1 and A2, and its fallbacks F1 to
  F4 at the end).
- `calibration.json`: all 70 cells. Per cell: its configuration (with each seed's wall time,
  `wall_s`); per receiver-band (`bands`), each seed's crossings per particle and lifetime spread,
  the Lambert and uniform flags and the mean absorption; per quantity, a row per receiver-band
  with a value in every seed (`receiver`, `freq_hz`, each seed's value and model standard deviation
  before calibration, `seed_values` and `seed_model_sd`, seed 1 first, and from them `mean`,
  `observed_sd`, `predicted_sd` and `predicted_scatter`), and under `incomplete` every other
  receiver-band with each seed's value and model standard deviation or `null` where the seed
  gave none: every run's result. Six significant digits. The suite re-derives every number of
  the code from it.
- `calibration-round3.txt`: round 3's calibration cells read alone, the output that set its numbers
  (`4d2c6ef`); `validation-round3.txt`: its validation, the named counts on every pair and, per
  cell, the values through the shipped model and why the rest are refused; `analysis.txt`:
  everything read at the end (rounds 1 and 2's rules printed with their own factors).
- `calibration-cells.txt`, `validation-round1.txt`, `validation-round2.txt`: rounds 1 and 2's reads
  (`732951c` stripped the byte-order marks of the first two; `validation-round1.txt` had lost its
  first line, `CELL C-R1 …`, when cargo's build lines were cut from it, and has it back from the
  scratch output it was taken from; nothing else changed).
- `selection.py`, `selection.txt`: the selection by the refusal, on `calibration.json`.
- Scratch, not committed: rounds 1 and 2's 320 runs and tutorial 1's 20 runs in the main
  checkout's `target/agents/pm8-noise-scratch/` (`runs/noise-cal-1790307822`,
  `runs/noise-cal-1790310131`, `runs/tutorial1-default-1790308917`, with `report.json` read by
  `d0af0f6` and `report-after.json` by `7132e42`), and round 3's 380 runs, its logs and tutorial 1's
  reports read by round 3 in `target/agents/pm8-fix-noise-scratch/` (`runs/noise-cal-1790316455`,
  `tutorial1-round3/`).

**To reproduce:** `cargo test --release -p simpa --test noise_calibration -- --ignored --nocapture
--exact noise_calibration_runs` with `$SIMPA_EVIDENCE_ROOT` (and `$SIMPA_NOISE_CELLS` for a list of
cells), then `noise_calibration` with `$SIMPA_NOISE_FROM` (the runs' folders, `;`-separated),
`$SIMPA_NOISE_ROLES` (`calibration,validation,validation2,calibration3`, then with `validation3`)
and `$SIMPA_NOISE_OUT`. The runs are seeded; a second read writes the same receipt.

## Open, for Burhan

- **Energetic T20 and T30 outside uniform Lambert rooms** still sit at or near M7's structure (1;
  0.73 for T30 with Lambert walls and concentrated absorption), 1.6 to 38 times the spread. A
  tighter model there needs the spread of the particles' energies per step, which SPPS does not
  write.
- **The refused-resamples rule** (more than 10 of 200 resamples refusing the value) refuses
  energetic T30 in uniform Lambert rooms at α 0.2 to 0.4 below about a million particles although
  its calibrated noise is within the limit, because the resamples carry M7's structure's noise.
  M8's energetic cells run 1.5 million and more; the M8 bed should count what comes through.
- **M8's counts** in `docs/results.md`, "What M8 needs", were counted with M7's model; the bed
  should re-count with round 3's (random T30 now 1.6·√(1 + 3n) of the bootstrap; energetic T30 in
  M8's uniform Lambert rooms 0.052 of it).
- **The receiver radius**: random mode's correction is large (κ up to 3), so large receivers buy
  less than `R²` suggests, and past `n` 2.16 values are refused. Measured up to 1.4 m.
- **The addendum** changed two rules after round 3's calibration cells were read, before its
  validation (A1, A2 above). Both are disclosed; A2 is what gives M8's rooms their factor.
