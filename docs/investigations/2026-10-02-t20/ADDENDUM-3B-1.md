# T20 part 3b, Addendum 1 (before the scored run)

2026-10-02 13:46. Technical calls. Before this, only the builder's smoke had been seen: one tested run (G1, energetic,
1 ms) and G1's four truths, giving 48 rows, 36 excluded for truth se, and 12 answered (12 covered, errors -1.03 % to
+0.89 %).

1. **The truth is noisier than the pre-registration assumed.** G1's K = 4 random-mode truths at 1 M particles scatter
   1.5-2.5 % between runs. The truth's se has a median of 0.70 % and a maximum of 1.38 %, against the 0.5 % limit.
   The product's `mc_sd` there is about 0.3 %. As written, most rows would leave the denominators. So:
   - **Truth-se limit: 1.0 %** (1/5 JND) instead of 0.5 %.
   - **Covered:** `|value - truth| ≤ 2.5·sqrt(mc_sd² + se_truth²) + 0.5 %·value`. Two noisy estimates are compared
     with both uncertainties, at the same Z. The wrong-silent definition is unchanged (answered, not covered, and
     beyond 5 %).
   - **This test is weaker than PREREG-3B's.** Every row is also scored under the original rule (se ≤ 0.5 %, product
     `mc_sd` only), and that result is reported beside the gated one.
   - **If more than 25 % of rows are excluded** at the 1.0 % limit, part 3b is INCONCLUSIVE. More truth runs (about
     1.5-2 h of machine time) are then put to Burhan as a spend.
2. **J3 subgroups:** room × step, per mode, with receivers pooled. Room × receiver × step holds 18 rows outside G6, so
   it could never reach the 20-row floor. The worst room × receiver × step rate is reported.
3. **The builder's readings, confirmed:**
   - G6's 150 k and 50 k runs are pooled for J2 and J3, and split in the per-room report.
   - J4 uses the design T60 per band (round 2's H4 convention).
   - Refusals are counted by `why`, else by `code`.
   - A truth with no fit is excluded and counted.
   - Blocked receivers use round 2's `split(blocked=True)`.
