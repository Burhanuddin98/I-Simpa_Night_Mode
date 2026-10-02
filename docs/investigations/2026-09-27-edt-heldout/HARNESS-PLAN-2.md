# M8b EDT held-out test, round 2: plan for the harness (a delta against HARNESS-PLAN.md)

Drafted 2026-10-02 00:40 on branch `m8b-edt`, **before** any round-2 row, run or seed is used. It plans the code and the
parameters that run `PREREG-2.md` (with amendment 1, Z = 2.5). Not committed. Paths are relative to this folder (`D`).
Where this file is silent, `HARNESS-PLAN.md` as frozen by `ADDENDUM-A1.md` stands, P-number by P-number (section 1).

**In plain words.** Round 2 re-runs round 1's machine with: a new method file (v2.1) behind its own hash, seven new
SPPS rooms (one of them very dead), new seeds, a corpus list widened by everything round 1 made, and a scorer whose H4
asks only the question Burhan ruled on (normal receivers, 1 ms, T60 <= 3 s). Tests come first and fail first; a new freeze
file (`ADDENDUM-B1.md`) hashes everything before the first solver run. Round 1's code keeps its behaviour by default
(every change takes a `round_` argument that defaults to 1), so run1 can be reproduced and is never re-scored.

Preview disclosure: the G-room designs and the ISM-fresh-2 draw below were made in scratch scripts that only compute
geometry (`C:\Users\Burhan\AppData\Local\Temp\claude\...\scratchpad\plan2\`): no echogram, no histogram, no method call.

## 1. Every parameter, changed or reused

| P | Round 2 | What |
|---|---|---|
| P1 | **changed** | Freeze = first round-2 row, run, seed or room project of G1-G7. `ADDENDUM-B1.md` replaces A1 as the gate (`driver.ADDENDUM`, :114); every held-out guard asks `b1_committed()` for round-2 items. Round-1 items still ask A1 (control) |
| P2 | **changed** (P2', section 2) | Corpus list widened to `corpus_rooms_2.json` |
| P3 | reused | Rule unchanged. Now applied to non-box rooms against F3, F4 (round 1 had no non-box corpus room) |
| P4 | reused | `frozen2/method.py` runs before B1 only on corpus inputs and planted fixtures; round-2 draws go through a stub. Run1 data shaped v2.1 (PREREG-2 says so) and is DEV |
| P5 | reused | Eyring with air. G3 uses P5's coupled rule (chamber, doorway at alpha 1 = late slope; main room = early) |
| P6 | **extended** | Reported, not gated, after the truth runs: G2 truth T30 at 1 kHz >= 2.5 s; G3 T30/EDT at 1 kHz >= 1.25 at half its receivers; **G7 truth T30 at 1 kHz <= 0.25 s** |
| P7 | reused | Same OBJ -> `simpa import` path (`rooms._Grid`, `obj_text`, `project_from_import` reused by `rooms2.py`) |
| P8, P9 | reused | Product defaults, seeds non-zero, intersection files off. R = 0.31 m |
| P10 | reused | 10 s tested runs; T11-style check that a fresh import still defaults to 10 s |
| P11 | reused | Energetic as its own set; 144 tested runs |
| P12 | **changed** | New SPPS seeds (section 3) |
| P13 | reused (shape) | 7 rooms x 3 steps x 3 seeds at 150k, plus G6 x 3 x 3 at 50k: 72 per mode, 144 tested; truth 7 x K = 4 = 28; 172 runs, 6,912 rows |
| P14 | reused | Same verified solvers; B1 records `solvers/manifest.json` hash, spps.exe code sha256 `550485c6...` and `simpa.exe` sha256 as found at the freeze |
| P15-P16, P18-P19 | reused | Truth function, K = 4 at 1M particles and 0.1 ms, -60 dB test, u <= 1 % |
| P17 | reused (recomputed) | Table in section 2 |
| P20, P22 | reused | ISM generator, receivers and runs (12 per room, R log-U[0.1, 1.5], bands to 20 kHz, 1/2/5 ms, 2.0 s) |
| P21 | **changed** (seed, section 3) | Same recipe, new seed, relatives redrawn against the widened list |
| P23, P24 | reused ranges, **new seed** | Double slopes 1.5 and 5, T60 0.1-10 s, DRR -20..+10 dB, R log-U[0.1, 1.5], same bounds |
| P25 | **changed** | Sandbox holds `frozen2/method.py` (section 5); folder `C:\tmp\m8b-edt\attack2\` |
| P26 | reused | Classes, 20 draws, 3 judges, implausible only 3 of 3, reproducible at >= 5 |
| P27-P29, P31 | reused | Exclusions, covered/wrong-silent, H2/H3 (every radius), H5 strict |
| P30 | **changed** -> P30' | H4 as PREREG-2 (section 4) |
| P32 | **changed** | No Z = 3 instance, no secondary Z. Upstream port unchanged (`57f391e6...`) |
| P33 | reused | `weak_spots.json` (sha `c75d5321...`) not regenerated, so the ranges of P21-P24 are exactly round 1's, as PREREG-2 says. T22 stays as control |
| 8.2 calls | reused | Synth edge, d < R exclusion, split-borderline, truncated-retry (`retry_truncated=True`), gap in every room, PHYSICS.md diffuse check |
| **P34** new | | Logging: a line when each ISM room **starts** and after each of its receivers (section 7) |
| **P35** new | | Parallelism: SPPS queue <= 4 workers (Burhan, after a machine reset under 8), enforced; ISM scoring 2 workers (7.6 GB per large echogram) |
| **P36** new | | Paths: room projects `C:\tmp\m8b-edt\heldout-rooms-2` (`rooms._not_on_b` refuses B:), run outputs `B:\data\m8b-edt\heldout2`, results `B:\data\m8b-edt\results\run2`, scratch/log on C:. Round-1 folders are refused as round-2 roots |

## 2. P2': the corpus to keep clear of, and SPPS-fresh-2

`corpus_rooms_2.json` (new, built by `m8b/corpus2.py`, deterministic, never by hand). `corpus_rooms.json` stays byte-identical
(sha256 `dc3d0b2c0b8be0f69c7927f0c749a88dad76cc791edae2474f09cb1fd44da882`, asserted before the build).

| Added to the 119 rooms | n | Source and check |
|---|---|---|
| F1-F7 (`run1_spps`) | 7 | `rooms.rooms()`; F3, F4 are the corpus's first non-box rooms |
| P0, P0b (`run1_probe`) | 2 | Never scored; included because their histograms exist. Can only remove candidates |
| Run1's 12 ISM-fresh rooms (`run1_ism`) | 12 | `ism_fresh.draw(2026100102)` (dims, alphas, source, 12 receivers each, T60). Cross-checked against `B:\data\m8b-edt\results\run1\counts.json` `ism_rooms` (equal dims, alphas, T60) |
| G1-G7 (`round2_spps`) | 7 | Added last, so ISM-fresh-2 and later draws keep clear of them. A G room's own clearance is computed over entries of other sets |

Total 147 (kinds: 143 box, 4 non-box). Synth-fresh round 1 (seed 2026100101, 4,000 specs) and run1's SPPS/ISM/truth seeds go in as
**lists** inside the file (`seeds_used`, `synth_specs_sha256`), not rooms: T37 and T34 check disjointness. The hash of the file,
of the 12-room draw and of the 4,000-spec list go in B1.

SPPS-fresh-2 (designed with `rooms.p3_gap`, `rooms.eyring`, `rooms._build`, `driver.truth_duration_s`; Lambert 1 except G5):

| Room | Geometry (m) | alpha | Design T60 125 / 500 / 1 k / 4 k Hz (s) | V m3 | P3 clearance (nearest) | Role |
|---|---|---|---|---|---|---|
| G1 | box 4.2 x 2.5 x 2.2 | 0.20 | 0.33 / 0.33 / 0.33 / 0.31 | 23.1 | 0.185 (P0b); 0.235 (F1) | small (V <= 30) |
| G2 | box 28 x 12 x 7 | 0.09 | 3.23 / 3.10 / **3.00** / 2.10 | 2,352 | 0.246 (z3 box003) | T60 >= 2.5 at 1 kHz |
| G3 | main 7.5 x 6 x 3.5 + 0.2 m partition with doorway y 2.4-3.9, z 0-2.2 + chamber 10 x 6 x 3.5 | main + its partition face 0.35; chamber, reveals, its partition face 0.06 | late 1.90 / 1.86 / 1.82 / 1.44; early 0.31 / 0.30 (125 / 1 k) | 368.2 (S 411.4) | 0.164 (F3, non-box) | non-uniform absorption, double slope, coupled; blocked receiver |
| G4 | L: arm 14 x 4 x 3.0 + arm x 10-14, y 4-12 | 0.18 | 0.64 / 0.64 / 0.63 / 0.58 | 264.0 (S 332.0) | 0.420 (F4, non-box) | L-shaped; blocked receiver |
| G5 | box 18.5 x 10.5 x 3.0 | 0.20, scattering **0.1** | 0.75 / 0.74 / 0.73 / 0.66 | 582.8 | 0.312 (m8a 20x8x4) | specular low hall |
| G6 | box 32 x 6.5 x 2.5 | 0.35 | 0.32 / 0.32 / 0.32 / 0.30 | 520.0 | 0.333 (F6) | long, absorbent; the 50k room |
| G7 | box 9.0 x 7.5 x 2.5 | 0.50 | 0.18 / 0.18 / **0.179** / 0.175 | 168.8 | 0.250 (ISM t1) | **dead room**: 1 kHz T60 0.179 <= 0.25 |

Minimum P3 clearance over all G rooms and the whole widened list: **0.164** (G3 against F3); no pair of G rooms is under 0.15 apart.
Round 1's smallest was 0.171. Fresh needs > 0.10.

Source, then receivers (d in m, from `rooms._build`; every point >= 0.6 m from every surface, `check`ed; B = blocked):
```
G1 src (0.7,1.25,1.1)  (1.5,1.5,1.0).84 (1.9,0.9,1.4)1.29 (1.4,1.9,0.8)1.00 (2.4,1.2,1.2)1.70 | mid (3.0,1.6,1.4)2.35 (3.4,1.0,1.0)2.71 (3.6,1.9,1.6)3.01 (2.9,1.4,0.8)2.23
G2 src (3.5,4.0,1.6)   (4.7,4.7,1.3)1.42 (3.9,5.6,1.7)1.65 (2.6,3.0,1.2)1.40 | mid (8.0,6.0,1.4)4.93 (13.0,3.0,2.2)9.57 (9.0,10.0,1.5)8.14 | far (20.0,8.0,1.4)16.98 (26.5,10.5,3.0)23.94
G3 src (2.0,3.2,1.5)   (3.0,3.8,1.2)1.20 (1.2,4.4,1.7)1.46 (3.1,2.0,1.4)1.63 | mid (5.0,4.5,1.3)3.28 (6.4,1.5,1.8)4.73 (4.2,5.2,2.2)3.05 | far (15.0,3.4,1.2)13.00 through the doorway; (13.0,0.9,1.3)11.24 B
G4 src (1.8,2.0,1.4)   (2.9,2.7,1.2)1.32 (1.0,3.0,1.5)1.28 (2.6,1.0,1.6)1.30 | mid (5.5,2.5,1.4)3.73 (8.0,1.5,1.7)6.23 (10.5,3.0,1.2)8.76 | far (13.0,1.5,1.3)11.21; (12.5,10.5,1.2)13.67 B
G5 src (3.0,5.25,1.5)  (4.2,5.7,1.2)1.32 (2.2,4.2,1.7)1.34 (3.6,6.8,1.8)1.69 | mid (7.5,4.0,1.3)4.67 (9.0,7.5,1.8)6.42 (6.5,9.0,2.0)5.15 | far (14.0,8.5,1.3)11.47 (15.0,2.0,1.6)12.43
G6 src (2.0,3.25,1.5)  (3.2,3.8,1.2)1.35 (2.7,2.2,1.6)1.27 (1.0,4.6,1.3)1.69 | mid (6.0,2.5,1.4)4.07 (9.5,4.5,1.8)7.61 (11.0,1.5,1.2)9.17 | far (16.0,3.0,1.3)14.00 (29.5,5.0,1.6)27.56
G7 src (2.2,2.4,1.3)   (3.2,3.0,1.2)1.17 (2.6,1.2,1.5)1.28 (1.1,3.6,1.4)1.63 | mid (5.0,3.5,1.4)3.01 (7.0,2.0,1.7)4.83 (6.5,6.2,1.2)5.74 (8.0,6.8,1.6)7.29 (4.5,5.5,1.0)3.87
```
Classes: near < 2, far > 10 (G1 and G7 have none, as F1 and F7). Blocked (segment leaves the room): G3 R007, G4 R007, as built.
Reference run length by P17 (steps at 0.1 ms; solver limit 65,536, `validate.rs:287`): G1 3.0 s (30,000), **G2 6.5 s (65,000)**,
G3 3.9 s (39,000), G4 3.0 s (30,000), G5 3.0 s (30,000), G6 3.0 s (30,000), G7 3.0 s (30,000). Tested runs: 10 s / 1 ms = 10,000 steps.
G2's 6.5 s is 2.0 x its 125 Hz T60 (3.23 s): the P18 last-10 % test is the guard. `rooms2.py` adds a check that every run's steps <= 65,536.
`rooms2.py` (new) holds `NAMES = ('G1'..'G7')`, `_PLAN`, a generic `coupled` flag in place of `rooms._build`'s name test for 'F3', and
`rooms()`, `check_rooms()`, `write_projects()`, `mesh_projects()` by reuse; `rooms.py` itself is not edited.

## 3. Seeds and draws

| Item | Round 1 (P12 etc.) | Round 2 |
|---|---|---|
| SPPS 150k | 1101-1103 (1 ms), 1201-1203 (2 ms), 1501-1503 (5 ms) | **3101-3103, 3201-3203, 3501-3503** |
| SPPS 50k (G6) | 2101-2103, 2201-2203, 2501-2503 | **4101-4103, 4201-4203, 4501-4503** |
| Truth (K = 4) | 9001-9004 | **9101-9104** |
| Reserved probes | 9998, 9999 | unchanged, still refused |
| ISM-fresh | `SeedSequence(2026100102)` | **`SeedSequence(2026100201)`** |
| Synth-fresh | `SeedSequence(2026100101)` | **`SeedSequence(2026100202)`** |
| Dev seeds | PROJECT_SEED 1, DRY_SEED 20261001 | unchanged; grep of `harness/**/*.py` finds none of the new seeds in use |

The new seeds join `driver.HELDOUT_SEEDS` (guarded by B1); T34 checks disjointness from every round-1 seed, probe and dev seed.
**ISM relatives:** the four parents are still the corpus's ISM rooms (set `ism`: t1, corridor, dead, deader). `ism_fresh._relative`
rejects a draw when `rooms.p3_clearance` against `corpus_rooms_2.json` says not fresh; run1's four relatives and eight drawn rooms are
entries of that list (`run1_ism`), so every round-2 relative is a P3 non-duplicate of round 1's relative of the same parent, by
construction, and T36 asserts it pairwise as well. Rejections stay counted by reason (the draw already returns them).
`ism_fresh.draw(seed, *, a1=None)` gains `corpus_path=None, round_=1` (defaults = round 1) so it can read `corpus_rooms_2.json`.
Preview with seed 2026100201 against the list **with G1-G7**: 21 `relative_near_duplicate`, 2 `drawn_near_duplicate`, 1,073
`receiver_position_outside_class`, 0 over the image cap; 12 rooms, 2.44e8 images (run1: 3.26e8); relatives clear their neighbours by
0.103-0.117; 80 of the 144 receivers have R <= 0.5 m. This is a preview: the draw that counts is the one made at the freeze.
Synth-fresh-2 is `synth_fresh.draw(2026100202)` with unchanged `BOUNDS`; T37 asserts no spec's parameter vector equals one of round 1's.

## 4. Scorer changes (`score.py`, `score_heldout.py`, `spps_rows.py`, `method.py`, `corpus.py`)

New `m8b/round2.py` is the one place for round 2's constants (method path and pin, seeds, rooms module, roots, B1 name, H4 rule);
`CONFIG[1]` reproduces round 1, `CONFIG[2]` is this plan. Each function below takes `round_=1`.

| Where | Change |
|---|---|
| `corpus.py` | add `FROZEN2 = D/frozen2/method.py` and `FROZEN2_SHA256` (set at the freeze; today `029d90ac5e8f6a634a51a7ffce136cbd4c0b7ea3d3ad8a18b78fd8240ff224a0`, LF bytes). `FROZEN`/`FROZEN_SHA256` untouched |
| `D/.gitattributes` | add `frozen2/** text eol=lf`. **Needed:** git reports `frozen2/method.py` as `i/lf w/lf attr/` with `core.autocrlf=true` on Grace, so a fresh checkout would give CRLF and a different hash (round 1 hit this and added `frozen/** eol=crlf`, `00e793e`). T23 checks it |
| `method.py` `load(path, z)`, `load_z3` | `load(path=None, z=None, *, pin=None)`: default pin = round 1's. Round 2 passes `FROZEN2_SHA256`; `Z` is read from the file (2.5), never set after import; `load_z3` is not called in round 2 |
| `spps_rows.rows_from_runs` (`method_path=None`, `m = method.load(...)`) | takes `round_`/`geometry` (G-room geometry from `rooms2`); its split-borderline test calls the **frozen2** `analyse`; adds `R_m = half_width * C_SPPS` (rounded 1e-9) to each row |
| `ism_fresh.rows_for_scoring`, `score_heldout.build_synth` | rows carry `R_m` (`spec['R_m']`); `rows_for_scoring(D, on_receiver=None)` (P34) |
| `score.ROW_FIELDS` (:150) | + `'R_m'`; `_check_inputs` (:493, `_in_h4` call :516) raises if an SPPS/ISM row lacks `R_m` |
| `score.py` constants (:143-145) | + `H4_R_MAX_M = 0.5`, `H4_RTL = 'receiver_too_large'` |
| `score._in_h4` (:391) | row in H4 iff step 1 ms (`_step`) **and** design T60 <= 3.0 **and** `R_m <= 0.5 + 1e-9` |
| `score.h4` (:402) | from the filtered rows remove those with `status == 'refused'` and `reason == 'receiver_too_large'`; usable share over the rest, `>= 90 %`; per set also return `n_before`, `n_receiver_too_large`, `share_receiver_too_large`, `ok_share` (reported), `refused` by reason. A 0 denominator fails (P27) |
| `score.h1` (:312), `h3` (:369), `h2`, `h5` | **unchanged**: no radius filter anywhere (T29 plants a wrong-silent row at R = 1.4 m) |
| `score.tally` (:248) / `report_md` (:744) | `refused` by reason already holds `receiver_too_large` over **all** radii; add a table of it by set and by R class (<= 0.5, 0.5-1.0, > 1.0 m): count, share of the set's rows. Energetic stays its own label (`LABELS`, P11) |
| `score.INSTANCES` (:152), `evaluate` (:579-640), `report_md` (:705-718, 746-766, 787-791, 805-812, 825-841), `H_TEXT['H4']` (:160) | instances `('frozen', 'upstream')` when `round_ == 2`: no `z3` load (:581), no `z3` in `runs` (:586), loops (:600, :603), `summary['z3']` (:614), report columns and sentences; `decided_by` reads Z = 2.5; H4 text is PREREG-2's. Upstream is scored on the same rows and paired by id (H5) |
| `score_heldout.py` | `build_spps` (:76: `driver.plan2()`, round-2 `TRUTH_SEEDS`), `build_synth` (:184: `R_m`), `CSV_FIELDS` (:200: `R_m`), `counts_of` (:215: no `z3`), `results_md` (:236: "Z = 2" strings :241, 246, 269, 287, 302, the Z = 3 block :320-:340 removed, H4 block with the receiver_too_large line), `main` (:350: `--round`, ISM/Synth seeds :379-:383, log text :386, :402). `--round 2` refuses round-1 roots, `results\run1` and any F-room |
| `rooms2.py`, `driver.py` | `driver.plan2()`, round-2 `TESTED_SEEDS`/`TRUTH_SEEDS`/seeds, `ROOM_ALLOWLIST` (:132) + G names, `b1_committed()`, `launch()` guards (:376+) ask B1 for G rooms and round-2 seeds. `run_heldout.py`: `--round`, default `--workers 4` (:312, was 8) and values > 4 refused, `DEFAULT_DATA_ROOT` (:55) per round |

## 5. Attack (H6) against frozen2

| Where | Change |
|---|---|
| `attack.py` :49, `build_sandbox` (:398) | `build_sandbox(dest, round_=1)`: for 2 it checks and copies `frozen2/method.py` by `FROZEN2_SHA256` (VoidRun on mismatch) into `C:\tmp\m8b-edt\attack2\`; still exactly `SANDBOX_FILES` |
| `interface_md` (:197) | adds the statuses and the **refusal list**, taken from the file: no_energy, no_energy_after_arrival, run_too_short, direct_only, step_too_coarse, not_decaying, too_few_particles, not_decaying_at_run_end, **receiver_too_large** (h = R/c above 0.01 of the fitted EDT). Docs say a refusal is never wrong-silent. Round-1 text unchanged for `round_=1` |
| `physics_md`, `judge_prompt`, `validate_class`, `draws`, `panel`, `generators_py` | unchanged (hash-pinned in B1); the judge prompt holds class files and PHYSICS.md only |
| Gate | `build_sandbox(round_=2)` refuses (`attack_before_sentinel`) until `B:\data\m8b-edt\results\run2\SENTINEL.md` exists (run order, section 7) |

## 6. Tests, written first (T-numbers continue from round 1's T22; same style; RED-2.md by an independent subagent)

Each new module starts as a stub (returns None or raises NotImplementedError), so each test fails on its assertion. A control passes before and after.
Round 1's suite (T1-T22 and later files) stays as a regression control with `round_=1` defaults; its one known failure,
`test_driver.py::test_t13` (frozen-file, logged 18:21), is explained or fixed before B1.

| Id | Asserts | Before |
|---|---|---|
| T23 | `frozen2/method.py` hashes to `FROZEN2_SHA256`; `Z == 2.5` as imported; `.gitattributes` gives `frozen2/**` eol=lf and `git ls-files --eol` shows it | fails |
| T24 | `method.load` (pin 2): one flipped byte -> VoidRun; round-1 pin rejects frozen2 and round-2 pin rejects `frozen/`; round-1 loader unchanged | fails (control for round 1) |
| T25 | `evaluate(round_=2)`: instances are frozen + upstream, no `z3` key, `load_z3` never called (monkeypatched to raise), "Z = 3" absent from REPORT.md and RESULTS.md, Z printed 2.5 | fails |
| T26 | H4 filter: R_m exactly 0.5 in, 0.5000001 out; 2 ms out; design T60 3.0 in, 3.01 out; missing `R_m` raises | fails |
| T27 | H4 denominator: planted `receiver_too_large` rows in the filter leave it, are counted (count, share, set); other refusals stay in; empty denominator fails; ok share reported, not gated | fails |
| T28 | Refusals by reason over all radii incl. `receiver_too_large`, per set and per R class, Energetic separate | fails |
| T29 | H1 and H3 count a wrong-silent row at R = 1.4 m (no radius filter); H2 and H5 likewise | control |
| T30 | Upstream is scored on the same ids as frozen in every set (H5 pairing), upstream row has `R_m`; Energetic never mixes into Random's H2-H5 (P11) | fails (id/R_m part) |
| T31 | Rows carry `R_m`: spps 0.31, ism = spec, synth = spec; every set passes `_check_inputs` | fails |
| T32 | `corpus_rooms_2.json`: 147 entries as section 2, kinds match their sources, `corpus_rooms.json` sha unchanged, rebuild is byte-identical, 12 run1 ISM rooms equal `counts.json` | fails |
| T33 | G rooms: V, T60 per band (P5), classes, blocked flags, 0.6 m clearance, 8 receivers each; features (G7 T60(1 k) <= 0.25; G2 T60(1 k) >= 2.5; G1 V <= 30; G3 early/late; G4 L; blocked in G3, G4); P3 clearance >= 0.15 against the list minus its own set (min 0.164); G pairs apart; P17 lengths and steps <= 65,536 | fails |
| T34 | `plan2()`: 144 tested + 28 truth, seeds as section 3, no repeat, none in round 1's `HELDOUT_SEEDS`, probes (9998, 9999) or dev seeds; ids unique | fails |
| T35 | Guards: a round-2 seed or G room is refused before B1 and allowed after; F rooms and round-1 seeds still ask A1; `--workers 5` refused; round-1 data root refused; project folder on B: refused; free-space floor | fails |
| T36 | ISM-fresh-2 (dev run and the real seed): four relatives, one per ISM parent, each P3-fresh against the corpus list **and** against round 1's relative of that parent; eight drawn fresh incl. G rooms; <= 1e8 images; rejections counted and reproducible; the round-1 seed is refused | fails |
| T37 | Synth-fresh-2: 4,000 specs, ranges and `BOUNDS` equal round 1's, no spec equals one of round 1's, reproducible from the seed | fails |
| T38 | Planted dead-room histogram (EDT 0.05 s, R 0.31 m) is `receiver_too_large` under frozen2 and `ok`-wrong under frozen (control: the round-1 method still says what it said) | fails |
| T39 | Dry run on planted inputs (D2 builder + one `receiver_too_large` row + G-room mock): every count and H verdict equals `expected_dry_2.json`, committed first | fails |
| T40 | Attack round 2: sandbox = exactly `SANDBOX_FILES`, `method.py` byte-equal to frozen2, INTERFACE.md names every reason found by regex in `frozen2/method.py` (set equality), VoidRun on a changed file, refuses before SENTINEL.md | fails |
| T41 | Logging (P34): with fake ISM workers, a `start` line for every room precedes the first `done` line and each carries its image count; one line per receiver | fails |
| T42 | `m8b.freeze2`: writes `ADDENDUM-B1.md`'s hash table; `--check` recomputes every sha256 and fails on a planted one-byte change; the list below is complete (every `m8b/*.py`) | fails |
| T43 | Run order: scorer refuses without B1 committed or with a missing/partial run folder; attack refuses without SENTINEL.md; `--round 2` never scores run1 | fails |

Dry run D1'-D4': D1 (1,980 rows) and D4 controls re-run as round 1 did; D2' adds the planted dead-room row. Harness bar: T23-T43 pass, round-1 suite passes,
review by a default subagent finds no blocker, `C:` keeps >= 8 GB, nothing exists under `B:\data\m8b-edt\heldout2` or `heldout-rooms-2`, `launch.log` has no round-2 line.

## 7. Freeze (`ADDENDUM-B1.md`), run order, logging

B1 content (written by `freeze2`, committed before the first run; "no round-2 row exists" checked and recorded): sha256 of
`PREREG.md`, `PREREG-2.md` (with amendment 1), `HARNESS-PLAN.md`, `HARNESS-PLAN-2.md`, `ADDENDUM-A1.md`, `expected_dry_2.json`, `.gitattributes`;
`frozen/method.py` (`462c37cf...`) and `frozen2/method.py`; `corpus_rooms.json`, `corpus_rooms_2.json`, `weak_spots.json`; every `harness/m8b/*.py`
(round 1's 22 plus `round2.py`, `rooms2.py`, `corpus2.py`, `freeze2.py`); the 12-room ISM-2 draw and 4,000-spec Synth-2 list hashes; solver manifest, spps.exe code
sha256, `simpa.exe` sha256, Python 3.13.13 / numpy 2.5.2, git commit and tree. It restates sections 1-5 verbatim, as A1 restated P1-P33.

Run order: **freeze (B1) -> truth refs (28 runs, 9101-9104) -> tested runs (144, Random then Energetic) -> score -> sentinel -> attack -> verdict.**
`run_heldout.py --round 2 --workers 4` (truth first, resumable, STOP file). Score = SPPS rows, ISM-fresh-2, Synth-fresh-2, then `evaluate`. The sentinel seat
recounts every H independently and writes `results\run2\SENTINEL.md` (what unlocks the attack). `VERDICT-2.md` follows. If round 2 fails, PREREG-2's rule applies.

Round 1's logging gap: `score_heldout.build_ism` wrote a line only when a room **finished** (first line at 20:05 for a job started 19:13). P34: `_ism_room` (:152) logs
`ism room <id> start: <n images>, pid` through `Log` before work, and `rows_for_scoring(D, on_receiver=cb)` logs `receiver k/12` (every 27 rows) with elapsed time; the done line stays.

## 8. Cost forecast (labelled forecast; no round-2 run has happened)

| Part | Forecast | From |
|---|---|---|
| SPPS: 172 runs at 4 workers | **~22 min (18-30)**; slowest single run G2's truth, ~350-500 s | Round 1: 172 runs, 18:39-19:01 (1,296 s, `progress.log`); per-room truth 25-347 s (F7..F2), G rooms analogous by V and T60, G2 larger than F2 (2,352 vs 1,700 m3) |
| Reading SPPS rows, 6,912 | ~4 min | run1 19:09-19:13 |
| ISM-fresh-2 scoring, 12 rooms, 2 workers | **~3-4 h** | Run1: 19:13-23:13, 27,005 room-seconds / 2 workers, per room 486-4,806 s, 3.26e8 images; preview draw 2.44e8 images (about 0.75 x); retries make time super-linear in the largest rooms, so the upper end is run1's 4 h |
| Synth-fresh-2 and `evaluate` | minutes | run1 23:13 -> 23:13 (<1 min); two instances instead of three |
| Disk | ~7 GB, ~11k files on B: | run1 `heldout`: 7.08 GB, 10,988 files; B: free 51 GB, C: 25.6 GB |
Sequential total forecast ~4-4.5 h to RESULTS, then the sentinel and the attack round. Not forecast: the attacker round's wall time.

## 9. Open questions for Burhan

1. **G8, a deeper dead room?** G7 (T60 0.18 s) sits above the refusal edge for R = 0.31 m (EDT about 0.09 s), so `receiver_too_large` is exercised on SPPS only if something is deader. PREREG-2 asks for one dead room. Recommended default: **no G8**; the refusal is exercised over R up to 1.5 m by ISM-fresh-2 and Synth-fresh-2. (A G8 at T60 about 0.08 s costs ~3 min of runs and lengthens the matrix to 8 rooms.)
2. **Go-ahead.** Freeze (B1), runs and scoring follow without a stop, as 10:59 on 10-01 ruled for round 1 ("keep building ... finish the work"). Recommended default: **yes, no stop** until the verdict, with progress lines to `progress.log`.

## 9. Crucible resolutions (2026-10-02 00:45, before any build; these override the sections above)

Crucible on this plan: SOUND-WITH-FIXES, 0 blockers, 5 majors, 5 minors. Resolved as follows (technical calls).

| # | Finding | Resolution |
|---|---|---|
| M1 | G2 designed at exactly 3.00 s at 1 kHz, H4's edge (`score.py:399`) | G2 is redesigned (absorption only, geometry kept if P3 still clears) to a design T60 at 1 kHz in **[2.6, 2.9] s**: >= 2.5 as PREREG-2 needs, clearly inside H4. Recompute its per-band T60, P17 run length and P3 clearance; T33 asserts the band. |
| M2 | Features confirmed on truth runs are "reported, not gated", with no rule for a miss | Written now: every G room is scored whatever its truth shows. A feature that misses on the truth runs (G2 T30 < 2.5 s, G7 T30 > 0.25 s, G3 not double-sloped) is reported as missing in RESULTS.md and VERDICT.md. No room is swapped, redesigned or dropped after any truth run exists. |
| M3 | Seeds and G-room designs previewed but not pinned until B1 | Pinned at the commit of this section: the seeds of section 3 and the G-room table of section 2 (with M1's G2 change as its only permitted edit) are final. `corpus2.py` writes `preview_pin.json` (sha256 of the canonical JSON of the ISM-fresh-2 draw, the Synth-fresh-2 specs and the G-room geometry + receivers) during the build; B1 records it and the runner refuses if a fresh draw differs. |
| M4 | B1's hand-written hash list is wrong (19 not 22 modules; tests, fixtures, dry inputs unhashed) | B1 hashes **every file under `harness2/` and `frozen2/`** (glob, sorted, LF-normalised bytes as committed), plus PREREG-2.md, HARNESS-PLAN-2.md and `preview_pin.json`. No hand list. |
| M5 | `round_` arguments and CONFIG[1]/[2] dual paths are heavy and can leak round 1 into round 2 | **Dropped.** `harness/` stays exactly as committed (round 1's record; never re-run). Round 2 is `harness2/`, a copy of `harness/` edited in place for round 2 only: no `round_` argument, no CONFIG table. Round 1's tests that still apply are copied into `harness2/tests` and adapted; T23-T43 are added. No separate RED subagent: the builder writes each test, runs it, records it failing, then implements. |
| m1 | T23 compares the file to a constant derived from it | T23 also asserts `FROZEN2_SHA256` equals the value recorded in this repo's `frozen2.sha256` file, committed before B1. |
| m2 | `ROW_FIELDS`, `H_TEXT`, `INSTANCES` are module constants | Moot under M5: harness2 sets them for round 2 only. |
| m3 | "H4 on ISM leaves about 27 rows" | Not so: the preview has 80 of 144 ISM receivers at R <= 0.5 m, so about 720 rows at 1 ms before the T60 filter (run1 DEV: 716). The scorer prints H4's n per set. |
| m4 | G7 cannot fire `receiver_too_large` at R = 0.31 m | Known and stated in PREREG-2 (the refusal starts below EDT ~0.09 s). G7 tests the dead-room regime for wrong-silent on SPPS; the refusal is tested by ISM-fresh-2 and Synth-fresh-2 across radii. |
| m5 | Attack class draws may repeat round 1's | The attack draws take a new seed, 2026100203; T37 checks freshness of Synth specs by P3-style tolerance, not exact equality. |

Open questions 1 and 2 are taken at their defaults (no G8; freeze to verdict without a stop), per Burhan's 2026-10-01 10:59 "keep building and building and finish the work".
