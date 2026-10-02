# T20, part 3a (shown range per receiver, M8a runs): result

2026-10-02 13:03-13:36. Pre-registration `PREREG-3A.md` (`ca8a954`). Build `6435845` (per-receiver transport T20).
Re-read `B:\data\m8b-t20\part3a\20261002T110253Z\`; score `B:\data\m8b-t20\part3a\score\` (`harness/score3a.py`).
Reviewed by sentinel (`C:\tmp\t20-3a-sentinel\`).

## Verdict: PASS

- **P0:** every T30 number and every part-1 T20 number is bit-identical. Only `transports[].receivers_t20` is
  added, plus run metadata.
- **J2, each mode:** coverage is 3,840 of 3,840 rows in energetic and 3,840 of 3,840 in random. Wrong-silent is 0 in
  both.
- **J3:** no breach in 120 cell × receiver subgroups.
- **Rows:** 7,680 (40 cells × 10 seeds × 3 receivers × bands). All answered, none excluded. The truth's standard
  error is at most 0.145 % (median 0.04 %), against the 0.5 % limit.
- **The coverage is earned, not padded:**
  - Half-width / |error| has a median of 8.6 and a 1st percentile of 1.94.
  - With the 0.5 % term alone, 1,479 rows would fail, so `mc_sd` carries real width.
  - Coverage is 98.8 % at Z = 1, 99.99 % at Z = 2 and 100 % at Z = 3.
  - Five rows were checked by hand against `report.json`, receiver and band included.

## Part 1's lead is retired

Measured per receiver, the 20x8x4 energetic cells' mean errors run from -0.13 % to +0.10 % over 4 α × 3 receivers.
At R000 (2.8 m from the source), α 0.4, the mean is -0.02 %. Part 1's estimate of about -2.8 % came from scaling
the receiver-mean T20 by the T30 ratio, and it was wrong. The largest single row in those cells is 1.4 %, covered.

## The wide rows are refusals

- **The 98 rows:** covered but more than 1 JND off, all in 20x8x4 random air-off (α 0.4: 60, α 0.2: 29, α 0.1: 9),
  mostly at R1 and R2.
- **Their size:** |error| 4.8 to 23 %, and half-widths 10 to 160 % of the value.
- **All 98 are product refusals** (`monte_carlo_noise`). The product shows no T20 there, only the refusal. They
  are scored only because the bed judges a noise refusal's own value (SPEC section 4). A user never sees these
  numbers as answers.
- **The remaining limit:** in random mode at M8a's counts, the long room's T20 is too noisy to answer at R1 and R2,
  and the product says so.
