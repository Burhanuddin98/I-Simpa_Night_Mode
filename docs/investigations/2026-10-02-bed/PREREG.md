# The one test bed (decision 39): T20, Ts, C50, C80, D50, SPL, G, dB(A) at product grade

Pre-registered 2026-10-02 18:40, before any number in it is computed. STI is not here: it has no product code yet and
gets its own short addendum to this bed when it does. EDT and T30 are already passed and appear only as regression.

## What "passes" means (decision 39, rows 37 (3) and 41)

For each metric, on the sets below:

1. **Exact inputs** (set A): the product's own metric code, fed noise-free echograms, answers within 1/10 of the
   limen in at least 99 % of answered rows, and no answered row is beyond the limen.
2. **Default runs** (sets B, C): no **wrong-silent** row, and **coverage** of at least 90 % of answered rows.
3. **Answers often enough**: the share answered (`ok` or `wide`) is reported per metric and room. Below 80 % for a
   metric at T60 <= 3 s is reported to Burhan as a finding before the metric is called done; it is not a silent pass.

Definitions. Limens (ISO 3382-1 Table A.1, `STANDARDS-CHECK.md`): T20 5 %, C50 and C80 1 dB, D50 0.05, Ts 10 ms,
SPL and G 1 dB. **Wrong-silent**: answered, `|value - truth| >` the limen, and the truth outside the shown
`[lo, hi]` (an `ok` row with no range counts its value only). **Covered**: the truth within the shown range widened by
1/10 limen. A row whose truth uncertainty exceeds 1/10 limen is excluded and counted.

## Sets

- **A. Exact inputs, no solver runs.** The T20 part 2 shim (`crates/simpa-core/tests/t20_shim.rs`) extended to print
  every metric. S1: the closed-form decays of T20 part 2 (direct step, gap, single and double slope; 1 and 10 ms).
  S2: the 36 image-source ball echograms of T20 part 2. Truths from the fine-grid echogram by ISO 3382-1 A.2.3,
  time zero at the direct sound's centre `r/c`, written in Python, not ported from the product.
- **B. Default runs against high-count truths.** Rooms G1-G7 (round 2). Tested: the new-project defaults (energetic,
  150,000 particles, 10 s, 1 ms, `trans_epsilon` 7), seeds 4101-4103, 21 runs. Truths: energetic, 1,000,000
  particles, 0.5 ms, 10 s, `trans_epsilon` 9, seeds 9201-9202, pooled, 14 runs; truth uncertainty from the two
  seeds. The product's C/D/Ts/SPL code was not tuned on these rooms (build D used only synthetic histograms and
  counted refusals on them; this bed's runs are new seeds).
- **C. Default runs against an independent reference.** Two specular boxes (scattering 0) from T20 part 2's S2,
  the new defaults, seeds 4101-4103, 6 runs, against the exact image-source ball echogram's metrics. This is the
  one set where the reference shares nothing with SPPS.
- **D. G and dB(A).** Code and unit tests (free field at 10 m reads G = 0 dB; a 1 dB shift in source power leaves G
  unchanged; the exact `rho c` form, not ISO's "+31"; IEC 61672-1 A-weights at octave centres; single-band and
  flat-spectrum sums), then G and dB(A) on sets B and C from the same truths.

## Order

Build D (C/D/Ts arrival check), then the shim and scorer (tests first), then G and dB(A), then the 35 runs, then
scoring and a sentinel before any number reaches Burhan. One `RESULT.md`.

## Not covered (backlog 61)

Coupled-room edge cases beyond G3, corridors, T60 > 3 s, receiver balls over 0.5 m, random mode at user counts,
measured rooms (BRAS).
