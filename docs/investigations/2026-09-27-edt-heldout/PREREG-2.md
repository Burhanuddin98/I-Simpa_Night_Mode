# Held-out test of the EDT method, round 2: pre-registration

Written 2026-10-02 00:20, **before** any round-2 data exists. Round 1 (`PREREG.md`, run1) failed H1-H3
(`VERDICT.md`, `02aca79`); its causes are in `DIAGNOSIS.md` (`d57bc49`). As `PREREG.md`:4 requires, the fixed method
is tested on a different fresh set, and run1 is never re-scored. Nothing below changes once round-2 data exists.
The method file's hash and every harness hash are recorded in `ADDENDUM-B1.md` at the freeze, before the first
round-2 run, as `ADDENDUM-A1.md` did for round 1.

## Burhan's rulings this round rests on (verbatim, 2026-10-01/02)

- 23:2x "Fix EDT first, then M12"; the attacker round: "Skip it, save it for the fixed method".
- 23:45 a large receiver sphere in a very dead room: "Refuse it, say why".
- 00:08 "Z = 3, wider ranges"; on the test's sphere range, "lets be realistic for a second, what does a professional
  expect?"; 00:15 "proceed" to the rule below: never-lies tested over every sphere, answers-often-enough tested on the
  configurations professionals run.

## What is tested

- `frozen2/method.py`: version 2.1, developed in `target/agents/edt-v21/` from v1 and `DIAGNOSIS.md`. Z = 3. It adds a
  refusal `receiver_too_large` when the receiver's half-crossing time h = R/c exceeds 0.01 of the fitted EDT, and a
  noise term that treats the bins a particle's crossing spans as correlated. Its sha256 is verified before every run;
  a mismatch voids the run.
- Upstream's EDT, through the same validated port as round 1, on the same rows.
- No secondary Z is reported: Z is decided.

## The corpus to keep clear of

Round 1's corpus (`PREREG.md`:14-16, `HARNESS-PLAN.md` P2) plus everything round 1 produced or scored: its seven SPPS
rooms (F1-F7), its 12 ISM-fresh rooms and their receivers, its Synth-fresh seeds and draws, its seeds P12, and run1's
rows. A room is fresh by `HARNESS-PLAN.md` P3's rule, applied against this widened list.

## Held-out data

1. **SPPS-fresh-2.** At least 6 new rooms, solved by upstream SPPS through the product, with the same required
   features as round 1 (`PREREG.md`:20-26): non-uniform absorption, T60 ≥ 2.5 s, a small room, a coupled or L-shaped
   room, near / mid / far receivers, a blocked direct path. In addition, **one dead room with design T60 at 1 kHz of
   0.25 s or less**, so the dead-room regime the diagnosis found is exercised on real SPPS output (at R = 0.31 m the refusal itself starts only below an EDT of about 0.09 s). The tested-run matrix,
   settings, truth construction (K ≥ 4 references of ≥ 1M particles at 0.1 ms) and truth-uncertainty exclusion are
   round 1's (`HARNESS-PLAN.md` P7-P19), with new seeds disjoint from P12. Random and Energetic are scored as separate
   sets, as P11 rules. The receiver radius is the product default, 0.31 m.
2. **ISM-fresh-2.** 12 rooms drawn as P21-P22 with a new seed: four new relatives of the original corpus ISM rooms
   (fresh factors, each a non-duplicate of round 1's relatives by P3) and eight drawn rooms. Receiver radii log-U[0.1,
   1.5] m, as round 1.
3. **Synth-fresh-2.** P23-P24's families and ranges, R log-U[0.1, 1.5] m, with a new seed.
4. **Attack.** Round 1's attacker protocol (P25-P26) against `frozen2/method.py`.

## Definitions

As `PREREG.md`:43-48 (ok, usable, wrong-silent > 5 %, covered), with `HARNESS-PLAN.md` P27-P28's denominators and
tolerances.

## Pass criteria (all must hold; Energetic decides only energetic runs, P11)

| | Criterion |
|---|---|
| H1 | Noise-free sets (ISM-fresh-2, Synth-fresh-2): wrong-silent ≤ 0.5 % of ok rows, each set, **every receiver radius**. |
| H2 | SPPS-fresh-2: coverage ≥ 90 % of usable rows, and wrong-silent ≤ 3 % of ok rows. |
| H3 | No subgroup with ≥ 20 ok rows has wrong-silent > 10 % (round 1's subgroups, P29), every receiver radius. |
| H4 | **Answers often enough, on what professionals run.** At a 1 ms step and design T60 ≤ 3 s, on rows with receiver radius ≤ 0.5 m: usable ≥ 90 % of rows, SPPS-fresh-2 and ISM-fresh-2 each. Rows refused as `receiver_too_large` leave H4's denominator and are reported by count, share and set. The ok share is reported, not gated. |
| H5 | Wrong-silent count strictly lower than upstream's on every fresh set. |
| H6 | Attack: no reproducible class of physically plausible SPPS inputs (≥ 5 instances) gives wrong-silent. |

Refusals are never counted as wrong (`PREREG.md`:62). Every refusal is reported by reason and set, including
`receiver_too_large` over all radii, because it is what the user sees.

## What changed from round 1, and why it is not a moved goalpost

- **Never lies (H1-H3) is unchanged and still runs over the full sphere range.** A confidently wrong EDT at any
  radius fails the test.
- **H4 is narrowed to the population it was written for.** Round 1's H4 asked whether the method ducks normal cases.
  A refusal of a 1-1.5 m sphere in a 0.15 s room is the behaviour Burhan ruled for and a professional expects, not a
  ducked case. The narrowing is written here before any round-2 data, and the full-range answer rate is still reported.
- **Z = 3 is decided**, by Burhan, not chosen from round-2 numbers.
- Development numbers on run1 shaped the fix (`target/agents/edt-v2/`, `edt-v21/`). They are not evidence of passing
  and are not quoted as such.

## Who decides

The criteria are a technical call; the rulings above are Burhan's. If round 2 fails, the fix and a third fresh set
follow the same rule, and Burhan decides whether EDT ships in v1 at all.
