# Held-out test of the 75-line EDT method: pre-registration

Written 2026-09-27 16:22 and committed **before** any held-out data exists. Nothing below may change once the data
is generated. If the method fails, it is fixed and tested again on a *different* fresh set, never re-scored on this one.

## What is tested

- The method is `frozen/method.py`, sha256 `462c37cf159d4d9bd8abadd8261f975fa47ace66f0d4cb4b1e6f4234c77fdf6e`. It is a copy of
  `target/agents/edt-simplify/final/method.py` (FINAL.md section 3), frozen with Z = 2, MIN_POINTS = 8, TAIL_SHARE = 0.02 and HW_FLOOR = 0.005.
  The evaluator verifies the hash before every run. A mismatch voids the run.
- Z = 3 is reported as a secondary result by changing only `Z`, to inform Burhan's call. It does not decide the verdict.
- Upstream's EDT, through the validated port in `target/agents/upstream-edt/`, is scored on the same rows.

## Held-out data (none of it may appear in any set the method was tuned or checked on)

The tuned-on corpus is: t1 golden, z3 (217), z3grid, the 3,600 real rows (C-R3/4/6, V-R2/6, C-E3/4/6 and the others),
the pm8 noise seeds, the ISM sample (rooms 'dead' and 'corridor' and their receivers), and every critique scan's generator
settings in `docs/investigations/2026-09-27-edt-simplify/critique/`.

1. **SPPS-fresh.** At least 6 rooms that are not in the corpus, solved by upstream SPPS. They must include:
   - one room with non-uniform absorption (a double-slope decay);
   - one room with T60 of at least 2.5 s;
   - one small room;
   - a coupled or L-shaped room if the geometry tools allow it;
   - receivers near (< 2 m), mid, and far (> 10 m where the room allows);
   - one receiver with a blocked direct path, if the geometry allows it.

   Tested runs:
   - steps of 1, 2 and 5 ms;
   - 150k particles, plus 50k in one room;
   - 3 seeds each;
   - SPPS's default run length, whatever it is (recorded).

   **Truth** is the sum of K ≥ 4 reference runs, each with at least 1M particles, a 0.1 ms step, and a run long enough to reach −60 dB, read with the corpus's own truth definition (the same function the existing ISM/real truth used; the evaluator names it).
   **Truth uncertainty** is the spread of the K references' individual EDTs divided by √K. Rows whose truth uncertainty exceeds 1 % are excluded and counted.
2. **ISM-fresh.** Rooms, absorptions and positions drawn with new seeds, disjoint from the ISM sample's rooms. The image set extends to at least the run length plus 20 %, so truncation of the truth cannot occur.
3. **Synth-fresh.** New random seeds and new families not in the critique's scans:
   - double slopes with rate ratios of 1.5 and 5 (the critique used 3);
   - direct-to-reverberant ratios from −20 to +10 dB;
   - steps of 1, 2, 5 and 10 ms;
   - runs of 0.3 to 3 T60.
4. **Attack.** Cases built by an attacker that has seen only `frozen/method.py`, its interface and the physics. It has not seen the tuning logs.

## Definitions

- **ok row:** `status == 'ok'`.
- **Usable row:** status is `ok` or `wide`.
- **Wrong-silent:** an ok row with |edt/truth − 1| > 0.05 (the EDT JND, ISO 3382-1 Annex A).
- **Covered:** truth lies within [edt_lo, edt_hi] (usable rows).

## Pass criteria (all must hold)

| | Criterion |
|---|---|
| H1 | Noise-free sets (ISM-fresh, Synth-fresh): wrong-silent ≤ 0.5 % of ok rows, on each set. |
| H2 | SPPS-fresh: coverage ≥ 90 % of usable rows, and wrong-silent ≤ 3 % of ok rows. |
| H3 | No subgroup with ≥ 20 ok rows has wrong-silent > 10 %. Subgroups: room × receiver distance class (near < 2 m, mid, far > 10 m) × step for SPPS-fresh and ISM-fresh, and family × step for Synth-fresh. |
| H4 | At a 1 ms step and T60 ≤ 3 s, usable ≥ 90 % of rows (SPPS-fresh and ISM-fresh). The ok share is reported, not gated. |
| H5 | Wrong-silent count strictly lower than upstream's on every fresh set. |
| H6 | Attack: no reproducible class of physically plausible SPPS inputs (≥ 5 instances) gives wrong-silent. The verdict judges plausibility and gives its reasons. |

Refusals (`run_too_short`, `step_too_coarse`, ...) are never counted as wrong. They are reported by reason and by set,
because they are what the user sees.

## Who decides

These criteria are a technical call (Jarvis, decision-log row 13's division of labour). Z = 2 against Z = 3, and refusing short runs against
lengthening the default run, stay Burhan's calls (FINAL.md section 5). This test informs them and does not make them.
