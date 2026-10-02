# Build C: what `trans_epsilon` 5 → 6, 7 (and 9) costs and buys, energetic SPPS

2026-10-02 17:30-17:38. Not pre-registered: a measurement of cost and refusals, not a gate.

## Set-up

- Rooms G1-G7 of the M8b round-2 held-out set, each run being round 2's
  `tested-G?-energetic-1.0ms-150k-3101/project.simpa` with only `solvers.spps.extinction_exponent` changed
  (written to SPPS as `trans_epsilon`). Energetic, 150,000 particles, 1 ms, 10 s, seed 3101.
- Solvers: round 2's exact binaries, `C:\tmp\nm-m8a-solvers` (sha-checked by `simpa run`). Reader: `simpa`
  built at `032cebf` (`C:\tmp\nm-target-b2`).
- ε 5, 6, 7: 21 runs, 4 at a time, interleaved so that each batch mixes ε. ε 9: a second batch of 7, 4 at a
  time, as the converged reference.
- Control for Monte Carlo noise: round 2's ε 5 seed 3102 runs, copied and re-read with the same build.
- Determinism: the fresh ε 5 seed 3101 G1 run is bit-identical to round 2's (42 solver files; `config.xml`
  differs only in its working directory).
- Data: `B:\data\m8b-t20\eps-cost\`. Harness: `harness/` here. Raw tables: `bias.txt`, `vs9.txt`.

## Cost

Wall time, 4 runs at a time, sum over the 7 rooms: ε 5 159.2 s, ε 6 189.5 s (×1.19), ε 7 223.2 s (×1.40).
Every room is within ×1.16-1.22 at ε 6 and ×1.35-1.49 at ε 7. This is what the physics predicts: a particle's
energy decays exponentially and it is dropped at 10^-ε of its start, so its lifetime, and the run time, scale
with ε (6/5 = 1.2, 7/5 = 1.4). It holds at any time step, since it is the particle's life, not the step, that
lengthens. ε 9 ran in a different batch (×1.6-1.8 against ε 5, not comparable under a different load; 9/5 = 1.8).

## What it buys

Answered (value shown, `ok` or `wide`) of 336 receiver-bands (7 rooms × 8 receivers × 6 bands):

| | ε 5 | ε 6 | ε 7 | ε 9 |
|---|---|---|---|---|
| T20 | 300 | 329 | 329 | 329 |
| T30 | 122 | 238 | 257 | 258 |

The floor refusals (`missing_moves`, `missing_not_cleared`) at ε 5 → ε 6 → ε 7:
T20 35 → 0 → 0 (G3 15, G4 6, G5 4, G6 10); T30 165 → 20 → 1 (G6 1 at ε 7). G6, the case named in the
handoff: T20 10 → 0 → 0, T30 29 → 6 → 1. The refusals left at ε 7 are `monte_carlo_noise` (G2 T30 48, G4 T30
30, T20 7) and `params_bad_arrival` on C50/C80/D50/Ts, which ε does not touch.

## Accuracy

Each ε against ε 9, on receiver-bands answered in both (`harness/vs9.py`, `vs9.txt`). Same seed, but the
pairs are not the same draw: EDT, which the floor 50 dB down cannot reach, moves between ε as much as between
seeds (room means up to ±1.6 %, `vs9.txt`), so changing the floor changes the random sequence.
Cells in one room share its geometry, source and seed and are not independent, so significance is judged by
room: the share of rooms whose mean shift is below zero, with a two-sided sign test.

| | ε 5 | ε 6 | ε 7 | control: ε 5, seed 3101 vs 3102 |
|---|---|---|---|---|
| T30 mean shift, cells | −0.37 %, 122 | −0.32 %, 238 | 0.00 %, 257 | −0.04 %, 115 |
| T30 rooms short | 5 of 5 (p 0.06) | 6 of 6 (p 0.03) | 2 of 6 (p 0.69) | 2 of 5 (p 1.0) |
| T20 mean shift | −0.25 % | −0.09 % | −0.27 % | +0.09 % |
| T20 rooms short | 5 of 7 (p 0.45) | 3 of 7 | 4 of 7 | 3 of 7 |

So ε 5 and ε 6 answer T30 short, about 0.3-0.4 % on average, in every room that answers it: inside the
product's 0.5 % floor limit and far inside the 5 % JND, with no ε 5 answered cell beyond |z| 2.43 of ε 9 (z
using both runs' `mc_sd`, which assumes independent draws). ε 7 shows no such shift. T20 shows none at any ε
(G3's −1.5 % to −2.2 % sits in every ε, ε 7 included, so it is G3's noise, not the floor). G5's −1.20 % over 14
ε 5 T30 cells is noise: all 14 are `wide` (sd 2-11 %). EDT, Ts, C50, C80, D50, SPL: no shift beyond the seed
control (`bias.txt`, `vs9.txt`).

Checked by a fresh sentinel (sonnet): set-up, determinism (G2, G5 also bit-identical), timings and the answer
counts confirmed; no warning in any run's stderr. Its objection to an earlier per-cell t statistic (cells
treated as independent, script unsaved) is why this section judges by room and `vs9.py` is saved.

## Reading

ε 7 for energetic runs costs 40 % more run time, more than doubles T30's answers (122 → 257 of 336) and
removes nearly every floor refusal and ε 5's small T30 bias. ε 6 costs 20 % and gets most of the answers
(238), but keeps a measurable T30 bias (−0.32 %) and 20 T30 floor refusals. ε 9 buys nothing over ε 7 here.

Not measured: random mode (whose particles die by chance, not at the floor); time steps other than 1 ms
(the cost argument above does not depend on it; the refusal counts might).
