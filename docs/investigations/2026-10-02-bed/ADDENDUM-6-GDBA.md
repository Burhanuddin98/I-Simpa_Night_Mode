# ADDENDUM 6: G and dB(A) end to end on set C (written 2026-10-03 12:21, before any G or dB(A) row is scored)

Closes the open item "G and dB(A) end-to-end against set C's absolute ISM level" (`RESULT.md`, Open; backlog 70).
M12's generated `beds/summary.json` marks G and dB(A) FAIL because no bed artifact scores them; this addendum is that
bed. No new solver runs: set C's six tested runs (`B:\data\m8b-bed\C\tested-*`) and its cached image-source
references (`B:\data\m8b-bed\C-score-F\references\`, copied, never recomputed) are the data.

## Truths, written from the standards, not from the product

- **G** per receiver and band (ISO 3382-1 A.2.1): the reference band SPL from the image-source answer minus the same
  source's free-field level at 10 m, computed from the source's band power and the run's air (rho c from the run's
  temperature and pressure), in Python. The product's constant is not reused; the scorer states the free-field level
  it computes and the product's, per band, so any constant mismatch shows.
- **dB(A)** per receiver: the energy sum of the reference band SPL plus the IEC 61672-1 octave A-weights over the
  bands the run computes (set C: 125 Hz - 4 kHz), weights typed from the standard's table, not from the product.

## Pass rules (the bed's, unchanged)

Limen 1 dB for both (G per PREREG; dB(A) takes SPL's 1 dB). **Wrong-silent**: answered, `|value - truth| > 1 dB` and
the truth outside the shown `[lo, hi]`. Pass: no wrong-silent row, coverage (truth within the range widened by 0.1 dB)
>= 90 % of answered rows, answered share reported per room. Rows: 2 boxes x 3 receivers x 6 bands x 3 seeds = 108 for
G; 2 x 3 x 3 = 18 for dB(A). A row whose truth uncertainty exceeds 0.1 dB is excluded and counted.

Output: `B:\data\m8b-bed\C-score-GdBA\` (`rows.jsonl`, `summary.json` in the shape of the other set summaries).
