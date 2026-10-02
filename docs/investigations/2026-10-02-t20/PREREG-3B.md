# T20, part 3b: SPPS rooms the bed never saw (round 2's G1-G7), pre-registered

2026-10-02. Committed before any T20 number on these runs is computed. This is the last part of T20.

## Why these runs, and the held-out call

Round 2 of the EDT test drew seven SPPS rooms (`../2026-09-27-edt-heldout/HARNESS-PLAN-2.md`, P13-P19, rooms G1-G7):

| Room | Shape |
|---|---|
| G1 | small |
| G2 | T60 3.0-3.2 s |
| G3 | two coupled rooms, a curved decay |
| G4 | L-shaped, with a blocked receiver |
| G5 | specular low hall |
| G6 | corridor 32 × 6.5 × 2.5 m |
| G7 | dead room |

Each room has K = 4 truth runs at 1 M particles and 0.1 ms, and tested runs at 150 k particles (G6 at 50 k), steps 1,
2 and 5 ms, 3 seeds, both modes. That is 172 runs on `B:\data\m8b-edt\round2\`. **No T20 has ever been computed on
them, and T20's code has not changed since.**

The metrics plan's crucible M1 made round-2 rooms DEV-only, because a group's fix could be tuned on them. For T20,
which has had no fix, that risk does not arise yet. **Technical call:** these runs are T20's acceptance set. If part
3b fails and T20's code is changed, the re-test uses freshly drawn rooms (M1's rule then applies in full).

They cover what parts 1 and 2 left open: a corridor (G6), a coupled curved decay (G3), T60 around 3 s (G2), and SPPS's
own noise path on geometries outside M8a.

## Definitions

- **Truth:** ISO 3382-1 cl. 6's T20 (round 2's checked exact-integral fit, `_mirror.line`, -5/-25 dB, 0 dB including
  the direct sound), on the **mean of the K = 4 truth histograms** of the receiver and band.
- **Truth uncertainty:** the same fit on each of the 4, with sd / √4. A row whose truth standard error exceeds
  0.5 % (1/10 JND) leaves the denominators, and is counted.
- **Value:** the product's `parameters.t20_s` for the tested run's receiver and band, from `simpa results --json` at
  the frozen build.
  - A value is used with its `mc_sd`.
  - **Any refusal, noise refusals included, is unanswered and counted by code.** This differs from M8a's bed rule:
    here a refusal is what the user sees.
- **Covered:** the truth lies in `value ± (2.5·mc_sd + 0.5 %·value)`.
- **Wrong-silent:** answered, not covered, and `|value / truth − 1| > 5 %`.

## Criteria (PLAN.md J2-J4; each mode separately)

- **J2:** coverage ≥ 90 % of answered rows, and wrong-silent ≤ 3 %.
- **J3:** no subgroup (room × receiver × step) with ≥ 20 answered rows has wrong-silent > 10 %.
- **J4:** ≥ 90 % of rows answered at 1 ms, for T60 ≤ 3.2 s (G2 included) and R ≤ 0.5 m. Refusals are reported by
  code.
- **Reported:**
  - per room, with G6 (corridor), G3 (coupled) and G2 (long T60) called out;
  - per step;
  - coverage at Z = 2 and 3;
  - rows over 1 JND but covered.

**T20 is done** (with parts 1, 2 and 3a) when part 3b passes.
