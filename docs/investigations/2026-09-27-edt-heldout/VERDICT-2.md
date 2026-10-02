# M8b EDT held-out test, round 2: VERDICT

2026-10-02 05:45. **Random mode: PASS on H1-H6 as pre-registered and implemented** (radii pooled; see "Burhan's
ruling needed"). **Energetic mode: FAIL on H3**, so EDT is not shown for Energetic runs (P11). Method:
`frozen2/method.py` v2.1, Z = 2.5, sha256 `029d90ac...`, frozen by `ADDENDUM-B1.md` (`98a8770`) before any round-2 data.

## What it means

- For Random-mode runs, the fixed method did what it promised on data it had never seen: every value it called `ok`
  was within 5 % of the truth except 7 of 520 on noisy particle runs and 7 of 1,954 on synthetic decays, its range
  held the truth 96.8 % of the time, and it answered 1,150 of 1,152 realistic particle-run cases. Upstream, on the same
  rows, was silently wrong 1,471, 2,793 and 3,034 times.
- It is cautious: in three of the seven rooms (G2, G5, G7) no Random-mode EDT read `ok`; they read `wide`, shown with
  their range. That is honest, and it is what v1.1 backlog item 58 is about.
- Energetic mode failed one subgroup: one far receiver in G4 (d = 13.67 m), 9 wrong rows, 8 of them low, with ranges
  narrower than the error.

## Criteria (RESULTS-2.md; every count recomputed by a sentinel seat, SENTINEL-2.md)

| | Random (decides) | Energetic (decides only Energetic runs) |
|---|---|---|
| H1 noise-free, wrong-silent <= 0.5 % | **PASS**: ISM 0 / 115, Synth 7 / 1,954 = 0.36 % | same sets |
| H2 SPPS coverage >= 90 %, wrong-silent <= 3 % | **PASS**: 96.8 %, 7 / 520 = 1.35 % | PASS: 97.4 %, 11 / 1,487 |
| H3 no subgroup > 10 % | **PASS**: 0 of 19 judged | **FAIL**: `G4 far 1 ms` 6 / 26, `G4 far 2 ms` 3 / 22 |
| H4 answers often enough (R <= 0.5 m, 1 ms, T60 <= 3 s; `receiver_too_large` out) | **PASS**: SPPS 1,150 / 1,152, ISM 677 / 705 (15 removed) | PASS |
| H5 below upstream | **PASS**: 7 v 1,471; 0 v 2,793; 7 v 3,034 | PASS: 11 v 1,401 |
| H6 attack | **PASS**: 0 of 11 classes reproducible (0 of 220 draws wrong-silent; max error of an `ok` row 2.4 %) | not separate |

Room features, confirmed on the truth runs before scoring: G2 T30 2.81 s (>= 2.5), G3 double slope at 6 of 8
receivers, G7 T30 0.204 s (<= 0.25). All met.

## Burhan's ruling needed

1. **"Every receiver radius" (H1, H3).** The plan, the build and both reviews read it as "computed over all radii, no
   radius filter" (unlike H4), and that is what was frozen and scored. PREREG-2's prose also says "a confidently wrong
   EDT at any radius fails the test", which reads stricter. Split by radius, Synth spheres over 1 m have 5 wrong of 290
   `ok` rows (1.7 %), EDT 5-7 % low: the same unseen-shelf mechanism as round 1, below the `receiver_too_large`
   threshold. No radius bins were pre-registered, so the strict reading cannot be scored without choosing bins after the
   fact. Options: accept the pooled verdict and record the large-sphere residual (default), or rule large spheres out
   of `ok` (a method change, so a round 3).
2. **Energetic mode.** Ship EDT for Random runs only, with Energetic EDT marked not yet validated (default, P11), or fix
   and test Energetic in a round of its own.

## Caveats, stated plainly

- **Thin denominators:** ISM-fresh H1 rests on 115 `ok` rows, 83 of them from one room (`rel:t1`); Random H2's `ok`
  rows come almost entirely from G1 and G4 (509 of 520).
- **A weak attack.** One attacker, 11 classes, all synthetic, mostly noise-free, steps 0.1-0.25 ms. Its one mechanism
  assumed the arrival time without the source delay; the harness, like the product, passes the delayed arrival
  (`ADDENDUM-B2.md`), and with it 9 classes produced errors under 0.005 %. H6 passes as specified; it is not strong
  evidence beyond H1-H5.
- **A port requirement from the attack:** the method must receive `t_arrival` including the source's emission delay,
  rounded up to the next whole step, exactly as the product's `arrival_s`. Passed without the delay, it is wrong-silent
  in the attacker's c01 regime (attacker's own sandbox measurement, about 13 of 20). The Rust port of v2.1 carries a
  test for it.
- The H6 runner was written after B1 and after H1-H5 were scored, before any class ran; it changes no B1-hashed file
  (`ADDENDUM-B2.md`; reviewed SHIP).

## What M12 may show

EDT from Random-mode SPPS runs, with its range always shown (row 9) and `ok` / `wide` / refusal-with-reason as v2.1
returns them, once v2.1 is ported to Rust and the port is shown bit-equal to `frozen2/method.py` on round 2's inputs.
Energetic-mode EDT is marked not yet validated. M8b is not done: T20, Ts, C50, C80, D50, SPL and STI still need their
tests (Burhan, 2026-10-02 01:59).
