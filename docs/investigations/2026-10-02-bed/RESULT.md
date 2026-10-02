# The one test bed (decision 39): result

2026-10-02, 18:32-20:58. Rules: `PREREG.md` (1ee3688, frozen), `ADDENDUM-1.md` (dcc5e11, before any scored row).
Two sentinel rounds (sonnet) checked every number below against the artifacts. Data: `B:\data\m8b-bed\`.

## In plain words

At the new-project defaults (energetic, 150,000 particles, 10 s, 1 ms, `trans_epsilon` 7), on seven fresh rooms and
two specular boxes with an exact reference, **no value of T20, Ts, C50, C80, D50 or SPL was wrong without saying so**,
on 1,008 receiver-band rows per metric per seed set, with every metric answered in at least 92 % of rows. **T30 had one
borderline miss in 833** after build F turned its noise refusals into ranges. On exact noise-free inputs no answered
value was ever beyond the just-noticeable difference, but **C50 and D50 miss the bed's own stricter bar** (99 % within
a tenth of the JND): the product was over-claiming its precision at the 50 ms edge. Build F made it say so (`wide`)
instead; the values themselves did not change.

## Builds this bed found and drove

| Commit | What | Receipt |
|---|---|---|
| `8f2a662` | New projects step at 1 ms (decision 11 had never reached the code; they ran at 10 ms) | 1003 passed |
| `1315b1d` | C50/C80/D50/Ts accepted an arrival whose ball leading edge is in the onset bin (were refused at 1 ms in 243 of 336 cells per metric on G1-G7) | exact on 144 synthetic cases; assay SHIP-WITH-FIXES |
| `ceaf137`, `8ec5cec` | G (exact rho c form) and dB(A) in the report | 1019 passed; assay SHIP-WITH-FIXES |
| `917345c` (build F) | T30 noise-limited values shown `wide` with a range (G2 T30 was answered 1/144); C/D straddle bin bracketed, `wide` when it can move the value past 1/10 limen | 1026 passed; assay SHIP-WITH-FIXES |

## Set A: exact inputs (product code fed noise-free echograms through `tests/bed_shim.rs`)

S1: 9,600 closed-form rows; S2: 1,272 image-source ball-echogram rows; 1 and 10 ms. Pooled (as PREREG criterion 1 is
written; per sub-set in brackets where it differs). Before build F, head `8ec5cec` (re-run on the committed tree:
9,351 rows bit-identical):

| | answered | worst error / limen | within 1/10 limen | beyond limen |
|---|---|---|---|---|
| T20 | 85.9 % | 0.40 | 99.1 % (S2 98.5 %) | 0 |
| Ts | 80.6 % | 0.12 | 99.6 % | 0 |
| C50 | 96.3 % | 0.50 | **97.8 %** (S2 85.1 %) | 0 |
| C80 | 95.0 % | 0.23 | 99.3 % (S2 96.3 %) | 0 |
| D50 | 97.5 % | 0.54 | **98.1 %** (S2 86.2 %) | 0 |
| SPL | 99.0 % | 0.17 | 99.7 % | 0 |

**Criterion 1: C50 and D50 FAIL as written; T20 and C80 also fail if read per sub-set.** Cause (reproduced,
`s2|B26|rec1|R0.1|8000Hz|10ms`: C50 2.589 vs 2.086): the bin straddling te was read with an in-bin log-linear model and
nothing bounded it; a reflection inside that bin breaks the model. 179 of S2's 189 C50 misses are at 10 ms, 10 at 1 ms.
Build F brackets that bin (wholly early, wholly late) and shows `wide` when the bracket passes the limit: `ok` rows
beyond 1/10 limen, S2: C50 189 → 0, C80 47 → 0, D50 176 → 0; S1: C50 45 → 12, C80 24 → 12, D50 24 → 3 (the S1 rest
are the short double-slope variant at 1 ms, a tail-model miss: backlog). Values are unchanged by build F, so
criterion 1 as written still fails for C50 and D50; what changed is that the product no longer calls those `ok`.
SPL on set A checks a sum against itself (ADDENDUM item 6); set C is SPL's real test.

## Set B: seven rooms, default runs against high-count truths

G1-G7 (round 2's rooms), tested 150,000 particles at the defaults, truths energetic 1,000,000, 0.5 ms, `trans_epsilon`
9, seeds 9201+9202 pooled; 1,008 rows per metric per seed set. Build F, **fresh seeds 4201-4203** (run after build F):

| | scored | answered | wrong-silent | coverage |
|---|---|---|---|---|
| T20 | 840 | 100 % | 0 | 99.9 % |
| T30 | 834 | 99.9 % | **1** | 99.9 % |
| Ts | 972 | 95.8 % | 0 | 100 % |
| C50 | 954 | 97.3 % | 0 | 100 % |
| C80 | 948 | 92.4 % | 0 | 100 % |
| D50 | 981 | 96.5 % | 0 | 100 % |
| SPL | 987 | 100 % | 0 | 100 % |

Seeds 4101-4103 (the first draw) under build F: the same, T30 0 wrong-silent of 834. Before build F (seeds 4101-4103):
0 wrong-silent everywhere, but T30 answered 72 % (G2 1/144, G4 54/144). Ranges are narrow, not padded: median half-
width on `ok` rows 0.14 (Ts) to 0.54 (C50) of a limen. No room answers under 80 % on any metric.

**The T30 miss**: G6 R007 1 kHz seed 4201, `wide` 0.385 s (0.362-0.408), truth 0.360 (+6.8 %, limen 5 %), the truth
0.0015 s below the range; T20 of the same run reads the same +7 % and its wider range holds the truth. Of 397 scored
`wide` T30 rows, 1 has the truth outside the range (0.25 %; a calibrated 2.5 sd range would let about 1.2 % out). The
stand-in range has no calibration of its own (backlog 65). **T30 `wide` ranges by room** (median half-width): G2 2.3 %,
G4 2.6 %, G6 8.2 %, G5 25 %, **G3 87 %** (the coupled room: honest, but the value says little).

**Exclusions** (truth uncertainty above 1/10 limen, counted, not scored): T20 168, T30 174, Ts 36, C50 54, C80 60,
D50 27, SPL 21 of 1,008, concentrated in G3 (coupled: 40 % of its T20 rows), G5 and G6. The coupled room is therefore
weakly tested for T20/T30; coupled rooms are a v1.1 edge case (backlog 61).

## Set C: specular boxes against the exact image-source answer (shares nothing with SPPS)

S-live and Mixed, 3 receivers, 6 bands, seeds 4101-4103, 108 rows per metric. Before and after build F: **every metric
100 % answered, 0 wrong-silent, coverage 100 %** (T30 105 of 108 scored, 3 truth-uncertain).

## G and dB(A)

In the report since `ceaf137` (results version 8, 9 after build F). G = SPL minus the same source's free-field level
at 10 m with SPPS's own rho c, so its error is SPL's; dB(A) sums band SPL + IEC 61672-1 octave weights. Unit-tested
(free field at 10 m reads 0 dB, Lw shift invariance, the exact 30.85 dB constant not ISO's 31, weights, refusal and
range propagation). Not scored end-to-end here beyond SPL (assay finding: the G = 0 test re-derives SPL locally).

## Verdict against the pre-registration

| Criterion | Result |
|---|---|
| 1 exact inputs: 99 % within 1/10 limen, none beyond the limen | none beyond the limen anywhere; **C50, D50 fail the 99 %** (T20, C80 too if per sub-set). Build F makes those rows `wide`, so no `ok` claim is false on S2 |
| 2 default runs: no wrong-silent, coverage >= 90 % | **met for T20, Ts, C50, C80, D50, SPL** on sets B (both draws) and C; **T30: 1 wrong-silent in 833** on the fresh draw |
| 3 answers often enough (>= 80 % per room) | met for every metric and room after build F |

## Open

- C50/D50 exactness on exact inputs at 10 ms (the old default) and the short double-slope S1 rows at 1 ms; Ts not
  bracketed (backlog 64); the T30 stand-in range uncalibrated (backlog 65); the G3 T30 range (±87 %).
- G and dB(A) end-to-end against set C's absolute ISM level.
- STI (no product code yet), then the Simulate settings editor (PQ3), then M12.

## STI (ADDENDUM-2 to -5), 2026-10-02 21:30 to 2026-10-03 00:08

Product STI `e298a31` (IEC 60268-16:2011 ed. 4; male shown, female computed; masking from band k-1), against an
independent Python reference written from the standard by another agent (`9b695a9`, hash-pinned, reproduces Annex M's
worked example to its printed precision). New projects compute 125 Hz-8 kHz (`d2c3e6b`, decision 43). Tolerance
0.03 STI (not a limen: ed. 4 repeatability 0.02, rating bands 0.04). Two sentinel rounds.

| Set | Rows | Result |
|---|---|---|
| A exact inputs (S1 closed forms + S2 ISM, 7 bands) | 24,236 answered by both | product = reference to 1e-13 in every row: **PASS**. 528 S1 rows answered by the product on series shorter than half the true T (length judged on the cut series' T) |
| B7 seven rooms, defaults, seeds 4301-4303 | 336 (both sexes) | 0 wrong-silent, max diff 0.0092, all answered: **PASS** |
| C7 boxes vs exact ISM, seeds 4301-4303 (frozen rules) | 36 | 0 wrong-silent where answered (max 0.0008), but **FAIL on answer rate**: S-live male 0/9 (1 particle of 150,000 alive at 10 s refused the band); Mixed unscored (the reference's own 1.5x cut failed the 1.6 s length rule; ADDENDUM-4) |

**Build H** (`8c241b2`, `18f43e1`; tests first; 1053 passed): a band with particles alive at the end is answered when
the unseen energy, moved to its worst case through every m(F), the speech level and the masking, cannot move STI by
more than 0.003; the length check takes T per band as the larger of T30/T20/EDT and the decay at the response's end
(refusing an end within 60 dB of the loudest window that is not decaying); backlog 66 and 67 closed. Re-read with
build H: C7 24 → 36 of 36 answered, B7 336 → 336, no answered value moved (360 rows, max change 0). Set A S1:
product-only answers 528 → 0; 168 more short double slopes now refused (the stricter direction).

**Fresh C7 draw** (seeds 4401-4403, after build H; ADDENDUM-5 written before it): 6 runs exit 0 (two with one
particle alive at 250 Hz, answered). Frozen ADDENDUM-3 scoring: 18 scored (S-live), 0 wrong-silent, max 0.00097, S-live
9/9 per sex. **ADDENDUM-5 scoring: 36/36 answered and scored, 0 wrong-silent, max diff 0.00225, 9/9 every room and
sex: PASS.** Deviation: ADDENDUM-5 asks for the reference tail extended to 10 s by its fit; the scorer padded with
zeros after the fitted tail's first T60 (the skipped energy is at most 1e-6 of the tail's, argued, not measured).

**Not done:** STI's Monte Carlo noise is not modelled (`mc_sd` null; every value is scored against the tolerance
instead, and none failed); several sources at one receiver are refused; directional sources treated as omni; the
60 dB end gate is a stated choice (40 dB would refuse 24 of the 168).
