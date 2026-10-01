# M8b EDT held-out test: VERDICT

2026-10-01 23:25. **FAIL.** The frozen 75-line method (`frozen/method.py`, sha256 `462c37cf…`, Z = 2) does not pass
PREREG.md. EDT is not shown in the product on this method. Per PREREG.md:4 it is fixed and tested again on a
different fresh set; run1 is never re-scored.

## What it means

- In very dead rooms (design T60 about 0.13-0.2 s) at a 1 or 2 ms step, the method reads EDT low, median -7 % on
  its wrong rows, while calling the row `ok` with a range of 2.4-4.4 % that excludes the truth. 83 of 84 ISM-fresh
  wrong-silent rows are low; Synth-fresh, a closed-form truth built independently, shows the same: 20 of 20 low,
  median -7.0 %. Every wrong ISM row has design T60 / step under 300; 0 of 395 rows at 300 or above are wrong.
- On SPPS particle noise the range is too narrow by about 1.3-1.4 times at the median (truth a median 1.33 half-widths
  out, misses on both sides, signed error median -1.6 %). Z = 3 widens it 1.5 times and still fails H2
  (Random: coverage 92.7 %, wrong-silent 3.78 % against <= 3 %), and H1 fails at Z = 3 too.
- It is far better than upstream on every set (H5), and it does not refuse its way out (H4).

## Criteria (run1, `RESULTS.md`; counts recomputed independently by a sentinel seat, all exact)

| | Result | Numbers (frozen, Z = 2) |
|---|---|---|
| H1 | **FAIL** | ISM-fresh 84 / 1066 = 7.88 %, Synth-fresh 20 / 2138 = 0.94 % (target <= 0.5 % each) |
| H2 | **FAIL** | SPPS-Random coverage 80.0 % (2169 / 2710, >= 90 %), wrong-silent 8.14 % (132 / 1621, <= 3 %). Energetic (decides only energetic runs, P11): coverage 74.6 %, wrong-silent 0.96 % |
| H3 | **FAIL** | 13 of 59 judged subgroups (Random): 9 SPPS, all at 1 ms but one at 2 ms; 4 ISM, all mid / 1 ms (`rel:deader` 21 / 25) |
| H4 | pass | Usable at 1 ms, T60 <= 3 s: SPPS 100 %, ISM 96.1 % |
| H5 | pass | Wrong-silent ours / upstream: SPPS-Random 132 / 1371, Energetic 25 / 1269, ISM 84 / 2641, Synth 20 / 2997 |
| H6 | not run | Burhan, 23:2x: skip the attacker round for this method, "save it for the fixed method". The verdict was already FAIL |

## Not yet ruled out

The sentinel judged the dead-room bias about 75 % likely to be the method and not the truths, from outputs alone:
both truths share Definition A's direct / reflected split, and a fault there at 1 ms would show the same way. The
diagnosis step reads `frozen/method.py` and `harness/m8b/truth.py` before any fix is designed.

## Next (Burhan, 2026-10-01, "Fix EDT first, then M12")

1. Diagnose the dead-room low bias and the narrow SPPS range in code, on run1's rows (read-only).
2. Fix, re-freeze, and pre-register a second held-out set with new seeds and rooms (PREREG.md:4).
3. Run it, score it, review it; then the attacker round on the fixed method; then a verdict.
4. Then the Simulate settings editor, then M12.

Run1 outputs: `B:\data\m8b-edt\results\run1\` (rows.csv.gz, counts.json, score/).
