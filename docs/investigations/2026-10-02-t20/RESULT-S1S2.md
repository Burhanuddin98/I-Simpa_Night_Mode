# T20, part 2 (exact references, S1 and S2): result

2026-10-02. Pre-registration `PREREG-S1S2.md` (`0d34044`), `ADDENDUM-1.md` (`1d3f927`). Harness `harness/` at `567f646`.
Shim `crates/simpa-core/tests/t20_shim.rs`. Hashes are in each `summary.json`.

- **Outputs:** `B:\data\m8b-t20\part2\{s1,s2}\`.
- **Runs:** S1 10:47-10:49. S2's first run (10:49-11:45) crashed at unit 197 on the harness's own `log10` of an empty
  fine bin (`s2.out.crash1.log`). No scored row had been written. The fix was tested (`test_tail_extension_survives_
  empty_last_bins`) and S2 re-ran in full, 11:46-12:48.
- **Reviewed:** by sentinel (scripts in `C:\tmp\t20-s2-sentinel\`).

## Verdict: PASS, as pre-registered, with one limit stated below

**S1, closed-form double-slope decays.** 400 draws × 3 radii × 3 steps, long and short runs.
- Long runs:
  - Worst |T20 / truth − 1| is 0.027 % at 1 ms, 0.054 % at 2 ms and 0.36 % at 5 ms.
  - J1-b: 100 % of rows within 0.5 % at every step.
  - J4: 100 % answered; 1 refusal (`early_unresolved`, 5 ms).
- Short runs (0.3 to 3 × T60), J1-a only:
  - 217 of 400 answered per cell, worst 0.55 %, no wrong-silent.
  - The rest were refused: `range_not_reached` 183 and `truncated` about 366 per step.
- Say-NO (a), (b), (d) and (e) pass.

**S2, image-source echograms**, 36 boxes, 1 kHz and 8 kHz, 318 units, 1,908 rows.
- J1-a: no wrong-silent anywhere. Worst |err| is 0.07 % at 1 ms, 0.30 % at 2 ms and 0.61 % at 5 ms (B07, 8 kHz).
- J1-b: 1 ms and 2 ms 100 % within 0.5 %; 5 ms 577 of 578.
- J4: 578 of 578 converged 1 ms rows answered, with no refusal in S2.
- J1-c: the ball truth and the ISO point truth agree within 0.13 %.
- No room is biased: the per-room mean error at 1 ms is at most 2e-4.
- Say-NO (b), (c) and (e) pass. (a) and (d) are S1-only, as the pre-registration places them.

## The limit: slow, curved 1 kHz decays in elongated rooms are not covered

- **Where the unconverged rows are:** 174 of 1,908 S2 rows (9.1 %) are `truth_truncated`, under the 10 % line, and
  **all 174 are 1 kHz** (18.2 % of that band). The 8 kHz band has none, because air shortens its decays.
- **Which rooms:** they come from the elongated boxes (aspect 2.3 to 6.0).
  - B03, B11 and B07 lose 1 kHz entirely. B03's truth T20 is 3.9 × its Eyring T60.
  - B04, B13, B21, B17, B12 and Mixed lose some.
  - 27 of 36 rooms keep every row.
- **The rule's letter:** the pre-registered INCONCLUSIVE rule is per set, and the set passes. By band it would not:
  1 kHz alone is at 18 %. That is stated here, not hidden.
- **What these rooms do test:** their 8 kHz rows are scored and pass (B07 holds the worst errors anywhere, 0.07 % at
  1 ms). Counting the 58 unconverged 1 ms rows as unanswered, J4 is 578 / 636 = 90.9 %.
- **What is missing:** the product's T20 on a slow, non-exponential 1 kHz decay in a corridor-like room has no
  converged exact reference here. The unconverged truths move by 0.06 % to 4.5 % between cuts.
- **Where it is carried:** part 3, with long SPPS runs and their own high-count truths in an elongated room. Part 1
  already has one, 20x8x4 (aspect 5), as a reported cell.

## What part 2 does and does not establish

- **The reference is independent physics:** an exact image-source echogram and closed forms, with no SPPS and no
  Lambert transport.
- **The algorithm comparison:** product and truth read the **same** series. The 1e-5 to 1e-4 agreement shows the
  product's T20 fit (Rust, `decay.rs`) computes ISO 3382-1's T20 on that series, against the exact-integral fit in
  Python. It is not a test of SPPS's physics; parts 1 and 3 test that.
- **The noise path** (bootstrap `mc_sd`, `monte_carlo_noise`) is not exercised here. Part 3 tests it.
