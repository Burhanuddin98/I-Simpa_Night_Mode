# M8b EDT held-out test, round 2: RESULTS (H1-H5)

Run `results`, scored by `harness2/m8b/score_heldout.py` at commit `9e80d0c`. Method: frozen2/method.py (v2.1) sha256 `029d90ac5e8f6a634a51a7ffce136cbd4c0b7ea3d3ad8a18b78fd8240ff224a0`, Z = 2.5, checked before anything ran. H6 (the attacker round) is not part of this run. No secondary Z is reported (PREREG-2.md). Wrong-silent is |edt/truth - 1| > 5% on an ok row with a truth, at every receiver radius; refusals are never wrong (PREREG.md:62).

**Features that miss.** Every G room is scored whatever its truth shows (PREREG-2 / HARNESS-PLAN-2.md section 9 M2). A designed feature that a truth run does not confirm (G2 truth T30 under 2.5 s at 1 kHz, G7 truth T30 over 0.25 s, G3 not double-sloped) is reported here and in VERDICT-2.md as missing; no room is swapped, redesigned or dropped. The truth features checked: G2 truth T30 at 1 kHz = 2.812 s, target >= 2.5 s: met; G3 truth T30/EDT at 1 kHz >= 1.25 at 6 of 8 receivers (R000 2.42, R001 2.82, R002 2.31, R003 3.50, R004 3.68, R005 3.08, R006 1.07, R007 1.03), target at least half of them: met; G7 truth T30 at 1 kHz = 0.204 s, target <= 0.25 s: met.

## SPPS-fresh-2, Random (frozen, Z = 2.5)

- H2 **pass**: coverage 2365 of 2442 usable rows with a truth = 96.8 % (target >= 90 %); wrong-silent 7 of 520 ok rows with a truth = 1.35 % (target <= 3 %).
- H3 **pass** (all sets in this criterion: 0 failing subgroups of 19 judged): SPPS-fresh-2 subgroups: 57, judged 8, failing 0.
- H4 **pass**: n = 1152 rows (1152 in the filter, 0 refused as receiver_too_large = 0.0 %, which leave the denominator); usable 1150 of 1152 = 99.8 % (target >= 90 %); ok share 16.1 % (reported, not gated); other refusals {'too_few_particles': 1, 'direct_only': 1}.
- H5 **pass**: wrong-silent, ours 7 against upstream 1471, on 2562 rows with a truth.

## SPPS-fresh-2, Energetic (frozen, Z = 2.5)

- H2 **pass**: coverage 2376 of 2439 usable rows with a truth = 97.4 % (target >= 90 %); wrong-silent 11 of 1487 ok rows with a truth = 0.74 % (target <= 3 %).
- H3 **FAIL** (all sets in this criterion: 2 failing subgroups of 41 judged): SPPS-fresh-2 subgroups: 57, judged 30, failing 2.
  - `spps|G4|far|1`: 6 of 26 ok rows wrong-silent (23.1 %)
  - `spps|G4|far|2`: 3 of 22 ok rows wrong-silent (13.6 %)
- H4 **pass**: n = 1152 rows (1152 in the filter, 0 refused as receiver_too_large = 0.0 %, which leave the denominator); usable 1152 of 1152 = 100.0 % (target >= 90 %); ok share 54.7 % (reported, not gated); other refusals none.
- H5 **pass**: wrong-silent, ours 11 against upstream 1401, on 2555 rows with a truth.

## ISM-fresh-2 (frozen, Z = 2.5)

- H1 **pass**: wrong-silent 0 of 115 ok rows with a truth = 0.00 % (target <= 0.5 %), every receiver radius.
- H3 **pass**: ISM-fresh-2 subgroups 93, judged 3, failing 0.
- H4 **pass**: n = 705 rows (720 in the filter, 15 refused as receiver_too_large = 2.1 %, which leave the denominator); usable 677 of 705 = 96.0 % (target >= 90 %); ok share 6.8 % (reported, not gated); other refusals {'run_too_short': 8, 'step_too_coarse': 20}.
- H5 **pass**: wrong-silent, ours 0 against upstream 2793, on 3723 rows with a truth.

## Synth-fresh-2 (frozen, Z = 2.5)

- H1 **pass**: wrong-silent 7 of 1954 ok rows with a truth = 0.36 % (target <= 0.5 %), every receiver radius.
- H3 **pass**: Synth-fresh-2 subgroups (family x step) 8, judged 8, failing 0.
- H5 **pass**: wrong-silent, ours 7 against upstream 3034, on 3925 rows with a truth.

## H4 n, by set (the rows left in the denominator; the scorer prints it for every set and mode)

| Mode | Set | n before | receiver_too_large | n | usable | ok |
|---|---|---|---|---|---|---|
| random | SPPS-fresh | 1152 | 0 (0.0 %) | 1152 | 99.8 % | 16.1 % |
| random | ISM-fresh | 720 | 15 (2.1 %) | 705 | 96.0 % | 6.8 % |
| energetic | SPPS-fresh | 1152 | 0 (0.0 %) | 1152 | 100.0 % | 54.7 % |
| energetic | ISM-fresh | 720 | 15 (2.1 %) | 705 | 96.0 % | 6.8 % |

## Rows, usable and ok shares, exclusions and refusals, by set and instance

Each cell: frozen (Z = 2.5) / upstream.

| Set | rows | usable | ok | wrong-silent of ok with truth | truth outside range | benefit / regress vs upstream (frozen) |
|---|---|---|---|---|---|---|
| SPPS-fresh, Random | 3456 | 95.5 % / 99.4 % | 15.1 % / 99.4 % | 7 of 520 / 1471 of 2562 | 77 of 2442 / 2562 of 2562 | 177 / 755 |
| SPPS-fresh, Energetic | 3456 | 96.1 % / 100.0 % | 48.3 % / 100.0 % | 11 of 1487 / 1401 of 2555 | 63 of 2439 / 2555 of 2555 | 679 / 357 |
| ISM-fresh | 3888 | 63.6 % / 100.0 % | 3.5 % / 100.0 % | 0 of 115 / 2793 of 3723 | 1 of 2356 / 3723 of 3723 | 17 / 832 |
| Synth-fresh | 4000 | 72.9 % / 100.0 % | 48.9 % / 100.0 % | 7 of 1954 / 3034 of 3925 | 192 of 2915 / 3925 of 3925 | 1234 / 178 |

### Truth exclusions, by reason and set (leave every truth-based count, P27)

| Set | truth_split_borderline | truth_truncated | truth_nan | truth_uncertain | total |
|---|---|---|---|---|---|
| SPPS-fresh, Random | 3 | 0 | 0 | 891 | 894 |
| SPPS-fresh, Energetic | 10 | 0 | 0 | 891 | 901 |
| ISM-fresh | 0 | 165 | 0 | 0 | 165 |
| Synth-fresh | 0 | 0 | 75 | 0 | 75 |

### Refusals, by reason and set (frozen / upstream)

- SPPS-fresh, Random: direct_only 1, step_too_coarse 135, too_few_particles 18 / nan_or_nonpositive 20
- SPPS-fresh, Energetic: step_too_coarse 135 / none
- ISM-fresh: direct_only 9, receiver_too_large 502, run_too_short 48, step_too_coarse 856 / none
- Synth-fresh: direct_only 81, not_decaying_at_run_end 8, receiver_too_large 72, run_too_short 562, step_too_coarse 362 / none

### receiver_too_large over all radii, by set and radius class (frozen)

| Set | R <= 0.5 m | 0.5-1.0 m | R > 1.0 m |
|---|---|---|---|
| SPPS-fresh, Random | 0 of 3456 (0.0 %) | 0 of 0 (n/a) | 0 of 0 (n/a) |
| SPPS-fresh, Energetic | 0 of 3456 (0.0 %) | 0 of 0 (n/a) | 0 of 0 (n/a) |
| ISM-fresh | 19 of 2160 (0.9 %) | 190 of 1026 (18.5 %) | 293 of 702 (41.7 %) |
| Synth-fresh | 1 of 2409 (0.0 %) | 18 of 991 (1.8 %) | 53 of 600 (8.8 %) |

## The draws

- ISM-fresh-2 rejections by reason (seed 2026100201): {'relative_near_duplicate': 21, 'relative_over_image_cap': 0, 'drawn_V_outside': 0, 'drawn_t60_125_outside': 0, 'drawn_near_duplicate': 2, 'drawn_over_image_cap': 0, 'receiver_R_no_room': 0, 'receiver_position_outside_class': 1073, 'receiver_R_position_cap': 0}
- Synth-fresh-2 rejections by reason (seed 2026100202): {'d_near_source': 44}
- Rows built: {'spps': 6912, 'ism': 3888, 'synth': 4000}.

## score.evaluate's own report

# M8b EDT held-out test, round 2: the scorer's report

**In plain words.** In Random mode, the mode the PREREG-2's verdict is about, the round-2 EDT method (v2.1, Z = 2.5) **fails**: it misses 2 of the six criteria (H5, H6). In Energetic mode, scored as its own set, which decides only whether EDT is shown for energetic runs (P11), it fails (H3, H5, H6). Over all 14,800 rows, it gave a confident value (ok) with a known truth on 4,076, and was more than 5 % off without warning (wrong-silent) on 25 of them (0.61 %). Upstream's EDT on the same rows was wrong-silent on 8,699 of its 12,765 (68.15 %). It refused 574 rows as receiver_too_large (the ball is too large for the room's decay): SPPS-fresh, Random 0, SPPS-fresh, Energetic 0, ISM-fresh 502, Synth-fresh 72, Attack 0. 2,035 rows have no usable truth and are left out of every count that needs one, though not of the ok and usable shares (P27): truth_nan 75, truth_split_borderline 13, truth_truncated 165, truth_uncertain 1,782. No rows were given for Attack, so every criterion that needs them fails: a set that answers nothing cannot pass (P27). The method file was checked before anything ran: sha256 029d90ac5e8f6a634a51a7ffce136cbd4c0b7ea3d3ad8a18b78fd8240ff224a0, as PREREG-2.md pins it.

**What each criterion found** (the frozen method, Random mode; Energetic where it differs):

- **H1 passes.** Noise-free sets (ISM-fresh-2, Synth-fresh-2): wrong-silent ≤ 0.5 % of ok rows, on each set, every receiver radius. ISM-fresh: 0 of 115 ok rows with a truth wrong-silent (0.00 %); Synth-fresh: 7 of 1,954 ok rows with a truth wrong-silent (0.36 %).
- **H2 passes.** SPPS-fresh-2: coverage ≥ 90 % of usable rows, and wrong-silent ≤ 3 % of ok rows. coverage 2,365 of 2,442 usable rows with a truth (96.8 %); wrong-silent 7 of 520 ok rows with a truth (1.35 %). Energetic: passes; coverage 2,376 of 2,439 usable rows with a truth (97.4 %); wrong-silent 11 of 1,487 ok rows with a truth (0.74 %).
- **H3 passes.** No subgroup with ≥ 20 ok rows has wrong-silent > 10 %, every receiver radius. 158 subgroups, 19 judged (20 or more ok rows with a truth), 0 over 10 %. Energetic: fails; 158 subgroups, 41 judged (20 or more ok rows with a truth), 2 over 10 %: spps|G4|far|1 6 of 26; spps|G4|far|2 3 of 22.
- **H4 passes.** At a 1 ms step, design T60 ≤ 3 s and receiver radius ≤ 0.5 m, usable ≥ 90 % of the rows not refused as receiver_too_large (SPPS-fresh and ISM-fresh). SPPS-fresh: n = 1,152 (1,152 in the filter, 0 refused as receiver_too_large), 1,150 of 1,152 rows usable (99.8 %), ok 16.1 %; ISM-fresh: n = 705 (720 in the filter, 15 refused as receiver_too_large), 677 of 705 rows usable (96.0 %), ok 6.8 %. Energetic: passes; SPPS-fresh: n = 1,152 (1,152 in the filter, 0 refused as receiver_too_large), 1,152 of 1,152 rows usable (100.0 %), ok 54.7 %; ISM-fresh: n = 705 (720 in the filter, 15 refused as receiver_too_large), 677 of 705 rows usable (96.0 %), ok 6.8 %.
- **H5 fails.** Wrong-silent count strictly lower than upstream's on every fresh set. SPPS-fresh: 7 against upstream's 1,471 on 2,562 rows with a truth; ISM-fresh: 0 against upstream's 2,793 on 3,723 rows with a truth; Synth-fresh: 7 against upstream's 3,034 on 3,925 rows with a truth; Attack: 0 against upstream's 0 on 0 rows with a truth. Energetic: fails; SPPS-fresh: 11 against upstream's 1,401 on 2,555 rows with a truth; ISM-fresh: 0 against upstream's 2,793 on 3,723 rows with a truth; Synth-fresh: 7 against upstream's 3,034 on 3,925 rows with a truth; Attack: 0 against upstream's 0 on 0 rows with a truth.
- **H6 fails.** Attack: no reproducible class of physically plausible SPPS inputs (≥ 5 instances) gives wrong-silent. 0 classes, 0 reproducible (5 or more of 20 draws wrong-silent), 0 of those not called implausible by 3 of 3 judges.

## Criteria (PREREG-2.md)

| | Criterion | Random | Energetic |
|---|---|---|---|
| H1 | Noise-free sets (ISM-fresh-2, Synth-fresh-2): wrong-silent ≤ 0.5 % of ok rows, on each set, every receiver radius | passes | passes |
| H2 | SPPS-fresh-2: coverage ≥ 90 % of usable rows, and wrong-silent ≤ 3 % of ok rows | passes | passes |
| H3 | No subgroup with ≥ 20 ok rows has wrong-silent > 10 %, every receiver radius | passes | fails |
| H4 | At a 1 ms step, design T60 ≤ 3 s and receiver radius ≤ 0.5 m, usable ≥ 90 % of the rows not refused as receiver_too_large (SPPS-fresh and ISM-fresh) | passes | passes |
| H5 | Wrong-silent count strictly lower than upstream's on every fresh set | fails | fails |
| H6 | Attack: no reproducible class of physically plausible SPPS inputs (≥ 5 instances) gives wrong-silent | fails | fails |
| | **All six** | **fails** | **fails** |

Energetic decides only whether EDT is shown for energetic runs (P11); H1 and H6 hold no SPPS row and are the same for both modes.

## Each set (FINAL.md section 3)

Each cell reads frozen (Z = 2.5) · upstream, except benefit/regress, which is the method's. Wrong-silent: ok and more than 5 % off, of the ok rows with a truth. Truth outside the range: of the usable rows with a truth (P28's tolerance). Error: |edt / truth - 1| over usable rows with a truth. Half-width: (edt_hi - edt_lo) / (2 edt) over usable rows. Benefit/regress against upstream: rows with a truth on which only the method, or only upstream, is ok and within 5 %. Without a truth: rows left out of every count that needs one (P27).

| Set | rows | usable | ok | wrong-silent | truth outside range | median error | p95 error | median half-width | benefit/regress | without a truth |
|---|---|---|---|---|---|---|---|---|---|---|
| SPPS-fresh, Random | 3,456 | 95.5 % · 99.4 % | 15.1 % · 99.4 % | 7 of 520 · 1,471 of 2,562 | 77 of 2,442 · 2,562 of 2,562 | 2.21 % · 6.00 % | 9.55 % · 24.74 % | 10.99 % · 0.00 % | 177/755 | truth_split_borderline 3, truth_uncertain 891 |
| SPPS-fresh, Energetic | 3,456 | 96.1 % · 100.0 % | 48.3 % · 100.0 % | 11 of 1,487 · 1,401 of 2,555 | 63 of 2,439 · 2,555 of 2,555 | 1.06 % · 5.65 % | 4.63 % · 24.05 % | 4.99 % · 0.00 % | 679/357 | truth_split_borderline 10, truth_uncertain 891 |
| ISM-fresh | 3,888 | 63.6 % · 100.0 % | 3.5 % · 100.0 % | 0 of 115 · 2,793 of 3,723 | 1 of 2,356 · 3,723 of 3,723 | 1.05 % · 15.38 % | 9.07 % · 93.68 % | 16.83 % · 0.00 % | 17/832 | truth_truncated 165 |
| Synth-fresh | 4,000 | 72.9 % · 100.0 % | 48.9 % · 100.0 % | 7 of 1,954 · 3,034 of 3,925 | 192 of 2,915 · 3,925 of 3,925 | 0.49 % · 25.60 % | 16.68 % · 239.96 % | 1.62 % · 0.00 % | 1,234/178 | truth_nan 75 |
| Attack | 0 | – · – | – · – | 0 of 0 · 0 of 0 | 0 of 0 · 0 of 0 | – · – | – · – | – · – | 0/0 | none |

## H3: subgroups over 10 % wrong-silent (PREREG-2.md, P29)

- frozen, random: 158 subgroups, 19 judged, 0 over 10 %.
- frozen, energetic: 158 subgroups, 41 judged, 2 over 10 %.
  - `spps|G4|far|1`: 6 of 26 ok rows with a truth wrong-silent (23.1 %)
  - `spps|G4|far|2`: 3 of 22 ok rows with a truth wrong-silent (13.6 %)

## H4: at 1 ms, design T60 ≤ 3 s and receiver radius ≤ 0.5 m (PREREG-2.md, P30')

n is the rows left in the denominator: the rows in the filter (n before) less those refused as receiver_too_large, which are counted beside it.

| Mode | Set | n before | receiver_too_large (count, share) | n | usable | ok | other refusals |
|---|---|---|---|---|---|---|---|
| random | SPPS-fresh | 1,152 | 0 (0.0 %) | 1,152 | 99.8 % | 16.1 % | direct_only 1, too_few_particles 1 |
| random | ISM-fresh | 720 | 15 (2.1 %) | 705 | 96.0 % | 6.8 % | run_too_short 8, step_too_coarse 20 |
| energetic | SPPS-fresh | 1,152 | 0 (0.0 %) | 1,152 | 100.0 % | 54.7 % | none |
| energetic | ISM-fresh | 720 | 15 (2.1 %) | 705 | 96.0 % | 6.8 % | run_too_short 8, step_too_coarse 20 |

## H5: wrong-silent against upstream's on the same rows (P31)

| Mode | Set | rows with a truth | method | upstream | |
|---|---|---|---|---|---|
| random | SPPS-fresh | 2,562 | 7 | 1,471 | passes |
| random | ISM-fresh | 3,723 | 0 | 2,793 | passes |
| random | Synth-fresh | 3,925 | 7 | 3,034 | passes |
| random | Attack | 0 | 0 | 0 | fails |
| energetic | SPPS-fresh | 2,555 | 11 | 1,401 | passes |
| energetic | ISM-fresh | 3,723 | 0 | 2,793 | passes |
| energetic | Synth-fresh | 3,925 | 7 | 3,034 | passes |
| energetic | Attack | 0 | 0 | 0 | fails |

## H6: the attack classes (P26)

No attack class was scored, so H6 fails (P27).

## Refusals, by reason and set (PREREG-2.md)

Refusals are never counted as wrong. Each cell reads frozen (Z = 2.5) · upstream.

- SPPS-fresh, Random: direct_only 1, step_too_coarse 135, too_few_particles 18 · nan_or_nonpositive 20
- SPPS-fresh, Energetic: step_too_coarse 135 · none
- ISM-fresh: direct_only 9, receiver_too_large 502, run_too_short 48, step_too_coarse 856 · none
- Synth-fresh: direct_only 81, not_decaying_at_run_end 8, receiver_too_large 72, run_too_short 562, step_too_coarse 362 · none
- Attack: none · none

### receiver_too_large over all radii, by set and radius class (frozen)

Count of rows refused as receiver_too_large, of the set's rows in the class (share). Energetic is its own set (P11).

| Set | R <= 0.5 m | 0.5-1.0 m | R > 1.0 m |
|---|---|---|---|
| SPPS-fresh, Random | 0 of 3,456 (0.0 %) | 0 of 0 (–) | 0 of 0 (–) |
| SPPS-fresh, Energetic | 0 of 3,456 (0.0 %) | 0 of 0 (–) | 0 of 0 (–) |
| ISM-fresh | 19 of 2,160 (0.9 %) | 190 of 1,026 (18.5 %) | 293 of 702 (41.7 %) |
| Synth-fresh | 1 of 2,409 (0.0 %) | 18 of 991 (1.8 %) | 53 of 600 (8.8 %) |
| Attack | 0 of 0 (–) | 0 of 0 (–) | 0 of 0 (–) |

## The run

- Method: `B:\repos\I-Simpa_Night_Mode\.claude\worktrees\m8b-edt\docs\investigations\2026-09-27-edt-heldout\frozen2\method.py`, sha256 `029d90ac5e8f6a634a51a7ffce136cbd4c0b7ea3d3ad8a18b78fd8240ff224a0`, checked before anything ran (PREREG-2.md), Z = 2.5.
- Upstream: the port `B:\repos\I-Simpa_Night_Mode\.claude\worktrees\m8b-edt\docs\investigations\2026-09-27-edt-heldout\harness2\m8b\_upstream_edt.py`, sha256 `57f391e6cb09ded8a408462fbabc7aacd93743a3fa46cc28deeef3f7a319f282` (`target/agents/upstream-edt/uphunt_upstream_edt.py`, P32).
- Inputs: 14,800 rows; SPPS-fresh, Random 3,456, SPPS-fresh, Energetic 3,456, ISM-fresh 3,888, Synth-fresh 4,000, Attack 0. Attack classes: 0, with votes for 0.
- The truth of each set (PREREG.md:34):
  - SPPS-fresh: P15's Definition A, the corpus's truth_ideal (target/agents/followup-design/skeptic-gf3/attack_ism.py:55-77, with rerun/mirror.py:20, 109-146), run from its text copies m8b/_truth_ideal.py and m8b/_mirror.py by truth.assess on the sum of the K = 4 references (P16-P19): bins overlapping [t_arr - R/c, t_arr + R/c) are direct, a blocked receiver's direct is 0.
  - ISM-fresh: P15's Definition A on the image-source split: truth_ideal at 0.02 ms with continuous air (P20, ism_fresh.make_row).
  - Synth-fresh: P15's Definition A in closed form: critique/synth.py truth_edt.
  - Attack: computed by the harness for each draw from its generator, as for Synth-fresh and ISM-fresh (P26).
- Generated 2026-10-02T05:20:14+02:00, Python 3.13.13, numpy 2.5.2.
