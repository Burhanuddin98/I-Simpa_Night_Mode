# Bed addendum 4: two defects in my STI scoring, found AFTER the first C7 numbers

2026-10-02 23:05. Written after sets A-S1, B7 and C7 were first scored (`B:\data\m8b-bed\STI\`; the first B7/C7
scoring is kept as `summary.v1-refusals-excluded.json`). ADDENDUM-2 and ADDENDUM-3 are unchanged; this says what my
harness did wrong under them and what is reported because of it. Nothing here is a pre-registration.

1. **A product refusal was excluded as "truth uncertain" (a scorer bug, fixed).** When the product refused STI, its
   report carries no speech level for the refused band, so the reference (which takes the product's levels,
   ADDENDUM-3 item 9) could not be computed, the truth uncertainty was unknown, and the row was excluded. Every
   refusal was thus dropped from the answered share, which then read 100 % for a room the product answered 0 % of
   (C7 S-live, male). That defeats ADDENDUM-2's "answered in at least 80 % of receivers per room". Fixed: a row the
   product refused is never excluded; it counts as not answered (`stibed.exclusion`). B7 is unaffected (every row
   answered); C7 is rescored, and its first scoring's `pass_: true` is withdrawn.
2. **The C7 reference refuses its own exact echogram for length (reported both ways).** ADDENDUM-3 items 3 and 11
   apply the reference's cl. 8.3 a flags to the image-source series, which ends 60 dB below the energy at the cut
   (t20p2's tail). At the 1.5 x cut (the uncertainty's arm) the Mixed box's 8 kHz series is 1.25 s, under 1.6 s, so
   the uncertainty is unknown and the row excluded (30 of 36 rows on the first scoring). A noise-free series that has
   decayed is exactly the same series padded with zeros, which change neither MTF sum; the 1.6 s rule is a
   measurement rule. **The frozen scoring stands as the verdict.** Beside it, labelled post hoc, C7 is scored with
   the reference's series padded with zeros to the tested run's length from its arrival bin (`stibed.py c7 --pad`,
   `STI\c7-padded\`).
