# Bed addendum 5: set C7's reference length, ruled before the fresh draw (2026-10-02 23:33)

Written before any number of the fresh C7 draw (seeds 4401-4403, build H) exists.

**Why.** Under ADDENDUM-2/3, set C7's reference STI is computed on the image-source ball echogram cut at 2.5x (and,
for its uncertainty, 1.5x) the band's Eyring T60. In the Mixed box both cuts are shorter than IEC 60268-16's 1.6 s,
so the reference refused itself and all 18 Mixed rows were excluded (sentinel 23:31, `STI\c7\rows.jsonl`: `u` None).
The verdict on seeds 4301-4303 stands as scored (Mixed unscored); this addendum governs the fresh draw only.

**Rule for the fresh draw.** The reference echogram is extended past its cut to 10 s (the run length) by T20 part
2's fitted exponential tail (the same fit set C already uses for T20/T30), not by zeros; the reference's own length
check is applied to the extended series. Truth uncertainty: the STI difference between the echogram cut at 1.5x and
at 2.5x, each extended the same way; a row above 0.01 is excluded and counted. Everything else as ADDENDUM-2/3.
As a secondary, reported-only figure, the zero-padded variant (ADDENDUM-4's post-hoc mode) is also given.

**Pass** (unchanged): no wrong-silent row (|product − reference| > 0.03); answered in at least 80 % of receivers per
room and sex.
