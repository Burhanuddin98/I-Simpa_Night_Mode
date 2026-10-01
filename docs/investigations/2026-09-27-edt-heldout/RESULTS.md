# M8b EDT held-out test: RESULTS (H1-H5)

Run `run1`, scored by `harness/m8b/score_heldout.py` at commit `eff1d2f`. Frozen method sha256 `462c37cf159d4d9bd8abadd8261f975fa47ace66f0d4cb4b1e6f4234c77fdf6e` (Z = 2), checked before anything ran. H6 (the attacker round) is not part of this run. Z = 3 is a side report and decides nothing (PREREG.md:11). Wrong-silent is |edt/truth - 1| > 5% on an ok row with a truth; refusals are never wrong (PREREG.md:62).

## SPPS-fresh, Random (frozen, Z = 2)

- H2 **FAIL**: coverage 2169 of 2710 usable rows with a truth = 80.0 % (target >= 90 %); wrong-silent 132 of 1621 ok rows with a truth = 8.14 % (target <= 3 %).
- H3 **FAIL** (all sets in this criterion: 13 failing subgroups of 59 judged): SPPS-fresh subgroups: 57, judged 30, failing 9.
  - `spps|F2|far|1`: 5 of 27 ok rows wrong-silent (18.5 %)
  - `spps|F2|mid|2`: 4 of 32 ok rows wrong-silent (12.5 %)
  - `spps|F3|near|1`: 9 of 50 ok rows wrong-silent (18.0 %)
  - `spps|F4|mid|1`: 7 of 49 ok rows wrong-silent (14.3 %)
  - `spps|F5|far|1`: 6 of 21 ok rows wrong-silent (28.6 %)
  - `spps|F5|mid|1`: 9 of 42 ok rows wrong-silent (21.4 %)
  - `spps|F5|near|1`: 5 of 39 ok rows wrong-silent (12.8 %)
  - `spps|F6|mid|1`: 3 of 20 ok rows wrong-silent (15.0 %)
  - `spps|F6|near|1`: 19 of 101 ok rows wrong-silent (18.8 %)
- H4 **pass**: usable 1032 of 1032 rows at 1 ms and design T60 <= 3 s = 100.0 % (target >= 90 %); ok share 75.2 %; refused by reason none.
- H5 **pass**: wrong-silent, ours 132 against upstream 1371, on 2855 rows with a truth.

## SPPS-fresh, Energetic (frozen, Z = 2)

- H2 **FAIL**: coverage 2030 of 2720 usable rows with a truth = 74.6 % (target >= 90 %); wrong-silent 25 of 2617 ok rows with a truth = 0.96 % (target <= 3 %).
- H3 **FAIL** (all sets in this criterion: 4 failing subgroups of 78 judged): SPPS-fresh subgroups: 57, judged 49, failing 0.
- H4 **pass**: usable 1032 of 1032 rows at 1 ms and design T60 <= 3 s = 100.0 % (target >= 90 %); ok share 100.0 %; refused by reason none.
- H5 **pass**: wrong-silent, ours 25 against upstream 1269, on 2864 rows with a truth.

## ISM-fresh (frozen, Z = 2)

- H1 **FAIL**: wrong-silent 84 of 1066 ok rows with a truth = 7.88 % (target <= 0.5 %).
- H3 **FAIL**: ISM-fresh subgroups 87, judged 21, failing 4.
  - `ism|drawn:2|mid|1`: 4 of 37 ok rows wrong-silent (10.8 %)
  - `ism|drawn:6|mid|1`: 11 of 29 ok rows wrong-silent (37.9 %)
  - `ism|rel:deader|mid|1`: 21 of 25 ok rows wrong-silent (84.0 %)
  - `ism|rel:dead|mid|1`: 6 of 59 ok rows wrong-silent (10.2 %)
- H4 **pass**: usable 1246 of 1296 rows at 1 ms and design T60 <= 3 s = 96.1 % (target >= 90 %); ok share 45.6 %; refused by reason {'run_too_short': 8, 'step_too_coarse': 42}.
- H5 **pass**: wrong-silent, ours 84 against upstream 2641, on 3729 rows with a truth.

## Synth-fresh (frozen, Z = 2)

- H1 **FAIL**: wrong-silent 20 of 2138 ok rows with a truth = 0.94 % (target <= 0.5 %).
- H3 **pass**: Synth-fresh subgroups (family x step) 8, judged 8, failing 0.
- H5 **pass**: wrong-silent, ours 20 against upstream 2997, on 3946 rows with a truth.

## Rows, usable and ok shares, exclusions and refusals, by set and instance

Each cell: frozen (Z = 2) / Z = 3 (not deciding) / upstream.

| Set | rows | usable | ok | wrong-silent of ok with truth | truth outside range | benefit / regress vs upstream (frozen) |
|---|---|---|---|---|---|---|
| SPPS-fresh, Random | 3456 | 95.8 % / 95.8 % / 100.0 % | 50.3 % / 25.9 % / 100.0 % | 132 of 1621 / 33 of 873 / 1371 of 2855 | 541 of 2710 / 197 of 2710 / 2855 of 2855 | 463 / 458 |
| SPPS-fresh, Energetic | 3456 | 95.8 % / 95.8 % / 100.0 % | 91.6 % / 79.3 % / 100.0 % | 25 of 2617 / 18 of 2335 / 1269 of 2864 | 690 of 2720 / 330 of 2720 / 2864 of 2864 | 1074 / 77 |
| ISM-fresh | 3888 | 76.4 % / 76.4 % / 100.0 % | 30.2 % / 17.8 % / 100.0 % | 84 of 1066 / 42 of 631 / 2641 of 3729 | 276 of 2836 / 149 of 2836 / 3729 of 3729 | 342 / 448 |
| Synth-fresh | 4000 | 75.3 % / 75.3 % / 100.0 % | 53.5 % / 49.0 % / 100.0 % | 20 of 2138 / 12 of 1962 / 2997 of 3945 | 326 of 3013 / 209 of 3013 / 3945 of 3945 | 1329 / 159 |

### Truth exclusions, by reason and set (leave every truth-based count, P27)

| Set | truth_split_borderline | truth_truncated | truth_nan | truth_uncertain | total |
|---|---|---|---|---|---|
| SPPS-fresh, Random | 34 | 0 | 0 | 567 | 601 |
| SPPS-fresh, Energetic | 25 | 0 | 0 | 567 | 592 |
| ISM-fresh | 0 | 159 | 0 | 0 | 159 |
| Synth-fresh | 0 | 0 | 54 | 0 | 54 |

### Refusals, by reason and set (frozen / Z = 3 / upstream)

- SPPS-fresh, Random: step_too_coarse 145 / step_too_coarse 145 / none
- SPPS-fresh, Energetic: step_too_coarse 144 / step_too_coarse 144 / none
- ISM-fresh: direct_only 7, run_too_short 24, step_too_coarse 886 / direct_only 7, run_too_short 24, step_too_coarse 886 / none
- Synth-fresh: direct_only 62, not_decaying_at_run_end 13, run_too_short 566, step_too_coarse 346 / direct_only 62, not_decaying_at_run_end 13, run_too_short 566, step_too_coarse 346 / nan_or_nonpositive 1

## Z = 3 side report (not deciding, PREREG.md:11)

- SPPS random: H2 FAIL (coverage 92.7 %, wrong-silent 3.78 %); H3 FAIL (1 failing); H4 pass; H5 FAIL. ISM H1 FAIL, Synth H1 FAIL.
- SPPS energetic: H2 FAIL (coverage 87.9 %, wrong-silent 0.77 %); H3 pass (0 failing); H4 pass; H5 FAIL. ISM H1 FAIL, Synth H1 FAIL.

## The draws

- ISM-fresh rejections by reason (seed 2026100102): {'relative_near_duplicate': 0, 'relative_over_image_cap': 0, 'drawn_V_outside': 0, 'drawn_t60_125_outside': 0, 'drawn_near_duplicate': 1, 'drawn_over_image_cap': 0, 'receiver_R_no_room': 2, 'receiver_position_outside_class': 1602, 'receiver_R_position_cap': 0}
- Synth-fresh rejections by reason (seed 2026100101): {'d_near_source': 41}
- Rows built: {'spps': 6912, 'ism': 3888, 'synth': 4000}.

## score.evaluate's own report

# M8b EDT held-out test: the scorer's report

**In plain words.** In Random mode, the mode the PREREG's verdict is about, the frozen EDT method **fails**: it misses 5 of the six criteria (H1, H2, H3, H5, H6). In Energetic mode, scored as its own set, which decides only whether EDT is shown for energetic runs (P11), it fails (H1, H2, H3, H5, H6). With Z = 3, which is reported and decides nothing (PREREG.md:11), it would fail in Random mode and fail in Energetic mode. Over all 14,800 rows, it gave a confident value (ok) with a known truth on 7,442, and was more than 5 % off without warning (wrong-silent) on 261 of them (3.51 %). Upstream's EDT on the same rows was wrong-silent on 8,278 of its 13,393 (61.81 %). 1,406 rows have no usable truth and are left out of every count that needs one, though not of the ok and usable shares (P27): truth_nan 54, truth_split_borderline 59, truth_truncated 159, truth_uncertain 1,134. No rows were given for Attack, so every criterion that needs them fails: a set that answers nothing cannot pass (P27). The method file was checked before anything ran: sha256 462c37cf159d4d9bd8abadd8261f975fa47ace66f0d4cb4b1e6f4234c77fdf6e, as PREREG.md:8 pins it.

**What each criterion found** (the frozen method, Random mode; Energetic where it differs):

- **H1 fails.** Noise-free sets (ISM-fresh, Synth-fresh): wrong-silent ≤ 0.5 % of ok rows, on each set. ISM-fresh: 84 of 1,066 ok rows with a truth wrong-silent (7.88 %); Synth-fresh: 20 of 2,138 ok rows with a truth wrong-silent (0.94 %).
- **H2 fails.** SPPS-fresh: coverage ≥ 90 % of usable rows, and wrong-silent ≤ 3 % of ok rows. coverage 2,169 of 2,710 usable rows with a truth (80.0 %); wrong-silent 132 of 1,621 ok rows with a truth (8.14 %). Energetic: fails; coverage 2,030 of 2,720 usable rows with a truth (74.6 %); wrong-silent 25 of 2,617 ok rows with a truth (0.96 %).
- **H3 fails.** No subgroup with ≥ 20 ok rows has wrong-silent > 10 %. 152 subgroups, 59 judged (20 or more ok rows with a truth), 13 over 10 %: spps|F2|far|1 5 of 27; spps|F2|mid|2 4 of 32; spps|F3|near|1 9 of 50; spps|F4|mid|1 7 of 49; spps|F5|near|1 5 of 39; .... Energetic: fails; 152 subgroups, 78 judged (20 or more ok rows with a truth), 4 over 10 %: ism|rel:dead|mid|1 6 of 59; ism|rel:deader|mid|1 21 of 25; ism|drawn:2|mid|1 4 of 37; ism|drawn:6|mid|1 11 of 29.
- **H4 passes.** At a 1 ms step and T60 ≤ 3 s, usable ≥ 90 % of rows (SPPS-fresh and ISM-fresh). SPPS-fresh: 1,032 of 1,032 rows usable (100.0 %), ok 75.2 %; ISM-fresh: 1,246 of 1,296 rows usable (96.1 %), ok 45.6 %. Energetic: passes; SPPS-fresh: 1,032 of 1,032 rows usable (100.0 %), ok 100.0 %; ISM-fresh: 1,246 of 1,296 rows usable (96.1 %), ok 45.6 %.
- **H5 fails.** Wrong-silent count strictly lower than upstream's on every fresh set. SPPS-fresh: 132 against upstream's 1,371 on 2,855 rows with a truth; ISM-fresh: 84 against upstream's 2,641 on 3,729 rows with a truth; Synth-fresh: 20 against upstream's 2,997 on 3,946 rows with a truth; Attack: 0 against upstream's 0 on 0 rows with a truth. Energetic: fails; SPPS-fresh: 25 against upstream's 1,269 on 2,864 rows with a truth; ISM-fresh: 84 against upstream's 2,641 on 3,729 rows with a truth; Synth-fresh: 20 against upstream's 2,997 on 3,946 rows with a truth; Attack: 0 against upstream's 0 on 0 rows with a truth.
- **H6 fails.** Attack: no reproducible class of physically plausible SPPS inputs (≥ 5 instances) gives wrong-silent. 0 classes, 0 reproducible (5 or more of 20 draws wrong-silent), 0 of those not called implausible by 3 of 3 judges.

## Criteria (PREREG.md:51-60)

| | Criterion | Random | Energetic | Z = 3, Random | Z = 3, Energetic |
|---|---|---|---|---|---|
| H1 | Noise-free sets (ISM-fresh, Synth-fresh): wrong-silent ≤ 0.5 % of ok rows, on each set | fails | fails | fails | fails |
| H2 | SPPS-fresh: coverage ≥ 90 % of usable rows, and wrong-silent ≤ 3 % of ok rows | fails | fails | fails | fails |
| H3 | No subgroup with ≥ 20 ok rows has wrong-silent > 10 % | fails | fails | fails | passes |
| H4 | At a 1 ms step and T60 ≤ 3 s, usable ≥ 90 % of rows (SPPS-fresh and ISM-fresh) | passes | passes | passes | passes |
| H5 | Wrong-silent count strictly lower than upstream's on every fresh set | fails | fails | fails | fails |
| H6 | Attack: no reproducible class of physically plausible SPPS inputs (≥ 5 instances) gives wrong-silent | fails | fails | fails | fails |
| | **All six** | **fails** | **fails** | fails | fails |

Z = 3 is reported and decides nothing (PREREG.md:11, P32). Energetic decides only whether EDT is shown for energetic runs (P11); H1 and H6 hold no SPPS row and are the same for both modes.

## Each set (FINAL.md section 3)

Each cell reads frozen (Z = 2) · Z = 3 · upstream, except benefit/regress, which reads frozen · Z = 3. Wrong-silent: ok and more than 5 % off, of the ok rows with a truth. Truth outside the range: of the usable rows with a truth (P28's tolerance). Error: |edt / truth - 1| over usable rows with a truth. Half-width: (edt_hi - edt_lo) / (2 edt) over usable rows. Benefit/regress against upstream: rows with a truth on which only the method, or only upstream, is ok and within 5 %. Without a truth: rows left out of every count that needs one (P27).

| Set | rows | usable | ok | wrong-silent | truth outside range | median error | p95 error | median half-width | benefit/regress | without a truth |
|---|---|---|---|---|---|---|---|---|---|---|
| SPPS-fresh, Random | 3,456 | 95.8 % · 95.8 % · 100.0 % | 50.3 % · 25.9 % · 100.0 % | 132 of 1,621 · 33 of 873 · 1,371 of 2,855 | 541 of 2,710 · 197 of 2,710 · 2,855 of 2,855 | 2.07 % · 2.07 % · 4.59 % | 8.53 % · 8.53 % · 21.07 % | 4.81 % · 7.24 % · 0.00 % | 463/458 · 140/784 | truth_split_borderline 34, truth_uncertain 567 |
| SPPS-fresh, Energetic | 3,456 | 95.8 % · 95.8 % · 100.0 % | 91.6 % · 79.3 % · 100.0 % | 25 of 2,617 · 18 of 2,335 · 1,269 of 2,864 | 690 of 2,720 · 330 of 2,720 · 2,864 of 2,864 | 0.93 % · 0.93 % · 4.06 % | 3.54 % · 3.54 % · 20.03 % | 1.88 % · 2.83 % · 0.00 % | 1,074/77 · 892/170 | truth_split_borderline 25, truth_uncertain 567 |
| ISM-fresh | 3,888 | 76.4 % · 76.4 % · 100.0 % | 30.2 % · 17.8 % · 100.0 % | 84 of 1,066 · 42 of 631 · 2,641 of 3,729 | 276 of 2,836 · 149 of 2,836 · 3,729 of 3,729 | 1.72 % · 1.72 % · 10.00 % | 12.69 % · 12.69 % · 60.42 % | 6.20 % · 9.35 % · 0.00 % | 342/448 · 142/641 | truth_truncated 159 |
| Synth-fresh | 4,000 | 75.3 % · 75.3 % · 100.0 % | 53.5 % · 49.0 % · 100.0 % | 20 of 2,138 · 12 of 1,962 · 2,997 of 3,945 | 326 of 3,013 · 209 of 3,013 · 3,945 of 3,945 | 0.55 % · 0.55 % · 25.58 % | 16.84 % · 16.84 % · 261.10 % | 1.30 % · 1.95 % · 0.00 % | 1,329/159 · 1,184/182 | truth_nan 54 |
| Attack | 0 | – · – · – | – · – · – | 0 of 0 · 0 of 0 · 0 of 0 | 0 of 0 · 0 of 0 · 0 of 0 | – · – · – | – · – · – | – · – · – | 0/0 · 0/0 | none |

## H3: subgroups over 10 % wrong-silent (PREREG.md:57, P29)

- frozen, random: 152 subgroups, 59 judged, 13 over 10 %.
  - `spps|F2|far|1`: 5 of 27 ok rows with a truth wrong-silent (18.5 %)
  - `spps|F2|mid|2`: 4 of 32 ok rows with a truth wrong-silent (12.5 %)
  - `spps|F3|near|1`: 9 of 50 ok rows with a truth wrong-silent (18.0 %)
  - `spps|F4|mid|1`: 7 of 49 ok rows with a truth wrong-silent (14.3 %)
  - `spps|F5|near|1`: 5 of 39 ok rows with a truth wrong-silent (12.8 %)
  - `spps|F5|mid|1`: 9 of 42 ok rows with a truth wrong-silent (21.4 %)
  - `spps|F5|far|1`: 6 of 21 ok rows with a truth wrong-silent (28.6 %)
  - `spps|F6|near|1`: 19 of 101 ok rows with a truth wrong-silent (18.8 %)
  - `spps|F6|mid|1`: 3 of 20 ok rows with a truth wrong-silent (15.0 %)
  - `ism|rel:dead|mid|1`: 6 of 59 ok rows with a truth wrong-silent (10.2 %)
  - `ism|rel:deader|mid|1`: 21 of 25 ok rows with a truth wrong-silent (84.0 %)
  - `ism|drawn:2|mid|1`: 4 of 37 ok rows with a truth wrong-silent (10.8 %)
  - `ism|drawn:6|mid|1`: 11 of 29 ok rows with a truth wrong-silent (37.9 %)
- frozen, energetic: 152 subgroups, 78 judged, 4 over 10 %.
  - `ism|rel:dead|mid|1`: 6 of 59 ok rows with a truth wrong-silent (10.2 %)
  - `ism|rel:deader|mid|1`: 21 of 25 ok rows with a truth wrong-silent (84.0 %)
  - `ism|drawn:2|mid|1`: 4 of 37 ok rows with a truth wrong-silent (10.8 %)
  - `ism|drawn:6|mid|1`: 11 of 29 ok rows with a truth wrong-silent (37.9 %)
- Z = 3, random: 152 subgroups, 35 judged, 1 over 10 %.
  - `spps|F4|mid|1`: 5 of 39 ok rows with a truth wrong-silent (12.8 %)
- Z = 3, energetic: 152 subgroups, 64 judged, 0 over 10 %.

## H4: at 1 ms and design T60 ≤ 3 s (P30)

| Instance, mode | Set | rows | usable | ok | refused |
|---|---|---|---|---|---|
| frozen, random | SPPS-fresh | 1,032 | 100.0 % | 75.2 % | none |
| frozen, random | ISM-fresh | 1,296 | 96.1 % | 45.6 % | run_too_short 8, step_too_coarse 42 |
| frozen, energetic | SPPS-fresh | 1,032 | 100.0 % | 100.0 % | none |
| frozen, energetic | ISM-fresh | 1,296 | 96.1 % | 45.6 % | run_too_short 8, step_too_coarse 42 |
| Z = 3, random | SPPS-fresh | 1,032 | 100.0 % | 41.8 % | none |
| Z = 3, random | ISM-fresh | 1,296 | 96.1 % | 30.2 % | run_too_short 8, step_too_coarse 42 |
| Z = 3, energetic | SPPS-fresh | 1,032 | 100.0 % | 96.9 % | none |
| Z = 3, energetic | ISM-fresh | 1,296 | 96.1 % | 30.2 % | run_too_short 8, step_too_coarse 42 |

## H5: wrong-silent against upstream's on the same rows (P31)

| Instance, mode | Set | rows with a truth | method | upstream | |
|---|---|---|---|---|---|
| frozen, random | SPPS-fresh | 2,855 | 132 | 1,371 | passes |
| frozen, random | ISM-fresh | 3,729 | 84 | 2,641 | passes |
| frozen, random | Synth-fresh | 3,946 | 20 | 2,997 | passes |
| frozen, random | Attack | 0 | 0 | 0 | fails |
| frozen, energetic | SPPS-fresh | 2,864 | 25 | 1,269 | passes |
| frozen, energetic | ISM-fresh | 3,729 | 84 | 2,641 | passes |
| frozen, energetic | Synth-fresh | 3,946 | 20 | 2,997 | passes |
| frozen, energetic | Attack | 0 | 0 | 0 | fails |
| Z = 3, random | SPPS-fresh | 2,855 | 33 | 1,371 | passes |
| Z = 3, random | ISM-fresh | 3,729 | 42 | 2,641 | passes |
| Z = 3, random | Synth-fresh | 3,946 | 12 | 2,997 | passes |
| Z = 3, random | Attack | 0 | 0 | 0 | fails |
| Z = 3, energetic | SPPS-fresh | 2,864 | 18 | 1,269 | passes |
| Z = 3, energetic | ISM-fresh | 3,729 | 42 | 2,641 | passes |
| Z = 3, energetic | Synth-fresh | 3,946 | 12 | 2,997 | passes |
| Z = 3, energetic | Attack | 0 | 0 | 0 | fails |

## H6: the attack classes (P26)

No attack class was scored, so H6 fails (P27).

## Refusals, by reason and set (PREREG.md:62-63)

Refusals are never counted as wrong. Each cell reads frozen (Z = 2) · Z = 3 · upstream.

- SPPS-fresh, Random: step_too_coarse 145 · step_too_coarse 145 · none
- SPPS-fresh, Energetic: step_too_coarse 144 · step_too_coarse 144 · none
- ISM-fresh: direct_only 7, run_too_short 24, step_too_coarse 886 · direct_only 7, run_too_short 24, step_too_coarse 886 · none
- Synth-fresh: direct_only 62, not_decaying_at_run_end 13, run_too_short 566, step_too_coarse 346 · direct_only 62, not_decaying_at_run_end 13, run_too_short 566, step_too_coarse 346 · nan_or_nonpositive 1
- Attack: none · none · none

## The run

- Method: `B:\repos\I-Simpa_Night_Mode\.claude\worktrees\m8b-edt\docs\investigations\2026-09-27-edt-heldout\frozen\method.py`, sha256 `462c37cf159d4d9bd8abadd8261f975fa47ace66f0d4cb4b1e6f4234c77fdf6e`, checked before anything ran (PREREG.md:10), Z = 2.
- Z = 3 instance: the same file, sha256 `462c37cf159d4d9bd8abadd8261f975fa47ace66f0d4cb4b1e6f4234c77fdf6e`, with Z = 3 set after import (P32).
- Upstream: the port `B:\repos\I-Simpa_Night_Mode\.claude\worktrees\m8b-edt\docs\investigations\2026-09-27-edt-heldout\harness\m8b\_upstream_edt.py`, sha256 `57f391e6cb09ded8a408462fbabc7aacd93743a3fa46cc28deeef3f7a319f282` (`target/agents/upstream-edt/uphunt_upstream_edt.py`, P32).
- Inputs: 14,800 rows; SPPS-fresh, Random 3,456, SPPS-fresh, Energetic 3,456, ISM-fresh 3,888, Synth-fresh 4,000, Attack 0. Attack classes: 0, with votes for 0.
- The truth of each set (PREREG.md:34):
  - SPPS-fresh: P15's Definition A, the corpus's truth_ideal (target/agents/followup-design/skeptic-gf3/attack_ism.py:55-77, with rerun/mirror.py:20, 109-146), run from its text copies m8b/_truth_ideal.py and m8b/_mirror.py by truth.assess on the sum of the K = 4 references (P16-P19): bins overlapping [t_arr - R/c, t_arr + R/c) are direct, a blocked receiver's direct is 0.
  - ISM-fresh: P15's Definition A on the image-source split: truth_ideal at 0.02 ms with continuous air (P20, ism_fresh.make_row).
  - Synth-fresh: P15's Definition A in closed form: critique/synth.py truth_edt.
  - Attack: computed by the harness for each draw from its generator, as for Synth-fresh and ISM-fresh (P26).
- Generated 2026-10-01T23:13:48+02:00, Python 3.13.13, numpy 2.5.2.
