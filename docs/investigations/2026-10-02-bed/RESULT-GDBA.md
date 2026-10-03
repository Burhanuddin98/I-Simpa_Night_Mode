# G and dB(A) end to end on set C: result (ADDENDUM-6)

2026-10-03 12:26. Rules: `ADDENDUM-6-GDBA.md` (5832655, committed before any row was scored). Scorer
`harness/gdba.py`, tests `harness/test_gdba.py` (6 passed). Data: `B:\data\m8b-bed\C-score-GdBA\` (`rows.jsonl`,
`summary.json`, `freefield.jsonl`). References copied byte-identical from `C-score-F\references\` (36 files, sha256
compared), not recomputed. Nothing else under `B:\data\m8b-bed\` changed (file listing of `C\` and `C-score-F\`, size and
mtime, identical before and after).

## In plain words

On the two specular boxes against the exact image-source answer, **G and dB(A) were answered in every row, never wrong
without saying so, and the truth sat inside the shown range every time.** Worst error: G 0.18 dB, dB(A) 0.07 dB,
against a 1 dB limen. The product's free-field constant agrees with the one written from ISO 3382-1 to 0.0004 dB.

## Numbers

| | rows | excluded | scored | answered | wrong-silent | coverage | worst \|diff\| | mean diff | pass |
|---|---|---|---|---|---|---|---|---|---|
| G (`g_db`) | 108 | 0 | 108 | 100 % (54/54 each room) | 0 | 100 % | 0.182 dB | +0.001 dB | yes |
| dB(A) (`dba`) | 18 | 0 | 18 | 100 % (9/9 each room) | 0 | 100 % | 0.065 dB | +0.000 dB | yes |

All answered rows `ok`; no refusals. Worst G: Mixed R002 500 Hz seed 4103, 16.941 (16.657-17.225) vs truth 16.759.
Worst dB(A): Mixed R001 seed 4101, 66.409 (66.214-66.604) vs 66.343. Truth uncertainty (the reference's 1.5x-cut
check) at most 6e-5 dB, so nothing excluded. Median shown half-width: G 0.19 dB, dB(A) 0.13 dB.

Product values: the reports the C scorer already read (`C-score-F\reports`, build F, `917345c`). Re-made with the
newest build (`C:\tmp\nm-target-h`, built 10-03 10:36; into `C-score-GdBA\reports-nm-target-h\`): every G and dB(A)
value, lo and hi identical (max change 0.0), no status change.

## The free-field constant (ISO 3382-1 A.2.1), per band, never adjusted

Truth: `L10 = 10 lg(W rho c / (4 pi 10^2) / p0^2)`, W from the run's `config.xml` source band level, rho c of dry air
from the run's 20 °C and 101325 Pa (`P sqrt(1.4 / (287.05287 T))` = 413.2898). Product: band SPL minus G from the report
(rho c = `source_power_rho_c` / `band_power_w` = 413.249). Neither includes air absorption over the 10 m.

| band (Hz) | 125 | 250 | 500 | 1000 | 2000 | 4000 |
|---|---|---|---|---|---|---|
| Lw - L10, truth (dB) | 30.85015 | 30.85015 | 30.85015 | 30.85015 | 30.85015 | 30.85015 |
| Lw - L10, product (dB) | 30.85058 | 30.85058 | 30.85058 | 30.85058 | 30.85058 | 30.85058 |
| \|L10 product - truth\| (dB) | 0.00043 | 0.00043 | 0.00043 | 0.00043 | 0.00043 | 0.00043 |

The difference is the product's c = 343.2 m/s against sqrt(gamma R T) = 343.24 m/s. Band power in the report equals
`config.xml`'s to 4e-7 relative. ISO's "31" form would read 0.15 dB higher (`STANDARDS-CHECK.md` line 21).

dB(A): the product's weights `[-16.1, -8.6, -3.2, 0.0, 1.2, 1.0]` over 125 Hz-4 kHz equal the ones typed here from
IEC 61672-1:2013 Table 3, with no band left unweighted.

## What this does and does not test

- The reference SPL carries the product's own `W rho c` through its cached scale (ADDENDUM-1 item 6), so G's truth is
  SPL's truth minus an independently written constant: G's error here is SPL's error plus 0.0004 dB. This bed proves
  the G and dB(A) arithmetic, the constant and the range propagation end to end. It is not a second test of SPL.
- IEC 61672-1 is still not in hand (`STANDARDS-CHECK.md` line 11). The 34 typed A-weights were checked against the
  same standard's Annex E closed form at the exact base-10 frequencies: all agree to rounding (0.05 dB).
- Set C only (two specular boxes, 6 runs). Set B was not scored for G and dB(A) by this addendum.
