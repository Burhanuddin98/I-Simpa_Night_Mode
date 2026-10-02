# T20, part 3b (round 2's SPPS rooms G1-G7): result

2026-10-02 13:50-13:58. Pre-registration `PREREG-3B.md` (`add15dd`), `ADDENDUM-3B-1.md` (`d1a9e8c`). Scorer
`harness/score3b.py`. Product binary built at `6435845` (`C:\tmp\nm-target-t20p3`). Output `B:\data\m8b-t20\part3b\`,
log `B:\data\m8b-t20\part3b.out.log`. Reviewed by sentinel (`C:\tmp\t20-3b-sentinel\`).

## Verdict: INCONCLUSIVE, with J4 failing in both modes. T20 is not done.

**1. The truths cannot judge.**
- 5,004 of 6,912 rows (72 %) are excluded for truth se > 1.0 %, against Addendum 1's 25 % line.
- Round 2's K = 4 random-mode truths at 1 M particles scatter far more for T20 than for EDT. Truth se has a median of
  1.65 % and a maximum of 16.8 % (G6). By room the median is G1 0.7 %, G2 1.6 %, G6 2.6 % and G3 2.8 %.
- Under the original rule (se ≤ 0.5 %), 92 % of rows are excluded.
- The recompute checks: se = sd(4 singles) / 2 reproduces exactly.

**2. Random mode never answers at 150 k particles.**
- 0 of 1,152 rows at 1 ms are answered. 1,072 are refused `monte_carlo_noise` and 79 `range_not_reached`.
- Every one of the 144 random runs refuses T20, G1 included. By hand (G1, random, 1 ms): T20 about 0.33 s with sd
  0.011-0.019 (3-6 %), and the refusal asks for 0.6-2 M particles.
- The product is honest: it says why and how many particles it needs. But at this count a user sees no T20.

**3. Energetic mode answers 69 % at 1 ms** (796 of 1,152; J4 needs 90 %).
- Refusals: `monte_carlo_noise` 257, `truncated` 62, `missing_moves` 37.
- By room at 1 ms, 150 k: G3 108 of 144 unanswered (coupled; 102 noise), G5 98 (42 `truncated`: run length), G6 46
  (18 `missing_moves`); G1 0, G2 3, G7 2.

**4. Where energetic mode answers, it is right**, on the rows the truth can judge:
- coverage 856 of 872 (98.2 %), wrong-silent 1 (0.11 %);
- J2 and J3 hold for energetic.

**5. The one wrong-silent row is a bias, not a draw.** G2 R005 at 2 kHz, energetic, reads high on all three seeds:
+3.4 %, +4.9 % and +7.1 % (the last beyond 5 %, value 2.614 s, mc_sd 0.028, truth 2.440 s, se 0.019). The truth is
random-mode. This may be an energetic-mode bias of the kind behind EDT's energetic H3 failure. Unexplained.

## What it means for the product

T20 is accurate when it answers: parts 1, 2 and 3a, plus point 4 here. But at round 2's particle counts (150 k, the
counts the EDT test called user-realistic), it **refuses almost always in random mode and a third of the time in
energetic mode.** Decision-log row 37 (3) already ruled the direction: every metric shows a range, scored as EDT's,
which shows a value with a range marked `wide` rather than refusing for noise. T20 currently refuses on noise instead.

## Owed before T20 can be called done

1. **T20 on the row-37 (3) model:** a noise-limited value is shown with its range, not refused (EDT's `ok` / `wide`).
   This is a product change, so per crucible M1 it is re-tested on **freshly drawn rooms**.
2. **Truths that can judge:** energetic-mode high-count truths (the expected decay is the same physics), or more
   random truths. Sized from this result: random T20 sd is about 1.5 % per run at 1 M.
3. **Explain or bound the G2 R005 2 kHz energetic bias** before the fresh round.
4. **Run length in G5** (`truncated`) and `missing_moves` in G3/G6: check whether these refusals come from round 2's
   run lengths (pre-dating the 10 s default, decision 36) rather than from T20.
5. **Backlog:** the `monte_carlo_noise` refusal message prints an sd below the "limit" it then refuses on, e.g. "sd
   0.013 ... limit 0.025". It is unreadable as written.
