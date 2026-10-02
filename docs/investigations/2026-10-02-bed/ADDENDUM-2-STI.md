# Bed addendum 2: STI (written 2026-10-02 22:06, before any STI number is computed)

PREREG.md said STI "gets its own short addendum to this bed when it has product code". It has now (`e298a31`).
Everything in PREREG.md and ADDENDUM-1.md holds unless changed here.

## What is tested

The product's `sti` (male, shown; female, computed) per receiver, IEC 60268-16:2011, against the independent Python
reference `docs/investigations/2026-10-02-sti/reference/sti_reference.py` (hash in `reference/HASH.txt`, `9b695a9`),
written from the standard by another agent without reading the product's code, and reproducing Annex M's worked
example. Both are fed the same per-band energy responses, signal levels (Table A.4 at 60 dB(A) at 1 m, carried by the
band's room transfer) and noise levels; the reference is called once per sex with that sex's own spectrum.

**Tolerance:** 0.03 STI (STANDARDS-CHECK.md: not a limen; ed. 4 gives repeatability 0.02 and rating bands 0.04 wide).

## Sets and pass rules

- **A. Exact inputs.** S1 closed-form decays and S2 image-source ball echograms, each with the seven octaves 125 Hz to
  8 kHz, through the shim. Pass: product and reference agree within 0.01 (1/3 of the tolerance) in at least 99 % of
  rows where both answer, and never by more than 0.03; refusals listed by reason.
- **B. Default runs against high-count truths**, rooms G1-G7 with seven bands: tested at the new-project defaults
  (energetic, 150,000, 10 s, 1 ms, `trans_epsilon` 7, 125 Hz-8 kHz), seeds 4301-4303; truths energetic 1,000,000,
  0.5 ms, `trans_epsilon` 9, seeds 9301-9302, pooled. Reference STI on the pooled truth echograms. STI shows no range
  (`mc_sd` null), so **every answered value is scored against the tolerance: wrong-silent = |product − reference| >
  0.03**. Pass: no wrong-silent row; answered in at least 80 % of receivers per room. Truth uncertainty: half the two
  truth seeds' difference; a receiver above 0.01 is excluded and counted.
- **C. Specular boxes** (S-live, Mixed) with seven bands, seeds 4301-4303, reference STI on the exact image-source ball
  echogram. Same pass rule as B.

A miss is reported as it is, and a fix is tested on a fresh draw (seeds 4401-4403), as for build F.

## Not covered

Several sources at one receiver (the product refuses), directional sources, STIPA, measured rooms.
