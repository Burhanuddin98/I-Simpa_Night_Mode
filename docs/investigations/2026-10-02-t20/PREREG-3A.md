# T20, part 3a: the shown range, per receiver, on the M8a runs, pre-registered

2026-10-02. Committed before any per-receiver T20 number is computed. Decision-log row 37 (3) says every metric
shows a range, scored as EDT's is. Part 1 could not score that range per receiver, because the twin kept the
transport's T20 only as a receiver mean (`RESULT-S4.md`). This part adds it, and settles the lead in 20x8x4's
energetic far receiver.

## Build and run

- **Bed twin:** the transport stores T20 per receiver with its standard error, from the same rays and the same
  `Decay::receiver` path that T30's per-receiver values use. Neither T30 nor part 1's fields change, and P0 of
  `PREREG-S4.md` is re-checked bit for bit.
- **Re-read:** the M8a runs as in part 1, into `B:\data\m8b-t20\part3a\`. No new solver run.

## Definitions (PLAN.md shared definitions; EDT's H2 and H3)

- **Row:** one seed, receiver and band, in every M8a cell, gated **and** reported, both modes.
- **Value:** the product's `t20_s`, either a value or a noise refusal's own value, as in SPEC section 4. Any
  other refusal leaves the row unanswered and counted by code.
- **Truth:** the transport's T20 at that receiver and band.
- **Truth uncertainty:** a row whose transport standard error exceeds 0.5 % (1/10 of the 5 % JND) leaves the
  denominators, and is counted.
- **Shown range:** `value ± 2.5·mc_sd`, where 2.5 is Burhan's Z, the same as EDT's. The product's T20 carries
  `mc_sd`.
- **Covered:** the truth lies in `value ± (2.5·mc_sd + 0.5 %·value)`.
- **Wrong-silent:** answered, not covered, **and** `|value / truth − 1| > 5 %`.

## Criteria

- **J2:** in each mode, coverage ≥ 90 % of answered rows, and wrong-silent ≤ 3 %.
- **J3:** no subgroup (cell × receiver) with ≥ 20 answered rows has wrong-silent > 10 %.
- **Reported:**
  - coverage and wrong-silent per cell and receiver;
  - the 20x8x4 energetic far receiver's measured error, which replaces part 1's estimate of about -2.8 %;
  - rows over 1 JND but covered (wide ranges);
  - coverage at Z = 2 and Z = 3, to show how sensitive it is.

**Part 3a passes** when J2 and J3 hold. It is the range half of T20. The fresh-room half (part 3b) is
pre-registered after this result.
