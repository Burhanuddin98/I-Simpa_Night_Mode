# T20 part 2, Addendum 1 (before any scored row)

2026-10-02 10:22. These are technical calls (decision-log division of labour, 2026-09-27). They resolve points the
builder could not build as written from `PREREG-S1S2.md`. No scored S1 or S2 row had been computed.

Already seen before this addendum, and declared here:
- the say-NO subset runs (all five pass);
- 4 smoke rows: S2 errors -6e-6 and +2.2e-4, one S2 row `truth_truncated`, one S1 row at 2.7e-5, two S1 refusals;
- the say-NO subset's truncation rate of 9 of 102 rows.

## S1

1. **Double slope only.** `synth_fresh.py` draws decay ratios 1.5 and 5 and has no single-slope generator. Single
   exponentials are covered by say-NO (e). Draws: 200 per ratio.
2. **The drawn R is not used.** The R grid {0.1, 0.31, 0.5} supplies the half-width. A draw with d < 0.7 m is
   redrawn, so that the 0.5 m ball is not at the source.
3. **Two variants of each row:**
   - **long:** the truth's series, to 2.5 × the slow slope's T60. The product reads the same series. J1-a, J1-b
     and J4 are scored on it.
   - **short:** the generator's own run (0.3 to 3 × T60). Only J1-a is scored on it: a run too short for -25 dB
     must be refused, never answered more than 5 % wrong. Refusals are reported by reason.

## S2

4. **A fitted tail extension replaces the bare cut.** Eyring under-predicts T60 in elongated boxes: one box's
   truth T20 is 3.7 × its Eyring T60. For every row:
   - the fine echogram (to 2.5 × Eyring T60) is extended by an exponential tail;
   - the tail's slope is a least-squares fit to the last 10 dB of the cut series' Schroeder curve, and the tail runs
     to 60 dB below the energy at the cut;
   - truth and product read the same extended series.

   **Convergence:** the truth built from the 1.5 × cut plus its own fitted tail must agree with the truth built
   from the 2.5 × cut plus its tail, to 0.05 %. Otherwise the row is `truth_truncated`. The rule that more than 10 %
   `truth_truncated` makes a set INCONCLUSIVE stands.

   The extension is a model only beyond the cut. For an elongated box with a curved late decay it can be wrong,
   and the convergence check is what catches that.
5. **Readings confirmed:**
   - "Eyring at 1 kHz" includes ISO 9613 air.
   - J1-c also requires the point truth to converge.
   - At most 2 workers (about 6-7 GB each, estimated).
